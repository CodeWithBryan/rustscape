//! Region zones in the live world: `ZoneBuilder.startup`
//! (`world::wilderness_zone::startup`) configures WildernessZone.INSTANCE,
//! MultiwayCombatZone.INSTANCE, a ModeratorZone, a DarkZone, a KaramjaZone
//! and a BankZone into the scene's regions, where the zone monitors
//! ([`super::regions`]) find them; the player's `SkullManager`
//! (`world::player_managers::skull`) answers the wilderness zone's skull
//! services; overlays go through the interface manager.
//!
//! [`Live`] is the host of the zone libraries (`zone_impls::Host`,
//! `dark_zone::Host`, `wilderness_zone::Host`) and of `skull::Host`. Calls
//! into systems without a port (familiars, combat, NPC aggression, the dark
//! zone's light sources and insects, teleports, interfaces of content) are
//! refusals by name, listed in docs/live-world.md.
use super::{P, host::Live};
use crate::inventory::container_listener::Handle as SkullListener;
use crate::world::player_managers as skull_lib;
use crate::{
    player::details::Rights as DetailRights,
    pulses,
    world::{
        dark_zone::{
            self, Component, GlowScript, Hitsplat, InsectPulse, QueueStrength, TinderboxTask,
        },
        doors::Location,
        entity_regions, interfaces,
        player_managers::{
            Kind as ContainerKind,
            skull::{self, ListenerAttribute, SkullManager, answers},
        },
        player_walk::{Node, NodeHandle},
        wilderness_zone,
        zone_impls::{self, Rights, TeleportType},
        zones::{Error, Result, Zone},
    },
};
use std::collections::{HashMap, HashSet};

/// The live world's region zone state.
#[derive(Default)]
pub struct RegionZones {
    /// `player.getSkullManager()`, by player node.
    pub skulls: HashMap<Node, SkullManager<NodeHandle>>,
    /// WildernessZone.INSTANCE and MultiwayCombatZone.INSTANCE.
    pub wilderness: Option<Zone<Live>>,
    pub multiway: Option<Zone<Live>>,
    /// `Entity.properties.isMultiZone()` of entities outside the profile.
    pub multi: HashSet<Node>,
    /// The configuration's `enhanced_deep_wilderness` and `wild_pvp_enabled`.
    pub enhanced_deep_wilderness: bool,
    pub wild_pvp: bool,
}

fn refused<T>(what: &str) -> Result<T> {
    Err(Error::Binding(format!(
        "not ported in the live world: {what}"
    )))
}
fn zerr(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}

impl Live {
    /// `ZoneBuilder.startup()`.
    pub fn zone_startup(&mut self) -> Result<()> {
        let wilderness = wilderness_zone::wilderness_zone::<Live>();
        let multiway = zone_impls::multiway_combat_zone::<Live>();
        self.zones.wilderness = Some(wilderness.clone());
        self.zones.multiway = Some(multiway.clone());
        wilderness_zone::startup(self, &wilderness, &multiway)
    }
    fn is_player(&self, node: &NodeHandle) -> bool {
        self.inner.players.contains_key(&node.node)
    }
    fn player(&self, node: &NodeHandle) -> Result<P> {
        if self.is_player(node) {
            Ok(P(node.clone()))
        } else {
            Err(Error::ClassCast)
        }
    }
    fn ui(&mut self, node: &NodeHandle) -> Result<interfaces::Manager> {
        let p = self.player(node)?;
        self.inner.current = Some(p.clone());
        Ok(self.inner.canonical(&p).player.manager())
    }
}

// ---------------------------------------------------- SkullManager

impl skull::Host for Live {
    type Player = NodeHandle;
    /// `spawnTimer(identifier, runInterval)`'s answer: the identifier and
    /// interval, instantiated by `registerTimer` (super::timers).
    type Timer = (String, i32);
    fn skull(&mut self, player: &NodeHandle) -> skull_lib::Result<&mut SkullManager<NodeHandle>> {
        Ok(self
            .zones
            .skulls
            .entry(player.node)
            .or_insert_with(|| SkullManager::new(player.clone())))
    }
    fn remove_timer(&mut self, player: &NodeHandle, identifier: &str) -> skull_lib::Result<()> {
        super::timers::remove_timer(self, player, identifier)
            .map_err(|e| skull_lib::Error::Binding(format!("{e:?}")))
    }
    fn spawn_timer(
        &mut self,
        identifier: &str,
        run_interval: i32,
    ) -> skull_lib::Result<Option<(String, i32)>> {
        Ok(super::timers::spawnable(self, identifier)
            .then(|| (identifier.to_owned(), run_interval)))
    }
    fn register_timer(
        &mut self,
        player: &NodeHandle,
        (identifier, run_interval): (String, i32),
    ) -> skull_lib::Result<()> {
        super::timers::register_spawned(self, player, &identifier, run_interval)
            .map_err(|e| skull_lib::Error::Binding(format!("{e:?}")))
    }
    fn appearance_skull_icon(&mut self, player: &NodeHandle, icon: i32) -> skull_lib::Result<()> {
        let p = self
            .player(player)
            .map_err(|e| skull_lib::Error::Binding(format!("{e:?}")))?;
        let canonical = self.inner.canonical(&p).player.clone();
        canonical.appearance().set_skull_icon(icon);
        Ok(())
    }
    fn update_appearance(&mut self, player: &NodeHandle) -> skull_lib::Result<()> {
        let p = self
            .player(player)
            .map_err(|e| skull_lib::Error::Binding(format!("{e:?}")))?;
        self.inner
            .canonical(&p)
            .player
            .masks()
            .sync_appearance(super::now_millis)
            .map(|_| ())
            .map_err(|e| skull_lib::Error::Binding(format!("{e:?}")))
    }
    fn appearance_sync(&mut self, player: &NodeHandle) -> skull_lib::Result<()> {
        let p = self
            .player(player)
            .map_err(|e| skull_lib::Error::Binding(format!("{e:?}")))?;
        self.inner
            .appearance_sync(&p)
            .map(|_| ())
            .map_err(skull_lib::Error::Binding)
    }
    fn lock_teleport(&mut self, player: &NodeHandle, ticks: i32) -> skull_lib::Result<()> {
        let state = &mut self.inner.state;
        state
            .locks(player)
            .map_err(|e| skull_lib::Error::Binding(e.to_string()))?
            .lock_kind(
                crate::world::action_locks::Kind::Teleport,
                ticks,
                state,
                &mut crate::world::action_locks::Clock,
            )
            .map_err(|e| skull_lib::Error::Binding(format!("{e:?}")))
    }
    fn unlock_teleport(&mut self, player: &NodeHandle) -> skull_lib::Result<()> {
        self.inner
            .state
            .locks(player)
            .map_err(|e| skull_lib::Error::Binding(e.to_string()))?
            .unlock_kind(crate::world::action_locks::Kind::Teleport)
            .map_err(|e| skull_lib::Error::Binding(format!("{e:?}")))
    }
    fn enhanced_deep_wilderness(&mut self) -> bool {
        self.zones.enhanced_deep_wilderness
    }
    fn listener_attribute(&mut self, player: &NodeHandle) -> skull_lib::Result<ListenerAttribute> {
        let p = self
            .player(player)
            .map_err(|e| skull_lib::Error::Binding(format!("{e:?}")))?;
        Ok(
            match self
                .inner
                .canonical(&p)
                .player
                .attributes()
                .get("deepwild-value-listener")
            {
                None => ListenerAttribute::Absent,
                Some(_) => ListenerAttribute::Other,
            },
        )
    }
    fn set_listener_attribute(
        &mut self,
        _: &NodeHandle,
        _: &SkullListener,
    ) -> skull_lib::Result<()> {
        Err(skull_lib::Error::Binding(
            "not ported in the live world: an object-valued deepwild-value-listener attribute"
                .into(),
        ))
    }
    fn set_risk_attribute(&mut self, player: &NodeHandle, value: i64) -> skull_lib::Result<()> {
        let p = self
            .player(player)
            .map_err(|e| skull_lib::Error::Binding(format!("{e:?}")))?;
        self.inner.canonical(&p).player.attributes().set_value(
            "deepwild-value-risk",
            Some(crate::player::attributes::AttributeValue::Long(value)),
        );
        Ok(())
    }
    fn remove_attribute(&mut self, player: &NodeHandle, key: &str) -> skull_lib::Result<()> {
        let p = self
            .player(player)
            .map_err(|e| skull_lib::Error::Binding(format!("{e:?}")))?;
        self.inner.canonical(&p).player.attributes().remove(key);
        Ok(())
    }
    fn add_listener(
        &mut self,
        _: &NodeHandle,
        _: ContainerKind,
        _: &SkullListener,
    ) -> skull_lib::Result<()> {
        Err(skull_lib::Error::Binding(
            "not ported in the live world: the deep-wilderness container listener".into(),
        ))
    }
    fn remove_listener(
        &mut self,
        _: &NodeHandle,
        _: ContainerKind,
        _: &SkullListener,
    ) -> skull_lib::Result<()> {
        Err(skull_lib::Error::Binding(
            "not ported in the live world: the deep-wilderness container listener".into(),
        ))
    }
    fn items(
        &mut self,
        player: &NodeHandle,
        kind: ContainerKind,
    ) -> skull_lib::Result<Vec<Option<(i32, i32)>>> {
        let p = self
            .player(player)
            .map_err(|e| skull_lib::Error::Binding(format!("{e:?}")))?;
        let canonical = self.inner.canonical(&p).player.clone();
        let container = match kind {
            ContainerKind::Inventory => canonical.inventory(),
            ContainerKind::Equipment => canonical.equipment(),
            ContainerKind::Bank => canonical.bank().container(),
        };
        let items = container.items();
        let items = items.read();
        Ok(items
            .iter()
            .map(|i| i.as_ref().map(|i| (i.read().id(), i.read().amount())))
            .collect())
    }
    fn alchemy_value(&mut self, _: i32) -> skull_lib::Result<i32> {
        Err(skull_lib::Error::Binding(
            "not ported in the live world: the deep-wilderness risk value".into(),
        ))
    }
}

/// A skull library call from a zone service.
fn skull_call<T>(result: skull_lib::Result<T>) -> Result<T> {
    result.map_err(zerr)
}

// ---------------------------------------------------- the zone libraries' hosts

impl zone_impls::Host for Live {
    fn inventory_amount(&mut self, player: &NodeHandle, id: i32) -> Result<i32> {
        let p = self.player(player)?;
        let items = self.inner.canonical(&p).player.inventory().items();
        let items = items.read();
        Ok(items
            .iter()
            .flatten()
            .filter(|i| i.read().id() == id)
            .fold(0i32, |sum, i| sum.wrapping_add(i.read().amount())))
    }
    fn inventory_remove(&mut self, _: &NodeHandle, _: i32, _: i32) -> Result<bool> {
        refused("inventory removal from a zone (Karamja's content)")
    }
    fn interface_resizable(&mut self, player: &NodeHandle) -> Result<bool> {
        let p = self.player(player)?;
        Ok(self.inner.canonical(&p).client.is_resizable())
    }
    fn window_pane_id(&mut self, _: &NodeHandle) -> Result<i32> {
        refused("the window pane id from a zone")
    }
    fn send_interface(&mut self, _: &NodeHandle, _: i32, _: i32, _: i32, _: bool) -> Result<()> {
        refused("an interface packet from a zone (the multiway overlay)")
    }
    fn send_close_interface(
        &mut self,
        _: &NodeHandle,
        _: i32,
        _: i32,
        _: i32,
        _: bool,
    ) -> Result<()> {
        refused("a close-interface packet from a zone (the multiway overlay)")
    }
    fn interface_config(&mut self, _: &NodeHandle, _: i32, _: i32, _: bool) -> Result<()> {
        refused("an interface config packet from a zone (the multiway overlay)")
    }
    fn set_multi_zone(&mut self, entity: &NodeHandle, multi: bool) -> Result<()> {
        if self.is_player(entity) {
            let p = P(entity.clone());
            return self
                .inner
                .with_profile(&p, |profile| profile.properties.multi_zone = multi)
                .map_err(zerr);
        }
        if multi {
            self.zones.multi.insert(entity.node);
        } else {
            self.zones.multi.remove(&entity.node);
        }
        Ok(())
    }
    fn npc_walkable(&mut self, _: &NodeHandle) -> Result<bool> {
        refused("NPC.isWalks from a zone")
    }
    fn local_npcs(
        &mut self,
        location: Option<&Location>,
        distance: i32,
    ) -> Result<Vec<NodeHandle>> {
        let location = location.cloned().ok_or(Error::Null)?;
        self.local(location, distance, entity_regions::Select::Npcs)
            .map_err(zerr)
    }
    fn local_players(
        &mut self,
        location: Option<&Location>,
        distance: i32,
    ) -> Result<Vec<NodeHandle>> {
        let location = location.cloned().ok_or(Error::Null)?;
        self.local(location, distance, entity_regions::Select::Players)
            .map_err(zerr)
    }
    fn invisible(&mut self, npc: &NodeHandle) -> Result<bool> {
        Ok(self
            .inner
            .npcs
            .ai
            .get(&npc.node)
            .is_some_and(|ai| ai.invisible))
    }
    fn has_attack_option(&mut self, npc: &NodeHandle) -> Result<bool> {
        let row = self.inner.npcs.rows.get(&npc.node).ok_or(Error::Null)?;
        let options = row.definition.read().options.clone();
        Ok(options.is_some_and(|o| {
            o.read()
                .iter()
                .flatten()
                .any(|name| name.eq_ignore_ascii_case("attack"))
        }))
    }
    fn should_prevent_stacking(&mut self, _: &NodeHandle, _: &NodeHandle) -> Result<bool> {
        refused("NPC stacking from a zone")
    }
    fn size(&mut self, entity: &NodeHandle) -> Result<i32> {
        self.inner.state.node_size(entity.node).map_err(zerr)
    }
    fn rights(&mut self, player: &NodeHandle) -> Result<Rights> {
        let p = self.player(player)?;
        let rights = self
            .inner
            .with_profile(&p, |profile| profile.details.rights())
            .map_err(zerr)?
            .map_err(zerr)?;
        Ok(match rights {
            DetailRights::Administrator => Rights::Administrator,
            DetailRights::PlayerModerator => Rights::PlayerModerator,
            _ => Rights::RegularPlayer,
        })
    }
    fn home_location(&mut self) -> Result<Option<Location>> {
        Ok(self.inner.constants.home_location.clone())
    }
    fn teleporter_send(
        &mut self,
        _: &NodeHandle,
        _: Option<&Location>,
        _: TeleportType,
    ) -> Result<bool> {
        refused("Player.getTeleporter().send (teleports are not installed)")
    }
    fn restore_tabs(&mut self, _: &NodeHandle) -> Result<()> {
        refused("InterfaceManager.restoreTabs from a zone")
    }
}

impl dark_zone::Overlays for Live {
    fn open_overlay(&mut self, player: &NodeHandle, component: Component) -> Result<()> {
        let Component::Id(id) = component else {
            return refused("the darkness overlay (DarkZone)");
        };
        let manager = self.ui(player)?;
        let c = self.inner.registry.component(id);
        interfaces::open_overlay(&mut self.inner, &manager, Some(c)).map_err(zerr)
    }
    fn close_overlay(&mut self, player: &NodeHandle) -> Result<()> {
        let manager = self.ui(player)?;
        interfaces::close_overlay(&mut self.inner, &manager).map_err(zerr)
    }
    fn close_component(&mut self, player: &NodeHandle, component: Component) -> Result<bool> {
        let Component::Id(id) = component else {
            return refused("the darkness overlay (DarkZone)");
        };
        let manager = self.ui(player)?;
        let c = self.inner.registry.component(id);
        interfaces::close_component(&mut self.inner, &manager, &c).map_err(zerr)
    }
    fn overlay(&mut self, player: &NodeHandle) -> Result<Option<Component>> {
        let manager = self.ui(player)?;
        let overlay = manager.fields.read().overlay.clone();
        Ok(overlay.map(|c| Component::Id(c.read().id)))
    }
    fn is_item(&mut self, target: &NodeHandle) -> Result<bool> {
        Ok(self.inner.state.ground(target.node).is_ok())
    }
}

impl dark_zone::Host for Live {
    fn inventory_items(&mut self, player: &NodeHandle) -> Result<Vec<Option<i32>>> {
        let p = self.player(player)?;
        let items = self.inner.canonical(&p).player.inventory().items();
        let items = items.read();
        Ok(items
            .iter()
            .map(|i| i.as_ref().map(|i| i.read().id()))
            .collect())
    }
    fn equipment_items(&mut self, player: &NodeHandle) -> Result<Vec<Option<i32>>> {
        let p = self.player(player)?;
        let items = self.inner.canonical(&p).player.equipment().items();
        let items = items.read();
        Ok(items
            .iter()
            .map(|i| i.as_ref().map(|i| i.read().id()))
            .collect())
    }
    fn constant_glow(&mut self, _: &NodeHandle) -> Result<bool> {
        // SkillcapePerks.isActive: skillcape perks are off in this world.
        self.inner
            .not_ported
            .default_answer("skillcape perks (skillcape_perks off: false)");
        Ok(false)
    }
    fn option_name(&mut self, _: &()) -> Result<Option<String>> {
        refused("interaction options in DarkZone.interact")
    }
    fn hook_used_with(&mut self, _: &NodeHandle, _: &Zone<Self>) -> Result<()> {
        // Entity.hook(Event.UsedWith, zone): entity event hooks are not
        // ported; the hook would only run on a use-with event (a light).
        self.inner
            .not_ported
            .skip("Entity.hook(UsedWith) of DarkZone (entity event hooks not ported)");
        Ok(())
    }
    fn unhook(&mut self, _: &NodeHandle, _: &Zone<Self>) -> Result<()> {
        self.inner
            .not_ported
            .skip("Entity.unhook of DarkZone (entity event hooks not ported)");
        Ok(())
    }
    fn dark_extension(&mut self, _: &NodeHandle) -> Result<Option<InsectPulse>> {
        refused("DarkZone's insect pulse extension")
    }
    fn add_dark_extension(&mut self, _: &NodeHandle, _: InsectPulse) -> Result<()> {
        refused("DarkZone's insect pulse extension")
    }
    fn pulser_submit(&mut self, _: &pulses::Handle) -> Result<()> {
        refused("DarkZone's insect pulse")
    }
    fn component_open(&mut self, _: &NodeHandle, _: Component) -> Result<()> {
        refused("the darkness overlay (DarkZone)")
    }
    fn component_close(&mut self, _: &NodeHandle, _: Component) -> Result<bool> {
        refused("the darkness overlay (DarkZone)")
    }
    fn manual_hit(&mut self, _: &NodeHandle, _: &NodeHandle, _: i32, _: Hitsplat) -> Result<()> {
        refused("ImpactHandler.manualHit (combat is not installed)")
    }
    fn queue_script(
        &mut self,
        _: &NodeHandle,
        _: i32,
        _: QueueStrength,
        _: bool,
        _: GlowScript,
    ) -> Result<()> {
        refused("DarkZone's light source scripts")
    }
    fn stop_executing(&mut self, _: &NodeHandle) -> Result<bool> {
        refused("DarkZone's light source scripts")
    }
    fn run_task(&mut self, _: &NodeHandle, _: i32, _: i32, _: TinderboxTask) -> Result<()> {
        refused("DarkZone's tinderbox task")
    }
    fn item_name_by_id(&mut self, id: i32) -> Result<Option<String>> {
        Ok(self.inner.items.for_id(id).read().name.clone())
    }
}

impl wilderness_zone::Host for Live {
    fn skull_wilderness_disabled(&mut self, player: &NodeHandle) -> Result<bool> {
        skull_call(answers::is_wilderness_disabled(self, player))
    }
    fn set_skull_level(&mut self, player: &NodeHandle, level: i32) -> Result<()> {
        skull_call(skull::set_level(self, player, level))
    }
    fn set_skull_wilderness(&mut self, player: &NodeHandle, wilderness: bool) -> Result<()> {
        skull_call(answers::set_wilderness(self, player, wilderness))
    }
    fn skull_deep_wilderness(&mut self, player: &NodeHandle) -> Result<bool> {
        skull_call(answers::is_deep_wilderness(self, player))
    }
    fn deepwild_value_risk(&mut self, player: &NodeHandle) -> Result<i64> {
        let p = self.player(player)?;
        match self
            .inner
            .canonical(&p)
            .player
            .attributes()
            .get("deepwild-value-risk")
        {
            None => Ok(0),
            Some(crate::player::attributes::AttributeValue::Long(v)) => Ok(v),
            Some(_) => Err(Error::ClassCast),
        }
    }
    fn send_string(
        &mut self,
        player: &NodeHandle,
        text: &str,
        interface: i32,
        child: i32,
    ) -> Result<()> {
        let p = self.player(player)?;
        self.inner.text(&p, text, interface, child).map_err(zerr)
    }
    fn wild_pvp_enabled(&mut self) -> Result<bool> {
        Ok(self.zones.wild_pvp)
    }
    fn set_attack_option(&mut self, _: &NodeHandle) -> Result<()> {
        refused("the player Attack option (wild PvP)")
    }
    fn remove_attack_option(&mut self, player: &NodeHandle) -> Result<()> {
        // getInteraction().remove(Option._P_ATTACK): the option is never set
        // with wild PvP off, so the table's slot holds another option and
        // remove does nothing.
        let p = self.player(player)?;
        let table = self.inner.canonical(&p).options.clone();
        let attack = self.inner.options.attack.clone();
        let present = crate::world::interaction_options::get(&table, attack.read().index())
            .map_err(zerr)?
            .is_some_and(|o| o.same(&attack));
        if present {
            return refused("removing the player Attack option");
        }
        Ok(())
    }
    fn has_familiar(&mut self, _: &NodeHandle) -> Result<bool> {
        self.inner
            .not_ported
            .default_answer("FamiliarManager.hasFamiliar (not ported: false)");
        Ok(false)
    }
    fn has_pet(&mut self, _: &NodeHandle) -> Result<bool> {
        refused("FamiliarManager.hasPet")
    }
    fn familiar(&mut self, _: &NodeHandle) -> Result<Option<NodeHandle>> {
        refused("FamiliarManager.getFamiliar")
    }
    fn familiar_transform(&mut self, _: &NodeHandle) -> Result<()> {
        refused("Familiar.transform")
    }
    fn familiar_is_combat(&mut self, _: &NodeHandle) -> Result<bool> {
        refused("Familiar.isCombatFamiliar")
    }
    fn npc_retransform(&mut self, _: &NodeHandle) -> Result<()> {
        refused("NPC.reTransform")
    }
    fn appearance_sync(&mut self, player: &NodeHandle) -> Result<()> {
        let p = self.player(player)?;
        self.inner
            .appearance_sync(&p)
            .map(|_| ())
            .map_err(Error::Binding)
    }
    fn npc_aggressive(&mut self, npc: &NodeHandle) -> Result<bool> {
        // NPC.isAggressive(): the brain's `aggressive` field (NPC.configure).
        let row = self.inner.npcs.rows.get(&npc.node).ok_or(Error::Null)?;
        Ok(row.brain.fields.read().aggressive)
    }
    fn set_wilderness_aggression(&mut self, npc: &NodeHandle) -> Result<()> {
        // setAggressiveHandler(new AggressiveHandler(npc, WILDERNESS)) on the
        // start-up's AI row (target selection itself is not installed).
        use crate::world::npc_behaviour::aggression;
        let handler = aggression::Handler::new(
            &self.inner.npcs,
            npc.node,
            std::sync::Arc::new(aggression::Wilderness),
        );
        let ai = self.inner.npcs.ai.get_mut(&npc.node).ok_or(Error::Null)?;
        ai.handler = Some(handler);
        // The live world's own AI (super::combat::behaviour) takes the
        // WILDERNESS behaviour for this NPC.
        self.inner.combat.wilderness_aggressors.insert(npc.node);
        self.inner.combat.ai.remove(&npc.node);
        Ok(())
    }
    fn npc_name(&mut self, npc: &NodeHandle) -> Result<Option<String>> {
        let row = self.inner.npcs.rows.get(&npc.node).ok_or(Error::Null)?;
        Ok(row.definition.read().name.clone())
    }
    fn npc_combat_level(&mut self, npc: &NodeHandle) -> Result<i32> {
        let row = self.inner.npcs.rows.get(&npc.node).ok_or(Error::Null)?;
        Ok(row.definition.read().combat_level)
    }
    fn current_combat_level(&mut self, player: &NodeHandle) -> Result<i32> {
        let p = self.player(player)?;
        self.inner
            .with_profile(&p, |profile| profile.properties.combat_level)
            .map_err(zerr)
    }
    fn combat_pulse_attack(&mut self, entity: &NodeHandle, target: &NodeHandle) -> Result<()> {
        // getProperties().getCombatPulse().attack(target) (super::combat).
        self.combat_attack(&P(entity.clone()), &P(target.clone()))
            .map_err(|e| Error::Binding(format!("{e:?}")))
    }
    fn item_name(&mut self, id: i32) -> Result<Option<String>> {
        Ok(self.inner.items.for_id(id).read().name.clone())
    }
    fn drop_location(&mut self, _: &NodeHandle) -> Result<Option<Location>> {
        refused("NPC.getDropLocation (NPC death is not installed)")
    }
    fn ground_item_create(
        &mut self,
        _: i32,
        _: i32,
        _: Option<&Location>,
        _: &NodeHandle,
    ) -> Result<()> {
        refused("wilderness loot (NPC death is not installed)")
    }
    fn username(&mut self, player: &NodeHandle) -> Result<Option<String>> {
        Ok(self.inner.players.get(&player.node).map(|s| s.name.clone()))
    }
    fn send_news(&mut self, _: &str) -> Result<()> {
        refused("Repository.sendNews")
    }
}
