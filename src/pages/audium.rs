//! `/audium`, landing page for the terminal music player.

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
pub fn Audium() -> impl IntoView {
    view! {
        <Title text="audium, terminal music player"/>
        <Meta
            name="description"
            content="A keyboard-driven music player that lives in your terminal. No Electron. No cloud. Just audio."
        />
        <div class="page audium">
            <div class="bg-grid" aria-hidden="true"></div>
            <div class="bg-glow" aria-hidden="true"></div>

            <div class="wrap">

                <BackNav/>

                // ── hero ──────────────────────────────────────────────────
                <header class="a-hero reveal">
                    <div class="term">
                        <TermBar title="audium, now playing"/>
                        <div class="term-body">
                            <p class="line">
                                <span class="usr">"you"</span>
                                <span class="at">"@"</span>
                                <span class="host">"terminal"</span>
                                <span class="path">" ~ "</span>
                                <span class="prompt">"$ "</span>
                                <span class="run">"audium"</span>
                            </p>
                            <h1 class="name">"audium"</h1>
                            <p class="tagline">
                                <span class="prompt-sm">"// "</span>
                                "music that lives in your terminal"
                                <span class="a-caret" aria-hidden="true"></span>
                            </p>
                            <p class="hero-sub">
                                "a keyboard-driven music player for people who live in the "
                                "terminal. no electron. no cloud sync. no background daemons. "
                                "no ffmpeg. your files, your library, your rules."
                            </p>
                            <div class="hero-links">
                                <a class="btn primary" href="#install">"get started"</a>
                                <a class="btn" href="https://github.com/takashialpha/audium"
                                    target="_blank" rel="noreferrer">"view source ↗"</a>
                            </div>
                        </div>
                    </div>

                    <div class="term a-demo">
                        <TermBar title="~/.local/share/audium"/>
                        <img
                            src="https://raw.githubusercontent.com/takashialpha/audium/main/audium-demo.gif"
                            alt="audium running in a terminal"
                            loading="lazy"
                        />
                    </div>
                </header>

                // ── install ───────────────────────────────────────────────
                <section class="a-section reveal" id="install">
                    <p class="a-label">"installation"</p>
                    <h2 class="heading"><span class="hash">"## "</span>"pick your platform"</h2>
                    <div class="cards">
                        <div class="card">
                            <p class="a-cmd-title">"cargo (all platforms)"</p>
                            <code class="a-code">"cargo install audium"</code>
                            <p class="a-comment">"# requires rust (edition 2024)"</p>
                        </div>
                        <div class="card">
                            <p class="a-cmd-title">"aur (arch linux)"</p>
                            <code class="a-code">"paru -S audium"</code>
                            <p class="a-comment">"# or yay, or manually with makepkg"</p>
                        </div>
                    </div>
                    <p class="a-note">
                        "audium targets linux exclusively and uses alsa, the standard linux "
                        "audio api, its development headers are needed to build, see the "
                        <a href="https://github.com/takashialpha/audium#building-from-source"
                            target="_blank" rel="noreferrer">"readme"</a>
                        " for distro-specific instructions."
                    </p>
                </section>

                // ── features ──────────────────────────────────────────────
                <section class="a-section reveal" id="features">
                    <p class="a-label">"features"</p>
                    <h2 class="heading">
                        <span class="hash">"## "</span>"everything you need. "<em>"nothing you don't."</em>
                    </h2>
                    <div class="cards">
                        <Feature icon="♪" title="format agnostic">
                            "plays MP3, FLAC, OGG, WAV, AAC, M4A, Opus, AIFF and more via "
                            "Symphonia, with no FFmpeg required."
                        </Feature>
                        <Feature icon="⌨" title="keyboard-driven">
                            "built to be driven entirely from the keyboard, for people who live "
                            "in the terminal and never reach for the mouse. press "
                            <code>"?"</code>" anytime to see every keybinding."
                        </Feature>
                        <Feature icon="🏷" title="library & metadata">
                            "import your files and audium reads artist, album, year and genre "
                            "from their tags automatically, all editable in-app."
                        </Feature>
                        <Feature icon="🎤" title="lyrics">
                            "store plain or synced lyrics per track, with an overlay that "
                            "follows along as the song plays."
                        </Feature>
                        <Feature icon="🎨" title="themes">
                            "15 built-in themes including nord, gruvbox, catppuccin, rosé pine, "
                            "dracula and tokyo night. switch live, with transparency support."
                        </Feature>
                        <Feature icon="🎵" title="playlists & queue">
                            "build playlists, shuffle them into the queue, and loop a single "
                            "track or the whole thing."
                        </Feature>
                        <Feature icon="⚡" title="playback control">
                            "filter your library as you type, adjust playback speed, and seek "
                            "freely."
                        </Feature>
                        <Feature icon="🔊" title="threaded audio">
                            "playback runs on its own thread, so the interface never stutters "
                            "your music."
                        </Feature>
                        <Feature icon="💾" title="it's your library">
                            "stored at "<code>"$XDG_DATA_HOME/audium/library.json"</code>", plain "
                            "JSON, human-readable, editable by hand. audium doesn't rename your "
                            "files, doesn't embed metadata, and never phones home."
                        </Feature>
                    </div>
                </section>

                // ── why ───────────────────────────────────────────────────
                <section class="a-section reveal" id="why">
                    <p class="a-label">"why audium"</p>
                    <h2 class="heading">
                        <span class="hash">"## "</span>"lighter. simpler. "<em>"actually yours."</em>
                    </h2>
                    <p class="a-lede">
                        "alternatives like termusic ship with heavy dependency trees, FFmpeg "
                        "requirements, daemon processes, or config formats that take longer to "
                        "learn than the app itself. audium takes a different approach."
                    </p>
                    <div class="a-why-grid">
                        <WhyItem title="no ffmpeg, no daemon">
                            "one binary. Zero background processes. Symphonia handles every "
                            "format natively in-process."
                        </WhyItem>
                        <WhyItem title="smaller and faster to build">
                            "fewer dependencies means shorter compile times and a ~3 MB release "
                            "binary with no runtime surprises."
                        </WhyItem>
                        <WhyItem title="cleaner ui">
                            "built on ratatui with a layout designed for daily use, not just "
                            "feature completeness. Panels, queue, progress bar, volume: "
                            "everything visible at once."
                        </WhyItem>
                        <WhyItem title="modern codebase">
                            "written in Rust edition 2024. No unsafe, no global state, no "
                            "legacy baggage."
                        </WhyItem>
                        <WhyItem title="plain data, no lock-in">
                            "your library is a JSON file you can read, edit, back up, or move "
                            "between machines freely. audium is a tool, not a platform."
                        </WhyItem>
                        <WhyItem title="system audio, done right">
                            "audium plays through your default system output. Switch devices at "
                            "the OS level and it follows, no in-app device picker that fights "
                            "your setup."
                        </WhyItem>
                    </div>
                </section>

                // ── footer ────────────────────────────────────────────────
                <footer class="foot reveal">
                    <span class="prompt">"$ "</span>
                    <span class="run">"echo "</span>
                    <span class="str">"\"made for the terminal\""</span>
                    <span class="caret-static"></span>
                </footer>

            </div>
        </div>
    }
}
