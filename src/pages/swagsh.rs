//! `/swagsh`, landing page for the shell.

use leptos::prelude::*;
use leptos_meta::{Meta, Title};

use crate::components::{BackNav, Feature, TermBar};

#[component]
fn WhyItem(title: &'static str, children: Children) -> impl IntoView {
    view! {
        <div class="a-why-item">
            <h3>{title}</h3>
            <p>{children()}</p>
        </div>
    }
}

#[component]
pub fn Swagsh() -> impl IntoView {
    view! {
        <Title text="swagsh, a fast, minimal, modern shell"/>
        <Meta
            name="description"
            content="swagsh, a fast, minimal, modern Linux shell built in Rust. Shell grammar, expansions, tab completion, job control, a customisable prompt and persistent history."
        />
        <div class="page swagsh">
            <div class="bg-grid" aria-hidden="true"></div>
            <div class="bg-glow" aria-hidden="true"></div>

            <div class="wrap">

                <BackNav/>

                // ── hero ──────────────────────────────────────────────────
                <header class="a-hero reveal">
                    <div class="term">
                        <TermBar title="swagsh, ~"/>
                        <div class="term-body">
                            <p class="line">
                                <span class="usr">"you"</span>
                                <span class="at">"@"</span>
                                <span class="host">"arch"</span>
                                <span class="path">" ~ "</span>
                                <span class="prompt">"$ "</span>
                                <span class="run">"exec swagsh"</span>
                            </p>
                            <h1 class="name">"swagsh"</h1>
                            <p class="tagline">
                                <span class="prompt-sm">"// "</span>
                                "a shell, the way i want it"
                                <span class="a-caret" aria-hidden="true"></span>
                            </p>
                            <p class="hero-sub">
                                "a fast, minimal, modern linux shell built in rust. real shell "
                                "grammar, full expansions, job control, tab completion and a "
                                "customisable prompt. the name is swag, for stylish flair."
                            </p>
                            <div class="hero-links">
                                <a class="btn primary" href="#install">"get started"</a>
                                <a class="btn" href="https://github.com/takashialpha/swagsh"
                                    target="_blank" rel="noreferrer">"view source ↗"</a>
                            </div>
                        </div>
                    </div>

                    <div class="term">
                        <TermBar title="swagsh, session"/>
                        <div class="term-body">
                            <p class="line">
                                <span class="path">"~ "</span><span class="prompt">"$ "</span>
                                <span class="run">"for i in 1 2 3; do echo \"line $i\"; done"</span>
                            </p>
                            <p class="out">"line 1"</p>
                            <p class="out">"line 2"</p>
                            <p class="out">"line 3"</p>
                            <p class="line">
                                <span class="path">"~ "</span><span class="prompt">"$ "</span>
                                <span class="run">"name=takashi; echo \"hi, ${name}\""</span>
                            </p>
                            <p class="out">"hi, takashi"</p>
                            <p class="line">
                                <span class="path">"~ "</span><span class="prompt">"$ "</span>
                                <span class="run">"cat log | grep err | wc -l"</span>
                            </p>
                            <p class="out">"7"</p>
                            <p class="line">
                                <span class="path">"~ "</span><span class="prompt">"$ "</span>
                                <span class="a-caret" aria-hidden="true"></span>
                            </p>
                        </div>
                    </div>
                </header>

                // ── features ──────────────────────────────────────────────
                <section class="a-section reveal" id="features">
                    <p class="a-label">"features"</p>
                    <h2 class="heading">
                        <span class="hash">"## "</span>"a real shell. "<em>"no surprises."</em>
                    </h2>
                    <div class="cards">
                        <Feature icon="🧩" title="shell grammar">
                            "pipelines, redirections, control flow, functions, subshells and "
                            "here-documents."
                        </Feature>
                        <Feature icon="🔤" title="expansions">
                            "variable, parameter, tilde, glob and command substitution."
                        </Feature>
                        <Feature icon="⌨" title="tab completion">
                            "completes builtins, aliases, executables and filenames out of the box."
                        </Feature>
                        <Feature icon="🧵" title="job control">
                            "background jobs, foreground and background switching, stopping and "
                            "signalling."
                        </Feature>
                        <Feature icon="🎛" title="prompt">
                            "a customisable "<code>"$PS1"</code>" with escape sequences for the "
                            "working directory, user, host and more."
                        </Feature>
                        <Feature icon="🕘" title="history">
                            "persistent history that respects "<code>"$HISTFILE"</code>" and "
                            <code>"$HISTSIZE"</code>", with a private mode."
                        </Feature>
                    </div>
                </section>

                // ── performance ───────────────────────────────────────────
                <section class="a-section reveal" id="performance">
                    <p class="a-label">"performance"</p>
                    <h2 class="heading">
                        <span class="hash">"## "</span>"fast where it counts"
                    </h2>
                    <p class="a-lede">
                        "measured with "<code>"hyperfine --shell=none"</code>" on linux x86-64."
                    </p>
                    <div class="a-why-grid">
                        <WhyItem title="dash-class">
                            "across builtins, variable expansion, conditionals, loops and "
                            "function calls, swagsh delivers dash-class performance."
                        </WhyItem>
                        <WhyItem title="~40% faster than bash">
                            "on those same workloads, swagsh is about 40% faster than bash."
                        </WhyItem>
                        <WhyItem title="~25% faster on pipelines">
                            "on pipelines, where process-spawn overhead dominates and the shell "
                            "layer matters less, it's about 25% faster than bash."
                        </WhyItem>
                    </div>
                </section>

                // ── install ───────────────────────────────────────────────
                <section class="a-section reveal" id="install">
                    <p class="a-label">"installation"</p>
                    <h2 class="heading"><span class="hash">"## "</span>"pick your platform"</h2>
                    <div class="cards">
                        <div class="card">
                            <p class="a-cmd-title">"cargo (all platforms)"</p>
                            <code class="a-code">"cargo install swagsh"</code>
                            <p class="a-comment">"# requires rust (edition 2024)"</p>
                        </div>
                        <div class="card">
                            <p class="a-cmd-title">"aur (arch linux)"</p>
                            <code class="a-code">"paru -S swagsh"</code>
                            <p class="a-comment">"# or yay"</p>
                        </div>
                        <div class="card">
                            <p class="a-cmd-title">"from source"</p>
                            <code class="a-code">"git clone https://github.com/takashialpha/swagsh"</code>
                            <code class="a-code">"cd swagsh && cargo build --release"</code>
                            <p class="a-comment">"# binary at target/release/swagsh"</p>
                        </div>
                    </div>
                    <p class="a-note">
                        "run "<code>"swagsh --help"</code>" for the full option set."
                    </p>
                </section>

                // ── footer ────────────────────────────────────────────────
                <footer class="foot reveal">
                    <span class="prompt">"$ "</span>
                    <span class="run">"echo "</span>
                    <span class="str">"\"stay stylish\""</span>
                    <span class="caret-static"></span>
                </footer>

            </div>
        </div>
    }
}
