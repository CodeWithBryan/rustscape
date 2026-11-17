//! `RangedConfigLoader`: `ammo_configs.json` into `Ammunition.AMMUNITION` and
//! `ranged_weapon_configs.json` into `RangeWeapon.RANGE_WEAPONS`.
//!
//! The original builds each ammunition's projectile with
//! `Projectile.create(NPC(0, Location(0,0,0)), NPC(0, Location(0,0,0)), ...)`:
//! two throwaway NPC objects that are never registered. The pinned start-up
//! draws nothing in this loader (fixtures/startup-loaders), and nothing reads
//! those NPCs afterwards, so only the projectile's numbers are kept.
use super::{
    LoadError, Log, Result, array, int, kotlin_not_blank, list_get, object, parse, read_config,
    text,
};
use crate::definitions::{item::Registry as Items, property::Property};
use crate::world::drop_tables::LogLevel;
use rs09_game_core::java_json::Json;
use std::collections::HashMap;

/// `core.game.world.update.flag.context.Graphics(id, height)`: delay 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Graphics {
    pub id: i32,
    pub height: i32,
    pub delay: i32,
}

/// The numbers of the `Projectile` an ammunition carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectileTemplate {
    pub projectile_id: i32,
    pub start_height: i32,
    pub end_height: i32,
    pub start_delay: i32,
    pub speed: i32,
    pub angle: i32,
    pub distance: i32,
}

/// `BoltEffect`, in declaration order, with the item id `forId` matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoltEffect {
    Opal,
    Jade,
    Pearl,
    Topaz,
    Sapphire,
    Emerald,
    Ruby,
    Diamond,
    Dragon,
    Onyx,
}
impl BoltEffect {
    pub const ALL: [Self; 10] = [
        Self::Opal,
        Self::Jade,
        Self::Pearl,
        Self::Topaz,
        Self::Sapphire,
        Self::Emerald,
        Self::Ruby,
        Self::Diamond,
        Self::Dragon,
        Self::Onyx,
    ];
    pub const fn item_id(self) -> i32 {
        9236 + self as i32
    }
    pub const fn name(self) -> &'static str {
        match self {
            Self::Opal => "OPAL",
            Self::Jade => "JADE",
            Self::Pearl => "PEARL",
            Self::Topaz => "TOPAZ",
            Self::Sapphire => "SAPPHIRE",
            Self::Emerald => "EMERALD",
            Self::Ruby => "RUBY",
            Self::Diamond => "DIAMOND",
            Self::Dragon => "DRAGON",
            Self::Onyx => "ONYX",
        }
    }
    /// `BoltEffect.forId(id)`: the first constant with that item id.
    pub fn for_id(id: i32) -> Option<Self> {
        Self::ALL.into_iter().find(|e| e.item_id() == id)
    }
}

/// `core.game.node.entity.combat.equipment.Ammunition`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ammunition {
    pub item_id: i32,
    pub start_graphics: Graphics,
    pub dark_bow_graphics: Option<Graphics>,
    pub projectile: ProjectileTemplate,
    pub poison_damage: i32,
    pub effect: Option<BoltEffect>,
}

/// `core.game.node.entity.combat.equipment.RangeWeapon`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RangeWeapon {
    pub item_id: i32,
    /// The id of `Animation(animation)`.
    pub animation: i32,
    pub attack_speed: i32,
    pub ammunition_slot: i32,
    pub weapon_type: i32,
    pub drop_ammo: bool,
    pub ammunition: Vec<i32>,
}

/// `Ammunition.AMMUNITION` and `RangeWeapon.RANGE_WEAPONS`.
#[derive(Clone, Debug, Default)]
pub struct RangedTables {
    pub ammunition: HashMap<i32, Ammunition>,
    pub weapons: HashMap<i32, RangeWeapon>,
}

/// `load()` with `CONFIG_PATH`.
pub fn load_config(
    prefix: Option<&str>,
    items: &Items,
    tables: &mut RangedTables,
    log: &mut dyn Log,
) -> Result<()> {
    let text = read_config(prefix, "ammo_configs.json")?;
    load_ammunition(&text, tables, log)?;
    let text = read_config(prefix, "ranged_weapon_configs.json")?;
    load_weapons(&text, items, tables, log)?;
    Ok(())
}

/// The ammunition half of `load()`. A later row with the same item id replaces
/// the earlier one (`put`).
pub fn load_ammunition(source: &str, tables: &mut RangedTables, log: &mut dyn Log) -> Result<i32> {
    let mut count = 0i32;
    let data = parse(source)?;
    let configs = array(Some(&data))?;
    for entry in configs {
        let e = object(Some(entry))?;
        let mut dbowgfx = None;
        let projectile = text(e, "projectile");
        let projectile: Vec<&str> = projectile.split(',').collect();
        let darkbow = text(e, "darkbow_graphic");
        if kotlin_not_blank(&darkbow) {
            let darkbow: Vec<&str> = darkbow.split(',').collect();
            let id = int(list_get(&darkbow, 0)?)?;
            let height = int(list_get(&darkbow, 1)?)?;
            dbowgfx = Some(Graphics {
                id,
                height,
                delay: 0,
            });
        }
        let gfx = text(e, "start_graphic");
        let gfx: Vec<&str> = gfx.split(',').collect();
        // `Integer.parseInt(e["itemId"] as String)`.
        let item_id = match e.get("itemId") {
            None | Some(Json::Null) => return Err(LoadError::Null),
            Some(Json::Text(value)) => int(value)?,
            Some(_) => return Err(LoadError::Type),
        };
        let start_graphics = Graphics {
            id: int(list_get(&gfx, 0)?)?,
            height: int(list_get(&gfx, 1)?)?,
            delay: 0,
        };
        let p = |i: usize| -> Result<i32> { int(list_get(&projectile, i)?) };
        let projectile = ProjectileTemplate {
            projectile_id: p(0)?,
            start_height: p(1)?,
            end_height: p(2)?,
            start_delay: p(3)?,
            speed: p(4)?,
            angle: p(5)?,
            distance: p(6)?,
        };
        let poison_damage = int(&text(e, "poison_damage"))?;
        let mut ammo = Ammunition {
            item_id,
            start_graphics,
            dark_bow_graphics: dbowgfx,
            projectile,
            poison_damage,
            effect: None,
        };
        let effect = BoltEffect::for_id(int(&text(e, "itemId"))?);
        if effect.is_some() {
            ammo.effect = effect;
        }
        tables.ammunition.insert(int(&text(e, "itemId"))?, ammo);
        count = count.wrapping_add(1);
    }
    log.log(
        "RangedConfigLoader",
        LogLevel::Fine,
        &format!("Parsed {count} ammo configs..."),
    );
    Ok(count)
}

/// `ItemDefinition.forId(id).getConfiguration("attack_speed", 4)`. An id the
/// registry does not hold is a fresh `ItemDefinition` with no handlers.
fn attack_speed(items: &Items, id: i32) -> Result<i32> {
    let def = items.for_id(id);
    let def = def.read();
    match def.handlers.get("attack_speed") {
        None | Some(Property::Null) => Ok(4),
        Some(Property::Int(value)) => Ok(*value),
        Some(_) => Err(LoadError::Type),
    }
}

/// The weapon half of `load()`. The first row for an item id wins (`putIfAbsent`).
pub fn load_weapons(
    source: &str,
    items: &Items,
    tables: &mut RangedTables,
    log: &mut dyn Log,
) -> Result<i32> {
    let mut count = 0i32;
    let data = parse(source)?;
    let configs = array(Some(&data))?;
    for entry in configs {
        let e = object(Some(entry))?;
        let id = int(&text(e, "itemId"))?;
        let animation = int(&text(e, "animation"))?;
        let attack_speed = attack_speed(items, id)?;
        let ammunition_slot = int(&text(e, "ammo_slot"))?;
        let weapon_type = int(&text(e, "weapon_type"))?;
        // Kotlin `String.toBoolean()`: "true" in any case.
        let drop_ammo = text(e, "drop_ammo").eq_ignore_ascii_case("true");
        let ammunition = text(e, "ammunition")
            .split(',')
            .map(int)
            .collect::<Result<Vec<i32>>>()?;
        let weapon = RangeWeapon {
            item_id: id,
            animation,
            attack_speed,
            ammunition_slot,
            weapon_type,
            drop_ammo,
            ammunition,
        };
        tables.weapons.entry(weapon.item_id).or_insert(weapon);
        count = count.wrapping_add(1);
    }
    log.log(
        "RangedConfigLoader",
        LogLevel::Fine,
        &format!("Parsed {count} ranged weapon configs..."),
    );
    Ok(count)
}
