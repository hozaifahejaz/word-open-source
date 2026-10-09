//! Model editing and platform-independent command routing.
use document_core::*;
use egui::{Key, Modifiers};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    New,
    Open,
    Save,
    SaveAs,
    Quit,
    Undo,
    Redo,
    Bold,
    Italic,
    Underline,
    ClearFormatting,
    Cut,
    Copy,
    Paste,
    SelectAll,
    Find,
    PageBreak,
}
pub fn shortcut(key: Key, m: Modifiers) -> Option<Action> {
    if !m.command || m.alt {
        return None;
    }
    Some(match key {
        Key::N => Action::New,
        Key::O => Action::Open,
        Key::Q | Key::W => Action::Quit,
        Key::S if m.shift => Action::SaveAs,
        Key::S => Action::Save,
        Key::Z if m.shift => Action::Redo,
        Key::Z => Action::Undo,
        Key::Y => Action::Redo,
        Key::B => Action::Bold,
        Key::I => Action::Italic,
        Key::U => Action::Underline,
        Key::A => Action::SelectAll,
        Key::F | Key::H => Action::Find,
        Key::Enter => Action::PageBreak,
        _ => return None,
    })
}

pub fn style_at(editor: &Editor) -> TextStyle {
    let at = editor.selection().focus;
    let p = editor
        .document()
        .paragraph(at.block)
        .expect("validated caret");
    let mut end = 0;
    for run in &p.runs {
        end += run.text.len();
        if at.offset <= end {
            return run.style.clone();
        }
    }
    p.default_style.clone()
}
pub fn adjacent(doc: &Document, at: Position, forward: bool) -> Position {
    let p = doc.paragraph(at.block).expect("validated caret");
    if forward && at.offset < p.len_bytes() {
        return Position::new(at.block, p.next_boundary(at.offset).unwrap());
    }
    if !forward && at.offset > 0 {
        return Position::new(at.block, p.previous_boundary(at.offset).unwrap());
    }
    let indexes: Box<dyn Iterator<Item = usize>> = if forward {
        Box::new(at.block + 1..doc.blocks.len())
    } else {
        Box::new((0..at.block).rev())
    };
    for i in indexes {
        if let Ok(p) = doc.paragraph(i) {
            return Position::new(i, if forward { 0 } else { p.len_bytes() });
        }
    }
    at
}
pub fn select_all(doc: &Document) -> Selection {
    let last = doc.blocks.len() - 1;
    Selection::new(
        Position::new(0, 0),
        Position::new(last, doc.paragraph(last).unwrap().len_bytes()),
    )
}
pub fn selected_text(editor: &Editor) -> String {
    let (start, end) = editor.selection().ordered();
    let mut result = String::new();
    for i in start.block..=end.block {
        if let Ok(p) = editor.document().paragraph(i) {
            if i != start.block {
                result.push('\n');
            }
            let text = p.text();
            result.push_str(
                &text[if i == start.block { start.offset } else { 0 }..if i == end.block {
                    end.offset
                } else {
                    text.len()
                }],
            );
        }
    }
    result
}

#[derive(Clone, Copy)]
pub enum TextCase {
    Upper,
    Lower,
    Title,
    Sentence,
}

pub fn transform_case(text: &str, case: TextCase) -> String {
    match case {
        TextCase::Upper => text.to_uppercase(),
        TextCase::Lower => text.to_lowercase(),
        TextCase::Title => {
            let mut result = String::with_capacity(text.len());
            for token in text.split_inclusive(char::is_whitespace) {
                let first = token.graphemes(true).next().unwrap();
                result.push_str(&first.to_uppercase());
                result.push_str(&token[first.len()..].to_lowercase());
            }
            result
        }
        TextCase::Sentence => {
            let lower = text.to_lowercase();
            let mut result = String::with_capacity(lower.len());
            let mut capitalize = true;
            let mut after_terminal = false;
            for grapheme in lower.graphemes(true) {
                if grapheme.chars().all(char::is_whitespace) {
                    result.push_str(grapheme);
                    if after_terminal {
                        capitalize = true;
                    }
                } else {
                    if capitalize {
                        result.push_str(&grapheme.to_uppercase());
                    } else {
                        result.push_str(grapheme);
                    }
                    capitalize = false;
                }
                after_terminal = matches!(grapheme, "." | "?" | "!");
            }
            result
        }
    }
}

/// Visible labels and spoken action names accompany the exact inserted sequences.
pub const SYMBOLS: &[(&str, &str, &str)] = &[
    ("Nonbreaking space", "Insert nonbreaking space", "\u{00a0}"),
    (
        "Nonbreaking hyphen ‑",
        "Insert nonbreaking hyphen",
        "\u{2011}",
    ),
    ("Em dash —", "Insert em dash", "\u{2014}"),
    ("Ellipsis …", "Insert ellipsis", "\u{2026}"),
    ("Bullet •", "Insert bullet", "\u{2022}"),
    ("Copyright ©", "Insert copyright", "\u{00a9}"),
    ("Pound £", "Insert pound", "\u{00a3}"),
    ("Euro €", "Insert euro", "\u{20ac}"),
    ("Yen ¥", "Insert yen", "\u{00a5}"),
    ("Plus/minus ±", "Insert plus/minus", "\u{00b1}"),
    ("Multiplication ×", "Insert multiplication", "\u{00d7}"),
    ("Division ÷", "Insert division", "\u{00f7}"),
    ("Left arrow ←", "Insert left arrow", "\u{2190}"),
    ("Right arrow →", "Insert right arrow", "\u{2192}"),
    ("Up arrow ↑", "Insert up arrow", "\u{2191}"),
    ("Down arrow ↓", "Insert down arrow", "\u{2193}"),
    ("Check mark ✓", "Insert check mark", "\u{2713}"),
    ("Grinning face 😀", "Insert grinning face", "\u{1f600}"),
    ("Red heart ❤️", "Insert red heart", "\u{2764}\u{fe0f}"),
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Statistics {
    pub words: usize,
    /// User-perceived characters, including spaces; structural breaks are excluded.
    pub characters: usize,
}

pub fn document_statistics(doc: &Document) -> Statistics {
    statistics(doc, select_all(doc))
}

pub fn selection_statistics(editor: &Editor) -> Statistics {
    statistics(editor.document(), editor.selection())
}

fn statistics(doc: &Document, selection: Selection) -> Statistics {
    let (start, end) = selection.ordered();
    let mut result = Statistics::default();
    for block in start.block..=end.block {
        if let Ok(p) = doc.paragraph(block) {
            let text = p.text();
            let from = if block == start.block {
                start.offset
            } else {
                0
            };
            let to = if block == end.block {
                end.offset
            } else {
                text.len()
            };
            let selected = &text[from..to];
            result.words += selected.split_whitespace().count();
            result.characters += selected.graphemes(true).count();
        }
    }
    result
}
pub fn delete_command(editor: &Editor, forward: bool) -> Command {
    let selection = editor.selection();
    Command::Delete {
        selection: if selection.is_collapsed() {
            Selection::new(
                selection.focus,
                adjacent(editor.document(), selection.focus, forward),
            )
        } else {
            selection
        },
    }
}
pub fn move_to(editor: &mut Editor, at: Position, extend: bool) {
    let anchor = if extend {
        editor.selection().anchor
    } else {
        at
    };
    editor
        .set_selection(Selection::new(anchor, at))
        .expect("layout returns valid positions");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn statistics_count_graphemes_and_keep_structural_breaks_out_of_characters() {
        let mut e = Editor::new(Document {
            blocks: vec![
                Block::Paragraph(Paragraph::plain("Café e\u{301}")),
                Block::PageBreak,
                Block::Paragraph(Paragraph::plain("👩‍💻 hi")),
            ],
            ..Default::default()
        })
        .unwrap();
        assert_eq!(
            document_statistics(e.document()),
            Statistics {
                words: 4,
                characters: 10
            }
        );
        e.set_selection(Selection::new(Position::new(2, 14), Position::new(0, 6)))
            .unwrap();
        assert_eq!(
            selection_statistics(&e),
            Statistics {
                words: 3,
                characters: 5
            }
        );
        e.set_selection(Selection::caret(Position::new(0, 0)))
            .unwrap();
        assert_eq!(
            selection_statistics(&e),
            Statistics {
                words: 0,
                characters: 0
            }
        );
    }
    #[test]
    fn command_routes_respect_platform_command_and_shift() {
        let m = Modifiers {
            command: true,
            ..Modifiers::default()
        };
        assert_eq!(shortcut(Key::S, m), Some(Action::Save));
        assert_eq!(
            shortcut(Key::S, Modifiers { shift: true, ..m }),
            Some(Action::SaveAs)
        );
        assert_eq!(
            shortcut(Key::Z, Modifiers { shift: true, ..m }),
            Some(Action::Redo)
        );
        assert_eq!(shortcut(Key::B, Modifiers::default()), None);
        assert_eq!(shortcut(Key::B, Modifiers { alt: true, ..m }), None);
    }
    #[test]
    fn cross_page_delete_undo_and_graphemes() {
        let mut e = Editor::default();
        e.execute(Command::InsertText {
            at: Position::default(),
            text: "e\u{301}👩‍👩‍👦".into(),
            style: None,
        })
        .unwrap();
        let end = e.selection().focus;
        assert_eq!(adjacent(e.document(), end, false).offset, 3);
        e.execute(Command::InsertPageBreak { at: end }).unwrap();
        e.execute(Command::InsertText {
            at: e.selection().focus,
            text: "next".into(),
            style: None,
        })
        .unwrap();
        e.set_selection(Selection::caret(Position::new(2, 0)))
            .unwrap();
        e.execute(delete_command(&e, false)).unwrap();
        assert_eq!(e.document().blocks.len(), 1);
        e.execute(Command::Undo).unwrap();
        assert_eq!(e.document().blocks.len(), 3);
        assert_eq!(e.selection().focus, Position::new(2, 0));
    }
}
