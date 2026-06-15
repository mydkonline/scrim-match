mod calendar;
mod login;
mod matching;
mod messages;
mod nav;
mod team;

pub use calendar::Calendar;
pub use login::Login;
pub use matching::{Matching, TeamLogo};
pub use messages::Messages;
pub use nav::NavBar;
pub use team::TeamSetting;

/// 이름에서 아바타용 이니셜(최대 2글자)을 만든다.
pub fn initials(name: &str) -> String {
    name.chars().take(2).collect::<String>().to_uppercase()
}

fn hash(s: &str) -> u32 {
    let mut h: u32 = 5381;
    for b in s.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u32);
    }
    h
}

/// 로컬 LoL 프로필 아이콘(av0..av39)으로 결정적 아바타.
const AVATAR_COUNT: u32 = 40;
pub fn avatar_url(name: &str) -> String {
    format!("profile-icons/av{}.png", hash(name) % AVATAR_COUNT)
}

/// 지역(리그)→국기 이모지.
pub fn flag_for(region: &str) -> &'static str {
    match region {
        "LCK" | "ASL" => "🇰🇷",
        "LPL" => "🇨🇳",
        "LEC" => "🇪🇺",
        "LCS" => "🇺🇸",
        "CBLOL" => "🇧🇷",
        "LJL" => "🇯🇵",
        "PCS" => "🇹🇼",
        "VCS" => "🇻🇳",
        "LCP" => "🌏",
        "VCT Pacific" => "🌏",
        _ => "🏳️",
    }
}
