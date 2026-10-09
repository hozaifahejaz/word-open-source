use crate::*;

/// All UI mutations use this command surface. Commands are atomic and undoable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    InsertText {
        at: Position,
        text: String,
        style: Option<TextStyle>,
    },
    ReplaceText {
        selection: Selection,
        text: String,
        style: Option<TextStyle>,
    },
    Delete {
        selection: Selection,
    },
    SplitParagraph {
        at: Position,
    },
    JoinParagraph {
        block: usize,
    },
    FormatRuns {
        selection: Selection,
        patch: StylePatch,
    },
    FormatParagraphs {
        selection: Selection,
        patch: ParagraphPatch,
    },
    InsertPageBreak {
        at: Position,
    },
    ReplaceWithPageBreak {
        selection: Selection,
    },
    SetPageLayout {
        layout: PageLayout,
    },
    ReplaceAll {
        needle: String,
        replacement: String,
    },
    ReplaceAllWithOptions {
        needle: String,
        replacement: String,
        options: SearchOptions,
    },
    Undo,
    Redo,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditOutcome {
    pub selection: Selection,
    pub changed: bool,
    pub replacements: usize,
}

#[derive(Clone, Debug)]
pub struct Editor {
    document: Document,
    selection: Selection,
    saved: Document,
    recovered_dirty: bool,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    history_limit: usize,
}
#[derive(Clone, Debug)]
struct Snapshot {
    document: Document,
    selection: Selection,
}
impl Default for Editor {
    fn default() -> Self {
        Self::new(Document::default()).expect("valid default document")
    }
}
impl Editor {
    pub fn new(mut document: Document) -> Result<Self, CoreError> {
        document.validate()?;
        document.normalize();
        Ok(Self {
            saved: document.clone(),
            recovered_dirty: false,
            document,
            selection: Selection::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            history_limit: 100,
        })
    }
    pub fn document(&self) -> &Document {
        &self.document
    }
    pub fn selection(&self) -> Selection {
        self.selection
    }
    pub fn is_dirty(&self) -> bool {
        self.recovered_dirty || self.document != self.saved
    }
    pub fn mark_saved(&mut self) {
        self.saved = self.document.clone();
        self.recovered_dirty = false;
    }
    pub fn set_selection(&mut self, selection: Selection) -> Result<(), CoreError> {
        self.document.validate_selection(selection)?;
        self.selection = selection;
        Ok(())
    }
    /// Replace a successfully imported document and reset history/save state.
    pub fn load_document(&mut self, document: Document) -> Result<(), CoreError> {
        let mut loaded = Self::new(document)?;
        loaded.history_limit = self.history_limit;
        *self = loaded;
        Ok(())
    }
    /// Restore recovered content without history, keeping it dirty until saved.
    /// Invalid selections fall back to the start of the normalized document.
    pub fn load_recovered_document(
        &mut self,
        document: Document,
        selection: Selection,
    ) -> Result<(), CoreError> {
        let mut recovered = Self::new(document)?;
        recovered.selection = if recovered.document.validate_selection(selection).is_ok() {
            selection
        } else {
            Selection::caret(Position::new(0, 0))
        };
        recovered.recovered_dirty = true;
        recovered.history_limit = self.history_limit;
        *self = recovered;
        Ok(())
    }
    /// Bound snapshot history. Zero disables undo storage.
    pub fn set_history_limit(&mut self, limit: usize) {
        self.history_limit = limit;
        trim_history(&mut self.undo, limit);
        trim_history(&mut self.redo, limit);
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            document: self.document.clone(),
            selection: self.selection,
        }
    }
    pub fn execute(&mut self, command: Command) -> Result<EditOutcome, CoreError> {
        if command == Command::Undo || command == Command::Redo {
            let previous = if command == Command::Undo {
                self.undo.pop()
            } else {
                self.redo.pop()
            };
            if let Some(previous) = previous {
                let current = self.snapshot();
                if command == Command::Undo {
                    self.redo.push(current);
                } else {
                    self.undo.push(current);
                }
                self.document = previous.document;
                self.selection = previous.selection;
                return Ok(EditOutcome {
                    selection: self.selection,
                    changed: true,
                    replacements: 0,
                });
            }
            return Ok(EditOutcome {
                selection: self.selection,
                changed: false,
                replacements: 0,
            });
        }
        // Work on a candidate: even late validation errors cannot leak mutations.
        let mut candidate = self.document.clone();
        let (selection, replacements) = apply_command(&mut candidate, self.selection, command)?;
        candidate.normalize();
        candidate.validate()?;
        candidate.validate_selection(selection)?;
        let changed = candidate != self.document;
        if changed {
            self.undo.push(self.snapshot());
            trim_history(&mut self.undo, self.history_limit);
            self.redo.clear();
            self.document = candidate;
        }
        self.selection = selection;
        Ok(EditOutcome {
            selection,
            changed,
            replacements,
        })
    }
}

fn trim_history(history: &mut Vec<Snapshot>, limit: usize) {
    if history.len() > limit {
        history.drain(..history.len() - limit);
    }
}

fn apply_command(
    doc: &mut Document,
    current: Selection,
    command: Command,
) -> Result<(Selection, usize), CoreError> {
    let mut replacements = 0;
    let selection = match command {
        Command::InsertText { at, text, style } => {
            replace(doc, Selection::caret(at), &text, style)?
        }
        Command::ReplaceText {
            selection,
            text,
            style,
        } => replace(doc, selection, &text, style)?,
        Command::Delete { selection } => replace(doc, selection, "", None)?,
        Command::SplitParagraph { at } => replace(doc, Selection::caret(at), "\n", None)?,
        Command::JoinParagraph { block } => {
            let end = doc.paragraph(block)?.len_bytes();
            if doc.paragraph(block + 1).is_err() {
                return Err(CoreError::CannotJoin);
            }
            replace(
                doc,
                Selection::new(Position::new(block, end), Position::new(block + 1, 0)),
                "",
                None,
            )?
        }
        Command::InsertPageBreak { at } => insert_page_break(doc, at)?,
        Command::ReplaceWithPageBreak { selection } => {
            let caret = replace(doc, selection, "", None)?;
            insert_page_break(doc, caret.focus)?
        }
        Command::FormatRuns { selection, patch } => {
            doc.validate_selection(selection)?;
            let (start, end) = selection.ordered();
            if start != end {
                for block in start.block..=end.block {
                    if let Block::Paragraph(p) = &mut doc.blocks[block] {
                        let from = if block == start.block {
                            start.offset
                        } else {
                            0
                        };
                        let to = if block == end.block {
                            end.offset
                        } else {
                            p.len_bytes()
                        };
                        if from < to {
                            let mut runs = slice_runs(p, 0, from);
                            let mut selected = slice_runs(p, from, to);
                            for run in &mut selected {
                                patch.apply(&mut run.style);
                            }
                            runs.extend(selected);
                            runs.extend(slice_runs(p, to, p.len_bytes()));
                            p.runs = runs;
                        } else if p.runs.is_empty() && block != end.block {
                            patch.apply(&mut p.default_style);
                        }
                    }
                }
            }
            selection
        }
        Command::FormatParagraphs { selection, patch } => {
            doc.validate_selection(selection)?;
            let (start, end) = selection.ordered();
            let last = if start.block != end.block && end.offset == 0 {
                end.block - 1
            } else {
                end.block
            };
            for block in start.block..=last {
                if let Block::Paragraph(p) = &mut doc.blocks[block] {
                    patch.apply(&mut p.style);
                }
            }
            selection
        }
        Command::SetPageLayout { layout } => {
            doc.page_layout = layout;
            current
        }
        Command::ReplaceAll {
            needle,
            replacement,
        } => {
            return apply_command(
                doc,
                current,
                Command::ReplaceAllWithOptions {
                    needle,
                    replacement,
                    options: SearchOptions::default(),
                },
            );
        }
        Command::ReplaceAllWithOptions {
            needle,
            replacement,
            options,
        } => {
            let matches = doc.find_with_options(&needle, options)?;
            replacements = matches.len();
            let mut caret = current;
            for selection in matches.into_iter().rev() {
                // These ranges were validated against the original document.
                // Later replacements can merge graphemes with earlier matches,
                // but reverse order preserves their original UTF-8 offsets.
                caret = replace_validated(doc, selection, &replacement, None)?;
            }
            caret
        }
        Command::Undo | Command::Redo => unreachable!("handled by Editor"),
    };
    Ok((selection, replacements))
}

fn insert_page_break(doc: &mut Document, at: Position) -> Result<Selection, CoreError> {
    doc.validate_position(at)?;
    let original = doc.paragraph(at.block)?.clone();
    let mut left = original.clone();
    let mut right = original.clone();
    left.runs = slice_runs(&original, 0, at.offset);
    right.runs = slice_runs(&original, at.offset, original.len_bytes());
    right.default_style = style_at(&original, at.offset);
    doc.blocks.splice(
        at.block..=at.block,
        [
            Block::Paragraph(left),
            Block::PageBreak,
            Block::Paragraph(right),
        ],
    );
    Ok(Selection::caret(Position::new(at.block + 2, 0)))
}

/// Slice by offsets in the concatenated text, retaining each run's style.
fn slice_runs(p: &Paragraph, from: usize, to: usize) -> Vec<Run> {
    let mut offset = 0;
    let mut runs = Vec::new();
    for run in &p.runs {
        let end = offset + run.text.len();
        let left = from.max(offset);
        let right = to.min(end);
        if left < right {
            runs.push(Run::new(
                &run.text[left - offset..right - offset],
                run.style.clone(),
            ));
        }
        offset = end;
    }
    runs
}

/// Insertion inherits the run to the left; paragraph start uses the first run.
fn style_at(p: &Paragraph, at: usize) -> TextStyle {
    let mut end = 0;
    for run in &p.runs {
        end += run.text.len();
        if at <= end {
            return run.style.clone();
        }
    }
    p.default_style.clone()
}

fn replace(
    doc: &mut Document,
    selection: Selection,
    text: &str,
    style: Option<TextStyle>,
) -> Result<Selection, CoreError> {
    doc.validate_selection(selection)?;
    replace_validated(doc, selection, text, style)
}

fn replace_validated(
    doc: &mut Document,
    selection: Selection,
    text: &str,
    style: Option<TextStyle>,
) -> Result<Selection, CoreError> {
    if text.contains('\u{000c}') {
        return Err(CoreError::InvalidText);
    }
    let (start, end) = selection.ordered();
    let first = doc.paragraph(start.block)?.clone();
    let last = doc.paragraph(end.block)?.clone();
    let style = style.unwrap_or_else(|| style_at(&first, start.offset));
    style.validate()?;
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let parts: Vec<&str> = text.split('\n').collect();
    let mut blocks = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        let mut paragraph = if index == parts.len() - 1 && index > 0 {
            last.clone()
        } else {
            first.clone()
        };
        paragraph.runs.clear();
        paragraph.default_style = style.clone();
        if index == 0 {
            paragraph.runs.extend(slice_runs(&first, 0, start.offset));
        }
        if !part.is_empty() {
            paragraph.runs.push(Run::new(*part, style.clone()));
        }
        if index == parts.len() - 1 {
            paragraph
                .runs
                .extend(slice_runs(&last, end.offset, last.len_bytes()));
        }
        blocks.push(Block::Paragraph(paragraph));
    }
    // Empty insertions must not change an empty paragraph's typing style.
    if start == end && text.is_empty() {
        return Ok(Selection::caret(start));
    }
    let caret_block = start.block + parts.len() - 1;
    let caret_offset =
        parts.last().unwrap().len() + if parts.len() == 1 { start.offset } else { 0 };
    doc.blocks.splice(start.block..=end.block, blocks);
    let caret_offset = doc.paragraph(caret_block)?.snap_forward(caret_offset);
    Ok(Selection::caret(Position::new(caret_block, caret_offset)))
}
