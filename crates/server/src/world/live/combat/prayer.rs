//! Prayer in the live world: `world::prayer` (`Prayer`, `PrayerType`, the
//! prayer tab's listener, the drain in `Player.tick`, the head icon) with
//! each player's `PrayerState` kept in [`super::CombatWorld`].
//!
//! `prayer::Host::skills` hands out the player's Skills and a context: the
//! Skills are taken out of the profile for the call, and the context records
//! the skill effects (the prayer level packet) and replays them through the
//! live skills context right after the call returns.
use super::super::{
    host::Live,
    inner::{SkillsContext, placeholder_skills},
};
use super::{super::P, Submitted, host::bind};
use crate::{
    player::skills::{self, Effect, EntityKind, Skills},
    world::{
        combat_pulse as cp,
        combat_pulse::HitsplatType,
        combat_swing::{self as cs, CombatStyle, Error, Result},
        prayer::{self, Perk, PrayerEvent, PrayerState, PrayerType, ProjectileRequest},
    },
};

/// The skills context of a prayer call: answers what the prayer bodies
/// ask and records the effects for the live context.
pub struct Recorded {
    ticks: i32,
    combat_level: i32,
    summoning: i32,
    effects: Vec<Effect>,
    skipped: Vec<&'static str>,
}
impl skills::Context for Recorded {
    fn kind(&self) -> EntityKind {
        EntityKind::Player
    }
    fn version(&self) -> i32 {
        6
    }
    fn tick(&mut self) -> i32 {
        self.ticks
    }
    fn world_tick(&mut self) -> i32 {
        self.ticks
    }
    fn combat_level(&self) -> i32 {
        self.combat_level
    }
    fn summoning_combat_level(&self) -> i32 {
        self.summoning
    }
    fn npc_combat_level(&mut self) -> skills::Result<i32> {
        Err(skills::Error::Binding)
    }
    fn damage_spong_active(&mut self) -> skills::Result<bool> {
        self.skipped
            .push("skillcape perks (skillcape_perks off: false)");
        Ok(false)
    }
    fn assisted_level(&mut self, _: i32) -> skills::Result<Option<i32>> {
        self.skipped
            .push("AssistSession (no assist sessions: none)");
        Ok(None)
    }
    fn translate_experience(
        &mut self,
        _: &mut Skills,
        _: i32,
        _: f64,
        _: f64,
    ) -> skills::Result<bool> {
        Err(skills::Error::Binding)
    }
    fn brawling_present(&mut self) -> skills::Result<bool> {
        Err(skills::Error::Binding)
    }
    fn brawling_for_skill(&mut self, _: i32) -> skills::Result<Option<i32>> {
        Err(skills::Error::Binding)
    }
    fn equipment_contains(&mut self, _: i32) -> skills::Result<bool> {
        Err(skills::Error::Binding)
    }
    fn brawling_bonus(&mut self) -> skills::Result<f64> {
        Err(skills::Error::Binding)
    }
    fn tutorial_complete(&mut self) -> skills::Result<bool> {
        Err(skills::Error::Binding)
    }
    fn artificial(&mut self) -> skills::Result<bool> {
        Ok(false)
    }
    fn username(&mut self) -> skills::Result<String> {
        Err(skills::Error::Binding)
    }
    fn permadeath_present(&self) -> bool {
        false
    }
    fn effect(&mut self, _: &mut Skills, effect: Effect) -> skills::Result<()> {
        self.effects.push(effect);
        Ok(())
    }
}

/// `prayer::Host` for one player's call.
pub struct PrayerHost<'a> {
    pub live: &'a mut Live,
    pub player: P,
    pub state: PrayerState,
    pub skills: Skills,
    pub context: Recorded,
}

fn perr(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}
fn refuse<T>(what: &str) -> Result<T> {
    Err(Error::Binding(format!(
        "not ported in the live world: {what}"
    )))
}

/// Runs a prayer body for `player` and writes its state and skills back.
pub fn with_prayer<T>(
    live: &mut Live,
    player: &P,
    f: impl FnOnce(&mut PrayerHost<'_>, &P) -> Result<T>,
) -> Result<T> {
    let node = player.node();
    let state = live.inner.combat.prayers.remove(&node).unwrap_or_default();
    let (skills, combat_level, summoning) = live
        .inner
        .with_profile(player, |p| {
            (
                std::mem::replace(&mut p.skills, placeholder_skills()),
                p.properties.combat_level,
                p.summoning_combat_level,
            )
        })
        .map_err(perr)?;
    let ticks = live.inner.ticks;
    let mut host = PrayerHost {
        live,
        player: player.clone(),
        state,
        skills,
        context: Recorded {
            ticks,
            combat_level,
            summoning,
            effects: Vec::new(),
            skipped: Vec::new(),
        },
    };
    let result = f(&mut host, player);
    let PrayerHost {
        live,
        state,
        mut skills,
        context,
        ..
    } = host;
    live.inner.combat.prayers.insert(node, state);
    for what in context.skipped {
        live.inner.not_ported.default_answer(what);
    }
    let mut ctx = SkillsContext {
        inner: &mut live.inner,
        p: player.clone(),
    };
    let mut replay = Ok(());
    for effect in context.effects {
        if let Err(e) = skills::Context::effect(&mut ctx, &mut skills, effect) {
            replay = Err(perr(e));
            break;
        }
    }
    live.inner
        .with_profile(player, |p| p.skills = skills)
        .map_err(perr)?;
    live.sync_combat();
    let value = result?;
    replay?;
    Ok(value)
}

impl Live {
    /// The player's `Prayer` state (a copy), or an empty one for an NPC.
    pub fn prayer_state(&mut self, p: &P) -> cs::Result<PrayerState> {
        Ok(self
            .inner
            .combat
            .prayers
            .get(&p.node())
            .cloned()
            .unwrap_or_default())
    }
    /// The prayer tab's button (interface 271).
    pub fn prayer_button(&mut self, p: &P, button: i32) -> Result<bool> {
        with_prayer(self, p, |h, p| prayer::tab_button(h, p, button))
    }
    /// `Prayer.toggle(type)`.
    pub fn prayer_toggle(&mut self, p: &P, t: PrayerType) -> Result<bool> {
        with_prayer(self, p, |h, p| prayer::toggle(h, p, t))
    }
    /// `prayer.tick()` from `Player.tick`.
    pub fn prayer_tick(&mut self, p: &P) -> Result<()> {
        if self
            .inner
            .combat
            .prayers
            .get(&p.node())
            .is_none_or(|s| s.active.is_empty() && s.prayer_active_ticks == 0)
        {
            return Ok(());
        }
        with_prayer(self, p, |h, p| prayer::tick(h, p))
    }
}

/// `Entity.fullRestore()` from combat (`Player.fullRestore` for a player).
pub fn full_restore(live: &mut Live, entity: &P) -> cs::Result<()> {
    if live.inner.is_npc_node(entity.node()) {
        super::death::restore_npc_skills(live, entity);
        return Ok(());
    }
    with_prayer(live, entity, |h, p| prayer::full_restore(h, p)).map_err(bind)
}

/// `prayer.startRedemption()`.
pub fn start_redemption(live: &mut Live, entity: &P) -> cs::Result<()> {
    with_prayer(live, entity, |h, p| prayer::start_redemption(h, p)).map_err(bind)
}

/// The head-icon pulse `Prayer.reset` submits.
pub fn head_icon_pulse(live: &mut Live, p: &P) -> std::result::Result<bool, String> {
    with_prayer(live, p, |h, p| prayer::head_icon_pulse(h, p)).map_err(|e| format!("{e:?}"))
}

impl prayer::Host for PrayerHost<'_> {
    type Entity = P;
    type SkillsContext = Recorded;
    fn same_entity(&self, a: &P, b: &P) -> bool {
        a.node() == b.node()
    }
    fn is_npc(&self, entity: &P) -> bool {
        self.live.inner.is_npc_node(entity.node())
    }
    fn prayer(&mut self, _: &P) -> &mut PrayerState {
        &mut self.state
    }
    fn skills(&mut self, _: &P) -> (&mut Skills, &mut Recorded) {
        (&mut self.skills, &mut self.context)
    }
    fn set_varp(&mut self, player: &P, id: i32, value: i32, save: bool) -> Result<()> {
        self.live
            .inner
            .variables(player, |ctx, owner| {
                crate::player::variables::set_varp(ctx, Some(owner), id, value, save)
            })
            .map_err(perr)
    }
    fn get_varbit(&mut self, player: &P, id: i32) -> Result<i32> {
        self.live
            .inner
            .variables(player, |ctx, owner| {
                crate::player::variables::get_varbit(ctx, Some(owner), id)
            })
            .map_err(perr)
    }
    fn set_varbit(&mut self, player: &P, id: i32, value: i32, save: bool) -> Result<()> {
        self.live
            .inner
            .variables(player, |ctx, owner| {
                crate::player::variables::set_varbit(ctx, Some(owner), id, value, save)
            })
            .map_err(perr)
    }
    fn play_audio(&mut self, player: &P, id: i32) -> Result<()> {
        self.live
            .inner
            .queue_native(player, rs09_protocol::game::updates::synth_sound(id, 1, 0));
        Ok(())
    }
    fn dispatch(&mut self, _: &P, _: PrayerEvent) -> Result<()> {
        self.live.inner.not_ported.skip(
            "PrayerActivatedEvent/PrayerDeactivatedEvent dispatch (entity event hooks not ported)",
        );
        Ok(())
    }
    fn head_icon(&mut self, player: &P) -> Result<i32> {
        Ok(self
            .live
            .inner
            .canonical(player)
            .player
            .appearance()
            .head_icon())
    }
    fn set_head_icon(&mut self, player: &P, icon: i32) -> Result<()> {
        // appearance.setHeadIcon on the canonical appearance.
        self.live
            .inner
            .canonical(player)
            .player
            .appearance()
            .set_head_icon(icon);
        Ok(())
    }
    fn sync_appearance(&mut self, player: &P) -> Result<()> {
        self.live
            .inner
            .appearance_sync(player)
            .map(|_| ())
            .map_err(perr)
    }
    fn send_dialogue(&mut self, _: &P, _: &str) -> Result<()> {
        refuse("sendDialogue (the dialogue interpreter is not installed)")
    }
    fn has_kings_ransom(&mut self, _: &P) -> Result<bool> {
        self.live
            .inner
            .not_ported
            .default_answer("QuestRequirements.hasRequirement (no quests registered: false)");
        Ok(false)
    }
    fn submit_head_icon_pulse(&mut self, player: &P, delay: i32) -> Result<()> {
        self.live
            .submit_combat(
                delay,
                Vec::new(),
                "core.game.node.entity.player.link.prayer.Prayer$reset$1",
                Submitted::HeadIcon(player.clone()),
            )
            .map_err(perr)
    }
    fn send_message(&mut self, player: &P, text: &str) -> Result<()> {
        self.live.inner.message(player, text).map_err(perr)
    }
    fn bonuses(&mut self, player: &P) -> Result<Option<Vec<i32>>> {
        self.live
            .inner
            .with_profile(player, |p| Some(p.properties.bonuses.clone()))
            .map_err(perr)
    }
    fn perk_active(&mut self, _: &P, _: Perk) -> Result<bool> {
        self.live
            .inner
            .not_ported
            .default_answer("skillcape perks (skillcape_perks off: false)");
        Ok(false)
    }
    fn next_int(&mut self, bound: i32) -> Result<i32> {
        self.live
            .inner
            .generators
            .shared_next_int(bound)
            .ok_or(Error::Binding("nextInt bound".into()))
    }
    fn graphics(&mut self, player: &P, id: i32) -> Result<()> {
        self.live
            .entity_visualize(
                player,
                None,
                Some(crate::world::animation::Graphics::new(id, 0, 0)),
            )
            .map_err(perr)
    }
    fn location(&mut self, entity: &P) -> Result<Option<[i32; 3]>> {
        Ok(self.live.cloc(entity).map(|l| l.coordinates()))
    }
    fn send_projectile(&mut self, r: ProjectileRequest) -> Result<()> {
        // Projectile.create(start, destination, ...).send(): a location-based
        // projectile without a victim (combat/projectiles.rs).
        let at = |l: [i32; 3]| crate::world::doors::Location::new(l[0], l[1], l[2]);
        let projectile = super::projectiles::Projectile {
            source_location: Some(at(r.start)),
            victim: None,
            end_location: Some(at(r.destination)),
            id: r.id,
            start_height: r.start_height,
            end_height: r.end_height,
            start_delay: r.start_delay,
            speed: r.speed,
            angle: r.angle,
            distance: r.distance,
        };
        self.live.projectile_send(projectile).map_err(perr)
    }
    fn manual_hit(&mut self, owner: &P, source: &P, hit: i32, kind: HitsplatType) -> Result<()> {
        cp::impact::manual_hit(self.live, owner, Some(source), hit, Some(kind))
            .map(|_| ())
            .map_err(perr)
    }
    fn is_multi_zone(&mut self, player: &P) -> Result<bool> {
        crate::world::npc_behaviour::Host::is_multi_zone(self.live, player).map_err(perr)
    }
    fn local_npcs(&mut self, location: [i32; 3], distance: i32) -> Result<Vec<P>> {
        let at = crate::world::doors::Location::new(location[0], location[1], location[2]);
        let nodes = self
            .live
            .local(at, distance, crate::world::entity_regions::Select::Npcs)
            .map_err(perr)?;
        Ok(nodes.into_iter().map(P).collect())
    }
    fn local_players(&mut self, location: [i32; 3], distance: i32) -> Result<Vec<P>> {
        let at = crate::world::doors::Location::new(location[0], location[1], location[2]);
        crate::world::npc_behaviour::Host::local_players(self.live, at, distance).map_err(perr)
    }
    fn is_attackable(
        &mut self,
        entity: &P,
        attacker: &P,
        style: CombatStyle,
        message: bool,
    ) -> Result<bool> {
        crate::world::npc_behaviour::combat::is_attackable(
            self.live,
            entity,
            attacker,
            Some(style),
            message,
        )
        .map_err(perr)
    }
    fn is_playing(&mut self, player: &P) -> Result<bool> {
        Ok(self
            .live
            .inner
            .players
            .get(&player.node())
            .is_some_and(|s| s.playing))
    }
    fn entity_commence_death(&mut self, _: &P, _: Option<&P>) -> Result<()> {
        // Entity.commenceDeath(killer): an empty body.
        Ok(())
    }
    fn set_special_energy(&mut self, player: &P, value: i32) -> Result<()> {
        self.live
            .inner
            .canonical(player)
            .player
            .settings()
            .fields
            .write()
            .special_energy = value;
        Ok(())
    }
    fn update_run_energy(&mut self, _: &P, _: f64) -> Result<()> {
        refuse("settings.updateRunEnergy from Player.fullRestore")
    }
    fn entity_full_restore(&mut self, player: &P) -> Result<()> {
        // Entity.fullRestore: skills.restore() on the skills of this call.
        let _ = player;
        self.skills.restore(&mut self.context).map_err(perr)
    }
}
