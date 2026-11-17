//! PlayerFlags530.Appearance control flow over the actual native Appearance.
//! Required world/player receivers remain live; no prepared-data projection.
use super::appearance::{Context as AppearanceContext, Error, Result};
use crate::{definitions::item::Shared, world::player_walk::NodeHandle};

/// The mutable byte-output subset used by this flag. Clones share cursor/data,
/// including overwrites during reentry. This is not the whole IoBuffer API.
#[derive(Clone)]
pub struct Buffer(Shared<Output>);
struct Output {
    bytes: Vec<u8>,
    position: usize,
}
impl Default for Buffer {
    fn default() -> Self {
        Self::from_bytes(Vec::new())
    }
}
impl Buffer {
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        let position = bytes.len();
        Self(Shared::new(Output { bytes, position }))
    }
    pub fn position(&self) -> usize {
        self.0.read().position
    }
    pub fn set_position(&self, position: usize) -> Result<()> {
        let mut output = self.0.write();
        if position > output.bytes.len() {
            return Err(Error::Index);
        }
        output.position = position;
        Ok(())
    }
    pub fn bytes(&self) -> Vec<u8> {
        let output = self.0.read();
        output.bytes[..output.position].to_vec()
    }
    pub fn p1(&self, value: i32) {
        self.put(&[value as u8]);
    }
    fn put(&self, bytes: &[u8]) {
        let mut output = self.0.write();
        let position = output.position;
        let end = position + bytes.len();
        if end > output.bytes.len() {
            output.bytes.resize(end, 0);
        }
        output.bytes[position..end].copy_from_slice(bytes);
        output.position = end;
    }
    pub fn p2(&self, value: i32) {
        self.put(&(value as u16).to_be_bytes());
    }
    pub fn p8(&self, value: i64) {
        self.put(&value.to_be_bytes());
    }
    pub fn psize_add(&self, length: i32) -> Result<()> {
        let mut output = self.0.write();
        let index = (output.position as i32)
            .wrapping_sub(length)
            .wrapping_sub(1);
        *output.bytes.get_mut(index as usize).ok_or(Error::Index)? = length.wrapping_add(128) as u8;
        Ok(())
    }
}

pub trait Context: AppearanceContext {
    type Skull;
    type WorldSettings;
    type Familiar;
    type Skills;
    fn skull(&mut self, player: &NodeHandle) -> Result<Self::Skull>;
    fn wilderness(&mut self, skull: Self::Skull) -> Result<bool>;
    fn wilderness_disabled(&mut self, skull: Self::Skull) -> Result<bool>;
    fn wilderness_level(&mut self, skull: Self::Skull) -> Result<i32>;
    fn player_size(&mut self, player: &NodeHandle) -> Result<i32>;
    fn username(&mut self, player: &NodeHandle) -> Result<String>;
    fn world_settings(&mut self) -> Result<Self::WorldSettings>;
    fn pvp(&mut self, settings: Self::WorldSettings) -> Result<bool>;
    fn wild_pvp(&mut self, settings: Self::WorldSettings) -> Result<bool>;
    fn familiar(&mut self, player: &NodeHandle) -> Result<Self::Familiar>;
    fn using_summoning(&mut self, familiar: Self::Familiar) -> Result<bool>;
    fn summoning_combat_level(&mut self, familiar: Self::Familiar) -> Result<i32>;
    fn combat_level(&mut self, properties: Self::Properties) -> Result<i32>;
    fn current_combat_level(&mut self, properties: Self::Properties) -> Result<i32>;
    fn skills(&mut self, player: &NodeHandle) -> Result<Self::Skills>;
    fn total_level(&mut self, skills: Self::Skills) -> Result<i32>;
    /// Native flag dispatch supplies the actual invalid context type for logging.
    fn invalid_type(&mut self, actual_type: Option<&str>) -> Result<()>;
}

/// A typed registry selects Player or Invalid. Type names are logging metadata,
/// never runtime IDs or dispatch keys. Full flag registry startup is separate.
pub enum Input<'a> {
    Player(&'a NodeHandle),
    Invalid(Option<&'a str>),
}

pub fn write_to<C: Context>(ctx: &mut C, buffer: Option<&Buffer>, input: Input<'_>) -> Result<()> {
    // Kotlin checks the nonnull buffer parameter before examining context.
    let buffer = buffer.ok_or(Error::Null)?;
    let player = match input {
        Input::Player(player) => player,
        Input::Invalid(kind) => return ctx.invalid_type(kind),
    };
    let appearance = ctx.appearance(player)?;
    appearance.prepare_body(ctx, Some(player))?;
    let mut settings = i32::from(appearance.gender().ok_or(Error::Null)?.byte());
    let skull = ctx.skull(player)?;
    let non_pvp = if ctx.wilderness(skull)? {
        let skull = ctx.skull(player)?;
        ctx.wilderness_disabled(skull)?
    } else {
        false
    };
    if ctx.player_size(player)? > 1 {
        let size = ctx.player_size(player)?;
        settings = settings.wrapping_add(size.wrapping_sub(1).wrapping_shl(3));
    }
    if non_pvp {
        settings = settings.wrapping_add(4);
    }
    buffer.p1(0);
    let start = buffer.position();
    buffer.p1(settings);
    buffer.p1(appearance.skull_icon());
    buffer.p1(appearance.head_icon());
    let npc_id = appearance.npc_id();
    if npc_id == -1 {
        let parts = appearance.body();
        for i in 0..12 {
            if parts.read()[i] == 0 {
                buffer.p1(0);
            } else {
                let part = parts.read()[i];
                buffer.p2(part);
            }
        }
    } else {
        buffer.p2(-1);
        buffer.p2(npc_id);
        buffer.p1(255);
    }
    // Kotlin constructs the entire array before dereferencing its first element.
    let colors = [
        appearance.hair(),
        appearance.torso(),
        appearance.legs(),
        appearance.feet(),
        appearance.skin(),
    ];
    for part in colors {
        let color = part.ok_or(Error::Null)?.read().color;
        buffer.p1(color);
    }
    buffer.p2(appearance.render_animation(ctx)?);
    let username = ctx.username(player)?;
    buffer.p8(crate::world::appearance::name_long(&username));
    if non_pvp {
        let properties = ctx.properties(player)?;
        buffer.p1(ctx.current_combat_level(properties)?);
        let skills = ctx.skills(player)?;
        buffer.p2(ctx.total_level(skills)?);
    } else {
        let world = ctx.world_settings()?;
        let separate = if ctx.pvp(world)? {
            true
        } else {
            let world = ctx.world_settings()?;
            if ctx.wild_pvp(world)? {
                let skull = ctx.skull(player)?;
                ctx.wilderness(skull)?
            } else {
                false
            }
        };
        let separate = if separate {
            let familiar = ctx.familiar(player)?;
            !ctx.using_summoning(familiar)?
        } else {
            false
        };
        if separate {
            let properties = ctx.properties(player)?;
            buffer.p1(ctx.combat_level(properties)?);
            let properties = ctx.properties(player)?;
            let combat = ctx.combat_level(properties)?;
            let familiar = ctx.familiar(player)?;
            buffer.p1(combat.wrapping_add(ctx.summoning_combat_level(familiar)?));
        } else {
            let properties = ctx.properties(player)?;
            buffer.p1(ctx.current_combat_level(properties)?);
            let properties = ctx.properties(player)?;
            buffer.p1(ctx.current_combat_level(properties)?);
        }
        let world = ctx.world_settings()?;
        if ctx.pvp(world)? {
            let skull = ctx.skull(player)?;
            buffer.p1(ctx.wilderness_level(skull)?);
        } else {
            buffer.p1(-1);
        }
    }
    buffer.p1(0);
    buffer.psize_add((buffer.position() as i32).wrapping_sub(start as i32))
}
