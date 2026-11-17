//! Canonical player fields; the UI metadata table owns no substitute player.
#[cfg(test)]
use super::Probe;
use super::{Error, Node, Result};
#[cfg(test)]
use rs09_server::inventory::container_listener::Handle as ListenerHandle;
#[cfg(test)]
use rs09_server::{
    definitions::item::Registry,
    inventory::{self as inv, player_binding as binding},
    player::owned::Services,
};
use rs09_server::{inventory::container::Container, player::owned::Player as OwnedPlayer};
#[derive(Clone)]
pub(super) struct Capture(OwnedPlayer);
#[cfg(test)]
pub(super) fn capture(p: &mut Probe, node: Node) -> Result<Capture> {
    let owner = p
        .state
        .retain_node(node)
        .map_err(|e| Error::Binding(e.to_string()))?;
    p.state
        .player_fields(&owner)
        .map(Capture)
        .map_err(|e| Error::Binding(e.to_string()))
}
#[cfg(test)]
pub(super) fn from_listener(p: &Probe, listener: &ListenerHandle) -> Result<Capture> {
    let receiver = listener
        .value::<inv::listener::Listener>()
        .ok_or_else(|| Error::Binding("non-inventory listener".into()))?;
    let owner = receiver.owner().ok_or(Error::Null)?;
    p.state
        .player_fields(&owner)
        .map(Capture)
        .map_err(|e| Error::Binding(e.to_string()))
}
impl Capture {
    pub(super) fn new(fields: OwnedPlayer) -> Self {
        Self(fields)
    }
    pub(super) fn inventory(&self) -> Container {
        self.0.inventory()
    }
    #[cfg(test)]
    pub(super) fn has_pouch(&self) -> bool {
        *self.0.pouch().read()
    }
    pub(super) fn equipment(&self) -> Container {
        self.0.equipment()
    }
    pub(super) fn settings(&self) -> rs09_server::player::settings::Settings {
        self.0.settings()
    }
    #[cfg(test)]
    pub(super) fn apply<T>(
        &self,
        p: &mut Probe,
        operation: impl FnOnce(
            &mut Container,
            &Registry,
            &mut binding::Context<'_, '_, Services<'_>>,
        ) -> inv::Result<T>,
    ) -> Result<T> {
        self.0
            .apply(
                &mut p.state,
                &mut p.host.frames,
                super::super::now,
                operation,
            )
            .map_err(|e| Error::Binding(format!("{e:?}")))
    }
}
#[cfg(test)]
pub(super) fn apply<T>(
    p: &mut Probe,
    node: Node,
    operation: impl FnOnce(
        &mut Container,
        &Registry,
        &mut binding::Context<'_, '_, Services<'_>>,
    ) -> inv::Result<T>,
) -> Result<T> {
    capture(p, node)?.apply(p, operation)
}
pub(super) fn refresh(
    state: &mut super::super::State,
    host: &mut super::super::Host,
    node: Node,
) -> Result<()> {
    let owner = state
        .retain_node(node)
        .map_err(|e| Error::Binding(e.to_string()))?;
    let player = state
        .player_fields(&owner)
        .map_err(|e| Error::Binding(e.to_string()))?;
    player
        .apply(
            state,
            &mut host.frames,
            super::super::now,
            |inventory, definitions, context| inventory.refresh(None, definitions, context),
        )
        .map_err(|e| Error::Binding(format!("{e:?}")))
}
