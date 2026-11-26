//! Native replay of the activities corpus: `world::activities` over the real
//! native scene and `dynamic_region` with the exported cache regions, with
//! recording stand-ins for the systems the oracle's adapters record (player
//! and NPC members, interfaces, dialogue, camera, AntiMacro, the session).
//! The scripted content (TestActivity, TestCutscenePlugin, TestCutscene) is
//! the native counterpart of the oracle's: it calls only the framework's
//! members. RegionManager.move for an NPC, the pulser queue and the tick are
//! scaffolding, as in the dynamic-regions replay.
use crate::regions::{Bindings, Cache, MemoryAssets, cache, hex};
use rs09_protocol::{
    buffer::Reader,
    game::{Direction as Wire, Frame},
};
use rs09_server::{
    definitions::scenery as defs,
    player::attributes::AttributeValue,
    pulses,
    world::{
        activities::{
            self, Error, Expression, Host, Level, Pulses, Result,
            cutscene::{self, Continue, Cutscene, CutsceneRef},
            manager::{self, Manager},
            plugin::{self, Content, Core, PluginRef},
        },
        chunk::HandleChunk,
        doors::{self, Handle as Scenery, Location},
        dynamic_region as dynamic, dynamic_scene,
        force_movement::Direction,
        ground_item,
        landscape::{Chunks, Mapscape},
        player_walk::{NodeHandle, State},
        region, region_activity,
        scene::{Assets, Scene},
        scenery_node,
        zones::{self, Borders, EntityKind, Monitor, Restriction, Zone},
    },
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, OnceLock},
};

fn num(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}
fn loc(v: &Value) -> Option<Location> {
    if v.is_null() {
        return None;
    }
    Some(Location::new(num(&v[0]), num(&v[1]), num(&v[2])))
}
fn coords(l: Option<&Location>) -> Value {
    l.map_or(Value::Null, |l| json!(l.coordinates()))
}
fn borders_value(b: Option<&Borders>) -> Value {
    b.map_or(Value::Null, |b| {
        json!([
            b.south_west_x(),
            b.south_west_y(),
            b.north_east_x(),
            b.north_east_y()
        ])
    })
}
fn injected() -> Error {
    Error::Binding("injected".into())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Player,
    Npc,
}
struct Entity {
    name: String,
    node: NodeHandle,
    kind: Kind,
    location: Option<Location>,
    active: bool,
    attributes: BTreeMap<String, AttributeValue>,
    teleport: Option<Location>,
    spawn: Option<Location>,
    safe: bool,
    safe_respawn: Option<Location>,
}

enum Tracked {
    Plugin(Zone<Model>),
    Cutscene(Cutscene<Model>),
}

pub struct Model {
    scene: Scene<MemoryAssets>,
    services: Bindings,
    state: State,
    entities: Vec<Entity>,
    by_node: HashMap<usize, usize>,
    events: Vec<Value>,
    frames: Vec<String>,
    op_calls: i32,
    fail_at: i32,
    answers: Value,
    answer_calls: HashMap<String, usize>,
    queue: Vec<pulses::Handle>,
    tracked: Vec<(Tracked, String)>,
    scripts: Vec<(Cutscene<Model>, Arc<ScriptInner>)>,
    counters: HashMap<String, i32>,
    actions: Vec<Continue<Model>>,
    logout: BTreeMap<String, Cutscene<Model>>,
    hooks: usize,
    manager: Manager<Model>,
    pulses: Pulses<Model>,
    multiway: Option<Zone<Model>>,
    world: Value,
    npcs: i32,
    diagnostics: usize,
}

macro_rules! world {
    ($m:expr) => {
        $m.scene.context(&mut $m.services)
    };
}

impl Model {
    /// A recorded call; the case's failAt-th call of an operation fails.
    fn record(&mut self, name: &str, args: Vec<Value>) -> Result<()> {
        let mut e = vec![json!(name)];
        e.extend(args);
        self.events.push(Value::Array(e));
        self.op_calls += 1;
        if self.op_calls == self.fail_at {
            return Err(injected());
        }
        Ok(())
    }
    fn answer(&mut self, name: &str, fallback: bool) -> bool {
        let Some(v) = self.answers.get(name) else {
            return fallback;
        };
        if let Some(list) = v.as_array() {
            let calls = self.answer_calls.entry(name.into()).or_insert(0);
            let i = *calls;
            *calls += 1;
            return list
                .get(i)
                .or(list.last())
                .and_then(Value::as_bool)
                .unwrap_or(fallback);
        }
        v.as_bool().unwrap_or(fallback)
    }
    fn entity(&self, node: &NodeHandle) -> &Entity {
        &self.entities[self.by_node[&node.node]]
    }
    fn entity_mut(&mut self, node: &NodeHandle) -> &mut Entity {
        let i = self.by_node[&node.node];
        &mut self.entities[i]
    }
    fn name(&self, node: &NodeHandle) -> String {
        self.entity(node).name.clone()
    }
    fn add_entity(&mut self, name: String, kind: Kind, location: Option<Location>) -> NodeHandle {
        let node = self.state.add_npc_at(-1, None, [1, 2, 3, 4]);
        let handle = self.state.retain_node(node).unwrap();
        self.by_node.insert(handle.node, self.entities.len());
        self.entities.push(Entity {
            name,
            node: handle.clone(),
            kind,
            location,
            active: true,
            attributes: BTreeMap::new(),
            teleport: None,
            spawn: None,
            safe: false,
            safe_respawn: None,
        });
        handle
    }
    fn player(&self) -> NodeHandle {
        self.entities[0].node.clone()
    }
    fn by_name(&self, name: &Value) -> NodeHandle {
        match name.as_str() {
            None => self.player(),
            Some(n) => self
                .entities
                .iter()
                .find(|e| e.name == n)
                .unwrap()
                .node
                .clone(),
        }
    }
    fn track(&mut self, object: Tracked, kind: &str) {
        let n = self.counters.entry(kind.into()).or_insert(0);
        *n += 1;
        let label = format!("{kind}#{n}");
        self.tracked.push((object, label));
    }
    fn plugin_label(&self, core: &Arc<Core<Model>>) -> Option<String> {
        self.tracked.iter().find_map(|(t, l)| match t {
            Tracked::Plugin(z) => plugin::core(z)
                .ok()
                .filter(|c| Arc::ptr_eq(c, core))
                .map(|_| l.clone()),
            _ => None,
        })
    }
    fn zone_label(&self, zone: &Zone<Model>) -> Option<String> {
        self.tracked.iter().find_map(|(t, l)| match t {
            Tracked::Plugin(z) if z.same(zone) => Some(l.clone()),
            _ => None,
        })
    }
    fn cutscene_label(&self, c: &Cutscene<Model>) -> Option<String> {
        self.tracked.iter().find_map(|(t, l)| match t {
            Tracked::Cutscene(x) if x.same(c) => Some(l.clone()),
            _ => None,
        })
    }
    fn plugin_ref(&self, label: &Value) -> Zone<Model> {
        self.tracked
            .iter()
            .find_map(|(t, l)| match t {
                Tracked::Plugin(z) if l == label.as_str().unwrap() => Some(z.clone()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("unknown plugin {label}"))
    }
    fn cutscene_ref(&self, label: &Value) -> Cutscene<Model> {
        self.tracked
            .iter()
            .find_map(|(t, l)| match t {
                Tracked::Cutscene(c) if l == label.as_str().unwrap() => Some(c.clone()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("unknown cutscene {label}"))
    }
    fn value(&self, v: &AttributeValue) -> Value {
        match v {
            AttributeValue::Int(i) => json!(i),
            AttributeValue::Long(i) => json!({ "long": i }),
            AttributeValue::Bool(b) => json!(b),
            AttributeValue::Text(t) => json!(t),
            AttributeValue::Location(l) => json!({ "location": l.coordinates() }),
            AttributeValue::Object(o) => {
                if let Some(p) = o.as_any().downcast_ref::<PluginRef<Model>>() {
                    return json!({ "object": self.zone_label(&p.0) });
                }
                if let Some(c) = o.as_any().downcast_ref::<CutsceneRef<Model>>() {
                    return json!({ "object": self.cutscene_label(&c.0) });
                }
                panic!("unknown attribute object")
            }
            other => panic!("unexpected attribute value {:?}", other.java_string()),
        }
    }
    fn write_frame(&mut self, frame: &Frame) {
        let mut wire = vec![];
        frame.write(Wire::ServerToClient, None, &mut wire).unwrap();
        self.frames.push(hex(&wire));
    }
    fn owner_label(&self, pulse: &pulses::Handle) -> Value {
        match self.pulses.owner(pulse) {
            Some(activities::Owner::Plugin(core)) => json!(self.plugin_label(&core)),
            Some(activities::Owner::Cutscene(c)) => json!(self.cutscene_label(&c)),
            None => Value::Null,
        }
    }
    /// RegionManager.move for an NPC from nowhere: onto the destination's
    /// chunk, then the zone monitor at its location.
    fn place_npc(&mut self, npc: &NodeHandle, dst: &Location) -> Result<()> {
        let r = self.scene.region(zones::region_id(dst));
        let [x, y, z] = dst.coordinates();
        let chunks = r.read().chunks.clone().ok_or(Error::Null)?;
        let chunk: HandleChunk = chunks.read()[((x & 63) >> 3) as usize][((y & 63) >> 3) as usize]
            [z as usize]
            .clone()
            .ok_or(Error::Null)?;
        chunk.read().npcs.add(Some(npc.clone()));
        let name = self.name(npc);
        self.record("zoneUpdate", vec![json!(name), coords(Some(dst))])
    }
    fn tick(&mut self) -> Result<()> {
        let count = self.queue.len();
        for _ in 0..count {
            let p = self.queue.remove(0);
            let finished = pulses::Context::update(self, &p, &mut ()).map_err(Error::from)?;
            if !finished && pulses::Context::is_running(self, &p, &mut ()).map_err(Error::from)? {
                self.queue.push(p);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------- scripted content

/// TestActivity / TestCutscenePlugin.
struct TestPlugin {
    config: Value,
    cutscene: bool,
}
fn configure_content(host: &mut Model, plugin: &Zone<Model>, config: &Value) -> Result<()> {
    let core = plugin::core(plugin)?;
    for a in config["configure"].as_array().cloned().unwrap_or_default() {
        match a["do"].as_str().unwrap() {
            "create" => {
                let r = host.create_instance(num(&a["region"]))?;
                core.set_region(Some(r));
            }
            "setRegionBase" => plugin::set_region_base(host, plugin)?,
            "setRegionBaseArea" => {
                let b = &a["borders"];
                let all = host.create_instance_area(&Borders::new(
                    num(&b[0]),
                    num(&b[1]),
                    num(&b[2]),
                    num(&b[3]),
                ))?;
                plugin::set_region_base_regions(host, plugin, &all)?;
            }
            "register" => {
                let b = &a["borders"];
                plugin::register_borders(
                    host,
                    plugin,
                    &Borders::new(num(&b[0]), num(&b[1]), num(&b[2]), num(&b[3])),
                )?;
            }
            "emit" => host.record("content", vec![a["value"].clone()])?,
            "fail" => return Err(injected()),
            other => panic!("unknown configure action {other}"),
        }
    }
    Ok(())
}
fn construct_plugin(host: &mut Model, config: &Value) -> Result<Zone<Model>> {
    let name = config["name"].as_str();
    let cutscene = config["kind"] == "cutscene";
    let content = Box::new(TestPlugin {
        config: config.clone(),
        cutscene,
    });
    let zone = if cutscene {
        let fade = config.get("fade").is_none_or(|f| f.as_bool() == Some(true));
        plugin::new_cutscene(host, name, fade, content)?
    } else {
        let restrictions: Vec<Restriction> = config["restrictions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| match r.as_str().unwrap() {
                "FOLLOWERS" => Restriction::Followers,
                "RANDOM_EVENTS" => Restriction::RandomEvents,
                "FIRES" => Restriction::Fires,
                "MEMBERS" => Restriction::Members,
                "CANNON" => Restriction::Cannon,
                "GRAVES" => Restriction::Graves,
                "TELEPORT" => Restriction::Teleport,
                other => panic!("restriction {other}"),
            })
            .collect();
        let b = |k: &str| config[k] == true;
        plugin::new_activity(
            host,
            name,
            b("instanced"),
            b("multicombat"),
            b("safe"),
            &restrictions,
            content,
        )?
    };
    host.track(
        Tracked::Plugin(zone.clone()),
        if cutscene {
            "cutscenePlugin"
        } else {
            "activity"
        },
    );
    Ok(zone)
}
impl Content<Model> for TestPlugin {
    fn new_instance(
        &self,
        host: &mut Model,
        plugin: &Zone<Model>,
        player: Option<&NodeHandle>,
    ) -> Result<Option<Zone<Model>>> {
        let label = host.zone_label(plugin);
        let name = player.map(|p| host.name(p));
        host.record("newInstance", vec![json!(label), json!(name)])?;
        Ok(Some(construct_plugin(host, &self.config)?))
    }
    fn spawn_location(&self, _: &mut Model, _: &Arc<Core<Model>>) -> Result<Option<Location>> {
        Ok(loc(&self.config["spawn"]))
    }
    fn register(&self, host: &mut Model, plugin: &Zone<Model>) -> Result<()> {
        if self.cutscene {
            return Ok(());
        }
        let label = host.zone_label(plugin);
        host.record("contentRegister", vec![json!(label)])
    }
    fn configure(&self, host: &mut Model, plugin: &Zone<Model>) -> Result<()> {
        let label = host.zone_label(plugin);
        host.record("contentConfigure", vec![json!(label)])?;
        configure_content(host, plugin, &self.config)
    }
    fn start(
        &self,
        host: &mut Model,
        plugin: &Zone<Model>,
        player: &NodeHandle,
        login: bool,
        args: &[AttributeValue],
    ) -> Result<bool> {
        if !self.cutscene {
            let label = host.zone_label(plugin);
            let args: Vec<Value> = args.iter().map(|a| host.value(a)).collect();
            host.record(
                "contentStart",
                vec![json!(label), json!(login), json!(args)],
            )?;
        }
        plugin::start_base(host, plugin, player, login, args)
    }
    fn open(&self, host: &mut Model, core: &Arc<Core<Model>>) -> Result<()> {
        let label = host.plugin_label(core);
        host.record("open", vec![json!(label)])
    }
    fn fade(&self, host: &mut Model, core: &Arc<Core<Model>>) -> Result<()> {
        let label = host.plugin_label(core);
        host.record("fade", vec![json!(label)])
    }
    fn map_state(&self, _: &mut Model, _: &Arc<Core<Model>>) -> Result<i32> {
        Ok(self.config.get("mapState").map_or(2, num))
    }
    fn removed_tabs(&self, _: &mut Model, _: &Arc<Core<Model>>) -> Result<Vec<i32>> {
        Ok(match self.config.get("tabs") {
            None => plugin::TABS.to_vec(),
            Some(t) => t.as_array().unwrap().iter().map(num).collect(),
        })
    }
    fn start_location(&self, _: &mut Model, core: &Arc<Core<Model>>) -> Result<Option<Location>> {
        Ok(match self.config.get("startLocation") {
            Some(l) => loc(l),
            None => core.base(),
        })
    }
}

/// TestCutscene: setup and stages run the case's actions; `onContinue` and
/// `endActions` lambdas are action lists by id.
struct TestScript(Arc<ScriptInner>);
struct ScriptInner {
    script: Value,
    lambdas: std::sync::Mutex<Vec<Option<Vec<Value>>>>,
}
impl ScriptInner {
    fn lambda(&self, actions: Option<Vec<Value>>) -> u32 {
        let mut l = self.lambdas.lock().unwrap();
        l.push(actions);
        (l.len() - 1) as u32
    }
    fn run(
        &self,
        host: &mut Model,
        c: &Cutscene<Model>,
        actions: Option<&Vec<Value>>,
    ) -> Result<()> {
        let Some(actions) = actions else {
            return Ok(());
        };
        for a in actions {
            self.act(host, c, a)?;
        }
        Ok(())
    }
    fn act(&self, host: &mut Model, c: &Cutscene<Model>, a: &Value) -> Result<()> {
        let n = |k: &str| num(&a[k]);
        let cont = a
            .get("onContinue")
            .map(|l| self.lambda(Some(l.as_array().unwrap().clone())));
        let entity = |host: &mut Model| -> Result<NodeHandle> {
            if a.get("npc").is_some_and(|v| !v.is_null()) {
                cutscene::get_npc(c, n("npc")).ok_or(Error::Null)
            } else {
                Ok(host.player())
            }
        };
        let expression = |a: &Value| -> Expression {
            match a["expression"].as_str().unwrap() {
                "HAPPY" => "HAPPY",
                "NEUTRAL" => "NEUTRAL",
                "ASKING" => "ASKING",
                other => panic!("expression {other}"),
            }
        };
        let lines = |a: &Value| -> Vec<String> {
            a["lines"]
                .as_array()
                .unwrap()
                .iter()
                .map(|l| l.as_str().unwrap().to_owned())
                .collect()
        };
        let plane = || a.get("plane").map_or(0, num);
        match a["do"].as_str().unwrap() {
            "loadRegion" => cutscene::load_region(host, c, n("region")),
            "closeOverlay" => cutscene::close_overlay(host, c),
            "fadeToBlack" => cutscene::fade_to_black(host, c),
            "fadeFromBlack" => cutscene::fade_from_black(host, c),
            "teleport" => {
                let e = entity(host)?;
                cutscene::teleport(host, c, &e, n("x"), n("y"), plane())
            }
            "move" => {
                let e = entity(host)?;
                cutscene::move_entity(host, c, &e, n("x"), n("y"))
            }
            "npcDialogue" => cutscene::npc_dialogue(
                host,
                c,
                n("id"),
                expression(a),
                a["message"].as_str().unwrap(),
                cont,
                a["hide"] == true,
            ),
            "npcDialogueLines" => {
                cutscene::npc_dialogue_lines(host, c, n("id"), expression(a), &lines(a), cont)
            }
            "dialogueClose" => cutscene::dialogue_close(host, c),
            "dialogue" => cutscene::dialogue(host, c, a["message"].as_str().unwrap(), cont),
            "dialogueLines" => cutscene::dialogue_lines(host, c, &lines(a), cont),
            "playerDialogue" => cutscene::player_dialogue(
                host,
                c,
                expression(a),
                a["message"].as_str().unwrap(),
                cont,
            ),
            "timedUpdate" => {
                cutscene::timed_update(host, c, n("ticks"), a.get("stage").map_or(-1, num))
            }
            "getNPC" => {
                let npc = cutscene::get_npc(c, n("id")).map(|x| host.name(&x));
                host.record("gotNPC", vec![json!(npc)])
            }
            "getNPCs" => {
                let names: Vec<String> = cutscene::get_npcs(c, n("id"))
                    .iter()
                    .map(|x| host.name(x))
                    .collect();
                host.record("gotNPCs", vec![json!(names)])
            }
            "getObject" => {
                let o = cutscene::get_object(host, c, n("x"), n("y"), plane())?;
                host.record("gotObject", vec![json!(o.map(|o| o.read().id))])
            }
            "addNPC" => {
                let d = Direction::value_of(a["direction"].as_str().unwrap()).unwrap();
                cutscene::add_npc(host, c, n("id"), n("x"), n("y"), d, plane())
            }
            "endWithoutFade" => {
                let actions = if cont.is_some() || a.get("endActions").is_some() {
                    Some(self.lambda(a.get("endActions").and_then(Value::as_array).cloned()))
                } else {
                    None
                };
                cutscene::end_without_fade(host, c, actions)
            }
            "end" => {
                let fade = a.get("fade");
                if let Some(actions) = a.get("endActions") {
                    let id = self.lambda(actions.as_array().cloned());
                    cutscene::end(host, c, fade != Some(&Value::Bool(false)), Some(id))
                } else {
                    cutscene::end(host, c, fade.is_none_or(|f| f == &Value::Bool(true)), None)
                }
            }
            "moveCamera" => {
                let (h, s) = a
                    .get("height")
                    .map_or((300, 100), |_| (n("height"), n("speed")));
                cutscene::move_camera(host, c, n("x"), n("y"), h, s)
            }
            "rotateCamera" => {
                let (h, s) = a
                    .get("height")
                    .map_or((300, 100), |_| (n("height"), n("speed")));
                cutscene::rotate_camera(host, c, n("x"), n("y"), h, s)
            }
            "rotateCameraBy" => {
                let (h, s) = a
                    .get("height")
                    .map_or((300, 100), |_| (n("height"), n("speed")));
                cutscene::rotate_camera_by(host, c, n("x"), n("y"), h, s)
            }
            "shakeCamera" => {
                let kind = ["TRUCK", "PEDESTAL", "DOLLY", "PAN", "TILT"]
                    .iter()
                    .position(|t| *t == a["type"].as_str().unwrap())
                    .unwrap() as i32;
                let (j, am, f, s) = a.get("jitter").map_or((0, 0, 128, 2), |_| {
                    (n("jitter"), n("amplitude"), n("frequency"), n("speed"))
                });
                cutscene::shake_camera(host, c, kind, j, am, f, s)
            }
            "resetCamera" => cutscene::reset_camera(host, c),
            "setExit" => {
                cutscene::set_exit(c, &loc(&a["location"]).unwrap());
                Ok(())
            }
            "incrementStage" => cutscene::increment_stage(host, c),
            "updateStage" => cutscene::update_stage(host, c, n("stage")),
            "clearNPCs" => cutscene::clear_npcs(host, c),
            "log" => cutscene::log_cutscene(host, c, a["message"].as_str().unwrap()),
            "emit" => host.record("content", vec![a["value"].clone()]),
            "fail" => Err(injected()),
            other => panic!("unknown cutscene action {other}"),
        }
    }
}
impl cutscene::Script<Model> for TestScript {
    fn setup(&self, host: &mut Model, c: &Cutscene<Model>) -> Result<()> {
        let label = host.cutscene_label(c);
        host.record("setup", vec![json!(label)])?;
        self.0.run(
            host,
            c,
            self.0.script.get("setup").and_then(Value::as_array),
        )
    }
    fn run_stage(&self, host: &mut Model, c: &Cutscene<Model>, stage: i32) -> Result<()> {
        let label = host.cutscene_label(c);
        host.record("runStage", vec![json!(label), json!(stage)])?;
        let actions = self
            .0
            .script
            .get("stages")
            .and_then(|s| s.get(stage.to_string()))
            .and_then(Value::as_array);
        self.0.run(host, c, actions)
    }
    fn callback(&self, host: &mut Model, c: &Cutscene<Model>, id: u32) -> Result<()> {
        let actions = self.0.lambdas.lock().unwrap()[id as usize].clone();
        self.0.run(host, c, actions.as_ref())
    }
    fn class_name(&self) -> &str {
        "oracle.content.TestCutscene"
    }
}

// ---------------------------------------------------------------- the world's services

impl rs09_server::world::region::Context for Model {
    fn archive_id(&mut self, name: &str) -> region::Result<i32> {
        region::Context::archive_id(&mut world!(self), name)
    }
    fn mapscape_bytes(&mut self, group: i32) -> region::Result<Option<Vec<u8>>> {
        region::Context::mapscape_bytes(&mut world!(self), group)
    }
    fn xtea(&mut self, region: i32) -> region::Result<[u32; 4]> {
        region::Context::xtea(&mut world!(self), region)
    }
    fn landscape_bytes(&mut self, group: i32, key: [u32; 4]) -> region::Result<Option<Vec<u8>>> {
        region::Context::landscape_bytes(&mut world!(self), group, key)
    }
    fn parse_mapscape(&mut self, r: &region::Handle, m: &Mapscape, b: &[u8]) -> region::Result<()> {
        region::Context::parse_mapscape(&mut world!(self), r, m, b)
    }
    fn parse_landscape(
        &mut self,
        r: &region::Handle,
        c: Option<Chunks>,
        m: &Mapscape,
        b: &[u8],
        build: bool,
    ) -> region::Result<()> {
        region::Context::parse_landscape(&mut world!(self), r, c, m, b, build)
    }
    fn clip_mapscape(
        &mut self,
        r: &region::Handle,
        c: Option<Chunks>,
        m: &Mapscape,
    ) -> region::Result<()> {
        region::Context::clip_mapscape(&mut world!(self), r, c, m)
    }
    fn object(
        &mut self,
        l: Location,
        id: i32,
        shape: i32,
    ) -> region::Result<Option<doors::Handle>> {
        region::Context::object(&mut world!(self), l, id, shape)
    }
    fn remove(&mut self, h: &doors::Handle) -> region::Result<()> {
        region::Context::remove(&mut world!(self), h)
    }
    fn add(&mut self, h: &doors::Handle) -> region::Result<()> {
        region::Context::add(&mut world!(self), h)
    }
    fn exception(&mut self, e: &region::Error, id: Option<i32>) -> region::Result<()> {
        region::Context::exception(&mut world!(self), e, id)
    }
}
impl region_activity::Host for Model {
    fn submit_pulse(&mut self, pulse: &pulses::Handle) -> zones::Result<()> {
        let class = pulse.read().class_name.clone();
        let owner = self.owner_label(pulse);
        self.record("submit", vec![json!(class), owner])?;
        self.queue.push(pulse.clone());
        Ok(())
    }
    fn npc_active(&mut self, npc: &NodeHandle) -> zones::Result<bool> {
        Ok(self.entity(npc).active)
    }
    fn add_renderable_npc(&mut self, _: &NodeHandle) -> zones::Result<()> {
        Err(zones::Error::Binding(
            "renderable NPCs are outside this corpus".into(),
        ))
    }
    fn npc_region_inactivity(&mut self, _: &NodeHandle) -> zones::Result<()> {
        Err(zones::Error::Binding(
            "region inactivity is outside this corpus".into(),
        ))
    }
    fn log_error(&mut self, message: &str) -> zones::Result<()> {
        Ok(self.record("log", vec![json!("ERR"), json!(message)])?)
    }
    fn chunk_region(&mut self, region_id: i32) -> zones::Result<region::Handle> {
        Ok(self.scene.region(region_id))
    }
}
impl dynamic::Host for Model {
    type Zones = Model;
    fn reserved_areas(&mut self) -> zones::Result<dynamic::Reserved> {
        Ok(self.scene.reserved_areas.clone())
    }
    fn multiway_zone(&mut self) -> zones::Result<Zone<Model>> {
        if self.multiway.is_none() {
            self.multiway = Some(Zone::new(Some("Multicombat"), true, &[], Multiway));
        }
        Ok(self.multiway.clone().unwrap())
    }
    fn remove_region(&mut self, id: i32) -> zones::Result<Option<region::Handle>> {
        Ok(self.scene.regions.remove(&id))
    }
    fn is_familiar(&mut self, _: &NodeHandle) -> zones::Result<bool> {
        Ok(false)
    }
    fn recall_familiar(&mut self, _: &NodeHandle) -> zones::Result<()> {
        Err(zones::Error::Binding(
            "familiars are outside this corpus".into(),
        ))
    }
    fn clear_npc(&mut self, npc: &NodeHandle) -> zones::Result<()> {
        let name = self.name(npc);
        Ok(self.record("npcClear", vec![json!(name)])?)
    }
    fn destroy_ground_item(&mut self, _: Option<&ground_item::Handle>) -> zones::Result<()> {
        Err(zones::Error::Binding(
            "ground items are outside this corpus".into(),
        ))
    }
}
/// The stand-in multiway zone object (`MultiwayCombatZone.getInstance()`).
struct Multiway;
impl zones::Behavior<Model> for Multiway {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
fn unused<T>() -> zones::Result<T> {
    Err(zones::Error::Binding("outside this corpus".into()))
}
impl zones::Host for Model {
    type Target = ();
    type Opaque = ();
    fn kind(&mut self, entity: &NodeHandle) -> zones::Result<EntityKind> {
        Ok(match self.entity(entity).kind {
            Kind::Player => EntityKind::Player,
            Kind::Npc => EntityKind::Npc,
        })
    }
    fn monitor(&mut self, _: &NodeHandle) -> zones::Result<Monitor<Self>> {
        unused()
    }
    fn region(&mut self, region_id: i32) -> zones::Result<region::Handle> {
        Ok(self.scene.region(region_id))
    }
    fn target_entity(&mut self, _: &()) -> zones::Result<Option<NodeHandle>> {
        unused()
    }
    fn location(&mut self, entity: &NodeHandle) -> zones::Result<Option<Location>> {
        Ok(self.entity(entity).location.clone())
    }
    fn is_artificial(&mut self, _: &NodeHandle) -> zones::Result<bool> {
        unused()
    }
    fn entity_update_location(
        &mut self,
        _: &NodeHandle,
        _: Option<&Location>,
    ) -> zones::Result<()> {
        unused()
    }
    fn set_invisible(&mut self, npc: &NodeHandle, invisible: bool) -> zones::Result<()> {
        let name = self.name(npc);
        Ok(self.record("setInvisible", vec![json!(name), json!(invisible)])?)
    }
    fn entity_continue_attack(
        &mut self,
        _: &NodeHandle,
        _: &NodeHandle,
        _: Option<&()>,
        _: bool,
    ) -> zones::Result<bool> {
        unused()
    }
    fn skull_wilderness(&mut self, _: &NodeHandle) -> zones::Result<bool> {
        unused()
    }
    fn skull_level(&mut self, _: &NodeHandle) -> zones::Result<i32> {
        unused()
    }
    fn packet_message(&mut self, player: &NodeHandle, message: &str) -> zones::Result<()> {
        let name = self.name(player);
        Ok(self.record("packetMessage", vec![json!(name), json!(message)])?)
    }
    fn player_message(&mut self, _: &NodeHandle, _: &str) -> zones::Result<()> {
        unused()
    }
    fn has_timer(&mut self, _: &NodeHandle, _: &str) -> zones::Result<bool> {
        unused()
    }
    fn teleport_locked(&mut self, _: &NodeHandle) -> zones::Result<bool> {
        unused()
    }
    fn target_item_id(&mut self, _: Option<&()>) -> zones::Result<i32> {
        unused()
    }
    fn now_millis(&mut self) -> zones::Result<i64> {
        unused()
    }
    fn multi_zone(&mut self, _: &NodeHandle) -> zones::Result<bool> {
        unused()
    }
    fn entity_ignore_multi(&mut self, _: &NodeHandle, _: &NodeHandle) -> zones::Result<bool> {
        unused()
    }
    fn familiar_owner(&mut self, _: &NodeHandle) -> zones::Result<Option<NodeHandle>> {
        unused()
    }
    fn combat_attacker(&mut self, _: &NodeHandle) -> zones::Result<Option<NodeHandle>> {
        unused()
    }
    fn combat_time(&mut self, _: &NodeHandle) -> zones::Result<Option<i64>> {
        unused()
    }
    fn active(&mut self, entity: &NodeHandle) -> zones::Result<bool> {
        Ok(self.entity(entity).active)
    }
    fn id(&mut self, _: &NodeHandle) -> zones::Result<i32> {
        unused()
    }
    fn music_playing(&mut self, _: &NodeHandle) -> zones::Result<bool> {
        unused()
    }
    fn music_play_default(&mut self, _: &NodeHandle) -> zones::Result<()> {
        unused()
    }
    fn music_unlock(&mut self, _: &NodeHandle, _: i32, _: Option<bool>) -> zones::Result<()> {
        unused()
    }
    fn random_next_int(&mut self, _: i32) -> zones::Result<i32> {
        unused()
    }
    fn fresh_random_next_int(&mut self, _: i32) -> zones::Result<i32> {
        unused()
    }
    fn teleport_permitted(&mut self, _: &Location) -> zones::Result<bool> {
        unused()
    }
}
/// Building an instance reaches none of these in this corpus: no template
/// region holds NPCs or players and no chunk is rotated.
struct NoBuild;
impl<A: Assets> dynamic::Build<A> for NoBuild {
    fn update_scene_graph(&mut self, _: &mut Scene<A>, _: &NodeHandle) -> zones::Result<()> {
        unused()
    }
    fn npc_location(&mut self, _: &mut Scene<A>, _: &NodeHandle) -> zones::Result<Location> {
        unused()
    }
    fn copy_npc(&mut self, _: &mut Scene<A>, _: &NodeHandle, _: Location) -> zones::Result<()> {
        unused()
    }
    fn log_error(&mut self, _: &mut Scene<A>, _: &str) -> zones::Result<()> {
        unused()
    }
}
impl pulses::Context<()> for Model {
    fn pulse(&mut self, pulse: &pulses::Handle, _: &mut ()) -> pulses::Result<bool> {
        match activities::run_pulse(self, pulse) {
            Some(result) => Ok(result?),
            None => Err(pulses::Error::Binding("not an activity pulse".into())),
        }
    }
    fn stop(&mut self, pulse: &pulses::Handle, _: &mut ()) -> pulses::Result<()> {
        match activities::stop_pulse(self, pulse) {
            Some(result) => Ok(result?),
            None => {
                pulses::stop_base(pulse);
                Ok(())
            }
        }
    }
    fn node_active(&mut self, node: pulses::Node, _: &mut ()) -> pulses::Result<bool> {
        Ok(self.entities[self.by_node[&node]].active)
    }
    fn exception(&mut self, _: &pulses::Error, _: &mut ()) -> pulses::Result<()> {
        self.events.push(json!(["stackTrace"]));
        Ok(())
    }
    fn remove_weak_scripts(&mut self, _: &mut ()) -> pulses::Result<()> {
        unreachable!()
    }
    fn remove_normal_scripts(&mut self, _: &mut ()) -> pulses::Result<()> {
        unreachable!()
    }
    fn is_dead(&mut self, _: &mut ()) -> pulses::Result<bool> {
        unreachable!()
    }
    fn send_message(&mut self, _: pulses::Node, _: &str, _: &mut ()) -> pulses::Result<()> {
        unreachable!()
    }
    fn now_millis(&mut self, _: &mut ()) -> pulses::Result<i64> {
        Ok(0)
    }
    fn log(&mut self, _: pulses::Level, _: String, _: &mut ()) -> pulses::Result<()> {
        unreachable!()
    }
    fn pulse_length(&mut self, _: &str, _: i32, _: &mut ()) -> pulses::Result<()> {
        Ok(())
    }
    fn count_pulse(&mut self, _: &str, _: &mut ()) -> pulses::Result<()> {
        Ok(())
    }
    fn metrics_enabled(&self, _: &()) -> bool {
        false
    }
    fn set_bot_total(&mut self, _: i32, _: &mut ()) -> pulses::Result<()> {
        Ok(())
    }
    fn set_other_total(&mut self, _: i32, _: &mut ()) -> pulses::Result<()> {
        Ok(())
    }
}

impl Host for Model {
    fn dev_mode(&mut self) -> Result<Option<bool>> {
        let s = &self.world["settings"];
        Ok(if s.is_null() {
            None
        } else {
            Some(s["devMode"] == true)
        })
    }
    fn home_location(&mut self) -> Result<Option<Location>> {
        Ok(loc(&self.world["home"]))
    }
    fn log_cutscene(&mut self) -> Result<bool> {
        Ok(self.world["logCutscene"] != false)
    }
    fn log(&mut self, _: &str, level: Level, message: &str) -> Result<()> {
        self.record("log", vec![json!(level.name()), json!(message)])
    }
    fn print_stack_trace(&mut self, _: &Error) -> Result<()> {
        self.events.push(json!(["stackTrace"]));
        Ok(())
    }
    fn create_instance(&mut self, region_id: i32) -> Result<region::Handle> {
        let mut build = NoBuild;
        Ok(dynamic::create(&mut world!(self), &mut build, region_id)?)
    }
    fn create_instance_area(&mut self, borders: &Borders) -> Result<Vec<region::Handle>> {
        let mut build = NoBuild;
        Ok(dynamic::create_area(
            &mut world!(self),
            &mut build,
            borders,
        )?)
    }
    fn scenery(&mut self, location: &Location) -> Result<Option<Scenery>> {
        world!(self)
            .object(location.clone(), -1, -1)
            .map_err(|e| Error::Binding(e.0))
    }
    fn scenery_name(&mut self, object: &Scenery) -> Result<Option<String>> {
        let d = scenery_node::definition(object).map_err(|e| Error::Binding(format!("{e:?}")))?;
        let name = d.read().name.clone();
        Ok(name)
    }
    fn username(&mut self, entity: &NodeHandle) -> Result<String> {
        Ok(self.name(entity))
    }
    fn set_location(&mut self, entity: &NodeHandle, location: &Location) -> Result<()> {
        let name = self.name(entity);
        self.record("setLocation", vec![json!(name), coords(Some(location))])?;
        self.entity_mut(entity).location = Some(location.clone());
        Ok(())
    }
    fn get_attribute(&mut self, entity: &NodeHandle, key: &str) -> Result<Option<AttributeValue>> {
        let name = self.name(entity);
        self.record("getAttribute", vec![json!(name), json!(key)])?;
        Ok(self.entity(entity).attributes.get(key).cloned())
    }
    fn set_attribute(
        &mut self,
        entity: &NodeHandle,
        key: &str,
        value: AttributeValue,
    ) -> Result<()> {
        let name = self.name(entity);
        let shown = self.value(&value);
        self.record("setAttribute", vec![json!(name), json!(key), shown])?;
        self.entity_mut(entity).attributes.insert(key.into(), value);
        Ok(())
    }
    fn remove_attribute(&mut self, entity: &NodeHandle, key: &str) -> Result<()> {
        let name = self.name(entity);
        self.record("removeAttribute", vec![json!(name), json!(key)])?;
        self.entity_mut(entity).attributes.remove(key);
        Ok(())
    }
    fn lock(&mut self, entity: &NodeHandle) -> Result<()> {
        let name = self.name(entity);
        self.record("lock", vec![json!(name)])
    }
    fn unlock(&mut self, entity: &NodeHandle) -> Result<()> {
        let name = self.name(entity);
        self.record("unlock", vec![json!(name)])
    }
    fn walking_reset(&mut self, entity: &NodeHandle) -> Result<()> {
        let name = self.name(entity);
        self.record("walkingReset", vec![json!(name)])
    }
    fn lock_movement(&mut self, entity: &NodeHandle, ticks: i32) -> Result<()> {
        let name = self.name(entity);
        self.record("lockMovement", vec![json!(name), json!(ticks)])
    }
    fn unlock_movement(&mut self, entity: &NodeHandle) -> Result<()> {
        let name = self.name(entity);
        self.record("unlockMovement", vec![json!(name)])
    }
    fn set_teleport_location(
        &mut self,
        entity: &NodeHandle,
        location: Option<&Location>,
    ) -> Result<()> {
        let name = self.name(entity);
        self.record("teleportLocation", vec![json!(name), coords(location)])?;
        self.entity_mut(entity).teleport = location.cloned();
        Ok(())
    }
    fn set_spawn_location(
        &mut self,
        entity: &NodeHandle,
        location: Option<&Location>,
    ) -> Result<()> {
        let name = self.name(entity);
        self.record("spawnLocation", vec![json!(name), coords(location)])?;
        self.entity_mut(entity).spawn = location.cloned();
        Ok(())
    }
    fn set_safe_zone(&mut self, entity: &NodeHandle, safe: bool) -> Result<()> {
        let name = self.name(entity);
        self.record("safeZone", vec![json!(name), json!(safe)])?;
        self.entity_mut(entity).safe = safe;
        Ok(())
    }
    fn set_safe_respawn(&mut self, entity: &NodeHandle, location: Option<&Location>) -> Result<()> {
        self.entity_mut(entity).safe_respawn = location.cloned();
        Ok(())
    }
    fn face_location(&mut self, entity: &NodeHandle, location: &Location) -> Result<()> {
        let name = self.name(entity);
        self.record("faceLocation", vec![json!(name), coords(Some(location))])
    }
    fn run_movement(&mut self, entity: &NodeHandle, destination: &Location) -> Result<()> {
        let name = self.name(entity);
        self.record(
            "movementPulse",
            vec![json!(name), coords(Some(destination))],
        )
    }
    fn hook_self_death(&mut self, entity: &NodeHandle) -> Result<()> {
        let name = self.name(entity);
        self.record("hook", vec![json!(name), json!("SelfDeathEvent")])?;
        self.hooks += 1;
        Ok(())
    }
    fn unhook_self_death(&mut self, entity: &NodeHandle) -> Result<()> {
        let name = self.name(entity);
        self.record("unhook", vec![json!(name)])?;
        self.hooks = self.hooks.saturating_sub(1);
        Ok(())
    }
    fn remove_tabs(&mut self, player: &NodeHandle, tabs: &[i32]) -> Result<()> {
        let name = self.name(player);
        self.record("removeTabs", vec![json!(name), json!(tabs)])
    }
    fn restore_tabs(&mut self, player: &NodeHandle) -> Result<()> {
        let name = self.name(player);
        self.record("restoreTabs", vec![json!(name)])
    }
    fn open_default_tabs(&mut self, player: &NodeHandle) -> Result<()> {
        let name = self.name(player);
        self.record("openDefaultTabs", vec![json!(name)])
    }
    fn close_interfaces(&mut self, player: &NodeHandle) -> Result<bool> {
        let name = self.name(player);
        self.record("closeInterface", vec![json!(name)])?;
        Ok(self.answer("closeInterface", true))
    }
    fn open_overlay(&mut self, player: &NodeHandle, component: i32) -> Result<()> {
        let name = self.name(player);
        self.record("openOverlay", vec![json!(name), json!(component)])
    }
    fn close_overlay(&mut self, player: &NodeHandle) -> Result<()> {
        let name = self.name(player);
        self.record("closeOverlay", vec![json!(name)])
    }
    fn minimap_state(&mut self, _: &NodeHandle, state: i32) -> Result<()> {
        // PacketRepository.send: the session's write; a failing write is
        // caught and logged.
        if self.record("write", vec![]).is_err() {
            return self.record("packetError", vec![json!("MinimapState")]);
        }
        self.write_frame(&dynamic_scene::minimap_state(state));
        Ok(())
    }
    fn session_active(&mut self, _: &NodeHandle) -> Result<bool> {
        self.record("sessionActive", vec![])?;
        Ok(self.answer("sessionActive", true))
    }
    fn put_logout_listener(
        &mut self,
        player: &NodeHandle,
        key: &str,
        c: Cutscene<Model>,
    ) -> Result<()> {
        let name = self.name(player);
        self.record("logoutListener", vec![json!(name), json!(key)])?;
        self.logout.insert(key.into(), c);
        Ok(())
    }
    fn remove_logout_listener(&mut self, player: &NodeHandle, key: &str) -> Result<()> {
        let name = self.name(player);
        self.record("removeLogoutListener", vec![json!(name), json!(key)])?;
        self.logout.remove(key);
        Ok(())
    }
    fn add_dialogue_action(&mut self, player: &NodeHandle, action: Continue<Model>) -> Result<()> {
        let name = self.name(player);
        self.record("addAction", vec![json!(name)])?;
        self.actions.push(action);
        Ok(())
    }
    fn send_npc_dialogue(
        &mut self,
        player: &NodeHandle,
        npc: i32,
        message: &str,
        expression: Expression,
        hide: bool,
    ) -> Result<()> {
        let name = self.name(player);
        self.record(
            "sendNPCDialogue",
            vec![
                json!(name),
                json!(npc),
                json!(message),
                json!(expression),
                json!(hide),
            ],
        )
    }
    fn send_npc_dialogue_lines(
        &mut self,
        player: &NodeHandle,
        npc: i32,
        expression: Expression,
        hide: bool,
        lines: &[String],
    ) -> Result<()> {
        let name = self.name(player);
        self.record(
            "sendNPCDialogueLines",
            vec![
                json!(name),
                json!(npc),
                json!(expression),
                json!(hide),
                json!(lines),
            ],
        )
    }
    fn close_dialogue(&mut self, player: &NodeHandle) -> Result<()> {
        let name = self.name(player);
        self.record("closeDialogue", vec![json!(name)])
    }
    fn send_dialogue(&mut self, player: &NodeHandle, message: &str) -> Result<()> {
        let name = self.name(player);
        self.record("sendDialogue", vec![json!(name), json!(message)])
    }
    fn send_dialogue_lines(&mut self, player: &NodeHandle, lines: &[String]) -> Result<()> {
        let name = self.name(player);
        self.record("sendDialogueLines", vec![json!(name), json!(lines)])
    }
    fn send_player_dialogue(
        &mut self,
        player: &NodeHandle,
        message: &str,
        expression: Expression,
        hide: bool,
    ) -> Result<()> {
        let name = self.name(player);
        self.record(
            "sendPlayerDialogue",
            vec![json!(name), json!(message), json!(expression), json!(hide)],
        )
    }
    fn anti_macro_pause(&mut self, player: &NodeHandle) -> Result<()> {
        let name = self.name(player);
        self.record("antiMacroPause", vec![json!(name)])
    }
    fn anti_macro_unpause(&mut self, player: &NodeHandle) -> Result<()> {
        let name = self.name(player);
        self.record("antiMacroUnpause", vec![json!(name)])
    }
    fn camera_new(&mut self, player: &NodeHandle) -> Result<()> {
        let name = self.name(player);
        self.record("camera", vec![json!(name)])
    }
    fn camera_rotate_to(
        &mut self,
        player: &NodeHandle,
        x: i32,
        y: i32,
        height: i32,
        speed: i32,
    ) -> Result<()> {
        let name = self.name(player);
        self.record(
            "cameraRotateTo",
            vec![json!(name), json!(x), json!(y), json!(height), json!(speed)],
        )
    }
    fn camera_rotate_by(
        &mut self,
        player: &NodeHandle,
        x: i32,
        y: i32,
        height: i32,
        speed: i32,
    ) -> Result<()> {
        let name = self.name(player);
        self.record(
            "cameraRotateBy",
            vec![json!(name), json!(x), json!(y), json!(height), json!(speed)],
        )
    }
    fn camera_pan_to(
        &mut self,
        player: &NodeHandle,
        x: i32,
        y: i32,
        height: i32,
        speed: i32,
    ) -> Result<()> {
        let name = self.name(player);
        self.record(
            "cameraPanTo",
            vec![json!(name), json!(x), json!(y), json!(height), json!(speed)],
        )
    }
    fn camera_shake(
        &mut self,
        player: &NodeHandle,
        kind: i32,
        jitter: i32,
        amplitude: i32,
        frequency: i32,
        speed: i32,
    ) -> Result<()> {
        let name = self.name(player);
        self.record(
            "cameraShake",
            vec![
                json!(name),
                json!(kind),
                json!(jitter),
                json!(amplitude),
                json!(frequency),
                json!(speed),
            ],
        )
    }
    fn camera_reset(&mut self, player: &NodeHandle) -> Result<()> {
        let name = self.name(player);
        self.record("cameraReset", vec![json!(name)])
    }
    fn npc_new(&mut self, id: i32) -> Result<NodeHandle> {
        self.npcs += 1;
        let name = format!("npc{}", self.npcs);
        let npc = self.add_entity(name.clone(), Kind::Npc, None);
        self.record("npcNew", vec![json!(name), json!(id)])?;
        Ok(npc)
    }
    fn npc_set_respawn(&mut self, npc: &NodeHandle, value: bool) -> Result<()> {
        let name = self.name(npc);
        self.record("npcRespawn", vec![json!(name), json!(value)])
    }
    fn npc_set_aggressive(&mut self, npc: &NodeHandle, value: bool) -> Result<()> {
        let name = self.name(npc);
        self.record("npcAggressive", vec![json!(name), json!(value)])
    }
    fn npc_set_walks(&mut self, npc: &NodeHandle, value: bool) -> Result<()> {
        let name = self.name(npc);
        self.record("npcWalks", vec![json!(name), json!(value)])
    }
    fn npc_init(&mut self, npc: &NodeHandle) -> Result<()> {
        let name = self.name(npc);
        self.record("npcInit", vec![json!(name)])
    }
    fn activities(&mut self) -> &mut Manager<Model> {
        &mut self.manager
    }
    fn activity_pulses(&mut self) -> &mut Pulses<Model> {
        &mut self.pulses
    }
}

// ---------------------------------------------------------------- operations and snapshots

fn error_name(e: &Error) -> String {
    match e {
        Error::Null => "Null".into(),
        Error::ClassCast => "ClassCast".into(),
        Error::Uninitialized(_) => "Uninitialized".into(),
        Error::Index => "Index".into(),
        Error::ConcurrentModification => "ConcurrentModification".into(),
        Error::Binding(m) if m.starts_with("injected") => "Failure".into(),
        other => panic!("unexpected native error {other:?}"),
    }
}

impl Model {
    fn act(&mut self, a: &Value) -> Result<Value> {
        let player = self.player();
        Ok(match a["op"].as_str().unwrap() {
            "register" => {
                let p = construct_plugin(self, &a["config"])?;
                manager::register(self, &p)?;
                json!(self.zone_label(&p))
            }
            "start" => {
                let args: Vec<AttributeValue> = a
                    .get("args")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default()
                    .iter()
                    .map(|v| match v {
                        Value::Number(n) => AttributeValue::Int(n.as_i64().unwrap() as i32),
                        Value::String(s) => AttributeValue::Text(s.clone()),
                        other => panic!("argument {other}"),
                    })
                    .collect();
                json!(manager::start(
                    self,
                    &player,
                    a["name"].as_str(),
                    a["login"] == true,
                    &args
                )?)
            }
            "getActivity" => {
                let p = manager::get_activity(self, a["name"].as_str());
                json!(p.and_then(|p| self.zone_label(&p)))
            }
            "enter" => {
                let z = self.plugin_ref(&a["plugin"]);
                let e = self.by_name(&a["entity"]);
                json!(z.enter(self, &e).map_err(Error::from)?)
            }
            "leave" => {
                let z = self.plugin_ref(&a["plugin"]);
                let e = self.by_name(&a["entity"]);
                json!(
                    z.leave(self, &e, a["logout"] == true)
                        .map_err(Error::from)?
                )
            }
            "fireEvent" => {
                let z = self.plugin_ref(&a["plugin"]);
                let core = plugin::core(&z)?;
                let v = core
                    .content
                    .fire_event(self, &z, a["id"].as_str().unwrap())?;
                v.map_or(Value::Null, |v| self.value(&v))
            }
            "pluginStart" => {
                let z = self.plugin_ref(&a["plugin"]);
                json!(plugin::start(self, &z, &player, a["login"] == true, &[])?)
            }
            "stop" => {
                let core = plugin::core(&self.plugin_ref(&a["plugin"]))?;
                plugin::stop(self, &core, a["fade"] == true)?;
                Value::Null
            }
            "end" => {
                let core = plugin::core(&self.plugin_ref(&a["plugin"]))?;
                plugin::end(self, &core)?;
                Value::Null
            }
            "unpause" => {
                let core = plugin::core(&self.plugin_ref(&a["plugin"]))?;
                plugin::unpause(self, &core)?;
                Value::Null
            }
            "submitEndPulse" => {
                let core = plugin::core(&self.plugin_ref(&a["plugin"]))?;
                let p = plugin::new_end_pulse(self, &core)?;
                region_activity::Host::submit_pulse(self, &p)?;
                Value::Null
            }
            "submitStartPulse" => {
                let core = plugin::core(&self.plugin_ref(&a["plugin"]))?;
                let p = core.start_pulse()?;
                region_activity::Host::submit_pulse(self, &p)?;
                Value::Null
            }
            "cutscene" => {
                let inner = Arc::new(ScriptInner {
                    script: a["script"].clone(),
                    lambdas: std::sync::Mutex::new(vec![]),
                });
                let c = cutscene::new(self, &player, Box::new(TestScript(inner.clone())))?;
                self.track(Tracked::Cutscene(c.clone()), "cutscene");
                self.scripts.push((c.clone(), inner));
                json!(self.cutscene_label(&c))
            }
            "startCutscene" => {
                let c = self.cutscene_ref(&a["cutscene"]);
                cutscene::start(
                    self,
                    &c,
                    a.get("hide").is_none_or(|h| h == &Value::Bool(true)),
                )?;
                Value::Null
            }
            "cutsceneCall" => {
                let c = self.cutscene_ref(&a["cutscene"]);
                let script = self
                    .scripts
                    .iter()
                    .find(|(x, _)| x.same(&c))
                    .unwrap()
                    .1
                    .clone();
                script.act(self, &c, &a["action"])?;
                Value::Null
            }
            "continue" => {
                let action = self.actions.last().cloned().ok_or(Error::Index)?;
                cutscene::run_continue(self, &action)?;
                Value::Null
            }
            "death" => {
                for _ in 0..self.hooks {
                    cutscene::death_hook(self, &player)?;
                }
                Value::Null
            }
            "logout" => match self.logout.get(a["key"].as_str().unwrap()).cloned() {
                None => json!(false),
                Some(c) => {
                    cutscene::logout_listener(self, &c, &player)?;
                    json!(true)
                }
            },
            "tick" => {
                for _ in 0..num(&a["count"]) {
                    self.tick()?;
                }
                json!(self.queue.len())
            }
            "npc" => {
                let l = loc(&a["location"]).unwrap();
                let npc = self.add_entity(
                    a["name"].as_str().unwrap().into(),
                    Kind::Npc,
                    Some(l.clone()),
                );
                self.place_npc(&npc, &l)?;
                Value::Null
            }
            "attr" => {
                let key = a["key"].as_str().unwrap().to_owned();
                let v = &a["value"];
                let value = match v {
                    Value::Null => None,
                    Value::Number(n) => Some(AttributeValue::Int(n.as_i64().unwrap() as i32)),
                    Value::String(s) => Some(AttributeValue::Text(s.clone())),
                    Value::Object(o) if o.contains_key("location") => {
                        Some(AttributeValue::Location(loc(&o["location"]).unwrap()))
                    }
                    other => panic!("attribute value {other}"),
                };
                let e = self.entity_mut(&player);
                match value {
                    None => {
                        e.attributes.remove(&key);
                    }
                    Some(v) => {
                        e.attributes.insert(key, v);
                    }
                }
                Value::Null
            }
            "place" => {
                self.entity_mut(&player).location = loc(&a["location"]);
                Value::Null
            }
            "active" => {
                self.entity_mut(&player).active = a["value"] == true;
                Value::Null
            }
            other => panic!("unknown operation {other}"),
        })
    }

    fn snapshot(&mut self) -> serde_json::Map<String, Value> {
        let mut plugins = vec![];
        let mut scenes = vec![];
        for (t, label) in &self.tracked {
            match t {
                Tracked::Plugin(z) => {
                    let core = plugin::core(z).unwrap();
                    let mut m = json!({
                        "label": label, "name": z.name(), "instanced": core.is_instanced(), "multicombat": core.is_multicombat(),
                        "safe": core.is_safe(), "restriction": z.restriction(), "zoneType": z.zone_type(), "fireRandoms": z.is_fire_randoms(),
                        "base": coords(core.base().as_ref()), "region": core.region().map(|r| r.read().id()),
                        "player": core.player().map(|p| self.name(&p)), "safeRespawn": coords(core.safe_respawn().as_ref()),
                    });
                    if core.cutscene.is_some() {
                        m["fade"] = json!(core.is_fade().unwrap());
                        m["npcs"] = json!(core.npcs().unwrap().len());
                    }
                    plugins.push(m);
                }
                Tracked::Cutscene(c) => {
                    let npcs: Vec<Value> = c
                        .added_npcs()
                        .iter()
                        .map(|(id, l)| {
                            json!([id, l.iter().map(|n| self.name(n)).collect::<Vec<_>>()])
                        })
                        .collect();
                    scenes.push(json!({
                        "label": label, "ended": c.ended(), "exit": coords(Some(&c.exit_location())),
                        "base": coords(c.base_field().as_ref()), "region": c.region_field().map(|r| r.read().id()), "npcs": npcs,
                    }));
                }
            }
        }
        let mut activities = BTreeMap::new();
        for (name, z) in self.manager.entries() {
            activities.insert(name.unwrap(), json!(self.zone_label(&z)));
        }
        let mut regions = vec![];
        let ids: std::collections::BTreeSet<i32> = self.scene.regions.keys().copied().collect();
        for id in ids {
            let r = self.scene.regions[&id].clone();
            let is_dynamic = dynamic::is_dynamic(&r);
            let zones = region_activity::region_zones::<Model>(&r).unwrap().values();
            if !is_dynamic && zones.is_empty() {
                continue;
            }
            let zones: Vec<Value> = zones
                .iter()
                .map(|z| {
                    json!([
                        z.zone().name(),
                        self.zone_label(z.zone()),
                        borders_value(Some(z.borders()))
                    ])
                })
                .collect();
            let borders = if is_dynamic {
                borders_value(dynamic::borders(&r).unwrap().as_ref())
            } else {
                Value::Null
            };
            let multi = is_dynamic && dynamic::is_multicombat(&r).unwrap();
            regions.push(json!([
                id,
                is_dynamic,
                zones,
                borders,
                multi,
                coords(Some(&dynamic::base_location(&r)))
            ]));
        }
        let reserved: Vec<Value> = self
            .scene
            .reserved_areas
            .values()
            .iter()
            .map(|b| borders_value(Some(b)))
            .collect();
        let queue: Vec<Value> = self
            .queue
            .clone()
            .iter()
            .map(|p| {
                let (class, delay, passed, running) = {
                    let r = p.read();
                    (r.class_name.clone(), r.delay, r.ticks_passed, r.running)
                };
                json!([class, self.owner_label(p), delay, passed, running])
            })
            .collect();
        let p = &self.entities[0];
        let attributes: serde_json::Map<String, Value> = p
            .attributes
            .iter()
            .map(|(k, v)| (k.clone(), self.value(v)))
            .collect();
        let player = json!({
            "location": coords(p.location.as_ref()), "attributes": attributes, "teleport": coords(p.teleport.as_ref()),
            "spawn": coords(p.spawn.as_ref()), "safe": p.safe, "safeRespawn": coords(p.safe_respawn.as_ref()),
            "logout": self.logout.keys().collect::<Vec<_>>(), "hooks": self.hooks, "actions": self.actions.len(),
        });
        let mut m = serde_json::Map::new();
        m.insert("plugins".into(), json!(plugins));
        m.insert("cutscenes".into(), json!(scenes));
        m.insert("activities".into(), json!(activities));
        m.insert("regions".into(), json!(regions));
        m.insert("reserved".into(), json!(reserved));
        m.insert("queue".into(), json!(queue));
        m.insert("player".into(), player);
        m.insert("events".into(), json!(self.events));
        m.insert("frames".into(), json!(self.frames));
        m
    }
}

static CACHE: OnceLock<Cache> = OnceLock::new();

pub fn evaluate(input: &Value, cache_text: &str) -> Value {
    let cache = CACHE.get_or_init(|| cache(cache_text));
    struct Log;
    impl defs::LogContext for Log {
        fn log(&mut self, _: &mut defs::Registry, s: String) -> defs::Result<()> {
            panic!("unexpected definition diagnostic: {s}")
        }
    }
    let mut definitions = defs::Registry::default();
    for (id, data) in &cache.definitions {
        let h = definitions
            .decode(&mut Log, *id, &mut Reader::new(data))
            .unwrap();
        definitions.definitions.insert(*id, h);
    }
    let mut model = Model {
        scene: Scene::new(MemoryAssets(cache), definitions, doors::Registry::default()),
        services: Bindings,
        state: State::default(),
        entities: vec![],
        by_node: HashMap::new(),
        events: vec![],
        frames: vec![],
        op_calls: 0,
        fail_at: -1,
        answers: input.get("answers").cloned().unwrap_or(Value::Null),
        answer_calls: HashMap::new(),
        queue: vec![],
        tracked: vec![],
        scripts: vec![],
        counters: HashMap::new(),
        actions: vec![],
        logout: BTreeMap::new(),
        hooks: 0,
        manager: Manager::new(),
        pulses: Pulses::new(),
        multiway: None,
        world: input["world"].clone(),
        npcs: 0,
        diagnostics: 0,
    };
    let p = &input["player"];
    model.add_entity(
        p["name"].as_str().unwrap().into(),
        Kind::Player,
        loc(&p["location"]),
    );
    let mut steps = vec![];
    for a in input["ops"].as_array().unwrap() {
        model.events.clear();
        model.frames.clear();
        model.op_calls = 0;
        model.fail_at = a.get("failAt").map_or(-1, num);
        let result = match model.act(a) {
            Ok(v) => json!({ "value": v }),
            Err(e) => json!({ "error": error_name(&e) }),
        };
        model.fail_at = -1;
        let diagnostics = model.scene.diagnostics.len();
        for _ in model.diagnostics..diagnostics {
            model.events.push(json!(["stackTrace"]));
        }
        model.diagnostics = diagnostics;
        model.pulses.sweep();
        let mut s = model.snapshot();
        s.insert("result".into(), result);
        steps.push(Value::Object(s));
    }
    Value::Array(steps)
}
