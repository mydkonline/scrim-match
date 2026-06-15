use dioxus::prelude::*;
use shared::{Player, Squad, Team};

use crate::state::AppCtx;
use crate::views::{avatar_url, initials, TeamLogo};

/// 두 선수의 군(squad)을 교체.
fn swap_squads(ctx: AppCtx, a_id: &str, b_id: &str) {
    if a_id == b_id {
        return;
    }
    let Some(mut team) = ctx.my_team.read().clone() else { return };
    let sa = team.roster.iter().find(|p| p.id == a_id).map(|p| p.squad);
    let sb = team.roster.iter().find(|p| p.id == b_id).map(|p| p.squad);
    if let (Some(sa), Some(sb)) = (sa, sb) {
        for p in team.roster.iter_mut() {
            if p.id == a_id {
                p.squad = sb;
            } else if p.id == b_id {
                p.squad = sa;
            }
        }
        let mut mt = ctx.my_team;
        mt.set(Some(team));
    }
}

/// 드래그한 선수를 특정 군으로 이동(칼럼에 드롭).
fn move_to_squad(ctx: AppCtx, id: &str, squad: Squad) {
    let Some(mut team) = ctx.my_team.read().clone() else { return };
    let mut changed = false;
    for p in team.roster.iter_mut() {
        if p.id == id && p.squad != squad {
            p.squad = squad;
            changed = true;
        }
    }
    if changed {
        let mut mt = ctx.my_team;
        mt.set(Some(team));
    }
}

#[component]
pub fn TeamSetting() -> Element {
    let ctx = use_context::<AppCtx>();
    let team = ctx.my_team.read().clone();
    let dragging = use_signal(|| Option::<String>::None);

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
                div { class: "row-gap", style: "justify-content:space-between;margin-bottom:16px;",
                    h3 { class: "h-md", "Roster" }
                    span { class: "caption", "↔ 선수를 드래그해 군(群)을 교체하세요" }
                }
                div { class: "squad-grid",
                    SquadCol { team: team.clone(), squad: Squad::First, dragging }
                    SquadCol { team: team.clone(), squad: Squad::Second, dragging }
                    SquadCol { team: team.clone(), squad: Squad::Academy, dragging }
                }
            }
        }
    }
}

#[component]
fn SquadCol(team: Team, squad: Squad, dragging: Signal<Option<String>>) -> Element {
    let ctx = use_context::<AppCtx>();
    let players: Vec<Player> = team.squad(squad).into_iter().cloned().collect();
    let mut dragging = dragging;

    rsx! {
        div { class: "squad-col",
            // 칼럼 자체가 드롭 대상(빈 곳에 놓으면 이 군으로 이동)
            ondragover: move |e| e.prevent_default(),
            ondrop: move |e| {
                e.prevent_default();
                if let Some(id) = dragging.read().clone() {
                    move_to_squad(ctx, &id, squad);
                }
                dragging.set(None);
            },
            h4 { "{squad.label()}" }
            for p in players.iter() {
                {
                    let pid = p.id.clone();
                    let drop_id = p.id.clone();
                    let is_drag = dragging.read().as_deref() == Some(p.id.as_str());
                    let cls = if is_drag { "player draggable dragging" } else { "player draggable" };
                    rsx! {
                        div { key: "{p.id}", class: "{cls}",
                            draggable: "true",
                            ondragstart: move |_| dragging.set(Some(pid.clone())),
                            ondragend: move |_| dragging.set(None),
                            ondragover: move |e| e.prevent_default(),
                            ondrop: move |e| {
                                e.prevent_default();
                                e.stop_propagation();
                                if let Some(from) = dragging.read().clone() {
                                    swap_squads(ctx, &from, &drop_id);
                                }
                                dragging.set(None);
                            },
                            img { class: "avatar-img", src: "{avatar_url(&p.name)}", alt: "{initials(&p.name)}" }
                            div { class: "player-meta",
                                div { class: "pname", "{p.name}" }
                                div { class: "prole", "{p.role}" }
                            }
                        }
                    }
                }
            }
            if players.is_empty() {
                div { class: "squad-empty", "여기로 드래그" }
            }
        }
    }
}
