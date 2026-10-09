# Reproducible local toolchain

Stable Rust **1.90.0**, minimal profile plus **rustfmt/clippy**, is pinned in
`rust-toolchain.toml`. Direct dependencies use exact versions; `Cargo.lock`
fixes transitive resolution. Resolver 3 and `.cargo/config.toml` prefer compatible
Rust-version dependencies during updates. Normal validation uses `--locked`.

## macOS and Linux

Run `sh scripts/setup-rust.sh` from the checkout, then the README's exports in
each new shell. Official rustup uses `--no-modify-path`; downloads, temporary
installer files and caches stay in ignored `.tools/`. Setup needs curl and HTTPS
access to sh.rustup.rs, static.rust-lang.org and crates.io.

macOS requires Apple command-line tools (`xcrun --find clang`). Linux requires a
C linker, pkg-config and native windowing/OpenGL development libraries. Both
X11 and Wayland backends are enabled; rfd uses portal dialogs. Debian-family
prerequisites typically include build-essential, pkg-config, libx11-dev,
libxcursor-dev, libxrandr-dev, libxi-dev, libxkbcommon-dev, libwayland-dev and
libgl1-mesa-dev; names vary by distribution. The script does not install system
packages. Report missing libraries to the environment owner.

## Windows

Use the Visual Studio C++ Build Tools/Windows SDK and stable MSVC Rust. The POSIX
script is not a PowerShell installer. From the checkout root in PowerShell:

```powershell
$env:CARGO_HOME = "$PWD\.tools\cargo"
$env:RUSTUP_HOME = "$PWD\.tools\rustup"
$env:CARGO_TARGET_DIR = "$PWD\.tools\target"
$env:TEMP = "$PWD\.tools\tmp"
$env:TMP = $env:TEMP
New-Item -ItemType Directory -Force $env:TEMP | Out-Null
# x86_64 host; use the official aarch64 installer URL for ARM64.
Invoke-WebRequest https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe -OutFile .tools\rustup-init.exe
& .tools\rustup-init.exe -y --no-modify-path --profile minimal --default-toolchain 1.90.0 --component rustfmt --component clippy
if ($LASTEXITCODE -ne 0) { throw "rustup installation failed" }
$env:PATH = "$env:CARGO_HOME\bin;$env:PATH"
cargo build --workspace --locked
cargo test --workspace --locked
```

## Scoped caches

The script always writes its checkout's `.tools/`. Workers with narrower scopes
must have authorization for that path or install into a permitted ignored local
cache. Set `CARGO_HOME`, `RUSTUP_HOME`, `CARGO_TARGET_DIR`, `TMPDIR`, `TMP` and
`TEMP` to private permitted paths before running the downloaded official
installer with the same version/flags/components. Keep the installer itself
there too. If writes outside the checkout are prohibited, every path must remain
inside it. Never share mutable caches between concurrent workers.

Build/tests/fmt/clippy commands are in README. Core tests are headless; the shell
needs a graphical display to launch. One native host build does not establish
all-platform portability: each OS needs its own build and UI checks.

## Prepared offline checkout and bundle reproduction

Once setup and fetching have completed, Rust 1.90.0 and locked dependencies are
available under `.tools`. Use absolute checkout-local
CARGO_HOME, RUSTUP_HOME, CARGO_TARGET_DIR and temporary paths as in the root README.
All local integration Cargo commands use `--offline --locked` (fmt only inspects
source). The packaging script always uses offline mode, also compatible with
`CARGO_NET_OFFLINE=true`, and never installs a missing target.

Apple Silicon packaging requires `aarch64-apple-darwin` standard libraries in the
prepared toolchain. On a fresh authorized machine, prepare the same toolchain,
then run `rustup target add aarch64-apple-darwin --toolchain 1.90.0` and
`cargo fetch --locked --target aarch64-apple-darwin` before offline packaging.
`sh scripts/package-macos.sh` creates ignored `dist/Folio.app`; launch with
`open "$PWD/dist/Folio.app"`. Check architecture with
`file dist/Folio.app/Contents/MacOS/folio-desktop` and metadata with
`plutil -lint dist/Folio.app/Contents/Info.plist`. These are source-build
reproduction commands, not a signed/notarized installer or a byte-identical
binary guarantee across SDK/toolchain environments.

CI uses hosted native OS runners, Rust 1.90.0, the committed lockfile and network
access for preparation. Its steps execute serially within each OS job; OS jobs
may run concurrently. A successful build does not replace OS GUI/IME/assistive
technology checks. Linux also needs a running desktop/portal service for dialogs.
Windows requires a graphical session for UI checks and the MSVC SDK/linker.
