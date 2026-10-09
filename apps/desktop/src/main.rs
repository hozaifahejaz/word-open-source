mod editing;
mod files;
mod layout;
mod theme;
use document_core::*;
use editing::{Action, move_to};
use egui::{Color32, Key, Rect, Stroke, Vec2};
use layout::DocumentLayout;
use std::path::PathBuf;

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
    allow_close: bool,
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
            allow_close: false,
        }
    }
}
impl FolioApp {
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
    fn request(&mut self, pending: Pending, ctx: &egui::Context) {
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
                self.editor = Editor::default();
                self.path = None;
                self.protected = None;
                self.warnings.clear();
                self.typing = None;
                self.notice.clear();
                self.focus_canvas = true;
            }
            Pending::Open => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Word document", &["docx"])
                    .pick_file()
                {
                    match files::open(&path) {
                        Ok(report) => match self.editor.load_document(report.document) {
                            Ok(()) => {
                                self.protected =
                                    (!report.warnings.is_empty()).then(|| path.clone());
                                self.path = Some(path);
                                self.warnings = report.warnings;
                                self.typing = None;
                                self.notice = "Document opened".into();
                                self.focus_canvas = true;
                                self.reveal = true;
                            }
                            Err(e) => self.error = Some(e.to_string()),
                        },
                        Err(e) => {
                            self.error = Some(format!("Could not open {}: {e}", path.display()))
                        }
                    }
                }
            }
            Pending::Quit => {
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
        match files::save(self.editor.document(), &path, self.protected.as_deref()) {
            Ok(()) => {
                self.editor.mark_saved();
                self.path = Some(path);
                self.notice = "Saved".into();
                self.focus_canvas = true;
                if let Some(pending) = self.pending.take() {
                    self.perform(pending, ctx);
                }
                true
            }
            Err(e) => {
                self.error = Some(format!("Could not save {}: {e}", path.display()));
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
        if self.pending.is_some()
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
            if self.pending.is_some() || self.error.is_some() {
                break;
            }
        }
    }
    fn ribbon(&mut self, ctx: &egui::Context) {
        let frame = egui::Frame::new()
            .fill(Color32::WHITE)
            .inner_margin(egui::Margin::symmetric(16, 12));
        let panel = egui::TopBottomPanel::top("ribbon").frame(frame);
        panel.show(ctx, |ui| {
            if self.pending.is_some() || self.overwrite.is_some() || self.error.is_some() {
                ui.disable();
            }
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    egui::RichText::new("FOLIO")
                        .strong()
                        .color(theme::ACCENT)
                        .size(22.0),
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
                        theme::primary_button(label)
                    } else {
                        egui::Button::new(label)
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
                    ui.colored_label(theme::ACCENT, "Edited");
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
                    ui.horizontal(|ui| {
                        for (label, action) in [
                            ("New document", Action::New),
                            ("Open DOCX…", Action::Open),
                            ("Save", Action::Save),
                            ("Save As…", Action::SaveAs),
                        ] {
                            if ui.button(label).clicked() {
                                self.action(action, ctx);
                            }
                        }
                    });
                    ui.label("DOCX • A warned import always saves as a converted copy.");
                }
                Tab::Home => {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(egui::RichText::new("Text").small().color(theme::MUTED));
                        let style = self.current_style();
                        for (label, selected, action) in [
                            ("Bold", style.bold, Action::Bold),
                            ("Italic", style.italic, Action::Italic),
                            ("Underline", style.underline, Action::Underline),
                        ] {
                            if ui.selectable_label(selected, label).clicked() {
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
                        ui.label("Text color");
                        let mut rgb = [style.color.red, style.color.green, style.color.blue];
                        if ui
                            .color_edit_button_srgb(&mut rgb)
                            .on_hover_text("Text color")
                            .changed()
                        {
                            self.format(StylePatch {
                                color: Some(Color::rgb(rgb[0], rgb[1], rgb[2])),
                                ..Default::default()
                            });
                        }
                        ui.separator();
                        ui.label(egui::RichText::new("Paragraph").small().color(theme::MUTED));
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
                            if ui
                                .selectable_label(alignment == value, label)
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
                        if ui.button("Find / Replace").clicked() {
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
                        ui.label("Line spacing");
                        let mut multiple = match p.line_spacing {
                            LineSpacing::Multiple(n) => n,
                            _ => 100,
                        };
                        egui::ComboBox::from_id_salt("spacing")
                            .selected_text(format!("{}×", multiple as f32 / 100.0))
                            .show_ui(ui, |ui| {
                                for n in [100, 115, 150, 200] {
                                    if ui
                                        .selectable_value(
                                            &mut multiple,
                                            n,
                                            format!("{}×", n as f32 / 100.0),
                                        )
                                        .changed()
                                    {
                                        self.execute(Command::FormatParagraphs {
                                            selection: self.editor.selection(),
                                            patch: ParagraphPatch {
                                                line_spacing: Some(LineSpacing::Multiple(n)),
                                                ..Default::default()
                                            },
                                        });
                                        self.focus_canvas = true;
                                    }
                                }
                            });
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
                            .button("Page break")
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
                        if ui.button("100%").clicked() {
                            self.zoom = 1.0;
                        }
                        if ui.button("Fit page width").clicked() {
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
                        .add_enabled(!self.needle.is_empty(), egui::Button::new("Find next"))
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
                    if ui.button("Close find").clicked() {
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
            if self.pending.is_some() || self.overwrite.is_some() || self.error.is_some() {
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
                    ctx.copy_text(editing::selected_text(&self.editor));
                    if matches!(event, egui::Event::Cut) {
                        self.execute(Command::Delete {
                            selection: self.editor.selection(),
                        });
                    }
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
    fn canvas(&mut self, ctx: &egui::Context) {
        let id = egui::Id::new("document-canvas");
        let mut layout = DocumentLayout::build(ctx, self.editor.document(), self.zoom);
        let focused = ctx.memory(|m| m.has_focus(id));
        self.keyboard(ctx, &mut layout, focused);
        self.pages = layout.pages.len();
        self.active_page = layout
            .visual_line(self.editor.selection().focus, self.visual_line)
            .map_or(1, |i| layout.lines[i].page + 1);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::WORKSPACE).inner_margin(16.0))
            .show(ctx, |ui| {
                if self.pending.is_some() || self.overwrite.is_some() || self.error.is_some() {
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
                                move_to(&mut self.editor, at, ctx.input(|i| i.modifiers.shift));
                                self.visual_line = Some(line);
                                self.typing = None;
                                self.preferred_x = None;
                            }
                        }
                        if response.double_clicked() {
                            let at = self.editor.selection().focus;
                            let a = word_edge(self.editor.document(), at, false);
                            let b = word_edge(self.editor.document(), at, true);
                            self.editor.set_selection(Selection::new(a, b)).unwrap();
                        } else if response.dragged()
                            && let Some(pointer) = response.interact_pointer_pos()
                            && let Some((at, line)) = layout.hit_line((pointer - origin).to_pos2())
                        {
                            move_to(&mut self.editor, at, true);
                            self.visual_line = Some(line);
                            self.typing = None;
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
                                page.translate(Vec2::new(3.0, 4.0)),
                                2.0,
                                Color32::from_black_alpha(30),
                            );
                            painter.rect_filled(page, 1.0, Color32::WHITE);
                            painter.rect_stroke(
                                page,
                                1.0,
                                Stroke::new(1.0, theme::BORDER),
                                egui::StrokeKind::Inside,
                            );
                            painter.text(
                                page.right_bottom() - Vec2::new(20.0, 12.0),
                                egui::Align2::RIGHT_BOTTOM,
                                format!("{}", i + 1),
                                egui::FontId::proportional(10.0 * self.zoom),
                                Color32::GRAY,
                            );
                        }
                        for rect in layout.selection_rects(self.editor.selection()) {
                            painter.rect_filled(
                                rect.translate(origin.to_vec2()),
                                0.0,
                                Color32::from_rgba_unmultiplied(54, 130, 153, 75),
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
                            self.pending = None;
                            self.perform(pending, ctx);
                        }
                        if ui.button("Cancel").clicked() {
                            self.pending = None;
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
                    if ui.add_enabled(enabled, theme::primary_button("Save converted copy…")).clicked() {
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
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.request(Pending::Quit, ctx);
        }
        self.global_shortcuts(ctx);
        self.ribbon(ctx);
        // A pending destructive operation blocks document/ribbon input until resolved.
        self.dialogs(ctx);
        let frame = egui::Frame::new()
            .fill(Color32::WHITE)
            .inner_margin(egui::Margin::symmetric(16, 8));
        let panel = egui::TopBottomPanel::bottom("status").frame(frame);
        panel.show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                let words: usize = self
                    .editor
                    .document()
                    .blocks
                    .iter()
                    .filter_map(|b| {
                        if let Block::Paragraph(p) = b {
                            Some(p.text().split_whitespace().count())
                        } else {
                            None
                        }
                    })
                    .sum();
                ui.label(format!("Page {} of {}", self.active_page, self.pages));
                ui.separator();
                ui.label(format!("{words} words"));
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
                    ("New document", theme::MUTED)
                } else {
                    ("Saved", theme::MUTED)
                };
                ui.colored_label(color, status);
                if !self.notice.is_empty() && self.notice != status {
                    ui.label(
                        egui::RichText::new(&self.notice)
                            .small()
                            .color(theme::MUTED),
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
        self.canvas(ctx);
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
    }
}
fn main() -> eframe::Result {
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
            theme::install(&cc.egui_ctx);
            Ok(Box::new(FolioApp::default()))
        }),
    )
}

#[cfg(test)]
mod app_tests {
    use super::*;
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
