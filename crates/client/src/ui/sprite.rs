//! All frames of a SpriteLoader sheet, retaining trimmed bounds and palette indices.
use rs09_protocol::{Error, Result, buffer::Reader};
#[derive(Clone, Debug)]
pub struct Frame {
    pub offset: [usize; 2],
    pub size: [usize; 2],
    pub indices: Vec<u8>,
    pub alpha: Vec<u8>,
    pub has_alpha: bool,
}
#[derive(Clone, Debug)]
pub struct Sheet {
    pub size: [usize; 2],
    pub palette: Vec<u32>,
    pub frames: Vec<Frame>,
}
impl Sheet {
    pub fn decode(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let end = data
            .len()
            .checked_sub(2)
            .ok_or(Error::new(0, "truncated sprite sheet"))?;
        r.seek(end)?;
        let count = r.g2()? as usize;
        let meta = data
            .len()
            .checked_sub(count * 8 + 7)
            .ok_or(Error::new(0, "invalid sprite footer"))?;
        r.seek(meta)?;
        let size = [r.g2()? as usize, r.g2()? as usize];
        let colors = r.g1()? as usize + 1;
        let mut arrays = vec![vec![0usize; count]; 4];
        for a in &mut arrays {
            for v in a {
                *v = r.g2()? as usize;
            }
        }
        let palette_start = meta
            .checked_sub((colors - 1) * 3)
            .ok_or(Error::new(meta, "invalid sprite palette"))?;
        r.seek(palette_start)?;
        let mut palette = vec![0; colors];
        for v in palette.iter_mut().skip(1) {
            *v = (r.g3()? as u32).max(1);
        }
        r.seek(0)?;
        let mut frames = Vec::new();
        for i in 0..count {
            let offset = [arrays[0][i], arrays[1][i]];
            let dims = [arrays[2][i], arrays[3][i]];
            let len = dims[0]
                .checked_mul(dims[1])
                .filter(|&n| n <= 4 * 1024 * 1024)
                .ok_or(Error::new(r.position(), "oversized sprite"))?;
            let flags = r.g1()?;
            let mut indices = vec![0; len];
            let mut alpha = vec![0; len];
            fn plane(
                r: &mut Reader<'_>,
                out: &mut [u8],
                dims: [usize; 2],
                columns: bool,
            ) -> Result<()> {
                if columns {
                    for x in 0..dims[0] {
                        for y in 0..dims[1] {
                            out[y * dims[0] + x] = r.g1()? as u8;
                        }
                    }
                } else {
                    out.copy_from_slice(r.take(out.len())?);
                }
                Ok(())
            }
            plane(&mut r, &mut indices, dims, flags & 1 != 0)?;
            let mut has_alpha = false;
            if flags & 2 != 0 {
                plane(&mut r, &mut alpha, dims, flags & 1 != 0)?;
                has_alpha = alpha.iter().any(|&a| a != 255);
            }
            if r.position() > palette_start {
                return Err(Error::new(r.position(), "sprite pixels overlap palette"));
            }
            if indices.iter().any(|&p| p as usize >= colors) {
                return Err(Error::new(r.position(), "invalid palette index"));
            }
            frames.push(Frame {
                offset,
                size: dims,
                indices,
                alpha,
                has_alpha,
            });
        }
        Ok(Self {
            size,
            palette,
            frames,
        })
    }
    pub fn rgba(&self, index: usize) -> Result<Vec<u8>> {
        let f = self
            .frames
            .get(index)
            .ok_or(Error::new(index, "missing sprite frame"))?;
        let mut out = Vec::with_capacity(f.indices.len() * 4);
        for (i, &p) in f.indices.iter().enumerate() {
            let color = self.palette[p as usize];
            out.extend([
                (color >> 16) as u8,
                (color >> 8) as u8,
                color as u8,
                if f.has_alpha {
                    f.alpha[i]
                } else if color == 0 {
                    0
                } else {
                    255
                },
            ]);
        }
        Ok(out)
    }
}
