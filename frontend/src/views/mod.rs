mod calendar;
mod login;
mod matching;
mod nav;
mod team;

pub use calendar::Calendar;
pub use login::Login;
pub use matching::Matching;
pub use nav::NavBar;
pub use team::TeamSetting;

/// 이름에서 아바타용 이니셜(최대 2글자)을 만든다.
pub fn initials(name: &str) -> String {
    name.chars().take(2).collect::<String>().to_uppercase()
}
