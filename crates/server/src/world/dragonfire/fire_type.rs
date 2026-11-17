//! `FireType`: the four dragon breaths with their animation, projectile and
//! breath effect (`NodeTask.exec`).
use super::{Host, random};
use crate::world::combat_swing::{Error, Result};
use crate::world::special_attacks::{Priority, TimerArg};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FireType {
    FieryBreath,
    ShockingBreath,
    ToxicBreath,
    IcyBreath,
}

impl FireType {
    pub const ALL: [Self; 4] = [
        Self::FieryBreath,
        Self::ShockingBreath,
        Self::ToxicBreath,
        Self::IcyBreath,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::FieryBreath => "FIERY_BREATH",
            Self::ShockingBreath => "SHOCKING_BREATH",
            Self::ToxicBreath => "TOXIC_BREATH",
            Self::IcyBreath => "ICY_BREATH",
        }
    }

    /// `getAnimation()`: `new Animation(id, Priority.HIGH)`.
    pub fn animation(self) -> (i32, Priority) {
        let id = match self {
            Self::FieryBreath => 81,
            Self::ShockingBreath => 84,
            Self::ToxicBreath => 82,
            Self::IcyBreath => 83,
        };
        (id, Priority::High)
    }

    /// `getProjectileId()`.
    pub fn projectile_id(self) -> i32 {
        match self {
            Self::FieryBreath => 393,
            Self::ShockingBreath => 396,
            Self::ToxicBreath => 394,
            Self::IcyBreath => 395,
        }
    }

    /// `getTask().exec(node, n...)`: always true. Shocking breath lowers a
    /// random one of attack, defence and strength by 5 three times in ten;
    /// toxic breath poisons with `n[0]` as the source at 40; icy breath
    /// registers a seven-tick freeze.
    pub fn exec<H: Host>(
        self,
        h: &mut H,
        node: &H::Entity,
        others: &[Option<H::Entity>],
    ) -> Result<bool> {
        match self {
            Self::FieryBreath => {}
            Self::ShockingBreath => {
                if random(h, 10)? < 3 {
                    let skill = random(h, 3)?;
                    h.update_level(node, skill, -5, 0)?;
                    if h.kind(node).is_player() {
                        h.send_message(node, "You have been shocked.")?;
                    }
                }
            }
            Self::ToxicBreath => {
                let source = others.first().ok_or(Error::Index)?;
                let source = source.as_ref().ok_or(Error::Null)?;
                h.apply_poison(node, source, 40)?;
            }
            Self::IcyBreath => {
                let args = [TimerArg::Int(7), TimerArg::Bool(true)];
                if let Some(timer) = h.spawn_timer("frozen", &args)? {
                    h.register_timer(node, timer)?;
                }
            }
        }
        Ok(true)
    }
}
