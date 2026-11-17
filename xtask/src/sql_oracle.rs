use crate::{Result, run};
use mysql::{Conn, OptsBuilder, Row, Value as SqlValue, prelude::Queryable};
use rs09_server::auth::{
    self,
    account::{Account, FieldValue, Timestamp, UserAccountInfo},
    sql::{SqlConfig, SqlStorage},
    storage::AccountStorage,
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

const ROWS: &str = "SELECT username,password,UID,rights,credits,ip,lastGameIp,muteTime,banTime,contacts,blocked,clanName,currentClan,clanReqs,timePlayed,lastLogin,CAST(online AS SIGNED),DATE_FORMAT(joined_date,'%Y-%m-%d %H:%i:%s.%f') FROM members ORDER BY UID";
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
fn snapshot(account: &Account) -> Value {
    let info = account.lock().unwrap();
    json!({"values":info.values().iter().map(value).collect::<Vec<_>>(),
    "initial":info.initial_values.as_ref().map(|items|items.iter().map(value).collect::<Vec<_>>()),"changed":info.changed_fields().ok().map(|(changed,_)|changed),"hash":info.java_hash(),"default":info.is_default()})
}
fn set(account: &Account, index: usize, value: &Value) -> auth::Result<()> {
    let value = match index {
        2..=4 => FieldValue::Int(value.as_i64().ok_or(auth::AuthError::Storage)? as i32),
        7 | 8 | 14 | 15 => FieldValue::Long(value.as_i64().ok_or(auth::AuthError::Storage)?),
        16 => FieldValue::Bool(value.as_bool().ok_or(auth::AuthError::Storage)?),
        17 => {
            let stamp = Timestamp::new(value["millis"].as_i64().ok_or(auth::AuthError::Storage)?);
            if let Some(nanos) = value.get("nanos") {
                stamp.set_nanos(nanos.as_u64().ok_or(auth::AuthError::Storage)? as u32)?;
            }
            FieldValue::Timestamp(stamp)
        }
        _ => FieldValue::Text(value.as_str().ok_or(auth::AuthError::Storage)?.to_owned()),
    };
    account.lock().unwrap().set_value(index, value)
}
fn database_value(value: SqlValue) -> Result<Value> {
    Ok(match value {
        SqlValue::NULL => Value::Null,
        SqlValue::Bytes(bytes) => json!(String::from_utf8(bytes)?),
        SqlValue::Int(value) => json!(value),
        SqlValue::UInt(value) => json!(value),
        other => return Err(format!("unexpected raw account SQL field {other:?}").into()),
    })
}
fn rows(connection: &mut Conn) -> Option<Value> {
    let rows: Vec<Row> = connection.exec(ROWS, ()).ok()?;
    let values: std::result::Result<Vec<_>, _> = rows
        .into_iter()
        .map(|row| {
            row.unwrap()
                .into_iter()
                .map(database_value)
                .collect::<Result<Vec<_>>>()
        })
        .collect();
    values.ok().map(|rows| json!(rows))
}
fn evaluate(root: &Path, test: &Value, port: u16, datadir: &Path, tls: bool) -> Result<Value> {
    let mut connection = Conn::new(
        OptsBuilder::default()
            .ip_or_hostname(Some("127.0.0.1"))
            .tcp_port(port)
            .user(Some("root"))
            .prefer_socket(false),
    )?;
    let actual: String = connection
        .query_first("SELECT @@datadir")?
        .ok_or("missing SQL datadir")?;
    if PathBuf::from(actual).canonicalize()? != datadir.canonicalize()? {
        return Err("SQL server is outside this owned run".into());
    }
    let schema = fs::read_to_string(root.join("fixtures/sql/schema.sql"))?
        .replace(
            "CREATE DATABASE global;",
            "DROP DATABASE IF EXISTS rs09_native; CREATE DATABASE rs09_native;",
        )
        .replace("USE global;", "USE rs09_native;");
    for statement in schema
        .split(';')
        .map(str::trim)
        .filter(|sql| !sql.is_empty())
    {
        connection.query_drop(statement)?;
    }
    let online = Arc::new(Mutex::new(BTreeSet::<String>::new()));
    let lookups = Arc::new(Mutex::new(Vec::<String>::new()));
    let on = Arc::clone(&online);
    let looked = Arc::clone(&lookups);
    let mut sql_config = SqlConfig::new(
        "127.0.0.1".into(),
        port,
        "rs09_native".into(),
        "root".into(),
        String::new(),
    );
    sql_config.max_connections = 1;
    let storage = SqlStorage::new(sql_config, move |name| {
        looked.lock().unwrap().push(name.to_owned());
        on.lock().unwrap().contains(name)
    })?;
    let encrypted = storage.transport_encrypted().map_err(|_| {
        format!(
            "SQL connection failed: {:?}",
            storage.last_connection_error()
        )
    })?;
    if encrypted != tls {
        return Err("native SQL transport did not match server TLS capability".into());
    }
    // Exercise the real factory too: DATABASE_PORT is deliberately unrelated to
    // the address's port, matching ProductionAuthenticator.configureFor.
    let mut registry = auth::factory::Registry::default();
    registry.configure(
        &auth::factory::Settings {
            use_auth: true,
            persist_accounts: true,
            database: auth::factory::DatabaseSettings {
                address: Some(format!("127.0.0.1:{port}")),
                name: Some("rs09_native".into()),
                username: Some("root".into()),
                password: Some(String::new()),
                port: Some("1".into()),
            },
            ..Default::default()
        },
        Arc::new(auth::password::Passwords::default()),
        |_| false,
    )?;
    let auth::factory::Provider::Sql(configured) =
        registry.provider().ok_or("missing factory provider")?
    else {
        return Err("factory did not select SQL".into());
    };
    if configured.transport_encrypted()? != tls || configured.username_taken("not-created")? {
        return Err("factory SQL connection/query did not match reference settings".into());
    }
    let mut required = SqlConfig::new(
        "127.0.0.1".into(),
        port,
        "rs09_native".into(),
        "root".into(),
        String::new(),
    );
    required.encryption = auth::sql::Encryption::Required;
    let required = SqlStorage::new(required, |_| false)?.transport_encrypted();
    if tls && required != Ok(true) || !tls && required.is_ok() {
        return Err("required SQL TLS policy mismatch".into());
    }
    let mut records = BTreeMap::<String, Account>::new();
    let mut now = test["now"].as_i64().ok_or("missing controlled SQL time")?;
    let mut outputs = Vec::new();
    for action in test["actions"].as_array().ok_or("missing SQL actions")? {
        let id = action["id"].as_str().unwrap_or("");
        let mut output = Value::Null;
        let result =
            (|| -> auth::Result<()> {
                match action["op"].as_str().ok_or(auth::AuthError::Storage)? {
                    "new" => {
                        let record = UserAccountInfo::create_default(now).shared();
                        for (field, value) in action["fields"]
                            .as_object()
                            .ok_or(auth::AuthError::Storage)?
                        {
                            set(
                                &record,
                                field.parse().map_err(|_| auth::AuthError::Storage)?,
                                value,
                            )?;
                        }
                        records.insert(id.to_owned(), record);
                    }
                    "get" => {
                        records.insert(
                            id.to_owned(),
                            storage.get_account_info(
                                action["key"].as_str().ok_or(auth::AuthError::Storage)?,
                                now,
                            )?,
                        );
                    }
                    "set" => set(
                        records.get(id).ok_or(auth::AuthError::Storage)?,
                        action["field"].as_u64().ok_or(auth::AuthError::Storage)? as usize,
                        &action["value"],
                    )?,
                    "store" => storage
                        .store(Arc::clone(records.get(id).ok_or(auth::AuthError::Storage)?))?,
                    "update" => storage
                        .update(Arc::clone(records.get(id).ok_or(auth::AuthError::Storage)?))?,
                    "remove" => storage.remove(records.get(id).ok_or(auth::AuthError::Storage)?)?,
                    "baseline" => records
                        .get(id)
                        .ok_or(auth::AuthError::Storage)?
                        .lock()
                        .unwrap()
                        .set_initial_reference_values(),
                    "uninitialize" => {
                        records
                            .get(id)
                            .ok_or(auth::AuthError::Storage)?
                            .lock()
                            .unwrap()
                            .initial_values = None
                    }
                    "timestamp_time" => records
                        .get(id)
                        .ok_or(auth::AuthError::Storage)?
                        .lock()
                        .unwrap()
                        .join_date
                        .set_time(action["value"].as_i64().ok_or(auth::AuthError::Storage)?),
                    "timestamp_nanos" => records
                        .get(id)
                        .ok_or(auth::AuthError::Storage)?
                        .lock()
                        .unwrap()
                        .join_date
                        .set_nanos(
                            action["value"].as_u64().ok_or(auth::AuthError::Storage)? as u32
                        )?,
                    "taken" => {
                        output = json!(storage.username_taken(
                            action["key"].as_str().ok_or(auth::AuthError::Storage)?
                        )?)
                    }
                    "friends" => {
                        output = json!(storage.online_friends(
                            action["key"].as_str().ok_or(auth::AuthError::Storage)?
                        )?)
                    }
                    "ip" => {
                        output = json!(storage.usernames_with_ip(
                            action["key"].as_str().ok_or(auth::AuthError::Storage)?
                        )?)
                    }
                    "online" => {
                        *online.lock().unwrap() = action["value"]
                            .as_array()
                            .ok_or(auth::AuthError::Storage)?
                            .iter()
                            .map(|name| name.as_str().unwrap().to_owned())
                            .collect()
                    }
                    "clock" => now = action["value"].as_i64().ok_or(auth::AuthError::Storage)?,
                    "sql" => connection
                        .query_drop(action["value"].as_str().ok_or(auth::AuthError::Storage)?)
                        .map_err(|_| auth::AuthError::Storage)?,
                    _ => return Err(auth::AuthError::Storage),
                }
                Ok(())
            })();
        let observed: BTreeMap<_, _> = records
            .iter()
            .map(|(name, record)| (name, snapshot(record)))
            .collect();
        outputs.push(json!({"output":output,"error":result.is_err(),"records":observed,"rows":rows(&mut connection),"lookups":*lookups.lock().unwrap()}));
    }
    Ok(json!(outputs))
}
pub fn native(root: &Path, args: &[String]) -> Result<()> {
    if args.len() != 3 || args[1] != "--config" {
        return Err("sql-native requires its owned controller config".into());
    }
    let config: Value = serde_json::from_slice(&fs::read(&args[2])?)?;
    if config["database"] != "rs09_native" {
        return Err("refusing unexpected SQL test schema".into());
    }
    let datadir = PathBuf::from(
        config["datadir"]
            .as_str()
            .ok_or("missing owned SQL datadir")?,
    )
    .canonicalize()?;
    if !datadir.starts_with(root.join("runs/sql-db").canonicalize()?) {
        return Err("SQL test datadir is outside owned workspace".into());
    }
    let port = u16::try_from(config["port"].as_u64().ok_or("missing SQL test port")?)?;
    let input = fs::read_to_string(config["input"].as_str().ok_or("missing SQL request file")?)?;
    let mut output = fs::File::create(config["output"].as_str().ok_or("missing SQL output path")?)?;
    for line in input.lines().filter(|line| !line.is_empty()) {
        let mut test: Value = serde_json::from_str(line)?;
        match test["kind"].as_str().ok_or("missing SQL case kind")? {
            "sequence" => {
                test["expected"] = evaluate(
                    root,
                    &test,
                    port,
                    &datadir,
                    config["tls"]
                        .as_bool()
                        .ok_or("missing SQL transport mode")?,
                )?
            }
            "update_query" => {
                test["expected"] = json!(auth::sql::build_update_query(
                    &test["indices"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|value| value.as_i64().unwrap() as usize)
                        .collect::<Vec<_>>()
                ))
            }
            "contacts" => {
                let result = auth::sql::parse_contact_names(test["text"].as_str().unwrap());
                test["error"] = json!(result.is_err());
                test["expected"] = result.map_or(Value::Null, |names| json!(names));
            }
            "map_order" => {
                test["expected"] = json!(rs09_game_core::java_map::string_key_order(
                    test["keys"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|key| key.as_str().unwrap().to_owned())
                ))
            }
            _ => return Err("unknown SQL case kind".into()),
        }
        writeln!(output, "{}", serde_json::to_string(&test)?)?;
    }
    Ok(())
}
pub fn generate(root: &Path, args: &[String]) -> Result<()> {
    let mut command = crate::login_oracle::invocation(root, args, "oracle/sql/generate.py")?;
    command.arg("--native").arg(std::env::current_exe()?);
    let result = run(&mut command)?;
    let output = String::from_utf8(result.stdout)?;
    let proof: Value = serde_json::from_str(
        output
            .lines()
            .last()
            .ok_or("missing SQL comparison report")?,
    )?;
    if proof["nativeMatched"] != true || proof["serverStopped"] != true {
        return Err("SQL comparison incomplete".into());
    }
    let metadata = json!({"format":1,"upstream":crate::verify_reference(root)?,"results":proof,
        "scope":"Original/native SQL storage on pinned schema and real MariaDB, over plaintext and TLS. No world-admission or complete-game claim.",
        "nativeBuild":crate::network_oracle::native_build()?,"nativeSources":crate::network_oracle::native_sources(root)?,
        "oracle":serde_json::from_slice::<Value>(&fs::read(root.join("fixtures/sql/provenance.json"))?)?});
    fs::write(
        root.join("runs/sql-oracle/report.json"),
        format!("{}\n", serde_json::to_string_pretty(&metadata)?),
    )?;
    print!("{output}");
    Ok(())
}
