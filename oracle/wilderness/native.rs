//! Native replay of the wilderness and dark zone corpus. The case's inputs
//! become the required services of `zones::Host`, `zone_impls::Host`,
//! `dark_zone::Host` and `wilderness_zone::Host`; every service records its
//! call. The zones, monitors, regions, the darkness overlay's open/close
//! bodies, the insect pulse and the pulse runner are the real native ones.
//! The interface manager here keeps one overlay field and calls the
//! components' close/open in the order the oracle's adapter does.
use rs09_game_core::random::JavaRandom;
use rs09_server::{
    definitions::item::Shared,
    pulses,
    world::{
        dark_zone::{
            self, Component, DarkZone, GlowScript, Hitsplat, InsectPulse, LightSource,
            QueueStrength, TinderboxTask,
        },
        doors::Location,
        player_walk::{NodeHandle, State},
        region, region_activity as activity,
        wilderness_zone::{self, WildernessZone},
        zone_impls::{
            self, BankZone, KaramjaZone, ModeratorZone, MultiwayCombatZone, Rights, TeleportType,
        },
        zones::{self, Behavior, Borders, EntityKind, Error, Monitor, RegionZone, Result, Zone},
    },
};
use serde_json::{Map, Value, json};
use std::{
    any::Any,
    collections::{BTreeMap, BTreeSet, HashMap},
};

fn num(value: &Value) -> i32 {
    value.as_i64().unwrap() as i32
}
fn place(value: &Value) -> Option<Location> {
    value
        .as_array()
        .map(|a| Location::new(num(&a[0]), num(&a[1]), num(&a[2])))
}
fn coord(location: Option<&Location>) -> Value {
    location.map_or(Value::Null, |l| json!(l.coordinates()))
}

#[derive(Clone)]
pub enum Target {
    Entity(NodeHandle),
    Item(i32),
    Other,
}

struct Row {
    handle: NodeHandle,
    kind: EntityKind,
    state: Map<String, Value>,
    monitor: Monitor<Model>,
    overlay: Option<Component>,
    extension: Option<InsectPulse>,
}
pub struct Model {
    input: Value,
    _state: State,
    nodes: BTreeMap<i32, Row>,
    zones: Vec<(String, Zone<Model>)>,
    wilderness: Zone<Model>,
    multiway: Zone<Model>,
    regions: BTreeMap<i32, region::Handle>,
    events: Vec<Value>,
    calls: HashMap<String, i64>,
    fired: BTreeSet<usize>,
    runner: pulses::Runner,
    random: JavaRandom,
    insects: Vec<InsectPulse>,
    scripts: Vec<GlowScript>,
    tasks: Vec<TinderboxTask>,
}

/// MapZone with nothing overridden.
struct Plain;
impl Behavior<Model> for Plain {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn pulse_error(error: Error) -> pulses::Error {
    pulses::Error::Binding(name(&error))
}
fn name(error: &Error) -> String {
    match error {
        Error::Null => "Null".into(),
        Error::ClassCast => "ClassCast".into(),
        Error::ConcurrentModification => "ConcurrentModification".into(),
        Error::NoSuchElement => "NoSuchElementException".into(),
        Error::IllegalState(_) => "IllegalState".into(),
        Error::IllegalArgument => "IllegalArgument".into(),
        Error::Arithmetic => "Arithmetic".into(),
        Error::Limit => "Limit".into(),
        Error::Dynamic => "Dynamic".into(),
        Error::Binding(text) => text.clone(),
    }
}
fn light_constant(source: LightSource) -> &'static str {
    match source {
        LightSource::Candle => "CANDLE",
        LightSource::BlackCandle => "BLACK_CANDLE",
        LightSource::Torch => "TORCH",
        LightSource::CandleLantern => "CANDLE_LANTERN",
        LightSource::OilLamp => "OIL_LAMP",
        LightSource::OilLantern => "OIL_LANTERN",
        LightSource::BullseyeLantern => "BULLSEYE_LANTERN",
        LightSource::SapphireLantern => "SAPPHIRE_LANTERN",
        LightSource::EmeraldLantern => "EMERALD_LANTERN",
        LightSource::MiningHelmet => "MINING_HELMET",
    }
}
fn light(source: Option<LightSource>) -> Value {
    source.map_or(Value::Null, |s| json!(light_constant(s)))
}
fn component_label(component: Option<Component>) -> Value {
    match component {
        None => Value::Null,
        Some(Component::Darkness) => json!("darkness"),
        Some(Component::Id(id)) => json!(id),
    }
}

impl Model {
    fn slot_of(&self, handle: &NodeHandle) -> i32 {
        *self
            .nodes
            .iter()
            .find(|(_, row)| row.handle == *handle)
            .expect("a node of the case")
            .0
    }
    fn slot(&self, handle: Option<&NodeHandle>) -> Value {
        handle.map_or(Value::Null, |h| json!(self.slot_of(h)))
    }
    fn row(&mut self, handle: &NodeHandle) -> &mut Row {
        let id = self.slot_of(handle);
        self.nodes.get_mut(&id).unwrap()
    }
    fn handle(&self, value: &Value) -> Option<NodeHandle> {
        value
            .as_i64()
            .and_then(|id| self.nodes.get(&(id as i32)))
            .map(|row| row.handle.clone())
    }
    fn node(&self, value: &Value) -> Result<NodeHandle> {
        self.handle(value).ok_or(Error::Null)
    }
    fn player(&self, value: &Value) -> Result<NodeHandle> {
        let node = self.node(value)?;
        if self.nodes[&self.slot_of(&node)].kind != EntityKind::Player {
            return Err(Error::ClassCast);
        }
        Ok(node)
    }
    fn target(&self, value: &Value) -> Option<Target> {
        let map = value.as_object()?;
        if let Some(id) = map.get("node") {
            return self.handle(id).map(Target::Entity);
        }
        if let Some(id) = map.get("item") {
            return Some(Target::Item(num(id)));
        }
        Some(Target::Other)
    }
    fn target_label(&self, target: Option<&Target>) -> Value {
        match target {
            None => Value::Null,
            Some(Target::Entity(handle)) => json!(["node", self.slot_of(handle)]),
            Some(Target::Item(id)) => json!(["item", id]),
            Some(Target::Other) => json!(["other"]),
        }
    }
    fn attr(&self, handle: &NodeHandle, key: &str) -> Value {
        self.nodes[&self.slot_of(handle)]
            .state
            .get(key)
            .cloned()
            .unwrap_or(Value::Null)
    }
    fn has(&self, handle: &NodeHandle, key: &str) -> bool {
        self.nodes[&self.slot_of(handle)].state.contains_key(key)
    }
    fn flag(&self, handle: &NodeHandle, key: &str, fallback: bool) -> bool {
        self.attr(handle, key).as_bool().unwrap_or(fallback)
    }
    fn int(&self, handle: &NodeHandle, key: &str, fallback: i32) -> i32 {
        self.attr(handle, key)
            .as_i64()
            .map_or(fallback, |v| v as i32)
    }
    fn put(&mut self, handle: &NodeHandle, key: &str, value: Value) {
        self.row(handle).state.insert(key.into(), value);
    }
    fn emit(&mut self, event: &str, args: Vec<Value>) -> Result<()> {
        let mut row = vec![json!(event)];
        row.extend(args);
        self.events.push(Value::Array(row));
        let count = {
            let count = self.calls.entry(event.into()).or_default();
            *count += 1;
            *count
        };
        let hooks = self.input["hooks"].as_array().cloned().unwrap_or_default();
        for (index, hook) in hooks.iter().enumerate() {
            if hook["event"] == event
                && hook["at"].as_i64().unwrap_or(1) == count
                && self.fired.insert(index)
            {
                for action in hook["actions"].as_array().unwrap() {
                    self.act(action)?;
                }
            }
        }
        if self.input["failures"][event].as_i64() == Some(count) {
            return Err(Error::Binding("Failure".into()));
        }
        Ok(())
    }
    fn answer(&self, key: &str) -> Option<bool> {
        let answers = self.input["answers"][key].as_array()?;
        let index = (self.calls[key] as usize - 1).min(answers.len() - 1);
        answers[index].as_bool()
    }
    fn item_name_of(&self, id: i32) -> Option<String> {
        let key = id.to_string();
        match self.input["names"]
            .as_object()
            .and_then(|names| names.get(&key))
        {
            Some(name) => name.as_str().map(str::to_owned),
            None => Some(format!("Item {id}")),
        }
    }
    fn raw_region(&mut self, id: i32) -> region::Handle {
        self.regions
            .entry(id)
            .or_insert_with(|| Shared::new(region::Region::new((id >> 8) & 0xFF, id & 0xFF)))
            .clone()
    }
    fn zone(&self, value: &Value) -> Result<Zone<Model>> {
        let id = value.as_str().ok_or(Error::Null)?;
        self.zones
            .iter()
            .find(|(name, _)| name == id)
            .map(|(_, zone)| zone.clone())
            .ok_or(Error::Null)
    }
    fn zone_label(&self, zone: &Zone<Model>) -> Value {
        if let Some((id, _)) = self
            .zones
            .iter()
            .find(|(_, candidate)| candidate.same(zone))
        {
            return json!(id);
        }
        let class = if zone.behavior::<WildernessZone>().is_some() {
            "WildernessZone"
        } else if zone.behavior::<DarkZone>().is_some() {
            "DarkZone"
        } else if zone.behavior::<MultiwayCombatZone>().is_some() {
            "MultiwayCombatZone"
        } else if zone.behavior::<ModeratorZone>().is_some() {
            "ModeratorZone"
        } else if zone.behavior::<KaramjaZone>().is_some() {
            "KaramjaZone"
        } else if zone.behavior::<BankZone>().is_some() {
            "BankZone"
        } else {
            ""
        };
        json!(class)
    }
    fn pulse_label(&mut self, pulse: &InsectPulse) -> Value {
        let index = match self.insects.iter().position(|p| p.same(pulse)) {
            Some(index) => index,
            None => {
                self.insects.push(pulse.clone());
                self.insects.len() - 1
            }
        };
        json!(format!("pulse{index}"))
    }
    fn describe_borders(borders: &Borders) -> Value {
        json!([
            borders.south_west_x(),
            borders.south_west_y(),
            borders.north_east_x(),
            borders.north_east_y(),
            borders.plane()
        ])
    }
    fn describe(&self, list: &zones::List<RegionZone<Model>>) -> Value {
        list.values()
            .iter()
            .map(|z| {
                json!([
                    self.zone_label(z.zone()),
                    Self::describe_borders(z.borders())
                ])
            })
            .collect()
    }
    fn new_borders(value: &Value) -> Borders {
        let v: Vec<i32> = value.as_array().unwrap().iter().map(num).collect();
        if v.len() == 5 {
            Borders::with_plane(v[0], v[1], v[2], v[3], v[4])
        } else {
            Borders::new(v[0], v[1], v[2], v[3])
        }
    }
    fn instance(&self) -> &WildernessZone {
        self.wilderness.behavior::<WildernessZone>().unwrap()
    }
    fn component(spec: &Value) -> Component {
        if spec == "darkness" {
            Component::Darkness
        } else {
            Component::Id(num(spec))
        }
    }
    /// The component's own open: DARKNESS_OVERLAY's override or the base.
    fn open_component(&mut self, player: &NodeHandle, component: Component) -> Result<()> {
        match component {
            Component::Darkness => dark_zone::darkness_open(self, player),
            other => dark_zone::Host::component_open(self, player, other),
        }
    }
    /// The component's own close.
    fn close_own(&mut self, player: &NodeHandle, component: Component) -> Result<bool> {
        match component {
            Component::Darkness => dark_zone::darkness_close(self, player),
            other => dark_zone::Host::component_close(self, player, other),
        }
    }
    fn hook(&mut self, z: &Zone<Model>, a: &Value) -> Result<Value> {
        let n = self.node(&a["node"]);
        let flag = |key: &str| a[key].as_bool().unwrap_or(false);
        Ok(match a["hook"].as_str().unwrap() {
            "enter" => json!(z.enter(self, &n?)?),
            "leave" => json!(z.leave(self, &n?, flag("logout"))?),
            "death" => json!(z.death(self, &n?, self.handle(&a["killer"]).as_ref())?),
            "interact" => json!(z.interact(
                self,
                &n?,
                self.target(&a["target"]).as_ref(),
                (!a["option"].is_null()).then_some(&a["option"])
            )?),
            "teleport" => json!(z.teleport(
                self,
                &n?,
                a["type"].as_i64().unwrap_or(0) as i32,
                self.target(&a["target"]).as_ref()
            )?),
            "attack" => json!(z.continue_attack(
                self,
                &n?,
                self.target(&a["target"]).as_ref(),
                (!a["style"].is_null()).then_some(&a["style"]),
                flag("message")
            )?),
            "locationUpdate" => {
                z.location_update(self, &n?, place(&a["last"]).as_ref())?;
                Value::Null
            }
            other => return Err(Error::Binding(format!("unknown hook {other}"))),
        })
    }
    fn act(&mut self, a: &Value) -> Result<Value> {
        let op = a["op"].as_str().unwrap();
        let n = self.node(&a["node"]);
        let m = n
            .clone()
            .map(|handle| self.nodes[&self.slot_of(&handle)].monitor.clone());
        let z = self.zone(&a["zone"]);
        let flag = |key: &str| a[key].as_bool().unwrap_or(false);
        let style = (!a["style"].is_null()).then_some(&a["style"]);
        Ok(match op {
            "set" => {
                let n = n?;
                for (key, value) in a.as_object().unwrap() {
                    if key != "op" && key != "node" {
                        self.put(&n, key, value.clone());
                    }
                }
                Value::Null
            }
            "configure" => {
                z?.configure(self)?;
                Value::Null
            }
            "startup" => {
                let (wilderness, multiway) = (self.wilderness.clone(), self.multiway.clone());
                wilderness_zone::startup(self, &wilderness, &multiway)?;
                Value::Null
            }
            "register" => {
                z?.register(self, &Self::new_borders(&a["borders"]))?;
                Value::Null
            }
            "zoneInfo" => {
                let z = z?;
                json!([
                    z.name(),
                    z.uid()?,
                    z.is_overlappable(),
                    z.is_fire_randoms(),
                    z.restriction(),
                    z.zone_type(),
                    z.hash_code()
                ])
            }
            "hook" => self.hook(&z?, a)?,
            "update" => json!(m?.update_location(self, place(&a["last"]).as_ref())?),
            "teleport" => {
                json!(m?.teleport(self, num(&a["type"]), self.target(&a["target"]).as_ref())?)
            }
            "continueAttack" => json!(m?.continue_attack(
                self,
                self.target(&a["target"]).as_ref(),
                style,
                flag("message")
            )?),
            "interact" => json!(m?.interact(
                self,
                self.target(&a["target"]).as_ref(),
                (!a["option"].is_null()).then_some(&a["option"])
            )?),
            "death" => json!(m?.handle_death(self, self.handle(&a["killer"]).as_ref())?),
            "clear" => json!(m?.clear(self)?),
            "wildLevel" => json!(wilderness_zone::get_wilderness(self, &n?)?),
            "inZone" => {
                let wilderness = self.wilderness.clone();
                let instance = wilderness.behavior::<WildernessZone>().unwrap();
                json!(instance.is_in_zone(self, &n?)?)
            }
            "inZoneLocation" => {
                json!(
                    self.instance()
                        .is_in_zone_location(place(&a["location"]).as_ref())?
                )
            }
            "checkTeleport" => {
                let p = self.player(&a["node"])?;
                json!(wilderness_zone::check_teleport(self, &p, num(&a["level"]))?)
            }
            "show" => {
                let p = self.player(&a["node"])?;
                wilderness_zone::show(self, &p)?;
                Value::Null
            }
            "leavePlayer" => {
                if z?.behavior::<WildernessZone>().is_none() {
                    return Err(Error::ClassCast);
                }
                let p = self.player(&a["node"])?;
                wilderness_zone::leave_player(self, &p)?;
                Value::Null
            }
            "dropRate" => json!(wilderness_zone::new_drop_rate(num(&a["level"]))),
            "wildBorders" => self
                .instance()
                .borders()
                .iter()
                .map(Self::describe_borders)
                .collect(),
            "constants" => json!([
                wilderness_zone::WILDERNESS_PROT_ATTR,
                wilderness_zone::WILDERNESS_HIGHER_NEXTFEE
            ]),
            "updateOverlay" => {
                let z = z?;
                let dark = z.behavior::<DarkZone>().ok_or(Error::ClassCast)?;
                let p = self.player(&a["node"])?;
                dark.update_overlay(self, &p)?;
                Value::Null
            }
            "checkDark" => {
                let p = self.player(&a["node"])?;
                json!(dark_zone::check_dark_area(self, &p)?)
            }
            "process" => {
                if z?.behavior::<DarkZone>().is_none() {
                    return Err(Error::ClassCast);
                }
                dark_zone::process_use_with(self, &n?, num(&a["used"]), num(&a["with"]))?;
                Value::Null
            }
            "darkOpen" => {
                let p = self.player(&a["node"])?;
                dark_zone::darkness_open(self, &p)?;
                Value::Null
            }
            "darkClose" => {
                let p = self.player(&a["node"])?;
                json!(dark_zone::darkness_close(self, &p)?)
            }
            "openOverlay" => {
                let p = self.player(&a["node"])?;
                dark_zone::Overlays::open_overlay(self, &p, Self::component(&a["component"]))?;
                Value::Null
            }
            "closeOverlay" => {
                let p = self.player(&a["node"])?;
                dark_zone::Overlays::close_overlay(self, &p)?;
                Value::Null
            }
            "runScript" => {
                let script = self
                    .scripts
                    .get(num(&a["index"]) as usize)
                    .cloned()
                    .ok_or_else(|| Error::Binding("IndexOutOfBoundsException".into()))?;
                json!(dark_zone::glow_script(
                    self,
                    &script,
                    a["stage"].as_i64().unwrap_or(0) as i32
                )?)
            }
            "runTask" => {
                let task = self
                    .tasks
                    .get(num(&a["index"]) as usize)
                    .cloned()
                    .ok_or_else(|| Error::Binding("IndexOutOfBoundsException".into()))?;
                dark_zone::tinderbox_task(self, &task)?;
                Value::Null
            }
            "tick" => {
                let runner = self.runner.clone();
                for _ in 0..a["count"].as_i64().unwrap_or(1) {
                    runner
                        .update_all(&mut (), self)
                        .map_err(|e| Error::Binding(format!("{e:?}")))?;
                }
                json!(runner.current().len())
            }
            "lightTable" => {
                let mut rows: Vec<Value> = LightSource::ALL
                    .iter()
                    .map(|s| {
                        json!([
                            light_constant(*s),
                            s.name(),
                            s.level(),
                            s.raw(),
                            s.product(),
                            s.is_open(),
                            s.interface_id(),
                            s.strength()
                        ])
                    })
                    .collect();
                rows.push(json!(LightSource::raw_ids()));
                Value::Array(rows)
            }
            "lightFor" => json!([
                light(LightSource::for_product_id(num(&a["id"]))),
                light(LightSource::for_id(num(&a["id"])))
            ]),
            "activeLight" => {
                let p = self.player(&a["node"])?;
                light(dark_zone::active_light_source(self, &p)?)
            }
            "hasLight" => {
                let p = self.player(&a["node"])?;
                json!(dark_zone::has_active_light_source(self, &p)?)
            }
            "gloves" => (-1..=15)
                .map(|i| json!(wilderness_zone::brawling_gloves(i)))
                .collect(),
            other => return Err(Error::Binding(format!("unknown action {other}"))),
        })
    }
}

impl zones::Host for Model {
    type Target = Target;
    type Opaque = Value;
    fn kind(&mut self, entity: &NodeHandle) -> Result<EntityKind> {
        Ok(self.nodes[&self.slot_of(entity)].kind)
    }
    fn monitor(&mut self, entity: &NodeHandle) -> Result<Monitor<Self>> {
        Ok(self.nodes[&self.slot_of(entity)].monitor.clone())
    }
    fn region(&mut self, region_id: i32) -> Result<region::Handle> {
        Ok(self.raw_region(region_id))
    }
    fn target_entity(&mut self, target: &Target) -> Result<Option<NodeHandle>> {
        Ok(match target {
            Target::Entity(handle) => Some(handle.clone()),
            _ => None,
        })
    }
    fn location(&mut self, entity: &NodeHandle) -> Result<Option<Location>> {
        let location = place(&self.attr(entity, "location"));
        self.emit("location", vec![self.slot(Some(entity))])?;
        Ok(location)
    }
    fn is_artificial(&mut self, player: &NodeHandle) -> Result<bool> {
        let answer = self.flag(player, "artificial", false);
        self.emit("artificial", vec![self.slot(Some(player))])?;
        Ok(answer)
    }
    fn entity_update_location(
        &mut self,
        entity: &NodeHandle,
        last: Option<&Location>,
    ) -> Result<()> {
        self.emit("entityUpdate", vec![self.slot(Some(entity)), coord(last)])
    }
    fn set_invisible(&mut self, npc: &NodeHandle, invisible: bool) -> Result<()> {
        self.put(npc, "invisible", json!(invisible));
        self.emit("setInvisible", vec![self.slot(Some(npc)), json!(invisible)])
    }
    fn entity_continue_attack(
        &mut self,
        entity: &NodeHandle,
        target: &NodeHandle,
        style: Option<&Value>,
        message: bool,
    ) -> Result<bool> {
        let answer = self.flag(entity, "continueAttack", true);
        self.emit(
            "entityContinueAttack",
            vec![
                self.slot(Some(entity)),
                self.slot(Some(target)),
                style.cloned().unwrap_or(Value::Null),
                json!(message),
            ],
        )?;
        Ok(answer)
    }
    fn skull_wilderness(&mut self, player: &NodeHandle) -> Result<bool> {
        let answer = self.flag(player, "wilderness", false);
        self.emit("wilderness", vec![self.slot(Some(player))])?;
        Ok(answer)
    }
    fn skull_level(&mut self, player: &NodeHandle) -> Result<i32> {
        let answer = self.int(player, "skullLevel", 0);
        self.emit("skullLevel", vec![self.slot(Some(player))])?;
        Ok(answer)
    }
    fn packet_message(&mut self, player: &NodeHandle, message: &str) -> Result<()> {
        self.emit(
            "packetMessage",
            vec![self.slot(Some(player)), json!(message)],
        )
    }
    fn player_message(&mut self, player: &NodeHandle, message: &str) -> Result<()> {
        self.emit(
            "playerMessage",
            vec![self.slot(Some(player)), json!(message)],
        )
    }
    fn has_timer(&mut self, entity: &NodeHandle, name: &str) -> Result<bool> {
        let answer = self.flag(entity, "teleTimer", false);
        self.emit("timer", vec![self.slot(Some(entity)), json!(name)])?;
        Ok(answer)
    }
    fn teleport_locked(&mut self, entity: &NodeHandle) -> Result<bool> {
        let answer = self.flag(entity, "teleportLocked", false);
        self.emit("teleportLocked", vec![self.slot(Some(entity))])?;
        Ok(answer)
    }
    fn target_item_id(&mut self, target: Option<&Target>) -> Result<i32> {
        match target {
            None => Err(Error::Null),
            Some(Target::Item(id)) => Ok(*id),
            Some(_) => Err(Error::ClassCast),
        }
    }
    fn now_millis(&mut self) -> Result<i64> {
        // MapZone.checkMulti reads the wall clock; no combat time is set here.
        Ok(1_800_000_000_000)
    }
    fn multi_zone(&mut self, entity: &NodeHandle) -> Result<bool> {
        let answer = self.flag(entity, "multiZone", false);
        self.emit("multiZone", vec![self.slot(Some(entity))])?;
        Ok(answer)
    }
    fn entity_ignore_multi(&mut self, entity: &NodeHandle, victim: &NodeHandle) -> Result<bool> {
        let answer = self.flag(entity, "ignoreMulti", false);
        self.emit(
            "entityIgnoreMulti",
            vec![self.slot(Some(entity)), self.slot(Some(victim))],
        )?;
        Ok(answer)
    }
    fn familiar_owner(&mut self, familiar: &NodeHandle) -> Result<Option<NodeHandle>> {
        let owner = self.handle(&self.attr(familiar, "owner"));
        if let Some(owner) = &owner
            && self.nodes[&self.slot_of(owner)].kind != EntityKind::Player
        {
            return Err(Error::ClassCast);
        }
        self.emit("owner", vec![self.slot(Some(familiar))])?;
        Ok(owner)
    }
    fn combat_attacker(&mut self, entity: &NodeHandle) -> Result<Option<NodeHandle>> {
        let stored = self.handle(&self.attr(entity, "attacker"));
        self.emit(
            "attribute",
            vec![self.slot(Some(entity)), json!("combat-attacker")],
        )?;
        Ok(stored)
    }
    fn combat_time(&mut self, entity: &NodeHandle) -> Result<Option<i64>> {
        let stored = self.attr(entity, "combatTime").as_i64();
        self.emit(
            "attribute",
            vec![self.slot(Some(entity)), json!("combat-time")],
        )?;
        Ok(stored)
    }
    fn active(&mut self, entity: &NodeHandle) -> Result<bool> {
        let answer = self.flag(entity, "active", true);
        self.emit("active", vec![self.slot(Some(entity))])?;
        Ok(answer)
    }
    fn id(&mut self, entity: &NodeHandle) -> Result<i32> {
        let answer = self.int(entity, "npcId", 0);
        self.emit("id", vec![self.slot(Some(entity))])?;
        Ok(answer)
    }
    fn music_playing(&mut self, player: &NodeHandle) -> Result<bool> {
        let answer = self.flag(player, "playing", false);
        self.emit("musicPlaying", vec![self.slot(Some(player))])?;
        Ok(answer)
    }
    fn music_play_default(&mut self, player: &NodeHandle) -> Result<()> {
        self.emit("playDefault", vec![self.slot(Some(player))])
    }
    fn music_unlock(&mut self, player: &NodeHandle, id: i32, play: Option<bool>) -> Result<()> {
        self.emit(
            "unlock",
            vec![self.slot(Some(player)), json!(id), json!(play)],
        )
    }
    fn random_next_int(&mut self, bound: i32) -> Result<i32> {
        self.random.next_bound(bound).ok_or(Error::IllegalArgument)
    }
    fn fresh_random_next_int(&mut self, _: i32) -> Result<i32> {
        Err(Error::Binding("getRandomLoc is outside this corpus".into()))
    }
    fn teleport_permitted(&mut self, _: &Location) -> Result<bool> {
        self.emit("teleportPermitted", vec![])?;
        Ok(true)
    }
}

impl zone_impls::Host for Model {
    fn inventory_amount(&mut self, player: &NodeHandle, item_id: i32) -> Result<i32> {
        let amount = self.int(player, "rum", 0);
        self.emit(
            "inventoryAmount",
            vec![self.slot(Some(player)), json!(item_id)],
        )?;
        Ok(amount)
    }
    fn inventory_remove(&mut self, player: &NodeHandle, item_id: i32, amount: i32) -> Result<bool> {
        self.emit(
            "inventoryRemove",
            vec![self.slot(Some(player)), json!(item_id), json!(amount)],
        )?;
        Ok(true)
    }
    fn interface_resizable(&mut self, player: &NodeHandle) -> Result<bool> {
        let answer = self.flag(player, "resizable", false);
        self.emit("resizable", vec![self.slot(Some(player))])?;
        Ok(answer)
    }
    fn window_pane_id(&mut self, player: &NodeHandle) -> Result<i32> {
        let answer = self.int(player, "windowPane", 548);
        self.emit("windowPane", vec![self.slot(Some(player))])?;
        Ok(answer)
    }
    fn send_interface(
        &mut self,
        player: &NodeHandle,
        window: i32,
        component: i32,
        interface: i32,
        walkable: bool,
    ) -> Result<()> {
        self.emit(
            "packet",
            vec![
                json!("Interface"),
                self.slot(Some(player)),
                json!(window),
                json!(component),
                json!(interface),
                json!(walkable),
            ],
        )
    }
    fn send_close_interface(
        &mut self,
        player: &NodeHandle,
        window: i32,
        component: i32,
        interface: i32,
        walkable: bool,
    ) -> Result<()> {
        self.emit(
            "packet",
            vec![
                json!("CloseInterface"),
                self.slot(Some(player)),
                json!(window),
                json!(component),
                json!(interface),
                json!(walkable),
            ],
        )
    }
    fn interface_config(
        &mut self,
        player: &NodeHandle,
        interface: i32,
        child: i32,
        hide: bool,
    ) -> Result<()> {
        self.emit(
            "interfaceConfig",
            vec![
                self.slot(Some(player)),
                json!(interface),
                json!(child),
                json!(hide),
            ],
        )
    }
    fn set_multi_zone(&mut self, entity: &NodeHandle, multi: bool) -> Result<()> {
        self.put(entity, "multiZone", json!(multi));
        self.emit("setMultiZone", vec![self.slot(Some(entity)), json!(multi)])
    }
    fn npc_walkable(&mut self, entity: &NodeHandle) -> Result<bool> {
        let answer = self.flag(entity, "npcWalkable", false);
        self.emit("npcWalkable", vec![self.slot(Some(entity))])?;
        Ok(answer)
    }
    fn local_npcs(
        &mut self,
        location: Option<&Location>,
        distance: i32,
    ) -> Result<Vec<NodeHandle>> {
        let location = location.ok_or(Error::Null)?;
        self.emit("localNpcs", vec![coord(Some(location)), json!(distance)])?;
        Ok(self.input["localNpcs"]
            .as_array()
            .map(|ids| ids.iter().filter_map(|id| self.handle(id)).collect())
            .unwrap_or_default())
    }
    fn local_players(
        &mut self,
        location: Option<&Location>,
        distance: i32,
    ) -> Result<Vec<NodeHandle>> {
        let location = location.ok_or(Error::Null)?;
        self.emit("localPlayers", vec![coord(Some(location)), json!(distance)])?;
        Ok(self.input["localPlayers"]
            .as_array()
            .map(|ids| ids.iter().filter_map(|id| self.handle(id)).collect())
            .unwrap_or_default())
    }
    fn invisible(&mut self, npc: &NodeHandle) -> Result<bool> {
        let answer = self.flag(npc, "invisible", false);
        self.emit("invisible", vec![self.slot(Some(npc))])?;
        Ok(answer)
    }
    fn has_attack_option(&mut self, npc: &NodeHandle) -> Result<bool> {
        let answer = self.flag(npc, "attackOption", true);
        self.emit("attackOption", vec![self.slot(Some(npc))])?;
        Ok(answer)
    }
    fn should_prevent_stacking(&mut self, npc: &NodeHandle, mover: &NodeHandle) -> Result<bool> {
        let answer = self.flag(npc, "preventStacking", true);
        self.emit(
            "preventStacking",
            vec![self.slot(Some(npc)), self.slot(Some(mover))],
        )?;
        Ok(answer)
    }
    fn size(&mut self, entity: &NodeHandle) -> Result<i32> {
        let answer = self.int(entity, "size", 1);
        self.emit("size", vec![self.slot(Some(entity))])?;
        Ok(answer)
    }
    fn rights(&mut self, player: &NodeHandle) -> Result<Rights> {
        let answer = match self.attr(player, "rights").as_str() {
            Some("PLAYER_MODERATOR") => Rights::PlayerModerator,
            Some("ADMINISTRATOR") => Rights::Administrator,
            _ => Rights::RegularPlayer,
        };
        self.emit("rights", vec![self.slot(Some(player))])?;
        Ok(answer)
    }
    fn home_location(&mut self) -> Result<Option<Location>> {
        Ok(None)
    }
    fn teleporter_send(
        &mut self,
        player: &NodeHandle,
        location: Option<&Location>,
        kind: TeleportType,
    ) -> Result<bool> {
        let TeleportType::Normal = kind;
        self.emit(
            "teleporterSend",
            vec![self.slot(Some(player)), coord(location), json!("NORMAL")],
        )?;
        Ok(true)
    }
    fn restore_tabs(&mut self, player: &NodeHandle) -> Result<()> {
        self.emit("restoreTabs", vec![self.slot(Some(player))])
    }
}

impl dark_zone::Overlays for Model {
    fn open_overlay(&mut self, player: &NodeHandle, component: Component) -> Result<()> {
        self.emit(
            "openOverlay",
            vec![self.slot(Some(player)), component_label(Some(component))],
        )?;
        if let Some(old) = self.row(player).overlay
            && !self.close_own(player, old)?
        {
            return Ok(());
        }
        self.row(player).overlay = Some(component);
        self.open_component(player, component)
    }
    fn close_overlay(&mut self, player: &NodeHandle) -> Result<()> {
        self.emit("closeOverlay", vec![self.slot(Some(player))])?;
        if let Some(overlay) = self.row(player).overlay
            && dark_zone::Overlays::close_component(self, player, overlay)?
        {
            self.row(player).overlay = None;
        }
        Ok(())
    }
    fn close_component(&mut self, player: &NodeHandle, component: Component) -> Result<bool> {
        self.emit(
            "closeComponent",
            vec![self.slot(Some(player)), component_label(Some(component))],
        )?;
        self.close_own(player, component)
    }
    fn overlay(&mut self, player: &NodeHandle) -> Result<Option<Component>> {
        self.emit("getOverlay", vec![self.slot(Some(player))])?;
        Ok(self.row(player).overlay)
    }
    fn is_item(&mut self, target: &Target) -> Result<bool> {
        Ok(matches!(target, Target::Item(_)))
    }
}

impl dark_zone::Host for Model {
    fn inventory_items(&mut self, player: &NodeHandle) -> Result<Vec<Option<i32>>> {
        let items = self.items(player, "inventory");
        self.emit("toArray", vec![self.slot(Some(player)), json!("inventory")])?;
        Ok(items)
    }
    fn equipment_items(&mut self, player: &NodeHandle) -> Result<Vec<Option<i32>>> {
        let items = self.items(player, "equipment");
        self.emit("toArray", vec![self.slot(Some(player)), json!("equipment")])?;
        Ok(items)
    }
    fn constant_glow(&mut self, player: &NodeHandle) -> Result<bool> {
        let active = self.flag(player, "glow", false);
        self.emit(
            "perk",
            vec![self.slot(Some(player)), json!("CONSTANT_GLOW")],
        )?;
        Ok(active)
    }
    fn option_name(&mut self, option: &Value) -> Result<Option<String>> {
        Ok(option.as_str().map(str::to_owned))
    }
    fn hook_used_with(&mut self, entity: &NodeHandle, zone: &Zone<Self>) -> Result<()> {
        let label = self.zone_label(zone);
        self.emit(
            "hook",
            vec![self.slot(Some(entity)), json!("UseWithEvent"), label],
        )
    }
    fn unhook(&mut self, entity: &NodeHandle, zone: &Zone<Self>) -> Result<()> {
        let label = self.zone_label(zone);
        self.emit("unhook", vec![self.slot(Some(entity)), label])
    }
    fn dark_extension(&mut self, player: &NodeHandle) -> Result<Option<InsectPulse>> {
        let extension = self.row(player).extension.clone();
        self.emit(
            "getExtension",
            vec![self.slot(Some(player)), json!("DarkZone")],
        )?;
        Ok(extension)
    }
    fn add_dark_extension(&mut self, player: &NodeHandle, pulse: InsectPulse) -> Result<()> {
        self.row(player).extension = Some(pulse.clone());
        let label = self.pulse_label(&pulse);
        self.emit(
            "addExtension",
            vec![self.slot(Some(player)), json!("DarkZone"), label],
        )
    }
    fn pulser_submit(&mut self, pulse: &pulses::Handle) -> Result<()> {
        self.runner
            .submit(Some(pulse.clone()))
            .map_err(|e| Error::Binding(format!("{e:?}")))
    }
    fn component_open(&mut self, player: &NodeHandle, component: Component) -> Result<()> {
        self.emit(
            "componentOpen",
            vec![self.slot(Some(player)), component_label(Some(component))],
        )
    }
    fn component_close(&mut self, player: &NodeHandle, component: Component) -> Result<bool> {
        self.emit(
            "componentClose",
            vec![self.slot(Some(player)), component_label(Some(component))],
        )?;
        Ok(self.answer("componentClose").unwrap_or(true))
    }
    fn manual_hit(
        &mut self,
        entity: &NodeHandle,
        source: &NodeHandle,
        hit: i32,
        kind: Hitsplat,
    ) -> Result<()> {
        let Hitsplat::Normal = kind;
        self.emit(
            "manualHit",
            vec![
                self.slot(Some(entity)),
                self.slot(Some(source)),
                json!(hit),
                json!("NORMAL"),
            ],
        )
    }
    fn queue_script(
        &mut self,
        entity: &NodeHandle,
        delay: i32,
        strength: QueueStrength,
        persist: bool,
        script: GlowScript,
    ) -> Result<()> {
        let strength = match strength {
            QueueStrength::Weak => "WEAK",
            QueueStrength::Normal => "NORMAL",
            QueueStrength::Strong => "STRONG",
            QueueStrength::Soft => "SOFT",
        };
        self.emit(
            "queueScript",
            vec![
                self.slot(Some(entity)),
                json!(delay),
                json!(strength),
                json!(persist),
            ],
        )?;
        self.scripts.push(script);
        Ok(())
    }
    fn stop_executing(&mut self, entity: &NodeHandle) -> Result<bool> {
        self.emit("stopExecuting", vec![self.slot(Some(entity))])?;
        Ok(self.answer("stopExecuting").unwrap_or(true))
    }
    fn run_task(
        &mut self,
        entity: &NodeHandle,
        delay: i32,
        repeat: i32,
        task: TinderboxTask,
    ) -> Result<()> {
        self.emit(
            "runTask",
            vec![self.slot(Some(entity)), json!(delay), json!(repeat)],
        )?;
        self.tasks.push(task);
        Ok(())
    }
    fn item_name_by_id(&mut self, id: i32) -> Result<Option<String>> {
        self.emit("getItemName", vec![json!(id)])?;
        Ok(self.item_name_of(id))
    }
}

impl Model {
    fn items(&self, player: &NodeHandle, which: &str) -> Vec<Option<i32>> {
        self.attr(player, which)
            .as_array()
            .map(|ids| {
                ids.iter()
                    .map(|id| id.as_i64().map(|id| id as i32))
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl wilderness_zone::Host for Model {
    fn skull_wilderness_disabled(&mut self, player: &NodeHandle) -> Result<bool> {
        let answer = self.flag(player, "wildDisabled", false);
        self.emit("wildernessDisabled", vec![self.slot(Some(player))])?;
        Ok(answer)
    }
    fn set_skull_level(&mut self, player: &NodeHandle, level: i32) -> Result<()> {
        self.put(player, "skullLevel", json!(level));
        self.emit("setSkullLevel", vec![self.slot(Some(player)), json!(level)])
    }
    fn set_skull_wilderness(&mut self, player: &NodeHandle, wilderness: bool) -> Result<()> {
        self.put(player, "wilderness", json!(wilderness));
        self.emit(
            "setWilderness",
            vec![self.slot(Some(player)), json!(wilderness)],
        )
    }
    fn skull_deep_wilderness(&mut self, player: &NodeHandle) -> Result<bool> {
        let answer = self.flag(player, "deep", false);
        self.emit("deepWilderness", vec![self.slot(Some(player))])?;
        Ok(answer)
    }
    fn deepwild_value_risk(&mut self, player: &NodeHandle) -> Result<i64> {
        let risk = self.attr(player, "risk").as_i64().unwrap_or(0);
        self.emit(
            "attribute",
            vec![self.slot(Some(player)), json!("deepwild-value-risk")],
        )?;
        Ok(risk)
    }
    fn send_string(
        &mut self,
        player: &NodeHandle,
        text: &str,
        interface: i32,
        child: i32,
    ) -> Result<()> {
        self.emit(
            "sendString",
            vec![
                self.slot(Some(player)),
                json!(text),
                json!(interface),
                json!(child),
            ],
        )
    }
    fn wild_pvp_enabled(&mut self) -> Result<bool> {
        if self.input["noSettings"].as_bool().unwrap_or(false) {
            return Err(Error::Null);
        }
        let enabled = self.input["pvp"].as_bool().unwrap_or(true);
        self.emit("wildPvp", vec![])?;
        Ok(enabled)
    }
    fn set_attack_option(&mut self, player: &NodeHandle) -> Result<()> {
        self.emit("optionSet", vec![self.slot(Some(player)), json!("Attack")])
    }
    fn remove_attack_option(&mut self, player: &NodeHandle) -> Result<()> {
        self.emit(
            "optionRemove",
            vec![self.slot(Some(player)), json!("Attack")],
        )
    }
    fn has_familiar(&mut self, player: &NodeHandle) -> Result<bool> {
        let has =
            !self.attr(player, "familiar").is_null() || self.flag(player, "hasFamiliar", false);
        self.emit("hasFamiliar", vec![self.slot(Some(player))])?;
        Ok(has)
    }
    fn has_pet(&mut self, player: &NodeHandle) -> Result<bool> {
        let pet = self.flag(player, "pet", false);
        self.emit("hasPet", vec![self.slot(Some(player))])?;
        Ok(pet)
    }
    fn familiar(&mut self, player: &NodeHandle) -> Result<Option<NodeHandle>> {
        let familiar = self.handle(&self.attr(player, "familiar"));
        if let Some(familiar) = &familiar
            && self.nodes[&self.slot_of(familiar)].kind != EntityKind::Familiar
        {
            return Err(Error::ClassCast);
        }
        self.emit("getFamiliar", vec![self.slot(Some(player))])?;
        Ok(familiar)
    }
    fn familiar_transform(&mut self, familiar: &NodeHandle) -> Result<()> {
        self.emit("transform", vec![self.slot(Some(familiar))])
    }
    fn familiar_is_combat(&mut self, familiar: &NodeHandle) -> Result<bool> {
        let combat = self.flag(familiar, "combatFamiliar", false);
        self.emit("combatFamiliar", vec![self.slot(Some(familiar))])?;
        Ok(combat)
    }
    fn npc_retransform(&mut self, npc: &NodeHandle) -> Result<()> {
        self.emit("reTransform", vec![self.slot(Some(npc))])
    }
    fn appearance_sync(&mut self, player: &NodeHandle) -> Result<()> {
        self.emit("sync", vec![self.slot(Some(player))])
    }
    fn npc_aggressive(&mut self, npc: &NodeHandle) -> Result<bool> {
        let aggressive = self.flag(npc, "aggressive", false);
        self.emit("aggressive", vec![self.slot(Some(npc))])?;
        Ok(aggressive)
    }
    fn set_wilderness_aggression(&mut self, npc: &NodeHandle) -> Result<()> {
        self.emit(
            "aggressiveHandler",
            vec![self.slot(Some(npc)), json!("WILDERNESS")],
        )
    }
    fn npc_name(&mut self, npc: &NodeHandle) -> Result<Option<String>> {
        let name = if self.has(npc, "name") {
            self.attr(npc, "name").as_str().map(str::to_owned)
        } else {
            Some(format!("npc{}", self.slot_of(npc)))
        };
        self.emit("npcName", vec![self.slot(Some(npc))])?;
        Ok(name)
    }
    fn npc_combat_level(&mut self, npc: &NodeHandle) -> Result<i32> {
        let level = self.int(npc, "npcCombat", 1);
        self.emit("npcCombat", vec![self.slot(Some(npc))])?;
        Ok(level)
    }
    fn current_combat_level(&mut self, player: &NodeHandle) -> Result<i32> {
        let level = self.int(player, "combat", 3);
        self.emit("combatLevel", vec![self.slot(Some(player))])?;
        Ok(level)
    }
    fn combat_pulse_attack(&mut self, player: &NodeHandle, target: &Target) -> Result<()> {
        let label = self.target_label(Some(target));
        self.emit("attack", vec![self.slot(Some(player)), label])
    }
    fn item_name(&mut self, id: i32) -> Result<Option<String>> {
        let name = self.item_name_of(id);
        self.emit("itemGetName", vec![json!(id)])?;
        Ok(name)
    }
    fn drop_location(&mut self, npc: &NodeHandle) -> Result<Option<Location>> {
        let location = place(&self.attr(npc, "drop"));
        self.emit("dropLocation", vec![self.slot(Some(npc))])?;
        Ok(location)
    }
    fn ground_item_create(
        &mut self,
        id: i32,
        amount: i32,
        location: Option<&Location>,
        player: &NodeHandle,
    ) -> Result<()> {
        self.emit(
            "groundItem",
            vec![
                json!(id),
                json!(amount),
                coord(location),
                self.slot(Some(player)),
            ],
        )
    }
    fn username(&mut self, player: &NodeHandle) -> Result<Option<String>> {
        let name = if self.has(player, "username") {
            self.attr(player, "username").as_str().map(str::to_owned)
        } else {
            Some(format!("user{}", self.slot_of(player)))
        };
        self.emit("username", vec![self.slot(Some(player))])?;
        Ok(name)
    }
    fn send_news(&mut self, text: &str) -> Result<()> {
        self.emit("news", vec![json!(text)])
    }
}

/// The runner's services: the only pulses of a case are insect pulses.
impl pulses::Context<()> for Model {
    fn pulse(&mut self, pulse: &pulses::Handle, _: &mut ()) -> pulses::Result<bool> {
        let insect = self
            .insects
            .iter()
            .find(|insect| insect.handle().same(pulse))
            .cloned()
            .ok_or(pulses::Error::Null)?;
        dark_zone::insect_pulse(self, &insect).map_err(pulse_error)
    }
    fn node_active(&mut self, node: pulses::Node, _: &mut ()) -> pulses::Result<bool> {
        let handle = self
            .nodes
            .values()
            .find(|row| row.handle.node == node)
            .map(|row| row.handle.clone())
            .ok_or(pulses::Error::Null)?;
        zones::Host::active(self, &handle).map_err(pulse_error)
    }
    fn exception(&mut self, _: &pulses::Error, _: &mut ()) -> pulses::Result<()> {
        self.events.push(json!(["stackTrace"]));
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
    fn log(&mut self, level: pulses::Level, message: String, _: &mut ()) -> pulses::Result<()> {
        let level = match level {
            pulses::Level::Warn => "WARN",
            pulses::Level::Error => "ERR",
        };
        self.emit("log", vec![json!(level), json!(message)])
            .map_err(pulse_error)
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

fn make(model: &Model, spec: &Value) -> Zone<Model> {
    match spec["class"].as_str().unwrap_or("wilderness") {
        "wilderness" => model.wilderness.clone(),
        "wildernessBorders" => wilderness_zone::with_borders(
            spec["borders"]
                .as_array()
                .unwrap()
                .iter()
                .map(Model::new_borders)
                .collect(),
        ),
        "dark" => dark_zone::dark_zone(),
        "plain" => Zone::new(spec["id"].as_str(), true, &[], Plain),
        "multiway" => model.multiway.clone(),
        other => panic!("unknown zone class {other}"),
    }
}

/// A node's slot, zone list, kind, overlay and stored pulse after a case.
type NodeSummary = (
    i32,
    zones::List<RegionZone<Model>>,
    EntityKind,
    Option<Component>,
    Option<InsectPulse>,
);

pub fn evaluate(input: &Value) -> Value {
    let mut model = Model {
        input: input.clone(),
        _state: State::default(),
        nodes: BTreeMap::new(),
        zones: Vec::new(),
        wilderness: wilderness_zone::wilderness_zone(),
        multiway: zone_impls::multiway_combat_zone(),
        regions: BTreeMap::new(),
        events: Vec::new(),
        calls: HashMap::new(),
        fired: BTreeSet::new(),
        runner: pulses::Runner::default(),
        random: JavaRandom::new(input["seed"].as_i64().unwrap_or(530)),
        insects: Vec::new(),
        scripts: Vec::new(),
        tasks: Vec::new(),
    };
    for spec in input["nodes"].as_array().into_iter().flatten() {
        let id = num(&spec["id"]);
        let kind = match spec["kind"].as_str().unwrap_or("player") {
            "player" => EntityKind::Player,
            "npc" => EntityKind::Npc,
            "familiar" => EntityKind::Familiar,
            _ => EntityKind::Other,
        };
        let node = model._state.add_npc_at(id, None, [1, 2, 3, 4]);
        let handle = model._state.retain_node(node).unwrap();
        model.nodes.insert(
            id,
            Row {
                monitor: Monitor::new(handle.clone()),
                handle,
                kind,
                state: spec.as_object().unwrap().clone(),
                overlay: None,
                extension: None,
            },
        );
    }
    for spec in input["zones"].as_array().into_iter().flatten() {
        let zone = make(&model, spec);
        model
            .zones
            .push((spec["id"].as_str().unwrap().to_owned(), zone));
    }
    let results: Vec<Value> = input["actions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|action| match model.act(action) {
            Ok(value) => json!({"value": value}),
            Err(error) => json!({"error": name(&error)}),
        })
        .collect();
    let rows: Vec<NodeSummary> = model
        .nodes
        .iter()
        .map(|(id, row)| {
            (
                *id,
                row.monitor.zones(),
                row.kind,
                row.overlay,
                row.extension.clone(),
            )
        })
        .collect();
    let mut monitors = BTreeMap::new();
    for (id, zones, kind, overlay, extension) in rows {
        let extension = match extension {
            None => Value::Null,
            Some(pulse) => {
                let label = model.pulse_label(&pulse);
                json!([label, pulses::is_running_base(pulse.handle())])
            }
        };
        let overlay = if kind == EntityKind::Player {
            component_label(overlay)
        } else {
            Value::Null
        };
        monitors.insert(
            id.to_string(),
            json!({"zones": model.describe(&zones), "overlay": overlay, "extension": extension}),
        );
    }
    let mut regions = BTreeMap::new();
    for (id, region) in &model.regions {
        let zones = activity::region_zones::<Model>(region).unwrap();
        if !zones.is_empty() {
            regions.insert(id.to_string(), model.describe(&zones));
        }
    }
    json!({
        "results": results,
        "events": model.events,
        "monitors": monitors,
        "regions": regions,
        "queue": model.runner.current().len(),
        "scripts": model.scripts.len(),
        "tasks": model.tasks.len(),
        "probe": model.random.next_i32(),
    })
}
