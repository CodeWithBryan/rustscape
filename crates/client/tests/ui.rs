use rs09_client::ui::Component;
use serde_json::Value;
use sha2::{Digest, Sha256};
fn canonical(out: &mut Vec<u8>, value: &Value) {
    fn text(out: &mut Vec<u8>, s: &str) {
        out.extend((s.len() as i32).to_be_bytes());
        out.extend(s.as_bytes());
    }
    match value {
        Value::Null => out.push(0),
        Value::Bool(b) => out.push(if *b { 2 } else { 1 }),
        Value::Number(n) => {
            out.push(3);
            out.extend(n.as_i64().unwrap().to_be_bytes());
        }
        Value::String(s) => {
            out.push(4);
            text(out, s);
        }
        Value::Array(a) => {
            out.push(5);
            out.extend((a.len() as i32).to_be_bytes());
            for v in a {
                canonical(out, v);
            }
        }
        Value::Object(m) => {
            out.push(6);
            out.extend((m.len() as i32).to_be_bytes());
            for (k, v) in m {
                text(out, k);
                canonical(out, v);
            }
        }
    }
}
#[test]
fn every_cached_widget_and_layout_matches_original_rt4() {
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ui");
    let oracle: Value =
        serde_json::from_slice(&std::fs::read(fixtures.join("oracle.json")).unwrap()).unwrap();
    let raw = std::fs::read(fixtures.join("components.bin")).unwrap();
    for case in oracle["components"].as_array().unwrap() {
        let id = case["id"].as_i64().unwrap() as i32;
        let start = case["offset"].as_u64().unwrap() as usize;
        let size = case["size"].as_u64().unwrap() as usize;
        let (mut c, position) = Component::decode(id, &raw[start..start + size])
            .unwrap_or_else(|e| panic!("component {id}: {e}"));
        assert_eq!(
            position,
            case["position"].as_u64().unwrap() as usize,
            "component {id} decode position"
        );
        let mut bytes = Vec::new();
        canonical(&mut bytes, &serde_json::to_value(&c).unwrap());
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            case["sha256"].as_str().unwrap(),
            "component {id} fields"
        );
        c.calculate_size(765, 503).unwrap();
        c.calculate_position(765, 503);
        assert_eq!(
            serde_json::json!([c.x, c.y, c.width, c.height]),
            case["layout"],
            "component {id} layout"
        );
    }
    assert_eq!(oracle["components"].as_array().unwrap().len(), 31622);
    for case in oracle["scripts"].as_array().unwrap() {
        let start = case["offset"].as_u64().unwrap() as usize;
        let size = case["size"].as_u64().unwrap() as usize;
        let script = rs09_client::ui::script::Script::decode(&raw[start..start + size]).unwrap();
        let mut bytes = Vec::new();
        canonical(&mut bytes, &serde_json::to_value(script).unwrap());
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            case["sha256"].as_str().unwrap(),
            "script {}",
            case["id"]
        );
    }
    assert_eq!(oracle["scripts"].as_array().unwrap().len(), 2063);
    for case in oracle["items"].as_array().unwrap() {
        let start = case["offset"].as_u64().unwrap() as usize;
        let size = case["size"].as_u64().unwrap() as usize;
        let id = case["id"].as_i64().unwrap() as i32;
        let (item, pos) =
            rs09_client::ui::item::ItemType::decode(id, &raw[start..start + size]).unwrap();
        assert_eq!(pos, case["position"].as_u64().unwrap() as usize);
        let mut bytes = Vec::new();
        canonical(&mut bytes, &serde_json::to_value(item).unwrap());
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes)),
            case["sha256"].as_str().unwrap(),
            "item {id}"
        );
    }
    assert_eq!(oracle["items"].as_array().unwrap().len(), 14654);
    for case in oracle["layouts"].as_array().unwrap() {
        let x: Vec<i32> = case["input"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap() as i32)
            .collect();
        let mut c = Component {
            base_x: x[0],
            base_y: x[1],
            base_width: x[2],
            base_height: x[3],
            x_mode: x[4],
            y_mode: x[5],
            dynamic_width_value: x[6],
            dynamic_height_value: x[7],
            kind: x[8],
            inv_margin_x: x[9],
            inv_margin_y: x[10],
            aspect_width: x[11],
            aspect_height: x[12],
            ..Component::default()
        };
        c.calculate_size(x[13], x[14]).unwrap();
        c.calculate_position(x[13], x[14]);
        assert_eq!(
            serde_json::json!([c.x, c.y, c.width, c.height]),
            case["expected"],
            "layout probe {x:?}"
        );
    }
}

#[test]
fn inventory_triangle_pixels_match_original_scan_converter() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ui/oracle.json");
    let oracle: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let palette: Vec<u32> = (0..65536u32)
        .map(|i| (i.wrapping_mul(131586) & 0xffffff) | 1)
        .collect();
    let mut failures = Vec::new();
    for (index, case) in oracle["raster"].as_array().unwrap().iter().enumerate() {
        let points = std::array::from_fn(|i| {
            std::array::from_fn(|j| case["points"][i][j].as_i64().unwrap() as i32)
        });
        let colors = std::array::from_fn(|i| case["colors"][i].as_i64().unwrap() as i32);
        let mut pixels = vec![0x42657b; 36 * 32];
        rs09_client::ui::raster::gouraud(
            &mut pixels,
            [36, 32],
            &palette,
            points,
            colors,
            case["alpha"].as_i64().unwrap() as i32,
        )
        .unwrap();
        let bytes: Vec<u8> = pixels.iter().flat_map(|p| p.to_be_bytes()).collect();
        if format!("{:x}", Sha256::digest(bytes)) != case["sha256"].as_str().unwrap() {
            failures.push(index);
        }
    }
    assert!(
        failures.is_empty(),
        "{} raster mismatches: {:?}",
        failures.len(),
        &failures[..failures.len().min(20)]
    );
}

#[test]
fn textured_inventory_pixels_match_original_perspective_blocks() {
    use rs09_client::ui::raster::{self, Texture, TexturedTriangle};
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ui/oracle.json");
    let oracle: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let palette: Vec<u32> = (0..65536u32)
        .map(|i| (i.wrapping_mul(131586) & 0xffffff) | 1)
        .collect();
    let texels: Vec<u32> = (0..16384u32)
        .map(|i| {
            if i % 7 == 0 {
                0
            } else {
                (i.wrapping_mul(131586) & 0xffffff) | 1
            }
        })
        .collect();
    let mut failures = Vec::new();
    for (index, case) in oracle["textured"].as_array().unwrap().iter().enumerate() {
        let low = case["low_detail"].as_bool().unwrap();
        let texture = Texture {
            texels: &texels[..if low { 4096 } else { 16384 }],
            low_detail: low,
            opaque: case["opaque"].as_bool().unwrap(),
            average: 4500,
        };
        let face = TexturedTriangle {
            points: std::array::from_fn(|i| {
                std::array::from_fn(|j| case["points"][i][j].as_i64().unwrap() as i32)
            }),
            lights: std::array::from_fn(|i| case["lights"][i].as_i64().unwrap() as i32),
            view: std::array::from_fn(|i| {
                std::array::from_fn(|j| case["view"][i][j].as_i64().unwrap() as i32)
            }),
            alpha: case["alpha"].as_i64().unwrap() as i32,
        };
        let mut pixels = vec![0x42657b; 36 * 32];
        raster::textured(&mut pixels, [36, 32], &palette, &texture, &face).unwrap();
        let bytes: Vec<u8> = pixels.iter().flat_map(|p| p.to_be_bytes()).collect();
        if format!("{:x}", Sha256::digest(bytes)) != case["sha256"].as_str().unwrap() {
            failures.push(index);
        }
    }
    assert!(
        failures.is_empty(),
        "{} textured raster mismatches: {:?}",
        failures.len(),
        &failures[..failures.len().min(20)]
    );
}

#[test]
fn item_and_widget_actions_match_original_bytes_and_access_masks() {
    use rs09_client::{
        pack::Pack,
        ui::{
            actions,
            state::{Key, State},
        },
    };
    use rs09_protocol::game::Direction;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ui/oracle.json");
    let oracle: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let manifest = br#"{"source":"empty controlled UI binding","files":{}}"#;
    let mut pack = b"RS09VIS1".to_vec();
    pack.extend((manifest.len() as u32).to_le_bytes());
    pack.extend(manifest);
    let mut state = State::new(Pack::decode(pack).unwrap());
    for case in oracle["actions"].as_array().unwrap() {
        let action = case["action"].as_i64().unwrap() as i32;
        let slot = case["slot"].as_i64().unwrap() as i32;
        let component = case["component"].as_i64().unwrap() as i32;
        let frame = if case["kind"] == "item" {
            actions::item_packet(action, case["id"].as_i64().unwrap() as i32, slot, component)
                .unwrap()
        } else {
            let key = Key {
                id: component,
                child: slot,
            };
            state.loaded.insert(component >> 16);
            state.components.insert(
                key,
                Component {
                    id: component,
                    created_component_id: slot,
                    ..Component::default()
                },
            );
            assert!(
                state.button_packet(key, action).unwrap().is_none(),
                "Disabled button must not transmit"
            );
            state.properties.insert(
                key,
                rs09_client::ui::Properties {
                    events: 1 << action,
                    target_param: -1,
                },
            );
            let frame = state.button_packet(key, action).unwrap().unwrap();
            state.properties.remove(&key);
            frame
        };
        let mut bytes = Vec::new();
        frame
            .write(Direction::ClientToServer, None, &mut bytes)
            .unwrap();
        let hex = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        assert_eq!(hex, case["hex"].as_str().unwrap(), "action {case}");
    }
    assert_eq!(oracle["actions"].as_array().unwrap().len(), 148);
}

#[test]
fn native_script_component_operations_match_unchanged_runner_branches() {
    use rs09_client::{
        pack::Pack,
        ui::{
            HookArg,
            state::{Hook, Key, State},
            vm::Vm,
        },
    };
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/ui");
    let oracle: Value =
        serde_json::from_slice(&std::fs::read(fixtures.join("oracle.json")).unwrap()).unwrap();
    let raw = std::fs::read(fixtures.join("components.bin")).unwrap();
    let components = oracle["components"].as_array().unwrap();
    let mut failures = Vec::new();
    for case in oracle["operations"].as_array().unwrap() {
        let id = case["component"].as_i64().unwrap() as i32;
        let op = case["opcode"].as_i64().unwrap() as i32;
        let source = components.iter().find(|c| c["id"] == id).unwrap();
        let offset = source["offset"].as_u64().unwrap() as usize;
        let size = source["size"].as_u64().unwrap() as usize;
        let (component, _) = Component::decode(id, &raw[offset..offset + size]).unwrap();
        let mut script = vec![0u8];
        let mut instructions = 0i32;
        let mut push = |opcode: u16, operand: &[u8]| {
            script.extend(opcode.to_be_bytes());
            script.extend(operand);
            instructions += 1;
        };
        for n in case["ints"].as_array().unwrap() {
            push(0, &(n.as_i64().unwrap() as i32).to_be_bytes());
        }
        for text in case["strings"].as_array().unwrap() {
            let mut bytes = text
                .as_str()
                .unwrap()
                .chars()
                .map(|c| c as u8)
                .collect::<Vec<_>>();
            bytes.push(0);
            push(3, &bytes);
        }
        push(0, &id.to_be_bytes());
        if op == 1702 {
            // 2702 is hasChild, not an explicit-component getChildId.
            push(201, &[0]);
            push(1702, &[0]);
        } else {
            push((op + 1000) as u16, &[0]);
        }
        let result = case["result"].as_str();
        if result == Some("int") {
            push(43, &999i32.to_be_bytes());
        }
        if result == Some("string") {
            push(48, &999i32.to_be_bytes());
        }
        push(21, &[0]);
        script.extend(instructions.to_be_bytes());
        script.extend([0; 9]);
        script.extend(1u16.to_be_bytes());
        let mut blob = script.clone();
        let mut files = serde_json::Map::new();
        files.insert("script/1".into(), serde_json::json!([0, script.len()]));
        for item in oracle["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| [995, 1277, 4151].contains(&(c["id"].as_i64().unwrap() as i32)))
        {
            let offset = item["offset"].as_u64().unwrap() as usize;
            let size = item["size"].as_u64().unwrap() as usize;
            files.insert(
                format!("item/{}", item["id"]),
                serde_json::json!([blob.len(), size]),
            );
            blob.extend(&raw[offset..offset + size]);
        }
        let manifest = serde_json::to_vec(
            &serde_json::json!({"source":"Controlled childless UI oracle bindings","files":files}),
        )
        .unwrap();
        let mut pack = b"RS09VIS1".to_vec();
        pack.extend((manifest.len() as u32).to_le_bytes());
        pack.extend(manifest);
        pack.extend(blob);
        let mut state = State::new(Pack::decode(pack).unwrap());
        let key = Key::component(id);
        state.loaded.insert(id >> 16);
        state.components.insert(key, component);
        let mut vm = Vm::default();
        vm.run(&mut state, &Hook::new(key, vec![HookArg::Int(1)]), 1000)
            .unwrap_or_else(|e| panic!("{case}: {e}"));
        let mut bytes = Vec::new();
        canonical(
            &mut bytes,
            &serde_json::to_value(&state.components[&key]).unwrap(),
        );
        let ints = if result == Some("int") {
            vec![vm.varcs[&999]]
        } else {
            Vec::new()
        };
        let strings = if result == Some("string") {
            vec![vm.varcstrs[&999].clone()]
        } else {
            Vec::new()
        };
        canonical(&mut bytes, &serde_json::json!(ints));
        canonical(&mut bytes, &serde_json::json!(strings));
        let drag = state.drag_parents.get(&key);
        bytes.extend(drag.map_or(-1, |k| k.id).to_be_bytes());
        bytes.extend(drag.map_or(-1, |k| k.child).to_be_bytes());
        if format!("{:x}", Sha256::digest(bytes)) != case["sha256"].as_str().unwrap() {
            failures.push(case.clone());
        }
    }
    assert!(
        failures.is_empty(),
        "{} component operation mismatches: {:?}",
        failures.len(),
        &failures[..failures.len().min(12)]
    );
}
#[test]
fn delayed_state_and_varp_timers_match_original_intrusive_queues() {
    use rs09_client::ui::{delayed::Queue, varps::Pending};
    use std::{collections::BTreeMap, fmt::Write};
    let corpus: Value =
        serde_json::from_str(include_str!("../../../fixtures/ui/delayed.json")).unwrap();
    for (index, case) in corpus["cases"].as_array().unwrap().iter().enumerate() {
        let mut delayed = Queue::default();
        let mut pending = Pending::default();
        let mut active = BTreeMap::new();
        let mut now = 1000;
        let mut observed = String::new();
        for c in case["commands"].as_array().unwrap() {
            let op = c[0].as_str().unwrap();
            let a = c.get(1).and_then(Value::as_i64).unwrap_or(0) as i32;
            let b = c.get(2).and_then(Value::as_i64).unwrap_or(0) as i32;
            match op {
                "T" => now += i64::from(a),
                "C" => delayed.client(a, b, now),
                "S" => delayed.server(a, b, c[3].as_i64().unwrap() as i32, None),
                "V" => pending.server(a, b, now),
                "L" => {
                    active.insert(a, b);
                    pending.client(a, now);
                }
                "P" => {
                    observed.push('[');
                    while let Some(change) = delayed.poll(now) {
                        write!(observed, "{}:{}:{},", change.kind, change.id, change.value)
                            .unwrap();
                    }
                    observed.push('|');
                    for id in pending.poll(&mut active, now) {
                        write!(observed, "{id}:{},", active[&id]).unwrap();
                    }
                    observed.push(']');
                }
                _ => unreachable!(),
            }
        }
        for id in 0..100 {
            write!(
                observed,
                "{}:{},",
                active.get(&id).unwrap_or(&0),
                pending.values.get(&id).unwrap_or(&0)
            )
            .unwrap();
        }
        assert_eq!(
            observed,
            case["expected"].as_str().unwrap(),
            "scenario {index}"
        );
    }
}
#[test]
fn inventory_rearrangement_and_component_drag_wires_match_original() {
    use rs09_client::{
        pack::Pack,
        ui::{
            drag,
            state::{Key, State},
        },
    };
    let corpus: Value =
        serde_json::from_str(include_str!("../../../fixtures/ui/input.json")).unwrap();
    let manifest = br#"{"source":"Controlled empty UI binding","files":{}}"#;
    let mut raw = b"RS09VIS1".to_vec();
    raw.extend((manifest.len() as u32).to_le_bytes());
    raw.extend(manifest);
    for case in corpus["cases"].as_array().unwrap() {
        let a = case["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap() as i32)
            .collect::<Vec<_>>();
        let mut state = State::new(Pack::decode(raw.clone()).unwrap());
        let key = Key::component(a[0]);
        state.loaded.insert((a[0] as u32 >> 16) as i32);
        state.components.insert(
            key,
            Component {
                id: a[0],
                client_code: a[1],
                properties: rs09_client::ui::Properties {
                    events: a[2],
                    target_param: -1,
                },
                obj_types: Some(serde_json::from_value(case["ids"].clone()).unwrap()),
                obj_counts: Some(serde_json::from_value(case["counts"].clone()).unwrap()),
                ..Default::default()
            },
        );
        let frame = drag::rearrange(&mut state, key, a[3], a[4], a[5] == 1).unwrap();
        assert_eq!(
            serde_json::json!(state.components[&key].obj_types),
            case["expected_ids"]
        );
        assert_eq!(
            serde_json::json!(state.components[&key].obj_counts),
            case["expected_counts"]
        );
        let hex = |frame: rs09_protocol::game::Frame| {
            std::iter::once(&frame.opcode)
                .chain(frame.payload.iter())
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        };
        assert_eq!(hex(frame), case["reorder"].as_str().unwrap());
        let source = Key {
            id: a[0],
            child: a[6],
        };
        let parent = Key::component(!a[0]);
        state.components.insert(
            parent,
            Component {
                id: parent.id,
                ..Component::default()
            },
        );
        state.components.insert(
            source,
            Component {
                id: source.id,
                created_component_id: source.child,
                overlayer: parent.id,
                properties: rs09_client::ui::Properties {
                    events: if a[9] == 0 { 1 << 18 } else { 0 },
                    target_param: -1,
                },
                ..Component::default()
            },
        );
        let release = drag::Release::Component {
            source,
            target: Key {
                id: a[7],
                child: a[8],
            },
        };
        // Model callback mutation before the source's final getDragRenderParent test.
        state.components.get_mut(&source).unwrap().properties.events =
            if a[9] != 0 { 1 << 18 } else { 0 };
        let release = release.resolve(&state).map_or_else(String::new, hex);
        assert_eq!(release, case["release"].as_str().unwrap());
        assert!(state.containers.is_empty());
    }
}
#[test]
fn text_collation_and_paragraph_breaks_match_original() {
    use rs09_client::ui::{font::Metrics, text};
    let corpus: Value =
        serde_json::from_str(include_str!("../../../fixtures/ui/text.json")).unwrap();
    let bytes = |hex: &str| {
        hex.as_bytes()
            .chunks_exact(2)
            .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
            .collect::<Vec<_>>()
    };
    for c in corpus["compare"].as_array().unwrap() {
        let a = rs09_client::ui::latin1(&bytes(c["a"].as_str().unwrap()));
        let b = rs09_client::ui::latin1(&bytes(c["b"].as_str().unwrap()));
        assert_eq!(
            text::compare(&a, &b),
            c["expected"].as_i64().unwrap() as i32,
            "{c}"
        );
    }
    for c in corpus["wrap"].as_array().unwrap() {
        let mut metrics = Metrics::decode(&bytes(c["metrics"].as_str().unwrap())).unwrap();
        metrics.name_icons = serde_json::from_value(c["icons"].clone()).unwrap();
        let text = c["text"].as_str().unwrap();
        assert_eq!(
            metrics.width(text),
            c["expected_width"].as_i64().unwrap() as i32,
            "{c}"
        );
        assert_eq!(
            serde_json::json!(metrics.split(text, Some(&[c["width"].as_i64().unwrap() as i32]))),
            c["lines"],
            "{c}"
        );
        let layout = c["layout"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap() as i32)
            .collect::<Vec<_>>();
        let paragraphs = metrics.paragraph(
            text,
            layout[..4].try_into().unwrap(),
            layout[4..6].try_into().unwrap(),
            layout[6],
            layout[7..].try_into().unwrap(),
        );
        let observed = paragraphs
            .into_iter()
            .map(|line| {
                format!(
                    "{}:{}:{}:{}",
                    line.position[0],
                    line.position[1],
                    line.spacing,
                    line.text
                        .chars()
                        .map(|c| format!("{:02x}", c as u8))
                        .collect::<String>()
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(serde_json::json!(observed), c["positions"], "{c}");
    }
}
#[test]
fn npc_metadata_menus_priorities_and_use_packets_match_original() {
    use rs09_client::{
        interaction::Menu, npc::Definition, npc_interaction, ui::actions::SelectedItem,
    };
    let corpus: Value =
        serde_json::from_str(include_str!("../../../fixtures/ui/npc.json")).unwrap();
    let bytes = |hex: &str| {
        hex.as_bytes()
            .chunks_exact(2)
            .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
            .collect::<Vec<_>>()
    };
    let hex = |s: &str| {
        s.chars()
            .map(|c| format!("{:02x}", c as u8))
            .collect::<String>()
    };
    let wire = |frame: rs09_protocol::game::Frame| {
        std::iter::once(frame.opcode)
            .chain(frame.payload)
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    for c in corpus["cases"].as_array().unwrap() {
        let mut d = Definition::decode(&bytes(c["raw"].as_str().unwrap())).unwrap();
        let mut meta = format!(
            "{}:{}:{}:{}:{}:{}:{}",
            d.combat_level,
            i32::from(d.interactive),
            d.cursors[0],
            d.cursors[1],
            d.cursors[2],
            d.cursors[3],
            d.attack_cursor
        );
        for op in &d.options {
            meta.push(':');
            meta.push_str(&op.as_ref().map_or_else(|| "-".into(), |s| hex(s)));
        }
        assert_eq!(meta, c["metadata"].as_str().unwrap());
        let a = c["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap() as i32)
            .collect::<Vec<_>>();
        if a[1] != -2 {
            d.combat_level = a[1];
        }
        if a[2] != -1 {
            d.interactive = a[2] == 1;
        }
        let selected = SelectedItem {
            id: a[6],
            slot: a[7],
            component: a[5],
            text: "<col=ff9040>Seed<col=ffffff>".into(),
        };
        let mut menu = Menu::default();
        npc_interaction::entries(&mut menu, &d, a[4], a[0], (a[3] == 1).then_some(&selected));
        let observed = menu
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
                    hex(&e.option),
                    hex(&e.target)
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(serde_json::json!(observed), c["entries"], "{c}");
        let mut packets = [17, 16, 4, 19, 2, 1007]
            .map(|action| {
                wire(
                    npc_interaction::packet(action, a[4], c["id"].as_i64().unwrap() as i32)
                        .unwrap(),
                )
            })
            .to_vec();
        packets.extend([
            wire(selected.on_item(a[4], a[7], a[5])),
            wire(selected.on_npc(a[4])),
            wire(selected.on_location(a[4], [a[7], a[5]])),
        ]);
        assert_eq!(serde_json::json!(packets), c["wires"]);
    }
}
#[test]
fn chat_dialog_frames_and_character_boundaries_match_original() {
    use rs09_client::ui::{chat, text};
    use rs09_protocol::{game::Direction, huffman::Huffman};
    let corpus: Value =
        serde_json::from_str(include_str!("../../../fixtures/ui/chat.json")).unwrap();
    let bytes = |hex: &str| {
        hex.as_bytes()
            .chunks_exact(2)
            .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
            .collect::<Vec<_>>()
    };
    let codec = Huffman::new(&bytes(corpus["huffman"].as_str().unwrap())).unwrap();
    for c in corpus["cases"].as_array().unwrap() {
        let op = c["op"].as_i64().unwrap() as i32;
        let text = c["text"].as_str().unwrap();
        let frame = if op == 5008 {
            chat::public(
                text,
                &codec,
                c["rights"].as_i64().unwrap() as i32,
                c["restricted"] == 1,
                c["quick"] == 1,
            )
            .unwrap()
        } else {
            Some(chat::dialog(op, text).unwrap())
        };
        let mut raw = Vec::new();
        if let Some(frame) = frame {
            frame
                .write(Direction::ClientToServer, None, &mut raw)
                .unwrap();
        }
        assert_eq!(raw, bytes(c["wire"].as_str().unwrap()), "{c}");
    }
    for c in corpus["characters"].as_array().unwrap() {
        let n = c["c"].as_i64().unwrap() as i32;
        let values = [4113, 4114, 4115, 4116, 4122, 4123].map(|op| text::character(op, n));
        assert_eq!(serde_json::json!(values), c["expected"], "{c}");
    }
}
#[test]
fn queued_sound_trimming_and_finite_loop_pcm_match_original_mixer() {
    use rs09_client::audio::SoundEffect;
    let corpus: Value =
        serde_json::from_str(include_str!("../../../fixtures/ui/audio.json")).unwrap();
    for c in corpus["cases"].as_array().unwrap() {
        let raw = c["raw"]
            .as_str()
            .unwrap()
            .as_bytes()
            .chunks_exact(2)
            .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
            .collect::<Vec<_>>();
        let (effect, ticks) = SoundEffect::decode_for_queue(&raw).unwrap();
        assert_eq!(ticks, c["ticks"].as_i64().unwrap() as i32, "{c}");
        assert_eq!(effect.loop_start, c["start"].as_u64().unwrap() as usize);
        assert_eq!(effect.loop_end, c["end"].as_u64().unwrap() as usize);
        let digest = |samples: &[i8]| {
            format!(
                "{:x}",
                Sha256::digest(samples.iter().map(|&s| s as u8).collect::<Vec<_>>())
            )
        };
        assert_eq!(
            digest(&effect.samples),
            c["sha256"].as_str().unwrap(),
            "{}",
            c["name"]
        );
        for (loops, expected) in [1, 2, 3, 8].into_iter().zip(c["loops"].as_array().unwrap()) {
            let repeated = effect.repeated(loops);
            if expected == "error" {
                assert!(repeated.is_err(), "{} loops{loops}", c["name"]);
            } else {
                let mut repeated = repeated.unwrap().samples;
                repeated.extend([0; 32]);
                assert_eq!(
                    digest(&repeated),
                    expected.as_str().unwrap(),
                    "{} loops{loops}",
                    c["name"]
                );
            }
        }
    }
}
#[test]
fn all_sprite_sheets_metadata_alpha_and_rgba_match_original() {
    use rs09_client::ui::sprite::Sheet;
    let corpus: Value =
        serde_json::from_str(include_str!("../../../fixtures/ui/sprites.json")).unwrap();
    let blob = include_bytes!("../../../fixtures/ui/sprites.bin");
    for c in corpus["cases"].as_array().unwrap() {
        let offset = c["offset"].as_u64().unwrap() as usize;
        let size = c["size"].as_u64().unwrap() as usize;
        let sheet = Sheet::decode(&blob[offset..offset + size]).unwrap();
        let mut bytes = Vec::new();
        for n in [
            sheet.size[0],
            sheet.size[1],
            sheet.frames.len(),
            sheet.palette.len(),
        ] {
            bytes.extend((n as i32).to_be_bytes());
        }
        for n in &sheet.palette {
            bytes.extend(n.to_be_bytes());
        }
        for f in &sheet.frames {
            for n in [f.offset[0], f.offset[1], f.size[0], f.size[1]] {
                bytes.extend((n as i32).to_be_bytes());
            }
            bytes.push(u8::from(f.has_alpha));
            bytes.extend((f.indices.len() as i32).to_be_bytes());
            bytes.extend(&f.indices);
            bytes.extend(&f.alpha);
        }
        for i in 0..sheet.frames.len() {
            bytes.extend(sheet.rgba(i).unwrap());
        }
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            c["sha256"].as_str().unwrap(),
            "{}",
            c["name"]
        );
    }
}
#[test]
fn immutable_text_editing_matches_original_byte_strings() {
    use rs09_client::ui::text;
    let corpus: Value =
        serde_json::from_str(include_str!("../../../fixtures/ui/editing.json")).unwrap();
    let decode = |s: &str| {
        s.as_bytes()
            .chunks_exact(2)
            .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
            .map(char::from)
            .collect::<String>()
    };
    for c in corpus["cases"].as_array().unwrap() {
        let input = decode(c["text"].as_str().unwrap());
        let arg = |i: usize| c["args"][i].as_i64().unwrap() as i32;
        let result = if c["op"] == "C" {
            text::concat_character(&input, arg(0))
        } else {
            text::substring(&input, arg(0), arg(1))
        };
        if c["expected"] == "error" {
            assert!(result.is_err(), "{c}");
        } else {
            assert_eq!(
                result.unwrap(),
                decode(c["expected"].as_str().unwrap()),
                "{c}"
            );
        }
    }
}
#[test]
fn ordinary_game_messages_from_the_server_enter_the_chatbox_history() {
    use rs09_client::{
        pack::Pack,
        ui::{state::State, vm::Vm},
    };
    use rs09_protocol::game::Frame;
    let manifest = br#"{"source":"Controlled empty UI binding","files":{}}"#;
    let mut raw = b"RS09VIS1".to_vec();
    raw.extend((manifest.len() as u32).to_le_bytes());
    raw.extend(manifest);
    let mut state = State::new(Pack::decode(raw).unwrap());
    let mut vm = Vm::default();
    state.transmit_timer = 67;
    for line in [
        "Bowl: This item currently costs 4 coins.",
        "Useful for mixing things.",
    ] {
        let mut payload = line.as_bytes().to_vec();
        payload.push(0);
        assert!(
            vm.apply_frame(
                &mut state,
                &Frame {
                    opcode: 70,
                    payload
                }
            )
            .unwrap()
        );
    }
    assert_eq!(vm.messages[0].text, "Useful for mixing things.");
    assert_eq!(
        vm.messages[1].text,
        "Bowl: This item currently costs 4 coins."
    );
    assert_eq!((vm.messages[0].kind, vm.messages[0].phrase), (0, -1));
    assert_eq!(state.chat_transmit_at, 67);
}

#[test]
fn chat_history_and_message_notifications_match_original() {
    use rs09_client::{
        pack::Pack,
        ui::{
            HookArg,
            state::{DrawNode, Key, Rect, State},
            vm::{ChatLine, Vm},
        },
    };
    let corpus: Value =
        serde_json::from_str(include_str!("../../../fixtures/ui/chat-state.json")).unwrap();
    let manifest = br#"{"source":"Controlled empty UI binding","files":{}}"#;
    let mut raw = b"RS09VIS1".to_vec();
    raw.extend((manifest.len() as u32).to_le_bytes());
    raw.extend(manifest);
    let decode = |s: &str| {
        s.as_bytes()
            .chunks_exact(2)
            .map(|p| char::from(u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap()))
            .collect::<String>()
    };
    let hex = |s: &str| {
        s.chars()
            .map(|c| format!("{:02x}", c as u8))
            .collect::<String>()
    };
    for c in corpus["cases"].as_array().unwrap() {
        let mut state = State::new(Pack::decode(raw.clone()).unwrap());
        state.transmit_timer = c["timer"].as_i64().unwrap() as i32;
        let mut vm = Vm::default();
        vm.chat_lines = c["size"].as_i64().unwrap() as i32;
        let key = Key::component(1);
        state.components.insert(
            key,
            Component {
                id: 1,
                // Hooks belong to IF3 components the walk reaches from the
                // open top-level interface.
                if3: true,
                width: 1,
                height: 1,
                has_event_handlers: true,
                on_msg: Some(vec![HookArg::Int(1)]),
                ..Component::default()
            },
        );
        state.loaded.insert(0);
        state.root = 0;
        let rect = Rect {
            x: 0,
            y: 0,
            w: 1,
            h: 1,
        };
        let nodes = [DrawNode {
            key,
            rect,
            clip: rect,
            paint_clip: rect,
            subtree_end: 0,
        }];
        for (step, expected) in c["steps"]
            .as_array()
            .unwrap()
            .iter()
            .zip(c["hooks"].as_array().unwrap())
        {
            if !step.is_null() {
                vm.add_message(
                    &mut state,
                    ChatLine {
                        kind: step["kind"].as_i64().unwrap() as i32,
                        phrase: step["phrase"].as_i64().unwrap() as i32,
                        text: decode(step["text"].as_str().unwrap()),
                        clan: decode(step["clan"].as_str().unwrap()),
                        name: decode(step["name"].as_str().unwrap()),
                    },
                );
            }
            state.queue_events(&nodes);
            assert_eq!(state.hooks.len(), expected.as_u64().unwrap() as usize);
            state.hooks.clear();
        }
        let mut output = format!(
            "{};{};{};{}",
            vm.chat_lines,
            state.chat_transmit_at,
            state.transmit_timer,
            state.components[&key].last_transmit_timer
        );
        for i in 0..100 {
            let fallback = ChatLine::default();
            let line = vm.messages.get(i).unwrap_or(&fallback);
            output.push_str(&format!(
                "|{},{},{},{},{}",
                line.kind,
                line.phrase,
                hex(&line.text),
                hex(&line.clan),
                hex(&line.name)
            ));
        }
        assert_eq!(output, c["expected"].as_str().unwrap());
    }
}
#[test]
fn client_skill_levels_and_stat_misc_notifications_match_original() {
    use rs09_client::{
        pack::Pack,
        ui::{
            HookArg, skills,
            state::{DrawNode, Key, Rect, State},
            vm::Vm,
        },
    };
    use rs09_protocol::game::Frame;
    let corpus: Value =
        serde_json::from_str(include_str!("../../../fixtures/ui/stats.json")).unwrap();
    assert_eq!(
        serde_json::json!(skills::thresholds().as_slice()),
        corpus["thresholds"]
    );
    let manifest = br#"{"source":"Controlled empty UI binding","files":{}}"#;
    let mut raw = b"RS09VIS1".to_vec();
    raw.extend((manifest.len() as u32).to_le_bytes());
    raw.extend(manifest);
    for c in corpus["cases"].as_array().unwrap() {
        let mut state = State::new(Pack::decode(raw.clone()).unwrap());
        state.stat_writer = c["writer"].as_i64().unwrap() as i32;
        state.transmit_timer = c["timer"].as_i64().unwrap() as i32;
        let mut vm = Vm::default();
        let key = Key::component(1);
        state.components.insert(
            key,
            Component {
                id: 1,
                if3: true,
                width: 1,
                height: 1,
                has_event_handlers: true,
                on_stat_transmit: Some(vec![HookArg::Int(1)]),
                on_misc_transmit: Some(vec![HookArg::Int(2)]),
                updated_stats_reader_index: c["reader"].as_i64().unwrap() as i32,
                stat_triggers: serde_json::from_value(c["triggers"].clone()).unwrap(),
                ..Component::default()
            },
        );
        state.loaded.insert(0);
        state.root = 0;
        let rect = Rect {
            x: 0,
            y: 0,
            w: 1,
            h: 1,
        };
        let nodes = [DrawNode {
            key,
            rect,
            clip: rect,
            paint_clip: rect,
            subtree_end: 0,
        }];
        let mut expected = c["hooks"].as_array().unwrap().iter();
        for step in c["steps"].as_array().unwrap() {
            if step.is_null() {
                state.queue_events(&nodes);
                assert_eq!(
                    state.hooks.len(),
                    expected.next().unwrap().as_u64().unwrap() as usize
                );
                state.hooks.clear();
            } else {
                let payload = step["data"]
                    .as_str()
                    .unwrap()
                    .as_bytes()
                    .chunks_exact(2)
                    .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
                    .collect();
                assert!(
                    vm.apply_frame(
                        &mut state,
                        &Frame {
                            opcode: step["op"].as_u64().unwrap() as u8,
                            payload
                        }
                    )
                    .unwrap()
                );
            }
        }
        assert_eq!(
            serde_json::json!([
                vm.boosted_levels.to_vec(),
                vm.base_levels.to_vec(),
                vm.experience.to_vec(),
                state.updated_stats.to_vec(),
                vec![
                    state.stat_writer,
                    state.components[&key].updated_stats_reader_index,
                    vm.run_energy,
                    vm.weight,
                    state.misc_transmit_at,
                    state.transmit_timer,
                    state.components[&key].last_transmit_timer
                ]
            ]),
            c["expected"]
        );
    }
}
