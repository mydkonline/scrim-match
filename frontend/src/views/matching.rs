use dioxus::prelude::*;
use shared::{ClientMsg, Listing};

use crate::state::{AppCtx, Screen};
use crate::views::initials;

#[component]
pub fn Matching() -> Element {
    let ctx = use_context::<AppCtx>();
    let online = *ctx.online.read();
    let searching = *ctx.searching.read();
    let listings = ctx.listings.read().clone();
    let outgoing = ctx.outgoing.read().clone();
    let inbox_n = ctx.inbox.read().len();

    rsx! {
        h1 { class: "h-lg", "스크림 매칭" }
        p { class: "muted", "슬롯을 정하고 전 세계 팀을 검색해 스크림을 신청하세요." }

        if !online {
            div { class: "card mt-xl", style: "border-color:var(--hairline-strong);",
                p { class: "muted", "⚠️ 실서버에 연결되지 않았습니다(오프라인). 매칭은 온라인에서만 동작합니다." }
            }
        }

        // 들어온 신청 알림 배너
        if inbox_n > 0 {
            div { class: "inbox-banner mt-xl",
                "📩 새 스크림 신청 {inbox_n}건이 도착했습니다."
                button {
                    class: "btn btn-primary",
                    onclick: move |_| ctx.goto(Screen::Messages),
                    "수신함 열기"
                }
            }
        }

        div { class: "mt-xl",
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
