# Changelog

Project author: hozaifahejaz.

## Unreleased

- Add original line icons alongside labeled ribbon buttons and tooltips.
- Add native clipboard toolbar actions and undoable clear-text-formatting.
- Expose exact/minimum line heights, preserving imported values and applying
  typed heights at commit. Add selection and document word/grapheme counts.
- Test clipboard focus, clear formatting, Unicode statistics and exact line-height
  editing through undo and DOCX roundtrip.
- Report omitted paragraph-mark formatting in named DOCX styles so imported
  sources require a protected converted copy.
- Preserve spacing and typing styles around imported break-only paragraphs.
- Replace a selection with a page break as one atomic, undoable core command.
- Compute Unicode grapheme caret boundaries in one pass. The manual debug
  benchmark for 4,000 accented characters improved from 1.64 s to 9.6 ms locally.
- Polish the desktop light theme with grouped tools, a prominent Save action,
  wrapping controls, clearer document status and a converted-copy warning action.
- Add regression coverage and a manually runnable Unicode layout benchmark.

## 0.1.0 — desktop foundation (2026-10-09)

- Integrated a pure document model, grapheme-safe editing, formatting, bounded
  snapshot undo/redo, saved-content tracking and literal find/replace.
- Integrated bounded ZIP/XML DOCX import/export for styled paragraphs, Unicode,
  inherited styles, alignment/spacing, explicit breaks and single page layout.
  Unsupported content and approximations produce structured warnings.
- Added a native paginated rich-text editor, File/Home/Layout/View controls,
  clipboard/IME event handling, visible import warnings, native file dialogs,
  unsaved-change prompts and atomic saves. Warned sources and aliases are protected;
  converted copies save separately.
- Added reproducible offline Apple Silicon release packaging into local unsigned
  `dist/Folio.app`, bundle ID `io.github.hozaifahejaz.folio`, version 0.1.0.
- Fixed imported Noto Serif display/selector mapping while retaining DOCX font
  metadata. Added an app/core/codec/filesystem roundtrip regression test.
- Bundle rebuilds replace the executable through a staged file rather than
  rewriting a previously launched Mach-O in place.
- Added Windows/macOS/Linux CI for formatting, workspace tests, clippy and release
  desktop builds. CI and other-platform behavior require their own results.
- Retained Rust 1.90.0, exact direct dependencies, lockfile, MIT attribution and
  original Folio branding. Build and tool artifacts remain ignored.
- Documented [actual acceptance results](docs/ACCEPTANCE.md), prerequisites and
  known layout/fidelity/accessibility limits. Full Word parity is not claimed.
