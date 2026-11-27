//! Replays the JVM donie-dialogue corpus (oracle/donie-dialogue) against
//! `world::content::lumbridge::donie_dialogue` over the dialogue-dsl
//! recording host and the dialogue content kit, and pins the fixture.
#[path = "../../../oracle/content-kit/hosts.rs"]
mod hosts;
#[path = "../../../oracle/content-kit/dialogue.rs"]
mod kit;
#[path = "../../../oracle/dialogue-dsl/native.rs"]
mod native;
use native::Model;
use rs09_server::world::{content, dialogue::Plugin};

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
