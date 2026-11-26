use rs09_client::{
    animation::Sequence,
    ui::{Component, model},
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[path = "support/packs.rs"]
mod packs;
use packs::recorded;

fn fixture() -> Value {
    serde_json::from_str(include_str!("../../../fixtures/ui/models.json")).unwrap()
}
fn ints(v: &Value) -> Vec<i32> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_i64().unwrap() as i32)
        .collect()
}

#[test]
fn original_model_widget_camera_projection_and_transform_calls_match() {
    for (i, case) in fixture()["camera"].as_array().unwrap().iter().enumerate() {
        let x = ints(&case["args"]);
        let c = Component {
            if3: x[0] != 0,
            model_ortho: x[1] != 0,
            width: x[2],
            height: x[3],
            model_viewport_width: x[4],
            model_viewport_height: x[5],
            model_origin_x: x[6],
            model_origin_y: x[7],
            model_x_angle: x[8],
            model_y_angle: x[9],
            model_y_offset: x[10],
            model_zoom: x[11],
            model_x_offset: x[12],
            model_z_offset: x[13],
            model_view_angle: x[14],
            model_near_clip: x[15],
            model_transparent: x[16] != 0,
            ..Component::default()
        };
        let camera = model::Camera::new(&c, [x[17], x[18]], [x[20], x[21]], x[19]).unwrap();
        let bits: Vec<u32> = camera.projection.iter().map(|v| v.to_bits()).collect();
        assert_eq!(
            serde_json::to_value(bits).unwrap(),
            case["expected"][0],
            "projection{i}"
        );
        let ops: Vec<Vec<u32>> = camera
            .operations
            .iter()
            .map(|op| op.iter().map(|v| v.to_bits()).collect())
            .collect();
        assert_eq!(
            serde_json::to_value(ops).unwrap(),
            case["expected"][1],
            "transform{i}"
        );
        assert_eq!(
            serde_json::to_value(camera.viewport).unwrap(),
            case["expected"][2],
            "viewport{i}"
        );
        // The original draw restores the inherited raster clip after clearing
        // depth. Dialogue heads extend outside their 32px interaction bounds.
        use rs09_client::{
            pack::Pack,
            ui::state::{Key, State},
        };
        let manifest = br#"{"source":"clip oracle","files":{}}"#;
        let mut bytes = b"RS09VIS1".to_vec();
        bytes.extend((manifest.len() as u32).to_le_bytes());
        bytes.extend(manifest);
        let mut state = State::new(Pack::decode(bytes).unwrap());
        state.root = 1;
        state.loaded.insert(1);
        state.components.insert(
            Key::component(65536),
            Component {
                id: 65536,
                kind: 0,
                base_x: x[22],
                base_y: x[23],
                base_width: x[24] - x[22],
                base_height: x[25] - x[23],
                ..Component::default()
            },
        );
        state.components.insert(
            Key::component(65537),
            Component {
                id: 65537,
                kind: 6,
                overlayer: 65536,
                base_x: x[17] - x[22],
                base_y: x[18] - x[23],
                base_width: x[2],
                base_height: x[3],
                ..Component::default()
            },
        );
        let nodes = state.layout(x[20], x[21]).unwrap();
        let clip = nodes.iter().find(|n| n.key.id == 65537).unwrap().paint_clip;
        assert_eq!(
            serde_json::json!([clip.x, clip.y, clip.x + clip.w, clip.y + clip.h]),
            case["expected"][3],
            "raster clip{i}"
        );
    }
}

#[test]
fn original_model_widget_frame_boundaries_replay_hidden_and_rotation_match() {
    for (i, case) in fixture()["clocks"].as_array().unwrap().iter().enumerate() {
        let x = ints(&case["args"]);
        let delays = ints(&case["delays"]);
        let mut c = Component {
            kind: 6,
            if3: x[0] != 0,
            hidden: x[1] != 0,
            model_seq_id: x[3],
            active_model_seq_id: x[4],
            seq_frame: x[5],
            seq_next_frame: x[6],
            seq_cycle: x[7],
            model_x_angle: x[8],
            model_y_angle: x[9],
            model_rotation_speed: x[10],
            ..Component::default()
        };
        let seq = Sequence {
            frames: vec![0; delays.len()],
            delays: delays.iter().map(|&v| v as u16).collect(),
            replay_offset: (x[12] >= 0).then_some(x[12] as usize),
            ..Sequence::default()
        };
        let count = model::advance(&mut c, x[2] != 0, (x[13] != 0).then_some(&seq), x[11]).unwrap();
        let actual = [
            c.seq_frame,
            c.seq_next_frame,
            c.seq_cycle,
            c.model_x_angle,
            c.model_y_angle,
            count as i32,
        ];
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            case["expected"],
            "clock{i}"
        );
    }
}

#[test]
fn original_component_active_model_dispatch_and_partial_head_fields_match() {
    for (i, case) in fixture()["selection"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let x = ints(&case["args"]);
        let c = Component {
            model_type: x[0],
            model_id: x[1],
            active_model_type: x[2],
            active_model_id: x[3],
            model_frame_cycle: x[4],
            model_seq_id: x[5],
            active_model_seq_id: x[6],
            seq_frame: x[7],
            seq_next_frame: x[8],
            seq_cycle: x[9],
            ..Component::default()
        };
        let value = if let Some(c) = model::select(&c, x[10] != 0) {
            let id = if c.model_type == 3 { 0 } else { c.model_id };
            let detail = match c.model_type {
                4 => 10,
                7 => c.model_frame_cycle,
                _ => 0,
            };
            let clock = if c.model_seq_id == -1 {
                [-1; 3]
            } else {
                [c.seq_frame, c.seq_next_frame, c.seq_cycle]
            };
            serde_json::json!([
                c.model_type,
                id,
                detail,
                c.model_seq_id,
                clock[0],
                clock[1],
                clock[2]
            ])
        } else {
            Value::Null
        };
        assert_eq!(value, case["expected"], "model selection{i}");
    }
}

fn canonical(model: &rs09_client::model::Model) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend((model.vertices.len() as i32).to_be_bytes());
    out.extend((model.faces.len() as i32).to_be_bytes());
    for point in &model.vertices {
        for n in point {
            out.extend(n.to_be_bytes());
        }
    }
    for face in &model.faces {
        for &n in face {
            out.extend((n as i32).to_be_bytes());
        }
    }
    for n in &model.colors {
        out.extend(n.to_be_bytes());
    }
    out.extend(&model.alpha);
    out.extend((model.vertex_labels.len() as i32).to_be_bytes());
    out.extend(&model.vertex_labels);
    out.extend((model.face_labels.len() as i32).to_be_bytes());
    out.extend(&model.face_labels);
    for n in &model.textures {
        out.extend(n.to_be_bytes());
    }
    out
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[test]
fn every_cached_item_widget_geometry_normals_uvs_palette_resize_and_bounds_match() {
    use rs09_client::{
        model::Model,
        pack::Pack,
        surface, texture,
        ui::{item::Catalog, model_assets},
    };
    let corpus: Value =
        serde_json::from_str(include_str!("../../../fixtures/ui/item-models.json")).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ui_bytes = std::fs::read(recorded("ui-models.pack")).unwrap();
    let world_bytes =
        std::fs::read(root.join("assets/visual/live/lumbridge-minimap.pack")).unwrap();
    assert_eq!(hash(&ui_bytes), corpus["uiSha256"]);
    assert_eq!(hash(&world_bytes), corpus["worldSha256"]);
    let ui = Pack::decode(ui_bytes).unwrap();
    let world = Pack::decode(world_bytes).unwrap();
    let materials = texture::materials(world.get("materials").unwrap()).unwrap();
    let palette = std::array::from_fn(|i| corpus["palette"][i].as_u64().unwrap() as u16);
    let mut catalog = Catalog::default();
    let mut raw = BTreeMap::new();
    for (i, case) in corpus["cases"].as_array().unwrap().iter().enumerate() {
        let id = case["id"].as_i64().unwrap() as i32;
        let count = case["count"].as_i64().unwrap() as i32;
        let item = catalog.get(&ui, id).unwrap();
        let item = catalog.get(&ui, item.count_variant(count)).unwrap();
        let Ok(bytes) = ui.get(&format!("model/{}", item.model)) else {
            assert!(case["expected"].is_null());
            continue;
        };
        let original = raw
            .entry(item.model)
            .or_insert_with(|| Model::decode(bytes).unwrap());
        let m = model_assets::item_widget(&item, original, &palette, &materials);
        let mut out = Vec::new();
        out.extend((m.vertices.len() as i32).to_be_bytes());
        for v in &m.vertices {
            for n in v {
                out.extend(n.to_be_bytes());
            }
        }
        out.extend((m.faces.len() as i32).to_be_bytes());
        for (j, s) in m.surfaces.iter().enumerate() {
            let visible = surface::visible(&m, j, &materials);
            out.push(u8::from(visible));
            if !visible {
                continue;
            }
            out.extend(m.colors[j].to_be_bytes());
            out.extend(m.textures[j].to_be_bytes());
            out.push(m.alpha[j]);
            for k in 0..3 {
                for n in s.normals[k] {
                    out.extend((n as i16 as i32).to_be_bytes());
                }
                for n in s.uv[k] {
                    // DataOutputStream.writeFloat canonicalizes NaN payloads.
                    out.extend((if n.is_nan() { 0x7fc00000 } else { n.to_bits() }).to_be_bytes());
                }
            }
        }
        let order = surface::face_order(&m, &materials);
        out.extend((order.len() as i32).to_be_bytes());
        for n in order {
            out.extend((n as i32).to_be_bytes());
        }
        assert_eq!(
            serde_json::json!([
                item.id,
                m.lighting[0],
                m.lighting[1],
                model_assets::item_memory(&m),
                hash(&out)
            ]),
            case["expected"],
            "item widget{i} id{id} count{count}"
        );
    }
}

#[test]
fn real_kit_item_npc_and_player_head_geometry_matches_original_assembly() {
    use rs09_client::{
        appearance::IdentityKit,
        model::Model,
        npc::Definition,
        pack::Pack,
        ui::{
            item::{Catalog, ItemType},
            model_assets,
        },
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture: Value =
        serde_json::from_str(include_str!("../../../fixtures/ui/model-geometry.json")).unwrap();
    let ui_bytes = std::fs::read(recorded("ui-models.pack")).unwrap();
    let world_bytes =
        std::fs::read(root.join("assets/visual/live/lumbridge-minimap.pack")).unwrap();
    assert_eq!(hash(&ui_bytes), fixture["uiSha256"]);
    assert_eq!(hash(&world_bytes), fixture["worldSha256"]);
    let ui = Pack::decode(ui_bytes).unwrap();
    let world = Pack::decode(world_bytes).unwrap();
    let models: BTreeMap<u32, Model> = ui
        .manifest
        .files
        .keys()
        .filter_map(|name| {
            name.strip_prefix("model/").map(|id| {
                (
                    id.parse().unwrap(),
                    Model::decode(ui.get(name).unwrap()).unwrap(),
                )
            })
        })
        .collect();
    let palette: Vec<u16> = serde_json::from_value(fixture["palette"].clone()).unwrap();
    let palette: [u16; 256] = palette.try_into().unwrap();
    let mut items = Catalog::default();
    for case in fixture["cases"].as_array().unwrap() {
        let id = case["id"].as_i64().unwrap_or(0) as i32;
        let model = match case["kind"].as_str().unwrap() {
            "kit" => Some(
                IdentityKit::decode(ui.get(&format!("kit/{id}")).unwrap())
                    .unwrap()
                    .head_model(&models)
                    .unwrap(),
            ),
            "item" => {
                let (item, _) =
                    ItemType::decode(id, ui.get(&format!("item/{id}")).unwrap()).unwrap();
                model_assets::item_head(&item, &models, case["female"].as_bool().unwrap()).unwrap()
            }
            "npc" => {
                let d = Definition::decode(world.get(&format!("npc/{id}")).unwrap()).unwrap();
                if case["expected"] == "morph" {
                    assert!(d.children.is_some());
                    continue;
                }
                d.assemble_head(&models, &palette).unwrap()
            }
            "player" => Some(
                model_assets::player_head(
                    &ui,
                    &models,
                    &mut items,
                    serde_json::from_value(case["slots"].clone()).unwrap(),
                    serde_json::from_value(case["colors"].clone()).unwrap(),
                    case["female"].as_bool().unwrap(),
                )
                .unwrap(),
            ),
            "partial" => Some(
                model_assets::partial_head(
                    &ui,
                    &models,
                    serde_json::from_value(case["ids"].clone()).unwrap(),
                    serde_json::from_value(case["colors"].clone()).unwrap(),
                )
                .unwrap(),
            ),
            other => panic!("Unknown head fixture {other}"),
        };
        let actual = model
            .as_ref()
            .map_or("null".into(), |m| hash(&canonical(m)));
        assert_eq!(actual, case["expected"].as_str().unwrap(), "{case}");
    }
}

#[test]
fn original_entity_animation_readiness_flags_secondary_indices_and_passes_match() {
    use rs09_client::ui::model_animation::{self, Framesets};
    use std::cell::RefCell;
    struct Host {
        mode: i32,
        events: RefCell<Vec<Vec<i32>>>,
    }
    impl Framesets for Host {
        fn ready(&self, g: i32) -> bool {
            self.events.borrow_mut().push(vec![0, g]);
            self.mode != 2 && !(self.mode == 1 && g % 7 == 0)
        }
        fn alpha(&self, g: i32, f: i32) -> bool {
            self.events.borrow_mut().push(vec![1, g, f]);
            (g + f) % 3 == 0
        }
        fn color(&self, g: i32, f: i32) -> bool {
            self.events.borrow_mut().push(vec![2, g, f]);
            (g * 7 + f) % 5 == 0
        }
    }
    let fixture: Value =
        serde_json::from_str(include_str!("../../../fixtures/ui/model-animation.json")).unwrap();
    for (i, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let x = ints(&case["args"]);
        let h = Host {
            mode: x[6],
            events: RefCell::default(),
        };
        let seq = Sequence {
            frames: ints(&case["frames"])
                .into_iter()
                .map(|n| n as u32)
                .collect(),
            delays: ints(&case["delays"])
                .into_iter()
                .map(|n| n as u16)
                .collect(),
            secondary_frames: if case["secondary"].is_null() {
                Vec::new()
            } else {
                ints(&case["secondary"])
                    .into_iter()
                    .map(|n| n as u32)
                    .collect()
            },
            tween: x[3] != 0,
            transform_normals: x[5] != 0,
            ..Sequence::default()
        };
        let plan = model_animation::plan(&seq, x[0], x[1], x[2], x[4] != 0, &h).unwrap();
        let mut events = h.events.into_inner();
        events.push(vec![
            3,
            i32::from(plan.copy[0]),
            i32::from(plan.copy[1]),
            i32::from(plan.copy[2]),
        ]);
        for s in plan.steps {
            events.push(vec![
                4,
                s.group,
                s.frame,
                s.next_group.unwrap_or(-1),
                s.next_frame,
                s.delta,
                s.length,
                i32::from(s.normals),
            ]);
        }
        assert_eq!(
            serde_json::to_value(events).unwrap(),
            case["expected"],
            "entity animation{i}"
        );
    }
}

#[test]
fn original_model_update_readers_delayed_overwrites_protection_and_partial_writes_match() {
    use rs09_client::{
        pack::Pack,
        ui::{
            delayed::Queue,
            state::{Key, State},
            vm::Vm,
        },
    };
    use rs09_protocol::game::Frame;
    let corpus: Value =
        serde_json::from_str(include_str!("../../../fixtures/ui/model-updates.json")).unwrap();
    let bytes = std::fs::read(recorded("ui.pack")).unwrap();
    assert_eq!(hash(&bytes), corpus["uiSha256"]);
    let mut state = State::new(Pack::decode(bytes).unwrap());
    let pointer = corpus["pointer"].as_i64().unwrap() as i32;
    let key = Key::component(pointer);
    for (i, case) in corpus["cases"].as_array().unwrap().iter().enumerate() {
        let x = ints(&case["initial"]);
        let config = ints(&case["config"]);
        let c = Component {
            id: pointer,
            kind: 6,
            model_type: x[0],
            model_id: x[1],
            model_frame_cycle: x[2],
            model_seq_id: x[3],
            seq_frame: x[4],
            seq_next_frame: x[5],
            seq_cycle: x[6],
            model_x_angle: x[7],
            model_y_angle: x[8],
            model_zoom: x[9],
            model_x_offset: x[10],
            model_y_offset: x[11],
            model_z_offset: x[12],
            obj_id: x[13],
            obj_count: x[14],
            x: x[15],
            base_x: x[16],
            y: x[17],
            base_y: x[18],
            x_mode: x[19],
            y_mode: x[20],
            if3: config[0] != 0,
            base_width: config[1],
            model_viewport_width: config[2],
            ..Component::default()
        };
        state.components.clear();
        state.loaded.clear();
        state.loaded.insert(pointer >> 16);
        state.components.insert(key, c);
        state.delayed = Queue::default();
        state.verify_id = 0;
        state.model_redraws = 0;
        state.hooks.clear();
        let mut vm = Vm::default();
        vm.player_head_id = config[3];
        let mut now = 1000;
        let mut failures = 0;
        let mut snapshots = Vec::new();
        for command in case["commands"].as_array().unwrap() {
            let n = command.get(1).and_then(Value::as_i64).unwrap_or(0) as i32;
            match command[0].as_str().unwrap() {
                "T" => now += i64::from(n),
                "C" => state.delayed.client(n, pointer, now),
                "H" => vm.player_head_id = n,
                "S" => {
                    let raw: Vec<u8> =
                        serde_json::from_value(corpus["wires"][n as usize]["expected"][0].clone())
                            .unwrap();
                    let f = Frame {
                        opcode: raw[0],
                        payload: raw[1..].to_vec(),
                    };
                    if vm.apply_frame(&mut state, &f).is_err() {
                        failures += 1;
                    }
                }
                "P" => {
                    vm.poll_updates(&mut state, now).unwrap();
                    let c = state.get(key).unwrap();
                    snapshots.push(serde_json::json!([
                        [
                            c.model_type,
                            c.model_id,
                            c.model_frame_cycle,
                            c.model_seq_id,
                            c.seq_frame,
                            c.seq_next_frame,
                            c.seq_cycle,
                            c.model_x_angle,
                            c.model_y_angle,
                            c.model_zoom,
                            c.model_x_offset,
                            c.model_y_offset,
                            c.model_z_offset,
                            c.obj_id,
                            c.obj_count,
                            c.x,
                            c.base_x,
                            c.y,
                            c.base_y,
                            c.x_mode,
                            c.y_mode,
                            state.verify_id,
                            state.model_redraws as i32
                        ],
                        failures
                    ]));
                }
                op => panic!("Unknown model update command{op}"),
            }
        }
        assert_eq!(
            serde_json::to_value(snapshots).unwrap(),
            case["expected"],
            "model wire state{i}"
        );
    }
}
