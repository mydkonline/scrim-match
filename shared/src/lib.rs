//! 프론트엔드와 백엔드가 공유하는 도메인 타입과 WebSocket 프로토콜 정의.

use serde::{Deserialize, Serialize};

pub mod seed;

/// 팀별 공식 시리얼 코드(결정적 FNV-1a 해시).
///
/// 비밀 보장: 이 코드를 아는 사람만 해당 팀으로 접속 가능.
/// 운영에서는 발급/회수 가능한 비밀로 교체하고 이 함수는 데모/마이그레이션용으로만 사용한다.
pub fn serial_for(team_id: &str) -> String {
    let mut h: u32 = 2166136261;
    for b in team_id.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    let prefix = team_id.split('-').next().unwrap_or("TEAM").to_uppercase();
    format!("{prefix}-{:04}", h % 10000)
}

/// 지원 종목.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Game {
    Lol,
    Valorant,
    Starcraft,
}

impl Game {
    pub fn label(&self) -> &'static str {
        match self {
            Game::Lol => "League of Legends",
            Game::Valorant => "VALORANT",
            Game::Starcraft => "StarCraft",
        }
    }
    pub fn short(&self) -> &'static str {
        match self {
            Game::Lol => "LoL",
            Game::Valorant => "VAL",
            Game::Starcraft => "SC",
        }
    }
}

/// 1군 / 2군 / 아카데미 구분.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Squad {
    First,
    Second,
    Academy,
}

impl Squad {
    pub fn label(&self) -> &'static str {
        match self {
            Squad::First => "1군",
            Squad::Second => "2군",
            Squad::Academy => "Academy",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Player {
    pub id: String,
    pub name: String,
    pub role: String,
    pub squad: Squad,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Staff {
    pub manager: String,
    pub coaches: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Team {
    pub id: String,
    pub name: String,
    pub tag: String,
    pub game: Game,
    pub region: String,
    pub staff: Staff,
    pub roster: Vec<Player>,
}

impl Team {
    pub fn squad(&self, squad: Squad) -> Vec<&Player> {
        self.roster.iter().filter(|p| p.squad == squad).collect()
    }
}

/// 매칭 진행 상태.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchStatus {
    /// 서버가 두 팀을 페어링하고 양쪽에 제안한 상태.
    Pending,
    /// 한쪽이 수락(Apply)한 상태.
    Applied,
    /// 양쪽 모두 수락 → 일정 확정.
    Confirmed,
    /// 거절됨.
    Denied,
}

/// 확정/진행 중인 스크림 한 건.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScrimMatch {
    pub id: String,
    pub team_a: String,
    pub team_b: String,
    pub game: Game,
    pub date: String,
    pub time: String,
    /// 두 팀만 공유하는 비공개 입장 코드.
    pub code: String,
    pub status: MatchStatus,
}

/// 캘린더 한 칸(예정/완료된 스크림).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalendarEntry {
    pub date: String,
    pub opponent: String,
    pub game: Game,
    pub result: Option<String>,
}

// ───────────────────────── WebSocket 프로토콜 ─────────────────────────

/// 클라이언트 → 서버.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMsg {
    /// 접속 인증: 시리얼 코드 + 우리 팀 + 종목.
    Hello {
        serial: String,
        team_id: String,
        game: Game,
    },
    /// 같은 슬롯을 찾는 다른 팀과 페어링 요청.
    /// - `region`: 지정 시 같은 지역 팀하고만 매칭.
    /// - `target_team`: 지정 시 그 팀하고만 매칭(지정 스크림). 없으면 공개 매칭.
    FindScrim {
        date: String,
        time: String,
        #[serde(default)]
        region: Option<String>,
        #[serde(default)]
        target_team: Option<String>,
    },
    /// 매칭 제안 수락.
    Apply { match_id: String },
    /// 매칭 제안 거절.
    Deny { match_id: String },
    /// 대기열 이탈.
    Cancel,
}

/// 서버 → 클라이언트.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMsg {
    /// 인증 성공, 내 팀 정보 반환.
    Welcome { team: Team },
    /// 대기열 진입함.
    Queued,
    /// 상대가 잡혀 매칭이 제안됨.
    MatchOffer { scrim: ScrimMatch },
    /// 매칭 상태 변경(수락/확정/거절).
    MatchUpdate { scrim: ScrimMatch },
    /// 오류.
    Error { message: String },
}
