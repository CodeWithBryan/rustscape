//! Replays fixtures/social (the unchanged CommunicationInfo, ClanRepository,
//! ManagementEvents, GlobalChat, ChatMessage, outgoing packets and the
//! selected PacketProcessor/Decoders530/Login/Player text on JDK 11) against
//! world::social, joined to the native PlayerDetails, CommunicationInfo
//! (contacts in JDK HashMap order), Settings, ClanRank, account records and
//! JavaHashMap. Every recorded call, result, failure and state snapshot of
//! every step must match.
use rs09_game_core::java_lower::lowercase_root;
use rs09_server::auth::account::{AccountFields, Timestamp, UserAccountInfo};
use rs09_server::player::details::{PlayerDetails, Rights};
use rs09_server::player::settings::{self, Settings};
use rs09_server::world::repository::JavaHashMap;
use rs09_server::world::social::chat::{self, Decoded};
use rs09_server::world::social::management::{ContactInfo, FriendUpdateType, MemberInfo};
use rs09_server::world::social::packets::Queued;
use rs09_server::world::social::{
    ChatMessage, ClanId, Error, Event, Host, Level, PulseId, Result, Social, Value, Written,
    rank_at,
};
use serde_json::{Value as Json, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

type Events = Arc<Mutex<Vec<Json>>>;

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct Player {
    name: String,
    details: PlayerDetails,
    settings: Settings,
    kind: String,
    session: bool,
    resizable: bool,
}

struct World {
    case: Json,
    events: Events,
    clock: Arc<AtomicI64>,
    players: BTreeMap<String, Player>,
    names: JavaHashMap<Option<String>, String>,
    list: Vec<String>,
    attributes: HashMap<String, HashMap<String, Value>>,
    locks: HashMap<String, Vec<String>>,
    cipher_draws: HashMap<String, usize>,
    calls: HashMap<String, i64>,
    seed_index: usize,
    stashed: Option<Error>,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn value_json(value: &Value) -> Json {
    match value {
        Value::Int(v) => json!(v),
        Value::Text(v) => json!(v),
        Value::Bool(v) => json!(v),
        Value::Rights(r) => json!(match r {
            Rights::RegularPlayer => "REGULAR_PLAYER",
            Rights::PlayerModerator => "PLAYER_MODERATOR",
            Rights::Administrator => "ADMINISTRATOR",
        }),
    }
}

fn input_value(raw: &Json) -> Value {
    match raw {
        Json::Object(map) => {
            Value::Rights(Rights::from_ordinal(map["rights"].as_i64().unwrap() as i32).unwrap())
        }
        Json::Bool(b) => Value::Bool(*b),
        Json::String(s) => Value::Text(s.clone()),
        other => Value::Int(other.as_i64().unwrap() as i32),
    }
}

fn describe(event: &Event) -> Json {
    match event {
        Event::PlayerStatusUpdate {
            username,
            world,
            notify_friends_only,
        } => {
            json!({"@": "PlayerStatusUpdate", "username": username, "world": world, "notifyFriendsOnly": notify_friends_only})
        }
        Event::RequestContactInfo { username, world } => {
            json!({"@": "RequestContactInfo", "username": username, "world": world})
        }
        Event::SendContactInfo {
            username,
            contacts,
            blocked,
        } => json!({"@": "SendContactInfo", "username": username,
            "contacts": contacts.iter().map(|c| json!({"@": "Contact", "username": c.username, "world": c.world, "rank": c.rank})).collect::<Vec<_>>(),
            "blocked": blocked}),
        Event::FriendUpdate {
            kind,
            username,
            friend,
        } => json!({"@": "FriendUpdate", "type": match kind {
            FriendUpdateType::Add => "ADD", FriendUpdateType::Remove => "REMOVE" }, "username": username, "friend": friend}),
        Event::PrivateMessage {
            sender,
            receiver,
            message,
            rank,
        } => {
            json!({"@": "PrivateMessage", "sender": sender, "receiver": receiver, "message": message, "rank": rank})
        }
        Event::ClanMessage {
            sender,
            clan_name,
            message,
            rank,
        } => {
            json!({"@": "ClanMessage", "sender": sender, "clanName": clan_name, "message": message, "rank": rank})
        }
        Event::JoinClanRequest {
            username,
            clan_name,
        } => json!({"@": "JoinClanRequest", "username": username, "clanName": clan_name}),
        Event::LeaveClanRequest {
            username,
            clan_name,
        } => json!({"@": "LeaveClanRequest", "username": username, "clanName": clan_name}),
        Event::ClanJoinNotification {
            username,
            clan_name,
            world,
        } => {
            json!({"@": "ClanJoinNotification", "username": username, "clanName": clan_name, "world": world})
        }
        Event::ClanLeaveNotification {
            username,
            clan_name,
            world,
        } => {
            json!({"@": "ClanLeaveNotification", "username": username, "clanName": clan_name, "world": world})
        }
        Event::RequestClanInfo { world, clan_owner } => {
            json!({"@": "RequestClanInfo", "world": world, "clanOwner": clan_owner})
        }
        Event::SendClanInfo {
            clan_owner,
            has_info,
            clan_name,
            join_requirement,
            kick_requirement,
            message_requirement,
            loot_requirement,
            members,
        } => {
            json!({"@": "SendClanInfo", "clanOwner": clan_owner, "hasInfo": has_info, "clanName": clan_name,
            "joinRequirement": join_requirement, "kickRequirement": kick_requirement, "messageRequirement": message_requirement,
            "lootRequirement": loot_requirement,
            "members": members.iter().map(|m| json!({"@": "ClanMember", "username": m.username, "world": m.world, "rank": m.rank})).collect::<Vec<_>>()})
        }
    }
}

fn build(spec: &Json) -> Event {
    let s = |key: &str| {
        spec.get(key)
            .and_then(Json::as_str)
            .unwrap_or("")
            .to_owned()
    };
    let i = |key: &str| spec.get(key).and_then(Json::as_i64).unwrap_or(0) as i32;
    let b = |key: &str| spec.get(key).and_then(Json::as_bool).unwrap_or(false);
    let list = |key: &str| {
        spec.get(key)
            .and_then(Json::as_array)
            .cloned()
            .unwrap_or_default()
    };
    match spec["@"].as_str().unwrap() {
        "PlayerStatusUpdate" => Event::PlayerStatusUpdate {
            username: s("username"),
            world: i("world"),
            notify_friends_only: b("notifyFriendsOnly"),
        },
        "RequestContactInfo" => Event::RequestContactInfo {
            username: s("username"),
            world: i("world"),
        },
        "SendContactInfo" => Event::SendContactInfo {
            username: s("username"),
            contacts: list("contacts")
                .iter()
                .map(|c| ContactInfo {
                    username: c["username"].as_str().unwrap_or("").to_owned(),
                    world: c.get("world").and_then(Json::as_i64).unwrap_or(0) as i32,
                    rank: c.get("rank").and_then(Json::as_i64).unwrap_or(0) as i32,
                })
                .collect(),
            blocked: list("blocked")
                .iter()
                .map(|v| v.as_str().unwrap().to_owned())
                .collect(),
        },
        "FriendUpdate" => Event::FriendUpdate {
            kind: if s("type") == "REMOVE" {
                FriendUpdateType::Remove
            } else {
                FriendUpdateType::Add
            },
            username: s("username"),
            friend: s("friend"),
        },
        "PrivateMessage" => Event::PrivateMessage {
            sender: s("sender"),
            receiver: s("receiver"),
            message: s("message"),
            rank: i("rank"),
        },
        "ClanMessage" => Event::ClanMessage {
            sender: s("sender"),
            clan_name: s("clanName"),
            message: s("message"),
            rank: i("rank"),
        },
        "JoinClanRequest" => Event::JoinClanRequest {
            username: s("username"),
            clan_name: s("clanName"),
        },
        "LeaveClanRequest" => Event::LeaveClanRequest {
            username: s("username"),
            clan_name: s("clanName"),
        },
        "ClanJoinNotification" => Event::ClanJoinNotification {
            username: s("username"),
            clan_name: s("clanName"),
            world: i("world"),
        },
        "ClanLeaveNotification" => Event::ClanLeaveNotification {
            username: s("username"),
            clan_name: s("clanName"),
            world: i("world"),
        },
        "RequestClanInfo" => Event::RequestClanInfo {
            world: i("world"),
            clan_owner: s("clanOwner"),
        },
        "SendClanInfo" => Event::SendClanInfo {
            clan_owner: s("clanOwner"),
            has_info: b("hasInfo"),
            clan_name: s("clanName"),
            join_requirement: i("joinRequirement"),
            kick_requirement: i("kickRequirement"),
            message_requirement: i("messageRequirement"),
            loot_requirement: i("lootRequirement"),
            members: list("members")
                .iter()
                .map(|m| MemberInfo {
                    username: m["username"].as_str().unwrap().to_owned(),
                    world: m.get("world").and_then(Json::as_i64).unwrap_or(0) as i32,
                    rank: m.get("rank").and_then(Json::as_i64).unwrap_or(0) as i32,
                })
                .collect(),
        },
        other => panic!("unknown event {other}"),
    }
}

impl World {
    fn emit(&self, event: Json) {
        self.events.lock().unwrap().push(event);
    }
    fn check(&mut self, name: &str) -> Result<()> {
        let count = {
            let entry = self.calls.entry(name.to_owned()).or_insert(0);
            *entry += 1;
            *entry
        };
        let fails = self.case["failures"][name]
            .as_array()
            .is_some_and(|at| at.iter().any(|n| n.as_i64() == Some(count)));
        if fails {
            return Err(Error::Host(format!(
                "oracle.Support$Injected: {name}#{count}"
            )));
        }
        Ok(())
    }
    fn call(&mut self, name: &str, args: Vec<Json>) -> Result<()> {
        let mut event = vec![json!(name)];
        event.extend(args);
        self.emit(Json::Array(event));
        self.check(name)
    }
    fn world(&self, key: &str) -> &Json {
        &self.case["world"][key]
    }
    fn player_mut(&mut self, label: &str) -> &mut Player {
        self.players.get_mut(label).unwrap()
    }
}

struct SettingsContext<'a> {
    world: &'a mut World,
    label: String,
}
impl settings::Context for SettingsContext<'_> {
    fn varp(&mut self, _: &Settings, _: i32, _: i32) -> settings::Result<()> {
        unreachable!()
    }
    fn session(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        Ok(self.world.players[&self.label].session.then_some(0))
    }
    fn send(
        &mut self,
        _: &Settings,
        session: Option<usize>,
        frame: rs09_protocol::game::Frame,
    ) -> settings::Result<()> {
        if session.is_none() {
            self.world.stashed = Some(Error::NullPointer);
            return Err(settings::Error::Binding);
        }
        let label = self.label.clone();
        if let Err(error) = self.world.call(
            "write",
            vec![
                json!(label),
                json!(frame.opcode),
                json!("NORMAL"),
                json!(hex(&frame.payload)),
            ],
        ) {
            self.world.stashed = Some(error);
            return Err(settings::Error::Binding);
        }
        Ok(())
    }
    fn dispatcher(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        unreachable!()
    }
    fn run_energy(&mut self, _: &Settings, _: Option<usize>) -> settings::Result<()> {
        unreachable!()
    }
    fn message(&mut self, _: &Settings, _: Option<usize>, _: &str) -> settings::Result<()> {
        unreachable!()
    }
    fn string(
        &mut self,
        _: &Settings,
        _: Option<usize>,
        _: String,
        _: i32,
        _: i32,
    ) -> settings::Result<()> {
        unreachable!()
    }
    fn properties(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        unreachable!()
    }
    fn retaliating(&mut self, _: &Settings, _: Option<usize>) -> settings::Result<bool> {
        unreachable!()
    }
    fn set_retaliating(&mut self, _: &Settings, _: Option<usize>, _: bool) -> settings::Result<()> {
        unreachable!()
    }
    fn ironman(&mut self, _: &Settings) -> settings::Result<bool> {
        unreachable!()
    }
    fn container(&mut self, _: &Settings, _: settings::ContainerKind) -> settings::Result<usize> {
        unreachable!()
    }
    fn item(&mut self, _: &Settings, _: usize, _: i32) -> settings::Result<Option<usize>> {
        unreachable!()
    }
    fn weight(&mut self, _: &Settings, _: usize) -> settings::Result<f64> {
        unreachable!()
    }
}

impl Host<String> for World {
    fn name(&mut self, player: &String) -> String {
        self.players[player].name.clone()
    }
    fn with_details<R>(&mut self, player: &String, f: impl FnOnce(&mut PlayerDetails) -> R) -> R {
        f(&mut self.player_mut(player).details)
    }
    fn private_chat_setting(&mut self, player: &String) -> i32 {
        self.players[player]
            .settings
            .fields
            .read()
            .private_chat_setting
    }
    fn is_ai(&mut self, player: &String) -> bool {
        self.players[player].kind == "ai"
    }
    fn is_artificial(&mut self, player: &String) -> bool {
        self.players[player].kind != "player"
    }
    fn has_session(&mut self, player: &String) -> bool {
        self.players[player].session
    }
    fn is_resizable(&mut self, player: &String) -> bool {
        self.players[player].resizable
    }
    fn world_id(&mut self) -> i32 {
        self.world("worldId").as_i64().unwrap() as i32
    }
    fn world_name(&mut self) -> String {
        self.world("name")
            .as_str()
            .unwrap_or("2009scape")
            .to_owned()
    }
    fn dev_mode(&mut self) -> bool {
        self.world("devMode").as_bool().unwrap_or(false)
    }
    fn enable_default_clan(&mut self) -> bool {
        self.world("defaultClan").as_bool().unwrap_or(false)
    }
    fn server_name(&mut self) -> String {
        self.world("serverName").as_str().unwrap_or("").to_owned()
    }
    fn ms_secret_key(&mut self) -> String {
        self.world("msKey").as_str().unwrap_or("").to_owned()
    }
    fn global_chat_enabled(&mut self) -> bool {
        self.world("globalChat").as_bool().unwrap_or(false)
    }
    fn ms_enabled(&mut self) -> bool {
        self.world("ms").as_str() == Some("AVAILABLE")
    }
    fn ms_session(&mut self) -> bool {
        self.world("msSession").as_bool().unwrap_or(false)
    }
    fn player_by_name(&mut self, name: &str) -> Option<String> {
        let key = lowercase_root(name).replace(' ', "_");
        self.names.get(&Some(key)).cloned()
    }
    fn player_names(&mut self) -> Vec<String> {
        self.names
            .iter()
            .map(|(name, _)| name.clone().unwrap())
            .collect()
    }
    fn players(&mut self) -> Vec<String> {
        self.list.clone()
    }
    fn current_time_millis(&mut self) -> i64 {
        let now = self.clock.load(Ordering::SeqCst);
        self.emit(json!(["clock", now]));
        now
    }
    fn random_seed(&mut self) -> i64 {
        let seed = self.case["seeds"][self.seed_index].as_i64().unwrap();
        self.seed_index += 1;
        self.emit(json!(["random", seed]));
        seed
    }
    fn player_message(&mut self, player: &String, text: &str) -> Result<()> {
        self.call("playerMessage", vec![json!(player), json!(text)])
    }
    fn dispatch_message(&mut self, player: &String, text: &str) -> Result<()> {
        self.call("dispatchMessage", vec![json!(player), json!(text)])
    }
    fn api_message(&mut self, player: &String, text: &str) -> Result<()> {
        self.call("apiMessage", vec![json!(player), json!(text)])
    }
    fn send_string(
        &mut self,
        player: &String,
        text: &str,
        interface: i32,
        child: i32,
    ) -> Result<()> {
        self.call(
            "sendString",
            vec![json!(player), json!(text), json!(interface), json!(child)],
        )
    }
    fn open_component(&mut self, player: &String, id: i32) -> Result<()> {
        self.call("open", vec![json!(player), json!(id)])
    }
    fn get_attribute(&mut self, player: &String, key: &str) -> Result<Option<Value>> {
        self.emit(json!(["getAttribute", player, key]));
        self.check("getAttribute")?;
        Ok(self
            .attributes
            .entry(player.clone())
            .or_default()
            .get(&key.replace("/save:", ""))
            .cloned())
    }
    fn set_attribute(&mut self, player: &String, key: &str, value: Value) -> Result<()> {
        self.emit(json!(["setAttribute", player, key, value_json(&value)]));
        self.check("setAttribute")?;
        let stored = key.strip_prefix("/save:").unwrap_or(key).to_owned();
        self.attributes
            .entry(player.clone())
            .or_default()
            .insert(stored, value);
        Ok(())
    }
    fn is_locked(&mut self, player: &String, key: &str) -> Result<bool> {
        self.call("isLocked", vec![json!(player), json!(key)])?;
        Ok(self
            .locks
            .entry(player.clone())
            .or_default()
            .iter()
            .any(|k| k == key))
    }
    fn lock(&mut self, player: &String, key: &str, ticks: i32) -> Result<()> {
        self.call("lock", vec![json!(player), json!(key), json!(ticks)])?;
        self.locks
            .entry(player.clone())
            .or_default()
            .push(key.to_owned());
        Ok(())
    }
    fn register_chat(
        &mut self,
        player: &String,
        message: &ChatMessage<String>,
        sync: bool,
    ) -> Result<bool> {
        self.call(
            "mask",
            vec![
                json!(player),
                json!("Chat"),
                json!(sync),
                json!(message.player),
                json!(message.text),
                json!(message.effects),
                json!(message.num_chars),
                json!(message.chat_icon),
                json!(message.is_quick_chat),
            ],
        )?;
        Ok(true)
    }
    fn cipher(&mut self, player: &String) -> Result<i32> {
        self.call("cipher", vec![json!(player)])?;
        let draws = self.cipher_draws.entry(player.clone()).or_insert(0);
        let index = *draws;
        *draws += 1;
        let values = self.case["players"][player]["cipher"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        Ok(if values.is_empty() {
            0
        } else {
            values[index % values.len()].as_i64().unwrap() as i32
        })
    }
    fn write(&mut self, player: &String, packet: Written) -> Result<()> {
        self.call(
            "write",
            vec![
                json!(player),
                json!(packet.opcode),
                json!(packet.header.name()),
                json!(hex(&packet.payload)),
            ],
        )
    }
    fn ms_write(&mut self, packet: Written) -> Result<()> {
        self.call(
            "msWrite",
            vec![
                json!(packet.opcode),
                json!(packet.header.name()),
                json!(hex(&packet.payload)),
            ],
        )
    }
    fn varp(&mut self, player: &String, index: i32, value: i32, save: bool) -> Result<()> {
        self.call(
            "varp",
            vec![json!(player), json!(index), json!(value), json!(save)],
        )
    }
    fn submit_pulse(&mut self, pulse: PulseId, delay: i32) -> Result<()> {
        self.call("submit", vec![json!(pulse.0), json!(delay)])
    }
    fn stop_pulse(&mut self, pulse: PulseId) -> Result<()> {
        self.call("pulseStop", vec![json!(pulse.0)])
    }
    fn log(&mut self, origin: &str, level: Level, message: &str) -> Result<()> {
        // The oracle records the message line by line without JVM frames.
        let mut lines: Vec<&str> = message
            .split('\n')
            .map(|l| l.trim_end_matches('\r'))
            .collect();
        while lines.last() == Some(&"") {
            lines.pop();
        }
        let kept: String = lines
            .iter()
            .filter(|l| !l.starts_with("\tat ") && !l.starts_with("\t... "))
            .map(|l| format!("{l}\n"))
            .collect();
        self.call("log", vec![json!(origin), json!(level.name()), json!(kept)])
    }
    fn log_ms(&mut self, text: &str) -> Result<()> {
        self.call("logMS", vec![json!(text)])
    }
    fn log_chat(&mut self, player: &String, kind: &str, message: &str) -> Result<()> {
        self.call("logChat", vec![json!(player), json!(kind), json!(message)])
    }
    fn log_private_chat(&mut self, player: &String, receiver: &str, message: &str) -> Result<()> {
        self.call(
            "logPrivateChat",
            vec![json!(player), json!(receiver), json!(message)],
        )
    }
    fn account_info(&mut self, username: &str) -> Result<UserAccountInfo> {
        self.call("accountInfo", vec![json!(username)])?;
        let account = self.case["accounts"].get(username).cloned();
        let Some(account) = account else {
            let now = self.current_time_millis();
            return Ok(UserAccountInfo::create_default(now));
        };
        let s = |key: &str, default: &str| {
            account
                .get(key)
                .and_then(Json::as_str)
                .unwrap_or(default)
                .to_owned()
        };
        let mut info = UserAccountInfo::new(AccountFields {
            username: username.to_owned(),
            password: s("password", ""),
            uid: 0,
            rights: account.get("rights").and_then(Json::as_i64).unwrap_or(0) as i32,
            credits: 0,
            ip: String::new(),
            last_used_ip: String::new(),
            mute_end_time: 0,
            ban_end_time: 0,
            contacts: s("contacts", ""),
            blocked: s("blocked", ""),
            clan_name: s("clanName", ""),
            current_clan: s("currentClan", ""),
            clan_reqs: s("clanReqs", "1,0,8,9"),
            time_played: 0,
            last_login: 0,
            online: false,
            join_date: Timestamp::new(0),
        });
        info.set_initial_reference_values();
        Ok(info)
    }
    fn online_friends(&mut self, username: &str) -> Result<Vec<String>> {
        self.call("onlineFriends", vec![json!(username)])?;
        Ok(self.case["onlineFriends"][username]
            .as_array()
            .map(|names| {
                names
                    .iter()
                    .map(|n| n.as_str().unwrap().to_owned())
                    .collect()
            })
            .unwrap_or_default())
    }
    fn account_update(&mut self, info: &UserAccountInfo) -> Result<()> {
        self.call(
            "accountUpdate",
            vec![
                json!(info.username),
                json!(info.clan_name),
                json!(info.clan_reqs),
                json!(info.rights),
            ],
        )
    }
    fn create_account(&mut self, info: &UserAccountInfo) -> Result<bool> {
        self.call(
            "createAccount",
            vec![
                json!(info.username),
                json!(info.password),
                json!(info.rights),
            ],
        )?;
        Ok(true)
    }
    fn define_command(
        &mut self,
        name: &str,
        privilege: &str,
        usage: &str,
        description: &str,
    ) -> Result<()> {
        self.call(
            "define",
            vec![
                json!(name),
                json!(privilege),
                json!(usage),
                json!(description),
            ],
        )
    }
    fn update_chat_settings(
        &mut self,
        player: &String,
        public: i32,
        private: i32,
        trade: i32,
    ) -> Result<()> {
        let settings = self.players[player].settings.clone();
        let mut context = SettingsContext {
            world: self,
            label: player.clone(),
        };
        match settings.update_chat_values(&mut context, public, private, trade) {
            Ok(()) => Ok(()),
            Err(_) => Err(self.stashed.take().expect("stashed social failure")),
        }
    }
    fn ai_incoming_chat(
        &mut self,
        player: &String,
        opcode: i32,
        other: &str,
        message: &str,
    ) -> Result<()> {
        self.call(
            "aiChat",
            vec![json!(player), json!(opcode), json!(other), json!(message)],
        )
    }
    fn management_take(&mut self, event: &Event) {
        self.emit(json!(["take", describe(event)]));
    }
}

struct Replay {
    world: World,
    social: Social<String>,
    ids: HashMap<ClanId, usize>,
    order: Vec<ClanId>,
}

impl Replay {
    fn id(&mut self, clan: Option<ClanId>) -> Json {
        match clan {
            None => Json::Null,
            Some(clan) => {
                if let Some(&id) = self.ids.get(&clan) {
                    return json!(id);
                }
                let id = self.ids.len();
                self.ids.insert(clan, id);
                self.order.push(clan);
                json!(id)
            }
        }
    }

    fn snapshot(&mut self) -> Json {
        let mut registry: BTreeMap<String, ClanId> = BTreeMap::new();
        for (owner, clan) in self.social.registry() {
            registry.insert(owner.to_owned(), clan);
        }
        let mut registry_json = serde_json::Map::new();
        for (owner, clan) in registry {
            let id = self.id(Some(clan));
            registry_json.insert(owner, id);
        }
        let labels: Vec<String> = self.world.players.keys().cloned().collect();
        let mut people = serde_json::Map::new();
        for label in labels {
            let clan = self.social.clan_of(&label);
            let clan = self.id(clan);
            let pulse = self
                .social
                .loot_share_pulse(&label)
                .map_or(-1, |p| p.0 as i64);
            let player = &self.world.players[&label];
            let c = &player.details.communication;
            let contacts: Vec<Json> = c
                .contacts
                .iter()
                .map(|(key, contact)| {
                    json!([
                        key,
                        contact.username,
                        contact.rank.map(|r| r.ordinal()),
                        contact.world_id
                    ])
                })
                .collect();
            let fields = player.settings.fields.read();
            // What PlayerDetails.save writes back (player::communication's save strings).
            let saved = json!([
                c.contact_string()
                    .unwrap_or_else(|_| "!NullPointerException".into()),
                c.blocked_string(),
                c.clan_requirement_string()
                    .unwrap_or_else(|_| "!NullPointerException".into()),
            ]);
            people.insert(
                label.clone(),
                json!({
                    "contacts": contacts,
                    "blocked": c.blocked,
                    "clanName": c.clan_name,
                    "currentClan": c.current_clan,
                    "reqs": c.requirements.iter().map(|r| r.map(|r| r.ordinal())).collect::<Vec<_>>(),
                    "lootShare": c.loot_share,
                    "clan": clan,
                    "lootPulse": pulse,
                    "saved": saved,
                    "settings": [fields.public_chat_setting, fields.private_chat_setting, fields.trade_setting],
                }),
            );
        }
        let pending: Vec<Json> = self.social.pending().map(describe).collect();
        let mut waiting = serde_json::Map::new();
        for (name, events) in self.social.waiting() {
            waiting.insert(
                name.to_owned(),
                Json::Array(events.iter().map(describe).collect()),
            );
        }
        let mut requested = serde_json::Map::new();
        for (name, value) in self.social.requested() {
            requested.insert(name.to_owned(), json!(value));
        }
        let queued: Vec<Queued<String>> = self.social.queued_writes().cloned().collect();
        let mut writes = Vec::new();
        for packet in queued {
            writes.push(match packet {
                Queued::Contact {
                    player,
                    kind,
                    name,
                    world_id,
                } => json!(["ContactPackets", player, kind as i32, name, world_id]),
                Queued::Message {
                    player,
                    opcode,
                    other,
                    chat_icon,
                    message,
                } => json!([
                    "CommunicationMessage",
                    player,
                    opcode,
                    other,
                    chat_icon,
                    message
                ]),
                Queued::Clan {
                    player,
                    clan,
                    leave,
                } => {
                    let id = self.id(Some(clan));
                    json!(["UpdateClanChat", player, id, leave])
                }
            });
        }
        let mut clans = serde_json::Map::new();
        for (index, clan) in self.order.clone().into_iter().enumerate() {
            let c = self.social.clan(clan);
            let mut ranks = serde_json::Map::new();
            for (name, rank) in &c.ranks {
                ranks.insert(
                    name.clone().unwrap_or_else(|| "null".into()),
                    json!(rank.map(|r| r.ordinal())),
                );
            }
            let mut banned = serde_json::Map::new();
            for (name, time) in &c.banned {
                banned.insert(name.clone(), json!(time));
            }
            let reqs: Vec<Option<i32>> = [
                c.join_requirement,
                c.message_requirement,
                c.kick_requirement,
                c.loot_requirement,
            ]
            .iter()
            .map(|r| r.map(|r| r.ordinal()))
            .collect();
            clans.insert(
                index.to_string(),
                json!({
                    "owner": c.owner,
                    "name": c.name,
                    "reqs": reqs,
                    "ranks": ranks,
                    "banned": banned,
                    "players": c.players.iter().map(|e| json!([e.name, e.world_id, e.player])).collect::<Vec<_>>(),
                }),
            );
        }
        json!({
            "registry": registry_json,
            "players": people,
            "pending": pending,
            "waiting": waiting,
            "requested": requested,
            "writes": writes,
            "clans": clans,
        })
    }

    fn clan(&self, action: &Json) -> Result<ClanId> {
        let clan = match action.get("clanOf") {
            Some(label) => self.social.clan_of(&label.as_str().unwrap().to_owned()),
            None => self.social.registered(action["clan"].as_str().unwrap()),
        };
        clan.ok_or(Error::NullPointer)
    }

    fn perform(&mut self, action: &Json, input: Option<&str>) -> Result<Json> {
        let op = action["op"].as_str().unwrap();
        let p = action
            .get("player")
            .and_then(Json::as_str)
            .map(str::to_owned)
            .unwrap_or_default();
        let text = |key: &str| action[key].as_str().unwrap().to_owned();
        let rank = || {
            action
                .get("rank")
                .and_then(Json::as_i64)
                .map(|r| rank_at(r).unwrap())
        };
        let (w, s) = (&mut self.world, &mut self.social);
        match op {
            "packet" | "chat" | "pm" => {
                let opcode = match op {
                    "chat" => 237,
                    "pm" => 201,
                    _ => action["opcode"].as_i64().unwrap() as i32,
                };
                let bytes = unhex(input.expect("recorded input bytes"));
                let decoded = chat::decode(opcode, &bytes);
                w.emit(json!(["decoded", describe_packet(&decoded)]));
                if !matches!(decoded, Decoded::DecodingError(_) | Decoded::UnhandledOp) {
                    s.process(w, &p, &decoded)?;
                }
                Ok(Json::Null)
            }
            "add" => s.add(w, &p, &text("name")).map(|_| Json::Null),
            "remove" => s
                .remove(
                    w,
                    &p,
                    &text("name"),
                    action.get("block").and_then(Json::as_bool).unwrap_or(false),
                )
                .map(|_| Json::Null),
            "block" => s.block(w, &p, &text("name")).map(|_| Json::Null),
            "updateRank" => s
                .update_clan_rank(w, &p, &text("name"), rank())
                .map(|_| Json::Null),
            "hasContact" => Ok(json!(s.has_contact(w, &p, &text("name")))),
            "showActive" => Ok(json!(match action.get("target") {
                Some(target) => s.show_active(w, &p, &target.as_str().unwrap().to_owned()),
                None => s.show_active_by_name(w, &p, &text("name")),
            })),
            "sendMessage" => s
                .send_message(w, &p, &text("name"), &text("text"))
                .map(|_| Json::Null),
            "toggleLoot" => s.toggle_lootshare(w, &p).map(|_| Json::Null),
            "runPulse" => s
                .run_pulse(w, PulseId(action["id"].as_u64().unwrap() as usize))
                .map(|r| json!(r)),
            "setClan" => {
                let clan = if action.get("clan").is_some() || action.get("clanOf").is_some() {
                    Some(self.clan(action)?)
                } else {
                    None
                };
                self.social.set_clan(&mut self.world, &p, clan);
                Ok(Json::Null)
            }
            "login" => {
                s.login_events(w, &p);
                Ok(Json::Null)
            }
            "logout" => {
                s.logout_events(w, &p);
                Ok(Json::Null)
            }
            "removePlayer" => {
                w.list.retain(|l| *l != p);
                let name = w.players[&p].name.clone();
                w.names.remove(&Some(name));
                Ok(Json::Null)
            }
            "addPlayer" => {
                w.list.push(p.clone());
                let name = w.players[&p].name.clone();
                w.names.put(Some(name), p.clone());
                Ok(Json::Null)
            }
            "pump" => {
                s.pump(w);
                Ok(Json::Null)
            }
            "flush" => s.flush(w).map(|_| Json::Null),
            "publish" => {
                s.publish(build(&action["event"]));
                Ok(Json::Null)
            }
            "clock" => {
                w.clock
                    .store(action["value"].as_i64().unwrap(), Ordering::SeqCst);
                Ok(Json::Null)
            }
            "unlock" => {
                let key = text("key");
                w.locks.entry(p).or_default().retain(|k| *k != key);
                Ok(Json::Null)
            }
            "attribute" => {
                let value = input_value(&action["value"]);
                w.attributes
                    .entry(p)
                    .or_default()
                    .insert(text("key"), value);
                Ok(Json::Null)
            }
            "clanGet" => {
                let clan = s.clan_get(
                    w,
                    &text("owner"),
                    action
                        .get("create")
                        .and_then(Json::as_bool)
                        .unwrap_or(false),
                )?;
                Ok(self.id(clan))
            }
            "clanEnter" => {
                let clan = self.clan(action)?;
                Ok(json!(self.social.clan_enter(&mut self.world, clan, &p)?))
            }
            "clanLeave" => {
                let clan = self.clan(action)?;
                let remove = action
                    .get("remove")
                    .and_then(Json::as_bool)
                    .unwrap_or(false);
                match action.get("message") {
                    Some(message) => self.social.clan_leave(
                        &mut self.world,
                        clan,
                        &p,
                        remove,
                        message.as_str().unwrap(),
                    )?,
                    None => self
                        .social
                        .clan_leave_default(&mut self.world, clan, &p, remove)?,
                }
                Ok(Json::Null)
            }
            "clanKick" => {
                let clan = self.clan(action)?;
                self.social
                    .clan_kick(&mut self.world, clan, &p, &text("target"))?;
                Ok(Json::Null)
            }
            "clanMessage" => {
                let clan = self.clan(action)?;
                self.social
                    .clan_message(&mut self.world, clan, &p, &text("text"))?;
                Ok(Json::Null)
            }
            "clanRank" => {
                let clan = self.clan(action)?;
                self.social
                    .clan_rank(&mut self.world, clan, &text("name"), rank())?;
                Ok(Json::Null)
            }
            "clanClean" => {
                let clan = self.clan(action)?;
                let disable = action
                    .get("disable")
                    .and_then(Json::as_bool)
                    .unwrap_or(false);
                self.social.clan_clean(&mut self.world, clan, disable)?;
                Ok(Json::Null)
            }
            "clanDelete" => {
                let clan = self.clan(action)?;
                self.social.clan_delete(&mut self.world, clan)?;
                Ok(Json::Null)
            }
            "clanUpdate" => {
                let clan = self.clan(action)?;
                self.social.clan_update(&mut self.world, clan);
                Ok(Json::Null)
            }
            "clanIsBanned" => {
                let clan = self.clan(action)?;
                Ok(json!(self.social.clan_is_banned(
                    &mut self.world,
                    clan,
                    &text("name")
                )))
            }
            "clanGetRank" => {
                let clan = self.clan(action)?;
                Ok(json!(
                    self.social
                        .clan_rank_of(&mut self.world, clan, &p)?
                        .ordinal()
                ))
            }
            "clanSet" => {
                let clan = self.clan(action)?;
                match action["which"].as_str().unwrap() {
                    "join" => {
                        self.social
                            .clan_set_join_requirement(&mut self.world, clan, rank())?
                    }
                    "message" => self.social.clan_set_message_requirement(clan, rank()),
                    "kick" => self
                        .social
                        .clan_set_kick_requirement(&mut self.world, clan, rank()),
                    "loot" => self.social.clan_set_loot_requirement(clan, rank()),
                    _ => self.social.clan_set_name(clan, text("name")),
                }
                Ok(Json::Null)
            }
            "openSettings" => s.clan_open_settings(w, &p).map(|_| Json::Null),
            "defineGlobal" => s.define_global_commands(w).map(|_| Json::Null),
            "command" => {
                assert_eq!(action["name"], "muteglobal");
                s.mute_global(w, &p).map(|_| Json::Null)
            }
            "globalProcess" => s
                .global_process(
                    w,
                    &text("sender"),
                    &text("text"),
                    action["rights"].as_i64().unwrap() as i32,
                )
                .map(|_| Json::Null),
            "split" => Ok(json!(chat::split_chat_message(
                &text("text"),
                action["clanLength"].as_i64().unwrap() as i32,
                action.get("rank").and_then(Json::as_bool).unwrap_or(false)
            ))),
            "chatMessage" => {
                let message = ChatMessage::new(
                    w,
                    &p,
                    &text("text"),
                    action["effects"].as_i64().unwrap() as i32,
                    action["count"].as_i64().unwrap() as i32,
                )?;
                Ok(json!([
                    message.player,
                    message.text,
                    message.effects,
                    message.num_chars,
                    message.chat_icon,
                    message.is_quick_chat
                ]))
            }
            other => panic!("unknown op {other}"),
        }
    }
}

fn describe_packet(decoded: &Decoded) -> Json {
    match decoded {
        Decoded::AddFriend { username } => json!(["AddFriend", username]),
        Decoded::RemoveFriend { username } => json!(["RemoveFriend", username]),
        Decoded::AddIgnore { username } => json!(["AddIgnore", username]),
        Decoded::RemoveIgnore { username } => json!(["RemoveIgnore", username]),
        Decoded::JoinClan { clan_name } => json!(["JoinClan", clan_name]),
        Decoded::SetClanRank { username, rank } => json!(["SetClanRank", username, rank]),
        Decoded::KickFromClan { username } => json!(["KickFromClan", username]),
        Decoded::PrivateMessage { username, message } => {
            json!(["PrivateMessage", username, message])
        }
        Decoded::ChatMessage { effects, message } => json!(["ChatMessage", effects, message]),
        Decoded::ChatSetting {
            public,
            private,
            trade,
        } => json!(["ChatSetting", public, private, trade]),
        Decoded::DecodingError(message) => json!(["DecodingError", message]),
        Decoded::UnhandledOp => json!(["UnhandledOp"]),
    }
}

fn error_name(error: &Error) -> String {
    match error {
        Error::Host(text) => match text.strip_prefix("oracle.Support$Injected: ") {
            Some(rest) => format!("Injected:{rest}"),
            None => text.clone(),
        },
        other => other.class_name(),
    }
}

fn replay(row: &Json) {
    let case = row["case"].clone();
    let events: Events = Arc::new(Mutex::new(Vec::new()));
    let clock = Arc::new(AtomicI64::new(case["clock"].as_i64().unwrap_or(0)));
    let clock_fn: Arc<rs09_server::auth::Clock> = {
        let (events, clock) = (events.clone(), clock.clone());
        Arc::new(move || {
            let now = clock.load(Ordering::SeqCst);
            events.lock().unwrap().push(json!(["clock", now]));
            now
        })
    };
    let mut players = BTreeMap::new();
    for (label, spec) in case["players"].as_object().unwrap() {
        let name = spec["name"].as_str().unwrap().to_owned();
        let username = spec
            .get("username")
            .and_then(Json::as_str)
            .unwrap_or(&name)
            .to_owned();
        let mut details = PlayerDetails::new(&username, clock_fn.clone());
        {
            let mut account = lock(&details.account);
            let s = |key: &str, default: &str| {
                spec.get(key)
                    .and_then(Json::as_str)
                    .unwrap_or(default)
                    .to_owned()
            };
            account.rights = spec.get("rights").and_then(Json::as_i64).unwrap_or(0) as i32;
            account.mute_end_time = spec.get("muteEnd").and_then(Json::as_i64).unwrap_or(0);
            account.contacts = s("contacts", "");
            account.blocked = s("blocked", "");
            account.clan_name = s("clanName", "");
            account.current_clan = s("currentClan", "");
            account.clan_reqs = s("clanReqs", "1,0,8,9");
        }
        let account = details.account.clone();
        details
            .communication
            .parse_account(&lock(&account))
            .unwrap();
        let settings = Settings::default();
        if let Some(values) = spec.get("settings").and_then(Json::as_array) {
            let v = |i: usize| values[i].as_i64().unwrap() as i32;
            settings.set_chat_settings(v(0), v(1), v(2));
        }
        players.insert(
            label.clone(),
            Player {
                name,
                details,
                settings,
                kind: spec
                    .get("kind")
                    .and_then(Json::as_str)
                    .unwrap_or("player")
                    .to_owned(),
                session: spec.get("session").and_then(Json::as_bool).unwrap_or(true),
                resizable: spec
                    .get("resizable")
                    .and_then(Json::as_bool)
                    .unwrap_or(false),
            },
        );
    }
    let mut attributes: HashMap<String, HashMap<String, Value>> = HashMap::new();
    for (label, spec) in case["players"].as_object().unwrap() {
        if let Some(attrs) = spec.get("attributes").and_then(Json::as_object) {
            for (key, value) in attrs {
                attributes
                    .entry(label.clone())
                    .or_default()
                    .insert(key.clone(), input_value(value));
            }
        }
    }
    let mut world = World {
        case: case.clone(),
        events: events.clone(),
        clock,
        players,
        names: JavaHashMap::new(),
        list: Vec::new(),
        attributes,
        locks: HashMap::new(),
        cipher_draws: HashMap::new(),
        calls: HashMap::new(),
        seed_index: 0,
        stashed: None,
    };
    for label in case["online"].as_array().unwrap() {
        let label = label.as_str().unwrap().to_owned();
        let name = world.players[&label].name.clone();
        world.list.push(label.clone());
        world.names.put(Some(name), label);
    }
    let mut replay = Replay {
        world,
        social: Social::new(),
        ids: HashMap::new(),
        order: Vec::new(),
    };
    let name = case["name"].as_str().unwrap();
    let setup: Vec<Json> = std::mem::take(&mut *events.lock().unwrap());
    assert_eq!(Json::Array(setup), row["setup"], "{name}: setup calls");
    assert_eq!(replay.snapshot(), row["setupState"], "{name}: setup state");
    for (index, (action, expected)) in case["actions"]
        .as_array()
        .unwrap()
        .iter()
        .zip(row["steps"].as_array().unwrap())
        .enumerate()
    {
        let outcome = replay.perform(action, expected.get("input").and_then(Json::as_str));
        let recorded: Vec<Json> = std::mem::take(&mut *events.lock().unwrap());
        let (result, error) = match outcome {
            Ok(value) => (value, Json::Null),
            Err(error) => (Json::Null, json!(error_name(&error))),
        };
        assert_eq!(
            Json::Array(recorded),
            expected["events"],
            "{name} step {index} {action}: calls"
        );
        assert_eq!(
            error, expected["error"],
            "{name} step {index} {action}: error"
        );
        assert_eq!(
            result, expected["result"],
            "{name} step {index} {action}: result"
        );
        assert_eq!(
            replay.snapshot(),
            expected["state"],
            "{name} step {index} {action}: state"
        );
    }
}

fn fixture() -> (Vec<Json>, Json) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/social");
    let data = std::fs::read(root.join("oracle.jsonl")).unwrap();
    let provenance: Json =
        serde_json::from_slice(&std::fs::read(root.join("provenance.json")).unwrap()).unwrap();
    assert_eq!(
        hex(&Sha256::digest(&data)),
        provenance["fixtureSha256"].as_str().unwrap(),
        "fixture hash"
    );
    let rows: Vec<Json> = std::str::from_utf8(&data)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(rows.len() as u64, provenance["caseCount"].as_u64().unwrap());
    (rows, provenance)
}

#[test]
fn original_social_corpus_replays_exactly() {
    let (rows, _) = fixture();
    for row in &rows {
        replay(row);
    }
}

#[test]
fn names_and_chat_compression_match_the_original_examples() {
    use rs09_server::world::social::names::*;
    assert_eq!(string_to_long("bob"), 0xcdf);
    assert_eq!(long_to_string(0xcdf), "bob");
    assert_eq!(long_to_string(-1), "");
    assert_eq!(format_display_name("dave_smith"), "Dave Smith");
    let mut out = [0u8; 256];
    let encoded = cp1252_bytes("hello there");
    let length = encrypt_player_chat(&mut out, 0, 0, encoded.len() as i32, &encoded);
    assert_eq!(hex(&out[..length as usize]), "838c70f41d58");
    let mut position = 0;
    assert_eq!(
        decrypt_player_chat(&out[..length as usize], &mut position, 11),
        "hello there"
    );
}
