//! Original NPCRenderer membership, motion bits and mask dispatch. Concrete NPC
//! visibility, mask payloads, aggression and spatial enumeration are services.
use super::{doors::Location, player_render};
use crate::definitions::item::Shared;
pub use player_render::{Error, Result};
use rs09_protocol::game::Frame;
pub type Handle = Shared<Npc>;
pub struct Npc {
    pub index: i32,
    pub id: i32,
    pub location: Option<Location>,
    pub teleporting: bool,
    pub direction: i32,
    pub walk_dir: i32,
    pub run_dir: i32,
    pub masks_required: bool,
    pub aggressive: bool,
}
impl Npc {
    pub fn new(index: i32, id: i32, location: Location) -> Self {
        Self {
            index,
            id,
            location: Some(location),
            teleporting: false,
            direction: 0,
            walk_dir: -1,
            run_dir: -1,
            masks_required: false,
            aggressive: false,
        }
    }
}
#[derive(Default)]
pub struct List {
    nodes: Vec<Option<Handle>>,
    version: u32,
}
impl List {
    pub fn values(&self) -> &[Option<Handle>] {
        &self.nodes
    }
    pub fn len(&self) -> usize {
        self.nodes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
    pub fn push(&mut self, n: Option<Handle>) {
        self.nodes.push(n);
        self.version = self.version.wrapping_add(1);
    }
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.version = self.version.wrapping_add(1);
    }
    pub fn set(&mut self, i: usize, n: Option<Handle>) -> Result<Option<Handle>> {
        Ok(std::mem::replace(
            self.nodes.get_mut(i).ok_or(Error::Index)?,
            n,
        ))
    }
    pub fn remove(&mut self, i: usize) -> Result<Option<Handle>> {
        if i >= self.nodes.len() {
            return Err(Error::Index);
        }
        self.version = self.version.wrapping_add(1);
        Ok(self.nodes.remove(i))
    }
    pub fn contains(&self, n: &Option<Handle>) -> bool {
        self.nodes.iter().any(|v| same(v, n))
    }
    fn remove_all(&mut self, removed: &[Option<Handle>]) {
        let old = self.nodes.len();
        self.nodes.retain(|n| !removed.iter().any(|r| same(n, r)));
        self.version = self.version.wrapping_add((old - self.nodes.len()) as u32);
    }
}
fn same(a: &Option<Handle>, b: &Option<Handle>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.same(b),
        (None, None) => true,
        _ => false,
    }
}
pub trait Context {
    fn hidden(&mut self, local: &player_render::Handle, npc: &Handle) -> Result<bool>;
    fn candidates(&mut self, location: Location, distance: i32) -> Result<Vec<Option<Handle>>>;
    fn masks(
        &mut self,
        local: &player_render::Handle,
        npc: &Handle,
        out: &mut Vec<u8>,
        sync: bool,
    ) -> Result<()>;
    fn remove_tolerance(&mut self, npc: &Handle, player_index: i32) -> Result<()>;
    fn add_tolerance(&mut self, npc: &Handle, player_index: i32) -> Result<()>;
    fn send(&mut self, local: &player_render::Handle, frame: Frame) -> Result<()>;
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
fn player_location(local: &player_render::Handle) -> Result<Location> {
    local.read().location.clone().ok_or(Error::Null)
}
fn npc_location(npc: &Handle) -> Result<Location> {
    npc.read().location.clone().ok_or(Error::Null)
}
fn within(a: &Location, b: &Location) -> bool {
    let [a, bz, c] = a.coordinates();
    let [x, y, z] = b.coordinates();
    c == z && a.wrapping_sub(x).wrapping_abs() <= 15 && bz.wrapping_sub(y).wrapping_abs() <= 15
}
fn flag(
    ctx: &mut impl Context,
    local: &player_render::Handle,
    npc: &Handle,
    bits: &mut Bits,
    masks: &mut Vec<u8>,
    sync: bool,
) -> Result<()> {
    let required = npc.read().masks_required;
    bits.put(1, i32::from(required));
    if required {
        ctx.masks(local, npc, masks, sync)?;
    }
    Ok(())
}
pub fn render(ctx: &mut impl Context, local: &player_render::Handle) -> Result<()> {
    let info = local.read().info.clone();
    let list = info.read().local_npcs.clone();
    let mut bits = Bits::default();
    let mut masks = vec![];
    bits.put(8, list.read().len() as i32);
    let version = list.read().version;
    let mut cursor = 0;
    let mut removed = vec![];
    while cursor < list.read().len() {
        let npc = {
            let l = list.read();
            if l.version != version {
                return Err(Error::ConcurrentModification);
            }
            l.nodes
                .get(cursor)
                .cloned()
                .ok_or(Error::Index)?
                .ok_or(Error::Null)?
        };
        cursor += 1;
        let within = within(&player_location(local)?, &npc_location(&npc)?);
        if ctx.hidden(local, &npc)? || !within || npc.read().teleporting {
            bits.put(1, 1);
            bits.put(2, 3);
            removed.push(Some(npc.clone()));
            if !within && npc.read().aggressive {
                let index = local.read().index;
                ctx.remove_tolerance(&npc, index)?;
            }
        } else if npc.read().run_dir != -1 {
            bits.put(1, 1);
            bits.put(2, 2);
            bits.put(3, npc.read().walk_dir);
            bits.put(3, npc.read().run_dir);
            flag(ctx, local, &npc, &mut bits, &mut masks, false)?;
        } else if npc.read().walk_dir != -1 {
            bits.put(1, 1);
            bits.put(2, 1);
            bits.put(3, npc.read().walk_dir);
            flag(ctx, local, &npc, &mut bits, &mut masks, false)?;
        } else if npc.read().masks_required {
            bits.put(1, 1);
            bits.put(2, 0);
            ctx.masks(local, &npc, &mut masks, false)?;
        } else {
            bits.put(1, 0);
        }
    }
    list.write().remove_all(&removed);
    for npc in ctx.candidates(player_location(local)?, 15)? {
        if list.read().len() >= 255 {
            break;
        }
        if list.read().contains(&npc) {
            continue;
        }
        let npc = npc.ok_or(Error::Null)?;
        if ctx.hidden(local, &npc)? {
            continue;
        }
        bits.put(15, npc.read().index);
        bits.put(1, i32::from(npc.read().teleporting));
        bits.put(3, npc.read().direction);
        flag(ctx, local, &npc, &mut bits, &mut masks, true)?;
        let np = npc_location(&npc)?;
        let pl = player_location(local)?;
        let mut x = np.coordinates()[0].wrapping_sub(pl.coordinates()[0]);
        let mut y = np.coordinates()[1].wrapping_sub(pl.coordinates()[1]);
        if x < 0 {
            x = x.wrapping_add(32);
        }
        if y < 0 {
            y = y.wrapping_add(32);
        }
        bits.put(5, y);
        bits.put(14, npc.read().id);
        bits.put(5, x);
        if npc.read().aggressive {
            let index = local.read().index;
            ctx.add_tolerance(&npc, index)?;
        }
        list.write().push(Some(npc));
    }
    if !masks.is_empty() {
        bits.put(15, 32767);
    }
    bits.data.extend(masks);
    ctx.send(
        local,
        Frame {
            opcode: 32,
            payload: bits.data,
        },
    )
}
