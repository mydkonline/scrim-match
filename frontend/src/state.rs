//! 전역 앱 상태(Dioxus 시그널 묶음)와 화면 전환 정의.

use dioxus::prelude::*;
use futures::channel::mpsc::UnboundedSender;
use shared::{ClientMsg, Game, Listing, ScrimMatch, Team};

/// 최상위 화면.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Login,
    Matching,
    Team,
    Calendar,
}

/// 채팅 한 줄.
#[derive(Debug, Clone, PartialEq)]
pub struct ChatMsg {
    pub mine: bool,
    pub name: String,
    pub text: String,
}

/// 컨텍스트로 공유되는 앱 상태. 모든 필드가 Signal 이라 Copy.
#[derive(Clone, Copy)]
pub struct AppCtx {
    pub screen: Signal<Screen>,
    pub serial: Signal<String>,
    pub game: Signal<Game>,
    pub teams: Signal<Vec<Team>>,
    pub my_team: Signal<Option<Team>>,
    pub online: Signal<bool>,
    pub status: Signal<String>,
    pub ws_tx: Signal<Option<UnboundedSender<ClientMsg>>>,

    // ── 매칭 플로우 ──
    /// 검색 중(지구본 회전).
    pub searching: Signal<bool>,
    /// 현재 슬롯에서 스크림 가능한 팀 목록.
    pub listings: Signal<Vec<Listing>>,
    /// 내가 신청 보낸 상태: (match_id, 상대).
    pub outgoing: Signal<Option<(String, Listing)>>,
    /// 나에게 들어온 신청: (match_id, 신청자).
    pub incoming: Signal<Option<(String, Listing)>>,
    /// 확정된 매칭: (match_id, scrim, 상대).
    pub confirmed: Signal<Option<(String, ScrimMatch, Listing)>>,
    /// 확정 매칭 채팅 로그.
    pub chat_log: Signal<Vec<ChatMsg>>,
}

impl AppCtx {
    pub fn new() -> Self {
        Self {
            screen: use_signal(|| Screen::Login),
            serial: use_signal(String::new),
            game: use_signal(|| Game::Lol),
            teams: use_signal(Vec::new),
            my_team: use_signal(|| Option::<Team>::None),
            online: use_signal(|| false),
            status: use_signal(String::new),
            ws_tx: use_signal(|| Option::<UnboundedSender<ClientMsg>>::None),
            searching: use_signal(|| false),
            listings: use_signal(Vec::new),
            outgoing: use_signal(|| Option::<(String, Listing)>::None),
            incoming: use_signal(|| Option::<(String, Listing)>::None),
            confirmed: use_signal(|| Option::<(String, ScrimMatch, Listing)>::None),
            chat_log: use_signal(Vec::new),
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

    /// 매칭 관련 상태를 모두 초기화(처음으로).
    pub fn reset_matching(self) {
        let mut s = self.searching;
        s.set(false);
        let mut l = self.listings;
        l.set(Vec::new());
        let mut o = self.outgoing;
        o.set(None);
        let mut i = self.incoming;
        i.set(None);
        let mut c = self.confirmed;
        c.set(None);
        let mut ch = self.chat_log;
        ch.set(Vec::new());
    }
}
