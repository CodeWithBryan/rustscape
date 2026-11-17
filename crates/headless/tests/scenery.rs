use rs09_headless::{
    protocol::{
        buffer::Reader,
        game::{self, ClientAction, Direction, updates},
    },
    scenery::{Event, Observer},
    session::SceneryAction,
};
use sha2::{Digest, Sha256};
#[test]
fn every_original_rt4_location_writer_case_uses_the_shared_graphics_free_api() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/visual/oracle.json")).unwrap();
    let mut count = 0;
    for c in fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == "locaction")
    {
        let wire = game::actions::location_packet(
            c["action"].as_i64().unwrap() as i32,
            c["location"].as_i64().unwrap() as i32,
            [
                c["tile"][0].as_i64().unwrap() as i32,
                c["tile"][1].as_i64().unwrap() as i32,
            ],
        )
        .unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(wire)),
            c["expected"].as_str().unwrap(),
            "{}",
            c["id"]
        );
        count += 1;
    }
    assert_eq!(count, 48);
}
#[test]
fn public_scenery_actions_roundtrip_all_original_options_and_keep_real_live_bytes() {
    for option in 0..5 {
        let action = SceneryAction {
            option,
            id: 12348,
            tile: [3207, 3217],
        };
        let frame = action.frame().unwrap();
        assert_eq!(
            ClientAction::decode(frame.opcode, &mut Reader::new(&frame.payload)).unwrap(),
            Some(ClientAction::SceneryAction {
                opt_index: i32::from(option),
                id: 12348,
                x: 3207,
                y: 3217
            })
        );
    }
    let mut wire = vec![];
    SceneryAction {
        option: 0,
        id: 12348,
        tile: [3207, 3217],
    }
    .frame()
    .unwrap()
    .write(Direction::ClientToServer, None, &mut wire)
    .unwrap();
    assert_eq!(wire, [254, 135, 12, 48, 188, 12, 145]);
    assert!(
        SceneryAction {
            option: 5,
            id: 12348,
            tile: [3207, 3217]
        }
        .frame()
        .is_err()
    );
}
#[test]
fn unchanged_original_scene_wire_reaches_absolute_observations_once() {
    // First original scene case: rotation0 opens toward the west. This literal
    // is retained in the unchanged JVM scene corpus, not authored by this API.
    let corpus = include_str!("../../../fixtures/scene/oracle.jsonl");
    let pin: serde_json::Value =
        serde_json::from_str(include_str!("../../../fixtures/scene/provenance.json")).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(corpus)),
        pin["fixtureSha256"]
    );
    assert!(corpus.contains("e6000ab050c30071b3816128d7"));
    let bytes = [230, 0, 10, 176, 80, 195, 0, 113, 179, 129, 97, 40, 215];
    let mut decoder = game::Decoder::new(Direction::ServerToClient, 65536, 65535);
    for byte in bytes {
        decoder.push(&[byte]).unwrap();
    }
    let frame = decoder.next_frame().unwrap().unwrap();
    let mut observer = Observer::default();
    observer.frame(&frame, 2, Some([3152, 3168]), 3).unwrap();
    let events = observer.events().collect::<Vec<_>>();
    assert_eq!(events.len(), 2);
    assert_eq!(observer.zone(), [48, 48]);
    match events[0].event {
        Event::Location { slot, location, .. } => {
            assert_eq!(slot.plane, 2);
            assert_eq!(slot.tile, [3207, 3217]);
            assert!(location.is_none());
        }
        _ => panic!("not original remove"),
    }
    match events[1].event {
        Event::Location { slot, location, .. } => {
            assert_eq!(slot.tile, [3206, 3217]);
            let l = location.unwrap();
            assert_eq!((l.id, l.shape, l.rotation), (10327, 0, 1));
        }
        _ => panic!("not original add"),
    }
}
#[test]
fn origins_rebuilds_resets_and_late_failures_preserve_observed_prefixes() {
    let mut observer = Observer::default();
    observer
        .frame(&updates::zone_origin(48, 48, false), 0, None, 0)
        .unwrap();
    let frame = updates::loc_add(7, 1, 0, 1, 12348);
    assert!(observer.frame(&frame, 0, None, 0).is_err());
    assert_eq!(observer.events().count(), 0);
    observer.frame(&frame, 0, Some([3152, 3168]), 1).unwrap();
    let old = observer.events().next().unwrap().clone();
    observer.frame(&frame, 3, Some([3160, 3176]), 2).unwrap();
    observer
        .frame(
            &updates::zone_origin(40, 48, true),
            3,
            Some([3160, 3176]),
            2,
        )
        .unwrap();
    assert!(matches!(
        observer.events().last().unwrap().event,
        Event::ResetChunk {
            plane: 3,
            origin: [3200, 3224]
        }
    ));
    assert!(matches!(old.event,Event::Location{slot,..}if slot.tile==[3207,3217]&&slot.plane==0));
    let enclosed = updates::zone_enclosed(
        48,
        48,
        &[
            updates::loc_del(7, 1, 0, 1),
            updates::loc_add(7, 2, 63, 0, 10327),
        ],
    )
    .unwrap();
    assert!(observer.frame(&enclosed, 3, Some([3152, 3168]), 3).is_err());
    assert_eq!(observer.events().count(), 4);
    assert!(matches!(
        observer.events().last().unwrap().event,
        Event::Location { location: None, .. }
    ));
}
#[test]
fn private_sound_fields_and_bounded_history_are_data_without_synthesis() {
    let mut observer = Observer::default();
    for i in 0..300 {
        observer
            .frame(&updates::synth_sound(62, 1, i), 0, None, i as usize)
            .unwrap();
    }
    assert_eq!(observer.events().count(), 256);
    assert_eq!(observer.omitted(), 44);
    assert!(matches!(
        observer.events().next().unwrap().event,
        Event::Sound {
            id: 62,
            loops: 1,
            delay: 44
        }
    ));
    assert_eq!(observer.drain().count(), 256);
    assert_eq!(observer.events().count(), 0);
}
