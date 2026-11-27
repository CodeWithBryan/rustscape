//! Replay of a dialogue content corpus (oracle/content-kit/dialogue.py) over
//! the dialogue-dsl recording host (oracle/dialogue-dsl/native.rs): the kit's
//! actions `register` (the ported plugin's prototype, then `init`) and
//! `openId` (`DialogueInterpreter.open(id, npc)`) on top of the corpus's own.
//! A content test includes both files as sibling modules:
//!
//! ```ignore
//! #[path = "../../../oracle/dialogue-dsl/native.rs"]
//! mod native;
//! #[path = "../../../oracle/content-kit/dialogue.rs"]
//! mod kit;
//! ```
use super::native::{self, Model};
use rs09_server::world::dialogue::{Arg, Plugin, Result};
use serde_json::{Value as J, json};

/// The native prototype of a scanned class name (`content::dialogue_plugin`).
pub type Prototype = fn(&str) -> Option<Plugin<Model>>;

/// Replays one case.
pub fn evaluate(row: &J, prototype: Prototype) -> J {
    native::evaluate_with(row, &mut |model, action| match action["op"].as_str()? {
        "register" => Some(register(model, action, prototype)),
        "openId" => Some(open_id(model, action)),
        _ => None,
    })
}

/// ClassScanner's DIALOGUE branch: the prototype, then `init()`.
fn register(model: &mut Model, a: &J, prototype: Prototype) -> Result<J> {
    let class = a["class"].as_str().unwrap();
    let plugin = prototype(class).unwrap_or_else(|| panic!("no native port of {class}"));
    let registry = model.interp.registry();
    plugin.init(model, &registry)?;
    Ok(J::Null)
}

/// `player.getDialogueInterpreter().open(id, npc)`, or `open(id)`.
fn open_id(model: &mut Model, a: &J) -> Result<J> {
    let key = a["key"].as_i64().unwrap() as i32;
    let args: Vec<Arg<Model>> = a["npc"]
        .as_u64()
        .map(|n| Arg::Npc(n as usize))
        .into_iter()
        .collect();
    let interp = model.interp.clone();
    Ok(json!(interp.open(model, key, &args)?))
}

/// The world's `GameWorld.settings?.name` (DialogueOps.java's `settings()`),
/// read by content: the kit's GameWorld stand-in records a read from a
/// `content.` class as "GameWorld.settings" (the dialogue core's own read,
/// native.rs `server_name`, stays "settings").
pub fn settings_name(model: &mut Model) -> Result<Option<Vec<u16>>> {
    model.emit("GameWorld.settings", vec![])?;
    let source = &model.source;
    if source.get("settings") == Some(&J::Null) {
        return Ok(None);
    }
    let name = source["serverName"].as_str().unwrap_or("2009scape");
    Ok(Some(name.encode_utf16().collect()))
}

/// The first difference between a native and an original snapshot.
pub fn difference(a: &J, b: &J, path: String) -> Option<String> {
    if a == b {
        return None;
    }
    if let (Some(a), Some(b)) = (a.as_array(), b.as_array())
        && a.len() == b.len()
    {
        for (i, (a, b)) in a.iter().zip(b).enumerate() {
            if let Some(d) = difference(a, b, format!("{path}/{i}")) {
                return Some(d);
            }
        }
    }
    if let (Some(a), Some(b)) = (a.as_object(), b.as_object()) {
        for key in a.keys().chain(b.keys()) {
            if let Some(d) = difference(
                a.get(key).unwrap_or(&J::Null),
                b.get(key).unwrap_or(&J::Null),
                format!("{path}/{key}"),
            ) {
                return Some(d);
            }
        }
    }
    Some(format!("{path}: native={a}, original={b}"))
}

/// Replays every case of a corpus (on a thread with a large stack, as the
/// dialogue-dsl replay does) and checks the case counts against its
/// provenance. Panics with the first difference, and refuses a corpus that
/// is not a port yet: no designed cases, or a scope that still says TODO
/// (what the scaffold writes).
pub fn replay(topic: &'static str, corpus: &'static str, provenance: &str, prototype: Prototype) {
    let p: J = serde_json::from_str(provenance).unwrap();
    assert!(
        p["coverage"]["designedCases"].as_u64().unwrap_or(0) > 0,
        "{topic}: the corpus has no designed cases (fill oracle/{topic}/cases.py)"
    );
    assert!(
        !p["scope"].as_str().unwrap_or("TODO").contains("TODO"),
        "{topic}: the provenance scope still says TODO"
    );
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .stack_size(32 << 20)
        .spawn(move || {
            let (mut cases, mut failures) = (0usize, 0usize);
            for (i, line) in corpus.lines().enumerate() {
                let row: J = serde_json::from_str(line).unwrap();
                let actual = evaluate(&row, prototype);
                if let Some(diff) = difference(&actual, &row["expected"], String::new()) {
                    tx.send(Err(format!("{topic} case {i} ({}): {diff}", row["name"])))
                        .unwrap();
                    return;
                }
                cases += 1;
                failures += usize::from(row["onlyLast"] == J::Bool(true));
            }
            tx.send(Ok((cases, failures))).unwrap();
        })
        .unwrap();
    let (cases, failures) = rx
        .recv_timeout(std::time::Duration::from_secs(300))
        .expect("the replay deadlocked")
        .unwrap();
    assert_eq!(p["caseCount"], cases, "{topic}: provenance caseCount");
    assert_eq!(
        p["coverage"]["failureCases"], failures,
        "{topic}: failure cases"
    );
    assert_eq!(corpus.lines().count(), cases);
    use sha2::{Digest, Sha256};
    let hash: String = Sha256::digest(corpus.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    assert_eq!(p["fixtureSha256"], hash, "{topic}: the fixture's hash");
}
