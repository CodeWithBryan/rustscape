//! The members the generic AbstractNPC classes override, dispatched by the
//! NPC's class: `init` (ShopNPC, BankerNPC, DarkWizardNPC), `tick` (CowNPC,
//! DuckNPC), `handleTickActions` (IdleAbstractNPC), `finalizeDeath`
//! (RatNPC, HighwayManNPC, IceGiantNPC), `onAttack` (HighwayManNPC),
//! `onImpact` (DarkWizardNPC), `getSwingHandler` (MonkNPC and its
//! `CombatAction`) and `getWalkRadius` (ShopNPC). An NPC without one of
//! these classes runs the NPC member.
use super::{Class, CombatStyle, Diary, Error, Host, Kind, Result, Spell, Value, banker, idle};

/// `Item(300)`: the rat tail.
pub const RAT_TAIL: i32 = 300;
/// `Quests.WITCHS_POTION`.
pub const WITCHS_POTION: &str = "WITCHS_POTION";
/// `DarkWizardNPC.SPELLS`, by the position of the NPC's id in `ID`.
pub const DARK_WIZARD_SPELLS: [[i32; 2]; 2] = [[6, 7], [4, 2]];

/// `init()`.
pub fn init<H: Host + ?Sized>(h: &mut H, npc: &H::Npc) -> Result<()> {
    match h.class(npc) {
        Some(Class::Shop) => {
            h.base_init(npc)?;
            h.set_walks(npc, true)
        }
        Some(Class::Banker) => banker::init(h, npc),
        Some(Class::DarkWizard) => {
            h.base_init(npc)?;
            h.set_combat_style(npc, CombatStyle::Magic)?;
            dark_wizard_default(h, npc)
        }
        _ => h.base_init(npc),
    }
}

/// `tick()`.
pub fn tick<H: Host + ?Sized>(h: &mut H, npc: &H::Npc) -> Result<()> {
    match h.class(npc) {
        Some(Class::Cow) => {
            // calves (1766) don't moo
            if h.random(45)? == 5 && h.id(npc)? != 1766 {
                h.send_chat(npc, "Moo")?;
            }
            h.base_tick(npc)
        }
        Some(Class::Duck) => {
            if h.random(45)? == 5 {
                h.send_chat(npc, "Quack!")?;
            }
            h.base_tick(npc)
        }
        _ => h.base_tick(npc),
    }
}

/// `handleTickActions()`.
pub fn tick_actions<H: Host + ?Sized>(h: &mut H, npc: &H::Npc) -> Result<()> {
    if h.kind(npc) == Kind::Idle {
        idle::handle_tick_actions(h, npc)
    } else {
        h.base_tick_actions(npc)
    }
}

/// `finalizeDeath(killer)`.
pub fn finalize_death<H: Host + ?Sized>(
    h: &mut H,
    npc: &H::Npc,
    killer: Option<&H::Npc>,
) -> Result<()> {
    let player = |h: &H| killer.filter(|k| h.kind(k) == Kind::Player);
    match h.class(npc) {
        Some(Class::Rat) => {
            h.base_finalize_death(npc, killer)?;
            if let Some(p) = player(h) {
                if !h.get_quest(p, WITCHS_POTION)? {
                    return Err(Error::Null);
                }
                if h.quest_started(p, WITCHS_POTION)? {
                    let location = h.location(npc)?;
                    h.ground_item(RAT_TAIL, 1, location, p)?;
                }
            }
            Ok(())
        }
        Some(Class::HighwayMan) => {
            h.base_finalize_death(npc, killer)?;
            if h.id(npc)? == 180 {
                if let Some(p) = player(h) {
                    falador_task(h, p, 0, 10, |_, _| Ok(true))?;
                }
            }
            Ok(())
        }
        Some(Class::IceGiant) => {
            h.base_finalize_death(npc, killer)?;
            if let Some(p) = player(h) {
                let location = h.location(npc)?.ok_or(Error::Null)?;
                let near = within_distance(location, [3052, 9573, 0], 100);
                falador_task(h, p, 1, 4, move |_, _| Ok(near))?;
            }
            Ok(())
        }
        _ => h.base_finalize_death(npc, killer),
    }
}

/// `if (<first> && !getDiary(FALADOR).isComplete(level, task))
/// getDiary(FALADOR).updateTask(player, level, task, true)`.
fn falador_task<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Npc,
    level: i32,
    task: i32,
    first: impl FnOnce(&mut H, &H::Npc) -> Result<bool>,
) -> Result<()> {
    if !first(h, p)? {
        return Ok(());
    }
    if !h.get_diary(p, Diary::Falador)? {
        return Err(Error::Null);
    }
    if !h.diary_complete(Diary::Falador, level, task)? {
        if !h.get_diary(p, Diary::Falador)? {
            return Err(Error::Null);
        }
        h.diary_update(Diary::Falador, p, level, task, true)?;
    }
    Ok(())
}

/// `Location.withinDistance(other, dist)`: the same plane and
/// `getDistance` (Euclidean, from Int differences) at most `dist`.
pub fn within_distance(a: [i32; 3], b: [i32; 3], dist: i32) -> bool {
    if a[2] != b[2] {
        return false;
    }
    let dx = a[0].wrapping_sub(b[0]);
    let dy = a[1].wrapping_sub(b[1]);
    let sum = dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy));
    f64::from(sum).sqrt() <= f64::from(dist)
}

/// `onAttack(target)`.
pub fn on_attack<H: Host + ?Sized>(h: &mut H, npc: &H::Npc, target: Option<&H::Npc>) -> Result<()> {
    match h.class(npc) {
        Some(Class::HighwayMan) => h.send_chat(npc, "Stand and deliver!").map(|_| ()),
        _ => h.base_on_attack(npc, target),
    }
}

/// `onImpact(entity, state)`.
pub fn on_impact<H: Host + ?Sized>(h: &mut H, npc: &H::Npc, entity: Option<&H::Npc>) -> Result<()> {
    match h.class(npc) {
        Some(Class::DarkWizard) => {
            h.base_on_impact(npc, entity)?;
            let switched = h.get_attribute(npc, "switched")?;
            if Value::bool(switched, false)? {
                h.remove_attribute(npc, "switched")?;
                dark_wizard_default(h, npc)?;
                return Ok(());
            }
            if h.random(6)? > 4 {
                // setSpells(getSpells())
                let id = h.id(npc)?;
                let index = super::ids::DARK_WIZARD
                    .iter()
                    .position(|i| *i == id)
                    .unwrap_or(0);
                let spells = DARK_WIZARD_SPELLS[index];
                let pick = h.random(spells.len() as i32)?;
                let button = *usize::try_from(pick)
                    .ok()
                    .and_then(|i| spells.get(i))
                    .ok_or(Error::Index)?;
                let spell = combat_spell(h.modern_spell(button)?)?;
                h.set_autocast_spell(npc, spell)?;
                h.set_attribute(npc, "switched", Value::Bool(true))?;
            }
            Ok(())
        }
        _ => h.base_on_impact(npc, entity),
    }
}

/// `DarkWizardNPC.setDefault()`.
fn dark_wizard_default<H: Host + ?Sized>(h: &mut H, npc: &H::Npc) -> Result<()> {
    let button = if h.id(npc)? == 172 { 6 } else { 4 };
    let spell = combat_spell(h.modern_spell(button)?)?;
    h.set_autocast_spell(npc, spell)
}

/// `(CombatSpell) spell`.
fn combat_spell(spell: Option<Spell>) -> Result<Option<i32>> {
    match spell {
        None => Ok(None),
        Some(Spell::Combat(id)) => Ok(Some(id)),
        Some(Spell::Other) => Err(Error::Cast),
    }
}

/// `getWalkRadius()`.
pub fn walk_radius<H: Host + ?Sized>(h: &mut H, npc: &H::Npc) -> Result<i32> {
    match h.class(npc).and_then(Class::walk_radius) {
        Some(radius) => Ok(radius),
        None => h.base_walk_radius(npc),
    }
}

/// A swing handler: MonkNPC's static `CombatAction`, or the host's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Swing<T> {
    /// `MonkNPC.COMBAT` (one object for every monk).
    Monk,
    Other(T),
}

/// `getSwingHandler(swing)`.
pub fn swing_handler<H: Host + ?Sized>(
    h: &mut H,
    npc: &H::Npc,
    swing: bool,
) -> Result<Swing<H::Handler>> {
    match h.class(npc) {
        Some(Class::Monk) => Ok(Swing::Monk),
        _ => h.base_swing_handler(npc, swing).map(Swing::Other),
    }
}

/// `handler.swing(entity, victim, state)` for MonkNPC's `CombatAction`:
/// heal when hurt and `randomize(10) < 2`, otherwise the melee swing.
pub fn monk_swing<H: Host + ?Sized>(
    h: &mut H,
    entity: &H::Npc,
    victim: Option<&H::Npc>,
) -> Result<i32> {
    if h.lifepoints(entity)? != h.maximum_lifepoints(entity)? && h.randomize(10)? < 2 {
        h.animate(entity, 709)?;
        h.heal(entity, 2)?;
        h.set_next_attack(entity, 4)?;
        return Ok(-1);
    }
    h.melee_swing(entity, victim)
}
