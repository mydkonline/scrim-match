//! Postgres 영속화 계층 (sqlx, 런타임 쿼리).
//!
//! `DATABASE_URL` 이 있으면 팀·로스터를 DB에서 로드하고 확정 매칭을 저장한다.
//! 없으면 main 에서 인메모리 시드로 폴백한다(컴파일 타임 DB 불필요).

use shared::{CalendarEntry, Game, MatchStatus, Player, ScrimMatch, Squad, Staff, Team};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};

fn game_to_str(g: Game) -> &'static str {
    match g {
        Game::Lol => "Lol",
        Game::Valorant => "Valorant",
        Game::Starcraft => "Starcraft",
    }
}
fn str_to_game(s: &str) -> Game {
    match s {
        "Valorant" => Game::Valorant,
        "Starcraft" => Game::Starcraft,
        _ => Game::Lol,
    }
}
fn squad_to_str(s: Squad) -> &'static str {
    match s {
        Squad::First => "First",
        Squad::Second => "Second",
        Squad::Academy => "Academy",
    }
}
fn str_to_squad(s: &str) -> Squad {
    match s {
        "Second" => Squad::Second,
        "Academy" => Squad::Academy,
        _ => Squad::First,
    }
}
fn status_to_str(s: MatchStatus) -> &'static str {
    match s {
        MatchStatus::Pending => "Pending",
        MatchStatus::Applied => "Applied",
        MatchStatus::Confirmed => "Confirmed",
        MatchStatus::Denied => "Denied",
    }
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS teams (
  id text PRIMARY KEY, name text NOT NULL, tag text NOT NULL,
  game text NOT NULL, region text NOT NULL, manager text NOT NULL, logo text
);
ALTER TABLE teams ADD COLUMN IF NOT EXISTS logo text;
CREATE TABLE IF NOT EXISTS coaches (
  team_id text NOT NULL, idx int NOT NULL, name text NOT NULL,
  PRIMARY KEY (team_id, idx)
);
CREATE TABLE IF NOT EXISTS players (
  id text PRIMARY KEY, team_id text NOT NULL, name text NOT NULL,
  role text NOT NULL, squad text NOT NULL
);
CREATE TABLE IF NOT EXISTS matches (
  id text PRIMARY KEY, team_a text NOT NULL, team_b text NOT NULL,
  game text NOT NULL, date text NOT NULL, time text NOT NULL,
  code text NOT NULL, status text NOT NULL, created_at timestamptz DEFAULT now()
);
"#;

pub async fn init(url: &str) -> Result<PgPool, sqlx::Error> {
    let pool = PgPoolOptions::new().max_connections(5).connect(url).await?;
    sqlx::raw_sql(SCHEMA).execute(&pool).await?;
    sync_seed(&pool).await?;
    Ok(pool)
}

/// 코드의 시드를 DB에 동기화(upsert). 시드 데이터가 바뀌면 부팅 시 반영된다.
/// (확정 매칭 `matches` 는 건드리지 않음.)
async fn sync_seed(pool: &PgPool) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    for t in shared::seed::seed_teams() {
        sqlx::query(
            "INSERT INTO teams(id,name,tag,game,region,manager,logo) VALUES($1,$2,$3,$4,$5,$6,$7) \
             ON CONFLICT(id) DO UPDATE SET name=EXCLUDED.name, tag=EXCLUDED.tag, game=EXCLUDED.game, \
             region=EXCLUDED.region, manager=EXCLUDED.manager, logo=EXCLUDED.logo",
        )
        .bind(&t.id).bind(&t.name).bind(&t.tag)
        .bind(game_to_str(t.game)).bind(&t.region).bind(&t.staff.manager).bind(&t.logo)
        .execute(&mut *tx).await?;
        for (i, c) in t.staff.coaches.iter().enumerate() {
            sqlx::query(
                "INSERT INTO coaches(team_id,idx,name) VALUES($1,$2,$3) \
                 ON CONFLICT(team_id,idx) DO UPDATE SET name=EXCLUDED.name",
            )
            .bind(&t.id).bind(i as i32).bind(c)
            .execute(&mut *tx).await?;
        }
        for p in &t.roster {
            sqlx::query(
                "INSERT INTO players(id,team_id,name,role,squad) VALUES($1,$2,$3,$4,$5) \
                 ON CONFLICT(id) DO UPDATE SET team_id=EXCLUDED.team_id, name=EXCLUDED.name, \
                 role=EXCLUDED.role, squad=EXCLUDED.squad",
            )
            .bind(&p.id).bind(&t.id).bind(&p.name).bind(&p.role).bind(squad_to_str(p.squad))
            .execute(&mut *tx).await?;
        }
    }
    tx.commit().await?;
    tracing::info!("synced seed teams into Postgres");
    Ok(())
}

pub async fn load_teams(pool: &PgPool) -> Vec<Team> {
    let trows = sqlx::query("SELECT id,name,tag,game,region,manager,logo FROM teams ORDER BY id")
        .fetch_all(pool).await.unwrap_or_default();
    let crows = sqlx::query("SELECT team_id,name FROM coaches ORDER BY team_id,idx")
        .fetch_all(pool).await.unwrap_or_default();
    let prows = sqlx::query("SELECT id,team_id,name,role,squad FROM players")
        .fetch_all(pool).await.unwrap_or_default();

    trows
        .iter()
        .map(|tr| {
            let id: String = tr.get("id");
            let coaches = crows
                .iter()
                .filter(|c| c.get::<String, _>("team_id") == id)
                .map(|c| c.get::<String, _>("name"))
                .collect();
            let roster = prows
                .iter()
                .filter(|p| p.get::<String, _>("team_id") == id)
                .map(|p| Player {
                    id: p.get("id"),
                    name: p.get("name"),
                    role: p.get("role"),
                    squad: str_to_squad(&p.get::<String, _>("squad")),
                })
                .collect();
            Team {
                id: id.clone(),
                name: tr.get("name"),
                tag: tr.get("tag"),
                game: str_to_game(&tr.get::<String, _>("game")),
                region: tr.get("region"),
                logo: tr.get("logo"),
                staff: Staff { manager: tr.get("manager"), coaches },
                roster,
            }
        })
        .collect()
}

pub async fn persist_match(pool: &PgPool, m: &ScrimMatch) {
    let res = sqlx::query(
        "INSERT INTO matches(id,team_a,team_b,game,date,time,code,status) \
         VALUES($1,$2,$3,$4,$5,$6,$7,$8) \
         ON CONFLICT(id) DO UPDATE SET status=EXCLUDED.status",
    )
    .bind(&m.id).bind(&m.team_a).bind(&m.team_b).bind(game_to_str(m.game))
    .bind(&m.date).bind(&m.time).bind(&m.code).bind(status_to_str(m.status))
    .execute(pool).await;
    if let Err(e) = res {
        tracing::error!("persist_match failed: {e}");
    }
}

pub async fn load_calendar(pool: &PgPool, teams: &[Team]) -> Vec<CalendarEntry> {
    let rows = sqlx::query(
        "SELECT team_b,game,date FROM matches WHERE status='Confirmed' ORDER BY date",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    rows.iter()
        .map(|r| {
            let tb: String = r.get("team_b");
            let opponent = teams.iter().find(|t| t.id == tb).map(|t| t.name.clone()).unwrap_or(tb);
            CalendarEntry {
                date: r.get("date"),
                opponent,
                game: str_to_game(&r.get::<String, _>("game")),
                result: None,
            }
        })
        .collect()
}
