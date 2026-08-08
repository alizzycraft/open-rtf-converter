use std::env;
use std::error::Error;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use lopdf::Document;
use open_rtf_converter::{ConvertOptions, convert_rtf_to_pdf};
use sha2::{Digest, Sha256};

const DEFAULT_ITERATIONS: usize = 16;
const DEFAULT_WARMUPS: usize = 2;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args_os().skip(1);
    let input_path = args.next().map(PathBuf::from).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: cargo run --release --example measure_baseline -- <input.rtf> [iterations] [warmups]",
        )
    })?;
    let iterations = parse_count(args.next(), "iterations", DEFAULT_ITERATIONS)?;
    let warmups = parse_count(args.next(), "warmups", DEFAULT_WARMUPS)?;
    if args.next().is_some() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "too many arguments").into());
    }
    if iterations == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "iterations must be greater than zero",
        )
        .into());
    }

    let input = fs::read(&input_path)?;
    let options = ConvertOptions::browser_safe_defaults();

    for _ in 0..warmups {
        convert_rtf_to_pdf(&input, &options)?;
    }

    let mut durations = Vec::with_capacity(iterations);
    let mut expected_pdf = None::<Vec<u8>>;
    let mut expected_pages = None::<usize>;
    for iteration in 0..iterations {
        let started = Instant::now();
        let output = convert_rtf_to_pdf(&input, &options)?;
        durations.push(started.elapsed());

        if let Some(expected) = &expected_pdf {
            if output.pdf != *expected {
                return Err(io::Error::other(format!(
                    "PDF bytes changed during measured iteration {}",
                    iteration + 1
                ))
                .into());
            }
        } else {
            expected_pdf = Some(output.pdf.clone());
        }
        if let Some(expected) = expected_pages {
            if output.pages != expected {
                return Err(io::Error::other(format!(
                    "page count changed during measured iteration {}",
                    iteration + 1
                ))
                .into());
            }
        } else {
            expected_pages = Some(output.pages);
        }
    }

    durations.sort_unstable();
    let pdf = expected_pdf.expect("positive iteration count guarantees output");
    let pages = expected_pages.expect("positive iteration count guarantees a page count");
    let parsed_pdf = Document::load_mem(&pdf)?;
    if parsed_pdf.get_pages().len() != pages {
        return Err(io::Error::other("conversion and parsed-PDF page counts disagree").into());
    }

    println!("input={}", input_path.display());
    println!("input_bytes={}", input.len());
    println!("iterations={iterations}");
    println!("warmups={warmups}");
    println!(
        "median_ms={:.3}",
        median(&durations).as_secs_f64() * 1_000.0
    );
    println!("pdf_bytes={}", pdf.len());
    println!("pdf_sha256={:x}", Sha256::digest(&pdf));
    println!("pages={pages}");
    println!("pdf_indirect_objects={}", parsed_pdf.objects.len());

    Ok(())
}

fn parse_count(
    value: Option<std::ffi::OsString>,
    label: &str,
    default: usize,
) -> Result<usize, Box<dyn Error>> {
    let Some(value) = value else {
        return Ok(default);
    };
    let value = value.into_string().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} must be valid UTF-8"),
        )
    })?;
    value.parse::<usize>().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid {label} value {value:?}: {error}"),
        )
        .into()
    })
}

fn median(sorted: &[Duration]) -> Duration {
    let middle = sorted.len() / 2;
    if sorted.len() % 2 == 1 {
        sorted[middle]
    } else {
        sorted[middle - 1].saturating_add(sorted[middle]) / 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_selects_middle_odd_duration() {
        let values = [
            Duration::from_millis(1),
            Duration::from_millis(3),
            Duration::from_millis(9),
        ];
        assert_eq!(median(&values), Duration::from_millis(3));
    }

    #[test]
    fn median_averages_middle_even_durations() {
        let values = [
            Duration::from_millis(1),
            Duration::from_millis(3),
            Duration::from_millis(9),
            Duration::from_millis(11),
        ];
        assert_eq!(median(&values), Duration::from_millis(6));
    }
}
