use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LexError {
    #[error("control word is not valid UTF-8 at byte {0}")]
    InvalidControlWord(usize),
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Control {
    pub name: String,
    pub parameter: Option<i32>,
}

pub struct Lexer<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            pos: 0,
        }
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>, LexError> {
        let mut tokens = Vec::new();

        while self.pos < self.input.len() {
            let offset = self.pos;
            match self.input[self.pos] {
                b'{' => {
                    self.pos += 1;
                    tokens.push(Token {
                        kind: TokenKind::StartGroup,
                        offset,
                    });
                }
                b'}' => {
                    self.pos += 1;
                    tokens.push(Token {
                        kind: TokenKind::EndGroup,
                        offset,
                    });
                }
                b'\\' => tokens.push(self.read_control_or_escape()?),
                b'\r' | b'\n' => {
                    self.pos += 1;
                }
                _ => tokens.push(self.read_text()),
            }
        }

        Ok(tokens)
    }

    fn read_text(&mut self) -> Token {
        let offset = self.pos;
        let start = self.pos;
        while self.pos < self.input.len() {
            match self.input[self.pos] {
                b'{' | b'}' | b'\\' | b'\r' | b'\n' => break,
                _ => self.pos += 1,
            }
        }

        Token {
            kind: TokenKind::Text(String::from_utf8_lossy(&self.input[start..self.pos]).into()),
            offset,
        }
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
                let byte = self.read_hex_byte().unwrap_or(b'?');
                Ok(Token {
                    kind: TokenKind::Text((byte as char).to_string()),
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

    fn read_hex_byte(&mut self) -> Option<u8> {
        if self.pos + 1 >= self.input.len() {
            return None;
        }

        let hi = hex_value(self.input[self.pos])?;
        let lo = hex_value(self.input[self.pos + 1])?;
        self.pos += 2;
        Some((hi << 4) | lo)
    }

    fn read_control_word(&mut self, offset: usize) -> Result<Token, LexError> {
        let start = self.pos;
        while self.pos < self.input.len() && self.input[self.pos].is_ascii_alphabetic() {
            self.pos += 1;
        }

        let name = std::str::from_utf8(&self.input[start..self.pos])
            .map_err(|_| LexError::InvalidControlWord(offset))?
            .to_string();

        let mut sign = 1;
        if self.pos < self.input.len() && self.input[self.pos] == b'-' {
            sign = -1;
            self.pos += 1;
        }

        let number_start = self.pos;
        while self.pos < self.input.len() && self.input[self.pos].is_ascii_digit() {
            self.pos += 1;
        }

        let parameter = if self.pos > number_start {
            let raw = std::str::from_utf8(&self.input[number_start..self.pos])
                .map_err(|_| LexError::InvalidControlWord(offset))?;
            Some(sign * raw.parse::<i32>().unwrap_or(0))
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
        let tokens = Lexer::new(r"{\rtf1 Hello \{world\}\par}")
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
}
