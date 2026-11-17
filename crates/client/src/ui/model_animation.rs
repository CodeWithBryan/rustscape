//! SeqType.animateEntity's original frameset selection and ordered animation passes.
use crate::animation::Sequence;

pub trait Framesets {
    fn ready(&self, group: i32) -> bool;
    fn alpha(&self, group: i32, frame: i32) -> bool;
    fn color(&self, group: i32, frame: i32) -> bool;
}
#[derive(Clone, Debug)]
pub struct Step {
    pub group: i32,
    pub frame: i32,
    pub next_group: Option<i32>,
    pub next_frame: i32,
    pub delta: i32,
    pub length: i32,
    pub normals: bool,
}
#[derive(Clone, Debug)]
pub struct Plan {
    pub copy: [bool; 3],
    pub steps: Vec<Step>,
}

pub fn plan(
    seq: &Sequence,
    frame: i32,
    mut next: i32,
    cycle: i32,
    tween: bool,
    host: &impl Framesets,
) -> Result<Plan, String> {
    let frame_id = *seq
        .frames
        .get(frame as usize)
        .ok_or("UI animation frame index")?;
    let length = i32::from(
        *seq.delays
            .get(frame as usize)
            .ok_or("UI animation frame delay")?,
    );
    let group = (frame_id as i32) >> 16;
    let index = (frame_id & 65535) as i32;
    if !host.ready(group) {
        return Ok(Plan {
            copy: [true; 3],
            steps: Vec::new(),
        });
    }
    let tween = tween || seq.tween;
    let mut next_group = None;
    if tween && next != -1 && next < seq.frames.len() as i32 {
        let id = *seq
            .frames
            .get(next as usize)
            .ok_or("UI animation next index")?;
        let g = (id as i32) >> 16;
        if host.ready(g) {
            next_group = Some(g);
        }
        next = (id & 65535) as i32;
    }
    let mut second_group = None;
    let mut second_index = 0;
    let mut second_next = None;
    let mut second_next_index = 0;
    if let Some(&id) = seq.secondary_frames.get(frame as usize) {
        second_index = id as i32;
        if id != 65535 {
            let g = (id as i32) >> 16;
            if host.ready(g) {
                second_group = Some(g);
            }
            second_index = (id & 65535) as i32;
        }
    }
    // The reference reuses the rewritten low frame index here, rather than the
    // original sequence slot. Preserve that quirk for dialogue face overlays.
    if let Some(&id) = seq
        .secondary_frames
        .get(next as usize)
        .filter(|_| tween && next != -1)
    {
        second_next_index = id as i32;
        if id != 65535 {
            let g = (id as i32) >> 16;
            if host.ready(g) {
                second_next = Some(g);
            }
            second_next_index = (id & 65535) as i32;
        }
    }
    let mut no_alpha = !host.alpha(group, index);
    let mut no_color = !host.color(group, index);
    for (g, f) in [
        (second_group, second_index),
        (next_group, next),
        (second_next, second_next_index),
    ] {
        if let Some(g) = g {
            no_alpha &= !host.alpha(g, f);
            no_color &= !host.color(g, f);
        }
    }
    let mut steps = vec![Step {
        group,
        frame: index,
        next_group,
        next_frame: next,
        delta: cycle.wrapping_sub(1),
        length,
        normals: seq.transform_normals,
    }];
    if let Some(group) = second_group {
        steps.push(Step {
            group,
            frame: second_index,
            next_group: second_next,
            next_frame: second_next_index,
            delta: cycle.wrapping_sub(1),
            length,
            normals: seq.transform_normals,
        });
    }
    Ok(Plan {
        copy: [no_alpha, no_color, !seq.transform_normals],
        steps,
    })
}
