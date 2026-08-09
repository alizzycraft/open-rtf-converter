# Architecture

Status: Maintained

This document describes the architecture after completion of the 2026 improvement plan. The implementation history and acceptance evidence remain in `architecture-improvement-plan.md`.

## Supported boundary

The crate root is the only supported library boundary. It exposes:

- one-shot and reusable conversion through `convert_rtf_to_pdf`, `convert_rtf_bytes_to_pdf`, and `Converter`;
- bounded font-family inspection through `inspect_rtf`;
- configuration, limits, diagnostics, font inputs, and typed errors needed by those operations;
- the standalone `audit_passive_pdf_bytes` defense-in-depth check;
- a CLI-only filesystem convenience function when the `cli` feature is enabled.

Parser tokens, the normalized document model, layout records, PDF plans, and raw render functions are crate-private. The non-default `fuzzing` feature exposes only payload-opaque lexer/parser exercise hooks and is not a stable downstream API.

## Conversion pipeline

```text
RTF bytes + ConvertOptions
          |
          v
 validate limits and prepare immutable font catalog
          |
          v
 bounded lexer/parser --> typed normalized document + diagnostics
          |
          v
 layout engine + conversion-local bounded FontSession
          |
          v
 exact conversion-local PdfPlan
          |
          v
 checked PDF serialization --> size/page/item/object limits
          |
          v
 passive PDF audit --> ConversionOutput
```

Each conversion owns its mutable state. `Converter` reuses only validated immutable font preparation; every `convert(&self)` call creates an isolated `FontSession` and `PdfPlan`. There is no global mutable cache.

## Dependency direction

- `config`, `diagnostics`, and `fonts` define bounded inputs and reporting types.
- `rtf` consumes parser policy and produces the crate-private typed model.
- `layout` consumes the model and font session; it does not serialize PDF.
- `pdf` consumes layout and prepared fonts through an exact plan; it does not parse RTF.
- `convert` owns orchestration, final limits, and the passive-output audit.
- `inspect` provides a narrow metadata projection without exposing the model.
- `lib.rs` defines and documents the supported facade.

The core conversion path has no filesystem, network, browser, system-font, or office-suite dependency. Filesystem access is confined to the `cli` feature and is excluded from `wasm32-unknown-unknown` builds.

## Security and resource ownership

Untrusted input is bounded before or while resources are accumulated. `RtfLimits` covers input, tokens, destinations, media, model growth, layout, pages, indirect objects, and final PDF bytes. Signed geometry/range relationships are validated before lexing. Font assets have independent per-asset and aggregate limits.

Active RTF behavior is stripped, replaced, or rejected according to policy. PDF links are separately governed. The final audit rejects forbidden active PDF names and malformed stream boundaries, but it is deliberately not advertised as a general malware verdict for arbitrary PDFs.

Semantic content uses typed model variants rather than private-use sentinel characters. Model invariants are checked at the parser boundary. PDF object references are assigned by a checked contiguous allocator before serialization, and the renderer consumes the same plan used for limit accounting.

Errors and diagnostics contain bounded categories, values, and offsets rather than raw hostile payloads.

## Font architecture

`FontAsset` stores validated bytes in `Arc<[u8]>`, so cloned configuration does not duplicate font programs. `PreparedFontCatalog` normalizes family/style selection once. A bounded `FontSession` caches selection, glyph, and kerning results only for one conversion and stops inserting when a cache reaches its derived capacity.

Font parser views are recreated from shared bytes; the catalog does not use self-referential borrowed structures.

## PDF architecture

`PdfPlan` borrows layout and font payloads while owning compact derived records. It assigns every catalog, page, content, font, CMap, image, and mask reference before output allocation. Planning checks object overflow and the configured object limit. Rendering performs no second resource-discovery traversal.

The final byte limit and passive audit remain post-serialization checks because they depend on the emitted file.

## Test boundaries

White-box model, parser, layout, PDF, and security tests live inside the crate. Normal integration tests compile only against the supported root facade. Two black-box security tests preserve end-to-end policy claims. The separate fuzz crate explicitly enables the non-default opaque hooks.

`tests/test_structure.rs` enforces test inventories, facade-only integration imports, private implementation modules, the single plan-backed PDF resource path, typed-model sentinel removal, and narrow fuzz hooks.

## Compatibility and releases

The crate follows pre-1.0 semantic versioning. The 0.2.0 boundary intentionally removes accidentally public implementation modules while retaining the documented root conversion facade. Downstream migration guidance is in `migration-0.2.md`; release changes are recorded in the repository `CHANGELOG.md`.

## Maintainer gates

The authoritative commands are listed in the README and CI workflow. They include formatting, native/all-feature checks, strict rustdoc, configured Clippy categories, all native test partitions, fuzz-target compilation/regression, a bounded nightly Linux libFuzzer smoke job, and the no-default-features WASM check. Deterministic hashes and the repeatable local benchmark are maintained in `performance-baseline.md`.
