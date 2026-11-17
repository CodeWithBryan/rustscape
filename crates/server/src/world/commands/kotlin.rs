//! The Kotlin and JDK string conversions the commands use.
//!
//! The command line reaches these only as text the client can send: the
//! decoder widens each byte as a signed value, so every character is in
//! U+0001..U+007F or U+FF80..U+FFFF. `Character.digit` accepts no digit
//! outside ASCII in those ranges, so the parsers here accept ASCII digits
//! only (Kotlin and the JDK also accept other Unicode decimal digits).
use super::{Error, Result};

/// Kotlin `String.toIntOrNull()` (radix 10).
pub fn to_int_or_null(text: &str) -> Option<i32> {
    let chars: Vec<char> = text.chars().collect();
    let first = *chars.first()?;
    let (start, negative) = if first < '0' {
        if chars.len() == 1 {
            return None;
        }
        match first {
            '-' => (1, true),
            '+' => (1, false),
            _ => return None,
        }
    } else {
        (0, false)
    };
    let mut result: i64 = 0;
    for ch in &chars[start..] {
        let digit = ch.to_digit(10)? as i64;
        result = result * 10 + digit;
        if result > i64::from(i32::MAX) + 1 {
            return None;
        }
    }
    let value = if negative { -result } else { result };
    i32::try_from(value).ok()
}

/// `Integer.parseInt(text)` (Kotlin `toInt()`).
pub fn to_int(text: &str) -> Result<i32> {
    to_int_or_null(text).ok_or_else(|| Error::NumberFormat(format!("For input string: \"{text}\"")))
}

/// Kotlin `String.toFloatOrNull()`: the JDK's float grammar, then
/// `Float.parseFloat`. The value is parsed as a double and narrowed, which
/// can differ from `Float.parseFloat` by one unit where a decimal lies
/// exactly between two floats after the double rounding.
pub fn to_float_or_null(text: &str) -> Option<f32> {
    rs09_game_core::java_double::parse(text).map(|value| value as f32)
}

/// `Float.toString(value)` (JDK 11's `FloatingDecimal`, which is not
/// always the shortest form: `rs09_game_core::java_double::format_float`).
pub fn float_to_string(value: f32) -> String {
    rs09_game_core::java_double::format_float(value)
}

/// `TimeUnit`s the duration parser picks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeUnit {
    Nanoseconds,
    Seconds,
    Minutes,
    Hours,
    Days,
}
impl TimeUnit {
    /// `name.toLowerCase()`.
    pub fn lower_name(self) -> &'static str {
        match self {
            Self::Nanoseconds => "nanoseconds",
            Self::Seconds => "seconds",
            Self::Minutes => "minutes",
            Self::Hours => "hours",
            Self::Days => "days",
        }
    }
    /// `toMillis(value)`, saturating as the JDK does.
    pub fn to_millis(self, value: i64) -> i64 {
        let scale: i64 = match self {
            Self::Nanoseconds => return value / 1_000_000,
            Self::Seconds => 1_000,
            Self::Minutes => 60_000,
            Self::Hours => 3_600_000,
            Self::Days => 86_400_000,
        };
        value
            .checked_mul(scale)
            .unwrap_or(if value > 0 { i64::MAX } else { i64::MIN })
    }
}

/// `Character.isWhitespace`-free check of a single-character string:
/// `token.toString().toIntOrNull() != null`.
pub fn is_digit_token(ch: char) -> bool {
    ch.is_ascii_digit()
}

/// Kotlin `Char.isWhitespace()`: `Character.isWhitespace` or `isSpaceChar`.
pub fn is_kotlin_whitespace(ch: char) -> bool {
    matches!(
        ch,
        '\t' | '\n'
            | '\u{0B}'
            | '\u{0C}'
            | '\r'
            | '\u{1C}'..='\u{20}'
            | '\u{A0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200A}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
    )
}

/// Kotlin `String.trim()`.
pub fn kotlin_trim(text: &str) -> &str {
    text.trim_matches(is_kotlin_whitespace)
}

/// `split(..)[index]` of Kotlin's `split`: a string without the delimiter
/// splits to `listOf(this)`, whose `get` fails with the singleton list's
/// message; otherwise an `ArrayList`'s.
pub fn split_get<'a>(parts: &[&'a str], index: usize) -> super::Result<&'a str> {
    parts.get(index).copied().ok_or_else(|| {
        super::Error::Index(if parts.len() == 1 {
            format!("Index: {index}, Size: 1")
        } else {
            format!("Index {index} out of bounds for length {}", parts.len())
        })
    })
}
