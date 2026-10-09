use document_core::*;
use folio_docx::*;
use std::io::{Cursor, Write};
const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const CT: &str = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
const REL: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="main" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
const STYLE_REL: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="styles" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;
fn zip(parts: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in parts {
        out.start_file(
            *name,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
        out.write_all(bytes).unwrap();
    }
    out.finish().unwrap().into_inner()
}
fn parts(doc: &str) -> Vec<(&str, &[u8])> {
    vec![
        ("[Content_Types].xml", CT.as_bytes()),
        ("_rels/.rels", REL.as_bytes()),
        ("word/document.xml", doc.as_bytes()),
    ]
}
fn import(doc: &str) -> ImportReport {
    import_docx(Cursor::new(zip(&parts(doc)))).unwrap()
}
fn roundtrip(doc: &Document) -> ImportReport {
    let mut out = Cursor::new(Vec::new());
    let report = export_docx(doc, &mut out).unwrap();
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    let result = import_docx(Cursor::new(out.into_inner())).unwrap();
    assert!(result.warnings.is_empty(), "{:?}", result.warnings);
    let mut expected = doc.clone();
    expected.normalize();
    assert_eq!(expected, result.document);
    result
}
#[test]
fn authored_fixture_unicode_styles_page_and_breaks() {
    let report = import(include_str!("fixtures/styled.xml"));
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    let doc = report.document;
    assert_eq!(doc.blocks.len(), 5);
    let p = doc.paragraph(0).unwrap();
    assert_eq!(p.text(), "  Café & <你好> é 👩‍👩‍👧‍👦 \tاردو");
    assert_eq!(p.style.alignment, Alignment::Center);
    assert_eq!(p.style.line_spacing, LineSpacing::Multiple(150));
    assert_eq!(p.style.space_before_twips, 120);
    assert_eq!(p.runs[0].style.size_half_points, 29);
    assert_eq!(p.runs[0].style.color, Color::rgb(0x1A, 0x2B, 0x3C));
    assert!(p.runs[0].style.bold && p.runs[0].style.italic && p.runs[0].style.underline);
    assert_eq!(doc.page_layout.size, PageSize::LETTER);
    assert_eq!(doc.page_layout.orientation, Orientation::Landscape);
    assert_eq!(
        doc.page_layout.margins,
        Margins {
            top: 720,
            right: 800,
            bottom: 900,
            left: 1000
        }
    );
    roundtrip(&doc);
}
#[test]
fn inherited_paragraph_character_defaults_and_direct_formatting() {
    let mut pkg = parts(include_str!("fixtures/inherited.xml"));
    pkg.extend([
        ("word/_rels/document.xml.rels", STYLE_REL.as_bytes()),
        (
            "word/styles.xml",
            include_bytes!("fixtures/styles.xml").as_slice(),
        ),
    ]);
    let report = import_docx(Cursor::new(zip(&pkg))).unwrap();
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
    let p = report.document.paragraph(0).unwrap();
    assert_eq!(p.style.alignment, Alignment::Center);
    assert_eq!(p.style.space_before_twips, 60);
    assert_eq!(p.style.space_after_twips, 100);
    assert_eq!(p.default_style.size_half_points, 32);
    let s = &p.runs[0].style;
    assert!(!s.bold && !s.italic && s.underline);
    assert_eq!(s.font_family, "Noto Serif");
    assert_eq!(s.size_half_points, 28);
    assert_eq!(s.color, Color::rgb(0xAB, 0xCD, 0xEF));
    assert!(p.runs[1].style.italic);
    assert_eq!(p.runs[1].style.size_half_points, 28);
    assert!(report.document.paragraph(1).unwrap().runs[0].style.bold);
    roundtrip(&report.document);
}
#[test]
fn model_roundtrip_empty_paragraphs_all_alignments_and_breaks() {
    let mut doc = Document::default();
    doc.blocks.clear();
    for alignment in [
        Alignment::Left,
        Alignment::Center,
        Alignment::Right,
        Alignment::Justify,
    ] {
        let mut p = Paragraph::plain(" <&> \"quoted\" \t café 😀 ");
        p.style.alignment = alignment;
        p.default_style = TextStyle {
            font_family: "Empty & Font".into(),
            bold: true,
            ..TextStyle::default()
        };
        p.runs.push(Run::new(
            "second",
            TextStyle {
                italic: true,
                size_half_points: 36,
                ..TextStyle::default()
            },
        ));
        doc.blocks
            .extend([Block::Paragraph(p), Block::PageBreak, Block::PageBreak]);
    }
    doc.blocks.push(Block::Paragraph(Paragraph {
        default_style: TextStyle {
            italic: true,
            ..TextStyle::default()
        },
        ..Paragraph::default()
    }));
    roundtrip(&doc);
    doc.page_layout = PageLayout {
        size: PageSize::LETTER,
        orientation: Orientation::Portrait,
        margins: Margins::default(),
    };
    roundtrip(&doc);
}
#[test]
fn namespaces_are_uris_not_prefixes() {
    let doc = include_str!("fixtures/styled.xml");
    let alt = doc
        .replace("q:", "alternate:")
        .replace("xmlns:q", "xmlns:alternate");
    assert_eq!(import(doc).document, import(&alt).document);
    let strict = alt.replace(W, "http://purl.oclc.org/ooxml/wordprocessingml/main");
    assert_eq!(import(doc).document, import(&strict).document);
    let foreign = format!(
        "<w:document xmlns:w='{W}' xmlns:z='urn:foreign'><w:body><w:p><z:r><z:t>fake</z:t></z:r><w:r><w:t>real</w:t></w:r></w:p></w:body></w:document>"
    );
    let result = import(&foreign);
    assert_eq!(result.document.paragraph(0).unwrap().text(), "real");
    assert!(requires_converted_copy(&result));
}
#[test]
fn unsupported_content_requires_copy_and_is_not_preserved() {
    let report = import(include_str!("fixtures/unsupported.xml"));
    assert!(requires_converted_copy(&report));
    for feature in [
        Feature::Tables,
        Feature::Images,
        Feature::Lists,
        Feature::Sections,
        Feature::Fields,
        Feature::References,
        Feature::Reviewing,
        Feature::EmbeddedObjects,
        Feature::HeadersFooters,
    ] {
        assert!(
            report.warnings.iter().any(|w| w.feature == feature),
            "missing {feature:?}"
        );
    }
    assert_eq!(
        report.document.paragraph(0).unwrap().text(),
        "Visiblelinked textcachedinserted"
    );
    let converted = roundtrip(&report.document);
    assert!(!requires_converted_copy(&converted));
}
#[test]
fn malformed_archives_missing_parts_and_bad_xml() {
    assert!(import_docx(Cursor::new(b"not a ZIP")).is_err());
    let valid = zip(&parts(include_str!("fixtures/styled.xml")));
    assert!(import_docx(Cursor::new(&valid[..valid.len() / 2])).is_err());
    for missing in ["[Content_Types].xml", "_rels/.rels", "word/document.xml"] {
        let p = parts(include_str!("fixtures/styled.xml"));
        let filtered: Vec<_> = p.into_iter().filter(|(n, _)| *n != missing).collect();
        let err = import_docx(Cursor::new(zip(&filtered)))
            .unwrap_err()
            .to_string();
        assert!(err.contains(missing), "{err}");
    }
    for xml in [
        "<w:document>",
        "<a><b></a>",
        "<a/><a/>",
        "<!DOCTYPE a [<!ENTITY x 'boom'>]><a>&x;</a>",
        "<a>&unknown;</a>",
        "<a x='1' x='2'/>",
    ] {
        assert!(import_docx(Cursor::new(zip(&parts(xml)))).is_err(), "{xml}");
    }
}
#[test]
fn styles_cycles_missing_styles_and_missing_relationship_parts() {
    let mut p = parts(include_str!("fixtures/inherited.xml"));
    p.push(("word/_rels/document.xml.rels", STYLE_REL.as_bytes()));
    assert!(
        import_docx(Cursor::new(zip(&p)))
            .unwrap_err()
            .to_string()
            .contains("word/styles.xml")
    );
    let cycle = format!(
        "<w:styles xmlns:w='{W}'><w:style w:type='paragraph' w:styleId='A'><w:basedOn w:val='B'/></w:style><w:style w:type='paragraph' w:styleId='B'><w:basedOn w:val='A'/></w:style></w:styles>"
    );
    p.push(("word/styles.xml", cycle.as_bytes()));
    assert!(
        import_docx(Cursor::new(zip(&p)))
            .unwrap_err()
            .to_string()
            .contains("cyclic")
    );
    let missing = import(include_str!("fixtures/inherited.xml"));
    assert!(requires_converted_copy(&missing));
}
#[test]
fn unsafe_paths_external_main_and_non_docx_content_types() {
    let mut p = parts(include_str!("fixtures/styled.xml"));
    p.push(("../escape", b"data"));
    assert!(
        import_docx(Cursor::new(zip(&p)))
            .unwrap_err()
            .to_string()
            .contains("unsafe")
    );
    let rel = REL.replace("Target=", "TargetMode='External' Target=");
    let mut p = parts(include_str!("fixtures/styled.xml"));
    p[1].1 = rel.as_bytes();
    assert!(
        import_docx(Cursor::new(zip(&p)))
            .unwrap_err()
            .to_string()
            .contains("external")
    );
    let ct = CT.replace(
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
        "application/vnd.ms-word.document.macroEnabled.main+xml",
    );
    p[1].1 = REL.as_bytes();
    p[0].1 = ct.as_bytes();
    assert!(matches!(
        import_docx(Cursor::new(zip(&p))),
        Err(DocxError::UnsupportedPackage(_))
    ));
    let ole = [0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1];
    assert!(
        import_docx(Cursor::new(ole))
            .unwrap_err()
            .to_string()
            .contains("encrypted")
    );
}
#[test]
fn xml_and_archive_expansion_are_bounded() {
    let deep = format!("{}{}", "<a>".repeat(65), "</a>".repeat(65));
    assert!(matches!(
        import_docx(Cursor::new(zip(&parts(&deep)))),
        Err(DocxError::ResourceLimit(_))
    ));
    let huge = vec![b'a'; 8 * 1024 * 1024 + 1];
    let mut p = parts(include_str!("fixtures/styled.xml"));
    p.push(("word/big.bin", &huge));
    assert!(matches!(
        import_docx(Cursor::new(zip(&p))),
        Err(DocxError::ResourceLimit(_))
    ));
    let events = format!("<a>{}</a>", "<b/>".repeat(200_001));
    assert!(matches!(
        import_docx(Cursor::new(zip(&parts(&events)))),
        Err(DocxError::ResourceLimit(_))
    ));
}
#[test]
fn export_rejects_forbidden_xml_before_writing() {
    let doc = Document {
        blocks: vec![Block::Paragraph(Paragraph::plain("bad\0text"))],
        ..Document::default()
    };
    let mut out = Cursor::new(Vec::new());
    assert!(export_docx(&doc, &mut out).is_err());
    assert!(out.into_inner().is_empty());
}
#[test]
fn page_break_before_and_line_spacing_approximation_are_explicit() {
    let doc = format!(
        "<w:document xmlns:w='{W}'><w:body><w:p/><w:p><w:pPr><w:pageBreakBefore/><w:spacing w:line='241'/></w:pPr><w:r><w:t>B</w:t></w:r></w:p></w:body></w:document>"
    );
    let report = import(&doc);
    assert!(requires_converted_copy(&report));
    assert_eq!(report.document.blocks[1], Block::PageBreak);
    let mut document = Document::default();
    if let Block::Paragraph(p) = &mut document.blocks[0] {
        p.style.line_spacing = LineSpacing::Multiple(101);
    }
    let mut out = Cursor::new(Vec::new());
    assert!(
        !export_docx(&document, &mut out)
            .unwrap()
            .warnings
            .is_empty()
    );
    import_docx(Cursor::new(out.into_inner())).unwrap();
}
#[test]
fn encrypted_zip_and_duplicate_entries_fail_explicitly() {
    let mut encrypted = zip(&parts(include_str!("fixtures/styled.xml")));
    let central = encrypted
        .windows(4)
        .position(|w| w == b"PK\x01\x02")
        .unwrap();
    encrypted[central + 8] |= 1;
    assert!(matches!(
        import_docx(Cursor::new(encrypted)),
        Err(DocxError::EncryptedPackage)
    ));
    let mut duplicate = zip(&[("one.xml", b"a"), ("two.xml", b"b")]);
    for i in 0..duplicate.len() - 7 {
        if &duplicate[i..i + 7] == b"two.xml" {
            duplicate[i..i + 7].copy_from_slice(b"one.xml");
        }
    }
    assert!(
        import_docx(Cursor::new(duplicate))
            .unwrap_err()
            .to_string()
            .contains("duplicate")
    );
}
#[test]
fn main_and_style_parts_are_resolved_by_relationship() {
    let doc = include_str!("fixtures/styled.xml");
    let ct = CT.replace("/word/document.xml", "/custom/main.xml");
    let rel = REL.replace("word/document.xml", "custom/main.xml");
    let pkg = zip(&[
        ("[Content_Types].xml", ct.as_bytes()),
        ("_rels/.rels", rel.as_bytes()),
        ("custom/main.xml", doc.as_bytes()),
    ]);
    assert_eq!(
        import_docx(Cursor::new(pkg)).unwrap().document,
        import(doc).document
    );
    let style_rel = STYLE_REL.replace("styles.xml", "../custom/formats.xml");
    let mut pkg = parts(include_str!("fixtures/inherited.xml"));
    pkg.extend([
        ("word/_rels/document.xml.rels", style_rel.as_bytes()),
        (
            "custom/formats.xml",
            include_bytes!("fixtures/styles.xml").as_slice(),
        ),
    ]);
    assert!(!requires_converted_copy(
        &import_docx(Cursor::new(zip(&pkg))).unwrap()
    ));
}
#[test]
fn external_relationships_and_unknown_parts_warn_without_fetching() {
    let rel = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="x" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://invalid.example/" TargetMode="External"/></Relationships>"#;
    let mut pkg = parts(include_str!("fixtures/styled.xml"));
    pkg.extend([
        ("word/_rels/document.xml.rels", rel.as_bytes()),
        ("word/unrepresented.xml", b"<anything/>".as_slice()),
    ]);
    let report = import_docx(Cursor::new(zip(&pkg))).unwrap();
    assert!(requires_converted_copy(&report));
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.message.contains("External"))
    );
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.location.as_deref() == Some("word/unrepresented.xml"))
    );
}
#[test]
fn invalid_values_and_invalid_numeric_entities_are_rejected() {
    for fragment in [
        "<w:rPr><w:sz w:val='0'/></w:rPr>",
        "<w:rPr><w:b w:val='perhaps'/></w:rPr>",
        "<w:rPr><w:color w:val='red'/></w:rPr>",
        "<w:t>&#0;</w:t>",
    ] {
        let xml = format!(
            "<w:document xmlns:w='{W}'><w:body><w:p><w:r>{fragment}</w:r></w:p></w:body></w:document>"
        );
        assert!(
            import_docx(Cursor::new(zip(&parts(&xml)))).is_err(),
            "{fragment}"
        );
    }
}
#[test]
fn central_directory_entry_and_total_expansion_limits() {
    let data = b"<a/>";
    let names: Vec<String> = (0..1025).map(|i| format!("{i}.xml")).collect();
    let p: Vec<_> = names
        .iter()
        .map(|n| (n.as_str(), data.as_slice()))
        .collect();
    assert!(matches!(
        import_docx(Cursor::new(zip(&p))),
        Err(DocxError::ResourceLimit(_))
    ));
    let huge = vec![b'a'; 8 * 1024 * 1024];
    let names: Vec<String> = (0..9).map(|i| format!("{i}.bin")).collect();
    let p: Vec<_> = names
        .iter()
        .map(|n| (n.as_str(), huge.as_slice()))
        .collect();
    assert!(matches!(
        import_docx(Cursor::new(zip(&p))),
        Err(DocxError::ResourceLimit(_))
    ));
}
#[test]
fn inherited_page_break_before_and_direct_clear() {
    let styles = format!(
        "<w:styles xmlns:w='{W}'><w:style w:type='paragraph' w:styleId='Break'><w:pPr><w:pageBreakBefore/></w:pPr></w:style></w:styles>"
    );
    let document = format!(
        "<w:document xmlns:w='{W}'><w:body><w:p/><w:p><w:pPr><w:pStyle w:val='Break'/></w:pPr><w:r><w:t>B</w:t></w:r></w:p><w:p><w:pPr><w:pStyle w:val='Break'/><w:pageBreakBefore w:val='false'/></w:pPr></w:p></w:body></w:document>"
    );
    let mut p = parts(&document);
    p.extend([
        ("word/_rels/document.xml.rels", STYLE_REL.as_bytes()),
        ("word/styles.xml", styles.as_bytes()),
    ]);
    let report = import_docx(Cursor::new(zip(&p))).unwrap();
    assert!(report.warnings.is_empty());
    assert_eq!(report.document.blocks.len(), 4);
    assert_eq!(report.document.blocks[1], Block::PageBreak);
    roundtrip(&report.document);
}
#[test]
fn inherited_spacing_attributes_resolve_independently() {
    let styles = format!(
        "<w:styles xmlns:w='{W}'><w:style w:type='paragraph' w:styleId='Base'><w:pPr><w:spacing w:line='400' w:lineRule='exact'/></w:pPr></w:style><w:style w:type='paragraph' w:styleId='Child'><w:basedOn w:val='Base'/><w:pPr><w:spacing w:line='500'/></w:pPr></w:style></w:styles>"
    );
    let document = format!(
        "<w:document xmlns:w='{W}'><w:body><w:p><w:pPr><w:pStyle w:val='Child'/></w:pPr></w:p><w:p><w:pPr><w:pStyle w:val='Child'/><w:spacing w:lineRule='atLeast'/></w:pPr></w:p></w:body></w:document>"
    );
    let mut p = parts(&document);
    p.extend([
        ("word/_rels/document.xml.rels", STYLE_REL.as_bytes()),
        ("word/styles.xml", styles.as_bytes()),
    ]);
    let report = import_docx(Cursor::new(zip(&p))).unwrap();
    assert!(report.warnings.is_empty());
    assert_eq!(
        report.document.paragraph(0).unwrap().style.line_spacing,
        LineSpacing::Exact(500)
    );
    assert_eq!(
        report.document.paragraph(1).unwrap().style.line_spacing,
        LineSpacing::AtLeast(500)
    );
    roundtrip(&report.document);
}
#[test]
fn namespace_processing_and_attribute_allocation_are_bounded() {
    let attrs = (0..129).map(|i| format!("a{i}='v' ")).collect::<String>();
    let document = format!("<a {attrs}/>");
    assert!(matches!(
        import_docx(Cursor::new(zip(&parts(&document)))),
        Err(DocxError::ResourceLimit(_))
    ));
    let document = format!("<a xmlns='{}'><b/></a>", "x".repeat(257));
    assert!(matches!(
        import_docx(Cursor::new(zip(&parts(&document)))),
        Err(DocxError::ResourceLimit(_))
    ));
}
#[test]
fn exported_package_has_required_parts_relationships_and_oriented_size() {
    use std::io::Read;
    let doc = Document {
        page_layout: PageLayout {
            orientation: Orientation::Landscape,
            size: PageSize::LETTER,
            ..PageLayout::default()
        },
        ..Document::default()
    };
    let mut out = Cursor::new(Vec::new());
    export_docx(&doc, &mut out).unwrap();
    let mut archive = zip::ZipArchive::new(Cursor::new(out.into_inner())).unwrap();
    assert_eq!(archive.len(), 3);
    let mut ct = String::new();
    archive
        .by_name("[Content_Types].xml")
        .unwrap()
        .read_to_string(&mut ct)
        .unwrap();
    assert!(ct.contains("Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\""));
    assert!(ct.contains("PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\""));
    let mut rel = String::new();
    archive
        .by_name("_rels/.rels")
        .unwrap()
        .read_to_string(&mut rel)
        .unwrap();
    assert!(rel.contains("Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\""));
    assert!(rel.contains("Target=\"word/document.xml\""));
    let mut document = String::new();
    archive
        .by_name("word/document.xml")
        .unwrap()
        .read_to_string(&mut document)
        .unwrap();
    assert!(document.contains("w:w=\"15840\" w:h=\"12240\" w:orient=\"landscape\""));
}
#[test]
fn imported_xml_rejects_forbidden_controls_in_text_and_attributes() {
    // quick-xml tokenization is not sufficient XML 1.0 character validation.
    // Check literal controls before parsing and numeric entities after decoding.
    for control in ['\0', '\u{1}', '\u{b}'] {
        for fragment in [
            format!("<w:t>before{control}after</w:t>"),
            format!("<w:rPr><w:rFonts w:ascii='Font{control}Name'/></w:rPr>"),
            format!("<w:t>before&#{};after</w:t>", control as u32),
            format!(
                "<w:rPr><w:rFonts w:ascii='Font&#{};Name'/></w:rPr>",
                control as u32
            ),
        ] {
            let xml = format!(
                "<w:document xmlns:w='{W}'><w:body><w:p><w:r>{fragment}</w:r></w:p></w:body></w:document>"
            );
            let error = import_docx(Cursor::new(zip(&parts(&xml)))).unwrap_err();
            assert!(matches!(error, DocxError::InvalidPackage(_)), "{error}");
            let message = error.to_string();
            assert!(message.contains("word/document.xml"), "{message}");
            assert!(
                message.contains("forbidden XML 1.0 control characters")
                    || message.contains("not permitted in XML"),
                "{message}"
            );
            assert!(
                message.contains("remove") && message.contains("resave"),
                "{message}"
            );
        }
    }
    // TAB is legal XML 1.0 and part of the supported core text model.
    let xml = format!(
        "<w:document xmlns:w='{W}'><w:body><w:p><w:r><w:t>A&#9;B</w:t></w:r></w:p></w:body></w:document>"
    );
    assert_eq!(import(&xml).document.paragraph(0).unwrap().text(), "A\tB");
}
