//! Which actors enter the scene: `ScriptRunner.renderGameScene`'s insertion
//! order with `addPlayersToScene`/`addNpcsToScene`, their crowd thinning and
//! the low-detail rule for idle players.
//!
//! The original counts, per tile, the actors standing exactly on it and then
//! leaves out every such actor while the count is above one, counting down as
//! it goes. The counts are cleared once per frame and carried across the four
//! passes (own player, top-priority NPCs, players, other NPCs), so an actor
//! of an earlier pass keeps its tile and hides later ones standing on it.
//! Coordinates are fine units relative to the scene origin.

/// A player as the insertion code reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Player {
    /// `PlayerList.ids[i]`; it becomes the high half of the scene key.
    pub index: i32,
    /// `isVisible()`: the appearance has arrived.
    pub visible: bool,
    pub size: i32,
    pub fine: [i32; 2],
    /// `movementSeqId == getBasType().idleAnimationId`.
    pub idle: bool,
    /// An attachment exists and `attachmentSetAt <= loop < attachmentResetAt`.
    pub attached: bool,
}

/// An NPC as the insertion code reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Npc {
    /// `NpcList.ids[i]`.
    pub index: i32,
    /// `isVisible()` (a type is set) and `type.isMultiNpcValid()`.
    pub visible: bool,
    /// `type.toprenderpriority` (NpcType opcode 99).
    pub top: bool,
    pub size: i32,
    pub fine: [i32; 2],
    pub interactive: bool,
}

/// One frame's inputs. `players` and `npcs` are in list order with `None`
/// for an index whose table entry is null.
#[derive(Clone, Copy, Debug)]
pub struct Scene<'a> {
    /// `PlayerList.self`.
    pub own: Player,
    /// Position in `players` of the own player (index 2047), which the
    /// original lists first; that entry is the same object as `own`.
    pub own_listed: Option<usize>,
    pub players: &'a [Option<Player>],
    pub npcs: &'a [Option<Npc>],
    /// `PlayerList.size`.
    pub player_count: i32,
    /// `Preferences.manyIdleAnimations`.
    pub many_idle_animations: bool,
    /// `renderGameScene(minimapOnly)`: only the last NPC pass runs.
    pub minimap_only: bool,
    /// `LoginManager.mapFlagX/Y`.
    pub flag: [i32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Who {
    /// The own player, from the first pass.
    Own,
    /// A position in `Scene::players`.
    Player(usize),
    /// A position in `Scene::npcs`.
    Npc(usize),
}

/// One `SceneGraph.add` (or `addAttachedEntityToScene`) call, in call order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inserted {
    pub who: Who,
    /// The scene key the original passes (picking reads it back).
    pub key: i64,
    /// The radius argument, `(size - 1) * 64 + 60`.
    pub radius: i32,
    pub attached: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Frame {
    pub inserted: Vec<Inserted>,
    /// `Player.lowDetail` after the frame, per entry of `Scene::players`.
    pub low_detail: Vec<bool>,
    /// `lowDetail` of the own player, which the list pass may set.
    pub own_low_detail: bool,
    /// `LoginManager.mapFlagX` after the frame: zero once the own player
    /// stands on the flagged tile.
    pub flag_x: i32,
    /// Tile counts left at the end, for tests: `(x, y, count)`.
    pub counts: Vec<(i32, i32, i32)>,
}

const SIDE: i32 = 104;

struct Counts(Vec<i32>);
impl Counts {
    fn at(&mut self, x: i32, y: i32) -> &mut i32 {
        &mut self.0[(x * SIDE + y) as usize]
    }

    /// The first loop of both methods: count an actor standing exactly on a
    /// tile (size one) or exactly on its footprint.
    fn count(&mut self, size: i32, [x, y]: [i32; 2]) {
        let sub = [x & 0x7f, y & 0x7f];
        if size == 1 {
            if sub == [64, 64] {
                let (tx, ty) = (x >> 7, y >> 7);
                if (0..SIDE).contains(&tx) && (0..SIDE).contains(&ty) {
                    *self.at(tx, ty) += 1;
                }
            }
        } else if (size & 1 != 0 || sub == [0, 0]) && (size & 1 != 1 || sub == [64, 64]) {
            let (x0, y0) = (
                x.wrapping_sub(size.wrapping_mul(64)) >> 7,
                y.wrapping_sub(size.wrapping_mul(64)) >> 7,
            );
            let (x1, y1) = (
                size.wrapping_add(x0).min(SIDE),
                size.wrapping_add(y0).min(SIDE),
            );
            for tx in x0.max(0)..x1 {
                for ty in y0.max(0)..y1 {
                    *self.at(tx, ty) += 1;
                }
            }
        }
    }

    /// The second loop's test: false when the actor is left out this frame.
    /// `odd_y` is the fine remainder an odd-sized actor's y must have: 64 for
    /// NPCs, and 0 for players, whose test the original wrote that way.
    fn keeps(&mut self, size: i32, [x, y]: [i32; 2], odd_y: i32) -> bool {
        let sub = [x & 0x7f, y & 0x7f];
        if size == 1 {
            if sub == [64, 64] {
                let (tx, ty) = (x >> 7, y >> 7);
                if !(0..SIDE).contains(&tx) || !(0..SIDE).contains(&ty) {
                    return false;
                }
                if *self.at(tx, ty) > 1 {
                    *self.at(tx, ty) -= 1;
                    return false;
                }
            }
        } else if size & 1 == 0 && sub == [0, 0] || size & 1 == 1 && sub == [64, odd_y] {
            let (x0, y0) = (
                x.wrapping_sub(size.wrapping_mul(64)) >> 7,
                y.wrapping_sub(size.wrapping_mul(64)) >> 7,
            );
            let (x1, y1) = (
                size.wrapping_add(x0).min(SIDE),
                size.wrapping_add(y0).min(SIDE),
            );
            let (x0, y0) = (x0.max(0), y0.max(0));
            // The original's `break` leaves only the inner loop.
            let mut crowded = true;
            for tx in x0..x1 {
                for ty in y0..y1 {
                    if *self.at(tx, ty) <= 1 {
                        crowded = false;
                        break;
                    }
                }
            }
            if crowded {
                for tx in x0..x1 {
                    for ty in y0..y1 {
                        *self.at(tx, ty) -= 1;
                    }
                }
                return false;
            }
        }
        true
    }
}

/// `renderGameScene` lines 208–215 for one frame.
pub fn frame(scene: &Scene<'_>) -> Frame {
    let mut counts = Counts(vec![0; (SIDE * SIDE) as usize]);
    let mut out = Frame {
        low_detail: vec![false; scene.players.len()],
        flag_x: scene.flag[0],
        ..Frame::default()
    };
    let players = |counts: &mut Counts, out: &mut Frame, own_only: bool| {
        if out.flag_x == scene.own.fine[0] >> 7 && scene.own.fine[1] >> 7 == scene.flag[1] {
            out.flag_x = 0;
        }
        let own = [Some(scene.own)];
        let list: &[Option<Player>] = if own_only { &own } else { scene.players };
        for player in list.iter().flatten().filter(|p| p.visible) {
            counts.count(player.size, player.fine);
        }
        for (position, player) in list.iter().enumerate() {
            let Some(player) = player.filter(|p| p.visible) else {
                continue;
            };
            let is_own = own_only || scene.own_listed == Some(position);
            let mut low = (scene.many_idle_animations && scene.player_count > 200
                || scene.player_count > 50)
                && !own_only
                && player.idle;
            let kept = counts.keeps(player.size, player.fine, 0);
            if kept && player.attached {
                low = false;
            }
            if !own_only {
                out.low_detail[position] = low;
            }
            if is_own {
                out.own_low_detail = low;
            }
            if kept {
                out.inserted.push(Inserted {
                    who: if own_only {
                        Who::Own
                    } else {
                        Who::Player(position)
                    },
                    key: if own_only {
                        8791798054912
                    } else {
                        i64::from(player.index) << 32
                    },
                    radius: player
                        .size
                        .wrapping_sub(1)
                        .wrapping_mul(64)
                        .wrapping_add(60),
                    attached: player.attached,
                });
            }
        }
    };
    let npcs = |counts: &mut Counts, out: &mut Frame, top: bool| {
        let listed = |npc: &&Npc| npc.visible && npc.top == top;
        for npc in scene.npcs.iter().flatten().filter(listed) {
            counts.count(npc.size, npc.fine);
        }
        for (position, npc) in scene.npcs.iter().enumerate() {
            let Some(npc) = npc.as_ref().filter(listed) else {
                continue;
            };
            if counts.keeps(npc.size, npc.fine, 64) {
                let mut key = i64::from(npc.index) << 32 | 0x2000_0000;
                if !npc.interactive {
                    key |= i64::MIN;
                }
                out.inserted.push(Inserted {
                    who: Who::Npc(position),
                    key,
                    radius: npc.size.wrapping_mul(64).wrapping_add(60 - 64),
                    attached: false,
                });
            }
        }
    };
    if !scene.minimap_only {
        players(&mut counts, &mut out, true);
        npcs(&mut counts, &mut out, true);
        players(&mut counts, &mut out, false);
    }
    npcs(&mut counts, &mut out, false);
    for x in 0..SIDE {
        for y in 0..SIDE {
            let count = *counts.at(x, y);
            if count != 0 {
                out.counts.push((x, y, count));
            }
        }
    }
    out
}

impl Frame {
    /// Whether the player at `position` of `Scene::players` is drawn.
    pub fn draws_player(&self, position: usize) -> bool {
        self.inserted.iter().any(|i| i.who == Who::Player(position))
    }

    /// Whether the NPC at `position` of `Scene::npcs` is drawn.
    pub fn draws_npc(&self, position: usize) -> bool {
        self.inserted.iter().any(|i| i.who == Who::Npc(position))
    }
}
