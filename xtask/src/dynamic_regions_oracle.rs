//! `cargo xtask dynamic-regions-oracle [--check]`: export the real cache regions,
//! scenery definitions and XTEA keys the corpus reaches, then run the JVM oracle.
//!
//! The regions reached are found by the original itself: the oracle is run in
//! discovery mode with the regions exported so far and names every map or
//! landscape archive it looked up but was not given; those are exported and the
//! run repeated until nothing is missing. Definitions are every object id of the
//! exported landscapes with the transitive closure of their children.
use crate::{Result, cache_assets, run};
use rs09_cache::{
    Limits,
    archive::Archive,
    store::{StorageMode, Store},
};
use rs09_protocol::buffer::Reader;
use rs09_server::{
    definitions::scenery as defs,
    world::scene::{Assets, CacheAssets},
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::Path,
};

struct Log(Vec<String>);
impl defs::LogContext for Log {
    fn log(&mut self, _: &mut defs::Registry, s: String) -> defs::Result<()> {
        self.0.push(s);
        Ok(())
    }
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn smart(r: &mut Reader<'_>) -> Result<i32> {
    let first = r.g1()?;
    Ok(if first <= 127 {
        first
    } else {
        (first << 8 | r.g1()?) - 32768
    })
}
fn big_smart(r: &mut Reader<'_>) -> Result<i32> {
    let mut value = 0i32;
    let mut current = smart(r)?;
    while current == 32767 {
        current = smart(r)?;
        value = value.wrapping_add(32767);
    }
    Ok(value.wrapping_add(current))
}
/// The object ids of a landscape stream, as LandscapeParser walks it.
fn landscape_ids(bytes: &[u8], out: &mut BTreeSet<i32>) -> Result<()> {
    let mut r = Reader::new(bytes);
    let mut id = -1i32;
    loop {
        let offset = big_smart(&mut r)?;
        if offset == 0 {
            return Ok(());
        }
        id = id.wrapping_add(offset);
        out.insert(id);
        loop {
            let offset = smart(&mut r)?;
            if offset == 0 {
                break;
            }
            r.g1()?;
        }
    }
}
fn export_region(assets: &CacheAssets, id: i32, objects: &mut BTreeSet<i32>) -> Result<Value> {
    let (x, y) = ((id >> 8) & 255, id & 255);
    let m = assets.archive_id(&format!("m{x}_{y}"))?;
    let l = assets.archive_id(&format!("l{x}_{y}"))?;
    let map = if m >= 0 {
        // The original reads the unpacked container; file extraction is the
        // same only for a group holding exactly file zero.
        let info = assets
            .archive
            .index
            .group(m as u32)
            .ok_or("map metadata missing")?;
        if info.files.len() != 1 || info.files[0].id != 0 {
            return Err("terrain group is not a single file zero".into());
        }
        assets.mapscape(m)?.map(|b| hex(&b))
    } else {
        None
    };
    let land = if l >= 0 {
        let bytes = assets.landscape(l, assets.keys(id)?)?;
        if let Some(bytes) = &bytes
            && bytes.len() >= 4
        {
            landscape_ids(bytes, objects)?;
        }
        bytes.map(|b| hex(&b))
    } else {
        None
    };
    Ok(json!({"mapGroup": m, "landGroup": l, "map": map, "land": land}))
}

pub fn generate(root: &Path, args: &[String]) -> Result<()> {
    cache_assets::verify(root)?;
    let upstream = crate::verify_reference(root)?;
    let keys_source = crate::git(
        &root.join("reference/server"),
        &[
            "show",
            &format!(
                "{}:Server/data/configs/xteas.json",
                upstream["server"]["commit"]
                    .as_str()
                    .ok_or("missing server pin")?
            ),
        ],
    )?;
    let key_rows: Value = serde_json::from_str(&keys_source)?;
    let mut keys = HashMap::new();
    let mut key_json = BTreeMap::new();
    for row in key_rows["xteas"]
        .as_array()
        .ok_or("missing pinned XTEA rows")?
    {
        let id: i32 = row["regionId"]
            .as_str()
            .ok_or("missing XTEA region")?
            .parse()?;
        let values: Vec<_> = row["keys"]
            .as_str()
            .ok_or("missing XTEA keys")?
            .split(',')
            .map(str::parse::<i32>)
            .collect::<std::result::Result<_, _>>()?;
        let values: [i32; 4] = values.try_into().map_err(|_| "XTEA key length differs")?;
        keys.insert(id, values.map(|n| n as u32));
        key_json.insert(id.to_string(), json!(values));
    }
    let limits = Limits::default();
    let store = Store::open(
        &cache_assets::directory(root),
        StorageMode::Resident,
        limits,
    )?;
    let definitions_archive = Archive::open(store.clone(), 16, None, limits)?;
    let assets = CacheAssets {
        archive: Archive::open(store, 5, None, limits)?,
        keys,
    };
    let mut registry = defs::Registry::default();
    let mut logs = Log(vec![]);
    registry.load_archive(&mut logs, &definitions_archive, 1_000_000)?;
    if !logs.0.is_empty() {
        return Err(format!("unexpected definition logs: {:?}", logs.0).into());
    }
    let directory = root.join("runs/dynamic-regions-oracle");
    fs::create_dir_all(&directory)?;
    let input = directory.join("cache-input.json");
    let missing_path = directory.join("missing.json");
    let mut regions: BTreeSet<i32> = BTreeSet::new();
    let mut compiled = false;
    let mut forwarded: Vec<String> = vec![args[0].clone()];
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--python" || args[i] == "--java-home" {
            forwarded.push(args[i].clone());
            i += 1;
            forwarded.push(args.get(i).ok_or("missing option value")?.clone());
        } else if args[i] != "--check" {
            return Err(format!("unknown oracle option {}", args[i]).into());
        }
        i += 1;
    }
    let export = |regions: &BTreeSet<i32>| -> Result<Vec<u8>> {
        let mut objects = BTreeSet::new();
        let mut exported = BTreeMap::new();
        for id in regions {
            exported.insert(id.to_string(), export_region(&assets, *id, &mut objects)?);
        }
        let mut definitions = BTreeMap::new();
        let mut pending: Vec<i32> = objects.into_iter().collect();
        let mut seen = BTreeSet::new();
        while let Some(id) = pending.pop() {
            if id < 0 || !seen.insert(id) {
                continue;
            }
            if seen.len() > 200_000 {
                return Err("definition closure exceeds its bound".into());
            }
            if let Some(data) =
                definitions_archive.file((id as u32) >> 8, (id & 255) as u32, [0; 4])?
            {
                definitions.insert(id.to_string(), hex(&data));
            }
            if let Some(h) = registry.definitions.get(&id)
                && let Some(children) = h.read().children_ids.clone()
            {
                pending.extend(children.read().iter().copied());
            }
        }
        Ok(serde_json::to_vec(
            &json!({"format": 1, "regions": exported, "definitions": definitions, "keys": key_json}),
        )?)
    };
    for round in 0.. {
        if round > 64 {
            return Err("cache discovery does not converge".into());
        }
        fs::write(&input, export(&regions)?)?;
        let mut command = crate::login_oracle::invocation(
            root,
            &forwarded,
            "oracle/dynamic-regions/generate.py",
        )?;
        command
            .arg("--cache-input")
            .arg(&input)
            .arg("--discover")
            .arg(&missing_path);
        if compiled {
            command.arg("--skip-compile");
        }
        print!("{}", String::from_utf8(run(&mut command)?.stdout)?);
        compiled = true;
        let missing: Vec<String> = serde_json::from_slice(&fs::read(&missing_path)?)?;
        if missing.is_empty() {
            break;
        }
        let before = regions.len();
        for name in missing {
            let (x, y) = name[1..].split_once('_').ok_or("archive name")?;
            regions.insert((x.parse::<i32>()? << 8) | y.parse::<i32>()?);
        }
        if regions.len() == before {
            return Err("discovery names only regions already exported".into());
        }
    }
    println!("{} cache regions exported.", regions.len());
    let mut command =
        crate::login_oracle::invocation(root, args, "oracle/dynamic-regions/generate.py")?;
    command.arg("--cache-input").arg(&input);
    print!("{}", String::from_utf8(run(&mut command)?.stdout)?);
    Ok(())
}
