use rs09_server::{
    player::ironman::{self, Manager, Mode},
    world::{
        doors::Location,
        player_walk::{NodeHandle, State},
    },
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
fn mode(v: &Value) -> Option<Mode> {
    v.as_i64()
        .map(|n| [Mode::None, Mode::Standard, Mode::Hardcore, Mode::Ultimate][n as usize])
}
struct Messages {
    manager: Manager,
    input: Value,
    events: Vec<Value>,
}
impl ironman::Context for Messages {
    fn message(&mut self, p: Option<&NodeHandle>, text: &str) -> ironman::Result<()> {
        p.ok_or(ironman::Error::Null)?;
        self.events.push(json!(["message", text]));
        if let Some(v) = self.input.get("messageMode") {
            self.manager.set_mode(mode(v));
        }
        if self.input["failMessage"] == true {
            return Err(ironman::Error::Binding("callback".into()));
        }
        Ok(())
    }
}
#[test]
fn full_original_manager_modes_thresholds_aliases_and_callback_prefixes_match() {
    for (line, row) in include_str!("../../../fixtures/ironman/oracle.jsonl")
        .lines()
        .enumerate()
    {
        let input: Value = serde_json::from_str(row).unwrap();
        let mut state = State::default();
        let player = if input["player"] == true {
            let n = state.add(1, Location::new(3207, 3216, 0), [1, 2, 3, 4]);
            Some(state.retain_node(n).unwrap())
        } else {
            None
        };
        let manager = Manager::new(player.clone());
        let mut ctx = Messages {
            manager: manager.clone(),
            input: input.clone(),
            events: vec![],
        };
        let mut aliases = vec![];
        let mut states = vec![];
        for a in input["actions"].as_array().unwrap() {
            ctx.events.clear();
            let m = if a["alias"] == true {
                aliases.first().unwrap_or(&manager)
            } else {
                &manager
            };
            let result: ironman::Result<Value> = match a["op"].as_str().unwrap() {
                "set" => {
                    m.set_mode(mode(&a["mode"]));
                    Ok(Value::Null)
                }
                "is" => Ok(json!(m.is_ironman())),
                "default" => m.check_restriction(&mut ctx).map(|v| json!(v)),
                "check" => m.check(&mut ctx, mode(&a["minimum"])).map(|v| json!(v)),
                "owner" => Ok(json!(m.player().is_some())),
                "capture" => {
                    aliases.push(manager.clone());
                    Ok(json!(aliases[0].same(&manager)))
                }
                "icons" => Ok(json!(
                    [Mode::None, Mode::Standard, Mode::Hardcore, Mode::Ultimate].map(Mode::icon)
                )),
                _ => unreachable!(),
            };
            let (result, error) = match result {
                Ok(v) => (v, None),
                Err(ironman::Error::Null) => (Value::Null, Some("Null")),
                Err(_) => (Value::Null, Some("Binding")),
            };
            states.push(json!({"result":result,"error":error,"mode":manager.mode().map(|m|m as i32),"isIronman":manager.is_ironman(),"player":manager.player().is_some(),"samePlayer":manager.player()==player,"aliases":aliases.len(),"events":ctx.events}));
        }
        assert_eq!(json!(states), input["expected"], "case{line}: {row}");
    }
}
#[test]
fn complete_original_sources_and_fixture_are_pinned() {
    let bytes = include_bytes!("../../../fixtures/ironman/oracle.jsonl");
    let p: Value =
        serde_json::from_str(include_str!("../../../fixtures/ironman/provenance.json")).unwrap();
    assert_eq!(p["fixtureSha256"], format!("{:x}", Sha256::digest(bytes)));
    assert_eq!(
        p["caseCount"],
        std::str::from_utf8(bytes).unwrap().lines().count()
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (n, d) in p["sources"].as_object().unwrap() {
        let path = root.join(n);
        if path.is_file() {
            let b: Vec<_> = std::fs::read(path)
                .unwrap()
                .into_iter()
                .filter(|b| *b != b'\r')
                .collect();
            assert_eq!(
                format!("{:x}", Sha256::digest(b)),
                d.as_str().unwrap(),
                "{n}"
            );
        }
    }
}
