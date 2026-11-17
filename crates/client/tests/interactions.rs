use rs09_client::interaction::{self, LocationTarget, LocationType, Menu};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
fn input(name: &str) -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/visual")
            .join(name),
    )
    .unwrap()
}
fn integer(bytes: &mut Vec<u8>, value: i32) {
    bytes.extend(value.to_be_bytes());
}
fn text(bytes: &mut Vec<u8>, value: &str) {
    integer(bytes, value.len() as i32);
    bytes.extend(value.as_bytes());
}

#[test]
fn cache_defined_object_options_priority_and_original_action_bytes_match_rt4() {
    let fixture: Value = serde_json::from_slice(&input("oracle.json")).unwrap();
    let mut counts = [0; 3];
    for c in fixture["cases"].as_array().unwrap() {
        let kind = c["kind"].as_str().unwrap();
        if !matches!(kind, "locinfo" | "locmenu" | "locaction") {
            continue;
        }
        let mut bytes = Vec::new();
        if kind == "locaction" {
            counts[2] += 1;
            bytes = interaction::location_packet(
                c["action"].as_i64().unwrap() as i32,
                c["location"].as_i64().unwrap() as i32,
                [
                    c["tile"][0].as_i64().unwrap() as i32,
                    c["tile"][1].as_i64().unwrap() as i32,
                ],
            )
            .unwrap();
        } else {
            let d = LocationType::decode(&input(c["file"].as_str().unwrap())).unwrap();
            if kind == "locinfo" {
                counts[0] += 1;
                text(&mut bytes, &d.name);
                for v in [
                    d.size[0],
                    d.size[1],
                    d.blocked_sides,
                    d.interactable,
                    d.cursors[0].0,
                    d.cursors[0].1,
                    d.cursors[1].0,
                    d.cursors[1].1,
                ] {
                    integer(&mut bytes, v);
                }
                for option in &d.options {
                    if let Some(option) = option {
                        text(&mut bytes, option);
                    } else {
                        integer(&mut bytes, -1);
                    }
                }
            } else {
                counts[1] += 1;
                let mut menu = Menu::default();
                menu.location(
                    &d,
                    LocationTarget {
                        id: 12345,
                        tile: [52, 53],
                        plane: 0,
                        shape: 10,
                        rotation: 0,
                        size: [1; 2],
                        blocked_sides: 0,
                    },
                    [0; 2],
                );
                menu.sort();
                integer(&mut bytes, menu.entries.len() as i32);
                for entry in menu.entries {
                    text(&mut bytes, &entry.option);
                    text(&mut bytes, &entry.target);
                    integer(&mut bytes, entry.action);
                    bytes.extend(entry.key.to_be_bytes());
                    integer(&mut bytes, entry.args[0]);
                    integer(&mut bytes, entry.args[1]);
                    integer(&mut bytes, entry.cursor);
                }
            }
        }
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            c["expected"].as_str().unwrap(),
            "{kind}: {} {}",
            c["id"],
            c["file"]
        );
    }
    assert_eq!(counts, [793, 793, 48]);
}

#[test]
fn delayed_replacements_reversion_asset_readiness_and_flush_match_original_scene_queue() {
    use rs09_client::world::{Location, Scene, Slot};
    use std::collections::BTreeSet;
    let fixture: Value = serde_json::from_slice(&input("oracle.json")).unwrap();
    let mut count = 0;
    for c in fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["kind"] == "locchanges")
    {
        count += 1;
        let mut scene = Scene::default();
        let mut unready = BTreeSet::new();
        scene.locations.insert(
            Slot {
                plane: 0,
                tile: [50, 50],
                layer: 0,
            },
            Location {
                id: 100,
                shape: 0,
                rotation: 0,
            },
        );
        let mut bytes = Vec::new();
        for command in c["commands"].as_array().unwrap() {
            let a: Vec<_> = command
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap() as i32)
                .collect();
            let mut applied = Vec::new();
            match a[0] {
                0 => scene.push(
                    Slot {
                        plane: a[1] as usize,
                        tile: [a[2], a[3]],
                        layer: a[4] as u8,
                    },
                    Location {
                        id: a[5],
                        shape: a[6] as u8,
                        rotation: a[7] as u8,
                    },
                    a[8],
                    a[9],
                ),
                1 => applied = scene.advance([0; 2], |loc| !unready.contains(&loc.id)),
                2 => {
                    if a[2] == 0 {
                        unready.insert(a[1]);
                    } else {
                        unready.remove(&a[1]);
                    }
                }
                3 => scene.flush(),
                _ => unreachable!(),
            }
            integer(&mut bytes, scene.locations.len() as i32);
            for (slot, loc) in &scene.locations {
                for v in [
                    slot.plane as i32,
                    slot.tile[0],
                    slot.tile[1],
                    i32::from(slot.layer),
                    loc.id,
                    i32::from(loc.shape),
                    i32::from(loc.rotation),
                ] {
                    integer(&mut bytes, v);
                }
            }
            integer(&mut bytes, scene.changes.len() as i32);
            for change in &scene.changes {
                for v in [
                    change.slot.plane as i32,
                    change.slot.tile[0],
                    change.slot.tile[1],
                    i32::from(change.slot.layer),
                    change.original.id,
                    i32::from(change.original.shape),
                    i32::from(change.original.rotation),
                    change.replacement.id,
                    i32::from(change.replacement.shape),
                    i32::from(change.replacement.rotation),
                    change.set_loops,
                    change.reset_loops,
                ] {
                    integer(&mut bytes, v);
                }
            }
            integer(&mut bytes, applied.len() as i32);
            for (slot, loc) in applied {
                for v in [
                    loc.id,
                    slot.tile[0],
                    slot.plane as i32,
                    i32::from(loc.rotation),
                    slot.tile[1],
                    i32::from(loc.shape),
                    i32::from(slot.layer),
                ] {
                    integer(&mut bytes, v);
                }
            }
        }
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            c["expected"].as_str().unwrap(),
            "{}",
            c["id"]
        );
    }
    assert_eq!(count, 36);
}

#[test]
fn workbench_ray_rejects_occluded_behind_parallel_and_missed_triangles() {
    use rs09_client::world::Ray;
    let ray = Ray {
        origin: [0.0, 0.0, 0.0],
        direction: [0.0, 0.0, 1.0],
    };
    assert_eq!(
        ray.triangle([-1.0, -1.0, 4.0], [1.0, -1.0, 4.0], [0.0, 1.0, 4.0]),
        Some(4.0)
    );
    assert!(
        ray.triangle([-1.0, -1.0, -4.0], [1.0, -1.0, -4.0], [0.0, 1.0, -4.0])
            .is_none()
    );
    assert!(
        ray.triangle([2.0, -1.0, 4.0], [4.0, -1.0, 4.0], [3.0, 1.0, 4.0])
            .is_none()
    );
    assert!(
        ray.triangle([2.0, -1.0, 0.0], [2.0, -1.0, 4.0], [2.0, 1.0, 4.0])
            .is_none()
    );
    assert!(ray.bounds([-1.0, -1.0, 2.0], [1.0, 1.0, 4.0]));
    assert!(!ray.bounds([2.0, -1.0, 2.0], [4.0, 1.0, 4.0]));
    assert!(!ray.bounds([-1.0, -1.0, -4.0], [1.0, 1.0, -2.0]));
}
