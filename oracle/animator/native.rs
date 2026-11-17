use rs09_protocol::buffer::Reader;
use rs09_server::{
    definitions::{animation as defs, item::Shared},
    world::{
        animation::*,
        doors::{self, Location},
    },
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
fn num(v: &Value, key: &str) -> i32 {
    v[key].as_i64().unwrap_or(0) as i32
}
fn bytes(v: &Value) -> Option<Vec<u8>> {
    v.as_str().map(|s| {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    })
}
fn priority(v: &Value) -> Option<Priority> {
    v.as_i64().and_then(|n| Priority::from_ordinal(n as i32))
}
fn ints(v: &Value) -> Option<Shared<Vec<i32>>> {
    v.as_array()
        .map(|a| Shared::new(a.iter().map(|n| n.as_i64().unwrap() as i32).collect()))
}
pub fn definition(d: &defs::Definition) -> Value {
    json!({"anInt2136":d.an_int2136,"anInt2137":d.an_int2137,"anIntArray2139":d.frames,"anInt2140":d.an_int2140,
    "aBoolean2141":d.a_boolean2141,"anInt2142":d.an_int2142,"emoteItem":d.emote_item,"anInt2144":d.an_int2144,
    "handledSounds":d.handled_sounds,"aBooleanArray2149":d.flow,"anIntArray2151":d.secondary_frames,
    "aBoolean2152":d.a_boolean2152,"durations":d.durations,"anInt2155":d.an_int2155,"aBoolean2158":d.a_boolean2158,
    "aBoolean2159":d.a_boolean2159,"anInt2162":d.an_int2162,"anInt2163":d.an_int2163,"newHeader":d.new_header,
    "soundMinDelay":d.sound_min_delay,"soundMaxDelay":d.sound_max_delay,"anIntArray1362":d.an_int_array1362,
    "effect2Sound":d.effect2_sound,"milliseconds":d.duration_ms(),"cycles":d.cycles(),"ticks":d.duration_ticks()})
}
struct Harness {
    input: Value,
    animator: Animator<usize>,
    animations: Vec<Animation>,
    graphics: Vec<Graphics>,
    definitions: Vec<defs::Handle>,
    registry: defs::Registry,
    files: HashMap<i32, Option<Vec<u8>>>,
    locations: [Location; 2],
    objects: [doors::Handle; 2],
    clocks: [Option<Shared<Vec<i32>>>; 2],
    masks: [i32; 2],
    tick: i32,
    selected: i32,
    position: usize,
    accepted: bool,
    events: Vec<Value>,
    calls: HashMap<String, i32>,
    registrations: Vec<Value>,
    queued: Vec<Value>,
}
impl Harness {
    fn new(input: &Value) -> Self {
        let factory = Factory::default();
        let locations = [Location::new(3207, 3216, 0), Location::new(3208, 3216, 0)];
        Self {
            input: input.clone(),
            animator: Animator::new(
                if input.get("owner").is_some_and(Value::is_null) {
                    None
                } else {
                    Some(num(input, "owner") as usize)
                },
                factory.clone(),
            ),
            animations: vec![
                Animation::new(1),
                Animation::with(2, 7, Some(Priority::High)),
                Animation::new(0),
                Animation::new(-1),
                Animation::with(535, 30, Some(Priority::Low)),
                Animation::with(863, 0, None),
                factory.animation_reset(),
                factory.animator_reset(),
            ],
            graphics: vec![
                Graphics::new(111, 128, 60),
                Graphics::new(369, 256, 30),
                factory.graphics_reset(),
            ],
            definitions: vec![
                Shared::new(defs::Definition::default()),
                Shared::new(defs::Definition::default()),
            ],
            registry: defs::Registry::default(),
            files: input["files"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.parse().unwrap(), bytes(v)))
                .collect(),
            objects: std::array::from_fn(|i| {
                Shared::new(
                    doors::Scenery::new(i as i32, locations[i].clone(), 10, 0, None, false)
                        .unwrap(),
                )
            }),
            locations,
            clocks: std::array::from_fn(|_| {
                input
                    .get("clocks")
                    .map_or_else(|| Some(Shared::new(vec![0; 10])), ints)
            }),
            masks: [0, 1],
            tick: input["tick"].as_i64().unwrap_or(100) as i32,
            selected: input["chunk"].as_i64().unwrap_or(0) as i32,
            position: 0,
            accepted: input["register"] != false,
            events: vec![],
            calls: HashMap::new(),
            registrations: vec![],
            queued: vec![],
        }
    }
    fn emit(&mut self, event: &str, args: Vec<Value>) -> Result<()> {
        let mut row = vec![json!(event)];
        row.extend(args);
        self.events.push(json!(row));
        let at = self.calls.entry(event.to_owned()).or_default();
        *at += 1;
        let at = *at;
        let hooks = self.input["hooks"].as_array().cloned().unwrap_or_default();
        for h in hooks {
            if h["event"] == event && h["at"].as_i64().unwrap_or(1) == at as i64 {
                for a in h["actions"].as_array().unwrap() {
                    self.act(a)?;
                }
            }
        }
        Ok(())
    }
    fn anim(&self, a: &Value) -> Option<Animation> {
        a["animation"]
            .as_u64()
            .map(|i| self.animations[i as usize].clone())
    }
    fn gfx(&self, a: &Value) -> Option<Graphics> {
        a["graphic"]
            .as_u64()
            .map(|i| self.graphics[i as usize].clone())
    }
    fn aid(&self, a: Option<Animation>) -> Option<usize> {
        a.map(|a| self.animations.iter().position(|b| a.same(b)).unwrap())
    }
    fn gid(&self, g: Option<Graphics>) -> Option<usize> {
        g.map(|g| self.graphics.iter().position(|b| g.same(b)).unwrap())
    }
    fn lid(&self, l: Option<Location>) -> Option<usize> {
        l.map(|l| self.locations.iter().position(|b| l.same(b)).unwrap())
    }
    fn did(&mut self, d: Option<defs::Handle>) -> Option<usize> {
        d.map(|d| {
            if let Some(i) = self.definitions.iter().position(|b| d.same(b)) {
                i
            } else {
                self.definitions.push(d);
                self.definitions.len() - 1
            }
        })
    }
    fn act(&mut self, a: &Value) -> Result<Value> {
        let animator = self.animator.clone();
        match a["op"].as_str().unwrap() {
            "decode" => {
                let data = bytes(&a["data"]).unwrap();
                let mut r = Reader::new(&data);
                let d = self.definitions[num(a, "definition") as usize].clone();
                let result = d.write().read_value_loop(&mut r);
                self.position = r.position();
                result.map_err(|_| Error::Binding("Decode".into()))?;
            }
            "finish" => self.definitions[num(a, "definition") as usize]
                .write()
                .finish(),
            "def_patch" => {
                if let Some(v) = a.get("durations") {
                    self.definitions[num(a, "definition") as usize]
                        .write()
                        .durations = ints(v);
                }
            }
            "lookup" => {
                let d = self.registry.clone().for_id(self, num(a, "id"));
                return Ok(json!(self.did(d)));
            }
            "audit" => {
                let mut rows = vec![];
                for id in 0..num(a, "count") {
                    let d = self.registry.clone().for_id(self, id);
                    self.did(d.clone());
                    let second = self.registry.clone().for_id(self, id);
                    let same = match (&d, &second) {
                        (Some(a), Some(b)) => a.same(b),
                        (None, None) => true,
                        _ => false,
                    };
                    rows.push(json!([id, d.map(|d| definition(&d.read())), same]));
                }
                return Ok(json!(rows));
            }
            "cache_set" => {
                let d = a["definition"]
                    .as_u64()
                    .map(|i| self.definitions[i as usize].clone());
                self.registry.entries.write().insert(num(a, "id"), d);
            }
            "cache_clear" => self.registry.entries.write().clear(),
            "duration" => {
                let a = self.anim(a).ok_or(Error::Null)?;
                return Ok(json!(a.duration(self)));
            }
            "get_definition" => {
                let a = self.anim(a).ok_or(Error::Null)?;
                let d = a.definition(self);
                return Ok(json!(self.did(d)));
            }
            "animate" => {
                let anim = self.anim(a);
                let gfx = self.gfx(a);
                return Ok(json!(animator.animate(self, anim, gfx)?));
            }
            "force" => {
                let anim = self.anim(a);
                animator.force_animation(self, anim)?;
            }
            "reset" => animator.reset(self)?,
            "stop" => animator.stop(self)?,
            "is_animating" => return Ok(json!(animator.is_animating(self)?)),
            "priority" => animator.set_priority(priority(&a["value"])),
            "anim_priority" => self
                .anim(a)
                .ok_or(Error::Null)?
                .set_priority(priority(&a["value"])),
            "id" => self.anim(a).ok_or(Error::Null)?.set_id(num(a, "value")),
            "object" => self.anim(a).ok_or(Error::Null)?.set_object(
                a["value"]
                    .as_u64()
                    .map(|i| self.objects[i as usize].clone()),
            ),
            "set_animation" => animator.set_animation(self.anim(a)),
            "set_graphics" => animator.set_graphics(self.gfx(a)),
            "clock" => self.tick = num(a, "value"),
            "clocks" => self.clocks[num(a, "entity") as usize] = ints(&a["value"]),
            "mask_select" => self.masks[num(a, "entity") as usize] = num(a, "value"),
            "register_result" => self.accepted = a["value"].as_bool().unwrap(),
            "file" => {
                self.files.insert(num(a, "id"), bytes(&a["data"]));
            }
            "chunk_select" => self.selected = num(a, "value"),
            "send" => {
                let g = self.gfx(a);
                let l = a["location"]
                    .as_u64()
                    .map(|i| self.locations[i as usize].clone());
                Graphics::send(self, g, l)?;
            }
            "move" => {
                let c = a["coords"].as_array().unwrap();
                self.locations[num(a, "location") as usize].set(
                    c[0].as_i64().unwrap() as i32,
                    c[1].as_i64().unwrap() as i32,
                    c[2].as_i64().unwrap() as i32,
                );
            }
            "fail" => return Err(Error::Binding("Binding".into())),
            op => panic!("unknown {op}"),
        }
        Ok(Value::Null)
    }
    fn snapshot(&mut self, result: Value, error: Option<&str>) -> Value {
        let entries: BTreeMap<_, _> = self
            .registry
            .entries
            .read()
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();
        let cache: BTreeMap<_, _> = entries
            .into_iter()
            .map(|(k, v)| (k.to_string(), self.did(v)))
            .collect();
        let mut animations = vec![];
        for a in self.animations.clone() {
            let d = self.did(a.cached_definition());
            animations.push(json!([
                a.id(),
                a.delay(),
                a.priority().map(|p| p as i32),
                d,
                a.object().map(|o| o.read().id),
                a.to_string()
            ]));
        }
        let definitions: Vec<_> = self
            .definitions
            .iter()
            .map(|d| definition(&d.read()))
            .collect();
        let graphics: Vec<_> = self
            .graphics
            .iter()
            .map(|g| json!([g.id(), g.height(), g.delay(), g.to_string()]))
            .collect();
        json!({"result":result,"error":error,"events":self.events,"tick":self.tick,"position":self.position,
        "deadline":self.animator.deadline(),"priority":self.animator.priority().map(|p|p as i32),"animation":self.aid(self.animator.animation()),"graphic":self.gid(self.animator.graphics()),
        "cache":cache,"animations":animations,"definitions":definitions,"graphics":graphics,"clocks":self.clocks,"masks":self.masks,
        "accepted":self.accepted,"selected":self.selected,"registrations":self.registrations,"queued":self.queued,"locations":self.locations.iter().map(Location::coordinates).collect::<Vec<_>>()})
    }
}
impl defs::Source for Harness {
    fn file(&mut self, _: &defs::Registry, g: i32, f: i32) -> defs::Result<Option<Vec<u8>>> {
        let data = self.files.get(&(g.wrapping_shl(7) | f)).cloned().flatten();
        let hex = data
            .as_ref()
            .map(|v| v.iter().map(|b| format!("{b:02x}")).collect::<String>());
        self.emit("file", vec![json!(g), json!(f), json!(hex)])
            .map_err(|e| defs::Error::Binding(e.to_string()))?;
        Ok(data)
    }
}
impl Definitions for Harness {
    fn definition(&mut self, _: &Animation, id: i32) -> Option<defs::Handle> {
        self.registry.clone().for_id(self, id)
    }
}
impl Context<usize> for Harness {
    fn world_ticks(&mut self, _: &Animator<usize>) -> Result<i32> {
        let tick = self.tick;
        self.emit("tick", vec![json!(tick)])?;
        Ok(tick)
    }
    fn clocks(
        &mut self,
        _: &Animator<usize>,
        entity: Option<&usize>,
    ) -> Result<Option<Shared<Vec<i32>>>> {
        Ok(self.clocks[*entity.ok_or(Error::Null)?].clone())
    }
    fn register_animation(
        &mut self,
        _: &Animator<usize>,
        entity: Option<&usize>,
        a: Animation,
    ) -> Result<bool> {
        let e = *entity.ok_or(Error::Null)?;
        let m = self.masks[e];
        self.emit("masks", vec![json!(e), json!(m)])?;
        let id = self.aid(Some(a));
        self.registrations.push(json!([m, "Animate", id]));
        self.emit("register", vec![json!(m), json!("Animate"), json!(id)])?;
        Ok(self.accepted)
    }
    fn register_graphics(
        &mut self,
        _: &Animator<usize>,
        entity: Option<&usize>,
        g: Graphics,
    ) -> Result<bool> {
        let e = *entity.ok_or(Error::Null)?;
        let m = self.masks[e];
        self.emit("masks", vec![json!(e), json!(m)])?;
        let id = self.gid(Some(g));
        self.registrations.push(json!([m, "SpotAnim", id]));
        self.emit("register", vec![json!(m), json!("SpotAnim"), json!(id)])?;
        Ok(self.accepted)
    }
}
impl GraphicDelivery for Harness {
    type Chunk = i32;
    type Flag = (Option<Graphics>, Option<Location>);
    fn chunk(&mut self, l: Option<Location>) -> Result<Option<i32>> {
        let c = self.selected;
        self.emit("chunk", vec![json!(self.lid(l)), json!(c)])?;
        Ok((c >= 0).then_some(c))
    }
    fn make_flag(&mut self, g: Option<Graphics>, l: Option<Location>) -> Result<Self::Flag> {
        self.emit(
            "make_flag",
            vec![json!(self.gid(g.clone())), json!(self.lid(l.clone()))],
        )?;
        Ok((g, l))
    }
    fn enqueue(&mut self, c: Option<i32>, f: Self::Flag) -> Result<()> {
        let c = c.ok_or(Error::Null)?;
        let g = self.gid(f.0);
        let l = self.lid(f.1);
        self.queued.push(json!([c, g, l]));
        self.emit("enqueue", vec![json!(c), json!(g), json!(l)])
    }
}
pub fn evaluate(input: &Value) -> Value {
    let mut h = Harness::new(input);
    let mut states = vec![];
    for a in input["actions"].as_array().unwrap() {
        h.events.clear();
        let (result, error) = match h.act(a) {
            Ok(v) => (v, None),
            Err(Error::Null) => (Value::Null, Some("Null")),
            Err(Error::Index) => (Value::Null, Some("Index")),
            Err(Error::Binding(s)) => (
                Value::Null,
                Some(if s == "Decode" { "Decode" } else { "Binding" }),
            ),
        };
        states.push(h.snapshot(result, error));
    }
    json!(states)
}
