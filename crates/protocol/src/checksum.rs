const fn crc32_table() -> [u32; 256] {
    let mut table = [0; 256];
    let mut i = 0;
    while i < 256 {
        let mut value = i as u32;
        let mut n = 0;
        while n < 8 {
            value = if value & 1 != 0 {
                (value >> 1) ^ 0xedb88320
            } else {
                value >> 1
            };
            n += 1;
        }
        table[i] = value;
        i += 1;
    }
    table
}
const fn crc64_table() -> [u64; 256] {
    let mut table = [0; 256];
    let mut i = 0;
    while i < 256 {
        let mut value = i as u64;
        let mut n = 0;
        while n < 8 {
            value = if value & 1 != 0 {
                (value >> 1) ^ 0xc96c5795d7870f42
            } else {
                value >> 1
            };
            n += 1;
        }
        table[i] = value;
        i += 1;
    }
    table
}
const CRC32: [u32; 256] = crc32_table();
const CRC64: [u64; 256] = crc64_table();
pub fn crc32(bytes: &[u8]) -> u32 {
    !bytes.iter().fold(u32::MAX, |crc, b| {
        (crc >> 8) ^ CRC32[((crc ^ u32::from(*b)) & 255) as usize]
    })
}
pub fn crc64(bytes: &[u8]) -> u64 {
    !bytes.iter().fold(u64::MAX, |crc, b| {
        (crc >> 8) ^ CRC64[((crc ^ u64::from(*b)) & 255) as usize]
    })
}
