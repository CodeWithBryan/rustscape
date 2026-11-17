//! RT4 projectile trajectory and animation clocks (`ProjAnim`), in absolute
//! fine coordinates where the original uses scene-relative ones: differences
//! between coordinates are the same integers, the doubles they are added to
//! are larger.
use crate::animation::Sequence;
use rs09_protocol::game::updates::Projectile;

#[derive(Clone, Debug)]
pub struct Flight {
    pub packet: Projectile,
    pub plane: i32,
    pub start: i32,
    pub end: i32,
    pub destination: [i32; 2],
    pub position: [f64; 3],
    pub yaw: i32,
    /// `ProjAnim.pitch`, in 2048ths of a turn: the model's rotation about X.
    pub pitch: i32,
    pub frame: usize,
    /// `seqNextFrame`: the frame to tween towards, or -1.
    pub next_frame: i32,
    pub clock: i32,
    source: [i32; 3],
    velocity: [f64; 2],
    speed: f64,
    velocity_z: f64,
    acceleration_z: f64,
    moving: bool,
    aimed: bool,
}

impl Flight {
    pub fn new(packet: Projectile, plane: i32, cycle: i32, source: [i32; 2], height: i32) -> Self {
        Self::launch(
            packet,
            plane,
            cycle,
            source,
            [
                source[0] + packet.delta[0] * 128,
                source[1] + packet.delta[1] * 128,
            ],
            height - packet.start_height * 4,
        )
    }
    /// Explicit absolute fine endpoints and launch height, for the half-tile
    /// packets whose source is not a tile centre. `packet.delta` and
    /// `packet.start_height` are not consulted.
    pub fn launch(
        packet: Projectile,
        plane: i32,
        cycle: i32,
        source: [i32; 2],
        destination: [i32; 2],
        z: i32,
    ) -> Self {
        Self {
            packet,
            plane,
            start: cycle.wrapping_add(packet.start),
            end: cycle.wrapping_add(packet.end),
            destination,
            position: [f64::from(source[0]), f64::from(source[1]), f64::from(z)],
            yaw: 0,
            pitch: 0,
            frame: 0,
            next_frame: -1,
            clock: 0,
            source: [source[0], source[1], z],
            velocity: [0.0; 2],
            speed: 0.0,
            velocity_z: 0.0,
            acceleration_z: 0.0,
            moving: false,
            aimed: false,
        }
    }

    /// `ProjAnim.setTarget(targetY, cycle, targetZ, targetX)`: aim at `target`
    /// (fine x, fine y, height), to arrive at the end cycle. Until the first
    /// `update` this also places the projectile `packet.distance` fine units
    /// from its source towards the target.
    pub fn set_target(&mut self, cycle: i32, target: [i32; 3]) {
        let [tx, ty, tz] = target.map(f64::from);
        if !self.moving {
            let dy = f64::from(target[1].wrapping_sub(self.source[1]));
            let dx = f64::from(target[0].wrapping_sub(self.source[0]));
            let distance = (dx * dx + dy * dy).sqrt();
            let offset = f64::from(self.packet.distance);
            self.position[2] = f64::from(self.source[2]);
            self.position[1] = dy * offset / distance + f64::from(self.source[1]);
            self.position[0] = offset * dx / distance + f64::from(self.source[0]);
        }
        let remaining = f64::from(self.end.wrapping_add(1).wrapping_sub(cycle));
        self.velocity[1] = (ty - self.position[1]) / remaining;
        self.velocity[0] = (tx - self.position[0]) / remaining;
        self.speed =
            (self.velocity[1] * self.velocity[1] + self.velocity[0] * self.velocity[0]).sqrt();
        if self.packet.angle == -1 {
            self.velocity_z = (tz - self.position[2]) / remaining;
        } else {
            if !self.moving {
                self.velocity_z = -self.speed * (f64::from(self.packet.angle) * 0.02454369).tan();
            }
            self.acceleration_z = (tz - self.position[2] - self.velocity_z * remaining) * 2.0
                / (remaining * remaining);
        }
    }

    /// `ProjAnim.update(delta)`: move by `delta` cycles, turn to the velocity
    /// and run the spot animation's frame clock, keeping the next frame for
    /// tweening (`-1` when the following frame is outside the sequence).
    pub fn update(&mut self, delta: i32, sequence: Option<&Sequence>) {
        let step = f64::from(delta);
        self.position[0] += self.velocity[0] * step;
        self.position[1] += self.velocity[1] * step;
        self.moving = true;
        if self.packet.angle == -1 {
            self.position[2] += self.velocity_z * step;
        } else {
            self.position[2] += step * self.acceleration_z * 0.5 * step + step * self.velocity_z;
            self.velocity_z += self.acceleration_z * step;
        }
        self.yaw = ((self.velocity[0].atan2(self.velocity[1]) * 325.949) as i32).wrapping_add(1024)
            & 0x7ff;
        self.pitch = ((self.velocity_z.atan2(self.speed) * 325.949) as i32) & 0x7ff;
        let Some(sequence) = sequence else {
            return;
        };
        let length = sequence.frames.len() as i32;
        let replay = sequence.replay_offset.map_or(-1, |n| n as i32);
        self.clock = self.clock.wrapping_add(delta);
        // The original loops for ever on a replay section whose delays are
        // all zero; this stops after a bounded number of frames instead.
        for _ in 0..1 << 16 {
            let Some(&delay) = sequence.delays.get(self.frame) else {
                return;
            };
            if self.clock <= i32::from(delay) {
                return;
            }
            self.clock -= i32::from(delay);
            let mut frame = self.frame as i32 + 1;
            if frame >= length {
                frame -= replay;
                if frame < 0 || length <= frame {
                    frame = 0;
                }
            }
            self.frame = frame as usize;
            self.next_frame = frame + 1;
            if self.next_frame >= length {
                self.next_frame -= replay;
                if self.next_frame < 0 || self.next_frame >= length {
                    self.next_frame = -1;
                }
            }
        }
    }

    /// One frame of `ScriptRunner.updateSceneProjectiles` for a projectile
    /// that is still in the scene. Nothing happens before the start cycle.
    /// `destination_z` is the height the packet aims at over its destination
    /// (terrain there minus the packet's end height): `Protocol` aims the new
    /// projectile at it once, with the start cycle, which is done here on the
    /// first frame instead. `entity` is the tracked actor's fine position and
    /// aim height when it exists and stands inside the scene.
    pub fn step(
        &mut self,
        cycle: i32,
        delta: i32,
        destination_z: i32,
        entity: Option<[i32; 3]>,
        sequence: Option<&Sequence>,
    ) {
        if cycle < self.start {
            return;
        }
        if !self.aimed {
            self.aimed = true;
            self.set_target(
                self.start,
                [self.destination[0], self.destination[1], destination_z],
            );
        }
        if let Some(entity) = entity {
            self.set_target(cycle, entity);
        }
        self.update(delta, sequence);
    }

    /// [`Self::step`] towards a target that is always present.
    pub fn advance(
        &mut self,
        cycle: i32,
        delta: i32,
        target: [i32; 3],
        sequence: Option<&Sequence>,
    ) {
        if delta > 0 {
            self.step(cycle, delta, target[2], Some(target), sequence);
        }
    }
    pub fn visible(&self, cycle: i32) -> bool {
        self.moving && cycle.wrapping_sub(self.start) >= 0 && self.end.wrapping_sub(cycle) >= 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zone_packet_round_trips_and_projectile_lands_at_its_moving_target() {
        let packet = Projectile {
            offset: [3, 6],
            delta: [-4, 2],
            target: 9,
            spot: 15,
            start_height: 10,
            end_height: 7,
            start: 2,
            end: 11,
            angle: 32,
            distance: 16,
        };
        assert_eq!(Projectile::decode(&packet.frame()).unwrap(), packet);
        let mut shot = Flight::new(packet, 0, 100, [3200, 4400], 0);
        shot.advance(101, 1, [2700, 4656, -28], None);
        assert!(!shot.visible(101));
        // One cycle a frame, as the original moves by the cycles since the
        // last frame without regard to the cycles left.
        for cycle in 102..=106 {
            shot.advance(cycle, 1, [2700, 4656, -28], None);
        }
        assert!(shot.visible(106));
        assert!(shot.position[2] < -40.0);
        for cycle in 107..=111 {
            shot.advance(cycle, 1, [2816, 4700, -32], None);
        }
        assert!(
            shot.position
                .into_iter()
                .zip([2816.0, 4700.0, -32.0])
                .all(|(a, b)| (a - b).abs() < 1e-6)
        );
        assert!(!shot.visible(112));
    }
}
