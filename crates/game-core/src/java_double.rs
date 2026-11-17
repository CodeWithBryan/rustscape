/*
 * Copyright (c) 1996, 2016, Oracle and/or its affiliates. All rights reserved.
 * DO NOT ALTER OR REMOVE COPYRIGHT NOTICES OR THIS FILE HEADER.
 *
 * This code is free software; you can redistribute it and/or modify it
 * under the terms of the GNU General Public License version 2 only, as
 * published by the Free Software Foundation.  Oracle designates this
 * particular file as subject to the "Classpath" exception as provided
 * by Oracle in the LICENSE file that accompanied this code.
 *
 * This code is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License
 * version 2 for more details (a copy is included in the LICENSE file that
 * accompanied this code).
 *
 * You should have received a copy of the GNU General Public License version
 * 2 along with this work; if not, write to the Free Software Foundation,
 * Inc., 51 Franklin St, Fifth Floor, Boston, MA 02110-1301 USA.
 *
 * Please contact Oracle, 500 Oracle Parkway, Redwood Shores, CA 94065 USA
 * or visit www.oracle.com if you need additional information or have any
 * questions.
 */
// Rust adaptation: JDK 11 decimal digit/stopping/rounding behavior; native bigint arithmetic.
// SPDX-License-Identifier: GPL-2.0-only WITH Classpath-exception-2.0
//! Decimal output matching the JDK 11 used by the pinned server's save format.
//! Integer-scaled digit generation retains the legacy boundary/rounding rules.
use num_bigint::BigUint;
#[allow(
    clippy::approx_constant,
    reason = "The legacy decimal estimate uses this truncated constant, not LOG10_2."
)]
const ESTIMATE_LOG10_2: f64 = 0.301029995663981;
fn pow5(exponent: i32) -> BigUint {
    BigUint::from(5u8).pow(exponent as u32)
}
fn scaled(factor: u64, five: i32, two: i32) -> BigUint {
    (BigUint::from(factor) * pow5(five)) << (two as usize)
}
fn five_bits(exponent: i32) -> i32 {
    if exponent == 0 {
        0
    } else if exponent < 27 {
        pow5(exponent).bits() as i32
    } else {
        exponent * 3
    }
}
fn round(digits: &mut [u8], exponent: &mut i32) {
    for digit in digits.iter_mut().rev() {
        if *digit < 9 {
            *digit += 1;
            return;
        }
        *digit = 0;
    }
    digits[0] = 1;
    *exponent += 1;
}
fn render(negative: bool, digits: &[u8], exponent: i32) -> String {
    let mut out = String::with_capacity(26);
    if negative {
        out.push('-');
    }
    let chars = |slice: &[u8]| {
        slice
            .iter()
            .map(|&digit| char::from(b'0' + digit))
            .collect::<String>()
    };
    if (1..8).contains(&exponent) {
        let point = (exponent as usize).min(digits.len());
        out.push_str(&chars(&digits[..point]));
        if point < exponent as usize {
            out.extend(std::iter::repeat_n('0', exponent as usize - point));
            out.push_str(".0");
        } else {
            out.push('.');
            if point == digits.len() {
                out.push('0');
            } else {
                out.push_str(&chars(&digits[point..]));
            }
        }
    } else if (-2..=0).contains(&exponent) {
        out.push_str("0.");
        out.extend(std::iter::repeat_n('0', (-exponent) as usize));
        out.push_str(&chars(digits));
    } else {
        out.push(char::from(b'0' + digits[0]));
        out.push('.');
        if digits.len() == 1 {
            out.push('0');
        } else {
            out.push_str(&chars(&digits[1..]));
        }
        out.push('E');
        out.push_str(&(exponent - 1).to_string());
    }
    out
}
pub fn format(value: f64) -> String {
    if value.is_nan() {
        return "NaN".into();
    }
    if value.is_infinite() {
        return if value.is_sign_negative() {
            "-Infinity"
        } else {
            "Infinity"
        }
        .into();
    }
    let raw = value.to_bits();
    let negative = raw >> 63 != 0;
    let mut fraction = raw & ((1u64 << 52) - 1);
    let mut binary = ((raw >> 52) & 2047) as i32;
    if binary == 0 && fraction == 0 {
        return if negative { "-0.0" } else { "0.0" }.into();
    }
    // Normalize subnormals while retaining their actual significant-bit count.
    let significant = if binary == 0 {
        let leading = fraction.leading_zeros() as i32;
        let shift = leading - 11;
        fraction <<= shift;
        binary = 1 - shift;
        64 - leading
    } else {
        fraction |= 1 << 52;
        53
    };
    binary -= 1023;
    let trailing = fraction.trailing_zeros() as i32;
    let bits = 53 - trailing;
    let tiny = (bits - binary - 1).max(0);
    // Integral values use the legacy decimal-place discard and half-up path.
    if (-21..=62).contains(&binary) && tiny == 0 {
        let discard = if binary > significant + 2 {
            (1u64 << (binary - significant - 1)).ilog10()
        } else {
            0
        };
        let integer = if binary >= 52 {
            fraction << (binary - 52)
        } else {
            fraction >> (52 - binary)
        };
        let power = 10u64.pow(discard);
        let rounded = integer / power + u64::from(integer % power >= power / 2 && discard > 0);
        let mut text = rounded.to_string();
        let exponent = text.len() as i32 + discard as i32;
        while text.ends_with('0') {
            text.pop();
        }
        return render(
            negative,
            &text.bytes().map(|b| b - b'0').collect::<Vec<_>>(),
            exponent,
        );
    }
    let normalized = f64::from_bits((1023u64 << 52) | (fraction & ((1 << 52) - 1)));
    let mut decimal =
        ((normalized - 1.5) * 0.289529654 + 0.176091259 + f64::from(binary) * ESTIMATE_LOG10_2)
            .floor() as i32;
    // B/S is the scaled significand; M is its half-ULP stopping margin.
    // Remove common binary factors before choosing fixed-width or big integers.
    let b5 = (-decimal).max(0);
    let s5 = decimal.max(0);
    let mut b2 = b5 + tiny + binary - (bits - 1);
    let mut s2 = s5 + tiny;
    let mut m2 = b5 + tiny + binary - significant;
    fraction >>= trailing;
    let common = b2.min(s2);
    b2 -= common;
    s2 -= common;
    m2 -= common;
    if bits == 1 {
        m2 -= 1;
    }
    if m2 < 0 {
        b2 -= m2;
        s2 -= m2;
        m2 = 0;
    }
    let bbits = bits + b2 + five_bits(b5);
    let sbits = s2 + 1 + five_bits(s5 + 1);
    let mut digits = Vec::with_capacity(19);
    let (low, high, difference) = if bbits < 64 && sbits < 64 {
        // Overflow and strict comparison here are observable legacy behavior.
        // The big-integer branch below uses an inclusive upper stopping test.
        let narrow = |v: i128| {
            if bbits < 32 && sbits < 32 {
                v as i32 as i64
            } else {
                v as i64
            }
        };
        let word = |five: i32, two: i32| 5i64.pow(five as u32) << two;
        let mut b = narrow(i128::from(fraction) * i128::from(word(b5, b2)));
        let s = word(s5, s2);
        let mut m = word(b5, m2);
        let ten_s = s * 10;
        let (mut low, mut high);
        let mut first = true;
        loop {
            let q = b / s;
            b = narrow(i128::from(b % s) * 10);
            m = narrow(i128::from(m) * 10);
            if !first && m <= 0 {
                low = true;
                high = true;
            } else {
                low = b < m;
                high = narrow(i128::from(b) + i128::from(m)) > ten_s;
            }
            if first && q == 0 && !high {
                decimal -= 1;
            } else {
                digits.push(q as u8);
            }
            if first && !(-3..8).contains(&decimal) {
                low = false;
                high = false;
            }
            first = false;
            if low || high {
                break;
            }
        }
        (
            low,
            high,
            narrow(i128::from(narrow(i128::from(b) * 2)) - i128::from(ten_s)).cmp(&0),
        )
    } else {
        let mut b = scaled(fraction, b5, b2);
        let s = scaled(1, s5, s2);
        let mut m = scaled(10, b5, m2);
        let ten_s = &s * 10u8;
        let (mut low, mut high);
        let mut first = true;
        loop {
            let q = (&b / &s).to_u32_digits().first().copied().unwrap_or(0) as u8;
            b = (&b % &s) * 10u8;
            if !first {
                m *= 10u8;
            }
            low = b < m;
            high = &b + &m >= ten_s;
            if first && q == 0 && !high {
                decimal -= 1;
            } else {
                digits.push(q);
            }
            if first && !(-3..8).contains(&decimal) {
                low = false;
                high = false;
            }
            first = false;
            if low || high {
                break;
            }
        }
        (low, high, (&b * 2u8).cmp(&ten_s))
    };
    let mut exponent = decimal + 1;
    if high
        && (!low || difference.is_gt() || (difference.is_eq() && digits.last().unwrap() % 2 != 0))
    {
        round(&mut digits, &mut exponent);
    }
    render(negative, &digits, exponent)
}
/// Java decimal grammar, trimming only ASCII control/space, with optional type suffix.
/// Hexadecimal literals are decoded with a single ties-to-even rounding step.
pub fn parse(text: &str) -> Option<f64> {
    let text = text.trim_matches(|ch: char| ch <= ' ');
    let (negative, unsigned) = if let Some(s) = text.strip_prefix('-') {
        (true, s)
    } else {
        (false, text.strip_prefix('+').unwrap_or(text))
    };
    if unsigned == "NaN" {
        return Some(f64::NAN);
    }
    if unsigned == "Infinity" {
        return Some(if negative {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        });
    }
    if unsigned.starts_with(['+', '-']) {
        return None;
    }
    let unsigned = unsigned
        .strip_suffix(['f', 'F', 'd', 'D'])
        .unwrap_or(unsigned);
    if let Some(hex) = unsigned
        .strip_prefix("0x")
        .or_else(|| unsigned.strip_prefix("0X"))
    {
        let (mantissa, exponent) = hex.split_once(['p', 'P'])?;
        let exponent = exponent.parse::<i64>().ok().or_else(|| {
            if !exponent
                .strip_prefix(['+', '-'])
                .unwrap_or(exponent)
                .is_empty()
                && exponent
                    .strip_prefix(['+', '-'])
                    .unwrap_or(exponent)
                    .bytes()
                    .all(|b| b.is_ascii_digit())
            {
                Some(if exponent.starts_with('-') {
                    i64::MIN
                } else {
                    i64::MAX
                })
            } else {
                None
            }
        })?;
        let mut parts = mantissa.split('.');
        let integral = parts.next()?;
        let fractional = parts.next().unwrap_or("");
        if parts.next().is_some() || integral.len() + fractional.len() == 0 {
            return None;
        }
        let digits = format!("{integral}{fractional}");
        if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let value = BigUint::parse_bytes(digits.as_bytes(), 16)?;
        let scale = exponent.saturating_sub((fractional.len() as i64).saturating_mul(4));
        let bits = value.bits() as i64;
        let top = bits.saturating_sub(1).saturating_add(scale);
        let sign = u64::from(negative) << 63;
        if bits == 0 || top < -1075 {
            return Some(f64::from_bits(sign));
        }
        if top > 1023 {
            return Some(f64::from_bits(sign | (2047 << 52)));
        }
        let target = if top < -1022 { -1074 } else { top - 52 };
        let shift = target.saturating_sub(scale);
        let rounded = if shift > 0 {
            let mut n = &value >> (shift as usize);
            let remainder = &value - (&n << (shift as usize));
            let half = BigUint::from(1u8) << ((shift - 1) as usize);
            if remainder > half || (remainder == half && n.bit(0)) {
                n += 1u8;
            }
            n
        } else {
            value << ((-shift) as usize)
        };
        let n = rounded.to_u64_digits().first().copied().unwrap_or(0);
        let raw = if top < -1022 {
            n
        } else {
            (((top + 1023) as u64) << 52).wrapping_add(n.wrapping_sub(1 << 52))
        };
        return Some(f64::from_bits(sign | raw));
    }
    if unsigned.is_empty()
        || !unsigned
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-'))
    {
        return None;
    }
    let number = unsigned.parse::<f64>().ok()?;
    Some(if negative { -number } else { number })
}
