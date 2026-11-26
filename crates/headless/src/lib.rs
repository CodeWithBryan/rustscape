//! Data-only revision530 client sessions and structured world observations.
//! Uses shared wire, scenery and item code without a graphics or audio device.
#![forbid(unsafe_code)]
pub mod agent;
pub mod ground;
pub mod scenery;
pub mod session;
pub mod world;
pub use rs09_client_data as data;
pub use rs09_protocol as protocol;
