//! Six small listeners of `content/global/handlers/item/withobject`:
//! collecting sap (`SapCollectListener`, with its "empty" item option), the
//! Draynor Manor fountain (`PoisonFountainListener`), cheese for Ridgeley
//! (`HairdresserCheeseListener`), the herring jokes (`FishEasterEgg`), the
//! rope on the God Wars entrance hole (`GWDEntranceRopeListener`) and an axe
//! on a tree (`AxeOnTree`, which only starts `WoodcuttingSkillPulse`: the
//! pulse is `world::gathering::woodcutting`'s); and the coal trucks
//! (`CoalTruckListener`: coal used on a truck and the truck's "remove-coal"
//! and "investigate" options) with the two pieces of
//! `SeersVillageAchievementDiary` they reach ([`coal_removed_from_seers_truck`]
//! and the `AttributeSet` hook [`seers_attribute_set`]); the Karamja banana
//! crate (`KaramjaBananaCrateListener`: rum and bananas used on the crate
//! and its "search" and "fill" options) and the rope on the Lumbridge swamp
//! hole (`SwampHoleRopeListener`), both over the player's `GlobalData`.
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

/// `Items.COAL_453`.
pub const COAL: i32 = 453;
/// `Scenery.COAL_TRUCK_2114`.
pub const COAL_TRUCK: i32 = 2114;
/// CoalTruckListener's `ATTRIBUTE_COAL_TRUCK_INVENTORY`.
pub const COAL_TRUCK_INVENTORY: &str = "coal-truck-inventory";
/// The key the listener writes (the same attribute, saved).
pub const COAL_TRUCK_INVENTORY_SAVED: &str = "/save:coal-truck-inventory";
/// A truck's capacity.
pub const COAL_TRUCK_CAPACITY: i32 = 120;

/// `Items.KARAMJAN_RUM_431`.
pub const KARAMJAN_RUM: i32 = 431;
/// `Items.BANANA_1963`.
pub const BANANA: i32 = 1963;
/// `Scenery.CRATE_2072`.
pub const BANANA_CRATE: i32 = 2072;
/// KaramjaBananaCrateListener's `CRATE_CAPACITY`.
pub const BANANA_CRATE_CAPACITY: i32 = 10;
/// KaramjaBananaCrateListener's `Animation(832)`.
pub const CRATE_ANIMATION: i32 = 832;
/// The attribute the crate reads for the rum.
pub const STASHED_RUM: &str = "stashed-rum";
/// The key the crate writes (the same attribute, saved).
pub const STASHED_RUM_SAVED: &str = "/save:stashed-rum";
/// `Scenery.DARK_HOLE_5947`.
pub const DARK_HOLE: i32 = 5947;
/// SeersVillageAchievementDiary's `ATTRIBUTE_COAL_TRUCK_FULL`.
pub const SEERS_COAL_TRUCK_FULL: &str = "diary:seers:coal-truck-full";
/// SeersVillageAchievementDiary's `SEERS_COAL_TRUCK_LOCATIONS`.
pub const SEERS_COAL_TRUCKS: [[i32; 3]; 2] = [[2696, 3504, 0], [2698, 3507, 0]];
/// `DiaryLevel.MEDIUM`'s index in the Seers' Village diary.
pub const SEERS_MEDIUM: i32 = 1;
/// `MediumTasks.TRANSPORT_FULL_LOAD_OF_COAL`.
pub const SEERS_TRANSPORT_COAL: i32 = 2;

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
    /// `node.getId()`.
    fn id(&mut self, node: &Self::Node) -> Result<i32>;
    /// `node.getLocation()`.
    fn location(&mut self, node: &Self::Node) -> Result<[i32; 3]>;
    /// ContentAPI `getAttribute(player, key, default)` as an Int.
    fn attribute_int(&mut self, player: &Self::Node, key: &str, default: i32) -> Result<i32>;
    /// ContentAPI `setAttribute(player, key, value)` of an Int.
    /// `Entity.setAttribute` dispatches an AttributeSetEvent: an owner with
    /// the Seers' Village diary hooked runs [`seers_attribute_set`] for it.
    fn set_attribute_int(&mut self, player: &Self::Node, key: &str, value: i32) -> Result<()>;
    /// ContentAPI `removeAttribute(player, key)`.
    fn remove_attribute(&mut self, player: &Self::Node, key: &str) -> Result<()>;
    /// ContentAPI `sendDialogue(player, message)`.
    fn send_dialogue(&mut self, player: &Self::Node, text: &str) -> Result<()>;
    /// ContentAPI `freeSlots(player)`.
    fn free_slots(&mut self, player: &Self::Node) -> Result<i32>;
    /// ContentAPI `amountInInventory(player, id)`.
    fn amount_in_inventory(&mut self, player: &Self::Node, id: i32) -> Result<i32>;
    /// ContentAPI `addItem(player, id, amount)`: `inventory.add(Item(id,
    /// amount))`.
    fn add_items(&mut self, player: &Self::Node, id: i32, amount: i32) -> Result<bool>;
    /// ContentAPI `removeItem(player, Item(id, amount))`.
    fn remove_items(&mut self, player: &Self::Node, id: i32, amount: i32) -> Result<bool>;
    /// `achievementDiaryManager.hasCompletedTask(SEERS_VILLAGE, level,
    /// index)`.
    fn seers_task_complete(&mut self, player: &Self::Node, level: i32, index: i32) -> Result<bool>;
    /// `achievementDiaryManager.updateTask(player, SEERS_VILLAGE, level,
    /// index, complete)`.
    fn seers_update_task(
        &mut self,
        player: &Self::Node,
        level: i32,
        index: i32,
        complete: bool,
    ) -> Result<()>;
    /// `achievementDiaryManager.finishTask(player, SEERS_VILLAGE, level,
    /// index)`.
    fn finish_seers_task(&mut self, player: &Self::Node, level: i32, index: i32) -> Result<()>;
    /// `player.savedData.globalData.isLuthasTask`.
    fn luthas_task(&mut self, player: &Self::Node) -> Result<bool>;
    /// `player.savedData.globalData.karamjaBananas`.
    fn karamja_bananas(&mut self, player: &Self::Node) -> Result<i32>;
    /// `player.savedData.globalData.setKaramjaBannanas(value)`.
    fn set_karamja_bananas(&mut self, player: &Self::Node, value: i32) -> Result<()>;
    /// `player.savedData.globalData.hasTiedLumbridgeRope()`.
    fn lumbridge_rope(&mut self, player: &Self::Node) -> Result<bool>;
    /// `player.savedData.globalData.setLumbridgeRope(value)`.
    fn set_lumbridge_rope(&mut self, player: &Self::Node, value: bool) -> Result<()>;
    /// ContentAPI `sendItemDialogue(player, item, message)` with an `Item`.
    fn send_item_dialogue(
        &mut self,
        player: &Self::Node,
        item: &Self::Node,
        text: &str,
    ) -> Result<()>;
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
    /// CoalTruckListener's `on(COAL_TRUCK_2114, SCENERY, "remove-coal")`.
    CoalRemove,
    /// CoalTruckListener's `on(COAL_TRUCK_2114, SCENERY, "investigate")`.
    CoalInvestigate,
    /// CoalTruckListener's `onUseWith(SCENERY, COAL_453, COAL_TRUCK_2114)`.
    CoalDeposit,
    /// KaramjaBananaCrateListener's `onUseWith(SCENERY, KARAMJAN_RUM_431,
    /// CRATE_2072)`.
    CrateRum,
    /// KaramjaBananaCrateListener's `onUseWith(SCENERY, BANANA_1963,
    /// CRATE_2072)`.
    CrateBanana,
    /// KaramjaBananaCrateListener's `on(CRATE_2072, SCENERY, "search")`.
    CrateSearch,
    /// KaramjaBananaCrateListener's `on(CRATE_2072, SCENERY, "fill")`.
    CrateFill,
    /// SwampHoleRopeListener's `onUseWith(SCENERY, ROPE_954,
    /// DARK_HOLE_5947)`.
    SwampRope,
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
            Self::CoalRemove => {
                "content.global.handlers.item.withobject.CoalTruckListener$defineListeners$1"
            }
            Self::CoalInvestigate => {
                "content.global.handlers.item.withobject.CoalTruckListener$defineListeners$2"
            }
            Self::CoalDeposit => {
                "content.global.handlers.item.withobject.CoalTruckListener$defineListeners$3"
            }
            Self::CrateRum => {
                "content.global.handlers.item.withobject.KaramjaBananaCrateListener$defineListeners$1"
            }
            Self::CrateBanana => {
                "content.global.handlers.item.withobject.KaramjaBananaCrateListener$defineListeners$2"
            }
            Self::CrateSearch => {
                "content.global.handlers.item.withobject.KaramjaBananaCrateListener$defineListeners$3"
            }
            Self::CrateFill => {
                "content.global.handlers.item.withobject.KaramjaBananaCrateListener$defineListeners$4"
            }
            Self::SwampRope => {
                "content.global.handlers.item.withobject.SwampHoleRopeListener$defineListeners$1"
            }
        }
    }
    /// Whether the lambda is an `on` listener (the others are use-with).
    pub fn is_option(self) -> bool {
        matches!(
            self,
            Self::SapEmpty
                | Self::CoalRemove
                | Self::CoalInvestigate
                | Self::CrateSearch
                | Self::CrateFill
        )
    }
}

/// The nine listener classes, by their scanned names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    AxeOnTree,
    CoalTruck,
    FishEasterEgg,
    GwdEntranceRope,
    HairdresserCheese,
    KaramjaBananaCrate,
    PoisonFountain,
    SapCollect,
    SwampHoleRope,
}
impl Class {
    pub const ALL: [Self; 9] = [
        Self::AxeOnTree,
        Self::CoalTruck,
        Self::FishEasterEgg,
        Self::GwdEntranceRope,
        Self::HairdresserCheese,
        Self::KaramjaBananaCrate,
        Self::PoisonFountain,
        Self::SapCollect,
        Self::SwampHoleRope,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::AxeOnTree => "content.global.handlers.item.withobject.AxeOnTree",
            Self::CoalTruck => "content.global.handlers.item.withobject.CoalTruckListener",
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
            Self::KaramjaBananaCrate => {
                "content.global.handlers.item.withobject.KaramjaBananaCrateListener"
            }
            Self::SwampHoleRope => "content.global.handlers.item.withobject.SwampHoleRopeListener",
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
        Class::CoalTruck => {
            for (option, f) in [
                ("remove-coal", Interaction::CoalRemove),
                ("investigate", Interaction::CoalInvestigate),
            ] {
                registry
                    .add(
                        casing,
                        COAL_TRUCK,
                        scenery,
                        &[option.encode_utf16().collect()],
                        function(f),
                    )
                    .map_err(binding)?;
            }
            registry
                .add_use(
                    COAL,
                    COAL_TRUCK,
                    scenery,
                    function(Interaction::CoalDeposit),
                )
                .map_err(binding)
        }
        Class::KaramjaBananaCrate => {
            // The two onUseWith calls come first, then the two on calls.
            for (item, f) in [
                (KARAMJAN_RUM, Interaction::CrateRum),
                (BANANA, Interaction::CrateBanana),
            ] {
                registry
                    .add_use(item, BANANA_CRATE, scenery, function(f))
                    .map_err(binding)?;
            }
            for (option, f) in [
                ("search", Interaction::CrateSearch),
                ("fill", Interaction::CrateFill),
            ] {
                registry
                    .add(
                        casing,
                        BANANA_CRATE,
                        scenery,
                        &[option.encode_utf16().collect()],
                        function(f),
                    )
                    .map_err(binding)?;
            }
            Ok(())
        }
        Class::SwampHoleRope => registry
            .add_use(ROPE, DARK_HOLE, scenery, function(Interaction::SwampRope))
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
        Interaction::CoalDeposit => coal_deposit(h, player),
        Interaction::CrateRum => crate_rum(h, player, used),
        Interaction::CrateBanana => crate_banana(h, player, used),
        Interaction::SwampRope => swamp_rope(h, player, used),
        // An `on` lambda takes two arguments.
        Interaction::SapEmpty
        | Interaction::CoalRemove
        | Interaction::CoalInvestigate
        | Interaction::CrateSearch
        | Interaction::CrateFill => Err(Error::Cast),
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
        Interaction::CoalRemove => coal_remove(h, player, node),
        Interaction::CoalInvestigate => coal_investigate(h, player),
        Interaction::CrateSearch => crate_search(h, player),
        Interaction::CrateFill => crate_fill(h, player),
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

/// The crates' first check: Luthas has not given the task.
fn no_luthas_task<H: Host + ?Sized>(h: &mut H, player: &H::Node) -> Result<bool> {
    if !h.luthas_task(player)? {
        h.send_message(player, "I don't know what goes in there.")?;
        return Ok(true);
    }
    Ok(false)
}

/// KaramjaBananaCrateListener's rum lambda.
pub fn crate_rum<H: Host + ?Sized>(h: &mut H, player: &H::Node, used: &H::Node) -> Result<bool> {
    if no_luthas_task(h, player)? {
        return Ok(true);
    }
    if h.attribute(player, STASHED_RUM, false)? {
        h.send_message(player, "There's already some rum in here...")?;
        return Ok(true);
    }
    if !h.remove_item(player, used)? {
        return Ok(false);
    }
    h.lock(player, 2)?;
    h.animate(player, CRATE_ANIMATION)?;
    h.send_dialogue(player, "You stash the rum in the crate.")?;
    h.set_attribute(player, STASHED_RUM_SAVED, true)?;
    Ok(true)
}

/// KaramjaBananaCrateListener's banana lambda.
pub fn crate_banana<H: Host + ?Sized>(h: &mut H, player: &H::Node, used: &H::Node) -> Result<bool> {
    if no_luthas_task(h, player)? {
        return Ok(true);
    }
    let in_crate = h.karamja_bananas(player)?;
    if in_crate >= BANANA_CRATE_CAPACITY {
        h.send_message(player, "The crate is already full.")?;
        return Ok(true);
    }
    if !h.remove_item(player, used)? {
        return Ok(false);
    }
    h.lock(player, 2)?;
    h.animate(player, CRATE_ANIMATION)?;
    h.send_dialogue(player, "You pack a banana into the crate.")?;
    h.set_karamja_bananas(player, in_crate.wrapping_add(1))?;
    Ok(true)
}

/// KaramjaBananaCrateListener's "search" lambda.
pub fn crate_search<H: Host + ?Sized>(h: &mut H, player: &H::Node) -> Result<bool> {
    if no_luthas_task(h, player)? {
        return Ok(true);
    }
    let in_crate = h.karamja_bananas(player)?;
    if in_crate == 0 {
        h.send_message(player, "The crate is completely empty.")?;
        return Ok(true);
    }
    if in_crate >= BANANA_CRATE_CAPACITY {
        h.send_message(player, "The crate is full of bananas.")?;
        return Ok(true);
    }
    let plural = if in_crate > 1 { "s" } else { "" };
    h.send_message(
        player,
        &format!("The crate has {in_crate} banana{plural} inside."),
    )?;
    Ok(true)
}

/// KaramjaBananaCrateListener's "fill" lambda. The count held and the
/// amount to add are read before the checks; `globalData.karamjaBananas`
/// is read three times.
pub fn crate_fill<H: Host + ?Sized>(h: &mut H, player: &H::Node) -> Result<bool> {
    if no_luthas_task(h, player)? {
        return Ok(true);
    }
    let held = h.amount_in_inventory(player, BANANA)?;
    let in_crate = h.karamja_bananas(player)?;
    let to_add = held.min(BANANA_CRATE_CAPACITY.wrapping_sub(h.karamja_bananas(player)?));
    if held == 0 {
        return Ok(true);
    }
    if in_crate >= BANANA_CRATE_CAPACITY {
        h.send_message(player, "The crate is already full.")?;
        return Ok(true);
    }
    if !h.remove_items(player, BANANA, to_add)? {
        return Ok(false);
    }
    h.lock(player, 2)?;
    h.animate(player, CRATE_ANIMATION)?;
    h.send_message(player, "You pack all your bananas into the crate.")?;
    let now = h.karamja_bananas(player)?;
    h.set_karamja_bananas(player, now.wrapping_add(to_add))?;
    Ok(true)
}

/// SwampHoleRopeListener's lambda.
pub fn swamp_rope<H: Host + ?Sized>(h: &mut H, player: &H::Node, used: &H::Node) -> Result<bool> {
    if h.lumbridge_rope(player)? {
        h.send_dialogue(player, "There is already a rope tied to the entrance.")?;
        return Ok(true);
    }
    if !h.remove_item(player, used)? {
        return Ok(false);
    }
    h.send_item_dialogue(
        player,
        used,
        "You tie the rope to the top of the entrance and throw it down.",
    )?;
    h.set_lumbridge_rope(player, true)?;
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

/// CoalTruckListener's "remove-coal" lambda: as much coal as fits, the
/// attribute, and the Seers' Village diary when any was taken.
pub fn coal_remove<H: Host + ?Sized>(h: &mut H, player: &H::Node, node: &H::Node) -> Result<bool> {
    let mut in_truck = h.attribute_int(player, COAL_TRUCK_INVENTORY, 0)?;
    if in_truck == 0 {
        h.send_dialogue(player, "The coal truck is empty.")?;
        return Ok(true);
    }
    let full = in_truck == COAL_TRUCK_CAPACITY;
    let mut to_remove = h.free_slots(player)?;
    if to_remove > in_truck {
        to_remove = in_truck;
    }
    if h.add_items(player, COAL, to_remove)? {
        in_truck = in_truck.wrapping_sub(to_remove);
        h.set_attribute_int(player, COAL_TRUCK_INVENTORY_SAVED, in_truck)?;
        if to_remove > 0 {
            coal_removed_from_seers_truck(h, player, node, full)?;
        }
    }
    Ok(true)
}

/// CoalTruckListener's "investigate" lambda.
pub fn coal_investigate<H: Host + ?Sized>(h: &mut H, player: &H::Node) -> Result<bool> {
    let in_truck = h.attribute_int(player, COAL_TRUCK_INVENTORY, 0)?;
    let text = format!(
        "There is currently {in_truck} coal in the truck. The truck has space for {} more coal.",
        COAL_TRUCK_CAPACITY.wrapping_sub(in_truck)
    );
    h.send_dialogue(player, &text)?;
    Ok(true)
}

/// CoalTruckListener's use-with lambda: all the coal held, or what fills
/// the truck (with its message).
pub fn coal_deposit<H: Host + ?Sized>(h: &mut H, player: &H::Node) -> Result<bool> {
    let mut in_truck = h.attribute_int(player, COAL_TRUCK_INVENTORY, 0)?;
    let mut held = h.amount_in_inventory(player, COAL)?;
    if held.wrapping_add(in_truck) >= COAL_TRUCK_CAPACITY {
        held = COAL_TRUCK_CAPACITY.wrapping_sub(in_truck);
        h.send_message(player, "You have filled up the coal truck.")?;
    }
    if h.remove_items(player, COAL, held)? {
        in_truck = in_truck.wrapping_add(held);
        h.set_attribute_int(player, COAL_TRUCK_INVENTORY_SAVED, in_truck)?;
    }
    Ok(true)
}

/// `SeersVillageAchievementDiary.onCoalRemovedFromSeersTruck`: coal taken
/// from a truck that was full, at one of the two Seers' Village trucks,
/// finishes the task once its requirement attribute is set
/// (`whenTaskRequirementFulfilled`).
pub fn coal_removed_from_seers_truck<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    node: &H::Node,
    full: bool,
) -> Result<()> {
    if !full {
        return Ok(());
    }
    if h.id(node)? != COAL_TRUCK {
        return Ok(());
    }
    if !SEERS_COAL_TRUCKS.contains(&h.location(node)?) {
        return Ok(());
    }
    if h.attribute(player, SEERS_COAL_TRUCK_FULL, false)? {
        h.finish_seers_task(player, SEERS_MEDIUM, SEERS_TRANSPORT_COAL)?;
        h.remove_attribute(player, SEERS_COAL_TRUCK_FULL)?;
    }
    Ok(())
}

/// `SeersVillageAchievementDiary.onAttributeSet` for an Int value: the
/// truck's attribute reaching 120 fulfils the task's requirement
/// (`fulfillTaskRequirement`). Every other key is ignored.
pub fn seers_attribute_set<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    key: &str,
    value: i32,
) -> Result<()> {
    if key != COAL_TRUCK_INVENTORY_SAVED || value < COAL_TRUCK_CAPACITY {
        return Ok(());
    }
    if h.seers_task_complete(player, SEERS_MEDIUM, SEERS_TRANSPORT_COAL)?
        || h.attribute(player, SEERS_COAL_TRUCK_FULL, false)?
    {
        return Ok(());
    }
    h.seers_update_task(player, SEERS_MEDIUM, SEERS_TRANSPORT_COAL, false)?;
    h.set_attribute(player, &format!("/save:{SEERS_COAL_TRUCK_FULL}"), true)
}
