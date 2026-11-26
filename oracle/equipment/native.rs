//! Recording services around the production EquipmentContainer implementation.
use rs09_server::{
    definitions::{
        item::{ItemDefinition, Registry, Shared},
        plugin::{self, NativePlugin},
        property::{Property, Requirements},
    },
    inventory::{
        self as inv,
        container::{self, Container, ContainerType, Event, Factory, SortType},
        container_listener,
        equipment::{self, Perk},
        item::{Handle, Item},
    },
    world::{
        doors::Location,
        player_walk::{NodeHandle, State},
    },
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};
fn n(v: &Value, key: &str, d: i32) -> i32 {
    v[key].as_i64().map_or(d, |v| v as i32)
}
fn observe(item: Option<&Handle>) -> Value {
    item.map_or(Value::Null, |v| {
        let v = v.read();
        json!({"id":v.id(),"amount":v.amount(),"charge":v.charge(),"index":v.index,"name":v.name})
    })
}
fn items(c: &Container) -> Value {
    json!(
        c.items()
            .read()
            .iter()
            .map(|v| observe(v.as_ref()))
            .collect::<Vec<_>>()
    )
}
struct Plugin;
impl NativePlugin for Plugin {
    fn label(&self) -> &str {
        "equipment-test"
    }
}
struct Context {
    row: Value,
    owner: NodeHandle,
    inventory: Container,
    equipment: Container,
    target: Container,
    defs: Arc<Registry>,
    null_item: Handle,
    named: BTreeMap<String, Option<Handle>>,
    trace: Vec<Value>,
    frames: Vec<Value>,
    hits: BTreeMap<String, i32>,
    speed: i32,
    bonuses: Shared<Vec<i32>>,
    weights: i32,
    animations: i32,
    appearance_updates: i32,
    defence: i32,
    dfs: bool,
}
impl Context {
    fn hit(&mut self, key: &str, args: Vec<Value>) -> inv::Result<()> {
        let mut row = vec![json!(key)];
        row.extend(args);
        self.trace.push(json!(row));
        let count = self.hits.entry(key.into()).or_default();
        *count += 1;
        let count = *count;
        let hooks = self.row["hooks"].as_array().unwrap().clone();
        for hook in hooks {
            if hook["when"] != key || n(&hook, "at", 1) != count {
                continue;
            }
            match hook["action"].as_str().unwrap() {
                "fail" => return Err(inv::Error::Binding("controlled".into())),
                "clearInventory" => self.inventory.items().write().fill(None),
                "clearEquipment" => self.target.items().write().fill(None),
                "amount" => self.named[hook["item"].as_str().unwrap()]
                    .as_ref()
                    .unwrap()
                    .write()
                    .set_amount(n(&hook, "value", 3)),
                "index" => {
                    self.named[hook["item"].as_str().unwrap()]
                        .as_ref()
                        .unwrap()
                        .write()
                        .index = n(&hook, "value", 3)
                }
                "event" => self
                    .target
                    .event()
                    .read()
                    .flag(1, self.named.get("new").cloned().flatten())?,
                "replaceArray" => self
                    .target
                    .copy_items(&Shared::new(vec![None; 6]), &self.defs),
                "clearBonuses" => self.bonuses.write().fill(0),
                "reenter" => {
                    let mut c = self.target.clone();
                    let d = self.defs.clone();
                    c.refresh(None, &d, self)?;
                }
                v => panic!("unknown hook{v}"),
            }
        }
        Ok(())
    }
    fn state(&self) -> Value {
        json!({"inventory":items(&self.inventory),"equipment":items(&self.target),"ownEquipment":items(&self.equipment),"speed":self.speed,"bonuses":&*self.bonuses.read(),"weights":self.weights,"animations":self.animations,"appearanceUpdates":self.appearance_updates,"defence":self.defence,"dfs":self.dfs})
    }
}
impl container::Context for Context {
    fn equip(
        &mut self,
        c: &mut Container,
        item: Option<&Handle>,
        fire: bool,
        defs: &Registry,
    ) -> inv::Result<bool> {
        equipment::add(c, item, fire, true, defs, self)
    }
    fn update(
        &mut self,
        l: container_listener::Handle,
        c: &mut Container,
        e: Event,
        defs: &Registry,
    ) -> inv::Result<()> {
        l.value::<equipment::Listener>()
            .ok_or(inv::Error::Type)?
            .update_event(self, c, Some(&e), defs)
    }
    fn refresh(
        &mut self,
        l: container_listener::Handle,
        c: &mut Container,
        defs: &Registry,
    ) -> inv::Result<()> {
        l.value::<equipment::Listener>()
            .ok_or(inv::Error::Type)?
            .refresh(self, c, defs)
    }
    fn ground(&mut self, _c: &mut Container, _i: Option<Handle>, _d: &Registry) -> inv::Result<()> {
        Err(inv::Error::Binding("outside equipment corpus".into()))
    }
}
impl equipment::Context for Context {
    type Dispatch = ();
    type Properties = ();
    type Weapon = ();
    type CombatPulse = ();
    type Settings = ();
    type Appearance = ();
    type Interfaces = ();
    fn inventory(&mut self, p: &NodeHandle) -> inv::Result<Container> {
        assert!(p == &self.owner);
        self.hit("inventory", vec![])?;
        Ok(self.inventory.clone())
    }
    fn equipment(&mut self, p: &NodeHandle) -> inv::Result<Container> {
        assert!(p == &self.owner);
        self.hit("equipment", vec![])?;
        Ok(self.equipment.clone())
    }
    fn requirements(
        &mut self,
        d: rs09_server::definitions::item::Handle,
        _p: Option<&NodeHandle>,
        a: bool,
        b: bool,
    ) -> inv::Result<bool> {
        let id = d.read().id;
        self.hit("requirements", vec![json!(id), json!(a), json!(b)])?;
        Ok(self.row["allowed"] == true)
    }
    fn dispatch(&mut self, _p: &NodeHandle) -> inv::Result<()> {
        self.hit("dispatch", vec![])
    }
    fn message(&mut self, _d: (), s: &str) -> inv::Result<()> {
        self.hit("message", vec![json!(s)])
    }
    fn text(&mut self, _d: (), s: Option<String>, i: i32, c: i32) -> inv::Result<()> {
        self.hit("text", vec![json!(s), json!(i), json!(c)])
    }
    fn unequip_plugin(
        &mut self,
        p: plugin::Handle,
        _n: &NodeHandle,
        item: Handle,
    ) -> inv::Result<Property> {
        assert!(p.value::<Plugin>().is_some());
        self.hit("plugin", vec![json!("unequip"), observe(Some(&item))])?;
        Ok(serde_json::from_value(self.row["plugin"].clone()).unwrap())
    }
    fn interaction(
        &mut self,
        id: i32,
        _n: &NodeHandle,
        item: Handle,
        e: bool,
    ) -> inv::Result<bool> {
        self.hit(
            "interaction",
            vec![json!(id), observe(Some(&item)), json!(e)],
        )?;
        Ok(self.row["interaction"] == true)
    }
    fn send(&mut self, p: equipment::Packet, _c: &mut Container) -> inv::Result<()> {
        self.hit("packet", vec![json!(p.slots)])?;
        if p.player.is_none() {
            return Err(inv::Error::Null);
        }
        let f = p.frame(&self.null_item)?;
        self.frames.push(json!({"opcode":f.opcode,"payloadHex":f.payload.iter().map(|b|format!("{b:02x}")).collect::<String>()}));
        Ok(())
    }
    fn properties(&mut self, _p: &NodeHandle) -> inv::Result<()> {
        self.hit("properties", vec![])
    }
    fn attack_speed(&mut self, _p: (), v: i32) -> inv::Result<()> {
        self.hit("attackSpeed", vec![json!(v)])?;
        self.speed = v;
        Ok(())
    }
    fn extension(&mut self, _p: &NodeHandle) -> inv::Result<Option<()>> {
        self.hit("extension", vec![])?;
        Ok((self.row["extension"] == true).then_some(()))
    }
    fn update_weapon(&mut self, _p: ()) -> inv::Result<()> {
        self.hit("weapon", vec![])
    }
    fn defence_animation(&mut self, _p: ()) -> inv::Result<()> {
        self.hit("defence", vec![])?;
        self.defence += 1;
        Ok(())
    }
    fn cape_perks(&mut self, _p: &NodeHandle, _c: &mut Container, s: usize) -> inv::Result<()> {
        self.hit("cape", vec![json!(s)])
    }
    fn attribute_bool(&mut self, _p: &NodeHandle, k: &str, d: bool) -> inv::Result<bool> {
        self.hit("attribute", vec![json!(k), json!(d)])?;
        Ok(self.dfs)
    }
    fn remove_attribute(&mut self, _p: &NodeHandle, k: &str) -> inv::Result<()> {
        self.hit("removeAttribute", vec![json!(k)])?;
        self.dfs = false;
        Ok(())
    }
    fn combat_pulse(&mut self, _p: ()) -> inv::Result<()> {
        self.hit("combatPulse", vec![])
    }
    fn clear_combat_handler(&mut self, _p: ()) -> inv::Result<()> {
        self.hit("clearHandler", vec![])
    }
    fn settings(&mut self, _p: &NodeHandle) -> inv::Result<()> {
        self.hit("settings", vec![])
    }
    fn special_toggled(&mut self, _p: ()) -> inv::Result<bool> {
        self.hit("special", vec![])?;
        Ok(self.row["special"] == true)
    }
    fn set_varp(&mut self, _p: &NodeHandle, i: i32, v: i32) -> inv::Result<()> {
        self.hit("varp", vec![json!(i), json!(v)])
    }
    fn appearance(&mut self, _p: &NodeHandle) -> inv::Result<()> {
        self.hit("appearance", vec![])
    }
    fn set_animations(&mut self, _p: ()) -> inv::Result<()> {
        self.hit("animations", vec![])?;
        self.animations += 1;
        Ok(())
    }
    fn update_appearance(&mut self, _p: &NodeHandle) -> inv::Result<()> {
        self.hit("updateAppearance", vec![])?;
        self.appearance_updates += 1;
        Ok(())
    }
    fn update_weight(&mut self, _p: ()) -> inv::Result<()> {
        self.hit("weight", vec![])?;
        self.weights += 1;
        Ok(())
    }
    fn perk(&mut self, _p: &NodeHandle, p: Perk) -> inv::Result<bool> {
        let k = match p {
            Perk::FineAttunement => "fine",
            Perk::GrandBullwark => "grand",
        };
        self.hit("perk", vec![json!(k)])?;
        Ok(self.row[k] == true)
    }
    fn set_bonuses(&mut self, _p: (), v: Shared<Vec<i32>>) -> inv::Result<()> {
        let values = json!(&*v.read());
        self.hit("setBonuses", vec![values])?;
        self.bonuses = v;
        Ok(())
    }
    fn bonuses(&mut self, _p: ()) -> inv::Result<Shared<Vec<i32>>> {
        self.hit("bonuses", vec![])?;
        Ok(self.bonuses.clone())
    }
    fn interfaces(&mut self, _p: &NodeHandle) -> inv::Result<()> {
        self.hit("interfaces", vec![])
    }
    fn has_main(&mut self, _p: (), id: i32) -> inv::Result<bool> {
        self.hit("hasMain", vec![json!(id)])?;
        Ok(self.row["main"] == true)
    }
}
pub fn evaluate(row: &Value) -> Value {
    let mut defs = Registry::default();
    for (id, data) in row["definitions"].as_object().unwrap() {
        let id = id.parse().unwrap();
        let mut d = ItemDefinition {
            id,
            name: Some(format!("item{id}")),
            stackable: data["stack"] == true,
            ..Default::default()
        };
        for (key, v) in data.as_object().unwrap() {
            let key = match key.as_str() {
                "slot" => "equipment_slot",
                "two" => "two_handed",
                "speed" => "attack_speed",
                k => k,
            };
            let p = if key == "bonuses" && v.is_array() {
                Property::IntArray(Shared::new(
                    v.as_array()
                        .unwrap()
                        .iter()
                        .map(|v| v.as_i64().unwrap() as i32)
                        .collect(),
                ))
            } else if key == "requirements" && v.is_object() {
                let mut r = Requirements::default();
                for (k, v) in v.as_object().unwrap() {
                    r.insert(k.parse().unwrap(), v.as_i64().unwrap() as i32);
                }
                Property::Requirements(Shared::new(r))
            } else if key == "equipment" && v == "plugin" {
                Property::Plugin(plugin::Handle::new(Plugin))
            } else {
                serde_json::from_value(v.clone()).unwrap()
            };
            d.handlers.insert(key.into(), p);
        }
        defs.insert(id, Shared::new(d));
    }
    let defs = Arc::new(defs);
    let factory = Factory::new(&defs);
    let mut state = State::default();
    let node = state.add(1, Location::new(3207, 3216, 0), [1, 2, 3, 4]);
    let owner = state.retain_node(node).unwrap();
    let inventory = factory
        .create(28, ContainerType::Default, SortType::Id)
        .unwrap();
    let own_equipment = equipment::create(&factory, Some(owner.clone())).unwrap();
    let equipment = if row["nullPlayer"] == true || row["detachedEquipment"] == true {
        equipment::create(
            &factory,
            if row["nullPlayer"] == true {
                None
            } else {
                Some(owner.clone())
            },
        )
        .unwrap()
    } else {
        own_equipment.clone()
    };
    let mut named = BTreeMap::new();
    for (name, v) in row["items"].as_object().unwrap() {
        let item = (!v.is_null()).then(|| {
            let mut item = Item::new(
                n(v, "id", 1),
                n(v, "amount", 1),
                n(v, "charge", 1000),
                &defs,
            );
            item.index = n(v, "index", -1);
            if v.get("definition").is_some() {
                item.definition = Some(defs.for_id(n(v, "definition", 0)));
            }
            if v.get("name").is_some() {
                item.name = v["name"].as_str().map(String::from);
            }
            Shared::new(item)
        });
        named.insert(name.clone(), item);
    }
    for (key, target) in [("inventory", &inventory), ("equipment", &equipment)] {
        for (slot, name) in row[key].as_object().unwrap() {
            target.items().write()[slot.parse::<usize>().unwrap()] =
                named[name.as_str().unwrap()].clone();
        }
    }
    for (slot, name) in row["event"].as_object().unwrap() {
        equipment
            .event()
            .read()
            .flag(
                slot.parse().unwrap(),
                named.get(name.as_str().unwrap()).cloned().flatten(),
            )
            .unwrap();
    }
    if let Some(items) = row["ownEquipment"].as_object() {
        for (slot, name) in items {
            own_equipment.items().write()[slot.parse::<usize>().unwrap()] =
                named[name.as_str().unwrap()].clone();
        }
    }
    let mut ctx = Context {
        row: row.clone(),
        owner,
        inventory,
        equipment: own_equipment,
        target: equipment,
        defs: defs.clone(),
        null_item: factory.null_item,
        named,
        trace: vec![],
        frames: vec![],
        hits: BTreeMap::new(),
        speed: 0,
        bonuses: Shared::new(vec![0; 15]),
        weights: 0,
        animations: 0,
        appearance_updates: 0,
        defence: 0,
        dfs: row["dfs"] == true,
    };
    let mut results = Vec::new();
    for action in row["actions"].as_array().unwrap() {
        let item = action["item"]
            .as_str()
            .and_then(|k| ctx.named.get(k).cloned().flatten());
        let mut c = ctx.target.clone();
        let fire = action["fire"] != false;
        let result = match action["op"].as_str().unwrap() {
            "equip" => c
                .add_two(item.as_ref(), fire, &defs, &mut ctx)
                .map(|v| json!(v)),
            "explicit" => equipment::add_at(
                &mut c,
                item.as_ref(),
                n(action, "inventorySlot", 0),
                fire,
                action["fromInventory"] != false,
                &defs,
                &mut ctx,
            )
            .map(|v| json!(v)),
            "one" => c
                .add(item.as_ref(), true, -1, &defs, &mut ctx)
                .map(|v| json!(v)),
            "three" => c
                .add(item.as_ref(), fire, -1, &defs, &mut ctx)
                .map(|v| json!(v)),
            "array" => c
                .add_many(&Shared::new(vec![item.clone()]), &defs, &mut ctx)
                .map(|v| json!(v)),
            "list" => c
                .add_list(std::slice::from_ref(&item), &defs, &mut ctx)
                .map(|_| Value::Null),
            "all" => c
                .add_many(&Shared::new(vec![item.clone()]), &defs, &mut ctx)
                .map(|_| Value::Null),
            "absent" => c
                .add_if_absent(item.as_ref(), &defs, &mut ctx)
                .map(|v| json!(v)),
            "shift" => c.shift(&defs, &mut ctx).map(|_| Value::Null),
            "refresh" => c.refresh(None, &defs, &mut ctx).map(|_| Value::Null),
            "update" => c.update(None, &defs, &mut ctx).map(|_| Value::Null),
            "bonuses" => {
                equipment::update_bonuses(&ctx.owner.clone(), &mut ctx).map(|_| Value::Null)
            }
            "stats" => equipment::update(&ctx.owner.clone(), &mut ctx).map(|_| Value::Null),
            v => panic!("unknown op{v}"),
        };
        let (value, error) = match result {
            Ok(v) => (v, Value::Null),
            Err(e) => (
                Value::Null,
                json!(match e {
                    inv::Error::Null => "Null",
                    inv::Error::Bounds => "Bounds",
                    inv::Error::Type => "Type",
                    inv::Error::Binding(_) => "Binding",
                    _ => panic!("unexpected error{e:?}"),
                }),
            ),
        };
        results.push(json!({"result":value,"error":error,"state":ctx.state()}));
    }
    let named: BTreeMap<_, _> = ctx
        .named
        .iter()
        .map(|(k, v)| (k, observe(v.as_ref())))
        .collect();
    json!({"results":results,"trace":ctx.trace,"frames":ctx.frames,"named":named})
}
