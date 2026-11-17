//! Explicit offline ground update inputs. These files do not implement drops,
//! pickup rules, inventory transactions or a live content handler.
use rs09_protocol::game::{Direction, Frame, updates};
use std::{fs, path::Path};
fn write(path: &Path, frames: &[Frame]) {
    let mut bytes = Vec::new();
    for frame in frames {
        frame
            .write(Direction::ServerToClient, None, &mut bytes)
            .unwrap();
    }
    fs::write(path, bytes).unwrap();
}
fn main() {
    let output = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "runs/visual/ground-preview".into());
    let output = Path::new(&output);
    fs::create_dir_all(output).unwrap();
    let spawn = updates::zone_enclosed(
        64,
        48,
        &[
            updates::ground_add(7, 2, 995, 100),
            updates::ground_add(6, 3, 1333, 1),
            updates::ground_add(5, 2, 526, 1),
            updates::ground_add(6, 1, 1201, 1),
            updates::ground_add(6, 1, 1277, 1),
            // The original unsigned16 wire truncates this amount to24464.
            updates::ground_add(6, 1, 995, 90000),
            updates::ground_add(6, 1, 995, 1000),
        ],
    )
    .unwrap();
    let amount =
        updates::zone_enclosed(64, 48, &[updates::ground_amount(7, 2, 995, 100, 60000)]).unwrap();
    let remove =
        updates::zone_enclosed(64, 48, &[updates::ground_remove(6, 1, 995 | 0x8000)]).unwrap();
    let clear = updates::zone_origin(64, 48, true);
    for (name, frames) in [
        ("spawn", vec![spawn.clone()]),
        ("amount", vec![spawn.clone(), amount.clone()]),
        ("remove", vec![spawn.clone(), remove.clone()]),
        ("clear", vec![spawn.clone(), clear.clone()]),
        ("sequence", vec![spawn, amount, remove, clear]),
        (
            "excluded",
            vec![
                updates::zone_enclosed(
                    64,
                    48,
                    &[
                        updates::ground_add_excluding(7, 2, 995, 123, 0),
                        updates::ground_add_excluding(6, 3, 1333, 1, 1),
                    ],
                )
                .unwrap(),
            ],
        ),
        (
            "table",
            vec![updates::zone_enclosed(56, 40, &[updates::ground_add(0, 6, 995, 1000)]).unwrap()],
        ),
        (
            "support-sequence",
            vec![
                updates::zone_enclosed(56, 40, &[updates::ground_add(0, 6, 995, 1000)]).unwrap(),
                updates::zone_enclosed(56, 40, &[updates::loc_del(0, 6, 10, 0)]).unwrap(),
                updates::zone_enclosed(56, 40, &[updates::ground_amount(0, 6, 995, 1000, 1000)])
                    .unwrap(),
                updates::zone_enclosed(56, 40, &[updates::loc_add(0, 6, 10, 0, 36821)]).unwrap(),
                updates::zone_enclosed(56, 40, &[updates::ground_amount(0, 6, 995, 1000, 1000)])
                    .unwrap(),
            ],
        ),
    ] {
        write(&output.join(format!("{name}.bin")), &frames);
    }
    fs::write(
        output.join("outdoor-npcs.json"),
        r#"{"start":[3222,3218],"frames":[]}"#,
    )
    .unwrap();
    fs::write(
        output.join("indoor-npcs.json"),
        r#"{"start":[3207,3216],"frames":[]}"#,
    )
    .unwrap();
    println!(
        "Wrote explicit offline ground previews to {}",
        output.display()
    );
}
