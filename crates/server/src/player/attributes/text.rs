use super::{Error, Result};
use serde_json::Value;
pub fn kotlin_trim(value: &str) -> &str {
    value.trim_matches(|c:char|matches!(c,'\t'..='\r'|'\u{001c}'..='\u{0020}'|'\u{00a0}'|'\u{1680}'|'\u{2000}'..='\u{200a}'|'\u{2028}'|'\u{2029}'|'\u{202f}'|'\u{205f}'|'\u{3000}'))
}
pub fn nullable_string(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => "null".into(),
        Some(Value::String(value)) => value.clone(),
        Some(Value::Number(value)) if value.is_f64() => {
            rs09_game_core::java_double::format(value.as_f64().unwrap())
        }
        Some(Value::Array(_) | Value::Object(_)) => json_string(value.unwrap()),
        Some(value) => value.to_string(),
    }
}
fn quote(text: &str) -> String {
    let mut out = String::from("\"");
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '/' => out.push_str("\\/"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch <= '\u{1f}' || matches!(ch,'\u{7f}'..='\u{9f}'|'\u{2000}'..='\u{20ff}') => {
                use std::fmt::Write;
                write!(out, "\\u{:04X}", ch as u32).unwrap();
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}
fn json_string(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::String(value) => quote(value),
        Value::Array(values) => format!(
            "[{}]",
            values.iter().map(json_string).collect::<Vec<_>>().join(",")
        ),
        Value::Object(values) => {
            let keys = rs09_game_core::java_map::string_key_order(values.keys().cloned());
            format!(
                "{{{}}}",
                keys.iter()
                    .map(|key| format!("{}:{}", quote(key), json_string(&values[key])))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
        Value::Number(value) if value.is_f64() => {
            rs09_game_core::java_double::format(value.as_f64().unwrap())
        }
        _ => value.to_string(),
    }
}
const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
pub fn base64_byte(value: i8) -> String {
    let value = value as u8;
    String::from_utf8(vec![
        BASE64[(value >> 2) as usize],
        BASE64[((value & 3) << 4) as usize],
        b'=',
        b'=',
    ])
    .unwrap()
}
/// Validate the entire basic Base64 stream, retaining only byte zero consumed by
/// PlayerSaveParser. Java accepts missing padding and nonzero unused trailing bits.
pub fn first_base64_byte(text: &str) -> Result<i8> {
    let input = text.as_bytes();
    let mut count = 0usize;
    let mut word = 0u32;
    let mut first = None;
    let mut cursor = 0;
    while cursor < input.len() {
        let byte = input[cursor];
        if byte == b'=' {
            if count == 2 {
                if input.get(cursor + 1) != Some(&b'=') || cursor + 2 != input.len() {
                    return Err(Error::Base64);
                }
            } else if count == 3 {
                if cursor + 1 != input.len() {
                    return Err(Error::Base64);
                }
            } else {
                return Err(Error::Base64);
            }
            if first.is_none() {
                first = Some((word >> (if count == 2 { 4 } else { 10 })) as u8);
            }
            return Ok(first.unwrap() as i8);
        }
        let value = BASE64
            .iter()
            .position(|&value| value == byte)
            .ok_or(Error::Base64)?;
        word = (word << 6) | value as u32;
        count += 1;
        if count == 4 {
            if first.is_none() {
                first = Some((word >> 16) as u8);
            }
            word = 0;
            count = 0;
        }
        cursor += 1;
    }
    if count == 1 {
        return Err(Error::Base64);
    }
    if first.is_none() && count >= 2 {
        first = Some((word >> (if count == 2 { 4 } else { 10 })) as u8);
    }
    first.map(|value| value as i8).ok_or(Error::Index)
}
