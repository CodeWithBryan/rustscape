//! Eating and drinking: core/game/consumable (Consumable, Food, Drink, Potion,
//! Cake, HalfableFood, BarbarianMix, FakeConsumable, ConsumableEffect), the
//! `Consumables` table, every effect class and ConsumableListener's eat/drink
//! handler with its clocks.
//!
//! The library runs against one player through [`Host`]. Skills and run
//! energy are the native `player::skills::Skills` and `player::settings::Settings`;
//! clocks are the entity's own array; every other outside call (messages,
//! animations, audio, containers, timers, the script queue, pulses, hits,
//! chat, teleports, diaries, the familiar, attributes, the tick and the random
//! draws) is a required host service with no default. `join` answers the timer
//! services from the native engine timers (timers::content).
pub mod effects;
pub mod join;
pub mod table;

use crate::player::{
    settings::{self, Settings},
    skills::{self, Skills},
};
use crate::world::combat_pulse::HitsplatType;
pub use effects::{Diary, Effect, TimerArg};
use std::collections::HashMap;
pub use table::Row;

/// The original exceptions: `NullPointerException`, `ClassCastException`,
/// `IndexOutOfBoundsException`, `NumberFormatException`, and a failing host
/// binding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Null,
    Type,
    Index,
    Number,
    Binding(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn skills_error(error: skills::Error) -> Error {
    match error {
        skills::Error::Index => Error::Index,
        skills::Error::Null => Error::Null,
        skills::Error::Type => Error::Type,
        skills::Error::Number => Error::Number,
        skills::Error::Binding => Error::Binding("skills".into()),
    }
}

pub(crate) fn settings_error(error: settings::Error) -> Error {
    match error {
        settings::Error::Index => Error::Index,
        settings::Error::Null => Error::Null,
        settings::Error::Type => Error::Type,
        settings::Error::Number => Error::Number,
        settings::Error::Binding => Error::Binding("settings".into()),
    }
}

/// `Clocks` indices (`entity.clocks`).
pub const MOVEMENT: usize = 0;
pub const ANIMATION_END: usize = 1;
pub const NEXT_EAT: usize = 2;
pub const NEXT_CONSUME: usize = 3;
pub const NEXT_DRINK: usize = 4;
pub const NEXT_ATTACK: usize = 5;

/// `Sounds.EAT_2393`, the drink sound 4580 and `Sounds.LIQUID_2401`.
pub const EAT_SOUND: i32 = 2393;
pub const DRINK_SOUND: i32 = 4580;
pub const LIQUID_SOUND: i32 = 2401;
/// Potion's `VIAL`.
pub const VIAL: i32 = 229;

/// An attribute value as `Entity.getAttribute`/`setAttribute` see it.
#[derive(Clone, Debug, PartialEq)]
pub enum Attr {
    Null,
    Int(i32),
    Bool(bool),
    Str(String),
    /// A value of any other class, by simple name.
    Other(String),
}

/// `Log` levels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Log {
    Fine,
    Info,
    Warn,
    Err,
    Debug,
}

impl Log {
    pub fn name(self) -> &'static str {
        match self {
            Self::Fine => "FINE",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Err => "ERR",
            Self::Debug => "DEBUG",
        }
    }
}

/// `QueueStrength`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueStrength {
    Weak,
    Normal,
    Strong,
    Soft,
}

impl QueueStrength {
    pub fn name(self) -> &'static str {
        match self {
            Self::Weak => "WEAK",
            Self::Normal => "NORMAL",
            Self::Strong => "STRONG",
            Self::Soft => "SOFT",
        }
    }
}

/// The containers `replaceSlot`/`removeItem` reach here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContainerKind {
    Inventory,
    Equipment,
}

impl ContainerKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Inventory => "inventory",
            Self::Equipment => "equipment",
        }
    }
}

/// An `Item`: the constructor's id (whose definition supplies the name), the
/// amount and the slot (`index`, -1 until a container sets it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub definition: i32,
    pub amount: i32,
    pub slot: i32,
}

impl Item {
    /// `new Item(id, amount)` (charge 1000), not yet in a slot.
    pub fn new(id: i32, amount: i32) -> Self {
        Self {
            definition: id,
            amount,
            slot: -1,
        }
    }

    pub fn at(id: i32, amount: i32, slot: i32) -> Self {
        Self {
            definition: id,
            amount,
            slot,
        }
    }

    /// `getId()`: `idHash >> 16 & 0xFFFF` of `id << 16 | charge`.
    pub fn id(&self) -> i32 {
        ((self.definition << 16 | 1000) >> 16) & 0xFFFF
    }

    /// `Item.toString()`.
    pub fn describe(&self, name: &str) -> String {
        format!(
            "Item id={}, name={}, amount={}",
            self.id(),
            name,
            self.amount
        )
    }
}

/// The node an item option names: an `Item` or any other node with an id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    Item(Item),
    Other { id: i32 },
}

impl Node {
    pub fn id(&self) -> i32 {
        match self {
            Self::Item(item) => item.id(),
            Self::Other { id } => *id,
        }
    }
}

/// The class of a `Consumable`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Food,
    Drink,
    Potion,
    Cake,
    HalfableFood,
    BarbarianMix,
    Fake,
}

impl Kind {
    /// The class's simple name.
    pub fn class_name(self) -> &'static str {
        match self {
            Self::Food => "Food",
            Self::Drink => "Drink",
            Self::Potion => "Potion",
            Self::Cake => "Cake",
            Self::HalfableFood => "HalfableFood",
            Self::BarbarianMix => "BarbarianMix",
            Self::Fake => "FakeConsumable",
        }
    }
}

/// One consumable object.
#[derive(Clone, Debug, PartialEq)]
pub struct Consumable {
    pub kind: Kind,
    pub ids: Vec<i32>,
    pub effect: Option<Effect>,
    pub animation: Option<i32>,
    pub messages: Vec<String>,
}

/// The potion's delayed dose message: `queueScript(player, 2, SOFT, false)`'s
/// lambda, which captures `dosesLeft`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DoseScript {
    pub doses_left: i32,
}

/// TroubleBrewingRumEffect's two pulses: `mainPulse` (delay 4) and the
/// `teleportation` pulse (delay 6) it runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RumPulse {
    Main { chat: String },
    Teleport,
}

/// The location the rum teleports to.
pub const TROUBLE_BREWING: [i32; 3] = [3813, 3022, 0];

impl RumPulse {
    pub fn main(chat: &str) -> Self {
        Self::Main { chat: chat.into() }
    }

    /// The pulse's construction delay.
    pub fn delay(&self) -> i32 {
        match self {
            Self::Main { .. } => 4,
            Self::Teleport => 6,
        }
    }

    /// `pulse()`: both return true.
    pub fn pulse<H: Host>(&self, host: &mut H) -> Result<bool> {
        match self {
            Self::Main { chat } => {
                host.send_chat(chat)?;
                host.run_pulse(Self::Teleport)?;
                Ok(true)
            }
            Self::Teleport => {
                host.teleport(TROUBLE_BREWING)?;
                Ok(true)
            }
        }
    }
}

/// Everything the consumables reach outside themselves, for one player. The
/// first group are plain accessors to state, not calls. Every other method is
/// one original call; a failure leaves the earlier effects in place.
pub trait Host {
    /// The binding of the native skills to this player's world.
    type SkillsContext: skills::Context;
    /// The binding of the native settings to this player's world.
    type SettingsContext: settings::Context;
    /// A timer handle (`RSTimer`).
    type Timer: Clone;
    /// The summoned familiar.
    type Familiar;

    /// `player.getSkills()` and the context its members need.
    fn skills(&mut self) -> (&mut Skills, &mut Self::SkillsContext);
    /// `player.getSettings()` (a shared handle) and its context.
    fn settings(&mut self) -> (Settings, &mut Self::SettingsContext);
    /// `entity.clocks`, the array itself.
    fn clocks(&mut self) -> &mut [i32; 10];
    /// The definition name an `Item` constructor reads for this id
    /// (`ItemDefinition.forId(id).getName()`).
    fn item_name(&self, definition: i32) -> String;
    /// `Disease.hitsLeft` of a disease timer (a field read).
    fn disease_hits_left(&mut self, timer: &Self::Timer) -> Result<i32>;
    /// `Disease.hitsLeft = value` (a field write).
    fn set_disease_hits_left(&mut self, timer: &Self::Timer, value: i32) -> Result<()>;

    /// `GameWorld.ticks`.
    fn ticks(&mut self) -> Result<i32>;
    /// `player.getAttribute(key, default)`.
    fn attribute(&mut self, key: &str, default: &Attr) -> Result<Attr>;
    /// `player.setAttribute(key, value)`.
    fn set_attribute(&mut self, key: &str, value: Attr) -> Result<()>;
    /// `player.animate(animation)`.
    fn animate(&mut self, animation: Option<i32>) -> Result<bool>;
    /// `playAudio(player, id, delay, loops)` (no location, radius 8).
    fn play_audio(&mut self, id: i32, delay: i32, loops: i32) -> Result<()>;
    /// `player.getPacketDispatch().sendMessage(text)`.
    fn packet_message(&mut self, text: &str) -> Result<()>;
    /// ContentAPI `sendMessage(player, text)`.
    fn send_message(&mut self, text: &str) -> Result<()>;
    /// `player.getImpactHandler().manualHit(player, hit, type)`.
    fn manual_hit(&mut self, hit: i32, kind: HitsplatType) -> Result<()>;
    /// `player.sendChat(text)`.
    fn send_chat(&mut self, text: &str) -> Result<bool>;
    /// `player.teleport(location)`.
    fn teleport(&mut self, location: [i32; 3]) -> Result<()>;
    /// `player.getPulseManager().run(pulse)`; the host calls
    /// [`RumPulse::pulse`] when the pulse fires.
    fn run_pulse(&mut self, pulse: RumPulse) -> Result<()>;
    /// `player.getAchievementDiaryManager().finishTask(player, diary, level, task)`.
    fn finish_task(&mut self, diary: Diary, level: i32, task: i32) -> Result<()>;
    /// `player.familiarManager.familiar`.
    fn familiar(&mut self) -> Result<Option<Self::Familiar>>;
    /// `familiar.updateSpecialPoints(diff)`.
    fn update_special_points(&mut self, familiar: &Self::Familiar, diff: i32) -> Result<()>;
    /// `container.replace(item, slot)`: the previous item.
    fn replace(
        &mut self,
        container: ContainerKind,
        item: Option<Item>,
        slot: i32,
    ) -> Result<Option<Item>>;
    /// `container.remove(item)`.
    fn remove(&mut self, container: ContainerKind, item: &Item) -> Result<bool>;
    /// `container.remove(item, slot, fireListener)`.
    fn remove_slot(
        &mut self,
        container: ContainerKind,
        item: &Item,
        slot: i32,
        fire: bool,
    ) -> Result<bool>;
    /// `container.contains(id, amount)`.
    fn contains(&mut self, container: ContainerKind, id: i32, amount: i32) -> Result<bool>;
    /// `PlayerMonitor.log(player, LogType.DUPE_ALERT, details)`.
    fn dupe_alert(&mut self, details: &str) -> Result<()>;
    /// `spawnTimer(identifier, *args)`: `None` when the registry has none.
    fn spawn_timer(&mut self, identifier: &str, args: &[TimerArg]) -> Result<Option<Self::Timer>>;
    /// `registerTimer(player, timer)`.
    fn register_timer(&mut self, timer: Option<&Self::Timer>) -> Result<()>;
    /// `removeTimer(player, identifier)`.
    fn remove_timer(&mut self, identifier: &str) -> Result<()>;
    /// `getTimer<Disease>(player)`.
    fn disease(&mut self) -> Result<Option<Self::Timer>>;
    /// `removeTimer<Disease>(player)`.
    fn remove_disease(&mut self) -> Result<()>;
    /// `entity.scripts.getActiveScript()?.persist`: `None` without an active script.
    fn active_script_persist(&mut self) -> Result<Option<bool>>;
    /// `entity.scripts.addToQueue(script, strength)` of the `QueuedScript` that
    /// ContentAPI `queueScript` built.
    fn add_to_queue(
        &mut self,
        script: DoseScript,
        strength: QueueStrength,
        next_execution: i32,
        persist: bool,
    ) -> Result<()>;
    /// `entity.scripts.reset()`.
    fn reset_scripts(&mut self) -> Result<()>;
    /// `log(origin, level, message)`.
    fn log(&mut self, level: Log, message: &str) -> Result<()>;
    /// `entity.properties.combatPulse.delayNextAttack(ticks)`.
    fn delay_next_attack(&mut self, ticks: i32) -> Result<()>;
    /// One `RandomFunction.RANDOM.nextInt(bound)` draw; `bound` is positive.
    fn next_int(&mut self, bound: i32) -> Result<i32>;
}

pub(crate) fn skills_call<H: Host, T>(
    host: &mut H,
    f: impl FnOnce(&mut Skills, &mut H::SkillsContext) -> skills::Result<T>,
) -> Result<T> {
    let (s, c) = host.skills();
    f(s, c).map_err(skills_error)
}

/// `getLifepoints()`.
pub(crate) fn lifepoints<H: Host>(host: &mut H) -> i32 {
    host.skills().0.lifepoints
}

/// `getMaximumLifepoints()`.
pub(crate) fn maximum_lifepoints<H: Host>(host: &mut H) -> Result<i32> {
    skills_call(host, |s, c| s.maximum_lifepoints(c))
}

/// `getStaticLevel(slot)`.
pub(crate) fn static_level<H: Host>(host: &mut H, slot: i32) -> Result<i32> {
    let s = host.skills().0;
    usize::try_from(slot)
        .ok()
        .and_then(|i| s.static_levels.get(i).copied())
        .ok_or(Error::Index)
}

/// `getLevel(slot)` (with its assist read).
pub(crate) fn level<H: Host>(host: &mut H, slot: i32) -> Result<i32> {
    skills_call(host, |s, c| s.level(slot, false, c))
}

/// `RandomFunction.random(maxValue)`.
pub(crate) fn random<H: Host>(host: &mut H, max: i32) -> Result<i32> {
    if max <= 0 {
        return Ok(0);
    }
    host.next_int(max)
}

/// `RandomFunction.random(a, b)`.
pub(crate) fn random_between<H: Host>(host: &mut H, a: i32, b: i32) -> Result<i32> {
    let n = b.wrapping_sub(a).wrapping_abs();
    let r = if n == 0 { 0 } else { random(host, n)? };
    Ok(a.min(b).wrapping_add(r))
}

/// Java `String.trim()`: characters up to U+0020 at either end.
fn java_trim(text: &str) -> &str {
    text.trim_matches(|c: char| c <= ' ')
}

fn lower(text: &str) -> String {
    text.to_lowercase()
}

/// The `Consumables` table with its `consumables` map (`putIfAbsent` per id
/// in declaration order) and `potions` list.
pub struct Table {
    pub rows: Vec<Row>,
    by_id: HashMap<i32, usize>,
    pub potions: Vec<i32>,
}

impl Default for Table {
    fn default() -> Self {
        Self::new()
    }
}

impl Table {
    /// The enum's static initialiser.
    pub fn new() -> Self {
        let rows = table::rows();
        let mut by_id = HashMap::new();
        let mut potions = Vec::new();
        for (index, row) in rows.iter().enumerate() {
            for &id in &row.consumable.ids {
                by_id.entry(id).or_insert(index);
            }
            if row.consumable.kind == Kind::Potion || row.consumable.kind == Kind::BarbarianMix {
                potions.extend(row.consumable.ids.iter().copied());
            }
        }
        Self {
            rows,
            by_id,
            potions,
        }
    }

    /// `Consumables.getConsumableById(id)`.
    pub fn lookup(&self, id: i32) -> Option<usize> {
        self.by_id.get(&id).copied()
    }

    /// `Consumables.valueOf(name)`.
    pub fn value_of(&self, name: &str) -> Option<usize> {
        self.rows.iter().position(|r| r.name == name)
    }
}

impl Consumable {
    /// `getNextItemId(id)`.
    pub fn next_item_id(&self, current: i32) -> i32 {
        let last = self.ids.len().wrapping_sub(1);
        for (i, &id) in self.ids.iter().enumerate() {
            if id == current && i != last {
                return self.ids[i + 1];
            }
        }
        -1
    }

    /// `getFormattedName(item)` of the class.
    pub fn formatted_name(&self, name: &str) -> String {
        match self.kind {
            Kind::Drink | Kind::Potion | Kind::BarbarianMix => {
                let mut text = name.to_string();
                for token in [
                    "(4)", "(3)", "(2)", "(1)", "(m4)", "(m3)", "(m2)", "(m1)", "(m)",
                ] {
                    text = text.replace(token, "");
                }
                lower(java_trim(&text))
            }
            Kind::HalfableFood => {
                let text = name
                    .replace("1/2", "")
                    .replace("Half an", "")
                    .replace("Half a", "");
                lower(java_trim(&text))
            }
            _ => {
                let mut text = name.to_string();
                for token in ["(4)", "(3)", "(2)", "(1)"] {
                    text = text.replace(token, "");
                }
                lower(java_trim(&text))
            }
        }
    }

    /// `getHealthEffectValue(player)`.
    pub fn health_value<H: Host>(&self, host: &mut H) -> Result<i32> {
        self.effect.as_ref().ok_or(Error::Null)?.health_value(host)
    }

    /// `Potion.getDose(potion)`.
    pub fn dose(&self, item: &Item, name: &str) -> Result<i32> {
        for (i, &id) in self.ids.iter().enumerate() {
            if id == item.id() {
                return Ok(self.ids.len() as i32 - i as i32);
            }
        }
        let digits: String = name
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        java_parse_int(&digits)
    }

    fn execute_consumption_actions<H: Host>(&self, host: &mut H) -> Result<()> {
        let sound = match self.kind {
            Kind::Fake => return Ok(()),
            Kind::Food | Kind::Cake | Kind::HalfableFood => EAT_SOUND,
            Kind::Drink => DRINK_SOUND,
            Kind::Potion | Kind::BarbarianMix => LIQUID_SOUND,
        };
        host.animate(self.animation)?;
        host.play_audio(sound, 0, 1)
    }

    fn healing_message<H: Host>(&self, host: &mut H, initial: i32) -> Result<()> {
        if lifepoints(host) > initial {
            host.packet_message("It heals some health.")?;
        }
        Ok(())
    }

    fn custom_messages<H: Host>(&self, host: &mut H) -> Result<()> {
        for message in &self.messages {
            host.packet_message(message)?;
        }
        Ok(())
    }

    fn food_default<H: Host>(&self, host: &mut H, item: &Item) -> Result<()> {
        let name = self.formatted_name(&host.item_name(item.definition));
        host.packet_message(&format!("You eat the {name}."))
    }

    fn default_messages<H: Host>(&self, host: &mut H, item: &Item) -> Result<()> {
        match self.kind {
            Kind::Food | Kind::Cake => self.food_default(host, item),
            Kind::HalfableFood => {
                let name = self.formatted_name(&host.item_name(item.definition));
                if item.id() == *self.ids.first().ok_or(Error::Index)? {
                    host.packet_message(&format!("You eat half the {name}."))
                } else if item.id() == *self.ids.get(1).ok_or(Error::Index)? {
                    host.packet_message(&format!("You eat the remaining {name}."))
                } else {
                    self.food_default(host, item)
                }
            }
            Kind::Drink => {
                let name = self.formatted_name(&host.item_name(item.definition));
                host.packet_message(&format!("You drink the {name}."))
            }
            Kind::BarbarianMix => host.packet_message("You drink the lumpy potion."),
            Kind::Potion => {
                let mut consumed = 1i32;
                let mut i = 0usize;
                while *self.ids.get(i).ok_or(Error::Index)? != item.id() {
                    consumed = consumed.wrapping_add(1);
                    i += 1;
                }
                let doses_left = (self.ids.len() as i32).wrapping_sub(consumed);
                let name = self.formatted_name(&host.item_name(item.definition));
                host.packet_message(&format!("You drink some of your {name}."))?;
                queue_script(
                    host,
                    2,
                    QueueStrength::Soft,
                    false,
                    DoseScript { doses_left },
                )
            }
            Kind::Fake => self.custom_messages(host),
        }
    }

    /// `consume(item, player)` of the class. `table` answers the effect
    /// lookup `Consumables.getConsumableById(item.getId())`.
    pub fn consume<H: Host>(&self, table: &Table, item: &Item, host: &mut H) -> Result<()> {
        match self.kind {
            Kind::Fake => return self.default_messages(host, item),
            Kind::Potion | Kind::BarbarianMix => {
                self.execute_consumption_actions(host)?;
                let next = self.next_item_id(item.id());
                let replacement = if next != -1 {
                    Item::new(next, 1)
                } else {
                    Item::new(VIAL, 1)
                };
                host.replace(ContainerKind::Inventory, Some(replacement), item.slot)?;
                let initial = lifepoints(host);
                activate_by_id(table, item, host)?;
                if self.messages.is_empty() {
                    self.default_messages(host, item)?;
                } else {
                    self.custom_messages(host)?;
                }
                return self.healing_message(host, initial);
            }
            Kind::Cake => {
                self.execute_consumption_actions(host)?;
                let next = self.next_item_id(item.id());
                if next != -1 {
                    host.replace(
                        ContainerKind::Inventory,
                        Some(Item::new(next, 1)),
                        item.slot,
                    )?;
                } else {
                    host.remove(ContainerKind::Inventory, item)?;
                }
                let initial = lifepoints(host);
                activate_by_id(table, item, host)?;
                if self.messages.is_empty() {
                    self.default_messages(host, item)?;
                } else {
                    let mut i = 0usize;
                    while *self.ids.get(i).ok_or(Error::Index)? != item.id() {
                        i += 1;
                    }
                    let message = self.messages.get(i).ok_or(Error::Index)?.clone();
                    host.packet_message(&message)?;
                }
                return self.healing_message(host, initial);
            }
            _ => {}
        }
        self.execute_consumption_actions(host)?;
        let next = self.next_item_id(item.id());
        if self.ids.len() == 1 {
            let replacement = Item::new(item.id(), item.amount.wrapping_sub(1));
            replace_slot(
                host,
                item.slot,
                replacement,
                Some(item),
                ContainerKind::Inventory,
            )?;
        } else {
            replace_slot(
                host,
                item.slot,
                Item::new(next, 1),
                Some(item),
                ContainerKind::Inventory,
            )?;
        }
        let initial = lifepoints(host);
        activate_by_id(table, item, host)?;
        if self.messages.is_empty() {
            self.default_messages(host, item)?;
            self.healing_message(host, initial)
        } else {
            self.custom_messages(host)
        }
    }
}

/// `Consumables.getConsumableById(item.getId()).getConsumable().effect.activate(player)`.
fn activate_by_id<H: Host>(table: &Table, item: &Item, host: &mut H) -> Result<()> {
    let row = table.lookup(item.id()).ok_or(Error::Null)?;
    let effect = table.rows[row]
        .consumable
        .effect
        .clone()
        .ok_or(Error::Null)?;
    effect.activate(host)
}

/// Java `Integer.parseInt` of ASCII digits and dots.
fn java_parse_int(text: &str) -> Result<i32> {
    if text.is_empty() || text.contains('.') {
        return Err(Error::Number);
    }
    text.parse::<i32>().map_err(|_| Error::Number)
}

/// ContentAPI `replaceSlot(player, slot, item, currentItem, container)`.
pub fn replace_slot<H: Host>(
    host: &mut H,
    slot: i32,
    item: Item,
    current: Option<&Item>,
    container: ContainerKind,
) -> Result<Option<Item>> {
    if item.id() == 65535 || item.amount <= 0 {
        return host.replace(container, None, slot);
    }
    let Some(current) = current else {
        return host.replace(container, Some(item), slot);
    };
    if host.remove_slot(container, current, slot, true)? {
        return Ok(host
            .replace(container, Some(item), slot)?
            .or_else(|| Some(current.clone())));
    }
    let name = host.item_name(item.definition);
    host.dupe_alert(&format!(
        "Potential slot-replacement-based dupe attempt, slot: {slot}, item: {}",
        item.describe(&name)
    ))?;
    let other = match container {
        ContainerKind::Inventory => ContainerKind::Equipment,
        _ => ContainerKind::Inventory,
    };
    // removeItem(player, currentItem, other).
    if host.remove(other, current)? {
        return Ok(host
            .replace(container, Some(item), slot)?
            .or_else(|| Some(current.clone())));
    }
    Ok(None)
}

/// ContentAPI `queueScript(entity, delay, strength, persist, script)`.
pub fn queue_script<H: Host>(
    host: &mut H,
    delay: i32,
    strength: QueueStrength,
    persist: bool,
    script: DoseScript,
) -> Result<()> {
    let next = host.ticks()?.wrapping_add(delay);
    host.add_to_queue(script, strength, next, persist)
}

/// ContentAPI `stopExecuting(entity)`.
pub fn stop_executing<H: Host>(host: &mut H) -> Result<bool> {
    if host.active_script_persist()? == Some(true) {
        // log(entity.scripts.getActiveScript()!!::class.java, ...) reads the script again.
        host.active_script_persist()?.ok_or(Error::Null)?;
        host.log(
            Log::Err,
            "Tried to call stopExecuting() on a persistent script! To halt execution of a persistent script, you MUST call clearScripts()!",
        )?;
        host.reset_scripts()?;
        return Ok(true);
    }
    Ok(true)
}

impl DoseScript {
    /// The lambda: the remaining-doses message, then `stopExecuting(player)`.
    pub fn run<H: Host>(&self, host: &mut H, _stage: i32) -> Result<bool> {
        if self.doses_left > 1 {
            host.packet_message(&format!(
                "You have {} doses of potion left.",
                self.doses_left
            ))?;
        } else if self.doses_left == 1 {
            host.packet_message("You have 1 dose of potion left.")?;
        } else {
            host.packet_message("You have finished your potion.")?;
        }
        stop_executing(host)
    }
}

/// ContentAPI `getUsedOption(player)`.
pub fn used_option<H: Host>(host: &mut H) -> Result<String> {
    match host.attribute("interact:option", &Attr::Str("INVALID".into()))? {
        Attr::Str(text) => Ok(text),
        Attr::Null => Err(Error::Null),
        _ => Err(Error::Type),
    }
}

/// ContentAPI `delayAttack(entity, ticks)` (the combat pulse is delayed by 3
/// whatever `ticks` is).
pub fn delay_attack<H: Host>(host: &mut H, ticks: i32) -> Result<()> {
    host.delay_next_attack(3)?;
    let now = host.ticks()?;
    host.clocks()[NEXT_ATTACK] = now.wrapping_add(ticks);
    Ok(())
}

/// The options ConsumableListener registers for `IntType.ITEM`.
pub const OPTIONS: [&str; 2] = ["eat", "drink"];

/// ConsumableListener.defineListeners: one `on(IntType.ITEM, "eat", "drink",
/// handler = ::handleConsumable)`; the registry binds [`handle`] to it.
pub fn define_listeners<E>(
    on: impl FnOnce(&'static str, &'static [&'static str]) -> std::result::Result<(), E>,
) -> std::result::Result<(), E> {
    on("ITEM", &OPTIONS)
}

/// ConsumableListener's `handleConsumable(player, node)`: always true.
pub fn handle<H: Host>(table: &Table, node: &Node, host: &mut H) -> Result<bool> {
    let Some(row) = table.lookup(node.id()) else {
        return Ok(true);
    };
    let food = used_option(host)? == "eat";
    let ignore = table.rows[row].ignore_main_clock;
    let consumable = &table.rows[row].consumable;
    if food {
        if ignore && {
            let clock = host.clocks()[NEXT_CONSUME];
            clock < host.ticks()?
        } {
            let Node::Item(item) = node else {
                return Ok(true);
            };
            consumable.consume(table, item, host)?;
            let now = host.ticks()?;
            host.clocks()[NEXT_CONSUME] = now.wrapping_add(2);
            let now = host.ticks()?;
            host.clocks()[NEXT_EAT] = now.wrapping_add(2);
            delay_attack(host, 3)?;
        } else if {
            let clock = host.clocks()[NEXT_CONSUME];
            clock < host.ticks()?
        } && {
            let clock = host.clocks()[NEXT_EAT];
            clock < host.ticks()?
        } {
            let Node::Item(item) = node else {
                return Ok(true);
            };
            consumable.consume(table, item, host)?;
            let now = host.ticks()?;
            host.clocks()[NEXT_EAT] = now.wrapping_add(2);
            delay_attack(host, 3)?;
        }
    } else if ignore && {
        let clock = host.clocks()[NEXT_CONSUME];
        clock < host.ticks()?
    } {
        let Node::Item(item) = node else {
            return Ok(true);
        };
        consumable.consume(table, item, host)?;
        let now = host.ticks()?;
        host.clocks()[NEXT_CONSUME] = now.wrapping_add(1);
        let now = host.ticks()?;
        host.clocks()[NEXT_DRINK] = now.wrapping_add(1);
    } else if {
        let clock = host.clocks()[NEXT_CONSUME];
        clock < host.ticks()?
    } && {
        let clock = host.clocks()[NEXT_DRINK];
        clock < host.ticks()?
    } {
        let Node::Item(item) = node else {
            return Ok(true);
        };
        consumable.consume(table, item, host)?;
        let now = host.ticks()?;
        host.clocks()[NEXT_DRINK] = now.wrapping_add(1);
    }
    Ok(true)
}
