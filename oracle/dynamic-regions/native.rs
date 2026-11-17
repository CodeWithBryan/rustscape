//! Native replay of the dynamic-region corpus: the real native scene over the
//! exported cache, `world::dynamic_region` and `world::dynamic_scene`, with
//! recording stand-ins for the outside systems the oracle's adapters record
//! (entities, the zone monitor, NPC creation, familiars, ground items).
//! RegionManager.move and RegionChunk.addPlayer/removePlayer are reproduced
//! here as the adapters' callers; they are scaffolding, not the port.
use rs09_protocol::{
    buffer::Reader,
    game::{Direction as Wire, Frame},
};
use rs09_server::{
    definitions::{item::Shared, scenery as defs},
    inventory::item,
    pulses,
    world::{
        chunk::{self, HandleChunk},
        doors::{self, Location},
        dynamic_region as dynamic, dynamic_scene, ground_item,
        landscape::{Chunks, Mapscape},
        player_render,
        player_walk::{NodeHandle, State},
        rebuild, region, region_activity,
        scene::{self, Assets, Scene, Services},
        zones::{self, Borders, EntityKind, Monitor, Zone},
    },
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::OnceLock,
};

type Error = zones::Error;
type Result<T> = zones::Result<T>;

pub fn bytes(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn num(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}
fn loc(v: &Value) -> Location {
    Location::new(num(&v[0]), num(&v[1]), num(&v[2]))
}
fn coords(l: Option<&Location>) -> Value {
    l.map_or(Value::Null, |l| json!(l.coordinates()))
}
fn region_of(l: &Location) -> i32 {
    zones::region_id(l)
}

/// The exported cache: map/landscape bytes per region, definitions and keys.
/// Map group, landscape group, map bytes, landscape bytes.
pub type CacheRegion = (i32, i32, Option<Vec<u8>>, Option<Vec<u8>>);
pub struct Cache {
    pub regions: HashMap<i32, CacheRegion>,
    pub definitions: BTreeMap<i32, Vec<u8>>,
    pub keys: HashMap<i32, [i32; 4]>,
}
pub fn cache(text: &str) -> Cache {
    let v: Value = serde_json::from_str(text).unwrap();
    let mut regions = HashMap::new();
    for (id, r) in v["regions"].as_object().unwrap() {
        regions.insert(
            id.parse().unwrap(),
            (
                num(&r["mapGroup"]),
                num(&r["landGroup"]),
                r["map"].as_str().map(bytes),
                r["land"].as_str().map(bytes),
            ),
        );
    }
    let definitions = v["definitions"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(id, d)| (id.parse().unwrap(), bytes(d.as_str().unwrap())))
        .collect();
    let keys = v["keys"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(id, k)| {
            (
                id.parse().unwrap(),
                [num(&k[0]), num(&k[1]), num(&k[2]), num(&k[3])],
            )
        })
        .collect();
    Cache {
        regions,
        definitions,
        keys,
    }
}
pub struct MemoryAssets(pub &'static Cache);
impl Assets for MemoryAssets {
    fn archive_id(&self, name: &str) -> scene::Result<i32> {
        let (x, y) = name[1..].split_once('_').unwrap();
        let id = (x.parse::<i32>().unwrap() << 8) | y.parse::<i32>().unwrap();
        let r = self
            .0
            .regions
            .get(&id)
            .unwrap_or_else(|| panic!("unsupplied cache region {name}"));
        Ok(if name.starts_with('m') { r.0 } else { r.1 })
    }
    fn mapscape(&self, group: i32) -> scene::Result<Option<Vec<u8>>> {
        let r = self.0.regions.values().find(|r| r.0 == group).unwrap();
        Ok(r.2.clone())
    }
    fn landscape(&self, group: i32, _: [u32; 4]) -> scene::Result<Option<Vec<u8>>> {
        let r = self.0.regions.values().find(|r| r.1 == group).unwrap();
        Ok(r.3.clone())
    }
    fn keys(&self, _: i32) -> scene::Result<[u32; 4]> {
        Ok([0; 4])
    }
}

/// The scene's own services: nothing in this corpus reaches doors, paths,
/// audio or scenery pulses.
pub struct Bindings;
fn unreached<T>(what: &str) -> scene::Result<T> {
    Err(scene::Error(format!("{what} is outside this corpus")))
}
impl<A: Assets> Services<A> for Bindings {
    fn is_player(&self, _: usize) -> bool {
        false
    }
    fn location(&mut self, _: &mut Scene<A>, _: usize) -> scene::Result<Location> {
        unreached("location")
    }
    fn lock(&mut self, _: &mut Scene<A>, _: usize, _: i32) -> scene::Result<()> {
        unreached("lock")
    }
    fn audio(&mut self, _: &mut Scene<A>, _: usize, _: i32, _: Option<i32>) -> scene::Result<()> {
        unreached("audio")
    }
    fn message(&mut self, _: &mut Scene<A>, _: usize, _: &str) -> scene::Result<()> {
        unreached("message")
    }
    fn logout_listener(
        &mut self,
        _: &mut Scene<A>,
        _: usize,
        _: Option<Location>,
    ) -> scene::Result<()> {
        unreached("logout listener")
    }
    fn reset_path(&mut self, _: &mut Scene<A>, _: usize) -> scene::Result<()> {
        unreached("path")
    }
    fn add_path(&mut self, _: &mut Scene<A>, _: usize, _: i32, _: i32) -> scene::Result<()> {
        unreached("path")
    }
    fn move_near(&mut self, _: &mut Scene<A>, _: usize, _: Location) -> scene::Result<bool> {
        unreached("path")
    }
    fn diary(
        &mut self,
        _: &mut Scene<A>,
        _: usize,
        _: doors::Diary,
        _: i32,
        _: i32,
    ) -> scene::Result<()> {
        unreached("diary")
    }
    fn ground_create(&mut self, _: &mut Scene<A>, _: Option<usize>) -> scene::Result<()> {
        unreached("ground")
    }
    fn pulse(&mut self, _: &mut Scene<A>, _: &pulses::Handle) -> scene::Result<bool> {
        unreached("external pulse")
    }
    fn stop(&mut self, _: &mut Scene<A>, _: &pulses::Handle) -> scene::Result<()> {
        unreached("external stop")
    }
    fn now_millis(&mut self) -> i64 {
        0
    }
}

/// The zone host type of the region zone lists: nothing calls into it.
pub struct ZoneHost;
fn unused<T>() -> Result<T> {
    Err(Error::Binding("zone host is outside this corpus".into()))
}
impl zones::Host for ZoneHost {
    type Target = ();
    type Opaque = ();
    fn kind(&mut self, _: &NodeHandle) -> Result<EntityKind> {
        unused()
    }
    fn monitor(&mut self, _: &NodeHandle) -> Result<Monitor<Self>> {
        unused()
    }
    fn region(&mut self, _: i32) -> Result<region::Handle> {
        unused()
    }
    fn target_entity(&mut self, _: &()) -> Result<Option<NodeHandle>> {
        unused()
    }
    fn location(&mut self, _: &NodeHandle) -> Result<Option<Location>> {
        unused()
    }
    fn is_artificial(&mut self, _: &NodeHandle) -> Result<bool> {
        unused()
    }
    fn entity_update_location(&mut self, _: &NodeHandle, _: Option<&Location>) -> Result<()> {
        unused()
    }
    fn set_invisible(&mut self, _: &NodeHandle, _: bool) -> Result<()> {
        unused()
    }
    fn entity_continue_attack(
        &mut self,
        _: &NodeHandle,
        _: &NodeHandle,
        _: Option<&()>,
        _: bool,
    ) -> Result<bool> {
        unused()
    }
    fn skull_wilderness(&mut self, _: &NodeHandle) -> Result<bool> {
        unused()
    }
    fn skull_level(&mut self, _: &NodeHandle) -> Result<i32> {
        unused()
    }
    fn packet_message(&mut self, _: &NodeHandle, _: &str) -> Result<()> {
        unused()
    }
    fn player_message(&mut self, _: &NodeHandle, _: &str) -> Result<()> {
        unused()
    }
    fn has_timer(&mut self, _: &NodeHandle, _: &str) -> Result<bool> {
        unused()
    }
    fn teleport_locked(&mut self, _: &NodeHandle) -> Result<bool> {
        unused()
    }
    fn target_item_id(&mut self, _: Option<&()>) -> Result<i32> {
        unused()
    }
    fn now_millis(&mut self) -> Result<i64> {
        unused()
    }
    fn multi_zone(&mut self, _: &NodeHandle) -> Result<bool> {
        unused()
    }
    fn entity_ignore_multi(&mut self, _: &NodeHandle, _: &NodeHandle) -> Result<bool> {
        unused()
    }
    fn familiar_owner(&mut self, _: &NodeHandle) -> Result<Option<NodeHandle>> {
        unused()
    }
    fn combat_attacker(&mut self, _: &NodeHandle) -> Result<Option<NodeHandle>> {
        unused()
    }
    fn combat_time(&mut self, _: &NodeHandle) -> Result<Option<i64>> {
        unused()
    }
    fn active(&mut self, _: &NodeHandle) -> Result<bool> {
        unused()
    }
    fn id(&mut self, _: &NodeHandle) -> Result<i32> {
        unused()
    }
    fn music_playing(&mut self, _: &NodeHandle) -> Result<bool> {
        unused()
    }
    fn music_play_default(&mut self, _: &NodeHandle) -> Result<()> {
        unused()
    }
    fn music_unlock(&mut self, _: &NodeHandle, _: i32, _: Option<bool>) -> Result<()> {
        unused()
    }
    fn random_next_int(&mut self, _: i32) -> Result<i32> {
        unused()
    }
    fn fresh_random_next_int(&mut self, _: i32) -> Result<i32> {
        unused()
    }
    fn teleport_permitted(&mut self, _: &Location) -> Result<bool> {
        unused()
    }
}
/// The stand-in multiway zone object: toggleMulticombat compares identity.
struct Multiway;
impl zones::Behavior<ZoneHost> for Multiway {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Player,
    Npc,
    Familiar,
}
struct Entity {
    name: String,
    node: NodeHandle,
    kind: Kind,
    id: i32,
    location: Location,
    direction: Option<String>,
    aggressive: bool,
    respawn: bool,
    walks: bool,
    active: bool,
    owner: Option<String>,
    render: Option<player_render::Handle>,
}

/// Everything but the scene: what building an instance reaches.
pub struct Rest {
    state: State,
    entities: BTreeMap<String, Entity>,
    by_node: HashMap<usize, String>,
    events: Vec<Value>,
    failures: HashMap<String, i32>,
    copies: i32,
}
impl Rest {
    fn emit(&mut self, name: &str, args: Vec<Value>) {
        let mut e = vec![json!(name)];
        e.extend(args);
        self.events.push(Value::Array(e));
    }
    fn fail(&mut self, service: &str) -> Result<()> {
        if let Some(n) = self.failures.get_mut(service) {
            *n -= 1;
            if *n == 0 {
                return Err(Error::Binding(format!("injected {service}")));
            }
        }
        Ok(())
    }
    fn name(&self, node: &NodeHandle) -> String {
        self.by_node[&node.node].clone()
    }
    fn entity(&mut self, node: &NodeHandle) -> &mut Entity {
        let name = self.name(node);
        self.entities.get_mut(&name).unwrap()
    }
    fn add(&mut self, mut e: Entity) -> NodeHandle {
        let node = self.state.add_npc_at(-1, None, [1, 2, 3, 4]);
        let handle = self.state.retain_node(node).unwrap();
        e.node = handle.clone();
        self.by_node.insert(handle.node, e.name.clone());
        self.entities.insert(e.name.clone(), e);
        handle
    }
    fn placeholder(&mut self) -> NodeHandle {
        let node = self.state.add_npc_at(-1, None, [1, 2, 3, 4]);
        self.state.retain_node(node).unwrap()
    }
}
fn chunk_of<A: Assets>(scene: &mut Scene<A>, l: &Location) -> Result<HandleChunk> {
    let r = scene.region(region_of(l));
    let [x, y, z] = l.coordinates();
    let chunks = r.read().chunks.clone().ok_or(Error::Null)?;
    let c = chunks.read()[((x & 63) >> 3) as usize][((y & 63) >> 3) as usize][z as usize]
        .clone()
        .ok_or(Error::Null)?;
    Ok(c)
}
/// RegionManager.move for an NPC: off the source chunk, onto the
/// destination's, then the zone monitor at the current location.
fn move_npc<A: Assets>(
    scene: &mut Scene<A>,
    rest: &mut Rest,
    npc: &NodeHandle,
    src: Option<&Location>,
    dst: &Location,
) -> Result<()> {
    let dst_chunk = chunk_of(scene, dst)?;
    let src_chunk = src.map(|s| chunk_of(scene, s)).transpose()?;
    if !src_chunk.as_ref().is_some_and(|s| s.same(&dst_chunk)) {
        if let Some(s) = &src_chunk {
            s.read().npcs.remove(&Some(npc.clone()));
        }
        dst_chunk.read().npcs.add(Some(npc.clone()));
    }
    let (name, at) = {
        let e = rest.entity(npc);
        (e.name.clone(), e.location.clone())
    };
    rest.emit("zoneUpdate", vec![json!(name), coords(Some(&at))]);
    Ok(())
}

impl<A: Assets> dynamic::Build<A> for Rest {
    fn update_scene_graph(&mut self, _: &mut Scene<A>, _: &NodeHandle) -> Result<()> {
        Err(Error::Binding("a new instance never holds a player".into()))
    }
    fn npc_location(&mut self, _: &mut Scene<A>, npc: &NodeHandle) -> Result<Location> {
        Ok(self.entity(npc).location.clone())
    }
    fn copy_npc(
        &mut self,
        scene: &mut Scene<A>,
        npc: &NodeHandle,
        location: Location,
    ) -> Result<()> {
        let (id, direction, aggressive, respawn, walks) = {
            let e = self.entity(npc);
            (e.id, e.direction.clone(), e.aggressive, e.respawn, e.walks)
        };
        self.copies += 1;
        let name = format!("copy{}", self.copies);
        let dummy = self.placeholder();
        let copy = self.add(Entity {
            name: name.clone(),
            node: dummy,
            kind: Kind::Npc,
            id,
            location: location.clone(),
            direction: direction.clone(),
            aggressive: false,
            respawn: false,
            walks: false,
            active: true,
            owner: None,
            render: None,
        });
        self.emit(
            "npcCreate",
            vec![
                json!(name),
                json!(id),
                coords(Some(&location)),
                json!(direction),
            ],
        );
        self.fail("npcCreate")?;
        self.emit("npcAggressive", vec![json!(name), json!(aggressive)]);
        self.entity(&copy).aggressive = aggressive;
        self.emit("npcRespawn", vec![json!(name), json!(respawn)]);
        self.entity(&copy).respawn = respawn;
        self.emit("npcWalks", vec![json!(name), json!(walks)]);
        self.entity(&copy).walks = walks;
        self.emit("npcInit", vec![json!(name)]);
        self.fail("npcInit")?;
        move_npc(scene, self, &copy, None, &location)
    }
    fn log_error(&mut self, _: &mut Scene<A>, message: &str) -> Result<()> {
        self.emit("log", vec![json!("ERR"), json!(message)]);
        Ok(())
    }
}

pub struct Model {
    scene: Scene<MemoryAssets>,
    services: Bindings,
    rest: Rest,
    regions: BTreeMap<String, region::Handle>,
    borders: BTreeMap<String, Borders>,
    runner: pulses::Runner,
    known: Vec<region::Handle>,
    owners: Vec<(pulses::Handle, i32)>,
    frames: Vec<String>,
    items: Vec<(ground_item::Handle, String)>,
    multiway: Zone<ZoneHost>,
    diagnostics: usize,
}
macro_rules! world {
    ($m:expr) => {
        $m.scene.context(&mut $m.services)
    };
}
impl region::Context for Model {
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
    fn submit_pulse(&mut self, pulse: &pulses::Handle) -> Result<()> {
        self.know();
        self.runner
            .submit(Some(pulse.clone()))
            .map_err(|e| Error::Binding(format!("{e:?}")))
    }
    fn npc_active(&mut self, npc: &NodeHandle) -> Result<bool> {
        Ok(self.rest.entity(npc).active)
    }
    fn add_renderable_npc(&mut self, npc: &NodeHandle) -> Result<()> {
        let name = self.rest.name(npc);
        self.rest.emit("renderable", vec![json!(name)]);
        Ok(())
    }
    fn npc_region_inactivity(&mut self, npc: &NodeHandle) -> Result<()> {
        let name = self.rest.name(npc);
        self.rest.emit("regionInactivity", vec![json!(name)]);
        Ok(())
    }
    fn log_error(&mut self, message: &str) -> Result<()> {
        self.rest.emit("log", vec![json!("ERR"), json!(message)]);
        Ok(())
    }
    fn chunk_region(&mut self, region_id: i32) -> Result<region::Handle> {
        Ok(self.scene.region(region_id))
    }
}
impl dynamic::Host for Model {
    type Zones = ZoneHost;
    fn reserved_areas(&mut self) -> Result<dynamic::Reserved> {
        Ok(self.scene.reserved_areas.clone())
    }
    fn multiway_zone(&mut self) -> Result<Zone<ZoneHost>> {
        Ok(self.multiway.clone())
    }
    fn remove_region(&mut self, id: i32) -> Result<Option<region::Handle>> {
        Ok(self.scene.regions.remove(&id))
    }
    fn is_familiar(&mut self, npc: &NodeHandle) -> Result<bool> {
        Ok(self.rest.entity(npc).kind == Kind::Familiar)
    }
    fn recall_familiar(&mut self, npc: &NodeHandle) -> Result<()> {
        let (name, owner, at) = {
            let e = self.rest.entity(npc);
            (e.name.clone(), e.owner.clone().unwrap(), e.location.clone())
        };
        let owner_location = self.rest.entities[&owner].location.clone();
        self.rest
            .emit("setInvisible", vec![json!(name), json!(true)]);
        self.rest.fail("familiar")?;
        move_npc(
            &mut self.scene,
            &mut self.rest,
            npc,
            Some(&at),
            &owner_location,
        )?;
        self.rest.emit(
            "setLocation",
            vec![json!(name), coords(Some(&owner_location))],
        );
        self.rest.entity(npc).location = owner_location;
        self.rest.emit(
            "queueScript",
            vec![json!(name), json!(0), json!("STRONG"), json!(false)],
        );
        Ok(())
    }
    fn clear_npc(&mut self, npc: &NodeHandle) -> Result<()> {
        let name = self.rest.name(npc);
        self.rest.emit("npcClear", vec![json!(name)]);
        self.rest.fail("npcClear")
    }
    fn destroy_ground_item(&mut self, item: Option<&ground_item::Handle>) -> Result<()> {
        let label = item.and_then(|i| {
            self.items
                .iter()
                .find(|(h, _)| h.same(i))
                .map(|(_, l)| l.clone())
        });
        self.rest.emit("groundDestroy", vec![json!(label)]);
        Ok(())
    }
}
impl pulses::Context<()> for Model {
    fn pulse(&mut self, pulse: &pulses::Handle, _: &mut ()) -> pulses::Result<bool> {
        self.know();
        let region = self
            .known
            .iter()
            .find(|r| region_activity::activity_pulse(r).same(pulse))
            .cloned()
            .ok_or(pulses::Error::Null)?;
        dynamic::pulse_body(self, &region).map_err(|e| pulses::Error::Binding(format!("{e:?}")))
    }
    fn node_active(&mut self, _: pulses::Node, _: &mut ()) -> pulses::Result<bool> {
        Ok(true)
    }
    fn exception(&mut self, _: &pulses::Error, _: &mut ()) -> pulses::Result<()> {
        self.rest.emit("stackTrace", vec![]);
        Ok(())
    }
    fn remove_weak_scripts(&mut self, _: &mut ()) -> pulses::Result<()> {
        Ok(())
    }
    fn remove_normal_scripts(&mut self, _: &mut ()) -> pulses::Result<()> {
        Ok(())
    }
    fn is_dead(&mut self, _: &mut ()) -> pulses::Result<bool> {
        Ok(false)
    }
    fn send_message(&mut self, _: pulses::Node, _: &str, _: &mut ()) -> pulses::Result<()> {
        Ok(())
    }
    fn now_millis(&mut self, _: &mut ()) -> pulses::Result<i64> {
        Ok(0)
    }
    fn log(&mut self, _: pulses::Level, message: String, _: &mut ()) -> pulses::Result<()> {
        self.rest.emit("log", vec![json!("ERR"), json!(message)]);
        Ok(())
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

/// The keys and the session of the scene packets.
struct Session<'a> {
    keys: &'a HashMap<i32, [i32; 4]>,
    frames: &'a mut Vec<String>,
    failures: &'a mut HashMap<String, i32>,
}
impl rebuild::Context for Session<'_> {
    fn keys(&mut self, id: i32) -> player_render::Result<[i32; 4]> {
        Ok(self.keys.get(&id).copied().unwrap_or([0; 4]))
    }
    fn send(&mut self, _: &player_render::Handle, frame: Frame) -> player_render::Result<()> {
        if let Some(n) = self.failures.get_mut("session") {
            *n -= 1;
            if *n == 0 {
                return Err(player_render::Error::Binding("injected session".into()));
            }
        }
        write(self.frames, &frame);
        Ok(())
    }
}
fn write(frames: &mut Vec<String>, frame: &Frame) {
    let mut wire = vec![];
    frame.write(Wire::ServerToClient, None, &mut wire).unwrap();
    frames.push(hex(&wire));
}

impl Model {
    fn know(&mut self) {
        for r in self.scene.regions.values() {
            if !self.known.iter().any(|k| k.same(r)) {
                self.known.push(r.clone());
            }
        }
    }
    fn region(&mut self, v: &Value) -> region::Handle {
        match v {
            Value::String(name) => self.regions[name].clone(),
            other => self.scene.region(num(other)),
        }
    }
    fn chunk(&mut self, a: &Value) -> Result<HandleChunk> {
        let r = self.region(&a["region"]);
        let c = &a["chunk"];
        let chunks = r.read().chunks.clone().ok_or(Error::Null)?;
        let chunk = chunks.read()[num(&c[0]) as usize][num(&c[1]) as usize][num(&c[2]) as usize]
            .clone()
            .ok_or(Error::Null)?;
        Ok(chunk)
    }
    fn borders(&self, v: &Value) -> Borders {
        match v {
            Value::String(name) => self.borders[name].clone(),
            other => Borders::new(
                num(&other[0]),
                num(&other[1]),
                num(&other[2]),
                num(&other[3]),
            ),
        }
    }
    fn chunk_region(&mut self, chunk: &HandleChunk) -> region::Handle {
        let base = chunk.read().current_base.clone();
        self.scene.region(region_of(&base))
    }
    /// RegionChunk.addPlayer: on the list, then the chunk's region flagged
    /// active (Region.flagActive, not overridden).
    fn add_player(&mut self, chunk: &HandleChunk, player: &NodeHandle) -> Result<()> {
        chunk.read().players.add(Some(player.clone()));
        let region = self.chunk_region(chunk);
        dynamic::flag_active(self, &region)
    }
    /// RegionChunk.removePlayer: off the list, then checkInactive.
    fn remove_player(&mut self, chunk: &HandleChunk, player: &NodeHandle) -> Result<()> {
        chunk.read().players.remove(&Some(player.clone()));
        let region = self.chunk_region(chunk);
        dynamic::check_inactive(self, &region).map(|_| ())
    }
    /// RegionManager.move for a player.
    fn move_player(
        &mut self,
        player: &NodeHandle,
        src: Option<&Location>,
        dst: &Location,
    ) -> Result<()> {
        let dst_chunk = chunk_of(&mut self.scene, dst)?;
        let src_chunk = src.map(|s| chunk_of(&mut self.scene, s)).transpose()?;
        if !src_chunk.as_ref().is_some_and(|s| s.same(&dst_chunk)) {
            let same_region = match src {
                Some(s) => {
                    let a = self.scene.region(region_of(s));
                    let b = self.scene.region(region_of(dst));
                    a.same(&b)
                }
                None => false,
            };
            if let Some(s) = &src_chunk {
                self.remove_player(s, player)?;
            }
            let _ = same_region;
            self.add_player(&dst_chunk, player)?;
        }
        let (name, at) = {
            let e = self.rest.entity(player);
            (e.name.clone(), e.location.clone())
        };
        self.rest
            .emit("zoneUpdate", vec![json!(name), coords(Some(&at))]);
        Ok(())
    }
    fn instance_name(&mut self, a: &Value, region: &region::Handle) {
        if let Some(name) = a["as"].as_str() {
            self.regions.insert(name.into(), region.clone());
        }
    }
    fn player_render(&mut self, name: &str) -> player_render::Handle {
        self.rest.entities[name].render.clone().unwrap()
    }

    fn act(&mut self, a: &Value) -> Result<Value> {
        let op = a["op"].as_str().unwrap();
        Ok(match op {
            "create" => {
                let r = dynamic::create(&mut world!(self), &mut self.rest, num(&a["template"]))?;
                self.instance_name(a, &r);
                json!(r.read().id())
            }
            "createBetween" => {
                let r = dynamic::create_between(
                    &mut world!(self),
                    &mut self.rest,
                    num(&a["one"]),
                    num(&a["two"]),
                )?;
                self.instance_name(a, &r);
                json!(r.read().id())
            }
            "createArea" => {
                let b = self.borders(&a["borders"]);
                let all = dynamic::create_area(&mut world!(self), &mut self.rest, &b)?;
                for (r, name) in all.iter().zip(a["as"].as_array().unwrap()) {
                    self.regions
                        .insert(name.as_str().unwrap().into(), r.clone());
                }
                json!(all.iter().map(|r| r.read().id()).collect::<Vec<_>>())
            }
            "copy" => {
                let to = loc(&a["to"]);
                let r = dynamic::copy(&mut world!(self), &mut self.rest, num(&a["template"]), &to)?;
                self.instance_name(a, &r);
                json!(r.read().id())
            }
            "reserve" => {
                let s = &a["size"];
                let b = dynamic::reserve_area(&self.scene.reserved_areas, num(&s[0]), num(&s[1]))?;
                if let Some(name) = a["as"].as_str() {
                    self.borders.insert(name.into(), b.clone());
                }
                borders_value(Some(&b))
            }
            "find" => {
                let s = &a["size"];
                let b =
                    dynamic::find_zone_borders(&self.scene.reserved_areas, num(&s[0]), num(&s[1]))?;
                borders_value(Some(&b))
            }
            "withBorders" => {
                let b = self.borders(&a["borders"]);
                let r = dynamic::with_borders(&mut world!(self), b)?;
                self.instance_name(a, &r);
                json!(r.read().id())
            }
            "emptyInstance" => {
                let z = dynamic::reserve_area(&self.scene.reserved_areas, 8, 8)?;
                let r = dynamic::construct(
                    &mut world!(self),
                    -1,
                    z.south_west_x() >> 6,
                    z.south_west_y() >> 6,
                )?;
                dynamic::set_borders(&r, Some(z))?;
                r.write().build = a["build"].as_bool().unwrap();
                let id = r.read().id();
                dynamic::add_region(&mut self.scene, id, &r);
                self.instance_name(a, &r);
                json!(id)
            }
            "load" => {
                let r = self.region(&a["region"]);
                region::load(&mut world!(self), &r, Some(a["build"].as_bool().unwrap()))
                    .map_err(Error::from)?;
                Value::Null
            }
            "replace" => {
                let from = self.region(&a["from"]);
                let s = &a["source"];
                let source = from.read().chunks.clone().unwrap().read()[num(&s[0]) as usize]
                    [num(&s[1]) as usize][num(&s[2]) as usize]
                    .clone()
                    .ok_or(Error::Null)?;
                let target = self.region(&a["region"]);
                let mut w = world!(self);
                let copy =
                    chunk::copy(&mut w, &source).map_err(|e| Error::Binding(format!("{e:?}")))?;
                dynamic::replace_chunk(
                    &mut w,
                    &mut self.rest,
                    &target,
                    num(&a["z"]) as usize,
                    num(&a["x"]) as usize,
                    num(&a["y"]) as usize,
                    &copy,
                    &from,
                )?;
                Value::Null
            }
            "rotate" => {
                let c = self.chunk(a)?;
                let direction = match a["direction"].as_str().unwrap() {
                    "NORTH" => 0,
                    "EAST" => 1,
                    "SOUTH" => 2,
                    "WEST" => 3,
                    "NORTH_EAST" => 4,
                    "SOUTH_EAST" => 5,
                    "SOUTH_WEST" => 6,
                    "NORTH_WEST" => 7,
                    other => panic!("direction {other}"),
                };
                dynamic::rotate(&mut world!(self), &mut self.rest, &c, direction)?;
                Value::Null
            }
            "link" => {
                let r = self.region(&a["region"]);
                let linked: Vec<_> = a["linked"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| self.region(v))
                    .collect();
                if !dynamic::is_dynamic(&r) || linked.iter().any(|l| !dynamic::is_dynamic(l)) {
                    // The content's `(DynamicRegion)` casts.
                    return Err(Error::ClassCast);
                }
                dynamic::link(self, &r, &linked)?;
                Value::Null
            }
            "multicombat" => {
                let r = self.region(&a["region"]);
                dynamic::toggle_multicombat(self, &r)?;
                Value::Null
            }
            "music" => {
                let r = self.region(&a["region"]);
                dynamic::set_music_id(&r, num(&a["music"]))?;
                Value::Null
            }
            "permanent" => {
                let r = self.region(&a["region"]);
                dynamic::set_permanent(&r, a["value"].as_bool().unwrap())?;
                Value::Null
            }
            "npc" => {
                let l = loc(&a["location"]);
                let name = a["name"].as_str().unwrap().to_owned();
                let familiar = a["owner"].as_str().map(str::to_owned);
                let dummy = self.rest.placeholder();
                let npc = self.rest.add(Entity {
                    name: name.clone(),
                    node: dummy,
                    kind: if familiar.is_some() {
                        Kind::Familiar
                    } else {
                        Kind::Npc
                    },
                    id: num(&a["id"]),
                    location: l.clone(),
                    direction: a["direction"].as_str().map(str::to_owned),
                    aggressive: false,
                    respawn: false,
                    walks: false,
                    active: true,
                    owner: familiar,
                    render: None,
                });
                for (key, event) in [
                    ("aggressive", "npcAggressive"),
                    ("respawn", "npcRespawn"),
                    ("walks", "npcWalks"),
                ] {
                    if a[key].as_bool() == Some(true) {
                        self.rest.emit(event, vec![json!(name), json!(true)]);
                        let e = self.rest.entity(&npc);
                        match key {
                            "aggressive" => e.aggressive = true,
                            "respawn" => e.respawn = true,
                            _ => e.walks = true,
                        }
                    }
                }
                if let Some(active) = a["active"].as_bool() {
                    self.rest.entity(&npc).active = active;
                }
                move_npc(&mut self.scene, &mut self.rest, &npc, None, &l)?;
                Value::Null
            }
            "player" => {
                let l = loc(&a["location"]);
                let name = a["name"].as_str().unwrap().to_owned();
                let dummy = self.rest.placeholder();
                let render = Shared::new(player_render::Player::new(1, l.clone(), 2048));
                let p = self.rest.add(Entity {
                    name,
                    node: dummy,
                    kind: Kind::Player,
                    id: -1,
                    location: l.clone(),
                    direction: None,
                    aggressive: false,
                    respawn: false,
                    walks: false,
                    active: true,
                    owner: None,
                    render: Some(render),
                });
                self.move_player(&p, None, &l)?;
                Value::Null
            }
            "move" => {
                let name = a["name"].as_str().unwrap();
                let to = loc(&a["to"]);
                let (node, from) = {
                    let e = &self.rest.entities[name];
                    (e.node.clone(), e.location.clone())
                };
                if self.rest.entities[name].kind == Kind::Player {
                    self.move_player(&node, Some(&from), &to)?;
                } else {
                    move_npc(&mut self.scene, &mut self.rest, &node, Some(&from), &to)?;
                }
                let e = self.rest.entities.get_mut(name).unwrap();
                e.location = to.clone();
                if let Some(r) = &e.render {
                    r.write().location = Some(to);
                }
                Value::Null
            }
            "leave" => {
                let name = a["name"].as_str().unwrap();
                let (node, at) = {
                    let e = &self.rest.entities[name];
                    (e.node.clone(), e.location.clone())
                };
                let c = chunk_of(&mut self.scene, &at)?;
                self.remove_player(&c, &node)?;
                Value::Null
            }
            "view" => {
                let r = self.region(&a["region"]);
                let name = a["name"].as_str().unwrap();
                json!(if a["add"].as_bool().unwrap() {
                    region_activity::increment_view_amount(&r, name)
                } else {
                    region_activity::decrement_view_amount(&r, name)
                })
            }
            "checkInactive" => {
                let r = self.region(&a["region"]);
                json!(dynamic::check_inactive(self, &r)?)
            }
            "isInactive" => {
                let r = self.region(&a["region"]);
                json!(dynamic::is_inactive(self, &r, a["run"].as_bool().unwrap())?)
            }
            "flagActive" => {
                let r = self.region(&a["region"]);
                dynamic::flag_active(self, &r)?;
                Value::Null
            }
            "flagInactive" => {
                let r = self.region(&a["region"]);
                let force = a["force"].as_bool().unwrap_or(false);
                json!(dynamic::flag_inactive(self, &r, force)?)
            }
            "unload" => {
                let r = self.region(&a["region"]);
                json!(dynamic::unload(self, &r, a["force"].as_bool().unwrap())?)
            }
            "timeout" => {
                let r = self.region(&a["region"]);
                region_activity::set_region_time_out(&r, num(&a["ticks"]));
                Value::Null
            }
            "tick" => {
                let runner = self.runner.clone();
                for _ in 0..num(&a["count"]) {
                    runner
                        .update_all(&mut (), self)
                        .map_err(|e| Error::Binding(format!("{e:?}")))?;
                }
                json!(runner.current().len())
            }
            "removeRegion" => {
                let r = self.region(&a["region"]);
                let id = r.read().id();
                dynamic::remove_region(self, id)?;
                Value::Null
            }
            "scene" => {
                let p = self.player_render(a["name"].as_str().unwrap());
                let at = p.read().location.clone().unwrap();
                let packet = if dynamic::is_dynamic(&self.scene.region(region_of(&at))) {
                    "BuildDynamicScene"
                } else {
                    "UpdateSceneGraph"
                };
                let mut session = Session {
                    keys: &CACHE.get().unwrap().keys,
                    frames: &mut self.frames,
                    failures: &mut self.rest.failures,
                };
                let written = dynamic_scene::update_scene_graph(
                    &mut self.scene,
                    &mut session,
                    &p,
                    a["login"].as_bool().unwrap_or(false),
                );
                match written {
                    Ok(()) => {}
                    // PacketRepository.send catches and logs a failing write.
                    Err(player_render::Error::Binding(m)) if m.starts_with("injected ") => {
                        self.rest.emit("packetError", vec![json!(packet)]);
                    }
                    Err(e) => return Err(Error::Binding(format!("{e:?}"))),
                }
                Value::Null
            }
            "minimap" => {
                write(
                    &mut self.frames,
                    &dynamic_scene::minimap_state(num(&a["state"])),
                );
                Value::Null
            }
            "clearFlag" => {
                write(&mut self.frames, &dynamic_scene::clear_minimap_flag());
                Value::Null
            }
            "track" => {
                let r = self.region(&a["region"]);
                let node = self.rest.entities[a["name"].as_str().unwrap()].node.clone();
                dynamic::npcs(&r)?.add(node);
                Value::Null
            }
            "clear" => {
                let r = self.region(&a["region"]);
                dynamic::clear(self, &r)?;
                Value::Null
            }
            "item" => {
                let c = self.chunk(a)?;
                let handle = Shared::new(ground_item::GroundItem {
                    item: Shared::new(item::Item::new(995, 1, 0, &Default::default())),
                    interaction: None,
                    node_size: 1,
                    location: None,
                    dropper: None,
                    dropper_uid: -1,
                    ticks: 0,
                    decay_time: 0,
                    remain_private: false,
                    removed: false,
                    force_visible: false,
                });
                self.items
                    .push((handle.clone(), a["label"].as_str().unwrap().into()));
                rs09_server::world::chunk_sync::get_items(&c)
                    .write()
                    .push(Some(handle));
                Value::Null
            }
            "nullNpc" => {
                let c = self.chunk(a)?;
                c.read().npcs.add(None);
                Value::Null
            }
            "object" => {
                let l = loc(&a["location"]);
                let o = world!(self)
                    .object(l, -1, -1)
                    .map_err(|e| Error::Binding(e.0))?;
                o.map_or(Value::Null, |o| json!(scenery(&o)))
            }
            "clip" => {
                let l = loc(&a["location"]);
                let mut w = world!(self);
                let walk = w
                    .clipping(l.clone(), false)
                    .map_err(|e| Error::Binding(e.0))?;
                let projectile = w
                    .clipping(l.clone(), true)
                    .map_err(|e| Error::Binding(e.0))?;
                let land = w.is_landscape(Some(&l)).map_err(|e| Error::Binding(e.0))?;
                json!([walk, projectile, land])
            }
            "fail" => {
                self.rest
                    .failures
                    .insert(a["service"].as_str().unwrap().into(), num(&a["at"]));
                Value::Null
            }
            other => panic!("unknown action {other}"),
        })
    }
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
fn scenery(o: &doors::Handle) -> String {
    let o = o.read();
    let [x, y, z] = o.location.coordinates();
    format!(
        "{},{},{},{x},{y},{z},{},{},{},{}",
        o.id, o.shape, o.rotation, o.active, o.renderable, o.constructed, o.charge
    )
}
fn landscape(f: &rs09_server::world::clipping::ChunkFlags) -> String {
    let Some(rows) = &f.landscape else {
        return "null".into();
    };
    let mut s = String::new();
    for x in 0..8 {
        let row = rows.read()[x].clone().unwrap();
        for y in 0..8 {
            s.push(if row.read()[y] { '1' } else { '0' });
        }
    }
    s
}
fn sha(text: &str) -> String {
    hex(&Sha256::digest(text.as_bytes()))
}
fn error_name(e: &Error) -> String {
    match e {
        Error::Null => "Null".into(),
        Error::ClassCast => "ClassCast".into(),
        Error::ConcurrentModification => "ConcurrentModification".into(),
        Error::IllegalState(_) => "IllegalState".into(),
        Error::Binding(m) if m.starts_with("injected ") => "Failure".into(),
        Error::Binding(m) if m.contains("ArrayIndexOutOfBounds") => "Index".into(),
        other => panic!("unexpected native error {other:?}"),
    }
}

impl Model {
    fn names(&self, list: &[Option<NodeHandle>]) -> String {
        let names: Vec<_> = list
            .iter()
            .map(|n| n.as_ref().map_or("null".into(), |n| self.rest.name(n)))
            .collect();
        format!("[{}]", names.join(","))
    }
    fn describe(&self, storage: &mut String, chunks: &mut String, id: i32, r: &region::Handle) {
        use std::fmt::Write;
        let all = r.read().chunks.clone().unwrap();
        for cx in 0..8 {
            for cy in 0..8 {
                for z in 0..4 {
                    let Some(c) = all.read()[cx][cy][z].clone() else {
                        writeln!(chunks, "C {id} {cx} {cy} {z} null").unwrap();
                        continue;
                    };
                    let c = c.read();
                    let [bx, by, bz] = c.base.coordinates();
                    let [ux, uy, uz] = c.current_base.coordinates();
                    let items = c
                        .items
                        .as_ref()
                        .map_or("null".into(), |i| i.read().len().to_string());
                    writeln!(
                        chunks,
                        "C {id} {cx} {cy} {z} {bx} {by} {bz} {ux} {uy} {uz} {} {} {} {items} {} {} {}",
                        c.rotation,
                        self.names(&c.players.values()),
                        self.names(&c.npcs.values()),
                        c.update_flags.read().len(),
                        landscape(&c.flags),
                        landscape(&c.projectile_flags)
                    )
                    .unwrap();
                    for (stat, objects) in [&c.static_objects, &c.objects].into_iter().enumerate() {
                        let objects = objects.read();
                        for (x, column) in objects.iter().enumerate() {
                            for (y, slots) in column.iter().enumerate() {
                                for (s, slot) in slots.iter().enumerate() {
                                    if let Some(o) = slot {
                                        writeln!(
                                            storage,
                                            "O {id} {cx} {cy} {z} {stat} {x} {y} {s} {}",
                                            scenery(o)
                                        )
                                        .unwrap();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    fn snapshot(&mut self, label: &str) -> Value {
        let mut ids: Vec<_> = self.scene.regions.keys().copied().collect();
        ids.sort();
        let mut regions = vec![];
        let mut instances = vec![];
        let (mut storage, mut chunks) = (String::new(), String::new());
        for id in ids {
            let r = self.scene.regions[&id].clone();
            let dynamic = dynamic::is_dynamic(&r);
            {
                let g = r.read();
                let d = g.data.read();
                regions.push(json!([
                    id,
                    dynamic,
                    dynamic::region_id(&r),
                    d.x,
                    d.y,
                    g.loaded,
                    g.build,
                    g.has_flags,
                    d.object_count,
                    region_activity::is_active(&r),
                    region_activity::viewing_players(&r)
                ]));
            }
            self.describe(&mut storage, &mut chunks, id, &r);
            if dynamic {
                let zones: Vec<_> = region_activity::region_zones::<ZoneHost>(&r)
                    .unwrap()
                    .values()
                    .iter()
                    .map(|z| json!([z.zone().name(), borders_value(Some(z.borders()))]))
                    .collect();
                let music: Vec<_> = r
                    .read()
                    .activity
                    .read()
                    .music_zones
                    .values()
                    .iter()
                    .map(|m| json!([m.music_id(), borders_value(Some(m.borders()))]))
                    .collect();
                let npcs: Vec<_> = dynamic::npcs(&r)
                    .unwrap()
                    .values()
                    .iter()
                    .map(|n| json!(self.rest.name(n)))
                    .collect();
                let linked = dynamic::linked(&r)
                    .unwrap()
                    .map(|l| json!(l.iter().map(|r| r.read().id()).collect::<Vec<_>>()));
                instances.push(json!([
                    id,
                    borders_value(dynamic::borders(&r).unwrap().as_ref()),
                    dynamic::is_multicombat(&r).unwrap(),
                    dynamic::is_permanent(&r).unwrap(),
                    dynamic::parent(&r).unwrap().map(|p| p.read().id()),
                    linked,
                    zones,
                    music,
                    npcs
                ]));
            }
        }
        let mut hash = Sha256::new();
        let keys: BTreeSet<_> = self.scene.flags.keys().map(|(k, _)| *k).collect();
        for k in keys {
            for p in [false, true] {
                if let Some(flags) = self.scene.flags.get(&(k, p)) {
                    hash.update(k.to_be_bytes());
                    hash.update([u8::from(p)]);
                    for i in *flags.read() {
                        hash.update(i.to_be_bytes());
                    }
                }
            }
        }
        let reserved: Vec<_> = self
            .scene
            .reserved_areas
            .values()
            .iter()
            .map(|b| borders_value(Some(b)))
            .collect();
        let queue: Vec<_> = self
            .runner
            .current()
            .iter()
            .map(|p| {
                let owner = self
                    .owners
                    .iter()
                    .find(|(h, _)| h.same(p))
                    .map(|(_, id)| *id);
                let p = p.read();
                json!([owner, p.delay, p.ticks_passed, p.running])
            })
            .collect();
        let players: Vec<_> = self
            .rest
            .entities
            .values()
            .filter(|e| e.kind == Kind::Player)
            .map(|e| {
                let last = e.render.as_ref().unwrap().read().last_scene.clone();
                json!([e.name, coords(Some(&e.location)), coords(last.as_ref())])
            })
            .collect();
        if let Ok(dump) = std::env::var("DYNAMIC_REGIONS_NATIVE_DUMP") {
            std::fs::write(
                std::path::Path::new(&dump).join(format!("{label}.txt")),
                format!("{chunks}{storage}"),
            )
            .unwrap();
        }
        json!({
            "regions": regions,
            "instances": instances,
            "storage": sha(&storage),
            "objects": storage.lines().count(),
            "chunks": sha(&chunks),
            "flags": hex(&hash.finalize()),
            "flagKeys": self.scene.flags.len(),
            "reserved": reserved,
            "queue": queue,
            "players": players,
            "events": self.rest.events.clone(),
            "frames": self.frames.clone(),
        })
    }
}

static CACHE: OnceLock<Cache> = OnceLock::new();

pub fn evaluate(input: &Value, cache_text: &str, index: usize) -> Value {
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
        rest: Rest {
            state: State::default(),
            entities: BTreeMap::new(),
            by_node: HashMap::new(),
            events: vec![],
            failures: HashMap::new(),
            copies: 0,
        },
        regions: BTreeMap::new(),
        borders: BTreeMap::new(),
        runner: pulses::Runner::default(),
        known: vec![],
        owners: vec![],
        frames: vec![],
        items: vec![],
        multiway: Zone::new(Some("MultiwayCombatZone"), true, &[], Multiway),
        diagnostics: 0,
    };
    let mut steps = vec![];
    for (step, a) in input["actions"].as_array().unwrap().iter().enumerate() {
        model.rest.events.clear();
        model.frames.clear();
        let result = match model.act(a) {
            Ok(v) => json!({ "value": v }),
            Err(e) => json!({ "error": error_name(&e) }),
        };
        let diagnostics = model.scene.diagnostics.len();
        for _ in model.diagnostics..diagnostics {
            model.rest.emit("stackTrace", vec![]);
        }
        model.diagnostics = diagnostics;
        model.know();
        for p in model.runner.current() {
            if !model.owners.iter().any(|(h, _)| h.same(&p)) {
                if let Some(r) = model
                    .scene
                    .regions
                    .values()
                    .find(|r| region_activity::activity_pulse(r).same(&p))
                {
                    let id = r.read().id();
                    model.owners.push((p.clone(), id));
                }
            }
        }
        let mut snapshot = model.snapshot(&format!("{index}-{step}"));
        snapshot["result"] = result;
        steps.push(snapshot);
    }
    json!({ "steps": steps })
}
