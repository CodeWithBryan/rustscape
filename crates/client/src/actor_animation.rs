//! Original player animation requests and NpcList.processAnimations clocks.
//! Motion/facing is joined by actor_motion; model construction and playback are bindings.
use crate::animation::Sequence;
use rs09_protocol::{Error, Result};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Track {
    pub id: i32,
    pub frame: i32,
    pub next: i32,
    pub clock: i32,
    pub delay: i32,
    pub repeats: i32,
}
impl Default for Track {
    fn default() -> Self {
        Self {
            id: -1,
            frame: 0,
            next: -1,
            clock: 0,
            delay: 0,
            repeats: 0,
        }
    }
}
impl Track {
    pub fn cursor(&self, seq: &Sequence) -> Option<(usize, Option<usize>, i32, i32)> {
        let frame = usize::try_from(self.frame).ok()?;
        Some((
            frame,
            usize::try_from(self.next)
                .ok()
                .filter(|&n| n < seq.frames.len()),
            self.clock.wrapping_sub(1),
            i32::from(*seq.delays.get(frame)?),
        ))
    }
}
#[derive(Clone, Debug, Default)]
pub struct State {
    pub movement: Track,
    pub action: Track,
    pub spot: Track,
    pub spot_start: i32,
    pub spot_height: i32,
    pub slots: [Option<Track>; 12],
    pub movement_steps: i32,
    pub force_start: i32,
    pub force_end: i32,
    pub stretches: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sound {
    pub sequence: i32,
    pub frame: i32,
}
fn sound(events: &mut Vec<Sound>, id: i32, frame: i32) {
    events.push(Sound {
        sequence: id,
        frame,
    });
}
fn get(sequences: &BTreeMap<u32, Sequence>, id: i32) -> Result<&Sequence> {
    sequences
        .get(&(id as u32))
        .ok_or(Error::new(0, "actor sequence asset unavailable"))
}
fn valid(seq: &Sequence) -> bool {
    !seq.frames.is_empty() && seq.frames.len() == seq.delays.len()
}
impl State {
    pub fn request(
        &mut self,
        id: i32,
        delay: i32,
        queue_size: i32,
        sequences: &BTreeMap<u32, Sequence>,
    ) -> Result<Vec<Sound>> {
        let mut events = vec![];
        if id == self.action.id && id != -1 {
            match get(sequences, id)?.exact_move {
                1 => {
                    self.action = Track {
                        id,
                        frame: 0,
                        next: 1,
                        clock: 0,
                        delay,
                        repeats: 0,
                    };
                    sound(&mut events, id, 0);
                }
                2 => self.action.repeats = 0,
                _ => {}
            }
        } else if id == -1
            || self.action.id == -1
            || get(sequences, id)?.priority >= get(sequences, self.action.id)?.priority
        {
            self.action = Track {
                id,
                frame: 0,
                next: 1,
                clock: 0,
                delay,
                repeats: 0,
            };
            self.movement_steps = queue_size;
            if id != -1 {
                get(sequences, id)?;
                sound(&mut events, id, 0);
            }
        }
        Ok(events)
    }
    pub fn request_slots(
        &mut self,
        requests: &[(i32, i32, i32)],
        sequences: &BTreeMap<u32, Sequence>,
    ) -> Result<Vec<Sound>> {
        let mut events = vec![];
        for &(id, delay, mask) in requests {
            // The slot number is also the mask bit.
            #[allow(clippy::needless_range_loop)]
            for slot in 0..12 {
                if (mask as u32) & (1 << slot) == 0 {
                    continue;
                }
                if id == -1 {
                    self.slots[slot] = None;
                    continue;
                }
                let seq = get(sequences, id)?;
                if let Some(track) = &mut self.slots[slot] {
                    if track.id == id {
                        match seq.exact_move {
                            0 => self.slots[slot] = None,
                            1 => {
                                *track = Track {
                                    id,
                                    frame: 0,
                                    next: 1,
                                    clock: 0,
                                    delay,
                                    repeats: 0,
                                };
                                sound(&mut events, id, 0);
                            }
                            2 => track.repeats = 0,
                            _ => {}
                        }
                    } else if seq.priority >= get(sequences, track.id)?.priority {
                        self.slots[slot] = None;
                    }
                }
                if self.slots[slot].is_none() {
                    self.slots[slot] = Some(Track {
                        id,
                        frame: 0,
                        next: 1,
                        clock: 0,
                        delay,
                        repeats: 0,
                    });
                    sound(&mut events, id, 0);
                }
            }
        }
        Ok(events)
    }
    /// Spot IDs resolve through original SpotAnimType definitions, not sequence IDs.
    pub fn request_spot(
        &mut self,
        id: i32,
        packed: i32,
        cycle: i32,
        spots: &BTreeMap<i32, i32>,
        sequences: &BTreeMap<u32, Sequence>,
    ) -> Result<Vec<Sound>> {
        self.request_spot_at(id, packed >> 16, packed & 65535, cycle, spots, sequences)
    }
    /// RESET_ANIMS clears only the action sequence ID; its clocks are untouched.
    pub fn reset_action(&mut self) {
        self.action.id = -1;
    }
    /// SPOTANIM_ENTITY sends the height and delay as separate unsigned fields;
    /// the actor mask packs the same two values into one word.
    pub fn request_spot_at(
        &mut self,
        id: i32,
        height: i32,
        delay: i32,
        cycle: i32,
        spots: &BTreeMap<i32, i32>,
        sequences: &BTreeMap<u32, Sequence>,
    ) -> Result<Vec<Sound>> {
        let sequence = |id| {
            spots
                .get(&id)
                .copied()
                .ok_or(Error::new(0, "spot animation asset unavailable"))
        };
        let mut events = vec![];
        let priority = |id: i32| -> Result<i32> {
            if id == -1 {
                Ok(5)
            } else {
                Ok(get(sequences, id)?.priority)
            }
        };
        if id == -1
            || self.spot.id == -1
            || priority(sequence(id)?)? >= priority(sequence(self.spot.id)?)?
        {
            self.spot_start = delay.wrapping_add(cycle);
            self.spot_height = height;
            self.spot = Track {
                id,
                frame: if self.spot_start > cycle { -1 } else { 0 },
                next: 1,
                ..Track::default()
            };
            if id != -1 && self.spot_start == cycle {
                let seq = sequence(id)?;
                if seq != -1 && valid(get(sequences, seq)?) {
                    sound(&mut events, seq, 0);
                }
            }
        }
        Ok(events)
    }
    pub fn tick(
        &mut self,
        cycle: i32,
        sequences: &BTreeMap<u32, Sequence>,
        spots: &BTreeMap<i32, i32>,
    ) -> Result<Vec<Sound>> {
        let mut events = vec![];
        self.stretches = false;
        if self.movement.id != -1 {
            if let Some(seq) = sequences
                .get(&(self.movement.id as u32))
                .filter(|s| valid(s))
            {
                let t = &mut self.movement;
                t.clock = t.clock.wrapping_add(1);
                if t.frame >= 0
                    && (t.frame as usize) < seq.frames.len()
                    && t.clock > i32::from(seq.delays[t.frame as usize])
                {
                    t.clock = 1;
                    t.frame += 1;
                    t.next += 1;
                    sound(&mut events, t.id, t.frame);
                }
                if t.frame as usize >= seq.frames.len() {
                    t.frame = 0;
                    t.clock = 0;
                    sound(&mut events, t.id, 0);
                }
                t.next = t.frame + 1;
                if t.next as usize >= seq.frames.len() {
                    t.next = 0;
                }
            } else {
                self.movement.id = -1;
            }
        }
        if self.spot.id != -1 && cycle >= self.spot_start {
            let id = *spots
                .get(&self.spot.id)
                .ok_or(Error::new(0, "spot animation asset unavailable"))?;
            if let Some(seq) = sequences.get(&(id as u32)).filter(|s| valid(s)) {
                let t = &mut self.spot;
                if t.frame < 0 {
                    t.frame = 0;
                    sound(&mut events, id, 0);
                }
                t.clock = t.clock.wrapping_add(1);
                if (t.frame as usize) < seq.frames.len()
                    && i32::from(seq.delays[t.frame as usize]) < t.clock
                {
                    t.frame += 1;
                    t.clock = 1;
                    sound(&mut events, id, t.frame);
                }
                if t.frame as usize >= seq.frames.len() {
                    t.id = -1;
                }
                t.next = t.frame + 1;
                if t.next as usize >= seq.frames.len() {
                    t.next = -1;
                }
            } else {
                self.spot.id = -1;
            }
        }
        if self.action.id != -1 && self.action.delay <= 1 {
            let seq = get(sequences, self.action.id)?;
            if seq.loop_type == 1
                && self.movement_steps > 0
                && cycle >= self.force_start
                && cycle > self.force_end
            {
                self.action.delay = 1;
                return Ok(events);
            }
        }
        if self.action.id != -1 && self.action.delay == 0 {
            if let Some(seq) = sequences.get(&(self.action.id as u32)).filter(|s| valid(s)) {
                advance_action(&mut self.action, seq, &mut events);
                self.stretches = seq.stretches;
            } else {
                self.action.id = -1;
            }
        }
        if self.action.delay > 0 {
            self.action.delay -= 1;
        }
        for slot in &mut self.slots {
            let Some(t) = slot else {
                continue;
            };
            if t.delay > 0 {
                t.delay -= 1;
                continue;
            }
            if let Some(seq) = sequences.get(&(t.id as u32)).filter(|s| valid(s)) {
                advance_action(t, seq, &mut events);
                if t.id == -1 {
                    *slot = None;
                }
            } else {
                *slot = None;
            }
        }
        Ok(events)
    }
}
fn advance_action(t: &mut Track, seq: &Sequence, events: &mut Vec<Sound>) {
    let id = t.id;
    t.clock = t.clock.wrapping_add(1);
    if t.frame >= 0
        && (t.frame as usize) < seq.frames.len()
        && t.clock > i32::from(seq.delays[t.frame as usize])
    {
        t.clock = 1;
        t.frame += 1;
        sound(events, id, t.frame);
    }
    let replay = seq.replay_offset.map_or(-1, |n| n as i32);
    if t.frame as usize >= seq.frames.len() {
        t.frame = t.frame.wrapping_sub(replay);
        t.repeats = t.repeats.wrapping_add(1);
        if t.repeats >= seq.replay_count || t.frame < 0 || t.frame as usize >= seq.frames.len() {
            t.id = -1;
        } else {
            sound(events, id, t.frame);
        }
    }
    t.next = t.frame + 1;
    if t.next as usize >= seq.frames.len() {
        t.next = t.next.wrapping_sub(replay);
        if seq.replay_count <= t.repeats + 1 || t.next < 0 || t.next as usize >= seq.frames.len() {
            t.next = -1;
        }
    }
}
