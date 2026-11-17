//! Native task/pulse scheduling. Concrete content, nodes, scripts, clock and
//! telemetry are bindings; the running world lifecycle is separate.
use crate::definitions::item::Shared;
use std::{
    collections::VecDeque,
    sync::{Arc, Condvar, Mutex},
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Null,
    Index,
    ConcurrentModification,
    QueueFull,
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;
pub type Node = usize;
pub type Checks = Shared<Vec<Option<Node>>>;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Class {
    Other,
    Movement,
    Combat,
    Bot { script: Option<String> },
    Unhandled { player: Node },
}
pub struct Pulse {
    pub running: bool,
    pub delay: i32,
    pub ticks_passed: i32,
    pub checks: Option<Checks>,
    pub class: Class,
    pub class_name: String,
}
pub type Handle = Shared<Pulse>;
impl Pulse {
    pub fn new(delay: i32, checks: Option<Checks>, class: Class, class_name: String) -> Self {
        Self {
            running: true,
            delay,
            ticks_passed: 0,
            checks,
            class,
            class_name,
        }
    }
    pub fn restart(&mut self) {
        self.ticks_passed = 0;
    }
    pub fn set_node(&mut self, index: i32, node: Option<Node>) -> Result<()> {
        *self
            .checks
            .as_ref()
            .ok_or(Error::Null)?
            .write()
            .get_mut(index as usize)
            .ok_or(Error::Index)? = node;
        Ok(())
    }
    pub fn node(&self, index: i32) -> Result<Option<Node>> {
        self.checks
            .as_ref()
            .ok_or(Error::Null)?
            .read()
            .get(index as usize)
            .copied()
            .ok_or(Error::Index)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Warn,
    Error,
}
pub fn start_base(pulse: &Handle) {
    pulse.write().running = true;
}
pub fn stop_base(pulse: &Handle) {
    pulse.write().running = false;
}
pub fn is_running_base(pulse: &Handle) -> bool {
    pulse.read().running
}
/// The original runUnhandledAction's anonymous constructor, separated so an
/// owning world can register its captured body before scheduling callbacks run.
pub fn unhandled_pulse(player: Node) -> Handle {
    Shared::new(Pulse::new(
        1,
        Some(Shared::new(vec![Some(player)])),
        Class::Unhandled { player },
        "core.game.node.entity.impl.PulseManager$1".into(),
    ))
}
/// Default virtual hooks are the actual base Pulse implementation. Concrete
/// content can override them and explicitly call the corresponding base helper.
pub trait Context<S> {
    fn pulse(&mut self, pulse: &Handle, state: &mut S) -> Result<bool>;
    fn node_active(&mut self, node: Node, state: &mut S) -> Result<bool>;
    fn exception(&mut self, error: &Error, state: &mut S) -> Result<()>;
    fn start(&mut self, pulse: &Handle, _: &mut S) -> Result<()> {
        start_base(pulse);
        Ok(())
    }
    fn stop(&mut self, pulse: &Handle, _: &mut S) -> Result<()> {
        stop_base(pulse);
        Ok(())
    }
    fn is_running(&mut self, pulse: &Handle, _: &mut S) -> Result<bool> {
        Ok(is_running_base(pulse))
    }
    fn has_inactive(&mut self, pulse: &Handle, state: &mut S) -> Result<bool> {
        has_inactive_base(pulse, state, self)
    }
    fn update(&mut self, pulse: &Handle, state: &mut S) -> Result<bool> {
        update_base(pulse, state, self)
    }
    fn remove_for(&mut self, _: &Handle, _: Option<&str>, _: &mut S) -> Result<bool> {
        Ok(true)
    }
    fn remove_weak_scripts(&mut self, state: &mut S) -> Result<()>;
    fn remove_normal_scripts(&mut self, state: &mut S) -> Result<()>;
    fn is_dead(&mut self, state: &mut S) -> Result<bool>;
    fn send_message(&mut self, player: Node, message: &str, state: &mut S) -> Result<()>;
    fn now_millis(&mut self, state: &mut S) -> Result<i64>;
    fn log(&mut self, level: Level, message: String, state: &mut S) -> Result<()>;
    fn pulse_length(&mut self, class: &str, millis: i32, state: &mut S) -> Result<()>;
    fn count_pulse(&mut self, class: &str, state: &mut S) -> Result<()>;
    fn metrics_enabled(&self, state: &S) -> bool;
    fn set_bot_total(&mut self, millis: i32, state: &mut S) -> Result<()>;
    fn set_other_total(&mut self, millis: i32, state: &mut S) -> Result<()>;
}
pub fn has_inactive_base<S, C: Context<S> + ?Sized>(
    pulse: &Handle,
    state: &mut S,
    ctx: &mut C,
) -> Result<bool> {
    let Some(checks) = pulse.read().checks.clone() else {
        return Ok(false);
    };
    let size = checks.read().len();
    for index in 0..size {
        // The original reloads the checks field on every loop iteration.
        let checks = pulse.read().checks.clone().ok_or(Error::Null)?;
        let node = checks.read().get(index).copied().ok_or(Error::Index)?;
        if let Some(node) = node {
            if !ctx.node_active(node, state)? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
pub fn update_base<S, C: Context<S> + ?Sized>(
    pulse: &Handle,
    state: &mut S,
    ctx: &mut C,
) -> Result<bool> {
    if ctx.has_inactive(pulse, state)? {
        ctx.stop(pulse, state)?;
        return Ok(true);
    }
    if !ctx.is_running(pulse, state)? {
        return Ok(true);
    }
    let due = {
        let mut p = pulse.write();
        p.ticks_passed = p.ticks_passed.wrapping_add(1);
        p.ticks_passed >= p.delay
    };
    if due {
        pulse.write().ticks_passed = 0;
        let result = (|| {
            if ctx.pulse(pulse, state)? {
                ctx.stop(pulse, state)?;
                return Ok(true);
            }
            Ok(false)
        })();
        match result {
            Ok(true) => return Ok(true),
            Err(error) => {
                ctx.exception(&error, state)?;
                ctx.stop(pulse, state)?;
                return Ok(true);
            }
            Ok(false) => {}
        }
        return Ok(!ctx.is_running(pulse, state)?);
    }
    Ok(false)
}
pub fn run_base<S>(pulse: &Handle, state: &mut S, ctx: &mut impl Context<S>) -> Result<()> {
    ctx.update(pulse, state)?;
    Ok(())
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Standard,
    Strong,
    Custom1,
    Custom2,
}
impl Kind {
    pub const ALL: [Self; 4] = [Self::Standard, Self::Strong, Self::Custom1, Self::Custom2];
    fn index(self) -> usize {
        match self {
            Self::Standard => 0,
            Self::Strong => 1,
            Self::Custom1 => 2,
            Self::Custom2 => 3,
        }
    }
}
struct Entry {
    key: Option<Kind>,
    value: Option<Handle>,
    next: Option<Shared<Entry>>,
}
struct Slots {
    heads: [Option<Shared<Entry>>; 16],
    hashes: [i32; 4],
    version: u32,
    size: usize,
}
impl Slots {
    fn bucket(&self, key: Option<Kind>) -> usize {
        let hash = key.map_or(0, |k| self.hashes[k.index()]) as u32;
        ((hash ^ (hash >> 16)) & 15) as usize
    }
    fn get(&self, key: Option<Kind>) -> Option<Handle> {
        let mut node = self.heads[self.bucket(key)].clone();
        while let Some(entry) = node {
            let entry = entry.read();
            if entry.key == key {
                return entry.value.clone();
            }
            node = entry.next.clone()
        }
        None
    }
    fn put(&mut self, key: Option<Kind>, value: Option<Handle>) {
        let bucket = self.bucket(key);
        let mut cursor = self.heads[bucket].clone();
        let mut last = None;
        while let Some(node) = cursor {
            if node.read().key == key {
                node.write().value = value;
                return;
            }
            cursor = node.read().next.clone();
            last = Some(node);
        }
        let node = Shared::new(Entry {
            key,
            value,
            next: None,
        });
        if let Some(last) = last {
            last.write().next = Some(node)
        } else {
            self.heads[bucket] = Some(node)
        }
        self.version = self.version.wrapping_add(1);
        self.size += 1;
    }
    fn remove(&mut self, key: Option<Kind>) {
        let bucket = self.bucket(key);
        let mut cursor = self.heads[bucket].clone();
        let mut previous: Option<Shared<Entry>> = None;
        while let Some(node) = cursor {
            let next = node.read().next.clone();
            if node.read().key == key {
                if let Some(previous) = previous {
                    previous.write().next = next
                } else {
                    self.heads[bucket] = next
                }
                self.version = self.version.wrapping_add(1);
                self.size -= 1;
                return;
            }
            previous = Some(node);
            cursor = next;
        }
    }
}
#[derive(Clone)]
pub struct Manager {
    slots: Shared<Slots>,
}
impl Manager {
    /// Native owner accounting for the manager's shared slot map. This is not
    /// Java's currentPulses.size or an observable gameplay property.
    pub fn strong_count(&self) -> usize {
        self.slots.strong_count()
    }
    /// Enum identity hashes are an explicit world/replay input: Java's enum
    /// HashMap iteration order is not declaration or insertion order.
    pub fn new(enum_hashes: [i32; 4]) -> Self {
        Self {
            slots: Shared::new(Slots {
                heads: std::array::from_fn(|_| None),
                hashes: enum_hashes,
                version: 0,
                size: 0,
            }),
        }
    }
    pub fn put(&self, key: Option<Kind>, pulse: Option<Handle>) {
        self.slots.write().put(key, pulse)
    }
    pub fn get(&self, key: Option<Kind>) -> Option<Handle> {
        self.slots.read().get(key)
    }
    pub fn entries(&self) -> Vec<(Option<Kind>, Option<Handle>)> {
        let mut result = Vec::new();
        let slots = self.slots.read();
        for head in &slots.heads {
            let mut cursor = head.clone();
            while let Some(node) = cursor {
                let node = node.read();
                result.push((node.key, node.value.clone()));
                cursor = node.next.clone()
            }
        }
        result
    }
    fn for_each(
        &self,
        mut f: impl FnMut(Option<Kind>, Option<Handle>) -> Result<()>,
    ) -> Result<()> {
        if self.slots.read().size == 0 {
            return Ok(());
        }
        let version = self.slots.read().version;
        for bucket in 0..16 {
            let mut cursor = self.slots.read().heads[bucket].clone();
            while let Some(node) = cursor {
                let (key, value) = {
                    let node = node.read();
                    (node.key, node.value.clone())
                };
                f(key, value)?;
                cursor = node.read().next.clone();
            }
        }
        if self.slots.read().version != version {
            return Err(Error::ConcurrentModification);
        }
        Ok(())
    }
    pub fn current(&self) -> Option<Handle> {
        Kind::ALL.into_iter().find_map(|kind| self.get(Some(kind)))
    }
    pub fn has_running<S>(&self, state: &mut S, ctx: &mut impl Context<S>) -> Result<bool> {
        match self.current() {
            Some(pulse) => ctx.is_running(&pulse, state),
            None => Ok(false),
        }
    }
    pub fn is_moving<S>(&self, state: &mut S, ctx: &mut impl Context<S>) -> Result<bool> {
        if !self.has_running(state, ctx)? {
            return Ok(false);
        }
        Ok(self
            .current()
            .is_some_and(|p| matches!(p.read().class, Class::Movement | Class::Combat)))
    }
    pub fn clear<S>(&self, state: &mut S, ctx: &mut impl Context<S>) -> Result<()> {
        ctx.remove_weak_scripts(state)?;
        ctx.remove_normal_scripts(state)?;
        self.for_each(|kind, pulse| {
            if kind != Some(Kind::Strong) {
                if let Some(pulse) = pulse {
                    ctx.stop(&pulse, state)?
                }
            }
            Ok(())
        })
    }
    pub fn clear_kind<S>(
        &self,
        kind: Option<Kind>,
        state: &mut S,
        ctx: &mut impl Context<S>,
    ) -> Result<bool> {
        ctx.remove_weak_scripts(state)?;
        ctx.remove_normal_scripts(state)?;
        if let Some(pulse) = self.get(kind) {
            ctx.stop(&pulse, state)?;
            self.slots.write().remove(kind)
        }
        Ok(true)
    }
    pub fn run<S>(
        &self,
        pulse: Option<&Handle>,
        kind: Option<Kind>,
        runner: &Runner,
        state: &mut S,
        ctx: &mut impl Context<S>,
    ) -> Result<()> {
        let mut remove = Vec::new();
        self.for_each(|key, pulse| {
            if let Some(pulse) = pulse {
                if !ctx.is_running(&pulse, state)? {
                    remove.push(key)
                }
            }
            Ok(())
        })?;
        for key in remove {
            self.slots.write().remove(key)
        }
        if self.get(Some(Kind::Strong)).is_some() {
            return Ok(());
        }
        if !self.clear_kind(kind, state, ctx)? {
            return Ok(());
        }
        if kind == Some(Kind::Strong) {
            self.clear(state, ctx)?
        }
        self.put(kind, pulse.cloned());
        let pulse = pulse.ok_or(Error::Null)?;
        ctx.start(pulse, state)?;
        if ctx.is_running(pulse, state)? {
            runner.submit(Some(pulse.clone()))?
        }
        Ok(())
    }
    pub fn cancel_death<S>(&self, state: &mut S, ctx: &mut impl Context<S>) -> Result<()> {
        if !ctx.is_dead(state)? {
            return Ok(());
        }
        if self.current().is_none() {
            return Ok(());
        }
        ctx.stop(&self.current().ok_or(Error::Null)?, state)
    }
    pub fn unhandled<S>(
        &self,
        player: Node,
        kind: Option<Kind>,
        runner: &Runner,
        state: &mut S,
        ctx: &mut impl Context<S>,
    ) -> Result<Handle> {
        let pulse = unhandled_pulse(player);
        self.run(Some(&pulse), kind, runner, state, ctx)?;
        Ok(pulse)
    }
}
#[derive(Default)]
struct Queue {
    values: Mutex<VecDeque<Handle>>,
    available: Condvar,
}
#[derive(Clone, Default)]
pub struct Runner {
    queue: Arc<Queue>,
}
impl Runner {
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.queue, &other.queue)
    }
    pub fn current(&self) -> Vec<Handle> {
        self.queue
            .values
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .cloned()
            .collect()
    }
    pub fn submit(&self, pulse: Option<Handle>) -> Result<()> {
        let pulse = pulse.ok_or(Error::Null)?;
        let mut queue = self.queue.values.lock().unwrap_or_else(|e| e.into_inner());
        if queue.len() >= i32::MAX as usize {
            return Err(Error::QueueFull);
        }
        queue.push_back(pulse);
        self.queue.available.notify_one();
        Ok(())
    }
    fn take(&self) -> Handle {
        let mut queue = self.queue.values.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if let Some(pulse) = queue.pop_front() {
                return pulse;
            }
            queue = self
                .queue
                .available
                .wait(queue)
                .unwrap_or_else(|e| e.into_inner());
        }
    }
    pub fn update_all<S>(&self, state: &mut S, ctx: &mut impl Context<S>) -> Result<()> {
        let count = self
            .queue
            .values
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len();
        let (mut bots, mut others) = (0i32, 0i32);
        for _ in 0..count {
            let pulse = self.take();
            let start = ctx.now_millis(state)?;
            let result = (|| {
                if !ctx.update(&pulse, state)? && ctx.is_running(&pulse, state)? {
                    self.submit(Some(pulse.clone()))?
                }
                Ok(())
            })();
            if let Err(error) = result {
                ctx.log(
                    Level::Error,
                    "Pulse execution error. Stack trace below.".into(),
                    state,
                )?;
                ctx.exception(&error, state)?;
            }
            let elapsed = ctx.now_millis(state)?.wrapping_sub(start);
            let (name, class) = {
                let pulse = pulse.read();
                (pulse.class_name.clone(), pulse.class.clone())
            };
            if matches!(class, Class::Bot { .. }) || name.contains("ScriptAPI") {
                bots = bots.wrapping_add(elapsed as i32)
            } else {
                others = others.wrapping_add(elapsed as i32)
            }
            ctx.pulse_length(&name, elapsed as i32, state)?;
            ctx.count_pulse(&name, state)?;
            if elapsed >= 30 {
                let critical = if elapsed >= 100 {
                    "CRITICALLY long"
                } else {
                    "Long"
                };
                let message = match class {
                    Class::Bot { script } => format!(
                        "{critical} bot-script tick - {} took {elapsed} ms",
                        script.ok_or(Error::Null)?
                    ),
                    _ => format!("{critical} running pulse - {name} took {elapsed} ms"),
                };
                ctx.log(Level::Warn, message, state)?;
            }
        }
        if ctx.metrics_enabled(state) {
            ctx.set_bot_total(bots, state)?;
            ctx.set_other_total(others, state)?
        }
        Ok(())
    }
}
