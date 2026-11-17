//! Complete MapscapeParser terrain-byte and floor-clipping loops. Region chunk
//! ownership and global clipping storage remain the caller's native context.
use super::{
    clipping,
    landscape::{Chunks, Mapscape},
};
use rs09_protocol::buffer::Reader;
#[derive(Debug)]
pub enum Error {
    Null,
    Decode(rs09_protocol::Error),
    Clipping(clipping::Error),
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;
impl From<rs09_protocol::Error> for Error {
    fn from(e: rs09_protocol::Error) -> Self {
        Self::Decode(e)
    }
}
impl From<clipping::Error> for Error {
    fn from(e: clipping::Error) -> Self {
        Self::Clipping(e)
    }
}
pub trait Context: clipping::Context {
    fn region_chunks(&mut self) -> Result<Option<Chunks>>;
}
pub fn parse(ctx: &mut impl Context, map: &Mapscape, r: &mut Reader<'_>) -> Result<()> {
    if ctx.region_chunks()?.is_none() {
        return Ok(());
    }
    for z in 0..4 {
        for x in 0..64 {
            for y in 0..64 {
                let chunks = ctx.region_chunks()?.ok_or(Error::Null)?;
                let chunk = chunks.read()[x >> 3][y >> 3][z].clone();
                let landscape = if let Some(chunk) = chunk {
                    chunk.read().flags.landscape.clone()
                } else {
                    clipping::ChunkFlags::new(0, 0, 0, false).landscape
                };
                loop {
                    let value = r.g1()?;
                    if value == 0 {
                        break;
                    }
                    if value == 1 {
                        r.g1()?;
                        break;
                    }
                    let mark = if value <= 49 {
                        r.g1()? > 0
                    } else if value <= 81 {
                        map.write()[z][x][y] = (value - 49) as i8;
                        false
                    } else {
                        ((value - 81) as i8 as u8) > 0
                    };
                    if mark {
                        let rows = landscape.as_ref().ok_or(Error::Null)?;
                        let row = rows
                            .read()
                            .get(x & 7)
                            .ok_or(clipping::Error::Index)?
                            .clone()
                            .ok_or(Error::Null)?;
                        *row.write().get_mut(y & 7).ok_or(clipping::Error::Index)? = true;
                    }
                }
            }
        }
    }
    Ok(())
}
pub fn clip(ctx: &mut impl Context, chunks: &Chunks, map: &Mapscape) -> Result<()> {
    for z in 0..4 {
        for cx in 0..8 {
            for cy in 0..8 {
                let chunk = chunks.read()[cx][cy][z].clone().ok_or(Error::Null)?;
                for x in 0..8 {
                    for y in 0..8 {
                        let flags = chunk.read().flags.clone();
                        flags.empty_tile(ctx, x as i32, y as i32)?;
                        let (mx, my) = (cx * 8 + x, cy * 8 + y);
                        if map.read()[z][mx][my] & 1 == 1 {
                            let mut plane = z as i32;
                            if map.read()[1][mx][my] & 2 == 2 {
                                plane -= 1;
                            }
                            if plane >= 0 {
                                let destination = ctx.region_chunks()?.ok_or(Error::Null)?;
                                let target = destination.read()[cx][cy][plane as usize]
                                    .clone()
                                    .ok_or(Error::Null)?;
                                let flags = target.read().flags.clone();
                                flags.solid_tile(ctx, x as i32, y as i32)?;
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
