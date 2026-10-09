mod editing;
mod files;
mod icons;
mod layout;
mod mcp;
mod theme;
mod workspace;
use document_core::*;
use editing::{Action, move_to};
use egui::{Color32, Key, Rect, Stroke, Vec2};
use icons::{Icon, IconButton, RecentDocumentButton};
use layout::DocumentLayout;
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use workspace::{CheckpointDebounce, RecoverySnapshot, StoredPath, WorkspaceState, WorkspaceStore};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    File,
    Home,
    Layout,
    View,
}
#[derive(Clone, Copy)]
enum Pending {
    New,
    Open,
    Quit,
}
struct FolioApp {
    editor: Editor,
    path: Option<PathBuf>,
    protected: Option<PathBuf>,
    warnings: Vec<ImportWarning>,
    tab: Tab,
    zoom: f32,
    typing: Option<TextStyle>,
    composition: Option<String>,
    ime_enabled: bool,
    error: Option<String>,
    notice: String,
    pending: Option<Pending>,
    open_target: Option<PathBuf>,
    observed_caret: Option<(PathBuf, Position)>,
    overwrite: Option<PathBuf>,
    search_open: bool,
    focus_search: bool,
    needle: String,
    replacement: String,
    focus_canvas: bool,
    reveal: bool,
    preferred_x: Option<f32>,
    visual_line: Option<usize>,
    pages: usize,
    active_page: usize,
    dark_mode: bool,
    focus_mode: bool,
    show_document_info: bool,
    show_ai_connection: bool,
    ai_bridge: Option<mcp::Bridge>,
    allow_close: bool,
    workspace_store: Option<WorkspaceStore>,
    recovery_checkpoint: CheckpointDebounce,
    autosave_checkpoint: CheckpointDebounce,
    observed_document: Document,
    last_recovery_document: Option<Document>,
    recovery_cleanup_pending: bool,
    workspace_state: WorkspaceState,
    observed_workspace: WorkspaceState,
    saved_workspace: WorkspaceState,
    state_checkpoint: CheckpointDebounce,
    pending_recovery: Option<RecoverySnapshot>,
    unreadable_recovery: bool,
}
impl Default for FolioApp {
    fn default() -> Self {
        Self {
            editor: Editor::default(),
            path: None,
            protected: None,
            warnings: vec![],
            tab: Tab::Home,
            zoom: 1.0,
            typing: None,
            composition: None,
            ime_enabled: false,
            error: None,
            notice: String::new(),
            pending: None,
            open_target: None,
            observed_caret: None,
            overwrite: None,
            search_open: false,
            focus_search: false,
            needle: String::new(),
            replacement: String::new(),
            focus_canvas: true,
            reveal: false,
            preferred_x: None,
            visual_line: None,
            pages: 1,
            active_page: 1,
            dark_mode: false,
            focus_mode: false,
            show_document_info: false,
            show_ai_connection: false,
            ai_bridge: None,
            allow_close: false,
            workspace_store: None,
            recovery_checkpoint: CheckpointDebounce::default(),
            autosave_checkpoint: CheckpointDebounce::default(),
            observed_document: Document::default(),
            last_recovery_document: None,
            recovery_cleanup_pending: false,
            workspace_state: WorkspaceState::default(),
            observed_workspace: WorkspaceState::default(),
            saved_workspace: WorkspaceState::default(),
            state_checkpoint: CheckpointDebounce::default(),
            pending_recovery: None,
            unreadable_recovery: false,
        }
    }
}
impl FolioApp {
    fn with_workspace_store(store: WorkspaceStore) -> Self {
        let mut app = Self::default();
        let mut errors = Vec::new();
        match store.load_state() {
            Ok(Some(state)) => {
                app.dark_mode = state.dark_mode;
                app.zoom = state.zoom;
                app.workspace_state = state;
            }
            Ok(None) => {}
            Err(error) => errors.push(format!("Could not load workspace: {error}")),
        }
        match store.load_recovery() {
            Ok(Some(snapshot)) => {
                // Validate all restored data before offering it; leave the stored file intact.
                match Self::recovered_editor(&snapshot) {
                    Ok(_) => app.pending_recovery = Some(snapshot),
                    Err(error) => {
                        app.unreadable_recovery = true;
                        errors.push(format!("Could not load recovery: {error}"));
                    }
                }
            }
            Ok(None) => {}
            Err(error) => {
                app.unreadable_recovery = true;
                errors.push(format!("Could not load recovery: {error}"));
            }
        }
        if !errors.is_empty() {
            app.error = Some(errors.join("\n\n"));
        }
        app.observed_workspace = app.workspace_state.clone();
        app.saved_workspace = app.workspace_state.clone();
        app.workspace_store = Some(store);
        app
    }
    fn from_user_profile() -> Self {
        match WorkspaceStore::for_user() {
            Ok(store) => Self::with_workspace_store(store),
            Err(error) => Self {
                error: Some(format!("Could not initialize workspace: {error}")),
                ..Default::default()
            },
        }
    }
    fn recovered_editor(snapshot: &RecoverySnapshot) -> Result<Editor, String> {
        for path in [&snapshot.path, &snapshot.protected_source]
            .into_iter()
            .flatten()
        {
            path.to_path().map_err(|error| error.to_string())?;
        }
        let mut editor = Editor::default();
        editor
            .load_recovered_document(snapshot.document.clone(), snapshot.selection)
            .map_err(|error| error.to_string())?;
        Ok(editor)
    }
    fn restore_startup_recovery(&mut self) -> bool {
        let Some(snapshot) = self.pending_recovery.as_ref() else {
            return false;
        };
        let editor = match Self::recovered_editor(snapshot) {
            Ok(editor) => editor,
            Err(error) => {
                self.error = Some(format!("Could not restore recovery: {error}"));
                return false;
            }
        };
        self.path = snapshot.path.as_ref().map(|path| path.to_path().unwrap());
        self.protected = snapshot
            .protected_source
            .as_ref()
            .map(|path| path.to_path().unwrap());
        self.warnings = snapshot.warnings.clone();
        self.editor = editor;
        self.observed_document = self.editor.document().clone();
        self.last_recovery_document = Some(self.editor.document().clone());
        self.pending_recovery = None;
        self.typing = None;
        self.focus_canvas = true;
        self.reveal = true;
        self.notice = "Recovered unsaved document".into();
        true
    }
    fn discard_startup_recovery(&mut self) -> bool {
        if !self.remove_recovery() {
            return false;
        }
        self.pending_recovery = None;
        self.unreadable_recovery = false;
        self.reset_recovery_tracking();
        self.focus_canvas = true;
        self.notice = "Recovery discarded".into();
        true
    }
    fn schedule_checkpoints(&mut self, now: Instant) {
        self.schedule_workspace(now);
        if self.observed_document != *self.editor.document() {
            self.observed_document = self.editor.document().clone();
            if self.editor.is_dirty() {
                self.recovery_checkpoint.mark_changed(now);
                self.autosave_checkpoint.mark_changed(now);
            }
        }
    }
    fn schedule_workspace(&mut self, now: Instant) {
        self.capture_caret();
        self.zoom = if self.zoom.is_finite() {
            self.zoom.clamp(0.25, 2.5)
        } else {
            1.0
        };
        self.workspace_state.dark_mode = self.dark_mode;
        self.workspace_state.zoom = self.zoom;
        if self.workspace_state != self.observed_workspace {
            self.observed_workspace = self.workspace_state.clone();
            self.state_checkpoint.mark_changed(now);
        }
    }
    fn flush_workspace(&mut self) -> bool {
        self.schedule_workspace(Instant::now());
        self.state_checkpoint.clear();
        if self.workspace_state == self.saved_workspace {
            return true;
        }
        if let Some(store) = &self.workspace_store
            && let Err(error) = store.save_state(&self.workspace_state)
        {
            self.error = Some(format!(
                "Could not save workspace preferences: {error}. Check the application data directory and try again."
            ));
            return false;
        }
        self.saved_workspace = self.workspace_state.clone();
        true
    }
    fn recovery_snapshot(&self) -> Result<RecoverySnapshot, workspace::StoreError> {
        Ok(RecoverySnapshot {
            document: self.editor.document().clone(),
            path: self
                .path
                .as_deref()
                .map(StoredPath::from_path)
                .transpose()?,
            protected_source: self
                .protected
                .as_deref()
                .map(StoredPath::from_path)
                .transpose()?,
            warnings: self.warnings.clone(),
            selection: self.editor.selection(),
            captured_unix_seconds: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        })
    }
    fn flush_recovery(&mut self) -> bool {
        if !self.editor.is_dirty()
            || self.workspace_store.is_none()
            || self.last_recovery_document.as_ref() == Some(self.editor.document())
        {
            self.recovery_checkpoint.clear();
            return true;
        }
        if self.unreadable_recovery {
            self.error = Some(
                "Could not checkpoint recovery: the unreadable recovery file has been preserved. Save your current document to keep your changes, or choose Discard recovery to delete the old recovery file and enable new checkpoints."
                    .into(),
            );
            self.recovery_checkpoint.clear();
            return false;
        }
        let result = self.recovery_snapshot().and_then(|snapshot| {
            self.workspace_store
                .as_ref()
                .unwrap()
                .save_recovery(&snapshot)
        });
        match result {
            Ok(()) => {
                self.last_recovery_document = Some(self.editor.document().clone());
                self.recovery_checkpoint.clear();
                true
            }
            Err(error) => {
                self.error = Some(format!("Could not checkpoint recovery: {error}"));
                // A later edit or confirmed close can retry; avoid disk IO on every frame.
                self.recovery_checkpoint.clear();
                false
            }
        }
    }
    fn clear_recovery(&mut self) -> bool {
        // An unrelated New/Open/Save only owns this session's valid snapshot.
        // Unreadable startup recovery requires the separate explicit discard action.
        if !self.unreadable_recovery && !self.remove_recovery() {
            return false;
        }
        self.reset_recovery_tracking();
        true
    }
    fn remove_recovery(&mut self) -> bool {
        if let Some(store) = &self.workspace_store
            && let Err(error) = store.clear_recovery()
        {
            self.error = Some(format!("Could not clear recovery: {error}"));
            self.recovery_cleanup_pending = true;
            return false;
        }
        true
    }
    fn reset_recovery_tracking(&mut self) {
        self.last_recovery_document = None;
        self.recovery_cleanup_pending = false;
        self.recovery_checkpoint.clear();
        self.autosave_checkpoint.clear();
        self.observed_document = self.editor.document().clone();
    }
    fn new_document(&mut self) -> Result<(), String> {
        self.ensure_recovery_resolved()?;
        if !self.clear_recovery() {
            return Err(self.error.clone().unwrap());
        }
        self.capture_caret();
        self.editor = Editor::default();
        self.path = None;
        self.protected = None;
        self.warnings.clear();
        self.typing = None;
        self.notice.clear();
        self.focus_canvas = true;
        self.observed_document = self.editor.document().clone();
        Ok(())
    }
    fn ensure_recovery_resolved(&self) -> Result<(), String> {
        if self.pending_recovery.is_some() {
            Err("Resolve the startup recovery prompt before opening or saving documents".into())
        } else {
            Ok(())
        }
    }
    fn capture_caret(&mut self) {
        let Some(path) = self.path.as_deref() else {
            self.observed_caret = None;
            return;
        };
        let position = self.editor.selection().focus;
        if self
            .observed_caret
            .as_ref()
            .is_some_and(|(observed_path, observed_position)| {
                observed_path == path && *observed_position == position
            })
        {
            return;
        }
        if let Ok(stored) = StoredPath::from_path(path) {
            self.observed_caret = Some((path.to_path_buf(), position));
            // Aliases share one document history. Update every accessible alias so an
            // older exact-path entry cannot override the latest editing position.
            for caret in &mut self.workspace_state.carets {
                if caret
                    .path
                    .to_path()
                    .is_ok_and(|other| files::same_file(path, &other))
                {
                    caret.position = position;
                }
            }
            if self.workspace_state.caret_for(&stored) != Some(position) {
                self.workspace_state.set_caret(stored, position);
            }
        }
    }
    fn remembered_caret(&self, path: &Path, stored: &StoredPath) -> Option<Position> {
        self.workspace_state.caret_for(stored).or_else(|| {
            self.workspace_state.carets.iter().find_map(|caret| {
                caret
                    .path
                    .to_path()
                    .ok()
                    .filter(|other| files::same_file(path, other))
                    .map(|_| caret.position)
            })
        })
    }
    fn record_recent_path(&mut self, path: &Path, stored: StoredPath) {
        // Resolve identity only for accessible paths; disconnected entries remain intact.
        self.workspace_state.recent.retain(|recent| {
            !recent
                .path
                .to_path()
                .is_ok_and(|other| files::same_file(path, &other))
        });
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.workspace_state.record_recent(stored, timestamp);
    }
    fn request_open_path(&mut self, path: PathBuf, ctx: &egui::Context) {
        if let Err(error) = self.ensure_recovery_resolved() {
            self.error = Some(error);
            return;
        }
        self.open_target = Some(path);
        self.request(Pending::Open, ctx);
    }
    fn accept_open_choice(&mut self, path: Option<PathBuf>) {
        if let Some(path) = path
            && let Err(error) = self.open_path(&path)
        {
            self.error = Some(format!("Could not open {}: {error}", path.display()));
        }
    }
    fn open_document(&mut self, path: PathBuf) -> Result<(), String> {
        self.open_path(&path)
    }
    // The caller resolves unsaved changes before invoking this shared UI/MCP operation.
    fn open_path(&mut self, path: &Path) -> Result<(), String> {
        self.ensure_recovery_resolved()?;
        let report = files::open(path)?;
        let stored = StoredPath::from_path(path).map_err(|error| error.to_string())?;
        // Validate the incoming document before cleanup, and retain the current editor
        // until both validation and recovery cleanup have succeeded.
        let mut editor = self.editor.clone();
        editor
            .load_document(report.document)
            .map_err(|error| error.to_string())?;
        let caret = if self
            .path
            .as_deref()
            .is_some_and(|current| files::same_file(current, path))
        {
            Some(self.editor.selection().focus)
        } else {
            self.remembered_caret(path, &stored)
        };
        if let Some(position) = caret {
            // Invalid/stale positions leave load_document's valid initial caret intact.
            let _ = editor.set_selection(Selection::caret(position));
        }
        if !self.clear_recovery() {
            return Err(self.error.clone().unwrap());
        }
        self.capture_caret();
        self.editor = editor;
        self.protected = (!report.warnings.is_empty()).then(|| path.to_path_buf());
        self.path = Some(path.to_path_buf());
        self.warnings = report.warnings;
        self.typing = None;
        self.notice = "Document opened".into();
        self.focus_canvas = true;
        self.reveal = true;
        self.observed_document = self.editor.document().clone();
        self.record_recent_path(path, stored);
        self.capture_caret();
        Ok(())
    }
    fn save_document(&mut self, path: PathBuf) -> Result<(), String> {
        self.ensure_recovery_resolved()?;
        let stored = StoredPath::from_path(&path).map_err(|error| error.to_string())?;
        files::save(self.editor.document(), &path, self.protected.as_deref())?;
        self.capture_caret();
        self.editor.mark_saved();
        self.record_recent_path(&path, stored);
        self.path = Some(path);
        self.capture_caret();
        self.notice = "Saved".into();
        self.focus_canvas = true;
        if !self.clear_recovery() {
            let error = format!(
                "Document saved, but recovery cleanup failed: {}",
                self.error.as_deref().unwrap()
            );
            self.error = Some(error.clone());
            return Err(error);
        }
        Ok(())
    }
    fn process_checkpoints(&mut self, now: Instant, ctx: &egui::Context) {
        if self.allow_close {
            return;
        }
        if self
            .state_checkpoint
            .is_due(now, Duration::from_millis(500))
        {
            self.flush_workspace();
        }
        if self.recovery_checkpoint.is_due(now, Duration::from_secs(2)) {
            self.flush_recovery();
            ctx.request_repaint();
        }
        let eligible = self.editor.is_dirty() && self.path.is_some() && self.protected.is_none();
        if self.autosave_checkpoint.is_due(now, Duration::from_secs(5))
            && self.pending.is_none()
            && self.overwrite.is_none()
        {
            self.autosave_checkpoint.clear();
            if eligible {
                self.save_to(self.path.clone().unwrap(), ctx);
                ctx.request_repaint();
            }
        }
        let mut deadline = self
            .recovery_checkpoint
            .changed_at
            .map(|t| t + Duration::from_secs(2));
        if eligible
            && self.pending.is_none()
            && self.overwrite.is_none()
            && let Some(changed) = self.autosave_checkpoint.changed_at
        {
            let save_at = changed + Duration::from_secs(5);
            deadline = Some(deadline.map_or(save_at, |recovery| recovery.min(save_at)));
        }
        if let Some(changed) = self.state_checkpoint.changed_at {
            let state_at = changed + Duration::from_millis(500);
            deadline = Some(deadline.map_or(state_at, |other| other.min(state_at)));
        }
        if let Some(deadline) = deadline {
            ctx.request_repaint_after(deadline.saturating_duration_since(now));
        }
    }

    fn process_ai_requests(&mut self, ctx: &egui::Context) {
        let requests: Vec<_> = self
            .ai_bridge
            .as_ref()
            .map(|bridge| bridge.receiver.try_iter().take(8).collect())
            .unwrap_or_default();
        for request in requests {
            let result = if self.pending_recovery.is_some() {
                Err("Resolve the startup recovery prompt before using AI tools".into())
            } else {
                mcp::call(self, &request.name, request.args)
            };
            let _ = request.reply.send(result);
            ctx.request_repaint();
        }
    }
    fn ai_connection_window(&mut self, ctx: &egui::Context) {
        if !self.show_ai_connection {
            return;
        }
        let mut open = self.show_ai_connection;
        egui::Window::new("AI connection")
            .open(&mut open)
            .default_width(510.0)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading("Connect an AI assistant");
                ui.label("Use an MCP-compatible client to read and edit this document. AI edits appear here and share your undo history.");
                ui.add_space(10.0);
                if self.ai_bridge.is_some() {
                    ui.colored_label(theme::ACCENT, "Live access enabled");
                    if ui.add(IconButton::new(Icon::Close, "Disable live access")).clicked() {
                        self.ai_bridge = None;
                        self.notice = "AI connection disabled".into();
                    }
                } else if ui.add(IconButton::new(Icon::Connection, "Enable live access")).clicked() {
                    match mcp::Bridge::start(ctx.clone()) {
                        Ok(bridge) => self.ai_bridge = Some(bridge),
                        Err(error) => self.error = Some(format!("Could not enable AI connection: {error}")),
                    }
                }
                if let Some(bridge) = &self.ai_bridge {
                    ui.label("Copy this configuration into your AI client's MCP settings. It applies to this Folio window until you disable access or quit.");
                    let config = serde_json::to_string_pretty(&bridge.configuration()).unwrap();
                    if ui.add(IconButton::new(Icon::Copy, "Copy MCP configuration")).clicked() {
                        ctx.copy_text(config.clone());
                    }
                    ui.collapsing("View configuration", |ui| {
                        egui::ScrollArea::both().max_height(160.0).show(ui, |ui| {
                            ui.add(egui::Label::new(egui::RichText::new(config).monospace()).wrap_mode(egui::TextWrapMode::Extend));
                        });
                    });
                }
                ui.separator();
                ui.strong("Background documents");
                ui.label("For an independent session without a window, configure the Folio executable with the argument --mcp. Save the document before ending that session.");
                if ui.add(IconButton::new(Icon::Copy, "Copy background configuration")).clicked() {
                    let config = serde_json::json!({"mcpServers":{"folio-background":{"command":std::env::current_exe().unwrap_or_default().to_string_lossy(),"args":["--mcp"]}}});
                    ctx.copy_text(serde_json::to_string_pretty(&config).unwrap());
                }
            });
        self.show_ai_connection = open;
    }
    fn execute(&mut self, command: Command) {
        self.visual_line = None;
        match self.editor.execute(command) {
            Ok(outcome) => {
                if outcome.changed {
                    self.notice.clear();
                }
                self.reveal = true;
                self.preferred_x = None;
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    fn insert(&mut self, text: String) {
        self.execute(Command::ReplaceText {
            selection: self.editor.selection(),
            text,
            style: self.typing.clone(),
        });
    }
    fn current_style(&self) -> TextStyle {
        self.typing
            .clone()
            .unwrap_or_else(|| editing::style_at(&self.editor))
    }
    fn format(&mut self, patch: StylePatch) {
        if self.editor.selection().is_collapsed() {
            let mut s = self.current_style();
            if let Some(v) = patch.bold {
                s.bold = v;
            }
            if let Some(v) = patch.italic {
                s.italic = v;
            }
            if let Some(v) = patch.underline {
                s.underline = v;
            }
            if let Some(v) = patch.font_family {
                s.font_family = v;
            }
            if let Some(v) = patch.size_half_points {
                s.size_half_points = v;
            }
            if let Some(v) = patch.color {
                s.color = v;
            }
            self.typing = Some(s);
        } else {
            self.execute(Command::FormatRuns {
                selection: self.editor.selection(),
                patch,
            });
            self.typing = None;
        }
    }
    fn font_size_control(&mut self, ui: &mut egui::Ui, style: &TextStyle) -> egui::Response {
        let label = ui.label("Size");
        let mut size = style.size_half_points as f32 / 2.0;
        let response = ui
            .push_id("font-size", |ui| {
                ui.add(
                    egui::DragValue::new(&mut size)
                        .update_while_editing(false)
                        .range(6.0..=96.0)
                        .speed(0.5)
                        .suffix(" pt"),
                )
            })
            .inner
            .labelled_by(label.id)
            .on_hover_text("Font size in points");
        if response.changed() {
            self.format(StylePatch {
                size_half_points: Some((size * 2.0).round() as u16),
                ..Default::default()
            });
        }
        response
    }
    fn line_spacing_control(
        &mut self,
        ui: &mut egui::Ui,
        current: LineSpacing,
    ) -> Option<egui::Response> {
        let label = ui.label("Line spacing");
        let mut spacing = current;
        let text = match current {
            LineSpacing::Multiple(n) => format!("{}×", f64::from(n) / 100.0),
            LineSpacing::Exact(n) => format!("Exactly {} pt", f64::from(n) / 20.0),
            LineSpacing::AtLeast(n) => format!("At least {} pt", f64::from(n) / 20.0),
        };
        let height = match current {
            LineSpacing::Exact(n) | LineSpacing::AtLeast(n) => n,
            _ => 240,
        };
        egui::ComboBox::from_id_salt("spacing")
            .selected_text(text)
            .show_ui(ui, |ui| {
                for n in [100, 115, 150, 200] {
                    ui.selectable_value(
                        &mut spacing,
                        LineSpacing::Multiple(n),
                        format!("{}×", f64::from(n) / 100.0),
                    );
                }
                ui.separator();
                ui.selectable_value(&mut spacing, LineSpacing::Exact(height), "Exactly");
                ui.selectable_value(&mut spacing, LineSpacing::AtLeast(height), "At least");
            })
            .response
            .labelled_by(label.id);
        let mode_changed = spacing != current;
        let mut response = None;
        if let LineSpacing::Exact(height) | LineSpacing::AtLeast(height) = spacing {
            let label = ui.label("Height");
            let mut points = f64::from(height) / 20.0;
            let numeric = ui
                .push_id("line-spacing-height", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut points)
                            .update_while_editing(false)
                            .range(0.05..=f64::from(u32::MAX) / 20.0)
                            .speed(0.5)
                            .suffix(" pt"),
                    )
                })
                .inner
                .labelled_by(label.id)
                .on_hover_text("Line height in points (0.05 pt increments)");
            if numeric.changed() {
                let twips = (points * 20.0).round().clamp(1.0, f64::from(u32::MAX)) as u32;
                spacing = if matches!(spacing, LineSpacing::Exact(_)) {
                    LineSpacing::Exact(twips)
                } else {
                    LineSpacing::AtLeast(twips)
                };
            }
            response = Some(numeric);
        }
        if spacing != current {
            self.execute(Command::FormatParagraphs {
                selection: self.editor.selection(),
                patch: ParagraphPatch {
                    line_spacing: Some(spacing),
                    ..Default::default()
                },
            });
            if mode_changed {
                self.focus_canvas = true;
            }
        }
        response
    }
    fn toggle_theme(&mut self, ctx: &egui::Context) {
        self.dark_mode = !self.dark_mode;
        theme::install_mode(ctx, self.dark_mode);
        ctx.request_repaint();
    }
    fn toggle_focus_mode(&mut self) {
        self.focus_mode = !self.focus_mode;
        self.focus_canvas = true;
        self.show_document_info = false;
    }
    fn toggle_document_info(&mut self) {
        self.show_document_info = !self.show_document_info;
    }
    fn discard_pending(&mut self, pending: Pending, ctx: &egui::Context) {
        if !self.clear_recovery() {
            return;
        }
        self.pending = None;
        if matches!(pending, Pending::Quit) {
            if !self.flush_workspace() {
                return;
            }
            self.allow_close = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else {
            self.perform(pending, ctx);
        }
    }
    fn request(&mut self, pending: Pending, ctx: &egui::Context) {
        if self.pending_recovery.is_some() {
            return;
        }
        if !matches!(pending, Pending::Open) {
            self.open_target = None;
        }
        self.composition = None;
        if self.editor.is_dirty() {
            self.pending = Some(pending);
        } else {
            self.perform(pending, ctx);
        }
    }
    fn perform(&mut self, pending: Pending, ctx: &egui::Context) {
        match pending {
            Pending::New => {
                if let Err(error) = self.new_document() {
                    self.error = Some(error);
                }
            }
            Pending::Open => {
                let path = self.open_target.take().or_else(|| {
                    rfd::FileDialog::new()
                        .add_filter("Word document", &["docx"])
                        .pick_file()
                });
                self.accept_open_choice(path);
            }
            Pending::Quit => {
                self.capture_caret();
                if self.recovery_cleanup_pending && !self.clear_recovery() {
                    return;
                }
                if !self.flush_recovery() {
                    return;
                }
                if !self.flush_workspace() {
                    return;
                }
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
    // Returns true only after a fully successful save, never for a cancelled dialog.
    fn save(&mut self, save_as: bool, ctx: &egui::Context) -> bool {
        let choose = save_as
            || self.path.is_none()
            || self
                .protected
                .as_ref()
                .zip(self.path.as_ref())
                .is_some_and(|(a, b)| files::same_file(a, b));
        let path = if choose {
            let mut dialog = rfd::FileDialog::new().add_filter("Word document", &["docx"]);
            if let Some(path) = &self.path {
                if let Some(parent) = path.parent() {
                    dialog = dialog.set_directory(parent);
                }
                let name = path.file_stem().unwrap_or_default().to_string_lossy();
                dialog = dialog.set_file_name(if self.protected.is_some() {
                    format!("{name}-converted.docx")
                } else {
                    format!("{name}.docx")
                });
            } else {
                dialog = dialog.set_file_name("Untitled.docx");
            }
            let Some(mut path) = dialog.save_file() else {
                return false;
            };
            if path.extension().is_none() {
                path.set_extension("docx");
            }
            path
        } else {
            self.path.clone().unwrap()
        };
        if !path
            .extension()
            .is_some_and(|x| x.eq_ignore_ascii_case("docx"))
        {
            self.error = Some("Choose a .docx file. Other formats are not supported.".into());
            return false;
        }
        if self
            .protected
            .as_ref()
            .is_some_and(|p| files::same_file(p, &path))
        {
            self.error=Some("Choose a different file for this warned import. The original source is protected, including aliases.".into());
            return false;
        }
        if choose && path.exists() {
            self.overwrite = Some(path);
            return false;
        }
        self.save_to(path, ctx)
    }
    fn save_to(&mut self, path: PathBuf, ctx: &egui::Context) -> bool {
        match self.save_document(path.clone()) {
            Ok(()) => {
                if let Some(pending) = self.pending.take() {
                    self.perform(pending, ctx);
                }
                true
            }
            Err(error) => {
                self.error = Some(format!("Could not finish save {}: {error}", path.display()));
                false
            }
        }
    }
    fn action(&mut self, action: Action, ctx: &egui::Context) {
        match action {
            Action::New => self.request(Pending::New, ctx),
            Action::Open => self.request(Pending::Open, ctx),
            Action::Quit => self.request(Pending::Quit, ctx),
            Action::Save => {
                self.save(false, ctx);
            }
            Action::SaveAs => {
                self.save(true, ctx);
            }
            Action::Undo => {
                self.execute(Command::Undo);
                self.typing = None;
                self.focus_canvas = true;
            }
            Action::Redo => {
                self.execute(Command::Redo);
                self.typing = None;
                self.focus_canvas = true;
            }
            Action::Bold => self.format(StylePatch {
                bold: Some(!self.current_style().bold),
                ..Default::default()
            }),
            Action::Italic => self.format(StylePatch {
                italic: Some(!self.current_style().italic),
                ..Default::default()
            }),
            Action::Underline => self.format(StylePatch {
                underline: Some(!self.current_style().underline),
                ..Default::default()
            }),
            Action::ClearFormatting => {
                let style = TextStyle::default();
                self.format(StylePatch {
                    bold: Some(style.bold),
                    italic: Some(style.italic),
                    underline: Some(style.underline),
                    font_family: Some(style.font_family),
                    size_half_points: Some(style.size_half_points),
                    color: Some(style.color),
                });
                self.focus_canvas = true;
            }
            Action::Copy | Action::Cut => {
                if !self.editor.selection().is_collapsed() {
                    ctx.copy_text(editing::selected_text(&self.editor));
                    if action == Action::Cut {
                        self.execute(Command::Delete {
                            selection: self.editor.selection(),
                        });
                    }
                }
                self.focus_canvas = true;
            }
            Action::Paste => {
                self.composition = None;
                self.ime_enabled = false;
                self.focus_canvas = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
            }
            Action::SelectAll => {
                self.editor
                    .set_selection(editing::select_all(self.editor.document()))
                    .unwrap();
                self.typing = None;
            }
            Action::Find => {
                self.search_open = true;
                self.composition = None;
                self.ime_enabled = false;
                self.focus_search = true;
                self.focus_canvas = false;
                ctx.memory_mut(|m| m.request_focus(egui::Id::new("find-input")));
            }
            Action::PageBreak => {
                self.execute(Command::ReplaceWithPageBreak {
                    selection: self.editor.selection(),
                });
                self.focus_canvas = true;
            }
        }
        if matches!(action, Action::Bold | Action::Italic | Action::Underline) {
            self.focus_canvas = true;
        }
    }
    fn global_shortcuts(&mut self, ctx: &egui::Context) {
        if self.pending_recovery.is_some()
            || self.pending.is_some()
            || self.overwrite.is_some()
            || self.error.is_some()
            || self.ime_enabled
        {
            return;
        }
        for event in ctx.input(|i| i.events.clone()) {
            let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = event
            else {
                continue;
            };
            if modifiers.command && modifiers.shift && !modifiers.alt {
                let handled = match key {
                    Key::F => {
                        self.toggle_focus_mode();
                        true
                    }
                    Key::D => {
                        self.toggle_theme(ctx);
                        true
                    }
                    Key::I => {
                        if !self.focus_mode {
                            self.toggle_document_info();
                        }
                        true
                    }
                    _ => false,
                };
                if handled {
                    ctx.input_mut(|i| i.consume_key(modifiers, key));
                    continue;
                }
            }
            if key == Key::Escape && self.focus_mode {
                self.toggle_focus_mode();
                ctx.input_mut(|i| i.consume_key(modifiers, key));
                continue;
            }
            let Some(action) = editing::shortcut(key, modifiers) else {
                continue;
            };
            if !matches!(
                action,
                Action::New
                    | Action::Open
                    | Action::Save
                    | Action::SaveAs
                    | Action::Find
                    | Action::Quit
            ) {
                continue;
            }
            ctx.input_mut(|i| {
                i.consume_key(modifiers, key);
            });
            self.action(action, ctx);
            if self.pending_recovery.is_some() || self.pending.is_some() || self.error.is_some() {
                break;
            }
        }
    }
    fn focus_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("focus-bar")
            .frame(
                egui::Frame::new()
                    .fill(theme::surface(self.dark_mode))
                    .inner_margin(egui::Margin::symmetric(16, 6)),
            )
            .show(ctx, |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(IconButton::new(Icon::ExitFocus, "Exit focus mode").compact())
                        .on_hover_text("Exit focus mode (Escape or Command/Ctrl + Shift + F)")
                        .clicked()
                    {
                        self.toggle_focus_mode();
                    }
                    ui.label(
                        egui::RichText::new("Focus mode")
                            .small()
                            .color(theme::muted(self.dark_mode)),
                    );
                    if self.ai_bridge.is_some()
                        && ui
                            .add(IconButton::new(Icon::Connection, "AI access enabled").compact())
                            .on_hover_text("Live AI access is enabled. Open connection settings.")
                            .clicked()
                    {
                        self.show_ai_connection = true;
                    }
                });
            });
    }
    fn document_info_panel(&mut self, ctx: &egui::Context) {
        if !self.show_document_info || self.focus_mode {
            return;
        }
        egui::SidePanel::right("document-info")
            .resizable(false)
            .default_width(228.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::surface(self.dark_mode))
                    .inner_margin(egui::Margin::symmetric(18, 16)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Document info")
                            .strong()
                            .color(theme::text(self.dark_mode)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(IconButton::new(Icon::Close, "Close document info").compact())
                            .clicked()
                        {
                            self.toggle_document_info();
                        }
                    });
                });
                ui.separator();
                let total = editing::document_statistics(self.editor.document());
                ui.label(
                    egui::RichText::new("Document")
                        .small()
                        .color(theme::muted(self.dark_mode)),
                );
                ui.label(format!("{} pages", self.pages));
                ui.label(format!("{} words", total.words));
                ui.label(format!("{} characters", total.characters));
                ui.add_space(14.0);
                ui.label(
                    egui::RichText::new("Selection")
                        .small()
                        .color(theme::muted(self.dark_mode)),
                );
                if self.editor.selection().is_collapsed() {
                    ui.label("No selection");
                } else {
                    let selected = editing::selection_statistics(&self.editor);
                    ui.label(format!("{} words", selected.words));
                    ui.label(format!("{} characters", selected.characters));
                }
                ui.add_space(14.0);
                ui.label(
                    egui::RichText::new(format!("Page {} of {}", self.active_page, self.pages))
                        .color(theme::muted(self.dark_mode)),
                );
            });
    }
    fn recent_documents_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.add_space(6.0);
        ui.menu_button("Recent Documents", |ui| {
            ui.set_max_width(360.0);
            if self.workspace_state.recent.is_empty() {
                ui.colored_label(
                    theme::muted(self.dark_mode),
                    "Your opened documents will appear here.",
                );
            }
            self.recent_entries_ui(ui, ctx);
        });
    }
    fn recent_entries_ui(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
    ) -> egui::scroll_area::ScrollAreaOutput<Vec<egui::Id>> {
        let height =
            (ctx.screen_rect().bottom() - ui.next_widget_position().y - 16.0).clamp(44.0, 320.0);
        egui::ScrollArea::vertical()
            .id_salt("recent_document_entries")
            .max_height(height)
            .show(ui, |ui| {
                let mut controls = Vec::new();
                for recent in self.workspace_state.recent.clone() {
                    let Ok(path) = recent.path.to_path() else {
                        continue;
                    };
                    let presentation = workspace::recent_document_presentation(&path);
                    let row = ui.push_id(&path, |ui| {
                        ui.horizontal(|ui| {
                            let location = if presentation.available {
                                presentation.location.clone()
                            } else {
                                format!("Unavailable • {}", presentation.location)
                            };
                            let width = (ui.available_width() - 36.0).clamp(60.0, 300.0);
                            let open = ui
                                .add_enabled(
                                    presentation.available,
                                    RecentDocumentButton {
                                        name: &presentation.name,
                                        location: &location,
                                        width,
                                    },
                                )
                                .on_hover_text(path.display().to_string());
                            if open.clicked() {
                                ui.close();
                                self.request_open_path(path.clone(), ctx);
                            }
                            let remove = ui.add(
                                IconButton::new(Icon::Close, "Remove from recent documents")
                                    .compact(),
                            );
                            if remove.gained_focus() {
                                remove.scroll_to_me(Some(egui::Align::Center));
                            }
                            if remove.clicked() {
                                self.workspace_state.remove_recent(&recent.path);
                            }
                            [open.id, remove.id]
                        })
                        .inner
                    });
                    controls.extend(row.inner);
                }
                controls
            })
    }
    fn ribbon(&mut self, ctx: &egui::Context) {
        let frame = egui::Frame::new()
            .fill(theme::surface(self.dark_mode))
            .inner_margin(egui::Margin::symmetric(24, 12));
        let panel = egui::TopBottomPanel::top("ribbon").frame(frame);
        panel.show(ctx, |ui| {
            if self.pending_recovery.is_some() || self.pending.is_some() || self.overwrite.is_some() || self.error.is_some() {
                ui.disable();
            }
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    egui::RichText::new("folio.")
                        .font(egui::FontId::new(
                            26.0,
                            egui::FontFamily::Name("Serif-Regular".into()),
                        ))
                        .color(theme::text(self.dark_mode)),
                );
                ui.separator();
                for (label, action, enabled) in [
                    ("New", Action::New, true),
                    ("Open", Action::Open, true),
                    ("Save", Action::Save, true),
                    ("Undo", Action::Undo, self.editor.can_undo()),
                    ("Redo", Action::Redo, self.editor.can_redo()),
                ] {
                    if action == Action::Undo {
                        ui.separator();
                    }
                    let button = if action == Action::Save {
                        IconButton::new(Icon::Save, label).primary()
                    } else {
                        IconButton::new(Icon::for_action(action), label).compact()
                    };
                    let command = if cfg!(target_os = "macos") {
                        "⌘"
                    } else {
                        "Ctrl+"
                    };
                    let key = match action {
                        Action::New => "N",
                        Action::Open => "O",
                        Action::Save => "S",
                        Action::Undo => "Z",
                        Action::Redo => "Shift+Z",
                        _ => "",
                    };
                    if ui
                        .add_enabled(enabled, button)
                        .on_hover_text(format!("{label} ({command}{key})"))
                        .clicked()
                    {
                        self.action(action, ctx);
                    }
                }
                ui.separator();
                let name = self
                    .path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or("Untitled".into());
                ui.add(egui::Label::new(egui::RichText::new(name).strong()).truncate());
                if self.editor.is_dirty() {
                    ui.colored_label(theme::muted(self.dark_mode), "• Unsaved");
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                for (label, tab) in [
                    ("File", Tab::File),
                    ("Home", Tab::Home),
                    ("Layout", Tab::Layout),
                    ("View", Tab::View),
                ] {
                    if ui.selectable_label(self.tab == tab, label).clicked() {
                        self.tab = tab;
                    }
                }
            });
            ui.separator();
            match self.tab {
                Tab::File => {
                    ui.horizontal_wrapped(|ui| {
                        for (label, action) in [
                            ("New document", Action::New),
                            ("Open DOCX…", Action::Open),
                            ("Save", Action::Save),
                            ("Save As…", Action::SaveAs),
                        ] {
                            if ui
                                .add(IconButton::new(Icon::for_action(action), label))
                                .clicked()
                            {
                                self.action(action, ctx);
                            }
                        }
                    });
                    ui.label("DOCX • A warned import always saves as a converted copy.");
                    self.recent_documents_ui(ui, ctx);
                }
                Tab::Home => {
                    ui.horizontal_wrapped(|ui| {
                        for (label, action, key) in [
                            ("Cut", Action::Cut, "X"),
                            ("Copy", Action::Copy, "C"),
                            ("Paste", Action::Paste, "V"),
                        ] {
                            let enabled =
                                action == Action::Paste || !self.editor.selection().is_collapsed();
                            let command = if cfg!(target_os = "macos") {
                                "⌘"
                            } else {
                                "Ctrl+"
                            };
                            if ui
                                .add_enabled(
                                    enabled,
                                    IconButton::new(Icon::for_action(action), label).compact(),
                                )
                                .on_hover_text(format!("{label} ({command}{key})"))
                                .clicked()
                            {
                                self.action(action, ctx);
                            }
                        }
                        ui.separator();
                        if ui
                            .add(
                                IconButton::new(Icon::ClearFormatting, "Clear formatting")
                                    .compact(),
                            )
                            .on_hover_text(
                                "Reset text to Noto Sans, 12 pt, black; keep paragraph layout",
                            )
                            .clicked()
                        {
                            self.action(Action::ClearFormatting, ctx);
                        }
                        ui.separator();
                        let style = self.current_style();
                        for (label, selected, action) in [
                            ("Bold", style.bold, Action::Bold),
                            ("Italic", style.italic, Action::Italic),
                            ("Underline", style.underline, Action::Underline),
                        ] {
                            if ui
                                .add(
                                    IconButton::new(Icon::for_action(action), label)
                                        .selected(selected)
                                        .compact(),
                                )
                                .on_hover_text(format!("Toggle {label}"))
                                .clicked()
                            {
                                self.action(action, ctx);
                            }
                        }
                        ui.separator();
                        let mut family = if layout::is_serif_family(&style.font_family) {
                            "serif"
                        } else {
                            "sans-serif"
                        };
                        egui::ComboBox::from_id_salt("font-family")
                            .selected_text(if family == "serif" {
                                "Noto Serif"
                            } else {
                                "Noto Sans"
                            })
                            .show_ui(ui, |ui| {
                                if ui
                                    .selectable_value(&mut family, "sans-serif", "Noto Sans")
                                    .changed()
                                {
                                    self.format(StylePatch {
                                        font_family: Some(family.into()),
                                        ..Default::default()
                                    });
                                }
                                if ui
                                    .selectable_value(&mut family, "serif", "Noto Serif")
                                    .changed()
                                {
                                    self.format(StylePatch {
                                        font_family: Some(family.into()),
                                        ..Default::default()
                                    });
                                }
                            });
                        self.font_size_control(ui, &style);

                        let mut rgb = [style.color.red, style.color.green, style.color.blue];
                        if ui
                            .scope(|ui| {
                                ui.spacing_mut().interact_size = Vec2::splat(22.0);
                                ui.color_edit_button_srgb(&mut rgb)
                                    .on_hover_text("Text color")
                                    .changed()
                            })
                            .inner
                        {
                            self.format(StylePatch {
                                color: Some(Color::rgb(rgb[0], rgb[1], rgb[2])),
                                ..Default::default()
                            });
                        }
                        ui.separator();

                        let alignment = self
                            .editor
                            .document()
                            .paragraph(self.editor.selection().focus.block)
                            .unwrap()
                            .style
                            .alignment;
                        for (label, value) in [
                            ("Left", Alignment::Left),
                            ("Center", Alignment::Center),
                            ("Right", Alignment::Right),
                            ("Justify", Alignment::Justify),
                        ] {
                            let icon = match value {
                                Alignment::Left => Icon::AlignLeft,
                                Alignment::Center => Icon::AlignCenter,
                                Alignment::Right => Icon::AlignRight,
                                Alignment::Justify => Icon::AlignJustify,
                            };
                            if ui
                                .add(
                                    IconButton::new(icon, label)
                                        .selected(alignment == value)
                                        .compact(),
                                )
                                .on_hover_text(format!("Align paragraph {label}"))
                                .clicked()
                            {
                                self.execute(Command::FormatParagraphs {
                                    selection: self.editor.selection(),
                                    patch: ParagraphPatch {
                                        alignment: Some(value),
                                        ..Default::default()
                                    },
                                });
                                self.focus_canvas = true;
                            }
                        }
                        if ui
                            .add(IconButton::new(Icon::Find, "Find / Replace"))
                            .on_hover_text("Find and replace text (Command/Ctrl + F)")
                            .clicked()
                        {
                            if self.search_open {
                                self.search_open = false;
                                self.focus_canvas = true;
                            } else {
                                self.action(Action::Find, ctx);
                            }
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        let p = self
                            .editor
                            .document()
                            .paragraph(self.editor.selection().focus.block)
                            .unwrap()
                            .style
                            .clone();
                        for (label, current, before) in [
                            ("Before", p.space_before_twips, true),
                            ("After", p.space_after_twips, false),
                        ] {
                            let accessible_label = ui.label(label);
                            let mut points = current as f32 / 20.0;
                            if ui
                                .add(
                                    egui::DragValue::new(&mut points)
                                        .update_while_editing(false)
                                        .range(0.0..=144.0)
                                        .suffix(" pt"),
                                )
                                .labelled_by(accessible_label.id)
                                .on_hover_text(format!("Paragraph spacing {label}"))
                                .changed()
                            {
                                let mut patch = ParagraphPatch::default();
                                if before {
                                    patch.space_before_twips = Some((points * 20.0) as u32)
                                } else {
                                    patch.space_after_twips = Some((points * 20.0) as u32)
                                }
                                self.execute(Command::FormatParagraphs {
                                    selection: self.editor.selection(),
                                    patch,
                                });
                            }
                        }
                        self.line_spacing_control(ui, p.line_spacing);
                    });
                }
                Tab::Layout => {
                    ui.horizontal_wrapped(|ui| {
                        let mut page = self.editor.document().page_layout.clone();
                        let old = page.clone();
                        ui.label("Paper");
                        egui::ComboBox::from_id_salt("paper")
                            .selected_text(if page.size == PageSize::LETTER {
                                "Letter"
                            } else if page.size == PageSize::A4 {
                                "A4"
                            } else {
                                "Custom"
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut page.size, PageSize::A4, "A4");
                                ui.selectable_value(&mut page.size, PageSize::LETTER, "Letter");
                            });
                        ui.selectable_value(
                            &mut page.orientation,
                            Orientation::Portrait,
                            "Portrait",
                        );
                        ui.selectable_value(
                            &mut page.orientation,
                            Orientation::Landscape,
                            "Landscape",
                        );
                        for (label, margin) in [
                            ("Top", &mut page.margins.top),
                            ("Bottom", &mut page.margins.bottom),
                            ("Left", &mut page.margins.left),
                            ("Right", &mut page.margins.right),
                        ] {
                            let accessible_label = ui.label(label);
                            let mut inches = *margin as f32 / 1440.0;
                            if ui
                                .add(
                                    egui::DragValue::new(&mut inches)
                                        .update_while_editing(false)
                                        .range(0.0..=5.0)
                                        .speed(0.05)
                                        .suffix(" in"),
                                )
                                .labelled_by(accessible_label.id)
                                .on_hover_text(format!("{label} page margin"))
                                .changed()
                            {
                                *margin = (inches * 1440.0).round() as u32;
                            }
                        }
                        if page != old {
                            self.execute(Command::SetPageLayout { layout: page });
                        }
                        if ui
                            .add(IconButton::new(Icon::PageBreak, "Page break"))
                            .on_hover_text("Insert explicit page break (Command/Ctrl + Enter)")
                            .clicked()
                        {
                            self.action(Action::PageBreak, ctx);
                        }
                    });
                }
                Tab::View => {
                    ui.horizontal(|ui| {
                        ui.label("Zoom");
                        ui.add(
                            egui::Slider::new(&mut self.zoom, 0.25..=2.5)
                                .custom_formatter(|n, _| format!("{:.0}%", n * 100.0)),
                        );
                        if ui.add(IconButton::new(Icon::Zoom, "100%")).clicked() {
                            self.zoom = 1.0;
                        }
                        if ui
                            .add(IconButton::new(Icon::FitWidth, "Fit page width"))
                            .clicked()
                        {
                            self.zoom = ((ctx.screen_rect().width() - 80.0)
                                / (self
                                    .editor
                                    .document()
                                    .page_layout
                                    .effective_size()
                                    .width_twips as f32
                                    / 15.0))
                                .clamp(0.25, 2.5);
                        }
                    });
                    ui.separator();
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .add(IconButton::new(
                                if self.focus_mode { Icon::ExitFocus } else { Icon::Focus },
                                if self.focus_mode { "Exit focus mode" } else { "Focus mode" },
                            ))
                            .on_hover_text("Hide editing chrome for distraction-free writing")
                            .clicked()
                        {
                            self.toggle_focus_mode();
                        }
                        if ui
                            .add(IconButton::new(
                                if self.dark_mode { Icon::Sun } else { Icon::Moon },
                                if self.dark_mode { "Light appearance" } else { "Dark appearance" },
                            ))
                            .on_hover_text("Switch between light and dark appearance (Command/Ctrl + Shift + D)")
                            .clicked()
                        {
                            self.toggle_theme(ctx);
                        }
                        if ui
                            .add(IconButton::new(Icon::Info, "Document info"))
                            .on_hover_text("Show document and selection statistics (Command/Ctrl + Shift + I)")
                            .clicked()
                        {
                            self.toggle_document_info();
                        }
                        if ui
                            .add(IconButton::new(Icon::Connection, "AI connection"))
                            .on_hover_text("Connect an MCP-compatible AI client to Folio")
                            .clicked()
                        {
                            self.show_ai_connection = true;
                        }
                    });
                }
            }
            if self.search_open {
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    let label = ui.label("Find");
                    let search_id = egui::Id::new("find-input");
                    if self.focus_search && ui.is_enabled() {
                        ui.memory_mut(|m| m.request_focus(search_id));
                    }
                    let search = ui
                        .add(
                            egui::TextEdit::singleline(&mut self.needle)
                                .id(search_id)
                                .hint_text("Literal, case-sensitive text")
                                .desired_width(180.0),
                        )
                        .labelled_by(label.id);
                    if self.focus_search && ui.is_enabled() {
                        search.request_focus();
                        self.focus_search = false;
                    }
                    let label = ui.label("Replace with");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.replacement)
                            .id(egui::Id::new("replace-input"))
                            .hint_text("Replacement")
                            .desired_width(160.0),
                    )
                    .labelled_by(label.id);
                    if ui
                        .add_enabled(
                            !self.needle.is_empty(),
                            IconButton::new(Icon::Find, "Find next"),
                        )
                        .clicked()
                    {
                        self.find_next();
                    }
                    if ui
                        .add_enabled(!self.needle.is_empty(), egui::Button::new("Replace"))
                        .clicked()
                    {
                        if editing::selected_text(&self.editor) == self.needle {
                            self.execute(Command::ReplaceText {
                                selection: self.editor.selection(),
                                text: self.replacement.clone(),
                                style: None,
                            });
                        }
                        self.find_next();
                    }
                    if ui
                        .add_enabled(!self.needle.is_empty(), egui::Button::new("Replace all"))
                        .clicked()
                    {
                        match self.editor.execute(Command::ReplaceAll {
                            needle: self.needle.clone(),
                            replacement: self.replacement.clone(),
                        }) {
                            Ok(result) => {
                                self.notice = format!("{} replacements", result.replacements);
                                self.reveal = true;
                                self.typing = None;
                                self.focus_canvas = true;
                            }
                            Err(e) => self.error = Some(e.to_string()),
                        }
                    }
                    if ui.add(IconButton::new(Icon::Close, "Close find")).clicked() {
                        self.search_open = false;
                        self.focus_canvas = true;
                    }
                });
            }
        });
    }
    fn find_next(&mut self) {
        match self.editor.document().find(&self.needle) {
            Ok(matches) => {
                let end = self.editor.selection().ordered().1;
                if let Some(s) = matches.iter().find(|s| s.anchor >= end).or(matches.first()) {
                    self.editor.set_selection(*s).unwrap();
                    self.typing = None;
                    self.reveal = true;
                    self.focus_canvas = true;
                    self.notice = format!("{} matches", matches.len());
                } else {
                    self.notice = "No matches".into();
                }
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    fn keyboard(&mut self, ctx: &egui::Context, layout: &mut DocumentLayout, focused: bool) {
        if !focused {
            self.composition = None;
            self.ime_enabled = false;
        }
        let events = ctx.input(|i| i.events.clone());
        for event in events {
            if self.pending_recovery.is_some()
                || self.pending.is_some()
                || self.overwrite.is_some()
                || self.error.is_some()
            {
                break;
            }
            let mut handled = false;
            match event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    if let Some(action) = editing::shortcut(key, modifiers) {
                        let global = matches!(
                            action,
                            Action::New
                                | Action::Open
                                | Action::Save
                                | Action::SaveAs
                                | Action::Find
                                | Action::Quit
                        );
                        if (focused || global) && !self.ime_enabled {
                            self.action(action, ctx);
                            handled = true;
                        }
                    } else if focused && !self.ime_enabled {
                        let at = self.editor.selection().focus;
                        let extend = modifiers.shift;
                        match key {
                            Key::ArrowLeft | Key::ArrowRight => {
                                let forward = key == Key::ArrowRight;
                                let next = if !extend && !self.editor.selection().is_collapsed() {
                                    let (a, b) = self.editor.selection().ordered();
                                    if forward { b } else { a }
                                } else if modifiers.mac_cmd {
                                    layout.edge_with_hint(at, forward, self.visual_line)
                                } else if modifiers.alt || modifiers.ctrl {
                                    word_edge(self.editor.document(), at, forward)
                                } else {
                                    editing::adjacent(self.editor.document(), at, forward)
                                };
                                self.visual_line = if modifiers.mac_cmd {
                                    layout.visual_line(at, self.visual_line)
                                } else {
                                    None
                                };
                                move_to(&mut self.editor, next, extend);
                                self.preferred_x = None;
                                self.typing = None;
                                self.reveal = true;
                                handled = true;
                            }
                            Key::ArrowUp | Key::ArrowDown | Key::PageUp | Key::PageDown => {
                                let x = self.preferred_x.unwrap_or_else(|| {
                                    layout
                                        .caret_with_hint(at, self.visual_line)
                                        .map_or(0.0, |r| r.left())
                                });
                                let delta = match key {
                                    Key::ArrowUp => -1,
                                    Key::ArrowDown => 1,
                                    Key::PageUp => -((600.0 * self.zoom / 20.0) as isize),
                                    _ => (600.0 * self.zoom / 20.0) as isize,
                                };
                                let next = if modifiers.mac_cmd {
                                    let (a, b) =
                                        editing::select_all(self.editor.document()).ordered();
                                    if delta < 0 { a } else { b }
                                } else {
                                    {
                                        let (at, hint) = layout.vertical_with_hint(
                                            at,
                                            delta,
                                            x,
                                            self.visual_line,
                                        );
                                        self.visual_line = hint;
                                        at
                                    }
                                };
                                move_to(&mut self.editor, next, extend);
                                self.preferred_x = Some(x);
                                self.typing = None;
                                self.reveal = true;
                                handled = true;
                            }
                            Key::Home | Key::End => {
                                let next = if modifiers.command || modifiers.ctrl {
                                    let (a, b) =
                                        editing::select_all(self.editor.document()).ordered();
                                    if key == Key::Home { a } else { b }
                                } else {
                                    layout.edge_with_hint(at, key == Key::End, self.visual_line)
                                };
                                self.visual_line = if modifiers.command || modifiers.ctrl {
                                    None
                                } else {
                                    layout.visual_line(at, self.visual_line)
                                };
                                move_to(&mut self.editor, next, extend);
                                self.preferred_x = None;
                                self.typing = None;
                                self.reveal = true;
                                handled = true;
                            }
                            Key::Backspace | Key::Delete => {
                                let forward = key == Key::Delete;
                                let command = if self.editor.selection().is_collapsed()
                                    && (modifiers.mac_cmd || modifiers.alt || modifiers.ctrl)
                                {
                                    let target = if modifiers.mac_cmd {
                                        layout.edge_with_hint(at, forward, self.visual_line)
                                    } else {
                                        word_edge(self.editor.document(), at, forward)
                                    };
                                    Command::Delete {
                                        selection: Selection::new(at, target),
                                    }
                                } else {
                                    editing::delete_command(&self.editor, forward)
                                };
                                self.execute(command);
                                handled = true;
                            }
                            Key::Enter => {
                                self.insert("\n".into());
                                handled = true;
                            }
                            Key::Tab => {
                                self.insert("\t".into());
                                handled = true;
                            }
                            Key::Escape => {
                                self.composition = None;
                                self.ime_enabled = false;
                                handled = true;
                            }
                            _ => {}
                        }
                    }
                    if handled {
                        ctx.input_mut(|i| {
                            i.consume_key(modifiers, key);
                        });
                    }
                }
                egui::Event::Text(text) if focused && !self.ime_enabled => {
                    self.insert(text);
                    handled = true;
                }
                egui::Event::Paste(text) if focused => {
                    self.composition = None;
                    self.insert(text);
                    handled = true;
                }
                egui::Event::Copy | egui::Event::Cut if focused => {
                    self.action(
                        if matches!(event, egui::Event::Cut) {
                            Action::Cut
                        } else {
                            Action::Copy
                        },
                        ctx,
                    );
                    handled = true;
                }
                egui::Event::Ime(ime) if focused => {
                    match ime {
                        egui::ImeEvent::Enabled => {
                            self.ime_enabled = true;
                        }
                        egui::ImeEvent::Preedit(text) => {
                            self.ime_enabled = true;
                            self.composition = Some(text);
                        }
                        egui::ImeEvent::Commit(text) => {
                            self.composition = None;
                            self.ime_enabled = false;
                            if !text.is_empty() {
                                self.insert(text);
                            }
                        }
                        egui::ImeEvent::Disabled => {
                            self.composition = None;
                            self.ime_enabled = false;
                        }
                    }
                    handled = true;
                }
                _ => {}
            }
            if handled {
                *layout = DocumentLayout::build(ctx, self.editor.document(), self.zoom);
            }
        }
    }
    #[cfg(test)]
    fn canvas(&mut self, ctx: &egui::Context) {
        let layout = self.canvas_state(ctx);
        self.paint_canvas(ctx, layout);
    }
    fn canvas_state(&mut self, ctx: &egui::Context) -> DocumentLayout {
        let id = egui::Id::new("document-canvas");
        let mut layout = DocumentLayout::build(ctx, self.editor.document(), self.zoom);
        let focused = ctx.memory(|m| m.has_focus(id));
        self.keyboard(ctx, &mut layout, focused);
        self.pages = layout.pages.len();
        self.active_page = layout
            .visual_line(self.editor.selection().focus, self.visual_line)
            .map_or(1, |i| layout.lines[i].page + 1);
        layout
    }
    fn paint_canvas(&mut self, ctx: &egui::Context, layout: DocumentLayout) {
        let id = egui::Id::new("document-canvas");
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(theme::workspace(self.dark_mode))
                    .inner_margin(24.0),
            )
            .show(ctx, |ui| {
                if self.pending_recovery.is_some()
                    || self.pending.is_some()
                    || self.overwrite.is_some()
                    || self.error.is_some()
                {
                    ui.disable();
                }
                egui::ScrollArea::both()
                    .id_salt("pages")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let extra = ((ui.available_width() - layout.size.x) * 0.5).max(0.0);
                        let (allocated, _) = ui.allocate_exact_size(
                            Vec2::new(layout.size.x + extra * 2.0, layout.size.y),
                            egui::Sense::hover(),
                        );
                        let origin = allocated.min + Vec2::new(extra, 0.0);
                        let rect = Rect::from_min_size(origin, layout.size);
                        let response = ui.interact(rect, id, egui::Sense::click_and_drag());
                        response.widget_info(|| {
                            let value = self
                                .editor
                                .document()
                                .blocks
                                .iter()
                                .filter_map(|b| {
                                    if let Block::Paragraph(p) = b {
                                        Some(p.text())
                                    } else {
                                        None
                                    }
                                })
                                .collect::<Vec<_>>()
                                .join("\n");
                            let mut info = egui::WidgetInfo::text_edit(
                                true,
                                &value,
                                &value,
                                "Document canvas",
                            );
                            info.label = Some("Editable paginated document".into());
                            info
                        });
                        if self.focus_canvas && ui.is_enabled() {
                            response.request_focus();
                            self.focus_canvas = false;
                        }
                        if response.clicked() || response.drag_started() {
                            response.request_focus();
                            self.composition = None;
                            self.ime_enabled = false;
                            if let Some(pointer) = response.interact_pointer_pos()
                                && let Some((at, line)) =
                                    layout.hit_line((pointer - origin).to_pos2())
                            {
                                let selection_before = self.editor.selection();
                                move_to(&mut self.editor, at, ctx.input(|i| i.modifiers.shift));
                                self.visual_line = Some(line);
                                self.typing = None;
                                self.preferred_x = None;
                                if self.editor.selection() != selection_before {
                                    ctx.request_repaint();
                                }
                            }
                        }
                        if response.double_clicked() {
                            let at = self.editor.selection().focus;
                            let a = word_edge(self.editor.document(), at, false);
                            let b = word_edge(self.editor.document(), at, true);
                            let selection_before = self.editor.selection();
                            self.editor.set_selection(Selection::new(a, b)).unwrap();
                            if self.editor.selection() != selection_before {
                                ctx.request_repaint();
                            }
                        } else if response.dragged()
                            && let Some(pointer) = response.interact_pointer_pos()
                            && let Some((at, line)) = layout.hit_line((pointer - origin).to_pos2())
                        {
                            let selection_before = self.editor.selection();
                            move_to(&mut self.editor, at, true);
                            self.visual_line = Some(line);
                            self.typing = None;
                            if self.editor.selection() != selection_before {
                                ctx.request_repaint();
                            }
                            if pointer.y < ui.clip_rect().top() + 20.0 {
                                ui.scroll_with_delta(Vec2::new(0.0, 15.0));
                            } else if pointer.y > ui.clip_rect().bottom() - 20.0 {
                                ui.scroll_with_delta(Vec2::new(0.0, -15.0));
                            }
                        }
                        let painter = ui.painter();
                        for (i, page) in layout.pages.iter().enumerate() {
                            let page = page.translate(origin.to_vec2());
                            painter.rect_filled(
                                page.translate(Vec2::new(0.0, 6.0)).expand(2.0),
                                4.0,
                                Color32::from_black_alpha(8),
                            );
                            painter.rect_filled(page, 1.0, Color32::WHITE);
                            painter.rect_stroke(
                                page,
                                1.0,
                                Stroke::new(1.0, theme::border(self.dark_mode)),
                                egui::StrokeKind::Inside,
                            );
                            painter.text(
                                page.right_bottom() - Vec2::new(20.0, 12.0),
                                egui::Align2::RIGHT_BOTTOM,
                                format!("{}", i + 1),
                                egui::FontId::proportional(10.0 * self.zoom),
                                theme::muted(self.dark_mode),
                            );
                        }
                        for rect in layout.selection_rects(self.editor.selection()) {
                            painter.rect_filled(
                                rect.translate(origin.to_vec2()),
                                0.0,
                                theme::TEXT_SELECTION,
                            );
                        }
                        for line in &layout.lines {
                            let line_rect = line.rect.translate(origin.to_vec2());
                            if !ui.clip_rect().intersects(line_rect) {
                                continue;
                            }
                            let page = layout.pages[line.page].translate(origin.to_vec2());
                            painter
                                .with_clip_rect(page.intersect(ui.clip_rect()))
                                .galley(line_rect.min, line.galley.clone(), Color32::BLACK);
                        }
                        if let Some(caret) =
                            layout.caret_with_hint(self.editor.selection().focus, self.visual_line)
                        {
                            let caret = caret.translate(origin.to_vec2());
                            if self.reveal {
                                ui.scroll_to_rect(caret.expand(24.0), None);
                                self.reveal = false;
                            }
                            if response.has_focus() {
                                painter.line_segment(
                                    [caret.left_top(), caret.left_bottom()],
                                    Stroke::new(1.5, Color32::from_rgb(25, 98, 91)),
                                );
                                ctx.output_mut(|o| {
                                    o.mutable_text_under_cursor = true;
                                    o.ime = Some(egui::output::IMEOutput {
                                        rect,
                                        cursor_rect: caret,
                                    });
                                });
                                if let Some(text) = &self.composition {
                                    let at = caret.left_bottom();
                                    let galley = painter.layout_no_wrap(
                                        text.clone(),
                                        egui::FontId::proportional(16.0 * self.zoom),
                                        Color32::BLACK,
                                    );
                                    let pre = Rect::from_min_size(at, galley.size());
                                    painter.rect_filled(pre, 0.0, Color32::from_rgb(237, 246, 243));
                                    painter.galley(at, galley, Color32::BLACK);
                                    painter.line_segment(
                                        [pre.left_bottom(), pre.right_bottom()],
                                        Stroke::new(1.0, Color32::DARK_GREEN),
                                    );
                                }
                            }
                        }
                        if response.hovered() {
                            ctx.set_cursor_icon(egui::CursorIcon::Text);
                        }
                    });
            });
    }
    fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(snapshot) = self
            .pending_recovery
            .as_ref()
            .filter(|_| self.error.is_none())
        {
            let captured = snapshot.captured_unix_seconds;
            let elapsed = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .saturating_sub(captured);
            let age = if elapsed < 60 {
                "just now".into()
            } else if elapsed < 3600 {
                format!("{} minutes ago", elapsed / 60)
            } else if elapsed < 86400 {
                format!("{} hours ago", elapsed / 3600)
            } else {
                format!("{} days ago", elapsed / 86400)
            };
            egui::Window::new("Recover your writing")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.set_max_width(420.0);
                    ui.label("Folio found an unsaved document from a previous session.");
                    ui.label(format!("Captured {age}")).on_hover_text(format!(
                        "Capture time: {captured} seconds since the Unix epoch"
                    ));
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add(IconButton::new(Icon::Open, "Restore").primary())
                            .clicked()
                        {
                            self.restore_startup_recovery();
                        }
                        if ui.button("Discard recovery").clicked() {
                            self.discard_startup_recovery();
                        }
                    });
                });
        }
        if let Some(pending) = self
            .pending
            .filter(|_| self.overwrite.is_none() && self.error.is_none())
        {
            egui::Window::new("Unsaved changes")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label("Save your changes before continuing?");
                    ui.horizontal(|ui| {
                        if ui.button("Save changes").clicked() {
                            self.save(false, ctx);
                        }
                        if ui.button("Discard changes").clicked() {
                            self.discard_pending(pending, ctx);
                        }
                        if ui.button("Cancel").clicked() {
                            self.pending = None;
                            self.open_target = None;
                            self.focus_canvas = true;
                        }
                    });
                });
        }
        if let Some(path) = self.overwrite.clone() {
            egui::Window::new("Replace existing file?")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(path.display().to_string());
                    ui.horizontal(|ui| {
                        if ui.button("Replace file").clicked() {
                            self.overwrite = None;
                            self.save_to(path, ctx);
                        }
                        if ui.button("Cancel replacement").clicked() {
                            self.overwrite = None;
                        }
                    });
                });
        }
        if let Some(error) = self.error.clone() {
            egui::Window::new("Folio — operation failed")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.set_max_width(500.0);
                    ui.label(error);
                    if self.unreadable_recovery {
                        ui.add_space(8.0);
                        ui.label("Your recovery file is unchanged. You can close this error and continue opening or saving documents. Discard recovery permanently deletes the unreadable file.");
                        if ui.button("Discard recovery").clicked()
                            && self.discard_startup_recovery()
                        {
                            self.error = None;
                        }
                    }
                    if ui.button("Close error").clicked() {
                        self.error = None;
                        self.focus_canvas = true;
                    }
                });
        }
        if !self.warnings.is_empty() {
            let frame = egui::Frame::new()
                .fill(Color32::from_rgb(255, 248, 232))
                .inner_margin(egui::Margin::symmetric(16, 10));
            let panel = egui::TopBottomPanel::bottom("import-warnings").frame(frame);
            panel.show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("Some document features could not be preserved").strong().color(theme::WARNING));
                    let enabled = self.pending.is_none() && self.overwrite.is_none() && self.error.is_none();
                    if ui.add_enabled(enabled, IconButton::new(Icon::SaveAs, "Save converted copy…").primary()).clicked() {
                        self.save(true, ctx);
                    }
                });
                ui.label("Your original file is protected. Save the converted document to a different file.");
                let count = self.warnings.len();
                let label = if count == 1 { "warning" } else { "warnings" };
                egui::CollapsingHeader::new(format!("{count} {label}")).show(
                    ui,
                    |ui| {
                        egui::ScrollArea::vertical()
                            .max_height(140.0)
                            .show(ui, |ui| {
                                for w in &self.warnings {
                                    ui.label(format!(
                                        "{}{}",
                                        w.message,
                                        w.location
                                            .as_ref()
                                            .map(|l| format!(" ({l})"))
                                            .unwrap_or_default()
                                    ));
                                }
                            });
                    },
                );
            });
        }
    }
}
fn word_edge(doc: &Document, at: Position, forward: bool) -> Position {
    let p = doc.paragraph(at.block).unwrap();
    let text = p.text();
    let mut next = editing::adjacent(doc, at, forward);
    if next.block != at.block {
        return next;
    }
    // Word navigation respects the core's grapheme boundaries.
    let word = |offset: usize| {
        text[offset..]
            .chars()
            .next()
            .is_some_and(|c| c.is_alphanumeric() || c == '_')
    };
    let mut category = if forward {
        word(at.offset)
    } else {
        word(next.offset)
    };
    loop {
        let candidate = editing::adjacent(doc, next, forward);
        if candidate == next || candidate.block != at.block {
            break;
        }
        let current = if forward {
            word(next.offset)
        } else {
            word(candidate.offset)
        };
        if category && !current {
            break;
        }
        if !category && current {
            category = true;
        }
        next = candidate;
    }
    next
}
impl eframe::App for FolioApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_ai_requests(ctx);
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if self.pending_recovery.is_some() {
                // Closing the launch prompt preserves the snapshot for the next launch.
                if !self.flush_workspace() {
                    return;
                }
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else {
                self.request(Pending::Quit, ctx);
            }
        }
        self.global_shortcuts(ctx);
        if self.focus_mode {
            self.focus_bar(ctx);
        } else {
            self.ribbon(ctx);
        }
        // A pending destructive operation blocks document/ribbon input until resolved.
        self.dialogs(ctx);
        let layout = self.canvas_state(ctx);
        if !self.focus_mode {
            let frame = egui::Frame::new()
                .fill(theme::surface(self.dark_mode))
                .inner_margin(egui::Margin::symmetric(16, 8));
            let panel = egui::TopBottomPanel::bottom("status").frame(frame);
            panel.show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                let total = editing::document_statistics(self.editor.document());
                ui.label(format!("Page {} of {}", self.active_page, self.pages));
                ui.separator();
                let (icon_rect, _) = ui.allocate_exact_size(Vec2::splat(16.0), egui::Sense::hover());
                Icon::Statistics.paint(ui.painter(), icon_rect, theme::muted(self.dark_mode));
                ui.label(format!("{} words • {} characters", total.words, total.characters))
                    .on_hover_text("Words are separated by whitespace. Characters include spaces and count each grapheme once; paragraph/page breaks are excluded.");
                if !self.editor.selection().is_collapsed() {
                    let selected = editing::selection_statistics(&self.editor);
                    ui.separator();
                    ui.label(format!("Selected: {} words • {} characters", selected.words, selected.characters));
                }
                if self.ai_bridge.is_some() && ui
                    .add(IconButton::new(Icon::Connection, "AI access enabled"))
                    .on_hover_text("Live AI access is enabled. Open connection settings to disable it.")
                    .clicked()
                {
                    self.show_ai_connection = true;
                }
                ui.separator();
                let (status, color) = if self.editor.is_dirty() {
                    ("Unsaved changes", theme::ACCENT)
                } else if self
                    .protected
                    .as_ref()
                    .zip(self.path.as_ref())
                    .is_some_and(|(a, b)| files::same_file(a, b))
                {
                    ("Imported • Save a copy", theme::WARNING)
                } else if self.path.is_none() {
                    ("New document", theme::muted(self.dark_mode))
                } else {
                    ("Saved", theme::muted(self.dark_mode))
                };
                ui.colored_label(color, status);
                if !self.notice.is_empty() && self.notice != status {
                    ui.label(
                        egui::RichText::new(&self.notice)
                            .small()
                            .color(theme::muted(self.dark_mode)),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(
                        egui::Slider::new(&mut self.zoom, 0.25..=2.5)
                            .custom_formatter(|n, _| format!("{:.0}%", n * 100.0)),
                    )
                    .on_hover_text("Document zoom");
                });
            });
            });
        }
        self.document_info_panel(ctx);
        self.paint_canvas(ctx, layout);
        self.ai_connection_window(ctx);
        let title = format!(
            "{}{} — Folio",
            self.path
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or("Untitled".into()),
            if self.editor.is_dirty() { " •" } else { "" }
        );
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        let now = Instant::now();
        self.schedule_checkpoints(now);
        self.process_checkpoints(now, ctx);
    }
}
fn main() -> eframe::Result {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if !arguments.is_empty() {
        let result = match arguments.as_slice() {
            [mode] if mode == "--mcp" => mcp::background(),
            [mode, address] if mode == "--mcp-connect" => match std::env::var("FOLIO_MCP_TOKEN") {
                Ok(token) => mcp::connect(address, &token),
                Err(_) => Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Missing FOLIO_MCP_TOKEN; copy configuration from View → AI connection",
                )),
            },
            _ => {
                eprintln!("Usage: folio-desktop [--mcp | --mcp-connect ADDRESS]");
                std::process::exit(2);
            }
        };
        if let Err(error) = result {
            eprintln!("Folio MCP: {error}");
            std::process::exit(1);
        }
        return Ok(());
    }
    eframe::run_native(
        "Folio",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1180.0, 850.0])
                .with_min_inner_size([700.0, 500.0]),
            ..Default::default()
        },
        Box::new(|cc| {
            layout::install_fonts(&cc.egui_ctx);
            let app = FolioApp::from_user_profile();
            if app.dark_mode {
                theme::install_mode(&cc.egui_ctx, true);
            } else {
                theme::install(&cc.egui_ctx);
            }
            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
mod app_tests {
    use super::*;
    struct RecoveryDirectory(PathBuf);
    impl RecoveryDirectory {
        fn new() -> Self {
            static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            Self(std::env::temp_dir().join(format!(
                "folio-recovery-app-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            )))
        }
        fn app(&self) -> FolioApp {
            FolioApp {
                workspace_store: Some(workspace::WorkspaceStore::at(self.0.clone())),
                ..Default::default()
            }
        }
    }
    impl Drop for RecoveryDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn lifecycle_file(dir: &RecoveryDirectory, name: &str) -> PathBuf {
        let path = dir.0.join(name);
        std::fs::create_dir_all(&dir.0).unwrap();
        files::save(&Document::default(), &path, None).unwrap();
        path
    }
    #[test]
    fn twelve_recent_entries_scroll_and_keyboard_focus_reaches_every_control_at_minimum_viewport() {
        let dir = RecoveryDirectory::new();
        let mut app = dir.app();
        for index in 0..12 {
            let path = lifecycle_file(&dir, &format!("document-{index}.docx"));
            app.workspace_state
                .record_recent(StoredPath::from_path(&path).unwrap(), index);
        }
        let ctx = egui::Context::default();
        let mut focused = std::collections::HashSet::new();
        let mut max_offset: f32 = 0.0;
        let mut scroll_extent = 0.0;
        let mut controls = Vec::new();
        for frame in 0..60 {
            let events = if frame > 1 && frame % 2 == 0 {
                vec![key(Key::Tab, egui::Modifiers::NONE)]
            } else {
                vec![]
            };
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(700., 500.))),
                    time: Some(frame as f64 * 0.5),
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::Area::new(egui::Id::new("recent_test_popup"))
                        .fixed_pos(egui::pos2(24., 170.))
                        .show(ctx, |ui| {
                            ui.set_max_width(360.);
                            let output = app.recent_entries_ui(ui, ctx);
                            assert!(output.inner_rect.bottom() <= 500.);
                            assert!(output.inner_rect.height() <= 320.);
                            max_offset = max_offset.max(output.state.offset.y);
                            scroll_extent = output.content_size.y - output.inner_rect.height();
                            controls = output.inner;
                        });
                },
            );
            if let Some(id) = ctx.memory(|memory| memory.focused()) {
                focused.insert(id);
            }
        }
        assert_eq!(controls.len(), 24);
        assert!(
            controls.iter().all(|id| focused.contains(id)),
            "all twelve open and remove controls must be reachable"
        );
        assert!(scroll_extent > 200., "the test must actually overflow");
        assert!(
            max_offset >= scroll_extent - 1.,
            "keyboard focus must reveal the final row: offset={max_offset}, extent={scroll_extent}"
        );
    }
    #[test]
    fn preferences_and_caret_share_debounce_and_survive_restart() {
        let dir = RecoveryDirectory::new();
        let path = lifecycle_file(&dir, "preferences.docx");
        let mut app = dir.app();
        app.open_path(&path).unwrap();
        let start = Instant::now();
        app.dark_mode = true;
        app.zoom = 20.0;
        app.schedule_checkpoints(start);
        app.process_checkpoints(
            start + Duration::from_millis(499),
            &egui::Context::default(),
        );
        assert!(
            app.workspace_store
                .as_ref()
                .unwrap()
                .load_state()
                .unwrap()
                .is_none()
        );
        app.zoom = 1.5;
        app.insert("hello".into());
        app.schedule_checkpoints(start + Duration::from_millis(300));
        app.process_checkpoints(
            start + Duration::from_millis(799),
            &egui::Context::default(),
        );
        assert!(
            app.workspace_store
                .as_ref()
                .unwrap()
                .load_state()
                .unwrap()
                .is_none()
        );
        app.process_checkpoints(
            start + Duration::from_millis(800),
            &egui::Context::default(),
        );
        let restored = FolioApp::with_workspace_store(WorkspaceStore::at(dir.0.clone()));
        assert!(restored.dark_mode);
        assert_eq!(restored.zoom, 1.5);
        assert_eq!(
            restored
                .workspace_state
                .caret_for(&StoredPath::from_path(&path).unwrap()),
            Some(Position::new(0, 5))
        );
        assert!(app.state_checkpoint.changed_at.is_none());
    }
    #[test]
    fn confirmed_close_flushes_preferences_and_failed_write_keeps_window_open() {
        let dir = RecoveryDirectory::new();
        let mut app = dir.app();
        app.zoom = 10.0;
        app.perform(Pending::Quit, &egui::Context::default());
        assert!(app.allow_close);
        assert_eq!(
            app.workspace_store
                .as_ref()
                .unwrap()
                .load_state()
                .unwrap()
                .unwrap()
                .zoom,
            2.5
        );
        let blocked = RecoveryDirectory::new();
        std::fs::write(&blocked.0, "not a directory").unwrap();
        let mut app = blocked.app();
        app.dark_mode = true;
        app.perform(Pending::Quit, &egui::Context::default());
        assert!(!app.allow_close);
        assert!(
            app.error
                .as_deref()
                .unwrap()
                .contains("Could not save workspace")
        );
        assert_ne!(app.saved_workspace, app.workspace_state);
    }
    #[test]
    fn unchanged_workspace_does_not_create_state_and_corrupt_state_is_preserved() {
        let dir = RecoveryDirectory::new();
        let mut app = dir.app();
        assert!(app.flush_workspace());
        assert!(!dir.0.exists());
        std::fs::create_dir_all(&dir.0).unwrap();
        std::fs::write(dir.0.join("workspace.json"), "broken").unwrap();
        app.dark_mode = true;
        assert!(!app.flush_workspace());
        assert_eq!(
            std::fs::read_to_string(dir.0.join("workspace.json")).unwrap(),
            "broken"
        );
    }
    #[test]
    fn successful_open_adds_recent_but_cancelled_dialog_does_not() {
        let dir = RecoveryDirectory::new();
        let path = lifecycle_file(&dir, "open.docx");
        let mut app = dir.app();
        app.open_path(&path).unwrap();
        assert_eq!(app.workspace_state.recent[0].path.to_path().unwrap(), path);
        let state = app.workspace_state.clone();
        app.accept_open_choice(None);
        assert_eq!(app.workspace_state, state);
    }
    #[test]
    fn recent_open_waits_for_unsaved_change_decision() {
        let dir = RecoveryDirectory::new();
        let path = lifecycle_file(&dir, "recent.docx");
        let mut app = dir.app();
        app.insert("unsaved".into());
        let before = app.editor.document().clone();
        let ctx = egui::Context::default();
        app.request_open_path(path.clone(), &ctx);
        assert!(matches!(app.pending, Some(Pending::Open)));
        assert_eq!(app.editor.document(), &before);
        assert!(app.workspace_state.recent.is_empty());
        app.discard_pending(Pending::Open, &ctx);
        assert_eq!(app.path, Some(path));
        assert!(!app.editor.is_dirty());
    }
    #[test]
    fn successful_save_adds_recent_and_clears_recovery() {
        let dir = RecoveryDirectory::new();
        let mut app = dir.app();
        app.insert("save".into());
        assert!(app.flush_recovery());
        let path = dir.0.join("saved.docx");
        app.save_document(path.clone()).unwrap();
        assert_eq!(app.workspace_state.recent[0].path.to_path().unwrap(), path);
        assert!(
            app.workspace_store
                .as_ref()
                .unwrap()
                .load_recovery()
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn failed_save_retains_recovery() {
        let dir = RecoveryDirectory::new();
        let mut app = dir.app();
        app.insert("save".into());
        assert!(app.flush_recovery());
        assert!(app.save_document(dir.0.join("absent/saved.docx")).is_err());
        assert!(app.workspace_state.recent.is_empty());
        assert!(
            app.workspace_store
                .as_ref()
                .unwrap()
                .load_recovery()
                .unwrap()
                .is_some()
        );
    }
    #[test]
    fn same_file_alias_does_not_duplicate_recent() {
        let dir = RecoveryDirectory::new();
        let path = lifecycle_file(&dir, "original.docx");
        let alias = dir.0.join("alias.docx");
        std::fs::hard_link(&path, &alias).unwrap();
        let mut app = dir.app();
        app.open_path(&path).unwrap();
        app.open_path(&alias).unwrap();
        assert_eq!(app.workspace_state.recent.len(), 1);
        assert_eq!(app.workspace_state.recent[0].path.to_path().unwrap(), alias);
    }
    #[test]
    fn missing_recent_path_is_retained() {
        let dir = RecoveryDirectory::new();
        let missing = dir.0.join("missing.docx");
        let mut app = dir.app();
        app.workspace_state
            .record_recent(StoredPath::from_path(&missing).unwrap(), 1);
        let state = app.workspace_state.clone();
        assert!(app.open_path(&missing).is_err());
        assert_eq!(app.workspace_state, state);
    }
    #[test]
    fn stale_caret_restores_as_valid_caret() {
        let dir = RecoveryDirectory::new();
        let path = lifecycle_file(&dir, "caret.docx");
        let mut app = dir.app();
        app.workspace_state.set_caret(
            StoredPath::from_path(&path).unwrap(),
            Position::new(90, 900),
        );
        app.open_path(&path).unwrap();
        assert_eq!(
            app.editor.selection(),
            Selection::caret(Position::new(0, 0))
        );
    }
    #[test]
    fn remembered_caret_tracks_focus_and_restores_across_aliases() {
        let dir = RecoveryDirectory::new();
        let mut app = dir.app();
        app.insert("hello 🌻".into());
        let path = dir.0.join("caret.docx");
        std::fs::create_dir_all(&dir.0).unwrap();
        app.save_document(path.clone()).unwrap();
        let alias = dir.0.join("alias.docx");
        std::fs::hard_link(&path, &alias).unwrap();
        app.editor
            .set_selection(Selection {
                anchor: Position::new(0, 0),
                focus: Position::new(0, 5),
            })
            .unwrap();
        app.schedule_checkpoints(Instant::now());
        assert_eq!(
            app.workspace_state
                .caret_for(&StoredPath::from_path(&path).unwrap()),
            Some(Position::new(0, 5))
        );
        app.new_document().unwrap();
        app.open_path(&alias).unwrap();
        assert_eq!(
            app.editor.selection(),
            Selection::caret(Position::new(0, 5))
        );
        app.new_document().unwrap();
        app.workspace_state
            .set_caret(StoredPath::from_path(&path).unwrap(), Position::new(0, 7));
        app.open_path(&path).unwrap();
        assert_eq!(
            app.editor.selection(),
            Selection::caret(Position::new(0, 0))
        );
    }
    #[test]
    fn alias_caret_history_returns_to_original_at_latest_position() {
        let dir = RecoveryDirectory::new();
        let mut app = dir.app();
        app.insert("abcdef".into());
        std::fs::create_dir_all(&dir.0).unwrap();
        let original = dir.0.join("original.docx");
        app.save_document(original.clone()).unwrap();
        let alias = dir.0.join("alias.docx");
        std::fs::hard_link(&original, &alias).unwrap();
        app.editor
            .set_selection(Selection::caret(Position::new(0, 2)))
            .unwrap();
        app.schedule_checkpoints(Instant::now());
        app.open_path(&alias).unwrap();
        app.editor
            .set_selection(Selection::caret(Position::new(0, 5)))
            .unwrap();
        app.schedule_checkpoints(Instant::now());
        let missing = StoredPath::from_path(&dir.0.join("disconnected.docx")).unwrap();
        app.workspace_state
            .set_caret(missing.clone(), Position::new(8, 9));
        app.new_document().unwrap();
        app.open_path(&original).unwrap();
        assert_eq!(
            app.editor.selection(),
            Selection::caret(Position::new(0, 5))
        );
        assert_eq!(
            app.workspace_state.caret_for(&missing),
            Some(Position::new(8, 9))
        );
    }
    #[test]
    fn recent_open_preserves_unresolved_startup_recovery() {
        let dir = RecoveryDirectory::new();
        let path = lifecycle_file(&dir, "target.docx");
        let mut app = dir.app();
        app.insert("recovered".into());
        assert!(app.flush_recovery());
        let mut startup = FolioApp::with_workspace_store(WorkspaceStore::at(dir.0.clone()));
        startup.request_open_path(path.clone(), &egui::Context::default());
        assert!(startup.pending_recovery.is_some());
        assert!(startup.pending.is_none());
        assert!(startup.open_target.is_none());
        assert!(startup.workspace_state.recent.is_empty());
        assert!(startup.open_path(&path).is_err());
        assert!(
            startup
                .workspace_store
                .as_ref()
                .unwrap()
                .load_recovery()
                .unwrap()
                .is_some()
        );
    }
    #[test]
    fn startup_restores_theme_zoom_and_recents() {
        let dir = RecoveryDirectory::new();
        let store = WorkspaceStore::at(dir.0.clone());
        let mut state = workspace::WorkspaceState {
            dark_mode: true,
            zoom: 1.75,
            ..Default::default()
        };
        state.record_recent(
            StoredPath::from_path(&dir.0.join("recent.docx")).unwrap(),
            42,
        );
        store.save_state(&state).unwrap();
        let app = FolioApp::with_workspace_store(store);
        assert!(app.dark_mode);
        assert_eq!(app.zoom, 1.75);
        assert_eq!(app.workspace_state, state);
        assert!(FolioApp::default().workspace_store.is_none());
    }
    fn captured_draft(dir: &RecoveryDirectory) -> RecoverySnapshot {
        let mut app = dir.app();
        app.editor
            .execute(Command::ReplaceText {
                selection: app.editor.selection(),
                text: "Recovered draft".into(),
                style: None,
            })
            .unwrap();
        app.path = Some(dir.0.join("draft.docx"));
        app.protected = app.path.clone();
        app.warnings = vec![ImportWarning {
            code: WarningCode::UnsupportedFeature,
            feature: Feature::Tables,
            location: Some("word/document.xml".into()),
            message: "Table omitted".into(),
        }];
        app.editor
            .set_selection(Selection {
                anchor: Position::new(0, 2),
                focus: Position::new(0, 8),
            })
            .unwrap();
        app.recovery_snapshot().unwrap()
    }
    #[test]
    fn recovery_prompt_waits_for_explicit_restore_or_discard() {
        let dir = RecoveryDirectory::new();
        let store = WorkspaceStore::at(dir.0.clone());
        let snapshot = captured_draft(&dir);
        store.save_recovery(&snapshot).unwrap();
        let mut app = FolioApp::with_workspace_store(store);
        assert_eq!(app.editor.document(), &Document::default());
        assert!(!app.editor.is_dirty());
        assert_eq!(app.pending_recovery.as_ref(), Some(&snapshot));
        let ctx = egui::Context::default();
        layout::install_fonts(&ctx);
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::Text("accidental edit".into())],
        );
        assert_eq!(app.editor.document(), &Document::default());
        assert_eq!(app.pending_recovery.as_ref(), Some(&snapshot));
        assert!(app.discard_startup_recovery());
        assert!(app.pending_recovery.is_none());
        assert!(!app.editor.is_dirty());
        assert!(
            app.workspace_store
                .as_ref()
                .unwrap()
                .load_recovery()
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn recovery_restores_editor_as_dirty() {
        let dir = RecoveryDirectory::new();
        let store = WorkspaceStore::at(dir.0.clone());
        let snapshot = captured_draft(&dir);
        store.save_recovery(&snapshot).unwrap();
        let mut app = FolioApp::with_workspace_store(store);
        assert!(app.restore_startup_recovery());
        assert!(app.pending_recovery.is_none());
        assert_eq!(app.editor.document(), &snapshot.document);
        assert_eq!(app.editor.selection(), snapshot.selection);
        assert!(app.editor.is_dirty());
        assert!(!app.editor.can_undo());
        assert_eq!(app.path, Some(dir.0.join("draft.docx")));
        assert_eq!(app.protected, app.path);
        assert_eq!(app.warnings, snapshot.warnings);
        assert!(
            app.workspace_store
                .as_ref()
                .unwrap()
                .load_recovery()
                .unwrap()
                .is_some()
        );
    }
    #[test]
    fn corrupt_recovery_does_not_block_startup() {
        let dir = RecoveryDirectory::new();
        std::fs::create_dir_all(&dir.0).unwrap();
        std::fs::write(dir.0.join("recovery.json"), b"{broken").unwrap();
        std::fs::write(dir.0.join("workspace.json"), b"{broken state").unwrap();
        let app = FolioApp::with_workspace_store(WorkspaceStore::at(dir.0.clone()));
        assert!(app.pending_recovery.is_none());
        assert_eq!(app.editor.document(), &Document::default());
        let error = app.error.unwrap();
        assert!(error.contains("workspace"));
        assert!(error.contains("recovery"));
        assert_eq!(
            std::fs::read(dir.0.join("recovery.json")).unwrap(),
            b"{broken"
        );
        assert_eq!(
            std::fs::read(dir.0.join("workspace.json")).unwrap(),
            b"{broken state"
        );
    }
    fn unreadable_recovery_bytes() -> [&'static [u8]; 2] {
        [
            b"{broken\n  keep these bytes",
            b"{\"schema_version\":999,\"data\":{\"future\":true}}\n",
        ]
    }
    #[test]
    fn unreadable_recovery_survives_new_open_and_save_after_error_dismissal() {
        for bytes in unreadable_recovery_bytes() {
            for operation in ["new", "open", "save"] {
                let dir = RecoveryDirectory::new();
                let target = lifecycle_file(&dir, "unrelated.docx");
                let recovery = dir.0.join("recovery.json");
                std::fs::write(&recovery, bytes).unwrap();
                let mut app = FolioApp::with_workspace_store(WorkspaceStore::at(dir.0.clone()));
                assert!(app.error.take().unwrap().contains("recovery"));
                match operation {
                    "new" => {
                        app.insert("discard this session only".into());
                        app.discard_pending(Pending::New, &egui::Context::default());
                        assert_eq!(app.editor.document(), &Document::default());
                        assert!(app.path.is_none());
                    }
                    "open" => {
                        app.open_path(&target).unwrap();
                        assert_eq!(app.path, Some(target));
                    }
                    "save" => {
                        app.insert("saved document".into());
                        app.save_document(target.clone()).unwrap();
                        assert!(!app.editor.is_dirty());
                        assert_eq!(
                            files::open(&target).unwrap().document,
                            *app.editor.document()
                        );
                    }
                    _ => unreachable!(),
                }
                assert!(app.error.is_none(), "{operation}");
                assert_eq!(std::fs::read(&recovery).unwrap(), bytes, "{operation}");
                app.insert("new checkpoint".into());
                assert!(!app.flush_recovery(), "{operation}");
                assert!(app.error.as_ref().unwrap().contains("checkpoint recovery"));
                assert_eq!(std::fs::read(&recovery).unwrap(), bytes, "{operation}");
            }
        }
    }
    #[test]
    fn unreadable_recovery_can_be_explicitly_discarded_from_error_dialog() {
        for bytes in unreadable_recovery_bytes() {
            let dir = RecoveryDirectory::new();
            std::fs::create_dir_all(&dir.0).unwrap();
            let recovery = dir.0.join("recovery.json");
            std::fs::write(&recovery, bytes).unwrap();
            let mut app = FolioApp::with_workspace_store(WorkspaceStore::at(dir.0.clone()));
            let ctx = egui::Context::default();
            let mut discard_position = None;
            for _ in 0..2 {
                let output = ctx.run(egui::RawInput::default(), |ctx| app.dialogs(ctx));
                discard_position = output.shapes.iter().find_map(|shape| {
                    if let egui::Shape::Text(text) = &shape.shape
                        && text.galley.job.text == "Discard recovery"
                    {
                        Some(text.pos + text.galley.size() * 0.5)
                    } else {
                        None
                    }
                });
            }
            assert_eq!(std::fs::read(&recovery).unwrap(), bytes);
            let position =
                discard_position.expect("unreadable recovery needs an explicit discard button");
            for pressed in [true, false] {
                let _ = ctx.run(
                    egui::RawInput {
                        events: vec![
                            egui::Event::PointerMoved(position),
                            egui::Event::PointerButton {
                                pos: position,
                                button: egui::PointerButton::Primary,
                                pressed,
                                modifiers: egui::Modifiers::NONE,
                            },
                        ],
                        ..Default::default()
                    },
                    |ctx| app.dialogs(ctx),
                );
            }
            assert!(!recovery.exists());
            assert!(app.error.is_none());
            app.insert("recover this session".into());
            assert!(app.flush_recovery());
            assert_eq!(
                app.workspace_store
                    .as_ref()
                    .unwrap()
                    .load_recovery()
                    .unwrap()
                    .unwrap()
                    .document,
                *app.editor.document()
            );
        }
    }
    #[test]
    fn failed_unreadable_recovery_discard_keeps_preservation_until_retry_succeeds() {
        let dir = RecoveryDirectory::new();
        std::fs::create_dir_all(&dir.0).unwrap();
        let recovery = dir.0.join("recovery.json");
        let bytes = unreadable_recovery_bytes()[0];
        std::fs::write(&recovery, bytes).unwrap();
        let mut app = FolioApp::with_workspace_store(WorkspaceStore::at(dir.0.clone()));
        std::fs::remove_file(&recovery).unwrap();
        std::fs::create_dir(&recovery).unwrap();
        std::fs::write(recovery.join("sentinel"), bytes).unwrap();
        assert!(!app.discard_startup_recovery());
        assert!(
            app.error
                .as_ref()
                .unwrap()
                .contains("Could not clear recovery")
        );
        assert!(app.unreadable_recovery);
        assert_eq!(std::fs::read(recovery.join("sentinel")).unwrap(), bytes);
        std::fs::remove_dir_all(&recovery).unwrap();
        std::fs::write(&recovery, bytes).unwrap();
        app.error = None;
        app.new_document().unwrap();
        assert_eq!(std::fs::read(&recovery).unwrap(), bytes);
        assert!(app.discard_startup_recovery());
        assert!(!recovery.exists());
        assert!(!app.unreadable_recovery);
    }
    #[test]
    fn invalid_recovery_document_is_reported_and_preserved() {
        let dir = RecoveryDirectory::new();
        let store = WorkspaceStore::at(dir.0.clone());
        let mut snapshot = captured_draft(&dir);
        snapshot.document.blocks.clear();
        // Write invalid data directly to model a damaged file, bypassing app validation.
        std::fs::create_dir_all(&dir.0).unwrap();
        let bytes = serde_json::to_vec(&serde_json::json!({"schema_version": 1, "data": snapshot}))
            .unwrap();
        std::fs::write(dir.0.join("recovery.json"), &bytes).unwrap();
        let app = FolioApp::with_workspace_store(store);
        assert!(app.pending_recovery.is_none());
        assert!(
            app.error
                .as_ref()
                .unwrap()
                .contains("Invalid recovered document")
        );
        assert!(
            app.workspace_store
                .as_ref()
                .unwrap()
                .save_recovery(&captured_draft(&dir))
                .is_err()
        );
        assert_eq!(std::fs::read(dir.0.join("recovery.json")).unwrap(), bytes);
    }
    #[test]
    fn failed_startup_discard_retains_prompt_and_snapshot() {
        let dir = RecoveryDirectory::new();
        let store = WorkspaceStore::at(dir.0.clone());
        store.save_recovery(&captured_draft(&dir)).unwrap();
        let mut app = FolioApp::with_workspace_store(store);
        std::fs::remove_file(dir.0.join("recovery.json")).unwrap();
        std::fs::create_dir(dir.0.join("recovery.json")).unwrap();
        assert!(!app.discard_startup_recovery());
        assert!(app.pending_recovery.is_some());
        assert!(app.error.is_some());
        assert!(!app.editor.is_dirty());
    }
    #[test]
    fn recovery_snapshot_round_trips_warning_guard_and_selection() {
        let dir = RecoveryDirectory::new();
        let mut app = dir.app();
        app.insert("draft".into());
        app.path = Some(PathBuf::from("copy.docx"));
        app.protected = Some(PathBuf::from("source.docx"));
        app.warnings = vec![ImportWarning {
            code: WarningCode::UnsupportedFeature,
            feature: Feature::Tables,
            location: Some("word/document.xml".into()),
            message: "Table omitted".into(),
        }];
        app.editor
            .set_selection(Selection {
                anchor: Position::new(0, 1),
                focus: Position::new(0, 4),
            })
            .unwrap();
        assert!(app.flush_recovery());
        let snapshot = app
            .workspace_store
            .as_ref()
            .unwrap()
            .load_recovery()
            .unwrap()
            .unwrap();
        assert_eq!(
            snapshot.path.unwrap().to_path().unwrap(),
            PathBuf::from("copy.docx")
        );
        assert_eq!(
            snapshot.protected_source.unwrap().to_path().unwrap(),
            PathBuf::from("source.docx")
        );
        assert_eq!(snapshot.warnings[0].message, "Table omitted");
        assert_eq!(snapshot.selection.anchor, Position::new(0, 1));
        assert_eq!(snapshot.selection.focus, Position::new(0, 4));
    }
    #[test]
    fn recovery_mcp_save_new_and_open_clear_snapshots_and_cache() {
        for operation in [
            "folio_save_document",
            "folio_new_document",
            "folio_open_document",
        ] {
            let dir = RecoveryDirectory::new();
            let mut app = dir.app();
            app.insert("prior".into());
            assert!(app.flush_recovery());
            let target = dir.0.join("target.docx");
            let arguments = if operation == "folio_new_document" {
                serde_json::json!({"discard_unsaved":true})
            } else {
                if operation == "folio_open_document" {
                    files::save(&Document::default(), &target, None).unwrap();
                }
                serde_json::json!({"path":target, "discard_unsaved":true})
            };
            // Save has a separate strict argument schema.
            let arguments = if operation == "folio_save_document" {
                serde_json::json!({"path":target})
            } else {
                arguments
            };
            mcp::call(&mut app, operation, arguments).unwrap();
            assert!(
                app.workspace_store
                    .as_ref()
                    .unwrap()
                    .load_recovery()
                    .unwrap()
                    .is_none(),
                "{operation}"
            );
            assert!(app.last_recovery_document.is_none(), "{operation}");
            // Reusing exactly the prior document after a transition must write a fresh snapshot.
            if operation != "folio_save_document" {
                app.insert("prior".into());
                assert!(app.flush_recovery());
                assert!(
                    app.workspace_store
                        .as_ref()
                        .unwrap()
                        .load_recovery()
                        .unwrap()
                        .is_some()
                );
            }
        }
    }
    #[test]
    fn recovery_cleanup_failure_blocks_save_and_quit_but_keeps_disk_save() {
        let dir = RecoveryDirectory::new();
        let mut app = dir.app();
        app.insert("saved on disk".into());
        std::fs::create_dir_all(dir.0.join("recovery.json")).unwrap();
        app.pending = Some(Pending::Quit);
        let destination = dir.0.join("saved.docx");
        assert!(!app.save_to(destination.clone(), &egui::Context::default()));
        assert!(!app.editor.is_dirty());
        assert_eq!(app.path, Some(destination.clone()));
        assert_eq!(
            files::open(&destination)
                .unwrap()
                .document
                .paragraph(0)
                .unwrap()
                .text(),
            "saved on disk"
        );
        assert!(app.error.is_some());
        assert!(matches!(app.pending, Some(Pending::Quit)));
        assert!(!app.allow_close);
        app.perform(Pending::Quit, &egui::Context::default());
        assert!(
            !app.allow_close,
            "retry close must still honor failed cleanup"
        );
    }
    #[test]
    fn recovery_cleanup_failure_blocks_mcp_transitions_and_save_reports_partial_success() {
        for operation in [
            "folio_new_document",
            "folio_open_document",
            "folio_save_document",
        ] {
            let dir = RecoveryDirectory::new();
            let mut app = dir.app();
            app.insert("keep current".into());
            assert!(app.flush_recovery());
            std::fs::remove_file(dir.0.join("recovery.json")).unwrap();
            std::fs::create_dir(dir.0.join("recovery.json")).unwrap();
            std::fs::write(dir.0.join("recovery.json/sentinel"), b"keep snapshot").unwrap();
            let before = app.editor.document().clone();
            let destination = dir.0.join("target.docx");
            if operation == "folio_open_document" {
                files::save(&Document::default(), &destination, None).unwrap();
            }
            let arguments = match operation {
                "folio_new_document" => serde_json::json!({"discard_unsaved":true}),
                "folio_open_document" => {
                    serde_json::json!({"path":destination,"discard_unsaved":true})
                }
                _ => serde_json::json!({"path":destination}),
            };
            assert!(
                mcp::call(&mut app, operation, arguments).is_err(),
                "{operation}"
            );
            assert_eq!(app.editor.document(), &before);
            assert!(app.error.is_some());
            assert!(app.last_recovery_document.is_some());
            assert_eq!(
                std::fs::read(dir.0.join("recovery.json/sentinel")).unwrap(),
                b"keep snapshot"
            );
            if operation == "folio_save_document" {
                assert!(!app.editor.is_dirty());
                assert_eq!(app.path, Some(destination.clone()));
                assert_eq!(files::open(&destination).unwrap().document, before);
            } else {
                assert!(app.editor.is_dirty());
                assert!(app.path.is_none());
            }
        }
    }
    #[test]
    fn checkpoint_debounce_coalesces_rapid_edits() {
        let now = std::time::Instant::now();
        let mut checkpoint = workspace::CheckpointDebounce::default();
        checkpoint.mark_changed(now);
        checkpoint.mark_changed(now + std::time::Duration::from_secs(1));
        assert!(!checkpoint.is_due(
            now + std::time::Duration::from_secs(2),
            std::time::Duration::from_secs(2)
        ));
        assert!(checkpoint.is_due(
            now + std::time::Duration::from_secs(3),
            std::time::Duration::from_secs(2)
        ));
        checkpoint.clear();
        assert!(!checkpoint.is_due(
            now + std::time::Duration::from_secs(10),
            std::time::Duration::from_secs(2)
        ));
    }
    #[test]
    fn autosave_debounce_waits_for_five_seconds_of_idle() {
        let dir = RecoveryDirectory::new();
        let mut app = dir.app();
        let ctx = egui::Context::default();
        app.path = Some(dir.0.join("draft.docx"));
        app.insert("first".into());
        let now = std::time::Instant::now();
        app.schedule_checkpoints(now);
        app.process_checkpoints(now + std::time::Duration::from_secs(2), &ctx);
        assert!(app.editor.is_dirty());
        assert!(
            app.workspace_store
                .as_ref()
                .unwrap()
                .load_recovery()
                .unwrap()
                .is_some()
        );
        app.insert(" second".into());
        app.schedule_checkpoints(now + std::time::Duration::from_secs(3));
        app.process_checkpoints(now + std::time::Duration::from_secs(7), &ctx);
        assert!(app.editor.is_dirty());
        app.process_checkpoints(now + std::time::Duration::from_secs(8), &ctx);
        assert!(!app.editor.is_dirty());
        assert_eq!(
            files::open(app.path.as_ref().unwrap())
                .unwrap()
                .document
                .paragraph(0)
                .unwrap()
                .text(),
            "first second"
        );
        assert!(
            app.workspace_store
                .as_ref()
                .unwrap()
                .load_recovery()
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn autosave_is_ineligible_for_untitled_or_warned_import() {
        for warned in [false, true] {
            let dir = RecoveryDirectory::new();
            let mut app = dir.app();
            if warned {
                std::fs::create_dir_all(&dir.0).unwrap();
                let source = dir.0.join("source.docx");
                std::fs::write(&source, b"original source").unwrap();
                app.path = Some(source.clone());
                app.protected = Some(source);
            }
            app.insert("retain me".into());
            let now = std::time::Instant::now();
            app.schedule_checkpoints(now);
            app.process_checkpoints(
                now + std::time::Duration::from_secs(5),
                &egui::Context::default(),
            );
            assert!(app.editor.is_dirty());
            assert_eq!(
                app.workspace_store
                    .as_ref()
                    .unwrap()
                    .load_recovery()
                    .unwrap()
                    .unwrap()
                    .document
                    .paragraph(0)
                    .unwrap()
                    .text(),
                "retain me"
            );
            if warned {
                assert_eq!(
                    std::fs::read(app.path.as_ref().unwrap()).unwrap(),
                    b"original source"
                );
            }
        }
    }
    #[test]
    fn recovery_replacement_is_single_atomic_snapshot() {
        let dir = RecoveryDirectory::new();
        let mut app = dir.app();
        app.insert("first".into());
        assert!(app.flush_recovery());
        app.insert(" second".into());
        assert!(app.flush_recovery());
        assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 1);
        let snapshot = app
            .workspace_store
            .as_ref()
            .unwrap()
            .load_recovery()
            .unwrap()
            .unwrap();
        assert_eq!(
            snapshot.document.paragraph(0).unwrap().text(),
            "first second"
        );
        assert_eq!(snapshot.selection.focus, Position::new(0, 12));
        let bytes = std::fs::read(dir.0.join("recovery.json")).unwrap();
        assert!(app.flush_recovery());
        assert_eq!(std::fs::read(dir.0.join("recovery.json")).unwrap(), bytes);
    }
    #[test]
    fn failed_recovery_write_keeps_previous_snapshot() {
        let dir = RecoveryDirectory::new();
        let mut app = dir.app();
        app.insert("old".into());
        assert!(app.flush_recovery());
        let bytes = std::fs::read(dir.0.join("recovery.json")).unwrap();
        // Unsupported schema forces a deterministic refusal without replacing the old bytes.
        let blocked = String::from_utf8(bytes)
            .unwrap()
            .replace("\"schema_version\": 1", "\"schema_version\": 999");
        std::fs::write(dir.0.join("recovery.json"), &blocked).unwrap();
        app.insert(" new".into());
        assert!(!app.flush_recovery());
        assert!(app.error.is_some());
        assert_eq!(
            std::fs::read_to_string(dir.0.join("recovery.json")).unwrap(),
            blocked
        );
        assert_eq!(
            app.last_recovery_document
                .as_ref()
                .unwrap()
                .paragraph(0)
                .unwrap()
                .text(),
            "old"
        );
    }
    #[test]
    fn recovery_discard_quit_does_not_recapture_unsaved_document() {
        let dir = RecoveryDirectory::new();
        let mut app = dir.app();
        app.insert("discard".into());
        assert!(app.flush_recovery());
        app.pending = Some(Pending::Quit);
        app.discard_pending(Pending::Quit, &egui::Context::default());
        assert!(app.allow_close);
        assert!(app.pending.is_none());
        assert!(
            app.workspace_store
                .as_ref()
                .unwrap()
                .load_recovery()
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn failed_recovery_close_keeps_document_and_window_open() {
        let dir = RecoveryDirectory::new();
        std::fs::create_dir_all(&dir.0).unwrap();
        std::fs::write(dir.0.join("recovery.json"), b"{broken").unwrap();
        let mut app = dir.app();
        app.insert("unsaved".into());
        app.perform(Pending::Quit, &egui::Context::default());
        assert!(!app.allow_close);
        assert!(app.editor.is_dirty());
        assert!(app.error.is_some());
        assert_eq!(
            std::fs::read(dir.0.join("recovery.json")).unwrap(),
            b"{broken"
        );
    }
    #[test]
    fn failed_autosave_retains_recovery_and_dirty_document() {
        let dir = RecoveryDirectory::new();
        let mut app = dir.app();
        app.path = Some(dir.0.join("missing/draft.docx"));
        app.insert("unsaved".into());
        let now = Instant::now();
        app.schedule_checkpoints(now);
        app.process_checkpoints(now + Duration::from_secs(5), &egui::Context::default());
        assert!(app.editor.is_dirty());
        assert!(app.error.is_some());
        assert!(
            app.workspace_store
                .as_ref()
                .unwrap()
                .load_recovery()
                .unwrap()
                .is_some()
        );
    }
    #[test]
    fn recovery_flushes_on_confirmed_close_and_clears_on_new() {
        let dir = RecoveryDirectory::new();
        let mut app = dir.app();
        app.insert("keep on close".into());
        app.perform(Pending::Quit, &egui::Context::default());
        assert!(app.allow_close);
        assert!(
            app.workspace_store
                .as_ref()
                .unwrap()
                .load_recovery()
                .unwrap()
                .is_some()
        );
        app.perform(Pending::New, &egui::Context::default());
        assert!(
            app.workspace_store
                .as_ref()
                .unwrap()
                .load_recovery()
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn view_tools_toggle_focus_theme_and_document_info() {
        let ctx = egui::Context::default();
        layout::install_fonts(&ctx);
        theme::install(&ctx);
        let mut app = FolioApp::default();
        assert!(!app.focus_mode);
        assert!(!app.dark_mode);
        assert!(!app.show_document_info);
        app.toggle_focus_mode();
        assert!(app.focus_mode);
        assert!(!app.show_document_info);
        app.toggle_focus_mode();
        app.toggle_document_info();
        assert!(app.show_document_info);
        app.toggle_theme(&ctx);
        assert!(app.dark_mode);
        assert!(ctx.style().visuals.dark_mode);
    }

    #[test]
    fn dark_theme_switches_visuals_and_can_return_to_light() {
        let ctx = egui::Context::default();
        theme::install_mode(&ctx, true);
        assert!(ctx.style().visuals.dark_mode);
        assert_eq!(ctx.style().visuals.window_fill, theme::surface(true));
        let _ = ctx.run(
            egui::RawInput {
                system_theme: Some(egui::Theme::Light),
                ..Default::default()
            },
            |_| {},
        );
        assert!(ctx.style().visuals.dark_mode);
        theme::install_mode(&ctx, false);
        assert!(!ctx.style().visuals.dark_mode);
        assert_eq!(ctx.style().visuals.window_fill, theme::surface(false));
    }

    #[test]
    fn view_shortcuts_toggle_focus_appearance_and_info_without_editing() {
        let ctx = egui::Context::default();
        layout::install_fonts(&ctx);
        theme::install(&ctx);
        let mut app = FolioApp::default();
        app.insert("Keep this text".into());
        app.editor.mark_saved();
        let original = app.editor.document().clone();
        let command_shift = egui::Modifiers {
            command: true,
            mac_cmd: true,
            shift: true,
            ..Default::default()
        };
        frame(&mut app, &ctx, vec![key(Key::F, command_shift)]);
        assert!(app.focus_mode);
        frame(&mut app, &ctx, vec![key(Key::F, command_shift)]);
        assert!(!app.focus_mode);
        frame(&mut app, &ctx, vec![key(Key::D, command_shift)]);
        assert!(app.dark_mode);
        frame(&mut app, &ctx, vec![key(Key::I, command_shift)]);
        assert!(app.show_document_info);
        assert_eq!(app.editor.document(), &original);
        assert!(!app.editor.is_dirty());
    }

    #[test]
    fn line_spacing_control_displays_exact_and_minimum_imported_values() {
        fn has_text(shape: &egui::Shape, expected: &str) -> bool {
            match shape {
                egui::Shape::Text(text) => text.galley.job.text == expected,
                egui::Shape::Vec(shapes) => shapes.iter().any(|shape| has_text(shape, expected)),
                _ => false,
            }
        }
        let ctx = egui::Context::default();
        layout::install_fonts(&ctx);
        theme::install(&ctx);
        let mut app = FolioApp::default();
        for (spacing, expected) in [
            (LineSpacing::Exact(360), "Exactly 18 pt"),
            (LineSpacing::AtLeast(480), "At least 24 pt"),
        ] {
            app.editor
                .execute(Command::FormatParagraphs {
                    selection: app.editor.selection(),
                    patch: ParagraphPatch {
                        line_spacing: Some(spacing),
                        ..Default::default()
                    },
                })
                .unwrap();
            frame(&mut app, &ctx, vec![]);
            let output = frame(&mut app, &ctx, vec![]);
            assert!(
                output.shapes.iter().any(|s| has_text(&s.shape, expected)),
                "Missing line spacing label: {expected}"
            );
            assert_eq!(
                app.editor
                    .document()
                    .paragraph(0)
                    .unwrap()
                    .style
                    .line_spacing,
                spacing
            );
        }
    }
    #[test]
    fn exact_line_height_commits_once_preserves_selection_and_roundtrips() {
        let ctx = egui::Context::default();
        layout::install_fonts(&ctx);
        let mut app = FolioApp::default();
        app.insert("first\nsecond".into());
        let selection = editing::select_all(app.editor.document());
        app.editor.set_selection(selection).unwrap();
        app.execute(Command::FormatParagraphs {
            selection,
            patch: ParagraphPatch {
                line_spacing: Some(LineSpacing::Exact(240)),
                ..Default::default()
            },
        });
        app.editor.mark_saved();
        app.focus_canvas = false;
        let original = app.editor.document().clone();
        let mut numeric_frame = |events: Vec<egui::Event>, focus: bool| {
            ctx.begin_pass(egui::RawInput {
                events,
                ..Default::default()
            });
            let mut id = egui::Id::NULL;
            egui::TopBottomPanel::top("spacing-test").show(&ctx, |ui| {
                let current = app
                    .editor
                    .document()
                    .paragraph(0)
                    .unwrap()
                    .style
                    .line_spacing;
                ui.horizontal(|ui| {
                    let response = app.line_spacing_control(ui, current).unwrap();
                    id = response.id;
                    if focus {
                        response.request_focus();
                    }
                });
            });
            app.canvas(&ctx);
            let _ = ctx.end_pass();
            id
        };
        let id = numeric_frame(vec![], true);
        numeric_frame(vec![], false);
        numeric_frame(
            vec![key(
                Key::A,
                egui::Modifiers {
                    command: true,
                    mac_cmd: true,
                    ..Default::default()
                },
            )],
            false,
        );
        numeric_frame(vec![egui::Event::Text("18.5".into())], false);
        assert!(ctx.memory(|m| m.has_focus(id)));
        numeric_frame(vec![key(Key::Enter, Default::default())], false);
        assert_eq!(app.editor.selection(), selection);
        for block in 0..2 {
            assert_eq!(
                app.editor
                    .document()
                    .paragraph(block)
                    .unwrap()
                    .style
                    .line_spacing,
                LineSpacing::Exact(370)
            );
        }
        let mut bytes = std::io::Cursor::new(Vec::new());
        let report = folio_docx::export_docx(app.editor.document(), &mut bytes).unwrap();
        assert!(report.warnings.is_empty());
        bytes.set_position(0);
        let imported = folio_docx::import_docx(bytes).unwrap();
        assert_eq!(imported.document, *app.editor.document());
        app.action(Action::Undo, &ctx);
        assert_eq!(app.editor.document(), &original);
        assert!(!app.editor.is_dirty());
    }
    #[test]
    fn clear_formatting_resets_only_selected_text_and_is_undoable() {
        let ctx = egui::Context::default();
        let mut app = FolioApp {
            typing: Some(TextStyle {
                bold: true,
                italic: true,
                underline: true,
                font_family: "serif".into(),
                size_half_points: 36,
                color: Color::rgb(150, 20, 20),
            }),
            ..Default::default()
        };
        app.insert("before selected after".into());
        let original = app.editor.document().clone();
        app.editor
            .set_selection(Selection::new(Position::new(0, 7), Position::new(0, 15)))
            .unwrap();
        app.action(Action::ClearFormatting, &ctx);
        let p = app.editor.document().paragraph(0).unwrap();
        assert_eq!(p.runs.len(), 3);
        assert_eq!(p.runs[1].text, "selected");
        assert_eq!(p.runs[1].style, TextStyle::default());
        assert!(p.runs[0].style.bold && p.runs[2].style.bold);
        app.action(Action::Undo, &ctx);
        assert_eq!(app.editor.document(), &original);
    }

    #[test]
    fn clear_formatting_at_a_caret_changes_future_typing_only() {
        let ctx = egui::Context::default();
        let mut app = FolioApp {
            typing: Some(TextStyle {
                bold: true,
                ..Default::default()
            }),
            ..Default::default()
        };
        app.insert("bold".into());
        app.editor.mark_saved();
        app.action(Action::ClearFormatting, &ctx);
        assert!(!app.editor.is_dirty());
        app.insert(" plain".into());
        let runs = &app.editor.document().paragraph(0).unwrap().runs;
        assert_eq!(runs.len(), 2);
        assert!(runs[0].style.bold);
        assert_eq!(runs[1].style, TextStyle::default());
    }

    #[test]
    fn clipboard_toolbar_copy_and_cut_use_selection_and_one_undo_step() {
        let ctx = egui::Context::default();
        let mut app = FolioApp::default();
        app.insert("first\nsecond".into());
        app.editor.mark_saved();
        let selection = Selection::new(Position::new(0, 2), Position::new(1, 3));
        app.editor.set_selection(selection).unwrap();
        ctx.begin_pass(Default::default());
        app.action(Action::Copy, &ctx);
        let output = ctx.end_pass();
        assert!(
            output
                .platform_output
                .commands
                .iter()
                .any(|c| matches!(c, egui::OutputCommand::CopyText(s) if s == "rst\nsec"))
        );
        assert!(!app.editor.is_dirty());
        ctx.begin_pass(Default::default());
        app.action(Action::Cut, &ctx);
        let output = ctx.end_pass();
        assert!(
            output
                .platform_output
                .commands
                .iter()
                .any(|c| matches!(c, egui::OutputCommand::CopyText(s) if s == "rst\nsec"))
        );
        assert_eq!(app.editor.document().paragraph(0).unwrap().text(), "fiond");
        app.action(Action::Undo, &ctx);
        assert_eq!(app.editor.selection(), selection);
        assert!(!app.editor.is_dirty());
    }

    #[test]
    fn keyboard_activating_paste_does_not_insert_the_activation_key() {
        let ctx = egui::Context::default();
        layout::install_fonts(&ctx);
        let mut app = FolioApp::default();
        app.insert("keep".into());
        app.editor.mark_saved();
        app.focus_canvas = false;
        let mut button_frame = |events: Vec<egui::Event>, focus: bool| {
            ctx.begin_pass(egui::RawInput {
                events,
                ..Default::default()
            });
            egui::TopBottomPanel::top("paste-test").show(&ctx, |ui| {
                let response = ui.add(IconButton::new(Icon::Paste, "Paste"));
                if focus {
                    response.request_focus();
                }
                if response.clicked() {
                    app.action(Action::Paste, &ctx);
                }
            });
            app.canvas(&ctx);
            ctx.end_pass()
        };
        button_frame(vec![], true);
        let output = button_frame(vec![key(Key::Enter, Default::default())], false);
        assert!(
            output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::RequestPaste))
        );
        assert!(!app.editor.is_dirty());
        assert_eq!(app.editor.document().paragraph(0).unwrap().text(), "keep");
        frame(&mut app, &ctx, vec![egui::Event::Paste(" pasted".into())]);
        assert_eq!(
            app.editor.document().paragraph(0).unwrap().text(),
            "keep pasted"
        );
        app.action(Action::Undo, &ctx);
        assert!(!app.editor.is_dirty());
    }
    #[test]
    fn clipboard_toolbar_paste_requests_native_clipboard_and_replaces_selection() {
        let ctx = egui::Context::default();
        layout::install_fonts(&ctx);
        let mut app = FolioApp::default();
        app.insert("replace".into());
        app.editor
            .set_selection(editing::select_all(app.editor.document()))
            .unwrap();
        frame(&mut app, &ctx, vec![]);
        ctx.begin_pass(Default::default());
        app.action(Action::Paste, &ctx);
        app.ribbon(&ctx);
        app.canvas(&ctx);
        let output = ctx.end_pass();
        assert!(
            output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .iter()
                .any(|c| matches!(c, egui::ViewportCommand::RequestPaste))
        );
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::Paste("Café\nnext".into())],
        );
        assert_eq!(app.editor.document().paragraph(0).unwrap().text(), "Café");
        assert_eq!(app.editor.document().paragraph(1).unwrap().text(), "next");
        app.action(Action::Undo, &ctx);
        assert_eq!(
            app.editor.document().paragraph(0).unwrap().text(),
            "replace"
        );
    }
    #[test]
    fn page_break_replaces_selection_in_one_undo_step() {
        let ctx = egui::Context::default();
        let mut app = FolioApp::default();
        app.insert("first\nsecond".into());
        app.editor.mark_saved();
        let original = app.editor.document().clone();
        let selection = Selection::new(Position::new(1, 3), Position::new(0, 2));
        app.editor.set_selection(selection).unwrap();
        app.action(Action::PageBreak, &ctx);
        assert_eq!(app.editor.document().paragraph(0).unwrap().text(), "fi");
        assert_eq!(app.editor.document().paragraph(2).unwrap().text(), "ond");
        let replaced = app.editor.document().clone();
        app.action(Action::Undo, &ctx);
        assert_eq!(app.editor.document(), &original);
        assert_eq!(app.editor.selection(), selection);
        assert!(!app.editor.is_dirty());
        app.action(Action::Redo, &ctx);
        assert_eq!(app.editor.document(), &replaced);
        assert_eq!(
            app.editor.selection(),
            Selection::caret(Position::new(2, 0))
        );
    }
    #[test]
    fn integrated_docx_save_reopen_and_warned_copy_destination() {
        let ctx = egui::Context::default();
        let mut app = FolioApp::default();
        let dir = std::env::temp_dir().join(format!("folio-roundtrip-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("source.docx");
        let copy = dir.join("converted.docx");
        app.insert("Café 你好 e\u{301} 👩‍👩‍👧‍👦".into());
        app.editor
            .set_selection(Selection::new(
                Position::new(0, 0),
                app.editor.selection().focus,
            ))
            .unwrap();
        app.format(StylePatch {
            bold: Some(true),
            italic: Some(true),
            font_family: Some("Noto Serif".into()),
            ..Default::default()
        });
        app.execute(Command::InsertPageBreak {
            at: app.editor.selection().focus,
        });
        app.insert("Second page".into());
        assert!(app.save_to(source.clone(), &ctx));
        assert!(!app.editor.is_dirty());
        let report = files::open(&source).unwrap();
        assert!(report.warnings.is_empty());
        assert_eq!(&report.document, app.editor.document());
        layout::install_fonts(&ctx);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let rendered = DocumentLayout::build(ctx, &report.document, 1.0);
            assert_eq!(
                rendered.lines[0].galley.job.sections[0]
                    .format
                    .font_id
                    .family,
                egui::FontFamily::Name("Serif-BoldItalic".into()),
            );
        });
        let original = std::fs::read(&source).unwrap();
        // All import warnings set this protection; keep it after saving a copy.
        app.protected = Some(source.clone());
        app.insert(" changed".into());
        assert!(!app.save_to(source.clone(), &ctx));
        assert!(app.editor.is_dirty());
        assert_eq!(std::fs::read(&source).unwrap(), original);
        assert!(app.save_to(copy.clone(), &ctx));
        assert_eq!(app.path.as_ref(), Some(&copy));
        assert_eq!(app.protected.as_ref(), Some(&source));
        app.insert(" again".into());
        assert!(app.save(false, &ctx));
        assert_eq!(std::fs::read(&source).unwrap(), original);
        assert_eq!(&files::open(&copy).unwrap().document, app.editor.document());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn destructive_commands_wait_for_unsaved_decision() {
        let ctx = egui::Context::default();
        let mut app = FolioApp::default();
        app.insert("keep me".into());
        app.request(Pending::New, &ctx);
        assert!(matches!(app.pending, Some(Pending::New)));
        assert_eq!(
            app.editor.document().paragraph(0).unwrap().text(),
            "keep me"
        );
        app.pending = None;
        assert!(app.editor.is_dirty());
        app.perform(Pending::New, &ctx);
        assert!(!app.editor.is_dirty());
        assert!(
            app.editor
                .document()
                .paragraph(0)
                .unwrap()
                .text()
                .is_empty()
        );
    }
    #[test]
    fn failed_save_keeps_dirty_document_path_and_pending_operation() {
        let ctx = egui::Context::default();
        let mut app = FolioApp::default();
        app.insert("unsaved".into());
        app.pending = Some(Pending::Quit);
        let path = std::env::temp_dir()
            .join("folio-absent-directory")
            .join("file.docx");
        assert!(!app.save_to(path, &ctx));
        assert!(app.editor.is_dirty());
        assert!(app.path.is_none());
        assert!(app.error.is_some());
        assert!(matches!(app.pending, Some(Pending::Quit)));
        assert!(!app.allow_close);
    }
    #[test]
    fn formatting_selection_changes_model_and_undo_restores_it() {
        let ctx = egui::Context::default();
        let mut app = FolioApp::default();
        app.insert("first\nsecond".into());
        app.editor
            .set_selection(editing::select_all(app.editor.document()))
            .unwrap();
        app.action(Action::Bold, &ctx);
        assert!(
            app.editor.document().paragraph(0).unwrap().runs[0]
                .style
                .bold
        );
        assert!(
            app.editor.document().paragraph(1).unwrap().runs[0]
                .style
                .bold
        );
        app.action(Action::Undo, &ctx);
        assert!(
            !app.editor.document().paragraph(0).unwrap().runs[0]
                .style
                .bold
        );
    }

    fn frame(
        app: &mut FolioApp,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        ctx.begin_pass(egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(1180.0, 850.0),
            )),
            events,
            ..Default::default()
        });
        app.global_shortcuts(ctx);
        app.ribbon(ctx);
        app.canvas(ctx);
        ctx.end_pass()
    }
    fn key(key: Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }
    #[test]
    fn command_f_focuses_search_and_typing_preserves_document_and_selection() {
        let ctx = egui::Context::default();
        layout::install_fonts(&ctx);
        let mut app = FolioApp::default();
        app.insert("document stays intact".into());
        app.editor.mark_saved();
        app.editor
            .set_selection(Selection::new(Position::new(0, 2), Position::new(0, 8)))
            .unwrap();
        let selection = app.editor.selection();
        let document = app.editor.document().clone();
        frame(&mut app, &ctx, vec![]);
        frame(
            &mut app,
            &ctx,
            vec![
                key(
                    Key::F,
                    egui::Modifiers {
                        command: true,
                        mac_cmd: true,
                        ..Default::default()
                    },
                ),
                egui::Event::Text("needle".into()),
            ],
        );
        assert!(app.search_open);
        assert!(ctx.memory(|m| m.has_focus(egui::Id::new("find-input"))));
        assert_eq!(app.needle, "needle");
        frame(&mut app, &ctx, vec![egui::Event::Text(" more".into())]);
        assert_eq!(app.needle, "needle more");
        assert_eq!(app.editor.document(), &document);
        assert_eq!(app.editor.selection(), selection);
        assert!(!app.editor.is_dirty());
    }
    #[test]
    fn numeric_text_focus_persists_until_commit_and_isolated_from_document() {
        let ctx = egui::Context::default();
        layout::install_fonts(&ctx);
        let mut app = FolioApp::default();
        app.insert("keep".into());
        app.editor.mark_saved();
        app.focus_canvas = false;
        let document = app.editor.document().clone();
        let selection = app.editor.selection();
        let numeric_frame = |app: &mut FolioApp, events: Vec<egui::Event>, focus: bool| {
            ctx.begin_pass(egui::RawInput {
                screen_rect: Some(Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(1180.0, 850.0),
                )),
                events,
                ..Default::default()
            });
            let mut id = egui::Id::NULL;
            egui::TopBottomPanel::top("numeric-test").show(&ctx, |ui| {
                let style = app.current_style();
                let response = app.font_size_control(ui, &style);
                id = response.id;
                if focus {
                    response.request_focus();
                }
            });
            app.canvas(&ctx);
            let _ = ctx.end_pass();
            id
        };
        let id = numeric_frame(&mut app, vec![], true);
        numeric_frame(&mut app, vec![], false);
        numeric_frame(
            &mut app,
            vec![key(
                Key::A,
                egui::Modifiers {
                    command: true,
                    mac_cmd: true,
                    ..Default::default()
                },
            )],
            false,
        );
        numeric_frame(&mut app, vec![egui::Event::Text("1".into())], false);
        assert!(ctx.memory(|m| m.has_focus(id)));
        assert_eq!(app.current_style().size_half_points, 24);
        numeric_frame(&mut app, vec![egui::Event::Text("8".into())], false);
        assert!(ctx.memory(|m| m.has_focus(id)));
        assert_eq!(app.current_style().size_half_points, 24);
        numeric_frame(&mut app, vec![key(Key::Enter, Default::default())], false);
        assert_eq!(app.current_style().size_half_points, 36);
        assert_eq!(app.editor.document(), &document);
        assert_eq!(app.editor.selection(), selection);
        assert!(!app.editor.is_dirty());
    }
    #[test]
    fn ime_preedit_is_transient_and_commit_is_one_undo_step() {
        let ctx = egui::Context::default();
        layout::install_fonts(&ctx);
        let mut app = FolioApp::default();
        frame(&mut app, &ctx, vec![]);
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::Ime(egui::ImeEvent::Enabled),
                egui::Event::Ime(egui::ImeEvent::Preedit("日本".into())),
            ],
        );
        assert_eq!(app.composition.as_deref(), Some("日本"));
        assert!(!app.editor.is_dirty());
        assert!(!app.editor.can_undo());
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::Ime(egui::ImeEvent::Commit("日本語".into()))],
        );
        assert_eq!(app.editor.document().paragraph(0).unwrap().text(), "日本語");
        assert!(app.composition.is_none());
        app.action(Action::Undo, &ctx);
        assert!(!app.editor.is_dirty());
    }
    #[test]
    fn clipboard_copy_and_paste_use_model_selection() {
        let ctx = egui::Context::default();
        layout::install_fonts(&ctx);
        let mut app = FolioApp::default();
        app.insert("copy me".into());
        frame(&mut app, &ctx, vec![]);
        app.editor
            .set_selection(editing::select_all(app.editor.document()))
            .unwrap();
        let output = frame(&mut app, &ctx, vec![egui::Event::Copy]);
        assert!(
            output
                .platform_output
                .commands
                .iter()
                .any(|c| matches!(c,egui::OutputCommand::CopyText(s) if s=="copy me"))
        );
        frame(
            &mut app,
            &ctx,
            vec![egui::Event::Paste("first\nsecond".into())],
        );
        assert_eq!(app.editor.document().blocks.len(), 2);
        app.action(Action::Undo, &ctx);
        assert_eq!(
            app.editor.document().paragraph(0).unwrap().text(),
            "copy me"
        );
    }

    #[test]
    fn keyboard_wrap_end_affinity_and_formatted_cross_page_undo() {
        let ctx = egui::Context::default();
        layout::install_fonts(&ctx);
        let mut app = FolioApp::default();
        app.insert("formatted editable page text ".repeat(700));
        frame(&mut app, &ctx, vec![]);
        ctx.begin_pass(Default::default());
        let layout = DocumentLayout::build(&ctx, app.editor.document(), 1.0);
        assert!(layout.pages.len() > 1);
        let first = layout.lines[0].stops[0].at;
        let end = layout.lines[0].stops.last().unwrap().at;
        app.editor.set_selection(Selection::caret(first)).unwrap();
        app.visual_line = Some(0);
        app.canvas(&ctx);
        let _ = ctx.end_pass();
        frame(&mut app, &ctx, vec![key(Key::End, Default::default())]);
        assert_eq!(app.editor.selection().focus, end);
        assert_eq!(app.visual_line, Some(0));
        frame(
            &mut app,
            &ctx,
            vec![key(
                Key::ArrowDown,
                egui::Modifiers {
                    command: true,
                    mac_cmd: true,
                    ..Default::default()
                },
            )],
        );
        let final_at = app.editor.selection().focus;
        assert_eq!(
            final_at.offset,
            app.editor.document().paragraph(0).unwrap().len_bytes()
        );
        app.editor
            .set_selection(Selection::new(first, final_at))
            .unwrap();
        app.action(Action::Bold, &ctx);
        assert!(
            app.editor
                .document()
                .paragraph(0)
                .unwrap()
                .runs
                .iter()
                .all(|r| r.style.bold)
        );
        app.action(Action::Undo, &ctx);
        assert!(
            app.editor
                .document()
                .paragraph(0)
                .unwrap()
                .runs
                .iter()
                .all(|r| !r.style.bold)
        );
    }
}
