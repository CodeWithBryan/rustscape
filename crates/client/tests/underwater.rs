//! JVM differential for the underwater scene: what the original loads from a
//! region's `um`/`ul` map files and builds from them, against
//! `rs09_client::underwater` and `gl_tile` with its water arguments.
//!
//! `fixtures/underwater/oracle.json` holds SHA-256 digests of what the
//! original code produced over seven 104 x 104 scenes cut from the pinned cache;
//! `examples/terrain_oracle.rs` generates it (its second pass) and
//! `fixtures/underwater/provenance.json` lists the original sources, the scope
//! and every region of the cache that has underwater files. Here the Rust port
//! runs over the same scenes and must reproduce every byte. Without
//! `assets/cache` the tests are skipped.
#[path = "terrain_support/mod.rs"]
mod terrain_support;

use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::Path, sync::OnceLock};
use terrain_support::underwater as support;

struct Run {
    digests: BTreeMap<String, String>,
    expected: BTreeMap<String, String>,
    stats: BTreeMap<String, support::Stats>,
    regions: Vec<(i32, i32, bool, bool)>,
    listed: usize,
}

fn run() -> Option<&'static Run> {
    static RUN: OnceLock<Option<Run>> = OnceLock::new();
    RUN.get_or_init(|| {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let source = match terrain_support::open(&root) {
            Ok(source) => source,
            Err(error) => {
                eprintln!("skipped: no cache or map keys ({error})");
                return None;
            }
        };
        let read = |name: &str| -> serde_json::Value {
            serde_json::from_slice(
                &std::fs::read(root.join("fixtures/underwater").join(name)).unwrap(),
            )
            .unwrap()
        };
        let fixture = read("oracle.json");
        let hex = |bytes: &[u8]| format!("{:x}", Sha256::digest(bytes));
        let scenes: Vec<_> = support::SCENES
            .iter()
            .map(|(name, base)| support::scene(&source, name, *base).unwrap())
            .collect();
        assert_eq!(
            hex(&support::input(&source, &scenes)),
            fixture["inputSha256"].as_str().unwrap(),
            "the scenes cut from the cache are not the ones the oracle was given; \
             if a decoder changed on purpose, regenerate with \
             `cargo run -p rs09-client --example terrain_oracle -- --underwater`"
        );
        let mut digests = BTreeMap::new();
        let mut stats = BTreeMap::new();
        for scene in &scenes {
            let (sections, scene_stats) = support::sections(&source, scene);
            stats.insert(scene.name.clone(), scene_stats);
            for (name, bytes) in sections {
                digests.insert(format!("{}/{name}", scene.name), hex(&bytes));
            }
        }
        let expected = fixture["sections"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, entry)| (name.clone(), entry["sha256"].as_str().unwrap().to_owned()))
            .collect();
        Some(Run {
            digests,
            expected,
            stats,
            regions: support::regions(&source),
            listed: read("provenance.json")["underwaterRegions"]["count"]
                .as_u64()
                .unwrap() as usize,
        })
    })
    .as_ref()
}

/// Every fixture section named in `names` is reproduced, for every scene.
fn reproduces(names: &[&str]) -> Option<&'static Run> {
    let run = run()?;
    let wanted: Vec<_> = run
        .expected
        .keys()
        .filter(|name| names.contains(&name.split('/').nth(1).unwrap()))
        .collect();
    assert_eq!(wanted.len(), names.len() * support::SCENES.len());
    let wrong: Vec<_> = wanted
        .into_iter()
        .filter(|name| run.digests.get(*name) != run.expected.get(*name))
        .collect();
    assert!(wrong.is_empty(), "differs from the original: {wrong:?}");
    Some(run)
}

#[test]
fn underwater_maps_are_read_as_the_original_reads_them() {
    // Heights below the surface, the four tile arrays, the shade of regions
    // without a map, and the lightness and blended colours built from them.
    let Some(run) = reproduces(&["heights", "tiles", "shadow", "light", "colors"]) else {
        return;
    };
    for (name, stats) in &run.stats {
        assert!(stats.deep > 400, "{name}: {stats:?}");
    }
    // Lumbridge's eastern regions have no underwater map and are cleared;
    // the cave is off the surface map, where nothing is.
    assert_eq!(run.stats["lumbridge"].cleared, 2);
    assert_eq!(run.stats["cave"].cleared, 0);
    // The cache sweep the provenance records.
    assert_eq!(run.regions.len(), run.listed);
    assert_eq!(run.regions.len(), 597);
    assert!(
        run.regions
            .iter()
            .all(|&(_, _, terrain, locations)| terrain && locations)
    );
    assert!(run.regions.contains(&(50, 50, true, true)));
}

#[test]
fn water_data_follows_the_surface_and_underwater_scenery() {
    let Some(run) = reproduces(&["water0", "water1"]) else {
        return;
    };
    for (name, stats) in &run.stats {
        assert!(stats.watered > 500, "{name}: {stats:?}");
    }
    assert!(run.stats["lumbridge"].spread > 0 && run.stats["west"].spread > 0);
}

#[test]
fn the_underwater_floor_is_the_originals_vertex_for_vertex() {
    // The groups with their 40-byte vertices (the underwater coordinate
    // last), every face, and the diagonal of every plain tile.
    let Some(run) = reproduces(&["plain", "diag"]) else {
        return;
    };
    for (name, stats) in &run.stats {
        assert!(stats.faces > 900 && stats.sunk > 1000, "{name}: {stats:?}");
    }
    // Tiles cut by an overlay shape (the shore), whose flagged vertices stay
    // at the surface.
    assert!(run.stats.values().map(|stats| stats.cut).sum::<usize>() > 4000);
}

#[test]
fn underwater_locations_stand_as_in_the_original() {
    // Which tiles have a tile object, and every wall, decoration, scenery
    // and ground decoration slot with its model after terrain alignment.
    let Some(run) = reproduces(&["exists", "locs"]) else {
        return;
    };
    let total = |pick: fn(&support::Stats) -> usize| run.stats.values().map(pick).sum::<usize>();
    assert!(total(|s| s.entities) > 1500);
    // Hill skew types 4 and 5 read the surface as the level above.
    assert!(total(|s| s.skews[4]) > 50 && total(|s| s.skews[5]) > 500);
    assert!(total(|s| s.walls) > 50 && total(|s| s.decorations) > 0 && total(|s| s.ground) > 0);
}

// The underwater material against the recorded GL calls of the original
// renderer (`cargo xtask materials-oracle`; `fixtures/underwater/
// material.jsonl`, scope in `material-provenance.json`). The trace says what
// state the original sets; what a GL driver computes from it is the
// transcribed part, stated at `rs09_client::underwater::shade`.
mod material {
    use rs09_client::{materials, underwater};
    use serde_json::Value;
    use std::path::Path;

    // OpenGL enumerants as the trace prints them.
    const TEXTURE1: i64 = 0x84C1;
    const TEXTURE2: i64 = 0x84C2;
    const TEXTURE_ENV: i64 = 0x2300;
    const TEXTURE_ENV_COLOR: i64 = 0x2201;
    const COMBINE_RGB: i64 = 0x8571;
    const COMBINE_ALPHA: i64 = 0x8572;
    const EYE_PLANE: i64 = 0x2502;
    const TEXTURE_GEN_S: i64 = 0x0C60;
    const S: i64 = 0x2000;
    const T: i64 = 0x2001;
    const R: i64 = 0x2002;
    // The oracle's frame.
    const LIGHT: i32 = 0xFFE0C0;
    const AMBIENT: f32 = 1.1523438;
    const DIFFUSE: f32 = 0.69921875;

    fn cases() -> Vec<Value> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        std::fs::read_to_string(root.join("fixtures/underwater/material.jsonl"))
            .unwrap()
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
    fn case(name: &str) -> Value {
        cases()
            .into_iter()
            .find(|case| case["case"] == name)
            .unwrap_or_else(|| panic!("no case {name}"))
    }
    /// The calls of a case as `(name, arguments)`.
    fn calls(case: &Value) -> Vec<(String, Vec<Value>)> {
        case["trace"]
            .as_array()
            .unwrap()
            .iter()
            .map(|call| {
                (
                    call["c"].as_str().unwrap().to_owned(),
                    call["a"].as_array().unwrap().clone(),
                )
            })
            .collect()
    }
    fn floats(value: &Value) -> Vec<f32> {
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap() as f32)
            .collect()
    }
    /// The calls made while `unit` is the active texture unit.
    fn on_unit(calls: &[(String, Vec<Value>)], unit: i64) -> Vec<(String, Vec<Value>)> {
        let mut active = 0x84C0;
        let mut out = Vec::new();
        for (name, args) in calls {
            if name == "glActiveTexture" {
                active = args[0].as_i64().unwrap();
            } else if active == unit {
                out.push((name.clone(), args.clone()));
            }
        }
        out
    }
    /// The last plane `glTexGenfv` gave coordinate `axis`.
    fn plane(calls: &[(String, Vec<Value>)], axis: i64) -> Vec<f32> {
        calls
            .iter()
            .rev()
            .find(|(name, args)| name == "glTexGenfv" && args[0] == axis && args[1] == EYE_PLANE)
            .map(|(_, args)| floats(&args[2]))
            .unwrap_or_else(|| panic!("no eye plane for {axis}"))
    }
    fn env(calls: &[(String, Vec<Value>)], parameter: i64) -> i64 {
        calls
            .iter()
            .rev()
            .find(|(name, args)| {
                name == "glTexEnvi" && args[0] == TEXTURE_ENV && args[1] == parameter
            })
            .map(|(_, args)| args[2].as_i64().unwrap())
            .unwrap()
    }
    fn env_color(calls: &[(String, Vec<Value>)]) -> Vec<f32> {
        calls
            .iter()
            .rev()
            .find(|(name, args)| name == "glTexEnvfv" && args[1] == TEXTURE_ENV_COLOR)
            .map(|(_, args)| floats(&args[2]))
            .unwrap()
    }

    #[test]
    fn the_gradient_texture_is_the_one_the_renderer_uploads() {
        let init = case("init");
        // Three texture units and 3D textures: the gradient is on unit 2.
        assert_eq!(init["use3DTexture"], true);
        assert_eq!(init["secondaryUnit"], TEXTURE2);
        let calls = calls(&init);
        let upload = calls
            .iter()
            .find(|(name, args)| name == "glTexImage1D" && args[3] == 8)
            .unwrap();
        let bytes: Vec<u8> = upload.1[7]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u8)
            .collect();
        assert_eq!(bytes, underwater::ramp_texels());
        // GL_ALPHA, GL_LINEAR both ways, GL_CLAMP_TO_EDGE: what `ramp` samples.
        assert_eq!(
            (upload.1[2].as_i64(), upload.1[5].as_i64()),
            (Some(0x1906), Some(0x1906))
        );
        let at = calls.iter().position(|call| call == upload).unwrap();
        let parameters: Vec<[i64; 2]> = calls[at + 1..at + 4]
            .iter()
            .map(|(name, args)| {
                assert_eq!(name, "glTexParameteri");
                [args[1].as_i64().unwrap(), args[2].as_i64().unwrap()]
            })
            .collect();
        assert_eq!(
            parameters,
            [[0x2801, 0x2601], [0x2800, 0x2601], [0x2802, 0x812F]]
        );
    }

    #[test]
    fn the_floor_binds_noise_on_unit_one_and_the_gradient_on_unit_two() {
        let bound = case("bind-floor");
        assert_eq!(
            (bound["currentType"].as_i64(), bound["currentArg"].as_i64()),
            (Some(3), Some(-1))
        );
        let calls = calls(&bound);
        // Unit 1 adds the 3D texture's colour and keeps the alpha.
        let noise = on_unit(&calls, TEXTURE1);
        assert_eq!(env(&noise, COMBINE_RGB), 0x0104, "GL_ADD");
        assert_eq!(env(&noise, COMBINE_ALPHA), 0x1E01, "GL_REPLACE");
        assert_eq!(env(&noise, 0x8588), 0x8578, "alpha from GL_PREVIOUS");
        let (s, t, r) = (plane(&noise, S), plane(&noise, T), plane(&noise, R));
        // `caustics` is the three planes applied to a scene position.
        let at = |point: [f32; 3]| underwater::caustics(point, 37);
        assert_eq!(
            s[..3],
            [
                at([1.0, 0.0, 0.0])[0],
                at([0.0, 1.0, 0.0])[0],
                at([0.0, 0.0, 1.0])[0]
            ]
        );
        assert_eq!(
            t[..3],
            [
                at([1.0, 0.0, 0.0])[1],
                at([0.0, 1.0, 0.0])[1],
                at([0.0, 0.0, 1.0])[1]
            ]
        );
        assert_eq!((s[3], t[3]), (0.0, 0.0));
        assert_eq!(r, [0.0, 0.0, 0.0, at([0.0; 3])[2]]);
        // Unit 2 interpolates toward the constant by the texture's alpha.
        let gradient = on_unit(&calls, TEXTURE2);
        assert_eq!(env(&gradient, COMBINE_RGB), 0x8575, "GL_INTERPOLATE");
        assert_eq!(env(&gradient, 0x8580), 0x8576, "source 0 is GL_CONSTANT");
        assert_eq!(env(&gradient, 0x8582), 0x1702, "source 2 is GL_TEXTURE");
        assert_eq!(env(&gradient, COMBINE_ALPHA), 0x1E01);
        assert_eq!(env(&gradient, 0x8588), 0x8578);
        let color = env_color(&gradient);
        assert_eq!(
            color[..3],
            underwater::water_fog(0x122B3D, LIGHT, AMBIENT, DIFFUSE)
        );
        // Argument -1: no generated coordinate, the vertices bring it.
        let generated = gradient
            .iter()
            .rev()
            .find(|(_, args)| args.first().and_then(Value::as_i64) == Some(TEXTURE_GEN_S))
            .unwrap();
        assert_eq!(generated.0, "glDisable");
    }

    #[test]
    fn no_other_material_is_bound_while_rendering_underwater() {
        let locked = case("locked");
        assert!(calls(&locked).is_empty());
        assert_eq!(
            (
                locked["currentType"].as_i64(),
                locked["currentArg"].as_i64()
            ),
            (Some(3), Some(-1))
        );
        // Before anything is bound a colour change touches no state.
        assert!(calls(&case("colour-unbound")).is_empty());
        // Leaving the pass unbinds it.
        let leave = case("leave");
        assert_eq!(leave["currentType"], 0);
        assert!(!calls(&leave).is_empty());
    }

    #[test]
    fn scenery_takes_its_gradient_coordinate_from_its_height() {
        for (name, argument, range, water, clock) in [
            ("scenery-200", 200, 128, 0x122B3D, 37),
            ("colour-bound", 200, 128, 0x17281B, 37),
            ("range-48", 200, 48, 0x17281B, 37),
            ("scenery-0", 0, 48, 0x17281B, 38),
        ] {
            let case = case(name);
            assert_eq!(case["currentArg"], argument, "{name}");
            let calls = calls(&case);
            let gradient = on_unit(&calls, TEXTURE2);
            let s = plane(&gradient, S);
            assert_eq!((s[0], s[2]), (0.0, 0.0), "{name}");
            for y in [-400.0f32, -200.0, -73.0, 0.0, 64.0] {
                assert_eq!(
                    y * s[1] + s[3],
                    underwater::scenery_depth(y, argument, range),
                    "{name} at {y}"
                );
            }
            assert_eq!(
                env_color(&gradient)[..3],
                underwater::water_fog(water, LIGHT, AMBIENT, DIFFUSE),
                "{name}"
            );
            let generated = gradient
                .iter()
                .rev()
                .find(|(_, args)| args.first().and_then(Value::as_i64) == Some(TEXTURE_GEN_S))
                .unwrap();
            assert_eq!(generated.0, "glEnable", "{name}");
            let r = plane(&on_unit(&calls, TEXTURE1), R);
            assert_eq!(r[3], underwater::caustics([0.0; 3], clock)[2], "{name}");
        }
        // A change of the fog range alone re-sends the floor's state.
        let floor = case("range-128");
        assert_eq!(floor["currentArg"], -1);
    }

    #[test]
    fn the_backdrop_is_unlit_without_depth_test_and_fogged_from_its_own_start() {
        let backdrop = case("backdrop");
        assert_eq!(backdrop["viewDistance"], materials::VIEW_DISTANCE);
        assert_eq!(
            backdrop["viewFadeDistance"].as_f64().unwrap() as f32,
            materials::VIEW_FADE_DISTANCE
        );
        let calls = calls(&backdrop);
        let names: Vec<&str> = calls.iter().map(|(name, _)| name.as_str()).collect();
        let at = |name: &str| {
            names
                .iter()
                .position(|n| *n == name)
                .unwrap_or_else(|| panic!("{name}"))
        };
        // One draw: the face above an underwater tile, of the water group.
        let draws: Vec<_> = calls
            .iter()
            .filter(|(name, _)| name == "glDrawElements")
            .collect();
        assert_eq!(draws.len(), 1);
        assert_eq!(draws[0].1[1], 6);
        let draw = at("glDrawElements");
        // Before it: lighting off, depth test off, depth writes off, the fog
        // start, the colour, no texture and the surface group's own layer.
        let disabled = calls
            .iter()
            .position(|(name, args)| name == "glDisable" && args[0] == 0x0B71)
            .unwrap();
        assert!(disabled < draw);
        assert!(
            at("GlRenderer.setLightingEnabled") < draw && at("GlRenderer.disableDepthMask") < draw
        );
        assert_eq!(calls[at("GlRenderer.setLightingEnabled")].1[0], 0);
        let fog = &calls[at("glFogf")];
        assert_eq!(fog.1[0], 0x0B63);
        assert_eq!(
            fog.1[1].as_f64().unwrap() as f32,
            materials::BACKDROP_FOG_START
        );
        assert!(at("glFogf") < draw);
        let color = floats(&calls[at("glColor4fv")].1[0]);
        assert_eq!(
            color[..3],
            underwater::backdrop_color(0x122B3D, LIGHT, AMBIENT, DIFFUSE)
        );
        assert_eq!(color[3], 1.0);
        assert_eq!(calls[at("GlRenderer.setTextureId")].1[0], -1);
        assert_eq!(
            calls[at("GlRenderer.setDepthLayer")].1[0].as_f64().unwrap() as f32,
            underwater::SURFACE_WATER_LAYER
        );
        // After it everything is put back.
        for restored in [
            "GlRenderer.restoreLighting",
            "glPopAttrib",
            "GlRenderer.enableDepthMask",
        ] {
            assert!(at(restored) > draw, "{restored}");
        }
        // The pass's uniforms carry the same colour and fog start.
        let uniforms = underwater::uniforms(
            underwater::Pass::Backdrop,
            &rs09_client::atmosphere::Parameters {
                color: LIGHT,
                ambient: AMBIENT,
                diffuse: [DIFFUSE, 1.2],
                fog_color: 0,
                fog_depth: 300,
            },
        );
        assert_eq!(uniforms.fog_range[0] * 128.0, materials::BACKDROP_FOG_START);
        for (channel, value) in [0x12 as f32, 0x2B as f32, 0x3D as f32]
            .into_iter()
            .enumerate()
        {
            assert!((value / 255.0 * uniforms.lights[0][channel] - color[channel]).abs() < 1e-6);
        }
        assert!(!uniforms.depth_write);
    }
}
