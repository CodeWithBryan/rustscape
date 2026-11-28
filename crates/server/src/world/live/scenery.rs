//! Scenery options in the live world: `processSceneryAction`
//! (`world::scenery_action` through `world::scenery_runtime`, the legacy
//! `InteractPlugin.handle` walk and arrival) over the cache scene's objects,
//! scenery's InteractPlugin tables (`world::scenery_options`, with the global
//! handlers' option defaults and per-definition handlers) and the global
//! handlers' OptionHandlers (LadderManagingPlugin, LookAt, Search), which run
//! with the whole world right after the walking call that reached them
//! ([`super::global::HandlerCall`]).
use super::{host::Live, walk::WalkHost};
use crate::world::{
    doors::{Handle, Location},
    global_handlers::registry::OptionPlugin,
    legacy_interaction as legacy,
    player_walk::{Node, State},
    scenery_action::{self as action, Error, Result},
    scenery_node, scenery_options, scenery_runtime,
};
use rs09_game_core::java_lower::lowercase_root;

fn binding(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}

/// A player context for `Scenery.getChild(player)`: varbit- and
/// varp-configured scenery (morphs) is refused by name.
pub(super) struct NoMorphs;
impl crate::definitions::scenery::PlayerContext for NoMorphs {
    fn varbit(&mut self, _: i32, _: usize) -> crate::definitions::scenery::Result<Option<i32>> {
        Err(crate::definitions::scenery::Error::Binding(
            "not ported in the live world: varbit-configured scenery (morphs)".into(),
        ))
    }
    fn varp(&mut self, _: i32, _: usize) -> crate::definitions::scenery::Result<i32> {
        Err(crate::definitions::scenery::Error::Binding(
            "not ported in the live world: varp-configured scenery (morphs)".into(),
        ))
    }
}

impl Live {
    /// `processSceneryAction`.
    pub fn scenery_action(
        &mut self,
        node: Node,
        action: &rs09_protocol::game::ClientAction,
    ) -> Result<()> {
        self.with_walk_host(|state, host| {
            let request = action::Request::from_action(node, action).ok_or(Error::Null)?;
            host.walk.scenery_dispatch = true;
            let result = scenery_runtime::process(state, host, request);
            host.walk.scenery_dispatch = false;
            result
        })
    }
}

/// A scenery definition's OptionHandler this world runs: a global handler's
/// or a content plugin's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneryPlugin {
    Global(OptionPlugin),
    /// CowMilkingPlugin (super::items::milking).
    CowMilking,
    /// FurnaceOptionPlugin (super::items::smelting).
    Furnace,
    /// SmithingPlugin's anonymous OptionHandler (super::items::smithing).
    Anvil,
}
impl SceneryPlugin {
    pub fn class(self) -> &'static str {
        match self {
            Self::Global(plugin) => plugin.class(),
            Self::CowMilking => super::items::milking::CLASS,
            Self::Furnace => crate::world::smelting::PLUGIN_CLASS,
            Self::Anvil => crate::world::smithing::OPTION_CLASS,
        }
    }
    /// `isWalk()`: OptionHandler's true for a content plugin.
    pub fn is_walk(self) -> bool {
        match self {
            Self::Global(plugin) => plugin.is_walk(),
            Self::CowMilking | Self::Furnace | Self::Anvil => true,
        }
    }
    /// `isWalk(player, node)`: OptionHandler's false.
    pub fn is_walk_for(self) -> bool {
        match self {
            Self::Global(plugin) => plugin.is_walk_for(),
            Self::CowMilking | Self::Furnace | Self::Anvil => false,
        }
    }
    /// `isDelayed(player)`: OptionHandler's true.
    pub fn is_delayed(self) -> bool {
        match self {
            Self::Global(plugin) => plugin.is_delayed(),
            Self::CowMilking | Self::Furnace | Self::Anvil => true,
        }
    }
}

impl WalkHost<'_> {
    /// The handler an option handler id of the scenery arena holds.
    pub fn scenery_handler(&self, id: usize) -> legacy::Result<SceneryPlugin> {
        let handler = self
            .walk
            .scenery
            .handler(id)
            .map_err(|e| legacy::Error::Binding(Some(format!("{e:?}"))))?;
        let handler = handler.read();
        if let Some(plugin) = handler.value::<OptionPlugin>() {
            return Ok(SceneryPlugin::Global(*plugin));
        }
        if handler
            .value::<super::items::milking::CowPlugin>()
            .is_some()
        {
            return Ok(SceneryPlugin::CowMilking);
        }
        if handler
            .value::<super::items::smelting::FurnacePlugin>()
            .is_some()
        {
            return Ok(SceneryPlugin::Furnace);
        }
        if handler
            .value::<super::items::smithing::AnvilPlugin>()
            .is_some()
        {
            return Ok(SceneryPlugin::Anvil);
        }
        Err(legacy::Error::Binding(Some(
            "not ported in the live world: a scenery option handler that is neither a global handler, DoorManagingPlugin nor a ported content plugin (content)".into(),
        )))
    }
}

impl scenery_runtime::Bindings for WalkHost<'_> {
    fn scenery_object(
        &mut self,
        _: &mut State,
        x: i32,
        y: i32,
        z: i32,
        id: i32,
        shape: i32,
    ) -> Result<Option<Handle>> {
        self.walk
            .scene
            .context(&mut self.walk.maps)
            .object(Location::new(x, y, z), id, shape)
            .map_err(|e| Error::Binding(e.0))
    }
    fn scenery_construct(&mut self, _: &mut State, _: i32, _: Location) -> Result<Handle> {
        Err(Error::Binding(
            "not ported in the live world: the hardcoded constructed scenery of processSceneryAction".into(),
        ))
    }
    fn scenery_child(
        &mut self,
        _: &mut State,
        object: &Handle,
        player: Node,
    ) -> Result<Option<Handle>> {
        scenery_node::child(
            &mut self.walk.scene.definitions,
            &mut NoMorphs,
            object,
            Some(player),
        )
        .map(Some)
        .map_err(binding)
    }
    fn scenery_interaction(
        &mut self,
        state: &mut State,
        object: &Handle,
    ) -> Result<scenery_options::Captured> {
        let registry = self.walk.scenery.clone();
        registry
            .initialize(state, &mut self.walk.scene.definitions, object, |t| {
                Ok(lowercase_root(t))
            })
            .map_err(binding)?;
        registry.capture(state, object).map_err(binding)
    }
    fn scenery_varbit(&mut self, _: &mut State, _: i32) -> Result<Option<i32>> {
        Err(Error::Binding(
            "not ported in the live world: VarbitDefinition.forObjectID (debug of morphs)".into(),
        ))
    }
    fn scenery_listeners(
        &mut self,
        state: &mut State,
        id: i32,
        option: Option<String>,
        player: Node,
        object: &Handle,
    ) -> Result<bool> {
        // InteractionListeners.run(id, SCENERY, option, player, node) of an
        // `on` listener or a defineInteraction script: the run itself (the weak scripts, the lock check,
        // the option, the walk and the arrival's lambda) needs the whole
        // world, so it is deferred to right after this packet's kernel
        // returns (`Live::scenery_listener_run`); its answer, false only
        // for an interaction-locked player, is computed here. The global
        // handlers' bank chest and deposit box lambdas are refused by name
        // where they are invoked (super::items `invoke_on`). With no
        // listener registered the answer is false.
        let registry = self.walk.interactions.clone().ok_or(Error::Null)?;
        let text: Vec<u16> = option.as_deref().unwrap_or("null").encode_utf16().collect();
        let scenery = crate::world::interaction_listeners::Kind::Scenery;
        let casing = super::global::RootCasing;
        let listener = registry
            .get(&casing, id, scenery.ordinal(), &text)
            .is_some()
            || registry
                .get_generic(&casing, &text, scenery.ordinal())
                .is_some();
        // defineInteraction's metadata: an interaction script installed in
        // the player's ScriptProcessor and the walk to the object, made by
        // the same deferred run. Only a handler this world runs
        // (super::scripts: the gathering listeners'); the bank booths'
        // stays refused by name.
        let script = !listener
            && match registry.metadata(&casing, id, scenery, &text) {
                Some(meta) if Live::script_installed(&meta.read().function) => true,
                Some(_) => {
                    return Err(Error::Binding(
                        "not ported in the live world: InteractionListeners' defineInteraction scripts for scenery other than the gathering listeners' (the bank booths' interaction scripts)".into(),
                    ));
                }
                None => false,
            };
        if listener || script {
            let owner = state.retain_node(player).map_err(binding)?;
            let locks = state.locks(&owner).map_err(binding)?;
            let locked = locks
                .is_kind_locked(
                    crate::world::action_locks::Kind::Interaction,
                    state,
                    &mut crate::world::action_locks::Clock,
                )
                .map_err(binding)?;
            self.deferred.push(super::walk::Deferred::SceneryListener {
                player: owner,
                id,
                option: text,
                object: object.clone(),
            });
            return Ok(!locked);
        }
        // A door option the pinned content claims for this object (its own
        // InteractionListeners `on` or per-definition `option:` handler,
        // super::door_claims): not ported, so DoorManagingPlugin must not
        // answer in its place.
        let lower = lowercase_root(option.as_deref().unwrap_or("null"));
        let at = object.read().location.coordinates();
        if let Some(class) = super::door_claims::claim(id, &lower)
            .or_else(|| super::door_claims::claim_at(at, &lower))
        {
            return Err(Error::Binding(format!(
                "not ported in the live world: {class}'s \"{lower}\" on scenery {id} (content that claims the object before DoorManagingPlugin)"
            )));
        }
        // ZoneMonitor.interact: a content map zone whose interact is not
        // ported (super::door_claims::ZONE_BORDERS) is asked before the
        // option handler; for a door option the door handler must not answer
        // in its place, so the click is refused inside such a zone.
        if ["open", "close", "shut", "go-through"].contains(&lower.as_str()) {
            let here = state
                .node_location(player)
                .map_err(binding)?
                .map(|l| l.coordinates());
            if let Some(class) = here
                .and_then(super::door_claims::zone_at)
                .or_else(|| super::door_claims::zone_at(at))
            {
                return Err(Error::Binding(format!(
                    "not ported in the live world: {class}'s interact (ZoneMonitor.interact) for \"{lower}\" on scenery {id}"
                )));
            }
        }
        self.not_ported.default_answer(
            "InteractionListeners for scenery (none registered for the option: false)",
        );
        Ok(false)
    }
    fn scenery_plugin(&mut self, _: &mut State, _: Node, object: &Handle) -> Result<bool> {
        // PluginInteractionManager.handle: a content plugin registered for
        // this object (super::door_claims::PLUGINS) is not ported.
        let id = object.read().id;
        if let Some(class) = super::door_claims::plugin(id) {
            return Err(Error::Binding(format!(
                "not ported in the live world: {class} (PluginInteractionManager, OBJECT) on scenery {id}"
            )));
        }
        self.not_ported.default_answer(
            "PluginInteractionManager for scenery (no plugin registered for the object: false)",
        );
        Ok(false)
    }
}
