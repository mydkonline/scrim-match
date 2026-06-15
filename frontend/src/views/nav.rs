use dioxus::prelude::*;

use crate::state::{AppCtx, Screen};

#[component]
pub fn NavBar() -> Element {
    let ctx = use_context::<AppCtx>();
    let screen = *ctx.screen.read();
    let my = ctx.my_team.read().clone();
    let online = *ctx.online.read();
    let unread = ctx.unread_count();

    let (pill_cls, pill_txt) = if online {
        ("status-pill online", "● online")
    } else {
        ("status-pill offline", "● demo")
    };

    rsx! {
        nav { class: "nav",
            div { class: "brand", style: "cursor:pointer;",
                onclick: move |_| ctx.goto(Screen::Matching),
                span { class: "dot" } "Scrim.GG"
            }
            div { class: "nav-links",
                NavBtn { label: "Matching", target: Screen::Matching, current: screen }
                NavBtn { label: "Messages", target: Screen::Messages, current: screen, badge: unread }
                NavBtn { label: "Team", target: Screen::Team, current: screen }
                NavBtn { label: "Calendar", target: Screen::Calendar, current: screen }
            }
            div { class: "nav-right",
                {
                    if let Some(t) = my {
                        rsx! { span { class: "team-badge", "팀 ", b { "{t.name}" } } }
                    } else {
                        rsx! {}
                    }
                }
                span { class: "{pill_cls}", "{pill_txt}" }
            }
        }
    }
}

#[component]
fn NavBtn(label: String, target: Screen, current: Screen, #[props(default = 0)] badge: usize) -> Element {
    let ctx = use_context::<AppCtx>();
    let cls = if target == current { "active" } else { "" };
    rsx! {
        button {
            class: "{cls}",
            onclick: move |_| ctx.goto(target),
            "{label}"
            if badge > 0 { span { class: "nav-badge", "{badge}" } }
        }
    }
}
