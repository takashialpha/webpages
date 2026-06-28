use leptos::prelude::*;
use leptos_meta::{Link, MetaTags, Stylesheet, provide_meta_context};
use leptos_router::{
    StaticSegment,
    components::{Route, Router, Routes},
};

use crate::pages::audium::Audium;
use crate::pages::home::{HomePage, NotFound};
use crate::pages::swagsh::Swagsh;
use crate::pages::takashialpha::Takashialpha;

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <AutoReload options=options.clone() />
                <HydrationScripts options/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/webpages.css"/>

        // JetBrains Mono is self-hosted from /public/fonts (see @font-face in main.css).
        // Preload the upright weight that paints first.
        <Link
            rel="preload"
            href="/fonts/jetbrains-mono-400.woff2"
            as_="font"
            type_="font/woff2"
            crossorigin=""
        />

        <Link rel="icon" type_="image/x-icon" href="/favicon.ico"/>
        <Link rel="icon" type_="image/png" sizes="32x32" href="/favicon-32x32.png"/>
        <Link rel="icon" type_="image/png" sizes="16x16" href="/favicon-16x16.png"/>
        <Link rel="apple-touch-icon" sizes="180x180" href="/apple-touch-icon.png"/>
        <Link rel="manifest" href="/site.webmanifest"/>

        <Router>
            <main>
                <Routes fallback=|| view! { <NotFound/> }>
                    <Route path=StaticSegment("") view=HomePage/>
                    <Route path=StaticSegment("takashialpha") view=Takashialpha/>
                    <Route path=StaticSegment("audium") view=Audium/>
                    <Route path=StaticSegment("swagsh") view=Swagsh/>
                </Routes>
            </main>
        </Router>
    }
}
