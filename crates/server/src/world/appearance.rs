//! PlayerFlags530.Appearance's wire fields after body/equipment preparation.
//! Equipment, skull, familiar and combat systems supply their live values.
use rs09_protocol::buffer::Writer;

/// PlayerFlags530.Animate payload (mask 0x8, ordinal 2), before appearance.
pub fn animation(id: i32, delay: i32) -> [u8; 3] {
    let [high, low] = (id as u16).to_be_bytes();
    [high, low, delay as u8]
}
/// PlayerFlags530.SpotAnim (mask0x100, ordinal9). Java uses addition for the
/// packed word; out-of-range delay bits can carry into the height field.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SpotAnimation {
    pub id: i32,
    #[serde(default)]
    pub height: i32,
    #[serde(default)]
    pub delay: i32,
}
impl SpotAnimation {
    pub fn packed(self) -> i32 {
        self.height.wrapping_shl(16).wrapping_add(self.delay)
    }
    pub fn encode(self) -> [u8; 6] {
        let id = (self.id as u16).to_le_bytes();
        let word = self.packed().to_be_bytes();
        [id[0], id[1], word[1], word[0], word[3], word[2]]
    }
}
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Data {
    pub username: String,
    pub gender: i32,
    pub size: i32,
    pub skull_icon: i32,
    pub head_icon: i32,
    pub npc_id: i32,
    pub body_parts: [i32; 12],
    pub colors: [i32; 5],
    pub render_animation: i32,
    pub wilderness: bool,
    pub wilderness_disabled: bool,
    pub pvp: bool,
    pub wild_pvp: bool,
    pub using_summoning: bool,
    pub combat_level: i32,
    pub current_combat_level: i32,
    pub summoning_combat_level: i32,
    pub total_level: i32,
    pub wilderness_level: i32,
}
impl Default for Data {
    fn default() -> Self {
        Self {
            username: "self".into(),
            gender: 0,
            size: 1,
            skull_icon: -1,
            head_icon: -1,
            npc_id: -1,
            // Gender.MALE defaults, prepared with an empty equipment container.
            body_parts: [0, 0, 0, 0, 274, 0, 282, 292, 256, 289, 298, 266],
            colors: [0; 5],
            render_animation: 1426,
            wilderness: false,
            wilderness_disabled: false,
            pvp: false,
            wild_pvp: false,
            using_summoning: false,
            combat_level: 3,
            current_combat_level: 3,
            summoning_combat_level: 0,
            total_level: 33,
            wilderness_level: 0,
        }
    }
}
pub fn name_long(text: &str) -> i64 {
    let mut value = 0i64;
    for c in text.encode_utf16().take(12) {
        value = value.wrapping_mul(37);
        value = value.wrapping_add(match c {
            65..=90 => i64::from(c - 64),
            97..=122 => i64::from(c - 96),
            48..=57 => i64::from(c - 21),
            _ => 0,
        });
    }
    while value % 37 == 0 && value != 0 {
        value /= 37;
    }
    value
}
impl Data {
    pub fn naked(username: String, gender: i32) -> Self {
        let looks = if gender == 0 {
            [0, 10, 18, 26, 33, 36, 42]
        } else {
            [45, 1000, 56, 61, 68, 70, 79]
        };
        Self {
            username,
            gender: i32::from(gender != 0),
            body_parts: [
                0,
                0,
                0,
                0,
                looks[2] + 256,
                0,
                looks[3] + 256,
                looks[5] + 256,
                looks[0] + 256,
                looks[4] + 256,
                looks[6] + 256,
                looks[1] + 256,
            ],
            ..Self::default()
        }
    }
    /// Includes the original add-transformed appearance-length byte, not mask 4.
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::new();
        let non_pvp = self.wilderness && self.wilderness_disabled;
        let mut settings = self.gender;
        if self.size > 1 {
            settings = settings.wrapping_add(self.size.wrapping_sub(1).wrapping_shl(3));
        }
        if non_pvp {
            settings = settings.wrapping_add(4);
        }
        w.p1(settings);
        w.p1(self.skull_icon);
        w.p1(self.head_icon);
        if self.npc_id == -1 {
            for part in self.body_parts {
                if part == 0 {
                    w.p1(0);
                } else {
                    w.p2(part);
                }
            }
        } else {
            w.p2(-1);
            w.p2(self.npc_id);
            w.p1(255);
        }
        for color in self.colors {
            w.p1(color);
        }
        w.p2(self.render_animation);
        w.p8(name_long(&self.username));
        if non_pvp {
            w.p1(self.current_combat_level);
            w.p2(self.total_level);
        } else {
            if (self.pvp || (self.wild_pvp && self.wilderness)) && !self.using_summoning {
                w.p1(self.combat_level);
                w.p1(self.combat_level.wrapping_add(self.summoning_combat_level));
            } else {
                w.p1(self.current_combat_level);
                w.p1(self.current_combat_level);
            }
            w.p1(if self.pvp { self.wilderness_level } else { -1 });
        }
        w.p1(0);
        let mut field = vec![(w.as_slice().len() as u8).wrapping_add(128)];
        field.extend(w.into_vec());
        field
    }
}
