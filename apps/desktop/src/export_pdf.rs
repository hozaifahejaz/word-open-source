//! Native vector PDF export. Layout is shared with the editor; font programs and
//! Unicode maps are embedded, so text remains searchable without installed fonts.
use crate::layout::{self, DocumentLayout};
use document_core::{Block, Document, TextStyle, VerticalAlign};
use pdf_writer::{Filter, Finish, Name, Pdf, Rect, Ref, Str};
use std::{collections::BTreeMap, fmt::Write as _, io::Write as _};
use ttf_parser::{Face, GlyphId, Permissions};

const PX_TO_PT: f32 = 0.75;
const FONT_DATA: [&[u8]; 8] = [
    include_bytes!("../assets/fonts/NotoSans-Regular.ttf"),
    include_bytes!("../assets/fonts/NotoSans-Bold.ttf"),
    include_bytes!("../assets/fonts/NotoSans-Italic.ttf"),
    include_bytes!("../assets/fonts/NotoSans-BoldItalic.ttf"),
    include_bytes!("../assets/fonts/NotoSerif-Regular.ttf"),
    include_bytes!("../assets/fonts/NotoSerif-Bold.ttf"),
    include_bytes!("../assets/fonts/NotoSerif-Italic.ttf"),
    include_bytes!("../assets/fonts/NotoSerif-BoldItalic.ttf"),
];
struct FontUse {
    chars: BTreeMap<char, (u16, GlyphId)>,
    id: Ref,
}
fn font_index(style: &TextStyle) -> usize {
    usize::from(layout::is_serif_family(&style.font_family)) * 4
        + usize::from(style.bold)
        + usize::from(style.italic) * 2
}
fn invisible(ch: char) -> bool {
    matches!(ch, '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}'
        | '\u{2060}'..='\u{206f}' | '\u{feff}' | '\u{fe00}'..='\u{fe0f}'
        | '\u{e0100}'..='\u{e01ef}')
}
fn utf16_hex(text: &str, bom: bool) -> String {
    let mut result = if bom { "FEFF".into() } else { String::new() };
    for unit in text.encode_utf16() {
        write!(result, "{unit:04X}").unwrap();
    }
    result
}
fn compressed(data: &[u8]) -> Result<Vec<u8>, String> {
    let mut compressor =
        flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    compressor.write_all(data).map_err(|e| e.to_string())?;
    compressor.finish().map_err(|e| e.to_string())
}
fn stream(pdf: &mut Pdf, id: Ref, data: &[u8]) -> Result<(), String> {
    pdf.stream(id, &compressed(data)?)
        .filter(Filter::FlateDecode);
    Ok(())
}
fn allocate(next: &mut i32) -> Ref {
    let id = Ref::new(*next);
    *next += 1;
    id
}
fn embed_font(
    pdf: &mut Pdf,
    next: &mut i32,
    data: &[u8],
    face: &Face<'_>,
    used: &FontUse,
) -> Result<(), String> {
    let descendant = allocate(next);
    let descriptor = allocate(next);
    let program = allocate(next);
    let unicode = allocate(next);
    let cid_map = allocate(next);
    let name = face
        .names()
        .into_iter()
        .filter(|name| name.name_id == ttf_parser::name_id::POST_SCRIPT_NAME)
        .find_map(|name| name.to_string())
        .unwrap_or_else(|| format!("FolioFont{}", used.id.get()));
    let name = Name(name.as_bytes());
    {
        let mut font = pdf.indirect(used.id).dict();
        font.pair(Name(b"Type"), Name(b"Font"));
        font.pair(Name(b"Subtype"), Name(b"Type0"));
        font.pair(Name(b"BaseFont"), name);
        font.pair(Name(b"Encoding"), Name(b"Identity-H"));
        font.insert(Name(b"DescendantFonts"))
            .array()
            .item(descendant);
        font.pair(Name(b"ToUnicode"), unicode);
    }
    let em = f32::from(face.units_per_em());
    let metric = |v: i16| f32::from(v) * 1000.0 / em;
    {
        let mut font = pdf.indirect(descendant).dict();
        font.pair(Name(b"Type"), Name(b"Font"));
        font.pair(Name(b"Subtype"), Name(b"CIDFontType2"));
        font.pair(Name(b"BaseFont"), name);
        {
            let mut system = font.insert(Name(b"CIDSystemInfo")).dict();
            system.pair(Name(b"Registry"), Str(b"Adobe"));
            system.pair(Name(b"Ordering"), Str(b"Identity"));
            system.pair(Name(b"Supplement"), 0);
        }
        font.pair(Name(b"FontDescriptor"), descriptor);
        font.pair(Name(b"CIDToGIDMap"), cid_map);
        let mut widths = font.insert(Name(b"W")).array();
        for &(cid, gid) in used.chars.values() {
            widths.item(i32::from(cid));
            widths
                .push()
                .array()
                .item(f32::from(face.glyph_hor_advance(gid).unwrap_or(0)) * 1000.0 / em);
        }
    }
    {
        let mut desc = pdf.indirect(descriptor).dict();
        desc.pair(Name(b"Type"), Name(b"FontDescriptor"));
        desc.pair(Name(b"FontName"), name);
        // Symbolic flag is required for Identity-H CID fonts.
        desc.pair(Name(b"Flags"), 4 | if face.is_italic() { 64 } else { 0 });
        let bbox = face.global_bounding_box();
        desc.insert(Name(b"FontBBox")).array().items([
            metric(bbox.x_min),
            metric(bbox.y_min),
            metric(bbox.x_max),
            metric(bbox.y_max),
        ]);
        desc.pair(Name(b"ItalicAngle"), face.italic_angle());
        desc.pair(Name(b"Ascent"), metric(face.ascender()));
        desc.pair(Name(b"Descent"), metric(face.descender()));
        desc.pair(
            Name(b"CapHeight"),
            metric(face.capital_height().unwrap_or(face.ascender())),
        );
        desc.pair(Name(b"StemV"), if face.is_bold() { 120 } else { 80 });
        desc.pair(Name(b"FontFile2"), program);
    }
    pdf.stream(program, &compressed(data)?)
        .filter(Filter::FlateDecode)
        .pair(Name(b"Length1"), data.len() as i32);
    let mut map = vec![0u8; (used.chars.len() + 1) * 2];
    let mut cmap = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /FolioUnicode def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    let entries: Vec<_> = used.chars.iter().collect();
    for group in entries.chunks(100) {
        writeln!(cmap, "{} beginbfchar", group.len()).unwrap();
        for &(ch, &(cid, gid)) in group {
            map[usize::from(cid) * 2..usize::from(cid) * 2 + 2]
                .copy_from_slice(&gid.0.to_be_bytes());
            writeln!(cmap, "<{cid:04X}> <{}>", utf16_hex(&ch.to_string(), false)).unwrap();
        }
        cmap.push_str("endbfchar\n");
    }
    cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    stream(pdf, unicode, cmap.as_bytes())?;
    stream(pdf, cid_map, &map)
}

/// Encode an already validated document as selectable vector PDF, without writing files.
/// Unsupported visible glyphs and fonts that forbid embedding cause an error.
pub fn encode(doc: &Document) -> Result<Vec<u8>, String> {
    doc.validate()
        .map_err(|e| format!("Cannot export PDF: {e}"))?;
    let ctx = egui::Context::default();
    layout::install_fonts(&ctx);
    ctx.begin_pass(egui::RawInput::default());
    let composed = DocumentLayout::build(&ctx, doc, 1.0);
    let _ = ctx.end_pass();
    let mut data = FONT_DATA
        .iter()
        .map(|bytes| bytes.to_vec())
        .collect::<Vec<_>>();
    // Match the editor's first fallback. The OS font is never added to the app
    // bundle; embedding is permitted only by the font's own OS/2 permissions.
    #[cfg(target_os = "macos")]
    if let Ok(font) = std::fs::read("/System/Library/Fonts/Supplemental/Arial Unicode.ttf") {
        data.push(font);
    }
    let faces = data
        .iter()
        .map(|bytes| Face::parse(bytes, 0).map_err(|e| format!("Cannot parse PDF font: {e:?}")))
        .collect::<Result<Vec<_>, _>>()?;
    let mut next = 3;
    let mut fonts = faces
        .iter()
        .map(|_| FontUse {
            chars: BTreeMap::new(),
            id: allocate(&mut next),
        })
        .collect::<Vec<_>>();
    let mut contents = vec![String::new(); composed.pages.len()];
    for line in &composed.lines {
        let Block::Paragraph(paragraph) = &doc.blocks[line.block] else {
            continue;
        };
        let text = paragraph.text();
        let content = &mut contents[line.page];
        let row = &line.galley.rows[0];
        let page = composed.pages[line.page];
        // ActualText preserves combining marks, zero-width controls and exact
        // source scalars in selections, independently of reader glyph heuristics.
        writeln!(
            content,
            "/Span << /ActualText <{}> >> BDC",
            utf16_hex(&text[line.start..line.end], true)
        )
        .unwrap();
        let mut offset = line.start;
        for glyph in &row.glyphs {
            let style = paragraph
                .runs
                .iter()
                .scan(0usize, |end, run| {
                    let start = *end;
                    *end += run.text.len();
                    Some((start..*end, &run.style))
                })
                .find(|(range, _)| range.contains(&offset))
                .map(|(_, style)| style)
                .ok_or("PDF layout did not map to a text run")?;
            offset += glyph.chr.len_utf8();
            let requested = font_index(style);
            let visible = !invisible(glyph.chr);
            let lookup = if glyph.chr == '\t' || !visible {
                ' '
            } else {
                glyph.chr
            };
            let selected = std::iter::once(requested)
                .chain(8..faces.len())
                .find_map(|i| faces[i].glyph_index(lookup).map(|gid| (i, gid)))
                .ok_or_else(|| {
                    format!(
                        "Cannot export PDF: no embeddable font covers U+{:04X} ({})",
                        glyph.chr as u32, glyph.chr
                    )
                })?;
            let (font_index, gid) = selected;
            let face = &faces[font_index];
            if !matches!(
                face.permissions(),
                Some(
                    Permissions::Installable | Permissions::PreviewAndPrint | Permissions::Editable
                )
            ) || !face.is_outline_embedding_allowed()
            {
                return Err(format!(
                    "Cannot export PDF: font embedding is restricted for U+{:04X}",
                    glyph.chr as u32
                ));
            }
            let font = &mut fonts[font_index];
            let cid = if let Some(&(cid, _)) = font.chars.get(&glyph.chr) {
                cid
            } else {
                let cid = u16::try_from(font.chars.len() + 1)
                    .map_err(|_| "Too many distinct PDF characters")?;
                font.chars.insert(glyph.chr, (cid, gid));
                cid
            };
            let x = (line.rect.left() + glyph.pos.x) * PX_TO_PT;
            let baseline =
                page.height() * PX_TO_PT - (line.rect.top() - page.top() + glyph.pos.y) * PX_TO_PT;
            let width = glyph.advance_width * PX_TO_PT;
            let color = style.color;
            let rgb = |c: document_core::Color| {
                format!(
                    "{:.5} {:.5} {:.5}",
                    c.red as f32 / 255.0,
                    c.green as f32 / 255.0,
                    c.blue as f32 / 255.0
                )
            };
            if let Some(highlight) = style.highlight {
                let top = baseline + glyph.font_ascent * PX_TO_PT;
                writeln!(
                    content,
                    "{} rg {x:.5} {:.5} {width:.5} {:.5} re f",
                    rgb(highlight),
                    top - glyph.line_height * PX_TO_PT,
                    glyph.line_height * PX_TO_PT
                )
                .unwrap();
            }
            // egui first converts em size to ab_glyph's ascender-to-descender
            // scale, then rounds that scale to physical pixels. Undo the latter
            // conversion for PDF's em-based Tf without losing the rounding.
            let script = if style.vertical_align == VerticalAlign::Baseline {
                1.0
            } else {
                0.75
            };
            let font_height = f32::from(face.ascender()) - f32::from(face.descender());
            let pixels =
                (f32::from(style.size_half_points) * 0.5 / PX_TO_PT * script * font_height
                    / f32::from(face.units_per_em()))
                .round();
            let size = pixels * f32::from(face.units_per_em()) / font_height * PX_TO_PT;
            writeln!(content, "{} rg BT /F{} {size:.5} Tf {} Tr 1 0 0 1 {x:.5} {baseline:.5} Tm <{cid:04X}> Tj ET", rgb(color), font_index, if visible { 0 } else { 3 }).unwrap();
            if style.underline || style.strikethrough {
                writeln!(content, "{} RG 0.75 w", rgb(color)).unwrap();
                for y in [
                    style
                        .underline
                        .then_some(baseline + (glyph.font_ascent - glyph.line_height) * PX_TO_PT),
                    style.strikethrough.then_some(
                        baseline + (glyph.font_ascent - glyph.line_height * 0.5) * PX_TO_PT,
                    ),
                ]
                .into_iter()
                .flatten()
                {
                    writeln!(content, "{x:.5} {y:.5} m {:.5} {y:.5} l S", x + width).unwrap();
                }
            }
        }
        content.push_str("EMC\n");
    }
    let mut pdf = Pdf::new();
    pdf.catalog(Ref::new(1)).pages(Ref::new(2));
    let mut page_ids = Vec::new();
    for (page_rect, content) in composed.pages.iter().zip(contents) {
        let page_id = allocate(&mut next);
        let content_id = allocate(&mut next);
        page_ids.push(page_id);
        let mut page = pdf.page(page_id);
        page.parent(Ref::new(2))
            .media_box(Rect::new(
                0.0,
                0.0,
                page_rect.width() * PX_TO_PT,
                page_rect.height() * PX_TO_PT,
            ))
            .contents(content_id);
        {
            let mut resources = page.resources();
            let mut resource_fonts = resources.fonts();
            for (i, font) in fonts
                .iter()
                .enumerate()
                .filter(|(_, font)| !font.chars.is_empty())
            {
                let name = format!("F{i}");
                resource_fonts.pair(Name(name.as_bytes()), font.id);
            }
        }
        page.finish();
        stream(&mut pdf, content_id, content.as_bytes())?;
    }
    pdf.pages(Ref::new(2))
        .kids(page_ids.iter().copied())
        .count(page_ids.len() as i32);
    for ((bytes, face), used) in data
        .iter()
        .zip(&faces)
        .zip(&fonts)
        .filter(|(_, used)| !used.chars.is_empty())
    {
        embed_font(&mut pdf, &mut next, bytes, face, used)?;
    }
    Ok(pdf.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use document_core::{Alignment, Color, LineSpacing, Orientation, Paragraph, Run};

    fn decoded_streams(bytes: &[u8]) -> String {
        use std::io::Read;
        let mut output = String::new();
        let mut remaining = bytes;
        while let Some(start) = remaining.windows(7).position(|part| part == b"stream\n") {
            remaining = &remaining[start + 7..];
            let Some(end) = remaining
                .windows(10)
                .position(|part| part == b"\nendstream")
            else {
                break;
            };
            let mut stream = Vec::new();
            flate2::read::ZlibDecoder::new(&remaining[..end])
                .read_to_end(&mut stream)
                .unwrap();
            if let Ok(text) = String::from_utf8(stream) {
                output.push_str(&text);
            }
            remaining = &remaining[end + 10..];
        }
        output
    }
    fn rich_fixture() -> Document {
        let mut doc = Document::default();
        doc.page_layout.orientation = Orientation::Landscape;
        let mut heading =
            Paragraph::plain("Native PDF: café Ελληνικά e\u{301} a\u{200d}b z\u{e0100}");
        heading.runs[0].style.size_half_points = 40;
        heading.runs[0].style.bold = true;
        heading.style.alignment = Alignment::Center;
        heading.style.space_after_twips = 240;
        doc.blocks = vec![Block::Paragraph(heading)];
        let mut samples = Paragraph::default();
        for serif in [false, true] {
            for (bold, italic) in [(false, false), (true, false), (false, true), (true, true)] {
                let style = TextStyle {
                    font_family: if serif { "Noto Serif" } else { "Noto Sans" }.into(),
                    bold,
                    italic,
                    ..Default::default()
                };
                samples.runs.push(Run::new(
                    format!(
                        "{} {}{} café | ",
                        if serif { "Serif" } else { "Sans" },
                        if bold { "bold" } else { "regular" },
                        if italic { " italic" } else { "" }
                    ),
                    style,
                ));
            }
        }
        samples.style.line_spacing = LineSpacing::Multiple(150);
        doc.blocks.push(Block::Paragraph(samples));
        let p = Paragraph {
            runs: vec![
                Run::new(
                    "Underline and strike ",
                    TextStyle {
                        underline: true,
                        strikethrough: true,
                        color: Color::rgb(170, 20, 40),
                        ..Default::default()
                    },
                ),
                Run::new(
                    "Highlight ",
                    TextStyle {
                        highlight: Some(Color::rgb(255, 220, 50)),
                        ..Default::default()
                    },
                ),
                Run::new("x", TextStyle::default()),
                Run::new(
                    "2",
                    TextStyle {
                        vertical_align: VerticalAlign::Superscript,
                        ..Default::default()
                    },
                ),
                Run::new(" H", TextStyle::default()),
                Run::new(
                    "2",
                    TextStyle {
                        vertical_align: VerticalAlign::Subscript,
                        ..Default::default()
                    },
                ),
                Run::new("O", TextStyle::default()),
            ],
            ..Default::default()
        };
        doc.blocks.push(Block::Paragraph(p));
        // Noto's Latin extended characters include supplementary-plane scalars.
        let face = Face::parse(FONT_DATA[0], 0).unwrap();
        if let Some(ch) = (0x10000..0x20000)
            .filter_map(char::from_u32)
            .find(|&ch| face.glyph_index(ch).is_some())
        {
            doc.blocks.push(Block::Paragraph(Paragraph::plain(format!(
                "Supplementary scalar: {ch}"
            ))));
        }
        #[cfg(target_os = "macos")]
        doc.blocks.push(Block::Paragraph(Paragraph::plain(
            "System fallback: 中文 اردو",
        )));
        doc.blocks.push(Block::PageBreak);
        doc.blocks.push(Block::Paragraph(Paragraph::plain(
            "Explicit page break, right aligned.",
        )));
        if let Some(Block::Paragraph(p)) = doc.blocks.last_mut() {
            p.style.alignment = Alignment::Right;
        }
        for index in 0..40 {
            let mut p = Paragraph::plain(format!("Pagination line {index:02}: The native exporter shares the editor's widths, margins, wrapping, spacing and glyph positions. ").repeat(3));
            p.style.alignment = Alignment::Justify;
            p.style.space_before_twips = 30;
            p.style.space_after_twips = 60;
            p.style.line_spacing = LineSpacing::AtLeast(300);
            doc.blocks.push(Block::Paragraph(p));
        }
        doc
    }
    fn media_box(bytes: &[u8]) -> Vec<f32> {
        let raw = String::from_utf8_lossy(bytes);
        raw.split("/MediaBox [")
            .nth(1)
            .unwrap()
            .split(']')
            .next()
            .unwrap()
            .split_whitespace()
            .map(|number| number.parse().unwrap())
            .collect()
    }
    #[test]
    fn blank_document_is_one_page_and_invalid_documents_fail() {
        let blank = encode(&Document::default()).unwrap();
        assert!(blank.starts_with(b"%PDF-1.7"));
        let raw = String::from_utf8_lossy(&blank);
        assert!(raw.contains("/Count 1"));
        let bounds = media_box(&blank);
        assert!((bounds[2] - 595.3).abs() < 0.01);
        assert!((bounds[3] - 841.9).abs() < 0.01);
        let mut invalid = Document::default();
        invalid.blocks.clear();
        assert!(encode(&invalid).unwrap_err().contains("begin and end"));
        invalid = Document::default();
        invalid.page_layout.margins.left = u32::MAX;
        assert!(encode(&invalid).is_err());
    }
    #[test]
    fn explicit_blank_page_and_orientation_match_shared_layout() {
        let mut doc = Document::default();
        doc.page_layout.orientation = Orientation::Landscape;
        doc.blocks.extend([
            Block::PageBreak,
            Block::PageBreak,
            Block::Paragraph(Paragraph::plain("Last page")),
        ]);
        let bytes = encode(&doc).unwrap();
        let raw = String::from_utf8_lossy(&bytes);
        assert!(raw.contains("/Count 3"));
        let bounds = media_box(&bytes);
        assert!((bounds[2] - 841.9).abs() < 0.01);
        assert!((bounds[3] - 595.3).abs() < 0.01);
        assert!(decoded_streams(&bytes).contains(&utf16_hex("Last page", true)));
    }
    #[test]
    fn styled_unicode_preserves_embedding_maps_and_vector_geometry() {
        let doc = rich_fixture();
        let bytes = encode(&doc).unwrap();
        let raw = String::from_utf8_lossy(&bytes);
        assert_eq!(
            raw.matches("/FontFile2 ").count(),
            if cfg!(target_os = "macos") { 9 } else { 8 }
        );
        assert_eq!(
            raw.matches("/ToUnicode ").count(),
            if cfg!(target_os = "macos") { 9 } else { 8 }
        );
        assert!(!raw.contains("/Subtype /Image"));
        let streams = decoded_streams(&bytes);
        assert!(streams.contains(&utf16_hex(
            "Native PDF: café Ελληνικά e\u{301} a\u{200d}b z\u{e0100}",
            true
        )));
        assert!(streams.contains("<0301>"));
        assert!(streams.contains("<200D>"));
        assert!(streams.contains("<DB40DD00>"));
        assert!(streams.contains("3 Tr"));
        assert!(streams.contains(" re f"));
        assert!(streams.contains("0.75 w"));
        assert!(streams.contains(" Tm <"));
        assert_eq!(utf16_hex("𝄞", false), "D834DD1E");
        // Base and script sizes must produce distinct native text transforms.
        assert!(streams.contains("/F0 12.11454 Tf"));
        assert!(streams.contains("/F0 8.81057 Tf"));
    }
    #[test]
    fn glyph_outline_width_matches_editor_advance_after_pixel_rounding() {
        let doc = Document {
            blocks: vec![Block::Paragraph(Paragraph::plain("MMMM"))],
            ..Default::default()
        };
        let bytes = encode(&doc).unwrap();
        let streams = decoded_streams(&bytes);
        let text = streams.split(" Tf").next().unwrap();
        let pdf_size: f32 = text.split_whitespace().last().unwrap().parse().unwrap();
        let ctx = egui::Context::default();
        layout::install_fonts(&ctx);
        ctx.begin_pass(egui::RawInput::default());
        let composed = DocumentLayout::build(&ctx, &doc, 1.0);
        let _ = ctx.end_pass();
        let face = Face::parse(FONT_DATA[0], 0).unwrap();
        let advance = face
            .glyph_hor_advance(face.glyph_index('M').unwrap())
            .unwrap();
        let outline_width = pdf_size * f32::from(advance) / f32::from(face.units_per_em());
        let editor_width = composed.lines[0].galley.rows[0].glyphs[0].advance_width * PX_TO_PT;
        assert!(
            (outline_width - editor_width).abs() < 0.05,
            "PDF outline {outline_width}pt does not match editor advance {editor_width}pt"
        );
    }
    #[test]
    fn unsupported_visible_character_returns_clear_error() {
        let doc = Document {
            blocks: vec![Block::Paragraph(Paragraph::plain("\u{10ffff}"))],
            ..Default::default()
        };
        let error = encode(&doc).unwrap_err();
        assert!(
            error.contains("no embeddable font covers U+10FFFF"),
            "{error}"
        );
    }
    #[test]
    #[ignore = "Manual Poppler fixture: set FOLIO_PDF_FIXTURE_DIR to an explicit scratch directory"]
    fn write_review_fixtures() {
        let directory = std::env::var_os("FOLIO_PDF_FIXTURE_DIR")
            .expect("Set FOLIO_PDF_FIXTURE_DIR to a scratch directory");
        let directory = std::path::Path::new(&directory);
        std::fs::create_dir_all(directory).unwrap();
        std::fs::write(directory.join("rich.pdf"), encode(&rich_fixture()).unwrap()).unwrap();
        std::fs::write(
            directory.join("blank.pdf"),
            encode(&Document::default()).unwrap(),
        )
        .unwrap();
        let explicit = Document {
            blocks: vec![
                Block::Paragraph(Paragraph::plain("First page")),
                Block::PageBreak,
                Block::PageBreak,
                Block::Paragraph(Paragraph::plain("Third page")),
            ],
            ..Default::default()
        };
        std::fs::write(directory.join("explicit.pdf"), encode(&explicit).unwrap()).unwrap();
    }
}
