//! Original EncryptionManager BCrypt format on RustCrypto Blowfish primitives.
use super::{AuthError, Result};
use blowfish::Blowfish;
use std::sync::Arc;
const ALPHABET: &[u8] = b"./ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
pub const DEFAULT_COST: u32 = 12;
pub trait PasswordCodec: Send + Sync {
    fn hash(&self, password: &str) -> Result<String>;
    fn verify(&self, password: &str, stored: &str) -> Result<bool>;
}
pub struct Passwords {
    cost: u32,
    salt_source: Arc<dyn Fn() -> Result<[u8; 16]> + Send + Sync>,
}
impl Default for Passwords {
    fn default() -> Self {
        Self {
            cost: DEFAULT_COST,
            salt_source: Arc::new(|| {
                let mut salt = [0; 16];
                getrandom::fill(&mut salt).map_err(|_| AuthError::Entropy)?;
                Ok(salt)
            }),
        }
    }
}
impl Passwords {
    pub fn with_salt_source(
        cost: u32,
        source: impl Fn() -> Result<[u8; 16]> + Send + Sync + 'static,
    ) -> Result<Self> {
        if !(4..=31).contains(&cost) {
            return Err(AuthError::InvalidCost);
        }
        Ok(Self {
            cost,
            salt_source: Arc::new(source),
        })
    }
}
impl PasswordCodec for Passwords {
    fn hash(&self, password: &str) -> Result<String> {
        hash_with_salt(password, &salt_string(self.cost, (self.salt_source)()?)?)
    }
    fn verify(&self, password: &str, stored: &str) -> Result<bool> {
        verify(password, stored)
    }
}
fn encode(bytes: &[u8]) -> String {
    let mut out = String::new();
    let mut bits = 0u32;
    let mut count = 0;
    for &byte in bytes {
        bits = (bits << 8) | u32::from(byte);
        count += 8;
        while count >= 6 {
            count -= 6;
            out.push(ALPHABET[((bits >> count) & 63) as usize] as char);
        }
    }
    if count > 0 {
        out.push(ALPHABET[((bits << (6 - count)) & 63) as usize] as char);
    }
    out
}
fn decode_salt(text: &[u8]) -> Result<[u8; 16]> {
    let mut output = Vec::new();
    let mut bits = 0u32;
    let mut count = 0;
    for &byte in text {
        let value = ALPHABET
            .iter()
            .position(|&entry| entry == byte)
            .ok_or(AuthError::InvalidSalt)? as u32;
        bits = (bits << 6) | value;
        count += 6;
        if count >= 8 {
            count -= 8;
            output.push((bits >> count) as u8);
            if output.len() == 16 {
                break;
            }
        }
    }
    output.try_into().map_err(|_| AuthError::InvalidSalt)
}
pub fn salt_string(cost: u32, salt: [u8; 16]) -> Result<String> {
    if !(4..=31).contains(&cost) {
        return Err(AuthError::InvalidCost);
    }
    Ok(format!("$2a${cost:02}${}", encode(&salt)))
}
pub fn hash_with_salt(password: &str, salt: &str) -> Result<String> {
    let text = salt.as_bytes();
    if !text.starts_with(b"$2") {
        return Err(AuthError::InvalidHash);
    }
    let minor = match text.get(2..4) {
        Some(b"a$") => true,
        Some(bytes) if bytes[0] == b'$' => false,
        _ => return Err(AuthError::InvalidHash),
    };
    let offset = if minor { 4 } else { 3 };
    let prefix = text.get(offset..offset + 3).ok_or(AuthError::InvalidSalt)?;
    if prefix[2] > b'$' {
        return Err(AuthError::InvalidSalt);
    }
    let digits = std::str::from_utf8(&prefix[..2]).map_err(|_| AuthError::InvalidCost)?;
    let cost = digits.parse::<i32>().map_err(|_| AuthError::InvalidCost)?;
    if !(4..=31).contains(&cost) {
        return Err(AuthError::InvalidCost);
    }
    let salt = decode_salt(
        text.get(offset + 3..offset + 25)
            .ok_or(AuthError::InvalidSalt)?,
    )?;
    let mut key = password.as_bytes().to_vec();
    if minor {
        key.push(0);
    }
    if key.is_empty() {
        return Err(AuthError::InvalidHash);
    }
    key.truncate(72);
    let mut state = Blowfish::bc_init_state();
    state.salted_expand_key(&salt, &key);
    // Java's signed int overflow makes cost 31 perform zero expansion rounds.
    for _ in 0..1i32.wrapping_shl(cost as u32) {
        state.bc_expand_key(&key);
        state.bc_expand_key(&salt);
    }
    let mut data = [
        0x4f727068, 0x65616e42, 0x65686f6c, 0x64657253, 0x63727944, 0x6f756274,
    ];
    for pair in data.chunks_exact_mut(2) {
        for _ in 0..64 {
            let [left, right] = state.bc_encrypt([pair[0], pair[1]]);
            pair[0] = left;
            pair[1] = right;
        }
    }
    let bytes: Vec<_> = data.into_iter().flat_map(u32::to_be_bytes).collect();
    Ok(format!(
        "$2{}${cost:02}${}{}",
        if minor { "a" } else { "" },
        encode(&salt),
        encode(&bytes[..23])
    ))
}
pub fn verify(password: &str, stored: &str) -> Result<bool> {
    if !stored.starts_with("$2a$") {
        return Err(AuthError::InvalidHash);
    }
    let actual = hash_with_salt(password, stored)?;
    if actual.len() != stored.len() {
        return Ok(false);
    }
    let difference = actual
        .bytes()
        .zip(stored.bytes())
        .fold(0u8, |difference, (a, b)| difference | (a ^ b));
    Ok(difference == 0)
}
