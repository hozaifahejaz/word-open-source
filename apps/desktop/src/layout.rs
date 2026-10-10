//! One composed layout owns glyph painting, pagination and valid caret stops.
use document_core::*;
use egui::{
    Color32, FontFamily, FontId, Pos2, Rect, Stroke, Vec2,
    epaint::Galley,
    text::{LayoutJob, TextFormat},
};
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let mut fallback = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    // Use installed OS glyph coverage without redistributing proprietary fonts.
    #[cfg(target_os = "macos")]
    if let Ok(data) = std::fs::read("/System/Library/Fonts/Supplemental/Arial Unicode.ttf") {
        fonts.font_data.insert(
            "System-Unicode".into(),
            egui::FontData::from_owned(data).into(),
        );
        fallback.insert(0, "System-Unicode".into());
        fonts
            .families
            .get_mut(&FontFamily::Proportional)
            .unwrap()
            .push("System-Unicode".into());
    }
    macro_rules! face {
        ($name:literal,$file:literal) => {{
            fonts.font_data.insert(
                $name.into(),
                egui::FontData::from_static(include_bytes!($file)).into(),
            );
            fonts.families.insert(
                FontFamily::Name($name.into()),
                std::iter::once($name.into())
                    .chain(fallback.iter().cloned())
                    .collect(),
            );
        }};
    }
    face!("Sans-Regular", "../assets/fonts/NotoSans-Regular.ttf");
    face!("Sans-Bold", "../assets/fonts/NotoSans-Bold.ttf");
    face!("Sans-Italic", "../assets/fonts/NotoSans-Italic.ttf");
    face!("Sans-BoldItalic", "../assets/fonts/NotoSans-BoldItalic.ttf");
    face!("Serif-Regular", "../assets/fonts/NotoSerif-Regular.ttf");
    face!("Serif-Bold", "../assets/fonts/NotoSerif-Bold.ttf");
    face!("Serif-Italic", "../assets/fonts/NotoSerif-Italic.ttf");
    face!(
        "Serif-BoldItalic",
        "../assets/fonts/NotoSerif-BoldItalic.ttf"
    );
    ctx.set_fonts(fonts);
}
pub fn is_serif_family(family: &str) -> bool {
    family.eq_ignore_ascii_case("serif") || family.eq_ignore_ascii_case("Noto Serif")
}
fn format(style: &TextStyle, spacing: LineSpacing, zoom: f32) -> TextFormat {
    let base_size = style.size_half_points as f32 * 0.5 * 96.0 / 72.0 * zoom;
    let scale = if style.vertical_align == VerticalAlign::Baseline {
        1.0
    } else {
        0.75
    };
    let size = base_size * scale;
    let family = if is_serif_family(&style.font_family) {
        "Serif"
    } else {
        "Sans"
    };
    let face = match (style.bold, style.italic) {
        (true, true) => "BoldItalic",
        (true, false) => "Bold",
        (false, true) => "Italic",
        _ => "Regular",
    };
    let color = Color32::from_rgb(style.color.red, style.color.green, style.color.blue);
    let natural = base_size * 1.35;
    let line_height = match spacing {
        LineSpacing::Multiple(n) => natural * n as f32 / 100.0,
        LineSpacing::Exact(n) => n as f32 / 15.0 * zoom,
        LineSpacing::AtLeast(n) => natural.max(n as f32 / 15.0 * zoom),
    };
    TextFormat {
        font_id: FontId::new(size, FontFamily::Name(format!("{family}-{face}").into())),
        color,
        underline: if style.underline {
            Stroke::new(zoom, color)
        } else {
            Stroke::NONE
        },
        // egui aligns using row height minus *glyph line height*. Script
        // sections must be shorter as well as using a smaller font.
        line_height: Some(line_height * scale),
        valign: match style.vertical_align {
            VerticalAlign::Baseline => egui::Align::Center,
            VerticalAlign::Superscript => egui::Align::TOP,
            VerticalAlign::Subscript => egui::Align::BOTTOM,
        },
        strikethrough: if style.strikethrough {
            Stroke::new(zoom, color)
        } else {
            Stroke::NONE
        },
        background: style
            .highlight
            .map(|c| Color32::from_rgb(c.red, c.green, c.blue))
            .unwrap_or(Color32::TRANSPARENT),
        ..Default::default()
    }
}
#[derive(Clone, Debug)]
pub struct Stop {
    pub at: Position,
    pub x: f32,
}
pub struct Line {
    pub page: usize,
    pub block: usize,
    pub rect: Rect,
    pub galley: Arc<Galley>,
    pub stops: Vec<Stop>,
    pub start: usize,
    pub end: usize,
}
pub struct DocumentLayout {
    pub pages: Vec<Rect>,
    pub lines: Vec<Line>,
    pub size: Vec2,
}
impl DocumentLayout {
    pub fn build(ctx: &egui::Context, doc: &Document, zoom: f32) -> Self {
        let page = doc.page_layout.effective_size();
        let unit = zoom / 15.0;
        let width = page.width_twips as f32 * unit;
        let height = page.height_twips as f32 * unit;
        let m = doc.page_layout.margins;
        let content_width = (width - (m.left + m.right) as f32 * unit).max(1.0);
        let top = m.top as f32 * unit;
        let bottom = height - m.bottom as f32 * unit;
        let gap = 24.0 * zoom;
        let mut result = Self {
            pages: vec![],
            lines: vec![],
            size: Vec2::ZERO,
        };
        let mut page_index = 0;
        let mut y = top;
        for (block, value) in doc.blocks.iter().enumerate() {
            let Block::Paragraph(p) = value else {
                page_index += 1;
                y = top;
                continue;
            };
            y += (p.style.space_before_twips as f32 * unit).min(bottom - top);
            let mut job = LayoutJob::default();
            job.wrap.max_width = content_width;
            job.justify = p.style.alignment == Alignment::Justify;
            if p.runs.is_empty() {
                job.append(
                    "",
                    0.0,
                    format(&p.default_style, p.style.line_spacing, zoom),
                );
            }
            for run in &p.runs {
                job.append(
                    &run.text,
                    0.0,
                    format(&run.style, p.style.line_spacing, zoom),
                );
            }
            let galley = ctx.fonts(|f| f.layout_job(job));
            let text = p.text();
            let char_bytes: Vec<usize> = text
                .char_indices()
                .map(|(i, _)| i)
                .chain(Some(text.len()))
                .collect();
            let boundaries = ctx.data_mut(|data| {
                type Cache =
                    std::collections::HashMap<String, Arc<std::collections::HashSet<usize>>>;
                let cache = data.get_temp_mut_or_default::<Cache>(egui::Id::new(
                    "paragraph-grapheme-boundaries",
                ));
                if let Some(boundaries) = cache.get(&text) {
                    return boundaries.clone();
                }
                if cache.len() > 128 {
                    cache.clear();
                }
                let boundaries: Arc<std::collections::HashSet<usize>> = Arc::new(
                    text.grapheme_indices(true)
                        .map(|(offset, _)| offset)
                        .chain(std::iter::once(text.len()))
                        .collect(),
                );
                cache.insert(text.clone(), boundaries.clone());
                boundaries
            });
            let mut char_start = 0;
            for (row_index, original_row) in galley.rows.iter().enumerate() {
                let row_count = original_row.char_count_excluding_newline();
                let start = char_bytes[char_start];
                let end = char_bytes[char_start + row_count];
                let minimum = galley
                    .job
                    .sections
                    .iter()
                    .filter(|section| {
                        section.byte_range.start < end && section.byte_range.end > start
                    })
                    .map(|section| {
                        let height = section.format.line_height.unwrap_or(0.0);
                        if section.format.valign == egui::Align::Center {
                            height
                        } else {
                            height / 0.75
                        }
                    })
                    .fold(0.0_f32, f32::max);
                // first_row_min_height only affects the first row in egui. Shape
                // a bounded window (this row and its continuation) when a row
                // has no full-height glyph. Keeping the continuation preserves
                // original wrapping and justification; no artificial glyphs
                // enter the document or its caret map.
                let adjusted;
                let placed = if minimum > original_row.height() + 0.5 {
                    let next_count = galley
                        .rows
                        .get(row_index + 1)
                        .map(|row| row.char_count_including_newline())
                        .unwrap_or(0);
                    let window_end = char_bytes[(char_start
                        + original_row.char_count_including_newline()
                        + next_count)
                        .min(char_bytes.len() - 1)];
                    let mut row_job = LayoutJob::default();
                    row_job.wrap.max_width = content_width;
                    row_job.justify = p.style.alignment == Alignment::Justify;
                    row_job.first_row_min_height = minimum;
                    for section in &galley.job.sections {
                        let a = start.max(section.byte_range.start);
                        let b = window_end.min(section.byte_range.end);
                        if a < b {
                            row_job.append(&text[a..b], 0.0, section.format.clone());
                        }
                    }
                    adjusted = ctx.fonts(|f| f.layout_job(row_job));
                    &adjusted.rows[0]
                } else {
                    original_row
                };
                let row_height = placed.height().max(1.0);
                if y + row_height > bottom && y > top {
                    page_index += 1;
                    y = top;
                }
                let x = m.left as f32 * unit
                    + match p.style.alignment {
                        Alignment::Center => (content_width - placed.size.x) * 0.5,
                        Alignment::Right => content_width - placed.size.x,
                        _ => 0.0,
                    };
                let line_y = page_index as f32 * (height + gap) + y;
                let rect = Rect::from_min_size(
                    Pos2::new(x, line_y),
                    Vec2::new(placed.size.x.max(1.0), row_height),
                );
                let mut stops = Vec::new();
                for column in 0..=row_count {
                    let offset = char_bytes[char_start + column];
                    if boundaries.contains(&offset) {
                        stops.push(Stop {
                            at: Position::new(block, offset),
                            x: x + placed.x_offset(column),
                        });
                    }
                }
                // Isolate the already-shaped row, preserving glyph geometry and atlas mesh.
                let mut row = placed.clone();
                row.pos = Pos2::ZERO;
                let mut single = (*galley).clone();
                single.rows = vec![row];
                single.rect = Rect::from_min_size(Pos2::ZERO, placed.size);
                single.mesh_bounds = placed.visuals.mesh_bounds;
                single.num_vertices = placed.visuals.mesh.vertices.len();
                single.num_indices = placed.visuals.mesh.indices.len();
                result.lines.push(Line {
                    page: page_index,
                    block,
                    rect,
                    galley: Arc::new(single),
                    stops,
                    start: char_bytes[char_start],
                    end: char_bytes[char_start + row_count],
                });
                char_start += original_row.char_count_including_newline();
                y += row_height;
            }
            y += (p.style.space_after_twips as f32 * unit).min(bottom - top);
        }
        for i in 0..=page_index {
            result.pages.push(Rect::from_min_size(
                Pos2::new(0.0, i as f32 * (height + gap)),
                Vec2::new(width, height),
            ));
        }
        result.size = Vec2::new(width, (page_index + 1) as f32 * (height + gap) - gap);
        result
    }
    pub fn line_at(&self, at: Position) -> Option<usize> {
        // At wrap boundaries prefer the following row (also the following page).
        self.lines
            .iter()
            .rposition(|l| l.stops.iter().any(|s| s.at == at))
    }
    pub fn visual_line(&self, at: Position, hint: Option<usize>) -> Option<usize> {
        hint.filter(|&i| {
            self.lines
                .get(i)
                .is_some_and(|l| l.stops.iter().any(|s| s.at == at))
        })
        .or_else(|| self.line_at(at))
    }
    #[cfg(test)]
    pub fn caret(&self, at: Position) -> Option<Rect> {
        self.caret_with_hint(at, None)
    }
    pub fn caret_with_hint(&self, at: Position, hint: Option<usize>) -> Option<Rect> {
        let l = &self.lines[self.visual_line(at, hint)?];
        let x = l.stops.iter().find(|s| s.at == at)?.x;
        Some(Rect::from_min_size(
            Pos2::new(x, l.rect.top()),
            Vec2::new(1.5, l.rect.height()),
        ))
    }
    fn nearest(line: &Line, x: f32) -> Option<Position> {
        line.stops
            .iter()
            .min_by(|a, b| (a.x - x).abs().total_cmp(&(b.x - x).abs()))
            .map(|s| s.at)
    }
    #[cfg(test)]
    pub fn hit(&self, point: Pos2) -> Option<Position> {
        self.hit_line(point).map(|(at, _)| at)
    }
    pub fn hit_line(&self, point: Pos2) -> Option<(Position, usize)> {
        self.lines
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.stops.is_empty())
            .min_by(|(_, a), (_, b)| {
                let distance = |l: &Line| {
                    if point.y < l.rect.top() {
                        l.rect.top() - point.y
                    } else if point.y > l.rect.bottom() {
                        point.y - l.rect.bottom()
                    } else {
                        0.0
                    }
                };
                distance(a).total_cmp(&distance(b))
            })
            .and_then(|(i, l)| Self::nearest(l, point.x).map(|at| (at, i)))
    }
    pub fn vertical_with_hint(
        &self,
        at: Position,
        delta: isize,
        x: f32,
        hint: Option<usize>,
    ) -> (Position, Option<usize>) {
        let Some(index) = self.visual_line(at, hint) else {
            return (at, None);
        };
        let target = (index as isize + delta).clamp(0, self.lines.len() as isize - 1) as usize;
        (
            Self::nearest(&self.lines[target], x).unwrap_or(at),
            Some(target),
        )
    }
    #[cfg(test)]
    pub fn vertical(&self, at: Position, delta: isize, x: f32) -> Position {
        let Some(index) = self.line_at(at) else {
            return at;
        };
        let target = (index as isize + delta).clamp(0, self.lines.len() as isize - 1) as usize;
        Self::nearest(&self.lines[target], x).unwrap_or(at)
    }
    pub fn edge_with_hint(&self, at: Position, end: bool, hint: Option<usize>) -> Position {
        self.visual_line(at, hint)
            .and_then(|i| {
                if end {
                    self.lines[i].stops.last()
                } else {
                    self.lines[i].stops.first()
                }
            })
            .map(|s| s.at)
            .unwrap_or(at)
    }
    pub fn selection_rects(&self, selection: Selection) -> Vec<Rect> {
        let (start, end) = selection.ordered();
        if start == end {
            return vec![];
        }
        self.lines
            .iter()
            .filter_map(|l| {
                let left = Position::new(l.block, l.start);
                let right = Position::new(l.block, l.end);
                if end < left || start > right || end == left {
                    return None;
                }
                let x1 = if start <= left {
                    l.rect.left()
                } else {
                    l.stops.iter().find(|s| s.at == start)?.x
                };
                let x2 = if end > right {
                    l.rect.right() + 5.0
                } else {
                    l.stops.iter().find(|s| s.at == end)?.x
                };
                (x2 > x1).then(|| {
                    Rect::from_min_max(Pos2::new(x1, l.rect.top()), Pos2::new(x2, l.rect.bottom()))
                })
            })
            .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rich_scripts_have_real_vertical_offsets_and_grapheme_safe_hits() {
        for zoom in [0.75, 1.0, 1.5] {
            let doc = Document {
                blocks: vec![Block::Paragraph(Paragraph {
                    runs: vec![
                        Run::new("x", TextStyle::default()),
                        Run::new(
                            "e\u{301}",
                            TextStyle {
                                vertical_align: VerticalAlign::Superscript,
                                strikethrough: true,
                                highlight: Some(Color::rgb(240, 230, 120)),
                                ..Default::default()
                            },
                        ),
                        Run::new(
                            "e",
                            TextStyle {
                                vertical_align: VerticalAlign::Subscript,
                                ..Default::default()
                            },
                        ),
                    ],
                    ..Default::default()
                })],
                ..Default::default()
            };
            with_layout(&doc, zoom, |layout| {
                let line = &layout.lines[0];
                let sections = &line.galley.job.sections;
                assert!(
                    (sections[1].format.font_id.size / sections[0].format.font_id.size - 0.75)
                        .abs()
                        < 0.001
                );
                assert_eq!(
                    sections[1].format.background,
                    Color32::from_rgb(240, 230, 120)
                );
                assert!(sections[1].format.strikethrough.width > 0.0);
                let glyphs = &line.galley.rows[0].glyphs;
                // Compare the same glyph/font: subscript must genuinely be lower.
                assert!(glyphs[3].pos.y > glyphs[1].pos.y + zoom * 2.0, "{glyphs:?}");
                assert!(glyphs[1].pos.y < glyphs[0].pos.y);
                assert_eq!(
                    line.stops.iter().map(|s| s.at.offset).collect::<Vec<_>>(),
                    vec![0, 1, 4, 5]
                );
                for stop in &line.stops {
                    let caret = layout.caret(stop.at).unwrap();
                    assert_eq!(
                        layout.hit(Pos2::new(stop.x, caret.center().y)),
                        Some(stop.at)
                    );
                }
                let rects = layout
                    .selection_rects(Selection::new(Position::new(0, 1), Position::new(0, 4)));
                assert_eq!(rects.len(), 1);
                assert!(rects[0].width() > 0.0);
            });
            let script = Document {
                blocks: vec![Block::Paragraph(Paragraph {
                    runs: vec![Run::new(
                        "e".repeat(300),
                        TextStyle {
                            vertical_align: VerticalAlign::Subscript,
                            ..Default::default()
                        },
                    )],
                    ..Default::default()
                })],
                ..Default::default()
            };
            with_layout(&script, zoom, |layout| {
                for line in &layout.lines {
                    let glyph = &line.galley.rows[0].glyphs[0];
                    assert!(line.rect.height() > glyph.line_height + zoom * 2.0);
                    assert!(glyph.pos.y > glyph.font_impl_ascent + zoom * 2.0);
                }
            });
        }
    }
    #[test]
    fn rich_script_wrapping_preserves_justification_and_background_meshes() {
        let doc = Document {
            blocks: vec![Block::Paragraph(Paragraph {
                runs: vec![Run::new(
                    "é words 👩‍💻 spaced ".repeat(80),
                    TextStyle {
                        vertical_align: VerticalAlign::Subscript,
                        highlight: Some(Color::rgb(240, 230, 120)),
                        strikethrough: true,
                        ..Default::default()
                    },
                )],
                style: ParagraphStyle {
                    alignment: Alignment::Justify,
                    ..Default::default()
                },
                ..Default::default()
            })],
            ..Default::default()
        };
        with_layout(&doc, 1.0, |layout| {
            assert!(layout.lines.len() > 2);
            let mut end = 0;
            for (index, line) in layout.lines.iter().enumerate() {
                assert_eq!(line.start, end);
                end = line.end;
                let text = doc.paragraph(0).unwrap().text();
                assert_eq!(
                    line.galley.rows[0]
                        .glyphs
                        .iter()
                        .map(|g| g.chr)
                        .collect::<String>(),
                    text[line.start..line.end]
                );
                let content = layout.pages[0].width()
                    - (doc.page_layout.margins.left + doc.page_layout.margins.right) as f32 / 15.0;
                if index + 1 < layout.lines.len() {
                    assert!((line.rect.width() - content).abs() < 1.0);
                }
                assert!(
                    line.galley.rows[0]
                        .visuals
                        .mesh
                        .vertices
                        .iter()
                        .any(|v| v.color == Color32::from_rgb(240, 230, 120))
                );
                for stop in &line.stops {
                    let (hit, hint) = layout
                        .hit_line(Pos2::new(stop.x, line.rect.center().y))
                        .unwrap();
                    // Justification can collapse trailing whitespace stops at
                    // the same x. The returned caret must occupy that position.
                    let hit_stop = line.stops.iter().find(|s| s.at == hit).unwrap();
                    assert!((hit_stop.x - stop.x).abs() < 0.01);
                    assert!(
                        layout.caret_with_hint(hit, Some(hint)).unwrap().center().y
                            == line.rect.center().y
                    );
                }
            }
            assert_eq!(end, doc.paragraph(0).unwrap().len_bytes());
        });
    }
    #[test]
    fn caret_stops_do_not_split_graphemes_across_styled_runs() {
        let doc = Document {
            blocks: vec![Block::Paragraph(Paragraph {
                runs: vec![
                    Run::new("e", TextStyle::default()),
                    Run::new(
                        "\u{301}👩‍💻é",
                        TextStyle {
                            bold: true,
                            ..Default::default()
                        },
                    ),
                ],
                ..Default::default()
            })],
            ..Default::default()
        };
        with_layout(&doc, 1.0, |l| {
            let offsets: Vec<_> = l
                .lines
                .iter()
                .flat_map(|line| line.stops.iter().map(|s| s.at.offset))
                .collect();
            assert_eq!(offsets, vec![0, 3, 14, 16]);
        });
    }

    #[test]
    #[ignore = "manual cold-layout benchmark; run with --ignored --nocapture"]
    fn unicode_cold_layout_benchmark() {
        for n in [1000, 2000, 4000] {
            let doc = Document {
                blocks: vec![Block::Paragraph(Paragraph::plain("é".repeat(n)))],
                ..Default::default()
            };
            let ctx = egui::Context::default();
            install_fonts(&ctx);
            ctx.begin_pass(Default::default());
            let start = std::time::Instant::now();
            let layout = DocumentLayout::build(&ctx, &doc, 1.0);
            println!("Unicode cold layout {n} chars: {:?}", start.elapsed());
            assert_eq!(
                layout.lines.last().unwrap().stops.last().unwrap().at.offset,
                n * 2
            );
            let _ = ctx.end_pass();
        }
    }
    fn with_layout(doc: &Document, zoom: f32, check: impl FnOnce(DocumentLayout)) {
        let ctx = egui::Context::default();
        install_fonts(&ctx);
        ctx.begin_pass(Default::default());
        let layout = DocumentLayout::build(&ctx, doc, zoom);
        check(layout);
        let _ = ctx.end_pass();
    }
    #[test]
    fn pagination_uses_the_editable_rows_and_roundtrips_hits() {
        let mut e = Editor::default();
        e.execute(Command::InsertText {
            at: Position::default(),
            text: "alpha βeta 👩‍👩‍👦 e\u{301} ".repeat(200),
            style: None,
        })
        .unwrap();
        with_layout(e.document(), 1.0, |l| {
            assert!(l.pages.len() > 1);
            for line in &l.lines {
                assert!(l.pages[line.page].contains(line.rect.center()));
                for stop in &line.stops {
                    assert!(e.document().validate_position(stop.at).is_ok());
                    let hit = l.hit(Pos2::new(stop.x, line.rect.center().y)).unwrap();
                    assert_eq!(hit, stop.at);
                    let (hit, index) = l.hit_line(Pos2::new(stop.x, line.rect.center().y)).unwrap();
                    assert_eq!(
                        l.caret_with_hint(hit, Some(index)).unwrap().center().y,
                        line.rect.center().y
                    );
                }
            }
            let at = l.lines.iter().find(|r| r.page == 1).unwrap().stops[0].at;
            assert!(l.caret(at).unwrap().top() >= l.pages[1].top());
            assert!(
                !l.selection_rects(select_everything(e.document()))
                    .is_empty()
            );
        });
    }
    fn select_everything(d: &Document) -> Selection {
        Selection::new(
            Position::default(),
            Position::new(0, d.paragraph(0).unwrap().len_bytes()),
        )
    }
    #[test]
    fn explicit_break_zoom_and_formatted_edit_undo() {
        let mut e = Editor::default();
        e.execute(Command::InsertText {
            at: Position::default(),
            text: "first".into(),
            style: Some(TextStyle {
                bold: true,
                ..Default::default()
            }),
        })
        .unwrap();
        e.execute(Command::InsertPageBreak {
            at: e.selection().focus,
        })
        .unwrap();
        e.execute(Command::InsertText {
            at: e.selection().focus,
            text: "second".into(),
            style: None,
        })
        .unwrap();
        with_layout(e.document(), 1.0, |a| {
            with_layout(e.document(), 1.5, |b| {
                assert_eq!(a.pages.len(), 2);
                assert_eq!(b.pages.len(), 2);
                assert!((b.size.x / a.size.x - 1.5).abs() < 0.01);
                assert_eq!(a.vertical(Position::new(0, 5), 1, 0.0), Position::new(2, 0));
            })
        });
        e.execute(Command::Undo).unwrap();
        assert!(e.document().paragraph(2).unwrap().text().is_empty());
        assert!(e.document().paragraph(0).unwrap().runs[0].style.bold);
    }
}
