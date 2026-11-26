//! Native replay of the player death corpus: the case's inputs answer the
//! required services of `player_death::Host` (and of the death task, drop and
//! ground manager it extends), every service records its call as the Java
//! adapters do, and the native containers, items, item definitions, ground
//! manager, ground items, `npc_death` task functions and `player_death`
//! libraries are the real ones.
use rs09_server::{
    definitions::{
        item::{self as item_definitions, ItemDefinition, Shared},
        npc as npc_definitions,
        property::Property,
    },
    inventory::{
        container::{self, Container, ContainerType, Factory, SortType},
        container_listener::Handle as Listener,
        item::{self, Item},
    },
    player::ironman::Mode,
    pulses,
    world::{
        chunk::{HandleChunk, RegionChunk},
        chunk_sync::{self, Flag, Items},
        doors::Location,
        drop_tables::{
            NpcDropTable, RollError, RolledItem,
            drop::{self, Error as DropError},
            random::{JavaSource, Source},
        },
        ground_item as ground, ground_manager as gm, npc,
        npc_death::{self, Attribute, ImpactLog, Task},
        player_death::{
            self as death, Controller, Error, GraveType, Prayer, Rights, barrows, grave,
            store::{self, Json},
        },
        player_walk::{NodeHandle, State},
    },
};
use serde_json::{Map, Value, json};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

type Events = Rc<RefCell<Vec<Value>>>;
type R<T> = drop::Result<T>;

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
fn triple(item: &item::Handle) -> Value {
    let item = item.read();
    json!([item.id(), item.amount(), item.charge()])
}
fn ground_triple(item: &ground::Handle) -> Value {
    triple(&item.read().item)
}
fn bits(value: f64) -> String {
    (value.to_bits() as i64).to_string()
}
fn unbits(value: &Value) -> f64 {
    f64::from_bits(value.as_str().unwrap().parse::<i64>().unwrap() as u64)
}
fn binding(text: &str) -> DropError {
    DropError::Binding(text.into())
}

#[derive(Clone, Debug)]
enum Attr {
    Bool(bool),
    Int(i32),
    Text(String),
    Place(Location),
}
impl Attr {
    fn json(&self) -> Value {
        match self {
            Self::Bool(value) => json!(value),
            Self::Int(value) => json!(value),
            Self::Text(value) => json!(value),
            Self::Place(location) => coordinates(Some(location)),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Player,
    Npc,
    Grave,
}

struct Ent {
    handle: NodeHandle,
    label: String,
    kind: Kind,
    input: Value,
    attributes: BTreeMap<String, Attr>,
    location: Option<Location>,
    impact: Shared<ImpactLog>,
    inventory: Option<Container>,
    equipment: Option<Container>,
    prayer: Option<f64>,
    id: i32,
    ticking: bool,
    grave: Option<grave::Handle>,
}

#[derive(Clone)]
struct ItemPlugin {
    id: i32,
    death_item: Value,
}

struct Model {
    events: Events,
    input: Value,
    tick: i32,
    ents: Vec<Ent>,
    definitions: Rc<item_definitions::Registry>,
    factory: Rc<Factory>,
    plugins: BTreeMap<i32, ItemPlugin>,
    equipment_plugins: BTreeMap<i32, i32>,
    chunk: HandleChunk,
    manager: gm::Manager,
    controller: Controller,
    uid_map: Vec<(i32, NodeHandle)>,
    labels: Vec<(ground::Handle, String)>,
    archive: Json,
    source: JavaSource,
    actors: State,
}

fn category(error: &Error) -> String {
    use rs09_server::inventory::Error as I;
    match error {
        Error::Death(DropError::Null) | Error::Container(I::Null) => "Null".into(),
        Error::Death(DropError::Type) | Error::Container(I::Type) => "Type".into(),
        Error::Death(DropError::Index) | Error::Container(I::Bounds | I::ListBounds) => {
            "Index".into()
        }
        Error::Number | Error::Container(I::Number) => "Number".into(),
        Error::Uninitialized => "Uninitialized".into(),
        Error::Death(DropError::Ground(
            gm::Error::Null | gm::Error::Ground(ground::Error::Null),
        )) => "Null".into(),
        other => format!("{other:?}"),
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
    fn ent_mut(&mut self, handle: &NodeHandle) -> &mut Ent {
        let index = self.index(handle);
        &mut self.ents[index]
    }
    fn label(&self, handle: &NodeHandle) -> String {
        self.ent(handle).label.clone()
    }
    fn optional(&self, handle: Option<&NodeHandle>) -> Value {
        handle.map_or(Value::Null, |handle| json!(self.label(handle)))
    }
    fn named(&self, name: &Value) -> Option<NodeHandle> {
        let text = name.as_str()?;
        self.ents
            .iter()
            .find(|ent| ent.label == text)
            .map(|ent| ent.handle.clone())
    }
    fn input_of(&self, handle: &NodeHandle) -> &Value {
        &self.ent(handle).input
    }
    fn new_item(&self, id: i32, amount: i32, charge: i32) -> item::Handle {
        Shared::new(Item::new(id, amount, charge, &self.definitions))
    }
    fn item_from(&self, value: &Value) -> item::Handle {
        let list = value.as_array().unwrap();
        let charge = list.get(2).map_or(1000, int);
        self.new_item(int(&list[0]), int(&list[1]), charge)
    }
    fn ground_label(&mut self, item: &ground::Handle) -> String {
        if let Some((_, label)) = self.labels.iter().find(|(known, _)| known.same(item)) {
            return label.clone();
        }
        let label = format!("i{}", self.labels.len());
        self.labels.push((item.clone(), label.clone()));
        label
    }
    fn attribute(&mut self, entity: &NodeHandle, key: &str) -> Option<Attr> {
        let label = self.label(entity);
        self.emit(json!(["getAttribute", label, key]));
        self.ent(entity).attributes.get(key).cloned()
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
        Ok(self.input_of(player).get("uid").map_or(0, int))
    }
    fn username(&mut self, player: &NodeHandle) -> ground::Result<Option<String>> {
        self.emit(json!(["username", self.label(player)]));
        Ok(self.input_of(player)["username"]
            .as_str()
            .map(str::to_owned))
    }
    fn ticks(&mut self) -> ground::Result<i32> {
        Ok(self.tick)
    }
}

impl gm::Context for Model {
    type Plugin = ItemPlugin;
    fn plugin(&mut self, definition: &item_definitions::Handle) -> gm::Result<Option<ItemPlugin>> {
        Ok(self.plugins.get(&definition.read().id).cloned())
    }
    fn plugin_remove(
        &mut self,
        plugin: ItemPlugin,
        player: Option<NodeHandle>,
        item: &ground::Handle,
        option: i32,
    ) -> gm::Result<()> {
        let player = self.optional(player.as_ref());
        self.emit(json!([
            "pluginRemove",
            plugin.id,
            player,
            ground_triple(item),
            option
        ]));
        Ok(())
    }
    fn chunk(&mut self, location: Location) -> gm::Result<HandleChunk> {
        self.emit(json!(["chunk", location.coordinates()]));
        Ok(self.chunk.clone())
    }
    fn flagged(&mut self, chunk: &HandleChunk) -> gm::Result<()> {
        let flags = chunk.read().update_flags.clone();
        let last = flags.read().last().cloned().flatten();
        if let Some(flag) = last
            && let Flag::Item { item, kind, .. } = &*flag.read()
        {
            self.emit(json!(["flagged", kind, ground_triple(item)]));
        }
        Ok(())
    }
    fn player_location(&mut self, player: &NodeHandle) -> gm::Result<Option<Location>> {
        Ok(self.ent(player).location.clone())
    }
    fn playing(&mut self, player: &NodeHandle) -> gm::Result<bool> {
        self.emit(json!(["isPlaying", self.label(player)]));
        Ok(flag(self.input_of(player), "playing", true))
    }
    fn artificial(&mut self, player: &NodeHandle) -> gm::Result<bool> {
        self.emit(json!(["isArtificial", self.label(player)]));
        Ok(flag(self.input_of(player), "artificial", false))
    }
    fn ai_items(&mut self, player: &NodeHandle) -> gm::Result<Option<Items>> {
        self.emit(json!(["aiItems", self.label(player)]));
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
        let player = self.optional(player.as_ref());
        self.emit(json!(["packet", kind, player, ground_triple(item)]));
        Ok(())
    }
}

impl container::Context for Model {
    fn update(
        &mut self,
        _: Listener,
        _: &mut Container,
        _: container::Event,
        _: &item_definitions::Registry,
    ) -> rs09_server::inventory::Result<()> {
        Err(rs09_server::inventory::Error::Binding(
            "no container listener in this corpus".into(),
        ))
    }
    fn refresh(
        &mut self,
        _: Listener,
        _: &mut Container,
        _: &item_definitions::Registry,
    ) -> rs09_server::inventory::Result<()> {
        Err(rs09_server::inventory::Error::Binding(
            "no container listener in this corpus".into(),
        ))
    }
    fn ground(
        &mut self,
        _: &mut Container,
        _: Option<item::Handle>,
        _: &item_definitions::Registry,
    ) -> rs09_server::inventory::Result<()> {
        Err(rs09_server::inventory::Error::Binding(
            "add-or-ground is outside this corpus".into(),
        ))
    }
}

impl drop::Host for Model {
    type Clan = usize;
    fn is_player(&mut self, entity: &NodeHandle) -> bool {
        self.ent(entity).kind == Kind::Player
    }
    fn is_ai_player(&mut self, _: &NodeHandle) -> bool {
        false
    }
    fn random(&mut self) -> &mut dyn Source {
        &mut self.source
    }
    fn roll(
        &mut self,
        _: &NpcDropTable,
        _: Option<&NodeHandle>,
    ) -> Result<Vec<RolledItem>, RollError> {
        Err(RollError::Behavior)
    }
    fn manager(&mut self) -> gm::Manager {
        self.manager.clone()
    }
    fn on_drop_table_rolled(
        &mut self,
        _: &NodeHandle,
        _: &NodeHandle,
        _: &mut Vec<RolledItem>,
    ) -> R<()> {
        Err(binding("drops are outside this corpus"))
    }
    fn npc_id(&mut self, npc: &NodeHandle) -> R<i32> {
        // Node.getId() of a player is the original text (-1) and records nothing.
        if self.ent(npc).kind == Kind::Player {
            return Ok(-1);
        }
        self.emit(json!(["npcId", self.label(npc)]));
        Ok(self.ent(npc).id)
    }
    fn npc_drop_location(&mut self, _: &NodeHandle) -> R<Option<Location>> {
        Err(binding("drops are outside this corpus"))
    }
    fn npc_location(&mut self, npc: &NodeHandle) -> R<Option<Location>> {
        self.emit(json!(["npcLocation", self.label(npc)]));
        Ok(self.ent(npc).location.clone())
    }
    fn npc_multi_zone(&mut self, _: &NodeHandle) -> R<bool> {
        Err(binding("drops are outside this corpus"))
    }
    fn plugin_create_drop(
        &mut self,
        _: ItemPlugin,
        _: &item::Handle,
        _: &NodeHandle,
        _: &NodeHandle,
        _: &Location,
    ) -> R<bool> {
        Err(binding("drops are outside this corpus"))
    }
    fn plugin_get_item(
        &mut self,
        _: ItemPlugin,
        _: &item::Handle,
        _: &NodeHandle,
    ) -> R<Option<item::Handle>> {
        Err(binding("drops are outside this corpus"))
    }
    fn send_message(&mut self, player: &NodeHandle, message: &str) -> R<()> {
        self.emit(json!(["playerMessage", self.label(player), message]));
        Ok(())
    }
    fn resizable(&mut self, _: &NodeHandle) -> R<bool> {
        Err(binding("drops are outside this corpus"))
    }
    fn bone_crusher_enabled(&mut self, _: &NodeHandle) -> R<bool> {
        Err(binding("drops are outside this corpus"))
    }
    fn add_experience(&mut self, _: &NodeHandle, _: i32, _: f64) -> R<()> {
        Err(binding("drops are outside this corpus"))
    }
    fn clan(&mut self, _: &NodeHandle) -> R<Option<usize>> {
        Err(binding("drops are outside this corpus"))
    }
    fn clan_loot_requirement(&mut self, _: &usize) -> R<i32> {
        Err(binding("drops are outside this corpus"))
    }
    fn loot_share(&mut self, _: &NodeHandle) -> R<bool> {
        Err(binding("drops are outside this corpus"))
    }
    fn loot_requirement(&mut self, _: &NodeHandle) -> R<i32> {
        Err(binding("drops are outside this corpus"))
    }
    fn ironman(&mut self, player: &NodeHandle) -> R<bool> {
        self.emit(json!(["isIronman", self.label(player)]));
        let mode = self.input_of(player)["mode"].as_str().map(str::to_owned);
        Ok(mode.is_some_and(|mode| mode != "NONE"))
    }
    fn loot_share_points(&mut self, _: &NodeHandle) -> R<i32> {
        Err(binding("drops are outside this corpus"))
    }
    fn set_loot_share_points(&mut self, _: &NodeHandle, _: i32) -> R<()> {
        Err(binding("drops are outside this corpus"))
    }
    fn local_players(&mut self, _: Option<&Location>, _: i32) -> R<Vec<Option<NodeHandle>>> {
        Err(binding("drops are outside this corpus"))
    }
    fn recommended_price(&mut self, _: i32, _: bool) -> R<i32> {
        Err(binding("drops are outside this corpus"))
    }
    fn repository_players(&mut self) -> R<Vec<NodeHandle>> {
        Err(binding("drops are outside this corpus"))
    }
    fn send_news(&mut self, _: &str) -> R<()> {
        Err(binding("drops are outside this corpus"))
    }
    fn increment_rare_drop(&mut self, _: &NodeHandle, _: &item::Handle) -> R<()> {
        Err(binding("drops are outside this corpus"))
    }
    fn ai_add_item(&mut self, _: &ground::Handle) -> R<()> {
        Err(binding("drops are outside this corpus"))
    }
    fn bot_script_running(&mut self, _: &NodeHandle) -> R<bool> {
        Err(binding("drops are outside this corpus"))
    }
    fn bot_add_drop(&mut self, _: &NodeHandle, _: &ground::Handle) -> R<()> {
        Err(binding("drops are outside this corpus"))
    }
}

fn death_animation(input: &Value) -> (i32, i32) {
    input
        .get("deathAnimation")
        .and_then(Value::as_array)
        .map_or((836, 4), |a| (int(&a[0]), int(&a[1])))
}
/// `Animator.RESET_A` and `RESET_G`, as the oracle's fixture shows them.
const RESET_ANIMATION: i32 = -1;
const RESET_GRAPHICS: i32 = -1;

fn lift(error: Error) -> DropError {
    match error {
        Error::Death(error) => error,
        other => DropError::Binding(format!("{other:?}")),
    }
}

impl npc_death::Host for Model {
    fn is_npc(&mut self, entity: &NodeHandle) -> bool {
        self.ent(entity).kind != Kind::Player
    }
    fn identity_hash(&mut self, _: &NodeHandle) -> i32 {
        0
    }
    fn impact_log(&mut self, entity: &NodeHandle) -> R<Shared<ImpactLog>> {
        Ok(self.ent(entity).impact.clone())
    }
    fn player_by_uid(&mut self, _: i32) -> R<Option<NodeHandle>> {
        Err(binding("kill credit is outside this corpus"))
    }
    fn set_attribute(&mut self, entity: &NodeHandle, key: &str, value: Attribute) -> R<()> {
        let value = match value {
            Attribute::Bool(value) => Attr::Bool(value),
            Attribute::Int(value) => Attr::Int(value),
        };
        self.emit(json!([
            "setAttribute",
            self.label(entity),
            key,
            value.json()
        ]));
        self.ent_mut(entity).attributes.insert(key.into(), value);
        Ok(())
    }
    fn remove_attribute(&mut self, entity: &NodeHandle, key: &str) -> R<()> {
        self.ent_mut(entity).attributes.remove(key);
        self.emit(json!(["removeAttribute", self.label(entity), key]));
        Ok(())
    }
    fn attribute_bool(&mut self, entity: &NodeHandle, key: &str, fallback: bool) -> R<bool> {
        match self.attribute(entity, key) {
            None => Ok(fallback),
            Some(Attr::Bool(value)) => Ok(value),
            Some(_) => Err(DropError::Type),
        }
    }
    fn attribute_int(&mut self, entity: &NodeHandle, key: &str, fallback: i32) -> R<i32> {
        match self.attribute(entity, key) {
            None => Ok(fallback),
            Some(Attr::Int(value)) => Ok(value),
            Some(_) => Err(DropError::Type),
        }
    }
    fn increment_attribute(&mut self, player: &NodeHandle, key: &str) -> R<()> {
        self.emit(json!(["incrementAttribute", self.label(player), key]));
        Ok(())
    }
    fn run_strong(&mut self, _: &Task) -> R<()> {
        Err(binding("DeathTask.startDeath is outside this corpus"))
    }
    fn stop_combat_pulse(&mut self, _: &NodeHandle) -> R<()> {
        Err(binding("combat is outside this corpus"))
    }
    fn zone_start_death(&mut self, _: &NodeHandle, _: Option<&NodeHandle>) -> R<bool> {
        Err(binding("startDeath is outside this corpus"))
    }
    fn zone_handle_death(&mut self, _: &NodeHandle, _: &NodeHandle) -> R<bool> {
        Err(binding("a player's zone death uses zone_player_death"))
    }
    fn walking_reset(&mut self, entity: &NodeHandle) -> R<()> {
        self.emit(json!(["walkingReset", self.label(entity)]));
        Ok(())
    }
    fn lock(&mut self, entity: &NodeHandle, ticks: i32) -> R<()> {
        self.emit(json!(["lock", self.label(entity), ticks]));
        Ok(())
    }
    fn unlock(&mut self, entity: &NodeHandle) -> R<()> {
        self.emit(json!(["unlock", self.label(entity)]));
        Ok(())
    }
    fn face(&mut self, entity: &NodeHandle, target: Option<&NodeHandle>) -> R<()> {
        let target = self.optional(target);
        self.emit(json!(["face", self.label(entity), target]));
        Ok(())
    }
    fn play_global_audio(&mut self, _: Option<&Location>, _: i32) -> R<()> {
        Err(binding("a player's death plays no global audio"))
    }
    fn reset_graphics(&mut self, entity: &NodeHandle) -> R<()> {
        self.emit(json!(["graphics", self.label(entity), RESET_GRAPHICS]));
        Ok(())
    }
    fn visualize_death(&mut self, entity: &NodeHandle) -> R<()> {
        let label = self.label(entity);
        self.emit(json!(["deathAnimation", label]));
        let (id, _) = death_animation(self.input_of(entity));
        self.emit(json!(["visualize", label, id, Value::Null]));
        Ok(())
    }
    fn force_death_animation(&mut self, entity: &NodeHandle) -> R<()> {
        let label = self.label(entity);
        self.emit(json!(["deathAnimation", label]));
        let (id, _) = death_animation(self.input_of(entity));
        self.emit(json!(["forceAnimation", label, id]));
        Ok(())
    }
    fn force_reset_animation(&mut self, entity: &NodeHandle) -> R<()> {
        self.emit(json!([
            "forceAnimation",
            self.label(entity),
            RESET_ANIMATION
        ]));
        Ok(())
    }
    fn death_animation_duration(&mut self, entity: &NodeHandle) -> R<i32> {
        self.emit(json!(["deathAnimation", self.label(entity)]));
        Ok(death_animation(self.input_of(entity)).1)
    }
    fn respawn_location(&mut self, entity: &NodeHandle) -> R<Option<Location>> {
        let label = self.label(entity);
        self.emit(json!(["isSafeZone", label]));
        if flag(self.input_of(entity), "safeZone", false) {
            return Ok(None);
        }
        self.emit(json!(["spawnLocation", label]));
        Ok(place(&self.input_of(entity)["spawn"]))
    }
    fn set_teleport_location(&mut self, entity: &NodeHandle, location: Option<&Location>) -> R<()> {
        self.emit(json!([
            "teleport",
            self.label(entity),
            coordinates(location)
        ]));
        Ok(())
    }
    fn dispatch_self_death(&mut self, entity: &NodeHandle, killer: &NodeHandle) -> R<()> {
        self.emit(json!([
            "dispatch",
            self.label(entity),
            "SelfDeathEvent",
            self.label(killer)
        ]));
        Ok(())
    }
    fn player_commence_death(&mut self, player: &NodeHandle, killer: &NodeHandle) -> R<()> {
        death::commence_death(self, player, Some(killer)).map_err(lift)
    }
    fn player_finalize_death(&mut self, player: &NodeHandle, killer: &NodeHandle) -> R<()> {
        let mut controller = std::mem::take(&mut self.controller);
        let (factory, definitions) = (self.factory.clone(), self.definitions.clone());
        let result = death::finalize_death(
            self,
            &mut controller,
            player,
            Some(killer),
            &factory,
            &definitions,
        );
        self.controller = controller;
        result.map_err(lift)
    }
    fn skills_restore(&mut self, entity: &NodeHandle) -> R<()> {
        self.emit(json!(["restore", self.label(entity)]));
        Ok(())
    }
    fn recharge_prayer(&mut self, entity: &NodeHandle) -> R<()> {
        self.emit(json!(["rechargePrayer", self.label(entity)]));
        Ok(())
    }
    fn clear_impact_queue(&mut self, entity: &NodeHandle) -> R<()> {
        self.emit(json!(["clearImpactQueue", self.label(entity)]));
        Ok(())
    }
    fn timers_on_death(&mut self, entity: &NodeHandle) -> R<()> {
        self.emit(json!(["timersOnDeath", self.label(entity)]));
        Ok(())
    }
    fn transform(&mut self, npc: &NodeHandle, id: i32) -> R<()> {
        self.emit(json!(["transform", self.label(npc), id]));
        self.ent_mut(npc).id = id;
        Ok(())
    }
    fn npc_state(&mut self, _: &NodeHandle) -> R<npc::State> {
        Err(binding("NPC state is outside this corpus"))
    }
    fn npc_definition(&mut self, _: &NodeHandle) -> R<Option<npc_definitions::Handle>> {
        Err(binding("NPC definitions are outside this corpus"))
    }
    fn npc_original_id(&mut self, _: &NodeHandle) -> R<i32> {
        Err(binding("NPC deaths are outside this corpus"))
    }
    fn npc_respawns(&mut self, _: &NodeHandle) -> R<bool> {
        Err(binding("NPC deaths are outside this corpus"))
    }
    fn npc_clear(&mut self, npc: &NodeHandle) -> R<()> {
        self.emit(json!(["clear", self.label(npc)]));
        self.ent_mut(npc).ticking = false;
        Ok(())
    }
    fn on_death_started(&mut self, _: &NodeHandle, _: &NodeHandle) -> R<()> {
        Err(binding("NPC deaths are outside this corpus"))
    }
    fn on_death_finished(&mut self, _: &NodeHandle, _: &NodeHandle) -> R<()> {
        Err(binding("NPC deaths are outside this corpus"))
    }
    fn increment_kills(&mut self, _: &NodeHandle, _: i32) -> R<()> {
        Err(binding("NPC deaths are outside this corpus"))
    }
    fn dispatch_npc_kill(&mut self, _: &NodeHandle, _: &NodeHandle) -> R<()> {
        Err(binding("NPC deaths are outside this corpus"))
    }
}

impl death::KeptHost for Model {
    fn inventory(&mut self, player: &NodeHandle) -> R<Container> {
        self.ent(player).inventory.clone().ok_or(DropError::Null)
    }
    fn equipment(&mut self, player: &NodeHandle) -> R<Container> {
        self.ent(player).equipment.clone().ok_or(DropError::Null)
    }
    fn skulled(&mut self, player: &NodeHandle) -> R<bool> {
        self.emit(json!(["skulled", self.label(player)]));
        Ok(flag(self.input_of(player), "skulled", false))
    }
    fn prayer_active(&mut self, player: &NodeHandle, prayer: Prayer) -> R<bool> {
        let (name, key) = match prayer {
            Prayer::ProtectItems => ("PROTECT_ITEMS", "protect"),
            Prayer::Retribution => ("RETRIBUTION", "retribution"),
        };
        self.emit(json!(["prayer", self.label(player), name]));
        Ok(flag(self.input_of(player), key, false))
    }
    fn ironman_mode(&mut self, player: &NodeHandle) -> R<Option<Mode>> {
        self.emit(json!(["ironmanMode", self.label(player)]));
        Ok(match self.input_of(player)["mode"].as_str() {
            None => None,
            Some("NONE") => Some(Mode::None),
            Some("STANDARD") => Some(Mode::Standard),
            Some("HARDCORE") => Some(Mode::Hardcore),
            Some("ULTIMATE") => Some(Mode::Ultimate),
            Some(other) => panic!("mode {other}"),
        })
    }
}

impl death::Host for Model {
    type EquipmentPlugin = i32;
    fn is_active(&mut self, node: &NodeHandle) -> R<bool> {
        self.emit(json!(["isActive", self.label(node)]));
        Ok(flag(self.input_of(node), "active", true))
    }
    fn is_male(&mut self, player: &NodeHandle) -> R<bool> {
        self.emit(json!(["isMale", self.label(player)]));
        Ok(flag(self.input_of(player), "male", true))
    }
    fn get_name(&mut self, player: &NodeHandle) -> R<Option<String>> {
        self.emit(json!(["name", self.label(player)]));
        Ok(self.input_of(player)["name"].as_str().map(str::to_owned))
    }
    fn name_field(&mut self, player: &NodeHandle) -> R<Option<String>> {
        Ok(self.input_of(player)["name"].as_str().map(str::to_owned))
    }
    fn rights(&mut self, player: &NodeHandle) -> R<Rights> {
        self.emit(json!(["rights", self.label(player)]));
        Ok(match self.input_of(player)["rights"].as_str() {
            None | Some("REGULAR_PLAYER") => Rights::RegularPlayer,
            Some("PLAYER_MODERATOR") => Rights::PlayerModerator,
            Some("ADMINISTRATOR") => Rights::Administrator,
            Some(other) => panic!("rights {other}"),
        })
    }
    fn increment_death_count(&mut self) -> R<()> {
        self.emit(json!(["deathCount"]));
        Ok(())
    }
    fn set_special_energy(&mut self, player: &NodeHandle, value: i32) -> R<()> {
        self.emit(json!(["specialEnergy", self.label(player), value]));
        Ok(())
    }
    fn run_energy(&mut self, player: &NodeHandle) -> R<f64> {
        self.emit(json!(["runEnergy", self.label(player)]));
        Ok(self.input_of(player).get("runEnergy").map_or(100.0, unbits))
    }
    fn update_run_energy(&mut self, player: &NodeHandle, value: f64) -> R<()> {
        self.emit(json!(["updateRunEnergy", self.label(player), bits(value)]));
        Ok(())
    }
    fn dispatch_message(&mut self, player: &NodeHandle, message: &str) -> R<()> {
        self.emit(json!(["dispatchMessage", self.label(player), message]));
        Ok(())
    }
    fn temp_music(&mut self, player: &NodeHandle, id: i32) -> R<()> {
        self.emit(json!(["tempMusic", self.label(player), id]));
        Ok(())
    }
    fn api_message(&mut self, player: &NodeHandle, message: &str) -> R<()> {
        self.emit(json!(["sendMessage", self.label(player), message]));
        Ok(())
    }
    fn zone_player_death(&mut self, player: &NodeHandle, killer: Option<&NodeHandle>) -> R<bool> {
        let killer = self.optional(killer);
        self.emit(json!(["zoneHandleDeath", self.label(player), killer]));
        Ok(flag(self.input_of(player), "zoneHandle", false))
    }
    fn safe_zone(&mut self, player: &NodeHandle) -> R<bool> {
        self.emit(json!(["isSafeZone", self.label(player)]));
        Ok(flag(self.input_of(player), "safeZone", false))
    }
    fn zone_type(&mut self, player: &NodeHandle) -> R<i32> {
        self.emit(json!(["zoneType", self.label(player)]));
        Ok(self.input_of(player).get("zoneType").map_or(0, int))
    }
    fn graves_restricted(&mut self, player: &NodeHandle) -> R<bool> {
        self.emit(json!(["zoneRestricted", self.label(player), "GRAVES"]));
        Ok(flag(self.input_of(player), "restrictGraves", false))
    }
    fn skull_wilderness(&mut self, player: &NodeHandle) -> R<bool> {
        self.emit(json!(["wilderness", self.label(player)]));
        Ok(flag(self.input_of(player), "wilderness", false))
    }
    fn in_wilderness_zone(&mut self, player: &NodeHandle) -> R<bool> {
        self.emit(json!(["wildernessZone", self.label(player)]));
        Ok(flag(self.input_of(player), "inWilderness", false))
    }
    fn set_skulled(&mut self, player: &NodeHandle, value: bool) -> R<()> {
        self.emit(json!(["setSkulled", self.label(player), value]));
        Ok(())
    }
    fn start_retribution(&mut self, player: &NodeHandle, killer: Option<&NodeHandle>) -> R<()> {
        let killer = self.optional(killer);
        self.emit(json!(["retribution", self.label(player), killer]));
        Ok(())
    }
    fn prayer_reset(&mut self, player: &NodeHandle) -> R<()> {
        self.emit(json!(["prayerReset", self.label(player)]));
        Ok(())
    }
    fn attribute_location(
        &mut self,
        entity: &NodeHandle,
        key: &str,
    ) -> R<Option<Option<Location>>> {
        match self.attribute(entity, key) {
            None => Ok(None),
            Some(Attr::Place(location)) => Ok(Some(Some(location))),
            Some(_) => Err(DropError::Type),
        }
    }
    fn attributes_contain(&mut self, entity: &NodeHandle, key: &str) -> R<bool> {
        self.emit(json!(["attributes", self.label(entity)]));
        Ok(self.ent(entity).attributes.contains_key(key))
    }
    fn location_field(&mut self, node: &NodeHandle) -> R<Option<Location>> {
        Ok(self.ent(node).location.clone())
    }
    fn set_location_field(&mut self, node: &NodeHandle, location: Option<Location>) -> R<()> {
        self.ent_mut(node).location = location;
        Ok(())
    }
    fn repository_news(&mut self, message: &str) -> R<()> {
        self.emit(json!(["repositoryNews", message]));
        Ok(())
    }
    fn total_level(&mut self, player: &NodeHandle) -> R<i32> {
        self.emit(json!(["totalLevel", self.label(player)]));
        Ok(self.input_of(player).get("totalLevel").map_or(0, int))
    }
    fn permadeath(&mut self, player: &NodeHandle) -> R<()> {
        self.emit(json!(["permadeath", self.label(player)]));
        Ok(())
    }
    fn run_equip_listeners(
        &mut self,
        id: i32,
        player: &NodeHandle,
        item: &item::Handle,
        equip: bool,
    ) -> R<()> {
        self.emit(json!([
            "unequipListeners",
            id,
            self.label(player),
            triple(item),
            equip
        ]));
        Ok(())
    }
    fn equipment_plugin(&mut self, definition: &item_definitions::Handle) -> R<Option<i32>> {
        Ok(self.equipment_plugins.get(&definition.read().id).copied())
    }
    fn fire_unequip(&mut self, plugin: i32, player: &NodeHandle, item: &item::Handle) -> R<()> {
        self.emit(json!([
            "equipmentEvent",
            plugin,
            "unequip",
            self.label(player),
            triple(item)
        ]));
        Ok(())
    }
    fn item_name(&mut self, id: i32) -> R<Option<String>> {
        self.emit(json!(["itemName", id]));
        Ok(self.definitions.for_id(id).read().name.clone())
    }
    fn item_definition(&mut self, id: i32) -> R<item_definitions::Handle> {
        self.emit(json!(["itemDefinition", id]));
        Ok(self.definitions.for_id(id))
    }
    fn stat_level(&mut self, entity: &NodeHandle, skill: i32) -> R<i32> {
        self.emit(json!(["statLevel", self.label(entity), skill]));
        Ok(self.input_of(entity)["levels"]
            .get(skill.to_string())
            .map_or(1, int))
    }
    fn plugin_death_item(
        &mut self,
        plugin: ItemPlugin,
        item: &item::Handle,
    ) -> R<Option<item::Handle>> {
        self.emit(json!(["deathItem", plugin.id, triple(item)]));
        Ok(match &plugin.death_item {
            Value::Null => None,
            Value::String(_) => Some(item.clone()),
            value => Some(self.item_from(value)),
        })
    }
    fn monitor_pk(&mut self, player: &NodeHandle, details: &str) -> R<()> {
        self.emit(json!(["monitor", self.label(player), "PK", details]));
        Ok(())
    }
    fn slayer_flags(&mut self, player: &NodeHandle) -> R<()> {
        self.emit(json!(["slayerFlags", self.label(player)]));
        Ok(())
    }
    fn dismiss_familiar(&mut self, player: &NodeHandle) -> R<()> {
        self.emit(json!(["dismissFamiliar", self.label(player)]));
        Ok(())
    }
    fn open_default_tabs(&mut self, player: &NodeHandle) -> R<()> {
        self.emit(json!(["openDefaultTabs", self.label(player)]));
        Ok(())
    }
    fn component_visibility(
        &mut self,
        player: &NodeHandle,
        iface: i32,
        child: i32,
        hide: bool,
    ) -> R<()> {
        self.emit(json!([
            "componentVisibility",
            self.label(player),
            iface,
            child,
            hide
        ]));
        Ok(())
    }
    fn appearance_sync(&mut self, player: &NodeHandle) -> R<()> {
        self.emit(json!(["appearanceSync", self.label(player)]));
        Ok(())
    }
    fn death_screen_disabled(&mut self, player: &NodeHandle) -> R<bool> {
        self.emit(json!(["deathScreenDisabled", self.label(player)]));
        Ok(flag(self.input_of(player), "deathScreenDisabled", false))
    }
    fn set_death_screen_disabled(&mut self, player: &NodeHandle, value: bool) -> R<()> {
        self.emit(json!(["setDisableDeathScreen", self.label(player), value]));
        Ok(())
    }
    fn open_component(&mut self, player: &NodeHandle, id: i32) -> R<()> {
        self.emit(json!(["openComponent", self.label(player), id]));
        Ok(())
    }
    fn close_interface(&mut self, player: &NodeHandle) -> R<()> {
        self.emit(json!(["closeInterface", self.label(player)]));
        Ok(())
    }
    fn new_grave(&mut self) -> R<grave::Handle> {
        let index = self
            .ents
            .iter()
            .filter(|ent| ent.kind == Kind::Grave)
            .count();
        let label = format!("g{index}");
        let node = self.actors.add_npc(
            1000 + self.ents.len() as i32,
            Location::new(0, 0, 0),
            [1, 2, 3, 4],
        );
        let handle = self.actors.retain_node(node).unwrap();
        let grave = Shared::new(grave::Grave::new(handle.clone()));
        self.ents.push(Ent {
            handle,
            label: label.clone(),
            kind: Kind::Grave,
            input: json!({}),
            attributes: BTreeMap::new(),
            location: Some(Location::new(0, 0, 0)),
            impact: Shared::new(ImpactLog::default()),
            inventory: None,
            equipment: None,
            prayer: None,
            id: 6571,
            ticking: false,
            grave: Some(grave.clone()),
        });
        self.emit(json!(["graveNew", label, 6571, [0, 0, 0], false]));
        Ok(grave)
    }
    fn set_respawn(&mut self, npc: &NodeHandle, value: bool) -> R<()> {
        self.emit(json!(["setRespawn", self.label(npc), value]));
        Ok(())
    }
    fn set_walks(&mut self, npc: &NodeHandle, value: bool) -> R<()> {
        self.emit(json!(["setWalks", self.label(npc), value]));
        Ok(())
    }
    fn set_never_walks(&mut self, npc: &NodeHandle, value: bool) -> R<()> {
        self.emit(json!(["setNeverWalks", self.label(npc), value]));
        Ok(())
    }
    fn npc_init(&mut self, npc: &NodeHandle) -> R<()> {
        self.emit(json!(["init", self.label(npc)]));
        self.ent_mut(npc).ticking = true;
        Ok(())
    }
    fn uid_map(&mut self, uid: i32) -> R<Option<NodeHandle>> {
        self.emit(json!(["uidMap", uid]));
        Ok(self
            .uid_map
            .iter()
            .find(|(key, _)| *key == uid)
            .map(|(_, player)| player.clone()))
    }
    fn register_hint_icon(&mut self, player: &NodeHandle, node: &NodeHandle) -> R<()> {
        self.emit(json!(["hintIcon", self.label(player), self.label(node)]));
        Ok(())
    }
    fn clear_hint_icon(&mut self, player: &NodeHandle) -> R<()> {
        self.emit(json!(["clearHintIcon", self.label(player)]));
        Ok(())
    }
    fn set_varbit(&mut self, player: &NodeHandle, id: i32, value: i32) -> R<()> {
        self.emit(json!(["varbit", self.label(player), id, value]));
        Ok(())
    }
    fn open_interface(&mut self, player: &NodeHandle, id: i32) -> R<()> {
        self.emit(json!(["openInterface", self.label(player), id]));
        Ok(())
    }
    fn interface_text(&mut self, player: &NodeHandle, text: &str, iface: i32, child: i32) -> R<()> {
        self.emit(json!([
            "interfaceText",
            self.label(player),
            text,
            iface,
            child
        ]));
        Ok(())
    }
    fn play_audio(&mut self, player: &NodeHandle, id: i32) -> R<()> {
        self.emit(json!(["audio", self.label(player), id]));
        Ok(())
    }
    fn animate(&mut self, entity: &NodeHandle, id: i32) -> R<()> {
        self.emit(json!(["animate", self.label(entity), id]));
        Ok(())
    }
    fn prayer_points(&mut self, player: &NodeHandle) -> R<f64> {
        self.emit(json!(["prayerPoints", self.label(player)]));
        let ent = self.ent(player);
        Ok(ent
            .prayer
            .unwrap_or_else(|| ent.input.get("prayerPoints").map_or(0.0, unbits)))
    }
    fn set_prayer_points(&mut self, player: &NodeHandle, value: f64) -> R<()> {
        self.emit(json!(["setPrayerPoints", self.label(player), bits(value)]));
        self.ent_mut(player).prayer = Some(value);
        Ok(())
    }
    fn store_archive(&mut self, name: &str) -> R<()> {
        self.emit(json!(["archive", name]));
        Ok(())
    }
}

fn build(input: &Value) -> Model {
    let events: Events = Rc::default();
    let mut definitions = item_definitions::Registry::default();
    let mut plugins = BTreeMap::new();
    let mut equipment_plugins = BTreeMap::new();
    let empty = Map::new();
    for (id, fields) in input["items"].as_object().unwrap_or(&empty) {
        let id: i32 = id.parse().unwrap();
        let mut definition = ItemDefinition {
            id,
            value: fields.get("value").map_or(1, int),
            stackable: flag(fields, "stackable", false),
            unnoted: flag(fields, "unnoted", true),
            note_id: fields.get("noteId").map_or(-1, int),
            ..ItemDefinition::default()
        };
        definition.name = fields["name"].as_str().map(str::to_owned);
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
        for (key, value) in fields["handlers"].as_object().unwrap_or(&empty) {
            match key.as_str() {
                "wrapper" => {
                    plugins.insert(
                        id,
                        ItemPlugin {
                            id: int(&value["id"]),
                            death_item: value["deathItem"].clone(),
                        },
                    );
                }
                "equipment" => {
                    equipment_plugins.insert(id, int(value));
                }
                _ => {
                    let property = match value {
                        Value::Bool(value) => Property::Bool(*value),
                        Value::Number(value) => Property::Int(value.as_i64().unwrap() as i32),
                        other => panic!("handler {other}"),
                    };
                    definition.handlers.insert(key.clone(), property);
                }
            }
        }
        definitions.insert(id, Shared::new(definition));
    }
    let definitions = Rc::new(definitions);
    let factory = Rc::new(Factory::new(&definitions));
    let mut actors = State::default();
    let mut ents = Vec::new();
    let mut uid_map = Vec::new();
    let none = Vec::new();
    let fill = |container: &mut Container,
                slots: &Value,
                definitions: &item_definitions::Registry,
                model_ctx: &mut NoListeners| {
        for (slot, value) in slots.as_array().unwrap_or(&none).iter().enumerate() {
            if let Some(list) = value.as_array() {
                let charge = list.get(2).map_or(1000, int);
                let item =
                    Shared::new(Item::new(int(&list[0]), int(&list[1]), charge, definitions));
                container
                    .replace(Some(&item), slot as i32, false, definitions, model_ctx)
                    .unwrap();
            }
        }
    };
    for (index, fields) in input["players"]
        .as_array()
        .unwrap_or(&none)
        .iter()
        .enumerate()
    {
        let location = place(&fields["location"]);
        let node = actors.add(
            index as i32 + 1,
            location.clone().unwrap_or(Location::new(0, 0, 0)),
            [1, 2, 3, 4],
        );
        let handle = actors.retain_node(node).unwrap();
        let mut inventory = factory
            .create(28, ContainerType::Default, SortType::Id)
            .unwrap();
        let mut equipment = factory
            .create(14, ContainerType::Default, SortType::Id)
            .unwrap();
        fill(
            &mut inventory,
            &fields["inventory"],
            &definitions,
            &mut NoListeners,
        );
        fill(
            &mut equipment,
            &fields["equipment"],
            &definitions,
            &mut NoListeners,
        );
        let mut attributes = BTreeMap::new();
        for (key, value) in fields["attributes"].as_object().unwrap_or(&empty) {
            let value = match value {
                Value::Bool(value) => Attr::Bool(*value),
                Value::Number(value) => Attr::Int(value.as_i64().unwrap() as i32),
                Value::String(value) => Attr::Text(value.clone()),
                Value::Object(map) => Attr::Place(place(&map["loc"]).unwrap()),
                other => panic!("attribute {other}"),
            };
            attributes.insert(key.clone(), value);
        }
        if flag(fields, "online", true) {
            let uid = fields.get("uid").map_or(0, int);
            match uid_map.iter_mut().find(|(key, _)| *key == uid) {
                Some(slot) => slot.1 = handle.clone(),
                None => uid_map.push((uid, handle.clone())),
            }
        }
        ents.push(Ent {
            handle,
            label: format!("p{index}"),
            kind: Kind::Player,
            input: fields.clone(),
            attributes,
            location,
            impact: Shared::new(ImpactLog::default()),
            inventory: Some(inventory),
            equipment: Some(equipment),
            prayer: None,
            id: -1,
            ticking: false,
            grave: None,
        });
    }
    for (index, fields) in input["npcs"].as_array().unwrap_or(&none).iter().enumerate() {
        let location = place(&fields["location"]);
        let node = actors.add_npc(
            100 + index as i32,
            location.clone().unwrap_or(Location::new(0, 0, 0)),
            [1, 2, 3, 4],
        );
        ents.push(Ent {
            handle: actors.retain_node(node).unwrap(),
            label: format!("n{index}"),
            kind: Kind::Npc,
            input: fields.clone(),
            attributes: BTreeMap::new(),
            location,
            impact: Shared::new(ImpactLog::default()),
            inventory: None,
            equipment: None,
            prayer: None,
            id: fields.get("id").map_or(1, int),
            ticking: false,
            grave: None,
        });
    }
    Model {
        events,
        input: input.clone(),
        tick: input.get("tick").map_or(0, int),
        ents,
        definitions,
        factory,
        plugins,
        equipment_plugins,
        chunk: Shared::new(RegionChunk::new(Location::new(3200, 3200, 0), 0)),
        manager: gm::Manager::default(),
        controller: Controller::default(),
        uid_map,
        labels: Vec::new(),
        archive: Json::Object(Vec::new()),
        source: JavaSource::new(1, 2, 3),
        actors,
    }
}

/// The container services of the case set-up: no listener is registered.
struct NoListeners;
impl container::Context for NoListeners {
    fn update(
        &mut self,
        _: Listener,
        _: &mut Container,
        _: container::Event,
        _: &item_definitions::Registry,
    ) -> rs09_server::inventory::Result<()> {
        unreachable!()
    }
    fn refresh(
        &mut self,
        _: Listener,
        _: &mut Container,
        _: &item_definitions::Registry,
    ) -> rs09_server::inventory::Result<()> {
        unreachable!()
    }
    fn ground(
        &mut self,
        _: &mut Container,
        _: Option<item::Handle>,
        _: &item_definitions::Registry,
    ) -> rs09_server::inventory::Result<()> {
        unreachable!()
    }
}

fn slots(container: &Container) -> Value {
    Value::Array(
        container
            .items()
            .read()
            .iter()
            .map(|item| item.as_ref().map_or(Value::Null, triple))
            .collect(),
    )
}

impl Model {
    fn grave_of(&self, name: &Value) -> Option<grave::Handle> {
        let handle = self.named(name)?;
        self.ent(&handle).grave.clone()
    }
    fn player(&self, action: &Value) -> NodeHandle {
        self.named(&action["player"]).unwrap()
    }
    fn step(&mut self) -> death::Result<()> {
        self.tick = self.tick.wrapping_add(1);
        let order: Vec<String> = self.input["order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|phase| phase.as_str().unwrap().to_owned())
            .collect();
        for phase in order {
            match phase.as_str() {
                "graves" => {
                    let mut controller = std::mem::take(&mut self.controller);
                    let result = controller.tick(self);
                    self.controller = controller;
                    result?;
                }
                "ground" => {
                    let manager = self.manager.clone();
                    gm::pulse(self, &manager)?;
                }
                "npcs" => {
                    let graves: Vec<grave::Handle> = self
                        .ents
                        .iter()
                        .filter(|ent| ent.kind == Kind::Grave && ent.ticking)
                        .filter_map(|ent| ent.grave.clone())
                        .collect();
                    for grave in graves {
                        grave::npc_tick(self, &grave)?;
                    }
                }
                other => panic!("phase {other}"),
            }
        }
        Ok(())
    }
    fn task(&self, a: &Value) -> Task {
        let entity = self.player(a);
        let killer = self.named(&a["killer"]).unwrap_or_else(|| entity.clone());
        Task {
            pulse: Shared::new(pulses::Pulse::new(
                1,
                Some(Shared::new(vec![Some(entity.node)])),
                pulses::Class::Other,
                npc_death::PULSE_CLASS.into(),
            )),
            entity,
            killer,
        }
    }
    fn with_controller<T>(
        &mut self,
        body: impl FnOnce(&mut Self, &mut Controller) -> death::Result<T>,
    ) -> death::Result<T> {
        let mut controller = std::mem::take(&mut self.controller);
        let result = body(self, &mut controller);
        self.controller = controller;
        result
    }

    fn act(&mut self, a: &Value) -> death::Result<Value> {
        let definitions = self.definitions.clone();
        let factory = self.factory.clone();
        match a["op"].as_str().unwrap() {
            "containers" => {
                let player = self.player(a);
                let [kept, lost] = death::get_containers(self, &player, &factory, &definitions)?;
                Ok(json!({"kept": slots(&kept), "lost": slots(&lost)}))
            }
            "commence" => {
                let (player, killer) = (self.player(a), self.named(&a["killer"]));
                death::commence_death(self, &player, killer.as_ref())?;
                Ok(Value::Null)
            }
            "finalize" => {
                let (player, killer) = (self.player(a), self.named(&a["killer"]));
                self.with_controller(|model, controller| {
                    death::finalize_death(
                        model,
                        controller,
                        &player,
                        killer.as_ref(),
                        &factory,
                        &definitions,
                    )
                })?;
                Ok(Value::Null)
            }
            "start" => {
                let task = self.task(a);
                npc_death::task_start(self, &task)?;
                Ok(Value::Null)
            }
            "exec" => {
                let task = self.task(a);
                Ok(json!(npc_death::task_pulse(self, &task)?))
            }
            "stop" => {
                let task = self.task(a);
                npc_death::task_stop(self, &task)?;
                Ok(Value::Null)
            }
            "tick" => {
                self.step()?;
                Ok(Value::Null)
            }
            "run" => {
                for _ in 0..int(&a["ticks"]) {
                    self.emit(json!(["tick", self.tick.wrapping_add(1)]));
                    self.step()?;
                }
                Ok(Value::Null)
            }
            "set" => {
                self.tick = int(&a["tick"]);
                Ok(Value::Null)
            }
            op @ ("get" | "take") => {
                let player = self.named(&a["player"]);
                let found = gm::get(self, int(&a["id"]), place(&a["location"]), player)?;
                if let Some(found) = &found
                    && op == "take"
                {
                    let manager = self.manager.clone();
                    gm::destroy(self, &manager, Some(found.clone()))?;
                }
                Ok(found.map_or(Value::Null, |found| json!(self.ground_label(&found))))
            }
            op @ ("read" | "bless" | "repair" | "demolish") => {
                let player = self.player(a);
                let grave = self.grave_of(&a["grave"]);
                let result = match op {
                    "read" => grave::on_read(self, &player, grave.as_ref())?,
                    "bless" => grave::on_bless(self, &player, grave.as_ref())?,
                    "repair" => grave::on_repair(self, &player, grave.as_ref())?,
                    _ => self.with_controller(|model, controller| {
                        grave::on_demolish(model, controller, &player, grave.as_ref())
                    })?,
                };
                Ok(json!(result))
            }
            "logout" => {
                let player = self.player(a);
                let uid = self.input_of(&player).get("uid").map_or(0, int);
                self.uid_map.retain(|(key, _)| *key != uid);
                self.ent_mut(&player).input["playing"] = json!(false);
                Ok(Value::Null)
            }
            "login" => {
                let player = self.player(a);
                let uid = self.input_of(&player).get("uid").map_or(0, int);
                match self.uid_map.iter_mut().find(|(key, _)| *key == uid) {
                    Some(slot) => slot.1 = player.clone(),
                    None => self.uid_map.push((uid, player.clone())),
                }
                self.ent_mut(&player).input["playing"] = json!(true);
                Ok(Value::Null)
            }
            "setPlayer" => {
                let player = self.player(a);
                let key = a["key"].as_str().unwrap().to_owned();
                if key == "location" {
                    self.ent_mut(&player).location = place(&a["value"]);
                }
                self.ent_mut(&player).input[key] = a["value"].clone();
                Ok(Value::Null)
            }
            "setGrave" => {
                let grave = self.grave_of(&a["grave"]).unwrap();
                if let Some(value) = a.get("ticksRemaining") {
                    grave.write().ticks_remaining = int(value);
                }
                if let Some(value) = a.get("ticksSinceDeath") {
                    grave.write().ticks_since_death = int(value);
                }
                Ok(Value::Null)
            }
            "addTime" => {
                let grave = self.grave_of(&a["grave"]).unwrap();
                grave::add_time(self, &grave, int(&a["ticks"]))?;
                Ok(Value::Null)
            }
            "collapse" => {
                let grave = self.grave_of(&a["grave"]).unwrap();
                self.with_controller(|model, controller| {
                    grave::collapse(model, controller, &grave)
                })?;
                Ok(Value::Null)
            }
            "save" => {
                let archive =
                    self.with_controller(|model, controller| store::serialize(model, controller))?;
                let text = store::to_json_simple(&archive);
                self.archive = archive;
                Ok(json!(text))
            }
            "parse" => {
                self.archive = store::parse_text(a["archive"].as_str().unwrap()).unwrap();
                let archive = self.archive.clone();
                self.with_controller(|model, controller| {
                    store::deserialize(model, controller, &archive, &definitions)
                })?;
                Ok(Value::Null)
            }
            "deathButton" => {
                let player = self.player(a);
                Ok(json!(death::death_interface_button(
                    self,
                    &player,
                    int(&a["button"])
                )?))
            }
            "graveType" => {
                let player = self.player(a);
                Ok(json!(grave::grave_type(self, &player)?.ordinal()))
            }
            "allow" => {
                let player = self.player(a);
                Ok(json!(grave::allow_generate(self, &player)?))
            }
            "crumble" => Ok(json!(grave::should_crumble(self, int(&a["id"]))?)),
            "release" => Ok(json!(grave::should_release(self, int(&a["id"]))?)),
            "checkTransform" => {
                let item = self.item_from(&a["item"]);
                Ok(triple(&grave::check_transform(self, &item)?))
            }
            "barrows" => {
                let item = self.item_from(&a["item"]);
                let id = item.read().id();
                let reduced = barrows::grave_death_durability_reduction(&item, &definitions)
                    .map_err(|_| Error::Death(DropError::Index))?;
                Ok(json!({
                    "is": barrows::is_barrows(id),
                    "repaired": barrows::is_fully_repaired(id),
                    "broken": barrows::is_broken(id),
                    "brokenId": barrows::broken_id(id),
                    "reduced": reduced.as_ref().map_or(Value::Null, triple),
                }))
            }
            "types" => {
                let rows: Vec<Value> = GraveType::ALL
                    .iter()
                    .map(|kind| {
                        json!([
                            kind.name(),
                            kind.npc_id(),
                            kind.cost(),
                            kind.duration_minutes(),
                            kind.members(),
                            kind.required_quest(),
                            kind.text()
                        ])
                    })
                    .collect();
                Ok(json!([rows, GraveType::ids()]))
            }
            other => panic!("op {other}"),
        }
    }

    fn state(&mut self) -> Value {
        let mut players = Vec::new();
        for ent in self.ents.iter().filter(|ent| ent.kind == Kind::Player) {
            let attributes: Map<String, Value> = ent
                .attributes
                .iter()
                .map(|(k, v)| (k.clone(), v.json()))
                .collect();
            players.push(json!({
                "inventory": slots(ent.inventory.as_ref().unwrap()),
                "equipment": slots(ent.equipment.as_ref().unwrap()),
                "attributes": attributes,
                "disabled": ent.impact.read().disabled_ticks,
                "prayerPoints": ent.prayer.map_or(Value::Null, |value| json!(bits(value))),
            }));
        }
        let manager: Vec<ground::Handle> = self
            .manager
            .get_items()
            .read()
            .iter()
            .flatten()
            .cloned()
            .collect();
        let manager: Vec<Value> = manager
            .iter()
            .map(|item| json!(self.ground_label(item)))
            .collect();
        let chunk: Vec<ground::Handle> = chunk_sync::get_items(&self.chunk)
            .read()
            .iter()
            .flatten()
            .cloned()
            .collect();
        let chunk: Vec<Value> = chunk
            .iter()
            .map(|item| json!(self.ground_label(item)))
            .collect();
        let active: Vec<Value> = self
            .controller
            .entries()
            .iter()
            .map(|(uid, grave)| {
                let npc = grave.read().npc.clone();
                json!([uid, self.label(&npc)])
            })
            .collect();
        let mut graves = Vec::new();
        let grave_ents: Vec<usize> = (0..self.ents.len())
            .filter(|i| self.ents[*i].kind == Kind::Grave)
            .collect();
        for index in grave_ents {
            let grave = self.ents[index].grave.clone().unwrap();
            let items: Vec<ground::Handle> = grave.read().items();
            let items: Vec<Value> = items
                .iter()
                .map(|item| json!(self.ground_label(item)))
                .collect();
            let ent = &self.ents[index];
            let g = grave.read();
            let attributes: Map<String, Value> = ent
                .attributes
                .iter()
                .map(|(k, v)| (k.clone(), v.json()))
                .collect();
            graves.push(json!({
                "id": ent.id,
                "location": coordinates(ent.location.as_ref()),
                "owner": g.owner_uid,
                "username": g.owner_username,
                "remaining": g.ticks_remaining,
                "since": g.ticks_since_death,
                "type": g.kind().ok().map(GraveType::name),
                "items": items,
                "attributes": attributes,
            }));
        }
        let mut grounds = Vec::new();
        for (item, _) in &self.labels {
            let g = item.read();
            let dropper = g
                .dropper
                .as_ref()
                .map_or(Value::Null, |d| json!(self.label(d)));
            grounds.push(json!([
                g.item.read().id(),
                g.item.read().amount(),
                g.item.read().charge(),
                coordinates(g.location.as_ref()),
                dropper,
                g.dropper_uid,
                g.ticks,
                g.decay_time,
                g.remain_private,
                g.removed
            ]));
        }
        let npcs: Vec<Value> = self
            .ents
            .iter()
            .filter(|ent| ent.kind == Kind::Npc)
            .map(|ent| {
                Value::Object(
                    ent.attributes
                        .iter()
                        .map(|(k, v)| (k.clone(), v.json()))
                        .collect(),
                )
            })
            .collect();
        json!({
            "tick": self.tick,
            "players": players,
            "manager": manager,
            "chunk": chunk,
            "active": active,
            "graves": graves,
            "grounds": grounds,
            "npcs": npcs,
        })
    }
}

/// Replays one case and returns, per action, what the oracle records.
pub fn evaluate(case: &Value) -> Value {
    let mut model = build(case);
    let mut steps = Vec::new();
    for action in case["actions"].as_array().unwrap() {
        model.events.borrow_mut().clear();
        let (result, error) = match model.act(action) {
            Ok(result) => (result, Value::Null),
            Err(error) => (Value::Null, json!(category(&error))),
        };
        let state = model.state();
        chunk_sync::reset_flags(&model.chunk);
        let events = model.events.borrow().clone();
        steps.push(json!({"result": result, "error": error, "events": events, "state": state}));
    }
    Value::Array(steps)
}
