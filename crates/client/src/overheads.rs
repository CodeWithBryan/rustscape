//! Original actor hit slots, overhead clocks and ScriptRunner draw ordering.
//! Projection, font metrics, cache sprites and plugin callbacks are services.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const HITMARKS: i32 = -1;
pub const HITBARS: i32 = -2;
pub const PRAYERS: i32 = -3;
pub const SKULLS: i32 = -4;
pub const HINTS: i32 = -5;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Hit {
    pub kind: i32,
    pub damage: i32,
    pub until: i32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Chat {
    pub text: String,
    pub color: i32,
    pub effect: i32,
    pub loops: i32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct State {
    pub hits: [Hit; 4],
    pub bar: i32,
    pub bar_until: i32,
    pub chat: Option<Chat>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            hits: Default::default(),
            bar: 0,
            bar_until: -1000,
            chat: None,
        }
    }
}
impl State {
    /// PathingEntity.addHit: reuse the first expired slot; discard a fifth hit.
    pub fn hit(&mut self, kind: i32, cycle: i32, damage: i32) {
        if let Some(hit) = self.hits.iter_mut().find(|h| cycle >= h.until) {
            *hit = Hit {
                kind,
                damage,
                until: cycle.wrapping_add(70),
            };
        }
    }
    pub fn tick_chat(&mut self) {
        if let Some(chat) = &mut self.chat
            && chat.loops > 0
        {
            chat.loops -= 1;
            if chat.loops == 0 {
                self.chat = None;
            }
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct NpcStyle {
    pub headicon: i32,
    pub icon_height: i32,
    pub hitbar: i32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Actor {
    pub index: i32,
    pub visible: bool,
    pub npc: Option<NpcStyle>,
    #[serde(default)]
    pub npc_head: Option<NpcStyle>,
    pub model_height: i32,
    pub icons: [i32; 2],
    pub friend: bool,
    pub state: State,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hint {
    pub npc: bool,
    pub target: i32,
    pub sprite: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub cycle: i32,
    pub render_cycle: i32,
    pub viewport: [i32; 4],
    pub public_filter: i32,
    pub effects_disabled: i32,
    pub font_height: i32,
    pub prayer_count: i32,
    /// Width/height of the original empty sprite; custom arrays require two frames.
    pub default_bar: [i32; 2],
    pub custom_bars: BTreeMap<i32, [i32; 2]>,
    pub hints: Vec<Hint>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Draw {
    Sprite {
        group: i32,
        frame: usize,
        at: [i32; 2],
    },
    Text {
        small: bool,
        mode: i32,
        text: String,
        at: [i32; 2],
        color: i32,
        tick: i32,
        age: i32,
    },
    Clip {
        rect: [i32; 4],
    },
}
fn sprite(out: &mut Vec<Draw>, group: i32, frame: i32, x: i32, y: i32) {
    if let Ok(frame) = usize::try_from(frame) {
        out.push(Draw::Sprite {
            group,
            frame,
            at: [x, y],
        });
    }
}
fn color(code: i32, render: i32, age: i32) -> i32 {
    match code {
        0..=5 => [16776960, 16711680, 65280, 65535, 16711935, 16777215][code as usize],
        6 => {
            if render % 20 >= 10 {
                16776960
            } else {
                16711680
            }
        }
        7 => {
            if render % 20 < 10 {
                255
            } else {
                65535
            }
        }
        8 => {
            if render % 20 >= 10 {
                8454016
            } else {
                45056
            }
        }
        9 if age < 50 => age.wrapping_mul(1280).wrapping_add(16711680),
        9 if age < 100 => 33160960i32.wrapping_sub(age.wrapping_mul(327680)),
        9 if age < 150 => age.wrapping_mul(5).wrapping_add(64780),
        10 if age < 50 => age.wrapping_mul(5).wrapping_add(16711680),
        10 if age < 100 => 16711935i32.wrapping_sub(age.wrapping_sub(50).wrapping_mul(327680)),
        10 if age < 150 => age
            .wrapping_mul(327680)
            .wrapping_add(755)
            .wrapping_sub(age.wrapping_mul(5))
            .wrapping_sub(32768000),
        11 if age < 50 => 16777215i32.wrapping_sub(age.wrapping_mul(327685)),
        11 if age < 100 => age.wrapping_mul(327685).wrapping_sub(16318970),
        11 if age < 150 => 49545215i32.wrapping_sub(age.wrapping_mul(327680)),
        _ => 16776960,
    }
}
/// Actors must be in original order: self, local players, then local NPCs.
/// Projection returns viewport-relative coordinates, with X=-1 for clipped actors.
pub fn layout(
    actors: &[Actor],
    c: &Config,
    mut width: impl FnMut(&str) -> i32,
    mut project: impl FnMut(usize, i32) -> [i32; 2],
) -> Vec<Draw> {
    let [left, top, w, h] = c.viewport;
    let reset = [left, top, left.wrapping_add(w), top.wrapping_add(h)];
    let mut out = Vec::new();
    let mut chats: Vec<([i32; 2], i32, &Chat)> = Vec::new();
    for (i, a) in actors.iter().enumerate() {
        if !a.visible {
            continue;
        }
        let mut offset = 30;
        if let Some(n) = a.npc_head.or(a.npc) {
            if n.headicon >= 0 && n.headicon < c.prayer_count {
                let [x, y] = project(
                    i,
                    if n.icon_height == -1 {
                        a.model_height
                    } else {
                        n.icon_height
                    }
                    .wrapping_add(15),
                );
                if x > -1 {
                    sprite(
                        &mut out,
                        PRAYERS,
                        n.headicon,
                        left.wrapping_add(x).wrapping_sub(12),
                        top.wrapping_add(y).wrapping_sub(30),
                    );
                }
            }
        } else if a.icons != [-1, -1] {
            let [x, y] = project(i, a.model_height.wrapping_add(15));
            if x > -1 {
                if a.icons[0] != -1 {
                    sprite(
                        &mut out,
                        SKULLS,
                        a.icons[0],
                        left.wrapping_add(x).wrapping_sub(12),
                        top.wrapping_add(y).wrapping_sub(30),
                    );
                    offset += 25;
                }
                if a.icons[1] != -1 {
                    sprite(
                        &mut out,
                        PRAYERS,
                        a.icons[1],
                        left.wrapping_add(x).wrapping_sub(12),
                        top.wrapping_add(y).wrapping_sub(offset),
                    );
                    offset += 25;
                }
            }
        }
        for hint in &c.hints {
            if hint.npc != a.npc.is_some()
                || hint.target != a.index
                || (!hint.npc && i == 0)
                || (hint.npc && c.cycle % 20 >= 10)
            {
                continue;
            }
            let base = a.npc_head.or(a.npc).map_or(a.model_height, |n| {
                if n.icon_height == -1 {
                    a.model_height
                } else {
                    n.icon_height
                }
            });
            let [x, y] = project(i, base.wrapping_add(15));
            if x > -1 {
                out.push(Draw::Sprite {
                    group: HINTS,
                    frame: hint.sprite,
                    at: [
                        left.wrapping_add(x).wrapping_sub(12),
                        top.wrapping_add(y)
                            .wrapping_sub(if hint.npc { 28 } else { offset }),
                    ],
                });
            }
        }
        if let Some(chat) = &a.state.chat
            && (a.npc.is_some()
                || c.public_filter == 0
                || c.public_filter == 3
                || c.public_filter == 1 && a.friend)
        {
            let p = project(i, a.model_height);
            if p[0] > -1 && chats.len() < 50 {
                chats.push((p, width(&chat.text) / 2, chat));
            }
        }
        let base = a.npc.map_or(a.model_height, |n| {
            if n.icon_height == -1 {
                a.model_height
            } else {
                n.icon_height
            }
        });
        if a.state.bar_until > c.cycle {
            let custom = a
                .npc
                .and_then(|n| c.custom_bars.get(&n.hitbar).map(|size| (n.hitbar, *size)));
            let (group, [bw, bh]) = custom.unwrap_or((HITBARS, c.default_bar));
            let [x, y] = project(i, bh.wrapping_add(base).wrapping_add(10));
            if x > -1 {
                let x = x.wrapping_add(left).wrapping_sub(bw >> 1);
                let y = y.wrapping_add(top).wrapping_sub(3);
                sprite(&mut out, group, 0, x, y);
                let fill = bw.wrapping_mul(a.state.bar).wrapping_div(255);
                out.push(Draw::Clip {
                    rect: [x, y, x.wrapping_add(fill), y.wrapping_add(bh)],
                });
                sprite(&mut out, group, 1, x, y);
                out.push(Draw::Clip { rect: reset });
            }
        }
        for (slot, hit) in a.state.hits.iter().enumerate() {
            if hit.until <= c.cycle {
                continue;
            }
            let [mut x, mut y] = project(i, base / 2);
            if x <= -1 {
                continue;
            }
            match slot {
                1 => y = y.wrapping_sub(20),
                2 => {
                    x = x.wrapping_sub(15);
                    y = y.wrapping_sub(10)
                }
                3 => {
                    x = x.wrapping_add(15);
                    y = y.wrapping_sub(10)
                }
                _ => {}
            }
            sprite(
                &mut out,
                HITMARKS,
                hit.kind,
                left.wrapping_add(x).wrapping_sub(12),
                top.wrapping_add(y).wrapping_sub(12),
            );
            out.push(Draw::Text {
                small: true,
                mode: 0,
                text: hit.damage.to_string(),
                at: [
                    x.wrapping_add(left).wrapping_sub(1),
                    y.wrapping_add(top).wrapping_add(3),
                ],
                color: 16777215,
                tick: 0,
                age: 0,
            });
        }
    }
    for i in 0..chats.len() {
        let ([x, mut y], half, chat) = chats[i];
        loop {
            let mut moved = false;
            for (p, previous_half, _) in &chats[..i] {
                if p[1].wrapping_sub(c.font_height) < y.wrapping_add(2)
                    && y.wrapping_sub(c.font_height) < p[1].wrapping_add(2)
                    && x.wrapping_sub(half) < p[0].wrapping_add(*previous_half)
                    && p[0].wrapping_sub(*previous_half) < x.wrapping_add(half)
                    && p[1].wrapping_sub(c.font_height) < y
                {
                    y = p[1].wrapping_sub(c.font_height);
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
        chats[i].0[1] = y;
        let age = 150i32.wrapping_sub(chat.loops);
        let effect = if c.effects_disabled == 0 {
            chat.effect
        } else {
            0
        };
        if !(0..=5).contains(&effect) {
            continue;
        }
        let color = if c.effects_disabled == 0 {
            color(chat.color, c.render_cycle, age)
        } else {
            16776960
        };
        let mut at = [left.wrapping_add(x), top.wrapping_add(y)];
        let mut mode = effect;
        if effect == 4 {
            let offset = age
                .wrapping_mul(width(&chat.text).wrapping_add(100))
                .wrapping_div(150);
            out.push(Draw::Clip {
                rect: [
                    left.wrapping_add(x).wrapping_sub(50),
                    top,
                    left.wrapping_add(x).wrapping_add(50),
                    top.wrapping_add(h),
                ],
            });
            at[0] = at[0].wrapping_add(50).wrapping_sub(offset);
        } else if effect == 5 {
            out.push(Draw::Clip {
                rect: [
                    left,
                    top.wrapping_add(y)
                        .wrapping_sub(c.font_height)
                        .wrapping_sub(1),
                    left.wrapping_add(w),
                    top.wrapping_add(y).wrapping_add(5),
                ],
            });
            at[1] = at[1].wrapping_add(if age < 25 {
                age.wrapping_sub(25)
            } else if age > 125 {
                age.wrapping_sub(125)
            } else {
                0
            });
            mode = 0;
        }
        out.push(Draw::Text {
            small: false,
            mode,
            text: chat.text.clone(),
            at,
            color,
            tick: if (1..=3).contains(&effect) {
                c.render_cycle
            } else {
                0
            },
            age: if effect == 3 { age } else { 0 },
        });
        if effect == 4 || effect == 5 {
            out.push(Draw::Clip { rect: reset });
        }
    }
    out
}

/// Font.renderWave/renderWave2/renderShake offsets, indexed by drawn glyph/icon.
pub fn glyph_offsets(effect: i32, length: usize, tick: i32, age: i32) -> Vec<[i32; 2]> {
    let t = f64::from(tick);
    let amplitude = (7.0 - f64::from(age) / 8.0).max(0.0);
    (0..length)
        .map(|i| {
            let i = i as f64;
            match effect {
                1 => [0, ((i / 2.0 + t / 5.0).sin() * 5.0) as i32],
                2 => [
                    ((i / 5.0 + t / 5.0).sin() * 5.0) as i32,
                    ((i / 3.0 + t / 5.0).sin() * 5.0) as i32,
                ],
                3 => [0, ((i / 1.5 + t).sin() * amplitude) as i32],
                _ => [0, 0],
            }
        })
        .collect()
}
