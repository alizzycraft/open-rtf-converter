# Web-Safe Rust Library Guide for RTF-to-PDF Core

## Goal

Design the Rust converter core so it can run in:

```text
CLI app
desktop app
server worker
browser WebAssembly/WASM static site
```

The core library should be **pure, deterministic, sandbox-friendly, and browser-compatible**.

---

## Core API shape

The main conversion API should take bytes and return bytes.

```rust
pub fn convert_rtf_to_pdf(
    input: &[u8],
    options: ConvertOptions,
) -> Result<ConversionOutput, ConvertError>
```

```rust
pub struct ConversionOutput {
    pub pdf: Vec<u8>,
    pub warnings: Vec<ConversionWarning>,
}
```

Avoid APIs that require file paths:

```rust
// Avoid in core:
convert_file_to_file(input_path, output_path)
```

File handling belongs in CLI/web/desktop wrappers, not the converter core.

---

## Core rules

The library must not depend on:

```text
filesystem access
network access
environment variables
system fonts
OS-specific APIs
shell commands
threads as a requirement
Word/LibreOffice/Pandoc
C/C++ native libraries
```

The core should work from memory only:

```text
RTF bytes in -> PDF bytes out
```

---

## WASM-friendly requirements

Prefer dependencies that support:

```text
wasm32-unknown-unknown
no native C build step
no OS file APIs
no system font discovery
no sockets/networking
```

Before adding a crate, check whether it works in WASM.

Avoid dependencies that require:

```text
std::fs in core logic
std::process
native image libraries
fontconfig/CoreText/DirectWrite
OpenSSL
system-installed tools
```

---

## Browser-safe conversion policy

Use stricter defaults for browser mode.

```rust
impl ConvertOptions {
    pub fn browser_safe_defaults() -> Self {
        Self {
            compatibility_mode: CompatibilityMode::WordCompatiblePassive,
            active_content_policy: ActiveContentPolicy::Placeholder,
            link_policy: PdfLinkPolicy::RenderVisibleTextOnly,
            limits: RtfLimits::browser_defaults(),
        }
    }
}
```

Browser defaults should:

```text
strip or placeholder OLE
disable external fetching
disable active PDF features
reject huge files
reject excessive nesting
limit images
limit binary blobs
limit output size
```

This matters because malicious RTFs may use parser quirks, binary data, embedded objects, and obfuscation tricks to confuse converters or scanners. 

---

## No persistent storage in core

The core library should not know about:

```text
localStorage
IndexedDB
OPFS
browser downloads
temporary files
```

That belongs in the web wrapper.

The static website can keep everything in memory:

```text
user selects RTF
browser reads ArrayBuffer
WASM converts
browser creates PDF Blob
user downloads PDF
```

No server and no storage required.

---

## Worker-friendly design

Assume the WASM converter will run inside a Web Worker.

Good:

```text
single conversion call
progress callbacks optional
cancel/token support optional
no UI dependencies
no DOM access
```

Avoid:

```text
blocking UI assumptions
browser APIs inside Rust core
global mutable state
shared singleton converter state
```

Optional future API:

```rust
pub fn convert_with_progress(
    input: &[u8],
    options: ConvertOptions,
    progress: impl FnMut(ConversionProgress),
) -> Result<ConversionOutput, ConvertError>
```

---

## Font strategy

Do not rely on system fonts in the core.

Use one of these strategies:

```text
bundled open fonts
caller-provided font bytes
simple fallback font
font substitution warnings
```

Example:

```rust
pub struct FontProvider {
    pub fonts: Vec<FontAsset>,
}
```

The web app can bundle fonts or let the user add them later.

---

## Image strategy

For web-safe MVP:

```text
support PNG/JPEG only
limit image byte size
limit decoded pixel count
placeholder unsupported formats
placeholder WMF/EMF initially
```

Do not use native image decoders that break WASM.

---

## PDF output restrictions

The PDF writer must never emit:

```text
JavaScript
launch actions
embedded files
attachments copied from RTF
remote auto-open actions
raw OLE data
```

Allowed:

```text
text
static layout
static images
basic links only if explicitly sanitized
```

Default link policy:

```rust
PdfLinkPolicy::RenderVisibleTextOnly
```

---

## Recommended crate layout

```text
crates/
  rtf_pdf_core/
    pure Rust, memory-only converter

  rtf_pdf_cli/
    filesystem wrapper

  rtf_pdf_wasm/
    wasm-bindgen wrapper

web/
  static site
  worker.ts
  drag/drop UI
```

Core dependency direction:

```text
cli -> core
wasm -> core
web -> wasm
core -> nothing platform-specific
```