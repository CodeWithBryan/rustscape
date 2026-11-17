//! Native replay of the NPC death and drop corpus: the case's inputs become
//! the required services of `drop::Host` and `npc_death::Host`, every service
//! records its call, and the native ground manager, pulse manager and runner,
//! NPC tick state, item and NPC definitions are the real ones.
use rs09_server::{
    definitions::{
        item::{self as item_definitions, ItemDefinition, Shared},
        npc::{self as npc_definitions, NpcDefinition},
        property::Property,
    },
    inventory::item,
    pulses,
    world::{
        chunk::{HandleChunk, RegionChunk},
        chunk_sync::{self, DirtyTracker, Flag, Items},
        doors::Location,
        drop_tables::{
            ClueLevel, Context, LogLevel, Looter, NpcDropTable, NpcDropTables, PlayerLooter,
            RollError, RolledItem, WeightedItem,
            drop::{self, Error},
            random::{JavaSource, Source},
            shared::{ParseFailure, SharedTables},
        },
        ground_item as ground, ground_manager as gm, npc,
        npc_death::{self as death, Attribute, ImpactLog, Task},
        player_walk::{NodeHandle, State},
    },
};
use serde_json::{Map, Value, json};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc, sync::Arc};

type Events = Rc<RefCell<Vec<Value>>>;

fn int(value: &Value) -> i32 {
    value.as_i64().unwrap() as i32
}
fn flag(value: &Value, key: &str, fallback: bool) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(fallback)
}
fn place(value: &Value) -> Option<Location> {
    value
        .as_array()
        .map(|a| Location::new(int(&a[0]), int(&a[1]), int(&a[2])))
}
fn coordinates(location: Option<&Location>) -> Value {
    location.map_or(Value::Null, |l| json!(l.coordinates()))
}
fn pair(item: &item::Handle) -> Value {
    let item = item.read();
    json!([item.id(), item.amount()])
}
fn ground_pair(item: &ground::Handle) -> Value {
    pair(&item.read().item)
}

#[derive(Clone)]
struct Plugin {
    id: i32,
    create: bool,
    replacement: Value,
}

struct Ent {
    handle: NodeHandle,
    label: String,
    npc: bool,
    ai: bool,
    location: Option<Location>,
    spawn: Option<Location>,
    safe_respawn: Option<Location>,
    safe_zone: bool,
    zone_start: bool,
    zone_handle: bool,
    hash: i32,
    death_animation: (i32, i32),
    death_gfx: Option<i32>,
    attributes: BTreeMap<String, Value>,
    impact: Shared<ImpactLog>,
    manager: pulses::Manager,
    active: bool,
    username: Option<String>,
    uid: i32,
    clan: Option<usize>,
    loot_share: bool,
    loot_requirement: i32,
    ironman: bool,
    points: i32,
    resizable: bool,
    bone_crusher: bool,
    wealth: bool,
    bot: Option<bool>,
    online: bool,
    id: i32,
    original_id: i32,
    drop_location: Option<Option<Location>>,
    multi: bool,
    respawns: bool,
    definition: Option<npc_definitions::Handle>,
    state: npc::State,
}

/// `drop_tables::Host`: no clue is registered; the log is recorded.
struct Recorder(Events);
impl rs09_server::world::drop_tables::Host for Recorder {
    fn clue_scrolls(&self, _: ClueLevel) -> Option<&[i32]> {
        None
    }
    fn log(&mut self, origin: &'static str, level: LogLevel, message: &str) {
        let level = match level {
            LogLevel::Fine => "FINE",
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Err => "ERR",
            LogLevel::Debug => "DEBUG",
        };
        self.0
            .borrow_mut()
            .push(json!(["log", origin, level, message]));
    }
    fn stack_trace(&mut self, _: &'static str, _: ParseFailure) {}
}

struct Equipment {
    events: Events,
    label: String,
    wealth: bool,
}
impl PlayerLooter for Equipment {
    fn removes_nothings(&mut self) -> bool {
        self.events.borrow_mut().push(json!(["wealth", self.label]));
        self.wealth
    }
    fn in_equipment(&mut self, item_id: i32) -> bool {
        self.events
            .borrow_mut()
            .push(json!(["equipped", self.label, item_id]));
        false
    }
    fn update_glove_charges(&mut self) {
        self.events
            .borrow_mut()
            .push(json!(["fogCharges", self.label]));
    }
}

struct Model {
    events: Events,
    input: Value,
    tick: i32,
    players: usize,
    ents: Vec<Ent>,
    clans: Vec<i32>,
    definitions: item_definitions::Registry,
    plugins: BTreeMap<i32, Plugin>,
    source: JavaSource,
    shared: SharedTables,
    recorder: Recorder,
    chunk: HandleChunk,
    dirty: DirtyTracker,
    manager: gm::Manager,
    runner: pulses::Runner,
    tasks: Vec<Task>,
    behavior_calls: i32,
    local: Option<Vec<Option<usize>>>,
    repository: Vec<usize>,
    _actors: State,
}

fn category(error: &Error) -> String {
    match error {
        Error::Null | Error::Roll(RollError::NullLooter) => "Null".into(),
        Error::Type => "Type".into(),
        Error::Index | Error::Roll(RollError::NoClue) => "Index".into(),
        Error::Roll(RollError::BadWeight) => "BadWeight".into(),
        Error::Roll(RollError::Behavior) => "Behavior".into(),
        Error::Roll(other) => format!("{other:?}"),
        Error::Ground(gm::Error::Null | gm::Error::Ground(ground::Error::Null)) => "Null".into(),
        Error::Ground(gm::Error::Type) => "Type".into(),
        Error::Ground(other) => format!("{other:?}"),
        Error::TooManyItems => "TooManyItems".into(),
        Error::Binding(text) => text.clone(),
    }
}

impl Model {
    fn emit(&self, event: Value) {
        self.events.borrow_mut().push(event);
    }
    fn index(&self, handle: &NodeHandle) -> usize {
        self.ents
            .iter()
            .position(|ent| ent.handle == *handle)
            .expect("a node of this case")
    }
    fn ent(&self, handle: &NodeHandle) -> &Ent {
        &self.ents[self.index(handle)]
    }
    fn label(&self, handle: &NodeHandle) -> String {
        self.ent(handle).label.clone()
    }
    fn optional_label(&self, handle: Option<&NodeHandle>) -> Value {
        handle.map_or(Value::Null, |handle| json!(self.label(handle)))
    }
    /// `"p0"`/`"n1"` of the case, or nothing for null.
    fn named(&self, name: &Value) -> Option<NodeHandle> {
        let text = name.as_str()?;
        let index: usize = text[1..].parse().unwrap();
        let index = if text.starts_with('p') {
            index
        } else {
            self.players + index
        };
        Some(self.ents[index].handle.clone())
    }
    fn npc(&self, action: &Value) -> NodeHandle {
        self.ents[self.players + int(&action["npc"]) as usize]
            .handle
            .clone()
    }
    fn new_item(&mut self, id: i32, amount: i32, charge: i32) -> item::Handle {
        if !self.definitions.definitions().contains_key(&id) {
            self.definitions.insert(
                id,
                Shared::new(ItemDefinition {
                    id,
                    ..ItemDefinition::default()
                }),
            );
        }
        Shared::new(item::Item::new(id, amount, charge, &self.definitions))
    }
}

impl ground::Context for Model {
    fn copy_item(&mut self, id: i32, amount: i32, charge: i32) -> ground::Result<item::Handle> {
        Ok(self.new_item(id, amount, charge))
    }
    fn default_interaction(&mut self, _: &ground::Handle) -> ground::Result<()> {
        Ok(())
    }
    fn uid(&mut self, player: &NodeHandle) -> ground::Result<i32> {
        self.emit(json!(["uid", self.label(player)]));
        Ok(self.ent(player).uid)
    }
    fn username(&mut self, player: &NodeHandle) -> ground::Result<Option<String>> {
        self.emit(json!(["username", self.label(player)]));
        Ok(self.ent(player).username.clone())
    }
    fn ticks(&mut self) -> ground::Result<i32> {
        Ok(self.tick)
    }
}

impl gm::Context for Model {
    type Plugin = Plugin;
    fn plugin(&mut self, definition: &item_definitions::Handle) -> gm::Result<Option<Plugin>> {
        let id = definition.read().id;
        self.emit(json!(["plugin", id]));
        Ok(self.plugins.get(&id).cloned())
    }
    fn plugin_remove(
        &mut self,
        plugin: Plugin,
        player: Option<NodeHandle>,
        item: &ground::Handle,
        option: i32,
    ) -> gm::Result<()> {
        let player = self.optional_label(player.as_ref());
        self.emit(json!([
            "pluginRemove",
            plugin.id,
            player,
            ground_pair(item),
            option
        ]));
        Ok(())
    }
    fn chunk(&mut self, location: Location) -> gm::Result<HandleChunk> {
        self.emit(json!(["chunk", location.coordinates()]));
        Ok(self.chunk.clone())
    }
    fn flagged(&mut self, chunk: &HandleChunk) -> gm::Result<()> {
        self.dirty.on_flagged(chunk);
        self.emit(json!(["flagged"]));
        Ok(())
    }
    fn player_location(&mut self, player: &NodeHandle) -> gm::Result<Option<Location>> {
        self.emit(json!(["playerLocation", self.label(player)]));
        Ok(self.ent(player).location.clone())
    }
    fn playing(&mut self, _: &NodeHandle) -> gm::Result<bool> {
        Err(gm::Error::Binding("playing is outside this corpus".into()))
    }
    fn artificial(&mut self, _: &NodeHandle) -> gm::Result<bool> {
        Ok(false)
    }
    fn ai_items(&mut self, _: &NodeHandle) -> gm::Result<Option<Items>> {
        Ok(None)
    }
    fn packet(
        &mut self,
        kind: gm::Packet,
        player: Option<NodeHandle>,
        item: &ground::Handle,
        _: i32,
    ) -> gm::Result<()> {
        let kind = match kind {
            gm::Packet::Construct => "construct",
            gm::Packet::Clear => "clear",
            gm::Packet::Amount => "amount",
        };
        let player = self.optional_label(player.as_ref());
        self.emit(json!(["packet", kind, player, ground_pair(item)]));
        Ok(())
    }
}

impl drop::Host for Model {
    type Clan = usize;
    fn is_player(&mut self, entity: &NodeHandle) -> bool {
        !self.ent(entity).npc
    }
    fn is_ai_player(&mut self, entity: &NodeHandle) -> bool {
        self.ent(entity).ai
    }
    fn random(&mut self) -> &mut dyn Source {
        &mut self.source
    }
    fn roll(
        &mut self,
        table: &NpcDropTable,
        looter: Option<&NodeHandle>,
    ) -> Result<Vec<RolledItem>, RollError> {
        let mut equipment = looter.map(|looter| {
            let ent = self.ent(looter);
            (
                ent.npc,
                Equipment {
                    events: self.events.clone(),
                    label: ent.label.clone(),
                    wealth: ent.wealth,
                },
            )
        });
        let receiver = match equipment.as_mut() {
            None => Looter::None,
            Some((true, _)) => Looter::Entity,
            Some((false, player)) => Looter::Player(player),
        };
        let mut context = Context::new(&mut self.source, &mut self.shared, &mut self.recorder);
        table.roll(&mut context, receiver, 1)
    }
    fn manager(&mut self) -> gm::Manager {
        self.manager.clone()
    }
    fn on_drop_table_rolled(
        &mut self,
        npc: &NodeHandle,
        looter: &NodeHandle,
        drops: &mut Vec<RolledItem>,
    ) -> drop::Result<()> {
        // The original list holds items, whose id is the low 16 bits.
        let list: Vec<_> = drops
            .iter()
            .map(|drop| json!([drop.id & 0xffff, drop.amount]))
            .collect();
        self.emit(json!(["rolled", self.label(npc), self.label(looter), list]));
        self.behavior_calls += 1;
        let behavior = self.input["behavior"].clone();
        if behavior.is_null() {
            return Ok(());
        }
        if behavior["failAt"].as_i64() == Some(i64::from(self.behavior_calls)) {
            return Err(Error::Roll(RollError::Behavior));
        }
        if flag(&behavior, "removeFirst", false) && !drops.is_empty() {
            drops.remove(0);
        }
        if let Some(add) = behavior["add"].as_array() {
            drops.push(RolledItem {
                id: int(&add[0]),
                amount: int(&add[1]),
            });
        }
        Ok(())
    }
    fn npc_id(&mut self, npc: &NodeHandle) -> drop::Result<i32> {
        Ok(self.ent(npc).id)
    }
    fn npc_drop_location(&mut self, npc: &NodeHandle) -> drop::Result<Option<Location>> {
        // The base method is getLocation(); a subclass answers by itself.
        match self.ent(npc).drop_location.clone() {
            Some(location) => Ok(location),
            None => self.npc_location(npc),
        }
    }
    fn npc_location(&mut self, npc: &NodeHandle) -> drop::Result<Option<Location>> {
        self.emit(json!(["npcLocation", self.label(npc)]));
        Ok(self.ent(npc).location.clone())
    }
    fn npc_multi_zone(&mut self, npc: &NodeHandle) -> drop::Result<bool> {
        self.emit(json!(["multiZone", self.label(npc)]));
        Ok(self.ent(npc).multi)
    }
    fn plugin_create_drop(
        &mut self,
        plugin: Plugin,
        item: &item::Handle,
        player: &NodeHandle,
        npc: &NodeHandle,
        location: &Location,
    ) -> drop::Result<bool> {
        self.emit(json!([
            "pluginCreateDrop",
            plugin.id,
            pair(item),
            self.label(player),
            self.label(npc),
            location.coordinates()
        ]));
        Ok(plugin.create)
    }
    fn plugin_get_item(
        &mut self,
        plugin: Plugin,
        item: &item::Handle,
        npc: &NodeHandle,
    ) -> drop::Result<Option<item::Handle>> {
        self.emit(json!([
            "pluginGetItem",
            plugin.id,
            pair(item),
            self.label(npc)
        ]));
        Ok(match &plugin.replacement {
            Value::Null => None,
            Value::String(_) => Some(item.clone()),
            other => Some(self.new_item(int(&other[0]), int(&other[1]), 1000)),
        })
    }
    fn send_message(&mut self, player: &NodeHandle, message: &str) -> drop::Result<()> {
        self.emit(json!(["message", self.label(player), message]));
        Ok(())
    }
    fn resizable(&mut self, player: &NodeHandle) -> drop::Result<bool> {
        self.emit(json!(["resizable", self.label(player)]));
        Ok(self.ent(player).resizable)
    }
    fn bone_crusher_enabled(&mut self, player: &NodeHandle) -> drop::Result<bool> {
        self.emit(json!(["boneCrusher", self.label(player)]));
        Ok(self.ent(player).bone_crusher)
    }
    fn add_experience(
        &mut self,
        player: &NodeHandle,
        skill: i32,
        experience: f64,
    ) -> drop::Result<()> {
        let bits = (experience.to_bits() as i64).to_string();
        self.emit(json!(["xp", self.label(player), skill, bits]));
        Ok(())
    }
    fn clan(&mut self, player: &NodeHandle) -> drop::Result<Option<usize>> {
        self.emit(json!(["clan", self.label(player)]));
        Ok(self.ent(player).clan)
    }
    fn clan_loot_requirement(&mut self, clan: &usize) -> drop::Result<i32> {
        self.emit(json!(["clanReq", clan]));
        Ok(self.clans[*clan])
    }
    fn loot_share(&mut self, player: &NodeHandle) -> drop::Result<bool> {
        self.emit(json!(["lootShare", self.label(player)]));
        Ok(self.ent(player).loot_share)
    }
    fn loot_requirement(&mut self, player: &NodeHandle) -> drop::Result<i32> {
        self.emit(json!(["lootReq", self.label(player)]));
        Ok(self.ent(player).loot_requirement)
    }
    fn ironman(&mut self, player: &NodeHandle) -> drop::Result<bool> {
        self.emit(json!(["ironman", self.label(player)]));
        Ok(self.ent(player).ironman)
    }
    fn loot_share_points(&mut self, player: &NodeHandle) -> drop::Result<i32> {
        self.emit(json!(["lsp", self.label(player)]));
        Ok(self.ent(player).points)
    }
    fn set_loot_share_points(&mut self, player: &NodeHandle, points: i32) -> drop::Result<()> {
        self.emit(json!(["setLsp", self.label(player), points]));
        let index = self.index(player);
        self.ents[index].points = points;
        Ok(())
    }
    fn local_players(
        &mut self,
        location: Option<&Location>,
        distance: i32,
    ) -> drop::Result<Vec<Option<NodeHandle>>> {
        self.emit(json!(["localPlayers", coordinates(location), distance]));
        let local = self.local.clone().ok_or(Error::Null)?;
        Ok(local
            .into_iter()
            .map(|index| index.map(|index| self.ents[index].handle.clone()))
            .collect())
    }
    fn recommended_price(&mut self, item_id: i32, from: bool) -> drop::Result<i32> {
        self.emit(json!(["price", item_id, from]));
        Ok(self.input["prices"].get(item_id.to_string()).map_or(0, int))
    }
    fn repository_players(&mut self) -> drop::Result<Vec<NodeHandle>> {
        self.emit(json!(["players"]));
        Ok(self
            .repository
            .iter()
            .map(|index| self.ents[*index].handle.clone())
            .collect())
    }
    fn send_news(&mut self, message: &str) -> drop::Result<()> {
        self.emit(json!(["news", message]));
        Ok(())
    }
    fn increment_rare_drop(
        &mut self,
        player: &NodeHandle,
        item: &item::Handle,
    ) -> drop::Result<()> {
        self.emit(json!(["rareDrop", self.label(player), pair(item)]));
        Ok(())
    }
    fn ai_add_item(&mut self, item: &ground::Handle) -> drop::Result<()> {
        self.emit(json!(["aiAdd", ground_pair(item)]));
        Ok(())
    }
    fn bot_script_running(&mut self, player: &NodeHandle) -> drop::Result<bool> {
        self.emit(json!(["botScript", self.label(player)]));
        Ok(self.ent(player).bot == Some(true))
    }
    fn bot_add_drop(&mut self, player: &NodeHandle, item: &ground::Handle) -> drop::Result<()> {
        self.emit(json!(["botDrop", self.label(player), ground_pair(item)]));
        Ok(())
    }
}

impl death::Host for Model {
    fn is_npc(&mut self, entity: &NodeHandle) -> bool {
        self.ent(entity).npc
    }
    fn identity_hash(&mut self, entity: &NodeHandle) -> i32 {
        self.ent(entity).hash
    }
    fn impact_log(&mut self, entity: &NodeHandle) -> drop::Result<Shared<ImpactLog>> {
        Ok(self.ent(entity).impact.clone())
    }
    fn player_by_uid(&mut self, uid: i32) -> drop::Result<Option<NodeHandle>> {
        self.emit(json!(["playerByUid", uid]));
        Ok(self.ents[..self.players]
            .iter()
            .rev()
            .find(|ent| ent.online && ent.uid == uid)
            .map(|ent| ent.handle.clone()))
    }
    fn set_attribute(
        &mut self,
        entity: &NodeHandle,
        key: &str,
        value: Attribute,
    ) -> drop::Result<()> {
        let value = match value {
            Attribute::Bool(value) => json!(value),
            Attribute::Int(value) => json!(value),
        };
        let index = self.index(entity);
        self.ents[index]
            .attributes
            .insert(key.into(), value.clone());
        self.emit(json!(["setAttribute", self.label(entity), key, value]));
        Ok(())
    }
    fn remove_attribute(&mut self, entity: &NodeHandle, key: &str) -> drop::Result<()> {
        let index = self.index(entity);
        self.ents[index].attributes.remove(key);
        self.emit(json!(["removeAttribute", self.label(entity), key]));
        Ok(())
    }
    fn attribute_bool(
        &mut self,
        entity: &NodeHandle,
        key: &str,
        fallback: bool,
    ) -> drop::Result<bool> {
        match self.ent(entity).attributes.get(key) {
            None | Some(Value::Null) => Ok(fallback),
            Some(Value::Bool(value)) => Ok(*value),
            Some(_) => Err(Error::Type),
        }
    }
    fn attribute_int(
        &mut self,
        entity: &NodeHandle,
        key: &str,
        fallback: i32,
    ) -> drop::Result<i32> {
        match self.ent(entity).attributes.get(key) {
            None | Some(Value::Null) => Ok(fallback),
            Some(Value::Number(value)) => Ok(value.as_i64().unwrap() as i32),
            Some(_) => Err(Error::Type),
        }
    }
    fn increment_attribute(&mut self, player: &NodeHandle, key: &str) -> drop::Result<()> {
        self.emit(json!(["incrementAttribute", self.label(player), key]));
        Ok(())
    }
    fn run_strong(&mut self, task: &Task) -> drop::Result<()> {
        self.tasks.push(task.clone());
        let manager = self.ent(&task.entity).manager.clone();
        let runner = self.runner.clone();
        manager
            .run(
                Some(&task.pulse),
                Some(pulses::Kind::Strong),
                &runner,
                self,
                &mut Dispatch,
            )
            .map_err(from_pulse)
    }
    fn stop_combat_pulse(&mut self, entity: &NodeHandle) -> drop::Result<()> {
        self.emit(json!(["stopCombat", self.label(entity)]));
        Ok(())
    }
    fn zone_start_death(
        &mut self,
        entity: &NodeHandle,
        killer: Option<&NodeHandle>,
    ) -> drop::Result<bool> {
        self.emit(json!([
            "zoneStartDeath",
            self.label(entity),
            self.optional_label(killer)
        ]));
        Ok(self.ent(entity).zone_start)
    }
    fn zone_handle_death(
        &mut self,
        entity: &NodeHandle,
        killer: &NodeHandle,
    ) -> drop::Result<bool> {
        self.emit(json!([
            "zoneHandleDeath",
            self.label(entity),
            self.label(killer)
        ]));
        Ok(self.ent(entity).zone_handle)
    }
    fn walking_reset(&mut self, entity: &NodeHandle) -> drop::Result<()> {
        self.emit(json!(["walkReset", self.label(entity)]));
        Ok(())
    }
    fn lock(&mut self, entity: &NodeHandle, ticks: i32) -> drop::Result<()> {
        self.emit(json!(["lock", self.label(entity), ticks]));
        Ok(())
    }
    fn unlock(&mut self, entity: &NodeHandle) -> drop::Result<()> {
        self.emit(json!(["unlock", self.label(entity)]));
        Ok(())
    }
    fn face(&mut self, entity: &NodeHandle, target: Option<&NodeHandle>) -> drop::Result<()> {
        self.emit(json!([
            "face",
            self.label(entity),
            self.optional_label(target)
        ]));
        Ok(())
    }
    fn play_global_audio(&mut self, location: Option<&Location>, id: i32) -> drop::Result<()> {
        // The original's parameter is non-null.
        let location = location.ok_or(Error::Null)?;
        self.emit(json!(["globalAudio", location.coordinates(), id]));
        Ok(())
    }
    fn reset_graphics(&mut self, entity: &NodeHandle) -> drop::Result<()> {
        self.emit(json!(["graphics", self.label(entity), -1]));
        Ok(())
    }
    fn visualize_death(&mut self, entity: &NodeHandle) -> drop::Result<()> {
        let ent = self.ent(entity);
        self.emit(json!([
            "visualize",
            ent.label,
            ent.death_animation.0,
            ent.death_gfx
        ]));
        Ok(())
    }
    fn force_death_animation(&mut self, entity: &NodeHandle) -> drop::Result<()> {
        let ent = self.ent(entity);
        self.emit(json!(["forceAnimation", ent.label, ent.death_animation.0]));
        Ok(())
    }
    fn force_reset_animation(&mut self, entity: &NodeHandle) -> drop::Result<()> {
        self.emit(json!(["forceAnimation", self.label(entity), -1]));
        Ok(())
    }
    fn death_animation_duration(&mut self, entity: &NodeHandle) -> drop::Result<i32> {
        Ok(self.ent(entity).death_animation.1)
    }
    fn respawn_location(&mut self, entity: &NodeHandle) -> drop::Result<Option<Location>> {
        let ent = self.ent(entity);
        Ok(if ent.safe_zone {
            ent.safe_respawn.clone()
        } else {
            ent.spawn.clone()
        })
    }
    fn set_teleport_location(
        &mut self,
        entity: &NodeHandle,
        location: Option<&Location>,
    ) -> drop::Result<()> {
        self.emit(json!([
            "teleportLocation",
            self.label(entity),
            coordinates(location)
        ]));
        Ok(())
    }
    fn dispatch_self_death(
        &mut self,
        entity: &NodeHandle,
        killer: &NodeHandle,
    ) -> drop::Result<()> {
        self.emit(json!([
            "dispatch",
            self.label(entity),
            "SelfDeathEvent",
            self.label(killer)
        ]));
        Ok(())
    }
    fn player_commence_death(
        &mut self,
        player: &NodeHandle,
        killer: &NodeHandle,
    ) -> drop::Result<()> {
        self.emit(json!([
            "playerCommenceDeath",
            self.label(player),
            self.label(killer)
        ]));
        Ok(())
    }
    fn player_finalize_death(
        &mut self,
        player: &NodeHandle,
        killer: &NodeHandle,
    ) -> drop::Result<()> {
        self.emit(json!([
            "playerFinalizeDeath",
            self.label(player),
            self.label(killer)
        ]));
        Ok(())
    }
    fn skills_restore(&mut self, entity: &NodeHandle) -> drop::Result<()> {
        self.emit(json!(["restore", self.label(entity)]));
        Ok(())
    }
    fn recharge_prayer(&mut self, entity: &NodeHandle) -> drop::Result<()> {
        self.emit(json!(["rechargePrayer", self.label(entity)]));
        Ok(())
    }
    fn clear_impact_queue(&mut self, entity: &NodeHandle) -> drop::Result<()> {
        self.emit(json!(["clearImpactQueue", self.label(entity)]));
        Ok(())
    }
    fn timers_on_death(&mut self, entity: &NodeHandle) -> drop::Result<()> {
        self.emit(json!(["timersDeath", self.label(entity)]));
        Ok(())
    }
    fn transform(&mut self, npc: &NodeHandle, id: i32) -> drop::Result<()> {
        self.emit(json!(["transform", self.label(npc), id]));
        Ok(())
    }
    fn npc_state(&mut self, npc: &NodeHandle) -> drop::Result<npc::State> {
        Ok(self.ent(npc).state.clone())
    }
    fn npc_definition(
        &mut self,
        npc: &NodeHandle,
    ) -> drop::Result<Option<npc_definitions::Handle>> {
        Ok(self.ent(npc).definition.clone())
    }
    fn npc_original_id(&mut self, npc: &NodeHandle) -> drop::Result<i32> {
        Ok(self.ent(npc).original_id)
    }
    fn npc_respawns(&mut self, npc: &NodeHandle) -> drop::Result<bool> {
        Ok(self.ent(npc).respawns)
    }
    fn npc_clear(&mut self, npc: &NodeHandle) -> drop::Result<()> {
        self.emit(json!(["clear", self.label(npc)]));
        let index = self.index(npc);
        self.ents[index].active = false;
        Ok(())
    }
    fn on_death_started(&mut self, npc: &NodeHandle, killer: &NodeHandle) -> drop::Result<()> {
        self.emit(json!([
            "onDeathStarted",
            self.label(npc),
            self.label(killer)
        ]));
        Ok(())
    }
    fn on_death_finished(&mut self, npc: &NodeHandle, killer: &NodeHandle) -> drop::Result<()> {
        self.emit(json!([
            "onDeathFinished",
            self.label(npc),
            self.label(killer)
        ]));
        Ok(())
    }
    fn increment_kills(&mut self, player: &NodeHandle, npc_id: i32) -> drop::Result<()> {
        self.emit(json!(["incrementKills", self.label(player), npc_id]));
        Ok(())
    }
    fn dispatch_npc_kill(&mut self, killer: &NodeHandle, npc: &NodeHandle) -> drop::Result<()> {
        self.emit(json!([
            "dispatch",
            self.label(killer),
            "NPCKillEvent",
            self.label(npc)
        ]));
        Ok(())
    }
}

fn from_pulse(error: pulses::Error) -> Error {
    match error {
        pulses::Error::Binding(text) => Error::Binding(text),
        other => Error::Binding(format!("{other:?}")),
    }
}
fn to_pulse(error: Error) -> pulses::Error {
    pulses::Error::Binding(category(&error))
}

/// The pulse runner's virtual dispatch: a death task's pulse goes to the
/// native task functions, every other pulse of a case is the bare base class.
struct Dispatch;
impl Dispatch {
    fn task(pulse: &pulses::Handle, state: &Model) -> Option<Task> {
        state
            .tasks
            .iter()
            .find(|task| task.pulse.same(pulse))
            .cloned()
    }
}
impl pulses::Context<Model> for Dispatch {
    fn pulse(&mut self, pulse: &pulses::Handle, state: &mut Model) -> pulses::Result<bool> {
        match Self::task(pulse, state) {
            Some(task) => death::task_pulse(state, &task).map_err(to_pulse),
            None => Ok(true),
        }
    }
    fn node_active(&mut self, node: pulses::Node, state: &mut Model) -> pulses::Result<bool> {
        Ok(state
            .ents
            .iter()
            .find(|ent| ent.handle.node == node)
            .is_some_and(|ent| ent.active))
    }
    fn exception(&mut self, error: &pulses::Error, state: &mut Model) -> pulses::Result<()> {
        let text = match error {
            pulses::Error::Binding(text) => text.clone(),
            other => format!("{other:?}"),
        };
        state.emit(json!(["stackTrace", text]));
        Ok(())
    }
    fn start(&mut self, pulse: &pulses::Handle, state: &mut Model) -> pulses::Result<()> {
        match Self::task(pulse, state) {
            Some(task) => death::task_start(state, &task).map_err(to_pulse),
            None => {
                pulses::start_base(pulse);
                Ok(())
            }
        }
    }
    fn stop(&mut self, pulse: &pulses::Handle, state: &mut Model) -> pulses::Result<()> {
        match Self::task(pulse, state) {
            Some(task) => death::task_stop(state, &task).map_err(to_pulse),
            None => {
                pulses::stop_base(pulse);
                Ok(())
            }
        }
    }
    fn remove_for(
        &mut self,
        pulse: &pulses::Handle,
        _: Option<&str>,
        state: &mut Model,
    ) -> pulses::Result<bool> {
        Ok(Self::task(pulse, state).is_none() || death::TASK_REMOVE_FOR)
    }
    fn remove_weak_scripts(&mut self, _: &mut Model) -> pulses::Result<()> {
        Ok(())
    }
    fn remove_normal_scripts(&mut self, _: &mut Model) -> pulses::Result<()> {
        Ok(())
    }
    fn is_dead(&mut self, _: &mut Model) -> pulses::Result<bool> {
        Err(pulses::Error::Binding(
            "cancelDeathTask is outside this corpus".into(),
        ))
    }
    fn send_message(&mut self, _: pulses::Node, _: &str, _: &mut Model) -> pulses::Result<()> {
        Err(pulses::Error::Binding(
            "unhandled actions are outside this corpus".into(),
        ))
    }
    fn now_millis(&mut self, _: &mut Model) -> pulses::Result<i64> {
        Ok(0)
    }
    fn log(
        &mut self,
        level: pulses::Level,
        message: String,
        state: &mut Model,
    ) -> pulses::Result<()> {
        let level = match level {
            pulses::Level::Warn => "WARN",
            pulses::Level::Error => "ERR",
        };
        state.emit(json!(["log", "PulseRunner", level, message]));
        Ok(())
    }
    fn pulse_length(&mut self, _: &str, _: i32, _: &mut Model) -> pulses::Result<()> {
        Ok(())
    }
    fn count_pulse(&mut self, _: &str, _: &mut Model) -> pulses::Result<()> {
        Ok(())
    }
    fn metrics_enabled(&self, _: &Model) -> bool {
        false
    }
    fn set_bot_total(&mut self, _: i32, _: &mut Model) -> pulses::Result<()> {
        Ok(())
    }
    fn set_other_total(&mut self, _: i32, _: &mut Model) -> pulses::Result<()> {
        Ok(())
    }
}

/// The services of one NPC's tick that this corpus reaches: the respawn head
/// of `NPC.tick`, then recorded stand-ins for its automatic actions.
struct NpcTick<'a> {
    events: Events,
    label: &'a str,
    tick: i32,
}
impl npc::Context for NpcTick<'_> {
    fn location(&self, state: &npc::State) -> Option<Location> {
        self.events
            .borrow_mut()
            .push(json!(["npcLocation", self.label]));
        state.fields.read().location.clone()
    }
    fn ticks(&mut self, _: &npc::State) -> npc::Result<i32> {
        Ok(self.tick)
    }
    fn region_active(&mut self, _: &npc::State, _: Location) -> npc::Result<bool> {
        Ok(true)
    }
    fn query(&mut self, _: &npc::State, query: npc::Query) -> npc::Result<bool> {
        match query {
            npc::Query::Behavior => {
                self.events
                    .borrow_mut()
                    .push(json!(["tickActions", self.label]));
                Ok(false)
            }
            other => Err(npc::Error::Binding(format!(
                "{other:?} is outside this corpus"
            ))),
        }
    }
    fn event(&mut self, _: &npc::State, event: npc::Event) -> npc::Result<()> {
        let event = match event {
            npc::Event::Teleport(location) => {
                json!(["teleport", self.label, coordinates(location.as_ref())])
            }
            npc::Event::BehaviorRespawn => json!(["onRespawn", self.label]),
            npc::Event::Respawn => return Ok(()),
            npc::Event::Restore => json!(["fullRestore", self.label]),
            npc::Event::EntityTick => json!(["entityTick", self.label]),
            npc::Event::Inactivity => json!(["inactivity", self.label]),
            other => {
                return Err(npc::Error::Binding(format!(
                    "{other:?} is outside this corpus"
                )));
            }
        };
        self.events.borrow_mut().push(event);
        Ok(())
    }
    fn pulse_running(&mut self, _: &npc::State, _: usize) -> npc::Result<bool> {
        Err(npc::Error::Binding("outside this corpus".into()))
    }
    fn return_pulse(&mut self, _: &npc::State, _: Option<Location>) -> npc::Result<usize> {
        Err(npc::Error::Binding("outside this corpus".into()))
    }
    fn dialogue_active(&mut self, _: &npc::State, _: usize) -> npc::Result<bool> {
        Err(npc::Error::Binding("outside this corpus".into()))
    }
    fn dialogue_chatbox(&mut self, _: &npc::State, _: usize) -> npc::Result<bool> {
        Err(npc::Error::Binding("outside this corpus".into()))
    }
    fn random_bool(&mut self, _: &npc::State) -> npc::Result<bool> {
        Err(npc::Error::Binding("outside this corpus".into()))
    }
    fn randomize(&mut self, _: &npc::State, _: i32) -> npc::Result<i32> {
        Err(npc::Error::Binding("outside this corpus".into()))
    }
    fn random_range(&mut self, _: &npc::State, _: i32, _: i32) -> npc::Result<i32> {
        Err(npc::Error::Binding("outside this corpus".into()))
    }
    fn can_move(&mut self, _: &npc::State, _: Option<Location>) -> npc::Result<bool> {
        Err(npc::Error::Binding("outside this corpus".into()))
    }
    fn walk(&mut self, _: &npc::State, _: Option<Location>, _: bool) -> npc::Result<()> {
        Err(npc::Error::Binding("outside this corpus".into()))
    }
}

fn property(value: &Value) -> Property {
    match value {
        Value::Null => Property::Null,
        Value::Bool(value) => Property::Bool(*value),
        Value::Number(value) => Property::Int(value.as_i64().unwrap() as i32),
        Value::String(value) => Property::Text(value.clone()),
        Value::Array(values) => Property::IntArray(Shared::new(values.iter().map(int).collect())),
        Value::Object(_) => unreachable!(),
    }
}

fn build(input: &Value) -> Model {
    let events: Events = Rc::default();
    let seed = |name: &str, fallback: i64| input["seeds"][name].as_i64().unwrap_or(fallback);
    let mut source = JavaSource::new(seed("random", 530), seed("shuffle", 531), seed("tlr", 532));
    let mut definitions = item_definitions::Registry::default();
    let mut plugins = BTreeMap::new();
    let empty = Map::new();
    for (id, fields) in input["items"].as_object().unwrap_or(&empty) {
        let id: i32 = id.parse().unwrap();
        let mut definition = ItemDefinition {
            id,
            stackable: flag(fields, "stackable", false),
            unnoted: flag(fields, "unnoted", true),
            note_id: fields.get("noteId").map_or(-1, int),
            ..ItemDefinition::default()
        };
        if let Some(name) = fields.get("name") {
            definition.name = name.as_str().map(str::to_owned);
        }
        if let Some(options) = fields.get("options") {
            definition.options = options.as_array().map(|options| {
                Shared::new(
                    options
                        .iter()
                        .map(|option| option.as_str().map(str::to_owned))
                        .collect(),
                )
            });
        }
        for key in ["tradeable", "rare_item"] {
            if let Some(value) = fields.get(key) {
                definition.handlers.insert(key.into(), property(value));
            }
        }
        if let Some(plugin) = fields.get("plugin").filter(|plugin| !plugin.is_null()) {
            plugins.insert(
                id,
                Plugin {
                    id: int(&plugin["id"]),
                    create: flag(plugin, "create", true),
                    replacement: plugin["item"].clone(),
                },
            );
        }
        definitions.insert(id, Shared::new(definition));
    }

    let clans: Vec<i32> = input["clans"]
        .as_array()
        .map(|clans| clans.iter().map(int).collect())
        .unwrap_or_default();
    let mut actors = State::default();
    let mut ents = Vec::new();
    let none = Vec::new();
    let default_player = Location::new(3200, 3200, 0);
    let default_npc = Location::new(3210, 3210, 0);
    let location_of = |fields: &Value, key: &str, fallback: &Location| match fields.get(key) {
        Some(value) => place(value),
        None => Some(fallback.clone()),
    };
    let animation = |fields: &Value| {
        fields
            .get("deathAnim")
            .and_then(Value::as_array)
            .map_or((836, 4), |animation| {
                (int(&animation[0]), int(&animation[1]))
            })
    };
    let player_rows = input["players"].as_array().unwrap_or(&none);
    for (index, fields) in player_rows.iter().enumerate() {
        let location = location_of(fields, "location", &default_player);
        let node = actors.add(index as i32 + 1, location.clone().unwrap(), [1, 2, 3, 4]);
        ents.push(Ent {
            handle: actors.retain_node(node).unwrap(),
            label: format!("p{index}"),
            npc: false,
            ai: flag(fields, "ai", false),
            location,
            spawn: location_of(fields, "spawn", &Location::new(3222, 3218, 0)),
            safe_respawn: fields.get("safeRespawn").and_then(place),
            safe_zone: flag(fields, "safeZone", false),
            zone_start: flag(fields, "zoneStart", true),
            zone_handle: false,
            hash: fields.get("hash").map_or(index as i32 + 1, int),
            death_animation: animation(fields),
            death_gfx: None,
            attributes: BTreeMap::new(),
            impact: Shared::new(ImpactLog::default()),
            manager: pulses::Manager::new([1, 2, 3, 4]),
            active: true,
            username: match fields.get("name") {
                Some(name) => name.as_str().map(str::to_owned),
                None => Some(format!("Player{index}")),
            },
            uid: fields.get("uid").map_or(100 + index as i32, int),
            clan: fields
                .get("clan")
                .and_then(Value::as_u64)
                .map(|clan| clan as usize),
            loot_share: flag(fields, "lootShare", false),
            loot_requirement: fields.get("lootReq").map_or(0, int),
            ironman: flag(fields, "ironman", false),
            points: fields.get("lsp").map_or(0, int),
            resizable: flag(fields, "resizable", false),
            bone_crusher: flag(fields, "boneCrusher", false),
            wealth: flag(fields, "wealth", false),
            bot: fields.get("bot").and_then(Value::as_bool),
            online: flag(fields, "online", true),
            id: 0,
            original_id: 0,
            drop_location: None,
            multi: false,
            respawns: true,
            definition: None,
            state: npc::State::new(default_player.clone()),
        });
    }
    let players = ents.len();
    for (index, fields) in input["npcs"].as_array().unwrap_or(&none).iter().enumerate() {
        let id = int(&fields["id"]);
        let location = location_of(fields, "location", &default_npc);
        let spawn = match fields.get("spawn") {
            Some(spawn) => place(spawn),
            None => location.clone(),
        };
        let node = actors.add_npc(index as i32 + 1, location.clone().unwrap(), [1, 2, 3, 4]);
        let state = npc::State::new(location.clone().unwrap());
        state.fields.write().spawn = spawn.clone();
        let definition = flag(fields, "definition", true).then(|| {
            let mut definition = NpcDefinition {
                id,
                ..NpcDefinition::default()
            };
            definition.name = fields["name"].as_str().map(str::to_owned);
            for (key, value) in fields["config"].as_object().unwrap_or(&empty) {
                definition
                    .handlers
                    .write()
                    .insert(key.clone(), property(value));
            }
            match &fields["tables"] {
                Value::String(_) => definition.drop_tables = None,
                Value::Array(adds) => {
                    let mut table = NpcDropTable::new();
                    for add in adds {
                        let weighted = WeightedItem::new(
                            int(&add[1]),
                            int(&add[2]),
                            int(&add[3]),
                            f64::from_bits(add[4].as_i64().unwrap() as u64),
                            add[5].as_bool().unwrap(),
                        );
                        match add[0].as_str().unwrap() {
                            "charm" => table.add_to_charms(&mut source, weighted),
                            "tertiary" => table.add_to_tertiary(&mut source, weighted),
                            _ => table.add(&mut source, weighted),
                        };
                    }
                    let mut tables = NpcDropTables::new();
                    tables.table = Arc::new(table);
                    definition.drop_tables = Some(tables);
                }
                _ => {}
            }
            Shared::new(definition)
        });
        let mut attributes = BTreeMap::new();
        if flag(fields, "disableDrop", false) {
            attributes.insert(death::DISABLE_DROP.to_owned(), json!(true));
        }
        ents.push(Ent {
            handle: actors.retain_node(node).unwrap(),
            label: format!("n{index}"),
            npc: true,
            ai: false,
            location,
            spawn,
            safe_respawn: fields.get("safeRespawn").and_then(place),
            safe_zone: flag(fields, "safeZone", false),
            zone_start: flag(fields, "zoneStart", true),
            zone_handle: flag(fields, "zoneHandle", false),
            hash: fields.get("hash").map_or((players + index) as i32 + 1, int),
            death_animation: animation(fields),
            death_gfx: fields
                .get("deathGfx")
                .and_then(Value::as_i64)
                .map(|id| id as i32),
            attributes,
            impact: Shared::new(ImpactLog::default()),
            manager: pulses::Manager::new([1, 2, 3, 4]),
            active: true,
            username: None,
            uid: 0,
            clan: None,
            loot_share: false,
            loot_requirement: 0,
            ironman: false,
            points: 0,
            resizable: false,
            bone_crusher: false,
            wealth: false,
            bot: None,
            online: false,
            id,
            original_id: fields.get("originalId").map_or(id, int),
            drop_location: fields.get("dropLocation").map(place),
            multi: flag(fields, "multi", false),
            respawns: flag(fields, "respawn", true),
            definition,
            state,
        });
    }
    let indices = |value: &Value| {
        value.as_array().map(|list| {
            list.iter()
                .map(|index| index.as_u64().map(|index| index as usize))
                .collect::<Vec<_>>()
        })
    };
    Model {
        recorder: Recorder(events.clone()),
        events,
        input: input.clone(),
        tick: input.get("tick").map_or(1000, int),
        players,
        clans,
        definitions,
        plugins,
        source,
        shared: SharedTables::new(),
        chunk: Shared::new(RegionChunk::new(Location::new(3208, 3208, 0), 0)),
        dirty: DirtyTracker::default(),
        manager: gm::Manager::default(),
        runner: pulses::Runner::default(),
        tasks: Vec::new(),
        behavior_calls: 0,
        local: indices(&input["local"]),
        repository: indices(&input["repository"])
            .map(|list| list.into_iter().flatten().collect())
            .unwrap_or_else(|| (0..players).collect()),
        ents,
        _actors: actors,
    }
}

impl Model {
    fn act(&mut self, a: &Value) -> drop::Result<Value> {
        match a["op"].as_str().unwrap() {
            "bones" => Ok(json!(
                drop::BONES
                    .iter()
                    .enumerate()
                    .map(|(index, (id, experience))| {
                        let last = drop::BONES.iter().rposition(|(other, _)| other == id);
                        json!([
                            id,
                            (experience.to_bits() as i64).to_string(),
                            last == Some(index)
                        ])
                    })
                    .collect::<Vec<_>>()
            )),
            "boneFor" => Ok(drop::bone_experience(int(&a["id"]))
                .map_or(Value::Null, |xp| json!((xp.to_bits() as i64).to_string()))),
            "format" => Ok(json!(
                a["values"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|value| drop::formatted_number(int(value)))
                    .collect::<Vec<_>>()
            )),
            "drop" => {
                let npc = self.npc(a);
                let definition = self.ent(&npc).definition.clone().ok_or(Error::Null)?;
                let tables = definition.read().drop_tables.clone().ok_or(Error::Null)?;
                let looter = self.named(&a["looter"]);
                drop::drop(self, &tables, &npc, looter.as_ref())?;
                Ok(Value::Null)
            }
            "createDrop" => {
                let npc = self.npc(a);
                let item = a["item"]
                    .as_array()
                    .map(|row| self.new_item(int(&row[0]), int(&row[1]), 1000));
                let player = a["player"]
                    .as_u64()
                    .map(|index| self.ents[index as usize].handle.clone());
                drop::create_drop(self, item, player.as_ref(), &npc, place(&a["location"]))?;
                Ok(Value::Null)
            }
            "getLooter" => {
                let npc = self.npc(a);
                let item = self.new_item(int(&a["item"][0]), int(&a["item"][1]), 1000);
                let player = self.ents[int(&a["player"]) as usize].handle.clone();
                let looter = drop::get_looter(self, &player, &npc, &item)?;
                Ok(json!(self.label(&looter)))
            }
            "hit" => {
                let target = self.named(&a["target"]).unwrap();
                let source = self.named(&a["source"]);
                death::log_hit(self, &target, source.as_ref(), int(&a["amount"]))?;
                if flag(a, "lethal", false) {
                    death::lethal_impact(self, &target, source.as_ref())?;
                }
                Ok(Value::Null)
            }
            "credit" => {
                let target = self.named(&a["target"]).unwrap();
                let killer = self.named(&a["killer"]);
                let credited = death::most_damage_entity(self, &target, killer.as_ref())?;
                Ok(self.optional_label(credited.as_ref()))
            }
            "startDeath" => {
                let target = self.named(&a["target"]).unwrap();
                let killer = self.named(&a["killer"]);
                death::entity_start_death(self, &target, killer.as_ref())?;
                Ok(Value::Null)
            }
            "isDead" => {
                let target = self.named(&a["target"]).unwrap();
                Ok(json!(death::is_dead(self, &target)?))
            }
            "strong" => {
                let target = self.named(&a["target"]).unwrap();
                let pulse = Shared::new(pulses::Pulse::new(
                    int(&a["delay"]),
                    None,
                    pulses::Class::Other,
                    "oracle.Support$1".into(),
                ));
                let manager = self.ent(&target).manager.clone();
                let runner = self.runner.clone();
                manager
                    .run(
                        Some(&pulse),
                        Some(pulses::Kind::Strong),
                        &runner,
                        self,
                        &mut Dispatch,
                    )
                    .map_err(from_pulse)?;
                Ok(Value::Null)
            }
            "tick" => {
                for _ in 0..a.get("count").map_or(1, int) {
                    self.tick = self.tick.wrapping_add(1);
                    self.emit(json!(["tick", self.tick]));
                    let runner = self.runner.clone();
                    runner.update_all(self, &mut Dispatch).map_err(from_pulse)?;
                    if flag(a, "npcs", false) {
                        for index in self.players..self.ents.len() {
                            let state = self.ents[index].state.clone();
                            let mut context = NpcTick {
                                events: self.events.clone(),
                                label: &self.ents[index].label,
                                tick: self.tick,
                            };
                            state
                                .tick(&mut context)
                                .map_err(|error| Error::Binding(format!("{error:?}")))?;
                        }
                    }
                }
                Ok(Value::Null)
            }
            "groundPulse" => {
                let manager = self.manager.clone();
                gm::pulse(self, &manager)?;
                Ok(Value::Null)
            }
            "set" => {
                if let Some(online) = a.get("online").and_then(Value::as_bool) {
                    self.ents[int(&a["player"]) as usize].online = online;
                }
                if let Some(active) = a.get("active").and_then(Value::as_bool) {
                    let target = self.named(&a["target"]).unwrap();
                    let index = self.index(&target);
                    self.ents[index].active = active;
                }
                if let Some(value) = a.get("disableDrop") {
                    let target = self.named(&a["target"]).unwrap();
                    let index = self.index(&target);
                    self.ents[index]
                        .attributes
                        .insert(death::DISABLE_DROP.to_owned(), value.clone());
                }
                if let Some(tick) = a.get("tick") {
                    self.tick = int(tick);
                }
                Ok(Value::Null)
            }
            other => panic!("unknown operation {other}"),
        }
    }

    fn entity_state(&self, ent: &Ent) -> Map<String, Value> {
        let log = ent.impact.read();
        let npc_log: Vec<_> = log
            .npcs()
            .into_iter()
            .map(|(source, amount)| json!([self.optional_label(source.as_ref()), amount]))
            .collect();
        let mut out = Map::new();
        out.insert("attributes".into(), json!(ent.attributes));
        out.insert("disabledTicks".into(), json!(log.disabled_ticks));
        out.insert("playerLog".into(), json!(log.players()));
        out.insert("npcLog".into(), json!(npc_log));
        out.insert("active".into(), json!(ent.active));
        out
    }

    fn state(&self) -> Value {
        let grounds: Vec<_> = self
            .manager
            .get_items()
            .read()
            .iter()
            .flatten()
            .map(|ground| {
                let ground = ground.read();
                let item = ground.item.read();
                json!([
                    item.id(),
                    item.amount(),
                    item.charge(),
                    coordinates(ground.location.as_ref()),
                    self.optional_label(ground.dropper.as_ref()),
                    ground.dropper_uid,
                    ground.ticks,
                    ground.decay_time,
                    ground.remain_private,
                    ground.removed
                ])
            })
            .collect();
        let chunk: Vec<_> = chunk_sync::get_items(&self.chunk)
            .read()
            .iter()
            .flatten()
            .map(ground_pair)
            .collect();
        let flags: Vec<_> = self
            .chunk
            .read()
            .update_flags
            .read()
            .iter()
            .flatten()
            .filter_map(|flag| match &*flag.read() {
                Flag::Item { item, kind, .. } => Some(json!([ground_pair(item), kind])),
                _ => None,
            })
            .collect();
        let npcs: Vec<_> = self.ents[self.players..]
            .iter()
            .map(|ent| {
                let mut row = self.entity_state(ent);
                let fields = ent.state.fields.read();
                row.insert("respawnTick".into(), json!(fields.respawn_tick));
                row.insert("respawning".into(), json!(fields.respawning));
                Value::Object(row)
            })
            .collect();
        let players: Vec<_> = self.ents[..self.players]
            .iter()
            .map(|ent| {
                let mut row = self.entity_state(ent);
                row.insert("lsp".into(), json!(ent.points));
                Value::Object(row)
            })
            .collect();
        json!({
            "tick": self.tick,
            "grounds": grounds,
            "chunk": chunk,
            "flags": flags,
            "npcs": npcs,
            "players": players,
            "queued": self.runner.current().len(),
            "rng": [
                self.source.random.clone().next_i64().to_string(),
                self.source.shuffle.clone().next_i64().to_string(),
                self.source.thread_local.seed().to_string(),
            ],
        })
    }
}

/// The `expected` list of one case, from the native implementation.
pub fn evaluate(case: &Value) -> Value {
    let mut model = build(case);
    let mut steps = Vec::new();
    for action in case["actions"].as_array().unwrap() {
        model.events.borrow_mut().clear();
        let (value, error) = match model.act(action) {
            Ok(value) => (value, Value::Null),
            Err(error) => (Value::Null, json!(category(&error))),
        };
        let events = std::mem::take(&mut *model.events.borrow_mut());
        steps.push(
            json!({"value": value, "error": error, "events": events, "state": model.state()}),
        );
    }
    Value::Array(steps)
}
