//! Registered native content factories and automatic entity timer installation.
use super::{Context, Error, Handle, List, Manager, Result, Timer, TimerList, next};
use crate::definitions::item::Shared;
use rs09_game_core::{java_lower::lowercase_root, java_map::StringKeyOrder};
use std::collections::BTreeMap;
#[derive(Default)]
struct Map {
    keys: StringKeyOrder,
    values: BTreeMap<String, Option<Handle>>,
}
#[derive(Clone)]
pub struct Registry {
    map: Shared<Map>,
    pub automatic: TimerList,
}
impl Default for Registry {
    fn default() -> Self {
        Self {
            map: Shared::new(Map::default()),
            automatic: Shared::new(List::default()),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Warn,
    Error,
}
/// Concrete native timer types supply their constructor/argument and subtype
/// behavior. No reflection or Java objects are used in the runtime.
pub trait Factories<S>: Context<S> {
    type Argument;
    fn instantiate(
        &mut self,
        prototype: &Handle,
        args: Option<&[Self::Argument]>,
        registry: &Registry,
        manager: &mut Manager,
        state: &mut S,
    ) -> Result<Option<Handle>>;
    fn registry_log(
        &mut self,
        level: Level,
        message: String,
        registry: &Registry,
        manager: &mut Manager,
        state: &mut S,
    ) -> Result<()>;
}
impl Registry {
    /// Exact-key insertion models the original public map's put operation.
    /// Registration below applies ROOT lowercasing before using this map.
    pub fn put(&self, key: String, value: Option<Handle>) -> Option<Option<Handle>> {
        let mut map = self.map.write();
        map.keys.insert(key.clone());
        map.values.insert(key, value)
    }
    pub fn entries(&self) -> Vec<(String, Option<Handle>)> {
        let map = self.map.read();
        map.keys
            .iter()
            .map(|(_, key)| (key.to_owned(), map.values[key].clone()))
            .collect()
    }
    pub fn register<S>(
        &self,
        timer: Option<&Handle>,
        manager: &mut Manager,
        state: &mut S,
        ctx: &mut impl Factories<S>,
    ) -> Result<()> {
        let timer = timer.ok_or(Error::Null)?;
        let class = timer.read().class_name.clone();
        ctx.registry_log(
            Level::Warn,
            format!("Registering timer {class}"),
            self,
            manager,
            state,
        )?;
        let id = timer.read().identifier.clone();
        let key = lowercase_root(&id);
        let existing = self.map.read().values.get(&key).cloned();
        if let Some(existing) = existing {
            let existing = existing.ok_or(Error::Null)?;
            let prior = existing.read().class_name.clone();
            ctx.registry_log(
                Level::Error,
                format!("Timer identifier {id} already in use by {prior}! Not loading {class}!"),
                self,
                manager,
                state,
            )?;
            return Ok(());
        }
        self.put(key, Some(timer.clone()));
        if timer.read().is_auto {
            self.automatic.write().push(Some(timer.clone()));
        }
        Ok(())
    }
    pub fn instance<S, F: Factories<S>>(
        &self,
        id: &str,
        args: &[F::Argument],
        manager: &mut Manager,
        state: &mut S,
        ctx: &mut F,
    ) -> Result<Option<Handle>> {
        self.instance_impl(id, args, manager, state, ctx)
    }
    fn instance_impl<S, F: Factories<S>>(
        &self,
        id: &str,
        args: &[F::Argument],
        manager: &mut Manager,
        state: &mut S,
        ctx: &mut F,
    ) -> Result<Option<Handle>> {
        let timer = self
            .map
            .read()
            .values
            .get(&lowercase_root(id))
            .cloned()
            .flatten();
        match timer {
            None => Ok(None),
            Some(timer) => ctx.instantiate(
                &timer,
                (!args.is_empty()).then_some(args),
                self,
                manager,
                state,
            ),
        }
    }
    pub fn instance_where<S, F: Factories<S>>(
        &self,
        predicate: impl Fn(&Timer) -> bool,
        args: &[F::Argument],
        manager: &mut Manager,
        state: &mut S,
        ctx: &mut F,
    ) -> Result<Option<Handle>> {
        let timer = {
            let map = self.map.read();
            map.keys.iter().find_map(|(_, key)| {
                map.values[key]
                    .as_ref()
                    .filter(|timer| predicate(&timer.read()))
                    .cloned()
            })
        };
        if let Some(timer) = timer {
            return Ok(ctx
                .instantiate(
                    &timer,
                    (!args.is_empty()).then_some(args),
                    self,
                    manager,
                    state,
                )?
                .filter(|timer| predicate(&timer.read())));
        }
        Ok(None)
    }
    pub fn add_automatic<S>(
        &self,
        manager: &mut Manager,
        state: &mut S,
        ctx: &mut impl Factories<S>,
    ) -> Result<()> {
        let version = self.automatic.read().version;
        let mut cursor = 0;
        while let Some(timer) = next(&self.automatic, &mut cursor, version)? {
            let timer = timer.ok_or(Error::Null)?;
            let id = timer.read().identifier.clone();
            if manager.get_identifier(&id)?.is_some() {
                continue;
            }
            let instance = ctx.instantiate(&timer, None, self, manager, state)?;
            manager.register_optional(instance.as_ref(), state, ctx)?;
        }
        Ok(())
    }
    pub fn get_or_start<S, F: Factories<S>>(
        &self,
        id: &str,
        args: &[F::Argument],
        manager: &mut Manager,
        state: &mut S,
        ctx: &mut F,
    ) -> Result<Option<Handle>> {
        if let Some(existing) = manager.get_identifier(id)? {
            return Ok(Some(existing));
        }
        let instance = self.instance_impl(id, args, manager, state, ctx)?;
        manager.register_optional(instance.as_ref(), state, ctx)?;
        Ok(instance)
    }
    pub fn spawn_where<S, F: Factories<S>>(
        &self,
        predicate: impl Fn(&Timer) -> bool,
        args: &[F::Argument],
        manager: &mut Manager,
        state: &mut S,
        ctx: &mut F,
    ) -> Result<Handle> {
        self.instance_where(predicate, args, manager, state, ctx)?
            .ok_or(Error::Null)
    }
    pub fn get_or_start_where<S, F: Factories<S>>(
        &self,
        predicate: impl Fn(&Timer) -> bool,
        args: &[F::Argument],
        manager: &mut Manager,
        state: &mut S,
        ctx: &mut F,
    ) -> Result<Handle> {
        if let Some(existing) = manager.get_where(&predicate)? {
            return Ok(existing);
        }
        let instance = self.spawn_where(predicate, args, manager, state, ctx)?;
        manager.register(Some(&instance), state, ctx)?;
        Ok(instance)
    }
}
