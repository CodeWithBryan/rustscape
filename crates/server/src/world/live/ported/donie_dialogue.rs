//! The live world's answers to `world::content::lumbridge::donie_dialogue::Host`
//! (content/region/misthalin/lumbridge/dialogue/DonieDialogue.kt). An answer that needs a system without a port refuses
//! (and is listed in docs/live-world.md); none fabricates a success.
use super::super::host::Live;
#[allow(unused_imports)]
use crate::world::{
    content::lumbridge::donie_dialogue,
    dialogue::{Error, Result},
};

impl donie_dialogue::Host for Live {}
