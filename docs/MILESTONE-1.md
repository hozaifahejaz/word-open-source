# First milestone acceptance

Folio targets an offline native editor on Windows, macOS and Linux, with one
document-wide layout and an explicitly limited DOCX subset.

## Foundation — this change

- Build all three crates with `cargo build --workspace --locked` on the available
  host, with compatible pinned dependencies and `Cargo.lock`.
- Pass `cargo test --workspace --locked`, covering styled cross-run/paragraph
  edits, extended graphemes, formatting, history, saved-state tracking, atomic
  failures, search, and page layout.
- Pass fmt/clippy checks with checkout-local tools and dependency caches.
- Document units, positions, commands, dirty state, warnings and stream contracts;
  keep the core independent of UI and filesystem operations.
- Integrate the bounded DOCX subset and native rich-text editor with complete manifests.

## Integrated product acceptance

- Launch/build on each target OS, recording tested versions rather than inferring
  portability from one build.
- Create/open/save, with unsaved-change prompts and failure-safe saving that
  preserves the previous file and does not clear dirty state on failure.
- Render/edit styled runs with caret/selection, keyboard/clipboard editing,
  multiline insert/delete, split/join, and undo/redo through core commands.
  Respect grapheme boundaries and document font fallback limits.
- Expose supported fonts/styles, alignment/spacing, size/orientation/margins and
  page breaks; provide literal find/replace with the documented semantics.
- Roundtrip the supported DOCX subset using real packages. Fixtures cover mixed
  runs, whitespace/entities, paragraph settings, breaks and single page layout.
  Malformed/resource-heavy packages fail safely.
- Show structured warnings for unsupported/approximated content. Any import with
  fidelity or unsupported-content warnings must use converted-copy **Save As to
  a different path**, retaining the original source unchanged. Acknowledging loss
  does not authorize overwriting the source. Test source retention and reject
  alternate paths that alias the source; never imply unknown parts are preserved.
- Add CI and integration checks. Validate native rendering, dialogs, shortcuts,
  IME and assistive technology manually where automation is insufficient.

Pixel-identical Word rendering, full DOCX fidelity, and the deferred
[feature families](FEATURES.md) are outside this milestone. Current local evidence and outstanding checks are recorded in [acceptance](ACCEPTANCE.md).
