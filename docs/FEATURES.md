# Feature inventory

This is a scoped Folio inventory, not exhaustive Word parity. **Supported** means
implemented within the stated scope; **Partial** means limited support; **Deferred**
means no implementation. The integrated 0.1.0 desktop and DOCX subset are implemented.
Implementation support is distinct from native-host verification; see
[acceptance evidence](ACCEPTANCE.md). This is a desktop foundation, not Word parity.

References reviewed **2026-10-09**. Word features vary by platform, subscription
and build; published help scopes are recorded below without invented build numbers.

## Reference baseline

- [Create a document](https://support.microsoft.com/en-us/word/training/create-a-document-in-word):
  Microsoft 365, Word 2024/2021; explicitly Windows desktop.
- [Design and edit](https://support.microsoft.com/en-us/word/training/design-and-edit-in-word):
  Microsoft 365 and Word 2024/2021/2019/2016 help scopes for formatting/proofing.
- [Mac quick start](https://download.microsoft.com/download/6/3/4/634e576c-136a-4638-b750-2f9d1b90a573/Word%20for%20MAC%20Quick%20Start%20Guide.pdf):
  Microsoft 365 for macOS; no exact build specified. Folio needs separate Mac tests.
- [Desktop/web comparison](https://support.microsoft.com/en-au/word/word-features-comparison-word-for-the-web-vs-desktop):
  current broad capability overview, not an OS-specific build matrix.
- [Web service description](https://learn.microsoft.com/en-us/office365/servicedescriptions/office-online-service-description/word-online):
  browser Microsoft 365 scope, distinct from desktop.
- [Mobile Copilot](https://support.microsoft.com/en-us/word/copilot/copilot-in-word-on-mobile-devices):
  iPad/iPhone/Android, subject to subscription and organization settings.
- [ECMA-376](https://ecma-international.org/publications-and-standards/standards/ecma-376/):
  Part 1 fifth edition December 2016 (markup), Part 2 December 2021 (packaging),
  Part 3 December 2015 (compatibility), Part 4 December 2016 (transitional features).
- [WordprocessingML structure](https://learn.microsoft.com/en-us/office/open-xml/word/structure-of-a-wordprocessingml-document):
  paragraphs/runs and related stories/parts; references ISO/IEC 29500:2016.
- [MS-DOCX](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-docx/b839fe1f-e1ca-4fa6-8c26-5954d0abbccd):
  OOXML extensions, revision 23.0 dated 2026-08-18; not implicitly supported.
- [MS-DOC](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-doc/):
  Word 97–2003 binary format, distinct from ZIP/XML DOCX.

## Current state and later work

| Family | Status | Folio boundary | Reference |
| --- | --- | --- | --- |
| Paragraphs/styled text | Supported | Bold/italic/underline/strike, exclusive super/subscript, RGB text highlight, family/size/RGB; native rich-text canvas with font fallback | Create document; ECMA Part 1 |
| Selection/editing | Supported | Grapheme-safe commands, selection, history, visual navigation; IME event support, OS verification limited | Design/edit; contracts |
| Editing toolbar/statistics | Supported | Labeled original icons, plain-text Cut/Copy/Paste, clear text formatting; document/selection whitespace-word and grapheme counts | Desktop guide |
| Find/replace | Supported | Literal paragraph-local, cross-run search; optional Unicode lowercase comparison and UAX #29 whole words; virtualized keyboard/mouse result navigation/counter; regex deferred | Design/edit; contracts |
| Case conversion/symbols | Supported | Selection-only Unicode upper/lower/title/sentence case preserving rich document structure; 19 named symbol actions insert at caret or replace selection; undo/redo | [Acceptance evidence](ACCEPTANCE.md) |
| Writing workbench | Supported | Searchable command palette, shortcut reference, 1-based page/wrapped-line/paragraph navigation, date/time insertion, one-use complete format painter, clear paragraph formatting, local goals/snippets and session timer | Desktop guide |
| Alignment/spacing | Supported | Four alignments; before/after and multiple/exact/at-least spacing; indent/tab stops deferred | Desktop/web; ECMA Part 1 |
| Page settings/breaks | Supported | One size/orientation/margin set and explicit breaks; automatic/explicit pagination implemented | Desktop/web; ECMA Part 1 |
| Local workspace | Supported | File-tab Recent Documents (12 unique paths), unavailable-entry removal, per-document caret, remembered light/dark appearance and zoom | [Acceptance evidence](ACCEPTANCE.md) |
| Auto-save/recovery | Supported | Named unprotected DOCX files auto-save after five idle seconds; dirty recovery checkpoints after two seconds; explicit startup Restore/Discard retains import guards | [Acceptance evidence](ACCEPTANCE.md) |
| Advanced workspace | Partial | Original template gallery, distinct DOCX copies, UTF-8 text export, single-DOCX drag/drop and read-only editing mode; tabs/windows, rename, password protection, version history and comparison deferred | Approved feature roadmap |
| DOCX | Partial | Supported semantic subset import/export; inherited styles flattened; omissions/approximations warn | ECMA Parts 1–4; MS-DOCX |
| Tables/images | Deferred | No model/rendering; imports must warn | Create document; ECMA Part 1 |
| Styles/templates | Partial | Inherited named styles resolved on import and flattened; original Blank/Letter/Meeting Notes/Project Brief document templates; named-style editor and Word template codecs deferred | Web service; WordprocessingML |
| Lists | Deferred | No bullets, numbering definitions/hierarchy | Web service; ECMA Part 1 |
| Sections/columns | Partial | Single layout only; mixed sections/columns absent | Desktop/web; ECMA Part 1 |
| Headers/footers | Deferred | No repeating stories/page-number evaluation | WordprocessingML |
| References/fields | Deferred | No notes, TOC, citations, bibliography, captions or fields | Desktop/web; WordprocessingML |
| Reviewing | Deferred | No comments, tracking, acceptance/rejection or comparison | Mac quick start; Desktop/web |
| Collaboration | Deferred | Local editing only; no coauthoring/version history/sync | Create document; Web service |
| Proofing | Deferred | No spelling/grammar, dictionary, translation or thesaurus | Design/edit; Desktop/web |
| Mail merge | Deferred | No data sources, merge fields, envelopes/labels | Desktop/web; ECMA Part 1 |
| Printing/PDF | Deferred | No print pipeline or PDF export | Desktop/web |
| Accessibility | Partial | eframe backend enabled; custom editing semantics/screen-reader checks/audit pending | Desktop/web |
| Automation/add-ins | Partial | 19 MCP tools for live/background documents; no VBA/macros/Office add-ins execution | [MCP setup](MCP.md); Desktop/web |
| Legacy/other formats | Deferred | No .doc, .docm, .dot/.dotx, RTF or ODT codec | MS-DOC; ECMA |
| Cloud/AI | Deferred | No OneDrive/accounts, online services, dictation or AI assistant | Create document; Mobile Copilot |
| Browser/mobile | Deferred | Windows/macOS/Linux native first; web/iOS/Android front ends later | Web service; Mobile Copilot |

Folio does not bundle Word branding, templates or assets. Use original fixtures
and record supported elements, approximations and rejected features explicitly.
Unknown imported content must warn rather than quietly disappear on save.
Update statuses after implementation; planned work is not supported behavior.

## Writing tools increment — 2026-10-10

The Home toolbar's Aa menu converts a non-empty selection to uppercase,
lowercase, title case or sentence case using Rust Unicode mappings. Title case
uppercases the first original cased grapheme of each Unicode-whitespace-delimited
token and lowercases the rest with full token context. Sentence case lowercases
text, then uppercases the first cased grapheme and the first after `.`, `?` or `!`
followed by optional closing quotes/brackets and whitespace. Opening punctuation
does not consume capitalization. Whitespace and punctuation stay intact. The UI-independent
`ConvertCase` core command preserves each run style, paragraph properties, empty-paragraph typing
style and explicit page break. One transaction collapses the selection at its
transformed end and supports Undo/Redo; unchanged text adds no dirty state or
history, and a caret alone leaves the document unchanged.

Symbols offers 19 named, keyboard-reachable actions: nonbreaking space/hyphen,
em dash, ellipsis, bullet, copyright, pound/euro/yen, plus/minus,
multiplication/division, four arrows, check mark, grinning face and red heart
(including its emoji variation selector). Insertion replaces the current selection
or inserts at the caret, closes the picker and returns focus to the document.
These are literal characters, including the bullet; they do not implement list
formatting. Date/time insertion is available through the writing workbench.
Custom replacements and emoji search remain **Deferred**. Native screen-reader and glyph-rendering limitations remain as
recorded in [acceptance evidence](ACCEPTANCE.md).

## Search increment — 2026-10-10

Find / Replace provides Match case (enabled by default), Whole words, a current
result counter, and a compact virtualized scrollable list that selects and reveals
matches. Tab enters the list; Up/Down move one result, Home/End reach the first/last
result, and navigation scrolls and transfers row focus while revealing the document
range. Enter activates the focused row and returns focus to the canvas. Mouse
selection remains available.

Find next wraps through the current query's matches; Replace accepts only a
selection whose range is a current match, and Replace all is one undoable command.
Changing the query/options or editing the document refreshes the results. Empty
queries disable replacement and remain rejected by the core search commands.

Insensitive matching compares Unicode scalar lowercase mappings and maps results
back to original grapheme boundaries, including lowercase expansions. It does not
perform locale-specific case folding or Unicode normalization. Whole words uses
UAX #29 boundaries in the original paragraph. Paragraph/page breaks remain search
barriers. Existing core `Document::find`, `Command::ReplaceAll`, and omitted MCP search
options retain literal case-sensitive behavior. Regex search remains **Deferred**.

## Workspace/recovery increment — 2026-10-10

Local versioned state records preferences, recent paths and caret positions; it is
not cloud sync or document version history. Recovery stores one dirty document
snapshot, including selection, warnings and protected source. Restore is explicit
and remains unsaved until explicit Save or a fresh edit re-enables normal idle
auto-save for eligible named files. Successful save, explicit discard or Undo back
to the saved baseline clears this session's valid snapshot; cleanup failures are
reported and retried on confirmed close without discarding the journal.
Failed disk operations preserve recovery and report an error. Invalid/future state
is reported and preserved rather than silently replaced. Idle auto-save does not
apply to untitled documents or protected warned imports; those require explicit
Save/Save converted copy. Recovery cannot retain edits made after the last completed
checkpoint, and concurrent Folio windows sharing this store are not supported.

This delivers the first approved workspace increment. All other features in the
[approved roadmap](superpowers/specs/2026-10-10-folio-feature-roadmap-design.md),
including map integration, remain deferred unless explicitly supported above.
The current verification evidence and outstanding native checks are recorded in
[acceptance results](ACCEPTANCE.md).

## Earlier 0.1.0 verification boundary

The macOS 27.0 arm64 release bundle was exercised for text/Unicode paste,
selection/formatting, automatic and explicit pages, undo/redo, literal replacement,
DOCX save/reopen, unsaved decisions, failures and protected converted-copy saving.
All 56 workspace tests and serial formatting/clippy/release checks passed locally.
CI is configured for Windows/macOS/Linux; its results remain pending. Native
Windows/Linux behavior has not been tested from this Mac. Word/LibreOffice were
unavailable; textutil independently recovered exported text but did not validate
Word rendering. OS dead-key preedit/commit worked, while full CJK candidate selection
and cancellation, VoiceOver and accessibility auditing remain unverified.
Emoji composition, Urdu/bidi shaping and exact Word pagination remain fidelity
limitations. See [the detailed acceptance report](ACCEPTANCE.md).

## Rich text formatting increment — 2026-10-10

Home provides accessible original Strike, Superscript, Subscript and Highlight
icons. Highlight offers None and seven named colors; core and DOCX formatting
support arbitrary RGB backgrounds. Script text uses 75% font size and native
top/bottom alignment. Glyph geometry, backgrounds, strike lines, pagination,
selection and caret hit testing share the composed layout. Clear formatting resets
all text properties. Pointer and keyboard activation return focus to the canvas
as with Bold; script buttons toggle one mutually exclusive enum.

DOCX supports `w:strike`, `w:vertAlign`, RGB clear run shading and named highlight
colors. Highlight overrides shading across inheritance; layered backgrounds and
unsupported patterns/theme colors warn. Workspace/recovery writes schema 2 and
reads schema 1 with default new properties, preserving unsupported schema 3+
files against save. See [codec boundaries](../crates/docx/README.md) and
[acceptance evidence](ACCEPTANCE.md).
### Document lifecycle and exports

File and Commands offer an original template gallery: Blank, Letter, Meeting
notes and Project brief. Templates use supported paragraphs and formatted runs.
Nonblank templates start unsaved. Switching uses the same Save/Discard/Cancel
prompt as New; cancelled or failed switches preserve current content and recovery.
Drop a single DOCX onto the window to use that open flow. Multiple files and other
formats show an error without changing the document.

Duplicate writes a complete supported DOCX to a distinct file. Export text writes
UTF-8 `.txt`, for the whole document or the current selection. Paragraph boundaries
are newlines; explicit page breaks are form feeds on their own lines. Reversed
selections and Unicode grapheme boundaries use the document's validated selection.
Copy/export requires an explicit replacement decision for existing destinations,
rejects active-source aliases (including hard links), and uses atomic replacement.
It preserves the active file, dirty state, selection, history and import protections.

View → Read-only mode (also Commands) disables document mutations, formatting,
paste, undo/redo and page layout, while allowing selection, navigation, copying,
New/Open and exports. A Read-only badge and accessible canvas label identify the
mode. Auto-save pauses; recovery still checkpoints preexisting unsaved edits.
Save/Save changes uses Save As; a distinct destination is allowed and overwriting
the active source or an alias is blocked. Successful New/Open/template resets the
mode. This is an editing mode, with no encryption or filesystem permission claim.

## MCP editing parity

The 19 local MCP tools include Unicode case and whole-word search controls,
structure-preserving case conversion, full text and paragraph formatting, complete
page-layout read/edit, template discovery/new documents, distinct DOCX copies and
UTF-8 text export. RGB highlight omission preserves it and explicit null clears it;
nested input fields are strict. Read-only and interaction dialogs protect mutation
routes, with navigation/read/export available in read-only mode. Background SDK
smoke uses disposable temporary DOCX/TXT files and verifies saved formatting on
reopen; the live smoke only inserts, finds and undoes text in a blank test window.
