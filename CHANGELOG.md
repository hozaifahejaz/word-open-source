# Changelog

Project author: hozaifahejaz.

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
