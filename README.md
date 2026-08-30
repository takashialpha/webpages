# my webpages

[![live at takashialpha.com](https://img.shields.io/badge/live-takashialpha.com-a6e3a1?style=flat-square&labelColor=11111b)](https://takashialpha.com)
[![Leptos](https://img.shields.io/badge/Leptos-ef3939?style=flat-square&labelColor=11111b)](https://github.com/leptos-rs/leptos)
[![Axum](https://img.shields.io/badge/Axum-cba6f7?style=flat-square&labelColor=11111b)](https://github.com/tokio-rs/axum)

My personal site, served live at **[takashialpha.com](https://takashialpha.com)**. The whole page is a tty: it opens on a login banner and everything past that is typed. A server-rendered Rust app built on [Leptos](https://github.com/leptos-rs/leptos) and [Axum](https://github.com/tokio-rs/axum), hydrated on the client.

There is one route. The server renders the banner, which is also the only thing search engines see, and the shell runs entirely in the browser from there. No command changes the URL.

## Content

Everything the site says lives in `content/` as plain text, laid out the way the shell sees it. `src/fs.rs` maps those files into the tree it walks, so adding a section is a new file and one line in `ROOT`, never a new command. `ls`, `cat`, `cd`, and tab completion all read that same tree.

It is a home directory, arranged like one:

```
~
├── bin/         programs
├── documents/   the writing
├── projects/    one line and a URL each
└── var/
    └── wall.txt the shared board
```

`var` because that is what `var` has always meant: the one thing here that changes while you are looking at it, kept apart from the things that do not. `bin` for the same reason a home directory has always had one.

There is no root. This tree is a home directory and nothing else, so a path starting with `/` does not resolve: it is naming something above a place that has no above, and saying so is more honest than treating `/` and `~` as the same. Paths are relative, or start at `~`.

One entry is not in the binary. `~/var/wall.txt` is a `Node::Live`, a file the server holds, so reading it is a request rather than a read. `cat` resolves its operands into a plan first and only answers late when the plan contains one, which keeps the order of `cat documents/now.txt var/wall.txt` right and leaves a purely static `cat` immediate. One request covers however many times the file is named.

The login banner does not keep its own copy of the intro. It renders the first paragraph of `content/documents/about.txt`, which is also the page's meta description, so there is one place to edit it.

## Commands

`help` lists them, generated from the registry in `src/commands.rs` rather than written out, so it cannot fall behind. Adding a command is one entry in `COMMANDS` and one function.

The line editor answers to the readline bindings a shell does: `ctrl-c` abandons the line and echoes `^C`, `ctrl-l` clears the screen and keeps what is half typed, `ctrl-d` closes the session on an empty line, and `ctrl-a`, `ctrl-e`, `ctrl-u` and `ctrl-k` move to the ends of the line and cut to them. `ctrl-c` gives way to the browser when there is a selection, because copying is what it means then. `ctrl-w` and `ctrl-n` are deliberately absent: browsers reserve them for closing the tab and opening a window and will not hand them over, so binding them would delete a word and close the tab.

A command can also answer later. `Output::Pending` carries a task that the terminal spawns, handing it a `Sink` to write into once it has something, so a command that has to ask the server does not make the prompt wait. `wall` is the one that does.

Each entry carries a `Spec` saying which options the command takes and how many operands, and `src/args.rs` checks the line against it before the function runs. No command parses its own arguments, so none of them can quietly ignore what they were given: an unknown option, a missing operand, and one operand too many all stop in the same place and report the same way. Options may be clustered, `--` ends them, and a lone `-` is an operand, which is what lets `cd -` name a directory.

The same spec is what `help <command>` prints and what tab completion offers after a `-`, so usage text, completion, and what actually runs are one thing. Every command takes `-h`.

It also says what a command's operands can be, so Tab offers the right things: `cd` is only offered directories, and a command that takes no operands is offered nothing.

## Layout

A workspace: the site at the root, and the guest programs under `bin/` as members. They share `[workspace.lints]`, so a program is held to the same conventions the site is.

One lint cannot be shared. `unsafe_code` is only *denied* by the workspace, because a guest has to declare and call its wasm imports and both are unsafe. The site adds `#![forbid(unsafe_code)]` at its crate roots, which is stricter and cannot be undone; each guest confines its unsafe to one `tty` module, with a reason on the `expect` and a safety comment on every call.

`default-members` is the site alone, so an ordinary `cargo build` or `cargo clippy` never tries to link a guest for the host. Building one is asked for explicitly, which is what the `justfile` does. Adding a program is creating its directory under `bin/`: the workspace globs that, and so does the recipe.

## Prerequisites

1. WASM targets: `rustup target add wasm32-unknown-unknown wasm32-wasip1`
2. `cargo install cargo-leptos just`

## Running locally

```sh
just serve
```

The server listens on `[::1]:3000` (IPv6 loopback only) by default, configurable via `site-addr` in `Cargo.toml` or the `LEPTOS_SITE_ADDR` environment variable.

## Building for release

```sh
just build
```

This is what `just build` does, and it is two builds because the guests are for a different target: `just guests` puts them in `public/`, then cargo-leptos copies that into the site. It produces the server binary in `target/release` and the site assets in `target/site`. Bundle filenames are content hashed, so `target/release/hash.txt` is written alongside the binary to map each one; the server reads it to build the `/pkg` URLs.

## Pre-built binary

Every push to `main` publishes a rolling [`build`](https://github.com/takashialpha/webpages/releases/tag/build) release carrying `webpages-x86_64-linux.tar.gz` and its `.sha256`. The archive holds the server binary next to `hash.txt` and a `site` directory, already laid out the way the environment variables below expect.

Extract it whole rather than copying pieces out of it. All three are one build and have to move together: `hash.txt` maps the content-hashed bundle names, and the server looks for it **beside the binary**, not under the site root. Leave it behind and the process still starts, then panics on the first render and serves nothing, so a proxy in front answers 502. Set `LEPTOS_HASH_FILE_NAME` to an absolute path if your layout cannot keep the two adjacent.

There are no version tags; the latest build is always whatever is in `main`.

## Deploying

Build with `just build` rather than `cargo leptos build` alone, or `bin/` ships empty and every program in it 404s when it is run. Then copy the server binary, `target/release/hash.txt`, and the `target/site` directory to the target host, keeping `hash.txt` in the same directory as the binary. Then set:

```sh
export LEPTOS_OUTPUT_NAME="webpages"
export LEPTOS_SITE_ROOT="site"
export LEPTOS_SITE_PKG_DIR="pkg"
export LEPTOS_SITE_ADDR="[::1]:3000"
export LEPTOS_HASH_FILES="true"
export WALL_PATH="/var/lib/webpages/wall.txt"
```

and run the binary.

`WALL_PATH` has to point outside the release directory. A deploy replaces that directory wholesale, so a board kept inside it is destroyed on the next one. Under systemd, `StateDirectory=webpages` creates and owns `/var/lib/webpages`. Leave the variable unset and the board falls back to `wall.txt` in the working directory, which is fine for a local run and wrong for a deploy.

The server shuts down on SIGINT or SIGTERM. It stops accepting new connections and waits for the requests already in flight to finish before it exits, so a restart never cuts a page off mid render. It exits with a non-zero status if it fails to start, and says why. `GET /health` reports uptime and the commit the binary was built from.

## Logging

The server writes plain text logs to stdout. `RUST_LOG` sets the verbosity, defaulting to `warn,webpages=info` when it is unset: the startup and shutdown lines, plus any warnings and errors.

Request logging sits one level below that, so it is off unless asked for:

```sh
export RUST_LOG="webpages=debug"
```

Each request then gets a single line with its method, path, status, and how long it took.

The format follows where stdout goes. Color is used only when stdout is a terminal, and the timestamp is left out when systemd owns stdout, because journald records its own timestamp for every line.

At startup the server also reports the two ways a deploy can start cleanly and still be broken: `LEPTOS_SITE_ROOT` not pointing at a directory, which serves pages with no CSS or WASM, and `hash.txt` missing from beside the binary, which panics on every render and serves nothing at all. Both are checked before the listener opens, and the second is an `ERROR` naming the exact path it looked at, since the process itself gives no other clue.

## Time

`date` and `uptime` answer with the server's clock, not the browser's. The server stamps its UTC time and uptime onto the document, and the browser only measures how long the page has been open, so a visitor with a skewed system clock still sees the right time. UTC is formatted in `src/clock.rs` rather than pulled from a date library.

## Programs

A program takes the whole terminal and draws into a grid of cells, then gives it back exactly as it was. That is the alternate screen a terminal has always had: the scrollback is the normal buffer and is left alone underneath, which is why leaving `vim` does not eat your shell history.

Programs are files. They live in `~/bin` as `Node::Program`, so `ls` lists them, `help` names them, and completion offers them for the same reason it offers anything else, and the tree is the only place that says what exists.

Only a path runs one. `./bin/life` and `bin/life` both work and a bare `life` does not, because there is no `PATH` here and inventing one would mean a name resolving to something the tree does not say is there. A path that lands on something which is not a program still reads as `command not found`, with a line under it saying the file is there but is not one, so the two cases are told apart without being reported differently.

The first word is completed as both, for the same reason: a command name, and a path, since typing a path is how a program is reached.

A program is given the same treatment as a command: it carries a `Spec`, so `life --help` prints usage and `life nonsense` is refused rather than quietly ignored. `About` in `src/args.rs` is what the two share.

A program that only prints runs to completion the moment it is opened: it finishes in microseconds, so there is nothing to yield to, and what it printed belongs in the scrollback rather than on a screen of its own. One that draws is different. The terminal owns the frame loop and calls it once per frame, rather than the program running its own loop. That is not a style choice. WebAssembly cannot suspend a synchronous call, so a guest running its own loop would block the page until it finished; handing it one frame at a time is what lets it be interactive at all. `src/program.rs` is that interface and `src/screen.rs` is the grid, which cuts each row into runs of matching colour and repaints only the rows that changed.

Colours are indices into the sixteen a console has, resolved by the stylesheet, so a program follows whatever `theme` is set to without knowing themes exist. The grid is measured in whole cells from the terminal's own size, so it fits the window rather than assuming 80 by 24.

While a program runs the scrollback is moved out of sight rather than hidden outright, because the input inside it is the terminal's keyboard and an element that is `display: none` cannot hold focus. `q` and `ctrl-c` both leave.

### Guests

A program can also be a `wasm32-wasip1` binary, fetched when it is run so none of it is in the bundle until someone asks. The browser has a WebAssembly engine, so nothing here interprets anything: the guest is handed to that engine with an import object standing in for the operating system it thinks it has.

`src/wasi.rs` is that import object. Only what a terminal program reaches for is implemented, which turns out to be seven calls: `fd_write`, `fd_read`, `environ_get`, `environ_sizes_get`, `clock_time_get`, `random_get` and `proc_exit`. Nothing else is stubbed, because an import a guest needs and this does not provide fails at instantiation naming the exact function, which is where a missing piece should be noticed rather than at the moment it is called.

A second module, `tty`, carries what WASI has no concept of, since preview1 cannot address a screen or read a key:

```
tty::cols() -> u32          tty::rows() -> u32
tty::put(x, y, ch, fg, bg)  tty::clear()
tty::key() -> i32           // -1 when nothing is waiting
```

Keys arrive as the bytes a terminal would really have sent, escape sequences included, so a guest parsing `\x1b[A` is parsing the same thing it would anywhere else.

`proc_exit` records the code and returns. A guest treats that call as never returning and runs straight into its own unreachable, which traps, and the trap is what ends the call; having recorded the code first is what tells a clean exit from a crash.

Guests live in `bin/` as separate crates, workspace members held to the same lints as the site. `just guests` builds them into `public/bin/`, which is what cargo-leptos copies into the site; they are build output, not source.

### The two there are

`life` and `stars` both draw. Neither is compiled into the site: there is no second, native way to be a program, because a second way is a way to drift. It draws two cells to a character using the half blocks the ROM font carries, which is both twice the resolution and square cells, since a character is twice as tall as it is wide. Its generation counter is abbreviated past a thousand so a run left going overnight cannot push the status bar off the screen, and the bar drops its least useful hint rather than overflowing when the screen is too narrow to hold them all.

## The wall

The board is a file, `~/var/wall.txt`, so `ls` lists it and `cat` prints it raw, the way `cat` prints anything else. The frame and the axes belong to `wall`, which is the thing that draws it: `wall` prints the shared 80 by 24 board with its coordinates, and `wall <x> <y> <char>` sets one cell of it to any printable character. `wall <x> <y>` with nothing after it clears the cell, which is the only way to reach a blank through a line that was split on whitespace. It is the only thing on the site anyone but me can change.

It is a wall, not a document, so it is addressed like one: `0,0` is the bottom left and `y` counts upwards, the first quadrant of a graph. It sits in a frame closed on all four sides, drawn in the box-drawing characters the VGA ROM font actually carries rather than in `+` and `-`, with the rows counted up the left and the columns along the bottom in tens and units. Eighty by twenty-four is what a terminal has always been. It does not fit a phone, and is not meant to: the board is printed into a box that scrolls sideways, because a grid that wraps is not a grid.

The frame is 85 columns wide, being the board, a border either side, and the row numbers, and that is where the stylesheet's line measure comes from. Everything the command prints is kept inside it, the hint underneath included, since a longer line would make the whole board scroll sideways to read it.

Storage runs the other way, top row first, so the file reads in the same order as the board is drawn and editing it by hand needs no arithmetic. `Grid::set` is the one place the two orders meet.

It is bounded on purpose. There is no free-form text: a write is one printable character at one coordinate. Control characters, multi-byte characters, and anything longer than a single character are refused rather than truncated. The worst anyone can do is spell something across the grid that the next visitor writes over.

Writing is charged by the cell: eighty a minute per address, and six hundred a minute across everyone. The per-address budget is the fair share and the ceiling is what makes it enforceable, because the address comes from `CF-Connecting-IP`, which Cloudflare sets and strips from whatever the client sent. That header is only trustworthy behind the proxy: anything reaching the origin directly could claim a fresh address per request and never meet a per-address limit, which is what the ceiling is for. At most four thousand addresses are remembered at once and the oldest is dropped past that, so choosing the key cannot also mean choosing how much is remembered.

Board text is never linkified and never becomes markup. It is rendered as text, so what somebody writes stays what they wrote.

The board is stored as twenty-four lines of eighty characters, so moderating it is opening the file in an editor. Reading it back is forgiving about length: a short line is padded and a long one is cut, so hand-editing cannot put the board into a shape the rest of the code does not expect.

`src/wall.rs` holds the grid, the validation, and the limiter; `src/main.rs` mounts `GET` and `POST /api/wall`.

## The screen

The terminal is sized to the visual viewport, not the layout viewport, and `src/viewport.rs` is what measures it.

Those two are the same thing until a phone raises its keyboard, which shrinks only the visual one. Chrome can be told to keep them together with `interactive-widget=resizes-content` in the viewport meta, which is set, but Safari ignores it, so on iOS the layout viewport carries on behind the keyboard. Anything sized to `100dvh` is then taller than the screen, and anything `position: fixed` to the bottom is pinned somewhere you cannot see.

So the height and the offset of the visual viewport are written onto the document as `--screen-height` and `--screen-top`, and the stylesheet builds the terminal from those. Three things follow from that. The terminal is fixed over the visible area and moves with it, so the page behind it never scrolls, which is the scrolling that drags a fixed element out of place on iOS. `.screen` is the only scroller, sized to what the key row leaves, so the prompt scrolls to a bottom that really is the bottom. And the key row is an ordinary flex item under it rather than an overlay, so nothing has to reserve space for it and it cannot be scrolled away or end up beneath the keyboard.

The extra-keys row itself is tab, up, and down, which a phone keyboard has none of. It renders only on touch devices.

## Fonts

The page is set in the IBM VGA 8x16 ROM font, the face a text-mode console and a bios screen actually draw, taken from the [Ultimate Oldschool PC Font Pack](https://int10h.org/oldschool-pc-fonts/) and served as woff2 from `public/fonts/`, about 6 KB. It is licensed under [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/) by VileR; the license text ships alongside it in `public/fonts/vga-LICENSE.txt`.

The cell is 8 wide by 16 tall, so the font is only crisp at multiples of 16px and the type scale is pinned to them: 16px on a phone, 32px from 720px up.

The typed text is drawn by a span over the input rather than by the input itself. An input renders text its own way, which is not quite how a span renders the same characters, so a line changed appearance the moment it was echoed into the scrollback as one. Both are spans now and cannot differ. The input still holds the text and takes the keys; it is transparent rather than hidden, so the selection it draws still shows, behind those glyphs. The one thing this gives up is that an input method's in-progress text is drawn by the input and would not show, which for a terminal typing ascii commands is a trade worth making.

Drawing the text is also what lets a line longer than the field scroll in whole columns, the way a terminal scrolls and an input does not: left to itself the input stops half way through a character. The first visible column is tracked here instead, moving only when the caret would otherwise leave the field, and the block cursor is placed against the same offset.

The prompt is built by one function wherever it appears, live or echoed into the scrollback. It is split into separate elements so that no single text node holds anything shaped like an email address, which Cloudflare rewrites, and that split has to match in both: each inline box is shaped and rounded on its own, so the same characters in one box and in four land a sixteenth of a pixel apart and the line visibly shifts the moment you press Enter.

Everything measured in cells uses a `--cell` custom property, set beside the type size, rather than the `ch` unit. `ch` comes from a metric the woff2 conversion rounded and reports 15.9998px where every glyph actually advances 16, which is enough to put a line of text on fractional pixels. Text on a fractional pixel rasterises differently from the same text on a whole one, so the input and the line it becomes when you press Enter did not quite look alike. Line measure, caret position, and the flex gap in the prompt are all whole cells now.

The padding is added onto the line measure rather than included in it, because `box-sizing: border-box` otherwise takes the padding out of the character count and slices the last column down the middle. There is no fluid sizing anywhere in the stylesheet, and `line-height` is 1 because the cell already is the line. On a 1280x800 screen that works out to exactly 80 columns by 25 rows, which is classic VGA text mode.

There is no bold face, because the ROM font has none. A console fakes bold with a bright color and so does the stylesheet; asking for a weight the font does not have gets you a smeared synthetic bold instead.

The favicons and the social card are generated from this same font rather than drawn by hand, so they cannot drift from how the site looks.

## License

Licensed under the [GNU Affero General Public License v3.0 or later](LICENSE).
