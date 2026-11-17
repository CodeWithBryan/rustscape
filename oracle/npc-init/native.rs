//! Test-only constructor/entity/property/skill/repository services.
use rs09_game_core::direction::Direction;
use rs09_server::{
    definitions::{
        item::Shared,
        npc::{Graphics, Handle as Definition, NpcDefinition},
        property::{Animation, Property},
    },
    world::{
        doors::Location,
        npc,
        npc_init::{self, Change, Context, Error, Node, Result, Skill, Slot},
    },
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
struct Actor {
    id: i32,
    index: i32,
    size: i32,
    loc: Option<Location>,
    direction: Option<Direction>,
    active: bool,
    def: Option<Definition>,
    props: Option<usize>,
    skills: Option<usize>,
    brain: npc::State,
    children: Option<Shared<Vec<Option<Node>>>>,
    kind: u8,
}
#[derive(Default)]
struct Props {
    combat: i32,
    speed: i32,
    spawn: Option<Location>,
    bonuses: Option<Shared<Vec<i32>>>,
    animation: [Option<Shared<Animation>>; 5],
    death: Option<Shared<Graphics>>,
}
#[derive(Default)]
struct Skills {
    statics: [i32; 24],
    currents: [i32; 24],
}
struct Model {
    input: Value,
    nodes: Vec<Actor>,
    props: Vec<Props>,
    skills: Vec<Skills>,
    defs: BTreeMap<i32, Definition>,
    events: Vec<Value>,
    calls: BTreeMap<String, i32>,
    repository: Vec<Node>,
    renderable: Vec<Node>,
    next_index: i32,
    recording: bool,
}
fn v(a: &Value, k: &str, d: i32) -> i32 {
    a[k].as_i64().map_or(d, |v| v as i32)
}
fn loc(v: &Value) -> Option<Location> {
    let a = v.as_array()?;
    Some(Location::new(
        a[0].as_i64()? as i32,
        a[1].as_i64()? as i32,
        a[2].as_i64()? as i32,
    ))
}
fn coord(l: &Option<Location>) -> Value {
    l.as_ref().map_or(Value::Null, |l| json!(l.coordinates()))
}
fn anim(a: &Option<Shared<Animation>>) -> Value {
    a.as_ref().map_or(Value::Null, |a| {
        let a = a.read();
        json!([a.id, a.delay, a.priority])
    })
}
fn graphic(g: &Option<Shared<Graphics>>) -> Value {
    g.as_ref().map_or(Value::Null, |g| {
        let g = g.read();
        json!([g.id, g.height, g.delay])
    })
}
fn slot(s: Slot) -> usize {
    match s {
        Slot::Attack => 0,
        Slot::Defence => 1,
        Slot::Death => 2,
        Slot::Magic => 3,
        Slot::Range => 4,
    }
}
fn name(s: Slot) -> &'static str {
    match s {
        Slot::Attack => "Attack",
        Slot::Defence => "Defence",
        Slot::Death => "Death",
        Slot::Magic => "Magic",
        Slot::Range => "Range",
    }
}
impl Model {
    fn definition_id(&mut self, id: i32) -> Definition {
        self.defs
            .entry(id)
            .or_insert_with(|| {
                Shared::new(NpcDefinition {
                    id,
                    ..NpcDefinition::default()
                })
            })
            .clone()
    }
    fn emit(&mut self, phase: &str, args: Vec<Value>) -> Result<()> {
        if !self.recording {
            return Ok(());
        }
        let mut row = vec![json!(phase)];
        row.extend(args);
        self.events.push(json!(row));
        let n = self.calls.entry(phase.into()).or_default();
        *n += 1;
        let at = *n;
        for h in self.input["hooks"].as_array().cloned().unwrap_or_default() {
            if h["event"] == phase && v(&h, "at", 1) == at {
                for a in h["actions"].as_array().unwrap() {
                    self.act(a)?;
                }
            }
        }
        if self.input["failures"][phase].as_i64() == Some(i64::from(at)) {
            return Err(Error::Binding(phase.into()));
        }
        Ok(())
    }
    fn make(
        &mut self,
        id: i32,
        l: Option<Location>,
        d: Option<Direction>,
        kind: u8,
    ) -> Result<Node> {
        let n = self.nodes.len();
        let def = self.definition_id(id);
        let size = def.read().size;
        let p = self.props.len();
        let s = self.skills.len();
        self.props.push(Props::default());
        self.skills.push(Skills::default());
        let brain = npc::State::new(l.clone().unwrap_or_else(|| Location::new(0, 0, 0)));
        brain.fields.write().location = l.clone();
        self.nodes.push(Actor {
            id,
            index: 0,
            size,
            loc: l.clone(),
            direction: d,
            active: false,
            def: Some(def),
            props: Some(p),
            skills: Some(s),
            brain,
            children: None,
            kind,
        });
        self.emit(
            "base",
            vec![
                json!(n),
                json!(id),
                coord(&l),
                json!(d.map(|d| Direction::ALL.iter().position(|v| *v == d).unwrap())),
            ],
        )?;
        if kind == 1 {
            self.nodes[n].brain.fields.write().walks = true;
        }
        Ok(n)
    }
    fn set_def(&mut self, a: &Value) {
        let d = self.definition_id(v(a, "id", 0));
        let mut d = d.write();
        if a.get("level").is_some() {
            d.combat_level = v(a, "level", 0);
        }
        if a.get("size").is_some() {
            d.size = v(a, "size", 0);
        }
        if a.get("children").is_some() {
            d.child_npc_ids = a["children"]
                .as_array()
                .map(|a| Shared::new(a.iter().map(|v| v.as_i64().unwrap() as i32).collect()));
        }
        if let Some(config) = a["config"].as_object() {
            let mut h = d.handlers.write();
            h.clear();
            for (k, v) in config {
                let p = if let Some(id) = v["animation"].as_i64() {
                    Property::Animation(Shared::new(Animation::high(id as i32)))
                } else if let Some(a) = v.as_array() {
                    Property::IntArray(Shared::new(
                        a.iter().map(|v| v.as_i64().unwrap() as i32).collect(),
                    ))
                } else {
                    serde_json::from_value(v.clone()).unwrap()
                };
                h.insert(k.clone(), p);
            }
        }
    }
    fn set_node(&mut self, a: &Value) {
        let n = v(a, "node", 0) as usize;
        let definition = a
            .get("definition")
            .map(|v| v.as_i64().map(|id| self.definition_id(id as i32)));
        let actor = &mut self.nodes[n];
        if a.get("location").is_some() {
            actor.loc = loc(&a["location"]);
        }
        if let Some(d) = definition {
            actor.def = d;
        }
        if a.get("index").is_some() {
            actor.index = v(a, "index", 0);
        }
        if a.get("size").is_some() {
            actor.size = v(a, "size", 0);
        }
        if a.get("props").is_some() {
            actor.props = a["props"].as_u64().map(|v| v as usize);
        }
        if a.get("skills").is_some() {
            actor.skills = a["skills"].as_u64().map(|v| v as usize);
        }
        if a.get("aggressive").is_some() {
            actor.brain.fields.write().aggressive = a["aggressive"] == true;
        }
        if a.get("children").is_some() {
            actor.children = a["children"]
                .as_array()
                .map(|a| Shared::new(a.iter().map(|v| v.as_u64().map(|v| v as usize)).collect()));
        }
    }
    fn act(&mut self, a: &Value) -> Result<Value> {
        match a["op"].as_str().unwrap() {
            "config" => npc_init::init_config(self, v(a, "node", 0) as usize)?,
            "init" => npc_init::init(self, v(a, "node", 0) as usize)?,
            "set" => self.set_node(a),
            "def" => self.set_def(a),
            "factory" => {
                let objects = a["args"].as_array().map_or(vec![], |a| {
                    a.iter().map(|v| v.as_i64().unwrap() as i32).collect()
                });
                let d = a["direction"].as_u64().map(|v| Direction::ALL[v as usize]);
                let result =
                    npc_init::create(self, v(a, "id", 0), loc(&a["location"]), d, &objects)?;
                return Ok(json!(result));
            }
            _ => unreachable!(),
        }
        Ok(Value::Null)
    }
    fn state(&self, result: Value, error: Value) -> Value {
        fn tag<T>(v: &Shared<T>, ids: &mut BTreeMap<usize, usize>) -> usize {
            let next = ids.len();
            *ids.entry(v.identity()).or_insert(next)
        }
        let mut refs = BTreeMap::new();
        let nodes: Vec<_> = self
            .nodes
            .iter()
            .map(|n| {
                let b = n.brain.fields.read();
                json!([
                    n.id,
                    n.id,
                    n.index,
                    n.size,
                    coord(&n.loc),
                    n.direction
                        .map(|d| Direction::ALL.iter().position(|v| *v == d).unwrap()),
                    n.active,
                    n.def.as_ref().map(|d| d.read().id),
                    n.props,
                    n.skills,
                    b.aggressive,
                    b.radius,
                    b.force_talk
                        .as_ref()
                        .map(|s| String::from_utf16(s).unwrap()),
                    n.children,
                    n.children.as_ref().map(|c| tag(c, &mut refs))
                ])
            })
            .collect();
        let props: Vec<_> = self
            .props
            .iter()
            .map(|p| {
                json!([
                    p.combat,
                    p.speed,
                    coord(&p.spawn),
                    p.bonuses,
                    anim(&p.animation[0]),
                    anim(&p.animation[1]),
                    anim(&p.animation[2]),
                    anim(&p.animation[3]),
                    anim(&p.animation[4]),
                    graphic(&p.death),
                    p.bonuses.as_ref().map(|c| tag(c, &mut refs))
                ])
            })
            .collect();
        let skills: Vec<_> = self
            .skills
            .iter()
            .map(|s| json!([s.statics, s.currents]))
            .collect();
        let graphics: Vec<_> = self
            .defs
            .iter()
            .map(|(id, d)| {
                let d = d.read();
                let gs: Vec<_> = d
                    .combat_graphics
                    .as_ref()
                    .unwrap()
                    .read()
                    .iter()
                    .map(graphic)
                    .collect();
                json!([id, gs])
            })
            .collect();
        json!({"nodes":nodes,"props":props,"skills":skills,"graphics":graphics,"repository":self.repository,"renderable":self.renderable,"nextIndex":self.next_index,"result":result,"error":error,"events":self.events})
    }
}
impl Context for Model {
    type Arguments = Vec<i32>;
    fn prototype(&mut self, id: i32) -> Result<Option<Node>> {
        self.emit("prototype", vec![json!(id)])?;
        Ok(self.input["prototype"].as_u64().map(|v| v as usize))
    }
    fn construct(
        &mut self,
        p: Node,
        id: i32,
        l: Option<Location>,
        o: &Vec<i32>,
    ) -> Result<Option<Node>> {
        self.emit("construct", vec![json!(p), json!(id), coord(&l), json!(o)])?;
        if self.input.get("constructed").is_some() {
            Ok(self.input["constructed"].as_u64().map(|v| v as usize))
        } else {
            self.make(id, l, Some(Direction::South), 1).map(Some)
        }
    }
    fn base(&mut self, id: i32, l: Option<Location>, d: Option<Direction>) -> Result<Node> {
        self.make(id, l, d, 0)
    }
    fn definition(&self, n: Node) -> Result<Option<Definition>> {
        Ok(self.nodes[n].def.clone())
    }
    fn brain(&self, n: Node) -> Result<npc::State> {
        Ok(self.nodes[n].brain.clone())
    }
    fn location(&self, n: Node) -> Result<Option<Location>> {
        Ok(self.nodes[n].loc.clone())
    }
    fn index(&self, n: Node) -> Result<i32> {
        Ok(self.nodes[n].index)
    }
    fn size(&self, n: Node) -> Result<i32> {
        Ok(self.nodes[n].size)
    }
    fn set_index(&mut self, n: Node, v: i32) -> Result<()> {
        self.nodes[n].index = v;
        Ok(())
    }
    fn set_size(&mut self, n: Node, v: i32) -> Result<()> {
        self.nodes[n].size = v;
        Ok(())
    }
    fn children(&self, n: Node) -> Result<Option<Shared<Vec<Option<Node>>>>> {
        Ok(self.nodes[n].children.clone())
    }
    fn set_children(&mut self, n: Node, c: Shared<Vec<Option<Node>>>) -> Result<()> {
        self.nodes[n].children = Some(c);
        Ok(())
    }
    fn entity_init(&mut self, n: Node) -> Result<()> {
        self.nodes[n].active = true;
        self.emit("entity", vec![json!(n)])
    }
    fn properties(&mut self, n: Node) -> Result<Option<usize>> {
        let result = self.nodes[n].props;
        self.emit("properties", vec![json!(n)])?;
        Ok(result)
    }
    fn skills(&mut self, n: Node) -> Result<Option<usize>> {
        let result = self.nodes[n].skills;
        self.emit("skills", vec![json!(n)])?;
        Ok(result)
    }
    fn set_property(&mut self, p: Option<usize>, v: Change) -> Result<()> {
        let p = p.ok_or(Error::Null)?;
        match v {
            Change::CombatLevel(v) => {
                self.props[p].combat = v;
                self.emit("combat", vec![json!(p), json!(v)])
            }
            Change::AttackSpeed(v) => {
                self.props[p].speed = v;
                self.emit("speed", vec![json!(p), json!(v)])
            }
            Change::Bonuses(a) => {
                let value = json!(a);
                self.props[p].bonuses = Some(a);
                self.emit("bonuses", vec![json!(p), value])
            }
            Change::Animation(s, a) => {
                let value = anim(&a);
                self.props[p].animation[slot(s)] = a;
                self.emit("animation", vec![json!(p), json!(name(s)), value])
            }
            Change::DeathGraphic(g) => {
                self.props[p].death = Some(g);
                Ok(())
            }
        }
    }
    fn get_animation(&mut self, p: Option<usize>, s: Slot) -> Result<Option<Shared<Animation>>> {
        let p = p.ok_or(Error::Null)?;
        let result = self.props[p].animation[slot(s)].clone();
        self.emit("getAnimation", vec![json!(p), json!(name(s))])?;
        Ok(result)
    }
    fn set_skill(&mut self, s: Option<usize>, kind: Skill, id: i32, level: i32) -> Result<()> {
        let s = s.ok_or(Error::Null)?;
        let key = match kind {
            Skill::Static => {
                self.skills[s].statics[id as usize] = level;
                "static"
            }
            Skill::Current => {
                self.skills[s].currents[id as usize] = level;
                "current"
            }
        };
        self.emit(key, vec![json!(s), json!(id), json!(level)])
    }
    fn maximum_life_points(&mut self, s: Option<usize>) -> Result<i32> {
        let s = s.ok_or(Error::Null)?;
        let result = self.skills[s].statics[3];
        self.emit("maximum", vec![json!(s)])?;
        Ok(result)
    }
    fn familiar(&self, n: Node) -> Result<bool> {
        Ok(self.nodes[n].kind == 2)
    }
    fn attribute_hp(&mut self, n: Node, fallback: i32) -> Result<i32> {
        self.emit("hp", vec![json!(n)])?;
        match self.input.get("hp") {
            None => Ok(fallback),
            Some(Value::Null) => Err(Error::Null),
            Some(Value::Number(v)) => Ok(v.as_i64().unwrap() as i32),
            _ => Err(Error::Type),
        }
    }
    fn spawn_location(&mut self, p: Option<usize>, l: Option<Location>) -> Result<()> {
        let p = p.ok_or(Error::Null)?;
        self.props[p].spawn = l.clone();
        self.emit("spawn", vec![json!(p), coord(&l)])
    }
    fn add_repository(&mut self, n: Node) -> Result<()> {
        self.nodes[n].index = self.next_index;
        self.next_index = self.next_index.wrapping_add(1);
        self.repository.push(n);
        self.emit("repository", vec![json!(n)])
    }
    fn region_move(&mut self, n: Node, a: Option<Location>, b: Option<Location>) -> Result<()> {
        self.emit("move", vec![json!(n), coord(&a), coord(&b)])
    }
    fn region_active(&mut self, l: Location) -> Result<bool> {
        self.emit("active", vec![json!(l.coordinates())])?;
        Ok(self.input["active"] != false)
    }
    fn add_renderable(&mut self, n: Node) -> Result<()> {
        self.renderable.push(n);
        self.emit("renderable", vec![json!(n)])
    }
    fn interact_default(&mut self, n: Node) -> Result<()> {
        self.emit("interact", vec![json!(n)])
    }
    fn configure(&mut self, n: Node) -> Result<()> {
        self.emit("configure", vec![json!(n)])
    }
    fn default_behavior(&mut self, n: Node) -> Result<()> {
        self.emit("defaultBehavior", vec![json!(n)])
    }
    fn behavior_created(&mut self, n: Node) -> Result<()> {
        self.emit("created", vec![json!(n)])
    }
}
fn class(e: &Error) -> &'static str {
    match e {
        Error::Null | Error::Definition(rs09_server::definitions::npc::Error::Null) => "Null",
        Error::Index => "Index",
        Error::Type | Error::Definition(rs09_server::definitions::npc::Error::Type) => "Type",
        _ => "Binding",
    }
}
pub fn evaluate(input: &Value) -> Value {
    let mut m = Model {
        input: input.clone(),
        nodes: vec![],
        props: vec![],
        skills: vec![],
        defs: BTreeMap::new(),
        events: vec![],
        calls: BTreeMap::new(),
        repository: vec![],
        renderable: vec![],
        next_index: 1,
        recording: false,
    };
    for d in input["defs"].as_array().cloned().unwrap_or_default() {
        m.set_def(&d);
    }
    for (n, k, x) in [(0, 0, 3207), (1, 1, 3210), (2, 2, 3207)] {
        m.make(
            n,
            Some(Location::new(x, 3216, 0)),
            Some(Direction::South),
            k,
        )
        .unwrap();
    }
    for a in input["nodes"].as_array().cloned().unwrap_or_default() {
        m.set_node(&a);
    }
    m.recording = true;
    let mut out = vec![];
    for a in input["actions"].as_array().unwrap() {
        m.events.clear();
        let (result, error) = match m.act(a) {
            Ok(v) => (v, Value::Null),
            Err(e) => (Value::Null, json!(class(&e))),
        };
        out.push(m.state(result, error));
    }
    json!(out)
}
