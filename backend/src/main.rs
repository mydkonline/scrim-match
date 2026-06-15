//! Scrim.GG 백엔드 — Axum REST + WebSocket 실시간 스크림 매칭.
//!
//! - REST:  팀/로스터/캘린더 조회
//! - WS:    `/ws` 에서 시리얼 코드 인증 → 슬롯 기반 페어링 → 수락/확정/거절
//!
//! 상태는 MVP 단계라 인메모리로 관리합니다(운영 시 Postgres 등으로 교체).

mod db;

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use futures::{SinkExt, StreamExt};
use shared::{
    CalendarEntry, ClientMsg, Game, Listing, MatchStatus, ScrimMatch, ServerMsg, Team,
};
use std::time::Duration;
use tokio::sync::mpsc::{self, UnboundedSender};
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;

/// 검색 풀 한 칸: 어떤 팀이 어떤 슬롯을 찾는지.
#[derive(Clone)]
struct PoolEntry {
    team_id: String,
    game: Game,
    date: String,
    time: String,
    region: Option<String>,
}

/// 진행 중(Pending)/확정 매칭. 신청자→피신청자.
#[derive(Clone)]
struct MatchRec {
    scrim: ScrimMatch,
    inviter: String,
    invitee: String,
}

/// 코드로 대기 중인 신청(메시지 큐).
#[derive(Clone)]
struct Pending {
    code: String,
    from: String,
    to: String,
    date: String,
    time: String,
}

#[derive(Default)]
struct Inner {
    /// team_id → 그 팀 소켓으로 메시지를 흘려보내는 송신부.
    clients: HashMap<String, UnboundedSender<ServerMsg>>,
    /// 현재 스크림 상대를 찾는 팀들.
    pool: Vec<PoolEntry>,
    matches: HashMap<String, MatchRec>,
    /// 코드 발급된 대기 신청들.
    pending: Vec<Pending>,
}

struct AppState {
    /// 로스터 변경(드래그 스왑)을 반영하기 위해 Mutex.
    teams: Mutex<Vec<Team>>,
    inner: Mutex<Inner>,
    /// Postgres 풀(없으면 인메모리 전용).
    pool: Option<sqlx::PgPool>,
}

impl AppState {
    fn find_team(&self, id: &str) -> Option<Team> {
        self.teams.lock().unwrap().iter().find(|t| t.id == id).cloned()
    }
    fn all_teams(&self) -> Vec<Team> {
        self.teams.lock().unwrap().clone()
    }
}

type Shared = Arc<AppState>;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "backend=info,tower_http=info".into()),
        )
        .init();

    // DATABASE_URL 이 있으면 Postgres 에서 팀을 로드, 없으면 시드로 폴백.
    let (teams, pool) = match std::env::var("DATABASE_URL").ok() {
        Some(url) => match db::init(&url).await {
            Ok(p) => {
                let t = db::load_teams(&p).await;
                tracing::info!("Postgres 연결됨 — 팀 {}개 로드", t.len());
                (t, Some(p))
            }
            Err(e) => {
                tracing::error!("Postgres 초기화 실패({e}) — 시드로 폴백");
                (shared::seed::seed_teams(), None)
            }
        },
        None => {
            tracing::info!("DATABASE_URL 없음 — 인메모리 시드 사용");
            (shared::seed::seed_teams(), None)
        }
    };

    let state: Shared = Arc::new(AppState {
        teams: Mutex::new(teams),
        inner: Mutex::new(Inner::default()),
        pool,
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/teams", get(list_teams))
        .route("/api/teams/:id", get(get_team))
        .route("/api/calendar", get(calendar))
        .route("/ws", get(ws_handler))
        .layer(cors)
        .with_state(state);

    // 호스팅 환경(PORT 환경변수)과 로컬(기본 8080) 모두 지원.
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    tracing::info!("Scrim.GG backend listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// ───────────────────────────── REST ─────────────────────────────

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok", "service": "scrim-gg" }))
}

async fn list_teams(State(state): State<Shared>) -> impl IntoResponse {
    Json(state.all_teams())
}

async fn get_team(
    State(state): State<Shared>,
    Path(id): Path<String>,
) -> Result<Json<Team>, StatusCode> {
    state
        .find_team(&id)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

fn fallback_calendar() -> Vec<CalendarEntry> {
    vec![
        CalendarEntry { date: "2026-06-15".into(), opponent: "GANDONG".into(), game: Game::Lol, result: None },
        CalendarEntry { date: "2026-06-18".into(), opponent: "DRX".into(), game: Game::Lol, result: None },
        CalendarEntry { date: "2026-06-21".into(), opponent: "Gen.G".into(), game: Game::Lol, result: Some("2-1 W".into()) },
        CalendarEntry { date: "2026-06-25".into(), opponent: "KT Rolster".into(), game: Game::Valorant, result: None },
    ]
}

/// DB 연결 시 확정 매칭 기반, 없으면 데모 폴백.
async fn calendar(State(state): State<Shared>) -> impl IntoResponse {
    if let Some(pool) = &state.pool {
        let teams = state.all_teams();
        let cal = db::load_calendar(pool, &teams).await;
        if !cal.is_empty() {
            return Json(cal);
        }
    }
    Json(fallback_calendar())
}

// ────────────────────────── WebSocket ──────────────────────────

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Shared>) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

fn gen_code() -> String {
    let n = Uuid::new_v4().as_u128() % 1_000_000;
    format!("{n:06}")
}

fn send_to(inner: &Inner, team_id: &str, msg: ServerMsg) {
    if let Some(tx) = inner.clients.get(team_id) {
        let _ = tx.send(msg);
    }
}

fn region_ok(
    my_region: Option<&str>,
    my_filter: &Option<String>,
    other_region: Option<&str>,
    other_filter: &Option<String>,
) -> bool {
    if let Some(r) = my_filter {
        if other_region != Some(r.as_str()) {
            return false;
        }
    }
    if let Some(r) = other_filter {
        if my_region != Some(r.as_str()) {
            return false;
        }
    }
    true
}

/// 특정 검색자 기준으로 같은 슬롯의 실제 검색 팀 + 데모 봇 목록을 만든다.
fn compute_listings(state: &Shared, inner: &Inner, me: &PoolEntry) -> Vec<Listing> {
    let teams = state.all_teams();
    let team_by = |id: &str| teams.iter().find(|t| t.id == id).cloned();
    let my_region = team_by(&me.team_id).map(|t| t.region);
    let mut out = Vec::new();

    // 데모: 일정(날짜/시간) 무관하게 같은 종목이면 매칭.
    for q in &inner.pool {
        if q.team_id == me.team_id || q.game != me.game {
            continue;
        }
        let q_region = team_by(&q.team_id).map(|t| t.region);
        if !region_ok(my_region.as_deref(), &me.region, q_region.as_deref(), &q.region) {
            continue;
        }
        if let Some(t) = team_by(&q.team_id) {
            out.push(Listing::from_team(&t, !inner.clients.contains_key(&q.team_id)));
        }
    }

    // 데모 봇: 같은 종목의 시드 팀(접속/검색중 아님) 최대 6팀.
    let mut demo = 0;
    for t in &teams {
        if demo >= 6 {
            break;
        }
        if t.game != me.game || t.id == me.team_id {
            continue;
        }
        if inner.clients.contains_key(&t.id) || inner.pool.iter().any(|q| q.team_id == t.id) {
            continue;
        }
        if let Some(r) = &me.region {
            if &t.region != r {
                continue;
            }
        }
        out.push(Listing::from_team(t, true));
        demo += 1;
    }
    out
}

/// 풀이 바뀔 때 검색 중인 모든 실제 클라이언트에 개인화된 리스트를 푸시.
fn broadcast_lists(state: &Shared, inner: &Inner) {
    let pool = inner.pool.clone();
    for entry in &pool {
        if !inner.clients.contains_key(&entry.team_id) {
            continue;
        }
        let listings = compute_listings(state, inner, entry);
        send_to(inner, &entry.team_id, ServerMsg::ScrimList { listings });
    }
}

/// 매칭을 확정 처리하고 양쪽(실 접속자)에 MatchConfirmed 전송, 풀에서 제거.
fn confirm_and_notify(state: &Shared, inner: &mut Inner, match_id: &str) -> Option<ScrimMatch> {
    let (scrim, inviter, invitee) = {
        let rec = inner.matches.get_mut(match_id)?;
        rec.scrim.status = MatchStatus::Confirmed;
        (rec.scrim.clone(), rec.inviter.clone(), rec.invitee.clone())
    };
    let invitee_live = inner.clients.contains_key(&invitee);
    let inviter_live = inner.clients.contains_key(&inviter);
    if let Some(t) = state.find_team(&invitee) {
        send_to(inner, &inviter, ServerMsg::MatchConfirmed {
            match_id: match_id.to_string(),
            scrim: scrim.clone(),
            opponent: Listing::from_team(&t, !invitee_live),
        });
    }
    if let Some(t) = state.find_team(&inviter) {
        send_to(inner, &invitee, ServerMsg::MatchConfirmed {
            match_id: match_id.to_string(),
            scrim: scrim.clone(),
            opponent: Listing::from_team(&t, !inviter_live),
        });
    }
    inner.pool.retain(|q| q.team_id != inviter && q.team_id != invitee);
    Some(scrim)
}

fn persist(state: &Shared, scrim: ScrimMatch) {
    if let Some(pool) = state.pool.clone() {
        tokio::spawn(async move { db::persist_match(&pool, &scrim).await; });
    }
}

/// 데모: 검색 시작 후 잠시 뒤 시드 팀이 사용자에게 스크림을 신청(수락/거절 체험용).
fn schedule_demo_invite(
    state: Shared,
    my_id: String,
    game: Game,
    date: String,
    time: String,
    region: Option<String>,
) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(3200)).await;
        let mut inner = state.inner.lock().unwrap();
        if !inner.pool.iter().any(|q| q.team_id == my_id) {
            return; // 검색 중단됨
        }
        if inner.matches.values().any(|r| r.inviter == my_id || r.invitee == my_id) {
            return; // 이미 진행 중인 매칭 있음
        }
        let teams = state.all_teams();
        let pick = teams
            .iter()
            .find(|t| {
                t.game == game
                    && t.id != my_id
                    && !inner.clients.contains_key(&t.id)
                    && !inner.pool.iter().any(|q| q.team_id == t.id)
                    && region.as_ref().map_or(true, |r| &t.region == r)
            })
            .cloned();
        if let Some(d) = pick {
            let match_id = Uuid::new_v4().to_string();
            let scrim = ScrimMatch {
                id: match_id.clone(),
                team_a: d.id.clone(),
                team_b: my_id.clone(),
                game,
                date,
                time,
                code: gen_code(),
                status: MatchStatus::Pending,
            };
            inner.matches.insert(match_id.clone(), MatchRec { scrim, inviter: d.id.clone(), invitee: my_id.clone() });
            send_to(&inner, &my_id, ServerMsg::InviteIncoming { match_id, from: Listing::from_team(&d, true) });
        }
    });
}

/// 데모: 사용자가 봇에게 신청하면 잠시 뒤 자동 수락.
fn schedule_demo_accept(state: Shared, match_id: String) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(1800)).await;
        let mut inner = state.inner.lock().unwrap();
        let scrim = confirm_and_notify(&state, &mut inner, &match_id);
        broadcast_lists(&state, &inner);
        drop(inner);
        if let Some(s) = scrim {
            persist(&state, s);
        }
    });
}

/// 데모: 봇 상대에게 보낸 채팅에 자동 응답.
fn schedule_demo_chat(state: Shared, match_id: String, demo_team: String, user_team: String) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(1300)).await;
        let inner = state.inner.lock().unwrap();
        let name = state.find_team(&demo_team).map(|t| t.name).unwrap_or_else(|| demo_team.clone());
        let replies = [
            "좋습니다! 그 시간 가능합니다 👍",
            "콜! 디스코드로 바로 들어갈게요",
            "오케이, 풀 5인 준비됐습니다",
            "넵 그때 봬요. 코드 확인했습니다",
        ];
        let idx = match_id.bytes().map(|b| b as usize).sum::<usize>() % replies.len();
        send_to(&inner, &user_team, ServerMsg::Chat {
            match_id: match_id.clone(),
            from_team: demo_team.clone(),
            from_name: name,
            text: replies[idx].to_string(),
        });
    });
}

async fn handle_socket(socket: WebSocket, state: Shared) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMsg>();

    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let txt = match serde_json::to_string(&msg) {
                Ok(t) => t,
                Err(_) => continue,
            };
            if sink.send(Message::Text(txt)).await.is_err() {
                break;
            }
        }
    });

    let mut me: Option<(String, Game)> = None;

    while let Some(Ok(msg)) = stream.next().await {
        let text = match msg {
            Message::Text(t) => t,
            Message::Close(_) => break,
            _ => continue,
        };
        let client_msg: ClientMsg = match serde_json::from_str(&text) {
            Ok(m) => m,
            Err(e) => {
                let _ = tx.send(ServerMsg::Error { message: format!("bad message: {e}") });
                continue;
            }
        };

        match client_msg {
            ClientMsg::Hello { serial, team_id, game } => {
                if serial.trim() != shared::serial_for(&team_id) {
                    let _ = tx.send(ServerMsg::Error { message: "시리얼 코드가 팀과 일치하지 않습니다".into() });
                    continue;
                }
                let Some(team) = state.find_team(&team_id) else {
                    let _ = tx.send(ServerMsg::Error { message: "알 수 없는 팀".into() });
                    continue;
                };
                {
                    let mut inner = state.inner.lock().unwrap();
                    inner.clients.insert(team_id.clone(), tx.clone());
                }
                me = Some((team_id.clone(), game));
                let _ = tx.send(ServerMsg::Welcome { team });
                tracing::info!("team {team_id} authenticated for {:?}", game);

                // 접속 시: 나에게 온 대기 신청(메시지 큐)을 전달 — 브라우저를 꺼놨어도 받음.
                {
                    let inner = state.inner.lock().unwrap();
                    let mine: Vec<Pending> = inner.pending.iter().filter(|p| p.to == team_id).cloned().collect();
                    for p in mine {
                        if let Some(t) = state.find_team(&p.from) {
                            let _ = tx.send(ServerMsg::InviteIncoming {
                                match_id: p.code.clone(),
                                from: Listing::from_team(&t, !inner.clients.contains_key(&p.from)),
                            });
                        }
                    }
                }
            }

            ClientMsg::Search { date, time, region } => {
                let Some((my_id, my_game)) = me.clone() else {
                    let _ = tx.send(ServerMsg::Error { message: "먼저 Hello 로 인증하세요".into() });
                    continue;
                };
                {
                    let mut inner = state.inner.lock().unwrap();
                    inner.pool.retain(|q| q.team_id != my_id);
                    inner.pool.push(PoolEntry {
                        team_id: my_id.clone(),
                        game: my_game,
                        date: date.clone(),
                        time: time.clone(),
                        region: region.clone(),
                    });
                    broadcast_lists(&state, &inner);
                }
                schedule_demo_invite(state.clone(), my_id, my_game, date, time, region);
            }

            ClientMsg::StopSearch => {
                if let Some((my_id, _)) = me.clone() {
                    let mut inner = state.inner.lock().unwrap();
                    inner.pool.retain(|q| q.team_id != my_id);
                    broadcast_lists(&state, &inner);
                }
            }

            ClientMsg::ApplyQueue { target_team, date, time, squad: _ } => {
                let Some((my_id, _)) = me.clone() else { continue };
                let Some(target) = state.find_team(&target_team) else {
                    let _ = tx.send(ServerMsg::Error { message: "알 수 없는 상대".into() });
                    continue;
                };
                let code = gen_code();
                let mut inner = state.inner.lock().unwrap();
                inner.pending.retain(|p| !(p.from == my_id && p.to == target_team));
                inner.pending.push(Pending {
                    code: code.clone(),
                    from: my_id.clone(),
                    to: target_team.clone(),
                    date,
                    time,
                });
                let target_live = inner.clients.contains_key(&target_team);
                send_to(&inner, &my_id, ServerMsg::Applied {
                    code: code.clone(),
                    to: Listing::from_team(&target, !target_live),
                });
                // 상대가 접속 중이면 즉시 신청 푸시(브라우저/터미널 모두 수신).
                if target_live {
                    if let Some(t) = state.find_team(&my_id) {
                        send_to(&inner, &target_team, ServerMsg::InviteIncoming {
                            match_id: code,
                            from: Listing::from_team(&t, false),
                        });
                    }
                }
            }

            ClientMsg::AcceptCode { code } => {
                let Some((my_id, _)) = me.clone() else { continue };
                let mut inner = state.inner.lock().unwrap();
                let pos = inner.pending.iter().position(|p| p.code == code);
                let Some(pos) = pos else {
                    let _ = tx.send(ServerMsg::Error { message: "코드를 찾을 수 없습니다".into() });
                    continue;
                };
                let p = inner.pending.remove(pos);
                let game = state.find_team(&p.from).map(|t| t.game).unwrap_or(Game::Lol);
                let scrim = ScrimMatch {
                    id: Uuid::new_v4().to_string(),
                    team_a: p.from.clone(),
                    team_b: my_id.clone(),
                    game,
                    date: p.date.clone(),
                    time: p.time.clone(),
                    code: p.code.clone(),
                    status: MatchStatus::Confirmed,
                };
                let my_live = inner.clients.contains_key(&my_id);
                let from_live = inner.clients.contains_key(&p.from);
                if let Some(t) = state.find_team(&my_id) {
                    send_to(&inner, &p.from, ServerMsg::MatchConfirmed {
                        match_id: scrim.id.clone(),
                        scrim: scrim.clone(),
                        opponent: Listing::from_team(&t, !my_live),
                    });
                }
                if let Some(t) = state.find_team(&p.from) {
                    send_to(&inner, &my_id, ServerMsg::MatchConfirmed {
                        match_id: scrim.id.clone(),
                        scrim: scrim.clone(),
                        opponent: Listing::from_team(&t, !from_live),
                    });
                }
                drop(inner);
                persist(&state, scrim);
            }

            ClientMsg::Invite { target_team } => {
                let Some((my_id, my_game)) = me.clone() else { continue };
                let Some(target) = state.find_team(&target_team) else {
                    let _ = tx.send(ServerMsg::Error { message: "알 수 없는 상대".into() });
                    continue;
                };
                let mut inner = state.inner.lock().unwrap();
                let Some((date, time)) = inner
                    .pool
                    .iter()
                    .find(|q| q.team_id == my_id)
                    .map(|q| (q.date.clone(), q.time.clone()))
                else {
                    drop(inner);
                    let _ = tx.send(ServerMsg::Error { message: "먼저 스크림을 검색하세요".into() });
                    continue;
                };
                let match_id = Uuid::new_v4().to_string();
                let scrim = ScrimMatch {
                    id: match_id.clone(),
                    team_a: my_id.clone(),
                    team_b: target_team.clone(),
                    game: my_game,
                    date,
                    time,
                    code: gen_code(),
                    status: MatchStatus::Pending,
                };
                inner.matches.insert(match_id.clone(), MatchRec {
                    scrim,
                    inviter: my_id.clone(),
                    invitee: target_team.clone(),
                });
                let target_live = inner.clients.contains_key(&target_team);
                send_to(&inner, &my_id, ServerMsg::InviteSent {
                    match_id: match_id.clone(),
                    to: Listing::from_team(&target, !target_live),
                });
                if target_live {
                    if let Some(mine) = state.find_team(&my_id) {
                        send_to(&inner, &target_team, ServerMsg::InviteIncoming {
                            match_id,
                            from: Listing::from_team(&mine, false),
                        });
                    }
                } else {
                    drop(inner);
                    schedule_demo_accept(state.clone(), match_id);
                }
            }

            ClientMsg::Accept { match_id } => {
                let mut inner = state.inner.lock().unwrap();
                let scrim = confirm_and_notify(&state, &mut inner, &match_id);
                broadcast_lists(&state, &inner);
                drop(inner);
                if let Some(s) = scrim {
                    persist(&state, s);
                }
            }

            ClientMsg::Reject { match_id } => {
                let mut inner = state.inner.lock().unwrap();
                if let Some(rec) = inner.matches.remove(&match_id) {
                    send_to(&inner, &rec.inviter, ServerMsg::InviteRejected { match_id });
                }
            }

            ClientMsg::Chat { match_id, text } => {
                let Some((my_id, _)) = me.clone() else { continue };
                let inner = state.inner.lock().unwrap();
                let Some(rec) = inner.matches.get(&match_id).cloned() else { continue };
                let other = if rec.inviter == my_id { rec.invitee.clone() } else { rec.inviter.clone() };
                let from_name = state.find_team(&my_id).map(|t| t.name).unwrap_or_else(|| my_id.clone());
                if inner.clients.contains_key(&other) {
                    send_to(&inner, &other, ServerMsg::Chat {
                        match_id: match_id.clone(),
                        from_team: my_id.clone(),
                        from_name,
                        text,
                    });
                } else {
                    drop(inner);
                    schedule_demo_chat(state.clone(), match_id, other, my_id);
                }
            }

            ClientMsg::MovePlayer { player_id, squad } => {
                let Some((my_id, _)) = me.clone() else { continue };
                // 내 팀 선수만 변경 가능. 인메모리 즉시 반영.
                let mut belongs = false;
                {
                    let mut teams = state.teams.lock().unwrap();
                    if let Some(t) = teams.iter_mut().find(|t| t.id == my_id) {
                        if let Some(p) = t.roster.iter_mut().find(|p| p.id == player_id) {
                            p.squad = squad;
                            belongs = true;
                        }
                    }
                }
                // DB 영속화.
                if belongs {
                    if let Some(pool) = state.pool.clone() {
                        let pid = player_id.clone();
                        tokio::spawn(async move { db::update_player_squad(&pool, &pid, squad).await; });
                    }
                }
            }
        }
    }

    if let Some((my_id, _)) = me {
        let mut inner = state.inner.lock().unwrap();
        inner.clients.remove(&my_id);
        inner.pool.retain(|q| q.team_id != my_id);
        broadcast_lists(&state, &inner);
    }
    send_task.abort();
}

