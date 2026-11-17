use super::*;
use crate::ui::{Component, HookArg, Properties, component_handles::Handle};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{cell::RefCell, rc::Rc};

struct Services {
    arguments: Vec<i32>,
    events: Vec<String>,
    source: Option<Handle>,
    wire: Vec<u8>,
}
fn append(services: &Rc<RefCell<Services>>, frame: rs09_protocol::game::Frame) {
    frame
        .write(
            rs09_protocol::game::Direction::ClientToServer,
            None,
            &mut services.borrow_mut().wire,
        )
        .unwrap();
}
fn component(key: Key, color: i32, flags: i32, code: i32, hook: bool) -> Component {
    Component {
        id: key.id,
        created_component_id: key.child,
        kind: 4,
        if3: key.child >= 0,
        color,
        client_code: code,
        properties: Properties {
            events: flags,
            ..Default::default()
        },
        on_option_click: hook.then_some(vec![HookArg::Int(1)]),
        ..Default::default()
    }
}
fn execute(
    state: &mut State,
    services: &Rc<RefCell<Services>>,
    key: Key,
    op: i32,
    base: &str,
) -> Result<(), String> {
    let result = state.button_with_hook(
        key,
        op,
        |state, source, _| {
            let color = state.component_ref(source).color;
            let a = {
                let mut s = services.borrow_mut();
                s.events.push(format!("hook:{color}:{op}:{base}"));
                s.source = Some(source.clone());
                s.arguments.clone()
            };
            match a[7] {
                1 => state.remove_component(key),
                2 => state.replace_component(key, component(key, 22, a[8], a[9], false)),
                3 => state.component_ref_mut(source).properties.events = a[8],
                4 => state.component_ref_mut(source).client_code = a[9],
                5 => {
                    if a[10] != 0 {
                        state.properties.insert(
                            key,
                            Properties {
                                events: a[11],
                                ..Default::default()
                            },
                        );
                    } else {
                        state.properties.remove(&key);
                    }
                }
                6 => {
                    state.remove_component(key);
                    state.properties.remove(&key);
                    append(
                        services,
                        rs09_protocol::game::Frame {
                            opcode: 184,
                            payload: Vec::new(),
                        },
                    );
                }
                7 => return Err("hook".into()),
                8 => {
                    state.replace_component(key, component(key, 22, a[8], 0, false));
                    execute(state, services, key, 2, "nested")?;
                }
                _ => {}
            }
            Ok(())
        },
        |state, source| {
            let c = state.component_ref(source);
            let mut s = services.borrow_mut();
            s.events
                .push(format!("special:{}:{}", c.color, c.client_code));
            if s.arguments[12] == 2 {
                return Err("special".into());
            }
            Ok(s.arguments[12] == 0)
        },
    );
    if let Some(frame) = result? {
        append(services, frame);
    }
    Ok(())
}
fn fields(c: Option<&Component>) -> Value {
    c.map_or(Value::Null, |c| {
        json!([c.color, c.properties.events, c.client_code])
    })
}

#[test]
fn whole_original_button_callbacks_keep_captured_object_and_nested_wire_prefix() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let f: Value = serde_json::from_slice(
        &std::fs::read(root.join("fixtures/ui/button-clicks.json")).unwrap(),
    )
    .unwrap();
    for (name, hash) in f["sources"].as_object().unwrap() {
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
    assert_eq!(f["cases"].as_array().unwrap().len(), 768);
    for (index, row) in f["cases"].as_array().unwrap().iter().enumerate() {
        let a = row["arguments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_i64().unwrap() as i32)
            .collect::<Vec<_>>();
        let key = Key {
            id: 65536,
            child: a[13],
        };
        let mut state = super::legacy::legacy_tests::empty();
        if key.child >= 0 {
            state.components.insert(
                Key::component(key.id),
                component(Key::component(key.id), 0, 0, 0, false),
            );
            state.created_lengths.insert(key.id, 1);
        }
        if a[0] != 0 {
            state
                .components
                .insert(key, component(key, 11, a[3], a[4], a[1] != 0));
        }
        if a[5] != 0 {
            state.properties.insert(
                key,
                Properties {
                    events: a[6],
                    ..Default::default()
                },
            );
        }
        let services = Rc::new(RefCell::new(Services {
            arguments: a.clone(),
            events: Vec::new(),
            source: None,
            wire: Vec::new(),
        }));
        let error = execute(&mut state, &services, key, a[2], "outer").err();
        let source = services.borrow().source.clone();
        let source = fields(source.as_ref().map(|h| state.component_ref(h)));
        let s = services.borrow();
        let wire = s
            .wire
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert_eq!(
            json!({"error":error,"wire":wire,"events":s.events,"current":fields(state.components.get(&key)),"source":source,"override":state.properties.get(&key).map(|p|p.events)}),
            row["expected"],
            "button callback {index}"
        );
        assert!(state.diagnostics.is_empty());
    }
}

#[test]
fn actual_vm_close_hook_retires_button_and_emits_callback_before_enabled_outer_packet() {
    for code in [0, 205] {
        // Original-format CS2: CLOSE_MODAL3103, RETURN21; no arguments or locals.
        let mut script = vec![0, 12, 31, 0, 0, 21, 0];
        script.extend(2i32.to_be_bytes());
        script.extend([0; 8]);
        script.push(0);
        script.extend(1u16.to_be_bytes());
        let manifest=serde_json::to_vec(&json!({"source":"authored CS2 modal-close callback","files":{"script/42":[0,script.len()]}})).unwrap();
        let mut bytes = b"RS09VIS1".to_vec();
        bytes.extend((manifest.len() as u32).to_le_bytes());
        bytes.extend(manifest);
        bytes.extend(script);
        let mut state = State::new(crate::pack::Pack::decode(bytes).unwrap());
        state.loaded.extend([1, 2]);
        state.root = 2;
        let key = Key::component(65536);
        let parent = Key::component(131072);
        let mut c = component(key, 11, 2, code, true);
        c.on_option_click = Some(vec![HookArg::Int(42)]);
        state.components.insert(key, c);
        state.components.insert(
            parent,
            Component {
                id: parent.id,
                kind: 0,
                ..Default::default()
            },
        );
        state.subinterfaces.insert(parent.id, (1, 0));
        state.properties.insert(
            key,
            Properties {
                events: 0,
                ..Default::default()
            },
        );
        let mut vm = crate::ui::vm::Vm::default();
        vm.click_button(&mut state, key, 1, "Close").unwrap();
        assert!(!state.components.contains_key(&key));
        assert!(state.subinterfaces.is_empty());
        assert!(state.diagnostics.is_empty());
        assert_eq!(
            vm.outgoing.iter().map(|f| f.opcode).collect::<Vec<_>>(),
            [184, 155]
        );
        assert_eq!(vm.outgoing[1].payload, [0, 1, 0, 0, 255, 255]);

        assert_eq!(
            vm.logout_on_disconnect_timer,
            if code == 205 { 250 } else { 0 }
        );
    }
}
