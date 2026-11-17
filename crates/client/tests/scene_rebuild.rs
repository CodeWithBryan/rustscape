use rs09_client::world::rebuild_logical;
use rs09_protocol::{
    Result,
    buffer::Writer,
    game::{
        Frame,
        ground::{self, Info, Tile},
        players, rebuild, updates,
    },
};
use std::collections::BTreeMap;
struct Context {
    base: [i32; 2],
    heights: Vec<(i32, [i32; 2])>,
}
impl ground::Context for Context {
    fn info(&mut self, _: &mut ground::State, _: i32) -> Result<Info> {
        Ok(Info {
            cost: 1,
            stackable: 1,
        })
    }
    fn height(&mut self, _: &mut ground::State, plane: i32, x: i32, y: i32) -> Result<i32> {
        self.heights
            .push((plane, [self.base[0] * 128 + x, self.base[1] * 128 + y]));
        Ok(-plane * 1000)
    }
}
fn packet(plane: i32) -> Frame {
    let mut w = Writer::new();
    w.p2add(34);
    // Four region-key rows for this ordinary rebuild; key lookup is external.
    for _ in 0..16 {
        w.p4(0);
    }
    w.p1sub(plane);
    w.p2(401);
    w.p2add(401);
    w.p2add(46);
    Frame {
        opcode: 162,
        payload: w.into_vec(),
    }
}
#[test]
fn upper_plane_rebuild_publishes_new_base_plane_and_queue_before_ground_updates() {
    for plane in [0, 1, 3] {
        let mut players = players::State::new(40, 40);
        let mut ground = ground::State::default();
        ground.self_id = 7;
        let mut context = Context {
            base: [3152, 3168],
            heights: Vec::new(),
        };
        ground
            .apply_frame(
                &updates::zone_enclosed(40, 40, &[updates::ground_add(0, 0, 995, 1)]).unwrap(),
                &mut context,
            )
            .unwrap();
        let old_tile = Tile {
            plane: 0,
            x: 40,
            y: 40,
        };
        let retained = ground.stack(old_tile).unwrap().unwrap().items()[0].clone();
        assert!(ground.pile(old_tile).is_some());
        let mut supports = BTreeMap::from([(old_tile, 96)]);
        let mut base = Some(context.base);
        let rebuild = rebuild::decode_normal(&packet(plane)).unwrap();
        let next = rebuild_logical(
            &rebuild,
            &mut base,
            &mut players,
            &mut ground,
            &mut supports,
        );
        assert_eq!(next, [3160, 3160]);
        assert_eq!(base, Some(next));
        assert_eq!(players.plane, plane);
        assert_eq!(ground.plane, plane);
        assert_eq!(ground.self_id, 7);
        assert_eq!(players.self_player().tile(), [34, 46]);
        assert_eq!(players.self_player().queue_size, 1);
        assert_eq!(
            [
                players.self_player().queue_x[1],
                players.self_player().queue_y[1]
            ],
            [32, 48]
        );
        assert!(supports.is_empty());
        assert_eq!(ground.piles().count(), 0);
        let shifted = Tile {
            plane: 0,
            x: 32,
            y: 48,
        };
        assert_eq!(ground.stack(shifted).unwrap().unwrap().items()[0], retained);
        context.base = next;
        context.heights.clear();
        ground
            .apply_frame(
                &updates::zone_enclosed(
                    32,
                    40,
                    &[
                        updates::ground_add(3, 6, 1333, 1),
                        updates::ground_add_excluding(4, 6, 526, 1, 7),
                    ],
                )
                .unwrap(),
                &mut context,
            )
            .unwrap();
        let target = Tile {
            plane,
            x: 35,
            y: 46,
        };
        assert_eq!(ground.pile(target).unwrap().primary.get().id, 1333);
        assert!(
            ground
                .stack(Tile {
                    plane,
                    x: 36,
                    y: 46
                })
                .unwrap()
                .is_none()
        );
        assert_eq!(
            context.heights,
            [(plane, [3195 * 128 + 64, 3206 * 128 + 64])]
        );
        if plane != 0 {
            assert!(
                ground
                    .stack(Tile {
                        plane: 0,
                        x: 35,
                        y: 46
                    })
                    .unwrap()
                    .is_none()
            );
        }
        assert!(ground.pile(old_tile).is_none());
    }
}

/// Transcription test of the rest of `LoginManager.loadRegion` (the
/// unchanged-zone return, `ChangeLocRequest` pruning, `Camera.cameraType = 1`,
/// the map flag shift and the spot animation and projectile lists). Expected
/// values are worked out by hand from the Java, which is not executed.
#[test]
fn rebuild_drops_what_load_region_drops_and_keeps_the_map_flag_on_its_tile() {
    use rs09_client::{
        camera,
        scene_rebuild::{self, Parts},
        ui::vm::CameraRequest,
        walking::DestinationFlag,
        world::{Location, Scene, Slot},
        world_spot,
    };
    use rs09_protocol::game::events::{HalfTileProjectile, ServerEvent};

    // The packet centres the scene on zone (401, 401): base (3160, 3160).
    let rebuild = rebuild::decode_normal(&packet(0)).unwrap();
    assert!(!scene_rebuild::unchanged(None, &rebuild));
    assert!(!scene_rebuild::unchanged(Some([3152, 3168]), &rebuild));
    assert!(!scene_rebuild::unchanged(Some([3160, 3168]), &rebuild));
    // With every level visible the plane is not compared.
    for plane in [0, 1, 3] {
        let rebuild = rebuild::decode_normal(&packet(plane)).unwrap();
        assert!(scene_rebuild::unchanged(Some([3160, 3160]), &rebuild));
    }

    let previous = [3152, 3168];
    let base = rebuild.base();
    assert_eq!(base, [3160, 3160]);
    let mut scene = Scene::default();
    let tiles = [
        // Local (0, 0) and (103, 103) are the corners of the new scene.
        ([3160, 3160], 0, true),
        ([3263, 3263], 3, true),
        // One tile outside on each side.
        ([3159, 3200], 0, false),
        ([3264, 3200], 0, false),
        ([3200, 3159], 1, false),
        ([3200, 3264], 0, false),
        // Inside the old scene (base 3152, 3168) but west of the new one.
        ([3155, 3170], 0, false),
    ];
    for (tile, plane, _) in tiles {
        scene.push(
            Slot {
                plane,
                tile,
                layer: 2,
            },
            Location {
                id: 5,
                shape: 10,
                rotation: 0,
            },
            0,
            -1,
        );
    }
    let mut spots = world_spot::State {
        zone: [8, 16],
        ..Default::default()
    };
    spots.effects.push(world_spot::Effect {
        id: 1,
        plane: 0,
        fine: [64, 64],
        z: 0,
        start: 0,
        sequence: -1,
        frame: 0,
        clock: 0,
        finished: false,
    });
    spots.launch(
        &HalfTileProjectile {
            offset: [1, 1],
            delta: [1, 1],
            target: 0,
            source: None,
            spot: 90,
            start_offset: 0,
            end_height: 0,
            start: 0,
            end: 10,
            angle: 0,
            distance: 0,
        },
        world_spot::Context {
            origin: previous,
            plane: 0,
            cycle: 0,
        },
        |_, _, _| None,
        &mut |_, _| 0,
    );
    assert_eq!((spots.effects.len(), spots.projectiles.len()), (1, 1));

    // A locked camera with a shake running.
    let mut locked = camera::Script::default();
    locked
        .apply(
            &ServerEvent::CameraMoveTo {
                counter: 0,
                x: 10,
                y: 10,
                height: 300,
                min_step: 1,
                speed: 100,
            },
            |_| 0,
        )
        .unwrap();
    locked
        .apply(
            &ServerEvent::CameraShake {
                counter: 0,
                axis: 1,
                jitter: 2,
                amplitude: 3,
                frequency: 4,
                phase: 5,
            },
            |_| 0,
        )
        .unwrap();
    assert!(locked.locked);
    let shakes = locked.shakes;
    let render = locked.render;

    let mut flag = DestinationFlag { tile: [50, 60] };
    scene_rebuild::apply(
        Some(previous),
        base,
        Parts {
            changes: &mut scene,
            spots: &mut spots,
            camera: &mut locked,
            flag: &mut flag,
        },
    );
    let kept: Vec<[i32; 2]> = scene.changes.iter().map(|c| c.slot.tile).collect();
    let expected: Vec<[i32; 2]> = tiles
        .iter()
        .filter(|(_, _, kept)| *kept)
        .map(|(tile, _, _)| *tile)
        .collect();
    assert_eq!(kept, expected);
    assert!(spots.effects.is_empty() && spots.projectiles.is_empty());
    // The zone origin is not part of the scene lists.
    assert_eq!(spots.zone, [8, 16]);
    // The camera follows again; shakes and its position are not reset.
    assert!(!locked.locked && locked.spline.is_none());
    assert_eq!((locked.shakes, locked.render), (shakes, render));
    // The base moved by (+8, -8), so the same tile is now local (42, 68).
    assert_eq!(flag.tile, [42, 68]);

    // A spline camera (type 3) is ended too.
    let mut spline = camera::Script::default();
    spline.script(
        CameraRequest::Spline {
            position: [[0; 3]; 4],
            look: [[0; 3]; 4],
            speed_start: 1,
            speed_end: 1,
        },
        previous,
        |_| 0,
    );
    assert!(spline.spline.is_some());
    // A hidden flag (X of zero) is not moved, and a flag whose X lands on
    // zero becomes hidden, as in the original.
    for (before, after) in [([0, 60], [0, 60]), ([8, 60], [0, 68]), ([9, 0], [1, 8])] {
        let mut flag = DestinationFlag { tile: before };
        scene_rebuild::apply(
            Some(previous),
            base,
            Parts {
                changes: &mut scene,
                spots: &mut spots,
                camera: &mut spline,
                flag: &mut flag,
            },
        );
        assert_eq!(flag.tile, after);
    }
    assert!(spline.spline.is_none());
    // The first scene of a session has no previous base to shift from.
    let mut flag = DestinationFlag { tile: [50, 60] };
    scene_rebuild::apply(
        None,
        base,
        Parts {
            changes: &mut scene,
            spots: &mut spots,
            camera: &mut spline,
            flag: &mut flag,
        },
    );
    assert_eq!(flag.tile, [50, 60]);
    assert_eq!(scene.changes.len(), 2);
}
