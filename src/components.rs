//! Small presentational components shared across pages.

use leptos::prelude::*;
use leptos_router::components::A;

/// Terminal-style "back to index" link shown at the top of every sub-page.
#[component]
pub fn BackNav() -> impl IntoView {
    view! {
        <nav class="topnav reveal">
            <A href="/" attr:class="back">
                <span class="back-arrow">"←"</span>
                <span class="prompt">"$ "</span>
                <span class="back-cmd">"cd .."</span>
            </A>
        </nav>
    }
}

/// The chrome bar of a terminal window: traffic-light dots and a title.
#[component]
pub fn TermBar(#[prop(into)] title: String) -> impl IntoView {
    view! {
        <div class="term-bar">
            <span class="dot red"></span>
            <span class="dot yellow"></span>
            <span class="dot green"></span>
            <span class="term-title">{title}</span>
        </div>
    }
}

/// A feature card: emoji icon, title, and a free-form body.
#[component]
pub fn Feature(icon: &'static str, title: &'static str, children: Children) -> impl IntoView {
    view! {
        <div class="card feature">
            <span class="feature-icon">{icon}</span>
            <h3>{title}</h3>
            <p>{children()}</p>
        </div>
    }
}
