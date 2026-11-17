//! Replays the force movement corpus (fixtures/force-movement) through
//! `world::force_movement` on the native pulse base (`pulses::update_base`).
//! The host answers every service from the case the way the oracle's
//! recording adapters do. Every operation's result, error, recorded calls and
//! printed stack traces, the shared animations and every movement's fields
//! are compared whole. A second test joins `start` to the native update masks.
use rs09_server::pulses::{Error, Result};
use rs09_server::world::force_movement::{
    self, AnimationHandle, Direction, ForceMoveCtx, ForceMovement, Handle, Host, Location, Node,
    Priority, RunForm, Statics,
};
use rs09_server::world::prayer::location;
use rs09_server::world::update_masks::{self, Flag, Kind, Masks, Provider, ProviderHandle};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Arc;

const CORPUS: &str = include_str!("../../../fixtures/force-movement/oracle.jsonl");
const PROVENANCE: &str = include_str!("../../../fixtures/force-movement/provenance.json");

/// Differences between the original and the port, by case group and the
/// reason. Empty: every case matches. A closed gap or a new difference fails.
const GAPS: &[(&str, &str)] = &[];

fn int(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}

fn place(v: &Value) -> Option<Location> {
    let a = v.as_array()?;
    Some(location(int(&a[0]), int(&a[1]), int(&a[2])))
}

fn loc(l: Option<Location>) -> Value {
    l.map_or(Value::Null, |l| json!(l))
}

struct Entity {
    player: bool,
    location: Option<Location>,
    active: bool,
}

struct Replay {
    entities: Vec<Entity>,
    statics: Statics,
    anims: Vec<AnimationHandle>,
    movements: Vec<Handle>,
    events: Vec<Value>,
    calls: i64,
    fail_at: i64,
}

impl Replay {
    fn emit(&mut self, event: Value) -> Result<()> {
        self.events.push(event);
        self.calls += 1;
        if self.calls == self.fail_at {
            Err(Error::Binding("injected".into()))
        } else {
            Ok(())
        }
    }

    fn anim_label(&self, a: Option<&AnimationHandle>) -> Value {
        match a {
            None => Value::Null,
            Some(a) if a.same(&self.statics.walk_animation) => json!("walk"),
            Some(a) => json!(format!(
                "a{}",
                self.anims.iter().position(|x| x.same(a)).unwrap()
            )),
        }
    }

    fn anim(&self, a: Option<&AnimationHandle>) -> Value {
        match a {
            None => Value::Null,
            Some(h) => {
                let x = h.read().clone();
                json!([self.anim_label(a), x.id, x.priority.name()])
            }
        }
    }

    fn label(&mut self, fm: &Handle) -> String {
        if !self.movements.iter().any(|m| m.same(fm)) {
            self.movements.push(fm.clone());
        }
        format!(
            "f{}",
            self.movements.iter().position(|m| m.same(fm)).unwrap()
        )
    }

    fn animation(&self, v: &Value) -> Option<AnimationHandle> {
        match v.as_str() {
            None => None,
            Some("walk") => Some(self.statics.walk_animation.clone()),
            Some(label) => Some(self.anims[label[1..].parse::<usize>().unwrap()].clone()),
        }
    }

    fn describe(&self, fm: &Handle) -> Value {
        let f = fm.read();
        let p = f.pulse.read();
        let checks = p.checks.as_ref().map(|c| c.read().clone());
        json!([
            f.entity,
            loc(f.start),
            loc(f.destination),
            self.anim_label(f.start_anim.as_ref()),
            self.anim_label(f.animation.as_ref()),
            self.anim_label(f.end_animation.as_ref()),
            f.direction.map(|d| d.name()),
            f.commence_speed,
            f.path_speed,
            f.unlock_after,
            p.delay,
            p.ticks_passed,
            p.running,
            checks,
        ])
    }

    fn state(&self) -> Value {
        let mut anims = serde_json::Map::new();
        let walk = self.statics.walk_animation.read().clone();
        anims.insert("walk".into(), json!([walk.id, walk.priority.name()]));
        for (i, a) in self.anims.iter().enumerate() {
            let a = a.read().clone();
            anims.insert(format!("a{i}"), json!([a.id, a.priority.name()]));
        }
        json!({"anims": anims, "movements": self.movements.iter().map(|m| self.describe(m)).collect::<Vec<_>>()})
    }
}

impl Host for Replay {
    fn is_player(&self, entity: Node) -> bool {
        self.entities[entity].player
    }
    fn location(&mut self, entity: Node) -> Result<Option<Location>> {
        self.emit(json!(["location", entity]))?;
        Ok(self.entities[entity].location)
    }
    fn animate(&mut self, entity: Node, animation: Option<&AnimationHandle>) -> Result<bool> {
        let a = self.anim(animation);
        self.emit(json!(["animate", entity, a]))?;
        Ok(true)
    }
    fn set_disabled_ticks(&mut self, entity: Node, ticks: i32) -> Result<()> {
        self.emit(json!(["disabled_ticks", entity, ticks]))
    }
    fn register_force_move(&mut self, entity: Node, c: ForceMoveCtx) -> Result<bool> {
        self.emit(json!([
            "register_mask",
            entity,
            "ForceMove",
            [
                c.start,
                c.dest,
                c.start_arrive,
                c.dest_arrive,
                c.direction.name()
            ]
        ]))?;
        Ok(true)
    }
    fn update_region(
        &mut self,
        entity: Node,
        start: Location,
        destination: Location,
        forced: bool,
    ) -> Result<bool> {
        self.emit(json!(["update_region", entity, start, destination, forced]))?;
        Ok(true)
    }
    fn set_teleport_location(&mut self, entity: Node, location: Option<Location>) -> Result<()> {
        self.emit(json!(["teleport_location", entity, loc(location)]))
    }
    fn lock(&mut self, entity: Node) -> Result<()> {
        self.emit(json!(["lock", entity]))
    }
    fn unlock(&mut self, entity: Node) -> Result<()> {
        self.emit(json!(["unlock", entity]))
    }
    fn submit(&mut self, movement: &Handle) -> Result<()> {
        let label = self.label(movement);
        self.emit(json!(["submit", label]))
    }
    fn node_active(&mut self, node: Node) -> Result<bool> {
        self.emit(json!(["active", node]))?;
        Ok(self.entities[node].active)
    }
    fn exception(&mut self, error: &Error) -> Result<()> {
        // A printed stack trace: an observation, never a failing call.
        self.events.push(json!(["exception", error_name(error)]));
        Ok(())
    }
}

fn error_name(e: &Error) -> &'static str {
    match e {
        Error::Binding(_) => "Binding",
        Error::Null => "Null",
        Error::Index => "Index",
        other => panic!("error {other:?}"),
    }
}

fn entity(op: &Value, key: &str) -> Option<Node> {
    op[key].as_u64().map(|v| v as usize)
}

fn direction(v: &Value) -> Option<Direction> {
    v.as_str().map(|d| Direction::value_of(d).unwrap())
}

fn movement(r: &Replay, op: &Value) -> Handle {
    r.movements[int(&op["fm"]) as usize].clone()
}

fn construct(r: &mut Replay, op: &Value) -> Result<Handle> {
    let (e, start, dest) = (entity(op, "e"), place(&op["start"]), place(&op["dest"]));
    let (start_anim, anim, dir) = (
        r.animation(&op["startAnim"]),
        r.animation(&op["anim"]),
        direction(&op["direction"]),
    );
    let statics = r.statics.clone();
    match op["form"].as_str().unwrap() {
        "full" => Ok(ForceMovement::new(
            e,
            start,
            dest,
            start_anim,
            anim,
            dir,
            int(&op["commence"]),
            int(&op["path"]),
            op["unlock"].as_bool().unwrap(),
        )),
        "speed" => ForceMovement::with_speed(&statics, e, start, dest, anim, int(&op["speed"])),
        "speeds" => {
            ForceMovement::with_speeds(r, &statics, e, dest, int(&op["commence"]), int(&op["path"]))
        }
        "animation" => ForceMovement::with_animation(&statics, e, start, dest, anim),
        "detached" => ForceMovement::detached(&statics, start, dest, anim),
        other => panic!("form {other}"),
    }
}

fn run_static(r: &mut Replay, op: &Value) -> Result<Handle> {
    let (e, start, dest) = (entity(op, "e"), place(&op["start"]), place(&op["dest"]));
    let (sa, a, d) = (
        r.animation(&op["startAnim"]),
        r.animation(&op["anim"]),
        direction(&op["direction"]),
    );
    let n = |k: &str| int(&op[k]);
    let form = match op["form"].as_str().unwrap() {
        "r2" => RunForm::Destination,
        "r3" => RunForm::Start,
        "r4" => RunForm::Animation(a),
        "r5" => RunForm::AnimationSpeed(a, n("speed")),
        "r5a" => RunForm::Animations(sa, a),
        "r6" => RunForm::Direction(sa, a, d),
        "r7" => RunForm::PathSpeed(sa, a, d, n("path")),
        "r8" => RunForm::Speeds(sa, a, d, n("commence"), n("path")),
        "r9" => RunForm::Full(
            sa,
            a,
            d,
            n("commence"),
            n("path"),
            op["unlock"].as_bool().unwrap(),
        ),
        "rSpeeds" => RunForm::DestinationSpeeds(n("commence"), n("path")),
        other => panic!("form {other}"),
    };
    let statics = r.statics.clone();
    force_movement::run_static(r, &statics, e, start, dest, form)
}

fn run(r: &mut Replay, op: &Value) -> Result<Value> {
    Ok(match op["op"].as_str().unwrap() {
        "set" => {
            let e = &mut r.entities[int(&op["e"]) as usize];
            for (k, v) in op["fields"].as_object().unwrap() {
                match k.as_str() {
                    "active" => e.active = v.as_bool().unwrap(),
                    "location" => e.location = place(v),
                    other => panic!("field {other}"),
                }
            }
            Value::Null
        }
        "construct" => {
            let fm = construct(r, op)?;
            json!(r.label(&fm))
        }
        "run" => {
            let fm = run_static(r, op)?;
            json!(r.label(&fm))
        }
        "runWith" => {
            let fm = movement(r, op);
            force_movement::run_with(r, &fm, entity(op, "e"), int(&op["speed"]))?;
            Value::Null
        }
        "runEntity" => {
            let fm = movement(r, op);
            force_movement::run_entity(r, &fm, entity(op, "e"))?;
            Value::Null
        }
        "runSelf" => {
            let fm = movement(r, op);
            force_movement::run_self(r, &fm)?;
            Value::Null
        }
        "start" => {
            let fm = movement(r, op);
            force_movement::start(r, &fm)?;
            Value::Null
        }
        "pulse" => {
            let fm = movement(r, op);
            json!(force_movement::pulse(r, &fm)?)
        }
        "update" => {
            let fm = movement(r, op);
            json!(force_movement::update(r, &fm)?)
        }
        "updates" => {
            let fm = movement(r, op);
            let mut out = Vec::new();
            for _ in 0..int(&op["n"]) {
                out.push(force_movement::update(r, &fm)?);
            }
            json!(out)
        }
        "stop" => {
            let fm = movement(r, op);
            force_movement::stop(r, &fm)?;
            Value::Null
        }
        "direction" => json!(
            force_movement::direction(
                place(&op["start"]).ok_or(Error::Null)?,
                place(&op["dest"]).ok_or(Error::Null)?
            )
            .name()
        ),
        "setStart" => {
            movement(r, op).write().start = place(&op["value"]);
            Value::Null
        }
        "setDestination" => {
            movement(r, op).write().destination = place(&op["value"]);
            Value::Null
        }
        "setDirection" => {
            movement(r, op).write().direction = direction(&op["value"]);
            Value::Null
        }
        "setCommenceSpeed" => {
            movement(r, op).write().commence_speed = int(&op["value"]);
            Value::Null
        }
        "setPathSpeed" => {
            movement(r, op).write().path_speed = int(&op["value"]);
            Value::Null
        }
        "setEntity" => {
            movement(r, op).write().entity = entity(op, "value");
            Value::Null
        }
        "setStartAnim" => {
            let a = r.animation(&op["value"]);
            movement(r, op).write().start_anim = a;
            Value::Null
        }
        "setAnimation" => {
            let a = r.animation(&op["value"]);
            movement(r, op).write().animation = a;
            Value::Null
        }
        "setEndAnimation" => {
            let a = r.animation(&op["value"]);
            movement(r, op).write().end_animation = a;
            Value::Null
        }
        "get" => {
            let fm = movement(r, op);
            let f = fm.read();
            match op["field"].as_str().unwrap() {
                "start" => loc(f.start),
                "destination" => loc(f.destination),
                "direction" => json!(f.direction.map(|d| d.name())),
                "commenceSpeed" => json!(f.commence_speed),
                "pathSpeed" => json!(f.path_speed),
                "entity" => json!(f.entity),
                "startAnim" => r.anim_label(f.start_anim.as_ref()),
                "animation" => r.anim_label(f.animation.as_ref()),
                "endAnimation" => r.anim_label(f.end_animation.as_ref()),
                other => panic!("field {other}"),
            }
        }
        other => panic!("op {other}"),
    })
}

fn replay_case(row: &Value) -> Vec<String> {
    let case = &row["input"];
    let mut r = Replay {
        entities: case["entities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| Entity {
                player: e["kind"] != "npc",
                location: place(&e["location"]),
                active: e["active"].as_bool().unwrap_or(true),
            })
            .collect(),
        statics: Statics::default(),
        anims: case["anims"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| {
                let priority = match a[1].as_str().unwrap() {
                    "LOW" => Priority::Low,
                    "MID" => Priority::Mid,
                    "HIGH" => Priority::High,
                    _ => Priority::VeryHigh,
                };
                rs09_server::definitions::item::Shared::new(force_movement::Animation {
                    id: int(&a[0]),
                    priority,
                })
            })
            .collect(),
        movements: Vec::new(),
        events: Vec::new(),
        calls: 0,
        fail_at: -1,
    };
    let mut previous = Value::Null;
    let mut differences = Vec::new();
    for (k, (op, expected)) in case["ops"]
        .as_array()
        .unwrap()
        .iter()
        .zip(row["expected"].as_array().unwrap())
        .enumerate()
    {
        r.events.clear();
        r.calls = 0;
        r.fail_at = op["failAt"].as_i64().unwrap_or(-1);
        let outcome = run(&mut r, op);
        r.fail_at = -1;
        let (result, error) = match outcome {
            Ok(v) => (v, Value::Null),
            Err(e) => (Value::Null, json!(error_name(&e))),
        };
        let state = r.state();
        let shown = if state == previous {
            Value::Null
        } else {
            state.clone()
        };
        previous = state;
        let actual = json!({"result": result, "error": error, "events": r.events, "state": shown});
        for key in ["result", "error", "events", "state"] {
            if actual[key] != expected[key] {
                differences.push(format!(
                    "op {k} {key}: expected {} got {}",
                    expected[key], actual[key]
                ));
            }
        }
        if !differences.is_empty() {
            break;
        }
    }
    differences
}

#[test]
fn corpus_matches_provenance() {
    let provenance: Value = serde_json::from_str(PROVENANCE).unwrap();
    let digest = format!("{:x}", Sha256::digest(CORPUS.as_bytes()));
    assert_eq!(provenance["fixtureSha256"], json!(digest));
    assert_eq!(provenance["caseCount"], json!(CORPUS.lines().count()));
}

#[test]
fn every_case_replays() {
    let mut failures: Vec<(String, String)> = Vec::new();
    let mut count = 0;
    for line in CORPUS.lines() {
        let row: Value = serde_json::from_str(line).unwrap();
        count += 1;
        let differences = replay_case(&row);
        if !differences.is_empty() {
            let group = row["input"]["group"].as_str().unwrap().to_string();
            failures.push((
                group,
                format!("{} :: {}", row["input"]["ops"], differences.join(" | ")),
            ));
        }
    }
    let mut unexpected: Vec<&(String, String)> = failures
        .iter()
        .filter(|(g, _)| !GAPS.iter().any(|(gap, _)| gap == g))
        .collect();
    unexpected.truncate(8);
    assert!(
        unexpected.is_empty(),
        "{} of {count} cases differ, first: {unexpected:#?}",
        failures.len()
    );
    for (gap, reason) in GAPS {
        assert!(
            failures.iter().any(|(g, _)| g == gap),
            "gap {gap} ({reason}) is closed"
        );
    }
}

/// A world whose force-move registration is the native update masks (the
/// player provider table: ForceMove presence 0x400, ordinal 5).
struct MaskWorld {
    masks: Masks<usize, ForceMoveCtx>,
    provider: ProviderHandle,
    location: Location,
}

impl update_masks::Registry<usize, ForceMoveCtx> for MaskWorld {
    fn provider(
        &mut self,
        _: &Masks<usize, ForceMoveCtx>,
        _: Kind,
        flag: Flag,
    ) -> update_masks::Result<Option<ProviderHandle>> {
        Ok((flag == Flag::ForceMove).then(|| self.provider.clone()))
    }
    fn missing(
        &mut self,
        _: &Masks<usize, ForceMoveCtx>,
        _: Kind,
        _: Flag,
    ) -> update_masks::Result<()> {
        unreachable!()
    }
    fn now_millis(&mut self, _: &Masks<usize, ForceMoveCtx>) -> update_masks::Result<i64> {
        unreachable!()
    }
}

impl Host for MaskWorld {
    fn is_player(&self, _: Node) -> bool {
        true
    }
    fn location(&mut self, _: Node) -> Result<Option<Location>> {
        Ok(Some(self.location))
    }
    fn animate(&mut self, _: Node, _: Option<&AnimationHandle>) -> Result<bool> {
        Ok(true)
    }
    fn set_disabled_ticks(&mut self, _: Node, _: i32) -> Result<()> {
        Ok(())
    }
    fn register_force_move(&mut self, _: Node, context: ForceMoveCtx) -> Result<bool> {
        let masks = self.masks.clone();
        masks
            .register(self, Flag::ForceMove, Some(context), false)
            .map_err(|e| Error::Binding(format!("{e:?}")))
    }
    fn update_region(&mut self, _: Node, _: Location, _: Location, _: bool) -> Result<bool> {
        Ok(true)
    }
    fn set_teleport_location(&mut self, _: Node, location: Option<Location>) -> Result<()> {
        self.location = location.unwrap();
        Ok(())
    }
    fn lock(&mut self, _: Node) -> Result<()> {
        Ok(())
    }
    fn unlock(&mut self, _: Node) -> Result<()> {
        Ok(())
    }
    fn submit(&mut self, _: &Handle) -> Result<()> {
        Ok(())
    }
    fn node_active(&mut self, _: Node) -> Result<bool> {
        Ok(true)
    }
    fn exception(&mut self, e: &Error) -> Result<()> {
        panic!("{e:?}")
    }
}

#[test]
fn start_registers_the_native_force_move_mask_and_the_pulse_lands_on_the_destination() {
    let provider = Arc::new(Provider {
        revision: 530,
        kind: Kind::Player,
        presence: 0x400,
        ordinal: 5,
        flag: Flag::ForceMove,
    });
    let mut world = MaskWorld {
        masks: Masks::new(0, Kind::Player),
        provider,
        location: location(3200, 3200, 0),
    };
    let statics = Statics::default();
    let fm = force_movement::run_static(
        &mut world,
        &statics,
        Some(0),
        Some(location(3198, 3200, 0)),
        Some(location(3204, 3200, 0)),
        RunForm::Full(None, None, Some(Direction::East), 10, 10, true),
    )
    .unwrap();
    let status = world.masks.status();
    assert_eq!(status.presence, 0x400);
    let snapshot = world.masks.snapshot();
    let element = snapshot.current[5].as_ref().unwrap();
    assert_eq!(
        element.context,
        Some(ForceMoveCtx {
            start: location(3198, 3200, 0),
            dest: location(3204, 3200, 0),
            start_arrive: 60,
            dest_arrive: 180,
            direction: Direction::East
        })
    );
    // Two ticks to reach the start, six along the path: the pulse stops on the eighth update.
    let results: Vec<bool> = (0..8)
        .map(|_| force_movement::update(&mut world, &fm).unwrap())
        .collect();
    assert_eq!(
        results,
        [false, false, false, false, false, false, false, true]
    );
    assert_eq!(world.location, location(3204, 3200, 0));
    assert!(!fm.read().pulse.read().running);
}
