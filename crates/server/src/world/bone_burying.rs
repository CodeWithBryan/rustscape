//! `content.global.skill.prayer.BoneBuryListener` and the `Bones` table it
//! reads: the bones' "bury" option.
//!
//! Ported from the pinned source without a JVM corpus: the unit tests below
//! check the order of the calls against the Kotlin by reading, not against a
//! recorded run. Every outside call is a [`Host`] method, so the body runs
//! the same over the recording host of the tests and over the live world
//! (live/items/bones.rs).
//!
//! Covered: the listener's lambda ([`bury`]) and its queued script
//! ([`buried`]) with `removeBones`. Not covered: `Bones.forBoneMeal` and the
//! bonemeal ids (the Ectofuntus is not ported), and PrayerAltarListener.

/// `Skills.PRAYER`.
pub const PRAYER: i32 = 5;
/// `Clocks.SKILLING`.
pub const SKILLING: i32 = 7;
/// `Animations.HUMAN_BURYING_BONES_827`.
pub const ANIMATION: i32 = 827;
/// `Sounds.BONES_DOWN_2738`.
pub const SOUND: i32 = 2738;
/// `BarbarianTraining.attributePyrePrayerBonus`.
pub const ATTRIBUTE_PYRE_PRAYER_BONUS: &str = "/save:barbtraining:pyre-prayer-bonus";
/// The listener's class (`scan.contentInterfaces`).
pub const LISTENER_CLASS: &str = "content.global.skill.prayer.BoneBuryListener";
/// The `on(Bones.array, IntType.ITEM, "bury")` lambda.
pub const LAMBDA: &str = "content.global.skill.prayer.BoneBuryListener$defineListeners$1";
/// The lambda's `queueScript` lambda.
pub const QUEUED_LAMBDA: &str = "content.global.skill.prayer.BoneBuryListener$defineListeners$1$1";

/// `Bones`: the item id and the experience, in declaration order.
pub const BONES: [(i32, f64); 31] = [
    (526, 4.5),
    (2530, 4.5),
    (3187, 4.5),
    (2859, 4.5),
    (528, 4.5),
    (3179, 16.0),
    (3180, 18.0),
    (3181, 18.0),
    (3182, 18.0),
    (3183, 5.0),
    (3185, 5.0),
    (3186, 5.0),
    (530, 5.3),
    (532, 15.0),
    (3125, 15.0),
    (4812, 22.5),
    (3123, 25.0),
    (534, 30.0),
    (6812, 50.0),
    (536, 72.0),
    (4830, 84.0),
    (4832, 96.0),
    (6729, 125.0),
    (4834, 140.0),
    (3127, 16.0),
    (3128, 17.0),
    (3129, 17.0),
    (3130, 18.0),
    (3131, 17.0),
    (3132, 17.0),
    (3133, 18.0),
];

/// `Bones.forId(itemId)`: the experience of the bones.
pub fn experience(id: i32) -> Option<f64> {
    BONES.iter().find(|(bone, _)| *bone == id).map(|b| b.1)
}

/// `Bones.array`. The original walks a HashMap's keys; the registrations it
/// feeds are per id, so the order does not show.
pub fn ids() -> Vec<i32> {
    BONES.iter().map(|b| b.0).collect()
}

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// A NullPointerException.
    Null,
    /// An ArrayIndexOutOfBoundsException (`replaceSlot` of a slot outside
    /// the inventory).
    IndexOutOfBounds,
    /// A ClassCastException.
    Cast,
    /// A failure of the binding, not of the original.
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;

/// What the listener asks of the world, for its one player.
pub trait Host {
    /// An inventory `Item` by identity.
    type Item;

    /// `clockReady(player, Clocks.SKILLING)`.
    fn clock_ready(&mut self) -> Result<bool>;
    /// `inInventory(player, id)`.
    fn in_inventory(&mut self, id: i32) -> Result<bool>;
    /// `stopWalk(player)`.
    fn stop_walk(&mut self) -> Result<()>;
    /// `lock(player, ticks)`.
    fn lock(&mut self, ticks: i32) -> Result<()>;
    /// `delayClock(player, Clocks.SKILLING, ticks)`.
    fn delay_clock(&mut self, ticks: i32) -> Result<()>;
    /// `sendMessage(player, message)`.
    fn message(&mut self, message: &str) -> Result<()>;
    /// `animate(player, id)`.
    fn animate(&mut self, id: i32) -> Result<()>;
    /// `playAudio(player, id)`.
    fn play_audio(&mut self, id: i32) -> Result<()>;
    /// `queueScript(player, 1, QueueStrength.STRONG) { ... }` with the
    /// bones and the item its lambda captured; the script is [`buried`].
    fn queue_strong(&mut self, delay: i32, id: i32, item: Self::Item) -> Result<()>;
    /// `item.slot`, read when the queued script runs.
    fn slot(&mut self, item: &Self::Item) -> Result<i32>;
    /// `replaceSlot(player, slot, Item())`: the slot emptied, and the item
    /// it held.
    fn empty_slot(&mut self, slot: i32) -> Result<Option<Self::Item>>;
    /// `removedBones == item`: `Item` does not override `equals`, so this is
    /// identity.
    fn same(&mut self, a: &Self::Item, b: &Self::Item) -> bool;
    /// `getAttribute(player, key, default)`.
    fn attribute_int(&mut self, key: &str, default: i32) -> Result<i32>;
    /// `setAttribute(player, key, value)`.
    fn set_attribute_int(&mut self, key: &str, value: i32) -> Result<()>;
    /// `rewardXP(player, Skills.PRAYER, xp)`.
    fn reward_xp(&mut self, xp: f64) -> Result<()>;
    /// `player.dispatch(BoneBuryEvent(id))`.
    fn dispatch_bone_bury(&mut self, id: i32) -> Result<()>;
    /// `stopExecuting(player)`.
    fn stop_executing(&mut self) -> Result<bool>;
}

/// The `on(Bones.array, IntType.ITEM, "bury")` lambda for the item `node`
/// with the id `id`.
pub fn bury<H: Host>(host: &mut H, id: i32, node: H::Item) -> Result<bool> {
    if experience(id).is_none() {
        return Ok(true);
    }
    if !host.clock_ready()? {
        return Ok(true);
    }
    if !host.in_inventory(id)? {
        return Ok(true);
    }
    host.stop_walk()?;
    host.lock(2)?;
    host.delay_clock(2)?;
    host.message("You dig a hole in the ground.")?;
    host.animate(ANIMATION)?;
    host.play_audio(SOUND)?;
    host.queue_strong(1, id, node)?;
    Ok(true)
}

/// `removeBones(player, item)`. The slot is the item's own field, so an item
/// that left its slot since the click has whatever now lies there removed,
/// as in the original.
fn remove_bones<H: Host>(host: &mut H, item: &H::Item) -> Result<bool> {
    let slot = host.slot(item)?;
    let Some(removed) = host.empty_slot(slot)? else {
        // `null == item`.
        return Ok(false);
    };
    if !host.same(&removed, item) {
        return Ok(false);
    }
    Ok(host.slot(&removed)? == host.slot(item)?)
}

/// The queued script of [`bury`], with what it captured.
pub fn buried<H: Host>(host: &mut H, id: i32, item: &H::Item) -> Result<bool> {
    // `bones` is the lambda's capture: the listener returned before queueing
    // when there was none.
    let experience = experience(id).ok_or(Error::Null)?;
    if remove_bones(host, item)? {
        host.message("You bury the bones.")?;
        let charges = host.attribute_int(ATTRIBUTE_PYRE_PRAYER_BONUS, 0)?;
        let xp = if charges > 0 {
            host.set_attribute_int(ATTRIBUTE_PYRE_PRAYER_BONUS, charges - 1)?;
            experience * 4.0
        } else {
            experience
        };
        host.reward_xp(xp)?;
        host.dispatch_bone_bury(id)?;
    }
    host.stop_executing()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A recording host: an inventory of item tokens.
    #[derive(Default)]
    struct World {
        calls: Vec<String>,
        clock_ready: bool,
        /// The inventory: the token and id in each slot.
        slots: Vec<Option<(u32, i32)>>,
        /// Each token's `slot` field.
        index: std::collections::BTreeMap<u32, i32>,
        charges: i32,
        queued: Vec<(i32, i32, u32)>,
    }
    impl World {
        fn holding(items: &[(u32, i32)]) -> Self {
            let mut world = Self {
                clock_ready: true,
                ..Self::default()
            };
            for (slot, (token, id)) in items.iter().enumerate() {
                world.slots.push(Some((*token, *id)));
                world.index.insert(*token, slot as i32);
            }
            world
        }
    }
    impl Host for World {
        type Item = u32;
        fn clock_ready(&mut self) -> Result<bool> {
            Ok(self.clock_ready)
        }
        fn in_inventory(&mut self, id: i32) -> Result<bool> {
            Ok(self.slots.iter().flatten().any(|(_, held)| *held == id))
        }
        fn stop_walk(&mut self) -> Result<()> {
            self.calls.push("stopWalk".into());
            Ok(())
        }
        fn lock(&mut self, ticks: i32) -> Result<()> {
            self.calls.push(format!("lock {ticks}"));
            Ok(())
        }
        fn delay_clock(&mut self, ticks: i32) -> Result<()> {
            self.calls.push(format!("delayClock {ticks}"));
            Ok(())
        }
        fn message(&mut self, message: &str) -> Result<()> {
            self.calls.push(format!("message {message}"));
            Ok(())
        }
        fn animate(&mut self, id: i32) -> Result<()> {
            self.calls.push(format!("animate {id}"));
            Ok(())
        }
        fn play_audio(&mut self, id: i32) -> Result<()> {
            self.calls.push(format!("audio {id}"));
            Ok(())
        }
        fn queue_strong(&mut self, delay: i32, id: i32, item: u32) -> Result<()> {
            self.calls.push(format!("queue {delay}"));
            self.queued.push((delay, id, item));
            Ok(())
        }
        fn slot(&mut self, item: &u32) -> Result<i32> {
            self.index.get(item).copied().ok_or(Error::Null)
        }
        fn empty_slot(&mut self, slot: i32) -> Result<Option<u32>> {
            let at = usize::try_from(slot).or(Err(Error::IndexOutOfBounds))?;
            let held = self.slots.get_mut(at).ok_or(Error::IndexOutOfBounds)?;
            Ok(held.take().map(|(token, _)| token))
        }
        fn same(&mut self, a: &u32, b: &u32) -> bool {
            a == b
        }
        fn attribute_int(&mut self, _: &str, default: i32) -> Result<i32> {
            Ok(if self.charges == 0 {
                default
            } else {
                self.charges
            })
        }
        fn set_attribute_int(&mut self, _: &str, value: i32) -> Result<()> {
            self.charges = value;
            Ok(())
        }
        fn reward_xp(&mut self, xp: f64) -> Result<()> {
            self.calls.push(format!("xp {xp}"));
            Ok(())
        }
        fn dispatch_bone_bury(&mut self, id: i32) -> Result<()> {
            self.calls.push(format!("event {id}"));
            Ok(())
        }
        fn stop_executing(&mut self) -> Result<bool> {
            self.calls.push("stopExecuting".into());
            Ok(true)
        }
    }

    #[test]
    fn the_table_has_every_constant() {
        assert_eq!(BONES.len(), 31);
        assert_eq!(experience(526), Some(4.5));
        assert_eq!(experience(536), Some(72.0));
        assert_eq!(experience(4834), Some(140.0));
        assert_eq!(experience(592), None);
        let mut ids = ids();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 31);
    }

    #[test]
    fn bury_digs_and_queues() {
        let mut world = World::holding(&[(1, 526)]);
        assert_eq!(bury(&mut world, 526, 1), Ok(true));
        assert_eq!(
            world.calls,
            [
                "stopWalk",
                "lock 2",
                "delayClock 2",
                "message You dig a hole in the ground.",
                "animate 827",
                "audio 2738",
                "queue 1",
            ]
        );
        assert_eq!(world.queued, [(1, 526, 1)]);
    }

    #[test]
    fn bury_waits_for_the_clock_and_the_inventory() {
        let mut world = World::holding(&[(1, 526)]);
        world.clock_ready = false;
        assert_eq!(bury(&mut world, 526, 1), Ok(true));
        assert!(world.calls.is_empty());
        let mut world = World::holding(&[(1, 532)]);
        assert_eq!(bury(&mut world, 526, 1), Ok(true));
        assert!(world.calls.is_empty());
        // An id outside the table.
        let mut world = World::holding(&[(1, 592)]);
        assert_eq!(bury(&mut world, 592, 1), Ok(true));
        assert!(world.calls.is_empty());
    }

    #[test]
    fn buried_removes_the_item_and_rewards() {
        let mut world = World::holding(&[(1, 526), (2, 532)]);
        assert_eq!(buried(&mut world, 532, &2), Ok(true));
        assert_eq!(
            world.calls,
            [
                "message You bury the bones.",
                "xp 15",
                "event 532",
                "stopExecuting"
            ]
        );
        assert_eq!(world.slots, [Some((1, 526)), None]);
    }

    #[test]
    fn a_pyre_charge_quadruples_the_experience() {
        let mut world = World::holding(&[(1, 536)]);
        world.charges = 2;
        assert_eq!(buried(&mut world, 536, &1), Ok(true));
        assert!(world.calls.contains(&"xp 288".to_string()));
        assert_eq!(world.charges, 1);
    }

    #[test]
    fn an_item_that_left_its_slot_is_not_buried() {
        // The bones were dropped and another item took the slot: the slot
        // is emptied all the same, and nothing is rewarded.
        let mut world = World::holding(&[(1, 526)]);
        world.slots[0] = Some((9, 1511));
        assert_eq!(buried(&mut world, 526, &1), Ok(true));
        assert_eq!(world.calls, ["stopExecuting"]);
        assert_eq!(world.slots, [None]);
        // An empty slot.
        let mut world = World::holding(&[(1, 526)]);
        world.slots[0] = None;
        assert_eq!(buried(&mut world, 526, &1), Ok(true));
        assert_eq!(world.calls, ["stopExecuting"]);
    }
}
