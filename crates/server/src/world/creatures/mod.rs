//! The remaining global creature classes of content/global/handlers/npc.
//!
//! Ports `SheepBehavior` ([`sheep`]), `ChinchompaDeathListener`
//! ([`chinchompa_death`]), `ShooAwayStrayDogListener` ([`stray_dog`]),
//! `CowMilkingPlugin` with its `BucketHandler` ([`cow_milking`]) and
//! `DecantListener` with its `DecantingDialogue` ([`decant`]), and the world
//! enum of AbstractNPC classes the start-up registers ([`classes`]: the
//! `npc_classes::Class` slice and the dragon classes).
//!
//! A behaviour's hooks are functions a world's `npc_behaviour::Behaviour`
//! for the class's ids calls; listener and plugin registrations are
//! recorded services, and their handlers are functions here. The pulses the
//! classes submit are [`CreaturePulse`]s a world runs. Everything outside the
//! classes is a required [`Host`] service with no default. See
//! docs/equipment-registers.md.
pub mod chinchompa_death;
pub mod classes;
pub mod cow_milking;
pub mod decant;
pub mod sheep;
pub mod stray_dog;

use crate::world::combat_pulse::handler as base;
use crate::world::combat_swing::{Error, Graphics, Result};
use crate::world::dragonfire::ItemValue;
use crate::world::equipment_registers as er;

/// `Direction`, with the original's values (`toInteger`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    NorthWest,
    North,
    NorthEast,
    West,
    East,
    SouthWest,
    South,
    SouthEast,
}

impl Direction {
    pub fn value(self) -> i32 {
        match self {
            Self::North => 0,
            Self::East => 1,
            Self::South => 2,
            Self::West => 3,
            Self::NorthEast => 4,
            Self::SouthEast => 5,
            Self::SouthWest => 6,
            Self::NorthWest => 7,
        }
    }

    pub fn step(self) -> (i32, i32) {
        match self {
            Self::NorthWest => (-1, 1),
            Self::North => (0, 1),
            Self::NorthEast => (1, 1),
            Self::West => (-1, 0),
            Self::East => (1, 0),
            Self::SouthWest => (-1, -1),
            Self::South => (0, -1),
            Self::SouthEast => (1, -1),
        }
    }

    /// `Direction.get(rotation)`: the first constant with that value.
    pub fn get(rotation: i32) -> Option<Self> {
        [
            Self::NorthWest,
            Self::North,
            Self::NorthEast,
            Self::West,
            Self::East,
            Self::SouthWest,
            Self::South,
            Self::SouthEast,
        ]
        .into_iter()
        .find(|d| d.value() == rotation)
    }

    /// `getOpposite()`: `get(toInteger() + 2 & 3)`, so a diagonal's
    /// "opposite" is a cardinal direction.
    pub fn opposite(self) -> Self {
        Self::get((self.value() + 2) & 3).unwrap_or(Self::North)
    }

    /// `Direction.getDirection(location, l)`: from the first location to
    /// the second; no difference is north.
    pub fn between(from: [i32; 3], to: [i32; 3]) -> Self {
        let dx = to[0].wrapping_sub(from[0]);
        let dy = to[1].wrapping_sub(from[1]);
        if dx < 0 {
            if dy < 0 {
                Self::SouthWest
            } else if dy > 0 {
                Self::NorthWest
            } else {
                Self::West
            }
        } else if dx > 0 {
            if dy < 0 {
                Self::SouthEast
            } else if dy > 0 {
                Self::NorthEast
            } else {
                Self::East
            }
        } else if dy < 0 {
            Self::South
        } else {
            Self::North
        }
    }
}

/// The new `Location(x + stepX * steps, y + stepY * steps, z)` an entity
/// walks to away from another location (the sheep's 3 steps, the dog's 12).
pub fn away_from(from: [i32; 3], other: [i32; 3], steps: i32) -> [i32; 3] {
    let (sx, sy) = Direction::between(from, other).opposite().step();
    base::location(
        from[0].wrapping_add(sx.wrapping_mul(steps)),
        from[1].wrapping_add(sy.wrapping_mul(steps)),
        from[2],
    )
}

/// `Location.getSurroundingTiles()`: the eight neighbours in its order.
pub fn surrounding_tiles(l: [i32; 3]) -> [[i32; 3]; 8] {
    [
        (-1, -1),
        (0, -1),
        (1, -1),
        (1, 0),
        (1, 1),
        (0, 1),
        (-1, 1),
        (-1, 0),
    ]
    .map(|(dx, dy)| base::location(l[0].wrapping_add(dx), l[1].wrapping_add(dy), l[2]))
}

/// An argument of `dialogueInterpreter.open(id, args...)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogueArg {
    Str(&'static str),
    Bool(bool),
}

/// `Consumables.getConsumableById(id)?.consumable as? Potion`: the potion
/// object (its identity, the row of the table) and its ids.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Potion {
    pub identity: usize,
    pub ids: Vec<i32>,
}

/// What an anonymous pulse of these classes does when it fires.
pub enum CreatureTask<H: Host> {
    /// SheepBehavior's 80-tick `sheep.reTransform()`.
    Retransform { npc: H::Entity },
    /// CowMilkingPlugin's 8-tick milking pulse (its `stop` resets the
    /// player's animation).
    Milk { player: H::Entity },
}

/// One `Pulse` subclass instance of these classes.
pub struct CreaturePulse<H: Host> {
    pub running: bool,
    pub delay: i32,
    pub ticks_passed: i32,
    pub checks: Vec<Option<H::Entity>>,
    pub task: CreatureTask<H>,
}

impl<H: Host> CreaturePulse<H> {
    /// `Pulse(delay, checks...)`.
    pub fn new(delay: i32, checks: Vec<Option<H::Entity>>, task: CreatureTask<H>) -> Self {
        Self {
            running: true,
            delay,
            ticks_passed: 0,
            checks,
            task,
        }
    }

    /// `start()`.
    pub fn start(&mut self) {
        self.running = true;
    }

    /// `stop()`, with the milking pulse's override.
    pub fn stop(&mut self, h: &mut H) -> Result<()> {
        self.running = false;
        if let CreatureTask::Milk { player } = &self.task {
            let player = player.clone();
            let animation = h.new_animation(-1, crate::world::special_attacks::Priority::Mid);
            h.animate(&player, Some(&animation))?;
        }
        Ok(())
    }

    /// `Pulse.update()`: true when the runner must drop the pulse. A failure
    /// inside the body is printed and stops the pulse; a failure of `stop`
    /// itself leaves `update`.
    pub fn update(&mut self, h: &mut H) -> Result<bool> {
        for at in 0..self.checks.len() {
            if let Some(node) = self.checks[at].clone() {
                if !h.is_active(&node)? {
                    self.stop(h)?;
                    return Ok(true);
                }
            }
        }
        if !self.running {
            return Ok(true);
        }
        self.ticks_passed = self.ticks_passed.wrapping_add(1);
        if self.ticks_passed >= self.delay {
            self.ticks_passed = 0;
            match self.body(h) {
                Ok(true) => {
                    self.stop(h)?;
                    return Ok(true);
                }
                Ok(false) => {}
                Err(error) => {
                    h.print_stack_trace(&error)?;
                    self.stop(h)?;
                    return Ok(true);
                }
            }
            return Ok(!self.running);
        }
        Ok(false)
    }

    fn body(&mut self, h: &mut H) -> Result<bool> {
        match &self.task {
            CreatureTask::Retransform { npc } => {
                let npc = npc.clone();
                h.npc_re_transform(&npc)?;
                Ok(true)
            }
            CreatureTask::Milk { player } => {
                let player = player.clone();
                cow_milking::milk_pulse(h, &player)
            }
        }
    }
}

/// Everything these classes reach beyond the combat slices', the special
/// attacks', dragonfire's and the equipment registers' services. The first
/// group are plain accessors to static content tables; each fallible
/// method is one original call, and none has a default.
pub trait Host: er::Host {
    /// `Consumables.getConsumableById(id)?.consumable as? Potion`.
    fn potion(&self, id: i32) -> Option<Potion>;
    /// `Consumables.potions`.
    fn potion_ids(&self) -> Vec<i32>;
    /// `CalquatDecant.allKegIds`.
    fn keg_ids(&self) -> Vec<i32>;

    /// ContentAPI `sendChat(entity, message)`.
    fn api_chat(&mut self, entity: &Self::Entity, text: &str) -> Result<()>;
    /// ContentAPI `animate(entity, id)`.
    fn api_animate(&mut self, entity: &Self::Entity, id: i32) -> Result<()>;
    /// ContentAPI `lock(entity, duration)`.
    fn lock(&mut self, entity: &Self::Entity, ticks: i32) -> Result<()>;
    /// ContentAPI `unlock(entity)`.
    fn unlock(&mut self, entity: &Self::Entity) -> Result<()>;
    /// ContentAPI `forceWalk(entity, dest, type)`.
    fn force_walk(&mut self, entity: &Self::Entity, dest: [i32; 3], kind: &str) -> Result<()>;
    /// `player.questRepository.getQuest(quest).isStarted(player)`.
    fn quest_started(&mut self, player: &Self::Entity, quest: &str) -> Result<bool>;
    /// `inventory.contains(id, amount)` (ContentAPI `inInventory`).
    fn inventory_contains(&mut self, player: &Self::Entity, id: i32, amount: i32) -> Result<bool>;
    /// `inventory.remove(item)`.
    fn inventory_remove(&mut self, player: &Self::Entity, item: ItemValue) -> Result<bool>;
    /// `inventory.add(item)`.
    fn inventory_add(&mut self, player: &Self::Entity, item: ItemValue) -> Result<bool>;
    /// ContentAPI `freeSlots(player)`.
    fn free_slots(&mut self, player: &Self::Entity) -> Result<i32>;
    /// ContentAPI `hasOption(npc, option)`.
    fn has_option(&mut self, npc: &Self::Entity, option: &str) -> Result<bool>;
    /// `npc.transform(id)`.
    fn npc_transform(&mut self, npc: &Self::Entity, id: i32) -> Result<()>;
    /// `npc.reTransform()`.
    fn npc_re_transform(&mut self, npc: &Self::Entity) -> Result<()>;
    /// ContentAPI `addItem(player, id, amount)`.
    fn api_add_item(&mut self, player: &Self::Entity, id: i32, amount: i32) -> Result<bool>;
    /// ContentAPI `removeItem(player, item)` of an `Item`.
    fn api_remove_item(&mut self, player: &Self::Entity, item: ItemValue) -> Result<bool>;
    /// ContentAPI `sendGraphics(graphics, location)`.
    fn send_graphics(&mut self, graphics: Graphics, at: [i32; 3]) -> Result<()>;
    /// `GameWorld.Pulser.submit(pulse)`.
    fn submit_creature(&mut self, pulse: CreaturePulse<Self>) -> Result<()>;
    /// `pulseManager.run(pulse)`: the manager starts the pulse and runs it.
    fn run_creature_pulse(
        &mut self,
        player: &Self::Entity,
        pulse: CreaturePulse<Self>,
    ) -> Result<()>;
    /// `dialogueInterpreter.open(id, args...)`.
    fn open_dialogue(&mut self, player: &Self::Entity, id: i32, args: &[DialogueArg])
    -> Result<()>;
    /// `dialogueInterpreter.open(DecantingDialogue(), npc)`.
    fn open_decanting_dialogue(&mut self, player: &Self::Entity, npc: &Self::Entity) -> Result<()>;
    /// `DialogueFile.npc(message)`.
    fn dialogue_npc(&mut self, player: &Self::Entity, text: &str) -> Result<()>;
    /// `DialogueFile.player(message)`.
    fn dialogue_player(&mut self, player: &Self::Entity, text: &str) -> Result<()>;
    /// ContentAPI `decantContainer(player.inventory)`: the items to remove
    /// and to add.
    fn decant_container(
        &mut self,
        player: &Self::Entity,
    ) -> Result<(Vec<ItemValue>, Vec<ItemValue>)>;
    /// `CalquatDecant.decantCalquat(player.inventory)`.
    fn decant_calquat(&mut self, player: &Self::Entity)
    -> Result<(Vec<ItemValue>, Vec<ItemValue>)>;
    /// `CalquatDecant.isKeg(id)`.
    fn is_keg(&mut self, id: i32) -> Result<bool>;
    /// `CalquatDecant.combineKegs(player, used, with)`.
    fn combine_kegs(
        &mut self,
        player: &Self::Entity,
        used: &Self::Item,
        with: &Self::Item,
    ) -> Result<bool>;
    /// `CalquatDecant.pourIntoGlass(player, glass, keg)`.
    fn pour_into_glass(
        &mut self,
        player: &Self::Entity,
        glass: &Self::Item,
        keg: &Self::Item,
    ) -> Result<bool>;
    /// `on(ids, IntType.NPC, options...)` (no ids: `on(IntType.NPC, options...)`).
    fn on_npc(&mut self, ids: Option<&[i32]>, options: &[&str]) -> Result<()>;
    /// `onUseWith(IntType.ITEM, used, with...)`.
    fn on_use_with(&mut self, used: &[i32], with: &[i32]) -> Result<()>;
    /// `SceneryDefinition.forId(id).getHandlers().put(key, plugin)`.
    fn put_scenery_handler(&mut self, id: i32, key: &str) -> Result<()>;
    /// `SceneryDefinition.setOptionHandler(name, plugin)`.
    fn scenery_option_handler(&mut self, name: &str) -> Result<()>;
    /// `ClassScanner.definePlugin(new BucketHandler())`.
    fn define_bucket_plugin(&mut self) -> Result<()>;
    /// `UseWithHandler.addHandler(id, OBJECT_TYPE, handler)` of the bucket
    /// handler (used item 1925).
    fn add_use_with_handler(&mut self, id: i32, kind: i32) -> Result<()>;
}

/// `RandomFunction.random(a, b)`: `min + random(|b - a|)`.
pub(crate) fn random_between<H: Host>(h: &mut H, a: i32, b: i32) -> Result<i32> {
    let n = b.wrapping_sub(a).wrapping_abs();
    let r = if n == 0 { 0 } else { random(h, n)? };
    Ok(a.min(b).wrapping_add(r))
}

/// `RandomFunction.random(maxValue)`.
pub(crate) fn random<H: Host>(h: &mut H, max_value: i32) -> Result<i32> {
    if max_value <= 0 {
        return Ok(0);
    }
    h.next_int(max_value)
}

/// `entity.getLocation()`, required non-null.
pub(crate) fn at<H: Host>(h: &mut H, entity: &H::Entity) -> Result<[i32; 3]> {
    h.location(entity)?.ok_or(Error::Null)
}
