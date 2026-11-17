//! The zone packets whose effects need live actors or the sound queue:
//! SOUND_AREA (97), MAP_PROJANIM (104) and MAP_PROJANIM_3 (121).
//!
//! `Protocol.readZonePacket` handles them the same way whether they arrive on
//! their own or as children of UPDATE_ZONE_PARTIAL_ENCLOSED (230), which first
//! sets the zone origin they are relative to. The original length table gives
//! 104 no payload, so the envelope is the only way it can arrive.
use rs09_protocol::{
    Result,
    game::{
        Frame,
        events::{HalfTileProjectile, ServerEvent},
        updates::ServerUpdate,
    },
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SceneEvent {
    /// For `world_spot::State::launch`, which applies twice the zone origin.
    Projectile(HalfTileProjectile),
    /// A sound at a scene-local tile (zone origin applied), with its id,
    /// radius, loops and delay, for `sound_queue::Queue::area`.
    AreaSound { tile: [i32; 2], fields: [i32; 4] },
}

fn event(frame: &Frame, zone: [i32; 2]) -> Result<Option<SceneEvent>> {
    Ok(match ServerEvent::decode(frame)? {
        Some(ServerEvent::Projectile(packet)) => Some(SceneEvent::Projectile(packet)),
        Some(ServerEvent::AreaSound {
            x,
            y,
            id,
            radius,
            loops,
            delay,
        }) => Some(SceneEvent::AreaSound {
            tile: [zone[0] + x, zone[1] + y],
            fields: [id, radius, loops, delay],
        }),
        _ => None,
    })
}

/// The scene events of one server frame, in the order the original runs them.
/// `zone` is the current zone origin (`SceneGraph.currentChunkX/Y`); an
/// envelope replaces it before its children are read and leaves it set.
/// Frames that carry none of the three packets give an empty list.
pub fn scene_events(frame: &Frame, zone: &mut [i32; 2]) -> Result<Vec<SceneEvent>> {
    match frame.opcode {
        97 | 104 | 121 => Ok(event(frame, *zone)?.into_iter().collect()),
        230 => {
            let Some(ServerUpdate::ZoneEnclosed { x, y, frames }) = ServerUpdate::decode(frame)?
            else {
                return Ok(vec![]);
            };
            *zone = [x, y];
            let mut events = vec![];
            for child in frames
                .iter()
                .filter(|child| matches!(child.opcode, 97 | 104 | 121))
            {
                events.extend(event(child, *zone)?);
            }
            Ok(events)
        }
        _ => Ok(vec![]),
    }
}

#[cfg(test)]
mod tests {
    //! Transcription tests against `Protocol.readZonePacket` (SOUND_AREA,
    //! MAP_PROJANIM, MAP_PROJANIM_3) and the UPDATE_ZONE_PARTIAL_ENCLOSED
    //! branch of `readPacketInternal`; the original is not executed.
    use super::*;
    use crate::{
        sound_queue::{Entry, Queue},
        world_spot::{Context, State},
    };
    use rs09_protocol::buffer::Writer;

    /// MAP_PROJANIM as `readZonePacket` reads it: half-tile offset (5, 9),
    /// delta (-3, 4), launching actor, tracked actor, graphic, signed source
    /// height, target height, start and end cycles, angle and arc.
    fn projectile_104(w: &mut Writer, launcher: i32) {
        w.p1(104);
        w.p1(5 << 4 | 9);
        w.p1(-3);
        w.p1(4);
        w.p2(launcher);
        w.p2(-12);
        w.p2(90);
        w.p1(-100);
        w.p1(30);
        w.p2(5);
        w.p2(40);
        w.p1(16);
        w.p1(64);
    }
    /// SOUND_AREA at offset (3, 5): sound 50, radius 2, one loop, delay 9.
    fn sound_97(w: &mut Writer) {
        w.p1(97);
        w.p1(3 << 4 | 5);
        w.p2(50);
        w.p1(2 << 4 | 1);
        w.p1(9);
    }
    fn envelope(x: i32, y: i32, children: impl FnOnce(&mut Writer)) -> Frame {
        let mut w = Writer::new();
        w.p1add(y);
        w.p1sub(x);
        children(&mut w);
        Frame {
            opcode: 230,
            payload: w.into_vec(),
        }
    }
    fn expected_projectile() -> HalfTileProjectile {
        HalfTileProjectile {
            offset: [5, 9],
            delta: [-3, 4],
            target: -12,
            source: Some(0),
            spot: 90,
            start_offset: -100,
            end_height: 30,
            start: 5,
            end: 40,
            angle: 16,
            distance: 64,
        }
    }

    #[test]
    fn an_envelope_runs_its_projectile_and_sound_children_at_its_origin() {
        let frame = envelope(16, 24, |w| {
            projectile_104(w, 0);
            // A ground item between them belongs to another reducer.
            w.p1(33);
            w.p2(995);
            w.p1(0);
            w.p2(1);
            sound_97(w);
        });
        let mut zone = [80, 88];
        let events = scene_events(&frame, &mut zone).unwrap();
        // The envelope's origin replaces the previous one and stays set.
        assert_eq!(zone, [16, 24]);
        assert_eq!(
            events,
            [
                SceneEvent::Projectile(expected_projectile()),
                SceneEvent::AreaSound {
                    tile: [16 + 3, 24 + 5],
                    fields: [50, 2, 1, 9]
                }
            ]
        );

        // Applied as the client applies them: the projectile from twice the
        // zone origin in half tiles, the sound at the zone tile.
        let mut spots = State {
            zone,
            ..State::default()
        };
        let mut queue = Queue::default();
        for event in &events {
            match event {
                SceneEvent::Projectile(packet) => spots.launch(
                    packet,
                    Context {
                        origin: [3200, 3400],
                        plane: 0,
                        cycle: 1000,
                    },
                    |_, _, _| panic!("launcher 0 names no actor"),
                    &mut |_, _| -500,
                ),
                SceneEvent::AreaSound { tile, fields } => queue.area(*tile, [20, 30], *fields, 127),
            }
        }
        let shot = &spots.projectiles[0];
        // (16 * 2 + 5, 24 * 2 + 9) half tiles of 64 units; the launch height
        // is the terrain height less the signed source height (-100).
        assert_eq!(
            shot.position,
            [
                f64::from(37 * 64 + 3200 * 128),
                f64::from(57 * 64 + 3400 * 128),
                -400.0
            ]
        );
        assert_eq!(
            shot.destination,
            [34 * 64 + 3200 * 128, 61 * 64 + 3400 * 128]
        );
        assert_eq!((shot.start, shot.end), (1005, 1040));
        assert_eq!(
            queue.entries,
            [Entry {
                id: 50,
                loops: 1,
                delay: 9,
                position: 2 + (19 << 16) + (29 << 8),
                start: None
            }]
        );
    }

    #[test]
    fn top_level_packets_use_the_origin_already_set() {
        let mut w = Writer::new();
        sound_97(&mut w);
        let payload = w.into_vec();
        let frame = Frame {
            opcode: 97,
            payload: payload[1..].to_vec(),
        };
        let mut zone = [40, 48];
        assert_eq!(
            scene_events(&frame, &mut zone).unwrap(),
            [SceneEvent::AreaSound {
                tile: [43, 53],
                fields: [50, 2, 1, 9]
            }]
        );
        assert_eq!(zone, [40, 48]);

        // MAP_PROJANIM_3 has no launching actor and an unsigned height times four.
        let mut w = Writer::new();
        w.p1(5 << 4 | 9);
        w.p1(-3);
        w.p1(4);
        w.p2(-12);
        w.p2(90);
        w.p1(200);
        w.p1(30);
        w.p2(5);
        w.p2(40);
        w.p1(255);
        w.p1(64);
        let frame = Frame {
            opcode: 121,
            payload: w.into_vec(),
        };
        assert_eq!(
            scene_events(&frame, &mut zone).unwrap(),
            [SceneEvent::Projectile(HalfTileProjectile {
                source: None,
                start_offset: 800,
                angle: -1,
                ..expected_projectile()
            })]
        );

        // 104 has no top-level length, so a top-level frame carries nothing;
        // other opcodes are not scene events.
        for opcode in [104, 16, 17, 26, 112, 70] {
            let frame = Frame {
                opcode,
                payload: vec![],
            };
            assert!(
                scene_events(&frame, &mut zone).unwrap().is_empty(),
                "{opcode}"
            );
        }
        assert_eq!(zone, [40, 48]);
    }

    #[test]
    fn an_envelope_without_these_children_only_moves_the_origin() {
        let mut zone = [0, 0];
        let frame = envelope(8, 96, |w| {
            w.p1(33);
            w.p2(995);
            w.p1(0);
            w.p2(1);
        });
        assert!(scene_events(&frame, &mut zone).unwrap().is_empty());
        assert_eq!(zone, [8, 96]);
        // A child cut short is an error, as it is for the other reducers.
        let frame = envelope(8, 96, |w| {
            w.p1(97);
            w.p1(0);
        });
        assert!(scene_events(&frame, &mut zone).is_err());
        // The launching actor of 104 is kept for the caller to resolve.
        let frame = envelope(8, 96, |w| projectile_104(w, -5));
        let events = scene_events(&frame, &mut zone).unwrap();
        assert_eq!(
            events,
            [SceneEvent::Projectile(HalfTileProjectile {
                source: Some(-5),
                ..expected_projectile()
            })]
        );
    }
}
