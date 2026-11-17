//! Unmorphed revision 530 NPC menu priorities and original action bytes.
use crate::{
    interaction::{Menu, MenuEntry},
    npc::Definition,
    ui::actions::SelectedItem,
};
use rs09_protocol::{buffer::Writer, game::Frame};
pub fn level_color(other: i32, local: i32) -> u32 {
    match local.wrapping_sub(other) {
        n if n < -9 => 0xff0000,
        n if n < -6 => 0xff3000,
        n if n < -3 => 0xff7000,
        n if n < 0 => 0xffb000,
        n if n > 9 => 0x00ff00,
        n if n > 6 => 0x40ff00,
        n if n <= 3 => {
            if n > 0 {
                0xc0ff00
            } else {
                0xffff00
            }
        }
        _ => 0x80ff00,
    }
}
pub fn display_name(d: &Definition, local_level: i32) -> String {
    if d.combat_level != 0 {
        format!(
            "{}<col={:06x}> (level: {})",
            d.name,
            level_color(d.combat_level, local_level),
            d.combat_level
        )
    } else {
        d.name.clone()
    }
}
pub fn entries(
    menu: &mut Menu,
    d: &Definition,
    index: i32,
    local_level: i32,
    selected: Option<&SelectedItem>,
) {
    if menu.entries.len() >= 400 || !d.interactive {
        return;
    }
    let name = display_name(d, local_level);
    if let Some(selected) = selected {
        menu.add(MenuEntry {
            option: "Use".into(),
            target: format!("{} -> <col=ffff00>{name}", selected.text),
            action: 26,
            key: index as u64,
            args: [index, 0],
            cursor: -1,
        });
        return;
    }
    for attack in [false, true] {
        for i in (0..5).rev() {
            let Some(option) = &d.options[i] else {
                continue;
            };
            if option.eq_ignore_ascii_case("Attack") != attack {
                continue;
            }
            let action = [17, 16, 4, 19, 2][i]
                + if attack && d.combat_level > local_level {
                    2000
                } else {
                    0
                };
            let mut cursor = if attack { d.attack_cursor } else { -1 };
            if !attack {
                if d.cursors[0] == i as i32 {
                    cursor = d.cursors[1];
                }
                if d.cursors[2] == i as i32 {
                    cursor = d.cursors[3];
                }
            }
            menu.add(MenuEntry {
                option: option.clone(),
                target: format!("<col=ffff00>{name}"),
                action,
                key: index as u64,
                args: [index, 0],
                cursor,
            });
        }
    }
    menu.add(MenuEntry {
        option: "Examine".into(),
        target: format!("<col=ffff00>{name}"),
        action: 1007,
        key: index as u64,
        args: [index, 0],
        cursor: -1,
    });
}
pub fn packet(action: i32, index: i32, definition: i32) -> Option<Frame> {
    let mut w = Writer::new();
    let action = if action >= 2000 {
        action - 2000
    } else {
        action
    };
    let opcode = match action {
        17 => {
            w.ip2(index);
            78
        }
        16 => {
            w.ip2add(index);
            3
        }
        4 => {
            w.p2add(index);
            148
        }
        19 => {
            w.p2(index);
            30
        }
        2 => {
            w.ip2(index);
            218
        }
        1007 => {
            w.p2(definition);
            72
        }
        _ => return None,
    };
    Some(Frame {
        opcode,
        payload: w.into_vec(),
    })
}
