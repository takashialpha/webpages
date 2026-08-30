# The site is two builds. The guests are separate crates for a different target,
# so cargo cannot do both at once and cargo-leptos has no hook to run one first.
#
# A command runner, not a build system: cargo already knows what is stale, and a
# second layer of timestamps would only be another thing that can be wrong.

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
    # Whatever is in bin/, the same list the workspace globs.
    names=$(ls bin)
    cargo build --profile guest --target wasm32-wasip1 \
        $(for name in $names; do printf -- '-p %s ' "$name"; done)
    # Content-hashed, the way cargo-leptos names the bundle, so a program can
    # be cached forever and a changed one gets a new URL. `hash.txt` is how the
    # server learns the names; see program.rs. Emptied first, or yesterday's
    # hashes pile up in there.
    rm -rf public/bin
    mkdir -p public/bin
    : > public/bin/hash.txt
    for name in $names; do
        built="target/wasm32-wasip1/guest/$name.wasm"
        hash=$(sha256sum "$built" | cut -c1-16)
        cp "$built" "public/bin/$name.$hash.wasm"
        echo "$name: $hash" >> public/bin/hash.txt
        echo "public/bin/$name.$hash.wasm"
    done

# build the guests and serve the site
serve: guests
    #!/bin/sh
    # Ctrl-C is how a server is stopped, not a failure. It reaches the whole
    # foreground group, so catch it and end quietly rather than let the shell
    # report a signalled child.
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
