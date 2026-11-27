//! Ported content: the original's region, NPC and item content classes,
//! each written against the native library it runs on (the dialogue DSLs,
//! InteractionListeners, ...) and checked against its own JVM corpus. See
//! docs/content-porting.md for the recipe and `tools/content_scaffold.py`
//! for the generator that lays a port out.
//!
//! Every call a port makes outside its library is a required service with
//! no default: shared ones on [`Services`], a port's own on its module's
//! `Host` trait. A host that cannot answer refuses (it never fabricates a
//! success). The live world registers the ported classes in the class
//! scan's order (world/live/ported).
use crate::world::{dialogue::Plugin, dialogue_dsl};

pub mod lumbridge;
// content-scaffold: modules

/// Services content in general reads, beyond the dialogue DSLs' host.
pub trait Services: dialogue_dsl::Host {
    /// `GameWorld.settings?.name`: `None` when there are no settings.
    fn settings_name(&mut self) -> dialogue_dsl::Result<Option<dialogue_dsl::Text>>;
}

/// Everything the ported content calls: [`Services`] and each port's own
/// `Host` trait.
pub trait Host: Services + lumbridge::donie_dialogue::Host // content-scaffold: hosts
{
}
impl<T> Host for T
where
    T: Services,
    T: lumbridge::donie_dialogue::Host,
    // content-scaffold: host bounds
{
}

/// The native prototype of a ported `DialoguePlugin`, by the scanned class
/// name (`scan.initializable`), for ClassScanner's DIALOGUE branch
/// (`init()` registers its ids).
pub fn dialogue_plugin<H: Host>(class: &str) -> Option<Plugin<H>> {
    let plugin = match class {
        "content.region.misthalin.lumbridge.dialogue.DonieDialogue" => {
            lumbridge::donie_dialogue::prototype()
        }
        // content-scaffold: dialogue plugins
        _ => return None,
    };
    Some(plugin)
}
