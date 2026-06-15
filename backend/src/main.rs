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
    CalendarEntry, ClientMsg, Game, MatchStatus, ScrimMatch, ServerMsg, Team,
};
use tokio::sync::mpsc::{self, UnboundedSender};
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;

/// 대기열 한 칸: 어떤 팀이 어떤 슬롯(종목·날짜·시간)을 찾는지.
#[derive(Clone)]
struct QueueEntry {
    team_id: String,
    game: Game,
    date: String,
    time: String,
    /// 같은 지역만 원할 때의 필터.
    region: Option<String>,
    /// 지정 스크림: 이 팀하고만 매칭.
    target: Option<String>,
}

/// 진행 중인 매칭 레코드. 양쪽 수락 여부를 추적.
struct MatchRecord {
    scrim: ScrimMatch,
    accepted_a: bool,
    accepted_b: bool,
}

#[derive(Default)]
struct Inner {
    /// team_id → 그 팀 소켓으로 메시지를 흘려보내는 송신부.
    clients: HashMap<String, UnboundedSender<ServerMsg>>,
    queue: Vec<QueueEntry>,
    matches: HashMap<String, MatchRecord>,
}

struct AppState {
    teams: Vec<Team>,
    inner: Mutex<Inner>,
    /// Postgres 풀(없으면 인메모리 전용).
    pool: Option<sqlx::PgPool>,
}

impl AppState {
    fn find_team(&self, id: &str) -> Option<Team> {
        self.teams.iter().find(|t| t.id == id).cloned()
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
        teams,
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
    Json(state.teams.clone())
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
        let cal = db::load_calendar(pool, &state.teams).await;
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

async fn handle_socket(socket: WebSocket, state: Shared) {
    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMsg>();

    // 서버 → 클라이언트 송신 펌프.
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

    // 이 소켓이 인증한 팀 정보.
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
                let inner = state.inner.lock().unwrap();
                if let Some((id, _)) = &me {
                    send_to(&inner, id, ServerMsg::Error { message: format!("bad message: {e}") });
                }
                drop(inner);
                continue;
            }
        };

        match client_msg {
            ClientMsg::Hello { serial, team_id, game } => {
                // 비밀 보장: 시리얼 코드가 해당 팀의 공식 코드와 일치해야 접속 가능.
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
            }

            ClientMsg::FindScrim { date, time, region, target_team: target } => {
                let Some((my_id, my_game)) = me.clone() else {
                    let _ = tx.send(ServerMsg::Error { message: "먼저 Hello 로 인증하세요".into() });
                    continue;
                };
                let my_region = state.find_team(&my_id).map(|t| t.region);
                let mut inner = state.inner.lock().unwrap();

                // 같은 슬롯 + 필터 조건을 만족하는 다른 팀이 대기열에 있나?
                let opponent_pos = inner.queue.iter().position(|q| {
                    if q.game != my_game || q.date != date || q.time != time || q.team_id == my_id {
                        return false;
                    }
                    // 지정 스크림: 내 target 이 있으면 상대가 그 팀이어야 하고,
                    // 상대의 target 이 있으면 그게 나여야 한다.
                    if let Some(t) = &target {
                        if &q.team_id != t {
                            return false;
                        }
                    }
                    if let Some(t) = &q.target {
                        if t != &my_id {
                            return false;
                        }
                    }
                    // 지역 필터: 양쪽 중 하나라도 region 을 걸면 상대 지역과 일치해야 한다.
                    let opp_region = state.find_team(&q.team_id).map(|t| t.region);
                    if let Some(r) = &region {
                        if opp_region.as_deref() != Some(r.as_str()) {
                            return false;
                        }
                    }
                    if let Some(r) = &q.region {
                        if my_region.as_deref() != Some(r.as_str()) {
                            return false;
                        }
                    }
                    true
                });

                if let Some(pos) = opponent_pos {
                    let opp = inner.queue.remove(pos);
                    let scrim = ScrimMatch {
                        id: Uuid::new_v4().to_string(),
                        team_a: opp.team_id.clone(),
                        team_b: my_id.clone(),
                        game: my_game,
                        date: date.clone(),
                        time: time.clone(),
                        code: gen_code(),
                        status: MatchStatus::Pending,
                    };
                    inner.matches.insert(
                        scrim.id.clone(),
                        MatchRecord { scrim: scrim.clone(), accepted_a: false, accepted_b: false },
                    );
                    send_to(&inner, &opp.team_id, ServerMsg::MatchOffer { scrim: scrim.clone() });
                    send_to(&inner, &my_id, ServerMsg::MatchOffer { scrim });
                } else {
                    // 중복 큐 방지 후 대기열 진입.
                    inner.queue.retain(|q| q.team_id != my_id);
                    inner.queue.push(QueueEntry {
                        team_id: my_id.clone(),
                        game: my_game,
                        date,
                        time,
                        region,
                        target,
                    });
                    send_to(&inner, &my_id, ServerMsg::Queued);
                }
            }

            ClientMsg::Apply { match_id } => {
                let Some((my_id, _)) = me.clone() else { continue };
                let mut inner = state.inner.lock().unwrap();
                let (update, a, b) = {
                    let Some(rec) = inner.matches.get_mut(&match_id) else {
                        let _ = tx.send(ServerMsg::Error { message: "존재하지 않는 매칭".into() });
                        continue;
                    };
                    if rec.scrim.team_a == my_id {
                        rec.accepted_a = true;
                    } else if rec.scrim.team_b == my_id {
                        rec.accepted_b = true;
                    }
                    rec.scrim.status = if rec.accepted_a && rec.accepted_b {
                        MatchStatus::Confirmed
                    } else {
                        MatchStatus::Applied
                    };
                    (rec.scrim.clone(), rec.scrim.team_a.clone(), rec.scrim.team_b.clone())
                };
                send_to(&inner, &a, ServerMsg::MatchUpdate { scrim: update.clone() });
                send_to(&inner, &b, ServerMsg::MatchUpdate { scrim: update.clone() });
                drop(inner);

                // 확정된 매칭은 Postgres 에 영속화.
                if update.status == MatchStatus::Confirmed {
                    if let Some(pool) = state.pool.clone() {
                        tokio::spawn(async move { db::persist_match(&pool, &update).await; });
                    }
                }
            }

            ClientMsg::Deny { match_id } => {
                let mut inner = state.inner.lock().unwrap();
                if let Some(mut rec) = inner.matches.remove(&match_id) {
                    rec.scrim.status = MatchStatus::Denied;
                    let (a, b) = (rec.scrim.team_a.clone(), rec.scrim.team_b.clone());
                    send_to(&inner, &a, ServerMsg::MatchUpdate { scrim: rec.scrim.clone() });
                    send_to(&inner, &b, ServerMsg::MatchUpdate { scrim: rec.scrim });
                }
            }

            ClientMsg::Cancel => {
                if let Some((my_id, _)) = me.clone() {
                    let mut inner = state.inner.lock().unwrap();
                    inner.queue.retain(|q| q.team_id != my_id);
                }
            }
        }
    }

    // 정리: 클라이언트/대기열 제거.
    if let Some((my_id, _)) = me {
        let mut inner = state.inner.lock().unwrap();
        inner.clients.remove(&my_id);
        inner.queue.retain(|q| q.team_id != my_id);
    }
    send_task.abort();
}
