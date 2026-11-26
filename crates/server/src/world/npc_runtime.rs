//! NPCs made and changed after the start-up: the three `NPC` constructors,
//! `NPC.transform(id)`, `transformWithHpCarryover(id)`, `reTransform()`,
//! `NPC.clear()`, and `Entity.init()`/`Entity.clear()` (which a player
//! reaches too).
//!
//! `NPC.init()` itself is `world::npc_init::init` (npc-init-oracle); its
//! `super.init()` is [`entity_init`] here, and the object it works on is
//! made by [`construct`].
//!
//! The world is the [`Host`]. Field writes are host accessors without a
//! result; every call the original makes outside the object (the definition
//! table, `InteractPlugin`, `NPCBehavior`, the virtual `initConfig` and
//! `configure`, `EntityFlags.getOrdinal`, `UpdateMasks`, `Skills`,
//! `TimerRegistry.addAutoTimers`, the region chunk and tolerance removals,
//! `PulseManager.clear`, the renderable list and `Repository.getNpcs()`) is
//! a required service that may fail. No guard spans a host call.
//! Differentially tested by npc-runtime-oracle (docs/npc-runtime.md).
use super::doors::Location;
use rs09_game_core::direction::Direction;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// A `NullPointerException` of the original.
    Null,
    /// A failure of a host service.
    Binding(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

/// `instanceof Player` / `instanceof NPC` of an entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Player,
    Npc,
    Other,
}

pub trait Host {
    type Entity: Clone;
    /// An `NPCDefinition` object.
    type Definition: Clone;

    fn kind(&self, entity: &Self::Entity) -> Kind;
    /// The `location` field.
    fn location(&self, entity: &Self::Entity) -> Option<Location>;
    /// The `active` field.
    fn set_active(&mut self, entity: &Self::Entity, active: bool);
    /// The `name` field.
    fn set_name(&mut self, npc: &Self::Entity, name: Option<String>);
    /// The `size` field.
    fn set_size(&mut self, npc: &Self::Entity, size: i32);
    /// The `direction` field.
    fn set_direction(&mut self, npc: &Self::Entity, direction: Option<Direction>);
    /// The `id` field.
    fn id(&self, npc: &Self::Entity) -> i32;
    fn set_id(&mut self, npc: &Self::Entity, id: i32);
    /// The final `originalId` field (written once, by the constructor).
    fn original_id(&self, npc: &Self::Entity) -> i32;
    fn set_original_id(&mut self, npc: &Self::Entity, id: i32);
    /// The `definition` field.
    fn definition(&self, npc: &Self::Entity) -> Option<Self::Definition>;
    fn set_definition(&mut self, npc: &Self::Entity, definition: Option<Self::Definition>);
    /// `definition.getName()`.
    fn definition_name(&self, definition: &Self::Definition) -> Option<String>;
    /// `definition.size`.
    fn definition_size(&self, definition: &Self::Definition) -> i32;
    /// Whether the public `behavior` field holds an object.
    fn has_behavior(&self, npc: &Self::Entity) -> bool;

    /// `NPCDefinition.forId(id)`.
    fn definition_for_id(&mut self, id: i32) -> Result<Option<Self::Definition>>;
    /// `super(name, location)`: the new entity object.
    fn allocate(
        &mut self,
        name: Option<String>,
        location: Option<Location>,
    ) -> Result<Self::Entity>;
    /// `super.interactPlugin = new InteractPlugin(this)`.
    fn new_interact_plugin(&mut self, npc: &Self::Entity) -> Result<()>;
    /// `this.behavior = NPCBehavior.forId(id)`.
    fn behavior_for_id(&mut self, npc: &Self::Entity, id: i32) -> Result<()>;
    /// The virtual `initConfig()` (the base body is
    /// `world::npc_init::init_config`).
    fn init_config(&mut self, npc: &Self::Entity) -> Result<()>;
    /// The virtual `configure()`.
    fn configure(&mut self, npc: &Self::Entity) -> Result<()>;
    /// `interactPlugin.setDefault()`.
    fn interact_default(&mut self, npc: &Self::Entity) -> Result<()>;
    /// `EntityFlags.getOrdinal(EFlagType.NPC, EntityFlag.TypeSwap)`.
    fn type_swap_ordinal(&mut self) -> Result<i32>;
    /// `getUpdateMasks().unregisterSynced(ordinal)`.
    fn unregister_synced(&mut self, npc: &Self::Entity, ordinal: i32) -> Result<()>;
    /// `getUpdateMasks().register(EntityFlag.TypeSwap, id, sync)`.
    fn register_type_swap(&mut self, npc: &Self::Entity, id: i32, sync: bool) -> Result<()>;
    /// `getSkills().getLifepoints()`.
    fn lifepoints(&mut self, npc: &Self::Entity) -> Result<i32>;
    /// `getSkills().setLifepoints(lp)`.
    fn set_lifepoints(&mut self, npc: &Self::Entity, lifepoints: i32) -> Result<()>;
    /// `TimerRegistry.addAutoTimers(this)`.
    fn add_auto_timers(&mut self, entity: &Self::Entity) -> Result<()>;
    /// `location.getChunk().removePlayer(asPlayer())`.
    fn chunk_remove_player(&mut self, player: &Self::Entity, location: &Location) -> Result<()>;
    /// `location.getRegion().getTolerances().remove(asPlayer().getName())`.
    fn tolerances_remove(&mut self, player: &Self::Entity, location: &Location) -> Result<()>;
    /// `location.getChunk().remove(asNpc())`.
    fn chunk_remove_npc(&mut self, npc: &Self::Entity, location: &Location) -> Result<()>;
    /// `pulseManager.clear()`.
    fn pulse_manager_clear(&mut self, entity: &Self::Entity) -> Result<()>;
    /// `Repository.removeRenderableNPC(this)`.
    fn remove_renderable(&mut self, npc: &Self::Entity) -> Result<()>;
    /// `Repository.getNpcs().remove(this)`.
    fn repository_remove(&mut self, npc: &Self::Entity) -> Result<()>;
    /// `behavior.onRemoval(this)`.
    fn behavior_removed(&mut self, npc: &Self::Entity) -> Result<()>;
}

/// `new NPC(id)`: no location, facing south.
pub fn new_npc<H: Host>(h: &mut H, id: i32) -> Result<H::Entity> {
    new_npc_at(h, id, None)
}

/// `new NPC(id, location)`: facing south.
pub fn new_npc_at<H: Host>(h: &mut H, id: i32, location: Option<Location>) -> Result<H::Entity> {
    construct(h, id, location, Some(Direction::South))
}

/// `new NPC(id, location, direction)`: the definition is looked up twice
/// (for the name the entity is made with, then for the field); an id
/// without a definition fails before the entity exists.
pub fn construct<H: Host>(
    h: &mut H,
    id: i32,
    location: Option<Location>,
    direction: Option<Direction>,
) -> Result<H::Entity> {
    let named = h.definition_for_id(id)?.ok_or(Error::Null)?;
    let name = h.definition_name(&named);
    let npc = h.allocate(name, location)?;
    h.set_id(&npc, id);
    h.set_original_id(&npc, id);
    let definition = h.definition_for_id(id)?;
    h.set_definition(&npc, definition);
    let definition = h.definition(&npc).ok_or(Error::Null)?;
    let size = h.definition_size(&definition);
    h.set_size(&npc, size);
    h.set_direction(&npc, direction);
    h.new_interact_plugin(&npc)?;
    h.behavior_for_id(&npc, id)?;
    Ok(npc)
}

/// `Entity.init()`: active, then the automatic timers.
pub fn entity_init<H: Host>(h: &mut H, entity: &H::Entity) -> Result<()> {
    h.set_active(entity, true);
    h.add_auto_timers(entity)
}

/// `Entity.clear()`: inactive, out of its chunk when it has a location (a
/// player also out of the region's tolerances), then the pulses.
pub fn entity_clear<H: Host>(h: &mut H, entity: &H::Entity) -> Result<()> {
    h.set_active(entity, false);
    if h.location(entity).is_some() {
        if h.kind(entity) == Kind::Player {
            let location = h.location(entity).ok_or(Error::Null)?;
            h.chunk_remove_player(entity, &location)?;
            let location = h.location(entity).ok_or(Error::Null)?;
            h.tolerances_remove(entity, &location)?;
        }
        if h.kind(entity) == Kind::Npc {
            let location = h.location(entity).ok_or(Error::Null)?;
            h.chunk_remove_npc(entity, &location)?;
        }
    }
    h.pulse_manager_clear(entity)
}

/// `NPC.clear()`.
pub fn clear<H: Host>(h: &mut H, npc: &H::Entity) -> Result<()> {
    entity_clear(h, npc)?;
    h.remove_renderable(npc)?;
    h.repository_remove(npc)?;
    if !h.has_behavior(npc) {
        return Err(Error::Null);
    }
    h.behavior_removed(npc)
}

/// `NPC.transform(id)`: the id, definition, name, size, a new
/// InteractPlugin, `initConfig`, `configure`, the plugin's defaults, then
/// the TypeSwap mask: back at the original id the synced TypeSwap is
/// unregistered and the mask is registered unsynced, otherwise synced.
pub fn transform<H: Host>(h: &mut H, npc: &H::Entity, id: i32) -> Result<()> {
    h.set_id(npc, id);
    let definition = h.definition_for_id(id)?;
    h.set_definition(npc, definition);
    let definition = h.definition(npc).ok_or(Error::Null)?;
    let name = h.definition_name(&definition);
    h.set_name(npc, name);
    let definition = h.definition(npc).ok_or(Error::Null)?;
    let size = h.definition_size(&definition);
    h.set_size(npc, size);
    h.new_interact_plugin(npc)?;
    h.init_config(npc)?;
    h.configure(npc)?;
    h.interact_default(npc)?;
    if id == h.original_id(npc) {
        let ordinal = h.type_swap_ordinal()?;
        h.unregister_synced(npc, ordinal)?;
    }
    let sync = id != h.original_id(npc);
    h.register_type_swap(npc, id, sync)
}

/// `NPC.transformWithHpCarryover(id)`.
pub fn transform_with_hp_carryover<H: Host>(h: &mut H, npc: &H::Entity, id: i32) -> Result<()> {
    let lifepoints = h.lifepoints(npc)?;
    transform(h, npc, id)?;
    h.set_lifepoints(npc, lifepoints)
}

/// `NPC.reTransform()`: nothing at the original id.
pub fn re_transform<H: Host>(h: &mut H, npc: &H::Entity) -> Result<()> {
    if h.original_id(npc) == h.id(npc) {
        return Ok(());
    }
    let original = h.original_id(npc);
    transform(h, npc, original)
}
