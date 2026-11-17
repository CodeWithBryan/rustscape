use rs09_client::ui::{Component, widget_legacy, widget_style};
use serde_json::{Value, json};
use std::cell::RefCell;

fn hex(s: &str) -> String {
    s.chars().map(|c| format!("{:02x}", c as u8)).collect()
}
#[test]
fn original_legacy_item_lists_tooltips_and_gl_lines_match() {
    use sha2::{Digest, Sha256};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let f: Value = serde_json::from_slice(
        &std::fs::read(root.join("fixtures/ui/widget-legacy-draw.json")).unwrap(),
    )
    .unwrap();
    for (name, hash) in f["sources"].as_object().unwrap() {
        let s = std::fs::read_to_string(
            root.join("reference/client/client/src/main/java/rt4")
                .join(name),
        )
        .unwrap()
        .replace("\r\n", "\n");
        assert_eq!(
            format!("{:x}", Sha256::digest(s.as_bytes())),
            hash.as_str().unwrap()
        );
    }
    assert_eq!(f["cases"].as_array().unwrap().len(), 1536);
    for (index, row) in f["cases"].as_array().unwrap().iter().enumerate() {
        let a: Vec<i32> = serde_json::from_value(row["arguments"].clone()).unwrap();
        let c = Component {
            kind: a[0],
            base_width: a[1],
            base_height: a[2],
            inv_margin_x: a[3],
            inv_margin_y: a[4],
            color: a[5],
            halign: a[6],
            width: a[7],
            height: a[8],
            line_width: a[9],
            line_flipped: a[10] != 0,
            shadowed: a[11] != 0,
            obj_types: Some(serde_json::from_value(row["ids"].clone()).unwrap()),
            obj_counts: Some(serde_json::from_value(row["counts"].clone()).unwrap()),
            text: row["text"].as_str().unwrap().into(),
            ..Default::default()
        };
        let calls = RefCell::new(Vec::<Value>::new());
        let draw_text = |line: widget_legacy::Line| {
            calls.borrow_mut().push(json!([
                (["Left", "Center", "Right"][line.align as usize]),
                hex(&line.text),
                line.position[0],
                line.position[1],
                line.color,
                if line.shadow { 0 } else { -1 }
            ]))
        };
        let origin = [a[18], a[19]];
        match c.kind {
            7 => {
                calls.borrow_mut().push(json!(["Font"]));
                if a[12] != 0 {
                    widget_legacy::item_list(
                        &c,
                        origin,
                        |id| {
                            calls.borrow_mut().push(json!(["Item", id]));
                            Ok((if id % 2 == 0 { "Coins" } else { "Sword" }.into(), id % 3))
                        },
                        draw_text,
                    )
                    .unwrap();
                    calls.borrow_mut().push(json!([
                        "Plugin",
                        19,
                        origin[0].wrapping_add(c.inv_margin_x).wrapping_add(115),
                        origin[1].wrapping_add(c.inv_margin_y).wrapping_add(12)
                    ]));
                } else if a[13] != 0 {
                    calls.borrow_mut().push(json!(["Redraw"]));
                }
            }
            8 if a[16] == 50 && a[17] != 0 => {
                let mut counts = [0i32; 5];
                let mut plugin_pos = [0; 2];
                widget_legacy::tooltip(
                    &c,
                    origin,
                    [a[20], a[21]],
                    a[14],
                    || {
                        widget_style::interpolate(
                            &c.text,
                            |i| {
                                calls.borrow_mut().push(json!(["Run", i]));
                                let v = a[23 + i].wrapping_add(counts[i]);
                                counts[i] += 1;
                                v
                            },
                            None,
                        )
                    },
                    |s| {
                        calls.borrow_mut().push(json!(["Width", hex(s)]));
                        (s.chars().count() as i32).wrapping_mul(a[15])
                    },
                    |part| match part {
                        widget_legacy::TooltipPart::Box([x, y, w, h]) => {
                            calls
                                .borrow_mut()
                                .push(json!(["Fill", x, y, w, h, 16777120]));
                            calls.borrow_mut().push(json!(["Border", x, y, w, h, 0]));
                            plugin_pos = [x.wrapping_add(3), y.wrapping_add(a[14]).wrapping_add(2)];
                        }
                        widget_legacy::TooltipPart::Text(line) => {
                            plugin_pos[1] = line.position[1].wrapping_add(a[14]).wrapping_add(1);
                            draw_text(line);
                        }
                    },
                )
                .unwrap();
                calls
                    .borrow_mut()
                    .push(json!(["Plugin", 19, plugin_pos[0], plugin_pos[1]]));
            }
            9 => {
                let g = widget_legacy::line(&c, origin, a[22]);
                if !g.vertices.is_empty() {
                    calls.borrow_mut().push(json!(["Begin2D"]));
                    if g.thin {
                        calls.borrow_mut().push(json!(["Begin", 2]));
                    }
                    calls.borrow_mut().push(json!([
                        "Color",
                        c.color >> 16 & 255,
                        c.color >> 8 & 255,
                        c.color & 255
                    ]));
                    if !g.thin {
                        calls.borrow_mut().push(json!(["Begin", 6]));
                    }
                    for [x, y] in g.vertices {
                        calls.borrow_mut().push(json!([
                            "Vertex",
                            x.to_bits() as i32,
                            y.to_bits() as i32
                        ]));
                    }
                    calls.borrow_mut().push(json!(["End"]));
                }
                calls.borrow_mut().push(json!([
                    "Plugin",
                    19,
                    origin[0].wrapping_add(c.width),
                    if c.line_flipped {
                        origin[1].wrapping_add(c.height)
                    } else {
                        origin[1]
                    }
                ]));
            }
            _ => {}
        }
        assert_eq!(
            json!(*calls.borrow()),
            row["expected"],
            "legacy draw case {index}"
        );
    }
    for row in f["ticks"].as_array().unwrap() {
        let timer = row["timer"].as_i64().unwrap() as i32;
        let selected = row["selected"].as_bool().unwrap();
        let next = widget_legacy::tooltip_tick(timer, selected);
        let redraw = selected && timer < 50 && next == 50;
        assert_eq!(
            json!([
                next,
                if redraw {
                    vec![json!(["Redraw"])]
                } else {
                    vec![]
                }
            ]),
            row["expected"]
        );
    }
}
