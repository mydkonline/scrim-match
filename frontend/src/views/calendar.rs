use dioxus::prelude::*;
use shared::CalendarEntry;

use crate::net;

fn month_of(date: &str) -> u32 {
    date.split('-').nth(1).and_then(|m| m.parse().ok()).unwrap_or(0)
}

#[component]
pub fn Calendar() -> Element {
    let entries = use_resource(|| async move { net::fetch_calendar().await });
    let mut month = use_signal(|| 6u32);
    let selected = *month.read();
    let data: Option<Vec<CalendarEntry>> = entries.read().clone();

    rsx! {
        h1 { class: "h-lg", "2026" }
        p { class: "muted", "예정·완료된 스크림 일정" }

        div { class: "cal-tabs mt-xl",
            for m in [6u32, 7, 8] {
                {
                    let cls = if selected == m { "cal-tab active" } else { "cal-tab" };
                    rsx! {
                        button { key: "{m}", class: "{cls}", onclick: move |_| month.set(m), "{m}월" }
                    }
                }
            }
        }

        div { class: "cal-list",
            {
                match data {
                    Some(list) => {
                        let rows: Vec<CalendarEntry> =
                            list.into_iter().filter(|e| month_of(&e.date) == selected).collect();
                        if rows.is_empty() {
                            rsx! { p { class: "muted", "이 달에는 일정이 없습니다." } }
                        } else {
                            rsx! {
                                for e in rows.iter() {
                                    div { key: "{e.date}-{e.opponent}", class: "cal-row",
                                        span { class: "date", "{e.date}" }
                                        span { class: "vs", "VS {e.opponent}  ·  {e.game.short()}" }
                                        {
                                            match &e.result {
                                                Some(r) => rsx! { span { class: "result win", "{r}" } },
                                                None => rsx! { span { class: "result muted", "예정" } },
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    None => rsx! { p { class: "muted", "불러오는 중…" } },
                }
            }
        }
    }
}
