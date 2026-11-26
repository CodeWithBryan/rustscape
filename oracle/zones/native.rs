//! Native replay of the zones and region-activity corpus. The case's inputs
//! become the required services of `zones::Host`, `zone_impls::Host` and
//! `region_activity::Host`; every service records its call. The regions,
//! chunks and entity lists, `region::load`, the activity pulse and the pulse
//! runner are the real native ones.
use rs09_game_core::random::JavaRandom;
use rs09_server::{
    definitions::item::Shared,
    pulses,
    world::{
        chunk::HandleChunk,
        doors::{self, Location},
        landscape::{Chunks, Mapscape},
        player_walk::{NodeHandle, State},
        region, region_activity as activity,
        zone_impls::{self, ModeratorZone, Rights, TeleportType},
        zones::{
            self, Behavior, Borders, EntityKind, Error, Monitor, MusicZone, RegionZone,
            Restriction, Result, Zone, ZoneType,
        },
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
fn text(value: &Value) -> Option<&str> {
    value.as_str()
}
fn opaque(value: &Value) -> Option<&Value> {
    (!value.is_null()).then_some(value)
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
}
pub struct Model {
    input: Value,
    _state: State,
    nodes: BTreeMap<i32, Row>,
    zones: Vec<(String, Zone<Model>)>,
    borders: HashMap<String, Borders>,
    musics: Vec<(String, MusicZone)>,
    regions: BTreeMap<i32, region::Handle>,
    events: Vec<Value>,
    calls: HashMap<String, i64>,
    fired: BTreeSet<usize>,
    permitted_calls: usize,
    runner: pulses::Runner,
    random: JavaRandom,
    fresh: JavaRandom,
    home: Option<Location>,
}

/// A zone whose every overridable member records and answers from the case.
/// The const parameter gives the two scripted classes of the oracle.
struct Scripted<const CLASS: usize> {
    id: String,
}
struct Plain;
impl Behavior<Model> for Plain {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
impl<const CLASS: usize> Scripted<CLASS> {
    fn ask(&self, host: &mut Model, hook: &str, args: Vec<Value>) -> Result<Option<bool>> {
        let key = format!("{}.{hook}", self.id);
        host.emit(&key, args)?;
        Ok(host.answer(&key))
    }
}
impl<const CLASS: usize> Behavior<Model> for Scripted<CLASS> {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn enter(&self, zone: &Zone<Model>, host: &mut Model, e: &NodeHandle) -> Result<bool> {
        match self.ask(host, "enter", vec![host.slot(Some(e))])? {
            Some(answer) => Ok(answer),
            None => zones::base_enter(zone, host, e),
        }
    }
    fn leave(
        &self,
        _: &Zone<Model>,
        host: &mut Model,
        e: &NodeHandle,
        logout: bool,
    ) -> Result<bool> {
        Ok(self
            .ask(host, "leave", vec![host.slot(Some(e)), json!(logout)])?
            .unwrap_or(true))
    }
    fn can_logout(&self, _: &Zone<Model>, host: &mut Model, p: &NodeHandle) -> Result<bool> {
        Ok(self
            .ask(host, "canLogout", vec![host.slot(Some(p))])?
            .unwrap_or(true))
    }
    fn death(
        &self,
        _: &Zone<Model>,
        host: &mut Model,
        e: &NodeHandle,
        killer: Option<&NodeHandle>,
    ) -> Result<bool> {
        Ok(self
            .ask(host, "death", vec![host.slot(Some(e)), host.slot(killer)])?
            .unwrap_or(false))
    }
    fn interact(
        &self,
        _: &Zone<Model>,
        host: &mut Model,
        e: &NodeHandle,
        target: Option<&Target>,
        option: Option<&Value>,
    ) -> Result<bool> {
        let args = vec![
            host.slot(Some(e)),
            host.target_label(target),
            option.cloned().unwrap_or(Value::Null),
        ];
        Ok(self.ask(host, "interact", args)?.unwrap_or(false))
    }
    fn handle_use_with(
        &self,
        _: &Zone<Model>,
        host: &mut Model,
        player: &NodeHandle,
        used: Option<&Value>,
        with: Option<&Target>,
    ) -> Result<bool> {
        let args = vec![
            host.slot(Some(player)),
            used.cloned().unwrap_or(Value::Null),
            host.target_label(with),
        ];
        Ok(self.ask(host, "useWith", args)?.unwrap_or(false))
    }
    fn action_button(
        &self,
        _: &Zone<Model>,
        host: &mut Model,
        player: &NodeHandle,
        interface_id: i32,
        button_id: i32,
        slot: i32,
        item_id: i32,
        opcode: i32,
    ) -> Result<bool> {
        let args = vec![
            host.slot(Some(player)),
            json!(interface_id),
            json!(button_id),
            json!(slot),
            json!(item_id),
            json!(opcode),
        ];
        Ok(self.ask(host, "button", args)?.unwrap_or(false))
    }
    fn continue_pvp(
        &self,
        _: &Zone<Model>,
        host: &mut Model,
        e: Option<&NodeHandle>,
        target: Option<&Target>,
        style: Option<&Value>,
        message: bool,
    ) -> Result<bool> {
        let args = vec![
            host.slot(e),
            host.target_label(target),
            style.cloned().unwrap_or(Value::Null),
            json!(message),
        ];
        Ok(self.ask(host, "pvp", args)?.unwrap_or(false))
    }
    fn continue_attack(
        &self,
        _: &Zone<Model>,
        host: &mut Model,
        e: &NodeHandle,
        target: Option<&Target>,
        style: Option<&Value>,
        message: bool,
    ) -> Result<bool> {
        let args = vec![
            host.slot(Some(e)),
            host.target_label(target),
            style.cloned().unwrap_or(Value::Null),
            json!(message),
        ];
        Ok(self.ask(host, "attack", args)?.unwrap_or(true))
    }
    fn ignore_multi_boundaries(
        &self,
        _: &Zone<Model>,
        host: &mut Model,
        attacker: &NodeHandle,
        victim: Option<&NodeHandle>,
    ) -> Result<bool> {
        Ok(self
            .ask(
                host,
                "ignoreMulti",
                vec![host.slot(Some(attacker)), host.slot(victim)],
            )?
            .unwrap_or(false))
    }
    fn teleport(
        &self,
        _: &Zone<Model>,
        host: &mut Model,
        e: &NodeHandle,
        kind: i32,
        node: Option<&Target>,
    ) -> Result<bool> {
        let args = vec![host.slot(Some(e)), json!(kind), host.target_label(node)];
        Ok(self.ask(host, "teleport", args)?.unwrap_or(true))
    }
    fn start_death(
        &self,
        _: &Zone<Model>,
        host: &mut Model,
        e: Option<&NodeHandle>,
        killer: Option<&NodeHandle>,
    ) -> Result<bool> {
        Ok(self
            .ask(host, "startDeath", vec![host.slot(e), host.slot(killer)])?
            .unwrap_or(true))
    }
    fn can_request(
        &self,
        _: &Zone<Model>,
        host: &mut Model,
        request: Option<&Value>,
        player: &NodeHandle,
        target: Option<&NodeHandle>,
    ) -> Result<bool> {
        let args = vec![
            request.cloned().unwrap_or(Value::Null),
            host.slot(Some(player)),
            host.slot(target),
        ];
        Ok(self.ask(host, "request", args)?.unwrap_or(true))
    }
    fn move_entity(
        &self,
        _: &Zone<Model>,
        host: &mut Model,
        e: &NodeHandle,
        from: Option<&Location>,
        to: Option<&Location>,
    ) -> Result<bool> {
        let args = vec![host.slot(Some(e)), coord(from), coord(to)];
        Ok(self.ask(host, "move", args)?.unwrap_or(true))
    }
    fn parse_command(
        &self,
        _: &Zone<Model>,
        host: &mut Model,
        player: Option<&NodeHandle>,
        name: Option<&str>,
        arguments: &[String],
    ) -> Result<bool> {
        let args = vec![host.slot(player), json!(name), json!(arguments)];
        Ok(self.ask(host, "command", args)?.unwrap_or(false))
    }
    fn location_update(
        &self,
        _: &Zone<Model>,
        host: &mut Model,
        e: &NodeHandle,
        last: Option<&Location>,
    ) -> Result<()> {
        self.ask(
            host,
            "locationUpdate",
            vec![host.slot(Some(e)), coord(last)],
        )?;
        Ok(())
    }
    fn configure(&self, _: &Zone<Model>, host: &mut Model) -> Result<()> {
        self.ask(host, "configure", vec![])?;
        Ok(())
    }
    fn is_dynamic_zone(&self, _: &Zone<Model>, host: &mut Model) -> Result<bool> {
        Ok(self.ask(host, "dynamic", vec![])?.unwrap_or(false))
    }
}

fn region_error(error: Error) -> region::Error {
    region::Error::Binding(name(&error))
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
    fn handle(&self, value: &Value) -> Option<NodeHandle> {
        value
            .as_i64()
            .and_then(|id| self.nodes.get(&(id as i32)))
            .map(|row| row.handle.clone())
    }
    fn node(&self, value: &Value) -> Result<NodeHandle> {
        self.handle(value).ok_or(Error::Null)
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
    fn flag(&self, handle: &NodeHandle, key: &str, fallback: bool) -> bool {
        self.attr(handle, key).as_bool().unwrap_or(fallback)
    }
    fn int(&self, handle: &NodeHandle, key: &str, fallback: i32) -> i32 {
        self.attr(handle, key)
            .as_i64()
            .map_or(fallback, |v| v as i32)
    }
    fn put(&mut self, handle: &NodeHandle, key: &str, value: Value) {
        let id = self.slot_of(handle);
        self.nodes
            .get_mut(&id)
            .unwrap()
            .state
            .insert(key.into(), value);
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
    fn raw_region(&mut self, id: i32) -> region::Handle {
        self.regions
            .entry(id)
            .or_insert_with(|| Shared::new(region::Region::new((id >> 8) & 0xFF, id & 0xFF)))
            .clone()
    }
    fn chunk(&mut self, value: &Value) -> Result<HandleChunk> {
        let location = place(value).ok_or(Error::Null)?;
        let [x, y, z] = location.coordinates();
        let region = self.raw_region(zones::region_id(&location));
        let chunks = region.read().chunks.clone().ok_or(Error::Null)?;
        let chunk = chunks.read()[((x & 63) >> 3) as usize][((y & 63) >> 3) as usize][z as usize]
            .clone()
            .ok_or(Error::Null)?;
        Ok(chunk)
    }
    fn zone(&self, value: &Value) -> Result<Zone<Model>> {
        let id = text(value).ok_or(Error::Null)?;
        self.zones
            .iter()
            .find(|(name, _)| name == id)
            .map(|(_, zone)| zone.clone())
            .ok_or(Error::Null)
    }
    fn zone_id(&self, zone: &Zone<Model>) -> Value {
        self.zones
            .iter()
            .find(|(_, candidate)| candidate.same(zone))
            .map_or(Value::Null, |(id, _)| json!(id))
    }
    fn border(&self, value: &Value) -> Result<Borders> {
        text(value)
            .and_then(|id| self.borders.get(id))
            .cloned()
            .ok_or(Error::Null)
    }
    fn music(&self, value: &Value) -> Result<MusicZone> {
        let id = text(value).ok_or(Error::Null)?;
        self.musics
            .iter()
            .find(|(name, _)| name == id)
            .map(|(_, zone)| zone.clone())
            .ok_or(Error::Null)
    }
    fn music_id(&self, zone: &MusicZone) -> Value {
        self.musics
            .iter()
            .find(|(_, candidate)| candidate.same(zone))
            .map_or(Value::Null, |(id, _)| json!(id))
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
            .map(|z| json!([self.zone_id(z.zone()), Self::describe_borders(z.borders())]))
            .collect()
    }
    fn region_info(&self, region: &region::Handle) -> Result<Value> {
        let zones = activity::region_zones::<Model>(region)?;
        let (pulse, music, music_zones) = {
            let region = region.read();
            let state = region.activity.read();
            (state.pulse.clone(), state.music, state.music_zones.values())
        };
        let (running, ticks, delay, class) = {
            let pulse = pulse.read();
            (
                pulse.running,
                pulse.ticks_passed,
                pulse.delay,
                pulse.class_name.clone(),
            )
        };
        Ok(json!({
            "active": activity::is_active(region),
            "loaded": region.read().loaded,
            "viewers": activity::viewing_players(region),
            "pulse": [running, ticks, delay],
            "pulseClass": class,
            "zones": self.describe(&zones),
            "music": music,
            "musicZones": music_zones.iter().map(|z| self.music_id(z)).collect::<Vec<_>>(),
        }))
    }
    fn random_summary(borders: &Borders, drawn: &[Location]) -> Value {
        let (sw_x, sw_y, ne_x, ne_y) = (
            borders.south_west_x(),
            borders.south_west_y(),
            borders.north_east_x(),
            borders.north_east_y(),
        );
        let mut inside = true;
        let (mut xs, mut ys, mut zs) = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
        for location in drawn {
            let [x, y, z] = location.coordinates();
            inside &= sw_x <= x && x <= ne_x && sw_y <= y && y <= ne_y;
            xs.insert(x);
            ys.insert(y);
            zs.insert(z);
        }
        let many = drawn.len() >= 96;
        json!({
            "inside": inside,
            "xs": (many && ne_x.wrapping_sub(sw_x) <= 2).then_some(xs),
            "ys": (many && ne_y.wrapping_sub(sw_y) <= 2).then_some(ys),
            "zs": zs,
        })
    }
    fn words(value: &Value) -> Vec<String> {
        value
            .as_array()
            .map(|list| {
                list.iter()
                    .map(|word| word.as_str().unwrap().to_owned())
                    .collect()
            })
            .unwrap_or_default()
    }
    fn hook(&mut self, z: &Zone<Model>, a: &Value) -> Result<Value> {
        let n = self.node(&a["node"]);
        let flag = |key: &str| a[key].as_bool().unwrap_or(false);
        Ok(match a["hook"].as_str().unwrap() {
            "enter" => json!(z.enter(self, &n?)?),
            "leave" => json!(z.leave(self, &n?, flag("logout"))?),
            "canLogout" => json!(z.can_logout(self, &n?)?),
            "death" => json!(z.death(self, &n?, self.handle(&a["killer"]).as_ref())?),
            "interact" => json!(z.interact(
                self,
                &n?,
                self.target(&a["target"]).as_ref(),
                opaque(&a["option"])
            )?),
            "useWith" => json!(z.handle_use_with(
                self,
                &n?,
                opaque(&a["used"]),
                self.target(&a["with"]).as_ref()
            )?),
            "button" => json!(z.action_button(self, &n?, 1, 2, 3, 4, 5)?),
            "pvp" => json!(z.continue_pvp(
                self,
                n.ok().as_ref(),
                self.target(&a["target"]).as_ref(),
                opaque(&a["style"]),
                flag("message")
            )?),
            "attack" => json!(z.continue_attack(
                self,
                &n?,
                self.target(&a["target"]).as_ref(),
                opaque(&a["style"]),
                flag("message")
            )?),
            "ignoreMulti" => {
                json!(z.ignore_multi_boundaries(self, &n?, self.handle(&a["victim"]).as_ref())?)
            }
            "teleport" => json!(z.teleport(
                self,
                &n?,
                a["type"].as_i64().unwrap_or(0) as i32,
                self.target(&a["target"]).as_ref()
            )?),
            "startDeath" => {
                json!(z.start_death(self, n.ok().as_ref(), self.handle(&a["killer"]).as_ref())?)
            }
            "request" => json!(z.can_request(
                self,
                opaque(&a["request"]),
                &n?,
                self.handle(&a["other"]).as_ref()
            )?),
            "move" => json!(z.move_entity(
                self,
                &n?,
                place(&a["from"]).as_ref(),
                place(&a["to"]).as_ref()
            )?),
            "command" => json!(z.parse_command(
                self,
                n.ok().as_ref(),
                text(&a["name"]),
                &Self::words(&a["args"])
            )?),
            "locationUpdate" => {
                z.location_update(self, &n?, place(&a["last"]).as_ref())?;
                Value::Null
            }
            "dynamic" => json!(z.is_dynamic_zone(self)?),
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
        let b = self.border(&a["borders"]);
        let flag = |key: &str| a[key].as_bool().unwrap_or(false);
        let entity = |model: &Model, key: &str| model.handle(&a[key]);
        Ok(match op {
            "newBorders" => {
                let v: Vec<i32> = a["args"].as_array().unwrap().iter().map(num).collect();
                let made = if let Some(check) = a.get("zeroCheck") {
                    Borders::with_plane_check(
                        v[0],
                        v[1],
                        v[2],
                        v[3],
                        v[4],
                        check.as_bool().unwrap(),
                    )
                } else if v.len() == 5 {
                    Borders::with_plane(v[0], v[1], v[2], v[3], v[4])
                } else {
                    Borders::new(v[0], v[1], v[2], v[3])
                };
                self.borders.insert(a["id"].as_str().unwrap().into(), made);
                Value::Null
            }
            "between" => {
                let made = Borders::between(
                    &place(&a["a"]).ok_or(Error::Null)?,
                    &place(&a["b"]).ok_or(Error::Null)?,
                );
                self.borders.insert(a["id"].as_str().unwrap().into(), made);
                Value::Null
            }
            "forRegion" => {
                self.borders.insert(
                    a["id"].as_str().unwrap().into(),
                    Borders::for_region(num(&a["region"])),
                );
                Value::Null
            }
            "inside" => {
                let v = a["at"].as_array().unwrap();
                json!(b?.inside(num(&v[0]), num(&v[1]), num(&v[2]))?)
            }
            "insideXY" => {
                let v = a["at"].as_array().unwrap();
                json!(b?.inside_xy(num(&v[0]), num(&v[1]))?)
            }
            "insideLocation" => json!(b?.inside_location(place(&a["location"]).as_ref())?),
            "insideNode" => json!(b?.inside_node(self, &n?)?),
            "insideRegion" => json!(b?.inside_region(self, &n?)?),
            "regionIds" => json!(b?.region_ids()?),
            "corners" => {
                let b = b?;
                let mut out = Self::describe_borders(&b);
                out.as_array_mut()
                    .unwrap()
                    .push(json!(b.exceptions().map(|list| list.len())));
                out
            }
            "addException" => {
                b?.add_exception(self.border(&a["exception"])?);
                Value::Null
            }
            "setPlane" => {
                b?.set_plane(num(&a["plane"]));
                Value::Null
            }
            "bordersEquals" => {
                let (x, y) = (self.border(&a["a"])?, self.border(&a["b"])?);
                json!([x.equals(&y)?, x.hash_code()? == y.hash_code()?])
            }
            "bordersHash" => json!(b?.hash_code()?),
            "bordersString" => json!(b?.java_string()?),
            "weighted" => {
                let location = b?.weighted_random_loc(self, num(&a["intensity"]))?;
                coord(Some(&location))
            }
            "randomLoc" => {
                let b = b?;
                let mut drawn = Vec::new();
                for _ in 0..num(&a["count"]) {
                    drawn.push(b.random_loc(self)?);
                }
                Self::random_summary(&b, &drawn)
            }
            "randomWalkable" => {
                let b = b?;
                let location = b.random_walkable_loc(self)?;
                Self::random_summary(&b, &[location])
            }
            "zoneInfo" => {
                let z = z?;
                json!([
                    z.name(),
                    z.name().map(|_| z.uid()).transpose()?,
                    z.is_overlappable(),
                    z.is_fire_randoms(),
                    z.restriction(),
                    z.zone_type(),
                    z.hash_code()
                ])
            }
            "zoneSet" => {
                let z = z?;
                if let Some(name) = a.get("name") {
                    z.set_name(name.as_str());
                }
                if let Some(uid) = a.get("uid") {
                    z.set_uid(num(uid));
                }
                if let Some(value) = a.get("overlappable") {
                    z.set_overlappable(value.as_bool().unwrap());
                }
                if let Some(kind) = a.get("type") {
                    z.set_zone_type(num(kind));
                }
                if let Some(flag) = a.get("restriction") {
                    z.add_restriction(num(flag));
                }
                if let Some(name) = a.get("restrictionName") {
                    z.add_restriction(restriction(name.as_str().unwrap()).flag());
                }
                if let Some(fire) = a.get("fire") {
                    z.set_fire_random_events(fire.as_bool().unwrap());
                }
                if a.get("disableRandoms").is_some() {
                    z.disable_random_events();
                }
                Value::Null
            }
            "zoneEquals" => json!(self.zone(&a["a"])?.equals(&self.zone(&a["b"])?)?),
            "zoneRestricted" => json!(z?.is_restricted(num(&a["flag"]))),
            "regionZone" => {
                let x = RegionZone::new(self.zone(&a["za"])?, self.border(&a["ba"])?);
                let y = RegionZone::new(self.zone(&a["zb"])?, self.border(&a["bb"])?);
                json!([x.equals(&y)?, x.hash_code()?])
            }
            "configure" => {
                z?.configure(self)?;
                Value::Null
            }
            "register" => {
                z?.register(self, &b?)?;
                Value::Null
            }
            "unregister" => {
                z?.unregister(self, &b?)?;
                Value::Null
            }
            "registerRegion" => {
                z?.register_region(self, num(&a["region"]))?;
                Value::Null
            }
            "registerRegionBorders" => {
                z?.register_region_borders(self, num(&a["region"]), &b?)?;
                Value::Null
            }
            "unregisterRegion" => {
                z?.unregister_region(self, num(&a["region"]))?;
                Value::Null
            }
            "enums" => json!([
                Restriction::ALL
                    .iter()
                    .map(|r| r.flag())
                    .collect::<Vec<_>>(),
                ZoneType::ALL.iter().map(|t| t.id()).collect::<Vec<_>>()
            ]),
            "say" => {
                zones::message(self, &n?, a["text"].as_str().unwrap())?;
                Value::Null
            }
            "checkMulti" => json!(zones::check_multi(
                self,
                &n?,
                &self.node(&a["target"])?,
                flag("message")
            )?),
            "standingIn" => {
                let v: Vec<i32> = a["args"].as_array().unwrap().iter().map(num).collect();
                json!(zone_impls::is_standing_in(
                    v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7]
                ))
            }
            "hook" => self.hook(&z?, a)?,
            "update" => json!(m?.update_location(self, place(&a["last"]).as_ref())?),
            "moveCheck" => {
                json!(m?.move_entity(self, place(&a["from"]).as_ref(), place(&a["to"]).as_ref())?)
            }
            "interact" => {
                json!(m?.interact(
                    self,
                    self.target(&a["target"]).as_ref(),
                    opaque(&a["option"])
                )?)
            }
            "canLogout" => json!(m?.can_logout(self)?),
            "teleport" => {
                json!(m?.teleport(self, num(&a["type"]), self.target(&a["target"]).as_ref())?)
            }
            "death" => json!(m?.handle_death(self, entity(self, "killer").as_ref())?),
            "startDeath" => json!(m?.start_death(
                self,
                entity(self, "entity").as_ref(),
                entity(self, "killer").as_ref()
            )?),
            "clear" => json!(m?.clear(self)?),
            "restricted" => json!(m?.is_restricted(num(&a["flag"]))?),
            "type" => json!(m?.get_type()?),
            "pvp" => json!(m?.is_pvpable(
                self,
                entity(self, "player").as_ref(),
                self.target(&a["target"]).as_ref(),
                opaque(&a["style"]),
                flag("message")
            )?),
            "continueAttack" => json!(m?.continue_attack(
                self,
                self.target(&a["target"]).as_ref(),
                opaque(&a["style"]),
                flag("message")
            )?),
            "useWith" => {
                json!(m?.use_with(self, opaque(&a["used"]), self.target(&a["with"]).as_ref())?)
            }
            "button" => json!(m?.click_button(self, 1, 2, 3, 4, 5)?),
            "ignoreMulti" => {
                json!(m?.is_ignore_multi_boundaries(self, entity(self, "victim").as_ref())?)
            }
            "canFire" => json!(m?.can_fire_random_event()?),
            "command" => json!(m?.parse_command(
                self,
                entity(self, "player").as_ref(),
                text(&a["name"]),
                &Self::words(&a["args"])
            )?),
            "canRequest" => {
                json!(m?.can_request(
                    self,
                    opaque(&a["request"]),
                    entity(self, "target").as_ref()
                )?)
            }
            "inZone" => json!(m?.is_in_zone(text(&a["name"]))?),
            "removeZone" => {
                m?.remove(&z?)?;
                Value::Null
            }
            "checkMusic" => {
                m?.check_music_zones(self)?;
                Value::Null
            }
            "teleBlocked" => json!(zones::is_tele_blocked(self, &n?)?),
            "monitorAdd" => {
                m?.zones().add(RegionZone::new(z?, b?));
                Value::Null
            }
            "monitorClear" => {
                m?.zones().clear();
                Value::Null
            }
            "musicAdd" => {
                m?.music_zones().add(self.music(&a["music"])?);
                Value::Null
            }
            "view" => {
                let region = self.raw_region(num(&a["region"]));
                let name = a["name"].as_str().unwrap();
                json!(if num(&a["delta"]) > 0 {
                    activity::increment_view_amount(&region, name)
                } else {
                    activity::decrement_view_amount(&region, name)
                })
            }
            "viewed" => json!(activity::is_viewed(&self.raw_region(num(&a["region"])))),
            "checkInactive" => {
                let region = self.raw_region(num(&a["region"]));
                json!(activity::check_inactive(self, &region)?)
            }
            "isInactive" => {
                let region = self.raw_region(num(&a["region"]));
                json!(activity::is_inactive(
                    self,
                    &region,
                    a["run"].as_bool().unwrap()
                )?)
            }
            "flagActive" => {
                let region = self.raw_region(num(&a["region"]));
                activity::flag_active(self, &region)?;
                Value::Null
            }
            "flagInactive" => {
                let region = self.raw_region(num(&a["region"]));
                json!(activity::flag_inactive(self, &region, flag("force"))?)
            }
            "unload" => {
                let region = self.raw_region(num(&a["region"]));
                json!(activity::unload(self, &region, flag("force"))?)
            }
            "timeout" => {
                activity::set_region_time_out(
                    &self.raw_region(num(&a["region"])),
                    num(&a["ticks"]),
                );
                Value::Null
            }
            "regionInfo" => {
                let region = self.raw_region(num(&a["region"]));
                self.region_info(&region)?
            }
            "newMusic" => {
                let id = a["id"].as_str().unwrap().to_owned();
                self.musics.push((id, MusicZone::new(num(&a["music"]), b?)));
                Value::Null
            }
            "musicZone" => {
                let region = self.raw_region(num(&a["region"]));
                let list = region.read().activity.read().music_zones.clone();
                list.add(self.music(&a["music"])?);
                Value::Null
            }
            "setMusic" => {
                let region = self.raw_region(num(&a["region"]));
                region.read().activity.write().music = num(&a["music"]);
                Value::Null
            }
            "musicEnter" => json!(self.music(&a["music"])?.enter(self, &n?)?),
            "tick" => {
                let runner = self.runner.clone();
                for _ in 0..a["count"].as_i64().unwrap_or(1) {
                    runner
                        .update_all(&mut (), self)
                        .map_err(|e| Error::Binding(format!("{e:?}")))?;
                }
                json!(runner.current().len())
            }
            "chunkAdd" | "chunkRemove" | "chunkClearList" => {
                let chunk = self.chunk(&a["chunk"])?;
                let list = if a["list"] == "players" {
                    chunk.read().players.clone()
                } else {
                    chunk.read().npcs.clone()
                };
                match op {
                    "chunkAdd" => {
                        list.add(n.ok());
                    }
                    "chunkRemove" => {
                        list.remove(&n.ok());
                    }
                    _ => list.clear(),
                }
                Value::Null
            }
            "chunkStuff" => {
                let chunk = self.chunk(&a["chunk"])?;
                let (flags, objects, statics) = {
                    let chunk = chunk.read();
                    (
                        chunk.update_flags.clone(),
                        chunk.objects.clone(),
                        chunk.static_objects.clone(),
                    )
                };
                flags.write().push(None);
                for layer in [objects, statics] {
                    let scenery =
                        doors::Scenery::new(1, Location::new(0, 0, 0), 10, 0, None, false)
                            .map_err(|e| Error::Binding(format!("{e:?}")))?;
                    layer.write()[1][2][0] = Some(Shared::new(scenery));
                }
                Value::Null
            }
            "set" => {
                let n = n?;
                for (key, value) in a.as_object().unwrap() {
                    if key != "op" && key != "node" {
                        self.put(&n, key, value.clone());
                    }
                }
                Value::Null
            }
            "toggle" | "home" | "modTeleport" => {
                let n = n?;
                match op {
                    "toggle" => {
                        let zone = self
                            .zones
                            .iter()
                            .find(|(_, zone)| zone.behavior::<ModeratorZone>().is_some())
                            .map(|(_, zone)| zone.clone())
                            .ok_or(Error::Null)?;
                        let moderator = zone.behavior::<ModeratorZone>().ok_or(Error::Null)?;
                        moderator.toggle(self, &n, a["on"].as_bool().unwrap())?;
                    }
                    "home" => ModeratorZone::home(self, &n)?,
                    _ => ModeratorZone::teleport_to(self, &n)?,
                }
                Value::Null
            }
            "setHome" => {
                self.home = place(&a["location"]);
                Value::Null
            }
            other => return Err(Error::Binding(format!("unknown action {other}"))),
        })
    }
}
fn restriction(name: &str) -> Restriction {
    match name {
        "FOLLOWERS" => Restriction::Followers,
        "RANDOM_EVENTS" => Restriction::RandomEvents,
        "FIRES" => Restriction::Fires,
        "MEMBERS" => Restriction::Members,
        "CANNON" => Restriction::Cannon,
        "GRAVES" => Restriction::Graves,
        "TELEPORT" => Restriction::Teleport,
        "OFF_MAP" => Restriction::OffMap,
        other => panic!("unknown restriction {other}"),
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
        // MapZone.checkMulti reads the wall clock; the corpus keeps every
        // combat time far from any plausible value of it.
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
        // The Java adapter casts its stored owner to Player before recording
        // the call; a non-player owner is a harness state, not original data.
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
    fn fresh_random_next_int(&mut self, bound: i32) -> Result<i32> {
        self.fresh.next_bound(bound).ok_or(Error::IllegalArgument)
    }
    fn teleport_permitted(&mut self, _: &Location) -> Result<bool> {
        let answers = self.input["permitted"]
            .as_array()
            .cloned()
            .unwrap_or_else(|| vec![json!(true)]);
        let answer = answers[self.permitted_calls.min(answers.len() - 1)]
            .as_bool()
            .unwrap();
        self.permitted_calls += 1;
        self.emit("teleportPermitted", vec![])?;
        Ok(answer)
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
        Ok(self.home.clone())
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

impl activity::Host for Model {
    fn submit_pulse(&mut self, pulse: &pulses::Handle) -> Result<()> {
        self.runner
            .submit(Some(pulse.clone()))
            .map_err(|e| Error::Binding(format!("{e:?}")))
    }
    fn npc_active(&mut self, npc: &NodeHandle) -> Result<bool> {
        zones::Host::active(self, npc)
    }
    fn add_renderable_npc(&mut self, npc: &NodeHandle) -> Result<()> {
        self.emit("renderable", vec![self.slot(Some(npc))])
    }
    fn npc_region_inactivity(&mut self, npc: &NodeHandle) -> Result<()> {
        self.emit("regionInactivity", vec![self.slot(Some(npc))])
    }
    fn log_error(&mut self, message: &str) -> Result<()> {
        self.emit("log", vec![json!("ERR"), json!(message)])
    }
    fn chunk_region(&mut self, region_id: i32) -> Result<region::Handle> {
        Ok(self.raw_region(region_id))
    }
}

/// Region.load's services. The case's cache holds no map archive, so the
/// load stops after its lookup; nothing else may be reached.
impl region::Context for Model {
    fn archive_id(&mut self, name: &str) -> region::Result<i32> {
        let id = self.input["archive"].as_i64().unwrap_or(-1) as i32;
        self.emit("archiveId", vec![json!(name)])
            .map_err(region_error)?;
        Ok(id)
    }
    fn mapscape_bytes(&mut self, _: i32) -> region::Result<Option<Vec<u8>>> {
        Err(region::Error::Binding(
            "map loading is outside this corpus".into(),
        ))
    }
    fn xtea(&mut self, _: i32) -> region::Result<[u32; 4]> {
        Err(region::Error::Binding(
            "map loading is outside this corpus".into(),
        ))
    }
    fn landscape_bytes(&mut self, _: i32, _: [u32; 4]) -> region::Result<Option<Vec<u8>>> {
        Err(region::Error::Binding(
            "map loading is outside this corpus".into(),
        ))
    }
    fn parse_mapscape(&mut self, _: &region::Handle, _: &Mapscape, _: &[u8]) -> region::Result<()> {
        Err(region::Error::Binding(
            "map loading is outside this corpus".into(),
        ))
    }
    fn parse_landscape(
        &mut self,
        _: &region::Handle,
        _: Option<Chunks>,
        _: &Mapscape,
        _: &[u8],
        _: bool,
    ) -> region::Result<()> {
        Err(region::Error::Binding(
            "map loading is outside this corpus".into(),
        ))
    }
    fn clip_mapscape(
        &mut self,
        _: &region::Handle,
        _: Option<Chunks>,
        _: &Mapscape,
    ) -> region::Result<()> {
        Err(region::Error::Binding(
            "map loading is outside this corpus".into(),
        ))
    }
    fn object(&mut self, _: Location, _: i32, _: i32) -> region::Result<Option<doors::Handle>> {
        Err(region::Error::Binding(
            "map loading is outside this corpus".into(),
        ))
    }
    fn remove(&mut self, _: &doors::Handle) -> region::Result<()> {
        Err(region::Error::Binding(
            "map loading is outside this corpus".into(),
        ))
    }
    fn add(&mut self, _: &doors::Handle) -> region::Result<()> {
        Err(region::Error::Binding(
            "map loading is outside this corpus".into(),
        ))
    }
    fn exception(&mut self, _: &region::Error, _: Option<i32>) -> region::Result<()> {
        self.events.push(json!(["stackTrace"]));
        Ok(())
    }
}

/// The runner's services: the only pulses of a case are activity pulses.
impl pulses::Context<()> for Model {
    fn pulse(&mut self, pulse: &pulses::Handle, _: &mut ()) -> pulses::Result<bool> {
        let region = self
            .regions
            .values()
            .find(|region| activity::activity_pulse(region).same(pulse))
            .cloned()
            .ok_or(pulses::Error::Null)?;
        activity::pulse_body(self, &region).map_err(pulse_error)
    }
    fn node_active(&mut self, _: pulses::Node, _: &mut ()) -> pulses::Result<bool> {
        Ok(true)
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

fn make(spec: &Value) -> Zone<Model> {
    let id = spec["id"].as_str().unwrap().to_owned();
    let name = match spec.get("name") {
        Some(name) => name.as_str().map(str::to_owned),
        None => Some(id.clone()),
    };
    let overlappable = spec["overlappable"].as_bool().unwrap_or(true);
    let restrictions: Vec<_> = spec["restrictions"]
        .as_array()
        .map(|names| {
            names
                .iter()
                .map(|name| restriction(name.as_str().unwrap()))
                .collect()
        })
        .unwrap_or_default();
    match spec["class"].as_str().unwrap_or("scripted") {
        "scripted" => Zone::new(
            name.as_deref(),
            overlappable,
            &restrictions,
            Scripted::<1> { id },
        ),
        "scripted2" => Zone::new(
            name.as_deref(),
            overlappable,
            &restrictions,
            Scripted::<2> { id },
        ),
        "plain" => Zone::new(name.as_deref(), overlappable, &restrictions, Plain),
        "bank" => zone_impls::bank_zone(),
        "karamja" => zone_impls::karamja_zone(),
        "moderator" => zone_impls::moderator_zone(),
        "multiway" => zone_impls::multiway_combat_zone(),
        other => panic!("unknown zone class {other}"),
    }
}

pub fn evaluate(input: &Value) -> Value {
    let mut model = Model {
        input: input.clone(),
        _state: State::default(),
        nodes: BTreeMap::new(),
        zones: Vec::new(),
        borders: HashMap::new(),
        musics: Vec::new(),
        regions: BTreeMap::new(),
        events: Vec::new(),
        calls: HashMap::new(),
        fired: BTreeSet::new(),
        permitted_calls: 0,
        runner: pulses::Runner::default(),
        random: JavaRandom::new(input["seed"].as_i64().unwrap_or(530)),
        // getRandomLoc's generators are unseeded in the original; any stream
        // will do, because only draw-independent facts are compared.
        fresh: JavaRandom::new(0x5eed),
        home: None,
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
            },
        );
    }
    for spec in input["zones"].as_array().into_iter().flatten() {
        model
            .zones
            .push((spec["id"].as_str().unwrap().to_owned(), make(spec)));
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
    let monitors: BTreeMap<String, Value> = model
        .nodes
        .iter()
        .map(|(id, row)| {
            let music: Vec<Value> = row
                .monitor
                .music_zones()
                .values()
                .iter()
                .map(|zone| model.music_id(zone))
                .collect();
            (
                id.to_string(),
                json!({"zones": model.describe(&row.monitor.zones()), "music": music}),
            )
        })
        .collect();
    let mut regions = BTreeMap::new();
    let mut chunks = BTreeMap::new();
    for (id, region) in &model.regions {
        regions.insert(id.to_string(), model.region_info(region).unwrap());
        let table = region.read().chunks.clone().unwrap();
        for x in 0..8 {
            for y in 0..8 {
                for plane in 0..4 {
                    let chunk = table.read()[x][y][plane].clone().unwrap();
                    let chunk = chunk.read();
                    let objects = [chunk.objects.clone(), chunk.static_objects.clone()]
                        .iter()
                        .map(|layer| layer.read().iter().flatten().flatten().flatten().count())
                        .sum::<usize>();
                    let (players, npcs) = (chunk.players.values(), chunk.npcs.values());
                    let flags = chunk.update_flags.read().len();
                    if players.is_empty() && npcs.is_empty() && flags == 0 && objects == 0 {
                        continue;
                    }
                    let ids = |list: &[Option<NodeHandle>]| -> Vec<Value> {
                        list.iter().map(|n| model.slot(n.as_ref())).collect()
                    };
                    chunks.insert(
                        format!("{id}:{x},{y},{plane}"),
                        json!({"players": ids(&players), "npcs": ids(&npcs), "flags": flags, "objects": objects}),
                    );
                }
            }
        }
    }
    let open = model
        .zones
        .iter()
        .find_map(|(_, zone)| zone.behavior::<ModeratorZone>().map(ModeratorZone::is_open))
        .unwrap_or(true);
    json!({
        "results": results,
        "events": model.events,
        "monitors": monitors,
        "regions": regions,
        "chunks": chunks,
        "queue": model.runner.current().len(),
        "probe": model.random.next_i32(),
        "open": open,
    })
}
