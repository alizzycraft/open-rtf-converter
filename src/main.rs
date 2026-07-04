use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use open_rtf_converter::{ConvertOptions, convert_rtf_file_to_pdf};

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

    let options = ConvertOptions {
        diagnostics: cli.diagnostics,
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
