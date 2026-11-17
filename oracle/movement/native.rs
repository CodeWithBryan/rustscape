use rs09_game_core::direction::Direction;
use rs09_server::{
    definitions::item::Shared,
    world::{
        doors::Location,
        movement::{
            self, Constructor, Context, DestinationService, Error, Flag, Kind, Movement, Node,
            NodeView, Result, Route, RouteHandle, Target,
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
    view: NodeView,
    current: Option<usize>,
    behavior: bool,
    finder: Option<usize>,
}
struct Model {
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
                return self
                    .names
                    .get(s)
                    .and_then(|n| self.nodes[n].view.location.clone());
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
            let v = &r.view;
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
            .map(|n| n.view.clone())
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
        let dest = match &target {
            Target::Node(n) => json!(self.name(Some(*n))),
            Target::Location(l) => json!(l.coordinates()),
        };
        self.emit(
            "find",
            vec![
                json!(self.name(p.mover())),
                dest,
                json!(true),
                json!(finder),
            ],
        )?;
        let i = self.calls["find"] as usize - 1;
        let row = self.input["paths"].get(i).cloned().unwrap_or(json!({}));
        if row["null"].as_bool().unwrap_or(false) {
            return Ok(None);
        }
        let ps = if let Some(ps) = row["points"].as_array() {
            ps.clone()
        } else {
            let l = match target {
                Target::Node(n) => self.nodes[&n].view.location.clone().ok_or(Error::Null)?,
                Target::Location(l) => l,
            };
            let [x, y, _] = l.coordinates();
            vec![json!([x, y])]
        };
        let path = Shared::new(Route {
            successful: row["successful"].as_bool().unwrap_or(true),
            move_near: row["near"].as_bool().unwrap_or(false),
            points: Shared::new(
                ps.iter()
                    .map(|v| {
                        Shared::new(Point::origin(
                            v[0].as_i64().unwrap() as i32,
                            v[1].as_i64().unwrap() as i32,
                        ))
                    })
                    .collect(),
            ),
        });
        self.paths.push(path.clone());
        Ok(Some(path))
    }
    fn walk_reset(
        &mut self,
        _: Option<&mut Movement>,
        id: Node,
        running: Option<bool>,
    ) -> Result<()> {
        self.emit("reset", vec![json!(self.name(Some(id))), json!(running)])?;
        let r = self.nodes.get_mut(&id).ok_or(Error::Null)?;
        let loc = r.view.location.clone();
        let mut points = r.view.walking.points.write();
        points.clear();
        r.view.walking.has_path = false;
        let [x, y, _] = loc.ok_or(Error::Null)?.coordinates();
        points.push_back(Shared::new(Point::origin(x, y)));
        r.view.walking.has_path = true;
        if let Some(v) = running {
            r.view.walking.running = v;
        }
        Ok(())
    }
    fn walk_add(&mut self, _: &mut Movement, id: Node, x: i32, y: i32) -> Result<()> {
        self.emit("add", vec![json!(self.name(Some(id))), json!(x), json!(y)])?;
        let r = self.nodes.get_mut(&id).unwrap();
        r.view
            .walking
            .points
            .write()
            .push_back(Shared::new(Point::origin(x, y)));
        r.view.walking.has_path = true;
        Ok(())
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
        let (result, error) = match m.act(a) {
            Ok(v) => (v, None),
            Err(e) => (Value::Null, Some(failure(&e))),
        };
        out.push(m.snapshot(result, error));
    }
    json!(out)
}
