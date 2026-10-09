# Integration contracts

## Boundaries and model

`document-core` owns public document, selection, command, validation, history,
and import warning types. It depends only on unicode-segmentation and performs
no filesystem, networking, UI or ZIP/XML operations. `folio-docx` converts
streams; `folio-desktop` owns rendering, clipboard, dialogs, files, and warnings.
Manifests contain the required zip/quick-xml and egui/eframe/rfd dependencies.
Downstream implementations can remain entirely in their crate directories.

`Document { blocks, page_layout }` has public fields for codec construction.
`Block` is `Paragraph` or `PageBreak`. Documents must start and end with a
paragraph. Empty paragraphs are valid; page breaks have no text positions.
`Paragraph` has `runs`, `style: ParagraphStyle`, and `default_style: TextStyle`
for empty-paragraph typing. `Run { text, style }` contains UTF-8 text; tabs are
allowed, CR/LF/form-feed are not (breaks are structural). `validate()` checks
structure, text, styles and layout. `normalize()` drops empty runs and merges
adjacent equal styles. Only represented model content can be preserved.

`TextStyle` stores bold/italic/underline, font family, positive
`size_half_points` (24 = 12pt), and RGB `Color`. Font names are metadata; the
core neither bundles nor resolves fonts. Theme colors, font fallback, underline
variants, and complex-script font slots are not represented.

`ParagraphStyle` stores alignment, nonnegative before/after spacing in twips
(1440/inch), and positive `LineSpacing::Multiple(100)` for single spacing,
`Exact(twips)`, or `AtLeast(twips)`. Indentation, tab stops, keep-with-next and
widow control are deferred.

`PageLayout` stores nominal unrotated `PageSize`, `Orientation`, and four
`Margins`, all dimensions in twips. `effective_size()` swaps dimensions for
landscape. Margins must leave positive content area. DOCX must convert oriented
`w:pgSz` dimensions into this convention once, avoiding double rotation. A4
and Letter constants exist. Only one layout applies; extra sections must warn.

## Positions

`Position { block, offset }` uses the block index and **UTF-8 byte offset in the
paragraph's concatenated text**, independent of run boundaries. Positions must
be extended grapheme boundaries; UTF-8 characters, combining sequences and ZWJ
emoji cannot be split. Paragraph end is valid. `Selection { anchor, focus }`
preserves direction; `ordered()` returns the half-open range.
`Document::validate_selection()` rejects invalid endpoints. Selections may span
paragraphs/page-break blocks but must end in paragraphs. Indexes are ephemeral,
not stable IDs: refresh after every command.

egui uses Unicode scalar cursor indexes. Convert with
`Paragraph::byte_from_char_index()` / `char_index_from_byte()`; a scalar cursor
inside a grapheme is rejected and must be snapped by the UI. Use
`previous_boundary()` / `next_boundary()` for whole-grapheme navigation and
backspace/delete. Visual navigation, bidirectional layout, IME composition and
shaping are UI responsibilities. A grapheme may span differently styled runs;
validation always considers the full paragraph text.

## Editor commands and state

`Editor::new(document)` validates/normalizes input, starts clean, and places a
caret at the first paragraph. `document()` is read-only; mutations use
`execute(Command)`. `set_selection()` validates without recording history.

| Command | Semantics |
| --- | --- |
| `InsertText { at, text, style }` | Insert at a caret, with optional full typing style |
| `ReplaceText { selection, text, style }` | Replace a half-open range, removing intervening paragraphs/page breaks |
| `Delete { selection }` | Replace with empty text; a whole-document deletion leaves a paragraph |
| `SplitParagraph { at }` | Insert a paragraph boundary |
| `JoinParagraph { block }` | Join the immediately following paragraph; cannot cross page-break blocks |
| `FormatRuns { selection, patch }` | Apply Some fields of `StylePatch` to selected text; collapsed selection is a no-op |
| `FormatParagraphs { selection, patch }` | Apply `ParagraphPatch` to intersected paragraphs; exclude an end paragraph at offset zero unless collapsed |
| `InsertPageBreak { at }` | Produce left paragraph, structural break, right paragraph |
| `ReplaceWithPageBreak { selection }` | Replace the range with a structural break in one transaction |
| `SetPageLayout { layout }` | Change document-wide settings |
| `ReplaceAll { needle, replacement }` | Replace original literal case-sensitive matches atomically, in one history step |
| `ReplaceAllWithOptions { needle, replacement, options }` | Replace original option-aware matches atomically, in one history step |
| `Undo` / `Redo` | Restore content and recorded selection together |

Insert/replace normalize CRLF and CR to LF and split LF into paragraphs; form
feed is rejected. Unspecified insertion style inherits from the run to the left,
the first run at paragraph start, or an empty paragraph's default style. The UI
keeps pending typing style for collapsed formatting and passes `Some(TextStyle)`
when inserting. `Some(false)` explicitly clears a boolean; `None` preserves it.
Replacement keeps the starting paragraph's properties in the first/intermediate
paragraphs and the ending paragraph's properties in the last new paragraph.
Joining keeps the starting properties. Carets snap forward when an edit creates
a new combining cluster at the insertion boundary.

`EditOutcome { selection, changed, replacements }` returns authoritative selection,
whether document content changed, and replace-all count. `CoreError` leaves all
state unchanged, including history and selection. No-op edits retain redo and
add no history. Every changed command is one transaction. Snapshot history
defaults to 100 entries; `set_history_limit()` trims it (zero disables storage).
Snapshots may cost significant memory on large documents.

`can_undo()` / `can_redo()` drive action availability. `is_dirty()` compares current
content against the last saved normalized document. `mark_saved()` sets the
baseline only after successful save. Undo back to saved content becomes clean.
`load_document()` atomically validates/loads, resets selection/history, and marks
clean, retaining the history limit. Selection, warnings, path and pending typing
style are application state, outside dirty tracking. Async saves must reconcile
their captured snapshot before marking the current document saved.

`Document::find(needle)` returns forward, literal, case-sensitive, non-overlapping
matches. Runs are transparent; paragraphs and page breaks are barriers. Empty
needles return `CoreError::EmptySearch`; matches splitting graphemes are excluded.
`Document::find_with_options(needle, SearchOptions { match_case, whole_words })`
uses the same matching/range contract. Defaults are `match_case: true` and
`whole_words: false`. Insensitive comparison uses Unicode scalar lowercase
mappings and maps complete matches back to original grapheme boundaries; partial
lowercase expansions are excluded. Whole words uses UAX #29 boundaries in the
original paragraph. Regex, Unicode normalization and locale-specific/full case
folding remain deferred. Both replace-all commands use the original matches and
do not rescan replacement text. Existing MCP callers retain `ReplaceAll` behavior.

```rust
use document_core::{Command, Editor, Position};
let mut editor = Editor::default();
editor.execute(Command::InsertText {
    at: Position::new(0, 0), text: "Hello".into(), style: None,
}).unwrap();
assert!(editor.is_dirty());
```

## DOCX API and warnings

`folio_docx::import_docx<R: Read + Seek>(reader: R) -> Result<ImportReport, DocxError>`
accepts caller-owned streams such as `File` or `Cursor<Vec<u8>>`. `ImportReport`
contains a valid `document` and `warnings: Vec<ImportWarning>`.
`export_docx<W: Write + Seek>(&Document, writer: W) -> Result<ExportReport, DocxError>`
accepts a writable stream; `ExportReport` contains warnings. The implemented codec
validates input and package limits before export. Supported semantic documents
round-trip without warnings; unrepresentable line multiples produce warnings.
See `crates/docx/README.md` for package handling and resource bounds.

`ImportWarning { code, feature, location, message }` uses `WarningCode` for
unsupported feature, approximation, missing part, invalid value, or resource
limit, and `Feature` for tables, images, styles, lists, sections, headers/footers,
fields, references, reviewing, embedded objects, or `Other(String)`. `location`
is an optional package part/XML path, not a disk path; `message` is readable.
Warnings are report metadata. If an import reports any fidelity or unsupported-
content warnings, the app must show them and require a converted-copy **Save As
to a different path**. Acknowledging loss never permits overwriting the imported
source. Retain the original source file unchanged; enforce this rule in the save
handler as well as the dialog, including paths that alias the source file.

Codec implementation must report unsupported content/approximations; unknown
parts are not implicitly roundtripped. Resolve main-document package relationships;
handle namespaces, entities and whitespace. Reject malformed input with ZIP/XML
errors or `InvalidPackage`; bound resource use and return `ResourceLimit` when
exceeded. Never execute macros, external relationships or embedded objects.
Validate before returning success. Desktop owns file handles, overwrite prompts,
temporary save files, flush/replacement, and marking saved after success.

## Downstream ownership

Task 2 implements codec/tests in `crates/docx/`; task 3 implements editing UI and
application integration in `apps/desktop/`; task 4 owns integrated platform checks
and CI. The milestone contracts need no root/core edits for these tasks. Request
extensions for unrepresented features rather than bypassing shared commands.
