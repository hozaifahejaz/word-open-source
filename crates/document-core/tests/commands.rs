use document_core::*;

fn at(block: usize, offset: usize) -> Position {
    Position::new(block, offset)
}
fn range(a: (usize, usize), b: (usize, usize)) -> Selection {
    Selection::new(at(a.0, a.1), at(b.0, b.1))
}
fn editor(paragraphs: &[&str]) -> Editor {
    Editor::new(Document {
        blocks: paragraphs
            .iter()
            .map(|s| Block::Paragraph(Paragraph::plain(*s)))
            .collect(),
        ..Document::default()
    })
    .unwrap()
}
fn text(e: &Editor, block: usize) -> String {
    e.document().paragraph(block).unwrap().text()
}
fn insert(e: &mut Editor, pos: Position, s: &str) -> EditOutcome {
    e.execute(Command::InsertText {
        at: pos,
        text: s.into(),
        style: None,
    })
    .unwrap()
}

#[test]
fn page_break_replacement_rejects_invalid_range_atomically() {
    let mut e = editor(&["e\u{301} text"]);
    let original = e.document().clone();
    let selection = e.selection();
    assert_eq!(
        e.execute(Command::ReplaceWithPageBreak {
            selection: range((0, 1), (0, 3))
        }),
        Err(CoreError::InvalidPosition)
    );
    assert_eq!(e.document(), &original);
    assert_eq!(e.selection(), selection);
    assert!(!e.can_undo());
    assert!(!e.is_dirty());
}

#[test]
fn page_break_replacement_keeps_surviving_styles_and_undo_selection() {
    let mut e = editor(&["left", "right"]);
    e.execute(Command::FormatRuns {
        selection: range((1, 0), (1, 5)),
        patch: StylePatch {
            italic: Some(true),
            ..Default::default()
        },
    })
    .unwrap();
    e.mark_saved();
    let original = e.document().clone();
    let selection = range((1, 2), (0, 2));
    e.set_selection(selection).unwrap();
    e.execute(Command::ReplaceWithPageBreak { selection })
        .unwrap();
    assert_eq!(text(&e, 0), "le");
    assert_eq!(text(&e, 2), "ght");
    assert!(e.document().paragraph(2).unwrap().runs[0].style.italic);
    assert_eq!(e.selection(), Selection::caret(at(2, 0)));
    e.execute(Command::Undo).unwrap();
    assert_eq!(e.document(), &original);
    assert_eq!(e.selection(), selection);
    assert!(!e.is_dirty());
}

#[test]
fn cross_run_replacement_preserves_surviving_styles() {
    let bold = TextStyle {
        bold: true,
        ..TextStyle::default()
    };
    let italic = TextStyle {
        italic: true,
        ..TextStyle::default()
    };
    let p = Paragraph {
        runs: vec![
            Run::new("ab", bold.clone()),
            Run::new("cde", italic.clone()),
        ],
        ..Paragraph::default()
    };
    let mut e = Editor::new(Document {
        blocks: vec![Block::Paragraph(p)],
        ..Document::default()
    })
    .unwrap();
    e.execute(Command::ReplaceText {
        selection: range((0, 1), (0, 4)),
        text: "X".into(),
        style: None,
    })
    .unwrap();
    let p = e.document().paragraph(0).unwrap();
    assert_eq!(p.text(), "aXe");
    assert_eq!(p.runs, vec![Run::new("aX", bold), Run::new("e", italic)]);
    assert_eq!(e.selection(), Selection::caret(at(0, 2)));
}

#[test]
fn unicode_boundaries_reject_split_utf8_combining_and_zwj_sequences() {
    let mut e = editor(&["éa\u{301}👩‍💻Z"]);
    let original = e.document().clone();
    for offset in [1, 3, 6, 9] {
        assert_eq!(
            e.execute(Command::InsertText {
                at: at(0, offset),
                text: "x".into(),
                style: None
            }),
            Err(CoreError::InvalidPosition)
        );
        assert_eq!(e.document(), &original);
        assert!(!e.is_dirty());
    }
    e.execute(Command::Delete {
        selection: range((0, 2), (0, 5)),
    })
    .unwrap();
    assert_eq!(text(&e, 0), "é👩‍💻Z");
    assert_eq!(
        e.document().paragraph(0).unwrap().byte_from_char_index(1),
        Ok(2)
    );
    assert_eq!(
        e.document().paragraph(0).unwrap().byte_from_char_index(3),
        Err(CoreError::InvalidPosition)
    );
}

#[test]
fn insertion_and_deletion_snap_caret_after_new_grapheme_merges() {
    let mut e = editor(&["aZ"]);
    let result = insert(&mut e, at(0, 1), "\u{301}");
    assert_eq!(text(&e, 0), "a\u{301}Z");
    assert_eq!(result.selection.focus, at(0, 3));
    let mut e = editor(&["aX\u{301}Z"]);
    e.execute(Command::Delete {
        selection: range((0, 1), (0, 4)),
    })
    .unwrap();
    assert_eq!(text(&e, 0), "aZ");
    e.document().validate_selection(e.selection()).unwrap();
}

#[test]
fn multiline_insert_split_and_join_keep_paragraph_properties() {
    let mut e = editor(&["hello"]);
    e.execute(Command::FormatParagraphs {
        selection: Selection::caret(at(0, 0)),
        patch: ParagraphPatch {
            alignment: Some(Alignment::Center),
            ..ParagraphPatch::default()
        },
    })
    .unwrap();
    insert(&mut e, at(0, 2), "A\r\nB\rC");
    assert_eq!(
        (text(&e, 0), text(&e, 1), text(&e, 2)),
        ("heA".into(), "B".into(), "Cllo".into())
    );
    assert_eq!(
        e.document().paragraph(2).unwrap().style.alignment,
        Alignment::Center
    );
    e.execute(Command::SplitParagraph { at: at(2, 1) }).unwrap();
    assert_eq!(text(&e, 3), "llo");
    e.execute(Command::JoinParagraph { block: 2 }).unwrap();
    assert_eq!(text(&e, 2), "Cllo");
    assert_eq!(e.selection(), Selection::caret(at(2, 1)));
}

#[test]
fn reverse_cross_paragraph_delete_joins_and_preserves_start_properties() {
    let mut e = editor(&["one", "two", "three"]);
    e.execute(Command::FormatParagraphs {
        selection: Selection::caret(at(0, 0)),
        patch: ParagraphPatch {
            alignment: Some(Alignment::Right),
            ..ParagraphPatch::default()
        },
    })
    .unwrap();
    e.execute(Command::Delete {
        selection: range((2, 2), (0, 1)),
    })
    .unwrap();
    assert_eq!(text(&e, 0), "oree");
    assert_eq!(e.document().blocks.len(), 1);
    assert_eq!(
        e.document().paragraph(0).unwrap().style.alignment,
        Alignment::Right
    );
}

#[test]
fn range_formatting_splits_runs_and_does_not_touch_outside_text() {
    let mut e = editor(&["abcdef"]);
    let patch = StylePatch {
        bold: Some(true),
        italic: Some(true),
        underline: Some(true),
        font_family: Some("Example Serif".into()),
        size_half_points: Some(27),
        color: Some(Color::rgb(5, 6, 7)),
        ..Default::default()
    };
    e.execute(Command::FormatRuns {
        selection: range((0, 2), (0, 4)),
        patch,
    })
    .unwrap();
    let runs = &e.document().paragraph(0).unwrap().runs;
    assert_eq!(
        runs.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(),
        vec!["ab", "cd", "ef"]
    );
    assert!(runs[1].style.bold && runs[1].style.italic && runs[1].style.underline);
    assert_eq!(runs[1].style.color, Color::rgb(5, 6, 7));
    assert_eq!(runs[1].style.size_half_points, 27);
    assert!(!runs[0].style.bold && !runs[2].style.bold);
    e.execute(Command::FormatRuns {
        selection: range((0, 2), (0, 4)),
        patch: StylePatch {
            bold: Some(false),
            ..StylePatch::default()
        },
    })
    .unwrap();
    assert!(!e.document().paragraph(0).unwrap().runs[1].style.bold);
    assert!(e.document().paragraph(0).unwrap().runs[1].style.italic);
}

#[test]
fn paragraph_range_uses_exclusive_end_and_preserves_empty_paragraph_style() {
    let mut e = editor(&["x", "", "y"]);
    e.execute(Command::FormatParagraphs {
        selection: range((0, 0), (2, 0)),
        patch: ParagraphPatch {
            alignment: Some(Alignment::Justify),
            space_before_twips: Some(120),
            space_after_twips: Some(240),
            line_spacing: Some(LineSpacing::Multiple(150)),
        },
    })
    .unwrap();
    assert_eq!(
        e.document().paragraph(0).unwrap().style.line_spacing,
        LineSpacing::Multiple(150)
    );
    assert_eq!(
        e.document().paragraph(1).unwrap().style.space_after_twips,
        240
    );
    assert_eq!(
        e.document().paragraph(2).unwrap().style.alignment,
        Alignment::Left
    );
}

#[test]
fn undo_redo_restores_selection_and_tracks_saved_content() {
    let mut e = editor(&["a"]);
    e.set_selection(Selection::caret(at(0, 1))).unwrap();
    insert(&mut e, at(0, 1), "b");
    assert!(e.is_dirty() && e.can_undo());
    e.mark_saved();
    assert!(!e.is_dirty());
    e.execute(Command::Undo).unwrap();
    assert_eq!(text(&e, 0), "a");
    assert_eq!(e.selection(), Selection::caret(at(0, 1)));
    assert!(e.is_dirty() && e.can_redo());
    e.execute(Command::Redo).unwrap();
    assert_eq!(text(&e, 0), "ab");
    assert!(!e.is_dirty());
    e.execute(Command::Undo).unwrap();
    insert(&mut e, at(0, 1), "c");
    assert!(!e.can_redo());
}

#[test]
fn noops_and_failed_commands_preserve_history() {
    let mut e = editor(&["a"]);
    insert(&mut e, at(0, 1), "b");
    e.execute(Command::Undo).unwrap();
    let result = insert(&mut e, at(0, 0), "");
    assert!(!result.changed);
    assert!(e.can_redo());
    assert_eq!(
        e.execute(Command::FormatRuns {
            selection: range((0, 0), (0, 1)),
            patch: StylePatch {
                size_half_points: Some(0),
                ..StylePatch::default()
            }
        }),
        Err(CoreError::InvalidStyle)
    );
    assert!(e.can_redo());
    assert_eq!(text(&e, 0), "a");
}

#[test]
fn page_break_is_structural_and_range_delete_removes_it() {
    let mut e = editor(&["abcd"]);
    e.execute(Command::InsertPageBreak { at: at(0, 2) })
        .unwrap();
    assert!(matches!(e.document().blocks[1], Block::PageBreak));
    assert_eq!(text(&e, 2), "cd");
    assert_eq!(e.selection(), Selection::caret(at(2, 0)));
    assert_eq!(
        e.execute(Command::JoinParagraph { block: 0 }),
        Err(CoreError::CannotJoin)
    );
    e.execute(Command::Delete {
        selection: range((0, 2), (2, 0)),
    })
    .unwrap();
    assert_eq!(text(&e, 0), "abcd");
    assert_eq!(e.document().blocks.len(), 1);
}

#[test]
fn literal_find_crosses_runs_but_respects_graphemes_and_paragraphs() {
    let p = Paragraph {
        runs: vec![
            Run::new("a.", TextStyle::default()),
            Run::new(
                "*a.*",
                TextStyle {
                    bold: true,
                    ..TextStyle::default()
                },
            ),
        ],
        ..Paragraph::default()
    };
    let d = Document {
        blocks: vec![
            Block::Paragraph(p),
            Block::Paragraph(Paragraph::plain("éa\u{301}")),
        ],
        ..Document::default()
    };
    assert_eq!(
        d.find(".*").unwrap(),
        vec![range((0, 1), (0, 3)), range((0, 4), (0, 6))]
    );
    assert!(d.find("a").unwrap().iter().all(|s| s.anchor.block == 0));
    assert!(d.find("\n").unwrap().is_empty());
    assert_eq!(d.find(""), Err(CoreError::EmptySearch));
}

#[test]
fn replace_all_is_single_history_step_and_does_not_replace_inserted_matches() {
    let mut e = editor(&["aa aa", "aa"]);
    let outcome = e
        .execute(Command::ReplaceAll {
            needle: "aa".into(),
            replacement: "aaX\nY".into(),
        })
        .unwrap();
    assert_eq!(outcome.replacements, 3);
    assert_eq!(e.document().blocks.len(), 5);
    assert_eq!(text(&e, 0), "aaX");
    assert_eq!(text(&e, 1), "Y aaX");
    e.execute(Command::Undo).unwrap();
    assert_eq!(text(&e, 0), "aa aa");
    assert_eq!(text(&e, 1), "aa");
    assert!(!e.can_undo() && !e.is_dirty());
}

#[test]
fn layout_validation_and_changes_are_atomic_and_undoable() {
    let mut e = Editor::default();
    let layout = PageLayout {
        size: PageSize::LETTER,
        orientation: Orientation::Landscape,
        ..PageLayout::default()
    };
    e.execute(Command::SetPageLayout {
        layout: layout.clone(),
    })
    .unwrap();
    assert_eq!(e.document().page_layout.effective_size().width_twips, 15840);
    let invalid = PageLayout {
        margins: Margins {
            left: u32::MAX,
            ..Margins::default()
        },
        ..layout.clone()
    };
    assert_eq!(
        e.execute(Command::SetPageLayout { layout: invalid }),
        Err(CoreError::InvalidLayout)
    );
    assert_eq!(e.document().page_layout, layout);
    e.execute(Command::Undo).unwrap();
    assert_eq!(e.document().page_layout, PageLayout::default());
}

#[test]
fn validation_rejects_invalid_imports_and_preserves_empty_document() {
    let invalid = Document {
        blocks: vec![],
        ..Document::default()
    };
    assert_eq!(
        Editor::new(invalid).unwrap_err(),
        CoreError::InvalidStructure
    );
    let invalid = Document {
        blocks: vec![Block::Paragraph(Paragraph::plain("a\nb"))],
        ..Document::default()
    };
    assert_eq!(Editor::new(invalid).unwrap_err(), CoreError::InvalidText);
    let mut e = editor(&["x"]);
    e.execute(Command::Delete {
        selection: range((0, 0), (0, 1)),
    })
    .unwrap();
    assert_eq!(text(&e, 0), "");
    assert!(e.document().paragraph(0).unwrap().runs.is_empty());
    e.document().validate().unwrap();
}

#[test]
fn replace_all_handles_new_combining_clusters_between_original_matches() {
    let mut e = editor(&["aa"]);
    let result = e
        .execute(Command::ReplaceAll {
            needle: "a".into(),
            replacement: "\u{301}".into(),
        })
        .unwrap();
    assert_eq!(result.replacements, 2);
    assert_eq!(text(&e, 0), "\u{301}\u{301}");
    e.document().validate_selection(e.selection()).unwrap();
    e.execute(Command::Undo).unwrap();
    assert_eq!(text(&e, 0), "aa");
}

#[test]
fn history_limit_and_document_loading_reset_session_correctly() {
    let mut e = editor(&[""]);
    e.set_history_limit(1);
    insert(&mut e, at(0, 0), "a");
    insert(&mut e, at(0, 1), "b");
    e.execute(Command::Undo).unwrap();
    assert_eq!(text(&e, 0), "a");
    assert!(!e.can_undo());
    e.load_document(Document::default()).unwrap();
    assert!(!e.is_dirty() && !e.can_undo() && !e.can_redo());
    e.set_history_limit(0);
    insert(&mut e, at(0, 0), "z");
    assert!(e.is_dirty() && !e.can_undo());
    assert!(!e.execute(Command::Undo).unwrap().changed);
}

#[test]
fn invalid_import_or_replace_all_cannot_mutate_session() {
    let mut e = editor(&["abc abc"]);
    insert(&mut e, at(0, 0), "x");
    let original = e.document().clone();
    let selection = e.selection();
    assert!(
        e.load_document(Document {
            blocks: vec![Block::PageBreak],
            ..Document::default()
        })
        .is_err()
    );
    assert_eq!(e.document(), &original);
    assert!(e.can_undo());
    assert_eq!(
        e.execute(Command::ReplaceAll {
            needle: "abc".into(),
            replacement: "\u{000c}".into()
        }),
        Err(CoreError::InvalidText)
    );
    assert_eq!(e.document(), &original);
    assert_eq!(e.selection(), selection);
    assert_eq!(
        e.execute(Command::ReplaceAll {
            needle: "".into(),
            replacement: "z".into()
        }),
        Err(CoreError::EmptySearch)
    );
}

#[test]
fn formatting_crosses_paragraphs_and_breaks_without_changing_structure() {
    let mut e = editor(&["ab", "cd"]);
    e.execute(Command::InsertPageBreak { at: at(0, 2) })
        .unwrap();
    e.execute(Command::FormatRuns {
        selection: range((0, 1), (3, 1)),
        patch: StylePatch {
            underline: Some(true),
            ..StylePatch::default()
        },
    })
    .unwrap();
    assert_eq!(e.document().blocks.len(), 4);
    assert!(matches!(e.document().blocks[1], Block::PageBreak));
    let p = e.document().paragraph(0).unwrap();
    assert_eq!(p.runs[0].text, "a");
    assert!(!p.runs[0].style.underline);
    assert!(p.runs[1].style.underline);
    assert!(e.document().paragraph(2).unwrap().default_style.underline);
    assert_eq!(e.document().paragraph(3).unwrap().runs[0].text, "c");
    assert!(!e.document().paragraph(3).unwrap().runs[1].style.underline);
}

#[test]
fn explicit_insertion_style_and_empty_paragraph_typing_style_are_retained() {
    let mut e = Editor::default();
    let style = TextStyle {
        bold: true,
        font_family: "Example".into(),
        ..TextStyle::default()
    };
    e.execute(Command::InsertText {
        at: at(0, 0),
        text: "x".into(),
        style: Some(style.clone()),
    })
    .unwrap();
    e.execute(Command::SplitParagraph { at: at(0, 1) }).unwrap();
    assert_eq!(e.document().paragraph(1).unwrap().default_style, style);
    insert(&mut e, at(1, 0), "y");
    assert!(e.document().paragraph(1).unwrap().runs[0].style.bold);
    let result = e
        .execute(Command::FormatRuns {
            selection: Selection::caret(at(1, 0)),
            patch: StylePatch {
                italic: Some(true),
                ..StylePatch::default()
            },
        })
        .unwrap();
    assert!(!result.changed); // UI supplies pending typing style explicitly.
}

#[test]
fn navigation_uses_whole_extended_graphemes_across_runs() {
    let p = Paragraph {
        runs: vec![
            Run::new("a", TextStyle::default()),
            Run::new(
                "\u{301}👩‍💻",
                TextStyle {
                    bold: true,
                    ..TextStyle::default()
                },
            ),
        ],
        ..Paragraph::default()
    };
    assert_eq!(p.next_boundary(0), Ok(3));
    assert_eq!(p.next_boundary(3), Ok(p.len_bytes()));
    assert_eq!(p.previous_boundary(p.len_bytes()), Ok(3));
    assert_eq!(p.previous_boundary(3), Ok(0));
    assert_eq!(p.char_index_from_byte(3), Ok(2));
    assert_eq!(p.byte_from_char_index(1), Err(CoreError::InvalidPosition));
}

#[test]
fn search_options_preserve_literal_case_sensitive_compatibility() {
    let d = editor(&["Cat cat .* .*", "cat"]).document().clone();
    assert_eq!(
        d.find("cat").unwrap(),
        vec![range((0, 4), (0, 7)), range((1, 0), (1, 3))]
    );
    assert_eq!(
        d.find_with_options("cat", SearchOptions::default())
            .unwrap(),
        d.find("cat").unwrap()
    );
    assert_eq!(
        d.find_with_options(
            ".*",
            SearchOptions {
                match_case: false,
                whole_words: false
            }
        )
        .unwrap(),
        vec![range((0, 8), (0, 10)), range((0, 11), (0, 13))]
    );
    assert_eq!(
        d.find_with_options("", SearchOptions::default()),
        Err(CoreError::EmptySearch)
    );
}
#[test]
fn insensitive_search_maps_unicode_lowercase_expansion_to_original_ranges() {
    let d = editor(&["İ i\u{307} I i CAFÉ café"]).document().clone();
    let options = SearchOptions {
        match_case: false,
        whole_words: false,
    };
    assert_eq!(
        d.find_with_options("İ", options).unwrap(),
        vec![range((0, 0), (0, 2)), range((0, 3), (0, 6))]
    );
    // Partial lowercase expansions and combining graphemes cannot become selections.
    assert_eq!(
        d.find_with_options("i", options).unwrap(),
        vec![range((0, 7), (0, 8)), range((0, 9), (0, 10))]
    );
    assert!(d.find_with_options("\u{307}", options).unwrap().is_empty());
    assert_eq!(
        d.find_with_options("café", options).unwrap(),
        vec![range((0, 11), (0, 16)), range((0, 17), (0, 22))]
    );
}
#[test]
fn whole_word_search_uses_unicode_words_in_original_text() {
    let d = editor(&[
        "é élan aé é2 2é é\u{301} é-猫 猫 猫2",
        "can't can 123 123a 123_4",
    ])
    .document()
    .clone();
    let options = SearchOptions {
        match_case: false,
        whole_words: true,
    };
    assert_eq!(
        d.find_with_options("é", options).unwrap(),
        vec![range((0, 0), (0, 2)), range((0, 26), (0, 28))]
    );
    assert_eq!(
        d.find_with_options("é\u{301}", options).unwrap(),
        vec![range((0, 21), (0, 25))]
    );
    assert_eq!(
        d.find_with_options("can", options).unwrap(),
        vec![range((1, 6), (1, 9))]
    );
    assert_eq!(
        d.find_with_options("123", options).unwrap(),
        vec![range((1, 10), (1, 13))]
    );
}
#[test]
fn option_search_remains_grapheme_safe_and_cross_run_paragraph_local() {
    let d = Document {
        blocks: vec![
            Block::Paragraph(Paragraph {
                runs: vec![
                    Run::new("C", TextStyle::default()),
                    Run::new(
                        "AT E\u{301} 👩‍👩‍👧‍👦",
                        TextStyle {
                            bold: true,
                            ..Default::default()
                        },
                    ),
                ],
                ..Default::default()
            }),
            Block::PageBreak,
            Block::Paragraph(Paragraph::plain("CAT")),
        ],
        ..Default::default()
    };
    let options = SearchOptions {
        match_case: false,
        whole_words: false,
    };
    assert_eq!(
        d.find_with_options("cat", options).unwrap(),
        vec![range((0, 0), (0, 3)), range((2, 0), (2, 3))]
    );
    assert!(d.find_with_options("e", options).unwrap().is_empty());
    assert!(d.find_with_options("👩", options).unwrap().is_empty());
    assert!(d.find_with_options("cat\ncat", options).unwrap().is_empty());
}
#[test]
fn options_replace_all_is_atomic_and_one_undo_step_with_original_offsets() {
    let mut e = editor(&["İ i\u{307} İ2", "İ"]);
    let original = e.document().clone();
    let selection = range((1, 0), (1, 2));
    e.set_selection(selection).unwrap();
    let result = e
        .execute(Command::ReplaceAllWithOptions {
            needle: "İ".into(),
            replacement: "İX\nY".into(),
            options: SearchOptions {
                match_case: false,
                whole_words: true,
            },
        })
        .unwrap();
    assert_eq!(result.replacements, 3);
    assert_eq!(
        (0..5).map(|block| text(&e, block)).collect::<Vec<_>>(),
        vec!["İX", "Y İX", "Y İ2", "İX", "Y"]
    );
    e.execute(Command::Undo).unwrap();
    assert_eq!(e.document(), &original);
    assert_eq!(e.selection(), selection);
    assert!(!e.can_undo() && !e.is_dirty());
    assert!(e.can_redo());
    for needle in ["", "İ"] {
        let replacement = if needle.is_empty() {
            "valid"
        } else {
            "bad\u{000c}text"
        };
        assert_eq!(
            e.execute(Command::ReplaceAllWithOptions {
                needle: needle.into(),
                replacement: replacement.into(),
                options: SearchOptions {
                    match_case: false,
                    whole_words: true
                }
            }),
            Err(if needle.is_empty() {
                CoreError::EmptySearch
            } else {
                CoreError::InvalidText
            })
        );
        assert_eq!(e.document(), &original);
        assert_eq!(e.selection(), selection);
        assert!(e.can_redo());
        assert!(!e.can_undo());
    }
    let result = e
        .execute(Command::ReplaceAll {
            needle: "İ".into(),
            replacement: "x".into(),
        })
        .unwrap();
    assert_eq!(result.replacements, 3);
    assert_eq!(
        (text(&e, 0), text(&e, 1)),
        ("x i\u{307} x2".into(), "x".into())
    );
}

#[test]
fn rejected_whole_word_candidate_does_not_hide_later_overlapping_match() {
    for match_case in [true, false] {
        let options = SearchOptions {
            match_case,
            whole_words: true,
        };
        let d = editor(&["ba a a"]).document().clone();
        assert_eq!(
            d.find_with_options("a a", options).unwrap(),
            vec![range((0, 3), (0, 6))]
        );
        // Accepted results still consume their range and never overlap.
        let d = editor(&["a a a"]).document().clone();
        assert_eq!(
            d.find_with_options("a a", options).unwrap(),
            vec![range((0, 0), (0, 3))]
        );
    }
}
#[test]
fn replace_all_finds_valid_overlap_after_rejected_whole_word_candidate() {
    for match_case in [true, false] {
        let mut e = editor(&["ba a a"]);
        let original = e.document().clone();
        let result = e
            .execute(Command::ReplaceAllWithOptions {
                needle: "a a".into(),
                replacement: "XX".into(),
                options: SearchOptions {
                    match_case,
                    whole_words: true,
                },
            })
            .unwrap();
        assert_eq!(result.replacements, 1);
        assert_eq!(text(&e, 0), "ba XX");
        e.execute(Command::Undo).unwrap();
        assert_eq!(e.document(), &original);
        assert!(!e.is_dirty());
    }
}

fn case_fixture(first: &str, second: &str) -> Document {
    let bold = TextStyle {
        bold: true,
        ..Default::default()
    };
    let italic = TextStyle {
        italic: true,
        ..Default::default()
    };
    Document {
        blocks: vec![
            Block::Paragraph(Paragraph {
                runs: vec![
                    Run::new("keep ", TextStyle::default()),
                    Run::new(first, bold.clone()),
                    Run::new(second, italic.clone()),
                ],
                style: ParagraphStyle {
                    alignment: Alignment::Center,
                    ..Default::default()
                },
                default_style: bold.clone(),
            }),
            Block::Paragraph(Paragraph {
                default_style: italic.clone(),
                style: ParagraphStyle {
                    space_before_twips: 140,
                    ..Default::default()
                },
                ..Default::default()
            }),
            Block::PageBreak,
            Block::Paragraph(Paragraph {
                runs: vec![
                    Run::new(first, italic.clone()),
                    Run::new(second, bold.clone()),
                    Run::new(" after", TextStyle::default()),
                ],
                style: ParagraphStyle {
                    alignment: Alignment::Right,
                    space_after_twips: 240,
                    ..Default::default()
                },
                default_style: italic,
            }),
        ],
        ..Default::default()
    }
}

#[test]
fn case_conversion_preserves_blocks_styles_expansions_and_atomic_history() {
    for (first, second, upper, mapped_first, mapped_second) in [
        ("ß", "é", true, "SS", "É"),
        ("İ", "Σ", false, "i\u{307}", "ς"),
    ] {
        let mut e = Editor::new(case_fixture(first, second)).unwrap();
        let original = e.document().clone();
        let selection = range((3, first.len() + second.len()), (0, 5));
        e.set_selection(selection).unwrap();
        let expected = case_fixture(mapped_first, mapped_second);
        let outcome = e
            .execute(Command::ConvertCase {
                selection,
                case: if upper {
                    TextCase::Upper
                } else {
                    TextCase::Lower
                },
            })
            .unwrap();
        assert_eq!(
            e.document(),
            &expected,
            "case conversion must retain rich document structure"
        );
        let caret = Selection::caret(at(3, mapped_first.len() + mapped_second.len()));
        assert_eq!(outcome.selection, caret);
        assert!(outcome.changed);
        assert!(e.is_dirty());
        e.execute(Command::Undo).unwrap();
        assert_eq!(e.document(), &original);
        assert_eq!(e.selection(), selection);
        assert!(!e.is_dirty());
        assert!(!e.can_undo(), "one transaction only");
        e.execute(Command::Redo).unwrap();
        assert_eq!(e.document(), &expected);
        assert_eq!(e.selection(), caret);
    }
}

#[test]
fn unchanged_case_conversion_preserves_metadata_clean_state_and_history() {
    let mut e = Editor::new(case_fixture("SS", "É")).unwrap();
    let original = e.document().clone();
    let selection = range((0, 5), (3, 4));
    e.set_selection(selection).unwrap();
    let outcome = e
        .execute(Command::ConvertCase {
            selection,
            case: TextCase::Upper,
        })
        .unwrap();
    assert_eq!(e.document(), &original);
    assert!(!outcome.changed);
    assert!(!e.is_dirty());
    assert!(!e.can_undo());
    assert!(!e.can_redo());
    assert_eq!(e.selection(), Selection::caret(at(3, 4)));
}

#[test]
fn title_case_keeps_final_sigma_context_styles_and_history() {
    let mut e = Editor::new(case_fixture("Ο", "Σ ΟΣ")).unwrap();
    let original = e.document().clone();
    let selection = range((3, "ΟΣ ΟΣ".len()), (0, 5));
    e.set_selection(selection).unwrap();
    e.execute(Command::ConvertCase {
        selection,
        case: TextCase::Title,
    })
    .unwrap();
    let expected = case_fixture("Ο", "ς Ος");
    assert_eq!(e.document(), &expected);
    let caret = Selection::caret(at(3, "Ος Ος".len()));
    assert_eq!(e.selection(), caret);
    e.document().validate().unwrap();
    e.execute(Command::Undo).unwrap();
    assert_eq!(e.document(), &original);
    assert_eq!(e.selection(), selection);
    assert!(!e.is_dirty());
    assert!(!e.can_undo());
    e.execute(Command::Redo).unwrap();
    assert_eq!(e.document(), &expected);
    assert_eq!(e.selection(), caret);
}

#[test]
fn title_case_skips_opening_punctuation_and_preserves_styles() {
    assert_case_preserves_styles(
        TextCase::Title,
        "\"HELLO\"  “ΟΣ”\t(İSTANBUL) ...",
        "\"Hello\"  “Ος”\t(İstanbul) ...",
    );
}

#[test]
fn sentence_case_skips_opening_and_closing_punctuation_and_preserves_styles() {
    for (case, input, expected) in [
        (
            TextCase::Sentence,
            "\"HELLO.\"  “WORLD!”\t(ΟΣ) ...",
            "\"Hello.\"  “World!”\t(Ος) ...",
        ),
        (TextCase::Sentence, "... \t\"...\"", "... \t\"...\""),
        (
            TextCase::Sentence,
            "(E\u{301}COLE) IS OPEN?  \"YES\".",
            "(E\u{301}cole) is open?  \"Yes\".",
        ),
    ] {
        assert_case_preserves_styles(case, input, expected);
    }
}

fn assert_case_preserves_styles(case: TextCase, input: &str, expected: &str) {
    let mut e = Editor::new(case_fixture("\"", input)).unwrap();
    let original = e.document().clone();
    let selection = range((0, 5), (3, 1 + input.len()));
    e.set_selection(selection).unwrap();
    e.execute(Command::ConvertCase { selection, case }).unwrap();
    assert_eq!(e.document(), &case_fixture("\"", expected), "{input}");
    e.document().validate().unwrap();
    if input != expected {
        e.execute(Command::Undo).unwrap();
        assert_eq!(e.document(), &original);
        assert_eq!(e.selection(), selection);
    }
}

#[test]
fn rich_formatting_preserves_mixed_graphemes_and_undo_redo() {
    let bold = TextStyle {
        bold: true,
        ..Default::default()
    };
    let italic = TextStyle {
        italic: true,
        ..Default::default()
    };
    let document = Document {
        blocks: vec![Block::Paragraph(Paragraph {
            runs: vec![
                Run::new("ae", bold.clone()),
                Run::new("\u{301}👩‍💻z", italic.clone()),
            ],
            ..Default::default()
        })],
        ..Default::default()
    };
    let mut e = Editor::new(document.clone()).unwrap();
    let selection = range((0, 1), (0, 15));
    e.execute(Command::FormatRuns {
        selection,
        patch: StylePatch {
            strikethrough: Some(true),
            vertical_align: Some(VerticalAlign::Superscript),
            highlight: Some(Some(Color::rgb(200, 210, 220))),
            ..Default::default()
        },
    })
    .unwrap();
    let formatted = e.document().clone();
    let p = formatted.paragraph(0).unwrap();
    assert_eq!(p.text(), "ae\u{301}👩‍💻z");
    assert_eq!(p.runs.len(), 4);
    assert!(!p.runs[0].style.strikethrough && !p.runs[3].style.strikethrough);
    assert!(p.runs[1].style.bold && p.runs[2].style.italic);
    for run in &p.runs[1..3] {
        assert!(run.style.strikethrough);
        assert_eq!(run.style.vertical_align, VerticalAlign::Superscript);
        assert_eq!(run.style.highlight, Some(Color::rgb(200, 210, 220)));
    }
    e.execute(Command::Undo).unwrap();
    assert_eq!(e.document(), &document);
    e.execute(Command::Redo).unwrap();
    assert_eq!(e.document(), &formatted);
    e.execute(Command::FormatRuns {
        selection,
        patch: StylePatch {
            highlight: Some(None),
            vertical_align: Some(VerticalAlign::Subscript),
            ..Default::default()
        },
    })
    .unwrap();
    for run in &e.document().paragraph(0).unwrap().runs[1..3] {
        assert_eq!(run.style.highlight, None);
        assert_eq!(run.style.vertical_align, VerticalAlign::Subscript);
        assert!(run.style.strikethrough);
    }
    let before = e.document().clone();
    assert_eq!(
        e.execute(Command::FormatRuns {
            selection,
            patch: StylePatch {
                size_half_points: Some(0),
                highlight: Some(Some(Color::BLACK)),
                ..Default::default()
            }
        }),
        Err(CoreError::InvalidStyle)
    );
    assert_eq!(e.document(), &before);
}
