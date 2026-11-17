//! Original IF1 type2 slot traversal and sprite request selection.
use super::Component;

#[derive(Clone, Copy, Debug)]
pub struct Context {
    pub origin: [i32; 2],
    /// Left, top, right, bottom; preserve Java comparisons even for empty clips.
    pub clip: [i32; 4],
    pub dragged_slot: Option<i32>,
    pub selected_slot: Option<i32>,
    pub pressed_slot: Option<i32>,
}
#[derive(Clone, Copy, Debug)]
pub enum Request {
    Item {
        slot: i32,
        at: [i32; 2],
        id: i32,
        count: i32,
        outline: i32,
        shadow: u32,
        draw_text: bool,
        alpha: i32,
        dragged: bool,
    },
    Background {
        slot: i32,
        at: [i32; 2],
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    NullArray,
    ArrayIndex,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NullArray => "Missing inventory render array",
            Self::ArrayIndex => "Invalid inventory render array index",
        })
    }
}
fn element(a: Option<&[i32]>, slot: i32) -> Result<i32, Error> {
    a.ok_or(Error::NullArray)?
        .get(slot as usize)
        .copied()
        .ok_or(Error::ArrayIndex)
}
pub fn visit(c: &Component, context: Context, mut draw: impl FnMut(Request)) -> Result<(), Error> {
    let mut slot = 0i32;
    for row in 0..c.base_height {
        for col in 0..c.base_width {
            let mut y =
                context.origin[1].wrapping_add(row.wrapping_mul(c.inv_margin_y.wrapping_add(32)));
            let mut x = col
                .wrapping_mul(c.inv_margin_x.wrapping_add(32))
                .wrapping_add(context.origin[0]);
            if slot < 20 {
                y = y.wrapping_add(element(c.inv_offset_y.as_deref(), slot)?);
                x = x.wrapping_add(element(c.inv_offset_x.as_deref(), slot)?);
            }
            let encoded = element(c.obj_types.as_deref(), slot)?;
            if encoded > 0 {
                let [left, top, right, bottom] = context.clip;
                let dragged = context.dragged_slot == Some(slot);
                if left < x.wrapping_add(32) && x < right && top < y.wrapping_add(32) && y < bottom
                    || dragged
                {
                    let selected = context.selected_slot == Some(slot);
                    let count = element(c.obj_counts.as_deref(), slot)?;
                    draw(Request::Item {
                        slot,
                        at: [x, y],
                        id: encoded - 1,
                        count,
                        outline: if selected { 2 } else { 1 },
                        shadow: if selected { 0 } else { 3153952 },
                        draw_text: c.obj_draw_text,
                        alpha: if dragged || context.pressed_slot == Some(slot) {
                            128
                        } else {
                            256
                        },
                        dragged,
                    });
                }
            } else if c.inv_sprite.is_some() && slot < 20 {
                draw(Request::Background { slot, at: [x, y] });
            }
            slot = slot.wrapping_add(1);
        }
    }
    Ok(())
}
