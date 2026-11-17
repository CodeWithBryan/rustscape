//! Explicit offline equipped appearance packets; this performs no equipment transaction.
use rs09_client::{appearance::IdentityKit, minimap, pack::Pack, ui::item::Catalog};
use rs09_protocol::buffer::Writer;
fn main() {
    let pack = Pack::decode(std::fs::read("assets/visual/ui-players.pack").unwrap()).unwrap();
    let mut items = Catalog::default();
    let mut ids: Vec<i32> = pack
        .manifest
        .files
        .keys()
        .filter_map(|n| n.strip_prefix("item/")?.parse().ok())
        .collect();
    // Equipment wire indices follow original numeric item order.
    ids.sort();
    let types = ids
        .into_iter()
        .map(|id| {
            let t = items.get(&pack, id).unwrap();
            (id, t.manwear, t.womanwear)
        })
        .collect::<Vec<_>>();
    let equipment = minimap::equipment_ids(types);
    for female in [false, true] {
        let mut slots = [0; 12];
        for name in pack.manifest.files.keys().filter(|n| n.starts_with("kit/")) {
            let k = IdentityKit::decode(pack.get(name).unwrap()).unwrap();
            let start = if female { 7 } else { 0 };
            if (start..start + 7).contains(&k.feature) {
                let slot = [8, 11, 4, 6, 9, 7, 10][k.feature - start];
                if slots[slot] == 0 {
                    slots[slot] = 256 + name[4..].parse::<i32>().unwrap();
                }
            }
        }
        for (slot, id) in [
            (0, 1163),
            (1, 6570),
            (2, 1704),
            (3, 1333),
            (4, 1127),
            (5, 1201),
            (7, 1079),
            (9, 7462),
            (10, 3105),
        ] {
            slots[slot] = 32768 + equipment.iter().position(|&n| n == id).unwrap() as i32;
        }
        for slot in [6, 8, 11] {
            slots[slot] = 0;
        }
        let mut w = Writer::new();
        w.p1(i32::from(female));
        w.p1(-1);
        w.p1(-1);
        for n in slots {
            if n == 0 {
                w.p1(0);
            } else {
                w.p2(n);
            }
        }
        for n in [0, 0, 0, 0, 0] {
            w.p1(n);
        }
        w.p2(1426);
        w.p8(rs09_protocol::login::encode_name(b"Gear preview"));
        w.p1(126);
        w.p1(126);
        w.p1(255);
        w.p1(0);
        let name = if female { "female" } else { "male" };
        std::fs::write(format!("runs/visual/player-{name}.bin"), w.as_slice()).unwrap();
    }
    println!("Offline male/female rune equipment appearances prepared.");
}
