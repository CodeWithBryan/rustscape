use crate::{Error, Result};

/// Buffer.tinydec decrypts complete blocks after the five-byte JS5 header.
/// Any partial final block and the header remain unchanged.
pub fn decrypt_js5(bytes: &mut [u8], length: usize, key: [u32; 4]) -> Result<()> {
    if length < 5 || length > bytes.len() {
        return Err(Error::new(length, "invalid XTEA container length"));
    }
    let block_end = 5 + (length - 5) / 8 * 8;
    for block in bytes[5..block_end].chunks_exact_mut(8) {
        let mut v0 = u32::from_be_bytes(block[..4].try_into().expect("four bytes"));
        let mut v1 = u32::from_be_bytes(block[4..].try_into().expect("four bytes"));
        let mut sum = 0xc6ef3720u32;
        for _ in 0..32 {
            v1 = v1.wrapping_sub(
                key[((sum >> 11) & 3) as usize].wrapping_add(sum)
                    ^ v0.wrapping_add((v0 >> 5) ^ (v0 << 4)),
            );
            sum = sum.wrapping_sub(0x9e3779b9);
            v0 = v0.wrapping_sub(
                ((v1 >> 5) ^ (v1 << 4)).wrapping_add(v1)
                    ^ key[(sum & 3) as usize].wrapping_add(sum),
            );
        }
        block[..4].copy_from_slice(&v0.to_be_bytes());
        block[4..].copy_from_slice(&v1.to_be_bytes());
    }
    Ok(())
}
