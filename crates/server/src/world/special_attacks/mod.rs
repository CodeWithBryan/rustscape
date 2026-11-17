//! Weapon special attacks.
//!
//! Ports the 34 `@Initializable` special attack handlers of
//! content/global/handlers/item/equipment/special (`Backstab` .. `Weaken`,
//! `AncientMace` and `SaradominsLightning`), their registration through
//! `CombatSwingHandler.register`, the lookup `Player.getSwingHandler` makes
//! with `getSpecial`, their `fireEvent` answers and the instant special of the
//! combat tab, and core/game/node/entity/combat/MultihitTargets.kt
//! ([`multihit`]).
//!
//! Each handler is an instance of the melee or range swing handler class with
//! its own `type` and flags (a host [`cs::Handler`]) and a subclass identity
//! ([`Special`]). The members a subclass overrides are joined to the native
//! swing and pulse through [`combat_pulse::Overrides`]: a world answers
//! `combat_pulse::Host::overrides` with [`overrides`] for the instance's
//! special, so `CombatPulse`, the impact pulse and the base classes reach the
//! subclass bodies here exactly where the original's virtual calls do. Special
//! energy is the native [`Settings`]. Everything else is a required
//! [`Host`] service with no default. See docs/special-attacks.md.
pub mod melee;
pub mod multihit;
pub mod range;

use crate::player::settings::{self, Settings};
use crate::world::combat_pulse::{
    self as cp, HitsplatType, Overrides, handler as base, impact, pulse as cpulse,
};
use crate::world::combat_swing::{
    self as cs, BattleState, CombatStyle, Error, Graphics, Handler, HandlerKind, Result,
};

/// `Skills` slots the handlers name.
pub mod skill {
    pub const ATTACK: i32 = 0;
    pub const DEFENCE: i32 = 1;
    pub const STRENGTH: i32 = 2;
    pub const RANGE: i32 = 4;
    pub const PRAYER: i32 = 5;
    pub const MAGIC: i32 = 6;
    pub const WOODCUTTING: i32 = 8;
}

/// `Animator.Priority`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Priority {
    Low,
    Mid,
    High,
    VeryHigh,
}

impl Priority {
    pub fn name(self) -> &'static str {
        match self {
            Self::Low => "LOW",
            Self::Mid => "MID",
            Self::High => "HIGH",
            Self::VeryHigh => "VERY_HIGH",
        }
    }
}

/// `Rights`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rights {
    RegularPlayer,
    PlayerModerator,
    Administrator,
}

/// One special attack subclass.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Special {
    AncientMace,
    Backstab,
    Chainhit,
    Cleave,
    Clobber,
    DescentOfDarkness,
    EnergyDrain,
    Excalibur,
    Feint,
    Hamstring,
    HealingBlade,
    IceCleave,
    Impale,
    Judgement,
    Liquefy,
    PhantomStrike,
    Powershot,
    Powerstab,
    Puncture,
    QuickSmash,
    Rampage,
    SaradominsLightning,
    Seercull,
    Sever,
    Shatter,
    Shove,
    SliceAndDice,
    Smash,
    Snapshot,
    Snipe,
    SpearWall,
    Sweep,
    Warstrike,
    Weaken,
}

/// How a `newInstance` registers its items: every call, or a chain of `&&`
/// whose first refusal skips the rest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Registration {
    Each(&'static [i32]),
    Chained(&'static [i32]),
}

/// `fireEvent(identifier, args)`'s answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fired {
    Null,
    /// Kotlin's `Unit`.
    Unit,
    True,
}

impl Special {
    /// The order this module registers the plugins in (alphabetical by class).
    /// The original class scan order is not observable: no item id is
    /// registered by two classes.
    pub const ALL: [Self; 34] = [
        Self::AncientMace,
        Self::Backstab,
        Self::Chainhit,
        Self::Cleave,
        Self::Clobber,
        Self::DescentOfDarkness,
        Self::EnergyDrain,
        Self::Excalibur,
        Self::Feint,
        Self::Hamstring,
        Self::HealingBlade,
        Self::IceCleave,
        Self::Impale,
        Self::Judgement,
        Self::Liquefy,
        Self::PhantomStrike,
        Self::Powershot,
        Self::Powerstab,
        Self::Puncture,
        Self::QuickSmash,
        Self::Rampage,
        Self::SaradominsLightning,
        Self::Seercull,
        Self::Sever,
        Self::Shatter,
        Self::Shove,
        Self::SliceAndDice,
        Self::Smash,
        Self::Snapshot,
        Self::Snipe,
        Self::SpearWall,
        Self::Sweep,
        Self::Warstrike,
        Self::Weaken,
    ];

    /// The runtime class simple name.
    pub fn class_name(self) -> &'static str {
        match self {
            Self::AncientMace => "AncientMaceSpecialHandler",
            Self::Backstab => "BackstabSpecialHandler",
            Self::Chainhit => "ChainhitSpecialHandler",
            Self::Cleave => "CleaveSpecialHandler",
            Self::Clobber => "ClobberSpecialHandler",
            Self::DescentOfDarkness => "DescentOfDarknessSpecialHandler",
            Self::EnergyDrain => "EnergyDrainSpecialHandler",
            Self::Excalibur => "ExcaliburSpecialHandler",
            Self::Feint => "FeintSpecialHandler",
            Self::Hamstring => "HamstringSpecialHandler",
            Self::HealingBlade => "HealingBladeSpecialHandler",
            Self::IceCleave => "IceCleaveSpecialHandler",
            Self::Impale => "ImpaleSpecialHandler",
            Self::Judgement => "JudgementSpecialHandler",
            Self::Liquefy => "LiquefySpecialHandler",
            Self::PhantomStrike => "PhantomStrikeSpecialHandler",
            Self::Powershot => "PowershotSpecialHandler",
            Self::Powerstab => "PowerstabSpecialHandler",
            Self::Puncture => "PunctureSpecialHandler",
            Self::QuickSmash => "QuickSmashSpecialHandler",
            Self::Rampage => "RampageSpecialHandler",
            Self::SaradominsLightning => "SaradominsLightningHandler",
            Self::Seercull => "SeercullSpecialHandler",
            Self::Sever => "SeverSpecialHandler",
            Self::Shatter => "ShatterSpecialHandler",
            Self::Shove => "ShoveSpecialHandler",
            Self::SliceAndDice => "SliceAndDiceSpecialHandler",
            Self::Smash => "SmashSpecialHandler",
            Self::Snapshot => "SnapshotSpecialHandler",
            Self::Snipe => "SnipeSpecialHandler",
            Self::SpearWall => "SpearWallSpecialHandler",
            Self::Sweep => "SweepSpecialHandler",
            Self::Warstrike => "WarstrikeSpecialHandler",
            Self::Weaken => "WeakenSpecialHandler",
        }
    }

    /// The base class the subclass extends.
    pub fn base(self) -> HandlerKind {
        match self {
            Self::Chainhit
            | Self::DescentOfDarkness
            | Self::Hamstring
            | Self::PhantomStrike
            | Self::Powershot
            | Self::Seercull
            | Self::Snapshot
            | Self::Snipe => HandlerKind::Range,
            _ => HandlerKind::Melee,
        }
    }

    /// The flags the constructor passes to its base class.
    pub fn flags(self) -> &'static [cs::Flag] {
        match self {
            Self::Chainhit | Self::Seercull | Self::Snapshot => {
                &[cs::Flag::IgnorePrayerBoostsDamage]
            }
            _ => &[],
        }
    }

    /// A fresh instance: the base class constructor with the subclass flags.
    pub fn instance(self) -> Handler {
        Handler::new(self.base(), self.flags())
    }

    /// `SPECIAL_ENERGY`.
    pub fn energy(self) -> i32 {
        match self {
            Self::Chainhit => 10,
            Self::Cleave
            | Self::Feint
            | Self::Impale
            | Self::Puncture
            | Self::Shatter
            | Self::Shove => 25,
            Self::Sweep => 30,
            Self::Powershot | Self::Smash => 35,
            Self::EnergyDrain
            | Self::Hamstring
            | Self::HealingBlade
            | Self::Judgement
            | Self::PhantomStrike
            | Self::QuickSmash
            | Self::SliceAndDice
            | Self::SpearWall
            | Self::Weaken => 50,
            Self::DescentOfDarkness | Self::Sever | Self::Snapshot => 55,
            Self::IceCleave | Self::Powerstab => 60,
            Self::Backstab | Self::Snipe => 75,
            Self::AncientMace
            | Self::Clobber
            | Self::Excalibur
            | Self::Rampage
            | Self::SaradominsLightning
            | Self::Seercull
            | Self::Warstrike => 100,
            // Liquefy has no energy of its own: it falls back to the melee swing.
            Self::Liquefy => 0,
        }
    }

    /// The items `newInstance` registers, on `CombatStyle.MELEE`'s handler or
    /// `CombatStyle.RANGE`'s ([`Self::base`]).
    pub fn registration(self) -> Registration {
        use Registration::{Chained, Each};
        match self {
            Self::AncientMace => Each(&[11061]),
            Self::Backstab => Each(&[8872, 8874, 8876, 8878]),
            Self::Chainhit => Each(&[805]),
            Self::Cleave => Chained(&[1305, 13475]),
            Self::Clobber => Each(&[6739]),
            Self::DescentOfDarkness => Chained(&[11235, 13405, 14803, 14804, 14805, 14806]),
            Self::EnergyDrain => Each(&[4151]),
            Self::Excalibur => Each(&[35, 14632]),
            Self::Feint => Chained(&[13899, 13901]),
            Self::Hamstring => Each(&[13883]),
            Self::HealingBlade => Each(&[11698, 13452]),
            Self::IceCleave => Each(&[11700, 13453]),
            Self::Impale => Each(&[3101]),
            Self::Judgement => Chained(&[11694, 13450]),
            Self::Liquefy => Each(&[11037]),
            Self::PhantomStrike => Each(&[13879, 13880, 13881, 13882]),
            Self::Powershot => Each(&[859, 10284]),
            Self::Powerstab => Each(&[7158]),
            Self::Puncture => Chained(&[1215, 1231, 5680, 5698, 13465, 13466, 13467, 13468]),
            Self::QuickSmash => Each(&[4153]),
            Self::Rampage => Each(&[1377]),
            Self::SaradominsLightning => Each(&[11730]),
            Self::Seercull => Each(&[6724]),
            Self::Sever => Chained(&[4587, 13477]),
            Self::Shatter => Each(&[1434, 13479]),
            Self::Shove => Each(&[1249, 1263, 3176, 5716, 5730, 11716, 14662]),
            Self::SliceAndDice => Each(&[14484, 14486]),
            Self::Smash => Chained(&[13902, 13904]),
            Self::Snapshot => Each(&[861]),
            Self::Snipe => Each(&[8880]),
            Self::SpearWall => Each(&[13905, 13907]),
            Self::Sweep => Each(&[3204]),
            Self::Warstrike => Each(&[11696, 13451]),
            Self::Weaken => Each(&[6746]),
        }
    }

    /// `fireEvent(identifier, args)`. Clobber reads
    /// `ServerConstants.DRAGON_AXE_USE_OSRS_SPEC` (`osrs_axe`).
    pub fn fire_event(self, identifier: &str, osrs_axe: bool) -> Fired {
        match self {
            Self::Excalibur | Self::Rampage => match identifier {
                "instant_spec" | "ncspec" => Fired::True,
                _ => Fired::Null,
            },
            Self::QuickSmash => match identifier {
                "instant_spec" => Fired::True,
                _ => Fired::Null,
            },
            Self::Clobber => {
                if osrs_axe && identifier == "instant_spec" {
                    Fired::True
                } else {
                    Fired::Unit
                }
            }
            Self::Liquefy => Fired::Unit,
            _ => Fired::Null,
        }
    }
}

/// The members [`Special`] overrides, for `combat_pulse::Host::overrides`.
pub fn overrides<H: Host>(special: Special) -> Overrides<H> {
    let mut o = Overrides::none();
    o.swing = Some(swing_member::<H>);
    match special {
        Special::AncientMace => {
            o.visualize = Some(melee::visualize::<H>);
            o.formatted_hit = Some(melee::ancient_mace_formatted_hit::<H>);
        }
        Special::Chainhit => {
            o.visualize = Some(range::visualize::<H>);
            o.visualize_impact = Some(range::chainhit_visualize_impact::<H>);
            o.impact = Some(nothing::<H>);
        }
        Special::DescentOfDarkness => {
            o.visualize = Some(range::visualize::<H>);
            o.visualize_impact = Some(range::descent_visualize_impact::<H>);
        }
        Special::Excalibur
        | Special::Hamstring
        | Special::Liquefy
        | Special::PhantomStrike
        | Special::Rampage => {}
        Special::IceCleave => {
            o.visualize = Some(melee::visualize::<H>);
            o.adjust_battle_state = Some(melee::ice_cleave_adjust::<H>);
        }
        Special::Powerstab | Special::SpearWall => {
            o.visualize = Some(melee::visualize::<H>);
            o.visualize_impact = Some(melee::targets_visualize_impact::<H>);
            o.impact = Some(melee::targets_impact::<H>);
        }
        Special::Puncture => {
            o.visualize = Some(melee::visualize::<H>);
            o.impact = Some(melee::puncture_impact::<H>);
        }
        Special::SaradominsLightning => {
            o.visualize = Some(melee::visualize::<H>);
            o.impact = Some(melee::lightning_impact::<H>);
        }
        Special::Seercull => {
            o.visualize = Some(range::visualize::<H>);
            o.impact = Some(range::seercull_impact::<H>);
        }
        Special::Shove => {
            o.visualize = Some(melee::visualize::<H>);
            o.visualize_impact = Some(nothing::<H>);
            o.impact = Some(nothing::<H>);
        }
        Special::Snapshot => {
            o.visualize = Some(range::visualize::<H>);
            o.impact = Some(range::snapshot_impact::<H>);
        }
        Special::Sweep => {
            o.visualize = Some(melee::visualize::<H>);
            o.adjust_battle_state = Some(melee::sweep_adjust::<H>);
            o.impact = Some(melee::sweep_impact::<H>);
            o.visualize_impact = Some(melee::targets_visualize_impact::<H>);
        }
        Special::Powershot | Special::Snipe => o.visualize = Some(range::visualize::<H>),
        _ => o.visualize = Some(melee::visualize::<H>),
    }
    o
}

/// An overriding member with an empty body.
fn nothing<H: Host>(
    _: &mut H,
    _: &H::HandlerRef,
    _: Option<&H::Entity>,
    _: Option<&H::Entity>,
    _: &mut BattleState<H>,
) -> Result<()> {
    Ok(())
}

/// The `swing` override of the instance's special, with the pulse's state.
fn swing_member<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<i32> {
    swing(h, r, entity, victim, Some(state))
}

/// `swing(entity, victim, state)` of the instance's special. `state` is
/// `None` for the combat tab's instant special, which passes null.
pub fn swing<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: Option<&mut BattleState<H>>,
) -> Result<i32> {
    let special = h.special_of(r).ok_or(Error::Null)?;
    match special.base() {
        HandlerKind::Range => range::swing(h, r, special, entity, victim, state),
        _ => melee::swing(h, r, special, entity, victim, state),
    }
}

/// The registrations `newInstance` of every class makes. `melee` and `range`
/// are `CombatStyle.MELEE.swingHandler` and `CombatStyle.RANGE.swingHandler`;
/// `instance` is the handler instance the world made for each class (with
/// [`Special::instance`]) in [`Special::ALL`] order.
pub fn register_all<H: Host>(
    h: &mut H,
    melee: &H::HandlerRef,
    range: &H::HandlerRef,
    mut instance: impl FnMut(&mut H, Special) -> H::HandlerRef,
) -> Result<()> {
    for special in Special::ALL {
        let made = instance(h, special);
        new_instance(h, special, melee, range, &made)?;
    }
    Ok(())
}

/// One class's `newInstance(arg)`.
pub fn new_instance<H: Host>(
    h: &mut H,
    special: Special,
    melee: &H::HandlerRef,
    range: &H::HandlerRef,
    made: &H::HandlerRef,
) -> Result<()> {
    let target = match special.base() {
        HandlerKind::Range => range,
        _ => melee,
    };
    match special.registration() {
        Registration::Each(ids) => {
            for &id in ids {
                base::register(h, target, id, made)?;
            }
        }
        Registration::Chained(ids) => {
            for &id in ids {
                if !base::register(h, target, id, made)? {
                    break;
                }
            }
        }
    }
    Ok(())
}

/// What `Player.getSwingHandler(swing)` answers.
pub enum Chosen<R> {
    /// `CombatStyle.X.swingHandler` or a registered special.
    Handler(R),
    /// `ChinchompaSwingHandler.getInstance()` (not ported here).
    Chinchompa,
    /// `SalamanderSwingHandler.INSTANCE` (not ported here).
    Salamander,
}

/// `Player.getSwingHandler(swing)`: the special registered for the weapon on
/// the pulse style's handler when the special bar is on. `handlers` are the
/// three `CombatStyle` singletons in melee, range, magic order.
pub fn player_swing_handler<H: Host>(
    h: &mut H,
    player: &H::Entity,
    swing: bool,
    handlers: [&H::HandlerRef; 3],
) -> Result<Chosen<H::HandlerRef>> {
    let style = h.combat_pulse(player).style;
    let weapon = h.equipment_new(player, cp::slot::WEAPON)?;
    let weapon_id = h.item_id(&weapon);
    let of = |style: CombatStyle| match style {
        CombatStyle::Melee => handlers[0].clone(),
        CombatStyle::Range => handlers[1].clone(),
        CombatStyle::Magic => handlers[2].clone(),
    };
    if swing {
        if h.spell(player)?.is_some() || h.autocast_spell(player)?.is_some() {
            return Ok(Chosen::Handler(handlers[2].clone()));
        }
        let (settings, _) = h.settings(player);
        let toggled = settings.fields.read().special_toggled;
        if toggled {
            if let Some(special) = base::special(h, &of(style), weapon_id) {
                return Ok(Chosen::Handler(special));
            }
            h.send_message(
                player,
                &format!("Unhandled special attack for item {weapon_id}!"),
            )?;
        }
    }
    if (style == CombatStyle::Range && weapon_id == 10033) || weapon_id == 10034 {
        return Ok(Chosen::Chinchompa);
    }
    if (10146..=10149).contains(&weapon_id) {
        return Ok(Chosen::Salamander);
    }
    Ok(Chosen::Handler(of(style)))
}

/// `CombatTabInterface.handleInstantSpec`: `handler.swing(p, victim, null)`.
pub fn instant_swing<H: Host>(h: &mut H, r: &H::HandlerRef, player: &H::Entity) -> Result<i32> {
    let victim = h.combat_pulse(player).victim.clone();
    swing(h, r, Some(player), victim.as_ref(), None)
}

/// An argument of `spawnTimer(identifier, args...)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimerArg {
    Int(i32),
    Bool(bool),
}

/// The anonymous `RSTimer` subclasses Hamstring and Phantom strike register.
pub enum TimerBody<H: Host> {
    /// Hamstring's: a message on register, and `run` answers false.
    Hamstrung,
    /// Phantom strike's: bleeds the initial damage in hits of at most five.
    PhantomStrike {
        attacker: H::Entity,
        remaining_damage: i32,
    },
}

/// One anonymous `RSTimer(runInterval, identifier, isSoft, isAuto, flags)`.
pub struct SpecialTimer<H: Host> {
    pub run_interval: i32,
    pub identifier: &'static str,
    pub is_soft: bool,
    pub is_auto: bool,
    /// `TimerFlag` names.
    pub flags: &'static [&'static str],
    pub body: TimerBody<H>,
}

impl<H: Host> SpecialTimer<H> {
    /// `onRegister(entity)`.
    pub fn on_register(&mut self, h: &mut H, entity: &H::Entity) -> Result<()> {
        if !h.kind(entity).is_player() {
            return Ok(());
        }
        let text = match self.body {
            TimerBody::Hamstrung => {
                "You've been hamstrung! For the next minute, your run energy will drain 4x faster."
            }
            TimerBody::PhantomStrike { .. } => {
                "You start to bleed as the result of the javelin strike."
            }
        };
        h.player_send_message(entity, text)
    }

    /// `run(entity)`: whether the timer runs again.
    pub fn run(&mut self, h: &mut H, entity: &H::Entity) -> Result<bool> {
        match &mut self.body {
            TimerBody::Hamstrung => Ok(false),
            TimerBody::PhantomStrike {
                attacker,
                remaining_damage,
            } => {
                if h.kind(entity).is_player() {
                    h.player_send_message(
                        entity,
                        "You continue to bleed as the result of the javelin strike.",
                    )?;
                }
                let tick_damage = (*remaining_damage).min(5);
                *remaining_damage = remaining_damage.wrapping_sub(tick_damage);
                let attacker = attacker.clone();
                impact::manual_hit(
                    h,
                    entity,
                    Some(&attacker),
                    tick_damage,
                    Some(HitsplatType::Normal),
                )?;
                match &self.body {
                    TimerBody::PhantomStrike {
                        remaining_damage, ..
                    } => Ok(*remaining_damage > 0),
                    TimerBody::Hamstrung => Ok(false),
                }
            }
        }
    }
}

/// What a pulse a special submits does when it fires.
pub enum SpecialTask<H: Host> {
    /// Chainhit's door-support pulse: `n.impactHandler.manualHit(victim, 1, NORMAL)`.
    ChainDoor { npc: H::Entity, victim: H::Entity },
    /// `handleHit`'s first pulse: `super.onImpact(player, victim, state)` and
    /// `super.impact(entity, victim, state)`.
    ChainImpact {
        handler: H::HandlerRef,
        entity: H::Entity,
        victim: H::Entity,
        player: H::Entity,
        state: Box<BattleState<H>>,
    },
    /// The bounce to the next chain-hit victim.
    ChainBounce {
        handler: H::HandlerRef,
        entity: H::Entity,
        victim: H::Entity,
        target: H::Entity,
        player: H::Entity,
    },
    /// Snapshot's second arrow beyond two tiles.
    SnapshotSecond {
        entity: H::Entity,
        victim: H::Entity,
        hit: i32,
    },
}

impl<H: Host> SpecialTask<H> {
    /// A label for the pulse's anonymous class.
    pub fn label(&self) -> &'static str {
        match self {
            Self::ChainDoor { .. } => "chainDoor",
            Self::ChainImpact { .. } => "chainImpact",
            Self::ChainBounce { .. } => "chainBounce",
            Self::SnapshotSecond { .. } => "snapshotSecond",
        }
    }
}

/// A pulse a special hands to `GameWorld.Pulser.submit`, with its `Pulse` fields.
pub struct SpecialPulse<H: Host> {
    pub running: bool,
    pub delay: i32,
    pub ticks_passed: i32,
    pub checks: Vec<Option<H::Entity>>,
    pub task: SpecialTask<H>,
}

impl<H: Host> SpecialPulse<H> {
    /// `Pulse(delay, checks...)`.
    pub fn new(delay: i32, checks: Vec<Option<H::Entity>>, task: SpecialTask<H>) -> Self {
        Self {
            running: true,
            delay,
            ticks_passed: 0,
            checks,
            task,
        }
    }

    /// `Pulse.update()`: true when the runner must drop the pulse. A failure
    /// inside the body is printed and stops the pulse.
    pub fn update(&mut self, h: &mut H) -> Result<bool> {
        for at in 0..self.checks.len() {
            if let Some(node) = self.checks[at].clone() {
                if !h.is_active(&node)? {
                    self.running = false;
                    return Ok(true);
                }
            }
        }
        if !self.running {
            return Ok(true);
        }
        self.ticks_passed = self.ticks_passed.wrapping_add(1);
        if self.ticks_passed >= self.delay {
            self.ticks_passed = 0;
            match self.body(h) {
                Ok(true) => {
                    self.running = false;
                    return Ok(true);
                }
                Ok(false) => {}
                Err(error) => {
                    h.print_stack_trace(&error)?;
                    self.running = false;
                    return Ok(true);
                }
            }
            return Ok(!self.running);
        }
        Ok(false)
    }

    fn body(&mut self, h: &mut H) -> Result<bool> {
        match &mut self.task {
            SpecialTask::ChainDoor { npc, victim } => {
                let (npc, victim) = (npc.clone(), victim.clone());
                impact::manual_hit(h, &npc, Some(&victim), 1, Some(HitsplatType::Normal))?;
                Ok(true)
            }
            SpecialTask::ChainImpact {
                handler,
                entity,
                victim,
                player,
                state,
            } => {
                let (r, entity, victim, player) = (
                    handler.clone(),
                    entity.clone(),
                    victim.clone(),
                    player.clone(),
                );
                base::on_impact(h, Some(&player), Some(&victim), Some(state))?;
                base::impact_super(h, &r, Some(&entity), Some(&victim), state)?;
                Ok(true)
            }
            SpecialTask::ChainBounce {
                handler,
                entity,
                victim,
                target,
                player,
            } => {
                let (r, entity, victim, target, player) = (
                    handler.clone(),
                    entity.clone(),
                    victim.clone(),
                    target.clone(),
                    player.clone(),
                );
                range::chainhit_bounce(h, &r, &entity, &victim, &target, &player)?;
                Ok(true)
            }
            SpecialTask::SnapshotSecond {
                entity,
                victim,
                hit,
            } => {
                let (entity, victim, hit) = (entity.clone(), victim.clone(), *hit);
                impact::handle_impact(
                    h,
                    &victim,
                    Some(&entity),
                    hit,
                    Some(CombatStyle::Range),
                    None,
                    false,
                )?;
                Ok(true)
            }
        }
    }
}

/// A projectile a special sends (`Projectile.create`/`copy` and `send()`).
pub enum SpecialProjectile<'a, H: Host> {
    /// `Projectile.create(source, victim, id, startHeight, endHeight,
    /// startDelay, speed, angle, distance)`.
    Create {
        source: &'a H::Entity,
        victim: Option<&'a H::Entity>,
        id: i32,
        start_height: i32,
        end_height: i32,
        start_delay: i32,
        speed: i32,
        angle: i32,
        distance: i32,
    },
    /// `Projectile.create(source, victim, id, startHeight, endHeight,
    /// startDelay, speed)`.
    CreateShort {
        source: &'a H::Entity,
        victim: Option<&'a H::Entity>,
        id: i32,
        start_height: i32,
        end_height: i32,
        start_delay: i32,
        speed: i32,
    },
    /// `template.copy(source, victim, speedMultiplier)` of a static template
    /// made with the nine-argument `create(null, null, ...)`.
    Copy {
        template: [i32; 7],
        source: &'a H::Entity,
        victim: Option<&'a H::Entity>,
        speed: f64,
    },
}

/// Everything the special handlers reach outside their classes beyond the
/// combat slices' services. Each fallible method is one original call; a
/// failure leaves earlier effects in place. None has a default.
pub trait Host: cp::Host {
    /// The binding of the native settings to a player's world.
    type SettingsContext: settings::Context;
    /// A timer handle (`RSTimer`) `spawnTimer` answers.
    type Timer;

    /// `handler is XSpecialHandler`: the subclass of a handler instance.
    fn special_of(&self, handler: &Self::HandlerRef) -> Option<Special>;
    /// `player.getSettings()` (a shared handle) and its context.
    fn settings(&mut self, player: &Self::Entity) -> (Settings, &mut Self::SettingsContext);
    /// `ServerConstants.DRAGON_AXE_USE_OSRS_SPEC`.
    fn osrs_dragon_axe(&self) -> bool;
    /// `Animation(id, priority)`.
    fn new_animation(&self, id: i32, priority: Priority) -> Self::Animation;

    /// `properties.isMultiZone()`.
    fn is_multi_zone(&mut self, entity: &Self::Entity) -> Result<bool>;
    /// `RegionManager.getLocalEntities(location, distance)`.
    fn local_entities(&mut self, at: [i32; 3], distance: i32) -> Result<Vec<Self::Entity>>;
    /// `RegionManager.getLocalNPCs(location, distance)`.
    fn local_npcs(&mut self, at: [i32; 3], distance: i32) -> Result<Vec<Self::Entity>>;
    /// `RegionManager.getLocalPlayers(location, distance)`.
    fn local_players(&mut self, at: [i32; 3], distance: i32) -> Result<Vec<Self::Entity>>;
    /// `WildernessZone.getInstance().continueAttack(attacker, victim, style, message)`.
    fn wilderness_continue_attack(
        &mut self,
        attacker: &Self::Entity,
        victim: &Self::Entity,
        style: CombatStyle,
        message: bool,
    ) -> Result<bool>;
    /// `Repository.findNPC(location)`.
    fn find_npc(&mut self, at: [i32; 3]) -> Result<Option<Self::Entity>>;
    /// `npc.getOriginalId()`.
    fn original_id(&mut self, npc: &Self::Entity) -> Result<i32>;
    /// `skills.setPrayerPoints(value)`.
    fn set_prayer_points(&mut self, entity: &Self::Entity, value: f64) -> Result<()>;
    /// `skills.incrementPrayerPoints(amount)`.
    fn increment_prayer_points(&mut self, entity: &Self::Entity, amount: f64) -> Result<()>;
    /// ContentAPI `sendMessage(player, message)`.
    fn api_message(&mut self, player: &Self::Entity, text: &str) -> Result<()>;
    /// `entity.sendChat(text)`.
    fn send_chat(&mut self, entity: &Self::Entity, text: &str) -> Result<()>;
    /// ContentAPI `playAudio(player, id, delay)` with its default loops,
    /// location and radius.
    fn play_audio(&mut self, player: &Self::Entity, id: i32, delay: i32) -> Result<()>;
    /// `playGlobalAudio(location, id, delay)` with its default loops and radius.
    fn play_global_audio_delayed(&mut self, at: [i32; 3], id: i32, delay: i32) -> Result<()>;
    /// `TimerRegistry.getTimerInstance(identifier, args)`.
    fn spawn_timer(&mut self, identifier: &str, args: &[TimerArg]) -> Result<Option<Self::Timer>>;
    /// `entity.timers.registerTimer(timer)` of a spawned timer.
    fn register_timer(&mut self, entity: &Self::Entity, timer: Self::Timer) -> Result<()>;
    /// `entity.timers.registerTimer(timer)` of an anonymous special timer.
    fn register_special_timer(
        &mut self,
        entity: &Self::Entity,
        timer: SpecialTimer<Self>,
    ) -> Result<()>;
    /// `details.getRights()`.
    fn rights(&mut self, player: &Self::Entity) -> Result<Option<Rights>>;
    /// `prayer.get(type)` of a protection prayer named by its `PrayerType`.
    fn prayer_on(&mut self, player: &Self::Entity, prayer: &'static str) -> Result<bool>;
    /// `prayer.toggle(type)`.
    fn prayer_toggle(&mut self, player: &Self::Entity, prayer: &'static str) -> Result<bool>;
    /// `player.continueAttack(target, style, message)`.
    fn continue_attack(
        &mut self,
        player: &Self::Entity,
        target: &Self::Entity,
        style: CombatStyle,
        message: bool,
    ) -> Result<bool>;
    /// `getAttribute("combat-stop", -1)`.
    fn combat_stop(&mut self, entity: &Self::Entity) -> Result<i32>;
    /// `locks.lockMovement(ticks)`.
    fn lock_movement(&mut self, entity: &Self::Entity, ticks: i32) -> Result<()>;
    /// `entity.clocks[Clocks.STUN] = value`.
    fn set_stun_clock(&mut self, entity: &Self::Entity, value: i32) -> Result<()>;
    /// `Pathfinder.find(entity, dest, false, Pathfinder.DUMB).isSuccessful()`.
    fn dumb_path(&mut self, entity: &Self::Entity, dest: [i32; 3]) -> Result<bool>;
    /// `walkingQueue.addPath(x, y)`.
    fn add_path(&mut self, entity: &Self::Entity, x: i32, y: i32) -> Result<()>;
    /// `ItemDefinition.forId(id).getName()`.
    fn definition_name(&mut self, item_id: i32) -> Result<String>;
    /// `getAttribute("chain-hit_v")`: the stored list, copied.
    fn chain_victims(&mut self, entity: &Self::Entity) -> Result<Option<Vec<Self::Entity>>>;
    /// `setAttribute("chain-hit_v", list)`.
    fn set_chain_victims(&mut self, entity: &Self::Entity, list: Vec<Self::Entity>) -> Result<()>;
    /// The stored list object itself, for the list's own (unrecorded) edits.
    fn chain_victims_mut(&mut self, entity: &Self::Entity) -> Option<&mut Vec<Self::Entity>>;
    /// `GameWorld.Pulser.submit(pulse)` of a special's pulse. The runner
    /// calls [`SpecialPulse::update`] once a tick until it answers true.
    fn submit_special(&mut self, pulse: SpecialPulse<Self>) -> Result<()>;
    fn send_special_projectile(&mut self, projectile: SpecialProjectile<'_, Self>) -> Result<()>;
}

pub(crate) fn settings_error(error: settings::Error) -> Error {
    match error {
        settings::Error::Null => Error::Null,
        settings::Error::Index => Error::Index,
        other => Error::Binding(format!("settings {other:?}")),
    }
}

/// `((Player) entity).getSettings().drainSpecial(amount)`.
pub(crate) fn drain_special<H: Host>(h: &mut H, player: &H::Entity, amount: i32) -> Result<bool> {
    let (settings, ctx) = h.settings(player);
    settings.drain_special(ctx, amount).map_err(settings_error)
}

/// `getSettings().toggleSpecialBar()`.
pub(crate) fn toggle_special_bar<H: Host>(h: &mut H, player: &H::Entity) -> Result<()> {
    let (settings, ctx) = h.settings(player);
    settings.toggle_special_bar(ctx).map_err(settings_error)
}

/// The cast `(Player) entity`: a non-player is not modelled (see docs).
pub(crate) fn player<'a, H: Host>(h: &H, entity: Option<&'a H::Entity>) -> Result<&'a H::Entity> {
    let entity = entity.ok_or(Error::Null)?;
    if !h.kind(entity).is_player() {
        return Err(Error::Binding("ClassCastException: not a player".into()));
    }
    Ok(entity)
}

/// `RandomFunction.random(maxValue)`.
pub(crate) fn random<H: Host>(h: &mut H, max_value: i32) -> Result<i32> {
    if max_value <= 0 {
        return Ok(0);
    }
    h.next_int(max_value)
}

/// `RandomFunction.random(a, b)`.
pub(crate) fn random_between<H: Host>(h: &mut H, a: i32, b: i32) -> Result<i32> {
    let n = b.wrapping_sub(a).wrapping_abs();
    let low = a.min(b);
    Ok(low.wrapping_add(if n == 0 { 0 } else { random(h, n)? }))
}

/// The instance's base-class fields.
pub(crate) fn instance<H: Host>(h: &mut H, r: &H::HandlerRef) -> Handler {
    h.handler(r).clone()
}

/// `entity.getLocation()`, required non-null.
pub(crate) fn at<H: Host>(h: &mut H, entity: &H::Entity) -> Result<[i32; 3]> {
    h.location(entity)?.ok_or(Error::Null)
}

/// `entity.getLocation().getDistance(victim.getLocation())`.
pub(crate) fn distance<H: Host>(h: &mut H, a: &H::Entity, b: &H::Entity) -> Result<f64> {
    let here = h.location(a)?;
    let there = h.location(b)?;
    Ok(cs::distance(
        here.ok_or(Error::Null)?,
        there.ok_or(Error::Null)?,
    ))
}

/// `playGlobalAudio(entity.getLocation(), id)`.
pub(crate) fn audio<H: Host>(h: &mut H, entity: &H::Entity, id: i32) -> Result<()> {
    let location = at(h, entity)?;
    h.play_global_audio(location, id)
}

/// `entity.visualize(new Animation(id, HIGH), graphics)`.
pub(crate) fn show<H: Host>(
    h: &mut H,
    entity: &H::Entity,
    animation: i32,
    graphics: Option<Graphics>,
) -> Result<()> {
    let animation = h.new_animation(animation, Priority::High);
    h.visualize_entity(entity, Some(&animation), graphics)
}

/// `new Graphics(id, height, delay)`.
pub(crate) const fn gfx(id: i32, height: i32, delay: i32) -> Graphics {
    Graphics { id, height, delay }
}

/// `Location.withinDistance(other, dist)`.
pub(crate) fn within_distance(this: [i32; 3], other: [i32; 3], dist: i32) -> bool {
    if other[2] != this[2] {
        return false;
    }
    cs::distance(this, other) <= f64::from(dist)
}

/// `Location.transform(dx, dy, z)`.
pub(crate) fn transform(l: [i32; 3], dx: i32, dy: i32, dz: i32) -> [i32; 3] {
    base::location(
        l[0].wrapping_add(dx),
        l[1].wrapping_add(dy),
        l[2].wrapping_add(dz).wrapping_rem(4),
    )
}

/// `victim.getProperties().getCombatPulse().isAttacking()`.
pub(crate) fn is_attacking<H: Host>(h: &mut H, entity: &H::Entity) -> Result<bool> {
    cpulse::is_attacking(h, entity)
}
