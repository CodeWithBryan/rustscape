//! `menu_at_with` over synthetic node lists: where the viewport's entries
//! sit in the drawing order and what removes them. Expectations transcribed
//! from Cs1ScriptRunner.renderComponent (lines 387-399 and 541-579).
use super::*;
use crate::{
    interaction::{Menu, MenuEntry},
    ui::{Component, viewport},
};

const WORLD: i32 = i32::MIN;

fn state() -> State {
    let manifest =
        serde_json::to_vec(&serde_json::json!({"source":"menu test","files":{}})).unwrap();
    let mut bytes = b"RS09VIS1".to_vec();
    bytes.extend((manifest.len() as u32).to_le_bytes());
    bytes.extend(manifest);
    State::new(Pack::decode(bytes).unwrap())
}

fn entry(action: i32, option: &str) -> MenuEntry {
    MenuEntry {
        option: option.into(),
        target: String::new(),
        action,
        key: 0,
        args: [0; 2],
        cursor: -1,
    }
}

fn node(key: Key, rect: [i32; 4], end: usize) -> DrawNode {
    let rect = Rect {
        x: rect[0],
        y: rect[1],
        w: rect[2],
        h: rect[3],
    };
    DrawNode {
        key,
        rect,
        clip: rect,
        paint_clip: rect,
        subtree_end: end,
    }
}

/// The menu a mouse at `point` gets, with the viewport marking its place
/// the way the client's main loop does.
fn menu(state: &mut State, nodes: &[DrawNode], point: [i32; 2]) -> Vec<(i32, String)> {
    let mut menu = Menu::default();
    menu.add(entry(1005, "Cancel"));
    state.menu_at_with(nodes, point, &mut menu, |c, _, menu| {
        (c.client_code == viewport::CLIENT_CODE).then(|| {
            menu.add(entry(WORLD, "world"));
            false
        })
    });
    menu.entries
        .into_iter()
        .map(|e| (e.action, e.option))
        .collect()
}

fn layer(id: i32) -> Component {
    Component {
        id,
        created_component_id: -1,
        kind: 0,
        if3: true,
        ..Default::default()
    }
}

fn button(id: i32, op: &str) -> Component {
    Component {
        id,
        created_component_id: -1,
        kind: 3,
        if3: true,
        ops: Some(vec![Some(op.into())]),
        properties: crate::ui::Properties {
            events: 2,
            ..Default::default()
        },
        ..Default::default()
    }
}

const ROOT: i32 = 548 << 16;

fn frame(state: &mut State) -> (Key, Key, Key, Key) {
    let keys = (
        Key::component(ROOT),
        Key::component(ROOT | 1),
        Key::component(ROOT | 2),
        Key::component(ROOT | 3),
    );
    state.components.insert(keys.0, layer(ROOT));
    state.components.insert(
        keys.1,
        Component {
            client_code: viewport::CLIENT_CODE,
            ..layer(ROOT | 1)
        },
    );
    state.components.insert(keys.2, button(ROOT | 2, "Under"));
    state.components.insert(keys.3, button(ROOT | 3, "Over"));
    keys
}

#[test]
fn world_entries_sit_where_the_viewport_is_drawn() {
    let mut state = state();
    let (root, view, under, over) = frame(&mut state);
    let nodes = [
        node(root, [0, 0, 765, 503], 4),
        node(under, [100, 100, 50, 50], 2),
        node(view, [4, 4, 512, 334], 3),
        node(over, [120, 120, 50, 50], 4),
    ];
    // Under the mouse: a button below the view, the view, a button above it.
    assert_eq!(
        menu(&mut state, &nodes, [130, 130]),
        [
            (1005, "Cancel".to_owned()),
            (9, "Under".to_owned()),
            (WORLD, "world".to_owned()),
            (9, "Over".to_owned()),
        ]
    );
    // Inside the view only.
    assert_eq!(
        menu(&mut state, &nodes, [300, 300]),
        [(1005, "Cancel".to_owned()), (WORLD, "world".to_owned())]
    );
    // Outside the view's rectangle there is no world entry at all.
    assert_eq!(
        menu(&mut state, &nodes, [600, 400]),
        [(1005, "Cancel".to_owned())]
    );
    assert!(viewport::find(&state, &nodes).is_some());
}

#[test]
fn only_a_layer_that_blocks_clicks_resets_the_menu() {
    let mut state = state();
    let (root, view, _, over) = frame(&mut state);
    let (panel, plain) = (Key::component(ROOT | 4), Key::component(ROOT | 5));
    state.components.insert(
        panel,
        Component {
            no_click_through: true,
            ..layer(ROOT | 4)
        },
    );
    // The same flag on something that is not a layer does nothing.
    state.components.insert(
        plain,
        Component {
            no_click_through: true,
            kind: 3,
            ..layer(ROOT | 5)
        },
    );
    let nodes = [
        node(root, [0, 0, 765, 503], 5),
        node(view, [4, 4, 512, 334], 2),
        node(plain, [0, 0, 60, 60], 3),
        node(panel, [100, 100, 200, 200], 5),
        node(over, [120, 120, 50, 50], 5),
    ];
    assert_eq!(
        menu(&mut state, &nodes, [30, 30]),
        [(1005, "Cancel".to_owned()), (WORLD, "world".to_owned())]
    );
    // The panel is drawn after the view: the world goes, its own child stays.
    assert_eq!(
        menu(&mut state, &nodes, [130, 130]),
        [(1005, "Cancel".to_owned()), (9, "Over".to_owned())]
    );
    assert_eq!(
        menu(&mut state, &nodes, [250, 250]),
        [(1005, "Cancel".to_owned())]
    );
    // Drawn before the view, the same panel removes nothing of the world.
    let nodes = [
        node(root, [0, 0, 765, 503], 4),
        node(panel, [100, 100, 200, 200], 3),
        node(over, [120, 120, 50, 50], 3),
        node(view, [4, 4, 512, 334], 4),
    ];
    assert_eq!(
        menu(&mut state, &nodes, [130, 130]),
        [
            (1005, "Cancel".to_owned()),
            (9, "Over".to_owned()),
            (WORLD, "world".to_owned())
        ]
    );
}

#[test]
fn a_modal_sub_interface_resets_the_menu_after_its_host_layers_children() {
    let mut state = state();
    let (root, view, under, _) = frame(&mut state);
    let host = Key::component(ROOT | 6);
    state.components.insert(host, layer(ROOT | 6));
    let modal = Key::component(300 << 16);
    state.components.insert(modal, button(300 << 16, "Inside"));
    let nodes = [
        node(root, [0, 0, 765, 503], 5),
        node(view, [4, 4, 512, 334], 2),
        node(host, [100, 100, 200, 200], 5),
        node(under, [100, 100, 50, 50], 4),
        node(modal, [100, 100, 80, 80], 5),
    ];
    // An overlay (type 3) sub-interface leaves everything in place.
    state.subinterfaces.insert(ROOT | 6, (300, 3));
    assert_eq!(
        menu(&mut state, &nodes, [130, 130]),
        [
            (1005, "Cancel".to_owned()),
            (WORLD, "world".to_owned()),
            (9, "Under".to_owned()),
            (9, "Inside".to_owned()),
        ]
    );
    // A modal one (type 0) clears what its host's own children added, and
    // the world, before its own components add theirs.
    state.subinterfaces.insert(ROOT | 6, (300, 0));
    assert_eq!(
        menu(&mut state, &nodes, [130, 130]),
        [(1005, "Cancel".to_owned()), (9, "Inside".to_owned())]
    );
    // Hovering the host beside the modal's components still clears.
    assert_eq!(
        menu(&mut state, &nodes, [250, 250]),
        [(1005, "Cancel".to_owned())]
    );
    // Outside the host nothing is cleared.
    assert_eq!(
        menu(&mut state, &nodes, [400, 50]),
        [(1005, "Cancel".to_owned()), (WORLD, "world".to_owned())]
    );
}
