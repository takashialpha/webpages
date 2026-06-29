# my webpages

[![live at takashialpha.com](https://img.shields.io/badge/live-takashialpha.com-a6e3a1?style=flat-square&labelColor=11111b)](https://takashialpha.com)
[![Leptos](https://img.shields.io/badge/Leptos-ef3939?style=flat-square&labelColor=11111b)](https://github.com/leptos-rs/leptos)
[![Axum](https://img.shields.io/badge/Axum-cba6f7?style=flat-square&labelColor=11111b)](https://github.com/tokio-rs/axum)

My personal site, served live at **[takashialpha.com](https://takashialpha.com)**. A server-rendered Rust app built on a [Leptos](https://github.com/leptos-rs/leptos) and [Axum](https://github.com/tokio-rs/axum) stack, with hydration on the client.

## Prerequisites

1. WASM target: `rustup target add wasm32-unknown-unknown`
2. `cargo install cargo-leptos`

## Running locally

```bash
cargo leptos serve --release
```

The server listens on `[::1]:3000` (IPv6 loopback only) by default, configurable via `site-addr` in `Cargo.toml` or the `LEPTOS_SITE_ADDR` environment variable.

## Building for release

```bash
cargo leptos build --release
```

Produces the server binary in `target/release` and the site assets in `target/site`.

## Deploying

After `cargo leptos build --release`, copy the server binary and the `target/site` directory to the target host, then set:

```sh
export LEPTOS_OUTPUT_NAME="webpages"
export LEPTOS_SITE_ROOT="site"
export LEPTOS_SITE_PKG_DIR="pkg"
export LEPTOS_SITE_ADDR="[::1]:3000"
```

and run the binary.

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
