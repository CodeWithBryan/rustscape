use rs09_game_core::direction::Direction;
use rs09_server::world::{
    doors::Location,
    npc_spawns::{self, Context, Error, Result},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
struct Npc {
    id: i32,
    location: Location,
    walks: bool,
    direction: Direction,
    attributes: BTreeMap<&'static str, bool>,
    initialized: bool,
}
struct Model<'a> {
    input: &'a Value,
    npcs: Vec<Npc>,
    events: Vec<Value>,
    calls: BTreeMap<&'static str, i64>,
}
impl Model<'_> {
    fn emit(&mut self, phase: &'static str, args: Vec<Value>) -> Result<()> {
        let mut row = vec![json!(phase)];
        row.extend(args);
        self.events.push(json!(row));
        let at = self.calls.entry(phase).or_default();
        *at += 1;
        if self.input["failures"][phase].as_i64() == Some(*at) {
            return Err(Error::Binding(phase.into()));
        }
        Ok(())
    }
}
impl Context for Model<'_> {
    type Npc = usize;
    fn create(&mut self, id: i32, location: Location) -> Result<usize> {
        let slot = self.npcs.len();
        let coordinates = location.coordinates();
        self.npcs.push(Npc {
            id,
            location,
            walks: false,
            direction: Direction::South,
            attributes: BTreeMap::new(),
            initialized: false,
        });
        self.emit("create", vec![json!(slot), json!(id), json!(coordinates)])?;
        Ok(slot)
    }
    fn walks(&mut self, n: &usize, b: bool) -> Result<()> {
        self.npcs[*n].walks = b;
        self.emit("walks", vec![json!(n), json!(b)])
    }
    fn direction(&mut self, n: &usize, d: Direction) -> Result<()> {
        self.npcs[*n].direction = d;
        self.emit(
            "direction",
            vec![
                json!(n),
                json!(Direction::ALL.iter().position(|x| *x == d).unwrap()),
            ],
        )
    }
    fn attribute(&mut self, n: &usize, k: &'static str, v: bool) -> Result<()> {
        self.npcs[*n].attributes.insert(k, v);
        self.emit("attribute", vec![json!(n), json!(k), json!(v)])
    }
    fn facing_booth(&mut self, n: &usize) -> Result<bool> {
        self.emit("booth", vec![json!(n)])?;
        Ok(self.input["booths"]
            .as_array()
            .is_some_and(|a| a.contains(&json!(self.npcs[*n].id))))
    }
    fn init(&mut self, n: &usize) -> Result<()> {
        self.npcs[*n].initialized = true;
        self.emit("init", vec![json!(n)])
    }
    fn log(&mut self, count: i32) -> Result<()> {
        self.emit("log", vec![json!(format!("Spawned {count} NPCs"))])
    }
}
pub fn evaluate(c: &Value) -> Value {
    let mut model = Model {
        input: c,
        npcs: vec![],
        events: vec![],
        calls: BTreeMap::new(),
    };
    let text = c["text"]
        .as_str()
        .unwrap_or(include_str!("../../fixtures/npc-spawns/npc_spawns.json"));
    let error = npc_spawns::load(&mut model, text).err().map(|e| match e {
        Error::Null => "Null",
        Error::Type => "Type",
        Error::Number => "Number",
        Error::Index => "Index",
        Error::Json(_) => "Json",
        Error::Binding(_) => "Binding",
    });
    let npcs: Vec<_> = model
        .npcs
        .iter()
        .map(|n| {
            json!([
                n.id,
                n.location.coordinates(),
                n.walks,
                Direction::ALL
                    .iter()
                    .position(|x| *x == n.direction)
                    .unwrap(),
                n.attributes,
                n.initialized
            ])
        })
        .collect();
    json!({"npcs":npcs,"events":model.events,"error":error})
}
