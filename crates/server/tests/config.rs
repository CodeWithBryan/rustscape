use rs09_server::config::{Configuration, parse_location, parse_path};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
const CORPUS: &str = include_str!("../../../fixtures/config/oracle.jsonl");
fn cases(kind: &str) -> Vec<Value> {
    CORPUS
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|case| case["kind"] == kind)
        .collect()
}
fn snapshot(config: &Configuration) -> Value {
    json!({"constants":config.constants(),"world":config.world().map(|world|world.values()),"worldString":config.world().map(|world|world.summary()),"tomlLoaded":config.toml_loaded(),"personalizedShops":config.boolean("world.personalized_shops",false).map_or(json!("error"),|value|json!(value)),"listenerPorts":config.listener_ports()})
}
#[test]
fn configuration_fixtures_pin_whole_original_classes_and_unmodified_default_file() {
    let metadata: Value =
        serde_json::from_str(include_str!("../../../fixtures/config/provenance.json")).unwrap();
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
    assert_eq!(
        ["config", "path", "location"]
            .iter()
            .map(|kind| cases(kind).len())
            .sum::<usize>(),
        CORPUS.lines().count()
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for (path, expected) in metadata["sources"].as_object().unwrap() {
        if path.starts_with("oracle/") || path.starts_with("fixtures/") {
            let bytes = std::fs::read(root.join(path))
                .unwrap()
                .into_iter()
                .filter(|&b| b != b'\r')
                .collect::<Vec<_>>();
            assert_eq!(*expected, format!("{:x}", Sha256::digest(bytes)), "{path}");
        }
    }
}
#[test]
fn full_configuration_assignments_defaults_errors_and_reparse_state_match_original() {
    for (index, case) in cases("config").iter().enumerate() {
        let mut config = Configuration::default();
        let mut actual = Vec::new();
        for action in case["actions"].as_array().unwrap() {
            let result = config.apply(
                action["text"].as_str().unwrap(),
                '\\',
                Some(action["javaVersion"].as_str().unwrap_or("11.0.32")),
            );
            actual.push(json!({"error":result.is_err(),"exit":result.err().and_then(|error|error.reference_exit_code()),"state":snapshot(&config)}));
        }
        let expected = case["expected"].as_array().unwrap();
        assert_eq!(actual.len(), expected.len());
        for (step, (actual, expected)) in actual.iter().zip(expected).enumerate() {
            if actual != expected {
                let differences = actual["state"]["constants"]
                    .as_object()
                    .unwrap()
                    .iter()
                    .filter(|(key, value)| **value != expected["state"]["constants"][*key])
                    .map(|(key, value)| json!([key, value, expected["state"]["constants"][key]]))
                    .collect::<Vec<_>>();
                panic!(
                    "config case {index} step {step}: error/exit actual={}/{} expected={}/{}; constant differences={differences:?}; world actual={} expected={}; loaded={}/{}",
                    actual["error"],
                    actual["exit"],
                    expected["error"],
                    expected["exit"],
                    actual["state"]["world"],
                    expected["state"]["world"],
                    actual["state"]["tomlLoaded"],
                    expected["state"]["tomlLoaded"]
                );
            }
        }
    }
}
#[test]
fn path_separators_and_packed_location_construction_match_original() {
    for case in cases("path") {
        assert_eq!(
            parse_path(
                case["text"].as_str().unwrap(),
                case["separator"].as_str().unwrap().chars().next().unwrap()
            ),
            case["expected"].as_str().unwrap()
        );
    }
    for case in cases("location") {
        let result = parse_location(case["text"].as_str().unwrap());
        assert_eq!(result.is_err(), case["error"].as_bool().unwrap(), "{case}");
        if let Ok(value) = result {
            assert_eq!(json!(value), case["expected"], "{case}");
        }
    }
}
#[test]
fn shipped_configuration_projects_the_original_authentication_policy() {
    let mut config = Configuration::default();
    config
        .apply(
            include_str!("../../../fixtures/config/default.conf"),
            '\\',
            None,
        )
        .unwrap();
    let auth = config.auth_settings();
    assert!(!auth.use_auth);
    assert!(!auth.persist_accounts);
    assert!(auth.default_admin);
    assert_eq!(auth.database.address.as_deref(), Some("127.0.0.1"));
    assert_eq!(config.world().unwrap().world_id(), 1);
    assert_eq!(config.constants()["START_LOCATION"], json!([3094, 3107, 0]));
    assert_eq!(config.constants()["JAVA_VERSION"], json!(11));
}

#[test]
#[cfg(windows)]
fn native_file_loading_preserves_reference_filename_paths_and_reports_missing_files() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let config = Configuration::load(&root.join("fixtures/config/default.conf")).unwrap();
    assert_eq!(config.world().unwrap().world_id(), 1);
    assert_eq!(config.constants()["CACHE_PATH"], json!("data\\cache\\"));
    let missing = root.join("fixtures/config/native-config-missing-file.conf");
    assert!(!missing.exists());
    let error = Configuration::load(&missing).err().unwrap();
    assert_eq!(error.kind, rs09_server::config::ErrorKind::FileMissing);
    assert_eq!(error.reference_exit_code(), Some(0));
}
