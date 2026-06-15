use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;
use shared::{Listing, MatchStatus, ScrimMatch};

use crate::state::{AppCtx, ChatMsg, Thread};
use crate::views::{flag_for, TeamLogo};

fn mock_code(seed: &str) -> String {
    let mut h: u32 = 5381;
    for b in seed.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u32);
    }
    format!("{:06}", h % 1_000_000)
}

/// 받은 신청을 수락 → 대화 스레드 생성(로컬).
fn accept_invite(ctx: AppCtx, match_id: &str, from: &Listing) {
    let my_id = ctx.my_team.read().as_ref().map(|t| t.id.clone()).unwrap_or_default();
    let date = ctx.scrim_date.read().clone();
    let time = ctx.scrim_time.read().clone();
    let squad = *ctx.scrim_squad.read();
    let scrim = ScrimMatch {
        id: match_id.to_string(),
        team_a: from.team_id.clone(),
        team_b: my_id,
        game: from.game,
        date: date.clone(),
        time: time.clone(),
        code: mock_code(&format!("{match_id}{date}{time}")),
        status: MatchStatus::Confirmed,
    };
    let mut threads = ctx.threads.read().clone();
    if !threads.iter().any(|t| t.match_id == match_id) {
        threads.push(Thread {
            match_id: match_id.to_string(),
            opponent: from.clone(),
            scrim,
            squad_label: squad.label().to_string(),
            chat: Vec::new(),
            unread: 0,
        });
        let mut ts = ctx.threads;
        ts.set(threads);
    }
    // 수신함에서 제거
    let remaining: Vec<_> = ctx.inbox.read().clone().into_iter().filter(|i| i.match_id != match_id).collect();
    let mut ib = ctx.inbox;
    ib.set(remaining);
    let mut a = ctx.active;
    a.set(Some(match_id.to_string()));
    let mut st = ctx.status;
    st.set(format!("✅ 매칭 확정! vs {}", from.name));
}

#[component]
pub fn Messages() -> Element {
    let ctx = use_context::<AppCtx>();
    let mut query = use_signal(String::new);
    let q = query.read().to_lowercase();
    let inbox: Vec<_> = ctx.inbox.read().clone().into_iter()
        .filter(|i| q.is_empty() || i.from.name.to_lowercase().contains(&q)).collect();
    let threads: Vec<_> = ctx.threads.read().clone().into_iter()
        .filter(|t| q.is_empty() || t.opponent.name.to_lowercase().contains(&q)).collect();
    let sent: Vec<_> = ctx.sent.read().clone().into_iter()
        .filter(|l| q.is_empty() || l.name.to_lowercase().contains(&q)).collect();
    let active = ctx.active.read().clone();
    let total_n = ctx.inbox.read().len() + ctx.sent.read().len() + ctx.threads.read().len();
    // 수신함에서 선택한 신청(있으면 우측에 수락/거절 표시).
    let mut sel_inbox = use_signal(|| Option::<String>::None);
    let mut sel_sent = use_signal(|| Option::<String>::None);
    let sel = sel_inbox.read().clone();
    let sel_s = sel_sent.read().clone();

    rsx! {
        div { class: "msg-dashboard",
            // ── 좌측: 목록 ──
            aside { class: "msg-list",
                div { class: "msg-list-head",
                    h2 { class: "msg-title", "Messages" }
                    if total_n > 0 { span { class: "msg-count", "{total_n}" } }
                }
                div { class: "msg-search",
                    span { class: "msg-search-ico", "🔍" }
                    input {
                        class: "msg-search-input",
                        placeholder: "팀 검색…",
                        value: "{query}",
                        oninput: move |e| query.set(e.value()),
                    }
                }

                if !inbox.is_empty() {
                    div { class: "msg-section", "📩 수신함 (스크림 신청)" }
                    for item in inbox.iter() {
                        {
                            let mid = item.match_id.clone();
                            let cls = if sel.as_deref() == Some(item.match_id.as_str()) { "msg-item active" } else { "msg-item" };
                            rsx! {
                                div { key: "{item.match_id}", class: "{cls}",
                                    onclick: move |_| {
                                        sel_inbox.set(Some(mid.clone()));
                                        let mut a = ctx.active; a.set(None);
                                    },
                                    TeamLogo { logo: item.from.logo.clone(), tag: item.from.tag.clone(), size: 44 }
                                    div { class: "msg-item-meta",
                                        div { class: "msg-item-name", "{item.from.name}" }
                                        div { class: "msg-item-sub", "{flag_for(&item.from.region)} {item.from.region} · 스크림 신청" }
                                    }
                                    span { class: "badge-new", "NEW" }
                                }
                            }
                        }
                    }
                }

                if !sent.is_empty() {
                    div { class: "msg-section", "📨 보낸 신청 (수락 대기중)" }
                    for l in sent.iter() {
                        {
                            let tid = l.team_id.clone();
                            let cls = if sel_s.as_deref() == Some(l.team_id.as_str()) { "msg-item active" } else { "msg-item" };
                            rsx! {
                                div { key: "{l.team_id}", class: "{cls}",
                                    onclick: move |_| {
                                        sel_sent.set(Some(tid.clone()));
                                        sel_inbox.set(None);
                                        let mut a = ctx.active; a.set(None);
                                    },
                                    TeamLogo { logo: l.logo.clone(), tag: l.tag.clone(), size: 44 }
                                    div { class: "msg-item-meta",
                                        div { class: "msg-item-name", "{flag_for(&l.region)} {l.name}" }
                                        div { class: "msg-item-sub", "내가 신청함 · 수락 대기중" }
                                    }
                                    span { class: "badge-wait", "대기" }
                                }
                            }
                        }
                    }
                }

                div { class: "msg-section", "💬 대화" }
                if threads.is_empty() && inbox.is_empty() && sent.is_empty() {
                    p { class: "caption", style: "padding:16px;", "아직 대화가 없습니다. 매칭이 확정되면 여기에 표시됩니다." }
                }
                for t in threads.iter() {
                    {
                        let mid = t.match_id.clone();
                        let cls = if active.as_deref() == Some(t.match_id.as_str()) { "msg-item active" } else { "msg-item" };
                        let last = t.chat.last().map(|c| c.text.clone()).unwrap_or_else(|| "매칭 확정됨".to_string());
                        rsx! {
                            div { key: "{t.match_id}", class: "{cls}",
                                onclick: move |_| {
                                    sel_inbox.set(None);
                                    let mut a = ctx.active; a.set(Some(mid.clone()));
                                    // 미읽음 리셋
                                    let mut th = ctx.threads.read().clone();
                                    if let Some(x) = th.iter_mut().find(|x| x.match_id == mid) { x.unread = 0; }
                                    let mut ts = ctx.threads; ts.set(th);
                                },
                                div { class: "msg-ava-wrap",
                                    TeamLogo { logo: t.opponent.logo.clone(), tag: t.opponent.tag.clone(), size: 44 }
                                    span { class: "online-dot" }
                                }
                                div { class: "msg-item-meta",
                                    div { class: "msg-item-name", "{flag_for(&t.opponent.region)} {t.opponent.name}" }
                                    div { class: "msg-item-sub", "{last}" }
                                }
                                if t.unread > 0 { span { class: "badge-unread", "{t.unread}" } }
                            }
                        }
                    }
                }
            }

            // ── 우측: 대화/신청 상세 ──
            main { class: "msg-conversation",
                {
                    if let Some(mid) = sel.clone() {
                        let item = inbox.iter().find(|i| i.match_id == mid).cloned();
                        match item {
                            Some(it) => rsx! { InvitePane { match_id: it.match_id.clone(), from: it.from.clone(), sel_inbox } },
                            None => rsx! { EmptyPane {} },
                        }
                    } else if let Some(tid) = sel_s.clone() {
                        let it = sent.iter().find(|l| l.team_id == tid).cloned();
                        match it {
                            Some(l) => rsx! { SentPane { team: l, sel_sent } },
                            None => rsx! { EmptyPane {} },
                        }
                    } else if let Some(mid) = active.clone() {
                        let thread = threads.iter().find(|t| t.match_id == mid).cloned();
                        match thread {
                            Some(t) => rsx! { ChatPane { thread: t } },
                            None => rsx! { EmptyPane {} },
                        }
                    } else {
                        rsx! { EmptyPane {} }
                    }
                }
            }
        }
    }
}

#[component]
fn SentPane(team: Listing, sel_sent: Signal<Option<String>>) -> Element {
    let ctx = use_context::<AppCtx>();
    let tid = team.team_id.clone();
    rsx! {
        div { class: "conv-head",
            TeamLogo { logo: team.logo.clone(), tag: team.tag.clone(), size: 44 }
            div { div { class: "conv-name", "{team.name}" } div { class: "conv-sub", "{team.region}" } }
        }
        div { class: "msg-empty",
            div { style: "font-size:40px;", "📨" }
            h3 { class: "h-md", "{team.name} 에게 스크림을 신청했습니다" }
            p { class: "muted", "메시지 큐에 전달됨 · 상대가 수락하면 대화가 시작됩니다." }
            button {
                class: "btn btn-danger",
                style: "margin-top:16px;",
                onclick: move |_| {
                    // 신청 취소 → 목록으로 복귀(다시 신청 가능)
                    let remaining: Vec<_> = ctx.sent.read().clone().into_iter().filter(|l| l.team_id != tid).collect();
                    let mut s = ctx.sent; s.set(remaining);
                    sel_sent.set(None);
                    let mut st = ctx.status; st.set("신청을 취소했습니다".into());
                },
                "신청 취소"
            }
        }
    }
}

#[component]
fn EmptyPane() -> Element {
    rsx! {
        div { class: "msg-empty",
            div { style: "font-size:48px;", "💬" }
            p { class: "muted", "왼쪽에서 신청 또는 대화를 선택하세요." }
        }
    }
}

#[component]
fn InvitePane(match_id: String, from: shared::Listing, sel_inbox: Signal<Option<String>>) -> Element {
    let ctx = use_context::<AppCtx>();
    let accept_id = match_id.clone();
    let reject_id = match_id.clone();
    rsx! {
        div { class: "conv-head",
            TeamLogo { logo: from.logo.clone(), tag: from.tag.clone(), size: 44 }
            div { div { class: "conv-name", "{from.name}" } div { class: "conv-sub", "{from.region}" } }
        }
        div { class: "msg-empty",
            div { style: "font-size:40px;", "🤝" }
            h3 { class: "h-md", "{from.name} 가 스크림을 신청했습니다" }
            p { class: "muted", "수락하면 비밀 코드가 발급되고 대화가 시작됩니다." }
            div { class: "row-gap", style: "margin-top:16px;",
                button {
                    class: "btn btn-primary",
                    onclick: {
                        let from = from.clone();
                        move |_| {
                            accept_invite(ctx, &accept_id, &from);
                            sel_inbox.set(None);
                        }
                    },
                    "수락"
                }
                button {
                    class: "btn btn-danger",
                    onclick: move |_| {
                        let remaining: Vec<_> = ctx.inbox.read().clone().into_iter().filter(|i| i.match_id != reject_id).collect();
                        let mut ib = ctx.inbox; ib.set(remaining);
                        let mut st = ctx.status; st.set("신청을 거절했습니다".into());
                        sel_inbox.set(None);
                    },
                    "거절"
                }
            }
        }
    }
}

#[component]
fn ChatPane(thread: crate::state::Thread) -> Element {
    let ctx = use_context::<AppCtx>();
    let my_team = ctx.my_team.read().clone();
    let my_name = my_team.as_ref().map(|t| t.name.clone()).unwrap_or_default();
    let mut draft = use_signal(String::new);
    let mid = thread.match_id.clone();

    let send = move |mid: String, my_name: String, draft: &mut Signal<String>| {
        let text = draft.read().trim().to_string();
        if text.is_empty() { return; }
        {
            let mut th = ctx.threads.read().clone();
            if let Some(t) = th.iter_mut().find(|t| t.match_id == mid) {
                t.chat.push(ChatMsg { mine: true, name: my_name.clone(), text });
            }
            let mut ts = ctx.threads; ts.set(th);
        }
        draft.set(String::new());
        // 상대 자동 응답(데모)
        let mid2 = mid.clone();
        spawn(async move {
            TimeoutFuture::new(1100).await;
            let mut th = ctx.threads.read().clone();
            if let Some(t) = th.iter_mut().find(|t| t.match_id == mid2) {
                let name = t.opponent.name.clone();
                let replies = [
                    "좋습니다! 그 시간 가능합니다 👍",
                    "콜! 디스코드로 바로 들어갈게요",
                    "오케이, 풀 5인 준비됐습니다",
                    "넵 그때 봬요. 코드 확인했습니다",
                ];
                let idx = t.chat.len() % replies.len();
                t.chat.push(ChatMsg { mine: false, name, text: replies[idx].to_string() });
                let mut ts = ctx.threads; ts.set(th);
            }
        });
    };

    rsx! {
        div { class: "conv-head",
            TeamLogo { logo: thread.opponent.logo.clone(), tag: thread.opponent.tag.clone(), size: 44 }
            div {
                div { class: "conv-name", "{thread.opponent.name}" }
                div { class: "conv-sub", "{thread.squad_label} · {thread.scrim.date} {thread.scrim.time} · CODE {thread.scrim.code}" }
            }
        }
        div { class: "chat-log conv-log",
            if thread.chat.is_empty() {
                p { class: "caption center", style: "padding:16px;", "인사를 건네보세요. 일정·맵·서버를 조율하세요." }
            }
            for (i, m) in thread.chat.iter().enumerate() {
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
                    let mut send = send.clone();
                    move |e: Event<KeyboardData>| {
                        if e.key() == Key::Enter { send(mid.clone(), my_name.clone(), &mut draft); }
                    }
                },
            }
            button {
                class: "btn btn-primary",
                onclick: {
                    let mid = mid.clone();
                    let my_name = my_name.clone();
                    let mut send = send.clone();
                    move |_| send(mid.clone(), my_name.clone(), &mut draft)
                },
                "전송"
            }
        }
    }
}
