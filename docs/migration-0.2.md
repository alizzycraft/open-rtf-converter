# Migrating from 0.1 to 0.2

Version 0.2 establishes the crate-root facade as the supported API. Normal callers that already use `convert_rtf_to_pdf`, `convert_rtf_bytes_to_pdf`, or the CLI do not need to change conversion behavior.

## Raw modules are private

The following former implementation paths are no longer accessible downstream:

```text
open_rtf_converter::config
open_rtf_converter::convert
open_rtf_converter::diagnostics
open_rtf_converter::fonts
open_rtf_converter::layout
open_rtf_converter::model
open_rtf_converter::pdf
open_rtf_converter::rtf
```

Import supported types and functions directly from `open_rtf_converter` instead:

```rust
use open_rtf_converter::{
    ConvertOptions, Converter, RtfParseOptions, audit_passive_pdf_bytes,
    convert_rtf_to_pdf, inspect_rtf,
};
```

There is no replacement public API for constructing normalized model/layout records, tokenizing raw RTF, calling the parser directly, estimating raw renderer objects, or rendering a layout directly. Those paths bypassed parts of the supported validation, limit, rendering, and audit boundary.

## Metadata inspection

Code that parsed an RTF document only to discover requested fonts should use the bounded projection:

```rust
use open_rtf_converter::{RtfParseOptions, inspect_rtf};

let input = br"{\rtf1\fonttbl{\f0 Example Sans;}}";
let inspection = inspect_rtf(input, &RtfParseOptions::browser_safe_defaults())?;
for family in inspection.requested_font_families {
    println!("{family}");
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Font assets

`FontAsset` payload fields are private. Construct validated assets with `FontAsset::from_bytes` or `FontAsset::from_shared_bytes`, and read them through `family_names`, `style`, and `bytes`. Cloning shared assets or conversion options no longer copies font payload bytes.

For repeated conversions with the same fonts, create `Converter::new(options)` once and call `convert(&self, input)` for each document. One-shot functions remain supported and use the same conversion path.

## Configuration validation

Call `RtfLimits::validate`, `RtfParseOptions::validate`, or `ConvertOptions::validate` when configuration errors must be reported before an input is available. Conversion still validates automatically before lexing.

Every signed limit documents its unit and valid range in generated API documentation. Existing struct-literal configuration remains supported in 0.2; a limits builder or grouped limit structures are deferred to a future version.

## Fuzzing and advanced integrations

The `fuzzing` feature is non-default, unstable, and intended only for this repository's fuzz crate. Its hooks deliberately return no lexer token or normalized-model data. Downstream products should not depend on them.

If an application genuinely needs a lower-level parser or model API, open a design request describing the bounded product use case. A future advanced API would require its own versioned invariants and security contract rather than re-exposing internal modules.

## Output compatibility

The architecture migration preserves the recorded representative PDF hashes, page counts, object counts, and byte sizes. It does not deliberately change active-content policy or rendering fidelity. As always for document conversion, validate important application fixtures when upgrading.
