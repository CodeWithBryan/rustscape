//! The live world's answers to `world::content::lumbridge::donie_dialogue::Host`
//! (content/region/misthalin/lumbridge/dialogue/DonieDialogue.kt). Donie
//! calls nothing outside the dialogue library but `GameWorld.settings?.name`,
//! which `content::Services` answers (world/live/ported).
use super::super::host::Live;
use crate::world::content::lumbridge::donie_dialogue;

impl donie_dialogue::Host for Live {}
