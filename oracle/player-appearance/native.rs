//! Recording boundary over actual native Appearance/Container/Animation values.
use rs09_server::{
    definitions::{
        item::{ItemDefinition, Registry, Shared},
        npc::{Handle as Npc, NpcDefinition},
        property::Property,
    },
    inventory::{
        self as inv,
        container::{self, Container, ContainerType, Factory as Containers, SortType},
        item::{Handle as ItemHandle, Item},
    },
    player::appearance::{self as app, Appearance, CombatAnimation, Factory, Gender, Part, Parts},
    world::{
        animation::{Animation, Priority},
        doors::Location,
        player_walk::{NodeHandle, State},
    },
};
use serde_json::{Value, json};
use std::{any::Any, collections::BTreeMap, sync::Arc};
fn n(v: &Value, key: &str, d: i32) -> i32 {
    v[key].as_i64().map_or(d, |v| v as i32)
}
#[derive(Default)]
struct Ids {
    ids: BTreeMap<usize, usize>,
    keep: Vec<Box<dyn Any>>,
}
impl Ids {
    fn tag<T: Any>(&mut self, v: &Shared<T>) -> usize {
        let key = v.identity();
        if let Some(i) = self.ids.get(&key) {
            *i
        } else {
            let id = self.ids.len();
            self.ids.insert(key, id);
            self.keep.push(Box::new(v.clone()));
            id
        }
    }
}
#[derive(Default)]
struct Observe {
    arrays: Ids,
    parts: Ids,
    animations: Ids,
}
impl Observe {
    fn part(&mut self, p: Option<&Part>) -> Value {
        p.map_or(Value::Null, |p| {
            let reference = self.parts.tag(p);
            let p = p.read();
            json!({"ref":reference,"look":p.look,"color":p.color})
        })
    }
    fn animation(&mut self, a: Option<&Animation>) -> Value {
        a.map_or(Value::Null,|a|{let reference=self.animations.tag(&a.shared());json!({"ref":reference,"id":a.id(),"delay":a.delay(),"priority":a.priority().map(Priority::name)})})
    }
    fn parts(&mut self, p: &Parts) -> Value {
        json!(
            p.read()
                .iter()
                .map(|v| self.part(v.as_ref()))
                .collect::<Vec<_>>()
        )
    }
    fn appearance(&mut self, a: &Appearance, players: &[Player]) -> Value {
        let parts = a.parts();
        let parts_ref = self.arrays.tag(&parts);
        let values = self.parts(&parts);
        let animations = a.animations();
        let animations_ref = self.arrays.tag(&animations);
        let body = a.body();
        let body_ref = self.arrays.tag(&body);
        let owner = a
            .player()
            .and_then(|n| players.iter().position(|p| p.owner == n))
            .map_or(-1, |i| i as i32);
        json!({"partsRef":parts_ref,"parts":values,"animationsRef":animations_ref,"animations":&*animations.read(),"bodyRef":body_ref,"body":&*body.read(),"gender":a.gender().map(Gender::byte),"npc":a.npc_id(),"riding":a.riding(),"render":a.cached_render(),"skull":a.skull_icon(),"head":a.head_icon(),"player":owner})
    }
}
enum Held {
    Parts(Parts),
    Animations(Shared<[i32; 7]>),
    Body(Shared<[i32; 14]>),
}
pub(crate) struct Player {
    pub(crate) owner: NodeHandle,
    pub(crate) equipment: Container,
    pub(crate) appearance: String,
    pub(crate) prepared: [bool; 2],
    pub(crate) active: usize,
    pub(crate) size: i32,
    combat: [Option<Animation>; 3],
}
pub(crate) struct Context {
    pub(crate) row: Value,
    pub(crate) players: Vec<Player>,
    pub(crate) apps: BTreeMap<String, Appearance>,
    items: BTreeMap<String, Option<ItemHandle>>,
    npcs: BTreeMap<i32, Npc>,
    defs: Arc<Registry>,
    factory: Factory,
    pub(crate) override_: Property,
    held: BTreeMap<String, Held>,
    observe: Observe,
    _state: State,
    #[allow(dead_code)] // Used by the composed writer observer.
    pub(crate) extension: Option<Box<dyn Any>>,
    pub(crate) extension_hit: Option<fn(&mut Context, &str) -> app::Result<()>>,
    pub(crate) properties_lookup: Option<fn(&mut Context, usize) -> app::Result<usize>>,
    pub(crate) hits: BTreeMap<String, i32>,
    pub(crate) trace: Vec<Value>,
}
impl Context {
    pub(crate) fn owner(&self, p: &NodeHandle) -> app::Result<usize> {
        self.players
            .iter()
            .position(|x| x.owner == *p)
            .ok_or_else(|| app::Error::Binding("foreign player".into()))
    }
    pub(crate) fn hit(&mut self, key: &str, args: Vec<Value>) -> app::Result<()> {
        let mut event = vec![json!(key)];
        event.extend(args);
        self.trace.push(json!(event));
        let count = self.hits.entry(key.into()).or_default();
        *count += 1;
        let count = *count;
        let hooks = self.row["hooks"].as_array().unwrap().clone();
        for h in hooks {
            if h["when"] != key || n(&h, "at", 1) != count {
                continue;
            }
            match h["action"].as_str().unwrap() {
                "fail" => return Err(app::Error::Binding("controlled".into())),
                "clearEquipment" => self.players[0].equipment.items().write().fill(None),
                "redirect" => self.players[0].appearance = "B".into(),
                "part" => self.apps["A"].hair().ok_or(app::Error::Null)?.write().look = 77,
                "override" => self.override_ = Property::Null,
                "prepared" => self.players[0].active = 1,
                "reenter" => {
                    let a = self.apps["A"].clone();
                    a.transform_npc(self, -1)?;
                }
                v => panic!("unknown hook {v}"),
            }
        }
        if let Some(hook) = self.extension_hit {
            hook(self, key)?;
        }
        Ok(())
    }
    pub(crate) fn snapshot(&mut self) -> Value {
        let mut apps = BTreeMap::new();
        for (name, a) in &self.apps {
            apps.insert(name, self.observe.appearance(a, &self.players));
        }
        let players=self.players.iter().map(|p|{let attack=self.observe.animation(p.combat[0].as_ref());let defence=self.observe.animation(p.combat[1].as_ref());let death=self.observe.animation(p.combat[2].as_ref());json!({"equipment":p.equipment.items().read().iter().map(|v|v.as_ref().map(|i|{let i=i.read();json!({"id":i.id(),"amount":i.amount(),"charge":i.charge(),"index":i.index})})).collect::<Vec<_>>(),"size":p.size,"prepared":p.prepared,"activeRender":p.active,"attack":attack,"defence":defence,"death":death})}).collect::<Vec<_>>();
        let mut held = BTreeMap::new();
        for (name, v) in &self.held {
            let (reference, values) = match v {
                Held::Parts(p) => (self.observe.arrays.tag(p), self.observe.parts(p)),
                Held::Animations(a) => (self.observe.arrays.tag(a), json!(&*a.read())),
                Held::Body(b) => (self.observe.arrays.tag(b), json!(&*b.read())),
            };
            held.insert(name, json!({"ref":reference,"values":values}));
        }
        json!({"apps":apps,"players":players,"held":held})
    }
}
impl container::Context for Context {
    fn update(
        &mut self,
        _l: inv::container_listener::Handle,
        _c: &mut Container,
        _e: container::Event,
        _d: &Registry,
    ) -> inv::Result<()> {
        Err(inv::Error::Binding(
            "unexpected listener outside Appearance corpus".into(),
        ))
    }
    fn refresh(
        &mut self,
        _l: inv::container_listener::Handle,
        _c: &mut Container,
        _d: &Registry,
    ) -> inv::Result<()> {
        Err(inv::Error::Binding(
            "unexpected listener outside Appearance corpus".into(),
        ))
    }
    fn ground(
        &mut self,
        _c: &mut Container,
        _i: Option<ItemHandle>,
        _d: &Registry,
    ) -> inv::Result<()> {
        Err(inv::Error::Binding(
            "ground outside Appearance corpus".into(),
        ))
    }
}
impl app::BodyContext for Context {
    type Render = (usize, usize);
    fn equipment(&mut self, p: &NodeHandle) -> app::Result<Container> {
        let id = self.owner(p)?;
        self.hit("equipment", vec![json!(id)])?;
        Ok(self.players[id].equipment.clone())
    }
    fn render(&mut self, p: &NodeHandle) -> app::Result<(usize, usize)> {
        let id = self.owner(p)?;
        self.hit("render", vec![json!(id)])?;
        Ok((id, self.players[id].active))
    }
    fn prepared(&mut self, (p, r): (usize, usize)) -> app::Result<bool> {
        self.hit("prepared", vec![json!(r)])?;
        Ok(self.players[p].prepared[r])
    }
    fn set_prepared(&mut self, (p, r): (usize, usize), v: bool) -> app::Result<()> {
        self.hit("setPrepared", vec![json!(r), json!(v)])?;
        self.players[p].prepared[r] = v;
        Ok(())
    }
}
impl app::Context for Context {
    type Settings = usize;
    type Properties = usize;
    fn npc(&mut self, id: i32) -> app::Result<Npc> {
        self.hit("npc", vec![json!(id)])?;
        Ok(self
            .npcs
            .entry(id)
            .or_insert_with(|| Shared::new(NpcDefinition::default()))
            .clone())
    }
    fn size(&mut self, p: &NodeHandle, v: i32) -> app::Result<()> {
        let id = self.owner(p)?;
        self.hit("size", vec![json!(id), json!(v)])?;
        self.players[id].size = v;
        Ok(())
    }
    fn settings(&mut self, p: &NodeHandle) -> app::Result<usize> {
        let id = self.owner(p)?;
        self.hit("settings", vec![json!(id)])?;
        Ok(id)
    }
    fn attack_style(&mut self, _s: usize) -> app::Result<i32> {
        self.hit("style", vec![])?;
        Ok(n(&self.row, "style", 0))
    }
    fn properties(&mut self, p: &NodeHandle) -> app::Result<usize> {
        let id = self.owner(p)?;
        self.hit("properties", vec![json!(id)])?;
        if let Some(lookup) = self.properties_lookup {
            return lookup(self, id);
        }
        Ok(id)
    }
    fn combat_animation(
        &mut self,
        p: usize,
        slot: CombatAnimation,
        a: Option<Animation>,
    ) -> app::Result<()> {
        let (name, i) = match slot {
            CombatAnimation::Attack => ("Attack", 0),
            CombatAnimation::Defence => ("Defence", 1),
            CombatAnimation::Death => ("Death", 2),
        };
        let v = self.observe.animation(a.as_ref());
        self.hit("combat", vec![json!(name), v])?;
        self.players[p].combat[i] = a;
        Ok(())
    }
    fn update_appearance(&mut self, p: &NodeHandle) -> app::Result<()> {
        let id = self.owner(p)?;
        self.hit("sync", vec![json!(id)])
    }
    fn appearance(&mut self, p: &NodeHandle) -> app::Result<Appearance> {
        let id = self.owner(p)?;
        self.hit("appearance", vec![json!(id)])?;
        self.apps
            .get(&self.players[id].appearance)
            .cloned()
            .ok_or(app::Error::Null)
    }
    fn animate(&mut self, p: &NodeHandle, a: Animation) -> app::Result<bool> {
        let id = self.owner(p)?;
        let v = self.observe.animation(Some(&a));
        self.hit("animate", vec![json!(id), v])?;
        Ok(self.row["animate"] == true)
    }
    fn attribute(
        &mut self,
        p: &NodeHandle,
        k: &str,
        default: Option<i32>,
    ) -> app::Result<Property> {
        let id = self.owner(p)?;
        self.hit("attribute", vec![json!(id), json!(k), json!(default)])?;
        Ok(if self.override_.is_null() {
            default.map_or(Property::Null, Property::Int)
        } else {
            self.override_.clone()
        })
    }
}
fn property(v: &Value) -> Property {
    serde_json::from_value(v.clone()).unwrap()
}
pub(crate) fn setup(row: &Value) -> Context {
    let mut defs = Registry::default();
    for (id, d) in row["definitions"].as_object().unwrap() {
        let mut def = ItemDefinition {
            id: id.parse().unwrap(),
            equip_id: n(d, "equip", 0),
            ..Default::default()
        };
        for (key, v) in d.as_object().unwrap() {
            let p = if key == "attack_anims" && v.is_array() {
                Property::Animations(Shared::new(
                    v.as_array()
                        .unwrap()
                        .iter()
                        .map(|v| {
                            v.as_i64().map(|id| {
                                Animation::with(id as i32, 0, Some(Priority::High)).shared()
                            })
                        })
                        .collect(),
                ))
            } else {
                property(v)
            };
            def.handlers.insert(key.clone(), p);
        }
        defs.insert(id.parse().unwrap(), Shared::new(def));
    }
    let defs = Arc::new(defs);
    let factory = Factory::default();
    let containers = Containers::new(&defs);
    let mut npcs = BTreeMap::new();
    for (id, d) in row["npcs"].as_object().unwrap() {
        let npc = NpcDefinition {
            size: n(d, "size", 1),
            render_animation_id: n(d, "render", 0),
            stand_animation: n(d, "stand", -1),
            walk_animation: n(d, "walk", -1),
            turn_animation: n(d, "turn", -1),
            turn180_animation: n(d, "turn180", -1),
            turn_cw_animation: n(d, "cw", -1),
            turn_ccw_animation: n(d, "ccw", -1),
            ..Default::default()
        };
        for (key, config) in [
            ("melee", "melee_animation"),
            ("defence", "defence_animation"),
            ("death", "death_animation"),
        ] {
            if let Some(v) = d.get(key) {
                let p = if let Some(id) = v.as_i64() {
                    Property::Animation(
                        Animation::with(id as i32, 0, Some(Priority::High)).shared(),
                    )
                } else {
                    property(v)
                };
                npc.handlers.write().insert(config.into(), p);
            }
        }
        npcs.insert(id.parse().unwrap(), Shared::new(npc));
    }
    let mut state = State::default();
    let mut players = Vec::new();
    let mut apps = BTreeMap::new();
    for id in 0..2 {
        let node = state.add(id + 1, Location::new(3207, 3216, 0), [1, 2, 3, 4]);
        let owner = state.retain_node(node).unwrap();
        let name = if id == 0 { "A" } else { "B" }.to_owned();
        let a = Appearance::new(
            if id == 0 && row["nullOwner"] == true {
                None
            } else {
                Some(owner.clone())
            },
            factory.clone(),
        )
        .unwrap();
        apps.insert(name.clone(), a);
        players.push(Player {
            owner,
            equipment: containers
                .create(14, ContainerType::Default, SortType::Id)
                .unwrap(),
            appearance: name,
            prepared: [false; 2],
            active: 0,
            size: 1,
            combat: [None, None, None],
        });
    }
    if row["redirect"] == true {
        players[0].appearance = "B".into();
    }
    let mut items = BTreeMap::new();
    for (name, v) in row["items"].as_object().unwrap() {
        let item = (!v.is_null()).then(|| {
            let mut i = Item::new(
                n(v, "id", 0),
                n(v, "amount", 1),
                n(v, "charge", 1000),
                &defs,
            );
            if v.get("definition").is_some() {
                i.definition = if v["definition"].is_null() {
                    None
                } else {
                    Some(defs.for_id(n(v, "definition", 0)))
                };
            }
            Shared::new(i)
        });
        items.insert(name.clone(), item);
    }
    for (slot, name) in row["equipment"].as_object().unwrap() {
        players[0].equipment.items().write()[slot.parse::<usize>().unwrap()] =
            items.get(name.as_str().unwrap()).cloned().flatten();
    }
    Context {
        row: row.clone(),
        players,
        apps,
        items,
        npcs,
        defs,
        factory,
        override_: property(&row["override"]),
        held: BTreeMap::new(),
        observe: Observe::default(),
        hits: BTreeMap::new(),
        trace: vec![],
        _state: state,
        extension: None,
        extension_hit: None,
        properties_lookup: None,
    }
}
pub fn evaluate(row: &Value) -> Value {
    let mut ctx = setup(row);
    let mut results = Vec::new();
    for action in row["actions"].as_array().unwrap() {
        let a = ctx.apps[action["target"].as_str().unwrap_or("A")].clone();
        let index = n(action, "index", 0);
        let result = (|| -> app::Result<Value> {
            match action["op"].as_str().unwrap() {
                "construct" => {
                    ctx.apps.insert(
                        action["name"].as_str().unwrap().into(),
                        Appearance::new(Some(ctx.players[0].owner.clone()), ctx.factory.clone())?,
                    );
                }
                "prepare" => {
                    let owner = ctx.players[n(action, "player", 0) as usize].owner.clone();
                    a.prepare_body(&mut ctx, Some(&owner))?;
                }
                "gender" => a.set_gender(if action["value"].is_null() {
                    None
                } else if n(action, "value", 0) == 0 {
                    Some(Gender::Male)
                } else {
                    Some(Gender::Female)
                })?,
                "changeGender" => a.change_gender(
                    &mut ctx,
                    Some(if n(action, "value", 0) == 0 {
                        Gender::Male
                    } else {
                        Gender::Female
                    }),
                )?,
                "parse" => {
                    let text = serde_json::to_string(&action["value"]).unwrap();
                    let v = rs09_game_core::java_json::parse(&text).unwrap();
                    a.parse(&v)?;
                }
                "part" => {
                    let p = a.part(index as usize)?.ok_or(app::Error::Null)?;
                    if action.get("look").is_some() {
                        p.write().look = n(action, "look", 0);
                    }
                    if action.get("color").is_some() {
                        p.write().color = n(action, "color", 0);
                    }
                }
                "icons" => {
                    a.set_skull_icon(n(action, "skull", 0));
                    a.set_head_icon(n(action, "head", 0));
                }
                "animation" => a.set_animation(index as usize, n(action, "value", 0))?,
                "getter" => {
                    let p = match action["name"].as_str().unwrap() {
                        "hair" => a.hair(),
                        "beard" => a.beard(),
                        "torso" => a.torso(),
                        "arms" => a.arms(),
                        "wrists" => a.wrists(),
                        "skin" => a.skin(),
                        "legs" => a.legs(),
                        _ => a.feet(),
                    };
                    return Ok(ctx.observe.part(p.as_ref()));
                }
                "nullPart" => {
                    *a.parts()
                        .write()
                        .get_mut(index as usize)
                        .ok_or(app::Error::Index)? = None
                }
                "heldSet" => {
                    let value = n(action, "value", 0);
                    match &ctx.held[action["name"].as_str().unwrap()] {
                        Held::Animations(a) => {
                            *a.write().get_mut(index as usize).ok_or(app::Error::Index)? = value
                        }
                        Held::Body(b) => {
                            *b.write().get_mut(index as usize).ok_or(app::Error::Index)? = value
                        }
                        _ => return Err(app::Error::Type),
                    }
                }
                "capture" => {
                    let v = match action["kind"].as_str().unwrap() {
                        "parts" => Held::Parts(a.parts()),
                        "animations" => Held::Animations(a.animations()),
                        _ => Held::Body(a.body()),
                    };
                    ctx.held.insert(action["name"].as_str().unwrap().into(), v);
                }
                "retainedPart" => {
                    let Held::Parts(parts) = &ctx.held[action["name"].as_str().unwrap()] else {
                        return Err(app::Error::Type);
                    };
                    let p = parts
                        .read()
                        .get(index as usize)
                        .ok_or(app::Error::Index)?
                        .clone()
                        .ok_or(app::Error::Null)?;
                    if action.get("look").is_some() {
                        p.write().look = n(action, "look", 0);
                    }
                    if action.get("color").is_some() {
                        p.write().color = n(action, "color", 0);
                    }
                }
                "copy" => a.copy(action["source"].as_str().and_then(|n| ctx.apps.get(n)))?,
                "riding" => a.set_riding(action["value"] == true),
                "defaultAnimations" => a.default_animations(),
                "default" => {
                    let value = n(action, "value", 0);
                    match action["kind"].as_str().unwrap() {
                        "gender" => {
                            ctx.factory.gender_cache(Gender::Male).read()[index as usize]
                                .as_ref()
                                .unwrap()
                                .write()
                                .look = value
                        }
                        "animations" => {
                            ctx.factory.animation_defaults().write()[index as usize] = value
                        }
                        _ => {
                            let p = ctx.factory.default_attacks().read()[index as usize]
                                .clone()
                                .ok_or(app::Error::Null)?;
                            Animation::from_shared(p).set_id(value);
                        }
                    }
                }
                "defaultPart" => {
                    let p = ctx.factory.gender_cache(Gender::Male).read()[index as usize]
                        .clone()
                        .ok_or(app::Error::Null)?;
                    let mut p = p.write();
                    p.look = n(action, "look", 0);
                    p.color = n(action, "color", 0);
                }
                "defaultNull" => {
                    ctx.factory.gender_cache(Gender::Male).write()[index as usize] = None
                }
                "drawClothes" => a.draw_clothes(index, n(action, "value", 0))?,
                "drawItem" => a.draw_item(
                    index,
                    action["item"].as_str().and_then(|n| ctx.items[n].as_ref()),
                )?,
                "clear" => a.clear_body_part(index)?,
                "clip" => a.hat_clipping()?,
                "setAnimations" => a.set_animations(&mut ctx)?,
                "transform" => a.transform_npc(&mut ctx, n(action, "value", -1))?,
                "setRender" => {
                    a.set_render_animation(&mut ctx, Some(&Animation::new(n(action, "value", 0))))?
                }
                "render" => return a.render_animation(&mut ctx).map(|v| json!(v)),
                "configAnimation" => {
                    let v = ctx.npcs[&1]
                        .read()
                        .handlers
                        .read()
                        .get("melee_animation")
                        .cloned();
                    match v {
                        Some(Property::Animation(v)) => {
                            Animation::from_shared(v).set_id(n(action, "value", 0))
                        }
                        None | Some(Property::Null) => return Err(app::Error::Null),
                        _ => return Err(app::Error::Type),
                    }
                }
                "cart" => a.ride_cart(&mut ctx, action["value"] == true)?,
                "copter" => {
                    let d = ctx.defs.clone();
                    a.fly_copter(&mut ctx, &d)?;
                }
                v => panic!("unknown action{v}"),
            }
            Ok(Value::Null)
        })();
        let (value, error) = match result {
            Ok(v) => (v, Value::Null),
            Err(e) => (
                Value::Null,
                json!(match e {
                    app::Error::Null => "Null",
                    app::Error::Type => "Type",
                    app::Error::Index => "Index",
                    app::Error::Number => "Number",
                    app::Error::Binding(_) => "Binding",
                    app::Error::Inventory(inv::Error::Bounds) => "Index",
                    app::Error::Inventory(inv::Error::Null) => "Null",
                    e => panic!("unexpected{e:?}"),
                }),
            ),
        };
        let snapshot = ctx.snapshot();
        results.push(json!({"result":value,"error":error,"state":snapshot}));
    }
    json!({"results":results,"trace":ctx.trace})
}
