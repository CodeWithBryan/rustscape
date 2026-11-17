//! Original InterfaceListeners registry. Clone actual function objects before
//! invocation; no registry guard spans callbacks or reentrant registration.
use super::{
    component_objects::{NativeObject, Plugin},
    interfaces::{Error, Handle, Result},
    player_walk::NodeHandle,
};
use crate::definitions::item::Shared;
use std::{any::Any, collections::HashMap, sync::Arc};
/// Kotlin function object using the shared opaque-object header. This wrapper
/// cannot be installed as a ComponentPlugin through the public API.
#[derive(Clone)]
pub struct Callback(Plugin, u8);
impl Callback {
    pub fn open<T: NativeObject>(value: T) -> Self {
        Self(Plugin::new(value), 1)
    }
    pub fn button<T: NativeObject>(value: T) -> Self {
        Self(Plugin::new(value), 2)
    }
    pub fn both<T: NativeObject>(value: T) -> Self {
        Self(Plugin::new(value), 3)
    }
    pub fn value<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        self.0.value()
    }
    pub fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
    pub fn identity(&self) -> usize {
        self.0.identity()
    }
}
pub type Mapping = Shared<HashMap<String, Option<Callback>>>;
#[derive(Clone)]
pub struct Registry {
    pub button_listeners: Mapping,
    pub open_listeners: Mapping,
}
impl Default for Registry {
    fn default() -> Self {
        Self {
            button_listeners: Shared::new(HashMap::with_capacity(1000)),
            open_listeners: Shared::new(HashMap::with_capacity(100)),
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Button {
    pub opcode: i32,
    pub button: i32,
    pub slot: i32,
    pub item: i32,
}
pub trait Context {
    fn invoke_open(
        &mut self,
        callback: Callback,
        player: &NodeHandle,
        component: &Handle,
    ) -> Result<bool>;
    fn invoke_button(
        &mut self,
        callback: Callback,
        player: &NodeHandle,
        component: &Handle,
        button: Button,
    ) -> Result<bool>;
}
impl Registry {
    pub fn add(&self, component: i32, button: Option<i32>, callback: Callback) -> Result<()> {
        if callback.1 & 2 == 0 {
            return Err(Error::Type);
        }
        let key = button.map_or_else(
            || component.to_string(),
            |button| format!("{component}:{button}"),
        );
        self.button_listeners.write().insert(key, Some(callback));
        Ok(())
    }
    pub fn add_open(&self, component: i32, callback: Callback) -> Result<()> {
        if callback.1 & 1 == 0 {
            return Err(Error::Type);
        }
        self.open_listeners
            .write()
            .insert(component.to_string(), Some(callback));
        Ok(())
    }
    pub fn add_close(&self, component: i32, callback: Callback) -> Result<()> {
        if callback.1 & 1 == 0 {
            return Err(Error::Type);
        }
        self.open_listeners
            .write()
            .insert(format!("close:{component}"), Some(callback));
        Ok(())
    }
    fn cast(callback: Option<Callback>, kind: u8) -> Result<Option<Callback>> {
        if callback.as_ref().is_some_and(|c| c.1 & kind == 0) {
            Err(Error::Type)
        } else {
            Ok(callback)
        }
    }
    pub fn get(&self, component: i32, button: Option<i32>) -> Result<Option<Callback>> {
        let key = button.map_or_else(
            || component.to_string(),
            |button| format!("{component}:{button}"),
        );
        Self::cast(self.button_listeners.read().get(&key).cloned().flatten(), 2)
    }
    pub fn get_open(&self, component: i32) -> Result<Option<Callback>> {
        Self::cast(
            self.open_listeners
                .read()
                .get(&component.to_string())
                .cloned()
                .flatten(),
            1,
        )
    }
    pub fn get_close(&self, component: i32) -> Result<Option<Callback>> {
        Self::cast(
            self.open_listeners
                .read()
                .get(&format!("close:{component}"))
                .cloned()
                .flatten(),
            1,
        )
    }
    pub fn run_open(
        &self,
        ctx: &mut impl Context,
        player: Option<&NodeHandle>,
        component: Option<&Handle>,
    ) -> Result<bool> {
        let player = player.ok_or(Error::Null)?;
        let component = component.ok_or(Error::Null)?;
        let id = component.read().id;
        match self.get_open(id)? {
            Some(callback) => ctx.invoke_open(callback, player, component),
            None => Ok(false),
        }
    }
    pub fn run_close(
        &self,
        ctx: &mut impl Context,
        player: Option<&NodeHandle>,
        component: Option<&Handle>,
    ) -> Result<bool> {
        let player = player.ok_or(Error::Null)?;
        let component = component.ok_or(Error::Null)?;
        let id = component.read().id;
        match self.get_close(id)? {
            Some(callback) => ctx.invoke_open(callback, player, component),
            None => Ok(true),
        }
    }
    pub fn run(
        &self,
        ctx: &mut impl Context,
        player: Option<&NodeHandle>,
        component: Option<&Handle>,
        button: Button,
    ) -> Result<bool> {
        let player = player.ok_or(Error::Null)?;
        let component = component.ok_or(Error::Null)?;
        let id = component.read().id;
        let callback = match self.get(id, Some(button.button))? {
            Some(callback) => Some(callback),
            None => self.get(component.read().id, None)?,
        };
        match callback {
            Some(callback) => ctx.invoke_button(callback, player, component, button),
            None => Ok(false),
        }
    }
}
