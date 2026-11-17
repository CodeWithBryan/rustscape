use rs09_protocol::{buffer::Reader, game::Direction};
use rs09_server::{
    definitions::scenery as defs,
    pulses,
    world::{
        doors::{self, Handle, Location},
        scene::{self, Assets, Scene, Services},
        scenery, scenery_node,
    },
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub fn bytes(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn loc(v: &Value) -> Location {
    Location::new(
        v[0].as_i64().unwrap() as i32,
        v[1].as_i64().unwrap() as i32,
        v[2].as_i64().unwrap() as i32,
    )
}
pub struct MemoryAssets {
    pub landscape: Vec<u8>,
    pub missing: bool,
}
impl Assets for MemoryAssets {
    fn archive_id(&self, name: &str) -> scene::Result<i32> {
        let (x, y) = name[1..].split_once('_').unwrap();
        let (x, y): (i32, i32) = (x.parse().unwrap(), y.parse().unwrap());
        Ok(
            if self.missing || !(49..=51).contains(&x) || !(49..=51).contains(&y) {
                -1
            } else {
                ((x << 8) | y) * 2 + i32::from(name.starts_with('l'))
            },
        )
    }
    fn mapscape(&self, _: i32) -> scene::Result<Option<Vec<u8>>> {
        Ok(Some(vec![0; 16384]))
    }
    fn landscape(&self, group: i32, _: [u32; 4]) -> scene::Result<Option<Vec<u8>>> {
        Ok(Some(if group / 2 == 12850 {
            self.landscape.clone()
        } else {
            vec![0; 4]
        }))
    }
    fn keys(&self, _: i32) -> scene::Result<[u32; 4]> {
        Ok([0; 4])
    }
}
#[derive(Default)]
pub struct Bindings {
    pub events: Vec<Value>,
}
impl<A: Assets> Services<A> for Bindings {
    fn is_player(&self, _: usize) -> bool {
        true
    }
    fn location(&mut self, _: &mut Scene<A>, _: usize) -> scene::Result<Location> {
        self.events.push(json!(["location"]));
        Ok(Location::new(3207, 3217, 0))
    }
    fn lock(&mut self, _: &mut Scene<A>, _: usize, t: i32) -> scene::Result<()> {
        self.events.push(json!(["lock", t]));
        Ok(())
    }
    fn audio(
        &mut self,
        _: &mut Scene<A>,
        _: usize,
        id: i32,
        delay: Option<i32>,
    ) -> scene::Result<()> {
        self.events.push(json!(["audio", id, delay]));
        Ok(())
    }
    fn message(&mut self, _: &mut Scene<A>, _: usize, s: &str) -> scene::Result<()> {
        self.events.push(json!(["message", s]));
        Ok(())
    }
    fn logout_listener(
        &mut self,
        _: &mut Scene<A>,
        _: usize,
        l: Option<Location>,
    ) -> scene::Result<()> {
        self.events.push(json!([if l.is_some() {
            "logoutAdd"
        } else {
            "logoutRemove"
        }]));
        Ok(())
    }
    fn reset_path(&mut self, _: &mut Scene<A>, _: usize) -> scene::Result<()> {
        self.events.push(json!(["walkReset"]));
        Ok(())
    }
    fn add_path(&mut self, _: &mut Scene<A>, _: usize, x: i32, y: i32) -> scene::Result<()> {
        self.events.push(json!(["walkAdd", x, y]));
        Ok(())
    }
    fn move_near(&mut self, _: &mut Scene<A>, _: usize, l: Location) -> scene::Result<bool> {
        self.events.push(json!(["find", l.coordinates()]));
        Ok(false)
    }
    fn diary(
        &mut self,
        _: &mut Scene<A>,
        _: usize,
        d: doors::Diary,
        level: i32,
        task: i32,
    ) -> scene::Result<()> {
        self.events.push(json!([
            "diary",
            if d == doors::Diary::Falador {
                "FALADOR"
            } else {
                "LUMBRIDGE"
            },
            level,
            task
        ]));
        Ok(())
    }
    fn ground_create(&mut self, _: &mut Scene<A>, item: Option<usize>) -> scene::Result<()> {
        self.events.push(json!(["ground", item]));
        Ok(())
    }
    fn pulse(&mut self, _: &mut Scene<A>, _: &pulses::Handle) -> scene::Result<bool> {
        Err(scene::Error("unexpected external pulse".into()))
    }
    fn stop(&mut self, _: &mut Scene<A>, _: &pulses::Handle) -> scene::Result<()> {
        Err(scene::Error("unexpected external stop".into()))
    }
    fn now_millis(&mut self) -> i64 {
        0
    }
}
pub fn create(input: &Value) -> Scene<MemoryAssets> {
    struct Log;
    impl defs::LogContext for Log {
        fn log(&mut self, _: &mut defs::Registry, s: String) -> defs::Result<()> {
            panic!("unexpected definition diagnostic: {s}")
        }
    }
    let samples: BTreeMap<String, String> =
        serde_json::from_str(include_str!("../scenery-definitions/cache-samples.json")).unwrap();
    let mut definitions = defs::Registry::default();
    for (id, data) in samples {
        let id = id.parse().unwrap();
        let h = definitions
            .decode(&mut Log, id, &mut Reader::new(&bytes(&data)))
            .unwrap();
        definitions.definitions.insert(id, h);
    }
    let doors = doors::Registry::default();
    doors::Loader::default()
        .load_text(
            include_str!("../../fixtures/doors/door_configs.json"),
            &doors,
            &mut |_| Ok(()),
        )
        .unwrap();
    Scene::new(
        MemoryAssets {
            landscape: bytes(input["land"].as_str().unwrap()),
            missing: input["missing"].as_bool().unwrap_or(false),
        },
        definitions,
        doors,
    )
}
fn value(h: Option<&Handle>) -> Value {
    let Some(h) = h else {
        return Value::Null;
    };
    let h = h.read();
    let previous = h.replaced.as_ref().map(|p| {
        let p = p.read();
        json!([p.id, p.location.coordinates(), p.rotation])
    });
    json!([
        h.id,
        h.shape,
        h.rotation,
        h.location.coordinates(),
        h.active,
        h.renderable,
        h.constructed,
        h.charge,
        previous
    ])
}
fn snapshot<A: Assets>(
    scene: &mut Scene<A>,
    services: &Bindings,
    held: &BTreeMap<String, Option<Handle>>,
) -> Value {
    let mut regions = vec![];
    let mut storage = vec![];
    let mut ids: Vec<_> = scene.regions.keys().copied().collect();
    ids.sort();
    for id in ids {
        let r = scene.regions[&id].read();
        regions.push(json!([
            id,
            r.loaded,
            r.build,
            r.has_flags,
            r.data.read().object_count
        ]));
        let chunks = r.chunks.as_ref().unwrap().read();
        for cx in 0..8 {
            for cy in 0..8 {
                for z in 0..4 {
                    let c = chunks[cx][cy][z].as_ref().unwrap().read();
                    for (stat, objects) in [&c.static_objects, &c.objects].into_iter().enumerate() {
                        let objects = objects.read();
                        for x in 0..8 {
                            for y in 0..8 {
                                for s in 0..4 {
                                    if let Some(h) = &objects[x][y][s] {
                                        let aliases: Vec<_> = held
                                            .iter()
                                            .filter(|(_, n)| n.as_ref().is_some_and(|n| n.same(h)))
                                            .map(|(k, _)| k)
                                            .collect();
                                        storage.push(json!([
                                            id,
                                            cx,
                                            cy,
                                            z,
                                            stat,
                                            x,
                                            y,
                                            s,
                                            value(Some(h)),
                                            aliases
                                        ]));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    let mut hash = Sha256::new();
    let keys: BTreeSet<_> = scene.flags.keys().map(|(k, _)| *k).collect();
    for k in keys {
        for p in [false, true] {
            if let Some(flags) = scene.flags.get(&(k, p)) {
                hash.update(k.to_be_bytes());
                hash.update([u8::from(p)]);
                for i in *flags.read() {
                    hash.update(i.to_be_bytes());
                }
            }
        }
    }
    let held: BTreeMap<_, _> = held.iter().map(|(k, h)| (k, value(h.as_ref()))).collect();
    let queue: Vec<_> = scene
        .runner
        .current()
        .iter()
        .map(|p| {
            let p = p.read();
            json!([p.delay, p.ticks_passed, p.running])
        })
        .collect();
    let mut frames = vec![];
    let mut keys: Vec<_> = scene.updates.keys().copied().collect();
    keys.sort();
    for k in keys {
        let frame = scene
            .update_frame(k, &Location::new(3207, 3216, 0))
            .unwrap()
            .unwrap();
        let mut wire = vec![];
        frame
            .write(Direction::ServerToClient, None, &mut wire)
            .unwrap();
        frames.push(json!([k, hex(&wire)]));
    }
    scene.clear_updates();
    assert!(scene.diagnostics.is_empty(), "{:?}", scene.diagnostics);
    json!({"regions":regions,"storage":storage,"flags":hex(&hash.finalize()),"held":held,"queue":queue,"frames":frames,"events":services.events})
}
pub fn evaluate(input: &Value) -> Value {
    evaluate_scene(input, create(input))
}
pub fn evaluate_scene<A: Assets>(input: &Value, mut scene: Scene<A>) -> Value {
    let mut services = Bindings::default();
    let mut held: BTreeMap<String, Option<Handle>> = BTreeMap::new();
    let mut out = vec![];
    for a in input["actions"].as_array().unwrap() {
        services.events.clear();
        let h = a["node"]
            .as_str()
            .and_then(|s| held.get(s))
            .and_then(Clone::clone);
        let mut world = scene.context(&mut services);
        match a["op"].as_str().unwrap() {
            "load" => {
                world
                    .load(
                        a["id"].as_i64().unwrap() as i32,
                        Some(a["build"].as_bool().unwrap()),
                    )
                    .unwrap();
            }
            "lookup" => {
                let h = world.object(loc(&a["location"]), -1, 0).unwrap();
                held.insert(a["name"].as_str().unwrap().into(), h);
            }
            "door" => world.door(0, &h.unwrap()).unwrap(),
            "tick" => {
                for _ in 0..a["count"].as_u64().unwrap() {
                    world.tick().unwrap();
                }
            }
            "remove" => {
                scenery::remove(&mut world, h.as_ref()).unwrap();
            }
            "respawn" => {
                scenery::remove_respawn(
                    &mut world,
                    h.as_ref(),
                    a["ticks"].as_i64().unwrap() as i32,
                )
                .unwrap();
            }
            "expire" => {
                scenery::add(
                    &mut world,
                    h.as_ref(),
                    a["ticks"].as_i64().unwrap() as i32,
                    None,
                )
                .unwrap();
            }
            "client" => {
                let other = scenery_node::construct(
                    &mut world.scene.definitions,
                    12348,
                    Location::new(3207, 3217, 0),
                    0,
                    1,
                    false,
                )
                .unwrap();
                scenery::replace(
                    &mut world,
                    h.as_ref(),
                    Some(&other),
                    scenery::ReplaceMode::Temporary {
                        clip: false,
                        ticks: a["ticks"].as_i64().unwrap() as i32,
                    },
                )
                .unwrap();
            }
            other => panic!("unknown scene action {other}"),
        }
        out.push(snapshot(&mut scene, &services, &held));
    }
    json!(out)
}
