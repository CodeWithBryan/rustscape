//! The classes' registrations and the typed callbacks they register.
//!
//! Each Kotlin lambda and OptionHandler the original registers is a typed
//! native value: [`Interaction`] (InteractionListeners), [`OptionPlugin`]
//! (item definition option handlers) and [`DialoguePlugin`] (the dialogue
//! plugin registry). [`define`] performs a class's registration in the
//! original's order through a [`Registrar`]; [`register_scanned`] walks the
//! start-up scan manifest (content interfaces, then `@Initializable`
//! plugins). [`Native`] is the registrar over the native registries. The
//! `run_*` functions dispatch a registered value to its body. Class names
//! are real program metadata (the registries' duplicate rules print them);
//! they never select behaviour.
use super::{
    Arg, Dialogue, Error, Host, Outcome, Result, Task, TaskInput, items, jewellery, tablets,
};
use crate::definitions::option_handler::{self, NativeOptionHandler};
use crate::world::component_objects::NativeObject;
use crate::world::interaction_listeners::{self as interactions, Kind as IntType};

/// An InteractionListeners function (one per Kotlin lambda object).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Interaction {
    /// EnchantedJewelleryListener's `operate` (captures the listener).
    JewelleryOperate,
    /// EnchantedJewelleryListener's `rub` (captures the listener).
    JewelleryRub,
    /// EnchantJewelleryTabListener's `break` (captures nothing: one object).
    TabletBreak,
    /// EnchantJewelleryTabListener's use-with (captures `enchanted`).
    TabletUse { enchanted: i32 },
    /// TeleTabsListener's `break` (captures nothing).
    TeleTab,
    /// HouseTeleTabOptionListener's `break` (captures nothing).
    HouseTab,
    /// EctophialListener's use on the Ectofuntus (captures the listener).
    EctophialRefill,
    /// EctophialListener's `empty` (captures the listener).
    EctophialEmpty,
    /// ExplorersRingPlugin's `run-replenish`.
    RingReplenish,
    /// ExplorersRingPlugin's `low-alchemy`.
    RingAlchemy,
    /// ExplorersRingPlugin's `cabbage-port`.
    RingCabbage,
    /// ExplorersRingPlugin's `operate`/`rub`.
    RingOperate,
}
impl NativeObject for Interaction {}

const H: &str = "content.global.handlers.item.";

impl Interaction {
    /// The Kotlin class the original compiles the lambda to.
    pub fn class(self) -> String {
        let (owner, n) = match self {
            Self::JewelleryOperate => ("EnchantedJewelleryListener", 1),
            Self::JewelleryRub => ("EnchantedJewelleryListener", 2),
            Self::TabletBreak => ("EnchantJewelleryTabListener", 1),
            Self::TabletUse { .. } => ("EnchantJewelleryTabListener", 2),
            Self::TeleTab => ("TeleTabsListener", 1),
            Self::HouseTab => ("HouseTeleTabOptionListener", 1),
            Self::EctophialRefill => ("EctophialListener", 1),
            Self::EctophialEmpty => ("EctophialListener", 2),
            Self::RingReplenish => ("ExplorersRingPlugin", 1),
            Self::RingAlchemy => ("ExplorersRingPlugin", 2),
            Self::RingCabbage => ("ExplorersRingPlugin", 3),
            Self::RingOperate => ("ExplorersRingPlugin", 4),
        };
        format!("{H}{owner}$defineListeners${n}")
    }
    /// The registry's function kind (`on` or `onUseWith`).
    pub fn kind(self) -> interactions::FunctionKind {
        match self {
            Self::TabletUse { .. } | Self::EctophialRefill => interactions::FunctionKind::UseWith,
            _ => interactions::FunctionKind::On,
        }
    }
    /// A lambda that captures nothing is one object (its class's INSTANCE)
    /// however often it is registered; every other lambda is a new object
    /// per registration.
    pub fn shared(self) -> bool {
        matches!(self, Self::TabletBreak | Self::TeleTab | Self::HouseTab)
    }
}

/// An item definition OptionHandler (one object per `newInstance`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionPlugin {
    TeleportCrystal,
    FaladorShield,
}
impl NativeOptionHandler for OptionPlugin {}
impl OptionPlugin {
    pub fn class(self) -> &'static str {
        match self {
            Self::TeleportCrystal => "content.global.handlers.item.TeleportCrystalPlugin",
            Self::FaladorShield => "content.global.handlers.item.FaladorShieldPlugin",
        }
    }
    /// `isWalk()`: false for the teleport crystal.
    pub fn is_walk(self) -> bool {
        self != Self::TeleportCrystal
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

/// A DialoguePlugin the classes register (`init()`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialoguePlugin {
    TeleportCrystal,
}
impl DialoguePlugin {
    pub fn class(self) -> &'static str {
        match self {
            Self::TeleportCrystal => {
                "content.global.handlers.item.TeleportCrystalPlugin$TeleportCrystalDialogue"
            }
        }
    }
    /// `getIds()`.
    pub fn ids(self) -> [i32; 1] {
        match self {
            Self::TeleportCrystal => [items::CRYSTAL_DIALOGUE],
        }
    }
}

/// The ported classes, by their original names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Ectophial,
    EnchantJewelleryTab,
    EnchantedJewellery,
    ExplorersRing,
    HouseTeleTab,
    TeleTabs,
    FaladorShield,
    TeleportCrystal,
}
impl Class {
    pub const ALL: [Self; 8] = [
        Self::Ectophial,
        Self::EnchantJewelleryTab,
        Self::EnchantedJewellery,
        Self::ExplorersRing,
        Self::HouseTeleTab,
        Self::TeleTabs,
        Self::FaladorShield,
        Self::TeleportCrystal,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Ectophial => "content.global.handlers.item.EctophialListener",
            Self::EnchantJewelleryTab => "content.global.handlers.item.EnchantJewelleryTabListener",
            Self::EnchantedJewellery => "content.global.handlers.item.EnchantedJewelleryListener",
            Self::ExplorersRing => "content.global.handlers.item.ExplorersRingPlugin",
            Self::HouseTeleTab => "content.global.handlers.item.HouseTeleTabOptionListener",
            Self::TeleTabs => "content.global.handlers.item.TeleTabsListener",
            Self::FaladorShield => "content.global.handlers.item.FaladorShieldPlugin",
            Self::TeleportCrystal => "content.global.handlers.item.TeleportCrystalPlugin",
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
    /// An OptionHandler object (`this` of the plugin).
    type HandlerRef: Clone;
    /// A DialoguePlugin object (`new X()`).
    type DialogueRef: Clone;
    /// The lambda object a registration passes ([`Interaction::shared`]
    /// lambdas are one object).
    fn function(&mut self, f: Interaction) -> Self::FunctionRef;
    /// Constructs a plugin's OptionHandler object.
    fn handler_object(&mut self, handler: OptionPlugin) -> Self::HandlerRef;
    /// Constructs a DialoguePlugin object.
    fn dialogue_object(&mut self, dialogue: DialoguePlugin) -> Self::DialogueRef;
    /// `InteractionListener.on(id(s), type, options..., handler)`.
    fn on(
        &mut self,
        ids: &[i32],
        kind: IntType,
        options: &[&str],
        f: &Self::FunctionRef,
    ) -> Result<()>;
    /// `InteractionListener.onUseWith(type, used, with..., handler)`.
    fn on_use_with(
        &mut self,
        kind: IntType,
        used: i32,
        with: &[i32],
        f: &Self::FunctionRef,
    ) -> Result<()>;
    /// `ItemDefinition.forId(id).getHandlers().put(key, handler)`.
    fn put_item_handler(&mut self, id: i32, key: &str, handler: &Self::HandlerRef) -> Result<()>;
    /// `DialogueInterpreter.add(id, plugin)` (DialoguePlugin.init).
    fn add_dialogue(&mut self, id: i32, plugin: &Self::DialogueRef) -> Result<()>;
}

/// A class's `defineListeners()` / `newInstance(null)`.
pub fn define<R: Registrar + ?Sized>(r: &mut R, class: Class) -> Result<()> {
    use Interaction as I;
    match class {
        Class::Ectophial => {
            let refill = r.function(I::EctophialRefill);
            r.on_use_with(
                IntType::Scenery,
                items::ECTOPHIAL_EMPTY,
                &[items::ECTOFUNTUS],
                &refill,
            )?;
            let empty = r.function(I::EctophialEmpty);
            r.on(&[items::ECTOPHIAL], IntType::Item, &["empty"], &empty)
        }
        Class::EnchantJewelleryTab => {
            for (tablet, mapping) in tablets::TAB_MAPPING {
                let f = r.function(I::TabletBreak);
                r.on(&[tablet], IntType::Item, &["break"], &f)?;
                for (plain, enchanted) in mapping {
                    let f = r.function(I::TabletUse { enchanted });
                    r.on_use_with(IntType::Item, tablet, &[plain], &f)?;
                }
            }
            Ok(())
        }
        Class::EnchantedJewellery => {
            // The field `ids` is initialised before defineListeners runs.
            let ids = jewellery::id_map_keys();
            let operate = r.function(I::JewelleryOperate);
            r.on(&ids, IntType::Item, &["operate"], &operate)?;
            let rub = r.function(I::JewelleryRub);
            r.on(&ids, IntType::Item, &["rub"], &rub)
        }
        Class::ExplorersRing => {
            let f = r.function(I::RingReplenish);
            r.on(&items::RINGS, IntType::Item, &["run-replenish"], &f)?;
            let f = r.function(I::RingAlchemy);
            r.on(&items::RINGS, IntType::Item, &["low-alchemy"], &f)?;
            let f = r.function(I::RingCabbage);
            r.on(&items::RINGS, IntType::Item, &["cabbage-port"], &f)?;
            let f = r.function(I::RingOperate);
            r.on(&items::RINGS, IntType::Item, &["operate", "rub"], &f)
        }
        Class::HouseTeleTab => {
            let f = r.function(I::HouseTab);
            r.on(&[tablets::HOUSE_TABLET], IntType::Item, &["break"], &f)
        }
        Class::TeleTabs => {
            let f = r.function(I::TeleTab);
            r.on(&tablets::TeleTab::ids(), IntType::Item, &["break"], &f)
        }
        Class::FaladorShield => {
            let h = r.handler_object(OptionPlugin::FaladorShield);
            for key in ["option:operate", "option:prayer-restore"] {
                for id in items::SHIELDS {
                    r.put_item_handler(id, key, &h)?;
                }
            }
            Ok(())
        }
        Class::TeleportCrystal => {
            let h = r.handler_object(OptionPlugin::TeleportCrystal);
            for id in items::CRYSTALS {
                r.put_item_handler(id, "option:activate", &h)?;
            }
            let d = r.dialogue_object(DialoguePlugin::TeleportCrystal);
            for id in DialoguePlugin::TeleportCrystal.ids() {
                r.add_dialogue(id, &d)?;
            }
            Ok(())
        }
    }
}

/// Registers every ported class in the original's order: the start-up
/// manifest's content interfaces, then its `@Initializable` classes.
/// Returns the classes registered.
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

/// Runs an InteractionListeners function: `on` lambdas get the node, use-with
/// lambdas the used and the with nodes.
pub fn run_interaction<H: Host + ?Sized>(
    h: &mut H,
    f: Interaction,
    p: &H::Node,
    node: &H::Node,
    with: Option<&H::Node>,
) -> Result<bool> {
    use Interaction as I;
    match f {
        I::JewelleryOperate => jewellery::handle(h, p, node, true),
        I::JewelleryRub => jewellery::handle(h, p, node, false),
        I::TabletBreak => tablets::enchant_break(h, p),
        I::TabletUse { enchanted } => {
            tablets::enchant_use(h, p, node, with.ok_or(Error::Null)?, enchanted)
        }
        I::TeleTab => tablets::teletab(h, p, node),
        I::HouseTab => tablets::house_tablet(h, p, node),
        I::EctophialRefill => items::ectophial_refill(h, p),
        I::EctophialEmpty => items::ectophial_empty(h, p, node),
        I::RingReplenish => items::ring_replenish(h, p, node),
        I::RingAlchemy => items::ring_alchemy(h, p),
        I::RingCabbage => items::ring_cabbage(h, p),
        I::RingOperate => items::ring_operate(h, p, node),
    }
}

/// Runs an OptionHandler's `handle(player, node, option)`.
pub fn run_handler<H: Host + ?Sized>(
    h: &mut H,
    handler: OptionPlugin,
    p: &H::Node,
    node: Option<&H::Node>,
    option: Option<&str>,
) -> Result<bool> {
    match handler {
        OptionPlugin::TeleportCrystal => items::crystal(h, p, node),
        OptionPlugin::FaladorShield => items::falador_shield(h, p, node, option),
    }
}

/// `DialogueInterpreter.open(key, args)` reaching a registered plugin:
/// `newInstance(player)`, then `open(args)`. The world keeps the dialogue to
/// run its `handle` through [`run_dialogue`].
pub fn open_dialogue<H: Host + ?Sized>(
    h: &mut H,
    plugin: DialoguePlugin,
    p: &H::Node,
    args: &[Arg],
) -> (Dialogue<H::Node>, Result<bool>) {
    match plugin {
        DialoguePlugin::TeleportCrystal => {
            let mut d = items::crystal_dialogue(p);
            let opened = items::crystal_open(h, &mut d, args);
            (d, opened)
        }
    }
}

/// A dialogue file's or plugin's `handle(component, button)`.
pub fn run_dialogue<H: Host + ?Sized>(
    h: &mut H,
    dialogue: &mut Dialogue<H::Node>,
    _component: i32,
    button: i32,
) -> Result<Outcome> {
    match dialogue {
        Dialogue::Jewellery { .. } => {
            jewellery::dialogue(h, dialogue, button)?;
            Ok(Outcome::Unit)
        }
        Dialogue::Crystal { .. } => Ok(Outcome::Bool(items::crystal_handle(h, dialogue, button)?)),
    }
}

/// Runs a deferred body: a queued script with its stage, a pulse, a dialogue
/// action with its button or an item-select callback with its slot.
pub fn run_task<H: Host + ?Sized>(
    h: &mut H,
    task: &mut Task<H::Node>,
    input: TaskInput,
) -> Result<Outcome> {
    let mismatch = || Error::Binding(Some(format!("{input:?} does not run this task")));
    match (task, input) {
        (Task::Jewellery(t), TaskInput::Stage(stage)) => {
            Ok(Outcome::Bool(jewellery::teleport_script(h, t, stage)?))
        }
        (
            Task::Enchant {
                player,
                node,
                enchanted,
            },
            TaskInput::Stage(_),
        ) => Ok(Outcome::Bool(tablets::enchant_script(
            h,
            &player.clone(),
            &node.clone(),
            *enchanted,
        )?)),
        (Task::Ectophial { player, node }, TaskInput::Stage(stage)) => Ok(Outcome::Bool(
            items::ectophial_script(h, &player.clone(), &node.clone(), stage)?,
        )),
        (Task::AlchemyAction { player, remaining }, TaskInput::Button(_)) => {
            items::ring_alchemy_action(h, &player.clone(), *remaining)?;
            Ok(Outcome::Unit)
        }
        (Task::AlchemySelect { player, remaining }, TaskInput::Select { slot, .. }) => {
            items::ring_alchemy_select(h, &player.clone(), *remaining, slot)?;
            Ok(Outcome::Unit)
        }
        (
            Task::FaladorEmote {
                entity,
                level,
                delay,
            },
            TaskInput::Pulse,
        ) => Ok(Outcome::Bool(items::falador_pulse(
            h,
            &entity.clone(),
            *level,
            delay,
        )?)),
        _ => Err(mismatch()),
    }
}

// ------------------------------------------------------------------ the native registries

/// The registrar over the native registries: the InteractionListeners
/// registry (with the world's casing) takes the lambdas; item definition
/// handlers and dialogue plugins are collected for the world to bind into
/// its item definitions' `option:` properties and its dialogue plugin
/// registry.
pub struct Native<'a, C: interactions::Casing> {
    pub interactions: &'a interactions::Registry,
    pub casing: &'a C,
    pub item_handlers: Vec<(i32, String, option_handler::Handle)>,
    pub dialogue_plugins: Vec<(i32, DialoguePlugin)>,
    shared: Vec<(Interaction, interactions::Function)>,
}

impl<'a, C: interactions::Casing> Native<'a, C> {
    pub fn new(interactions: &'a interactions::Registry, casing: &'a C) -> Self {
        Self {
            interactions,
            casing,
            item_handlers: Vec::new(),
            dialogue_plugins: Vec::new(),
            shared: Vec::new(),
        }
    }
}

fn interaction_error(e: interactions::Error) -> Error {
    Error::Binding(Some(format!("{e:?}")))
}

impl<C: interactions::Casing> Registrar for Native<'_, C> {
    type FunctionRef = interactions::Function;
    type HandlerRef = option_handler::Handle;
    type DialogueRef = DialoguePlugin;
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
    fn handler_object(&mut self, handler: OptionPlugin) -> option_handler::Handle {
        handler.native_handler()
    }
    fn dialogue_object(&mut self, dialogue: DialoguePlugin) -> DialoguePlugin {
        dialogue
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
        used: i32,
        with: &[i32],
        f: &interactions::Function,
    ) -> Result<()> {
        self.interactions
            .add_use_ids(kind.ordinal(), used, with, f.clone())
            .map_err(interaction_error)
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
    fn add_dialogue(&mut self, id: i32, plugin: &DialoguePlugin) -> Result<()> {
        self.dialogue_plugins.push((id, *plugin));
        Ok(())
    }
}
