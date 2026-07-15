# open-rtf-converter

A Rust CLI and library core for converting RTF documents to passive PDF output without shelling out to office suites, browser print engines, Pandoc, or existing RTF converter libraries.

The implementation owns the RTF tokenization/parsing, normalized document model, layout, pagination, and PDF drawing pipeline. The core API converts in-memory RTF bytes to PDF bytes so it can be used by CLI, desktop, server-worker, and browser/WASM wrappers.

## Usage

```powershell
cargo run -- fixtures/simple.rtf target/simple.pdf --diagnostics
```

Supported output format in this first version is PDF.

## Current Scope

- Parses bounded RTF groups, controls, escaped text, Unicode/code-page data, font/color/style/list tables, fields, objects, pictures, shapes, tables, headers/footers, footnotes/endnotes, sections, and common Word-generated metadata.
- Renders passive text, fonts, paragraph formatting, lists, tables, page/section layout, headers/footers, notes, static images, static drawing shapes, and safe object/field fallback results.
- Strips, placeholders, or rejects active content such as OLE payloads, OCX controls, executable packages, external-resource fields, object metadata, and unsafe PDF features.
- Supports browser-safe defaults with stricter limits, no filesystem requirement in the core conversion API, caller-provided font assets, and `wasm32-unknown-unknown` compatibility for the no-default-features core.
- Emits diagnostics for intentional security differences and layout approximations.

Remaining fidelity work is tracked incrementally against `docs/project_plan.md`; Word-exported reference PDFs/PNGs are still needed before claiming full visual coverage.
