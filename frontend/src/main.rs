//! Scrim.GG 프론트엔드 — Dioxus(web/WASM).
//! Supabase 디자인 언어 기반 4화면: Login → Matching → Team → Calendar.

mod net;
mod state;
mod views;

use dioxus::prelude::*;
use state::{AppCtx, Screen};

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let ctx = AppCtx::new();
    use_context_provider(|| ctx);

    // 마운트 시 팀 목록 적재.
    use_future(move || async move {
        let mut teams = ctx.teams;
        teams.set(net::fetch_teams().await);
    });

    let screen = *ctx.screen.read();

    if screen == Screen::Login {
        return rsx! { views::Login {} };
    }

    let status = ctx.status.read().clone();

    rsx! {
        div { class: "app",
            views::NavBar {}
            div { class: "page",
                {match screen {
                    Screen::Matching => rsx! { views::Matching {} },
                    Screen::Team => rsx! { views::TeamSetting {} },
                    Screen::Calendar => rsx! { views::Calendar {} },
                    Screen::Login => rsx! {},
                }}
            }
        }
        {
            if status.is_empty() {
                rsx! {}
            } else {
                rsx! { div { class: "toast", "{status}" } }
            }
        }
    }
}
