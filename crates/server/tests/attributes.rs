use rs09_server::{
    auth::Clock,
    player::attributes::{
        AttributeLocation, AttributeValue, Error, GameAttributes, LegacyLoad, Result,
    },
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicI64, AtomicUsize, Ordering},
    },
};
const CORPUS: &str = include_str!("../../../fixtures/attributes/oracle.jsonl");
fn cases() -> Vec<Value> {
    CORPUS
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
fn value(input: &Value) -> Option<AttributeValue> {
    let data = &input["value"];
    Some(match input["kind"].as_str().unwrap() {
        "int" => AttributeValue::Int(data.as_i64().unwrap() as i32),
        "long" => AttributeValue::Long(data.as_i64().unwrap()),
        "short" => AttributeValue::Short(data.as_i64().unwrap() as i16),
        "byte" => AttributeValue::Byte(data.as_i64().unwrap() as i8),
        "bool" => AttributeValue::Bool(data.as_bool().unwrap()),
        "str" => AttributeValue::Text(data.as_str().unwrap().into()),
        "location" => AttributeValue::Location(AttributeLocation::new(
            data[0].as_i64().unwrap() as i32,
            data[1].as_i64().unwrap() as i32,
            data[2].as_i64().unwrap() as i32,
        )),
        "object" => AttributeValue::Object(Arc::new(data.as_str().unwrap().to_owned())),
        "null" => return None,
        _ => panic!(),
    })
}
fn snapshot_value(value: Option<&AttributeValue>) -> Value {
    let Some(value) = value else {
        return Value::Null;
    };
    let data = match value {
        AttributeValue::Int(v) => json!(v),
        AttributeValue::Long(v) => json!(v),
        AttributeValue::Short(v) => json!(v),
        AttributeValue::Byte(v) => json!(v),
        AttributeValue::Bool(v) => json!(v),
        _ => json!(value.java_string()),
    };
    json!({"kind":if matches!(value,AttributeValue::Object(_)){"object"}else{value.saved_type()},"value":data})
}
fn snapshot(attrs: &GameAttributes) -> Value {
    let sort = |a: &Option<String>, b: &Option<String>| {
        a.as_deref()
            .map(|s| s.encode_utf16().collect::<Vec<_>>())
            .cmp(&b.as_deref().map(|s| s.encode_utf16().collect::<Vec<_>>()))
    };
    let mut values = attrs
        .values
        .iter()
        .map(|(key, value)| (Some(key.clone()), value.as_ref()))
        .collect::<Vec<_>>();
    if let Some(value) = &attrs.null_value {
        values.push((None, value.as_ref()));
    }
    values.sort_by(|a, b| sort(&a.0, &b.0));
    let mut expiry = attrs
        .expirations
        .iter()
        .map(|(key, value)| (Some(key.clone()), *value))
        .collect::<Vec<_>>();
    if let Some(expiry_value) = attrs.null_expiration {
        expiry.push((None, expiry_value));
    }
    expiry.sort_by(|a, b| sort(&a.0, &b.0));
    json!({"values":values.into_iter().map(|(key,value)|json!([key,snapshot_value(value)])).collect::<Vec<_>>(),"saved":attrs.saved,"expirations":expiry})
}
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let root =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../runs/attributes-native-tests");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join(format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.0.join("player.xml"));
        std::fs::remove_dir(&self.0).unwrap();
    }
}
fn attempt(result: Result<Value>) -> Value {
    match result {
        Ok(value) => json!({"ok":value}),
        Err(error) => {
            json!({"error":match error{Error::Null=>"null",Error::Type=>"type",Error::Number=>"number",Error::Index=>"index",Error::Base64=>"base64",_=>panic!("Unexpected propagated XML/I/O error")}})
        }
    }
}
fn evaluate(case: &Value, directory: &Directory) -> Value {
    let mut attrs = GameAttributes::default();
    let now = Arc::new(AtomicI64::new(case["now"].as_i64().unwrap()));
    let reads = Arc::new(AtomicUsize::new(0));
    let step = case["step"].as_i64().unwrap();
    let counted = Arc::clone(&reads);
    let observed = Arc::clone(&now);
    let clock: Arc<Clock> = Arc::new(move || {
        counted.fetch_add(1, Ordering::Relaxed);
        observed.fetch_add(step, Ordering::Relaxed)
    });
    let prefix = format!("{}{}", directory.0.display(), std::path::MAIN_SEPARATOR);
    let file = directory.0.join("player.xml");
    if file.exists() {
        std::fs::remove_file(&file).unwrap();
    }
    let output=case["actions"].as_array().unwrap().iter().map(|action|{
        let mut root=serde_json::Map::new();let mut warnings=Vec::new();let mut legacy_error=false;
        let result=(||->Result<Value>{match action["op"].as_str().unwrap(){
            "set"=>attrs.set(action["key"].as_str().unwrap(),value(&action["value"])),
            "raw"=>{let value=value(&action["value"]);if let Some(key)=action["key"].as_str(){attrs.values.insert(key.into(),value);}else{attrs.null_value=Some(value);}},
            "saved"=>attrs.saved.push(action["key"].as_str().map(str::to_owned)),
            "expiry"=>{let expiry=action["value"].as_i64();if let Some(key)=action["key"].as_str(){attrs.expirations.insert(key.into(),expiry);}else{attrs.null_expiration=Some(expiry);}},
            "get"=>return Ok(snapshot_value(attrs.get(action["key"].as_str().unwrap()))),
            "get_or"=>{let fallback=value(&action["value"]).unwrap();return Ok(snapshot_value(Some(attrs.get_or(action["key"].as_str().unwrap(),&fallback))));},
            "remove"=>attrs.remove(action["key"].as_str().unwrap()),
            "save"=>attrs.save_json_with_log(&mut root,&mut|message|warnings.push(message.to_owned())),
            "json"=>{let outcome=attrs.parse_json_with_log(&action["root"],&*clock,&prefix,"player",&mut|message|warnings.push(message.to_owned()))?;legacy_error=matches!(outcome,LegacyLoad::Failed(_));},
            "json_roundtrip"=>{let mut saved=serde_json::Map::new();attrs.save_json_with_log(&mut saved,&mut|message|warnings.push(message.to_owned()));let outcome=attrs.parse_json_with_log(&Value::Object(saved),&*clock,&prefix,"player",&mut|message|warnings.push(message.to_owned()))?;legacy_error=matches!(outcome,LegacyLoad::Failed(_));},
            "xml"=>{std::fs::write(&file,action["bytes"].as_str().unwrap().as_bytes().chunks_exact(2).map(|pair|u8::from_str_radix(std::str::from_utf8(pair).unwrap(),16).unwrap()).collect::<Vec<_>>()).unwrap();legacy_error=matches!(attrs.parse_legacy_file(&prefix,"player.xml"),LegacyLoad::Failed(_));},
            "dump"=>attrs.dump("unused.xml"),
            "alias_location"=>{let AttributeValue::Location(location)=attrs.get(action["key"].as_str().unwrap()).unwrap()else{panic!()};let alias=location.clone();let coords=&action["value"];alias.set(coords[0].as_i64().unwrap()as i32,coords[1].as_i64().unwrap()as i32,coords[2].as_i64().unwrap()as i32);},_=>panic!()};Ok(Value::Null)})();
        json!({"result":attempt(result),"state":snapshot(&attrs),"root":root,"warnings":warnings,"legacyError":legacy_error,"clockReads":reads.load(Ordering::Relaxed),"clock":now.load(Ordering::Relaxed)})
    }).collect::<Vec<_>>();
    json!(output)
}
#[test]
fn attribute_store_profile_roundtrips_and_legacy_xml_match_original() {
    let directory = Directory::new();
    for (index, case) in cases().iter().enumerate() {
        let actual = evaluate(case, &directory);
        if actual != case["expected"] {
            let actual_steps = actual.as_array().unwrap();
            let expected = case["expected"].as_array().unwrap();
            let step = actual_steps
                .iter()
                .zip(expected)
                .position(|(a, b)| a != b)
                .unwrap();
            panic!(
                "attribute case {index}, step {step}, action {}, actual {}, expected {}",
                case["actions"][step], actual_steps[step], expected[step]
            );
        }
    }
}
#[test]
fn opaque_attribute_objects_keep_identity_and_render_their_current_state() {
    struct Counter(Arc<AtomicI64>);
    impl std::fmt::Display for Counter {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "counter={}", self.0.load(Ordering::Relaxed))
        }
    }
    let counter = Arc::new(AtomicI64::new(1));
    let object = Arc::new(Counter(Arc::clone(&counter)));
    let mut attrs = GameAttributes::default();
    attrs.set(
        "/save:counter",
        Some(AttributeValue::Object(object.clone())),
    );
    counter.store(2, Ordering::Relaxed);
    let AttributeValue::Object(stored) = attrs.get("counter").unwrap() else {
        panic!()
    };
    assert!(stored.as_any().is::<Counter>());
    let mut saved = serde_json::Map::new();
    attrs.save_json(&mut saved);
    assert_eq!(saved["attributes"][0]["value"], "counter=2");
}
#[test]
fn attributes_pin_original_store_json_methods_location_data_and_explicit_adapters() {
    let metadata: Value =
        serde_json::from_str(include_str!("../../../fixtures/attributes/provenance.json")).unwrap();
    assert_eq!(
        metadata["fixtureSha256"],
        format!("{:x}", Sha256::digest(CORPUS.as_bytes()))
    );
    assert_eq!(
        metadata["caseCount"].as_u64().unwrap() as usize,
        CORPUS.lines().count()
    );
    assert_eq!(
        metadata["upstream"],
        serde_json::from_str::<Value>(include_str!("../../../upstream.lock.json")).unwrap()
    );
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (path, expected) in metadata["sources"].as_object().unwrap() {
        if path.starts_with("oracle/") {
            let bytes = std::fs::read(root.join(path))
                .unwrap()
                .into_iter()
                .filter(|&b| b != b'\r')
                .collect::<Vec<_>>();
            assert_eq!(*expected, format!("{:x}", Sha256::digest(bytes)), "{path}");
        }
    }
}
