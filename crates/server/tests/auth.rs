use rs09_server::auth::{
    AuthError, Authenticator, Mode, Result,
    account::{Account, FieldValue, Timestamp, UserAccountInfo},
    factory::{DatabaseSettings, Provider, Registry, Settings},
    password::{self, PasswordCodec, Passwords},
    storage::{AccountStorage, InMemoryStorage},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicI64, Ordering},
    },
};

const CORPUS: &str = include_str!("../../../fixtures/auth/oracle.jsonl");
fn cases(kind: &str) -> Vec<Value> {
    CORPUS
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|case| case["kind"] == kind)
        .collect()
}
fn value(value: &FieldValue) -> Value {
    match value {
        FieldValue::Text(text) => json!(text),
        FieldValue::Int(n) => json!(n),
        FieldValue::Long(n) => json!(n),
        FieldValue::Bool(b) => json!(b),
        FieldValue::Timestamp(stamp) => {
            json!({"millis":stamp.millis(),"nanos":stamp.value().nanos})
        }
    }
}
fn values(info: &Account) -> Value {
    json!(
        info.lock()
            .unwrap()
            .values()
            .iter()
            .map(value)
            .collect::<Vec<_>>()
    )
}
fn snapshot(info: &Account) -> Value {
    let info = info.lock().unwrap();
    json!({"values":info.values().iter().map(value).collect::<Vec<_>>(),"hash":info.java_hash(),"default":info.is_default(),
        "changed":info.changed_fields().ok().map(|(changed,_)|changed),"initial":info.initial_values.as_ref().map(|fields|fields.iter().map(value).collect::<Vec<_>>())})
}
fn set(info: &Account, index: usize, value: &Value) -> Result<()> {
    let value = match index {
        2..=4 => FieldValue::Int(value.as_i64().unwrap() as i32),
        7 | 8 | 14 | 15 => FieldValue::Long(value.as_i64().unwrap()),
        16 => FieldValue::Bool(value.as_bool().unwrap()),
        17 => {
            let stamp = Timestamp::new(value["millis"].as_i64().unwrap());
            if let Some(nanos) = value.get("nanos") {
                stamp.set_nanos(nanos.as_u64().unwrap() as u32)?;
            }
            FieldValue::Timestamp(stamp)
        }
        _ => FieldValue::Text(value.as_str().unwrap().to_owned()),
    };
    info.lock().unwrap().set_value(index, value)
}
#[derive(Default)]
struct RecordingStorage {
    memory: InMemoryStorage,
    calls: Mutex<Vec<Value>>,
    fail: Mutex<String>,
    time: Option<Arc<AtomicI64>>,
    get_delay: i64,
}
impl RecordingStorage {
    fn call(&self, method: &str, value: Value) -> Result<()> {
        self.calls.lock().unwrap().push(json!([method, value]));
        if *self.fail.lock().unwrap() == method {
            Err(AuthError::Storage)
        } else {
            Ok(())
        }
    }
}
impl AccountStorage for RecordingStorage {
    fn username_taken(&self, name: &str) -> Result<bool> {
        self.call("taken", json!(name))?;
        self.memory.username_taken(name)
    }
    fn get_account_info(&self, name: &str, now: i64) -> Result<Account> {
        self.call("get", json!(name))?;
        let result = self.memory.get_account_info(name, now);
        if let Some(time) = &self.time {
            time.fetch_add(self.get_delay, Ordering::Relaxed);
        }
        result
    }
    fn store(&self, info: Account) -> Result<()> {
        self.call("store", values(&info))?;
        self.memory.store(info)
    }
    fn update(&self, info: Account) -> Result<()> {
        self.call("update", values(&info))?;
        self.memory.update(info)
    }
    fn remove(&self, info: &Account) -> Result<()> {
        let name = info.lock().unwrap().username.clone();
        self.call("remove", json!(name))?;
        self.memory.remove(info)
    }
    fn online_friends(&self, name: &str) -> Result<Vec<String>> {
        self.call("friends", json!(name))?;
        self.memory.online_friends(name)
    }
    fn usernames_with_ip(&self, ip: &str) -> Result<Vec<Option<String>>> {
        self.call("ip", json!(ip))?;
        self.memory.usernames_with_ip(ip)
    }
}
struct TestPasswords {
    fail: Mutex<bool>,
    codec: Passwords,
}
struct TimedPasswords {
    codec: Passwords,
    time: Arc<AtomicI64>,
    hash_delay: i64,
    verify_delay: i64,
    fail: bool,
}
impl PasswordCodec for TimedPasswords {
    fn hash(&self, password: &str) -> Result<String> {
        let result = if self.fail {
            Err(AuthError::Entropy)
        } else {
            self.codec.hash(password)
        };
        self.time.fetch_add(self.hash_delay, Ordering::Relaxed);
        result
    }
    fn verify(&self, password: &str, stored: &str) -> Result<bool> {
        let result = self.codec.verify(password, stored);
        self.time.fetch_add(self.verify_delay, Ordering::Relaxed);
        result
    }
}
impl PasswordCodec for TestPasswords {
    fn hash(&self, password: &str) -> Result<String> {
        if *self.fail.lock().unwrap() {
            Err(AuthError::Entropy)
        } else {
            self.codec.hash(password)
        }
    }
    fn verify(&self, password: &str, stored: &str) -> Result<bool> {
        self.codec.verify(password, stored)
    }
}
fn state(store: &RecordingStorage, records: &BTreeMap<String, Account>) -> Value {
    let entries = store.memory.snapshot_handles();
    let mut all = BTreeMap::new();
    for (key, account) in records {
        all.insert(format!("handle:{key}"), Arc::clone(account));
    }
    for (key, account) in &entries {
        all.insert(format!("stored:{key}"), Arc::clone(account));
    }
    let aliases: Vec<_> = all
        .iter()
        .map(|(name, value)| {
            json!([
                name,
                all.iter()
                    .filter(|(_, other)| Arc::ptr_eq(value, other))
                    .map(|(name, _)| name)
                    .collect::<Vec<_>>()
            ])
        })
        .collect();
    let handles: BTreeMap<_, _> = records
        .iter()
        .map(|(key, account)| (key, snapshot(account)))
        .collect();
    let stored: BTreeMap<_, _> = entries
        .iter()
        .map(|(key, account)| (key, snapshot(account)))
        .collect();
    json!({"handles":handles,"stored":stored,"aliases":aliases,"calls":*store.calls.lock().unwrap()})
}

#[test]
fn clock_sampling_after_password_and_storage_work_matches_original_authenticators() {
    let cases = cases("clock")
        .into_iter()
        .chain(cases("admission"))
        .collect::<Vec<_>>();
    for (index, case) in cases.iter().enumerate() {
        let now = case["now"].as_i64().unwrap();
        let time = Arc::new(AtomicI64::new(now));
        let store = Arc::new(RecordingStorage {
            time: Some(Arc::clone(&time)),
            get_delay: case["getDelay"].as_i64().unwrap(),
            ..Default::default()
        });
        let passwords = Arc::new(TimedPasswords {
            codec: Passwords::with_salt_source(4, || Ok([0; 16])).unwrap(),
            time: Arc::clone(&time),
            hash_delay: case["hashDelay"].as_i64().unwrap(),
            verify_delay: case["verifyDelay"].as_i64().unwrap(),
            fail: case["failHash"].as_bool().unwrap(),
        });
        let info = UserAccountInfo::create_default(now).shared();
        {
            let mut info = info.lock().unwrap();
            info.username = "alice".into();
            info.password = "password".into();
            info.ban_end_time = case["ban"].as_i64().unwrap();
            info.online = case["online"].as_bool().unwrap();
        }
        if case["existing"] == true {
            info.lock().unwrap().password = Passwords::with_salt_source(4, || Ok([0; 16]))
                .unwrap()
                .hash("password")
                .unwrap();
            store.store(Arc::clone(&info)).unwrap();
        }
        *store.fail.lock().unwrap() = case["failStorage"].as_str().unwrap_or("").into();
        let mode = if case["mode"] == "production" {
            Mode::Production
        } else {
            Mode::Development {
                default_admin: true,
            }
        };
        let auth = Authenticator::new(mode, store.clone(), passwords);
        let mut records = BTreeMap::from([("a".to_owned(), Arc::clone(&info))]);
        let clock_time = Arc::clone(&time);
        let clock = move || clock_time.load(Ordering::Relaxed);
        let mut result = Value::Null;
        let outcome = (|| -> Result<()> {
            match if case["kind"] == "admission" {
                "admission"
            } else {
                case["operation"].as_str().unwrap()
            } {
                "admission" => {
                    use rs09_protocol::login::{AuthResponse, LoginInfo, LoginOutcome};
                    use rs09_server::auth::admission::{self, Decision};
                    let parsed = LoginOutcome {
                        response: AuthResponse::from_byte(
                            case["parseResponse"].as_u64().unwrap() as u8
                        )
                        .unwrap(),
                        info: case["info"].as_bool().unwrap().then(|| LoginInfo {
                            username: case["username"].as_str().unwrap().into(),
                            password: case["password"].as_str().unwrap().into(),
                            ..Default::default()
                        }),
                    };
                    result = match admission::check(parsed, &auth, &clock) {
                        Decision::Reply(response) => {
                            json!({"responses":[response as u8],"ready":false})
                        }
                        Decision::Ready(ready) => {
                            records.insert("logged".into(), ready.account);
                            json!({"responses":[],"ready":true})
                        }
                    };
                }
                "create" => result = json!(auth.create_account_with_clock(&info, &clock)?),
                "login" => {
                    let outcome = auth.check_login_with_clock(
                        case["username"].as_str().unwrap(),
                        case["password"].as_str().unwrap(),
                        &clock,
                    )?;
                    result = json!(outcome.response as u8);
                    if let Some(info) = outcome.account {
                        records.insert("logged".into(), info);
                    }
                }
                "update" => auth.update_password_with_clock(
                    case["username"].as_str().unwrap(),
                    case["password"].as_str().unwrap(),
                    &clock,
                )?,
                other => panic!("unknown clock operation {other}"),
            }
            Ok(())
        })();
        assert_eq!(
            json!({"result":result,"error":outcome.is_err(),"now":time.load(Ordering::Relaxed),"state":state(&store,&records)}),
            case["expected"],
            "clock case {index}: {case}"
        );
    }
}

#[test]
fn memory_storage_reads_the_clock_only_when_creating_a_missing_account() {
    let storage = InMemoryStorage::default();
    let time = Arc::new(AtomicI64::new(100));
    let read = Arc::clone(&time);
    let clock = move || read.fetch_add(1, Ordering::Relaxed);
    let first = storage
        .get_account_info_with_clock("alice", &clock)
        .unwrap();
    let second = storage
        .get_account_info_with_clock("alice", &clock)
        .unwrap();
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(time.load(Ordering::Relaxed), 101);
    assert_eq!(first.lock().unwrap().join_date.millis(), 100);
}

#[test]
fn account_corpus_covers_original_classes_and_pins_sources_and_compiler() {
    let metadata: Value =
        serde_json::from_str(include_str!("../../../fixtures/auth/provenance.json")).unwrap();
    assert_eq!(
        metadata["upstream"],
        serde_json::from_str::<Value>(include_str!("../../../upstream.lock.json")).unwrap()
    );
    assert_eq!(
        metadata["fixtureSha256"],
        format!("{:x}", Sha256::digest(CORPUS.as_bytes()))
    );
    assert_eq!(
        metadata["caseCount"].as_u64().unwrap() as usize,
        CORPUS.lines().count()
    );
    assert_eq!(
        [
            "sequence",
            "hash",
            "verify",
            "salt",
            "factory",
            "clock",
            "admission"
        ]
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
        if path.starts_with("oracle/") {
            let bytes: Vec<_> = std::fs::read(root.join(path))
                .unwrap()
                .into_iter()
                .filter(|&b| b != b'\r')
                .collect();
            assert_eq!(*expected, format!("{:x}", Sha256::digest(bytes)), "{path}");
        }
    }
}

fn provider_id(
    ids: &mut Vec<Arc<dyn AccountStorage>>,
    provider: Option<Arc<dyn AccountStorage>>,
) -> Option<usize> {
    provider.map(|provider| {
        if let Some(index) = ids.iter().position(|other| Arc::ptr_eq(&provider, other)) {
            index + 1
        } else {
            ids.push(provider);
            ids.len()
        }
    })
}
fn factory_state(registry: &Registry, ids: &mut Vec<Arc<dyn AccountStorage>>) -> Value {
    let provider = registry.provider();
    let id = provider_id(ids, provider.map(Provider::storage));
    let auth = registry.authenticator();
    let auth_id = provider_id(ids, auth.map(|auth| Arc::clone(&auth.storage)));
    let sql = match provider {
        Some(Provider::Sql(sql)) => sql.reference_connection().map(|connection| {
            json!([
                connection.connection_string,
                connection.username,
                connection.password
            ])
        }),
        _ => None,
    };
    json!({"provider":provider.map(|provider| match provider {Provider::Memory(_)=>"memory",Provider::Sql(_)=>"sql"}),
        "providerId":id,"auth":auth.map(|auth| match auth.mode{Mode::Production=>"production",Mode::Development{..}=>"development"}),
        "authProviderId":auth_id,"sql":sql})
}
#[test]
fn provider_factory_matches_original_defaults_selection_failure_and_reconfiguration() {
    for (index, case) in cases("factory").iter().enumerate() {
        let now = case["now"].as_i64().unwrap();
        let passwords = Arc::new(TestPasswords {
            fail: Mutex::new(false),
            codec: Passwords::with_salt_source(4, || Ok([0; 16])).unwrap(),
        });
        let mut registry = Registry::default();
        let mut ids = Vec::new();
        let mut results = Vec::new();
        for action in case["actions"].as_array().unwrap() {
            let mut output = Value::Null;
            let mut error = false;
            match action["op"].as_str().unwrap() {
                "inspect" => {}
                "configure" => {
                    let database = &action["database"];
                    let get = |key: &str| database[key].as_str().map(str::to_owned);
                    let settings = Settings {
                        use_auth: action["use_auth"].as_bool().unwrap(),
                        persist_accounts: action["persist_accounts"].as_bool().unwrap(),
                        default_admin: action["default_admin"].as_bool().unwrap(),
                        database: DatabaseSettings {
                            address: get("address"),
                            name: get("name"),
                            username: get("username"),
                            password: get("password"),
                            port: get("port"),
                        },
                    };
                    error = registry
                        .configure(&settings, passwords.clone(), |_| false)
                        .is_err();
                }
                "login" => {
                    let result =
                        registry
                            .authenticator()
                            .ok_or(AuthError::Storage)
                            .and_then(|auth| {
                                auth.check_login(
                                    action["username"].as_str().unwrap(),
                                    action["password"].as_str().unwrap(),
                                    now,
                                )
                            });
                    match result {
                        Ok(outcome) => {
                            output = json!({"response":outcome.response as u8,"account":outcome.account.as_ref().map(snapshot)})
                        }
                        Err(_) => error = true,
                    }
                }
                "create" => {
                    let info = UserAccountInfo::create_default(now).shared();
                    {
                        let mut fields = info.lock().unwrap();
                        fields.username = action["username"].as_str().unwrap().into();
                        fields.password = action["password"].as_str().unwrap().into();
                    }
                    let result = registry
                        .authenticator()
                        .ok_or(AuthError::Storage)
                        .and_then(|auth| auth.create_account(&info, now));
                    error = result.is_err();
                    output = json!({"created":result.ok(),"account":snapshot(&info)});
                }
                other => panic!("unknown factory operation {other}"),
            }
            results.push(
                json!({"output":output,"error":error,"state":factory_state(&registry,&mut ids)}),
            );
        }
        assert_eq!(
            json!(results),
            case["expected"],
            "factory case {index}: {case}"
        );
    }
}

#[test]
fn initial_factory_settings_and_unconfigured_sql_do_not_open_connections() {
    let settings = Settings::default();
    assert!(!settings.use_auth);
    assert!(!settings.persist_accounts);
    assert!(settings.default_admin);
    let sql = rs09_server::auth::sql::SqlStorage::unconfigured(|_| false);
    assert!(sql.username_taken("alice").is_err());
    assert_eq!(sql.reference_connection().unwrap().connection_string, "");
    assert_eq!(
        sql.last_connection_error().as_deref(),
        Some("SQL provider is unconfigured or its address is invalid")
    );
}

#[test]
fn account_hashes_timestamps_aliases_storage_side_effects_and_auth_rules_match_original_jvm() {
    for (case_index, case) in cases("sequence").into_iter().enumerate() {
        let mut now = case["now"].as_i64().unwrap();
        let store = Arc::new(RecordingStorage::default());
        let passwords = Arc::new(TestPasswords {
            fail: Mutex::new(false),
            codec: Passwords::with_salt_source(4, || Ok([0; 16])).unwrap(),
        });
        let mode = if case["mode"] == "production" {
            Mode::Production
        } else {
            Mode::Development {
                default_admin: case["admin"].as_bool().unwrap(),
            }
        };
        let auth = Authenticator::new(mode, store.clone(), passwords.clone());
        let mut records = BTreeMap::<String, Account>::new();
        for (index, action) in case["actions"].as_array().unwrap().iter().enumerate() {
            let id = action["id"].as_str().unwrap_or("");
            let mut output = Value::Null;
            let result = (|| -> Result<()> {
                match action["op"].as_str().unwrap() {
                    "new" => {
                        let account = UserAccountInfo::create_default(now).shared();
                        for (field, value) in action["fields"].as_object().unwrap() {
                            set(&account, field.parse().unwrap(), value)?;
                        }
                        records.insert(id.into(), account);
                    }
                    "get" => {
                        records.insert(
                            id.into(),
                            store.get_account_info(action["key"].as_str().unwrap(), now)?,
                        );
                    }
                    "set" => set(
                        &records[id],
                        action["field"].as_u64().unwrap() as usize,
                        &action["value"],
                    )?,
                    "baseline" => records[id].lock().unwrap().set_initial_reference_values(),
                    "uninitialize" => records[id].lock().unwrap().initial_values = None,
                    "timestamp_time" => records[id]
                        .lock()
                        .unwrap()
                        .join_date
                        .set_time(action["value"].as_i64().unwrap()),
                    "timestamp_nanos" => records[id]
                        .lock()
                        .unwrap()
                        .join_date
                        .set_nanos(action["value"].as_u64().unwrap() as u32)?,
                    "store" => store.store(Arc::clone(&records[id]))?,
                    "update" => store.update(Arc::clone(&records[id]))?,
                    "remove" => store.remove(&records[id])?,
                    "taken" => {
                        output = json!(store.username_taken(action["key"].as_str().unwrap())?)
                    }
                    "friends" => {
                        output = json!(store.online_friends(action["key"].as_str().unwrap())?)
                    }
                    "ip" => {
                        output = json!(store.usernames_with_ip(action["key"].as_str().unwrap())?)
                    }
                    "create" => output = json!(auth.create_account(&records[id], now)?),
                    "can_create" => output = json!(auth.can_create_account(&records[id])?),
                    "login" => {
                        let result = auth.check_login(
                            action["username"].as_str().unwrap(),
                            action["password"].as_str().unwrap(),
                            now,
                        )?;
                        output = json!(result.response as u8);
                        if let Some(account) = result.account {
                            records.insert(id.into(), account);
                        }
                    }
                    "password" => {
                        output = json!(auth.check_password(
                            action["stored"].as_str().unwrap(),
                            action["password"].as_str().unwrap()
                        )?)
                    }
                    "update_password" => auth.update_password(
                        action["username"].as_str().unwrap(),
                        action["password"].as_str().unwrap(),
                        now,
                    )?,
                    "clock" => now = action["value"].as_i64().unwrap(),
                    "fail_storage" => {
                        *store.fail.lock().unwrap() = action["value"].as_str().unwrap().into()
                    }
                    "fail_hash" => {
                        *passwords.fail.lock().unwrap() = action["value"].as_bool().unwrap()
                    }
                    op => panic!("unsupported action {op}"),
                }
                Ok(())
            })();
            let actual =
                json!({"output":output,"error":result.is_err(),"state":state(&store,&records)});
            assert_eq!(
                actual, case["expected"][index],
                "case {case_index}, action {index}: {action}"
            );
        }
    }
}

#[test]
fn bcrypt_hashes_salts_utf8_nuls_truncation_and_cost_31_overflow_match_original() {
    for case in cases("hash") {
        let actual = password::hash_with_salt(
            case["password"].as_str().unwrap(),
            case["salt"].as_str().unwrap(),
        );
        assert_eq!(actual.is_err(), case["error"].as_bool().unwrap(), "{case}");
        if let Ok(hash) = actual {
            assert_eq!(hash, case["expected"], "{case}");
        }
    }
    for case in cases("salt") {
        let salt: [u8; 16] = case["bytes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_u64().unwrap() as u8)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        assert_eq!(
            password::salt_string(case["cost"].as_u64().unwrap() as u32, salt).unwrap(),
            case["expected"]
        );
    }
}

#[test]
fn production_password_verification_preserves_2a_gate_and_failure_results() {
    for case in cases("verify") {
        let actual = password::verify(
            case["password"].as_str().unwrap(),
            case["stored"].as_str().unwrap(),
        );
        assert_eq!(actual.is_err(), case["error"].as_bool().unwrap(), "{case}");
        if let Ok(value) = actual {
            assert_eq!(json!(value), case["expected"], "{case}");
        }
    }
}

#[test]
fn concurrent_in_memory_access_matches_serial_disjoint_account_results() {
    fn exercise(store: &InMemoryStorage, name: &str) {
        let account = UserAccountInfo::create_default(530).shared();
        {
            let mut value = account.lock().unwrap();
            value.username = name.to_owned();
            value.rights = 2;
        }
        store.store(Arc::clone(&account)).unwrap();
        assert!(Arc::ptr_eq(
            &account,
            &store.get_account_info(name, 530).unwrap()
        ));
        account.lock().unwrap().credits = 17;
        store.update(account).unwrap();
    }
    let serial = InMemoryStorage::default();
    let parallel = Arc::new(InMemoryStorage::default());
    for i in 0..128 {
        exercise(&serial, &format!("agent_{i}"));
    }
    std::thread::scope(|scope| {
        for worker in 0..8 {
            let store = Arc::clone(&parallel);
            scope.spawn(move || {
                for i in (worker..128).step_by(8) {
                    exercise(&store, &format!("agent_{i}"));
                }
            });
        }
    });
    let snapshots = |store: &InMemoryStorage| {
        store
            .snapshot_handles()
            .iter()
            .map(|(key, record)| (key.clone(), snapshot(record)))
            .collect::<BTreeMap<_, _>>()
    };
    assert_eq!(snapshots(&serial), snapshots(&parallel));
    let barrier = Arc::new(std::sync::Barrier::new(32));
    let handles = std::thread::scope(|scope| {
        let tasks: Vec<_> = (0..32)
            .map(|_| {
                let store = Arc::clone(&parallel);
                let barrier = Arc::clone(&barrier);
                scope.spawn(move || {
                    barrier.wait();
                    store.get_account_info("one_missing", 530).unwrap()
                })
            })
            .collect();
        tasks
            .into_iter()
            .map(|task| task.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert!(
        handles
            .iter()
            .all(|handle| Arc::ptr_eq(handle, &handles[0]))
    );
}
