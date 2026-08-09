//! Renderer-independent enforcement of the converter's passive-PDF policy.
//!
//! This module inspects serialized PDF syntax only. It deliberately has no
//! dependency on layout or rendering so callers can audit bytes in isolation.

use std::error::Error;
use std::fmt;

const ACTIVE_PDF_NAME_TOKENS: &[(&[u8], &str)] = &[
    (b"/3D", "/3D"),
    (b"/A", "/A"),
    (b"/AA", "/AA"),
    (b"/AF", "/AF"),
    (b"/AFRelationship", "/AFRelationship"),
    (b"/AcroForm", "/AcroForm"),
    (b"/Action", "/Action"),
    (b"/Annot", "/Annot"),
    (b"/Annots", "/Annots"),
    (b"/Cert", "/Cert"),
    (b"/Collection", "/Collection"),
    (b"/DocMDP", "/DocMDP"),
    (b"/DSS", "/DSS"),
    (b"/EF", "/EF"),
    (b"/EmbeddedFile", "/EmbeddedFile"),
    (b"/EmbeddedFiles", "/EmbeddedFiles"),
    (b"/Encrypt", "/Encrypt"),
    (b"/FileAttachment", "/FileAttachment"),
    (b"/FDF", "/FDF"),
    (b"/FieldMDP", "/FieldMDP"),
    (b"/Filespec", "/Filespec"),
    (b"/GoTo", "/GoTo"),
    (b"/GoTo3DView", "/GoTo3DView"),
    (b"/GoToE", "/GoToE"),
    (b"/GoToR", "/GoToR"),
    (b"/Hide", "/Hide"),
    (b"/ImportData", "/ImportData"),
    (b"/JavaScript", "/JavaScript"),
    (b"/JS", "/JS"),
    (b"/Launch", "/Launch"),
    (b"/Link", "/Link"),
    (b"/Movie", "/Movie"),
    (b"/Names", "/Names"),
    (b"/Named", "/Named"),
    (b"/NeedAppearances", "/NeedAppearances"),
    (b"/Next", "/Next"),
    (b"/ObjStm", "/ObjStm"),
    (b"/OpenAction", "/OpenAction"),
    (b"/Perms", "/Perms"),
    (b"/Rendition", "/Rendition"),
    (b"/ResetForm", "/ResetForm"),
    (b"/RichMedia", "/RichMedia"),
    (b"/Screen", "/Screen"),
    (b"/SetOCGState", "/SetOCGState"),
    (b"/Sig", "/Sig"),
    (b"/SigFlags", "/SigFlags"),
    (b"/Sound", "/Sound"),
    (b"/SubmitForm", "/SubmitForm"),
    (b"/Trans", "/Trans"),
    (b"/URI", "/URI"),
    (b"/UR3", "/UR3"),
    (b"/UseAttachments", "/UseAttachments"),
    (b"/VRI", "/VRI"),
    (b"/Widget", "/Widget"),
    (b"/XDP", "/XDP"),
    (b"/XRef", "/XRef"),
    (b"/XFA", "/XFA"),
];
const MAX_AUDITED_PDF_NAME_BYTES: usize = 128;
const OVERLONG_PDF_NAME_TOKEN: &str = "<overlong PDF name>";
const MALFORMED_PDF_NAME_ESCAPE_TOKEN: &str = "<malformed PDF name escape>";
const MISSING_STREAM_LENGTH_TOKEN: &str = "<missing direct stream length>";
const DUPLICATE_STREAM_LENGTH_TOKEN: &str = "<duplicate direct stream length>";

#[derive(Debug, Clone, PartialEq, Eq)]
/// One forbidden or malformed PDF name found by the passive-output audit.
pub struct PassivePdfIssue {
    /// Bounded canonical token label; never an arbitrary payload excerpt.
    pub token: &'static str,
    /// Byte offset of the token in the audited PDF.
    pub offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Failure returned when a PDF violates the converter's passive-output policy.
pub struct PassivePdfError {
    /// All bounded issues found during the audit.
    pub issues: Vec<PassivePdfIssue>,
}

impl fmt::Display for PassivePdfError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(issue) = self.issues.first() {
            write!(
                formatter,
                "passive PDF audit found forbidden token {} at byte {}",
                issue.token, issue.offset
            )
        } else {
            formatter.write_str("passive PDF audit failed")
        }
    }
}

impl Error for PassivePdfError {}

/// Checks PDF syntax emitted by this converter for forbidden active-feature
/// names and malformed stream boundaries.
///
/// This is a defense-in-depth check for the converter's passive-output policy,
/// not a general malware or safety verdict for arbitrary third-party PDFs.
pub fn audit_passive_pdf_bytes(pdf: &[u8]) -> Result<(), PassivePdfError> {
    let mut issues = Vec::new();
    let mut offset = 0;

    while offset < pdf.len() {
        if is_stream_marker_at(pdf, offset) {
            let Some(stream_start) = stream_data_start(pdf, offset) else {
                issues.push(PassivePdfIssue {
                    token: "<malformed stream>",
                    offset,
                });
                break;
            };
            let stream_end = match stream_end_from_declared_length(pdf, offset, stream_start) {
                Some(DeclaredStreamEnd::Found(stream_end)) => stream_end,
                Some(DeclaredStreamEnd::Malformed) => {
                    issues.push(PassivePdfIssue {
                        token: "<malformed stream length>",
                        offset,
                    });
                    break;
                }
                Some(DeclaredStreamEnd::DuplicateLength) => {
                    issues.push(PassivePdfIssue {
                        token: DUPLICATE_STREAM_LENGTH_TOKEN,
                        offset,
                    });
                    break;
                }
                None => {
                    issues.push(PassivePdfIssue {
                        token: MISSING_STREAM_LENGTH_TOKEN,
                        offset,
                    });
                    break;
                }
            };
            offset = stream_end + b"endstream".len();
            continue;
        }

        if pdf[offset] == b'/' {
            if let Some(parsed_name) = parse_pdf_name_token(pdf, offset) {
                if parsed_name.truncated {
                    issues.push(PassivePdfIssue {
                        token: OVERLONG_PDF_NAME_TOKEN,
                        offset,
                    });
                }
                if parsed_name.malformed_escape {
                    issues.push(PassivePdfIssue {
                        token: MALFORMED_PDF_NAME_ESCAPE_TOKEN,
                        offset,
                    });
                }
                for (token, label) in ACTIVE_PDF_NAME_TOKENS {
                    if parsed_name.name == *token {
                        issues.push(PassivePdfIssue {
                            token: label,
                            offset,
                        });
                    }
                }
                offset = parsed_name.end;
                continue;
            } else {
                for (token, label) in ACTIVE_PDF_NAME_TOKENS {
                    if pdf[offset..].starts_with(token)
                        && is_pdf_name_boundary(pdf.get(offset + token.len()).copied())
                    {
                        issues.push(PassivePdfIssue {
                            token: label,
                            offset,
                        });
                    }
                }
            }
        }

        offset += 1;
    }

    if issues.is_empty() {
        Ok(())
    } else {
        Err(PassivePdfError { issues })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedPdfName {
    name: Vec<u8>,
    end: usize,
    truncated: bool,
    malformed_escape: bool,
}

fn parse_pdf_name_token(pdf: &[u8], offset: usize) -> Option<ParsedPdfName> {
    if pdf.get(offset).copied()? != b'/' {
        return None;
    }

    let mut name = Vec::new();
    name.push(b'/');
    let mut truncated = false;
    let mut malformed_escape = false;
    let mut pos = offset + 1;
    while pos < pdf.len() && !is_pdf_name_boundary(Some(pdf[pos])) {
        if pdf[pos] == b'#' {
            if pos + 2 < pdf.len()
                && let (Some(high), Some(low)) = (hex_value(pdf[pos + 1]), hex_value(pdf[pos + 2]))
            {
                truncated |= push_audited_pdf_name_byte(&mut name, (high << 4) | low);
                pos += 3;
                continue;
            }
            malformed_escape = true;
        }

        truncated |= push_audited_pdf_name_byte(&mut name, pdf[pos]);
        pos += 1;
    }

    Some(ParsedPdfName {
        name,
        end: pos,
        truncated,
        malformed_escape,
    })
}

fn push_audited_pdf_name_byte(name: &mut Vec<u8>, byte: u8) -> bool {
    if name.len() < MAX_AUDITED_PDF_NAME_BYTES {
        name.push(byte);
        false
    } else {
        true
    }
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn is_stream_marker_at(pdf: &[u8], offset: usize) -> bool {
    pdf[offset..].starts_with(b"stream")
        && offset
            .checked_sub(1)
            .and_then(|idx| pdf.get(idx))
            .is_some_and(|byte| is_pdf_whitespace(*byte))
        && stream_marker_follows_dictionary(pdf, offset)
        && pdf
            .get(offset + b"stream".len())
            .is_some_and(|byte| matches!(*byte, b'\r' | b'\n'))
}

fn stream_marker_follows_dictionary(pdf: &[u8], stream_offset: usize) -> bool {
    let Some(first_trailing_whitespace) = stream_offset.checked_sub(1) else {
        return false;
    };
    let mut cursor = first_trailing_whitespace;
    while let Some(byte) = pdf.get(cursor).copied() {
        if !is_pdf_whitespace(byte) {
            break;
        }
        let Some(previous) = cursor.checked_sub(1) else {
            return false;
        };
        cursor = previous;
    }

    pdf.get(cursor).copied() == Some(b'>')
        && cursor.checked_sub(1).and_then(|idx| pdf.get(idx)).copied() == Some(b'>')
}

fn stream_data_start(pdf: &[u8], stream_offset: usize) -> Option<usize> {
    let after_marker = stream_offset.checked_add(b"stream".len())?;
    match pdf.get(after_marker).copied()? {
        b'\r' if pdf.get(after_marker + 1).copied() == Some(b'\n') => Some(after_marker + 2),
        b'\r' | b'\n' => Some(after_marker + 1),
        _ => None,
    }
}

enum DeclaredStreamEnd {
    Found(usize),
    Malformed,
    DuplicateLength,
}

fn stream_end_from_declared_length(
    pdf: &[u8],
    stream_offset: usize,
    stream_start: usize,
) -> Option<DeclaredStreamEnd> {
    let (dictionary_start, dictionary_end) = stream_dictionary_bounds(pdf, stream_offset)?;
    let declared_length = match pdf_dictionary_direct_length(pdf, dictionary_start, dictionary_end)?
    {
        PdfStreamLength::Direct(length) => length,
        PdfStreamLength::Duplicate => return Some(DeclaredStreamEnd::DuplicateLength),
    };
    let Some(stream_data_end) = stream_start.checked_add(declared_length) else {
        return Some(DeclaredStreamEnd::Malformed);
    };
    let Some(endstream_offset) = stream_end_marker_after_declared_data(pdf, stream_data_end) else {
        return Some(DeclaredStreamEnd::Malformed);
    };
    Some(DeclaredStreamEnd::Found(endstream_offset))
}

fn stream_dictionary_bounds(pdf: &[u8], stream_offset: usize) -> Option<(usize, usize)> {
    let mut cursor = stream_offset.checked_sub(1)?;
    while is_pdf_whitespace(*pdf.get(cursor)?) {
        cursor = cursor.checked_sub(1)?;
    }
    if pdf.get(cursor).copied() != Some(b'>')
        || cursor.checked_sub(1).and_then(|idx| pdf.get(idx)).copied() != Some(b'>')
    {
        return None;
    }
    let dictionary_end = cursor + 1;
    let mut offset = pdf_object_scan_start_before_stream(pdf, stream_offset);
    let mut depth = 0usize;
    let mut dictionary_start = None;
    while offset < dictionary_end {
        match pdf[offset] {
            b'%' => {
                offset = skip_pdf_comment(pdf, offset, dictionary_end);
                continue;
            }
            b'(' => {
                offset = skip_pdf_literal_string(pdf, offset, dictionary_end);
                continue;
            }
            b'<' if pdf.get(offset + 1).copied() == Some(b'<') => {
                if depth == 0 {
                    dictionary_start = Some(offset);
                }
                depth = depth.checked_add(1)?;
                offset += 2;
                continue;
            }
            b'<' => {
                offset = skip_pdf_hex_string(pdf, offset, dictionary_end);
                continue;
            }
            b'>' if pdf.get(offset + 1).copied() == Some(b'>') => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                let close_end = offset + 2;
                if depth == 0 {
                    if close_end == dictionary_end {
                        return dictionary_start.map(|start| (start, dictionary_end));
                    }
                    dictionary_start = None;
                }
                offset = close_end;
                continue;
            }
            _ => {}
        }
        offset += 1;
    }
    None
}

fn pdf_object_scan_start_before_stream(pdf: &[u8], stream_offset: usize) -> usize {
    let mut cursor = stream_offset.saturating_sub(b"obj".len());
    while cursor > 0 {
        if pdf
            .get(cursor..cursor + b"obj".len())
            .is_some_and(|candidate| candidate == b"obj")
            && cursor
                .checked_sub(1)
                .and_then(|idx| pdf.get(idx))
                .copied()
                .is_some_and(is_pdf_whitespace)
            && pdf
                .get(cursor + b"obj".len())
                .copied()
                .is_some_and(is_pdf_whitespace)
        {
            return cursor + b"obj".len();
        }
        cursor -= 1;
    }
    0
}

fn skip_pdf_comment(pdf: &[u8], mut offset: usize, limit: usize) -> usize {
    offset += 1;
    while offset < limit {
        match pdf[offset] {
            b'\r' => {
                offset += 1;
                if offset < limit && pdf[offset] == b'\n' {
                    offset += 1;
                }
                return offset;
            }
            b'\n' => return offset + 1,
            _ => offset += 1,
        }
    }
    offset
}

fn skip_pdf_literal_string(pdf: &[u8], mut offset: usize, limit: usize) -> usize {
    offset += 1;
    let mut depth = 1usize;
    while offset < limit {
        match pdf[offset] {
            b'\\' => {
                offset = offset.saturating_add(2).min(limit);
            }
            b'(' => {
                depth = depth.saturating_add(1);
                offset += 1;
            }
            b')' => {
                depth = depth.saturating_sub(1);
                offset += 1;
                if depth == 0 {
                    return offset;
                }
            }
            _ => offset += 1,
        }
    }
    offset
}

fn skip_pdf_hex_string(pdf: &[u8], mut offset: usize, limit: usize) -> usize {
    offset += 1;
    while offset < limit {
        if pdf[offset] == b'>' {
            return offset + 1;
        }
        offset += 1;
    }
    offset
}

fn skip_pdf_whitespace_and_comments(pdf: &[u8], mut offset: usize, limit: usize) -> usize {
    loop {
        while offset < limit && pdf.get(offset).copied().is_some_and(is_pdf_whitespace) {
            offset += 1;
        }
        if offset < limit && pdf[offset] == b'%' {
            offset = skip_pdf_comment(pdf, offset, limit);
            continue;
        }
        return offset;
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum PdfStreamLength {
    Direct(usize),
    Duplicate,
}

fn pdf_dictionary_direct_length(
    pdf: &[u8],
    dictionary_start: usize,
    dictionary_end: usize,
) -> Option<PdfStreamLength> {
    let mut offset = dictionary_start.checked_add(2)?;
    let mut depth = 0usize;
    let mut array_depth = 0usize;
    let mut length = None;
    while offset < dictionary_end {
        match pdf[offset] {
            b'%' => {
                offset = skip_pdf_comment(pdf, offset, dictionary_end);
                continue;
            }
            b'(' => {
                offset = skip_pdf_literal_string(pdf, offset, dictionary_end);
                continue;
            }
            b'<' if pdf.get(offset + 1).copied() == Some(b'<') => {
                depth = depth.checked_add(1)?;
                offset += 2;
                continue;
            }
            b'<' => {
                offset = skip_pdf_hex_string(pdf, offset, dictionary_end);
                continue;
            }
            b'>' if pdf.get(offset + 1).copied() == Some(b'>') => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                offset += 2;
                continue;
            }
            b'[' if depth == 0 => {
                array_depth = array_depth.checked_add(1)?;
                offset += 1;
                continue;
            }
            b']' if depth == 0 && array_depth > 0 => {
                array_depth -= 1;
                offset += 1;
                continue;
            }
            _ => {}
        }
        if depth == 0
            && array_depth == 0
            && pdf[offset] == b'/'
            && let Some(parsed_name) = parse_pdf_name_token(pdf, offset)
        {
            if parsed_name.name == b"/Length" {
                let parsed_length =
                    parse_pdf_direct_unsigned_integer(pdf, parsed_name.end, dictionary_end)?;
                if length.replace(parsed_length).is_some() {
                    return Some(PdfStreamLength::Duplicate);
                }
            }
            offset = parsed_name.end;
            continue;
        }
        offset += 1;
    }
    length.map(PdfStreamLength::Direct)
}

#[derive(Debug, Copy, Clone)]
struct ParsedPdfUnsignedInteger {
    value: usize,
    end: usize,
}

fn parse_pdf_direct_unsigned_integer(pdf: &[u8], offset: usize, limit: usize) -> Option<usize> {
    let parsed = parse_pdf_unsigned_integer(pdf, offset, limit)?;
    if !is_pdf_direct_integer_value_boundary(pdf.get(parsed.end).copied()) {
        return None;
    }
    if pdf_indirect_reference_after_integer(pdf, parsed.end, limit) {
        return None;
    }
    Some(parsed.value)
}

fn parse_pdf_unsigned_integer(
    pdf: &[u8],
    mut offset: usize,
    limit: usize,
) -> Option<ParsedPdfUnsignedInteger> {
    offset = skip_pdf_whitespace_and_comments(pdf, offset, limit);
    let mut value = 0usize;
    let mut digits = 0usize;
    while offset < limit {
        let byte = *pdf.get(offset)?;
        if !byte.is_ascii_digit() {
            break;
        }
        value = value
            .checked_mul(10)?
            .checked_add(usize::from(byte - b'0'))?;
        digits += 1;
        offset += 1;
    }
    if digits == 0 {
        None
    } else {
        Some(ParsedPdfUnsignedInteger { value, end: offset })
    }
}

fn pdf_indirect_reference_after_integer(pdf: &[u8], offset: usize, limit: usize) -> bool {
    let mut cursor = offset;
    if cursor >= limit
        || !(pdf.get(cursor).copied().is_some_and(is_pdf_whitespace)
            || pdf.get(cursor).copied() == Some(b'%'))
    {
        return false;
    }
    cursor = skip_pdf_whitespace_and_comments(pdf, cursor, limit);
    let Some(generation) = parse_pdf_unsigned_integer(pdf, cursor, limit) else {
        return false;
    };
    if !is_pdf_direct_integer_value_boundary(pdf.get(generation.end).copied()) {
        return false;
    }
    cursor = generation.end;
    if cursor >= limit
        || !(pdf.get(cursor).copied().is_some_and(is_pdf_whitespace)
            || pdf.get(cursor).copied() == Some(b'%'))
    {
        return false;
    }
    cursor = skip_pdf_whitespace_and_comments(pdf, cursor, limit);
    pdf.get(cursor).copied() == Some(b'R')
        && is_pdf_direct_integer_value_boundary(pdf.get(cursor + 1).copied())
}

fn is_pdf_direct_integer_value_boundary(byte: Option<u8>) -> bool {
    match byte {
        None => true,
        Some(byte) if is_pdf_whitespace(byte) => true,
        Some(b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%') => true,
        _ => false,
    }
}

fn stream_end_marker_after_declared_data(pdf: &[u8], stream_data_end: usize) -> Option<usize> {
    let mut marker_offset = stream_data_end;
    match pdf.get(marker_offset).copied() {
        Some(b'\r') if pdf.get(marker_offset + 1).copied() == Some(b'\n') => {
            marker_offset += 2;
        }
        Some(b'\r' | b'\n') => {
            marker_offset += 1;
        }
        _ => {}
    }
    if pdf
        .get(marker_offset..)
        .is_some_and(|tail| tail.starts_with(b"endstream"))
        && is_pdf_whitespace_or_eof(pdf.get(marker_offset + b"endstream".len()).copied())
    {
        Some(marker_offset)
    } else {
        None
    }
}

fn is_pdf_name_boundary(byte: Option<u8>) -> bool {
    byte.is_none_or(|byte| {
        is_pdf_whitespace(byte)
            || matches!(byte, b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'/' | b'%')
    })
}

fn is_pdf_whitespace_or_eof(byte: Option<u8>) -> bool {
    byte.is_none_or(is_pdf_whitespace)
}

fn is_pdf_whitespace(byte: u8) -> bool {
    matches!(byte, b'\0' | b'\t' | b'\n' | b'\x0c' | b'\r' | b' ')
}

#[cfg(test)]
mod tests {
    use super::*;
    include!("audit_tests.rs");
}
