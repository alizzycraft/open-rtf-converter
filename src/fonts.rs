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
        self.assets.iter().any(|asset| {
            asset
                .family_names
                .iter()
                .any(|candidate| normalized_family_name(candidate) == family_name)
        })
    }

    pub fn coverage_for_char(&self, family_name: &str, ch: char) -> FontCoverage {
        let family_name = normalized_family_name(family_name);
        if family_name.is_empty() {
            return FontCoverage::NoAsset;
        }
        let mut found_asset = false;
        for asset in &self.assets {
            if !asset
                .family_names
                .iter()
                .any(|candidate| normalized_family_name(candidate) == family_name)
            {
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
        let family_name = normalized_family_name(family_name);
        if family_name.is_empty() {
            return None;
        }
        for asset in &self.assets {
            if !asset
                .family_names
                .iter()
                .any(|candidate| normalized_family_name(candidate) == family_name)
            {
                continue;
            }
            let Ok(face) = Face::parse(&asset.bytes, 0) else {
                continue;
            };
            let Some(glyph_id) = face.glyph_index(ch) else {
                continue;
            };
            let Some(advance_units) = face.glyph_hor_advance(glyph_id) else {
                continue;
            };
            return Some(FontGlyphMetrics {
                units_per_em: face.units_per_em(),
                advance_units,
                ascender_units: face.ascender(),
                descender_units: face.descender(),
            });
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontAsset {
    pub family_names: Vec<String>,
    pub style: FontAssetStyle,
    pub bytes: Vec<u8>,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontProviderLimits {
    pub max_assets: usize,
    pub max_asset_bytes: usize,
    pub max_total_bytes: usize,
    pub max_family_name_len: usize,
}

impl Default for FontProviderLimits {
    fn default() -> Self {
        Self {
            max_assets: 64,
            max_asset_bytes: 10 * 1024 * 1024,
            max_total_bytes: 40 * 1024 * 1024,
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
