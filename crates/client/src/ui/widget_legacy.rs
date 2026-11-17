//! Ordinary legacy item text, tooltips and original GL line geometry.
use super::{Component, widget_style};

#[derive(Debug)]
pub struct Line {
    pub text: String,
    pub position: [i32; 2],
    pub align: i32,
    pub color: i32,
    pub shadow: bool,
}

/// Font readiness is checked by the caller before any item service is invoked.
/// Commands are delivered as they complete, preserving a failed lookup's prefix.
pub fn item_list(
    c: &Component,
    origin: [i32; 2],
    mut item: impl FnMut(i32) -> Result<(String, i32), String>,
    mut draw: impl FnMut(Line),
) -> Result<(), String> {
    let slots = i64::from(c.base_width.max(0)) * i64::from(c.base_height.max(0));
    if slots > 1_048_576 {
        return Err("Legacy item-text list exceeds native bound".into());
    }
    let mut slot = 0;
    for row in 0..c.base_height {
        for col in 0..c.base_width {
            let id = *c
                .obj_types
                .as_ref()
                .and_then(|a| a.get(slot))
                .ok_or("Missing legacy item-text slot")?;
            if id > 0 {
                let (name, stackable) = item(id - 1)?;
                let count = *c
                    .obj_counts
                    .as_ref()
                    .and_then(|a| a.get(slot))
                    .ok_or("Missing legacy item-text count")?;
                let text = if stackable != 1 && count == 1 {
                    format!("<col=ff9040>{name}</col>")
                } else {
                    format!(
                        "<col=ff9040>{name}</col> x{}",
                        widget_style::item_count(count)
                    )
                };
                let x = origin[0].wrapping_add(col.wrapping_mul(c.inv_margin_x.wrapping_add(115)));
                let y = origin[1].wrapping_add(row.wrapping_mul(c.inv_margin_y.wrapping_add(12)));
                let align = if c.halign == 0 {
                    0
                } else if c.halign == 1 {
                    1
                } else {
                    2
                };
                draw(Line {
                    text,
                    position: [x.wrapping_add([0, 57, 114][align as usize]), y],
                    align,
                    color: c.color,
                    shadow: c.shadowed,
                });
            }
            slot += 1;
        }
    }
    Ok(())
}

pub enum TooltipPart {
    Box([i32; 4]),
    Text(Line),
}
fn lines(text: &str) -> impl Iterator<Item = &str> {
    // The original while(length>0) keeps interior/leading empty lines, but does
    // not create another line after a trailing <br>.
    text.split_terminator("<br>")
}
/// Interpolation occurs twice in the original: measuring and then drawing. The
/// provider is called separately so changing CS1 service values remain visible.
pub fn tooltip(
    c: &Component,
    origin: [i32; 2],
    clip_end: [i32; 2],
    line_height: i32,
    mut interpolate: impl FnMut() -> Result<String, String>,
    mut width: impl FnMut(&str) -> i32,
    mut draw: impl FnMut(TooltipPart),
) -> Result<(), String> {
    let mut w: i32 = 0;
    let mut h: i32 = 0;
    for line in lines(&interpolate()?) {
        w = w.max(width(line));
        h = h.wrapping_add(line_height.wrapping_add(1));
    }
    w = w.wrapping_add(6);
    h = h.wrapping_add(7);
    let mut y = origin[1].wrapping_add(c.height).wrapping_add(5);
    if y.wrapping_add(h) > clip_end[1] {
        y = clip_end[1].wrapping_sub(h);
    }
    let mut x = origin[0]
        .wrapping_add(c.width)
        .wrapping_sub(w)
        .wrapping_sub(5);
    x = x.max(origin[0].wrapping_add(5));
    if w.wrapping_add(x) > clip_end[0] {
        x = clip_end[0].wrapping_sub(w);
    }
    let mut baseline = y.wrapping_add(line_height).wrapping_add(2);
    draw(TooltipPart::Box([x, y, w, h]));
    for line in lines(&interpolate()?) {
        draw(TooltipPart::Text(Line {
            text: line.to_owned(),
            position: [x.wrapping_add(3), baseline],
            align: 0,
            color: 0,
            shadow: false,
        }));
        baseline = baseline.wrapping_add(line_height.wrapping_add(1));
    }
    Ok(())
}

#[derive(Debug)]
pub struct Geometry {
    pub vertices: Vec<[f32; 2]>,
    pub thin: bool,
}
/// Coordinates are in original GL canvas space, including its 0.3f thin-line
/// offset. Alpha is intentionally absent: both original line calls are opaque.
pub fn line(c: &Component, origin: [i32; 2], canvas_height: i32) -> Geometry {
    let [x1, top] = origin;
    let x2 = x1.wrapping_add(c.width);
    let bottom = top.wrapping_add(c.height);
    let (y1, y2) = if c.line_flipped {
        (bottom, top)
    } else {
        (top, bottom)
    };
    if c.line_width == 1 {
        return Geometry {
            thin: true,
            vertices: vec![
                [x1 as f32 + 0.3, canvas_height as f32 - y1 as f32 - 0.3],
                [x2 as f32 + 0.3, canvas_height as f32 - y2 as f32 - 0.3],
            ],
        };
    }
    let dx = x2.wrapping_sub(x1);
    let dy = y2.wrapping_sub(y1);
    let distance = dx.wrapping_abs().max(dy.wrapping_abs());
    if distance == 0 {
        return Geometry {
            thin: false,
            vertices: Vec::new(),
        };
    }
    let mut sx = dx.wrapping_shl(16).wrapping_div(distance);
    let mut sy = dy.wrapping_shl(16).wrapping_div(distance);
    if sy <= sx {
        sx = sx.wrapping_neg();
    } else {
        sy = sy.wrapping_neg();
    }
    let px1 = c.line_width.wrapping_mul(sy) >> 17;
    let px2 = c.line_width.wrapping_mul(sy).wrapping_add(1) >> 17;
    let py1 = c.line_width.wrapping_mul(sx) >> 17;
    let py2 = c.line_width.wrapping_mul(sx).wrapping_add(1) >> 17;
    let a = [x1.wrapping_add(px1), y1.wrapping_add(py1)];
    let b = [x1.wrapping_sub(px2), y1.wrapping_sub(py2)];
    let c = [
        x1.wrapping_add(dx).wrapping_sub(px2),
        y1.wrapping_add(dy).wrapping_sub(py2),
    ];
    let d = [
        x1.wrapping_add(dx).wrapping_add(px1),
        y1.wrapping_add(dy).wrapping_add(py1),
    ];
    let corners = if sy <= sx { [d, c, b, a] } else { [a, b, c, d] };
    Geometry {
        thin: false,
        vertices: corners
            .map(|[x, y]| [x as f32, canvas_height.wrapping_sub(y) as f32])
            .to_vec(),
    }
}

pub const TOOLTIP_DELAY: i32 = 50;
pub fn tooltip_tick(timer: i32, selected: bool) -> i32 {
    if selected && timer < TOOLTIP_DELAY {
        timer.wrapping_add(1)
    } else if !selected && timer > 0 {
        timer - 1
    } else {
        timer
    }
}
