//! RT4 PLAYER_INFO (225) membership, movement queues and complete mask fields.
//! Animation priorities, chat decoding/filtering, sounds and model construction
//! are client bindings; this module does not synthesize entity simulation.
use super::Frame;
use crate::{Error, Result, buffer::Reader};

pub const SELF: usize = 2047;
pub const CAPACITY: usize = 2048;
const ANGLES: [i32; 8] = [768, 1024, 1280, 512, 1536, 256, 0, 1792];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawAppearance {
    pub flags: i32,
    pub skull_icon: i32,
    pub prayer_icon: i32,
    /// Original two-byte equipment encodings; item indices are not item IDs.
    pub slots: [i32; 12],
    pub npc_id: i32,
    pub npc_team: i32,
    pub colors: [i32; 5],
    pub bas: i32,
    pub name37: i64,
    pub combat: i32,
    pub skill: i32,
    pub combat_with_summoning: i32,
    pub combat_range: i32,
    pub sound_radius: i32,
    /// idle, crawl, walk, run. Retaining previous actor sounds when radius is
    /// zero belongs to the consumer; those words are absent from that packet.
    pub sounds: Option<[i32; 4]>,
    pub consumed: usize,
}
pub trait AppearanceLookup {
    fn equipment_id(&self, index: usize) -> Result<i32>;
    fn item_team(&self, id: i32) -> Result<i32>;
    fn body_color_count(&self, index: usize) -> usize;
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Appearance {
    pub identikit: [i32; 12],
    pub colors: [i32; 5],
    pub team: i32,
    pub raw: RawAppearance,
}
/// A character-design change from the appearance interface's scripts
/// (CS2 403, 404 and 410), applied to the local player's appearance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DesignEdit {
    /// `feature` is an identity-kit feature code, as IdkType stores it.
    Kit {
        feature: i32,
        kit: i32,
    },
    Colour {
        index: i32,
        colour: i32,
    },
    Female(bool),
}
/// `PlayerAppearance.MALE_FEATURES`, `FEMALE_FEATURES` and `BASE_PART_MAP`:
/// a feature's position in either list selects the body slot it dresses.
const MALE_FEATURES: [i32; 8] = [0, 1, 2, 3, 4, 5, 6, 14];
const FEMALE_FEATURES: [i32; 8] = [7, 8, 9, 10, 11, 12, 13, 15];
const BASE_PART_MAP: [usize; 8] = [8, 11, 4, 6, 9, 7, 10, 0];

impl RawAppearance {
    /// `kit_exists` is `IdkTypeList.get(kit) != null`. A kit is only placed in
    /// a slot that is already occupied, and replaces whatever was there.
    pub fn design(&mut self, edit: DesignEdit, kit_exists: impl Fn(i32) -> bool) -> Result<()> {
        match edit {
            DesignEdit::Kit { feature, kit } => {
                let Some(position) = MALE_FEATURES
                    .iter()
                    .position(|&f| f == feature)
                    .or_else(|| FEMALE_FEATURES.iter().position(|&f| f == feature))
                else {
                    return Ok(());
                };
                let part = BASE_PART_MAP[position];
                if self.slots[part] != 0 && kit_exists(kit) {
                    // The wire form of an identity kit is its ID plus 256.
                    self.slots[part] = kit.wrapping_add(256);
                }
            }
            DesignEdit::Colour { index, colour } => {
                let slot = usize::try_from(index)
                    .ok()
                    .and_then(|index| self.colors.get_mut(index))
                    .ok_or(Error::new(
                        0,
                        "body colour index outside the original array",
                    ))?;
                *slot = colour;
            }
            DesignEdit::Female(female) => {
                self.flags = (self.flags & !1) | i32::from(female);
            }
        }
        Ok(())
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let mut r = Reader::new(bytes);
        let flags = r.g1()?;
        let skull_icon = r.g1b()?;
        let prayer_icon = r.g1b()?;
        let mut slots = [0; 12];
        let mut npc_id = -1;
        let mut npc_team = 0;
        for (i, slot) in slots.iter_mut().enumerate() {
            let hi = r.g1()?;
            if hi != 0 {
                *slot = (hi << 8) + r.g1()?;
                if i == 0 && *slot == 65535 {
                    npc_id = r.g2()?;
                    npc_team = r.g1()?;
                    break;
                }
            }
        }
        let mut colors = [0; 5];
        for color in &mut colors {
            *color = r.g1()?;
        }
        let bas = r.g2()?;
        let name37 = r.g8()?;
        let combat = r.g1()?;
        let (skill, combat_with_summoning, combat_range) = if flags & 4 != 0 {
            (r.g2()?, combat, -1)
        } else {
            let sum = r.g1()?;
            let range = r.g1()?;
            (0, sum, if range == 255 { -1 } else { range })
        };
        let sound_radius = r.g1()?;
        let sounds = if sound_radius == 0 {
            None
        } else {
            Some([r.g2()?, r.g2()?, r.g2()?, r.g2()?])
        };
        Ok(Self {
            flags,
            skull_icon,
            prayer_icon,
            slots,
            npc_id,
            npc_team,
            colors,
            bas,
            name37,
            combat,
            skill,
            combat_with_summoning,
            combat_range,
            sound_radius,
            sounds,
            consumed: r.position(),
        })
    }
    pub fn female(&self) -> bool {
        self.flags & 1 != 0
    }
    pub fn size(&self) -> i32 {
        (self.flags >> 3 & 7) + 1
    }
    pub fn title(&self) -> i32 {
        self.flags >> 6 & 3
    }
    pub fn resolve(&self, lookup: &impl AppearanceLookup) -> Result<Appearance> {
        let mut identikit = [0; 12];
        let mut team = self.npc_team;
        if self.npc_id == -1 {
            for (i, &value) in self.slots.iter().enumerate() {
                if value == 0 {
                    continue;
                }
                identikit[i] = if value >= 32768 {
                    let id = lookup.equipment_id((value - 32768) as usize)?;
                    let item_team = lookup.item_team(id)?;
                    if item_team != 0 {
                        team = item_team;
                    }
                    id | 0x40000000
                } else {
                    value.wrapping_sub(256) | i32::MIN
                };
            }
        }
        let mut colors = self.colors;
        for (i, color) in colors.iter_mut().enumerate() {
            if *color < 0 || *color as usize >= lookup.body_color_count(i) {
                *color = 0;
            }
        }
        Ok(Appearance {
            identikit,
            colors,
            team,
            raw: self.clone(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Player {
    pub queue_x: [i32; 10],
    pub queue_y: [i32; 10],
    pub queue_speed: [i32; 10],
    pub queue_size: usize,
    pub last_seen: i32,
    pub current_angle: i32,
    pub target_angle: i32,
    pub appearance: Option<RawAppearance>,
}
impl Player {
    pub fn new(x: i32, y: i32) -> Self {
        let mut p = Self {
            queue_x: [0; 10],
            queue_y: [0; 10],
            queue_speed: [0; 10],
            queue_size: 0,
            last_seen: 0,
            current_angle: 0,
            target_angle: 0,
            appearance: None,
        };
        p.queue_x[0] = x;
        p.queue_y[0] = y;
        p
    }
    pub fn tile(&self) -> [i32; 2] {
        [self.queue_x[0], self.queue_y[0]]
    }
    fn push(&mut self, x: i32, y: i32, speed: i32) {
        self.queue_size = (self.queue_size + 1).min(9);
        for i in (1..=self.queue_size).rev() {
            self.queue_x[i] = self.queue_x[i - 1];
            self.queue_y[i] = self.queue_y[i - 1];
            self.queue_speed[i] = self.queue_speed[i - 1];
        }
        self.queue_x[0] = x;
        self.queue_y[0] = y;
        self.queue_speed[0] = speed;
    }
    fn step(&mut self, speed: i32, direction: usize) {
        const DELTA: [[i32; 2]; 8] = [
            [-1, 1],
            [0, 1],
            [1, 1],
            [-1, 0],
            [1, 0],
            [-1, -1],
            [0, -1],
            [1, -1],
        ];
        let [dx, dy] = DELTA[direction];
        self.push(
            self.queue_x[0].wrapping_add(dx),
            self.queue_y[0].wrapping_add(dy),
            speed,
        );
    }
    /// Queue part of Player.teleport, also used by normal scene rebuilds.
    pub fn teleport(&mut self, x: i32, y: i32, discard: bool) {
        let dx = x.wrapping_sub(self.queue_x[0]);
        let dy = y.wrapping_sub(self.queue_y[0]);
        if !discard && (-8..=8).contains(&dx) && (-8..=8).contains(&dy) {
            self.push(x, y, 1);
        } else {
            self.queue_size = 0;
            self.queue_x[0] = x;
            self.queue_y[0] = y;
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Movement {
    Step {
        index: usize,
        speed: i32,
        direction: usize,
    },
    Teleport {
        index: usize,
        tile: [i32; 2],
        discard: bool,
    },
    Added {
        index: usize,
        is_new: bool,
        angle: i32,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Field {
    PublicChat {
        color_effect: i32,
        rights: i32,
        payload: Vec<u8>,
    },
    Hit {
        damage: i32,
        kind: i32,
        bar: Option<i32>,
    },
    Animation {
        id: i32,
        delay: i32,
    },
    Appearance {
        bytes: Vec<u8>,
        appearance: RawAppearance,
    },
    FaceEntity(i32),
    ForcedMovement {
        start: [i32; 2],
        destination: [i32; 2],
        start_cycle: i32,
        end_cycle: i32,
        direction: i32,
    },
    OverheadText(Vec<u8>),
    SlotAnimations(Vec<SlotAnimation>),
    SpotAnimation {
        id: i32,
        packed: i32,
    },
    FaceTile([i32; 2]),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlotAnimation {
    pub id: i32,
    pub delay: i32,
    pub mask: i32,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Extended {
    pub index: usize,
    pub flags: i32,
    pub fields: Vec<Field>,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Update {
    pub movements: Vec<Movement>,
    pub extended: Vec<Extended>,
    pub removed_candidates: Vec<usize>,
    pub removed: Vec<usize>,
}
pub struct State {
    pub players: Vec<Option<Player>>,
    pub local_ids: Vec<usize>,
    pub appearance_cache: Vec<Option<Vec<u8>>>,
    pub plane: i32,
}
impl State {
    pub fn new(local_x: i32, local_y: i32) -> Self {
        let mut players = vec![None; CAPACITY];
        players[SELF] = Some(Player::new(local_x, local_y));
        Self {
            players,
            local_ids: vec![],
            appearance_cache: vec![None; CAPACITY],
            plane: 0,
        }
    }
    pub fn self_player(&self) -> &Player {
        self.players[SELF].as_ref().expect("self player invariant")
    }
    pub fn self_player_mut(&mut self) -> &mut Player {
        self.players[SELF].as_mut().expect("self player invariant")
    }
    pub fn rebase(&mut self, delta: [i32; 2]) {
        for player in self.players.iter_mut().flatten() {
            for x in &mut player.queue_x {
                *x = x.wrapping_sub(delta[0]);
            }
            for y in &mut player.queue_y {
                *y = y.wrapping_sub(delta[1]);
            }
        }
    }
    pub fn decode_frame(&mut self, frame: &Frame, cycle: i32) -> Result<Update> {
        if frame.opcode != 225 {
            return Err(Error::new(0, "not PLAYER_INFO"));
        }
        self.decode(&frame.payload, cycle)
    }
    pub fn decode(&mut self, payload: &[u8], cycle: i32) -> Result<Update> {
        let mut r = Reader::new(payload);
        let mut bits = r.bits();
        let mut out = Update::default();
        let mut extended = Vec::new();
        if bits.read(1)? != 0 {
            match bits.read(2)? {
                0 => extended.push(SELF),
                1 => {
                    let direction = bits.read(3)? as usize;
                    self.move_player(SELF, 1, direction, &mut out)?;
                    if bits.read(1)? != 0 {
                        extended.push(SELF);
                    }
                }
                2 => {
                    if bits.read(1)? != 0 {
                        for _ in 0..2 {
                            let direction = bits.read(3)? as usize;
                            self.move_player(SELF, 2, direction, &mut out)?;
                        }
                    } else {
                        let direction = bits.read(3)? as usize;
                        self.move_player(SELF, 0, direction, &mut out)?;
                    }
                    if bits.read(1)? != 0 {
                        extended.push(SELF);
                    }
                }
                _ => {
                    let y = bits.read(7)? as i32;
                    let discard = bits.read(1)? != 0;
                    self.plane = bits.read(2)? as i32;
                    if bits.read(1)? != 0 {
                        extended.push(SELF);
                    }
                    let x = bits.read(7)? as i32;
                    self.teleport_player(SELF, [x, y], discard, &mut out)?;
                }
            }
        }
        let count = bits.read(8)? as usize;
        if count > self.local_ids.len() {
            return Err(Error::new(0, "gppov1"));
        }
        out.removed_candidates
            .extend_from_slice(&self.local_ids[count..]);
        let previous = std::mem::take(&mut self.local_ids);
        for &index in &previous[..count] {
            if self.players.get(index).and_then(Option::as_ref).is_none() {
                return Err(Error::new(0, "missing local player"));
            }
            if bits.read(1)? == 0 {
                self.keep(index, cycle);
                continue;
            }
            match bits.read(2)? {
                0 => {
                    self.keep(index, cycle);
                    extended.push(index);
                }
                1 => {
                    self.keep(index, cycle);
                    let direction = bits.read(3)? as usize;
                    self.move_player(index, 1, direction, &mut out)?;
                    if bits.read(1)? != 0 {
                        extended.push(index);
                    }
                }
                2 => {
                    self.keep(index, cycle);
                    if bits.read(1)? != 0 {
                        for _ in 0..2 {
                            let direction = bits.read(3)? as usize;
                            self.move_player(index, 2, direction, &mut out)?;
                        }
                    } else {
                        let direction = bits.read(3)? as usize;
                        self.move_player(index, 0, direction, &mut out)?;
                    }
                    if bits.read(1)? != 0 {
                        extended.push(index);
                    }
                }
                _ => out.removed_candidates.push(index),
            }
        }
        while bits.remaining() >= 11 {
            let index = bits.read(11)? as usize;
            if index == SELF {
                break;
            }
            let is_new = self.players[index].is_none();
            if is_new {
                let mut player = Player::new(0, 0);
                if let Some(bytes) = &self.appearance_cache[index] {
                    player.appearance = Some(RawAppearance::decode(bytes)?);
                }
                self.players[index] = Some(player);
            }
            if self.local_ids.len() == CAPACITY {
                return Err(Error::new(0, "player list capacity exceeded"));
            }
            self.keep(index, cycle);
            if bits.read(1)? != 0 {
                extended.push(index);
            }
            let mut first_offset = bits.read(5)? as i32;
            let angle = ANGLES[bits.read(3)? as usize];
            if first_offset > 15 {
                first_offset -= 32;
            }
            if is_new {
                let p = self.players[index].as_mut().unwrap();
                p.current_angle = angle;
                p.target_angle = angle;
            }
            let discard = bits.read(1)? != 0;
            let mut second_offset = bits.read(5)? as i32;
            if second_offset > 15 {
                second_offset -= 32;
            }
            let [x, y] = self.self_player().tile();
            out.movements.push(Movement::Added {
                index,
                is_new,
                angle,
            });
            // The source names the first offset dy but adds it to queue X.
            self.teleport_player(
                index,
                [x.wrapping_add(first_offset), y.wrapping_add(second_offset)],
                discard,
                &mut out,
            )?;
        }
        bits.finish();
        for index in extended {
            let mut flags = r.g1()?;
            if flags & 0x10 != 0 {
                flags += r.g1()? << 8;
            }
            let fields = decode_fields(&mut r, flags, cycle)?;
            for field in &fields {
                if let Field::ForcedMovement { .. } = field {
                    self.players[index]
                        .as_mut()
                        .ok_or(Error::new(r.position(), "missing extended player"))?
                        .queue_size = 1;
                }
                if let Field::Appearance { bytes, appearance } = field {
                    self.appearance_cache[index] = Some(bytes.clone());
                    self.players[index]
                        .as_mut()
                        .ok_or(Error::new(r.position(), "missing extended player"))?
                        .appearance = Some(appearance.clone());
                }
            }
            out.extended.push(Extended {
                index,
                flags,
                fields,
            });
        }
        for &index in &out.removed_candidates {
            if self.players[index]
                .as_ref()
                .is_some_and(|p| p.last_seen != cycle)
            {
                self.players[index] = None;
                out.removed.push(index);
            }
        }
        if r.remaining() != 0 {
            return Err(Error::new(
                r.position(),
                "gpp1: player payload not fully consumed",
            ));
        }
        if self
            .local_ids
            .iter()
            .any(|&index| self.players[index].is_none())
        {
            return Err(Error::new(r.position(), "gpp2: missing retained player"));
        }
        Ok(out)
    }
    fn keep(&mut self, index: usize, cycle: i32) {
        self.local_ids.push(index);
        self.players[index].as_mut().unwrap().last_seen = cycle;
    }
    fn move_player(
        &mut self,
        index: usize,
        speed: i32,
        direction: usize,
        out: &mut Update,
    ) -> Result<()> {
        self.players
            .get_mut(index)
            .and_then(Option::as_mut)
            .ok_or(Error::new(0, "missing moving player"))?
            .step(speed, direction);
        out.movements.push(Movement::Step {
            index,
            speed,
            direction,
        });
        Ok(())
    }
    fn teleport_player(
        &mut self,
        index: usize,
        tile: [i32; 2],
        discard: bool,
        out: &mut Update,
    ) -> Result<()> {
        self.players
            .get_mut(index)
            .and_then(Option::as_mut)
            .ok_or(Error::new(0, "missing teleport player"))?
            .teleport(tile[0], tile[1], discard);
        out.movements.push(Movement::Teleport {
            index,
            tile,
            discard,
        });
        Ok(())
    }
}
fn sentinel(value: i32) -> i32 {
    if value == 65535 { -1 } else { value }
}
pub fn decode_fields(r: &mut Reader<'_>, flags: i32, cycle: i32) -> Result<Vec<Field>> {
    let mut fields = Vec::new();
    if flags & 0x80 != 0 {
        let color_effect = r.ig2()?;
        let rights = r.g1()?;
        let len = r.g1()? as usize;
        let payload = r.take(len)?.iter().rev().copied().collect();
        fields.push(Field::PublicChat {
            color_effect,
            rights,
            payload,
        });
    }
    if flags & 1 != 0 {
        let damage = r.gsmarts()?;
        let kind = r.g1add()?;
        let bar = Some(r.g1sub()?);
        fields.push(Field::Hit { damage, kind, bar });
    }
    if flags & 8 != 0 {
        let id = sentinel(r.g2()?);
        let delay = r.g1()?;
        fields.push(Field::Animation { id, delay });
    }
    if flags & 4 != 0 {
        let len = r.g1add()? as usize;
        let bytes = r.take(len)?.to_vec();
        let appearance = RawAppearance::decode(&bytes)?;
        fields.push(Field::Appearance { bytes, appearance });
    }
    if flags & 2 != 0 {
        fields.push(Field::FaceEntity(sentinel(r.g2add()?)));
    }
    if flags & 0x400 != 0 {
        let start = [r.g1neg()?, r.g1()?];
        let destination = [r.g1add()?, r.g1()?];
        let start_cycle = r.ig2()?.wrapping_add(cycle);
        let end_cycle = r.ig2()?.wrapping_add(cycle);
        let direction = r.g1neg()?;
        fields.push(Field::ForcedMovement {
            start,
            destination,
            start_cycle,
            end_cycle,
            direction,
        });
    }
    if flags & 0x20 != 0 {
        fields.push(Field::OverheadText(r.gjstr()?.to_vec()));
    }
    if flags & 0x200 != 0 {
        let damage = r.gsmarts()?;
        let kind = r.g1sub()?;
        fields.push(Field::Hit {
            damage,
            kind,
            bar: None,
        });
    }
    if flags & 0x800 != 0 {
        let n = r.g1neg()?;
        let mut animations = Vec::new();
        for _ in 0..n {
            animations.push(SlotAnimation {
                id: sentinel(r.ig2()?),
                delay: r.g1add()?,
                mask: r.g2()?,
            });
        }
        fields.push(Field::SlotAnimations(animations));
    }
    if flags & 0x100 != 0 {
        let id = sentinel(r.ig2()?);
        let packed = r.mg4()?;
        fields.push(Field::SpotAnimation { id, packed });
    }
    if flags & 0x40 != 0 {
        fields.push(Field::FaceTile([r.g2()?, r.ig2add()?]));
    }
    Ok(fields)
}

#[cfg(test)]
mod design_tests {
    use super::*;

    fn appearance() -> RawAppearance {
        let mut slots = [0; 12];
        // Hair (slot 8), torso (4), arms (6), hands (9), legs (7), feet (10).
        for (slot, kit) in [(8, 0), (4, 18), (6, 26), (9, 33), (7, 36), (10, 42)] {
            slots[slot] = kit + 256;
        }
        RawAppearance {
            flags: 0,
            skull_icon: -1,
            prayer_icon: -1,
            slots,
            npc_id: -1,
            npc_team: 0,
            colors: [0; 5],
            bas: 1426,
            name37: 1,
            combat: 3,
            skill: 0,
            combat_with_summoning: 3,
            combat_range: -1,
            sound_radius: 0,
            sounds: None,
            consumed: 0,
        }
    }

    #[test]
    fn kits_dress_the_slot_their_feature_maps_to() {
        let mut a = appearance();
        // Male feature 2 (torso) and female feature 9 share position 2: slot 4.
        a.design(
            DesignEdit::Kit {
                feature: 2,
                kit: 20,
            },
            |_| true,
        )
        .unwrap();
        assert_eq!(a.slots[4], 276);
        a.design(
            DesignEdit::Kit {
                feature: 9,
                kit: 56,
            },
            |_| true,
        )
        .unwrap();
        assert_eq!(a.slots[4], 312);
        // Features 14 and 15 are the last position, slot 0.
        a.slots[0] = 32768 + 5;
        a.design(
            DesignEdit::Kit {
                feature: 14,
                kit: 7,
            },
            |_| true,
        )
        .unwrap();
        assert_eq!(a.slots[0], 263, "the kit replaces the equipment encoding");
        // An empty slot is left empty (jaw, slot 11, for this appearance).
        a.design(
            DesignEdit::Kit {
                feature: 1,
                kit: 10,
            },
            |_| true,
        )
        .unwrap();
        assert_eq!(a.slots[11], 0);
        // An unknown kit or an unknown feature changes nothing.
        a.design(
            DesignEdit::Kit {
                feature: 0,
                kit: 999,
            },
            |kit| kit < 100,
        )
        .unwrap();
        a.design(
            DesignEdit::Kit {
                feature: 99,
                kit: 1,
            },
            |_| true,
        )
        .unwrap();
        assert_eq!(a.slots[8], 256);
    }

    #[test]
    fn colours_and_gender() {
        let mut a = appearance();
        a.design(
            DesignEdit::Colour {
                index: 4,
                colour: 7,
            },
            |_| true,
        )
        .unwrap();
        assert_eq!(a.colors, [0, 0, 0, 0, 7]);
        for index in [-1, 5] {
            assert!(
                a.design(DesignEdit::Colour { index, colour: 1 }, |_| true)
                    .is_err()
            );
        }
        a.flags = 0b1010;
        a.design(DesignEdit::Female(true), |_| true).unwrap();
        assert!(a.female() && a.flags == 0b1011);
        a.design(DesignEdit::Female(false), |_| true).unwrap();
        assert!(!a.female() && a.flags == 0b1010, "other flag bits are kept");
    }
}
