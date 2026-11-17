//! Revision 530 public chat with the original wordpack and actor mask framing.
use super::Frame;
use crate::{Error, Result, buffer::Reader, huffman::Huffman};
use std::sync::LazyLock;

pub const PUBLIC: u8 = 237;
pub static WORDPACK: LazyLock<Huffman> = LazyLock::new(|| {
    Huffman::new(include_bytes!(
        "../../../../fixtures/public-chat/huffman.bin"
    ))
    .expect("pinned revision 530 Huffman table")
});

/// Only one bounded, well-formed chat body reaches the mask producer.
pub fn public_mask(frame: &Frame) -> Result<Option<(u16, Vec<u8>)>> {
    let mut input = Reader::new(&frame.payload);
    let color = input.g1b()?.clamp(0, 11) as u16;
    let effect = input.g1b()?.clamp(0, 5) as u16;
    let count = input.gsmarts()? as usize;
    if !(1..=127).contains(&count) {
        return Ok(None);
    }
    let compressed = input.take(input.remaining())?;
    if compressed.len() > 253 {
        return Err(Error::new(0, "public chat packet exceeds its byte length"));
    }
    let (body, used) = WORDPACK.decode(compressed, count)?;
    if body.iter().all(u8::is_ascii_whitespace) || body.contains(&0) {
        return Ok(None);
    }
    let mut payload = Vec::with_capacity(used + 2);
    payload.push(count as u8);
    payload.extend_from_slice(&compressed[..used]);
    // PlayerFlags530 writes one final zero with IoBuffer.putReverse's inclusive bound.
    payload.push(0);
    Ok(Some((color << 8 | effect, payload)))
}

pub fn public_text(payload: &[u8]) -> Result<Vec<u8>> {
    let mut r = Reader::new(payload);
    let count = r.gsmarts()? as usize;
    if count > 256 {
        return Err(Error::new(0, "public chat message is too long"));
    }
    let (mut bytes, _) = WORDPACK.decode(r.take(r.remaining())?, count)?;
    let mut state = 2;
    for c in &mut bytes {
        match *c {
            b'a'..=b'z' | 224..=246 | 248..=254 => {
                if state == 2 {
                    *c -= 32;
                }
                state = 0;
            }
            b'A'..=b'Z' | 192..=214 | 216..=222 => {
                if state == 0 {
                    *c += 32;
                }
                state = 0;
            }
            b'.' | b'!' | b'?' => state = 2,
            b' ' if state == 2 => (),
            _ => state = 1,
        }
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_chat_wordpack_and_mask_padding_round_trip() {
        let mut bytes = vec![1, 2, 20];
        bytes.extend(WORDPACK.encode(b"hELLO there! tESTING").unwrap());
        let (effects, mask) = public_mask(&Frame {
            opcode: PUBLIC,
            payload: bytes,
        })
        .unwrap()
        .unwrap();
        assert_eq!(effects, 258);
        assert_eq!(mask.last(), Some(&0));
        assert_eq!(public_text(&mask).unwrap(), b"Hello there! Testing");
        assert!(
            public_mask(&Frame {
                opcode: PUBLIC,
                payload: vec![0, 0, 127]
            })
            .is_err()
        );
    }
}
