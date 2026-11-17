use super::*;
use serde_json::Value;
use sha2::{Digest, Sha256};

#[test]
fn original_continue_wire_wait_aliases_missing_children_and_close_resets_match() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture: Value =
        serde_json::from_slice(&std::fs::read(root.join("fixtures/ui/widget-wait.json")).unwrap())
            .unwrap();
    for (name, hash) in fixture["sources"].as_object().unwrap() {
        let s = std::fs::read_to_string(
            root.join("reference/client/client/src/main/java/rt4")
                .join(name),
        )
        .unwrap()
        .replace("\r\n", "\n");
        assert_eq!(
            format!("{:x}", Sha256::digest(s.as_bytes())),
            hash.as_str().unwrap()
        );
    }
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 256);
    for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let mut state = super::legacy_tests::empty();
        let mut vm = crate::ui::vm::Vm::default();
        let base = 65536;
        state.created_lengths.insert(base, 3);
        for slot in -1..3 {
            let key = Key {
                id: base,
                child: slot,
            };
            state.components.insert(
                key,
                crate::ui::Component {
                    id: base,
                    created_component_id: slot,
                    kind: 4,
                    if3: slot >= 0,
                    overlayer: if slot < 0 { -1 } else { base },
                    color: 101 + slot,
                    ..Default::default()
                },
            );
        }
        let pane = case["paneKind"].as_i64().unwrap() as i32;
        if pane >= 0 {
            state.loaded.insert(2);
            state.subinterfaces.insert(base, (2, pane));
            state.components.insert(
                Key::component(131072),
                crate::ui::Component {
                    id: 131072,
                    kind: 4,
                    ..Default::default()
                },
            );
        }
        let mut wire = Vec::new();
        let mut observed = Vec::new();
        for (i, step) in case["steps"].as_array().unwrap().iter().enumerate() {
            let op = step[0].as_i64().unwrap();
            let slot = step[1].as_i64().unwrap() as i32;
            let key = Key {
                id: base,
                child: slot,
            };
            match op {
                0 => {
                    if let Some(frame) = state.request_continue(key) {
                        frame
                            .write(
                                rs09_protocol::game::Direction::ClientToServer,
                                None,
                                &mut wire,
                            )
                            .unwrap();
                    }
                }
                1 => {
                    let mut c =
                        state
                            .components
                            .get(&key)
                            .cloned()
                            .unwrap_or(crate::ui::Component {
                                id: base,
                                created_component_id: slot,
                                kind: 4,
                                if3: slot >= 0,
                                overlayer: base,
                                ..Default::default()
                            });
                    c.color = 201 + i as i32;
                    state.replace_component(key, c);
                }
                2 => state.remove_component(key),
                3 => state.clear_waiting(),
                4 => {
                    vm.close_modal(&mut state);
                    for frame in vm.outgoing.drain(..) {
                        frame
                            .write(
                                rs09_protocol::game::Direction::ClientToServer,
                                None,
                                &mut wire,
                            )
                            .unwrap();
                    }
                }
                _ => unreachable!(),
            }
            let color = state
                .please_wait
                .as_ref()
                .map_or(-1, |h| state.component_ref(h).color);
            let hex = wire.iter().map(|b| format!("{b:02x}")).collect::<String>();
            observed.push(serde_json::json!([color, hex, state.subinterfaces.len()]));
        }
        assert_eq!(
            serde_json::json!(observed),
            case["expected"],
            "waiting lifecycle {index}"
        );
        assert!(
            state.diagnostics.is_empty(),
            "Missing but loaded child is an ordinary null lookup: {index} {:?}",
            state.diagnostics
        );
    }
}
