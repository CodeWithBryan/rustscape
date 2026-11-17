//! GPU-independent RT4 item definitions and prepared asset containers.
#![forbid(unsafe_code)]
pub mod item;
mod item_fields;
pub mod pack;
pub mod scenery;
use serde::Serialize;

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(untagged)]
pub enum HookArg {
    Int(i32),
    Text(String),
    Null,
}

pub fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}
