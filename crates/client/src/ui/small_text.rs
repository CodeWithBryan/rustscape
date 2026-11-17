//! Small text rules of the original client that several callers share:
//! `JagString.parseHexString` and the quick-chat phrase slot read by CS2 5012.
//! (The context-menu line of `MiniMenu.getOp` is `context_menu::op_text`.)

/// `JagString.parseHexString(radix)`, which `parseInt` calls with radix 10.
///
/// An optional sign in the first position, then digits `0-9`, `A-Z` and `a-z`
/// below the radix. `None` stands for the original's `NumberFormatException`:
/// no digit, a character outside the radix, or a product that does not divide
/// back to the previous value (the original's overflow test, on wrapping
/// 32-bit arithmetic). `Font.parseTag` swallows that exception, so a tag with
/// a bad number leaves the previous colour or alpha in place.
pub fn parse_radix(text: &str, radix: i32) -> Option<i32> {
    if radix == 0 {
        return None;
    }
    let mut negative = false;
    let mut has_digit = false;
    let mut result = 0i32;
    for (index, ch) in text.chars().enumerate() {
        // The original reads Latin-1 bytes; anything else is not a digit.
        let code = u32::from(ch);
        if index == 0 {
            if code == 45 {
                negative = true;
                continue;
            }
            if code == 43 {
                continue;
            }
        }
        let mut digit = match code {
            48..=57 => code as i32 - 48,
            65..=90 => code as i32 - 55,
            97..=122 => code as i32 - 87,
            _ => return None,
        };
        if radix <= digit {
            return None;
        }
        if negative {
            digit = -digit;
        }
        let next = digit.wrapping_add(result.wrapping_mul(radix));
        if next.wrapping_div(radix) != result {
            return None;
        }
        result = next;
        has_digit = true;
    }
    has_digit.then_some(result)
}

/// CS2 5012 (`ScriptRunner` 4217-4226): `Chat.phraseIds[index]` for an index
/// below 100, else -1. The array starts as zeros, so a line that was never
/// added reads 0. `phrase` is the stored line's phrase, when there is one.
pub fn phrase_id(index: i32, phrase: Option<i32>) -> i32 {
    if index < 100 { phrase.unwrap_or(0) } else { -1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Transcription tests: expected values worked out by hand from
    // JagString.parseHexString (333-374). The JVM corpus in
    // tests/ui_fidelity.rs covers the same function with seeded inputs.
    #[test]
    fn numbers_follow_the_original_parser() {
        assert_eq!(parse_radix("ff9040", 16), Some(0xff9040));
        assert_eq!(parse_radix("FF", 16), Some(255));
        assert_eq!(parse_radix("+7f", 16), Some(127));
        assert_eq!(parse_radix("-1", 16), Some(-1));
        assert_eq!(parse_radix("7fffffff", 16), Some(i32::MAX));
        // 0x80000000 wraps negative and fails the divide-back test.
        assert_eq!(parse_radix("80000000", 16), None);
        assert_eq!(parse_radix("ffffffff", 16), None);
        assert_eq!(parse_radix("", 16), None);
        assert_eq!(parse_radix("-", 16), None);
        assert_eq!(parse_radix("g", 16), None);
        assert_eq!(parse_radix("12 ", 16), None);
        assert_eq!(parse_radix("1-2", 16), None);
        assert_eq!(parse_radix("a", 10), None);
        assert_eq!(parse_radix("-2147483648", 10), Some(i32::MIN));
        assert_eq!(parse_radix("2147483648", 10), None);
        assert_eq!(parse_radix("0128", 10), Some(128));
    }

    #[test]
    fn unfilled_phrase_slots_read_zero() {
        assert_eq!(phrase_id(0, Some(17)), 17);
        assert_eq!(phrase_id(5, None), 0);
        assert_eq!(phrase_id(99, None), 0);
        assert_eq!(phrase_id(100, None), -1);
        assert_eq!(phrase_id(100, Some(3)), -1);
    }
}
