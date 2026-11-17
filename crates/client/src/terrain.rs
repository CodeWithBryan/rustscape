//! Tile stream and procedural height calculation from RT4 SceneGraph/PerlinNoise.
use crate::animation::sin_cos;
use crate::atmosphere::Environment;
use rs09_protocol::{Error, Result, buffer::Reader};

#[derive(Clone, Copy, Debug, Default)]
pub struct Tile {
    pub height: i32,
    pub underlay: u8,
    pub overlay: u8,
    pub shape: u8,
    pub rotation: u8,
    pub flags: u8,
}
pub struct Region {
    pub x: i32,
    pub z: i32,
    pub tiles: Vec<Tile>,
    pub environment: Environment,
    /// Retained raw point-light records; their rendering is a separate parity step.
    pub point_lights: Vec<Vec<u8>>,
    /// An instanced scene's atmosphere for each 8x8 chunk, indexed
    /// `chunk_x * 8 + chunk_z`. Empty for an ordinary region, whose chunks all
    /// share `environment`.
    pub chunk_environments: Vec<Environment>,
    /// Map opcode 129 records in file order: the camera's occlusion heights,
    /// which `follow_camera::Occlusion` lays into the scene as
    /// `SceneGraph.readTerrain` does.
    pub occlusion: Vec<[OcclusionLevel; 4]>,
    /// The region's `um`/`ul` map files, when the source of the region
    /// says whether it has any.
    pub underwater: crate::underwater::Source,
}

/// One level of a map opcode 129 record.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OcclusionLevel {
    /// 0 clears, 1 carries heights, 2 copies the level below; other values do
    /// nothing.
    pub kind: i8,
    /// For kind 1: 16 x 16 heights, one per 4 x 4 tiles, x-major. Else empty.
    pub heights: Vec<u8>,
}

/// SceneGraph.getTileHeight over the prepared regions and current scene origin.
/// Fine coordinates are global; the original104-tile bounds are scene-local.
pub fn scene_height(regions: &[Region], origin: [i32; 2], plane: i32, fine: [i32; 2]) -> i32 {
    let local = std::array::from_fn(|i| fine[i].wrapping_sub(origin[i].wrapping_mul(128)));
    crate::ground_geometry::tile_height(plane, local, |p, x, z| {
        let x = origin[0].wrapping_add(x);
        let z = origin[1].wrapping_add(z);
        let tile = regions
            .iter()
            .find(|r| r.x == x.div_euclid(64) && r.z == z.div_euclid(64))
            .map_or(Tile::default(), |r| {
                r.tile(
                    p as usize,
                    x.rem_euclid(64) as usize,
                    z.rem_euclid(64) as usize,
                )
            });
        (tile.height, tile.flags)
    })
}

/// SceneGraph.getRenderLevel: forced-ground flags precede bridge lowering.
pub fn render_level(plane: usize, flags: u8, bridge_flags: u8) -> usize {
    if flags & 8 != 0 {
        0
    } else if plane > 0 && bridge_flags & 2 != 0 {
        plane - 1
    } else {
        plane
    }
}
impl Region {
    pub fn decode(x: i32, z: i32, data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let mut tiles = vec![Tile::default(); 4 * 64 * 64];
        for plane in 0..4 {
            for lx in 0..64 {
                for lz in 0..64 {
                    let index = plane * 4096 + lx * 64 + lz;
                    let mut tile = Tile::default();
                    loop {
                        match r.g1()? {
                            0 => {
                                tile.height = if plane == 0 {
                                    -tile_height(
                                        z * 64 + lz as i32 + 556238,
                                        x * 64 + lx as i32 + 932731,
                                    ) * 8
                                } else {
                                    tiles[index - 4096].height - 240
                                };
                                break;
                            }
                            1 => {
                                let mut height = r.g1()?;
                                if height == 1 {
                                    height = 0;
                                }
                                tile.height = if plane == 0 {
                                    -height * 8
                                } else {
                                    tiles[index - 4096].height - height * 8
                                };
                                break;
                            }
                            op @ 2..=49 => {
                                tile.overlay = r.g1()? as u8;
                                tile.shape = ((op - 2) / 4) as u8;
                                tile.rotation = ((op - 2) & 3) as u8;
                            }
                            op @ 50..=81 => tile.flags = (op - 49) as u8,
                            op => tile.underlay = (op - 81) as u8,
                        }
                    }
                    tiles[index] = tile;
                }
            }
        }
        let mut environment = Environment::default();
        let mut point_lights = Vec::new();
        let mut occlusion = Vec::new();
        while r.remaining() > 0 {
            match r.g1()? {
                129 => {
                    let mut record: [OcclusionLevel; 4] = Default::default();
                    for level in &mut record {
                        level.kind = r.g1b()? as i8;
                        if level.kind == 1 {
                            level.heights = r.take(256)?.to_vec();
                        }
                    }
                    occlusion.push(record);
                }
                0 => environment = Environment::decode(&mut r)?,
                1 => {
                    let count = r.g1()?;
                    for _ in 0..count {
                        let start = r.position();
                        r.take(7)?; // flags and three unsigned coordinates
                        let radius = r.g1()? as usize;
                        r.take((radius * 2 + 1) * 2 + 2)?; // spans and palette color
                        let preset = r.g1()? & 31;
                        if preset == 31 {
                            r.g2()?;
                        }
                        point_lights.push(data[start..r.position()].to_vec());
                    }
                }
                _ => {
                    return Err(Error::new(
                        r.position() - 1,
                        "unknown map environment entry",
                    ));
                }
            }
        }
        Ok(Self {
            x,
            z,
            tiles,
            environment,
            point_lights,
            chunk_environments: vec![],
            occlusion,
            underwater: Default::default(),
        })
    }
    /// `FogManager.chunksAtmosphere` for the chunk holding a region-local tile.
    pub fn environment_at(&self, x: usize, z: usize) -> Environment {
        self.chunk_environments
            .get((x.min(63) / 8) * 8 + z.min(63) / 8)
            .copied()
            .unwrap_or(self.environment)
    }
    pub fn tile(&self, plane: usize, x: usize, z: usize) -> Tile {
        self.tiles[plane * 4096 + x.min(63) * 64 + z.min(63)]
    }
}
pub fn noise(x: i32, y: i32) -> i32 {
    let n = y.wrapping_mul(57).wrapping_add(x);
    let h = n ^ (n.wrapping_shl(13));
    let v = h
        .wrapping_mul(h)
        .wrapping_mul(15731)
        .wrapping_add(789221)
        .wrapping_mul(h)
        .wrapping_add(1376312589)
        & i32::MAX;
    (v >> 19) & 255
}
fn smooth(x: i32, y: i32) -> i32 {
    let corners =
        noise(x - 1, y - 1) + noise(x + 1, y - 1) + noise(x - 1, y + 1) + noise(x + 1, y + 1);
    let sides = noise(x - 1, y) + noise(x + 1, y) + noise(x, y - 1) + noise(x, y + 1);
    sides / 8 + corners / 16 + noise(x, y) / 4
}
fn interpolate(a: i32, b: i32, t: i32, scale: i32) -> i32 {
    let weight = (65536 - sin_cos(t * 1024 / scale).1) >> 1;
    ((a * (65536 - weight)) >> 16) + ((b * weight) >> 16)
}
fn interpolated(frequency: i32, x: i32, y: i32) -> i32 {
    let gx = x / frequency;
    let gy = y / frequency;
    let fx = x & (frequency - 1);
    let fy = y & (frequency - 1);
    interpolate(
        interpolate(smooth(gx, gy), smooth(gx + 1, gy), fx, frequency),
        interpolate(smooth(gx, gy + 1), smooth(gx + 1, gy + 1), fx, frequency),
        fy,
        frequency,
    )
}
pub fn tile_height(x: i32, y: i32) -> i32 {
    let height = interpolated(4, y + 45365, x + 91923)
        + ((interpolated(2, y + 10294, x + 37821) - 128) >> 1)
        + ((interpolated(1, y, x) - 128) >> 2)
        - 128;
    ((f64::from(height) * 0.3) as i32 + 35).clamp(10, 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tile_opcode_height_one_is_zero_and_planes_are_relative() {
        let mut bytes = vec![1, 1];
        bytes.resize(4 * 4096 + 1, 0);
        let r = Region::decode(50, 50, &bytes).unwrap();
        assert_eq!(r.tile(0, 0, 0).height, 0);
        assert_eq!(r.tile(1, 0, 0).height, -240);
        assert!(Region::decode(50, 50, &[]).is_err());
    }
}
