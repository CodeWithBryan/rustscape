use crate::{Result, cache_assets, run};
use rs09_cache::{
    Limits,
    archive::Archive,
    store::{StorageMode, Store},
};
use rs09_server::definitions::item::Registry;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
#[path = "../../oracle/items/native.rs"]
mod native;
pub fn generate(root: &Path, args: &[String]) -> Result<()> {
    let output = run(&mut crate::login_oracle::invocation(
        root,
        args,
        "oracle/items/generate.py",
    )?)?;
    print!("{}", String::from_utf8(output.stdout)?);
    Ok(())
}

fn archive(root: &Path) -> Result<Archive> {
    cache_assets::verify(root)?;
    let limits = Limits::default();
    Ok(Archive::open(
        Store::open(
            &cache_assets::directory(root),
            StorageMode::Resident,
            limits,
        )?,
        19,
        None,
        limits,
    )?)
}
pub fn samples(root: &Path) -> Result<()> {
    let archive = archive(root)?;
    let mut ids: BTreeSet<u32> = [
        0, 1, 2, 3, 6, 10, 88, 315, 316, 526, 527, 536, 537, 995, 1323, 2513, 3140, 4151, 4152,
        5614, 6106, 8610, 9765, 10446, 11126, 11967, 13659, 13660, 13845, 14076, 14958,
    ]
    .into_iter()
    .collect();
    for group in &archive.index.groups {
        if let Some(file) = group.files.first() {
            ids.insert((group.id << 8) | file.id);
        }
    }
    let mut samples = BTreeMap::new();
    for id in ids {
        if let Some(data) = archive.file(id >> 8, id & 255, [0; 4])? {
            samples.insert(id.to_string(), hex(&data));
        }
    }
    fs::write(
        root.join("oracle/items/cache-samples.json"),
        format!("{}\n", serde_json::to_string_pretty(&samples)?),
    )?;
    println!(
        "Wrote {} pinned item-cache samples; regenerate and review item fixtures.",
        samples.len()
    );
    Ok(())
}
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}").unwrap();
    }
    output
}
pub fn audit(root: &Path, args: &[String]) -> Result<()> {
    let mut forwarded = vec![args[0].clone()];
    let mut output = None;
    let mut index = 1;
    while index < args.len() {
        if args[index] == "--output" {
            if output.is_some() {
                return Err("duplicate item audit output".into());
            }
            index += 1;
            output = Some(
                root.join(
                    args.get(index)
                        .ok_or("missing item audit output directory")?,
                ),
            );
        } else {
            forwarded.push(args[index].clone());
        }
        index += 1;
    }
    let archive = archive(root)?;
    let mut registry = Registry::default();
    let count = registry.load_archive(&archive, 1_000_000)?;
    let mut files = BTreeMap::new();
    let mut byte_count = 0usize;
    let mut raw_hash = Sha256::new();
    for id in 0..count as u32 {
        if let Some(data) = archive.file(id >> 8, id & 255, [0; 4])? {
            raw_hash.update(id.to_be_bytes());
            raw_hash.update((data.len() as u32).to_be_bytes());
            raw_hash.update(&*data);
            byte_count += data.len();
            files.insert(id.to_string(), hex(&data));
        }
    }
    let request = json!({"kind":"sequence","initial":[],"actions":[{"op":"load","count":count,"files":files}]});
    // Exercise the public archive loader independently of the shared test command adapter.
    let expected = json!([{"result":null,"error":null,"state":native::state(&registry,None)}]);
    if native::evaluate(&request) != expected {
        return Err("archive loader and file-input parser disagree".into());
    }
    let directory = output.unwrap_or_else(|| root.join("runs/item-oracle"));
    fs::create_dir_all(&directory)?;
    let input = directory.join("audit-input.json");
    fs::write(&input, serde_json::to_vec(&request)?)?;
    let mut command =
        crate::login_oracle::invocation(root, &forwarded, "oracle/items/generate.py")?;
    command
        .env("RS09_ITEM_ORACLE_RUN", &directory)
        .arg("--check")
        .arg("--audit-file")
        .arg(&input);
    print!("{}", String::from_utf8(run(&mut command)?.stdout)?);
    let observed: Value = serde_json::from_slice(&fs::read(directory.join("audit-output.json"))?)?;
    if observed["expected"] != expected {
        fs::write(
            directory.join("native-output.json"),
            serde_json::to_vec(&expected)?,
        )?;
        return Err(format!(
            "full item archive differs from original; inspect {}",
            directory.display()
        )
        .into());
    }
    let records = expected[0]["state"]["definitions"]
        .as_object()
        .ok_or("missing item records")?;
    let count_flag = |key: &str| {
        records
            .values()
            .filter(|record| record[key].as_i64().is_some_and(|id| id != -1))
            .count()
    };
    let mut report = json!({"format":1,"upstream":crate::verify_reference(root)?,"assetObjects":cache_assets::manifest(root)?,
        "scope":"Entire pinned server item-cache archive before SQL/content configuration. Original parse/template methods compared field-for-field, including aliases, against the public native strict archive loader. Cache decompression has a separate whole-cache JVM audit. No complete item class, plugin, equipment, container, profile, gameplay or performance claim.",
        "definitionCount":count,"presentFiles":files.len(),"missingFiles":count-files.len(),"decodedInputBytes":byte_count,
        "rawFileSha256":format!("{:x}",raw_hash.finalize()),"observationSha256":format!("{:x}",Sha256::digest(serde_json::to_vec(&expected)?)),
        "noteTemplates":count_flag("noteTemplateId"),"lendTemplates":count_flag("lendTemplateId"),"recolourTemplates":count_flag("recolourTemplateId"),
        "oracle":serde_json::from_slice::<Value>(&fs::read(root.join("fixtures/items/provenance.json"))?)?,
        "nativeBuild":crate::network_oracle::native_build()?,"nativeSources":crate::network_oracle::native_sources(root)?,"differences":0});
    // Source files outside crates/ and xtask/ also define the observation adapter.
    report["nativeSources"]["oracle/items/native.rs"] = json!(format!(
        "{:x}",
        Sha256::digest(crate::normalized(&fs::read(
            root.join("oracle/items/native.rs")
        )?))
    ));
    fs::write(
        directory.join("audit.json"),
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    println!(
        "All {count} native item definitions match the original JVM; {} files, {byte_count} input bytes, no field or alias differences.",
        files.len()
    );
    Ok(())
}
