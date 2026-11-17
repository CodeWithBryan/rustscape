//! Native inventory listener joined to its actual Settings and containers.
//! The host retains familiar/appearance/session/content receiver ownership.
use super::{
    Error, Result,
    container::{self, Container, Event},
    container_listener::Handle as ListenerHandle,
    item::Handle,
    listener::{self, Listener, Packet},
};
use crate::{
    definitions::{
        item::{Registry, Shared},
        property::Property,
    },
    player::settings::{self, ContainerKind, Settings},
    world::player_walk::NodeHandle,
};
use rs09_protocol::game::Frame;

struct CapturedFields {
    player: NodeHandle,
    inventory: Container,
    equipment: Container,
    settings: Settings,
    null_item: Handle,
}
/// Retains the actual fields used by InventoryListener/Settings, independently
/// of a later registry lookup. This component is not a complete player profile.
#[derive(Clone)]
pub struct Capture(Shared<CapturedFields>);
impl Capture {
    pub fn keeping_from<T: std::any::Any + Send + Sync>(&self, owner: &Shared<T>) -> Self {
        Self(self.0.keeping_from(owner))
    }
    pub fn new(
        player: NodeHandle,
        inventory: Container,
        equipment: Container,
        settings: Settings,
        null_item: Handle,
    ) -> Self {
        Self(Shared::new(CapturedFields {
            player,
            inventory,
            equipment,
            settings,
            null_item,
        }))
    }
    pub fn owner(&self) -> NodeHandle {
        self.0.read().player.clone()
    }
    pub fn inventory(&self) -> Container {
        self.0.read().inventory.clone()
    }
    pub fn equipment(&self) -> Container {
        self.0.read().equipment.keeping_from(&self.0)
    }
    pub(crate) fn null_item(&self) -> Handle {
        self.0.read().null_item.clone()
    }
    pub fn settings(&self) -> Settings {
        let fields = self.0.read().settings.fields.keeping_from(&self.0);
        Settings { fields }
    }
    pub(crate) fn ownership(&self) -> (NodeHandle, usize, [Container; 2]) {
        let f = self.0.read();
        (
            f.player.clone(),
            self.0.strong_count(),
            [f.inventory.clone(), f.equipment.clone()],
        )
    }
    pub fn apply<T, S: Services>(
        &self,
        definitions: &Registry,
        services: &mut S,
        operation: impl FnOnce(&mut Container, &Registry, &mut Context<'_, '_, S>) -> Result<T>,
    ) -> Result<T> {
        let (player, null_item) = {
            let f = self.0.read();
            (f.player.clone(), f.null_item.clone())
        };
        let mut inventory = self.inventory();
        let mut equipment = self.equipment();
        let settings = self.settings();
        let mut links = Links {
            player: &player,
            inventory: inventory.clone(),
            equipment: &mut equipment,
            settings: &settings,
            null_item: &null_item,
        };
        operation(
            &mut inventory,
            definitions,
            &mut Context {
                links: &mut links,
                services,
            },
        )
    }
}

/// Borrow existing player fields. Equipment and inventory arrays are never
/// mirrored: a callback's Container.clear/copy is visible to later reads.
pub struct Links<'a> {
    pub player: &'a NodeHandle,
    pub inventory: Container,
    pub equipment: &'a mut Container,
    pub settings: &'a Settings,
    pub null_item: &'a Handle,
}
pub trait Services {
    type Familiar;
    type Appearance;
    fn send(
        &mut self,
        links: &mut Links<'_>,
        inventory: &mut Container,
        frame: Frame,
    ) -> Result<()>;
    fn text(
        &mut self,
        links: &mut Links<'_>,
        inventory: &mut Container,
        text: String,
        interface: i32,
        child: i32,
    ) -> Result<()>;
    fn familiar(
        &mut self,
        links: &mut Links<'_>,
        inventory: &mut Container,
    ) -> Result<Option<Self::Familiar>>;
    fn has_pouch(
        &mut self,
        links: &mut Links<'_>,
        inventory: &mut Container,
        familiar: Self::Familiar,
    ) -> Result<bool>;
    fn set_has_pouch(
        &mut self,
        links: &mut Links<'_>,
        inventory: &mut Container,
        familiar: Self::Familiar,
        value: bool,
    ) -> Result<()>;
    fn appearance(
        &mut self,
        links: &mut Links<'_>,
        inventory: &mut Container,
    ) -> Result<Option<Self::Appearance>>;
    fn sync(
        &mut self,
        links: &mut Links<'_>,
        inventory: &mut Container,
        appearance: Self::Appearance,
    ) -> Result<()>;
    fn ground(
        &mut self,
        links: &mut Links<'_>,
        inventory: &mut Container,
        item: Option<Handle>,
        definitions: &Registry,
    ) -> Result<()>;
    fn update_foreign(
        &mut self,
        links: &mut Links<'_>,
        listener: ListenerHandle,
        inventory: &mut Container,
        event: Event,
        definitions: &Registry,
    ) -> Result<()>;
    fn refresh_foreign(
        &mut self,
        links: &mut Links<'_>,
        listener: ListenerHandle,
        inventory: &mut Container,
        definitions: &Registry,
    ) -> Result<()>;
}
pub struct Context<'a, 'b, S> {
    pub links: &'a mut Links<'b>,
    pub services: &'a mut S,
}
impl<S> Context<'_, '_, S> {
    fn owner(&self, player: &NodeHandle) -> Result<()> {
        if player != self.links.player {
            return Err(Error::Binding(
                "inventory listener belongs to another canonical player".into(),
            ));
        }
        Ok(())
    }
}
fn inventory_error(error: settings::Error) -> Error {
    match error {
        settings::Error::Null => Error::Null,
        settings::Error::Type => Error::Type,
        settings::Error::Number => Error::Number,
        settings::Error::Index => Error::Bounds,
        settings::Error::Binding => Error::Binding("owning-player weight service failed".into()),
    }
}
impl<S: Services> listener::Context for Context<'_, '_, S> {
    type Settings = Settings;
    type Familiar = S::Familiar;
    type Appearance = S::Appearance;
    fn send(&mut self, packet: Packet, inventory: &mut Container) -> Result<()> {
        self.owner(packet.player.as_ref().ok_or(Error::Null)?)?;
        let frame = packet.frame(self.links.null_item)?;
        self.services.send(self.links, inventory, frame)
    }
    fn settings(&mut self, player: &NodeHandle, _: &mut Container) -> Result<Option<Settings>> {
        self.owner(player)?;
        Ok(Some(self.links.settings.clone()))
    }
    fn update_weight(&mut self, settings: Settings, inventory: &mut Container) -> Result<()> {
        settings
            .update_weight(&mut Weight {
                binding: self,
                inventory,
                item: None,
            })
            .map_err(inventory_error)
    }
    fn familiar(
        &mut self,
        player: &NodeHandle,
        inventory: &mut Container,
    ) -> Result<Option<S::Familiar>> {
        self.owner(player)?;
        self.services.familiar(self.links, inventory)
    }
    fn has_pouch(&mut self, familiar: S::Familiar, inventory: &mut Container) -> Result<bool> {
        self.services.has_pouch(self.links, inventory, familiar)
    }
    fn set_has_pouch(
        &mut self,
        familiar: S::Familiar,
        value: bool,
        inventory: &mut Container,
    ) -> Result<()> {
        self.services
            .set_has_pouch(self.links, inventory, familiar, value)
    }
    fn appearance(
        &mut self,
        player: &NodeHandle,
        inventory: &mut Container,
    ) -> Result<Option<S::Appearance>> {
        self.owner(player)?;
        self.services.appearance(self.links, inventory)
    }
    fn sync(&mut self, appearance: S::Appearance, inventory: &mut Container) -> Result<()> {
        self.services.sync(self.links, inventory, appearance)
    }
}
impl<S: Services> container::Context for Context<'_, '_, S> {
    fn update(
        &mut self,
        listener: ListenerHandle,
        inventory: &mut Container,
        event: Event,
        definitions: &Registry,
    ) -> Result<()> {
        if let Some(receiver) = listener.value::<Listener>().filter(|receiver| {
            receiver
                .owner()
                .as_ref()
                .is_none_or(|owner| owner == self.links.player)
        }) {
            receiver.update_event(self, inventory, Some(&event))
        } else {
            self.services
                .update_foreign(self.links, listener, inventory, event, definitions)
        }
    }
    fn refresh(
        &mut self,
        listener: ListenerHandle,
        inventory: &mut Container,
        definitions: &Registry,
    ) -> Result<()> {
        if let Some(receiver) = listener.value::<Listener>().filter(|receiver| {
            receiver
                .owner()
                .as_ref()
                .is_none_or(|owner| owner == self.links.player)
        }) {
            receiver.refresh(self, inventory)
        } else {
            self.services
                .refresh_foreign(self.links, listener, inventory, definitions)
        }
    }
    fn ground(
        &mut self,
        inventory: &mut Container,
        item: Option<Handle>,
        definitions: &Registry,
    ) -> Result<()> {
        self.services
            .ground(self.links, inventory, item, definitions)
    }
}

/// Settings.updateWeight is the only exposed caller. Its item token retains the
/// exact Item returned by the immediately preceding getter, with no allocation.
struct Weight<'a, 'b, 'c, S> {
    binding: &'a mut Context<'b, 'c, S>,
    inventory: &'a mut Container,
    item: Option<Handle>,
}
impl<S: Services> settings::Context for Weight<'_, '_, '_, S> {
    fn container(&mut self, _: &Settings, kind: ContainerKind) -> settings::Result<usize> {
        Ok(match kind {
            ContainerKind::Inventory => 0,
            ContainerKind::Equipment => 1,
        })
    }
    fn item(
        &mut self,
        _: &Settings,
        container: usize,
        slot: i32,
    ) -> settings::Result<Option<usize>> {
        self.item = match container {
            0 => self.binding.links.inventory.get(slot),
            1 => self.binding.links.equipment.get(slot),
            _ => return Err(settings::Error::Binding),
        };
        Ok(self.item.as_ref().map(|_| 0))
    }
    fn weight(&mut self, _: &Settings, item: usize) -> settings::Result<f64> {
        if item != 0 {
            return Err(settings::Error::Binding);
        }
        let definition = self
            .item
            .as_ref()
            .ok_or(settings::Error::Null)?
            .read()
            .definition
            .clone()
            .ok_or(settings::Error::Null)?;
        let definition = definition.read();
        match definition
            .handlers
            .get(crate::definitions::item_config::constants::WEIGHT)
        {
            None | Some(Property::Null) => Ok(0.0),
            Some(Property::Double(value)) => Ok(*value),
            Some(_) => Err(settings::Error::Type),
        }
    }
    fn dispatcher(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        Ok(Some(0))
    }
    fn string(
        &mut self,
        _: &Settings,
        dispatcher: Option<usize>,
        text: String,
        interface: i32,
        child: i32,
    ) -> settings::Result<()> {
        if dispatcher != Some(0) {
            return Err(settings::Error::Binding);
        }
        self.binding
            .services
            .text(self.binding.links, self.inventory, text, interface, child)
            .map_err(|_| settings::Error::Binding)
    }
    // Other Settings APIs require their complete owning-world context. They
    // cannot accidentally succeed through this private weight-only adapter.
    fn varp(&mut self, _: &Settings, _: i32, _: i32) -> settings::Result<()> {
        Err(settings::Error::Binding)
    }
    fn session(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        Err(settings::Error::Binding)
    }
    fn send(&mut self, _: &Settings, _: Option<usize>, _: Frame) -> settings::Result<()> {
        Err(settings::Error::Binding)
    }
    fn run_energy(&mut self, _: &Settings, _: Option<usize>) -> settings::Result<()> {
        Err(settings::Error::Binding)
    }
    fn message(&mut self, _: &Settings, _: Option<usize>, _: &str) -> settings::Result<()> {
        Err(settings::Error::Binding)
    }
    fn properties(&mut self, _: &Settings) -> settings::Result<Option<usize>> {
        Err(settings::Error::Binding)
    }
    fn retaliating(&mut self, _: &Settings, _: Option<usize>) -> settings::Result<bool> {
        Err(settings::Error::Binding)
    }
    fn set_retaliating(&mut self, _: &Settings, _: Option<usize>, _: bool) -> settings::Result<()> {
        Err(settings::Error::Binding)
    }
    fn ironman(&mut self, _: &Settings) -> settings::Result<bool> {
        Err(settings::Error::Binding)
    }
}
