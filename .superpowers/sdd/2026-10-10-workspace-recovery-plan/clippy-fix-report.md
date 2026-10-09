# Workspace Clippy cleanup

Collapsed the four nested `if` statements reported by `build/validation/workspace-recovery-clippy.log` in `apps/desktop/src/main.rs`. Existing conditions, errors, and early-return behavior are preserved.

Validation from the repository root:

- `CARGO_HOME="$PWD/.tools/cargo" RUSTUP_HOME="$PWD/.tools/rustup" PATH="$PWD/.tools/cargo/bin:$PATH" cargo fmt --all -- --check` — passed (exit 0).
- `CARGO_HOME="$PWD/.tools/cargo" RUSTUP_HOME="$PWD/.tools/rustup" PATH="$PWD/.tools/cargo/bin:$PATH" CARGO_TARGET_DIR="$PWD/.tools/target" cargo clippy --workspace --all-targets --offline --locked -- -D warnings` — passed (exit 0; `Finished dev profile`).
- `git diff --check` — passed.
