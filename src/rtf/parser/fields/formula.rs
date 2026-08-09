//! Bounded passive formula parsing and integer evaluation.

use super::super::*;
use super::formatting::*;

pub(in crate::rtf::parser) struct FormulaParser<'a, 'resolver> {
    pub(in crate::rtf::parser) input: &'a str,
    pub(in crate::rtf::parser) pos: usize,
    pub(in crate::rtf::parser) identifier_resolver: Option<&'resolver dyn Fn(&str) -> Option<i64>>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(in crate::rtf::parser) enum FormulaComparisonOperator {
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

pub(in crate::rtf::parser) const MAX_PASSIVE_FORMULA_FUNCTION_ARGS: usize = 32;
pub(in crate::rtf::parser) fn passive_formula_field_result(
    instruction: &str,
) -> Option<PassiveFieldResult> {
    let expression = field_formula_expression(instruction)?;
    if expression.is_empty() || expression.len() > 1024 {
        return None;
    }
    let value = FormulaParser::new(expression).parse()?;
    Some(PassiveFieldResult {
        text: value.to_string(),
        font_name: None,
        font_size_half_points: None,
        form_field: false,
    })
}

pub(in crate::rtf::parser) fn passive_formula_reference_value(text: &str) -> Option<i64> {
    let text = text.trim();
    if text.is_empty()
        || text.len() > 128
        || text.chars().any(|ch| ch.is_control())
        || contains_internal_marker(text)
    {
        return None;
    }
    FormulaParser::new(text).parse()
}

impl<'a, 'resolver> FormulaParser<'a, 'resolver> {
    pub(in crate::rtf::parser) fn new(input: &'a str) -> Self {
        Self {
            input,
            pos: 0,
            identifier_resolver: None,
        }
    }

    pub(in crate::rtf::parser) fn with_identifier_resolver(
        mut self,
        identifier_resolver: &'resolver dyn Fn(&str) -> Option<i64>,
    ) -> Self {
        self.identifier_resolver = Some(identifier_resolver);
        self
    }

    pub(in crate::rtf::parser) fn parse(mut self) -> Option<i64> {
        let value = self.parse_comparison_expression()?;
        self.skip_ws();
        (self.pos == self.input.len()).then_some(value)
    }

    pub(in crate::rtf::parser) fn parse_comparison_expression(&mut self) -> Option<i64> {
        let left = self.parse_expression()?;
        self.skip_ws();
        if let Some(operator) = self.parse_comparison_operator() {
            let right = self.parse_expression()?;
            if formula_comparison_matches(left, operator, right) {
                Some(1)
            } else {
                Some(0)
            }
        } else {
            Some(left)
        }
    }

    pub(in crate::rtf::parser) fn parse_expression(&mut self) -> Option<i64> {
        let mut value = self.parse_term()?;
        loop {
            self.skip_ws();
            if self.consume('+') {
                value = value.checked_add(self.parse_term()?)?;
            } else if self.consume('-') {
                value = value.checked_sub(self.parse_term()?)?;
            } else {
                return Some(value);
            }
        }
    }

    pub(in crate::rtf::parser) fn parse_term(&mut self) -> Option<i64> {
        let mut value = self.parse_power()?;
        loop {
            self.skip_ws();
            if self.consume('*') {
                value = value.checked_mul(self.parse_power()?)?;
            } else if self.consume('/') {
                let divisor = self.parse_power()?;
                if divisor == 0 {
                    return None;
                }
                value = value.checked_div(divisor)?;
            } else {
                return Some(value);
            }
        }
    }

    pub(in crate::rtf::parser) fn parse_power(&mut self) -> Option<i64> {
        let mut value = self.parse_factor()?;
        loop {
            self.skip_ws();
            if self.consume('^') {
                let exponent = u32::try_from(self.parse_factor()?).ok()?;
                value = value.checked_pow(exponent)?;
            } else {
                return Some(value);
            }
        }
    }

    pub(in crate::rtf::parser) fn parse_factor(&mut self) -> Option<i64> {
        self.skip_ws();
        if self.consume('(') {
            let value = self.parse_comparison_expression()?;
            self.skip_ws();
            return self.consume(')').then_some(value);
        }

        let negative = self.consume('-');
        if !negative {
            self.consume('+');
        }
        if self.peek().is_some_and(|ch| ch.is_ascii_alphabetic()) {
            let value = self.parse_identifier_value()?;
            return if negative {
                value.checked_neg()
            } else {
                Some(value)
            };
        }
        let value = self.parse_number()?;
        if negative {
            value.checked_neg()
        } else {
            Some(value)
        }
    }

    pub(in crate::rtf::parser) fn parse_identifier_value(&mut self) -> Option<i64> {
        let name = self.parse_identifier()?;
        self.skip_ws();
        if self.peek() == Some('(') {
            return self.parse_function_arguments(&name);
        }
        match name.as_str() {
            "TRUE" => Some(1),
            "FALSE" => Some(0),
            _ => self
                .identifier_resolver
                .and_then(|resolver| resolver(name.as_str())),
        }
    }

    pub(in crate::rtf::parser) fn parse_number(&mut self) -> Option<i64> {
        self.skip_ws();
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
        (self.pos > start).then(|| self.input[start..self.pos].parse::<i64>().ok())?
    }

    pub(in crate::rtf::parser) fn parse_function_arguments(&mut self, name: &str) -> Option<i64> {
        self.skip_ws();
        if !self.consume('(') {
            return None;
        }

        let mut args = Vec::new();
        let mut expecting_argument = false;
        loop {
            self.skip_ws();
            if self.consume(')') {
                if expecting_argument {
                    return None;
                }
                return evaluate_formula_function(name, &args);
            }

            if args.len() >= MAX_PASSIVE_FORMULA_FUNCTION_ARGS {
                return None;
            }
            let argument = self.parse_comparison_expression()?;
            args.push(argument);

            self.skip_ws();
            if self.consume_argument_separator() {
                expecting_argument = true;
                continue;
            }
            if self.consume(')') {
                return evaluate_formula_function(name, &args);
            }
            return None;
        }
    }

    pub(in crate::rtf::parser) fn parse_identifier(&mut self) -> Option<String> {
        self.skip_ws();
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
        (self.pos > start).then(|| self.input[start..self.pos].to_ascii_uppercase())
    }

    pub(in crate::rtf::parser) fn skip_ws(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
    }

    pub(in crate::rtf::parser) fn consume(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.pos += expected.len_utf8();
            true
        } else {
            false
        }
    }

    pub(in crate::rtf::parser) fn consume_str(&mut self, expected: &str) -> bool {
        if self.input[self.pos..].starts_with(expected) {
            self.pos += expected.len();
            true
        } else {
            false
        }
    }

    pub(in crate::rtf::parser) fn consume_argument_separator(&mut self) -> bool {
        self.consume(',') || self.consume(';')
    }

    pub(in crate::rtf::parser) fn parse_comparison_operator(
        &mut self,
    ) -> Option<FormulaComparisonOperator> {
        self.skip_ws();
        if self.consume_str("<=") {
            Some(FormulaComparisonOperator::LessOrEqual)
        } else if self.consume_str(">=") {
            Some(FormulaComparisonOperator::GreaterOrEqual)
        } else if self.consume_str("<>") {
            Some(FormulaComparisonOperator::NotEqual)
        } else if self.consume_str("=") {
            Some(FormulaComparisonOperator::Equal)
        } else if self.consume_str("<") {
            Some(FormulaComparisonOperator::Less)
        } else if self.consume_str(">") {
            Some(FormulaComparisonOperator::Greater)
        } else {
            None
        }
    }

    pub(in crate::rtf::parser) fn peek(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }
}

pub(in crate::rtf::parser) fn formula_comparison_matches(
    left: i64,
    operator: FormulaComparisonOperator,
    right: i64,
) -> bool {
    match operator {
        FormulaComparisonOperator::Equal => left == right,
        FormulaComparisonOperator::NotEqual => left != right,
        FormulaComparisonOperator::Less => left < right,
        FormulaComparisonOperator::LessOrEqual => left <= right,
        FormulaComparisonOperator::Greater => left > right,
        FormulaComparisonOperator::GreaterOrEqual => left >= right,
    }
}

pub(in crate::rtf::parser) fn evaluate_formula_function(name: &str, args: &[i64]) -> Option<i64> {
    match name {
        "TRUE" if args.is_empty() => Some(1),
        "FALSE" if args.is_empty() => Some(0),
        "SUM" if !args.is_empty() => args
            .iter()
            .try_fold(0i64, |value, argument| value.checked_add(*argument)),
        "MIN" if !args.is_empty() => args.iter().copied().min(),
        "MAX" if !args.is_empty() => args.iter().copied().max(),
        "ABS" if args.len() == 1 => args[0].checked_abs(),
        "PRODUCT" if !args.is_empty() => args
            .iter()
            .try_fold(1i64, |value, argument| value.checked_mul(*argument)),
        "POWER" if args.len() == 2 => {
            let exponent = u32::try_from(args[1]).ok()?;
            args[0].checked_pow(exponent)
        }
        "MOD" if args.len() == 2 => {
            if args[1] == 0 {
                None
            } else {
                args[0].checked_rem(args[1])
            }
        }
        "QUOTIENT" if args.len() == 2 => {
            if args[1] == 0 {
                None
            } else {
                args[0].checked_div(args[1])
            }
        }
        "ROUND" if args.len() == 2 => (args[1] == 0).then_some(args[0]),
        "COUNT" if !args.is_empty() => i64::try_from(args.len()).ok(),
        "N" if args.len() == 1 => Some(args[0]),
        "INT" if args.len() == 1 => Some(args[0]),
        "SIGN" if args.len() == 1 => Some(args[0].signum()),
        "EVEN" if args.len() == 1 => checked_formula_even(args[0]),
        "ODD" if args.len() == 1 => checked_formula_odd(args[0]),
        "SQRT" if args.len() == 1 => checked_exact_integer_sqrt(args[0]),
        "AVERAGE" if !args.is_empty() => {
            let sum = args
                .iter()
                .try_fold(0i64, |value, argument| value.checked_add(*argument))?;
            let divisor = i64::try_from(args.len()).ok()?;
            (sum % divisor == 0).then_some(sum / divisor)
        }
        "AND" if !args.is_empty() => Some(i64::from(args.iter().all(|argument| *argument != 0))),
        "OR" if !args.is_empty() => Some(i64::from(args.iter().any(|argument| *argument != 0))),
        "NOT" if args.len() == 1 => Some(i64::from(args[0] == 0)),
        "XOR" if !args.is_empty() => Some(i64::from(
            args.iter().filter(|argument| **argument != 0).count() % 2 == 1,
        )),
        "IF" if args.len() == 3 => Some(if args[0] != 0 { args[1] } else { args[2] }),
        _ => None,
    }
}

pub(in crate::rtf::parser) fn checked_formula_even(value: i64) -> Option<i64> {
    if value % 2 == 0 {
        Some(value)
    } else if value > 0 {
        value.checked_add(1)
    } else {
        value.checked_sub(1)
    }
}

pub(in crate::rtf::parser) fn checked_formula_odd(value: i64) -> Option<i64> {
    if value % 2 != 0 {
        Some(value)
    } else if value > 0 {
        value.checked_add(1)
    } else if value < 0 {
        value.checked_sub(1)
    } else {
        Some(1)
    }
}

pub(in crate::rtf::parser) fn checked_exact_integer_sqrt(value: i64) -> Option<i64> {
    let value = u64::try_from(value).ok()?;
    let mut low = 0u64;
    let mut high = value.min(i64::MAX as u64);
    while low <= high {
        let mid = low + (high - low) / 2;
        match mid.checked_mul(mid)?.cmp(&value) {
            std::cmp::Ordering::Equal => return i64::try_from(mid).ok(),
            std::cmp::Ordering::Less => low = mid.checked_add(1)?,
            std::cmp::Ordering::Greater => high = mid.checked_sub(1)?,
        }
    }
    None
}
