use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};

use clap::{Parser, ValueEnum};
use open_rtf_converter::{
    ConvertOptions, FontAsset, FontAssetStyle, FontProvider, FontProviderError,
    convert_rtf_file_to_pdf,
};

#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about = "Convert RTF documents to PDF without external document converters."
)]
struct Cli {
    /// Input RTF file.
    input: PathBuf,

    /// Output file. If omitted, the input stem is reused with the selected format extension.
    output: Option<PathBuf>,

    /// Explicit output path.
    #[arg(short, long)]
    output_path: Option<PathBuf>,

    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Pdf)]
    format: OutputFormat,

    /// Print parser/layout diagnostics to stderr.
    #[arg(long)]
    diagnostics: bool,

    /// Caller-provided passive font asset in the form FAMILY[,ALIAS...]=PATH. Repeat for multiple fonts.
    #[arg(long = "font", value_name = "FAMILY[,ALIAS...]=PATH")]
    fonts: Vec<String>,
}

#[derive(Debug, Copy, Clone, ValueEnum)]
enum OutputFormat {
    Pdf,
}

fn main() {
    let cli = Cli::parse();
    let output = match cli.output_path.or(cli.output) {
        Some(path) => path,
        None => cli.input.with_extension(match cli.format {
            OutputFormat::Pdf => "pdf",
        }),
    };

    let font_provider = match load_cli_font_provider(&cli.fonts) {
        Ok(provider) => provider,
        Err(error) => {
            eprintln!("error: {error}");
            let mut source = std::error::Error::source(&error);
            while let Some(next) = source {
                eprintln!("  caused by: {next}");
                source = next.source();
            }
            std::process::exit(1);
        }
    };

    let options = ConvertOptions {
        diagnostics: cli.diagnostics,
        font_provider,
        ..ConvertOptions::default()
    };

    match convert_rtf_file_to_pdf(&cli.input, &output, &options) {
        Ok(report) => {
            for diagnostic in report.diagnostics {
                eprintln!("{diagnostic}");
            }
            if cli.diagnostics {
                eprintln!("converted {} page(s) to {}", report.pages, output.display());
            }
        }
        Err(error) => {
            eprintln!("error: {error}");
            let mut source = std::error::Error::source(&error);
            while let Some(next) = source {
                eprintln!("  caused by: {next}");
                source = next.source();
            }
            std::process::exit(match error.to_string().contains("parse") {
                true => 2,
                false => 1,
            });
        }
    }
}

fn load_cli_font_provider(specs: &[String]) -> Result<FontProvider, CliFontError> {
    if specs.is_empty() {
        return Ok(FontProvider::default());
    }

    let mut provider = FontProvider::default();
    if specs.len() > provider.limits.max_assets {
        return Err(CliFontError::Provider(FontProviderError::TooManyAssets {
            count: specs.len(),
            limit: provider.limits.max_assets,
        }));
    }
    let mut total_bytes = 0usize;
    for spec in specs {
        let (families, path) = parse_font_spec(spec)?;
        let bytes = read_bounded_font_file(path, provider.limits.max_asset_bytes)?;
        total_bytes =
            total_bytes
                .checked_add(bytes.len())
                .ok_or(CliFontError::TotalFontBytesTooLarge {
                    size: usize::MAX,
                    limit: provider.limits.max_total_bytes,
                })?;
        if total_bytes > provider.limits.max_total_bytes {
            return Err(CliFontError::TotalFontBytesTooLarge {
                size: total_bytes,
                limit: provider.limits.max_total_bytes,
            });
        }
        provider.assets.push(FontAsset {
            family_names: families,
            style: FontAssetStyle::default(),
            bytes,
        });
    }
    provider.validate().map_err(CliFontError::Provider)?;
    Ok(provider)
}

fn parse_font_spec(spec: &str) -> Result<(Vec<String>, &Path), CliFontError> {
    let Some((family, path)) = spec.split_once('=') else {
        return Err(CliFontError::MalformedFontSpec {
            spec: spec.to_string(),
        });
    };
    let families = parse_font_family_aliases(family, spec)?;
    let path = path.trim();
    if path.is_empty() {
        return Err(CliFontError::MalformedFontSpec {
            spec: spec.to_string(),
        });
    }
    Ok((families, Path::new(path)))
}

fn parse_font_family_aliases(families: &str, spec: &str) -> Result<Vec<String>, CliFontError> {
    let parsed = families
        .split(',')
        .map(str::trim)
        .filter(|family| !family.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if parsed.is_empty() {
        return Err(CliFontError::MalformedFontSpec {
            spec: spec.to_string(),
        });
    }
    Ok(parsed)
}

fn read_bounded_font_file(path: &Path, limit: usize) -> Result<Vec<u8>, CliFontError> {
    let mut file = std::fs::File::open(path).map_err(|source| CliFontError::ReadFont {
        path: path.to_path_buf(),
        source,
    })?;
    if let Ok(metadata) = file.metadata() {
        let size = metadata.len();
        if size > limit as u64 {
            return Err(CliFontError::FontTooLarge {
                path: path.to_path_buf(),
                size: size as usize,
                limit,
            });
        }
    }

    let mut bytes = Vec::new();
    let mut bounded = file.by_ref().take(limit as u64 + 1);
    bounded
        .read_to_end(&mut bytes)
        .map_err(|source| CliFontError::ReadFont {
            path: path.to_path_buf(),
            source,
        })?;
    if bytes.len() > limit {
        return Err(CliFontError::FontTooLarge {
            path: path.to_path_buf(),
            size: bytes.len(),
            limit,
        });
    }
    Ok(bytes)
}

#[derive(Debug)]
enum CliFontError {
    MalformedFontSpec {
        spec: String,
    },
    ReadFont {
        path: PathBuf,
        source: std::io::Error,
    },
    FontTooLarge {
        path: PathBuf,
        size: usize,
        limit: usize,
    },
    TotalFontBytesTooLarge {
        size: usize,
        limit: usize,
    },
    Provider(FontProviderError),
}

impl fmt::Display for CliFontError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedFontSpec { spec } => write!(
                formatter,
                "font asset must use FAMILY[,ALIAS...]=PATH syntax, got {spec:?}"
            ),
            Self::ReadFont { path, .. } => {
                write!(formatter, "failed to read font asset {}", path.display())
            }
            Self::FontTooLarge { path, size, limit } => write!(
                formatter,
                "font asset {} exceeded configured limit: {size} bytes > {limit} bytes",
                path.display()
            ),
            Self::TotalFontBytesTooLarge { size, limit } => write!(
                formatter,
                "font assets exceeded configured total limit: {size} bytes > {limit} bytes"
            ),
            Self::Provider(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for CliFontError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadFont { source, .. } => Some(source),
            Self::Provider(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_font_spec_with_spaced_family_name() {
        let (families, path) = parse_font_spec("Times New Roman=fixtures/fonts/Tuffy.ttf").unwrap();

        assert_eq!(families, vec!["Times New Roman"]);
        assert_eq!(path, Path::new("fixtures/fonts/Tuffy.ttf"));
    }

    #[test]
    fn parses_font_spec_with_alias_family_names() {
        let (families, path) =
            parse_font_spec("Times New Roman,Arial,Book Antiqua=fixtures/fonts/Tuffy.ttf").unwrap();

        assert_eq!(families, vec!["Times New Roman", "Arial", "Book Antiqua"]);
        assert_eq!(path, Path::new("fixtures/fonts/Tuffy.ttf"));
    }

    #[test]
    fn rejects_malformed_font_specs() {
        assert!(matches!(
            parse_font_spec("fixtures/fonts/Tuffy.ttf"),
            Err(CliFontError::MalformedFontSpec { .. })
        ));
        assert!(matches!(
            parse_font_spec("Tuffy="),
            Err(CliFontError::MalformedFontSpec { .. })
        ));
        assert!(matches!(
            parse_font_spec("=fixtures/fonts/Tuffy.ttf"),
            Err(CliFontError::MalformedFontSpec { .. })
        ));
        assert!(matches!(
            parse_font_spec(" , =fixtures/fonts/Tuffy.ttf"),
            Err(CliFontError::MalformedFontSpec { .. })
        ));
    }

    #[test]
    fn loads_valid_cli_font_provider_without_system_fonts() {
        let specs = vec!["Tuffy,Tuffy Alias=fixtures/fonts/Tuffy.ttf".to_string()];
        let provider = load_cli_font_provider(&specs).unwrap();

        assert_eq!(provider.assets.len(), 1);
        assert_eq!(
            provider.assets[0].family_names,
            vec!["Tuffy", "Tuffy Alias"]
        );
        assert_eq!(
            provider.coverage_for_char("Tuffy", 'A'),
            open_rtf_converter::FontCoverage::Covered
        );
        assert_eq!(
            provider.coverage_for_char("Tuffy Alias", 'A'),
            open_rtf_converter::FontCoverage::Covered
        );
    }

    #[test]
    fn rejects_cli_font_aliases_over_provider_limit() {
        let aliases = (0..=FontProvider::default().limits.max_family_names_per_asset)
            .map(|idx| format!("Alias {idx}"))
            .collect::<Vec<_>>()
            .join(",");
        let specs = vec![format!("{aliases}=fixtures/fonts/Tuffy.ttf")];
        let error = load_cli_font_provider(&specs).unwrap_err();

        assert!(matches!(
            error,
            CliFontError::Provider(FontProviderError::TooManyFamilyNames { .. })
        ));
    }

    #[test]
    fn rejects_invalid_cli_font_bytes_before_conversion() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hostile-font.ttf");
        std::fs::write(&path, b"not a real font").unwrap();
        let specs = vec![format!("Hostile={}", path.display())];
        let error = load_cli_font_provider(&specs).unwrap_err();

        assert!(matches!(
            error,
            CliFontError::Provider(FontProviderError::InvalidAsset { .. })
        ));
    }
}
