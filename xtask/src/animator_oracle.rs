use crate::{Result, cache_assets, run};
use rs09_cache::{
    Limits,
    archive::Archive,
    store::{StorageMode, Store},
};
use rs09_server::definitions::animation::{CacheSource, Registry};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path};
#[path = "../../oracle/animator/native.rs"]
mod native;
pub fn audit(root: &Path, args: &[String]) -> Result<()> {
    cache_assets::verify(root)?;
    let limits = Limits::default();
    let archive = Archive::open(
        Store::open(
            &cache_assets::directory(root),
            StorageMode::Resident,
            limits,
        )?,
        20,
        None,
        limits,
    )?;
    let count = archive
        .index
        .groups
        .iter()
        .flat_map(|g| g.files.iter().map(move |f| (g.id << 7) | f.id))
        .max()
        .ok_or("empty animation archive")?
        + 1;
    let registry = Registry::default();
    let mut source = CacheSource(archive);
    let mut files = BTreeMap::new();
    let mut raw = Sha256::new();
    let mut bytes = 0;
    let mut observed = vec![];
    for id in 0..count {
        if let Some(data) = source.0.file(id >> 7, id & 127, [0; 4])? {
            raw.update(id.to_be_bytes());
            raw.update((data.len() as u32).to_be_bytes());
            raw.update(&*data);
            bytes += data.len();
            files.insert(
                id.to_string(),
                data.iter().map(|b| format!("{b:02x}")).collect::<String>(),
            );
        }
        let d = registry.resolve(&mut source, id as i32)?;
        let second = registry.resolve(&mut source, id as i32)?;
        observed.push(json!([id, native::definition(&d.read()), d.same(&second)]));
    }
    let input = json!({"files":files,"actions":[{"op":"audit","count":count}]});
    let expected = native::evaluate(&input);
    if expected[0]["result"] != json!(observed) {
        return Err("strict animation archive source differs from replay input".into());
    }
    let output = if let Some(i) = args.iter().position(|a| a == "--output") {
        std::path::PathBuf::from(args.get(i + 1).ok_or("--output requires DIR")?)
    } else {
        root.join("runs/animator-audit")
    };
    fs::create_dir_all(&output)?;
    let path = output.join("audit-input.json");
    fs::write(&path, serde_json::to_vec(&input)?)?;
    let mut oracle_args = vec![];
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--output" {
            i += 2;
        } else {
            oracle_args.push(args[i].clone());
            i += 1;
        }
    }
    let mut command =
        crate::login_oracle::invocation(root, &oracle_args, "oracle/animator/generate.py")?;
    command.arg("--check").arg("--audit-file").arg(&path);
    print!("{}", String::from_utf8(run(&mut command)?.stdout)?);
    let original: Value = serde_json::from_slice(&fs::read(output.join("audit-output.json"))?)?;
    if original["expected"] != expected {
        fs::write(
            output.join("native-output.json"),
            serde_json::to_vec(&expected)?,
        )?;
        return Err("complete animation archive differs from original JVM observations".into());
    }
    let report = json!({"format":1,"upstream":crate::verify_reference(root)?,"assetObjects":cache_assets::manifest(root)?,"definitionCount":count,"presentFiles":files.len(),"missingFiles":count as usize-files.len(),"rawBytes":bytes,"rawSha256":format!("{:x}",raw.finalize()),"observationsSha256":format!("{:x}",Sha256::digest(serde_json::to_vec(&expected)?)),"nativeBuild":crate::network_oracle::native_build()?,"nativeSources":crate::network_oracle::native_sources(root)?,"differences":0,"scope":"Every archive20 animation definition through original complete server AnimationDefinition and native strict Archive source, every stored field/duration, cached identity and original lookup events. Server high-word precedence is intentional and differs from the RT4 sequence decoder. No gameplay action, rendering, performance or capacity claim."});
    fs::write(
        output.join("report.json"),
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    println!(
        "All {count} server animation definitions match original JVM; {} present files, {bytes} bytes.",
        files.len()
    );
    Ok(())
}
