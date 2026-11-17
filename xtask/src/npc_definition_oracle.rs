use crate::{Result, cache_assets, run};
use rs09_cache::{
    Limits,
    archive::Archive,
    store::{StorageMode, Store},
};
use rs09_server::definitions::npc::{Registry, config};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path};
#[path = "../../oracle/npc-definitions/native.rs"]
mod native;
fn hex(data: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(data.len() * 2);
    for b in data {
        write!(s, "{b:02x}").unwrap();
    }
    s
}
pub fn audit(root: &Path, args: &[String]) -> Result<()> {
    let mut forwarded = vec![args[0].clone()];
    let mut output = None;
    let mut index = 1;
    while index < args.len() {
        if args[index] == "--output" {
            if output.is_some() {
                return Err("duplicate NPC audit output".into());
            }
            index += 1;
            output = Some(
                root.join(
                    args.get(index)
                        .ok_or("missing NPC audit output directory")?,
                ),
            );
        } else {
            forwarded.push(args[index].clone());
        }
        index += 1;
    }
    cache_assets::verify(root)?;
    let limits = Limits::default();
    let archive = Archive::open(
        Store::open(
            &cache_assets::directory(root),
            StorageMode::Resident,
            limits,
        )?,
        18,
        None,
        limits,
    )?;
    let mut registry = Registry::default();
    let count = registry.load_archive(&archive, 1_000_000)?;
    let mut files = BTreeMap::new();
    let mut raw = Sha256::new();
    let mut bytes = 0usize;
    for id in 0..count as u32 {
        if let Some(data) = archive.file(id >> 7, id & 127, [0; 4])? {
            raw.update(id.to_be_bytes());
            raw.update((data.len() as u32).to_be_bytes());
            raw.update(&*data);
            bytes += data.len();
            files.insert(id.to_string(), hex(&data));
        }
    }
    let request = json!({"kind":"sequence","initial":[],"actions":[{"op":"load","files":files},{"op":"preload","count":count},{"op":"config"}]});
    let expected = native::evaluate(&request);
    let context = native::Model::default();
    if expected[1]["error"] != Value::Null
        || native::state(&registry, &context) != expected[1]["state"]
    {
        return Err("Native NPC archive loader and lazy file-input parser disagree".into());
    }
    // Independently apply the production parser to the public archive loader.
    struct Context;
    impl rs09_server::definitions::npc::Files for Context {
        fn file(
            &mut self,
            _: &mut Registry,
            _: i32,
        ) -> rs09_server::definitions::npc::Result<Option<Vec<u8>>> {
            Ok(None)
        }
    }
    impl config::Context for Context {
        fn log(
            &mut self,
            _: &mut Registry,
            _: config::Level,
            _: String,
        ) -> rs09_server::definitions::npc::Result<()> {
            Ok(())
        }
    }
    let config = fs::read_to_string(root.join("fixtures/npc-definitions/npc_configs.json"))?;
    let rows = config::Parser::default().load_text(&config, &mut registry, &mut Context)?;
    let defs: BTreeMap<_, _> = registry
        .definitions
        .iter()
        .map(|(id, h)| (id.to_string(), serde_json::to_value(&*h.read()).unwrap()))
        .collect();
    if expected[2]["error"] != Value::Null || json!(defs) != expected[2]["state"]["definitions"] {
        return Err("Native NPC configuration and public archive data disagree".into());
    }
    let directory = output.unwrap_or_else(|| root.join("runs/npc-definition-oracle"));
    fs::create_dir_all(&directory)?;
    let input = directory.join("audit-input.json");
    fs::write(&input, serde_json::to_vec(&request)?)?;
    let mut command =
        crate::login_oracle::invocation(root, &forwarded, "oracle/npc-definitions/generate.py")?;
    command
        .env("RS09_NPC_DEFINITION_ORACLE_RUN", &directory)
        .arg("--check")
        .arg("--audit-file")
        .arg(&input);
    print!("{}", String::from_utf8(run(&mut command)?.stdout)?);
    let original: Value = serde_json::from_slice(&fs::read(directory.join("audit-output.json"))?)?;
    if original["expected"] != expected {
        fs::write(
            directory.join("native-output.json"),
            serde_json::to_vec(&expected)?,
        )?;
        return Err(
            "Full NPC archive/configuration differs from original; inspect NPC audit outputs"
                .into(),
        );
    }
    let report = json!({"format":1,"upstream":crate::verify_reference(root)?,"assetObjects":cache_assets::manifest(root)?,"definitionCount":count,"presentFiles":files.len(),"decodedInputBytes":bytes,"rawFileSha256":format!("{:x}",raw.finalize()),"configurationRows":rows,"registrySize":registry.definitions.len(),"observationSha256":format!("{:x}",Sha256::digest(serde_json::to_vec(&expected)?)),"differences":0,"nativeBuild":crate::network_oracle::native_build()?,"nativeSources":crate::network_oracle::native_sources(root)?,"scope":"All pinned server archive18 definitions before/after full pinned NPC configuration. Complete original NPCDefinition/Definition/NPCConfigParser compiled; all stored scalar/array/map fields and outer aliases compared except opaque dropTables service. Explicit cache, varbit/player, enum-value metadata and world services. No full NPC behavior/drop/combat/content/world or performance claim."});
    fs::write(
        directory.join("audit.json"),
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    println!(
        "All {count} NPC definitions and {rows} original configuration rows match JVM fields/aliases; {} files, {bytes} raw bytes.",
        files.len()
    );
    Ok(())
}
