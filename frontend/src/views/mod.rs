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

/// 실제 프로 선수 사진이 있는 파일 목록(이름키.확장자).
const PLAYER_PHOTOS: &[&str] = &[
    "aiming.jpg","aria.jpg","bdd.jpg","beryl.jpg","bin.jpg","blaber.jpg","brokenblade.jpg","busio.jpg",
    "bwipo.jpg","canyon.jpeg","caps.jpg","cariok.jpeg","chovy.jpg","crisp.jpg","cuzz.jpg","delight.jpg",
    "deokdam.jpg","doggo.jpg","doran.jpg","elk.jpg","faker.jpg","gumayusi.jpeg","hanssama.jpg","harp.jpg",
    "hongq.jpeg","humanoid.jpg","inspired.jpg","jun.jpg","kaiwing.jpeg","kiin.jpg","knight.jpg","kuri.jpeg",
    "kyeahoo.jpg","lehends.jpg","lucid.jpeg","massu.jpg","mikyx.jpg","noah.jpg","on.jpg","oner.jpg",
    "oscarinin.jpg","paduck.jpg","peanut.jpg","perfect.jpg","peyz.jpeg","pleata.jpg","quad.jpg","razork.jpg",
    "redbert.jpg","rest.jpg","rich.jpg","roamer.jpg","robo.jpeg","route.jpg","showmaker.jpeg","siwoo.jpeg",
    "steal.jpg","tarzan.jpg","tatu.jpeg","thanatos.jpg","theshy.jpg","tinowns.jpeg","titan.jpg","viper.jpg",
    "vulcan.jpg","wei.jpg","wizer.jpg","xiaohu.jpg","yike.jpg","zeka.jpg","zeus.jpg","zven.jpg",
];

fn sanitize_key(name: &str) -> String {
    name.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase()
}

/// 로컬 LoL 프로필 아이콘(av0..av39) 폴백.
const AVATAR_COUNT: u32 = 40;

/// 실제 프로 선수 사진이 있으면 그 사진을, 없으면 프로필 아이콘을 반환.
pub fn avatar_url(name: &str) -> String {
    let key = sanitize_key(name);
    for f in PLAYER_PHOTOS {
        if f.split('.').next() == Some(key.as_str()) {
            return format!("players/{f}");
        }
    }
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
