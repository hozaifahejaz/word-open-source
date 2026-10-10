//! Offline writing tools and shared command metadata.
use crate::{editing::Action as EditorAction, layout::DocumentLayout};
use document_core::{Block, Document, Position, TextStyle};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snippet {
    pub title: String,
    pub text: String,
}
impl Snippet {
    pub fn validate(&self) -> Result<(), String> {
        let count = self.title.trim().chars().count();
        if !(1..=80).contains(&count) {
            return Err("Snippet title must contain 1–80 characters after trimming".into());
        }
        if self.text.trim().is_empty() || self.text.len() > 65_536 {
            return Err("Snippet text must be nonempty and at most 65,536 UTF-8 bytes".into());
        }
        Ok(())
    }
}
pub const BUILT_INS: &[(&str, &str)] = &[
    (
        "Meeting notes",
        "Meeting notes\nDate: \nAttendees: \nDiscussion\nDecisions\nNext steps",
    ),
    (
        "Project outline",
        "Project outline\nPurpose\nScope\nMilestones\nRisks\nNext steps",
    ),
    (
        "Daily reflection",
        "Daily reflection\nWhat went well?\nWhat did I learn?\nWhat will I focus on tomorrow?",
    ),
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Navigation {
    Page,
    Line,
    Paragraph,
}
impl Navigation {
    pub fn label(self) -> &'static str {
        match self {
            Self::Page => "page",
            Self::Line => "visual line",
            Self::Paragraph => "paragraph",
        }
    }
}
pub fn paragraph_position(doc: &Document, number: usize) -> Option<Position> {
    number
        .checked_sub(1)
        .and_then(|n| {
            doc.blocks
                .iter()
                .enumerate()
                .filter(|(_, b)| matches!(b, Block::Paragraph(_)))
                .nth(n)
        })
        .map(|(b, _)| Position::new(b, 0))
}
pub fn line_position(layout: &DocumentLayout, number: usize) -> Option<(Position, usize)> {
    let index = number.checked_sub(1)?;
    Some((layout.lines.get(index)?.stops.first()?.at, index))
}
pub fn page_position(layout: &DocumentLayout, number: usize) -> Option<(Position, usize)> {
    let page = number.checked_sub(1)?;
    layout
        .lines
        .iter()
        .enumerate()
        .find(|(_, l)| l.page == page)
        .and_then(|(i, l)| Some((l.stops.first()?.at, i)))
}
pub fn estimate(words: usize, per_minute: usize) -> Duration {
    Duration::from_secs((words as u64 * 60).div_ceil(per_minute as u64))
}
#[derive(Default)]
pub struct Session {
    started: Option<Instant>,
    elapsed: Duration,
    baseline: Option<usize>,
}
impl Session {
    pub fn start(&mut self, now: Instant, words: usize) {
        self.baseline.get_or_insert(words);
        self.started.get_or_insert(now);
    }
    pub fn elapsed(&self, now: Instant) -> Duration {
        self.elapsed
            + self
                .started
                .map_or(Duration::ZERO, |s| now.saturating_duration_since(s))
    }
    pub fn pause(&mut self, now: Instant) {
        self.elapsed = self.elapsed(now);
        self.started = None;
    }
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn running(&self) -> bool {
        self.started.is_some()
    }
    pub fn net_words(&self, words: usize) -> i64 {
        self.baseline.map_or(0, |b| words as i64 - b as i64)
    }
}
#[derive(Clone, Copy)]
pub enum Tool {
    Action(EditorAction),
    Palette,
    Templates,
    Duplicate,
    ExportDocument,
    ExportText,
    ExportSelection,
    ReadOnly,
    Navigate(Navigation),
    Snippets,
    Shortcuts,
    Progress,
    DateTime,
    CaptureFormat,
    PaintFormat,
    ClearParagraph,
    Focus,
    Theme,
    Info,
    Connection,
    Case(document_core::TextCase),
    Highlight,
    Home,
    Layout,
    Align(document_core::Alignment),
    Symbol(&'static str),
    FitWidth,
    Zoom100,
}
#[derive(Clone, Copy)]
pub struct NamedCommand {
    pub label: &'static str,
    pub tool: Tool,
}
pub fn commands() -> Vec<NamedCommand> {
    use Tool::*;
    let mut result = vec![];
    for (label, action) in [
        ("New document", EditorAction::New),
        ("Open document", EditorAction::Open),
        ("Save", EditorAction::Save),
        ("Save as", EditorAction::SaveAs),
        ("Quit", EditorAction::Quit),
        ("Undo", EditorAction::Undo),
        ("Redo", EditorAction::Redo),
        ("Copy", EditorAction::Copy),
        ("Cut", EditorAction::Cut),
        ("Paste", EditorAction::Paste),
        ("Select all", EditorAction::SelectAll),
        ("Find and replace", EditorAction::Find),
        ("Bold", EditorAction::Bold),
        ("Italic", EditorAction::Italic),
        ("Underline", EditorAction::Underline),
        ("Strikethrough", EditorAction::Strike),
        ("Superscript", EditorAction::Superscript),
        ("Subscript", EditorAction::Subscript),
        ("Clear text formatting", EditorAction::ClearFormatting),
        ("Insert page break", EditorAction::PageBreak),
    ] {
        result.push(NamedCommand {
            label,
            tool: Action(action),
        });
    }
    for (label, tool) in [
        ("Template gallery", Templates),
        ("Duplicate document", Duplicate),
        ("Export document…", ExportDocument),
        ("Export document as text", ExportText),
        ("Export selection as text", ExportSelection),
        ("Toggle read-only mode", ReadOnly),
        ("Go to page", Navigate(Navigation::Page)),
        ("Go to visual line", Navigate(Navigation::Line)),
        ("Go to paragraph", Navigate(Navigation::Paragraph)),
        ("Insert local date and time", DateTime),
        ("Capture format painter", CaptureFormat),
        ("Apply format painter", PaintFormat),
        ("Clear paragraph formatting", ClearParagraph),
        ("Writing progress", Progress),
        ("Insert snippets", Snippets),
        ("Keyboard shortcuts", Shortcuts),
        ("Focus mode", Focus),
        ("Switch appearance", Theme),
        ("Document info", Info),
        ("AI connection", Connection),
        ("Uppercase selection", Case(document_core::TextCase::Upper)),
        ("Lowercase selection", Case(document_core::TextCase::Lower)),
        ("Title case selection", Case(document_core::TextCase::Title)),
        (
            "Sentence case selection",
            Case(document_core::TextCase::Sentence),
        ),
        ("Highlight selection yellow", Highlight),
        ("Text formatting controls", Home),
        ("Page layout controls", Layout),
        ("Align left", Align(document_core::Alignment::Left)),
        ("Align center", Align(document_core::Alignment::Center)),
        ("Align right", Align(document_core::Alignment::Right)),
        (
            "Justify paragraphs",
            Align(document_core::Alignment::Justify),
        ),
        ("Fit page width", FitWidth),
        ("Zoom 100%", Zoom100),
    ] {
        result.push(NamedCommand { label, tool });
    }
    for &(_, label, text) in crate::editing::SYMBOLS {
        result.push(NamedCommand {
            label,
            tool: Symbol(text),
        });
    }
    result
}
#[derive(Default)]
pub struct Workbench {
    pub palette: bool,
    pub query: String,
    pub selected: usize,
    pub focus: bool,
    pub navigation: Option<Navigation>,
    pub number: String,
    pub validation: Option<String>,
    pub snippets: bool,
    pub title: String,
    pub text: String,
    pub shortcuts: bool,
    pub progress: bool,
    pub painter: Option<TextStyle>,
    pub session: Session,
    pub reveal_page: Option<usize>,
}
pub fn local_datetime() -> String {
    if let Ok(output) = std::process::Command::new("date")
        .arg("+%Y-%m-%d %H:%M %Z")
        .output()
        && output.status.success()
        && let Ok(value) = String::from_utf8(output.stdout)
        && !value.trim().is_empty()
    {
        return value.trim().into();
    }
    // UTC fallback uses the system clock; label the zone explicitly.
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    utc_datetime(seconds)
}
fn utc_datetime(seconds: u64) -> String {
    let days = (seconds / 86400) as i64 + 719468;
    let era = days / 146097;
    let doe = days - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        seconds / 3600 % 24,
        seconds / 60 % 60
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn datetime_fallback_is_labeled_and_calendar_correct() {
        assert_eq!(utc_datetime(0), "1970-01-01 00:00 UTC");
        assert_eq!(utc_datetime(951868740), "2000-02-29 23:59 UTC");
        let local = local_datetime();
        assert!(local.len() >= 20 && local.len() <= 80);
        assert!(local.contains(':'));
        assert!(!local.contains('\n'));
    }
    #[test]
    fn timer_pause_reset_and_deletions() {
        let now = Instant::now();
        let mut s = Session::default();
        s.start(now, 10);
        s.start(now + Duration::from_secs(2), 99);
        s.pause(now + Duration::from_secs(5));
        assert_eq!(s.elapsed(now + Duration::from_secs(50)).as_secs(), 5);
        assert_eq!(s.net_words(7), -3);
        s.start(now + Duration::from_secs(60), 7);
        assert_eq!(s.elapsed(now + Duration::from_secs(62)).as_secs(), 7);
        s.reset();
        assert_eq!(s.net_words(20), 0);
        assert!(!s.running());
    }
    #[test]
    fn snippet_unicode_limits_and_estimates() {
        assert!(
            Snippet {
                title: "界".repeat(80),
                text: "x".into()
            }
            .validate()
            .is_ok()
        );
        assert!(
            Snippet {
                title: "界".repeat(81),
                text: "x".into()
            }
            .validate()
            .is_err()
        );
        assert!(
            Snippet {
                title: "ok".into(),
                text: "界".repeat(21846)
            }
            .validate()
            .is_err()
        );
        assert_eq!(estimate(201, 200).as_secs(), 61);
        assert_eq!(estimate(130, 130).as_secs(), 60);
    }
}
