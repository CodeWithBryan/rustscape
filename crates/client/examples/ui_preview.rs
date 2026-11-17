//! Explicit offline wire fixture for exercising UI containers; never a player save or live world.
use rs09_client::{
    pack::Pack,
    ui::{icon::Icons, item::Catalog},
};
use rs09_protocol::game::{
    Direction,
    updates::{self, ScriptArg, WireItem},
};
fn main() {
    let large_bank = std::env::args().any(|arg| arg == "large-bank");
    let ids = [
        995, 1277, 1171, 1153, 1129, 1095, 1061, 1059, 1351, 590, 1511, 317, 4151, 554, 555, 561,
        556, 557, 560, 563, 565, 566, 379, 385, 2440, 2434, 526, 436,
    ];
    let inventory: Vec<_> = ids
        .iter()
        .enumerate()
        .map(|(i, &id)| {
            Some(WireItem {
                id,
                amount: if i == 0 {
                    1234567
                } else if (13..=21).contains(&i) {
                    250
                } else {
                    1
                },
            })
        })
        .collect();
    let bank: Vec<_> = ids
        .iter()
        .cycle()
        .take(if large_bank { 280 } else { ids.len() })
        .enumerate()
        .map(|(i, &id)| {
            Some(WireItem {
                id,
                amount: (i as i32 + 1) * 123,
            })
        })
        .collect();
    let mut frames = vec![
        updates::windows_pane(746, 0, 0),
        // Normal BankContainer.open() invokes BankInterface.onOpen first.
        updates::interface_events(762 << 16 | 73, 0, 500, 2623486, 1),
        updates::varbit(4893, 1),
        updates::varc(190, 1, 2),
        updates::open_interface(0, 746 << 16 | 6, 2, 762),
        updates::interface_events(763 << 16, 0, 27, 2097790, 3),
        updates::varc(192, 28, 4),
        updates::open_interface(1, 746 << 16 | 76, 5, 763),
        updates::inventory_full(763 << 16 | 64000, 93, &inventory).unwrap(),
        updates::inventory_full(762 << 16 | 64000, 95, &bank).unwrap(),
        updates::varc(192, 28, 3),
        updates::varp(1249, 0),
    ];
    // Explicit offline game-frame setup from InterfaceManager's default panes.
    let mut bootstrap = vec![updates::windows_pane(746, 0, 0)];
    for (slot, group) in [
        92, 320, 274, 149, 387, 271, 192, -1, 550, 551, 589, 261, 464, 187, 182,
    ]
    .into_iter()
    .enumerate()
    {
        if group >= 0 {
            bootstrap.push(updates::open_interface(
                1,
                746 << 16 | (93 + slot as i32),
                slot as i32 + 1,
                group,
            ));
        }
    }
    for (child, group) in [
        (13, 748),
        (14, 749),
        (15, 750),
        (16, 747),
        (71, 754),
        (23, 751),
        (70, 752),
    ] {
        bootstrap.push(updates::open_interface(1, 746 << 16 | child, 20, group));
    }
    bootstrap.push(updates::open_interface(1, 752 << 16 | 8, 21, 137));
    bootstrap.push(updates::run_client_script(115, b"i", &[ScriptArg::Int(3)], 22).unwrap());
    // Explicit synthetic player inputs delivered through the original packets.
    for skill in 0..24 {
        bootstrap.push(updates::skill_level(
            skill,
            if skill == 3 { 10 } else { 1 },
            if skill == 3 { 1154.0 } else { 0.0 },
        ));
    }
    bootstrap.push(updates::run_energy(100.0));
    let shop = std::env::args().any(|arg| arg == "shop");
    let gameframe = std::env::args().any(|arg| arg == "gameframe");
    if shop {
        frames = vec![
            updates::windows_pane(746, 0, 0),
            updates::interface_text(620 << 16 | 22, b"Lumbridge General Store", 1).unwrap(),
            updates::open_interface(0, 746 << 16 | 6, 2, 620),
            updates::open_interface(1, 746 << 16 | 76, 3, 621),
            updates::interface_events(620 << 16 | 23, 0, 40, 2046, 4),
            updates::interface_events(621 << 16, 0, 28, 2099198, 5),
        ];
        for (component, container, columns, rows, verb) in [
            (620 << 16 | 23, 92, 10, 4, "Buy"),
            (621 << 16, 93, 4, 7, "Sell"),
        ] {
            let mut objects = [
                "",
                "",
                "",
                "",
                &format!("{verb} X"),
                &format!("{verb} 10"),
                &format!("{verb} 5"),
                &format!("{verb} 1"),
                "Value",
            ]
            .into_iter()
            .map(|s| ScriptArg::Text(s.as_bytes().to_vec()))
            .collect::<Vec<_>>();
            objects.extend(
                [-1, 0, rows, columns, container, component]
                    .into_iter()
                    .map(ScriptArg::Int),
            );
            frames.push(updates::run_client_script(150, b"IviiiIsssssssss", &objects, 6).unwrap());
        }
        for (child, hidden) in [
            (23, false),
            (24, true),
            (29, false),
            (25, true),
            (27, true),
            (26, false),
        ] {
            frames.push(updates::interface_hidden(hidden, 7, 620 << 16 | child));
        }
        frames.push(updates::inventory_full(-1, 92, &bank).unwrap());
        frames.push(updates::inventory_full(-1, 93, &inventory).unwrap());
    }
    if gameframe {
        frames.clear();
        frames.push(updates::inventory_full(149 << 16, 93, &inventory).unwrap());
    }
    bootstrap.extend(frames.into_iter().skip(usize::from(!gameframe)));
    let mut bytes = Vec::new();
    for frame in bootstrap {
        frame
            .write(Direction::ServerToClient, None, &mut bytes)
            .unwrap();
    }
    std::fs::create_dir_all("runs/visual").unwrap();
    std::fs::write(
        if large_bank {
            "runs/visual/ui-bank-scroll.bin"
        } else if gameframe {
            "runs/visual/ui-gameframe.bin"
        } else if shop {
            "runs/visual/ui-shop.bin"
        } else {
            "runs/visual/ui-preview.bin"
        },
        bytes,
    )
    .unwrap();
    let ui = Pack::decode(std::fs::read("assets/visual/ui.pack").unwrap()).unwrap();
    let visual = Pack::decode(std::fs::read("assets/visual/demo.pack").unwrap()).unwrap();
    let mut icons = Icons::new(&visual, 1.0).unwrap();
    let mut catalog = Catalog::default();
    let mut atlas = vec![0u8; 36 * 7 * 32 * 4 * 4];
    let mut count = 0;
    for (i, &id) in ids.iter().enumerate() {
        let pixels = icons
            .render(&mut catalog, &ui, &visual, id, 123, 1, 0x302020)
            .unwrap_or_else(|e| panic!("Item {id}: {e}"));
        for y in 0..32 {
            for x in 0..36 {
                let p = pixels[y * 36 + x];
                let off = ((i / 7 * 32 + y) * 36 * 7 + i % 7 * 36 + x) * 4;
                atlas[off..off + 4].copy_from_slice(&[
                    (p >> 16) as u8,
                    (p >> 8) as u8,
                    p as u8,
                    if p == 0 { 0 } else { 255 },
                ]);
            }
        }
        count += 1;
    }
    let mut image = macroquad::texture::Image {
        bytes: atlas,
        width: 36 * 7,
        height: 32 * 4,
    };
    for y in 0..usize::from(image.height) / 2 {
        let row = usize::from(image.width) * 4;
        for x in 0..row {
            image
                .bytes
                .swap(y * row + x, (usize::from(image.height) - 1 - y) * row + x);
        }
    }
    image.export_png("runs/visual/item-icons.png");
    println!("Wrote offline UI replay and {count} native item icons");
}
