//! The test-world kit's runner (docs/test-world.md), shared by the
//! `agent_run` example and the `test_world_kit` test.
//!
//! A script is JSON: a `character` to seed before login and a list of
//! `steps`. Seeding makes the character's save the way the `live_world` and
//! `live_agent` tests do: a template character logs in and out once so the
//! server's own writer saves it, and the copy's location, items, skills and
//! attributes are replaced, then checked with the native save parser. Steps
//! are `rs09_headless::agent::Action`s, raw interface buttons, and waits on
//! what the server sent (the `View`); nothing is inferred from a sent action.
#![allow(dead_code)]

use rs09_headless::{
    agent::{Action, Agent, EQUIPMENT, Event, Held, View},
    protocol::game::Frame,
    session::{LoginRejected, SceneryAction, Session},
};
use serde_json::{Map, Value, json};
use std::{
    io,
    net::SocketAddr,
    path::Path,
    time::{Duration, Instant},
};
use tokio::time::timeout_at;

/// The character every seeded save is copied from (logged in once per saves folder).
pub const TEMPLATE: &str = "kitseed";
const TEMPLATE_PASSWORD: &str = "kitseed";
const DEFAULT_PASSWORD: &str = "test";
const DEFAULT_TIMEOUT: f64 = 20.0;
/// The port of the world people play on (`cargo xtask play`); never driven.
pub const PLAY_PORT: u16 = 43594;

pub const SKILLS: [&str; 24] = [
    "attack",
    "defence",
    "strength",
    "hitpoints",
    "ranged",
    "prayer",
    "magic",
    "cooking",
    "woodcutting",
    "fletching",
    "fishing",
    "firemaking",
    "crafting",
    "smithing",
    "mining",
    "herblore",
    "agility",
    "thieving",
    "slayer",
    "farming",
    "runecrafting",
    "hunter",
    "construction",
    "summoning",
];

fn bad(what: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, what.into())
}

fn skill_index(value: &Value) -> io::Result<usize> {
    match value {
        Value::Number(n) => n
            .as_u64()
            .map(|n| n as usize)
            .filter(|&n| n < SKILLS.len())
            .ok_or_else(|| bad(format!("skill {n} is not 0..23"))),
        Value::String(s) => s
            .parse::<usize>()
            .ok()
            .filter(|&n| n < SKILLS.len())
            .or_else(|| SKILLS.iter().position(|k| k.eq_ignore_ascii_case(s)))
            .ok_or_else(|| bad(format!("unknown skill {s:?}"))),
        other => Err(bad(format!("skill must be a name or index, not {other}"))),
    }
}

fn int(value: &Value, what: &str) -> io::Result<i32> {
    value
        .as_i64()
        .map(|n| n as i32)
        .ok_or_else(|| bad(format!("{what} must be an integer, not {value}")))
}

fn ints<const N: usize>(value: &Value, what: &str) -> io::Result<[i32; N]> {
    let list = value.as_array().filter(|a| a.len() == N).ok_or_else(|| {
        bad(format!(
            "{what} must be a list of {N} integers, not {value}"
        ))
    })?;
    let mut out = [0; N];
    for (slot, v) in out.iter_mut().zip(list) {
        *slot = int(v, what)?;
    }
    Ok(out)
}

/// The character a script seeds before it logs in.
#[derive(Clone, Debug)]
pub struct Character {
    pub name: String,
    pub password: String,
    pub location: [i32; 3],
    /// (slot, id, amount)
    pub inventory: Vec<(i32, i32, i32)>,
    /// (equipment slot, id, amount)
    pub equipment: Vec<(i32, i32, i32)>,
    /// (skill index, level)
    pub skills: Vec<(usize, i32)>,
    /// `tutorial:complete`: off Tutorial Island's rules (no experience cap)
    /// and the welcome screen at login, which the runner answers with Play.
    pub tutorial_complete: bool,
}

impl Character {
    pub fn parse(value: &Value) -> io::Result<Self> {
        let name = value["name"]
            .as_str()
            .ok_or_else(|| bad("character.name is required"))?
            .to_ascii_lowercase();
        if name.is_empty()
            || name.len() > 12
            || name == TEMPLATE
            || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Err(bad(format!(
                "character.name {name:?}: 1..12 letters, digits or _ (not {TEMPLATE})"
            )));
        }
        let items = |key: &str| -> io::Result<Vec<(i32, i32, i32)>> {
            let mut out = Vec::new();
            for (i, item) in value[key].as_array().into_iter().flatten().enumerate() {
                let slot = match &item["slot"] {
                    Value::Null if key == "inventory" => i as i32,
                    v => int(v, &format!("{key}[{i}].slot"))?,
                };
                let id = int(&item["id"], &format!("{key}[{i}].id"))?;
                let amount = match &item["amount"] {
                    Value::Null => 1,
                    v => int(v, &format!("{key}[{i}].amount"))?,
                };
                out.push((slot, id, amount));
            }
            Ok(out)
        };
        let mut skills = Vec::new();
        if let Some(map) = value["skills"].as_object() {
            for (skill, level) in map {
                let level = int(level, &format!("skills.{skill}"))?;
                if !(1..=99).contains(&level) {
                    return Err(bad(format!("skills.{skill}: level {level} is not 1..99")));
                }
                skills.push((skill_index(&Value::String(skill.clone()))?, level));
            }
        }
        Ok(Self {
            name,
            password: value["password"]
                .as_str()
                .unwrap_or(DEFAULT_PASSWORD)
                .to_owned(),
            location: ints(&value["location"], "character.location")?,
            inventory: items("inventory")?,
            equipment: items("equipment")?,
            skills,
            tutorial_complete: value["tutorialComplete"].as_bool().unwrap_or(true),
        })
    }

    /// The template's save with this character's fields replaced. Rows are in
    /// the native writer's form (every number a string; experience a double).
    pub fn edit(&self, template: &Value) -> io::Result<Value> {
        let mut save = template.clone();
        let [x, y, z] = self.location;
        save["core_data"]["location"] = Value::String(format!("{x},{y},{z}"));
        let rows = |items: &[(i32, i32, i32)]| -> Value {
            items
                .iter()
                .map(|(slot, id, amount)| {
                    json!({"slot": slot.to_string(), "id": id.to_string(),
                        "amount": amount.to_string(), "charge": "1000"})
                })
                .collect()
        };
        save["core_data"]["inventory"] = rows(&self.inventory);
        save["core_data"]["equipment"] = rows(&self.equipment);
        for &(skill, level) in &self.skills {
            let row = save["skills"]
                .as_array_mut()
                .and_then(|rows| {
                    rows.iter_mut()
                        .find(|r| r["id"].as_str() == Some(&skill.to_string()))
                })
                .ok_or_else(|| bad(format!("the template save has no skill row {skill}")))?;
            let experience =
                rs09_server::player::skills::Skills::experience_by_level(level).to_string();
            row["static"] = Value::String(level.to_string());
            row["dynamic"] = Value::String(level.to_string());
            row["experience"] = Value::String(format!("{experience}.0"));
            if skill == 3 && row.get("lifepoints").is_some() {
                row["lifepoints"] = Value::String(level.to_string());
            }
            if skill == 5 && row.get("prayerPoints").is_some() {
                row["prayerPoints"] = Value::String(format!("{level}.0"));
            }
        }
        if self.tutorial_complete {
            let attributes = save
                .as_object_mut()
                .ok_or_else(|| bad("the template save is not an object"))?
                .entry("attributes")
                .or_insert_with(|| Value::Array(vec![]));
            let list = attributes
                .as_array_mut()
                .ok_or_else(|| bad("the template's attributes are not a list"))?;
            list.retain(|a| a["key"] != "tutorial:complete");
            list.push(json!({"key": "tutorial:complete", "type": "bool", "value": true}));
        }
        Ok(save)
    }
}

/// Log in, and through the welcome screen when the save has
/// `tutorial:complete` (the server sends it before the game world).
pub async fn login(address: SocketAddr, name: &str, password: &str) -> io::Result<Agent> {
    // Code 5 (already logged in): the world has not yet removed this
    // character's previous session; it does a few ticks after a logout.
    let give_up = Instant::now() + Duration::from_secs(15);
    let already_logged_in = |e: &io::Error| {
        e.get_ref()
            .and_then(|e| e.downcast_ref::<LoginRejected>())
            .is_some_and(|r| r.code == 5)
    };
    let session = loop {
        let attempt = tokio::time::timeout(
            Duration::from_secs(20),
            Session::connect(address, false, name, password),
        )
        .await
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::TimedOut,
                format!("{name}'s login: no reply from {address} within 20 s"),
            )
        })?;
        match attempt {
            Err(e) if already_logged_in(&e) && Instant::now() < give_up => {
                tokio::time::sleep(Duration::from_millis(600)).await;
            }
            other => break other?,
        }
    };
    let mut agent = Agent::new(session.into_managed());
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    let late = || io::Error::new(io::ErrorKind::TimedOut, format!("{name}'s login"));
    loop {
        let frame = timeout_at(deadline, agent.next_frame())
            .await
            .map_err(|_| late())??;
        if agent.view.position().is_some() {
            break;
        }
        // The window pane packet (164) of LoginConfiguration.sendLobbyScreen:
        // the welcome screen 378 is up; press its Play button (child 140).
        if frame.opcode == 164 {
            agent.session.sender().send_frame(&button(378, 140)).await?;
        }
    }
    Ok(agent)
}

/// IF_BUTTON (155): a button on an open interface, as the original client sends it.
pub fn button(interface: i32, child: i32) -> Frame {
    let mut payload = ((interface << 16) | child).to_be_bytes().to_vec();
    payload.extend((-1i16).to_be_bytes());
    Frame {
        opcode: 155,
        payload,
    }
}

async fn wait_for_save(path: &Path) -> io::Result<String> {
    let end = Instant::now() + Duration::from_secs(20);
    loop {
        if let Ok(text) = std::fs::read_to_string(path)
            && rs09_server::player::save::parser::parse_text(&text).is_ok()
        {
            return Ok(text);
        }
        if Instant::now() > end {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("no save at {}", path.display()),
            ));
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// Whether `saves` belongs to a disposable world: it is under
/// `runs/test-world/`, or the folder holding it has a `world.json` (as
/// `cargo xtask test-world start` and the kit's test make).
pub fn is_test_world_saves(saves: &Path) -> bool {
    let Ok(saves) = std::path::absolute(saves) else {
        return false;
    };
    let names: Vec<_> = saves
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_ascii_lowercase())
        .collect();
    let under_runs = names
        .windows(2)
        .any(|pair| pair[0] == "runs" && pair[1] == "test-world");
    under_runs
        || saves
            .parent()
            .is_some_and(|p| p.join("world.json").is_file())
}

/// Write `character`'s save into `saves/players` before it logs in. The first
/// call for a saves folder logs the template character in and out so the
/// server writes a new character's save with its own writer. Refused unless
/// `saves` is a test world's ([`is_test_world_saves`]): seeding replaces the
/// character's save file.
pub async fn seed(address: SocketAddr, saves: &Path, character: &Character) -> io::Result<()> {
    if !is_test_world_saves(saves) {
        return Err(bad(format!(
            "{} is not a test world's saves folder (under runs/test-world/, or beside a world.json); seeding would replace {}'s save there",
            saves.display(),
            character.name
        )));
    }
    let players = saves.join("players");
    let template = players.join(format!("{TEMPLATE}.json"));
    if !template.exists() {
        let agent = login(address, TEMPLATE, TEMPLATE_PASSWORD).await?;
        agent.close().await?;
        wait_for_save(&template).await?;
        // The disconnection queue removes the character a few ticks later.
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
    let text = wait_for_save(&template).await?;
    let template: Value = serde_json::from_str(&text)?;
    let save = character.edit(&template)?;
    let text = serde_json::to_string(&save)?;
    rs09_server::player::save::parser::parse_text(&text)
        .map_err(|e| bad(format!("the seeded save does not parse: {e:?}")))?;
    std::fs::write(players.join(format!("{}.json", character.name)), text)
}

/// The state a run keeps beside the view.
struct Run {
    agent: Agent,
    /// Every event the view reported, in order.
    history: Vec<Event>,
    /// Where event conditions start reading: the last action step.
    mark: usize,
    /// Experience per skill right after login.
    baseline: [Option<i32>; 25],
    /// The NPC the last NPC step named.
    target: Option<i32>,
}

impl Run {
    fn collect(&mut self) {
        let (events, omitted) = self.agent.view.drain_events();
        if omitted > 0 {
            eprintln!("warning: the view dropped {omitted} events");
        }
        for event in events {
            eprintln!("  event {}", event.json());
            self.history.push(event);
        }
    }

    async fn pump_until(
        &mut self,
        limit: Duration,
        mut done: impl FnMut(&Self) -> io::Result<bool>,
    ) -> io::Result<bool> {
        let deadline = tokio::time::Instant::now() + limit;
        loop {
            self.collect();
            if done(self)? {
                return Ok(true);
            }
            match timeout_at(deadline, self.agent.next_frame()).await {
                Err(_) => return Ok(false),
                Ok(frame) => {
                    frame?;
                }
            }
        }
    }

    fn since_mark(&self) -> &[Event] {
        &self.history[self.mark.min(self.history.len())..]
    }

    fn npc_index(&self, selector: &Value) -> io::Result<Option<i32>> {
        let view = &self.agent.view;
        if selector == "target" {
            return Ok(self.target);
        }
        if let Some(index) = selector.get("index") {
            return Ok(Some(int(index, "npc.index")?));
        }
        let types: Vec<i32> = match (selector.get("type"), selector.get("types")) {
            (Some(t), _) => vec![int(t, "npc.type")?],
            (None, Some(Value::Array(list))) => list
                .iter()
                .map(|t| int(t, "npc.types"))
                .collect::<io::Result<_>>()?,
            _ => {
                return Err(bad(format!(
                    "npc selector {selector}: \"target\", index, type or types"
                )));
            }
        };
        let skip = selector.get("skip").and_then(Value::as_u64).unwrap_or(0) as usize;
        let Some([x, y, _]) = view.position() else {
            return Ok(None);
        };
        let mut npcs: Vec<_> = view
            .npcs()
            .into_iter()
            .filter(|n| types.contains(&n.type_id) && n.health != Some(0))
            .collect();
        npcs.sort_by_key(|n| ((n.tile[0] - x).abs().max((n.tile[1] - y).abs()), n.index));
        Ok(npcs.get(skip).map(|n| n.index))
    }

    async fn resolve_npc(&mut self, selector: &Value) -> io::Result<i32> {
        let mut found = None;
        self.pump_until(Duration::from_secs(10), |run| {
            found = run.npc_index(selector)?;
            Ok(found.is_some())
        })
        .await?;
        let index = found.ok_or_else(|| bad(format!("no NPC matches {selector}")))?;
        self.target = Some(index);
        Ok(index)
    }

    async fn held(&mut self, value: &Value, what: &str) -> io::Result<Held> {
        let id = int(value, what)?;
        let mut found = None;
        self.pump_until(Duration::from_secs(5), |run| {
            found = run.agent.view.held(id);
            Ok(found.is_some())
        })
        .await?;
        found.ok_or_else(|| bad(format!("item {id} is not in the inventory")))
    }

    fn holds(view: &View, container: i32, id: i32, amount: i64) -> bool {
        let total: i64 = view
            .container(container)
            .iter()
            .flatten()
            .filter(|i| i.id == id)
            .map(|i| i64::from(i.amount))
            .sum();
        total >= amount
    }

    /// Whether `cond` holds now.
    fn check(&self, cond: &Value) -> io::Result<bool> {
        let view = &self.agent.view;
        let map = cond
            .as_object()
            .ok_or_else(|| bad(format!("condition {cond} must be an object")))?;
        if map.len() != 1 {
            return Err(bad(format!("condition {cond}: exactly one key")));
        }
        let (key, arg) = map.iter().next().unwrap();
        Ok(match key.as_str() {
            "all" => {
                for c in arg.as_array().ok_or_else(|| bad("all: a list"))? {
                    if !self.check(c)? {
                        return Ok(false);
                    }
                }
                true
            }
            "any" => {
                for c in arg.as_array().ok_or_else(|| bad("any: a list"))? {
                    if self.check(c)? {
                        return Ok(true);
                    }
                }
                false
            }
            "not" => !self.check(arg)?,
            "position" => {
                let want = ints::<3>(arg, "position")?;
                view.position() == Some(want)
            }
            "near" => {
                let [x, y] = ints::<2>(&arg["tile"], "near.tile")?;
                let within = arg.get("within").map_or(Ok(0), |w| int(w, "near.within"))?;
                view.position()
                    .is_some_and(|[px, py, _]| (px - x).abs().max((py - y).abs()) <= within)
            }
            "npcDied" | "npcHit" => {
                let index = self.npc_index(arg)?;
                let died = key == "npcDied";
                index.is_some_and(|index| {
                    self.since_mark().iter().any(|e| match e {
                        Event::NpcRemoved {
                            index: i,
                            died: true,
                            ..
                        } => died && *i == index,
                        Event::NpcHit { index: i, .. } => !died && *i == index,
                        _ => false,
                    })
                })
            }
            "dialogue" => match arg {
                Value::Bool(b) => view.dialogue().is_some() == *b,
                Value::String(text) => view.dialogue().is_some_and(|d| {
                    let text = text.to_ascii_lowercase();
                    d.title
                        .iter()
                        .chain(&d.lines)
                        .any(|l| l.to_ascii_lowercase().contains(&text))
                }),
                _ => return Err(bad("dialogue: true, false or a text")),
            },
            "message" => {
                let text = arg.as_str().ok_or_else(|| bad("message: a text"))?;
                self.since_mark()
                    .iter()
                    .any(|e| matches!(e, Event::Message(m) if m.contains(text)))
            }
            "holding" | "wearing" => {
                let (id, amount) = match arg {
                    Value::Object(_) => {
                        (int(&arg["id"], key)?, arg["amount"].as_i64().unwrap_or(1))
                    }
                    v => (int(v, key)?, 1),
                };
                let container = if key == "holding" {
                    rs09_headless::agent::INVENTORY
                } else {
                    EQUIPMENT
                };
                Self::holds(view, container, id, amount)
            }
            "interface" => {
                let id = int(arg, "interface")?;
                view.open_interfaces().any(|i| i == id)
            }
            "xpGained" | "level" => {
                let (skill, at_least) = match arg {
                    Value::Object(m) => (
                        skill_index(m.get("skill").unwrap_or(&Value::Null))?,
                        m.get("atLeast").map_or(Ok(1), |v| int(v, "atLeast"))?,
                    ),
                    v => (skill_index(v)?, 1),
                };
                let Some(now) = view.skill(skill) else {
                    return Ok(false);
                };
                if key == "level" {
                    now.level >= at_least
                } else {
                    let before = self.baseline[skill].unwrap_or(now.experience);
                    now.experience - before >= at_least
                }
            }
            other => return Err(bad(format!("unknown condition {other:?}"))),
        })
    }
}

/// The result of a script run.
pub struct Outcome {
    /// Every step passed and nothing went wrong.
    pub ok: bool,
    /// What went wrong outside a failed wait or assertion: a step's error
    /// (bad step, lost connection), the logout or the logout save. Also in
    /// the report as `error`.
    pub error: Option<String>,
    pub report: Value,
}

fn seconds(value: &Value, default: f64) -> Duration {
    Duration::from_secs_f64(value.as_f64().unwrap_or(default).max(0.0))
}

/// Run one step; Ok(false) is a failed wait, assertion or missing dialogue.
async fn step(run: &mut Run, step: &Map<String, Value>) -> io::Result<(bool, Value)> {
    let get = |key: &str| step.get(key).unwrap_or(&Value::Null);
    let what = get("do")
        .as_str()
        .ok_or_else(|| bad(format!("step {step:?} has no \"do\"")))?;
    let action = match what {
        "wait" | "assert" => {
            let cond = get("until");
            let cond = if cond.is_null() { get("that") } else { cond };
            let limit = if what == "assert" {
                Duration::ZERO
            } else {
                seconds(get("timeout"), DEFAULT_TIMEOUT)
            };
            let ok = run.pump_until(limit, |run| run.check(cond)).await?;
            return Ok((ok, json!({"condition": cond})));
        }
        "pump" => {
            run.pump_until(seconds(get("seconds"), 1.0), |_| Ok(false))
                .await?;
            return Ok((true, Value::Null));
        }
        "snapshot" => {
            run.collect();
            return Ok((true, run.agent.view.snapshot()));
        }
        "walk" => Action::Walk {
            tile: ints(get("to"), "walk.to")?,
            running: get("run").as_bool().unwrap_or(false),
        },
        "attack" => Action::Attack {
            index: run.resolve_npc(get("npc")).await?,
        },
        "talkTo" => Action::TalkTo {
            index: run.resolve_npc(get("npc")).await?,
        },
        "npcOption" => Action::NpcOption {
            option: int(get("option"), "npcOption.option")? as u8,
            index: run.resolve_npc(get("npc")).await?,
        },
        "itemOption" => Action::ItemOption {
            option: int(get("option"), "itemOption.option")? as u8,
            item: run.held(get("item"), "itemOption.item").await?,
        },
        "useOnItem" => Action::UseOnItem {
            used: run.held(get("item"), "useOnItem.item").await?,
            target: run.held(get("target"), "useOnItem.target").await?,
        },
        "useOnNpc" => Action::UseOnNpc {
            used: run.held(get("item"), "useOnNpc.item").await?,
            index: run.resolve_npc(get("npc")).await?,
        },
        "useOnScenery" => Action::UseOnScenery {
            used: run.held(get("item"), "useOnScenery.item").await?,
            id: int(get("scenery"), "useOnScenery.scenery")?,
            tile: ints(get("tile"), "useOnScenery.tile")?,
        },
        "useOnPlayer" => Action::UseOnPlayer {
            used: run.held(get("item"), "useOnPlayer.item").await?,
            index: int(get("player"), "useOnPlayer.player")?,
        },
        "scenery" => Action::Scenery(SceneryAction {
            option: int(get("option"), "scenery.option")? as u8,
            id: int(get("scenery"), "scenery.scenery")?,
            tile: ints(get("tile"), "scenery.tile")?,
        }),
        "continue" | "choose" => {
            let mut action = None;
            let text = get("text").as_str().map(str::to_owned);
            run.pump_until(seconds(get("timeout"), 10.0), |run| {
                action = run.agent.view.dialogue().and_then(|d| match &text {
                    Some(text) => d.choose_text(text),
                    None => d.resume(),
                });
                Ok(action.is_some())
            })
            .await?;
            match action {
                Some(action) => action,
                None => {
                    return Ok((
                        false,
                        json!({"error": "no dialogue with that Continue or option"}),
                    ));
                }
            }
        }
        "close" => Action::CloseInterface,
        "button" => {
            run.mark = run.history.len();
            let frame = button(
                int(get("interface"), "button.interface")?,
                int(get("child"), "button.child")?,
            );
            run.agent.session.sender().send_frame(&frame).await?;
            return Ok((true, Value::Null));
        }
        other => return Err(bad(format!("unknown step {other:?}"))),
    };
    run.mark = run.history.len();
    run.agent.act(&action).await?;
    Ok((
        true,
        json!({"action": format!("{action:?}"), "target": run.target}),
    ))
}

/// Seed the script's character (unless `seed_character` is false), log it in,
/// run the steps and report. Stops at the first failed step.
pub async fn run(
    address: SocketAddr,
    saves: Option<&Path>,
    script: &Value,
    seed_character: bool,
) -> io::Result<Outcome> {
    if address.port() == PLAY_PORT {
        return Err(bad(format!(
            "port {PLAY_PORT} is the play world's; the kit drives only test worlds (cargo xtask test-world start)"
        )));
    }
    let character = Character::parse(&script["character"])?;
    if seed_character {
        let saves = saves.ok_or_else(|| bad("seeding needs the world's saves folder"))?;
        seed(address, saves, &character).await?;
        eprintln!("seeded {} at {:?}", character.name, character.location);
    }
    let agent = login(address, &character.name, &character.password).await?;
    let mut run = Run {
        agent,
        history: Vec::new(),
        mark: 0,
        baseline: [None; 25],
        target: None,
    };
    // Skills arrive with the login's first frames; read them once settled.
    run.pump_until(Duration::from_secs(2), |run| {
        Ok((0..SKILLS.len()).all(|s| run.agent.view.skill(s).is_some()))
    })
    .await?;
    for (skill, slot) in run.baseline.iter_mut().enumerate() {
        *slot = run.agent.view.skill(skill).map(|s| s.experience);
    }
    eprintln!(
        "{} logged in at {:?}",
        character.name,
        run.agent.view.position()
    );
    let start = run.agent.view.snapshot();
    let mut steps = Vec::new();
    let mut ok = true;
    let mut errors = Vec::new();
    for (i, s) in script["steps"].as_array().into_iter().flatten().enumerate() {
        let began = Instant::now();
        // A step's error (a bad step, a lost connection) fails the step; the
        // report and the view are kept.
        let result = match s.as_object() {
            Some(map) => step(&mut run, map).await,
            None => Err(bad(format!("step {i} is not an object"))),
        };
        let (passed, detail) = match result {
            Ok(result) => result,
            Err(e) => {
                errors.push(format!("step {i}: {e}"));
                (false, json!({"error": e.to_string()}))
            }
        };
        let took = began.elapsed().as_secs_f64();
        eprintln!(
            "step {i} {} {} ({took:.1} s)",
            s.get("do").and_then(Value::as_str).unwrap_or("?"),
            if passed { "ok" } else { "FAILED" },
        );
        steps.push(json!({"step": i, "spec": s, "ok": passed, "seconds": took, "detail": detail}));
        if !passed {
            ok = false;
            break;
        }
    }
    run.collect();
    let gained: Map<String, Value> = SKILLS
        .iter()
        .enumerate()
        .filter_map(|(i, name)| {
            let now = run.agent.view.skill(i)?.experience;
            let before = run.baseline[i]?;
            (now != before).then(|| (name.to_string(), json!(now - before)))
        })
        .collect();
    let events: Vec<Value> = run.history.iter().map(Event::json).collect();
    let view = run.agent.view.snapshot();
    // Wait for the logout save, so a later seed of this character is not
    // overwritten by it and a later login is not refused as a second one.
    let saved = saves.map(|saves| {
        saves
            .join("players")
            .join(format!("{}.json", character.name))
    });
    let modified = |path: &Path| std::fs::metadata(path).and_then(|m| m.modified()).ok();
    let before = saved.as_deref().and_then(modified);
    match run.agent.close().await {
        Err(e) => errors.push(format!("logout: {e}")),
        Ok(()) => {
            if let Some(path) = saved {
                let end = Instant::now() + Duration::from_secs(20);
                while modified(&path) == before && Instant::now() < end {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
                if modified(&path) == before {
                    errors.push(format!(
                        "the server did not write the logout save {} within 20 s",
                        path.display()
                    ));
                } else {
                    // The disconnection queue removes the character a few ticks later.
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }
    let error = (!errors.is_empty()).then(|| errors.join("; "));
    let ok = ok && error.is_none();
    let report = json!({
        "ok": ok,
        "error": error,
        "character": character.name,
        "steps": steps,
        "experienceGained": gained,
        "events": events,
        "start": start,
        "view": view,
    });
    Ok(Outcome { ok, error, report })
}
