use rs09_client::ui::inventory_render::{self, Context, Error, Request};
use serde_json::{Value, json};
#[test]
fn original_type2_requests_positions_and_lazy_array_failures_match() {
    use sha2::{Digest, Sha256};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let rows: Value = serde_json::from_slice(
        &std::fs::read(root.join("fixtures/ui/inventory-render.json")).unwrap(),
    )
    .unwrap();
    for (name, hash) in rows["sources"].as_object().unwrap() {
        let original = std::fs::read_to_string(
            root.join("reference/client/client/src/main/java/rt4")
                .join(name),
        )
        .unwrap()
        .replace("\r\n", "\n");
        assert_eq!(
            format!("{:x}", Sha256::digest(original.as_bytes())),
            hash.as_str().unwrap()
        );
    }
    assert_eq!(rows["cases"].as_array().unwrap().len(), 2048);
    let mut failed_prefixes = 0;
    let mut calls = 0;
    for (index, row) in rows["cases"].as_array().unwrap().iter().enumerate() {
        let a: Vec<i32> = serde_json::from_value(row["arguments"].clone()).unwrap();
        let arrays: Vec<Option<Vec<i32>>> = serde_json::from_value(row["arrays"].clone()).unwrap();
        let c = rs09_client::ui::Component {
            kind: 2,
            id: a[14],
            base_width: a[0],
            base_height: a[1],
            inv_margin_x: a[2],
            inv_margin_y: a[3],
            obj_draw_text: a[13] != 0,
            inv_offset_x: arrays[0].clone(),
            inv_offset_y: arrays[1].clone(),
            obj_types: arrays[2].clone(),
            obj_counts: arrays[3].clone(),
            inv_sprite: arrays[4].clone(),
            ..Default::default()
        };
        let context = Context {
            origin: [a[4], a[5]],
            clip: [a[6], a[7], a[8], a[9]],
            dragged_slot: (a[10] >= 0).then_some(a[10]),
            selected_slot: (a[11] >= 0 && a[14] == a[15]).then_some(a[11]),
            pressed_slot: (a[12] >= 0).then_some(a[12]),
        };
        let mut log = Vec::new();
        let result = inventory_render::visit(&c, context, |request| match request {
            Request::Item {
                at,
                id,
                count,
                outline,
                shadow,
                draw_text,
                alpha,
                ..
            } => {
                log.push(json!([0, outline, id, i32::from(draw_text), count, shadow]));
                log.push(json!([2, at[0], at[1], alpha]));
            }
            Request::Background { slot, at } => {
                log.push(json!([1, slot]));
                if c.inv_sprite
                    .as_ref()
                    .unwrap()
                    .get(slot as usize)
                    .is_some_and(|&n| n >= 0)
                {
                    log.push(json!([2, at[0], at[1], 256]));
                }
            }
        });
        let error = result.err().map(|e| match e {
            Error::NullArray => "NullPointerException",
            Error::ArrayIndex => "ArrayIndexOutOfBoundsException",
        });
        let actual = json!([log, error]);
        assert_eq!(actual, row["expected"], "case{index}");
        failed_prefixes += usize::from(error.is_some());
        calls += log.len();
    }
    assert_eq!((calls, failed_prefixes), (6791, 112));
}
