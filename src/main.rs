use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};

use clap::{Parser, ValueEnum};
use open_rtf_converter::{
    ActiveContentPolicy, CompatibilityMode, ConvertOptions, FontAsset, FontAssetStyle,
    FontProvider, FontProviderError, PdfLinkPolicy, convert_rtf_file_to_pdf,
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

    /// Use stricter browser/WASM-oriented conversion limits.
    #[arg(long)]
    browser_safe: bool,

    /// Active content handling policy.
    #[arg(long, value_enum)]
    active_content_policy: Option<CliActiveContentPolicy>,

    /// PDF link handling policy.
    #[arg(long, value_enum)]
    pdf_link_policy: Option<CliPdfLinkPolicy>,

    /// RTF compatibility mode.
    #[arg(long, value_enum)]
    compatibility_mode: Option<CliCompatibilityMode>,

    /// Caller-provided passive font asset in the form FAMILY[,ALIAS...][:STYLE]=PATH. Repeat for multiple fonts.
    ///
    /// Use * as a family alias to apply a vetted fallback font to otherwise unmatched Word font names.
    /// STYLE may be regular, bold, italic, or bold-italic. If omitted, regular is used.
    #[arg(long = "font", value_name = "FAMILY[,ALIAS...][:STYLE]=PATH")]
    fonts: Vec<String>,
}

#[derive(Debug, Copy, Clone, ValueEnum)]
enum OutputFormat {
    Pdf,
}

#[derive(Debug, Copy, Clone, ValueEnum)]
enum CliActiveContentPolicy {
    Strip,
    Placeholder,
    Reject,
}

impl From<CliActiveContentPolicy> for ActiveContentPolicy {
    fn from(value: CliActiveContentPolicy) -> Self {
        match value {
            CliActiveContentPolicy::Strip => Self::Strip,
            CliActiveContentPolicy::Placeholder => Self::Placeholder,
            CliActiveContentPolicy::Reject => Self::Reject,
        }
    }
}

#[derive(Debug, Copy, Clone, ValueEnum)]
enum CliPdfLinkPolicy {
    DisableAll,
    RenderVisibleTextOnly,
    AllowSanitizedHttpLinks,
}

impl From<CliPdfLinkPolicy> for PdfLinkPolicy {
    fn from(value: CliPdfLinkPolicy) -> Self {
        match value {
            CliPdfLinkPolicy::DisableAll => Self::DisableAll,
            CliPdfLinkPolicy::RenderVisibleTextOnly => Self::RenderVisibleTextOnly,
            CliPdfLinkPolicy::AllowSanitizedHttpLinks => Self::AllowSanitizedHttpLinks,
        }
    }
}

#[derive(Debug, Copy, Clone, ValueEnum)]
enum CliCompatibilityMode {
    StrictSpec,
    WordCompatiblePassive,
}

impl From<CliCompatibilityMode> for CompatibilityMode {
    fn from(value: CliCompatibilityMode) -> Self {
        match value {
            CliCompatibilityMode::StrictSpec => Self::StrictSpec,
            CliCompatibilityMode::WordCompatiblePassive => Self::WordCompatiblePassive,
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let output = match cli.output_path.as_ref().or(cli.output.as_ref()).cloned() {
        Some(path) => path,
        None => cli.input.with_extension(match cli.format {
            OutputFormat::Pdf => "pdf",
        }),
    };

    let font_provider = match load_cli_font_provider(&cli.fonts, cli.browser_safe) {
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

    let options = build_convert_options(&cli, font_provider);

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

fn build_convert_options(cli: &Cli, font_provider: FontProvider) -> ConvertOptions {
    let mut options = if cli.browser_safe {
        ConvertOptions::browser_safe_defaults()
    } else {
        ConvertOptions::default()
    };
    options.diagnostics = cli.diagnostics;
    options.font_provider = font_provider;
    if let Some(policy) = cli.active_content_policy {
        options.parse_options.active_content_policy = policy.into();
    }
    if let Some(policy) = cli.pdf_link_policy {
        options.parse_options.pdf_link_policy = policy.into();
    }
    if let Some(mode) = cli.compatibility_mode {
        options.parse_options.compatibility_mode = mode.into();
    }
    options
}

fn load_cli_font_provider(
    specs: &[String],
    browser_safe: bool,
) -> Result<FontProvider, CliFontError> {
    let mut provider = if browser_safe {
        FontProvider::browser_safe_defaults()
    } else {
        FontProvider::default()
    };
    if specs.is_empty() {
        return Ok(provider);
    }

    if specs.len() > provider.limits.max_assets {
        return Err(CliFontError::Provider(FontProviderError::TooManyAssets {
            count: specs.len(),
            limit: provider.limits.max_assets,
        }));
    }
    let mut total_bytes = 0usize;
    for spec in specs {
        let (families, style, path) = parse_font_spec(spec)?;
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
            style,
            bytes,
        });
    }
    provider.validate().map_err(CliFontError::Provider)?;
    Ok(provider)
}

fn parse_font_spec(spec: &str) -> Result<(Vec<String>, FontAssetStyle, &Path), CliFontError> {
    let Some((family, path)) = spec.split_once('=') else {
        return Err(CliFontError::MalformedFontSpec {
            spec: spec.to_string(),
        });
    };
    let (family, style) = parse_font_family_aliases_and_style(family, spec)?;
    let path = path.trim();
    if path.is_empty() {
        return Err(CliFontError::MalformedFontSpec {
            spec: spec.to_string(),
        });
    }
    Ok((family, style, Path::new(path)))
}

fn parse_font_family_aliases_and_style(
    families: &str,
    spec: &str,
) -> Result<(Vec<String>, FontAssetStyle), CliFontError> {
    let (families, style) = if let Some((families, style)) = families.rsplit_once(':') {
        (families, parse_font_asset_style(style, spec)?)
    } else {
        (families, FontAssetStyle::default())
    };
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
    Ok((parsed, style))
}

fn parse_font_asset_style(style: &str, spec: &str) -> Result<FontAssetStyle, CliFontError> {
    let normalized = style.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "regular" | "normal" => Ok(FontAssetStyle {
            bold: false,
            italic: false,
        }),
        "bold" => Ok(FontAssetStyle {
            bold: true,
            italic: false,
        }),
        "italic" | "oblique" => Ok(FontAssetStyle {
            bold: false,
            italic: true,
        }),
        "bold-italic" | "bolditalic" | "bold_italic" | "bold-oblique" | "boldoblique" => {
            Ok(FontAssetStyle {
                bold: true,
                italic: true,
            })
        }
        _ => Err(CliFontError::MalformedFontSpec {
            spec: spec.to_string(),
        }),
    }
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
                "font asset must use FAMILY[,ALIAS...][:STYLE]=PATH syntax, got {spec:?}"
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
        let (families, style, path) =
            parse_font_spec("Times New Roman=fixtures/fonts/Tuffy.ttf").unwrap();

        assert_eq!(families, vec!["Times New Roman"]);
        assert_eq!(style, FontAssetStyle::default());
        assert_eq!(path, Path::new("fixtures/fonts/Tuffy.ttf"));
    }

    #[test]
    fn parses_font_spec_with_alias_family_names() {
        let (families, style, path) =
            parse_font_spec("Times New Roman,Arial,Book Antiqua=fixtures/fonts/Tuffy.ttf").unwrap();

        assert_eq!(families, vec!["Times New Roman", "Arial", "Book Antiqua"]);
        assert_eq!(style, FontAssetStyle::default());
        assert_eq!(path, Path::new("fixtures/fonts/Tuffy.ttf"));
    }

    #[test]
    fn parses_font_spec_with_wildcard_fallback_alias() {
        let (families, style, path) = parse_font_spec("*=fixtures/fonts/Tuffy.ttf").unwrap();

        assert_eq!(families, vec!["*"]);
        assert_eq!(style, FontAssetStyle::default());
        assert_eq!(path, Path::new("fixtures/fonts/Tuffy.ttf"));
    }

    #[test]
    fn parses_font_spec_with_style_suffix() {
        let (families, style, path) =
            parse_font_spec("Times New Roman,Book Antiqua:bold-italic=fixtures/fonts/Tuffy.ttf")
                .unwrap();

        assert_eq!(families, vec!["Times New Roman", "Book Antiqua"]);
        assert_eq!(
            style,
            FontAssetStyle {
                bold: true,
                italic: true
            }
        );
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
        assert!(matches!(
            parse_font_spec("Tuffy:heavy=fixtures/fonts/Tuffy.ttf"),
            Err(CliFontError::MalformedFontSpec { .. })
        ));
    }

    #[test]
    fn loads_valid_cli_font_provider_without_system_fonts() {
        let specs = vec![
            "Tuffy,Tuffy Alias=fixtures/fonts/Tuffy.ttf".to_string(),
            "Tuffy,Tuffy Alias:bold=fixtures/fonts/Tuffy.ttf".to_string(),
            "Tuffy,Tuffy Alias:italic=fixtures/fonts/Tuffy.ttf".to_string(),
        ];
        let provider = load_cli_font_provider(&specs, false).unwrap();

        assert_eq!(provider.assets.len(), 3);
        assert_eq!(
            provider.assets[0].family_names,
            vec!["Tuffy", "Tuffy Alias"]
        );
        assert_eq!(provider.assets[0].style, FontAssetStyle::default());
        assert_eq!(
            provider.assets[1].style,
            FontAssetStyle {
                bold: true,
                italic: false
            }
        );
        assert_eq!(
            provider.assets[2].style,
            FontAssetStyle {
                bold: false,
                italic: true
            }
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
    fn cli_security_policy_switches_override_conversion_defaults() {
        let cli = Cli::try_parse_from([
            "open-rtf-converter",
            "--browser-safe",
            "--active-content-policy",
            "reject",
            "--pdf-link-policy",
            "disable-all",
            "--compatibility-mode",
            "strict-spec",
            "--diagnostics",
            "input.rtf",
            "output.pdf",
        ])
        .unwrap();
        let options = build_convert_options(&cli, FontProvider::default());

        assert!(options.diagnostics);
        assert_eq!(
            options.parse_options.active_content_policy,
            ActiveContentPolicy::Reject
        );
        assert_eq!(
            options.parse_options.pdf_link_policy,
            PdfLinkPolicy::DisableAll
        );
        assert_eq!(
            options.parse_options.compatibility_mode,
            CompatibilityMode::StrictSpec
        );
        assert_eq!(
            options.parse_options.limits.max_pdf_output_bytes,
            20 * 1024 * 1024
        );
    }

    #[test]
    fn cli_policy_switches_accept_passive_renderer_defaults_explicitly() {
        let cli = Cli::try_parse_from([
            "open-rtf-converter",
            "--active-content-policy",
            "placeholder",
            "--pdf-link-policy",
            "render-visible-text-only",
            "--compatibility-mode",
            "word-compatible-passive",
            "input.rtf",
        ])
        .unwrap();
        let options = build_convert_options(&cli, FontProvider::default());

        assert_eq!(
            options.parse_options.active_content_policy,
            ActiveContentPolicy::Placeholder
        );
        assert_eq!(
            options.parse_options.pdf_link_policy,
            PdfLinkPolicy::RenderVisibleTextOnly
        );
        assert_eq!(
            options.parse_options.compatibility_mode,
            CompatibilityMode::WordCompatiblePassive
        );
        assert_eq!(
            options.parse_options.limits.max_pdf_output_bytes,
            100 * 1024 * 1024
        );
    }

    #[test]
    fn rejects_unknown_cli_security_policy_value() {
        let error = Cli::try_parse_from([
            "open-rtf-converter",
            "--active-content-policy",
            "execute",
            "input.rtf",
        ])
        .unwrap_err();

        assert_eq!(error.kind(), clap::error::ErrorKind::InvalidValue);
    }

    #[test]
    fn rejects_cli_font_aliases_over_provider_limit() {
        let aliases = (0..=FontProvider::default().limits.max_family_names_per_asset)
            .map(|idx| format!("Alias {idx}"))
            .collect::<Vec<_>>()
            .join(",");
        let specs = vec![format!("{aliases}=fixtures/fonts/Tuffy.ttf")];
        let error = load_cli_font_provider(&specs, false).unwrap_err();

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
        let error = load_cli_font_provider(&specs, false).unwrap_err();

        assert!(matches!(
            error,
            CliFontError::Provider(FontProviderError::InvalidAsset { .. })
        ));
    }

    #[test]
    fn browser_safe_cli_uses_stricter_font_provider_limits() {
        let provider = load_cli_font_provider(&[], true).unwrap();

        assert_eq!(
            provider.limits.max_assets,
            FontProvider::browser_safe_defaults().limits.max_assets
        );
        assert_eq!(
            provider.limits.max_total_bytes,
            FontProvider::browser_safe_defaults().limits.max_total_bytes
        );

        let aliases = (0..=FontProvider::browser_safe_defaults()
            .limits
            .max_family_names_per_asset)
            .map(|idx| format!("Alias {idx}"))
            .collect::<Vec<_>>()
            .join(",");
        let specs = vec![format!("{aliases}=fixtures/fonts/Tuffy.ttf")];
        let error = load_cli_font_provider(&specs, true).unwrap_err();

        assert!(matches!(
            error,
            CliFontError::Provider(FontProviderError::TooManyFamilyNames { .. })
        ));
        assert!(
            load_cli_font_provider(&specs, false).is_ok(),
            "normal CLI mode should keep the wider default font alias limit"
        );
    }

    #[test]
    fn browser_safe_cli_rejects_too_many_font_assets_before_reading() {
        let specs = (0..=FontProvider::browser_safe_defaults().limits.max_assets)
            .map(|idx| format!("Tuffy{idx}=missing-font-{idx}.ttf"))
            .collect::<Vec<_>>();
        let error = load_cli_font_provider(&specs, true).unwrap_err();

        assert!(matches!(
            error,
            CliFontError::Provider(FontProviderError::TooManyAssets { .. })
        ));
    }
}
