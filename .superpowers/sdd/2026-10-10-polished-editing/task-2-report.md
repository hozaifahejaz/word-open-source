# Task 2 — Rich text formatting end to end

Status: DONE. Implemented on authorized `main`, base `c1f7f0b`, using configured
`hozaifahejaz <319821010+hozaifahejaz@users.noreply.github.com>`. This report is
included in the scoped feature commit. Controller's plan amendments are excluded.
No new dependencies, subagents, network build tools or toolchain changes.

## Implementation and files

- `crates/document-core/src/model.rs`: public snake_case `VerticalAlign`, serde-defaulted strike/script/highlight properties, matching patches including tri-state highlight, and shared `StylePatch::apply`. Existing validated transactional `FormatRuns` supplies splitting, history and document validation; new properties are fully included by style equality/serialization. The enum and RGB bytes have no invalid in-memory combinations.
- `crates/document-core/tests/{commands,recovery_serialization}.rs`: mixed styles across combining/ZWJ graphemes, bounded selected formatting, highlight clear, superscript→subscript replacement, Undo/Redo, invalid-style rollback, old JSON defaults and new-value serialization. Existing exhaustive literals now supply defaults.
- `crates/docx/src/{formatting,codec}.rs` and `crates/docx/tests/roundtrip.rs`: strike toggle/direct behavior, vertical alignment, arbitrary RGB clear run shading export, named highlight palette import, clear/nil/solid shading handling and diagnostics. Import-only background layers survive defaults/named/direct inheritance. Highlight wins regardless of child order; clearing it reveals inherited shading. Layer flattening and unsupported patterns/theme/automatic shading warn. Solid reads foreground `color`, never background `fill`.
- `apps/desktop/src/{layout,main,editing,icons,workspace}.rs`: native strike/background tessellation, 75% script fonts and actual top/bottom glyph offsets, full-height script-only rows, original accessible Home icons and palette with None, shared validated caret patches, clear-format defaults, matching Bold focus behavior, schema-1 reads/schema-2 writes/schema-3+ refusal. Future recovery sentinels now use 3 and retain their original byte-preservation assertions.
- `crates/docx/README.md`, `docs/{FEATURES,INTERFACES,ACCEPTANCE}.md`: supported behavior, migration/patch contracts, codec semantics and acceptance boundaries.

## RED/GREEN evidence

Each command below used `CARGO_HOME="$PWD/.tools/cargo"`,
`RUSTUP_HOME="$PWD/.tools/rustup"`, `CARGO_TARGET_DIR="$PWD/.tools/target"`
and `.tools/cargo/bin/cargo` (Rust 1.90.0, offline locked cache).
Observed excerpts from tool outputs:

```text
cargo test -p document-core --test recovery_serialization rich_style --offline --locked
RED: rich_style_json_preserves_new_properties_and_old_defaults FAILED
left: Null; right: false (new default strikethrough absent)
test result: FAILED. 0 passed; 1 failed
GREEN: test result: ok. 1 passed; 0 failed

cargo test -p document-core --test commands rich_formatting --offline --locked
RED: rich_formatting_preserves_mixed_graphemes_and_undo_redo FAILED
left: 2; right: 4 (new patch did not split/style selected runs)
test result: FAILED. 0 passed; 1 failed
GREEN: test result: ok. 1 passed; 0 failed

cargo test -p folio-docx --test roundtrip rich_run --offline --locked
RED: unsupported strike/vertAlign/shd warnings, 0 passed; 1 failed
cargo test -p folio-docx --test roundtrip rich_ --offline --locked
RED: all three rich tests FAILED: background None instead of expected RGB;
unsupported run-property warnings; inherited background None instead of green
GREEN: test result: ok. 3 passed; 0 failed

cargo test -p folio-desktop rich_ --offline --locked
RED: 0 passed; 3 failed
rich_caret_formatting_uses_validated_shared_patch_and_clear_resets_all:
  style.strikethrough assertion failed
schema_one_migrates_and_schema_two_preserves_rich_recovery:
  left: Number(1); right: 2
rich_scripts_have_real_vertical_offsets_and_grapheme_safe_hits:
  expected font ratio 0.75 assertion failed
GREEN: test result: ok. 3 passed; 0 failed

cargo test -p folio-desktop rich_home_controls --offline --locked
RED: missing Strike control, 0 passed; 1 failed
GREEN: real AccessKit-node-bound pointer activation, 1 passed; 0 failed

cargo test -p folio-desktop rich_ --offline --locked
Final focused GREEN: 6 passed; 0 failed; 99 filtered out
```

A test-only missing `Block` qualification was corrected before the desktop
behavioral RED run. A first pointer helper searched painted text, but compact
icons deliberately paint no label; it was replaced by actual accessible label
and bounds lookup. Additional geometry/keyboard regression assertions exposed
existing justified trailing-space coordinate coincidence and canvas Tab insertion.
The tests now compare coincident valid caret coordinates and start traversal
with ribbon focus after Escape, matching established UI behavior. Product
rendering or text handling was not weakened to satisfy those assumptions.

## Full verification

```text
$ sh scripts/validate-local.sh
Running cargo fmt --check
fmt: PASS
Running cargo test --workspace --offline --locked
tests: PASS
Running cargo clippy --workspace --all-targets --offline --locked -- -D warnings
clippy: PASS
Running sh scripts/package-macos.sh
desktop: PASS
```

Exit 0. `build/validation/tests.log` records 171 passed, 0 failed:
35 core commands + 6 core recovery serialization + 104 desktop + 26 DOCX;
one intentionally ignored manual cold-layout benchmark. Zero-test unit/doc-test
targets also passed. `build/validation/clippy.log` records all three crates checked
with warnings denied. `build/validation/desktop.log` records optimized offline
release compilation, valid Info.plist, Mach-O arm64 binary and the local unsigned
`dist/Folio.app`. `git diff --check` exited 0. These build logs are ignored local
artifacts; the commands and observations above are the durable report.

## Self-review and limitations

Reviewed the full scoped diff and requirement list after validation. Existing
core transactions preserve each partial run style and all grapheme bytes;
new properties participate in undo snapshots, normalization and recovery.
Caret-only patches remain pending typing state and validate before retention.
Clear formatting resets every new property at a caret and in selected text.
Schema headers are checked before full decode and writes first load existing
state, preserving unreadable/future bytes. Original recovery lifecycle assertions
remain intact, with the minimum future schema 3 exercised explicitly.

Inspected local `epaint-0.32.3/src/text/text_layout.rs`: egui computes vertical
offsets from row height minus glyph line height. Script sections therefore use
75% font AND line height. Rows lacking a full-height glyph are shaped in bounded
two-row windows with a first-row minimum; the continuation preserves wrapping
and justification. No fabricated glyphs enter text or caret maps. Regression
checks inspect actual glyph y coordinates, RGB mesh vertices, row text/width,
full-height script-only wrapped rows, zoomed hits/carets and selection rectangles.
All-script rendering may do this additional bounded shaping work; no new
performance benchmark or optimization claim is made.

The Home palette exposes None plus seven named colors; arbitrary RGB remains
supported through core styles and DOCX. OOXML background layers are flattened
with warnings because the public model retains one visible color. Themes,
patterned/automatic backgrounds and Word/LibreOffice visual/schema conformance
remain outside support. Existing MCP format-tool schema was not expanded by this
scoped task. Automated native-egui/AccessKit/input checks and macOS packaging do
not constitute a new OS screen-reader audit or a Windows/Linux GUI run; no native
bundle launch or cross-application visual comparison was performed.

Official semantics reviewed 2026-10-10:
[Open XML Shading](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.shading?view=openxml-3.0.1)
and [Open XML Highlight](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.highlight?view=openxml-3.0.1).
No unresolved test failures or implementation blockers.
