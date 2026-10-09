# Workspace State and Crash Recovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. The user approved working in the current `main` checkout and authorized multiple agents when useful. Keep shared edits coordinated and commit only reviewed, passing milestones.

**Goal:** Give Folio a durable workspace foundation: a useful recent-documents list, automatic saving for eligible documents, crash recovery for unsaved edits, and remembered appearance, zoom, and editing location, without changing DOCX safety semantics.

**Architecture:** Add a versioned local workspace store and a separate versioned recovery snapshot in the operating system’s per-user application-data directory. Serialize the complete native document model for recovery instead of round-tripping through the limited DOCX codec; atomically replace state files and retain warned-import protection metadata. Debounce writes from the UI, offer recovery explicitly at launch, and keep recent paths and per-document caret locations separate from unsaved content.

**Tech Stack:** Rust 2024, serde/serde_json, document-core, egui/eframe, standard filesystem APIs, Cargo workspace tests.

**Spec:** [2026-10-10 Folio feature roadmap design](../specs/2026-10-10-folio-feature-roadmap-design.md)

## Scope

This is the first implementation increment of the approved multi-stage roadmap. It includes recent documents, idle auto-save for named and unprotected DOCX documents, crash recovery, remembered theme and zoom, and remembered editing position. Untitled documents and warned imports use the recovery journal until the user explicitly saves a named converted copy. Tabs/windows, templates, password protection, and the remaining writing, layout, content, review, export, productivity, and accessibility items remain in the roadmap and need later implementation plans. This plan does not label those deferred features as complete.

## File Boundaries

- `crates/document-core/src/model.rs`: serde support for the persistent document/import-warning graph only.
- `crates/document-core/src/editor.rs`: explicit recovery restore semantics; existing editor history is not serialized.
- `apps/desktop/src/workspace.rs`: workspace/recovery types, versioned JSON, platform data path, lossless local path codec, validation, and store operations.
- `apps/desktop/src/files.rs`: expose the existing atomic replacement primitive to the sibling workspace module without duplicating it.
- `apps/desktop/src/main.rs`: app lifecycle integration, debounce scheduling, startup recovery prompt, recent-document UI, and preference persistence.
- `docs/FEATURES.md`, `docs/ACCEPTANCE.md`, `README.md`: document delivered behavior and verification boundaries.

## Global Constraints

- Work on the existing `main` checkout; do not create or switch worktrees.
- Preserve the rule that a warned DOCX import cannot overwrite its source or an alias. Recovery must retain that protection state.
- Recovery snapshots are private local application data, never DOCX files, and never silently replace a user document. Restore and discard require explicit user actions.
- Use versioned JSON envelopes and atomic writes. A corrupt or newer unsupported state file must not be treated as an empty valid file and overwritten without surfacing the issue.
- Create the per-user Folio data directory with owner-only permissions on Unix and keep both JSON state files under that directory; use normal per-user application-data ACLs on Windows.
- Bound the recent list to 12 unique paths. Keep missing entries visible but disabled/removable so a temporarily disconnected volume does not erase the entry.
- Auto-save a dirty document to its current named path after 5 seconds without document edits, only when it has no protected warned-import source. Untitled documents and warned imports do not auto-overwrite; their recovery journal remains until explicit save/discard.
- Debounce workspace preference/caret writes by 500 ms and recovery snapshots by 2 seconds after edits; flush pending state synchronously before a confirmed application close. Persist only when state changed.
- Persist the caret position per document, not an active selection. Preserve the full selection in a recovery snapshot. Validate recovered positions against grapheme boundaries; fall back to a valid caret if stale.
- Keep platform-specific path encoding lossless (Unix path bytes, Windows UTF-16 code units). Do not store paths through lossy display strings.
- If a settings/recovery write fails, retain the previous atomic file, show an actionable status/error, and keep editing available.
- Do not introduce cloud sync, telemetry, a spelling/grammar service, collaboration, or password encryption in this increment.

## Review Focus

1. **Truncated, malformed, old, and future-version JSON:** Task 3 tests explicit parse/version errors and verifies the source bytes remain unchanged.
2. **Non-UTF-8 Unix paths, Windows UTF-16 paths, missing files, aliases, and duplicates:** Task 3 tests platform-conditional path round-trips and uniqueness; Task 6 tests aliases and missing recent paths.
3. **Stale block indices, out-of-range byte offsets, split graphemes, and changed document structure:** Task 2 tests safe selection fallback; Task 6 tests remembered-caret fallback.
4. **Read-only directories, full disks, interrupted writes, and failed atomic replacement:** Task 3 tests the previous state remains readable; Task 4 tests recovery remains available after a failed write.
5. **Warned DOCX imports and rapid edits restored after a crash:** Task 4 tests protected-source behavior, auto-save eligibility, and debounce coalescing; Task 6 retains source-overwrite tests and verifies a single recovery snapshot is replaced.

## Tasks

### Task 1: Make the native document graph serializable

**Files:** `Cargo.toml`, `crates/document-core/Cargo.toml`, `crates/document-core/src/model.rs`, `crates/document-core/tests/recovery_serialization.rs`

**Interfaces:** Produces serde-serializable `Document`, `Position`, `Selection`, `ImportWarning`, `WarningCode`, and `Feature` graphs for Tasks 2–4. `Command`, `Editor`, undo history, and UI types remain non-serializable. Pin `serde = "=1.0.229"` with `derive` and `serde_json = "=1.0.145"` in `[workspace.dependencies]`; use both from core tests and the desktop store.

- [ ] **Step 1: Write the failing test.** Add `native_document_json_round_trips_every_model_variant`, constructing styled and empty paragraphs, a page break, landscape page layout, and warnings containing every `WarningCode`/`Feature` shape. Assert JSON round-trip equality and `Document::validate()` on the decoded model.
- [ ] **Step 2: Run the test and confirm it fails to compile** because the persistent model graph does not implement serde.
- [ ] **Step 3: Implement serialization.** Add the pinned dependencies above to `Cargo.toml`, use serde in `document-core`, add serde_json as a core dev-dependency, and derive `Serialize`/`Deserialize` on only the model/import-warning types used by recovery.
- [ ] **Step 4: Run the focused test and core suite.** Run `cargo test -p document-core native_document_json_round_trips_every_model_variant`, then `cargo test -p document-core`.
- [ ] **Step 5: Commit the passing model change** as `feat(core): serialize recovery document model`.

### Task 2: Define editor recovery semantics

**Files:** `crates/document-core/src/editor.rs`, `crates/document-core/tests/recovery_serialization.rs`

**Interfaces:** Consumes the serializable types from Task 1. Produces `Editor::load_recovered_document(document: Document, selection: Selection) -> Result<(), CoreError>` for Task 5. The method validates and normalizes the document, restores a valid selection or uses `Selection::caret(Position::new(0, 0))`, creates no fake undo entry, and leaves the editor dirty until `mark_saved()`.

- [ ] **Step 1: Write failing tests.** Add `recovered_document_restores_valid_selection_and_is_dirty`, `recovered_document_rejects_invalid_document`, `recovered_document_falls_back_from_invalid_selection`, and `mark_saved_clears_recovered_dirty_state`.
- [ ] **Step 2: Run those tests and confirm the recovery API/dirty behavior is absent.**
- [ ] **Step 3: Implement the recovery API.** Add a recovered-dirty marker to `Editor`; `is_dirty()` includes it and `mark_saved()` clears it. Keep normal `new`, `load_document`, and undo/redo semantics unchanged.
- [ ] **Step 4: Run the focused and full core suites.** Execute `cargo test -p document-core recovered_document` and `cargo test -p document-core`.
- [ ] **Step 5: Commit the passing editor change** as `feat(core): support dirty recovery restore`.

### Task 3: Build the versioned workspace persistence module

**Files:** `apps/desktop/src/workspace.rs`, `apps/desktop/src/files.rs`, `apps/desktop/src/main.rs`, `apps/desktop/Cargo.toml`

**Interfaces:** Produces `WorkspaceStore::at(root: PathBuf) -> WorkspaceStore`, `WorkspaceStore::for_user() -> Result<WorkspaceStore, StoreError>`, `load_state() -> Result<Option<WorkspaceState>, StoreError>`, `save_state(&WorkspaceState) -> Result<(), StoreError>`, `load_recovery() -> Result<Option<RecoverySnapshot>, StoreError>`, `save_recovery(&RecoverySnapshot) -> Result<(), StoreError>`, and `clear_recovery() -> Result<(), StoreError>`. `StoreError` distinguishes IO, malformed JSON, and unsupported schema versions. `WorkspaceState` contains `dark_mode: bool`, `zoom: f32`, `recent: Vec<RecentDocument>`, and `carets: Vec<SavedCaret>`. `RecentDocument` contains `path: StoredPath` and `last_opened_unix_seconds: u64`; `SavedCaret` contains `path: StoredPath` and `position: Position`. `WorkspaceState::record_recent(path: StoredPath, timestamp: u64)`, `remove_recent(path: &StoredPath)`, `set_caret(path: StoredPath, position: Position)`, and `caret_for(path: &StoredPath) -> Option<Position>` manage these collections. `StoredPath::from_path(path: &Path) -> Result<StoredPath, StoreError>` and `to_path(&self) -> Result<PathBuf, StoreError>` use a tagged `Vec<u8>` payload (`unix-bytes` on Unix, UTF-16LE bytes on Windows). `RecoverySnapshot` contains `document: Document`, `path: Option<StoredPath>`, `protected_source: Option<StoredPath>`, `warnings: Vec<ImportWarning>`, `selection: Selection`, and `captured_unix_seconds: u64`. JSON envelopes carry `schema_version: u32`; filenames are `workspace.json` and `recovery.json`.

- [ ] **Step 1: Write failing store tests.** Add `workspace_state_round_trips_preferences_and_recent_order`, `recent_list_is_unique_and_limited_to_twelve`, `unix_non_utf8_path_round_trips` (Unix), `windows_utf16_path_round_trips` (Windows), `future_schema_is_rejected_without_changing_bytes`, `malformed_state_is_reported_without_replacement`, and `failed_state_write_preserves_previous_file`.
- [ ] **Step 2: Run the new tests and confirm the module/store APIs are missing.**
- [ ] **Step 3: Implement `workspace.rs`.** Use typed versioned envelopes, an injectable root for tests, an OS data-directory resolver (`APPDATA` on Windows, `~/Library/Application Support/Folio` on macOS, absolute XDG data home or `~/.local/share/folio` on Linux), and the tagged path representation above. Create the directory with Unix mode `0700`, reject unsupported schema versions without rewriting them, and validate/clamp zoom to `0.25..=2.5`.
- [ ] **Step 4: Reuse atomic writes.** Expose `files::atomic_write` as `pub(super)` and call it from the workspace module. Do not duplicate the temporary-file/replace logic.
- [ ] **Step 5: Run the store and existing file tests.** Execute `cargo test -p folio-desktop workspace::` and `cargo test -p folio-desktop files::tests`.
- [ ] **Step 6: Commit the passing persistence module** as `feat(desktop): add versioned workspace storage`.

### Task 4: Add debounced recovery snapshots

**Files:** `apps/desktop/src/workspace.rs`, `apps/desktop/src/main.rs`

**Interfaces:** Consumes `WorkspaceStore` and `RecoverySnapshot` from Task 3. Produces a `CheckpointDebounce` value with `changed_at: Option<Instant>`, `mark_changed(now: Instant)`, `is_due(now: Instant, delay: Duration) -> bool`, and `clear()`, plus app helpers that capture the current editor/path/protection/warnings/selection in one snapshot.

- [ ] **Step 1: Write failing tests.** Add `recovery_snapshot_round_trips_warning_guard_and_selection`, `checkpoint_debounce_coalesces_rapid_edits`, `autosave_debounce_waits_for_five_seconds_of_idle`, `autosave_is_ineligible_for_untitled_or_warned_import`, `recovery_replacement_is_single_atomic_snapshot`, and `failed_recovery_write_keeps_previous_snapshot`.
- [ ] **Step 2: Run the focused tests and confirm the recovery capture/scheduler behavior is absent.**
- [ ] **Step 3: Implement automatic save and recovery scheduling.** Auto-save through the existing `files::save` / `save_to` path after five idle seconds only when `path.is_some()` and `protected.is_none()`. After two idle seconds, save one `recovery.json` snapshot; compare against the last successfully written `Document` to skip unchanged snapshots. Keep file operations out of per-frame redraws and call `ctx.request_repaint_after` only while a checkpoint is pending.
- [ ] **Step 4: Flush pending recovery before a confirmed close.** Clear recovery only after successful save, explicit discard, or an accepted New/Open transition after the existing unsaved-change prompt. On write failure, keep the old snapshot and surface the error. Workspace preference flushing is added in Task 7.
- [ ] **Step 5: Run the store and recovery app tests.** Execute `cargo test -p folio-desktop workspace::` and the recovery-specific app tests.
- [ ] **Step 6: Commit the passing recovery lifecycle** as `feat(desktop): checkpoint unsaved recovery state`.

### Task 5: Restore workspace preferences and offer recovery at launch

**Files:** `apps/desktop/src/main.rs`, `apps/desktop/src/workspace.rs`

**Interfaces:** Produces `FolioApp::with_workspace_store(store: WorkspaceStore) -> FolioApp` for tests and `FolioApp::from_user_profile() -> FolioApp` for desktop startup. `FolioApp` holds `workspace_store: Option<WorkspaceStore>` so existing `Default`-based unit tests remain isolated from the real profile. The constructor loads valid theme/zoom/recent state and retains `Option<RecoverySnapshot>` for a restore prompt; a missing file means normal first launch, while an invalid file produces a visible notice and is left unchanged.

- [ ] **Step 1: Write failing app tests.** Add `startup_restores_theme_zoom_and_recents`, `recovery_prompt_waits_for_explicit_restore_or_discard`, `recovery_restores_editor_as_dirty`, and `corrupt_recovery_does_not_block_startup`.
- [ ] **Step 2: Run the tests and confirm the app currently starts with no persistent workspace state.**
- [ ] **Step 3: Implement the injectable constructor and startup load.** Use `with_workspace_store` in tests and `from_user_profile` in `main`; keep `Default` for existing unit tests with `workspace_store: None` so tests never touch the real profile.
- [ ] **Step 4: Add the restore dialog.** Show **Restore**, **Discard recovery**, and the capture time; restore through `Editor::load_recovered_document`. Do not load recovery into the editor until Restore is clicked.
- [ ] **Step 5: Run the named app tests and desktop suite.** Execute `cargo test -p folio-desktop`.
- [ ] **Step 6: Commit the passing startup integration** as `feat(desktop): restore workspace and offer recovery`.

### Task 6: Integrate document actions, recent paths, and remembered carets

**Files:** `apps/desktop/src/main.rs`, `apps/desktop/src/workspace.rs`

**Interfaces:** Produces a testable `FolioApp::open_path(path: &Path) -> Result<(), String>` that loads a document after the existing unsaved-change gate. `FolioApp::request_open_path(path: PathBuf, ctx: &egui::Context)` stores the target and routes recent-item activation through `request(Pending::Open, ctx)`; `perform(Pending::Open)` consumes that target after confirmation, or shows the dialog when no target exists. Successful open/save encodes the path with `StoredPath::from_path(path)` and updates state via `WorkspaceState::record_recent(stored_path, timestamp)`; remembered caret access uses `WorkspaceState::caret_for(&stored_path)` / `set_caret(stored_path, position)`.

- [ ] **Step 1: Write failing lifecycle tests.** Add `successful_open_adds_recent_but_cancelled_dialog_does_not`, `recent_open_waits_for_unsaved_change_decision`, `successful_save_adds_recent_and_clears_recovery`, `failed_save_retains_recovery`, `same_file_alias_does_not_duplicate_recent`, `missing_recent_path_is_retained`, and `stale_caret_restores_as_valid_caret`.
- [ ] **Step 2: Run the tests and confirm lifecycle integration is missing.**
- [ ] **Step 3: Extract the path-based `open_path` operation** from `Pending::Open`, preserving warnings, protected-source state, and error behavior; make dialog cancellation a no-op.
- [ ] **Step 4: Integrate transitions.** Record a path only after successful open/save; deduplicate with `files::same_file` when possible; apply a validated remembered caret on open; capture caret changes; clear recovery after successful save or explicit discard only. Keep `request_open_path` as the only recent-item entry point so unsaved-change decisions run first.
- [ ] **Step 5: Run lifecycle and DOCX safety tests.** Execute the named app tests and existing DOCX protection/save tests.
- [ ] **Step 6: Commit the passing document lifecycle integration** as `feat(desktop): integrate recent document lifecycle`.

### Task 7: Add Recent Documents UI and preference persistence

**Files:** `apps/desktop/src/main.rs`, `apps/desktop/src/icons.rs`, `apps/desktop/src/workspace.rs`

**Interfaces:** Adds `FolioApp::recent_documents_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context)` in the File tab. The UI consumes `WorkspaceState::recent` and routes selection to Task 6's `request_open_path`. Add `RecentDocumentPresentation { name: String, location: String, available: bool }` and `recent_document_presentation(path: &Path) -> RecentDocumentPresentation` as a testable view model. Theme and zoom changes update `WorkspaceState`; caret/theme/zoom changes share the 500 ms state debounce from Task 4.

- [ ] **Step 1: Add view-model tests** named `recent_presentation_uses_file_and_parent_names` and `recent_presentation_marks_missing_paths_unavailable`, plus a state test that removal leaves other recents in order.
- [ ] **Step 2: Implement a compact File-tab Recent Documents group** with a leading icon, filename, parent location, disabled-but-visible missing entries, and a remove action. Keep the existing ribbon compact at narrow widths and preserve keyboard focus traversal.
- [ ] **Step 3: Persist preferences and caret position.** Detect actual changes, schedule one workspace-state write after 500 ms, clamp zoom to the supported range, and flush before a confirmed close.
- [ ] **Step 4: Run view-model and app tests.** Execute `cargo fmt --all -- --check` and `cargo test -p folio-desktop`.
- [ ] **Step 5: Commit the passing File-tab UI/preferences** as `feat(ui): add recent documents and saved preferences`.

### Task 8: Update feature inventory and verify the integrated increment

**Files:** `docs/FEATURES.md`, `docs/ACCEPTANCE.md`, `README.md`

**Interfaces:** Documentation consumes the actual behavior from Tasks 1–7; no future roadmap item may be marked supported by this task.

- [ ] **Step 1: Update documentation** with the local recovery lifecycle and exact delivered boundaries: idle auto-save for named unprotected DOCX files, recovery for untitled and warned imports, local recent list, per-document caret, remembered theme/zoom, explicit restore/discard.
- [ ] **Step 2: Run `cargo fmt --all -- --check`, `cargo test --workspace`, and `cargo clippy --workspace --all-targets -- -D warnings`.
- [ ] **Step 3: Launch Folio and manually verify** first-run defaults, theme/zoom persistence, recent open and missing-file handling, dirty recovery restore/discard, save-and-clear, and warned-import protection. Record OS/toolchain and limits in `docs/ACCEPTANCE.md`.
- [ ] **Step 4: Review the final diff and commit the documentation** as `docs: document workspace recovery and verification`.
- [ ] **Step 5: Push the passing implementation on `main` to the existing origin** under the approved `hozaifahejaz` GitHub author workflow.

## Completion Criteria

- A successful open/save is visible in Recent Documents and appears only once; eligible named documents auto-save after five idle seconds.
- Theme, zoom, and caret location survive a normal restart.
- A dirty document can be restored after an unclean exit, remains marked unsaved, and retains its original import-protection state.
- Save/discard clears recovery only after the requested operation succeeds; a failed save leaves recovery available.
- Invalid state never crashes startup or silently overwrites the last valid state.
- Existing workspace tests, formatting, clippy, and documented native smoke checks pass.
- `docs/FEATURES.md` reports only this delivered increment as supported; the remaining approved roadmap is still explicitly deferred.
