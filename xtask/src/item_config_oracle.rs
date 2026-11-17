use crate::{Result, run};
use std::path::Path;
#[path = "../../oracle/item-config/native.rs"]
mod native;
pub fn generate(root: &Path, args: &[String]) -> Result<()> {
    let output = run(&mut crate::login_oracle::invocation(
        root,
        args,
        "oracle/item-config/generate.py",
    )?)?;
    print!("{}", String::from_utf8(output.stdout)?);
    Ok(())
}
pub fn audit(root: &Path, args: &[String]) -> Result<()> {
    let mut forwarded = vec![args[0].clone()];
    let mut output = None;
    let mut index = 1;
    while index < args.len() {
        if args[index] == "--output" {
            if output.is_some() {
                return Err("duplicate item config audit output".into());
            }
            index += 1;
            output = Some(
                root.join(
                    args.get(index)
                        .ok_or("missing item config audit output directory")?,
                ),
            );
        } else {
            forwarded.push(args[index].clone());
        }
        index += 1;
    }
    use rs09_cache::{
        Limits,
        archive::Archive,
        store::{StorageMode, Store},
    };
    use rs09_server::definitions::{
        item::Registry,
        item_config::{Parser, json::Json},
    };
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};
    use std::{collections::BTreeMap, fs, process::Command};
    crate::cache_assets::verify(root)?;
    let limits = Limits::default();
    let archive = Archive::open(
        Store::open(
            &crate::cache_assets::directory(root),
            StorageMode::Resident,
            limits,
        )?,
        19,
        None,
        limits,
    )?;
    let mut definitions = Registry::default();
    let count = definitions.load_archive(&archive, 1_000_000)?;
    let directory = output.unwrap_or_else(|| root.join("runs/item-config-oracle"));
    fs::create_dir_all(&directory)?;
    let original = run(Command::new("git")
        .arg("-C")
        .arg(root.join("reference/server"))
        .args(["show", "HEAD:Server/data/configs/item_configs.json"]))?
    .stdout;
    let config_path = directory.join("item_configs.json");
    fs::write(&config_path, &original)?;
    let text = String::from_utf8(original.clone())?;
    let data = rs09_server::definitions::item_config::json::parse(&text)
        .map_err(|e| format!("pinned JSON: {e:?}"))?;
    let rows = match data {
        Json::Array(rows) => rows.len(),
        _ => return Err("pinned item configuration is not an array".into()),
    };
    let mut initial = BTreeMap::new();
    for (key, handle) in definitions.definitions() {
        let def = handle.read();
        initial.insert(key.to_string(),json!({"id":def.id,"name":def.name,"value":def.value,"examine":def.examine,"handlers":def.handlers}));
    }
    let request = json!({"kind":"load","initial":initial,"steps":[{"text":text}]});
    let mut parser = Parser::default();
    let mut log = native::Log::default();
    let result = parser.load_file(&config_path, &definitions, &mut log);
    let complete = result.is_ok();
    let (error, position) = match result {
        Ok(_) => (None, None),
        Err(e) => {
            let (e, p) = native::error(e);
            (Some(e), p)
        }
    };
    let expected = json!([{"error":error,"position":position,"state":native::state(&parser,&definitions,&[],&log)}]);
    if native::evaluate(&request) != expected {
        return Err("native file/text configuration entry points disagree".into());
    }
    let input = directory.join("audit-input.json");
    fs::write(&input, serde_json::to_vec(&request)?)?;
    let mut command =
        crate::login_oracle::invocation(root, &forwarded, "oracle/item-config/generate.py")?;
    command
        .env("RS09_ITEM_CONFIG_ORACLE_RUN", &directory)
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
        fn difference(a: &Value, b: &Value, path: String) -> Option<String> {
            if a == b {
                return None;
            }
            if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
                for key in a.keys().chain(b.keys()) {
                    if let Some(diff) = difference(
                        a.get(key).unwrap_or(&Value::Null),
                        b.get(key).unwrap_or(&Value::Null),
                        format!("{path}/{key}"),
                    ) {
                        return Some(diff);
                    }
                }
            }
            if let (Some(a), Some(b)) = (a.as_array(), b.as_array()) {
                if a.len() == b.len() {
                    for (i, (a, b)) in a.iter().zip(b).enumerate() {
                        if let Some(diff) = difference(a, b, format!("{path}/{i}")) {
                            return Some(diff);
                        }
                    }
                }
            }
            Some(format!("{path}: native={a:?}, original={b:?}"))
        }
        return Err(format!(
            "pinned item configuration differs: {}",
            difference(&expected, &observed["expected"], String::new()).unwrap()
        )
        .into());
    }
    let mut types = BTreeMap::<String, usize>::new();
    let mut configured = 0;
    for definition in expected[0]["state"]["definitions"]
        .as_object()
        .unwrap()
        .values()
    {
        let handlers = definition["handlers"].as_object().unwrap();
        if handlers.contains_key("id") {
            configured += 1;
        }
        for property in handlers.values() {
            *types
                .entry(property["type"].as_str().unwrap().to_owned())
                .or_default() += 1;
        }
    }
    let mut sources = crate::network_oracle::native_sources(root)?;
    sources.insert(
        "oracle/item-config/native.rs".into(),
        format!(
            "{:x}",
            Sha256::digest(crate::normalized(&fs::read(
                root.join("oracle/item-config/native.rs")
            )?))
        ),
    );
    let report = json!({"format":1,"upstream":crate::verify_reference(root)?,"nativeBuild":crate::network_oracle::native_build()?,"nativeSources":sources,"oracle":serde_json::from_slice::<Value>(&fs::read(root.join("fixtures/item-config/provenance.json"))?)?,
        "configBlob":"e599ff0b5b973a5878673e0a3aff25ce8a883773","configBytes":original.len(),"configSha256":format!("{:x}",Sha256::digest(&original)),"assetObjects":crate::cache_assets::manifest(root)?,"definitionCount":count,"configRows":rows,"registeredConfiguredDefinitions":configured,"completed":complete,"error":error,"propertyTypes":types,"differences":0,"observationSha256":format!("{:x}",Sha256::digest(serde_json::to_vec(&expected)?)),
        "scope":"Complete pinned item_configs.json over all native archive-19 definitions before player/world integration. Actual native file and text loading compared with complete original ItemConfigParser, including typed properties, constructor fields, aliases, requirements ordering and logs. Original initial raw-cache field values have a separate complete definition audit. No complete animation execution, item plugins, equipment/profile/world or performance claim."});
    fs::write(
        directory.join("audit.json"),
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    println!(
        "All {rows} pinned configuration rows compared across {count} definitions; {configured} registered definitions configured, completed={complete}, no observed differences."
    );
    if !complete {
        return Err("original and native pinned configuration both failed; review recorded outcome before claiming a working load".into());
    }
    Ok(())
}
