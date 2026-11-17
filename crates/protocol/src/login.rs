//! Pinned LoginManager/Buffer writer and Kotlin Login.decodeFromBuffer behavior.
use crate::{
    Error, Result,
    buffer::{Reader, Writer},
    profile::Profile,
};
use num_bigint::{BigInt, Sign};

pub const CACHE_INDEX_COUNT: usize = 29;
pub const RSA_MODULUS: &str = "96982303379631821170939875058071478695026608406924780574168393250855797534862289546229721580153879336741968220328805101128831071152160922518190059946555203865621183480223212969502122536662721687753974815205744569357388338433981424032996046420057284324856368815997832596174397728134370577184183004453899764051";
pub const RSA_PUBLIC_EXPONENT: &str = "65537";
/// Publicly checked-in reference-server key, retained for compatibility with its client.
pub const RSA_REFERENCE_PRIVATE_EXPONENT: &str = "52317200263721308660411803146360972546561037484450290559823448967617618536819222494429186211525706853703641369936136465589036631055945454547936148730495933263344792588795811788941129493188907621550836988152620502378278134421731002382361670176785306598134280732756356458964850508114958769985438054979422820241";

#[derive(Clone)]
pub struct RsaKey {
    exponent: BigInt,
    modulus: BigInt,
}
impl RsaKey {
    pub fn new(exponent: &str, modulus: &str) -> Result<Self> {
        let exponent = BigInt::parse_bytes(exponent.as_bytes(), 10)
            .ok_or(Error::new(0, "invalid RSA exponent"))?;
        let modulus = BigInt::parse_bytes(modulus.as_bytes(), 10)
            .ok_or(Error::new(0, "invalid RSA modulus"))?;
        if exponent.sign() == Sign::Minus || modulus.sign() != Sign::Plus {
            return Err(Error::new(
                0,
                "RSA requires nonnegative exponent and positive modulus",
            ));
        }
        Ok(Self { exponent, modulus })
    }
    pub fn reference_public() -> Self {
        Self::new(RSA_PUBLIC_EXPONENT, RSA_MODULUS).expect("pinned key")
    }
    pub fn reference_private() -> Self {
        Self::new(RSA_REFERENCE_PRIVATE_EXPONENT, RSA_MODULUS).expect("pinned key")
    }
    /// Java BigInteger uses signed two's-complement input and output, including a leading zero.
    pub fn transform(&self, bytes: &[u8]) -> Result<Vec<u8>> {
        if bytes.is_empty() {
            return Err(Error::new(0, "empty signed RSA integer"));
        }
        Ok(BigInt::from_signed_bytes_be(bytes)
            .modpow(&self.exponent, &self.modulus)
            .to_signed_bytes_be())
    }
    pub fn frame(&self, bytes: &[u8]) -> Result<Vec<u8>> {
        let encoded = self.transform(bytes)?;
        let count = u8::try_from(encoded.len())
            .map_err(|_| Error::new(0, "RSA block exceeds wire byte length"))?;
        let mut frame = Vec::with_capacity(encoded.len() + 1);
        frame.push(count);
        frame.extend(encoded);
        Ok(frame)
    }
    /// Original decryptRSABuffer substitutes [-1] only for a framing underflow.
    pub fn unframe(&self, reader: &mut Reader<'_>) -> Result<Vec<u8>> {
        match reader.g1().and_then(|length| reader.take(length as usize)) {
            Ok(bytes) => self.transform(bytes),
            Err(_) => Ok(vec![255]),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum AuthResponse {
    UnexpectedError,
    CouldNotAd,
    Success,
    InvalidCredentials,
    AccountDisabled,
    AlreadyOnline,
    Updated,
    FullWorld,
    LoginServerOffline,
    LoginLimitExceeded,
    BadSessionId,
    WeakPassword,
    MembersWorld,
    CouldNotLogin,
    Updating,
    TooManyIncorrectLogins,
    StandingInMembersArea,
    AccountLocked,
    ClosedBeta,
    InvalidLoginServer,
    MovingWorld,
    ErrorLoadingProfile,
    BannedUser,
}
impl AuthResponse {
    pub fn from_byte(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::UnexpectedError,
            1 => Self::CouldNotAd,
            2 => Self::Success,
            3 => Self::InvalidCredentials,
            4 => Self::AccountDisabled,
            5 => Self::AlreadyOnline,
            6 => Self::Updated,
            7 => Self::FullWorld,
            8 => Self::LoginServerOffline,
            9 => Self::LoginLimitExceeded,
            10 => Self::BadSessionId,
            11 => Self::WeakPassword,
            12 => Self::MembersWorld,
            13 => Self::CouldNotLogin,
            14 => Self::Updating,
            15 => Self::TooManyIncorrectLogins,
            16 => Self::StandingInMembersArea,
            17 => Self::AccountLocked,
            18 => Self::ClosedBeta,
            19 => Self::InvalidLoginServer,
            20 => Self::MovingWorld,
            21 => Self::ErrorLoadingProfile,
            22 => Self::BannedUser,
            _ => return None,
        })
    }
}

/// JagString.encode37 reads at most twelve original bytes and removes trailing zero digits.
pub fn encode_name(bytes: &[u8]) -> i64 {
    let mut name = 0;
    for &byte in bytes.iter().take(12) {
        name = name * 37
            + match byte {
                b'A'..=b'Z' => i64::from(byte - b'A' + 1),
                b'a'..=b'z' => i64::from(byte - b'a' + 1),
                b'0'..=b'9' => i64::from(byte - b'0' + 27),
                _ => 0,
            };
    }
    while name != 0 && name % 37 == 0 {
        name /= 37;
    }
    name
}

/// StringUtils.longToString returns an empty string on negative or overlong names.
pub fn decode_name(mut value: i64) -> String {
    const CHARS: &[u8] = b"_abcdefghijklmnopqrstuvwxyz0123456789";
    let mut output = [0; 12];
    let mut position = 12;
    while value != 0 {
        if value < 0 || position == 0 {
            return String::new();
        }
        position -= 1;
        output[position] = CHARS[(value % 37) as usize];
        value /= 37;
    }
    String::from_utf8(output[position..].to_vec()).expect("ASCII name alphabet")
}

/// ByteBufferUtils.getString sign-extends each byte to a Kotlin Char.
pub fn server_string(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&byte| {
            char::from_u32(u32::from(byte as i8 as u16)).expect("non-surrogate byte character")
        })
        .collect()
}

pub fn handshake(username: &[u8]) -> [u8; 2] {
    [14, ((encode_name(username) >> 16) & 31) as u8]
}

pub struct LoginRequest<'a> {
    pub reconnect: bool,
    pub login_type: i32,
    pub advertisements_suppressed: bool,
    pub window_mode: i32,
    pub screen_width: i32,
    pub screen_height: i32,
    pub display_mode: i32,
    pub uid: [u8; 24],
    pub settings: &'a [u8],
    pub affiliate: i32,
    pub settings_hash: i32,
    pub packet_count: i32,
    pub crc_sums: [i32; CACHE_INDEX_COUNT],
    pub isaac_seed: [i32; 4],
    pub username: &'a [u8],
    pub password: &'a [u8],
}
impl Default for LoginRequest<'_> {
    fn default() -> Self {
        Self {
            reconnect: false,
            login_type: 0,
            advertisements_suppressed: false,
            window_mode: 0,
            screen_width: 765,
            screen_height: 503,
            display_mode: 0,
            uid: [0; 24],
            settings: b"",
            affiliate: 0,
            settings_hash: 0,
            packet_count: 0,
            crc_sums: [0; 29],
            isaac_seed: [0; 4],
            username: b"",
            password: b"",
        }
    }
}
impl LoginRequest<'_> {
    pub fn encode(&self, profile: Profile, public_key: &RsaKey) -> Result<Vec<u8>> {
        let mut secret = Writer::new();
        secret.p1(10);
        for seed in self.isaac_seed {
            secret.p4(seed);
        }
        secret.p8(encode_name(self.username));
        secret.pjstr(self.password)?;
        if profile.login_extra_info {
            secret.pdata(&[0, 0, 0]);
        }
        let encrypted = public_key.frame(secret.as_slice())?;
        let mut body = Writer::new();
        body.p4(profile.revision as i32);
        body.p1(self.login_type);
        body.p1(i32::from(self.advertisements_suppressed));
        body.p1(1);
        body.p1(self.window_mode);
        body.p2(self.screen_width);
        body.p2(self.screen_height);
        body.p1(self.display_mode);
        body.pdata(&self.uid);
        body.pjstr(self.settings)?;
        body.p4(self.affiliate);
        body.p4(self.settings_hash);
        body.p2(self.packet_count);
        for &checksum in &self.crc_sums[..28] {
            body.p4(checksum);
        }
        if profile.login_fake_index_28 {
            body.p4(0);
        }
        body.pdata(&encrypted);
        if body.as_slice().len() > i16::MAX as usize {
            return Err(Error::new(0, "login frame exceeds signed length"));
        }
        let mut frame = Writer::new();
        frame.p1(if self.reconnect { 18 } else { 16 });
        frame.p2(body.as_slice().len() as i32);
        frame.pdata(body.as_slice());
        Ok(frame.into_vec())
    }
}

#[derive(Clone, Default, Eq, PartialEq)]
pub struct LoginInfo {
    pub opcode: i32,
    pub show_ads: bool,
    pub window_mode: i32,
    pub screen_width: i32,
    pub screen_height: i32,
    pub display_mode: i32,
    pub affiliate: i32,
    pub settings_hash: i32,
    pub packet_count: i32,
    pub crc_sums: [i32; CACHE_INDEX_COUNT],
    pub username: String,
    pub password: String,
    /// None matches the default pair's empty seeds before verification byte succeeds.
    pub isaac_seed: Option<[i32; 4]>,
}
impl LoginInfo {
    pub fn outgoing_seed(&self) -> Option<[i32; 4]> {
        self.isaac_seed
            .map(|seed| seed.map(|word| word.wrapping_add(50)))
    }
}

pub struct LoginOutcome {
    pub response: AuthResponse,
    pub info: Option<LoginInfo>,
}
fn outcome(response: AuthResponse, info: Option<LoginInfo>) -> LoginOutcome {
    LoginOutcome { response, info }
}

pub fn decode(packet: &[u8], key: &RsaKey, is_online: impl FnOnce(&str) -> bool) -> LoginOutcome {
    decode_checked(packet, key, is_online)
        .unwrap_or_else(|_| outcome(AuthResponse::UnexpectedError, None))
}
fn decode_checked(
    packet: &[u8],
    key: &RsaKey,
    is_online: impl FnOnce(&str) -> bool,
) -> Result<LoginOutcome> {
    let mut reader = Reader::new(packet);
    let mut info = LoginInfo {
        opcode: reader.g1b()?,
        ..LoginInfo::default()
    };
    if reader.g2b()? as i64 != reader.remaining() as i64 {
        return Ok(outcome(AuthResponse::BadSessionId, None));
    }
    if reader.g4()? != 530 {
        return Ok(outcome(AuthResponse::Updated, None));
    }
    if info.opcode != 16 && info.opcode != 18 {
        return Ok(outcome(AuthResponse::InvalidLoginServer, None));
    }
    reader.g1()?;
    info.show_ads = reader.g1b()? == 1;
    reader.g1()?;
    info.window_mode = reader.g1b()?;
    info.screen_width = reader.g2b()?;
    info.screen_height = reader.g2b()?;
    info.display_mode = reader.g1b()?;
    reader.take(24)?;
    reader.gjstr()?;
    info.affiliate = reader.g4()?;
    info.settings_hash = reader.g4()?;
    info.packet_count = reader.g2b()?;
    for checksum in &mut info.crc_sums {
        *checksum = reader.g4()?;
    }
    // decryptRSABuffer catches an underflow itself and substitutes [-1].
    let secret = key.unframe(&mut reader)?;
    let mut secret = Reader::new(&secret);
    if secret.g1b()? != 10 {
        return Ok(outcome(AuthResponse::UnexpectedError, Some(info)));
    }
    let mut seed = [0; 4];
    for word in &mut seed {
        *word = secret.g4()?;
    }
    info.isaac_seed = Some(seed);
    info.username = decode_name(secret.g8()?);
    info.password = server_string(secret.gjstr()?);
    let response = if is_online(&info.username) {
        AuthResponse::AlreadyOnline
    } else {
        AuthResponse::Success
    };
    Ok(outcome(response, Some(info)))
}

/// Exact LoginWriteEvent bytes. Ordinal 21, rather than the stale comment's
/// MovingWorld ordinal, includes the low server-key byte in the pinned source.
pub fn reply(response: AuthResponse, rights: i32, index: i32, server_key: i64) -> Vec<u8> {
    let mut output = Writer::new();
    output.p1(i32::from(response as u8));
    match response {
        AuthResponse::Success => {
            output.p1(rights);
            output.pdata(&[0, 0, 0, 1, 0, 0]);
            output.p2(index);
            output.pdata(&[1, 1]);
        }
        AuthResponse::ErrorLoadingProfile => output.p1(server_key as i32),
        _ => {}
    }
    output.into_vec()
}
