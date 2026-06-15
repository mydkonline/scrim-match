use dioxus::prelude::*;
use shared::{ClientMsg, Game, MatchStatus, ScrimMatch, Squad, Team};

use crate::state::AppCtx;
use crate::views::initials;

/// 오프라인 데모용 6자리 비밀 코드(djb2 해시).
fn mock_code(seed: &str) -> String {
    let mut h: u32 = 5381;
    for b in seed.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u32);
    }
    format!("{:06}", h % 1_000_000)
}

fn mock_match(me: &Team, op: &Team, game: Game, date: &str, time: &str) -> ScrimMatch {
    ScrimMatch {
        id: format!("demo-{}-{}", me.id, op.id),
        team_a: me.id.clone(),
        team_b: op.id.clone(),
        game,
        date: date.to_string(),
        time: time.to_string(),
        code: mock_code(&format!("{}{}{}{}", me.id, op.id, date, time)),
        status: MatchStatus::Pending,
    }
}

#[component]
pub fn Matching() -> Element {
    let ctx = use_context::<AppCtx>();
    let my_team = ctx.my_team.read().clone();
    let teams = ctx.teams;
    let game = *ctx.game.read();
    let online = *ctx.online.read();
    let cur = ctx.current_match.read().clone();

    let mut date = use_signal(|| "2026-06-15".to_string());
    let mut time = use_signal(|| "18:00".to_string());

    let my_id = my_team.as_ref().map(|m| m.id.clone());
    let opp_teams: Vec<Team> = teams
        .read()
        .iter()
        .filter(|t| t.game == game && Some(t.id.clone()) != my_id)
        .cloned()
        .collect();

    let opp_id = ctx.opponent_id.read().clone();
    let opponent: Option<Team> =
        opp_id.as_ref().and_then(|id| teams.read().iter().find(|t| &t.id == id).cloned());

    let can_find = my_team.is_some() && opponent.is_some();

    rsx! {
        h1 { class: "h-lg", "Matching" }
        p { class: "muted", "{game.label()} · 같은 슬롯을 찾는 팀과 자동 페어링됩니다." }

        div { class: "match-grid mt-xl",
            // ───── 왼쪽: 우리 팀 ─────
            TeamColumn { team: my_team.clone(), side: "left" }

            // ───── 가운데: 매칭 카드 ─────
            div { class: "match-center",
                div { class: "meeting-pill", "On Game Meeting" }

                // 상대 선택 + 슬롯
                div { class: "card", style: "padding:16px;",
                    div { class: "field", style: "margin-bottom:12px;",
                        label { "OPPONENT" }
                        select {
                            class: "select",
                            onchange: move |e| {
                                let mut o = ctx.opponent_id;
                                let v = e.value();
                                o.set(if v.is_empty() { None } else { Some(v) });
                            },
                            option { value: "", "상대 팀 선택…" }
                            for t in opp_teams.iter() {
                                option { key: "{t.id}", value: "{t.id}", "{t.name} · {t.region}" }
                            }
                        }
                    }
                    div { class: "slot-row",
                        div { class: "field", style: "margin:0;",
                            label { "DATE" }
                            input { class: "input", r#type: "date", value: "{date}",
                                oninput: move |e| date.set(e.value()) }
                        }
                        div { class: "field", style: "margin:0;",
                            label { "TIME" }
                            input { class: "input", r#type: "time", value: "{time}",
                                oninput: move |e| time.set(e.value()) }
                        }
                    }
                    button {
                        class: "btn btn-primary btn-block",
                        style: "margin-top:12px;",
                        disabled: !can_find,
                        onclick: {
                            let me = my_team.clone();
                            let op = opponent.clone();
                            move |_| {
                                if online {
                                    ctx.send(ClientMsg::FindScrim { date: date.read().clone(), time: time.read().clone() });
                                } else if let (Some(me), Some(op)) = (me.clone(), op.clone()) {
                                    let scrim = mock_match(&me, &op, game, &date.read(), &time.read());
                                    let mut cm = ctx.current_match;
                                    cm.set(Some(scrim));
                                    let mut st = ctx.status;
                                    st.set("데모 매칭 생성됨 — Apply 로 확정".into());
                                }
                            }
                        },
                        "🔍 Find Scrim"
                    }
                }

                // 비밀 코드 카드 (매칭 존재 시)
                {
                    if let Some(m) = cur.clone() {
                        rsx! { ScrimCard { scrim: m } }
                    } else {
                        rsx! { p { class: "caption center", "아직 매칭이 없습니다. 슬롯을 잡고 Find Scrim 을 누르세요." } }
                    }
                }
            }

            // ───── 오른쪽: 상대 팀 ─────
            TeamColumn { team: opponent.clone(), side: "right" }
        }
    }
}

#[component]
fn ScrimCard(scrim: ScrimMatch) -> Element {
    let ctx = use_context::<AppCtx>();
    let online = *ctx.online.read();
    let confirmed = scrim.status == MatchStatus::Confirmed;
    let denied = scrim.status == MatchStatus::Denied;

    rsx! {
        div { class: "scrim-code",
            div { class: "row", span { class: "k", "Game Scrum" } span { class: "v", "{scrim.date}" } }
            div { class: "row", span { class: "k", "CODE" } span { class: "big", "{scrim.code}" } }
            div { class: "row", span { class: "k", "TIME" } span { class: "v", "{scrim.time}" } }
            div { class: "row", span { class: "k", "STATUS" } span { class: "v", "{scrim.status:?}" } }
        }

        {
            if confirmed {
                rsx! {
                    div { class: "confirm-box",
                        div { class: "big", "Yes" }
                        p { class: "muted", "스크림 확정! 두 팀에게 비공개 코드가 공유되었습니다." }
                    }
                }
            } else if denied {
                rsx! { p { class: "caption center", "거절되었습니다." } }
            } else {
                let id_apply = scrim.id.clone();
                let id_deny = scrim.id.clone();
                let scrim_for_apply = scrim.clone();
                rsx! {
                    div { class: "match-actions",
                        button {
                            class: "btn btn-primary",
                            onclick: move |_| {
                                if online {
                                    ctx.send(ClientMsg::Apply { match_id: id_apply.clone() });
                                } else {
                                    let mut m = scrim_for_apply.clone();
                                    m.status = MatchStatus::Confirmed;
                                    let mut cm = ctx.current_match;
                                    cm.set(Some(m));
                                }
                            },
                            "Apply"
                        }
                        button {
                            class: "btn btn-danger",
                            onclick: move |_| {
                                if online {
                                    ctx.send(ClientMsg::Deny { match_id: id_deny.clone() });
                                }
                                let mut cm = ctx.current_match;
                                cm.set(None);
                            },
                            "Denied"
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn TeamColumn(team: Option<Team>, side: String) -> Element {
    let cls = format!("team-col {side}");
    match team {
        Some(t) => {
            let firsts: Vec<_> = t.squad(Squad::First).into_iter().cloned().collect();
            rsx! {
                div { class: "{cls}",
                    h3 { "{t.name}" }
                    div { class: "tag", "{t.tag} · {t.region}" }
                    div { class: "roster-circles",
                        for p in firsts.iter() {
                            div { key: "{p.id}", class: "player",
                                div { class: "avatar", "{initials(&p.name)}" }
                                div { class: "player-meta",
                                    div { class: "pname", "{p.name}" }
                                    div { class: "prole", "{p.role}" }
                                }
                            }
                        }
                    }
                }
            }
        }
        None => rsx! {
            div { class: "{cls}",
                h3 { class: "muted", "—" }
                div { class: "tag", "팀 미선택" }
            }
        },
    }
}
