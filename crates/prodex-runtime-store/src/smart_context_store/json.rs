use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum RuntimeSmartContextJsonValue {
    Null,
    Bool(bool),
    Number(i64),
    String(String),
    Array(Vec<RuntimeSmartContextJsonValue>),
    Object(BTreeMap<String, RuntimeSmartContextJsonValue>),
}

pub(super) struct RuntimeSmartContextJsonParser<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> RuntimeSmartContextJsonParser<'a> {
    pub(super) fn parse(
        input: &'a str,
    ) -> Result<RuntimeSmartContextJsonValue, RuntimeSmartContextArtifactStoreJsonError> {
        let mut parser = Self {
            input: input.as_bytes(),
            pos: 0,
        };
        let value = parser.parse_value()?;
        parser.skip_whitespace();
        if parser.pos != parser.input.len() {
            return Err(RuntimeSmartContextArtifactStoreJsonError::new(
                "trailing JSON content",
            ));
        }
        Ok(value)
    }

    fn parse_value(
        &mut self,
    ) -> Result<RuntimeSmartContextJsonValue, RuntimeSmartContextArtifactStoreJsonError> {
        self.skip_whitespace();
        match self.peek_byte() {
            Some(b'n') => {
                self.expect_literal(b"null")?;
                Ok(RuntimeSmartContextJsonValue::Null)
            }
            Some(b't') => {
                self.expect_literal(b"true")?;
                Ok(RuntimeSmartContextJsonValue::Bool(true))
            }
            Some(b'f') => {
                self.expect_literal(b"false")?;
                Ok(RuntimeSmartContextJsonValue::Bool(false))
            }
            Some(b'"') => self
                .parse_string()
                .map(RuntimeSmartContextJsonValue::String),
            Some(b'[') => self.parse_array(),
            Some(b'{') => self.parse_object(),
            Some(b'-' | b'0'..=b'9') => self
                .parse_number()
                .map(RuntimeSmartContextJsonValue::Number),
            Some(_) => Err(RuntimeSmartContextArtifactStoreJsonError::new(
                "invalid JSON value",
            )),
            None => Err(RuntimeSmartContextArtifactStoreJsonError::new(
                "unexpected end of JSON",
            )),
        }
    }

    fn parse_array(
        &mut self,
    ) -> Result<RuntimeSmartContextJsonValue, RuntimeSmartContextArtifactStoreJsonError> {
        self.expect_byte(b'[')?;
        self.skip_whitespace();
        let mut items = Vec::new();
        if self.consume_byte(b']') {
            return Ok(RuntimeSmartContextJsonValue::Array(items));
        }
        loop {
            items.push(self.parse_value()?);
            self.skip_whitespace();
            if self.consume_byte(b']') {
                return Ok(RuntimeSmartContextJsonValue::Array(items));
            }
            self.expect_byte(b',')?;
        }
    }

    fn parse_object(
        &mut self,
    ) -> Result<RuntimeSmartContextJsonValue, RuntimeSmartContextArtifactStoreJsonError> {
        self.expect_byte(b'{')?;
        self.skip_whitespace();
        let mut entries = BTreeMap::new();
        if self.consume_byte(b'}') {
            return Ok(RuntimeSmartContextJsonValue::Object(entries));
        }
        loop {
            self.skip_whitespace();
            let key = self.parse_string()?;
            self.skip_whitespace();
            self.expect_byte(b':')?;
            let value = self.parse_value()?;
            entries.insert(key, value);
            self.skip_whitespace();
            if self.consume_byte(b'}') {
                return Ok(RuntimeSmartContextJsonValue::Object(entries));
            }
            self.expect_byte(b',')?;
        }
    }

    fn parse_string(&mut self) -> Result<String, RuntimeSmartContextArtifactStoreJsonError> {
        self.expect_byte(b'"')?;
        let mut output = String::new();
        loop {
            let start = self.pos;
            while let Some(byte) = self.peek_byte() {
                match byte {
                    b'"' | b'\\' | 0x00..=0x1f => break,
                    _ => self.pos += 1,
                }
            }
            if self.pos > start {
                let chunk = std::str::from_utf8(&self.input[start..self.pos]).map_err(|_| {
                    RuntimeSmartContextArtifactStoreJsonError::new("invalid UTF-8 string")
                })?;
                output.push_str(chunk);
            }
            match self.next_byte() {
                Some(b'"') => return Ok(output),
                Some(b'\\') => output.push(self.parse_escape()?),
                Some(0x00..=0x1f) => {
                    return Err(RuntimeSmartContextArtifactStoreJsonError::new(
                        "unescaped control character in string",
                    ));
                }
                Some(_) => unreachable!("string scanner stops only on delimiter or control"),
                None => {
                    return Err(RuntimeSmartContextArtifactStoreJsonError::new(
                        "unterminated JSON string",
                    ));
                }
            }
        }
    }

    fn parse_escape(&mut self) -> Result<char, RuntimeSmartContextArtifactStoreJsonError> {
        match self.next_byte() {
            Some(b'"') => Ok('"'),
            Some(b'\\') => Ok('\\'),
            Some(b'/') => Ok('/'),
            Some(b'b') => Ok('\u{08}'),
            Some(b'f') => Ok('\u{0c}'),
            Some(b'n') => Ok('\n'),
            Some(b'r') => Ok('\r'),
            Some(b't') => Ok('\t'),
            Some(b'u') => {
                let codepoint = self.parse_hex4()?;
                char::from_u32(codepoint).ok_or_else(|| {
                    RuntimeSmartContextArtifactStoreJsonError::new("invalid unicode escape")
                })
            }
            Some(_) => Err(RuntimeSmartContextArtifactStoreJsonError::new(
                "invalid JSON escape",
            )),
            None => Err(RuntimeSmartContextArtifactStoreJsonError::new(
                "unterminated JSON escape",
            )),
        }
    }

    fn parse_hex4(&mut self) -> Result<u32, RuntimeSmartContextArtifactStoreJsonError> {
        let mut codepoint = 0_u32;
        for _ in 0..4 {
            let Some(byte) = self.next_byte() else {
                return Err(RuntimeSmartContextArtifactStoreJsonError::new(
                    "short unicode escape",
                ));
            };
            let digit = match byte {
                b'0'..=b'9' => u32::from(byte - b'0'),
                b'a'..=b'f' => u32::from(byte - b'a' + 10),
                b'A'..=b'F' => u32::from(byte - b'A' + 10),
                _ => {
                    return Err(RuntimeSmartContextArtifactStoreJsonError::new(
                        "invalid unicode escape",
                    ));
                }
            };
            codepoint = (codepoint << 4) | digit;
        }
        Ok(codepoint)
    }

    fn parse_number(&mut self) -> Result<i64, RuntimeSmartContextArtifactStoreJsonError> {
        let start = self.pos;
        self.consume_byte(b'-');
        let digit_start = self.pos;
        while matches!(self.peek_byte(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
        if self.pos == digit_start {
            return Err(RuntimeSmartContextArtifactStoreJsonError::new(
                "invalid JSON number",
            ));
        }
        let text = std::str::from_utf8(&self.input[start..self.pos])
            .map_err(|_| RuntimeSmartContextArtifactStoreJsonError::new("invalid JSON number"))?;
        text.parse::<i64>()
            .map_err(|_| RuntimeSmartContextArtifactStoreJsonError::new("invalid JSON number"))
    }

    fn expect_literal(
        &mut self,
        literal: &[u8],
    ) -> Result<(), RuntimeSmartContextArtifactStoreJsonError> {
        if self
            .input
            .get(self.pos..self.pos.saturating_add(literal.len()))
            == Some(literal)
        {
            self.pos += literal.len();
            Ok(())
        } else {
            Err(RuntimeSmartContextArtifactStoreJsonError::new(
                "invalid JSON literal",
            ))
        }
    }

    fn expect_byte(
        &mut self,
        expected: u8,
    ) -> Result<(), RuntimeSmartContextArtifactStoreJsonError> {
        match self.next_byte() {
            Some(byte) if byte == expected => Ok(()),
            _ => Err(RuntimeSmartContextArtifactStoreJsonError::new(
                "unexpected JSON token",
            )),
        }
    }

    fn consume_byte(&mut self, expected: u8) -> bool {
        if self.peek_byte() == Some(expected) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn next_byte(&mut self) -> Option<u8> {
        let byte = self.input.get(self.pos).copied()?;
        self.pos += 1;
        Some(byte)
    }

    fn peek_byte(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek_byte(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.pos += 1;
        }
    }
}
