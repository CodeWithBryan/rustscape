//! Scene drawing facts found by the frame comparison with the original
//! client (`oracle/refcompare`), checked against the pinned cache. Without
//! `assets/cache` these tests verify nothing and say so.
//!
//! Transcription tests: the expected values come from reading the cited
//! Java, not from running it.
use rs09_client::{
    cache_pack::Source, interaction::LocationType, loc_placement, model::Model, surface, texcoord,
    texture,
};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn cache() -> Option<PathBuf> {
    let path = root().join("assets/cache");
    if path.join("main_file_cache.dat2").exists() {
        Some(path)
    } else {
        eprintln!("assets/cache absent: nothing verified");
        None
    }
}

/// The tops of Lumbridge castle's walls (locations 33858 and 33864) are
/// faces of texture 147 whose texture triangle is edge-on to its second
/// axis. `GlModel`'s constructor (GlModel.java:489-495) then divides by
/// zero in float and `v` is NaN, which it hands to OpenGL as it is; the
/// original's frames show those faces in the texture's colour. The port
/// computes the same NaN and addresses it as the graphics card does.
#[test]
fn castle_wall_tops_have_nan_texture_coordinates_that_are_addressed_as_zero() {
    let Some(path) = cache() else {
        return;
    };
    let source = Source::open(&path).unwrap();
    let locations = source.archive(16).unwrap();
    let models = source.archive(7).unwrap();
    let materials = texture::materials(
        &source
            .archive(26)
            .unwrap()
            .file(0, 0, [0; 4])
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    for (id, expected) in [(33858u32, 2usize), (33864, 2)] {
        let files = locations.group(id >> 8, [0; 4]).unwrap().unwrap();
        let data = files.iter().find(|(f, _)| *f == (id & 255)).unwrap().1;
        let (_, d, _) = LocationType::decode_with_visual(data).unwrap();
        let loaded: std::collections::BTreeMap<u32, Model> = d
            .models
            .iter()
            .map(|&m| {
                let bytes = models.file(m, 0, [0; 4]).unwrap().unwrap();
                (m, Model::decode(&bytes).unwrap())
            })
            .collect();
        // A straight wall (shape 0) as SceneGraph.addLoc places it, turned
        // as on the castle's south side (rotation 1).
        let m = loc_placement::gl_model(&d, 0, 1, false, &materials, |x| loaded.get(&x))
            .unwrap()
            .unwrap();
        let surfaces = surface::prepare(&m, &materials);
        let top = m.vertices.iter().map(|v| v[1]).min().unwrap();
        let caps: Vec<usize> = (0..m.faces.len())
            .filter(|&i| {
                m.textures[i] == 147 && m.faces[i].iter().all(|&v| m.vertices[v][1] == top)
            })
            .collect();
        assert_eq!(caps.len(), expected, "location {id}: wall-top faces");
        for &i in &caps {
            assert!(
                surface::visible(&m, i, &materials),
                "location {id} face {i}"
            );
            for uv in surfaces[i].uv {
                assert!(uv[0].is_finite() && uv[1].is_nan(), "location {id}: {uv:?}");
                let gpu = texcoord::gpu(uv);
                assert!(gpu[0] == uv[0] && gpu[1] == 0.0, "location {id}: {gpu:?}");
            }
        }
    }
}

/// `SceneGraph.buildScene` (SceneGraph.java:993, 1075) has
/// `ShadowManager.addFloorShadow` stamp the floors of the upper levels with
/// the `floorshadows` sprite group. The scene built from the cache found the
/// group through the pack's named sprite list, which lacked it.
#[test]
fn the_cache_pack_names_the_floor_shadow_sprites() {
    let Some(path) = cache() else {
        return;
    };
    let pack = rs09_client::cache_pack::cache_pack(Path::new(&path), "test").unwrap();
    let ids: serde_json::Value =
        serde_json::from_slice(pack.get("minimap/assets").unwrap()).unwrap();
    let group = ids["floorshadows"].as_u64().expect("floorshadows listed");
    let sprites = rs09_client::shadow_rules::FloorSprites::decode(
        pack.get(&format!("sprite/{group}")).unwrap(),
    )
    .unwrap();
    assert!(!sprites.masks.is_empty());
    // The minimap's own entries are still there.
    for name in [
        "mapfunction",
        "mapdots",
        "mapflag",
        "compass",
        "hint_mapmarkers",
        "hint_mapedge",
    ] {
        assert!(ids[name].is_u64(), "{name}");
    }
}
