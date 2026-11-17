//! Shared player Skills binding for the gameframe and early gameplay.
use super::{Error, Result, UiContext};
use rs09_protocol::game::updates;
use rs09_server::player::skills::{self, Context, Effect, EntityKind, Skills};

impl UiContext<'_> {
    fn appearance(&self) -> std::sync::RwLockReadGuard<'_, rs09_server::world::appearance::Data> {
        self.host.masks[&self.node].body().read()
    }
}
impl Context for UiContext<'_> {
    fn kind(&self) -> EntityKind {
        EntityKind::Player
    }
    fn version(&self) -> i32 {
        6
    }
    fn tick(&mut self) -> i32 {
        self.state.ticks
    }
    fn world_tick(&mut self) -> i32 {
        self.state.ticks
    }
    fn combat_level(&self) -> i32 {
        self.appearance().combat_level
    }
    fn summoning_combat_level(&self) -> i32 {
        self.appearance().summoning_combat_level
    }
    fn npc_combat_level(&mut self) -> skills::Result<i32> {
        Err(skills::Error::Binding)
    }
    fn damage_spong_active(&mut self) -> skills::Result<bool> {
        Ok(false)
    }
    fn assisted_level(&mut self, _: i32) -> skills::Result<Option<i32>> {
        Ok(None)
    }
    fn translate_experience(
        &mut self,
        _: &mut Skills,
        _: i32,
        _: f64,
        _: f64,
    ) -> skills::Result<bool> {
        Ok(false)
    }
    fn brawling_present(&mut self) -> skills::Result<bool> {
        Ok(false)
    }
    fn brawling_for_skill(&mut self, _: i32) -> skills::Result<Option<i32>> {
        Err(skills::Error::Binding)
    }
    fn equipment_contains(&mut self, id: i32) -> skills::Result<bool> {
        let owner = self
            .state
            .retain_node(self.node)
            .map_err(|_| skills::Error::Binding)?;
        let player = self
            .state
            .player_fields(&owner)
            .map_err(|_| skills::Error::Binding)?;
        Ok(player.equipment().contain_items(&[id]))
    }
    fn brawling_bonus(&mut self) -> skills::Result<f64> {
        Err(skills::Error::Binding)
    }
    fn tutorial_complete(&mut self) -> skills::Result<bool> {
        Ok(true)
    }
    fn artificial(&mut self) -> skills::Result<bool> {
        Ok(false)
    }
    fn username(&mut self) -> skills::Result<String> {
        Ok(self.appearance().username.clone())
    }
    fn permadeath_present(&self) -> bool {
        false
    }
    fn effect(&mut self, state: &mut Skills, effect: Effect) -> skills::Result<()> {
        match effect {
            Effect::Combat(value) => {
                self.state
                    .actor(self.node)
                    .map_err(|_| skills::Error::Binding)?;
                self.host
                    .masks
                    .get(&self.node)
                    .ok_or(skills::Error::Binding)?
                    .body()
                    .write()
                    .combat_level = value;
            }
            Effect::SummoningCombat(value) => {
                self.state
                    .actor(self.node)
                    .map_err(|_| skills::Error::Binding)?;
                self.host
                    .masks
                    .get(&self.node)
                    .ok_or(skills::Error::Binding)?
                    .body()
                    .write()
                    .summoning_combat_level = value;
            }
            Effect::Packet(slot) => {
                let boosted = if slot == skills::HITPOINTS {
                    state.lifepoints
                } else if slot == skills::PRAYER {
                    state.prayer_points.ceil() as i32
                } else {
                    state.level(slot as i32, true, self)?
                };
                self.host.send(
                    self.node,
                    updates::skill_level(slot as i32, boosted, state.experience[slot]),
                );
            }
            Effect::Appearance => {
                self.host
                    .masks
                    .get(&self.node)
                    .ok_or(skills::Error::Binding)?
                    .sync_appearance(super::super::now)
                    .map_err(|_| skills::Error::Binding)?;
            }
            Effect::LevelUp { skill, .. } => self.host.send(
                self.node,
                super::Frame {
                    opcode: 70,
                    payload: format!(
                        "You've advanced a {} level.\0",
                        skills::NAMES[skill as usize]
                    )
                    .into_bytes(),
                },
            ),
            // The alpha has no quest/perk subscribers or durable XP audit sink.
            Effect::XpPlugins { .. } | Effect::XpEvent { .. } | Effect::XpLog(_) => {}
            // A fresh native player has no saved levelup:* attributes. Later XP
            // or loaded profile attributes require the complete LevelUp service.
            Effect::FlashingIcons(-1) => self.host.send(self.node, updates::varp(1179, 0)),
            _ => return Err(skills::Error::Binding),
        }
        Ok(())
    }
}

fn with_skills<T>(
    ctx: &mut UiContext<'_>,
    apply: impl FnOnce(&mut Skills, &mut UiContext<'_>) -> skills::Result<T>,
) -> Result<T> {
    let mut skills = ctx.row_mut()?.skills.take().ok_or(Error::Null)?;
    let result = apply(&mut skills, ctx);
    ctx.row_mut()?.skills = Some(skills);
    result.map_err(|e| Error::Binding(e.to_string()))
}

pub(super) fn award_combat(
    ctx: &mut UiContext<'_>,
    style: super::super::weapons::Stance,
    hit: i32,
) -> Result<()> {
    use super::super::weapons::Stance::*;
    with_skills(ctx, |s, c| {
        let hit = f64::from(hit);
        s.add_experience(skills::HITPOINTS as i32, hit * 1.33, true, c)?;
        let awards: &[(i32, f64)] = match style {
            Accurate => &[(0, 4.0)],
            Defensive => &[(1, 4.0)],
            Aggressive => &[(2, 4.0)],
            Controlled => &[(0, 4.0 / 3.0), (2, 4.0 / 3.0), (1, 4.0 / 3.0)],
            RangedAccurate | Rapid => &[(4, 4.0)],
            Longrange => &[(4, 2.0), (1, 2.0)],
            DefensiveCast | Cast => &[],
        };
        for &(skill, rate) in awards {
            s.add_experience(skill, hit * rate, true, c)?;
        }
        Ok(())
    })
}

pub(super) fn award(ctx: &mut UiContext<'_>, skill: i32, xp: f64) -> Result<()> {
    with_skills(ctx, |s, c| s.add_experience(skill, xp, true, c))
}

pub(super) fn take_melee_hit(ctx: &mut UiContext<'_>, damage: i32) -> Result<(i32, i32)> {
    with_skills(ctx, |s, c| {
        let damage = damage.min(s.lifepoints);
        s.hit(damage);
        let max = s.maximum_lifepoints(c)?;
        let remain = s.lifepoints;
        c.effect(s, Effect::Packet(skills::HITPOINTS))?;
        Ok((
            damage,
            if remain < max {
                remain * 255 / max
            } else {
                255
            },
        ))
    })
}

pub(super) fn revive_melee_player(ctx: &mut UiContext<'_>) -> Result<()> {
    with_skills(ctx, |s, c| {
        let max = s.maximum_lifepoints(c)?;
        s.set_lifepoints(max);
        c.effect(s, Effect::Packet(skills::HITPOINTS))
    })
}
pub(super) fn initialize(ctx: &mut UiContext<'_>) -> Result<()> {
    let skills = Skills::new(ctx).map_err(|e| Error::Binding(e.to_string()))?;
    ctx.row_mut()?.skills = Some(skills);
    Ok(())
}
pub(super) fn refresh(ctx: &mut UiContext<'_>) -> Result<()> {
    let mut skills = ctx.row_mut()?.skills.take().ok_or(Error::Null)?;
    let result = skills
        .refresh(ctx)
        .and_then(|()| skills.update_combat_level(ctx).map(|_| ()));
    // Retain partial state even if an external service fails.
    ctx.row_mut()?.skills = Some(skills);
    result.map_err(|e| Error::Binding(e.to_string()))
}

pub(super) fn restore(ctx: &mut UiContext<'_>, saved: &serde_json::Value) -> Result<()> {
    with_skills(ctx, |s, c| s.parse_profile(saved, c))
}
