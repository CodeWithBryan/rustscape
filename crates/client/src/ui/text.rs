//! Original byte collation, including ligature expansion and case-sensitive order.
use super::text_order::ORDER;
pub fn concat_character(text: &str, character: i32) -> Result<String, String> {
    if !(1..=255).contains(&character) {
        return Err("invalid char".into());
    }
    let mut result = text.to_owned();
    result.push(char::from(character as u8));
    Ok(result)
}
pub fn substring(text: &str, from: i32, to: i32) -> Result<String, String> {
    let chars: Vec<_> = text.chars().collect();
    if from < 0 || to < from || to as usize > chars.len() {
        return Err("Invalid substring bounds".into());
    }
    Ok(chars[from as usize..to as usize].iter().collect())
}
pub fn character(op: i32, c: i32) -> i32 {
    let letter = (65..=90).contains(&c) || (97..=122).contains(&c);
    let digit = (48..=57).contains(&c);
    match op {
        4113 => i32::from(
            (32..=126).contains(&c)
                || (160..=255).contains(&c)
                || matches!(c, 128 | 140 | 151 | 156 | 159),
        ),
        4114 => i32::from(letter || digit),
        4115 => i32::from(letter),
        4116 => i32::from(digit),
        4122 => {
            if (65..=90).contains(&c) || (192..=222).contains(&c) && c != 215 {
                c + 32
            } else {
                match c {
                    159 => 255,
                    140 => 156,
                    _ => c,
                }
            }
        }
        4123 => {
            if (97..=122).contains(&c) || (224..=254).contains(&c) && c != 247 {
                c - 32
            } else {
                match c {
                    255 => 159,
                    156 => 140,
                    _ => c,
                }
            }
        }
        _ => c,
    }
}
pub fn compare(a: &str, b: &str) -> i32 {
    let a = a.chars().map(|c| c as u8).collect::<Vec<_>>();
    let b = b.chars().map(|c| c as u8).collect::<Vec<_>>();
    let (mut left, mut right) = (a.len(), b.len());
    let (mut total_a, mut total_b) = (a.len(), b.len());
    let (mut i, mut j) = (0, 0);
    let (mut c1, mut c2) = (0, 0);
    while left != 0 && right != 0 {
        let next = |previous: u8, bytes: &[u8], index: &mut usize| match previous {
            156 | 230 => 101,
            140 | 198 => 69,
            223 => 115,
            _ => {
                let c = bytes[*index];
                *index += 1;
                c
            }
        };
        c1 = next(c1, &a, &mut i);
        c2 = next(c2, &b, &mut j);
        if matches!(c1, 198 | 230 | 156 | 140 | 223) {
            total_a += 1;
        } else {
            left -= 1;
        }
        if matches!(c2, 198 | 230 | 156 | 140 | 223) {
            total_b += 1;
        } else {
            right -= 1;
        }
        if ORDER[c1 as usize] < ORDER[c2 as usize] {
            return -1;
        }
        if ORDER[c1 as usize] > ORDER[c2 as usize] {
            return 1;
        }
    }
    match total_a.cmp(&total_b) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}
