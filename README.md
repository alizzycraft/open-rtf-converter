# open-rtf-converter

A Rust CLI for converting RTF documents to PDF without shelling out to office suites, browser print engines, Pandoc, or existing RTF converter libraries.

The current implementation owns the RTF tokenization/parsing, document model, pagination, and PDF drawing pipeline. It uses primitive crates for CLI parsing, PDF object writing, font discovery, and Unicode line-break opportunities.

## Usage

```powershell
cargo run -- fixtures/simple.rtf target/simple.pdf --diagnostics
```

Supported output format in this first version is PDF.

## Current Scope

- Parses RTF groups, controls, escaped text, unicode escapes, font tables, color tables, page settings, paragraph styles, and character styles.
- Renders paragraphs, basic styling, alignment, page breaks, underline strokes, and simple pagination.
- Warns on unsupported controls and keeps converting when the document structure is still usable.

Tables, embedded images, headers/footers, full font embedding/subsetting, and high-fidelity Word-compatible layout are planned follow-up areas.
