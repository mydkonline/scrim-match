use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;
use shared::{ClientMsg, Listing, Squad, Team};

use crate::state::{AppCtx, InboxItem, Screen, SentReq};
use crate::views::{flag_for, initials};

pub fn gen_code(seed: &str) -> String {
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
    let inbox_n = ctx.inbox.read().len();
    let sent_n = ctx.sent.read().len();

    rsx! {
        h1 { class: "h-lg", "스크림 매칭" }
        p { class: "muted", "슬롯·군을 정하고 전 세계 팀을 검색해 스크림을 신청하세요." }

        if inbox_n > 0 || sent_n > 0 {
            div { class: "inbox-banner mt-xl",
                if inbox_n > 0 { "📩 받은 신청 {inbox_n}건" } else { "📨 보낸 신청 {sent_n}건 (수락 대기중)" }
                button { class: "btn btn-primary", onclick: move |_| ctx.goto(Screen::Messages), "메시지함 열기" }
            }
        }

        div { class: "mt-xl",
            if searching {
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
    let mut fr = use_signal(|| Option::<String>::None);
    let cur = fr.read().clone();
    // 리스트에 존재하는 지역 목록
    let mut regions: Vec<String> = Vec::new();
    for l in listings.iter() {
        if !regions.contains(&l.region) { regions.push(l.region.clone()); }
    }
    let shown: Vec<Listing> = listings
        .iter()
        .filter(|l| cur.as_ref().map_or(true, |r| &l.region == r))
        .cloned()
        .collect();
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
                    p { class: "caption center", style: "margin-bottom:10px;", "🌐 연결됨 · 스크림 가능한 팀 (국가별)" }
                    // 국가/리그 필터 칩
                    div { class: "region-chips",
                        {
                            let active = cur.is_none();
                            let cls = if active { "region-chip active" } else { "region-chip" };
                            rsx! { button { class: "{cls}", onclick: move |_| fr.set(None), "🌐 전체 {listings.len()}" } }
                        }
                        for reg in regions.iter() {
                            {
                                let r = reg.clone();
                                let n = listings.iter().filter(|l| &l.region == reg).count();
                                let active = cur.as_deref() == Some(reg.as_str());
                                let cls = if active { "region-chip active" } else { "region-chip" };
                                rsx! { button { key: "{reg}", class: "{cls}", onclick: move |_| fr.set(Some(r.clone())), "{flag_for(reg)} {reg} {n}" } }
                            }
                        }
                    }
                    div { class: "listing-grid",
                        for l in shown.iter() {
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
    let applied = ctx.sent.read().iter().any(|r| r.listing.team_id == listing.team_id);
    let online = *ctx.online.read();
    rsx! {
        div { class: "listing-card",
            TeamLogo { logo: listing.logo.clone(), tag: listing.tag.clone(), size: 40 }
            div { class: "listing-meta",
                div { class: "lname", "{listing.name}" }
                div { class: "lregion", "{flag} {listing.region}" }
            }
            if applied {
                button { class: "btn btn-outline", disabled: true, "신청됨 ✓" }
            } else {
                button {
                    class: "btn btn-primary",
                    onclick: {
                        let opp = listing.clone();
                        move |_| {
                            if ctx.sent.read().iter().any(|r| r.listing.team_id == opp.team_id) { return; }
                            if online {
                                // 서버 메시지 큐에 적재 → Applied 로 코드 수신
                                ctx.send(ClientMsg::ApplyQueue {
                                    target_team: opp.team_id.clone(),
                                    date: ctx.scrim_date.read().clone(),
                                    time: ctx.scrim_time.read().clone(),
                                    squad: ctx.scrim_squad.read().label().to_string(),
                                });
                            } else {
                                // 오프라인 폴백: 로컬 코드 발급
                                let code = gen_code(&format!("{}{}", opp.team_id, ctx.scrim_time.read()));
                                let mut s = ctx.sent.read().clone();
                                s.push(SentReq { listing: opp.clone(), code: code.clone() });
                                let mut sent = ctx.sent; sent.set(s);
                                let mut st = ctx.status;
                                st.set(format!("신청 완료 — 코드 {} 전달, 상대 수락 대기중", code));
                            }
                        }
                    },
                    "신청"
                }
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
