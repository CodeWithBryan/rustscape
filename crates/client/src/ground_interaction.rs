//! RT4 ground-stack menus and action bytes. Pile order comes from the shared
//! protocol reducer; presentation must not merge equal item IDs.
use crate::{
    interaction::{Approach, Menu, MenuEntry},
    ui::{actions::SelectedItem, item::ItemType},
    walking::{self, Route},
};
use rs09_game_core::collision::CollisionMap;
use rs09_protocol::{buffer::Writer, game::Frame};

/// Call in the pile's tail-to-head order, as MiniMenu.addEntries does.
pub fn entries(menu: &mut Menu, item: &ItemType, tile: [i32; 2], selected: Option<&SelectedItem>) {
    if let Some(selected) = selected {
        menu.add(MenuEntry {
            option: "Use".into(),
            target: format!("{} -> <col=ff9040>{}", selected.text, item.name),
            action: 33,
            key: item.id as u64,
            args: tile,
            cursor: -1,
        });
        return;
    }
    for i in (0..5).rev() {
        let Some(option) = item.ops.get(i).and_then(Option::as_ref) else {
            continue;
        };
        let mut cursor = -1;
        if item.cursor1Op == i as i32 {
            cursor = item.cursor1;
        }
        if item.cursor2Op == i as i32 {
            cursor = item.cursor2;
        }
        menu.add(MenuEntry {
            option: option.clone(),
            target: format!("<col=ff9040>{}", item.name),
            action: [21, 34, 18, 20, 24][i],
            key: item.id as u64,
            args: tile,
            cursor,
        });
    }
    menu.add(MenuEntry {
        option: "Examine".into(),
        target: format!("<col=ff9040>{}", item.name),
        action: 1002,
        key: item.id as u64,
        args: tile,
        cursor: -1,
    });
}

/// Original ground actions first try the exact tile, then its 1x1 access sides.
/// Neither attempt accepts the nearest-tile approximation.
pub fn route(map: &CollisionMap, source: [i32; 2], tile: [i32; 2]) -> Option<Route> {
    route_sized(map, source, tile, 1)
}
pub fn route_sized(
    map: &CollisionMap,
    source: [i32; 2],
    tile: [i32; 2],
    size: i32,
) -> Option<Route> {
    walking::find_path_sized(map, source, tile, false, size).or_else(|| {
        walking::find_path_to_sized(
            map,
            source,
            &Approach {
                tile,
                size: [1; 2],
                shape: 0,
                angle: 0,
                blocked_sides: 0,
            },
            false,
            size,
        )
    })
}

/// ISAAC/framing is applied by the connected transport. Menu action 34 is the
/// item's second ground option and sends opcode 109 (MiniMenu.java:821-838);
/// its deobfuscated name `PLAYER_ACTION_BLOCK` is a mislabel.
pub fn packet(
    action: i32,
    id: i32,
    tile: [i32; 2],
    selected: Option<&SelectedItem>,
) -> Option<Frame> {
    let action = if action >= 2000 {
        action - 2000
    } else {
        action
    };
    let [x, y] = tile;
    let mut w = Writer::new();
    let opcode = match action {
        21 => {
            w.p2(id);
            w.ip2(x);
            w.ip2add(y);
            228
        }
        34 => return Some(rs09_protocol::game::outgoing::ground_item_option2(x, y, id)),
        18 => {
            w.ip2(x);
            w.p2(id);
            w.ip2add(y);
            66
        }
        20 => {
            w.p2(id);
            w.p2(x);
            w.ip2(y);
            33
        }
        24 => {
            w.p2add(x);
            w.ip2add(id);
            w.ip2(y);
            48
        }
        1002 => {
            w.ip2add(id);
            92
        }
        33 => {
            let selected = selected?;
            w.ip2add(x);
            w.ip2(selected.slot);
            w.ip2(selected.id);
            w.ip2(id);
            w.ip2add(y);
            w.mp4(selected.component);
            101
        }
        _ => return None,
    };
    Some(Frame {
        opcode,
        payload: w.into_vec(),
    })
}
