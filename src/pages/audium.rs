//! `/audium` — landing page for the terminal music player.

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
        <Title text="audium — terminal music player"/>
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
                        <TermBar title="audium — now playing"/>
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
                        <img src="/audium-demo.gif" alt="audium running in a terminal" loading="lazy"/>
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
                        "audio api — its development headers are needed to build, see the "
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
                            "Symphonia — no FFmpeg required."
                        </Feature>
                        <Feature icon="⌨" title="fully keyboard-driven">
                            "every action is a single keypress. Tab between panels, "
                            <code>"j"</code>"/"<code>"k"</code>" to navigate, space to play."
                        </Feature>
                        <Feature icon="📂" title="built-in file picker">
                            "browse your filesystem and import audio without ever leaving the app."
                        </Feature>
                        <Feature icon="🏷" title="track metadata">
                            "artist, album, year and genre are read automatically from your "
                            "files' tags on import and shown throughout the UI. Edit any field "
                            "in-app with "<code>"e"</code>", or open the built-in lyrics editor "
                            "from the same screen."
                        </Feature>
                        <Feature icon="🎨" title="themes">
                            "15 built-in themes — dark, light, nord, gruvbox, catppuccin, "
                            "rosé pine, dracula, tokyo night, and more. Switch live with "
                            "instant preview. Transparency support for composited terminals."
                        </Feature>
                        <Feature icon="🎵" title="playlists & loop modes">
                            "create, rename, and delete playlists. Loop a single track or the "
                            "whole queue — toggle with "<code>"l"</code>"."
                        </Feature>
                        <Feature icon="🔀" title="shuffle">
                            "shuffle any playlist into the queue with a single keypress. Keeps "
                            "your library intact while letting you rediscover it."
                        </Feature>
                        <Feature icon="⚡" title="playback speed">
                            "adjust speed from 0.05× to 3× in 0.05× steps with "<code>"["</code>
                            " and "<code>"]"</code>". Displayed in the player bar when not at 1×."
                        </Feature>
                        <Feature icon="🔍" title="tracklist filter">
                            "press "<code>"/"</code>" to filter by title, artist, album, year or "
                            "genre in real time. Esc clears. Works across all playlists."
                        </Feature>
                        <Feature icon="🎤" title="lyrics">
                            "store plain or LRC synced lyrics per track. Toggle an overlay with "
                            <code>"y"</code>" — synced lyrics auto-scroll to the current line; "
                            "plain lyrics scroll with "<code>"j/k"</code>". Edit directly with "
                            "the built-in text editor."
                        </Feature>
                        <Feature icon="🔊" title="threaded audio">
                            "playback runs on a dedicated thread. UI interactions will never "
                            "stutter your music."
                        </Feature>
                        <Feature icon="💾" title="it's your library">
                            "stored at "<code>"$XDG_DATA_HOME/audium/library.json"</code>" — plain "
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
                            "built on ratatui with a layout designed for daily use — not just "
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
                            "the OS level and it follows — no in-app device picker that fights "
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
