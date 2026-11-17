//! Java i32 wrapping and RT4 IntUtils quirks are deliberate compatibility behavior.
pub fn pow(exponent: i32, mut base: i32) -> i32 {
    let mut exponent = exponent;
    let mut result = 1i32;
    while exponent > 1 {
        if exponent & 1 != 0 {
            result = result.wrapping_mul(base);
        }
        base = base.wrapping_mul(base);
        exponent >>= 1;
    }
    if exponent == 1 {
        result.wrapping_mul(base)
    } else {
        result
    }
}
pub fn clp2(value: i32) -> i32 {
    let mut n = value.wrapping_sub(1) as u32;
    for shift in [1, 2, 4, 8, 16] {
        n |= n >> shift;
    }
    n.wrapping_add(1) as i32
}
pub fn bit_count(value: i32) -> i32 {
    (32 - (value as u32).leading_zeros()) as i32
}
pub fn clamp(max: i32, value: i32, min: i32) -> i32 {
    if min > value {
        min
    } else if value > max {
        max
    } else {
        value
    }
}
pub fn bit_reverse(bits: i32, value: i32) -> i32 {
    if bits <= 0 || bits >= 64 {
        0
    } else if bits <= 32 {
        ((value as u32).reverse_bits() >> (32 - bits)) as i32
    } else {
        ((value as u32).reverse_bits() << (bits - 32)) as i32
    }
}
pub fn bit_count_fast(value: i32) -> i32 {
    (value as u32).count_ones() as i32
}
/// RT4 reports true for zero and Integer.MIN_VALUE too.
pub fn is_power_of_two(value: i32) -> bool {
    value == (value.wrapping_neg() & value)
}
