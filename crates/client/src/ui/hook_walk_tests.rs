//! Transcription tests for the event sources around the hook walk that the
//! JVM corpus (`tests/ui_fidelity.rs`) does not reach: `resetAnimations` on
//! opening an interface (`Protocol` 1288, 3492), the forced layout of a moved
//! interface (`Protocol` 1742) and the discard of requests whose created
//! component was replaced (`Protocol.loop` 2994-3017). Expected values are
//! worked out by hand from those lines.
use super::super::{Hook, Key, State};
use crate::{
    pack::Pack,
    ui::{Component, HookArg, vm::Vm},
};

fn state() -> State {
    let manifest = br#"{"source":"Hook walk tests","files":{}}"#;
    let mut raw = b"RS09VIS1".to_vec();
    raw.extend((manifest.len() as u32).to_le_bytes());
    raw.extend(manifest);
    State::new(Pack::decode(raw).unwrap())
}
fn add(state: &mut State, key: Key, component: Component) {
    state.loaded.insert(key.id >> 16);
    state.components.insert(
        key,
        Component {
            id: key.id,
            created_component_id: key.child,
            if3: true,
            ..component
        },
    );
}
fn marker(hook: &Hook) -> i32 {
    match hook.arguments.first() {
        Some(HookArg::Int(value)) => *value,
        _ => panic!("hook without marker"),
    }
}

#[test]
fn opening_an_interface_restarts_its_static_animations() {
    let mut state = state();
    let running = Component {
        seq_frame: 5,
        seq_next_frame: 6,
        seq_cycle: 9,
        base_width: 50,
        base_height: 50,
        ..Component::default()
    };
    for group in [3, 4] {
        add(&mut state, Key::component(group << 16), running.clone());
        add(&mut state, Key::component(group << 16 | 1), running.clone());
    }
    // A created component is not in the interface's static array.
    let created = Key {
        id: 4 << 16,
        child: 0,
    };
    add(
        &mut state,
        created,
        Component {
            overlayer: 4 << 16,
            ..running.clone()
        },
    );
    let frames = |state: &State, key: Key| {
        let c = &state.components[&key];
        [c.seq_frame, c.seq_next_frame, c.seq_cycle]
    };
    state.open_root(3).unwrap();
    assert_eq!(frames(&state, Key::component(3 << 16)), [0, 1, 0]);
    assert_eq!(frames(&state, Key::component(3 << 16 | 1)), [0, 1, 0]);
    assert_eq!(frames(&state, Key::component(4 << 16)), [5, 6, 9]);
    state.open_sub(3 << 16, 4, 0).unwrap();
    assert_eq!(frames(&state, Key::component(4 << 16)), [0, 1, 0]);
    assert_eq!(frames(&state, Key::component(4 << 16 | 1)), [0, 1, 0]);
    assert_eq!(frames(&state, created), [5, 6, 9]);
}

#[test]
fn only_a_moved_interface_is_laid_out_with_resize_hooks() {
    let mut state = state();
    let layer = |width, height| Component {
        kind: 0,
        base_width: width,
        base_height: height,
        ..Component::default()
    };
    // Two hosts of different sizes in the top level, and an interface whose
    // root fills whichever host it is opened in.
    add(&mut state, Key::component(1 << 16), layer(100, 80));
    add(&mut state, Key::component(1 << 16 | 1), layer(300, 200));
    let filling = Component {
        kind: 0,
        dynamic_width_value: 1,
        dynamic_height_value: 1,
        on_resize: Some(vec![HookArg::Int(22)]),
        has_event_handlers: true,
        ..Component::default()
    };
    add(&mut state, Key::component(2 << 16), filling.clone());
    // Another resizable component elsewhere must not be notified.
    add(
        &mut state,
        Key::component(1 << 16 | 2),
        Component {
            base_width: 10,
            base_height: 10,
            on_resize: Some(vec![HookArg::Int(99)]),
            has_event_handlers: true,
            ..Component::default()
        },
    );
    state.open_root(1).unwrap();
    // Opening lays out without the resize trigger (layoutComponent(parent,
    // false)), although the root's size changes from 0 by 0 to 100 by 80.
    state.open_sub(1 << 16, 2, 0).unwrap();
    assert_eq!(
        (
            state.components[&Key::component(2 << 16)].width,
            state.components[&Key::component(2 << 16)].height
        ),
        (100, 80)
    );
    assert!(state.pending_resize_hooks().is_empty());
    // Moving it to the larger host is layoutComponent(target, true).
    state.move_sub(1 << 16, 1 << 16 | 1).unwrap();
    assert_eq!(state.components[&Key::component(2 << 16)].width, 300);
    assert_eq!(
        state
            .pending_resize_hooks()
            .iter()
            .map(|hook| (hook.source, marker(hook)))
            .collect::<Vec<_>>(),
        vec![(Key::component(2 << 16), 22)]
    );
    // The next walk hands the request to the script queue.
    state.hooks.clear();
    state.queue_input_events(&[], None);
    assert!(state.pending_resize_hooks().is_empty());
    assert_eq!(state.hooks.iter().map(marker).collect::<Vec<_>>(), [22]);
    // An unforced layout at the same canvas size raises nothing.
    state.layout(765, 503).unwrap();
    assert!(state.pending_resize_hooks().is_empty());
    // A new canvas size is layoutTopLevel(true), but only changed sizes
    // count: nothing here depends on the canvas.
    state.layout(800, 600).unwrap();
    assert!(state.pending_resize_hooks().is_empty());
}

#[test]
fn requests_for_a_replaced_created_component_are_dropped() {
    let mut state = state();
    add(
        &mut state,
        Key::component(5 << 16),
        Component {
            kind: 0,
            base_width: 100,
            base_height: 100,
            ..Component::default()
        },
    );
    let timed = |marker| Component {
        kind: 3,
        overlayer: 5 << 16,
        base_width: 10,
        base_height: 10,
        on_timer: Some(vec![HookArg::Int(marker)]),
        has_event_handlers: true,
        ..Component::default()
    };
    let (kept, replaced, deleted) = (
        Key {
            id: 5 << 16,
            child: 0,
        },
        Key {
            id: 5 << 16,
            child: 1,
        },
        Key {
            id: 5 << 16,
            child: 2,
        },
    );
    for (key, marker) in [(kept, 900001), (replaced, 900002), (deleted, 900003)] {
        add(&mut state, key, timed(marker));
    }
    state.created_lengths.insert(5 << 16, 3);
    // A static component's request is never checked.
    add(
        &mut state,
        Key::component(5 << 16 | 1),
        Component {
            overlayer: -1,
            ..timed(900004)
        },
    );
    state.open_root(5).unwrap();
    state.hooks.clear();
    state.queue_input_events(&[], None);
    assert_eq!(
        state.hooks.iter().map(marker).collect::<Vec<_>>(),
        [900001, 900002, 900003, 900004]
    );
    // A script replaces one slot with a new component and deletes another
    // before the requests run.
    state.replace_component(
        replaced,
        Component {
            id: replaced.id,
            created_component_id: 1,
            if3: true,
            ..timed(900005)
        },
    );
    state.remove_component(deleted);
    let current: Vec<bool> = state
        .hooks
        .iter()
        .map(|hook| state.hook_is_current(hook))
        .collect();
    assert_eq!(current, [true, false, false, true]);
    // The script queue runs the two live requests (their scripts do not
    // exist in this empty pack, which is how the attempt shows) and skips
    // the two stale ones without a word.
    let mut vm = Vm::default();
    vm.drain(&mut state);
    assert!(state.hooks.is_empty());
    let attempts: Vec<&str> = state
        .diagnostics
        .iter()
        .filter_map(|text| text.rsplit_once("script/").map(|(_, id)| id))
        .collect();
    assert_eq!(attempts, ["900001", "900004"], "{:?}", state.diagnostics);
    assert_eq!(vm.executed, 0);
    // The replacement is a component in its own right: once it has been
    // laid out (until then it is clipped to nothing) its request runs.
    state.layout(765, 503).unwrap();
    state.queue_input_events(&[], None);
    assert_eq!(
        state.hooks.iter().map(marker).collect::<Vec<_>>(),
        [900001, 900005, 900004]
    );
    assert!(state.hooks.iter().all(|hook| state.hook_is_current(hook)));
}
