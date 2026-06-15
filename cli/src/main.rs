//! Scrim.GG 터미널 클라이언트 — 브라우저 없이 스크림 신청을 받고 수락.
//!
//! 사용:
//!   cargo run -p cli -- --team t1-lol [--game lol] [--serial CODE] [--url wss://scrim-gg.fly.dev/ws]
//!
//! 접속하면 나에게 온 대기 신청(메시지 큐)을 받아 출력합니다.
//! 명령: help · list · accept <code> · apply <teamId> · chat <id> <메시지> · search <date> <time> · quit

use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use shared::{ClientMsg, Game, ServerMsg};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

fn parse_game(s: &str) -> Game {
    match s.to_lowercase().as_str() {
        "val" | "valorant" => Game::Valorant,
        "sc" | "starcraft" => Game::Starcraft,
        _ => Game::Lol,
    }
}

#[tokio::main]
async fn main() {
    let team = arg("--team").unwrap_or_else(|| {
        eprintln!("--team <팀ID> 가 필요합니다 (예: t1-lol, geng-lol, bnk-lol)");
        std::process::exit(1);
    });
    let game = parse_game(&arg("--game").unwrap_or_else(|| "lol".into()));
    let serial = arg("--serial").unwrap_or_else(|| shared::serial_for(&team));
    let url = arg("--url").unwrap_or_else(|| "wss://scrim-gg.fly.dev/ws".into());

    println!("🔌 연결 중: {url}");
    let (ws, _) = match tokio_tungstenite::connect_async(&url).await {
        Ok(x) => x,
        Err(e) => {
            eprintln!("❌ 연결 실패: {e}");
            std::process::exit(1);
        }
    };
    let (mut write, mut read) = ws.split();

    // 송신 채널
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<ClientMsg>();
    let send = |msg: ClientMsg| { let _ = tx.send(msg); };

    // Hello 인증
    send(ClientMsg::Hello { serial: serial.clone(), team_id: team.clone(), game });

    // 수신한 신청 코드 보관(list 용)
    let inbox: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));

    // 송신 펌프
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Ok(txt) = serde_json::to_string(&msg) {
                if write.send(Message::Text(txt)).await.is_err() {
                    break;
                }
            }
        }
    });

    // 수신 펌프
    let inbox_r = inbox.clone();
    tokio::spawn(async move {
        while let Some(Ok(msg)) = read.next().await {
            if let Message::Text(txt) = msg {
                if let Ok(sm) = serde_json::from_str::<ServerMsg>(&txt) {
                    match sm {
                        ServerMsg::Welcome { team } => {
                            println!("✅ 로그인: {} ({})", team.name, team.region);
                            println!("명령을 입력하세요. (help 로 도움말)");
                        }
                        ServerMsg::InviteIncoming { match_id, from } => {
                            inbox_r.lock().await.push((match_id.clone(), from.name.clone()));
                            println!("\n📩 새 스크림 신청: {} ({})  — 수락하려면: accept {}",
                                from.name, from.region, match_id);
                            print!("> ");
                            use std::io::Write;
                            let _ = std::io::stdout().flush();
                        }
                        ServerMsg::Applied { code, to } => {
                            println!("\n📨 신청 전송됨 → {} · 코드 {} (상대가 수락 대기)", to.name, code);
                        }
                        ServerMsg::MatchConfirmed { scrim, opponent, .. } => {
                            println!("\n🎉 매칭 확정! vs {} · {} {} · 코드 {}",
                                opponent.name, scrim.date, scrim.time, scrim.code);
                        }
                        ServerMsg::Chat { from_name, text, .. } => {
                            println!("\n💬 {from_name}: {text}");
                        }
                        ServerMsg::ScrimList { .. } | ServerMsg::InviteSent { .. }
                        | ServerMsg::InviteRejected { .. } => {}
                        ServerMsg::Error { message } => println!("\n⚠️  {message}"),
                    }
                }
            }
        }
        println!("\n🔌 연결이 종료되었습니다.");
        std::process::exit(0);
    });

    // 표준입력 명령 루프
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let today = "2026-06-20".to_string();
    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.splitn(3, ' ');
        let cmd = parts.next().unwrap_or("");
        match cmd {
            "help" => {
                println!("명령:");
                println!("  list                  받은 신청 목록");
                println!("  accept <code>         코드로 신청 수락 → 매칭 확정");
                println!("  apply <teamId>        상대 팀에 스크림 신청(코드 발급)");
                println!("  chat <id> <메시지>    확정 매칭에 채팅");
                println!("  search <date> <time>  스크림 검색 풀 진입");
                println!("  quit                  종료");
            }
            "list" => {
                let ib = inbox.lock().await;
                if ib.is_empty() {
                    println!("(받은 신청 없음)");
                } else {
                    for (code, name) in ib.iter() {
                        println!("  {code}  ←  {name}");
                    }
                }
            }
            "accept" => {
                if let Some(code) = parts.next() {
                    send(ClientMsg::AcceptCode { code: code.trim().to_string() });
                    println!("→ 수락 요청: {code}");
                } else {
                    println!("사용법: accept <code>");
                }
            }
            "apply" => {
                if let Some(t) = parts.next() {
                    send(ClientMsg::ApplyQueue {
                        target_team: t.trim().to_string(),
                        date: today.clone(),
                        time: "19:00".into(),
                        squad: "1군".into(),
                    });
                    println!("→ 신청 전송: {t}");
                } else {
                    println!("사용법: apply <teamId>");
                }
            }
            "chat" => {
                if let Some(id) = parts.next() {
                    let text = parts.next().unwrap_or("").to_string();
                    send(ClientMsg::Chat { match_id: id.trim().to_string(), text });
                } else {
                    println!("사용법: chat <id> <메시지>");
                }
            }
            "search" => {
                let date = parts.next().unwrap_or("2026-06-20").to_string();
                let time = parts.next().unwrap_or("19:00").to_string();
                send(ClientMsg::Search { date, time, region: None });
                println!("→ 검색 풀 진입");
            }
            "quit" | "exit" => {
                println!("👋 종료");
                std::process::exit(0);
            }
            _ => println!("알 수 없는 명령: {cmd} (help)"),
        }
    }
}
