//! Native indexed sprites, RT4 material definitions, and fixed-point texture graphs.
//! Unsupported procedural operations return an error instead of inventing a texture.
use crate::pack::Pack;
pub use crate::texture_graph::Graph;
use rs09_protocol::{Error, Result, buffer::Reader};

#[derive(Clone, Debug)]
pub struct Material {
    pub repeating: bool,
    pub opaque: bool,
    pub low_detail: bool,
    pub flipped: bool,
    pub speed: u8,
    pub brightness: u8,
    pub material_type: u8,
    pub animation_type: u8,
    pub average_color: u16,
}

/// `Js5GlTextureProvider.lowDetail`: `!Preferences.highDetailTextures`, given
/// to the provider's constructor (`client.mainLoad` state 90) and to
/// `setLowDetail` by CS2 6006.
static LOW_DETAIL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Sets the provider's flag. Returns whether it changed, which is when
/// `setLowDetail` has every texture generated again.
pub fn set_low_detail(low: bool) -> bool {
    LOW_DETAIL.swap(low, std::sync::atomic::Ordering::Relaxed) != low
}

/// `Js5GlTextureProvider.isLowDetail`: a texture is generated and bound at
/// 64 by 64 instead of 128 by 128 when its own flag or the provider's is set
/// (`GlTexture.bind`, `getPixels`, `getAnimatedPixels`).
pub fn low_detail(material: &Material) -> bool {
    LOW_DETAIL.load(std::sync::atomic::Ordering::Relaxed) || material.low_detail
}

/// SpecularMaterialRenderer restores primary opacity: texture alpha is a
/// specular intensity mask, including its no-cube-map fallback.
pub fn primary_alpha(material: Option<&Material>) -> bool {
    material.is_some_and(|m| m.material_type == 1)
}
pub fn materials(data: &[u8]) -> Result<Vec<Option<Material>>> {
    let mut r = Reader::new(data);
    let count = r.g2()? as usize;
    let defined = r.take(count)?.to_vec();
    let mut fields = vec![[0u8; 8]; count];
    // The file stores each field for every material in turn.
    #[allow(clippy::needless_range_loop)]
    for field in 0..8 {
        for (i, &flag) in defined.iter().enumerate() {
            if flag == 1 {
                fields[i][field] = r.g1()? as u8;
            }
        }
    }
    let mut result = Vec::with_capacity(count);
    for (i, flag) in defined.into_iter().enumerate() {
        result.push(if flag == 1 {
            let f = fields[i];
            Some(Material {
                repeating: f[0] == 1,
                opaque: f[1] == 1,
                low_detail: f[2] == 1,
                flipped: f[3] == 1,
                speed: f[4],
                brightness: f[5],
                material_type: f[6],
                animation_type: f[7],
                average_color: r.g2()? as u16,
            })
        } else {
            None
        });
    }
    Ok(result)
}
pub fn material_colors(data: &[u8]) -> Result<Vec<Option<u16>>> {
    Ok(materials(data)?
        .into_iter()
        .map(|m| m.map(|m| m.average_color))
        .collect())
}

pub struct Sprite {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}
impl Sprite {
    pub fn decode(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let end = data
            .len()
            .checked_sub(2)
            .ok_or(Error::new(0, "truncated sprite"))?;
        r.seek(end)?;
        let frames = r.g2()? as usize;
        if frames == 0 {
            return Err(Error::new(end, "empty sprite sheet"));
        }
        let meta = data
            .len()
            .checked_sub(frames * 8 + 7)
            .ok_or(Error::new(0, "invalid sprite footer"))?;
        r.seek(meta)?;
        let width = r.g2()? as usize;
        let height = r.g2()? as usize;
        let palette_count = r.g1()? as usize + 1;
        if width == 0 || height == 0 || width * height > 4 * 1024 * 1024 {
            return Err(Error::new(meta, "invalid sprite dimensions"));
        }
        let mut arrays = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        for a in &mut arrays {
            for _ in 0..frames {
                a.push(r.g2()? as usize);
            }
        }
        let palette_start = meta
            .checked_sub((palette_count - 1) * 3)
            .ok_or(Error::new(meta, "invalid sprite palette"))?;
        r.seek(palette_start)?;
        let mut palette = vec![0u32; palette_count];
        for c in palette.iter_mut().skip(1) {
            *c = r.g3()? as u32;
            if *c == 0 {
                *c = 1;
            }
        }
        let (ox, oy, inner_w, inner_h) = (arrays[0][0], arrays[1][0], arrays[2][0], arrays[3][0]);
        if ox + inner_w > width || oy + inner_h > height {
            return Err(Error::new(meta, "sprite trim exceeds sheet"));
        }
        r.seek(0)?;
        let flags = r.g1()?;
        let size = inner_w * inner_h;
        let indices = r.take(size)?;
        let alpha = if flags & 2 != 0 {
            Some(r.take(size)?)
        } else {
            None
        };
        let mut rgba = vec![0u8; width * height * 4];
        for y in 0..inner_h {
            for x in 0..inner_w {
                let src = if flags & 1 != 0 {
                    x * inner_h + y
                } else {
                    y * inner_w + x
                };
                let index = indices[src] as usize;
                let color = *palette
                    .get(index)
                    .ok_or(Error::new(src, "sprite palette index out of bounds"))?;
                let off = ((y + oy) * width + x + ox) * 4;
                rgba[off..off + 4].copy_from_slice(&[
                    (color >> 16) as u8,
                    (color >> 8) as u8,
                    color as u8,
                    alpha.map_or(if index == 0 { 0 } else { 255 }, |a| a[src]),
                ]);
            }
        }
        Ok(Self {
            width,
            height,
            rgba,
        })
    }
}

/// Generate the original HD RGBA graph at its fixed brightness of 0.7.
pub fn generate(pack: &Pack, id: i32, size: usize) -> std::result::Result<Vec<u8>, String> {
    let graph = crate::texture_graph::Graph::decode(pack.get(&format!("texture/{id}"))?)
        .map_err(|e| e.to_string())?;
    graph.rgba(pack, size, 0.7, true, 0)
}
