# web-page-test

A [Leptos](https://github.com/leptos-rs/leptos) + [Axum](https://github.com/tokio-rs/axum) web app, built with [cargo-leptos](https://github.com/leptos-rs/cargo-leptos).

## Prerequisites

1. Rust nightly: `rustup toolchain install nightly --allow-downgrade`
2. WASM target: `rustup target add wasm32-unknown-unknown`
3. `cargo install cargo-leptos --locked`
4. `dart-sass`: `npm install -g sass`

## Running locally

```bash
cargo leptos watch
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
export LEPTOS_OUTPUT_NAME="web-page-test"
export LEPTOS_SITE_ROOT="site"
export LEPTOS_SITE_PKG_DIR="pkg"
export LEPTOS_SITE_ADDR="[::1]:3000"
export LEPTOS_RELOAD_PORT="3001"
```

and run the binary.

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
