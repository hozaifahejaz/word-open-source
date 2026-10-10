# Folio desktop 0.1.0

Native Rust egui/eframe editor with Folio branding, File/Home/Layout/View ribbon,
quick-access actions, editable paginated paper, and page/word/character/zoom status.
Original line icons use compact controls for frequent actions, retaining accessible
names, tooltips and keyboard focus. The minimalist light theme combines a serif
wordmark, muted indigo accents, borderless tools and a softly elevated paper canvas.
Home consolidates clipboard, text and alignment tools, highlights Save, and wraps
controls in narrow windows. Import warnings include a direct converted-copy action.
The document canvas is a custom rich-text widget, not a plain-text widget plus
preview. Every paragraph is shaped once into styled egui glyph rows. The composed
layout supplies painting, grapheme caret stops, pointer hits, selection rectangles,
visual keyboard movement, and automatic pagination. A visual row hint preserves
caret affinity at wrapped lines and page boundaries.

## Editing

- Type, paste, select with drag or Shift navigation, and double-click words.
- Arrow keys, Home/End, Page Up/Down, word navigation, Backspace/Delete, paragraphs,
  explicit page breaks, and cross-page editing use document-core commands.
- Bold, italic, underline, Sans/Serif, size and color work on a selection or pending
  typing style. Paragraph alignment, spacing and line spacing update the model.
- Clear formatting resets selected text to the default text style in one undo step;
  at a caret it resets future typing. Paragraph formatting stays as configured.
- Line spacing offers multiples, Exactly and At least, with a point-height field
  for the latter two. Imported exact/minimum values display their actual heights.
- Paper, orientation, four margins, zoom and fit width update the editable canvas.
- Undo/redo restore model content and selection. Replacing a selection with a
  page break is one atomic history step.
- Clipboard uses native egui/eframe copy/cut/paste. IME preedit is transient and
  displayed at the caret; commit replaces the selection as one undoable edit.
  Home provides Cut/Copy/Paste buttons; cut and paste are single undoable edits.
- The status bar counts document and selected words and characters. Words are
  whitespace-separated tokens; characters are Unicode graphemes including spaces
  and tabs, excluding structural paragraph and page breaks.
- View adds Focus mode for distraction-free writing, a light/dark appearance
  switch, and a document info panel. Focus mode hides the ribbon and status bar;
  press Escape or Command/Ctrl + Shift + F to return. Appearance and panel
  choices apply to the current session and do not change document content.
- Find/replace is literal and case-sensitive; matches do not cross paragraphs.
- View → AI connection enables an authenticated local MCP connection to the
  current document. `folio-desktop --mcp` serves an independent background
  document through stdio. Both expose 14 read/edit/format/file tools with shared
  core validation and undo history. See [MCP setup and tools](../../docs/MCP.md).
  Command/Ctrl-F focuses a stable search field without changing the document
  selection. Search/replacement input never routes into the canvas. Numeric ribbon
  fields retain text focus, and typed values apply at commit rather than per digit.

Grapheme caret boundaries are computed in one pass per changed paragraph.
For the manual cold-layout benchmark, run
`cargo test -p folio-desktop --offline --locked unicode_cold_layout_benchmark -- --ignored --nocapture`.

Command on macOS, Ctrl on Windows/Linux: N New, O Open, S Save, Shift-S Save As,
Z Undo, Shift-Z/Y Redo, B/I/U formatting, A Select All, F/H Find/Replace,
Enter page break, Q/W close. On macOS, Command-Left/Right moves to visual line
edges; Command-Up/Down moves to document edges. Option-Left/Right moves by word.
Shift extends navigation. Numeric fields and search use their own egui editing
shortcuts rather than document commands.

Noto Sans and Noto Serif regular/bold/italic/bold-italic are bundled under the
SIL Open Font License in `assets/fonts/LICENSE`. On macOS, an installed Arial
Unicode font is read for additional script coverage; it is not redistributed.
Imported font names outside the two offered families use the Sans display face
while retaining their original model metadata until explicitly reformatted.

## Files and failure behavior

New/Open/Save/Save As use native rfd dialogs and the established `import_docx` /
`export_docx` stream APIs. A cancelled dialog changes no document, path or saved
baseline. New, Open, and close prompt for dirty content. Failed saves keep the
old destination, document, dirty state, and pending operation; errors remain
visible for retry or cancellation.

Import warnings remain visible and permanently protect the imported source.
Saving it requires a converted copy at a different path, checked in both the UI
and save handler. Canonical paths and filesystem identity reject symlink and
hard-link aliases. Subsequent ordinary Save targets the converted copy.

Supported exports encode into memory before touching a destination, then write,
flush and sync a uniquely created sibling temporary file. POSIX rename / Windows
MoveFileExW replaces the destination atomically without deleting it first. Failed
writes/replacements clean the temporary file. The editor marks saved only after
replacement succeeds. Export warnings stop the save to avoid silent content loss.

The integrated codec imports and exports the supported subset described in
[the codec guide](../../crates/docx/README.md). Supported semantics are tested
through save/reopen; unsupported imports retain visible conversion warnings.

## Scoped checks and launch

From the workspace root, after the [root setup steps](../../README.md), use
checkout-local Rust and caches (no shell-profile changes):

```sh
export CARGO_HOME="$PWD/.tools/cargo"
export RUSTUP_HOME="$PWD/.tools/rustup"
export CARGO_TARGET_DIR="$PWD/.tools/target"
export TMPDIR="$PWD/.tools/tmp"
export TMP="$TMPDIR" TEMP="$TMPDIR" PATH="$CARGO_HOME/bin:$PATH"
cargo test -p folio-desktop --offline --locked
cargo clippy -p folio-desktop --all-targets --offline --locked --no-deps -- -D warnings
cargo build -p folio-desktop --offline --locked
.tools/target/debug/folio-desktop
```

Workspace formatting is checked with `cargo fmt --check`.
Tests exercise shortcut routing, core edits/undo across pages, graphemes,
pagination/hit/caret agreement including wrap affinity, zoom, selection,
clipboard events, IME transactions, search focus/isolation, numeric focus/commit,
unsaved decisions, save failure state, atomic replacement and warning aliases.

## Deferred and untested

Tables, images, lists, headers/footers, comments, tracked changes, multiple
sections, printing, spellcheck, collaboration, and non-DOCX formats have no UI
controls. Full bidi/complex-script shaping, exact Word line-breaking fidelity,
advanced tab stops and paragraph flow rules, oversized-font overflow, rich
clipboard interchange, screen-reader per-character navigation, and inline IME
reflow are deferred. The word count uses whitespace-separated tokens.
The boundary cache accelerates repeated layout; the core's boundary validation
can still be expensive for very long non-ASCII paragraphs.

For the actual unsigned Apple Silicon bundle and GUI acceptance evidence, see
[acceptance results](../../docs/ACCEPTANCE.md). Reproduce with
`sh scripts/package-macos.sh` and `open "$PWD/dist/Folio.app"` from the root.
Windows/Linux windowing, native dialogs, IME and Windows replacement/identity FFI
need native-host testing; Mac results do not establish their behavior.

## Writing workbench

Command/Ctrl-K opens Commands in any editing view. Type to filter named actions,
use Up/Down and Enter to execute, or Escape to cancel. Unavailable Undo/Redo,
selection-only actions and the unapplied format painter are disabled. The View
ribbon also provides Commands, Writing progress, Snippets, Keyboard shortcuts
and Go to. Go to accepts 1-based page, wrapped visual line or paragraph numbers;
invalid input leaves selection untouched. Blank pages scroll into view without
inventing a caret. Soft-wrap navigation retains the visual row.

Home's Writing tools menu inserts a local date/time, captures/applies a format
painter, and clears paragraph formatting. Date/time uses the system `date` utility
when available, with an explicitly labeled UTC fallback. Insertion replaces the
selection as one undoable edit. Capture the source style, select the target text,
then choose Apply format painter; it applies the entire text style once while
preserving paragraph settings. Clear paragraph formatting restores alignment and
spacing defaults without changing text or its run styles.

Writing progress shows the current document's whitespace word count, an editable
0–1,000,000 word goal (0 disables), and reading/speaking estimates at 200/130 words
per minute. Start/Pause/Reset controls a session-only timer and net word change,
including deletions and Undo. Goals and user snippets persist privately in the
existing local workspace; there is no network synchronization. Three built-in
snippets and up to 32 user snippets insert at the selection. Titles contain 1–80
Unicode characters after trimming; text must be nonempty and at most 65,536 UTF-8
bytes. Invalid stored preferences report an error and retain their original bytes.
Schema-1 states default to no goal or user snippets; writes use schema 2.

Document info and `folio-desktop --version` show the application version and
build revision. `scripts/package-macos.sh` embeds the current Git revision;
ordinary development builds show `development`. Fit page width accounts for
visible Document info and Writing progress side panels.

File → Templates provides Blank, Letter, Meeting notes and Project brief. Useful
nonblank templates start unsaved and share New's unsaved-changes prompt. Drop one
DOCX to open through the same prompt; multiple/unsupported drops show an error.
File and Commands also offer Duplicate, Export text and Export selection. Copies
use a distinct explicit `.docx`/`.txt` destination, confirm replacement, and preserve
the active document's path, selection, history and unsaved state. Plain text is
UTF-8 with newlines between paragraphs and form feeds for explicit page breaks.

View → Read-only mode pauses editing and auto-save, keeps navigation/Copy/export,
and continues recovery for existing unsaved edits. Save uses Save As to a distinct
file; aliases of the active source are blocked. The mode resets on successful
New/Open/template, and is an editing convenience rather than encryption or a
filesystem security permission.
