//! Default-player PathingEntity queue commands and NpcList motion/facing phases.
//! Coordinates are global here; origin conversion preserves original local arithmetic.
use crate::{
    actor_animation::State,
    animation::Sequence,
    walking::{Movement, PlayerBas},
};
use rs09_protocol::{Error, Result};
use std::collections::BTreeMap;

fn sequence(sequences: &BTreeMap<u32, Sequence>, id: i32) -> Result<&Sequence> {
    sequences
        .get(&(id as u32))
        .ok_or(Error::new(0, "actor motion sequence unavailable"))
}

/// Explicit services used by the original per-entity movement phase.
pub struct Context<'a> {
    pub bas: &'a PlayerBas,
    pub size: i32,
    pub origin: [i32; 2],
    pub is_self: bool,
    pub cycle: i32,
    /// Resolve faceEntity using positions at this entity's turn in local-list order.
    pub face_target: Option<[i32; 2]>,
    pub sequences: &'a BTreeMap<u32, Sequence>,
}

impl Movement {
    pub fn cancel_on_move(
        &self,
        state: &mut State,
        sequences: &BTreeMap<u32, Sequence>,
    ) -> Result<()> {
        // A sequence that is not to be had is `SeqTypeList.get`'s new type
        // with nothing decoded, whose `movetype` is -1 (`SeqType.java` 39):
        // the action stays. An NPC update does not end over it.
        let move_type = sequences
            .get(&(state.action.id as u32))
            .map_or(-1, |s| s.move_type);
        if state.action.id != -1 && move_type == 1 {
            state.action.id = -1;
        }
        Ok(())
    }
    pub fn teleport_actor(&mut self, tile: [i32; 2], discard: bool, size: i32, state: &mut State) {
        let delta = [
            tile[0].wrapping_sub(self.head[0]),
            tile[1].wrapping_sub(self.head[1]),
        ];
        if !discard && delta.into_iter().all(|n| (-8..=8).contains(&n)) {
            self.enqueue(tile, 1);
            return;
        }
        self.catchup = 0;
        self.head = tile;
        self.queue.clear();
        state.movement_steps = 0;
        self.fine = tile.map(|n| n.wrapping_mul(128).wrapping_add(size.wrapping_mul(64)));
    }
    /// The mask overwrites queue size with one without changing its head or speed.
    pub fn force_actor(
        &mut self,
        start: [i32; 2],
        destination: [i32; 2],
        cycles: [i32; 2],
        direction: i32,
        state: &mut State,
    ) {
        self.force_start_tile = start;
        self.force_destination = destination;
        self.force_direction = direction;
        state.force_start = cycles[0];
        state.force_end = cycles[1];
        self.queue.clear();
        self.queue.push_back((self.head, self.head_speed));
        state.movement_steps = 0;
    }
    pub fn advance_actor(&mut self, state: &mut State, context: Context<'_>) -> Result<()> {
        let Context {
            bas,
            size,
            origin,
            is_self,
            cycle,
            face_target,
            sequences,
        } = context;
        let offset = origin.map(|n| n.wrapping_mul(128));
        let local = |fine: [i32; 2]| {
            [
                fine[0].wrapping_sub(offset[0]),
                fine[1].wrapping_sub(offset[1]),
            ]
        };
        let center =
            |tile: [i32; 2]| tile.map(|n| n.wrapping_mul(128).wrapping_add(size.wrapping_mul(64)));
        let forced = cycle < state.force_start || state.force_end >= cycle;
        if cycle < state.force_start {
            let left = state.force_start.wrapping_sub(cycle);
            let start = center(self.force_start_tile);
            for (axis, target) in start.into_iter().enumerate() {
                self.fine[axis] = self.fine[axis]
                    .wrapping_add(target.wrapping_sub(self.fine[axis]).wrapping_div(left));
            }
            self.catchup = 0;
        } else if state.force_end >= cycle {
            let action = &state.action;
            let interpolate = cycle == state.force_end || action.id == -1 || action.delay != 0 || {
                let seq = sequence(sequences, action.id)?;
                let delay = seq
                    .delays
                    .get(action.frame as usize)
                    .ok_or(Error::new(0, "forced movement action frame unavailable"))?;
                action.clock.wrapping_add(1) > i32::from(*delay)
            };
            if interpolate {
                let total = state.force_end.wrapping_sub(state.force_start);
                if total == 0 {
                    return Err(Error::new(0, "zero-duration forced movement"));
                }
                let elapsed = cycle.wrapping_sub(state.force_start);
                let start = local(center(self.force_start_tile));
                let dest = local(center(self.force_destination));
                for axis in 0..2 {
                    self.fine[axis] = elapsed
                        .wrapping_mul(dest[axis])
                        .wrapping_add(start[axis].wrapping_mul(total.wrapping_sub(elapsed)))
                        .wrapping_div(total)
                        .wrapping_add(offset[axis]);
                }
            }
            self.catchup = 0;
        } else {
            let blocked = state.action.id != -1 && state.action.delay == 0 && {
                let seq = sequence(sequences, state.action.id)?;
                if state.movement_steps > 0 {
                    seq.loop_type == 0
                } else {
                    seq.move_type == 0
                }
            };
            self.advance_path(bas, size, blocked, &mut state.movement_steps);
        }
        if forced {
            if let Some(angle) = [1024, 1536, 0, 512].get(self.force_direction as usize) {
                self.target_angle = *angle;
            }
            if cycle >= state.force_start {
                self.angle = self.target_angle;
            }
        }
        let fine = local(self.fine);
        if fine.into_iter().any(|n| !(128..13184).contains(&n))
            || is_self && fine.into_iter().any(|n| !(1536..11776).contains(&n))
        {
            state.action.id = -1;
            state.spot.id = -1;
            state.force_start = 0;
            state.force_end = 0;
            self.fine = center(self.head);
            self.queue.clear();
            state.movement_steps = 0;
        }
        self.advance_facing(bas, face_target);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! Transcription test of `SeqTypeList.get` for an id without data: a new
    //! `SeqType` whose `movetype` is -1 (`SeqType.java` 39).
    use super::*;

    #[test]
    fn a_sequence_not_to_be_had_cancels_nothing_and_fails_nothing() {
        let motion = Movement::new([3222, 3218]);
        let mut state = State::default();
        state.action.id = 4711;
        motion.cancel_on_move(&mut state, &BTreeMap::new()).unwrap();
        assert_eq!(state.action.id, 4711, "movetype -1 keeps the action");
        // A known sequence with movetype 1 is cancelled, as before.
        let walk_cancelled = Sequence {
            move_type: 1,
            ..Sequence::default()
        };
        let sequences = BTreeMap::from([(4711, walk_cancelled)]);
        motion.cancel_on_move(&mut state, &sequences).unwrap();
        assert_eq!(state.action.id, -1);
    }
}
