//! Legacy code-page, Unicode, Symbol, and dingbat text decoding.

use super::*;

pub(in crate::rtf::parser) fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub(in crate::rtf::parser) fn take_rtf_unicode_char(
    pending_high_surrogate: &mut Option<u16>,
    value: i32,
) -> Option<char> {
    if value > 0xFFFF {
        *pending_high_surrogate = None;
        return char::from_u32(value as u32);
    }

    let unit = if value < 0 {
        let Some(adjusted) = value.checked_add(65_536) else {
            *pending_high_surrogate = None;
            return None;
        };
        match u16::try_from(adjusted) {
            Ok(unit) => unit,
            Err(_) => {
                *pending_high_surrogate = None;
                return None;
            }
        }
    } else {
        match u16::try_from(value) {
            Ok(unit) => unit,
            Err(_) => {
                *pending_high_surrogate = None;
                return None;
            }
        }
    };

    if (0xD800..=0xDBFF).contains(&unit) {
        *pending_high_surrogate = Some(unit);
        return None;
    }

    if (0xDC00..=0xDFFF).contains(&unit) {
        let high = pending_high_surrogate.take()?;
        let codepoint =
            0x1_0000 + (((u32::from(high) - 0xD800) << 10) | (u32::from(unit) - 0xDC00));
        return char::from_u32(codepoint);
    }

    *pending_high_surrogate = None;
    char::from_u32(u32::from(unit))
}

pub(in crate::rtf::parser) fn decode_hex_byte(byte: u8, code_page: CodePage) -> char {
    match code_page {
        CodePage::Windows1250 => decode_high_byte(byte, &WINDOWS_1250_HIGH),
        CodePage::Windows1251 => decode_high_byte(byte, &WINDOWS_1251_HIGH),
        CodePage::Windows1252 => decode_windows_1252(byte),
        CodePage::Windows1253 => decode_high_byte(byte, &WINDOWS_1253_HIGH),
        CodePage::Windows1254 => decode_high_byte(byte, &WINDOWS_1254_HIGH),
        CodePage::Windows1257 => decode_high_byte(byte, &WINDOWS_1257_HIGH),
        CodePage::MacRoman => decode_high_byte(byte, &MAC_ROMAN_HIGH),
        CodePage::Ibm437 => decode_high_byte(byte, &CP437_HIGH),
        CodePage::Ibm850 => decode_high_byte(byte, &CP850_HIGH),
        CodePage::Unsupported if byte.is_ascii() => byte as char,
        CodePage::Unsupported => '\u{fffd}',
    }
}

pub(in crate::rtf::parser) fn decode_raw_bytes(bytes: &[u8], code_page: CodePage) -> String {
    bytes
        .iter()
        .map(|byte| decode_hex_byte(*byte, code_page))
        .collect()
}

pub(in crate::rtf::parser) fn raw_bytes_to_lossless_text(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| char::from(*byte)).collect()
}

fn decode_high_byte(byte: u8, high_table: &[char; 128]) -> char {
    if byte.is_ascii() {
        byte as char
    } else {
        high_table[usize::from(byte - 0x80)]
    }
}

pub(in crate::rtf::parser) fn map_symbol_char(ch: char) -> char {
    let code = ch as u32;
    if code > 0xff {
        return ch;
    }
    match code as u8 {
        b'A' => '\u{0391}',
        b'B' => '\u{0392}',
        b'C' => '\u{03a7}',
        b'D' => '\u{0394}',
        b'E' => '\u{0395}',
        b'F' => '\u{03a6}',
        b'G' => '\u{0393}',
        b'H' => '\u{0397}',
        b'I' => '\u{0399}',
        b'K' => '\u{039a}',
        b'L' => '\u{039b}',
        b'M' => '\u{039c}',
        b'N' => '\u{039d}',
        b'O' => '\u{039f}',
        b'P' => '\u{03a0}',
        b'Q' => '\u{0398}',
        b'R' => '\u{03a1}',
        b'S' => '\u{03a3}',
        b'T' => '\u{03a4}',
        b'U' => '\u{03a5}',
        b'W' => '\u{03a9}',
        b'X' => '\u{039e}',
        b'Y' => '\u{03a8}',
        b'Z' => '\u{0396}',
        b'a' => '\u{03b1}',
        b'b' => '\u{03b2}',
        b'c' => '\u{03c7}',
        b'd' => '\u{03b4}',
        b'e' => '\u{03b5}',
        b'f' => '\u{03c6}',
        b'g' => '\u{03b3}',
        b'h' => '\u{03b7}',
        b'i' => '\u{03b9}',
        b'j' => '\u{03d5}',
        b'k' => '\u{03ba}',
        b'l' => '\u{03bb}',
        b'm' => '\u{03bc}',
        b'n' => '\u{03bd}',
        b'o' => '\u{03bf}',
        b'p' => '\u{03c0}',
        b'q' => '\u{03b8}',
        b'r' => '\u{03c1}',
        b's' => '\u{03c3}',
        b't' => '\u{03c4}',
        b'u' => '\u{03c5}',
        b'v' => '\u{03d6}',
        b'w' => '\u{03c9}',
        b'x' => '\u{03be}',
        b'y' => '\u{03c8}',
        b'z' => '\u{03b6}',
        0x22 => '\u{2200}',
        0x24 => '\u{2203}',
        0x27 => '\u{220b}',
        0x2a => '\u{2217}',
        0x2d => '\u{2212}',
        0x40 => '\u{2245}',
        0x5c => '\u{2234}',
        0x5e => '\u{22a5}',
        0x7e => '\u{223c}',
        0xa3 => '\u{2264}',
        0xb3 => '\u{2265}',
        0xb4 => '\u{00d7}',
        0xb5 => '\u{221d}',
        0xb6 => '\u{2202}',
        0xb7 => '\u{2022}',
        0xb8 => '\u{00f7}',
        0xb9 => '\u{2260}',
        0xba => '\u{2261}',
        0xbb => '\u{2248}',
        0xbc => '\u{2026}',
        0xbd => '\u{23d0}',
        0xbe => '\u{23af}',
        0xbf => '\u{21b5}',
        0xc0 => '\u{2135}',
        0xc1 => '\u{2111}',
        0xc2 => '\u{211c}',
        0xc3 => '\u{2118}',
        0xc4 => '\u{2297}',
        0xc5 => '\u{2295}',
        0xc6 => '\u{2205}',
        0xc7 => '\u{2229}',
        0xc8 => '\u{222a}',
        0xc9 => '\u{2283}',
        0xca => '\u{2287}',
        0xcb => '\u{2284}',
        0xcc => '\u{2282}',
        0xcd => '\u{2286}',
        0xce => '\u{2208}',
        0xcf => '\u{2209}',
        0xd0 => '\u{2220}',
        0xd1 => '\u{2207}',
        0xd2 => '\u{00ae}',
        0xd3 => '\u{00a9}',
        0xd4 => '\u{2122}',
        0xd5 => '\u{220f}',
        0xd6 => '\u{221a}',
        0xd7 => '\u{22c5}',
        0xd8 => '\u{00ac}',
        0xd9 => '\u{2227}',
        0xda => '\u{2228}',
        0xdb => '\u{21d4}',
        0xdc => '\u{21d0}',
        0xdd => '\u{21d1}',
        0xde => '\u{21d2}',
        0xdf => '\u{21d3}',
        0xe0 => '\u{25ca}',
        0xe5 => '\u{2211}',
        0xe6 => '\u{239b}',
        0xe7 => '\u{239c}',
        0xe8 => '\u{239d}',
        0xe9 => '\u{23a1}',
        0xea => '\u{23a2}',
        0xeb => '\u{23a3}',
        0xec => '\u{23a7}',
        0xed => '\u{23a8}',
        0xee => '\u{23a9}',
        0xef => '\u{23aa}',
        0xf0 => '\u{20ac}',
        0xf1 => '\u{2329}',
        0xf2 => '\u{222b}',
        0xf3 => '\u{232a}',
        0xf4 => '\u{2320}',
        0xf5 => '\u{23ae}',
        0xf6 => '\u{2321}',
        0xf7 => '\u{239e}',
        0xf8 => '\u{239f}',
        0xf9 => '\u{23a0}',
        0xfa => '\u{23a4}',
        0xfb => '\u{23a5}',
        0xfc => '\u{23a6}',
        0xfd => '\u{23ab}',
        0xfe => '\u{23ac}',
        0xff => '\u{23ad}',
        _ => ch,
    }
}

pub(in crate::rtf::parser) fn map_symbol_unicode_char(ch: char) -> char {
    let code = ch as u32;
    if (0xf000..=0xf0ff).contains(&code) {
        return map_symbol_char(char::from_u32(code - 0xf000).unwrap_or(ch));
    }
    map_symbol_char(ch)
}

fn is_wingdings_font_name(name: &str) -> bool {
    name.contains("wingdings") && !is_wingdings2_font_name(name) && !is_wingdings3_font_name(name)
}

fn is_wingdings2_font_name(name: &str) -> bool {
    name.contains("wingdings 2") || name.contains("wingdings2")
}

fn is_wingdings3_font_name(name: &str) -> bool {
    name.contains("wingdings 3") || name.contains("wingdings3")
}

pub(in crate::rtf::parser) fn is_webdings_font_name(name: &str) -> bool {
    name.contains("webdings")
}

pub(in crate::rtf::parser) fn dingbats_mapper_for_font_name(
    name: &str,
) -> Option<fn(char) -> char> {
    if is_wingdings2_font_name(name) {
        Some(map_wingdings2_char)
    } else if is_wingdings3_font_name(name) {
        Some(map_wingdings3_char)
    } else if is_wingdings_font_name(name) {
        Some(map_wingdings_char)
    } else {
        None
    }
}

pub(in crate::rtf::parser) fn map_dingbats_codepoint(
    font_name: &str,
    codepoint: u32,
) -> Option<char> {
    if is_wingdings2_font_name(font_name) {
        map_wingdings2_codepoint(codepoint)
    } else if is_wingdings3_font_name(font_name) {
        map_wingdings3_codepoint(codepoint)
    } else if is_wingdings_font_name(font_name) {
        map_wingdings_codepoint(codepoint)
    } else if is_webdings_font_name(font_name) {
        map_webdings_codepoint(codepoint)
    } else {
        None
    }
}

fn map_wingdings_char(ch: char) -> char {
    map_wingdings_codepoint(ch as u32).unwrap_or(ch)
}

fn map_wingdings_codepoint(codepoint: u32) -> Option<char> {
    let code = if (0xf000..=0xf0ff).contains(&codepoint) {
        (codepoint - 0xf000) as u8
    } else if codepoint <= u8::MAX as u32 {
        codepoint as u8
    } else {
        return None;
    };

    match code {
        0x4a => Some('\u{263a}'),
        0xa3 => Some('\u{25c9}'),
        0xfb => Some('\u{2717}'),
        0xfc => Some('\u{2713}'),
        0xfd => Some('\u{2612}'),
        0xfe => Some('\u{2611}'),
        _ => None,
    }
}

fn map_wingdings2_char(ch: char) -> char {
    map_wingdings2_codepoint(ch as u32).unwrap_or(ch)
}

fn map_wingdings2_codepoint(codepoint: u32) -> Option<char> {
    let code = if (0xf000..=0xf0ff).contains(&codepoint) {
        (codepoint - 0xf000) as u8
    } else if codepoint <= u8::MAX as u32 {
        codepoint as u8
    } else {
        return None;
    };

    match code {
        0x4f => Some('\u{2717}'),
        0x50 => Some('\u{2713}'),
        0x51 => Some('\u{2612}'),
        0x52 => Some('\u{2611}'),
        // Wingdings 2 has three visibly different boxed-X glyphs. Keep the
        // script, light, and bold variants distinct until passive vector
        // emission instead of collapsing all three to U+2612.
        0x53 => Some('\u{1f5f5}'),
        0x54 => Some('\u{1f5f7}'),
        _ => None,
    }
}

fn map_wingdings3_char(ch: char) -> char {
    map_wingdings3_codepoint(ch as u32).unwrap_or(ch)
}

fn map_wingdings3_codepoint(codepoint: u32) -> Option<char> {
    let code = if (0xf000..=0xf0ff).contains(&codepoint) {
        (codepoint - 0xf000) as u8
    } else if codepoint <= u8::MAX as u32 {
        codepoint as u8
    } else {
        return None;
    };

    match code {
        b'f' => Some('\u{2190}'),
        b'g' => Some('\u{2192}'),
        b'h' => Some('\u{2191}'),
        b'i' => Some('\u{2193}'),
        _ => None,
    }
}

pub(in crate::rtf::parser) fn map_webdings_char(ch: char) -> char {
    map_webdings_codepoint(ch as u32).unwrap_or(ch)
}

fn map_webdings_codepoint(codepoint: u32) -> Option<char> {
    let code = if (0xf000..=0xf0ff).contains(&codepoint) {
        (codepoint - 0xf000) as u8
    } else if codepoint <= u8::MAX as u32 {
        codepoint as u8
    } else {
        return None;
    };

    match code {
        0x3f => Some('\u{1f4e5}'),
        0x61 => Some('\u{2714}'),
        0x63 => Some('\u{25a1}'),
        _ => None,
    }
}

fn decode_windows_1252(byte: u8) -> char {
    match byte {
        0x80 => '\u{20ac}',
        0x81 => '\u{fffd}',
        0x82 => '\u{201a}',
        0x83 => '\u{0192}',
        0x84 => '\u{201e}',
        0x85 => '\u{2026}',
        0x86 => '\u{2020}',
        0x87 => '\u{2021}',
        0x88 => '\u{02c6}',
        0x89 => '\u{2030}',
        0x8a => '\u{0160}',
        0x8b => '\u{2039}',
        0x8c => '\u{0152}',
        0x8d => '\u{fffd}',
        0x8e => '\u{017d}',
        0x8f => '\u{fffd}',
        0x90 => '\u{fffd}',
        0x91 => '\u{2018}',
        0x92 => '\u{2019}',
        0x93 => '\u{201c}',
        0x94 => '\u{201d}',
        0x95 => '\u{2022}',
        0x96 => '\u{2013}',
        0x97 => '\u{2014}',
        0x98 => '\u{02dc}',
        0x99 => '\u{2122}',
        0x9a => '\u{0161}',
        0x9b => '\u{203a}',
        0x9c => '\u{0153}',
        0x9d => '\u{fffd}',
        0x9e => '\u{017e}',
        0x9f => '\u{0178}',
        _ => byte as char,
    }
}

const WINDOWS_1250_HIGH: [char; 128] = [
    '\u{20ac}', '\u{fffd}', '\u{201a}', '\u{fffd}', '\u{201e}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{fffd}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{015a}', '\u{0164}', '\u{017d}', '\u{0179}',
    '\u{fffd}', '\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{fffd}', '\u{2122}', '\u{0161}', '\u{203a}', '\u{015b}', '\u{0165}', '\u{017e}', '\u{017a}',
    '\u{00a0}', '\u{02c7}', '\u{02d8}', '\u{0141}', '\u{00a4}', '\u{0104}', '\u{00a6}', '\u{00a7}',
    '\u{00a8}', '\u{00a9}', '\u{015e}', '\u{00ab}', '\u{00ac}', '\u{00ad}', '\u{00ae}', '\u{017b}',
    '\u{00b0}', '\u{00b1}', '\u{02db}', '\u{0142}', '\u{00b4}', '\u{00b5}', '\u{00b6}', '\u{00b7}',
    '\u{00b8}', '\u{0105}', '\u{015f}', '\u{00bb}', '\u{013d}', '\u{02dd}', '\u{013e}', '\u{017c}',
    '\u{0154}', '\u{00c1}', '\u{00c2}', '\u{0102}', '\u{00c4}', '\u{0139}', '\u{0106}', '\u{00c7}',
    '\u{010c}', '\u{00c9}', '\u{0118}', '\u{00cb}', '\u{011a}', '\u{00cd}', '\u{00ce}', '\u{010e}',
    '\u{0110}', '\u{0143}', '\u{0147}', '\u{00d3}', '\u{00d4}', '\u{0150}', '\u{00d6}', '\u{00d7}',
    '\u{0158}', '\u{016e}', '\u{00da}', '\u{0170}', '\u{00dc}', '\u{00dd}', '\u{0162}', '\u{00df}',
    '\u{0155}', '\u{00e1}', '\u{00e2}', '\u{0103}', '\u{00e4}', '\u{013a}', '\u{0107}', '\u{00e7}',
    '\u{010d}', '\u{00e9}', '\u{0119}', '\u{00eb}', '\u{011b}', '\u{00ed}', '\u{00ee}', '\u{010f}',
    '\u{0111}', '\u{0144}', '\u{0148}', '\u{00f3}', '\u{00f4}', '\u{0151}', '\u{00f6}', '\u{00f7}',
    '\u{0159}', '\u{016f}', '\u{00fa}', '\u{0171}', '\u{00fc}', '\u{00fd}', '\u{0163}', '\u{02d9}',
];

const WINDOWS_1251_HIGH: [char; 128] = [
    '\u{0402}', '\u{0403}', '\u{201a}', '\u{0453}', '\u{201e}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{20ac}', '\u{2030}', '\u{0409}', '\u{2039}', '\u{040a}', '\u{040c}', '\u{040b}', '\u{040f}',
    '\u{0452}', '\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{fffd}', '\u{2122}', '\u{0459}', '\u{203a}', '\u{045a}', '\u{045c}', '\u{045b}', '\u{045f}',
    '\u{00a0}', '\u{040e}', '\u{045e}', '\u{0408}', '\u{00a4}', '\u{0490}', '\u{00a6}', '\u{00a7}',
    '\u{0401}', '\u{00a9}', '\u{0404}', '\u{00ab}', '\u{00ac}', '\u{00ad}', '\u{00ae}', '\u{0407}',
    '\u{00b0}', '\u{00b1}', '\u{0406}', '\u{0456}', '\u{0491}', '\u{00b5}', '\u{00b6}', '\u{00b7}',
    '\u{0451}', '\u{2116}', '\u{0454}', '\u{00bb}', '\u{0458}', '\u{0405}', '\u{0455}', '\u{0457}',
    '\u{0410}', '\u{0411}', '\u{0412}', '\u{0413}', '\u{0414}', '\u{0415}', '\u{0416}', '\u{0417}',
    '\u{0418}', '\u{0419}', '\u{041a}', '\u{041b}', '\u{041c}', '\u{041d}', '\u{041e}', '\u{041f}',
    '\u{0420}', '\u{0421}', '\u{0422}', '\u{0423}', '\u{0424}', '\u{0425}', '\u{0426}', '\u{0427}',
    '\u{0428}', '\u{0429}', '\u{042a}', '\u{042b}', '\u{042c}', '\u{042d}', '\u{042e}', '\u{042f}',
    '\u{0430}', '\u{0431}', '\u{0432}', '\u{0433}', '\u{0434}', '\u{0435}', '\u{0436}', '\u{0437}',
    '\u{0438}', '\u{0439}', '\u{043a}', '\u{043b}', '\u{043c}', '\u{043d}', '\u{043e}', '\u{043f}',
    '\u{0440}', '\u{0441}', '\u{0442}', '\u{0443}', '\u{0444}', '\u{0445}', '\u{0446}', '\u{0447}',
    '\u{0448}', '\u{0449}', '\u{044a}', '\u{044b}', '\u{044c}', '\u{044d}', '\u{044e}', '\u{044f}',
];

const WINDOWS_1253_HIGH: [char; 128] = [
    '\u{20ac}', '\u{fffd}', '\u{201a}', '\u{0192}', '\u{201e}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{fffd}', '\u{2030}', '\u{fffd}', '\u{2039}', '\u{fffd}', '\u{fffd}', '\u{fffd}', '\u{fffd}',
    '\u{fffd}', '\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{fffd}', '\u{2122}', '\u{fffd}', '\u{203a}', '\u{fffd}', '\u{fffd}', '\u{fffd}', '\u{fffd}',
    '\u{00a0}', '\u{0385}', '\u{0386}', '\u{00a3}', '\u{00a4}', '\u{00a5}', '\u{00a6}', '\u{00a7}',
    '\u{00a8}', '\u{00a9}', '\u{fffd}', '\u{00ab}', '\u{00ac}', '\u{00ad}', '\u{00ae}', '\u{2015}',
    '\u{00b0}', '\u{00b1}', '\u{00b2}', '\u{00b3}', '\u{0384}', '\u{00b5}', '\u{00b6}', '\u{00b7}',
    '\u{0388}', '\u{0389}', '\u{038a}', '\u{00bb}', '\u{038c}', '\u{00bd}', '\u{038e}', '\u{038f}',
    '\u{0390}', '\u{0391}', '\u{0392}', '\u{0393}', '\u{0394}', '\u{0395}', '\u{0396}', '\u{0397}',
    '\u{0398}', '\u{0399}', '\u{039a}', '\u{039b}', '\u{039c}', '\u{039d}', '\u{039e}', '\u{039f}',
    '\u{03a0}', '\u{03a1}', '\u{fffd}', '\u{03a3}', '\u{03a4}', '\u{03a5}', '\u{03a6}', '\u{03a7}',
    '\u{03a8}', '\u{03a9}', '\u{03aa}', '\u{03ab}', '\u{03ac}', '\u{03ad}', '\u{03ae}', '\u{03af}',
    '\u{03b0}', '\u{03b1}', '\u{03b2}', '\u{03b3}', '\u{03b4}', '\u{03b5}', '\u{03b6}', '\u{03b7}',
    '\u{03b8}', '\u{03b9}', '\u{03ba}', '\u{03bb}', '\u{03bc}', '\u{03bd}', '\u{03be}', '\u{03bf}',
    '\u{03c0}', '\u{03c1}', '\u{03c2}', '\u{03c3}', '\u{03c4}', '\u{03c5}', '\u{03c6}', '\u{03c7}',
    '\u{03c8}', '\u{03c9}', '\u{03ca}', '\u{03cb}', '\u{03cc}', '\u{03cd}', '\u{03ce}', '\u{fffd}',
];

const WINDOWS_1254_HIGH: [char; 128] = [
    '\u{20ac}', '\u{fffd}', '\u{201a}', '\u{0192}', '\u{201e}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{02c6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{fffd}', '\u{fffd}', '\u{fffd}',
    '\u{fffd}', '\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{02dc}', '\u{2122}', '\u{0161}', '\u{203a}', '\u{0153}', '\u{fffd}', '\u{fffd}', '\u{0178}',
    '\u{00a0}', '\u{00a1}', '\u{00a2}', '\u{00a3}', '\u{00a4}', '\u{00a5}', '\u{00a6}', '\u{00a7}',
    '\u{00a8}', '\u{00a9}', '\u{00aa}', '\u{00ab}', '\u{00ac}', '\u{00ad}', '\u{00ae}', '\u{00af}',
    '\u{00b0}', '\u{00b1}', '\u{00b2}', '\u{00b3}', '\u{00b4}', '\u{00b5}', '\u{00b6}', '\u{00b7}',
    '\u{00b8}', '\u{00b9}', '\u{00ba}', '\u{00bb}', '\u{00bc}', '\u{00bd}', '\u{00be}', '\u{00bf}',
    '\u{00c0}', '\u{00c1}', '\u{00c2}', '\u{00c3}', '\u{00c4}', '\u{00c5}', '\u{00c6}', '\u{00c7}',
    '\u{00c8}', '\u{00c9}', '\u{00ca}', '\u{00cb}', '\u{00cc}', '\u{00cd}', '\u{00ce}', '\u{00cf}',
    '\u{011e}', '\u{00d1}', '\u{00d2}', '\u{00d3}', '\u{00d4}', '\u{00d5}', '\u{00d6}', '\u{00d7}',
    '\u{00d8}', '\u{00d9}', '\u{00da}', '\u{00db}', '\u{00dc}', '\u{0130}', '\u{015e}', '\u{00df}',
    '\u{00e0}', '\u{00e1}', '\u{00e2}', '\u{00e3}', '\u{00e4}', '\u{00e5}', '\u{00e6}', '\u{00e7}',
    '\u{00e8}', '\u{00e9}', '\u{00ea}', '\u{00eb}', '\u{00ec}', '\u{00ed}', '\u{00ee}', '\u{00ef}',
    '\u{011f}', '\u{00f1}', '\u{00f2}', '\u{00f3}', '\u{00f4}', '\u{00f5}', '\u{00f6}', '\u{00f7}',
    '\u{00f8}', '\u{00f9}', '\u{00fa}', '\u{00fb}', '\u{00fc}', '\u{0131}', '\u{015f}', '\u{00ff}',
];

const WINDOWS_1257_HIGH: [char; 128] = [
    '\u{20ac}', '\u{fffd}', '\u{201a}', '\u{fffd}', '\u{201e}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{fffd}', '\u{2030}', '\u{fffd}', '\u{2039}', '\u{fffd}', '\u{00a8}', '\u{02c7}', '\u{00b8}',
    '\u{fffd}', '\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{fffd}', '\u{2122}', '\u{fffd}', '\u{203a}', '\u{fffd}', '\u{00af}', '\u{02db}', '\u{fffd}',
    '\u{00a0}', '\u{fffd}', '\u{00a2}', '\u{00a3}', '\u{00a4}', '\u{fffd}', '\u{00a6}', '\u{00a7}',
    '\u{00d8}', '\u{00a9}', '\u{0156}', '\u{00ab}', '\u{00ac}', '\u{00ad}', '\u{00ae}', '\u{00c6}',
    '\u{00b0}', '\u{00b1}', '\u{00b2}', '\u{00b3}', '\u{00b4}', '\u{00b5}', '\u{00b6}', '\u{00b7}',
    '\u{00f8}', '\u{00b9}', '\u{0157}', '\u{00bb}', '\u{00bc}', '\u{00bd}', '\u{00be}', '\u{00e6}',
    '\u{0104}', '\u{012e}', '\u{0100}', '\u{0106}', '\u{00c4}', '\u{00c5}', '\u{0118}', '\u{0112}',
    '\u{010c}', '\u{00c9}', '\u{0179}', '\u{0116}', '\u{0122}', '\u{0136}', '\u{012a}', '\u{013b}',
    '\u{0160}', '\u{0143}', '\u{0145}', '\u{00d3}', '\u{014c}', '\u{00d5}', '\u{00d6}', '\u{00d7}',
    '\u{0172}', '\u{0141}', '\u{015a}', '\u{016a}', '\u{00dc}', '\u{017b}', '\u{017d}', '\u{00df}',
    '\u{0105}', '\u{012f}', '\u{0101}', '\u{0107}', '\u{00e4}', '\u{00e5}', '\u{0119}', '\u{0113}',
    '\u{010d}', '\u{00e9}', '\u{017a}', '\u{0117}', '\u{0123}', '\u{0137}', '\u{012b}', '\u{013c}',
    '\u{0161}', '\u{0144}', '\u{0146}', '\u{00f3}', '\u{014d}', '\u{00f5}', '\u{00f6}', '\u{00f7}',
    '\u{0173}', '\u{0142}', '\u{015b}', '\u{016b}', '\u{00fc}', '\u{017c}', '\u{017e}', '\u{02d9}',
];

const MAC_ROMAN_HIGH: [char; 128] = [
    '\u{00c4}', '\u{00c5}', '\u{00c7}', '\u{00c9}', '\u{00d1}', '\u{00d6}', '\u{00dc}', '\u{00e1}',
    '\u{00e0}', '\u{00e2}', '\u{00e4}', '\u{00e3}', '\u{00e5}', '\u{00e7}', '\u{00e9}', '\u{00e8}',
    '\u{00ea}', '\u{00eb}', '\u{00ed}', '\u{00ec}', '\u{00ee}', '\u{00ef}', '\u{00f1}', '\u{00f3}',
    '\u{00f2}', '\u{00f4}', '\u{00f6}', '\u{00f5}', '\u{00fa}', '\u{00f9}', '\u{00fb}', '\u{00fc}',
    '\u{2020}', '\u{00b0}', '\u{00a2}', '\u{00a3}', '\u{00a7}', '\u{2022}', '\u{00b6}', '\u{00df}',
    '\u{00ae}', '\u{00a9}', '\u{2122}', '\u{00b4}', '\u{00a8}', '\u{2260}', '\u{00c6}', '\u{00d8}',
    '\u{221e}', '\u{00b1}', '\u{2264}', '\u{2265}', '\u{00a5}', '\u{00b5}', '\u{2202}', '\u{2211}',
    '\u{220f}', '\u{03c0}', '\u{222b}', '\u{00aa}', '\u{00ba}', '\u{03a9}', '\u{00e6}', '\u{00f8}',
    '\u{00bf}', '\u{00a1}', '\u{00ac}', '\u{221a}', '\u{0192}', '\u{2248}', '\u{2206}', '\u{00ab}',
    '\u{00bb}', '\u{2026}', '\u{00a0}', '\u{00c0}', '\u{00c3}', '\u{00d5}', '\u{0152}', '\u{0153}',
    '\u{2013}', '\u{2014}', '\u{201c}', '\u{201d}', '\u{2018}', '\u{2019}', '\u{00f7}', '\u{25ca}',
    '\u{00ff}', '\u{0178}', '\u{2044}', '\u{20ac}', '\u{2039}', '\u{203a}', '\u{fb01}', '\u{fb02}',
    '\u{2021}', '\u{00b7}', '\u{201a}', '\u{201e}', '\u{2030}', '\u{00c2}', '\u{00ca}', '\u{00c1}',
    '\u{00cb}', '\u{00c8}', '\u{00cd}', '\u{00ce}', '\u{00cf}', '\u{00cc}', '\u{00d3}', '\u{00d4}',
    '\u{f8ff}', '\u{00d2}', '\u{00da}', '\u{00db}', '\u{00d9}', '\u{0131}', '\u{02c6}', '\u{02dc}',
    '\u{00af}', '\u{02d8}', '\u{02d9}', '\u{02da}', '\u{00b8}', '\u{02dd}', '\u{02db}', '\u{02c7}',
];

const CP437_HIGH: [char; 128] = [
    '\u{00c7}', '\u{00fc}', '\u{00e9}', '\u{00e2}', '\u{00e4}', '\u{00e0}', '\u{00e5}', '\u{00e7}',
    '\u{00ea}', '\u{00eb}', '\u{00e8}', '\u{00ef}', '\u{00ee}', '\u{00ec}', '\u{00c4}', '\u{00c5}',
    '\u{00c9}', '\u{00e6}', '\u{00c6}', '\u{00f4}', '\u{00f6}', '\u{00f2}', '\u{00fb}', '\u{00f9}',
    '\u{00ff}', '\u{00d6}', '\u{00dc}', '\u{00a2}', '\u{00a3}', '\u{00a5}', '\u{20a7}', '\u{0192}',
    '\u{00e1}', '\u{00ed}', '\u{00f3}', '\u{00fa}', '\u{00f1}', '\u{00d1}', '\u{00aa}', '\u{00ba}',
    '\u{00bf}', '\u{2310}', '\u{00ac}', '\u{00bd}', '\u{00bc}', '\u{00a1}', '\u{00ab}', '\u{00bb}',
    '\u{2591}', '\u{2592}', '\u{2593}', '\u{2502}', '\u{2524}', '\u{2561}', '\u{2562}', '\u{2556}',
    '\u{2555}', '\u{2563}', '\u{2551}', '\u{2557}', '\u{255d}', '\u{255c}', '\u{255b}', '\u{2510}',
    '\u{2514}', '\u{2534}', '\u{252c}', '\u{251c}', '\u{2500}', '\u{253c}', '\u{255e}', '\u{255f}',
    '\u{255a}', '\u{2554}', '\u{2569}', '\u{2566}', '\u{2560}', '\u{2550}', '\u{256c}', '\u{2567}',
    '\u{2568}', '\u{2564}', '\u{2565}', '\u{2559}', '\u{2558}', '\u{2552}', '\u{2553}', '\u{256b}',
    '\u{256a}', '\u{2518}', '\u{250c}', '\u{2588}', '\u{2584}', '\u{258c}', '\u{2590}', '\u{2580}',
    '\u{03b1}', '\u{00df}', '\u{0393}', '\u{03c0}', '\u{03a3}', '\u{03c3}', '\u{00b5}', '\u{03c4}',
    '\u{03a6}', '\u{0398}', '\u{03a9}', '\u{03b4}', '\u{221e}', '\u{03c6}', '\u{03b5}', '\u{2229}',
    '\u{2261}', '\u{00b1}', '\u{2265}', '\u{2264}', '\u{2320}', '\u{2321}', '\u{00f7}', '\u{2248}',
    '\u{00b0}', '\u{2219}', '\u{00b7}', '\u{221a}', '\u{207f}', '\u{00b2}', '\u{25a0}', '\u{00a0}',
];

const CP850_HIGH: [char; 128] = [
    '\u{00c7}', '\u{00fc}', '\u{00e9}', '\u{00e2}', '\u{00e4}', '\u{00e0}', '\u{00e5}', '\u{00e7}',
    '\u{00ea}', '\u{00eb}', '\u{00e8}', '\u{00ef}', '\u{00ee}', '\u{00ec}', '\u{00c4}', '\u{00c5}',
    '\u{00c9}', '\u{00e6}', '\u{00c6}', '\u{00f4}', '\u{00f6}', '\u{00f2}', '\u{00fb}', '\u{00f9}',
    '\u{00ff}', '\u{00d6}', '\u{00dc}', '\u{00f8}', '\u{00a3}', '\u{00d8}', '\u{00d7}', '\u{0192}',
    '\u{00e1}', '\u{00ed}', '\u{00f3}', '\u{00fa}', '\u{00f1}', '\u{00d1}', '\u{00aa}', '\u{00ba}',
    '\u{00bf}', '\u{00ae}', '\u{00ac}', '\u{00bd}', '\u{00bc}', '\u{00a1}', '\u{00ab}', '\u{00bb}',
    '\u{2591}', '\u{2592}', '\u{2593}', '\u{2502}', '\u{2524}', '\u{00c1}', '\u{00c2}', '\u{00c0}',
    '\u{00a9}', '\u{2563}', '\u{2551}', '\u{2557}', '\u{255d}', '\u{00a2}', '\u{00a5}', '\u{2510}',
    '\u{2514}', '\u{2534}', '\u{252c}', '\u{251c}', '\u{2500}', '\u{253c}', '\u{00e3}', '\u{00c3}',
    '\u{255a}', '\u{2554}', '\u{2569}', '\u{2566}', '\u{2560}', '\u{2550}', '\u{256c}', '\u{00a4}',
    '\u{00f0}', '\u{00d0}', '\u{00ca}', '\u{00cb}', '\u{00c8}', '\u{0131}', '\u{00cd}', '\u{00ce}',
    '\u{00cf}', '\u{2518}', '\u{250c}', '\u{2588}', '\u{2584}', '\u{00a6}', '\u{00cc}', '\u{2580}',
    '\u{00d3}', '\u{00df}', '\u{00d4}', '\u{00d2}', '\u{00f5}', '\u{00d5}', '\u{00b5}', '\u{00fe}',
    '\u{00de}', '\u{00da}', '\u{00db}', '\u{00d9}', '\u{00fd}', '\u{00dd}', '\u{00af}', '\u{00b4}',
    '\u{00ad}', '\u{00b1}', '\u{2017}', '\u{00be}', '\u{00b6}', '\u{00a7}', '\u{00f7}', '\u{00b8}',
    '\u{00b0}', '\u{00a8}', '\u{00b7}', '\u{00b9}', '\u{00b3}', '\u{00b2}', '\u{25a0}', '\u{00a0}',
];
