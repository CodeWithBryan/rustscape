//! Cache-defined location menus and RT4 single-tile interaction approach rules.
//! The displayed-camera pick ray is a presentation adapter; actions retain RT4 codes.
use crate::walking::{self, ClippingDefinition, Route};
use rs09_game_core::collision::CollisionMap;
use rs09_protocol::{Error, Result, buffer::Reader};

#[derive(Clone, Debug)]
pub struct LocationType {
    pub name: String,
    pub options: [Option<String>; 5],
    pub size: [i32; 2],
    pub blocked_sides: i32,
    pub interactable: i32,
    /// Opcode 96. `SceneGraph.addLoc` ORs `Integer.MIN_VALUE` into the scene
    /// key of such a location, which makes the key negative: it is never picked.
    pub has_animation: bool,
    pub support_items: i32,
    pub cursors: [(i32, i32); 2],
    pub map_function: i32,
    pub map_scene: i32,
    pub map_scene_rotated: bool,
    pub map_scene_angle: i32,
    pub morph_varbit: i32,
    pub morph_varp: i32,
    pub children: Option<Vec<i32>>,
    pub ambient: crate::ambient::Definition,
    pub params: std::collections::BTreeMap<i32, crate::ui::HookArg>,
    /// The fields `SceneGraph.addLoc` and `LocType.getStaticEntity` place by,
    /// among them opcode 82 (`hidden_in_hd`) and opcode 98 (`dynamic`).
    pub placement: crate::loc_placement::Rules,
    /// LocType.computeVertexColors (opcode 22): static models share their
    /// lighting with mergeable neighbours.
    pub mergeable: bool,
    /// LocType.active (cleared by opcode 64): walls and centrepieces of the
    /// type shade the floor.
    pub active: bool,
    /// LocType.occlude (opcode 23): walls of the type hide what is behind
    /// a long enough run of them (`crate::occlusion`).
    pub occlude: bool,
}
impl LocationType {
    pub fn has_area_sound(&self, mut child: impl FnMut(i32) -> bool) -> bool {
        self.children.as_ref().map_or_else(
            || self.ambient.audible(),
            |children| children.iter().any(|&id| id != -1 && child(id)),
        )
    }
    pub fn select_morph(&self, mut read: impl FnMut(bool, i32) -> i32) -> Option<u32> {
        let children = self.children.as_ref()?;
        let value = if self.morph_varbit != -1 {
            read(true, self.morph_varbit)
        } else if self.morph_varp != -1 {
            read(false, self.morph_varp)
        } else {
            -1
        };
        let selected = if value < 0
            || value as usize >= children.len() - 1
            || children[value as usize] == -1
        {
            *children.last()?
        } else {
            children[value as usize]
        };
        (selected != -1).then_some(selected as u32)
    }
    pub fn decode(data: &[u8]) -> Result<Self> {
        Self::decode_with_visual(data).map(|(interaction, _, _)| interaction)
    }

    pub fn decode_with_visual(
        data: &[u8],
    ) -> Result<(Self, crate::location::Definition, ClippingDefinition)> {
        let clipping = ClippingDefinition::decode(data)?;
        let mut visual = crate::location::Definition {
            width: clipping.size[0],
            length: clipping.size[1],
            ..Default::default()
        };
        let mut r = Reader::new(data);
        let mut d = Self {
            name: "null".into(),
            options: std::array::from_fn(|_| None),
            size: clipping.size,
            blocked_sides: 0,
            interactable: -1,
            has_animation: false,
            support_items: -1,
            cursors: [(-1, -1); 2],
            map_function: -1,
            map_scene: -1,
            map_scene_rotated: false,
            map_scene_angle: 0,
            morph_varbit: -1,
            morph_varp: -1,
            children: None,
            ambient: crate::ambient::Definition::default(),
            params: std::collections::BTreeMap::new(),
            placement: crate::loc_placement::Rules::default(),
            mergeable: false,
            active: true,
            occlude: false,
        };
        let mut first_shape = None;
        let mut has_models = false;
        // LocType.postDecode runs before LocTypeList's break-route override.
        let mut support_walk = 2;
        loop {
            match r.g1()? {
                0 => {
                    if d.support_items == -1 {
                        d.support_items = i32::from(support_walk != 0);
                    }
                    if clipping.break_route {
                        d.placement.block_walk = 0;
                    }
                    if d.interactable == -1 {
                        d.interactable = i32::from(
                            (has_models && first_shape.is_none_or(|s| s == 10))
                                || d.options.iter().any(Option::is_some),
                        );
                    }
                    return Ok((d, visual, clipping));
                }
                1 => {
                    let n = r.g1()?;
                    let use_models = !has_models && n > 0;
                    if use_models {
                        visual.shapes = Some(Vec::with_capacity(n as usize));
                    }
                    for i in 0..n {
                        let id = r.g2()?;
                        let shape = r.g1()?;
                        if use_models {
                            if i == 0 {
                                first_shape = Some(shape);
                            }
                            visual.models.push(id as u32);
                            visual.shapes.as_mut().unwrap().push(shape);
                        }
                    }
                    if n > 0 {
                        has_models = true;
                    }
                }
                5 => {
                    let n = r.g1()? as usize;
                    if n > 0 && !has_models {
                        has_models = true;
                        first_shape = None;
                        visual.models = (0..n)
                            .map(|_| r.g2().map(|id| id as u32))
                            .collect::<Result<_>>()?;
                    } else {
                        r.take(n * 2)?;
                    }
                }
                2 => d.name = String::from_utf8_lossy(r.gjstr()?).into_owned(),
                19 => d.interactable = r.g1()?,
                17 => {
                    support_walk = 0;
                    d.placement.opcode(17, 0);
                }
                27 => {
                    support_walk = 1;
                    d.placement.opcode(27, 0);
                }
                75 => d.support_items = r.g1()?,
                op @ 30..=34 => {
                    let value = String::from_utf8_lossy(r.gjstr()?).into_owned();
                    d.options[(op - 30) as usize] =
                        (!value.eq_ignore_ascii_case("hidden")).then_some(value);
                }
                69 => d.blocked_sides = r.g1()?,
                60 => d.map_function = r.g2()?,
                97 => d.map_scene_rotated = true,
                101 => d.map_scene_angle = r.g1()?,
                102 => d.map_scene = r.g2()?,
                op @ (99 | 100) => d.cursors[(op - 99) as usize] = (r.g1()?, r.g2()?),
                op @ (40 | 41) => {
                    let n = r.g1()? as usize;
                    let pairs = (0..n)
                        .map(|_| Ok([r.g2()? as u16, r.g2()? as u16]))
                        .collect::<Result<_>>()?;
                    if op == 40 {
                        visual.recolor = pairs;
                    } else {
                        visual.retexture = pairs;
                    }
                }
                42 => {
                    let n = r.g1()? as usize;
                    r.take(n)?;
                }
                op @ (77 | 92) => {
                    let nullable = |n| if n == 65535 { -1 } else { n };
                    d.morph_varbit = nullable(r.g2()?);
                    d.morph_varp = nullable(r.g2()?);
                    let fallback = if op == 92 { nullable(r.g2()?) } else { -1 };
                    let n = r.g1()? as usize;
                    let mut children = Vec::with_capacity(n + 2);
                    for _ in 0..=n {
                        children.push(nullable(r.g2()?));
                    }
                    children.push(fallback);
                    d.children = Some(children);
                }
                78 => {
                    d.ambient.sound = r.g2()?;
                    d.ambient.radius = r.g1()? * 128;
                }
                79 => {
                    d.ambient.intervals = [r.g2()?, r.g2()?];
                    d.ambient.radius = r.g1()? * 128;
                    let n = r.g1()? as usize;
                    d.ambient.sounds = Some((0..n).map(|_| r.g2()).collect::<Result<_>>()?);
                }
                249 => d.params.extend(crate::ui::targeting::read_params(&mut r)?),
                21 => {
                    visual.hill = true;
                    d.placement.opcode(21, 0);
                }
                24 => visual.animation = r.g2()?,
                29 => visual.ambient = r.g1b()?,
                39 => visual.contrast = r.g1b()?,
                62 => visual.mirror = true,
                op @ 65..=67 => visual.resize[(op - 65) as usize] = r.g2()?,
                op @ 70..=72 => visual.offset[(op - 70) as usize] = r.g2b()?,
                88 => visual.cast_shadow = false,
                89 => visual.randomized_animation = false,
                14 | 15 => {
                    r.g1()?;
                }
                op @ (28 | 81) => {
                    let value = r.g1()?;
                    d.placement.opcode(op, value);
                }
                93 => {
                    let value = r.g2()?;
                    d.placement.opcode(93, value);
                }
                op @ (73 | 82 | 90 | 94 | 95 | 98) => {
                    d.placement.opcode(op, 0);
                }
                96 => d.has_animation = true,
                22 => d.mergeable = true,
                64 => d.active = false,
                23 => d.occlude = true,
                18 | 74 | 91 => {}
                _ => {
                    return Err(Error::new(
                        r.position() - 1,
                        "unsupported location interaction opcode",
                    ));
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocationTarget {
    pub id: u32,
    pub tile: [i32; 2],
    pub plane: usize,
    pub shape: u8,
    pub rotation: u8,
    pub size: [i32; 2],
    pub blocked_sides: i32,
}
impl LocationTarget {
    pub fn key(&self, origin: [i32; 2]) -> u64 {
        (u64::from(self.id) << 32)
            | 0x40000000
            | (self.tile[0] - origin[0]) as u64
            | (((self.tile[1] - origin[1]) as u64) << 7)
            | (u64::from(self.shape) << 14)
            | (u64::from(self.rotation) << 20)
    }
    /// PathFinder.findPathToLoc rotates footprint and side-access flags only for
    /// centrepieces/ground decoration; walls use shape+1 and their original angle.
    pub fn approach(&self, origin: [i32; 2]) -> Approach {
        let rectangle = matches!(self.shape, 10 | 11 | 22);
        let mut size = if rectangle { self.size } else { [0; 2] };
        if self.rotation & 1 != 0 {
            size.swap(0, 1);
        }
        let angle = i32::from(self.rotation);
        let sides = if angle == 0 {
            self.blocked_sides
        } else {
            (self.blocked_sides << angle & 15) + (self.blocked_sides >> (4 - angle))
        };
        Approach {
            tile: [self.tile[0] - origin[0], self.tile[1] - origin[1]],
            size,
            shape: if rectangle {
                0
            } else {
                i32::from(self.shape) + 1
            },
            angle,
            blocked_sides: if rectangle { sides } else { 0 },
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Approach {
    pub tile: [i32; 2],
    pub size: [i32; 2],
    pub shape: i32,
    pub angle: i32,
    pub blocked_sides: i32,
}
impl Approach {
    pub fn reached(&self, map: &CollisionMap, p: [i32; 2]) -> bool {
        if p == self.tile {
            return true;
        }
        if self.shape != 0
            && (((self.shape < 5 || self.shape == 10)
                && at_wall(map, p, self.tile, self.shape - 1, self.angle))
                || (self.shape < 10
                    && at_decoration(map, p, self.tile, self.shape - 1, self.angle)))
        {
            return true;
        }
        if self.size[0] == 0 || self.size[1] == 0 {
            return false;
        }
        let [x, y] = p;
        let [tx, ty] = self.tile;
        let [w, h] = self.size;
        let clear = |mask| map.get(x, y).is_some_and(|f| f & mask == 0);
        (x >= tx && x < tx + w && y >= ty && y < ty + h)
            || (x == tx - 1 && y >= ty && y < ty + h && clear(8) && self.blocked_sides & 8 == 0)
            || (x == tx + w && y >= ty && y < ty + h && clear(128) && self.blocked_sides & 2 == 0)
            || (y == ty - 1 && x >= tx && x < tx + w && clear(2) && self.blocked_sides & 4 == 0)
            || (y == ty + h && x >= tx && x < tx + w && clear(32) && self.blocked_sides & 1 == 0)
    }
    pub fn route(&self, map: &CollisionMap, source: [i32; 2]) -> Option<Route> {
        walking::find_path_to(map, source, self, true)
    }
    pub fn reached_sized(&self, map: &CollisionMap, p: [i32; 2], size: i32) -> bool {
        if size == 1 {
            return self.reached(map, p);
        }
        if !(2..=104).contains(&size) {
            return false;
        }
        if p == self.tile {
            return true;
        }
        if self.shape != 0
            && (((self.shape < 5 || self.shape == 10)
                && crate::route_reach::wall(map, p, self.tile, size, self.shape - 1, self.angle))
                || (self.shape < 10
                    && crate::route_reach::decoration(
                        map,
                        p,
                        self.tile,
                        size,
                        self.shape - 1,
                        self.angle,
                    )))
        {
            return true;
        }
        self.size[0] != 0
            && self.size[1] != 0
            && crate::route_reach::rectangle(map, p, size, self.tile, self.size, self.blocked_sides)
    }
    pub fn route_sized(&self, map: &CollisionMap, source: [i32; 2], size: i32) -> Option<Route> {
        walking::find_path_to_sized(map, source, self, true, size)
    }
}
fn at_wall(map: &CollisionMap, p: [i32; 2], t: [i32; 2], shape: i32, angle: i32) -> bool {
    let d = [p[0] - t[0], p[1] - t[1]];
    let clear = |mask| map.get(p[0], p[1]).is_some_and(|f| f & mask == 0);
    let west = d == [-1, 0];
    let north = d == [0, 1];
    let east = d == [1, 0];
    let south = d == [0, -1];
    match shape {
        0 => match angle {
            0 => west || (north && clear(0x12c0120)) || (south && clear(0x12c0102)),
            1 => north || (west && clear(0x12c0108)) || (east && clear(0x12c0180)),
            2 => east || (north && clear(0x12c0120)) || (south && clear(0x12c0102)),
            3 => south || (west && clear(0x12c0108)) || (east && clear(0x12c0180)),
            _ => false,
        },
        2 => match angle {
            0 => west || north || (east && clear(0x12c0180)) || (south && clear(0x12c0102)),
            1 => north || east || (west && clear(0x12c0108)) || (south && clear(0x12c0102)),
            2 => east || south || (west && clear(0x12c0108)) || (north && clear(0x12c0120)),
            3 => south || west || (east && clear(0x12c0180)) || (north && clear(0x12c0120)),
            _ => false,
        },
        9 => {
            (west && clear(8))
                || (north && clear(32))
                || (east && clear(128))
                || (south && clear(2))
        }
        _ => false,
    }
}
fn at_decoration(map: &CollisionMap, p: [i32; 2], t: [i32; 2], shape: i32, angle: i32) -> bool {
    let d = [p[0] - t[0], p[1] - t[1]];
    let side = match d {
        [-1, 0] => 0,
        [0, 1] => 1,
        [1, 0] => 2,
        [0, -1] => 3,
        _ => return false,
    };
    let angle = if shape == 7 { (angle + 2) & 3 } else { angle };
    let allowed = shape == 8
        || (matches!(shape, 6 | 7)
            && match angle {
                0 => side == 2 || side == 3,
                1 => side == 0 || side == 3,
                2 => side == 0 || side == 1,
                3 => side == 2 || side == 1,
                _ => false,
            });
    allowed
        && map
            .get(p[0], p[1])
            .is_some_and(|f| f & [8, 32, 128, 2][side] == 0)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuEntry {
    pub option: String,
    pub target: String,
    pub action: i32,
    pub key: u64,
    pub args: [i32; 2],
    pub cursor: i32,
}
#[derive(Default, Clone)]
pub struct Menu {
    pub entries: Vec<MenuEntry>,
    pub open: bool,
}
impl Menu {
    pub fn add(&mut self, entry: MenuEntry) {
        if !self.open && self.entries.len() < 500 {
            self.entries.push(entry);
        }
    }
    pub fn sort(&mut self) {
        loop {
            let mut changed = false;
            for i in 0..self.entries.len().saturating_sub(1) {
                let (a, b) = (self.entries[i].action, self.entries[i + 1].action);
                if (a < 1000 && b > 1000) || (a > 7000 && b > a) {
                    self.entries.swap(i, i + 1);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }
    pub fn location(&mut self, d: &LocationType, t: LocationTarget, origin: [i32; 2]) {
        let args = [t.tile[0] - origin[0], t.tile[1] - origin[1]];
        for i in (0..5).rev() {
            if let Some(option) = &d.options[i] {
                let mut cursor = -1;
                for &(op, id) in &d.cursors {
                    if op == i as i32 {
                        cursor = id;
                    }
                }
                self.add(MenuEntry {
                    option: option.clone(),
                    target: format!("<col=00ffff>{}", d.name),
                    action: [42, 50, 49, 46, 1001][i],
                    key: t.key(origin),
                    args,
                    cursor,
                });
            }
        }
        self.add(MenuEntry {
            option: "Examine".into(),
            target: format!("<col=00ffff>{}", d.name),
            action: 1004,
            key: u64::from(t.id),
            args,
            cursor: -1,
        });
    }
}
pub use rs09_protocol::game::actions::location_packet;
