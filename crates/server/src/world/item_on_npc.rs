//! Item-on-NPC basics of `content/global/handlers/item/withnpc`: feeding the
//! stray dogs (`FeedOnStrayDog`) and the Falador sheepdog
//! (`FeedFaladorSheepdog`), with the two tables they read (`Bones.array`,
//! `content.data.Meat`).
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
}

/// The use-with lambdas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interaction {
    /// FeedOnStrayDog's `onUseWith(IntType.NPC, allAllowedItems, *dogs)`.
    StrayDog,
    /// FeedFaladorSheepdog's `onUseAnyWith(IntType.NPC, SHEEPDOG_2311)`.
    Sheepdog,
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
        }
    }
}

/// The two listener classes, by their scanned names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Sheepdog,
    StrayDog,
}
impl Class {
    pub fn name(self) -> &'static str {
        match self {
            Self::Sheepdog => "content.global.handlers.item.withnpc.FeedFaladorSheepdog",
            Self::StrayDog => "content.global.handlers.item.withnpc.FeedOnStrayDog",
        }
    }
    pub fn named(name: &str) -> Option<Self> {
        [Self::Sheepdog, Self::StrayDog]
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
    }
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
