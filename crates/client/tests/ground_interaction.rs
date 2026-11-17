use rs09_client::{
    ground_interaction,
    interaction::{Menu, MenuEntry},
    ui::{actions::SelectedItem, item::ItemType},
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::PathBuf};

fn n(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}
fn pair(v: &Value) -> [i32; 2] {
    [n(&v[0]), n(&v[1])]
}
fn text(out: &mut Vec<u8>, s: &str) {
    out.extend((s.len() as i32).to_be_bytes());
    out.extend(s.as_bytes());
}

#[test]
fn original_ground_menu_order_cursors_boundaries_and_six_action_branches() {
    let f: Value = serde_json::from_slice(
        &std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/ui/ground-interaction.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let mut counts = [0; 2];
    for (index, case) in f["cases"].as_array().unwrap().iter().enumerate() {
        let actual = if case["kind"] == "menu" {
            counts[0] += 1;
            let mut menu = Menu::default();
            for _ in 0..n(&case["initial"]) {
                menu.add(MenuEntry {
                    option: "Prefix".into(),
                    target: String::new(),
                    action: 1005,
                    key: 0,
                    args: [0; 2],
                    cursor: -1,
                });
            }
            menu.open = case["opened"].as_bool().unwrap();
            let selected = case["selected"].as_str().map(|text| SelectedItem {
                id: 0,
                slot: 0,
                component: 0,
                text: text.into(),
            });
            let mut catalog = BTreeMap::new();
            let items = case["items"].as_array().unwrap();
            for item in items {
                let d = ItemType {
                    id: n(&item["id"]),
                    name: item["name"].as_str().unwrap().into(),
                    ops: item["ops"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_str().map(String::from))
                        .collect(),
                    cursor1Op: n(&item["cursors"][0]),
                    cursor1: n(&item["cursors"][1]),
                    cursor2Op: n(&item["cursors"][2]),
                    cursor2: n(&item["cursors"][3]),
                    ..ItemType::default()
                };
                catalog.insert(d.id, d);
            }
            for item in items.iter().rev() {
                ground_interaction::entries(
                    &mut menu,
                    &catalog[&n(&item["id"])],
                    pair(&case["tile"]),
                    selected.as_ref(),
                );
            }
            menu.sort();
            let mut out = Vec::new();
            out.extend((menu.entries.len() as i32).to_be_bytes());
            for entry in menu.entries {
                text(&mut out, &entry.option);
                text(&mut out, &entry.target);
                out.extend(entry.action.to_be_bytes());
                out.extend(entry.key.to_be_bytes());
                for value in [entry.args[0], entry.args[1], entry.cursor] {
                    out.extend(value.to_be_bytes());
                }
            }
            format!("{:x}", Sha256::digest(out))
        } else {
            counts[1] += 1;
            let selected = &case["selected"];
            let frame = ground_interaction::packet(
                n(&case["action"]),
                n(&case["id"]),
                pair(&case["tile"]),
                Some(&SelectedItem {
                    id: n(&selected["id"]),
                    slot: n(&selected["slot"]),
                    component: n(&selected["component"]),
                    text: String::new(),
                }),
            )
            .unwrap();
            std::iter::once(frame.opcode)
                .chain(frame.payload)
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        };
        assert_eq!(actual, case["expected"].as_str().unwrap(), "case{index}");
    }
    assert_eq!(counts, [512, 1024]);
    // Action 34, the item's second ground option, is not in the fixture. It
    // sends opcode 109 as ip2(y), p2(x), ip2add(id) (MiniMenu.java:821-838);
    // these bytes are transcribed by hand, not produced by the original.
    for action in [34, 2034] {
        let second = ground_interaction::packet(action, 995, [3222, 3240], None).unwrap();
        assert_eq!(second.opcode, 109);
        assert_eq!(second.payload, [0xa8, 0x0c, 0x0c, 0x96, 0x63, 0x03]);
    }
    assert!(ground_interaction::packet(33, 100, [50; 2], None).is_none());
    assert!(ground_interaction::packet(35, 100, [50; 2], None).is_none());
}
