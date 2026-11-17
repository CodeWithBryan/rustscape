//! Entity timer state/ordering. Content implementations and the world clock are
//! explicit bindings; normal gameplay never invokes Kotlin or reflection here.
use crate::definitions::{
    item::Shared,
    item_config::json::{self, Json},
};
use serde::{Deserialize, Serialize};
pub mod registry;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Null,
    Type,
    Number,
    Index,
    ConcurrentModification,
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Flag {
    ClearOnDeath,
}
#[derive(Debug)]
pub struct Timer {
    pub identifier: String,
    pub class_name: String,
    pub run_interval: i32,
    pub is_soft: bool,
    pub is_auto: bool,
    pub flags: Vec<Flag>,
    pub persistent: bool,
    pub last_execution: i32,
    pub next_execution: i32,
}
impl Timer {
    pub fn new(interval: i32, identifier: String, class_name: String, persistent: bool) -> Self {
        Self {
            identifier,
            class_name,
            run_interval: interval,
            is_soft: false,
            is_auto: false,
            flags: Vec::new(),
            persistent,
            last_execution: 0,
            next_execution: 0,
        }
    }
}
pub type Handle = Shared<Timer>;
#[derive(Default)]
pub struct List {
    values: Vec<Option<Handle>>,
    version: u32,
}
impl List {
    pub fn values(&self) -> &[Option<Handle>] {
        &self.values
    }
    pub fn push(&mut self, value: Option<Handle>) {
        self.values.push(value);
        self.version = self.version.wrapping_add(1);
    }
    pub fn clear(&mut self) {
        self.values.clear();
        self.version = self.version.wrapping_add(1);
    }
    pub fn set(&mut self, index: usize, value: Option<Handle>) -> Result<Option<Handle>> {
        Ok(std::mem::replace(
            self.values.get_mut(index).ok_or(Error::Index)?,
            value,
        ))
    }
    pub fn remove(&mut self, index: usize) -> Result<Option<Handle>> {
        if index >= self.values.len() {
            return Err(Error::Index);
        }
        self.version = self.version.wrapping_add(1);
        Ok(self.values.remove(index))
    }
    pub fn contains(&self, value: &Option<Handle>) -> bool {
        self.values.iter().any(|entry| same(entry, value))
    }
    fn remove_all(&mut self, other: &[Option<Handle>]) {
        let old = self.values.len();
        self.values
            .retain(|entry| !other.iter().any(|value| same(entry, value)));
        self.version = self.version.wrapping_add((old - self.values.len()) as u32);
    }
}
fn same(left: &Option<Handle>, right: &Option<Handle>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.same(right),
        (None, None) => true,
        _ => false,
    }
}
pub type TimerList = Shared<List>;
fn next(list: &TimerList, cursor: &mut usize, version: u32) -> Result<Option<Option<Handle>>> {
    let list = list.read();
    if *cursor == list.values.len() {
        return Ok(None);
    }
    if list.version != version {
        return Err(Error::ConcurrentModification);
    }
    let value = list.values.get(*cursor).cloned().ok_or(Error::Index)?;
    *cursor += 1;
    Ok(Some(value))
}
pub struct Manager {
    pub active: TimerList,
    pub pending: TimerList,
    pub removing: TimerList,
}
impl Default for Manager {
    fn default() -> Self {
        Self {
            active: Shared::new(List::default()),
            pending: Shared::new(List::default()),
            removing: Shared::new(List::default()),
        }
    }
}
/// S is the owning entity's state, separate from its manager so callbacks can
/// mutate both without replacing a live manager with a temporary placeholder.
pub trait Context<S> {
    fn is_player(&self, state: &S) -> bool;
    fn modal_open(&mut self, state: &mut S) -> Result<bool>;
    fn script_delay(&mut self, state: &mut S) -> Result<i32>;
    fn ticks(&mut self, state: &mut S) -> Result<i32>;
    fn entity_name(&mut self, state: &mut S) -> Result<Option<String>>;
    fn before_register(
        &mut self,
        timer: &Handle,
        manager: &mut Manager,
        state: &mut S,
    ) -> Result<()>;
    fn on_register(&mut self, timer: &Handle, manager: &mut Manager, state: &mut S) -> Result<()>;
    fn on_removal(&mut self, timer: &Handle, manager: &mut Manager, state: &mut S) -> Result<()>;
    fn run(&mut self, timer: &Handle, manager: &mut Manager, state: &mut S) -> Result<bool>;
    fn initial_delay(
        &mut self,
        timer: &Handle,
        manager: &mut Manager,
        state: &mut S,
    ) -> Result<i32>;
    fn save(
        &mut self,
        timer: &Handle,
        root: &mut Json,
        manager: &mut Manager,
        state: &mut S,
    ) -> Result<()>;
    fn parse(
        &mut self,
        timer: &Handle,
        data: &Json,
        manager: &mut Manager,
        state: &mut S,
    ) -> Result<()>;
    fn resolve(
        &mut self,
        identifier: &str,
        manager: &mut Manager,
        state: &mut S,
    ) -> Result<Option<Handle>>;
    fn run_failure(
        &mut self,
        class_name: &str,
        entity_name: Option<&str>,
        state: &mut S,
    ) -> Result<()>;
    fn unknown_timer(&mut self, identifier: &str, state: &mut S) -> Result<()>;
    fn exception(&mut self, error: &Error, state: &mut S) -> Result<()>;
}
impl Manager {
    pub fn register<S>(
        &mut self,
        timer: Option<&Handle>,
        state: &mut S,
        ctx: &mut impl Context<S>,
    ) -> Result<()> {
        let timer = timer.ok_or(Error::Null)?;
        ctx.before_register(timer, self, state)?;
        self.pending.write().push(Some(timer.clone()));
        ctx.on_register(timer, self, state)
    }
    pub fn register_optional<S>(
        &mut self,
        timer: Option<&Handle>,
        state: &mut S,
        ctx: &mut impl Context<S>,
    ) -> Result<()> {
        if timer.is_some() {
            self.register(timer, state, ctx)?;
        }
        Ok(())
    }
    pub fn remove<S>(
        &mut self,
        timer: Option<&Handle>,
        state: &mut S,
        ctx: &mut impl Context<S>,
    ) -> Result<()> {
        let timer = timer.ok_or(Error::Null)?;
        timer.write().next_execution = i32::MAX;
        self.removing.write().push(Some(timer.clone()));
        if let Err(error) = ctx.on_removal(timer, self, state) {
            ctx.exception(&error, state)?;
        }
        Ok(())
    }
    pub fn clear(&mut self) {
        self.active.write().clear();
        self.pending.write().clear();
        self.removing.write().clear();
    }
    pub fn process<S>(&mut self, state: &mut S, ctx: &mut impl Context<S>) -> Result<()> {
        let removing = self.removing.read().values.clone();
        self.active.write().remove_all(&removing);
        self.pending.write().remove_all(&removing);
        self.removing.write().clear();
        let normal = if ctx.is_player(state) {
            if ctx.modal_open(state)? {
                false
            } else {
                let delay = ctx.script_delay(state)?;
                delay <= ctx.ticks(state)?
            }
        } else {
            true
        };
        let list = self.active.clone();
        let version = list.read().version;
        let mut cursor = 0;
        while let Some(timer) = next(&list, &mut cursor, version)? {
            let timer = timer.ok_or(Error::Null)?;
            let execution = timer.read().next_execution;
            if execution > ctx.ticks(state)? {
                continue;
            }
            if !normal && !timer.read().is_soft {
                continue;
            }
            let result = (|| {
                if ctx.run(&timer, self, state)? {
                    let ticks = ctx.ticks(state)?;
                    let interval = timer.read().run_interval;
                    timer.write().next_execution = ticks.wrapping_add(interval);
                } else {
                    self.remove(Some(&timer), state, ctx)?;
                }
                Ok(())
            })();
            if let Err(error) = result {
                let class = timer.read().class_name.clone();
                let name = ctx.entity_name(state)?;
                ctx.run_failure(&class, name.as_deref(), state)?;
                ctx.exception(&error, state)?;
                self.remove(Some(&timer), state, ctx)?;
            }
        }
        let list = self.pending.clone();
        let version = list.read().version;
        let mut cursor = 0;
        while let Some(timer) = next(&list, &mut cursor, version)? {
            self.active.write().push(timer.clone());
            let timer = timer.ok_or(Error::Null)?;
            let delay = ctx.initial_delay(&timer, self, state)?;
            let ticks = ctx.ticks(state)?;
            timer.write().next_execution = delay.wrapping_add(ticks);
        }
        self.pending.write().clear();
        Ok(())
    }
    pub fn death<S>(&mut self, state: &mut S, ctx: &mut impl Context<S>) -> Result<()> {
        let list = self.active.clone();
        let version = list.read().version;
        let mut cursor = 0;
        while let Some(timer) = next(&list, &mut cursor, version)? {
            let timer = timer.ok_or(Error::Null)?;
            let clear = timer.read().flags.contains(&Flag::ClearOnDeath);
            if clear {
                self.remove(Some(&timer), state, ctx)?;
            }
        }
        Ok(())
    }
    fn find(
        &self,
        mut predicate: impl FnMut(&Timer) -> bool,
        null_fails: bool,
    ) -> Result<Option<Handle>> {
        let mut found = None;
        for list in [&self.active, &self.pending] {
            let version = list.read().version;
            let mut cursor = 0;
            while let Some(timer) = next(list, &mut cursor, version)? {
                let Some(timer) = timer else {
                    if null_fails {
                        return Err(Error::Null);
                    }
                    continue;
                };
                if predicate(&timer.read()) {
                    found = Some(timer);
                }
            }
        }
        if found
            .as_ref()
            .is_some_and(|timer| self.removing.read().contains(&Some(timer.clone())))
        {
            return Ok(None);
        }
        Ok(found)
    }
    pub fn get_identifier(&self, identifier: &str) -> Result<Option<Handle>> {
        self.find(|timer| timer.identifier == identifier, true)
    }
    pub fn get_where(&self, predicate: impl FnMut(&Timer) -> bool) -> Result<Option<Handle>> {
        self.find(predicate, false)
    }
    fn remove_matching<S>(
        &mut self,
        mut predicate: impl FnMut(&Timer) -> bool,
        null_fails: bool,
        state: &mut S,
        ctx: &mut impl Context<S>,
    ) -> Result<()> {
        for list in [self.active.clone(), self.pending.clone()] {
            let version = list.read().version;
            let mut cursor = 0;
            while let Some(timer) = next(&list, &mut cursor, version)? {
                let Some(timer) = timer else {
                    if null_fails {
                        return Err(Error::Null);
                    }
                    continue;
                };
                let matches = predicate(&timer.read());
                if matches {
                    self.remove(Some(&timer), state, ctx)?;
                }
            }
        }
        Ok(())
    }
    pub fn remove_identifier<S>(
        &mut self,
        identifier: &str,
        state: &mut S,
        ctx: &mut impl Context<S>,
    ) -> Result<()> {
        self.remove_matching(|timer| timer.identifier == identifier, true, state, ctx)
    }
    pub fn remove_where<S>(
        &mut self,
        predicate: impl FnMut(&Timer) -> bool,
        state: &mut S,
        ctx: &mut impl Context<S>,
    ) -> Result<()> {
        self.remove_matching(predicate, false, state, ctx)
    }
    pub fn save<S>(
        &mut self,
        root: &mut Json,
        state: &mut S,
        ctx: &mut impl Context<S>,
    ) -> Result<()> {
        if !matches!(root, Json::Object(_)) {
            return Err(if matches!(root, Json::Null) {
                Error::Null
            } else {
                Error::Type
            });
        }
        for list in [self.active.clone(), self.pending.clone()] {
            let version = list.read().version;
            let mut cursor = 0;
            while let Some(timer) = next(&list, &mut cursor, version)? {
                let Some(timer) = timer.filter(|timer| timer.read().persistent) else {
                    continue;
                };
                let mut value = Json::object();
                ctx.save(&timer, &mut value, self, state)?;
                let id = timer.read().identifier.clone();
                root.put(id, value).map_err(|_| Error::Type)?;
            }
        }
        Ok(())
    }
    pub fn parse<S>(
        &mut self,
        root: &Json,
        state: &mut S,
        ctx: &mut impl Context<S>,
    ) -> Result<()> {
        let Json::Object(entries) = root else {
            return Err(if matches!(root, Json::Null) {
                Error::Null
            } else {
                Error::Type
            });
        };
        for index in json::key_order(entries) {
            let (id, data) = &entries[index];
            if !matches!(data, Json::Object(_)) {
                return Err(if matches!(data, Json::Null) {
                    Error::Null
                } else {
                    Error::Type
                });
            }
            let timer = ctx
                .resolve(id, self, state)?
                .filter(|timer| timer.read().persistent);
            let Some(timer) = timer else {
                ctx.unknown_timer(id, state)?;
                continue;
            };
            ctx.parse(&timer, data, self, state)?;
            self.register_optional(Some(&timer), state, ctx)?;
        }
        Ok(())
    }
}
pub fn save_base<S>(
    timer: &Handle,
    root: &mut Json,
    state: &mut S,
    ctx: &mut impl Context<S>,
) -> Result<()> {
    if !matches!(root, Json::Object(_)) {
        return Err(if matches!(root, Json::Null) {
            Error::Null
        } else {
            Error::Type
        });
    }
    let execution = timer.read().next_execution;
    let tick = ctx.ticks(state)?;
    root.put(
        "ticksLeft",
        Json::Text(execution.wrapping_sub(tick).to_string()),
    )
    .map_err(|_| Error::Type)?;
    Ok(())
}
pub fn parse_base(timer: &Handle, root: &Json) -> Result<()> {
    let text = root.get("ticksLeft").unwrap_or(&Json::Null).text();
    let interval = rs09_game_core::java_string::parse_i32(&text).ok_or(Error::Number)?;
    timer.write().run_interval = interval;
    Ok(())
}
