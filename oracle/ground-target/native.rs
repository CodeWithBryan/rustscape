//! Shared walking replay with actual native ground/subtype construction.
use super::*;
use rs09_server::{
    definitions::item as defs,
    inventory::item,
    world::{ground_item as ground, ground_spawn as spawn, player_walk::NodeHandle},
};
#[derive(Default)]
pub(super) struct Extra {
    pub target: Option<NodeHandle>,
    registry: spawn::Registry,
}
pub struct Constructor {
    pub definitions: defs::Registry,
    pub ticks: i32,
}
impl Default for Constructor {
    fn default() -> Self {
        let mut definitions = defs::Registry::default();
        definitions.insert(
            1,
            Shared::new(defs::ItemDefinition {
                id: 1,
                name: Some("item".into()),
                ..Default::default()
            }),
        );
        Self {
            definitions,
            ticks: 0,
        }
    }
}
impl ground::Context for Constructor {
    fn copy_item(&mut self, id: i32, amount: i32, charge: i32) -> ground::Result<item::Handle> {
        Ok(Shared::new(item::Item::new(
            id,
            amount,
            charge,
            &self.definitions,
        )))
    }
    // Explicit unobserved constructor/default-content adapter for this replay.
    fn default_interaction(&mut self, _: &ground::Handle) -> ground::Result<()> {
        Ok(())
    }
    fn uid(&mut self, _: &NodeHandle) -> ground::Result<i32> {
        Ok(7)
    }
    fn username(&mut self, _: &NodeHandle) -> ground::Result<Option<String>> {
        Ok(Some("1".into()))
    }
    fn ticks(&mut self) -> ground::Result<i32> {
        Ok(self.ticks)
    }
}
pub(super) fn initialize(m: &mut Model, state: &mut State) {
    replace(m, state, &m.input["ground"].clone());
    let mut option = OptionData::new(Some("Take".into()), 2);
    option.handler = handler(&m.input);
    m.option = Some(Shared::new(option));
}
fn replace(m: &mut Model, state: &mut State, a: &Value) {
    let mut ctx = Constructor {
        ticks: state.ticks,
        ..Default::default()
    };
    let input = Shared::new(item::Item::new(1, 1, 1000, &ctx.definitions));
    let location = if a.get("location").is_some() {
        input_location(&a["location"])
    } else {
        Some(Location::new(3211, 3217, 0))
    };
    let item = if a["spawn"] == true {
        let spawn = m
            .ground
            .registry
            .construct(&mut ctx, 3, &input, location)
            .unwrap();
        spawn::as_ground(&spawn)
    } else {
        ground::construct(&mut ctx, &input, location, n(a, "decay", 200), None).unwrap()
    };
    item.write().node_size = n(a, "size", 1);
    item.write().removed = a["removed"] == true;
    if let Some(decay) = a.get("decay") {
        item.write().decay_time = decay.as_i64().unwrap() as i32;
    }
    m.ground.target = Some(state.ground_target(item, &m.ground.registry).unwrap());
}
pub(super) fn act(m: &mut Model, state: &mut State, a: &Value) -> Result<()> {
    match a["op"].as_str().unwrap() {
        "groundSet" => {
            let g = state.ground(m.ground.target.as_ref().unwrap().node)?;
            let mut g = g.write();
            if a.get("location").is_some() {
                g.location = input_location(&a["location"]);
            }
            if a.get("size").is_some() {
                g.node_size = n(a, "size", 1);
            }
            if a.get("removed").is_some() {
                g.removed = a["removed"] == true;
            }
            if a.get("decay").is_some() {
                g.decay_time = n(a, "decay", 200);
            }
            Ok(())
        }
        "groundReplace" => {
            replace(m, state, a);
            Ok(())
        }
        op => panic!("unknown ground-target replay action {op}"),
    }
}
pub(super) fn snapshot(m: &Model, state: &State, value: &mut Value) {
    let item = state
        .ground(m.ground.target.as_ref().unwrap().node)
        .unwrap();
    let g = item.read();
    value["ground"] = json!([
        g.location.as_ref().map(Location::coordinates),
        g.node_size,
        g.removed,
        g.decay_time,
        m.ground.registry.get(&item).is_some()
    ]);
    let o = m.option.as_ref().unwrap().read();
    value["option"] = json!([o.name(), o.index(), o.handler]);
}
