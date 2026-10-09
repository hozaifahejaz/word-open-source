use document_core::*;

#[test]
fn recovered_document_restores_valid_selection_and_is_dirty() {
    let mut editor = Editor::default();
    editor
        .execute(Command::InsertText {
            at: Position::new(0, 0),
            text: "old".into(),
            style: None,
        })
        .unwrap();
    editor.execute(Command::Undo).unwrap();
    assert!(editor.can_redo());
    let document = Document {
        blocks: vec![Block::Paragraph(Paragraph {
            runs: vec![
                Run::new("", TextStyle::default()),
                Run::new("re", TextStyle::default()),
                Run::new("covered", TextStyle::default()),
            ],
            ..Paragraph::default()
        })],
        ..Document::default()
    };
    let selection = Selection::new(Position::new(0, 9), Position::new(0, 2));

    editor.load_recovered_document(document, selection).unwrap();

    assert_eq!(
        editor.document().paragraph(0).unwrap(),
        &Paragraph::plain("recovered")
    );
    assert_eq!(editor.selection(), selection);
    assert!(editor.is_dirty());
    assert!(!editor.can_undo());
    assert!(!editor.can_redo());
    assert!(!editor.execute(Command::Undo).unwrap().changed);
    assert!(!editor.execute(Command::Redo).unwrap().changed);
    assert!(editor.is_dirty());
    editor
        .execute(Command::InsertText {
            at: Position::new(0, 9),
            text: "!".into(),
            style: None,
        })
        .unwrap();
    editor.execute(Command::Undo).unwrap();
    assert_eq!(editor.document().paragraph(0).unwrap().text(), "recovered");
    assert_eq!(editor.selection(), selection);
    assert!(editor.is_dirty());
    editor.execute(Command::Redo).unwrap();
    assert_eq!(editor.document().paragraph(0).unwrap().text(), "recovered!");
    assert!(editor.is_dirty());
}

#[test]
fn recovered_document_rejects_invalid_document() {
    let mut editor = Editor::default();
    editor
        .execute(Command::InsertText {
            at: Position::new(0, 0),
            text: "existing".into(),
            style: None,
        })
        .unwrap();
    let before = editor.document().clone();
    let selection = editor.selection();
    let invalid = Document {
        blocks: vec![],
        ..Document::default()
    };

    assert_eq!(
        editor.load_recovered_document(invalid, Selection::default()),
        Err(CoreError::InvalidStructure)
    );
    assert_eq!(editor.document(), &before);
    assert_eq!(editor.selection(), selection);
    assert!(editor.is_dirty());
    assert!(editor.can_undo());
    assert!(!editor.can_redo());
    editor.execute(Command::Undo).unwrap();
    assert!(!editor.is_dirty());
}

#[test]
fn recovered_document_falls_back_from_invalid_selection() {
    let document = Document {
        blocks: vec![
            Block::Paragraph(Paragraph::plain("e\u{301}x")),
            Block::PageBreak,
            Block::Paragraph(Paragraph::plain("last")),
        ],
        ..Document::default()
    };
    for selection in [
        Selection::new(Position::new(0, 1), Position::new(2, 4)),
        Selection::new(Position::new(0, 0), Position::new(9, 0)),
        Selection::caret(Position::new(0, 99)),
        Selection::caret(Position::new(1, 0)),
    ] {
        let mut editor = Editor::default();
        editor
            .load_recovered_document(document.clone(), selection)
            .unwrap();
        assert_eq!(editor.selection(), Selection::caret(Position::new(0, 0)));
        assert_eq!(editor.document(), &document);
        assert!(editor.is_dirty());
        assert!(!editor.can_undo());
    }
}

#[test]
fn mark_saved_clears_recovered_dirty_state() {
    let mut editor = Editor::default();
    editor.set_history_limit(0);
    editor
        .load_recovered_document(Document::default(), Selection::default())
        .unwrap();
    assert!(editor.is_dirty());

    editor.mark_saved();

    assert!(!editor.is_dirty());
    editor
        .execute(Command::InsertText {
            at: Position::new(0, 0),
            text: "new".into(),
            style: None,
        })
        .unwrap();
    assert!(editor.is_dirty());
    assert!(!editor.can_undo(), "recovery preserves the history limit");
    editor
        .load_recovered_document(Document::default(), Selection::default())
        .unwrap();
    assert!(editor.is_dirty());
    editor.load_document(Document::default()).unwrap();
    assert!(
        !editor.is_dirty(),
        "normal document loading clears recovery state"
    );
    assert!(!editor.can_undo());
    assert!(!editor.can_redo());
}

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
