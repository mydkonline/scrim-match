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

/// 비-영숫자 문자를 퍼센트 인코딩(아바타 seed용 최소 인코더).
fn enc(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// 이름 기반 결정적 프로필 아바타 URL (DiceBear, 저작권 무관).
pub fn avatar_url(name: &str) -> String {
    format!("https://api.dicebear.com/9.x/thumbs/svg?seed={}&backgroundColor=f0f0f0", enc(name))
}
