use dioxus::prelude::*;
use shared::{Squad, Team};

use crate::state::AppCtx;
use crate::views::{avatar_url, initials, TeamLogo};

#[component]
pub fn TeamSetting() -> Element {
    let ctx = use_context::<AppCtx>();
    let team = ctx.my_team.read().clone();

    let Some(team) = team else {
        return rsx! { p { class: "muted", "팀을 먼저 선택하세요." } };
    };

    rsx! {
        div { class: "team-header",
            TeamLogo { logo: team.logo.clone(), tag: team.tag.clone(), size: 64 }
            div {
                h1 { class: "h-lg", "{team.name}" }
                p { class: "muted", "{team.region} · {team.game.label()}" }
            }
        }

        div { class: "team-layout mt-xl",
            div { class: "staff-card card",
                h3 { class: "h-md", style: "margin-bottom:16px;", "Staff" }
                div { class: "staff-row",
                    div { class: "role", "Manager" }
                    "{team.staff.manager}"
                }
                for (i, c) in team.staff.coaches.iter().enumerate() {
                    div { key: "{i}", class: "staff-row",
                        div { class: "role", "Coach {i + 1}" }
                        "{c}"
                    }
                }
            }

            div { class: "card",
                h3 { class: "h-md", style: "margin-bottom:16px;", "Roster" }
                div { class: "squad-grid",
                    SquadCol { team: team.clone(), squad: Squad::First }
                    SquadCol { team: team.clone(), squad: Squad::Second }
                    SquadCol { team: team.clone(), squad: Squad::Academy }
                }
            }
        }
    }
}

#[component]
fn SquadCol(team: Team, squad: Squad) -> Element {
    let players: Vec<_> = team.squad(squad).into_iter().cloned().collect();
    rsx! {
        div { class: "squad-col",
            h4 { "{squad.label()}" }
            for p in players.iter() {
                div { key: "{p.id}", class: "player",
                    img { class: "avatar-img", src: "{avatar_url(&p.name)}", alt: "{initials(&p.name)}" }
                    div { class: "player-meta",
                        div { class: "pname", "{p.name}" }
                        div { class: "prole", "{p.role}" }
                    }
                }
            }
        }
    }
}
