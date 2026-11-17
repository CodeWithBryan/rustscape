use rs09_server::{
    definitions::item::Shared,
    world::update_masks::{
        self as masks, Flag, Impacts, Kind, Masks, Provider, ProviderHandle, Registry, Writer,
    },
};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};
#[derive(Clone)]
enum Payload {
    Context(u8, Shared<i32>),
    Hit(i32, i32, u8),
}
type State = Masks<u8, Payload>;
type Result<T> = masks::Result<T>;
fn flag(value: &str) -> Flag {
    match value {
        "Chat" => Flag::Chat,
        "ForceChat" => Flag::ForceChat,
        "PrimaryHit" => Flag::PrimaryHit,
        "SecondaryHit" => Flag::SecondaryHit,
        "Animate" => Flag::Animate,
        "SpotAnim" => Flag::SpotAnim,
        "Appearance" => Flag::Appearance,
        "FaceEntity" => Flag::FaceEntity,
        "FaceLocation" => Flag::FaceLocation,
        "ForceMove" => Flag::ForceMove,
        "AnimSeq" => Flag::AnimSeq,
        "TypeSwap" => Flag::TypeSwap,
        _ => panic!("unknown flag"),
    }
}
fn flag_name(value: Flag) -> String {
    format!("{value:?}")
}
fn kind_name(value: Kind) -> &'static str {
    if value == Kind::Player {
        "Player"
    } else {
        "NPC"
    }
}
fn number(v: &Value, name: &str, default: i32) -> i32 {
    v[name].as_i64().map_or(default, |n| n as i32)
}
fn payload(value: Option<&Payload>) -> Value {
    match value {
        None => Value::Null,
        Some(Payload::Context(id, value)) => json!(["context", id, *value.read()]),
        Some(Payload::Hit(damage, kind, entity)) => json!(["hit", damage, kind, entity, true]),
    }
}
struct Harness {
    input: Value,
    kind: Kind,
    providers: HashMap<(Kind, Flag), ProviderHandle>,
    all: Vec<ProviderHandle>,
    contexts: [Shared<i32>; 2],
    handlers: [usize; 2],
    queues: [VecDeque<(i32, i32)>; 2],
    clock: i64,
    events: Vec<Value>,
    calls: HashMap<String, i32>,
    buffer: Vec<u8>,
}
impl Harness {
    fn provider_id(&self, p: &ProviderHandle) -> usize {
        self.all
            .iter()
            .position(|candidate| Arc::ptr_eq(candidate, p))
            .unwrap()
    }
    fn install(&mut self, kind: Kind, requested: Flag, v: &Value) {
        let p = Arc::new(Provider {
            revision: 530,
            kind,
            flag: v["encodedFlag"].as_str().map_or(requested, flag),
            presence: number(v, "presence", 64),
            ordinal: number(v, "ordinal", 10),
        });
        self.all.push(p.clone());
        self.providers.insert((kind, requested), p);
    }
    fn emit(&mut self, state: &State, event: &str, args: Vec<Value>) -> Result<()> {
        let mut row = vec![json!(event)];
        row.extend(args);
        self.events.push(Value::Array(row));
        let count = self.calls.entry(event.to_owned()).or_default();
        *count += 1;
        let at = *count;
        let hooks = self.input["hooks"].as_array().cloned().unwrap_or_default();
        for h in hooks {
            if h["event"] == event && number(&h, "at", 1) == at {
                for action in h["actions"].as_array().unwrap() {
                    self.act(state, action)?;
                }
            }
        }
        Ok(())
    }
    fn queue(&mut self, index: usize, v: &Value) {
        self.queues[index] = v
            .as_array()
            .unwrap()
            .iter()
            .map(|x| (x[0].as_i64().unwrap() as i32, x[1].as_i64().unwrap() as i32))
            .collect();
    }
    fn act(&mut self, state: &State, a: &Value) -> Result<Value> {
        let result = match a["op"].as_str().unwrap() {
            "register" => {
                let f = flag(a["flag"].as_str().unwrap_or("FaceLocation"));
                let c = a["ctx"]
                    .as_u64()
                    .map(|i| Payload::Context(i as u8, self.contexts[i as usize].clone()));
                json!(state.register(self, f, c, a["sync"].as_bool().unwrap_or(false))?)
            }
            "unregister" => json!(state.unregister_synced(number(a, "ordinal", 10))?),
            "reset" => {
                state.reset();
                Value::Null
            }
            "stamp" => {
                state.set_appearance_stamp(a["value"].as_i64().unwrap());
                Value::Null
            }
            "write" => {
                let entity = if a.get("entity").is_some_and(Value::is_null) {
                    None
                } else {
                    Some(number(a, "entity", 0) as u8)
                };
                let viewer = if a.get("viewer").is_some_and(Value::is_null) {
                    None
                } else {
                    Some(number(a, "viewer", 0) as u8)
                };
                // Encoder hooks explicitly publish this buffer while invoking
                // nested writes, preserving the original shared byte prefix.
                let mut buffer = std::mem::take(&mut self.buffer);
                let result = if a["synced"].as_bool().unwrap_or(false) {
                    state.write_synced(
                        self,
                        viewer,
                        entity.as_ref(),
                        a["appearance"].as_bool().unwrap_or(true),
                        &mut buffer,
                    )
                } else {
                    state.write(self, viewer, entity.as_ref(), &mut buffer)
                };
                self.buffer = buffer;
                result?;
                Value::Null
            }
            "prepare" => {
                state.prepare(self, number(a, "entity", 0) as u8)?;
                Value::Null
            }
            "context" => {
                *self.contexts[number(a, "id", 0) as usize].write() = number(a, "value", 0);
                Value::Null
            }
            "handler" => {
                self.handlers[number(a, "entity", 0) as usize] = number(a, "value", 0) as usize;
                Value::Null
            }
            "queue" => {
                self.queue(number(a, "handler", 0) as usize, &a["value"]);
                Value::Null
            }
            "provider" => {
                let f = flag(a["flag"].as_str().unwrap());
                if a["value"].is_null() {
                    self.providers.remove(&(self.kind, f));
                } else {
                    self.install(self.kind, f, &a["value"]);
                }
                Value::Null
            }
            "fail" => return Err(masks::Error::Binding("oracle service".into())),
            op => panic!("unknown operation {op}"),
        };
        Ok(result)
    }
    fn snapshot(&self, state: &State, result: Value, error: Option<&str>) -> Value {
        let snapshot = state.snapshot();
        let status = snapshot.status;
        let slots = |slots: Vec<Option<masks::Element<Payload>>>| {
            slots
                .iter()
                .map(|e| {
                    e.as_ref().map_or(Value::Null, |e| {
                        json!([self.provider_id(&e.encoder), payload(e.context.as_ref())])
                    })
                })
                .collect::<Vec<_>>()
        };
        json!({"result":result,"error":error,"events":self.events,"stamp":status.appearance_stamp,"updating":status.updating,
            "required":status.update_required(),"hasSynced":status.has_synced(),"presence":status.presence,"syncedPresence":status.synced_presence,
            "current":slots(snapshot.current),"synced":slots(snapshot.synced),"bytes":self.buffer,"queues":self.queues,
            "handlers":self.handlers,"contexts":[*self.contexts[0].read(),*self.contexts[1].read()],"clock":self.clock})
    }
}
impl Registry<u8, Payload> for Harness {
    fn provider(
        &mut self,
        state: &State,
        kind: Kind,
        flag: Flag,
    ) -> Result<Option<ProviderHandle>> {
        let p = self.providers.get(&(kind, flag)).cloned();
        self.emit(
            state,
            "provider",
            vec![
                json!(kind_name(kind)),
                json!(flag_name(flag)),
                json!(p.as_ref().map(|p| self.provider_id(p))),
            ],
        )?;
        Ok(p)
    }
    fn missing(&mut self, state: &State, kind: Kind, flag: Flag) -> Result<()> {
        self.emit(
            state,
            "missing",
            vec![json!(format!(
                "Tried to use flag {} which is not available for {} in this revision.",
                flag_name(flag),
                kind_name(kind)
            ))],
        )
    }
    fn now_millis(&mut self, state: &State) -> Result<i64> {
        let clock = self.clock;
        self.clock = self.clock.wrapping_add(1);
        self.emit(state, "clock", vec![json!(clock)])?;
        Ok(clock)
    }
}
impl Writer<u8, Payload> for Harness {
    fn is_player(&self, entity: Option<&u8>) -> bool {
        entity == Some(&0)
    }
    fn appearance_presence(&mut self, state: &State, kind: Kind) -> Result<i32> {
        let presence = self
            .providers
            .get(&(kind, Flag::Appearance))
            .map_or(0, |p| p.presence);
        self.emit(
            state,
            "presence",
            vec![json!(kind_name(kind)), json!("Appearance"), json!(presence)],
        )?;
        Ok(presence)
    }
    fn encode(
        &mut self,
        state: &State,
        provider: ProviderHandle,
        context: Option<Payload>,
        viewer: u8,
        buffer: &mut Vec<u8>,
    ) -> Result<()> {
        buffer.push(160);
        let id = self.provider_id(&provider);
        self.buffer = std::mem::take(buffer);
        let result = self.emit(
            state,
            "encode",
            vec![json!(id), payload(context.as_ref()), json!(viewer)],
        );
        *buffer = std::mem::take(&mut self.buffer);
        result?;
        let value = match context {
            None => 0,
            Some(Payload::Context(_, v)) => *v.read(),
            Some(Payload::Hit(d, _, _)) => d,
        };
        buffer.extend([id as u8, value as u8, viewer]);
        Ok(())
    }
}
impl Impacts<u8, Payload> for Harness {
    type Handler = usize;
    type Impact = (i32, i32);
    fn handler(&mut self, state: &State, entity: &u8) -> Result<usize> {
        let handler = self.handlers[*entity as usize];
        self.emit(state, "handler", vec![json!(entity), json!(handler)])?;
        Ok(handler)
    }
    fn peek(&mut self, state: &State, handler: &usize) -> Result<bool> {
        let result = self.queues[*handler].front().copied();
        self.emit(state, "peek", vec![json!(handler), json!(result)])?;
        Ok(result.is_some())
    }
    fn poll(&mut self, state: &State, handler: &usize) -> Result<Option<Self::Impact>> {
        let result = self.queues[*handler].pop_front();
        self.emit(state, "poll", vec![json!(handler), json!(result)])?;
        Ok(result)
    }
    fn hit_payload(&self, entity: u8, impact: Self::Impact) -> Payload {
        Payload::Hit(impact.0, impact.1, entity)
    }
}
pub fn evaluate(input: &Value) -> Value {
    let kind = if input["kind"] == "NPC" {
        Kind::Npc
    } else {
        Kind::Player
    };
    let state = State::new(u8::from(kind == Kind::Npc), kind);
    let mut h = Harness {
        input: input.clone(),
        kind,
        providers: HashMap::new(),
        all: vec![],
        contexts: [Shared::new(17), Shared::new(33)],
        handlers: [0, 1],
        queues: Default::default(),
        clock: 1000,
        events: vec![],
        calls: HashMap::new(),
        buffer: vec![],
    };
    for kind in [Kind::Player, Kind::Npc] {
        for p in input["defaults"][kind_name(kind)].as_array().unwrap() {
            h.install(kind, flag(p["flag"].as_str().unwrap()), p);
        }
    }
    for p in input["providers"].as_array().cloned().unwrap_or_default() {
        h.install(kind, flag(p["flag"].as_str().unwrap()), &p);
    }
    h.queue(0, &input.get("queue").cloned().unwrap_or(json!([])));
    Value::Array(
        input["actions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| {
                h.events.clear();
                let (result, error) = match h.act(&state, a) {
                    Ok(v) => (v, None),
                    Err(masks::Error::Null) => (Value::Null, Some("Null")),
                    Err(masks::Error::Index) => (Value::Null, Some("Index")),
                    Err(masks::Error::Binding(_)) => (Value::Null, Some("Binding")),
                };
                h.snapshot(&state, result, error)
            })
            .collect(),
    )
}
