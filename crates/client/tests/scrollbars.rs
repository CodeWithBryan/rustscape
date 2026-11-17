use rs09_client::ui::scrollbar::{self, Drag, Input, Part};
use serde_json::{Value, json};

fn part(p: Part) -> Value {
    match p {
        Part::Sprite(frame, [x, y]) => json!([0, frame, x, y]),
        Part::Fill([x, y, w, h], c) => json!([1, x, y, w, h, c]),
        Part::Vertical([x, y, n], c) => json!([2, x, y, n, c]),
        Part::Horizontal([x, y, n], c) => json!([3, x, y, n, c]),
    }
}

#[test]
fn original_scrollbar_input_paint_failure_prefixes_and_gl_vertices_match() {
    use sha2::{Digest, Sha256};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture: Value =
        serde_json::from_slice(&std::fs::read(root.join("fixtures/ui/scrollbars.json")).unwrap())
            .unwrap();
    for (name, hash) in fixture["sources"].as_object().unwrap() {
        let text = std::fs::read_to_string(
            root.join("reference/client/client/src/main/java/rt4")
                .join(name),
        )
        .unwrap()
        .replace("\r\n", "\n");
        assert_eq!(
            format!("{:x}", Sha256::digest(text.as_bytes())),
            hash.as_str().unwrap()
        );
    }
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 1024);
    for (index, row) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let mut scroll = row["initial"][0].as_i64().unwrap() as i32;
        let mut drag = Drag {
            dragging: row["initial"][1].as_bool().unwrap(),
            margin: row["initial"][2].as_i64().unwrap() as i32,
        };
        let mut result = Vec::new();
        for call in row["calls"].as_array().unwrap() {
            let a: Vec<i32> = serde_json::from_value(call.clone()).unwrap();
            let mut redraws = 0;
            let error = drag
                .handle(
                    &mut scroll,
                    Input {
                        width: a[0],
                        point: [a[1], a[2]],
                        bar: [a[3], a[4]],
                        height: a[5],
                        range: a[6],
                        pressed: a[7] != 0,
                        wheel: a[8],
                    },
                    || redraws += 1,
                )
                .is_err();
            result.push(json!([scroll, drag.dragging, drag.margin, redraws, error]));
        }
        assert_eq!(json!(result), row["expected"], "input sequence {index}");
    }
    for (index, row) in fixture["draws"].as_array().unwrap().iter().enumerate() {
        let a: Vec<i32> = serde_json::from_value(row["arguments"].clone()).unwrap();
        let mut log = Vec::new();
        let error = scrollbar::paint(a[0], a[1], [a[2], a[3]], a[4], |p| {
            log.push(part(p));
            Ok(())
        })
        .is_err();
        assert_eq!(json!([error, log]), row["expected"], "paint prefix {index}");
    }
    for (index, row) in fixture["geometry"].as_array().unwrap().iter().enumerate() {
        let a: Vec<i32> = serde_json::from_value(row["arguments"].clone()).unwrap();
        let p = match a[1] {
            1 => Part::Fill([a[2], a[3], a[4], a[5]], a[6]),
            2 => Part::Vertical([a[2], a[3], a[4]], a[6]),
            _ => Part::Horizontal([a[2], a[3], a[4]], a[6]),
        };
        let (g, c) = scrollbar::geometry(p, a[0]).unwrap();
        let mut log = vec![
            json!([4, if g.thin { 1 } else { 6 }]),
            json!([5, c >> 16 & 255, c >> 8 & 255, c & 255]),
        ];
        log.extend(
            g.vertices
                .iter()
                .map(|[x, y]| json!([6, x.to_bits() as i32, y.to_bits() as i32])),
        );
        log.push(json!([7]));
        assert_eq!(json!(log), row["expected"], "GL geometry {index}");
    }
}
