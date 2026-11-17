use rs09_server::world::{
    packet_queue::{self, Packet, Queue},
    tick,
    update::{self, Context, Initialization, List, Metric, Sequence},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
type Node = Option<usize>;
fn n(v: &Value, key: &str, default: i32) -> i32 {
    v[key].as_i64().map_or(default, |n| n as i32)
}
fn error(e: update::Error) -> String {
    match e {
        update::Error::Binding(s) => s,
        e => e.to_string(),
    }
}
#[derive(Clone)]
struct Work {
    id: i32,
    fail: bool,
}
struct Model {
    input: Value,
    lists: BTreeMap<String, List<Node>>,
    lobby: String,
    npcs: String,
    sequence: Sequence<Node>,
    nodes: [[bool; 2]; 10],
    events: Vec<Value>,
    calls: BTreeMap<String, i32>,
    clock: usize,
    metrics: [i32; 3],
    packets: Queue<Work>,
    ticks: i32,
    packet_time: i32,
}
impl Model {
    fn emit(&mut self, phase: &str, args: Vec<Value>) -> update::Result<()> {
        let mut row = vec![json!(phase)];
        row.extend(args);
        self.events.push(json!(row));
        let count = self.calls.entry(phase.into()).or_default();
        *count += 1;
        let count = *count;
        for hook in self.input["hooks"].as_array().cloned().unwrap_or_default() {
            if hook["event"] == phase && n(&hook, "at", 1) == count {
                for a in hook["actions"].as_array().unwrap() {
                    self.act(a)?;
                }
            }
        }
        if n(&self.input["failures"], phase, -1) == count {
            return Err(update::Error::Binding(phase.into()));
        }
        Ok(())
    }
    fn act(&mut self, a: &Value) -> update::Result<Value> {
        let list = a["list"].as_str().and_then(|s| self.lists.get(s)).cloned();
        let node = a["node"].as_u64().map(|n| n as usize);
        let index = n(a, "index", 0) as usize;
        let seq = self.sequence.clone();
        match a["op"].as_str().unwrap() {
            "add" => {
                return Ok(json!(list.unwrap().add(node)));
            }
            "remove" => {
                return Ok(json!(list.unwrap().remove(&node)));
            }
            "insert" => list.unwrap().insert(index, node)?,
            "set" => return Ok(json!(list.unwrap().set(index, node)?)),
            "removeAt" => return Ok(json!(list.unwrap().remove_at(index)?)),
            "clear" => list.unwrap().clear(),
            "sync" => self.lists["players"].clone().sync(self)?,
            "start" => seq.start(self)?,
            "run" => seq.run(self)?,
            "end" => seq.end(self)?,
            "terminate" => seq.terminate(),
            "repo" => {
                let s = a["list"].as_str().unwrap().to_owned();
                if a["kind"] == "npcs" {
                    self.npcs = s;
                } else {
                    self.lobby = s;
                }
            }
            "captured" => {
                let mut s = seq.fields.write();
                match a["kind"].as_str().unwrap() {
                    "npcs" => s.npcs = list,
                    "players" => s.players = list,
                    _ => s.lobby = list,
                }
            }
            "enqueue" => self.packets.enqueue(Packet::Dispatch(Work {
                id: n(a, "id", 0),
                fail: a["fail"].as_bool().unwrap_or(false),
            })),
            "no" => self.packets.enqueue(Packet::NoProcess),
            "process" => self
                .packets
                .clone()
                .process(self)
                .map_err(update::Error::Binding)?,
            "handle" => tick::handle(self, a["skip"].as_bool().unwrap_or(false))?,
            op => panic!("unknown action {op}"),
        }
        Ok(Value::Null)
    }
    fn alias(&self, list: &Option<List<Node>>) -> Option<&str> {
        let list = list.as_ref()?;
        self.lists
            .iter()
            .find_map(|(name, l)| l.same(list).then_some(name.as_str()))
    }
    fn state(&self, result: Value, error: Option<String>) -> Value {
        let f = self.sequence.fields.read();
        let queue = self.packets.snapshot().map(|a| {
            a.into_iter()
                .map(|p| match p {
                    Packet::NoProcess => -999,
                    Packet::Dispatch(p) => p.id,
                })
                .collect::<Vec<_>>()
        });
        json!({"lists":self.lists.iter().map(|(n,l)|(n,l.values())).collect::<BTreeMap<_,_>>(),"pending":self.lists["players"].pending(),"nodes":self.nodes.iter().enumerate().map(|(i,n)|json!([i,n[0],n[1]])).collect::<Vec<_>>(),"captured":[self.alias(&f.lobby),self.alias(&f.players),self.alias(&f.npcs)],"metrics":self.metrics,"clockIndex":self.clock,"ticks":self.ticks,"packetTime":self.packet_time,"queue":queue,"events":self.events,"result":result,"error":error})
    }
    fn entity(&mut self, phase: &str, node: Node) -> update::Result<()> {
        let node = node.ok_or(update::Error::Null)?;
        if phase.starts_with("npc") != [3, 4, 7].contains(&node) {
            return Err(update::Error::Type);
        }
        self.emit(phase, vec![json!(node)])
    }
}
impl Initialization<Node> for Model {
    fn active(&mut self, node: &Node, active: bool) -> update::Result<()> {
        let i = node.ok_or(update::Error::Null)?;
        self.nodes[i][0] = active;
        if [3, 4, 7].contains(&i) {
            return Ok(());
        }
        self.emit("active", vec![json!(i), json!(active)])
    }
    fn renderable(&mut self, node: &Node, renderable: bool) -> update::Result<()> {
        let i = node.ok_or(update::Error::Null)?;
        self.nodes[i][1] = renderable;
        if [3, 4, 7].contains(&i) {
            return Ok(());
        }
        self.emit("renderable", vec![json!(i), json!(renderable)])
    }
}
impl Context<Node> for Model {
    fn lobby(&mut self) -> update::Result<List<Node>> {
        Ok(self.lists[&self.lobby].clone())
    }
    fn players(&mut self) -> update::Result<List<Node>> {
        Ok(self.lists["players"].clone())
    }
    fn npcs(&mut self) -> update::Result<List<Node>> {
        Ok(self.lists[&self.npcs].clone())
    }
    fn clear_minimap(&mut self, n: Node) -> update::Result<()> {
        self.entity("minimap", n)
    }
    fn npc_tick(&mut self, n: Node) -> update::Result<()> {
        self.entity("npcTick", n)
    }
    fn player_tick(&mut self, n: Node) -> update::Result<()> {
        self.entity("playerTick", n)
    }
    fn player_update(&mut self, n: Node) -> update::Result<()> {
        self.entity("playerUpdate", n)
    }
    fn player_reset(&mut self, n: Node) -> update::Result<()> {
        self.entity("playerReset", n)
    }
    fn npc_reset(&mut self, n: Node) -> update::Result<()> {
        self.entity("npcReset", n)
    }
    fn now_millis(&mut self) -> update::Result<i64> {
        let value = self.input["clock"]
            .as_array()
            .and_then(|a| a.get(self.clock))
            .and_then(Value::as_i64)
            .unwrap_or(self.clock as i64);
        self.clock += 1;
        self.emit("clock", vec![json!(value)])?;
        Ok(value)
    }
    fn metric(&mut self, m: Metric, v: i32) -> update::Result<()> {
        let (i, phase) = match m {
            Metric::NpcTick => (0, "npcMetric"),
            Metric::PlayerTick => (1, "playerMetric"),
            Metric::PlayerRender => (2, "renderMetric"),
        };
        self.metrics[i] = v;
        self.emit(phase, vec![json!(v)])
    }
    fn reset_chunks(&mut self) -> update::Result<()> {
        self.emit("chunks", vec![])
    }
    fn ground_items(&mut self) -> update::Result<()> {
        self.emit("ground", vec![])
    }
}
impl packet_queue::Context<Work> for Model {
    type Error = String;
    fn process(&mut self, p: &Work) -> Result<(), String> {
        self.emit("packet", vec![json!(p.id)]).map_err(error)?;
        if p.fail {
            return Err(p.id.to_string());
        }
        Ok(())
    }
    fn exception_text(&mut self, e: String) -> Result<String, String> {
        Ok(format!("failure:{e}\n"))
    }
    fn log(&mut self, _: &Work, trace: &str) -> Result<(), String> {
        self.emit(
            "log",
            vec![json!(format!("Error Processing Work: {trace}"))],
        )
        .map_err(error)
    }
}
impl tick::Context for Model {
    type Error = update::Error;
    fn now_millis(&mut self) -> update::Result<i64> {
        Context::now_millis(self)
    }
    fn packets(&mut self) -> update::Result<()> {
        self.packets
            .clone()
            .process(self)
            .map_err(update::Error::Binding)
    }
    fn packet_time(&mut self, v: i32) -> update::Result<()> {
        self.packet_time = v;
        self.emit("packetTime", vec![json!(v)])
    }
    fn disconnects(&mut self) -> update::Result<()> {
        self.emit("disconnects", vec![])
    }
    fn pulses(&mut self) -> update::Result<()> {
        self.emit("pulses", vec![])
    }
    fn listeners(&mut self) -> update::Result<()> {
        self.emit("listeners", vec![])
    }
    fn start(&mut self) -> update::Result<()> {
        self.sequence.clone().start(self)
    }
    fn run(&mut self) -> update::Result<()> {
        self.sequence.clone().run(self)
    }
    fn end(&mut self) -> update::Result<()> {
        self.sequence.clone().end(self)
    }
    fn pulse_world(&mut self) -> update::Result<()> {
        self.emit("pulseWorld", vec![])?;
        self.ticks = self.ticks.wrapping_add(1);
        Ok(())
    }
    fn managers(&mut self) -> update::Result<()> {
        self.emit("managers", vec![])
    }
    fn flush(&mut self) -> update::Result<()> {
        self.emit("flush", vec![])
    }
    fn exception(&mut self, e: update::Error) -> update::Result<()> {
        if let update::Error::Binding(s) = e {
            self.emit("exception", vec![json!(s)])
        } else {
            Ok(())
        }
    }
}
pub fn evaluate(input: &Value) -> Value {
    let mut lists = BTreeMap::new();
    for (name, default) in [
        ("players", json!([1, 2])),
        ("npcs", json!([3, 4])),
        ("lobby", json!([5])),
        ("other", json!([])),
    ] {
        let list = if name == "npcs" {
            List::copy_on_write()
        } else if name == "players" {
            List::default()
        } else {
            List::plain()
        };
        for n in input.get(name).unwrap_or(&default).as_array().unwrap() {
            list.push_direct(n.as_u64().map(|n| n as usize));
        }
        lists.insert(name.to_string(), list);
    }
    let mut m = Model {
        input: input.clone(),
        lists,
        lobby: "lobby".into(),
        npcs: "npcs".into(),
        sequence: Sequence::default(),
        nodes: [[false; 2]; 10],
        events: vec![],
        calls: BTreeMap::new(),
        clock: 0,
        metrics: [0; 3],
        packets: Queue::default(),
        ticks: n(input, "ticks", 0),
        packet_time: 0,
    };
    let mut states = vec![];
    for a in input["actions"].as_array().unwrap() {
        m.events.clear();
        let (value, error) = match m.act(a) {
            Ok(v) => (v, None),
            Err(e) => (Value::Null, Some(error(e))),
        };
        states.push(m.state(value, error));
    }
    json!(states)
}
