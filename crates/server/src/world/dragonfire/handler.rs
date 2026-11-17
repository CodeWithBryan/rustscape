//! `DragonfireSwingHandler`: a direct subclass of `CombatSwingHandler` with
//! type MAGIC that overrides every abstract member.
use super::{
    DragonProjectile, Dragonfire, Host, ProjectileTemplate, Subclass, SwitchAttack, Value, api,
    gfx, random,
};
use crate::world::combat_pulse::{handler as base, impact, slot};
use crate::world::combat_swing::{
    BattleState, CombatStyle, Error, Graphics, Handler, HandlerKind, InteractionType, Result,
};
use crate::world::special_attacks::{Priority, TimerArg};

pub const DRAGONFIRE_SHIELD: i32 = 11283;
pub const DRAGONFIRE_SHIELD_UNCHARGED: i32 = 11284;

/// The instance's fields.
pub(crate) fn fields<H: Host>(h: &H, r: &H::HandlerRef) -> Result<Dragonfire<H>> {
    match h.subclass(r) {
        Some(Subclass::Dragonfire(d)) => Ok(d.clone()),
        _ => Err(Error::Binding("not a DragonfireSwingHandler".into())),
    }
}

/// `new DragonfireSwingHandler(meleeRange, maximumHit, attack, fire)`:
/// `super(CombatStyle.MAGIC)` and no flags.
pub fn new<H: Host>(
    h: &mut H,
    melee_range: bool,
    maximum_hit: i32,
    attack: Option<SwitchAttack<H>>,
    fire: bool,
) -> H::HandlerRef {
    h.new_handler(
        Handler::new(HandlerKind::Magic, &[]),
        Subclass::Dragonfire(Dragonfire {
            melee_range,
            maximum_hit,
            attack,
            fire,
            shield: None,
        }),
    )
}

/// `DragonfireSwingHandler.get(meleeRange, maximumHit, animation, startGraphic,
/// endGraphic, projectile[, fire])`: the attack uses its handler, and the
/// handler is a new instance holding the attack. The six-argument form is
/// fiery.
#[allow(clippy::too_many_arguments)]
pub fn get<H: Host>(
    h: &mut H,
    melee_range: bool,
    maximum_hit: i32,
    animation: Option<H::Animation>,
    start_graphic: Option<Graphics>,
    end_graphic: Option<Graphics>,
    projectile: Option<ProjectileTemplate<H>>,
    fire: bool,
) -> SwitchAttack<H> {
    let mut attack = SwitchAttack::new(None, animation, start_graphic, end_graphic, projectile);
    attack.use_handler = true;
    let made = new(h, melee_range, maximum_hit, Some(attack.clone()), fire);
    attack.handler = Some(made);
    attack
}

/// `canSwing`: the melee or the magic singleton's, by `meleeRange`.
pub fn can_swing<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: &H::Entity,
    victim: &H::Entity,
) -> Result<InteractionType> {
    let d = fields(h, r)?;
    let style = if d.melee_range {
        CombatStyle::Melee
    } else {
        CombatStyle::Magic
    };
    let singleton = h.style_handler(style);
    base::can_swing(h, &singleton, entity, victim)
}

pub(crate) fn swing_member<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<i32> {
    swing(h, r, entity, victim, Some(state))
}

/// `swing(entity, victim, state)`: the dragonfire maximum, a hit of
/// `random(max + 1)`, style MAGIC; 1 tick in melee range, otherwise
/// `2 + floor(distance / 2)` with `fireBreath` set to the tick it lands plus 2.
pub fn swing<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: Option<&mut BattleState<H>>,
) -> Result<i32> {
    let d = fields(h, r)?;
    let max = base::calculate_hit(h, r, entity, victim, 1.0)?;
    let hit = random(h, max.wrapping_add(1))?;
    let state = state.ok_or(Error::Null)?;
    state.maximum_hit = max;
    state.style = Some(CombatStyle::Magic);
    state.estimated_hit = hit;
    if d.melee_range {
        return Ok(1);
    }
    let entity = entity.ok_or(Error::Null)?;
    let here = h.location(entity)?.ok_or(Error::Null)?;
    let there = h.location(victim.ok_or(Error::Null)?)?.ok_or(Error::Null)?;
    let distance = crate::world::combat_swing::distance(here, there);
    let ticks = 2i32.wrapping_add((distance * 0.5).floor() as i32);
    let now = h.ticks()?;
    let value = now.wrapping_add(ticks.wrapping_add(2));
    h.set_attribute_value(entity, "fireBreath", Value::Int(value))?;
    Ok(ticks)
}

/// `visualize`: the attack's animation and start graphic, and a copy of its
/// projectile at speed 5.
pub fn visualize<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: &H::Entity,
    victim: Option<&H::Entity>,
    _state: &BattleState<H>,
) -> Result<()> {
    let d = fields(h, r)?;
    let attack = d.attack.ok_or(Error::Null)?;
    h.visualize_entity(entity, attack.animation.as_ref(), attack.start_graphic)?;
    if let Some(template) = &attack.projectile {
        h.send_dragon_projectile(DragonProjectile::Copy {
            template,
            source: entity,
            victim,
            speed: 5.0,
        })?;
    }
    Ok(())
}

/// `visualizeImpact`: an NPC's breath charges a player's dragonfire shield
/// (an uncharged one becomes a charged one at 0) by 20 up to 1000 and ends
/// there; icy breath may freeze; then the defence animation with the end
/// graphic.
pub fn visualize_impact<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    _state: &mut BattleState<H>,
) -> Result<()> {
    let d = fields(h, r)?;
    if let (Some(e), Some(p)) = (entity, victim)
        && h.kind(e).is_npc()
        && h.kind(p).is_player()
    {
        let shield = h.equipment(p, slot::SHIELD)?;
        let id = shield.as_ref().map(|s| h.item_id(s));
        if let Some(mut shield) = shield
            .filter(|_| id == Some(DRAGONFIRE_SHIELD) || id == Some(DRAGONFIRE_SHIELD_UNCHARGED))
        {
            if h.item_id(&shield) == DRAGONFIRE_SHIELD_UNCHARGED {
                h.equipment_replace(p, Some((DRAGONFIRE_SHIELD, 1, 1000)), slot::SHIELD)?;
                shield = h.equipment(p, slot::SHIELD)?.ok_or(Error::Null)?;
                h.set_item_charge(&shield, 0);
            }
            if h.item_charge(&shield) < 1000 {
                let charge = h.item_charge(&shield).wrapping_add(20);
                h.set_item_charge(&shield, charge);
                h.update_bonuses(p)?;
                h.send_message(p, "Your dragonfire shield glows more brightly.")?;
                h.play_audio(p, 3740, 0)?;
                let center = h.center_location(e)?;
                h.face_location(p, Some(center))?;
                let animation = h.new_animation(6695, Priority::Mid);
                h.visualize_entity(p, Some(&animation), Some(gfx(1163, 0)))?;
            } else {
                h.send_message(p, "Your dragonfire shield is already fully charged.")?;
            }
            return Ok(());
        }
    }
    let victim = victim.ok_or(Error::Null)?;
    if !d.fire && !h.timer_active(victim, "frozen:immunity")? && random(h, 4)? == 2 {
        let args = [TimerArg::Int(16), TimerArg::Bool(true)];
        if let Some(timer) = h.spawn_timer("frozen", &args)? {
            h.register_timer(victim, timer)?;
        }
        h.graphics(victim, gfx(502, 0))?;
    }
    let graphic = d.attack.as_ref().and_then(|a| a.end_graphic);
    let animation = h.defence_animation(victim)?;
    h.visualize_entity(victim, animation.as_ref(), graphic)
}

/// `impact`: both hits as MAGIC through the victim's impact handler.
pub fn impact<H: Host>(
    h: &mut H,
    _: &H::HandlerRef,
    entity: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    state: &mut BattleState<H>,
) -> Result<()> {
    for hit in [state.estimated_hit, state.secondary_hit] {
        if hit > -1 {
            let victim = victim.ok_or(Error::Null)?;
            let style = Some(CombatStyle::Magic);
            impact::handle_impact(h, victim, entity, hit, style, None, false)?;
        }
    }
    Ok(())
}

/// `adjustBattleState`: icy breath above 10 against an elemental or mind
/// shield is redrawn as `random(10)`; `CombatSwingHandler.adjustBattleState`
/// runs and the style is put back.
pub fn adjust_battle_state<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    entity: &H::Entity,
    victim: &H::Entity,
    state: &mut BattleState<H>,
) -> Result<()> {
    let d = fields(h, r)?;
    if h.kind(victim).is_player() && !d.fire {
        let item = h.equipment(victim, slot::SHIELD)?;
        let id = item.as_ref().map(|i| h.item_id(i));
        if (id == Some(api::ELEMENTAL_SHIELD) || id == Some(api::MIND_SHIELD))
            && state.estimated_hit > 10
        {
            state.estimated_hit = random(h, 10)?;
        }
    }
    let style = state.style;
    base::base_adjust(h, r, entity, victim, state)?;
    state.style = style;
    Ok(())
}

/// `getFormattedHit`: `formatHit(victim, hit)` alone.
pub fn formatted_hit<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    _: &H::Entity,
    victim: &H::Entity,
    _: &BattleState<H>,
    hit: i32,
) -> Result<i32> {
    let instance = h.handler(r).clone();
    instance.format_hit(h, Some(victim), hit)
}

pub fn calculate_accuracy<H: Host>(
    _: &mut H,
    _: &H::HandlerRef,
    _: Option<&H::Entity>,
) -> Result<i32> {
    Ok(4000)
}

/// `calculateHit`: `calculateDragonfireMaxHit(victim, maximumHit, !fire, 0,
/// true)`.
pub fn calculate_hit<H: Host>(
    h: &mut H,
    r: &H::HandlerRef,
    _: Option<&H::Entity>,
    victim: Option<&H::Entity>,
    _: f64,
) -> Result<i32> {
    let d = fields(h, r)?;
    let victim = victim.ok_or(Error::Null)?;
    api::calculate_dragonfire_max_hit(h, victim, d.maximum_hit, !d.fire, 0, true)
}

/// `calculateDefence`: the magic singleton's.
pub fn calculate_defence<H: Host>(
    h: &mut H,
    _: &H::HandlerRef,
    victim: Option<&H::Entity>,
    attacker: Option<&H::Entity>,
) -> Result<i32> {
    let magic = h.style_handler(CombatStyle::Magic);
    base::calculate_defence(h, &magic, victim, attacker)
}

pub fn set_multiplier<H: Host>(
    _: &mut H,
    _: &H::HandlerRef,
    _: Option<&H::Entity>,
    _: i32,
) -> Result<f64> {
    Ok(1.0)
}
