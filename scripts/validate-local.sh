#!/bin/sh
# Serial available-host validation using an already prepared offline cache.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
export CARGO_HOME="$root/.tools/cargo" RUSTUP_HOME="$root/.tools/rustup"
export CARGO_TARGET_DIR="$root/.tools/target"
export TMPDIR="$root/.tools/tmp" TMP="$root/.tools/tmp" TEMP="$root/.tools/tmp"
export PATH="$CARGO_HOME/bin:$PATH"
mkdir -p "$TMPDIR" build/validation
run_check() {
    name=$1
    shift
    echo "Running $*"
    if "$@" > "build/validation/$name.log" 2>&1; then
        echo "$name: PASS"
    else
        cat "build/validation/$name.log"
        exit 1
    fi
}
run_check fmt cargo fmt --check
run_check tests cargo test --workspace --offline --locked
run_check clippy cargo clippy --workspace --all-targets --offline --locked -- -D warnings
if [ "$(uname -s)" = Darwin ]; then
    run_check desktop sh scripts/package-macos.sh
else
    run_check desktop cargo build -p folio-desktop --release --offline --locked
fi
