//! A player's death in the live world: `world::player_death` (kept items,
//! `Player.commenceDeath`/`finalizeDeath`, the death screen) with the live
//! world as `player_death::KeptHost`/`Host`, reached from the death task
//! (`npc_death::Host::player_commence_death`/`player_finalize_death`).
//!
//! The containers are the canonical player's own inventory and equipment
//! with their listeners (the container context of the player whose death
//! is being finalized, [`super::CombatWorld::death_subject`]); the kept and
//! lost containers are the factory's. Ground items go through the ground
//! manager of the live world. Graves are not ported here: `new Grave()`
//! needs a grave NPC made at runtime, which the live world does not build;
//! a dangerous death that would leave a grave refuses there (named).
use super::super::{
    P,
    host::Live,
    inner::{EquipmentContext, SettingsContext},
};
use crate::{
    definitions::{item::Handle as Definition, property::Property},
    inventory::{
        self,
        container::{Container, Event, Factory},
        container_listener::Handle as ListenerHandle,
        item::Handle as ItemHandle,
    },
    player::{attributes::AttributeValue, ironman::Mode},
    world::{
        doors::Location,
        drop_tables::drop::{Error, Result},
        interfaces,
        player_death::{self as pd, Prayer, Rights},
        player_managers::skull,
        player_walk::NodeHandle,
        prayer::{self, PrayerType},
        wilderness_zone::WildernessZone,
        zones::Restriction,
    },
};

fn binding(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}
fn refused<T>(what: &str) -> Result<T> {
    Err(Error::Binding(format!(
        "not ported in the live world: {what}"
    )))
}

impl Live {
    fn dp(&self, node: &NodeHandle) -> Result<P> {
        if self.inner.is_player_node(node.node) {
            Ok(P(node.clone()))
        } else {
            Err(Error::Type)
        }
    }
    /// The interface manager of `p` with the interface context.
    fn death_ui(&mut self, p: &P) -> interfaces::Manager {
        self.inner.current = Some(p.clone());
        self.inner.canonical(p).player.manager()
    }
    /// Runs a container operation of the player whose death is finalized.
    fn subject_context<T>(
        &mut self,
        f: impl FnOnce(&mut EquipmentContext<'_>) -> inventory::Result<T>,
    ) -> inventory::Result<T> {
        let p = self.inner.combat.death_subject.clone().ok_or_else(|| {
            inventory::Error::Binding("a container listener outside a death".into())
        })?;
        let listeners = self.item_actions.listeners.clone();
        let mut ctx = EquipmentContext {
            inner: &mut self.inner,
            p,
            listeners: Some(listeners),
        };
        f(&mut ctx)
    }
}

/// `Player.commenceDeath(killer)` from the death task's start.
pub fn commence(live: &mut Live, player: &NodeHandle, killer: &NodeHandle) -> Result<()> {
    pd::commence_death(live, player, Some(killer)).map_err(death_error)
}

/// `Player.finalizeDeath(killer)` from the death task's stop.
pub fn finalize(live: &mut Live, player: &NodeHandle, killer: &NodeHandle) -> Result<()> {
    let p = live.dp(player)?;
    let previous = live.inner.combat.death_subject.replace(p);
    let factory = Factory {
        null_item: live.inner.factory.null_item.clone(),
    };
    let definitions = live.inner.items.clone();
    let mut controller = std::mem::take(&mut live.inner.combat.graves);
    let result = pd::finalize_death(
        live,
        &mut controller,
        player,
        Some(killer),
        &factory,
        &definitions,
    );
    live.inner.combat.graves = controller;
    live.inner.combat.death_subject = previous;
    for node in live.inner.frames.keys().copied().collect::<Vec<_>>() {
        if let Some(p) = live.player_of(node) {
            live.inner.drain(&p);
        }
    }
    result.map_err(death_error)
}

/// `DeathInterface`'s button 1 path is the global handlers'; the death
/// screen's open is [`pd::Host::open_component`].
fn death_error(error: pd::Error) -> Error {
    match error {
        pd::Error::Death(e) => e,
        other => Error::Binding(format!("{other:?}")),
    }
}

impl inventory::container::Context for Live {
    fn update(
        &mut self,
        listener: ListenerHandle,
        container: &mut Container,
        event: Event,
        definitions: &crate::definitions::item::Registry,
    ) -> inventory::Result<()> {
        self.subject_context(|ctx| {
            inventory::container::Context::update(ctx, listener, container, event, definitions)
        })
    }
    fn refresh(
        &mut self,
        listener: ListenerHandle,
        container: &mut Container,
        definitions: &crate::definitions::item::Registry,
    ) -> inventory::Result<()> {
        self.subject_context(|ctx| {
            inventory::container::Context::refresh(ctx, listener, container, definitions)
        })
    }
    fn ground(
        &mut self,
        _: &mut Container,
        _: Option<ItemHandle>,
        _: &crate::definitions::item::Registry,
    ) -> inventory::Result<()> {
        Err(inventory::Error::Binding(
            "not ported in the live world: a container's ground drop during a death".into(),
        ))
    }
}

impl pd::KeptHost for Live {
    fn inventory(&mut self, player: &NodeHandle) -> Result<Container> {
        let p = self.dp(player)?;
        Ok(self.inner.canonical(&p).player.inventory())
    }
    fn equipment(&mut self, player: &NodeHandle) -> Result<Container> {
        let p = self.dp(player)?;
        Ok(self.inner.canonical(&p).player.equipment())
    }
    fn skulled(&mut self, player: &NodeHandle) -> Result<bool> {
        skull::answers::is_skulled(self, player).map_err(binding)
    }
    fn prayer_active(&mut self, player: &NodeHandle, which: Prayer) -> Result<bool> {
        let p = self.dp(player)?;
        let state = self.prayer_state(&p).map_err(binding)?;
        Ok(state.get(match which {
            Prayer::ProtectItems => PrayerType::ProtectItems,
            Prayer::Retribution => PrayerType::Retribution,
        }))
    }
    fn ironman_mode(&mut self, player: &NodeHandle) -> Result<Option<Mode>> {
        let p = self.dp(player)?;
        Ok(self.inner.canonical(&p).player.ironman().mode())
    }
}

impl pd::Host for Live {
    type EquipmentPlugin = crate::definitions::plugin::Handle;

    fn is_active(&mut self, node: &NodeHandle) -> Result<bool> {
        Ok(self.inner.node_active(node.node))
    }
    fn is_male(&mut self, player: &NodeHandle) -> Result<bool> {
        let p = self.dp(player)?;
        Ok(self.inner.canonical(&p).player.appearance().is_male())
    }
    fn get_name(&mut self, player: &NodeHandle) -> Result<Option<String>> {
        let p = self.dp(player)?;
        Ok(Some(self.inner.slot(&p).name.clone()))
    }
    fn name_field(&mut self, player: &NodeHandle) -> Result<Option<String>> {
        let p = self.dp(player)?;
        Ok(Some(self.inner.slot(&p).name.clone()))
    }
    fn rights(&mut self, player: &NodeHandle) -> Result<Rights> {
        let p = self.dp(player)?;
        let rights = self
            .inner
            .with_profile(&p, |profile| profile.details.rights())
            .map_err(binding)?
            .map_err(binding)?;
        Ok(match rights {
            crate::player::details::Rights::RegularPlayer => Rights::RegularPlayer,
            crate::player::details::Rights::PlayerModerator => Rights::PlayerModerator,
            crate::player::details::Rights::Administrator => Rights::Administrator,
        })
    }
    fn increment_death_count(&mut self) -> Result<()> {
        self.inner
            .not_ported
            .skip("GlobalStats.incrementDeathCount (statistics database not ported)");
        Ok(())
    }
    fn set_special_energy(&mut self, player: &NodeHandle, value: i32) -> Result<()> {
        let p = self.dp(player)?;
        let settings = self.inner.canonical(&p).player.settings();
        let mut ctx = SettingsContext {
            inner: &mut self.inner,
            p: p.clone(),
        };
        settings
            .set_special_energy(&mut ctx, value)
            .map_err(binding)?;
        self.inner.drain(&p);
        Ok(())
    }
    fn run_energy(&mut self, player: &NodeHandle) -> Result<f64> {
        let p = self.dp(player)?;
        Ok(self
            .inner
            .canonical(&p)
            .player
            .settings()
            .fields
            .read()
            .run_energy)
    }
    fn update_run_energy(&mut self, player: &NodeHandle, value: f64) -> Result<()> {
        let p = self.dp(player)?;
        let settings = self.inner.canonical(&p).player.settings();
        let mut ctx = SettingsContext {
            inner: &mut self.inner,
            p: p.clone(),
        };
        settings
            .update_run_energy(&mut ctx, value)
            .map_err(binding)?;
        self.inner.drain(&p);
        Ok(())
    }
    fn dispatch_message(&mut self, player: &NodeHandle, message: &str) -> Result<()> {
        let p = self.dp(player)?;
        self.inner.message(&p, message).map_err(binding)
    }
    fn temp_music(&mut self, player: &NodeHandle, id: i32) -> Result<()> {
        let p = self.dp(player)?;
        self.inner
            .queue_native(&p, super::super::tabs::music_frame(id, true));
        Ok(())
    }
    fn api_message(&mut self, player: &NodeHandle, message: &str) -> Result<()> {
        let p = self.dp(player)?;
        self.inner.message(&p, message).map_err(binding)
    }
    fn zone_player_death(
        &mut self,
        player: &NodeHandle,
        killer: Option<&NodeHandle>,
    ) -> Result<bool> {
        let monitor = self.monitor(player);
        monitor.handle_death(self, killer).map_err(binding)
    }
    fn safe_zone(&mut self, player: &NodeHandle) -> Result<bool> {
        let p = self.dp(player)?;
        self.inner
            .with_profile(&p, |profile| profile.properties.safe_zone)
            .map_err(binding)
    }
    fn zone_type(&mut self, player: &NodeHandle) -> Result<i32> {
        let monitor = self.monitor(player);
        monitor.get_type().map_err(binding)
    }
    fn graves_restricted(&mut self, player: &NodeHandle) -> Result<bool> {
        let monitor = self.monitor(player);
        monitor
            .is_restricted(Restriction::Graves.flag())
            .map_err(binding)
    }
    fn skull_wilderness(&mut self, player: &NodeHandle) -> Result<bool> {
        skull::answers::is_wilderness(self, player).map_err(binding)
    }
    fn in_wilderness_zone(&mut self, player: &NodeHandle) -> Result<bool> {
        // WildernessZone.isInZone(player) over INSTANCE's borders: the
        // location read, then one test per border.
        let location = self
            .inner
            .state
            .node_location(player.node)
            .map_err(binding)?;
        for borders in WildernessZone::instance_borders() {
            if borders
                .inside_location(location.as_ref())
                .map_err(binding)?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn set_skulled(&mut self, player: &NodeHandle, value: bool) -> Result<()> {
        skull::answers::set_skulled(self, player, value).map_err(binding)
    }
    fn start_retribution(
        &mut self,
        player: &NodeHandle,
        killer: Option<&NodeHandle>,
    ) -> Result<()> {
        let p = self.dp(player)?;
        let killer = killer.map(|k| P(k.clone()));
        super::prayer::with_prayer(self, &p, |h, p| {
            prayer::start_retribution(h, p, killer.as_ref())
        })
        .map_err(binding)
    }
    fn prayer_reset(&mut self, player: &NodeHandle) -> Result<()> {
        let p = self.dp(player)?;
        super::prayer::with_prayer(self, &p, |h, p| prayer::reset(h, p)).map_err(binding)
    }
    fn attribute_location(
        &mut self,
        entity: &NodeHandle,
        key: &str,
    ) -> Result<Option<Option<Location>>> {
        let p = self.dp(entity)?;
        match self.inner.canonical(&p).player.attributes().get(key) {
            None => Ok(None),
            Some(AttributeValue::Location(l)) => {
                let [x, y, z] = l.coordinates();
                Ok(Some(Some(Location::new(x, y, z))))
            }
            Some(_) => Err(Error::Type),
        }
    }
    fn attributes_contain(&mut self, entity: &NodeHandle, key: &str) -> Result<bool> {
        Ok(self.inner.has_attribute(entity.node, key))
    }
    fn location_field(&mut self, node: &NodeHandle) -> Result<Option<Location>> {
        self.inner.state.node_location(node.node).map_err(binding)
    }
    fn set_location_field(&mut self, node: &NodeHandle, location: Option<Location>) -> Result<()> {
        // Kotlin's `this.location = ...` in Grave (world/live/runtime_npcs.rs).
        if !self.inner.npcs.rows.contains_key(&node.node) {
            return refused("the location field of a node that is not an NPC");
        }
        self.npc_set_location_field(node.node, location);
        Ok(())
    }
    fn repository_news(&mut self, _: &str) -> Result<()> {
        self.inner
            .not_ported
            .skip("Repository.sendNews (permadeath news)");
        Ok(())
    }
    fn total_level(&mut self, player: &NodeHandle) -> Result<i32> {
        let p = self.dp(player)?;
        self.inner
            .with_profile(&p, |profile| profile.skills.total_level())
            .map_err(binding)
    }
    fn permadeath(&mut self, _: &NodeHandle) -> Result<()> {
        refused("Permadeath.permadeath (the hardcore ironman reset)")
    }
    fn run_equip_listeners(
        &mut self,
        id: i32,
        _: &NodeHandle,
        _: &ItemHandle,
        equip: bool,
    ) -> Result<()> {
        // InteractionListeners.run(id, player, item, isEquip): with no equip
        // listener for the id, removeWeakScripts, the event, then true.
        let registry = self.item_actions.listeners.clone();
        if registry.get_equip(id, equip).is_some() {
            return refused("a content unequip listener during a death");
        }
        self.inner.not_ported.skip(
            "removeWeakScripts and the equip InteractionEvent in InteractionListeners.run (no content listener)",
        );
        Ok(())
    }
    fn equipment_plugin(
        &mut self,
        definition: &Definition,
    ) -> Result<Option<crate::definitions::plugin::Handle>> {
        match definition.read().handlers.get("equipment") {
            None | Some(Property::Null) => Ok(None),
            Some(Property::Plugin(plugin)) => Ok(Some(plugin.clone())),
            Some(_) => Err(Error::Type),
        }
    }
    fn fire_unequip(
        &mut self,
        _: crate::definitions::plugin::Handle,
        _: &NodeHandle,
        _: &ItemHandle,
    ) -> Result<()> {
        refused("equipment plugins' fireEvent(\"unequip\") (content)")
    }
    fn item_name(&mut self, id: i32) -> Result<Option<String>> {
        let definition = pd::Host::item_definition(self, id)?;
        let name = definition.read().name.clone();
        Ok(name)
    }
    fn item_definition(&mut self, id: i32) -> Result<Definition> {
        Ok(self.inner.items.for_id(id))
    }
    fn stat_level(&mut self, entity: &NodeHandle, skill: i32) -> Result<i32> {
        let p = self.dp(entity)?;
        self.inner
            .with_profile(&p, |profile| {
                profile
                    .skills
                    .static_levels
                    .get(skill as usize)
                    .copied()
                    .ok_or(Error::Index)
            })
            .map_err(binding)?
    }
    fn plugin_death_item(&mut self, _: (), _: &ItemHandle) -> Result<Option<ItemHandle>> {
        refused("ItemPlugin.getDeathItem (no item plugin is registered)")
    }
    fn monitor_pk(&mut self, _: &NodeHandle, details: &str) -> Result<()> {
        self.inner.log_line(format!("PlayerMonitor PK {details}"));
        self.inner
            .not_ported
            .skip("PlayerMonitor.log PK (logged only)");
        Ok(())
    }
    fn slayer_flags(&mut self, _: &NodeHandle) -> Result<()> {
        self.inner.not_ported.skip(
            "SlayerEquipmentFlags.updateFlags after a death (SlayerManager not ported: the flags are not stored)",
        );
        Ok(())
    }
    fn dismiss_familiar(&mut self, _: &NodeHandle) -> Result<()> {
        self.inner
            .not_ported
            .default_answer("FamiliarManager.dismiss (no familiar: nothing)");
        Ok(())
    }
    fn open_default_tabs(&mut self, player: &NodeHandle) -> Result<()> {
        let p = self.dp(player)?;
        let manager = self.death_ui(&p);
        let result = interfaces::open_default_tabs(&mut self.inner, &manager).map_err(binding);
        self.inner.drain(&p);
        result
    }
    fn component_visibility(
        &mut self,
        player: &NodeHandle,
        iface: i32,
        child: i32,
        hide: bool,
    ) -> Result<()> {
        // PacketDispatch.sendInterfaceConfig: InterfaceConfig with the
        // manager's packet counter.
        let p = self.dp(player)?;
        let counter = self.inner.canonical(&p).player.manager().packet_count(1);
        let frame = rs09_protocol::game::updates::interface_hidden(
            hide,
            counter,
            iface.wrapping_shl(16) | child,
        );
        self.inner.queue_native(&p, frame);
        Ok(())
    }
    fn appearance_sync(&mut self, player: &NodeHandle) -> Result<()> {
        let p = self.dp(player)?;
        self.inner
            .appearance_sync(&p)
            .map(|_| ())
            .map_err(Error::Binding)
    }
    fn death_screen_disabled(&mut self, player: &NodeHandle) -> Result<bool> {
        let p = self.dp(player)?;
        self.inner
            .with_profile(&p, |profile| profile.saved.global.disable_death_screen)
            .map_err(binding)
    }
    fn set_death_screen_disabled(&mut self, player: &NodeHandle, value: bool) -> Result<()> {
        let p = self.dp(player)?;
        self.inner
            .with_profile(&p, |profile| {
                profile.saved.global.disable_death_screen = value
            })
            .map_err(binding)
    }
    fn open_component(&mut self, player: &NodeHandle, id: i32) -> Result<()> {
        let p = self.dp(player)?;
        let manager = self.death_ui(&p);
        let result = interfaces::open_id(&mut self.inner, &manager, id)
            .map(|_| ())
            .map_err(binding);
        self.inner.drain(&p);
        result
    }
    fn close_interface(&mut self, player: &NodeHandle) -> Result<()> {
        let p = self.dp(player)?;
        let manager = self.death_ui(&p);
        let result = interfaces::close(&mut self.inner, &manager)
            .map(|_| ())
            .map_err(binding);
        self.inner.drain(&p);
        result
    }

    // Graves (world/live/graves.rs): the grave is an NPC made at runtime
    // (world/live/runtime_npcs.rs), ticked by GraveController and its own
    // Grave.tick.
    fn new_grave(&mut self) -> Result<pd::grave::Handle> {
        self.new_grave_npc()
    }
    fn set_respawn(&mut self, npc: &NodeHandle, value: bool) -> Result<()> {
        // NPC.setRespawn: read back by the death task's isRespawn().
        self.inner
            .set_attribute(npc.node, "npc:no-respawn", super::Attr::Bool(!value));
        Ok(())
    }
    fn set_walks(&mut self, npc: &NodeHandle, value: bool) -> Result<()> {
        let row = self.inner.npcs.rows.get(&npc.node).ok_or(Error::Null)?;
        row.brain.fields.write().walks = value;
        Ok(())
    }
    fn set_never_walks(&mut self, npc: &NodeHandle, value: bool) -> Result<()> {
        let row = self.inner.npcs.rows.get(&npc.node).ok_or(Error::Null)?;
        row.brain.fields.write().never_walks = value;
        Ok(())
    }
    fn npc_init(&mut self, npc: &NodeHandle) -> Result<()> {
        Live::npc_init(self, npc.node).map_err(binding)
    }
    fn uid_map(&mut self, uid: i32) -> Result<Option<NodeHandle>> {
        Ok(self
            .inner
            .players
            .values()
            .find(|s| s.uid == uid)
            .map(|s| s.canonical.owner.clone()))
    }
    fn register_hint_icon(&mut self, player: &NodeHandle, node: &NodeHandle) -> Result<()> {
        let p = self.dp(player)?;
        Live::register_hint_icon(self, &p, node).map_err(binding)
    }
    fn clear_hint_icon(&mut self, player: &NodeHandle) -> Result<()> {
        let p = self.dp(player)?;
        Live::clear_hint_icon(self, &p).map_err(binding)
    }
    fn set_varbit(&mut self, player: &NodeHandle, id: i32, value: i32) -> Result<()> {
        // ContentAPI.setVarbit(player, id, value) (save false).
        let p = self.dp(player)?;
        crate::world::login::Host::set_varbit(self, &p, id, value, false).map_err(binding)
    }
    fn open_interface(&mut self, player: &NodeHandle, id: i32) -> Result<()> {
        // ContentAPI.openInterface: interfaceManager.open(new Component(id)).
        pd::Host::open_component(self, player, id)
    }
    fn interface_text(
        &mut self,
        player: &NodeHandle,
        text: &str,
        iface: i32,
        child: i32,
    ) -> Result<()> {
        let p = self.dp(player)?;
        self.inner.text(&p, text, iface, child).map_err(binding)
    }
    fn play_audio(&mut self, player: &NodeHandle, id: i32) -> Result<()> {
        // ContentAPI.playAudio(player, id): delay 0, one loop, no location.
        let p = self.dp(player)?;
        self.inner
            .queue_native(&p, rs09_protocol::game::updates::synth_sound(id, 1, 0));
        Ok(())
    }
    fn animate(&mut self, entity: &NodeHandle, id: i32) -> Result<()> {
        // ContentAPI.animate(entity, id): animator.animate(new Animation(id)).
        let p = P(entity.clone());
        self.entity_visualize(&p, Some(crate::world::animation::Animation::new(id)), None)
            .map_err(binding)
    }
    fn prayer_points(&mut self, player: &NodeHandle) -> Result<f64> {
        let p = self.dp(player)?;
        self.inner
            .with_profile(&p, |profile| profile.skills.prayer_points)
            .map_err(binding)
    }
    fn set_prayer_points(&mut self, player: &NodeHandle, value: f64) -> Result<()> {
        // Kotlin's `skills.prayerPoints = value`: setPrayerPoints (the
        // prayer skill packet).
        let p = self.dp(player)?;
        self.with_skills(&p, |skills, ctx| skills.set_prayer(value, ctx))
            .map_err(binding)?
            .map_err(binding)
    }
    fn store_archive(&mut self, _: &str) -> Result<()> {
        refused(
            "ServerStore.getArchive for graves (the server store's grave archive is not read or written by the live world)",
        )
    }
}
