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
    let mut status = ctx.status;
    let mut current = ctx.current_match;
    let mut my_team = ctx.my_team;
    match msg {
        ServerMsg::Welcome { team } => {
            my_team.set(Some(team));
            status.set("실서버 연결됨".into());
        }
        ServerMsg::Queued => status.set("상대 팀 매칭 대기 중…".into()),
        ServerMsg::MatchOffer { scrim } => {
            current.set(Some(scrim));
            status.set("매칭 제안 도착!".into());
        }
        ServerMsg::MatchUpdate { scrim } => {
            status.set(format!("매칭 상태: {:?}", scrim.status));
            current.set(Some(scrim));
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
