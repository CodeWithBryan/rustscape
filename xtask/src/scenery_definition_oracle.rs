use crate::{Result, cache_assets, run};
use rs09_cache::{
    Limits,
    archive::Archive,
    store::{StorageMode, Store},
};
use rs09_server::definitions::scenery::Registry;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
#[path = "../../oracle/scenery-definitions/native.rs"]
mod native;
fn archive(root: &Path) -> Result<Archive> {
    cache_assets::verify(root)?;
    let limits = Limits::default();
    Ok(Archive::open(
        Store::open(
            &cache_assets::directory(root),
            StorageMode::Resident,
            limits,
        )?,
        16,
        None,
        limits,
    )?)
}
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        write!(out, "{b:02x}").unwrap();
    }
    out
}
pub fn samples(root: &Path) -> Result<()> {
    let archive = archive(root)?;
    let mut ids: BTreeSet<u32> = [
        0, 1, 10325, 10327, 11196, 11382, 12348, 12349, 12350, 2431, 29292, 31017, 36844, 36845,
        36846, 36847, 36848,
    ]
    .into_iter()
    .collect();
    for g in &archive.index.groups {
        if let Some(f) = g.files.first() {
            ids.insert((g.id << 8) | f.id);
        }
    }
    let mut samples = BTreeMap::new();
    for id in ids {
        if let Some(data) = archive.file(id >> 8, id & 255, [0; 4])? {
            samples.insert(id.to_string(), hex(&data));
        }
    }
    fs::write(
        root.join("oracle/scenery-definitions/cache-samples.json"),
        format!("{}\n", serde_json::to_string_pretty(&samples)?),
    )?;
    println!(
        "Authored {} pinned scenery cache samples; regenerate/review the fixtures.",
        samples.len()
    );
    Ok(())
}
pub fn audit(root: &Path, args: &[String]) -> Result<()> {
    let archive = archive(root)?;
    let mut registry = Registry::default();
    let mut context = native::Model::default();
    let count = registry.load_archive(&mut context, &archive, 1_000_000)?;
    let mut files = BTreeMap::new();
    let mut bytes = 0usize;
    let mut raw = Sha256::new();
    for id in 0..count as u32 {
        if let Some(data) = archive.file(id >> 8, id & 255, [0; 4])? {
            raw.update(id.to_be_bytes());
            raw.update((data.len() as u32).to_be_bytes());
            raw.update(&*data);
            bytes += data.len();
            files.insert(id.to_string(), hex(&data));
        }
    }
    let request = json!({"kind":"sequence","initial":[],"actions":[{"op":"load","count":count,"files":files}]});
    let expected = json!([{"result":null,"error":null,"state":native::state(&registry,&context),"events":native::events(&context)}]);
    if native::evaluate(&request) != expected {
        return Err("scenery archive loader and file-input parser disagree".into());
    }
    let directory = root.join("runs/scenery-definition-oracle");
    fs::create_dir_all(&directory)?;
    let input = directory.join("audit-input.json");
    fs::write(&input, serde_json::to_vec(&request)?)?;
    let mut command =
        crate::login_oracle::invocation(root, args, "oracle/scenery-definitions/generate.py")?;
    command.arg("--check").arg("--audit-file").arg(&input);
    print!("{}", String::from_utf8(run(&mut command)?.stdout)?);
    let original: Value = serde_json::from_slice(&fs::read(directory.join("audit-output.json"))?)?;
    if original["expected"] != expected {
        fs::write(
            directory.join("native-output.json"),
            serde_json::to_vec(&expected)?,
        )?;
        return Err("full scenery archive differs from original; inspect scenery-definition-oracle observations".into());
    }
    let mut report = json!({"format":1,"upstream":crate::verify_reference(root)?,"assetObjects":cache_assets::manifest(root)?,"definitionCount":count,"registrySize":registry.definitions.len(),"presentFiles":files.len(),"missingFiles":count-files.len(),"decodedInputBytes":bytes,"rawFileSha256":format!("{:x}",raw.finalize()),"observationSha256":format!("{:x}",Sha256::digest(serde_json::to_vec(&expected)?)),"logs":native::events(&context),"oracle":serde_json::from_slice::<Value>(&fs::read(root.join("fixtures/scenery-definitions/provenance.json"))?)?,"nativeBuild":crate::network_oracle::native_build()?,"nativeSources":crate::network_oracle::native_sources(root)?,"differences":0,"scope":"Entire pinned server scenery archive16 before content configuration. Original complete SceneryDefinition/Definition classes and strict native public archive loader compared for all instance fields, registry entries and array/map/definition aliases. Original decoder stop/log behavior is preserved; logs are included. Cache decompression has a separate full JVM audit. No completed Node/Scenery, content/plugins, varbit cache, map/region/world/session or performance claim."});
    for path in [
        "oracle/scenery-definitions/native.rs",
        "oracle/scenery-definitions/fields.rs",
    ] {
        report["nativeSources"][path] = json!(format!(
            "{:x}",
            Sha256::digest(crate::normalized(&fs::read(root.join(path))?))
        ));
    }
    fs::write(
        directory.join("audit.json"),
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    println!(
        "All {count} native scenery definitions match original JVM fields/aliases; {} files, {bytes} bytes, {} registry entries.",
        files.len(),
        registry.definitions.len()
    );
    Ok(())
}
