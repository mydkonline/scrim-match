use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;
use shared::{Listing, MatchStatus, ScrimMatch, Squad, Team};

use crate::state::{AppCtx, InboxItem, Screen, Thread};
use crate::views::{flag_for, initials};

fn mock_code(seed: &str) -> String {
    let mut h: u32 = 5381;
    for b in seed.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u32);
    }
    format!("{:06}", h % 1_000_000)
}

fn listing_of(t: &Team) -> Listing {
    Listing::from_team(t, false)
}

/// 같은 종목의 다른 팀 목록(로컬, 연결 무관). teams 비어있으면 시드 폴백.
fn local_listings(ctx: AppCtx) -> Vec<Listing> {
    let game = *ctx.game.read();
    let my_id = ctx.my_team.read().as_ref().map(|t| t.id.clone());
    let region = ctx.scrim_region.read().clone();
    let teams = ctx.teams.read().clone();
    let src = if teams.is_empty() { shared::seed::seed_teams() } else { teams };
    src.iter()
        .filter(|t| {
            t.game == game
                && Some(&t.id) != my_id.as_ref()
                && region.as_ref().map_or(true, |r| &t.region == r)
        })
        .map(listing_of)
        .collect()
}

/// 현재 종목에서 선택 가능한 지역(국가) 목록.
fn regions_for(ctx: AppCtx) -> Vec<String> {
    let game = *ctx.game.read();
    let teams = ctx.teams.read().clone();
    let src = if teams.is_empty() { shared::seed::seed_teams() } else { teams };
    let mut regs: Vec<String> = Vec::new();
    for t in src.iter().filter(|t| t.game == game) {
        if !regs.contains(&t.region) {
            regs.push(t.region.clone());
        }
    }
    regs
}

/// 매칭 확정 처리(로컬): 스레드 생성 후 메시지함으로 이동.
fn confirm(ctx: AppCtx, opp: Listing) {
    let my_id = ctx.my_team.read().as_ref().map(|t| t.id.clone()).unwrap_or_default();
    let date = ctx.scrim_date.read().clone();
    let time = ctx.scrim_time.read().clone();
    let squad = *ctx.scrim_squad.read();
    let mid = format!("m-{my_id}-{}", opp.team_id);
    let scrim = ScrimMatch {
        id: mid.clone(),
        team_a: my_id,
        team_b: opp.team_id.clone(),
        game: opp.game,
        date: date.clone(),
        time: time.clone(),
        code: mock_code(&format!("{}{}{}{}", mid, date, time, squad.label())),
        status: MatchStatus::Confirmed,
    };
    let mut threads = ctx.threads.read().clone();
    if !threads.iter().any(|t| t.match_id == mid) {
        threads.push(Thread {
            match_id: mid.clone(),
            opponent: opp.clone(),
            scrim,
            squad_label: squad.label().to_string(),
            chat: Vec::new(),
            unread: 0,
        });
        let mut ts = ctx.threads;
        ts.set(threads);
    }
    ctx.reset_search();
    let mut a = ctx.active;
    a.set(Some(mid));
    let mut st = ctx.status;
    st.set(format!("✅ 매칭 확정! vs {}", opp.name));
    ctx.goto(Screen::Messages);
}

#[component]
pub fn Matching() -> Element {
    let ctx = use_context::<AppCtx>();

    // 검색 시뮬레이션은 화면이 유지되는 Matching 스코프에서 실행(타이머 취소 방지).
    use_effect(move || {
        if *ctx.searching.read() {
            spawn(async move {
                TimeoutFuture::new(650).await;
                if *ctx.searching.read() && ctx.listings.read().is_empty() {
                    let mut l = ctx.listings;
                    l.set(local_listings(ctx));
                }
                // 잠시 뒤 다른 팀이 나에게 신청(수락/거절 체험용)
                TimeoutFuture::new(2800).await;
                if *ctx.searching.read() && ctx.inbox.read().is_empty() {
                    let cands = local_listings(ctx);
                    let pick = cands.iter().nth(1).or_else(|| cands.first()).cloned();
                    if let Some(from) = pick {
                        let mut ib = ctx.inbox.read().clone();
                        ib.push(InboxItem { match_id: format!("in-{}", from.team_id), from: from.clone() });
                        let mut s = ctx.inbox;
                        s.set(ib);
                        let mut st = ctx.status;
                        st.set(format!("📩 {} 가 스크림을 신청했습니다", from.name));
                    }
                }
            });
        }
    });

    let searching = *ctx.searching.read();
    let listings = ctx.listings.read().clone();
    let outgoing = ctx.outgoing.read().clone();
    let inbox_n = ctx.inbox.read().len();

    rsx! {
        h1 { class: "h-lg", "스크림 매칭" }
        p { class: "muted", "슬롯·군을 정하고 전 세계 팀을 검색해 스크림을 신청하세요." }

        if inbox_n > 0 {
            div { class: "inbox-banner mt-xl",
                "📩 새 스크림 신청 {inbox_n}건이 도착했습니다."
                button { class: "btn btn-primary", onclick: move |_| ctx.goto(Screen::Messages), "수신함 열기" }
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
    let mut date = ctx.scrim_date;
    let mut time = ctx.scrim_time;
    let squad = ctx.scrim_squad;
    let cur_squad = *squad.read();
    let cur_region = ctx.scrim_region.read().clone();
    let regions = regions_for(ctx);

    rsx! {
        div { class: "card", style: "max-width:460px;margin:0 auto;",
            h3 { class: "h-md", style: "margin-bottom:16px;", "스크림 슬롯" }
            div { class: "slot-row",
                div { class: "field", style: "margin:0;",
                    label { "DATE" }
                    input { class: "input", r#type: "date", value: "{date}", oninput: move |e| date.set(e.value()) }
                }
                div { class: "field", style: "margin:0;",
                    label { "TIME" }
                    input { class: "input", r#type: "time", value: "{time}", oninput: move |e| time.set(e.value()) }
                }
            }
            // 국가(지역) 필터
            div { class: "field", style: "margin-top:12px;margin-bottom:0;",
                label { "국가 / 리그" }
                select {
                    class: "select",
                    onchange: move |e| {
                        let v = e.value();
                        let mut r = ctx.scrim_region;
                        r.set(if v.is_empty() { None } else { Some(v) });
                    },
                    option { value: "", selected: cur_region.is_none(), "🌐 전 세계 전체" }
                    for reg in regions.iter() {
                        option { key: "{reg}", value: "{reg}", selected: cur_region.as_deref() == Some(reg.as_str()),
                            "{flag_for(reg)} {reg}" }
                    }
                }
            }
            // 1군 / 2군 / Academy 선택
            div { class: "field", style: "margin-top:12px;margin-bottom:0;",
                label { "사용할 로스터 (군)" }
                div { class: "squad-pick",
                    SquadPick { squad: Squad::First, current: cur_squad }
                    SquadPick { squad: Squad::Second, current: cur_squad }
                    SquadPick { squad: Squad::Academy, current: cur_squad }
                }
            }
            button {
                class: "btn btn-primary btn-block",
                style: "margin-top:16px;",
                onclick: move |_| {
                    let mut l = ctx.listings;
                    l.set(Vec::new());
                    let mut s = ctx.searching;
                    s.set(true);
                },
                "🔍 스크림 상대 찾기"
            }
        }
    }
}

#[component]
fn SquadPick(squad: Squad, current: Squad) -> Element {
    let ctx = use_context::<AppCtx>();
    let active = squad == current;
    let cls = if active { "squad-chip active" } else { "squad-chip" };
    rsx! {
        button {
            class: "{cls}",
            onclick: move |_| { let mut s = ctx.scrim_squad; s.set(squad); },
            "{squad.label()}"
        }
    }
}

#[component]
fn SearchingView(listings: Vec<Listing>) -> Element {
    let ctx = use_context::<AppCtx>();
    let found = !listings.is_empty();
    rsx! {
        div { class: "search-wrap",
            if !found {
                div { class: "globe", "🌐" }
                p { class: "h-md center", "전 세계 상대 검색 중…" }
                p { class: "caption center", "스크림 가능한 팀이 실시간으로 표시됩니다" }
                button { class: "btn btn-outline", style: "display:block;margin:12px auto;",
                    onclick: move |_| { let mut s = ctx.searching; s.set(false); }, "검색 취소" }
                p { class: "caption center", style: "margin-top:8px;", "상대를 찾는 중…" }
            } else {
                div { class: "connect-seq",
                    div { class: "connect-globe", "🌐" }
                    div { class: "connect-check", "✓" }
                    p { class: "h-md center", style: "margin:8px 0 2px;", "상대를 찾았습니다!" }
                    div { class: "connect-steps",
                        div { class: "cstep s1", span { class: "ci", "🔐" } "보안 채널 수립" span { class: "cok", "✓" } }
                        div { class: "cstep s2", span { class: "ci", "🤝" } "핸드셰이크" span { class: "cok", "✓" } }
                        div { class: "cstep s3", span { class: "ci", "🌐" } "상대 서버 연결" span { class: "cok", "✓" } }
                    }
                    div { class: "connect-bar", div { class: "connect-bar-fill" } }
                }
                div { class: "listing-reveal",
                    p { class: "caption center", style: "margin-bottom:8px;", "🌐 연결됨 · 스크림 가능한 팀 (국가별)" }
                    div { class: "listing-grid",
                        for l in listings.iter() {
                            ListingCard { listing: l.clone() }
                        }
                    }
                    button { class: "btn btn-outline", style: "display:block;margin:16px auto 0;",
                        onclick: move |_| { let mut s = ctx.searching; s.set(false); }, "검색 취소" }
                }
            }
        }
    }
}

#[component]
fn ListingCard(listing: Listing) -> Element {
    let ctx = use_context::<AppCtx>();
    let flag = flag_for(&listing.region);
    rsx! {
        div { class: "listing-card",
            TeamLogo { logo: listing.logo.clone(), tag: listing.tag.clone(), size: 40 }
            div { class: "listing-meta",
                div { class: "lname", "{listing.name}" }
                div { class: "lregion", "{flag} {listing.region}" }
            }
            button {
                class: "btn btn-primary",
                onclick: {
                    let opp = listing.clone();
                    move |_| {
                        let opp = opp.clone();
                        let mid = format!("out-{}", opp.team_id);
                        let mut o = ctx.outgoing; o.set(Some((mid, opp.clone())));
                        let mut st = ctx.status; st.set("신청이 완료되었습니다 — 상대 수락 대기 중".into());
                        spawn(async move {
                            TimeoutFuture::new(1700).await;
                            if ctx.outgoing.read().is_some() {
                                confirm(ctx, opp);
                            }
                        });
                    }
                },
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
            p { class: "caption", style: "color:var(--primary-deep);font-weight:600;", "신청이 완료되었습니다" }
            div { class: "spinner-dots", span {} span {} span {} }
            p { class: "muted", "상대의 수락을 기다리는 중…" }
            button { class: "btn btn-outline", onclick: move |_| { let mut o = ctx.outgoing; o.set(None); }, "취소" }
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
