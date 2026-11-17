use rs09_server::{
    definitions::item::{Registry, Shared},
    inventory::{
        Error, Result,
        container::{self, Container, ContainerType, Event, Factory, SortType},
        container_listener::Handle as ListenerHandle,
        item::{Handle, Item},
        listener::{self, Listener, Packet},
    },
    world::{
        doors::Location,
        player_walk::{NodeHandle, State},
    },
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

struct Silent;
impl container::Context for Silent {
    fn update(
        &mut self,
        _: ListenerHandle,
        _: &mut Container,
        _: Event,
        _: &Registry,
    ) -> Result<()> {
        unreachable!()
    }
    fn refresh(&mut self, _: ListenerHandle, _: &mut Container, _: &Registry) -> Result<()> {
        unreachable!()
    }
    fn ground(&mut self, _: &mut Container, _: Option<Handle>, _: &Registry) -> Result<()> {
        unreachable!()
    }
}
struct Context {
    row: Value,
    definitions: std::sync::Arc<Registry>,
    null: Handle,
    player: Option<NodeHandle>,
    log: Vec<Value>,
    frames: Vec<Value>,
    packets: Vec<Packet>,
    counts: BTreeMap<String, usize>,
    pouches: [bool; 2],
    active: usize,
    familiar_reads: usize,
    syncs: usize,
}
fn number(v: &Value, key: &str, default: i32) -> i32 {
    v[key].as_i64().map_or(default, |n| n as i32)
}
fn items(v: &container::Items) -> Value {
    Value::Array(
        v.read()
            .iter()
            .map(|i| {
                i.as_ref().map_or(Value::Null, |i| {
                    let i = i.read();
                    json!([i.id(), i.amount(), i.charge()])
                })
            })
            .collect(),
    )
}
impl Context {
    fn item(&self, value: &Value) -> Option<Handle> {
        if value.is_null() {
            return None;
        }
        if value == "NULL" {
            return Some(self.null.clone());
        }
        Some(Shared::new(Item::new(
            number(value, "id", 0),
            number(value, "amount", 1),
            number(value, "charge", 1000),
            &self.definitions,
        )))
    }
    fn nil(&self, key: &str) -> bool {
        self.row["null"] == key
    }
    fn hit(&mut self, label: &str, args: &[Value], c: &mut Container) -> Result<()> {
        self.log.push(if args.is_empty() {
            json!(label)
        } else {
            let mut row = vec![json!(label)];
            row.extend_from_slice(args);
            json!(row)
        });
        let count = self.counts.entry(label.into()).or_default();
        *count += 1;
        let count = *count;
        let hooks = self.row["hooks"].as_array().unwrap().clone();
        for hook in hooks {
            if hook["when"] != label || number(&hook, "at", 1) as usize != count {
                continue;
            }
            match hook["action"].as_str().unwrap() {
                "clear" => c.clear(false, &self.definitions, &mut Silent)?,
                "pouch" => c.items().write()[0] = self.item(&json!({"id":12047})),
                "ordinary" => {
                    c.items().write().fill(None);
                    c.items().write()[0] = self.item(&json!({"id":1}));
                }
                "swap" => self.active = 1 - self.active,
                "had" => self.pouches[0] = !self.pouches[0],
                "fail" => return Err(Error::Binding("hook".into())),
                "reenter" => Listener::new(self.player.clone()).update(self, c)?,
                _ => panic!("unknown hook"),
            }
        }
        Ok(())
    }
}
impl listener::Context for Context {
    type Settings = ();
    type Familiar = usize;
    type Appearance = ();
    fn send(&mut self, packet: Packet, c: &mut Container) -> Result<()> {
        self.hit("send", &[], c)?;
        let frame = packet.frame(&self.null)?;
        let has_player = packet.player.is_some();
        self.packets.push(packet);
        if !has_player {
            return Err(Error::Null);
        }
        let hex: String = frame.payload.iter().map(|b| format!("{b:02x}")).collect();
        self.frames
            .push(json!({"opcode":frame.opcode,"payloadHex":hex}));
        Ok(())
    }
    fn settings(&mut self, _: &NodeHandle, c: &mut Container) -> Result<Option<()>> {
        self.hit("settings", &[], c)?;
        Ok((!self.nil("settings")).then_some(()))
    }
    fn update_weight(&mut self, _: (), c: &mut Container) -> Result<()> {
        self.hit("weight", &[], c)
    }
    fn familiar(&mut self, _: &NodeHandle, c: &mut Container) -> Result<Option<usize>> {
        self.hit("familiar", &[], c)?;
        self.familiar_reads += 1;
        Ok((!self.nil(&format!("familiar{}", self.familiar_reads))).then_some(self.active))
    }
    fn has_pouch(&mut self, f: usize, c: &mut Container) -> Result<bool> {
        self.hit("has", &[json!(f)], c)?;
        Ok(self.pouches[f])
    }
    fn set_has_pouch(&mut self, f: usize, value: bool, c: &mut Container) -> Result<()> {
        self.hit("set", &[json!(f), json!(value)], c)?;
        self.pouches[f] = value;
        Ok(())
    }
    fn appearance(&mut self, _: &NodeHandle, c: &mut Container) -> Result<Option<()>> {
        self.hit("appearance", &[], c)?;
        Ok((!self.nil("appearance")).then_some(()))
    }
    fn sync(&mut self, _: (), c: &mut Container) -> Result<()> {
        self.hit("sync", &[], c)?;
        self.syncs += 1;
        Ok(())
    }
}
impl container::Context for Context {
    fn update(
        &mut self,
        listener: ListenerHandle,
        c: &mut Container,
        event: Event,
        _: &Registry,
    ) -> Result<()> {
        listener
            .value::<Listener>()
            .unwrap()
            .update_event(self, c, Some(&event))
    }
    fn refresh(&mut self, listener: ListenerHandle, c: &mut Container, _: &Registry) -> Result<()> {
        listener.value::<Listener>().unwrap().refresh(self, c)
    }
    fn ground(&mut self, _: &mut Container, _: Option<Handle>, _: &Registry) -> Result<()> {
        unreachable!()
    }
}
pub fn evaluate(row: &Value) -> Value {
    let mut actors = State::default();
    let node = actors.add(1, Location::new(3207, 3216, 0), [1, 2, 3, 4]);
    let player = actors.retain_node(node).unwrap();
    let definitions = std::sync::Arc::new(Registry::default());
    let factory = Factory::new(&definitions);
    let mut c = factory
        .create(4, ContainerType::Default, SortType::Id)
        .unwrap();
    let mut ctx = Context {
        row: row.clone(),
        definitions,
        null: factory.null_item.clone(),
        player: (row["null"] != "player").then_some(player),
        log: vec![],
        frames: vec![],
        packets: vec![],
        counts: BTreeMap::new(),
        pouches: [row["had"] == true, row["otherHad"] == true],
        active: 0,
        familiar_reads: 0,
        syncs: 0,
    };
    for (i, item) in row["items"].as_array().unwrap().iter().enumerate() {
        c.items().write()[i] = ctx.item(item);
    }
    let listener = Listener::new(ctx.player.clone());
    let result = match row["op"].as_str().unwrap() {
        "update" => listener.update(&mut ctx, &mut c),
        "refresh" => listener.refresh(&mut ctx, &mut c),
        "event" => {
            let e = Shared::new(container::ContainerEvent::new(4, factory.null_item.clone()));
            e.write().clear = row["eventClear"] == true;
            for (i, item) in row["event"].as_array().unwrap().iter().enumerate() {
                e.read().flag(i as i32, ctx.item(item)).unwrap();
            }
            let event = (!ctx.nil("event")).then_some(&e);
            listener.update_event(&mut ctx, &mut c, event)
        }
        "fire" => {
            let old = c.items().read().clone();
            for (i, item) in old.iter().enumerate() {
                c.replace(
                    item.as_ref(),
                    i as i32,
                    false,
                    &ctx.definitions,
                    &mut Silent,
                )
                .unwrap();
            }
            c.register(Some(ListenerHandle::new(Listener::new(ctx.player.clone()))));
            let definitions = ctx.definitions.clone();
            c.update(None, &definitions, &mut ctx)
        }
        _ => panic!("unknown operation"),
    };
    let error = result.err().map(|e| match e {
        Error::Null => "Null",
        Error::Binding(_) => "Binding",
        _ => panic!("{e:?}"),
    });
    let packets:Vec<_>=ctx.packets.iter().map(|p|json!({"items":items(&p.items),"slots":p.slots,"sameAsContainer":p.items.same(&c.items())})).collect();
    json!({"log":ctx.log,"frames":ctx.frames,"packets":packets,"pouches":ctx.pouches,"activeFamiliar":ctx.active,"syncs":ctx.syncs,"items":items(&c.items()),"error":error})
}
