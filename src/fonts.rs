use std::error::Error;
use std::fmt;

use ttf_parser::Face;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontProvider {
    pub assets: Vec<FontAsset>,
    pub limits: FontProviderLimits,
}

impl Default for FontProvider {
    fn default() -> Self {
        Self {
            assets: Vec::new(),
            limits: FontProviderLimits::default(),
        }
    }
}

impl FontProvider {
    pub fn browser_safe_defaults() -> Self {
        Self {
            assets: Vec::new(),
            limits: FontProviderLimits::browser_defaults(),
        }
    }

    pub fn validate(&self) -> Result<(), FontProviderError> {
        if self.assets.len() > self.limits.max_assets {
            return Err(FontProviderError::TooManyAssets {
                count: self.assets.len(),
                limit: self.limits.max_assets,
            });
        }

        let mut total_bytes = 0usize;
        for (asset_index, asset) in self.assets.iter().enumerate() {
            if asset.bytes.is_empty() {
                return Err(FontProviderError::EmptyAsset { asset_index });
            }
            if asset.bytes.len() > self.limits.max_asset_bytes {
                return Err(FontProviderError::AssetTooLarge {
                    asset_index,
                    size: asset.bytes.len(),
                    limit: self.limits.max_asset_bytes,
                });
            }
            total_bytes = total_bytes.checked_add(asset.bytes.len()).ok_or(
                FontProviderError::TotalBytesTooLarge {
                    size: usize::MAX,
                    limit: self.limits.max_total_bytes,
                },
            )?;
            if total_bytes > self.limits.max_total_bytes {
                return Err(FontProviderError::TotalBytesTooLarge {
                    size: total_bytes,
                    limit: self.limits.max_total_bytes,
                });
            }
            if asset.family_names.is_empty() {
                return Err(FontProviderError::MissingFamilyName { asset_index });
            }
            if asset.family_names.len() > self.limits.max_family_names_per_asset {
                return Err(FontProviderError::TooManyFamilyNames {
                    asset_index,
                    count: asset.family_names.len(),
                    limit: self.limits.max_family_names_per_asset,
                });
            }
            for family in &asset.family_names {
                let trimmed = family.trim();
                if trimmed.is_empty() {
                    return Err(FontProviderError::MissingFamilyName { asset_index });
                }
                if trimmed.len() > self.limits.max_family_name_len {
                    return Err(FontProviderError::FamilyNameTooLong {
                        asset_index,
                        len: trimmed.len(),
                        limit: self.limits.max_family_name_len,
                    });
                }
            }
            Face::parse(&asset.bytes, 0).map_err(|error| FontProviderError::InvalidAsset {
                asset_index,
                reason: error.to_string(),
            })?;
        }

        Ok(())
    }

    pub fn has_asset_for_family(&self, family_name: &str) -> bool {
        let family_name = normalized_family_name(family_name);
        if family_name.is_empty() {
            return false;
        }
        self.assets
            .iter()
            .any(|asset| asset.matches_family(&family_name))
    }

    pub fn coverage_for_char(&self, family_name: &str, ch: char) -> FontCoverage {
        let family_name = normalized_family_name(family_name);
        if family_name.is_empty() {
            return FontCoverage::NoAsset;
        }
        let mut found_asset = false;
        for asset in &self.assets {
            if !asset.matches_family(&family_name) {
                continue;
            }
            found_asset = true;
            if let Ok(face) = Face::parse(&asset.bytes, 0)
                && face.glyph_index(ch).is_some()
            {
                return FontCoverage::Covered;
            }
        }
        if found_asset {
            FontCoverage::MissingGlyph
        } else {
            FontCoverage::NoAsset
        }
    }

    pub fn glyph_metrics_for_char(&self, family_name: &str, ch: char) -> Option<FontGlyphMetrics> {
        self.glyph_metrics_for_char_with_style(family_name, FontAssetStyle::default(), ch)
    }

    pub fn glyph_metrics_for_char_with_style(
        &self,
        family_name: &str,
        style: FontAssetStyle,
        ch: char,
    ) -> Option<FontGlyphMetrics> {
        let family_name = normalized_family_name(family_name);
        if family_name.is_empty() {
            return None;
        }
        let asset = self.best_metric_asset_for_family_style_char(&family_name, style, ch)?;
        glyph_metrics_for_asset(asset, ch)
    }

    fn best_metric_asset_for_family_style_char(
        &self,
        family_name: &str,
        style: FontAssetStyle,
        ch: char,
    ) -> Option<&FontAsset> {
        self.assets
            .iter()
            .filter(|asset| asset.matches_family(family_name))
            .filter(|asset| glyph_metrics_for_asset(asset, ch).is_some())
            .min_by_key(|asset| supplied_font_style_mismatch_score(asset.style, style))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontAsset {
    pub family_names: Vec<String>,
    pub style: FontAssetStyle,
    pub bytes: Vec<u8>,
}

impl FontAsset {
    pub(crate) fn matches_family(&self, family_name: &str) -> bool {
        let family_name = normalized_family_name(family_name);
        !family_name.is_empty()
            && self
                .family_names
                .iter()
                .any(|candidate| font_family_names_match(candidate, &family_name))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct FontAssetStyle {
    pub bold: bool,
    pub italic: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FontCoverage {
    NoAsset,
    Covered,
    MissingGlyph,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct FontGlyphMetrics {
    pub units_per_em: u16,
    pub advance_units: u16,
    pub ascender_units: i16,
    pub descender_units: i16,
}

impl FontGlyphMetrics {
    pub fn advance_points(self, font_size_points: f32) -> f32 {
        if self.units_per_em == 0 {
            return 0.0;
        }
        font_size_points * f32::from(self.advance_units) / f32::from(self.units_per_em)
    }

    pub fn line_height_points(self, font_size_points: f32) -> Option<f32> {
        if self.units_per_em == 0 {
            return None;
        }
        let height_units = i32::from(self.ascender_units) - i32::from(self.descender_units);
        if height_units <= 0 {
            return None;
        }
        let height = font_size_points * height_units as f32 / f32::from(self.units_per_em);
        height.is_finite().then_some(height)
    }
}

fn glyph_metrics_for_asset(asset: &FontAsset, ch: char) -> Option<FontGlyphMetrics> {
    let face = Face::parse(&asset.bytes, 0).ok()?;
    let glyph_id = face.glyph_index(ch)?;
    let advance_units = face.glyph_hor_advance(glyph_id)?;
    Some(FontGlyphMetrics {
        units_per_em: face.units_per_em(),
        advance_units,
        ascender_units: face.ascender(),
        descender_units: face.descender(),
    })
}

fn supplied_font_style_mismatch_score(
    asset_style: FontAssetStyle,
    requested: FontAssetStyle,
) -> u8 {
    u8::from(asset_style.bold != requested.bold) + u8::from(asset_style.italic != requested.italic)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontProviderLimits {
    pub max_assets: usize,
    pub max_asset_bytes: usize,
    pub max_total_bytes: usize,
    pub max_family_names_per_asset: usize,
    pub max_family_name_len: usize,
}

impl Default for FontProviderLimits {
    fn default() -> Self {
        Self {
            max_assets: 64,
            max_asset_bytes: 10 * 1024 * 1024,
            max_total_bytes: 40 * 1024 * 1024,
            max_family_names_per_asset: 64,
            max_family_name_len: 128,
        }
    }
}

impl FontProviderLimits {
    pub fn browser_defaults() -> Self {
        Self {
            max_assets: 16,
            max_asset_bytes: 2 * 1024 * 1024,
            max_total_bytes: 8 * 1024 * 1024,
            max_family_names_per_asset: 24,
            max_family_name_len: 128,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontProviderError {
    TooManyAssets {
        count: usize,
        limit: usize,
    },
    EmptyAsset {
        asset_index: usize,
    },
    AssetTooLarge {
        asset_index: usize,
        size: usize,
        limit: usize,
    },
    TotalBytesTooLarge {
        size: usize,
        limit: usize,
    },
    TooManyFamilyNames {
        asset_index: usize,
        count: usize,
        limit: usize,
    },
    MissingFamilyName {
        asset_index: usize,
    },
    FamilyNameTooLong {
        asset_index: usize,
        len: usize,
        limit: usize,
    },
    InvalidAsset {
        asset_index: usize,
        reason: String,
    },
}

impl fmt::Display for FontProviderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyAssets { count, limit } => {
                write!(formatter, "too many passive font assets: {count} > {limit}")
            }
            Self::EmptyAsset { asset_index } => {
                write!(formatter, "passive font asset {asset_index} is empty")
            }
            Self::AssetTooLarge {
                asset_index,
                size,
                limit,
            } => write!(
                formatter,
                "passive font asset {asset_index} exceeded limit: {size} bytes > {limit} bytes"
            ),
            Self::TotalBytesTooLarge { size, limit } => write!(
                formatter,
                "passive font assets exceeded total limit: {size} bytes > {limit} bytes"
            ),
            Self::TooManyFamilyNames {
                asset_index,
                count,
                limit,
            } => write!(
                formatter,
                "passive font asset {asset_index} has too many family names: {count} > {limit}"
            ),
            Self::MissingFamilyName { asset_index } => {
                write!(
                    formatter,
                    "passive font asset {asset_index} has no family name"
                )
            }
            Self::FamilyNameTooLong {
                asset_index,
                len,
                limit,
            } => write!(
                formatter,
                "passive font asset {asset_index} family name exceeded limit: {len} bytes > {limit} bytes"
            ),
            Self::InvalidAsset {
                asset_index,
                reason,
            } => write!(
                formatter,
                "passive font asset {asset_index} is not a valid OpenType/TrueType font: {reason}"
            ),
        }
    }
}

impl Error for FontProviderError {}

fn normalized_family_name(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn canonical_word_charset_family_name(value: &str) -> String {
    let normalized = normalized_family_name(value);
    for suffix in [" ce", " cyr", " greek", " tur", " baltic"] {
        if let Some(base) = normalized.strip_suffix(suffix)
            && !base.trim().is_empty()
        {
            return base.trim().to_string();
        }
    }
    normalized
}

fn font_family_names_match(left: &str, right: &str) -> bool {
    let left = normalized_family_name(left);
    let right = normalized_family_name(right);
    !left.is_empty()
        && !right.is_empty()
        && (is_wildcard_font_family_alias(&left)
            || is_wildcard_font_family_alias(&right)
            || left == right
            || canonical_word_charset_family_name(&left)
                == canonical_word_charset_family_name(&right))
}

fn is_wildcard_font_family_alias(value: &str) -> bool {
    value == "*"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tuffy_provider() -> FontProvider {
        FontProvider {
            assets: vec![FontAsset {
                family_names: vec!["Times New Roman".to_string()],
                style: FontAssetStyle::default(),
                bytes: include_bytes!("../fixtures/fonts/Tuffy.ttf").to_vec(),
            }],
            limits: FontProviderLimits {
                max_asset_bytes: 256 * 1024,
                max_total_bytes: 256 * 1024,
                ..FontProviderLimits::default()
            },
        }
    }

    #[test]
    fn word_charset_suffixes_match_caller_base_font_family() {
        let provider = tuffy_provider();
        provider.validate().unwrap();

        for family in [
            "Times New Roman CE",
            "Times New Roman Cyr",
            "Times New Roman Greek",
            "Times New Roman Tur",
            "Times New Roman Baltic",
        ] {
            assert!(provider.has_asset_for_family(family), "{family}");
            assert_eq!(
                provider.coverage_for_char(family, 'A'),
                FontCoverage::Covered
            );
            assert!(
                provider.glyph_metrics_for_char(family, 'A').is_some(),
                "{family}"
            );
        }
    }

    #[test]
    fn word_charset_suffix_aliasing_does_not_match_unrelated_names() {
        let provider = tuffy_provider();
        provider.validate().unwrap();

        assert!(!provider.has_asset_for_family("Times New"));
        assert_eq!(
            provider.coverage_for_char("Times New", 'A'),
            FontCoverage::NoAsset
        );
        assert!(!provider.has_asset_for_family("Greek"));
    }

    #[test]
    fn wildcard_family_alias_matches_any_requested_font() {
        let provider = FontProvider {
            assets: vec![FontAsset {
                family_names: vec!["*".to_string()],
                style: FontAssetStyle::default(),
                bytes: include_bytes!("../fixtures/fonts/Tuffy.ttf").to_vec(),
            }],
            limits: FontProviderLimits {
                max_asset_bytes: 256 * 1024,
                max_total_bytes: 256 * 1024,
                ..FontProviderLimits::default()
            },
        };
        provider.validate().unwrap();

        for family in ["Book Antiqua", "Arial Narrow", "Unknown Word Font"] {
            assert!(provider.has_asset_for_family(family), "{family}");
            assert_eq!(
                provider.coverage_for_char(family, 'A'),
                FontCoverage::Covered
            );
            assert!(
                provider.glyph_metrics_for_char(family, 'A').is_some(),
                "{family}"
            );
        }
    }

    #[test]
    fn style_aware_metrics_prefer_exact_matching_asset() {
        let regular = FontAsset {
            family_names: vec!["Tuffy".to_string()],
            style: FontAssetStyle::default(),
            bytes: include_bytes!("../fixtures/fonts/Tuffy.ttf").to_vec(),
        };
        let bold = FontAsset {
            family_names: vec!["Tuffy".to_string()],
            style: FontAssetStyle {
                bold: true,
                italic: false,
            },
            bytes: include_bytes!("../fixtures/fonts/Tuffy.ttf").to_vec(),
        };
        let bold_italic = FontAsset {
            family_names: vec!["Tuffy".to_string()],
            style: FontAssetStyle {
                bold: true,
                italic: true,
            },
            bytes: include_bytes!("../fixtures/fonts/Tuffy.ttf").to_vec(),
        };
        let provider = FontProvider {
            assets: vec![regular, bold, bold_italic],
            limits: FontProviderLimits::default(),
        };

        let selected = provider
            .best_metric_asset_for_family_style_char(
                "Tuffy",
                FontAssetStyle {
                    bold: true,
                    italic: true,
                },
                'A',
            )
            .expect("style-matched asset");

        assert_eq!(
            selected.style,
            FontAssetStyle {
                bold: true,
                italic: true,
            }
        );
        assert!(
            provider
                .glyph_metrics_for_char_with_style(
                    "Tuffy",
                    FontAssetStyle {
                        bold: true,
                        italic: true,
                    },
                    'A'
                )
                .is_some()
        );
    }

    #[test]
    fn style_aware_metrics_prefer_closest_fallback_asset() {
        let regular = FontAsset {
            family_names: vec!["Tuffy".to_string()],
            style: FontAssetStyle::default(),
            bytes: include_bytes!("../fixtures/fonts/Tuffy.ttf").to_vec(),
        };
        let bold = FontAsset {
            family_names: vec!["Tuffy".to_string()],
            style: FontAssetStyle {
                bold: true,
                italic: false,
            },
            bytes: include_bytes!("../fixtures/fonts/Tuffy.ttf").to_vec(),
        };
        let provider = FontProvider {
            assets: vec![regular, bold],
            limits: FontProviderLimits::default(),
        };

        let selected = provider
            .best_metric_asset_for_family_style_char(
                "Tuffy",
                FontAssetStyle {
                    bold: true,
                    italic: true,
                },
                'A',
            )
            .expect("closest style asset");

        assert_eq!(
            selected.style,
            FontAssetStyle {
                bold: true,
                italic: false,
            }
        );
    }

    #[test]
    fn validates_family_name_count_per_asset() {
        let mut provider = tuffy_provider();
        provider.assets[0].family_names =
            vec!["One".to_string(), "Two".to_string(), "Three".to_string()];
        provider.limits.max_family_names_per_asset = 2;

        assert!(matches!(
            provider.validate(),
            Err(FontProviderError::TooManyFamilyNames {
                asset_index: 0,
                count: 3,
                limit: 2,
            })
        ));
    }
}
