use super::Player;
use crate::{
    definitions::varbit,
    player::variables::{Context, Error, Result, Saved, Values},
    world::player_walk::{Node, NodeHandle, State},
};
use rs09_protocol::game::{Frame, updates};
use std::collections::BTreeMap;
/// Captures the exact Player; raw primitive maps returned below have no player
/// backreference and may outlive it independently.
#[derive(Clone)]
pub struct Variables(pub(super) Player);
impl Variables {
    pub fn values(&self) -> Option<Values> {
        let store = self.0.0.read().variables.clone();
        store.0.read().values.clone()
    }
    pub fn saved(&self) -> Option<Saved> {
        let store = self.0.0.read().variables.clone();
        store.0.read().saved.clone()
    }
    pub fn set_values(&self, values: Option<Values>) {
        let store = self.0.0.read().variables.clone();
        store.0.write().values = values;
    }
    pub fn set_saved(&self, saved: Option<Saved>) {
        let store = self.0.0.read().variables.clone();
        store.0.write().saved = saved;
    }
    pub fn get_varp(&self, id: i32) -> Result<i32> {
        Ok(self.values().ok_or(Error::Null)?.get(id).unwrap_or(0))
    }
}
pub struct VariableServices<'a, A: varbit::Assets> {
    state: &'a mut State,
    frames: &'a mut BTreeMap<Node, Vec<Frame>>,
    assets: &'a mut A,
}
pub struct WorldVariableAssets(Option<std::sync::Arc<rs09_cache::archive::Archive>>);
impl varbit::Assets for WorldVariableAssets {
    fn file(
        &mut self,
        registry: &varbit::Registry,
        group: u32,
        file: u32,
    ) -> varbit::Result<Option<Vec<u8>>> {
        varbit::Assets::file(
            self.0.as_mut().ok_or_else(|| {
                varbit::Error::Binding("canonical variable cache archive is unavailable".into())
            })?,
            registry,
            group,
            file,
        )
    }
}
impl Player {
    pub fn apply_world_variables<T>(
        &self,
        state: &mut State,
        frames: &mut BTreeMap<Node, Vec<Frame>>,
        operation: impl FnOnce(&mut VariableServices<'_, WorldVariableAssets>) -> Result<T>,
    ) -> Result<T> {
        let mut assets = WorldVariableAssets(state.identity().varbit_assets());
        self.apply_variables(state, frames, &mut assets, operation)
    }
    pub fn apply_variables<T, A: varbit::Assets>(
        &self,
        state: &mut State,
        frames: &mut BTreeMap<Node, Vec<Frame>>,
        assets: &mut A,
        operation: impl FnOnce(&mut VariableServices<'_, A>) -> Result<T>,
    ) -> Result<T> {
        let owner = self.owner();
        if !self.in_world(&state.identity())
            || !state.player_fields(&owner).is_ok_and(|p| p.same(self))
        {
            return Err(Error::Binding(
                "variable owner belongs to another canonical world".into(),
            ));
        }
        operation(&mut VariableServices {
            state,
            frames,
            assets,
        })
    }
}
impl<A: varbit::Assets> VariableServices<'_, A> {
    fn player(&self, owner: &NodeHandle) -> Result<Player> {
        self.state
            .player_fields(owner)
            .map_err(|e| Error::Binding(e.to_string()))
    }
    fn send(&mut self, player: Player, frame: Frame) -> Result<()> {
        let owner = player.owner();
        // The outbox is keyed by a canonical lease, never a recycled wire index.
        // Disconnected owners keep their data mutations but cannot deliver to a
        // later player occupying their old wire slot.
        let actor = self
            .state
            .actor(owner.node)
            .map_err(|e| Error::Binding(e.to_string()))?;
        if !actor.connected {
            return Err(Error::Binding("variable recipient is disconnected".into()));
        }
        self.frames.entry(owner.node).or_default().push(frame);
        Ok(())
    }
}
impl<A: varbit::Assets> Context for VariableServices<'_, A> {
    type Dispatch = Player;
    fn values(&mut self, p: &NodeHandle) -> Result<Option<Values>> {
        Ok(self.player(p)?.variables().values())
    }
    fn saved(&mut self, p: &NodeHandle) -> Result<Option<Saved>> {
        Ok(self.player(p)?.variables().saved())
    }
    fn dispatch(&mut self, p: &NodeHandle) -> Result<Player> {
        self.player(p)
    }
    fn send_varp(&mut self, p: Player, id: i32, value: i32) -> Result<()> {
        self.send(p, updates::varp(id, value))
    }
    fn send_varc(&mut self, p: Player, id: i16, value: i32) -> Result<()> {
        let count = p.manager().packet_count(1);
        self.send(p, updates::varc(i32::from(id), value, count))
    }
    fn definition(&mut self, id: i32) -> Result<Option<varbit::Handle>> {
        let registry = self.state.identity().varbits();
        registry
            .for_id(id, self.assets)
            .map(Some)
            .map_err(Error::Definition)
    }
    fn missing_definition(&mut self, _: i32) -> Result<()> {
        Err(Error::Binding(
            "canonical definition registry unexpectedly returned null".into(),
        ))
    }
}
