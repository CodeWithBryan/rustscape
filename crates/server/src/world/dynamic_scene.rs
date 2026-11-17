//! The instanced scene packets: BuildDynamicScene (214) with
//! DynamicSceneContext, Player.updateSceneGraph's choice between it and
//! UpdateSceneGraph (`rebuild`), MinimapState (192) and ClearMinimapFlag
//! (153).
use super::{
    doors::Location,
    dynamic_region,
    player_render::{Error, Handle, Result},
    rebuild,
    scene::{Assets, Scene},
};
use rs09_protocol::{buffer::Writer, game::Frame};

/// MapChunkRenderer.BUILD_AREA_SIZE: 13 chunks.
pub const BUILD_AREA_SIZE: usize = 13;
pub const PLANES: usize = 4;

fn location(p: &Handle) -> Result<Location> {
    p.read().location.clone().ok_or(Error::Null)
}
/// IoBuffer's bit access: most significant bit first over zeroed bytes.
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

/// `Player.updateSceneGraph(login)`: BuildDynamicScene when the region at the
/// player's location (RegionManager.forId) is a DynamicRegion, otherwise
/// UpdateSceneGraph. `login` only selects the context; neither packet reads it.
pub fn update_scene_graph<A: Assets>(
    scene: &mut Scene<A>,
    ctx: &mut impl rebuild::Context,
    player: &Handle,
    _login: bool,
) -> Result<()> {
    let [x, y, _] = location(player)?.coordinates();
    let region = scene.region(((x >> 6) << 8) | (y >> 6));
    if dynamic_region::is_dynamic(&region) {
        send(scene, ctx, player)
    } else {
        rebuild::send(ctx, player)
    }
}

/// BuildDynamicScene.send: the local position, zone x and plane, then one
/// bit per chunk of the 13 by 13 build area on four planes (set, with the
/// copied chunk's rotation, plane and zone, for a chunk of a DynamicRegion
/// whose base is not negative), the XTEA keys of every copied region in
/// first-use order, and the zone y. Every region of the area is looked up
/// (and so created) through the scene's region map. The last scene location
/// is published after the write.
pub fn send<A: Assets>(
    scene: &mut Scene<A>,
    ctx: &mut impl rebuild::Context,
    player: &Handle,
) -> Result<()> {
    let mut w = Writer::new();
    let mut region_ids: Vec<i32> = Vec::new();
    let [x, _, _] = location(player)?.coordinates();
    w.ip2add(x.wrapping_sub(((x >> 3).wrapping_sub(6)).wrapping_shl(3)));
    w.ip2add(location(player)?.coordinates()[0] >> 3);
    w.p1sub(location(player)?.coordinates()[2]);
    let [_, y, _] = location(player)?.coordinates();
    w.ip2add(y.wrapping_sub(((y >> 3).wrapping_sub(6)).wrapping_shl(3)));
    let mut chunks: Vec<Option<super::chunk::HandleChunk>> =
        vec![None; BUILD_AREA_SIZE * BUILD_AREA_SIZE * PLANES];
    let index = |x: usize, y: usize, z: usize| (x * BUILD_AREA_SIZE + y) * PLANES + z;
    let base_x = (location(player)?.coordinates()[0] >> 3) - 6;
    let base_y = (location(player)?.coordinates()[1] >> 3) - 6;
    let mut x = base_x;
    while x <= (location(player)?.coordinates()[0] >> 3) + 6 {
        let mut y = base_y;
        while y <= (location(player)?.coordinates()[1] >> 3) + 6 {
            for z in 0..PLANES {
                let region = scene.region(((x >> 3) << 8) | (y >> 3));
                if dynamic_region::is_dynamic(&region) {
                    let (rx, ry) = {
                        let r = region.read();
                        let d = r.data.read();
                        (d.x, d.y)
                    };
                    let (cx, cy) = (x - (rx << 3), y - (ry << 3));
                    if !(0..8).contains(&cx) || !(0..8).contains(&cy) {
                        return Err(Error::Binding("ArrayIndexOutOfBounds".into()));
                    }
                    let chunks_of = region.read().chunks.clone().ok_or(Error::Null)?;
                    let chunk = chunks_of.read()[cx as usize][cy as usize][z].clone();
                    chunks[index((x - base_x) as usize, (y - base_y) as usize, z)] = chunk;
                }
            }
            y += 1;
        }
        x += 1;
    }
    let mut bits = Bits::default();
    for plane in 0..PLANES {
        for offset_x in 0..BUILD_AREA_SIZE {
            for offset_y in 0..BUILD_AREA_SIZE {
                let Some(c) = chunks[index(offset_x, offset_y, plane)].clone() else {
                    bits.put(1, 0);
                    continue;
                };
                let (base, rotation) = {
                    let c = c.read();
                    (c.base.coordinates(), c.rotation)
                };
                if base[0] < 0 || base[1] < 0 {
                    bits.put(1, 0);
                    continue;
                }
                let real_region_x = base[0] >> 3;
                let real_region_y = base[1] >> 3;
                let real_plane = base[2];
                let id = ((real_region_x >> 3) << 8) | (real_region_y >> 3);
                if !region_ids.contains(&id) {
                    region_ids.push(id);
                }
                bits.put(1, 1);
                bits.put(
                    26,
                    (rotation << 1)
                        | (real_plane << 24)
                        | (real_region_x << 14)
                        | (real_region_y << 3),
                );
            }
        }
    }
    w.pdata(&bits.data);
    for id in region_ids {
        for key in ctx.keys(id)? {
            w.mp4(key);
        }
    }
    w.p2(location(player)?.coordinates()[1] >> 3);
    let last_scene = location(player)?;
    ctx.send(
        player,
        Frame {
            opcode: 214,
            payload: w.into_vec(),
        },
    )?;
    player.write().last_scene = Some(last_scene);
    Ok(())
}

/// MinimapState: one byte, the state (0 normal, 2 hidden in the content).
pub fn minimap_state(state: i32) -> Frame {
    let mut w = Writer::new();
    w.p1(state);
    Frame {
        opcode: 192,
        payload: w.into_vec(),
    }
}
/// ClearMinimapFlag: no payload.
pub fn clear_minimap_flag() -> Frame {
    Frame {
        opcode: 153,
        payload: Vec::new(),
    }
}
