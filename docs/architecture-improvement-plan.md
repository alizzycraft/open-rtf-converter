# Architecture Improvement Project Plan

Status: Draft for implementation

Last updated: 2026-08-08

Owner: open-rtf-converter maintainers

## 1. Purpose

This document turns the architectural review into an implementation plan for improving the internal structure, public API contract, runtime ownership, and delivery safeguards of `open-rtf-converter`.

The work is intended to preserve the converter's current strengths:

- bounded, deterministic in-memory conversion;
- passive rendering and defense-in-depth PDF auditing;
- Microsoft Word-compatible passive behavior where safely supportable;
- browser/WASM compatibility without native system dependencies;
- a large security, conversion, fuzz-regression, and reference-fixture suite;
- the existing stage order from RTF bytes to a normalized model, layout, PDF, and final audit.

This is an evolutionary plan, not a rewrite. Each phase must leave the repository buildable and testable. Pure structural changes must be behavior-preserving and must not be combined with fidelity changes.

## 2. Desired outcomes

When this plan is complete:

1. The supported conversion API is small, documented, and clearly separated from unchecked implementation internals.
2. The safe conversion façade is the normal route to PDF output and is the only supported route that claims conversion limits and final passive-PDF auditing.
3. The normalized document model represents semantic inline content with typed nodes instead of private-use Unicode sentinels embedded in ordinary strings.
4. Related model data is stored in aggregate records instead of index-coupled parallel vectors.
5. Parser, layout, and PDF responsibilities are split into cohesive internal modules without changing stage direction.
6. Font inputs are validated and indexed once, their byte storage is cheaply shared, and per-conversion metric caches are bounded and local.
7. PDF object planning and PDF emission use the same resource plan, eliminating estimator/renderer drift.
8. Limit validation is a first-class public operation with typed errors.
9. Continuous integration enforces formatting, compilation, relevant Clippy groups, the full test matrix, and WASM compatibility.
10. Architectural rules, compatibility requirements, and performance expectations are documented well enough for background agentic work to proceed safely.

## 3. Current-state snapshot

This snapshot is evidence for the plan and should be refreshed before implementation starts.

### 3.1 Pipeline and dependency direction

The production dependency direction is currently acyclic and should be preserved:

```text
config ───────────────┐
                     v
RTF bytes -> lexer -> parser -> normalized model -> layout -> PDF writer
                         |             |              |         |
                         v             v              v         v
                    diagnostics      fonts          fonts   passive audit
                                                                  |
                                                                  v
                                                       ConversionOutput
```

`src/convert.rs::convert_rtf_to_pdf` is the orchestration point. It currently:

1. validates the caller's `FontProvider`;
2. parses with `RtfParseOptions`;
3. lays out the normalized `Document`;
4. gathers optional substitution and missing-glyph diagnostics;
5. checks layout-item, page, and estimated indirect-object limits;
6. renders PDF bytes;
7. checks the output-byte limit;
8. audits the completed PDF for forbidden active features.

That ordering is a core security property and must remain visible in the target design.

### 3.2 Source concentration

At the date of this draft:

| File | Physical lines | Principal responsibilities |
| --- | ---: | --- |
| `src/rtf/parser.rs` | 77,494 | parser state, destinations, controls, fields, formulas, tables, lists, shapes, images, JPEG, PNG, DIB, EMF, WMF, normalization, and 845 unit tests |
| `src/layout.rs` | 31,625 | page geometry, pagination, notes, tables, shapes, paragraph wrapping, text metrics, late fields, and 319 unit tests |
| `src/pdf.rs` | 11,707 | PDF safety audit, resource analysis, font encoding, image writing, vector drawing, object assignment, content writing, and 96 unit tests |
| `tests/security.rs` | 101,481 | 1,043 security and compatibility tests spanning most subsystems |
| `tests/reference_fixtures.rs` | 7,129 | reference-PDF and visual-fixture assertions |

The repository contains 2,400 `#[test]` functions across `src/` and `tests/` at this snapshot.

### 3.3 Public API surface

`src/lib.rs` currently declares all of these modules public:

```rust
pub mod config;
pub mod convert;
pub mod diagnostics;
pub mod fonts;
pub mod layout;
pub mod model;
pub mod pdf;
pub mod rtf;
```

This exposes the raw lexer, parser output model, layout model, object estimator, and renderer alongside the safe conversion façade. Direct calls to `pdf::render_pdf` or `pdf::render_pdf_with_font_provider` do not run the conversion-level page, item, object, or byte checks and do not run the final audit.

Several internal consumers currently depend on this public surface:

- `src/main.rs` parses the input directly to discover requested font families;
- `tests/security.rs` inspects parser, model, and layout internals;
- `tests/fuzz_regression.rs` directly invokes the lexer and parser;
- `tests/reference_fixtures.rs` and `tests/fuzz_regression.rs` directly invoke the PDF audit;
- the `fuzz/` crate directly exercises lexer, parser, and full conversion targets.

The public boundary therefore cannot be narrowed in one edit. Consumers must be migrated first or given intentionally narrow test/fuzz hooks.

### 3.4 Model invariants

`src/model.rs` currently uses private-use Unicode values and formatted marker strings for semantic content including:

- current page and total page fields;
- section and document statistics;
- bookmark page anchors and references;
- footnote and endnote references;
- inline-image references;
- passive layout advances;
- passive-math fraction and stack markers;
- nested-table anchors.

These values are stored inside `Run { text: String, style: CharacterStyle }` and interpreted later by layout code.

`Document` also uses parallel vectors whose indexes must remain aligned, including:

- `footnotes`, `footnote_section_indices`, and `footnote_block_indices`;
- `endnotes`, `endnote_section_indices`, and `endnote_placements`;
- `inline_images` and `inline_image_block_indices`.

Because most model fields are public, invalid combinations can be constructed outside the parser.

### 3.5 Font ownership

`FontProvider` owns `Vec<FontAsset>`, and each `FontAsset` owns `Vec<u8>`. The bundled TTF assets total 4,154,404 bytes. `FontProvider` and `ConvertOptions` are cloneable, so a clone deep-copies those bytes.

Font validation and glyph operations call `ttf_parser::Face::parse` in several paths across `fonts.rs`, `layout.rs`, and `pdf.rs`. Recent selection and encoding optimizations substantially improved normal conversion performance, so the proposed prepared-font work is primarily about ownership, validation, and maintainable caching rather than promising another large speedup.

### 3.6 PDF analysis

`estimate_passive_pdf_object_count` and `render_pdf_with_font_provider` independently derive overlapping information:

- page-used built-in fonts;
- supplied font assets and encodings;
- extended-Latin mappings;
- image XObjects and masks;
- per-page resources.

The estimator and renderer currently agree through tests, but the duplicated derivation creates a future drift risk. A previous broad resource-cache experiment was not retained because its bounded-memory form produced only a small performance gain for substantial complexity. Any new PDF plan must therefore justify itself through exactness and ownership, remain local to one conversion, and avoid adding caches to the public layout model.

### 3.7 Toolchain and automation

The repository currently has:

- Rust edition 2024;
- a working local toolchain of Rust 1.96.0;
- no `rust-version` package field;
- no `rust-toolchain.toml`;
- no `.github/workflows` CI configuration.

The core `wasm32-unknown-unknown` dependency tree is small and contains no native system library. That property is already checked by `tests/wasm_dependency_policy.rs` and must remain enforced.

## 4. Architectural principles

All implementation work under this plan must follow these rules.

### 4.1 Preserve the conversion security boundary

The supported conversion path must always perform:

```text
configuration validation
-> bounded parse
-> bounded normalized model
-> bounded layout
-> exact PDF resource planning
-> pre-render object limit
-> rendering
-> post-render byte limit
-> passive PDF audit
```

No optimization may bypass a limit or audit. Failed validation must return an error before expensive processing begins.

### 4.2 Keep stage dependencies one-way

The target production import rules, written as `consumer -> allowed dependency`, are:

```text
cli     -> supported crate façade
convert -> config, diagnostics, fonts, rtf, layout, pdf
pdf     -> layout, fonts
layout  -> model, fonts
rtf     -> config, diagnostics, model
fonts   -> external font parser only
model, config, diagnostics -> no upper conversion stage
```

More precisely:

- `config` and `diagnostics` do not depend on parser, model, layout, or PDF;
- `model` contains safe normalized data and does not depend on parser, layout, or PDF;
- `rtf` may depend on config, diagnostics, and model;
- `layout` may depend on model and prepared font interfaces, but not parser internals;
- `pdf` may depend on render-ready layout data and prepared font interfaces, but not RTF parser types;
- `convert` owns orchestration and may depend on all stages;
- `main.rs` uses the supported library façade rather than parser/model internals.

Test-only dependencies must not leak into production imports. Tests that exercise multiple stages may compose through the façade or through explicitly scoped crate-private test helpers.

### 4.3 Separate extraction from behavior change

A commit that moves existing functions or tests must not also change rendering behavior, diagnostics, limits, or public policy. Behavior changes require their own tests and commits after the extraction is complete.

For structural commits, existing fixture PDFs should remain byte-for-byte identical unless object ordering is intentionally changed by the PDF planning work. Where byte identity is not possible, structural PDF equivalence, extracted text, page geometry, passive-audit success, and approved reference-image comparisons are required.

### 4.4 Avoid hidden global state

Prepared indexes and caches must be owned by a converter, font catalog, or single conversion session. Do not add global mutable caches, process-wide font discovery, filesystem discovery in core code, network access, or browser APIs.

### 4.5 Keep memory bounded

New plans, indexes, and typed model nodes must remain bounded by existing input, token, destination, model, layout-item, image, font, page, object, and output limits. New auxiliary collections must use saturating or checked arithmetic when their size is derived from untrusted input.

## 5. Target module structure

The following is a target, not a requirement for one large move:

```text
src/
  lib.rs                         supported public façade
  config.rs                      policies, limits, typed validation
  diagnostics.rs
  convert.rs                     conversion orchestration

  fonts/
    mod.rs                       public input types and builder
    catalog.rs                   validated/indexed internal catalog
    metrics.rs                   selection and bounded session caches
    bundled.rs                   bundled asset declarations

  model/
    mod.rs
    document.rs
    inline.rs                    typed inline nodes and identifiers
    style.rs
    section.rs
    table.rs
    image.rs
    shape.rs

  rtf/
    mod.rs
    lexer.rs
    parser/
      mod.rs                     entry points and Parser core
      state.rs
      controls.rs
      destinations.rs
      text.rs                    Unicode and code-page handling
      fields/
        mod.rs
        instructions.rs
        formula.rs
        formatting.rs
      tables.rs
      lists.rs
      styles.rs
      shapes.rs
      media/
        mod.rs
        jpeg.rs
        png.rs
        dib.rs
        emf.rs
        wmf.rs

  layout/
    mod.rs                       LayoutEngine façade
    document.rs                  render-ready layout data
    page.rs
    paragraph.rs
    line_break.rs
    text_metrics.rs
    tables.rs
    notes.rs
    shapes.rs
    fields.rs                    late page/note/bookmark resolution

  pdf/
    mod.rs                       crate-private PDF façade
    audit.rs                     passive PDF byte audit
    plan.rs                      exact resource and object plan
    writer.rs
    fonts.rs
    images.rs
    vector.rs
    text.rs
```

Small modules should remain grouped where splitting would create excessive visibility plumbing. File count is not a goal by itself; cohesive ownership is.

## 6. Workstream A: continuous integration and toolchain policy

### A.1 Problem

The repository has a large and security-sensitive test suite but no repository-level automation. Structural work without an independent gate is too risky, especially when commits are produced in the background.

### A.2 Technical specification

Add:

- `rust-toolchain.toml`;
- a `rust-version` entry in `Cargo.toml`;
- `.github/workflows/ci.yml`;
- optionally `.github/dependabot.yml` after CI is stable;
- a short quality-gates section in `README.md` or `CONTRIBUTING.md`.

Initial toolchain policy:

- pin Rust 1.96.0 in `rust-toolchain.toml`, because that is the verified working toolchain at the time of this draft;
- set `rust-version = "1.96"` initially;
- install `rustfmt`, `clippy`, and `wasm32-unknown-unknown` through the toolchain file;
- treat lowering the MSRV as a separate measured task rather than claiming the edition's theoretical minimum without testing dependencies and code.

The initial CI matrix must run on a hosted Linux runner and contain these logical jobs:

1. `format-and-check`
   - `cargo fmt --all -- --check`
   - `cargo check --all-targets --all-features`
   - `cargo clippy --all-targets --all-features -- -D clippy::correctness -D clippy::suspicious -D clippy::perf`
2. `core-tests`
   - `cargo test --lib`
   - `cargo test --bin open-rtf-converter`
   - `cargo test --test conversion`
3. `security-tests`
   - `cargo test --test security`
4. `reference-and-regression`
   - `cargo test --test reference_fixtures`
   - `cargo test --test fuzz_regression`
5. `wasm-core`
   - `cargo check --no-default-features --target wasm32-unknown-unknown`
   - `cargo test --test wasm_dependency_policy`

The jobs may cache Cargo registry and target artifacts, but cache keys must include the lockfile and toolchain. CI must use `Cargo.lock` and must not run `cargo update` implicitly.

The first CI version should use the targeted Clippy groups above. The existing repository is not required to pass `-D warnings` or all style/pedantic lints before this plan can begin. A separate cleanup may progressively enable additional lints without mixing those edits into architecture changes.

### A.3 Acceptance criteria

- Every CI job passes on `main`.
- A deliberately malformed formatting change fails the format job in a test branch.
- A deliberately disabled active-content regression test demonstrates that the security job is actually executed.
- The no-default-features WASM build remains free of CLI and native-only dependencies.
- The workflow has a concurrency group that cancels superseded runs for the same branch without cancelling `main` validation.
- CI documentation lists the equivalent local commands.

### A.4 Risks and rollback

Reference and security jobs may be long. Split them rather than omitting them. If hosted-runner time becomes excessive, retain required core/security checks and move expensive visual or fuzz-regression checks to a scheduled plus pull-request path-filtered job. Never make security coverage manual-only.

## 7. Workstream B: define and enforce the supported API boundary

### B.1 Problem

All implementation modules are public. The safe conversion façade and lower-level unchecked APIs appear equally supported, even though only the façade applies the complete conversion policy.

### B.2 Supported API target

The crate root should expose these supported categories:

```rust
// Conversion
pub use convert::{
    convert_rtf_bytes_to_pdf,
    convert_rtf_to_pdf,
    ConversionOutput,
    ConvertError,
    ConvertOptions,
    ConvertReport,
};

// Native file convenience, feature-gated as today
pub use convert::convert_rtf_file_to_pdf;

// Configuration and validation
pub use config::{
    ActiveContentPolicy,
    CompatibilityMode,
    LimitValidationError,
    PdfLinkPolicy,
    RtfLimits,
    RtfParseOptions,
};

// Diagnostics
pub use diagnostics::{Diagnostic, Severity};

// Font input/configuration
pub use fonts::{
    FontAsset,
    FontAssetStyle,
    FontCoverage,
    FontProvider,
    FontProviderError,
    FontProviderLimits,
};

// Standalone output verification utility
pub use pdf::{
    audit_passive_pdf_bytes,
    PassivePdfError,
    PassivePdfIssue,
};

// Focused document inspection used by CLI font loading
pub use inspect::{
    inspect_rtf,
    InspectError,
    RtfInspection,
};
```

`layout`, `model`, the raw PDF renderer, PDF planner, lexer, and parser implementation should become crate-private after their current consumers are migrated.

The standalone PDF audit remains public because it is independently useful and does not bypass a conversion limit. Its documentation must state that a successful audit only checks the converter's forbidden-feature policy and is not a general-purpose PDF malware verdict.

### B.3 Crate-level documentation

Add crate documentation to `lib.rs` describing:

- the supported bytes-to-bytes API;
- the difference between normal and browser-safe defaults;
- the passive-content security claim;
- the absence of filesystem/network/system-font discovery in the core;
- the fact that direct internal stages are not a supported security boundary;
- a minimal conversion example that compiles as a doctest.

Add `#![warn(missing_docs)]` only after the supported surface has documentation. Do not enable it while hundreds of accidental public internals remain.

### B.4 Focused inspection API

The CLI currently parses the complete document to discover font families. Replace that dependency with:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtfInspection {
    pub requested_font_families: Vec<String>,
}

pub fn inspect_rtf(
    input: &[u8],
    options: &RtfParseOptions,
) -> Result<RtfInspection, InspectError>;
```

`InspectError` should preserve typed configuration, lexical, parsing, and resource-limit causes without exposing the private normalized model.

The first implementation may reuse the bounded parser and project its result. It must not create a second, less-bounded font-table parser. Later optimization may stop after safe font-table discovery only if identical limit and malformed-input behavior is retained.

The API must return primary and alternate family names, deduplicate them case-insensitively, and preserve first-seen display spelling and deterministic order. Do not add style usage or other document inspection fields until a concrete supported caller needs them.

### B.5 Test and fuzz migration

Narrowing proceeds in stages:

1. Document the supported root façade while keeping existing modules temporarily public and `#[doc(hidden)]`.
2. Move CLI font discovery to `inspect_rtf`.
3. Keep black-box security assertions in `tests/security/`, but move assertions that require private model/layout state into subsystem unit-test modules as those modules are extracted.
4. Keep conversion no-panic fuzz regression through the public façade.
5. Expose raw lexer/parser fuzz entry points only under a non-default `fuzzing` feature in a `#[doc(hidden)]` module. `fuzz/Cargo.toml` must enable that feature explicitly.
6. Re-export the passive PDF audit at the crate root and migrate external test imports.
7. Make implementation modules private once repository consumers no longer depend on them.

The `fuzzing` feature is not a supported application API. It must not be enabled by default or by the `cli` feature, and its build must remain compatible with the fuzz crate's target environment.

### B.6 Acceptance criteria

- The README and crate docs recommend only the supported façade.
- `src/main.rs` imports no `rtf`, `model`, `layout`, or raw `pdf` module.
- Normal integration tests import no private implementation module.
- Fuzz targets compile with their explicit non-default hook feature.
- A downstream compile test verifies the supported root API.
- A compile-fail or source-policy test prevents accidental reintroduction of `pub mod layout`, `pub mod model`, raw `pub mod pdf`, or raw `pub mod rtf` at the crate root.
- All existing CLI behavior and tests remain unchanged.

### B.7 Compatibility policy

The crate is currently version 0.1.0, so this is the appropriate period to narrow accidental public surface. Nevertheless:

- record removals and replacements in a changelog;
- provide one release with deprecation annotations where practical;
- do not deprecate the supported conversion façade;
- do not promise semantic version stability for `#[doc(hidden)]` fuzz hooks;
- consider a 0.2.0 release when the public boundary becomes enforced.

## 8. Workstream C: replace implicit model protocols with typed data

### C.1 Problem

Private-use Unicode marker strings and parallel vectors encode invariants that Rust's type system could enforce. This protocol couples parser, model, and layout through string searching and replacement and makes external construction of a valid model difficult.

### C.2 Target inline model

Introduce typed identifiers and inline nodes internally:

```rust
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct InlineImageId(usize);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct NoteId(usize);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct BookmarkId(usize);

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Inline {
    pub kind: InlineKind,
    pub style: CharacterStyle,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum InlineKind {
    Text(String),
    DynamicField(DynamicField),
    InlineImage(InlineImageId),
    FootnoteReference(NoteId),
    EndnoteReference(NoteId),
    BookmarkPageAnchor(BookmarkId),
    BookmarkPageReference(BookmarkId),
    LayoutAdvance(LayoutAdvance),
    Math(MathInline),
    NestedTableAnchor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DynamicField {
    PageNumber,
    TotalPages,
    SectionNumber,
    SectionPages,
    DocumentWords,
    DocumentCharacters,
    DocumentCharactersWithSpaces,
}
```

The exact names may change during implementation, but the following properties are mandatory:

- semantic controls are not represented as user-visible text;
- a text node cannot accidentally become a field by containing a private-use character;
- styles remain explicit on each inline unit;
- adjacent text with equal style can still be coalesced to avoid excessive node counts;
- typed identifiers are validated before dereference;
- late fields remain resolvable during pagination without string replacement.

### C.3 Aggregate related records

Replace parallel vectors with aggregate records:

```rust
pub(crate) struct Footnote {
    pub content: Paragraph,
    pub section_index: usize,
    pub body_block_index: usize,
}

pub(crate) struct Endnote {
    pub content: Paragraph,
    pub section_index: usize,
    pub placement: EndnotePlacement,
}

pub(crate) struct InlineImageEntry {
    pub image: StaticImage,
    pub body_block_index: usize,
}
```

Use newtype indexes where accidental interchange is plausible, for example `SectionIndex`, `BlockIndex`, `InlineImageId`, and `NoteId`. Do not wrap every integer indiscriminately; prioritize indexes that cross module boundaries or index separate collections.

### C.4 Transitional invariant validation

Before changing representation, add:

```rust
pub(crate) fn validate_document_invariants(
    document: &Document,
) -> Result<(), ModelInvariantError>;
```

It must check at least:

- all parallel-vector lengths and referenced indexes;
- inline-image identifiers;
- footnote/endnote identifiers;
- font and color indexes used by styles;
- section and block indexes;
- any remaining marker encoding during migration;
- layout-recursive wrapper bounds already governed by conversion limits.

Run this validator in debug/test builds at the parser boundary first. Before removal of the old representation, add a release-safe validation point where malformed externally constructed models can enter layout through any temporarily public advanced API.

The validator is migration scaffolding, not a substitute for typed invariants. Remove checks that become impossible by construction after migration, but retain checks for indexes derived from untrusted input.

### C.5 Migration sequence

Use a dual-representation migration only within tightly scoped commits. Do not maintain marker and typed protocols indefinitely.

Recommended order:

1. Add invariant validation and tests for deliberately inconsistent documents.
2. Introduce typed identifier definitions.
3. Aggregate inline images and note records while preserving parser/layout behavior.
4. Convert current-page, total-page, section, and document-statistic fields to `DynamicField`.
5. Convert inline images and note references.
6. Convert bookmark anchors/references and layout advances.
7. Convert nested-table anchors.
8. Convert passive-math marker and style-ID protocols as a dedicated subproject.
9. Remove marker constants, marker parsing, sanitizing, and string replacement helpers.
10. Make normalized model fields private to their owning modules and add narrow constructors/accessors where needed.

Passive math is last because it currently combines marker characters with multiple `CharacterStyle` identifiers and grouping flags. Its migration should define a typed math subtree or typed inline grouping before removing any existing IDs.

### C.6 Layout changes

Layout must consume `InlineKind` by matching typed variants:

- `Text` enters normal line breaking and measurement;
- `DynamicField` receives an initial or late-layout representation depending on field kind;
- image and note references become atomic inline boxes;
- bookmark anchors are zero-width typed events rather than text fragments;
- late page-count resolution updates typed fragment metadata or regenerates the affected fragment, not global string replacement;
- passive math exposes explicit grouping data to measurement and wrapping.

The final `LayoutDocument` should contain render-ready content. PDF code should not need to know RTF field markers or parse semantic strings.

### C.7 Acceptance criteria

- Production code contains no semantic private-use marker constants for fields, notes, bookmarks, images, tables, or layout advances.
- Literal private-use characters from an RTF input remain literal text or are sanitized according to an explicit policy; they cannot trigger internal semantics.
- Parallel note/image metadata vectors no longer exist.
- Invalid identifiers return typed parse/model errors rather than indexing panics.
- Existing conversion, security, reference, and fuzz-regression tests pass.
- Structural/model phases preserve PDF bytes for the existing deterministic fixture set.
- New tests cover adjacent styled text coalescing, markers mixed with ordinary text, late fields, notes, bookmarks, inline images, nested tables, and passive math.

### C.8 Risks and rollback

The largest risks are line-breaking drift and late-field pagination feedback. Migrate one semantic family at a time and keep old/new output comparison helpers in tests. If a migration changes layout unexpectedly, revert that semantic family without reverting already completed aggregate-record changes.

## 9. Workstream D: decompose parser, layout, PDF, and tests

### D.1 Problem

Large files obscure responsibility boundaries, create merge conflicts, and make extraction or ownership changes harder to review. The solution is cohesive modules, not arbitrary line-count targets.

### D.2 General extraction method

For each extraction:

1. identify a cohesive function/type cluster and its direct tests;
2. record baseline tests and representative PDF hashes;
3. move code without renaming or behavior change where possible;
4. use the narrowest visibility (`private`, `pub(super)`, then `pub(crate)`);
5. run formatting, targeted tests, full library/security tests, and relevant reference fixtures;
6. commit the extraction separately;
7. only then simplify APIs or data ownership.

Avoid circular sibling dependencies. Shared types should live in the nearest common parent module, not in a generic `utils.rs` dumping ground.

### D.3 Parser decomposition

Initial low-coupling extraction candidates already visible in `parser.rs` include:

- shape property parsing and point normalization;
- formula parsing/evaluation and numeric picture formatting;
- code-page byte decoding;
- JPEG parsing and bounded JPEG-to-RGB conversion;
- PNG parsing and normalization;
- DIB parsing, RLE decoding, and raster normalization;
- EMF record parsing and passive vector command recovery;
- WMF record parsing and passive vector command recovery.

Recommended parser phases:

1. `rtf/parser/media/{jpeg,png,dib}.rs`
2. `rtf/parser/media/{emf,wmf}.rs`
3. `rtf/parser/fields/{formula,formatting}.rs`
4. `rtf/parser/shapes.rs`
5. `rtf/parser/text.rs` for code-page and Unicode helpers
6. `rtf/parser/{tables,lists,styles}.rs`
7. stateful destination/control dispatch only after earlier seams are stable

Media decoders must continue receiving explicit byte/pixel/point limits. Extracted functions must not silently use `RtfLimits::default()`; limits must be passed from the active parse session.

Keep `Parser` and `ParserState` in `parser/mod.rs` or `parser/state.rs` until moving stateful methods produces a clear improvement. Child modules may use `pub(super)` methods, but widespread `pub(crate)` access is a sign that the boundary is wrong.

### D.4 Layout decomposition

Recommended cohesive groups:

- `notes.rs`: footnote/endnote placement, reservation, and numbering;
- `shapes.rs`: static shape placement, wrapping exclusions, and shape text;
- `tables.rs`: table, cell, padding, spacing, border, and vertical-flow layout;
- `paragraph.rs`: paragraph flow and frame handling;
- `line_break.rs`: segment construction, wrapping, hyphenation, emergency breaking, fit-text, and no-wrap behavior;
- `text_metrics.rs`: base-14/supplied-font measurement and kerning;
- `page.rs`: page geometry, columns, backgrounds, borders, headers, and footers;
- `fields.rs`: late page, section, note, and bookmark resolution.

`LayoutEngine::layout` and `layout_with_font_provider` remain the internal façade during extraction. The public conversion façade must not change.

The existing iterative `LayoutItem` fragment traversal should remain centralized. New wrapper variants must update the central traversal and recursive item count together, with a deep-wrapper regression test.

### D.5 PDF decomposition

Extract in this order:

1. `audit.rs`: passive PDF name and stream-boundary audit;
2. `images.rs`: alpha masks, RGB/indexed/grayscale image XObjects, compression decisions;
3. `fonts.rs`: built-in mappings, supplied-font encoding, CMaps, CID maps, font objects;
4. `vector.rs`: lines, polygons, fills, gradients, hatches, and recovered vector commands;
5. `text.rs`: normal and positioned text emission and passive glyph overlays;
6. `plan.rs`: resource discovery and exact object assignment;
7. `writer.rs`: catalog, pages, resources, streams, and final byte production.

`audit.rs` must not depend on the renderer. It should remain usable on arbitrary PDF bytes through the supported root utility.

Replace `standards_compliant_to_unicode_cmap`'s dependency on an `expect`ed upstream declaration with a typed `PdfRenderError::UnexpectedUnicodeCmapTemplate`. Rendering APIs inside the conversion path should return `Result`, even if the current writer usually cannot fail.

### D.6 Test decomposition

Split tests by subsystem while preserving their intent:

```text
tests/
  conversion/
    mod.rs
    text.rs
    tables.rs
    images.rs
    fields.rs
  security/
    mod.rs
    limits.rs
    malformed.rs
    active_content.rs
    fields.rs
    objects.rs
    images.rs
    pdf_audit.rs
  reference/
    mod.rs
    manifest.rs
    visual.rs
```

Cargo integration test targets can use a top-level `tests/security.rs` that declares submodules in `tests/security/`, or explicit `[[test]]` targets. Preserve existing test target names where CI and developer commands depend on them.

White-box tests should live beside their owning crate-private module. Black-box security claims should remain integration tests through the supported façade. Do not move a security assertion to a unit test merely to make a public boundary easier; preserve at least one end-to-end assertion for every security policy.

### D.7 Acceptance criteria

- The target production modules have documented single responsibilities.
- Parser media decoders are independently testable with explicit limits.
- PDF audit compiles without the renderer and remains publicly re-exported.
- Production imports obey the dependency rules in section 4.2.
- No extraction commit changes reference output unexpectedly.
- Test counts do not decrease without an explicit consolidation record showing equivalent coverage.
- The largest test targets are split into navigable domain files.
- No generic catch-all `utils`, `helpers`, or `common` module accumulates unrelated behavior.

## 10. Workstream E: prepared font catalog and cheap ownership

### E.1 Problem

Font configuration, owned bytes, validation, selection, metrics, and rendering concerns are currently concentrated in `FontProvider`. Public fields prevent internal representation changes, cloning duplicates megabytes of bundled fonts, and repeated face parsing appears across stages.

### E.2 Public input design

Make `FontAsset` fields private and supply constructors/builders:

```rust
#[derive(Debug, Clone)]
pub struct FontAsset {
    family_names: Vec<String>,
    style: FontAssetStyle,
    bytes: Arc<[u8]>,
}

impl FontAsset {
    pub fn from_bytes(
        family_names: Vec<String>,
        style: FontAssetStyle,
        bytes: Vec<u8>,
    ) -> Result<Self, FontProviderError>;

    pub fn from_shared_bytes(
        family_names: Vec<String>,
        style: FontAssetStyle,
        bytes: Arc<[u8]>,
    ) -> Result<Self, FontProviderError>;

    pub fn family_names(&self) -> &[String];
    pub fn style(&self) -> FontAssetStyle;
    pub fn bytes(&self) -> &[u8];
}
```

The owned-byte constructor is the simple path for callers loading files or browser buffers. The shared-byte constructor supports cheap application-level reuse. API clarity is more important than a highly generic constructor signature.

Bundled font bytes should use static `Arc<[u8]>`-compatible ownership so cloning a provider does not copy the TTF payloads. The core remains self-contained and performs no system-font discovery.

### E.3 Internal prepared catalog

Add a crate-private catalog:

```rust
pub(crate) struct PreparedFontCatalog {
    assets: Vec<PreparedFontAsset>,
    family_index: HashMap<NormalizedFamilyName, Vec<FontAssetId>>,
    fallback_assets: Vec<FontAssetId>,
    limits: FontProviderLimits,
}
```

Preparation must:

- validate asset count and byte limits with checked totals;
- parse every supplied face once to reject invalid data;
- normalize family aliases once;
- record style and stable input order;
- precompute selection metadata that does not borrow a `ttf_parser::Face`;
- retain bytes by shared ownership;
- avoid a self-referential structure holding a `Face` that borrows its own bytes.

`ttf_parser::Face` can be recreated as a cheap view when needed. Expensive repeated results should be cached in a conversion-local session, not by attempting to store a self-borrowing face.

### E.4 Conversion-local metric session

Add a bounded session owned by one conversion:

```rust
pub(crate) struct FontSession<'a> {
    catalog: &'a PreparedFontCatalog,
    selection_cache: HashMap<SelectionKey, Option<FontAssetId>>,
    glyph_cache: HashMap<GlyphMetricKey, Option<FontGlyphMetrics>>,
    kerning_cache: HashMap<KerningKey, f32>,
}
```

Cache bounds must be derived from existing font and output-text limits or from explicit small maxima. Reaching a cache bound should stop inserting and continue correctly without caching; it must not fail conversion or evict unpredictably in a way that changes output.

The same `FontSession` should serve layout and PDF planning where lifetime and ownership permit. If PDF planning needs information after mutable layout use ends, create a second session rather than placing caches in `LayoutDocument`.

### E.5 Optional reusable converter

After prepared ownership is stable, consider:

```rust
pub struct Converter {
    options: ConvertOptions,
    fonts: PreparedFontCatalog,
}

impl Converter {
    pub fn new(options: ConvertOptions) -> Result<Self, ConvertError>;
    pub fn convert(&self, input: &[u8]) -> Result<ConversionOutput, ConvertError>;
}
```

The existing one-shot functions remain and may construct a temporary `Converter`. A reusable converter amortizes font validation and indexing across documents. `convert(&self)` must create isolated per-call mutable sessions so `Converter` can remain free of global mutable state and can be made `Send + Sync` if its fields support that naturally.

Do not promise thread safety until compile-time assertions verify it on supported targets. WASM use in a single worker must remain valid regardless.

### E.6 Acceptance criteria

- Cloning default options does not copy bundled TTF byte buffers.
- Invalid assets fail during provider/catalog preparation before parsing or layout.
- Layout and PDF receive only validated prepared font state.
- Family/style selection preserves deterministic first-input-order tie breaking.
- Font fallback and diagnostics remain byte/text equivalent on current fixtures.
- Browser-safe font byte and asset limits remain enforced.
- Benchmarks show no material regression against the current optimized baseline.
- No unbounded or global font cache is introduced.

## 11. Workstream F: exact PDF resource planning

### F.1 Problem

Object estimation and rendering independently discover the same resources. This can drift as new PDF features are added and repeats some layout traversal.

### F.2 Target plan

Introduce a crate-private plan built immediately before the object-limit check:

```rust
pub(crate) struct PdfPlan<'a> {
    layout: &'a LayoutDocument,
    pages: Vec<PlannedPage>,
    builtin_fonts: Vec<PlannedBuiltinFont>,
    supplied_fonts: Vec<PlannedSuppliedFont>,
    images: Vec<PlannedImage>,
    object_count: usize,
    highest_object_id: i32,
}

pub(crate) fn plan_pdf<'a>(
    layout: &'a LayoutDocument,
    fonts: Option<&PreparedFontCatalog>,
    limits: &RtfLimits,
) -> Result<PdfPlan<'a>, PdfRenderError>;

pub(crate) fn render_pdf(plan: &PdfPlan<'_>) -> Result<Vec<u8>, PdfRenderError>;
```

The plan should own derived metadata but borrow large immutable layout, image, and font byte buffers. It must not duplicate image payloads or full font payloads.

`PlannedPage` should contain stable resource references for:

- page and content object IDs;
- built-in and supplied font resource indexes;
- image and alpha-mask references;
- opacity/blend-state names where represented inline;
- any future annotations only if permitted by the passive policy.

`PlannedSuppliedFont` should contain the encoded/subset metadata needed by both page resources and font writing, including used glyphs, CIDs, widths, ToUnicode data, and its six indirect object references.

### F.3 Exact object accounting

Object IDs must be assigned in one checked allocator:

```rust
struct ObjectAllocator {
    next: i32,
    count: usize,
}
```

Allocation must:

- use checked conversion from `usize` to `i32`;
- fail with a typed error before overflow;
- check `max_pdf_indirect_objects` while planning, before PDF writer allocation;
- expose the final exact count from the plan;
- remove or reduce `estimate_passive_pdf_object_count` to a compatibility wrapper around planning until it becomes private.

The conversion pipeline becomes:

```rust
let plan = plan_pdf(&layout, prepared_fonts, &limits)?;
// plan creation enforces the object limit
let pdf = render_pdf(&plan)?;
check_output_byte_limit(&pdf, &limits)?;
audit_passive_pdf_bytes(&pdf)?;
```

Page and layout-item limits remain before planning.

### F.4 Error model

Add a typed `PdfRenderError` with at least:

- `TooManyObjects { count, limit }` or map this at the conversion boundary to the existing `ConvertError::TooManyPdfObjects`;
- `ObjectIdOverflow`;
- `InvalidPreparedFontReference`;
- `InvalidImageReference`;
- `UnexpectedUnicodeCmapTemplate`;
- `InternalPlanMismatch` for impossible plan/writer divergence.

Internal mismatch errors must contain safe structural context, not raw untrusted payload bytes.

### F.5 Performance constraints

The plan is not a general layout cache. It must:

- exist only for one conversion;
- reuse centralized iterative layout-fragment traversal;
- encode each supplied text fragment at most once for planned output;
- discover each image XObject once;
- use bounded vectors/maps proportional to pages, layout items, fonts, glyphs, and images;
- avoid adding back-pointers or caches to public layout items;
- preserve deterministic resource and object ordering.

Benchmark `docs/sample.rtf` and representative font/image/table fixtures before and after. The plan is acceptable if it improves or preserves runtime and memory while making the object count exact. It must not be justified by speculative caching alone.

### F.6 Acceptance criteria

- Renderer and object-limit enforcement consume the same `PdfPlan`.
- The planned object count equals the actual highest emitted indirect object count for every PDF test fixture.
- Object limit failure occurs before rendering the byte buffer.
- Supplied text is not redundantly encoded in estimator and renderer passes.
- PDF output remains deterministic.
- Final byte limit and passive audit remain post-render checks.
- All malformed-plan cases return errors rather than panic.
- Existing passive PDF tests and external `lopdf` structural checks pass.

## 12. Workstream G: typed configuration validation

### G.1 Problem

`RtfLimits` exposes configurable fields, but `RtfLimits::validate` is crate-private and returns `&'static str`. Callers discover invalid combinations only when parsing.

### G.2 Error type and public validation

Add:

```rust
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LimitValidationError {
    #[error("{field} must be non-negative, got {value}")]
    Negative {
        field: &'static str,
        value: i64,
    },

    #[error("{field} must be positive, got {value}")]
    NonPositive {
        field: &'static str,
        value: i64,
    },

    #[error("{min_field} ({min}) must not exceed {max_field} ({max})")]
    InvertedRange {
        min_field: &'static str,
        min: i64,
        max_field: &'static str,
        max: i64,
    },
}

impl RtfLimits {
    pub fn validate(&self) -> Result<(), LimitValidationError>;
}

impl RtfParseOptions {
    pub fn validate(&self) -> Result<(), LimitValidationError>;
}
```

Add an options-level error that composes limit and font validation:

```rust
#[derive(Debug, thiserror::Error)]
pub enum ConvertOptionsError {
    #[error(transparent)]
    Limits(#[from] LimitValidationError),
    #[error(transparent)]
    Fonts(#[from] FontProviderError),
}

impl ConvertOptions {
    pub fn validate(&self) -> Result<(), ConvertOptionsError>;
}
```

`ConvertError` may transparently wrap `ConvertOptionsError`, or retain compatible variants and map them at the boundary. Avoid returning a stringly typed reason once the typed error exists.

### G.3 Builder decision

Do not immediately replace the large public `RtfLimits` struct with a builder. First expose typed validation and document units on every field. A builder may be added later if repeated invalid combinations or compatibility pressure justify it.

If fields become private in a future major API, prefer grouped configuration types such as parser, image, document, layout, and PDF limits rather than a builder with dozens of unrelated setters.

### G.4 Acceptance criteria

- Callers can validate limits and conversion options without parsing input.
- Every current validation branch maps to a typed error variant with field/value context.
- Default and browser-safe options validate successfully.
- Parser entry points still reject invalid limits before tokenization.
- Conversion rejects invalid font and limit configuration before parsing.
- Error text remains useful to CLI users and does not expose document payloads.

## 13. Workstream H: documentation and architectural enforcement

### H.1 Architecture document maintenance

On completion of the first implementation phase, either keep this document as the active plan or split stable decisions into `docs/architecture.md`. The stable architecture document should describe:

- supported public API;
- stage ownership and dependency direction;
- normalized model invariants;
- security boundary and audit scope;
- browser/WASM constraints;
- test taxonomy;
- performance and memory-limit policy.

Mark completed workstream items in this plan and record deviations in the decision log rather than silently changing the target.

### H.2 Enforcement tests

Add lightweight policy checks where they provide durable value:

- a source-policy test for forbidden root `pub mod` declarations;
- the existing WASM dependency-policy test;
- compile tests for the supported root API;
- model invariant tests during migration;
- plan-versus-emitted-object-count PDF tests;
- tests proving browser-safe defaults remain stricter than normal defaults for critical resource categories.

Avoid brittle tests that assert exact source line counts or module filenames. Architecture tests should enforce dependency/public-policy outcomes, not formatting.

### H.3 Performance baseline

Create a repeatable benchmark command or ignored test for representative workloads:

- `docs/sample.rtf`;
- text-heavy supplied-font input;
- image-heavy input;
- deeply wrapped static drawings;
- large tables and pagination;
- malformed bounded input for early rejection.

Record:

- wall-clock median over multiple runs;
- output bytes and SHA-256 for deterministic fixtures;
- page and object counts;
- peak memory where the platform can measure it reliably.

Do not make noisy wall-clock thresholds mandatory in shared CI. CI may enforce deterministic hashes/counts, while performance regression analysis runs locally or in a controlled scheduled environment.

## 14. Trackable work breakdown

This table is the implementation backlog. Update the checkbox and add a commit or pull-request reference when an item is completed. Splitting an item is allowed; combining unrelated items is discouraged.

| Status | ID | Deliverable | Depends on | Completion evidence |
| --- | --- | --- | --- | --- |
| [x] | BASE-01 | Add pinned toolchain and package `rust-version` | None | Verified with native and WASM checks on 2026-08-09 |
| [x] | BASE-02 | Add CI format, check, Clippy, test, security, reference, and WASM jobs | BASE-01 | All five hosted-job command sets passed locally on 2026-08-09; hosted execution starts on push |
| [x] | BASE-03 | Record deterministic fixture hashes/counts and benchmark procedure | BASE-02 | Five-fixture baseline, 2,402-test inventory, and repeatable harness recorded on 2026-08-09 |
| [x] | CFG-01 | Add `LimitValidationError` and public `RtfLimits::validate` | BASE-02 | Negative, non-positive, inverted-range, parser propagation, and public API tests passed on 2026-08-09 |
| [x] | CFG-02 | Add `RtfParseOptions::validate` and `ConvertOptions::validate` | CFG-01 | Typed limit/font composition and pre-lexing rejection tests passed on 2026-08-09 |
| [x] | API-01 | Add crate docs and declare the supported root façade | BASE-02 | Doctest, root-only integration test, generated-doc visibility, CI wiring, and WASM checks passed on 2026-08-09 |
| [x] | API-02 | Add bounded `inspect_rtf` font-family projection | CFG-02, API-01 | Primary/alternate/order/deduplication, Unicode, typed configuration/lexical/resource errors, public facade, full regression, Clippy, and WASM gates passed on 2026-08-09 |
| [x] | API-03 | Migrate CLI font discovery to `inspect_rtf` | API-02 | All 35 CLI tests, raw-dependency source policy, parser-error compatibility, native check, Clippy policy, and WASM check passed on 2026-08-09 |
| [x] | DEC-01 | Split integration/unit test files into domain modules without changing assertions | BASE-02 | 2,425 source tests/2,428 harness tests preserved across 45 domain files; inventory guard, full native/security/reference/fuzz, Clippy, WASM, and unchanged sample hash passed on 2026-08-09 |
| [x] | DEC-02 | Extract PDF audit module | BASE-03 | Renderer-independent standalone compile, 18 audit tests, public facade, 1,043 security tests, full regression/Clippy/WASM gates, and unchanged sample hash passed on 2026-08-09 |
| [x] | DEC-03 | Extract parser JPEG, PNG, and DIB modules | BASE-03, DEC-01 | Three independently limit-tested media modules, 57 existing raster tests, full native/security/reference/fuzz/Clippy/WASM gates, and unchanged sample hash passed on 2026-08-09 |
| [x] | DEC-04 | Extract parser EMF and WMF modules | DEC-03 | Explicit active-limit plumbing, 187 EMF/71 WMF tests, full native/security/reference/fuzz/Clippy/WASM gates, and unchanged sample hash passed on 2026-08-09 |
| [x] | DEC-05 | Extract parser formula, formatting, text, and shape helpers | DEC-03 | 97 field/formula, 70 text-encoding, and 30 shape tests plus full native/security/reference/fuzz/Clippy/WASM gates and unchanged sample hash passed on 2026-08-09 |
| [x] | DEC-06 | Extract layout notes, tables, wrapping, metrics, page, and late-field modules | DEC-01 | Eight documented layout modules, all 319 layout tests, full native/security/reference/fuzz/Clippy/WASM gates, and unchanged sample PDF bytes/hash/page/object counts passed on 2026-08-09 |
| [x] | DEC-07 | Extract PDF images, fonts, vector, text, and writer modules | DEC-02 | Six documented PDF modules, all 78 PDF tests, full native/security/reference/fuzz/Clippy/WASM gates, and unchanged sample PDF bytes/hash/page/object counts passed on 2026-08-09 |
| [x] | MODEL-01 | Add transitional model invariant validator | DEC-01 | Parser-boundary validation, typed public advanced-layout errors, five malformed-model integration tests, full native/security/reference/fuzz/Clippy/WASM gates, and unchanged sample PDF passed on 2026-08-09 |
| [x] | MODEL-02 | Replace note/image parallel vectors with aggregate records and typed IDs | MODEL-01, DEC-06 | Footnote, endnote, and inline-image aggregates with five typed index/ID newtypes; no parallel note/image metadata; full native/security/reference/fuzz/Clippy/WASM gates and unchanged sample PDF passed on 2026-08-09 |
| [x] | MODEL-03 | Replace page/section/statistic markers with typed dynamic fields | MODEL-02 | Seven typed dynamic-field variants with atomic late-field resolution and explicit Plane-15 private-use sanitization; no production field sentinel constants/string replacement; full native/security/reference/fuzz/Clippy/WASM gates and unchanged sample PDF passed on 2026-08-09 |
| [x] | MODEL-04 | Replace image, note, bookmark, advance, and table markers with typed inline nodes | MODEL-03 | Eight typed inline variants with typed IDs/advance payloads, atomic list-marker preservation, explicit former-scalar injection coverage, 1,300 library/1,044 security tests, full native/reference/fuzz/Clippy/WASM gates, and unchanged sample PDF passed on 2026-08-09 |
| [x] | MODEL-05 | Replace passive-math marker/style-ID protocol with typed math data | MODEL-04 | `MathGroupId`, consolidated `PassiveMathRun` grouping data, and three typed `MathInline` controls replaced raw style IDs/private-use markers; 33 focused Office Math tests, full native/security/reference/fuzz/Clippy/WASM gates, and unchanged sample PDF passed on 2026-08-09 |
| [x] | MODEL-06 | Remove obsolete marker parsing/sanitizing and privatize model construction | MODEL-05 | `Run` construction is non-exhaustive with disjoint text/inline constructors; obsolete marker sanitizer/statistics shims are removed; a production-source sentinel policy, 1,301 library/1,044 security tests, full native/reference/fuzz/Clippy/WASM gates, and unchanged sample PDF passed on 2026-08-09 |
| [x] | FONT-01 | Make font bytes cheaply shared and add validated constructors/accessors | API-01 | Private `Arc<[u8]>` storage, validated owned/shared constructors, read-only accessors, and pointer-identity tests prove cloned browser-safe options share all 13 bundled font allocations; 1,301 library/1,044 security/46 conversion tests, full native/reference/fuzz/Clippy/WASM gates, unchanged sample PDF, and a 127.887 ms local median passed on 2026-08-09 |
| [x] | FONT-02 | Add `PreparedFontCatalog` and normalized selection indexes | FONT-01 | Conversion prepares validated shallow-cloned assets before lexing, with canonical case/Word-suffix indexes that preserve exact and fallback input order; prepared-only layout/PDF boundaries, failure-precedence and allocation tests, 1,303 library/1,044 security tests, full native/reference/fuzz/Clippy/WASM gates, unchanged sample PDF, and a 134.584 ms local median passed on 2026-08-09 |
| [x] | FONT-03 | Add bounded conversion-local metric/selection caches | FONT-02, DEC-06 | One conversion-local `FontSession` spans diagnostics, layout, PDF estimation, and rendering with provider-derived hard bounds for selection/glyph/kerning caches; saturation tests prove full caches stop inserting without changing results; 1,304 library/1,044 security tests, full native/reference/fuzz/Clippy/WASM gates, unchanged sample PDF, and a 130.418 ms local median passed on 2026-08-09 |
| [x] | FONT-04 | Decide and, if approved, add reusable `Converter` | FONT-03 | Approved after non-regressing FONT-03 benchmarks; public `Converter::new` prepares once and `convert(&self)` creates isolated sessions, while one-shot APIs share the same path; repeated-output, construction-error, root-facade, doctest, and `Send + Sync` assertions plus full native/security/reference/fuzz/Clippy/WASM gates, unchanged sample PDF, and a 129.079 ms local median passed on 2026-08-09 |
| [x] | PDF-01 | Add typed `PdfRenderError` and remove renderer invariant panics | DEC-07 | Public typed errors cover object/reference/template/plan failures; conversion rendering is fallible, the Unicode CMap `expect` is removed, and malformed template/font-reference tests return errors; 1,306 library/1,044 security/80 PDF tests, full native/reference/fuzz/Clippy/WASM gates, unchanged sample PDF, and a 130.504 ms local median passed on 2026-08-09 |
| [x] | PDF-02 | Add conversion-local `PdfPlan` and planned resource records | PDF-01, FONT-02, MODEL-06 | A conversion-local borrowing plan now records stable per-page built-in/supplied-font, opacity, and image metadata while sharing layout and font payloads; compact-record/pointer-identity and malformed-plan tests, 1,307 library/1,044 security/81 PDF tests, full native/reference/fuzz/Clippy/WASM gates, unchanged sample PDF, and a 138.789 ms local median passed on 2026-08-09 |
| [x] | PDF-03 | Add checked contiguous object allocator and exact pre-render limit | PDF-02 | One checked allocator now assigns contiguous catalog/page/content/font/CMap/image/mask references, returns typed overflow/limit errors during planning, and proves planned count/highest ID equal emitted objects at exact limit boundaries; 1,309 library/1,044 security/83 PDF tests, full native/reference/fuzz/Clippy/WASM gates, unchanged sample PDF, and a 145.084 ms local median passed on 2026-08-09 |
| [ ] | PDF-04 | Render exclusively from `PdfPlan`; remove duplicate estimator traversal | PDF-03 | Output limits/audit ordering and deterministic output pass |
| [ ] | API-04 | Migrate white-box tests and add non-default narrow fuzz hooks | MODEL-06, PDF-04 | Normal tests need no raw public modules; fuzz targets compile |
| [ ] | API-05 | Make layout/model/raw parser/raw renderer modules crate-private | API-03, API-04 | Public docs contain only intentional APIs |
| [ ] | DOC-01 | Finalize architecture docs, changelog, and version migration notes | API-05 | Definition of done is fully evidenced |

## 15. Implementation phases and dependency order

### Phase 0: baseline and guardrails

Deliverables:

- CI and toolchain policy from workstream A;
- documented local quality-gate commands;
- refreshed test count and representative output hashes;
- baseline benchmark procedure;
- no production behavior change.

Exit gate:

- CI is green and required before subsequent merges.

### Phase 1: validation and API declaration

Deliverables:

- typed limit/options validation from workstream G;
- crate-level documentation;
- supported-root API list;
- raw modules temporarily `#[doc(hidden)]` with migration notes;
- root re-export of the passive PDF audit;
- focused `inspect_rtf` API and CLI migration.

Exit gate:

- CLI no longer parses raw model data directly;
- supported API doctests and compile tests pass.

### Phase 2: low-risk decomposition

Deliverables:

- split PDF audit, image, and font-writing modules;
- split parser raster/vector decoders and formula helpers;
- split layout notes/text-metrics/table helpers;
- split giant test targets by domain;
- no deliberate output change.

Exit gate:

- fixture PDFs remain byte-identical;
- all 2,400 baseline tests remain represented and passing.

### Phase 3: typed model migration

Deliverables:

- transitional invariant validator;
- typed identifiers and aggregate note/image records;
- typed field, note, bookmark, image, and layout nodes;
- passive-math typed representation;
- removal of marker protocol;
- private model construction.

Exit gate:

- no semantic sentinel constants remain in production code;
- parser and layout communicate only through typed model structures.

### Phase 4: prepared fonts

Deliverables:

- cheap shared font bytes;
- private validated catalog;
- conversion-local bounded metric caches;
- optional reusable `Converter` if justified by tests/benchmarks;
- migration away from public font struct fields.

Exit gate:

- cloning options does not copy font payloads;
- conversion behavior and diagnostics remain stable.

### Phase 5: PDF plan and error-returning renderer

Deliverables:

- exact `PdfPlan`;
- checked object allocator;
- one resource derivation path;
- error-returning PDF rendering;
- object estimator removal or compatibility wrapper;
- plan/emission equality tests.

Exit gate:

- object count is exact and checked before output allocation;
- final byte limit and audit still run after rendering.

### Phase 6: enforce private internals and finalize architecture

Deliverables:

- repository consumers migrated from raw modules;
- raw implementation modules crate-private;
- narrow non-default fuzz hooks only where necessary;
- `missing_docs` enabled for the supported public API;
- stable `docs/architecture.md` or finalized version of this document;
- version/changelog update.

Exit gate:

- only intentional supported APIs appear in generated public documentation;
- all architecture acceptance criteria are satisfied.

## 16. Global acceptance gate

Every phase must pass the checks relevant to its changes. Before declaring the plan complete, run all of the following from a clean checkout:

```powershell
cargo fmt --all -- --check
cargo check --all-targets --all-features
cargo clippy --all-targets --all-features -- -D clippy::correctness -D clippy::suspicious -D clippy::perf
cargo test --lib
cargo test --bin open-rtf-converter
cargo test --test conversion
cargo test --test public_api
cargo test --test test_structure
cargo test --test security
cargo test --test reference_fixtures
cargo test --test fuzz_regression
cargo test --test wasm_dependency_policy
cargo check --no-default-features --target wasm32-unknown-unknown
```

Also run the repository's cargo-fuzz smoke procedure for lexer, parser, and full passive conversion targets. Long fuzz campaigns remain scheduled/manual, but every changed parser/media boundary requires at least a bounded smoke run.

Completion additionally requires:

- no decrease in security policy coverage;
- no new native/WASM-incompatible core dependency;
- no global mutable cache;
- no new direct filesystem or network dependency in the core;
- no unchecked object-ID or allocation-size conversion derived from input;
- no raw untrusted payload in errors or diagnostics;
- no unexplained reference-rendering drift;
- documented migration notes for downstream users.

## 17. Explicit non-goals

This plan does not authorize:

- rewriting the converter in another language or splitting it into services;
- adding a DOM, browser API, server, database, or network layer;
- introducing system font discovery into the core;
- replacing the bounded whole-document pipeline with streaming parsing without a separate design and benchmark;
- changing active-content policy or weakening the final PDF audit;
- broad rendering-fidelity changes during module extraction;
- adding an unbounded persistent resource cache;
- chasing arbitrary source-file line-count limits;
- requiring all Clippy style/pedantic lints before architecture work can proceed;
- claiming arbitrary third-party PDFs are safe merely because the passive-name audit accepts them.

## 18. Risks and mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Structural move changes behavior | Security or fidelity regression | Separate extraction commits; byte/hash/reference checks |
| Public API narrowing breaks CLI/tests/fuzz | Build or coverage loss | Migrate CLI via inspection API; move white-box tests; add narrow non-default fuzz hooks |
| Typed inline model changes line breaking | Pagination drift | Migrate one semantic family at a time; compare layout/PDF outputs |
| Passive math migration loses grouping | Equation fidelity regression | Treat math as its own typed subproject with dedicated fixtures |
| Prepared font catalog adds lifetime complexity | Unsafe or overcomplicated design | Store shared bytes and metadata; recreate borrowed `Face` views; avoid self-references |
| Font caches grow with hostile text | Memory exhaustion | Conversion-local explicit bounds; stop caching at capacity |
| PDF plan duplicates large payloads | Peak-memory regression | Borrow layout/font/image bytes; own only derived metadata |
| CI becomes too slow | Developers bypass checks | Parallel jobs and caching; never omit required security coverage |
| Test splitting loses cases | False confidence | Track baseline count and map moved tests by domain |
| Toolchain pin goes stale | Dependency/security friction | Scheduled explicit toolchain upgrades with full CI |

## 19. Decision log

Record material deviations and decisions here.

### 2026-08-08: evolutionary refactor

Decision: retain the existing pipeline and refactor it incrementally rather than performing a rewrite.

Reason: dependency direction, security policy, WASM constraints, and regression coverage are strong; the main debt is boundaries and representation.

### 2026-08-08: CI before structural work

Decision: repository CI and a pinned toolchain are the first implementation phase.

Reason: the suite is large and security-sensitive, and subsequent work changes high-coupling internals.

### 2026-08-08: typed inline content

Decision: semantic private-use markers will be replaced with typed model nodes.

Reason: the marker protocol is cross-module hidden state and permits invalid construction.

### 2026-08-08: local PDF plan, not layout cache

Decision: any shared PDF resource analysis will live in a conversion-local `PdfPlan`, not in `LayoutDocument` or a persistent global cache.

Reason: exact object accounting is valuable, while a previous broad cache prototype added substantial complexity for only a small retained speed improvement.

### 2026-08-08: prepared fonts without self-referential faces

Decision: prepared fonts will store shared bytes, indexes, and cached results but will not store `ttf_parser::Face` values that borrow their own owning structure.

Reason: this keeps ownership safe and understandable while still allowing validation and selection to be amortized.

## 20. Open decisions

Resolve these during the named workstream, not by assumption:

1. Whether the supported minimum Rust version should remain 1.96 after the first reproducible CI baseline or be lowered through an explicit MSRV test.
2. Whether `Converter` should become a supported reusable API or remain an internal optimization.
3. Whether raw parser access has a genuine downstream product use that warrants a separately versioned advanced API, rather than only fuzz hooks and crate-private tests.
4. Whether the final typed math representation should be an inline tree or a separate math layout subtree.
5. Whether object planning can preserve byte-for-byte PDF identity or requires a documented one-time object-order migration.
6. Whether model/style limit fields should eventually be grouped into sub-structures in a 0.2 or later API.

An open decision does not block earlier independent phases.

## 21. Definition of done

The architectural improvement project is complete when:

- CI and toolchain policy are active and green;
- the supported public façade is documented and enforced;
- the CLI and normal integration tests no longer rely on raw modules;
- model semantics and related metadata are typed and internally consistent;
- parser, layout, PDF, and test responsibilities are decomposed into cohesive modules;
- font bytes are cheaply shared and font preparation is explicit;
- PDF planning and rendering share exact resource assignments;
- options can be validated directly with typed errors;
- full native, security, reference, regression, fuzz-smoke, and WASM gates pass;
- performance and deterministic-output baselines show no unexplained regression;
- stable architectural decisions are reflected in maintained repository documentation.
