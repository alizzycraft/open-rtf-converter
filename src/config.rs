#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CompatibilityMode {
    StrictSpec,
    WordCompatiblePassive,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ActiveContentPolicy {
    Strip,
    Placeholder,
    Reject,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PdfLinkPolicy {
    DisableAll,
    RenderVisibleTextOnly,
    AllowSanitizedHttpLinks,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtfLimits {
    pub max_file_size: usize,
    pub max_group_depth: usize,
    pub max_control_word_len: usize,
    pub max_parameter_digits: usize,
    pub max_text_run_len: usize,
    pub max_binary_blob_size: usize,
    pub max_total_binary_bytes: usize,
    pub max_token_count: usize,
    pub max_destination_bytes: usize,
    pub max_unicode_fallback_skip: usize,
    pub max_table_cells: usize,
    pub max_styles: usize,
    pub max_form_field_entries: usize,
    pub max_fonts: usize,
    pub max_colors: usize,
    pub max_images: usize,
    pub max_shapes: usize,
    pub max_shape_points: usize,
    pub max_image_pixels: usize,
    pub max_image_display_twips: i32,
    pub min_image_scaling_percent: i32,
    pub max_image_scaling_percent: i32,
    pub max_image_crop_twips: i32,
    pub max_tab_stops: usize,
    pub max_tab_stop_twips: i32,
    pub max_table_row_height_twips: i32,
    pub max_table_row_offset_twips: i32,
    pub max_table_cell_gap_twips: i32,
    pub max_table_border_width_twips: i32,
    pub min_page_dimension_twips: i32,
    pub max_page_dimension_twips: i32,
    pub max_page_margin_twips: i32,
    pub max_page_gutter_twips: i32,
    pub max_header_footer_distance_twips: i32,
    pub max_section_columns: usize,
    pub max_column_gap_twips: i32,
    pub max_page_number_start: i32,
    pub max_page_border_spacing_twips: i32,
    pub max_shape_offset_twips: i32,
    pub max_shape_dimension_twips: i32,
    pub max_shape_stroke_width_twips: i32,
    pub max_paragraph_indent_twips: i32,
    pub max_paragraph_spacing_twips: i32,
    pub max_line_spacing_twips: i32,
    pub max_hyphenation_consecutive_lines: usize,
    pub max_hyphenation_zone_twips: i32,
    pub max_font_size_half_points: i32,
    pub max_character_spacing_twips: i32,
    pub min_character_scaling_percent: i32,
    pub max_character_scaling_percent: i32,
    pub max_output_text_chars: usize,
    pub max_pdf_output_bytes: usize,
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
            max_unicode_fallback_skip: 64,
            max_table_cells: 100_000,
            max_styles: 10_000,
            max_form_field_entries: 1_024,
            max_fonts: 2_000,
            max_colors: 10_000,
            max_images: 500,
            max_shapes: 2_000,
            max_shape_points: 4_096,
            max_image_pixels: 100_000_000,
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
            max_pdf_output_bytes: 100 * 1024 * 1024,
        }
    }
}

impl RtfLimits {
    pub fn browser_defaults() -> Self {
        Self {
            max_file_size: 10 * 1024 * 1024,
            max_group_depth: 96,
            max_binary_blob_size: 2 * 1024 * 1024,
            max_total_binary_bytes: 8 * 1024 * 1024,
            max_token_count: 2_000_000,
            max_destination_bytes: 4 * 1024 * 1024,
            max_unicode_fallback_skip: 16,
            max_table_cells: 50_000,
            max_form_field_entries: 256,
            max_images: 250,
            max_shapes: 1_000,
            max_shape_points: 1_024,
            max_image_pixels: 50_000_000,
            max_image_display_twips: 31_680,
            max_tab_stops: 96,
            max_hyphenation_consecutive_lines: 8,
            max_hyphenation_zone_twips: 1_440,
            max_output_text_chars: 5_000_000,
            max_pdf_output_bytes: 20 * 1024 * 1024,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtfParseOptions {
    pub compatibility_mode: CompatibilityMode,
    pub active_content_policy: ActiveContentPolicy,
    pub pdf_link_policy: PdfLinkPolicy,
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
    pub fn browser_safe_defaults() -> Self {
        Self {
            compatibility_mode: CompatibilityMode::WordCompatiblePassive,
            active_content_policy: ActiveContentPolicy::Placeholder,
            pdf_link_policy: PdfLinkPolicy::RenderVisibleTextOnly,
            limits: RtfLimits::browser_defaults(),
        }
    }
}
