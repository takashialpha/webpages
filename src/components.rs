//! Small presentational components shared across pages.

use leptos::prelude::*;
use leptos_meta::{Link, Meta, Title};
use leptos_router::components::A;

use crate::SITE_URL;

/// Per-page SEO head tags: title, description, canonical URL, and the
/// Open Graph / Twitter card block. `path` is the page's route (e.g.
/// `/audium`), empty for the home page. `og_type` is the Open Graph object
/// type, defaulting to `website`; person pages should pass `profile`.
#[component]
pub fn Seo(
    title: &'static str,
    description: &'static str,
    path: &'static str,
    #[prop(default = "website")] og_type: &'static str,
) -> impl IntoView {
    let url = format!("{SITE_URL}{path}");
    let image = format!("{SITE_URL}/og.png");
    let alt = "takashialpha terminal-style banner";
    view! {
        <Title text=title/>
        <Meta name="description" content=description/>
        <Link rel="canonical" href=url.clone()/>

        <Meta property="og:type" content=og_type/>
        <Meta property="og:site_name" content="takashialpha"/>
        <Meta property="og:locale" content="en_US"/>
        <Meta property="og:title" content=title/>
        <Meta property="og:description" content=description/>
        <Meta property="og:url" content=url/>
        <Meta property="og:image" content=image.clone()/>
        <Meta property="og:image:width" content="1200"/>
        <Meta property="og:image:height" content="630"/>
        <Meta property="og:image:type" content="image/png"/>
        <Meta property="og:image:alt" content=alt/>

        <Meta name="twitter:card" content="summary_large_image"/>
        <Meta name="twitter:title" content=title/>
        <Meta name="twitter:description" content=description/>
        <Meta name="twitter:image" content=image/>
        <Meta name="twitter:image:alt" content=alt/>
    }
}

/// Emits a `<script type="application/ld+json">` block of structured data.
/// `inner_html` keeps the JSON raw, since script content is not HTML-decoded.
#[component]
pub fn JsonLd(json: &'static str) -> impl IntoView {
    view! { <script type="application/ld+json" inner_html=json></script> }
}

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

/// A titled blurb used in the "why" / comparison grids.
#[component]
pub fn WhyItem(title: &'static str, children: Children) -> impl IntoView {
    view! {
        <div class="a-why-item">
            <h3>{title}</h3>
            <p>{children()}</p>
        </div>
    }
}

/// The `user@host ~ $ cmd` line at the top of a hero terminal.
#[component]
pub fn PromptLine(user: &'static str, host: &'static str, cmd: &'static str) -> impl IntoView {
    view! {
        <p class="line">
            <span class="usr">{user}</span>
            <span class="at">"@"</span>
            <span class="host">{host}</span>
            <span class="path">" ~ "</span>
            <span class="prompt">"$ "</span>
            <span class="run">{cmd}</span>
        </p>
    }
}

/// Terminal-style page footer: an `echo` of a closing one-liner.
#[component]
pub fn Footer(msg: &'static str) -> impl IntoView {
    view! {
        <footer class="foot reveal">
            <span class="prompt">"$ "</span>
            <span class="run">"echo "</span>
            <span class="str">{format!("\"{msg}\"")}</span>
            <span class="caret-static"></span>
        </footer>
    }
}
