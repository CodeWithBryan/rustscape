//! The player changing level inside a scene, as climbing stairs does.
//!
//! The server moves the local player to another level with PLAYER_INFO
//! (225): movement type 3 reads the tile's y, the discard bit, then
//! `Player.plane = gBits(2)`, the extended bit and x (Protocol.java:778-787),
//! or with TELEPORT_LOCAL_PLAYER (13), `Player.plane = flags >> 1`
//! (Protocol.java:1564-1569). A rebuild never changes the level of a scene
//! centred where it is already: `LoginManager.loadRegion` returns before
//! `Player.plane = plane` (LoginManager.java:699-700, 768) when the zone is
//! the same and, in HD, whatever the level (`allLevelsAreVisible`). The next
//! cycle's `SceneGraph.checkPlaneChange` (Protocol.java:2566,
//! SceneGraph.java:2817-2824) then draws the minimap of the new level and
//! recomputes the roofs.
//!
//! Transcription tests: the packets were assembled by hand bit by bit from
//! the Java lines named (the same bytes the live upper-floor runs injected
//! through `RS09_FRAME_DUMP/inject`); no original class runs.
use rs09_client::scene_rebuild;
use rs09_protocol::game::{
    Frame,
    events::ServerEvent,
    players::{Movement, SELF, State},
    rebuild::NormalRebuild,
};

/// PLAYER_INFO with only the local player's type-3 move: 1, 3 (2 bits),
/// y (7), discard (1), plane (2), extended (0), x (7), then no other
/// players (8 bits of 0).
fn self_teleport(x: u32, y: u32, plane: u32, discard: bool) -> Vec<u8> {
    let mut bits = String::from("111");
    bits += &format!("{y:07b}{}{plane:02b}0{x:07b}00000000", u8::from(discard));
    while bits.len() % 8 != 0 {
        bits.push('0');
    }
    (0..bits.len())
        .step_by(8)
        .map(|at| u8::from_str_radix(&bits[at..at + 8], 2).unwrap())
        .collect()
}

#[test]
fn the_injected_bytes_are_the_self_teleports_they_stand_for() {
    // The live runs' `inject` lines: level 2, then back to level 1, at
    // local tile (54, 50), 3206,3210 in the scene based at 3152,3160.
    assert_eq!(self_teleport(54, 50, 2, true), [0xec, 0xb1, 0xb0, 0x00]);
    assert_eq!(self_teleport(54, 50, 1, true), [0xec, 0xa9, 0xb0, 0x00]);
}

#[test]
fn player_info_moves_the_local_player_to_another_level() {
    let mut players = State::new(55, 48);
    players.plane = 1;
    for (plane, x, y) in [(2, 54, 50), (1, 54, 50), (3, 56, 58), (0, 10, 90)] {
        let update = players
            .decode(&self_teleport(x, y, plane, true), 0)
            .unwrap();
        // Protocol.java:781: the level is read with the tile, not after it.
        assert_eq!(players.plane, plane as i32);
        assert_eq!(players.self_player().tile(), [x as i32, y as i32]);
        assert!(matches!(
            update.movements.as_slice(),
            [Movement::Teleport {
                index: SELF,
                discard: true,
                ..
            }]
        ));
    }
}

#[test]
fn teleport_local_player_carries_the_level_in_its_flags() {
    // Protocol.java:1565-1569: x (g1sub), flags (g1add), y (g1).
    let frame = Frame {
        opcode: 13,
        // g1sub is 128 - b, g1add b - 128 (Buffer.java).
        payload: vec![128 - 54, (2 << 1 | 1u8) + 128, 50],
    };
    assert_eq!(
        ServerEvent::decode(&frame).unwrap(),
        Some(ServerEvent::TeleportSelf {
            x: 54,
            y: 50,
            plane: 2,
            jump: true
        })
    );
}

#[test]
fn a_rebuild_where_the_scene_is_centred_does_not_change_the_level() {
    // LoginManager.java:699: same zone, any level in HD: nothing happens,
    // so `Player.plane` keeps what PLAYER_INFO last said.
    let rebuild = |plane| NormalRebuild {
        local_x: 54,
        local_y: 50,
        plane,
        zone_x: 400,
        zone_y: 401,
        keys: vec![],
    };
    let base = rebuild(0).base();
    for plane in 0..4 {
        assert!(scene_rebuild::unchanged(Some(base), &rebuild(plane)));
    }
    // Another zone rebuilds onto the level the packet names.
    assert!(!scene_rebuild::unchanged(
        Some([base[0] - 8, base[1]]),
        &rebuild(2)
    ));
}
