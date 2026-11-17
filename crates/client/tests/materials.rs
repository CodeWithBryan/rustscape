//! The HD material renderers (`rs09_client::materials`).
//!
//! Two kinds of evidence, kept apart:
//!
//! - JVM differential: the noise volumes and the liquid turbulence table are
//!   compared with `fixtures/materials/noise.jsonl`, which `cargo xtask
//!   materials-oracle` writes from the unchanged RT4 generator classes.
//! - Transcription tests: the coordinate and colour formulas are checked
//!   against values worked out by hand from `MaterialManager` and the
//!   `*MaterialRenderer` classes. No original code runs for these, and the
//!   GLSL that repeats the formulas is not executed by any test.
use rs09_client::{
    materials::{self, noise},
    pack::Pack,
    texture,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn fixture() -> BTreeMap<String, Value> {
    let text = std::fs::read_to_string(root().join("fixtures/materials/noise.jsonl")).unwrap();
    text.lines()
        .filter(|line| !line.is_empty())
        .map(|line| {
            let value: Value = serde_json::from_str(line).unwrap();
            (value["case"].as_str().unwrap().to_owned(), value)
        })
        .collect()
}
fn near(a: f32, b: f32) -> bool {
    (a - b).abs() <= 1e-5 * b.abs().max(1.0)
}

#[test]
fn oracle_sources_are_the_pinned_reference_files() {
    let provenance: Value = serde_json::from_slice(
        &std::fs::read(root().join("fixtures/materials/provenance.json")).unwrap(),
    )
    .unwrap();
    let lock: Value =
        serde_json::from_slice(&std::fs::read(root().join("upstream.lock.json")).unwrap()).unwrap();
    assert_eq!(
        provenance["upstream"]["client"]["commit"],
        lock["client"]["commit"]
    );
    assert_eq!(
        hash(
            std::fs::read_to_string(root().join("fixtures/materials/noise.jsonl"))
                .unwrap()
                .replace('\r', "")
                .as_bytes()
        ),
        provenance["fixtureSha256"].as_str().unwrap()
    );
    for group in ["sources", "transcribed"] {
        for (path, expected) in provenance[group].as_object().unwrap() {
            let path = root().join(path);
            if !path.exists() {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap().replace('\r', "");
            assert_eq!(
                hash(text.as_bytes()),
                expected.as_str().unwrap(),
                "{} changed: review the materials port",
                path.display()
            );
        }
    }
}

/// JVM differential: every byte of both volumes.
#[test]
fn noise_volumes_match_the_jvm_generators() {
    let fixture = fixture();
    for (name, bytes) in [("ridged", noise::ridged()), ("fractal", noise::fractal())] {
        let expected = &fixture[name];
        assert_eq!(bytes.len(), noise::VOLUME_BYTES);
        assert_eq!(expected["bytes"].as_u64().unwrap() as usize, bytes.len());
        let slice = noise::VOLUME_BYTES / noise::SIDE;
        for (z, wanted) in expected["slices"].as_array().unwrap().iter().enumerate() {
            assert_eq!(
                hash(&bytes[z * slice..(z + 1) * slice]),
                wanted.as_str().unwrap(),
                "{name} slice {z}"
            );
        }
        assert_eq!(hash(&bytes), expected["sha256"].as_str().unwrap(), "{name}");
        // The raw row the oracle prints: z 32, y 32.
        let start = (32 * 4096 + 32 * 64) * 2;
        let row: Vec<u64> = bytes[start..start + 128]
            .iter()
            .map(|&b| u64::from(b))
            .collect();
        let wanted: Vec<u64> = expected["row"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect();
        assert_eq!(row, wanted, "{name} row");
    }
}

/// JVM differential: every value of the turbulence table, for both of the
/// renderer's `generateNoiseTable` calls.
#[test]
fn liquid_table_matches_the_jvm_generator() {
    let fixture = fixture();
    let table = noise::liquid();
    assert_eq!(table.len(), noise::LIQUID_ROWS * noise::LIQUID_COLUMNS);
    for name in ["liquid-x", "liquid-y"] {
        let expected = &fixture[name];
        assert_eq!(expected["rows"], 256);
        assert_eq!(expected["columns"], 64);
        let wanted: Vec<i32> = expected["values"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap() as i32)
            .collect();
        assert_eq!(table, wanted, "{name}");
        let raw: Vec<u8> = table.iter().flat_map(|v| v.to_be_bytes()).collect();
        assert_eq!(hash(&raw), expected["sha256"].as_str().unwrap(), "{name}");
    }
}

/// Transcription: `MaterialManager.setMaterial` and `getFlags`.
#[test]
fn routing_remaps_water_without_high_detail_only() {
    assert_eq!(materials::route(4, 1, true), (4, 1));
    assert_eq!(materials::route(4, 1, false), (2, 2));
    assert_eq!(materials::route(4, 0, false), (2, 2));
    for kind in [0, 1, 2, 3, 5, 6] {
        for argument in [0, 1, 66, 137, 255] {
            for detail in [false, true] {
                assert_eq!(materials::route(kind, argument, detail), (kind, argument));
            }
        }
    }
    assert_eq!(
        [0, 1, 2, 3, 4, 5, 6].map(materials::flags),
        [0, 4, 0, 0, 15, 0, 0]
    );
}

/// The material table of the pinned cache, from the committed texture pack.
#[test]
fn pinned_cache_materials_have_the_expected_types_and_mipmap_modes() {
    let pack =
        Pack::decode(std::fs::read(root().join("fixtures/visual/textures.pack")).unwrap()).unwrap();
    let table = texture::materials(pack.get("materials").unwrap()).unwrap();
    let mut kinds = BTreeMap::<(u8, u8), usize>::new();
    let mut modes = BTreeMap::<u8, usize>::new();
    let mut mipmaps = vec![0; table.len()];
    for (id, material) in table.iter().enumerate() {
        let Some(material) = material else { continue };
        *kinds
            .entry((material.material_type, material.animation_type))
            .or_default() += 1;
        let graph = texture::Graph::decode(pack.get(&format!("texture/{id}")).unwrap()).unwrap();
        mipmaps[id] = graph.mipmap;
        *modes.entry(graph.mipmap).or_default() += 1;
    }
    let expected = [
        ((0, 0), 571),
        ((1, 1), 20),
        ((1, 2), 46),
        ((1, 3), 16),
        ((2, 1), 8),
        ((2, 2), 1),
        ((2, 3), 1),
        ((2, 66), 1),
        ((2, 130), 3),
        ((4, 0), 1),
        ((4, 1), 4),
        ((5, 9), 1),
        ((5, 24), 1),
        ((5, 137), 1),
        ((5, 152), 1),
        ((6, 1), 4),
    ];
    assert_eq!(kinds, expected.into_iter().collect());
    assert_eq!(modes, [(0, 6), (2, 674)].into_iter().collect());
    let kind = |id: usize| {
        let m = table[id].as_ref().unwrap();
        (m.material_type, m.animation_type)
    };
    assert_eq!(kind(669), (4, 1), "the common water overlay");
    assert_eq!(kind(61), (2, 66), "lava");
    assert_eq!(kind(638), (2, 1), "fountain water");
    assert_eq!(kind(646), (5, 137), "waterfall");
    assert_eq!(kind(110), (6, 1));

    // The tables texture carries them where the shader looks.
    let [width, height] = materials::TABLES;
    let tables = materials::tables(&table, &mipmaps);
    assert_eq!(tables.len(), width * height * 4);
    for id in [61usize, 110, 511, 512, 646, 669] {
        let at = ((768 + id / 512) * width + id % 512) * 4;
        let expected = match &table[id] {
            Some(m) => [m.material_type, m.animation_type, mipmaps[id], 255],
            None => [0; 4],
        };
        assert_eq!(tables[at..at + 4], expected, "texture {id}");
    }
    let (ridged, fractal) = (noise::ridged(), noise::fractal());
    for [x, y, z] in [[0usize, 0, 0], [5, 3, 10], [63, 63, 63], [17, 40, 33]] {
        let from = ((z * 64 + y) * 64 + x) * 2;
        let at = ((z / 8 * 64 + y) * width + z % 8 * 64 + x) * 4;
        assert_eq!(
            tables[at..at + 4],
            [ridged[from], ridged[from + 1], fractal[from], 255],
            "volume texel {x},{y},{z}"
        );
    }
    let liquid = noise::liquid();
    for [row, col] in [[0usize, 0], [0, 1], [37, 63], [255, 20]] {
        let at = ((512 + row) * width + col) * 4;
        let value = (liquid[row * 64 + col] + 32768) as u16;
        let [high, low] = value.to_be_bytes();
        assert_eq!(
            tables[at..at + 4],
            [high, low, high, low],
            "liquid {row},{col}"
        );
    }
}

/// Transcription: `WaterMaterialRenderer.bind` and its two texture units.
#[test]
fn water_fade_and_colour_follow_the_fixed_function_setup() {
    // nearDist = 2662.4001 + (pitch - 128) * 0.5, max = 7168 - 512.
    assert!(near(materials::water_range(128)[0], 2662.4001));
    assert_eq!(materials::water_range(128)[1], 6656.0);
    assert!(near(materials::water_range(383)[0], 2789.9001));
    assert!(near(materials::water_range(0)[0], 2598.4001));
    // nearDist >= max clamps to max - 1.
    assert_eq!(materials::water_range(8200), [6655.0, 6656.0]);

    // S = (depth - near) / (far - near) through a two-texel linear ramp
    // clamped to its edge: zero up to a quarter, one from three quarters.
    let range = [2000.0, 6000.0];
    for (depth, expected) in [
        (1000.0, 0.0),
        (2000.0, 0.0),
        (3000.0, 0.0),
        (3500.0, 0.25),
        (4000.0, 0.5),
        (4500.0, 0.75),
        (5000.0, 1.0),
        (7000.0, 1.0),
    ] {
        assert!(
            near(materials::water_fade(depth, range), expected),
            "depth {depth}"
        );
    }
    assert_eq!(materials::water_ramp(0.25), 0.0);
    assert_eq!(materials::water_ramp(0.75), 1.0);

    // Unit 0: (texture + (0.1, 0.1, 0.15)) * 2, alpha texture + 0.1.
    let surface = materials::water_surface([0.0, 0.0], [0.5; 3], 0.0);
    for (got, expected) in surface.into_iter().zip([0.2, 0.2, 0.3, 0.1]) {
        assert!(near(got, expected));
    }
    // The brightest ridged texel: luminance 23, alpha 63.
    let peak = materials::water_surface([23.0 / 255.0, 63.0 / 255.0], [0.5; 3], 0.0);
    for (got, expected) in peak.into_iter().zip([
        (23.0 / 255.0 + 0.1) * 2.0,
        (23.0 / 255.0 + 0.1) * 2.0,
        (23.0 / 255.0 + 0.15) * 2.0,
        63.0 / 255.0 + 0.1,
    ]) {
        assert!(near(got, expected));
    }
    // The doubled sum clamps at one.
    assert_eq!(
        materials::water_surface([0.6, 0.95], [0.0; 3], 0.0),
        [1.0; 4]
    );
    // Unit 1: constant * ramp + previous * (1 - ramp); the constant's alpha is one.
    let water = [18.0 / 255.0, 43.0 / 255.0, 61.0 / 255.0];
    let faded = materials::water_surface([0.05, 0.2], water, 1.0);
    assert_eq!(faded, [water[0], water[1], water[2], 1.0]);
    let half = materials::water_surface([0.0, 0.0], water, 0.5);
    for (got, expected) in half.into_iter().zip([
        0.1 + water[0] * 0.5,
        0.1 + water[1] * 0.5,
        0.15 + water[2] * 0.5,
        0.55,
    ]) {
        assert!(near(got, expected));
    }

    // FloType.waterColor's default, 0x122b3d, in a vertex's texture coordinate.
    assert_eq!(
        materials::water_uv(0x122b3d),
        [(18 * 256 + 43) as f32, 61.0]
    );
    assert_eq!(materials::water_uv(0xffffff), [65535.0, 255.0]);

    // applyFogFade: fog from 7168 - 1024 to 7168 - 512 over the plain colour.
    assert_eq!(materials::BACKDROP_FOG_START, 6144.0);
    let fog = [0.8, 0.7, 0.6];
    let clear = materials::water_backdrop(water, fog, 3000.0);
    let fogged = materials::water_backdrop(water, fog, 6656.0);
    let mid = materials::water_backdrop(water, fog, 6400.0);
    for i in 0..3 {
        assert!(near(clear[i], water[i]));
        assert!(near(fogged[i], fog[i]));
        assert!(near(mid[i], (water[i] + fog[i]) / 2.0));
    }
}

/// Transcription: `Camera.cameraPitch` from the scene camera's direction.
#[test]
fn camera_pitch_is_recovered_in_whole_units() {
    for pitch in [0, 1, 128, 200, 256, 383] {
        for yaw in [0, 300, 1024, 1700] {
            let unit = std::f32::consts::TAU / 2048.0;
            let (p, y) = (pitch as f32 * unit, yaw as f32 * unit);
            // `SceneCamera::posed`.
            let forward = [-y.sin() * p.cos(), -p.sin(), y.cos() * p.cos()];
            assert_eq!(materials::camera_pitch(forward), pitch);
            assert_eq!(
                materials::camera_pitch(forward.map(|n| n * 37.5)),
                pitch,
                "unnormalised"
            );
        }
    }
    assert_eq!(materials::camera_pitch([0.0; 3]), 0);
}

/// Transcription: `LiquidMaterialRenderer.setArgument` and its vertex program.
#[test]
fn liquid_coordinates_follow_the_vertex_program() {
    assert_eq!(
        [0u8, 1, 2, 3, 66, 130].map(materials::liquid_turbulence),
        [0.025, 0.025, 0.05, 0.1, 0.05, 0.05]
    );
    assert!(!materials::liquid_full_bright(1) && materials::liquid_full_bright(66));
    assert!(!materials::liquid_plain(66) && materials::liquid_plain(130));

    // noise.x = frac((x + z) / 8192) * 64, noise.y the same of z - x; ARL floors.
    assert_eq!(materials::liquid_columns(0.0, 0.0), [0, 0]);
    assert_eq!(materials::liquid_columns(4096.0, 0.0), [32, 32]);
    assert_eq!(materials::liquid_columns(1000.0, 3000.0), [31, 15]);
    assert_eq!(materials::liquid_columns(128.0, 0.0), [1, 63]);
    assert_eq!(
        materials::liquid_columns(8192.0 * 5.0 + 127.0, 0.0),
        [0, 63]
    );

    // Row clock & 255; texcoord = position / 128 + noise / 4096 * turbulence.
    let table = noise::liquid();
    // Row 0 starts 0, -1384 in the JVM fixture.
    assert_eq!(table[..2], [0, -1384]);
    let [s, t] = materials::liquid_texcoord(&table, 0, 1, 128.0, 0.0);
    assert!(near(s, 1.0 + -1384.0 / 4096.0 * 0.025));
    assert!(near(t, table[63] as f32 / 4096.0 * 0.025));
    let [s, t] = materials::liquid_texcoord(&table, 256 + 37, 3, 1000.0, 3000.0);
    assert!(near(
        s,
        1000.0 / 128.0 + table[37 * 64 + 31] as f32 / 4096.0 * 0.1
    ));
    assert!(near(
        t,
        3000.0 / 128.0 + table[37 * 64 + 15] as f32 / 4096.0 * 0.1
    ));

    // oFogCoord = clipPos.z with the frustum of near 50, far 7168.
    let [scale, offset] = materials::liquid_fog_row();
    assert!(near(scale, -7218.0 / 7118.0));
    assert!(near(offset, -716800.0 / 7118.0));
    assert!(near(
        materials::liquid_fog_coordinate(1000.0),
        7218.0 / 7118.0 * 1000.0 - 716800.0 / 7118.0
    ));
}

/// Transcription: `WaterfallMaterialRenderer.setArgument`.
#[test]
fn waterfall_arguments_select_speeds_scale_and_axis() {
    let fine = 4.8828125e-4;
    let coarse = 9.765625e-4;
    // The four waterfall textures of the pinned cache.
    let cases = [
        (137u8, 0.02f32, -0.02f32, fine, true),
        (152, 0.04, -0.01, fine, true),
        (24, 0.04, -0.01, fine, false),
        (9, 0.02, -0.02, fine, false),
        (0, 0.01, -0.01, fine, false),
        (0x40 | 3, 0.01, -0.04, coarse, false),
    ];
    for (argument, animation, scroll, scale, horizontal) in cases {
        let w = materials::waterfall(argument);
        assert!(near(w.animation_speed, animation), "{argument}");
        assert!(near(w.scroll_speed, scroll), "{argument}");
        assert_eq!(w.scale, scale, "{argument}");
        assert_eq!(w.horizontal, horizontal, "{argument}");
    }
    // S along X when horizontal, else Z; T along the height with the scroll; R the clock.
    let [s, t, r] = materials::waterfall(137).coordinate([2048.0, -4096.0, 1024.0], 10);
    assert!(near(s, 1.0) && near(t, -2.2) && near(r, 0.2));
    let [s, t, r] = materials::waterfall(24).coordinate([2048.0, -4096.0, 1024.0], 10);
    assert!(near(s, 0.5) && near(t, -2.1) && near(r, 0.4));
}

/// Transcription: `SpecularMaterialRenderer.createCubeMapTextures`.
#[test]
fn specular_cube_maps_hold_halved_powers_of_the_direction_z() {
    assert_eq!(
        [1u8, 2, 3].map(materials::specular_exponent),
        [96.0, 36.0, 12.0]
    );
    // (int) (pow(z, n) * 255) / 2 as a byte.
    assert_eq!(materials::specular_texel(1.0, 96.0), 127);
    assert_eq!(materials::specular_texel(1.0, 12.0), 127);
    assert_eq!(materials::specular_texel(0.99, 96.0), 48);
    assert_eq!(materials::specular_texel(0.5, 12.0), 0);
    assert_eq!(materials::specular_texel(0.9, 12.0), 36);
    assert_eq!(materials::specular_texel(0.0, 12.0), 0);
    assert_eq!(materials::specular_texel(-0.7, 36.0), 0);

    // Each face stores the Z of its texel's direction: +Z and -Z at their
    // centres, and the side faces along the edge that faces +Z.
    let diagonal = std::f32::consts::FRAC_1_SQRT_2;
    assert!(near(materials::specular_cube_z(4, 32, 32), 1.0));
    assert!(near(materials::specular_cube_z(5, 32, 32), -1.0));
    assert!(near(materials::specular_cube_z(0, 0, 32), diagonal));
    assert!(near(materials::specular_cube_z(1, 0, 32), -diagonal));
    assert!(near(materials::specular_cube_z(2, 32, 0), -diagonal));
    assert!(near(materials::specular_cube_z(3, 32, 0), diagonal));
    assert!(near(materials::specular_cube_z(4, 0, 0), 1.0 / 3f32.sqrt()));

    // glRotatef(22.5, 1, 0, 0) on the eye normal: z' = y sin + z cos.
    let (sin, cos) = 22.5f32.to_radians().sin_cos();
    assert!(near(materials::specular_direction([0.0, 0.0, 1.0]), cos));
    assert!(near(materials::specular_direction([0.0, 1.0, 0.0]), sin));
    assert!(near(materials::specular_direction([1.0, 0.0, 0.0]), 0.0));
    assert!(near(materials::specular_direction([0.3, sin, cos]), 1.0));
}

/// Mipmap levels: the box filter and the atlas layout the shader reads.
#[test]
fn mip_atlas_holds_three_filtered_levels_per_texture() {
    let base: Vec<u8> = (0..4 * 4 * 4).map(|i| (i * 3) as u8).collect();
    let half = materials::halve(&base, 4);
    assert_eq!(half.len(), 2 * 2 * 4);
    for y in 0..2 {
        for x in 0..2 {
            for c in 0..4 {
                let at =
                    |dx: usize, dy: usize| u32::from(base[((y * 2 + dy) * 4 + x * 2 + dx) * 4 + c]);
                assert_eq!(
                    u32::from(half[(y * 2 + x) * 4 + c]),
                    (at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1) + 2) / 4
                );
            }
        }
    }
    assert_eq!([1, 2, 3].map(materials::mip_column), [0, 64, 96]);

    let mut atlas = materials::MipAtlas::new(4);
    assert_eq!(atlas.size(), [512, 256]);
    let texture: Vec<u8> = (0..128 * 128 * 4).map(|i| (i % 251) as u8).collect();
    // Mode 0 stores nothing.
    atlas.add(5, &texture, 128, 0, |_| unreachable!()).unwrap();
    assert!(atlas.rgba.iter().all(|&b| b == 0));
    // Mode 2 filters each level from the one above.
    atlas.add(5, &texture, 128, 2, |_| unreachable!()).unwrap();
    let level1 = materials::halve(&texture, 128);
    let level2 = materials::halve(&level1, 64);
    let level3 = materials::halve(&level2, 32);
    let cell = [128, 64];
    for (level, pixels, side) in [(1, &level1, 64usize), (2, &level2, 32), (3, &level3, 16)] {
        for [x, y] in [[0, 0], [side - 1, side - 1], [side / 2, 3]] {
            let at = ((cell[1] + y) * 512 + cell[0] + materials::mip_column(level) + x) * 4;
            assert_eq!(
                atlas.rgba[at..at + 4],
                pixels[(y * side + x) * 4..(y * side + x) * 4 + 4],
                "level {level} texel {x},{y}"
            );
        }
    }
    // Mode 1 generates the texture again at each halved size.
    let mut sides = vec![];
    atlas
        .add(0, &texture[..64 * 64 * 4], 64, 1, |side| {
            sides.push(side);
            Ok(vec![side as u8; side * side * 4])
        })
        .unwrap();
    assert_eq!(sides, [32, 16, 8]);
    assert_eq!(atlas.rgba[0], 32);
    assert_eq!(atlas.rgba[64 * 4], 16);
    assert_eq!(atlas.rgba[96 * 4], 8);
    assert!(
        atlas
            .add(1, &texture, 128, 1, |_| Err("unsupported".into()))
            .is_err()
    );
}

/// The settings the frame's uniforms carry, and the shader's assembly.
#[test]
fn view_uniform_and_shader_sources_carry_the_settings() {
    let forward = {
        let pitch = 300.0 * std::f32::consts::TAU / 2048.0;
        [0.0, -pitch.sin(), pitch.cos()]
    };
    assert_eq!(
        materials::view(forward),
        [materials::water_range(300)[0], 1.0, 1.0, 1.0]
    );
    materials::set_high_water_detail(false);
    materials::set_underwater_pass(true);
    assert_eq!(materials::view(forward)[1..], [0.0, 1.0, 0.0]);
    materials::set_high_water_detail(true);
    materials::set_underwater_pass(false);
    assert!(materials::high_water_detail() && !materials::underwater_pass());

    let [vertex, fragment] = materials::world_shader(
        "attribute vec3 position;\n//@material-vertex\nvoid main(){}\n",
        "#version 100\n//@material-header\nprecision highp float;\n//@material-fragment\nvoid main(){}\n",
    );
    for source in [vertex, fragment] {
        assert!(!source.contains('@'), "an unfilled constant or marker");
    }
    assert!(vertex.contains("void materialVertex()"));
    assert!(fragment.starts_with("#version 100\n#ifdef GL_OES_standard_derivatives\n"));
    assert!(fragment.contains("vec4 materialSurface(vec4 result,vec4 tex)"));
    // The constants come from the functions above.
    assert!(fragment.contains("/(6656.0-MaterialView.x)"));
    assert!(fragment.contains("(FogRange.y-48.0)"));
    assert!(fragment.contains("vec3(0.1,0.1,0.15)"));
    let [scale, offset] = materials::liquid_fog_row();
    assert!(vertex.contains(&format!("*{:?}-{:?});", -scale, -offset / 128.0)));
}
