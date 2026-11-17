use rs09_client::ui::inventory_scroll::{self, Parent};
use serde_json::{Value, json};

#[test]
fn original_selected_inventory_draw_scroll_and_failed_parent_prefixes_match() {
    use sha2::{Digest, Sha256};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let corpus: Value = serde_json::from_slice(
        &std::fs::read(root.join("fixtures/ui/inventory-scroll.json")).unwrap(),
    )
    .unwrap();
    for (name, hash) in corpus["sources"].as_object().unwrap() {
        let text = std::fs::read_to_string(
            root.join("reference/client/client/src/main/java/rt4")
                .join(name),
        )
        .unwrap()
        .replace("\r\n", "\n");
        assert_eq!(
            format!("{:x}", Sha256::digest(text.as_bytes())),
            hash.as_str().unwrap()
        );
    }
    assert_eq!(corpus["cases"].as_array().unwrap().len(), 4096);
    for (index, row) in corpus["cases"].as_array().unwrap().iter().enumerate() {
        let a: Vec<i32> = serde_json::from_value(row["arguments"].clone()).unwrap();
        let mut parent = Parent {
            scroll: a[0],
            range: a[1],
            height: a[2],
        };
        let mut press = [a[3], a[4]];
        let mut log = Vec::new();
        let mut error = None;
        if a[14] != 0 && a[15] == a[17] {
            let d = inventory_scroll::delta([a[5], a[6]], press, a[7]);
            log.push(json!([
                0,
                a[18].wrapping_add(d[0]),
                a[19].wrapping_add(d[1]),
                128
            ]));
            match a[16] {
                2 => error = Some("ArrayIndexOutOfBoundsException"),
                3 => {}
                _ => {
                    if inventory_scroll::edge_scroll(
                        if a[16] == 1 { None } else { Some(&mut parent) },
                        &mut press[1],
                        a[19],
                        d[1],
                        if a[13] != 0 {
                            [a[9], a[10]]
                        } else {
                            [a[11], a[12]]
                        },
                        a[8],
                        |s, p| log.push(json!([1, s, p])),
                    )
                    .is_err()
                    {
                        error = Some("NullPointerException");
                    }
                }
            }
        }
        assert_eq!(
            json!([log, parent.scroll, press[0], press[1], error]),
            row["expected"],
            "selected inventory case {index}"
        );
    }
}

#[test]
fn actual_controller_uses_original_hover_then_pressed_slot_contract() {
    use rs09_client::{
        interaction::MenuEntry,
        pack::Pack,
        ui::{
            Component, Properties,
            drag::Controller,
            state::{DrawNode, Key, Rect, State},
        },
    };
    let corpus: Value =
        serde_json::from_str(include_str!("../../../fixtures/ui/input.json")).unwrap();
    let m = br#"{"source":"controlled native drag controller bindings","files":{}}"#;
    let mut raw = b"RS09VIS1".to_vec();
    raw.extend((m.len() as u32).to_le_bytes());
    raw.extend(m);
    let mut verified = 0;
    for row in corpus["cases"].as_array().unwrap() {
        let a: Vec<i32> = serde_json::from_value(row["args"].clone()).unwrap();
        let ids: Vec<i32> = serde_json::from_value(row["ids"].clone()).unwrap();
        let counts: Vec<i32> = serde_json::from_value(row["counts"].clone()).unwrap();
        let (hover, pressed) = (a[3], a[4]);
        if a[2] & (3 << 28) == 0 || ids[pressed as usize] <= 0 {
            continue;
        }
        let mut state = State::new(Pack::decode(raw.clone()).unwrap());
        let key = Key::component(a[0]);
        state.loaded.insert((a[0] as u32 >> 16) as i32);
        state.components.insert(
            key,
            Component {
                id: key.id,
                kind: 2,
                client_code: a[1],
                base_width: ids.len() as i32,
                base_height: 1,
                width: ids.len() as i32 * 32,
                height: 32,
                obj_types: Some(ids),
                obj_counts: Some(counts),
                properties: Properties {
                    events: a[2],
                    target_param: -1,
                },
                ..Default::default()
            },
        );
        let rect = Rect {
            x: 0,
            y: 0,
            w: 2000,
            h: 100,
        };
        let nodes = [DrawNode {
            key,
            rect,
            clip: rect,
            paint_clip: rect,
            subtree_end: 1,
        }];
        let press = [pressed * 32 + 16, 16];
        let point = [hover * 32 + 16, 16];
        let mut controller = Controller::default();
        assert!(controller.begin(
            &state,
            &nodes,
            press,
            0,
            Some(MenuEntry {
                option: "Drop".into(),
                target: String::new(),
                action: 23,
                key: 0,
                args: [pressed, key.id],
                cursor: -1
            })
        ));
        controller.step(&mut state, &nodes, point, 6, true, a[5] == 1);
        let (_, _, release) = controller.step(&mut state, &nodes, point, 7, false, a[5] == 1);
        let packet = release.unwrap().resolve(&state).unwrap();
        let wire = std::iter::once(packet.opcode)
            .chain(packet.payload)
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert_eq!(wire, row["reorder"]);
        assert_eq!(json!(state.components[&key].obj_types), row["expected_ids"]);
        assert_eq!(
            json!(state.components[&key].obj_counts),
            row["expected_counts"]
        );
        verified += 1;
    }
    assert!(
        verified > 100,
        "actual positive-item drag bindings must cover the original corpus"
    );
}

#[test]
fn render_feedback_keeps_selected_icon_stationary_while_its_parent_scrolls() {
    use rs09_client::{
        interaction::MenuEntry,
        pack::Pack,
        ui::{
            Component, Properties,
            drag::Controller,
            state::{DrawNode, Key, Rect, State},
        },
    };
    let m = br#"{"source":"controlled inventory presentation feedback","files":{}}"#;
    let mut raw = b"RS09VIS1".to_vec();
    raw.extend((m.len() as u32).to_le_bytes());
    raw.extend(m);
    let mut state = State::new(Pack::decode(raw).unwrap());
    let key = Key::component(149 << 16);
    state.loaded.insert(149);
    state.components.insert(
        key,
        Component {
            id: key.id,
            kind: 2,
            base_width: 1,
            base_height: 1,
            width: 32,
            height: 32,
            obj_types: Some(vec![1352]),
            obj_counts: Some(vec![1]),
            properties: Properties {
                events: 1 << 28,
                target_param: -1,
            },
            ..Default::default()
        },
    );
    let rect = Rect {
        x: 0,
        y: 0,
        w: 32,
        h: 32,
    };
    let clip = Rect {
        x: 0,
        y: 0,
        w: 100,
        h: 100,
    };
    let nodes = [DrawNode {
        key,
        rect,
        clip,
        paint_clip: clip,
        subtree_end: 1,
    }];
    let mut controller = Controller::default();
    assert!(controller.begin(
        &state,
        &nodes,
        [16, 16],
        0,
        Some(MenuEntry {
            option: "Drop".into(),
            target: String::new(),
            action: 23,
            key: 0,
            args: [0, key.id],
            cursor: -1
        })
    ));
    controller.step(&mut state, &nodes, [16, 150], 6, true, false);
    let mut v = controller.inventory_visual.unwrap();
    let d = inventory_scroll::delta(v.point, v.press, v.cycles);
    let first_y = rect.y + d[1];
    let mut parent = Parent {
        scroll: 100,
        range: 400,
        height: 100,
    };
    inventory_scroll::edge_scroll(
        Some(&mut parent),
        &mut v.press[1],
        rect.y,
        d[1],
        [0, 100],
        1,
        |_, _| {},
    )
    .unwrap();
    assert_eq!(parent.scroll, 110);
    assert_eq!(v.press[1], 6);
    controller.inventory_presented(Some(v));
    controller.step(&mut state, &nodes, [16, 150], 7, true, false);
    let next = controller.inventory_visual.unwrap();
    let d = inventory_scroll::delta(next.point, next.press, next.cycles);
    assert_eq!(
        rect.y - 10 + d[1],
        first_y,
        "fresh layout and compensated press must cancel exactly"
    );
    assert_eq!(state.components[&key].obj_types, Some(vec![1352]));
    assert!(
        state.containers.is_empty(),
        "drag presentation must not invent authoritative item changes"
    );
}
