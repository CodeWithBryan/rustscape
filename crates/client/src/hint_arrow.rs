//! Hint arrows in the 3D view: the sprite over a hinted tile
//! (`MiniMap.renderHeadHints`) and the arrow model at the own player's feet
//! that points at a hinted NPC, tile or player (`Player.render`'s marker
//! loop, `Player.renderHintArrow` and `HintArrowManager.getModel`).
use crate::{animation::sin_cos, minimap::Hint, model::Model};

/// One tile hint to draw: the hint-sprite frame and its top-left corner.
pub type TileSprite = (usize, [i32; 2]);

/// `MiniMap.renderHeadHints`: for every tile hint (marker type 2), project
/// the point `targetHeight * 2` above the hinted spot and, during the first
/// half of each 20-cycle period, draw the sprite 12 left of and 28 above it.
///
/// `project` receives the scene-relative fine position and the height above
/// the terrain there and returns viewport coordinates, x = -1 when the point
/// is not on screen (`ScriptRunner.projectToScreen`, which also rejects
/// positions outside fine 128..=13056). `viewport` is the 3D view's left and
/// top; `origin` the scene origin in tiles.
pub fn tile_sprites(
    hints: &[Option<Hint>],
    origin: [i32; 2],
    cycle: i32,
    viewport: [i32; 2],
    mut project: impl FnMut([i32; 2], i32) -> [i32; 2],
) -> Vec<TileSprite> {
    let mut out = Vec::new();
    for hint in hints.iter().flatten().filter(|h| h.kind == 2) {
        let fine: [i32; 2] = std::array::from_fn(|i| {
            (hint.tile[i].wrapping_sub(origin[i]) << 7).wrapping_add(hint.fine[i])
        });
        let [x, y] = if fine.into_iter().any(|n| !(128..=13056).contains(&n)) {
            [-1, -1]
        } else {
            project(fine, hint.height.wrapping_mul(2))
        };
        if x > -1 && cycle % 20 < 10 {
            out.push((
                hint.sprite,
                [
                    viewport[0].wrapping_add(x).wrapping_sub(12),
                    viewport[1].wrapping_sub(28).wrapping_add(y),
                ],
            ));
        }
    }
    out
}

/// One arrow model to draw at the own player's feet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FootArrow {
    /// Model id (the hint's `playerModelId`).
    pub model: i32,
    /// Rotation about Y towards the target, in 2048ths of a turn.
    pub angle: i32,
}

/// `Player.renderHintArrow`'s gate and angle. `dx`/`dy` are in quarter
/// tiles; nothing is drawn nearer than one tile or farther than 150.
pub fn arrow_angle(dx: i32, dy: i32) -> Option<i32> {
    let squared = dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy));
    if !(16..=360000).contains(&squared) {
        return None;
    }
    Some(((f64::from(dx).atan2(f64::from(dy)) * 325.949) as i32) & 0x7ff)
}

/// The marker loop of `Player.render` for the own player: hints are visited
/// from the last slot to the first, and each with a model yields an arrow
/// towards its NPC (type 1), tile (type 2) or player (type 10).
///
/// Fine coordinates are scene-relative, `origin` is the scene origin in
/// tiles, and `npc`/`player` return the fine position of the actor at an
/// index of `NpcList.npcs` / `PlayerList.players` when it exists.
pub fn foot_arrows(
    hints: &[Option<Hint>],
    own: [i32; 2],
    origin: [i32; 2],
    npc: impl Fn(i32) -> Option<[i32; 2]>,
    player: impl Fn(i32) -> Option<[i32; 2]>,
) -> Vec<FootArrow> {
    let mut out = Vec::new();
    let quarter = |fine: [i32; 2]| [fine[0] / 32 - own[0] / 32, fine[1] / 32 - own[1] / 32];
    for hint in hints.iter().rev().flatten().filter(|h| h.model != -1) {
        let delta = match hint.kind {
            1 if (0..32768).contains(&hint.actor) => npc(hint.actor).map(quarter),
            2 => Some(std::array::from_fn(|i| {
                hint.tile[i]
                    .wrapping_sub(origin[i])
                    .wrapping_mul(4)
                    .wrapping_add(2)
                    .wrapping_sub(own[i] / 32)
            })),
            10 if (0..2048).contains(&hint.actor) => player(hint.actor).map(quarter),
            _ => None,
        };
        if let Some(angle) = delta.and_then(|[dx, dy]| arrow_angle(dx, dy)) {
            out.push(FootArrow {
                model: hint.model,
                angle,
            });
        }
    }
    out
}

/// `[[minX, maxX], [minZ, maxZ]]` of a body model, as `HintArrowManager`
/// reads them from the own player's body before lean and terrain alignment.
pub fn body_extent(body: &Model) -> [[i32; 2]; 2] {
    [0, 2].map(|axis| {
        let values = body.vertices.iter().map(move |v| v[axis]);
        [values.clone().min().unwrap_or(0), values.max().unwrap_or(0)]
    })
}

/// `HintArrowManager.getModel` after the model is lit and copied: turn it
/// about Y, then lay it on the terrain. `extent` is [`body_extent`] of the
/// own player's body; it decides whether the ground under the player is
/// level. When the terrain height at either extent corner differs from
/// `base` (the height under the player), every vertex is moved by the height
/// difference under it. `height` samples the terrain at a scene-relative
/// fine position.
pub fn lay(
    arrow: &mut Model,
    angle: i32,
    extent: [[i32; 2]; 2],
    own: [i32; 2],
    base: i32,
    mut height: impl FnMut([i32; 2]) -> i32,
) {
    if angle != 0 {
        let (sin, cos) = sin_cos(angle);
        for v in &mut arrow.vertices {
            let x = v[2].wrapping_mul(sin).wrapping_add(v[0].wrapping_mul(cos)) >> 16;
            v[2] = v[2].wrapping_mul(cos).wrapping_sub(v[0].wrapping_mul(sin)) >> 16;
            v[0] = x;
        }
    }
    let [[min_x, max_x], [min_z, max_z]] = extent;
    let [x, y] = own;
    if base != height([x.wrapping_add(min_x), y.wrapping_add(min_z)])
        || base != height([x.wrapping_add(max_x), y.wrapping_add(max_z)])
    {
        for v in &mut arrow.vertices {
            let under = height([x.wrapping_add(v[0]), y.wrapping_add(v[2])]);
            v[1] = v[1].wrapping_add(under.wrapping_sub(base));
        }
    }
}

#[cfg(test)]
mod tests {
    //! Transcription tests: the expected values are worked out by hand from
    //! MiniMap.renderHeadHints, Player.render 450-479, Player.renderHintArrow
    //! and HintArrowManager.getModel. No JVM ran these.
    use super::*;

    fn hint(kind: i32, model: i32) -> Hint {
        Hint {
            kind,
            model,
            ..Hint::default()
        }
    }

    #[test]
    fn tile_hint_sprite_blinks_above_the_hinted_spot() {
        let tile = Hint {
            sprite: 3,
            tile: [3210, 3220],
            fine: [128, 64],
            height: 50,
            ..hint(2, -1)
        };
        let hints = [Some(hint(1, -1)), None, Some(tile), Some(hint(10, 7))];
        let seen = std::cell::RefCell::new(Vec::new());
        let mut project = |fine: [i32; 2], height: i32| {
            seen.borrow_mut().push((fine, height));
            [200, 150]
        };
        // Cycle 9 is in the lit half; left 4, top 30.
        let drawn = tile_sprites(&hints, [3200, 3200], 9, [4, 30], &mut project);
        assert_eq!(drawn, [(3, [4 + 200 - 12, 30 - 28 + 150])]);
        // (tile - origin) << 7, plus the hint's fine offset; height doubled.
        assert_eq!(*seen.borrow(), [([10 * 128 + 128, 20 * 128 + 64], 100)]);
        // Dark half of the period.
        assert!(tile_sprites(&hints, [3200, 3200], 10, [4, 30], &mut project).is_empty());
        assert!(!tile_sprites(&hints, [3200, 3200], 20, [4, 30], &mut project).is_empty());
        // Off screen.
        assert!(tile_sprites(&hints, [3200, 3200], 0, [4, 30], |_, _| [-1, -1]).is_empty());
        // projectToScreen rejects fine positions outside 128..=13056 without
        // projecting.
        let mut calls = 0;
        let edge = Hint {
            tile: [3200, 3220],
            fine: [64, 64],
            ..hint(2, -1)
        };
        let drawn = tile_sprites(&[Some(edge)], [3200, 3200], 0, [0, 0], |_, _| {
            calls += 1;
            [1, 1]
        });
        assert!(drawn.is_empty() && calls == 0);
    }

    #[test]
    fn foot_arrow_gate_and_angle() {
        // Nearer than one tile (16 = 4 quarter tiles squared) draws nothing.
        assert_eq!(arrow_angle(3, 2), None);
        // atan2(4, 0) * 325.949 = 511.9994..., truncated to 511: a quarter
        // turn is one unit short of 512, as in the original.
        assert_eq!(arrow_angle(4, 0), Some(511));
        assert_eq!(arrow_angle(0, 4), Some(0));
        assert_eq!(arrow_angle(0, -4), Some(1023));
        assert_eq!(arrow_angle(-4, 0), Some(-511 & 0x7ff));
        assert_eq!(arrow_angle(600, 0), Some(511));
        assert_eq!(arrow_angle(600, 1), None);
    }

    #[test]
    fn foot_arrows_follow_the_marker_loop() {
        let own = [50 * 128 + 64, 50 * 128 + 64];
        let npc_hint = Hint {
            actor: 9,
            ..hint(1, 100)
        };
        let tile_hint = Hint {
            tile: [3260, 3250],
            ..hint(2, 101)
        };
        let player_hint = Hint {
            actor: 5,
            ..hint(10, 102)
        };
        let no_model = Hint {
            tile: [3260, 3250],
            ..hint(2, -1)
        };
        let npc = |i: i32| (i == 9).then_some([50 * 128 + 64, 60 * 128 + 64]);
        let player = |i: i32| (i == 5).then_some([40 * 128 + 64, 50 * 128 + 64]);
        let arrows = foot_arrows(
            &[
                Some(npc_hint),
                Some(tile_hint),
                Some(player_hint),
                Some(no_model),
            ],
            own,
            [3200, 3200],
            npc,
            player,
        );
        // Last slot first; the hint without a model is skipped.
        assert_eq!(
            arrows,
            [
                // Player ten tiles west: dx = -40, dy = 0.
                FootArrow {
                    model: 102,
                    angle: -511 & 0x7ff
                },
                // Tile (60, 50): dx = 60*4 + 2 - 202 = 40, dy = 50*4 + 2 - 202 = 0.
                FootArrow {
                    model: 101,
                    angle: 511
                },
                // NPC ten tiles north: dx = 0, dy = 40.
                FootArrow {
                    model: 100,
                    angle: 0
                },
            ]
        );
        // A missing actor, or one on the player's own tile, draws nothing.
        let missing = foot_arrows(
            &[Some(npc_hint), Some(player_hint)],
            own,
            [3200, 3200],
            |_| None,
            |_| Some(own),
        );
        assert!(missing.is_empty());
    }

    #[test]
    fn arrow_model_turns_then_lies_on_uneven_ground() {
        let body = body_extent(&Model {
            vertices: vec![[-20, -200, -10], [30, 0, 40]],
            ..Model::default()
        });
        assert_eq!(body, [[-20, 30], [-10, 40]]);
        let flat = Model {
            vertices: vec![[100, -5, 0], [0, -5, 100]],
            ..Model::default()
        };
        // Level ground: only the turn. GlModel.rotateY:
        // x' = (z sin + x cos) >> 16, z' = (z cos - x sin) >> 16.
        let mut arrow = flat.clone();
        lay(&mut arrow, 400, body, [6464, 6464], -300, |_| -300);
        let (sin, cos) = sin_cos(400);
        assert_eq!(
            arrow.vertices,
            [
                [(100 * cos) >> 16, -5, (-100 * sin) >> 16],
                [(100 * sin) >> 16, -5, (100 * cos) >> 16]
            ]
        );
        // A slope seen at the body's far corner (max x, max z): each vertex
        // moves by the height under it minus the height under the player.
        let slope = |fine: [i32; 2]| -300 - (fine[0] - 6464) / 2;
        let mut arrow = flat.clone();
        lay(&mut arrow, 0, body, [6464, 6464], -300, slope);
        assert_eq!(arrow.vertices, [[100, -55, 0], [0, -5, 100]]);
        // Uneven ground away from both corners is not noticed.
        let pit = |fine: [i32; 2]| if fine == [6564, 6464] { -100 } else { -300 };
        let mut arrow = flat.clone();
        lay(&mut arrow, 0, body, [6464, 6464], -300, pit);
        assert_eq!(arrow.vertices, flat.vertices);
    }
}
