use crate::{Result, cache_assets, run};
use rs09_cache::{
    Limits,
    archive::Archive,
    store::{StorageMode, Store},
};
use rs09_server::{
    definitions::scenery as defs,
    world::{
        doors,
        scene::{Assets, CacheAssets, Scene},
    },
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::Path,
};
#[allow(dead_code)]
#[path = "../../oracle/scene/native.rs"]
mod native;
struct Log(Vec<String>);
impl defs::LogContext for Log {
    fn log(&mut self, _: &mut defs::Registry, s: String) -> defs::Result<()> {
        self.0.push(s);
        Ok(())
    }
}
fn export_region(assets: &CacheAssets, id: i32) -> Result<Value> {
    let (x, y) = ((id >> 8) & 255, id & 255);
    let m = assets.archive_id(&format!("m{x}_{y}"))?;
    let l = assets.archive_id(&format!("l{x}_{y}"))?;
    let map = if m >= 0 {
        // The original map path reads the unpacked container. File extraction is
        // identical only for a group containing exactly file zero.
        let info = assets
            .archive
            .index
            .group(m as u32)
            .ok_or("map metadata missing")?;
        if info.files.len() != 1 || info.files[0].id != 0 {
            return Err("terrain group is not a single file zero".into());
        }
        assets.mapscape(m)?.map(|b| native::hex(&b))
    } else {
        None
    };
    let land = if m >= 0 && l >= 0 {
        assets
            .landscape(l, assets.keys(id)?)?
            .map(|b| native::hex(&b))
    } else {
        None
    };
    Ok(json!({"mapGroup":m,"landGroup":l,"map":map,"land":land}))
}
pub fn audit(root: &Path, args: &[String]) -> Result<()> {
    let mut forwarded = vec![args[0].clone()];
    let mut output = None;
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--output" {
            if output.is_some() {
                return Err("duplicate scene audit output".into());
            }
            i += 1;
            output = Some(root.join(args.get(i).ok_or("missing scene audit output directory")?));
        } else {
            forwarded.push(args[i].clone());
        }
        i += 1;
    }
    let isolated = output.is_some();
    let directory = output.unwrap_or_else(|| root.join("runs/scene-oracle"));
    if isolated && directory.exists() && fs::read_dir(&directory)?.next().is_some() {
        return Err(
            "isolated scene audit output must be empty; existing evidence is preserved".into(),
        );
    }
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
    }
    let limits = Limits::default();
    let store = Store::open(
        &cache_assets::directory(root),
        StorageMode::Resident,
        limits,
    )?;
    let definitions_archive = Archive::open(store.clone(), 16, None, limits)?;
    let archive = Archive::open(store.clone(), 5, None, limits)?;
    let mut definitions = defs::Registry::default();
    let mut logs = Log(vec![]);
    let count = definitions.load_archive(&mut logs, &definitions_archive, 1_000_000)?;
    if !logs.0.is_empty() {
        return Err(format!("unexpected definition logs: {:?}", logs.0).into());
    }
    let mut files = BTreeMap::new();
    for id in 0..count as u32 {
        if let Some(data) = definitions_archive.file(id >> 8, id & 255, [0; 4])? {
            files.insert(id.to_string(), native::hex(&data));
        }
    }
    let export_assets = CacheAssets {
        archive: Archive::open(store, 5, None, limits)?,
        keys: keys.clone(),
    };
    let assets = CacheAssets { archive, keys };
    let mut regions = BTreeMap::new();
    let mut request = json!({"definitionCount":count,"definitionFiles":files,"actions":[
        {"op":"load","id":12850,"build":false},
        {"op":"lookup","name":"closed","location":[3207,3217,0]},
        {"op":"door","node":"closed"},
        {"op":"lookup","name":"opened","location":[3207,3218,0]},
        {"op":"tick","count":499},{"op":"tick","count":1},
        {"op":"lookup","name":"restored","location":[3207,3217,0]}
    ]});
    let registry = doors::Registry::default();
    doors::Loader::default()
        .load_text(
            include_str!("../../fixtures/doors/door_configs.json"),
            &registry,
            &mut |_| Ok(()),
        )
        .map_err(|e| format!("{e:?}"))?;
    let scene = Scene::new(assets, definitions, registry);
    let expected = native::evaluate_scene(&request, scene);
    // Border objects can recursively load beyond the immediate 3x3 neighborhood.
    // Supply every region actually reached by the unrestricted native provider;
    // do not truncate the original JVM provider at an artificial radius.
    let ids: BTreeSet<_> = expected
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|s| s["regions"].as_array().unwrap())
        .map(|r| r[0].as_i64().unwrap() as i32)
        .collect();
    for id in ids {
        regions.insert(id.to_string(), export_region(&export_assets, id)?);
    }
    request["cacheRegions"] = json!(regions);
    fs::create_dir_all(&directory)?;
    let input = directory.join("audit-input.json");
    fs::write(&input, serde_json::to_vec(&request)?)?;
    let mut command =
        crate::login_oracle::invocation(root, &forwarded, "oracle/scene/generate.py")?;
    if isolated {
        command.arg("--run-dir").arg(&directory);
    }
    command.arg("--check").arg("--audit-file").arg(&input);
    print!("{}", String::from_utf8(run(&mut command)?.stdout)?);
    let original: Value = serde_json::from_slice(&fs::read(directory.join("audit-output.json"))?)?;
    if original["expected"] != expected {
        fs::write(
            directory.join("native-output.json"),
            serde_json::to_vec(&expected)?,
        )?;
        return Err(
            "pinned Lumbridge scene differs from original; inspect scene-oracle observations"
                .into(),
        );
    }
    let mut wire = vec![];
    let mut batches = vec![];
    let mut tick = 0u64;
    for (i, (action, state)) in request["actions"]
        .as_array()
        .unwrap()
        .iter()
        .zip(expected.as_array().unwrap())
        .enumerate()
    {
        if action["op"] == "tick" {
            tick += action["count"].as_u64().unwrap();
        }
        for f in state["frames"].as_array().unwrap() {
            let bytes = native::bytes(f[1].as_str().unwrap());
            batches.push(json!({"action":i,"tick":tick,"chunk":f[0],"offset":wire.len(),"length":bytes.len(),"sha256":format!("{:x}",Sha256::digest(&bytes))}));
            wire.extend(bytes);
        }
    }
    fs::write(directory.join("lumbridge-door.s2c"), &wire)?;
    fs::write(
        directory.join("lumbridge-door.json"),
        serde_json::to_vec_pretty(
            &json!({"format":1,"direction":"ServerToClient","revision":530,"isaac":false,"lastScene":[3207,3216,0],"initialDoor":{"id":12348,"location":[3207,3217,0],"shape":0,"rotation":1},"batches":batches,"wireSha256":format!("{:x}",Sha256::digest(&wire)),"scope":"Actual strict native pinned cache scene. Native door open and 500-tick restoration frames, byte-compared with original joined server methods. Trace replay, not live world entry."}),
        )?,
    )?;
    let mut report = json!({"format":1,"upstream":upstream,"assetObjects":cache_assets::manifest(root)?,"definitionCount":count,"presentDefinitions":files.len(),"suppliedRegions":regions.len(),"actions":request["actions"],"inputSha256":format!("{:x}",Sha256::digest(serde_json::to_vec(&request)?)),"observationSha256":format!("{:x}",Sha256::digest(serde_json::to_vec(&expected)?)),"keySourceSha256":format!("{:x}",Sha256::digest(crate::normalized(keys_source.as_bytes()))),"oracle":serde_json::from_slice::<Value>(&fs::read(root.join("fixtures/scene/provenance.json"))?)?,"nativeBuild":crate::network_oracle::native_build()?,"nativeSources":crate::network_oracle::native_sources(root)?,"wireBytes":wire.len(),"batches":batches,"differences":0,"scope":"Strict native scenery archive and cache-region provider vs original full definition parse and joined region/scenery/door/collision/wire/restoration methods over real Lumbridge terrain and landscapes. All stored static/dynamic scenery and allocated movement/projectile flags compared at each action. Single-owner region routing, entity/path/audio services, fixed-prefix pulse scheduling and chunk fanout remain explicit bindings. No socket/world entry, movement, sound delivery, dynamic-region or performance/capacity claim."});
    report["nativeSources"]["oracle/scene/native.rs"] = json!(format!(
        "{:x}",
        Sha256::digest(crate::normalized(&fs::read(
            root.join("oracle/scene/native.rs")
        )?))
    ));
    fs::write(
        directory.join("audit.json"),
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    println!(
        "Pinned Lumbridge door scene matches original: {count} definitions, {} supplied cache regions, {} wire bytes in {} batches.",
        regions.len(),
        wire.len(),
        batches.len()
    );
    Ok(())
}
