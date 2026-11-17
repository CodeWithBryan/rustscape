//! Animator's owning-world binding for prepared players. The owner retains the
//! actual masks and ten Entity clocks, independently of a recyclable wire index.
use super::{
    animation::{self, Animation, Animator, Graphics},
    prepared_masks::PreparedMasks,
};
use crate::definitions::{animation as defs, item::Shared};
use std::sync::Arc;
#[derive(Clone)]
pub struct Owner {
    masks: PreparedMasks,
    clocks: Shared<Vec<i32>>,
    domain: Arc<()>,
}
impl Owner {
    pub fn masks(&self) -> &PreparedMasks {
        &self.masks
    }
    pub fn clocks(&self) -> &Shared<Vec<i32>> {
        &self.clocks
    }
}
#[derive(Clone, Default)]
pub struct Factory {
    resets: animation::Factory,
    domain: Arc<()>,
}
impl Factory {
    pub fn actor(&self, masks: PreparedMasks) -> Animator<Owner> {
        self.actor_with_clocks(masks, Shared::new(vec![0; 10]))
    }
    pub fn actor_with_clocks(
        &self,
        masks: PreparedMasks,
        clocks: Shared<Vec<i32>>,
    ) -> Animator<Owner> {
        Animator::new(
            Some(Owner {
                masks,
                clocks,
                domain: self.domain.clone(),
            }),
            self.resets.clone(),
        )
    }
    pub fn resets(&self) -> &animation::Factory {
        &self.resets
    }
}
pub struct World<S> {
    pub factory: Factory,
    pub definitions: defs::Registry,
    pub source: S,
    pub ticks: i32,
}
impl<S: defs::Source> World<S> {
    pub fn new(source: S) -> Self {
        Self {
            factory: Factory::default(),
            definitions: defs::Registry::default(),
            source,
            ticks: 0,
        }
    }
    fn owner<'a>(&self, owner: Option<&'a Owner>) -> animation::Result<&'a Owner> {
        let owner = owner.ok_or(animation::Error::Null)?;
        if !Arc::ptr_eq(&owner.domain, &self.factory.domain) {
            return Err(animation::Error::Binding(
                "foreign prepared animation world".into(),
            ));
        }
        Ok(owner)
    }
}
impl<S: defs::Source> animation::Definitions for World<S> {
    fn definition(&mut self, _: &Animation, id: i32) -> Option<defs::Handle> {
        self.definitions.for_id(&mut self.source, id)
    }
}
impl<S: defs::Source> animation::Context<Owner> for World<S> {
    fn world_ticks(&mut self, animator: &Animator<Owner>) -> animation::Result<i32> {
        if let Some(owner) = animator.owner() {
            self.owner(Some(&owner))?;
        }
        Ok(self.ticks)
    }
    fn clocks(
        &mut self,
        _: &Animator<Owner>,
        owner: Option<&Owner>,
    ) -> animation::Result<Option<Shared<Vec<i32>>>> {
        Ok(Some(self.owner(owner)?.clocks.clone()))
    }
    fn register_animation(
        &mut self,
        _: &Animator<Owner>,
        owner: Option<&Owner>,
        animation: Animation,
    ) -> animation::Result<bool> {
        self.owner(owner)?
            .masks
            .animate_handle(animation)
            .map_err(|e| animation::Error::Binding(e.to_string()))
    }
    fn register_graphics(
        &mut self,
        _: &Animator<Owner>,
        owner: Option<&Owner>,
        graphics: Graphics,
    ) -> animation::Result<bool> {
        self.owner(owner)?
            .masks
            .graphics_handle(graphics)
            .map_err(|e| animation::Error::Binding(e.to_string()))
    }
}
