//! Sweeps of the pinned cache for the actor features of `actor_turning`,
//! `crowd` and `recolor`. These read `assets/cache`; without it they pass
//! without establishing anything (and say so).
#![cfg(not(target_arch = "wasm32"))]
use rs09_cache::{
    Limits,
    archive::Archive,
    store::{StorageMode, Store},
};
use rs09_client::{model::Model, npc, recolor::Palette, walking::PlayerBas};
use std::{collections::BTreeMap, path::Path};

fn archives() -> Option<[Archive; 3]> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/cache");
    if !path.join("main_file_cache.dat2").exists() {
        eprintln!("assets/cache absent: the cache sweep verified nothing");
        return None;
    }
    let limits = Limits::default();
    let store = Store::open(&path, StorageMode::File, limits).unwrap();
    Some([2, 18, 7].map(|id| Archive::open(store.clone(), id, None, limits).unwrap()))
}

#[test]
fn cached_npc_types_with_palettes_assemble_and_follow_the_script_palette() {
    let Some([configs, npcs, models]) = archives() else {
        return;
    };
    let (mut total, mut palettes, mut top, mut accelerated) = (0, Vec::new(), Vec::new(), 0);
    let mut recoloured = 0;
    for id in 0..32768u32 {
        let Ok(Some(raw)) = npcs.file(id >> 7, id & 127, [0; 4]) else {
            continue;
        };
        let d = npc::Definition::decode(&raw).unwrap_or_else(|e| panic!("npc {id}: {e}"));
        total += 1;
        if d.top_render_priority {
            top.push(id);
        }
        let bas = if d.bas == -1 {
            PlayerBas::default()
        } else {
            let raw = configs
                .file(32, d.bas as u32, [0; 4])
                .unwrap()
                .unwrap_or_else(|| panic!("npc {id}: BAS {} absent", d.bas));
            PlayerBas::decode(&raw).unwrap_or_else(|e| panic!("npc {id} bas {}: {e}", d.bas))
        };
        if bas.turning != Default::default() {
            accelerated += 1;
        }
        if d.palette.is_empty() {
            continue;
        }
        palettes.push((id, d.name.clone(), d.palette.clone(), d.children.is_some()));
        if d.children.is_some() {
            continue;
        }
        // The body that was refused before: built with the palette as it is
        // before any script runs, and again after CS2 5401 set its entries.
        let mut parts = BTreeMap::new();
        for &model in d.models.iter().filter(|&&m| m != u32::MAX) {
            let raw = models.file(model, 0, [0; 4]).unwrap().unwrap();
            parts.insert(model, Model::decode(&raw).unwrap());
        }
        let before = d.assemble_with_bas(&parts, &bas).unwrap();
        let mut palette = Palette::default();
        for &index in &d.palette {
            palette.set(i32::from(index), 0xff0000).unwrap();
        }
        let after = d
            .assemble_with_palette(&parts, &bas, &palette.colors)
            .unwrap();
        assert_eq!(before.vertices, after.vertices, "npc {id}");
        // A recolour only shows when the model has a face of the source
        // colour; where it does, the palette must reach it.
        let reaches = d
            .recolor
            .iter()
            .take(d.palette.len())
            .any(|pair| parts.values().any(|m| m.colors.contains(&pair[0])));
        assert!(before.colors == after.colors || reaches, "npc {id}");
        recoloured += usize::from(before.colors != after.colors);
    }
    println!("{recoloured} palette bodies change colour with the palette");
    assert!(palettes.iter().all(|p| p.3) || recoloured > 0);
    println!("{total} cached NPC types");
    println!("{accelerated} use a BAS record with opcodes 29-37");
    println!(
        "{} have top render priority (opcode 99), first {:?}",
        top.len(),
        &top[..top.len().min(8)]
    );
    println!("{} have palette recolours (opcode 42):", palettes.len());
    for (id, name, palette, morph) in &palettes {
        println!(
            "  {id} {name:?} palette {palette:?}{}",
            if *morph { " (morph parent)" } else { "" }
        );
    }
    assert!(total > 5000);
    assert_eq!(accelerated, 0);
}
