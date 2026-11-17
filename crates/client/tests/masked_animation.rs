use rs09_client::{animation_parts as masked, model, surface::Surface};
use serde_json::{Value, json};
fn ints(v: &Value) -> Vec<i32> {
    v.as_array()
        .map(|a| a.iter().map(|v| v.as_i64().unwrap() as i32).collect())
        .unwrap_or_default()
}
fn points(v: &Value) -> Vec<[i32; 3]> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|p| ints(p).try_into().unwrap())
        .collect()
}
#[test]
fn unchanged_original_masked_gl_and_bas_matrices_match() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let f: Value =
        serde_json::from_slice(&std::fs::read(root.join("fixtures/players/masked.json")).unwrap())
            .unwrap();
    use sha2::{Digest, Sha256};
    for (name, hash) in f["sources"].as_object().unwrap() {
        let raw = std::fs::read_to_string(
            root.join("reference/client/client/src/main/java/rt4")
                .join(name),
        )
        .unwrap()
        .replace("\r\n", "\n");
        assert_eq!(
            format!("{:x}", Sha256::digest(raw.as_bytes())),
            hash.as_str().unwrap(),
            "source{name}"
        );
    }

    for (i, c) in f["basisCases"].as_array().unwrap().iter().enumerate() {
        let input = ints(&c["transform"]).try_into().unwrap();
        assert_eq!(
            json!(masked::basis(input).to_vec()),
            c["expected"],
            "BAS{i}"
        );
        assert_eq!(
            json!(masked::basis(input).to_vec()),
            c["npcExpected"],
            "NPC BAS{i}"
        );
    }
    for (i, c) in f["cases"].as_array().unwrap().iter().enumerate() {
        let normals = c["normals"].as_array().map(|_| points(&c["normals"]));
        let surfaces = normals
            .as_ref()
            .map(|n| {
                (0..3)
                    .map(|i| Surface {
                        normals: std::array::from_fn(|j| {
                            let a = n[i * 3 + j];
                            [
                                a[0] as i16 as i32,
                                a[1] as i16 as i32,
                                a[2] as i16 as i32,
                                0,
                            ]
                        }),
                        ..Default::default()
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut m = model::Model {
            vertices: points(&c["vertices"]),
            vertex_labels: ints(&c["vertexLabels"])
                .into_iter()
                .map(|n| n as u8)
                .collect(),
            vertex_sources: ints(&c["vertexSources"])
                .into_iter()
                .map(|n| n as u16)
                .collect(),
            faces: c["faces"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| {
                    ints(v)
                        .into_iter()
                        .map(|i| i as usize)
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap()
                })
                .collect(),
            face_labels: ints(&c["faceLabels"])
                .into_iter()
                .map(|n| n as u8)
                .collect(),
            face_sources: ints(&c["faceSources"])
                .into_iter()
                .map(|n| n as u16)
                .collect(),
            alpha: ints(&c["alpha"]).into_iter().map(|n| n as u8).collect(),
            colors: ints(&c["colors"]).into_iter().map(|n| n as u16).collect(),
            surfaces,
            ..Default::default()
        };
        let origin = ints(&c["origin"]);
        let mut o = masked::Origin {
            xyz: origin[..3].try_into().unwrap(),
            valid: origin[3] != 0,
            ..Default::default()
        };
        for (step, cmd) in c["commands"].as_array().unwrap().iter().enumerate() {
            let labels = ints(&cmd["labels"])
                .into_iter()
                .map(|n| n as u8)
                .collect::<Vec<_>>();
            let matrix: Option<[i32; 15]> = cmd["matrix"]
                .as_array()
                .map(|_| ints(&cmd["matrix"]).try_into().unwrap());
            masked::transform(
                &mut m,
                &mut o,
                cmd["kind"].as_u64().unwrap() as u8,
                &labels,
                ints(&cmd["xyz"]).try_into().unwrap(),
                cmd["normals"].as_bool().unwrap(),
                cmd["mask"].as_u64().unwrap() as u16,
                matrix.as_ref(),
            );
            let n = if normals.is_some() {
                Some(
                    m.surfaces
                        .iter()
                        .flat_map(|s| s.normals.iter().map(|n| [n[0], n[1], n[2]]))
                        .collect::<Vec<_>>(),
                )
            } else {
                None
            };
            let alpha = if c["alpha"].is_null() {
                None
            } else {
                Some(&m.alpha)
            };
            let actual = json!([
                m.vertices,
                o.xyz,
                o.valid,
                n,
                alpha,
                m.colors,
                o.colors_dirty,
                o.normals_dirty
            ]);
            assert_eq!(actual, c["expected"][step], "masked case{i}step{step}");
        }
    }
    println!("PASS complete original masked GL:2048sequences/16384steps and512BAS matrices");
}
