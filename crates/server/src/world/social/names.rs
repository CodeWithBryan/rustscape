//! The `StringUtils` and `CP1252` members the social code calls: name longs,
//! display names and the server's own chat compression. Strings are the
//! original's UTF-16 units; a Rust `String` holds them as scalar values.

/// `StringUtils.VALID_CHARS`.
const VALID_CHARS: &[u8; 37] = b"_abcdefghijklmnopqrstuvwxyz0123456789";

/// `CP1252.CHAR_MAP`.
const CHAR_MAP: [u16; 32] = [
    0x20ac, 0, 0x201a, 0x0192, 0x201e, 0x2026, 0x2020, 0x2021, 0x02c6, 0x2030, 0x0160, 0x2039,
    0x0152, 0, 0x017d, 0, 0, 0x2018, 0x2019, 0x201c, 0x201d, 0x2022, 0x2013, 0x2014, 0x02dc,
    0x2122, 0x0161, 0x203a, 0x0153, 0, 0x017e, 0x0178,
];

/// `StringUtils.stringToLong`: at most twelve units; letters of either case,
/// digits, and every other unit counts as `_`; trailing `_` are removed.
pub fn string_to_long(text: &str) -> i64 {
    let mut value = 0i64;
    for unit in text.encode_utf16().take(12) {
        value = value.wrapping_mul(37);
        value = value.wrapping_add(match unit {
            0x41..=0x5a => i64::from(unit) + 1 - 65,
            0x61..=0x7a => i64::from(unit) + 1 - 97,
            0x30..=0x39 => i64::from(unit) + 27 - 48,
            _ => 0,
        });
    }
    while value % 37 == 0 && value != 0 {
        value /= 37;
    }
    value
}

/// `StringUtils.longToString`. A negative value or one of more than twelve
/// digits indexes outside its arrays; the original catches that and returns
/// an empty string.
pub fn long_to_string(mut value: i64) -> String {
    let mut chars = [0u8; 32];
    let mut i: i64 = 0;
    while value != 0 {
        let before = value;
        value /= 37;
        let index = before.wrapping_sub(value.wrapping_mul(37));
        let slot = 11 - i;
        i += 1;
        if !(0..37).contains(&index) || !(0..32).contains(&slot) {
            return String::new();
        }
        chars[slot as usize] = VALID_CHARS[index as usize];
    }
    // `new String(ac, 12 - i, i)` with i <= 12 here.
    String::from_utf8(chars[(12 - i) as usize..12].to_vec()).expect("name alphabet")
}

/// `StringUtils.formatDisplayName`: underscores to spaces, default-locale
/// lower case (en-US, the same as ROOT), then the first unit and each unit
/// after a space upper-cased as a one-unit string.
pub fn format_display_name(name: &str) -> String {
    let lowered = rs09_game_core::java_lower::lowercase_root(&name.replace('_', " "));
    let units: Vec<u16> = lowered.encode_utf16().collect();
    let mut out: Vec<u16> = Vec::with_capacity(units.len());
    let mut was_space = true;
    for &unit in &units {
        if was_space {
            out.extend(upper_unit(unit));
            was_space = false;
        } else {
            out.push(unit);
        }
        if unit == u16::from(b' ') {
            was_space = true;
        }
    }
    String::from_utf16_lossy(&out)
}

/// `String.toUpperCase` of a one-unit string. Units outside the BMP scalar
/// range (lone surrogates) are returned unchanged, as the JDK does.
fn upper_unit(unit: u16) -> Vec<u16> {
    match char::from_u32(u32::from(unit)) {
        Some(ch) => ch
            .to_uppercase()
            .collect::<String>()
            .encode_utf16()
            .collect(),
        None => vec![unit],
    }
}

/// `CP1252.getByte`.
pub fn cp1252_byte(unit: u16) -> u8 {
    if (unit > 0 && unit < 128) || (160..=255).contains(&unit) {
        return unit as u8;
    }
    for (i, &mapped) in CHAR_MAP.iter().enumerate() {
        if mapped != 0 && mapped == unit {
            return (i + 128) as u8;
        }
    }
    63
}

/// `CP1252.toBytes`: one byte per UTF-16 unit.
pub fn cp1252_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16().map(cp1252_byte).collect()
}

/// `CP1252.getFromByte`; `None` where the original throws for a zero byte.
pub fn cp1252_char(value: u8) -> Option<u16> {
    let mut out = u16::from(value);
    if out == 0 {
        return None;
    }
    if (128..160).contains(&out) {
        let mut mapped = CHAR_MAP[usize::from(out - 128)];
        if mapped == 0 {
            mapped = 63;
        }
        out = mapped;
    }
    Some(out)
}

/// `StringUtils.encryptPlayerChat(is, i_25_, i_26_, i_27_, is_28_)` into the
/// 256-byte array its callers allocate. A write or read outside the arrays is
/// a RuntimeException that the original catches, returning 0 and keeping the
/// bytes already written.
pub fn encrypt_player_chat(
    out: &mut [u8; 256],
    input_offset: i32,
    output_offset: i32,
    length: i32,
    input: &[u8],
) -> i32 {
    let mut i25 = input_offset;
    let i27 = length.wrapping_add(i25);
    let mut i29: i32 = 0;
    let mut i30: i32 = output_offset << 3;
    let put = |out: &mut [u8; 256], index: i32, value: i32| -> bool {
        if (0..256).contains(&index) {
            out[index as usize] = value as u8;
            true
        } else {
            false
        }
    };
    while i27 > i25 {
        let Some(&byte) = usize::try_from(i25).ok().and_then(|i| input.get(i)) else {
            return 0;
        };
        let i31 = usize::from(byte);
        let i32_ = CODES[i31];
        let i33 = i32::from(LENGTHS[i31]);
        let mut i34 = i30 >> 3;
        let mut i35 = i30 & 7;
        i29 &= i35.wrapping_neg() >> 31;
        i30 = i30.wrapping_add(i33);
        let i36 = ((-1 + (i35 + i33)) >> 3) + i34;
        i35 += 24;
        i29 |= ((i32_ as u32) >> (i35 & 31)) as i32;
        if !put(out, i34, i29) {
            return 0;
        }
        if i36 > i34 {
            i34 += 1;
            i35 -= 8;
            i29 = ((i32_ as u32) >> (i35 & 31)) as i32;
            if !put(out, i34, i29) {
                return 0;
            }
            if i36 > i34 {
                i34 += 1;
                i35 -= 8;
                i29 = ((i32_ as u32) >> (i35 & 31)) as i32;
                if !put(out, i34, i29) {
                    return 0;
                }
                if i36 > i34 {
                    i35 -= 8;
                    i34 += 1;
                    i29 = ((i32_ as u32) >> (i35 & 31)) as i32;
                    if !put(out, i34, i29) {
                        return 0;
                    }
                    if i34 < i36 {
                        i35 -= 8;
                        i34 += 1;
                        i29 = i32_.wrapping_shl(i35.wrapping_neg() as u32 & 31);
                        if !put(out, i34, i29) {
                            return 0;
                        }
                    }
                }
            }
        }
        i25 += 1;
    }
    -output_offset + ((7 + i30) >> 3)
}

/// `StringUtils.decryptPlayerChat(buffer, totalChars)` over the bytes after
/// `position`. Returns the text and the new position. Any RuntimeException
/// (reading past the end, a tree index outside the table, a zero byte) is
/// caught by the original, which prints it and returns an empty string; the
/// position then stays where the failed read left it.
pub fn decrypt_player_chat(bytes: &[u8], position: &mut usize, total: i32) -> String {
    if total == 0 {
        return String::new();
    }
    let mut decoded = 0i32;
    let mut node: i32 = 0;
    let mut out: Vec<u16> = Vec::new();
    loop {
        let Some(&byte) = bytes.get(*position) else {
            return String::new();
        };
        *position += 1;
        let signed = byte as i8;
        for bit in 0..8 {
            let set = if bit == 0 {
                signed < 0
            } else {
                byte & (0x80 >> bit) != 0
            };
            if set {
                let Some(&next) = usize::try_from(node).ok().and_then(|i| DECODE_TREE.get(i))
                else {
                    return String::new();
                };
                node = next;
            } else {
                node += 1;
            }
            let Some(&leaf) = usize::try_from(node).ok().and_then(|i| DECODE_TREE.get(i)) else {
                return String::new();
            };
            if leaf < 0 {
                let Some(unit) = cp1252_char(!leaf as u8) else {
                    return String::new();
                };
                out.push(unit);
                decoded += 1;
                if decoded >= total {
                    return String::from_utf16_lossy(&out);
                }
                node = 0;
            }
        }
    }
}

/// `StringUtils.anIntArray241`: the decoding tree of decryptPlayerChat.
const DECODE_TREE: [i32; 512] = [
    215, 203, 83, 158, 104, 101, 93, 84, 107, 103, 109, 95, 94, 98, 89, 86, 70, 41, 32, 27, 24, 23,
    -1, -2, 26, -3, -4, 31, 30, -5, -6, -7, 37, 38, 36, -8, -9, -10, 40, -11, -12, 55, 48, 46, 47,
    -13, -14, -15, 52, 51, -16, -17, 54, -18, -19, 63, 60, 59, -20, -21, 62, -22, -23, 67, 66, -24,
    -25, 69, -26, -27, 199, 132, 80, 77, 76, -28, -29, 79, -30, -31, 87, 85, -32, -33, -34, -35,
    -36, 197, -37, 91, -38, 134, -39, -40, -41, 97, -42, -43, 133, 106, -44, 117, -45, -46, 139,
    -47, -48, 110, -49, -50, 114, 113, -51, -52, 116, -53, -54, 135, 138, 136, 129, 125, 124, -55,
    -56, 130, 128, -57, -58, -59, 183, -60, -61, -62, -63, -64, 148, -65, -66, 153, 149, 145, 144,
    -67, -68, 147, -69, -70, -71, 152, 154, -72, -73, -74, 157, 171, -75, -76, 207, 184, 174, 167,
    166, 165, -77, -78, -79, 172, 170, -80, -81, -82, 178, -83, 177, 182, -84, -85, 187, 181, -86,
    -87, -88, -89, 206, 221, -90, 189, -91, 198, 254, 262, 195, 196, -92, -93, -94, -95, -96, 252,
    255, 250, -97, 211, 209, -98, -99, 212, -100, 213, -101, -102, -103, 224, -104, 232, 227, 220,
    226, -105, -106, 246, 236, -107, 243, -108, -109, 231, 237, 235, -110, -111, 239, 238, -112,
    -113, -114, -115, -116, 241, -117, 244, -118, -119, 248, -120, 249, -121, -122, -123, 253,
    -124, -125, -126, -127, 259, 258, -128, -129, 261, -130, -131, 390, 327, 296, 281, 274, 271,
    270, -132, -133, 273, -134, -135, 278, 277, -136, -137, 280, -138, -139, 289, 286, 285, -140,
    -141, 288, -142, -143, 293, 292, -144, -145, 295, -146, -147, 312, 305, 302, 301, -148, -149,
    304, -150, -151, 309, 308, -152, -153, 311, -154, -155, 320, 317, 316, -156, -157, 319, -158,
    -159, 324, 323, -160, -161, 326, -162, -163, 359, 344, 337, 334, 333, -164, -165, 336, -166,
    -167, 341, 340, -168, -169, 343, -170, -171, 352, 349, 348, -172, -173, 351, -174, -175, 356,
    355, -176, -177, 358, -178, -179, 375, 368, 365, 364, -180, -181, 367, -182, -183, 372, 371,
    -184, -185, 374, -186, -187, 383, 380, 379, -188, -189, 382, -190, -191, 387, 386, -192, -193,
    389, -194, -195, 454, 423, 408, 401, 398, 397, -196, -197, 400, -198, -199, 405, 404, -200,
    -201, 407, -202, -203, 416, 413, 412, -204, -205, 415, -206, -207, 420, 419, -208, -209, 422,
    -210, -211, 439, 432, 429, 428, -212, -213, 431, -214, -215, 436, 435, -216, -217, 438, -218,
    -219, 447, 444, 443, -220, -221, 446, -222, -223, 451, 450, -224, -225, 453, -226, -227, 486,
    471, 464, 461, 460, -228, -229, 463, -230, -231, 468, 467, -232, -233, 470, -234, -235, 479,
    476, 475, -236, -237, 478, -238, -239, 483, 482, -240, -241, 485, -242, -243, 499, 495, 492,
    491, -244, -245, 494, -246, -247, 497, -248, 502, -249, 506, 503, -250, -251, 505, -252, -253,
    508, -254, 510, -255, -256, 0,
];
/// `StringUtils.anIntArray233`: the code of each byte.
const CODES: [i32; 256] = [
    0,
    1024,
    2048,
    3072,
    4096,
    5120,
    6144,
    8192,
    9216,
    12288,
    10240,
    11264,
    16384,
    18432,
    17408,
    20480,
    21504,
    22528,
    23552,
    24576,
    25600,
    26624,
    27648,
    28672,
    29696,
    30720,
    31744,
    32768,
    33792,
    34816,
    35840,
    36864,
    536870912,
    16777216,
    37888,
    65536,
    38912,
    131072,
    196608,
    33554432,
    524288,
    1048576,
    1572864,
    262144,
    67108864,
    4194304,
    134217728,
    327680,
    8388608,
    2097152,
    12582912,
    13631488,
    14680064,
    15728640,
    100663296,
    101187584,
    101711872,
    101974016,
    102760448,
    102236160,
    40960,
    393216,
    229376,
    117440512,
    104857600,
    109051904,
    201326592,
    205520896,
    209715200,
    213909504,
    106954752,
    218103808,
    226492416,
    234881024,
    222298112,
    224395264,
    268435456,
    272629760,
    276824064,
    285212672,
    289406976,
    223346688,
    293601280,
    301989888,
    318767104,
    297795584,
    298844160,
    310378496,
    102498304,
    335544320,
    299892736,
    300941312,
    301006848,
    300974080,
    39936,
    301465600,
    49152,
    1073741824,
    369098752,
    402653184,
    1342177280,
    1610612736,
    469762048,
    1476395008,
    -2147483648,
    -1879048192,
    352321536,
    1543503872,
    -2013265920,
    -1610612736,
    -1342177280,
    -1073741824,
    -1543503872,
    356515840,
    -1476395008,
    -805306368,
    -536870912,
    -268435456,
    1577058304,
    -134217728,
    360710144,
    -67108864,
    364904448,
    51200,
    57344,
    52224,
    301203456,
    53248,
    54272,
    55296,
    56320,
    301072384,
    301073408,
    301074432,
    301075456,
    301076480,
    301077504,
    301078528,
    301079552,
    301080576,
    301081600,
    301082624,
    301083648,
    301084672,
    301085696,
    301086720,
    301087744,
    301088768,
    301089792,
    301090816,
    301091840,
    301092864,
    301093888,
    301094912,
    301095936,
    301096960,
    301097984,
    301099008,
    301100032,
    301101056,
    301102080,
    301103104,
    301104128,
    301105152,
    301106176,
    301107200,
    301108224,
    301109248,
    301110272,
    301111296,
    301112320,
    301113344,
    301114368,
    301115392,
    301116416,
    301117440,
    301118464,
    301119488,
    301120512,
    301121536,
    301122560,
    301123584,
    301124608,
    301125632,
    301126656,
    301127680,
    301128704,
    301129728,
    301130752,
    301131776,
    301132800,
    301133824,
    301134848,
    301135872,
    301136896,
    301137920,
    301138944,
    301139968,
    301140992,
    301142016,
    301143040,
    301144064,
    301145088,
    301146112,
    301147136,
    301148160,
    301149184,
    301150208,
    301151232,
    301152256,
    301153280,
    301154304,
    301155328,
    301156352,
    301157376,
    301158400,
    301159424,
    301160448,
    301161472,
    301162496,
    301163520,
    301164544,
    301165568,
    301166592,
    301167616,
    301168640,
    301169664,
    301170688,
    301171712,
    301172736,
    301173760,
    301174784,
    301175808,
    301176832,
    301177856,
    301178880,
    301179904,
    301180928,
    301181952,
    301182976,
    301184000,
    301185024,
    301186048,
    301187072,
    301188096,
    301189120,
    301190144,
    301191168,
    301193216,
    301195264,
    301194240,
    301197312,
    301198336,
    301199360,
    301201408,
    301202432,
];
/// `StringUtils.aByteArray235`: the code length of each byte.
const LENGTHS: [i8; 256] = [
    22, 22, 22, 22, 22, 22, 21, 22, 22, 20, 22, 22, 22, 21, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22,
    22, 22, 22, 22, 22, 22, 22, 22, 3, 8, 22, 16, 22, 16, 17, 7, 13, 13, 13, 16, 7, 10, 6, 16, 10,
    11, 12, 12, 12, 12, 13, 13, 14, 14, 11, 14, 19, 15, 17, 8, 11, 9, 10, 10, 10, 10, 11, 10, 9, 7,
    12, 11, 10, 10, 9, 10, 10, 12, 10, 9, 8, 12, 12, 9, 14, 8, 12, 17, 16, 17, 22, 13, 21, 4, 7, 6,
    5, 3, 6, 6, 5, 4, 10, 7, 5, 6, 4, 4, 6, 10, 5, 4, 4, 5, 7, 6, 10, 6, 10, 22, 19, 22, 14, 22,
    22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22,
    22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22,
    22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22,
    22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22,
    22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 22, 21,
    22, 21, 22, 22, 22, 21, 22, 22,
];
