# The site is two builds. The guests are separate crates for a different
# target, so cargo cannot do both at once, and cargo-leptos has no hook to run
# one before the other.
#
# This is a command runner, not a build system: cargo already knows what is
# stale, and a second layer of timestamps on top of it would only be another
# thing that can be wrong. Every recipe here asks cargo, every time.

# list what there is to run
default:
    @just --list --unsorted

# build the guests and then the site
build: guests
    cargo leptos build --release

# build the guest programs into public/, which cargo-leptos copies into the site
guests:
    #!/bin/sh
    set -eu
    # The programs are whatever is in bin/, the same list the workspace globs.
    names=$(ls bin)
    cargo build --profile guest --target wasm32-wasip1 \
        $(for name in $names; do printf -- '-p %s ' "$name"; done)
    mkdir -p public/bin
    for name in $names; do
        cp "target/wasm32-wasip1/guest/$name.wasm" "public/bin/$name.wasm"
        echo "public/bin/$name.wasm"
    done

# build the guests and serve the site
serve: guests
    cargo leptos serve --release

# format and lint everything, site and guests, the way CI does
check:
    cargo fmt --all -- --check
    cargo clippy --all-targets --all-features -- -D warnings
    cargo clippy --workspace --exclude webpages --target wasm32-wasip1 -- -D warnings
    cargo shear

# throw away everything built
clean:
    cargo clean
    rm -rf public/bin
