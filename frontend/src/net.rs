//! REST 조회 + WebSocket 실시간 매칭 연결.
//!
//! 빌드 시 `BACKEND_URL` 환경변수가 주어지면 실서버에 연결하고,
//! 없으면 내장 시드 데이터로 동작하는 오프라인 데모 모드로 떨어진다.

use dioxus::prelude::*;
use futures::channel::mpsc;
use futures::{SinkExt, StreamExt};
use gloo_net::http::Request;
use gloo_net::websocket::{futures::WebSocket, Message};
use shared::{CalendarEntry, ClientMsg, Game, ServerMsg, Team};

use crate::state::AppCtx;

/// 컴파일 타임에 주입되는 백엔드 베이스 URL (예: https://scrim.fly.dev).
pub fn backend_base() -> String {
    option_env!("BACKEND_URL")
        .unwrap_or("")
        .trim_end_matches('/')
        .to_string()
}

/// 데모용 캘린더 폴백(백엔드와 동일 구성).
fn fallback_calendar() -> Vec<CalendarEntry> {
    vec![
        CalendarEntry { date: "2026-06-15".into(), opponent: "GANDONG".into(), game: Game::Lol, result: None },
        CalendarEntry { date: "2026-06-18".into(), opponent: "DRX".into(), game: Game::Lol, result: None },
        CalendarEntry { date: "2026-06-21".into(), opponent: "Gen.G".into(), game: Game::Lol, result: Some("2-1 W".into()) },
        CalendarEntry { date: "2026-07-02".into(), opponent: "KT Rolster".into(), game: Game::Valorant, result: None },
    ]
}

pub async fn fetch_teams() -> Vec<Team> {
    let base = backend_base();
    if base.is_empty() {
        return shared::seed::seed_teams();
    }
    match Request::get(&format!("{base}/api/teams")).send().await {
        Ok(r) => r.json::<Vec<Team>>().await.unwrap_or_else(|_| shared::seed::seed_teams()),
        Err(_) => shared::seed::seed_teams(),
    }
}

pub async fn fetch_calendar() -> Vec<CalendarEntry> {
    let base = backend_base();
    if base.is_empty() {
        return fallback_calendar();
    }
    match Request::get(&format!("{base}/api/calendar")).send().await {
        Ok(r) => r.json::<Vec<CalendarEntry>>().await.unwrap_or_else(|_| fallback_calendar()),
        Err(_) => fallback_calendar(),
    }
}

fn handle_server_msg(ctx: AppCtx, msg: ServerMsg) {
    use crate::state::{ChatMsg, InboxItem, Screen, Thread};
    let mut status = ctx.status;
    let mut my_team = ctx.my_team;
    let mut listings = ctx.listings;
    let mut outgoing = ctx.outgoing;
    let mut inbox = ctx.inbox;
    let mut threads = ctx.threads;
    let mut active = ctx.active;
    let mut searching = ctx.searching;

    match msg {
        ServerMsg::Welcome { team } => {
            my_team.set(Some(team));
            status.set("실서버 연결됨".into());
        }
        ServerMsg::ScrimList { listings: l } => {
            listings.set(l);
        }
        ServerMsg::InviteIncoming { match_id, from } => {
            let mut list = inbox.read().clone();
            if !list.iter().any(|i| i.match_id == match_id) {
                list.push(InboxItem { match_id, from: from.clone() });
                inbox.set(list);
            }
            status.set(format!("📩 {} 가 스크림을 신청했습니다", from.name));
        }
        ServerMsg::InviteSent { match_id, to } => {
            outgoing.set(Some((match_id, to)));
        }
        ServerMsg::InviteRejected { .. } => {
            outgoing.set(None);
            status.set("상대가 신청을 거절했습니다".into());
        }
        ServerMsg::MatchConfirmed { match_id, scrim, opponent } => {
            searching.set(false);
            listings.set(Vec::new());
            outgoing.set(None);
            // 수신함에서 제거
            let remaining: Vec<_> = inbox.read().clone().into_iter().filter(|i| i.match_id != match_id).collect();
            inbox.set(remaining);
            // 스레드 추가(중복 방지)
            let mut th = threads.read().clone();
            if !th.iter().any(|t| t.match_id == match_id) {
                th.push(Thread { match_id: match_id.clone(), opponent: opponent.clone(), scrim, chat: Vec::new(), unread: 0 });
                threads.set(th);
            }
            active.set(Some(match_id));
            status.set(format!("✅ 매칭 확정! vs {}", opponent.name));
            let mut s = ctx.screen;
            s.set(Screen::Messages);
        }
        ServerMsg::Chat { match_id, from_name, text, .. } => {
            let active_id = ctx.active.read().clone();
            let mut th = threads.read().clone();
            if let Some(t) = th.iter_mut().find(|t| t.match_id == match_id) {
                t.chat.push(ChatMsg { mine: false, name: from_name, text });
                if active_id.as_deref() != Some(match_id.as_str()) {
                    t.unread += 1;
                }
                threads.set(th);
            }
        }
        ServerMsg::Error { message } => status.set(format!("오류: {message}")),
    }
}

/// 로그인 시 호출: WebSocket 을 열고 Hello 인증 → 송수신 펌프를 띄운다.
pub fn connect(ctx: AppCtx, serial: String, team_id: String, game: Game) {
    let base = backend_base();
    let mut online = ctx.online;
    let mut status = ctx.status;

    if base.is_empty() {
        online.set(false);
        status.set("오프라인 데모 모드 (백엔드 미연결)".into());
        return;
    }

    let ws_url = format!("{}/ws", base.replacen("http", "ws", 1));
    let ws = match WebSocket::open(&ws_url) {
        Ok(w) => w,
        Err(_) => {
            online.set(false);
            status.set("WS 연결 실패 — 오프라인 모드".into());
            return;
        }
    };

    let (mut write, mut read) = ws.split();
    let (tx, mut rx) = mpsc::unbounded::<ClientMsg>();

    let mut ws_tx = ctx.ws_tx;
    ws_tx.set(Some(tx.clone()));
    let _ = tx.unbounded_send(ClientMsg::Hello { serial, team_id, game });

    // 송신 펌프: 채널 → 소켓.
    spawn(async move {
        while let Some(msg) = rx.next().await {
            if let Ok(txt) = serde_json::to_string(&msg) {
                if write.send(Message::Text(txt)).await.is_err() {
                    break;
                }
            }
        }
    });

    // 수신 펌프: 소켓 → 시그널.
    spawn(async move {
        online.set(true);
        while let Some(Ok(msg)) = read.next().await {
            if let Message::Text(txt) = msg {
                if let Ok(sm) = serde_json::from_str::<ServerMsg>(&txt) {
                    handle_server_msg(ctx, sm);
                }
            }
        }
        online.set(false);
    });
}
