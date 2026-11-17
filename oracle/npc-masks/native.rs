//! Native replay of the npc-masks corpus: `world::npc_masks::write_to` with a
//! recording host (the entity's skills reads, client index and the log).
use rs09_server::world::{
    npc_masks::{self, Context, Error, Host, PROVIDERS},
    update_masks::Flag,
};
use serde_json::{Value, json};

/// The recorded entity of a case.
#[derive(Clone, Debug, PartialEq)]
pub struct Entity {
    index: i32,
    max: i32,
    life: Vec<i32>,
}

struct Recorder {
    events: Vec<Value>,
    reads: usize,
}

impl Host<Entity> for Recorder {
    fn maximum_lifepoints(&mut self, entity: &Entity) -> npc_masks::Result<i32> {
        self.events.push(json!(["max"]));
        Ok(entity.max)
    }
    fn lifepoints(&mut self, entity: &Entity) -> npc_masks::Result<i32> {
        let at = self.reads.min(entity.life.len().saturating_sub(1));
        let value = *entity
            .life
            .get(at)
            .ok_or(Error::Binding("no life".into()))?;
        self.reads += 1;
        self.events.push(json!(["life", value]));
        Ok(value)
    }
    fn client_index(&mut self, entity: &Entity) -> npc_masks::Result<i32> {
        self.events.push(json!(["index"]));
        Ok(entity.index)
    }
    fn log_invalid(&mut self, provider: &'static str, message: &str) -> npc_masks::Result<()> {
        self.events.push(json!(["log", provider, "ERR", message]));
        Ok(())
    }
}

fn int(v: &Value) -> i32 {
    v.as_i64().expect("an int") as i32
}

fn entity(v: &Value) -> Option<(Entity, &'static str)> {
    if v.is_null() {
        return None;
    }
    let class = if v["class"] == "Player" {
        "Player"
    } else {
        "NPC"
    };
    Some((
        Entity {
            index: int(&v["index"]),
            max: int(&v["max"]),
            life: v["life"].as_array().unwrap().iter().map(int).collect(),
        },
        class,
    ))
}

fn context(v: &Value) -> Option<Context<Entity>> {
    if v.is_null() {
        return None;
    }
    Some(match v["type"].as_str().unwrap() {
        "HitMark" => Context::HitMark {
            damage: int(&v["damage"]),
            kind: int(&v["kind"]),
            entity: entity(&v["entity"]).map(|(e, _)| e),
        },
        "Animation" => Context::Animation {
            id: int(&v["id"]),
            delay: int(&v["delay"]),
        },
        "Graphics" => Context::Graphics {
            id: int(&v["id"]),
            height: int(&v["height"]),
            delay: int(&v["delay"]),
        },
        "Entity" => {
            let (entity, class) = entity(v).unwrap();
            Context::Entity { entity, class }
        }
        "Integer" => Context::Int(int(&v["value"])),
        "String" => Context::Text(v["value"].as_str().unwrap().to_owned()),
        "Location" => Context::Location {
            x: int(&v["x"]),
            y: int(&v["y"]),
        },
        "Long" => Context::Other("Long"),
        other => panic!("context {other}"),
    })
}

/// One case: the provider row and `writeTo`'s bytes, calls and error.
pub fn evaluate(case: &Value) -> Value {
    let name = case["provider"].as_str().unwrap();
    let (flag, presence, ordinal) = PROVIDERS
        .iter()
        .copied()
        .find(|(flag, _, _)| npc_masks::class_name(*flag) == Some(name))
        .unwrap_or_else(|| panic!("provider {name}"));
    let mut host = Recorder {
        events: Vec::new(),
        reads: 0,
    };
    let mut out = Vec::new();
    let context = context(&case["context"]);
    let error = match npc_masks::write_to(&mut host, flag, context.as_ref(), &mut out) {
        Ok(()) => Value::Null,
        Err(Error::Null) => json!("Null"),
        Err(Error::Arithmetic) => json!("Arithmetic"),
        Err(Error::Binding(text)) => panic!("binding {text}"),
    };
    let flag_name = match flag {
        Flag::AnimSeq => "AnimSeq".to_owned(),
        other => format!("{other:?}"),
    };
    json!({
        "bytes": out,
        "error": error,
        "events": host.events,
        "flag": flag_name,
        "ordinal": ordinal,
        "presence": presence,
        "revision": 530,
        "type": "NPC",
    })
}
