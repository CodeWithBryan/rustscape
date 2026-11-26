//! Replays helpers-oracle: the original AttackListener, Player debug
//! members and GraveController.hasGraveAt through `world::helpers`.
use rs09_server::world::{
    doors::Location,
    helpers::{self as hp, AttackHost, DebugHost, Error, Listeners, Stored, Style},
};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const FIXTURE: &str = include_str!("../../../fixtures/helpers/oracle.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/helpers/provenance.json");

fn int(v: &Value) -> i32 {
    i32::try_from(v.as_i64().unwrap()).unwrap()
}

fn place(v: &Value) -> Option<Location> {
    let a = v.as_array()?;
    Some(Location::new(int(&a[0]), int(&a[1]), int(&a[2])))
}

/// The swing handler the case gives the player.
#[derive(Clone, Copy)]
enum Handler {
    Absent,
    Typed(Option<Style>),
}

fn handler(v: &Value) -> Handler {
    match v.as_str() {
        Some("absent") => Handler::Absent,
        Some("MELEE") => Handler::Typed(Some(Style::Melee)),
        Some("RANGE") => Handler::Typed(Some(Style::Range)),
        Some("MAGIC") => Handler::Typed(Some(Style::Magic)),
        None => Handler::Typed(None),
        Some(other) => panic!("style {other}"),
    }
}

struct World {
    handler: Handler,
    administrator: bool,
    attributes: BTreeMap<String, Value>,
    /// (id, is an NPC)
    nodes: Vec<(i32, bool)>,
    graves: BTreeMap<i32, Option<Location>>,
    registered: bool,
    events: Vec<Value>,
    calls: i64,
    fail_at: i64,
}

impl World {
    fn record(&mut self, row: Value) -> hp::Result<()> {
        self.events.push(row);
        self.calls += 1;
        if self.calls == self.fail_at {
            return Err(Error::Binding("injected".into()));
        }
        Ok(())
    }
}

impl Listeners for World {
    fn flag_instant(&mut self, class: &str) -> hp::Result<()> {
        self.record(json!(["InteractionListener.flagInstant", class]))
    }
    fn on_npc(&mut self, options: &[&str]) -> hp::Result<()> {
        self.record(json!(["InteractionListener.on", "NPC", options]))?;
        self.registered = true;
        Ok(())
    }
}

impl AttackHost for World {
    type Player = ();
    type Node = usize;
    fn id(&self, node: &usize) -> i32 {
        self.nodes[*node].0
    }
    fn is_npc(&self, node: &usize) -> bool {
        self.nodes[*node].1
    }
    fn swing_handler_type(&mut self, _: &()) -> hp::Result<Option<Style>> {
        self.record(json!(["Player.getSwingHandler", false]))?;
        match self.handler {
            Handler::Absent => Err(Error::Null),
            Handler::Typed(style) => Ok(style),
        }
    }
    fn send_message(&mut self, _: &(), message: &str) -> hp::Result<()> {
        self.record(json!(["ContentAPI.sendMessage", "player", message]))
    }
    fn aggress_upon_killerwatt(&mut self, _: &(), npc: &usize) -> hp::Result<()> {
        self.record(json!([
            "KillerwattBehavior.aggressUponKillerwatt",
            "player",
            format!("n{npc}")
        ]))
    }
    fn attack(&mut self, _: &(), npc: &usize) -> hp::Result<()> {
        self.record(json!(["Entity.attack", "player", format!("n{npc}")]))
    }
}

impl DebugHost for World {
    type Player = ();
    fn attribute(&mut self, _: &(), key: &str) -> hp::Result<Option<Stored>> {
        self.record(json!(["Entity.getAttribute", key]))?;
        Ok(self.attributes.get(key).map(|v| match v {
            Value::Bool(b) => Stored::Bool(*b),
            _ => Stored::Other,
        }))
    }
    fn set_attribute(&mut self, _: &(), key: &str, value: bool) -> hp::Result<()> {
        self.record(json!(["Entity.setAttribute", key, value]))?;
        self.attributes.insert(key.into(), json!(value));
        Ok(())
    }
    fn send_message(&mut self, _: &(), text: &str) -> hp::Result<()> {
        self.record(json!(["PacketDispatch.sendMessage", text]))
    }
    fn is_administrator(&mut self, _: &()) -> hp::Result<bool> {
        self.record(json!(["PlayerDetails.getRights"]))?;
        Ok(self.administrator)
    }
}

fn op(w: &mut World, o: &[Value]) -> hp::Result<Value> {
    Ok(match o[0].as_str().unwrap() {
        "define" => {
            hp::define_listeners(w)?;
            Value::Null
        }
        "node" => {
            w.nodes.push((int(&o[1]), o[2] == true));
            json!(format!("n{}", w.nodes.len() - 1))
        }
        "attack" => {
            if !w.registered {
                // The oracle's handler field is still null: its own NullPointerException.
                return Err(Error::Null);
            }
            let node = o[1].as_u64().unwrap() as usize;
            json!(hp::attack(w, &(), &node)?)
        }
        "style" => {
            w.handler = handler(&o[1]);
            Value::Null
        }
        "toggleDebug" => {
            hp::toggle_debug(w, &())?;
            Value::Null
        }
        "debug" => {
            hp::debug(w, &(), o[1].as_str().unwrap())?;
            Value::Null
        }
        "isDebug" => json!(hp::is_debug(w, &())?),
        "attribute" => {
            let key = o[1].as_str().unwrap().to_owned();
            if o[2].is_null() {
                w.attributes.remove(&key);
            } else {
                w.attributes.insert(key, o[2].clone());
            }
            Value::Null
        }
        "rights" => {
            w.administrator = o[1] == "ADMINISTRATOR";
            Value::Null
        }
        "grave" => {
            w.graves.insert(int(&o[1]), place(&o[2]));
            Value::Null
        }
        "graveGone" => {
            w.graves.remove(&int(&o[1]));
            Value::Null
        }
        "hasGraveAt" => {
            let at = place(&o[1]);
            json!(hp::has_grave_at(w.graves.values().cloned(), at.as_ref())?)
        }
        other => panic!("operation {other}"),
    })
}

fn replay(row: &Value) -> Value {
    let mut w = World {
        handler: handler(&row["style"]),
        administrator: row["rights"] == "ADMINISTRATOR",
        attributes: row["attributes"]
            .as_object()
            .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default(),
        nodes: vec![],
        graves: BTreeMap::new(),
        registered: false,
        events: vec![],
        calls: 0,
        fail_at: row.get("failAt").and_then(Value::as_i64).unwrap_or(-1),
    };
    let mut steps = vec![];
    for o in row["ops"].as_array().unwrap() {
        let before = w.events.len();
        let mut step = Map::new();
        match op(&mut w, o.as_array().unwrap()) {
            Ok(v) => {
                step.insert("result".into(), v);
            }
            Err(e) => {
                step.insert(
                    "error".into(),
                    json!(match e {
                        Error::Null => "Null",
                        Error::Cast => "Cast",
                        Error::Binding(_) => "Binding",
                    }),
                );
            }
        }
        step.insert("events".into(), json!(w.events[before..]));
        step.insert("attributes".into(), json!(w.attributes));
        steps.push(Value::Object(step));
    }
    json!({"steps": steps, "calls": w.calls})
}

#[test]
fn original_attack_listener_debug_and_grave_lookup_match() {
    let mut count = 0;
    for (n, line) in FIXTURE.lines().enumerate() {
        let row: Value = serde_json::from_str(line).unwrap();
        let got = replay(&row);
        if got != row["expected"] {
            let want = row["expected"]["steps"].as_array().unwrap();
            let have = got["steps"].as_array().unwrap();
            for (i, (a, b)) in want.iter().zip(have).enumerate() {
                assert_eq!(b, a, "case {n} {} step {i} {}", row["name"], row["ops"][i]);
            }
            assert_eq!(got, row["expected"], "case {n} {}", row["name"]);
        }
        count += 1;
    }
    assert_eq!(count, 355);
}

#[test]
fn original_sources_and_fixture_are_pinned() {
    let provenance: Value = serde_json::from_str(PROVENANCE).unwrap();
    assert_eq!(provenance["caseCount"], FIXTURE.lines().count());
    assert_eq!(
        provenance["fixtureSha256"],
        format!("{:x}", Sha256::digest(FIXTURE))
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (name, hash) in provenance["sources"].as_object().unwrap() {
        let path = root.join(name);
        if path.is_file() {
            let bytes: Vec<_> = std::fs::read(path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(bytes)),
                hash.as_str().unwrap(),
                "{name}"
            );
        }
    }
}
