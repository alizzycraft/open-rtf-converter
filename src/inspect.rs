//! Focused, bounded projections of RTF metadata.

use thiserror::Error;

use crate::{RtfParseOptions, rtf::parse_rtf_bytes_with_options};

/// Metadata requested by an RTF document that callers may need before conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtfInspection {
    /// Primary and alternate font-family names in deterministic, first-seen order.
    pub requested_font_families: Vec<String>,
}

/// An error produced while inspecting RTF bytes.
///
/// Inspection uses the same bounded parser as conversion, so the wrapped
/// [`crate::ParseError`] preserves typed configuration, lexical, malformed-input,
/// active-content, and resource-limit failures.
#[derive(Debug, Error)]
pub enum InspectError {
    /// The bounded RTF parser rejected the input or its configuration.
    #[error(transparent)]
    Parse(#[from] crate::ParseError),
}

/// Inspects RTF bytes using the supplied parser policy and resource limits.
///
/// The returned font families include primary and alternate names, are
/// deduplicated case-insensitively, and preserve the first-seen display spelling
/// and parser order.
pub fn inspect_rtf(input: &[u8], options: &RtfParseOptions) -> Result<RtfInspection, InspectError> {
    let parsed = parse_rtf_bytes_with_options(input, options)?;
    let mut requested_font_families = Vec::new();

    for font in parsed.document.fonts {
        push_unique_font_family_name(&mut requested_font_families, &font.name);
        if let Some(alternate_name) = font.alternate_name.as_deref() {
            push_unique_font_family_name(&mut requested_font_families, alternate_name);
        }
    }

    Ok(RtfInspection {
        requested_font_families,
    })
}

fn push_unique_font_family_name(family_names: &mut Vec<String>, value: &str) {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || family_names
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(trimmed))
    {
        return;
    }
    family_names.push(trimmed.to_string());
}

#[cfg(test)]
mod tests {
    use crate::{LimitValidationError, ParseError, RtfParseOptions, rtf::LexError};

    use super::{InspectError, RtfInspection, inspect_rtf};

    #[test]
    fn returns_primary_and_alternate_families_in_first_seen_order() {
        let input = br"{\rtf1{\fonttbl{\f0 Alpha{\*\falt Alpha Alternate;};}{\f1 Beta;}}Body\par}";

        let inspection = inspect_rtf(input, &RtfParseOptions::default()).unwrap();

        assert_eq!(
            inspection,
            RtfInspection {
                requested_font_families: vec![
                    "Alpha".to_string(),
                    "Alpha Alternate".to_string(),
                    "Beta".to_string(),
                ],
            }
        );
    }

    #[test]
    fn deduplicates_names_case_insensitively_and_preserves_first_spelling() {
        let input = br"{\rtf1{\fonttbl{\f0 First Face{\*\falt first face;};}{\f1 FIRST FACE;}{\f2 Other;}}Body\par}";

        let inspection = inspect_rtf(input, &RtfParseOptions::default()).unwrap();

        assert_eq!(inspection.requested_font_families, ["First Face", "Other"]);
    }

    #[test]
    fn preserves_decoded_unicode_font_names() {
        let input =
            br"{\rtf1{\fonttbl{\f0 Tuff\u121?;}{\f1 Mystery{\*\falt Tuff\u121?;};}}Body\par}";

        let inspection = inspect_rtf(input, &RtfParseOptions::default()).unwrap();

        assert_eq!(inspection.requested_font_families, ["Tuffy", "Mystery"]);
    }

    #[test]
    fn preserves_typed_limit_validation_errors_before_lexing() {
        let mut options = RtfParseOptions::default();
        options.limits.min_page_dimension_twips = 0;

        let error = inspect_rtf(br"{not valid RTF", &options).unwrap_err();

        assert!(matches!(
            error,
            InspectError::Parse(ParseError::InvalidLimits(
                LimitValidationError::NonPositive {
                    field: "min_page_dimension_twips",
                    value: 0,
                }
            ))
        ));
    }

    #[test]
    fn preserves_malformed_input_errors() {
        let error = inspect_rtf(br"{\rtf1", &RtfParseOptions::default()).unwrap_err();

        assert!(matches!(
            error,
            InspectError::Parse(ParseError::Lex(LexError::UnclosedGroup(_)))
        ));
    }

    #[test]
    fn applies_font_resource_limits() {
        let input = br"{\rtf1{\fonttbl{\f0 Alpha;}{\f1 Beta;}}Body\par}";
        let mut options = RtfParseOptions::default();
        options.limits.max_fonts = 1;

        let error = inspect_rtf(input, &options).unwrap_err();

        assert!(matches!(
            error,
            InspectError::Parse(ParseError::ResourceLimitExceeded { resource, .. })
                if resource == "fonts"
        ));
    }
}
