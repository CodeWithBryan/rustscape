//! The registrations of this slice's classes and the dispatch of what they
//! register.
//!
//! `ClassScanner.loadContentInterfaces` constructs each content interface
//! and runs `defineListeners()` then `defineDestinationOverrides()`
//! (NPCTalkListener; BankerNPC, whose construction is an NPC's);
//! `loadPluginsFrom` constructs each `@Initializable` class and runs
//! `newInstance(null)` (the AbstractNPC classes register in
//! [`registry::Registry`]; NPCDisturbPlugin and HairCutPlugin set their NPC
//! option handlers).
use super::{
    Class, Error, Factory, Host, Result, banker, idle,
    registry::{self, Registry},
    talk,
};
use crate::definitions::option_handler::{self, NativeOptionHandler};
use crate::world::component_objects::NativeObject;
use crate::world::interaction_listeners::{self as interactions, Kind as IntType};

/// An InteractionListeners `on` handler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interaction {
    /// NPCTalkListener's barcrawl "talk-to"/"talk".
    Barcrawl,
    /// NPCTalkListener's catch-all "talk-to"/"talk"/"talk to".
    Talk,
    /// BankerNPC's "bank" (`Companion::attemptBank`).
    Bank,
    /// BankerNPC's "collect" (`Companion::attemptCollect`).
    Collect,
    /// BankerNPC's "talk-to".
    BankerTalk,
}
impl NativeObject for Interaction {}
impl Interaction {
    pub fn class(self) -> &'static str {
        match self {
            Self::Barcrawl => "content.global.handlers.npc.NPCTalkListener$defineListeners$1",
            Self::Talk => "content.global.handlers.npc.NPCTalkListener$defineListeners$2",
            Self::Bank => "content.global.handlers.npc.BankerNPC$defineListeners$1",
            Self::Collect => "content.global.handlers.npc.BankerNPC$defineListeners$2",
            Self::BankerTalk => "content.global.handlers.npc.BankerNPC$defineListeners$3",
        }
    }
}

/// A destination override (`setDest`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Destination {
    /// BankerNPC's `::provideDestinationOverride`.
    Banker,
}
impl NativeObject for Destination {}
impl Destination {
    pub fn class(self) -> &'static str {
        match self {
            Self::Banker => "content.global.handlers.npc.BankerNPC$defineDestinationOverrides$1",
        }
    }
}

/// An NPC OptionHandler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionPlugin {
    Disturb,
    HairCut,
}
impl NativeOptionHandler for OptionPlugin {}
impl OptionPlugin {
    pub fn class(self) -> &'static str {
        match self {
            Self::Disturb => "content.global.handlers.npc.NPCDisturbPlugin",
            Self::HairCut => "content.global.handlers.npc.HairCutPlugin",
        }
    }
    /// `isWalk()`: NPCDisturbPlugin false, OptionHandler's true.
    pub fn is_walk(self) -> bool {
        matches!(self, Self::HairCut)
    }
    pub fn native_handler(self) -> option_handler::Handle {
        option_handler::Handler::new(self.class(), self)
    }
}

/// A content interface of this slice (`loadContentInterfaces`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Listener {
    Talk,
    Banker,
}
impl Listener {
    pub fn name(self) -> &'static str {
        match self {
            Self::Talk => "content.global.handlers.npc.NPCTalkListener",
            Self::Banker => "content.global.handlers.npc.BankerNPC",
        }
    }
    pub fn named(name: &str) -> Option<Self> {
        [Self::Talk, Self::Banker]
            .into_iter()
            .find(|l| l.name() == name)
    }
}

/// An `@Initializable` plugin of this slice (`loadPluginsFrom`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plugin {
    Npc(Class),
    Disturb,
    HairCut,
}
impl Plugin {
    pub fn name(self) -> &'static str {
        match self {
            Self::Npc(c) => c.name(),
            Self::Disturb => OptionPlugin::Disturb.class(),
            Self::HairCut => OptionPlugin::HairCut.class(),
        }
    }
    pub fn named(name: &str) -> Option<Self> {
        Class::named(name).map(Self::Npc).or_else(|| {
            [Self::Disturb, Self::HairCut]
                .into_iter()
                .find(|p| p.name() == name)
        })
    }
}

/// The registries these classes register into, in the original's terms.
pub trait Registrar: Factory<Class = Class> {
    /// An OptionHandler object (`new X()`).
    type HandlerRef: Clone;
    /// An `NPCDefinition`.
    type Definition;
    fn handler_object(&mut self, handler: OptionPlugin) -> Self::HandlerRef;
    /// `InteractionListener.on(ids, IntType.NPC, options..., handler)` (no
    /// ids: the catch-all `on(IntType.NPC, options..., handler)`).
    fn on(&mut self, ids: Option<&[i32]>, options: &[&str], f: Interaction) -> Result<()>;
    /// `setDest(IntType.NPC, ids, options..., handler)`.
    fn set_dest(&mut self, ids: &[i32], options: &[&str], f: Destination) -> Result<()>;
    /// `NPCDefinition.forId(id)`.
    fn npc_definition(&mut self, id: i32) -> Result<Self::Definition>;
    /// `definition.getHandlers().put(key, handler)`.
    fn put_npc_handler(
        &mut self,
        definition: &Self::Definition,
        key: &str,
        handler: &Self::HandlerRef,
    ) -> Result<()>;
    /// `NPCDefinition.setOptionHandler(name, handler)`.
    fn npc_option_handler(&mut self, name: &str, handler: &Self::HandlerRef) -> Result<()>;
}

/// `defineListeners()` then `defineDestinationOverrides()` of a newly
/// constructed content interface.
pub fn define_listeners<R: Registrar + ?Sized>(r: &mut R, listener: Listener) -> Result<()> {
    match listener {
        Listener::Talk => {
            r.on(
                Some(&talk::BARCRAWL_NPCS),
                &["talk-to", "talk"],
                Interaction::Barcrawl,
            )?;
            r.on(None, &["talk-to", "talk", "talk to"], Interaction::Talk)
        }
        Listener::Banker => {
            // BankerNPC(): super(0, null).
            let (id, constructor) = super::Abstract::prototype(&Class::Banker);
            registry::construct_npc(r, Class::Banker, id, None, constructor)?;
            let ids = Class::Banker.ids();
            r.on(Some(ids), &["bank"], Interaction::Bank)?;
            r.on(Some(ids), &["collect"], Interaction::Collect)?;
            r.on(Some(ids), &["talk-to"], Interaction::BankerTalk)?;
            r.set_dest(ids, &["bank", "collect", "talk-to"], Destination::Banker)
        }
    }
}

/// A newly constructed plugin's `newInstance(null)`.
pub fn define_plugin<R: Registrar + ?Sized>(
    r: &mut R,
    registry: &mut Registry<Class, R::Npc>,
    plugin: Plugin,
) -> Result<()> {
    match plugin {
        Plugin::Npc(class) => registry::define(r, registry, class).map(|_| ()),
        Plugin::Disturb => {
            let h = r.handler_object(OptionPlugin::Disturb);
            r.npc_option_handler("disturb", &h)
        }
        Plugin::HairCut => {
            let h = r.handler_object(OptionPlugin::HairCut);
            let definition = r.npc_definition(598)?;
            r.put_npc_handler(&definition, "option:hair-cut", &h)
        }
    }
}

/// Registers this slice's classes in the start-up manifest's order: the
/// content interfaces, then the `@Initializable` classes `accept` admits.
/// Returns the names registered.
pub fn register_scanned<R: Registrar + ?Sized>(
    r: &mut R,
    registry: &mut Registry<Class, R::Npc>,
    accept: impl Fn(Plugin) -> bool,
) -> Result<Vec<&'static str>> {
    let mut done = Vec::new();
    for name in crate::startup::scan::section("scan.contentInterfaces") {
        if let Some(listener) = Listener::named(name) {
            define_listeners(r, listener)?;
            done.push(listener.name());
        }
    }
    for name in crate::startup::scan::section("scan.initializable") {
        if let Some(plugin) = Plugin::named(name).filter(|p| accept(*p)) {
            define_plugin(r, registry, plugin)?;
            done.push(plugin.name());
        }
    }
    Ok(done)
}

// ------------------------------------------------------------------ dispatch

/// Runs an `on` handler with the player and the node.
pub fn run_interaction<H: Host + ?Sized>(
    h: &mut H,
    f: Interaction,
    player: &H::Npc,
    node: Option<&H::Npc>,
) -> Result<bool> {
    match f {
        Interaction::Barcrawl => talk::barcrawl(h, player, node),
        Interaction::Talk => talk::talk(h, player, node),
        Interaction::Bank => banker::attempt_bank(h, player, node),
        Interaction::Collect => banker::attempt_collect(h, player, node),
        Interaction::BankerTalk => banker::talk_to(h, player, node),
    }
}

/// Runs a destination override with the entity and the node.
pub fn run_destination<H: Host + ?Sized>(
    h: &mut H,
    f: Destination,
    node: Option<&H::Npc>,
) -> Result<Option<[i32; 3]>> {
    match f {
        Destination::Banker => banker::destination(h, node),
    }
}

/// Runs an OptionHandler's `handle(player, node, option)`.
pub fn run_handler<H: Host + ?Sized>(
    h: &mut H,
    handler: OptionPlugin,
    player: &H::Npc,
    node: Option<&H::Npc>,
) -> Result<bool> {
    match handler {
        OptionPlugin::Disturb => idle::disturb_option(h, player, node),
        OptionPlugin::HairCut => idle::hair_cut(h, player, node),
    }
}

/// An OptionHandler's `isWalk(player, node)`.
pub fn handler_walks<H: Host + ?Sized>(
    h: &mut H,
    handler: OptionPlugin,
    player: &H::Npc,
    node: Option<&H::Npc>,
) -> Result<bool> {
    match handler {
        OptionPlugin::Disturb => idle::disturb_option_walks(h, player, node),
        OptionPlugin::HairCut => Ok(false),
    }
}

// ------------------------------------------------------------------ the native registries

/// The registrar over the native InteractionListeners registry. NPC option
/// handlers are collected for the world to bind into its NPC definitions
/// (`npc_handlers`) and default NPC options (`npc_options`); NPC
/// construction goes to the world's factory.
pub struct Native<'a, C: interactions::Casing, F: Factory<Class = Class>> {
    pub interactions: &'a interactions::Registry,
    pub casing: &'a C,
    pub factory: &'a mut F,
    pub npc_handlers: Vec<(i32, String, option_handler::Handle)>,
    pub npc_options: Vec<(String, option_handler::Handle)>,
}

fn interaction_error(e: interactions::Error) -> Error {
    Error::Binding(Some(format!("{e:?}")))
}
fn text(s: &str) -> interactions::Text {
    s.encode_utf16().collect()
}

impl<C: interactions::Casing, F: Factory<Class = Class>> Factory for Native<'_, C, F> {
    type Npc = F::Npc;
    type Class = Class;
    fn new_npc(
        &mut self,
        class: Option<Class>,
        id: i32,
        location: Option<[i32; 3]>,
        direction: Option<super::Direction>,
    ) -> Result<F::Npc> {
        self.factory.new_npc(class, id, location, direction)
    }
    fn set_walks(&mut self, npc: &F::Npc, walks: bool) -> Result<()> {
        self.factory.set_walks(npc, walks)
    }
    fn set_aggressive(&mut self, npc: &F::Npc, aggressive: bool) -> Result<()> {
        self.factory.set_aggressive(npc, aggressive)
    }
    fn log(&mut self, origin: &str, level: super::LogLevel, message: &str) -> Result<()> {
        self.factory.log(origin, level, message)
    }
}

impl<C: interactions::Casing, F: Factory<Class = Class>> Registrar for Native<'_, C, F> {
    type HandlerRef = option_handler::Handle;
    type Definition = i32;
    fn handler_object(&mut self, handler: OptionPlugin) -> option_handler::Handle {
        handler.native_handler()
    }
    fn on(&mut self, ids: Option<&[i32]>, options: &[&str], f: Interaction) -> Result<()> {
        let function = interactions::Function::new(interactions::FunctionKind::On, f.class(), f);
        let options: Vec<interactions::Text> = options.iter().map(|o| text(o)).collect();
        let kind = IntType::Npc.ordinal();
        match ids {
            Some(ids) => self
                .interactions
                .add_ids(self.casing, ids, kind, &options, function),
            None => self
                .interactions
                .add_generics(self.casing, &options, kind, function),
        }
        .map_err(interaction_error)
    }
    fn set_dest(&mut self, ids: &[i32], options: &[&str], f: Destination) -> Result<()> {
        let function =
            interactions::Function::new(interactions::FunctionKind::Destination, f.class(), f);
        let options: Vec<interactions::Text> = options.iter().map(|o| text(o)).collect();
        self.interactions
            .add_destinations(
                self.casing,
                IntType::Npc.ordinal(),
                Some(ids),
                &options,
                function,
            )
            .map_err(interaction_error)
    }
    fn npc_definition(&mut self, id: i32) -> Result<i32> {
        Ok(id)
    }
    fn put_npc_handler(
        &mut self,
        definition: &i32,
        key: &str,
        handler: &option_handler::Handle,
    ) -> Result<()> {
        self.npc_handlers
            .push((*definition, key.into(), handler.clone()));
        Ok(())
    }
    fn npc_option_handler(&mut self, name: &str, handler: &option_handler::Handle) -> Result<()> {
        self.npc_options.push((name.into(), handler.clone()));
        Ok(())
    }
}
