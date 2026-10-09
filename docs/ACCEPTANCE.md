# Folio 0.1.0 acceptance results

Recorded 2026-10-09 for the integrated desktop foundation by hozaifahejaz.
Available host: Apple Silicon arm64, macOS **27.0 (26A428)**, Apple clang
21.0.0, Rust **1.90.0**. Windows and Linux were not executed locally.
This report records scoped evidence, not full Word parity or a release certification.

## Editing tools and icons — 2026-10-09 follow-up

`sh scripts/validate-local.sh` passed formatting, 71 tests (22 core, 23 DOCX,
26 desktop; one manual benchmark ignored), clippy with warnings denied and arm64
release packaging. Regression tests cover native paste requests and keyboard
activation, clear formatting at a selection/caret, Unicode statistics and exact
line-height editing through undo and DOCX roundtrip.

The rebuilt macOS app was inspected with its original line icons and labeled
buttons. Toolbar Copy/Cut/Paste preserved `Café é` and `👩‍💻 hi` across two
paragraphs; document/selection statistics showed 4 words and 10 characters.
Bold toggled on and Clear formatting toggled it off. The exact line-height field
accepted 18.5 pt and displayed `Exactly 18.5 pt`. Other-platform GUI behavior
and a full screen-reader audit remain outside this check.

## Automated validation — locally passed

Final checks ran **serially**, with checkout-local tools, caches, targets and
scratch data, using `sh scripts/validate-local.sh`:

| Check | Result |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo test --workspace --offline --locked` | PASS: 20 core + 21 DOCX + 15 desktop = 56 tests; no failures |
| `cargo clippy --workspace --all-targets --offline --locked -- -D warnings` | PASS |
| `cargo build -p folio-desktop --release --target aarch64-apple-darwin --offline --locked` | PASS, via packaging script |
| Plist lint and `lipo -verify_arch arm64` / `file` | PASS: valid plist and Mach-O arm64 executable |
| Independent XML assertions and macOS `textutil` extraction of GUI exports | PASS; separate from rendering interoperability |

The lockfile was regenerated from the prepared offline registry and compared
byte-for-byte with the integrated lockfile: unchanged. No interface/dependency
mismatch required a dependency change. Imported Noto Serif names now select the
bundled Serif face in both the canvas and ribbon; the regression test verifies
the displayed face after DOCX save/reopen. Bundle rebuilds stage and replace the
executable inode rather than rewriting a previously launched Mach-O in place. Version 0.1.0 and author hozaifahejaz remain.
The added integration test crosses app/core/codec/filesystem boundaries: styled
Unicode plus a page break survive save/reopen, failed protected-source saving
retains dirty state, saving a converted copy changes the destination, and later
Save retains the original source. Existing tests cover styles/layout/empty
paragraphs, package/resource failures, atomic replacement, hard-link protection,
IME events, search and numeric focus, and editing/layout geometry.

CI is configured in `.github/workflows/ci.yml` for native Windows, macOS and Linux
fmt, workspace tests, clippy and release desktop builds. **CI results are pending**;
no remote jobs were triggered by this assignment. Native Windows/Linux GUI,
filesystem FFI, clipboard, IME and accessibility behavior remain untested.

## Actual macOS bundle smoke test

The actual `dist/Folio.app` release bundle was launched through native automation;
its bundle ID is `io.github.hozaifahejaz.folio`, and both version fields are 0.1.0.
The full smoke suite preceded the final narrow imported-font mapping fix. After
that fix, all combined checks passed again; the rebuilt bundle was relaunched,
its imported Noto Serif bold/italic face and selector were visually verified,
and its own styled Unicode export was saved/reopened. Other editing/file code
was unchanged by that fix.

| Area | Observed result |
| --- | --- |
| Typing and Unicode | Typed ASCII; pasted Café, Chinese, combining e + acute, ZWJ family emoji and Urdu. Model/AX text and exported text retained the exact Unicode sequences. Synthetic native `typeText` dropped some non-ASCII characters, so Unicode checks used clipboard paste and a real OS dead key. |
| Native composition | Option-E displayed a transient preedit overlay without dirtying the saved document; E committed é, and one Undo restored saved content. Escape/Backspace during dead-key preedit produced a spacing acute accent on this host; cancellation semantics are not certified. |
| Imported font mapping | Final build opened a named Noto Serif fixture with a matching selector and visible bold/italic Serif glyphs. Its edited Unicode export retained `Noto Serif`, bold and italic on save/reopen. |
| Selection/formatting | Command-A and Shift navigation selected text; bold, center and italic changes were visibly rendered. Font size accepted multi-digit 18 + Enter without inserting digits into the canvas. |
| Multiple pages | Self-authored 80-paragraph fixture paginated to six pages. Last-page text plus Command-Enter produced a seventh page; whole-document italic selection spanned pages. Save/reopen retained seven pages and the last-page edits. |
| Undo/redo | Formatting Undo/Redo worked. Deleting a selection across an explicit break reduced two pages to one; Undo restored both pages/text. |
| Find/replace | Command-F focused search; typing search left document text intact. Replace all changed “Folio smoke test” to “Folio bundle test”; zero-match search reported zero replacements. |
| Save/reopen | Native DOCX save sheet wrote styled Unicode; reopen was clean with no warnings. Export XML independently retained bold/center, multi-page italic and explicit break properties. Final rebuilt bundle reopened “Final bundle 0.1.0 — Café 你好”. |
| Unsaved decisions | New, Open and close prompted. Cancel preserved dirty content; Discard started a clean New document; Save changes saved successfully before New. Native Save As cancellation retained the document/path/dirty state. |
| Failed save | Temporarily replaced the disposable export destination with a directory. Save showed “Is a directory”, kept the dirty document and path, and removed its temporary file. Restoring the destination allowed retry through Save changes. |
| Unsupported import | Self-authored fixture showed 15 visible warnings, including omitted tables/drawing/fields/sections and underline approximation. Save defaulted to `unsupported-converted.docx`. |
| Protected source | After native replacement confirmation, app rejected original source, hard-link and symlink destinations. All three retained the original SHA256. Separate converted copy saved successfully; later ordinary Save used that copy. Reopen retained supported text without import warnings. |
| Native automation/accessibility | Canvas name/value, toolbar buttons, labels and dialogs were exposed. AX clicks on some text fields did not move focus reliably; pointer input and native Go To sheets completed the checks. VoiceOver and per-character screen-reader navigation were not tested. |

Full Japanese/Chinese IME candidate selection and cancellation were **not tested**.
Control-Space did not expose an alternate input source in the automation session;
no input methods were installed or system settings changed. Headless Japanese
preedit/commit tests are not equivalent to an OS candidate-window test. Native
clipboard paste was tested; OS copy/cut interchange beyond headless events was
not independently exercised. Clean app quit was observed before final rebuild.

## External interoperability and fidelity limits

Microsoft Word and LibreOffice are absent from `/Applications`; **neither external
application was available for interoperability/rendering validation**. No claims
about their pagination or visual fidelity are based on Folio reopening its own files.
macOS `textutil -convert txt -stdout` independently read `gui-export.docx`,
`multipage.docx`, `unsupported-converted.docx`, `final-bundle.docx` and
`serif-export.docx`. It recovered
Unicode text, the last-page edits and converted-copy text. This confirms independent
text readability only, not Word/LibreOffice rendering, schema conformance or
preservation of every formatting feature.

Known limits remain: generic/imported font metadata may use Sans fallback;
only bundled Sans/Serif faces are selectable. The family emoji rendered as
separate monochrome glyphs; Urdu did not have correct contextual shaping/order.
Full bidi/complex-script shaping, exact Word line breaking, advanced tabs,
widow/orphan/keep rules, oversized-font overflow and inline IME reflow are deferred.
Pagination is Folio's own glyph-row layout. There is no printing/PDF pipeline,
rich clipboard interchange or accessibility audit. Selecting text and replacing
it with a page break currently takes two history steps. Long non-ASCII paragraph
validation and snapshot history can be expensive. Warned unknown parts are omitted,
not round-tripped; converted-copy saving makes the loss explicit.

The supported semantic roundtrip, source-retention and combined-check acceptance
criteria pass locally within this subset. Other OS GUI behavior, full native IME,
assistive technology and Word/LibreOffice fidelity remain outstanding. This is a
**testable desktop foundation**, not a fully accepted cross-platform Word replacement.

## Reproducing the evidence

Prepare the toolchain/cache as in the root README, then run
`sh scripts/validate-local.sh`. Logs are in ignored `build/validation/`.
`python3 scripts/prepare-smoke-fixtures.py` creates self-authored input fixtures in
ignored `build/smoke/` (run before the GUI suite; rerunning replaces input fixtures).
Launch `open "$PWD/dist/Folio.app"`, perform the above GUI steps, then run
`python3 scripts/verify-smoke-exports.py` to check the expected completed exports.
The verifier expects the exact smoke edits described above; it does not drive GUI
input or replace the codec's semantic regression tests. Ignored artifacts are
local evidence, not delivered source artifacts or signed installation packages.
