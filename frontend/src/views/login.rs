use dioxus::prelude::*;
use shared::Game;

use crate::net;
use crate::state::{AppCtx, Screen};

#[component]
pub fn Login() -> Element {
    let ctx = use_context::<AppCtx>();
    let mut serial = ctx.serial;
    let game = *ctx.game.read();
    let teams = ctx.teams;
    let mut selected_team = use_signal(String::new);

    let game_teams: Vec<_> = teams
        .read()
        .iter()
        .filter(|t| t.game == game)
        .cloned()
        .collect();

    let can_connect = serial.read().trim().len() >= 4 && !selected_team.read().is_empty();

    rsx! {
        div { class: "login-wrap",
            div { class: "login-card card",
                div { class: "login-logo", span { class: "dot" } "OP.GG · Scrim.GG" }
                p { class: "muted", style: "margin-top:-16px;margin-bottom:24px;", "비밀 보장 스크림 매칭" }

                div { class: "field",
                    label { "SERIAL CODE" }
                    input {
                        class: "input",
                        r#type: "password",
                        placeholder: "팀 시리얼 코드 입력",
                        value: "{serial}",
                        oninput: move |e| serial.set(e.value()),
                    }
                }

                div { class: "field",
                    label { "SELECT GAME" }
                    div { class: "game-select",
                        GameChip { game: Game::Valorant, current: game }
                        GameChip { game: Game::Starcraft, current: game }
                        GameChip { game: Game::Lol, current: game }
                    }
                }

                div { class: "field",
                    label { "YOUR TEAM" }
                    select {
                        class: "select",
                        onchange: move |e| selected_team.set(e.value()),
                        option { value: "", "팀 선택…" }
                        for t in game_teams.iter() {
                            option { key: "{t.id}", value: "{t.id}", "{t.name} · {t.region}" }
                        }
                    }
                }

                button {
                    class: "btn btn-primary btn-block mt-xl",
                    disabled: !can_connect,
                    onclick: move |_| {
                        let tid = selected_team.read().clone();
                        if let Some(team) = teams.read().iter().find(|t| t.id == tid).cloned() {
                            let mut mt = ctx.my_team;
                            mt.set(Some(team));
                        }
                        net::connect(ctx, serial.read().clone(), tid, game);
                        ctx.goto(Screen::Matching);
                    },
                    "CONNECT →"
                }
                p { class: "caption center", style: "margin-top:16px;", "클릭 한 번으로 전 세계 팀과 연결" }
            }
        }
    }
}

#[component]
fn GameChip(game: Game, current: Game) -> Element {
    let ctx = use_context::<AppCtx>();
    let mut g = ctx.game;
    let active = game == current;
    let cls = if active { "game-chip active" } else { "game-chip" };
    let (icon, name) = match game {
        Game::Valorant => ("✦", "Valorant"),
        Game::Starcraft => ("✶", "StarCraft"),
        Game::Lol => ("✧", "LoL"),
    };
    rsx! {
        button {
            class: "{cls}",
            onclick: move |_| g.set(game),
            span { class: "g-icon", "{icon}" }
            "{name}"
        }
    }
}
