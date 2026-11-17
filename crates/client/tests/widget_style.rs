mod ui {
    pub use rs09_client::ui::Component;
}
use rs09_client::ui::widget_style;
use serde_json::{Value, json};
#[test]
fn original_rectangle_text_colors_counts_and_interpolation_match() {
    let fixture: Value = serde_json::from_slice(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/ui/widget-style.json"),
        )
        .unwrap(),
    )
    .unwrap();
    use sha2::{Digest, Sha256};
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference/client/client/src/main/java/rt4");
    for (name, hash) in fixture["sources"].as_object().unwrap() {
        let text = std::fs::read_to_string(source.join(name))
            .unwrap()
            .replace("\r\n", "\n");
        assert_eq!(
            format!("{:x}", Sha256::digest(text.as_bytes())),
            hash.as_str().unwrap()
        );
    }
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 2048);
    for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let args = case["arguments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_i64().unwrap() as i32)
            .collect::<Vec<_>>();
        let c = ui::Component {
            kind: args[0],
            if3: args[1] != 0,
            filled: args[7] != 0,
            shadowed: args[9] != 0,
            color: args[10],
            active_color: args[11],
            over_color: args[12],
            active_over_color: args[13],
            obj_id: args[14],
            obj_count: args[15],
            width: args[17],
            height: args[18],
            halign: args[19],
            valign: args[20],
            vpadding: args[21],
            text: case["text"].as_str().unwrap().into(),
            active_text: case["activeText"].as_str().unwrap().into(),
            ..Default::default()
        };
        let active = args[2] != 0;
        let hovered = args[3] != 0;
        let waiting = args[4] != 0;
        let mut calls = Vec::<Value>::new();
        if c.kind == 3 {
            calls.push(json!(["Active"]));
            let color = widget_style::color(&c, active, hovered);
            let kind = if c.filled { "Fill" } else { "Border" };
            calls.push(if args[8] == 0 {
                json!([kind, 7, 11, c.width, c.height, color])
            } else {
                json!([
                    format!("{kind}Alpha"),
                    7,
                    11,
                    c.width,
                    c.height,
                    color,
                    256 - (args[8] & 255)
                ])
            });
            calls.push(json!(["Plugin", 19, 7, 11]));
        } else {
            calls.push(json!(["Font"]));
            if args[5] != 0 {
                calls.push(json!(["Active"]));
                let item = if c.if3 && c.obj_id != -1 {
                    calls.push(json!(["Item", c.obj_id]));
                    Some(widget_style::Item {
                        name: case["itemName"].as_str(),
                        stackable: args[16],
                    })
                } else {
                    None
                };
                let dns = (args[23] != 0).then_some(widget_style::Dns {
                    ip: args[22],
                    result: case["dnsResult"].as_str(),
                });
                let text = widget_style::text(
                    &c,
                    active,
                    hovered,
                    waiting,
                    item,
                    |i| {
                        calls.push(json!(["Run", i]));
                        args[24 + i]
                    },
                    dns,
                    "Please wait...",
                )
                .unwrap();
                let hex = text
                    .content
                    .chars()
                    .map(|c| format!("{:02x}", c as u8))
                    .collect::<String>();
                calls.push(json!([
                    "Text",
                    hex,
                    7,
                    11,
                    c.width,
                    c.height,
                    text.color,
                    if c.shadowed { 0 } else { -1 },
                    c.halign,
                    c.valign,
                    c.vpadding
                ]));
                calls.push(json!(["Plugin", 19, 7, 11]));
            } else if args[6] != 0 {
                calls.push(json!(["Redraw"]));
            }
        }
        assert_eq!(json!(calls), case["expected"], "style case {index}");
    }
    println!("PASS2048originalrectangletextformatinterpolationcases");
}
