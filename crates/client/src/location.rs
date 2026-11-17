//! RT4 scenery animation clocks and GL location orientation transforms.
use crate::{
    animation::{Base, Frame, Sequence, animate_hd},
    model::Model,
};

#[derive(Clone, Debug, serde::Deserialize)]
pub struct Definition {
    pub models: Vec<u32>,
    pub shapes: Option<Vec<i32>>,
    pub width: i32,
    pub length: i32,
    pub resize: [i32; 3],
    pub offset: [i32; 3],
    pub mirror: bool,
    pub recolor: Vec<[u16; 2]>,
    #[serde(default)]
    pub retexture: Vec<[u16; 2]>,
    #[serde(default)]
    pub ambient: i32,
    #[serde(default)]
    pub contrast: i32,
    pub hill: bool,
    pub animation: i32,
    #[serde(default = "enabled")]
    pub randomized_animation: bool,
    #[serde(default = "enabled")]
    pub cast_shadow: bool,
}

fn enabled() -> bool {
    true
}

impl Default for Definition {
    fn default() -> Self {
        Self {
            models: vec![],
            shapes: None,
            width: 1,
            length: 1,
            resize: [128; 3],
            offset: [0; 3],
            mirror: false,
            recolor: vec![],
            retexture: vec![],
            ambient: 0,
            contrast: 0,
            hill: false,
            animation: -1,
            randomized_animation: true,
            cast_shadow: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Animation {
    pub frame: usize,
    pub next_frame: Option<usize>,
    pub progress: i32,
    pub start_loop: i32,
    pub active: bool,
}

impl Animation {
    /// The two random inputs replace Loc's Math.random calls, in their original order.
    /// Invalid zero-length delays are rejected instead of allowing an infinite loop.
    pub fn new(seq: &Sequence, loop_tick: i32, random: Option<[f64; 2]>) -> Option<Self> {
        if seq.frames.is_empty() || seq.frames.len() != seq.delays.len() || seq.delays.contains(&0)
        {
            return None;
        }
        let mut state = Self {
            frame: 0,
            next_frame: Some(usize::from(seq.frames.len() > 1)),
            progress: 1,
            start_loop: loop_tick.wrapping_sub(1),
            active: true,
        };
        if let Some([frame, progress]) = random.filter(|_| seq.replay_offset.is_some()) {
            if !(0.0..1.0).contains(&frame) || !(0.0..1.0).contains(&progress) {
                return None;
            }
            state.frame = (frame * seq.frames.len() as f64) as usize;
            state.next_frame = state.following(seq);
            state.progress = (progress * f64::from(seq.delays[state.frame])) as i32 + 1;
            state.start_loop = loop_tick.wrapping_sub(state.progress);
        }
        Some(state)
    }

    fn following(&self, seq: &Sequence) -> Option<usize> {
        let mut next = self.frame as i32 + 1;
        if next >= seq.frames.len() as i32 {
            next -= seq.replay_offset.map_or(-1, |n| n as i32);
        }
        (0..seq.frames.len() as i32)
            .contains(&next)
            .then_some(next as usize)
    }

    /// Advances on visible-scene visits, including RT4's long-absence optimization.
    /// Returns the frame sound triggers in source order, before advancing the frame.
    pub fn advance(&mut self, seq: &Sequence, loop_tick: i32) -> Vec<usize> {
        let mut sounds = Vec::new();
        if !self.active {
            return sounds;
        }
        let mut elapsed = loop_tick.wrapping_sub(self.start_loop);
        let replay = seq.replay_offset.map_or(-1, |n| n as i32);
        if elapsed > 100 && replay > 0 {
            let replay_start = seq.frames.len() as i32 - replay;
            while (self.frame as i32) < replay_start && i32::from(seq.delays[self.frame]) < elapsed
            {
                elapsed -= i32::from(seq.delays[self.frame]);
                self.frame += 1;
            }
            if self.frame as i32 >= replay_start {
                // Invalid replay offsets cannot index the original loop safely either.
                if replay_start < 0 {
                    self.active = false;
                    return sounds;
                }
                let duration: i32 = seq.delays[replay_start as usize..]
                    .iter()
                    .map(|&n| i32::from(n))
                    .sum();
                elapsed %= duration;
            }
            self.next_frame = self.following(seq);
        }
        while elapsed > i32::from(seq.delays[self.frame]) {
            sounds.push(self.frame);
            elapsed -= i32::from(seq.delays[self.frame]);
            let mut frame = self.frame as i32 + 1;
            if frame >= seq.frames.len() as i32 {
                frame -= replay;
                if !(0..seq.frames.len() as i32).contains(&frame) {
                    // RT4 retains the previous tween index when the sequence expires.
                    self.frame = frame as usize;
                    self.active = false;
                    break;
                }
            }
            self.frame = frame as usize;
            self.next_frame = self.following(seq);
        }
        self.progress = elapsed;
        self.start_loop = loop_tick.wrapping_sub(elapsed);
        sounds
    }
}

/// Integer 90-degree rotations. GL normal components retain their short casts.
pub fn rotate(model: &mut Model, quadrant: i32, normals: bool) {
    fn rotated(x: i32, z: i32, quadrant: i32) -> (i32, i32) {
        match quadrant & 3 {
            1 => (z, x.wrapping_neg()),
            2 => (x.wrapping_neg(), z.wrapping_neg()),
            3 => (z.wrapping_neg(), x),
            _ => (x, z),
        }
    }
    for v in &mut model.vertices {
        (v[0], v[2]) = rotated(v[0], v[2], quadrant);
    }
    if normals {
        for s in &mut model.surfaces {
            for n in &mut s.normals {
                let (x, z) = rotated(n[0], n[2], quadrant);
                n[0] = i32::from(x as i16);
                n[2] = i32::from(z as i16);
            }
        }
    }
}

/// The cached GL model already includes its rotation, scaling and translation.
/// SeqType.animateLocGl undoes only the quadrant, animates, then restores it.
#[allow(clippy::too_many_arguments)]
pub fn animate(
    model: &Model,
    rotation: i32,
    seq: &Sequence,
    base: &Base,
    frame: &Frame,
    next: Option<&Frame>,
    progress: i32,
    length: i32,
) -> Model {
    let mut pose = model.clone();
    rotate(&mut pose, -rotation, seq.transform_normals);
    pose = animate_hd(
        &pose,
        base,
        frame,
        next,
        progress.wrapping_sub(1),
        length,
        seq.transform_normals,
    );
    rotate(&mut pose, rotation, seq.transform_normals);
    pose
}
