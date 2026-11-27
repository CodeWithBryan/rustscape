//! Item-on-NPC basics of `content/global/handlers/item/withnpc`: feeding the
//! stray dogs (`FeedOnStrayDog`) and the Falador sheepdog
//! (`FeedFaladorSheepdog`), with the two tables they read (`Bones.array`,
//! `content.data.Meat`), and cider for the people of the Seers' Village pub
//! (`CiderOnForester` with its `CiderOnForesterDialogue`, a DialogueFile:
//! [`CiderDialogue`] over [`DialogueHost`]).
//!
//! Each use-with lambda is a typed native value ([`Interaction`]) whose body
//! is a function over [`Host`]; every call the original makes outside the
//! body is a required method with no default. Ported from the pinned
//! `FeedOnStrayDog.kt`, `FeedFaladorSheepdog.kt`, `Bones.kt` and `Meat.kt`
//! **without a JVM differential corpus**: `tests/item_on_npc.rs` checks the
//! bodies against a recording host written from the sources, not against
//! the original classes. See docs/item-on-npc.md.
use crate::world::global_items2::tables::java_hash_order;
pub use crate::world::global_items2::{Error, Result};
use crate::world::interaction_listeners::{self as interactions, FunctionKind};

/// `Animations.HUMAN_BURYING_BONES_827`.
pub const FEED_ANIMATION: i32 = 827;
/// The stray dog's `Animation(4777)` for meat.
pub const DOG_ANIMATION: i32 = 4777;
/// `NPCs.STRAY_DOG_4766`, `_4767`, `_5917`, `_5918`.
pub const STRAY_DOGS: [i32; 4] = [4766, 4767, 5917, 5918];
/// `NPCs.SHEEPDOG_2311`.
pub const SHEEPDOG: i32 = 2311;

/// `Items.CIDER_5763`.
pub const CIDER: i32 = 5763;
/// CiderOnForester's `ids`.
pub const FORESTERS: [i32; 5] = [1, 2, 3, 4, 5];
/// CiderOnForester's `inBorders(player, 2689, 3488, 2700, 3498)`.
pub const CIDER_PUB: [i32; 4] = [2689, 3488, 2700, 3498];
/// The attribute the cider dialogue counts in (not saved).
pub const SEERS_CIDER_PUB: &str = "seersCiderPub";
/// `DiaryType.SEERS_VILLAGE` level 0, task 6.
pub const SEERS_EASY: i32 = 0;
pub const SEERS_CIDER_TASK: i32 = 6;
/// The dialogue file's class.
pub const CIDER_DIALOGUE_CLASS: &str =
    "content.global.handlers.item.withnpc.CiderOnForesterDialogue";

/// `Items.BURNT_BONES_528`.
pub const BURNT_BONES: i32 = 528;

/// `Bones.values()`' item ids, in declaration order.
pub const BONES: [i32; 31] = [
    526, 2530, 3187, 2859, 528, 3179, 3180, 3181, 3182, 3183, 3185, 3186, 530, 532, 3125, 4812,
    3123, 534, 6812, 536, 4830, 4832, 6729, 4834, 3127, 3128, 3129, 3130, 3131, 3132, 3133,
];

/// `Bones.array`: the keys of the `bones` HashMap (one `put` per constant),
/// in that map's walk.
pub fn bones_array() -> Vec<i32> {
    java_hash_order(&BONES)
}

/// `MeatState`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeatState {
    InedibleRaw,
    InedibleBurnt,
    InedibleSpecial,
    EdibleCooked,
}

/// `Meat.values()`: id and state, in declaration order (two ids appear
/// twice, as in the original).
pub const MEAT: [(i32, MeatState); 40] = {
    use MeatState::{
        EdibleCooked as C, InedibleBurnt as B, InedibleRaw as R, InedibleSpecial as S,
    };
    [
        (522, S),
        (523, S),
        (524, S),
        (525, S),
        (1859, R),
        (1861, C),
        (2132, R),
        (2134, R),
        (2136, R),
        (2138, R),
        (2140, C),
        (2140, B),
        (2142, C),
        (2146, B),
        (3369, C),
        (3371, C),
        (3373, C),
        (4287, R),
        (4289, R),
        (4291, C),
        (4293, C),
        (7518, R),
        (7520, B),
        (7521, C),
        (7523, C),
        (7524, C),
        (7525, C),
        (7526, C),
        (7527, R),
        (9052, C),
        (9978, R),
        (9980, C),
        (9982, B),
        (9982, R),
        (9986, R),
        (9988, C),
        (9990, B),
        (10816, R),
        (12535, R),
        (12546, S),
    ]
};

/// `Meat.values().filter { INEDIBLE_RAW || EDIBLE_COOKED }.map { it.id }`.
pub fn consumable_meats() -> Vec<i32> {
    MEAT.iter()
        .filter(|(_, state)| matches!(state, MeatState::InedibleRaw | MeatState::EdibleCooked))
        .map(|(id, _)| *id)
        .collect()
}

/// A node's class, for the lambdas' casts.
pub use crate::world::global_handlers::Kind;

/// The services the two lambdas call, in the original's terms.
pub trait Host {
    /// A node (player, NPC, item); `==` is identity.
    type Node: Clone;
    fn kind(&self, node: &Self::Node) -> Kind;
    /// `node.getId()`.
    fn id(&mut self, node: &Self::Node) -> Result<i32>;
    /// `node.getName()` of an item.
    fn name(&mut self, node: &Self::Node) -> Result<Option<String>>;
    /// `item.definition.name` / `npc.definition.name`.
    fn definition_name(&mut self, node: &Self::Node) -> Result<Option<String>>;
    /// `String.toLowerCase()` (the default locale).
    fn lowercase_default(&self, text: &str) -> String;
    /// Kotlin's `lowercase()` (`Locale.ROOT`).
    fn lowercase_root(&self, text: &str) -> String;
    /// ContentAPI `removeItem(player, item)` (the inventory).
    fn remove_item(&mut self, player: &Self::Node, item: &Self::Node) -> Result<bool>;
    /// ContentAPI `sendMessage(player, message)`.
    fn send_message(&mut self, player: &Self::Node, text: &str) -> Result<()>;
    /// ContentAPI `sendDialogue(player, message)`.
    fn send_dialogue(&mut self, player: &Self::Node, text: &str) -> Result<()>;
    /// ContentAPI `animate(entity, animation)`: `animator.animate`.
    fn animate(&mut self, entity: &Self::Node, animation: i32) -> Result<()>;
    /// ContentAPI `sendChat(entity, message)`: `entity.sendChat`.
    fn send_chat(&mut self, entity: &Self::Node, text: &str) -> Result<()>;
    /// ContentAPI `inBorders(entity, swX, swY, neX, neY)`.
    fn in_borders(&mut self, entity: &Self::Node, borders: [i32; 4]) -> Result<bool>;
    /// `player.dialogueInterpreter.open(CiderOnForesterDialogue(), with)`.
    fn open_cider_dialogue(&mut self, player: &Self::Node, with: &Self::Node) -> Result<()>;
}

/// What `CiderOnForesterDialogue.handle` calls outside its body.
pub trait DialogueHost {
    type Node: Clone;
    /// `stage` of the DialogueFile.
    fn stage(&mut self) -> Result<i32>;
    fn set_stage(&mut self, stage: i32) -> Result<()>;
    /// `this.npc!!.id`: `Error::Null` without an NPC.
    fn npc_id(&mut self) -> Result<i32>;
    /// ContentAPI `sendNPCDialogue(player, npc, msg)` (NEUTRAL, shown).
    fn send_npc_dialogue(&mut self, player: &Self::Node, npc: i32, text: &str) -> Result<()>;
    /// `player.inventory.remove(Item(id, amount))`.
    fn remove_from_inventory(&mut self, player: &Self::Node, id: i32, amount: i32) -> Result<bool>;
    /// `player.getAttribute(key, default)` of an Int.
    fn attribute_int(&mut self, player: &Self::Node, key: &str, default: i32) -> Result<i32>;
    /// `player.setAttribute(key, value)` of an Int.
    fn set_attribute_int(&mut self, player: &Self::Node, key: &str, value: i32) -> Result<()>;
    /// `player.incrementAttribute(key, amount)`.
    fn increment_attribute(&mut self, player: &Self::Node, key: &str, amount: i32) -> Result<()>;
    /// `player.removeAttribute(key)`.
    fn remove_attribute(&mut self, player: &Self::Node, key: &str) -> Result<()>;
    /// `achievementDiaryManager.finishTask(player, SEERS_VILLAGE, level,
    /// index)`.
    fn finish_seers_task(&mut self, player: &Self::Node, level: i32, index: i32) -> Result<()>;
    /// `achievementDiaryManager.hasCompletedTask(SEERS_VILLAGE, level,
    /// index)`.
    fn seers_task_completed(&mut self, player: &Self::Node, level: i32, index: i32)
    -> Result<bool>;
    /// `end()`.
    fn end(&mut self) -> Result<()>;
}

/// `CiderOnForesterDialogue`: its `init` field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CiderDialogue {
    pub init: bool,
}
impl Default for CiderDialogue {
    fn default() -> Self {
        Self { init: true }
    }
}
impl CiderDialogue {
    /// `handle(componentID, buttonID)`. Stage 2 does not exist: the second
    /// line sets stage 3.
    pub fn handle<H: DialogueHost + ?Sized>(&mut self, h: &mut H, player: &H::Node) -> Result<()> {
        // `if(init) stage = 0; init = false`.
        if self.init {
            h.set_stage(0)?;
        }
        self.init = false;
        match h.stage()? {
            0 => {
                let npc = h.npc_id()?;
                h.send_npc_dialogue(player, npc, "Ah, a glass of cider, Don't mind if I do!")?;
                h.set_stage(1)
            }
            1 => {
                let npc = h.npc_id()?;
                h.send_npc_dialogue(player, npc, "Thanks!")?;
                // The removal's answer is not read.
                h.remove_from_inventory(player, CIDER, 1)?;
                h.set_stage(3)
            }
            3 => {
                if h.attribute_int(player, SEERS_CIDER_PUB, -1)? == 4 {
                    h.finish_seers_task(player, SEERS_EASY, SEERS_CIDER_TASK)?;
                    h.remove_attribute(player, SEERS_CIDER_PUB)?;
                    h.end()
                } else if !(0..=3).contains(&h.attribute_int(player, SEERS_CIDER_PUB, -1)?)
                    && !h.seers_task_completed(player, SEERS_EASY, SEERS_CIDER_TASK)?
                {
                    h.set_attribute_int(player, SEERS_CIDER_PUB, 1)?;
                    h.end()
                } else if (0..=3).contains(&h.attribute_int(player, SEERS_CIDER_PUB, -1)?) {
                    h.increment_attribute(player, SEERS_CIDER_PUB, 1)?;
                    h.end()
                } else {
                    h.end()
                }
            }
            _ => Ok(()),
        }
    }
}

/// The use-with lambdas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interaction {
    /// FeedOnStrayDog's `onUseWith(IntType.NPC, allAllowedItems, *dogs)`.
    StrayDog,
    /// FeedFaladorSheepdog's `onUseAnyWith(IntType.NPC, SHEEPDOG_2311)`.
    Sheepdog,
    /// CiderOnForester's `onUseWith(IntType.NPC, CIDER_5763, *ids)`.
    Cider,
}
impl crate::world::component_objects::NativeObject for Interaction {}
impl Interaction {
    /// The lambda's class.
    pub fn class(self) -> &'static str {
        match self {
            Self::StrayDog => {
                "content.global.handlers.item.withnpc.FeedOnStrayDog$defineListeners$1"
            }
            Self::Sheepdog => {
                "content.global.handlers.item.withnpc.FeedFaladorSheepdog$defineListeners$1"
            }
            Self::Cider => "content.global.handlers.item.withnpc.CiderOnForester$defineListeners$1",
        }
    }
}

/// The three listener classes, by their scanned names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Cider,
    Sheepdog,
    StrayDog,
}
impl Class {
    pub fn name(self) -> &'static str {
        match self {
            Self::Cider => "content.global.handlers.item.withnpc.CiderOnForester",
            Self::Sheepdog => "content.global.handlers.item.withnpc.FeedFaladorSheepdog",
            Self::StrayDog => "content.global.handlers.item.withnpc.FeedOnStrayDog",
        }
    }
    pub fn named(name: &str) -> Option<Self> {
        [Self::Cider, Self::Sheepdog, Self::StrayDog]
            .into_iter()
            .find(|c| c.name() == name)
    }
}

/// A class's `defineListeners` over the native InteractionListeners
/// registry: FeedOnStrayDog's array overload (`bones + consumableMeats`
/// against the four dogs), FeedFaladorSheepdog's `onUseAnyWith`.
pub fn define(registry: &interactions::Registry, class: Class) -> Result<()> {
    let npc = interactions::Kind::Npc.ordinal();
    let binding = |e: interactions::Error| Error::Binding(Some(format!("{e:?}")));
    match class {
        Class::StrayDog => {
            let f = Interaction::StrayDog;
            let function = interactions::Function::new(FunctionKind::UseWith, f.class(), f);
            let mut used = bones_array();
            used.extend(consumable_meats());
            registry
                .add_use_arrays(npc, &used, &STRAY_DOGS, function)
                .map_err(binding)
        }
        Class::Cider => {
            let f = Interaction::Cider;
            let function = interactions::Function::new(FunctionKind::UseWith, f.class(), f);
            registry
                .add_use_ids(npc, CIDER, &FORESTERS, function)
                .map_err(binding)
        }
        Class::Sheepdog => {
            let f = Interaction::Sheepdog;
            let function = interactions::Function::new(FunctionKind::UseWith, f.class(), f);
            registry
                .add_any(npc, &[SHEEPDOG], function)
                .map_err(binding)
        }
    }
}

/// Runs a lambda with the player, the used node and the node it is used on.
pub fn run<H: Host + ?Sized>(
    h: &mut H,
    f: Interaction,
    player: &H::Node,
    used: &H::Node,
    with: &H::Node,
) -> Result<bool> {
    match f {
        Interaction::StrayDog => stray_dog(h, player, used, with),
        Interaction::Sheepdog => sheepdog(h, player, used, with),
        Interaction::Cider => cider(h, player, with),
    }
}

/// CiderOnForester's lambda: inside the pub the dialogue opens; anywhere
/// else the answer is false.
pub fn cider<H: Host + ?Sized>(h: &mut H, player: &H::Node, with: &H::Node) -> Result<bool> {
    if h.in_borders(player, CIDER_PUB)? {
        h.open_cider_dialogue(player, with)?;
        return Ok(true);
    }
    Ok(false)
}

/// FeedOnStrayDog's lambda: the food goes (nothing happens when the removal
/// fails), the player buries it in front of the dog, and the dog answers by
/// what it was given.
pub fn stray_dog<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    used: &H::Node,
    with: &H::Node,
) -> Result<bool> {
    // `used as Item; with as NPC`.
    if h.kind(used) != Kind::Item || h.kind(with) != Kind::Npc {
        return Err(Error::Cast);
    }
    let mut woof = "Woof";
    if h.remove_item(player, used)? {
        let dog = h.definition_name(with)?.ok_or(Error::Null)?;
        let food = h.definition_name(used)?.ok_or(Error::Null)?;
        let text = format!(
            "You feed the {} your {}.",
            h.lowercase_root(&dog),
            h.lowercase_root(&food)
        );
        h.send_message(player, &text)?;
        h.animate(player, FEED_ANIMATION)?;
        let id = h.id(used)?;
        if BONES.contains(&id) {
            match id {
                BURNT_BONES => {
                    h.send_message(
                        player,
                        "The dog looks at you, disappointed, but takes the bones anyway.",
                    )?;
                    woof = "Woof...";
                }
                4830 | 4832 | 4834 => {
                    h.send_message(
                        player,
                        "The dog looks at you, confused, but takes the bones anyway.",
                    )?;
                    woof = "Woof..?";
                }
                534 | 532 => {
                    h.send_message(player, "The dog seems to be very happy.")?;
                    woof = "Woof!";
                }
                6812 | 536 => {
                    h.send_message(player, "The dog seems to be extremely overjoyed.")?;
                    woof = "WOOF!";
                }
                _ => {}
            }
        } else if consumable_meats().contains(&h.id(used)?) {
            h.send_message(player, "The dog wagged its tail happily.")?;
            h.animate(with, DOG_ANIMATION)?;
            woof = "Woof woof!";
        }
        h.send_chat(with, woof)?;
    }
    Ok(true)
}

/// FeedFaladorSheepdog's lambda: bones other than burnt ones and meat are
/// eaten (a dialogue line each); anything else is growled at.
pub fn sheepdog<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    used: &H::Node,
    with: &H::Node,
) -> Result<bool> {
    let id = h.id(used)?;
    // `used.asItem()` / `with.asNpc()` are casts.
    let as_item = |h: &H| -> Result<()> {
        if h.kind(used) == Kind::Item {
            Ok(())
        } else {
            Err(Error::Cast)
        }
    };
    let as_npc = |h: &H| -> Result<()> {
        if h.kind(with) == Kind::Npc {
            Ok(())
        } else {
            Err(Error::Cast)
        }
    };
    if id != BURNT_BONES && BONES.contains(&id) {
        as_item(h)?;
        if !h.remove_item(player, used)? {
            return Ok(true);
        }
        let name = h.name(used)?.ok_or(Error::Null)?;
        let text = format!(
            "You give the dog some nice {}. It happily gnaws on them.",
            h.lowercase_default(&name)
        );
        h.send_dialogue(player, &text)?;
    } else if consumable_meats().contains(&id) {
        as_item(h)?;
        if !h.remove_item(player, used)? {
            return Ok(true);
        }
        h.send_dialogue(
            player,
            "You give the dog a nice piece of meat. It gobbles it up.",
        )?;
    } else {
        h.send_message(player, "The dog doesn't seem interested in that.")?;
        as_npc(h)?;
        h.send_chat(with, "Grrrr!")?;
        return Ok(true);
    }
    h.animate(player, FEED_ANIMATION)?;
    as_npc(h)?;
    h.send_chat(with, "Woof woof!")?;
    Ok(true)
}
