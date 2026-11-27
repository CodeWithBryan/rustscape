//! A frame's layout walked after a cycle discarded components: the server
//! logging the player out (`InterfaceList.reset()` on the way back to the
//! title) and a sub-interface closed with its group unloaded.
use super::*;
use crate::{
    interaction::{Menu, MenuEntry},
    ui::Component,
};

const ROOT: i32 = 548;
const SUB: i32 = 149;
const TITLE: i32 = 378;

fn state() -> State {
    let manifest =
        serde_json::to_vec(&serde_json::json!({"source":"stale layout test","files":{}})).unwrap();
    let mut bytes = b"RS09VIS1".to_vec();
    bytes.extend((manifest.len() as u32).to_le_bytes());
    bytes.extend(manifest);
    State::new(Pack::decode(bytes).unwrap())
}

fn add(state: &mut State, id: i32, overlayer: i32, kind: i32, rect: [i32; 4], op: bool) {
    state.replace_component(
        Key::component(id),
        Component {
            id,
            kind,
            overlayer,
            base_x: rect[0],
            base_y: rect[1],
            base_width: rect[2],
            base_height: rect[3],
            button_type: i32::from(op),
            option: if op { "Use".into() } else { String::new() },
            ..Default::default()
        },
    );
}

/// A game root with a button and a host layer that holds an open
/// sub-interface with a button of its own.
fn in_game() -> State {
    let mut state = state();
    state.root = ROOT;
    state.loaded.insert(ROOT);
    add(&mut state, ROOT << 16, -1, 0, [0, 0, 765, 503], false);
    add(
        &mut state,
        ROOT << 16 | 1,
        ROOT << 16,
        5,
        [10, 10, 40, 40],
        true,
    );
    add(
        &mut state,
        ROOT << 16 | 2,
        ROOT << 16,
        0,
        [100, 100, 200, 200],
        false,
    );
    state.loaded.insert(SUB);
    add(&mut state, SUB << 16, -1, 0, [0, 0, 200, 200], false);
    add(
        &mut state,
        SUB << 16 | 1,
        SUB << 16,
        5,
        [20, 20, 40, 40],
        true,
    );
    state.subinterfaces.insert(ROOT << 16 | 2, (SUB, 1));
    state
}

fn menu(state: &mut State, nodes: &[DrawNode], point: [i32; 2]) -> Vec<String> {
    let mut menu = Menu::default();
    menu.add(MenuEntry {
        option: "Cancel".into(),
        target: String::new(),
        action: 1005,
        key: 0,
        args: [0; 2],
        cursor: -1,
    });
    state.menu_at(nodes, point, &mut menu);
    menu.entries.into_iter().map(|e| e.option).collect()
}

fn shape(nodes: &[DrawNode]) -> Vec<(Key, [i32; 4], usize)> {
    nodes
        .iter()
        .map(|n| {
            (
                n.key,
                [n.rect.x, n.rect.y, n.rect.w, n.rect.h],
                n.subtree_end,
            )
        })
        .collect()
}

/// The panic of `rs09-client` when the server logged it out: the frame's
/// layout was taken before the cycle that read the logout, and the menu was
/// built over it after every game component had gone.
#[test]
fn a_logout_in_the_frame_leaves_no_node_of_the_game_interfaces() {
    let mut state = in_game();
    let mut nodes = state.layout(765, 503).unwrap();
    assert_eq!(nodes.len(), 5);
    assert!(menu(&mut state, &nodes, [130, 130]).contains(&"Use".to_string()));
    // The cycle: the logout resets the interfaces and opens the title.
    state.close_everything(|_| {});
    state.root = TITLE;
    state.loaded.insert(TITLE);
    add(&mut state, TITLE << 16, -1, 0, [0, 0, 765, 503], false);
    // The walks after the cycles, over that frame's layout.
    state.retain_current(&mut nodes);
    for point in [[20, 20], [130, 130], [400, 400]] {
        assert_eq!(menu(&mut state, &nodes, point), ["Cancel"]);
    }
    assert!(state.inventory_slot_at(&nodes, [130, 130]).is_none());
    assert!(nodes.is_empty());
    // The next frame lays the title out.
    assert_eq!(state.layout(765, 503).unwrap().len(), 1);
}

/// A sub-interface closed and unloaded in a cycle: the rest of the frame's
/// layout stays, as the fresh layout of the remaining tree would have it.
#[test]
fn a_closed_sub_interface_leaves_the_rest_of_the_layout() {
    let mut state = in_game();
    let mut nodes = state.layout(765, 503).unwrap();
    state.close_sub(ROOT << 16 | 2, true);
    state.retain_current(&mut nodes);
    assert_eq!(menu(&mut state, &nodes, [130, 130]), ["Cancel"]);
    assert!(menu(&mut state, &nodes, [20, 20]).contains(&"Use".to_string()));
    assert_eq!(shape(&nodes), shape(&state.layout(765, 503).unwrap()));
    assert_eq!(nodes[0].subtree_end, nodes.len());
}

/// Nothing discarded: the list is left as it was laid out.
#[test]
fn a_current_layout_is_unchanged() {
    let mut state = in_game();
    let mut nodes = state.layout(765, 503).unwrap();
    let before = shape(&nodes);
    state.retain_current(&mut nodes);
    assert_eq!(shape(&nodes), before);
}
