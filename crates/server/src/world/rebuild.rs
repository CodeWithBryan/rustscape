//! Original UpdateSceneGraph's ordered reads, state assignment and key requests.
use super::{
    doors::Location,
    player_render::{Error, Handle, Result},
};
use rs09_protocol::{buffer::Writer, game::Frame};
pub trait Context {
    fn keys(&mut self, id: i32) -> Result<[i32; 4]>;
    fn send(&mut self, player: &Handle, frame: Frame) -> Result<()>;
}
fn location(p: &Handle) -> Result<Location> {
    p.read().location.clone().ok_or(Error::Null)
}
pub fn send(ctx: &mut impl Context, p: &Handle) -> Result<()> {
    let last_scene = location(p)?;
    p.write().last_scene = Some(last_scene);
    let mut w = Writer::new();
    let [x, _, _] = location(p)?.coordinates();
    w.p2add(x.wrapping_sub(((x >> 3).wrapping_sub(6)).wrapping_shl(3)));
    let mut x = ((location(p)?.coordinates()[0] >> 3) - 6) / 8;
    let mut rows = 0usize;
    while x <= ((location(p)?.coordinates()[0] >> 3) + 6) / 8 {
        let mut y = ((location(p)?.coordinates()[1] >> 3) - 6) / 8;
        while y <= ((location(p)?.coordinates()[1] >> 3) + 6) / 8 {
            rows += 1;
            if rows > 65536 {
                return Err(Error::Binding("normal rebuild key limit".into()));
            }
            for key in ctx.keys(x.wrapping_shl(8) | y)? {
                w.mp4(key);
            }
            y = y.wrapping_add(1);
        }
        x = x.wrapping_add(1);
    }
    w.p1sub(location(p)?.coordinates()[2]);
    w.p2(location(p)?.coordinates()[0] >> 3);
    w.p2add(location(p)?.coordinates()[1] >> 3);
    let [_, y, _] = location(p)?.coordinates();
    w.p2add(y.wrapping_sub(((y >> 3).wrapping_sub(6)).wrapping_shl(3)));
    ctx.send(
        p,
        Frame {
            opcode: 162,
            payload: w.into_vec(),
        },
    )
}
