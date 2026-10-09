use std::{error::Error, fmt};
use unicode_segmentation::UnicodeSegmentation;

/// Positions use UTF-8 byte offsets in a paragraph's concatenated text.
/// Only extended grapheme boundaries are valid editing positions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub block: usize,
    pub offset: usize,
}

impl Position {
    pub const fn new(block: usize, offset: usize) -> Self {
        Self { block, offset }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub anchor: Position,
    pub focus: Position,
}

impl Selection {
    pub const fn caret(at: Position) -> Self {
        Self {
            anchor: at,
            focus: at,
        }
    }
    pub const fn new(anchor: Position, focus: Position) -> Self {
        Self { anchor, focus }
    }
    pub fn ordered(self) -> (Position, Position) {
        if self.anchor <= self.focus {
            (self.anchor, self.focus)
        } else {
            (self.focus, self.anchor)
        }
    }
    pub fn is_collapsed(self) -> bool {
        self.anchor == self.focus
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}
impl Color {
    pub const BLACK: Self = Self {
        red: 0,
        green: 0,
        blue: 0,
    };
    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }
}

/// Font sizes are half-points (24 means 12pt), matching WordprocessingML.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextStyle {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub font_family: String,
    pub size_half_points: u16,
    pub color: Color,
}
impl Default for TextStyle {
    fn default() -> Self {
        Self {
            bold: false,
            italic: false,
            underline: false,
            font_family: "sans-serif".into(),
            size_half_points: 24,
            color: Color::BLACK,
        }
    }
}
impl TextStyle {
    pub fn validate(&self) -> Result<(), CoreError> {
        if self.font_family.trim().is_empty() || self.size_half_points == 0 {
            return Err(CoreError::InvalidStyle);
        }
        Ok(())
    }
}

/// None preserves a property; Some(false) explicitly clears a boolean.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StylePatch {
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub font_family: Option<String>,
    pub size_half_points: Option<u16>,
    pub color: Option<Color>,
}
impl StylePatch {
    pub(crate) fn apply(&self, style: &mut TextStyle) {
        if let Some(v) = self.bold {
            style.bold = v;
        }
        if let Some(v) = self.italic {
            style.italic = v;
        }
        if let Some(v) = self.underline {
            style.underline = v;
        }
        if let Some(v) = &self.font_family {
            style.font_family = v.clone();
        }
        if let Some(v) = self.size_half_points {
            style.size_half_points = v;
        }
        if let Some(v) = self.color {
            style.color = v;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    pub text: String,
    pub style: TextStyle,
}
impl Run {
    pub fn new(text: impl Into<String>, style: TextStyle) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Alignment {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

/// Paragraph distances are nonnegative twips (1/1440 inch).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineSpacing {
    Multiple(u16),
    Exact(u32),
    AtLeast(u32),
}
impl Default for LineSpacing {
    fn default() -> Self {
        Self::Multiple(100)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParagraphStyle {
    pub alignment: Alignment,
    pub space_before_twips: u32,
    pub space_after_twips: u32,
    pub line_spacing: LineSpacing,
}
impl ParagraphStyle {
    pub fn validate(&self) -> Result<(), CoreError> {
        match self.line_spacing {
            LineSpacing::Multiple(0) | LineSpacing::Exact(0) | LineSpacing::AtLeast(0) => {
                Err(CoreError::InvalidStyle)
            }
            _ => Ok(()),
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParagraphPatch {
    pub alignment: Option<Alignment>,
    pub space_before_twips: Option<u32>,
    pub space_after_twips: Option<u32>,
    pub line_spacing: Option<LineSpacing>,
}
impl ParagraphPatch {
    pub(crate) fn apply(&self, style: &mut ParagraphStyle) {
        if let Some(v) = self.alignment {
            style.alignment = v;
        }
        if let Some(v) = self.space_before_twips {
            style.space_before_twips = v;
        }
        if let Some(v) = self.space_after_twips {
            style.space_after_twips = v;
        }
        if let Some(v) = self.line_spacing {
            style.line_spacing = v;
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Paragraph {
    pub runs: Vec<Run>,
    pub style: ParagraphStyle,
    /// Typing style for an empty paragraph, independent of empty runs.
    pub default_style: TextStyle,
}
impl Paragraph {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            runs: vec![Run::new(text, TextStyle::default())],
            ..Self::default()
        }
    }
    pub fn text(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }
    pub fn len_bytes(&self) -> usize {
        self.runs.iter().map(|r| r.text.len()).sum()
    }
    pub fn is_boundary(&self, offset: usize) -> bool {
        let text = self.text();
        offset == text.len() || text.grapheme_indices(true).any(|(i, _)| i == offset)
    }
    /// Convert an egui-style Unicode scalar index; rejects a split grapheme.
    pub fn byte_from_char_index(&self, index: usize) -> Result<usize, CoreError> {
        let text = self.text();
        let offset = text
            .char_indices()
            .map(|(i, _)| i)
            .chain(std::iter::once(text.len()))
            .nth(index)
            .ok_or(CoreError::InvalidPosition)?;
        if self.is_boundary(offset) {
            Ok(offset)
        } else {
            Err(CoreError::InvalidPosition)
        }
    }
    pub fn char_index_from_byte(&self, offset: usize) -> Result<usize, CoreError> {
        if !self.is_boundary(offset) {
            return Err(CoreError::InvalidPosition);
        }
        Ok(self.text()[..offset].chars().count())
    }
    pub fn previous_boundary(&self, offset: usize) -> Result<usize, CoreError> {
        if !self.is_boundary(offset) {
            return Err(CoreError::InvalidPosition);
        }
        Ok(self
            .text()
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .take_while(|&i| i < offset)
            .last()
            .unwrap_or(0))
    }
    pub fn next_boundary(&self, offset: usize) -> Result<usize, CoreError> {
        if !self.is_boundary(offset) {
            return Err(CoreError::InvalidPosition);
        }
        let text = self.text();
        Ok(text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .find(|&i| i > offset)
            .unwrap_or(text.len()))
    }
    pub(crate) fn snap_forward(&self, offset: usize) -> usize {
        let text = self.text();
        text.grapheme_indices(true)
            .map(|(i, _)| i)
            .find(|&i| i >= offset)
            .unwrap_or(text.len())
    }
    pub fn normalize(&mut self) {
        let mut runs: Vec<Run> = Vec::new();
        for run in self.runs.drain(..).filter(|r| !r.text.is_empty()) {
            if let Some(last) = runs.last_mut().filter(|r| r.style == run.style) {
                last.text.push_str(&run.text);
            } else {
                runs.push(run);
            }
        }
        self.runs = runs;
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Paragraph(Paragraph),
    PageBreak,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Orientation {
    #[default]
    Portrait,
    Landscape,
}

/// Nominal width and height before orientation, in twips.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageSize {
    pub width_twips: u32,
    pub height_twips: u32,
}
impl PageSize {
    pub const A4: Self = Self {
        width_twips: 11906,
        height_twips: 16838,
    };
    pub const LETTER: Self = Self {
        width_twips: 12240,
        height_twips: 15840,
    };
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Margins {
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
}
impl Default for Margins {
    fn default() -> Self {
        Self {
            top: 1440,
            right: 1440,
            bottom: 1440,
            left: 1440,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageLayout {
    pub size: PageSize,
    pub orientation: Orientation,
    pub margins: Margins,
}
impl Default for PageLayout {
    fn default() -> Self {
        Self {
            size: PageSize::A4,
            orientation: Orientation::Portrait,
            margins: Margins::default(),
        }
    }
}
impl PageLayout {
    pub fn effective_size(&self) -> PageSize {
        match self.orientation {
            Orientation::Portrait => self.size,
            Orientation::Landscape => PageSize {
                width_twips: self.size.height_twips,
                height_twips: self.size.width_twips,
            },
        }
    }
    pub fn validate(&self) -> Result<(), CoreError> {
        let size = self.effective_size();
        if size.width_twips == 0
            || size.height_twips == 0
            || u64::from(self.margins.left) + u64::from(self.margins.right)
                >= u64::from(size.width_twips)
            || u64::from(self.margins.top) + u64::from(self.margins.bottom)
                >= u64::from(size.height_twips)
        {
            return Err(CoreError::InvalidLayout);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    pub blocks: Vec<Block>,
    pub page_layout: PageLayout,
}
impl Default for Document {
    fn default() -> Self {
        Self {
            blocks: vec![Block::Paragraph(Paragraph::default())],
            page_layout: PageLayout::default(),
        }
    }
}
impl Document {
    pub fn paragraph(&self, block: usize) -> Result<&Paragraph, CoreError> {
        match self.blocks.get(block) {
            Some(Block::Paragraph(p)) => Ok(p),
            _ => Err(CoreError::InvalidPosition),
        }
    }
    pub fn validate_position(&self, at: Position) -> Result<(), CoreError> {
        if self.paragraph(at.block)?.is_boundary(at.offset) {
            Ok(())
        } else {
            Err(CoreError::InvalidPosition)
        }
    }
    pub fn validate_selection(&self, selection: Selection) -> Result<(), CoreError> {
        self.validate_position(selection.anchor)?;
        self.validate_position(selection.focus)
    }
    pub fn validate(&self) -> Result<(), CoreError> {
        self.page_layout.validate()?;
        if !matches!(self.blocks.first(), Some(Block::Paragraph(_)))
            || !matches!(self.blocks.last(), Some(Block::Paragraph(_)))
        {
            return Err(CoreError::InvalidStructure);
        }
        for block in &self.blocks {
            if let Block::Paragraph(p) = block {
                p.style.validate()?;
                p.default_style.validate()?;
                for run in &p.runs {
                    run.style.validate()?;
                    if run.text.contains(['\n', '\r', '\u{000c}']) {
                        return Err(CoreError::InvalidText);
                    }
                }
            }
        }
        Ok(())
    }
    pub fn normalize(&mut self) {
        for block in &mut self.blocks {
            if let Block::Paragraph(p) = block {
                p.normalize();
            }
        }
    }
    /// Literal, case-sensitive, non-overlapping matches within paragraphs.
    /// Runs are transparent; paragraph and page breaks are search barriers.
    pub fn find(&self, needle: &str) -> Result<Vec<Selection>, CoreError> {
        if needle.is_empty() {
            return Err(CoreError::EmptySearch);
        }
        let mut matches = Vec::new();
        for (block, content) in self.blocks.iter().enumerate() {
            if let Block::Paragraph(p) = content {
                for (offset, matched) in p.text().match_indices(needle) {
                    let end = offset + matched.len();
                    if p.is_boundary(offset) && p.is_boundary(end) {
                        matches.push(Selection::new(
                            Position::new(block, offset),
                            Position::new(block, end),
                        ));
                    }
                }
            }
        }
        Ok(matches)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WarningCode {
    UnsupportedFeature,
    ApproximatedFormatting,
    MissingPart,
    InvalidValue,
    ResourceLimit,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Feature {
    Tables,
    Images,
    Styles,
    Lists,
    Sections,
    HeadersFooters,
    Fields,
    References,
    Reviewing,
    EmbeddedObjects,
    Other(String),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportWarning {
    pub code: WarningCode,
    pub feature: Feature,
    /// Package part or XML path, when available; never an absolute file path.
    pub location: Option<String>,
    pub message: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportReport {
    pub document: Document,
    pub warnings: Vec<ImportWarning>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreError {
    InvalidPosition,
    InvalidStyle,
    InvalidLayout,
    InvalidStructure,
    InvalidText,
    EmptySearch,
    CannotJoin,
}
impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidPosition => "position must be a paragraph grapheme boundary",
            Self::InvalidStyle => "invalid font or paragraph spacing",
            Self::InvalidLayout => "page dimensions must leave positive content area",
            Self::InvalidStructure => "document must begin and end with a paragraph",
            Self::InvalidText => "run text cannot contain paragraph or page separators",
            Self::EmptySearch => "search text must not be empty",
            Self::CannotJoin => "join requires an immediately following paragraph",
        })
    }
}
impl Error for CoreError {}
