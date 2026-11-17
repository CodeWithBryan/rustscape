use rs09_game_core::direction::Direction;
use rs09_server::{
    definitions::item::Shared,
    world::{
        doors::Location,
        walking::{Context, Entity, Error, Point, Queue, Result, RouteItem, RouteItems, Step},
    },
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
fn n(a: &Value, k: &str, d: i32) -> i32 {
    a[k].as_i64().map_or(d, |n| n as i32)
}
fn b(a: &Value, k: &str) -> bool {
    a[k].as_bool().unwrap_or(false)
}
fn double(v: &Value) -> f64 {
    v.as_f64()
        .unwrap_or_else(|| v.as_str().unwrap().parse().unwrap())
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
fn coord(l: &Option<Location>) -> Value {
    l.as_ref().map_or(Value::Null, |l| json!(l.coordinates()))
}
fn ordinal(d: Direction) -> usize {
    Direction::ALL.iter().position(|v| *v == d).unwrap()
}
fn point(p: &Step) -> Value {
    let p = p.read();
    json!([
        p.x(),
        p.y(),
        p.direction().map(ordinal),
        p.diff_x(),
        p.diff_y(),
        p.run_disabled
    ])
}
struct Model {
    input: Value,
    events: Vec<Value>,
    calls: HashMap<String, i32>,
    locations: BTreeMap<String, Location>,
    views: Vec<Step>,
    next_item: usize,
}
impl Model {
    fn loc(&self, v: &Value) -> Option<Location> {
        if let Some(name) = v.as_str() {
            return self.locations.get(name).cloned();
        }
        v.as_array().map(|a| {
            Location::new(
                a[0].as_i64().unwrap() as i32,
                a[1].as_i64().unwrap() as i32,
                a[2].as_i64().unwrap() as i32,
            )
        })
    }
    fn emit(&mut self, q: &mut Queue, event: &str, args: Vec<Value>) -> Result<()> {
        let mut row = vec![json!(event)];
        row.extend(args);
        self.events.push(json!(row));
        let count = self.calls.entry(event.into()).or_default();
        *count += 1;
        let count = *count;
        for hook in self.input["hooks"]
            .as_array()
            .into_iter()
            .flatten()
            .cloned()
            .collect::<Vec<_>>()
        {
            if hook["event"] == event && n(&hook, "at", 1) == count {
                for action in hook["actions"].as_array().unwrap() {
                    self.act(q, action)?;
                }
            }
        }
        Ok(())
    }
    fn set(&self, q: &mut Queue, a: &Value) {
        let mut guard = q.state.write();
        let state = &mut *guard;
        if a.get("location").is_some() {
            state.entity.location = self.loc(&a["location"]);
        }
        if a.get("teleport").is_some() {
            state.entity.teleport_location = self.loc(&a["teleport"]);
        }
        if a.get("last").is_some() {
            state.entity.last_scene = self.loc(&a["last"]);
        }
        if a.get("foot").is_some() {
            state.foot_print = self.loc(&a["foot"]);
        }
        for (key, field) in [
            ("scene", &mut state.entity.update_scene),
            ("teleporting", &mut state.entity.teleporting),
            ("running", &mut state.running),
            ("disabled", &mut state.run_disabled),
            ("toggled", &mut state.entity.run_toggled),
            ("draw", &mut state.entity.draw_path),
        ] {
            if let Some(v) = a[key].as_bool() {
                *field = v;
            }
        }
        if let Some(v) = a.get("energy") {
            state.entity.energy = double(v);
        }
        if let Some(v) = a.get("weight") {
            state.entity.weight = double(v);
        }
        if a.get("agility").is_some() {
            state.entity.agility = n(a, "agility", 1);
        }
        if a.get("incrementer").is_some() {
            state.entity.run_incrementer = n(a, "incrementer", 0);
        }
    }
    fn act(&mut self, q: &mut Queue, a: &Value) -> Result<Value> {
        match a["op"].as_str().unwrap() {
            "set" => self.set(q, a),
            "reset" => q.reset(self, a["running"].as_bool())?,
            "add" => q.add_path(self, n(a, "x", 0), n(a, "y", 0), a["disabled"].as_bool())?,
            "point" => q.add_point(n(a, "x", 0), n(a, "y", 0), b(a, "disabled")),
            "seed" => q
                .points()
                .write()
                .push_back(Shared::new(Point::origin(n(a, "x", 0), n(a, "y", 0)))),
            "retain" => self
                .views
                .push(q.points().read()[n(a, "index", 0) as usize].clone()),
            "mutatePoint" => {
                self.views[n(a, "index", 0) as usize].write().run_disabled = b(a, "disabled")
            }
            "clear" => q.points().write().clear(),
            "update" => q.update(self)?,
            "teleport" => return q.update_teleport(self).map(|v| json!(v)),
            "region" => {
                return q
                    .update_region(self, self.loc(&a["from"]), self.loc(&a["to"]), b(a, "move"))
                    .map(|v| json!(v));
            }
            "energy" => q.update_run_energy(self, b(a, "decrease"))?,
            "back" => q.walk_back(self)?,
            "routeNull" => q.route_items().write().push(None),
            "routeClear" => q.route_items().write().clear(),
            "routeReplace" => q.state.write().route_items = Shared::new(RouteItems::default()),
            "mutateLocation" => {
                self.loc(&a["which"])
                    .unwrap()
                    .set(n(a, "x", 0), n(a, "y", 0), n(a, "z", 0))
            }
            "fail" => return Err(Error::Binding("oracle".into())),
            op => panic!("unknown {op}"),
        }
        Ok(Value::Null)
    }
    fn snapshot(&self, q: &Queue, result: Value, error: Option<&str>) -> Value {
        let state = q.state.read().clone();
        let mut ls = vec![
            state.entity.location.clone(),
            state.foot_print.clone(),
            state.entity.last_scene.clone(),
            state.entity.teleport_location.clone(),
        ];
        ls.extend(self.locations.values().cloned().map(Some));
        let aliases: Vec<Vec<bool>> = ls
            .iter()
            .map(|l| {
                ls.iter()
                    .map(|r| l.as_ref().zip(r.as_ref()).is_some_and(|(l, r)| l.same(r)))
                    .collect()
            })
            .collect();
        let routes: Vec<Value> = q
            .route_items()
            .read()
            .values()
            .iter()
            .map(|i| {
                i.as_ref()
                    .map_or(Value::Null, |i| json!([i.token, i.location.coordinates()]))
            })
            .collect();
        json!({"points":state.points.read().iter().map(point).collect::<Vec<_>>(),"views":self.views.iter().map(point).collect::<Vec<_>>(),"routes":routes,
            "location":coord(&state.entity.location),"foot":coord(&state.foot_print),"last":coord(&state.entity.last_scene),"teleport":coord(&state.entity.teleport_location),
            "scene":state.entity.update_scene,"teleporting":state.entity.teleporting,"direction":state.entity.direction.map(ordinal),"walk":state.walk_dir,"run":state.run_dir,
            "running":state.running,"disabled":state.run_disabled,"runningBoth":q.is_running_both(),"moving":q.is_moving(),"hasPath":q.has_path(),
            "energy":bits(state.entity.energy),"weight":bits(state.entity.weight),"toggled":state.entity.run_toggled,"agility":state.entity.agility,"incrementer":state.entity.run_incrementer,"draw":state.entity.draw_path,
            "locations":ls.iter().map(coord).collect::<Vec<_>>(),"aliases":aliases,"events":self.events,"result":result,"error":error})
    }
}
impl Context for Model {
    fn timer(&mut self, q: &mut Queue, name: &str) -> Result<bool> {
        self.emit(q, "timer", vec![json!(name)])?;
        Ok(b(&self.input, name))
    }
    fn marathon(&mut self, q: &mut Queue) -> Result<bool> {
        self.emit(q, "marathon", vec![])?;
        Ok(b(&self.input, "marathon"))
    }
    fn energy(&mut self, q: &mut Queue, drain: f64) -> Result<()> {
        self.emit(q, "energy", vec![json!(bits(drain))])
    }
    fn run_toggled(&mut self, q: &mut Queue, value: bool) -> Result<()> {
        self.emit(q, "toggle", vec![json!(value)])
    }
    fn zone_move(&mut self, q: &mut Queue, from: Location, to: Location) -> Result<bool> {
        self.emit(
            q,
            "zone",
            vec![json!(from.coordinates()), json!(to.coordinates())],
        )?;
        let i = self.calls["zone"] as usize - 1;
        Ok(self.input["zoneAnswers"]
            .get(i)
            .and_then(Value::as_bool)
            .unwrap_or(true))
    }
    fn region_move(
        &mut self,
        q: &mut Queue,
        from: Option<Location>,
        to: Option<Location>,
    ) -> Result<()> {
        self.emit(q, "region", vec![coord(&from), coord(&to)])
    }
    fn clear_pulses(&mut self, q: &mut Queue) -> Result<()> {
        self.emit(q, "clearPulses", vec![])
    }
    fn route_create(&mut self, q: &mut Queue, location: Location) -> Result<RouteItem> {
        let item = RouteItem {
            token: self.next_item,
            location,
        };
        self.next_item += 1;
        self.emit(
            q,
            "create",
            vec![json!(item.token), json!(item.location.coordinates())],
        )?;
        Ok(item)
    }
    fn route_add(&mut self, q: &mut Queue, item: RouteItem) -> Result<()> {
        self.emit(
            q,
            "routeAdd",
            vec![json!(item.token), json!(item.location.coordinates())],
        )
    }
    fn route_remove(&mut self, q: &mut Queue, item: RouteItem) -> Result<()> {
        self.emit(
            q,
            "routeRemove",
            vec![json!(item.token), json!(item.location.coordinates())],
        )
    }
    fn null_location(&mut self, q: &mut Queue) -> Result<()> {
        self.emit(q, "nullLocation", vec![])
    }
}
pub fn evaluate(input: &Value) -> Value {
    let mut model = Model {
        input: input.clone(),
        events: vec![],
        calls: HashMap::new(),
        locations: BTreeMap::new(),
        views: vec![],
        next_item: 1,
    };
    for (name, value) in input["locations"].as_object().into_iter().flatten() {
        model
            .locations
            .insert(name.clone(), model.loc(value).unwrap());
    }
    let location = model.loc(input.get("location").unwrap_or(&json!([3207, 3216, 0])));
    let mut queue = Queue::new(Entity::new(
        input["player"].as_bool().unwrap_or(true),
        location,
    ));
    model.set(&mut queue, &input["fields"]);
    let mut out = vec![];
    for action in input["actions"].as_array().unwrap() {
        model.events.clear();
        let (result, error) = match model.act(&mut queue, action) {
            Ok(v) => (v, None),
            Err(Error::Null) => (Value::Null, Some("Null")),
            Err(Error::Type) => (Value::Null, Some("Type")),
            Err(Error::ConcurrentModification) => (Value::Null, Some("ConcurrentModification")),
            Err(Error::Binding(_)) => (Value::Null, Some("Binding")),
            Err(Error::Limit) => (Value::Null, Some("Limit")),
        };
        out.push(model.snapshot(&queue, result, error));
    }
    json!(out)
}
