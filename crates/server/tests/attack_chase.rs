//! Replays attack-chase-oracle: CombatPulse's private MovementPulse through
//! `world::combat_chase`, over the movement oracle's native replay world.
use serde_json::Value;
use sha2::{Digest, Sha256};

const CORPUS: &str = include_str!("../../../fixtures/attack-chase/oracle.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/attack-chase/provenance.json");
const CHASE: &str = include_str!("../../../fixtures/attack-chase/chase.json");

/// The movement oracle's replay world (its model, services and snapshot),
/// with the actions that stand for CombatPulse's statements run through
/// `world::combat_chase`.
#[allow(dead_code)]
mod native {
    include!("../../../oracle/movement/native.rs");
    use rs09_server::world::combat_chase;

    fn chase(m: &mut Model, a: &Value) -> Result<Value> {
        match a["chase"].as_str() {
            Some("new") => {
                assert_eq!(a["op"], "new");
                assert!(a["target"].is_null() && a.get("constructor").is_none());
                m.next += 1;
                let id = m.next;
                let mover = m.resolve(a.get("mover").unwrap_or(&json!("mover")));
                let p = combat_chase::new(m, id, mover)?;
                m.current = Some(p.clone());
                m.pulses.push(p);
            }
            Some("setVictim") => {
                assert_eq!(a["op"], "set");
                assert!(a["last"].is_null() && a.as_object().unwrap().len() == 4);
                let victim = m.resolve(&a["target"]);
                combat_chase::set_victim(&m.p()?, victim);
            }
            Some("updatePath") => {
                assert_eq!(a["op"], "path");
                combat_chase::update_path(&mut m.p()?, m)?;
            }
            Some(other) => panic!("chase statement {other}"),
            None => return m.act(a),
        }
        Ok(Value::Null)
    }

    pub fn evaluate_chase(input: &Value) -> Value {
        assert_eq!(input["pulseResult"], combat_chase::pulse());
        let mut m = Model {
            input: input.clone(),
            nodes: BTreeMap::new(),
            names: BTreeMap::new(),
            locations: BTreeMap::new(),
            current: None,
            pulses: vec![],
            paths: vec![],
            views: vec![],
            events: vec![],
            calls: HashMap::new(),
            next: 0,
        };
        for (i, (name, v)) in input["nodes"].as_object().unwrap().iter().enumerate() {
            let id = i + 1;
            m.names.insert(name.clone(), id);
            let kind = match v["type"].as_str().unwrap_or("location") {
                "player" => Kind::Player,
                "npc" => Kind::Npc,
                "entity" => Kind::Entity,
                "scenery" => Kind::Scenery,
                _ => Kind::Location,
            };
            m.nodes.insert(
                id,
                Record {
                    view: NodeView::new(kind, Some(Location::new(3207, 3216, 0))),
                    current: None,
                    behavior: true,
                    finder: None,
                },
            );
        }
        for (name, v) in input["nodes"].as_object().unwrap() {
            let id = m.names[name];
            m.set_node(id, v).unwrap();
        }
        let mut out = vec![];
        for a in input["actions"].as_array().unwrap() {
            m.events.clear();
            let (result, error) = match chase(&mut m, a) {
                Ok(v) => (v, None),
                Err(e) => (Value::Null, Some(failure(&e))),
            };
            out.push(m.snapshot(result, error));
        }
        json!(out)
    }
}

fn diff(a: &Value, b: &Value, path: String) -> Option<String> {
    if a == b {
        return None;
    }
    if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
        for key in a.keys().chain(b.keys()) {
            let d = diff(
                a.get(key).unwrap_or(&Value::Null),
                b.get(key).unwrap_or(&Value::Null),
                format!("{path}/{key}"),
            );
            if d.is_some() {
                return d;
            }
        }
    }
    if let (Some(a), Some(b)) = (a.as_array(), b.as_array()) {
        if a.len() == b.len() {
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                let d = diff(a, b, format!("{path}/{i}"));
                if d.is_some() {
                    return d;
                }
            }
        }
    }
    Some(format!("{path}:native={a},original={b}"))
}

#[test]
fn original_combat_chase_matches() {
    let mut statements = 0;
    let mut count = 0;
    for (i, line) in CORPUS.lines().enumerate() {
        let c: Value = serde_json::from_str(line).unwrap();
        statements += c["actions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|a| a.get("chase").is_some())
            .count();
        if let Some(d) = diff(&native::evaluate_chase(&c), &c["expected"], String::new()) {
            panic!("chase case {i}: {d}\n{}", c["actions"]);
        }
        // The same case through the movement replay alone: the statements are
        // exactly those MovementPulse calls.
        if let Some(d) = diff(&native::evaluate(&c), &c["expected"], String::new()) {
            panic!("chase case {i} as plain movement: {d}");
        }
        count += 1;
    }
    assert_eq!(count, 821);
    let chase: Value = serde_json::from_str(CHASE).unwrap();
    let recorded: u64 = chase["coverage"]["statements"]
        .as_object()
        .unwrap()
        .values()
        .map(|v| v.as_u64().unwrap())
        .sum();
    assert_eq!(statements as u64, recorded);
}

#[test]
fn original_sources_and_fixture_are_pinned() {
    let provenance: Value = serde_json::from_str(PROVENANCE).unwrap();
    let chase: Value = serde_json::from_str(CHASE).unwrap();
    assert_eq!(provenance["caseCount"], CORPUS.lines().count());
    assert_eq!(
        provenance["fixtureSha256"],
        format!("{:x}", Sha256::digest(CORPUS))
    );
    assert_eq!(chase["fixtureSha256"], provenance["fixtureSha256"]);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let hash = |path: std::path::PathBuf| {
        let bytes: Vec<_> = std::fs::read(path)
            .unwrap()
            .into_iter()
            .filter(|b| *b != b'\r')
            .collect();
        format!("{:x}", Sha256::digest(bytes))
    };
    for (name, expected) in provenance["sources"].as_object().unwrap() {
        let path = root.join(name);
        if path.is_file() {
            assert_eq!(hash(path), expected.as_str().unwrap(), "{name}");
        }
    }
    assert_eq!(
        hash(root.join("oracle/attack-chase/generate.py")),
        chase["generatorSha256"].as_str().unwrap()
    );
}
