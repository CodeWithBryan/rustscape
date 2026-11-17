//! Original type6 model viewport, transform calls and component animation clock.
use super::Component;
use crate::animation::{Sequence, sin_cos};

/// Selection before geometry lookup. Object widgets and the direct player-body
/// branch keep their primary model fields; partial heads always use modelId.
pub fn select(c: &Component, active: bool) -> Option<Component> {
    let mut selected = c.clone();
    selected.model_seq_id = if active {
        c.active_model_seq_id
    } else {
        c.model_seq_id
    };
    if c.obj_id != -1 || c.model_type == 5 {
        return Some(selected);
    }
    if active {
        selected.model_type = c.active_model_type;
        if selected.model_type != 7 {
            selected.model_id = c.active_model_id;
        }
    }
    if matches!(selected.model_type, 1 | 2 | 3 | 4 | 6 | 7)
        && !(selected.model_type == 1 && selected.model_id == -1)
    {
        Some(selected)
    } else {
        None
    }
}

#[derive(Clone, Debug)]
pub struct Camera {
    /// OpenGL column-major projection, including the original interface depth bias.
    pub projection: [f32; 16],
    /// Original modelview operations: [angle,x,y,z] or [x,y,z] translation.
    pub operations: Vec<Vec<f32>>,
    pub viewport: [i32; 4],
}

fn div(n: i32, d: i32) -> Result<i32, String> {
    if d == 0 {
        Err("Zero UI model viewport scale".into())
    } else {
        Ok(n.wrapping_div(d))
    }
}

impl Camera {
    /// `memory` is the original -itemModel.getMinY()/2; other model kinds pass0.
    pub fn new(
        c: &Component,
        position: [i32; 2],
        canvas: [i32; 2],
        memory: i32,
    ) -> Result<Self, String> {
        let sx = if c.model_viewport_width > 0 {
            div(c.width.wrapping_shl(8), c.model_viewport_width)?
        } else {
            256
        };
        let sy = if c.model_viewport_height > 0 {
            div(c.height.wrapping_shl(8), c.model_viewport_height)?
        } else {
            256
        };
        let cx = position[0]
            .wrapping_add(c.width / 2)
            .wrapping_add(sx.wrapping_mul(c.model_origin_x) >> 8);
        let cy = position[1]
            .wrapping_add(c.height / 2)
            .wrapping_add(sy.wrapping_mul(c.model_origin_y) >> 8);
        if sx == 0 || sy == 0 {
            return Err("Zero UI model viewport scale".into());
        }
        let mut projection = [0.0; 16];
        if c.model_ortho {
            let scale = c.model_zoom as f32 / 512.0;
            let ox = scale * (256.0 / sx as f32);
            let oy = scale * (256.0 / sy as f32);
            // GL receives the source's float products as doubles.
            let l = (cx.wrapping_neg() as f32 * ox) as f64;
            let r = (canvas[0].wrapping_sub(cx) as f32 * ox) as f64;
            let b = (canvas[1].wrapping_sub(cy).wrapping_neg() as f32 * oy) as f64;
            let t = (cy as f32 * oy) as f64;
            let depth = c.model_view_angle as i16 as i32;
            let near = 50i32.wrapping_sub(depth) as f64;
            let far = 7168i32.wrapping_sub(depth) as f64;
            projection[0] = (2.0 / (r - l)) as f32;
            projection[5] = (2.0 / (t - b)) as f32;
            projection[10] = (-2.0 / (far - near)) as f32;
            projection[12] = (-(r + l) / (r - l)) as f32;
            projection[13] = (-(t + b) / (t - b)) as f32;
            projection[14] = (-(far + near) / (far - near)) as f32;
            projection[15] = 1.0;
        } else {
            let l = div(cx.wrapping_neg().wrapping_shl(8), sx)? as f32 * 0.09765625;
            let r = div(canvas[0].wrapping_sub(cx).wrapping_shl(8), sx)? as f32 * 0.09765625;
            let top = div(cy.wrapping_neg().wrapping_shl(8), sy)?;
            let bottom = div(canvas[1].wrapping_sub(cy).wrapping_shl(8), sy)?;
            let b = bottom.wrapping_neg() as f32 * 0.09765625;
            let t = top.wrapping_neg() as f32 * 0.09765625;
            projection[0] = 100.0 / (r - l);
            projection[5] = 100.0 / (t - b);
            projection[8] = (r + l) / (r - l);
            projection[9] = (t + b) / (t - b);
            projection[10] = -(7168.0 + 50.0) / (7168.0 - 50.0);
            projection[11] = -1.0;
            projection[14] = -(100.0 * 7168.0) / (7168.0 - 50.0);
            let distance = (c.model_near_clip as i16) as f32;
            let bias = (c.model_view_angle as i16) as f32 * 1.5;
            if bias != 0.0 {
                let scale = distance / (bias + distance);
                let square = scale * scale;
                let adjustment = -projection[14] * (1.0 - scale) * (1.0 - scale) / bias;
                projection[10] += adjustment;
                projection[14] *= square;
            }
        }
        if !(0..2048).contains(&c.model_x_angle) {
            return Err("UI model pitch outside trig table".into());
        }
        let (sin, cos) = sin_cos(c.model_x_angle);
        let y = sin.wrapping_mul(c.model_zoom) >> 16;
        let z = cos.wrapping_mul(c.model_zoom) >> 16;
        let [roll, pitch_x, offset_y, offset_z] = if c.if3 {
            [
                c.model_y_offset,
                c.model_x_offset,
                c.model_z_offset.wrapping_add(y).wrapping_add(memory),
                c.model_z_offset.wrapping_add(z),
            ]
        } else {
            [0, 0, y, z]
        };
        let mut operations = vec![vec![180.0, 1.0, 0.0, 0.0]];
        if c.model_x_angle != 0 {
            operations.push(vec![c.model_x_angle as f32 * 0.17578125, 1.0, 0.0, 0.0]);
        }
        operations.push(vec![pitch_x as f32, offset_y as f32, offset_z as f32]);
        if c.model_y_angle != 0 {
            operations.push(vec![c.model_y_angle as f32 * 0.17578125, 0.0, 1.0, 0.0]);
        }
        if roll != 0 {
            operations.push(vec![roll.wrapping_neg() as f32 * 0.17578125, 0.0, 0.0, 1.0]);
        }
        Ok(Self {
            projection,
            operations,
            viewport: [0, 0, canvas[0], canvas[1]],
        })
    }
}

/// One component visit of InterfaceList.updateAnimationsRecursive. The caller
/// supplies original tree visibility/CS1 truth and the selected sequence lookup.
/// Zero-delay sequences are bounded instead of allowing the reference's loop to hang.
pub fn advance(
    c: &mut Component,
    active: bool,
    sequence: Option<&Sequence>,
    delta: i32,
) -> Result<u32, String> {
    if c.kind != 6 || (c.if3 && c.hidden) {
        return Ok(0);
    }
    let mut redraws = 0u32;
    if c.model_seq_id != -1 || c.active_model_seq_id != -1 {
        let id = if active {
            c.active_model_seq_id
        } else {
            c.model_seq_id
        };
        if let Some(seq) = sequence.filter(|_| id != -1) {
            if seq.frames.is_empty()
                || seq.frames.len() != seq.delays.len()
                || seq.delays.contains(&0)
            {
                return Err("Invalid UI model sequence".into());
            }
            let replay = seq.replay_offset.map_or(-1, |n| n as i32);
            c.seq_cycle = c.seq_cycle.wrapping_add(delta);
            loop {
                let delay = *seq
                    .delays
                    .get(c.seq_frame as usize)
                    .ok_or("UI model frame outside sequence")? as i32;
                if c.seq_cycle <= delay {
                    break;
                }
                c.seq_cycle = c.seq_cycle.wrapping_sub(delay);
                c.seq_frame = c.seq_frame.wrapping_add(1);
                if c.seq_frame >= seq.frames.len() as i32 {
                    c.seq_frame = c.seq_frame.wrapping_sub(replay);
                    if c.seq_frame < 0 || c.seq_frame >= seq.frames.len() as i32 {
                        c.seq_frame = 0;
                    }
                }
                c.seq_next_frame = c.seq_frame.wrapping_add(1);
                if c.seq_next_frame >= seq.frames.len() as i32 {
                    c.seq_next_frame = c.seq_next_frame.wrapping_sub(replay);
                    if c.seq_next_frame < 0 || c.seq_next_frame >= seq.frames.len() as i32 {
                        c.seq_next_frame = -1;
                    }
                }
                redraws = redraws.wrapping_add(1);
            }
        }
    }
    if c.model_rotation_speed != 0 && !c.if3 {
        let dx = (c.model_rotation_speed >> 16).wrapping_mul(delta);
        let dy = (c.model_rotation_speed as i16 as i32).wrapping_mul(delta);
        c.model_x_angle = c.model_x_angle.wrapping_add(dx) & 2047;
        c.model_y_angle = c.model_y_angle.wrapping_add(dy) & 2047;
        redraws = redraws.wrapping_add(1);
    }
    Ok(redraws)
}
