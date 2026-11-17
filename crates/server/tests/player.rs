use rs09_server::{
    auth::{
        self, AuthError,
        account::{Account, FieldValue, UserAccountInfo},
        storage::AccountStorage,
    },
    player::{
        self, Error,
        communication::{self, Cell, ClanRank, Columns, CommunicationInfo, Contact},
        details::{ClientInfo, PlayerDetails, Rights, UidInfo},
    },
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicI64, AtomicUsize, Ordering},
    },
};
const CORPUS: &str = include_str!("../../../fixtures/player/oracle.jsonl");
fn cases(kind: &str) -> Vec<Value> {
    CORPUS
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|case| case["kind"] == kind)
        .collect()
}
fn error(error: Error) -> &'static str {
    match error {
        Error::Number => "number",
        Error::Index => "index",
        Error::Null => "null",
        Error::Storage(AuthError::IllegalState) => "illegal_state",
        Error::Storage(_) => "storage",
    }
}
fn attempt(value: player::Result<Value>) -> Value {
    match value {
        Ok(value) => json!({"ok":value}),
        Err(err) => json!({"error":error(err)}),
    }
}
fn done(value: player::Result<()>) -> Value {
    attempt(value.map(|()| json!(true)))
}
fn rank(value: &Value) -> Option<ClanRank> {
    value
        .as_i64()
        .map(|value| ClanRank::from_ordinal(value as i32).unwrap())
}
fn contacts(c: &communication::Contacts) -> Value {
    json!(
        c.iter()
            .map(|(key, c)| json!([key, c.username, c.world_id, c.rank.map(ClanRank::ordinal)]))
            .collect::<Vec<_>>()
    )
}
fn state(c: &CommunicationInfo) -> Value {
    json!({"contacts":contacts(&c.contacts),"blocked":c.blocked,"clanName":c.clan_name,"currentClan":c.current_clan(),"requirements":c.requirements.map(|rank|rank.map(ClanRank::ordinal)),"lootShare":c.loot_share,"contactString":attempt(c.contact_string().map(|text|json!(text))),"blockedString":c.blocked_string(),"requirementString":attempt(c.clan_requirement_string().map(|text|json!(text)))})
}
fn account(input: &Value) -> UserAccountInfo {
    let mut a = UserAccountInfo::create_default(0);
    a.contacts = input["contacts"].as_str().unwrap().into();
    a.blocked = input["blocked"].as_str().unwrap().into();
    a.clan_name = input["clanName"].as_str().unwrap().into();
    a.current_clan = input["currentClan"].as_str().unwrap().into();
    a.clan_reqs = input["clanReqs"].as_str().unwrap().into();
    a
}
fn account_values(a: &UserAccountInfo) -> Value {
    json!(
        a.values()
            .into_iter()
            .map(|v| match v {
                FieldValue::Text(v) => json!(v),
                FieldValue::Int(v) => json!(v),
                FieldValue::Long(v) => json!(v),
                FieldValue::Bool(v) => json!(v),
                FieldValue::Timestamp(v) => json!(v.millis()),
            })
            .collect::<Vec<_>>()
    )
}
#[derive(Default)]
struct Table {
    values: BTreeMap<String, Cell>,
    trace: Vec<Value>,
    fail: String,
}
impl Table {
    fn new(action: &Value) -> Self {
        Self {
            values: action["values"]
                .as_object()
                .map(|values| {
                    values
                        .iter()
                        .map(|(key, value)| {
                            (
                                key.clone(),
                                Cell {
                                    text: value.as_str().map(str::to_owned),
                                    interned_empty: action["interned"].as_bool().unwrap_or(false),
                                },
                            )
                        })
                        .collect()
                })
                .unwrap_or_default(),
            fail: action["fail"].as_str().unwrap_or("").into(),
            ..Default::default()
        }
    }
    fn snapshot(&self) -> Value {
        json!(
            self.values
                .iter()
                .map(|(key, cell)| (key, cell.text.clone()))
                .collect::<BTreeMap<_, _>>()
        )
    }
}
impl Columns for Table {
    fn read(&mut self, name: &str) -> player::Result<Cell> {
        self.trace.push(json!(["read", name]));
        if self.fail == format!("read:{name}") {
            return Err(Error::Storage(AuthError::Storage));
        }
        Ok(self.values.get(name).cloned().unwrap_or_default())
    }
    fn write(&mut self, name: &str, value: Option<String>) -> player::Result<()> {
        self.trace.push(json!(["write", name, value]));
        if self.fail == format!("write:{name}") {
            return Err(Error::Storage(AuthError::Storage));
        }
        self.values.insert(
            name.into(),
            Cell {
                text: value,
                interned_empty: false,
            },
        );
        Ok(())
    }
}
fn comm_actions(input: &Value) -> Value {
    let mut c = CommunicationInfo::default();
    json!(input["actions"].as_array().unwrap().iter().map(|action|{
    let mut table=Table::new(action);
    let response=match action["op"].as_str().unwrap(){
        "account"=>c.parse_account(&account(action)),"columns"=>c.parse_columns(&mut table),"save_columns"=>c.save_columns(&mut table),
        "put"=>{let mut contact=Contact::new(action["username"].as_str().map(str::to_owned));contact.rank=rank(&action["rank"]);contact.world_id=action["world"].as_i64().unwrap() as i32;c.contacts.insert(action["key"].as_str().unwrap().into(),contact);Ok(())},
        "rank"=>{c.contacts.get_mut(action["key"].as_str().unwrap()).unwrap().rank=rank(&action["value"]);Ok(())},
        "block"=>{c.blocked.push(action["value"].as_str().map(str::to_owned));Ok(())},
        "requirement"=>{c.requirements[action["slot"].as_u64().unwrap() as usize]=rank(&action["value"]);Ok(())},_=>panic!("unknown action")};
    json!({"result":done(response),"state":state(&c),"trace":table.trace,"columns":table.snapshot()})
}).collect::<Vec<_>>())
}
fn uid(action: &Value) -> player::Result<UidInfo> {
    UidInfo::new(
        action["ip"].as_str().map(str::to_owned),
        action["computer"].as_str().map(str::to_owned),
        action["mac"].as_str().map(str::to_owned),
        action["serial"].as_str().map(str::to_owned),
    )
}
fn uid_state(uid: &UidInfo) -> Value {
    json!({"ip":uid.ip,"computer":uid.computer_name,"mac":uid.mac,"serial":uid.serial,"string":uid.to_string()})
}
#[derive(Default)]
struct Storage {
    calls: Mutex<Vec<Value>>,
    fail: Mutex<String>,
}
impl AccountStorage for Storage {
    fn update(&self, account: Account) -> auth::Result<()> {
        self.calls
            .lock()
            .unwrap()
            .push(account_values(&account.lock().unwrap()));
        match self.fail.lock().unwrap().as_str() {
            "" => Ok(()),
            "illegal_state" => Err(AuthError::IllegalState),
            _ => Err(AuthError::Storage),
        }
    }
    fn username_taken(&self, _: &str) -> auth::Result<bool> {
        unreachable!()
    }
    fn get_account_info(&self, _: &str, _: i64) -> auth::Result<Account> {
        unreachable!()
    }
    fn store(&self, _: Account) -> auth::Result<()> {
        unreachable!()
    }
    fn remove(&self, _: &Account) -> auth::Result<()> {
        unreachable!()
    }
    fn online_friends(&self, _: &str) -> auth::Result<Vec<String>> {
        unreachable!()
    }
    fn usernames_with_ip(&self, _: &str) -> auth::Result<Vec<Option<String>>> {
        unreachable!()
    }
}
fn details(input: &Value) -> Value {
    let now = input["now"].as_i64().unwrap();
    let reads = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&reads);
    let mut d = PlayerDetails::new(
        "alice",
        Arc::new(move || {
            counted.fetch_add(1, Ordering::Relaxed);
            now
        }),
    );
    let storage = Storage::default();
    json!(input["actions"].as_array().unwrap().iter().map(|action|{
        let response=match action["op"].as_str().unwrap(){
            "set"=>{let field=action["field"].as_u64().unwrap() as usize;let v=if let Some(text)=action["value"].as_str(){FieldValue::Text(text.into())}else if [2,3,4].contains(&field){FieldValue::Int(action["value"].as_i64().unwrap() as i32)}else{FieldValue::Long(action["value"].as_i64().unwrap())};d.account.lock().unwrap().set_value(field,v).unwrap();done(Ok(()))},
            "flags"=>attempt(Ok(json!({"banned":d.is_banned(),"muted":d.is_muted(),"permanent":d.is_permanent_mute(),"rights":attempt(d.rights().map(|rights|json!(rights.ordinal())))}))),
            "parsed"=>{d.save_parsed=action["value"].as_bool().unwrap();done(Ok(()))},
            "parse"=>done(d.communication.parse_account(&d.account.lock().unwrap())),
            "save"=>{*storage.fail.lock().unwrap()=action["fail"].as_str().unwrap_or("").into();done(d.save(&storage))},
            "requirement"=>{d.communication.requirements[action["slot"].as_u64().unwrap() as usize]=rank(&action["value"]);done(Ok(()))},
            "clan"=>{d.communication.clan_name=action["value"].as_str().map(str::to_owned);done(Ok(()))},
            "uid"=>done(uid(action).and_then(|uid|d.uid.translate(&uid))),
            "session"=>{d.session_address=Some(action["value"].as_str().map(str::to_owned));done(Ok(()))},
            "clear_session"=>{d.session_address=None;done(Ok(()))},_=>panic!("unknown details action")};
        let a=d.account.lock().unwrap();
        json!({"result":response,"account":account_values(&a),"state":state(&d.communication),"saveParsed":d.save_parsed,"ip":d.ip_address(),"uid":uid_state(&d.uid),"updates":*storage.calls.lock().unwrap(),"clockReads":reads.load(Ordering::Relaxed),"delegates":[a.username,a.password,a.uid,a.credits,a.last_login,a.time_played,a.mute_end_time,a.ban_end_time,d.uid.mac,d.uid.computer_name,d.uid.serial]})
    }).collect::<Vec<_>>())
}
#[test]
fn player_fixtures_pin_original_sources_and_explicit_adapter_scope() {
    let metadata: Value =
        serde_json::from_str(include_str!("../../../fixtures/player/provenance.json")).unwrap();
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
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
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
#[test]
fn clan_requirements_and_contact_parsers_match_original_including_hash_tree_order() {
    for kind in ["requirements", "contacts"] {
        for (index, case) in cases(kind).iter().enumerate() {
            let text = case["text"].as_str().unwrap();
            let actual = if kind == "requirements" {
                attempt(
                    communication::parse_clan_requirements(text)
                        .map(|ranks| json!(ranks.map(|r| r.map(ClanRank::ordinal)))),
                )
            } else {
                attempt(communication::parse_contacts(text).map(|c| contacts(&c)))
            };
            assert_eq!(actual, case["expected"], "{kind} case {index}");
        }
    }
}
#[test]
fn communication_account_and_column_state_transitions_and_partial_saves_match_original() {
    for (index, case) in cases("communication").iter().enumerate() {
        assert_eq!(
            comm_actions(case),
            case["expected"],
            "communication case {index}"
        );
    }
}
#[test]
fn player_details_shared_account_save_gates_clock_overflow_and_uid_session_fields_match_original() {
    for (index, case) in cases("details").iter().enumerate() {
        assert_eq!(details(case), case["expected"], "details case {index}");
    }
}
#[test]
fn uid_normalization_and_partial_translation_match_original() {
    for (index, case) in cases("uid").iter().enumerate() {
        let mut current = UidInfo::default();
        let actual = case["actions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|action| {
                let result = if action["op"] == "new" {
                    uid(action).map(|uid| current = uid)
                } else if action["empty"] == true {
                    current.translate(&UidInfo::default())
                } else {
                    uid(action).and_then(|uid| current.translate(&uid))
                };
                json!({"result":done(result),"state":uid_state(&current)})
            })
            .collect::<Vec<_>>();
        assert_eq!(json!(actual), case["expected"], "uid case {index}");
    }
}
#[test]
fn client_modes_rights_and_clan_rank_data_match_original() {
    for case in cases("client") {
        let mut c = ClientInfo {
            display_mode: case["display"].as_i64().unwrap() as i32,
            window_mode: case["window"].as_i64().unwrap() as i32,
            screen_width: case["width"].as_i64().unwrap() as i32,
            screen_height: case["height"].as_i64().unwrap() as i32,
        };
        let snap = |c: &ClientInfo| {
            json!([
                c.display_mode,
                c.window_mode,
                c.screen_width,
                c.screen_height,
                c.is_high_detail(),
                c.is_resizable()
            ])
        };
        let before = snap(&c);
        c.display_mode = c.display_mode.wrapping_neg();
        c.window_mode = c.window_mode.wrapping_add(1);
        c.screen_width = c.screen_height;
        c.screen_height = -1;
        assert_eq!(json!([before, snap(&c)]), case["expected"]);
    }
    for case in cases("rights") {
        let ordinal = case["ordinal"].as_i64().unwrap() as i32;
        assert_eq!(
            json!({"forId":attempt(Rights::for_id(ordinal).map(|r|json!(r.ordinal()))),"raw":attempt(Rights::from_ordinal(ordinal).map(|r|json!(r.ordinal())))}),
            case["expected"]
        );
    }
    for case in cases("ranks") {
        assert_eq!(
            json!(ClanRank::ALL.map(|rank| json!([
                rank.ordinal(),
                rank.value(),
                rank.info(),
                rank.to_string()
            ]))),
            case["expected"]
        );
    }
}
#[test]
fn authenticated_setup_retains_shared_account_client_and_seed_data() {
    let now = Arc::new(AtomicI64::new(1234));
    let reads = Arc::new(AtomicUsize::new(0));
    let account = UserAccountInfo::create_default(0).shared();
    account.lock().unwrap().username = "stored name".into();
    let login = rs09_protocol::login::LoginInfo {
        username: "packet name".into(),
        display_mode: 1,
        window_mode: 2,
        screen_width: 1024,
        screen_height: 768,
        isaac_seed: Some([1, 2, 3, 4]),
        ..Default::default()
    };
    let clock_now = Arc::clone(&now);
    let clock_reads = Arc::clone(&reads);
    let prepared = player::prepare(
        auth::admission::Authenticated {
            login: login.clone(),
            account: Arc::clone(&account),
        },
        Arc::new(move || {
            clock_reads.fetch_add(1, Ordering::Relaxed);
            clock_now.load(Ordering::Relaxed)
        }),
    )
    .unwrap();
    assert!(Arc::ptr_eq(&prepared.details.account, &account));
    assert_eq!(prepared.login.username, login.username);
    assert_eq!(prepared.login.isaac_seed, login.isaac_seed);
    assert_eq!(prepared.client.screen_width, 1024);
    assert_eq!(reads.load(Ordering::Relaxed), 1);
    assert_eq!(
        prepared.details.communication.blocked,
        [Some(String::new())]
    );
    prepared.details.set_rights(Rights::Administrator);
    assert_eq!(account.lock().unwrap().rights, 2);
}

#[test]
fn login_metadata_setup_matches_verbatim_original_event_suffix() {
    for (index, case) in cases("metadata").iter().enumerate() {
        let mut info = account(&case["values"]);
        info.username = "stored name".into();
        let account = info.shared();
        let reads = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&reads);
        let now = case["now"].as_i64().unwrap();
        let login = rs09_protocol::login::LoginInfo {
            username: case["username"].as_str().unwrap().into(),
            opcode: 16,
            display_mode: 1,
            window_mode: 2,
            screen_width: 1024,
            screen_height: 768,
            isaac_seed: Some([1, 2, 3, 4]),
            ..Default::default()
        };
        let result = player::prepare(
            auth::admission::Authenticated {
                login,
                account: Arc::clone(&account),
            },
            Arc::new(move || {
                counted.fetch_add(1, Ordering::Relaxed);
                now
            }),
        );
        let actual = match result {
            Ok(p) => {
                json!({"response":8,"client":[p.client.display_mode,p.client.window_mode,p.client.screen_width,p.client.screen_height],"seed":p.login.isaac_seed,"username":p.login.username,"opcode":p.login.opcode,"details":state(&p.details.communication),"shared":Arc::ptr_eq(&p.details.account,&account),"clockReads":reads.load(Ordering::Relaxed)})
            }
            Err(_) => {
                json!({"response":0,"client":null,"seed":null,"username":null,"opcode":null,"details":null,"shared":false,"clockReads":reads.load(Ordering::Relaxed)})
            }
        };
        assert_eq!(actual, case["expected"], "metadata case {index}");
    }
}
