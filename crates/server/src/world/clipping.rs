//! Server ChunkFlags. These are distinct from RT4's client collision-map defaults.
use crate::definitions::item::Shared;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Null,
    Index,
    Limit,
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;
pub type Flags = Shared<[i32; 64]>;
pub type Landscape = Shared<Vec<Option<Shared<Vec<bool>>>>>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Border {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub projectile: bool,
    pub mask: i32,
    pub add: bool,
}
pub trait Context {
    fn flags(&mut self, chunk_id: i32, projectile: bool) -> Result<Flags>;
    fn border(&mut self, change: Border) -> Result<()>;
}
#[derive(Clone)]
pub struct ChunkFlags {
    pub base_x: i32,
    pub base_y: i32,
    pub base_z: i32,
    pub projectile: bool,
    pub members: bool,
    pub landscape: Option<Landscape>,
}
impl ChunkFlags {
    pub fn new(x: i32, y: i32, z: i32, projectile: bool) -> Self {
        Self {
            base_x: x,
            base_y: y,
            base_z: z,
            projectile,
            members: false,
            landscape: Some(Shared::new(
                (0..8).map(|_| Some(Shared::new(vec![false; 8]))).collect(),
            )),
        }
    }
    pub fn key(&self) -> i32 {
        self.base_z.wrapping_shl(22) | ((self.base_x >> 3).wrapping_shl(11)) | (self.base_y >> 3)
    }
    fn index(x: i32, y: i32) -> Result<usize> {
        let n = x.wrapping_mul(8).wrapping_add(y);
        if (0..64).contains(&n) {
            Ok(n as usize)
        } else {
            Err(Error::Index)
        }
    }
    pub fn get(&self, ctx: &mut impl Context, x: i32, y: i32) -> Result<i32> {
        let f = ctx.flags(self.key(), self.projectile)?;
        let n = Self::index(x, y)?;
        let v = f.read()[n];
        Ok(v)
    }
    pub fn add_flag(&self, ctx: &mut impl Context, x: i32, y: i32, mask: i32) -> Result<()> {
        let old = self.get(ctx, x, y)?;
        let f = ctx.flags(self.key(), self.projectile)?;
        let n = Self::index(x, y)?;
        f.write()[n] = old.max(0) | mask;
        Ok(())
    }
    pub fn remove_flag(&self, ctx: &mut impl Context, x: i32, y: i32, mask: i32) -> Result<()> {
        let old = self.get(ctx, x, y)?;
        if old & mask == 0 {
            return Ok(());
        }
        let f = ctx.flags(self.key(), self.projectile)?;
        let n = Self::index(x, y)?;
        f.write()[n] = old.max(0) & !mask;
        Ok(())
    }
    pub fn clear(&self, ctx: &mut impl Context, x: i32, y: i32) -> Result<()> {
        self.assign(ctx, x, y, 0)
    }
    pub fn invalidate(&self, ctx: &mut impl Context, x: i32, y: i32) -> Result<()> {
        self.assign(ctx, x, y, -1)
    }
    fn assign(&self, ctx: &mut impl Context, x: i32, y: i32, v: i32) -> Result<()> {
        let f = ctx.flags(self.key(), self.projectile)?;
        let n = Self::index(x, y)?;
        f.write()[n] = v;
        Ok(())
    }
    pub fn flag(&self, ctx: &mut impl Context, x: i32, y: i32, mask: i32, add: bool) -> Result<()> {
        if (0..8).contains(&x) && (0..8).contains(&y) {
            if add {
                self.add_flag(ctx, x, y, mask)
            } else {
                self.remove_flag(ctx, x, y, mask)
            }
        } else {
            ctx.border(Border {
                x: self.base_x.wrapping_add(x),
                y: self.base_y.wrapping_add(y),
                z: self.base_z,
                projectile: self.projectile,
                mask,
                add,
            })
        }
    }
    pub fn solid_tile(&self, ctx: &mut impl Context, x: i32, y: i32) -> Result<()> {
        self.flag(ctx, x, y, 0x200000, true)
    }
    pub fn empty_tile(&self, ctx: &mut impl Context, x: i32, y: i32) -> Result<()> {
        self.flag(ctx, x, y, 0, true)
    }
    pub fn tile_object(&self, ctx: &mut impl Context, x: i32, y: i32, add: bool) -> Result<()> {
        self.flag(ctx, x, y, 0x40000, add)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn solid_object(
        &self,
        ctx: &mut impl Context,
        x: i32,
        y: i32,
        size_x: i32,
        size_y: i32,
        projectile_clipped: bool,
        add: bool,
    ) -> Result<()> {
        let end_x = x.wrapping_add(size_x);
        let end_y = y.wrapping_add(size_y);
        if end_x <= x || end_y <= y {
            return Ok(());
        }
        if (end_x as i64 - x as i64) as u64 * (end_y as i64 - y as i64) as u64 > 65536 {
            return Err(Error::Limit);
        }
        let mask = 0x100 | if projectile_clipped { 0x20000 } else { 0 };
        for x in x..end_x {
            for y in y..end_y {
                self.flag(ctx, x, y, mask, add)?;
            }
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    pub fn door(
        &self,
        ctx: &mut impl Context,
        x: i32,
        y: i32,
        rotation: i32,
        shape: i32,
        projectile_clipped: bool,
        add: bool,
    ) -> Result<()> {
        if !(0..4).contains(&rotation) || !(0..4).contains(&shape) {
            return Ok(());
        }
        const CARD: [(i32, i32, i32, i32); 4] = [
            (-1, 0, 0x80, 8),
            (0, 1, 2, 0x20),
            (1, 0, 8, 0x80),
            (0, -1, 0x20, 2),
        ];
        const DIAG: [(i32, i32, i32, i32); 4] = [
            (-1, 1, 1, 0x10),
            (1, 1, 4, 0x40),
            (1, -1, 0x10, 1),
            (-1, -1, 0x40, 4),
        ];
        let r = rotation as usize;
        let mut p = [(0, 0, 0); 3];
        let count;
        if shape == 2 {
            let a = CARD[r];
            let b = CARD[(r + 1) & 3];
            p[0] = (0, 0, a.2 | b.2);
            p[1] = (a.0, a.1, a.3);
            p[2] = (b.0, b.1, b.3);
            count = 3;
        } else {
            let a = if shape == 1 || shape == 3 {
                DIAG[r]
            } else {
                CARD[r]
            };
            p[0] = (0, 0, a.2);
            p[1] = (a.0, a.1, a.3);
            count = 2;
        }
        // Source completes the movement-mask phase before any projectile masks.
        for phase in 0..if projectile_clipped { 2 } else { 1 } {
            for &(dx, dy, mask) in &p[..count] {
                self.flag(
                    ctx,
                    x.wrapping_add(dx),
                    y.wrapping_add(dy),
                    mask << (phase * 9),
                    add,
                )?;
            }
        }
        Ok(())
    }
    pub fn mark_landscape(&self, x: i32, y: i32, value: bool) -> Result<()> {
        let rows = self.landscape.as_ref().ok_or(Error::Null)?;
        let row = rows
            .read()
            .get(x as usize)
            .ok_or(Error::Index)?
            .clone()
            .ok_or(Error::Null)?;
        *row.write().get_mut(y as usize).ok_or(Error::Index)? = value;
        Ok(())
    }
}
