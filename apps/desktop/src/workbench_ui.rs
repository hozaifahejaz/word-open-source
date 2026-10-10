use crate::{
    FolioApp, editing,
    icons::{Icon, IconButton},
    layout::DocumentLayout,
    workbench::*,
};
use document_core::*;
use egui::Key;
use std::time::Instant;
impl FolioApp {
    pub fn workbench_blocks_editing(&self) -> bool {
        self.export_picker.is_some()
            || self.template_gallery
            || self.workbench.palette
            || self.workbench.navigation.is_some()
            || self.workbench.snippets
            || self.workbench.shortcuts
    }
    fn tool_enabled(&self, tool: Tool) -> bool {
        if self.read_only && matches!(tool, Tool::Action(a) if a.mutates_document()) {
            return false;
        }
        if self.read_only
            && matches!(
                tool,
                Tool::DateTime
                    | Tool::PaintFormat
                    | Tool::ClearParagraph
                    | Tool::Case(_)
                    | Tool::Highlight
                    | Tool::Align(_)
                    | Tool::Symbol(_)
            )
        {
            return false;
        }
        if matches!(tool, Tool::ExportSelection) {
            return !self.editor.selection().is_collapsed();
        }
        match tool {
            Tool::Action(editing::Action::Undo) => self.editor.can_undo(),
            Tool::Action(editing::Action::Redo) => self.editor.can_redo(),
            Tool::Action(editing::Action::Cut | editing::Action::Copy) => {
                !self.editor.selection().is_collapsed()
            }
            Tool::Case(_) => !self.editor.selection().is_collapsed(),
            Tool::PaintFormat => {
                self.workbench.painter.is_some() && !self.editor.selection().is_collapsed()
            }
            _ => true,
        }
    }
    pub(crate) fn open_palette(&mut self) {
        self.workbench.palette = true;
        self.workbench.query.clear();
        self.workbench.selected = 0;
        self.workbench.focus = true;
        self.focus_canvas = false;
        self.composition = None;
        self.ime_enabled = false;
    }
    pub(crate) fn run_tool(&mut self, tool: Tool, ctx: &egui::Context) {
        if !self.tool_enabled(tool) {
            return;
        }
        self.focus_canvas = true;
        match tool {
            Tool::Action(action) => self.action(action, ctx),
            Tool::Templates => {
                self.template_gallery = true;
                self.focus_canvas = false;
                self.composition = None;
                self.ime_enabled = false;
            }
            Tool::Duplicate => self.choose_export(crate::WriteOperation::Duplicate, ctx),
            Tool::ExportDocument => {
                self.export_picker = Some(crate::ExportFormat::Pdf);
                self.focus_canvas = false;
                self.composition = None;
                self.ime_enabled = false;
            }
            Tool::ExportText => self.choose_export(crate::WriteOperation::Text(None), ctx),
            Tool::ExportSelection => self.choose_export(
                crate::WriteOperation::Text(Some(self.editor.selection())),
                ctx,
            ),
            Tool::ReadOnly => self.toggle_read_only(),
            Tool::Palette => self.open_palette(),
            Tool::Navigate(kind) => {
                self.workbench.navigation = Some(kind);
                self.workbench.number.clear();
                self.workbench.validation = None;
                self.workbench.focus = true;
                self.focus_canvas = false;
            }
            Tool::Snippets => {
                self.workbench.validation = None;
                self.workbench.snippets = true;
                self.focus_canvas = false;
            }
            Tool::Shortcuts => {
                self.workbench.shortcuts = true;
                self.focus_canvas = false;
            }
            Tool::Progress => self.workbench.progress = !self.workbench.progress,
            Tool::DateTime => {
                self.insert(local_datetime());
                self.focus_canvas = true;
            }
            Tool::CaptureFormat => {
                self.workbench.painter = Some(self.current_style());
                self.notice =
                    "Format captured. Select text and choose Apply format painter.".into();
                self.focus_canvas = true;
            }
            Tool::PaintFormat => {
                let s = self.workbench.painter.clone().unwrap();
                self.execute(Command::FormatRuns {
                    selection: self.editor.selection(),
                    patch: StylePatch {
                        bold: Some(s.bold),
                        italic: Some(s.italic),
                        underline: Some(s.underline),
                        font_family: Some(s.font_family),
                        size_half_points: Some(s.size_half_points),
                        color: Some(s.color),
                        strikethrough: Some(s.strikethrough),
                        vertical_align: Some(s.vertical_align),
                        highlight: Some(s.highlight),
                    },
                });
                if self.error.is_none() {
                    self.workbench.painter = None;
                }
                self.typing = None;
                self.focus_canvas = true;
            }
            Tool::ClearParagraph => {
                let s = ParagraphStyle::default();
                self.execute(Command::FormatParagraphs {
                    selection: self.editor.selection(),
                    patch: ParagraphPatch {
                        alignment: Some(s.alignment),
                        space_before_twips: Some(s.space_before_twips),
                        space_after_twips: Some(s.space_after_twips),
                        line_spacing: Some(s.line_spacing),
                    },
                });
                self.focus_canvas = true;
            }
            Tool::Align(alignment) => self.execute(Command::FormatParagraphs {
                selection: self.editor.selection(),
                patch: ParagraphPatch {
                    alignment: Some(alignment),
                    ..Default::default()
                },
            }),
            Tool::Symbol(text) => self.insert(text.into()),
            Tool::FitWidth => self.fit_page_width(ctx),
            Tool::Zoom100 => self.zoom = 1.0,
            Tool::Case(case) => self.change_case(case),
            Tool::Highlight => {
                self.format(StylePatch {
                    highlight: Some(Some(Color::rgb(255, 255, 0))),
                    ..Default::default()
                });
                self.focus_canvas = true;
            }
            Tool::Home => {
                self.tab = crate::Tab::Home;
                self.focus_mode = false;
            }
            Tool::Layout => {
                self.tab = crate::Tab::Layout;
                self.focus_mode = false;
            }
            Tool::Focus => self.toggle_focus_mode(),
            Tool::Theme => self.toggle_theme(ctx),
            Tool::Info => self.toggle_document_info(),
            Tool::Connection => self.show_ai_connection = true,
        }
    }
    pub(crate) fn fit_page_width(&mut self, ctx: &egui::Context) {
        let available = ctx.screen_rect().width()
            - 80.
            - if self.show_document_info && !self.focus_mode {
                228.
            } else {
                0.
            }
            - if self.workbench.progress && !self.focus_mode {
                230.
            } else {
                0.
            };
        let page = self
            .editor
            .document()
            .page_layout
            .effective_size()
            .width_twips as f32
            / 15.;
        self.zoom = (available / page).clamp(0.25, 2.5);
    }
    pub(crate) fn workbench_controls(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        for (icon, label, tool) in [
            (Icon::Commands, "Commands", Tool::Palette),
            (Icon::Statistics, "Writing progress", Tool::Progress),
            (Icon::Paste, "Snippets", Tool::Snippets),
            (Icon::Info, "Keyboard shortcuts", Tool::Shortcuts),
        ] {
            if ui.add(IconButton::new(icon, label)).clicked() {
                self.run_tool(tool, &ctx);
            }
        }
        ui.menu_button("Go to", |ui| {
            for kind in [Navigation::Page, Navigation::Line, Navigation::Paragraph] {
                if ui.button(kind.label()).clicked() {
                    self.run_tool(Tool::Navigate(kind), &ctx);
                    ui.close();
                }
            }
        });
    }
    pub(crate) fn productivity_controls(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        ui.menu_button("Writing tools", |ui| {
            for (label, tool) in [
                ("Insert date and time", Tool::DateTime),
                ("Capture format painter", Tool::CaptureFormat),
                ("Apply format painter", Tool::PaintFormat),
                ("Clear paragraph formatting", Tool::ClearParagraph),
            ] {
                if ui
                    .add_enabled(self.tool_enabled(tool), egui::Button::new(label))
                    .clicked()
                {
                    self.run_tool(tool, &ctx);
                    ui.close();
                }
            }
        });
    }
    fn navigate_workbench(&mut self, ctx: &egui::Context, kind: Navigation) {
        let layout = DocumentLayout::build(ctx, self.editor.document(), self.zoom);
        let limit = match kind {
            Navigation::Page => layout.pages.len(),
            Navigation::Line => layout.lines.len(),
            Navigation::Paragraph => self
                .editor
                .document()
                .blocks
                .iter()
                .filter(|b| matches!(b, Block::Paragraph(_)))
                .count(),
        };
        let number = self
            .workbench
            .number
            .trim()
            .parse::<usize>()
            .ok()
            .filter(|n| (1..=limit).contains(n));
        let Some(number) = number else {
            self.workbench.validation = Some(format!(
                "Enter a whole {} number from 1 to {limit}.",
                kind.label()
            ));
            return;
        };
        let target = match kind {
            Navigation::Page => page_position(&layout, number),
            Navigation::Line => line_position(&layout, number),
            Navigation::Paragraph => paragraph_position(self.editor.document(), number)
                .and_then(|p| Some((p, layout.visual_line(p, None)?))),
        };
        if let Some((position, line)) = target {
            if let Err(e) = self.editor.set_selection(Selection::caret(position)) {
                self.workbench.validation = Some(e.to_string());
                return;
            }
            self.visual_line = Some(line);
            self.typing = None;
            self.preferred_x = None;
            self.reveal = true;
        } else if kind == Navigation::Page {
            self.workbench.reveal_page = Some(number - 1);
            self.reveal = false;
        }
        self.workbench.navigation = None;
        self.focus_canvas = true;
    }
    pub(crate) fn workbench_windows(&mut self, ctx: &egui::Context) {
        if self.pending.is_some()
            || self.pending_recovery.is_some()
            || self.error.is_some()
            || self.overwrite.is_some()
        {
            return;
        }
        if self.workbench_blocks_editing()
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape))
        {
            self.export_picker = None;
            self.template_gallery = false;
            self.workbench.palette = false;
            self.workbench.navigation = None;
            self.workbench.snippets = false;
            self.workbench.shortcuts = false;
            self.focus_canvas = true;
            return;
        }
        if let Some(mut selected) = self.export_picker {
            let mut destination = false;
            let mut cancel = false;
            egui::Modal::new(egui::Id::new("export_document_picker")).show(ctx, |ui| {
                ui.set_width((ctx.screen_rect().width() - 64.).clamp(240., 480.));
                ui.heading("Export document");
                ui.label("Create a copy of the whole document. Choose the format that fits its destination.");
                ui.separator();
                for format in crate::export_formats::ALL {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut selected, format, format.label());
                        ui.weak(format!(".{}", format.extension()));
                    });
                }
                ui.separator();
                ui.label(selected.description());
                ui.horizontal(|ui| {
                    destination = ui.add(IconButton::new(Icon::ExportText, "Choose destination…").primary()).clicked();
                    cancel = ui.add(IconButton::new(Icon::Close, "Cancel")).clicked();
                });
            });
            self.export_picker = Some(selected);
            if destination || cancel {
                // Remove the modal guard before entering the native destination adapter.
                self.export_picker = None;
                if destination {
                    self.choose_export(crate::WriteOperation::Format(selected), ctx);
                }
                self.focus_canvas = true;
            }
            return;
        }
        if self.workbench.palette {
            let mut open = true;
            let mut execute = None;
            egui::Window::new("Commands — Command/Ctrl + K")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_width(430.)
                .max_width((ctx.screen_rect().width() - 40.).max(250.))
                .show(ctx, |ui| {
                    let mut reveal_selected = self.workbench.focus;
                    let input = ui.add(
                        egui::TextEdit::singleline(&mut self.workbench.query)
                            .hint_text("Search commands")
                            .id_salt("command-query")
                            .desired_width(f32::INFINITY),
                    );
                    input.widget_info(|| {
                        egui::WidgetInfo::labeled(
                            egui::WidgetType::TextEdit,
                            true,
                            "Search commands",
                        )
                    });
                    if self.workbench.focus {
                        input.request_focus();
                        self.workbench.focus = false;
                    }
                    if input.changed() {
                        reveal_selected = true;
                        self.workbench.selected = 0;
                    }
                    let query = self.workbench.query.to_lowercase();
                    let entries: Vec<_> = commands()
                        .into_iter()
                        .filter(|c| c.label.to_lowercase().contains(&query))
                        .collect();
                    if !entries.is_empty() {
                        self.workbench.selected = self.workbench.selected.min(entries.len() - 1);
                        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::ArrowDown)) {
                            reveal_selected = true;
                            self.workbench.selected =
                                (self.workbench.selected + 1).min(entries.len() - 1);
                        }
                        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::ArrowUp)) {
                            reveal_selected = true;
                            self.workbench.selected = self.workbench.selected.saturating_sub(1);
                        }
                        if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter)) {
                            execute = Some(entries[self.workbench.selected].tool);
                        }
                    }
                    egui::ScrollArea::vertical()
                        .max_height((ctx.screen_rect().height() - 180.).clamp(80., 360.))
                        .show(ui, |ui| {
                            for (index, command) in entries.iter().enumerate() {
                                let response = ui.add_enabled(
                                    self.tool_enabled(command.tool),
                                    IconButton::new(
                                        match command.tool {
                                            Tool::Action(action) => Icon::for_action(action),
                                            _ => Icon::Commands,
                                        },
                                        command.label,
                                    )
                                    .selected(index == self.workbench.selected),
                                );
                                if index == self.workbench.selected && reveal_selected {
                                    response.scroll_to_me(Some(egui::Align::Center));
                                }
                                if response.clicked() {
                                    execute = Some(command.tool);
                                }
                            }
                            if entries.is_empty() {
                                ui.label("No matching commands");
                            }
                        });
                });
            if let Some(tool) = execute
                && self.tool_enabled(tool)
            {
                self.workbench.palette = false;
                self.run_tool(tool, ctx);
            } else if !open {
                self.workbench.palette = false;
                self.focus_canvas = true;
            }
        }
        if let Some(kind) = self.workbench.navigation {
            let layout = DocumentLayout::build(ctx, self.editor.document(), self.zoom);
            let limit = match kind {
                Navigation::Page => layout.pages.len(),
                Navigation::Line => layout.lines.len(),
                Navigation::Paragraph => self
                    .editor
                    .document()
                    .blocks
                    .iter()
                    .filter(|b| matches!(b, Block::Paragraph(_)))
                    .count(),
            };
            let mut open = true;
            let mut go = false;
            egui::Window::new(format!("Go to {}", kind.label()))
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    let label = ui.label(format!("{} number (1–{limit})", kind.label()));
                    let input = ui
                        .add(
                            egui::TextEdit::singleline(&mut self.workbench.number)
                                .hint_text("1-based number"),
                        )
                        .labelled_by(label.id);
                    if self.workbench.focus {
                        input.request_focus();
                        self.workbench.focus = false;
                    }
                    if let Some(error) = &self.workbench.validation {
                        ui.colored_label(crate::theme::WARNING, error);
                    }
                    go = ui.button("Go").clicked()
                        || ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Enter));
                });
            if go {
                self.navigate_workbench(ctx, kind);
            } else if !open {
                self.workbench.navigation = None;
                self.focus_canvas = true;
            }
        }
        if self.workbench.shortcuts {
            let mut open = true;
            egui::Window::new("Keyboard shortcuts")
                .open(&mut open)
                .collapsible(false)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height((ctx.screen_rect().height() - 120.).clamp(80., 320.))
                        .show(ui, |ui| {
                            for text in [
                                "Command on macOS; Ctrl on Windows/Linux",
                                "K  Commands",
                                "N / O  New / Open",
                                "S / Shift + S  Save / Save as",
                                "Z / Shift + Z / Y  Undo / Redo",
                                "B / I / U  Bold / Italic / Underline",
                                "A / C / X / V  Select all / Copy / Cut / Paste",
                                "F / H  Find / Replace",
                                "Command/Ctrl + Enter  Page break",
                                "Shift + F  Focus mode (with Command/Ctrl)",
                                "Shift + D  Appearance (with Command/Ctrl)",
                                "Shift + I  Document info (with Command/Ctrl)",
                                "Shift + arrows  Extend selection",
                                "Escape  Close writing dialog / exit focus mode",
                            ] {
                                ui.label(text);
                            }
                        });
                });
            if !open {
                self.workbench.shortcuts = false;
                self.focus_canvas = true;
            }
        }
        if self.workbench.snippets {
            let mut open = true;
            let mut insert = None;
            let mut remove = None;
            egui::Window::new("Local snippets")
                .open(&mut open)
                .collapsible(false)
                .default_width(430.)
                .max_width((ctx.screen_rect().width() - 40.).max(250.))
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height((ctx.screen_rect().height() - 160.).clamp(100., 450.))
                        .show(ui, |ui| {
                            ui.label(
                                "Insert a block at the selection. Saved locally on this computer.",
                            );
                            for &(title, text) in BUILT_INS {
                                if ui
                                    .add_enabled(!self.read_only, egui::Button::new(title))
                                    .clicked()
                                {
                                    insert = Some(text.to_owned());
                                }
                            }
                            ui.separator();
                            for (index, snippet) in self.workspace_state.snippets.iter().enumerate()
                            {
                                ui.horizontal(|ui| {
                                    if ui
                                        .add_enabled_ui(!self.read_only, |ui| {
                                            ui.add_sized(
                                                [ui.available_width() - 40., 36.],
                                                egui::Button::new(&snippet.title).wrap(),
                                            )
                                        })
                                        .inner
                                        .clicked()
                                    {
                                        insert = Some(snippet.text.clone());
                                    }
                                    if ui
                                        .add(
                                            IconButton::new(
                                                Icon::Close,
                                                &format!("Remove {}", snippet.title),
                                            )
                                            .compact(),
                                        )
                                        .clicked()
                                    {
                                        remove = Some(index);
                                    }
                                });
                            }
                            let title_label = ui.label("New snippet title (1–80 characters)");
                            ui.text_edit_singleline(&mut self.workbench.title)
                                .labelled_by(title_label.id);
                            let text_label = ui.label("Text (up to 65,536 UTF-8 bytes)");
                            ui.add(
                                egui::TextEdit::multiline(&mut self.workbench.text)
                                    .desired_rows(4)
                                    .desired_width(f32::INFINITY),
                            )
                            .labelled_by(text_label.id);
                            if let Some(error) = &self.workbench.validation {
                                ui.colored_label(crate::theme::WARNING, error);
                            }
                            if ui
                                .add_enabled(
                                    self.workspace_state.snippets.len() < 32,
                                    egui::Button::new("Save snippet"),
                                )
                                .clicked()
                            {
                                let snippet = Snippet {
                                    title: self.workbench.title.trim().into(),
                                    text: self.workbench.text.clone(),
                                };
                                match snippet.validate() {
                                    Ok(()) => {
                                        self.workspace_state.snippets.push(snippet);
                                        self.workbench.title.clear();
                                        self.workbench.text.clear();
                                        self.workbench.validation = None;
                                    }
                                    Err(e) => self.workbench.validation = Some(e),
                                }
                            }
                            ui.label(format!(
                                "{} of 32 saved snippets",
                                self.workspace_state.snippets.len()
                            ));
                        });
                });
            if let Some(index) = remove {
                self.workspace_state.snippets.remove(index);
            }
            if let Some(text) = insert {
                self.workbench.snippets = false;
                self.insert(text);
                self.focus_canvas = true;
            } else if !open {
                self.workbench.snippets = false;
                self.focus_canvas = true;
            }
        }
    }
    pub(crate) fn writing_progress_panel(&mut self, ctx: &egui::Context) {
        if !self.workbench.progress || self.focus_mode {
            return;
        }
        egui::SidePanel::right("writing-progress")
            .resizable(false)
            .default_width(230.)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong("Writing progress");
                        if ui.button("Close").clicked() {
                            self.workbench.progress = false;
                        }
                    });
                    ui.separator();
                    let words = editing::document_statistics(self.editor.document()).words;
                    ui.label(format!("{words} words"));
                    let goal_label = ui.label("Word goal (0 disables)");
                    ui.add(
                        egui::DragValue::new(&mut self.workspace_state.word_goal)
                            .range(0..=1_000_000),
                    )
                    .labelled_by(goal_label.id);
                    if self.workspace_state.word_goal > 0 {
                        ui.add(
                            egui::ProgressBar::new(
                                (words as f32 / self.workspace_state.word_goal as f32).min(1.),
                            )
                            .show_percentage(),
                        );
                        ui.label(format!(
                            "{words} / {} words",
                            self.workspace_state.word_goal
                        ));
                    }
                    ui.label(format!(
                        "Reading: {} seconds (200 wpm)",
                        estimate(words, 200).as_secs()
                    ));
                    ui.label(format!(
                        "Speaking: {} seconds (130 wpm)",
                        estimate(words, 130).as_secs()
                    ));
                    ui.separator();
                    let now = Instant::now();
                    let elapsed = self.workbench.session.elapsed(now).as_secs();
                    ui.label(format!("Session: {:02}:{:02}", elapsed / 60, elapsed % 60));
                    ui.label(format!(
                        "Net words: {:+}",
                        self.workbench.session.net_words(words)
                    ));
                    ui.horizontal(|ui| {
                        if ui
                            .button(if self.workbench.session.running() {
                                "Pause"
                            } else {
                                "Start"
                            })
                            .clicked()
                        {
                            if self.workbench.session.running() {
                                self.workbench.session.pause(now);
                            } else {
                                self.workbench.session.start(now, words);
                            }
                        }
                        if ui.button("Reset").clicked() {
                            self.workbench.session.reset();
                        }
                    });
                    if self.workbench.session.running() {
                        ctx.request_repaint_after(std::time::Duration::from_secs(1));
                    }
                });
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(key: Key, command: bool) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                command,
                ctrl: command,
                ..Default::default()
            },
        }
    }
    fn frame(
        app: &mut FolioApp,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        ctx.begin_pass(egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(700., 500.),
            )),
            events,
            ..Default::default()
        });
        app.global_shortcuts(ctx);
        app.ribbon(ctx);
        let layout = app.canvas_state(ctx);
        app.writing_progress_panel(ctx);
        app.workbench_windows(ctx);
        app.paint_canvas(ctx, layout);
        ctx.end_pass()
    }
    #[test]
    fn palette_filters_routes_enter_and_owns_typing_shortcuts() {
        let ctx = egui::Context::default();
        crate::layout::install_fonts(&ctx);
        let mut app = FolioApp::default();
        app.insert("draft".into());
        let original = app.editor.document().clone();
        frame(&mut app, &ctx, vec![key(Key::K, true)]);
        assert!(app.workbench.palette);
        frame(
            &mut app,
            &ctx,
            vec![
                egui::Event::Text("Writing progress".into()),
                key(Key::B, true),
            ],
        );
        assert_eq!(app.workbench.query, "Writing progress");
        assert_eq!(app.editor.document(), &original);
        frame(&mut app, &ctx, vec![key(Key::Enter, false)]);
        assert!(app.workbench.progress);
        assert!(!app.workbench.palette);
        assert!(ctx.memory(|m| m.has_focus(egui::Id::new("document-canvas"))));
        assert_eq!(app.editor.document(), &original);
        app.open_palette();
        app.workbench.query = "nothing matches".into();
        frame(&mut app, &ctx, vec![key(Key::Enter, false)]);
        assert!(app.workbench.palette);
        frame(&mut app, &ctx, vec![key(Key::Escape, false)]);
        assert!(!app.workbench_blocks_editing());
        assert!(ctx.memory(|m| m.has_focus(egui::Id::new("document-canvas"))));
        assert_eq!(app.editor.document(), &original);
    }
    #[test]
    fn palette_arrows_and_accessible_view_controls() {
        let ctx = egui::Context::default();
        crate::layout::install_fonts(&ctx);
        ctx.enable_accesskit();
        let mut app = FolioApp {
            tab: crate::Tab::View,
            ..Default::default()
        };
        frame(&mut app, &ctx, vec![]);
        let output = frame(&mut app, &ctx, vec![]);
        let bounds = output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .find_map(|(_, node)| {
                (node.label() == Some("Navigation & writing"))
                    .then(|| node.bounds())
                    .flatten()
            })
            .unwrap();
        let pos = egui::pos2(
            ((bounds.x0 + bounds.x1) / 2.0) as f32,
            ((bounds.y0 + bounds.y1) / 2.0) as f32,
        );
        for pressed in [true, false] {
            frame(
                &mut app,
                &ctx,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ],
            );
        }
        let output = frame(&mut app, &ctx, vec![]);
        for label in [
            "Commands",
            "Writing progress",
            "Snippets",
            "Keyboard shortcuts",
        ] {
            assert!(
                output
                    .platform_output
                    .accesskit_update
                    .as_ref()
                    .unwrap()
                    .nodes
                    .iter()
                    .any(|(_, node)| node.label() == Some(label)),
                "missing {label}"
            );
        }
        app.open_palette();
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![key(Key::ArrowDown, false)]);
        assert_eq!(app.workbench.selected, 1);
        frame(&mut app, &ctx, vec![key(Key::ArrowUp, false)]);
        assert_eq!(app.workbench.selected, 0);
    }
    #[test]
    fn fit_page_width_reserves_only_visible_side_panels() {
        let ctx = egui::Context::default();
        ctx.begin_pass(egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200., 800.),
            )),
            ..Default::default()
        });
        let mut app = FolioApp::default();
        let page = app
            .editor
            .document()
            .page_layout
            .effective_size()
            .width_twips as f32
            / 15.;
        app.run_tool(Tool::FitWidth, &ctx);
        let full_width = app.zoom;
        assert!((full_width - 1120. / page).abs() < 0.0001);
        app.workbench.progress = true;
        app.run_tool(Tool::FitWidth, &ctx);
        assert!((app.zoom - 890. / page).abs() < 0.0001);
        app.show_document_info = true;
        app.run_tool(Tool::FitWidth, &ctx);
        assert!((app.zoom - 662. / page).abs() < 0.0001);
        app.run_tool(Tool::Focus, &ctx);
        assert!(app.workbench.progress);
        app.run_tool(Tool::FitWidth, &ctx);
        assert_eq!(app.zoom, full_width, "focus mode hides the progress panel");
        // The palette can toggle info while focus mode is active; its panel is hidden.
        app.run_tool(Tool::Info, &ctx);
        app.run_tool(Tool::FitWidth, &ctx);
        assert_eq!(app.zoom, full_width, "focus mode also hides document info");
        let _ = ctx.end_pass();
    }
    #[test]
    fn progress_start_pause_reset_controls_preserve_document() {
        let ctx = egui::Context::default();
        crate::layout::install_fonts(&ctx);
        ctx.enable_accesskit();
        let mut app = FolioApp::default();
        app.insert("one two".into());
        app.workbench.progress = true;
        let original = app.editor.document().clone();
        for label in ["Start", "Pause", "Reset"] {
            frame(&mut app, &ctx, vec![]);
            let output = frame(&mut app, &ctx, vec![]);
            let bounds = output
                .platform_output
                .accesskit_update
                .as_ref()
                .unwrap()
                .nodes
                .iter()
                .find_map(|(_, node)| {
                    (node.label() == Some(label))
                        .then(|| node.bounds())
                        .flatten()
                })
                .unwrap();
            let pos = egui::pos2(
                ((bounds.x0 + bounds.x1) / 2.) as f32,
                ((bounds.y0 + bounds.y1) / 2.) as f32,
            );
            for pressed in [true, false] {
                frame(
                    &mut app,
                    &ctx,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                );
            }
            assert_eq!(app.editor.document(), &original);
            assert_eq!(app.workbench.session.running(), label == "Start");
        }
        assert_eq!(app.workbench.session.net_words(2), 0);
    }
    #[test]
    fn disabled_palette_painter_does_not_mutate_history() {
        let ctx = egui::Context::default();
        crate::layout::install_fonts(&ctx);
        let mut app = FolioApp::default();
        app.run_tool(Tool::PaintFormat, &ctx);
        assert!(!app.editor.can_undo());
        app.open_palette();
        app.workbench.query = "Undo".into();
        frame(&mut app, &ctx, vec![]);
        frame(&mut app, &ctx, vec![key(Key::Enter, false)]);
        assert!(app.workbench.palette);
        assert!(!app.editor.can_undo());
    }
    #[test]
    fn painter_complete_style_one_undo_preserves_paragraph() {
        let ctx = egui::Context::default();
        crate::layout::install_fonts(&ctx);
        let mut app = FolioApp::default();
        app.insert("one\ntwo".into());
        app.editor
            .set_selection(Selection::new(Position::new(0, 0), Position::new(0, 3)))
            .unwrap();
        app.format(StylePatch {
            bold: Some(true),
            strikethrough: Some(true),
            vertical_align: Some(VerticalAlign::Subscript),
            highlight: Some(Some(Color::rgb(2, 4, 6))),
            ..Default::default()
        });
        app.run_tool(Tool::CaptureFormat, &ctx);
        app.editor
            .set_selection(Selection::new(Position::new(1, 0), Position::new(1, 3)))
            .unwrap();
        app.execute(Command::FormatParagraphs {
            selection: app.editor.selection(),
            patch: ParagraphPatch {
                alignment: Some(Alignment::Center),
                space_after_twips: Some(120),
                ..Default::default()
            },
        });
        let before = app.editor.document().clone();
        app.run_tool(Tool::PaintFormat, &ctx);
        assert_eq!(
            app.editor.document().paragraph(1).unwrap().runs[0].style,
            app.editor.document().paragraph(0).unwrap().runs[0].style
        );
        assert_eq!(
            app.editor.document().paragraph(1).unwrap().style,
            before.paragraph(1).unwrap().style
        );
        assert!(app.workbench.painter.is_none());
        app.execute(Command::Undo);
        assert_eq!(app.editor.document(), &before);
        app.run_tool(Tool::ClearParagraph, &ctx);
        assert_eq!(
            app.editor.document().paragraph(1).unwrap().style,
            ParagraphStyle::default()
        );
        assert_eq!(
            app.editor.document().paragraph(1).unwrap().runs,
            before.paragraph(1).unwrap().runs
        );
    }
    #[test]
    fn navigation_rejects_invalid_and_retains_unicode_wrap_affinity_and_empty_page() {
        let ctx = egui::Context::default();
        crate::layout::install_fonts(&ctx);
        let mut app = FolioApp::default();
        app.insert(format!("{}\nsecond", "界e\u{301} ".repeat(100)));
        ctx.begin_pass(egui::RawInput::default());
        let layout = DocumentLayout::build(&ctx, app.editor.document(), 1.);
        assert!(layout.lines.len() > 2);
        let original = app.editor.selection();
        for invalid in ["0", "-1", "1.1", "99999", "界"] {
            app.workbench.number = invalid.into();
            app.workbench.navigation = Some(Navigation::Line);
            app.navigate_workbench(&ctx, Navigation::Line);
            assert_eq!(app.editor.selection(), original);
            assert!(app.workbench.validation.is_some());
        }
        app.workbench.number = "2".into();
        app.navigate_workbench(&ctx, Navigation::Line);
        assert_eq!(app.editor.selection().focus, layout.lines[1].stops[0].at);
        assert_eq!(app.visual_line, Some(1));
        app.workbench.number = "2".into();
        app.navigate_workbench(&ctx, Navigation::Paragraph);
        assert_eq!(app.editor.selection().focus, Position::new(1, 0));
        let _ = ctx.end_pass();
        app.editor = Editor::new(Document {
            blocks: vec![
                Block::Paragraph(Paragraph::default()),
                Block::PageBreak,
                Block::PageBreak,
                Block::Paragraph(Paragraph::default()),
            ],
            ..Default::default()
        })
        .unwrap();
        let before = app.editor.selection();
        ctx.begin_pass(egui::RawInput::default());
        app.workbench.number = "2".into();
        app.navigate_workbench(&ctx, Navigation::Page);
        assert_eq!(app.editor.selection(), before);
        assert_eq!(app.workbench.reveal_page, Some(1));
        let _ = ctx.end_pass();
    }
    #[test]
    fn date_and_snippet_are_single_replacements_with_undo() {
        let ctx = egui::Context::default();
        crate::layout::install_fonts(&ctx);
        let mut app = FolioApp::default();
        app.insert("replace me".into());
        app.editor
            .set_selection(editing::select_all(app.editor.document()))
            .unwrap();
        let before = app.editor.document().clone();
        app.run_tool(Tool::DateTime, &ctx);
        assert_ne!(app.editor.document(), &before);
        app.execute(Command::Undo);
        assert_eq!(app.editor.document(), &before);
        app.insert(BUILT_INS[0].1.into());
        app.execute(Command::Undo);
        assert_eq!(app.editor.document(), &before);
    }
}
