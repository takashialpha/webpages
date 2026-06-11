use leptos::prelude::*;
use leptos_meta::{Link, MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::{
    StaticSegment,
    components::{A, Route, Router, Routes},
};

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

        <Title text="webpages"/>

        <Router>
            <main>
                <Routes fallback=|| view! { <NotFound/> }>
                    <Route path=StaticSegment("") view=HomePage/>
                    <Route path=StaticSegment("takashialpha") view=Takashialpha/>
                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn HomePage() -> impl IntoView {
    view! {
        <Title text="webpages"/>
        <section class="landing">
            <div class="landing-inner">
                <p class="landing-prompt">
                    <span class="path">"~"</span>
                    <span class="sep">" $ "</span>
                    <span class="cmd">"ls ./pages"</span>
                </p>
                <a class="landing-link" href="/takashialpha">"takashialpha/"</a>
            </div>
        </section>
    }
}

#[component]
fn NotFound() -> impl IntoView {
    view! {
        <Title text="404 — not found"/>
        <section class="landing">
            <div class="landing-inner">
                <p class="landing-prompt">
                    <span class="path">"~"</span>
                    <span class="sep">" $ "</span>
                    <span class="cmd">"cd ./here"</span>
                </p>
                <p class="landing-error">"cd: no such file or directory"</p>
                <a class="landing-link" href="/takashialpha">"cd ~/takashialpha"</a>
            </div>
        </section>
    }
}

/// A client-side typewriter that cycles through phrases, char by char.
#[component]
fn Typewriter(phrases: Vec<&'static str>) -> impl IntoView {
    let displayed = RwSignal::new(String::new());

    // Effects only run on the client, so the interval never touches SSR.
    Effect::new(move |_| {
        use std::cell::RefCell;
        use std::rc::Rc;
        use std::time::Duration;

        let phrases = phrases.clone();
        // (phrase_idx, char_idx, pause_ticks, typing)
        let state = Rc::new(RefCell::new((0usize, 0usize, 6i32, true)));

        set_interval(
            move || {
                let mut st = state.borrow_mut();
                if st.2 > 0 {
                    st.2 -= 1;
                    return;
                }
                let chars: Vec<char> = phrases[st.0].chars().collect();
                if st.3 {
                    if st.1 < chars.len() {
                        st.1 += 1;
                        displayed.set(chars[..st.1].iter().collect());
                        if st.1 == chars.len() {
                            st.2 = 36; // hold the finished line
                            st.3 = false;
                        }
                    }
                } else if st.1 > 0 {
                    st.1 -= 1;
                    displayed.set(chars[..st.1].iter().collect());
                } else {
                    st.3 = true;
                    st.0 = (st.0 + 1) % phrases.len();
                    st.2 = 4;
                }
            },
            Duration::from_millis(55),
        );
    });

    view! {
        <span class="tw">
            <span class="tw-text">{move || displayed.get()}</span>
            <span class="tw-caret" aria-hidden="true"></span>
        </span>
    }
}

#[component]
fn Takashialpha() -> impl IntoView {
    view! {
        <Title text="takashialpha — systems dev"/>
        <div class="page">
            <div class="bg-grid" aria-hidden="true"></div>
            <div class="bg-glow" aria-hidden="true"></div>

            <div class="wrap">

                // ── back to index ─────────────────────────────────────────
                <nav class="topnav reveal">
                    <A href="/" attr:class="back">
                        <span class="back-arrow">"←"</span>
                        <span class="prompt">"$ "</span>
                        <span class="back-cmd">"cd .."</span>
                    </A>
                </nav>

                // ── hero terminal ─────────────────────────────────────────
                <header class="hero reveal">
                    <div class="term">
                        <div class="term-bar">
                            <span class="dot red"></span>
                            <span class="dot yellow"></span>
                            <span class="dot green"></span>
                            <span class="term-title">"takashialpha@arch: ~"</span>
                        </div>
                        <div class="term-body">
                            <p class="line">
                                <span class="usr">"takashialpha"</span>
                                <span class="at">"@"</span>
                                <span class="host">"arch"</span>
                                <span class="path">" ~ "</span>
                                <span class="prompt">"$ "</span>
                                <span class="run">"whoami"</span>
                            </p>
                            <h1 class="name">"takashialpha"</h1>
                            <p class="tagline">
                                <span class="prompt-sm">"// "</span>
                                <Typewriter phrases=vec![
                                    "i build things that feel fast, clean, and make sense.",
                                    "terminal-first. performance-focused.",
                                    "if it's messy, i'll rewrite it.",
                                ]/>
                            </p>
                            <div class="hero-links">
                                <a class="btn primary" href="https://github.com/takashialpha"
                                    target="_blank" rel="noreferrer">
                                    "github ↗"
                                </a>
                                <a class="btn" href="#projects">"./projects"</a>
                            </div>
                        </div>
                    </div>
                </header>

                // ── about ─────────────────────────────────────────────────
                <section class="block reveal">
                    <h2 class="heading"><span class="hash">"## "</span>"about"</h2>
                    <p class="lede">
                        "systems-minded dev who basically lives in the terminal. i like "
                        "low-level work, clean abstractions, and tools that feel good the "
                        "second you run them. been on linux for years and i'm most at home "
                        "close to the metal."
                    </p>
                    <p class="muted">"if something feels off, i'll probably rewrite it."</p>
                </section>

                // ── stack ─────────────────────────────────────────────────
                <section class="block reveal">
                    <h2 class="heading"><span class="hash">"## "</span>"stack"</h2>
                    <div class="cards">
                        <article class="card accent">
                            <div class="card-top">
                                <span class="card-emoji">"🦀"</span>
                                <span class="badge">"home base"</span>
                            </div>
                            <h3>"rust"</h3>
                            <p>
                                "where i spend most of my time: low-level code, terminal "
                                "apps, and systems work."
                            </p>
                        </article>
                        <article class="card">
                            <div class="card-top">
                                <span class="card-emoji">"🐧"</span>
                                <span class="badge soft">"daily driver"</span>
                            </div>
                            <h3>"linux"</h3>
                            <p>
                                "comfortable end to end — syscalls, memory management, init, "
                                "filesystems, context switches. i know where things live, and "
                                "usually why they break."
                            </p>
                        </article>
                        <article class="card">
                            <div class="card-top">
                                <span class="card-emoji">"🌐"</span>
                                <span class="badge soft">"end to end"</span>
                            </div>
                            <h3>"infrastructure"</h3>
                            <p>
                                "network infrastructure across the whole path — backend "
                                "services down to the client. routing, dns, and proxying, "
                                "with containerized workloads on docker."
                            </p>
                        </article>
                    </div>
                </section>

                // ── projects ──────────────────────────────────────────────
                <section class="block reveal" id="projects">
                    <h2 class="heading"><span class="hash">"## "</span>"rust projects"</h2>
                    <ul class="projects">
                        <ProjectRow
                            name="audium"
                            href="https://github.com/takashialpha/audium"
                            desc="terminal-first audio work."
                            tag=None
                        />
                        <ProjectRow
                            name="swagsh"
                            href="https://github.com/takashialpha/swagsh"
                            desc="a shell, the way i want it."
                            tag=None
                        />
                        <ProjectRow
                            name="carboxyl"
                            href="https://github.com/carboxyl-rs/carboxyl"
                            desc="systems tooling."
                            tag=Some("maintaining")
                        />
                    </ul>
                    <p class="muted contrib">
                        "also poked around with "
                        <a href="https://github.com/ratatui/ratatui" target="_blank" rel="noreferrer">"ratatui"</a>
                        " and "
                        <a href="https://github.com/servo/servo" target="_blank" rel="noreferrer">"servo"</a>
                        ", and landed a few contributions in "
                        <a href="https://github.com/cloudflare/foundations" target="_blank" rel="noreferrer">"cloudflare foundations"</a>
                        "."
                    </p>
                </section>

                // ── philosophy + focus ────────────────────────────────────
                <section class="block reveal two-col">
                    <div class="col">
                        <h2 class="heading"><span class="hash">"## "</span>"how i think"</h2>
                        <p>
                            "i care about design at the "<em>"system"</em>" level. code that "
                            "stays simple, readable, and modern. structure over cleverness, "
                            "clarity over magic. clean foundations make everything after them "
                            "easier."
                        </p>
                    </div>
                    <div class="col">
                        <h2 class="heading"><span class="hash">"## "</span>"focused on now"</h2>
                        <ul class="focus">
                            <li>"building more terminal-first tools"</li>
                            <li>"low-level + performance-focused projects"</li>
                            <li>"making things that actually feel good to use"</li>
                        </ul>
                    </div>
                </section>

                // ── footer ────────────────────────────────────────────────
                <footer class="foot reveal">
                    <span class="prompt">"$ "</span>
                    <span class="run">"echo "</span>
                    <span class="str">"\"thanks for stopping by\""</span>
                    <span class="caret-static"></span>
                </footer>

            </div>
        </div>
    }
}

#[component]
fn ProjectRow(
    name: &'static str,
    href: &'static str,
    desc: &'static str,
    tag: Option<&'static str>,
) -> impl IntoView {
    view! {
        <li class="project">
            <a class="project-link" href=href target="_blank" rel="noreferrer">
                <span class="project-name">
                    <span class="project-bullet">"› "</span>
                    {name}
                </span>
                <span class="project-desc">{desc}</span>
                {tag.map(|t| view! { <span class="badge soft pulse">{t}</span> })}
                <span class="project-arrow">"↗"</span>
            </a>
        </li>
    }
}
