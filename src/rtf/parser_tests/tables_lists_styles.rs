    #[test]
    fn word_empty_paragraph_after_table_keeps_source_order() {
        let output =
            parse_rtf(r"{\rtf1\ansi\trowd\cellx1000\intbl A\cell\row\pard\par B\par}").unwrap();
        assert!(matches!(output.document.blocks[0], Block::Table(_)));
        let Block::Paragraph(empty) = &output.document.blocks[1] else {
            panic!("expected empty paragraph after table");
        };
        assert_eq!(empty.runs.len(), 1);
        assert!(empty.runs[0].text.is_empty());
        assert!(matches!(output.document.blocks[2], Block::Paragraph(_)));
    }

    #[test]
    fn escaped_special_characters_in_explicit_list_marker_stay_marker_text() {
        let output = parse_rtf(r"{\rtf1{\*\pntext\~\_\tab}List item\par}").unwrap();
        let text = document_text(&output.document);

        assert!(text.starts_with("\u{00a0}\u{2011}\tList item"));
        assert!(!text.contains("pntext"));
    }

    #[test]
    fn parses_font_and_color_tables() {
        let input = r"{\rtf1{\fonttbl{\f0\fswiss Arial;}{\f1 Courier New;}}{\colortbl;\red255\green0\blue0;}Hello}";
        let output = parse_rtf(input).unwrap();
        assert!(
            output
                .document
                .fonts
                .iter()
                .any(|font| font.name == "Arial" && font.family == FontFamilyHint::Swiss)
        );
        assert!(
            output
                .document
                .fonts
                .iter()
                .any(|font| font.name == "Courier New" && font.family == FontFamilyHint::Nil)
        );
        assert_eq!(
            output.document.colors[0],
            Color {
                red: 0,
                green: 0,
                blue: 0
            }
        );
        assert_eq!(
            output.document.colors[1],
            Color {
                red: 255,
                green: 0,
                blue: 0
            }
        );
    }

    #[test]
    fn unicode_font_table_names_skip_fallback_bytes() {
        let input =
            r"{\rtf1{\fonttbl{\f0 Tuff\u121?;}{\f1 Mystery{\*\falt Tuff\u121?;};}}\f0 A\f1 B\par}";
        let output = parse_rtf(input).unwrap();

        let primary = output
            .document
            .fonts
            .iter()
            .find(|font| font.index == 0)
            .expect("primary font");
        let alternate = output
            .document
            .fonts
            .iter()
            .find(|font| font.index == 1)
            .expect("alternate font");

        assert_eq!(primary.name, "Tuffy");
        assert_eq!(alternate.alternate_name.as_deref(), Some("Tuffy"));
        for forbidden in ["u121", "?", "Tuff?"] {
            assert!(
                output
                    .document
                    .fonts
                    .iter()
                    .all(|font| !font.name.contains(forbidden)
                        && !font
                            .alternate_name
                            .as_deref()
                            .is_some_and(|name| name.contains(forbidden))),
                "Unicode font-table fallback leaked into font metadata: {forbidden}"
            );
        }
    }

    #[test]
    fn unicode_font_table_separators_flush_font_entries() {
        let input = r"{\rtf1{\fonttbl{\f0 Tuff\u121?\u59?}{\f1 Mystery{\*\falt Tuff\u121?\u59?};}}\f0 A\f1 B\par}";
        let output = parse_rtf(input).unwrap();

        let primary = output
            .document
            .fonts
            .iter()
            .find(|font| font.index == 0)
            .expect("primary font");
        let alternate = output
            .document
            .fonts
            .iter()
            .find(|font| font.index == 1)
            .expect("alternate font");

        assert_eq!(primary.name, "Tuffy");
        assert_eq!(alternate.name, "Mystery");
        assert_eq!(alternate.alternate_name.as_deref(), Some("Tuffy"));
        for forbidden in ["u121", "u59", "?", ";", "Tuff?"] {
            assert!(
                output
                    .document
                    .fonts
                    .iter()
                    .all(|font| !font.name.contains(forbidden)
                        && !font
                            .alternate_name
                            .as_deref()
                            .is_some_and(|name| name.contains(forbidden))),
                "Unicode font-table separator/fallback leaked into font metadata: {forbidden}"
            );
        }
    }

    #[test]
    fn strips_embedded_font_destinations_from_font_table_metadata() {
        let input = r"{\rtf1{\fonttbl{\f0\fswiss Arial{\fontemb{\fontfile HOSTILE-FONT-PAYLOAD {\object\objdata 414243}}};}{\f1 Courier New;}}Visible\par}";
        let output = parse_rtf(input).unwrap();
        let text = document_text(&output.document);

        assert!(text.contains("Visible"));
        assert!(
            output
                .document
                .fonts
                .iter()
                .any(|font| { font.name == "Arial" && font.family == FontFamilyHint::Swiss })
        );
        assert!(
            output
                .document
                .fonts
                .iter()
                .any(|font| font.name == "Courier New")
        );
        for forbidden in [
            "HOSTILE-FONT-PAYLOAD",
            "fontemb",
            "fontfile",
            "object",
            "objdata",
            "414243",
        ] {
            assert!(
                !text.contains(forbidden),
                "embedded font payload leaked into text: {forbidden}"
            );
            assert!(
                output
                    .document
                    .fonts
                    .iter()
                    .all(|font| !font.name.contains(forbidden)),
                "embedded font payload leaked into font name: {forbidden}"
            );
        }
    }

    #[test]
    fn applies_stylesheet_paragraph_and_character_styles() {
        let options = RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1{\stylesheet{\s1\qc\li720\b Heading;}{\s2\ri360 Plain;}}\s1 Styled\par\s2 Plain\par}",
            &options,
        )
        .unwrap();

        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(first.style.alignment, Alignment::Center);
        assert_eq!(first.style.left_indent_twips, 720);
        assert!(first.runs[0].style.bold);
        assert_eq!(first.runs[0].text, "Styled");

        assert_eq!(second.style.right_indent_twips, 360);
        assert!(!second.runs[0].style.bold);
        assert_eq!(second.runs[0].text, "Plain");
    }

    #[test]
    fn missing_style_zero_uses_word_normal_defaults_without_warning() {
        let output = parse_rtf(
            r"{\rtf1{\stylesheet{\s1\b Heading;}}\s1 Bold\par\s0 Normal\par\s99 Unknown\par}",
        )
        .unwrap();
        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let third = match &output.document.blocks[2] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(document_text(&output.document), "BoldNormalUnknown");
        assert!(!first.runs[0].style.bold);
        assert!(!second.runs[0].style.bold);
        assert_eq!(second.style, ParagraphStyle::default());
        assert_eq!(third.runs[0].text, "Unknown");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unknown RTF style index 0"))
        );
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("unknown RTF style index 99") })
        );
    }

    #[test]
    fn applies_character_styles_without_resetting_paragraph_style() {
        let options = RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1{\stylesheet{\cs5\qc\li1440\b Emphasis;}}\qr Right \cs5 Bold only\par}",
            &options,
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.style.alignment, Alignment::Right);
        assert_eq!(paragraph.style.left_indent_twips, 0);
        assert_eq!(paragraph.runs[0].text, "Right ");
        assert!(!paragraph.runs[0].style.bold);
        assert_eq!(paragraph.runs[1].text, "Bold only");
        assert!(paragraph.runs[1].style.bold);
    }

    #[test]
    fn character_styles_preserve_direct_character_formatting() {
        let options = RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Courier New;}}{\stylesheet{\cs5\b Emphasis;}}\f1\i Direct \cs5 Direct and styled\par}",
            &options,
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "Direct ");
        assert_eq!(paragraph.runs[0].style.font_index, 1);
        assert!(paragraph.runs[0].style.italic);
        assert!(!paragraph.runs[0].style.bold);
        assert_eq!(paragraph.runs[1].text, "Direct and styled");
        assert_eq!(paragraph.runs[1].style.font_index, 1);
        assert!(paragraph.runs[1].style.italic);
        assert!(paragraph.runs[1].style.bold);
    }

    #[test]
    fn applies_stylesheet_based_on_inheritance() {
        let options = RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1{\stylesheet{\s2\sbasedon1\i Child;}{\s1\qc\li720\b Base;}}\s2 Inherited\par}",
            &options,
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.style.alignment, Alignment::Center);
        assert_eq!(paragraph.style.left_indent_twips, 720);
        assert!(paragraph.runs[0].style.bold);
        assert!(paragraph.runs[0].style.italic);
        assert_eq!(paragraph.runs[0].text, "Inherited");
    }

    #[test]
    fn retains_stylesheet_next_style_as_editor_metadata_only() {
        let options = RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1{\stylesheet{\s1\snext2\b Heading;}{\s2\qc\i Body;}}\s1 Heading\par Body text\par}",
            &options,
        )
        .unwrap();

        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(first.runs[0].text, "Heading");
        assert!(first.runs[0].style.bold);
        assert!(!first.runs[0].style.italic);

        assert_eq!(second.runs[0].text, "Body text");
        assert_eq!(second.style.alignment, Alignment::Left);
        assert!(second.runs[0].style.bold);
        assert!(!second.runs[0].style.italic);
    }

    #[test]
    fn cyclic_stylesheet_inheritance_is_bounded() {
        let options = RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1{\stylesheet{\s1\sbasedon2\b First;}{\s2\sbasedon1\i Second;}}\s1 Safe\par}",
            &options,
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert!(paragraph.runs[0].style.bold);
        assert!(paragraph.runs[0].style.italic);
        assert_eq!(paragraph.runs[0].text, "Safe");
    }

    #[test]
    fn word_compatible_stylesheet_references_keep_only_direct_formatting() {
        let output = parse_rtf(
            r"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Courier New;}}{\stylesheet{\s1\qc\b Base;}{\s2\sbasedon1\i Child;}{\cs5\b Emphasis;}}\s2 Plain\par\pard\qr Right \cs5 still plain\par\pard\f1\i Direct \cs5 remains direct\par}",
        )
        .unwrap();

        let inherited = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected inherited paragraph"),
        };
        assert_eq!(inherited.style.alignment, Alignment::Left);
        assert!(!inherited.runs[0].style.bold);
        assert!(!inherited.runs[0].style.italic);

        let right = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected right paragraph"),
        };
        assert_eq!(right.style.alignment, Alignment::Right);
        assert!(right.runs.iter().all(|run| !run.style.bold));

        let direct = match &output.document.blocks[2] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected direct paragraph"),
        };
        assert!(direct.runs.iter().all(|run| run.style.italic));
        assert!(direct.runs.iter().all(|run| run.style.font_index == 1));
        assert!(direct.runs.iter().all(|run| !run.style.bold));
    }

    #[test]
    fn deep_stylesheet_based_on_chain_resolves_iteratively() {
        let mut parser = Parser::new(Vec::new(), RtfParseOptions::default());
        let mut base_paragraph = ParagraphStyle {
            alignment: Alignment::Center,
            left_indent_twips: 720,
            ..ParagraphStyle::default()
        };
        let mut base_character = CharacterStyle {
            bold: true,
            ..CharacterStyle::default()
        };
        parser.styles.push(StyleDefinition {
            index: 1,
            name: None,
            based_on: None,
            next_style: None,
            kind: StyleKind::Paragraph,
            paragraph: base_paragraph.clone(),
            character: base_character.clone(),
        });
        for index in 2..=512 {
            base_paragraph = ParagraphStyle::default();
            base_character = CharacterStyle::default();
            if index == 512 {
                base_character.italic = true;
            }
            parser.styles.push(StyleDefinition {
                index,
                name: None,
                based_on: Some(index - 1),
                next_style: None,
                kind: StyleKind::Paragraph,
                paragraph: base_paragraph.clone(),
                character: base_character.clone(),
            });
        }
        let resolved = parser.resolve_style(512).expect("deep style chain");

        assert_eq!(resolved.paragraph.alignment, Alignment::Center);
        assert_eq!(resolved.paragraph.left_indent_twips, 720);
        assert!(resolved.character.bold);
        assert!(resolved.character.italic);
    }

    #[test]
    fn stylesheet_count_limit_is_enforced() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_styles: 1,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let error = parse_rtf_bytes_with_options(
            br"{\rtf1{\stylesheet{\s1 One;}{\s2 Two;}}Body\par}",
            &options,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ParseError::ResourceLimitExceeded { resource, .. } if resource == "styles"
        ));
    }

    #[test]
    fn normalizes_basic_table_rows_into_safe_table_model() {
        let output = parse_rtf(
            r"{\rtf1\trowd\cellx2000\cellx4000\intbl Name\cell Value\cell\row After\par}",
        )
        .unwrap();

        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        assert_eq!(table.column_widths_twips, vec![2000, 2000]);
        assert_eq!(table.rows[0].column_widths_twips, vec![2000, 2000]);
        assert_eq!(table.rows[0].cell_gap_twips, 0);
        assert!(!table.borders_visible);
        assert_eq!(table.rows.len(), 1);
        assert_eq!(table.rows[0].cells.len(), 2);
        assert_eq!(table.rows[0].cells[0].paragraphs[0].runs[0].text, "Name");
        assert_eq!(table.rows[0].cells[1].paragraphs[0].runs[0].text, "Value");

        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected following paragraph"),
        };
        assert_eq!(second.runs[0].text, "After");
    }

    #[test]
    fn carries_previous_table_row_definition_for_omitted_trowd_rows() {
        let output = parse_rtf(
            r"{\rtf1\trowd\trgaph108\trbrdrb\brdrs\brdrw20\clbrdrl\brdrs\brdrw30\clbrdrt\brdrs\brdrw30\clbrdrb\brdrs\brdrw30\clbrdrr\brdrs\brdrw30\cellx1200\clbrdrr\brdrs\brdrw30\cellx2400 A\cell B\cell\row\pard\intbl C\cell D\cell\row After\par}",
        )
        .unwrap();

        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        assert_eq!(table.column_widths_twips, vec![1200, 1200]);
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[1].cells.len(), 2);
        assert_eq!(table.rows[1].cell_gap_twips, 108);
        assert_eq!(table.rows[1].cells[0].paragraphs[0].runs[0].text, "C");
        assert_eq!(table.rows[1].cells[1].paragraphs[0].runs[0].text, "D");
        assert_eq!(table.rows[1].cells[0].borders.left.width_twips, 30);
        assert_eq!(table.rows[1].cells[0].borders.top.width_twips, 30);
        assert_eq!(table.rows[1].cells[0].borders.bottom.width_twips, 30);
        assert!(!table.rows[1].cells[1].borders.bottom.visible);
        assert_eq!(table.rows[1].cells[1].borders.right.width_twips, 30);

        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected following paragraph"),
        };
        assert_eq!(second.runs[0].text, "After");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("row-level table borders ignored to match Word print behavior")
        }));
    }

    #[test]
    fn intbl_zero_does_not_carry_table_state_into_body_text() {
        let output =
            parse_rtf(r"{\rtf1\trowd\cellx1200 A\cell\row\pard\intbl0 Outside table\par}").unwrap();

        let first_table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected first table block"),
        };
        assert_eq!(first_table.rows.len(), 1);
        assert_eq!(first_table.rows[0].cells[0].paragraphs[0].runs[0].text, "A");

        let paragraph = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected body paragraph after intbl0"),
        };
        assert_eq!(paragraph.runs[0].text, "Outside table");

        assert_eq!(output.document.blocks.len(), 2);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
            "intbl0 should be consumed as a known table-state-off control: {:?}",
            output.diagnostics
        );
    }

    #[test]
    fn normalizes_nested_table_cells_inside_outer_cell() {
        let output = parse_rtf(
            r"{\rtf1\trowd\cellx6000 Outer before {\trowd\itap2\trqr\trleft180\trrh720\trpaddl60\trspdr40\trcbpat2\trshdng5000\clbrdrt\brdrs\brdrw20\clcbpat1\clshdng5000\clpadl120\clvertalc\cltxbtlr\clNoWrap\cellx1000 {\b Inner A}\par\b0\nestcell\cellx3000 Inner B\nestrow} Outer after\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        let text = table.rows[0].cells[0]
            .paragraphs
            .iter()
            .flat_map(|paragraph| paragraph.runs.iter())
            .map(|run| run.text.as_str())
            .collect::<String>();

        assert_eq!(table.rows.len(), 1);
        assert_eq!(table.rows[0].cells.len(), 1);
        assert!(text.contains("Outer after"));
        let nested = table.rows[0].cells[0]
            .nested_table
            .as_ref()
            .expect("nested table should be normalized into the outer cell");
        assert_eq!(nested.rows.len(), 1);
        assert_eq!(nested.rows[0].cells.len(), 2);
        assert!(nested.borders_visible);
        assert_eq!(nested.rows[0].height_twips, Some(720));
        assert_eq!(nested.rows[0].alignment, TableRowAlignment::Right);
        assert_eq!(nested.rows[0].left_offset_twips, 180);
        assert_eq!(nested.rows[0].cells[0].padding.left_twips, Some(120));
        assert_eq!(nested.rows[0].cells[0].spacing.right_twips, Some(40));
        assert_eq!(nested.column_widths_twips, vec![1000, 2000]);
        assert_eq!(
            nested.rows[0].cells[0].paragraphs[0].runs[0].text,
            "Inner A"
        );
        assert!(nested.rows[0].cells[0].paragraphs[0].runs[0].style.bold);
        assert!(nested.rows[0].cells[0].paragraphs[0].style.no_wrap);
        assert!(nested.rows[0].cells[0].borders.top.visible);
        assert_eq!(
            nested.rows[0].cells[0].borders.top.style,
            BorderStyle::Single
        );
        assert_eq!(nested.rows[0].cells[0].shading_color_index, Some(1));
        assert_eq!(nested.rows[0].cells[0].shading_basis_points, 5000);
        assert_eq!(nested.rows[0].cells[1].shading_color_index, Some(2));
        assert_eq!(nested.rows[0].cells[1].shading_basis_points, 5000);
        assert_eq!(nested.rows[0].cells[0].padding.left_twips, Some(120));
        assert_eq!(
            nested.rows[0].cells[0].vertical_align,
            TableCellVerticalAlign::Center
        );
        assert_eq!(
            nested.rows[0].cells[0].text_direction,
            TableCellTextDirection::BottomToTopLeftToRight
        );
        assert_eq!(
            nested.rows[0].cells[1].paragraphs[0].runs[0].text,
            "Inner B"
        );
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
        assert_eq!(
            output
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic
                    .message
                    .contains("nested table normalized into its containing cell"))
                .count(),
            1
        );
    }

    #[test]
    fn unbordered_nested_cells_remain_unbordered() {
        let output = parse_rtf(
            r"{\rtf1\trowd\cellx4680 Outer {\trowd\itap2\cellx1000 Inner A\nestcell\cellx2000 Inner B\nestrow}\cell\row}",
        )
        .unwrap();
        let Block::Table(table) = &output.document.blocks[0] else {
            panic!("expected table");
        };
        let nested = table.rows[0].cells[0]
            .nested_table
            .as_ref()
            .expect("nested table");

        assert!(!nested.borders_visible);
        assert!(nested.rows[0].cells.iter().all(|cell| {
            let borders = cell.borders;
            !borders.left.visible
                && !borders.right.visible
                && !borders.top.visible
                && !borders.bottom.visible
                && !borders.diagonal_down.visible
                && !borders.diagonal_up.visible
        }));
    }

    #[test]
    fn deeper_nested_table_structure_stays_passive_in_containing_cell() {
        let output = parse_rtf(
            r"{\rtf1\trowd\cellx6000 Outer {\trowd\itap2\cellx3000 A {\trowd\itap16\cellx1500 Deep\nestcell\cellx3000 Deep2\nestrow} After\nestcell\cellx6000 B\nestrow}\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        let nested = table.rows[0].cells[0]
            .nested_table
            .as_ref()
            .expect("nested table");
        assert_eq!(nested.rows.len(), 1);
        assert_eq!(nested.rows[0].cells.len(), 2);
        let first_cell_text = nested.rows[0].cells[0]
            .paragraphs
            .iter()
            .flat_map(|paragraph| paragraph.runs.iter())
            .map(|run| run.text.as_str())
            .collect::<String>();
        assert!(first_cell_text.contains("A"));
        assert!(first_cell_text.contains("Deep"));
        assert!(first_cell_text.contains("After"));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("nested table normalized into its containing cell")
        }));
    }

    #[test]
    fn strict_spec_preserves_nested_row_inner_border_controls_as_passive_cell_edges() {
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\trowd\cellx6000 Outer {\trowd\itap2\trbrdrt\brdrth\brdrw40\trbrdrl\brdrdash\brdrw50\trbrdrr\brdrdot\brdrw60\trbrdrb\brdrdb\brdrw20\trbrdrh\brdrs\brdrw20\trbrdrv\brdrdot\brdrw30\cellx2000 A\nestcell\cellx4000 B\nestrow}\cell\row}",
            &RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        let nested = table.rows[0].cells[0]
            .nested_table
            .as_ref()
            .expect("nested table");
        assert_eq!(
            nested.rows[0].cells[0].borders.bottom.style,
            BorderStyle::Double
        );
        assert_eq!(
            nested.rows[0].cells[1].borders.bottom.style,
            BorderStyle::Double
        );
        assert_eq!(
            nested.rows[0].cells[0].borders.right.style,
            BorderStyle::Dotted
        );
        assert_eq!(
            nested.rows[0].cells[0].borders.top.style,
            BorderStyle::Thick
        );
        assert_eq!(
            nested.rows[0].cells[0].borders.left.style,
            BorderStyle::Dashed
        );
        assert_eq!(
            nested.rows[0].cells[1].borders.right.style,
            BorderStyle::Dotted
        );
        assert_eq!(nested.rows[0].cells[0].borders.bottom.width_twips, 20);
        assert_eq!(nested.rows[0].cells[0].borders.right.width_twips, 30);
        assert_eq!(nested.rows[0].cells[0].borders.top.width_twips, 40);
        assert_eq!(nested.rows[0].cells[0].borders.left.width_twips, 50);
        assert_eq!(nested.rows[0].cells[1].borders.right.width_twips, 60);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn word_compatible_ignores_nested_row_border_controls() {
        let output = parse_rtf(
            r"{\rtf1\trowd\cellx6000 Outer {\trowd\itap2\trbrdrt\brdrth\brdrw40\trbrdrl\brdrdash\brdrw50\trbrdrr\brdrdot\brdrw60\trbrdrb\brdrdb\brdrw20\trbrdrv\brdrdot\brdrw30\cellx2000 A\nestcell\cellx4000 B\nestrow}\cell\row}",
        )
        .unwrap();
        let Block::Table(table) = &output.document.blocks[0] else {
            panic!("expected table block");
        };
        let nested = table.rows[0].cells[0]
            .nested_table
            .as_ref()
            .expect("nested table");

        assert!(nested.rows[0].cells.iter().all(|cell| {
            cell.borders.top.style == BorderStyle::Single
                && cell.borders.right.style == BorderStyle::Single
                && cell.borders.bottom.style == BorderStyle::Single
                && cell.borders.left.style == BorderStyle::Single
                && cell.borders.top.width_twips == 10
                && cell.borders.right.width_twips == 10
                && cell.borders.bottom.width_twips == 10
                && cell.borders.left.width_twips == 10
        }));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("row-level table borders ignored to match Word print behavior")
        }));
    }

    #[test]
    fn normalizes_table_row_height_controls() {
        let output =
            parse_rtf(r"{\rtf1\trowd\trrh720\cellx2000 Tall\cell\row\trowd\trrh-360\cellx2000 Exact\cell\row}")
                .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].height_twips, Some(720));
        assert_eq!(table.rows[1].height_twips, Some(-360));
    }

    #[test]
    fn clamps_extreme_table_row_height_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_table_row_height_twips: 720,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\trowd\trrh9999\cellx2000 Tall\cell\row}",
            &options,
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].height_twips, Some(720));
        assert!(!output.diagnostics.is_empty());

        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\trowd\trrh-9999\cellx2000 Exact\cell\row}",
            &options,
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].height_twips, Some(-720));
    }

    #[test]
    fn normalizes_table_row_left_offset_controls() {
        let output = parse_rtf(r"{\rtf1\trowd\trleft720\cellx2000 Offset\cell\row}").unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].left_offset_twips, 720);
    }

    #[test]
    fn normalizes_floating_table_horizontal_position_as_bounded_row_offset() {
        let output =
            parse_rtf(r"{\rtf1\trowd\trleft360\tposx720\cellx2000 Offset\cell\row}").unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].left_offset_twips, 1080);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "floating table horizontal position interpreted as bounded passive row offset",
            )
        }));
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_signed_floating_table_horizontal_position() {
        let output = parse_rtf(
            r"{\rtf1\trowd\trleft360\tposnegx720\cellx2000 Positive\cell\row\trowd\trleft360\tposnegx-720\cellx2000 Negative\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].left_offset_twips, 1080);
        assert_eq!(table.rows[1].left_offset_twips, -360);
    }

    #[test]
    fn clamps_extreme_table_row_left_offset_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_table_row_offset_twips: 720,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\trowd\trleft-9999\cellx2000 Offset\cell\row}",
            &options,
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].left_offset_twips, -720);
        assert!(!output.diagnostics.is_empty());
    }

    #[test]
    fn clamps_extreme_floating_table_horizontal_position_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_table_row_offset_twips: 720,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\trowd\trleft360\tposx9999\cellx2000 Offset\cell\row}",
            &options,
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].left_offset_twips, 720);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("floating table horizontal position clamped")
        }));
    }

    #[test]
    fn normalizes_signed_floating_table_vertical_position_as_bounded_row_offset() {
        let output = parse_rtf(
            r"{\rtf1\trowd\tposy360\cellx2000 Down\cell\row\trowd\tposnegy240\cellx2000 Signed down\cell\row\trowd\tposnegy-240\cellx2000 Signed up\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].vertical_offset_twips, 360);
        assert_eq!(table.rows[1].vertical_offset_twips, 240);
        assert_eq!(table.rows[2].vertical_offset_twips, -240);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "floating table vertical position interpreted as bounded passive row offset",
            )
        }));
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn clamps_extreme_floating_table_vertical_position_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_table_row_offset_twips: 720,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\trowd\tposy9999\cellx2000 Offset\cell\row}",
            &options,
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].vertical_offset_twips, 720);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("floating table vertical position clamped")
        }));
    }

    #[test]
    fn normalizes_fixed_height_floating_table_vertical_center_bottom_alignment() {
        let output = parse_rtf(
            r"{\rtf1\trowd\trrh720\tposy120\tposyb\cellx2000 Bottom\cell\row\trowd\trrh600\tposyc\cellx2000 Center\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].vertical_offset_twips, -600);
        assert_eq!(table.rows[1].vertical_offset_twips, -300);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("floating table vertical alignment \\tposyb")
        }));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "floating table vertical center/bottom alignment interpreted as bounded passive row offset",
            )
        }));
        assert!(!output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("floating table positioning approximated by passive table flow")
        }));
    }

    #[test]
    fn dynamic_height_floating_table_vertical_center_bottom_alignment_warns() {
        let output = parse_rtf(r"{\rtf1\trowd\tposyb\cellx2000 Bottom\cell\row}").unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(
            table.rows[0].vertical_offset_twips,
            TABLE_ROW_DYNAMIC_VERTICAL_BOTTOM_OFFSET_BASE
        );
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "floating table vertical center/bottom alignment captured for computed passive row height",
            )
        }));
    }

    #[test]
    fn normalizes_table_row_alignment_controls() {
        let output = parse_rtf(
            r"{\rtf1\trowd\trqc\cellx2000 Center\cell\row\trowd\trqr\cellx2000 Right\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].alignment, TableRowAlignment::Center);
        assert_eq!(table.rows[1].alignment, TableRowAlignment::Right);
    }

    #[test]
    fn normalizes_floating_table_horizontal_alignment_controls() {
        let output = parse_rtf(
            r"{\rtf1\trowd\tposxc\cellx2000 Center\cell\row\trowd\tposxr\cellx2000 Right\cell\row\trowd\tposxl\cellx2000 Left\cell\row\trowd\tposxi\cellx2000 Inside\cell\row\trowd\tposxo\cellx2000 Outside\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].alignment, TableRowAlignment::Center);
        assert_eq!(table.rows[1].alignment, TableRowAlignment::Right);
        assert_eq!(table.rows[2].alignment, TableRowAlignment::Left);
        assert_eq!(table.rows[3].alignment, TableRowAlignment::Inside);
        assert_eq!(table.rows[4].alignment, TableRowAlignment::Outside);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("floating table horizontal alignment \\tposxc")
        }));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("floating table horizontal alignment \\tposxo")
        }));
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_floating_table_horizontal_anchor_controls() {
        let output = parse_rtf(
            r"{\rtf1\margl720\trowd\phpg\posx1440\cellx2000 Page anchored\cell\row\trowd\phmrg\posx1440\cellx2000 Margin anchored\cell\row\trowd\phcol\posx1440\cellx2000 Column anchored\cell\row\trowd\tphpg\posx1440\cellx2000 T page anchored\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].left_offset_twips, 720);
        assert_eq!(table.rows[1].left_offset_twips, 1440);
        assert_eq!(table.rows[2].left_offset_twips, 1440);
        assert_eq!(table.rows[3].left_offset_twips, 720);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("floating table horizontal anchor \\phpg")
        }));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("floating table horizontal anchor \\tphpg")
        }));
        assert!(!output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("floating table positioning approximated by passive table flow")
        }));
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn interprets_floating_table_no_overlap_through_passive_flow() {
        let output =
            parse_rtf(r"{\rtf1\trowd\tabsnoovrlp\cellx2000 Non overlapping\cell\row}").unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        let text = table.rows[0].cells[0].paragraphs[0].runs[0].text.clone();

        assert!(text.contains("Non overlapping"));
        assert!(table.rows[0].no_overlap);
        assert!(!text.contains("tabsnoovrlp"));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("floating table no-overlap \\tabsnoovrlp captured")
        }));
        assert!(!output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("floating table positioning approximated by passive table flow")
        }));
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn interprets_flow_relative_floating_table_vertical_controls() {
        let output = parse_rtf(
            r"{\rtf1\margt720\trowd\pvpg\posy1440\cellx2000 Page top\cell\row\trowd\pvmrg\posy1440\cellx2000 Margin top\cell\row\trowd\pvpara\posyt\cellx2000 Paragraph top\cell\row\trowd\tpvpg\posy1440\cellx2000 T page top\cell\row\trowd\tposyil\cellx2000 Inline top\cell\row\trowd\tposyin\cellx2000 Inside page\cell\row\trowd\posyout\cellx2000 Outside page\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].vertical_offset_twips, 720);
        assert_eq!(table.rows[1].vertical_offset_twips, 1440);
        assert_eq!(
            table.rows[2].cells[0].paragraphs[0].runs[0].text,
            "Paragraph top"
        );
        assert_eq!(
            table.rows[3].cells[0].paragraphs[0].runs[0].text,
            "T page top"
        );
        assert_eq!(table.rows[3].vertical_offset_twips, 720);
        assert_eq!(
            table.rows[4].cells[0].paragraphs[0].runs[0].text,
            "Inline top"
        );
        assert_eq!(
            table.rows[5].cells[0].paragraphs[0].runs[0].text,
            "Inside page"
        );
        assert_eq!(
            table.rows[6].cells[0].paragraphs[0].runs[0].text,
            "Outside page"
        );
        for control_name in ["pvpg", "pvmrg", "pvpara", "tpvpg"] {
            assert!(output.diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains(&format!("floating table vertical anchor \\{control_name}"))
            }));
        }
        for control_name in ["posyt", "tposyil", "tposyin", "posyout"] {
            assert!(output.diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains(&format!(
                    "floating table vertical flow control \\{control_name}"
                ))
            }));
        }
        assert!(!output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("floating table positioning approximated by passive table flow")
        }));
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_rtl_table_row_controls_to_visual_cell_order() {
        let output = parse_rtf(
            r"{\rtf1\trowd\taprtl\cellx1000 Right\cell\cellx3000 Left wide\cell\row\trowd\taprtl0\cellx1000 A\cell\cellx3000 B\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.column_widths_twips, vec![2000, 1000]);
        assert_eq!(
            table.rows[0].cells[0].paragraphs[0].runs[0].text,
            "Left wide"
        );
        assert_eq!(table.rows[0].cells[1].paragraphs[0].runs[0].text, "Right");
        assert_eq!(table.rows[1].cells[0].paragraphs[0].runs[0].text, "A");
        assert_eq!(table.rows[1].cells[1].paragraphs[0].runs[0].text, "B");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_rtl_table_row_horizontal_merge_groups_without_breaking_merge_markers() {
        let output = parse_rtf(
            r"{\rtf1\trowd\rtlrow\clmgf\cellx1000 Merged\cell\clmrg\cellx2500 Hidden\cell\cellx4000 Plain\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.column_widths_twips, vec![1500, 1000, 1500]);
        assert_eq!(table.rows[0].cells[0].paragraphs[0].runs[0].text, "Plain");
        assert_eq!(
            table.rows[0].cells[1].horizontal_merge,
            TableCellHorizontalMerge::First
        );
        assert_eq!(
            table.rows[0].cells[2].horizontal_merge,
            TableCellHorizontalMerge::Continuation
        );
    }

    #[test]
    fn normalizes_table_row_header_repeat_controls() {
        let output = parse_rtf(
            r"{\rtf1\trowd\trhdr\cellx2000 Header\cell\row\trowd\trhdr0\cellx2000 Body\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert!(table.rows[0].repeat_header);
        assert!(!table.rows[1].repeat_header);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_table_row_keep_together_controls() {
        let output = parse_rtf(
            r"{\rtf1\trowd\trkeep\cellx2000 Kept\cell\row\trowd\trkeep0\cellx2000 Normal\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert!(table.rows[0].keep_together);
        assert!(!table.rows[1].keep_together);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_table_row_cant_split_as_passive_keep_together_metadata() {
        let output = parse_rtf(
            r"{\rtf1\trowd\trcantSplit\cellx2000 Kept\cell\row\trowd\trcantSplit0\cellx2000 Normal\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert!(table.rows[0].keep_together);
        assert!(!table.rows[1].keep_together);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_table_row_keep_following_as_passive_keep_next_metadata() {
        let output = parse_rtf(
            r"{\rtf1\trowd\trkeepfollow\cellx2000 Kept\cell\row\trowd\trkeepfollow0\cellx2000 Normal\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert!(!table.rows[0].keep_together);
        assert!(table.rows[0].keep_with_next);
        assert!(!table.rows[1].keep_together);
        assert!(!table.rows[1].keep_with_next);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
        assert!(output.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("table row keep-with-following approximated")
        }));
    }

    #[test]
    fn normalizes_table_cell_gap_controls() {
        let output = parse_rtf(r"{\rtf1\trowd\trgaph240\cellx2000 Gap\cell\row}").unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].cell_gap_twips, 240);
    }

    #[test]
    fn clamps_extreme_table_cell_gap_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_table_cell_gap_twips: 240,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\trowd\trgaph9999\cellx2000 Gap\cell\row}",
            &options,
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].cell_gap_twips, 240);
        assert!(!output.diagnostics.is_empty());
    }

    #[test]
    fn word_compatible_table_cell_padding_matches_word_left_top_mapping() {
        let output = parse_rtf(
            r"{\rtf1\trowd\clpadl240\clpadr120\clpadt60\clpadb180\cellx2000 Padded\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        let padding = table.rows[0].cells[0].padding;

        assert_eq!(padding.left_twips, Some(60));
        assert_eq!(padding.right_twips, Some(120));
        assert_eq!(padding.top_twips, Some(240));
        assert_eq!(padding.bottom_twips, Some(180));
    }

    #[test]
    fn strict_spec_retains_published_table_cell_padding_sides() {
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\trowd\clpadl240\clpadr120\clpadt60\clpadb180\cellx2000 Padded\cell\row}",
            &RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        let padding = table.rows[0].cells[0].padding;

        assert_eq!(padding.left_twips, Some(240));
        assert_eq!(padding.right_twips, Some(120));
        assert_eq!(padding.top_twips, Some(60));
        assert_eq!(padding.bottom_twips, Some(180));
    }

    #[test]
    fn word_compatible_table_rows_share_the_largest_top_padding() {
        let output =
            parse_rtf(r"{\rtf1\trowd\trrh1440\cellx2000 A\cell\clpadl360\cellx4000 B\cell\row}")
                .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].height_twips, Some(1800));
        assert_eq!(table.rows[0].cells[0].padding.top_twips, Some(360));
        assert_eq!(table.rows[0].cells[1].padding.top_twips, Some(360));
    }

    #[test]
    fn normalizes_table_cell_spacing_controls() {
        let output = parse_rtf(
            r"{\rtf1\trowd\clspdl240\clspdr120\clspdt60\clspdb180\cellx2000 Spaced\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        let spacing = table.rows[0].cells[0].spacing;

        assert_eq!(spacing.left_twips, Some(240));
        assert_eq!(spacing.right_twips, Some(120));
        assert_eq!(spacing.top_twips, Some(60));
        assert_eq!(spacing.bottom_twips, Some(180));
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_table_row_default_padding_controls() {
        let output = parse_rtf(
            r"{\rtf1\trowd\trpaddl240\trpaddr120\trpaddt60\trpaddb180\cellx2000 A\cell\cellx4000 B\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        for cell in &table.rows[0].cells {
            assert_eq!(cell.padding.left_twips, Some(240));
            assert_eq!(cell.padding.right_twips, Some(120));
            assert_eq!(cell.padding.top_twips, Some(60));
            assert_eq!(cell.padding.bottom_twips, Some(180));
        }
    }

    #[test]
    fn omits_preferred_width_rows_when_authored_cell_boundaries_are_missing() {
        let output = parse_rtf(
            r"{\rtf1\trowd\clftsWidth3\clwWidth1440 A\cell\clftsWidth3\clwWidth2880 B\cell\row}",
        )
        .unwrap();
        assert!(
            output
                .document
                .blocks
                .iter()
                .all(|block| !matches!(block, Block::Table(_)))
        );
        assert!(document_text(&output.document).is_empty());
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(
                    "table row without authored cell boundaries omitted to match Word recovery"
                ))
        );
    }

    #[test]
    fn preferred_cell_widths_support_percent_units_without_overriding_cellx() {
        let output = parse_rtf(
            r"{\rtf1\paperw7200\margl720\margr720\trowd\clftsWidth2\clwWidth2500 Half\cell\clftsWidth2\clwWidth1250 Quarter\cell\row\trowd\clftsWidth3\clwWidth9999\cellx1000 Exact\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.column_widths_twips, vec![1000]);
        assert_eq!(table.rows.len(), 1);
        assert_eq!(table.rows[0].cells[0].paragraphs[0].runs[0].text, "Exact");
    }

    #[test]
    fn omits_preferred_row_widths_without_authored_cell_geometry() {
        let output = parse_rtf(
            r"{\rtf1\paperw7200\margl720\margr720\trowd\trftsWidth3\trwWidth2880 A\cell B\cell\row\trowd\trftsWidth2\trwWidth2500 C\cell D\cell\row}",
        )
        .unwrap();
        assert!(
            output
                .document
                .blocks
                .iter()
                .all(|block| !matches!(block, Block::Table(_)))
        );
        assert!(document_text(&output.document).is_empty());
        assert_eq!(
            output
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.message.contains(
                    "table row without authored cell boundaries omitted to match Word recovery"
                ))
                .count(),
            2
        );
    }

    #[test]
    fn nogrowautofit_normalizes_as_safe_table_width_metadata() {
        let output =
            parse_rtf(r"{\rtf1\nogrowautofit\trowd\cellx4000 A\cell\cellx8000 B\cell\row}")
                .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert!(table.preserve_authored_widths);
        assert_eq!(table.column_widths_twips, vec![4_000, 4_000]);
        assert_eq!(table.rows[0].column_widths_twips, vec![4_000, 4_000]);
        assert!(output.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("table autofit growth compatibility approximated")
        }));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("table autofit growth disabled through bounded passive authored widths")
        }));

        let output =
            parse_rtf(r"{\rtf1\nogrowautofit0\trowd\cellx4000 A\cell\cellx8000 B\cell\row}")
                .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        assert!(!table.preserve_authored_widths);
    }

    #[test]
    fn trautofit_normalizes_as_safe_table_width_metadata() {
        let output =
            parse_rtf(r"{\rtf1\trowd\trautofit0\cellx4000 A\cell\cellx8000 B\cell\row}").unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert!(table.preserve_authored_widths);
        assert_eq!(table.column_widths_twips, vec![4_000, 4_000]);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains(
                "table row autofit interpreted through bounded passive table width layout",
            )
        }));
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );

        let output =
            parse_rtf(r"{\rtf1\trowd\trautofit1\cellx4000 A\cell\cellx8000 B\cell\row}").unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        assert!(!table.preserve_authored_widths);
    }

    #[test]
    fn clamps_extreme_table_cell_padding_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_table_cell_gap_twips: 120,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\trowd\clpadl9999\clpadt-50\cellx2000 Padded\cell\row}",
            &options,
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        let padding = table.rows[0].cells[0].padding;

        assert_eq!(padding.left_twips, Some(0));
        assert_eq!(padding.top_twips, Some(120));
        assert!(!output.diagnostics.is_empty());
    }

    #[test]
    fn normalizes_table_border_visibility_controls() {
        let output = parse_rtf(r"{\rtf1\trowd\brdrnone\cellx2000 Borderless\cell\row}").unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        assert!(!table.borders_visible);

        let output =
            parse_rtf(r"{\rtf1\trowd\brdrnone\brdrs\cellx2000 Bordered\cell\row}").unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        assert!(table.borders_visible);
    }

    #[test]
    fn normalizes_table_cell_side_border_controls() {
        let output = parse_rtf(
            r"{\rtf1\trowd\clbrdrl\brdrnone\clbrdrr\brdrs\clbrdrt\brdrnil\clbrdrb\brdrs\cellx2000 Cell\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        let borders = table.rows[0].cells[0].borders;

        assert!(table.borders_visible);
        assert!(!borders.left.visible);
        assert!(borders.right.visible);
        assert!(!borders.top.visible);
        assert!(borders.bottom.visible);
    }

    #[test]
    fn normalizes_table_cell_diagonal_border_controls() {
        let output = parse_rtf(
            r"{\rtf1\trowd\cldgll\brdrdash\brdrw40\brdrcf1\cldglu\brdrs\brdrw30\cellx2000 Cell\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        let borders = table.rows[0].cells[0].borders;

        assert!(borders.diagonal_down.visible);
        assert_eq!(borders.diagonal_down.style, BorderStyle::Dashed);
        assert_eq!(borders.diagonal_down.width_twips, 40);
        assert_eq!(borders.diagonal_down.color_index, Some(1));
        assert!(borders.diagonal_up.visible);
        assert_eq!(borders.diagonal_up.style, BorderStyle::Single);
        assert_eq!(borders.diagonal_up.width_twips, 30);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn normalizes_table_cell_border_width_and_color_controls() {
        let output =
            parse_rtf(r"{\rtf1\trowd\clbrdrl\brdrs\brdrw80\brdrcf2\cellx2000 Cell\cell\row}")
                .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        let border = table.rows[0].cells[0].borders.left;

        assert!(border.visible);
        assert_eq!(border.width_twips, 80);
        assert_eq!(border.color_index, Some(2));
    }

    #[test]
    fn clamps_extreme_table_cell_border_width_controls() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_table_border_width_twips: 40,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\trowd\clbrdrl\brdrs\brdrw9999\cellx2000 Cell\cell\row}",
            &options,
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };
        let border = table.rows[0].cells[0].borders.left;

        assert_eq!(border.width_twips, 40);
        assert!(!output.diagnostics.is_empty());
    }

    #[test]
    fn normalizes_table_cell_shading_controls() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red220\green230\blue240;}\trowd\clcbpat1\cellx2000 Shaded\cell\clcbpat0\cellx4000 Plain\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].cells[0].shading_color_index, Some(1));
        assert_eq!(table.rows[0].cells[1].shading_color_index, None);
    }

    #[test]
    fn normalizes_table_row_default_shading_controls() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red220\green230\blue240;}\trowd\trcbpat1\cellx2000 A\cell\cellx4000 B\cell\row\trowd\trcfpat1\cellx2000 C\cell\row\trowd\trcbpat0\cellx2000 Plain\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].cells[0].shading_color_index, Some(1));
        assert_eq!(table.rows[0].cells[1].shading_color_index, Some(1));
        assert_eq!(table.rows[1].cells[0].shading_color_index, None);
        assert_eq!(
            table.rows[1].cells[0]
                .shading_pattern
                .foreground_color_index(),
            Some(1)
        );
        assert_eq!(table.rows[2].cells[0].shading_color_index, None);
    }

    #[test]
    fn normalizes_table_cell_vertical_alignment_controls() {
        let output = parse_rtf(
            r"{\rtf1\trowd\clvertalc\cellx2000 Center\cell\clvertalb\cellx4000 Bottom\cell\cellx6000 Top\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(
            table.rows[0].cells[0].vertical_align,
            TableCellVerticalAlign::Center
        );
        assert_eq!(
            table.rows[0].cells[1].vertical_align,
            TableCellVerticalAlign::Bottom
        );
        assert_eq!(
            table.rows[0].cells[2].vertical_align,
            TableCellVerticalAlign::Top
        );
    }

    #[test]
    fn normalizes_table_cell_text_direction_controls_as_safe_layout_metadata() {
        let output = parse_rtf(
            r"{\rtf1\trowd\cltxtbrl\cellx2000 Alias\cell\cltxtbrlv\cellx4000 ABC\cell\cltxbtlr\cellx6000 XY\cell\cltxlrtb\cellx8000 Flat\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(table.rows[0].cells[0].paragraphs[0].runs[0].text, "Alias");
        assert_eq!(
            table.rows[0].cells[0].text_direction,
            TableCellTextDirection::TopToBottomRightToLeft
        );
        assert_eq!(table.rows[0].cells[1].paragraphs[0].runs[0].text, "ABC");
        assert_eq!(
            table.rows[0].cells[1].text_direction,
            TableCellTextDirection::TopToBottomRightToLeft
        );
        assert_eq!(table.rows[0].cells[2].paragraphs[0].runs[0].text, "XY");
        assert_eq!(
            table.rows[0].cells[2].text_direction,
            TableCellTextDirection::BottomToTopLeftToRight
        );
        assert_eq!(table.rows[0].cells[3].paragraphs[0].runs[0].text, "Flat");
        assert_eq!(
            table.rows[0].cells[3].text_direction,
            TableCellTextDirection::LeftToRightTopToBottom
        );
        assert!(!document_text(&output.document).contains("cltxtbrl"));
        assert!(!document_text(&output.document).contains("cltxtbrlv"));
        assert!(!document_text(&output.document).contains("cltxbtlr"));
    }

    #[test]
    fn normalizes_table_cell_horizontal_merge_controls() {
        let output = parse_rtf(
            r"{\rtf1\trowd\clmgf\cellx2000 Merged\cell\clmrg\cellx4000 Hidden\cell\cellx6000 Plain\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(
            table.rows[0].cells[0].horizontal_merge,
            TableCellHorizontalMerge::First
        );
        assert_eq!(
            table.rows[0].cells[1].horizontal_merge,
            TableCellHorizontalMerge::Continuation
        );
        assert_eq!(
            table.rows[0].cells[2].horizontal_merge,
            TableCellHorizontalMerge::None
        );
    }

    #[test]
    fn normalizes_table_cell_vertical_merge_controls() {
        let output = parse_rtf(
            r"{\rtf1\trowd\clvmgf\cellx2000 Top\cell\cellx4000 A\cell\row\trowd\clvmrg\cellx2000 Hidden\cell\cellx4000 B\cell\row}",
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table block"),
        };

        assert_eq!(
            table.rows[0].cells[0].vertical_merge,
            TableCellVerticalMerge::First
        );
        assert_eq!(
            table.rows[1].cells[0].vertical_merge,
            TableCellVerticalMerge::Continuation
        );
        assert_eq!(
            table.rows[1].cells[1].vertical_merge,
            TableCellVerticalMerge::None
        );
    }

    #[test]
    fn resultless_styleref_number_switch_renders_passive_list_number_only() {
        let output = parse_rtf(
            r#"{\rtf1{\stylesheet{\s1 Heading One;}}{\*\listtable{\list{\listlevel\levelnfc0{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\s1\ls1\ilvl0 Numbered heading\par\pard Ref {\field{\*\fldinst STYLEREF "Heading One" \\n}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("0.\tNumbered headingRef 0"),
            "text was {text:?}"
        );
        assert!(!text.contains("Ref Numbered heading"));
        for forbidden in [
            "STYLEREF",
            "Heading One",
            "listtable",
            "leveltext",
            "levelnumbers",
            "fldinst",
        ] {
            assert!(
                !text.contains(forbidden),
                "STYLEREF number switch leaked style/list metadata: {forbidden}"
            );
        }

        let strict = parse_rtf_strict(
            r#"{\rtf1{\stylesheet{\s1 Heading One;}}{\*\listtable{\list{\listlevel\levelnfc0{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\s1\ls1\ilvl0 Numbered heading\par\pard Ref {\field{\*\fldinst STYLEREF "Heading One" \\n}}\par}"#,
        )
        .unwrap();
        assert!(
            document_text(&strict.document).contains("1.\tNumbered headingRef 1."),
            "strict text was {:?}",
            document_text(&strict.document)
        );
    }

    #[test]
    fn resultless_styleref_number_context_switches_render_passively() {
        let output = parse_rtf(
            r#"{\rtf1{\stylesheet{\s1 Heading One;}}{\*\listtable{\list{\listlevel\levelnfc0{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\s1\ls1\ilvl0 Numbered heading\par\pard Full {\field{\*\fldinst STYLEREF "Heading One" \\w}} relative {\field{\*\fldinst STYLEREF "Heading One" \\r}} malformed {\field{\*\fldinst STYLEREF "Heading One" \\nfoo}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains(
                "0.\tNumbered headingFull 0 relative 0 malformed [Field removed: no passive result]"
            ),
            "text was {text:?}"
        );
        for forbidden in [
            "STYLEREF",
            "Heading One",
            "nfoo",
            "listtable",
            "leveltext",
            "levelnumbers",
            "fldinst",
        ] {
            assert!(
                !text.contains(forbidden),
                "STYLEREF number-context switch leaked style/list metadata: {forbidden}"
            );
        }
    }

    #[test]
    fn unicode_stylesheet_name_terminator_ignores_trailing_text() {
        let output = parse_rtf(
            r#"{\rtf1{\stylesheet{\s1 Code\u65? Style\u59?Ignored;}}\s1 Styled heading\par\pard Ref {\field{\*\fldinst STYLEREF "Code\u65? Style" \\* Upper}}\par}"#,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Styled headingRef STYLED HEADING"),
            "Unicode style-name terminator should allow STYLEREF to resolve: {text:?}"
        );
        for forbidden in [
            "CodeA Style",
            "Ignored",
            "STYLEREF",
            "fldinst",
            "stylesheet",
            "u65",
            "u59",
            "?",
        ] {
            assert!(
                !text.contains(forbidden),
                "Unicode stylesheet terminator leaked unsafe text: {forbidden}"
            );
        }
    }

    #[test]
    fn style_reference_limit_is_independent_from_styles() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_style_references: 1,
                max_styles: 1,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };

        let output = parse_rtf_bytes_with_options(
            br"{\rtf1{\stylesheet{\s1 Heading;}}\s1 Visible heading\par\pard Body {\field{\*\fldinst STYLEREF 1}}\par}",
            &options,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Visible headingBody Visible heading"),
            "STYLEREF should render with an independent style-reference budget: {text:?}"
        );
    }

    #[test]
    fn style_reference_limit_rejects_excess_cached_style_texts() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_style_references: 1,
                max_styles: 2,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };

        let result = parse_rtf_bytes_with_options(
            br"{\rtf1{\stylesheet{\s1 One;}{\s2 Two;}}\s1 First\par\s2 Second\par}",
            &options,
        );

        assert!(
            matches!(
                result,
                Err(ParseError::ResourceLimitExceeded { ref resource, .. })
                    if resource == "style references"
            ),
            "unexpected result: {result:?}"
        );
    }

    #[test]
    fn old_style_list_markers_in_structural_destinations_do_not_prefix_body_text() {
        let output = parse_rtf(
            r"{\rtf1{\fonttbl{\f0 Arial;{\pntext Hidden\tab}{\pn\pndec\pnstart9{\pntxtb 9}{\pntxta .\tab}}}}Visible\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = paragraph
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>();

        assert_eq!(text, "Visible");
        assert!(!document_text(&output.document).contains("Hidden"));
        assert!(!document_text(&output.document).contains("9.\t"));
    }

    #[test]
    fn word_list_tab_metadata_does_not_emit_justification_warning() {
        let output = parse_rtf(r"{\rtf1\pard\fi-360\li360\jclisttab\tx360 Item\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "Item");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("Japanese text justification"))
        );
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn prefixes_old_style_pn_marker_text_to_following_paragraph() {
        let output = parse_rtf_strict(
            r"{\rtf1{\pn\pndec\pnstart3{\pntxtb 3}{\pntxta .\tab}}Third item\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected old-style list paragraph"),
        };
        assert_eq!(paragraph.runs[0].text, "3.\tThird item");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn word_suppresses_standalone_old_style_pn_destination() {
        let output = parse_rtf(
            r"{\rtf1 Before\par{\pn\pndec\pnstart3\pnindent720\pnhang\pnsp360}Decimal item\par{\pn\pnucrm\pnstart4\pnindent720\pnhang}Roman item\par After\par}",
        )
        .unwrap();
        let paragraphs = output
            .document
            .blocks
            .iter()
            .map(|block| match block {
                Block::Paragraph(paragraph) => paragraph,
                _ => panic!("expected paragraphs"),
            })
            .collect::<Vec<_>>();

        assert_eq!(
            paragraph_plain_text(paragraphs[0]).as_deref(),
            Some("Before")
        );
        assert_eq!(
            paragraph_plain_text(paragraphs[1]).as_deref(),
            Some("Decimal item")
        );
        assert_eq!(
            paragraph_plain_text(paragraphs[2]).as_deref(),
            Some("Roman item")
        );
        assert_eq!(
            paragraph_plain_text(paragraphs[3]).as_deref(),
            Some("After")
        );
        for paragraph in paragraphs {
            assert_eq!(paragraph.style.left_indent_twips, 0);
            assert_eq!(paragraph.style.first_line_indent_twips, 0);
            assert!(paragraph.style.tab_stops_twips.is_empty());
        }
    }

    #[test]
    fn synthesizes_old_style_roman_marker_without_explicit_marker_text() {
        let output = parse_rtf_strict(r"{\rtf1{\pn\pnucrm\pnstart4}Fourth item\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected old-style list paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "IV.\tFourth item");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn synthesizes_old_style_alpha_ordinal_and_bullet_markers() {
        let output = parse_rtf_strict(
            r"{\rtf1{\pn\pnlcltr\pnstart28}Lower alpha\par{\pn\pnord\pnstart13}Ordinal\par{\pn\pnbul}Bullet\par}",
        )
        .unwrap();
        let paragraph_text = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph.runs[0].text.as_str(),
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph_text(0), "ab.\tLower alpha");
        assert_eq!(paragraph_text(1), "13th.\tOrdinal");
        assert_eq!(paragraph_text(2), "\u{2022}\tBullet");
    }

    #[test]
    fn applies_old_style_list_indent_controls_to_marker_paragraph() {
        let output =
            parse_rtf_strict(r"{\rtf1{\pn\pndec\pnstart2\pnindent720\pnhang}Indented\par}")
                .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected old-style list paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "2.\tIndented");
        assert_eq!(paragraph.style.left_indent_twips, 720);
        assert_eq!(paragraph.style.first_line_indent_twips, -360);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn applies_old_style_list_spacing_control_as_safe_tab_stop() {
        let output =
            parse_rtf_strict(r"{\rtf1{\pn\pndec\pnstart2\pnindent720\pnhang\pnsp360}Spaced\par}")
                .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected old-style list paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "2.\tSpaced");
        assert_eq!(paragraph.style.left_indent_twips, 720);
        assert_eq!(paragraph.style.first_line_indent_twips, -360);
        assert_eq!(paragraph.style.tab_stops_twips, vec![360]);
        assert_eq!(paragraph.style.tab_stop_leaders, vec![TabLeader::None]);
        assert_eq!(
            paragraph.style.tab_stop_alignments,
            vec![TabAlignment::Left]
        );
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn old_style_list_spacing_obeys_tab_stop_limit() {
        let options = RtfParseOptions {
            compatibility_mode: CompatibilityMode::StrictSpec,
            limits: RtfLimits {
                max_tab_stops: 0,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };

        assert!(matches!(
            parse_rtf_bytes_with_options(br"{\rtf1{\pn\pndec\pnsp360}Blocked\par}", &options),
            Err(ParseError::ResourceLimitExceeded { resource, .. }) if resource == "tab stops"
        ));
    }

    #[test]
    fn applies_old_style_list_marker_character_format_controls_to_marker_run() {
        let output = parse_rtf_strict(
            r"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Courier New;}}{\colortbl;\red255\green0\blue0;}{\pn\pndec\pnb\pni\pnul\pnstrike\pncaps\pncf1\pnf1\pnfs28}Formatted item\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected old-style list paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "1.\t");
        assert!(paragraph.runs[0].style.bold);
        assert!(paragraph.runs[0].style.italic);
        assert_eq!(paragraph.runs[0].style.underline, UnderlineStyle::Single);
        assert!(paragraph.runs[0].style.strike);
        assert!(paragraph.runs[0].style.all_caps);
        assert_eq!(paragraph.runs[0].style.color_index, 1);
        assert_eq!(paragraph.runs[0].style.font_index, 1);
        assert_eq!(paragraph.runs[0].style.font_size_half_points, 28);
        assert_eq!(paragraph.runs[1].text, "Formatted item");
        assert!(!paragraph.runs[1].style.bold);
        assert!(!paragraph.runs[1].style.italic);
        assert_eq!(paragraph.runs[1].style.underline, UnderlineStyle::None);
        assert!(!paragraph.runs[1].style.strike);
        assert_eq!(paragraph.runs[1].style.color_index, 0);
        assert_eq!(paragraph.runs[1].style.font_index, 0);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn prefixes_ignorable_listtext_marker_to_following_paragraph() {
        let output = parse_rtf(r"{\rtf1{\*\listtext \u8226?\tab}Bullet item\par}").unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected list paragraph"),
        };
        assert_eq!(paragraph.runs[0].text, "\u{2022}\tBullet item");
    }

    #[test]
    fn preserves_formatted_explicit_listtext_marker_style() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red255\green0\blue0;}{\*\listtext\b\cf1 1.\tab}Styled explicit\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected list paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "1.\t");
        assert!(paragraph.runs[0].style.bold);
        assert_eq!(paragraph.runs[0].style.color_index, 1);
        assert_eq!(paragraph.runs[1].text, "Styled explicit");
        assert!(!paragraph.runs[1].style.bold);
        assert_eq!(paragraph.runs[1].style.color_index, 0);
    }

    #[test]
    fn preserves_mixed_formatted_explicit_listtext_marker_runs() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red255\green0\blue0;}{\*\listtext\b 1\b0\cf1 .\cf0\tab}Styled explicit\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected list paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "1");
        assert!(paragraph.runs[0].style.bold);
        assert_eq!(paragraph.runs[0].style.color_index, 0);
        assert_eq!(paragraph.runs[1].text, ".");
        assert!(!paragraph.runs[1].style.bold);
        assert_eq!(paragraph.runs[1].style.color_index, 1);
        assert_eq!(paragraph.runs[2].text, "\tStyled explicit");
        assert!(!paragraph.runs[2].style.bold);
        assert_eq!(paragraph.runs[2].style.color_index, 0);
    }

    #[test]
    fn synthesizes_decimal_markers_from_list_tables() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 First\par\pard\ls1\ilvl0 Second\par}",
        )
        .unwrap();
        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(first.runs[0].text, "1.\tFirst");
        assert_eq!(second.runs[0].text, "2.\tSecond");
    }

    #[test]
    fn applies_list_level_indent_and_spacing_as_safe_paragraph_metadata() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1\levelindent720\levelspace1080{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Indented\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "1.\tIndented");
        assert_eq!(paragraph.style.left_indent_twips, 720);
        assert_eq!(paragraph.style.tab_stops_twips, vec![1080]);
        assert_eq!(paragraph.style.tab_stop_leaders, vec![TabLeader::None]);
        assert_eq!(
            paragraph.style.tab_stop_alignments,
            vec![TabAlignment::Left]
        );
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn applies_list_level_follow_controls_to_synthesized_marker_suffix() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1\levelfollow0{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid1}{\list{\listlevel\levelnfc0\levelstartat1\levelfollow1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid2}{\list{\listlevel\levelnfc0\levelstartat1\levelfollow2{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid3}}{\*\listoverridetable{\listoverride\listid1\ls1}{\listoverride\listid2\ls2}{\listoverride\listid3\ls3}}\pard\ls1\ilvl0 Tab\par\pard\ls2\ilvl0 Space\par\pard\ls3\ilvl0 Nothing\par}",
        )
        .unwrap();
        let paragraph_text = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph.runs[0].text.as_str(),
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph_text(0), "1.\tTab");
        assert_eq!(paragraph_text(1), "1. Space");
        assert_eq!(paragraph_text(2), "1.Nothing");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn applies_list_level_marker_character_formatting_to_marker_run() {
        let output = parse_rtf(
            r"{\rtf1{\fonttbl{\f0 Arial;}{\f1 Courier New;}}{\colortbl;\red255\green0\blue0;\red255\green255\blue0;\red0\green0\blue255;}{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1\f1\fs28\b\i\ul\ulc3\strike\caps\cf1\highlight2\chshdng5000{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Styled item\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "1.\t");
        assert!(paragraph.runs[0].style.bold);
        assert!(paragraph.runs[0].style.italic);
        assert_eq!(paragraph.runs[0].style.underline, UnderlineStyle::Single);
        assert!(paragraph.runs[0].style.strike);
        assert!(paragraph.runs[0].style.all_caps);
        assert_eq!(paragraph.runs[0].style.font_index, 1);
        assert_eq!(paragraph.runs[0].style.font_size_half_points, 28);
        assert_eq!(paragraph.runs[0].style.color_index, 1);
        assert_eq!(paragraph.runs[0].style.highlight_index, Some(2));
        assert_eq!(
            paragraph.runs[0].style.highlight_shading_basis_points,
            5_000
        );
        assert_eq!(paragraph.runs[0].style.underline_color_index, Some(3));
        assert_eq!(paragraph.runs[1].text, "Styled item");
        assert!(!paragraph.runs[1].style.bold);
        assert!(!paragraph.runs[1].style.italic);
        assert_eq!(paragraph.runs[1].style.underline, UnderlineStyle::None);
        assert!(!paragraph.runs[1].style.strike);
        assert_eq!(paragraph.runs[1].style.font_index, 0);
        assert_eq!(paragraph.runs[1].style.color_index, 0);
        assert_eq!(paragraph.runs[1].style.highlight_index, None);
        assert_eq!(paragraph.runs[1].style.underline_color_index, None);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn applies_list_level_marker_double_strike_to_marker_run_only() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1\strikedl{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Double strike item\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "1.\t");
        assert!(paragraph.runs[0].style.strike);
        assert!(paragraph.runs[0].style.double_strike);
        assert_eq!(paragraph.runs[1].text, "Double strike item");
        assert!(!paragraph.runs[1].style.strike);
        assert!(!paragraph.runs[1].style.double_strike);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn applies_list_level_marker_underline_variants_to_marker_runs_only() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1\uldb{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}{\list{\listlevel\levelnfc0\levelstartat1\ulth{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid6}{\list{\listlevel\levelnfc0\levelstartat1\uld{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid7}{\list{\listlevel\levelnfc0\levelstartat1\uldash{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid8}{\list{\listlevel\levelnfc0\levelstartat1\ulwave{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid9}{\list{\listlevel\levelnfc0\levelstartat1\ulw{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid10}}{\*\listoverridetable{\listoverride\listid5\ls1}{\listoverride\listid6\ls2}{\listoverride\listid7\ls3}{\listoverride\listid8\ls4}{\listoverride\listid9\ls5}{\listoverride\listid10\ls6}}\pard\ls1\ilvl0 Double marker\par\pard\ls2\ilvl0 Thick marker\par\pard\ls3\ilvl0 Dotted marker\par\pard\ls4\ilvl0 Dashed marker\par\pard\ls5\ilvl0 Wave marker\par\pard\ls6\ilvl0 Words marker\par}",
        )
        .unwrap();

        for (index, (text, underline)) in [
            ("Double marker", UnderlineStyle::Double),
            ("Thick marker", UnderlineStyle::Thick),
            ("Dotted marker", UnderlineStyle::Dotted),
            ("Dashed marker", UnderlineStyle::Dashed),
            ("Wave marker", UnderlineStyle::Wave),
            ("Words marker", UnderlineStyle::Words),
        ]
        .into_iter()
        .enumerate()
        {
            let paragraph = match &output.document.blocks[index] {
                Block::Paragraph(paragraph) => paragraph,
                _ => panic!("expected paragraph"),
            };

            assert_eq!(paragraph.runs[0].text, "1.\t");
            assert_eq!(paragraph.runs[0].style.underline, underline);
            assert_eq!(paragraph.runs[1].text, text);
            assert_eq!(paragraph.runs[1].style.underline, UnderlineStyle::None);
        }

        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn applies_list_level_marker_text_effects_to_marker_runs_only() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1\outl{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}{\list{\listlevel\levelnfc0\levelstartat1\shad{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid6}{\list{\listlevel\levelnfc0\levelstartat1\embo{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid7}{\list{\listlevel\levelnfc0\levelstartat1\impr{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid8}{\list{\listlevel\levelnfc0\levelstartat1\scaps{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid9}}{\*\listoverridetable{\listoverride\listid5\ls1}{\listoverride\listid6\ls2}{\listoverride\listid7\ls3}{\listoverride\listid8\ls4}{\listoverride\listid9\ls5}}\pard\ls1\ilvl0 Outline marker\par\pard\ls2\ilvl0 Shadow marker\par\pard\ls3\ilvl0 Emboss marker\par\pard\ls4\ilvl0 Engrave marker\par\pard\ls5\ilvl0 Small caps marker\par}",
        )
        .unwrap();

        let marker_style = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => {
                assert_eq!(paragraph.runs[0].text, "1.\t");
                assert_eq!(paragraph.runs[1].style, CharacterStyle::default());
                &paragraph.runs[0].style
            }
            _ => panic!("expected paragraph"),
        };

        assert!(marker_style(0).outline);
        assert!(marker_style(1).shadow);
        assert_eq!(marker_style(2).relief, TextRelief::Emboss);
        assert_eq!(marker_style(3).relief, TextRelief::Engrave);
        assert!(marker_style(4).small_caps);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn applies_list_level_marker_emphasis_marks_to_marker_runs_only() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1\accdot{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}{\list{\listlevel\levelnfc0\levelstartat1\acccomma{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid6}{\list{\listlevel\levelnfc0\levelstartat1\accdot\accnone{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid7}}{\*\listoverridetable{\listoverride\listid5\ls1}{\listoverride\listid6\ls2}{\listoverride\listid7\ls3}}\pard\ls1\ilvl0 Dot marker\par\pard\ls2\ilvl0 Comma marker\par\pard\ls3\ilvl0\b Plain marker\par}",
        )
        .unwrap();

        let marker_style = |index: usize, text: &str| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => {
                assert_eq!(paragraph.runs[0].text, "1.\t");
                assert_eq!(paragraph.runs[1].text, text);
                assert_eq!(
                    paragraph.runs[1].style.emphasis_mark,
                    CharacterEmphasisMark::None
                );
                paragraph.runs[0].style.emphasis_mark
            }
            _ => panic!("expected paragraph"),
        };

        assert_eq!(marker_style(0, "Dot marker"), CharacterEmphasisMark::Dot);
        assert_eq!(
            marker_style(1, "Comma marker"),
            CharacterEmphasisMark::Comma
        );
        assert_eq!(marker_style(2, "Plain marker"), CharacterEmphasisMark::None);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn applies_list_level_marker_script_and_spacing_to_marker_runs_only() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1\super{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}{\list{\listlevel\levelnfc0\levelstartat1\sub{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid6}{\list{\listlevel\levelnfc0\levelstartat1\up8{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid7}{\list{\listlevel\levelnfc0\levelstartat1\dn6{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid8}{\list{\listlevel\levelnfc0\levelstartat1\expndtw80{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid9}{\list{\listlevel\levelnfc0\levelstartat1\kerning2{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid10}{\list{\listlevel\levelnfc0\levelstartat1\charscalex150{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid11}}{\*\listoverridetable{\listoverride\listid5\ls1}{\listoverride\listid6\ls2}{\listoverride\listid7\ls3}{\listoverride\listid8\ls4}{\listoverride\listid9\ls5}{\listoverride\listid10\ls6}{\listoverride\listid11\ls7}}\pard\ls1\ilvl0 Raised marker\par\pard\ls2\ilvl0 Lowered marker\par\pard\ls3\ilvl0 Manual up marker\par\pard\ls4\ilvl0 Manual down marker\par\pard\ls5\ilvl0 Spaced marker\par\pard\ls6\ilvl0 Kerned marker\par\pard\ls7\ilvl0 Scaled marker\par}",
        )
        .unwrap();

        let marker_style = |index: usize, text: &str| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => {
                assert_eq!(paragraph.runs[0].text, "1.\t");
                assert_eq!(paragraph.runs[1].text, text);
                assert_eq!(paragraph.runs[1].style, CharacterStyle::default());
                &paragraph.runs[0].style
            }
            _ => panic!("expected paragraph"),
        };

        let raised = marker_style(0, "Raised marker");
        assert_eq!(
            raised.baseline_shift_half_points,
            DEFAULT_SUPERSCRIPT_SHIFT_HALF_POINTS
        );
        assert_eq!(
            raised.font_size_scale_percent,
            DEFAULT_SCRIPT_FONT_SCALE_PERCENT
        );
        let lowered = marker_style(1, "Lowered marker");
        assert_eq!(
            lowered.baseline_shift_half_points,
            DEFAULT_SUBSCRIPT_SHIFT_HALF_POINTS
        );
        assert_eq!(
            lowered.font_size_scale_percent,
            DEFAULT_SCRIPT_FONT_SCALE_PERCENT
        );
        assert_eq!(
            marker_style(2, "Manual up marker").baseline_shift_half_points,
            8
        );
        assert_eq!(
            marker_style(3, "Manual down marker").baseline_shift_half_points,
            -6
        );
        assert_eq!(marker_style(4, "Spaced marker").character_spacing_twips, 80);
        assert_eq!(
            marker_style(5, "Kerned marker").character_kerning_half_points,
            2
        );
        assert_eq!(
            marker_style(6, "Scaled marker").character_scaling_percent,
            150
        );
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn applies_list_level_marker_character_border_to_marker_run_only() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red255\green0\blue0;}{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1\chbrdr\brdrdash\brdrw80\brdrcf1\brsp120{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Bordered marker\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "1.\t");
        assert!(paragraph.runs[0].style.border.visible);
        assert_eq!(paragraph.runs[0].style.border.style, BorderStyle::Dashed);
        assert_eq!(paragraph.runs[0].style.border.width_twips, 80);
        assert_eq!(paragraph.runs[0].style.border.color_index, Some(1));
        assert_eq!(paragraph.runs[0].style.border.spacing_twips, 120);
        assert_eq!(paragraph.runs[1].text, "Bordered marker");
        assert!(!paragraph.runs[1].style.border.visible);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn list_level_marker_plain_resets_accumulated_marker_character_style() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red255\green0\blue0;}{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1\b\ul\chbrdr\brdrs\brdrw80\plain\i\cf1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Reset item\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "1.\t");
        assert!(!paragraph.runs[0].style.bold);
        assert!(paragraph.runs[0].style.italic);
        assert_eq!(paragraph.runs[0].style.underline, UnderlineStyle::None);
        assert_eq!(paragraph.runs[0].style.color_index, 1);
        assert!(!paragraph.runs[0].style.border.visible);
        assert_eq!(paragraph.runs[1].text, "Reset item");
        assert!(!paragraph.runs[1].style.bold);
        assert!(!paragraph.runs[1].style.italic);
        assert_eq!(paragraph.runs[1].style.color_index, 0);
        assert!(!paragraph.runs[1].style.border.visible);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn list_level_marker_character_style_applies_to_marker_run_only() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red255\green0\blue0;}{\stylesheet{\cs5\b\ul\cf1 Marker emphasis;}}{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1\i\cs5{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Styled marker\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "1.\t");
        assert!(paragraph.runs[0].style.bold);
        assert!(paragraph.runs[0].style.italic);
        assert_eq!(paragraph.runs[0].style.underline, UnderlineStyle::Single);
        assert_eq!(paragraph.runs[0].style.color_index, 1);
        assert_eq!(paragraph.runs[1].text, "Styled marker");
        assert!(!paragraph.runs[1].style.bold);
        assert!(!paragraph.runs[1].style.italic);
        assert_eq!(paragraph.runs[1].style.underline, UnderlineStyle::None);
        assert_eq!(paragraph.runs[1].style.color_index, 0);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn list_level_spacing_obeys_tab_stop_limit() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_tab_stops: 0,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };

        assert!(matches!(
            parse_rtf_bytes_with_options(
                br"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelspace1080{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Blocked\par}",
                &options
            ),
            Err(ParseError::ResourceLimitExceeded { resource, .. }) if resource == "tab stops"
        ));
    }

    #[test]
    fn list_override_start_values_restart_synthesized_markers() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5{\lfolevel\listoverridestartat\levelstartat3}\ls1}}\pard\ls1\ilvl0 First\par\pard\ls1\ilvl0 Second\par}",
        )
        .unwrap();
        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(first.runs[0].text, "3.\tFirst");
        assert_eq!(second.runs[0].text, "4.\tSecond");
    }

    #[test]
    fn list_override_format_replaces_synthesized_marker_definition() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5{\lfolevel\listoverrideformat{\listlevel\levelnfc4\levelstartat3{\leveltext\'02\'00);}{\levelnumbers\'01;}}}\ls1}}\pard\ls1\ilvl0 First\par\pard\ls1\ilvl0 Second\par}",
        )
        .unwrap();
        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(first.runs[0].text, "c)\tFirst");
        assert_eq!(second.runs[0].text, "d)\tSecond");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn list_override_format_applies_marker_character_style_only() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red255\green0\blue0;}{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5{\lfolevel\listoverrideformat{\listlevel\levelnfc4\b\cf1\levelstartat3{\leveltext\'02\'00);}{\levelnumbers\'01;}}}\ls1}}\pard\ls1\ilvl0 Styled override\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "c)\t");
        assert!(paragraph.runs[0].style.bold);
        assert_eq!(paragraph.runs[0].style.color_index, 1);
        assert_eq!(paragraph.runs[1].text, "Styled override");
        assert!(!paragraph.runs[1].style.bold);
        assert_eq!(paragraph.runs[1].style.color_index, 0);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn list_level_legal_numbering_renders_parent_placeholders_as_decimal() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc1\levelstartat4{\leveltext\'02\'00.;}{\levelnumbers\'01;}}{\listlevel\levelnfc0\levellegal1\levelstartat1{\leveltext\'04\'00.\'01.;}{\levelnumbers\'01\'03;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Parent\par\pard\ls1\ilvl1 Child\par}",
        )
        .unwrap();
        let paragraph_text = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph.runs[0].text.as_str(),
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph_text(0), "IV.\tParent");
        assert_eq!(paragraph_text(1), "4.1.\tChild");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn list_level_no_restart_preserves_lower_level_counter_across_parent_items() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}{\listlevel\levelnfc0\levelnorestart1\levelstartat1{\leveltext\'04\'00.\'01.;}{\levelnumbers\'01\'03;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Top\par\pard\ls1\ilvl1 Child\par\pard\ls1\ilvl0 Next top\par\pard\ls1\ilvl1 Continued child\par}",
        )
        .unwrap();
        let paragraph_text = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph.runs[0].text.as_str(),
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph_text(0), "1.\tTop");
        assert_eq!(paragraph_text(1), "1.1.\tChild");
        assert_eq!(paragraph_text(2), "2.\tNext top");
        assert_eq!(paragraph_text(3), "2.2.\tContinued child");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn synthesizes_roman_and_alpha_markers_from_list_tables() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc1\levelstartat4{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid10}{\list{\listlevel\levelnfc2\levelstartat9{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid11}{\list{\listlevel\levelnfc3\levelstartat27{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid12}{\list{\listlevel\levelnfc4\levelstartat28{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid13}}{\*\listoverridetable{\listoverride\listid10\ls10}{\listoverride\listid11\ls11}{\listoverride\listid12\ls12}{\listoverride\listid13\ls13}}\pard\ls10\ilvl0 Upper roman\par\pard\ls10\ilvl0 Upper roman next\par\pard\ls11\ilvl0 Lower roman\par\pard\ls12\ilvl0 Upper alpha\par\pard\ls13\ilvl0 Lower alpha\par}",
        )
        .unwrap();
        let paragraph_text = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph.runs[0].text.as_str(),
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph_text(0), "IV.\tUpper roman");
        assert_eq!(paragraph_text(1), "V.\tUpper roman next");
        assert_eq!(paragraph_text(2), "ix.\tLower roman");
        assert_eq!(paragraph_text(3), "AA.\tUpper alpha");
        assert_eq!(paragraph_text(4), "ab.\tLower alpha");
    }

    #[test]
    fn synthesizes_ordinal_markers_from_list_tables() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc5\levelstartat1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid14}}{\*\listoverridetable{\listoverride\listid14\ls14}}\pard\ls14\ilvl0 First\par\pard\ls14\ilvl0 Second\par\pard\ls14\ilvl0 Third\par\pard\ls14\ilvl0 Fourth\par}",
        )
        .unwrap();
        let paragraph_text = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph.runs[0].text.as_str(),
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph_text(0), "1st.\tFirst");
        assert_eq!(paragraph_text(1), "2nd.\tSecond");
        assert_eq!(paragraph_text(2), "3rd.\tThird");
        assert_eq!(paragraph_text(3), "4th.\tFourth");
    }

    #[test]
    fn synthesizes_cardinal_and_ordinal_text_markers_from_list_tables() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc6\levelstartat21{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid21}{\list{\listlevel\levelnfc7\levelstartat22{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid22}{\list{\listlevel\levelnfc6\levelstartat1000{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid23}}{\*\listoverridetable{\listoverride\listid21\ls21}{\listoverride\listid22\ls22}{\listoverride\listid23\ls23}}\pard\ls21\ilvl0 Cardinal text\par\pard\ls22\ilvl0 Ordinal text\par\pard\ls23\ilvl0 Fallback text\par}",
        )
        .unwrap();
        let paragraph_text = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph.runs[0].text.as_str(),
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph_text(0), "twenty-one.\tCardinal text");
        assert_eq!(paragraph_text(1), "twenty-second.\tOrdinal text");
        assert_eq!(paragraph_text(2), "1000.\tFallback text");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn synthesizes_cardinal_and_ordinal_text_old_style_markers() {
        let output = parse_rtf_strict(
            r"{\rtf1{\pn\pncard\pnstart21}Cardinal old style\par{\pn\pnordt\pnstart22}Ordinal old style\par{\pn\pncard\pnstart1000}Fallback old style\par}",
        )
        .unwrap();
        let paragraph_text = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph.runs[0].text.as_str(),
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph_text(0), "twenty-one.\tCardinal old style");
        assert_eq!(paragraph_text(1), "twenty-second.\tOrdinal old style");
        assert_eq!(paragraph_text(2), "1000.\tFallback old style");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn ordinal_list_markers_handle_teen_suffix_exceptions() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc5\levelstartat11{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid15}}{\*\listoverridetable{\listoverride\listid15\ls15}}\pard\ls15\ilvl0 Eleventh\par\pard\ls15\ilvl0 Twelfth\par\pard\ls15\ilvl0 Thirteenth\par\pard\ls15\ilvl0 Fourteenth\par}",
        )
        .unwrap();
        let paragraph_text = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph.runs[0].text.as_str(),
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph_text(0), "11th.\tEleventh");
        assert_eq!(paragraph_text(1), "12th.\tTwelfth");
        assert_eq!(paragraph_text(2), "13th.\tThirteenth");
        assert_eq!(paragraph_text(3), "14th.\tFourteenth");
    }

    #[test]
    fn synthesizes_zero_padded_decimal_markers_from_list_tables() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc22\levelstartat1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid16}{\list{\listlevel\levelnfc62\levelstartat1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid17}{\list{\listlevel\levelnfc63\levelstartat1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid18}{\list{\listlevel\levelnfc64\levelstartat1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid19}}{\*\listoverridetable{\listoverride\listid16\ls16}{\listoverride\listid17\ls17}{\listoverride\listid18\ls18}{\listoverride\listid19\ls19}}\pard\ls16\ilvl0 Two digits\par\pard\ls17\ilvl0 Three digits\par\pard\ls18\ilvl0 Four digits\par\pard\ls19\ilvl0 Five digits\par}",
        )
        .unwrap();
        let paragraph_text = |index: usize| match &output.document.blocks[index] {
            Block::Paragraph(paragraph) => paragraph.runs[0].text.as_str(),
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph_text(0), "01.\tTwo digits");
        assert_eq!(paragraph_text(1), "001.\tThree digits");
        assert_eq!(paragraph_text(2), "0001.\tFour digits");
        assert_eq!(paragraph_text(3), "00001.\tFive digits");
    }

    #[test]
    fn oversized_roman_list_markers_fall_back_to_bounded_decimal() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc1\levelstartat5000{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid20}}{\*\listoverridetable{\listoverride\listid20\ls20}}\pard\ls20\ilvl0 Big roman\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "5000.\tBig roman");
    }

    #[test]
    fn synthesizes_bullet_markers_from_list_tables() {
        let output = parse_rtf(
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc23{\leveltext\'01\u8226 ?;}{\levelnumbers;}}\listid7}}{\*\listoverridetable{\listoverride\listid7\ls2}}\pard\ls2\ilvl0 Bullet\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(
            paragraph
                .runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>(),
            "\u{2022}\tBullet"
        );
    }

    #[test]
    fn normalizes_symbol_private_use_bullets_from_list_tables() {
        let output = parse_rtf(
            r"{\rtf1{\fonttbl{\f0 Arial;}{\f1\fcharset2 Symbol;}}{\*\listtable{\list{\listlevel\levelnfc23{\leveltext\'01\u-3913 ?;}{\levelnumbers;}\f1}\listid7}}{\*\listoverridetable{\listoverride\listid7\ls2}}\pard\ls2\ilvl0 Bullet\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(
            paragraph
                .runs
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>(),
            "\u{2022}\tBullet"
        );
    }

    #[test]
    fn list_level_template_length_ignores_trailing_metadata() {
        for input in [
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1{\leveltext\'02\'00.;Hidden payload;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Hex item\par}",
            r"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1{\leveltext\u2?\u0?.;Hidden payload;}{\levelnumbers\'01;}}\listid5}}{\*\listoverridetable{\listoverride\listid5\ls1}}\pard\ls1\ilvl0 Unicode item\par}",
        ] {
            let output = parse_rtf(input).unwrap();
            let text = document_text(&output.document);

            assert!(
                text.contains("1.\t"),
                "list level template length should preserve authored marker: {text:?}"
            );
            for forbidden in [
                "Hidden payload",
                "leveltext",
                "levelnumbers",
                "listtable",
                "u2",
                "u0",
            ] {
                assert!(
                    !text.contains(forbidden),
                    "trailing list template metadata leaked to visible text: {forbidden}"
                );
            }
        }
    }

    #[test]
    fn list_table_metadata_controls_do_not_pollute_synthesized_markers() {
        let output = parse_rtf(
            r#"{\rtf1{\*\listtable{\list\listtemplateid67698689\listsimple{\listlevel\levelnfc23\leveljc0{\leveltext\'01\u8226 ?;}{\levelnumbers{\object\objdata 414243};}}{\listname Hidden list name{\field{\*\fldinst HYPERLINK "https://example.com/list-name"}{\fldrslt Hidden link}}{\object\objdata 414243};}\listid7}}{\*\listoverridetable{\listoverride\listid7\listoverridecount0\ls2}}\pard\ls2\ilvl0 Bullet\par}"#,
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let text = document_text(&output.document);

        assert_eq!(paragraph.runs[0].text, "\u{2022}\tBullet");
        assert!(!text.contains("Hidden list name"));
        assert!(!text.contains("Hidden link"));
        assert!(!text.contains("https://example.com/list-name"));
        assert!(!text.contains("[Field removed"));
        assert!(!text.contains("objdata"));
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| { !diagnostic.message.contains("unsupported RTF control") })
        );
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("active content removed: field instruction in metadata")
        }));
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("active content removed: object payload in metadata")
        }));
    }

    #[test]
    fn list_definition_count_limit_is_enforced() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_lists: 1,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let error = parse_rtf_bytes_with_options(
            br"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0{\leveltext\'02\'00.;}}\listid1}{\list{\listlevel\levelnfc0{\leveltext\'02\'00.;}}\listid2}}Body\par}",
            &options,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ParseError::ResourceLimitExceeded { resource, .. } if resource == "lists"
        ));
    }

    #[test]
    fn list_limit_is_independent_from_styles() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_lists: 2,
                max_styles: 0,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0\levelstartat1{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid1}}{\*\listoverridetable{\listoverride\listid1\ls1}}\pard\ls1\ilvl0 Body\par}",
            &options,
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };

        assert_eq!(paragraph.runs[0].text, "1.\tBody");
    }

    #[test]
    fn list_limit_bounds_combined_definitions_and_overrides() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_lists: 1,
                max_styles: 0,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };
        let error = parse_rtf_bytes_with_options(
            br"{\rtf1{\*\listtable{\list{\listlevel\levelnfc0{\leveltext\'02\'00.;}{\levelnumbers\'01;}}\listid1}}{\*\listoverridetable{\listoverride\listid1\ls1}}Body\par}",
            &options,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            ParseError::ResourceLimitExceeded { resource, .. } if resource == "list overrides"
        ));
    }

    #[test]
    fn document_property_limit_is_independent_from_styles() {
        let options = RtfParseOptions {
            limits: RtfLimits {
                max_document_properties: 1,
                max_styles: 0,
                ..RtfLimits::default()
            },
            ..RtfParseOptions::default()
        };

        let output = parse_rtf_bytes_with_options(
            br#"{\rtf1{\*\docvar {Client}{Contoso}}Client {\field{\*\fldinst DOCVARIABLE Client}}\par}"#,
            &options,
        )
        .unwrap();
        let text = document_text(&output.document);

        assert!(
            text.contains("Client Contoso"),
            "metadata fields should render despite max_styles=0: {text:?}"
        );
    }

    #[test]
    fn normalizes_word_underline_style_controls() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red0\green0\blue0;\red255\green0\blue0;}{\ul single} {\ulw words only} {\uldb double} {\ulth thick} {\uld dotted} {\uldash dashed} {\ulwave wave} {\ulc2 colored} {\ulnone plain}\par}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let style_for = |text: &str| {
            paragraph
                .runs
                .iter()
                .find(|run| run.text.trim() == text)
                .map(|run| &run.style)
                .unwrap_or_else(|| panic!("missing run {text}"))
        };

        assert_eq!(style_for("single").underline, UnderlineStyle::Single);
        assert_eq!(style_for("words only").underline, UnderlineStyle::Words);
        assert_eq!(style_for("double").underline, UnderlineStyle::Double);
        assert_eq!(style_for("thick").underline, UnderlineStyle::Thick);
        assert_eq!(style_for("dotted").underline, UnderlineStyle::Dotted);
        assert_eq!(style_for("dashed").underline, UnderlineStyle::Dashed);
        assert_eq!(style_for("wave").underline, UnderlineStyle::Wave);
        assert_eq!(style_for("colored").underline_color_index, Some(2));
        assert_eq!(style_for("plain").underline, UnderlineStyle::None);
    }

    #[test]
    fn normalizes_page_break_before_paragraph_style() {
        let output = parse_rtf(r"{\rtf1 First\par\pagebb Second\par\pagebb0 Third\par}").unwrap();

        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected second paragraph, got {other:?}"),
        };
        assert!(second.style.page_break_before);

        let third = match &output.document.blocks[2] {
            Block::Paragraph(paragraph) => paragraph,
            other => panic!("expected third paragraph, got {other:?}"),
        };
        assert!(!third.style.page_break_before);
    }

    #[test]
    fn normalizes_hex_color_table_separators() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red255\green0\blue0\'3b\red0\green255\blue0\'3b}{\cf1 Red} {\highlight2 Marked}\par}",
        )
        .unwrap();
        assert_eq!(
            output.document.colors[1],
            Color {
                red: 255,
                green: 0,
                blue: 0
            }
        );
        assert_eq!(
            output.document.colors[2],
            Color {
                red: 0,
                green: 255,
                blue: 0
            }
        );

        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let red = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "Red")
            .expect("red run");
        assert_eq!(red.style.color_index, 1);
        let marked = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "Marked")
            .expect("marked run");
        assert_eq!(marked.style.highlight_index, Some(2));
    }

    #[test]
    fn normalizes_unicode_color_table_separators() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red255\green0\blue0\u59?\red0\green255\blue0\u59?}{\cf1 Red} {\highlight2 Marked}\par}",
        )
        .unwrap();
        assert_eq!(
            output.document.colors[1],
            Color {
                red: 255,
                green: 0,
                blue: 0
            }
        );
        assert_eq!(
            output.document.colors[2],
            Color {
                red: 0,
                green: 255,
                blue: 0
            }
        );

        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let red = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "Red")
            .expect("red run");
        assert_eq!(red.style.color_index, 1);
        let marked = paragraph
            .runs
            .iter()
            .find(|run| run.text.trim() == "Marked")
            .expect("marked run");
        assert_eq!(marked.style.highlight_index, Some(2));
    }

    #[test]
    fn normalizes_paragraph_and_table_shading_intensity_controls() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red255\green0\blue0;}\cbpat1\shading2500 paragraph\par\trowd\trcbpat1\trshdng5000\cellx1440 row\cell\clcbpat1\clshdng7500\cellx2880 cell\cell\row}",
        )
        .unwrap();

        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        assert_eq!(paragraph.style.shading_color_index, Some(1));
        assert_eq!(paragraph.style.shading_basis_points, 2_500);

        let table = match &output.document.blocks[1] {
            Block::Table(table) => table,
            _ => panic!("expected table"),
        };
        assert_eq!(table.rows[0].cells[0].shading_color_index, Some(1));
        assert_eq!(table.rows[0].cells[0].shading_basis_points, 5_000);
        assert_eq!(table.rows[0].cells[1].shading_color_index, Some(1));
        assert_eq!(table.rows[0].cells[1].shading_basis_points, 7_500);
    }

    #[test]
    fn normalizes_paragraph_and_table_shading_pattern_controls() {
        let output = parse_rtf(
            r"{\rtf1{\colortbl;\red255\green0\blue0;\red0\green0\blue255;}\cbpat1\cfpat2\shading2500\bghoriz paragraph\par\trowd\trcbpat1\trcfpat2\trshdng5000\trbgvert\cellx1440 row\cell\clcbpat2\clcfpat1\clshdng7500\clbghoriz\cellx2880 cell\cell\row}",
        )
        .unwrap();

        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        assert_eq!(paragraph.style.shading_color_index, Some(1));
        assert_eq!(paragraph.style.shading_basis_points, 2_500);
        assert_eq!(
            paragraph.style.shading_pattern,
            ShadingPattern::Horizontal.with_foreground_color_index(Some(2))
        );

        let table = match &output.document.blocks[1] {
            Block::Table(table) => table,
            _ => panic!("expected table"),
        };
        assert_eq!(table.rows[0].cells[0].shading_color_index, Some(1));
        assert_eq!(table.rows[0].cells[0].shading_basis_points, 5_000);
        assert_eq!(
            table.rows[0].cells[0].shading_pattern,
            ShadingPattern::Vertical.with_foreground_color_index(Some(2))
        );
        assert_eq!(table.rows[0].cells[1].shading_color_index, Some(2));
        assert_eq!(table.rows[0].cells[1].shading_basis_points, 7_500);
        assert_eq!(
            table.rows[0].cells[1].shading_pattern,
            ShadingPattern::Horizontal.with_foreground_color_index(Some(1))
        );
        for forbidden in ["bghoriz", "trbgvert", "clbghoriz"] {
            assert!(
                !document_text(&output.document).contains(forbidden),
                "shading pattern control leaked to text: {forbidden}"
            );
        }
    }

    #[test]
    fn clamps_extreme_paragraph_and_table_shading_intensity_controls() {
        let output = parse_rtf_bytes(
            br"{\rtf1{\colortbl;\red255\green0\blue0;}\cbpat1\shading-200 low\par\pard\cbpat1\shading99999 high\par\trowd\trcbpat1\trshdng99999\cellx1440 row\cell\clcbpat1\clshdng-50\cellx2880 cell\cell\row}",
        )
        .unwrap();

        let low = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected low paragraph"),
        };
        let high = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected high paragraph"),
        };
        assert_eq!(low.style.shading_basis_points, 0);
        assert_eq!(high.style.shading_basis_points, 10_000);

        let table = match &output.document.blocks[2] {
            Block::Table(table) => table,
            _ => panic!("expected table"),
        };
        assert_eq!(table.rows[0].cells[0].shading_basis_points, 10_000);
        assert_eq!(table.rows[0].cells[1].shading_basis_points, 0);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("paragraph shading clamped") })
        );
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("table row shading clamped") })
        );
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("table cell shading clamped") })
        );
    }

    #[test]
    fn normalizes_border_style_controls() {
        let output = parse_rtf(
            r"{\rtf1\brdrb\brdrdb Double paragraph\par\pard\chbrdr\brdrdash Dashed character\par\trowd\clbrdrl\brdrdot\cellx1440 dotted\cell\row}",
        )
        .unwrap();

        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let character = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => &paragraph.runs[0].style,
            _ => panic!("expected paragraph"),
        };
        let table = match &output.document.blocks[2] {
            Block::Table(table) => table,
            _ => panic!("expected table"),
        };

        assert_eq!(paragraph.style.borders.bottom.style, BorderStyle::Double);
        assert_eq!(character.border.style, BorderStyle::Dashed);
        assert_eq!(
            table.rows[0].cells[0].borders.left.style,
            BorderStyle::Dotted
        );
    }

    #[test]
    fn normalizes_extended_word_border_style_controls() {
        let output = parse_rtf(
            r"{\rtf1\box\brdrhair Hairline paragraph\par\pard\brdrb\brdrdashdot Dash dot paragraph\par\pard\brdrt\brdrtriple Triple paragraph\par\pard\brdrl\brdrinset Inset paragraph\par\pard\brdrr\brdroutset Outset paragraph\par\pard\brdrb\brdrengrave Engrave paragraph\par\pard\brdrt\brdremboss Emboss paragraph\par\pard\brdrl\brdrtnthsg Thin thick paragraph\par\pard\brdrr\brdrthtnmg Thick thin paragraph\par\pard\brdrb\brdrtnthtnlg Thin thick thin paragraph\par\pard\brdrr\brdrs\brdrsh Shadow paragraph\par\trowd\clbrdrl\brdrdashdd\cellx1440 dashdd\cell\row}",
        )
        .unwrap();
        let first = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let second = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let third = match &output.document.blocks[2] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let fourth = match &output.document.blocks[3] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let fifth = match &output.document.blocks[4] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let sixth = match &output.document.blocks[5] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let seventh = match &output.document.blocks[6] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let eighth = match &output.document.blocks[7] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let ninth = match &output.document.blocks[8] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let tenth = match &output.document.blocks[9] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let eleventh = match &output.document.blocks[10] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let table = match &output.document.blocks[11] {
            Block::Table(table) => table,
            _ => panic!("expected table"),
        };

        assert_eq!(first.style.borders.left.style, BorderStyle::Hairline);
        assert_eq!(first.style.borders.right.style, BorderStyle::Hairline);
        assert_eq!(first.style.borders.top.style, BorderStyle::Hairline);
        assert_eq!(first.style.borders.bottom.style, BorderStyle::Hairline);
        assert_eq!(second.style.borders.bottom.style, BorderStyle::Dashed);
        assert_eq!(third.style.borders.top.style, BorderStyle::Triple);
        assert_eq!(fourth.style.borders.left.style, BorderStyle::Engrave);
        assert_eq!(fifth.style.borders.right.style, BorderStyle::Emboss);
        assert_eq!(sixth.style.borders.bottom.style, BorderStyle::Engrave);
        assert_eq!(seventh.style.borders.top.style, BorderStyle::Emboss);
        assert_eq!(eighth.style.borders.left.style, BorderStyle::ThinThick);
        assert_eq!(ninth.style.borders.right.style, BorderStyle::ThickThin);
        assert_eq!(tenth.style.borders.bottom.style, BorderStyle::ThinThickThin);
        assert_eq!(eleventh.style.borders.right.style, BorderStyle::Emboss);
        assert_eq!(
            table.rows[0].cells[0].borders.left.style,
            BorderStyle::Dashed
        );
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("Word border effect \\brdrsh rendered as bounded passive relief border")
        }));
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("approximated as passive")),
            "extended border styles should normalize without border-style approximation: {:?}",
            output.diagnostics
        );
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control")),
            "extended border variants should not be unsupported: {:?}",
            output.diagnostics
        );
    }

    #[test]
    fn normalizes_wavy_word_border_style_controls() {
        let output = parse_rtf(
            r"{\rtf1\brdrb\brdrwavy Wavy paragraph\par\pard\chbrdr\brdrwavydb Wavy character\par\trowd\clbrdrl\brdrwavy\cellx1440 wavy cell\cell\row}",
        )
        .unwrap();
        let paragraph = match &output.document.blocks[0] {
            Block::Paragraph(paragraph) => paragraph,
            _ => panic!("expected paragraph"),
        };
        let character = match &output.document.blocks[1] {
            Block::Paragraph(paragraph) => &paragraph.runs[0].style,
            _ => panic!("expected paragraph"),
        };
        let table = match &output.document.blocks[2] {
            Block::Table(table) => table,
            _ => panic!("expected table"),
        };

        assert_eq!(paragraph.style.borders.bottom.style, BorderStyle::Wavy);
        assert_eq!(character.border.style, BorderStyle::Wavy);
        assert_eq!(table.rows[0].cells[0].borders.left.style, BorderStyle::Wavy);
    }

    #[test]
    fn strict_spec_normalizes_table_row_border_controls_as_cell_perimeter_borders() {
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1{\colortbl;\red255\green0\blue0;}\trowd\trbrdrt\brdrdb\brdrw80\brdrcf1\trbrdrl\brdrdash\brdrw40\trbrdrr\brdrdot\brdrw60\cellx1440 A\cell\cellx2880 B\cell\row}",
            &RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table"),
        };
        let first = &table.rows[0].cells[0];
        let second = &table.rows[0].cells[1];

        assert_eq!(first.borders.top.style, BorderStyle::Double);
        assert_eq!(first.borders.top.width_twips, 80);
        assert_eq!(first.borders.top.color_index, Some(1));
        assert_eq!(second.borders.top.style, BorderStyle::Double);
        assert_eq!(second.borders.top.width_twips, 80);
        assert_eq!(second.borders.top.color_index, Some(1));
        assert_eq!(first.borders.left.style, BorderStyle::Dashed);
        assert_eq!(first.borders.left.width_twips, 40);
        assert_eq!(second.borders.left.style, BorderStyle::Single);
        assert_eq!(first.borders.right.style, BorderStyle::Single);
        assert_eq!(second.borders.right.style, BorderStyle::Dotted);
        assert_eq!(second.borders.right.width_twips, 60);
    }

    #[test]
    fn strict_spec_normalizes_table_row_inner_border_controls_as_safe_cell_borders() {
        let output = parse_rtf_bytes_with_options(
            br"{\rtf1\trowd\trbrdrh\brdrdb\brdrw80\trbrdrv\brdrdot\brdrw60\cellx1000 A\cell\cellx2000 B\cell\cellx3000 C\cell\row}",
            &RtfParseOptions {
                compatibility_mode: CompatibilityMode::StrictSpec,
                ..RtfParseOptions::default()
            },
        )
        .unwrap();
        let table = match &output.document.blocks[0] {
            Block::Table(table) => table,
            _ => panic!("expected table"),
        };
        let first = &table.rows[0].cells[0];
        let second = &table.rows[0].cells[1];
        let third = &table.rows[0].cells[2];

        assert_eq!(first.borders.bottom.style, BorderStyle::Double);
        assert_eq!(first.borders.bottom.width_twips, 80);
        assert_eq!(second.borders.bottom.style, BorderStyle::Double);
        assert_eq!(second.borders.bottom.width_twips, 80);
        assert_eq!(third.borders.bottom.style, BorderStyle::Double);
        assert_eq!(third.borders.bottom.width_twips, 80);
        assert_eq!(first.borders.right.style, BorderStyle::Dotted);
        assert_eq!(first.borders.right.width_twips, 60);
        assert_eq!(second.borders.right.style, BorderStyle::Dotted);
        assert_eq!(second.borders.right.width_twips, 60);
        assert_eq!(third.borders.right.style, BorderStyle::Single);
        assert_ne!(third.borders.right.width_twips, 60);
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("unsupported RTF control"))
        );
    }

    #[test]
    fn word_compatible_ignores_row_borders_but_keeps_cell_borders() {
        let output = parse_rtf(
            r"{\rtf1\trowd\trbrdrb\brdrs\brdrw20\cellx1200\cellx2400 A\cell B\cell\row\trowd\trbrdrb\brdrs\brdrw20\cellx1200\clbrdrr\brdrs\brdrw30\cellx2400 C\cell D\cell\row}",
        )
        .unwrap();
        let Block::Table(table) = &output.document.blocks[0] else {
            panic!("expected table");
        };

        assert!(
            table.rows[0]
                .cells
                .iter()
                .all(|cell| !cell.borders.bottom.visible)
        );
        assert!(!table.rows[1].cells[0].borders.bottom.visible);
        assert!(!table.rows[1].cells[1].borders.bottom.visible);
        assert!(table.rows[1].cells[1].borders.right.visible);
        assert_eq!(table.rows[1].cells[1].borders.right.width_twips, 30);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("row-level table borders ignored to match Word print behavior")
        }));
    }
