//! 실제 프로팀 시드 데이터.
//!
//! 공개 정보를 기반으로 구성한 데모용 로스터입니다. 운영 시에는 DB로 대체합니다.

use crate::{Game, Player, Squad, Staff, Team};

fn p(id: &str, name: &str, role: &str, squad: Squad) -> Player {
    Player {
        id: id.to_string(),
        name: name.to_string(),
        role: role.to_string(),
        squad,
    }
}

fn staff(manager: &str, coaches: &[&str]) -> Staff {
    Staff {
        manager: manager.to_string(),
        coaches: coaches.iter().map(|c| c.to_string()).collect(),
    }
}

const LOL_ROLES: [&str; 5] = ["Top", "Jungle", "Mid", "ADC", "Support"];
const VAL_ROLES: [&str; 5] = ["Duelist", "Initiator", "Controller", "Sentinel", "Flex"];

/// 1군 5인 + 2군 2인 + 아카데미 3인 형태로 로스터를 채운다.
/// - `id_prefix`: player id 용(팀마다 유니크해야 함, 예: "drx-lol").
/// - `tag`: 표시 이름 접두(예: "DRX").
fn build_roster(id_prefix: &str, tag: &str, firsts: &[&str], roles: &[&str]) -> Vec<Player> {
    let mut roster = Vec::new();
    for (i, name) in firsts.iter().enumerate() {
        let role = roles.get(i).copied().unwrap_or("Flex");
        roster.push(p(&format!("{id_prefix}-1-{i}"), name, role, Squad::First));
    }
    // 데모용 2군 / 아카데미 자동 생성
    for i in 0..2 {
        roster.push(p(
            &format!("{id_prefix}-2-{i}"),
            &format!("{tag}_Sub{}", i + 1),
            roles.get(i).copied().unwrap_or("Flex"),
            Squad::Second,
        ));
    }
    for i in 0..3 {
        roster.push(p(
            &format!("{id_prefix}-a-{i}"),
            &format!("{tag}_Academy{}", i + 1),
            roles.get(i).copied().unwrap_or("Flex"),
            Squad::Academy,
        ));
    }
    roster
}

fn team(
    id: &str,
    name: &str,
    tag: &str,
    game: Game,
    region: &str,
    manager: &str,
    coaches: &[&str],
    firsts: &[&str],
    roles: &[&str],
) -> Team {
    Team {
        id: id.to_string(),
        name: name.to_string(),
        tag: tag.to_string(),
        game,
        region: region.to_string(),
        logo: None,
        staff: staff(manager, coaches),
        roster: build_roster(id, tag, firsts, roles),
    }
}

/// team id → 로고 에셋 경로(여러 종목이 같은 조직이면 로고 공유).
fn logo_for(id: &str) -> Option<String> {
    let key = match id {
        "t1-lol" | "t1-val" => "t1",
        "geng-lol" | "geng-val" => "geng",
        "drx-lol" | "drx-val" => "drx",
        "kt-lol" => "kt",
        "hle-lol" => "hle",
        "dk-lol" => "dk",
        "bnk-lol" => "bnk",
        "brion-lol" => "brion",
        "ns-lol" => "ns",
        "dns-lol" => "dns",
        _ => return None,
    };
    Some(format!("logos/{key}.webp"))
}

/// 서버 부팅 시 메모리에 적재되는 시드 팀 목록.
pub fn seed_teams() -> Vec<Team> {
    let mut teams = vec![
        // ───────── League of Legends · LCK ─────────
        team(
            "t1-lol", "T1", "T1", Game::Lol, "LCK",
            "Joe Marsh", &["kkOma", "Roach", "Tom"],
            &["Zeus", "Oner", "Faker", "Gumayusi", "Keria"], &LOL_ROLES,
        ),
        team(
            "geng-lol", "Gen.G", "GEN", Game::Lol, "LCK",
            "Arnold Hur", &["Score", "Wins"],
            &["Kiin", "Canyon", "Chovy", "Peyz", "Lehends"], &LOL_ROLES,
        ),
        team(
            "drx-lol", "DRX", "DRX", Game::Lol, "LCK",
            "DRX Mgmt", &["Ssong", "Pleata"],
            &["Rich", "Sponge", "Kyeahoo", "Paduck", "Pleata"], &LOL_ROLES,
        ),
        team(
            "kt-lol", "KT Rolster", "KT", Game::Lol, "LCK",
            "KT Mgmt", &["Lirang", "ssun"],
            &["PerfecT", "Cuzz", "Bdd", "deokdam", "Way"], &LOL_ROLES,
        ),
        team(
            "hle-lol", "Hanwha Life", "HLE", Game::Lol, "LCK",
            "HLE Mgmt", &["Daeny", "Serim"],
            &["Doran", "Peanut", "Zeka", "Viper", "Delight"], &LOL_ROLES,
        ),
        team(
            "dk-lol", "Dplus KIA", "DK", Game::Lol, "LCK",
            "DK Mgmt", &["Daeny", "Hirit"],
            &["Siwoo", "Lucid", "ShowMaker", "Aiming", "BeryL"], &LOL_ROLES,
        ),
        // 로스터 미정 팀(플레이스홀더 핸들 — 추후 실제 로스터로 교체).
        team(
            "bnk-lol", "BNK FEARX", "BFX", Game::Lol, "LCK",
            "BFX Mgmt", &["Coach"],
            &["bfxTop", "bfxJgl", "bfxMid", "bfxBot", "bfxSup"], &LOL_ROLES,
        ),
        team(
            "brion-lol", "Hanjin BRION", "BRO", Game::Lol, "LCK",
            "BRO Mgmt", &["Coach"],
            &["broTop", "broJgl", "broMid", "broBot", "broSup"], &LOL_ROLES,
        ),
        team(
            "ns-lol", "Nongshim RedForce", "NS", Game::Lol, "LCK",
            "NS Mgmt", &["Coach"],
            &["nsTop", "nsJgl", "nsMid", "nsBot", "nsSup"], &LOL_ROLES,
        ),
        team(
            "dns-lol", "DN SOOPers", "DNS", Game::Lol, "LCK",
            "DNS Mgmt", &["Coach"],
            &["dnsTop", "dnsJgl", "dnsMid", "dnsBot", "dnsSup"], &LOL_ROLES,
        ),
        // ───────── VALORANT · VCT Pacific ─────────
        team(
            "drx-val", "DRX", "DRX", Game::Valorant, "VCT Pacific",
            "DRX Mgmt", &["termi", "stax"],
            &["BuZz", "MaKo", "Rb", "Foxy9", "Zest"], &VAL_ROLES,
        ),
        team(
            "geng-val", "Gen.G", "GENV", Game::Valorant, "VCT Pacific",
            "Gen.G Mgmt", &["solo"],
            &["t3xture", "Karon", "Lakia", "Munchkin", "Meteor"], &VAL_ROLES,
        ),
        team(
            "prx-val", "Paper Rex", "PRX", Game::Valorant, "VCT Pacific",
            "PRX Mgmt", &["alecks"],
            &["something", "Jinggg", "f0rsakeN", "d4v41", "mindfreak"], &VAL_ROLES,
        ),
        team(
            "t1-val", "T1", "T1V", Game::Valorant, "VCT Pacific",
            "Joe Marsh", &["Autumn"],
            &["Sayaplayer", "Carpe", "iZu", "Meteor", "Sylvan"], &VAL_ROLES,
        ),
        // ───────── StarCraft (개인 종목, 1인 로스터) ─────────
        Team {
            id: "afr-sc".into(), name: "Afreeca Freecs".into(), tag: "AF".into(),
            game: Game::Starcraft, region: "ASL".into(), logo: None,
            staff: staff("AF Mgmt", &["Coach"]),
            roster: vec![p("af-1-0", "Light", "Terran", Squad::First)],
        },
        Team {
            id: "sk-sc".into(), name: "SKT Legends".into(), tag: "SKL".into(),
            game: Game::Starcraft, region: "ASL".into(), logo: None,
            staff: staff("SKL Mgmt", &["Coach"]),
            roster: vec![p("skl-1-0", "Rush", "Protoss", Squad::First)],
        },
    ];
    // 종목이 같은 조직이면 로고를 공유하도록 일괄 매핑.
    for t in teams.iter_mut() {
        t.logo = logo_for(&t.id);
    }
    teams
}
