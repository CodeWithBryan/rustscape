//! The recording host's answers to every ported content service: the shared
//! [`content::Services`] and each port's own `Host` trait. One file for all
//! ports because `content::Host` (what `content::dialogue_plugin` needs) is
//! the union of them; every content test includes it beside the kit:
//!
//! ```ignore
//! #[path = "../../../oracle/content-kit/hosts.rs"]
//! mod hosts;
//! ```
//!
//! Each answer records its call as the oracle's stand-in does (same name,
//! same value from the case). tools/content_scaffold.py appends a block per
//! port at the marker.
#![allow(unused_imports)]
use super::{kit, native::Model};
use rs09_server::world::{
    content,
    dialogue::{Result, Text},
};

impl content::Services for Model {
    fn settings_name(&mut self) -> Result<Option<Text>> {
        kit::settings_name(self)
    }
}

impl content::lumbridge::donie_dialogue::Host for Model {}

// content-scaffold: recording hosts
