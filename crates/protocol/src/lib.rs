#![forbid(unsafe_code)]
//! Revision 530 protocol primitives. Valid-wire behavior follows the pinned RT4 client.
pub mod buffer;
pub mod checksum;
pub mod game;
pub mod huffman;
pub mod isaac;
pub mod js5;
pub mod lobby;
pub mod login;
pub mod profile;
pub mod session;
pub mod xtea;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub offset: usize,
    pub reason: &'static str,
}
impl Error {
    pub const fn new(offset: usize, reason: &'static str) -> Self {
        Self { offset, reason }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at byte {}", self.reason, self.offset)
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
