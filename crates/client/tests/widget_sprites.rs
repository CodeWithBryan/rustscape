use rs09_client::ui::{sprite, widget_sprite};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
fn sha(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
#[test]
fn original_widget_sprite_pixels_alpha_offsets_keys_and_resized_bounds_match() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let f: Value =
        serde_json::from_slice(&fs::read(root.join("fixtures/ui/widget-sprites.json")).unwrap())
            .unwrap();
    let blob = fs::read(root.join("fixtures/ui/sprites.bin")).unwrap();
    for (name, hash) in f["sources"].as_object().unwrap() {
        let b = fs::read(
            root.join("reference/client/client/src/main/java/rt4")
                .join(name),
        )
        .unwrap();
        let b = String::from_utf8(b).unwrap().replace("\r\n", "\n");
        assert_eq!(sha(b.as_bytes()), hash.as_str().unwrap());
    }
    assert_eq!(f["cases"].as_array().unwrap().len(), 6042);
    assert_eq!(sha(&blob), f["sourceSpritesSha256"].as_str().unwrap());
    for (i, c) in f["cases"].as_array().unwrap().iter().enumerate() {
        let a = &c["asset"];
        let owned;
        if let Some(value) = c.get("raw") {
            owned = value
                .as_str()
                .unwrap()
                .as_bytes()
                .chunks_exact(2)
                .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
                .collect::<Vec<_>>();
        } else {
            let start = a["offset"].as_u64().unwrap() as usize;
            owned = blob[start..start + a["size"].as_u64().unwrap() as usize].to_vec();
        }
        let raw = &owned;
        assert_eq!(sha(raw), c["rawSha256"].as_str().unwrap());
        let sheet = sprite::Sheet::decode(raw).unwrap();
        let o = &c["options"];
        let options = widget_sprite::Options {
            has_alpha: o["has_alpha"].as_bool().unwrap(),
            h_flip: o["h_flip"].as_bool().unwrap(),
            v_flip: o["v_flip"].as_bool().unwrap(),
            outline: o["outline"].as_i64().unwrap() as i32,
            shadow: o["shadow"].as_i64().unwrap() as i32,
        };
        let id = c["ids"][usize::from(c["active"].as_bool().unwrap())]
            .as_i64()
            .unwrap() as i32;
        if c["expected"].is_null() {
            assert_eq!(id, -1);
            continue;
        }
        let image = widget_sprite::Image::prepare(&sheet, options)
            .unwrap_or_else(|e| panic!("case{i} asset{} {e}", a["name"]));
        let id = c["ids"][usize::from(c["active"].as_bool().unwrap())]
            .as_i64()
            .unwrap() as i32;
        let pixels: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_be_bytes()).collect();
        assert_eq!(
            json!({"key":options.key(id).to_string(),"inner":image.inner,"offset":image.offset,"size":image.size,"alpha":image.alpha,"pixelsSha256":sha(&pixels),"rgbaSha256":sha(&image.rgba()),"backingRgbaSha256":sha(&image.backing_rgba().unwrap().1),"resized":image.resized_rect(serde_json::from_value(c["rect"].clone()).unwrap()).unwrap()}),
            c["expected"],
            "case{i} asset{}",
            a["name"]
        );
    }
    println!("PASS6042originalwidgetimagecases");
}

#[test]
fn complete_original_type5_gl_draw_routing_and_callback_arguments_match() {
    use widget_sprite::{Draw, DrawInput, draw_commands};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let f: Value =
        serde_json::from_slice(&fs::read(root.join("fixtures/ui/widget-draw.json")).unwrap())
            .unwrap();
    for (name, hash) in f["sources"].as_object().unwrap() {
        let b = fs::read(
            root.join("reference/client/client/src/main/java/rt4")
                .join(name),
        )
        .unwrap();
        assert_eq!(
            sha(String::from_utf8(b)
                .unwrap()
                .replace("\r\n", "\n")
                .as_bytes()),
            hash.as_str().unwrap()
        );
    }
    assert_eq!(f["cases"].as_array().unwrap().len(), 2176);
    for (i, c) in f["cases"].as_array().unwrap().iter().enumerate() {
        let input = DrawInput {
            if3: c["if3"].as_bool().unwrap(),
            tiling: c["tiling"].as_bool().unwrap(),
            angle: c["angle"].as_i64().unwrap() as i32,
            inner: serde_json::from_value(c["inner"].clone()).unwrap(),
            size: serde_json::from_value(c["size"].clone()).unwrap(),
            rect: serde_json::from_value(c["rect"].clone()).unwrap(),
            alpha: c["alpha"].as_i64().unwrap() as i32,
            active: c["active"].as_bool().unwrap(),
            clip: serde_json::from_value(c["clip"].clone()).unwrap(),
        };
        let mut calls = vec![];
        draw_commands(input, |draw| {
            calls.push(match draw {
                Draw::Sprite(v) => json!(["Sprite", [i32::from(v)]]),
                Draw::Clip(v) => json!(["Clip", v]),
                Draw::Natural(v) => json!(["Natural", v]),
                Draw::AlphaNatural(v) => json!(["AlphaNatural", v]),
                Draw::Resized(v) => json!(["Resized", v]),
                Draw::AlphaResize(v) => json!(["AlphaResize", v]),
                Draw::Rotated(v) => json!(["Rotated", v]),
                Draw::Tiled(v) => json!(["Tiled", v]),
                Draw::AlphaTile(v) => json!(["AlphaTile", v]),
                Draw::Plugin(v) => json!(["Plugin", v]),
            })
        })
        .unwrap();
        assert_eq!(json!(calls), c["expected"], "type5 case{i}");
    }
}

#[test]
fn excessive_widget_tile_work_returns_an_explicit_error() {
    use widget_sprite::{Draw, DrawInput, draw_commands};
    let mut draws = 0;
    let result = draw_commands(
        DrawInput {
            if3: true,
            tiling: true,
            angle: 0,
            inner: [3, 3],
            size: [3, 3],
            rect: [0, 0, 20000, 20000],
            alpha: 0,
            active: false,
            clip: [0, 0, 1280, 720],
        },
        |d| {
            if !matches!(d, Draw::Sprite(_) | Draw::Clip(_)) {
                draws += 1;
            }
        },
    );
    assert!(result.is_err());
    assert_eq!(draws, 0);
}
