//! RT4 Buffer's byte/bit formats. Strings remain bytes until the text layer decodes them.
use crate::{Error, Result};

#[derive(Clone)]
pub struct Reader<'a> {
    data: &'a [u8],
    position: usize,
}
impl<'a> Reader<'a> {
    pub const fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }
    pub const fn position(&self) -> usize {
        self.position
    }
    pub fn remaining(&self) -> usize {
        self.data.len() - self.position
    }
    pub fn seek(&mut self, position: usize) -> Result<()> {
        if position > self.data.len() {
            return Err(Error::new(position, "seek past end"));
        }
        self.position = position;
        Ok(())
    }
    pub fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(Error::new(self.position, "length overflow"))?;
        let bytes = self
            .data
            .get(self.position..end)
            .ok_or(Error::new(self.position, "truncated buffer"))?;
        self.position = end;
        Ok(bytes)
    }
    pub fn g1(&mut self) -> Result<i32> {
        Ok(i32::from(self.take(1)?[0]))
    }
    pub fn g1b(&mut self) -> Result<i32> {
        Ok(i32::from(self.take(1)?[0] as i8))
    }
    pub fn g1add(&mut self) -> Result<i32> {
        Ok((self.g1()? - 128) & 255)
    }
    pub fn g1neg(&mut self) -> Result<i32> {
        Ok((-self.g1()?) & 255)
    }
    pub fn g1sub(&mut self) -> Result<i32> {
        Ok((128 - self.g1()?) & 255)
    }
    pub fn g1badd(&mut self) -> Result<i32> {
        Ok(i32::from(self.g1add()? as i8))
    }
    pub fn g1bneg(&mut self) -> Result<i32> {
        Ok(i32::from(self.g1neg()? as i8))
    }
    pub fn g1bsub(&mut self) -> Result<i32> {
        Ok(i32::from(self.g1sub()? as i8))
    }
    pub fn g2(&mut self) -> Result<i32> {
        let b = self.take(2)?;
        Ok(i32::from(u16::from_be_bytes([b[0], b[1]])))
    }
    pub fn ig2(&mut self) -> Result<i32> {
        let b = self.take(2)?;
        Ok(i32::from(u16::from_le_bytes([b[0], b[1]])))
    }
    pub fn g2add(&mut self) -> Result<i32> {
        let b = self.take(2)?;
        Ok((i32::from(b[0]) << 8) + i32::from(b[1].wrapping_sub(128)))
    }
    pub fn ig2add(&mut self) -> Result<i32> {
        let b = self.take(2)?;
        Ok((i32::from(b[1]) << 8) + i32::from(b[0].wrapping_sub(128)))
    }
    pub fn g2b(&mut self) -> Result<i32> {
        Ok(i32::from(self.g2()? as i16))
    }
    pub fn ig2b(&mut self) -> Result<i32> {
        Ok(i32::from(self.ig2()? as i16))
    }
    pub fn g2badd(&mut self) -> Result<i32> {
        Ok(i32::from(self.g2add()? as i16))
    }
    pub fn ig2badd(&mut self) -> Result<i32> {
        Ok(i32::from(self.ig2add()? as i16))
    }
    pub fn g3(&mut self) -> Result<i32> {
        let b = self.take(3)?;
        Ok((i32::from(b[0]) << 16) | (i32::from(b[1]) << 8) | i32::from(b[2]))
    }
    /// Preserve RT4's existing extra shift, rather than silently correcting ig3.
    pub fn ig3(&mut self) -> Result<i32> {
        let b = self.take(3)?;
        Ok((i32::from(b[2]) << 24) | (i32::from(b[1]) << 8) | i32::from(b[0]))
    }
    pub fn g4(&mut self) -> Result<i32> {
        let b = self.take(4)?;
        Ok(i32::from_be_bytes(b.try_into().expect("four bytes")))
    }
    pub fn ig4(&mut self) -> Result<i32> {
        let b = self.take(4)?;
        Ok(i32::from_le_bytes(b.try_into().expect("four bytes")))
    }
    pub fn mg4(&mut self) -> Result<i32> {
        let b = self.take(4)?;
        Ok(i32::from_be_bytes([b[1], b[0], b[3], b[2]]))
    }
    pub fn img4(&mut self) -> Result<i32> {
        let b = self.take(4)?;
        Ok(i32::from_be_bytes([b[2], b[3], b[0], b[1]]))
    }
    pub fn g8(&mut self) -> Result<i64> {
        let b = self.take(8)?;
        Ok(i64::from_be_bytes(b.try_into().expect("eight bytes")))
    }
    pub fn gsmart(&mut self) -> Result<i32> {
        let first = *self
            .data
            .get(self.position)
            .ok_or(Error::new(self.position, "missing smart"))?;
        if first < 128 {
            Ok(self.g1()? - 64)
        } else {
            Ok(self.g2()? - 0xc000)
        }
    }
    pub fn gsmarts(&mut self) -> Result<i32> {
        let first = *self
            .data
            .get(self.position)
            .ok_or(Error::new(self.position, "missing smart"))?;
        if first < 128 {
            self.g1()
        } else {
            Ok(self.g2()? - 0x8000)
        }
    }
    pub fn g_var_smart(&mut self) -> Result<i32> {
        let mut total = 0i32;
        loop {
            let value = self.gsmarts()?;
            total = total.wrapping_add(value);
            if value != 32767 {
                return Ok(total);
            }
        }
    }
    pub fn g_var_int(&mut self) -> Result<i32> {
        let mut value = 0i32;
        for _ in 0..5 {
            let b = self.g1()?;
            if b < 128 {
                return Ok(value | b);
            }
            value = (value | (b & 127)).wrapping_shl(7);
        }
        Err(Error::new(self.position, "varint exceeds five bytes"))
    }
    pub fn g_var_long(&mut self, length: usize) -> Result<i64> {
        if !(1..=8).contains(&length) {
            return Err(Error::new(self.position, "invalid varlong length"));
        }
        let mut result = 0u64;
        for b in self.take(length)? {
            result = (result << 8) | u64::from(*b);
        }
        Ok(result as i64)
    }
    pub fn gjstr(&mut self) -> Result<&'a [u8]> {
        let len = self.data[self.position..]
            .iter()
            .position(|b| *b == 0)
            .ok_or(Error::new(self.position, "unterminated string"))?;
        let bytes = self.take(len)?;
        self.take(1)?;
        Ok(bytes)
    }
    pub fn gjstr2(&mut self) -> Result<&'a [u8]> {
        if self.g1()? != 0 {
            return Err(Error::new(self.position - 1, "bad gjstr2 version"));
        }
        self.gjstr()
    }
    pub fn fast_gjstr(&mut self) -> Result<Option<&'a [u8]>> {
        if self.data.get(self.position) == Some(&0) {
            self.position += 1;
            Ok(None)
        } else {
            self.gjstr().map(Some)
        }
    }
    pub fn bits(&mut self) -> BitReader<'_, 'a> {
        let bit = self.position * 8;
        BitReader { reader: self, bit }
    }
}
pub struct BitReader<'r, 'a> {
    reader: &'r mut Reader<'a>,
    bit: usize,
}
impl BitReader<'_, '_> {
    pub fn read(&mut self, count: usize) -> Result<u32> {
        if count > 32
            || self
                .bit
                .checked_add(count)
                .is_none_or(|end| end > self.reader.data.len() * 8)
        {
            return Err(Error::new(self.bit / 8, "truncated or invalid bit field"));
        }
        let mut value = 0;
        let end = self.bit + count;
        while self.bit < end {
            value = (value << 1)
                | u32::from((self.reader.data[self.bit / 8] >> (7 - self.bit % 8)) & 1);
            self.bit += 1;
        }
        Ok(value)
    }
    pub fn remaining(&self) -> usize {
        self.reader.data.len() * 8 - self.bit
    }
    pub fn finish(self) {
        self.reader.position = self.bit.div_ceil(8);
    }
}

#[derive(Clone, Default)]
pub struct Writer {
    data: Vec<u8>,
}
impl Writer {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }
    pub fn into_vec(self) -> Vec<u8> {
        self.data
    }
    pub fn p1(&mut self, n: i32) {
        self.data.push(n as u8);
    }
    pub fn p1add(&mut self, n: i32) {
        self.p1(n.wrapping_add(128));
    }
    pub fn p1neg(&mut self, n: i32) {
        self.p1(n.wrapping_neg());
    }
    pub fn p1sub(&mut self, n: i32) {
        self.p1(128i32.wrapping_sub(n));
    }
    pub fn p2(&mut self, n: i32) {
        self.data.extend((n as u16).to_be_bytes());
    }
    pub fn p2add(&mut self, n: i32) {
        self.p1(n >> 8);
        self.p1add(n);
    }
    pub fn ip2(&mut self, n: i32) {
        self.data.extend((n as u16).to_le_bytes());
    }
    pub fn ip2add(&mut self, n: i32) {
        self.p1add(n);
        self.p1(n >> 8);
    }
    pub fn p3(&mut self, n: i32) {
        self.p1(n >> 16);
        self.p1(n >> 8);
        self.p1(n);
    }
    pub fn p4(&mut self, n: i32) {
        self.data.extend(n.to_be_bytes());
    }
    pub fn ip4(&mut self, n: i32) {
        self.data.extend(n.to_le_bytes());
    }
    pub fn mp4(&mut self, n: i32) {
        let b = n.to_be_bytes();
        self.data.extend([b[1], b[0], b[3], b[2]]);
    }
    pub fn imp4(&mut self, n: i32) {
        let b = n.to_be_bytes();
        self.data.extend([b[2], b[3], b[0], b[1]]);
    }
    pub fn p8(&mut self, n: i64) {
        self.data.extend(n.to_be_bytes());
    }
    pub fn p_var_int(&mut self, n: i32) {
        let n = n as u32;
        let mut started = false;
        for shift in [28, 21, 14, 7] {
            if n >> shift != 0 || started {
                self.p1(((n >> shift) | 128) as i32);
                started = true;
            }
        }
        self.p1((n & 127) as i32);
    }
    pub fn p_var_long(&mut self, length: usize, value: i64) -> Result<()> {
        if !(1..=8).contains(&length) {
            return Err(Error::new(self.data.len(), "invalid varlong length"));
        }
        self.data.extend(&value.to_be_bytes()[8 - length..]);
        Ok(())
    }
    pub fn psmarts(&mut self, n: i32) -> Result<()> {
        match n {
            0..=127 => self.p1(n),
            128..=32767 => self.p2(n + 32768),
            _ => return Err(Error::new(self.data.len(), "smart out of range")),
        }
        Ok(())
    }
    pub fn pjstr(&mut self, bytes: &[u8]) -> Result<()> {
        if bytes.contains(&0) {
            return Err(Error::new(self.data.len(), "string contains terminator"));
        }
        self.data.extend(bytes);
        self.data.push(0);
        Ok(())
    }
    pub fn pdata(&mut self, bytes: &[u8]) {
        self.data.extend(bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn truncated_reads_do_not_advance() {
        let mut reader = Reader::new(&[1]);
        assert!(reader.g4().is_err());
        assert_eq!(reader.position(), 0);
        assert!(reader.gjstr().is_err());
        assert_eq!(reader.position(), 0);
        assert!(reader.bits().read(9).is_err());
        assert_eq!(reader.position(), 0);
    }
    #[test]
    fn malformed_lengths_and_unterminated_varints_are_rejected() {
        assert!(Reader::new(&[255; 8]).g_var_int().is_err());
        assert!(Reader::new(&[1]).take(usize::MAX).is_err());
        assert!(Writer::new().psmarts(-1).is_err());
        assert!(Writer::new().p_var_long(9, 0).is_err());
    }
}
