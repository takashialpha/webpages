//! Root index (`/`) and the 404 fallback.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::components::A;

#[component]
pub fn HomePage() -> impl IntoView {
    view! {
        <Title text="webpages"/>
        <section class="landing">
            <div class="landing-inner">
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
