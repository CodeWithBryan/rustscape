//! Six small listeners of `content/global/handlers/item/withobject`:
//! collecting sap (`SapCollectListener`, with its "empty" item option), the
//! Draynor Manor fountain (`PoisonFountainListener`), cheese for Ridgeley
//! (`HairdresserCheeseListener`), the herring jokes (`FishEasterEgg`), the
//! rope on the God Wars entrance hole (`GWDEntranceRopeListener`) and an axe
//! on a tree (`AxeOnTree`, which only starts `WoodcuttingSkillPulse`: the
//! pulse is `world::gathering::woodcutting`'s).
//!
//! Each lambda is a typed native value ([`Interaction`]) whose body is a
//! function over [`Host`]; the pulses the lambdas submit are [`Pulse`]
//! values run by [`pulse`]. Every call the original makes outside a body is
//! a required method with no default. Ported from the pinned Kotlin sources
//! **without a JVM differential corpus**: `tests/item_on_object.rs` checks
//! the bodies against a recording host written from the sources, not
//! against the original classes. See docs/item-on-object.md.
pub use crate::world::global_handlers::Kind;
pub use crate::world::global_items2::{Error, Result};
use crate::world::interaction_listeners::{self as interactions, Casing, FunctionKind};

/// `Items.KNIFE_946`.
pub const KNIFE: i32 = 946;
/// `Items.BUCKET_1925`.
pub const BUCKET: i32 = 1925;
/// `Items.BUCKET_OF_SAP_4687`.
pub const BUCKET_OF_SAP: i32 = 4687;
/// `Animations.HUMAN_FILL_BUCKET_WITH_SAP_2009`.
pub const SAP_ANIMATION: i32 = 2009;
/// SapCollectListener's `TREES`, in declaration order.
pub const SAP_TREES: [i32; 27] = [
    1276, 1277, 1278, 1280, 1315, 1316, 1318, 1319, 1330, 1331, 1332, 3033, 3034, 3035, 3036, 3879,
    3881, 3882, 3883, 10041, 14308, 14309, 30132, 30133, 37477, 37478, 37652,
];

/// `Items.POISONED_FISH_FOOD_274`.
pub const POISONED_FISH_FOOD: i32 = 274;
/// `Scenery.FOUNTAIN_153`.
pub const FOUNTAIN: i32 = 153;
/// PoisonFountainListener's `Animation(881)`.
pub const POUR_ANIMATION: i32 = 881;
/// The attribute the fountain reads.
pub const PIRANHAS_KILLED: &str = "piranhas-killed";
/// The key the fountain writes (the same attribute, saved).
pub const PIRANHAS_KILLED_SAVED: &str = "/save:piranhas-killed";

/// `Items.CHEESE_1985`.
pub const CHEESE: i32 = 1985;
/// `Scenery.TREADMILL_11677`.
pub const TREADMILL: i32 = 11677;

/// FishEasterEgg's `fish`: `RAW_HERRING_345`, `HERRING_347`.
pub const HERRINGS: [i32; 2] = [345, 347];
/// FishEasterEgg's `doors`.
pub const DOORS: [i32; 2] = [1967, 1968];

/// `Items.ROPE_954`.
pub const ROPE: i32 = 954;
/// `Scenery.HOLE_26340`.
pub const GWD_HOLE: i32 = 26340;
/// The varbit the rope sets to 1 (saved).
pub const GWD_ROPE_VARBIT: i32 = 3932;

/// AxeOnTree's `axes`, in declaration order: bronze, mithril, iron, black,
/// steel, adamant, rune, dragon, the inferno adze.
pub const AXES: [i32; 9] = [1351, 1355, 1349, 1361, 1353, 1357, 1359, 6739, 13661];

/// FishEasterEgg's `TREE_IDs` and AxeOnTree's `trees`:
/// `WoodcuttingNode.values().map { it.id }`.
pub fn woodcutting_ids() -> Vec<i32> {
    crate::world::gathering::nodes::WoodcuttingNode::values()
        .iter()
        .map(|node| node.full)
        .collect()
}

/// A pulse a lambda hands to `player.pulseManager.run`.
#[derive(Clone, Debug, PartialEq)]
pub enum Pulse<N> {
    /// SapCollectListener's `Pulse(2)`.
    Sap { player: N },
    /// PoisonFountainListener's `Pulse(1)` with its `counter`.
    Piranhas { player: N, counter: i32 },
    /// HairdresserCheeseListener's `runTask(player, 3, 1)`: ContentAPI's
    /// `Pulse(delay)` around the task, with its `cycles`.
    Cheese { player: N, used: N, cycles: i32 },
}
impl<N> Pulse<N> {
    /// The pulse's class.
    pub fn class(&self) -> &'static str {
        match self {
            Self::Sap { .. } => {
                "content.global.handlers.item.withobject.SapCollectListener$defineListeners$1$1"
            }
            Self::Piranhas { .. } => {
                "content.global.handlers.item.withobject.PoisonFountainListener$defineListeners$1$1"
            }
            Self::Cheese { .. } => "core.api.ContentAPIKt$runTask$1",
        }
    }
    /// The player whose pulse manager runs it.
    pub fn player(&self) -> &N {
        match self {
            Self::Sap { player } | Self::Piranhas { player, .. } | Self::Cheese { player, .. } => {
                player
            }
        }
    }
}

/// The services the lambdas and pulses call, in the original's terms.
pub trait Host {
    /// A node (player, scenery, item); `==` is identity.
    type Node: Clone;
    fn kind(&self, node: &Self::Node) -> Kind;
    /// `item.slot`.
    fn slot(&mut self, item: &Self::Node) -> Result<i32>;
    /// ContentAPI `inInventory(player, id)`: `inventory.contains(id, 1)`.
    fn in_inventory(&mut self, player: &Self::Node, id: i32) -> Result<bool>;
    /// ContentAPI `sendMessage(player, message)`.
    fn send_message(&mut self, player: &Self::Node, text: &str) -> Result<()>;
    /// ContentAPI `removeItem(player, id)`: `inventory.remove(Item(id))`.
    fn remove_item_id(&mut self, player: &Self::Node, id: i32) -> Result<bool>;
    /// ContentAPI `removeItem(player, item)`: `inventory.remove(item)`.
    fn remove_item(&mut self, player: &Self::Node, item: &Self::Node) -> Result<bool>;
    /// ContentAPI `addItem(player, id)`: `inventory.add(Item(id, 1))`.
    fn add_item(&mut self, player: &Self::Node, id: i32) -> Result<bool>;
    /// ContentAPI `replaceSlot(player, slot, Item(id))` on the inventory:
    /// `inventory.replace(item, slot)`.
    fn replace_slot(&mut self, player: &Self::Node, slot: i32, id: i32) -> Result<()>;
    /// ContentAPI `animate(entity, Animation(id))`.
    fn animate(&mut self, entity: &Self::Node, animation: i32) -> Result<()>;
    /// ContentAPI `getAttribute(player, key, default)` as a Boolean.
    fn attribute(&mut self, player: &Self::Node, key: &str, default: bool) -> Result<bool>;
    /// ContentAPI `setAttribute(player, key, value)`.
    fn set_attribute(&mut self, player: &Self::Node, key: &str, value: bool) -> Result<()>;
    /// ContentAPI `lock(entity, duration)`: `entity.lock(duration)`.
    fn lock(&mut self, entity: &Self::Node, ticks: i32) -> Result<()>;
    /// ContentAPI `unlock(entity)`.
    fn unlock(&mut self, entity: &Self::Node) -> Result<()>;
    /// `player.achievementDiaryManager.finishTask(player, DiaryType.FALADOR,
    /// level, index)`.
    fn finish_falador_task(&mut self, player: &Self::Node, level: i32, index: i32) -> Result<()>;
    /// ContentAPI `setVarbit(player, id, value, save)`.
    fn set_varbit(&mut self, player: &Self::Node, id: i32, value: i32, save: bool) -> Result<()>;
    /// `entity.pulseManager.run(pulse)` of a new `Pulse(delay)`
    /// (`submitIndividualPulse` and `runTask`).
    fn run_pulse(&mut self, pulse: Pulse<Self::Node>, delay: i32) -> Result<()>;
    /// `submitIndividualPulse(player, WoodcuttingSkillPulse(player,
    /// scenery))`: the pulse is constructed, then run in the player's pulse
    /// manager.
    fn run_woodcutting_pulse(&mut self, player: &Self::Node, scenery: &Self::Node) -> Result<()>;
}

/// The lambdas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interaction {
    /// SapCollectListener's `onUseWith(SCENERY, KNIFE_946, *TREES)`.
    SapCollect,
    /// SapCollectListener's `on(BUCKET_OF_SAP_4687, ITEM, "empty")`.
    SapEmpty,
    /// PoisonFountainListener's `onUseWith(SCENERY, POISONED_FISH_FOOD_274,
    /// FOUNTAIN_153)`.
    PoisonFountain,
    /// HairdresserCheeseListener's `onUseWith(SCENERY, CHEESE_1985,
    /// TREADMILL_11677)`.
    HairdresserCheese,
    /// FishEasterEgg's `onUseWith(SCENERY, fish, *TREE_IDs)`.
    FishOnTree,
    /// FishEasterEgg's `onUseWith(SCENERY, fish, *doors)`.
    FishOnDoor,
    /// GWDEntranceRopeListener's `onUseWith(SCENERY, ROPE_954, HOLE_26340)`.
    GwdRope,
    /// AxeOnTree's `onUseWith(SCENERY, axes, *trees)`.
    AxeOnTree,
}
impl crate::world::component_objects::NativeObject for Interaction {}
impl Interaction {
    /// The lambda's class.
    pub fn class(self) -> &'static str {
        match self {
            Self::SapCollect => {
                "content.global.handlers.item.withobject.SapCollectListener$defineListeners$1"
            }
            Self::SapEmpty => {
                "content.global.handlers.item.withobject.SapCollectListener$defineListeners$2"
            }
            Self::PoisonFountain => {
                "content.global.handlers.item.withobject.PoisonFountainListener$defineListeners$1"
            }
            Self::HairdresserCheese => {
                "content.global.handlers.item.withobject.HairdresserCheeseListener$defineListeners$1"
            }
            Self::FishOnTree => {
                "content.global.handlers.item.withobject.FishEasterEgg$defineListeners$1"
            }
            Self::FishOnDoor => {
                "content.global.handlers.item.withobject.FishEasterEgg$defineListeners$2"
            }
            Self::GwdRope => {
                "content.global.handlers.item.withobject.GWDEntranceRopeListener$defineListeners$1"
            }
            Self::AxeOnTree => {
                "content.global.handlers.item.withobject.AxeOnTree$defineListeners$1"
            }
        }
    }
    /// Whether the lambda is an `on` listener (the others are use-with).
    pub fn is_option(self) -> bool {
        self == Self::SapEmpty
    }
}

/// The six listener classes, by their scanned names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    AxeOnTree,
    FishEasterEgg,
    GwdEntranceRope,
    HairdresserCheese,
    PoisonFountain,
    SapCollect,
}
impl Class {
    pub const ALL: [Self; 6] = [
        Self::AxeOnTree,
        Self::FishEasterEgg,
        Self::GwdEntranceRope,
        Self::HairdresserCheese,
        Self::PoisonFountain,
        Self::SapCollect,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::AxeOnTree => "content.global.handlers.item.withobject.AxeOnTree",
            Self::FishEasterEgg => "content.global.handlers.item.withobject.FishEasterEgg",
            Self::GwdEntranceRope => {
                "content.global.handlers.item.withobject.GWDEntranceRopeListener"
            }
            Self::HairdresserCheese => {
                "content.global.handlers.item.withobject.HairdresserCheeseListener"
            }
            Self::PoisonFountain => {
                "content.global.handlers.item.withobject.PoisonFountainListener"
            }
            Self::SapCollect => "content.global.handlers.item.withobject.SapCollectListener",
        }
    }
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.name() == name)
    }
}

/// A class's `defineListeners` over the native InteractionListeners
/// registry. The vararg overloads validate each pair; the array overload of
/// FishEasterEgg and AxeOnTree overwrites.
pub fn define(registry: &interactions::Registry, casing: &impl Casing, class: Class) -> Result<()> {
    let scenery = interactions::Kind::Scenery.ordinal();
    let binding = |e: interactions::Error| Error::Binding(Some(format!("{e:?}")));
    let function = |f: Interaction| {
        let kind = if f.is_option() {
            FunctionKind::On
        } else {
            FunctionKind::UseWith
        };
        interactions::Function::new(kind, f.class(), f)
    };
    match class {
        Class::AxeOnTree => registry
            .add_use_arrays(
                scenery,
                &AXES,
                &woodcutting_ids(),
                function(Interaction::AxeOnTree),
            )
            .map_err(binding),
        Class::SapCollect => {
            registry
                .add_use_ids(
                    scenery,
                    KNIFE,
                    &SAP_TREES,
                    function(Interaction::SapCollect),
                )
                .map_err(binding)?;
            registry
                .add(
                    casing,
                    BUCKET_OF_SAP,
                    interactions::Kind::Item.ordinal(),
                    &["empty".encode_utf16().collect()],
                    function(Interaction::SapEmpty),
                )
                .map_err(binding)
        }
        Class::PoisonFountain => registry
            .add_use(
                POISONED_FISH_FOOD,
                FOUNTAIN,
                scenery,
                function(Interaction::PoisonFountain),
            )
            .map_err(binding),
        Class::HairdresserCheese => registry
            .add_use(
                CHEESE,
                TREADMILL,
                scenery,
                function(Interaction::HairdresserCheese),
            )
            .map_err(binding),
        Class::FishEasterEgg => {
            registry
                .add_use_arrays(
                    scenery,
                    &HERRINGS,
                    &woodcutting_ids(),
                    function(Interaction::FishOnTree),
                )
                .map_err(binding)?;
            registry
                .add_use_arrays(
                    scenery,
                    &HERRINGS,
                    &DOORS,
                    function(Interaction::FishOnDoor),
                )
                .map_err(binding)
        }
        Class::GwdEntranceRope => registry
            .add_use(ROPE, GWD_HOLE, scenery, function(Interaction::GwdRope))
            .map_err(binding),
    }
}

/// Runs a use-with lambda with the player, the used node and the node it is
/// used on.
pub fn run<H: Host + ?Sized>(
    h: &mut H,
    f: Interaction,
    player: &H::Node,
    used: &H::Node,
    with: &H::Node,
) -> Result<bool> {
    match f {
        Interaction::AxeOnTree => {
            // `with.asScenery()`.
            if h.kind(with) != Kind::Scenery {
                return Err(Error::Cast);
            }
            h.run_woodcutting_pulse(player, with)?;
            Ok(true)
        }
        Interaction::SapCollect => sap_collect(h, player),
        Interaction::PoisonFountain => poison_fountain(h, player, used),
        Interaction::HairdresserCheese => hairdresser_cheese(h, player, used),
        Interaction::FishOnTree => {
            h.send_message(player, "This is not the mightiest tree in the forest.")?;
            Ok(true)
        }
        Interaction::FishOnDoor => {
            h.send_message(player, "It can't be done!")?;
            Ok(true)
        }
        Interaction::GwdRope => gwd_rope(h, player, used),
        // An `on` lambda takes two arguments.
        Interaction::SapEmpty => Err(Error::Cast),
    }
}

/// Runs an `on` lambda with the player and the node.
pub fn run_option<H: Host + ?Sized>(
    h: &mut H,
    f: Interaction,
    player: &H::Node,
    node: &H::Node,
) -> Result<bool> {
    match f {
        Interaction::SapEmpty => sap_empty(h, player, node),
        _ => Err(Error::Cast),
    }
}

/// SapCollectListener's use-with lambda: the message when no bucket is
/// held, and the pulse either way.
pub fn sap_collect<H: Host + ?Sized>(h: &mut H, player: &H::Node) -> Result<bool> {
    if !h.in_inventory(player, BUCKET)? {
        h.send_message(player, "You need a bucket to do that.")?;
    }
    h.run_pulse(
        Pulse::Sap {
            player: player.clone(),
        },
        2,
    )?;
    Ok(true)
}

/// SapCollectListener's "empty" lambda.
pub fn sap_empty<H: Host + ?Sized>(h: &mut H, player: &H::Node, node: &H::Node) -> Result<bool> {
    // `node as Item`.
    if h.kind(node) != Kind::Item {
        return Err(Error::Cast);
    }
    let slot = h.slot(node)?;
    if slot < 0 {
        return Ok(false);
    }
    h.replace_slot(player, slot, BUCKET)?;
    h.send_message(player, "You empty the contents of the bucket on the floor.")?;
    Ok(true)
}

/// PoisonFountainListener's lambda.
pub fn poison_fountain<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    used: &H::Node,
) -> Result<bool> {
    if h.attribute(player, PIRANHAS_KILLED, false)? {
        h.send_message(player, "The piranhas are dead already.")?;
        return Ok(true);
    }
    if !h.remove_item(player, used)? {
        return Ok(false);
    }
    h.lock(player, 5)?;
    h.animate(player, POUR_ANIMATION)?;
    h.send_message(player, "You pour the poisoned fish food into the fountain.")?;
    h.set_attribute(player, PIRANHAS_KILLED_SAVED, true)?;
    h.run_pulse(
        Pulse::Piranhas {
            player: player.clone(),
            counter: 0,
        },
        1,
    )?;
    Ok(true)
}

/// HairdresserCheeseListener's lambda. `isTaskSuccessful` is read before
/// the task has run, so the lambda always answers false (the scenery path
/// of `InteractionListeners.run` ignores the answer).
pub fn hairdresser_cheese<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    used: &H::Node,
) -> Result<bool> {
    h.lock(player, 5)?;
    h.run_pulse(
        Pulse::Cheese {
            player: player.clone(),
            used: used.clone(),
            cycles: 1,
        },
        3,
    )?;
    Ok(false)
}

/// GWDEntranceRopeListener's lambda.
pub fn gwd_rope<H: Host + ?Sized>(h: &mut H, player: &H::Node, used: &H::Node) -> Result<bool> {
    if !h.remove_item(player, used)? {
        return Ok(false);
    }
    h.set_varbit(player, GWD_ROPE_VARBIT, 1, true)?;
    Ok(true)
}

/// A pulse's `pulse()`: whether it finished.
pub fn pulse<H: Host + ?Sized>(h: &mut H, pulse: &mut Pulse<H::Node>) -> Result<bool> {
    match pulse {
        Pulse::Sap { player } => {
            if h.remove_item_id(player, BUCKET)? {
                h.animate(player, SAP_ANIMATION)?;
                if h.add_item(player, BUCKET_OF_SAP)? {
                    h.send_message(
                        player,
                        "You cut the tree and allow its sap to drip down into your bucket.",
                    )?;
                }
                return Ok(true);
            }
            Ok(false)
        }
        Pulse::Piranhas { player, counter } => {
            let at = *counter;
            *counter = counter.wrapping_add(1);
            match at {
                1 => h.send_message(player, "The piranhas start eating the food...")?,
                2 => {
                    h.send_message(player, "... then die and float to the surface.")?;
                    h.unlock(player)?;
                    return Ok(true);
                }
                _ => {}
            }
            Ok(false)
        }
        Pulse::Cheese {
            player,
            used,
            cycles,
        } => {
            if h.remove_item(player, used)? {
                h.send_message(
                    player,
                    "You throw the cheese to Ridgeley, for which he appears grateful.",
                )?;
                h.finish_falador_task(player, 0, 6)?;
                h.unlock(player)?;
            }
            *cycles = cycles.wrapping_sub(1);
            Ok(*cycles <= 0)
        }
    }
}
