use dioxus::prelude::*;
use shared::{ClientMsg, Listing, ScrimMatch};

use crate::state::{AppCtx, ChatMsg};
use crate::views::initials;

#[component]
pub fn Matching() -> Element {
    let ctx = use_context::<AppCtx>();
    let online = *ctx.online.read();
    let searching = *ctx.searching.read();
    let listings = ctx.listings.read().clone();
    let outgoing = ctx.outgoing.read().clone();
    let incoming = ctx.incoming.read().clone();
    let confirmed = ctx.confirmed.read().clone();

    rsx! {
        h1 { class: "h-lg", "스크림 매칭" }
        p { class: "muted", "슬롯을 정하고 전 세계 팀을 검색해 스크림을 신청하세요." }

        if !online {
            div { class: "card mt-xl", style: "border-color:var(--hairline-strong);",
                p { class: "muted", "⚠️ 실서버에 연결되지 않았습니다(오프라인). 매칭은 온라인에서만 동작합니다." }
            }
        }

        // 확정되면 매칭 + 채팅 화면만 표시
        if let Some((mid, scrim, opp)) = confirmed {
            ConfirmedPanel { match_id: mid, scrim, opponent: opp }
        } else {
            div { class: "mt-xl",
                // 들어온 신청 배너(있으면 최상단)
                if let Some((mid, from)) = incoming {
                    IncomingInvite { match_id: mid, from }
                }

                if let Some((_, to)) = outgoing {
                    WaitingCard { opponent: to }
                } else if searching {
                    SearchingView { listings }
                } else {
                    SearchForm {}
                }
            }
        }
    }
}

#[component]
fn SearchForm() -> Element {
    let ctx = use_context::<AppCtx>();
    let my_team = ctx.my_team.read().clone();
    let mut date = use_signal(|| "2026-06-20".to_string());
    let mut time = use_signal(|| "19:00".to_string());
    let mut same_region = use_signal(|| false);

    rsx! {
        div { class: "card", style: "max-width:460px;margin:0 auto;",
            h3 { class: "h-md", style: "margin-bottom:16px;", "스크림 슬롯" }
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
            label { class: "row-gap", style: "margin-top:12px;font-size:13px;color:var(--ink-secondary);",
                input { r#type: "checkbox", checked: "{same_region}",
                    onchange: move |e| same_region.set(e.checked()) }
                "같은 지역(리그) 팀만 검색"
            }
            button {
                class: "btn btn-primary btn-block",
                style: "margin-top:16px;",
                onclick: move |_| {
                    let region = if *same_region.read() {
                        my_team.as_ref().map(|t| t.region.clone())
                    } else { None };
                    let mut s = ctx.searching;
                    s.set(true);
                    let mut l = ctx.listings;
                    l.set(Vec::new());
                    ctx.send(ClientMsg::Search { date: date.read().clone(), time: time.read().clone(), region });
                },
                "🔍 스크림 상대 찾기"
            }
        }
    }
}

#[component]
fn SearchingView(listings: Vec<Listing>) -> Element {
    let ctx = use_context::<AppCtx>();
    rsx! {
        div { class: "search-wrap",
            div { class: "globe", "🌐" }
            p { class: "h-md center", "전 세계 상대 검색 중…" }
            p { class: "caption center", "스크림 가능한 팀이 실시간으로 표시됩니다" }

            button {
                class: "btn btn-outline",
                style: "display:block;margin:12px auto;",
                onclick: move |_| {
                    let mut s = ctx.searching;
                    s.set(false);
                    ctx.send(ClientMsg::StopSearch);
                },
                "검색 취소"
            }

            if listings.is_empty() {
                p { class: "caption center", style: "margin-top:8px;", "상대를 찾는 중…" }
            } else {
                div { class: "listing-grid",
                    for l in listings.iter() {
                        ListingCard { listing: l.clone() }
                    }
                }
            }
        }
    }
}

#[component]
fn ListingCard(listing: Listing) -> Element {
    let ctx = use_context::<AppCtx>();
    let tid = listing.team_id.clone();
    rsx! {
        div { class: "listing-card",
            TeamLogo { logo: listing.logo.clone(), tag: listing.tag.clone(), size: 40 }
            div { class: "listing-meta",
                div { class: "lname",
                    "{listing.name}"
                    if listing.demo { span { class: "demo-badge", "DEMO" } }
                }
                div { class: "lregion", "{listing.region}" }
            }
            button {
                class: "btn btn-primary",
                onclick: move |_| ctx.send(ClientMsg::Invite { target_team: tid.clone() }),
                "신청"
            }
        }
    }
}

#[component]
fn WaitingCard(opponent: Listing) -> Element {
    let ctx = use_context::<AppCtx>();
    rsx! {
        div { class: "card center", style: "max-width:420px;margin:0 auto;",
            TeamLogo { logo: opponent.logo.clone(), tag: opponent.tag.clone(), size: 64 }
            h3 { class: "h-md", style: "margin:12px 0 4px;", "{opponent.name}" }
            div { class: "spinner-dots", span {} span {} span {} }
            p { class: "muted", "상대의 수락을 기다리는 중…" }
            button {
                class: "btn btn-outline",
                onclick: move |_| { let mut o = ctx.outgoing; o.set(None); },
                "취소하고 목록으로"
            }
        }
    }
}

#[component]
fn IncomingInvite(match_id: String, from: Listing) -> Element {
    let ctx = use_context::<AppCtx>();
    let accept_id = match_id.clone();
    let reject_id = match_id.clone();
    rsx! {
        div { class: "incoming-card",
            TeamLogo { logo: from.logo.clone(), tag: from.tag.clone(), size: 48 }
            div { style: "flex:1;",
                div { class: "lname", "{from.name}" }
                div { class: "caption", "스크림을 신청했습니다 · {from.region}" }
            }
            div { class: "row-gap",
                button {
                    class: "btn btn-primary",
                    onclick: move |_| ctx.send(ClientMsg::Accept { match_id: accept_id.clone() }),
                    "수락"
                }
                button {
                    class: "btn btn-danger",
                    onclick: move |_| {
                        ctx.send(ClientMsg::Reject { match_id: reject_id.clone() });
                        let mut i = ctx.incoming;
                        i.set(None);
                    },
                    "거절"
                }
            }
        }
    }
}

#[component]
fn ConfirmedPanel(match_id: String, scrim: ScrimMatch, opponent: Listing) -> Element {
    let ctx = use_context::<AppCtx>();
    let my_team = ctx.my_team.read().clone();
    let chat = ctx.chat_log.read().clone();
    let mut draft = use_signal(String::new);
    let mid = match_id.clone();
    let my_name = my_team.as_ref().map(|t| t.name.clone()).unwrap_or_default();

    rsx! {
        div { class: "confirm-head",
            div { class: "confirm-team",
                TeamLogo { logo: my_team.as_ref().and_then(|t| t.logo.clone()), tag: my_team.as_ref().map(|t| t.tag.clone()).unwrap_or_default(), size: 56 }
                div { class: "lname", "{my_name}" }
            }
            div { class: "vs-badge", "VS" }
            div { class: "confirm-team",
                TeamLogo { logo: opponent.logo.clone(), tag: opponent.tag.clone(), size: 56 }
                div { class: "lname", "{opponent.name}" }
            }
        }

        div { class: "scrim-code", style: "max-width:420px;margin:16px auto;",
            div { class: "row", span { class: "k", "DATE" } span { class: "v", "{scrim.date}" } }
            div { class: "row", span { class: "k", "TIME" } span { class: "v", "{scrim.time}" } }
            div { class: "row", span { class: "k", "CODE" } span { class: "big", "{scrim.code}" } }
        }
        p { class: "center", style: "color:var(--primary-deep);font-weight:600;", "✅ 매칭 확정 — 두 팀만 아는 비밀 코드가 발급되었습니다" }

        // 채팅
        div { class: "chat", style: "max-width:480px;margin:16px auto 0;",
            div { class: "chat-head", "💬 {opponent.name} 와의 대화" }
            div { class: "chat-log",
                if chat.is_empty() {
                    p { class: "caption center", style: "padding:16px;", "인사를 건네보세요. 스크림 일정·맵·서버를 조율하세요." }
                }
                for (i, m) in chat.iter().enumerate() {
                    div { key: "{i}", class: if m.mine { "bubble mine" } else { "bubble" },
                        if !m.mine { div { class: "bubble-name", "{m.name}" } }
                        "{m.text}"
                    }
                }
            }
            div { class: "chat-input",
                input {
                    class: "input",
                    placeholder: "메시지 입력…",
                    value: "{draft}",
                    oninput: move |e| draft.set(e.value()),
                    onkeydown: {
                        let mid = mid.clone();
                        let my_name = my_name.clone();
                        move |e: KeyboardEvent| {
                            if e.key() == Key::Enter {
                                let text = draft.read().trim().to_string();
                                if !text.is_empty() {
                                    let mut log = ctx.chat_log.read().clone();
                                    log.push(ChatMsg { mine: true, name: my_name.clone(), text: text.clone() });
                                    let mut cl = ctx.chat_log; cl.set(log);
                                    ctx.send(ClientMsg::Chat { match_id: mid.clone(), text });
                                    draft.set(String::new());
                                }
                            }
                        }
                    },
                }
                button {
                    class: "btn btn-primary",
                    onclick: {
                        let mid = mid.clone();
                        let my_name = my_name.clone();
                        move |_| {
                            let text = draft.read().trim().to_string();
                            if !text.is_empty() {
                                let mut log = ctx.chat_log.read().clone();
                                log.push(ChatMsg { mine: true, name: my_name.clone(), text: text.clone() });
                                let mut cl = ctx.chat_log; cl.set(log);
                                ctx.send(ClientMsg::Chat { match_id: mid.clone(), text });
                                draft.set(String::new());
                            }
                        }
                    },
                    "전송"
                }
            }
        }

        button {
            class: "btn btn-outline",
            style: "display:block;margin:16px auto 0;",
            onclick: move |_| ctx.reset_matching(),
            "스크림 나가기 / 새 매칭"
        }
    }
}

/// 로고가 있으면 이미지, 없으면 태그 이니셜 원형.
#[component]
pub fn TeamLogo(logo: Option<String>, tag: String, size: u32) -> Element {
    let s = format!("width:{size}px;height:{size}px;");
    match logo {
        Some(src) => rsx! {
            img { class: "team-logo", style: "{s}", src: "{src}", alt: "{tag}" }
        },
        None => rsx! {
            div { class: "team-logo fallback", style: "{s}", "{initials(&tag)}" }
        },
    }
}
