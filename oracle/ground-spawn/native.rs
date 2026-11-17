//! Joined test bindings; production subtype, manager and Runner execute here.
use super::*;
use rs09_server::{pulses, world::ground_spawn as gs};

pub(super) struct Extra {
    pub registry: gs::Registry,
    pub values: Vec<gs::Handle>,
    pub pulses: Vec<pulses::Handle>,
    pub runners: [pulses::Runner; 2],
    pub current: usize,
    pub maximum: i32,
    pub population: i32,
}
impl Default for Extra {
    fn default() -> Self {
        Self {
            registry: gs::Registry::default(),
            values: vec![],
            pulses: vec![],
            runners: std::array::from_fn(|_| pulses::Runner::default()),
            current: 0,
            maximum: 2000,
            population: 0,
        }
    }
}
pub(super) fn schedule(
    m: &mut Model,
    spawn: &gs::Handle,
) -> rs09_server::world::ground_manager::Result<()> {
    let registry = m.spawns.registry.clone();
    let base = registry.respawn(m, spawn)?;
    m.spawns.pulses.push(base);
    Ok(())
}
pub(super) fn act(m: &mut Model, a: &Value) -> Result<Value> {
    let registry = m.spawns.registry.clone();
    let manager = m.extra.manager.clone();
    let spawn = || {
        m.spawns
            .values
            .get(n(a, "spawn", 0) as usize)
            .cloned()
            .ok_or_else(|| Error::Binding("Index".into()))
    };
    match a["op"].as_str().unwrap() {
        "spawnNew" => {
            let input = super::manager::raw(m, a)?.ok_or(Error::Null)?;
            let h = registry
                .construct(m, n(a, "rate", 3), &input, loc(&a["location"]))
                .map_err(Error::Ground)?;
            let ground = gs::as_ground(&h);
            let id = m.gid(&ground);
            m.grounds[id] = ground;
            m.spawns.values.push(h);
            return Ok(json!(id));
        }
        "spawnInit" => {
            let h = spawn()?;
            return gs::init(m, &manager, &h)
                .map(|g| json!(m.gid(&g)))
                .map_err(super::manager::error);
        }
        "spawnRate" => gs::set_respawn_rate(&spawn()?, n(a, "minimum", 1), n(a, "maximum", 2)),
        "spawnDuration" => {
            let h = spawn()?;
            return gs::respawn_duration(m, &h)
                .map(|v| json!(v))
                .map_err(super::manager::error);
        }
        "spawnRespawn" => {
            let h = spawn()?;
            schedule(m, &h).map_err(super::manager::error)?;
        }
        "spawnConfig" => {
            if let Some(value) = a.get("maximum") {
                m.spawns.maximum = num(value);
            }
            if let Some(value) = a.get("population") {
                m.spawns.population = num(value);
            }
        }
        "spawnRunner" => m.spawns.current = n(a, "index", 0) as usize,
        "spawnWorld" => {
            let runner = m.spawns.runners[n(a, "index", 0) as usize].clone();
            runner
                .update_all(&mut (), m)
                .map_err(|e| Error::Binding(format!("{e:?}")))?;
        }
        "spawnPulse" => {
            let base = m.spawns.pulses[n(a, "pulse", 0) as usize].clone();
            return registry
                .pulse(m, &manager, &base)
                .map(|v| json!(v))
                .map_err(super::manager::error);
        }
        _ => return super::loader::act(m, a),
    }
    Ok(Value::Null)
}
pub(super) fn snapshot(m: &Model, state: &mut Value) {
    let data = &m.spawns;
    state["spawns"] = json!(
        data.values
            .iter()
            .map(|s| json!([m.gid(&gs::as_ground(s)), s.read().respawn_rate]))
            .collect::<Vec<_>>()
    );
    state["spawnPulses"] = json!(
        data.pulses
            .iter()
            .map(|p| {
                let p = p.read();
                json!([
                    p.delay,
                    p.ticks_passed,
                    p.running,
                    p.checks.is_none(),
                    p.class_name
                ])
            })
            .collect::<Vec<_>>()
    );
    state["spawnQueues"] = json!(
        data.runners
            .iter()
            .map(|r| r
                .current()
                .iter()
                .map(|p| data.pulses.iter().position(|known| known.same(p)).unwrap())
                .collect::<Vec<_>>())
            .collect::<Vec<_>>()
    );
    state["population"] = json!([data.maximum, data.population, data.current as i32]);
}
fn binding(e: Error) -> rs09_server::world::ground_manager::Error {
    match e {
        Error::Null => rs09_server::world::ground_manager::Error::Null,
        Error::Ground(e) => rs09_server::world::ground_manager::Error::Ground(e),
        Error::Binding(s) => rs09_server::world::ground_manager::Error::Binding(s),
    }
}
impl gs::Context for Model {
    fn max_players(&mut self) -> rs09_server::world::ground_manager::Result<i32> {
        let v = self.spawns.maximum;
        self.emit("maximum", vec![json!(v)]).map_err(binding)?;
        Ok(v)
    }
    fn player_count(&mut self) -> rs09_server::world::ground_manager::Result<i32> {
        let v = self.spawns.population;
        self.emit("population", vec![json!(v)]).map_err(binding)?;
        Ok(v)
    }
    fn pulser(&mut self) -> rs09_server::world::ground_manager::Result<pulses::Runner> {
        let id = self.spawns.current;
        let runner = self.spawns.runners[id].clone();
        self.emit("pulser", vec![json!(id)]).map_err(binding)?;
        Ok(runner)
    }
}
impl pulses::Context<()> for Model {
    fn pulse(&mut self, p: &pulses::Handle, _: &mut ()) -> pulses::Result<bool> {
        let registry = self.spawns.registry.clone();
        let manager = self.extra.manager.clone();
        registry
            .pulse(self, &manager, p)
            .map_err(|e| pulses::Error::Binding(format!("{e:?}")))
    }
    fn node_active(&mut self, _: usize, _: &mut ()) -> pulses::Result<bool> {
        Ok(true)
    }
    fn exception(&mut self, _: &pulses::Error, _: &mut ()) -> pulses::Result<()> {
        Ok(())
    }
    fn remove_weak_scripts(&mut self, _: &mut ()) -> pulses::Result<()> {
        Ok(())
    }
    fn remove_normal_scripts(&mut self, _: &mut ()) -> pulses::Result<()> {
        Ok(())
    }
    fn is_dead(&mut self, _: &mut ()) -> pulses::Result<bool> {
        Ok(false)
    }
    fn send_message(&mut self, _: usize, _: &str, _: &mut ()) -> pulses::Result<()> {
        Ok(())
    }
    fn now_millis(&mut self, _: &mut ()) -> pulses::Result<i64> {
        Ok(0)
    }
    fn log(&mut self, level: pulses::Level, message: String, _: &mut ()) -> pulses::Result<()> {
        self.emit(
            "log",
            vec![
                json!(if level == pulses::Level::Error { 1 } else { 2 }),
                json!(message),
            ],
        )
        .map_err(|e| pulses::Error::Binding(err(e)))
    }
    fn pulse_length(&mut self, _: &str, _: i32, _: &mut ()) -> pulses::Result<()> {
        Ok(())
    }
    fn count_pulse(&mut self, _: &str, _: &mut ()) -> pulses::Result<()> {
        Ok(())
    }
    fn metrics_enabled(&self, _: &()) -> bool {
        false
    }
    fn set_bot_total(&mut self, _: i32, _: &mut ()) -> pulses::Result<()> {
        Ok(())
    }
    fn set_other_total(&mut self, _: i32, _: &mut ()) -> pulses::Result<()> {
        Ok(())
    }
}

#[test]
fn queued_spawn_and_exposed_views_retain_subtype_until_the_final_owner_drops() {
    let mut m = super::model(&json!({"manager":true,"spawns":true,"actions":[]}));
    let registry = m.spawns.registry.clone();
    let input = super::manager::raw(&mut m, &json!({})).unwrap().unwrap();
    let spawn = registry
        .construct(&mut m, 3, &input, Some(Location::new(3201, 3202, 0)))
        .unwrap();
    let weak = spawn.downgrade();
    let base = registry.respawn(&mut m, &spawn).unwrap();
    let runner = m.spawns.runners[0].clone();
    assert!(registry.owns_pulse(&base));
    assert!(!gs::Registry::default().owns_pulse(&base));
    m.grounds.clear();
    drop(spawn);
    assert!(
        weak.upgrade().is_some(),
        "queued/exposed pulse retains the captured spawn"
    );
    let ground = gs::as_ground(&weak.upgrade().unwrap());
    m.grounds.push(ground.clone());
    for _ in 0..3 {
        runner.update_all(&mut (), &mut m).unwrap();
    }
    assert!(runner.current().is_empty());
    assert_eq!(m.extra.manager.get_items().read().len(), 1);
    assert!(
        m.extra
            .manager
            .get_items()
            .read()
            .get(0)
            .unwrap()
            .as_ref()
            .unwrap()
            .same(&ground)
    );
    let item_view = ground::as_item(&ground);
    m.extra.manager.get_items().write().clear();
    for c in &m.chunks {
        sync::get_items(c).write().clear();
        c.read().update_flags.write().clear();
    }
    m.flags.clear();
    m.dirty.reset_all();
    m.grounds.clear();
    drop(ground);
    drop(base);
    registry.collect();
    assert_eq!(registry.retained_task_count(), 0);
    assert!(
        weak.upgrade().is_some(),
        "an exposed Item still retains its subtype"
    );
    drop(item_view);
    registry.collect();
    assert!(
        weak.upgrade().is_none(),
        "weak subtype/task metadata does not leak the spawn"
    );
}
