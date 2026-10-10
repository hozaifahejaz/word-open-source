//! Pure, full-document export adapters. Filesystem replacement belongs to `files`.
use document_core::{
    Alignment, Block, Color, Document, LineSpacing, ParagraphStyle, TextStyle, VerticalAlign,
};
use std::{
    fmt::Write as _,
    io::{Cursor, Write as _},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Pdf,
    Docx,
    Odt,
    Rtf,
    Html,
    Markdown,
    Text,
}
pub const ALL: [ExportFormat; 7] = [
    ExportFormat::Pdf,
    ExportFormat::Docx,
    ExportFormat::Odt,
    ExportFormat::Rtf,
    ExportFormat::Html,
    ExportFormat::Markdown,
    ExportFormat::Text,
];
impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Docx => "docx",
            Self::Odt => "odt",
            Self::Rtf => "rtf",
            Self::Html => "html",
            Self::Markdown => "md",
            Self::Text => "txt",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Pdf => "PDF",
            Self::Docx => "Word document",
            Self::Odt => "OpenDocument",
            Self::Rtf => "Rich Text",
            Self::Html => "HTML",
            Self::Markdown => "Markdown",
            Self::Text => "Plain text",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Pdf => {
                "Fixed pages with text formatting and embedded bundled Noto fonts; system fallback depends on glyph coverage and missing glyphs stop export."
            }
            Self::Docx => "Editable Word document with supported text formatting and page layout.",
            Self::Odt => "Editable OpenDocument with text formatting and page layout.",
            Self::Rtf => "Editable rich text with text formatting and page layout.",
            Self::Html => {
                "Standalone web document with text styles and print page settings; browser pagination varies."
            }
            Self::Markdown => {
                "Text with bold, italic, strike and HTML underline/scripts; fonts, colors, spacing and page geometry are omitted. Page breaks use HTML."
            }
            Self::Text => {
                "UTF-8 text with paragraph newlines and form-feed page breaks; all formatting and page geometry are omitted."
            }
        }
    }
}
pub fn encode(doc: &Document, format: ExportFormat) -> Result<Vec<u8>, String> {
    doc.validate().map_err(|e| e.to_string())?;
    if matches!(
        format,
        ExportFormat::Docx | ExportFormat::Odt | ExportFormat::Html
    ) {
        for block in &doc.blocks {
            if let Block::Paragraph(p) = block {
                for value in std::iter::once(p.default_style.font_family.as_str()).chain(
                    p.runs
                        .iter()
                        .flat_map(|r| [r.text.as_str(), r.style.font_family.as_str()]),
                ) {
                    if value.chars().any(|c| !xml_char(c)) {
                        return Err("Document contains a control character that this markup document format cannot represent".into());
                    }
                }
            }
        }
    }
    match format {
        ExportFormat::Pdf => crate::export_pdf::encode(doc),
        ExportFormat::Docx => {
            let mut out = Cursor::new(Vec::new());
            let report = folio_docx::export_docx(doc, &mut out).map_err(|e| e.to_string())?;
            if !report.warnings.is_empty() {
                return Err(format!(
                    "Export stopped: {}",
                    report
                        .warnings
                        .iter()
                        .map(|w| w.message.as_str())
                        .collect::<Vec<_>>()
                        .join("; ")
                ));
            }
            Ok(out.into_inner())
        }
        ExportFormat::Odt => odt(doc),
        ExportFormat::Rtf => Ok(rtf(doc).into_bytes()),
        ExportFormat::Html => Ok(html(doc).into_bytes()),
        ExportFormat::Markdown => Ok(markdown(doc).into_bytes()),
        ExportFormat::Text => Ok(crate::files::plain_text(doc, None)?.into_bytes()),
    }
}
fn xml_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r') || c >= '\u{20}' && c != '\u{fffe}' && c != '\u{ffff}'
}
fn escaped(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn color(c: Color) -> String {
    format!("#{:02x}{:02x}{:02x}", c.red, c.green, c.blue)
}
fn pt(twips: u32) -> String {
    format!("{:.4}pt", f64::from(twips) / 20.0)
}
fn alignment(a: Alignment) -> &'static str {
    match a {
        Alignment::Left => "left",
        Alignment::Center => "center",
        Alignment::Right => "right",
        Alignment::Justify => "justify",
    }
}
// CSS hexadecimal escapes prevent font names from breaking strings or style elements.
fn css_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        write!(out, "\\{:x} ", c as u32).unwrap();
    }
    out.push('"');
    out
}
fn run_css(s: &TextStyle) -> String {
    let decorations = [
        s.underline.then_some("underline"),
        s.strikethrough.then_some("line-through"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ");
    format!(
        "font-family:{};font-size:{}pt;font-weight:{};font-style:{};text-decoration:{};color:{};background-color:{};vertical-align:{};",
        css_string(&s.font_family),
        f64::from(s.size_half_points) / 2.0,
        if s.bold { "bold" } else { "normal" },
        if s.italic { "italic" } else { "normal" },
        if decorations.is_empty() {
            "none"
        } else {
            &decorations
        },
        color(s.color),
        s.highlight
            .map(color)
            .unwrap_or_else(|| "transparent".into()),
        match s.vertical_align {
            VerticalAlign::Baseline => "baseline",
            VerticalAlign::Superscript => "super",
            VerticalAlign::Subscript => "sub",
        }
    )
}
fn paragraph_css(s: &ParagraphStyle) -> String {
    let line = match s.line_spacing {
        LineSpacing::Multiple(n) => format!("line-height:{};", f64::from(n) / 100.0),
        LineSpacing::Exact(n) => format!("line-height:{};", pt(n)),
        LineSpacing::AtLeast(n) => format!("line-height:max(1em, {});", pt(n)),
    };
    format!(
        "text-align:{};margin:{} 0 {};{}",
        alignment(s.alignment),
        pt(s.space_before_twips),
        pt(s.space_after_twips),
        line
    )
}
fn html(doc: &Document) -> String {
    let page = doc.page_layout.effective_size();
    let m = doc.page_layout.margins;
    let mut out = format!(
        "<!doctype html>\n<html lang=\"und\"><head><meta charset=\"utf-8\"><title>Document</title><style>@page{{size:{} {};margin:{} {} {} {};}}body{{margin:0;}}p{{white-space:pre-wrap;overflow-wrap:break-word;min-height:1em;}}.page-break{{break-before:page;page-break-before:always;}}</style></head><body>\n",
        pt(page.width_twips),
        pt(page.height_twips),
        pt(m.top),
        pt(m.right),
        pt(m.bottom),
        pt(m.left)
    );
    for block in &doc.blocks {
        match block {
            Block::PageBreak => out.push_str("<div class=\"page-break\"></div>\n"),
            Block::Paragraph(p) => {
                write!(
                    out,
                    "<p style=\"{}{}\">",
                    escaped(&paragraph_css(&p.style)),
                    escaped(&format!(
                        "font-family:{};font-size:{}pt;color:{};",
                        css_string(&p.default_style.font_family),
                        f64::from(p.default_style.size_half_points) / 2.0,
                        color(p.default_style.color)
                    ))
                )
                .unwrap();
                for r in &p.runs {
                    write!(
                        out,
                        "<span style=\"{}\">{}</span>",
                        escaped(&run_css(&r.style)),
                        escaped(&r.text)
                    )
                    .unwrap();
                }
                out.push_str("</p>\n");
            }
        }
    }
    out.push_str("</body></html>\n");
    out
}
fn markdown(doc: &Document) -> String {
    let mut blocks = Vec::new();
    for block in &doc.blocks {
        match block {
            Block::PageBreak => blocks
                .push("<div style=\"break-before:page;page-break-before:always\"></div>".into()),
            Block::Paragraph(p) => {
                let mut projected = p.clone();
                for r in &mut projected.runs {
                    r.style.font_family = String::from("sans-serif");
                    r.style.size_half_points = 24;
                    r.style.color = Color::BLACK;
                    r.style.highlight = None;
                }
                projected.normalize();
                let mut value = String::new();
                for r in &projected.runs {
                    // Preserve literal text and prevent Markdown/HTML injection, including numbered lists.
                    let mut text = String::new();
                    for c in r.text.chars() {
                        match c {
                            '&' => text.push_str("&amp;"),
                            '<' => text.push_str("&lt;"),
                            '>' => text.push_str("&gt;"),
                            '\t' => text.push_str("&#9;"),
                            ' ' => text.push_str("&#32;"),
                            c if c.is_ascii_punctuation() => {
                                text.push('\\');
                                text.push(c)
                            }
                            c => text.push(c),
                        }
                    }
                    if text.is_empty() {
                        continue;
                    }
                    if r.style.bold {
                        text = format!("**{text}**");
                    }
                    if r.style.italic {
                        text = format!("<em>{text}</em>");
                    }
                    if r.style.strikethrough {
                        text = format!("~~{text}~~");
                    }
                    if r.style.underline {
                        text = format!("<u>{text}</u>");
                    }
                    text = match r.style.vertical_align {
                        VerticalAlign::Baseline => text,
                        VerticalAlign::Superscript => format!("<sup>{text}</sup>"),
                        VerticalAlign::Subscript => format!("<sub>{text}</sub>"),
                    };
                    value.push_str(&text);
                }
                blocks.push(if value.is_empty() {
                    "<br>".into()
                } else {
                    value
                });
            }
        }
    }
    format!("{}\n", blocks.join("\n\n"))
}
fn rtf_text(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '\\' | '{' | '}' => {
                out.push('\\');
                out.push(c)
            }
            '\t' => out.push_str("\\tab "),
            c if c.is_ascii() && !c.is_control() => out.push(c),
            c => {
                let mut units = [0; 2];
                for u in c.encode_utf16(&mut units) {
                    write!(out, "\\u{}?", *u as i16).unwrap();
                }
            }
        }
    }
    out
}
fn rtf(doc: &Document) -> String {
    let mut fonts = Vec::new();
    let mut colors = Vec::new();
    for block in &doc.blocks {
        if let Block::Paragraph(p) = block {
            for s in std::iter::once(&p.default_style).chain(p.runs.iter().map(|r| &r.style)) {
                if !fonts.contains(&s.font_family) {
                    fonts.push(s.font_family.clone());
                }
                for c in std::iter::once(s.color).chain(s.highlight) {
                    if !colors.contains(&c) {
                        colors.push(c);
                    }
                }
            }
        }
    }
    let mut out = String::from("{\\rtf1\\ansi\\ansicpg1252\\deff0\\uc1{\\fonttbl");
    for (i, f) in fonts.iter().enumerate() {
        // Semicolons delimit font names, so encode them as Unicode.
        write!(
            out,
            "{{\\f{i}\\fnil {};}}",
            rtf_text(f).replace(';', "\\u59?")
        )
        .unwrap();
    }
    out.push_str("}{\\colortbl;");
    for c in &colors {
        write!(out, "\\red{}\\green{}\\blue{};", c.red, c.green, c.blue).unwrap();
    }
    out.push('}');
    let page = doc.page_layout.effective_size();
    let m = doc.page_layout.margins;
    writeln!(
        out,
        "\\paperw{}\\paperh{}\\margl{}\\margr{}\\margt{}\\margb{}{}",
        page.width_twips,
        page.height_twips,
        m.left,
        m.right,
        m.top,
        m.bottom,
        if doc.page_layout.orientation == document_core::Orientation::Landscape {
            "\\landscape"
        } else {
            ""
        }
    )
    .unwrap();
    let style = |s: &TextStyle| {
        format!(
            "\\plain\\f{}\\fs{}\\cf{}\\highlight{}\\b{}\\i{}\\ul{}\\strike{}{} ",
            fonts.iter().position(|f| f == &s.font_family).unwrap(),
            s.size_half_points,
            colors.iter().position(|c| *c == s.color).unwrap() + 1,
            s.highlight
                .map(|c| colors.iter().position(|v| *v == c).unwrap() + 1)
                .unwrap_or(0),
            u8::from(s.bold),
            u8::from(s.italic),
            u8::from(s.underline),
            u8::from(s.strikethrough),
            match s.vertical_align {
                VerticalAlign::Baseline => "\\nosupersub",
                VerticalAlign::Superscript => "\\super",
                VerticalAlign::Subscript => "\\sub",
            }
        )
    };
    for (index, block) in doc.blocks.iter().enumerate() {
        match block {
            Block::PageBreak => out.push_str("\\page\n"),
            Block::Paragraph(p) => {
                let (sl, mult) = match p.style.line_spacing {
                    LineSpacing::Multiple(n) => (i64::from(n) * 240 / 100, 1),
                    LineSpacing::Exact(n) => (-i64::from(n), 0),
                    LineSpacing::AtLeast(n) => (i64::from(n), 0),
                };
                write!(
                    out,
                    "\\pard{}\\sb{}\\sa{}\\sl{}\\slmult{}{}",
                    match p.style.alignment {
                        Alignment::Left => "\\ql",
                        Alignment::Center => "\\qc",
                        Alignment::Right => "\\qr",
                        Alignment::Justify => "\\qj",
                    },
                    p.style.space_before_twips,
                    p.style.space_after_twips,
                    sl,
                    mult,
                    style(&p.default_style)
                )
                .unwrap();
                for r in &p.runs {
                    write!(out, "{{{}{}}}", style(&r.style), rtf_text(&r.text)).unwrap();
                }
                if index + 1 < doc.blocks.len() {
                    out.push_str("\\par\n");
                }
            }
        }
    }
    out.push('}');
    out
}
const NS: &str = "xmlns:office=\"urn:oasis:names:tc:opendocument:xmlns:office:1.0\" xmlns:style=\"urn:oasis:names:tc:opendocument:xmlns:style:1.0\" xmlns:text=\"urn:oasis:names:tc:opendocument:xmlns:text:1.0\" xmlns:fo=\"urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0\" xmlns:svg=\"urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0\"";
fn odt_text_style(s: &TextStyle) -> String {
    format!(
        "fo:font-family=\"{}\" fo:font-size=\"{}pt\" fo:font-weight=\"{}\" fo:font-style=\"{}\" fo:color=\"{}\" fo:background-color=\"{}\" style:text-underline-style=\"{}\" style:text-underline-type=\"{}\" style:text-line-through-style=\"{}\" style:text-position=\"{}\"",
        escaped(&s.font_family),
        f64::from(s.size_half_points) / 2.0,
        if s.bold { "bold" } else { "normal" },
        if s.italic { "italic" } else { "normal" },
        color(s.color),
        s.highlight
            .map(color)
            .unwrap_or_else(|| "transparent".into()),
        if s.underline { "solid" } else { "none" },
        if s.underline { "single" } else { "none" },
        if s.strikethrough { "solid" } else { "none" },
        match s.vertical_align {
            VerticalAlign::Baseline => "0% 100%",
            VerticalAlign::Superscript => "super 100%",
            VerticalAlign::Subscript => "sub 100%",
        }
    )
}
fn odt_text(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            ' ' => out.push_str("<text:s/>"),
            '\t' => out.push_str("<text:tab/>"),
            c => out.push_str(&escaped(&c.to_string())),
        }
    }
    out
}
fn odt(doc: &Document) -> Result<Vec<u8>, String> {
    let mut automatic = String::from(
        "<style:style style:name=\"Break\" style:family=\"paragraph\"><style:paragraph-properties fo:break-before=\"page\"/></style:style>",
    );
    let mut body = String::new();
    let mut breaks = 0;
    for (i, block) in doc.blocks.iter().enumerate() {
        match block {
            Block::PageBreak => breaks += 1,
            Block::Paragraph(p) => {
                let line = match p.style.line_spacing {
                    LineSpacing::Multiple(n) => format!("fo:line-height=\"{n}%\""),
                    LineSpacing::Exact(n) => format!("fo:line-height=\"{}\"", pt(n)),
                    LineSpacing::AtLeast(n) => format!("style:line-height-at-least=\"{}\"", pt(n)),
                };
                write!(automatic, "<style:style style:name=\"P{i}\" style:family=\"paragraph\" style:master-page-name=\"Standard\"><style:paragraph-properties fo:text-align=\"{}\" fo:margin-top=\"{}\" fo:margin-bottom=\"{}\" {} {}/><style:text-properties {}/></style:style>", alignment(p.style.alignment), pt(p.style.space_before_twips),pt(p.style.space_after_twips),line, if breaks>0 {"fo:break-before=\"page\""} else {""}, odt_text_style(&p.default_style)).unwrap();
                for _ in 1..breaks {
                    body.push_str("<text:p text:style-name=\"Break\"/>");
                }
                breaks = 0;
                write!(body, "<text:p text:style-name=\"P{i}\">").unwrap();
                for (j, r) in p.runs.iter().enumerate() {
                    write!(automatic,"<style:style style:name=\"T{i}_{j}\" style:family=\"text\"><style:text-properties {}/></style:style>",odt_text_style(&r.style)).unwrap();
                    write!(
                        body,
                        "<text:span text:style-name=\"T{i}_{j}\">{}</text:span>",
                        odt_text(&r.text)
                    )
                    .unwrap();
                }
                body.push_str("</text:p>");
            }
        }
    }
    let content = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><office:document-content {NS} office:version=\"1.3\"><office:automatic-styles>{automatic}</office:automatic-styles><office:body><office:text>{body}</office:text></office:body></office:document-content>"
    );
    let page = doc.page_layout.effective_size();
    let m = doc.page_layout.margins;
    let styles = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><office:document-styles {NS} office:version=\"1.3\"><office:styles/><office:automatic-styles><style:page-layout style:name=\"Page\"><style:page-layout-properties fo:page-width=\"{}\" fo:page-height=\"{}\" style:print-orientation=\"{}\" fo:margin-top=\"{}\" fo:margin-right=\"{}\" fo:margin-bottom=\"{}\" fo:margin-left=\"{}\"/></style:page-layout></office:automatic-styles><office:master-styles><style:master-page style:name=\"Standard\" style:page-layout-name=\"Page\"/></office:master-styles></office:document-styles>",
        pt(page.width_twips),
        pt(page.height_twips),
        if doc.page_layout.orientation == document_core::Orientation::Landscape {
            "landscape"
        } else {
            "portrait"
        },
        pt(m.top),
        pt(m.right),
        pt(m.bottom),
        pt(m.left)
    );
    let manifest = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><manifest:manifest xmlns:manifest=\"urn:oasis:names:tc:opendocument:xmlns:manifest:1.0\" manifest:version=\"1.3\"><manifest:file-entry manifest:full-path=\"/\" manifest:media-type=\"application/vnd.oasis.opendocument.text\" manifest:version=\"1.3\"/><manifest:file-entry manifest:full-path=\"content.xml\" manifest:media-type=\"text/xml\"/><manifest:file-entry manifest:full-path=\"styles.xml\" manifest:media-type=\"text/xml\"/></manifest:manifest>";
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data, compression) in [
        (
            "mimetype",
            "application/vnd.oasis.opendocument.text",
            zip::CompressionMethod::Stored,
        ),
        (
            "content.xml",
            content.as_str(),
            zip::CompressionMethod::Deflated,
        ),
        (
            "styles.xml",
            styles.as_str(),
            zip::CompressionMethod::Deflated,
        ),
        (
            "META-INF/manifest.xml",
            manifest,
            zip::CompressionMethod::Deflated,
        ),
    ] {
        archive
            .start_file(
                name,
                zip::write::SimpleFileOptions::default().compression_method(compression),
            )
            .map_err(|e| e.to_string())?;
        archive
            .write_all(data.as_bytes())
            .map_err(|e| e.to_string())?;
    }
    Ok(archive.finish().map_err(|e| e.to_string())?.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use document_core::{Orientation, Paragraph, Run};
    use std::io::Read;
    fn fixture() -> Document {
        let style = TextStyle {
            bold: true,
            italic: true,
            underline: true,
            strikethrough: true,
            vertical_align: VerticalAlign::Superscript,
            highlight: Some(Color::rgb(255, 240, 0)),
            font_family: "Font \"; </style><script>&".into(),
            size_half_points: 29,
            color: Color::rgb(12, 34, 56),
        };
        let mut p = Paragraph::plain("");
        p.runs = vec![Run::new("  <script>& {x} \\ 🦀 اردو e\u{301}\t", style)];
        p.style.alignment = Alignment::Justify;
        p.style.space_before_twips = 120;
        p.style.space_after_twips = 240;
        p.style.line_spacing = LineSpacing::AtLeast(300);
        let mut doc = Document {
            blocks: vec![
                Block::Paragraph(p),
                Block::PageBreak,
                Block::PageBreak,
                Block::Paragraph(Paragraph::plain("second")),
            ],
            ..Document::default()
        };
        doc.page_layout.orientation = Orientation::Landscape;
        doc
    }
    #[test]
    fn odt_has_valid_xml_manifest_stored_first_mimetype_and_rich_styles() {
        let bytes = encode(&fixture(), ExportFormat::Odt).unwrap();
        assert_eq!(&bytes[0..4], b"PK\x03\x04");
        // ODF's first local header has no extra field and an uncompressed MIME string.
        assert_eq!(&bytes[8..10], &[0, 0]);
        assert_eq!(&bytes[28..30], &[0, 0]);
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let first = archive.by_index(0).unwrap();
        assert_eq!(first.name(), "mimetype");
        assert_eq!(first.compression(), zip::CompressionMethod::Stored);
        drop(first);
        for name in ["content.xml", "styles.xml", "META-INF/manifest.xml"] {
            let mut xml = String::new();
            archive
                .by_name(name)
                .unwrap()
                .read_to_string(&mut xml)
                .unwrap();
            let mut reader = quick_xml::Reader::from_str(&xml);
            let mut starts = 0;
            loop {
                match reader.read_event().unwrap() {
                    quick_xml::events::Event::Start(_) => starts += 1,
                    quick_xml::events::Event::Eof => break,
                    _ => (),
                }
            }
            assert!(starts > 0);
            if name == "content.xml" {
                assert!(xml.contains("style:text-line-through-style=\"solid\""));
                assert!(xml.contains("style:text-position=\"super 100%\""));
                assert!(xml.contains("style:line-height-at-least=\"15.0000pt\""));
                assert!(xml.contains("&lt;script&gt;&amp;"));
                let mut text_reader = quick_xml::Reader::from_str(&xml);
                let mut first_paragraph = String::new();
                loop {
                    match text_reader.read_event().unwrap() {
                        quick_xml::events::Event::Text(t) => {
                            first_paragraph.push_str(&t.xml_content().unwrap())
                        }
                        quick_xml::events::Event::GeneralRef(r) => {
                            first_paragraph.push_str(
                                &quick_xml::escape::unescape(&format!("&{};", r.decode().unwrap()))
                                    .unwrap(),
                            );
                        }
                        quick_xml::events::Event::Empty(e) if e.name().as_ref() == b"text:s" => {
                            first_paragraph.push(' ')
                        }
                        quick_xml::events::Event::Empty(e) if e.name().as_ref() == b"text:tab" => {
                            first_paragraph.push('\t')
                        }
                        quick_xml::events::Event::End(e) if e.name().as_ref() == b"text:p" => break,
                        quick_xml::events::Event::Eof => panic!("missing paragraph"),
                        _ => (),
                    }
                }
                assert_eq!(first_paragraph, fixture().paragraph(0).unwrap().text());
                assert!(xml.contains("<text:tab/>"));
                assert_eq!(xml.matches("fo:break-before=\"page\"").count(), 2);
            }
            if name == "styles.xml" {
                assert!(xml.contains("style:print-orientation=\"landscape\""));
                assert!(xml.contains("fo:page-width=\"841.9000pt\""));
            }
        }
    }
    #[test]
    fn rtf_is_ascii_balanced_and_unicode_uses_signed_utf16() {
        let value = String::from_utf8(encode(&fixture(), ExportFormat::Rtf).unwrap()).unwrap();
        assert!(value.is_ascii());
        assert!(value.starts_with("{\\rtf1\\ansi"));
        assert!(value.ends_with('}'));
        assert!(value.contains("\\u-10178?\\u-8832?"));
        assert!(value.contains("\\strike1\\super"));
        assert!(value.contains("\\highlight3"));
        assert!(value.contains(
            "{\\colortbl;\\red0\\green0\\blue0;\\red12\\green34\\blue56;\\red255\\green240\\blue0;}"
        ));
        assert!(value.contains("\\sl300\\slmult0"));
        assert!(value.contains("\\paperw16838\\paperh11906"));
        assert_eq!(value.matches("\\page\n").count(), 2);
        let mut depth = 0;
        let mut chars = value.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                if let Some(next) = chars.next()
                    && matches!(next, '{' | '}' | '\\')
                {
                    continue;
                }
            } else if c == '{' {
                depth += 1;
            } else if c == '}' {
                depth -= 1;
                assert!(depth >= 0);
            }
        }
        assert_eq!(depth, 0);
    }
    #[test]
    fn html_and_markdown_never_insert_raw_text_or_font_markup() {
        let doc = fixture();
        let html = String::from_utf8(encode(&doc, ExportFormat::Html).unwrap()).unwrap();
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;&amp;"));
        assert!(html.contains("text-decoration:underline line-through"));
        assert!(html.contains("vertical-align:super"));
        assert!(html.contains("@page{size:841.9000pt 595.3000pt"));
        assert!(html.contains("🦀 اردو e\u{301}"));
        let md = String::from_utf8(encode(&doc, ExportFormat::Markdown).unwrap()).unwrap();
        assert!(md.contains("<sup><u>~~<em>**"));
        assert!(md.contains("&lt;script&gt;&amp;"));
        assert!(!md.contains("<script>"));
        assert!(!md.contains("Font"));
        assert_eq!(md.matches("break-before:page").count(), 2);
        assert_eq!(
            encode(&doc, ExportFormat::Text).unwrap(),
            crate::files::plain_text(&doc, None).unwrap().into_bytes()
        );
    }
    #[test]
    fn markdown_italic_is_semantic_inside_a_word() {
        let doc = Document {
            blocks: vec![Block::Paragraph(Paragraph {
                runs: vec![
                    Run::new("a", TextStyle::default()),
                    Run::new(
                        "b",
                        TextStyle {
                            italic: true,
                            ..TextStyle::default()
                        },
                    ),
                    Run::new("c", TextStyle::default()),
                ],
                ..Paragraph::default()
            })],
            ..Document::default()
        };
        assert_eq!(
            String::from_utf8(encode(&doc, ExportFormat::Markdown).unwrap()).unwrap(),
            "a<em>b</em>c\n"
        );
    }

    #[test]
    fn html_typing_style_does_not_decorate_plain_runs() {
        let typing = TextStyle {
            underline: true,
            strikethrough: true,
            highlight: Some(Color::rgb(255, 240, 0)),
            ..TextStyle::default()
        };
        let doc = Document {
            blocks: vec![Block::Paragraph(Paragraph {
                default_style: typing.clone(),
                runs: vec![
                    Run::new("plain", TextStyle::default()),
                    Run::new("marked", typing),
                ],
                ..Paragraph::default()
            })],
            ..Document::default()
        };
        let output = String::from_utf8(encode(&doc, ExportFormat::Html).unwrap()).unwrap();
        let p_style = output
            .split("<p style=\"")
            .nth(1)
            .unwrap()
            .split("\">")
            .next()
            .unwrap();
        assert!(!p_style.contains("text-decoration"));
        assert!(!p_style.contains("background-color"));
        let plain_span = output
            .split("<span style=\"")
            .nth(1)
            .unwrap()
            .split("</span>")
            .next()
            .unwrap();
        assert!(plain_span.ends_with("plain"));
        assert!(plain_span.contains("text-decoration:none"));
        assert!(plain_span.contains("background-color:transparent"));
        assert!(output.contains("text-decoration:underline line-through"));
        assert!(output.contains("background-color:#fff000"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_readers_recover_literal_unicode_text() {
        use std::process::{Command, Stdio};
        let doc = fixture();
        if let Some(directory) = std::env::var_os("FOLIO_EXPORT_REVIEW_DIR") {
            let directory = std::path::PathBuf::from(directory);
            std::fs::create_dir_all(&directory).unwrap();
            for format in [
                ExportFormat::Docx,
                ExportFormat::Odt,
                ExportFormat::Rtf,
                ExportFormat::Html,
                ExportFormat::Markdown,
                ExportFormat::Text,
            ] {
                std::fs::write(
                    directory.join(format!("rich-unicode.{}", format.extension())),
                    encode(&doc, format).unwrap(),
                )
                .unwrap();
            }
        }
        for format in [ExportFormat::Rtf, ExportFormat::Odt, ExportFormat::Docx] {
            let mut reader = Command::new("/usr/bin/textutil")
                .args([
                    "-convert",
                    "txt",
                    "-stdout",
                    "-stdin",
                    "-format",
                    format.extension(),
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            reader
                .stdin
                .take()
                .unwrap()
                .write_all(&encode(&doc, format).unwrap())
                .unwrap();
            let output = reader.wait_with_output().unwrap();
            assert!(
                output.status.success(),
                "{format:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let text = String::from_utf8(output.stdout).unwrap();
            assert!(
                text.starts_with(&doc.paragraph(0).unwrap().text()),
                "{format:?}: {text:?}"
            );
            assert!(text.trim_end().ends_with("second"), "{format:?}: {text:?}");
            if format == ExportFormat::Rtf {
                assert_eq!(text.matches('\u{000c}').count(), 2);
            }
        }
    }

    #[test]
    fn rejects_invalid_documents_before_any_encoding_and_illegal_xml() {
        let mut doc = fixture();
        doc.page_layout.size.width_twips = 0;
        for format in ALL {
            assert!(encode(&doc, format).is_err());
        }
        let doc = Document {
            blocks: vec![Block::Paragraph(Paragraph::plain("null\0"))],
            ..Document::default()
        };
        for format in [ExportFormat::Odt, ExportFormat::Docx, ExportFormat::Html] {
            assert!(encode(&doc, format).is_err());
        }
    }
    #[test]
    fn docx_reimports_rich_content_without_warnings() {
        let doc = fixture();
        let bytes = encode(&doc, ExportFormat::Docx).unwrap();
        let imported = folio_docx::import_docx(Cursor::new(bytes)).unwrap();
        assert!(imported.warnings.is_empty());
        assert_eq!(imported.document, doc);
    }
}
