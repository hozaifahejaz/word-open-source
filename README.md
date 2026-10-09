# Folio 0.1.0

Folio is an MIT-licensed Rust desktop word processor by **hozaifahejaz**, targeting
Windows, macOS and Linux. This is a desktop foundation, not full Microsoft Word
parity or a production-ready replacement.

The integrated app edits paginated styled paragraphs with Unicode-safe selection,
formatting, undo/redo, literal find/replace, page breaks and document-wide layout.
The DOCX codec imports/exports a documented subset, resolves inherited styles,
and warns about omitted or approximated content. Warned imports require a separate
converted copy; their source and filesystem aliases cannot be overwritten.
Tables, images, lists, headers/footers, reviewing, printing and proofing are deferred.
An [MCP interface](docs/MCP.md) lets compatible AI clients read and edit the live
document or use independent background sessions through 14 provider-neutral tools.
See [feature boundaries](docs/FEATURES.md) and [actual acceptance results](docs/ACCEPTANCE.md).

## Apple Silicon local app

Prerequisites: macOS on Apple Silicon, Apple command-line tools, and Rust 1.90.0
with rustfmt/clippy and the `aarch64-apple-darwin` target. From a fresh clone,
prepare checkout-local tools and locked dependencies with network access:

```sh
sh scripts/setup-rust.sh
export CARGO_HOME="$PWD/.tools/cargo"
export RUSTUP_HOME="$PWD/.tools/rustup"
export CARGO_TARGET_DIR="$PWD/.tools/target"
export TMPDIR="$PWD/.tools/tmp" TMP="$PWD/.tools/tmp" TEMP="$PWD/.tools/tmp"
export PATH="$CARGO_HOME/bin:$PATH"
rustup target add aarch64-apple-darwin --toolchain 1.90.0
cargo fetch --locked
```

Once prepared, the following checks and packaging run offline:

```sh
cargo fmt --check
cargo test --workspace --offline --locked
cargo clippy --workspace --all-targets --offline --locked -- -D warnings
sh scripts/package-macos.sh
open "$PWD/dist/Folio.app"
```

The script builds `folio-desktop --release --target aarch64-apple-darwin --offline
--locked`, checks the plist and arm64 architecture, and assembles **local unsigned**
`dist/Folio.app` with bundle ID `io.github.hozaifahejaz.folio` and version 0.1.0.
It does not install tools, sign, notarize or publish. `dist/`, build outputs and
tool caches are ignored; each clone must build its own bundle.
Offline packaging requires the cached toolchain, target and dependencies.
For a fresh authorized environment, [toolchain setup](docs/TOOLCHAIN.md) describes
preparing them; offline mode cannot fill a missing cache.

## Cross-platform development

Use the same exports on macOS/Linux. Run `cargo build -p folio-desktop --offline
--locked`, then `cargo run -p folio-desktop --offline --locked` with a graphical
session. Windows uses equivalent PowerShell exports and Visual Studio C++ Build
Tools/Windows SDK. Linux needs a C linker, pkg-config and X11/Wayland/OpenGL
libraries; rfd uses native/portal dialogs. See [platform prerequisites](docs/TOOLCHAIN.md).
CI is configured to run fmt, workspace tests, clippy and release desktop builds on
all three OSes.
CI results and native Windows/Linux GUI behavior remain separate from Mac results.

| Package | Responsibility | State |
| --- | --- | --- |
| `document-core` | Pure model, commands, selection, history and warnings | Implemented; 22 regression tests |
| `folio-docx` | Bounded ZIP/XML DOCX subset conversion | Implemented; 23 codec tests |
| `folio-desktop` | Native rich-text canvas, dialogs, MCP and atomic saves | Implemented; 36 regression tests and a manual layout benchmark |

Keep mutations in shared core commands and filesystem operations in the app.
`Cargo.lock` fixes transitive dependencies; normal builds use `--locked`.
Read the [contracts](docs/INTERFACES.md), [milestone](docs/MILESTONE-1.md),
[desktop guide](apps/desktop/README.md), [roadmap](ROADMAP.md) and
[changelog](CHANGELOG.md). Folio bundles original text branding and licensed
Noto fonts, with no Microsoft logos, templates or assets. [MIT license](LICENSE).
