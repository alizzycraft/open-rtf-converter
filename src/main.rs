use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};

use clap::{Parser, ValueEnum};
use open_rtf_converter::{
    ActiveContentPolicy, CompatibilityMode, ConvertOptions, FontAsset, FontAssetStyle,
    FontProvider, FontProviderError, PdfLinkPolicy, RtfParseOptions, convert_rtf_file_to_pdf,
    rtf::{ParseError, parse_rtf_bytes_with_options},
};
use ttf_parser::{Face, name_id};

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

    /// Directory of passive .ttf/.otf font assets to load by metadata family names. Non-recursive.
    #[arg(long = "font-dir", value_name = "PATH")]
    font_dirs: Vec<PathBuf>,

    /// Do not inspect platform system font directories in normal CLI mode.
    ///
    /// System font discovery is never enabled for --browser-safe.
    #[arg(long)]
    no_system_fonts: bool,

    /// Substitute a missing RTF font family with a font family found in --font-dir, in the form REQUESTED[,ALIAS...]=INSTALLED.
    #[arg(
        long = "font-substitute",
        value_name = "REQUESTED[,ALIAS...]=INSTALLED"
    )]
    font_substitutes: Vec<String>,
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

    let font_dirs = effective_cli_font_dirs(&cli);

    let requested_font_families = match requested_font_families_for_font_dirs(&cli, &font_dirs) {
        Ok(families) => families,
        Err(error) => {
            eprintln!("error: {error}");
            let mut source = std::error::Error::source(&error);
            while let Some(next) = source {
                eprintln!("  caused by: {next}");
                source = next.source();
            }
            std::process::exit(match error {
                CliInputFontError::Parse(_) => 2,
                CliInputFontError::ReadInput { .. } => 1,
            });
        }
    };

    let font_provider = match load_cli_font_provider(
        &cli.fonts,
        &font_dirs,
        &cli.font_substitutes,
        &requested_font_families,
        cli.browser_safe,
    ) {
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
    options.parse_options = build_parse_options(cli);
    options
}

fn build_parse_options(cli: &Cli) -> RtfParseOptions {
    let mut options = if cli.browser_safe {
        RtfParseOptions::browser_safe_defaults()
    } else {
        RtfParseOptions::default()
    };
    if let Some(policy) = cli.active_content_policy {
        options.active_content_policy = policy.into();
    }
    if let Some(policy) = cli.pdf_link_policy {
        options.pdf_link_policy = policy.into();
    }
    if let Some(mode) = cli.compatibility_mode {
        options.compatibility_mode = mode.into();
    }
    options
}

fn effective_cli_font_dirs(cli: &Cli) -> Vec<PathBuf> {
    let mut font_dirs = cli.font_dirs.clone();
    if cli.browser_safe || cli.no_system_fonts {
        return font_dirs;
    }
    append_existing_unique_font_dirs(&mut font_dirs, default_system_font_dirs());
    font_dirs
}

fn append_existing_unique_font_dirs(
    font_dirs: &mut Vec<PathBuf>,
    candidates: impl IntoIterator<Item = PathBuf>,
) {
    for candidate in candidates {
        if !candidate.is_dir() {
            continue;
        }
        if font_dirs
            .iter()
            .any(|existing| same_cli_path(existing, &candidate))
        {
            continue;
        }
        font_dirs.push(candidate);
    }
}

fn default_system_font_dirs() -> Vec<PathBuf> {
    if cfg!(windows) {
        vec![PathBuf::from(r"C:\Windows\Fonts")]
    } else if cfg!(target_os = "macos") {
        vec![
            PathBuf::from("/System/Library/Fonts"),
            PathBuf::from("/Library/Fonts"),
        ]
    } else {
        vec![
            PathBuf::from("/usr/share/fonts"),
            PathBuf::from("/usr/local/share/fonts"),
            PathBuf::from("/usr/share/fonts/truetype"),
        ]
    }
}

fn same_cli_path(left: &Path, right: &Path) -> bool {
    if cfg!(windows) {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    } else {
        left == right
    }
}

const CLI_FONT_DIR_MAX_DEPTH: usize = 4;
const CLI_FONT_DIR_MAX_ENTRIES: usize = 4096;

fn requested_font_families_for_font_dirs(
    cli: &Cli,
    font_dirs: &[PathBuf],
) -> Result<Vec<String>, CliInputFontError> {
    if font_dirs.is_empty() {
        return Ok(Vec::new());
    }
    let input = std::fs::read(&cli.input).map_err(|source| CliInputFontError::ReadInput {
        path: cli.input.clone(),
        source,
    })?;
    let parsed = parse_rtf_bytes_with_options(&input, &build_parse_options(cli))
        .map_err(CliInputFontError::Parse)?;
    let mut families = Vec::new();
    for font in parsed.document.fonts {
        push_unique_font_family_name(&mut families, font.name);
        if let Some(alternate) = font.alternate_name {
            push_unique_font_family_name(&mut families, alternate);
        }
    }
    Ok(families)
}

fn load_cli_font_provider(
    specs: &[String],
    font_dirs: &[PathBuf],
    font_substitutes: &[String],
    requested_font_families: &[String],
    browser_safe: bool,
) -> Result<FontProvider, CliFontError> {
    let mut provider = if browser_safe {
        FontProvider::browser_safe_defaults()
    } else {
        FontProvider::default()
    };
    if specs.is_empty() && font_dirs.is_empty() && font_substitutes.is_empty() {
        return Ok(provider);
    }
    let substitutions = effective_font_substitutions(font_substitutes)?;

    let requested_asset_count =
        specs
            .len()
            .checked_add(font_dirs.len())
            .ok_or(CliFontError::Provider(FontProviderError::TooManyAssets {
                count: usize::MAX,
                limit: provider.limits.max_assets,
            }))?;
    if requested_asset_count > provider.limits.max_assets {
        return Err(CliFontError::Provider(FontProviderError::TooManyAssets {
            count: requested_asset_count,
            limit: provider.limits.max_assets,
        }));
    }
    let mut total_bytes = 0usize;
    for spec in specs {
        let (families, style, path) = parse_font_spec(spec)?;
        let bytes = read_bounded_font_file(path, provider.limits.max_asset_bytes)?;
        total_bytes = checked_add_font_asset_bytes(total_bytes, bytes.len(), &provider)?;
        provider.assets.push(FontAsset {
            family_names: families,
            style,
            bytes,
        });
    }
    for font_dir in font_dirs {
        total_bytes = load_cli_font_dir(
            font_dir,
            requested_font_families,
            &substitutions,
            &mut provider,
            total_bytes,
        )?;
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

fn parse_font_substitutions(specs: &[String]) -> Result<Vec<FontSubstitution>, CliFontError> {
    specs
        .iter()
        .map(|spec| parse_font_substitution_spec(spec))
        .collect()
}

fn effective_font_substitutions(specs: &[String]) -> Result<Vec<FontSubstitution>, CliFontError> {
    let mut substitutions = parse_font_substitutions(specs)?;
    for (requested_names, installed_family) in builtin_cli_font_substitutions() {
        substitutions.push(FontSubstitution {
            requested_names: requested_names
                .iter()
                .map(|requested| (*requested).to_string())
                .collect(),
            installed_family: (*installed_family).to_string(),
        });
    }
    Ok(substitutions)
}

fn builtin_cli_font_substitutions() -> &'static [(&'static [&'static str], &'static str)] {
    &[
        (&["Arial Narrow", "Helvetica Narrow"], "Arial"),
        (&["Arial Unicode"], "Arial"),
        (&["Book Antiqua"], "Times New Roman"),
        (&["Courier"], "Courier New"),
        (&["MS Serif"], "Times New Roman"),
        (&["MS Sans Serif"], "Microsoft Sans Serif"),
    ]
}

fn parse_font_substitution_spec(spec: &str) -> Result<FontSubstitution, CliFontError> {
    let Some((requested, installed)) = spec.split_once('=') else {
        return Err(CliFontError::MalformedFontSubstitutionSpec {
            spec: spec.to_string(),
        });
    };
    let requested_names = parse_font_family_aliases(requested);
    let installed_family = installed.trim();
    if requested_names.is_empty() || installed_family.is_empty() || installed_family == "*" {
        return Err(CliFontError::MalformedFontSubstitutionSpec {
            spec: spec.to_string(),
        });
    }
    Ok(FontSubstitution {
        requested_names,
        installed_family: installed_family.to_string(),
    })
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
    let parsed = parse_font_family_aliases(families);
    if parsed.is_empty() {
        return Err(CliFontError::MalformedFontSpec {
            spec: spec.to_string(),
        });
    }
    Ok((parsed, style))
}

fn parse_font_family_aliases(families: &str) -> Vec<String> {
    families
        .split(',')
        .map(str::trim)
        .filter(|family| !family.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>()
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

fn load_cli_font_dir(
    dir: &Path,
    requested_font_families: &[String],
    substitutions: &[FontSubstitution],
    provider: &mut FontProvider,
    total_bytes: usize,
) -> Result<usize, CliFontError> {
    load_cli_font_dir_with_limits(
        dir,
        requested_font_families,
        substitutions,
        provider,
        total_bytes,
        CLI_FONT_DIR_MAX_DEPTH,
        CLI_FONT_DIR_MAX_ENTRIES,
    )
}

fn load_cli_font_dir_with_limits(
    dir: &Path,
    requested_font_families: &[String],
    substitutions: &[FontSubstitution],
    provider: &mut FontProvider,
    mut total_bytes: usize,
    max_depth: usize,
    max_entries: usize,
) -> Result<usize, CliFontError> {
    let mut pending_dirs = vec![(dir.to_path_buf(), 0usize)];
    let mut entry_count = 0usize;
    while let Some((current_dir, depth)) = pending_dirs.pop() {
        let entries =
            std::fs::read_dir(&current_dir).map_err(|source| CliFontError::ReadFontDir {
                path: current_dir.clone(),
                source,
            })?;
        for entry in entries {
            let entry = entry.map_err(|source| CliFontError::ReadFontDir {
                path: current_dir.clone(),
                source,
            })?;
            entry_count =
                entry_count
                    .checked_add(1)
                    .ok_or_else(|| CliFontError::FontDirectoryTooLarge {
                        path: dir.to_path_buf(),
                        entries: usize::MAX,
                        limit: max_entries,
                    })?;
            if entry_count > max_entries {
                return Err(CliFontError::FontDirectoryTooLarge {
                    path: dir.to_path_buf(),
                    entries: entry_count,
                    limit: max_entries,
                });
            }

            let file_type = entry
                .file_type()
                .map_err(|source| CliFontError::ReadFontDir {
                    path: current_dir.clone(),
                    source,
                })?;
            let path = entry.path();
            if file_type.is_dir() {
                if depth < max_depth {
                    pending_dirs.push((path, depth + 1));
                }
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            total_bytes = load_cli_font_file_from_dir(
                &path,
                requested_font_families,
                substitutions,
                provider,
                total_bytes,
            )?;
        }
    }
    Ok(total_bytes)
}

fn load_cli_font_file_from_dir(
    path: &Path,
    requested_font_families: &[String],
    substitutions: &[FontSubstitution],
    provider: &mut FontProvider,
    mut total_bytes: usize,
) -> Result<usize, CliFontError> {
    if !is_supported_cli_font_path(path) {
        return Ok(total_bytes);
    }
    if cli_font_dir_file_exceeds_asset_limit(path, provider.limits.max_asset_bytes)? {
        return Ok(total_bytes);
    }
    let bytes = read_bounded_font_file(path, provider.limits.max_asset_bytes)?;
    let (mut family_names, style) = cli_font_metadata(&bytes, path)?;
    let mut substitute_aliases =
        substitute_aliases_for_font(&family_names, requested_font_families, substitutions);
    for alias in passive_symbolic_aliases_for_font(&family_names, requested_font_families) {
        push_unique_font_family_name(&mut substitute_aliases, alias);
    }
    let direct_match = requested_font_families.is_empty()
        || font_metadata_matches_requested_families(&family_names, requested_font_families);
    if !direct_match && substitute_aliases.is_empty() {
        return Ok(total_bytes);
    }
    for alias in substitute_aliases {
        push_unique_font_family_name(&mut family_names, alias);
    }
    if provider.assets.len() >= provider.limits.max_assets {
        return Err(CliFontError::Provider(FontProviderError::TooManyAssets {
            count: provider.assets.len() + 1,
            limit: provider.limits.max_assets,
        }));
    }
    total_bytes = checked_add_font_asset_bytes(total_bytes, bytes.len(), provider)?;
    provider.assets.push(FontAsset {
        family_names,
        style,
        bytes,
    });
    Ok(total_bytes)
}

fn substitute_aliases_for_font(
    family_names: &[String],
    requested_font_families: &[String],
    substitutions: &[FontSubstitution],
) -> Vec<String> {
    let mut aliases = Vec::new();
    for substitution in substitutions {
        if !family_names
            .iter()
            .any(|family| cli_font_family_names_match(family, &substitution.installed_family))
        {
            continue;
        }
        for requested_name in &substitution.requested_names {
            if !requested_font_families.is_empty()
                && !requested_font_families
                    .iter()
                    .any(|requested| cli_font_family_names_match(requested, requested_name))
            {
                continue;
            }
            push_unique_font_family_name(&mut aliases, requested_name.clone());
        }
    }
    aliases
}

fn passive_symbolic_aliases_for_font(
    family_names: &[String],
    requested_font_families: &[String],
) -> Vec<String> {
    if requested_font_families.is_empty()
        || !requested_font_families
            .iter()
            .any(|requested| cli_font_family_names_match(requested, "ZapfDingbats"))
    {
        return Vec::new();
    }

    if family_names
        .iter()
        .any(|family| cli_font_family_names_match(family, "Segoe UI Symbol"))
    {
        vec!["ZapfDingbats".to_string()]
    } else {
        Vec::new()
    }
}

fn cli_font_dir_file_exceeds_asset_limit(path: &Path, limit: usize) -> Result<bool, CliFontError> {
    let metadata = std::fs::metadata(path).map_err(|source| CliFontError::ReadFont {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(metadata.len() > limit as u64)
}

fn font_metadata_matches_requested_families(
    family_names: &[String],
    requested_font_families: &[String],
) -> bool {
    family_names.iter().any(|family| {
        requested_font_families
            .iter()
            .any(|requested| cli_font_family_names_match(family, requested))
    })
}

fn cli_font_family_names_match(left: &str, right: &str) -> bool {
    let left = canonical_cli_font_family_name(left);
    let right = canonical_cli_font_family_name(right);
    !left.is_empty() && !right.is_empty() && left == right
}

fn canonical_cli_font_family_name(value: &str) -> String {
    let normalized = value
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    for suffix in [" ce", " cyr", " greek", " tur", " baltic"] {
        if let Some(base) = normalized.strip_suffix(suffix)
            && !base.trim().is_empty()
        {
            return base.trim().to_string();
        }
    }
    normalized
}

fn is_supported_cli_font_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            let normalized = extension.to_ascii_lowercase();
            normalized == "ttf" || normalized == "otf"
        })
        .unwrap_or(false)
}

fn checked_add_font_asset_bytes(
    total_bytes: usize,
    asset_bytes: usize,
    provider: &FontProvider,
) -> Result<usize, CliFontError> {
    let total_bytes =
        total_bytes
            .checked_add(asset_bytes)
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
    Ok(total_bytes)
}

fn cli_font_metadata(
    bytes: &[u8],
    path: &Path,
) -> Result<(Vec<String>, FontAssetStyle), CliFontError> {
    let face = Face::parse(bytes, 0).map_err(|source| CliFontError::InvalidFontMetadata {
        path: path.to_path_buf(),
        reason: source.to_string(),
    })?;
    let mut family_names = Vec::new();
    for name_id in [
        name_id::TYPOGRAPHIC_FAMILY,
        name_id::FAMILY,
        name_id::FULL_NAME,
    ] {
        for name in face.names() {
            if name.name_id != name_id {
                continue;
            }
            let Some(value) = cli_font_name_to_string(&name) else {
                continue;
            };
            push_unique_font_family_name(&mut family_names, value);
        }
    }
    if family_names.is_empty() {
        return Err(CliFontError::MissingFontMetadata {
            path: path.to_path_buf(),
        });
    }
    Ok((
        family_names,
        FontAssetStyle {
            bold: face.is_bold(),
            italic: face.is_italic(),
        },
    ))
}

fn push_unique_font_family_name(family_names: &mut Vec<String>, value: String) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return;
    }
    if family_names
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(trimmed))
    {
        return;
    }
    family_names.push(trimmed.to_string());
}

fn cli_font_name_to_string(name: &ttf_parser::name::Name<'_>) -> Option<String> {
    if !name.is_unicode() || name.name.len() % 2 != 0 {
        return None;
    }
    let utf16 = name
        .name
        .chunks_exact(2)
        .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    String::from_utf16(&utf16).ok()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FontSubstitution {
    requested_names: Vec<String>,
    installed_family: String,
}

#[derive(Debug)]
enum CliFontError {
    MalformedFontSpec {
        spec: String,
    },
    MalformedFontSubstitutionSpec {
        spec: String,
    },
    ReadFont {
        path: PathBuf,
        source: std::io::Error,
    },
    ReadFontDir {
        path: PathBuf,
        source: std::io::Error,
    },
    InvalidFontMetadata {
        path: PathBuf,
        reason: String,
    },
    MissingFontMetadata {
        path: PathBuf,
    },
    FontTooLarge {
        path: PathBuf,
        size: usize,
        limit: usize,
    },
    FontDirectoryTooLarge {
        path: PathBuf,
        entries: usize,
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
            Self::MalformedFontSubstitutionSpec { spec } => write!(
                formatter,
                "font substitute must use REQUESTED[,ALIAS...]=INSTALLED syntax, got {spec:?}"
            ),
            Self::ReadFont { path, .. } => {
                write!(formatter, "failed to read font asset {}", path.display())
            }
            Self::ReadFontDir { path, .. } => {
                write!(
                    formatter,
                    "failed to read font directory {}",
                    path.display()
                )
            }
            Self::InvalidFontMetadata { path, reason } => write!(
                formatter,
                "failed to read font metadata from {}: {reason}",
                path.display()
            ),
            Self::MissingFontMetadata { path } => write!(
                formatter,
                "font asset {} does not contain a usable family name",
                path.display()
            ),
            Self::FontTooLarge { path, size, limit } => write!(
                formatter,
                "font asset {} exceeded configured limit: {size} bytes > {limit} bytes",
                path.display()
            ),
            Self::FontDirectoryTooLarge {
                path,
                entries,
                limit,
            } => write!(
                formatter,
                "font directory {} exceeded configured traversal limit: {entries} entries > {limit} entries",
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
            Self::ReadFontDir { source, .. } => Some(source),
            Self::Provider(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug)]
enum CliInputFontError {
    ReadInput {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse(ParseError),
}

impl fmt::Display for CliInputFontError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadInput { path, .. } => {
                write!(
                    formatter,
                    "failed to inspect input fonts from {}",
                    path.display()
                )
            }
            Self::Parse(error) => write!(formatter, "failed to inspect input fonts: {error}"),
        }
    }
}

impl std::error::Error for CliInputFontError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ReadInput { source, .. } => Some(source),
            Self::Parse(error) => Some(error),
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
    fn parses_font_substitution_specs() {
        let substitution =
            parse_font_substitution_spec("Arial Narrow,Helvetica Narrow=Arial").unwrap();

        assert_eq!(
            substitution,
            FontSubstitution {
                requested_names: vec!["Arial Narrow".to_string(), "Helvetica Narrow".to_string()],
                installed_family: "Arial".to_string(),
            }
        );

        for spec in ["Arial", "Arial=", "=Arial", "Arial=*"] {
            assert!(
                matches!(
                    parse_font_substitution_spec(spec),
                    Err(CliFontError::MalformedFontSubstitutionSpec { .. })
                ),
                "{spec}"
            );
        }
    }

    #[test]
    fn effective_font_substitutions_include_legacy_word_aliases() {
        let substitutions = effective_font_substitutions(&[]).unwrap();

        assert!(
            substitutions.iter().any(|substitution| {
                substitution.installed_family == "Courier New"
                    && substitution
                        .requested_names
                        .iter()
                        .any(|requested| requested == "Courier")
            }),
            "{substitutions:?}"
        );
        assert!(
            substitutions.iter().any(|substitution| {
                substitution.installed_family == "Microsoft Sans Serif"
                    && substitution
                        .requested_names
                        .iter()
                        .any(|requested| requested == "MS Sans Serif")
            }),
            "{substitutions:?}"
        );
        assert!(
            substitutions.iter().any(|substitution| {
                substitution.installed_family == "Times New Roman"
                    && substitution
                        .requested_names
                        .iter()
                        .any(|requested| requested == "MS Serif")
            }),
            "{substitutions:?}"
        );
    }

    #[test]
    fn builtin_legacy_word_aliases_apply_to_matching_metadata_fonts() {
        let substitutions = effective_font_substitutions(&[]).unwrap();

        assert_eq!(
            substitute_aliases_for_font(
                &["Courier New".to_string()],
                &["Courier".to_string()],
                &substitutions,
            ),
            vec!["Courier".to_string()]
        );
        assert_eq!(
            substitute_aliases_for_font(
                &["Microsoft Sans Serif".to_string()],
                &["MS Sans Serif".to_string()],
                &substitutions,
            ),
            vec!["MS Sans Serif".to_string()]
        );
        assert_eq!(
            substitute_aliases_for_font(
                &["Times New Roman".to_string()],
                &["MS Serif".to_string()],
                &substitutions,
            ),
            vec!["MS Serif".to_string()]
        );
    }

    #[test]
    fn loads_valid_cli_font_provider_without_system_fonts() {
        let specs = vec![
            "Tuffy,Tuffy Alias=fixtures/fonts/Tuffy.ttf".to_string(),
            "Tuffy,Tuffy Alias:bold=fixtures/fonts/Tuffy.ttf".to_string(),
            "Tuffy,Tuffy Alias:italic=fixtures/fonts/Tuffy.ttf".to_string(),
        ];
        let provider = load_cli_font_provider(&specs, &[], &[], &[], false).unwrap();

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
    fn loads_cli_font_directory_from_bounded_metadata_names() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::copy("fixtures/fonts/Tuffy.ttf", dir.path().join("Tuffy.ttf")).unwrap();
        std::fs::write(dir.path().join("ignore.txt"), b"not a font").unwrap();

        let provider = load_cli_font_provider(
            &[],
            &[dir.path().to_path_buf()],
            &[],
            &["Tuffy".to_string()],
            false,
        )
        .unwrap();

        assert_eq!(provider.assets.len(), 1);
        assert!(
            provider.assets[0]
                .family_names
                .iter()
                .any(|name| name == "Tuffy"),
            "metadata family names were {:?}",
            provider.assets[0].family_names
        );
        assert_eq!(
            provider.coverage_for_char("Tuffy", 'A'),
            open_rtf_converter::FontCoverage::Covered
        );
    }

    #[test]
    fn loads_cli_font_directory_from_bounded_subdirectories() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("truetype").join("fixture");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::copy("fixtures/fonts/Tuffy.ttf", nested.join("Tuffy.ttf")).unwrap();

        let provider = load_cli_font_provider(
            &[],
            &[dir.path().to_path_buf()],
            &[],
            &["Tuffy".to_string()],
            false,
        )
        .unwrap();

        assert_eq!(provider.assets.len(), 1);
        assert_eq!(
            provider.coverage_for_char("Tuffy", 'A'),
            open_rtf_converter::FontCoverage::Covered
        );
    }

    #[test]
    fn cli_font_directory_traversal_obeys_entry_limit() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("one.txt"), b"not a font").unwrap();
        std::fs::write(dir.path().join("two.txt"), b"not a font").unwrap();
        let mut provider = FontProvider::default();

        let error = load_cli_font_dir_with_limits(dir.path(), &[], &[], &mut provider, 0, 1, 1)
            .unwrap_err();

        assert!(matches!(
            error,
            CliFontError::FontDirectoryTooLarge {
                entries: 2,
                limit: 1,
                ..
            }
        ));
        assert!(provider.assets.is_empty());
    }

    #[test]
    fn cli_font_directory_loads_only_requested_metadata_families() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::copy("fixtures/fonts/Tuffy.ttf", dir.path().join("Tuffy.ttf")).unwrap();

        let provider = load_cli_font_provider(
            &[],
            &[dir.path().to_path_buf()],
            &[],
            &["Missing Word Font".to_string()],
            false,
        )
        .unwrap();

        assert!(
            provider.assets.is_empty(),
            "unrequested directory font should not be loaded"
        );
        assert_eq!(
            provider.coverage_for_char("Tuffy", 'A'),
            open_rtf_converter::FontCoverage::NoAsset
        );
    }

    #[test]
    fn cli_font_directory_skips_oversized_unrequested_assets() {
        let dir = tempfile::tempdir().unwrap();
        let oversized = dir.path().join("HugeUnused.ttf");
        let file = std::fs::File::create(&oversized).unwrap();
        file.set_len((FontProvider::default().limits.max_asset_bytes + 1) as u64)
            .unwrap();
        std::fs::copy("fixtures/fonts/Tuffy.ttf", dir.path().join("Tuffy.ttf")).unwrap();

        let provider = load_cli_font_provider(
            &[],
            &[dir.path().to_path_buf()],
            &[],
            &["Tuffy".to_string()],
            false,
        )
        .unwrap();

        assert_eq!(provider.assets.len(), 1);
        assert_eq!(
            provider.coverage_for_char("Tuffy", 'A'),
            open_rtf_converter::FontCoverage::Covered
        );
    }

    #[test]
    fn cli_font_directory_matches_word_charset_suffixed_requests() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::copy("fixtures/fonts/Tuffy.ttf", dir.path().join("Tuffy.ttf")).unwrap();

        let provider = load_cli_font_provider(
            &[],
            &[dir.path().to_path_buf()],
            &[],
            &["Tuffy Cyr".to_string()],
            false,
        )
        .unwrap();

        assert_eq!(provider.assets.len(), 1);
        assert_eq!(
            provider.coverage_for_char("Tuffy Cyr", 'A'),
            open_rtf_converter::FontCoverage::Covered
        );
    }

    #[test]
    fn cli_font_directory_substitutes_requested_missing_family_with_installed_family() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::copy("fixtures/fonts/Tuffy.ttf", dir.path().join("Tuffy.ttf")).unwrap();
        let substitutions = vec!["Missing Word Font,Missing Alias=Tuffy".to_string()];

        let provider = load_cli_font_provider(
            &[],
            &[dir.path().to_path_buf()],
            &substitutions,
            &["Missing Word Font".to_string()],
            false,
        )
        .unwrap();

        assert_eq!(provider.assets.len(), 1);
        assert_eq!(
            provider.coverage_for_char("Missing Word Font", 'A'),
            open_rtf_converter::FontCoverage::Covered
        );
        assert_eq!(
            provider.coverage_for_char("Missing Alias", 'A'),
            open_rtf_converter::FontCoverage::NoAsset,
            "aliases not requested by the document should not be added"
        );
    }

    #[test]
    fn passive_symbolic_alias_maps_requested_zapf_to_segoe_ui_symbol() {
        let aliases = passive_symbolic_aliases_for_font(
            &["Segoe UI Symbol".to_string()],
            &["ZapfDingbats".to_string()],
        );

        assert_eq!(aliases, vec!["ZapfDingbats".to_string()]);
    }

    #[test]
    fn passive_symbolic_alias_does_not_map_legacy_private_symbol_fonts() {
        for family in ["Wingdings", "Webdings", "Symbol"] {
            let aliases = passive_symbolic_aliases_for_font(
                &[family.to_string()],
                &["ZapfDingbats".to_string()],
            );

            assert!(
                aliases.is_empty(),
                "{family} should not be treated as a Unicode ZapfDingbats substitute"
            );
        }
    }

    #[test]
    fn passive_symbolic_alias_only_applies_when_zapf_is_requested() {
        let aliases = passive_symbolic_aliases_for_font(
            &["Segoe UI Symbol".to_string()],
            &["Arial".to_string()],
        );

        assert!(aliases.is_empty());
    }

    #[test]
    fn explicit_cli_font_aliases_and_directory_fonts_can_be_combined() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::copy("fixtures/fonts/Tuffy.ttf", dir.path().join("Tuffy.ttf")).unwrap();
        let specs = vec!["Word Alias=fixtures/fonts/Tuffy.ttf".to_string()];

        let provider = load_cli_font_provider(
            &specs,
            &[dir.path().to_path_buf()],
            &[],
            &["Tuffy".to_string()],
            false,
        )
        .unwrap();

        assert_eq!(provider.assets.len(), 2);
        assert_eq!(
            provider.coverage_for_char("Word Alias", 'A'),
            open_rtf_converter::FontCoverage::Covered
        );
        assert_eq!(
            provider.coverage_for_char("Tuffy", 'A'),
            open_rtf_converter::FontCoverage::Covered
        );
    }

    #[test]
    fn appends_existing_unique_font_dirs_without_host_assumptions() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let missing = second.path().join("missing");
        let mut font_dirs = vec![first.path().to_path_buf()];

        append_existing_unique_font_dirs(
            &mut font_dirs,
            [
                first.path().to_path_buf(),
                second.path().to_path_buf(),
                missing,
            ],
        );

        assert_eq!(font_dirs.len(), 2);
        assert_eq!(font_dirs[0], first.path());
        assert_eq!(font_dirs[1], second.path());
    }

    #[test]
    fn cli_system_font_discovery_stays_out_of_browser_safe_mode() {
        let normal = Cli::try_parse_from(["open-rtf-converter", "input.rtf"]).unwrap();
        assert!(!normal.no_system_fonts);

        let no_system =
            Cli::try_parse_from(["open-rtf-converter", "--no-system-fonts", "input.rtf"]).unwrap();
        assert!(effective_cli_font_dirs(&no_system).is_empty());

        let browser_safe =
            Cli::try_parse_from(["open-rtf-converter", "--browser-safe", "input.rtf"]).unwrap();
        assert!(effective_cli_font_dirs(&browser_safe).is_empty());
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
        let error = load_cli_font_provider(&specs, &[], &[], &[], false).unwrap_err();

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
        let error = load_cli_font_provider(&specs, &[], &[], &[], false).unwrap_err();

        assert!(matches!(
            error,
            CliFontError::Provider(FontProviderError::InvalidAsset { .. })
        ));
    }

    #[test]
    fn browser_safe_cli_uses_stricter_font_provider_limits() {
        let provider = load_cli_font_provider(&[], &[], &[], &[], true).unwrap();

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
        let error = load_cli_font_provider(&specs, &[], &[], &[], true).unwrap_err();

        assert!(matches!(
            error,
            CliFontError::Provider(FontProviderError::TooManyFamilyNames { .. })
        ));
        assert!(
            load_cli_font_provider(&specs, &[], &[], &[], false).is_ok(),
            "normal CLI mode should keep the wider default font alias limit"
        );
    }

    #[test]
    fn browser_safe_cli_rejects_too_many_font_assets_before_reading() {
        let specs = (0..=FontProvider::browser_safe_defaults().limits.max_assets)
            .map(|idx| format!("Tuffy{idx}=missing-font-{idx}.ttf"))
            .collect::<Vec<_>>();
        let error = load_cli_font_provider(&specs, &[], &[], &[], true).unwrap_err();

        assert!(matches!(
            error,
            CliFontError::Provider(FontProviderError::TooManyAssets { .. })
        ));
    }

    #[test]
    fn browser_safe_cli_rejects_too_many_font_directories_before_reading() {
        let font_dirs = (0..=FontProvider::browser_safe_defaults().limits.max_assets)
            .map(|idx| PathBuf::from(format!("missing-font-dir-{idx}")))
            .collect::<Vec<_>>();
        let error = load_cli_font_provider(&[], &font_dirs, &[], &[], true).unwrap_err();

        assert!(matches!(
            error,
            CliFontError::Provider(FontProviderError::TooManyAssets { .. })
        ));
    }
}
