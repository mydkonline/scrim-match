//! 전역 앱 상태(Dioxus 시그널 묶음)와 화면 전환 정의.

use dioxus::prelude::*;
use futures::channel::mpsc::UnboundedSender;
use shared::{ClientMsg, Game, Listing, ScrimMatch, Squad, Team};

/// 최상위 화면.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Login,
    Matching,
    Messages,
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

/// 수신함 항목(들어온 스크림 신청).
#[derive(Debug, Clone, PartialEq)]
pub struct InboxItem {
    pub match_id: String,
    pub from: Listing,
}

/// 확정 매칭 대화 스레드.
#[derive(Debug, Clone, PartialEq)]
pub struct Thread {
    pub match_id: String,
    pub opponent: Listing,
    pub scrim: ScrimMatch,
    pub squad_label: String,
    pub chat: Vec<ChatMsg>,
    pub unread: u32,
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

    // ── 스크림 슬롯 선택 ──
    pub scrim_date: Signal<String>,
    pub scrim_time: Signal<String>,
    pub scrim_squad: Signal<Squad>,
    /// 국가(지역) 필터. None = 전체.
    pub scrim_region: Signal<Option<String>>,

    // ── 매칭 플로우 ──
    pub searching: Signal<bool>,
    pub listings: Signal<Vec<Listing>>,
    /// 내가 보낸 신청(메시지 큐 전달, 상대 수락 대기). 실시간 대기 없음.
    pub sent: Signal<Vec<Listing>>,
    /// 수신함: 들어온 스크림 신청들.
    pub inbox: Signal<Vec<InboxItem>>,
    /// 확정 매칭 대화 스레드들.
    pub threads: Signal<Vec<Thread>>,
    /// 메시지 대시보드에서 열려 있는 스레드 match_id.
    pub active: Signal<Option<String>>,
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
            scrim_date: use_signal(|| "2026-06-20".to_string()),
            scrim_time: use_signal(|| "19:00".to_string()),
            scrim_squad: use_signal(|| Squad::First),
            scrim_region: use_signal(|| Option::<String>::None),
            searching: use_signal(|| false),
            listings: use_signal(Vec::new),
            sent: use_signal(Vec::new),
            inbox: use_signal(Vec::new),
            threads: use_signal(Vec::new),
            active: use_signal(|| Option::<String>::None),
        }
    }

    pub fn send(self, msg: ClientMsg) {
        if let Some(tx) = self.ws_tx.read().as_ref() {
            let _ = tx.unbounded_send(msg);
        }
    }

    pub fn goto(self, screen: Screen) {
        let mut s = self.screen;
        s.set(screen);
    }

    /// 검색 관련 상태만 초기화(수신함·대화·보낸신청은 유지).
    pub fn reset_search(self) {
        let mut s = self.searching;
        s.set(false);
        let mut l = self.listings;
        l.set(Vec::new());
    }

    /// 수신함 + 보낸 신청 + 열린 대화의 미읽음 합계(네비 배지).
    pub fn unread_count(self) -> usize {
        let inbox = self.inbox.read().len();
        let sent = self.sent.read().len();
        let threads: u32 = self.threads.read().iter().map(|t| t.unread).sum();
        inbox + sent + threads as usize
    }
}
