use rs09_client::{
    interaction::Menu,
    pack::Pack,
    ui::{
        Component, HookArg, Properties,
        state::{DrawNode, Key, Rect, State},
        targeting::{ParamType, Targeting},
    },
};
use serde_json::Value;
fn corpus() -> Value {
    serde_json::from_str(include_str!("../../../fixtures/ui/targeting.json")).unwrap()
}
fn hex(data: impl IntoIterator<Item = u8>) -> String {
    data.into_iter().map(|n| format!("{n:02x}")).collect()
}
fn text_hex(text: &str) -> String {
    hex(text.chars().map(|c| c as u8))
}
fn bytes(text: &str) -> Vec<u8> {
    text.as_bytes()
        .chunks_exact(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
fn empty() -> State {
    let manifest = br#"{"source":"Explicit synthetic UI binding","files":{}}"#;
    let mut raw = b"RS09VIS1".to_vec();
    raw.extend((manifest.len() as u32).to_le_bytes());
    raw.extend(manifest);
    State::new(Pack::decode(raw).unwrap())
}
#[test]
fn all_seven_selected_component_payloads_match_original_writes() {
    for case in corpus()["wires"].as_array().unwrap() {
        let a: Vec<i32> = case["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_i64().unwrap() as i32)
            .collect();
        let target = Targeting {
            source: Key {
                id: a[4],
                child: a[5],
            },
            ..Targeting::default()
        };
        let args = if matches!(a[0], 11 | 38 | 39) {
            [a[2].wrapping_add(a[7]), a[3].wrapping_add(a[8])]
        } else {
            [a[2], a[3]]
        };
        let frame = target
            .packet(a[0], if a[0] == 38 { a[6] } else { a[1] }, args)
            .unwrap();
        assert_eq!(
            hex(std::iter::once(frame.opcode).chain(frame.payload)),
            case["expected"].as_str().unwrap(),
            "{case}"
        );
    }
}
#[test]
fn selected_widget_menus_match_unchanged_original() {
    for case in corpus()["menus"].as_array().unwrap() {
        let a: Vec<i32> = case["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_i64().unwrap() as i32)
            .collect();
        let mut state = empty();
        let key = Key {
            id: a[0],
            child: a[1],
        };
        state.components.insert(
            key,
            Component {
                id: a[0],
                created_component_id: a[1],
                if3: a[2] != 0,
                kind: 5,
                button_type: a[3],
                properties: Properties {
                    events: a[4],
                    target_param: -1,
                },
                option_circumfix: case["verb"].as_str().unwrap().into(),
                option_suffix: "Spell".into(),
                option_base: "Base".into(),
                ops: Some(
                    (0..10)
                        .map(|i| (a[5] >> i & 1 != 0).then(|| format!("Op{i}")))
                        .collect(),
                ),
                on_option_click: (a[6] != 0).then(|| vec![HookArg::Int(1)]),
                ..Component::default()
            },
        );
        state.targeting = Targeting {
            active: a[7] != 0,
            mask: a[8],
            param: -1,
            cursor: 37,
            verb: "Cast".into(),
            text: "Selected".into(),
            ..Targeting::default()
        };
        let rect = Rect {
            x: 0,
            y: 0,
            w: 32,
            h: 32,
        };
        let nodes = [DrawNode {
            key,
            rect,
            clip: rect,
            paint_clip: rect,
            subtree_end: 0,
        }];
        let mut menu = Menu::default();
        state.menu_at(&nodes, [0, 0], &mut menu);
        let actual = menu
            .entries
            .iter()
            .map(|e| {
                format!(
                    "{}:{}:{}:{}:{}:{}:{}",
                    e.action,
                    e.key,
                    e.args[0],
                    e.args[1],
                    e.cursor,
                    text_hex(&e.option),
                    text_hex(&e.target)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        assert_eq!(actual, case["expected"].as_str().unwrap(), "{case}");
    }
}
#[test]
fn parameter_decoding_matches_original_defaults_and_signed_values() {
    for case in corpus()["params"].as_array().unwrap() {
        let param = ParamType::decode(&bytes(case["raw"].as_str().unwrap())).unwrap();
        let actual = format!(
            "{}:{}:{}",
            param.default_int,
            i32::from(param.kind == 115),
            param
                .default_string
                .as_deref()
                .map(text_hex)
                .unwrap_or_else(|| "-".into())
        );
        assert_eq!(actual, case["expected"].as_str().unwrap(), "{case}");
    }
}
#[test]
fn typed_target_masks_and_default_comparisons_match_original() {
    use rs09_client::interaction::MenuEntry;
    use std::collections::BTreeMap;
    for case in corpus()["eligibility"].as_array().unwrap() {
        let a: Vec<i32> = case["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_i64().unwrap() as i32)
            .collect();
        let mut param = vec![2];
        param.extend(a[3].to_be_bytes());
        param.push(0);
        let manifest=serde_json::to_vec(&serde_json::json!({"source":"Explicit original parameter input","files":{"param/7":[0,param.len()]}})).unwrap();
        let mut raw = b"RS09VIS1".to_vec();
        raw.extend((manifest.len() as u32).to_le_bytes());
        raw.extend(manifest);
        raw.extend(param);
        let mut state = State::new(Pack::decode(raw).unwrap());
        state.targeting = Targeting {
            active: true,
            mask: a[1],
            param: a[2],
            cursor: 37,
            verb: "Cast".into(),
            text: "Selected".into(),
            ..Targeting::default()
        };
        let mut values = BTreeMap::new();
        if a[4] >= 2 {
            values.insert(
                if a[4] == 5 { 8 } else { 7 },
                if a[4] == 4 {
                    HookArg::Text("text".into())
                } else {
                    HookArg::Int(if a[4] == 3 {
                        a[3].wrapping_add(1)
                    } else {
                        a[3]
                    })
                },
            );
        }
        let (action, key, args, target) = match a[0] {
            1 => (39, 1, [5, 6], "<col=ff9040>Seed"),
            2 => (45, 13, [13, 0], "<col=ffff00>Npc"),
            4 => (38, 7u64 << 32, [5, 6], "<col=00ffff>Loc"),
            16 => (3, 1, [2, 149 << 16], "<col=ff9040>Seed"),
            _ => unreachable!(),
        };
        let mut menu = Menu::default();
        let result = state.target_entry(
            &mut menu,
            a[0],
            (a[4] != 0).then_some(&values),
            MenuEntry {
                option: String::new(),
                target: target.into(),
                action,
                key,
                args,
                cursor: -1,
            },
        );
        let actual = if result.is_err() {
            "error".into()
        } else {
            menu.entries
                .iter()
                .map(|e| {
                    format!(
                        "{}:{}:{}:{}:{}:{}:{}",
                        e.action,
                        e.key,
                        e.args[0],
                        e.args[1],
                        e.cursor,
                        text_hex(&e.option),
                        text_hex(&e.target)
                    )
                })
                .collect::<Vec<_>>()
                .join(",")
        };
        assert_eq!(actual, case["expected"].as_str().unwrap(), "{case}");
    }
}
fn callback(phase: i32, mode: i32) -> Vec<u8> {
    // Original CS2 format: mark callback order, then execute actual component
    // operations. The final unsupported opcode exercises ScriptRunner's error
    // reporting boundary after a completed prefix.
    let mut ops: Vec<(i32, i32, Option<&str>)> = vec![
        (42, 800, None),
        (0, 10, None),
        (4002, 0, None),
        (0, phase, None),
        (4000, 0, None),
        (43, 800, None),
    ];
    if matches!(mode, 1 | 2) {
        ops.extend([(0, 20, None), (0, 0, None), (200, 1, None), (38, 0, None)]);
    }
    if mode == 1 {
        ops.extend([
            (3, 0, Some("Changed")),
            (1306, 1, None),
            (3, 0, Some("ChangedBase")),
            (1305, 1, None),
        ]);
    }
    if mode == 2 {
        ops.push((101, 1, None));
    }
    if mode == 3 {
        ops.extend([
            (0, 20, None),
            (0, 5, None),
            (0, 0, None),
            (100, 1, None),
            (3, 0, Some("Replacement")),
            (1306, 1, None),
            (3, 0, Some("ReplacementBase")),
            (1305, 1, None),
        ]);
    }
    if mode == 4 {
        ops.push((9999, 0, None));
    }
    ops.push((21, 0, None));
    let mut data = vec![0];
    for &(op, arg, text) in &ops {
        data.extend((op as u16).to_be_bytes());
        if op == 3 {
            data.extend(text.unwrap().bytes());
            data.push(0);
        } else if op >= 100 || matches!(op, 21 | 38 | 39) {
            data.push(arg as u8);
        } else {
            data.extend(arg.to_be_bytes());
        }
    }
    data.extend((ops.len() as i32).to_be_bytes());
    data.extend([0; 8]);
    data.push(0);
    data.extend(1u16.to_be_bytes());
    data
}
fn lifecycle_state(cancel: i32, enter: i32) -> State {
    let mut blob = Vec::new();
    let mut files = serde_json::Map::new();
    for (id, phase, mode) in [(901, 1, cancel), (902, 2, enter)] {
        let data = callback(phase, mode);
        files.insert(
            format!("script/{id}"),
            serde_json::json!([blob.len(), data.len()]),
        );
        blob.extend(data);
    }
    let manifest = serde_json::to_vec(
        &serde_json::json!({"source":"Controlled CS2 lifecycle callbacks","files":files}),
    )
    .unwrap();
    let mut raw = b"RS09VIS1".to_vec();
    raw.extend((manifest.len() as u32).to_le_bytes());
    raw.extend(manifest);
    raw.extend(blob);
    let mut state = State::new(Pack::decode(raw).unwrap());
    state.loaded.insert(0);
    state.components.insert(
        Key::component(20),
        Component {
            id: 20,
            ..Component::default()
        },
    );
    state
}
#[test]
fn real_cs2_hooks_preserve_original_target_lifecycle_and_component_references() {
    use rs09_client::ui::{actions::SelectedItem, vm::Vm};
    for case in corpus()["lifecycle"].as_array().unwrap() {
        let a: Vec<i32> = case["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_i64().unwrap() as i32)
            .collect();
        let mut state = lifecycle_state(a[0], a[1]);
        let mut vm = Vm::default();
        state.components.insert(
            Key::component(10),
            Component {
                id: 10,
                on_use_with: Some(vec![HookArg::Int(901)]),
                ..Component::default()
            },
        );
        let key = Key { id: 20, child: 0 };
        state.components.insert(
            key,
            Component {
                id: 20,
                created_component_id: 0,
                if3: a[3] != 0,
                kind: 5,
                option_base: "Base".into(),
                option_suffix: "Spell".into(),
                option_circumfix: "Cast".into(),
                properties: Properties {
                    events: a[5] << 11,
                    target_param: 7,
                },
                target_cursor: 44,
                default_target_cursor: 45,
                on_use: Some(vec![HookArg::Int(902)]),
                ..Component::default()
            },
        );
        state.targeting = Targeting {
            active: a[2] != 0,
            source: Key::component(10),
            mask: 86,
            param: -1,
            cursor: 31,
            default_cursor: 32,
            verb: "Old".into(),
            text: "OldText".into(),
        };
        state.selected_item = Some(SelectedItem {
            id: 1,
            slot: 0,
            component: 30,
            text: "Seed".into(),
        });
        vm.select_target(&mut state, key).unwrap();
        if a[4] != 0 {
            vm.cancel_targeting(&mut state);
        }
        let t = &state.targeting;
        let current = state
            .components
            .get(&key)
            .map(|c| {
                format!(
                    "{}:{}",
                    text_hex(&c.option_circumfix),
                    text_hex(&c.option_base)
                )
            })
            .unwrap_or_else(|| "-".into());
        let actual = format!(
            "{},{},{},{},{},{},{},{},{},{};{};{};{}",
            i32::from(t.active),
            t.source.id,
            t.source.child,
            t.mask,
            t.param,
            t.cursor,
            t.default_cursor,
            i32::from(state.selected_item.is_some()),
            vm.varcs.get(&800).copied().unwrap_or(0),
            i32::from(!state.diagnostics.is_empty()),
            text_hex(&t.verb),
            text_hex(&t.text),
            current
        );
        // The workbench's existing diagnostic ring deduplicates equal messages;
        // compare error presence, while the CS2 trace checks both callbacks.
        let mut expected: Vec<String> = case["expected"]
            .as_str()
            .unwrap()
            .split(';')
            .map(str::to_owned)
            .collect();
        let mut fields: Vec<String> = expected[0].split(',').map(str::to_owned).collect();
        fields[9] = if fields[9] == "0" { "0" } else { "1" }.into();
        expected[0] = fields.join(",");
        assert_eq!(actual, expected.join(";"), "{case}");
    }
}
