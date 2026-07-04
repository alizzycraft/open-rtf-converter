use thiserror::Error;

use crate::config::RtfLimits;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LexError {
    #[error("input is larger than the configured limit")]
    FileTooLarge,
    #[error("RTF group depth limit exceeded at byte {0}")]
    GroupDepthExceeded(usize),
    #[error("control word is longer than the configured limit at byte {0}")]
    ControlWordTooLong(usize),
    #[error("numeric parameter is longer than the configured limit at byte {0}")]
    NumericParameterTooLong(usize),
    #[error("numeric parameter overflow at byte {0}")]
    NumericParameterOverflow(usize),
    #[error("binary length is missing or negative at byte {0}")]
    InvalidBinaryLength(usize),
    #[error("binary blob is larger than the configured limit at byte {0}")]
    BinaryBlobTooLarge(usize),
    #[error("total binary data is larger than the configured limit at byte {0}")]
    TotalBinaryLimitExceeded(usize),
    #[error("binary blob is shorter than its declared length at byte {0}")]
    ShortBinaryBlob(usize),
    #[error("token count limit exceeded at byte {0}")]
    TokenLimitExceeded(usize),
    #[error("text run is larger than the configured limit at byte {0}")]
    TextRunTooLong(usize),
    #[error("malformed hex escape at byte {0}")]
    MalformedHexEscape(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    StartGroup,
    EndGroup,
    Control(Control),
    Text(String),
    HexByte(u8),
    Binary(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Control {
    pub name: String,
    pub parameter: Option<i32>,
}

pub struct Lexer<'a> {
    input: &'a [u8],
    pos: usize,
    limits: RtfLimits,
    group_depth: usize,
    total_binary_bytes: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a [u8], limits: RtfLimits) -> Self {
        Self {
            input,
            pos: 0,
            limits,
            group_depth: 0,
            total_binary_bytes: 0,
        }
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>, LexError> {
        if self.input.len() > self.limits.max_file_size {
            return Err(LexError::FileTooLarge);
        }

        let mut tokens = Vec::new();

        while self.pos < self.input.len() {
            let offset = self.pos;
            match self.input[self.pos] {
                b'{' => {
                    self.pos += 1;
                    self.group_depth = self
                        .group_depth
                        .checked_add(1)
                        .ok_or(LexError::GroupDepthExceeded(offset))?;
                    if self.group_depth > self.limits.max_group_depth {
                        return Err(LexError::GroupDepthExceeded(offset));
                    }
                    push_token(
                        &mut tokens,
                        Token {
                            kind: TokenKind::StartGroup,
                            offset,
                        },
                        &self.limits,
                    )?;
                }
                b'}' => {
                    self.pos += 1;
                    self.group_depth = self.group_depth.saturating_sub(1);
                    push_token(
                        &mut tokens,
                        Token {
                            kind: TokenKind::EndGroup,
                            offset,
                        },
                        &self.limits,
                    )?;
                }
                b'\\' => {
                    let token = self.read_control_or_escape()?;
                    let is_bin = matches!(
                        &token.kind,
                        TokenKind::Control(control) if control.name == "bin"
                    );
                    let binary_len = match &token.kind {
                        TokenKind::Control(control) if control.name == "bin" => {
                            Some(parse_bin_length(control.parameter, offset, &self.limits)?)
                        }
                        _ => None,
                    };

                    push_token(&mut tokens, token, &self.limits)?;

                    if is_bin {
                        let len = binary_len.expect("checked above");
                        let binary = self.read_binary(offset, len)?;
                        push_token(&mut tokens, binary, &self.limits)?;
                    }
                }
                b'\r' | b'\n' => {
                    self.pos += 1;
                }
                _ => push_token(&mut tokens, self.read_text()?, &self.limits)?,
            }
        }

        Ok(tokens)
    }

    fn read_text(&mut self) -> Result<Token, LexError> {
        let offset = self.pos;
        let start = self.pos;
        while self.pos < self.input.len() {
            match self.input[self.pos] {
                b'{' | b'}' | b'\\' | b'\r' | b'\n' => break,
                _ => self.pos += 1,
            }

            if self.pos - start > self.limits.max_text_run_len {
                return Err(LexError::TextRunTooLong(offset));
            }
        }

        Ok(Token {
            kind: TokenKind::Text(String::from_utf8_lossy(&self.input[start..self.pos]).into()),
            offset,
        })
    }

    fn read_control_or_escape(&mut self) -> Result<Token, LexError> {
        let offset = self.pos;
        self.pos += 1;

        if self.pos >= self.input.len() {
            return Ok(Token {
                kind: TokenKind::Text("\\".to_string()),
                offset,
            });
        }

        let ch = self.input[self.pos];
        match ch {
            b'\\' | b'{' | b'}' => {
                self.pos += 1;
                Ok(Token {
                    kind: TokenKind::Text((ch as char).to_string()),
                    offset,
                })
            }
            b'\'' => {
                self.pos += 1;
                let byte = self.read_hex_byte(offset)?;
                Ok(Token {
                    kind: TokenKind::HexByte(byte),
                    offset,
                })
            }
            b'~' => {
                self.pos += 1;
                Ok(Token {
                    kind: TokenKind::Text("\u{00a0}".to_string()),
                    offset,
                })
            }
            b'-' => {
                self.pos += 1;
                Ok(Token {
                    kind: TokenKind::Text("\u{00ad}".to_string()),
                    offset,
                })
            }
            b'_' => {
                self.pos += 1;
                Ok(Token {
                    kind: TokenKind::Text("\u{2011}".to_string()),
                    offset,
                })
            }
            b'*' => {
                self.pos += 1;
                Ok(Token {
                    kind: TokenKind::Control(Control {
                        name: "*".to_string(),
                        parameter: None,
                    }),
                    offset,
                })
            }
            b if b.is_ascii_alphabetic() => self.read_control_word(offset),
            other => {
                self.pos += 1;
                Ok(Token {
                    kind: TokenKind::Control(Control {
                        name: (other as char).to_string(),
                        parameter: None,
                    }),
                    offset,
                })
            }
        }
    }

    fn read_hex_byte(&mut self, offset: usize) -> Result<u8, LexError> {
        if self.pos + 1 >= self.input.len() {
            return Err(LexError::MalformedHexEscape(offset));
        }

        let hi = hex_value(self.input[self.pos]).ok_or(LexError::MalformedHexEscape(offset))?;
        let lo = hex_value(self.input[self.pos + 1]).ok_or(LexError::MalformedHexEscape(offset))?;
        self.pos += 2;
        Ok((hi << 4) | lo)
    }

    fn read_control_word(&mut self, offset: usize) -> Result<Token, LexError> {
        let start = self.pos;
        while self.pos < self.input.len() && self.input[self.pos].is_ascii_alphabetic() {
            self.pos += 1;
            if self.pos - start > self.limits.max_control_word_len {
                return Err(LexError::ControlWordTooLong(offset));
            }
        }

        let name = String::from_utf8_lossy(&self.input[start..self.pos]).into_owned();

        let mut sign: i64 = 1;
        if self.pos < self.input.len() && self.input[self.pos] == b'-' {
            sign = -1;
            self.pos += 1;
        }

        let number_start = self.pos;
        while self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
            self.pos += 1;
            if self.pos - number_start > self.limits.max_parameter_digits {
                return Err(LexError::NumericParameterTooLong(offset));
            }
        }

        let parameter = if self.pos > number_start {
            let mut value: i64 = 0;
            for byte in &self.input[number_start..self.pos] {
                value = value
                    .checked_mul(10)
                    .and_then(|value| value.checked_add((byte - b'0') as i64))
                    .ok_or(LexError::NumericParameterOverflow(offset))?;
            }
            value = value
                .checked_mul(sign)
                .ok_or(LexError::NumericParameterOverflow(offset))?;
            Some(i32::try_from(value).map_err(|_| LexError::NumericParameterOverflow(offset))?)
        } else {
            None
        };

        if self.pos < self.input.len() && self.input[self.pos] == b' ' {
            self.pos += 1;
        }

        Ok(Token {
            kind: TokenKind::Control(Control { name, parameter }),
            offset,
        })
    }

    fn read_binary(&mut self, offset: usize, len: usize) -> Result<Token, LexError> {
        if self.pos + len > self.input.len() {
            return Err(LexError::ShortBinaryBlob(offset));
        }
        self.total_binary_bytes = self
            .total_binary_bytes
            .checked_add(len)
            .ok_or(LexError::TotalBinaryLimitExceeded(offset))?;
        if self.total_binary_bytes > self.limits.max_total_binary_bytes {
            return Err(LexError::TotalBinaryLimitExceeded(offset));
        }

        let bytes = self.input[self.pos..self.pos + len].to_vec();
        self.pos += len;
        Ok(Token {
            kind: TokenKind::Binary(bytes),
            offset,
        })
    }
}

fn push_token(tokens: &mut Vec<Token>, token: Token, limits: &RtfLimits) -> Result<(), LexError> {
    if tokens.len() >= limits.max_token_count {
        return Err(LexError::TokenLimitExceeded(token.offset));
    }
    tokens.push(token);
    Ok(())
}

fn parse_bin_length(
    parameter: Option<i32>,
    offset: usize,
    limits: &RtfLimits,
) -> Result<usize, LexError> {
    let Some(parameter) = parameter else {
        return Err(LexError::InvalidBinaryLength(offset));
    };
    if parameter < 0 {
        return Err(LexError::InvalidBinaryLength(offset));
    }
    let len = usize::try_from(parameter).map_err(|_| LexError::NumericParameterOverflow(offset))?;
    if len > limits.max_binary_blob_size {
        return Err(LexError::BinaryBlobTooLarge(offset));
    }
    Ok(len)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_groups_controls_and_escaped_text() {
        let tokens = Lexer::new(br"{\rtf1 Hello \{world\}\par}", RtfLimits::default())
            .tokenize()
            .unwrap();
        assert_eq!(tokens[0].kind, TokenKind::StartGroup);
        assert_eq!(
            tokens[1].kind,
            TokenKind::Control(Control {
                name: "rtf".to_string(),
                parameter: Some(1)
            })
        );
        assert!(
            tokens
                .iter()
                .any(|token| token.kind == TokenKind::Text("{".to_string()))
        );
    }

    #[test]
    fn hex_escapes_preserve_raw_byte_for_parser_decoding() {
        let tokens = Lexer::new(br"{\rtf1 \'92}", RtfLimits::default())
            .tokenize()
            .unwrap();

        assert!(
            tokens
                .iter()
                .any(|token| token.kind == TokenKind::HexByte(0x92))
        );
    }

    #[test]
    fn bin_consumes_exact_raw_bytes() {
        let tokens = Lexer::new(br"{\rtf1 \bin3 a\} visible}", RtfLimits::default())
            .tokenize()
            .unwrap();
        assert!(
            tokens
                .iter()
                .any(|token| token.kind == TokenKind::Binary(b"a\\}".to_vec()))
        );
        assert!(
            tokens
                .iter()
                .any(|token| token.kind == TokenKind::Text(" visible".to_string()))
        );
    }
}
