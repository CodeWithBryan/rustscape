//! Ordinary IF1 scrollbar arithmetic and paint order from the pinned RT4 client.
use super::widget_legacy::Geometry;

#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub point: [i32; 2],
    pub bar: [i32; 2],
    pub height: i32,
    pub range: i32,
    pub width: i32,
    pub pressed: bool,
    pub wheel: i32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Drag {
    pub dragging: bool,
    pub margin: i32,
}

fn div(n: i32, d: i32) -> Result<i32, String> {
    if d == 0 {
        Err("Scrollbar division by zero".into())
    } else {
        Ok(n.wrapping_div(d))
    }
}

impl Drag {
    /// The shared drag flag resets on every eligible component call, even when
    /// the pointer misses. Scroll clamping belongs to subsequent layout/paint.
    pub fn handle(
        &mut self,
        scroll: &mut i32,
        i: Input,
        mut redraw: impl FnMut(),
    ) -> Result<(), String> {
        self.margin = if self.dragging { 32 } else { 0 };
        self.dragging = false;
        let [mx, my] = i.point;
        let [x, y] = i.bar;
        let right = x.wrapping_add(16);
        let bottom = i.height.wrapping_add(y);
        if i.pressed {
            if x <= mx && right > mx && my >= y && y.wrapping_add(16) > my {
                *scroll = scroll.wrapping_sub(4);
                redraw();
            } else if mx >= x && mx < right && my >= bottom.wrapping_sub(16) && bottom > my {
                *scroll = scroll.wrapping_add(4);
                redraw();
            } else if mx >= x.wrapping_sub(self.margin)
                && mx < x.wrapping_add(self.margin).wrapping_add(16)
                && my >= y.wrapping_add(16)
                && bottom.wrapping_sub(16) > my
            {
                let thumb = div(i.height.wrapping_mul(i.height.wrapping_sub(32)), i.range)?.max(8);
                let track = i.height.wrapping_sub(thumb).wrapping_sub(32);
                let pos = my.wrapping_sub(thumb / 2).wrapping_sub(y).wrapping_sub(16);
                *scroll = div(i.range.wrapping_sub(i.height).wrapping_mul(pos), track)?;
                redraw();
                self.dragging = true;
            }
        }
        if i.wheel != 0 && x.wrapping_sub(i.width) <= mx && y <= my && mx < right && bottom >= my {
            *scroll = scroll.wrapping_add(i.wheel.wrapping_mul(45));
            redraw();
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Part {
    Sprite(usize, [i32; 2]),
    Fill([i32; 4], i32),
    Vertical([i32; 3], i32),
    Horizontal([i32; 3], i32),
}

/// Emits the two cached indexed arrows before computing the thumb, preserving
/// completed calls on arithmetic failure. The caller restores the parent clip.
pub fn paint(
    scroll: i32,
    range: i32,
    [x, y]: [i32; 2],
    height: i32,
    mut emit: impl FnMut(Part) -> Result<(), String>,
) -> Result<(), String> {
    emit(Part::Sprite(0, [x, y]))?;
    emit(Part::Sprite(
        1,
        [x, height.wrapping_add(y).wrapping_sub(16)],
    ))?;
    let thumb = div(height.wrapping_mul(height.wrapping_sub(32)), range)?.max(8);
    let pos = div(
        scroll.wrapping_mul(height.wrapping_sub(thumb).wrapping_sub(32)),
        range.wrapping_sub(height),
    )?;
    let top = pos.wrapping_add(y).wrapping_add(16);
    emit(Part::Fill(
        [x, y.wrapping_add(16), 16, height.wrapping_sub(32)],
        0x23201b,
    ))?;
    emit(Part::Fill(
        [x, y.wrapping_add(pos).wrapping_add(16), 16, thumb],
        0x4d4233,
    ))?;
    emit(Part::Vertical([x, top, thumb], 0x766654))?;
    emit(Part::Vertical([x.wrapping_add(1), top, thumb], 0x766654))?;
    emit(Part::Horizontal([x, top, 16], 0x766654))?;
    emit(Part::Horizontal([x, top.wrapping_add(1), 16], 0x766654))?;
    emit(Part::Vertical([x.wrapping_add(15), top, thumb], 0x332d25))?;
    emit(Part::Vertical(
        [
            x.wrapping_add(14),
            top.wrapping_add(1),
            thumb.wrapping_sub(1),
        ],
        0x332d25,
    ))?;
    emit(Part::Horizontal(
        [
            x,
            thumb.wrapping_add(y).wrapping_add(pos).wrapping_add(15),
            16,
        ],
        0x332d25,
    ))?;
    emit(Part::Horizontal(
        [
            x.wrapping_add(1),
            y.wrapping_add(14).wrapping_add(pos).wrapping_add(thumb),
            15,
        ],
        0x332d25,
    ))
}

/// Original float-operation order, including the GL axis-line 0.3f offset.
pub fn geometry(part: Part, canvas_height: i32) -> Option<(Geometry, i32)> {
    let (vertices, thin, color) = match part {
        Part::Sprite(..) => return None,
        Part::Fill([x, y, w, h], color) => {
            let left = x as f32;
            let right = left + w as f32;
            let top = canvas_height.wrapping_sub(y) as f32;
            let bottom = top - h as f32;
            (
                vec![[left, top], [left, bottom], [right, bottom], [right, top]],
                false,
                color,
            )
        }
        Part::Vertical([x, y, h], color) => {
            let x = x as f32 + 0.3;
            let top = canvas_height as f32 - y as f32 - 0.3;
            (vec![[x, top], [x, top - h as f32]], true, color)
        }
        Part::Horizontal([x, y, w], color) => {
            let left = x as f32 + 0.3;
            let y = canvas_height as f32 - y as f32 - 0.3;
            (vec![[left, y], [left + w as f32, y]], true, color)
        }
    };
    Some((Geometry { vertices, thin }, color))
}
