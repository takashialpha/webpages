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
    #!/bin/sh
    # Ctrl-C is how a server is stopped, not a failure. It reaches every
    # process in the foreground group at once, so the recipe catches it and
    # ends quietly rather than letting the shell report a signalled child.
    trap 'exit 0' INT TERM
    cargo leptos serve --release

# everything CI runs
check: fmt deps (lint "ssr") (lint "hydrate") (lint "guests")

# check formatting
fmt:
    cargo fmt --all -- --check

# check for unused dependencies
deps:
    cargo shear

# lint one target: ssr, hydrate, or guests
lint target:
    #!/bin/sh
    set -eu
    case "{{ target }}" in
      ssr)     args="--no-default-features --features ssr --all-targets" ;;
      hydrate) args="--no-default-features --features hydrate --target wasm32-unknown-unknown" ;;
      guests)  args="--workspace --exclude webpages --target wasm32-wasip1" ;;
      *) echo "no such target: {{ target }}" >&2; exit 1 ;;
    esac
    cargo clippy $args -- -D warnings

# throw away everything built
clean:
    cargo clean
    rm -rf public/bin
