//! Replays the JVM donie-dialogue corpus (oracle/donie-dialogue) against
//! `world::content::lumbridge::donie_dialogue` over the dialogue-dsl recording host
//! and the dialogue content kit, and pins the fixture.
#[path = "../../../oracle/content-kit/dialogue.rs"]
mod kit;
#[path = "../../../oracle/dialogue-dsl/native.rs"]
mod native;
use native::Model;
use rs09_server::world::{
    content::{self, lumbridge::donie_dialogue},
    dialogue::{Plugin, Result, Text},
};

impl content::Services for Model {
    fn settings_name(&mut self) -> Result<Option<Text>> {
        kit::settings_name(self)
    }
}
/// The recording answers of the port's own services: each records its call
/// as the oracle's stand-in does.
impl donie_dialogue::Host for Model {}

fn prototype(class: &str) -> Option<Plugin<Model>> {
    content::dialogue_plugin(class)
}

#[test]
fn original_donie_dialogue_observations_match() {
    kit::replay(
        "donie-dialogue",
        include_str!("../../../fixtures/donie-dialogue/oracle.jsonl"),
        include_str!("../../../fixtures/donie-dialogue/provenance.json"),
        prototype,
    );
}
