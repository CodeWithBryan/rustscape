//! Pickup over the owning world's actual ground manager, canonical player,
//! container/listener state, attributes and Scene. Content services stay required.
use super::{
    doors::Location,
    ground_item::{self as ground, Handle},
    ground_manager::{Error, Result},
    ground_world::{Parts, Services as GroundServices, World},
    pickup,
    player_walk::{Node, NodeHandle},
    scene,
};
use crate::{
    inventory::{self, container::Container, item},
    player::{ironman, owned::Player},
};
use rs09_protocol::game::Frame;
use std::collections::BTreeMap;

/// The real container receiver evaluated before each original operation.
pub struct Inventory {
    player: Player,
    container: Container,
}
pub trait Services<A: scene::Assets>: GroundServices<A> {
    fn player_frames(&mut self) -> &mut BTreeMap<Node, Vec<Frame>>;
    fn player_clock(&self) -> fn() -> i64;
    fn pickup_has_item(
        &mut self,
        world: &mut Parts<'_, A>,
        player: &NodeHandle,
        item: &item::Handle,
    ) -> Result<bool>;
    fn pickup_plugin(
        &mut self,
        world: &mut Parts<'_, A>,
        plugin: Self::Plugin,
        player: &NodeHandle,
        item: &Handle,
        kind: i32,
    ) -> Result<bool>;
    fn packet_message(
        &mut self,
        world: &mut Parts<'_, A>,
        player: &NodeHandle,
        text: &str,
    ) -> Result<()>;
    fn player_message(
        &mut self,
        world: &mut Parts<'_, A>,
        player: &NodeHandle,
        text: &str,
    ) -> Result<()>;
    fn player_messages(
        &mut self,
        world: &mut Parts<'_, A>,
        player: &NodeHandle,
        text: &[&str],
    ) -> Result<()>;
    fn pickup_guild_dialogue(
        &mut self,
        world: &mut Parts<'_, A>,
        player: &NodeHandle,
    ) -> Result<()>;
    fn pickup_player_name(
        &mut self,
        world: &mut Parts<'_, A>,
        player: &NodeHandle,
    ) -> Result<Option<String>>;
    fn pickup_log(
        &mut self,
        world: &mut Parts<'_, A>,
        dropper: Option<NodeHandle>,
        text: &str,
    ) -> Result<()>;
    fn pickup_animate(
        &mut self,
        world: &mut Parts<'_, A>,
        player: &NodeHandle,
        id: i32,
    ) -> Result<()>;
    fn pickup_audio(
        &mut self,
        world: &mut Parts<'_, A>,
        player: &NodeHandle,
        id: i32,
    ) -> Result<()>;
    fn pickup_event(
        &mut self,
        world: &mut Parts<'_, A>,
        player: &NodeHandle,
        id: i32,
    ) -> Result<()>;
}
fn error(e: impl std::fmt::Display) -> Error {
    Error::Binding(e.to_string())
}
fn inventory_error(e: inventory::Error) -> Error {
    match e {
        inventory::Error::Null => Error::Null,
        inventory::Error::Type => Error::Type,
        inventory::Error::Bounds | inventory::Error::ListBounds => Error::Index,
        e => error(format!("{e:?}")),
    }
}
fn player<A: scene::Assets>(world: &Parts<'_, A>, owner: &NodeHandle) -> Result<Player> {
    world.actors.player_fields(owner).map_err(error)
}
fn validate<A: scene::Assets>(world: &Parts<'_, A>, inventory: &Inventory) -> Result<()> {
    let owner = inventory.player.owner();
    if !inventory.player.in_world(&world.actors.identity())
        || !player(world, &owner)?.same(&inventory.player)
    {
        return Err(error("pickup inventory belongs to another canonical world"));
    }
    Ok(())
}
struct Ironman<'a, 'b, 'c, A: scene::Assets, S: Services<A>>(&'a mut World<'b, 'c, A, S>);
impl<A: scene::Assets, S: Services<A>> ironman::Context for Ironman<'_, '_, '_, A, S> {
    fn message(&mut self, p: Option<&NodeHandle>, text: &str) -> ironman::Result<()> {
        let p = p.ok_or(ironman::Error::Null)?;
        self.0
            .services
            .player_message(self.0.world, p, text)
            .map_err(|e| match e {
                Error::Null => ironman::Error::Null,
                e => ironman::Error::Binding(e.to_string()),
            })
    }
}
impl<A: scene::Assets, S: Services<A>> pickup::Context for World<'_, '_, A, S> {
    type Inventory = Inventory;
    fn inventory(&mut self, p: &NodeHandle) -> Result<Inventory> {
        let player = player(self.world, p)?;
        let container = player.inventory();
        Ok(Inventory { player, container })
    }
    fn has_space(&mut self, i: Inventory, item: &item::Handle) -> Result<bool> {
        validate(self.world, &i)?;
        i.container
            .has_space_for(Some(item))
            .map_err(inventory_error)
    }
    fn add(&mut self, i: Inventory, item: &item::Handle) -> Result<bool> {
        validate(self.world, &i)?;
        let clock = self.services.player_clock();
        let mut container = i.container;
        i.player
            .apply(
                self.world.actors,
                self.services.player_frames(),
                clock,
                |_, defs, ctx| container.add_two(Some(item), true, defs, ctx),
            )
            .map_err(inventory_error)
    }
    fn drop_deadline(&mut self, p: &NodeHandle, key: &str) -> Result<i32> {
        player(self.world, p)?
            .attributes()
            .integer(key, 0)
            .map_err(|e| match e {
                crate::player::attributes::Error::Null => Error::Null,
                crate::player::attributes::Error::Type => Error::Type,
                e => error(e),
            })
    }
    fn is_spawn(&mut self, h: &Handle) -> Result<bool> {
        Ok(self.world.items.spawns.get(h).is_some())
    }
    fn ironman_restriction(&mut self, p: &NodeHandle) -> Result<bool> {
        let manager = player(self.world, p)?.ironman();
        manager
            .check_restriction(&mut Ironman(self))
            .map_err(|e| match e {
                ironman::Error::Null => Error::Null,
                e => error(e),
            })
    }
    fn god_cape(&mut self, h: &Handle) -> Result<bool> {
        Ok(self
            .world
            .items
            .pickup_types
            .cape(&ground::as_item(h))
            .is_some())
    }
    fn has_god_cape(&mut self, p: &NodeHandle) -> Result<bool> {
        let types = self.world.items.pickup_types.clone();
        types.has_cape(|item| self.services.pickup_has_item(self.world, p, item))
    }
    fn rune_pouch(&mut self, h: &Handle) -> Result<bool> {
        Ok(self
            .world
            .items
            .pickup_types
            .pouch(&ground::as_item(h))
            .is_some())
    }
    fn has_item(&mut self, p: &NodeHandle, h: &Handle) -> Result<bool> {
        self.services
            .pickup_has_item(self.world, p, &ground::as_item(h))
    }
    fn plugin_pickup(
        &mut self,
        plugin: Self::Plugin,
        p: &NodeHandle,
        h: &Handle,
        kind: i32,
    ) -> Result<bool> {
        self.services.pickup_plugin(self.world, plugin, p, h, kind)
    }
    fn packet_message(&mut self, p: &NodeHandle, s: &str) -> Result<()> {
        self.services.packet_message(self.world, p, s)
    }
    fn message(&mut self, p: &NodeHandle, s: &str) -> Result<()> {
        self.services.player_message(self.world, p, s)
    }
    fn messages(&mut self, p: &NodeHandle, s: &[&str]) -> Result<()> {
        self.services.player_messages(self.world, p, s)
    }
    fn guild_dialogue(&mut self, p: &NodeHandle) -> Result<()> {
        self.services.pickup_guild_dialogue(self.world, p)
    }
    fn item_name(&mut self, id: i32) -> Result<String> {
        Ok(self
            .world
            .items
            .definitions
            .for_id(id)
            .read()
            .name
            .clone()
            .unwrap_or_else(|| "null".into()))
    }
    fn player_name(&mut self, p: &NodeHandle) -> Result<Option<String>> {
        self.services.pickup_player_name(self.world, p)
    }
    fn drop_trade_log(&mut self, p: Option<NodeHandle>, s: &str) -> Result<()> {
        self.services.pickup_log(self.world, p, s)
    }
    fn teleport_permitted(&mut self, l: Option<Location>) -> Result<bool> {
        self.world
            .scene
            .context(self.services)
            .teleport_permitted(l.as_ref())
            .map_err(error)
    }
    fn animate(&mut self, p: &NodeHandle, id: i32) -> Result<()> {
        self.services.pickup_animate(self.world, p, id)
    }
    fn audio(&mut self, p: &NodeHandle, id: i32) -> Result<()> {
        self.services.pickup_audio(self.world, p, id)
    }
    fn dispatch(&mut self, p: &NodeHandle, id: i32) -> Result<()> {
        self.services.pickup_event(self.world, p, id)
    }
    fn pickup_attribute(&mut self, p: &NodeHandle) -> Result<bool> {
        Ok(player(self.world, p)?.attributes().contains_key("pickup"))
    }
    fn remove_pickup_attribute(&mut self, p: &NodeHandle) -> Result<()> {
        player(self.world, p)?.attributes().remove("pickup");
        Ok(())
    }
}
