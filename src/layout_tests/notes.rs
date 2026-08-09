    use crate::model::{BlockIndex, Endnote, Footnote, SectionIndex};

    fn footnote(content: Paragraph) -> Footnote {
        Footnote {
            content,
            section_index: SectionIndex::new(1),
            body_block_index: BlockIndex::new(0),
            numbering: None,
        }
    }

    fn endnote(content: Paragraph, placement: EndnotePlacement) -> Endnote {
        Endnote {
            content,
            section_index: SectionIndex::new(1),
            placement,
            numbering: None,
        }
    }

    #[test]
    fn lays_out_footnotes_after_body_with_separator() {
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Body".to_string(),
                style: Default::default(),
            }],
        })];
        document.footnotes = vec![footnote(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "* Footnote text".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let text = layout_text(page);

        assert!(text.contains("Body"));
        assert!(text.contains("1. * Footnote text"));
        let label = text_fragment_for(page, "1");
        let note_text = first_text_fragment_except(page, &["Body", "1"]);
        assert_eq!(
            label.style.font_size_scale_percent,
            PASSIVE_NOTE_LABEL_FONT_SCALE_PERCENT
        );
        assert!(label.baseline_y > note_text.baseline_y);
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { x1, x2, width, .. }
                if (*x1 - 72.0).abs() < 0.01
                    && (*x2 - 216.0).abs() < 0.01
                    && (*width - 0.6).abs() < 0.01
        )));
    }

    #[test]
    fn lays_out_bottom_footnotes_at_page_bottom() {
        let mut document = Document::default();
        document.footnote_placement = FootnotePlacement::BottomOfPage;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Body".to_string(),
                style: Default::default(),
            }],
        })];
        document.footnotes = vec![footnote(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Bottom footnote".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let body_y = text_baseline_for(page, "Body");
        let note_y = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(fragment) if fragment.text.contains("Bottom") => {
                    Some(fragment.baseline_y)
                }
                _ => None,
            })
            .expect("bottom footnote text");
        let separator_y = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Line { x1, x2, y1, y2, .. }
                    if (*x1 - 72.0).abs() < 0.01
                        && (*x2 - 216.0).abs() < 0.01
                        && (*y1 - *y2).abs() < 0.01 =>
                {
                    Some(*y1)
                }
                _ => None,
            })
            .expect("footnote separator");

        assert!(body_y > 650.0);
        assert!(
            note_y < 90.0,
            "bottom footnote baseline should stay near page bottom, got {note_y}"
        );
        assert!(
            (separator_y - 90.5).abs() < 0.01,
            "separator should sit above bottom footnote band, got {separator_y}"
        );
    }

    #[test]
    fn compact_word_footnotes_reserve_the_source_bottom_band_lead() {
        let parsed = parse_rtf(include_str!(
            "../../fixtures/note-restart-numbering-passive.rtf"
        ))
        .unwrap();
        let provider = FontProvider::browser_safe_defaults();
        let layout = LayoutEngine::layout_with_font_provider(&parsed.document, Some(&provider));
        let page = &layout.pages[0];
        let separator_y = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Line { x1, x2, y1, y2, .. }
                    if (*x1 - 36.0).abs() < 0.01
                        && (*x2 - 180.0).abs() < 0.01
                        && (*y1 - *y2).abs() < 0.01
                        && *y1 < 120.0 =>
                {
                    Some(*y1)
                }
                _ => None,
            })
            .expect("bottom footnote separator");
        let first_note = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(text) if text.text.contains("First") => Some(text),
                _ => None,
            })
            .expect("first compact footnote");
        let second_note = page
            .items
            .iter()
            .find_map(|item| match item {
                LayoutItem::Text(text) if text.text.contains("Second") => Some(text),
                _ => None,
            })
            .expect("second compact footnote");

        assert!(
            (separator_y - 98.48).abs() < 0.10,
            "separator={separator_y}"
        );
        assert!((first_note.baseline_y - 79.82).abs() < 0.03);
        assert!((second_note.baseline_y - 52.20).abs() < 0.03);
    }

    #[test]
    fn lays_out_endnotes_after_body_with_separator() {
        let mut document = Document::default();
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Body".to_string(),
                style: Default::default(),
            }],
        })];
        document.endnotes = vec![endnote(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "* Endnote text".to_string(),
                style: Default::default(),
            }],
        }, document.endnote_placement)];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let text = layout_text(page);

        assert!(text.contains("Body"));
        assert!(text.contains("1. * Endnote text"));
        let label = text_fragment_for(page, "1");
        let note_text = first_text_fragment_except(page, &["Body", "1"]);
        assert_eq!(
            label.style.font_size_scale_percent,
            PASSIVE_NOTE_LABEL_FONT_SCALE_PERCENT
        );
        assert!(label.baseline_y > note_text.baseline_y);
        assert!(page.items.iter().any(|item| matches!(
            item,
            LayoutItem::Line { x1, x2, width, .. }
                if (*x1 - 72.0).abs() < 0.01
                    && (*x2 - 216.0).abs() < 0.01
                    && (*width - 0.6).abs() < 0.01
        )));
    }

    #[test]
    fn lays_out_end_of_document_endnotes_on_final_page() {
        let mut document = Document::default();
        document.endnote_placement = EndnotePlacement::EndOfDocument;
        document.blocks = vec![Block::Paragraph(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Body".to_string(),
                style: Default::default(),
            }],
        })];
        document.endnotes = vec![endnote(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Endnote text".to_string(),
                style: Default::default(),
            }],
        }, document.endnote_placement)];

        let layout = LayoutEngine::layout(&document);

        assert_eq!(layout.pages.len(), 1);
        assert!(layout_text(&layout.pages[0]).contains("Body"));
        assert!(layout_text(&layout.pages[0]).contains("1. Endnote text"));
    }

    #[test]
    fn lays_out_endnotes_before_final_page_footnote_band() {
        let mut document = Document::default();
        document.footnote_placement = FootnotePlacement::BottomOfPage;
        document.endnote_placement = EndnotePlacement::EndOfDocument;
        document.blocks = vec![paragraph_with_text("Body")];
        document.endnotes = vec![endnote(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Endnote text".to_string(),
                style: Default::default(),
            }],
        }, document.endnote_placement)];
        document.footnotes = vec![footnote(Paragraph {
            style: Default::default(),
            runs: vec![Run {
                text: "Footnote text".to_string(),
                style: Default::default(),
            }],
        })];

        let layout = LayoutEngine::layout(&document);
        let page = &layout.pages[0];
        let baseline_for = |needle: &str| {
            page.items
                .iter()
                .find_map(|item| match item {
                    LayoutItem::Text(fragment) if fragment.text.contains(needle) => {
                        Some(fragment.baseline_y)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("missing {needle} in {:?}", layout_text(page)))
        };

        assert_eq!(layout.pages.len(), 1);
        assert!(baseline_for("Endnote") > baseline_for("Footnote"));
    }
