use thiserror::Error;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
/// Selects how strictly the parser follows the RTF specification.
pub enum CompatibilityMode {
    /// Prefer specification-defined behavior over application quirks.
    StrictSpec,
    /// Reproduce safe Microsoft Word behavior where it differs from the specification.
    WordCompatiblePassive,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
/// Determines how active document features are handled.
pub enum ActiveContentPolicy {
    /// Remove active content without adding replacement text.
    Strip,
    /// Replace active content with a visible passive placeholder.
    Placeholder,
    /// Reject the document when active content is encountered.
    Reject,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
/// Determines how RTF hyperlinks are represented in the passive PDF.
pub enum PdfLinkPolicy {
    /// Disable links and emit only non-interactive content.
    DisableAll,
    /// Keep the visible label but do not create a PDF link action.
    RenderVisibleTextOnly,
    /// Allow only sanitized HTTP and HTTPS link actions.
    AllowSanitizedHttpLinks,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Resource and geometry limits applied while parsing, laying out, and rendering.
///
/// Byte and character limits use counts; dimensions and distances use RTF twips
/// (1/1440 inch) unless their names specify pixels, percentages, or half-points.
pub struct RtfLimits {
    /// Maximum input size in bytes.
    pub max_file_size: usize,
    /// Maximum nested RTF group depth.
    pub max_group_depth: usize,
    /// Maximum control-word length in bytes.
    pub max_control_word_len: usize,
    /// Maximum decimal digits in a control-word parameter.
    pub max_parameter_digits: usize,
    /// Maximum bytes accumulated in one lexer text run.
    pub max_text_run_len: usize,
    /// Maximum bytes in one binary payload.
    pub max_binary_blob_size: usize,
    /// Maximum aggregate binary-payload bytes.
    pub max_total_binary_bytes: usize,
    /// Maximum lexer token count.
    pub max_token_count: usize,
    /// Maximum bytes retained for one RTF destination.
    pub max_destination_bytes: usize,
    /// Maximum characters in one field instruction.
    pub max_field_instruction_chars: usize,
    /// Maximum characters skipped after an RTF Unicode escape.
    pub max_unicode_fallback_skip: usize,
    /// Maximum table-cell count.
    pub max_table_cells: usize,
    /// Maximum table-row count.
    pub max_table_rows: usize,
    /// Maximum nested table depth.
    pub max_table_nesting_level: usize,
    /// Maximum paragraphs retained across headers and footers.
    pub max_header_footer_paragraphs: usize,
    /// Maximum footnote and endnote count.
    pub max_notes: usize,
    /// Maximum bookmark count.
    pub max_bookmarks: usize,
    /// Maximum independently tracked field counters.
    pub max_field_counters: usize,
    /// Maximum document-property count.
    pub max_document_properties: usize,
    /// Maximum list-definition count.
    pub max_lists: usize,
    /// Maximum list-style reference count.
    pub max_style_references: usize,
    /// Maximum style-definition count.
    pub max_styles: usize,
    /// Maximum entries in one form field.
    pub max_form_field_entries: usize,
    /// Maximum font-table entry count.
    pub max_fonts: usize,
    /// Maximum color-table entry count.
    pub max_colors: usize,
    /// Maximum embedded-image count.
    pub max_images: usize,
    /// Maximum bytes in one decoded image payload.
    pub max_image_bytes: usize,
    /// Maximum shape count.
    pub max_shapes: usize,
    /// Maximum points in one vector shape.
    pub max_shape_points: usize,
    /// Maximum decoded pixels in one image.
    pub max_image_pixels: usize,
    /// Maximum declared image dimension in pixels.
    pub max_image_dimension_hint_px: u32,
    /// Maximum displayed image dimension in twips.
    pub max_image_display_twips: i32,
    /// Minimum image scaling percentage.
    pub min_image_scaling_percent: i32,
    /// Maximum image scaling percentage.
    pub max_image_scaling_percent: i32,
    /// Maximum absolute image crop in twips.
    pub max_image_crop_twips: i32,
    /// Maximum tab-stop count per paragraph.
    pub max_tab_stops: usize,
    /// Maximum tab-stop position in twips.
    pub max_tab_stop_twips: i32,
    /// Maximum absolute table-row height in twips.
    pub max_table_row_height_twips: i32,
    /// Maximum absolute table-row offset in twips.
    pub max_table_row_offset_twips: i32,
    /// Maximum table-cell gap in twips.
    pub max_table_cell_gap_twips: i32,
    /// Maximum table-border width in twips.
    pub max_table_border_width_twips: i32,
    /// Minimum page dimension in twips.
    pub min_page_dimension_twips: i32,
    /// Maximum page dimension in twips.
    pub max_page_dimension_twips: i32,
    /// Maximum page margin in twips.
    pub max_page_margin_twips: i32,
    /// Maximum page gutter in twips.
    pub max_page_gutter_twips: i32,
    /// Maximum header or footer distance in twips.
    pub max_header_footer_distance_twips: i32,
    /// Maximum section count.
    pub max_sections: usize,
    /// Maximum columns per section.
    pub max_section_columns: usize,
    /// Maximum section-column gap in twips.
    pub max_column_gap_twips: i32,
    /// Maximum page-number starting value.
    pub max_page_number_start: i32,
    /// Maximum page-border spacing in twips.
    pub max_page_border_spacing_twips: i32,
    /// Maximum absolute shape offset in twips.
    pub max_shape_offset_twips: i32,
    /// Maximum shape dimension in twips.
    pub max_shape_dimension_twips: i32,
    /// Maximum shape-stroke width in twips.
    pub max_shape_stroke_width_twips: i32,
    /// Maximum absolute paragraph indentation in twips.
    pub max_paragraph_indent_twips: i32,
    /// Maximum paragraph spacing in twips.
    pub max_paragraph_spacing_twips: i32,
    /// Maximum line spacing in twips.
    pub max_line_spacing_twips: i32,
    /// Maximum consecutive hyphenated lines.
    pub max_hyphenation_consecutive_lines: usize,
    /// Maximum hyphenation zone in twips.
    pub max_hyphenation_zone_twips: i32,
    /// Maximum font size in half-points.
    pub max_font_size_half_points: i32,
    /// Maximum absolute character spacing in twips.
    pub max_character_spacing_twips: i32,
    /// Minimum character scaling percentage.
    pub min_character_scaling_percent: i32,
    /// Maximum character scaling percentage.
    pub max_character_scaling_percent: i32,
    /// Maximum normalized output text characters.
    pub max_output_text_chars: usize,
    /// Maximum normalized document block count.
    pub max_document_blocks: usize,
    /// Maximum laid-out PDF item count.
    pub max_pdf_layout_items: usize,
    /// Maximum PDF indirect-object count.
    pub max_pdf_indirect_objects: usize,
    /// Maximum rendered PDF page count.
    pub max_pdf_pages: usize,
    /// Maximum rendered PDF size in bytes.
    pub max_pdf_output_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
/// Explains why a configured limit set is internally invalid.
pub enum LimitValidationError {
    /// A field that permits zero was configured with a negative value.
    #[error("{field} must be non-negative, got {value}")]
    Negative {
        /// Name of the invalid field.
        field: &'static str,
        /// Rejected value.
        value: i64,
    },
    /// A field that requires a positive value was configured with zero or less.
    #[error("{field} must be positive, got {value}")]
    NonPositive {
        /// Name of the invalid field.
        field: &'static str,
        /// Rejected value.
        value: i64,
    },
    /// A configured minimum exceeds its corresponding maximum.
    #[error("{min_field} ({min}) must not exceed {max_field} ({max})")]
    InvertedRange {
        /// Name of the minimum field.
        min_field: &'static str,
        /// Configured minimum.
        min: i64,
        /// Name of the maximum field.
        max_field: &'static str,
        /// Configured maximum.
        max: i64,
    },
}

impl Default for RtfLimits {
    fn default() -> Self {
        Self {
            max_file_size: 25 * 1024 * 1024,
            max_group_depth: 128,
            max_control_word_len: 64,
            max_parameter_digits: 12,
            max_text_run_len: 1024 * 1024,
            max_binary_blob_size: 5 * 1024 * 1024,
            max_total_binary_bytes: 20 * 1024 * 1024,
            max_token_count: 5_000_000,
            max_destination_bytes: 10 * 1024 * 1024,
            max_field_instruction_chars: 64 * 1024,
            max_unicode_fallback_skip: 64,
            max_table_cells: 100_000,
            max_table_rows: 50_000,
            max_table_nesting_level: 16,
            max_header_footer_paragraphs: 20_000,
            max_notes: 10_000,
            max_bookmarks: 10_000,
            max_field_counters: 10_000,
            max_document_properties: 10_000,
            max_lists: 10_000,
            max_style_references: 10_000,
            max_styles: 10_000,
            max_form_field_entries: 1_024,
            max_fonts: 2_000,
            max_colors: 10_000,
            max_images: 500,
            max_image_bytes: 5 * 1024 * 1024,
            max_shapes: 2_000,
            max_shape_points: 4_096,
            max_image_pixels: 100_000_000,
            max_image_dimension_hint_px: 100_000,
            max_image_display_twips: 63_360,
            min_image_scaling_percent: 1,
            max_image_scaling_percent: 400,
            max_image_crop_twips: 31_680,
            max_tab_stops: 128,
            max_tab_stop_twips: 31_680,
            max_table_row_height_twips: 31_680,
            max_table_row_offset_twips: 31_680,
            max_table_cell_gap_twips: 1_440,
            max_table_border_width_twips: 240,
            min_page_dimension_twips: 720,
            max_page_dimension_twips: 63_360,
            max_page_margin_twips: 31_680,
            max_page_gutter_twips: 7_200,
            max_header_footer_distance_twips: 31_680,
            max_sections: 10_000,
            max_section_columns: 12,
            max_column_gap_twips: 7_200,
            max_page_number_start: 32_767,
            max_page_border_spacing_twips: 2_880,
            max_shape_offset_twips: 31_680,
            max_shape_dimension_twips: 31_680,
            max_shape_stroke_width_twips: 240,
            max_paragraph_indent_twips: 31_680,
            max_paragraph_spacing_twips: 5_760,
            max_line_spacing_twips: 2_880,
            max_hyphenation_consecutive_lines: 16,
            max_hyphenation_zone_twips: 2_880,
            max_font_size_half_points: 400,
            max_character_spacing_twips: 1_000,
            min_character_scaling_percent: 25,
            max_character_scaling_percent: 400,
            max_output_text_chars: 10_000_000,
            max_document_blocks: 1_000_000,
            max_pdf_layout_items: 1_000_000,
            max_pdf_indirect_objects: 100_000,
            max_pdf_pages: 10_000,
            max_pdf_output_bytes: 100 * 1024 * 1024,
        }
    }
}

impl RtfLimits {
    /// Checks signed units and all minimum/maximum relationships.
    pub fn validate(&self) -> Result<(), LimitValidationError> {
        for (value, name) in [
            (self.max_image_crop_twips, "max_image_crop_twips"),
            (
                self.max_table_row_offset_twips,
                "max_table_row_offset_twips",
            ),
            (self.max_table_cell_gap_twips, "max_table_cell_gap_twips"),
            (
                self.max_table_border_width_twips,
                "max_table_border_width_twips",
            ),
            (self.max_page_margin_twips, "max_page_margin_twips"),
            (self.max_page_gutter_twips, "max_page_gutter_twips"),
            (
                self.max_header_footer_distance_twips,
                "max_header_footer_distance_twips",
            ),
            (self.max_column_gap_twips, "max_column_gap_twips"),
            (
                self.max_page_border_spacing_twips,
                "max_page_border_spacing_twips",
            ),
            (self.max_shape_offset_twips, "max_shape_offset_twips"),
            (
                self.max_shape_stroke_width_twips,
                "max_shape_stroke_width_twips",
            ),
            (
                self.max_paragraph_indent_twips,
                "max_paragraph_indent_twips",
            ),
            (
                self.max_paragraph_spacing_twips,
                "max_paragraph_spacing_twips",
            ),
            (self.max_line_spacing_twips, "max_line_spacing_twips"),
            (
                self.max_hyphenation_zone_twips,
                "max_hyphenation_zone_twips",
            ),
            (
                self.max_character_spacing_twips,
                "max_character_spacing_twips",
            ),
        ] {
            if value < 0 {
                return Err(LimitValidationError::Negative {
                    field: name,
                    value: i64::from(value),
                });
            }
        }

        for (value, name) in [
            (self.max_image_display_twips, "max_image_display_twips"),
            (self.min_image_scaling_percent, "min_image_scaling_percent"),
            (self.max_tab_stop_twips, "max_tab_stop_twips"),
            (
                self.max_table_row_height_twips,
                "max_table_row_height_twips",
            ),
            (self.min_page_dimension_twips, "min_page_dimension_twips"),
            (self.max_page_dimension_twips, "max_page_dimension_twips"),
            (self.max_shape_dimension_twips, "max_shape_dimension_twips"),
            (self.max_font_size_half_points, "max_font_size_half_points"),
            (
                self.min_character_scaling_percent,
                "min_character_scaling_percent",
            ),
        ] {
            if value <= 0 {
                return Err(LimitValidationError::NonPositive {
                    field: name,
                    value: i64::from(value),
                });
            }
        }

        if self.min_image_scaling_percent > self.max_image_scaling_percent {
            return Err(LimitValidationError::InvertedRange {
                min_field: "min_image_scaling_percent",
                min: i64::from(self.min_image_scaling_percent),
                max_field: "max_image_scaling_percent",
                max: i64::from(self.max_image_scaling_percent),
            });
        }
        if self.min_page_dimension_twips > self.max_page_dimension_twips {
            return Err(LimitValidationError::InvertedRange {
                min_field: "min_page_dimension_twips",
                min: i64::from(self.min_page_dimension_twips),
                max_field: "max_page_dimension_twips",
                max: i64::from(self.max_page_dimension_twips),
            });
        }
        if self.min_character_scaling_percent > self.max_character_scaling_percent {
            return Err(LimitValidationError::InvertedRange {
                min_field: "min_character_scaling_percent",
                min: i64::from(self.min_character_scaling_percent),
                max_field: "max_character_scaling_percent",
                max: i64::from(self.max_character_scaling_percent),
            });
        }
        Ok(())
    }

    /// Returns stricter limits suitable for browser and WASM workers.
    pub fn browser_defaults() -> Self {
        Self {
            max_file_size: 10 * 1024 * 1024,
            max_group_depth: 96,
            max_binary_blob_size: 2 * 1024 * 1024,
            max_total_binary_bytes: 8 * 1024 * 1024,
            max_token_count: 2_000_000,
            max_destination_bytes: 4 * 1024 * 1024,
            max_field_instruction_chars: 16 * 1024,
            max_unicode_fallback_skip: 16,
            max_table_cells: 50_000,
            max_table_rows: 20_000,
            max_table_nesting_level: 8,
            max_header_footer_paragraphs: 4_000,
            max_notes: 2_048,
            max_bookmarks: 2_048,
            max_field_counters: 2_048,
            max_document_properties: 2_048,
            max_lists: 2_048,
            max_style_references: 2_048,
            max_styles: 2_048,
            max_form_field_entries: 256,
            max_fonts: 256,
            max_colors: 2_048,
            max_images: 250,
            max_image_bytes: 2 * 1024 * 1024,
            max_shapes: 1_000,
            max_shape_points: 1_024,
            max_image_pixels: 50_000_000,
            max_image_dimension_hint_px: 50_000,
            max_image_display_twips: 31_680,
            max_tab_stops: 96,
            max_sections: 2_000,
            max_hyphenation_consecutive_lines: 8,
            max_hyphenation_zone_twips: 1_440,
            max_output_text_chars: 5_000_000,
            max_document_blocks: 200_000,
            max_pdf_layout_items: 200_000,
            max_pdf_indirect_objects: 25_000,
            max_pdf_pages: 2_000,
            max_pdf_output_bytes: 20 * 1024 * 1024,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Parser behavior and resource policy.
pub struct RtfParseOptions {
    /// Specification-versus-Word compatibility behavior.
    pub compatibility_mode: CompatibilityMode,
    /// Policy for active document features.
    pub active_content_policy: ActiveContentPolicy,
    /// Policy for PDF link creation.
    pub pdf_link_policy: PdfLinkPolicy,
    /// Resource and geometry limits.
    pub limits: RtfLimits,
}

impl Default for RtfParseOptions {
    fn default() -> Self {
        Self {
            compatibility_mode: CompatibilityMode::WordCompatiblePassive,
            active_content_policy: ActiveContentPolicy::Placeholder,
            pdf_link_policy: PdfLinkPolicy::RenderVisibleTextOnly,
            limits: RtfLimits::default(),
        }
    }
}

impl RtfParseOptions {
    /// Validates the configured resource limits.
    pub fn validate(&self) -> Result<(), LimitValidationError> {
        self.limits.validate()
    }

    /// Returns passive browser-oriented policy and stricter resource limits.
    pub fn browser_safe_defaults() -> Self {
        Self {
            compatibility_mode: CompatibilityMode::WordCompatiblePassive,
            active_content_policy: ActiveContentPolicy::Placeholder,
            pdf_link_policy: PdfLinkPolicy::RenderVisibleTextOnly,
            limits: RtfLimits::browser_defaults(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_limit_sets_validate() {
        assert_eq!(RtfLimits::default().validate(), Ok(()));
        assert_eq!(RtfLimits::browser_defaults().validate(), Ok(()));
        assert_eq!(RtfParseOptions::default().validate(), Ok(()));
        assert_eq!(RtfParseOptions::browser_safe_defaults().validate(), Ok(()));
    }

    #[test]
    fn validation_reports_negative_field_and_value() {
        let mut limits = RtfLimits::default();
        limits.max_table_row_offset_twips = -1;

        assert_eq!(
            limits.validate(),
            Err(LimitValidationError::Negative {
                field: "max_table_row_offset_twips",
                value: -1,
            })
        );
    }

    #[test]
    fn validation_reports_non_positive_field_and_value() {
        let mut limits = RtfLimits::default();
        limits.min_page_dimension_twips = 0;

        assert_eq!(
            limits.validate(),
            Err(LimitValidationError::NonPositive {
                field: "min_page_dimension_twips",
                value: 0,
            })
        );
    }

    #[test]
    fn validation_reports_inverted_range_fields_and_values() {
        let mut limits = RtfLimits::default();
        limits.min_page_dimension_twips = 2_000;
        limits.max_page_dimension_twips = 1_000;

        assert_eq!(
            limits.validate(),
            Err(LimitValidationError::InvertedRange {
                min_field: "min_page_dimension_twips",
                min: 2_000,
                max_field: "max_page_dimension_twips",
                max: 1_000,
            })
        );
    }

    #[test]
    fn parse_options_delegate_to_limit_validation() {
        let mut options = RtfParseOptions::default();
        options.limits.max_shape_dimension_twips = 0;

        assert_eq!(
            options.validate(),
            Err(LimitValidationError::NonPositive {
                field: "max_shape_dimension_twips",
                value: 0,
            })
        );
    }
}
