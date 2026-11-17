//! Selected legacy inventory icon presentation and per-render parent scrolling.

/// RT4 keeps exact +/-5 offsets while suppressing the interior deadzone.
pub fn delta(point: [i32; 2], press: [i32; 2], cycles: i32) -> [i32; 2] {
    let mut d = [
        point[0].wrapping_sub(press[0]),
        point[1].wrapping_sub(press[1]),
    ];
    for n in &mut d {
        if (-4..=4).contains(n) {
            *n = 0;
        }
    }
    if cycles < 5 {
        d = [0, 0];
    }
    d
}

#[derive(Clone, Copy, Debug)]
pub struct Parent {
    pub scroll: i32,
    pub range: i32,
    pub height: i32,
}

/// Runs after the selected sprite draw and parent lookup. These are independent
/// branches: a short raster clip can execute both and observe the first edit.
pub fn edge_scroll(
    mut parent: Option<&mut Parent>,
    press_y: &mut i32,
    y: i32,
    drag_y: i32,
    [top, bottom]: [i32; 2],
    scene_delta: i32,
    mut redraw: impl FnMut(i32, i32),
) -> Result<(), String> {
    if top > drag_y.wrapping_add(y) {
        let parent = parent
            .as_deref_mut()
            .ok_or("Missing legacy inventory scroll parent")?;
        if parent.scroll > 0 {
            let mut amount = scene_delta.wrapping_mul(top.wrapping_sub(drag_y).wrapping_sub(y)) / 3;
            if amount > scene_delta.wrapping_mul(10) {
                amount = scene_delta.wrapping_mul(10);
            }
            if amount > parent.scroll {
                amount = parent.scroll;
            }
            parent.scroll = parent.scroll.wrapping_sub(amount);
            *press_y = press_y.wrapping_add(amount);
            redraw(parent.scroll, *press_y);
        }
    }
    if bottom < drag_y.wrapping_add(y).wrapping_add(32) {
        let parent = parent.ok_or("Missing legacy inventory scroll parent")?;
        if parent.scroll < parent.range.wrapping_sub(parent.height) {
            let mut amount = y
                .wrapping_add(drag_y)
                .wrapping_add(32)
                .wrapping_sub(bottom)
                .wrapping_mul(scene_delta)
                / 3;
            if amount > scene_delta.wrapping_mul(10) {
                amount = scene_delta.wrapping_mul(10);
            }
            if parent
                .range
                .wrapping_sub(parent.scroll)
                .wrapping_sub(parent.height)
                < amount
            {
                amount = parent
                    .range
                    .wrapping_sub(parent.height)
                    .wrapping_sub(parent.scroll);
            }
            parent.scroll = parent.scroll.wrapping_add(amount);
            *press_y = press_y.wrapping_sub(amount);
            redraw(parent.scroll, *press_y);
        }
    }
    Ok(())
}
