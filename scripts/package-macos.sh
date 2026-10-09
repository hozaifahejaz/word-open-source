#!/bin/sh
# Local unsigned Apple Silicon bundle; no install, signing or publication.
set -eu
root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$root"
export CARGO_HOME="$root/.tools/cargo"
export RUSTUP_HOME="$root/.tools/rustup"
export CARGO_TARGET_DIR="$root/.tools/target"
export TMPDIR="$root/.tools/tmp" TMP="$root/.tools/tmp" TEMP="$root/.tools/tmp"
export PATH="$CARGO_HOME/bin:$PATH"
mkdir -p "$TMPDIR"
[ "$(uname -s)" = Darwin ] || { echo 'Requires macOS and Apple command-line tools' >&2; exit 1; }
cargo build -p folio-desktop --release --target aarch64-apple-darwin --offline --locked
bundle="$root/dist/Folio.app"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
# Replace the inode rather than rewriting a previously launched Mach-O in place.
# This also leaves a running instance's mapped executable intact during rebuilds.
staged="$bundle/Contents/MacOS/.folio-desktop-$$"
trap 'rm -f "$staged"' EXIT HUP INT TERM
cp "$CARGO_TARGET_DIR/aarch64-apple-darwin/release/folio-desktop" "$staged"
chmod 755 "$staged"
mv -f "$staged" "$bundle/Contents/MacOS/folio-desktop"
cat > "$bundle/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>Folio</string>
<key>CFBundleDisplayName</key><string>Folio</string>
<key>CFBundleExecutable</key><string>folio-desktop</string>
<key>CFBundleIdentifier</key><string>io.github.hozaifahejaz.folio</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>CFBundleVersion</key><string>0.1.0</string>
<key>NSHighResolutionCapable</key><true/>
<key>NSHumanReadableCopyright</key><string>MIT; hozaifahejaz</string>
</dict></plist>
PLIST
plutil -lint "$bundle/Contents/Info.plist"
lipo -verify_arch arm64 "$bundle/Contents/MacOS/folio-desktop"
file "$bundle/Contents/MacOS/folio-desktop"
echo "Built $bundle (local unsigned app; no distribution signing)"
