use rs09_client::{
    animation::{Base, Frame, Sequence},
    pack::Pack,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
fn int(h: &mut Sha256, n: i32) {
    h.update(n.to_be_bytes());
}
fn ints(h: &mut Sha256, a: impl IntoIterator<Item = i32>) {
    let a: Vec<_> = a.into_iter().collect();
    int(h, a.len() as i32);
    for n in a {
        int(h, n);
    }
}
#[test]
fn every_packaged_sequence_frame_and_base_matches_unchanged_original_decoders() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../fixtures/players/animation-assets.json"
    ))
    .unwrap();
    for (name, expected) in fixture["sources"].as_object().unwrap() {
        let source = std::fs::read_to_string(
            root.join("reference/client/client/src/main/java/rt4")
                .join(name),
        )
        .unwrap()
        .replace("\r\n", "\n");
        assert_eq!(
            format!("{:x}", Sha256::digest(source)),
            expected.as_str().unwrap(),
            "source {name}"
        );
    }
    let bytes = std::fs::read(root.join("assets/visual/animations.pack")).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        fixture["packSha256"].as_str().unwrap()
    );
    let pack = Pack::decode(bytes).unwrap();
    let bases: BTreeMap<_, _> = pack
        .manifest
        .files
        .keys()
        .filter_map(|n| {
            n.strip_prefix("base/").map(|id| {
                let id = id.parse::<u16>().unwrap();
                (id, Base::decode(id, pack.get(n).unwrap()).unwrap())
            })
        })
        .collect();
    for prefix in ["base", "frame", "seq"] {
        let mut h = Sha256::new();
        let mut count = 0;
        for name in pack
            .manifest
            .files
            .keys()
            .filter(|n| n.starts_with(&format!("{prefix}/")))
        {
            count += 1;
            int(&mut h, name.len() as i32);
            h.update(name.as_bytes());
            let raw = pack.get(name).unwrap();
            match prefix {
                "base" => {
                    let b = &bases[&name[5..].parse::<u16>().unwrap()];
                    int(&mut h, i32::from(b.id));
                    int(&mut h, b.types.len() as i32);
                    for i in 0..b.types.len() {
                        int(&mut h, i32::from(b.types[i]));
                        h.update([u8::from(b.shadow[i])]);
                        int(&mut h, i32::from(b.parts[i]));
                        ints(&mut h, b.labels[i].iter().map(|&n| i32::from(n)));
                    }
                }
                "frame" => {
                    let base = u16::from_be_bytes(raw[..2].try_into().unwrap());
                    let f =
                        Frame::decode(raw, &bases[&base]).unwrap_or_else(|e| panic!("{name}: {e}"));
                    int(&mut h, i32::from(f.base));
                    int(&mut h, f.transforms.len() as i32);
                    for t in f.transforms {
                        int(&mut h, t.index as i32);
                        for n in t.xyz {
                            int(&mut h, n);
                        }
                        int(&mut h, t.previous_origin.map_or(-1, |n| n as i32));
                        int(&mut h, i32::from(t.flags));
                    }
                }
                "seq" => {
                    let s = Sequence::decode(raw).unwrap_or_else(|e| panic!("{name}: {e}"));
                    ints(&mut h, s.frames.into_iter().map(|n| n as i32));
                    ints(&mut h, s.delays.into_iter().map(i32::from));
                    int(&mut h, s.replay_offset.map_or(-1, |n| n as i32));
                    for b in [s.transform_normals, s.tween, s.stretches, s.update_shadows] {
                        h.update([u8::from(b)]);
                    }
                    int(
                        &mut h,
                        s.frame_group.as_ref().map_or(-1, |a| a.len() as i32),
                    );
                    if let Some(mask) = s.frame_group {
                        for b in mask {
                            h.update([u8::from(b)]);
                        }
                    }
                    for n in [
                        s.priority,
                        s.replay_count,
                        s.loop_type,
                        s.move_type,
                        s.exact_move,
                        s.mainhand,
                        s.offhand,
                    ] {
                        int(&mut h, n);
                    }
                    ints(&mut h, s.secondary_frames.into_iter().map(|n| n as i32));
                    int(&mut h, s.sounds.len() as i32);
                    for row in s.sounds {
                        ints(&mut h, row.into_iter().map(|n| n as i32));
                    }
                }
                _ => unreachable!(),
            }
        }
        assert_eq!(
            count,
            fixture["sections"][prefix]["count"].as_i64().unwrap()
        );
        assert_eq!(
            format!("{:x}", h.finalize()),
            fixture["sections"][prefix]["sha256"].as_str().unwrap(),
            "complete {prefix} fields"
        );
    }
}
