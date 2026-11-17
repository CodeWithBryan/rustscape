//! The other files of `core.api`: Container, EquipmentSlot, God, InputType,
//! ContentAPI's GAMETAB, IfaceSettingsBuilder, ApiExtensions, Commands,
//! Event, MapArea, the listener interfaces, `core.tools.colorize`,
//! `Direction.getDirection`, `utils.Vector` and the regionspec DSL.
//! DialUtils' `splitLines` is [`crate::world::plain_dialogue::split_lines`]
//! (wrapped as [`super::ui::split_lines`]).
use super::{Arg, Error, Host, Location, Member as M, Result, b, n, nd, new_item, unit};

/// `core.api.Container`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Container {
    Inventory,
    Bank,
    Equipment,
    BoB,
}

/// `core.api.EquipmentSlot`; the ordinal is the equipment slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EquipmentSlot {
    Head,
    Cape,
    Neck,
    Weapon,
    Chest,
    Shield,
    Hidden1,
    Legs,
    Hidden2,
    Hands,
    Feet,
    Hidden3,
    Ring,
    Ammo,
}
impl EquipmentSlot {
    pub const ALL: [Self; 14] = [
        Self::Head,
        Self::Cape,
        Self::Neck,
        Self::Weapon,
        Self::Chest,
        Self::Shield,
        Self::Hidden1,
        Self::Legs,
        Self::Hidden2,
        Self::Hands,
        Self::Feet,
        Self::Hidden3,
        Self::Ring,
        Self::Ammo,
    ];
    pub fn name(self) -> &'static str {
        [
            "HEAD", "CAPE", "NECK", "WEAPON", "CHEST", "SHIELD", "HIDDEN_1", "LEGS", "HIDDEN_2",
            "HANDS", "FEET", "HIDDEN_3", "RING", "AMMO",
        ][self as usize]
    }
    /// `values().getOrNull(ordinal)`.
    pub fn from_ordinal(ordinal: i32) -> Option<Self> {
        usize::try_from(ordinal)
            .ok()
            .and_then(|o| Self::ALL.get(o).copied())
    }
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.name() == name)
    }
    /// `EquipmentSlot.slotByName(input)` (ported in
    /// [`crate::world::commands::system::slot_by_name`]).
    pub fn slot_by_name(input: &str) -> Option<Self> {
        crate::world::commands::system::slot_by_name(input).and_then(Self::from_ordinal)
    }
}

/// `core.api.God` and its `validItems` (ConstLib 1.4.28 ids).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum God {
    Armadyl,
    Bandos,
    Saradomin,
    Zamorak,
    Guthix,
}
const ARMADYL: [i32; 7] = [11720, 11694, 11718, 12671, 12670, 87, 11722];
const BANDOS: [i32; 5] = [11061, 11728, 11724, 11696, 11726];
const SARADOMIN: [i32; 25] = [
    3839, 3840, 1718, 542, 544, 10384, 2412, 10388, 10446, 10390, 10440, 10386, 2665, 11698, 2667,
    10452, 6762, 2661, 2663, 3479, 10464, 10458, 2415, 10470, 11730,
];
const ZAMORAK: [i32; 28] = [
    3841, 3842, 1724, 10368, 2414, 10372, 10450, 10374, 10444, 10370, 10790, 2657, 11700, 11716,
    2659, 10456, 6764, 10776, 2653, 2655, 3478, 10468, 10460, 10786, 2417, 10474, 1033, 1035,
];
const GUTHIX: [i32; 23] = [
    3843, 3844, 10376, 10720, 10380, 10448, 10382, 10442, 10378, 10794, 2673, 2675, 10454, 6760,
    10780, 2669, 2671, 3676, 10466, 10462, 10788, 2416, 10472,
];
impl God {
    pub const ALL: [Self; 5] = [
        Self::Armadyl,
        Self::Bandos,
        Self::Saradomin,
        Self::Zamorak,
        Self::Guthix,
    ];
    pub fn name(self) -> &'static str {
        ["ARMADYL", "BANDOS", "SARADOMIN", "ZAMORAK", "GUTHIX"][self as usize]
    }
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|g| g.name() == name)
    }
    pub fn valid_items(self) -> &'static [i32] {
        match self {
            Self::Armadyl => &ARMADYL,
            Self::Bandos => &BANDOS,
            Self::Saradomin => &SARADOMIN,
            Self::Zamorak => &ZAMORAK,
            Self::Guthix => &GUTHIX,
        }
    }
}

/// `core.api.InputType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputType {
    Amount,
    Numeric,
    StringShort,
    StringLong,
    Message,
}
impl InputType {
    pub const ALL: [Self; 5] = [
        Self::Amount,
        Self::Numeric,
        Self::StringShort,
        Self::StringLong,
        Self::Message,
    ];
    pub fn name(self) -> &'static str {
        [
            "AMOUNT",
            "NUMERIC",
            "STRING_SHORT",
            "STRING_LONG",
            "MESSAGE",
        ][self as usize]
    }
}

/// ContentAPI's `GAMETAB`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameTab {
    Combat,
    Stats,
    Quests,
    Inventory,
    Equipment,
    Prayer,
    Magic,
    Follower,
    Friends,
    Ignore,
    Clan,
    Settings,
    Emotes,
    Music,
    Logout,
    AmeDefault,
}
impl GameTab {
    pub const ALL: [Self; 16] = [
        Self::Combat,
        Self::Stats,
        Self::Quests,
        Self::Inventory,
        Self::Equipment,
        Self::Prayer,
        Self::Magic,
        Self::Follower,
        Self::Friends,
        Self::Ignore,
        Self::Clan,
        Self::Settings,
        Self::Emotes,
        Self::Music,
        Self::Logout,
        Self::AmeDefault,
    ];
    pub fn name(self) -> &'static str {
        [
            "COMBAT",
            "STATS",
            "QUESTS",
            "INVENTORY",
            "EQUIPMENT",
            "PRAYER",
            "MAGIC",
            "FOLLOWER",
            "FRIENDS",
            "IGNORE",
            "CLAN",
            "SETTINGS",
            "EMOTES",
            "MUSIC",
            "LOGOUT",
            "AME_DEFAULT",
        ][self as usize]
    }
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.name() == name)
    }
    /// The tab indices.
    pub fn indices(self) -> &'static [i32] {
        const ONE: [i32; 15] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14];
        match self {
            Self::AmeDefault => &[0, 1, 2, 3, 4, 5, 6, 7, 11, 12],
            other => std::slice::from_ref(&ONE[other as usize]),
        }
    }
}

/// `Direction` in declaration order, with `getDirection(diffX, diffY)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    NorthWest,
    North,
    NorthEast,
    West,
    East,
    SouthWest,
    South,
    SouthEast,
}
impl Direction {
    pub const ALL: [Self; 8] = [
        Self::NorthWest,
        Self::North,
        Self::NorthEast,
        Self::West,
        Self::East,
        Self::SouthWest,
        Self::South,
        Self::SouthEast,
    ];
    pub fn name(self) -> &'static str {
        [
            "NORTH_WEST",
            "NORTH",
            "NORTH_EAST",
            "WEST",
            "EAST",
            "SOUTH_WEST",
            "SOUTH",
            "SOUTH_EAST",
        ][self as usize]
    }
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.name() == name)
    }
    /// `Direction.getDirection(diffX, diffY)`.
    pub fn get(dx: i32, dy: i32) -> Self {
        if dx < 0 {
            if dy < 0 {
                Self::SouthWest
            } else if dy > 0 {
                Self::NorthWest
            } else {
                Self::West
            }
        } else if dx > 0 {
            if dy < 0 {
                Self::SouthEast
            } else if dy > 0 {
                Self::NorthEast
            } else {
                Self::East
            }
        } else if dy < 0 {
            Self::South
        } else {
            Self::North
        }
    }
}

/// `core.tools.colorize(line)`: the `%R`-style codes and `%rrggbb` colours,
/// then `</col>` and a space.
pub fn colorize(line: &str) -> String {
    let named = line
        .replace("%R", "<col=ff0000>")
        .replace("%O", "<col=ff6600>")
        .replace("%Y", "<col=ffff00>")
        .replace("%G", "<col=66ff33>")
        .replace("%B", "<col=3366ff>")
        .replace("%P", "<col=cc00ff>");
    let chars: Vec<char> = named.chars().collect();
    let mut out = String::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '%'
            && index + 7 <= chars.len()
            && chars[index + 1..index + 7]
                .iter()
                .all(|c| c.is_ascii_hexdigit())
        {
            out.push_str("<col=");
            out.extend(&chars[index + 1..index + 7]);
            out.push('>');
            index += 7;
        } else {
            out.push(chars[index]);
            index += 1;
        }
    }
    out + "</col> "
}

/// Java's `Double.toString`.
pub fn java_double_string(value: f64) -> String {
    rs09_game_core::java_double::format(value)
}

/// `core.api.IfaceSettingsBuilder`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IfaceSettingsBuilder {
    value: i32,
}
impl IfaceSettingsBuilder {
    pub fn enable_option(mut self, option: i32) -> Result<Self> {
        if !(0..=9).contains(&option) {
            return Err(Error::Argument("Option index must be 0-9.".into()));
        }
        self.value |= 1 << (option + 1);
        Ok(self)
    }
    /// `enableOptions(vararg ids: Int)`.
    pub fn enable_options(mut self, ids: &[i32]) -> Result<Self> {
        for id in ids {
            self = self.enable_option(*id)?;
        }
        Ok(self)
    }
    /// `enableOptions(ids: IntRange)` for `start until end`.
    pub fn enable_options_range(mut self, start: i32, end: i32) -> Result<Self> {
        for id in start..end {
            self = self.enable_option(id)?;
        }
        Ok(self)
    }
    /// `enableOptions(vararg options: String?)`: one option per index.
    pub fn enable_named_options(mut self, count: usize) -> Result<Self> {
        for id in 0..count as i32 {
            self = self.enable_option(id)?;
        }
        Ok(self)
    }
    pub fn enable_all_options(self) -> Result<Self> {
        self.enable_options_range(0, 10)
    }
    pub fn set_use_on_settings(
        mut self,
        ground: bool,
        npcs: bool,
        objects: bool,
        other: bool,
        own: bool,
        component: bool,
    ) -> Self {
        let mut flag = 0;
        for (on, bit) in [
            (ground, 0x1),
            (npcs, 0x2),
            (objects, 0x4),
            (other, 0x8),
            (own, 0x10),
            (component, 0x20),
        ] {
            if on {
                flag |= bit;
            }
        }
        self.value |= flag << 11;
        self
    }
    pub fn set_interface_events_depth(mut self, depth: i32) -> Result<Self> {
        if !(0..=7).contains(&depth) {
            return Err(Error::Argument("depth must be 0-7.".into()));
        }
        self.value &= !(0x7 << 18);
        self.value |= depth << 18;
        Ok(self)
    }
    pub fn enable_slot_switch(mut self) -> Self {
        self.value |= 1 << 21;
        self
    }
    pub fn enable_use_option(mut self) -> Self {
        self.value |= 1 << 17;
        self
    }
    pub fn enable_examine(mut self) -> Self {
        self.value |= 1 << 9;
        self
    }
    pub fn enable_use_on_self(mut self) -> Self {
        self.value |= 1 << 22;
        self
    }
    pub fn enable_null_slot_switch(mut self) -> Self {
        self.value |= 1 << 23;
        self
    }
    pub fn build(self) -> i32 {
        self.value
    }
}

// ---------------------------------------------------------------- ApiExtensions

/// `Int.asItem()`: `Item(this)`.
pub fn as_item<H: Host + ?Sized>(h: &mut H, id: i32) -> Result<H::Node> {
    new_item(h, id, 1, 1000)
}

/// Kotlin's `getProgressionLastElement`.
fn progression_last(start: i32, end: i32, step: i32) -> i32 {
    let modulo = |a: i32, b: i32| {
        let m = a % b;
        if m >= 0 { m } else { m + b }
    };
    let difference = |a: i32, b: i32, c: i32| modulo(modulo(a, c) - modulo(b, c), c);
    if step > 0 {
        if start >= end {
            end
        } else {
            end - difference(end, start, step)
        }
    } else if start <= end {
        end
    } else {
        end + difference(start, end, -step)
    }
}

/// `IntProgression.toIntArray()` of `IntProgression.fromClosedRange(first,
/// last, step)`: an array of `(last - first) / step + 1` elements.
pub fn progression_to_int_array(first: i32, last: i32, step: i32) -> Result<Vec<i32>> {
    let last = progression_last(first, last, step);
    let size = last.wrapping_sub(first) / step + 1;
    let size =
        usize::try_from(size).map_err(|_| Error::Argument("NegativeArraySizeException".into()))?;
    let mut out = vec![0; size];
    let mut value = first;
    for slot in out.iter_mut() {
        *slot = value;
        value = value.wrapping_add(step);
    }
    Ok(out)
}

/// `Collection<IntArray>.toIntArray()`.
pub fn flatten_int_arrays(arrays: &[Vec<i32>]) -> Vec<i32> {
    arrays.iter().flatten().copied().collect()
}

/// `isLast(element)`: `indexOf(element) == size - 1`.
pub fn is_last<T: PartialEq>(values: &[T], element: &T) -> bool {
    values
        .iter()
        .position(|v| v == element)
        .map_or(-1, |i| i as i64)
        == values.len() as i64 - 1
}

/// `getNext(element)`: the next element, or the element itself.
pub fn get_next<T: PartialEq + Clone>(values: &[T], element: &T) -> T {
    let index = values
        .iter()
        .position(|v| v == element)
        .map_or(-1, |i| i as i64);
    if index < values.len() as i64 - 1 {
        values[(index + 1) as usize].clone()
    } else {
        element.clone()
    }
}

/// `isNextLast(element)`.
pub fn is_next_last<T: PartialEq + Clone>(values: &[T], element: &T) -> bool {
    is_last(values, &get_next(values, element))
}

/// `LinkedList.tryPop(default)`.
pub fn try_pop<T>(list: &mut std::collections::VecDeque<T>, default: Option<T>) -> Option<T> {
    if list.is_empty() {
        return default;
    }
    list.pop_front()
}

/// `parseEnumEntry<E>(name)`: the exact constant name, else null.
pub fn parse_enum_entry<T: Copy>(
    values: &[T],
    name_of: impl Fn(T) -> &'static str,
    name: &str,
) -> Option<T> {
    values.iter().copied().find(|v| name_of(*v) == name)
}

// ---------------------------------------------------------------- Commands

/// `Privilege`.
pub const PRIVILEGES: [&str; 3] = ["STANDARD", "MODERATOR", "ADMIN"];

/// `Commands.reject(player, messages...)`: each message, then
/// `IllegalStateException`.
pub fn reject<H: Host + ?Sized>(h: &mut H, p: &H::Node, messages: &[String]) -> Result<()> {
    for message in messages {
        unit(
            h,
            M::PlayerSendMessage,
            vec![nd(p), Arg::Text(colorize(&format!("-->%R{message}")))],
        )?;
    }
    Err(Error::State(String::new()))
}

/// `Commands.notify(player, message, logToConsole = false)`; `origin` is the
/// implementing class's simple name.
pub fn notify<H: Host + ?Sized>(
    h: &mut H,
    origin: &str,
    p: &H::Node,
    message: &str,
    log_to_console: bool,
) -> Result<()> {
    if log_to_console {
        super::services::log(h, origin, super::services::LogLevel::Debug, message)?;
    }
    unit(
        h,
        M::PlayerSendMessage,
        vec![nd(p), Arg::Text(colorize(&format!("-->{message}")))],
    )
}

/// `Commands.define(name, privilege = ADMIN, usage = "", description = "", handle)`.
pub fn define<H: Host + ?Sized>(
    h: &mut H,
    name: &str,
    privilege: &'static str,
    usage: &str,
    description: &str,
    handle: &H::Node,
) -> Result<()> {
    let command = Arg::List(vec![
        Arg::text("Command"),
        Arg::text(name),
        Arg::text(privilege),
        Arg::text(usage),
        Arg::text(description),
        nd(handle),
    ]);
    unit(h, M::CommandRegister, vec![command])
}

// ---------------------------------------------------------------- Event, listeners, MapArea

/// `core.api.Event`: the event classes in declaration order.
pub const EVENTS: [&str; 35] = [
    "ResourceProducedEvent",
    "NPCKillEvent",
    "BoneBuryEvent",
    "TeleportEvent",
    "LitFireEvent",
    "LitLightSourceEvent",
    "InteractionEvent",
    "ButtonClickEvent",
    "DialogueOpenEvent",
    "DialogueOptionSelectionEvent",
    "DialogueCloseEvent",
    "UseWithEvent",
    "SelfDeathEvent",
    "TickEvent",
    "PickUpEvent",
    "InterfaceOpenEvent",
    "InterfaceCloseEvent",
    "AttributeSetEvent",
    "AttributeRemoveEvent",
    "SpellCastEvent",
    "SpellbookChangeEvent",
    "ItemAlchemizationEvent",
    "ItemEquipEvent",
    "ItemUnequipEvent",
    "ItemShopPurchaseEvent",
    "ItemShopSellEvent",
    "JobAssignmentEvent",
    "FairyRingDialEvent",
    "VarbitUpdateEvent",
    "DynamicSkillLevelChangeEvent",
    "SummoningPointsRechargeEvent",
    "PrayerPointsRechargeEvent",
    "XPGainEvent",
    "PrayerActivatedEvent",
    "PrayerDeactivatedEvent",
];

/// `ContentInterface` and the listener interfaces content implements.
pub trait ContentInterface {}
pub trait LoginListener<N>: ContentInterface {
    fn login(&mut self, player: &N) -> Result<()>;
}
pub trait LogoutListener<N>: ContentInterface {
    fn logout(&mut self, player: &N) -> Result<()>;
}
pub trait PersistPlayer<N, J>: ContentInterface {
    fn save_player(&mut self, player: &N, save: &mut J) -> Result<()>;
    fn parse_player(&mut self, player: &N, data: &J) -> Result<()>;
}
pub trait PersistWorld: ContentInterface {
    fn save(&mut self) -> Result<()>;
    fn parse(&mut self) -> Result<()>;
}
pub trait ShutdownListener: ContentInterface {
    fn shutdown(&mut self) -> Result<()>;
}
pub trait StartupListener: ContentInterface {
    fn startup(&mut self) -> Result<()>;
}
pub trait TickListener: ContentInterface {
    fn tick(&mut self) -> Result<()>;
}

/// `MapArea`: a content area whose zone is kept in the shared `zoneMaps`
/// under `<simple class name>MapArea`.
pub trait MapArea<N>: ContentInterface {
    /// `this::class.java.simpleName`.
    fn simple_name(&self) -> &str;
    fn define_area_borders(&self) -> Vec<[i32; 5]>;
    fn get_restrictions(&self) -> Vec<&'static str> {
        Vec::new()
    }
    fn area_enter(&mut self, _entity: &N) -> Result<()> {
        Ok(())
    }
    fn area_leave(&mut self, _entity: &N, _logout: bool) -> Result<()> {
        Ok(())
    }
    fn entity_step(&mut self, _entity: &N, _location: Location, _last: Location) -> Result<()> {
        Ok(())
    }
    fn is_pvp_allowed(
        &mut self,
        _entity: &N,
        _target: Option<&N>,
        _style: Option<&str>,
        _message: bool,
    ) -> Option<bool> {
        None
    }
    fn can_start_death(&mut self, _entity: &N, _killer: &N) -> Option<bool> {
        None
    }
}

/// `MapArea.zone`'s getter: `zoneMaps[name + "MapArea"]!!`.
pub fn map_area_zone<H: Host + ?Sized>(h: &mut H, simple_name: &str) -> Result<H::Node> {
    let key = format!("{simple_name}MapArea");
    h.zone_maps()
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, z)| z.clone())
        .ok_or(Error::Null)
}

/// `MapArea.zone`'s setter.
pub fn set_map_area_zone<H: Host + ?Sized>(h: &mut H, simple_name: &str, zone: H::Node) {
    let key = format!("{simple_name}MapArea");
    let maps = h.zone_maps();
    match maps.iter_mut().find(|(k, _)| *k == key) {
        Some(entry) => entry.1 = zone,
        None => maps.push((key, zone)),
    }
}

// ---------------------------------------------------------------- Vector

/// `core.api.utils.Vector`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vector {
    pub x: f64,
    pub y: f64,
}
impl Vector {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
    pub fn magnitude(self) -> f64 {
        // Math.pow(v, 2.0) is the correctly rounded square.
        (self.x * self.x + self.y * self.y).sqrt()
    }
    pub fn normalized(self) -> Self {
        let m = self.magnitude();
        Self::new(self.x / m, self.y / m)
    }
    pub fn times(self, by: f64) -> Self {
        Self::new(self.x * by, self.y * by)
    }
    pub fn times_int(self, by: i32) -> Self {
        Self::new(self.x * by as f64, self.y * by as f64)
    }
    pub fn plus(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y)
    }
    pub fn minus(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y)
    }
    pub fn invert(self) -> Self {
        Self::new(-self.x, -self.y)
    }
    /// `toLocation(plane = 0)`.
    pub fn to_location(self, plane: i32) -> Location {
        Location::new(
            super::java_double_to_int(self.x.floor()),
            super::java_double_to_int(self.y.floor()),
            plane,
        )
    }
    /// `toDirection()`.
    pub fn to_direction(self) -> Direction {
        let n = self.normalized();
        if n.x >= 0.85 {
            return Direction::East;
        } else if n.x <= -0.85 {
            return Direction::West;
        }
        if n.y > 0.0 {
            if n.y >= 0.85 {
                return Direction::North;
            }
            if n.x > 0.0 {
                Direction::NorthEast
            } else {
                Direction::NorthWest
            }
        } else {
            if n.y <= -0.85 {
                return Direction::South;
            }
            if n.x > 0.0 {
                Direction::SouthEast
            } else {
                Direction::SouthWest
            }
        }
    }
    /// `Vector.betweenLocs(from, to)`.
    pub fn between_locs(from: Location, to: Location) -> Self {
        Self::new(
            to.x().wrapping_sub(from.x()) as f64,
            to.y().wrapping_sub(from.y()) as f64,
        )
    }
    /// `Vector.deriveWithEqualComponents(magnitude)`.
    pub fn derive_with_equal_components(magnitude: f64) -> Self {
        let side = (magnitude * magnitude / 2.0).sqrt();
        Self::new(side, side)
    }
}
impl std::fmt::Display for Vector {
    /// `{x,y}` with Java's double text.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{{},{}}}",
            java_double_string(self.x),
            java_double_string(self.y)
        )
    }
}

// ---------------------------------------------------------------- regionspec

/// A region contract.
#[derive(Clone, Debug, PartialEq)]
pub enum RegionContract<N> {
    /// `EmptyRegionContract`: an 8x8 reserved area.
    Empty,
    /// `CloneRegionContract(regionId)` (`copyOf`).
    Clone(i32),
    /// `UseExistingRegionContract(region)` (`using`).
    Existing(N),
}

/// `FillChunkContract` (`fillWith`).
#[derive(Clone, Debug, PartialEq)]
pub struct FillChunk<N> {
    pub chunk: Option<N>,
    /// `chunkDelegate`: a content callback `(x, y, plane, region) -> chunk`
    /// (`None`: `{ _, _, _, _ -> chunk }`).
    pub delegate: Option<N>,
    /// `sourceRegion` (`lateinit`, set by `from`).
    pub source: Option<N>,
    pub planes: Vec<i32>,
    /// `replaceCondition` (`None`: always true).
    pub condition: Option<N>,
}
impl<N> FillChunk<N> {
    pub fn with_chunk(chunk: Option<N>) -> Self {
        Self {
            chunk,
            delegate: None,
            source: None,
            planes: vec![0],
            condition: None,
        }
    }
    pub fn with_delegate(delegate: N) -> Self {
        Self {
            chunk: None,
            delegate: Some(delegate),
            source: None,
            planes: vec![0],
            condition: None,
        }
    }
}

/// A chunk contract.
#[derive(Clone, Debug, PartialEq)]
pub enum ChunkContract<N> {
    Empty,
    Fill(FillChunk<N>),
}

/// `RegionSpecification(regionContract, chunkContracts...).build()`.
pub fn build_region<H: Host + ?Sized>(
    h: &mut H,
    region: &RegionContract<H::Node>,
    chunks: &[ChunkContract<H::Node>],
) -> Result<H::Node> {
    let dynamic = match region {
        RegionContract::Empty => {
            let borders = h
                .call(M::DynamicRegionReserve, vec![Arg::Int(8), Arg::Int(8)])?
                .borders()?;
            let borders = borders.map_or(Arg::Null, Arg::borders);
            n(h, M::DynamicRegionNew, vec![borders])?.ok_or(Error::Null)?
        }
        RegionContract::Clone(id) => {
            n(h, M::DynamicRegionCreate, vec![Arg::Int(*id)])?.ok_or(Error::Null)?
        }
        RegionContract::Existing(r) => r.clone(),
    };
    unit(h, M::RegionLoad, vec![nd(&dynamic)])?;
    for contract in chunks {
        if let ChunkContract::Fill(fill) = contract {
            populate(h, fill, &dynamic)?;
        }
    }
    Ok(dynamic)
}

/// `FillChunkContract.populateChunks(dyn)`.
fn populate<H: Host + ?Sized>(
    h: &mut H,
    fill: &FillChunk<H::Node>,
    dynamic: &H::Node,
) -> Result<()> {
    for plane in &fill.planes {
        for x in 0..8 {
            for y in 0..8 {
                let replace = match &fill.condition {
                    None => true,
                    Some(c) => b(
                        h,
                        M::CallbackBool,
                        vec![nd(c), Arg::Int(x), Arg::Int(y), Arg::Int(*plane)],
                    )?,
                };
                if !replace {
                    continue;
                }
                let source = fill.source.clone().ok_or(Error::Lateinit)?;
                let chunk = match &fill.delegate {
                    None => fill.chunk.clone(),
                    Some(d) => n(
                        h,
                        M::CallbackNode,
                        vec![
                            nd(d),
                            Arg::Int(x),
                            Arg::Int(y),
                            Arg::Int(*plane),
                            nd(&source),
                        ],
                    )?,
                };
                let Some(chunk) = chunk else {
                    continue;
                };
                let Some(copy) = n(h, M::ChunkCopy, vec![nd(&chunk)])? else {
                    continue;
                };
                unit(
                    h,
                    M::DynamicRegionReplaceChunk,
                    vec![
                        nd(dynamic),
                        Arg::Int(*plane),
                        Arg::Int(x),
                        Arg::Int(y),
                        nd(&copy),
                        nd(&source),
                    ],
                )?;
            }
        }
    }
    Ok(())
}
