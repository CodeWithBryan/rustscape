//! Actual owning-world helper replay; no callback ID dispatch or geometry stubs.
use rs09_server::{
    definitions::scenery::Registry as SceneryDefinitions,
    scripts::{Arguments, Context as _, Error, Level, Location, Result},
    world::{
        interaction_listeners::Function,
        interfaces::{self, Manager, Registry},
        player_walk::{self as walk, Event, Node, NodeHandle, Query, State},
        scenery_node,
        script_runtime::{self as runtime, Bindings},
        walk_request::Interfaces,
    },
};
use serde_json::{Value, json};
struct Unused;
impl walk::Services for Unused {
    fn query(&mut self, _: &mut State, _: Query<'_>) -> walk::Result<bool> {
        Err(walk::Error::Unsupported("unused helper query"))
    }
    fn event(&mut self, _: &mut State, _: Event<'_>) -> walk::Result<()> {
        Err(walk::Error::Unsupported("unused helper event"))
    }
    fn interfaces(&self, _: &State, _: Node) -> walk::Result<Interfaces> {
        Err(walk::Error::Unsupported("unused helper UI"))
    }
    fn region(&mut self, _: &mut State, _: &Location) -> walk::Result<Option<bool>> {
        Err(walk::Error::Unsupported("unused helper region"))
    }
    fn clipping(&mut self, _: &mut State, _: i32, _: i32, _: i32) -> walk::Result<i32> {
        Err(walk::Error::Unsupported("unused helper collision"))
    }
    fn now_millis(&mut self) -> i64 {
        panic!("unused helper wall clock")
    }
    fn metrics_enabled(&self) -> bool {
        false
    }
}
macro_rules! unused {
    ($name:ident($($arg:ident:$ty:ty),* $(,)?)->$ret:ty) => {
        fn $name(&mut self,$($arg:$ty),*)->Result<$ret> { Err(Error::Binding(concat!("unused helper binding: ",stringify!($name)).into())) }
    };
}
impl Bindings for Unused {
    unused!(modal_open(_state: &mut State, _owner: &NodeHandle) -> bool);
    unused!(interface_open(_state: &mut State, _owner: &NodeHandle) -> bool);
    unused!(chatbox_open(_state: &mut State, _owner: &NodeHandle) -> bool);
    unused!(close_interfaces(_state: &mut State, _owner: &NodeHandle) -> ());
    unused!(reset_animator(_state: &mut State, _owner: &NodeHandle) -> ());
    unused!(node_description(_state: &mut State, _node: Option<NodeHandle>) -> String);
    unused!(
        face_location(
            _state: &mut State,
            _node: NodeHandle,
            _from: Option<Location>,
        ) -> Option<Location>
    );
    unused!(center_location(_state: &mut State, _node: NodeHandle) -> Option<Location>);
    unused!(line_of_sight(_state: &mut State, _owner: &NodeHandle, _location: &Location) -> bool);
    unused!(face(_state: &mut State, _owner: &NodeHandle, _location: Location) -> ());
    unused!(
        execution_equals(
            _state: &mut State,
            _left: Option<Function>,
            _right: Option<Function>,
        ) -> bool
    );
    unused!(
        invoke(
            _state: &mut State,
            _owner: &NodeHandle,
            _execution: Option<Function>,
            _arguments: Arguments<NodeHandle>,
        ) -> bool
    );
    unused!(exception_text(_state: &mut State, _error: &Error) -> String);
    unused!(log(_state: &mut State, _owner: &str, _level: Level, _text: String) -> ());
    unused!(lock_entity(_state: &mut State, _owner: &NodeHandle, _ticks: i32) -> ());
}
fn number(row: &Value, key: &str) -> i32 {
    row[key].as_i64().unwrap() as i32
}
fn location(row: &Value) -> Location {
    let a = row["location"].as_array().unwrap();
    Location::new(
        a[0].as_i64().unwrap() as i32,
        a[1].as_i64().unwrap() as i32,
        a[2].as_i64().unwrap() as i32,
    )
}
pub fn evaluate(row: &Value) -> Value {
    let mut values = vec![];
    let result: Result<()> = (|| {
        match row["op"].as_str().unwrap() {
            "modal" => {
                let mut registry = Registry::default();
                let manager = Manager::default();
                {
                    let mut f = manager.fields.write();
                    f.opened = row["opened"]
                        .as_i64()
                        .map(|id| registry.component(id as i32));
                    f.chatbox = row["chatbox"]
                        .as_i64()
                        .map(|id| registry.component(id as i32));
                }
                values.push(json!(runtime::has_modal_open(&manager)));
                values.push(json!(interfaces::is_opened(&manager)));
                values.push(json!(interfaces::has_chatbox(&manager)));
            }
            "clocks" => {
                let mut state = State::default();
                let n = state.add(1, Location::new(0, 0, 0), [1, 2, 3, 4]);
                let owner = state.retain_node(n).unwrap();
                let processor = state.scripts(&owner).unwrap();
                processor.set_delay(number(row, "delay"));
                state.ticks = number(row, "tick");
                let clocks = state.clocks(&owner).unwrap();
                {
                    let mut c = clocks.write();
                    *c = vec![0; row["length"].as_u64().unwrap() as usize];
                    if !c.is_empty() {
                        c[0] = number(row, "movement");
                    }
                    if c.len() > 6 {
                        c[6] = number(row, "stun");
                    }
                }
                let mut service = Unused;
                let mut ctx = runtime::Context::new(&state, &mut service, &processor)?;
                values.push(json!(ctx.delayed(&processor, &mut state)?));
                values.push(json!(ctx.stunned(&processor, &mut state)?));
                values.push(json!(ctx.finished_moving(&processor, &mut state)?));
            }
            "center" | "sceneryCenter" => {
                let mut state = State::default();
                let target = if row["op"] == "center" {
                    let n = state.add_npc(1, location(row), [1, 2, 3, 4]);
                    state.actor_mut(n).unwrap().view.size = number(row, "size");
                    state.retain_node(n).unwrap()
                } else {
                    let mut definitions = SceneryDefinitions::default();
                    let definition = definitions.for_id(1);
                    {
                        let mut d = definition.write();
                        d.size_x = row["dimensions"][0].as_i64().unwrap() as i32;
                        d.size_y = row["dimensions"][1].as_i64().unwrap() as i32;
                    }
                    let scenery =
                        scenery_node::construct(&mut definitions, 1, location(row), 10, 0, false)
                            .unwrap();
                    scenery.write().direction =
                        rs09_game_core::direction::Direction::from_code(number(row, "direction"))
                            .unwrap();
                    state.scenery_target(scenery).unwrap()
                };
                values.push(json!(
                    runtime::center_location(&state, &target)?.coordinates()
                ));
            }
            op => panic!("unknown helper {op}"),
        }
        Ok(())
    })();
    let error = match result {
        Ok(()) => Value::Null,
        Err(Error::Index) => json!("Index"),
        Err(e) => panic!("unexpected helper error {e:?}"),
    };
    json!({"values":values,"error":error})
}
