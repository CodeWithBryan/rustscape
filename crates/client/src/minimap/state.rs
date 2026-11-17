use rs09_protocol::{Result, buffer::Reader, game::Frame};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Hint {
    pub kind: i32,
    pub sprite: usize,
    pub fine: [i32; 2],
    pub tile: [i32; 2],
    pub height: i32,
    pub actor: i32,
    pub model: i32,
}
impl Default for Hint {
    fn default() -> Self {
        Self {
            kind: 0,
            sprite: 0,
            fine: [0; 2],
            tile: [0; 2],
            height: 0,
            actor: 0,
            model: -1,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct State {
    pub state: i32,
    pub hints: [Option<Hint>; 4],
    pub head_hint_count: usize,
}
impl State {
    /// Original state192 and four-slot hint217 reducer, with bounded malformed reads.
    pub fn apply(&mut self, frame: &Frame) -> Result<bool> {
        let mut r = Reader::new(&frame.payload);
        match frame.opcode {
            192 => self.state = r.g1()?,
            217 => {
                let flags = r.g1()?;
                let sprite = r.g1()? as usize;
                if sprite < self.head_hint_count {
                    let mut hint = Hint {
                        kind: flags & 63,
                        sprite,
                        ..Default::default()
                    };
                    if hint.kind == 1 || hint.kind == 10 {
                        hint.actor = r.g2()?;
                        r.take(3)?;
                    } else if (2..=6).contains(&hint.kind) {
                        hint.fine = match hint.kind {
                            2 => [64, 64],
                            3 => [0, 64],
                            4 => [128, 64],
                            5 => [64, 0],
                            _ => [64, 128],
                        };
                        hint.kind = 2;
                        hint.tile = [r.g2()?, r.g2()?];
                        hint.height = r.g1()?;
                    }
                    hint.model = r.g2()?;
                    if hint.model == 65535 {
                        hint.model = -1;
                    }
                    self.hints[(flags >> 6) as usize] = Some(hint);
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HintPosition {
    Icon([i32; 2]),
    Edge { center: [i32; 2], angle: i32 },
}
impl super::View {
    pub fn hint(self, size: [i32; 2], icon: [i32; 2], delta: [i32; 2]) -> Option<HintPosition> {
        let [x, y] = delta;
        let distance = x.wrapping_mul(x).wrapping_add(y.wrapping_mul(y));
        if distance > 360000 {
            return None;
        }
        let radius = (size[0] / 2).min(size[1] / 2);
        if radius.wrapping_mul(radius) >= distance {
            return self.icon(size, icon, delta).map(HintPosition::Icon);
        }
        let radius = radius - 10;
        let (sin, cos) = crate::animation::sin_cos(self.angle());
        let zoom = self.zoom_offset.wrapping_add(256);
        let sin = sin.wrapping_mul(256).wrapping_div(zoom);
        let cos = cos.wrapping_mul(256).wrapping_div(zoom);
        let sx = y.wrapping_mul(sin).wrapping_add(cos.wrapping_mul(x)) >> 16;
        let sy = y.wrapping_mul(cos).wrapping_sub(sin.wrapping_mul(x)) >> 16;
        let angle = f64::from(sx).atan2(f64::from(sy));
        let edge = [
            (angle.sin() * f64::from(radius)) as i32,
            (angle.cos() * f64::from(radius)) as i32,
        ];
        Some(HintPosition::Edge {
            center: [(size[0] / 2 + edge[0]) * 16, (size[1] / 2 - edge[1]) * 16],
            angle: (angle * 10430.378) as i32,
        })
    }
}

pub fn hints_visible(cycle: i32) -> bool {
    cycle % 20 < 10
}
