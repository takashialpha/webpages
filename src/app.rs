use leptos::prelude::*;
use leptos_meta::{Link, provide_meta_context};

use crate::fs;
use crate::seo::Seo;
use crate::terminal::Terminal;
use crate::{short_build, uname};

// The document shell only ever renders on the server; the browser hydrates the
// body it produced. Keeping it out of the WASM build drops the whole head from
// the bundle.
#[cfg(not(feature = "hydrate"))]
use leptos_meta::{HashedStylesheet, MetaTags};

#[cfg(not(feature = "hydrate"))]
use crate::{server_uptime_secs, theme};

/// Reapplies the stored palette before the first paint. Kept in sync with
/// [`theme::STORAGE_KEY`] by hand, because it has to run before the WASM loads.
#[cfg(not(feature = "hydrate"))]
const THEME_SCRIPT: &str = "try{var t=localStorage.getItem('theme');\
if(t)document.documentElement.setAttribute('data-theme',t)}catch(e){}";

#[cfg(not(feature = "hydrate"))]
#[must_use]
pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en" data-uname=uname() data-uptime=server_uptime_secs().to_string() data-theme=theme::DEFAULT>
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <meta name="theme-color" content="#000000"/>
                // Applies the stored palette before first paint, so a
                // visitor who picked one does not watch the default flash
                // past first. Inert if storage is unavailable.
                <script inner_html=THEME_SCRIPT></script>
                <AutoReload options=options.clone()/>
                // Resolves the content-hashed stylesheet name from `hash.txt`,
                // so this cannot be a fixed href. Ahead of the hydration
                // scripts so the CSS request goes out first.
                <HashedStylesheet options=options.clone() id="leptos"/>
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

/// The login banner, and the only markup the server sends. Everything a crawler
/// or a visitor without JavaScript sees comes from here, so the prose in the
/// middle is the site's entire indexable surface.
#[component]
fn Banner() -> impl IntoView {
    view! {
        <div class="banner">
            <p class="line dim">{uname()}</p>
            <p class="line">""</p>
            <p class="line">
                <span class="dim">" * source:   "</span>
                <a href="https://github.com/takashialpha/webpages">
                    "https://github.com/takashialpha/webpages"
                </a>
            </p>
            <p class="line">
                <span class="dim">" * build:    "</span>
                <span class="accent">{short_build()}</span>
            </p>
            <p class="line">""</p>
            // The intro is `about.txt`, not a second copy of it. One place to
            // edit, and it flows rather than carrying hard wraps that would
            // wrap again on a narrow screen.
            <p class="line">{fs::intro()}</p>
            <p class="line">""</p>
            <p class="line dim">
                "type " <span class="accent">"help"</span> " for the command list, or "
                <span class="accent">"ls"</span> " to look around."
            </p>
            <p class="line">""</p>
        </div>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Seo/>

        // The VGA console font is self-hosted from /public/fonts (see main.css).
        <Link
            rel="preload"
            href="/fonts/vga.woff2"
            as_="font"
            type_="font/woff2"
            crossorigin=""
        />

        <Link rel="icon" type_="image/x-icon" href="/favicon.ico"/>
        <Link rel="icon" type_="image/png" sizes="32x32" href="/favicon-32x32.png"/>
        <Link rel="icon" type_="image/png" sizes="16x16" href="/favicon-16x16.png"/>
        <Link rel="apple-touch-icon" sizes="180x180" href="/apple-touch-icon.png"/>
        <Link rel="manifest" href="/site.webmanifest"/>

        <Terminal>
            <Banner/>
        </Terminal>
    }
}
