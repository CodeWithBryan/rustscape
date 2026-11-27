//! Ported content: the original's region, NPC and item content classes,
//! each written against the native library it runs on (the dialogue DSLs,
//! InteractionListeners, ...) and checked against its own JVM corpus. See
//! docs/content-porting.md for the recipe and `tools/content_scaffold.py`
//! for the generator that lays a port out.
//!
//! Every call a port makes outside its library is a required service with
//! no default: shared ones on [`Services`], a port's own on its module's
//! `Host` trait. A host that cannot answer refuses (it never fabricates a
//! success). [`dialogue_plugin`] is the table the replay tests read; the
//! live world registers only the classes of [`LIVE_DIALOGUES`], in the class
//! scan's order among themselves (world/live/ported).
use crate::world::{dialogue::Plugin, dialogue_dsl};

pub mod lumbridge;
// content-scaffold: modules

/// Services content in general reads, beyond the dialogue DSLs' host.
pub trait Services: dialogue_dsl::Host {
    /// `GameWorld.settings?.name`: `None` when there are no settings.
    fn settings_name(&mut self) -> dialogue_dsl::Result<Option<dialogue_dsl::Text>>;
}

/// Declares [`Host`], everything the ported content calls ([`Services`]
/// and each port's own `Host` trait), with its blanket implementation.
macro_rules! hosts {
    ($($port:path),* $(,)?) => {
        /// Everything the ported content calls: [`Services`] and each
        /// port's own `Host` trait.
        pub trait Host: Services $(+ $port)* {}
        impl<T: Services $(+ $port)*> Host for T {}
    };
}
hosts! {
    lumbridge::donie_dialogue::Host,
    // content-scaffold: hosts
}

/// The dialogue plugins the live world registers (`Live::install_ported`).
/// A class goes here by hand once its bodies are ported, its corpus has
/// designed cases and replays, and its live Host answers are written
/// (docs/content-porting.md, step 6): until then Talk-to keeps the named
/// not_ported answer. The scaffold never adds one, and the content_registry
/// test fails while a module of a listed class keeps a "not ported yet" stub.
pub const LIVE_DIALOGUES: &[&str] = &["content.region.misthalin.lumbridge.dialogue.DonieDialogue"];

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
