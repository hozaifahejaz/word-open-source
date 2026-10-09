use document_core::*;

#[test]
fn native_document_json_round_trips_every_model_variant() {
    let styled = TextStyle {
        bold: true,
        italic: true,
        underline: true,
        font_family: "Example Serif".into(),
        size_half_points: 27,
        color: Color::rgb(12, 34, 56),
    };
    let document = Document {
        blocks: vec![
            Block::Paragraph(Paragraph {
                runs: vec![Run::new("styled text", styled)],
                style: ParagraphStyle {
                    alignment: Alignment::Justify,
                    space_before_twips: 120,
                    space_after_twips: 240,
                    line_spacing: LineSpacing::AtLeast(300),
                },
                ..Paragraph::default()
            }),
            Block::Paragraph(Paragraph::default()),
            Block::PageBreak,
            Block::Paragraph(Paragraph {
                style: ParagraphStyle {
                    line_spacing: LineSpacing::Exact(360),
                    ..ParagraphStyle::default()
                },
                ..Paragraph::default()
            }),
        ],
        page_layout: PageLayout {
            size: PageSize::LETTER,
            orientation: Orientation::Landscape,
            margins: Margins::default(),
        },
    };
    let warnings = vec![
        WarningCode::UnsupportedFeature,
        WarningCode::ApproximatedFormatting,
        WarningCode::MissingPart,
        WarningCode::InvalidValue,
        WarningCode::ResourceLimit,
    ]
    .into_iter()
    .zip([
        Feature::Tables,
        Feature::Images,
        Feature::Styles,
        Feature::Lists,
        Feature::Sections,
    ])
    .map(|(code, feature)| ImportWarning {
        code,
        feature,
        location: Some("word/document.xml".into()),
        message: "recovery warning".into(),
    })
    .chain(
        [
            Feature::HeadersFooters,
            Feature::Fields,
            Feature::References,
            Feature::Reviewing,
            Feature::EmbeddedObjects,
            Feature::Other("custom feature".into()),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, feature)| ImportWarning {
            code: WarningCode::UnsupportedFeature,
            feature,
            location: (index % 2 == 0).then(|| "word/document.xml".into()),
            message: "additional feature warning".into(),
        }),
    )
    .collect::<Vec<_>>();

    let json = serde_json::to_string(&document).unwrap();
    let decoded: Document = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, document);
    decoded.validate().unwrap();

    let json = serde_json::to_string(&warnings).unwrap();
    let decoded: Vec<ImportWarning> = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, warnings);

    let position = Position::new(3, 12);
    let json = serde_json::to_string(&position).unwrap();
    assert_eq!(serde_json::from_str::<Position>(&json).unwrap(), position);
    let selection = Selection::new(Position::new(0, 2), position);
    let json = serde_json::to_string(&selection).unwrap();
    assert_eq!(serde_json::from_str::<Selection>(&json).unwrap(), selection);
}
