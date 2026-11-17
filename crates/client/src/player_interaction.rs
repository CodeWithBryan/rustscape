//! Original revision 530 player options, labels and action payloads.
//! Scene picking and authoritative trade/combat/follow outcomes are external.
use crate::{
    interaction::{Menu, MenuEntry},
    ui::{actions::SelectedItem, targeting::Targeting},
};
use rs09_protocol::{
    buffer::{Reader, Writer},
    game::{Frame, players::RawAppearance},
};

pub const ACTIONS: [i32; 8] = [30, 6, 31, 29, 10, 44, 37, 57];

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Options {
    pub labels: [Option<String>; 8],
    pub cursors: [i32; 8],
    pub secondary: [bool; 8],
    pub prefixes: Option<Vec<Option<String>>>,
    pub suffixes: Option<Vec<Option<String>>>,
}
impl Options {
    pub fn apply(&mut self, frame: &Frame) -> rs09_protocol::Result<bool> {
        if frame.opcode != 44 {
            return Ok(false);
        }
        let mut r = Reader::new(&frame.payload);
        let cursor = r.ig2add()?;
        let top = r.g1()?;
        let index = r.g1()?;
        let label = crate::ui::latin1(r.gjstr()?);
        if (1..=8).contains(&index) {
            let i = index as usize - 1;
            self.labels[i] = (!label.eq_ignore_ascii_case("null")).then_some(label);
            self.cursors[i] = if cursor == 65535 { -1 } else { cursor };
            self.secondary[i] = top == 0;
        }
        Ok(true)
    }
    /// CS2 5422 allocates each array only on a nonempty assignment. An empty
    /// string leaves the old entry in place; later failure retains the prefix.
    pub fn titles(
        &mut self,
        game: i32,
        index: i32,
        prefix: Option<String>,
        suffix: Option<String>,
    ) -> Result<(), String> {
        for (values, value) in [(&mut self.prefixes, prefix), (&mut self.suffixes, suffix)] {
            let value = value.ok_or("Null player title")?;
            if value.is_empty() {
                continue;
            }
            if values.is_none() {
                let count = match game {
                    0 => 1,
                    1 => 4,
                    _ => return Err("Player title game index".into()),
                };
                *values = Some(vec![None; count]);
            }
            let slot = values
                .as_mut()
                .unwrap()
                .get_mut(index as usize)
                .ok_or("Player title index")?;
            *slot = Some(value);
        }
        Ok(())
    }
    pub fn name(&self, raw: &RawAppearance) -> Result<String, String> {
        let mut name = username(raw.name37).ok_or("Invalid player base37 name")?;
        if let Some(prefixes) = &self.prefixes {
            let prefix = prefixes
                .get(raw.title() as usize)
                .ok_or("Player title index")?;
            name = format!("{}{name}", prefix.as_deref().unwrap_or("null"));
        }
        if let Some(suffixes) = &self.suffixes {
            let suffix = suffixes
                .get(raw.title() as usize)
                .ok_or("Player title index")?;
            name.push_str(suffix.as_deref().unwrap_or("null"));
        }
        Ok(name)
    }
}

/// Base37.decode37().toTitleCase(), including underscores following digits.
pub fn username(value: i64) -> Option<String> {
    if value <= 0 || value >= 6_582_952_005_840_035_281 || value % 37 == 0 {
        return None;
    }
    let mut capitalize = true;
    Some(
        rs09_protocol::login::decode_name(value)
            .chars()
            .map(|c| {
                if c == '_' {
                    capitalize = true;
                    ' '
                } else {
                    let result = if capitalize {
                        c.to_ascii_uppercase()
                    } else {
                        c
                    };
                    capitalize = false;
                    result
                }
            })
            .collect(),
    )
}

#[derive(Clone, Debug)]
pub struct Player<'a> {
    pub name: &'a str,
    pub combat: i32,
    pub summoning: i32,
    pub range: i32,
    pub skill: i32,
    pub team: i32,
}
pub struct Context<'a> {
    pub game: i32,
    pub local: Player<'a>,
    pub selected: Option<&'a SelectedItem>,
    pub targeting: &'a Targeting,
    pub use_cursor: i32,
}
pub fn display_name(game: i32, local: &Player<'_>, other: &Player<'_>) -> String {
    if other.skill != 0 {
        return format!("{} (skill: {})", other.name, other.skill);
    }
    let mut within = true;
    if local.range != -1 && other.range != -1 {
        let calc = (other.combat.max(local.combat).wrapping_mul(10) / 100)
            .wrapping_add(other.range.min(local.range))
            .wrapping_add(5);
        let mut delta = local.combat.wrapping_sub(other.combat);
        if delta < 0 {
            delta = delta.wrapping_neg();
        }
        within = calc >= delta;
    }
    let color = if within {
        crate::npc_interaction::level_color(other.combat, local.combat)
    } else {
        0xffffff
    };
    let label = if game == 1 { "rating" } else { "level" };
    let sum = if other.summoning > other.combat {
        format!("+{}", other.summoning.wrapping_sub(other.combat))
    } else {
        String::new()
    };
    format!(
        "{}<col={color:06x}> ({label}: {}{sum})",
        other.name, other.combat
    )
}
pub fn entries(
    menu: &mut Menu,
    options: &Options,
    context: Context<'_>,
    other: &Player<'_>,
    index: i32,
    tile: [i32; 2],
    is_self: bool,
) {
    if is_self || menu.entries.len() >= 400 {
        return;
    }
    let name = display_name(context.game, &context.local, other);
    let mut add = |option: String, target: String, action: i32, cursor: i32| {
        menu.add(MenuEntry {
            option,
            target,
            action,
            key: index as u64,
            args: tile,
            cursor,
        });
    };
    if let Some(selected) = context.selected {
        add(
            "Use".into(),
            format!("{} -> <col=ffffff>{name}", selected.text),
            1,
            context.use_cursor,
        );
    } else if !context.targeting.active {
        for i in (0..8).rev() {
            let Some(option) = &options.labels[i] else {
                continue;
            };
            let mut priority = 0;
            if context.game == 0 && option.eq_ignore_ascii_case("Attack") {
                if other.combat > context.local.combat {
                    priority = 2000;
                }
                if context.local.team != 0 && other.team != 0 {
                    priority = if context.local.team == other.team {
                        2000
                    } else {
                        0
                    };
                }
            } else if options.secondary[i] {
                priority = 2000;
            }
            add(
                option.clone(),
                format!("<col=ffffff>{name}"),
                ACTIONS[i] + priority,
                options.cursors[i],
            );
        }
    } else if context.targeting.mask & 8 != 0 {
        // Player targets use only bit8; unlike item/NPC/location targets they
        // never consult targetParamId or ParamType defaults.
        add(
            context.targeting.verb.clone(),
            format!("{} -> <col=ffffff>{name}", context.targeting.text),
            15,
            context.targeting.cursor,
        );
    }
    if let Some(walk) = menu.entries.iter_mut().find(|e| e.action == 60) {
        walk.target = format!("<col=ffffff>{name}");
    }
}
pub fn packet(action: i32, index: i32) -> Option<Frame> {
    let mut w = Writer::new();
    let opcode = match if action >= 2000 {
        action - 2000
    } else {
        action
    } {
        30 => {
            w.ip2add(index);
            68
        }
        6 => {
            w.p2(index);
            106
        }
        31 => {
            w.ip2add(index);
            71
        }
        29 => {
            w.ip2add(index);
            180
        }
        10 => {
            w.ip2(index);
            4
        }
        44 => {
            w.ip2(index);
            133
        }
        37 => {
            w.ip2add(index);
            114
        }
        57 => {
            w.p2add(index);
            175
        }
        _ => return None,
    };
    Some(Frame {
        opcode,
        payload: w.into_vec(),
    })
}

/// CS2 3107 (`ClientProt.clickPlayerOption`): the packet for a scripted click
/// on player option 1, 4, 5, 6 or 7. Other options send nothing. The original
/// also walks toward the player; this sends the option alone.
pub fn script_option(option: i32, index: i32) -> Option<Frame> {
    let mut w = rs09_protocol::buffer::Writer::new();
    let opcode = match option {
        1 => {
            w.ip2add(index);
            68
        }
        4 => {
            w.ip2add(index);
            180
        }
        5 => {
            w.ip2(index);
            4
        }
        6 => {
            w.ip2(index);
            133
        }
        7 => {
            w.ip2add(index);
            114
        }
        _ => return None,
    };
    Some(Frame {
        opcode,
        payload: w.into_vec(),
    })
}

#[cfg(test)]
mod script_option_tests {
    use super::*;
    use rs09_protocol::game::{Direction, Size};

    #[test]
    fn scripted_player_options_use_the_original_packets() {
        for (option, opcode, payload) in [
            (1, 68, [0x34 + 128, 0x12]),
            (4, 180, [0x34 + 128, 0x12]),
            (5, 4, [0x34, 0x12]),
            (6, 133, [0x34, 0x12]),
            (7, 114, [0x34 + 128, 0x12]),
        ] {
            let frame = script_option(option, 0x1234).unwrap();
            assert_eq!(
                (frame.opcode, frame.payload.as_slice()),
                (opcode, &payload[..])
            );
            assert_eq!(
                Direction::ClientToServer.size(opcode),
                Some(Size::Fixed(2)),
                "{opcode}"
            );
        }
        for option in [0, 2, 3, 8] {
            assert!(script_option(option, 1).is_none());
        }
    }
}
