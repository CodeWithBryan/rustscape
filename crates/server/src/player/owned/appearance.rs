//! Actual equipment/render receivers for full body preparation. The remaining
//! Appearance services need original Player properties/content before binding.
use crate::{
    definitions::item::Shared,
    inventory::container::Container,
    player::{
        appearance::{self, BodyContext},
        owned::Player,
    },
    world::{
        player_render::RenderInfo,
        player_walk::{NodeHandle, State},
    },
};
pub(super) struct BodyServices<'a> {
    state: &'a State,
}
impl<'a> BodyServices<'a> {
    pub(super) fn new(state: &'a State) -> Self {
        Self { state }
    }
    fn receiver(&self, node: &NodeHandle) -> appearance::Result<Player> {
        self.state
            .player_fields(node)
            .map_err(|e| appearance::Error::Binding(e.to_string()))
    }
}
impl BodyContext for BodyServices<'_> {
    type Render = Shared<RenderInfo>;
    fn equipment(&mut self, player: &NodeHandle) -> appearance::Result<Container> {
        Ok(self.receiver(player)?.equipment())
    }
    fn render(&mut self, player: &NodeHandle) -> appearance::Result<Self::Render> {
        Ok(self.receiver(player)?.render_info())
    }
    fn prepared(&mut self, render: Self::Render) -> appearance::Result<bool> {
        Ok(render.read().prepared_appearance)
    }
    fn set_prepared(&mut self, render: Self::Render, value: bool) -> appearance::Result<()> {
        render.write().prepared_appearance = value;
        Ok(())
    }
}
