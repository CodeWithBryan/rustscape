//! Server messages decoded by `ServerEvent`, applied through the real VM and
//! the DelayedStateChange queue. Payloads are written with the original
//! byte orders; expected effects are from RT4 `Protocol.readPacketInternal`
//! and its per-cycle DelayedStateChange loop.
use super::*;
use crate::{
    pack::Pack,
    ui::{Component, Properties, state::Key},
};
use rs09_protocol::{buffer::Writer, game::Frame as Wire};

const GROUP: i32 = 7;
fn pointer(child: i32) -> i32 {
    (GROUP << 16) | child
}
fn setup() -> (State, Vm) {
    let manifest =
        serde_json::to_vec(&serde_json::json!({"source":"Server event tests","files":{}})).unwrap();
    let mut bytes = b"RS09VIS1".to_vec();
    bytes.extend((manifest.len() as u32).to_le_bytes());
    bytes.extend(manifest);
    let mut state = State::new(Pack::decode(bytes).unwrap());
    state.loaded.insert(GROUP);
    for child in 0..4 {
        let key = Key::component(pointer(child));
        state.components.insert(
            key,
            Component {
                id: key.id,
                height: 100,
                scroll_max_v: 250,
                properties: Properties {
                    events: 0x30,
                    target_param: 9,
                },
                ..Default::default()
            },
        );
    }
    (state, Vm::default())
}
fn send(vm: &mut Vm, state: &mut State, opcode: u8, w: Writer) -> bool {
    vm.apply_frame(
        state,
        &Wire {
            opcode,
            payload: w.into_vec(),
        },
    )
    .unwrap()
}
fn component(state: &State, child: i32) -> &Component {
    state.get(Key::component(pointer(child))).unwrap()
}

#[test]
fn colour_scroll_rotation_and_body_head_wait_for_the_delayed_poll() {
    let (mut state, mut vm) = setup();
    let mut w = Writer::new();
    w.imp4(pointer(0));
    w.p2add(40);
    // Red 31, green 0, blue 16 in 15 bits.
    w.ip2add((31 << 10) | 16);
    assert!(send(&mut vm, &mut state, 2, w));
    assert_eq!(state.verify_id, 41);

    let mut w = Writer::new();
    w.mp4(pointer(1));
    w.ip2(900);
    w.p2(41);
    assert!(send(&mut vm, &mut state, 220, w));

    let mut w = Writer::new();
    w.mp4(pointer(2));
    w.p2add(42);
    w.p2(3);
    w.p2add(5);
    assert!(send(&mut vm, &mut state, 207, w));

    let mut w = Writer::new();
    w.p2add(65535);
    w.mp4(pointer(3));
    w.ip2add(11);
    w.ip2(2);
    w.ip2add(9);
    assert!(send(&mut vm, &mut state, 111, w));
    // The counter wraps to zero exactly as setVerifyId's 16-bit mask does.
    assert_eq!(state.verify_id, 0);

    // Nothing is visible until the per-cycle poll applies the queue.
    assert_eq!(component(&state, 0).color, 0);
    assert_eq!(component(&state, 1).scroll_y, 0);
    vm.poll_updates(&mut state, 0).unwrap();
    assert_eq!(component(&state, 0).color, (31 << 19) | (16 << 3));
    // 900 exceeds scrollMaxV - height and is clamped.
    assert_eq!(component(&state, 1).scroll_y, 150);
    assert_eq!(component(&state, 2).model_rotation_speed, 5 + (3 << 16));
    let head = component(&state, 3);
    assert_eq!(
        (head.model_type, head.model_frame_cycle, head.model_id),
        (7, 11, (2 << 16) | 9)
    );
}

#[test]
fn scroll_ignores_non_layers_and_clamps_negative_positions() {
    let (mut state, mut vm) = setup();
    state.get_mut(Key::component(pointer(0))).unwrap().kind = 4;
    state.get_mut(Key::component(pointer(1))).unwrap().scroll_y = 60;
    for (child, position) in [(0, 20), (1, 65535)] {
        let mut w = Writer::new();
        w.mp4(pointer(child));
        w.ip2(position);
        w.p2(1);
        assert!(send(&mut vm, &mut state, 220, w));
    }
    // A component that does not exist is skipped without failing the poll.
    let mut w = Writer::new();
    w.mp4(pointer(99));
    w.ip2(5);
    w.p2(1);
    assert!(send(&mut vm, &mut state, 220, w));
    vm.poll_updates(&mut state, 0).unwrap();
    assert_eq!(component(&state, 0).scroll_y, 0);
    // The wire value is unsigned, so 65535 clamps to the maximum.
    assert_eq!(component(&state, 1).scroll_y, 150);
}

#[test]
fn varc_strings_use_either_packet_and_notify_the_ring() {
    let (mut state, mut vm) = setup();
    let mut w = Writer::new();
    w.ip2(12);
    w.p2add(7);
    w.pjstr(b"first").unwrap();
    assert!(send(&mut vm, &mut state, 123, w));
    let mut w = Writer::new();
    w.p2(8);
    w.pjstr(b"second").unwrap();
    w.ip2add(13);
    assert!(send(&mut vm, &mut state, 48, w));
    assert_eq!(state.verify_id, 9);
    let before = state.varcstr_writer;
    vm.poll_updates(&mut state, 0).unwrap();
    assert_eq!(vm.varcstrs[&12], "first");
    assert_eq!(vm.varcstrs[&13], "second");
    assert_eq!(state.varcstr_writer, before + 2);
}

#[test]
fn target_parameter_keeps_events_and_events_keep_the_target_parameter() {
    let (mut state, mut vm) = setup();
    // Child -1 inherits the component's own events; other children start at 0.
    let mut w = Writer::new();
    w.ip2add(77);
    w.ip4(pointer(0));
    w.p2add(1);
    w.ip2(1);
    w.p2add(65535);
    assert!(send(&mut vm, &mut state, 9, w));
    let get = |state: &State, child| {
        state.properties[&Key {
            id: pointer(0),
            child,
        }]
    };
    assert_eq!(
        (get(&state, -1).events, get(&state, -1).target_param),
        (0x30, 77)
    );
    assert_eq!(
        (get(&state, 1).events, get(&state, 1).target_param),
        (0, 77)
    );
    // SET_INTERFACE_SETTINGS then replaces events and keeps that parameter.
    let frame = rs09_protocol::game::updates::interface_events(pointer(0), 0, 1, 0x7f, 2);
    assert!(vm.apply_frame(&mut state, &frame).unwrap());
    assert_eq!(
        (get(&state, 1).events, get(&state, 1).target_param),
        (0x7f, 77)
    );
    // And a second target parameter keeps those events.
    let mut w = Writer::new();
    w.ip2add(78);
    w.ip4(pointer(0));
    w.p2add(3);
    w.ip2(1);
    w.p2add(1);
    assert!(send(&mut vm, &mut state, 9, w));
    assert_eq!(
        (get(&state, 1).events, get(&state, 1).target_param),
        (0x7f, 78)
    );
}

#[test]
fn switch_widget_moves_or_replaces_the_open_interface() {
    let (mut state, mut vm) = setup();
    state.loaded.insert(20);
    state.loaded.insert(21);
    state.subinterfaces.insert(pointer(0), (20, 0));
    state.subinterfaces.insert(pointer(1), (21, 1));
    let mut w = Writer::new();
    w.imp4(pointer(0));
    w.p2add(5);
    w.imp4(pointer(1));
    assert!(send(&mut vm, &mut state, 176, w));
    assert_eq!(state.verify_id, 6);
    assert_eq!(state.subinterfaces.get(&pointer(0)), None);
    assert_eq!(state.subinterfaces.get(&pointer(1)), Some(&(20, 0)));
    // The replaced interface differed from the moved one, so it was unloaded.
    assert!(!state.loaded.contains(&21) && state.loaded.contains(&20));

    // With no source the target is simply closed and unloaded.
    let mut w = Writer::new();
    w.imp4(pointer(2));
    w.p2add(6);
    w.imp4(pointer(1));
    assert!(send(&mut vm, &mut state, 176, w));
    assert!(state.subinterfaces.is_empty() && !state.loaded.contains(&20));
}

#[test]
fn inventory_delete_reboot_timer_and_social_stamps() {
    let (mut state, mut vm) = setup();
    state.transmit_timer = 50;
    state.containers.insert(
        93,
        vec![rs09_protocol::game::updates::WireItem { id: 995, amount: 1 }],
    );
    let writer = state.inventory_writer;
    let mut w = Writer::new();
    w.ip2(93);
    assert!(send(&mut vm, &mut state, 191, w));
    assert!(!state.containers.contains_key(&93));
    assert_eq!(state.inventory_writer, writer + 1);
    assert_eq!(state.updated_inventories[(writer & 31) as usize], 93);

    let mut w = Writer::new();
    w.p2(20);
    assert!(send(&mut vm, &mut state, 85, w));
    assert_eq!((vm.reboot_timer, state.misc_transmit_at), (600, 50));

    state.transmit_timer = 51;
    let mut w = Writer::new();
    w.p1(2);
    assert!(send(&mut vm, &mut state, 197, w));
    assert_eq!((vm.social.friend_state, state.friend_transmit_at), (2, 51));

    state.transmit_timer = 52;
    let mut w = Writer::new();
    w.p1(1);
    w.p1(2);
    w.p2(4151);
    w.p4(100);
    w.p4(5);
    w.p4(0);
    w.p4(0);
    assert!(send(&mut vm, &mut state, 116, w));
    assert_eq!(state.stock_transmit_at, 52);
    assert_eq!(
        (vm.social.offers[1].item, vm.social.offers[1].count),
        (4151, 5)
    );

    state.transmit_timer = 53;
    let mut w = Writer::new();
    w.p8(0);
    assert!(send(&mut vm, &mut state, 55, w));
    assert_eq!(state.clan_transmit_at, 53);
}

#[test]
fn private_message_reaches_the_chat_log() {
    let (mut state, mut vm) = setup();
    state.transmit_timer = 9;
    let mut w = Writer::new();
    w.p8(crate::ui::social::encode("bob"));
    w.p2(1);
    w.p3(2);
    w.p1(0);
    w.p1(2);
    w.pdata(&rs09_protocol::game::chat::WORDPACK.encode(b"hi").unwrap());
    assert!(send(&mut vm, &mut state, 0, w));
    assert_eq!(
        (
            vm.messages[0].kind,
            vm.messages[0].name.as_str(),
            vm.messages[0].text.as_str()
        ),
        (3, "Bob", "Hi")
    );
    assert_eq!(state.chat_transmit_at, 9);
    // Standing on Tutorial Island suppresses player messages.
    vm.location = [3100, 3100, 0];
    let mut w = Writer::new();
    w.p8(crate::ui::social::encode("bob"));
    w.p2(1);
    w.p3(3);
    w.p1(0);
    w.p1(2);
    w.pdata(&rs09_protocol::game::chat::WORDPACK.encode(b"hi").unwrap());
    assert!(send(&mut vm, &mut state, 0, w));
    assert_eq!(vm.messages.len(), 1);
}

#[test]
fn varp_cache_reset_and_forced_refresh() {
    let (mut state, mut vm) = setup();
    let frame = rs09_protocol::game::updates::varp(300, 5);
    assert!(vm.apply_frame(&mut state, &frame).unwrap());
    // FORCE_VARP_REFRESH publishes the pending value without waiting for a poll.
    let writer = state.varp_writer;
    assert!(send(&mut vm, &mut state, 128, Writer::new()));
    assert_eq!(vm.varps.get(&300), Some(&5));
    assert_eq!(state.varp_writer, writer + 1);
    assert_eq!(state.updated_varps[(writer & 31) as usize], 300);
    // A second refresh has nothing new to publish.
    assert!(send(&mut vm, &mut state, 128, Writer::new()));
    assert_eq!(state.varp_writer, writer + 1);
    // RESET_CLIENT_VARCACHE invalidates every listener. This pack defines no
    // varp types, so the original loop has nothing to zero; which varps are
    // cleared is covered by the tests in vm_semantics.rs.
    assert!(send(&mut vm, &mut state, 89, Writer::new()));
    assert_eq!(vm.varps.get(&300), Some(&5));
    assert_eq!(vm.pending_varps.values.get(&300), Some(&5));
    assert_eq!(state.varp_writer, writer + 33);
}

#[test]
fn walk_text_settings_url_and_last_login() {
    let (mut state, mut vm) = setup();
    let mut w = Writer::new();
    w.pjstr(b"Dig here").unwrap();
    assert!(send(&mut vm, &mut state, 160, w));
    assert_eq!(vm.walk_text.as_deref(), Some("Dig here"));
    // An empty packet restores the localized default.
    assert!(send(&mut vm, &mut state, 160, Writer::new()));
    assert_eq!(vm.walk_text, None);

    let mut w = Writer::new();
    w.pjstr(b"cookie").unwrap();
    assert!(send(&mut vm, &mut state, 142, w));
    assert_eq!(vm.settings.as_deref(), Some("cookie"));

    let mut w = Writer::new();
    w.pdata(b"http://example.invalid/");
    assert!(send(&mut vm, &mut state, 42, w));
    assert_eq!(vm.requested_urls, ["http://example.invalid/"]);

    let mut w = Writer::new();
    w.imp4(0x7f00_0001);
    assert!(send(&mut vm, &mut state, 164, w));
    // The address is recorded with no lookup result; rendering formats it.
    assert_eq!(state.last_login_address, Some((0x7f00_0001, None)));
}

#[test]
fn a_script_drags_its_active_component_from_the_given_offset() {
    use crate::ui::{
        drag::Controller,
        state::{DrawNode, Rect},
    };
    let (mut state, mut vm) = setup();
    let key = Key::component(pointer(0));
    let parent = Key::component(pointer(1));
    // CS2 3109 names the active component of whichever kind the opcode
    // selects, and takes the press offset as (x, y).
    vm.active[0] = Some(state.retain_component(key).unwrap());
    let mut stack = Stack {
        ints: vec![7, 9],
        strings: vec![],
    };
    vm.operation(&mut state, 3109, true, &mut stack).unwrap();
    assert!(stack.ints.is_empty());
    assert_eq!(vm.drag_requests, [(key, [7, 9])]);
    // Without an active component of that kind the script fails.
    assert!(
        vm.operation(
            &mut state,
            3109,
            false,
            &mut Stack {
                ints: vec![1, 2],
                strings: vec![]
            }
        )
        .is_err()
    );

    let rect = Rect {
        x: 100,
        y: 50,
        w: 30,
        h: 20,
    };
    let nodes = [DrawNode {
        key,
        rect,
        clip: rect,
        paint_clip: rect,
        subtree_end: 1,
    }];
    let mut drag = Controller::default();
    // No drag parent: startComponentDrag does nothing.
    assert!(!drag.begin_scripted(&state, &nodes, key, [7, 9], 3, false));
    state.drag_parents.insert(key, parent);
    // Not while the menu is open, and not for a component that is not shown.
    assert!(!drag.begin_scripted(&state, &nodes, key, [7, 9], 3, true));
    assert!(!drag.begin_scripted(&state, &[], key, [7, 9], 3, false));
    assert!(drag.begin_scripted(&state, &nodes, key, [7, 9], 3, false));
    assert!(drag.active());
    assert_eq!(drag.snapshot(None)["press"], serde_json::json!([107, 59]));
    // A second request is ignored while a drag is in progress.
    assert!(!drag.begin_scripted(&state, &nodes, parent, [0, 0], 4, false));
}

#[test]
fn a_drag_the_title_walk_begins_follows_the_mouse_from_its_click() {
    // Transcribed from Cs1ScriptRunner.startComponentDrag and
    // updateComponentDrag (Cs1ScriptRunner.java:1348-1412), which the
    // title's client.mainUpdate runs after its hooks (client.java:1227-1229):
    // the component keeps the click's offset inside it, the cycle of the
    // click is the first update, and moving past the dead zone after the
    // dead time runs onDragStart at the component's place in its parent.
    use crate::ui::{
        drag::Controller,
        state::{DrawNode, Rect},
    };
    let (mut state, _) = setup();
    let key = Key::component(pointer(0));
    let parent = Key::component(pointer(1));
    let node = |key, rect| DrawNode {
        key,
        rect,
        clip: rect,
        paint_clip: rect,
        subtree_end: 0,
    };
    let source = Rect {
        x: 100,
        y: 50,
        w: 30,
        h: 20,
    };
    let area = Rect {
        x: 0,
        y: 0,
        w: 400,
        h: 300,
    };
    let nodes = [node(parent, area), node(key, source)];
    state.components.get_mut(&key).unwrap().on_drag_start = Some(vec![HookArg::Int(1)]);
    let mut drag = Controller::default();
    // No drag parent: nothing to drag within.
    assert!(!drag.begin_walked(&state, &nodes, key, [110, 55], 10));
    state.drag_parents.insert(key, parent);
    assert!(!drag.begin_walked(&state, &[], key, [110, 55], 10));
    // Pressed and let go in the same cycle: no drag, nothing run.
    assert!(drag.begin_walked(&state, &nodes, key, [110, 55], 10));
    let (hooks, click, release) = drag.step(&mut state, &nodes, [110, 55], 10, false, false);
    assert!(hooks.is_empty() && click.is_none() && release.is_none());
    assert!(!drag.active());
    // Held: still within the dead zone at the click's own cycle.
    assert!(drag.begin_walked(&state, &nodes, key, [110, 55], 20));
    assert!(!drag.begin_walked(&state, &nodes, parent, [5, 5], 20));
    assert!(
        drag.step(&mut state, &nodes, [110, 55], 20, true, false)
            .0
            .is_empty()
    );
    // Moved 20 to the right: the component is at x 120 in its parent.
    let (hooks, _, _) = drag.step(&mut state, &nodes, [130, 55], 21, true, false);
    assert_eq!(hooks.len(), 1);
    assert_eq!(hooks[0].mouse, [120, 50]);
    // Let go: the drag ends.
    drag.step(&mut state, &nodes, [130, 55], 22, false, false);
    assert!(!drag.active());
}
