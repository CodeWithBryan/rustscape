//! The Appearance flag of the live world: `player::appearance_writer` (the
//! original's PlayerFlags530.Appearance writer) over the live receivers,
//! run where the original runs it, when the update phase writes the mask.
//!
//! Receivers: the canonical player's Appearance, equipment, render info and
//! attributes, the profile's Properties and Skills, the SkullManager of
//! `super::region_zones`, the canonical node's size and the configuration's
//! `pvp` and `wild_pvp_enabled`. `FamiliarManager.isUsingSummoning` is the
//! pouch flag (familiars are not installed) and its summoning combat level
//! is the profile's. `transformNPC` is not joined: the services only it
//! reaches are refused.
use super::{P, inner::Inner};
use crate::{
    definitions::{item::Registry, item::Shared, npc, property::Property},
    inventory::{
        self,
        container::{Container, Context as ContainerContext, Event},
        container_listener::Handle as Listener,
        item::Handle as ItemHandle,
    },
    player::{
        appearance::{self, Appearance, BodyContext, CombatAnimation, Error, Result},
        appearance_writer::{self, Buffer, Input},
        attributes::AttributeValue,
        owned::Player,
        properties::CombatLevelInputs,
        settings::Settings,
    },
    world::{
        animation::Animation,
        player_managers::skull::SkullManager,
        player_render::RenderInfo,
        player_walk::{Node, NodeHandle},
        update_masks as masks,
    },
};
use std::collections::HashMap;

fn refused<T>(what: &str) -> Result<T> {
    Err(Error::Binding(format!(
        "not joined in the live world's appearance flag: {what}"
    )))
}
fn bind(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}

/// What the flag writer reads, borrowed for one write.
pub struct Receivers<'a> {
    pub inner: &'a Inner,
    /// `player.getSkullManager()`, by player node; a player without an
    /// entry has the constructor's fields.
    pub skulls: &'a HashMap<Node, SkullManager<NodeHandle>>,
    /// `GameWorld.settings.wild_pvp_enabled`.
    pub wild_pvp: bool,
}

impl Receivers<'_> {
    fn player(&self, node: &NodeHandle) -> Result<Player> {
        self.inner
            .players
            .get(&node.node)
            .map(|slot| slot.canonical.player.clone())
            .ok_or_else(|| Error::Binding("the appearance of an unknown player".into()))
    }
    fn skull_field<T>(
        &self,
        node: &NodeHandle,
        read: impl FnOnce(&SkullManager<NodeHandle>) -> T,
    ) -> T {
        match self.skulls.get(&node.node) {
            Some(skull) => read(skull),
            None => read(&SkullManager::new(node.clone())),
        }
    }
    /// The whole field (the transformed length byte first) as the original
    /// appends it to the mask block.
    pub fn encode(&mut self, player: &NodeHandle, output: &mut Vec<u8>) -> masks::Result<()> {
        let buffer = Buffer::default();
        appearance_writer::write_to(self, Some(&buffer), Input::Player(player))
            .map_err(|e| masks::Error::Binding(format!("PlayerFlags530.Appearance: {e:?}")))?;
        output.extend(buffer.bytes());
        Ok(())
    }
}

impl BodyContext for Receivers<'_> {
    type Render = Shared<RenderInfo>;
    fn equipment(&mut self, player: &NodeHandle) -> Result<Container> {
        Ok(self.player(player)?.equipment())
    }
    fn render(&mut self, player: &NodeHandle) -> Result<Self::Render> {
        Ok(self.player(player)?.render_info())
    }
    fn prepared(&mut self, render: Self::Render) -> Result<bool> {
        Ok(render.read().prepared_appearance)
    }
    fn set_prepared(&mut self, render: Self::Render, value: bool) -> Result<()> {
        render.write().prepared_appearance = value;
        Ok(())
    }
}

/// The flag reads the equipment; it never changes a container.
impl ContainerContext for Receivers<'_> {
    fn update(
        &mut self,
        _: Listener,
        _: &mut Container,
        _: Event,
        _: &Registry,
    ) -> inventory::Result<()> {
        Err(inventory::Error::Binding(
            "a container update from the appearance flag".into(),
        ))
    }
    fn refresh(&mut self, _: Listener, _: &mut Container, _: &Registry) -> inventory::Result<()> {
        Err(inventory::Error::Binding(
            "a container refresh from the appearance flag".into(),
        ))
    }
    fn ground(
        &mut self,
        _: &mut Container,
        _: Option<ItemHandle>,
        _: &Registry,
    ) -> inventory::Result<()> {
        Err(inventory::Error::Binding(
            "a ground drop from the appearance flag".into(),
        ))
    }
}

impl appearance::Context for Receivers<'_> {
    type Settings = Settings;
    type Properties = NodeHandle;
    fn npc(&mut self, _: i32) -> Result<npc::Handle> {
        refused("NPCDefinition.forId (transformNPC)")
    }
    fn size(&mut self, _: &NodeHandle, _: i32) -> Result<()> {
        refused("Player.setSize (transformNPC)")
    }
    fn settings(&mut self, player: &NodeHandle) -> Result<Self::Settings> {
        Ok(self.player(player)?.settings())
    }
    fn attack_style(&mut self, _: Self::Settings) -> Result<i32> {
        refused("Settings.getAttackStyleIndex (transformNPC)")
    }
    fn properties(&mut self, player: &NodeHandle) -> Result<Self::Properties> {
        self.player(player)?;
        Ok(player.clone())
    }
    fn combat_animation(
        &mut self,
        _: Self::Properties,
        _: CombatAnimation,
        _: Option<Animation>,
    ) -> Result<()> {
        refused("Properties combat animations (transformNPC)")
    }
    fn update_appearance(&mut self, player: &NodeHandle) -> Result<()> {
        self.player(player)?
            .masks()
            .sync_appearance(super::now_millis)
            .map(|_| ())
            .map_err(bind)
    }
    fn appearance(&mut self, player: &NodeHandle) -> Result<Appearance> {
        Ok(self.player(player)?.appearance())
    }
    fn animate(&mut self, _: &NodeHandle, _: Animation) -> Result<bool> {
        refused("Player.animate (flyCopter)")
    }
    fn attribute(
        &mut self,
        player: &NodeHandle,
        key: &str,
        default: Option<i32>,
    ) -> Result<Property> {
        // GameAttributes.getAttribute(key, default): the stored object, or
        // the default. A stored object that is not an Integer fails the
        // caller's cast, as it does here for any other Property.
        Ok(match self.player(player)?.attributes().get(key) {
            Some(AttributeValue::Int(value)) => Property::Int(value),
            Some(AttributeValue::Long(value)) => Property::Long(value),
            Some(AttributeValue::Bool(value)) => Property::Bool(value),
            Some(other) => Property::Text(other.java_string()),
            None => default.map_or(Property::Null, Property::Int),
        })
    }
}

impl appearance_writer::Context for Receivers<'_> {
    type Skull = NodeHandle;
    type WorldSettings = ();
    type Familiar = NodeHandle;
    type Skills = NodeHandle;
    fn skull(&mut self, player: &NodeHandle) -> Result<Self::Skull> {
        Ok(player.clone())
    }
    fn wilderness(&mut self, skull: Self::Skull) -> Result<bool> {
        Ok(self.skull_field(&skull, |s| s.wilderness))
    }
    fn wilderness_disabled(&mut self, skull: Self::Skull) -> Result<bool> {
        Ok(self.skull_field(&skull, |s| s.wilderness_disabled))
    }
    fn wilderness_level(&mut self, skull: Self::Skull) -> Result<i32> {
        Ok(self.skull_field(&skull, |s| s.level))
    }
    fn player_size(&mut self, player: &NodeHandle) -> Result<i32> {
        self.inner.state.node_size(player.node).map_err(bind)
    }
    fn username(&mut self, player: &NodeHandle) -> Result<String> {
        self.inner
            .players
            .get(&player.node)
            .map(|slot| slot.name.clone())
            .ok_or(Error::Null)
    }
    fn world_settings(&mut self) -> Result<Self::WorldSettings> {
        Ok(())
    }
    fn pvp(&mut self, (): ()) -> Result<bool> {
        Ok(self.inner.combat.pvp_world)
    }
    fn wild_pvp(&mut self, (): ()) -> Result<bool> {
        Ok(self.wild_pvp)
    }
    fn familiar(&mut self, player: &NodeHandle) -> Result<Self::Familiar> {
        Ok(player.clone())
    }
    fn using_summoning(&mut self, familiar: Self::Familiar) -> Result<bool> {
        // isUsingSummoning(): hasPouch || (hasFamiliar() && !(familiar is
        // Pet)); no familiar exists in this world.
        Ok(*self.player(&familiar)?.pouch().read())
    }
    fn summoning_combat_level(&mut self, familiar: Self::Familiar) -> Result<i32> {
        self.player(&familiar)?
            .with_profile(|profile| profile.summoning_combat_level)
            .map_err(bind)
    }
    fn combat_level(&mut self, properties: Self::Properties) -> Result<i32> {
        self.player(&properties)?
            .with_profile(|profile| profile.properties.combat_level)
            .map_err(bind)
    }
    fn current_combat_level(&mut self, properties: Self::Properties) -> Result<i32> {
        // Properties.getCurrentCombatLevel(): its own reads of the world
        // settings, the SkullManager and the FamiliarManager.
        let pvp_world = self.inner.combat.pvp_world;
        let wild_pvp = self.wild_pvp;
        let in_wilderness = wild_pvp && self.skull_field(&properties, |s| s.wilderness);
        let player = self.player(&properties)?;
        let using_summoning = *player.pouch().read();
        player
            .with_profile(|profile| {
                profile.properties.current_combat_level(CombatLevelInputs {
                    pvp_world,
                    wild_pvp,
                    in_wilderness,
                    using_summoning,
                    summoning_combat_level: profile.summoning_combat_level,
                })
            })
            .map_err(bind)
    }
    fn skills(&mut self, player: &NodeHandle) -> Result<Self::Skills> {
        Ok(player.clone())
    }
    fn total_level(&mut self, skills: Self::Skills) -> Result<i32> {
        self.player(&skills)?
            .with_profile(|profile| profile.skills.total_level())
            .map_err(bind)
    }
    fn invalid_type(&mut self, actual_type: Option<&str>) -> Result<()> {
        // The original logs and writes nothing; the live world only hands
        // the flag a player.
        Err(Error::Binding(format!(
            "PlayerFlags530.Appearance with a context of {actual_type:?}"
        )))
    }
}

/// `Appearance.setAnimations()` of a player from the EquipmentListener: the
/// weapon's stand, walk, run and turn animations and its render animation
/// (the appearance block's stance). Not called for a transformed player.
pub fn set_animations(inner: &Inner, p: &P) -> Result<()> {
    let appearance = inner.canonical(p).player.appearance();
    if appearance.is_npc() {
        return refused("Appearance.setAnimations of a transformed player");
    }
    let skulls = HashMap::new();
    appearance.set_animations(&mut Receivers {
        inner,
        skulls: &skulls,
        wild_pvp: false,
    })
}
