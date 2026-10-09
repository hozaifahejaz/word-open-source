# Folio Feature Roadmap Design

**Status:** Approved architecture direction. The user's renewed request to check the whole repo, add functions, polish them and make them work authorizes staged implementation on the previously selected main checkout. Owning-stage design gates for cryptography and online collaboration still apply.

## Purpose

Grow Folio from its existing cross-platform word-processing foundation into a
modern, polished document editor that covers the feature list supplied by the
user on 2026-10-10. Deliver the work in usable releases. Keep document editing
local by default, preserve existing files and supported DOCX content, and do not
claim a feature is complete until its behavior and limits are implemented,
verified, and recorded.

## Current foundation

Folio is a native Rust desktop application for macOS, Windows, and Linux. The
workspace separates `document-core` (the validated document model, editing
commands, selections, and history), `folio-docx` (bounded DOCX conversion), and
`folio-desktop` (UI and file handling). The editor already has Unicode-safe
paragraphs and styled runs, selection, undo/redo, basic formatting, literal
find/replace, page breaks and one page layout, statistics, focus mode, appearance
switching, document information, and a 14-tool MCP service. DOCX currently
supports a documented semantic subset; omitted or approximated content warns.

The implementation inventory in `docs/FEATURES.md` remains the source of truth
for shipped behavior. The feature list below is a target inventory, not a claim
that the current application already implements those features.

## Product and design principles

- Keep the interface calm and uncluttered, with clear hierarchy, intentional
  spacing, readable type, consistent icons and labels, and refined interactive
  states. Add controls where the related task happens; use panels for supporting
  information rather than crowding the document canvas.
- Preserve the existing minimalist Folio visual language while making advanced
  features discoverable. Support keyboard use and visible focus, screen-reader
  names, adequate contrast, scalable layouts, and reduced motion.
- Keep native editing, recovery, and ordinary file operations usable offline.
  Folio does not send document content to an AI, grammar, sync, or cloud service
  unless the user enables a future connected feature and chooses to use it.
- Keep edits validated and undoable through shared core commands. Avoid separate
  feature-specific mutation paths that bypass selection, validation, or history.
- Evolve the document model and DOCX codec deliberately. Preserve data on
  roundtrips where supported; warn clearly before unsupported or approximated
  content could be lost. Never silently discard unknown imported content.
- Keep preferences, recovery data, collaboration state, and document content
  under distinct ownership and retention rules. Recovery data must not be
  confused with an explicitly saved document.
- Expose eligible document capabilities through MCP as the model grows, with
  discoverable schemas, validated ranges, permission-aware actions, and explicit
  errors for unsupported operations.

## Release sequence

Each stage is a separate project and must leave the previous stages working.
Stages may be split into smaller specs if implementation reveals independent
model or service boundaries. No stage promises a delivery date.

### 1. Workspace and document lifecycle

Add a recent-documents menu; automatic saving; crash recovery; multiple tabs and
separate windows; remembered editing position; document templates and a template
gallery; duplication and rename; drag-and-drop opening; read-only mode; and
password-protected documents. Establish versioned preferences and recovery
storage before storing more application or document state. Add a reversible
recovery flow and explicit local read/write behavior. Define password protection
using a reviewed encryption format and key handling design before implementation;
never invent a reversible password wrapper or silently change DOCX encryption
semantics. Keep tabs and separate windows compatible with the existing native
window model.

### 2. Writing and formatting

**Writing and editing:** spell checking, grammar suggestions, custom spelling
dictionaries, automatic capitalization, smart quotation marks, automatic text
replacements, configurable autocorrect, case conversion, case-insensitive and
whole-word search, regular-expression search, a search results list, navigation
to a page, line, or heading, date/time and special-character insertion, emoji,
nonbreaking spaces and hyphens, paste without formatting, a format painter,
repeat-last-action, invisible-character display, a shortcut reference, and
custom shortcuts.

Spell/grammar providers and dictionaries must have clear local/online behavior.
Suggestions remain optional, dismissible, and must not silently change saved text.
Regex search reports invalid expressions without changing the document.

**Text and paragraphs:** named paragraph styles; heading, title, and subtitle
styles; custom style creation/editing; text highlight and expanded color palettes;
superscript, subscript, strikethrough, and small caps; character spacing; before
and after spacing; first-line and hanging indents; tab stops; paragraph borders
and shading; keep-with-next and widow/orphan controls; clearing paragraph
formatting; and default document formatting.

**Lists and structure:** bulleted, numbered, nested, and custom-symbol lists;
restart/continue numbering; checklists; automatic list creation; a document
outline; collapsible headings; drag-to-reorder headings; automatic table of
contents; bookmarks; internal links; cross-references; footnotes; endnotes;
citations; bibliography and index generation; and section breaks.

### 3. Page layout and long-document navigation

Add page size selection, orientation, custom margins, horizontal/vertical rulers,
headers and footers, automatic page numbers, first-page and odd/even variants,
multiple columns and column breaks, page colors, watermarks and borders,
section-specific settings, continuous and two-page reading, fit-page and
fit-width controls, remembered zoom, and full-screen mode. Complete the
navigation and reference behaviors in Stage 2 against stable structure and page
layout identities. Ensure pagination changes remain predictable and testable as
long documents are edited.

### 4. Tables and embedded content

**Tables:** insert tables; add/delete rows and columns; resize; merge/split;
cell alignment; borders and fills; style presets; repeated header rows; sorting;
text/table conversion; simple calculations; and keyboard navigation.

**Images and other content:** insert, drag, and paste images; resize, crop, and
rotate; image alignment and text wrapping; captions and alternative text;
insert/edit/remove hyperlinks; horizontal rules; text boxes and basic shapes;
mathematical equations; charts; code blocks; attachments; and QR codes.

All content types need explicit model, rendering, selection, pagination, undo,
DOCX import/export, and warning behavior before appearing in the UI. An
unsupported object must not vanish silently during open or save.

### 5. Review, history, and collaboration

Add comments, replies and resolution; tracked changes and accept/reject; reviewer
identity and colors; a review sidebar; exporting with or without markup; shared
links; live collaborative editing and presence; view/edit permissions; document
activity history; version history and comparison.

Local comments, revisions, version snapshots, and comparison can be designed as
native document features. Shared links, live multi-user edits, presence, and
permissions require a separate reviewed service/security specification covering
identity, authorization, transport, encryption, revocation, conflict resolution,
retention, and deployment before their implementation is planned. The word
processor must remain useful without that service.

### 6. Export, printing, productivity, and access

**Export and printing:** PDF export; print preview and native printing; print
page ranges; plain text; Markdown, HTML, OpenDocument, and Rich Text Format
import/export; selection export; page images; batch conversion; export
accessibility checks; and stronger DOCX compatibility reporting.

**Personalization and accessibility:** saved appearance and system-theme
following; interface scale; accent colors; configurable toolbar; command
palette; right-click menus; screen-reader work; high contrast; reduced motion;
full keyboard navigation; interface translations; right-to-left and mixed
direction text; font previews and favorites; missing-font warnings; and
typewriter scrolling.

**Writing productivity:** word-count goals, daily targets, a session timer and
word progress, reading/speaking time estimates, frequently used snippets,
reusable content blocks, and document metadata editing.

## Technical boundaries

- Keep the core UI-independent. Add document semantics and shared commands to
  `document-core`; file/package handling belongs in `folio-docx` or the desktop
  file layer as appropriate; drawing, dialogs, and interaction belong in focused
  desktop modules.
- Use stable internal identifiers for structured objects, sections, and
  annotations where index-based positions would become unreliable. Preserve
  Unicode grapheme safety for text ranges.
- Version persistent settings, recovery journals, templates, and any new native
  document container. Define migrations and corruption recovery before shipping
  persistent data. Do not reinterpret existing DOCX packages as native containers.
- Keep the current DOCX warning and protected-source rules. Add fixtures for each
  supported OOXML feature and tests for unsupported-content warnings before
  advertising compatibility.
- Keep supported MCP behavior provider-independent. Add tools only after the
  corresponding document command is stable; use explicit read-only/destructive
  metadata and validated arguments.
- Use no Microsoft branding or bundled Microsoft templates/assets. Keep existing
  cross-platform Rust versions and build requirements unless a separately
  reviewed spec changes them.

## Acceptance criteria

1. Every target feature above is mapped to a tracked implementation stage; the
   `docs/FEATURES.md` inventory is updated as features move from deferred to
   partial or supported.
2. Each stage produces working software that builds on the previous stage,
   documents limitations, and has UI paths that work by keyboard as well as
   pointer where applicable.
3. Document edits pass through validated core commands and share undo/redo where
   the operation changes document content.
4. Saving, recovery, and format conversion preserve the original document on
   failure; unsupported import/export content raises a visible warning.
5. The interface remains legible and usable at supported window sizes, themes,
   text directions, and accessibility settings; major feature surfaces receive
   native visual inspection.
6. Online processing and shared access are opt-in. Access can be understood,
   limited, and revoked, with user-visible status and clear data handling.

## Open design gates

- Select native persistence formats and migration policies for preferences,
  recovery, templates, and version history in the owning stage specs.
- Decide whether password protection targets a native Folio format, encrypted
  DOCX interoperability, or both; review cryptography and recovery behavior
  before implementation.
- Specify which spell/grammar functions can work offline and how optional
  provider-backed checking obtains consent.
- Specify hosting, identity, trust, and operating costs before online sharing,
  shared links, or collaborative editing are built.
- Define the native model and DOCX mapping for tables, media, sections, lists,
  annotations, and references in their stage specs.
- Decide export dependency and platform-printing strategies in the export stage.

These are intentionally separate implementation decisions; they do not remove
the corresponding features from the target inventory.
