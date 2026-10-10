# Folio 0.1.0 acceptance results

Recorded 2026-10-09 for the integrated desktop foundation by hozaifahejaz.
Available host: Apple Silicon arm64, macOS **27.0 (26A428)**, Apple clang
21.0.0, Rust **1.90.0**. Windows and Linux were not executed locally.
This report records scoped evidence, not full Word parity or a release certification.

## Rich text formatting increment — 2026-10-10

Focused RED/GREEN tests establish mixed-run grapheme/style preservation with
Undo/Redo, old JSON defaults and snake_case vertical alignment, highlight clear,
transactional style validation, schema-1 migration/write-2 and schema-3 byte
preservation. DOCX regressions cover strike/script/RGB shading round trips,
highlight precedence in both XML child orders, inherited layers and highlight
clearing, solid foreground color, and unsupported pattern/theme diagnostics.

Desktop tests inspect real egui glyph positions and painted background meshes,
75% script font sizes, top/bottom offsets, all-script wrapped rows, justification,
and valid selection/caret/hit geometry at 0.75, 1.0 and 1.5 zoom. AccessKit node
bounds drive pointer activation of the four new original icon controls and
palette None; keyboard traversal activates each control and palette Yellow,
returns canvas focus and does not insert the activation Enter. Canvas Tab retains
its existing literal-tab behavior; keyboard traversal starts with ribbon focus.
Justified trailing whitespace may share a caret x coordinate, so hit checks
compare the actual caret position when multiple valid boundaries coincide.

`sh scripts/validate-local.sh` passed: `cargo fmt --check`, 171 workspace tests
(35 core commands, 6 recovery serialization, 104 desktop, 26 DOCX; 0 failures,
one intentionally ignored manual benchmark), clippy with warnings denied, and
local unsigned Apple Silicon release packaging. All commands used the checkout's
Rust 1.90.0 and offline locked dependency cache. `git diff --check` also passed.

The scope includes native egui layout and automated input/accessibility checks;
it does not claim a new OS screen-reader audit, Windows/Linux GUI run, or visual
comparison in Word/LibreOffice. Final local validation and detailed RED/GREEN
outputs are recorded in [Task 2 report](../.superpowers/sdd/2026-10-10-polished-editing/task-2-report.md).

## Writing tools increment — 2026-10-10

Task 2 adds the selection-only Aa case menu and the labeled Symbols picker.
Tests use the real egui ribbon/canvas, pointer events, keyboard events and
accessibility output. No native bundle smoke, release packaging, Windows/Linux
GUI run, or screen-reader audit was performed for this increment.

| Automated check | Observed result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS, exit 0 |
| `cargo test --workspace --offline --locked` | PASS: 29 core commands + 5 core recovery + 92 desktop + 23 DOCX = 149 passed; 0 failed; one manual desktop benchmark ignored |
| `cargo clippy --workspace --all-targets --offline --locked -- -D warnings` | PASS, exit 0 |

Four new integration tests cover all four transformations with Unicode case
expansion (`ß`, `İ`), original-grapheme title casing (`İETA` → `İeta`), combining
marks, contextual Greek lowercase, emoji, punctuation, tabs/newlines/nonbreaking
spaces, and sentence punctuation without following whitespace. Reversed selection
conversion collapses at its new byte end; Undo restores text, selection and clean
state, and Redo reapplies it. Caret-only conversion does not mutate text or dirty
state. The 19 symbol sequences are checked individually, both at a caret and over
a reversed selection, including `U+2764 U+FE0F` red heart; each returns canvas focus,
requests reveal, dismisses the picker and supports Undo/Redo. Accessibility output
exposes Change case, its four actions, Symbols and all 19 spoken insertion names.
Tab reaches the Symbols control and Enter opens its picker. Tab then reaches
every symbol action; Enter inserts the red heart without inserting an extra
paragraph, and subsequent typing returns to the canvas.

The initial tests failed on absent Aa/Symbols controls and accessible names before
implementation. A further title-case regression failed because lowercasing the
first original `İ` before uppercasing changed its exact sequence; title case now
uppercases that original grapheme directly. Final verification output is clean.
Log: `build/validation/writing-tools-task-2-tests.log` (ignored local artifact).
Date/time insertion, custom replacements and emoji search are deferred. Existing
emoji glyph fidelity and native accessibility limits below remain outstanding.

## Whole-branch preservation and keyboard fixes — 2026-10-10

The Aa actions now execute the UI-independent `Command::ConvertCase`. Core
regressions compare exact documents across styled runs, two paragraphs with
distinct paragraph/default styles, an empty paragraph and an explicit page break.
Uppercase/lowercase expansions retain their source styles; Undo restores the
original reversed selection and clean document, and Redo restores the transformed
caret. An already-uppercase selection preserves all content/metadata and adds no
dirty state or history.

A real egui test enters a virtualized 200-result list with Tab, moves with
Up/Down and Home/End, and reaches result 200 beyond the initial viewport.
Navigation selects the original range, updates the counter, requests canvas
reveal, scrolls the destination into view and transfers row focus. Enter activates
that row and returns focus to the canvas without modifying document text.
Existing mouse-selection and viewport-only rendering tests remain passing.

The regressions were observed failing before implementation: the original case
path flattened styles and removed the page break, including unchanged text;
keyboard arrows left the selection at the document end instead of the next row.
Final verification passed: `cargo test --workspace --offline --locked` ran 152
tests (31 core commands, 5 recovery, 93 desktop and 23 DOCX), with zero failures
and one existing manual cold-layout benchmark ignored. `cargo fmt --all -- --check`
and `cargo clippy --workspace --all-targets --offline --locked -- -D warnings` both
exited 0. Detailed verification logs are recorded in
`build/validation/whole-branch-fix-*.log` (ignored local artifacts). Native OS and
screen-reader behavior was not reverified; regex, date/time and the broader
backlog remain deferred.

## Search increment — 2026-10-10

Task 1 adds Match case, Unicode whole words, the visible current-result counter,
a compact selectable result list, and option-aware Find next / Replace / Replace
all. Verification uses the real egui ribbon and document canvas with pointer and
keyboard events, not mocked widgets. No native bundle smoke, Windows/Linux GUI
run, release packaging, or external Word/LibreOffice verification was performed
for this increment; earlier native evidence below remains historical.

| Automated check | Observed result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS, exit 0 |
| `cargo test --workspace --offline --locked` | PASS: 29 core commands + 5 core recovery + 88 desktop + 23 DOCX = 145 passed; 0 failed; one manual desktop benchmark ignored |
| `cargo clippy --workspace --all-targets --offline --locked -- -D warnings` | PASS, exit 0 |

Focused tests cover legacy literal case-sensitive compatibility, Unicode lowercase
expansion (`İ` ↔ `i` + combining dot), rejected partial expansions/graphemes,
UAX #29 word boundaries around accented letters, combining marks, digits,
apostrophes and underscores, cross-run matching and paragraph/page barriers.
Option-aware replacement covers original byte offsets, multiline replacements,
single-step Undo, empty/invalid-input atomicity, and preserved selection/history.
Real egui checks toggle both options, change the focused query, click a result
and request canvas reveal, wrap Find next, reject a selected substring excluded
by Whole words, replace a reversed uppercase match, refresh after Replace all,
and preserve the protected-source path. Existing workspace/source-protection
regressions pass in the full suite. At this increment MCP used the original literal case-sensitive command; the
later MCP parity increment exposes the same options with compatible defaults. Regex search is deferred.

An initial full test run passed with one test-helper `unused_must_use` warning;
the initial clippy run rejected that warning. Explicitly consuming the egui pass
output resolved it; the final full suite and clippy run above have no warnings.
Logs: `build/validation/search-task-1-tests.log` and
`build/validation/search-task-1-review-1-tests.log` (ignored local artifacts).

Round 1 review regression checks also prove that rejecting the candidate at 1..4
in `ba a a` does not suppress the valid whole-word `a a` range at 3..6;
option-aware Replace all produces `ba XX` and Undo restores the original.
Accepted matches remain non-overlapping. Both Match case settings are covered.
The result list now virtualizes rows and caches each visible paragraph preview
within the frame. A real egui accessibility-output check bounds constructed
result widgets for 200 matches to viewport rows, then scrolls to result 200 and
verifies selection, reveal, current counter, and unchanged document/dirty state.
The final counts above include these three review regression tests.

## Workspace/recovery increment — 2026-10-10

Host: macOS **27.0 (26A428)**, arm64, Rust **1.90.0**
(`1159e78c4 2025-09-14`). Checkout-local Cargo/Rustup, target and scratch directories
were used. Final formatting, tests, clippy and release packaging cover the workspace
increment through recovery fix `3eef71d`. Native smoke below remains through
`61fc4c2`; no additional native smoke followed the fixes. Earlier sections remain
historical evidence.

| Automated check | Observed result |
| --- | --- |
| `cargo fmt --all -- --check` | PASS, exit 0, no output |
| `cargo test --workspace --offline --locked` | PASS: 22 core + 5 core recovery integration + 23 DOCX + 85 desktop = 135 passed; 0 failed; one manual desktop benchmark ignored |
| `cargo clippy --workspace --all-targets --offline --locked -- -D warnings` | PASS after recovery fix `3eef71d` |
| `sh scripts/package-macos.sh` | PASS after `3eef71d`: release build, plist lint, arm64 Mach-O verification; unsigned local bundle rebuilt |

The initial clippy attempt reported four `collapsible_if` findings in `main.rs`;
`add8019` resolved them. The controller reran all three code checks after the
subsequent recovery fix `3eef71d`:
formatting exited 0, tests passed with the counts above, and clippy exited 0.
The controller also reran `sh scripts/package-macos.sh` after `3eef71d`; release
build, plist validation and arm64 Mach-O verification passed. This rebuilt bundle
was not subjected to additional native smoke.
The initial task logs are `build/validation/workspace-recovery-{tests,clippy,package}.log`
(ignored local artifacts, recorded through `61fc4c2`); their clippy failure is
superseded by the final rerun, and the package log predates the latest release rebuild after `3eef71d`.
No Windows/Linux native checks or remote CI results are implied.

The rebuilt `dist/Folio.app` was exercised with native UI automation, accessibility
state and screenshots using disposable writing/fixtures:

| Native case | Observed result |
| --- | --- |
| Fresh state | Blank clean document, light appearance, 100% zoom and empty Recent Documents message |
| Appearance/zoom | Dark appearance displayed readable light controls against dark chrome and white paper; Fit page width displayed 139%. Normal close/relaunch retained 139% and the Light appearance action indicating dark mode. Later smoke ended in light mode. |
| Untitled crash recovery | Native paste created a dirty draft; recovery file existed. After SIGKILL and relaunch, explicit recovery prompt disabled editing. Restore returned exact draft text with Unsaved indicator. |
| Save and auto-save | Native Save wrote a DOCX and cleared `recovery.json`; subsequent edit became clean/Saved after idle, without invoking Save. |
| Recent open/missing path | Saved file appeared once; picker reopened exact saved/auto-saved text. Temporarily renaming the disposable file showed a disabled Unavailable entry with an active Remove control. File was restored afterward. |
| Warned-import recovery | Existing unsupported fixture displayed 15 warnings and protected-source banner. Editing left source SHA256 unchanged after idle; checkpoint contained all 15 warnings and protected source. SIGKILL/relaunch/Restore retained dirty text and protected-source banner. |
| Explicit discard | Relaunch of the remaining warned-import checkpoint offered recovery; Discard recovery returned blank clean editor and Recovery discarded notice. |

One initial batched Restore/Save interaction produced a CUA “app changed”/“App
quit” error; cause remains unconfirmed. Repeating Restore as its own observed step
succeeded for both untitled and warned-import documents. This does not certify the
initial failure as an application defect or resolve it conclusively.

Native resizing attempts did not change window geometry, so a **700×500 native
window, twelve-row popup and bottom-edge popup were not manually verified**. The
passing desktop UI integration test renders twelve entries at 700×500 and verifies
scrolling plus keyboard reachability of all 24 controls. Task 7's deferred Minor
finding remains: its 44pt minimum scroll height can exceed remaining space if the
popup starts fewer than 44pt from the viewport bottom. The ordinary popup was
visible and legible; that observation does not clear this edge case.

Per-document caret restart, failed-save recovery retention, corrupt/future-state
preservation and close-on-write-failure were verified by automated injected-store
cases, not independently repeated through native dialogs in this smoke. Native
Windows/Linux filesystem/UTF-16 paths, full screen-reader interaction and external
Word/LibreOffice interoperability remain outstanding. This increment does not
complete the full approved feature backlog.

## MCP integration — 2026-10-10

The MCP pass adds 14 provider-independent tools in live-window and background
stdio modes. The official Python MCP SDK 2.3.0 successfully initialized, listed
tools, read/edited/searched the document, and undid the edit in both modes against
the packaged macOS executable. Native inspection verified the AI connection
controls and the restored blank document. Live access was disabled after testing.
Validation passed formatting, 81 tests (22 core, 23 DOCX, 36 desktop; one manual
benchmark ignored), clippy with warnings denied, and arm64 packaging. New tests
cover protocol lifecycles, invalid arguments, Unicode-safe edits, stale-range
checks, DOCX file guards, live authentication, and access revocation.

## Editing tools and icons — 2026-10-09 follow-up

The subsequent minimalist refresh passed the same 74-test validation, clippy,
formatting and arm64 packaging. Native visual inspection confirmed the compact
Home toolbar, serif wordmark, indigo selection states and softer paper workspace.
Accessibility inspection retained action names and formatting/alignment toggle
states when their visible text labels were replaced with compact icons.

The feature pass adds Focus mode, session light/dark appearance switching, and a
right-side document info panel. macOS inspection verified Focus mode hides the
ribbon/status chrome and restores it with Escape, dark appearance preserves the
white paper canvas against a dark workspace, and the info panel reports pages,
words, characters and selection state. The shortcuts are Command/Ctrl + Shift +
F, D and I respectively.

`sh scripts/validate-local.sh` passed formatting, 74 tests (22 core, 23 DOCX,
29 desktop; one manual benchmark ignored), clippy with warnings denied and arm64
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

## Writing workbench increment — 2026-10-10

The desktop adds a bounded searchable Commands palette (Command/Ctrl-K), View
shortcuts/navigation/snippets/progress controls, and Home writing tools. Focused
RED/GREEN evidence includes invalid stored writing-goal refusal without replacing
original bytes, and successful nonmodal palette execution restoring canvas focus.
Egui tests verify query typing and formatting-shortcut isolation, Up/Down/Enter
routing, disabled Undo/painter behavior, Escape cancellation and focus restoration,
accessible View controls, and actual pointer activation of session Start/Pause/Reset
without document changes. Navigation tests cover invalid/Unicode input, valid
wrapped grapheme stops with visual-line hints, paragraph numbers, and empty-page
scrolling without moving the selection. Rich format painting copies all style fields
in one undoable edit while retaining paragraph properties; clearing paragraph
formatting preserves runs. Date/time and snippet insertion replace the selection
with one Undo step. UTC fallback calendar and label, Unicode snippet-title/UTF-8
text bounds, goal defaults/round trips, and rejected state preservation are tested.

`sh scripts/validate-local.sh` passed formatting, 184 workspace tests (35 core,
6 recovery serialization, 117 desktop, 26 DOCX; 0 failures, one ignored manual
benchmark), clippy with warnings denied, and unsigned Apple Silicon release
packaging. The packaged executable's `--version` prints Folio 0.1.0 plus the
embedded Git revision. Native window smoke remains a separate release step;
these egui/CLI results do not establish Windows/Linux native UI or screen-reader
speech behavior. Session timing uses monotonic wall time and is intentionally
session-only. Reading/speaking estimates use 200/130 whitespace words per minute;
snippets/goals are local workspace preferences with no network synchronization.

### Lifecycle, template and read-only checks

- From File or Commands, select each template. Blank is clean; the other three
  have formatted titles and useful editable prompts and start unsaved. Edit first,
  select another, then Cancel: current content and recovery remain. Repeat using
  Discard with a missing Open target: original content/recovery remain.
- Duplicate and export whole/selected text to temporary destinations. Reject an
  existing destination, then explicitly replace it. Confirm editor selection,
  source path, dirty status and Undo remain unchanged. Test reversed selections
  across paragraph/page breaks and a grapheme such as `é`; `.txt` uses newlines
  and form feeds. Active-source hard links/aliases must be rejected.
- Drop one DOCX while dirty: Save/Discard/Cancel appears. Drop multiple DOCX or a
  PDF: a visible error appears, and no document opens.
- Toggle View → Read-only. Typing, IME commit, paste, formatting, undo/redo and
  Layout controls must not change text or page layout. Select, arrow navigation,
  Copy, Find, New/Open, duplicate/export and distinct Save As remain usable.
  The status badge and canvas accessibility node identify Read-only. Preexisting
  dirty content checkpoints recovery but does not auto-save to the source.

Automated lifecycle regressions use only temporary files and verify model/history
preservation, aliases/overwrite/failure guards, drop dispatch, recovery and
read-only accessibility state. Native OS drag-and-drop and screen-reader speech
remain manual release checks.

## MCP editing parity

The 19 local MCP tools include Unicode case and whole-word search controls,
structure-preserving case conversion, full text and paragraph formatting, complete
page-layout read/edit, template discovery/new documents, distinct DOCX copies and
UTF-8 text export. RGB highlight omission preserves it and explicit null clears it;
nested input fields are strict. Read-only and interaction dialogs protect mutation
routes, with navigation/read/export available in read-only mode. Background SDK
smoke uses disposable temporary DOCX/TXT files and verifies saved formatting on
reopen; the live smoke only inserts, finds and undoes text in a blank test window.

Task 5 verification (2026-10-10): `sh scripts/validate-local.sh` passes formatting,
196 workspace tests (35 core commands, 6 recovery, 129 desktop, 26 DOCX), clippy
with warnings denied, and unsigned macOS packaging. One manual cold-layout
benchmark remains intentionally ignored. Official Python MCP SDK 2.3.0 background
smoke passes against the fresh packaged executable, including DOCX formatting
save/reopen. Live-window/native release smoke remains a separate release check;
no user documents were exported or sent during this pass.

## Polished editing release verification — 2026-10-10

Independent task and whole-pass reviews approved the implementation after fixes
for empty-script paragraph height, hidden focus-mode sidebar sizing, read-only
autosave resumption, restored-document autosave eligibility, and absolute MCP
copy/export paths. No outstanding code findings remained in those reviews.

Final `sh scripts/validate-local.sh` passed formatting, all 199 tests (35 core
commands, 6 recovery serialization, 132 desktop, 26 DOCX), warning-free Clippy
and unsigned Apple Silicon packaging. One existing manual layout benchmark was
intentionally ignored. The official Python MCP SDK 2.3.0 test passed against the
fresh executable: 19 tools, read/edit/find/undo, disposable background parity,
and DOCX formatting/page-layout save/reopen.

Native macOS checks used disposable content and the actual bundled application.
The old clean process was quit, its exit verified, and the fresh product build
`bf86e51fc8b7` confirmed in Document info and `--version`. Home exposed the new
formatting controls; Strike, Superscript and Yellow highlight visibly rendered.
Command-K filtered and executed commands without inserting query text; writing
progress Start/Pause/Reset worked. Invalid visual-line navigation showed an
inline error; Escape returned canvas focus. Local date/time insertion and a
built-in snippet both returned editor focus and each Undo restored the blank
document. File controls displayed distinct template/duplicate/export icons.

The gallery created a formatted unsaved Meeting notes document. Cancelling its
replacement preserved its text; read-only prevented typing and Undo while keeping
the accessible canvas focusable. Native duplicate and text export preserved the
active untitled dirty document. Inspection of their disposable files confirmed
matching paragraph text and retained bold DOCX template formatting. The test
document was cleared and the app returned to a clean blank state. An unexpected
preexisting unsaved draft was saved to a distinct local file before relaunch,
rather than discarded. No live AI permission was enabled.

Automated egui/input tests cover format painter, preference limits, Unicode
wrapped navigation, read-only IME, drop dispatch, cancellation, failed writes and
source alias protection. Real OS drag-and-drop, VoiceOver speech, Windows/Linux
GUI, and Word/LibreOffice visual interoperability remain unverified. The broader
roadmap still contains substantial deferred functionality; this release does not
claim every requested feature is implemented.
