//! `/audium`, landing page for the terminal music player.

use leptos::prelude::*;

use crate::components::{BackNav, Feature, Footer, JsonLd, PromptLine, Seo, TermBar, WhyItem};

/// schema.org `SoftwareApplication` entity for audium.
const APP_LD: &str = include_str!("../../schema/audium.json");

#[component]
pub fn Audium() -> impl IntoView {
    view! {
        <Seo
            title="audium"
            description="A terminal music app. Keyboard-driven, no Electron, no cloud, no daemons. Just your files."
            path="/audium"
        />
        <JsonLd json=APP_LD/>
        <div class="page audium">
            <div class="bg-grid" aria-hidden="true"></div>
            <div class="bg-glow" aria-hidden="true"></div>

            <div class="wrap">

                <BackNav/>

                // ── hero ──────────────────────────────────────────────────
                <header class="a-hero reveal">
                    <div class="term">
                        <TermBar title="audium"/>
                        <div class="term-body">
                            <PromptLine user="you" host="terminal" cmd="audium"/>
                            <h1 class="name">"audium"</h1>
                            <p class="tagline">
                                <span class="prompt-sm">"// "</span>
                                "a terminal music app"
                                <span class="a-caret" aria-hidden="true"></span>
                            </p>
                            <p class="hero-sub">
                                "your music, played from the keyboard, in the window you already "
                                "have open. no electron, no cloud, nothing running in the "
                                "background. just your files."
                            </p>
                            <div class="hero-links">
                                <a class="btn primary" href="#install">"get started"</a>
                                <a class="btn" href="https://github.com/takashialpha/audium"
                                    target="_blank" rel="noreferrer">"view source ↗"</a>
                            </div>
                        </div>
                    </div>

                    <div class="term a-demo">
                        <TermBar title="audium, demo"/>
                        // Served via jsDelivr (proper video/mp4 + range support);
                        // raw.githubusercontent sends octet-stream + nosniff, which
                        // browsers refuse to play. Pinned to the release tag so a
                        // new demo is a new URL rather than a stale CDN cache entry.
                        <video
                            src="https://cdn.jsdelivr.net/gh/takashialpha/audium@v2.0.0/audium-demo.mp4"
                            width="1280"
                            height="740"
                            autoplay
                            loop
                            muted
                            playsinline
                            preload="metadata"
                            aria-label="audium running in a terminal"
                        >
                            <a href="https://github.com/takashialpha/audium" target="_blank" rel="noreferrer">
                                "watch the audium demo"
                            </a>
                        </video>
                    </div>
                </header>

                // ── features ──────────────────────────────────────────────
                <section class="a-section reveal" id="features">
                    <p class="a-label">"features"</p>
                    <h2 class="heading">
                        <span class="hash">"## "</span>"everything you need. "<em>"nothing you don't."</em>
                    </h2>
                    <div class="cards">
                        <Feature icon="♪" title="plays everything">
                            "mp3, flac, ogg, wav, aac, m4a, aiff and more, straight out of the "
                            "box. no ffmpeg, no codec packs, nothing to configure."
                        </Feature>
                        <Feature icon="⌨" title="all keyboard">
                            "every action is a keystroke, for people who never reach for the "
                            "mouse. press "<code>"?"</code>" at any time for the full map."
                        </Feature>
                        <Feature icon="🎨" title="looks right anywhere">
                            "15 themes, nord, gruvbox, catppuccin, rosé pine, dracula, tokyo "
                            "night and friends, switched live. drop into a bare tty and it "
                            "adapts on its own."
                        </Feature>
                        <Feature icon="🎵" title="playlists & queue">
                            "your collection and your playlists sit side by side. queue either "
                            "one, shuffle it, loop a track or the lot."
                        </Feature>
                        <Feature icon="🎤" title="lyrics">
                            "plain or synced lyrics per track, with an overlay that follows "
                            "along line by line while the song plays."
                        </Feature>
                        <Feature icon="⏯" title="picks up where you left off">
                            "queue, track and position come back on the next launch, paused, so "
                            "nothing starts playing until you say so."
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
                        "most music players want to be a platform: an account, a sync service, a "
                        "config language, half a gigabyte of runtime. audium is a program that "
                        "plays your songs."
                    </p>
                    <div class="a-why-list">
                        <WhyItem title="one small binary">
                            "about 5 mb, no daemon, no background processes, nothing to keep "
                            "running between sessions. install it and it works."
                        </WhyItem>
                        <WhyItem title="your files stay yours">
                            "titles, artists and lyrics live in the tags of your own files, "
                            "edits included, so they travel with the music and any other player "
                            "can read them. audium never phones home."
                        </WhyItem>
                        <WhyItem title="nothing to learn">
                            "no config file to write before you can hear a song. import, press "
                            "play, and change what you want from inside the app."
                        </WhyItem>
                        <WhyItem title="never in your way">
                            "audio runs on its own thread, so the interface never stutters your "
                            "music, and playback follows whatever output your system is using."
                        </WhyItem>
                    </div>
                </section>

                // ── install ───────────────────────────────────────────────
                <section class="a-section reveal" id="install">
                    <p class="a-label">"installation"</p>
                    <h2 class="heading"><span class="hash">"## "</span>"pick your platform"</h2>
                    <div class="cards">
                        <div class="card">
                            <p class="a-cmd-title">"cargo"</p>
                            <code class="a-code">"cargo install audium --locked"</code>
                            <p class="a-comment">"# needs the latest stable rust"</p>
                        </div>
                        <div class="card">
                            <p class="a-cmd-title">"aur (arch linux)"</p>
                            <code class="a-code">"paru -S audium"</code>
                            <p class="a-comment">"# or yay, or manually with makepkg"</p>
                        </div>
                    </div>
                    <p class="a-note">
                        "linux only, through alsa. building needs its development headers, see "
                        "the "
                        <a href="https://github.com/takashialpha/audium#building-from-source"
                            target="_blank" rel="noreferrer">"readme"</a>
                        " for the package name on your distro."
                    </p>
                </section>

                // ── footer ────────────────────────────────────────────────
                <Footer msg="made for the terminal"/>

            </div>
        </div>
    }
}
