//! Retain toml4j 0.7.2's string replacement ordering on the native TOML syntax tree.
// Algorithms adapted from toml4j, Copyright (c) 2013-2015 Moandji Ezana, MIT.
// The full permission notice is in third-party/toml4j-LICENSE.txt.
use super::{Error, ErrorKind, Result};
use toml_edit::{ImDocument, Item, Value};

fn invalid() -> Error {
    Error::at("toml", ErrorKind::Syntax)
}
fn string(raw: &str, decoded: &str) -> Result<String> {
    if !raw.starts_with('"') {
        return Ok(decoded.into());
    }
    let multiline = raw.starts_with("\"\"\"");
    let mut body = if multiline {
        let body = raw
            .strip_prefix("\"\"\"")
            .and_then(|s| s.strip_suffix("\"\"\""))
            .ok_or_else(invalid)?;
        body.strip_prefix('\n').unwrap_or(body).to_owned()
    } else {
        if raw.as_bytes().get(raw.len().saturating_sub(2)) == Some(&b'\\') {
            return Err(invalid());
        }
        raw.strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .ok_or_else(invalid)?
            .to_owned()
    };
    if multiline {
        // java.util.regex's default \s is ASCII whitespace; a backslash plus
        // whitespace is removed even when it is not a line-ending continuation.
        let mut folded = String::new();
        let mut chars = body.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\\'
                && chars
                    .peek()
                    .is_some_and(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0b' | '\x0c'))
            {
                while chars
                    .peek()
                    .is_some_and(|c| matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0b' | '\x0c'))
                {
                    chars.next();
                }
            } else {
                folded.push(ch);
            }
        }
        body = folded;
    }
    // The old regex consumes four hex digits for BOTH \u and \U. Matches are
    // collected before replacements, including occurrences behind a backslash.
    let original = body.clone();
    let mut chars = original.char_indices().peekable();
    while let Some((start, ch)) = chars.next() {
        if ch != '\\' || !chars.peek().is_some_and(|(_, ch)| matches!(ch, 'u' | 'U')) {
            continue;
        }
        chars.next();
        let mut digits = String::new();
        let mut end = start + 2;
        for _ in 0..4 {
            let Some((offset, ch)) = chars.next() else {
                digits.clear();
                break;
            };
            if matches!(ch, '\n' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}') {
                digits.clear();
                break;
            }
            digits.push(ch);
            end = offset + ch.len_utf8();
        }
        if digits.is_empty() {
            continue;
        }
        let code = u32::from_str_radix(&digits, 16)
            .ok()
            .and_then(char::from_u32)
            .ok_or_else(invalid)?;
        body = body.replace(&original[start..end], &code.to_string());
    }
    let bytes = body.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'\\' {
            if !matches!(
                bytes[i + 1],
                b'b' | b'f' | b'n' | b't' | b'r' | b'"' | b'\\'
            ) {
                return Err(invalid());
            }
            if bytes[i + 1] == b'\\' {
                i += 1;
            }
        }
        i += 1;
    }
    Ok(body
        .replace("\\n", "\n")
        .replace("\\\"", "\"")
        .replace("\\t", "\t")
        .replace("\\r", "\r")
        .replace("\\\\", "\\")
        .replace("\\/", "/")
        .replace("\\b", "\x08")
        .replace("\\f", "\x0c"))
}
fn value(value: &Value, source: &str) -> Result<toml::Value> {
    Ok(match value {
        Value::String(s) => {
            let range = s.span().ok_or_else(invalid)?;
            toml::Value::String(string(&source[range], s.value())?)
        }
        Value::Integer(n) => toml::Value::Integer(*n.value()),
        Value::Float(n) => toml::Value::Float(*n.value()),
        Value::Boolean(b) => toml::Value::Boolean(*b.value()),
        Value::Datetime(date) => toml::Value::Datetime(*date.value()),
        Value::Array(items) => toml::Value::Array(
            items
                .iter()
                .map(|item| self::value(item, source))
                .collect::<Result<_>>()?,
        ),
        Value::InlineTable(items) => toml::Value::Table(
            items
                .iter()
                .map(|(key, v)| Ok((key.to_owned(), self::value(v, source)?)))
                .collect::<Result<_>>()?,
        ),
    })
}
fn table(table: &toml_edit::Table, source: &str) -> Result<toml::Table> {
    let mut out = toml::Table::new();
    for (key, item) in table.iter() {
        let v = match item {
            Item::None => continue,
            Item::Value(v) => value(v, source)?,
            Item::Table(t) => toml::Value::Table(self::table(t, source)?),
            Item::ArrayOfTables(tables) => toml::Value::Array(
                tables
                    .iter()
                    .map(|t| self::table(t, source).map(toml::Value::Table))
                    .collect::<Result<_>>()?,
            ),
        };
        out.insert(key.to_owned(), v);
    }
    Ok(out)
}
pub(super) fn parse(text: &str) -> Result<toml::Table> {
    // BufferedReader.readLine normalizes CRLF and CR before toml4j sees input.
    let source = text.replace("\r\n", "\n").replace('\r', "\n");
    let document = ImDocument::parse(&source).map_err(|_| invalid())?;
    table(document.as_table(), &source)
}
