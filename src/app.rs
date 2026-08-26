use leptos::prelude::*;
use leptos_meta::{Link, provide_meta_context};

use crate::fs;
use crate::seo::Seo;
use crate::terminal::Terminal;
use crate::{short_build, uname};

// The document shell only ever renders on the server; the browser hydrates the
// body it produced. Keeping it out of the WASM build drops the whole head from
// the bundle. Gated on `ssr` rather than `not(hydrate)` so that enabling both
// features at once, as `--all-features` does, still leaves the server binary
// with a shell to render.
#[cfg(feature = "ssr")]
use leptos_meta::{HashedStylesheet, MetaTags};

#[cfg(feature = "ssr")]
use crate::{clock, theme};

#[cfg(feature = "ssr")]
#[must_use]
pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html
            lang="en"
            data-uname=uname()
            // The server's clock, which the browser carries forward rather than
            // consulting its own. See clock.rs.
            data-time=clock::now_millis().to_string()
            data-uptime=clock::uptime_secs().to_string()
            data-theme=theme::DEFAULT
        >
            <head>
                <meta charset="utf-8"/>
                // `interactive-widget` is what keeps the on-screen keyboard
                // from burying the key row. By default a virtual keyboard
                // shrinks only the visual viewport, so a fixed element still
                // sits against the bottom of the layout viewport, behind the
                // keys. Resizing the content instead shrinks the layout
                // viewport, which is also what makes `100dvh` mean the part of
                // the screen you can actually see.
                <meta
                    name="viewport"
                    content="width=device-width, initial-scale=1, interactive-widget=resizes-content"
                />
                <meta name="theme-color" content="#000000"/>
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
                <span class="dim">" * source:     "</span>
                <a
                    href="https://github.com/takashialpha/webpages"
                    target="_blank"
                    rel="noreferrer"
                >
                    "https://github.com/takashialpha/webpages"
                </a>
            </p>
            // An motd lists what the box runs, which is also the natural place
            // to credit the people whose work this is built on.
            <p class="line">
                <span class="dim">" * built with: "</span>
                <a href="https://leptos.dev" target="_blank" rel="noreferrer">"leptos"</a>
                <span class="dim">" and "</span>
                <a
                    href="https://github.com/tokio-rs/axum"
                    target="_blank"
                    rel="noreferrer"
                >
                    "axum"
                </a>
                <span class="dim">", in rust"</span>
            </p>
            <p class="line">
                <span class="dim">" * build:      "</span>
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
