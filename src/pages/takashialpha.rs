//! `/takashialpha`, personal page.

use std::cell::RefCell;
use std::time::Duration;

use leptos::prelude::*;

use crate::components::{BackNav, Footer, JsonLd, PromptLine, Seo, TermBar};

/// schema.org `ProfilePage` whose `mainEntity` is the site owner. This is the
/// page actually about the person, so the entity lives here.
const PROFILE_LD: &str = include_str!("../../schema/profile.json");

/// A client-side typewriter that cycles through phrases, char by char.
#[component]
fn Typewriter(phrases: Vec<&'static str>) -> impl IntoView {
    /// Delay between ticks.
    const TICK: Duration = Duration::from_millis(55);
    /// Ticks to wait before typing a phrase's first character.
    const START_PAUSE: u32 = 6;
    /// Ticks to hold a fully typed phrase before deleting it.
    const HOLD_PAUSE: u32 = 36;
    /// Ticks to wait after deleting before starting the next phrase.
    const NEXT_PAUSE: u32 = 4;

    /// Where the typewriter is in its type/hold/delete cycle.
    struct Cursor {
        phrase: usize,
        shown: usize,
        pause: u32,
        typing: bool,
    }

    let displayed = RwSignal::new(String::new());

    // Effects only run on the client, so the interval never touches SSR.
    Effect::new(move |_| {
        let phrases = phrases.clone();
        let cursor = RefCell::new(Cursor {
            phrase: 0,
            shown: 0,
            pause: START_PAUSE,
            typing: true,
        });

        set_interval(
            move || {
                let mut c = cursor.borrow_mut();
                if c.pause > 0 {
                    c.pause -= 1;
                    return;
                }
                let phrase = phrases[c.phrase];
                let len = phrase.chars().count();
                if c.typing {
                    if c.shown < len {
                        c.shown += 1;
                        displayed.set(phrase.chars().take(c.shown).collect());
                        if c.shown == len {
                            c.pause = HOLD_PAUSE;
                            c.typing = false;
                        }
                    }
                } else if c.shown > 0 {
                    c.shown -= 1;
                    displayed.set(phrase.chars().take(c.shown).collect());
                } else {
                    c.typing = true;
                    c.phrase = (c.phrase + 1) % phrases.len();
                    c.pause = NEXT_PAUSE;
                }
            },
            TICK,
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

#[component]
pub fn Takashialpha() -> impl IntoView {
    view! {
        <Seo
            title="whoami · takashialpha"
            description="systems-minded developer who lives in the terminal. low-level Rust, clean abstractions, and tools that feel good the second you run them."
            path="/takashialpha"
            og_type="profile"
        />
        <JsonLd json=PROFILE_LD/>
        <div class="page">
            <div class="bg-grid" aria-hidden="true"></div>
            <div class="bg-glow" aria-hidden="true"></div>

            <div class="wrap">

                <BackNav/>

                // ── hero terminal ─────────────────────────────────────────
                <header class="hero reveal">
                    <div class="term">
                        <TermBar title="takashialpha@arch: ~"/>
                        <div class="term-body">
                            <PromptLine user="takashialpha" host="arch" cmd="whoami"/>
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
                                "comfortable end to end, syscalls, memory management, init, "
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
                                "network infrastructure across the whole path, backend "
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
                            desc="a keyboard-driven terminal music player."
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
                        <ProjectRow
                            name="niri-takashialpha"
                            href="https://github.com/takashialpha/niri-takashialpha"
                            desc="a leaner personal fork of the niri wayland compositor."
                            tag=Some("fork")
                        />
                        <ProjectRow
                            name="webpages"
                            href="https://github.com/takashialpha/webpages"
                            desc="this site, leptos, ssr, hand-written css."
                            tag=Some("you're here")
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
                <Footer msg="thanks for stopping by"/>

            </div>
        </div>
    }
}
