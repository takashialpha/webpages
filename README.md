# my webpages

[![live at takashialpha.com](https://img.shields.io/badge/live-takashialpha.com-a6e3a1?style=flat-square&labelColor=11111b)](https://takashialpha.com)
[![Leptos](https://img.shields.io/badge/Leptos-ef3939?style=flat-square&labelColor=11111b)](https://github.com/leptos-rs/leptos)
[![Axum](https://img.shields.io/badge/Axum-cba6f7?style=flat-square&labelColor=11111b)](https://github.com/tokio-rs/axum)

My personal site, served live at **[takashialpha.com](https://takashialpha.com)**. The whole page is a tty: it opens on a login banner and everything past that is typed. A server-rendered Rust app on [Leptos](https://github.com/leptos-rs/leptos) and [Axum](https://github.com/tokio-rs/axum), hydrated on the client.

There is one route. The server renders the banner, which is also all a search engine sees, and the shell runs in the browser from there. No command changes the URL.

## Content

Everything the site says is in `content/` as plain text, laid out the way the shell sees it. `src/fs.rs` maps those files into the tree it walks, so adding a section is a new file and one line in `ROOT`, never a new command. `ls`, `cat`, `cd` and tab completion all read that same tree.

It is a home directory, arranged like one:

```
~
├── bin/         programs
├── documents/   the writing
├── projects/    one line and a URL each
└── var/
    └── wall.txt the shared board
```

There is no root. A path starting with `/` never resolves; paths are relative, or start at `~`.

One entry is not in the binary. `~/var/wall.txt` is a `Node::Live`, held by the server, so reading it is a request. `cat` resolves its operands into a plan first and only answers late if the plan holds one, which keeps `cat documents/now.txt var/wall.txt` in order and leaves a static `cat` immediate. One request covers however many times the file is named.

The banner does not keep its own copy of the intro. It renders the first paragraph of `content/documents/about.txt`, which is also the page's meta description, so there is one place to edit it.

## Commands

`help` lists them, generated from the registry in `src/commands.rs` rather than written out, so it cannot fall behind. Adding a command is one entry in `COMMANDS` and one function.

The line editor takes the readline bindings a shell does. `ctrl-c` abandons the line, `ctrl-l` clears the screen and keeps what is half typed, `ctrl-d` closes the session on an empty line, and `ctrl-a`/`e`/`u`/`k` move to the ends and cut to them.

`ctrl-c` gives way to the browser when there is a selection, since that means copy. `ctrl-w` and `ctrl-n` are absent: browsers keep them and will not hand them over.

A command can answer later. `Output::Pending` carries a task the terminal spawns, handing it a `Sink` to write into once it has something, so a command that has to ask the server does not make the prompt wait. `wall` is the one that does.

Each entry carries a `Spec` of the options and operands it takes, and `src/args.rs` checks the line against it before the function runs. No command parses its own arguments, so none can quietly ignore what it was given. Options may be clustered, `--` ends them, and a lone `-` is an operand, which is what lets `cd -` name a directory.

The same spec is what `help <command>` prints and what completion offers after a `-`, so usage text, completion and what actually runs are one thing. Every command takes `-h`. It also says what a command's operands can be, so Tab offers the right things: `cd` is only offered directories, and a command that takes no operands is offered nothing.

## Layout

A workspace: the site at the root, the programs under `bin/`, and what they share under `lib/`. Every member takes `[workspace.lints]`, so a program is held to the same conventions the site is.

```
.
├── bin/     the programs, one crate each
├── lib/
│   └── guest/   what all of them need from the terminal
├── content/ everything the site says
└── src/     the site
```

Inside `src/`, a module that got big enough to be doing several things has the rest beside it: `server.rs` with the routes, the logging and the shutdown; `terminal.rs` with the line editor, the alternate screen and the rendering; `wall.rs` with the grid, the server's copy and the browser's side of it; `wasi.rs` with the host state and the import object; `commands.rs` with the registry here and the functions grouped by what they touch.

One lint cannot be shared. `unsafe_code` is only *denied* by the workspace, because a guest has to declare and call its wasm imports and both are unsafe. The site adds `#![forbid(unsafe_code)]` at its crate roots, which is stricter and cannot be undone. Each guest keeps its unsafe in one `tty` module, with a reason on the `expect` and a safety comment on every call.

`default-members` is the site alone, so an ordinary `cargo build` never tries to link a guest for the host; the `justfile` asks for them explicitly. Adding a program is creating its directory under `bin/`, which both the workspace and the recipe glob.

## Prerequisites

1. WASM targets: `rustup target add wasm32-unknown-unknown wasm32-wasip1`
2. `cargo install cargo-leptos just`

## Running locally

```sh
just serve
```

The server listens on `[::1]:3000` (IPv6 loopback only), configurable via `site-addr` in `Cargo.toml` or `LEPTOS_SITE_ADDR`.

## Building for release

```sh
just build
```

Two builds, because the guests are for a different target: `just guests` puts them in `public/`, then cargo-leptos copies that into the site. It produces the server binary in `target/release` and the site assets in `target/site`. Bundle filenames are content hashed, so `target/release/hash.txt` is written beside the binary to map each one; the server reads it to build the `/pkg` URLs.

## Pre-built binary

Every push to `main` publishes a rolling [`build`](https://github.com/takashialpha/webpages/releases/tag/build) release carrying `webpages-x86_64-linux.tar.gz` and its `.sha256`. The archive holds the server binary next to `hash.txt` and a `site` directory, already laid out the way the environment variables below expect.

Extract it whole rather than copying pieces out. All three are one build and move together: `hash.txt` maps the content-hashed bundle names, and the server looks for it **beside the binary**, not under the site root. Leave it behind and the process still starts, then panics on the first render and serves nothing, so a proxy in front answers 502. Set `LEPTOS_HASH_FILE_NAME` to an absolute path if your layout cannot keep the two adjacent.

There are no version tags; the latest build is whatever is in `main`.

## Deploying

The unit, the deploy script and the proxy config are in `deploy/`. The first two travel in the release archive, so the server can fetch the pair that matches the binary. Install them by fetching, never by pasting into an editor: a paste that loses line breaks produces a script that still looks right and does not parse.

```sh
sudo curl -fsSL -o /usr/local/bin/deploy-webpages \
  https://raw.githubusercontent.com/takashialpha/webpages/main/deploy/deploy-webpages
sudo chmod +x /usr/local/bin/deploy-webpages
sudo curl -fsSL -o /etc/systemd/system/webpages.service \
  https://raw.githubusercontent.com/takashialpha/webpages/main/deploy/webpages.service
sudo systemctl daemon-reload
```

Then `deploy-webpages` fetches the latest build, swaps it in, and rolls back if the new release does not serve its own assets.

To do it by hand: build with `just build` rather than `cargo leptos build` alone, or `bin/` ships empty and every program 404s when it is run. Copy the server binary, `target/release/hash.txt` and the `target/site` directory to the host, keeping `hash.txt` in the same directory as the binary. Then set:

```sh
export LEPTOS_OUTPUT_NAME="webpages"
export LEPTOS_SITE_ROOT="site"
export LEPTOS_SITE_PKG_DIR="pkg"
export LEPTOS_SITE_ADDR="[::1]:3000"
export LEPTOS_HASH_FILES="true"
export WALL_PATH="/home/takashi/webpages/wall.txt"
```

and run the binary.

`WALL_PATH` has to point outside the release directory. A deploy replaces that directory wholesale, so a board kept inside it is destroyed on the next one. Leave the variable unset and it falls back to `wall.txt` in the working directory: fine locally, wrong for a deploy.

The board lives in a home directory so it can be edited by hand. That needs three things, once:

```sh
sudo -u takashi mkdir -p /home/takashi/webpages
sudo -u takashi touch /home/takashi/webpages/wall.txt
sudo chmod 2775 /home/takashi/webpages          # setgid: new files keep the group
sudo chmod 664  /home/takashi/webpages/wall.txt # the service writes it too
sudo chmod g+x  /home/takashi                   # so the service can traverse in
```

That last one is the easy one to miss. A home directory is usually `drwx------`, and permission checks take the first matching class and stop: a service in group `takashi` gets the group bits and never falls through to the ones set for everyone else. Giving *others* traverse does nothing for it. The group needs `x`.

The unit then wants `SupplementaryGroups=takashi`, plus `ProtectHome=read-only` with `ReadWritePaths=/home/takashi/webpages`. `ProtectHome=yes` would hide the directory entirely, and `ReadWritePaths=` cannot open a path that is not there.

If the service cannot write the board it says so at startup, as an `ERROR` naming the path. Nothing drawn would be kept, and that is not otherwise visible.

The server shuts down on SIGINT or SIGTERM. It stops accepting connections and waits for the requests in flight, so a restart never cuts a page off mid render. It exits non-zero if it fails to start, and says why. `GET /health` reports uptime and the commit it was built from.

## Logging

Plain text to stdout. `RUST_LOG` sets the verbosity, defaulting to `warn,webpages=info`: the startup and shutdown lines, plus any warnings and errors.

Request logging sits one level below, so it is off unless asked for:

```sh
export RUST_LOG="webpages=debug"
```

Each request then gets one line with its method, path, status and how long it took.

The format follows where stdout goes. Colour only when stdout is a terminal, and no timestamp when systemd owns stdout, because journald records its own.

The server checks two things before it opens the listener, because either can start cleanly and still be broken. `LEPTOS_SITE_ROOT` not pointing at a directory serves pages with no CSS or wasm. `hash.txt` missing from beside the binary panics on every render and serves nothing; that one is an `ERROR` naming the path it looked at.

## Time

`date` and `uptime` answer with the server's clock, not the browser's. The server stamps its UTC time and uptime onto the document, and the browser only measures how long the page has been open, so a skewed system clock still shows the right time. The formatting is `time`'s, in `src/clock.rs`.

## Programs

A program takes the whole terminal, draws into a grid of cells, and gives it back exactly as it was. That is the alternate screen: the scrollback sits untouched underneath, which is why leaving `vim` does not eat your shell history.

Programs are files. They live in `~/bin` as `Node::Program`, so `ls` lists them and completion offers them; `help` points at `bin/` rather than naming them, since they are not commands. The tree is the only place that says what exists.

Only a path runs one. `./bin/life` and `bin/life` both work and a bare `life` does not, because there is no `PATH` here and inventing one would mean a name resolving to something the tree does not say is there. A path landing on something that is not a program still reads as `command not found`, with a line under it saying the file is there but is not one.

The first word is completed as both, for the same reason: a command name, and a path, since typing a path is how a program is reached.

A program gets the same treatment as a command: it carries a `Spec`, so `life --help` prints usage and `life nonsense` is refused rather than quietly ignored. `About` in `src/args.rs` is what the two share.

A program that only prints runs to the end when opened: it takes microseconds, and what it printed belongs in the scrollback.

One that draws is different. The terminal owns the frame loop and calls it once a frame, because WebAssembly cannot suspend a synchronous call: a guest running its own loop would hold the page still until it finished. `src/program.rs` is that interface, `src/screen.rs` is the grid. Rows are cut into runs of matching colour, and only changed rows are repainted.

Colours are indices into the sixteen a console has, resolved by the stylesheet, so a program follows `theme` without knowing themes exist. The grid is measured from the terminal's real size rather than assuming 80 by 24.

While a program runs the scrollback is made invisible rather than hidden, because the input inside it is the keyboard and a `display: none` element cannot hold focus. It keeps its full size while it is out of sight: shrinking it throws its scroll position away and leaves the focused input somewhere ios tries to scroll the page to. `q` and `ctrl-c` both leave.

### Guests

A program can also be a `wasm32-wasip1` binary, fetched when it is run so none of it is in the bundle until someone asks. The browser has a WebAssembly engine, so nothing here interprets anything: the guest goes to that engine with an import object standing in for the operating system it thinks it has.

`src/wasi.rs` and the two modules beside it are that import object. It implements seven calls, which is all a terminal program reaches for: `fd_write`, `fd_read`, `environ_get`, `environ_sizes_get`, `clock_time_get`, `random_get` and `proc_exit`. Nothing else is stubbed. A missing import fails at instantiation and names itself, which is where it should be noticed.

`clock_time_get` answers the wall clock with the server's time and every other clock with how long the page has been open, so a guest timing itself gets something that only counts up.

A second module, `tty`, carries what WASI has no concept of, since preview1 cannot address a screen or read a key:

```
tty::cols() -> u32          tty::rows() -> u32
tty::put(x, y, ch, fg, bg)  tty::clear()
tty::key() -> i32           // -1 when nothing is waiting
```

Keys arrive as the bytes a terminal would send, escape sequences included, so a guest parses `\x1b[A` exactly as it would anywhere else.

`proc_exit` records the code and returns. The guest treats it as never returning and hits its own unreachable, and that trap ends the call. The recorded code is what tells a clean exit from a crash.

Guests live in `bin/` as workspace members, held to the same lints as the site. `just guests` builds them into `public/bin/`, which cargo-leptos copies into the site. They are build output, not source.

Their filenames carry a content hash, the same as the bundle: `tetris.15a6d22cdaaafa87.wasm`. A changed program gets a new URL, so the proxy can cache them forever and a deploy is picked up the moment the page reloads. The names cannot be baked into the site, because the site has to compile without the programs having been built — `just check` does exactly that. So `just guests` writes a `hash.txt` beside them, the server reads it once at startup and stamps it onto `<html data-programs>`, and the browser reads it back. That is the same route `data-uname` takes, for the same reason: it is something only the server can know.

What they all need from the terminal is one crate, `lib/guest`: the `tty` imports, the keyboard, and the chrome. Not under `bin/`, which holds programs and a library is not one. It is there because four copies of the same thing drift, and these had: `life` bound `[` to a slower speed, and an arrow key sends `esc [ A`, so pressing one slowed the simulation down. Reading the keyboard through one escape-aware reader is what fixes that everywhere at once.

### The ones there are

`life`, `snake`, `tetris` and `stars`, all guests. None is compiled into the site: there is only one way to be a program.

`life`, `snake` and `tetris` draw two cells per character with half blocks, which doubles the resolution and makes the cells square, and each carries a status bar that drops hints rather than overflow a narrow screen. `stars` is one character per star and has neither.

`snake` takes arrows or wasd. Arrows arrive as `esc [ A`, so it tracks the escape sequence rather than matching the bare letter, or a typed `A` would steer. A turn is stored and applied at the next step, so two keys in one frame cannot fold the snake into itself.

`tetris` is the only one with a fixed shape, so it is centred rather than sized to the screen. A block is drawn one or two characters across, whichever fits, and the same number of half rows down, which is what keeps it square: a character cell is exactly twice as tall as it is wide, and three would be a block and a half. Every screen half-cell asks which block it falls inside, so one path draws either size.

All seven pieces spawn lying flat, two rows at most, which is why the next-piece box is two rows and not four. J and L start a turn along from where their rotation tables are usually written, because that list stands them on end. The piece is centred in that box rather than drawn where the well would put it, since they do not all start in the same corner of their own 4x4.

All four take their randomness from `fastrand` rather than a hand-rolled generator, seeded through the shim's clock, and read the keyboard through `lib/guest`, which puts the escape sequence an arrow arrives as back together before the program sees it.

## The wall

The board is a file, `~/var/wall.txt`, so `ls` lists it and `cat` prints it raw, the way `cat` prints anything else. The frame and axes belong to `wall`, which draws it: `wall` prints the board with its coordinates, and `wall <x> <y> <char>` sets one cell. `wall <x> <y>` with nothing after it clears the cell, which is the only way to ask for a blank through a line that was split on whitespace. It is the only thing on the site anyone but me can change.

It is a wall, not a document, so `0,0` is the bottom left and `y` counts up, like the first quadrant of a graph. A frame closes it on all four sides, drawn with the box characters the VGA font carries, with rows numbered up the left and columns along the bottom. Eighty by twenty-four does not fit a phone, so the board goes in a box that scrolls sideways: a grid that wraps is not a grid.

The frame is 85 columns wide, being the board, a border either side and the row numbers, and that is where the stylesheet's line measure comes from. Everything the command prints stays inside it, the hint underneath included, since a longer line would make the whole board scroll sideways to read it.

Storage runs the other way, top row first, so the file reads in the same order as the board is drawn and editing it by hand needs no arithmetic. `Grid::set` is the one place the two orders meet.

Bounded on purpose. A write is one printable character at one coordinate; control characters, multi-byte characters and anything longer are refused rather than truncated. The worst anyone can do is spell something the next visitor writes over.

Writing is charged by the cell: eighty a minute per address, six hundred a minute across everyone. Both are `governor` rate limiters, so the budget refills steadily rather than all at once on the minute. A write is checked against the address's own budget first and everyone's second: a refused check costs nothing, so somebody hammering from one address spends their own budget and not the shared one.

That address comes from `CF-Connecting-IP`, which Cloudflare sets and strips from whatever the client sent. It is only trustworthy behind the proxy, so the global ceiling is what makes the limit enforceable: anything reaching the origin directly could claim a fresh address per request. Asking about an address is what starts remembering it, so past four thousand the table is swept of everyone whose budget has fully refilled.

Board text is never linkified and never becomes markup, so what somebody writes stays what they wrote.

The board is stored as twenty-four lines of eighty characters, so moderating it is editing a file. The server notices: it compares the file's modification time against its own last write and reads it back before answering if somebody has been in there. No restart, and no watcher running between edits. Reading it back pads short lines and cuts long ones, so an edit cannot leave it a shape the code does not expect.

Reading the file back is also why only one write runs at a time. Two writes racing each render a board and then let go of the lock, so the older render can reach the file last, and the next write reads that stale file back over a board that was right. Sixteen writers filling the board at once used to lose most of what they wrote; they now all land, and the file always matches what is in memory.

Writing is refused when the request came from another site's page. A `text/plain` POST is a CORS simple request, so any page anywhere could send one and never need to see the answer, and it would be the visitor's own address that got charged for it. `Sec-Fetch-Site` is set by the browser and a page cannot forge it. Nothing at all means it did not come from a browser, which is `curl` acting for whoever ran it, and that stays allowed.

`src/wall.rs` holds what a board is, `wall/grid.rs` the grid, `wall/state.rs` the server's copy and its limits, and `wall/client.rs` the browser's side. `src/server/board.rs` mounts `GET` and `POST /api/wall`.

## The screen

The terminal is sized to the visual viewport, not the layout one. `src/viewport.rs` measures it.

Those two are the same thing until a phone raises its keyboard, which shrinks only the visual one. Chrome can be told to keep them together with `interactive-widget=resizes-content` in the viewport meta, which is set, but Safari ignores it, so on iOS the layout viewport carries on behind the keyboard. Anything sized to `100dvh` is then taller than the screen, and anything `position: fixed` to the bottom is pinned somewhere you cannot see.

So the height and offset of the visual viewport are written onto the document as `--screen-height` and `--screen-top`, and the stylesheet builds the terminal from those. Three things follow. The terminal is fixed over the visible area and moves with it, so the page behind never scrolls, which is the scrolling that drags a fixed element out of place on iOS. `.screen` is the only scroller, sized to what the key row leaves, so the prompt scrolls to a bottom that really is the bottom. The key row is a flex item under it rather than an overlay, so nothing reserves space for it and it cannot be scrolled away or hidden by the keyboard.

Keeping the prompt in view is done by setting that scroller's own `scrollTop`, not by scrolling an element into view. Scrolling into view walks every scrollable ancestor, and iOS will scroll the document itself even with `overflow: hidden` on the body, which is exactly the thing that displaces a fixed terminal.

The extra-keys row is tab, up and down, which a phone keyboard has none of. It renders only on touch devices.

## Fonts

The page is set in the IBM VGA 8x16 ROM font, what a text-mode console and a bios screen actually draw. It comes from the [Ultimate Oldschool PC Font Pack](https://int10h.org/oldschool-pc-fonts/) and is served as woff2 from `public/fonts/`, about 6 KB. It is licensed [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/) by VileR; the license text ships alongside it in `public/fonts/vga-LICENSE.txt`.

The cell is 8 wide by 16 tall, so the font is only crisp at multiples of 16px and the type scale is pinned to them: 16px on a phone, 32px from 720px up.

The typed text is drawn by a span over the input, not by the input itself, so it renders the same as the line it becomes when echoed. The input still holds the text and takes the keys; it is transparent rather than hidden, so the selection it draws still shows behind those glyphs. The cost is that an input method's in-progress text would not show, which is acceptable for a terminal typing ASCII.

Drawing the text also lets a long line scroll in whole columns, which an input will not do: left alone it stops halfway through a character. The first visible column is tracked here, moving only when the caret would leave the field, and the cursor uses the same offset.

One function builds the prompt wherever it appears. It is split into elements so no text node looks like an email address, which Cloudflare rewrites. Both prompts must split the same way: each inline box rounds on its own, so the same text in one box and in four lands a sixteenth of a pixel apart.

Everything measured in cells uses a `--cell` custom property rather than the `ch` unit. `ch` comes from a font metric the woff2 conversion rounded: it reports 15.9998px where every glyph advances 16, which is enough to land text on fractional pixels, where it rasterises differently. Line measure, caret position and the prompt's gap are all whole cells.

The cell itself is measured once by putting a run of characters on the page, then remembered. Probing costs a DOM node and a forced layout, and the frame loop asks once a frame. It is measured again when the viewport resizes, which is the only thing that crosses the stylesheet's breakpoint, and when `document.fonts` reports the console font has arrived, since anything measured before that is the fallback's cell.

The padding is added onto the line measure rather than included in it, because `box-sizing: border-box` otherwise takes the padding out of the character count and slices the last column down the middle. There is no fluid sizing anywhere in the stylesheet, and `line-height` is 1 because the cell already is the line. On a 1280x800 screen that works out to exactly 80 columns by 25 rows, which is classic VGA text mode.

There is no bold face, because the ROM font has none. A console fakes bold with a bright colour and so does the stylesheet; asking for a weight the font does not have gets you a smeared synthetic one.

The favicons and the social card are generated from this same font rather than drawn by hand, so they cannot drift from how the site looks.

## License

Licensed under the [GNU Affero General Public License v3.0 or later](LICENSE).
