//! Integer parsing and UTF-16 case comparison for the pinned JDK 11.
#[path = "java_case.rs"]
mod case;
/// JDK 11 compares UTF-16 code units here, including separate surrogate halves.
pub fn equals_ignore_case(left: &str, right: &str) -> bool {
    let mut left = left.encode_utf16();
    let mut right = right.encode_utf16();
    loop {
        match (left.next(), right.next()) {
            (None, None) => return true,
            (Some(a), Some(b)) if a == b || fold_case_unit(a) == fold_case_unit(b) => {}
            _ => return false,
        }
    }
}
pub fn fold_case_unit(code: u16) -> u16 {
    if code < 128 {
        return (code as u8).to_ascii_lowercase().into();
    }
    case::FOLD
        .binary_search_by_key(&code, |&(from, _)| from)
        .map_or(code, |index| case::FOLD[index].1)
}
pub fn parse_i32(text: &str) -> Option<i32> {
    parse_i64(text)?.try_into().ok()
}
pub fn parse_i64(text: &str) -> Option<i64> {
    const ZEROES: &[u32] = &[
        0x30, 0x660, 0x6f0, 0x7c0, 0x966, 0x9e6, 0xa66, 0xae6, 0xb66, 0xbe6, 0xc66, 0xce6, 0xd66,
        0xde6, 0xe50, 0xed0, 0xf20, 0x1040, 0x1090, 0x17e0, 0x1810, 0x1946, 0x19d0, 0x1a80, 0x1a90,
        0x1b50, 0x1bb0, 0x1c40, 0x1c50, 0xa620, 0xa8d0, 0xa900, 0xa9d0, 0xa9f0, 0xaa50, 0xabf0,
        0xff10,
    ];
    let mut chars = text.encode_utf16();
    let first = chars.next()?;
    let negative = first == u16::from(b'-');
    let sign = negative || first == u16::from(b'+');
    let limit = if negative { i64::MIN } else { -i64::MAX };
    let mut value = 0i64;
    let mut count = 0;
    for code in (!sign).then_some(first).into_iter().chain(chars) {
        let code = u32::from(code);
        let digit = ZEROES.iter().find_map(|&zero| {
            (zero..zero + 10)
                .contains(&code)
                .then_some(code.wrapping_sub(zero) as i64)
        })?;
        value = value.checked_mul(10)?.checked_sub(digit)?;
        if value < limit {
            return None;
        }
        count += 1;
    }
    if count == 0 {
        None
    } else if negative {
        Some(value)
    } else {
        Some(-value)
    }
}
