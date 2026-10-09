use crate::{
    DocxError, ExportReport,
    formatting::{Diagnostics, Styles, number, on, para_props, run_props},
    invalid,
    package::{Package, relationships, rels_path, target},
    xml::{self, Node},
};
use document_core::*;
use std::{
    collections::BTreeSet,
    io::{Read, Seek, Write},
};
const MAIN_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
const OFFICE_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/";
const STRICT_REL: &str = "http://purl.oclc.org/ooxml/officeDocument/relationships/";
fn kind(rel: &str, name: &str) -> bool {
    rel == format!("{OFFICE_REL}{name}") || rel == format!("{STRICT_REL}{name}")
}

pub(crate) fn import<R: Read + Seek>(reader: R) -> Result<ImportReport, DocxError> {
    let package = Package::read(reader)?;
    let ct = package.xml("[Content_Types].xml")?;
    if ct.ns != xml::CT || ct.name != "Types" {
        return Err(invalid("content types part has wrong root"));
    }
    let root_rels = relationships(&package.xml("_rels/.rels")?)?;
    let mains: Vec<_> = root_rels
        .iter()
        .filter(|r| kind(&r.kind, "officeDocument"))
        .collect();
    if mains.len() != 1 {
        return Err(invalid(
            "package requires exactly one officeDocument relationship",
        ));
    }
    if mains[0].external {
        return Err(invalid("main document cannot be external"));
    }
    let main = target("", &mains[0].target)?;
    let mut defaults = std::collections::BTreeMap::new();
    let mut overrides = std::collections::BTreeMap::new();
    for c in &ct.children {
        if c.ns != xml::CT {
            return Err(invalid("invalid content type namespace"));
        }
        let ty = c
            .plain("ContentType")
            .ok_or_else(|| invalid("content type missing ContentType"))?;
        match c.name.as_str() {
            "Default" => {
                let extension = c
                    .plain("Extension")
                    .ok_or_else(|| invalid("content type missing Extension"))?;
                if defaults.insert(extension, ty).is_some() {
                    return Err(invalid("duplicate default content type"));
                }
            }
            "Override" => {
                let name = c
                    .plain("PartName")
                    .ok_or_else(|| invalid("content type missing PartName"))?;
                if !name.starts_with('/') {
                    return Err(invalid("content type PartName must be absolute"));
                }
                if overrides.insert(name, ty).is_some() {
                    return Err(invalid("duplicate override content type"));
                }
            }
            _ => return Err(invalid("invalid content type element")),
        }
    }
    let main_key = format!("/{main}");
    let main_type = overrides
        .get(main_key.as_str())
        .copied()
        .or_else(|| {
            main.rsplit_once('.')
                .and_then(|(_, ext)| defaults.get(ext).copied())
        })
        .ok_or_else(|| invalid("main document content type missing"))?;
    if main_type != MAIN_TYPE {
        return Err(DocxError::UnsupportedPackage(format!(
            "main content type {main_type} (macros/templates/non-Word documents are unsupported)"
        )));
    }
    let mut consumed = BTreeSet::from([
        "[Content_Types].xml".to_string(),
        "_rels/.rels".to_string(),
        main.clone(),
    ]);
    let mut d = Diagnostics {
        part: "_rels/.rels".into(),
        ..Diagnostics::default()
    };
    for r in &root_rels {
        if !kind(&r.kind, "officeDocument") {
            d.warn(
                Feature::Other("package relationship".into()),
                format!("Relationship {} omitted", r.kind),
            );
        }
    }
    let rels = rels_path(&main);
    let mut styles = Styles::default();
    if package.parts.contains_key(&rels) {
        consumed.insert(rels.clone());
        d.part = rels.clone();
        let mut found_styles = false;
        for r in relationships(&package.xml(&rels)?)? {
            if kind(&r.kind, "styles") && !r.external {
                if found_styles {
                    return Err(invalid("multiple styles relationships"));
                }
                found_styles = true;
                let name = target(&main, &r.target)?;
                consumed.insert(name.clone());
                d.part = name.clone();
                styles = Styles::parse(package.xml(&name)?, &mut d)?;
            } else {
                let feature = if kind(&r.kind, "header") || kind(&r.kind, "footer") {
                    Feature::HeadersFooters
                } else if kind(&r.kind, "image") {
                    Feature::Images
                } else if kind(&r.kind, "numbering") {
                    Feature::Lists
                } else {
                    Feature::Other("relationship".into())
                };
                d.part = rels.clone();
                d.warn(
                    feature,
                    format!(
                        "{} relationship {} omitted; external targets are never fetched",
                        if r.external {
                            "External"
                        } else {
                            "Unsupported"
                        },
                        r.kind
                    ),
                );
            }
        }
    }
    for name in package.parts.keys() {
        if !consumed.contains(name) {
            d.part = name.clone();
            d.warn(
                Feature::Other("package part".into()),
                "Unrepresented package part omitted from converted copy",
            );
        }
    }
    d.part = main;
    let root = package.xml(&d.part)?;
    if !root.is("document") {
        return Err(invalid(
            "main document root must be WordprocessingML document",
        ));
    }
    d.attrs(&root, &[], &[]);
    unexpected_text(&root, &mut d);
    let bodies: Vec<_> = root.children.iter().filter(|n| n.is("body")).collect();
    if bodies.len() != 1 {
        return Err(invalid("document requires exactly one body"));
    }
    for n in &root.children {
        if !n.is("body") {
            d.unknown(n);
        }
    }
    let body = bodies[0];
    d.attrs(body, &[], &[]);
    unexpected_text(body, &mut d);
    let mut doc = Document {
        blocks: Vec::new(),
        page_layout: PageLayout::default(),
    };
    let mut section = false;
    for n in &body.children {
        if n.is("p") {
            paragraph(n, &styles, &mut doc.blocks, &mut d)?;
        } else if n.is("sectPr") {
            if section {
                d.warn(
                    Feature::Sections,
                    "Multiple section layouts reduced to the final layout",
                );
            }
            section = true;
            layout(n, &mut doc.page_layout, &mut d)?;
        } else {
            d.unknown(n);
        }
    }
    if !matches!(doc.blocks.first(), Some(Block::Paragraph(_))) {
        doc.blocks.insert(0, Block::Paragraph(Paragraph::default()));
    }
    if !matches!(doc.blocks.last(), Some(Block::Paragraph(_))) {
        doc.blocks.push(Block::Paragraph(Paragraph::default()));
    }
    doc.normalize();
    doc.validate()?;
    Ok(ImportReport {
        document: doc,
        warnings: d.warnings,
    })
}
fn paragraph(
    n: &Node,
    styles: &Styles,
    blocks: &mut Vec<Block>,
    d: &mut Diagnostics,
) -> Result<(), DocxError> {
    d.attrs(n, &[], &[]);
    unexpected_text(n, d);
    let mut p = Paragraph {
        style: styles.paragraph.clone(),
        default_style: styles.run.clone(),
        runs: Vec::new(),
    };
    single(n, "pPr")?;
    let props = n.child("pPr");
    let pid = props
        .and_then(|n| n.child("pStyle"))
        .and_then(|n| n.attr("val"))
        .or(styles.default_p.as_deref());
    if let Some(id) = pid {
        styles.apply(id, &mut p.default_style, Some(&mut p.style), d)?;
    }
    let run_base = p.default_style.clone();
    let mut break_before = if let Some(id) = pid {
        styles.page_break_before(id)?.unwrap_or(false)
    } else {
        false
    };
    if let Some(props) = props {
        para_props(props, &mut p.style, d)?;
        single(props, "rPr")?;
        if let Some(mark) = props.child("rPr") {
            if let Some(id) = mark.child("rStyle").and_then(|r| r.attr("val")) {
                styles.apply(id, &mut p.default_style, None, d)?;
            }
            run_props(mark, &mut p.default_style, false, d)?;
        }
        if let Some(before) = props.child("pageBreakBefore") {
            break_before = on(before)?;
        }
    }
    if break_before {
        blocks.push(Block::PageBreak);
    }
    let mut pieces = Vec::new();
    for c in &n.children {
        if c.is("pPr") {
            continue;
        }
        inline(c, styles, &run_base, &mut pieces, d)?;
    }
    // Only an unformatted break-only paragraph is the canonical encoding of a
    // structural break. Keep styled empty paragraphs on either side of a break
    // so spacing and paragraph-mark formatting survive conversion.
    if !pieces.is_empty() && pieces.iter().all(Option::is_none) && p == Paragraph::default() {
        blocks.extend(pieces.into_iter().map(|_| Block::PageBreak));
        return Ok(());
    }
    for piece in pieces {
        if let Some(run) = piece {
            p.runs.push(run);
        } else {
            blocks.push(Block::Paragraph(p.clone()));
            p.runs.clear();
            blocks.push(Block::PageBreak);
        }
    }
    blocks.push(Block::Paragraph(p));
    Ok(())
}
fn inline(
    n: &Node,
    styles: &Styles,
    base: &TextStyle,
    pieces: &mut Vec<Option<Run>>,
    d: &mut Diagnostics,
) -> Result<(), DocxError> {
    if !n.is("r") {
        d.unknown(n);
        // Preserve readable cached text inside hyperlinks/fields/revisions while
        // explicitly reporting the semantics that have been discarded.
        if n.is("hyperlink")
            || n.is("fldSimple")
            || n.is("ins")
            || n.is("sdt")
            || n.is("sdtContent")
        {
            for c in &n.children {
                inline(c, styles, base, pieces, d)?;
            }
        }
        return Ok(());
    }
    d.attrs(n, &[], &[]);
    unexpected_text(n, d);
    let mut style = base.clone();
    single(n, "rPr")?;
    let props = n.child("rPr");
    let rid = props
        .and_then(|n| n.child("rStyle"))
        .and_then(|n| n.attr("val"))
        .or(styles.default_r.as_deref());
    if let Some(id) = rid {
        styles.apply(id, &mut style, None, d)?;
    }
    if let Some(props) = props {
        run_props(props, &mut style, false, d)?;
    }
    for c in &n.children {
        if c.is("rPr") {
            continue;
        }
        if c.is("t") {
            d.attrs(c, &[], &[]);
            if !c.children.is_empty() {
                return Err(invalid("text element must not contain nested markup"));
            }
            let mut text = c.text.clone();
            if text.contains(['\n', '\r', '\u{c}']) {
                d.warn(
                    Feature::Other("text separators".into()),
                    "Literal line separators in text approximated as spaces",
                );
                text = text.replace(['\n', '\r', '\u{c}'], " ");
            }
            pieces.push(Some(Run::new(text, style.clone())));
        } else if c.is("tab") {
            d.attrs(c, &[], &[]);
            pieces.push(Some(Run::new("\t", style.clone())));
        } else if c.is("br") {
            d.attrs(c, &["type"], &[]);
            if c.attr("type") == Some("page") {
                pieces.push(None);
            } else {
                d.warn(
                    Feature::Other("line break".into()),
                    "Line/column break approximated as a space",
                );
                pieces.push(Some(Run::new(" ", style.clone())));
            }
        } else {
            d.unknown(c);
        }
    }
    Ok(())
}
fn layout(n: &Node, p: &mut PageLayout, d: &mut Diagnostics) -> Result<(), DocxError> {
    d.attrs(n, &[], &[]);
    unexpected_text(n, d);
    single(n, "pgSz")?;
    single(n, "pgMar")?;
    for c in &n.children {
        if c.is("pgSz") {
            d.attrs(c, &["w", "h", "orient"], &[]);
            let w = number(c, "w")?.ok_or_else(|| invalid("page size missing w"))?;
            let h = number(c, "h")?.ok_or_else(|| invalid("page size missing h"))?;
            p.orientation = match c.attr("orient").unwrap_or("portrait") {
                "portrait" => Orientation::Portrait,
                "landscape" => Orientation::Landscape,
                _ => return Err(invalid("invalid page orientation")),
            };
            p.size = if p.orientation == Orientation::Landscape {
                PageSize {
                    width_twips: h,
                    height_twips: w,
                }
            } else {
                PageSize {
                    width_twips: w,
                    height_twips: h,
                }
            };
        } else if c.is("pgMar") {
            d.attrs(
                c,
                &[
                    "top", "right", "bottom", "left", "header", "footer", "gutter",
                ],
                &[],
            );
            if let Some(v) = number(c, "top")? {
                p.margins.top = v;
            }
            if let Some(v) = number(c, "right")? {
                p.margins.right = v;
            }
            if let Some(v) = number(c, "bottom")? {
                p.margins.bottom = v;
            }
            if let Some(v) = number(c, "left")? {
                p.margins.left = v;
            }
            if number::<u32>(c, "gutter")?.is_some_and(|v| v != 0) {
                d.warn(Feature::Sections, "Nonzero page gutter omitted");
            }
            // Header/footer offsets have no visible effect without header/footer
            // content, whose references and package parts separately warn.
        } else {
            d.unknown(c);
        }
    }
    p.validate()?;
    Ok(())
}
fn escape(s: &str) -> String {
    quick_xml::escape::escape(s).into_owned()
}
fn run_xml(s: &TextStyle) -> String {
    // CT_RPr and CT_PPr schema order matters to Word/Open XML validators.
    format!(
        "<w:rPr><w:rFonts w:ascii=\"{}\" w:hAnsi=\"{}\"/><w:b w:val=\"{}\"/><w:i w:val=\"{}\"/><w:color w:val=\"{:02X}{:02X}{:02X}\"/><w:sz w:val=\"{}\"/><w:u w:val=\"{}\"/></w:rPr>",
        escape(&s.font_family),
        escape(&s.font_family),
        s.bold,
        s.italic,
        s.color.red,
        s.color.green,
        s.color.blue,
        s.size_half_points,
        if s.underline { "single" } else { "none" }
    )
}
pub(crate) fn export<W: Write + Seek>(
    doc: &Document,
    writer: W,
) -> Result<ExportReport, DocxError> {
    doc.validate()?;
    let mut d = Diagnostics {
        part: "word/document.xml".into(),
        ..Diagnostics::default()
    };
    // Validate XML characters before writing anything to the caller's stream.
    for block in &doc.blocks {
        if let Block::Paragraph(p) = block {
            for value in std::iter::once(p.default_style.font_family.as_str()).chain(
                p.runs
                    .iter()
                    .flat_map(|r| [r.text.as_str(), r.style.font_family.as_str()]),
            ) {
                if value.chars().any(|c| !matches!(c as u32, 0x9 | 0xA | 0xD | 0x20..=0xD7FF | 0xE000..=0xFFFD | 0x10000..=0x10FFFF)) { return Err(invalid("text/font contains characters forbidden in XML 1.0")); }
            }
        }
    }
    let mut xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><w:document xmlns:w=\"{}\"><w:body>",
        xml::W
    );
    for block in &doc.blocks {
        match block {
            Block::PageBreak => xml.push_str("<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>"),
            Block::Paragraph(p) => {
                let align = match p.style.alignment {
                    Alignment::Left => "left",
                    Alignment::Center => "center",
                    Alignment::Right => "right",
                    Alignment::Justify => "both",
                };
                let (line, rule) = match p.style.line_spacing {
                    LineSpacing::Multiple(v) => {
                        if u32::from(v) * 240 % 100 != 0 {
                            d.approximate(
                                Feature::Styles,
                                "Line spacing rounded to Word's 1/240-line units",
                            );
                        }
                        ((u32::from(v) * 240 + 50) / 100, "auto")
                    }
                    LineSpacing::Exact(v) => (v, "exact"),
                    LineSpacing::AtLeast(v) => (v, "atLeast"),
                };
                xml.push_str(&format!("<w:p><w:pPr><w:spacing w:before=\"{}\" w:after=\"{}\" w:line=\"{line}\" w:lineRule=\"{rule}\"/><w:jc w:val=\"{align}\"/>{}</w:pPr>", p.style.space_before_twips, p.style.space_after_twips, run_xml(&p.default_style)));
                for r in &p.runs {
                    xml.push_str(&format!("<w:r>{}", run_xml(&r.style)));
                    for (i, text) in r.text.split('\t').enumerate() {
                        if i != 0 {
                            xml.push_str("<w:tab/>");
                        }
                        xml.push_str(&format!(
                            "<w:t xml:space=\"preserve\">{}</w:t>",
                            escape(text)
                        ));
                    }
                    xml.push_str("</w:r>");
                }
                xml.push_str("</w:p>");
            }
        }
        if xml.len() as u64 > crate::package::MAX_PART {
            return Err(DocxError::ResourceLimit(
                "exported document XML exceeds 8 MiB".into(),
            ));
        }
    }
    let size = doc.page_layout.effective_size();
    let m = doc.page_layout.margins;
    xml.push_str(&format!("<w:sectPr><w:pgSz w:w=\"{}\" w:h=\"{}\" w:orient=\"{}\"/><w:pgMar w:top=\"{}\" w:right=\"{}\" w:bottom=\"{}\" w:left=\"{}\" w:header=\"0\" w:footer=\"0\" w:gutter=\"0\"/></w:sectPr></w:body></w:document>", size.width_twips, size.height_twips, if doc.page_layout.orientation == Orientation::Landscape { "landscape" } else { "portrait" }, m.top, m.right, m.bottom, m.left));
    if xml.len() as u64 > crate::package::MAX_PART {
        return Err(DocxError::ResourceLimit(
            "exported document XML exceeds 8 MiB".into(),
        ));
    }
    // Reopening must stay inside the same structural/event limits as import.
    xml::parse(xml.as_bytes())?;
    let ct = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Types xmlns=\"{}\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/word/document.xml\" ContentType=\"{MAIN_TYPE}\"/></Types>",
        xml::CT
    );
    let rels = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"{}\"><Relationship Id=\"rId1\" Type=\"{OFFICE_REL}officeDocument\" Target=\"word/document.xml\"/></Relationships>",
        xml::REL
    );
    let mut zip = zip::ZipWriter::new(writer);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in [
        ("[Content_Types].xml", ct),
        ("_rels/.rels", rels),
        ("word/document.xml", xml),
    ] {
        zip.start_file(name, options)?;
        zip.write_all(bytes.as_bytes())?;
    }
    zip.finish()?;
    Ok(ExportReport {
        warnings: d.warnings,
    })
}

fn single(n: &Node, name: &str) -> Result<(), DocxError> {
    if n.children.iter().filter(|n| n.is(name)).count() > 1 {
        return Err(invalid(format!("duplicate {name} in {}", n.name)));
    }
    Ok(())
}

fn unexpected_text(n: &Node, d: &mut Diagnostics) {
    if !n.text.trim().is_empty() {
        d.warn(
            Feature::Other("unexpected text".into()),
            format!(
                "Text outside a supported text element in {} omitted",
                n.name
            ),
        );
    }
}
