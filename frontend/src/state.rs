//! 전역 앱 상태(Dioxus 시그널 묶음)와 화면 전환 정의.

use dioxus::prelude::*;
use futures::channel::mpsc::UnboundedSender;
use shared::{ClientMsg, Game, ScrimMatch, Team};

/// 최상위 화면.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Login,
    Matching,
    Team,
    Calendar,
}

/// 컨텍스트로 공유되는 앱 상태. 모든 필드가 Signal 이라 Copy.
#[derive(Clone, Copy)]
pub struct AppCtx {
    pub screen: Signal<Screen>,
    pub serial: Signal<String>,
    pub game: Signal<Game>,
    pub teams: Signal<Vec<Team>>,
    pub my_team: Signal<Option<Team>>,
    pub opponent_id: Signal<Option<String>>,
    pub current_match: Signal<Option<ScrimMatch>>,
    pub status: Signal<String>,
    pub online: Signal<bool>,
    pub ws_tx: Signal<Option<UnboundedSender<ClientMsg>>>,
}

impl AppCtx {
    /// 컴포넌트 안에서 시그널을 생성해 컨텍스트 인스턴스를 만든다.
    pub fn new() -> Self {
        Self {
            screen: use_signal(|| Screen::Login),
            serial: use_signal(String::new),
            game: use_signal(|| Game::Lol),
            teams: use_signal(Vec::new),
            my_team: use_signal(|| Option::<Team>::None),
            opponent_id: use_signal(|| Option::<String>::None),
            current_match: use_signal(|| Option::<ScrimMatch>::None),
            status: use_signal(String::new),
            online: use_signal(|| false),
            ws_tx: use_signal(|| Option::<UnboundedSender<ClientMsg>>::None),
        }
    }

    /// WebSocket 으로 클라이언트 메시지를 보낸다(연결돼 있을 때만).
    pub fn send(self, msg: ClientMsg) {
        if let Some(tx) = self.ws_tx.read().as_ref() {
            let _ = tx.unbounded_send(msg);
        }
    }

    pub fn goto(self, screen: Screen) {
        let mut s = self.screen;
        s.set(screen);
    }
}
