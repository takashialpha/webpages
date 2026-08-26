# my webpages

[![live at takashialpha.com](https://img.shields.io/badge/live-takashialpha.com-a6e3a1?style=flat-square&labelColor=11111b)](https://takashialpha.com)
[![Leptos](https://img.shields.io/badge/Leptos-ef3939?style=flat-square&labelColor=11111b)](https://github.com/leptos-rs/leptos)
[![Axum](https://img.shields.io/badge/Axum-cba6f7?style=flat-square&labelColor=11111b)](https://github.com/tokio-rs/axum)

My personal site, served live at **[takashialpha.com](https://takashialpha.com)**. The whole page is a tty: it opens on a login banner and everything past that is typed. A server-rendered Rust app built on [Leptos](https://github.com/leptos-rs/leptos) and [Axum](https://github.com/tokio-rs/axum), hydrated on the client.

There is one route. The server renders the banner, which is also the only thing search engines see, and the shell runs entirely in the browser from there. No command changes the URL.

## Content

Everything the site says lives in `content/` as plain text. `src/fs.rs` maps those files into the tree the shell walks, so adding a section is a new file and one line in `ROOT`, never a new command. `ls`, `cat`, `cd`, and tab completion all read that same tree.

The login banner does not keep its own copy of the intro. It renders the first paragraph of `content/about.txt`, which is also the page's meta description, so there is one place to edit it.

## Commands

`help` lists them, generated from the registry in `src/commands.rs` rather than written out, so it cannot fall behind. Adding a command is one entry in `COMMANDS` and one function.

## Prerequisites

1. WASM target: `rustup target add wasm32-unknown-unknown`
2. `cargo install cargo-leptos`

## Running locally

```sh
cargo leptos serve --release
```

The server listens on `[::1]:3000` (IPv6 loopback only) by default, configurable via `site-addr` in `Cargo.toml` or the `LEPTOS_SITE_ADDR` environment variable.

## Building for release

```sh
cargo leptos build --release
```

Produces the server binary in `target/release` and the site assets in `target/site`. Bundle filenames are content hashed, so `target/release/hash.txt` is written alongside the binary to map each one; the server reads it to build the `/pkg` URLs.

## Pre-built binary

Every push to `main` publishes a rolling [`build`](https://github.com/takashialpha/webpages/releases/tag/build) release carrying `webpages-x86_64-linux.tar.gz` and its `.sha256`. The archive holds the server binary next to `hash.txt` and a `site` directory, already laid out the way the environment variables below expect.

Extract it whole rather than copying pieces out of it. All three are one build and have to move together: `hash.txt` maps the content-hashed bundle names, and the server looks for it **beside the binary**, not under the site root. Leave it behind and the site answers 200 while every asset 404s, which no plain uptime check will catch. Set `LEPTOS_HASH_FILE_NAME` to an absolute path if your layout cannot keep the two adjacent.

There are no version tags; the latest build is always whatever is in `main`.

## Deploying

After `cargo leptos build --release`, copy the server binary, `target/release/hash.txt`, and the `target/site` directory to the target host, keeping `hash.txt` in the same directory as the binary. Then set:

```sh
export LEPTOS_OUTPUT_NAME="webpages"
export LEPTOS_SITE_ROOT="site"
export LEPTOS_SITE_PKG_DIR="pkg"
export LEPTOS_SITE_ADDR="[::1]:3000"
export LEPTOS_HASH_FILES="true"
```

and run the binary.

The server shuts down on SIGINT or SIGTERM. It stops accepting new connections and waits for the requests already in flight to finish before it exits, so a restart never cuts a page off mid render. It exits with a non-zero status if it fails to start, and says why. `GET /health` reports uptime and the commit the binary was built from.

## Logging

The server writes plain text logs to stdout. `RUST_LOG` sets the verbosity, defaulting to `warn,webpages=info` when it is unset: the startup and shutdown lines, plus any warnings and errors.

Request logging sits one level below that, so it is off unless asked for:

```sh
export RUST_LOG="webpages=debug"
```

Each request then gets a single line with its method, path, status, and how long it took.

The format follows where stdout goes. Color is used only when stdout is a terminal, and the timestamp is left out when systemd owns stdout, because journald records its own timestamp for every line.

At startup the server also reports the two ways a deploy can come up looking healthy while serving broken pages: `LEPTOS_SITE_ROOT` not pointing at a directory, and `hash.txt` missing from beside the binary. The second is an `ERROR` and names the exact path it looked at, because the page still renders and returns 200 either way.

## Time

`date` and `uptime` answer with the server's clock, not the browser's. The server stamps its UTC time and uptime onto the document, and the browser only measures how long the page has been open, so a visitor with a skewed system clock still sees the right time. UTC is formatted in `src/clock.rs` rather than pulled from a date library.

## Fonts

The page is set in the IBM VGA 8x16 ROM font, the face a text-mode console and a bios screen actually draw, taken from the [Ultimate Oldschool PC Font Pack](https://int10h.org/oldschool-pc-fonts/) and served as woff2 from `public/fonts/`, about 6 KB. It is licensed under [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/) by VileR; the license text ships alongside it in `public/fonts/vga-LICENSE.txt`.

The cell is 8 wide by 16 tall, so the font is only crisp at multiples of 16px and the type scale is pinned to them: 16px on a phone, 32px from 720px up. There is no fluid sizing anywhere in the stylesheet, and `line-height` is 1 because the cell already is the line. On a 1280x800 screen that works out to exactly 80 columns by 25 rows, which is classic VGA text mode.

There is no bold face, because the ROM font has none. A console fakes bold with a bright color and so does the stylesheet; asking for a weight the font does not have gets you a smeared synthetic bold instead.

The favicons and the social card are generated from this same font rather than drawn by hand, so they cannot drift from how the site looks.

## License

Licensed under the [GNU Affero General Public License v3.0 or later](LICENSE).
