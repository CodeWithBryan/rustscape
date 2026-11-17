//! Crowd thinning as a live session feeds it.
//!
//! `live_scenes_thin_like_the_original` is a JVM differential: the
//! `crowd-live` lines of `fixtures/actors/oracle.jsonl` come from fixed scenes
//! run through `ScriptRunner.addPlayersToScene`/`addNpcsToScene` (copied
//! verbatim at generation time, `cargo xtask actors-oracle`) with the own
//! player outside `PlayerList.ids`, as `Protocol.readNewPlayerInfo` keeps it.
//! The other tests are transcription tests: expected values derived by hand
//! from the Java lines cited.
use rs09_client::{crowd, entity_menu};
use rs09_protocol::game::players::{SELF, State};
use serde_json::{Value, json};

fn cases(kind: &str) -> Vec<Value> {
    include_str!("../../../fixtures/actors/oracle.jsonl")
        .lines()
        .filter(|line| line.starts_with(&format!("{{\"case\":\"{kind}\"")))
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
fn int(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}
fn ints(v: &Value) -> Vec<i32> {
    v.as_array().unwrap().iter().map(int).collect()
}

#[test]
fn live_scenes_thin_like_the_original() {
    let cases = cases("crowd-live");
    assert_eq!(cases.len(), 13);
    let mut added_per_case = Vec::new();
    for case in &cases {
        let id = int(&case["id"]);
        let rows: Vec<Vec<i32>> = case["players"]
            .as_array()
            .unwrap()
            .iter()
            .map(ints)
            .collect();
        let player = |r: &[i32]| crowd::Player {
            index: r[0],
            visible: r[3] != 0,
            size: r[4],
            fine: [r[5], r[6]],
            idle: r[7] == r[8],
            attached: false,
        };
        // The own row is never listed in these scenes.
        assert_eq!(rows[0][..2], [2047, 0], "case {id}");
        let others: Vec<Option<crowd::Player>> =
            rows[1..].iter().map(|r| Some(player(r))).collect();
        let npc_rows: Vec<Vec<i32>> = case["npcs"].as_array().unwrap().iter().map(ints).collect();
        let npcs: Vec<Option<crowd::Npc>> = npc_rows
            .iter()
            .map(|r| {
                Some(crowd::Npc {
                    index: r[0],
                    visible: r[1] != 0 && r[3] != 0,
                    top: r[2] != 0,
                    interactive: r[4] != 0,
                    size: r[5],
                    fine: [r[6], r[7]],
                })
            })
            .collect();
        let flag = ints(&case["flag"]);
        let frame = crowd::live_frame(player(&rows[0]), &others, &npcs, false, [flag[0], flag[1]]);
        let actual: Vec<Value> = frame
            .inserted
            .iter()
            .map(|i| {
                let fine = match i.who {
                    crowd::Who::Own => [rows[0][5], rows[0][6]],
                    crowd::Who::Player(p) => [rows[p + 1][5], rows[p + 1][6]],
                    crowd::Who::Npc(p) => [npc_rows[p][6], npc_rows[p][7]],
                };
                json!([
                    0,
                    (i.key as u64) >> 32,
                    i.key as u64 & 0xffff_ffff,
                    fine[0],
                    fine[1],
                    i.radius,
                    0,
                    0
                ])
            })
            .collect();
        assert_eq!(Value::from(actual), case["added"], "case {id}");
        assert_eq!(
            frame
                .low_detail
                .iter()
                .map(|&b| i32::from(b))
                .collect::<Vec<_>>(),
            ints(&case["low"]),
            "case {id}"
        );
        assert_eq!(
            i32::from(frame.own_low_detail),
            int(&case["self_low"]),
            "case {id}"
        );
        assert_eq!(frame.flag_x, int(&case["flag_x"]), "case {id}");
        assert_eq!(
            frame
                .counts
                .iter()
                .map(|&(x, y, n)| vec![x, y, n])
                .collect::<Vec<_>>(),
            case["counts"]
                .as_array()
                .unwrap()
                .iter()
                .map(ints)
                .collect::<Vec<_>>(),
            "case {id}"
        );
        added_per_case.push(frame.inserted.len());
    }
    // One other player on the own tile, three of them, one more next to it,
    // two on the next tile, an NPC, a top NPC on the own tile, ...: what the
    // original inserts (own player first).
    assert_eq!(added_per_case[..12], [1, 1, 2, 2, 1, 1, 2, 2, 1, 2, 2, 2]);
    // 55 listed players: every idle one is low detail, the own one is not.
    let low = ints(&cases[12]["low"]);
    assert_eq!(low.len(), 55);
    assert_eq!(low.iter().filter(|&&b| b != 0).count(), 44);
}

/// A PLAYER_INFO payload writer: bits most significant first, as
/// `Buffer.gBits` reads them.
#[derive(Default)]
struct Bits {
    bytes: Vec<u8>,
    at: usize,
}
impl Bits {
    fn put(&mut self, count: u32, value: u32) {
        for bit in (0..count).rev() {
            if self.at % 8 == 0 {
                self.bytes.push(0);
            }
            if value >> bit & 1 != 0 {
                *self.bytes.last_mut().unwrap() |= 0x80 >> (self.at % 8);
            }
            self.at += 1;
        }
    }
    /// `Protocol.readNewPlayerInfo`: id, extended flag, x offset, angle,
    /// discard flag, y offset.
    fn add(&mut self, index: u32, extended: bool) {
        self.put(11, index);
        self.put(1, u32::from(extended));
        self.put(5, 0);
        self.put(3, 0);
        self.put(1, 1);
        self.put(5, 0);
    }
}

/// An appearance block (mask 0x4): size 1, no equipment, no sounds.
fn appearance() -> Vec<u8> {
    let mut block = vec![0, 0xff, 0xff];
    block.extend([0; 12]);
    block.extend([0; 5]);
    block.extend([0, 0]);
    block.extend(37u64.to_be_bytes());
    block.extend([3, 3, 255, 0]);
    let mut out = vec![0x04, block.len() as u8 + 128];
    out.extend(block);
    out
}

/// What the live client feeds `crowd::live_frame`: the update state's local
/// list (never the own player, Protocol.java:858 `playerId != 2047`) and
/// `Player.isVisible()` (`appearance != null`, Player.java:578-580), which
/// holds from the frame whose appearance block set it until the player
/// leaves. A later frame without any appearance block leaves both players on
/// the own tile thinned (ScriptRunner.java:668-801): the own one is inserted
/// by the self pass, the visible other one is counted on its tile and left
/// out, and the one whose appearance never came is neither counted nor
/// inserted.
#[test]
fn live_update_state_feeds_the_thinning_its_visibility() {
    let mut state = State::new(40, 40);
    // Frame 1: players 5 (with an appearance) and 9 (without) arrive on the
    // own tile.
    let mut bits = Bits::default();
    bits.put(1, 0);
    bits.put(8, 0);
    bits.add(5, true);
    bits.add(9, false);
    bits.put(11, SELF as u32);
    let mut payload = bits.bytes;
    payload.extend(appearance());
    let update = state.decode(&payload, 1).unwrap();
    assert_eq!(update.extended.len(), 1);
    // Frame 2: both stay, nothing extended.
    let mut bits = Bits::default();
    bits.put(1, 0);
    bits.put(8, 2);
    bits.put(1, 0);
    bits.put(1, 0);
    let update = state.decode(&bits.bytes, 2).unwrap();
    assert!(update.extended.is_empty());
    assert_eq!(state.local_ids, [5, 9]);
    assert!(crowd::visible(&state, 5));
    assert!(!crowd::visible(&state, 9));
    assert!(!crowd::visible(&state, 7));

    let fine = |index: usize| {
        let tile = state.players[index].as_ref().unwrap().tile();
        tile.map(|n| n * 128 + 64)
    };
    let player = |index: usize, visible: bool| crowd::Player {
        index: index as i32,
        visible,
        size: 1,
        fine: fine(index),
        idle: true,
        attached: false,
    };
    let own = player(SELF, true);
    assert_eq!(own.fine, [40 * 128 + 64; 2]);
    let others: Vec<_> = state
        .local_ids
        .iter()
        .map(|&index| Some(player(index, crowd::visible(&state, index))))
        .collect();
    assert_eq!(others[0].unwrap().fine, own.fine);
    assert_eq!(others[1].unwrap().fine, own.fine);
    let frame = crowd::live_frame(own, &others, &[], false, [0, 0]);
    assert_eq!(frame.inserted.len(), 1);
    assert_eq!(frame.inserted[0].who, crowd::Who::Own);
    assert_eq!(frame.inserted[0].key, (SELF as i64) << 32);
    assert!(!frame.draws_player(0) && !frame.draws_player(1));
    assert_eq!(frame.counts, [(40, 40, 1)]);
    assert_eq!(frame.low_detail, [false, false]);
}

/// MiniMenu.java:1301-1323: a picked own body (`PlayerList.players[2047]`)
/// lists the NPCs, then the listed players, standing on its tile, then
/// itself; MiniMenu.java:1277-1299: a picked NPC on that tile lists the
/// listed players there, which never include the own one.
#[test]
fn the_own_body_lists_the_actors_on_its_tile() {
    let (c, e) = (50 * 128 + 64, 51 * 128 + 64);
    let actor = |index, x| entity_menu::Actor {
        index,
        fine: [x, c],
        size: 1,
    };
    let npcs = [actor(7, c), actor(8, e)];
    let players = [actor(1, c), actor(2, e)];
    let own = actor(SELF, c);
    let picked = entity_menu::actor_key(entity_menu::Kind::Player, SELF, true);
    assert_eq!(picked, 8791798054912);
    let found = |targets: Vec<entity_menu::Target>| {
        targets
            .into_iter()
            .map(|t| (t.kind, t.index))
            .collect::<Vec<_>>()
    };
    use entity_menu::Kind::{Npc, Player};
    assert_eq!(
        found(entity_menu::expand_with_own(
            &[picked],
            &npcs,
            &players,
            Some(&own)
        )),
        [(Npc, 7), (Player, 1), (Player, SELF)]
    );
    // Without the own body nothing names index 2047.
    assert!(entity_menu::expand(&[picked], &npcs, &players).is_empty());
    let npc = entity_menu::actor_key(Npc, 7, true);
    assert_eq!(
        found(entity_menu::expand_with_own(
            &[npc],
            &npcs,
            &players,
            Some(&own)
        )),
        [(Player, 1), (Npc, 7)]
    );
}
