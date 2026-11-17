use rs09_client::{ground_geometry, interaction::LocationType};
use serde_json::Value;
use std::path::PathBuf;
fn n(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}
#[test]
fn original_ground_height_bridge_wrapping_support_and_metadata() {
    let f: Value = serde_json::from_slice(
        &std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/ui/ground-geometry.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let mut counts = [0; 3];
    for (index, c) in f["cases"].as_array().unwrap().iter().enumerate() {
        let actual = match c["kind"].as_str().unwrap() {
            "metadata" => {
                counts[0] += 1;
                let s = c["bytes"].as_str().unwrap();
                let raw: Vec<_> = (0..s.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
                    .collect();
                LocationType::decode(&raw).unwrap().support_items
            }
            "height" => {
                counts[1] += 1;
                let fine = [n(&c["point"][0]), n(&c["point"][1])];
                let [x, y] = fine.map(|n| n >> 7);
                ground_geometry::tile_height(n(&c["plane"]), fine, |plane, tx, ty| {
                    let at = (tx - x) + (ty - y) * 2;
                    assert!((0..4).contains(&at));
                    (
                        n(&c["heights"][plane as usize][at as usize]),
                        n(&c["flags"]) as u8,
                    )
                })
            }
            "support" => {
                counts[2] += 1;
                ground_geometry::support_height(
                    c["supports"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| (v[0].as_i64().unwrap() & 0x400000 == 0x400000, n(&v[1]))),
                )
            }
            _ => panic!("unknown case"),
        };
        assert_eq!(actual, n(&c["expected"]), "case{index}");
    }
    assert_eq!(counts, [512, 1024, 512]);
}
