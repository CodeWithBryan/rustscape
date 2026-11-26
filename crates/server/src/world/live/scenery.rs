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
struct NoMorphs;
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

impl WalkHost<'_> {
    /// The global handler an option handler id of the scenery arena holds.
    pub fn scenery_handler(&self, id: usize) -> legacy::Result<OptionPlugin> {
        let handler = self
            .walk
            .scenery
            .handler(id)
            .map_err(|e| legacy::Error::Binding(Some(format!("{e:?}"))))?;
        let plugin = handler.read().value::<OptionPlugin>().copied();
        plugin.ok_or_else(|| {
            legacy::Error::Binding(Some(
                "not ported in the live world: a scenery option handler that is not a global handler (doors and other content)".into(),
            ))
        })
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
        _: &mut State,
        id: i32,
        option: Option<String>,
        _: Node,
        _: &Handle,
    ) -> Result<bool> {
        // InteractionListeners.run(id, SCENERY, option, player, node): the
        // registered functions are the global handlers' bank booth, chest
        // and deposit box listeners, whose bodies need the bank (not
        // installed here): refused. With none registered the answer is false.
        let registry = self.walk.interactions.clone().ok_or(Error::Null)?;
        let text: Vec<u16> = option.as_deref().unwrap_or("null").encode_utf16().collect();
        let kind = crate::world::interaction_listeners::Kind::Scenery.ordinal();
        let casing = super::global::RootCasing;
        if registry.get(&casing, id, kind, &text).is_some()
            || registry.get_generic(&casing, &text, kind).is_some()
        {
            return Err(Error::Binding(
                "not ported in the live world: InteractionListeners on listeners for scenery (the bank chest and deposit box listeners: the walk to scenery for a content listener)".into(),
            ));
        }
        // defineInteraction's metadata (the bank booths): an interaction
        // script in the player's script queue, which is content script
        // invocation (not ported here).
        if registry
            .metadata(
                &casing,
                id,
                crate::world::interaction_listeners::Kind::Scenery,
                &text,
            )
            .is_some()
        {
            return Err(Error::Binding(
                "not ported in the live world: InteractionListeners' defineInteraction scripts for scenery (the bank booths' interaction scripts)".into(),
            ));
        }
        self.not_ported.default_answer(
            "InteractionListeners for scenery (none registered for the option: false)",
        );
        Ok(false)
    }
    fn scenery_plugin(&mut self, _: &mut State, _: Node, _: &Handle) -> Result<bool> {
        self.not_ported.default_answer(
            "PluginInteractionManager for scenery (content plugins not ported: false)",
        );
        Ok(false)
    }
}
