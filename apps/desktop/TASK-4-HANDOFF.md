# Desktop handoff to task-4

Version remains **0.1.0**. No shared source, root manifest/lockfile, packaging,
Git staging/commit/branch or shell profile edits were made by this task.
All delivered changes are under `apps/desktop`; local caches are ignored `.tools`.
The app manifest only updates its description, requiring no lockfile change.

## Shared interface observations

- All core/codec names conform to `docs/INTERFACES.md`; no blocking mismatch was
  found. The codec and desktop implementations are now integrated; final results are
  recorded in `docs/ACCEPTANCE.md`. The earlier scoped launch below was only a
  pre-integration check.
- `export_docx` warnings are conservatively treated as a failed save. Integration
  should verify that fully supported documents return no export warnings.
- Replacing a selection with a structural page break takes Delete plus
  InsertPageBreak, hence two history steps. A future atomic selection/page-break
  command would remove that limitation without bypassing core mutation rules.
- A public iterator of grapheme boundaries would avoid repeated whole-paragraph
  validation for large non-ASCII paragraphs. Current app caches boundary sets
  and directly accepts only proven-safe ASCII-to-ASCII boundaries.
- Font family metadata is preserved. Only bundled Sans/Serif are selectable;
  arbitrary imported font names display with Sans fallback.

## Integration verification

Create/test the unsigned Apple Silicon Folio.app bundle in your packaging scope.
The built worker binary is `apps/desktop/.tools/target/debug/folio-desktop` (Mach-O
arm64). It was launched and stayed running without stderr; CUA cannot identify an
unbundled executable, so no claim of native visual/dialog QA is made here.
The test process was stopped after the launch check.

After integrating the codec, exercise New/Open/Save/Save As, cancelled native
sheets, overwrite confirmation, errors, close with Save/Discard/Cancel, and a
warned source plus symlink/hard-link alias. Confirm the source is unchanged and
subsequent Save uses its converted-copy path. Test macOS Command-F followed by
typing, ribbon multi-digit numeric entry/Enter, native copy/cut/paste, OS IME
preedit/commit/cancel, keyboard selection across automatic and explicit page
boundaries, formatting and undo. Confirm visible text/caret/page geometry and
VoiceOver labels in the native bundle. Windows/Linux remain untested.

App-scoped Rust 1.90.0 tests, clippy with warnings denied, and arm64 debug build
pass. See `README.md` for command/environment setup and deferred features.

## Integrated packaging handoff

Preserve ignored `dist/Folio.app` until the lead captures it before review cleanup.
Ignored `.tools`, `build/smoke` and `dist` artifacts are not integrated with source.
After integration the lead must reproduce from the root README: prepare Rust
1.90.0/target and `cargo fetch --locked` if absent, set its checkout-local exports,
then run serial fmt/test/clippy and `sh scripts/package-macos.sh`, followed by
`open "$PWD/dist/Folio.app"`. The current assignment uses the supplied caches
and performs no installation or fetching. Git finalization/checkpoint repair
belongs to the lead as hozaifahejaz. See `docs/ACCEPTANCE.md` for actual checks.

## Task-4 current result

Serial fmt/test/clippy/arm64 release validation passed (20 core, 21 codec,
15 desktop tests). Actual bundle smoke checks completed within the limits in
`docs/ACCEPTANCE.md`; full CJK IME/cancellation and VoiceOver remain pending.
Word/LibreOffice were unavailable; independent textutil extraction passed.
The final bundle is preserved at `dist/Folio.app`, with logs at
`build/validation/` and GUI DOCX exports at `build/smoke/`. The final app is left
open on saved `serif-export.docx` for capture. No staging/commits/pushes occurred.
The final executable SHA256 is
`43cb025af89c113b961f5058b4ba2c40160369893644c2cf1d6a13e57350c5f6`.
Warned original/alias SHA256 before and after all save attempts was
`f19a4c44e6a21a9a2bc6c23660d46bc3799eb0d1d1523710a5424d2bd95dd69d`.
Do not rerun fixture preparation until after capture: it regenerates inputs.
