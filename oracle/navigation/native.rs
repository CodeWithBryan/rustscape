use rs09_game_core::direction::Direction;
use rs09_server::world::{
    navigation::{self, Navigator},
    pathfinding::{self, dispatch},
    walking::{self, Queue},
};
use rs09_server::{
    definitions::item::Shared,
    world::{
        doors::Location,
        movement::{
            self, Constructor, Context, DestinationService, Error, Flag, Kind, Movement, Node,
            NodeView, Result, RouteHandle, Target,
        },
        walking::{Point, Step},
    },
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, VecDeque};
fn n(a: &Value, k: &str, d: i32) -> i32 {
    a[k].as_i64().map_or(d, |v| v as i32)
}
fn flag(v: &Value) -> Option<Flag> {
    Some(match v.as_str()? {
        "location" => Flag::Location,
        "entity" => Flag::Entity,
        "follow" => Flag::FollowEntity,
        "combat" => Flag::Combat,
        "item" => Flag::Item,
        "object" => Flag::Object,
        "custom" => Flag::Custom(0),
        s => panic!("flag{s}"),
    })
}
fn flag_name(f: Option<Flag>) -> Option<&'static str> {
    f.map(|f| match f {
        Flag::Location => "location",
        Flag::Entity => "entity",
        Flag::FollowEntity => "follow",
        Flag::Combat => "combat",
        Flag::Item => "item",
        Flag::Object => "object",
        Flag::Custom(_) => "custom",
    })
}
fn coord(l: &Option<Location>) -> Value {
    l.as_ref().map_or(Value::Null, |l| json!(l.coordinates()))
}
fn points<'a>(p: impl IntoIterator<Item = &'a Step>) -> Value {
    json!(
        p.into_iter()
            .map(|p| {
                let p = p.read();
                [p.x(), p.y()]
            })
            .collect::<Vec<_>>()
    )
}
fn failure(e: &Error) -> &'static str {
    match e {
        Error::Null => "Null",
        Error::Type => "Type",
        Error::Index => "Index",
        Error::NoSuchElement => "NoSuchElement",
        Error::Limit => "Limit",
        Error::Binding(_) => "Binding",
    }
}
struct Record {
    queue: Option<Queue>,
    view: NodeView,
    current: Option<usize>,
    behavior: bool,
    finder: Option<usize>,
}
struct Model {
    navigator: Option<Navigator>,
    input: Value,
    nodes: BTreeMap<Node, Record>,
    names: BTreeMap<String, Node>,
    locations: BTreeMap<String, Location>,
    current: Option<Movement>,
    pulses: Vec<Movement>,
    paths: Vec<RouteHandle>,
    views: Vec<Shared<VecDeque<Step>>>,
    events: Vec<Value>,
    calls: HashMap<String, i32>,
    next: usize,
}
impl Model {
    fn name(&self, n: Option<Node>) -> Option<String> {
        n.and_then(|n| {
            self.names
                .iter()
                .find(|(_, v)| **v == n)
                .map(|(k, _)| k.clone())
        })
    }
    fn resolve(&self, v: &Value) -> Option<Node> {
        v.as_str().and_then(|s| self.names.get(s)).copied()
    }
    fn loc(&self, v: &Value) -> Option<Location> {
        if let Some(s) = v.as_str() {
            if let Some(s) = s.strip_prefix('@') {
                return self.names.get(s).and_then(|n| self.node(*n).ok()?.location);
            }
            return self.locations.get(s).cloned();
        }
        v.as_array().map(|a| {
            Location::new(
                a[0].as_i64().unwrap() as i32,
                a[1].as_i64().unwrap() as i32,
                a[2].as_i64().unwrap() as i32,
            )
        })
    }
    fn emit(&mut self, event: &str, args: Vec<Value>) -> Result<()> {
        let mut row = vec![json!(event)];
        row.extend(args);
        self.events.push(json!(row));
        let count = self.calls.entry(event.into()).or_default();
        *count += 1;
        let count = *count;
        let hooks = self.input["hooks"].as_array().cloned().unwrap_or_default();
        for hook in hooks {
            if hook["event"] == event && n(&hook, "at", 1) == count {
                for a in hook["actions"].as_array().unwrap() {
                    self.act(a)?;
                }
            }
        }
        Ok(())
    }
    fn p(&self) -> Result<Movement> {
        self.current.clone().ok_or(Error::Null)
    }
    fn set_node(&mut self, id: Node, a: &Value) -> Result<()> {
        let location = a.get("location").map(|v| self.loc(v));
        let victim = a.get("victim").map(|v| self.resolve(v));
        let r = self.nodes.get_mut(&id).ok_or(Error::Null)?;
        if let Some(l) = location {
            r.view.location = l;
        }
        if a.get("size").is_some() {
            r.view.size = n(a, "size", 1);
        }
        if a.get("active").is_some() {
            r.view.active = a["active"].as_bool().unwrap();
        }
        if a.get("direction").is_some() {
            r.view.direction = a["direction"]
                .as_i64()
                .and_then(|n| Direction::from_code(n as i32));
        }
        if r.view.kind.entity() {
            if let Some(p) = a["points"].as_array() {
                let mut out = r.view.walking.points.write();
                out.clear();
                for p in p {
                    out.push_back(Shared::new(Point::origin(
                        p[0].as_i64().unwrap() as i32,
                        p[1].as_i64().unwrap() as i32,
                    )));
                }
                r.view.walking.has_path = !out.is_empty();
            }
            if let Some(v) = a["running"].as_bool() {
                r.view.walking.running = v;
            }
            if let Some(v) = a["moving"].as_bool() {
                r.view.walking.moving = v;
            }
            if let Some(v) = a["combat"].as_bool() {
                r.view.combat_running = v;
            }
            if let Some(v) = victim {
                r.view.victim = v;
            }
            if let Some(v) = a["draw"].as_bool() {
                r.view.draw_intersect = v;
            }
            if r.view.kind == Kind::Npc {
                if let Some(v) = a["never"].as_bool() {
                    r.view.never_walks = v;
                }
                if let Some(v) = a["behavior"].as_bool() {
                    r.behavior = v;
                }
                if a.get("finder").is_some() {
                    r.finder = a["finder"].as_u64().map(|v| v as usize);
                }
            }
        }
        if r.view.kind == Kind::Scenery {
            for (key, field) in [
                ("kind", &mut r.view.scenery_kind),
                ("rotation", &mut r.view.rotation),
                ("walkFlag", &mut r.view.walking_flag),
            ] {
                if a.get(key).is_some() {
                    *field = n(a, key, 0);
                }
            }
            if a.get("sx").is_some() {
                r.view.dimensions[0] = n(a, "sx", 1);
            }
            if a.get("sy").is_some() {
                r.view.dimensions[1] = n(a, "sy", 1);
            }
        }
        if let Some(q) = &r.queue {
            let mut q = q.state.write();
            if a.get("location").is_some() {
                q.entity.location = r.view.location.clone();
            }
            if a.get("direction").is_some() {
                q.entity.direction = r.view.direction;
            }
            if let Some(v) = a["running"].as_bool() {
                q.running = v;
            }
            if let Some(v) = a["energy"].as_f64() {
                q.entity.energy = v;
            }
        }
        Ok(())
    }
    fn act(&mut self, a: &Value) -> Result<Value> {
        match a["op"].as_str().unwrap() {
            "new" => {
                self.next += 1;
                let id = self.next;
                let m = self.resolve(a.get("mover").unwrap_or(&json!("mover")));
                let d = self.resolve(a.get("target").unwrap_or(&json!("target")));
                if m.is_some_and(|m| !self.nodes[&m].view.kind.entity()) {
                    return Err(Error::Type);
                }
                let token = n(a, "token", 0) as usize;
                let constructor = match a["constructor"].as_str().unwrap_or("basic") {
                    "option" => Constructor::Option(Some(token)),
                    "use" => Constructor::UseWith(Some(token)),
                    "flag" => Constructor::Destination {
                        flag: flag(&a["flag"]),
                        override_method: None,
                    },
                    "override" => Constructor::Destination {
                        flag: flag(&a["flag"]),
                        override_method: Some(token),
                    },
                    _ => Constructor::Basic,
                };
                let p = Movement::new(
                    self,
                    id,
                    m,
                    d,
                    a["finder"].as_u64().map(|v| v as usize),
                    a["force"].as_bool().unwrap_or(false),
                    constructor,
                )?;
                self.current = Some(p.clone());
                self.pulses.push(p);
            }
            "walkPlan" => {
                let id = self
                    .resolve(a.get("node").unwrap_or(&json!("mover")))
                    .ok_or(Error::Null)?;
                let q = self.nodes[&id].queue.clone().ok_or(Error::Null)?;
                navigation::reset(
                    &mut Walking(self),
                    &q,
                    Some(a["running"].as_bool().unwrap_or(false)),
                )?;
                navigation::add(&mut Walking(self), &q, n(a, "x", 0), n(a, "y", 0))?;
            }
            "walk" => {
                let id = self
                    .resolve(a.get("node").unwrap_or(&json!("mover")))
                    .ok_or(Error::Null)?;
                let mut queue = self.nodes[&id].queue.clone().ok_or(Error::Null)?;
                queue
                    .update(&mut Walking(self))
                    .map_err(navigation::walking_error)?;
            }
            "update" => return self.p()?.update(self).map(|v| json!(v)),
            "path" => self.p()?.update_path(self)?,
            "stop" => self.p()?.stop(self)?,
            "start" => self.p()?.base().write().running = true,
            "node" => {
                let id = self
                    .resolve(a.get("node").unwrap_or(&json!("mover")))
                    .ok_or(Error::Null)?;
                self.set_node(id, a)?;
            }
            "set" => {
                let p = self.p()?;
                let mut s = p.state.write();
                for (key, field) in [("forceRun", &mut s.force_run)] {
                    if let Some(v) = a[key].as_bool() {
                        *field = v;
                    }
                }
                if let Some(v) = a["near"].as_bool() {
                    s.near = v;
                }
                if let Some(v) = a["usingTruncatedPath"].as_bool() {
                    s.truncated = v;
                }
                if let Some(v) = a["isMoveNearSet"].as_bool() {
                    s.move_near_set = v;
                }
                if let Some(v) = a["running"].as_bool() {
                    s.base.write().running = v;
                }
                if a.get("target").is_some() {
                    s.destination = self.resolve(&a["target"]);
                }
                if a.get("mover").is_some() {
                    s.mover = self.resolve(&a["mover"]);
                }
                if a.get("last").is_some() {
                    s.last = self.loc(&a["last"]);
                }
                if a.get("interactLocation").is_some() {
                    s.interact = self.loc(&a["interactLocation"]);
                }
                if a.get("previousLoc").is_some() {
                    s.previous = self.loc(&a["previousLoc"]);
                }
                if a.get("flag").is_some() {
                    s.flag = flag(&a["flag"]);
                }
            }
            "checks" => {
                let checks = a["values"]
                    .as_array()
                    .map(|v| Shared::new(v.iter().map(|v| self.resolve(v)).collect()));
                self.p()?.base().write().checks = checks;
            }
            "pointClear" => {
                let id = self
                    .resolve(a.get("node").unwrap_or(&json!("mover")))
                    .ok_or(Error::Null)?;
                let r = self.nodes.get_mut(&id).unwrap();
                r.view.walking.points.write().clear();
                r.view.walking.has_path = false;
            }
            "pathClear" => {
                let i = n(a, "index", self.paths.len() as i32 - 1) as usize;
                self.paths
                    .get(i)
                    .ok_or(Error::Index)?
                    .read()
                    .points
                    .write()
                    .clear();
            }
            "pathReplace" => {
                let i = n(a, "index", self.paths.len() as i32 - 1) as usize;
                self.paths.get(i).ok_or(Error::Index)?.write().points =
                    Shared::new(VecDeque::new());
            }
            "pathView" => {
                let i = n(a, "index", self.paths.len() as i32 - 1) as usize;
                self.views
                    .push(self.paths.get(i).ok_or(Error::Index)?.read().points.clone());
            }
            "mutateLocation" => self.loc(&a["which"]).ok_or(Error::Null)?.set(
                n(a, "x", 0),
                n(a, "y", 0),
                n(a, "z", 0),
            ),
            "global" => {
                for (k, v) in a.as_object().unwrap() {
                    if k != "op" {
                        self.input[k] = v.clone();
                    }
                }
            }
            "flagDestination" | "closest" => {
                let m = self
                    .resolve(a.get("mover").unwrap_or(&json!("mover")))
                    .ok_or(Error::Null)?;
                let d = self
                    .resolve(a.get("target").unwrap_or(&json!("target")))
                    .ok_or(Error::Null)?;
                let f = flag(&a["flag"]).ok_or(Error::Null)?;
                if a["op"] == "closest" {
                    return f
                        .closest(
                            self,
                            Some(m),
                            Some(d),
                            self.loc(&a["suggestion"]).ok_or(Error::Null)?,
                        )
                        .map(|v| coord(&Some(v)));
                }
                return f.destination(self, Some(m), Some(d)).map(|v| coord(&v));
            }
            "traversal" => {
                return flag(&a["flag"])
                    .ok_or(Error::Null)?
                    .traversal(
                        self,
                        &self.loc(&a["location"]).ok_or(Error::Null)?,
                        Direction::from_code(n(a, "direction", 0)).ok_or(Error::Index)?,
                    )
                    .map(|v| json!(v));
            }
            "truncate" => {
                let m = self
                    .resolve(a.get("mover").unwrap_or(&json!("mover")))
                    .ok_or(Error::Null)?;
                return movement::truncate(self, m, &self.loc(&a["location"]).ok_or(Error::Null)?)
                    .map(|(b, l)| json!([b, l.coordinates()]));
            }
            "fail" => return Err(Error::Binding("oracle".into())),
            s => panic!("op{s}"),
        }
        Ok(Value::Null)
    }
    fn snapshot(&self, result: Value, error: Option<&str>) -> Value {
        let mut ns = BTreeMap::new();
        let mut refs = vec![];
        for (name, id) in &self.names {
            let r = &self.nodes[id];
            let v = &self.node(*id).unwrap();
            refs.push(v.location.clone());
            let mut n = json!({"location":coord(&v.location),"size":v.size,"active":v.active,"direction":v.direction.map(Direction::code)});
            if v.kind.entity() {
                let o = n.as_object_mut().unwrap();
                o.insert("points".into(), points(v.walking.points.read().iter()));
                o.insert("running".into(), json!(v.walking.running));
                o.insert("moving".into(), json!(v.walking.moving));
                o.insert("victim".into(), json!(self.name(v.victim)));
                o.insert("combat".into(), json!(v.combat_running));
                o.insert("draw".into(), json!(v.draw_intersect));
                o.insert("walk".into(), json!(r.queue.as_ref().unwrap().walk_dir()));
                o.insert("run".into(), json!(r.queue.as_ref().unwrap().run_dir()));
                o.insert(
                    "energy".into(),
                    json!(bits(r.queue.as_ref().unwrap().entity().energy)),
                );
                o.insert(
                    "foot".into(),
                    coord(&r.queue.as_ref().unwrap().foot_print()),
                );
                o.insert("current".into(), json!(r.current));
            }
            ns.insert(name.clone(), n);
        }
        let mut ps = vec![];
        for p in &self.pulses {
            let s = p.state.read();
            refs.extend([s.last.clone(), s.interact.clone(), s.previous.clone()]);
            let checks = s
                .base
                .read()
                .checks
                .clone()
                .map(|v| v.read().iter().map(|v| self.name(*v)).collect::<Vec<_>>());
            ps.push(json!({"running":s.base.read().running,"near":s.near,"forceRun":s.force_run,"usingTruncatedPath":s.truncated,"isMoveNearSet":s.move_near_set,"id":s.id,"mover":self.name(s.mover),"target":self.name(s.destination),"finder":s.finder,"flag":flag_name(s.flag),"last":coord(&s.last),"interactLocation":coord(&s.interact),"previousLoc":coord(&s.previous),"checks":checks}));
        }
        let paths: Vec<_> = self
            .paths
            .iter()
            .map(|p| {
                let p = p.read();
                json!([p.successful, p.move_near, points(p.points.read().iter())])
            })
            .collect();
        let views: Vec<_> = self.views.iter().map(|v| points(v.read().iter())).collect();
        refs.extend(self.locations.values().cloned().map(Some));
        let aliases: Vec<Vec<_>> = refs
            .iter()
            .map(|l| {
                refs.iter()
                    .map(|r| l.as_ref().zip(r.as_ref()).is_some_and(|(l, r)| l.same(r)))
                    .collect()
            })
            .collect();
        json!({"nodes":ns,"pulses":ps,"current":self.current.as_ref().map(Movement::id),"paths":paths,"views":views,"aliases":aliases,"locations":refs.iter().map(coord).collect::<Vec<_>>(),"events":self.events,"result":result,"error":error})
    }
}
impl Context for Model {
    fn clipping(&mut self, z: i32, x: i32, y: i32) -> Result<i32> {
        self.emit("clip", vec![json!(z), json!(x), json!(y)])?;
        let mut f = n(&self.input, "clip", 0);
        for row in self.input["flags"].as_array().into_iter().flatten() {
            let row = row.as_array().unwrap();
            if row[0].as_i64().unwrap() as i32 == x
                && row[1].as_i64().unwrap() as i32 == y
                && (row.len() < 4 || row[3].as_i64().unwrap() as i32 == z)
            {
                f = row[2].as_i64().unwrap() as i32;
            }
        }
        Ok(f)
    }
    fn world_clipping(&mut self, z: i32, x: i32, y: i32) -> Result<i32> {
        self.clipping(z, x, y)
    }
    fn node(&self, id: Node) -> Result<NodeView> {
        self.nodes
            .get(&id)
            .map(|n| {
                n.queue
                    .as_ref()
                    .map_or_else(|| n.view.clone(), |q| navigation::view(n.view.clone(), q))
            })
            .ok_or(Error::Null)
    }
    fn region(&mut self, l: &Location) -> Result<Option<bool>> {
        self.emit("region", vec![json!(l.coordinates())])?;
        let i = self.calls["region"] as usize - 1;
        let v = self.input["regionAnswers"]
            .get(i)
            .or_else(|| self.input.get("region"));
        Ok(v.map_or(Some(true), Value::as_bool))
    }
    fn npc_finder(&mut self, id: Node) -> Result<Option<usize>> {
        if !self.nodes[&id].behavior {
            return Ok(None);
        }
        self.emit("npcFinder", vec![json!(self.name(Some(id)))])?;
        Ok(self.nodes[&id].finder)
    }
    fn current_movement(&self, id: Node) -> Result<Option<usize>> {
        Ok(self.nodes.get(&id).ok_or(Error::Null)?.current)
    }
    fn stop_previous(&mut self, id: usize) -> Result<()> {
        self.pulses
            .iter()
            .find(|p| p.id() == id)
            .cloned()
            .ok_or(Error::Null)?
            .stop(self)
    }
    fn assign_current(&mut self, id: Node, pulse: usize) -> Result<()> {
        self.nodes.get_mut(&id).ok_or(Error::Null)?.current = Some(pulse);
        Ok(())
    }
    fn active(&mut self, _: &mut Movement, id: Node) -> Result<bool> {
        self.emit("active", vec![json!(self.name(Some(id)))])?;
        Ok(self.nodes[&id].view.active)
    }
    fn scripts(&mut self, _: &mut Movement, id: Node, weak: bool) -> Result<()> {
        self.emit(
            if weak { "weak" } else { "normal" },
            vec![json!(self.name(Some(id)))],
        )
    }
    fn face(&mut self, _: &mut Movement, id: Node, target: Option<Node>) -> Result<()> {
        self.emit(
            "face",
            vec![json!(self.name(Some(id))), json!(self.name(target))],
        )
    }
    fn pulse(&mut self, p: &mut Movement) -> Result<bool> {
        self.emit("pulse", vec![json!(p.id())])?;
        let i = self.calls["pulse"] as usize - 1;
        Ok(self.input["pulseAnswers"]
            .get(i)
            .and_then(Value::as_bool)
            .unwrap_or_else(|| self.input["pulseResult"].as_bool().unwrap_or(true)))
    }
    fn destination(
        &mut self,
        mover: Option<Node>,
        target: Option<Node>,
        service: DestinationService,
    ) -> Result<Option<Location>> {
        let (service, token) = match service {
            DestinationService::Option(t) => ("option", t),
            DestinationService::UseWith(t) => ("use", t),
            DestinationService::Override(t) => ("override", t),
            DestinationService::Custom(t) => ("custom", t),
        };
        self.emit(
            "destination",
            vec![
                json!(service),
                json!(token),
                json!(self.name(mover)),
                json!(self.name(target)),
            ],
        )?;
        Ok(self.loc(&self.input[format!("{service}Location")]))
    }
    fn find(
        &mut self,
        p: &mut Movement,
        target: Target,
        finder: usize,
    ) -> Result<Option<RouteHandle>> {
        let mut nav = self
            .navigator
            .take()
            .ok_or(Error::Binding("reentrant oracle search".into()))?;
        let result = nav.movement(&mut Routing(self), p, target, finder);
        self.navigator = Some(nav);
        result
    }
    fn walk_reset(
        &mut self,
        _: Option<&mut Movement>,
        id: Node,
        running: Option<bool>,
    ) -> Result<()> {
        let q = self.nodes[&id].queue.clone().ok_or(Error::Null)?;
        navigation::reset(&mut Walking(self), &q, running)
    }
    fn walk_add(&mut self, _: &mut Movement, id: Node, x: i32, y: i32) -> Result<()> {
        let q = self.nodes[&id].queue.clone().ok_or(Error::Null)?;
        navigation::add(&mut Walking(self), &q, x, y)
    }
    fn message(&mut self, _: &mut Movement, id: Node, text: &str) -> Result<()> {
        self.emit("message", vec![json!(self.name(Some(id))), json!(text)])
    }
    fn clear_minimap(&mut self, _: &mut Movement, id: Node) -> Result<()> {
        self.emit("minimap", vec![json!(self.name(Some(id)))])
    }
    fn exception(&mut self, _: &mut Movement, error: &Error) -> Result<()> {
        self.emit("exception", vec![json!(failure(error))])
    }
    fn hint_clear(&mut self, _: &mut Movement, id: Node) -> Result<()> {
        self.emit("hintClear", vec![json!(self.name(Some(id)))])
    }
    fn hint_add(
        &mut self,
        _: &mut Movement,
        id: Node,
        l: Option<Location>,
        kind: i32,
    ) -> Result<()> {
        self.emit(
            "hintAdd",
            vec![json!(self.name(Some(id))), coord(&l), json!(kind)],
        )
    }
    fn teleport_permitted(&mut self, l: &Location) -> Result<bool> {
        self.emit("permitted", vec![json!(l.coordinates())])?;
        Ok(self.input["permitted"].as_bool().unwrap_or(true)
            && !self.input["denied"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|v| *v == json!(l.coordinates())))
    }
    fn door_destination(&mut self, m: Option<Node>, s: Option<Node>) -> Result<Option<Location>> {
        self.emit(
            "doorDestination",
            vec![json!(self.name(m)), json!(self.name(s))],
        )?;
        Ok(if let Some(v) = self.input.get("doorLocation") {
            self.loc(v)
        } else {
            self.nodes[&s.ok_or(Error::Null)?].view.location.clone()
        })
    }
    fn max_distance(&self) -> i32 {
        n(&self.input, "maxDistance", 25)
    }
}
pub fn evaluate(input: &Value) -> Value {
    let mut m = Model {
        navigator: Some(Navigator::default()),
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
    for (k, v) in input["locations"].as_object().into_iter().flatten() {
        m.locations.insert(k.clone(), m.loc(v).unwrap());
    }
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
                queue: if kind.entity() {
                    let q = Queue::new(walking::Entity::new(kind == Kind::Player, None));
                    q.state.write().entity.direction = Some(Direction::South);
                    Some(q)
                } else {
                    None
                },
                view: NodeView::new(kind, Some(Location::new(3207, 3216, 0))),
                current: None,
                behavior: true,
                finder: None,
            },
        );
    }
    m.navigator.as_mut().unwrap().bfs = input["bfs"].as_bool().unwrap_or(false);
    m.navigator.as_mut().unwrap().max_distance = n(input, "maxDistance", 25);
    for r in m.nodes.values_mut() {
        if let Some(q) = &r.queue {
            r.view.walking.points = q.points();
        }
    }
    for (name, v) in input["nodes"].as_object().unwrap() {
        let id = m.names[name];
        m.set_node(id, v).unwrap();
    }
    let mut out = vec![];
    for a in input["actions"].as_array().unwrap() {
        m.events.clear();
        let (result, error) = match m.act(a) {
            Ok(v) => (v, None),
            Err(e) => (Value::Null, Some(failure(&e))),
        };
        out.push(m.snapshot(result, error));
    }
    json!(out)
}

fn bits(v: f64) -> String {
    format!(
        "{:x}",
        if v.is_nan() {
            f64::NAN.to_bits()
        } else {
            v.to_bits()
        }
    )
}
fn path_error(e: Error) -> pathfinding::Error {
    match e {
        Error::Null => pathfinding::Error::Null,
        Error::Index => pathfinding::Error::Index,
        Error::Limit => pathfinding::Error::Limit,
        Error::Binding(s) => pathfinding::Error::Binding(s),
        other => pathfinding::Error::Binding(format!("{other:?}")),
    }
}
struct Routing<'a>(&'a mut Model);
impl navigation::Context for Routing<'_> {
    fn node(&self, id: usize) -> pathfinding::Result<dispatch::View> {
        let n = self.0.node(id).map_err(path_error)?;
        Ok(dispatch::View {
            kind: match n.kind {
                Kind::Player | Kind::Entity => dispatch::Kind::Entity,
                Kind::Npc => dispatch::Kind::Npc,
                Kind::Scenery => dispatch::Kind::Scenery,
                _ => dispatch::Kind::Other,
            },
            location: n.location,
            size: n.size,
            scenery: if n.kind == Kind::Scenery {
                Some(dispatch::Scenery {
                    kind: n.scenery_kind,
                    rotation: n.rotation,
                    definition: Some(dispatch::Definition {
                        size: n.dimensions,
                        walking_flag: n.walking_flag,
                    }),
                })
            } else {
                None
            },
        })
    }
    fn npc_supplier(&mut self, id: usize) -> pathfinding::Result<Option<dispatch::Supplier>> {
        if !self.0.nodes[&id].behavior {
            Err(pathfinding::Error::Null)
        } else {
            Ok(None)
        }
    }
    fn teleport_permitted(&mut self, l: Option<Location>) -> pathfinding::Result<bool> {
        self.0
            .teleport_permitted(&l.ok_or(pathfinding::Error::Null)?)
            .map_err(path_error)
    }
    fn clipping(
        &mut self,
        _: dispatch::Supplier,
        z: i32,
        x: i32,
        y: i32,
    ) -> pathfinding::Result<i32> {
        self.0.clipping(z, x, y).map_err(path_error)
    }
    fn world_clipping(&mut self, z: i32, x: i32, y: i32) -> pathfinding::Result<i32> {
        self.0.world_clipping(z, x, y).map_err(path_error)
    }
}
struct Walking<'a>(&'a mut Model);
fn walk_error(e: Error) -> walking::Error {
    match e {
        Error::Null => walking::Error::Null,
        Error::Binding(s) => walking::Error::Binding(s),
        e => walking::Error::Binding(format!("{e:?}")),
    }
}
impl walking::Context for Walking<'_> {
    fn timer(&mut self, _: &mut Queue, name: &str) -> walking::Result<bool> {
        self.0
            .emit("timer", vec![json!(name)])
            .map_err(walk_error)?;
        Ok(self.0.input[name].as_bool().unwrap_or(false))
    }
    fn marathon(&mut self, _: &mut Queue) -> walking::Result<bool> {
        self.0.emit("marathon", vec![]).map_err(walk_error)?;
        Ok(self.0.input["marathon"].as_bool().unwrap_or(false))
    }
    fn energy(&mut self, _: &mut Queue, drain: f64) -> walking::Result<()> {
        self.0
            .emit("energy", vec![json!(bits(drain))])
            .map_err(walk_error)
    }
    fn run_toggled(&mut self, _: &mut Queue, value: bool) -> walking::Result<()> {
        self.0
            .emit("toggle", vec![json!(value)])
            .map_err(walk_error)
    }
    fn zone_move(&mut self, _: &mut Queue, from: Location, to: Location) -> walking::Result<bool> {
        self.0
            .emit("zone", vec![coord(&Some(from)), coord(&Some(to))])
            .map_err(walk_error)?;
        let i = self.0.calls["zone"] as usize - 1;
        Ok(self.0.input["zoneAnswers"]
            .get(i)
            .and_then(Value::as_bool)
            .unwrap_or(true))
    }
    fn region_move(
        &mut self,
        _: &mut Queue,
        from: Option<Location>,
        to: Option<Location>,
    ) -> walking::Result<()> {
        self.0
            .emit("move", vec![coord(&from), coord(&to)])
            .map_err(walk_error)
    }
    fn clear_pulses(&mut self, _: &mut Queue) -> walking::Result<()> {
        self.0.emit("clearPulses", vec![]).map_err(walk_error)
    }
    fn route_create(&mut self, _: &mut Queue, _: Location) -> walking::Result<walking::RouteItem> {
        Err(walking::Error::Binding(
            "unexpected route-debug create".into(),
        ))
    }
    fn route_add(&mut self, _: &mut Queue, _: walking::RouteItem) -> walking::Result<()> {
        Err(walking::Error::Binding("unexpected route-debug add".into()))
    }
    fn route_remove(&mut self, _: &mut Queue, _: walking::RouteItem) -> walking::Result<()> {
        Err(walking::Error::Binding(
            "unexpected route-debug remove".into(),
        ))
    }
    fn null_location(&mut self, _: &mut Queue) -> walking::Result<()> {
        Ok(())
    }
}
