//! BAS opcodes 29–37: accelerated movement speed, accelerated yaw and body
//! lean, from `BasType.decode`, `NpcList.processMovement`/`processFacing` and
//! `PathingEntity.applyBodyLean`.
//!
//! All arithmetic wraps as Java `int` does. The one place the original can
//! fail is a lean record whose acceleration is zero: there it divides by zero
//! inside the render call; [`State::lean`] returns an error instead and the
//! caller draws the body without lean.
use crate::{
    animation::sin_cos,
    model::Model,
    walking::{Movement, PlayerBas},
};
use rs09_protocol::{Result, buffer::Reader};

/// The nine `BasType` fields behind opcodes 29–37, with their defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bas {
    /// Opcode 29, `yawAcceleration`; zero selects the plain turning code.
    pub yaw_acceleration: i32,
    /// Opcode 30, `yawMaxSpeed`.
    pub yaw_max_speed: i32,
    /// Opcode 31, `rollAcceleration`.
    pub roll_acceleration: i32,
    /// Opcode 32, `rollMaxSpeed`.
    pub roll_max_speed: i32,
    /// Opcode 33, `rollTargetAngle` (signed).
    pub roll_target: i32,
    /// Opcode 34, `pitchAcceleration`.
    pub pitch_acceleration: i32,
    /// Opcode 35, `pitchMaxSpeed`.
    pub pitch_max_speed: i32,
    /// Opcode 36, `pitchTargetAngle` (signed).
    pub pitch_target: i32,
    /// Opcode 37, `movementAcceleration`; -1 selects the plain speeds.
    pub movement_acceleration: i32,
}
impl Default for Bas {
    fn default() -> Self {
        Self {
            yaw_acceleration: 0,
            yaw_max_speed: 0,
            roll_acceleration: 0,
            roll_max_speed: 0,
            roll_target: 0,
            pitch_acceleration: 0,
            pitch_max_speed: 0,
            pitch_target: 0,
            movement_acceleration: -1,
        }
    }
}
impl Bas {
    /// One of opcodes 29–37; the caller has consumed the opcode byte.
    pub fn decode(&mut self, opcode: i32, r: &mut Reader<'_>) -> Result<()> {
        match opcode {
            29 => self.yaw_acceleration = r.g1()?,
            30 => self.yaw_max_speed = r.g2()?,
            31 => self.roll_acceleration = r.g1()?,
            32 => self.roll_max_speed = r.g2()?,
            33 => self.roll_target = r.g2b()?,
            34 => self.pitch_acceleration = r.g1()?,
            35 => self.pitch_max_speed = r.g2()?,
            36 => self.pitch_target = r.g2b()?,
            37 => self.movement_acceleration = r.g1()?,
            _ => {}
        }
        Ok(())
    }

    /// `applyBodyLean` returns at once unless one of the lean targets is set.
    pub fn leans(&self) -> bool {
        self.roll_target != 0 || self.pitch_target != 0
    }
}

/// One lean axis: `rollTarget`/`rollVelocity`/`rollAngle`/`rollMidpoint`/
/// `rollAccelerating`, or the pitch fields of the same shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Axis {
    pub target: i32,
    pub velocity: i32,
    pub angle: i32,
    pub midpoint: i32,
    pub accelerating: bool,
}
impl Axis {
    /// One render call's worth of one axis. `None` is the original's division
    /// by zero.
    fn step(&mut self, target: i32, acceleration: i32, max_speed: i32) -> Option<()> {
        let divisor = acceleration.wrapping_mul(2);
        let stopping = |velocity: i32| velocity.wrapping_mul(velocity).checked_div(divisor);
        let cruise = || max_speed.wrapping_mul(max_speed).checked_div(divisor);
        if self.target != target {
            self.target = target;
            if self.velocity > 0 && self.angle < target {
                let stop = stopping(self.velocity)?;
                let delta = target.wrapping_sub(self.angle);
                if stop <= delta {
                    self.accelerating = true;
                    self.midpoint = target.wrapping_add(self.angle).wrapping_sub(stop) / 2;
                    let limit = target.wrapping_sub(cruise()?);
                    if self.midpoint < limit {
                        self.midpoint = limit;
                    }
                } else {
                    self.accelerating = false;
                }
            } else if self.velocity < 0 && self.angle > target {
                let stop = stopping(self.velocity)?;
                let delta = target.wrapping_sub(self.angle);
                if delta >= stop {
                    self.accelerating = true;
                    self.midpoint = self.angle.wrapping_add(stop).wrapping_add(target) / 2;
                    let limit = cruise()?.wrapping_add(target);
                    if self.midpoint > limit {
                        self.midpoint = limit;
                    }
                } else {
                    self.accelerating = false;
                }
            } else {
                self.accelerating = false;
            }
        }
        if self.velocity == 0 {
            let remaining = self.target.wrapping_sub(self.angle);
            if acceleration.wrapping_neg() < remaining && acceleration > remaining {
                self.angle = self.target;
            } else {
                self.accelerating = true;
                let cruise = cruise()?;
                self.midpoint = self.target.wrapping_add(self.angle) / 2;
                if remaining >= 0 {
                    let limit = self.target.wrapping_sub(cruise);
                    self.velocity = acceleration;
                    if limit > self.midpoint {
                        self.midpoint = limit;
                    }
                } else {
                    self.velocity = acceleration.wrapping_neg();
                    let limit = cruise.wrapping_add(self.target);
                    if self.midpoint > limit {
                        self.midpoint = limit;
                    }
                }
            }
        } else if self.velocity > 0 {
            if self.angle >= self.midpoint {
                self.accelerating = false;
            }
            if !self.accelerating {
                self.velocity = self.velocity.wrapping_sub(acceleration);
                if self.velocity < 0 {
                    self.velocity = 0;
                }
            } else if max_speed > self.velocity {
                self.velocity = self.velocity.wrapping_add(acceleration);
            }
        } else {
            if self.midpoint >= self.angle {
                self.accelerating = false;
            }
            if !self.accelerating {
                self.velocity = self.velocity.wrapping_add(acceleration);
                if self.velocity > 0 {
                    self.velocity = 0;
                }
            } else if self.velocity > max_speed.wrapping_neg() {
                self.velocity = self.velocity.wrapping_sub(acceleration);
            }
        }
        self.angle = self.angle.wrapping_add(self.velocity);
        Some(())
    }

    /// The model angle for a non-zero lean: `angle >> 5 & 0x7FF`.
    fn rotation(&self) -> Option<i32> {
        (self.angle != 0).then_some(self.angle >> 5 & 0x7ff)
    }
}

/// What one `applyBodyLean` call does to the body model: a roll about Z and
/// then a pitch about X, each about the model's half height.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Lean {
    pub roll: Option<i32>,
    pub pitch: Option<i32>,
}
impl Lean {
    /// `translate(0, -minY/2, 0)`, rotate, `translate(0, minY/2, 0)` for each
    /// axis; the model's `minY` is read again after the roll, as the original
    /// reads it from the already rolled model.
    pub fn apply(self, model: &mut Model) {
        for (roll, angle) in [(true, self.roll), (false, self.pitch)] {
            let Some(angle) = angle else {
                continue;
            };
            let half = model.vertices.iter().map(|v| v[1]).min().unwrap_or(0) / 2;
            let (sin, cos) = sin_cos(angle);
            for v in &mut model.vertices {
                let [x, y, z] = [v[0], v[1].wrapping_sub(half), v[2]];
                if roll {
                    v[0] = y.wrapping_mul(sin).wrapping_add(x.wrapping_mul(cos)) >> 16;
                    v[1] = y.wrapping_mul(cos).wrapping_sub(x.wrapping_mul(sin)) >> 16;
                } else {
                    v[1] = y.wrapping_mul(cos).wrapping_sub(z.wrapping_mul(sin)) >> 16;
                    v[2] = y.wrapping_mul(sin).wrapping_add(z.wrapping_mul(cos)) >> 16;
                }
                v[1] = v[1].wrapping_add(half);
            }
        }
    }
}

/// The `PathingEntity` fields the three accelerated phases keep.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct State {
    /// `movementSpeed`, in 1/128 fine units per cycle.
    pub movement_speed: i32,
    pub yaw_velocity: i32,
    /// `preciseYaw`: 16-bit yaw, 32 units per angle unit.
    pub precise_yaw: i32,
    pub precise_target_yaw: i32,
    pub yaw_distance: i32,
    pub yaw_decel_threshold: i32,
    pub yaw_accelerating: bool,
    pub roll: Axis,
    pub pitch: Axis,
}
impl State {
    /// `NpcList.processMovement` lines 232–268: `speed` is the plain speed in
    /// fine units, `queue` the movement queue length, `distance` the absolute
    /// fine distance left on each axis. Returns the speed to move by.
    pub fn accelerate(
        &mut self,
        acceleration: i32,
        speed: i32,
        queue: usize,
        distance: [i32; 2],
    ) -> i32 {
        let speed = speed.wrapping_shl(7);
        if queue == 1 {
            let [x, y] = distance.map(|n| n.wrapping_shl(7));
            let squared = self.movement_speed.wrapping_mul(self.movement_speed);
            let furthest = x.max(y);
            let braking = acceleration.wrapping_mul(2).wrapping_mul(furthest);
            if braking < squared {
                self.movement_speed /= 2;
            } else if furthest < squared / 2 {
                self.movement_speed = self.movement_speed.wrapping_sub(acceleration);
                if self.movement_speed < 0 {
                    self.movement_speed = 0;
                }
            } else if self.movement_speed < speed {
                self.movement_speed = self.movement_speed.wrapping_add(acceleration);
                if self.movement_speed > speed {
                    self.movement_speed = speed;
                }
            }
        } else if speed > self.movement_speed {
            self.movement_speed = self.movement_speed.wrapping_add(acceleration);
            if speed < self.movement_speed {
                self.movement_speed = speed;
            }
        } else if self.movement_speed > 0 {
            self.movement_speed = self.movement_speed.wrapping_sub(acceleration);
            if self.movement_speed < 0 {
                self.movement_speed = 0;
            }
        }
        (self.movement_speed >> 7).max(1)
    }

    /// `PathingEntity.applyBodyLean`, called once for each time the body is
    /// drawn. The error is the original's division by a zero acceleration.
    pub fn lean(&mut self, bas: &Bas) -> std::result::Result<Lean, &'static str> {
        if !bas.leans() {
            return Ok(Lean::default());
        }
        let (mut roll, mut pitch) = (0, 0);
        if self.yaw_accelerating && self.yaw_velocity != 0 {
            pitch = bas.pitch_target;
            roll = if self.yaw_velocity >= 0 {
                bas.roll_target
            } else {
                bas.roll_target.wrapping_neg()
            };
        }
        const ZERO: &str = "BAS lean acceleration is zero";
        self.roll
            .step(roll, bas.roll_acceleration, bas.roll_max_speed)
            .ok_or(ZERO)?;
        let roll = self.roll.rotation();
        self.pitch
            .step(pitch, bas.pitch_acceleration, bas.pitch_max_speed)
            .ok_or(ZERO)?;
        Ok(Lean {
            roll,
            pitch: self.pitch.rotation(),
        })
    }
}

impl Movement {
    /// `NpcList.processFacing` lines 380–475: the branch taken while the
    /// angle differs from its target and the BAS has a yaw acceleration.
    /// `preciseYaw` is the only source of the angle here; an angle assigned
    /// elsewhere (forced movement, a spawn) is overwritten from it, as in the
    /// original.
    pub(crate) fn turn_accelerated(&mut self, b: &PlayerBas) {
        let (acceleration, max_speed) = (b.turning.yaw_acceleration, b.turning.yaw_max_speed);
        if b.idle == self.sequence && self.standing_turns > 25 {
            self.sequence = if b.stand[1] == -1 { b.walk } else { b.stand[1] };
        }
        let divisor = acceleration.wrapping_mul(2);
        // `acceleration` is a non-zero unsigned byte here, so `divisor` is not zero.
        let cruise = || {
            max_speed
                .wrapping_mul(max_speed)
                .wrapping_div(divisor)
                .min(32767)
        };
        let s = &mut self.turning;
        let target = self.target_angle.wrapping_shl(5);
        if target != s.precise_target_yaw {
            s.yaw_distance = 0;
            s.precise_target_yaw = target;
            let ahead = target.wrapping_sub(s.precise_yaw) & 0xffff;
            let stop = s
                .yaw_velocity
                .wrapping_mul(s.yaw_velocity)
                .wrapping_div(divisor);
            if s.yaw_velocity > 0 && ahead >= stop && ahead.wrapping_sub(stop) < 32768 {
                s.yaw_decel_threshold = ahead / 2;
                s.yaw_accelerating = true;
                let cruise = cruise();
                if cruise < s.yaw_decel_threshold {
                    s.yaw_decel_threshold = ahead.wrapping_sub(cruise);
                }
            } else if s.yaw_velocity < 0
                && stop <= 65536 - ahead
                && (65536 - ahead).wrapping_sub(stop) < 32768
            {
                s.yaw_decel_threshold = (65536 - ahead) / 2;
                s.yaw_accelerating = true;
                let cruise = cruise();
                if cruise < s.yaw_decel_threshold {
                    s.yaw_decel_threshold = (65536 - ahead).wrapping_sub(cruise);
                }
            } else {
                s.yaw_accelerating = false;
            }
        }
        if s.yaw_velocity == 0 {
            let ahead = s.precise_target_yaw.wrapping_sub(s.precise_yaw) & 0xffff;
            if ahead < acceleration {
                s.precise_yaw = s.precise_target_yaw;
            } else {
                s.yaw_distance = 0;
                let cruise = cruise();
                s.yaw_accelerating = true;
                if ahead >= 32768 {
                    s.yaw_velocity = acceleration.wrapping_neg();
                    s.yaw_decel_threshold = (65536 - ahead) / 2;
                    if cruise < s.yaw_decel_threshold {
                        s.yaw_decel_threshold = (65536 - ahead).wrapping_sub(cruise);
                    }
                } else {
                    s.yaw_velocity = acceleration;
                    s.yaw_decel_threshold = ahead / 2;
                    if cruise < s.yaw_decel_threshold {
                        s.yaw_decel_threshold = ahead.wrapping_sub(cruise);
                    }
                }
            }
        } else if s.yaw_velocity <= 0 {
            if s.yaw_distance >= s.yaw_decel_threshold {
                s.yaw_accelerating = false;
            }
            if !s.yaw_accelerating {
                s.yaw_velocity = s.yaw_velocity.wrapping_add(acceleration);
                if s.yaw_velocity > 0 {
                    s.yaw_velocity = 0;
                }
            } else if max_speed.wrapping_neg() < s.yaw_velocity {
                s.yaw_velocity = s.yaw_velocity.wrapping_sub(acceleration);
            }
        } else {
            if s.yaw_decel_threshold <= s.yaw_distance {
                s.yaw_accelerating = false;
            }
            if !s.yaw_accelerating {
                s.yaw_velocity = s.yaw_velocity.wrapping_sub(acceleration);
                if s.yaw_velocity < 0 {
                    s.yaw_velocity = 0;
                }
            } else if s.yaw_velocity < max_speed {
                s.yaw_velocity = s.yaw_velocity.wrapping_add(acceleration);
            }
        }
        s.precise_yaw = s.precise_yaw.wrapping_add(s.yaw_velocity) & 0xffff;
        if s.yaw_velocity <= 0 {
            s.yaw_distance = s.yaw_distance.wrapping_sub(s.yaw_velocity);
        } else {
            s.yaw_distance = s.yaw_distance.wrapping_add(s.yaw_velocity);
        }
        self.angle = s.precise_yaw >> 5;
    }
}
