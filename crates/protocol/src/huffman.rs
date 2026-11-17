//! Jagex's symbol-order Huffman assignment, used by revision 530 chat.
use crate::{Error, Result};

#[derive(Clone, Default)]
struct Node {
    children: [Option<usize>; 2],
    symbol: Option<u8>,
}

#[derive(Clone)]
pub struct Huffman {
    lengths: Vec<u8>,
    codewords: Vec<u32>,
    nodes: Vec<Node>,
}

impl Huffman {
    pub fn new(lengths: &[u8]) -> Result<Self> {
        if lengths.len() > 256 || lengths.iter().any(|&length| length > 32) {
            return Err(Error::new(0, "invalid Huffman alphabet"));
        }
        let mut result = Self {
            lengths: lengths.to_vec(),
            codewords: vec![0; lengths.len()],
            nodes: vec![Node::default()],
        };
        let mut next = [0u32; 33];
        for (symbol, &length) in lengths.iter().enumerate() {
            let length = usize::from(length);
            if length == 0 {
                continue;
            }
            let bit = 1u32 << (32 - length);
            let codeword = next[length];
            result.codewords[symbol] = codeword;
            let next_codeword = if codeword & bit == 0 {
                for parent in (1..length).rev() {
                    let parent_codeword = next[parent];
                    if parent_codeword != codeword {
                        break;
                    }
                    let mask = 1u32 << (32 - parent);
                    if parent_codeword & mask != 0 {
                        next[parent] = next[parent - 1];
                        break;
                    }
                    next[parent] = parent_codeword | mask;
                }
                codeword | bit
            } else {
                next[length - 1]
            };
            next[length] = next_codeword;
            for longer in &mut next[length + 1..] {
                if *longer == codeword {
                    *longer = next_codeword;
                }
            }
            let mut node = 0;
            for offset in 0..length {
                if result.nodes[node].symbol.is_some() {
                    return Err(Error::new(symbol, "overlapping Huffman codewords"));
                }
                let branch = ((codeword >> (31 - offset)) & 1) as usize;
                node = match result.nodes[node].children[branch] {
                    Some(child) => child,
                    None => {
                        let child = result.nodes.len();
                        result.nodes.push(Node::default());
                        result.nodes[node].children[branch] = Some(child);
                        child
                    }
                };
            }
            if result.nodes[node].symbol.is_some()
                || result.nodes[node].children.iter().any(Option::is_some)
            {
                return Err(Error::new(symbol, "overlapping Huffman codewords"));
            }
            result.nodes[node].symbol = Some(symbol as u8);
        }
        Ok(result)
    }

    pub fn encode(&self, source: &[u8]) -> Result<Vec<u8>> {
        let bits = source.iter().try_fold(0usize, |total, &symbol| {
            let length = *self
                .lengths
                .get(usize::from(symbol))
                .ok_or(Error::new(total / 8, "missing Huffman symbol"))?;
            if length == 0 {
                return Err(Error::new(total / 8, "missing Huffman codeword"));
            }
            total
                .checked_add(usize::from(length))
                .ok_or(Error::new(total / 8, "Huffman output length overflow"))
        })?;
        let mut output = vec![0; bits.div_ceil(8)];
        let mut position = 0;
        for &symbol in source {
            let symbol = usize::from(symbol);
            for offset in 0..self.lengths[symbol] {
                output[position / 8] |=
                    (((self.codewords[symbol] >> (31 - offset)) & 1) as u8) << (7 - position % 8);
                position += 1;
            }
        }
        Ok(output)
    }

    /// Decode `count` symbols; return bytes and whole input bytes consumed.
    pub fn decode(&self, source: &[u8], count: usize) -> Result<(Vec<u8>, usize)> {
        let available_bits = source
            .len()
            .checked_mul(8)
            .ok_or(Error::new(0, "Huffman input length overflow"))?;
        if count > available_bits {
            return Err(Error::new(0, "Huffman output exceeds possible symbols"));
        }
        let mut output = Vec::with_capacity(count);
        let mut position = 0;
        let mut node = 0;
        while output.len() < count {
            let byte = *source
                .get(position / 8)
                .ok_or(Error::new(position / 8, "truncated Huffman data"))?;
            let branch = usize::from((byte >> (7 - position % 8)) & 1);
            node = self.nodes[node].children[branch]
                .ok_or(Error::new(position / 8, "invalid Huffman codeword"))?;
            position += 1;
            if let Some(symbol) = self.nodes[node].symbol {
                output.push(symbol);
                node = 0;
            }
        }
        Ok((output, position.div_ceil(8)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_ambiguous_tables_and_missing_symbols() {
        assert!(Huffman::new(&[1, 1, 1]).is_err());
        assert!(Huffman::new(&[33]).is_err());
        assert!(Huffman::new(&[0]).unwrap().encode(&[0]).is_err());
        assert!(Huffman::new(&[1]).unwrap().decode(&[128], 1).is_err());
        assert!(Huffman::new(&[1]).unwrap().decode(&[], usize::MAX).is_err());
    }
}
