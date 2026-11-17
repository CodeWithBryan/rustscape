use rs09_client::minimap::{self, Tile, View};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[test]
fn original_hd_edge_arrow_fixed_pivot_and_gl_transform_match() {
    let f: Value =
        serde_json::from_str(include_str!("../../../fixtures/minimap/edge.json")).unwrap();
    let data = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../reference/client/client/src/main/java/rt4/GlSprite.java"),
    )
    .unwrap()
    .into_iter()
    .filter(|&v| v != b'\r')
    .collect::<Vec<_>>();
    assert_eq!(
        format!("{:x}", Sha256::digest(data)),
        f["sources"]["GlSprite"].as_str().unwrap()
    );
    for c in f["cases"].as_array().unwrap() {
        let n: Vec<i32> = serde_json::from_value(c["args"].clone()).unwrap();
        assert_eq!(
            json!(minimap::edge_transform([n[0], n[1]], n[2]).map(|v| v.to_bits() as i32)),
            c["expected"],
            "{c}"
        );
    }
    assert_eq!(f["cases"].as_array().unwrap().len(), 512);
}

#[test]
fn overlaid_controls_clear_special_component_actions_in_draw_order() {
    use rs09_client::{
        interaction::{Menu, MenuEntry},
        pack::Pack,
        ui::{
            Component,
            state::{DrawNode, Key, Rect, State},
        },
    };
    let manifest = br#"{"source":"controlled overlay regression","files":{}}"#;
    let mut bytes = b"RS09VIS1".to_vec();
    bytes.extend((manifest.len() as u32).to_le_bytes());
    bytes.extend(manifest);
    let mut ui = State::new(Pack::decode(bytes).unwrap());
    let map = Key::component(1);
    let overlay = Key::component(2);
    ui.components.insert(
        map,
        Component {
            id: 1,
            client_code: 1338,
            ..Default::default()
        },
    );
    ui.components.insert(
        overlay,
        Component {
            id: 2,
            kind: 0,
            no_click_through: true,
            ..Default::default()
        },
    );
    let rect = Rect {
        x: 0,
        y: 0,
        w: 100,
        h: 100,
    };
    let nodes = [
        DrawNode {
            key: map,
            rect,
            clip: rect,
            paint_clip: rect,
            subtree_end: 0,
        },
        DrawNode {
            key: overlay,
            rect,
            clip: rect,
            paint_clip: rect,
            subtree_end: 0,
        },
    ];
    let run = |ui: &mut State, nodes: &[DrawNode]| {
        let mut menu = Menu::default();
        menu.add(MenuEntry {
            option: "Cancel".into(),
            target: String::new(),
            action: 1005,
            key: 0,
            args: [0; 2],
            cursor: -1,
        });
        assert!(ui.menu_at_with(nodes, [50, 50], &mut menu, |c, _, m| {
            if c.client_code != 1338 {
                return None;
            }
            m.add(MenuEntry {
                option: "Walk here".into(),
                target: String::new(),
                action: 60,
                key: 1,
                args: [1, 2],
                cursor: -1,
            });
            Some(true)
        }));
        menu.entries
            .into_iter()
            .map(|e| e.action)
            .collect::<Vec<_>>()
    };
    assert_eq!(run(&mut ui, &nodes), vec![1005]);
    assert_eq!(run(&mut ui, &nodes[..1]), vec![1005, 60]);
    assert_eq!(run(&mut ui, &[nodes[1], nodes[0]]), vec![1005, 60]);
}

#[test]
fn original_minimap_walk_prefix_and_click_trailer_match() {
    let f: Value =
        serde_json::from_str(include_str!("../../../fixtures/minimap/wire.json")).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (name, expected) in f["sources"].as_object().unwrap() {
        let data = std::fs::read(root.join(format!(
            "reference/client/client/src/main/java/rt4/{name}.java"
        )))
        .unwrap()
        .into_iter()
        .filter(|&v| v != b'\r')
        .collect::<Vec<_>>();
        assert_eq!(
            format!("{:x}", Sha256::digest(data)),
            expected.as_str().unwrap()
        );
    }
    for c in f["cases"].as_array().unwrap() {
        let n: Vec<i32> = serde_json::from_value(c["args"].clone()).unwrap();
        let route = rs09_client::walking::Route {
            turns: serde_json::from_value(c["turns"].clone()).unwrap(),
            steps: Vec::new(),
            approximate: n[10] != 0,
        };
        let bytes = View {
            yaw: n[5],
            angle_offset: n[6],
            zoom_offset: n[7],
        }
        .walk_packet(&route, [n[1], n[2]], n[0] != 0, [n[3], n[4]], [n[8], n[9]]);
        let actual = bytes.iter().map(|v| format!("{v:02x}")).collect::<String>();
        assert_eq!(actual, c["expected"].as_str().unwrap(), "{c}");
    }
    assert_eq!(f["cases"].as_array().unwrap().len(), 1024);
}

#[test]
fn original_minimap_packets_hints_morphs_equipment_and_jitter_match() {
    let f: Value =
        serde_json::from_str(include_str!("../../../fixtures/minimap/state.json")).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (name, expected) in f["sources"].as_object().unwrap() {
        let data = std::fs::read(root.join(format!(
            "reference/client/client/src/main/java/rt4/{name}.java"
        )))
        .unwrap()
        .into_iter()
        .filter(|&v| v != b'\r')
        .collect::<Vec<_>>();
        assert_eq!(
            format!("{:x}", Sha256::digest(data)),
            expected.as_str().unwrap()
        );
    }
    let mut state = minimap::State {
        head_hint_count: 3,
        ..Default::default()
    };
    for c in f["cases"].as_array().unwrap() {
        let n: Vec<i32> = serde_json::from_value(c["args"].clone()).unwrap();
        let actual = match c["kind"].as_str().unwrap() {
            "P" => {
                let hex = c["extra"].as_str().unwrap();
                let bytes = (0..hex.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                    .collect();
                assert!(
                    state
                        .apply(&rs09_protocol::game::Frame {
                            opcode: n[0] as u8,
                            payload: bytes
                        })
                        .unwrap()
                );
                json!([
                    state.state,
                    state.hints.map(|hint| hint.map(|h| [
                        h.kind,
                        h.sprite as i32,
                        h.fine[0],
                        h.fine[1],
                        h.tile[0],
                        h.tile[1],
                        h.height,
                        h.actor,
                        h.model
                    ]))
                ])
            }
            "H" => {
                let view = View {
                    yaw: n[2],
                    angle_offset: n[3],
                    zoom_offset: n[4],
                };
                let projected =
                    view.hint([n[0], n[1]], [n[7], n[8]], [n[5], n[6]])
                        .map(|h| match h {
                            minimap::HintPosition::Icon([x, y]) => vec![0, x, y],
                            minimap::HintPosition::Edge {
                                center: [x, y],
                                angle,
                            } => vec![1, x, y, angle],
                        });
                json!([projected, minimap::hints_visible(n[9])])
            }
            "R" => {
                let mut loc = rs09_client::interaction::LocationType::decode(&[0]).unwrap();
                loc.morph_varbit = n[0];
                loc.morph_varp = n[1];
                loc.children = Some(serde_json::from_value(c["extra"].clone()).unwrap());
                json!(loc.select_morph(|bit, _| if bit { n[2] } else { n[3] }))
            }
            "E" => json!(minimap::equipment_ids((0..n[0] as usize).map(|i| (
                i as i32,
                n[1 + i * 2],
                n[2 + i * 2]
            )))),
            "J" => json!(minimap::jitter(
                serde_json::from_value(c["extra"].clone()).unwrap()
            )),
            _ => panic!("unknown {c}"),
        };
        assert_eq!(actual, c["expected"], "{c}");
    }
    assert_eq!(f["cases"].as_array().unwrap().len(), 3072);
    let before = serde_json::to_value(&state).unwrap();
    assert!(
        state
            .apply(&rs09_protocol::game::Frame {
                opcode: 217,
                payload: vec![1, 0]
            })
            .is_err()
    );
    assert_eq!(serde_json::to_value(&state).unwrap(), before);
}

#[test]
fn original_minimap_location_floor_and_mapscene_cache_metadata_match() {
    let f: Value =
        serde_json::from_str(include_str!("../../../fixtures/minimap/metadata.json")).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (name, expected) in f["sources"].as_object().unwrap() {
        let name = if name.ends_with(".java") {
            name.clone()
        } else {
            format!("{name}.java")
        };
        let data =
            std::fs::read(root.join(format!("reference/client/client/src/main/java/rt4/{name}")))
                .unwrap()
                .into_iter()
                .filter(|&v| v != b'\r')
                .collect::<Vec<_>>();
        assert_eq!(
            format!("{:x}", Sha256::digest(data)),
            expected.as_str().unwrap()
        );
    }
    for c in f["cases"].as_array().unwrap() {
        let text = c["raw"].as_str().unwrap();
        let raw = (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect::<Vec<_>>();
        let actual = match c["kind"].as_str().unwrap() {
            "L" => {
                let d = rs09_client::interaction::LocationType::decode(&raw).unwrap();
                json!([
                    d.map_function,
                    d.map_scene,
                    d.map_scene_rotated,
                    d.map_scene_angle,
                    d.size[0],
                    d.size[1],
                    d.interactable,
                    d.morph_varbit,
                    d.morph_varp,
                    d.children
                ])
            }
            "O" => {
                let d = rs09_client::floor::Overlay::decode(&raw).unwrap();
                json!([
                    d.color,
                    d.secondary_color,
                    d.texture,
                    d.scale,
                    d.blend,
                    d.water_color,
                    d.water_opacity
                ])
            }
            "M" => {
                let d = minimap::MapScene::decode(&raw).unwrap();
                json!([d.sprite, d.tint, d.stretch])
            }
            _ => panic!("unknown {c}"),
        };
        assert_eq!(actual, c["expected"], "{c}");
    }
}

#[test]
fn original_minimap_scenery_indexed_pixels_bridge_pass_and_colors_match() {
    use rs09_client::{floor::Overlay, texture::Material};
    let f: Value =
        serde_json::from_str(include_str!("../../../fixtures/minimap/scene.json")).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (name, expected) in f["sources"].as_object().unwrap() {
        let data = std::fs::read(root.join(format!(
            "reference/client/client/src/main/java/rt4/{name}.java"
        )))
        .unwrap()
        .into_iter()
        .filter(|&v| v != b'\r')
        .collect::<Vec<_>>();
        assert_eq!(
            format!("{:x}", Sha256::digest(data)),
            expected.as_str().unwrap()
        );
    }
    let palette = (0..65536).collect::<Vec<u32>>();
    let hash = |pixels: Vec<u32>| {
        let bytes = pixels
            .into_iter()
            .flat_map(u32::to_be_bytes)
            .collect::<Vec<_>>();
        json!(format!("{:x}", Sha256::digest(bytes)))
    };
    for c in f["cases"].as_array().unwrap() {
        let n: Vec<i32> = serde_json::from_value(c["args"].clone()).unwrap();
        let actual = match c["kind"].as_str().unwrap() {
            "F" => {
                let mut pixels = vec![0x123456; 512 * 512];
                let tile = [n[0] as usize, n[1] as usize];
                minimap::stroke(
                    &mut pixels,
                    tile,
                    n[2] as u8,
                    n[3] as u8,
                    n[4] as u8,
                    n[5] != 0,
                    [n[6] as u32, n[7] as u32],
                );
                let base = (103 - tile[1]) * 2048 + tile[0] * 4 + 24624;
                json!(
                    (0..16)
                        .map(|i| pixels[base + i / 4 * 512 + i % 4])
                        .collect::<Vec<_>>()
                )
            }
            "S" => {
                let text = c["pixels"].as_str().unwrap();
                let indices = (0..text.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
                    .collect();
                let mut s = minimap::Indexed {
                    size: [n[0], n[1]],
                    inner: [n[2], n[3]],
                    offset: [n[4], n[5]],
                    indices,
                    palette: serde_json::from_value(c["palette"].clone()).unwrap(),
                };
                s.adjust_palette([n[6], n[7], n[8]]);
                if n[9] == 1 {
                    s.inner = s.size;
                }
                for _ in 0..n[10] {
                    s.rotate();
                }
                if n[11] == 1 {
                    s.trim();
                }
                let mut pixels = vec![0x123456; 512 * 512];
                s.scaled(&mut pixels, [n[12], n[13]], [n[14], n[15]], n[16] as u32)
                    .unwrap_or_else(|e| panic!("{c}: {e}"));
                hash(pixels)
            }
            "B" => {
                let seed = n[0] as usize;
                let z = |p: usize, x: usize, y: usize| (seed + p * 71 + x * 17 + y * 31) & 65535;
                hash(minimap::background(
                    n[1] as usize,
                    |p, x, y| {
                        let z = z(p, x, y);
                        if z % 7 == 0 {
                            return None;
                        }
                        let shape = z % 13;
                        let plain = if z % 5 == 0 { 0 } else { z + 1 };
                        Some(if shape < 2 {
                            Tile {
                                shape,
                                rotation: 0,
                                underlay: plain as u32,
                                overlay: plain as u32,
                            }
                        } else {
                            Tile {
                                shape,
                                rotation: z & 3,
                                underlay: if z % 5 == 0 { 0 } else { (z + 2) as u32 },
                                overlay: if z % 3 == 0 { 0 } else { (z + 3) as u32 },
                            }
                        })
                    },
                    |p, x, y| {
                        let z = z(p, x, y);
                        if z % 13 == 0 {
                            8
                        } else if z % 11 == 0 {
                            24
                        } else {
                            0
                        }
                    },
                ))
            }
            "C" => {
                let material = Material {
                    repeating: n[5] != 0,
                    opaque: false,
                    low_detail: false,
                    flipped: false,
                    speed: 0,
                    brightness: 0,
                    material_type: 0,
                    animation_type: 0,
                    average_color: n[6] as u16,
                };
                let overlay = Overlay {
                    texture: n[2],
                    color: n[3],
                    secondary_color: n[4],
                    ..Overlay::default()
                };
                json!(minimap::floor_rgb(
                    (n[0] >= 0).then_some(n[0] as u16),
                    Some(&overlay),
                    &[Some(material)],
                    [n[7], n[8]],
                    &palette
                ))
            }
            _ => panic!("unknown {c}"),
        };
        assert_eq!(actual, c["expected"], "{c}");
    }
    assert_eq!(f["cases"].as_array().unwrap().len(), 1736);
}

#[test]
fn original_minimap_raster_masks_icons_clicks_and_hd_uvs_match() {
    let f: Value =
        serde_json::from_str(include_str!("../../../fixtures/minimap/oracle.json")).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (name, expected) in f["sources"].as_object().unwrap() {
        let data = std::fs::read(root.join(format!(
            "reference/client/client/src/main/java/rt4/{name}.java"
        )))
        .unwrap()
        .into_iter()
        .filter(|&v| v != b'\r')
        .collect::<Vec<_>>();
        assert_eq!(
            format!("{:x}", Sha256::digest(data)),
            expected.as_str().unwrap(),
            "{name}"
        );
    }
    for c in f["cases"].as_array().unwrap() {
        let n: Vec<i32> = serde_json::from_value(c["args"].clone()).unwrap();
        let actual = match c["kind"].as_str().unwrap() {
            "T" => {
                let mut pixels = vec![n[4] as u32; 512 * 512];
                Tile {
                    shape: n[0] as usize,
                    rotation: n[1] as usize,
                    underlay: n[2] as u32,
                    overlay: n[3] as u32,
                }
                .draw(&mut pixels, 0);
                json!(
                    (0..16)
                        .map(|i| pixels[i / 4 * 512 + i % 4])
                        .collect::<Vec<_>>()
                )
            }
            "M" => {
                let text = c["extra"].as_str().unwrap();
                let pixels = (0..text.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
                    .collect::<Vec<_>>();
                json!(
                    minimap::click_mask(n[0] as usize, n[1] as usize, &pixels)
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                )
            }
            "I" => json!(
                View {
                    yaw: n[4],
                    angle_offset: n[5],
                    zoom_offset: n[6]
                }
                .icon([n[0], n[1]], [n[2], n[3]], [n[7], n[8]])
            ),
            "C" => {
                let raw: Vec<i32> = serde_json::from_value(c["extra"].clone()).unwrap();
                let mask = raw
                    .chunks_exact(2)
                    .map(|c| [c[0], c[1]])
                    .collect::<Vec<_>>();
                json!(
                    View {
                        yaw: n[2],
                        angle_offset: n[3],
                        zoom_offset: n[4]
                    }
                    .click([n[0], n[1]], &mask, [n[5], n[6]], [n[7], n[8]])
                )
            }
            "U" => json!(
                minimap::rotated_uv([n[0], n[1]], [n[2], n[3]], n[4], n[5], [n[6], n[7]])
                    .into_iter()
                    .flatten()
                    .map(|v| v.to_bits() as i32)
                    .collect::<Vec<_>>()
            ),
            _ => panic!("unknown case {c}"),
        };
        assert_eq!(actual, c["expected"], "{c}");
    }
    assert_eq!(f["cases"].as_array().unwrap().len(), 3744);
}
