# Rust RTF-to-PDF Converter: Word-Compatible Passive Rendering Security Guide

## 1. Project objective

Build an RTF-to-PDF converter in Rust that aims to render documents similarly to Microsoft Word **when the behavior affects visible output**, while enforcing a strict passive-output security model.

The converter should support Word-compatible interpretation of:

```text
text
fonts
paragraphs
styles
lists
tables
headers/footers
sections
page layout
Unicode/code pages
static images
common Word-generated RTF structures
```

The converter must not support active behavior such as:

```text
OLE activation
OCX controls
macros
scripts
embedded executable packages
external resource fetching
automatic object updates
PDF JavaScript
PDF launch actions
PDF embedded files copied from RTF
```

The output PDF must be passive: text, layout, static shapes, and safe static images only.

---

# 2. Core principle

## Emulate Word’s visual interpretation, not Word’s attack surface.

When Word has a quirk that changes visible rendering, the converter may emulate it.

When Word has a quirk that activates, embeds, fetches, launches, updates, or preserves dangerous content, the converter must strip, placeholder, or reject it.

Use this rule:

```text
If a feature affects passive visual output:
    emulate it with bounds and tests.

If a feature affects active behavior or hidden payload handling:
    parse only enough to skip, remove, placeholder, or reject it.

If continuing safely is uncertain:
    reject the document.
```

---

# 3. Security posture

All RTF input is hostile.

Assume the file may attempt to:

```text
crash the parser
cause stack overflow
cause excessive memory use
cause infinite loops
hide object data
split dangerous control words
abuse binary length parameters
abuse hex decoding state
abuse oversized control words
abuse unknown destinations
abuse Word/parser mismatch
smuggle active content into the output PDF
```

The converter’s response should be:

```text
bounded parsing
typed errors
safe stripping
no active output
no panics
no unsafe parsing
no shell execution
no network access
```

---

# 4. Important architectural distinction

Do **not** build this as:

```text
RTF tokens -> PDF directly
```

Build it as:

```text
RTF bytes
  -> tokenizer
  -> Word-compatible bounded parser
  -> normalized document model
  -> layout model
  -> passive PDF renderer
```

The normalized model is the security boundary.

Raw RTF data, raw `\objdata`, OLE bytes, unknown destination data, and untrusted binary blobs must not cross into the PDF renderer.

Refer to the document 'D:\dev\github\open-rtf-converter\docs\web-safe-rust.md' when making decisions on what rust dependencies to use as a WASM client is one of the desired uses.

---

# 5. Recommended pipeline

```text
1. Input validation
2. Byte-level tokenizer
3. Group-aware parser
4. Destination interpreter
5. Word-compatible normalization
6. Active-content stripping
7. Safe document model generation
8. Layout engine
9. Passive PDF writer
10. Warning/error report
```

The PDF writer should only know about safe concepts:

```rust
enum SafeBlock {
    Paragraph(Vec<SafeInline>),
    Table(SafeTable),
    StaticImage(SafeImageRef),
    PageBreak,
    SectionBreak,
    Placeholder(String),
}

enum SafeInline {
    Text(String),
    StyledText {
        text: String,
        style: TextStyle,
    },
    LineBreak,
    Tab,
}
```

The PDF writer should never see:

```rust
RawRtfToken
ObjData
OleObject
UnknownDestinationBytes
RawBinaryBlob
Script
ExternalLinkTargetFromObject
```

---

# 6. Compatibility modes

Implement explicit compatibility modes, even if only one is used at first.

```rust
pub enum CompatibilityMode {
    StrictSpec,
    WordCompatiblePassive,
}
```

Default for this project:

```rust
CompatibilityMode::WordCompatiblePassive
```

Meaning:

```text
Try to match Word’s passive visual result.
Do not activate Word-like active behavior.
Apply strict resource limits.
Reject unsafe ambiguity.
```

---

# 7. Active content policy

Use a central policy enum.

```rust
pub enum ActiveContentPolicy {
    Strip,
    Placeholder,
    Reject,
}
```

Recommended default:

```rust
ActiveContentPolicy::Placeholder
```

For example, an embedded OLE object becomes:

```text
[Embedded object removed]
```

or, in strict server environments:

```rust
ActiveContentPolicy::Reject
```

The policy should apply to:

```text
OLE objects
OCX controls
embedded packages
linked objects
external images
external templates
scripts
macros
active fields
PDF actions
PDF attachments
```

---

# 8. Rust safety requirements

Use safe Rust by default.

Avoid `unsafe`.

Any `unsafe` must require a written justification and review.

Forbidden in the core parser:

```text
raw pointer parsing
unchecked indexing
manual buffer writes
C/C++ RTF parser bindings
calling Word/LibreOffice/Pandoc from parser code
unbounded allocation from file-provided lengths
panic-based parsing
```

Required:

```text
checked arithmetic
explicit limits
typed errors
fuzzing
no panics on malformed input
no recursive parsing without depth guard
no filesystem writes during parse
no network access
```

---

# 9. Global parsing limits

Create a central limits object.

```rust
pub struct RtfLimits {
    pub max_file_size: usize,
    pub max_group_depth: usize,
    pub max_control_word_len: usize,
    pub max_parameter_digits: usize,
    pub max_text_run_len: usize,
    pub max_binary_blob_size: usize,
    pub max_total_binary_bytes: usize,
    pub max_token_count: usize,
    pub max_destination_bytes: usize,
    pub max_table_cells: usize,
    pub max_styles: usize,
    pub max_fonts: usize,
    pub max_colors: usize,
    pub max_images: usize,
    pub max_image_pixels: usize,
    pub max_output_text_chars: usize,
}
```

Suggested starting defaults:

```rust
RtfLimits {
    max_file_size: 25 * 1024 * 1024,
    max_group_depth: 128,
    max_control_word_len: 64,
    max_parameter_digits: 12,
    max_text_run_len: 1024 * 1024,
    max_binary_blob_size: 5 * 1024 * 1024,
    max_total_binary_bytes: 20 * 1024 * 1024,
    max_token_count: 5_000_000,
    max_destination_bytes: 10 * 1024 * 1024,
    max_table_cells: 100_000,
    max_styles: 10_000,
    max_fonts: 2_000,
    max_colors: 10_000,
    max_images: 500,
    max_image_pixels: 100_000_000,
    max_output_text_chars: 10_000_000,
}
```

These values can be tuned, but the principle should not change: **every attacker-controlled growth path needs a limit.**

---

# 10. Error model

Use structured errors.

```rust
pub enum RtfError {
    FileTooLarge,
    GroupDepthExceeded,
    ControlWordTooLong,
    NumericParameterTooLong,
    NumericParameterOverflow,
    BinaryBlobTooLarge,
    TotalBinaryLimitExceeded,
    TokenLimitExceeded,
    DestinationTooLarge,
    MalformedInput {
        offset: usize,
        reason: String,
    },
    UnsupportedRequiredFeature {
        feature: String,
    },
    ActiveContentRejected {
        feature: String,
    },
    ResourceLimitExceeded {
        resource: String,
    },
}
```

Use warnings for safe degradation.

```rust
pub struct ConversionWarning {
    pub offset: Option<usize>,
    pub kind: WarningKind,
    pub message: String,
}

pub enum WarningKind {
    UnsupportedFeatureStripped,
    ActiveContentRemoved,
    FontSubstituted,
    ImageSkipped,
    LayoutApproximation,
    UnknownDestinationSkipped,
}
```

Never silently preserve risky content.

---

# 11. Tokenizer design

RTF is byte-oriented. Do not convert the whole file to UTF-8 at the start.

Bad:

```rust
let s = String::from_utf8(bytes)?;
```

Good:

```rust
let tokens = RtfTokenizer::new(bytes, limits).tokenize()?;
```

Suggested token types:

```rust
pub enum Token<'a> {
    GroupStart {
        offset: usize,
    },
    GroupEnd {
        offset: usize,
    },
    ControlWord {
        offset: usize,
        name: &'a [u8],
        parameter: Option<ControlParameter>,
        has_delimiter_space: bool,
    },
    ControlSymbol {
        offset: usize,
        symbol: u8,
        parameter: Option<u8>,
    },
    Text {
        offset: usize,
        bytes: &'a [u8],
    },
    Binary {
        offset: usize,
        bytes: &'a [u8],
    },
}
```

Use borrowed slices where possible, but never allow downstream stages to keep raw object data unless explicitly safe.

---

# 12. Control word parsing

RTF control words begin with `\` and are followed by letters, optionally a signed numeric parameter.

Examples:

```rtf
\b
\b0
\fs24
\par
\u8217?
```

Rules:

```text
control word name length must be bounded
parameter digit count must be bounded
parameter parsing must be checked
unknown control words must not panic
unsupported control words must be skipped or recorded
control word effects must be scoped to the current group
```

Important: for Word-compatible passive mode, do not assume the formal spec is enough. The Mandiant article notes that Microsoft Word’s parser may behave differently around oversized control words and numeric parameters. Attackers use those quirks to hide data from simpler tools. 

Recommended policy:

```text
For normal in-limit input:
    emulate Word-compatible behavior where it affects visual output.

For oversized or malformed control words/parameters:
    do not emulate dangerous overflow/truncation quirks by default.
    reject unless a specific, tested compatibility rule is implemented safely.
```

---

# 13. Group parsing

RTF groups are delimited by `{` and `}`.

Use an explicit stack.

```rust
struct GroupFrame {
    destination: Destination,
    character_style: CharacterStyle,
    paragraph_style: ParagraphStyle,
    section_style: SectionStyle,
    skip_mode: SkipMode,
}
```

Rules:

```text
push on {
pop on }
reject unmatched }
reject EOF before all groups close, unless recovery mode explicitly supports it
limit depth
avoid recursive descent unless bounded
group state must be restored on pop
```

For compatibility, Word often recovers from malformed documents. Recovery can be useful, but it must be explicit:

```rust
pub enum MalformedInputPolicy {
    Reject,
    WordLikeRecoverWithinLimits,
}
```

Default for security-sensitive contexts:

```rust
Reject
```

Default for desktop/local batch conversion may be:

```rust
WordLikeRecoverWithinLimits
```

but only after tests.

---

# 14. Destinations

RTF destinations are groups or regions with special meaning: font table, color table, stylesheet, object data, picture data, etc.

Use an explicit destination enum.

```rust
pub enum Destination {
    Document,
    FontTable,
    ColorTable,
    StyleSheet,
    ListTable,
    Header,
    Footer,
    Footnote,
    Annotation,
    Field,
    Picture,
    Object,
    ObjectData,
    Unknown {
        ignorable: bool,
    },
    Skipped,
}
```

Do not let arbitrary destination content flow into document text.

## Destination policy

```text
Known visual destination:
    parse into safe model.

Known active/dangerous destination:
    strip, placeholder, or reject.

Unknown ignorable destination:
    skip whole group.

Unknown non-ignorable destination:
    skip if safe, reject if it contains binary/object/control content that cannot be safely classified.
```

RTF’s `\*` control symbol marks ignorable destinations. Attackers can abuse destination skipping behavior, so implement it carefully.

---

# 15. Word-compatible visual behavior to emulate

These are worth emulating because they affect PDF output.

## Text

```text
plain text
escaped braces
escaped backslashes
paragraph breaks
line breaks
tabs
nonbreaking spaces
hyphens/dashes where supported
```

## Unicode and code pages

Support:

```text
\ansi
\mac
\pc
\pca
\ansicpgN
\uN fallback behavior
\ucN fallback character count
hex escapes \'hh
font charset hints
```

This matters a lot for visual fidelity.

A common Word-compatible behavior:

```rtf
\u8217?
```

means:

```text
insert Unicode codepoint 8217
then skip fallback text length according to \ucN
```

Implement this carefully with bounds.

## Character formatting

```text
bold
italic
underline
strike
superscript
subscript
small caps if feasible
font size
font face
foreground color
background/highlight color
```

## Paragraph formatting

```text
paragraph alignment
indents
first-line indent
spacing before/after
line spacing
tabs
page breaks
keep-with-next if feasible
```

## Page and section layout

```text
paper size
margins
orientation
section breaks
columns later if needed
headers
footers
page numbers as static render
```

## Lists

```text
bullets
numbering
indentation
Word-style list tables
fallback simple list rendering
```

## Tables

Tables are essential for real-world Word fidelity.

Support gradually:

```text
row start/end
cell boundaries
cell widths
borders
shading
merged cells later
nested tables later
```

Tables should have hard limits:

```text
max rows
max cells
max nesting
max width calculations
```

## Static images

Support `\pict` only as passive image rendering.

Allowed:

```text
PNG
JPEG
maybe WMF/EMF only through a hardened converter, later
```

Be very cautious with WMF/EMF because those formats can have complex historical behavior. For MVP, render placeholder for unsupported image formats.

---

# 16. Word-compatible behavior to emulate only defensively

These are needed because they affect parsing boundaries or hidden content, but they are security-sensitive.

## `\binN`

`\binN` says that the next N bytes are raw binary data.

Rules:

```text
N must be non-negative
N must parse without overflow
N must be within max_binary_blob_size
remaining bytes must be at least N
binary bytes must not be tokenized as RTF
total binary bytes must be tracked
```

Safe parser:

```rust
fn parse_bin_length(param: i32, limits: &RtfLimits) -> Result<usize, RtfError> {
    if param < 0 {
        return Err(RtfError::MalformedInput {
            offset: 0,
            reason: "negative \\bin length".into(),
        });
    }

    let len = usize::try_from(param).map_err(|_| RtfError::NumericParameterOverflow)?;

    if len > limits.max_binary_blob_size {
        return Err(RtfError::BinaryBlobTooLarge);
    }

    Ok(len)
}
```

Do emulate the boundary effect of `\binN`: if `\bin3 abc` appears, those three bytes are binary payload, not text/control syntax.

Do not trust `\binN` content as safe.

## Hex escapes

RTF supports:

```rtf
\'hh
```

Rules:

```text
must have exactly two hex digits
decode according to current code page in text context
inside object/picture data, behavior must be destination-specific
malformed escapes should reject or replacement-character according to policy
```

The article discusses how escaped hex can affect parser state in object data. That is a warning: test these cases thoroughly. 

## Ignored control words

Some control words do not produce visible text. Word may consume the control word but leave following data.

Implement known behavior where it affects visible output, but never let ignored controls become a way to smuggle active content.

## Unknown destinations and `\*`

Implement Word-compatible destination skipping, but bounded.

```text
{\*\unknown ...}
```

should usually be skipped.

But if unknown content is huge or contains binary/object-like content, still enforce limits.

---

# 17. Active content handling

## OLE and embedded objects

The Mandiant article emphasizes that malicious RTFs often use embedded objects, including OLE and object data, and that attackers use obfuscation around `\objdata` to bypass simple detection. 

For this project:

```text
Do not activate OLE.
Do not render OCX controls.
Do not extract embedded packages.
Do not copy embedded objects into the PDF.
Do not preserve OLE as PDF attachments.
```

Object-related controls:

```text
\object
\objdata
\objocx
\objemb
\objlink
\objautlink
\objupdate
\result
```

Policy:

```text
\object:
    parse enough to find boundaries
    render \result if it contains safe passive fallback content
    strip actual object data
    otherwise placeholder

\objdata:
    consume safely with limits
    do not decode into executable/object
    do not pass to PDF renderer

\objocx:
    reject or placeholder

\objemb:
    placeholder or strip

\objlink / \objautlink:
    do not fetch/update
    render static \result if safe
```

Important Word-compatible rule:

Many RTF objects contain a `\result` group with fallback visual representation. If safe, rendering the `\result` group may improve fidelity without activating the object.

Policy:

```text
For embedded objects:
    prefer safe static \result rendering.
    if no safe result exists:
        placeholder.
```

---

# 18. Fields

RTF fields can represent things like dates, page numbers, hyperlinks, references, and sometimes active-ish behavior.

Controls:

```text
\field
\fldinst
\fldrslt
```

Policy:

```text
Render \fldrslt when available and safe.
Do not execute \fldinst.
Do not fetch links.
Do not update fields dynamically unless implemented as passive built-ins.
```

Examples:

```text
HYPERLINK:
    render visible text from \fldrslt
    optionally preserve sanitized URL only if link policy allows
    default: render plain text, no clickable link

DATE/TIME:
    do not update to current time by default
    render existing \fldrslt

PAGE:
    may render actual page number if layout engine supports it
    otherwise render \fldrslt
```

This gives Word-like visible output without executing field instructions.

---

# 19. External resources

Never fetch anything during conversion.

Reject, strip, or render placeholder for:

```text
external linked images
linked OLE objects
external templates
INCLUDEPICTURE fields
remote URLs in active contexts
```

A converter should be deterministic and offline.

---

# 20. Image handling

For `\pict`, support only safe static image paths.

RTF image data may be hexadecimal or binary.

Rules:

```text
enforce max image count
enforce max image data size
decode only after full bounded extraction
use hardened image decoder
enforce max pixel count
do not allow decompression bombs
do not allow external image fetching
unsupported formats become placeholder
```

Recommended initial support:

```text
PNG
JPEG
```

Later optional support:

```text
BMP with limits
WMF/EMF only through a carefully sandboxed renderer
```

For MVP, do not support WMF/EMF unless absolutely required.

---

# 21. PDF output restrictions

Generated PDF must be passive.

Forbidden:

```text
JavaScript
Launch actions
embedded files
file attachments
rich media
auto-submit forms
remote GoTo actions
automatic URI opening
copied OLE data
copied raw RTF binary blobs
```

Allowed:

```text
text
fonts
paths
static images
static annotations only if explicitly sanitized
```

Default links policy:

```rust
pub enum PdfLinkPolicy {
    DisableAll,
    RenderVisibleTextOnly,
    AllowSanitizedHttpLinks,
}
```

Recommended default:

```rust
PdfLinkPolicy::RenderVisibleTextOnly
```

---

# 22. Normalized document model as security boundary

The parser should produce a safe model.

Example:

```rust
pub struct Document {
    pub sections: Vec<Section>,
    pub warnings: Vec<ConversionWarning>,
}

pub struct Section {
    pub page: PageSettings,
    pub blocks: Vec<Block>,
    pub header: Option<Vec<Block>>,
    pub footer: Option<Vec<Block>>,
}

pub enum Block {
    Paragraph(Paragraph),
    Table(Table),
    Image(ImageRef),
    Placeholder(String),
}

pub struct Paragraph {
    pub style: ParagraphStyle,
    pub runs: Vec<Inline>,
}

pub enum Inline {
    Text {
        text: String,
        style: CharacterStyle,
    },
    Tab,
    LineBreak,
}
```

Do not include:

```rust
Inline::RawRtf(...)
Block::OleObject(...)
Block::UnknownBinary(...)
Block::PdfAction(...)
```

---

# 23. Layout fidelity strategy

Replacing Word is hard because Word’s layout engine is complex.

Use an incremental compatibility approach:

## Phase 1

```text
plain text
paragraphs
basic inline styles
page size/margins
basic fonts
```

## Phase 2

```text
headers/footers
lists
basic tables
static images
```

## Phase 3

```text
better table layout
style sheets
sections
page numbering
footnotes
```

## Phase 4

```text
shapes
text boxes
advanced borders
complex pagination
```

Do not let layout ambition weaken parser security.

---

# 24. Word reference testing

To replace Word-based conversion, use Word as a **development reference**, not as a production dependency.

Create fixtures:

```text
fixtures/
  simple/
    input.rtf
    word.pdf
    word-page-1.png
  tables/
    input.rtf
    word.pdf
    word-page-1.png
  objects/
    input.rtf
    word.pdf
    expected-policy.json
```

Compare:

```text
page count
extracted text
font/style runs where feasible
image positions
table cell positions
visual snapshot diff
warnings emitted
active content removed
```

Use categories:

```text
must match closely
acceptable approximation
intentional security difference
unsupported
```

Security-sensitive fixtures should explicitly document expected differences from Word.

Example:

```text
Input: RTF with embedded OLE object
Word: may expose embedded object or render result
Our converter: renders safe \result or placeholder; strips object
Status: intentional security difference
```

---

# 25. Security tests based on the article

Add tests for the article’s themes.

## Split or obfuscated control content

Purpose:

```text
Ensure parser does not rely on raw substring matching.
Ensure parser produces deterministic normalized output.
Ensure dangerous content hidden by syntax does not reach PDF.
```

## `pFragments` / CVE-style shape properties

Even if shapes are unsupported, parser should not crash.

Expected:

```text
bounded parse
unsupported shape stripped or placeholder
no panic
no unsafe allocation
```

## Embedded object obfuscation

Test:

```rtf
{\rtf1{\object\objocx\objdata 414243}}
```

Expected:

```text
object removed or placeholder
objdata not in PDF
warning emitted
```

## `\binN`

Test:

```rtf
{\rtf1 \bin3 abc}
{\rtf1 \bin-1 abc}
{\rtf1 \bin999999999999 abc}
{\rtf1 \bin5 abc}
```

Expected:

```text
valid bounded binary consumed
negative rejected
overflow rejected
short data rejected
```

## Hex escapes

Test:

```rtf
{\rtf1 \'41\'42\'43}
{\rtf1 \'}
{\rtf1 \'GZ}
{\rtf1 \'1}
```

Expected:

```text
valid decodes
malformed escapes handled according to policy
no panic
```

## Unknown destinations

Test:

```rtf
{\rtf1{\*\unknown ignored} visible}
{\rtf1{\*\unknown{\object\objdata 414243}} visible}
```

Expected:

```text
unknown skipped
object data not emitted
visible text preserved
```

## Oversized control words

Test:

```text
control word length at limit
control word length over limit
parameter digits at limit
parameter digits over limit
```

Expected:

```text
in limit accepted
over limit rejected
```

## Deep nesting

Expected:

```text
max depth accepted
max depth + 1 rejected
no stack overflow
```

---

# 26. Fuzzing requirements

Use fuzzing early.

Targets:

```text
tokenizer
control word parser
group parser
destination parser
unicode decoder
hex decoder
binary reader
full RTF-to-safe-model pipeline
```

Fuzz properties:

```text
no panics
no stack overflow
no infinite loop
no unbounded memory growth
no raw object data in safe model
no active PDF features in output
all errors are typed
parser always advances or terminates
```

Seed corpus:

```rtf
{\rtf1 hello}
{\rtf1{\b bold}}
{\rtf1 \u8217?}
{\rtf1{\fonttbl{\f0 Arial;}}Hello}
{\rtf1{\colortbl;\red255\green0\blue0;}Hello}
{\rtf1{\*\unknown ignored}visible}
{\rtf1{\object{\objdata 414243}}}
{\rtf1{\object{\result visible fallback}}}
{\rtf1{\field{\*\fldinst HYPERLINK "https://example.com"}{\fldrslt visible}}}
{\rtf1\bin3 abc}
{\rtf1\'41\'42\'43}
```

Hostile seeds:

```rtf
{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{{
{\rtf1\bin999999999999999999999 abc}
{\rtf1\bin-1 abc}
{\rtf1{\object{\objdata 414243}}}
{\rtf1{\*\unknown{\object\objdata 414243}}}
{\rtf1\aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa text}
{\rtf1\'GZ}
```

---

# 27. Dependency policy

Before adding a crate, check:

```text
does it parse untrusted input?
does it use unsafe?
does it bind to C/C++?
is it maintained?
does it fuzz?
does it allocate based on untrusted dimensions?
can it fetch network resources?
can it execute commands?
```

Avoid:

```text
generic document conversion wrappers
office automation
shelling out from library code
unmaintained binary parsers
image decoders without size limits
```

For image decoding, wrap the decoder behind your own limits.

---

# 28. Sandboxing recommendation

Even with Rust, run conversion in an isolated worker if the tool processes untrusted files.

For CLI:

```text
support --timeout
support --max-memory where platform allows
write temp files only under controlled temp dir
no network
clear temp files
```

For server:

```text
never parse in web request process
queue conversion jobs
run worker with low privileges
disable network
limit CPU time
limit memory
limit input size before parse
store files in quarantine-like location
delete temp files
rate-limit uploads
```

---

# 29. Suggested module layout

```text
src/
  lib.rs
  config.rs
  limits.rs
  error.rs
  warnings.rs

  rtf/
    mod.rs
    tokenizer.rs
    token.rs
    parser.rs
    group_stack.rs
    controls.rs
    destinations.rs
    unicode.rs
    codepage.rs
    binary.rs
    hex.rs
    object_policy.rs
    picture.rs
    field.rs
    stylesheet.rs
    fonts.rs
    colors.rs
    tables.rs
    lists.rs
    normalize.rs

  document/
    mod.rs
    model.rs
    style.rs
    section.rs
    table.rs
    image.rs

  layout/
    mod.rs
    page.rs
    paragraph.rs
    line_break.rs
    table_layout.rs

  pdf/
    mod.rs
    writer.rs
    fonts.rs
    images.rs
    safety.rs

  security/
    mod.rs
    active_content.rs
    limits.rs
    audit.rs
```

Hard boundary:

```text
rtf/* may know about RTF.
document/* must not contain raw RTF.
layout/* must not contain raw RTF or object data.
pdf/* must not contain active features.
```

---

# 30. Coding agent implementation workflow

For each feature:

```text
1. Define whether it is visual, active, or both.
2. Define Word-compatible behavior required for visual output.
3. Define security policy for active behavior.
4. Add limits if feature can grow.
5. Add normal tests.
6. Add malformed tests.
7. Add fuzz seed.
8. Implement parser support.
9. Normalize into safe model.
10. Ensure PDF renderer receives only safe data.
11. Add warnings for stripped/approximated behavior.
12. Compare against Word reference fixture if applicable.
```

Do not implement broad support without tests.

Do not preserve unsupported content “for later.”

---

# 31. Feature classification table

| Feature              |         Visual? |        Risk | Policy                                       |
| -------------------- | --------------: | ----------: | -------------------------------------------- |
| Plain text           |             Yes |         Low | Render                                       |
| Unicode escapes      |             Yes |      Medium | Emulate with bounds                          |
| Fonts                |             Yes |  Low/Medium | Render with fallback                         |
| Colors               |             Yes |         Low | Render with limits                           |
| Paragraph formatting |             Yes |         Low | Render                                       |
| Tables               |             Yes |      Medium | Render with limits                           |
| Headers/footers      |             Yes |  Low/Medium | Render                                       |
| Static images        |             Yes | Medium/High | Decode safely with limits                    |
| WMF/EMF              |             Yes |        High | Placeholder unless sandboxed                 |
| OLE object result    |             Yes |      Medium | Render safe `\result` only                   |
| OLE object data      |       No/Hidden |        High | Strip/reject                                 |
| OCX controls         |       No/Active |        High | Strip/reject                                 |
| Embedded packages    |       No/Active |        High | Strip/reject                                 |
| Linked objects       |           Maybe |        High | Do not fetch; render safe result             |
| Fields               |       Sometimes |      Medium | Render `\fldrslt`; do not execute `\fldinst` |
| Hyperlinks           | Visual text yes |      Medium | Render text; links disabled by default       |
| External images      |           Maybe |        High | Do not fetch                                 |
| Macros/scripts       |       No/Active |        High | Reject/strip                                 |
| PDF JavaScript       |       No/Active |        High | Never emit                                   |
| PDF attachments      |       No/Hidden |        High | Never copy from RTF                          |

---

# 32. Specific exploit-guard rules inspired by the article

The article’s examples point to a few exact defensive rules.

## Do not use static string detection as parser logic

Bad:

```text
if bytes contain "\objdata", then object exists
```

Good:

```text
tokenize groups/control words
track destination state
normalize according to parser rules
apply object policy
```

Attackers can split, encode, or interrupt meaningful structures. 

## Treat `\objdata` as toxic unless converted into safe visual fallback

Never allow object data to become:

```text
PDF attachment
PDF embedded file
temporary executable
raw output stream
debug dump by default
```

## Treat `\binN` as a parser boundary, not normal text

The bytes after `\binN` must be consumed exactly as bounded binary data.

## Treat malformed numeric parameters as hostile

No wrapping, no truncation, no unchecked casts.

## Treat oversized control words as hostile

Do not reproduce unsafe Word implementation limits unless deliberately modeled and proven safe.

## Treat ignored destinations as still resource-consuming

Even skipped content must be bounded.

---

# 33. Minimum acceptance criteria

The converter is not acceptable until:

```text
malformed input cannot panic
deep nesting cannot overflow stack
binary lengths cannot cause unbounded allocation
object data cannot reach PDF output
OLE/OCX cannot activate
external resources cannot be fetched
PDF output has no active features
unsupported active content is stripped/placeholder/rejected
fuzz tests exist for tokenizer/parser
Word reference tests exist for visual fixtures
warnings document intentional security differences
```

---

# 34. Product wording

Use this as the public/internal design statement:

> This converter aims for Microsoft Word-compatible passive rendering of RTF documents. It emulates Word behavior where needed for visible output, but deliberately removes or disables active content such as OLE, linked objects, scripts, external fetching, and embedded executable payloads. The generated PDF is passive and does not preserve active document behavior.

That is much more accurate than:

```text
“fully Word-compatible”
```

or:

```text
“strict RTF renderer”
```

---
\