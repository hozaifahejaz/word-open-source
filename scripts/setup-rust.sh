#!/bin/sh
# Run from any directory; keep Rust and installer artifacts in this checkout.
set -eu

project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
export CARGO_HOME="$project_root/.tools/cargo"
export RUSTUP_HOME="$project_root/.tools/rustup"
export TMPDIR="$project_root/.tools/tmp"
export TMP="$TMPDIR"
export TEMP="$TMPDIR"
mkdir -p "$CARGO_HOME" "$RUSTUP_HOME" "$TMPDIR"

if [ ! -x "$CARGO_HOME/bin/rustup" ]; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        -o "$project_root/.tools/rustup-init.sh"
    sh "$project_root/.tools/rustup-init.sh" -y --no-modify-path \
        --profile minimal --default-toolchain none
fi

export PATH="$CARGO_HOME/bin:$PATH"
cd "$project_root"
# Keep this version in sync with rust-toolchain.toml.
rustup toolchain install 1.90.0 --profile minimal --component rustfmt --component clippy
rustup show active-toolchain
rustc --version
cargo --version
