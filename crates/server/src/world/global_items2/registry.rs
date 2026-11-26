//! The classes' registrations and the typed callbacks they register.
//!
//! Each Kotlin lambda and OptionHandler the original registers is a typed
//! native value: [`Interaction`] (InteractionListeners `on`, use-with and
//! equip), [`InterfaceFn`] (InterfaceListeners) and [`OptionPlugin`] (item
//! definition option handlers). [`define`] performs a class's registration
//! in the original's order through a [`Registrar`] (ClassScanner's order for
//! a content class: login listener, interaction listeners, interface
//! listeners, player-save hook); [`register_scanned`] walks the start-up scan
//! manifest. [`Native`] is the registrar over the native registries. The
//! `run_*` functions dispatch a registered value to its body. Class names are
//! real program metadata; they never select behaviour.
use super::random::Random;
use super::tables;
use super::toys::Toy;
use super::{
    Definition, Dialogue, Error, Host, Result, RunBody, Task, dialogues, diango, dig, everyday,
    gloves, toys, withobject,
};
use crate::definitions::option_handler::{self, NativeOptionHandler};
use crate::world::component_objects::NativeObject;
use crate::world::dark_zone::LightSource;
use crate::world::drop_tables::WeightBasedTable;
use crate::world::interaction_listeners::{self as interactions, Kind as IntType};
use crate::world::interface_listeners;

const H: &str = "content.global.handlers.item.";

/// An InteractionListeners function (one per Kotlin lambda object).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Interaction {
    /// EmptyOptionListener's `empty`.
    Empty,
    /// LightSourceListeners' `drop`.
    LightDrop,
    /// CasketListener's `open`.
    Casket,
    /// BirdNestScript's `search` (`::handleNest`, bound to the listener).
    NestSearch,
    /// ItemQuestRequirementListener's `onEquip` lambdas, by position.
    QuestEquip(u8),
    /// MorphItemListener's `onEquip` (captures the listener).
    MorphEquip,
    /// NewspaperListener's newspaper `read`.
    Newspaper,
    /// NewspaperListener's flyer `read`.
    Flyer,
    /// ToyHorseListener's `play-with` (captures the listener).
    Horsey,
    /// ToyListeners' lambdas.
    Toy(Toy),
    /// FOGGlovesListener's `inspect`.
    FogInspect,
    /// ChestKeyListener's use-with.
    ChestKey,
    /// HatStand's use-with.
    HatStand,
    /// SackOnHay's use-with.
    SackOnHay,
    /// SandSourceListener's use-with.
    Sand,
    /// WaterSourceListener's use-with (captures the listener).
    Water,
    /// CadavaPotionListener's `drink` (captures the listener).
    CadavaDrink,
    /// CadavaPotionListener's `look-at` (captures the listener).
    CadavaLook,
    /// DrinkBlamishOilListener's `drink`.
    BlamishOil,
    /// EnchantedGemListener's `activate`.
    Gem,
    /// PetRockListener's `interact`.
    PetRock,
}
impl NativeObject for Interaction {}

impl Interaction {
    /// The Kotlin class the original compiles the lambda to.
    pub fn class(self) -> String {
        let (owner, n) = match self {
            Self::Empty => ("EmptyOptionListener", 1),
            Self::LightDrop => ("LightSourceListeners", 1),
            Self::Casket => ("CasketListener", 1),
            Self::NestSearch => ("BirdNestScript", 1),
            Self::QuestEquip(k) => ("ItemQuestRequirementListener", usize::from(k) + 1),
            Self::MorphEquip => ("MorphItemListener", 1),
            Self::Newspaper => ("NewspaperListener", 1),
            Self::Flyer => ("NewspaperListener", 2),
            Self::Horsey => ("ToyHorseListener", 1),
            Self::Toy(t) => (
                "ToyListeners",
                Toy::ALL.iter().position(|x| *x == t).unwrap_or(0) + 1,
            ),
            Self::FogInspect => ("equipment.fistofguthixgloves.FOGGlovesListener", 1),
            Self::ChestKey => ("withobject.ChestKeyListener", 1),
            Self::HatStand => ("withobject.HatStand", 1),
            Self::SackOnHay => ("withobject.SackOnHay", 1),
            Self::Sand => ("withobject.SandSourceListener", 1),
            Self::Water => ("withobject.WaterSourceListener", 1),
            Self::CadavaDrink => {
                return "rs09.game.interaction.object.CadavaPotionListener$defineListeners$1"
                    .into();
            }
            Self::CadavaLook => {
                return "rs09.game.interaction.object.CadavaPotionListener$defineListeners$2"
                    .into();
            }
            Self::BlamishOil => ("DrinkBlamishOilListener", 1),
            Self::Gem => ("EnchantedGemListener", 1),
            Self::PetRock => ("PetRockListener", 1),
        };
        format!("{H}{owner}$defineListeners${n}")
    }
    /// The registry's function kind.
    pub fn kind(self) -> interactions::FunctionKind {
        match self {
            Self::ChestKey | Self::HatStand | Self::SackOnHay | Self::Sand | Self::Water => {
                interactions::FunctionKind::UseWith
            }
            _ => interactions::FunctionKind::On,
        }
    }
    /// A lambda that captures nothing is one object (its class's INSTANCE)
    /// however often it is registered; the rest are new objects.
    pub fn shared(self) -> bool {
        !matches!(
            self,
            Self::NestSearch
                | Self::MorphEquip
                | Self::Horsey
                | Self::Water
                | Self::CadavaDrink
                | Self::CadavaLook
        )
    }
}

/// An InterfaceListeners function.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InterfaceFn {
    /// MorphItemListener's button listener of interface 375.
    MorphButton,
    /// MorphItemListener's close listener of interface 375 (captures the
    /// listener).
    MorphClose,
}
impl NativeObject for InterfaceFn {}
impl InterfaceFn {
    pub fn class(self) -> String {
        let n = match self {
            Self::MorphButton => 1,
            Self::MorphClose => 2,
        };
        format!("{H}MorphItemListener$defineInterfaceListeners${n}")
    }
    pub fn shared(self) -> bool {
        self == Self::MorphButton
    }
}

/// An item definition OptionHandler (one object per `newInstance`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionPlugin {
    HerbBox,
    Lamp,
    RingUncharge,
    RunePack,
    SpadeDig,
    StarterPack,
    /// BirdNestPlugin (not `@Initializable`: start-up never registers it).
    BirdNest,
    /// DiangoOptionHandler (an NPC definition handler).
    Diango,
}
impl NativeOptionHandler for OptionPlugin {}
impl OptionPlugin {
    pub fn class(self) -> &'static str {
        match self {
            Self::HerbBox => "content.global.handlers.item.HerbBoxPlugin",
            Self::Lamp => "content.global.handlers.item.LampPlugin",
            Self::RingUncharge => "content.global.handlers.item.RingUnchargePlugin",
            Self::RunePack => "content.global.handlers.item.RunePackPlugin",
            Self::SpadeDig => "content.global.handlers.item.SpadeDigOptionPlugin",
            Self::StarterPack => "content.global.handlers.item.StarterPackPlugin",
            Self::BirdNest => "content.global.handlers.item.BirdNestPlugin",
            Self::Diango => "content.global.handlers.item.toys.DiangoOptionHandler",
        }
    }
    /// `isWalk()`: false where the plugin overrides it.
    pub fn is_walk(self) -> bool {
        matches!(self, Self::RingUncharge | Self::RunePack | Self::Diango)
    }
    /// `isWalk(player, node)`: OptionHandler's false.
    pub fn is_walk_for(self) -> bool {
        false
    }
    /// `isDelayed(player)`: OptionHandler's true.
    pub fn is_delayed(self) -> bool {
        true
    }
    /// The native option handler object for the item definitions.
    pub fn native_handler(self) -> option_handler::Handle {
        option_handler::Handler::new(self.class(), self)
    }
}

/// A ComponentPlugin the classes register (`ComponentDefinition.put`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentPlugin {
    /// DiangoReclaimInterface (interface 468).
    DiangoReclaim,
    /// SnowGlobePlugin's inner SnowGlobeInterface (interface 659).
    SnowGlobe,
}
impl ComponentPlugin {
    pub fn class(self) -> &'static str {
        match self {
            Self::DiangoReclaim => "content.global.handlers.item.toys.DiangoReclaimInterface",
            Self::SnowGlobe => {
                "content.global.handlers.item.toys.SnowGlobePlugin$SnowGlobeInterface"
            }
        }
    }
    /// The interface it handles.
    pub fn component(self) -> i32 {
        match self {
            Self::DiangoReclaim => diango::RECLAIM_INTERFACE,
            Self::SnowGlobe => diango::SNOW_GLOBE_INTERFACE,
        }
    }
}

/// The ported classes, by their original names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    BirdNestScript,
    Casket,
    BlamishOil,
    EmptyOption,
    ItemQuestRequirement,
    LightSource,
    MorphItem,
    Newspaper,
    ToyHorse,
    Toys,
    BrawlingGloves,
    FogGloves,
    ChestKey,
    HatStand,
    SackOnHay,
    SandSource,
    WaterSource,
    CadavaPotion,
    HerbBox,
    Lamp,
    RingUncharge,
    RunePack,
    SpadeDig,
    StarterPack,
    BirdNestPlugin,
    EnchantedGem,
    PetRock,
    Diango,
    DiangoReclaim,
    SnowGlobe,
}
impl Class {
    pub const ALL: [Self; 30] = [
        Self::BirdNestScript,
        Self::Casket,
        Self::BlamishOil,
        Self::EmptyOption,
        Self::EnchantedGem,
        Self::ItemQuestRequirement,
        Self::LightSource,
        Self::MorphItem,
        Self::Newspaper,
        Self::PetRock,
        Self::ToyHorse,
        Self::Toys,
        Self::BrawlingGloves,
        Self::FogGloves,
        Self::ChestKey,
        Self::HatStand,
        Self::SackOnHay,
        Self::SandSource,
        Self::WaterSource,
        Self::CadavaPotion,
        Self::HerbBox,
        Self::Lamp,
        Self::RingUncharge,
        Self::RunePack,
        Self::SpadeDig,
        Self::StarterPack,
        Self::Diango,
        Self::DiangoReclaim,
        Self::SnowGlobe,
        Self::BirdNestPlugin,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::BirdNestScript => "content.global.handlers.item.BirdNestScript",
            Self::Casket => "content.global.handlers.item.CasketListener",
            Self::BlamishOil => "content.global.handlers.item.DrinkBlamishOilListener",
            Self::EmptyOption => "content.global.handlers.item.EmptyOptionListener",
            Self::ItemQuestRequirement => {
                "content.global.handlers.item.ItemQuestRequirementListener"
            }
            Self::LightSource => "content.global.handlers.item.LightSourceListeners",
            Self::MorphItem => "content.global.handlers.item.MorphItemListener",
            Self::Newspaper => "content.global.handlers.item.NewspaperListener",
            Self::ToyHorse => "content.global.handlers.item.ToyHorseListener",
            Self::Toys => "content.global.handlers.item.ToyListeners",
            Self::BrawlingGloves => {
                "content.global.handlers.item.equipment.brawling_gloves.BrawlingGlovesManager"
            }
            Self::FogGloves => {
                "content.global.handlers.item.equipment.fistofguthixgloves.FOGGlovesListener"
            }
            Self::ChestKey => "content.global.handlers.item.withobject.ChestKeyListener",
            Self::HatStand => "content.global.handlers.item.withobject.HatStand",
            Self::SackOnHay => "content.global.handlers.item.withobject.SackOnHay",
            Self::SandSource => "content.global.handlers.item.withobject.SandSourceListener",
            Self::WaterSource => "content.global.handlers.item.withobject.WaterSourceListener",
            Self::CadavaPotion => "rs09.game.interaction.object.CadavaPotionListener",
            Self::HerbBox => "content.global.handlers.item.HerbBoxPlugin",
            Self::Lamp => "content.global.handlers.item.LampPlugin",
            Self::RingUncharge => "content.global.handlers.item.RingUnchargePlugin",
            Self::RunePack => "content.global.handlers.item.RunePackPlugin",
            Self::SpadeDig => "content.global.handlers.item.SpadeDigOptionPlugin",
            Self::StarterPack => "content.global.handlers.item.StarterPackPlugin",
            Self::BirdNestPlugin => "content.global.handlers.item.BirdNestPlugin",
            Self::EnchantedGem => "content.global.handlers.item.EnchantedGemListener",
            Self::PetRock => "content.global.handlers.item.PetRockListener",
            Self::Diango => "content.global.handlers.item.toys.DiangoOptionHandler",
            Self::DiangoReclaim => "content.global.handlers.item.toys.DiangoReclaimInterface",
            Self::SnowGlobe => "content.global.handlers.item.toys.SnowGlobePlugin",
        }
    }
    /// The class of a scanned name, if this module ports it.
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.name() == name)
    }
}

/// The registries a class registers into, in the original's terms.
pub trait Registrar {
    /// A lambda object.
    type FunctionRef: Clone;
    /// An interface listener lambda object.
    type InterfaceRef: Clone;
    /// An OptionHandler object (`this` of the plugin).
    type HandlerRef: Clone;
    /// A content class instance (a LoginListener and PersistPlayer).
    type ObjectRef: Clone;
    /// A ComponentPlugin object.
    type ComponentRef: Clone;
    /// The lambda object a registration passes ([`Interaction::shared`]
    /// lambdas are one object).
    fn function(&mut self, f: Interaction) -> Self::FunctionRef;
    fn interface_function(&mut self, f: InterfaceFn) -> Self::InterfaceRef;
    /// Constructs a plugin's OptionHandler object.
    fn handler_object(&mut self, handler: OptionPlugin) -> Self::HandlerRef;
    /// Constructs a content class's instance.
    fn content_object(&mut self, class: Class) -> Self::ObjectRef;
    /// Constructs a ComponentPlugin object.
    fn component_object(&mut self, plugin: ComponentPlugin) -> Self::ComponentRef;
    /// `NPCDefinition.forId(id).getHandlers().put(key, handler)`.
    fn put_npc_handler(&mut self, id: i32, key: &str, handler: &Self::HandlerRef) -> Result<()>;
    /// `ComponentDefinition.put(id, plugin)`.
    fn component_plugin(&mut self, id: i32, plugin: &Self::ComponentRef) -> Result<()>;
    /// `ClassScanner.definePlugin(plugin)` before its `newInstance(null)`.
    fn define_plugin(&mut self, plugin: &Self::ComponentRef) -> Result<()>;
    /// `DiangoReclaimInterface.ITEMS`.
    fn diango_items(&mut self) -> &mut Vec<i32>;
    /// `InteractionListener.on(id(s), type, options..., handler)`.
    fn on(
        &mut self,
        ids: &[i32],
        kind: IntType,
        options: &[&str],
        f: &Self::FunctionRef,
    ) -> Result<()>;
    /// `onUseWith(type, used, with..., handler)`; `array` is the overload
    /// with an array of used ids.
    fn on_use_with(
        &mut self,
        kind: IntType,
        used: &[i32],
        array: bool,
        with: &[i32],
        f: &Self::FunctionRef,
    ) -> Result<()>;
    /// `InteractionListeners.addEquip(id, handler)`.
    fn on_equip(&mut self, id: i32, f: &Self::FunctionRef) -> Result<()>;
    /// `InterfaceListener.on(componentID, handler)`.
    fn on_interface(&mut self, id: i32, f: &Self::InterfaceRef) -> Result<()>;
    /// `InterfaceListener.onClose(componentID, handler)`.
    fn on_interface_close(&mut self, id: i32, f: &Self::InterfaceRef) -> Result<()>;
    /// `ItemDefinition.forId(id).getHandlers().put(key, handler)`.
    fn put_item_handler(&mut self, id: i32, key: &str, handler: &Self::HandlerRef) -> Result<()>;
    /// `GameWorld.loginListeners.add(instance)`.
    fn login_listener(&mut self, object: &Self::ObjectRef) -> Result<()>;
    /// `PlayerSaver.contentHooks.add(instance)`.
    fn persist_player(&mut self, object: &Self::ObjectRef) -> Result<()>;
    /// `ItemDefinition.getDefinitions().values`, in that HashMap's order.
    fn definitions(&mut self) -> Vec<Definition>;
    /// `lowercase(Locale.getDefault())`.
    fn lowercase_default(&self, text: &str) -> String;
    /// The generators a class initialiser draws from.
    fn random(&mut self) -> &mut dyn Random;
    /// `CasketListener.loot` (`None` until the class initialises).
    fn casket_table(&mut self) -> &mut Option<WeightBasedTable>;
}

/// A class's registration: ClassScanner's order for a content class, or
/// `newInstance(null)` for a plugin.
pub fn define<R: Registrar + ?Sized>(r: &mut R, class: Class) -> Result<()> {
    use Interaction as I;
    match class {
        Class::BirdNestScript => {
            let ids: Vec<i32> = tables::BirdNest::ALL.iter().map(|n| n.nest().id).collect();
            let f = r.function(I::NestSearch);
            r.on(&ids, IntType::Item, &["search"], &f)
        }
        Class::Casket => {
            if r.casket_table().is_none() {
                let table = everyday::casket_table(r.random());
                *r.casket_table() = Some(table);
            }
            let f = r.function(I::Casket);
            r.on(&[everyday::CASKET], IntType::Item, &["open"], &f)
        }
        Class::BlamishOil => {
            let f = r.function(I::BlamishOil);
            r.on(&[1582], IntType::Item, &["drink"], &f)
        }
        Class::EmptyOption => {
            let definitions = r.definitions();
            let ids = tables::empty_item_list(&definitions, |text| r.lowercase_default(text))?;
            let f = r.function(I::Empty);
            r.on(
                &ids,
                IntType::Item,
                &["empty", "empty bowl", "empty dish"],
                &f,
            )
        }
        Class::ItemQuestRequirement => {
            for (k, (_, ids)) in tables::quest_equipment().into_iter().enumerate() {
                let f = r.function(I::QuestEquip(k as u8));
                for id in ids {
                    r.on_equip(id, &f)?;
                }
            }
            Ok(())
        }
        Class::LightSource => {
            let ids: Vec<i32> = LightSource::ALL.iter().map(|l| l.product()).collect();
            let f = r.function(I::LightDrop);
            r.on(&ids, IntType::Item, &["drop"], &f)
        }
        Class::MorphItem => {
            let f = r.function(I::MorphEquip);
            for id in tables::MORPH_ITEMS {
                r.on_equip(id, &f)?;
            }
            let button = r.interface_function(InterfaceFn::MorphButton);
            r.on_interface(tables::MORPH_INTERFACE, &button)?;
            let close = r.interface_function(InterfaceFn::MorphClose);
            r.on_interface_close(tables::MORPH_INTERFACE, &close)
        }
        Class::Newspaper => {
            let f = r.function(I::Newspaper);
            r.on(&[11169], IntType::Item, &["read"], &f)?;
            let f = r.function(I::Flyer);
            r.on(&[7922], IntType::Item, &["read"], &f)
        }
        Class::ToyHorse => {
            let ids: Vec<i32> = tables::HORSEY_MAP.iter().map(|(id, _)| *id).collect();
            let f = r.function(I::Horsey);
            r.on(&ids, IntType::Item, &["play-with"], &f)
        }
        Class::Toys => {
            for toy in Toy::ALL {
                let (ids, options) = toy.registration();
                let f = r.function(I::Toy(toy));
                r.on(ids, IntType::Item, options, &f)?;
            }
            Ok(())
        }
        Class::BrawlingGloves => {
            let o = r.content_object(class);
            r.login_listener(&o)?;
            r.persist_player(&o)
        }
        Class::FogGloves => {
            let ids: Vec<i32> = (tables::FOG_FIRST..tables::FOG_FIRST + 10).collect();
            let f = r.function(I::FogInspect);
            r.on(&ids, IntType::Item, &["inspect"], &f)
        }
        Class::ChestKey => {
            let f = r.function(I::ChestKey);
            r.on_use_with(
                IntType::Scenery,
                &[withobject::CHEST_KEY],
                false,
                &[withobject::CHEST],
                &f,
            )
        }
        Class::HatStand => {
            let definitions = r.definitions();
            let hats = withobject::hats(&definitions);
            let f = r.function(I::HatStand);
            r.on_use_with(IntType::Scenery, &hats, true, &[withobject::HAT_STAND], &f)
        }
        Class::SackOnHay => {
            let f = r.function(I::SackOnHay);
            r.on_use_with(
                IntType::Scenery,
                &[withobject::EMPTY_SACK],
                false,
                &tables::HAY,
                &f,
            )
        }
        Class::SandSource => {
            let f = r.function(I::Sand);
            r.on_use_with(
                IntType::Scenery,
                &[withobject::BUCKET],
                false,
                &tables::SAND_PITS,
                &f,
            )
        }
        Class::WaterSource => {
            let inputs = tables::WaterVessel::input_keys();
            let f = r.function(I::Water);
            r.on_use_with(IntType::Scenery, &inputs, true, &tables::WATER_SOURCES, &f)
        }
        Class::CadavaPotion => {
            let f = r.function(I::CadavaDrink);
            r.on(&[everyday::CADAVA_POTION], IntType::Item, &["drink"], &f)?;
            let f = r.function(I::CadavaLook);
            r.on(&[everyday::CADAVA_POTION], IntType::Item, &["look-at"], &f)
        }
        Class::HerbBox => {
            let h = r.handler_object(OptionPlugin::HerbBox);
            for key in ["option:bank-all", "option:take-one", "option:check"] {
                r.put_item_handler(everyday::HERB_BOX, key, &h)?;
            }
            Ok(())
        }
        Class::Lamp => {
            let h = r.handler_object(OptionPlugin::Lamp);
            for (id, _, _) in tables::LAMPS {
                r.put_item_handler(id, "option:rub", &h)?;
            }
            Ok(())
        }
        Class::RingUncharge => {
            let h = r.handler_object(OptionPlugin::RingUncharge);
            for id in tables::IMBUED_RINGS {
                r.put_item_handler(id, "option:uncharge", &h)?;
            }
            Ok(())
        }
        Class::RunePack => {
            let h = r.handler_object(OptionPlugin::RunePack);
            for id in tables::RUNE_PACKS {
                r.put_item_handler(id, "option:open", &h)?;
            }
            Ok(())
        }
        Class::SpadeDig => {
            let h = r.handler_object(OptionPlugin::SpadeDig);
            r.put_item_handler(dig::SPADE, "option:dig", &h)
        }
        Class::StarterPack => {
            let h = r.handler_object(OptionPlugin::StarterPack);
            r.put_item_handler(everyday::STARTER_PACK, "option:open", &h)
        }
        Class::EnchantedGem => {
            let f = r.function(I::Gem);
            r.on(
                &[dialogues::ENCHANTED_GEM],
                IntType::Item,
                &["activate"],
                &f,
            )
        }
        Class::PetRock => {
            let f = r.function(I::PetRock);
            r.on(&[dialogues::PET_ROCK], IntType::Item, &["interact"], &f)
        }
        Class::Diango => {
            let h = r.handler_object(OptionPlugin::Diango);
            r.put_npc_handler(diango::DIANGO, "option:holiday-items", &h)
        }
        Class::DiangoReclaim => {
            let c = r.component_object(ComponentPlugin::DiangoReclaim);
            r.component_plugin(diango::RECLAIM_INTERFACE, &c)?;
            diango::reclaim_new_instance(r.diango_items());
            Ok(())
        }
        Class::SnowGlobe => {
            let c = r.component_object(ComponentPlugin::SnowGlobe);
            r.define_plugin(&c)?;
            r.component_plugin(diango::SNOW_GLOBE_INTERFACE, &c)
        }
        Class::BirdNestPlugin => {
            let h = r.handler_object(OptionPlugin::BirdNest);
            for nest in tables::BirdNest::ALL {
                r.put_item_handler(nest.nest().id, "option:search", &h)?;
            }
            Ok(())
        }
    }
}

/// Registers every ported class in the original's order: the start-up
/// manifest's content interfaces, then its `@Initializable` classes.
/// BirdNestPlugin is in neither. Returns the classes registered.
pub fn register_scanned<R: Registrar + ?Sized>(r: &mut R) -> Result<Vec<Class>> {
    let mut done = Vec::new();
    for section in ["scan.contentInterfaces", "scan.initializable"] {
        for name in crate::startup::scan::section(section) {
            if let Some(class) = Class::named(name) {
                define(r, class)?;
                done.push(class);
            }
        }
    }
    Ok(done)
}

// ------------------------------------------------------------------ dispatch

/// Runs an InteractionListeners function: `on` and equip lambdas get the
/// node, use-with lambdas the used and the with nodes.
pub fn run_interaction<H: Host + ?Sized>(
    h: &mut H,
    f: Interaction,
    p: &H::Node,
    node: &H::Node,
    with: Option<&H::Node>,
) -> Result<bool> {
    use Interaction as I;
    let with = || with.ok_or(Error::Null);
    match f {
        I::Empty => everyday::empty(h, p, node),
        I::LightDrop => everyday::drop_light(h, p, node),
        I::Casket => everyday::casket(h, p, node),
        I::NestSearch => everyday::nest_script(h, p, node),
        I::QuestEquip(k) => {
            let quest = tables::quest_equipment()
                .get(usize::from(k))
                .ok_or(Error::Index)?
                .0;
            toys::quest_equip(h, p, quest)
        }
        I::MorphEquip => toys::morph_equip(h, p, node),
        I::Newspaper => everyday::newspaper(h, p),
        I::Flyer => everyday::flyer(h, p),
        I::Horsey => toys::horsey(h, p, node),
        I::Toy(t) => toys::toy(h, t, p, node),
        I::FogInspect => gloves::fog_inspect(h, p, node),
        I::ChestKey => withobject::chest_key(h, p, node, with()?),
        I::HatStand => withobject::hat_stand(h, p),
        I::SackOnHay => withobject::sack_on_hay(h, p, node),
        I::Sand => withobject::sand(h, p, node),
        I::Water => withobject::water(h, p, node, with()?),
        I::CadavaDrink => everyday::cadava(h, p, false),
        I::CadavaLook => everyday::cadava(h, p, true),
        I::BlamishOil => everyday::blamish_oil(h, p),
        I::Gem => dialogues::gem_activate(h, p),
        I::PetRock => dialogues::pet_rock_interact(h, p),
    }
}

/// Runs an InterfaceListeners function: a button (`Some(button)`) or the
/// close listener.
pub fn run_interface<H: Host + ?Sized>(
    h: &mut H,
    f: InterfaceFn,
    p: &H::Node,
    button: Option<i32>,
) -> Result<bool> {
    match f {
        InterfaceFn::MorphButton => toys::morph_button(h, p, button.ok_or(Error::Null)?),
        InterfaceFn::MorphClose => toys::morph_close(h, p),
    }
}

/// Runs an OptionHandler's `handle(player, node, option)`; `identity` is
/// the registered object's (`this`).
pub fn run_handler<H: Host + ?Sized>(
    h: &mut H,
    handler: OptionPlugin,
    identity: usize,
    p: &H::Node,
    node: Option<&H::Node>,
    option: Option<&str>,
) -> Result<bool> {
    match handler {
        OptionPlugin::HerbBox => everyday::herb_box(h, p, node, option),
        OptionPlugin::Lamp => everyday::lamp(h, identity, p, node),
        OptionPlugin::RingUncharge => everyday::ring_uncharge(h, p, node),
        OptionPlugin::RunePack => everyday::rune_pack(h, p, node),
        OptionPlugin::SpadeDig => dig::spade(h, p),
        OptionPlugin::StarterPack => everyday::starter_pack(h, p, node, option),
        OptionPlugin::BirdNest => everyday::nest_plugin(h, p, node),
        OptionPlugin::Diango => diango::holiday_items(h, p),
    }
}

/// `plugin.handleSelectionCallback(skill, player)` (Plugin's default does
/// nothing; LampPlugin overrides it).
pub fn run_selection<H: Host + ?Sized>(
    h: &mut H,
    handler: OptionPlugin,
    skill: i32,
    p: &H::Node,
) -> Result<()> {
    match handler {
        OptionPlugin::Lamp => everyday::lamp_select(h, skill, p),
        _ => Ok(()),
    }
}

/// A ComponentPlugin's `handle(player, component, opcode, button, slot, itemId)`.
pub fn run_component<H: Host + ?Sized>(
    h: &mut H,
    plugin: ComponentPlugin,
    p: &H::Node,
    opcode: i32,
    button: i32,
    slot: i32,
) -> Result<bool> {
    match plugin {
        ComponentPlugin::DiangoReclaim => diango::reclaim_handle(h, p, opcode, slot),
        ComponentPlugin::SnowGlobe => diango::snow_globe_handle(h, p, button),
    }
}

/// A dialogue file's `handle(componentID, buttonID)`.
pub fn run_dialogue<H: Host + ?Sized>(
    h: &mut H,
    dialogue: &mut Dialogue<H::Node>,
    p: &H::Node,
    _component: i32,
    button: i32,
) -> Result<()> {
    match dialogue {
        Dialogue::Gem {
            first_run,
            stage,
            npc,
        } => dialogues::gem_handle(h, p, first_run, stage, npc),
        Dialogue::PetRock { stage } => dialogues::pet_rock_handle(h, p, stage, button),
    }
}

/// Runs a deferred body: a pulse's `pulse()`, a queued script or a close
/// event's `close`.
pub fn run_task<H: Host + ?Sized>(
    h: &mut H,
    task: &mut Task<H::Node, H::Callback>,
) -> Result<bool> {
    match task {
        Task::LampClose { player } => everyday::lamp_close(h, &player.clone()),
        Task::PetRockStroke { player } => dialogues::pet_rock_script(h, &player.clone()),
        Task::SnowGlobe { player } => diango::snow_globe_pulse(h, &player.clone()),
        Task::Dig { player, action } => dig::dig_pulse(h, &player.clone(), &action.clone()),
        Task::RunTask { cycles, body } => {
            match body {
                RunBody::Sand {
                    player,
                    used,
                    trigger,
                } => {
                    let (player, used) = (player.clone(), used.clone());
                    withobject::sand_task(h, &player, &used, trigger)?;
                }
                other => toys::toy_task(h, &other.clone())?,
            }
            *cycles = cycles.wrapping_sub(1);
            Ok(*cycles <= 0)
        }
        Task::WaterFill {
            player,
            used,
            with,
            vessel,
        } => withobject::water_pulse(h, &player.clone(), &used.clone(), &with.clone(), *vessel),
    }
}

// ------------------------------------------------------------------ the native registries

/// The registrar over the native registries: the InteractionListeners and
/// InterfaceListeners registries take the lambdas; item definition handlers
/// and the content objects (login listeners, save hooks) are collected for
/// the world to bind.
pub struct Native<'a, C: interactions::Casing> {
    pub interactions: &'a interactions::Registry,
    pub interfaces: &'a interface_listeners::Registry,
    pub casing: &'a C,
    pub definitions: Vec<Definition>,
    pub random: &'a mut dyn Random,
    pub casket: Option<WeightBasedTable>,
    pub item_handlers: Vec<(i32, String, option_handler::Handle)>,
    pub login_listeners: Vec<Class>,
    pub persist_players: Vec<Class>,
    pub npc_handlers: Vec<(i32, String, option_handler::Handle)>,
    pub components: Vec<(i32, ComponentPlugin)>,
    pub diango_items: Vec<i32>,
    shared: Vec<(Interaction, interactions::Function)>,
    shared_interfaces: Vec<(InterfaceFn, interface_listeners::Callback)>,
}

impl<'a, C: interactions::Casing> Native<'a, C> {
    pub fn new(
        interactions: &'a interactions::Registry,
        interfaces: &'a interface_listeners::Registry,
        casing: &'a C,
        definitions: Vec<Definition>,
        random: &'a mut dyn Random,
    ) -> Self {
        Self {
            interactions,
            interfaces,
            casing,
            definitions,
            random,
            casket: None,
            item_handlers: Vec::new(),
            login_listeners: Vec::new(),
            persist_players: Vec::new(),
            npc_handlers: Vec::new(),
            components: Vec::new(),
            diango_items: Vec::new(),
            shared: Vec::new(),
            shared_interfaces: Vec::new(),
        }
    }
}

fn interaction_error(e: interactions::Error) -> Error {
    Error::Binding(Some(format!("{e:?}")))
}

impl<C: interactions::Casing> Registrar for Native<'_, C> {
    type FunctionRef = interactions::Function;
    type InterfaceRef = interface_listeners::Callback;
    type HandlerRef = option_handler::Handle;
    type ObjectRef = Class;
    type ComponentRef = ComponentPlugin;
    fn function(&mut self, f: Interaction) -> interactions::Function {
        if f.shared()
            && let Some((_, existing)) = self.shared.iter().find(|(k, _)| *k == f)
        {
            return existing.clone();
        }
        let function = interactions::Function::new(f.kind(), f.class(), f);
        if f.shared() {
            self.shared.push((f, function.clone()));
        }
        function
    }
    fn interface_function(&mut self, f: InterfaceFn) -> interface_listeners::Callback {
        if f.shared()
            && let Some((_, existing)) = self.shared_interfaces.iter().find(|(k, _)| *k == f)
        {
            return existing.clone();
        }
        let callback = match f {
            InterfaceFn::MorphButton => interface_listeners::Callback::button(f),
            InterfaceFn::MorphClose => interface_listeners::Callback::open(f),
        };
        if f.shared() {
            self.shared_interfaces.push((f, callback.clone()));
        }
        callback
    }
    fn handler_object(&mut self, handler: OptionPlugin) -> option_handler::Handle {
        handler.native_handler()
    }
    fn content_object(&mut self, class: Class) -> Class {
        class
    }
    fn component_object(&mut self, plugin: ComponentPlugin) -> ComponentPlugin {
        plugin
    }
    fn put_npc_handler(
        &mut self,
        id: i32,
        key: &str,
        handler: &option_handler::Handle,
    ) -> Result<()> {
        self.npc_handlers.push((id, key.into(), handler.clone()));
        Ok(())
    }
    fn component_plugin(&mut self, id: i32, plugin: &ComponentPlugin) -> Result<()> {
        self.components.push((id, *plugin));
        Ok(())
    }
    fn define_plugin(&mut self, _: &ComponentPlugin) -> Result<()> {
        Ok(())
    }
    fn diango_items(&mut self) -> &mut Vec<i32> {
        &mut self.diango_items
    }
    fn on(
        &mut self,
        ids: &[i32],
        kind: IntType,
        options: &[&str],
        f: &interactions::Function,
    ) -> Result<()> {
        let options: Vec<interactions::Text> =
            options.iter().map(|o| o.encode_utf16().collect()).collect();
        self.interactions
            .add_ids(self.casing, ids, kind.ordinal(), &options, f.clone())
            .map_err(interaction_error)
    }
    fn on_use_with(
        &mut self,
        kind: IntType,
        used: &[i32],
        array: bool,
        with: &[i32],
        f: &interactions::Function,
    ) -> Result<()> {
        if array {
            return self
                .interactions
                .add_use_arrays(kind.ordinal(), used, with, f.clone())
                .map_err(interaction_error);
        }
        for &u in used {
            self.interactions
                .add_use_ids(kind.ordinal(), u, with, f.clone())
                .map_err(interaction_error)?;
        }
        Ok(())
    }
    fn on_equip(&mut self, id: i32, f: &interactions::Function) -> Result<()> {
        self.interactions
            .add_equip(id, f.clone(), true)
            .map_err(interaction_error)
    }
    fn on_interface(&mut self, id: i32, f: &interface_listeners::Callback) -> Result<()> {
        self.interfaces
            .add(id, None, f.clone())
            .map_err(|e| Error::Binding(Some(format!("{e:?}"))))
    }
    fn on_interface_close(&mut self, id: i32, f: &interface_listeners::Callback) -> Result<()> {
        self.interfaces
            .add_close(id, f.clone())
            .map_err(|e| Error::Binding(Some(format!("{e:?}"))))
    }
    fn put_item_handler(
        &mut self,
        id: i32,
        key: &str,
        handler: &option_handler::Handle,
    ) -> Result<()> {
        self.item_handlers.push((id, key.into(), handler.clone()));
        Ok(())
    }
    fn login_listener(&mut self, object: &Class) -> Result<()> {
        self.login_listeners.push(*object);
        Ok(())
    }
    fn persist_player(&mut self, object: &Class) -> Result<()> {
        self.persist_players.push(*object);
        Ok(())
    }
    fn definitions(&mut self) -> Vec<Definition> {
        self.definitions.clone()
    }
    fn lowercase_default(&self, text: &str) -> String {
        let utf16: Vec<u16> = text.encode_utf16().collect();
        String::from_utf16_lossy(&self.casing.lowercase_default(&utf16))
    }
    fn random(&mut self) -> &mut dyn Random {
        &mut *self.random
    }
    fn casket_table(&mut self) -> &mut Option<WeightBasedTable> {
        &mut self.casket
    }
}
