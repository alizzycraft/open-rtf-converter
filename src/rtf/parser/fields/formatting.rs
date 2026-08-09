//! Passive field results and bounded text, numeric, and date formatting.

use super::super::*;
use super::formula::*;

pub(in crate::rtf::parser) struct PassiveFieldResult {
    pub(in crate::rtf::parser) dynamic_field: Option<InlineKind>,
    pub(in crate::rtf::parser) text: String,
    pub(in crate::rtf::parser) font_name: Option<String>,
    pub(in crate::rtf::parser) font_size_half_points: Option<i32>,
    pub(in crate::rtf::parser) form_field: bool,
}

pub(in crate::rtf::parser) struct PassiveFieldSegment {
    pub(in crate::rtf::parser) text: String,
    pub(in crate::rtf::parser) baseline_shift_half_points: Option<i32>,
    pub(in crate::rtf::parser) font_size_scale_percent: Option<i32>,
    pub(in crate::rtf::parser) overline: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub(in crate::rtf::parser) struct PassiveAdvanceFieldOffset {
    pub(in crate::rtf::parser) horizontal_twips: i32,
    pub(in crate::rtf::parser) vertical_twips: i32,
}

pub(in crate::rtf::parser) struct FieldSequenceInstruction {
    pub(in crate::rtf::parser) name: String,
    pub(in crate::rtf::parser) repeat_current: bool,
    pub(in crate::rtf::parser) hidden: bool,
    pub(in crate::rtf::parser) reset_value: Option<i32>,
}

pub(in crate::rtf::parser) struct FieldListNumberInstruction {
    pub(in crate::rtf::parser) name: String,
    pub(in crate::rtf::parser) level: i32,
    pub(in crate::rtf::parser) reset_value: Option<i32>,
}

pub(in crate::rtf::parser) const MAX_PASSIVE_FIELD_FORMAT_TEXT_CHARS: usize = 4096;
pub(in crate::rtf::parser) const MAX_PASSIVE_FIELD_NUMERIC_PICTURE_CHARS: usize = 64;
pub(in crate::rtf::parser) const MAX_PASSIVE_FIELD_DATE_PICTURE_CHARS: usize = 96;
pub(in crate::rtf::parser) const MAX_PASSIVE_FIELD_SWITCH_VALUE_CHARS: usize = 128;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(in crate::rtf::parser) enum FieldTextFormatSwitch {
    Upper,
    Lower,
    FirstCap,
    Caps,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(in crate::rtf::parser) enum FieldNumberFormatSwitch {
    Arabic,
    ArabicDash,
    CardText,
    DollarText,
    UpperAlphabetic,
    LowerAlphabetic,
    UpperRoman,
    LowerRoman,
    Ordinal,
    OrdinalText,
    Hex,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(in crate::rtf::parser) enum FieldFormatSwitch {
    Text(FieldTextFormatSwitch),
    Number(FieldNumberFormatSwitch),
    NoOp,
}

impl PassiveFieldResult {
    pub(in crate::rtf::parser) fn dynamic(field: DynamicField) -> Self {
        Self {
            dynamic_field: Some(InlineKind::DynamicField(field)),
            text: String::new(),
            font_name: None,
            font_size_half_points: None,
            form_field: false,
        }
    }
}

pub(in crate::rtf::parser) fn passive_advance_field_offset_twips(
    instruction: &str,
    max_advance_twips: i32,
) -> PassiveAdvanceFieldOffset {
    let right = field_switch_i32_value(instruction, 'r').unwrap_or(0);
    let left = field_switch_i32_value(instruction, 'l').unwrap_or(0);
    let up = field_switch_i32_value(instruction, 'u').unwrap_or(0);
    let down = field_switch_i32_value(instruction, 'd').unwrap_or(0);
    let horizontal = i64::from(right) - i64::from(left);
    let vertical = i64::from(up) - i64::from(down);
    let limit = i64::from(max_advance_twips.max(0));
    PassiveAdvanceFieldOffset {
        horizontal_twips: horizontal.clamp(-limit, limit) as i32,
        vertical_twips: vertical.clamp(-limit, limit) as i32,
    }
}

pub(in crate::rtf::parser) fn passive_malformed_advance_literal(
    instruction: &str,
) -> Option<String> {
    let rest = field_rest_after_name(instruction)?.trim();
    if rest.is_empty() || rest.contains('\\') {
        return None;
    }
    let values = rest.split_whitespace().collect::<Vec<_>>();
    if values.is_empty()
        || values.len() > 4
        || values.iter().any(|value| value.parse::<i32>().is_err())
    {
        return None;
    }
    Some(values.join(" "))
}

pub(in crate::rtf::parser) fn apply_field_format_switches(
    instruction: &str,
    mut result: PassiveFieldResult,
) -> Option<PassiveFieldResult> {
    if let Some(InlineKind::DynamicField(field)) = result.dynamic_field {
        return (field_instruction_dynamic_field(instruction) == Some(field)).then_some(result);
    }
    if matches!(
        result.dynamic_field,
        Some(InlineKind::BookmarkPageReference(_))
    ) {
        return (field_instruction_name(instruction) == Some("PAGEREF")).then_some(result);
    }
    if result.dynamic_field.is_some() {
        return None;
    }
    if result.font_name.is_some() {
        if !passive_field_result_text_is_safe(&result.text) {
            return None;
        }
        return Some(result);
    }
    if contains_internal_marker(&result.text) {
        return None;
    }

    if let Some(picture) = field_numeric_picture_switch(instruction)? {
        result.text = apply_field_numeric_picture_switch(&result.text, &picture)?;
        if result.text.chars().count() > MAX_PASSIVE_FIELD_FORMAT_TEXT_CHARS {
            return None;
        }
    }

    if !passive_field_result_text_is_safe(&result.text) {
        return None;
    }

    let switches = field_format_switches(instruction)?;
    if switches.is_empty() {
        return Some(result);
    }
    if result.text.chars().count() > MAX_PASSIVE_FIELD_FORMAT_TEXT_CHARS {
        return None;
    }

    for switch in switches {
        result.text = match switch {
            FieldFormatSwitch::Text(switch) => apply_field_text_format_switch(&result.text, switch),
            FieldFormatSwitch::Number(switch) => {
                apply_field_number_format_switch(&result.text, switch)?
            }
            FieldFormatSwitch::NoOp => result.text.clone(),
        };
        if result.text.chars().count() > MAX_PASSIVE_FIELD_FORMAT_TEXT_CHARS {
            return None;
        }
    }

    if result.text.chars().any(|ch| ch.is_control()) || contains_internal_marker(&result.text) {
        return None;
    }

    Some(result)
}

pub(in crate::rtf::parser) fn field_instruction_dynamic_field(
    instruction: &str,
) -> Option<DynamicField> {
    match field_instruction_name(instruction)? {
        "PAGE" => Some(DynamicField::PageNumber),
        "NUMPAGES" => Some(DynamicField::TotalPages),
        "NUMWORDS" => Some(DynamicField::DocumentWords),
        "NUMCHARS" => Some(DynamicField::DocumentCharacters),
        "NUMCHARSWS" => Some(DynamicField::DocumentCharactersWithSpaces),
        "SECTION" => Some(DynamicField::SectionNumber),
        "SECTIONPAGES" => Some(DynamicField::SectionPages),
        "DOCPROPERTY" | "INFO" => field_first_argument(instruction)
            .and_then(|name| document_stat_property_dynamic_field(&name)),
        _ => None,
    }
}

pub(in crate::rtf::parser) fn passive_field_result_text_is_safe(text: &str) -> bool {
    text.chars().count() <= MAX_PASSIVE_FIELD_FORMAT_TEXT_CHARS
        && !text.chars().any(|ch| ch.is_control())
        && !contains_internal_marker(text)
}

pub(in crate::rtf::parser) fn field_uses_numeric_formatting(instruction: &str) -> bool {
    if matches!(field_numeric_picture_switch(instruction), Some(Some(_))) {
        return true;
    }
    field_format_switches(instruction).is_some_and(|switches| {
        switches
            .iter()
            .any(|switch| matches!(switch, FieldFormatSwitch::Number(_)))
    })
}

pub(in crate::rtf::parser) fn passive_field_result(
    instruction: &str,
    form_checkbox_checked: Option<bool>,
    form_checkbox_size_half_points: Option<i32>,
    form_default_text: &str,
    form_dropdown_entries: &[String],
    form_dropdown_selected_index: Option<i32>,
) -> Option<PassiveFieldResult> {
    match field_instruction_name(instruction)? {
        "PAGE" => Some(PassiveFieldResult::dynamic(DynamicField::PageNumber)),
        "NUMPAGES" => Some(PassiveFieldResult::dynamic(DynamicField::TotalPages)),
        "NUMWORDS" => Some(PassiveFieldResult::dynamic(DynamicField::DocumentWords)),
        "NUMCHARS" => Some(PassiveFieldResult::dynamic(
            DynamicField::DocumentCharacters,
        )),
        "NUMCHARSWS" => Some(PassiveFieldResult::dynamic(
            DynamicField::DocumentCharactersWithSpaces,
        )),
        "SECTION" => Some(PassiveFieldResult::dynamic(DynamicField::SectionNumber)),
        "SECTIONPAGES" => Some(PassiveFieldResult::dynamic(DynamicField::SectionPages)),
        "FORMTEXT" => passive_form_text_field_result(form_default_text),
        "FORMDROPDOWN" => passive_form_dropdown_field_result(
            form_dropdown_entries,
            form_dropdown_selected_index.unwrap_or(0),
        ),
        "FORMCHECKBOX" => Some(PassiveFieldResult {
            dynamic_field: None,
            text: if form_checkbox_checked.unwrap_or(false) {
                "\u{2611}"
            } else {
                "\u{2610}"
            }
            .to_string(),
            font_name: Some("ZapfDingbats".to_string()),
            font_size_half_points: form_checkbox_size_half_points,
            form_field: true,
        }),
        "FORMULA" => passive_formula_field_result(instruction),
        "COMPARE" => passive_compare_field_result(instruction),
        "IF" => passive_if_field_result(instruction),
        "EQ" => passive_eq_field_result(instruction),
        "GOTOBUTTON" => passive_gotobutton_field_result(instruction),
        "MACROBUTTON" => passive_macrobutton_field_result(instruction),
        "MERGEFIELD" => passive_mergefield_result(instruction),
        "QUOTE" => passive_quote_field_result(instruction),
        "SYMBOL" => passive_symbol_field_result(instruction),
        _ => None,
    }
}

pub(in crate::rtf::parser) fn passive_field_result_needs_diagnostic(instruction: &str) -> bool {
    !matches!(field_instruction_name(instruction), Some("SYMBOL"))
}

pub(in crate::rtf::parser) fn passive_field_result_diagnostic(instruction: &str) -> String {
    match field_instruction_name(instruction).unwrap_or("unknown") {
        name @ ("FORMTEXT" | "FORMDROPDOWN" | "FORMCHECKBOX") => {
            format!("rendering passive form field {name} without creating PDF form actions")
        }
        name => format!("rendering passive field {name} without executing field instruction"),
    }
}

pub(in crate::rtf::parser) fn stored_field_result_diagnostic(instruction: &str) -> String {
    match field_instruction_name(instruction) {
        Some(name @ ("FORMTEXT" | "FORMDROPDOWN" | "FORMCHECKBOX")) => {
            format!(
                "rendering stored result for passive form field {name} without creating PDF form actions"
            )
        }
        Some(name) => {
            format!("rendering stored result for field {name} without executing field instruction")
        }
        None => "rendering stored field result without executing field instruction".to_string(),
    }
}

pub(in crate::rtf::parser) fn is_form_field_instruction_name(name: &str) -> bool {
    matches!(name, "FORMTEXT" | "FORMDROPDOWN" | "FORMCHECKBOX")
}

pub(in crate::rtf::parser) fn field_instruction_name(instruction: &str) -> Option<&'static str> {
    if instruction.trim_start().starts_with('=') {
        return Some("FORMULA");
    }
    let name = instruction
        .trim_start()
        .chars()
        .take_while(|ch| ch.is_ascii_alphabetic())
        .collect::<String>()
        .to_ascii_uppercase();
    match name.as_str() {
        "ADDIN" => Some("ADDIN"),
        "ADDRESSBLOCK" => Some("ADDRESSBLOCK"),
        "ADVANCE" => Some("ADVANCE"),
        "ASK" => Some("ASK"),
        "AUTHOR" => Some("AUTHOR"),
        "AUTOTEXT" => Some("AUTOTEXT"),
        "AUTOTEXTLIST" => Some("AUTOTEXTLIST"),
        "AUTONUM" => Some("AUTONUM"),
        "AUTONUMLGL" => Some("AUTONUMLGL"),
        "AUTONUMOUT" => Some("AUTONUMOUT"),
        "BARCODE" => Some("BARCODE"),
        "BIBLIOGRAPHY" => Some("BIBLIOGRAPHY"),
        "CITATION" => Some("CITATION"),
        "COMMENTS" => Some("COMMENTS"),
        "COMPARE" => Some("COMPARE"),
        "COMPANY" => Some("COMPANY"),
        "CONTROL" => Some("CONTROL"),
        "PAGE" => Some("PAGE"),
        "NUMPAGES" => Some("NUMPAGES"),
        "NUMWORDS" => Some("NUMWORDS"),
        "NUMCHARS" => Some("NUMCHARS"),
        "NUMCHARSWS" => Some("NUMCHARSWS"),
        "NOTEREF" => Some("NOTEREF"),
        "PAGEREF" => Some("PAGEREF"),
        "QUOTE" => Some("QUOTE"),
        "REF" => Some("REF"),
        "REVNUM" => Some("REVNUM"),
        "SECTION" => Some("SECTION"),
        "SECTIONPAGES" => Some("SECTIONPAGES"),
        "SEQ" => Some("SEQ"),
        "SET" => Some("SET"),
        "SHAPE" => Some("SHAPE"),
        "TA" => Some("TA"),
        "TC" => Some("TC"),
        "CREATEDATE" => Some("CREATEDATE"),
        "DATE" => Some("DATE"),
        "DOCPROPERTY" => Some("DOCPROPERTY"),
        "DOCVARIABLE" => Some("DOCVARIABLE"),
        "EDITTIME" => Some("EDITTIME"),
        "EMBED" => Some("EMBED"),
        "EQ" => Some("EQ"),
        "FILENAME" => Some("FILENAME"),
        "FILESIZE" => Some("FILESIZE"),
        "FILLIN" => Some("FILLIN"),
        "FORMDROPDOWN" => Some("FORMDROPDOWN"),
        "FORMTEXT" => Some("FORMTEXT"),
        "FORMCHECKBOX" => Some("FORMCHECKBOX"),
        "DATABASE" => Some("DATABASE"),
        "DDE" => Some("DDE"),
        "DDEAUTO" => Some("DDEAUTO"),
        "DISPLAYBARCODE" => Some("DISPLAYBARCODE"),
        "GOTOBUTTON" => Some("GOTOBUTTON"),
        "GREETINGLINE" => Some("GREETINGLINE"),
        "HTMLBUTTON" => Some("HTMLBUTTON"),
        "HTMLCHECKBOX" => Some("HTMLCHECKBOX"),
        "HTMLCONTROL" => Some("HTMLCONTROL"),
        "HTMLHIDDEN" => Some("HTMLHIDDEN"),
        "HTMLINPUT" => Some("HTMLINPUT"),
        "HTMLLISTBOX" => Some("HTMLLISTBOX"),
        "HTMLOPTION" => Some("HTMLOPTION"),
        "HTMLPASSWORD" => Some("HTMLPASSWORD"),
        "HTMLRADIO" => Some("HTMLRADIO"),
        "HTMLRADIOBUTTON" => Some("HTMLRADIOBUTTON"),
        "HTMLSELECT" => Some("HTMLSELECT"),
        "HTMLTEXTAREA" => Some("HTMLTEXTAREA"),
        "HTMLTEXTBOX" => Some("HTMLTEXTBOX"),
        "HYPERLINK" => Some("HYPERLINK"),
        "IF" => Some("IF"),
        "INDEX" => Some("INDEX"),
        "IMPORT" => Some("IMPORT"),
        "INCLUDEPICTURE" => Some("INCLUDEPICTURE"),
        "INCLUDETEXT" => Some("INCLUDETEXT"),
        "INFO" => Some("INFO"),
        "KEYWORDS" => Some("KEYWORDS"),
        "LASTSAVEDBY" => Some("LASTSAVEDBY"),
        "LINK" => Some("LINK"),
        "LISTNUM" => Some("LISTNUM"),
        "MACROBUTTON" => Some("MACROBUTTON"),
        "MANAGER" => Some("MANAGER"),
        "MERGEFIELD" => Some("MERGEFIELD"),
        "MERGEBARCODE" => Some("MERGEBARCODE"),
        "MERGEREC" => Some("MERGEREC"),
        "MERGESEQ" => Some("MERGESEQ"),
        "NEXT" => Some("NEXT"),
        "NEXTIF" => Some("NEXTIF"),
        "PRINTDATE" => Some("PRINTDATE"),
        "PRINT" => Some("PRINT"),
        "PRIVATE" => Some("PRIVATE"),
        "RD" => Some("RD"),
        "SAVEDATE" => Some("SAVEDATE"),
        "SUBJECT" => Some("SUBJECT"),
        "SYMBOL" => Some("SYMBOL"),
        "SKIPIF" => Some("SKIPIF"),
        "STYLEREF" => Some("STYLEREF"),
        "CATEGORY" => Some("CATEGORY"),
        "TIME" => Some("TIME"),
        "TITLE" => Some("TITLE"),
        "TEMPLATE" => Some("TEMPLATE"),
        "TOA" => Some("TOA"),
        "TOC" => Some("TOC"),
        "USERADDRESS" => Some("USERADDRESS"),
        "USERINITIALS" => Some("USERINITIALS"),
        "USERNAME" => Some("USERNAME"),
        "XE" => Some("XE"),
        _ => None,
    }
}

pub(in crate::rtf::parser) fn document_property_field_name(name: &str) -> Option<DocumentProperty> {
    match name.trim().to_ascii_lowercase().as_str() {
        "title" => Some(DocumentProperty::Title),
        "subject" => Some(DocumentProperty::Subject),
        "author" => Some(DocumentProperty::Author),
        "keywords" => Some(DocumentProperty::Keywords),
        "comments" | "comment" => Some(DocumentProperty::Comments),
        "lastsavedby" | "last saved by" | "last author" | "last modified by" | "operator" => {
            Some(DocumentProperty::Operator)
        }
        "manager" => Some(DocumentProperty::Manager),
        "company" => Some(DocumentProperty::Company),
        "category" => Some(DocumentProperty::Category),
        "hyperlink base" | "hyperlinkbase" => Some(DocumentProperty::HyperlinkBase),
        _ => None,
    }
}

pub(in crate::rtf::parser) fn document_shortcut_property_field_name(
    name: &str,
) -> Option<DocumentProperty> {
    match name {
        "AUTHOR" => Some(DocumentProperty::Author),
        "TITLE" => Some(DocumentProperty::Title),
        "SUBJECT" => Some(DocumentProperty::Subject),
        "KEYWORDS" => Some(DocumentProperty::Keywords),
        "COMMENTS" => Some(DocumentProperty::Comments),
        "LASTSAVEDBY" => Some(DocumentProperty::Operator),
        "MANAGER" => Some(DocumentProperty::Manager),
        "COMPANY" => Some(DocumentProperty::Company),
        "CATEGORY" => Some(DocumentProperty::Category),
        _ => None,
    }
}

pub(in crate::rtf::parser) fn document_timestamp_field_name(
    name: &str,
) -> Option<DocumentTimestampKind> {
    match name {
        "CREATEDATE" => Some(DocumentTimestampKind::Created),
        "SAVEDATE" => Some(DocumentTimestampKind::Saved),
        "PRINTDATE" => Some(DocumentTimestampKind::Printed),
        _ => None,
    }
}

pub(in crate::rtf::parser) fn document_timestamp_property_field_name(
    name: &str,
) -> Option<DocumentTimestampKind> {
    match normalized_document_property_name(name).as_str() {
        "create time" | "creation date" | "creation time" => Some(DocumentTimestampKind::Created),
        "last save time" | "last saved time" | "last save date" | "last saved date" => {
            Some(DocumentTimestampKind::Saved)
        }
        "last printed" | "last print time" | "last printed time" | "last print date"
        | "last printed date" => Some(DocumentTimestampKind::Printed),
        _ => None,
    }
}

pub(in crate::rtf::parser) fn document_stat_property_dynamic_field(
    name: &str,
) -> Option<DynamicField> {
    match normalized_document_property_name(name).as_str() {
        "number of pages" | "pages" => Some(DynamicField::TotalPages),
        "number of words" | "words" => Some(DynamicField::DocumentWords),
        "number of characters" | "characters" => Some(DynamicField::DocumentCharacters),
        "number of characters with spaces" | "characters with spaces" => {
            Some(DynamicField::DocumentCharactersWithSpaces)
        }
        _ => None,
    }
}

pub(in crate::rtf::parser) fn normalized_document_property_name(name: &str) -> String {
    name.trim()
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(in crate::rtf::parser) fn clean_document_property_text(
    text: &str,
    offset: usize,
    limits: &RtfLimits,
) -> Result<Option<String>, ParseError> {
    let text = text.trim().to_string();
    if text.is_empty()
        || text.chars().any(|ch| ch.is_control())
        || contains_internal_marker(&text)
        || contains_active_pdf_marker_text(&text)
    {
        return Ok(None);
    }
    if text.chars().count() > limits.max_text_run_len {
        return Err(ParseError::ResourceLimitExceeded {
            resource: "document property text".to_string(),
            offset,
        });
    }
    Ok(Some(text))
}

pub(in crate::rtf::parser) fn clean_field_bookmark_value(
    text: &str,
    offset: usize,
    limits: &RtfLimits,
) -> Result<Option<String>, ParseError> {
    let text = text.trim().to_string();
    if text.is_empty()
        || text.chars().any(|ch| ch.is_control())
        || contains_internal_marker(&text)
        || contains_active_pdf_marker_text(&text)
    {
        return Ok(None);
    }
    if text.chars().count() > limits.max_text_run_len {
        return Err(ParseError::ResourceLimitExceeded {
            resource: "field bookmark value".to_string(),
            offset,
        });
    }
    Ok(Some(text))
}

pub(in crate::rtf::parser) fn clean_style_name(
    text: &str,
    offset: usize,
    limits: &RtfLimits,
) -> Result<Option<String>, ParseError> {
    let text = text.trim().trim_end_matches(';').trim().to_string();
    if text.is_empty() || text.chars().any(|ch| ch.is_control()) || contains_internal_marker(&text)
    {
        return Ok(None);
    }
    if text.chars().count() > limits.max_text_run_len {
        return Err(ParseError::ResourceLimitExceeded {
            resource: "style name".to_string(),
            offset,
        });
    }
    Ok(Some(text))
}

pub(in crate::rtf::parser) fn normalize_document_timestamp(
    timestamp: DocumentTimestamp,
) -> Option<DocumentTimestamp> {
    let year = timestamp.year?;
    let month = timestamp.month?;
    let day = timestamp.day?;
    let hour = timestamp.hour.unwrap_or(0);
    let minute = timestamp.minute.unwrap_or(0);
    let second = timestamp.second.unwrap_or(0);
    if !(1..=9999).contains(&year)
        || !(1..=12).contains(&month)
        || !(0..=23).contains(&hour)
        || !(0..=59).contains(&minute)
        || !(0..=59).contains(&second)
    {
        return None;
    }
    let max_day = days_in_month(year, month)?;
    if !(1..=max_day).contains(&day) {
        return None;
    }
    Some(DocumentTimestamp {
        year: Some(year),
        month: Some(month),
        day: Some(day),
        hour: Some(hour),
        minute: Some(minute),
        second: Some(second),
    })
}

pub(in crate::rtf::parser) fn days_in_month(year: i32, month: i32) -> Option<i32> {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
        4 | 6 | 9 | 11 => Some(30),
        2 if is_leap_year(year) => Some(29),
        2 => Some(28),
        _ => None,
    }
}

pub(in crate::rtf::parser) fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

pub(in crate::rtf::parser) fn is_non_visible_resultless_field(name: &str) -> bool {
    matches!(
        name,
        "ASK" | "NEXT" | "NEXTIF" | "SKIPIF" | "XE" | "TC" | "TA"
    )
}

pub(in crate::rtf::parser) fn is_active_non_visible_resultless_field(name: &str) -> bool {
    matches!(name, "ADDIN" | "PRINT" | "PRIVATE")
}

pub(in crate::rtf::parser) fn is_non_visible_external_resultless_field(name: &str) -> bool {
    matches!(name, "RD")
}

pub(in crate::rtf::parser) fn is_layout_positioning_resultless_field(name: &str) -> bool {
    matches!(name, "ADVANCE")
}

pub(in crate::rtf::parser) fn is_auto_number_field(name: &str) -> bool {
    matches!(name, "AUTONUM" | "AUTONUMLGL" | "AUTONUMOUT")
}

pub(in crate::rtf::parser) fn passive_word_compatible_number_field_syntax(
    instruction: &str,
    mut result: PassiveFieldResult,
) -> PassiveFieldResult {
    match field_instruction_name(instruction) {
        Some("AUTONUM" | "AUTONUMOUT") => result.text.push('.'),
        Some("AUTONUMLGL") if !field_has_flag_switch(instruction, 'e') => {
            result
                .text
                .push(field_switch_character_value(instruction, 's').unwrap_or('.'));
        }
        Some("LISTNUM") => {
            if let Some(list_number) = field_list_number_instruction(instruction) {
                let (format, prefix, suffix) =
                    passive_word_compatible_list_number_decoration(&list_number);
                if let Ok(value) = result.text.parse::<i32>() {
                    result.text = format_list_counter(value, format);
                }
                result.text = format!("{prefix}{}{suffix}", result.text);
            }
        }
        _ => {}
    }
    result
}

pub(in crate::rtf::parser) fn passive_word_compatible_list_number_counter_name(
    name: &str,
) -> String {
    if name.is_empty()
        || name.eq_ignore_ascii_case("NumberDefault")
        || name.eq_ignore_ascii_case("OutlineDefault")
        || name.eq_ignore_ascii_case("LegalDefault")
    {
        String::new()
    } else {
        name.to_ascii_lowercase()
    }
}

pub(in crate::rtf::parser) fn passive_word_compatible_list_number_decoration(
    instruction: &FieldListNumberInstruction,
) -> (ListNumberFormat, &'static str, &'static str) {
    if instruction.name.is_empty() || instruction.name.eq_ignore_ascii_case("NumberDefault") {
        return match instruction.level {
            1 => (ListNumberFormat::Decimal, "", ")"),
            2 => (ListNumberFormat::LowerLetter, "", ")"),
            3 => (ListNumberFormat::LowerRoman, "", ")"),
            4 => (ListNumberFormat::Decimal, "(", ")"),
            5 => (ListNumberFormat::LowerLetter, "(", ")"),
            6 => (ListNumberFormat::LowerRoman, "(", ")"),
            7 => (ListNumberFormat::Decimal, "", "."),
            8 => (ListNumberFormat::LowerLetter, "", "."),
            9 => (ListNumberFormat::LowerRoman, "", "."),
            _ => (ListNumberFormat::Decimal, "", "."),
        };
    }
    if instruction.name.eq_ignore_ascii_case("OutlineDefault") {
        return match instruction.level {
            1 => (ListNumberFormat::UpperRoman, "", "."),
            2 => (ListNumberFormat::UpperLetter, "", "."),
            3 => (ListNumberFormat::Decimal, "", "."),
            4 => (ListNumberFormat::LowerLetter, "", ")"),
            _ => (ListNumberFormat::Decimal, "", "."),
        };
    }
    if instruction.name.eq_ignore_ascii_case("LegalDefault") {
        return (ListNumberFormat::Decimal, "", ".");
    }
    (ListNumberFormat::Decimal, "", "")
}

pub(in crate::rtf::parser) fn is_external_resultless_field(name: &str) -> bool {
    matches!(
        name,
        "DATABASE"
            | "DDE"
            | "DDEAUTO"
            | "HYPERLINK"
            | "IMPORT"
            | "INCLUDEPICTURE"
            | "INCLUDETEXT"
            | "LINK"
    )
}

pub(in crate::rtf::parser) fn is_environment_resultless_field(name: &str) -> bool {
    matches!(
        name,
        "FILENAME" | "TEMPLATE" | "USERADDRESS" | "USERINITIALS" | "USERNAME"
    )
}

pub(in crate::rtf::parser) fn is_dynamic_clock_resultless_field(name: &str) -> bool {
    matches!(name, "DATE" | "TIME")
}

pub(in crate::rtf::parser) fn is_word_suppressed_resultless_timestamp_stat_field(
    name: &str,
) -> bool {
    document_timestamp_field_name(name).is_some()
        || matches!(name, "EDITTIME" | "REVNUM" | "DATE" | "TIME")
}

pub(in crate::rtf::parser) fn is_prompt_resultless_field(name: &str) -> bool {
    matches!(name, "FILLIN")
}

pub(in crate::rtf::parser) fn is_passive_field_instruction_control(name: &str) -> bool {
    matches!(name, "d" | "e")
}

pub(in crate::rtf::parser) fn is_mail_merge_data_resultless_field(name: &str) -> bool {
    matches!(
        name,
        "ADDRESSBLOCK" | "GREETINGLINE" | "MERGEREC" | "MERGESEQ"
    )
}

pub(in crate::rtf::parser) fn is_drawing_canvas_resultless_field(name: &str) -> bool {
    matches!(name, "SHAPE")
}

pub(in crate::rtf::parser) fn is_word_literal_drawing_side_bearing_field(name: &str) -> bool {
    matches!(name, "QUOTE" | "IF" | "COMPARE" | "MACROBUTTON" | "SHAPE")
}

pub(in crate::rtf::parser) fn is_active_control_resultless_field(name: &str) -> bool {
    matches!(
        name,
        "CONTROL"
            | "HTMLBUTTON"
            | "HTMLCHECKBOX"
            | "HTMLCONTROL"
            | "HTMLHIDDEN"
            | "HTMLINPUT"
            | "HTMLLISTBOX"
            | "HTMLOPTION"
            | "HTMLPASSWORD"
            | "HTMLRADIO"
            | "HTMLRADIOBUTTON"
            | "HTMLSELECT"
            | "HTMLTEXTAREA"
            | "HTMLTEXTBOX"
    )
}

pub(in crate::rtf::parser) fn is_generated_resultless_field(name: &str) -> bool {
    matches!(
        name,
        "BARCODE"
            | "BIBLIOGRAPHY"
            | "CITATION"
            | "DISPLAYBARCODE"
            | "INDEX"
            | "MERGEBARCODE"
            | "TOA"
            | "TOC"
    )
}

pub(in crate::rtf::parser) fn is_building_block_resultless_field(name: &str) -> bool {
    matches!(name, "AUTOTEXT" | "AUTOTEXTLIST")
}

pub(in crate::rtf::parser) fn is_embedded_object_resultless_field(name: &str) -> bool {
    matches!(name, "EMBED")
}

pub(in crate::rtf::parser) fn is_builtin_passive_result_font(font_name: &str) -> bool {
    font_name.eq_ignore_ascii_case("ZapfDingbats")
        || font_name.eq_ignore_ascii_case("Zapf Dingbats")
}

pub(in crate::rtf::parser) fn is_legacy_symbol_font_name(name: &str) -> bool {
    matches!(
        name.trim(),
        "symbol" | "symbol mt" | "symbolmt" | "standard symbols l"
    )
}

pub(in crate::rtf::parser) fn passive_form_text_field_result(
    form_default_text: &str,
) -> Option<PassiveFieldResult> {
    let text = form_default_text.trim_end().to_string();
    if text.is_empty() || text.chars().any(|ch| ch.is_control()) {
        return None;
    }
    Some(PassiveFieldResult {
        dynamic_field: None,
        text,
        font_name: None,
        font_size_half_points: None,
        form_field: true,
    })
}

pub(in crate::rtf::parser) fn passive_form_dropdown_field_result(
    entries: &[String],
    selected_index: i32,
) -> Option<PassiveFieldResult> {
    let selected_index = usize::try_from(selected_index).ok()?;
    let text = entries.get(selected_index)?.trim_end().to_string();
    if text.is_empty() || text.chars().any(|ch| ch.is_control()) {
        return None;
    }
    Some(PassiveFieldResult {
        dynamic_field: None,
        text,
        font_name: None,
        font_size_half_points: None,
        form_field: true,
    })
}

pub(in crate::rtf::parser) fn passive_eq_field_result(
    instruction: &str,
) -> Option<PassiveFieldResult> {
    let segments = passive_eq_field_segments(instruction)?;
    let text = segments
        .iter()
        .map(|segment| segment.text.as_str())
        .collect::<String>();
    Some(PassiveFieldResult {
        dynamic_field: None,
        text,
        font_name: None,
        font_size_half_points: None,
        form_field: false,
    })
}

pub(in crate::rtf::parser) fn passive_word_compatible_eq_field_result(
    instruction: &str,
) -> Option<PassiveFieldResult> {
    let rest = field_rest_after_name(instruction)?.trim_start();
    if rest.starts_with("\\f") || rest.starts_with("\\r") {
        return passive_eq_field_result(instruction);
    }

    let (left, right, remainder) = field_parenthesized_pair(rest)?;
    if !remainder.trim().is_empty()
        || !is_safe_passive_eq_component(&left)
        || !is_safe_passive_eq_component(&right)
    {
        return None;
    }

    Some(PassiveFieldResult {
        dynamic_field: None,
        text: rest.trim().to_string(),
        font_name: None,
        font_size_half_points: None,
        form_field: false,
    })
}

pub(in crate::rtf::parser) fn passive_eq_field_segments(
    instruction: &str,
) -> Option<Vec<PassiveFieldSegment>> {
    let mut rest = field_rest_after_name(instruction)?.trim_start();
    if let Some(stripped) = rest.strip_prefix("\\f") {
        rest = stripped.trim_start();
        let (numerator, denominator, remainder) = field_parenthesized_pair(rest)?;
        if !remainder.trim().is_empty() {
            return None;
        }
        if !is_safe_passive_eq_component(&numerator) || !is_safe_passive_eq_component(&denominator)
        {
            return None;
        }
        return Some(passive_eq_fraction_segments(numerator, denominator));
    }
    if let Some(stripped) = rest.strip_prefix("\\r") {
        rest = stripped.trim_start();
        let (degree, radicand, remainder) = field_parenthesized_optional_pair(rest)?;
        if !remainder.trim().is_empty() {
            return None;
        }
        if !is_safe_passive_eq_component(&radicand)
            || degree
                .as_ref()
                .is_some_and(|degree| !is_safe_passive_eq_component(degree))
        {
            return None;
        }
        return Some(passive_eq_root_segments(degree, radicand));
    }
    let (numerator, denominator, remainder) = field_parenthesized_pair(rest)?;
    if !remainder.trim().is_empty() {
        return None;
    }
    if !is_safe_passive_eq_component(&numerator) || !is_safe_passive_eq_component(&denominator) {
        return None;
    }
    Some(passive_eq_fraction_segments(numerator, denominator))
}

pub(in crate::rtf::parser) fn passive_eq_fraction_segments(
    numerator: String,
    denominator: String,
) -> Vec<PassiveFieldSegment> {
    vec![
        PassiveFieldSegment {
            text: numerator,
            baseline_shift_half_points: Some(DEFAULT_SUPERSCRIPT_SHIFT_HALF_POINTS),
            font_size_scale_percent: Some(DEFAULT_SCRIPT_FONT_SCALE_PERCENT),
            overline: false,
        },
        PassiveFieldSegment {
            text: "\u{2044}".to_string(),
            baseline_shift_half_points: None,
            font_size_scale_percent: None,
            overline: false,
        },
        PassiveFieldSegment {
            text: denominator,
            baseline_shift_half_points: Some(DEFAULT_SUBSCRIPT_SHIFT_HALF_POINTS),
            font_size_scale_percent: Some(DEFAULT_SCRIPT_FONT_SCALE_PERCENT),
            overline: false,
        },
    ]
}

pub(in crate::rtf::parser) fn passive_eq_root_segments(
    degree: Option<String>,
    radicand: String,
) -> Vec<PassiveFieldSegment> {
    let mut segments = Vec::new();
    if let Some(degree) = degree {
        segments.push(PassiveFieldSegment {
            text: degree,
            baseline_shift_half_points: Some(DEFAULT_SUPERSCRIPT_SHIFT_HALF_POINTS),
            font_size_scale_percent: Some(DEFAULT_SCRIPT_FONT_SCALE_PERCENT),
            overline: false,
        });
    }
    segments.push(PassiveFieldSegment {
        text: "\u{221a}".to_string(),
        baseline_shift_half_points: None,
        font_size_scale_percent: None,
        overline: false,
    });
    segments.push(PassiveFieldSegment {
        text: radicand,
        baseline_shift_half_points: None,
        font_size_scale_percent: None,
        overline: true,
    });
    segments
}

pub(in crate::rtf::parser) fn passive_if_field_result(
    instruction: &str,
) -> Option<PassiveFieldResult> {
    passive_if_field_result_with_resolver(instruction, &|_| None)
}

pub(in crate::rtf::parser) fn passive_if_field_result_with_resolver(
    instruction: &str,
    identifier_resolver: &dyn Fn(&str) -> Option<i64>,
) -> Option<PassiveFieldResult> {
    let rest = field_rest_after_name(instruction)?.trim_start();
    let (left, rest) = field_if_operand(rest)?;
    let (operator, rest) = field_if_operator(rest.trim_start())?;
    let (right, rest) = field_if_operand(rest.trim_start())?;
    let (true_text, rest) = field_if_result_text(rest.trim_start())?;
    let (false_text, rest) = field_if_optional_result_text(rest.trim_start())?;
    if !field_remainder_contains_only_passive_format_switches(rest) {
        return None;
    }

    let left = passive_compare_resolved_operand(left, identifier_resolver);
    let right = passive_compare_resolved_operand(right, identifier_resolver);
    let text = if field_if_condition_matches(&left, operator, &right) {
        true_text
    } else {
        false_text
    };
    if text.chars().count() > 1024
        || text.chars().any(|ch| ch.is_control())
        || contains_internal_marker(&text)
        || contains_active_pdf_marker_text(&text)
    {
        return None;
    }
    Some(PassiveFieldResult {
        dynamic_field: None,
        text,
        font_name: None,
        font_size_half_points: None,
        form_field: false,
    })
}

pub(in crate::rtf::parser) fn passive_compare_field_result(
    instruction: &str,
) -> Option<PassiveFieldResult> {
    passive_compare_field_result_with_resolver(instruction, &|_| None)
}

pub(in crate::rtf::parser) fn passive_compare_field_result_with_resolver(
    instruction: &str,
    identifier_resolver: &dyn Fn(&str) -> Option<i64>,
) -> Option<PassiveFieldResult> {
    let rest = field_rest_after_name(instruction)?.trim_start();
    let (left, rest) = field_if_operand(rest)?;
    let (operator, rest) = field_if_operator(rest.trim_start())?;
    let (right, rest) = field_if_operand(rest.trim_start())?;
    if !field_remainder_contains_only_passive_format_switches(rest.trim_start()) {
        return None;
    }

    let left = passive_compare_resolved_operand(left, identifier_resolver);
    let right = passive_compare_resolved_operand(right, identifier_resolver);
    Some(PassiveFieldResult {
        dynamic_field: None,
        text: if field_if_condition_matches(&left, operator, &right) {
            "1"
        } else {
            "0"
        }
        .to_string(),
        font_name: None,
        font_size_half_points: None,
        form_field: false,
    })
}

pub(in crate::rtf::parser) fn passive_compare_resolved_operand(
    operand: String,
    identifier_resolver: &dyn Fn(&str) -> Option<i64>,
) -> String {
    if operand
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic())
        && operand
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        && let Some(value) = identifier_resolver(&operand)
    {
        return value.to_string();
    }
    operand
}

pub(in crate::rtf::parser) fn passive_quote_field_result(
    instruction: &str,
) -> Option<PassiveFieldResult> {
    let text = field_first_quoted_argument(instruction)?;
    if text.chars().any(|ch| ch.is_control()) || contains_active_pdf_marker_text(&text) {
        return None;
    }
    Some(PassiveFieldResult {
        dynamic_field: None,
        text,
        font_name: None,
        font_size_half_points: None,
        form_field: false,
    })
}

pub(in crate::rtf::parser) fn passive_fillin_field_result(
    instruction: &str,
) -> Option<PassiveFieldResult> {
    let text = field_switch_string_value(instruction, b'd')
        .or_else(|| field_second_quoted_argument(instruction))?;
    if contains_active_pdf_marker_text(&text) {
        return None;
    }
    Some(PassiveFieldResult {
        dynamic_field: None,
        text,
        font_name: None,
        font_size_half_points: None,
        form_field: false,
    })
}

pub(in crate::rtf::parser) fn passive_greetingline_field_result(
    instruction: &str,
) -> Option<PassiveFieldResult> {
    if field_instruction_name(instruction) != Some("GREETINGLINE") {
        return None;
    }
    let text = field_switch_string_value(instruction, b'e')
        .or_else(|| field_last_quoted_argument_after_name(instruction))?;
    if contains_active_pdf_marker_text(&text) {
        return None;
    }
    Some(PassiveFieldResult {
        dynamic_field: None,
        text,
        font_name: None,
        font_size_half_points: None,
        form_field: false,
    })
}

pub(in crate::rtf::parser) fn field_second_quoted_argument(instruction: &str) -> Option<String> {
    let rest = field_rest_after_name(instruction)?;
    let rest = skip_field_argument(rest)?;
    let rest = rest.trim_start();
    field_quoted_prefix(rest)
}

pub(in crate::rtf::parser) fn field_last_quoted_argument_after_name(
    instruction: &str,
) -> Option<String> {
    let mut rest = field_rest_after_name(instruction)?.trim_start();
    let mut last = None;
    while !rest.is_empty() {
        if rest.starts_with('"') {
            last = Some(field_quoted_prefix(rest)?);
        }
        let next = skip_field_argument(rest)?;
        if next.len() == rest.len() {
            break;
        }
        rest = next.trim_start();
    }
    last
}

pub(in crate::rtf::parser) fn contains_active_pdf_marker_text(text: &str) -> bool {
    [
        "/JavaScript",
        "/EmbeddedFile",
        "/EmbeddedFiles",
        "/Filespec",
        "/Launch",
        "/OpenAction",
        "/RichMedia",
        "/AA",
        "/Action",
        "/AcroForm",
        "/Annots",
        "/Widget",
        "/URI",
        "/Sig",
        "/SigFlags",
        "/UseAttachments",
        "/DocMDP",
        "/FieldMDP",
        "/UR3",
        "/Cert",
        "/DSS",
        "/VRI",
        "/FDF",
        "/NeedAppearances",
        "/XDP",
        "/XFA",
    ]
    .iter()
    .any(|marker| text.contains(marker))
}

pub(in crate::rtf::parser) fn passive_macrobutton_field_result(
    instruction: &str,
) -> Option<PassiveFieldResult> {
    let rest = field_rest_after_name(instruction)?;
    let rest = skip_field_argument(rest)?;
    let display = rest.trim_start();
    if display.is_empty() {
        return None;
    }

    let text = if display.starts_with('"') {
        field_quoted_prefix(display)?
    } else {
        display.trim_end().to_string()
    };
    if text.is_empty()
        || text.chars().any(|ch| ch.is_control())
        || contains_active_pdf_marker_text(&text)
    {
        return None;
    }

    Some(PassiveFieldResult {
        dynamic_field: None,
        text,
        font_name: None,
        font_size_half_points: None,
        form_field: false,
    })
}

pub(in crate::rtf::parser) fn passive_gotobutton_field_result(
    instruction: &str,
) -> Option<PassiveFieldResult> {
    passive_gotobutton_field_result_with_quote_mode(instruction, false)
}

pub(in crate::rtf::parser) fn passive_word_compatible_gotobutton_field_result(
    instruction: &str,
) -> Option<PassiveFieldResult> {
    passive_gotobutton_field_result_with_quote_mode(instruction, true)
}

pub(in crate::rtf::parser) fn passive_gotobutton_field_result_with_quote_mode(
    instruction: &str,
    preserve_quoted_display: bool,
) -> Option<PassiveFieldResult> {
    let rest = field_rest_after_name(instruction)?;
    let rest = skip_field_argument(rest)?;
    let display = rest.trim_start();
    if display.is_empty() {
        return None;
    }

    let text = if display.starts_with('"') {
        let text = field_quoted_prefix(display)?;
        if preserve_quoted_display {
            format!("\"{text}\"")
        } else {
            text
        }
    } else {
        display.trim_end().to_string()
    };
    if text.is_empty()
        || text.chars().count() > MAX_PASSIVE_FIELD_FORMAT_TEXT_CHARS
        || text.chars().any(|ch| ch.is_control())
        || contains_internal_marker(&text)
        || contains_active_pdf_marker_text(&text)
    {
        return None;
    }

    Some(PassiveFieldResult {
        dynamic_field: None,
        text,
        font_name: None,
        font_size_half_points: None,
        form_field: false,
    })
}

pub(in crate::rtf::parser) fn passive_mergefield_result(
    instruction: &str,
) -> Option<PassiveFieldResult> {
    let name = field_first_argument(instruction)?;
    if name.is_empty()
        || name.chars().any(|ch| ch.is_control())
        || contains_internal_marker(&name)
        || contains_active_pdf_marker_text(&name)
    {
        return None;
    }

    Some(PassiveFieldResult {
        dynamic_field: None,
        text: format!("\u{00ab}{name}\u{00bb}"),
        font_name: None,
        font_size_half_points: None,
        form_field: false,
    })
}

pub(in crate::rtf::parser) fn passive_symbol_field_result(
    instruction: &str,
) -> Option<PassiveFieldResult> {
    let mut chars = instruction.trim_start().chars().peekable();
    while chars.next_if(|ch| ch.is_ascii_alphabetic()).is_some() {}
    while chars.next_if(|ch| ch.is_whitespace()).is_some() {}

    let negative = chars.next_if_eq(&'-').is_some();
    if negative {
        return None;
    }

    let mut value: u32 = 0;
    let mut digit_seen = false;
    while let Some(ch) = chars.peek().copied() {
        let Some(digit) = ch.to_digit(10) else {
            break;
        };
        digit_seen = true;
        value = value.checked_mul(10)?.checked_add(digit)?;
        chars.next();
    }
    if !digit_seen {
        return None;
    }

    let rest_after_value = chars.collect::<String>();
    let font_size_half_points = field_switch_i32_value(rest_after_value.as_str(), 's')
        .and_then(|points| points.checked_mul(2))
        .filter(|half_points| *half_points > 0);
    let font_name = field_switch_string_value(instruction, b'f')
        .filter(|name| !contains_active_pdf_marker_text(name));
    let passive_font_name = font_name.clone();
    let text = if let Some(name) = font_name.as_deref()
        && name.eq_ignore_ascii_case("Symbol")
        && value <= u8::MAX as u32
    {
        map_symbol_char(value as u8 as char).to_string()
    } else if let Some(name) = font_name.as_deref()
        && let Some(mapped) = map_dingbats_codepoint(&name.to_ascii_lowercase(), value)
    {
        mapped.to_string()
    } else {
        let ch = char::from_u32(value)?;
        let mut buffer = [0; 4];
        let text = ch.encode_utf8(&mut buffer);
        if ch.is_control() || contains_internal_marker(text) {
            return None;
        }
        ch.to_string()
    };

    Some(PassiveFieldResult {
        dynamic_field: None,
        text,
        font_name: passive_font_name,
        font_size_half_points,
        form_field: false,
    })
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(in crate::rtf::parser) enum FieldIfOperator {
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

pub(in crate::rtf::parser) fn field_if_operand(input: &str) -> Option<(String, &str)> {
    let input = input.trim_start();
    if input.is_empty() || input.starts_with('\\') {
        return None;
    }
    let (text, rest) = if input.starts_with('"') {
        (field_quoted_prefix(input)?, skip_field_argument(input)?)
    } else {
        let end = input
            .char_indices()
            .find_map(|(index, ch)| ch.is_whitespace().then_some(index))
            .unwrap_or(input.len());
        (input[..end].to_string(), &input[end..])
    };
    let text = text.trim().to_string();
    if text.is_empty() || text.chars().any(|ch| ch.is_control()) || contains_internal_marker(&text)
    {
        return None;
    }
    Some((text, rest))
}

pub(in crate::rtf::parser) fn field_parenthesized_pair(
    input: &str,
) -> Option<(String, String, &str)> {
    let input = input.trim_start();
    let rest = input.strip_prefix('(')?;
    let mut depth = 0usize;
    let mut comma_index = None;
    for (index, ch) in rest.char_indices() {
        match ch {
            '(' => depth = depth.checked_add(1)?,
            ')' if depth == 0 => {
                let comma_index = comma_index?;
                let left = rest[..comma_index].trim().to_string();
                let right = rest[comma_index + 1..index].trim().to_string();
                return Some((left, right, &rest[index + ch.len_utf8()..]));
            }
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 && comma_index.is_none() => comma_index = Some(index),
            _ => {}
        }
    }
    None
}

pub(in crate::rtf::parser) fn field_parenthesized_optional_pair(
    input: &str,
) -> Option<(Option<String>, String, &str)> {
    let input = input.trim_start();
    let rest = input.strip_prefix('(')?;
    let mut depth = 0usize;
    let mut comma_index = None;
    for (index, ch) in rest.char_indices() {
        match ch {
            '(' => depth = depth.checked_add(1)?,
            ')' if depth == 0 => {
                if let Some(comma_index) = comma_index {
                    let left = rest[..comma_index].trim().to_string();
                    let right = rest[comma_index + 1..index].trim().to_string();
                    return Some((Some(left), right, &rest[index + ch.len_utf8()..]));
                }
                let value = rest[..index].trim().to_string();
                return Some((None, value, &rest[index + ch.len_utf8()..]));
            }
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 && comma_index.is_none() => comma_index = Some(index),
            _ => {}
        }
    }
    None
}

pub(in crate::rtf::parser) fn is_safe_passive_eq_component(text: &str) -> bool {
    !text.is_empty()
        && text.chars().count() <= MAX_PASSIVE_FIELD_FORMAT_TEXT_CHARS
        && !text.chars().any(|ch| ch.is_control())
        && !contains_internal_marker(text)
        && !contains_active_pdf_marker_text(text)
        && !text.contains('\\')
        && !text.contains('{')
        && !text.contains('}')
}

pub(in crate::rtf::parser) fn field_if_operator(input: &str) -> Option<(FieldIfOperator, &str)> {
    for (token, operator) in [
        ("<>", FieldIfOperator::NotEqual),
        ("<=", FieldIfOperator::LessOrEqual),
        (">=", FieldIfOperator::GreaterOrEqual),
        ("=", FieldIfOperator::Equal),
        ("<", FieldIfOperator::Less),
        (">", FieldIfOperator::Greater),
    ] {
        if let Some(rest) = input.strip_prefix(token) {
            return Some((operator, rest));
        }
    }
    None
}

pub(in crate::rtf::parser) fn field_if_result_text(input: &str) -> Option<(String, &str)> {
    let input = input.trim_start();
    if !input.starts_with('"') {
        return None;
    }
    let text = field_quoted_prefix(input)?;
    let rest = skip_field_argument(input)?;
    Some((text, rest))
}

pub(in crate::rtf::parser) fn field_if_optional_result_text(input: &str) -> Option<(String, &str)> {
    let input = input.trim_start();
    if input.is_empty() {
        return Some((String::new(), input));
    }
    field_if_result_text(input)
}

pub(in crate::rtf::parser) fn field_if_condition_matches(
    left: &str,
    operator: FieldIfOperator,
    right: &str,
) -> bool {
    if let (Ok(left), Ok(right)) = (left.parse::<i64>(), right.parse::<i64>()) {
        return match operator {
            FieldIfOperator::Equal => left == right,
            FieldIfOperator::NotEqual => left != right,
            FieldIfOperator::Less => left < right,
            FieldIfOperator::LessOrEqual => left <= right,
            FieldIfOperator::Greater => left > right,
            FieldIfOperator::GreaterOrEqual => left >= right,
        };
    }

    match operator {
        FieldIfOperator::Equal => left == right,
        FieldIfOperator::NotEqual => left != right,
        FieldIfOperator::Less => left < right,
        FieldIfOperator::LessOrEqual => left <= right,
        FieldIfOperator::Greater => left > right,
        FieldIfOperator::GreaterOrEqual => left >= right,
    }
}

pub(in crate::rtf::parser) fn field_numeric_picture_switch(
    instruction: &str,
) -> Option<Option<String>> {
    let mut in_quote = false;
    let mut escaped = false;

    for (index, ch) in instruction.char_indices() {
        if in_quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_quote = false;
            }
            continue;
        }

        if ch == '"' {
            in_quote = true;
            continue;
        }

        if ch == '\\' {
            let after_backslash = index + ch.len_utf8();
            if instruction[after_backslash..].starts_with('#') {
                let after_hash = after_backslash + '#'.len_utf8();
                return field_numeric_picture_argument(&instruction[after_hash..]).map(Some);
            }
        }
    }

    Some(None)
}

pub(in crate::rtf::parser) fn field_numeric_picture_argument(input: &str) -> Option<String> {
    field_numeric_picture_argument_with_rest(input).map(|(picture, _)| picture)
}

pub(in crate::rtf::parser) fn field_numeric_picture_argument_with_rest(
    input: &str,
) -> Option<(String, &str)> {
    let input = input.trim_start();
    if input.is_empty() {
        return None;
    }

    let (picture, rest) = if input.starts_with('"') {
        (field_quoted_prefix(input)?, skip_field_argument(input)?)
    } else {
        let end = input
            .char_indices()
            .find_map(|(index, ch)| (ch.is_whitespace() || ch == '\\').then_some(index))
            .unwrap_or(input.len());
        (input[..end].to_string(), &input[end..])
    };
    let picture = picture.trim().to_string();
    if picture.is_empty()
        || picture.chars().count() > MAX_PASSIVE_FIELD_NUMERIC_PICTURE_CHARS
        || picture.chars().any(|ch| ch.is_control())
        || contains_internal_marker(&picture)
    {
        return None;
    }
    Some((picture, rest))
}

pub(in crate::rtf::parser) fn apply_field_numeric_picture_switch(
    text: &str,
    picture: &str,
) -> Option<String> {
    let value = text.trim().parse::<i64>().ok()?;
    let parsed = parse_simple_numeric_picture(picture)?;
    if parsed.decimal_places > 8 || parsed.min_integer_digits > 18 {
        return None;
    }

    let negative = value < 0;
    let magnitude = value.checked_abs()?;
    let mut integer = magnitude.to_string();
    if integer.len() < parsed.min_integer_digits {
        let mut padded = String::with_capacity(parsed.min_integer_digits);
        for _ in 0..parsed.min_integer_digits - integer.len() {
            padded.push('0');
        }
        padded.push_str(&integer);
        integer = padded;
    }
    if parsed.group_thousands {
        integer = group_decimal_digits(&integer);
    }

    let mut output = String::new();
    if negative {
        output.push('-');
    }
    output.push_str(&parsed.prefix);
    output.push_str(&integer);
    if parsed.decimal_places > 0 {
        output.push('.');
        for _ in 0..parsed.decimal_places {
            output.push('0');
        }
    }
    output.push_str(&parsed.suffix);
    Some(output)
}

pub(in crate::rtf::parser) fn field_date_picture_switch(
    instruction: &str,
) -> Option<Option<String>> {
    let mut in_quote = false;
    let mut escaped = false;

    for (index, ch) in instruction.char_indices() {
        if in_quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_quote = false;
            }
            continue;
        }

        if ch == '"' {
            in_quote = true;
            continue;
        }

        if ch == '\\' {
            let after_backslash = index + ch.len_utf8();
            if instruction[after_backslash..].starts_with('@') {
                let after_at = after_backslash + '@'.len_utf8();
                return field_date_picture_argument(&instruction[after_at..]).map(Some);
            }
        }
    }

    Some(None)
}

pub(in crate::rtf::parser) fn field_date_picture_argument(input: &str) -> Option<String> {
    let input = input.trim_start();
    if input.is_empty() {
        return None;
    }

    let picture = if input.starts_with('"') {
        field_quoted_prefix(input)?
    } else {
        let end = input
            .char_indices()
            .find_map(|(index, ch)| (ch.is_whitespace() || ch == '\\').then_some(index))
            .unwrap_or(input.len());
        input[..end].to_string()
    };
    let picture = picture.trim().to_string();
    if picture.is_empty()
        || picture.chars().count() > MAX_PASSIVE_FIELD_DATE_PICTURE_CHARS
        || picture.chars().any(|ch| ch.is_control())
        || contains_internal_marker(&picture)
        || !picture
            .chars()
            .all(|ch| ch.is_ascii() && !matches!(ch, '\\' | '{' | '}'))
    {
        return None;
    }
    Some(picture)
}

pub(in crate::rtf::parser) fn format_default_document_timestamp(
    timestamp: &DocumentTimestamp,
) -> String {
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        timestamp.year.unwrap_or(1),
        timestamp.month.unwrap_or(1),
        timestamp.day.unwrap_or(1),
        timestamp.hour.unwrap_or(0),
        timestamp.minute.unwrap_or(0),
        timestamp.second.unwrap_or(0)
    )
}

pub(in crate::rtf::parser) fn apply_field_date_picture(
    timestamp: &DocumentTimestamp,
    picture: &str,
) -> Option<String> {
    let mut output = String::new();
    let mut index = 0;
    while index < picture.len() {
        let rest = &picture[index..];
        if rest.starts_with('\'') {
            let literal = single_quoted_date_picture_literal(rest)?;
            output.push_str(literal.text);
            index += literal.bytes;
        } else if let Some(value) = rest.strip_prefix("A/P") {
            output.push(if timestamp.hour.unwrap_or(0) < 12 {
                'A'
            } else {
                'P'
            });
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix("a/p") {
            output.push(if timestamp.hour.unwrap_or(0) < 12 {
                'a'
            } else {
                'p'
            });
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix("AM/PM") {
            output.push_str(if timestamp.hour.unwrap_or(0) < 12 {
                "AM"
            } else {
                "PM"
            });
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix("am/pm") {
            output.push_str(if timestamp.hour.unwrap_or(0) < 12 {
                "am"
            } else {
                "pm"
            });
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix("yyyy") {
            output.push_str(&format!("{:04}", timestamp.year.unwrap_or(1)));
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix("yy") {
            output.push_str(&format!("{:02}", timestamp.year.unwrap_or(1) % 100));
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix("MMMM") {
            output.push_str(month_name(timestamp.month.unwrap_or(1))?);
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix("MMM") {
            output.push_str(month_abbreviation(timestamp.month.unwrap_or(1))?);
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix("MM") {
            output.push_str(&format!("{:02}", timestamp.month.unwrap_or(1)));
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix('M') {
            output.push_str(&timestamp.month.unwrap_or(1).to_string());
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix("dddd") {
            output.push_str(weekday_name(timestamp)?);
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix("ddd") {
            output.push_str(weekday_abbreviation(timestamp)?);
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix("dd") {
            output.push_str(&format!("{:02}", timestamp.day.unwrap_or(1)));
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix('d') {
            output.push_str(&timestamp.day.unwrap_or(1).to_string());
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix("HH") {
            output.push_str(&format!("{:02}", timestamp.hour.unwrap_or(0)));
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix('H') {
            output.push_str(&timestamp.hour.unwrap_or(0).to_string());
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix("hh") {
            output.push_str(&format!("{:02}", twelve_hour(timestamp.hour.unwrap_or(0))));
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix('h') {
            output.push_str(&twelve_hour(timestamp.hour.unwrap_or(0)).to_string());
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix("mm") {
            output.push_str(&format!("{:02}", timestamp.minute.unwrap_or(0)));
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix('m') {
            output.push_str(&timestamp.minute.unwrap_or(0).to_string());
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix("ss") {
            output.push_str(&format!("{:02}", timestamp.second.unwrap_or(0)));
            index = picture.len() - value.len();
        } else if let Some(value) = rest.strip_prefix('s') {
            output.push_str(&timestamp.second.unwrap_or(0).to_string());
            index = picture.len() - value.len();
        } else {
            let ch = rest.chars().next()?;
            if ch.is_ascii_alphabetic() {
                return None;
            }
            output.push(ch);
            index += ch.len_utf8();
        }
        if output.chars().count() > MAX_PASSIVE_FIELD_FORMAT_TEXT_CHARS {
            return None;
        }
    }
    Some(output)
}

pub(in crate::rtf::parser) struct DatePictureLiteral<'a> {
    pub(in crate::rtf::parser) text: &'a str,
    pub(in crate::rtf::parser) bytes: usize,
}

pub(in crate::rtf::parser) fn single_quoted_date_picture_literal(
    input: &str,
) -> Option<DatePictureLiteral<'_>> {
    let body = input.strip_prefix('\'')?;
    let end = body.find('\'')?;
    Some(DatePictureLiteral {
        text: &body[..end],
        bytes: '\''.len_utf8() + end + '\''.len_utf8(),
    })
}

pub(in crate::rtf::parser) fn twelve_hour(hour: i32) -> i32 {
    let hour = hour % 12;
    if hour == 0 { 12 } else { hour }
}

pub(in crate::rtf::parser) fn month_name(month: i32) -> Option<&'static str> {
    Some(match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => return None,
    })
}

pub(in crate::rtf::parser) fn month_abbreviation(month: i32) -> Option<&'static str> {
    Some(match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => return None,
    })
}

pub(in crate::rtf::parser) fn weekday_name(timestamp: &DocumentTimestamp) -> Option<&'static str> {
    Some(match weekday_index(timestamp)? {
        0 => "Sunday",
        1 => "Monday",
        2 => "Tuesday",
        3 => "Wednesday",
        4 => "Thursday",
        5 => "Friday",
        6 => "Saturday",
        _ => return None,
    })
}

pub(in crate::rtf::parser) fn weekday_abbreviation(
    timestamp: &DocumentTimestamp,
) -> Option<&'static str> {
    Some(match weekday_index(timestamp)? {
        0 => "Sun",
        1 => "Mon",
        2 => "Tue",
        3 => "Wed",
        4 => "Thu",
        5 => "Fri",
        6 => "Sat",
        _ => return None,
    })
}

pub(in crate::rtf::parser) fn weekday_index(timestamp: &DocumentTimestamp) -> Option<usize> {
    let year = timestamp.year.unwrap_or(1);
    let month = timestamp.month.unwrap_or(1);
    let day = timestamp.day.unwrap_or(1);
    if year < 1 || !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month)? {
        return None;
    }

    let month_offsets = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let mut adjusted_year = year;
    if month < 3 {
        adjusted_year -= 1;
    }
    let weekday = adjusted_year + adjusted_year / 4 - adjusted_year / 100
        + adjusted_year / 400
        + month_offsets[(month - 1) as usize]
        + day;
    Some(weekday.rem_euclid(7) as usize)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::rtf::parser) struct SimpleNumericPicture {
    pub(in crate::rtf::parser) prefix: String,
    pub(in crate::rtf::parser) suffix: String,
    pub(in crate::rtf::parser) min_integer_digits: usize,
    pub(in crate::rtf::parser) group_thousands: bool,
    pub(in crate::rtf::parser) decimal_places: usize,
}

pub(in crate::rtf::parser) fn parse_simple_numeric_picture(
    picture: &str,
) -> Option<SimpleNumericPicture> {
    if picture.contains(';')
        || picture.contains('%')
        || picture.contains('E')
        || picture.contains('e')
        || picture.contains('x')
        || picture.contains('X')
    {
        return None;
    }

    let first_digit = picture
        .char_indices()
        .find_map(|(index, ch)| matches!(ch, '0' | '#').then_some(index))?;
    let last_digit = picture
        .char_indices()
        .filter_map(|(index, ch)| matches!(ch, '0' | '#').then_some(index))
        .next_back()?;
    let suffix_start = last_digit + picture[last_digit..].chars().next()?.len_utf8();
    let prefix = picture[..first_digit].to_string();
    let suffix = picture[suffix_start..].to_string();
    if !is_safe_numeric_picture_literal(&prefix) || !is_safe_numeric_picture_literal(&suffix) {
        return None;
    }

    let core = &picture[first_digit..suffix_start];
    if core.chars().any(|ch| !matches!(ch, '0' | '#' | ',' | '.')) {
        return None;
    }
    if core.matches('.').count() > 1 {
        return None;
    }

    let (integer_pattern, decimal_pattern) = core.split_once('.').unwrap_or((core, ""));
    if integer_pattern.is_empty()
        || integer_pattern.chars().filter(|ch| *ch == '0').count() == 0
        || integer_pattern
            .chars()
            .rev()
            .skip_while(|ch| *ch == ',')
            .next()
            .is_none()
    {
        return None;
    }
    if decimal_pattern.chars().any(|ch| ch != '0') {
        return None;
    }

    let min_integer_digits = integer_pattern.chars().filter(|ch| *ch == '0').count();
    Some(SimpleNumericPicture {
        prefix,
        suffix,
        min_integer_digits,
        group_thousands: integer_pattern.contains(','),
        decimal_places: decimal_pattern.len(),
    })
}

pub(in crate::rtf::parser) fn is_safe_numeric_picture_literal(text: &str) -> bool {
    text.chars()
        .all(|ch| ch.is_ascii() && !ch.is_ascii_control() && !matches!(ch, '\\' | '{' | '}'))
}

pub(in crate::rtf::parser) fn group_decimal_digits(digits: &str) -> String {
    let mut output = String::new();
    let first_group_len = digits.len() % 3;
    for (index, ch) in digits.chars().enumerate() {
        if index > 0
            && (index == first_group_len
                || (index > first_group_len && (index - first_group_len) % 3 == 0))
        {
            output.push(',');
        }
        output.push(ch);
    }
    output
}

pub(in crate::rtf::parser) fn field_format_switches(
    instruction: &str,
) -> Option<Vec<FieldFormatSwitch>> {
    let mut switches = Vec::new();
    let mut in_quote = false;
    let mut escaped = false;

    for (index, ch) in instruction.char_indices() {
        if in_quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_quote = false;
            }
            continue;
        }

        if ch == '"' {
            in_quote = true;
            continue;
        }

        if ch == '\\' {
            let after_backslash = index + ch.len_utf8();
            if !instruction[after_backslash..].starts_with('*') {
                continue;
            }
            let after_star = after_backslash + '*'.len_utf8();
            if let Some((switch, _)) = field_format_switch_after_star(&instruction[after_star..]) {
                if switches.len() >= 16 {
                    return None;
                }
                switches.push(switch);
            }
        }
    }

    Some(switches)
}

pub(in crate::rtf::parser) fn field_remainder_contains_only_passive_format_switches(
    input: &str,
) -> bool {
    let mut rest = input.trim_start();
    while !rest.is_empty() {
        let Some(after_backslash) = rest.strip_prefix('\\') else {
            return false;
        };
        if let Some(after_star) = after_backslash.strip_prefix('*') {
            let Some((_, after_switch)) = field_format_switch_after_star(after_star) else {
                return false;
            };
            rest = after_switch.trim_start();
        } else if let Some(after_hash) = after_backslash.strip_prefix('#') {
            let Some((_, after_picture)) = field_numeric_picture_argument_with_rest(after_hash)
            else {
                return false;
            };
            rest = after_picture.trim_start();
        } else {
            return false;
        }
    }
    true
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(in crate::rtf::parser) enum StyleRefNumberMode {
    Rendered,
    RelativeContext,
    FullContext,
}

pub(in crate::rtf::parser) fn field_remainder_contains_only_style_ref_switches(
    input: &str,
) -> bool {
    let mut rest = input.trim_start();
    while !rest.is_empty() {
        let Some(after_backslash) = rest.strip_prefix('\\') else {
            return false;
        };
        if let Some(after_star) = after_backslash.strip_prefix('*') {
            let Some((_, after_switch)) = field_format_switch_after_star(after_star) else {
                return false;
            };
            rest = after_switch.trim_start();
        } else if let Some(after_hash) = after_backslash.strip_prefix('#') {
            let Some((_, after_picture)) = field_numeric_picture_argument_with_rest(after_hash)
            else {
                return false;
            };
            rest = after_picture.trim_start();
        } else if let Some(after_number) = strip_field_switch_letter(after_backslash, 'n') {
            rest = after_number.trim_start();
        } else if let Some(after_relative) = strip_field_switch_letter(after_backslash, 'r') {
            rest = after_relative.trim_start();
        } else if let Some(after_full) = strip_field_switch_letter(after_backslash, 'w') {
            rest = after_full.trim_start();
        } else {
            return false;
        }
    }
    true
}

pub(in crate::rtf::parser) fn field_style_ref_number_mode(
    instruction: &str,
) -> Option<Option<StyleRefNumberMode>> {
    let mut rest = field_rest_after_first_argument(instruction)?.trim_start();
    let mut mode = None;
    while !rest.is_empty() {
        let after_backslash = rest.strip_prefix('\\')?;
        if let Some(after_star) = after_backslash.strip_prefix('*') {
            let (_, after_switch) = field_format_switch_after_star(after_star)?;
            rest = after_switch.trim_start();
        } else if let Some(after_hash) = after_backslash.strip_prefix('#') {
            let (_, after_picture) = field_numeric_picture_argument_with_rest(after_hash)?;
            rest = after_picture.trim_start();
        } else if let Some(after_number) = strip_field_switch_letter(after_backslash, 'n') {
            mode = Some(StyleRefNumberMode::Rendered);
            rest = after_number.trim_start();
        } else if let Some(after_relative) = strip_field_switch_letter(after_backslash, 'r') {
            mode = Some(StyleRefNumberMode::RelativeContext);
            rest = after_relative.trim_start();
        } else if let Some(after_full) = strip_field_switch_letter(after_backslash, 'w') {
            mode = Some(StyleRefNumberMode::FullContext);
            rest = after_full.trim_start();
        } else {
            return None;
        }
    }
    Some(mode)
}

pub(in crate::rtf::parser) fn strip_field_switch_letter(input: &str, letter: char) -> Option<&str> {
    let after_letter = input.strip_prefix(letter)?;
    if after_letter.chars().next().is_none_or(char::is_whitespace) {
        Some(after_letter)
    } else {
        None
    }
}

pub(in crate::rtf::parser) fn field_format_switch_after_star(
    input: &str,
) -> Option<(FieldFormatSwitch, &str)> {
    let input = input.trim_start();
    let (name, rest) = if input.starts_with('"') {
        (field_quoted_prefix(input)?, skip_field_argument(input)?)
    } else {
        let end = input
            .char_indices()
            .find_map(|(index, ch)| (!ch.is_ascii_alphabetic()).then_some(index))
            .unwrap_or(input.len());
        if end == 0 {
            return None;
        }
        (input[..end].to_string(), &input[end..])
    };

    let name = name.trim();
    let switch = if let Some(switch) = field_text_format_switch(name) {
        FieldFormatSwitch::Text(switch)
    } else if let Some(switch) = field_number_format_switch(name) {
        FieldFormatSwitch::Number(switch)
    } else if is_noop_field_format_switch(name) {
        FieldFormatSwitch::NoOp
    } else {
        return None;
    };
    Some((switch, rest))
}

pub(in crate::rtf::parser) fn is_noop_field_format_switch(name: &str) -> bool {
    matches!(
        name.to_ascii_uppercase().as_str(),
        "MERGEFORMAT" | "CHARFORMAT"
    )
}

pub(in crate::rtf::parser) fn field_text_format_switch(
    name: &str,
) -> Option<FieldTextFormatSwitch> {
    match name.to_ascii_uppercase().as_str() {
        "UPPER" => Some(FieldTextFormatSwitch::Upper),
        "LOWER" => Some(FieldTextFormatSwitch::Lower),
        "FIRSTCAP" => Some(FieldTextFormatSwitch::FirstCap),
        "CAPS" => Some(FieldTextFormatSwitch::Caps),
        _ => None,
    }
}

pub(in crate::rtf::parser) fn field_number_format_switch(
    name: &str,
) -> Option<FieldNumberFormatSwitch> {
    let upper = name.to_ascii_uppercase();
    match upper.as_str() {
        "ARABIC" => Some(FieldNumberFormatSwitch::Arabic),
        "ARABICDASH" => Some(FieldNumberFormatSwitch::ArabicDash),
        "CARDTEXT" => Some(FieldNumberFormatSwitch::CardText),
        "DOLLARTEXT" => Some(FieldNumberFormatSwitch::DollarText),
        "ALPHABETIC" if name.chars().all(|ch| !ch.is_ascii_lowercase()) => {
            Some(FieldNumberFormatSwitch::UpperAlphabetic)
        }
        "ALPHABETIC" => Some(FieldNumberFormatSwitch::LowerAlphabetic),
        "ROMAN" if name.chars().all(|ch| !ch.is_ascii_lowercase()) => {
            Some(FieldNumberFormatSwitch::UpperRoman)
        }
        "ROMAN" => Some(FieldNumberFormatSwitch::LowerRoman),
        "ORDINAL" => Some(FieldNumberFormatSwitch::Ordinal),
        "ORDTEXT" | "ORDINALTEXT" => Some(FieldNumberFormatSwitch::OrdinalText),
        "HEX" => Some(FieldNumberFormatSwitch::Hex),
        _ => None,
    }
}

pub(in crate::rtf::parser) fn apply_field_text_format_switch(
    text: &str,
    switch: FieldTextFormatSwitch,
) -> String {
    match switch {
        FieldTextFormatSwitch::Upper => text.chars().flat_map(char::to_uppercase).collect(),
        FieldTextFormatSwitch::Lower => text.chars().flat_map(char::to_lowercase).collect(),
        FieldTextFormatSwitch::FirstCap => uppercase_first_alphabetic(text),
        FieldTextFormatSwitch::Caps => uppercase_each_word_start(text),
    }
}

pub(in crate::rtf::parser) fn apply_field_number_format_switch(
    text: &str,
    switch: FieldNumberFormatSwitch,
) -> Option<String> {
    let value = text.trim().parse::<i32>().ok()?;
    match switch {
        FieldNumberFormatSwitch::Arabic => Some(value.to_string()),
        FieldNumberFormatSwitch::ArabicDash => Some(format!("- {value} -")),
        FieldNumberFormatSwitch::CardText => format_cardinal_text_counter(value),
        FieldNumberFormatSwitch::DollarText => format_dollar_text_counter(value),
        FieldNumberFormatSwitch::UpperAlphabetic => Some(format_alpha_counter(value, false)),
        FieldNumberFormatSwitch::LowerAlphabetic => Some(format_alpha_counter(value, true)),
        FieldNumberFormatSwitch::UpperRoman => Some(format_roman_counter(value, false)),
        FieldNumberFormatSwitch::LowerRoman => Some(format_roman_counter(value, true)),
        FieldNumberFormatSwitch::Ordinal => Some(format_ordinal_counter(value)),
        FieldNumberFormatSwitch::OrdinalText => format_ordinal_text_counter(value),
        FieldNumberFormatSwitch::Hex if value >= 0 => Some(format!("{value:X}")),
        FieldNumberFormatSwitch::Hex => None,
    }
}

pub(in crate::rtf::parser) fn uppercase_first_alphabetic(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut changed = false;
    for ch in text.chars() {
        if !changed && ch.is_alphabetic() {
            output.extend(ch.to_uppercase());
            changed = true;
        } else {
            output.push(ch);
        }
    }
    output
}

pub(in crate::rtf::parser) fn uppercase_each_word_start(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut capitalize_next = true;
    for ch in text.chars() {
        if ch.is_alphabetic() {
            if capitalize_next {
                output.extend(ch.to_uppercase());
            } else {
                output.push(ch);
            }
            capitalize_next = false;
        } else {
            output.push(ch);
            capitalize_next = !ch.is_alphanumeric();
        }
    }
    output
}

pub(in crate::rtf::parser) fn field_first_quoted_argument(instruction: &str) -> Option<String> {
    field_quoted_prefix(field_rest_after_name(instruction)?.trim_start())
}

pub(in crate::rtf::parser) fn field_formula_expression(instruction: &str) -> Option<&str> {
    let rest = instruction.trim_start().strip_prefix('=')?;
    let end = rest
        .char_indices()
        .find_map(|(index, ch)| (ch == '\\').then_some(index))
        .unwrap_or(rest.len());
    Some(rest[..end].trim())
}

pub(in crate::rtf::parser) fn field_first_argument(instruction: &str) -> Option<String> {
    let rest = field_rest_after_name(instruction)?.trim_start();
    if rest.starts_with('"') {
        return field_quoted_prefix(rest).map(|text| text.trim().to_string());
    }

    let end = rest
        .char_indices()
        .find_map(|(index, ch)| (ch.is_whitespace() || ch == '\\').then_some(index))
        .unwrap_or(rest.len());
    Some(rest[..end].trim().to_string())
}

pub(in crate::rtf::parser) fn field_rest_after_first_argument(instruction: &str) -> Option<&str> {
    let rest = field_rest_after_name(instruction)?.trim_start();
    skip_field_argument(rest)
}

pub(in crate::rtf::parser) fn field_set_instruction(
    instruction: &str,
    offset: usize,
    limits: &RtfLimits,
) -> Result<Option<(String, String)>, ParseError> {
    let Some(rest) = field_rest_after_name(instruction) else {
        return Ok(None);
    };
    let Some((name, rest)) = field_simple_argument_with_rest(rest) else {
        return Ok(None);
    };
    let Some(name) = clean_bookmark_name(name) else {
        return Ok(None);
    };
    if name.chars().count() > limits.max_text_run_len {
        return Err(ParseError::ResourceLimitExceeded {
            resource: "field bookmark name".to_string(),
            offset,
        });
    }

    let Some((value, rest)) = field_simple_argument_with_rest(rest) else {
        return Ok(None);
    };
    if !field_remainder_contains_only_passive_format_switches(rest) {
        return Ok(None);
    }

    let Some(value) = clean_field_bookmark_value(&value, offset, limits)? else {
        return Ok(None);
    };
    Ok(Some((name, value)))
}

pub(in crate::rtf::parser) fn clean_style_ref_name(name: &str) -> Option<String> {
    let name = name.trim().to_string();
    if name.is_empty() || name.chars().any(|ch| ch.is_control()) || contains_internal_marker(&name)
    {
        return None;
    }
    Some(name)
}

pub(in crate::rtf::parser) fn field_simple_argument_with_rest(
    input: &str,
) -> Option<(String, &str)> {
    let input = input.trim_start();
    if input.is_empty() || input.starts_with('\\') {
        return None;
    }

    if input.starts_with('"') {
        return Some((field_quoted_prefix(input)?, skip_field_argument(input)?));
    }

    let end = input
        .char_indices()
        .find_map(|(index, ch)| (ch.is_whitespace() || ch == '\\').then_some(index))
        .unwrap_or(input.len());
    (end > 0).then(|| (input[..end].trim().to_string(), &input[end..]))
}

pub(in crate::rtf::parser) fn clean_bookmark_name(name: String) -> Option<String> {
    let name = name.trim().to_string();
    if name.is_empty()
        || name.starts_with('_')
        || name.chars().any(|ch| ch.is_control())
        || contains_internal_marker(&name)
        || contains_active_pdf_marker_text(&name)
    {
        return None;
    }
    Some(name)
}
