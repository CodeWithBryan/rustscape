//! Original PlayerRenderer ordering over shared native player/render state.
//! Mask encoders and spatial candidate enumeration belong to the owning world.
use super::doors::Location;
use crate::definitions::item::Shared;
use rs09_protocol::game::Frame;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Null,
    Index,
    ConcurrentModification,
    Binding(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
pub type Handle = Shared<Player>;
#[derive(Default)]
pub struct PlayerList {
    values: Vec<Option<Handle>>,
    version: i32,
}
impl PlayerList {
    pub fn values(&self) -> &[Option<Handle>] {
        &self.values
    }
    pub fn len(&self) -> usize {
        self.values.len()
    }
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
    pub fn push(&mut self, p: Option<Handle>) {
        self.values.push(p);
        self.version = self.version.wrapping_add(1);
    }
    pub fn clear(&mut self) {
        self.values.clear();
        self.version = self.version.wrapping_add(1);
    }
    pub fn remove(&mut self, i: usize) -> Result<Option<Handle>> {
        if i >= self.values.len() {
            return Err(Error::Index);
        }
        self.version = self.version.wrapping_add(1);
        Ok(self.values.remove(i))
    }
    pub fn contains(&self, h: &Handle) -> bool {
        self.values
            .iter()
            .any(|v| v.as_ref().is_some_and(|p| p.same(h)))
    }
}
pub struct RenderInfo {
    pub local_players: Shared<PlayerList>,
    pub local_npcs: Shared<super::npc_render::List>,
    pub appearance_stamps: Shared<Vec<i64>>,
    pub mask_updates: Shared<Vec<Option<usize>>>,
    pub mask_update_count: i32,
    pub last_location: Option<Location>,
    pub on_first_cycle: bool,
    pub prepared_appearance: bool,
}
impl RenderInfo {
    pub fn new(max_players: usize) -> Self {
        Self {
            local_players: Shared::new(PlayerList::default()),
            local_npcs: Shared::new(super::npc_render::List::default()),
            appearance_stamps: Shared::new(vec![0; max_players]),
            mask_updates: Shared::new(vec![None; 256]),
            mask_update_count: 0,
            last_location: None,
            on_first_cycle: true,
            prepared_appearance: false,
        }
    }
    pub fn update_information(&mut self, location: Option<Location>) {
        self.on_first_cycle = false;
        self.last_location = location;
        self.prepared_appearance = false;
    }
    pub fn register_mask_update(&mut self, entity: Option<usize>) -> Result<()> {
        let i = self.mask_update_count;
        self.mask_update_count = self.mask_update_count.wrapping_add(1);
        *self
            .mask_updates
            .write()
            .get_mut(i as usize)
            .ok_or(Error::Index)? = entity;
        Ok(())
    }
}
#[derive(Clone, Copy, Default)]
pub struct MaskState {
    pub required: bool,
    pub synced: bool,
    pub appearance_stamp: i64,
}
pub struct Player {
    pub index: i32,
    pub location: Option<Location>,
    pub last_scene: Option<Location>,
    pub update_scene: bool,
    pub teleporting: bool,
    pub active: bool,
    pub invisible: bool,
    pub walk_dir: i32,
    pub run_dir: i32,
    pub direction: i32,
    pub masks: MaskState,
    pub info: Shared<RenderInfo>,
}
impl Player {
    pub fn new(index: i32, location: Location, max_players: usize) -> Self {
        Self {
            index,
            location: Some(location),
            last_scene: None,
            update_scene: false,
            teleporting: false,
            active: true,
            invisible: false,
            walk_dir: -1,
            run_dir: -1,
            direction: 0,
            masks: MaskState::default(),
            info: Shared::new(RenderInfo::new(max_players)),
        }
    }
}
pub trait Context {
    fn candidates(&mut self, location: Location, distance: i32) -> Result<Vec<Option<Handle>>>;
    fn masks(
        &mut self,
        local: &Handle,
        other: &Handle,
        output: &mut Vec<u8>,
        appearance: bool,
        sync: bool,
    ) -> Result<()>;
    fn send(&mut self, local: &Handle, frame: Frame) -> Result<()>;
}
#[derive(Default)]
struct Bits {
    data: Vec<u8>,
    position: usize,
}
impl Bits {
    fn put(&mut self, n: usize, value: i32) {
        for shift in (0..n).rev() {
            let byte = self.position / 8;
            if byte == self.data.len() {
                self.data.push(0);
            }
            self.data[byte] |= (((value as u32 >> shift) & 1) as u8) << (7 - self.position % 8);
            self.position += 1;
        }
    }
}
fn location(h: &Handle) -> Result<Location> {
    h.read().location.clone().ok_or(Error::Null)
}
fn mask_update(
    ctx: &mut impl Context,
    local: &Handle,
    other: &Handle,
    flags: &mut Vec<u8>,
    appearance: bool,
    sync: bool,
) -> Result<()> {
    if sync || other.read().masks.required {
        ctx.masks(local, other, flags, appearance, sync)?;
    }
    Ok(())
}
fn flag(
    ctx: &mut impl Context,
    local: &Handle,
    other: &Handle,
    bits: &mut Bits,
    flags: &mut Vec<u8>,
) -> Result<()> {
    if other.read().masks.required {
        bits.put(1, 1);
        mask_update(ctx, local, other, flags, false, false)?;
    } else {
        bits.put(1, 0);
    }
    Ok(())
}
fn render_local(
    ctx: &mut impl Context,
    local: &Handle,
    other: &Handle,
    bits: &mut Bits,
    flags: &mut Vec<u8>,
) -> Result<()> {
    if other.read().run_dir != -1 {
        bits.put(1, 1);
        bits.put(2, 2);
        bits.put(1, 1);
        bits.put(3, other.read().walk_dir);
        bits.put(3, other.read().run_dir);
        flag(ctx, local, other, bits, flags)?;
    } else if other.read().walk_dir != -1 {
        bits.put(1, 1);
        bits.put(2, 1);
        bits.put(3, other.read().walk_dir);
        flag(ctx, local, other, bits, flags)?;
    } else if other.read().masks.required {
        bits.put(1, 1);
        bits.put(2, 0);
        mask_update(ctx, local, other, flags, false, false)?;
    } else {
        bits.put(1, 0);
    }
    Ok(())
}
fn render_self(
    ctx: &mut impl Context,
    local: &Handle,
    bits: &mut Bits,
    flags: &mut Vec<u8>,
) -> Result<()> {
    if local.read().update_scene || local.read().teleporting {
        bits.put(1, 1);
        bits.put(2, 3);
        let [_, y, _] = location(local)?.coordinates();
        let [_, last_y, _] = local
            .read()
            .last_scene
            .clone()
            .ok_or(Error::Null)?
            .coordinates();
        bits.put(
            7,
            y.wrapping_sub(((last_y >> 3).wrapping_sub(6)).wrapping_shl(3)),
        );
        bits.put(1, i32::from(local.read().teleporting));
        bits.put(2, location(local)?.coordinates()[2]);
        flag(ctx, local, local, bits, flags)?;
        let [x, _, _] = location(local)?.coordinates();
        let [last_x, _, _] = local
            .read()
            .last_scene
            .clone()
            .ok_or(Error::Null)?
            .coordinates();
        bits.put(
            7,
            x.wrapping_sub(((last_x >> 3).wrapping_sub(6)).wrapping_shl(3)),
        );
        Ok(())
    } else {
        render_local(ctx, local, local, bits, flags)
    }
}
fn within(a: &Location, b: &Location) -> bool {
    let [x, y, z] = a.coordinates();
    let [xx, yy, zz] = b.coordinates();
    if z != zz {
        return false;
    }
    let (dx, dy) = (x.wrapping_sub(xx), y.wrapping_sub(yy));
    f64::from(dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy))).sqrt() <= 15.0
}
fn add_local(
    ctx: &mut impl Context,
    local: &Handle,
    other: &Handle,
    info: &Shared<RenderInfo>,
    bits: &mut Bits,
    flags: &mut Vec<u8>,
) -> Result<()> {
    bits.put(11, other.read().index);
    let [x, _, _] = location(other)?.coordinates();
    let [xx, _, _] = location(local)?.coordinates();
    let mut dx = x.wrapping_sub(xx);
    let [_, y, _] = location(other)?.coordinates();
    let [_, yy, _] = location(local)?.coordinates();
    let mut dy = y.wrapping_sub(yy);
    if dy < 0 {
        dy = dy.wrapping_add(32);
    }
    if dx < 0 {
        dx = dx.wrapping_add(32);
    }
    let index = other.read().index as usize;
    let stamps = info.read().appearance_stamps.clone();
    let old = *stamps.read().get(index).ok_or(Error::Index)?;
    let appearance = old != other.read().masks.appearance_stamp;
    let update = appearance || other.read().masks.required || other.read().masks.synced;
    bits.put(1, i32::from(update));
    bits.put(5, dx);
    bits.put(3, other.read().direction);
    bits.put(1, i32::from(other.read().teleporting));
    bits.put(5, dy);
    info.read().local_players.write().push(Some(other.clone()));
    if update {
        if appearance {
            let stamps = info.read().appearance_stamps.clone();
            let index = other.read().index as usize;
            let stamp = other.read().masks.appearance_stamp;
            *stamps.write().get_mut(index).ok_or(Error::Index)? = stamp;
        }
        mask_update(ctx, local, other, flags, appearance, true)?;
    }
    Ok(())
}
pub fn render(ctx: &mut impl Context, local: &Handle) -> Result<()> {
    let mut bits = Bits::default();
    let mut flags = vec![];
    let info = local.read().info.clone();
    render_self(ctx, local, &mut bits, &mut flags)?;
    bits.put(8, info.read().local_players.read().len() as i32);
    let list = info.read().local_players.clone();
    let mut expected = list.read().version;
    let mut cursor = 0;
    while cursor < list.read().len() {
        let other = {
            let l = list.read();
            if expected != l.version {
                return Err(Error::ConcurrentModification);
            }
            l.values
                .get(cursor)
                .ok_or(Error::Index)?
                .clone()
                .ok_or(Error::Null)?
        };
        cursor += 1;
        if !other.read().active
            || !within(&location(&other)?, &location(local)?)
            || other.read().teleporting
            || other.read().invisible
        {
            bits.put(1, 1);
            bits.put(2, 3);
            let mut l = list.write();
            if expected != l.version {
                return Err(Error::ConcurrentModification);
            }
            l.remove(cursor - 1)?;
            cursor -= 1;
            expected = l.version;
        } else {
            render_local(ctx, local, &other, &mut bits, &mut flags)?;
        }
    }
    let mut count = 0i32;
    for other in ctx.candidates(location(local)?, 15)? {
        let other = other.ok_or(Error::Null)?;
        if other.same(local)
            || !other.read().active
            || info.read().local_players.read().contains(&other)
            || other.read().invisible
        {
            continue;
        }
        if info.read().local_players.read().len() >= 255 {
            break;
        }
        count = count.wrapping_add(1);
        if count == 10 {
            break;
        }
        add_local(ctx, local, &other, &info, &mut bits, &mut flags)?;
    }
    if !flags.is_empty() {
        bits.put(11, 2047);
    }
    bits.data.extend(flags);
    ctx.send(
        local,
        Frame {
            opcode: 225,
            payload: bits.data,
        },
    )
}
