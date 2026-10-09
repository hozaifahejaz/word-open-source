# Search and Writing Tools

## Goal

Ship the next visible editing increment in Folio after workspace and recovery:
make search useful for everyday document navigation, then add selection-aware
case conversion and quick insertion of common symbols. These are small,
local-first editing tools that fit the existing paginated editor and do not
require changing the DOCX document model.

The approved feature roadmap remains the product authority. This plan is one
increment, not a claim that the complete roadmap has shipped.

## Design decisions

- Preserve the existing literal, case-sensitive search behavior for current
  core callers and MCP clients. The UI exposes optional case-insensitive and
  whole-word matching; single and bulk replacement use the same options as
  navigation. Case-insensitive comparison uses Unicode lowercase mappings.
- Search stays paragraph-local, does not cross page/paragraph breaks, and only
  reports ranges aligned to valid grapheme boundaries. Whole-word matching uses
  Unicode word boundaries (UAX #29) in the original paragraph text.
- Show a compact result count and a selectable result list in the existing
  Find / Replace surface. Selecting a result moves the editor selection and
  reveals the match in the canvas. Keep virtualized results keyboard operable:
  Tab enters the list, arrow keys move through results and scroll as needed,
  Home/End reach the first/last match, and Enter activates the focused result.
- Add case conversion for selected text only: uppercase, lowercase, title case,
  and sentence case. A UI-independent undoable core command must preserve run
  styles, paragraph properties, page breaks, and all other document structure.
  An unchanged result must not create a dirty edit or an undo step.
- Add a small, keyboard-accessible symbol picker for common punctuation,
  nonbreaking characters, currency/math symbols, arrows, and representative
  emoji. Insertion replaces a non-empty selection and inserts at a caret.
- Keep the UI quiet and consistent with existing labeled icon controls. Do not
  add network calls, global autocorrect, background proofing, or a new format.
- Regular-expression search, date/time insertion, and the remaining roadmap
  features are out of scope for this increment and remain marked deferred.

## Global constraints

- Keep the core UI-independent and all content changes undoable.
- Retain Unicode-safe byte ranges and imported-document protections.
- Keep all features usable offline and preserve existing MCP semantics.
- Add meaningful tests before implementation (TDD), then run the full offline
  workspace suite and lint checks.
- Update `docs/FEATURES.md` and `docs/ACCEPTANCE.md` with implemented behavior
  and verification limits.

## Tasks

### Task 1: Search options and navigable result list

In `crates/document-core/src/model.rs`, add a search-options type and an
options-aware search method while leaving `Document::find` case-sensitive and
literal. Support optional Unicode-aware case-insensitive comparison and
whole-word matching. Use Unicode lowercase mappings to compare insensitive
queries, map each found range back to the original text, and accept only ranges
whose start and end are valid grapheme boundaries. Use Unicode word boundaries
(UAX #29) in the original text when whole-word mode is enabled. Add an
options-aware replace-all command; the existing `ReplaceAll` command must
retain its current behavior. Invalid empty queries must continue returning
`CoreError::EmptySearch`.

In `apps/desktop/src/main.rs`, extend the existing Find / Replace surface with
Match case and Whole words controls, a visible current-result `n of total`
counter, and a compact result list whose entries select and reveal their match.
Refresh results when the query/options change. Find next, Replace, and Replace
all must honor the selected options. Replace a selected result only when its
range is one of the current query's matches. Invalid/empty queries must not
mutate the document. Virtualization must preserve keyboard access: Tab enters
the result list, Up/Down moves one result and scrolls it into view, Home/End
jumps to the first/last result, and Enter activates the focused row. Preserve
existing keyboard focus and protected-import save behavior.

Tests must cover literal compatibility, case-insensitive matching (including a
Unicode lowercase expansion), UAX #29 whole-word boundaries around Unicode
letters, combining marks, and digits, grapheme safety, option-aware
replace-all/undo, and result navigation through the real egui surface.

Update the feature inventory and acceptance record. Leave regex search marked
deferred.

### Task 2: Selection case conversion and special-character picker

In `crates/document-core/src/case.rs` and the shared `Command::ConvertCase`
command, implement and test uppercase, lowercase, title-case, and sentence-case
transformations
for Unicode text. Use Rust's Unicode case mappings. Title case preserves
whitespace and uppercases the first grapheme of each Unicode-whitespace
delimited token while lowercasing the rest. Sentence case lowercases the text,
then uppercases the first non-whitespace grapheme and the first non-whitespace
grapheme after `.`, `?`, or `!` followed by whitespace. Preserve punctuation
and whitespace. Add an `Aa` toolbar menu in `apps/desktop/src/main.rs` that
applies the transform to the current non-empty selection through a
UI-independent core command. Preserve the original style of every transformed
grapheme, paragraph formatting, and explicit page-break blocks, including
when a selection spans multiple paragraphs or pages. Keep unchanged selections
unchanged. Collapse the selection at the transformed range's end and retain
one-step undo/redo.

Add a labeled Symbols toolbar control with a compact, keyboard-operable picker.
Include these exact characters with spoken/action labels: nonbreaking space
`U+00A0`, nonbreaking hyphen `U+2011`, em dash `U+2014`, ellipsis `U+2026`,
bullet `U+2022`, copyright `U+00A9`, pound `U+00A3`, euro `U+20AC`, yen
`U+00A5`, plus/minus `U+00B1`, multiplication `U+00D7`, division `U+00F7`,
left/right/up/down arrows `U+2190`/`U+2192`/`U+2191`/`U+2193`, check mark
`U+2713`, grinning face `U+1F600`, and red heart `U+2764 U+FE0F`. Selecting a
symbol inserts it at the caret or replaces the current selection through
`Command::ReplaceText`. Keep the picker dismissed after insertion, preserve
focus/reveal behavior, and provide accessible names for every symbol action.

Tests must exercise Unicode case conversion, punctuation and whitespace
preservation in title/sentence case, style-preserving conversion across mixed
runs and differently formatted paragraphs, page-break preservation, unchanged
text, undo/redo of a transformation, symbol insertion at a caret and over a
selection, and accessibility-visible controls.

Update feature inventory and acceptance evidence. Do not imply date/time,
custom replacements, or emoji search have shipped.

## Review focus

- Lowercasing can expand a source scalar into multiple bytes; never return an
  interior offset into the original text or select half of a grapheme.
- Whole-word checks must use Unicode word boundaries in original text rather
  than byte-offset assumptions and must behave consistently at paragraph edges.
- Replace all and UI replace must honor precisely the same search options as
  navigation, including undo behavior.
- Empty selections must not be changed by case-conversion actions.
- Symbol insertion must use the existing selection and command path so it is
  undoable and continues to mark the document dirty for recovery.
- Result-list controls and symbol buttons must remain reachable with keyboard
  navigation and expose meaningful labels.
- Virtualized search rows must be keyboard-navigable to offscreen matches; do
  not depend on mouse scrolling for results beyond the first viewport.
- Case conversion must preserve rich-text styles and explicit page structure,
  even when the converted letters are already in the requested case.

## Execution

Tasks are sequential because both touch the shared desktop UI. Each task is
implemented and committed by a fresh subagent, reviewed by a different fresh
subagent, and followed by a scoped re-review if the initial review identifies
blocking findings. A whole-branch review and full verification run follow the
last task.
