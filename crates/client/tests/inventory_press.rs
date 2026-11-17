use rs09_client::ui::inventory_press as press;
use serde_json::{Value, json};

#[test]
fn actual_inventory_release_overrides_feedback_after_the_menu_action() {
    use rs09_client::{
        interaction::MenuEntry,
        pack::Pack,
        ui::{
            Component, Properties,
            drag::Controller,
            state::{DrawNode, Key, Rect, State},
        },
    };
    let manifest = br#"{"source":"real controller pressed feedback","files":{}}"#;
    let mut bytes = b"RS09VIS1".to_vec();
    bytes.extend((manifest.len() as u32).to_le_bytes());
    bytes.extend(manifest);
    let mut state = State::new(Pack::decode(bytes).unwrap());
    let key = Key::component(1 << 16);
    state.loaded.insert(1);
    state.components.insert(
        key,
        Component {
            id: key.id,
            kind: 2,
            base_width: 2,
            base_height: 1,
            width: 64,
            height: 32,
            obj_types: Some(vec![1352, 996]),
            obj_counts: Some(vec![1, 1000]),
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
        w: 64,
        h: 32,
    };
    let nodes = [DrawNode {
        key,
        rect,
        clip: rect,
        paint_clip: rect,
        subtree_end: 1,
    }];
    let entry = MenuEntry {
        action: 47,
        key: 1351,
        args: [0, key.id],
        option: "Use".into(),
        target: "Bronze axe".into(),
        cursor: -1,
    };
    let mut controller = Controller::default();
    assert!(controller.begin(&state, &nodes, [4, 4], 100, Some(entry.clone())));
    let (_, click, packet) = controller.step(&mut state, &nodes, [4, 4], 101, false, false);
    let click = click.unwrap();
    assert!(packet.is_none());
    assert!(controller.take_inventory_release());
    assert!(!controller.take_inventory_release());
    state
        .capture_inventory_press(click.action, click.args[1], click.args[0])
        .unwrap();
    state.finish_inventory_action();
    assert_eq!(state.inventory_press_snapshot()["tick"], 0);
    state.released_inventory();
    for _ in 0..4 {
        state.advance_inventory_press();
    }
    assert_eq!(state.inventory_pressed_slot(key), Some(0));
    state.advance_inventory_press();
    assert!(!state.inventory_press_active());
    // An actual rearrangement also assigns10, even when its previous pressed
    // reference was captured by an earlier menu action and no new click runs.
    state.capture_inventory_press(47, key.id, 0).unwrap();
    for _ in 0..14 {
        state.advance_inventory_press();
    }
    assert!(controller.begin(&state, &nodes, [4, 4], 200, Some(entry)));
    controller.step(&mut state, &nodes, [40, 4], 205, true, false);
    let (_, click, packet) = controller.step(&mut state, &nodes, [40, 4], 206, false, false);
    assert!(click.is_none());
    assert!(packet.is_some());
    assert!(controller.take_inventory_release());
    state.released_inventory();
    assert_eq!(state.inventory_press_snapshot()["tick"], 10);
    assert_eq!(state.inventory_pressed_slot(key), Some(0));
}
#[test]
fn original_press_fields_timer_release_and_failure_prefixes_match() {
    use sha2::{Digest, Sha256};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let f: Value = serde_json::from_slice(
        &std::fs::read(root.join("fixtures/ui/inventory-press.json")).unwrap(),
    )
    .unwrap();
    for (name, hash) in f["sources"].as_object().unwrap() {
        let original = std::fs::read_to_string(
            root.join("reference/client/client/src/main/java/rt4")
                .join(name),
        )
        .unwrap()
        .replace("\r\n", "\n");
        assert_eq!(
            format!("{:x}", Sha256::digest(original.as_bytes())),
            hash.as_str().unwrap()
        );
    }
    assert_eq!(f["cases"].as_array().unwrap().len(), 2048);
    let families: Vec<i32> = serde_json::from_value(f["actionFamilies"].clone()).unwrap();
    for action in 0..=4005 {
        assert_eq!(press::handles_action(action), families.contains(&action));
    }
    let mut steps = 0;
    for (i, c) in f["cases"].as_array().unwrap().iter().enumerate() {
        let seed: Vec<i32> = serde_json::from_value(c["seed"].clone()).unwrap();
        let mut press = press::Press {
            component: (seed[0] >= 0).then_some(seed[0]),
            tick: seed[1],
            slot: seed[2],
        };
        let mut current = seed[3];
        let mut rows = vec![];
        for op in c["operations"].as_array().unwrap() {
            let a: Vec<i32> = serde_json::from_value(op.clone()).unwrap();
            let mut log = vec![];
            let result = match a[0] {
                0 => press
                    .assign(a[2], || {
                        log.push(json!([0, 100]));
                        if a[1] == -2 {
                            Err(())
                        } else {
                            current = a[1];
                            Ok((a[1] >= 0).then_some(a[1]))
                        }
                    })
                    .and_then(|()| {
                        press.after_action(|c| {
                            log.push(json!([1, c]));
                            if a[3] != 0 { Err(()) } else { Ok(()) }
                        })
                    }),
                1 => press.advance(|c| {
                    log.push(json!([1, c]));
                    if a[3] != 0 { Err(()) } else { Ok(()) }
                }),
                2 => {
                    press.released();
                    Ok(())
                }
                3 => {
                    current = a[1];
                    Ok(())
                }
                4 => press.after_action(|c| {
                    log.push(json!([1, c]));
                    if a[3] != 0 { Err(()) } else { Ok(()) }
                }),
                _ => panic!(),
            };
            rows.push(json!([
                log,
                press.component.unwrap_or(-1),
                press.slot,
                press.tick,
                current,
                press
                    .component
                    .is_some_and(|c| current >= 0 && c == current),
                result.err().map(|()| "IllegalStateException")
            ]));
            steps += 1;
        }
        assert_eq!(json!(rows), c["expected"], "case{i}");
    }
    assert_eq!(steps, 37888);
}
