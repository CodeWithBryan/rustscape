//! Complete Lock/ActionLocks algorithms. The world clock and actual LockElapse
//! invocation are explicit native bindings. No field guard spans either call.
use super::{
    component_objects::{Captures, NativeObject, Plugin},
    player_walk::{Identity, NodeHandle, State},
};
use crate::definitions::item::Shared;
use std::{collections::BTreeMap, sync::Arc};
pub type Text = Arc<[u16]>;
pub type Key = Option<Vec<u16>>;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Null,
    Binding(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
pub struct LockFields<E = Plugin> {
    domain: Option<Identity>,
    expiration: i32,
    elapse: Option<E>,
    message: Option<Text>,
}
pub struct Lock<E = Plugin> {
    pub fields: Shared<LockFields<E>>,
}
impl<E> Clone for Lock<E> {
    fn clone(&self) -> Self {
        Self {
            fields: self.fields.clone(),
        }
    }
}
pub type Custom<E = Plugin> = Shared<BTreeMap<Key, Lock<E>>>;
pub struct Fields<E = Plugin> {
    domain: Option<Identity>,
    movement: Lock<E>,
    teleport: Lock<E>,
    component: Lock<E>,
    interaction: Lock<E>,
    equipment: Option<Lock<E>>,
    custom: Custom<E>,
}
pub struct ActionLocks<E = Plugin> {
    pub fields: Shared<Fields<E>>,
}
impl<E> Clone for ActionLocks<E> {
    fn clone(&self) -> Self {
        Self {
            fields: self.fields.clone(),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Movement,
    Teleport,
    Component,
    Interaction,
    Equipment,
}
pub trait Context<S, E = Plugin, N = NodeHandle> {
    fn ticks(&mut self, state: &mut S) -> Result<i32>;
    fn elapse(
        &mut self,
        event: Option<E>,
        node: Option<N>,
        lock: &Lock<E>,
        actions: &ActionLocks<E>,
        state: &mut S,
    ) -> Result<()>;
}
/// Basic world-clock operations do not invoke LockElapse. Callers that unlock
/// custom locks with callbacks must use the real host-bound Runtime context.
pub struct Clock;
impl Context<State> for Clock {
    fn ticks(&mut self, state: &mut State) -> Result<i32> {
        Ok(state.ticks)
    }
    fn elapse(
        &mut self,
        _event: Option<Plugin>,
        _node: Option<NodeHandle>,
        _lock: &Lock,
        _actions: &ActionLocks,
        _state: &mut State,
    ) -> Result<()> {
        Err(Error::Binding(
            "actual LockElapse host binding is required".into(),
        ))
    }
}
pub trait Bindings {
    fn elapse(
        &mut self,
        state: &mut State,
        event: Option<Plugin>,
        node: Option<NodeHandle>,
        lock: &Lock,
    ) -> Result<()>;
}
pub struct Runtime<'a, B: Bindings> {
    bindings: &'a mut B,
}
impl<'a, B: Bindings> Runtime<'a, B> {
    pub fn new(bindings: &'a mut B) -> Self {
        Self { bindings }
    }
}
impl<B: Bindings> Context<State> for Runtime<'_, B> {
    fn ticks(&mut self, state: &mut State) -> Result<i32> {
        Ok(state.ticks)
    }
    fn elapse(
        &mut self,
        event: Option<Plugin>,
        node: Option<NodeHandle>,
        lock: &Lock,
        _actions: &ActionLocks,
        state: &mut State,
    ) -> Result<()> {
        self.bindings.elapse(state, event, node, lock)
    }
}
type Body<B> = fn(&Native<B>, &mut State, &mut B, Option<NodeHandle>, Lock) -> Result<()>;
pub struct Native<B> {
    captures: Captures,
    body: Body<B>,
}
impl<B: 'static> NativeObject for Native<B> {
    fn captures(&self) -> Captures {
        self.captures.clone()
    }
}
impl<B> Native<B> {
    pub fn new(captures: Captures, body: Body<B>) -> Self {
        Self { captures, body }
    }
    pub fn fields(&self) -> &Captures {
        &self.captures
    }
    pub fn invoke(
        &self,
        state: &mut State,
        bindings: &mut B,
        node: Option<NodeHandle>,
        lock: Lock,
    ) -> Result<()> {
        if node.as_ref().is_some_and(|n| !state.contains_handle(n)) {
            return Err(Error::Binding(
                "LockElapse argument belongs to another world".into(),
            ));
        }
        let mut fields = self.captures.clone();
        fields.locks.push(lock.clone());
        super::script_runtime::validate_captures(state, fields)
            .map_err(|e| Error::Binding(format!("{e:?}")))?;
        (self.body)(self, state, bindings, node, lock)
    }
}
impl<E> Lock<E> {
    pub fn new(message: Option<Text>) -> Self {
        Self::build(None, message)
    }
    pub fn new_in(domain: Identity, message: Option<Text>) -> Self {
        Self::build(Some(domain), message)
    }
    fn build(domain: Option<Identity>, message: Option<Text>) -> Self {
        Self {
            fields: Shared::new(LockFields {
                domain,
                expiration: 0,
                elapse: None,
                message,
            }),
        }
    }
    pub fn domain(&self) -> Option<Identity> {
        self.fields.read().domain.clone()
    }
    pub fn same(&self, other: &Self) -> bool {
        self.fields.same(&other.fields)
    }
    pub fn expiration(&self) -> i32 {
        self.fields.read().expiration
    }
    pub fn message(&self) -> Option<Text> {
        self.fields.read().message.clone()
    }
    pub fn set_message(&self, message: Option<Text>) {
        let old = { std::mem::replace(&mut self.fields.write().message, message) };
        drop(old);
    }
    pub fn unlock(&self) {
        self.fields.write().expiration = 0;
    }
}
impl<E: Clone> Lock<E> {
    pub fn elapse(&self) -> Option<E> {
        self.fields.read().elapse.clone()
    }
    pub fn set_elapse(&self, event: Option<E>) -> Self {
        let old = { std::mem::replace(&mut self.fields.write().elapse, event) };
        drop(old);
        self.clone()
    }
    pub fn lock<S, N>(
        &self,
        ticks: i32,
        state: &mut S,
        ctx: &mut impl Context<S, E, N>,
    ) -> Result<()> {
        // Java reads expiration before evaluating the first world-clock call.
        let expiration = self.expiration();
        if ticks > expiration.wrapping_sub(ctx.ticks(state)?) {
            let value = ctx.ticks(state)?.wrapping_add(ticks);
            self.fields.write().expiration = value;
        }
        Ok(())
    }
    pub fn lock_forever<S, N>(&self, state: &mut S, ctx: &mut impl Context<S, E, N>) -> Result<()> {
        let ticks = i32::MAX.wrapping_sub(ctx.ticks(state)?);
        self.lock(ticks, state, ctx)
    }
    pub fn is_locked<S, N>(&self, state: &mut S, ctx: &mut impl Context<S, E, N>) -> Result<bool> {
        let expiration = self.expiration();
        Ok(expiration > ctx.ticks(state)?)
    }
    pub(crate) fn clear_unreachable(&self) {
        let old = { self.fields.write().elapse.take() };
        drop(old);
    }
}
impl<E> Default for ActionLocks<E> {
    fn default() -> Self {
        Self::build(None)
    }
}
impl<E> ActionLocks<E> {
    pub fn new_in(domain: Identity) -> Self {
        Self::build(Some(domain))
    }
    fn build(domain: Option<Identity>) -> Self {
        let make = || Lock::build(domain.clone(), None);
        Self {
            fields: Shared::new(Fields {
                movement: make(),
                teleport: make(),
                component: make(),
                interaction: make(),
                equipment: None,
                custom: Shared::new(BTreeMap::new()),
                domain,
            }),
        }
    }
    pub fn same(&self, other: &Self) -> bool {
        self.fields.same(&other.fields)
    }
    pub fn domain(&self) -> Option<Identity> {
        self.fields.read().domain.clone()
    }
    pub fn get(&self, kind: Kind) -> Option<Lock<E>> {
        let f = self.fields.read();
        match kind {
            Kind::Movement => Some(f.movement.clone()),
            Kind::Teleport => Some(f.teleport.clone()),
            Kind::Component => Some(f.component.clone()),
            Kind::Interaction => Some(f.interaction.clone()),
            Kind::Equipment => f.equipment.clone(),
        }
    }
    pub fn set_equipment(&self, lock: Option<Lock<E>>) {
        let old = { std::mem::replace(&mut self.fields.write().equipment, lock) };
        drop(old);
    }
    /// Actual private map identity, for owning-graph inspection and retained
    /// oracle views. No iteration order is used for gameplay dispatch.
    pub fn custom(&self) -> Custom<E> {
        self.fields.read().custom.clone()
    }
    pub(crate) fn stored_fields(&self) -> (Vec<Lock<E>>, Custom<E>) {
        let f = self.fields.read();
        (
            std::iter::once(&f.movement)
                .chain([&f.teleport, &f.component, &f.interaction])
                .chain(f.equipment.iter())
                .cloned()
                .collect(),
            f.custom.clone(),
        )
    }
    pub fn unlock(&self) {
        self.get(Kind::Movement).unwrap().unlock();
        self.get(Kind::Interaction).unwrap().unlock();
    }
}
impl<E: Clone> ActionLocks<E> {
    pub fn lock<S, N>(
        &self,
        ticks: i32,
        state: &mut S,
        ctx: &mut impl Context<S, E, N>,
    ) -> Result<()> {
        self.get(Kind::Movement).unwrap().lock(ticks, state, ctx)?;
        self.get(Kind::Interaction).unwrap().lock(ticks, state, ctx)
    }
    pub fn lock_forever<S, N>(&self, state: &mut S, ctx: &mut impl Context<S, E, N>) -> Result<()> {
        let ticks = i32::MAX.wrapping_sub(ctx.ticks(state)?);
        self.lock(ticks, state, ctx)
    }
    pub fn lock_kind<S, N>(
        &self,
        kind: Kind,
        ticks: i32,
        state: &mut S,
        ctx: &mut impl Context<S, E, N>,
    ) -> Result<()> {
        self.get(kind).ok_or(Error::Null)?.lock(ticks, state, ctx)
    }
    pub fn unlock_kind(&self, kind: Kind) -> Result<()> {
        self.get(kind).ok_or(Error::Null)?.unlock();
        Ok(())
    }
    pub fn is_kind_locked<S, N>(
        &self,
        kind: Kind,
        state: &mut S,
        ctx: &mut impl Context<S, E, N>,
    ) -> Result<bool> {
        self.get(kind).ok_or(Error::Null)?.is_locked(state, ctx)
    }
    pub fn lock_custom<S, N>(
        &self,
        key: Key,
        ticks: i32,
        event: Option<E>,
        state: &mut S,
        ctx: &mut impl Context<S, E, N>,
    ) -> Result<Lock<E>> {
        let custom = self.custom();
        let found = custom.read().get(&key).cloned();
        let lock = if let Some(lock) = found {
            lock
        } else {
            let lock = Lock::build(self.domain(), None);
            let old = { custom.write().insert(key, lock.clone()) };
            drop(old);
            lock
        };
        lock.set_elapse(event).lock(ticks, state, ctx)?;
        Ok(lock)
    }
    pub fn unlock_custom<S, N: Clone>(
        &self,
        key: &Key,
        cache_remove: bool,
        node: Option<N>,
        state: &mut S,
        ctx: &mut impl Context<S, E, N>,
    ) -> Result<()> {
        let selected = self.custom().read().get(key).cloned();
        if let Some(lock) = selected {
            lock.unlock();
            if lock.elapse().is_some() {
                ctx.elapse(lock.elapse(), node, &lock, self, state)?;
            }
            if cache_remove {
                let old = { self.custom().write().remove(key) };
                drop(old);
            }
        }
        Ok(())
    }
    pub fn is_locked<S, N>(
        &self,
        key: &Key,
        state: &mut S,
        ctx: &mut impl Context<S, E, N>,
    ) -> Result<bool> {
        let lock = self.custom().read().get(key).cloned();
        match lock {
            Some(lock) => lock.is_locked(state, ctx),
            None => Ok(false),
        }
    }
}
