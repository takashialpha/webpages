//! Root index (`/`) and the 404 fallback.

use leptos::prelude::*;
use leptos_meta::{Meta, Title};
use leptos_router::components::A;

use crate::components::{JsonLd, Seo};

/// schema.org `WebSite` entity for the root. Helps search engines settle on
/// the site name shown in results.
const WEBSITE_LD: &str = include_str!("../../schema/website.json");

#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <Seo
            title="takashialpha"
            description="takashialpha, systems developer who lives in the terminal. low-level Rust, terminal-first tools, and projects like audium and swagsh."
            path=""
        />
        <JsonLd json=WEBSITE_LD/>
        <section class="landing">
            <div class="landing-inner">
                <h1 class="sr-only">"takashialpha"</h1>
                <p class="landing-prompt">
                    <span class="path">"~"</span>
                    <span class="sep">" $ "</span>
                    <span class="cmd">"ls ./pages"</span>
                </p>
                <div class="landing-links">
                    <A href="/takashialpha" attr:class="landing-link">"takashialpha/"</A>
                    <A href="/audium" attr:class="landing-link">"audium/"</A>
                    <A href="/swagsh" attr:class="landing-link">"swagsh/"</A>
                </div>
            </div>
        </section>
    }
}

#[component]
pub fn NotFound() -> impl IntoView {
    view! {
        <Title text="404, not found"/>
        <Meta name="robots" content="noindex"/>
        <section class="landing">
            <div class="landing-inner">
                <p class="landing-prompt">
                    <span class="path">"~"</span>
                    <span class="sep">" $ "</span>
                    <span class="cmd">"cd ./here"</span>
                </p>
                <p class="landing-error">"cd: no such file or directory"</p>
                <A href="/" attr:class="landing-link">"cd ~"</A>
            </div>
        </section>
    }
}
