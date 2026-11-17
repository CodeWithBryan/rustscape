//! JSON-simple's DOM grammar, numeric widths and stringification. Its object and
//! array states deliberately permit omitted/repeated separators. Invalid UTF-16,
//! excessive nesting and oversized text return bounded native errors.
use crate::{java_double, java_map::StringKeyOrder};
#[derive(Clone, Debug)]
pub enum Json {
    Null,
    Bool(bool),
    Int(i64),
    Double(f64),
    Text(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Character,
    Token,
    Number,
    Unicode,
    Limit,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error {
    pub position: usize,
    pub kind: ErrorKind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotObject;
impl Json {
    pub fn object() -> Self {
        Self::Object(Vec::new())
    }
    pub fn put(&mut self, key: impl Into<String>, value: Self) -> Result<Option<Self>, NotObject> {
        let Self::Object(entries) = self else {
            return Err(NotObject);
        };
        let key = key.into();
        if let Some((_, old)) = entries.iter_mut().find(|(name, _)| *name == key) {
            Ok(Some(std::mem::replace(old, value)))
        } else {
            entries.push((key, value));
            Ok(None)
        }
    }
    pub fn get(&self, key: &str) -> Option<&Self> {
        match self {
            Self::Object(entries) => entries
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }
    pub fn text(&self) -> String {
        match self {
            Self::Text(value) => value.clone(),
            Self::Double(value) => java_double::format(*value),
            _ => {
                let mut out = String::new();
                self.write(&mut out);
                out
            }
        }
    }
    fn write(&self, out: &mut String) {
        use std::fmt::Write;
        match self {
            Self::Null => out.push_str("null"),
            Self::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
            Self::Int(value) => write!(out, "{value}").unwrap(),
            Self::Double(value) => {
                if value.is_finite() {
                    out.push_str(&java_double::format(*value));
                } else {
                    out.push_str("null");
                }
            }
            Self::Text(value) => {
                out.push('"');
                for ch in value.chars() {
                    match ch {
                        '"' => out.push_str("\\\""),
                        '\\' => out.push_str("\\\\"),
                        '/' => out.push_str("\\/"),
                        '\u{8}' => out.push_str("\\b"),
                        '\u{c}' => out.push_str("\\f"),
                        '\n' => out.push_str("\\n"),
                        '\r' => out.push_str("\\r"),
                        '\t' => out.push_str("\\t"),
                        ch if ch <= '\u{1f}'
                            || ('\u{7f}'..='\u{9f}').contains(&ch)
                            || ('\u{2000}'..='\u{20ff}').contains(&ch) =>
                        {
                            write!(out, "\\u{:04X}", ch as u32).unwrap()
                        }
                        ch => out.push(ch),
                    }
                }
                out.push('"');
            }
            Self::Array(values) => {
                out.push('[');
                for (i, value) in values.iter().enumerate() {
                    if i != 0 {
                        out.push(',');
                    }
                    value.write(out);
                }
                out.push(']');
            }
            Self::Object(entries) => {
                out.push('{');
                let order = key_order(entries);
                for (i, index) in order.into_iter().enumerate() {
                    if i != 0 {
                        out.push(',');
                    }
                    let (key, value) = &entries[index];
                    Self::Text(key.clone()).write(out);
                    out.push(':');
                    value.write(out);
                }
                out.push('}');
            }
        }
    }
}
pub fn key_order(entries: &[(String, Json)]) -> Vec<usize> {
    let mut keys = StringKeyOrder::default();
    for (key, _) in entries {
        keys.insert(key.clone());
    }
    keys.iter().map(|(index, _)| index).collect()
}
#[derive(Debug)]
enum Token {
    Value(Json),
    Object,
    EndObject,
    Array,
    EndArray,
    Colon,
    Comma,
    End,
}
struct Parser<'a> {
    text: &'a str,
    index: usize,
    start: usize,
}
impl Parser<'_> {
    fn error(&self, kind: ErrorKind) -> Error {
        Error {
            position: self.text[..self.start].encode_utf16().count(),
            kind,
        }
    }
    fn byte(&self, index: usize) -> Option<u8> {
        self.text.as_bytes().get(index).copied()
    }
    fn next(&mut self) -> Result<Token, Error> {
        while self
            .byte(self.index)
            .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
        {
            self.index += 1;
        }
        self.start = self.index;
        let Some(byte) = self.byte(self.index) else {
            return Ok(Token::End);
        };
        self.index += 1;
        Ok(match byte {
            b'{' => Token::Object,
            b'}' => Token::EndObject,
            b'[' => Token::Array,
            b']' => Token::EndArray,
            b':' => Token::Colon,
            b',' => Token::Comma,
            b'"' => Token::Value(Json::Text(self.string()?)),
            b'n' if self.text[self.start..].starts_with("null") => {
                self.index = self.start + 4;
                Token::Value(Json::Null)
            }
            b't' if self.text[self.start..].starts_with("true") => {
                self.index = self.start + 4;
                Token::Value(Json::Bool(true))
            }
            b'f' if self.text[self.start..].starts_with("false") => {
                self.index = self.start + 5;
                Token::Value(Json::Bool(false))
            }
            b'-' | b'0'..=b'9' => {
                let digits = if byte == b'-' { self.index } else { self.start };
                let mut end = digits;
                while self.byte(end).is_some_and(|byte| byte.is_ascii_digit()) {
                    end += 1;
                }
                if end == digits {
                    return Err(self.error(ErrorKind::Character));
                }
                let mut float = false;
                if self.byte(end) == Some(b'.')
                    && self.byte(end + 1).is_some_and(|byte| byte.is_ascii_digit())
                {
                    float = true;
                    end += 1;
                    while self.byte(end).is_some_and(|byte| byte.is_ascii_digit()) {
                        end += 1;
                    }
                }
                if self
                    .byte(end)
                    .is_some_and(|byte| matches!(byte, b'e' | b'E'))
                {
                    let mut exponent = end + 1;
                    if self
                        .byte(exponent)
                        .is_some_and(|byte| matches!(byte, b'+' | b'-'))
                    {
                        exponent += 1;
                    }
                    if self
                        .byte(exponent)
                        .is_some_and(|byte| byte.is_ascii_digit())
                    {
                        float = true;
                        end = exponent + 1;
                        while self.byte(end).is_some_and(|byte| byte.is_ascii_digit()) {
                            end += 1;
                        }
                    }
                }
                self.index = end;
                let text = &self.text[self.start..end];
                Token::Value(if float {
                    Json::Double(java_double::parse(text).ok_or(self.error(ErrorKind::Number))?)
                } else {
                    Json::Int(text.parse().map_err(|_| self.error(ErrorKind::Number))?)
                })
            }
            _ => return Err(self.error(ErrorKind::Character)),
        })
    }
    fn string(&mut self) -> Result<String, Error> {
        let mut units = Vec::new();
        loop {
            let Some(ch) = self.text[self.index..].chars().next() else {
                self.start = self.index;
                return Err(self.error(ErrorKind::Token));
            };
            self.index += ch.len_utf8();
            match ch {
                '"' => {
                    return String::from_utf16(&units).map_err(|_| self.error(ErrorKind::Unicode));
                }
                '\\' => {
                    let escape = self.index - 1;
                    let Some(code) = self.byte(self.index) else {
                        self.start = self.index;
                        return Err(self.error(ErrorKind::Token));
                    };
                    self.index += 1;
                    match code {
                        b'"' => units.push(34),
                        b'\\' => units.push(92),
                        b'/' => units.push(47),
                        b'b' => units.push(8),
                        b'f' => units.push(12),
                        b'n' => units.push(10),
                        b'r' => units.push(13),
                        b't' => units.push(9),
                        b'u' => {
                            if !(0..4).all(|i| {
                                self.byte(self.index + i)
                                    .is_some_and(|byte| byte.is_ascii_hexdigit())
                            }) {
                                units.push(92);
                                self.index = escape + 1;
                                continue;
                            }
                            let mut value = 0u16;
                            for _ in 0..4 {
                                let Some(byte) = self.byte(self.index) else {
                                    self.start = escape;
                                    return Err(self.error(ErrorKind::Character));
                                };
                                let Some(digit) = (byte as char).to_digit(16) else {
                                    self.start = escape;
                                    return Err(self.error(ErrorKind::Character));
                                };
                                value = (value << 4) | (digit as u16);
                                self.index += 1;
                            }
                            units.push(value);
                        }
                        _ => {
                            units.push(92);
                            self.index = escape + 1;
                        }
                    }
                }
                ch => {
                    let mut encoded = [0; 2];
                    units.extend_from_slice(ch.encode_utf16(&mut encoded));
                }
            }
        }
    }
    fn value(&mut self, token: Token, depth: usize) -> Result<Json, Error> {
        if depth > 256 {
            return Err(self.error(ErrorKind::Limit));
        }
        match token {
            Token::Value(value) => Ok(value),
            Token::Array => {
                let mut values = Vec::new();
                loop {
                    let token = self.next()?;
                    match token {
                        Token::Comma => {}
                        Token::EndArray => return Ok(Json::Array(values)),
                        token => values.push(self.value(token, depth + 1)?),
                    }
                }
            }
            Token::Object => {
                let mut entries: Vec<(String, Json)> = Vec::new();
                loop {
                    let token = self.next()?;
                    let key = match token {
                        Token::Comma => continue,
                        Token::EndObject => return Ok(Json::Object(entries)),
                        Token::Value(Json::Text(key)) => key,
                        _ => return Err(self.error(ErrorKind::Token)),
                    };
                    let mut token = self.next()?;
                    while matches!(token, Token::Colon) {
                        token = self.next()?;
                    }
                    let value = self.value(token, depth + 1)?;
                    if let Some((_, old)) = entries.iter_mut().find(|(name, _)| *name == key) {
                        *old = value;
                    } else {
                        entries.push((key, value));
                    }
                }
            }
            _ => Err(self.error(ErrorKind::Token)),
        }
    }
}
pub fn parse(text: &str) -> Result<Json, Error> {
    if text.len() > 64 * 1024 * 1024 {
        return Err(Error {
            position: 0,
            kind: ErrorKind::Limit,
        });
    }
    let mut parser = Parser {
        text,
        index: 0,
        start: 0,
    };
    let token = parser.next()?;
    let value = parser.value(token, 0)?;
    if !matches!(parser.next()?, Token::End) {
        return Err(parser.error(ErrorKind::Token));
    }
    Ok(value)
}
