//! Original MiniMenu actor-overlap expansion for an ordered stream of picked keys.
//!
//! Actor lists are snapshots in the original local-list order. Missing picked
//! actors are skipped, rather than dereferencing stale/null renderer state.
//! Ray construction, rendered pick order and menu insertion remain caller-owned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Player,
    Npc,
}

#[derive(Clone, Copy, Debug)]
pub struct Actor {
    pub index: usize,
    pub fine: [i32; 2],
    pub size: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    pub kind: Kind,
    pub index: usize,
    /// All expanded actors retain the tile encoded by the original picked key.
    pub tile: [i32; 2],
}

pub fn key(kind: Kind, index: usize, tile: [i32; 2]) -> u64 {
    ((index as u64 & 0x7fff_ffff) << 32)
        | ((matches!(kind, Kind::Npc) as u64) << 29)
        | ((tile[1] as u64 & 127) << 7)
        | (tile[0] as u64 & 127)
}

fn minimum(actor: &Actor) -> [i32; 2] {
    actor
        .fine
        .map(|n| n.wrapping_add(64).wrapping_sub(actor.size.wrapping_mul(64)))
}

/// ScriptRunner actor scene keys carry no tile coordinates. Noninteractive NPC
/// keys have their sign bit set, which the original model picker rejects.
pub fn actor_key(kind: Kind, index: usize, interactive: bool) -> u64 {
    key(kind, index, [0, 0])
        | if kind == Kind::Npc && !interactive {
            1 << 63
        } else {
            0
        }
}

/// Original GL actor picking requires a positive signed key (player0 is zero).
pub fn pickable(key: u64) -> bool {
    key as i64 > 0
}

fn contains(parent: &Actor, other: &Actor) -> bool {
    let lower = minimum(parent);
    let child = minimum(other);
    (0..2).all(|axis| {
        child[axis] >= lower[axis]
            && other.size
                <= parent
                    .size
                    .wrapping_sub(child[axis].wrapping_sub(lower[axis]) >> 7)
    })
}

/// Preserve consecutive full-key suppression, NPC-then-player list expansion,
/// picked-actor-last order, and repeats from distinct picks or local-list slots.
pub fn expand(keys: &[u64], npcs: &[Actor], players: &[Actor]) -> Vec<Target> {
    expand_with_own(keys, npcs, players, None)
}

/// `expand` when the own player's body can be picked. Its scene key names
/// index 2047, which `MiniMenu.addEntries` reads as `PlayerList.players[2047]`
/// (`PlayerList.self`), but `PlayerList.ids` never lists it: `own` is only
/// looked up as the picked actor, never added as an overlapping one. So the
/// own body brings up the entries of the actors standing on its tile, crowd
/// thinning's left-out ones included (MiniMenu.java:1301-1323).
pub fn expand_with_own(
    keys: &[u64],
    npcs: &[Actor],
    players: &[Actor],
    own: Option<&Actor>,
) -> Vec<Target> {
    let mut output = Vec::new();
    let mut last = u64::MAX;
    for &picked in keys {
        if picked == last {
            continue;
        }
        last = picked;
        let kind = match (picked >> 29) & 3 {
            0 => Kind::Player,
            1 => Kind::Npc,
            _ => continue,
        };
        let index = ((picked >> 32) & 0x7fff_ffff) as usize;
        let tile = [(picked & 127) as i32, ((picked >> 7) & 127) as i32];
        let list = match kind {
            Kind::Npc => npcs,
            Kind::Player => players,
        };
        let Some(actor) = list
            .iter()
            .find(|a| a.index == index)
            .or(own.filter(|own| kind == Kind::Player && own.index == index))
        else {
            continue;
        };
        let aligned = match kind {
            Kind::Npc => {
                let offset = if actor.size & 1 == 0 { 0 } else { 64 };
                actor.fine.iter().all(|n| n & 127 == offset)
            }
            // This is deliberately independent of transformed-player size.
            Kind::Player => actor.fine.iter().all(|n| n & 127 == 64),
        };
        if aligned {
            for (other_kind, others) in [(Kind::Npc, npcs), (Kind::Player, players)] {
                for other in others {
                    if (other_kind != kind || other.index != index) && contains(actor, other) {
                        output.push(Target {
                            kind: other_kind,
                            index: other.index,
                            tile,
                        });
                    }
                }
            }
        }
        output.push(Target { kind, index, tile });
    }
    output
}
