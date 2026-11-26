//! Weapon special attacks in the live world: `world::special_attacks` with
//! the live world as `special_attacks::Host`.
//!
//! At boot one handler instance per special class joins the swing handler
//! table ([`super::HandlerSlot`] with its [`Special`]) and `register_all`
//! fills the melee and range singletons' `specialHandlers` maps
//! (`CombatSwingHandler.register`). `combat_pulse::Host::overrides` answers
//! `special_attacks::overrides` for a special instance and
//! `combat_pulse::Host::swing_handler` answers `player_swing_handler` for a
//! player; the combat tab's special bar reaches `getSpecial`, `fireEvent`
//! and `handleInstantSpec` through the global handlers' services.
//!
//! The settings calls of a special (`drainSpecial`, `toggleSpecialBar`,
//! `setSpecialEnergy` and their varps and messages) are recorded by
//! [`SettingsCalls`] during the call and made when the combat call returns
//! ([`flush_settings`], from `sync_combat`): their packets keep their order
//! among themselves but follow the swing's other packets of that call.
use super::super::{P, host::Live};
use super::{HandlerSlot, Submitted};
use crate::{
    player::settings::{self, Settings},
    world::{
        animation::{Animation, Priority as AnimPriority},
        combat_pulse::{self as cp, Specials},
        combat_swing::{CombatStyle, Error, Result},
        doors::Location,
        entity_regions::Select,
        prayer::PrayerType,
        special_attacks::{
            self as sa, Priority, Rights, Special, SpecialProjectile, SpecialPulse, SpecialTimer,
            TimerArg,
        },
    },
};
use rs09_protocol::game::{Frame, updates};

fn bind(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}
fn refuse<T>(what: &str) -> Result<T> {
    Err(Error::Binding(format!(
        "not ported in the live world: {what}"
    )))
}

/// One settings call a special made.
enum Call {
    Varp(i32, i32),
    Direct(usize, Frame),
    RunEnergy(f64),
    Message(String),
    Text(String, i32, i32),
}

/// `settings::Context` for the special handlers: the calls are recorded for
/// the player [`sa::Host::settings`] was asked for and made afterwards.
#[derive(Default)]
pub struct SettingsCalls {
    player: Option<P>,
    session: Option<usize>,
    calls: Vec<(P, Call)>,
}

impl SettingsCalls {
    fn push(&mut self, call: Call) -> settings::Result<()> {
        let p = self.player.clone().ok_or(settings::Error::Null)?;
        self.calls.push((p, call));
        Ok(())
    }
}

impl settings::Context for SettingsCalls {
    fn varp(&mut self, _: &Settings, id: i32, value: i32) -> settings::Result<()> {
        self.push(Call::Varp(id, value))
    }
    fn session(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        Ok(self.session)
    }
    fn send(&mut self, _: &Settings, session: Option<usize>, frame: Frame) -> settings::Result<()> {
        let session = session.ok_or(settings::Error::Null)?;
        self.push(Call::Direct(session, frame))
    }
    fn dispatcher(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        Ok(Some(0))
    }
    fn run_energy(&mut self, state: &Settings, _: Option<usize>) -> settings::Result<()> {
        let energy = state.fields.read().run_energy;
        self.push(Call::RunEnergy(energy))
    }
    fn message(&mut self, _: &Settings, _: Option<usize>, text: &str) -> settings::Result<()> {
        self.push(Call::Message(text.to_owned()))
    }
    fn string(
        &mut self,
        _: &Settings,
        _: Option<usize>,
        text: String,
        interface: i32,
        child: i32,
    ) -> settings::Result<()> {
        self.push(Call::Text(text, interface, child))
    }
    fn properties(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        Ok(Some(0))
    }
    // The members below are not reached by a special's settings calls
    // (drainSpecial, toggleSpecialBar, setSpecialToggled, setSpecialEnergy).
    fn retaliating(&mut self, _: &Settings, _: Option<usize>) -> settings::Result<bool> {
        Err(settings::Error::Binding)
    }
    fn set_retaliating(&mut self, _: &Settings, _: Option<usize>, _: bool) -> settings::Result<()> {
        Err(settings::Error::Binding)
    }
    fn ironman(&mut self, _: &Settings) -> settings::Result<bool> {
        Err(settings::Error::Binding)
    }
    fn container(&mut self, _: &Settings, _: settings::ContainerKind) -> settings::Result<usize> {
        Err(settings::Error::Binding)
    }
    fn item(&mut self, _: &Settings, _: usize, _: i32) -> settings::Result<Option<usize>> {
        Err(settings::Error::Binding)
    }
    fn weight(&mut self, _: &Settings, _: usize) -> settings::Result<f64> {
        Err(settings::Error::Binding)
    }
}

/// Makes the settings calls the special handlers recorded.
pub fn flush_settings(live: &mut Live) {
    let calls = std::mem::take(&mut live.inner.combat.special_settings.calls);
    for (p, call) in calls {
        if !live.inner.players.contains_key(&p.node()) {
            continue;
        }
        let result: std::result::Result<(), String> = match call {
            Call::Varp(id, value) => live
                .inner
                .variables(&p, |ctx, owner| {
                    crate::player::variables::set_varp(ctx, Some(owner), id, value, false)
                })
                .map_err(|e| format!("{e:?}")),
            Call::Direct(session, frame) => {
                live.inner
                    .direct_native(session as super::super::SessionId, frame);
                Ok(())
            }
            Call::RunEnergy(energy) => {
                live.inner.queue_native(&p, updates::run_energy(energy));
                Ok(())
            }
            Call::Message(text) => live.inner.message(&p, &text).map_err(|e| format!("{e:?}")),
            Call::Text(text, interface, child) => live
                .inner
                .text(&p, &text, interface, child)
                .map_err(|e| format!("{e:?}")),
        };
        if let Err(error) = result {
            live.inner
                .log_line(format!("a special attack's settings call failed: {error}"));
        }
    }
}

impl Live {
    /// The special handler instances and `register_all` (the class scan's
    /// `newInstance` of the 34 classes) at boot.
    pub fn install_specials(&mut self) -> std::result::Result<(), String> {
        let (melee, range) = (super::MELEE, super::RANGE);
        sa::register_all(self, &melee, &range, |live, special| {
            live.inner.combat.handlers.push(HandlerSlot {
                handler: special.instance(),
                specials: Specials::default(),
                class: special.class_name(),
                special: Some(special),
            });
            live.inner.combat.handlers.len() - 1
        })
        .map_err(|e| format!("{e:?}"))
    }

    /// `CombatStyle.MELEE.swingHandler.getSpecial(itemId)`.
    pub fn melee_special(&mut self, item: i32) -> Option<usize> {
        cp::handler::special(self, &super::MELEE, item)
    }

    fn to_animation(id: i32, priority: Priority) -> Animation {
        let priority = match priority {
            Priority::Low => AnimPriority::Low,
            Priority::Mid => AnimPriority::Mid,
            Priority::High => AnimPriority::High,
            Priority::VeryHigh => AnimPriority::VeryHigh,
        };
        Animation::with(id, 0, Some(priority))
    }

    fn prayer_named(name: &str) -> Result<PrayerType> {
        PrayerType::ALL
            .into_iter()
            .find(|t| t.name() == name)
            .ok_or(Error::Null)
    }
}

fn location_of(at: [i32; 3]) -> Location {
    Location::new(at[0], at[1], at[2])
}

impl sa::Host for Live {
    type SettingsContext = SettingsCalls;
    /// What `TimerRegistry.getTimerInstance(identifier, args)` names: the
    /// instance is made by `registerTimer`, which has the entity.
    type Timer = (String, Vec<TimerArg>);

    fn special_of(&self, handler: &usize) -> Option<Special> {
        self.inner
            .combat
            .handlers
            .get(*handler)
            .and_then(|s| s.special)
    }
    fn settings(&mut self, player: &P) -> (Settings, &mut SettingsCalls) {
        let settings = self.inner.canonical(player).player.settings();
        let session = self.inner.slot(player).session.map(|s| s as usize);
        let calls = &mut self.inner.combat.special_settings;
        calls.player = Some(player.clone());
        calls.session = session;
        (settings, calls)
    }
    fn osrs_dragon_axe(&self) -> bool {
        self.inner.combat.osrs_dragon_axe
    }
    fn new_animation(&self, id: i32, priority: Priority) -> Animation {
        Self::to_animation(id, priority)
    }
    fn is_multi_zone(&mut self, entity: &P) -> Result<bool> {
        crate::world::npc_behaviour::Host::is_multi_zone(self, entity)
    }
    fn local_entities(&mut self, at: [i32; 3], distance: i32) -> Result<Vec<P>> {
        let nodes = self
            .local(location_of(at), distance, Select::Npcs)
            .map_err(bind)?;
        let mut all: Vec<P> = nodes.into_iter().map(P).collect();
        let players = self
            .local(location_of(at), distance, Select::Players)
            .map_err(bind)?;
        all.extend(players.into_iter().map(P));
        Ok(all)
    }
    fn local_npcs(&mut self, at: [i32; 3], distance: i32) -> Result<Vec<P>> {
        let nodes = self
            .local(location_of(at), distance, Select::Npcs)
            .map_err(bind)?;
        Ok(nodes.into_iter().map(P).collect())
    }
    fn local_players(&mut self, at: [i32; 3], distance: i32) -> Result<Vec<P>> {
        crate::world::npc_behaviour::Host::local_players(self, location_of(at), distance)
    }
    fn wilderness_continue_attack(
        &mut self,
        attacker: &P,
        victim: &P,
        style: CombatStyle,
        message: bool,
    ) -> Result<bool> {
        let zone = self.zones.wilderness.clone().ok_or(Error::Null)?;
        let previous = self.inner.combat.zone_style.replace(style);
        let result = zone.continue_attack(self, &attacker.0, Some(&victim.0), Some(&()), message);
        self.inner.combat.zone_style = previous;
        result.map_err(bind)
    }
    fn find_npc(&mut self, _: [i32; 3]) -> Result<Option<P>> {
        refuse("Repository.findNPC(location) (Chainhit's door supports)")
    }
    fn original_id(&mut self, npc: &P) -> Result<i32> {
        self.npc_row(npc).map(|r| r.id).ok_or(Error::Null)
    }
    fn set_prayer_points(&mut self, entity: &P, value: f64) -> Result<()> {
        if self.inner.is_npc_node(entity.node()) {
            return refuse("prayer points of an NPC");
        }
        self.with_skills(entity, |skills, ctx| skills.set_prayer(value, ctx))?
            .map_err(bind)
    }
    fn increment_prayer_points(&mut self, entity: &P, amount: f64) -> Result<()> {
        if self.inner.is_npc_node(entity.node()) {
            return refuse("prayer points of an NPC");
        }
        self.with_skills(entity, |skills, ctx| skills.increment_prayer(amount, ctx))?
            .map_err(bind)
    }
    fn api_message(&mut self, player: &P, text: &str) -> Result<()> {
        self.inner.message(player, text).map_err(bind)
    }
    fn send_chat(&mut self, entity: &P, text: &str) -> Result<()> {
        if self.inner.is_npc_node(entity.node()) {
            return self
                .npc_chat(entity.node(), text.to_owned())
                .map(|_| ())
                .map_err(bind);
        }
        refuse("Player.sendChat (force chat on a player)")
    }
    fn play_audio(&mut self, player: &P, id: i32, delay: i32) -> Result<()> {
        self.inner
            .queue_native(player, updates::synth_sound(id, 1, delay));
        Ok(())
    }
    fn play_global_audio_delayed(&mut self, at: [i32; 3], id: i32, delay: i32) -> Result<()> {
        self.global_audio_delayed(location_of(at), id, delay)
    }
    fn spawn_timer(
        &mut self,
        identifier: &str,
        args: &[TimerArg],
    ) -> Result<Option<(String, Vec<TimerArg>)>> {
        Ok(super::super::timers::spawnable(self, identifier)
            .then(|| (identifier.to_owned(), args.to_vec())))
    }
    fn register_timer(
        &mut self,
        entity: &P,
        (identifier, args): (String, Vec<TimerArg>),
    ) -> Result<()> {
        // A player's or an NPC's timer manager (super::super::timers).
        let args: Vec<crate::timers::content::Arg<P>> = args
            .into_iter()
            .map(|a| match a {
                TimerArg::Int(v) => crate::timers::content::Arg::Int(v),
                TimerArg::Bool(v) => crate::timers::content::Arg::Bool(v),
            })
            .collect();
        self.with_timers(entity, |natives, manager| {
            let registry = natives.registry.clone();
            let timer = registry.instance(&identifier, &args, manager, &mut (), natives)?;
            manager.register_optional(timer.as_ref(), &mut (), natives)
        })
        .map_err(bind)
    }
    fn register_special_timer(&mut self, _: &P, _: SpecialTimer<Self>) -> Result<()> {
        refuse(
            "the anonymous RSTimer of Hamstring and Phantom strike (the live timer manager holds timers::content timers only)",
        )
    }
    fn rights(&mut self, player: &P) -> Result<Option<Rights>> {
        let rights = self
            .inner
            .with_profile(player, |profile| profile.details.rights())
            .map_err(bind)?
            .map_err(bind)?;
        Ok(Some(match rights {
            crate::player::details::Rights::RegularPlayer => Rights::RegularPlayer,
            crate::player::details::Rights::PlayerModerator => Rights::PlayerModerator,
            crate::player::details::Rights::Administrator => Rights::Administrator,
        }))
    }
    fn prayer_on(&mut self, player: &P, prayer: &'static str) -> Result<bool> {
        let t = Self::prayer_named(prayer)?;
        Ok(self.prayer_state(player)?.get(t))
    }
    fn prayer_toggle(&mut self, player: &P, prayer: &'static str) -> Result<bool> {
        let t = Self::prayer_named(prayer)?;
        Live::prayer_toggle(self, player, t).map_err(bind)
    }
    fn continue_attack(
        &mut self,
        player: &P,
        target: &P,
        style: CombatStyle,
        message: bool,
    ) -> Result<bool> {
        crate::world::npc_behaviour::combat::player_continue_attack(
            self,
            player,
            target,
            Some(style),
            message,
        )
    }
    fn combat_stop(&mut self, entity: &P) -> Result<i32> {
        match self.inner.attribute(entity.node(), "combat-stop") {
            None => Ok(-1),
            Some(super::Attr::Int(v)) => Ok(v),
            Some(_) => Err(Error::Binding("ClassCastException: combat-stop".into())),
        }
    }
    fn lock_movement(&mut self, entity: &P, ticks: i32) -> Result<()> {
        let state = &mut self.inner.state;
        state
            .locks(&entity.0)
            .map_err(bind)?
            .lock_kind(
                crate::world::action_locks::Kind::Movement,
                ticks,
                state,
                &mut crate::world::action_locks::Clock,
            )
            .map_err(bind)
    }
    fn set_stun_clock(&mut self, _: &P, _: i32) -> Result<()> {
        refuse("Clocks.STUN (the entity clocks are not joined to the walking queue)")
    }
    fn dumb_path(&mut self, _: &P, _: [i32; 3]) -> Result<bool> {
        refuse("Pathfinder.find with Pathfinder.DUMB (Shove)")
    }
    fn add_path(&mut self, _: &P, _: i32, _: i32) -> Result<()> {
        refuse("walkingQueue.addPath (Shove)")
    }
    fn definition_name(&mut self, item_id: i32) -> Result<String> {
        let name = self.inner.items.for_id(item_id).read().name.clone();
        Ok(name.unwrap_or_else(|| "null".into()))
    }
    fn chain_victims(&mut self, entity: &P) -> Result<Option<Vec<P>>> {
        Ok(self.inner.combat.chain_victims.get(&entity.node()).cloned())
    }
    fn set_chain_victims(&mut self, entity: &P, list: Vec<P>) -> Result<()> {
        self.inner.combat.chain_victims.insert(entity.node(), list);
        Ok(())
    }
    fn chain_victims_mut(&mut self, entity: &P) -> Option<&mut Vec<P>> {
        self.inner.combat.chain_victims.get_mut(&entity.node())
    }
    fn submit_special(&mut self, pulse: SpecialPulse<Self>) -> Result<()> {
        let checks = pulse
            .checks
            .iter()
            .map(|c| c.as_ref().map(P::node))
            .collect();
        let delay = pulse.delay;
        let label = format!(
            "content.global.handlers.item.equipment.special ({})",
            pulse.task.label()
        );
        self.submit_combat(delay, checks, &label, Submitted::Special(pulse))
            .map_err(Error::Binding)
    }
    fn send_special_projectile(&mut self, projectile: SpecialProjectile<'_, Self>) -> Result<()> {
        let projectile = match projectile {
            SpecialProjectile::Create {
                source,
                victim,
                id,
                start_height,
                end_height,
                start_delay,
                speed,
                angle,
                distance,
            } => {
                let mut p = self.projectile_create(
                    source,
                    victim,
                    id,
                    start_height,
                    end_height,
                    start_delay,
                    speed,
                    angle,
                );
                p.distance = distance;
                p
            }
            SpecialProjectile::CreateShort {
                source,
                victim,
                id,
                start_height,
                end_height,
                start_delay,
                speed,
            } => {
                // The seven-argument create: angle 5, distance
                // source.size() << 5 (the source is never null here).
                self.projectile_create(
                    source,
                    victim,
                    id,
                    start_height,
                    end_height,
                    start_delay,
                    speed,
                    5,
                )
            }
            SpecialProjectile::Copy {
                template,
                source,
                victim,
                speed,
            } => {
                let template = crate::startup::loaders::ranged::ProjectileTemplate {
                    projectile_id: template[0],
                    start_height: template[1],
                    end_height: template[2],
                    start_delay: template[3],
                    speed: template[4],
                    angle: template[5],
                    distance: template[6],
                };
                let victim = victim.ok_or(Error::Null)?;
                self.projectile_copy(&template, source, victim, speed)?
            }
        };
        self.projectile_send(projectile)
    }
}
