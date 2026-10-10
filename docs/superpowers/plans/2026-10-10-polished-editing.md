# Polished Editing and Repo Correctness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Implement and review tasks sequentially on the user-selected main checkout.

**Goal:** Repair audited bugs and deliver functioning formatting, writing-workbench and MCP tools from the user's approved feature roadmap.

**Architecture:** Keep validated undoable mutations in document-core, DOCX semantics in folio-docx, and interaction in desktop modules. Add focused helpers instead of further expanding main.rs with pure utility logic. The user has explicitly requested implementation again; preserve their main/subagent/commit/push choices without another permission loop.

**Tech Stack:** Rust 1.90, egui/eframe 0.32.3, existing locked offline dependencies.

**Spec:** docs/superpowers/specs/2026-10-10-folio-feature-roadmap-design.md (writing, formatting, accessibility/productivity and MCP portions).

## Global Constraints

- Keep the core UI-independent; document edits use validated core commands and undo/redo.
- Preserve Unicode grapheme safety, original files on failure and unsupported-content warnings.
- Version persistent data; new serialized fields must read existing recovery snapshots.
- Keep native ordinary editing offline; add no provider/network dependency.
- Keep minimalist legible controls with icons, names, keyboard paths and visible focus.
- Commit as configured hozaifahejaz on main; no force push or unrelated cleanup.
- Use checkout-local toolchain via scripts/validate-local.sh or matching environment.
- This plan advertises only implemented behavior; the full roadmap remains tracked separately.

## Review Focus

- Undo to clean state must remove this session's stale recovery without destroying unreadable startup data.
- New formatting must survive DOCX, mixed runs, Unicode edits, undo and old serialized recovery.
- Workbench dialogs must not hijack canvas input, block unsaved decisions or mutate content when cancelled.
- Navigation must use valid grapheme positions and current wrapped layout, including page breaks.
- MCP schemas, defaults, validation and tool execution must agree for live and background sessions.

### Task 1: Audited correctness fixes

**Files:** Modify apps/desktop/src/main.rs, crates/document-core/src/case.rs, core command tests and desktop regression tests. Read /tmp/folio-core-audit.md and /tmp/folio-desktop-audit.md.

**Interfaces:** Preserve existing public interfaces. Recovery cleanup remains fallible and preserves unreadable/future startup recovery. Title conversion preserves rich structure through Command::ConvertCase.

- [ ] Add failing regressions: dirty checkpoint then Undo to baseline and idle/normal Quit must leave no recoverable undone text; journal removal failure must report and retain data, while unreadable startup data survives. Title case of Greek ΟΣ must yield Ος; test mixed styles and undo. Include opening punctuation in title/sentence capitalization if doing so can preserve style mapping reliably, updating the stated policy.
- [ ] Run focused tests and observe failure.
- [ ] Fix the root causes, retaining explicit unsaved restore (do not silently autosave recovered content until a fresh edit). Test/document this restore policy.
- [ ] Run workspace tests, fmt and clippy using the local toolchain; append evidence to report.
- [ ] Commit the independently working fixes.

### Task 2: Rich text formatting end to end

**Files:** Modify core model/editor, DOCX formatting/codec tests, desktop layout/main/icons/workspace and docs. Add focused module only where needed.

**Interfaces:** Add TextStyle.strikethrough: bool, TextStyle.vertical_align: VerticalAlign (Baseline default, Superscript, Subscript), TextStyle.highlight: Option<Color>. New fields have serde defaults for old snapshots. Matching StylePatch fields support setting/clearing highlight through Option<Option<Color>>. All style validation and clear formatting must include new fields. Export VerticalAlign from document-core.

- [ ] Add failing tests for mixed-run formatting, undo/redo, old serialized style defaults, highlight clear, and toggling superscript/subscript without both being active.
- [ ] Upgrade workspace/recovery envelope schema to 2, read/migrate schema 1 with default new fields, write schema 2, and reject/preserve schema 3+; update future-version tests without weakening them. This keeps old schema-1 apps from silently dropping new formatting.
- [ ] Implement shared style commands and DOCX w:strike, w:vertAlign, run w:shd with val=clear and RGB fill for arbitrary highlight; import named OOXML w:highlight palette and simple clear run shading. Handle solid only using its foreground color, never interpreting fill as solid's foreground. Unsupported shading patterns/theme colors warn instead of silently claiming support. Highlight precedence over shading must not depend on XML child order; any unrepresentable layered inheritance warns. Preserve inherited style behavior. See official Open XML Shading/Highlight documentation.
- [ ] Render strikethrough/highlight in the native rich-text canvas. Render super/subscript at 75% size with top/bottom alignment and correct selection/caret hit testing; include layout regression assertions. Apply highlight to text, not the page.
- [ ] Add accessible Home controls and cohesive original icons for Strike, Superscript, Subscript and Highlight (palette including None). Clear formatting resets all new properties. Keep keyboard/pointer focus behavior consistent with existing Bold.
- [ ] Run targeted red/green tests, full workspace tests/fmt/clippy, update scoped feature/acceptance docs and commit.

### Task 3: Writing workbench and UI polish

**Files:** Create apps/desktop/src/workbench.rs for command metadata/state/navigation/productivity helpers and workbench_ui.rs for focused FolioApp interaction; modify main.rs/icons.rs and user docs. Persistent writing goals/preferences use existing versioned WorkspaceState with defaulted fields; session timer is session-only.

**Interfaces:** Pure helpers for first valid position of a 1-based paragraph, wrapped visual line and current-layout page start; reading/speaking duration estimates (200/130 words per minute); reusable command metadata. Produce FolioApp::workbench_blocks_editing() -> bool for palette/navigation/snippet/shortcut modal interaction guards consumed by later tasks. No native document model changes.

- [ ] Add working command palette (Command/Ctrl+K): query filters named commands, Up/Down/Enter executes, Escape cancels, and disabled commands cannot mutate. Include all current major actions plus new writing tools; use a bounded scrollable panel with accessible labels/icons.
- [ ] Add keyboard shortcut reference in View and palette. Add Go to page, Go to line (wrapped visual lines), and Go to paragraph with explicit 1-based limits, invalid inputs shown inline without moving caret; select/reveal via current layout and valid stops, retaining visual-line hint at soft wraps. Empty pages without editable positions must scroll to the requested page without fabricating a caret or moving to the wrong page. Add date/time insertion using local system time where available without a new dependency (fallback UTC must be labeled). Insert as one undoable edit.
- [ ] Add format painter: capture complete current text style, apply once to a nonempty selection, preserve paragraph styles, and make it one undoable transaction. Add clear paragraph formatting using default ParagraphPatch fields; do not flatten text.
- [ ] Add writing-progress panel: editable positive word goal (0 disables), actual document word count/progress, reading/speaking estimates, start/pause/reset session timer, net word change from start. Session progress handles deletions/undo and never edits document text.
- [ ] Add reusable local snippet insertion UI with at least three built-in useful blocks plus user-defined title/text saved in WorkspaceState. Limits: 32 user snippets, titles 1–80 Unicode scalars after trimming, nonempty text up to 65,536 UTF-8 bytes; word goal 0–1,000,000. Old states default goal=0 and snippets=[]; invalid stored entries cause explicit load/save error and preserve original state rather than silent deletion. Allow add/remove, and insertion replaces current selection through the existing command. No network/sync claims.
- [ ] Show application version and build revision in Document info. Have package-macos.sh set FOLIO_BUILD_REVISION from git for the release build and --version print it, so old running instances can be distinguished. Update the stale desktop README.
- [ ] Add meaningful unit/egui integration tests for palette routing/focus, invalid/Unicode navigation, timer pause/reset, goal/snippet serialization/validation, format painter history and new controls; run red/green and full checks, update docs, commit.

### Task 4: Useful document lifecycle and export functions

**Files:** Create apps/desktop/src/templates.rs; modify main.rs, editing.rs, icons.rs, files.rs and docs. Tests use temporary files, no real user documents.

**Interfaces:** Produce TemplateId (Blank, Letter, MeetingNotes, ProjectBrief), a template catalog with stable string IDs blank/letter/meeting_notes/project_brief, and validated original document builders. Produce FolioApp.read_only: bool and is_mutation_blocked() method for Task 5. Produce duplicate_to(path, overwrite) and export_text_to(path, selection: Option<Selection>, overwrite), preserving active editor state/history/path/dirty/import protection. Set read_only=false on successful New/Open/template switch; recovery preserves current content as before.

- [ ] Build a minimalist template gallery accessible from File and command palette. Three original useful formatted templates plus Blank use real document runs/paragraphs, not unsupported heading/list semantics. Choosing a template uses the same Save/Discard/Cancel lifecycle as New, clears path/warnings only on successful switch, and marks a nonblank template as unsaved. Cancellation/failure preserves the current editor and recovery.
- [ ] Implement Duplicate… as saving the complete current supported document to a distinct DOCX path while preserving active editor state. Warned-import source guards still apply; existing destination requires overwrite decision. Explicitly do not delete/rename original files.
- [ ] Add whole-document and selected-content plain-text export through atomic replacement, explicit .txt destination and overwrite handling. Preserve paragraph/page boundaries as newlines/form feed. Selected export respects reversed/grapheme selection and page breaks. Reject same-file source aliases and failed writes without changing editor state.
- [ ] Support dragging a single DOCX file onto the window through the existing unsaved-open flow. Reject multiple/unsupported dropped files visibly, preserving the editor. Do not silently open the last of several files.
- [ ] Add Read-only mode toggle in View and palette with an unmistakable subtle status badge. Enforce it in text/formatting/paste/undo/redo/page-layout paths, disable mutation toolbar actions, preserve navigation/copy/export/New/Open, and suspend auto-save while read-only. Recovery continues for preexisting unsaved edits. Main execute and format guards must prevent alternate routes. Treat this as editing mode, not encryption/security permission.
- [ ] Read-only mode permits duplicate/export/Save As to a distinct path but blocks overwriting the active source, including shared save adapters; resolve dirty read-only lifecycle Save through Save As instead of a dead end. Do not clear read-only on failed/cancelled transitions. Accessible canvas semantics must indicate read-only while preserving selection/navigation.
- [ ] Add meaningful TDD regressions for template cancellation/dirty state/validation; duplicate/export overwrite, failed IO/alias protection/state preservation; drop dispatch; and read-only mutations versus permitted navigation/copy. Update feature/acceptance docs, run full checks and commit.

### Task 5: MCP parity and headless integration

**Files:** Modify apps/desktop/src/mcp.rs, scripts/test-mcp.py, docs/MCP.md and feature docs.

**Interfaces:** Existing tool names/defaults remain compatible. Optional match_case defaults true and whole_words defaults false for folio_find/folio_replace_all. Add folio_convert_case(selection, case enum upper/lower/title/sentence). Text formatting accepts RGB color/highlight and strikethrough/vertical_align; highlight null clears it. Paragraph formatting accepts existing spacing fields and line spacing. Add folio_set_page_layout validated through Command::SetPageLayout using the existing model. Read output includes full page_layout, full run styles and paragraph styles.

Paragraph line_spacing uses {kind: multiple|exact|at_least, value: integer}, percentages for multiple and twips for exact/at_least; alignment becomes optional so spacing-only patches work. Page layout tool accepts {layout: {size: {width_twips,height_twips}, orientation: portrait|landscape, margins: {top,right,bottom,left}}}; read output uses the same layout shape. RGB uses {red,green,blue} integers 0–255. Validate unknown nested fields too. New vertical_align enum strings are baseline/superscript/subscript. Missing highlight preserves, explicit null clears; ordinary nested Option serde alone does not distinguish these.

Add folio_list_templates (read-only), optional template string to folio_new_document, folio_duplicate_document(path, overwrite=false), folio_export_text(path, selection optional, overwrite=false) adapting Task 4. Read output includes read_only and rejects document-mutating tools in read-only mode; navigation/read/export remain available. Do not expose an AI tool to disable the UI's read-only mode. Block MCP edits while an active workbench modal owns interaction, as well as existing lifecycle dialogs.

- [ ] Add failing schema/adapter tests for options defaults, Unicode and whole-word search, structure-preserving case+undo, all new text properties, paragraph spacing and page-layout get/edit/get. Invalid values/unknown fields must fail without changing selection/history/document.
- [ ] Implement adapters using shared core commands, with UI typing state/selection/reveal synchronization. Tool descriptions accurately state boundaries and units.
- [ ] Extend headless Python MCP smoke to exercise new tools and save/reopen DOCX formatting, preserving prior stdio/live lifecycle tests. No tests send user documents or start network services beyond existing authenticated loopback fixtures.
- [ ] Update MCP setup/discovery docs, run full workspace+headless checks/fmt/clippy and commit.

## Release verification

- [ ] Review each task diff; fix important findings before the next task.
- [ ] Whole-pass review against this plan and audit reports.
- [ ] Run sh scripts/validate-local.sh and python3 scripts/test-mcp.py against the fresh executable (inspect invocation first).
- [ ] Close the old running Folio process through its UI, preserving user content, then launch dist/Folio.app and confirm build revision and new controls. Perform native smoke with disposable text; leave a clean document.
- [ ] Push approved main changes to origin and report exact shipped features and remaining limitations.
