# my webpages

[![live at takashialpha.com](https://img.shields.io/badge/live-takashialpha.com-a6e3a1?style=flat-square&labelColor=11111b)](https://takashialpha.com)
[![Leptos](https://img.shields.io/badge/Leptos-ef3939?style=flat-square&labelColor=11111b)](https://github.com/leptos-rs/leptos)
[![Axum](https://img.shields.io/badge/Axum-cba6f7?style=flat-square&labelColor=11111b)](https://github.com/tokio-rs/axum)

My personal site, live at **[takashialpha.com](https://takashialpha.com)**. The whole page is a tty: it opens on a login banner and everything after that is typed. Rust on [Leptos](https://github.com/leptos-rs/leptos) and [Axum](https://github.com/tokio-rs/axum), rendered on the server and hydrated in the browser.

There is one page. The server renders the banner, which is also all a search engine sees, and the shell runs in the browser from there. No command changes the URL.

Why any of it works the way it does is in the code, next to the thing it explains. This file is what the site does and how to run it.

## What it does

**A shell.** `help` lists the commands, generated from the registry rather than written out, so it cannot fall behind. Each command declares the options and operands it takes, and that one declaration is what gets enforced, what `help <command>` prints, and what Tab offers. Every command takes `-h`.

The line editor answers the readline bindings you would expect: `ctrl-c` abandons the line, `ctrl-l` clears the screen and keeps what you were typing, `ctrl-d` closes the session on an empty line, `ctrl-a`, `ctrl-e`, `ctrl-u` and `ctrl-k` move to the ends and cut to them. `ctrl-w` and `ctrl-n` are missing because browsers keep those for themselves. Tab completes command names, paths and options, and offers only what would actually work: `cd` is given directories, a command that takes no operands is given nothing. On a touch device a row of tab, up and down keys sits under the screen, since a phone keyboard has none of them.

`date` and `uptime` answer with the server's clock rather than the browser's, so a skewed laptop still shows the right time.

**A home directory.** Everything the site says lives in `content/` as plain text.

```
~
├── bin/         programs
├── documents/   the writing
├── projects/    one line and a URL each
└── var/
    └── wall.txt the shared board
```

`src/fs.rs` maps those files into the tree that `ls`, `cat`, `cd` and completion all walk, so adding a section is a new file and one line in `ROOT`, never a new command. There is no root directory: paths are relative, or start at `~`.

**Programs.** `~/bin` holds `life`, `snake`, `tetris` and `stars`. Run one by its path, `bin/tetris` or `./bin/tetris`; a bare `tetris` will not do, because there is no `PATH` to find it on. A program takes the whole terminal, draws into a grid of cells, and hands the scrollback back untouched when it exits. `q` or `ctrl-c` leaves, and the terminal answers both itself, so nothing that draws can trap you. Every key a program answers is named in its own status bar.

Typing something that is not a command gets a shell's answer rather than one catch-all message: `Is a directory`, `Is a file`, `No such file or directory` for a path that is not there, `command not found` for a bare word, each with the command that would have worked under it.

**A wall.** `~/var/wall.txt` is the one thing here that anyone else can write to. `wall` prints the board with its coordinates, `wall <x> <y> <char>` sets a cell, and `wall <x> <y>` with nothing after it clears one. `0,0` is the bottom left and `y` counts up, like the first quadrant of a graph. A write is one printable character at a time, charged against a per address budget of eighty a minute and a shared one of six hundred, and board text never becomes markup. It is stored as twenty four lines of eighty characters, so moderating it is editing a file; the server notices the edit and reads it back without a restart.

**Themes.** `theme` lists the palettes, `theme <name>` switches. Programs draw in the sixteen colours a console has and the stylesheet resolves them, so they follow the palette without knowing palettes exist.

**A 404 in character.** Every path but `/` answers 404 with this same page and a line naming what was asked for. The URL is then put right with a `replaceState`, so a reload or a bookmark asks for the page that is actually there.

## Running it

The wasm targets and the tools:

```sh
rustup target add wasm32-unknown-unknown wasm32-wasip1
cargo install cargo-leptos cargo-shear just
```

Then:

```sh
just serve   # build the programs, then serve on [::1]:3000
just build   # release build into target/release and target/site
just check   # what CI runs: fmt, unused deps, clippy for ssr, hydrate and the programs
```

`just --list` has the rest. The address comes from `site-addr` in `Cargo.toml`, or `LEPTOS_SITE_ADDR`.

One site takes two builds, because the programs compile for a different target. `just guests` builds them, hashes their filenames and writes `public/bin/hash.txt`, and cargo-leptos copies that into the site. Build with `just build` rather than `cargo leptos build` on its own, or `bin/` ships empty and every program 404s when it is run.

## Deploying

Every push to `main` publishes a rolling [`build`](https://github.com/takashialpha/webpages/releases/tag/build) release with `webpages-x86_64-linux.tar.gz` and its `.sha256`. There are no version tags; the latest build is whatever is in `main`.

Extract the archive whole. The binary, `hash.txt` and the `site` directory are one build and move together, and `hash.txt` has to stay **beside the binary**, not under the site root. Leave it behind and the process starts, then panics on the first render and serves nothing, which a proxy reports as a 502. `LEPTOS_HASH_FILE_NAME` can point at it if your layout cannot keep the two adjacent.

Install the deploy script and the unit by fetching them, never by pasting into an editor. A paste that loses line breaks still looks right and does not parse.

```sh
sudo curl -fsSL -o /usr/local/bin/deploy-webpages \
  https://raw.githubusercontent.com/takashialpha/webpages/main/deploy/deploy-webpages
sudo chmod +x /usr/local/bin/deploy-webpages
sudo curl -fsSL -o /etc/systemd/system/webpages.service \
  https://raw.githubusercontent.com/takashialpha/webpages/main/deploy/webpages.service
sudo systemctl daemon-reload
```

`deploy-webpages` then fetches the latest build, swaps it in, and rolls back if the new release does not serve its own assets. The proxy config is in `deploy/` too.

To do it by hand, copy the binary, `target/release/hash.txt` and `target/site` to the host, keep `hash.txt` next to the binary, and set:

```sh
export LEPTOS_OUTPUT_NAME="webpages"
export LEPTOS_SITE_ROOT="site"
export LEPTOS_SITE_PKG_DIR="pkg"
export LEPTOS_SITE_ADDR="[::1]:3000"
export LEPTOS_HASH_FILES="true"
export WALL_PATH="/home/takashi/webpages/wall.txt"
```

`WALL_PATH` has to point outside the release directory, since a deploy replaces that directory wholesale and would take the board with it. Unset, it falls back to `wall.txt` in the working directory, which is fine locally and wrong for a deploy. The board lives in a home directory so it can be moderated with an editor, which needs this once:

```sh
sudo -u takashi mkdir -p /home/takashi/webpages
sudo -u takashi touch /home/takashi/webpages/wall.txt
sudo chmod 2775 /home/takashi/webpages          # setgid: new files keep the group
sudo chmod 664  /home/takashi/webpages/wall.txt # the service writes it too
sudo chmod g+x  /home/takashi                   # so the service can traverse in
```

That last one is the easy one to miss. A permission check stops at the first class that matches, so a service in group `takashi` gets the group bits and never falls through to the ones set for everyone else. The unit wants `SupplementaryGroups=takashi`, plus `ProtectHome=read-only` with `ReadWritePaths=/home/takashi/webpages`. If the service cannot write the board it says so at startup, as an `ERROR` naming the path.

The server shuts down on SIGINT or SIGTERM, waiting for the requests in flight, and exits non zero if it fails to start. `GET /health` reports uptime and the commit it was built from.

## Logging

Plain text to stdout, coloured only when stdout is a terminal, and with no timestamp under journald, which stamps its own. `RUST_LOG` defaults to `warn,webpages=info`, which is startup, shutdown, warnings and errors.

```sh
export RUST_LOG="webpages=debug"
```

adds a line per request with its method, path, status and duration.

## Layout

A workspace: the site at the root, the programs under `bin/`, one crate each, and what they share under `lib/`.

```
.
├── bin/         the programs
├── lib/guest/   what they need from the terminal
├── content/     everything the site says
├── deploy/      unit, deploy script, proxy config
└── src/         the site
```

Every member takes `[workspace.lints]`, so the programs are held to the same rules as the site. `unsafe_code` is denied rather than forbidden there, because a guest has to declare and call its wasm imports; the site forbids it outright at its crate roots. `default-members` is the site alone, so a plain `cargo build` never tries to link a program for the host.

Inside `src/`, a module with enough going on has the rest beside it: `server.rs` has the routes, logging and shutdown, `terminal.rs` the line editor, the alternate screen and the rendering, `wall.rs` the grid and both sides of the board, `wasi.rs` the host state and the import object, `commands.rs` the registry with the functions grouped by what they touch.

## Writing a program

A program is a `wasm32-wasip1` binary, fetched when it is run, so none of it is in the bundle until someone asks for it. The browser's own engine runs it against an import object standing in for an operating system: seven WASI calls, which is all a terminal program reaches for, and a `tty` module for what WASI has no concept of.

```
tty::cols() -> u32          tty::rows() -> u32
tty::put(x, y, ch, fg, bg)  tty::clear()
tty::key() -> i32           // -1 when nothing is waiting
```

Export `frame(elapsed: f32) -> i32` to draw one frame at a time, returning non zero to quit. The terminal owns the loop and calls in, because WebAssembly cannot suspend a synchronous call and a program running its own loop would hold the page still. Export only `_start` and it runs to the end when opened, with whatever it printed going into the scrollback.

Keys arrive as the bytes a terminal would send, escape sequences included, so an arrow is `esc [ A`. `lib/guest` wraps all of this: the imports, a keyboard that puts those sequences back together, and the chrome the programs draw the same way.

To add one, make a directory under `bin/`; the workspace and `just guests` both glob it. The filenames it builds carry a content hash, so a changed program gets a new URL and can be cached forever; the server reads `hash.txt` at startup and stamps the names onto the document for the browser to read back.

## Fonts

The page is set in the IBM VGA 8x16 ROM font, what a text mode console actually draws. It comes from the [Ultimate Oldschool PC Font Pack](https://int10h.org/oldschool-pc-fonts/), about 6 KB of woff2 in `public/fonts/`, licensed [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/) by VileR with the license text beside it.

The cell is 8 wide by 16 tall, so the font is only crisp at multiples of 16px and the type scale is pinned to them: 16px on a phone, 32px from 720px up. On a 1280x800 screen that comes out at exactly 80 columns by 25 rows. There is no bold face, because the ROM font has none; a console fakes bold with a bright colour and so does the stylesheet. The favicons and the social card are generated from the same font, so they cannot drift from the page.

## License

Licensed under the [GNU Affero General Public License v3.0 or later](LICENSE).
