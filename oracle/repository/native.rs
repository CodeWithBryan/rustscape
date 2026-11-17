//! Replays a repository oracle case on the native library. Everything the
//! library asks of its host is answered from the case, as Support.java answers
//! the original classes, and recorded in the same form.
use rs09_game_core::java_lower::lowercase_root;
use rs09_server::world::{
    disconnection::{self, SaveTask},
    repository::{
        self, Error, Host, JavaHashMap, LogLevel, NodeList, NodeListIter, Repository, Result,
        SessionObject,
    },
};
use serde_json::{Map, Value, json};
use std::{collections::BTreeMap, rc::Rc};

/// A player or NPC of the case. Equality is identity, never the label.
#[derive(Clone)]
struct Node(Rc<str>);
impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
impl Node {
    fn label(&self) -> &str {
        &self.0
    }
}

#[derive(Clone)]
enum Object {
    Null,
    Player(String),
    Other,
}

struct Session {
    active: bool,
    key: bool,
    object: Object,
    username: Option<String>,
}

struct Model {
    input: Value,
    repository: Repository<Node>,
    nodes: BTreeMap<String, Node>,
    players: Vec<String>,
    npcs: Vec<String>,
    properties: BTreeMap<String, Map<String, Value>>,
    index: BTreeMap<String, i32>,
    playing: BTreeMap<String, bool>,
    npc_renderable: BTreeMap<String, bool>,
    sessions: BTreeMap<String, Session>,
    renderable: Vec<String>,
    chunk: Vec<String>,
    tasks: Vec<SaveTask<Node>>,
    iterators: BTreeMap<String, NodeListIter<Node>>,
    ticks: i32,
    thread: String,
    world_id: Option<i32>,
    events: Vec<Value>,
    calls: BTreeMap<String, i32>,
    quiet: bool,
}

fn category(error: &Error) -> String {
    match error {
        Error::Exception(message) => format!("exception:{message}"),
        Error::Throwable(message) => format!("error:{message}"),
        Error::Io(message) => format!("io:{message}"),
        other => format!("{other:?}"),
    }
}

fn int(value: &Value) -> i32 {
    value.as_i64().expect("integer") as i32
}

fn text(value: &Value) -> Option<String> {
    value.as_str().map(str::to_owned)
}

fn location(value: &Value) -> Option<[i32; 3]> {
    let list = value.as_array()?;
    Some([int(&list[0]), int(&list[1]), int(&list[2])])
}

impl Model {
    fn node(&self, label: &Value) -> Option<Node> {
        let label = label.as_str()?;
        Some(
            self.nodes
                .get(label)
                .unwrap_or_else(|| panic!("unknown node {label}"))
                .clone(),
        )
    }

    fn required(&self, label: &Value) -> Node {
        self.node(label).expect("the action names a node")
    }

    fn list(&self, action: &Value) -> NodeList<Node> {
        if action["list"] == "npcs" {
            self.repository.npcs()
        } else {
            self.repository.players()
        }
    }

    fn many(&self, labels: &Value) -> Vec<Node> {
        labels
            .as_array()
            .expect("node list")
            .iter()
            .map(|label| self.required(label))
            .collect()
    }

    fn property(&self, node: &Node, key: &str) -> &Value {
        self.properties[node.label()]
            .get(key)
            .unwrap_or(&Value::Null)
    }

    fn flag(&self, node: &Node, key: &str) -> bool {
        self.property(node, key) == &Value::Bool(true)
    }

    /// Records a call, runs the case's hooks for it and returns the failure
    /// the case planned for it, if any.
    fn record(&mut self, event: &str, args: Vec<Value>) -> Result<Option<(String, String)>> {
        if !self.quiet {
            let mut row = vec![json!(event)];
            row.extend(args);
            self.events.push(Value::Array(row));
        }
        let count = {
            let count = self.calls.entry(event.to_owned()).or_default();
            *count += 1;
            *count
        };
        let hooks = self.input["hooks"].as_array().cloned().unwrap_or_default();
        for hook in hooks {
            if hook["event"] == event && hook["at"].as_i64().unwrap_or(1) == i64::from(count) {
                for action in hook["actions"].as_array().expect("hook actions") {
                    self.act(action)?;
                }
            }
        }
        Ok(self.input["failures"][event][count.to_string()]
            .as_str()
            .map(|kind| (kind.to_owned(), format!("{event}#{count}"))))
    }

    fn emit(&mut self, event: &str, args: Vec<Value>) -> Result<()> {
        match self.record(event, args)? {
            None => Ok(()),
            Some((kind, message)) => Err(match kind.as_str() {
                "exception" => Error::Exception(message),
                "error" => Error::Throwable(message),
                kind => panic!("a {kind} failure cannot leave {event}"),
            }),
        }
    }

    /// `Socket.close`, the one call that may fail with an `IOException`.
    fn emit_io(&mut self, event: &str, args: Vec<Value>) -> Result<()> {
        match self.record(event, args)? {
            None => Ok(()),
            Some((kind, message)) => Err(match kind.as_str() {
                "io" => Error::Io(message),
                "error" => Error::Throwable(message),
                _ => Error::Exception(message),
            }),
        }
    }

    fn label_of(node: Option<Node>) -> Value {
        node.map_or(Value::Null, |node| json!(node.label()))
    }

    fn act(&mut self, a: &Value) -> Result<Value> {
        let repository = self.repository.clone();
        let queue = repository.disconnection_queue();
        let name = a["name"].as_str();
        let node = self.node(&a["node"]);
        let need = || node.clone().expect("the action names a node");
        Ok(match a["op"].as_str().expect("op") {
            "addPlayer" => {
                repository.add_player(self, &need())?;
                Value::Null
            }
            "removePlayer" => {
                repository.remove_player(self, &need())?;
                Value::Null
            }
            "byName" => Self::label_of(repository.get_player_by_name(self, name)),
            "byUid" => Self::label_of(repository.get_player_by_uid(int(&a["uid"]))),
            "news1" | "newsDefault" => {
                repository.send_default_news(self, a["text"].as_str().unwrap())?;
                Value::Null
            }
            "geNewsDefault" => {
                repository.send_grand_exchange_news(self, a["text"].as_str().unwrap())?;
                Value::Null
            }
            op @ ("news2" | "news3" | "news4" | "geNews3") => {
                let color = a["color"].as_str().unwrap_or(repository::NEWS_COLOR);
                let ge = op == "geNews3" || a["ge"] == true;
                repository.send_news(
                    self,
                    a["text"].as_str().unwrap(),
                    int(&a["icon"]),
                    color,
                    ge,
                )?;
                Value::Null
            }
            "addRenderable" => {
                repository.add_renderable_npc(self, &need());
                Value::Null
            }
            "removeRenderable" => {
                repository.remove_renderable_npc(self, &need());
                Value::Null
            }
            "findNpcId" => Self::label_of(repository.find_npc(self, int(&a["id"]))?),
            "findNpcAt" => {
                let at = location(&a["location"]).expect("location");
                Self::label_of(repository.find_npc_at(self, &at)?)
            }
            "renderableNpcs" => json!(
                repository
                    .renderable_npcs()
                    .values()
                    .iter()
                    .map(Node::label)
                    .collect::<Vec<_>>()
            ),
            "lobbyAdd" => json!(repository.lobby_players().add(need())),
            "lobbyRemove" => json!(repository.lobby_players().remove(&need())),

            "nlAdd" => {
                let node = need();
                let slot = self.list(a).add(node.clone());
                if let Some(slot) = slot {
                    self.set_index(&node, slot);
                }
                json!(slot.is_some())
            }
            "nlAddAll" => {
                let nodes = self.many(&a["nodes"]);
                let slots = self.list(a).add_all(nodes.iter().cloned());
                for (node, slot) in nodes.iter().zip(&slots) {
                    if let Some(slot) = slot {
                        self.set_index(node, *slot);
                    }
                }
                json!(slots.iter().any(Option::is_some))
            }
            "nlGet" => Self::label_of(self.list(a).get(int(&a["index"]))?),
            "nlRemoveAt" => json!(self.list(a).remove_at(int(&a["index"]))?),
            "nlRemove" => json!(self.list(a).remove(node.as_ref())),
            "nlRemoveAll" => {
                let nodes = self.many(&a["nodes"]);
                json!(self.list(a).remove_all(nodes.iter().map(Some)))
            }
            "nlRetainAll" => {
                let nodes = self.many(&a["nodes"]);
                json!(self.list(a).retain_all(|node| nodes.contains(node)))
            }
            "nlContains" => json!(self.list(a).contains(node.as_ref())),
            "nlContainsAll" => {
                let nodes = self.many(&a["nodes"]);
                json!(self.list(a).contains_all(nodes.iter().map(Some)))
            }
            "nlClear" => {
                self.list(a).clear();
                Value::Null
            }
            "nlIsEmpty" => json!(self.list(a).is_empty()),
            "nlSize" => json!(self.list(a).size()),
            "nlLength" => json!(self.list(a).length()),
            "nlToArray" => json!(
                self.list(a)
                    .to_array()?
                    .into_iter()
                    .map(Self::label_of)
                    .collect::<Vec<_>>()
            ),
            // NodeList.indexOf(node) is node.getIndex().
            "nlIndexOf" => json!(self.index[need().label()]),
            "itNew" => {
                let iterator = self.list(a).iter()?;
                self.iterators
                    .insert(a["it"].as_str().unwrap().to_owned(), iterator);
                Value::Null
            }
            "itHasNext" => json!(
                self.iterators
                    .get_mut(a["it"].as_str().unwrap())
                    .unwrap()
                    .has_next()
            ),
            "itNext" => {
                let node = self
                    .iterators
                    .get_mut(a["it"].as_str().unwrap())
                    .unwrap()
                    .next_node()?;
                json!(node.label())
            }
            "itRemove" => {
                self.iterators
                    .get_mut(a["it"].as_str().unwrap())
                    .unwrap()
                    .remove()?;
                Value::Null
            }

            "qAdd" => {
                queue.add(self, &need(), a["clear"] == true)?;
                Value::Null
            }
            "qUpdate" => {
                queue.update(self, &repository)?;
                Value::Null
            }
            "qClear" => {
                queue.clear(self, &repository)?;
                Value::Null
            }
            "qGet" => Self::label_of(queue.get(name)),
            "qContains" => json!(queue.contains(name)),
            "qRemove" => {
                queue.remove(name);
                Value::Null
            }
            "qIsEmpty" => json!(queue.is_empty()),
            "qSave" => json!(queue.save(self, &need(), a["sql"] == true)?),
            "tick" => {
                for _ in 0..a["count"].as_i64().unwrap_or(1) {
                    queue.update(self, &repository)?;
                    self.ticks = self.ticks.wrapping_add(1);
                }
                Value::Null
            }
            "setTicks" => {
                self.ticks = int(&a["value"]);
                Value::Null
            }
            "runTasks" => {
                let pending = std::mem::take(&mut self.tasks);
                for task in &pending {
                    task.run(self, &queue)?;
                }
                json!(pending.len())
            }

            "clear" => {
                disconnection::player_clear(self, &repository, &need())?;
                Value::Null
            }
            "finishClear" => {
                disconnection::finish_clear(self, &need())?;
                Value::Null
            }
            "allowRemoval" => json!(disconnection::allow_removal(self, &need())?),
            "sendLogout" => {
                disconnection::send_logout(self, &repository, &need())?;
                Value::Null
            }
            "disconnect" => {
                let session = a["session"].as_str().unwrap().to_owned();
                disconnection::session_disconnect(self, &repository, &session)?;
                Value::Null
            }
            "sessionPlayer" => {
                let session = a["session"].as_str().unwrap().to_owned();
                Self::label_of(disconnection::session_player(self, &repository, &session))
            }
            "set" => {
                let label = a["node"].as_str().unwrap();
                self.properties
                    .get_mut(label)
                    .unwrap()
                    .insert(a["field"].as_str().unwrap().to_owned(), a["value"].clone());
                Value::Null
            }
            "setSession" => {
                let session = self
                    .sessions
                    .get_mut(a["session"].as_str().unwrap())
                    .unwrap();
                if let Some(active) = a.get("active") {
                    session.active = active == true;
                }
                if let Some(object) = a.get("object") {
                    session.object = object_of(object, &self.players);
                }
                Value::Null
            }
            "renderableAdd" => {
                self.renderable.push(a["node"].as_str().unwrap().to_owned());
                Value::Null
            }
            op => panic!("unknown action {op}"),
        })
    }

    fn state(&self) -> Value {
        let slots = |list: NodeList<Node>| {
            list.occupied()
                .into_iter()
                .map(|(index, node)| json!([index, node.label()]))
                .collect::<Vec<_>>()
        };
        let queue = self.repository.disconnection_queue();
        json!({
            "players": slots(self.repository.players()),
            "playersSize": self.repository.players().size(),
            "npcs": slots(self.repository.npcs()),
            "npcsSize": self.repository.npcs().size(),
            "uids": self.repository.uid_entries().iter().map(|(uid, player)| json!([uid, player.label()])).collect::<Vec<_>>(),
            "names": self.repository.name_entries().iter().map(|(name, player)| json!([name, player.label()])).collect::<Vec<_>>(),
            "renderableNpcs": self.repository.renderable_npcs().values().iter().map(Node::label).collect::<Vec<_>>(),
            "lobby": self.repository.lobby_players().values().iter().map(Node::label).collect::<Vec<_>>(),
            "loggedIn": *self.repository.logged_in_players().read(),
            "queue": queue.entries().iter().map(|(name, player, clear)| json!([name, player.label(), clear])).collect::<Vec<_>>(),
            "timers": queue.timers().iter().map(|(name, waited)| json!([name, waited])).collect::<Vec<_>>(),
            "playerNodes": self.players.iter().map(|label| json!([label, self.index[label], self.playing[label]])).collect::<Vec<_>>(),
            "npcNodes": self.npcs.iter().map(|label| json!([label, self.index[label], self.npc_renderable[label]])).collect::<Vec<_>>(),
            "sessions": self.sessions.iter().map(|(label, session)| json!([label, session.active, match &session.object {
                Object::Null => Value::Null,
                Object::Player(player) => json!(player),
                Object::Other => json!("other"),
            }])).collect::<Vec<_>>(),
            "renderablePlayers": self.renderable,
            "tasks": self.tasks.len(),
            "ticks": self.ticks,
            "thread": self.thread,
        })
    }
}

fn object_of(value: &Value, players: &[String]) -> Object {
    match value.as_str() {
        None => Object::Null,
        Some(label) if players.iter().any(|player| player == label) => {
            Object::Player(label.to_owned())
        }
        Some(_) => Object::Other,
    }
}

impl Host<Node> for Model {
    type Session = String;
    type Clan = String;
    type Location = [i32; 3];

    fn uid(&mut self, player: &Node) -> i32 {
        self.property(player, "uid").as_i64().unwrap_or(0) as i32
    }
    fn name(&mut self, node: &Node) -> Option<String> {
        text(self.property(node, "name"))
    }
    fn details_username(&mut self, player: &Node) -> Option<String> {
        text(self.property(player, "username"))
    }
    fn set_index(&mut self, node: &Node, index: i32) {
        self.index.insert(node.label().to_owned(), index);
    }
    fn is_artificial(&mut self, player: &Node) -> bool {
        self.flag(player, "artificial")
    }
    fn afk_logout(&mut self, player: &Node) -> Option<bool> {
        self.property(player, "afk").as_bool()
    }
    fn set_playing(&mut self, player: &Node, playing: bool) {
        self.playing.insert(player.label().to_owned(), playing);
    }
    fn session(&mut self, player: &Node) -> Option<String> {
        text(self.property(player, "session")).filter(|label| self.sessions.contains_key(label))
    }
    fn ticks(&mut self) -> i32 {
        self.ticks
    }
    fn global_chat_enabled(&mut self) -> bool {
        self.input["globalChat"] == true
    }
    fn to_lower_case(&mut self, text: &str) -> String {
        // The oracle runs in the en-US locale, where toLowerCase() is the ROOT mapping.
        lowercase_root(text)
    }

    fn session_active(&mut self, session: &String) -> bool {
        self.sessions[session].active
    }
    fn set_session_active(&mut self, session: &String, active: bool) {
        self.sessions.get_mut(session).unwrap().active = active;
    }
    fn session_has_key(&mut self, session: &String) -> bool {
        self.sessions[session].key
    }
    fn key_cancel(&mut self, session: &String) -> Result<()> {
        self.emit("keyCancel", vec![json!(session)])
    }
    fn socket_close(&mut self, session: &String) -> Result<()> {
        self.emit_io("socketClose", vec![json!(session)])
    }
    fn session_object(&mut self, session: &String) -> SessionObject<Node> {
        match &self.sessions[session].object {
            Object::Null => SessionObject::Null,
            Object::Player(label) => SessionObject::Player(self.nodes[label].clone()),
            Object::Other => SessionObject::Other,
        }
    }
    fn set_session_object(&mut self, session: &String, player: Option<Node>) {
        self.sessions.get_mut(session).unwrap().object = player.map_or(Object::Null, |player| {
            Object::Player(player.label().to_owned())
        });
    }
    fn session_username(&mut self, session: &String) -> Option<String> {
        self.sessions[session].username.clone()
    }

    fn player_clear(&mut self, repository: &Repository<Node>, player: &Node) -> Result<()> {
        disconnection::player_clear(self, repository, player)
    }
    fn renderable_players_remove(&mut self, player: &Node) -> Result<bool> {
        self.emit("renderableRemove", vec![json!(player.label())])?;
        let position = self
            .renderable
            .iter()
            .position(|label| label == player.label());
        Ok(position
            .map(|position| self.renderable.remove(position))
            .is_some())
    }
    fn in_combat(&mut self, player: &Node) -> Result<bool> {
        self.emit("inCombat", vec![json!(player.label())])?;
        Ok(self.flag(player, "combat"))
    }
    fn lifepoints(&mut self, player: &Node) -> Result<i32> {
        self.emit("lifepoints", vec![json!(player.label())])?;
        Ok(self.property(player, "lifepoints").as_i64().unwrap_or(10) as i32)
    }
    fn is_dead(&mut self, player: &Node) -> Result<bool> {
        self.emit("isDead", vec![json!(player.label())])?;
        Ok(self.flag(player, "dead"))
    }
    fn is_teleporting(&mut self, player: &Node) -> Result<bool> {
        self.emit("teleporting", vec![json!(player.label())])?;
        Ok(self.flag(player, "teleporting"))
    }
    fn has_soft_script(&mut self, player: &Node) -> Result<bool> {
        self.emit("scriptQueue", vec![json!(player.label()), json!("SOFT")])?;
        Ok(self.flag(player, "soft"))
    }
    fn logout_packet(&mut self, player: &Node) -> Result<()> {
        self.emit("logoutPacket", vec![json!(player.label())])
    }
    fn world_logout_listeners(&mut self, player: &Node) -> Result<()> {
        let listeners = self.input["worldListeners"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        for listener in listeners {
            self.emit("worldLogout", vec![listener, json!(player.label())])?;
        }
        Ok(())
    }
    fn walking_queue_reset(&mut self, player: &Node) -> Result<()> {
        self.emit("walkReset", vec![json!(player.label())])
    }
    fn player_logout_listeners(&mut self, player: &Node) -> Result<()> {
        // The player's listeners are a HashMap keyed by name, filled in the case's order.
        let mut listeners = JavaHashMap::<Option<String>, ()>::new();
        for key in self
            .property(player, "listeners")
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            listeners.put(text(&key), ());
        }
        let keys: Vec<_> = listeners.iter().map(|(key, ())| key.clone()).collect();
        for key in keys {
            self.emit("playerLogout", vec![json!(key), json!(player.label())])?;
        }
        Ok(())
    }
    fn has_familiar(&mut self, player: &Node) -> Result<bool> {
        self.emit("hasFamiliar", vec![json!(player.label())])?;
        Ok(self.flag(player, "familiar"))
    }
    fn familiar_clear(&mut self, player: &Node) -> Result<()> {
        self.emit("familiarClear", vec![json!(player.label())])
    }
    fn interface_close(&mut self, player: &Node) -> Result<()> {
        self.emit("interfaceClose", vec![json!(player.label())])
    }
    fn interface_close_single_tab(&mut self, player: &Node) -> Result<()> {
        self.emit("closeSingleTab", vec![json!(player.label())])
    }
    fn entity_clear(&mut self, player: &Node) -> Result<()> {
        self.emit("entityClear", vec![json!(player.label())])
    }
    fn zone_monitor_clear(&mut self, player: &Node) -> Result<()> {
        self.emit("zoneClear", vec![json!(player.label())])
    }
    fn house_leave(&mut self, player: &Node) -> Result<()> {
        self.emit("houseLeave", vec![json!(player.label())])
    }
    fn publish_player_status(&mut self, username: &str, world: i32, friends: bool) -> Result<()> {
        self.emit(
            "publishStatus",
            vec![json!(username), json!(world), json!(friends)],
        )
    }
    fn clan(&mut self, player: &Node) -> Result<Option<String>> {
        self.emit("clan", vec![json!(player.label())])?;
        Ok(text(self.property(player, "clan")))
    }
    fn world_id(&mut self) -> Result<i32> {
        let world = self.world_id.ok_or(Error::NullPointer)?;
        self.emit("worldId", vec![])?;
        Ok(world)
    }
    fn clan_owner(&mut self, clan: &String) -> Result<Option<String>> {
        self.emit("clanOwner", vec![json!(clan)])?;
        Ok(Some(clan.clone()))
    }
    fn publish_clan_leave(&mut self, username: &str, world: i32, clan_name: &str) -> Result<()> {
        self.emit(
            "publishClanLeave",
            vec![json!(username), json!(world), json!(clan_name)],
        )
    }
    fn wealth_update(&mut self, player: &Node, force: bool) -> Result<()> {
        self.emit("wealthUpdate", vec![json!(player.label()), json!(force)])
    }
    fn clan_leave(&mut self, clan: &String, player: &Node, remove: bool) -> Result<()> {
        self.emit(
            "clanLeave",
            vec![json!(clan), json!(player.label()), json!(remove)],
        )
    }
    fn details_save(&mut self, player: &Node) -> Result<()> {
        self.emit("detailsSave", vec![json!(player.label())])
    }

    fn execute_sql(&mut self, task: SaveTask<Node>) -> Result<()> {
        self.emit("launch", vec![])?;
        self.tasks.push(task);
        Ok(())
    }
    fn set_thread_name(&mut self, name: &str) {
        self.thread = name.to_owned();
    }
    fn save_immediately(&mut self, player: &Node) -> Result<()> {
        let thread = self.thread.clone();
        self.emit(
            "saveImmediately",
            vec![json!(player.label()), json!(thread)],
        )
    }
    fn log(&mut self, level: LogLevel, message: &str) -> Result<()> {
        let level = match level {
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
        };
        self.emit(
            "log",
            vec![json!("DisconnectionQueue"), json!(level), json!(message)],
        )
    }
    fn print_stack_trace(&mut self, error: &Error) {
        // Only an injected failure records its trace in the oracle; one the
        // original raises itself goes to the JVM's stderr.
        if !matches!(
            error,
            Error::Exception(_) | Error::Throwable(_) | Error::Io(_)
        ) {
            return;
        }
        self.emit("stackTrace", vec![json!(category(error))])
            .expect("printing a stack trace cannot fail");
    }

    fn is_revenant(&mut self, npc: &Node) -> bool {
        self.flag(npc, "revenant")
    }
    fn set_renderable(&mut self, npc: &Node, renderable: bool) {
        self.npc_renderable
            .insert(npc.label().to_owned(), renderable);
    }
    fn npc_id(&mut self, npc: &Node) -> i32 {
        self.property(npc, "id").as_i64().unwrap_or(0) as i32
    }
    fn region_chunk_npcs(&mut self, at: &[i32; 3]) -> Result<Vec<Node>> {
        self.emit(
            "regionChunk",
            vec![json!(at[0]), json!(at[1]), json!(at[2])],
        )?;
        Ok(self
            .chunk
            .iter()
            .map(|label| self.nodes[label].clone())
            .collect())
    }
    fn npc_at(&mut self, npc: &Node, at: &[i32; 3]) -> bool {
        location(self.property(npc, "location")).as_ref() == Some(at)
    }
    fn ge_news_muted(&mut self, player: &Node) -> Result<bool> {
        self.emit(
            "attribute",
            vec![
                json!(player.label()),
                json!(repository::GE_NEWS_MUTE_ATTRIBUTE),
            ],
        )?;
        Ok(self.flag(player, "geMuted"))
    }
    fn send_message(&mut self, player: &Node, message: &str) -> Result<()> {
        self.emit("message", vec![json!(player.label()), json!(message)])
    }
}

fn objects(input: &Value, key: &str) -> BTreeMap<String, Map<String, Value>> {
    input[key]
        .as_object()
        .map(|map| {
            map.iter()
                .map(|(label, spec)| (label.clone(), spec.as_object().expect("spec").clone()))
                .collect()
        })
        .unwrap_or_default()
}

/// `java.util.HashMap<String, Integer>` or `<Integer, Integer>` driven directly.
fn map_case<K: repository::hash_map::Key>(
    input: &Value,
    key: impl Fn(&Value) -> K,
    shown: impl Fn(&K) -> Value,
) -> Value {
    let mut map = match input["capacity"].as_i64() {
        Some(capacity) => JavaHashMap::<K, i32>::with_capacity(capacity as i32).expect("capacity"),
        None => JavaHashMap::new(),
    };
    let mut out = Vec::new();
    for step in input["steps"].as_array().expect("steps") {
        out.push(match step[0].as_str().expect("op") {
            "put" => json!(map.put(key(&step[1]), int(&step[2]))),
            "remove" => json!(map.remove(&key(&step[1]))),
            "get" => json!(map.get(&key(&step[1]))),
            "clear" => {
                map.clear();
                Value::Null
            }
            "dump" => json!(
                map.iter()
                    .map(|(key, value)| json!([shown(key), value]))
                    .collect::<Vec<_>>()
            ),
            op => panic!("unknown map step {op}"),
        });
    }
    Value::Array(out)
}

pub fn evaluate(input: &Value) -> Value {
    match input["kind"].as_str() {
        Some("map") => return map_case(input, text, |key: &Option<String>| json!(key)),
        Some("intMap") => return map_case(input, int, |key: &i32| json!(key)),
        _ => {}
    }
    let max_players = input["maxPlayers"]
        .as_i64()
        .map_or(repository::MAX_PLAYERS, |n| n as i32);
    let max_npcs = input["maxNpcs"]
        .as_i64()
        .map_or(repository::MAX_NPCS, |n| n as i32);
    let mut model = Model {
        input: input.clone(),
        repository: Repository::new(max_players, max_npcs).expect("repository"),
        nodes: BTreeMap::new(),
        players: Vec::new(),
        npcs: Vec::new(),
        properties: BTreeMap::new(),
        index: BTreeMap::new(),
        playing: BTreeMap::new(),
        npc_renderable: BTreeMap::new(),
        sessions: BTreeMap::new(),
        renderable: Vec::new(),
        chunk: Vec::new(),
        tasks: Vec::new(),
        iterators: BTreeMap::new(),
        ticks: input["ticks"].as_i64().unwrap_or(0) as i32,
        thread: "main".to_owned(),
        world_id: match input.get("worldId") {
            Some(Value::Null) => None,
            Some(world) => Some(int(world)),
            None => Some(1),
        },
        events: Vec::new(),
        calls: BTreeMap::new(),
        quiet: false,
    };
    for (label, mut spec) in objects(input, "players") {
        if !spec.contains_key("username") {
            let name = spec.get("name").cloned().unwrap_or(Value::Null);
            spec.insert("username".to_owned(), name);
        }
        model
            .nodes
            .insert(label.clone(), Node(Rc::from(label.as_str())));
        model.index.insert(label.clone(), 0);
        model.playing.insert(
            label.clone(),
            spec.get("playing") != Some(&Value::Bool(false)),
        );
        if spec.get("renderable") != Some(&Value::Bool(false)) {
            model.renderable.push(label.clone());
        }
        model.players.push(label.clone());
        model.properties.insert(label, spec);
    }
    for (label, spec) in objects(input, "sessions") {
        let session = Session {
            active: spec.get("active") != Some(&Value::Bool(false)),
            key: spec.get("key") == Some(&Value::Bool(true)),
            object: object_of(spec.get("object").unwrap_or(&Value::Null), &model.players),
            username: spec.get("username").and_then(text),
        };
        model.sessions.insert(label, session);
    }
    for (label, spec) in objects(input, "npcs") {
        model
            .nodes
            .insert(label.clone(), Node(Rc::from(label.as_str())));
        model.index.insert(label.clone(), 0);
        model.npc_renderable.insert(label.clone(), false);
        model.npcs.push(label.clone());
        model.properties.insert(label, spec);
    }
    model.chunk = input["chunk"]
        .as_array()
        .map(|labels| labels.iter().filter_map(text).collect())
        .unwrap_or_default();

    let mut steps = Vec::new();
    for action in input["actions"].as_array().expect("actions") {
        model.events.clear();
        model.quiet = action["quiet"] == true;
        let (result, error) = match model.act(action) {
            Ok(result) => (result, Value::Null),
            Err(error) => (Value::Null, json!(category(&error))),
        };
        let mut step = json!({"result": result, "error": error, "events": model.events});
        if !model.quiet {
            step["state"] = model.state();
        }
        steps.push(step);
        model.quiet = false;
    }
    Value::Array(steps)
}
