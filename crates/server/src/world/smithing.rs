//! Smithing at anvils: `SmithingPlugin` (the use-with handler of the nine
//! anvils and its anonymous "smith" option handler), `SmithingBuilder` (the
//! interface 300), `SmithingInterface`, `SmithingPulse` over `SkillPulse`,
//! and the `Bars`, `BarType` and `SmithingType` tables, from the pinned
//! `content/global/handlers/item/withobject`, `content/global/skill/smithing`
//! and `content/global/handlers/iface`.
//!
//! Every body is a function over [`Host`]; each call the original makes
//! outside the class is a required method with no default. Ported from the
//! pinned sources **without a JVM differential corpus**:
//! `tests/smithing.rs` checks the bodies against a recording host written
//! from the same sources, and the tables against the source text. See
//! docs/smithing.md for what is kept (the amount replaced by the bar count,
//! the first run that only sets the delay, the studs' fifth button, the
//! lantern buttons that resolve to the bullseye row) and what is not ported
//! (the dragonfire shield, dragon square shield and godsword dialogues).
pub use crate::world::global_items2::{Error, Result};
use crate::world::smelting::{self, format_display_name};

/// `Skills.SMITHING`.
pub const SMITHING: i32 = 13;
/// The smithing interface.
pub const INTERFACE: i32 = 300;
pub const HAMMER: i32 = 2347;
/// `new Animation(898)`; 2068 with the barefisted perk.
pub const ANIMATION: i32 = 898;
pub const BAREFISTED_ANIMATION: i32 = 2068;
/// Doric's anvil.
pub const DORICS_ANVIL: i32 = 2782;
/// The anvil the anonymous option handler is put on.
pub const OPTION_ANVIL: i32 = 42027;
pub const DORICS_QUEST: &str = "Doric's Quest";
pub const THE_TOURIST_TRAP: &str = "The Tourist Trap";
pub const DEATH_PLATEAU: &str = "Death Plateau";
/// The dialogue keys the plugin opens.
pub const SHIELD_DIALOGUE: i32 = 82127843;
pub const GODSWORD_DIALOGUE: i32 = 62362;
/// `UseWithHandler.OBJECT_TYPE`.
pub const OBJECT_TYPE: i32 = 2;
pub const PLUGIN_CLASS: &str = "content.global.handlers.item.withobject.SmithingPlugin";
pub const OPTION_CLASS: &str = "content.global.handlers.item.withobject.SmithingPlugin$1";
pub const INTERFACE_CLASS: &str = "content.global.handlers.iface.SmithingInterface";
pub const PULSE_CLASS: &str = "content.global.skill.smithing.SmithingPulse";

/// `SmithingPlugin()`'s allowed nodes.
pub const ALLOWED: [i32; 17] = [
    2349, 2351, 2353, 2359, 2361, 2363, 2366, 2368, 9467, 11286, 1540, 11710, 11712, 11714, 11686,
    11688, 11692,
];
/// The anvils `newInstance` adds the handler for.
pub const ANVILS: [i32; 9] = [2782, 2783, 4306, 6150, 22725, 26817, 26822, 37622, 42027];
/// The godsword pieces.
pub const GODSWORD_PIECES: [i32; 6] = [11710, 11712, 11714, 11686, 11688, 11692];

/// A `BarType` row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BarType {
    pub name: &'static str,
    pub bar: i32,
    pub experience: f64,
    pub bar_name: &'static str,
}

/// `BarType.values()`, in declaration order.
pub const BAR_TYPES: [BarType; 7] = [
    BarType {
        name: "BRONZE",
        bar: 2349,
        experience: 12.5,
        bar_name: "Bronze Smithing",
    },
    BarType {
        name: "BLURITE",
        bar: 9467,
        experience: 16.0,
        bar_name: "Blurite Smithing",
    },
    BarType {
        name: "IRON",
        bar: 2351,
        experience: 25.0,
        bar_name: "Iron Smithing",
    },
    BarType {
        name: "STEEL",
        bar: 2353,
        experience: 37.5,
        bar_name: "Steel Smithing",
    },
    BarType {
        name: "MITHRIL",
        bar: 2359,
        experience: 50.0,
        bar_name: "Mithril Smithing",
    },
    BarType {
        name: "ADAMANT",
        bar: 2361,
        experience: 62.5,
        bar_name: "Adamant Smithing",
    },
    BarType {
        name: "RUNITE",
        bar: 2363,
        experience: 75.0,
        bar_name: "Runite Smithing",
    },
];
pub const BLURITE: usize = 1;

/// `BarType.getBarTypeForId(itemId)`.
pub fn bar_type_for_id(item: i32) -> Option<usize> {
    BAR_TYPES.iter().position(|kind| kind.bar == item)
}

/// A `SmithingType` row: the bars one product takes, the interface child of
/// the item, the child of the name, the buttons (1, 5, X, All) and the
/// products one smithing makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SmithingType {
    pub name: &'static str,
    pub required: i32,
    pub child: i32,
    pub name_child: i32,
    pub buttons: &'static [i32],
    pub product_amount: i32,
}

/// A `Bars` row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bars {
    pub name: &'static str,
    /// The `BarType`'s ordinal.
    pub bar_type: usize,
    /// The `SmithingType`'s ordinal.
    pub kind: usize,
    pub product: i32,
    pub level: i32,
}

/// `SmithingType`, in declaration order.
pub const TYPES: [SmithingType; 31] = [
    SmithingType {
        name: "TYPE_DAGGER",
        required: 1,
        child: 18,
        name_child: 19,
        buttons: &[24, 23, 22, 21],
        product_amount: 1,
    },
    SmithingType {
        name: "TYPE_AXE",
        required: 1,
        child: 26,
        name_child: 27,
        buttons: &[32, 31, 30, 29],
        product_amount: 1,
    },
    SmithingType {
        name: "TYPE_MACE",
        required: 1,
        child: 34,
        name_child: 35,
        buttons: &[40, 39, 38, 37],
        product_amount: 1,
    },
    SmithingType {
        name: "TYPE_MEDIUM_HELM",
        required: 1,
        child: 42,
        name_child: 43,
        buttons: &[48, 47, 46, 45],
        product_amount: 1,
    },
    SmithingType {
        name: "TYPE_CROSSBOW_BOLT",
        required: 1,
        child: 50,
        name_child: 51,
        buttons: &[56, 55, 54, 53],
        product_amount: 10,
    },
    SmithingType {
        name: "TYPE_SWORD",
        required: 1,
        child: 58,
        name_child: 59,
        buttons: &[64, 63, 62, 61],
        product_amount: 1,
    },
    SmithingType {
        name: "TYPE_DART_TIP",
        required: 1,
        child: 66,
        name_child: 67,
        buttons: &[72, 71, 70, 69],
        product_amount: 10,
    },
    SmithingType {
        name: "TYPE_NAIL",
        required: 1,
        child: 74,
        name_child: 75,
        buttons: &[80, 79, 78, 77],
        product_amount: 15,
    },
    SmithingType {
        name: "TYPE_BULLSEYE",
        required: 1,
        child: 162,
        name_child: 163,
        buttons: &[168, 167, 166, 165],
        product_amount: 1,
    },
    SmithingType {
        name: "TYPE_SPIT_IRON",
        required: 1,
        child: 90,
        name_child: 91,
        buttons: &[96, 95, 94, 93],
        product_amount: 1,
    },
    SmithingType {
        name: "TYPE_WIRE",
        required: 1,
        child: 82,
        name_child: 83,
        buttons: &[88, 87, 86, 85],
        product_amount: 1,
    },
    SmithingType {
        name: "TYPE_ARROW_TIP",
        required: 1,
        child: 106,
        name_child: 107,
        buttons: &[112, 111, 110, 109],
        product_amount: 15,
    },
    SmithingType {
        name: "TYPE_SCIMITAR",
        required: 2,
        child: 114,
        name_child: 115,
        buttons: &[120, 119, 118, 117],
        product_amount: 1,
    },
    SmithingType {
        name: "TYPE_CROSSBOW_LIMB",
        required: 1,
        child: 122,
        name_child: 123,
        buttons: &[128, 127, 126, 125],
        product_amount: 1,
    },
    SmithingType {
        name: "TYPE_LONGSWORD",
        required: 2,
        child: 130,
        name_child: 131,
        buttons: &[136, 135, 134, 133],
        product_amount: 1,
    },
    SmithingType {
        name: "TYPE_THROWING_KNIFE",
        required: 1,
        child: 138,
        name_child: 139,
        buttons: &[144, 143, 142, 141],
        product_amount: 5,
    },
    SmithingType {
        name: "TYPE_FULL_HELM",
        required: 2,
        child: 146,
        name_child: 147,
        buttons: &[152, 151, 150, 149],
        product_amount: 1,
    },
    SmithingType {
        name: "TYPE_SQUARE_SHIELD",
        required: 2,
        child: 154,
        name_child: 155,
        buttons: &[160, 159, 158, 157],
        product_amount: 1,
    },
    SmithingType {
        name: "TYPE_OIL_LANTERN",
        required: 1,
        child: 162,
        name_child: 163,
        buttons: &[168, 167, 166, 165],
        product_amount: 1,
    },
    SmithingType {
        name: "TYPE_GRAPPLE_TIP",
        required: 1,
        child: 170,
        name_child: 171,
        buttons: &[176, 175, 174, 173],
        product_amount: 1,
    },
    SmithingType {
        name: "TYPE_STUDS",
        required: 1,
        child: 98,
        name_child: 99,
        buttons: &[104, 103, 102, 101, 100],
        product_amount: 1,
    },
    SmithingType {
        name: "TYPE_WARHAMMER",
        required: 3,
        child: 178,
        name_child: 179,
        buttons: &[184, 183, 182, 181],
        product_amount: 1,
    },
    SmithingType {
        name: "TYPE_BATTLE_AXE",
        required: 3,
        child: 186,
        name_child: 187,
        buttons: &[192, 191, 190, 189],
        product_amount: 1,
    },
    SmithingType {
        name: "TYPE_CHAINBODY",
        required: 3,
        child: 194,
        name_child: 195,
        buttons: &[200, 199, 198, 197],
        product_amount: 1,
    },
    SmithingType {
        name: "TYPE_KITE_SHIELD",
        required: 3,
        child: 202,
        name_child: 203,
        buttons: &[208, 207, 206, 205],
        product_amount: 1,
    },
    SmithingType {
        name: "TYPE_CLAWS",
        required: 2,
        child: 210,
        name_child: 211,
        buttons: &[216, 215, 214, 213],
        product_amount: 1,
    },
    SmithingType {
        name: "TYPE_TWO_HAND_SWORD",
        required: 3,
        child: 218,
        name_child: 219,
        buttons: &[224, 223, 222, 221],
        product_amount: 1,
    },
    SmithingType {
        name: "TYPE_PLATE_SKIRT",
        required: 3,
        child: 226,
        name_child: 227,
        buttons: &[232, 231, 230, 229],
        product_amount: 1,
    },
    SmithingType {
        name: "TYPE_PLATELEG",
        required: 3,
        child: 234,
        name_child: 235,
        buttons: &[240, 239, 238, 237],
        product_amount: 1,
    },
    SmithingType {
        name: "TYPE_PLATEBODY",
        required: 5,
        child: 242,
        name_child: 243,
        buttons: &[248, 247, 246, 245],
        product_amount: 1,
    },
    SmithingType {
        name: "TYPE_PICKAXE",
        required: 2,
        child: 267,
        name_child: 268,
        buttons: &[273, 272, 271, 270],
        product_amount: 1,
    },
];

pub const TYPE_DAGGER: usize = 0;
pub const TYPE_AXE: usize = 1;
pub const TYPE_MACE: usize = 2;
pub const TYPE_MEDIUM_HELM: usize = 3;
pub const TYPE_CROSSBOW_BOLT: usize = 4;
pub const TYPE_SWORD: usize = 5;
pub const TYPE_DART_TIP: usize = 6;
pub const TYPE_NAIL: usize = 7;
pub const TYPE_BULLSEYE: usize = 8;
pub const TYPE_SPIT_IRON: usize = 9;
pub const TYPE_WIRE: usize = 10;
pub const TYPE_ARROW_TIP: usize = 11;
pub const TYPE_SCIMITAR: usize = 12;
pub const TYPE_CROSSBOW_LIMB: usize = 13;
pub const TYPE_LONGSWORD: usize = 14;
pub const TYPE_THROWING_KNIFE: usize = 15;
pub const TYPE_FULL_HELM: usize = 16;
pub const TYPE_SQUARE_SHIELD: usize = 17;
pub const TYPE_OIL_LANTERN: usize = 18;
pub const TYPE_GRAPPLE_TIP: usize = 19;
pub const TYPE_STUDS: usize = 20;
pub const TYPE_WARHAMMER: usize = 21;
pub const TYPE_BATTLE_AXE: usize = 22;
pub const TYPE_CHAINBODY: usize = 23;
pub const TYPE_KITE_SHIELD: usize = 24;
pub const TYPE_CLAWS: usize = 25;
pub const TYPE_TWO_HAND_SWORD: usize = 26;
pub const TYPE_PLATE_SKIRT: usize = 27;
pub const TYPE_PLATELEG: usize = 28;
pub const TYPE_PLATEBODY: usize = 29;
pub const TYPE_PICKAXE: usize = 30;

/// `Bars`, in declaration order.
pub const BARS: [Bars; 158] = [
    Bars {
        name: "BRONZE_DAGGER",
        bar_type: 0,
        kind: TYPE_DAGGER,
        product: 1205,
        level: 1,
    },
    Bars {
        name: "BRONZE_AXE",
        bar_type: 0,
        kind: TYPE_AXE,
        product: 1351,
        level: 1,
    },
    Bars {
        name: "BRONZE_MACE",
        bar_type: 0,
        kind: TYPE_MACE,
        product: 1422,
        level: 2,
    },
    Bars {
        name: "BRONZE_MED_HELM",
        bar_type: 0,
        kind: TYPE_MEDIUM_HELM,
        product: 1139,
        level: 3,
    },
    Bars {
        name: "BRONZE_CROSSBOW_BOLT",
        bar_type: 0,
        kind: TYPE_CROSSBOW_BOLT,
        product: 9375,
        level: 3,
    },
    Bars {
        name: "BRONZE_SWORD",
        bar_type: 0,
        kind: TYPE_SWORD,
        product: 1277,
        level: 4,
    },
    Bars {
        name: "BRONZE_DART_TIPS",
        bar_type: 0,
        kind: TYPE_DART_TIP,
        product: 819,
        level: 4,
    },
    Bars {
        name: "BRONZE_NAILS",
        bar_type: 0,
        kind: TYPE_NAIL,
        product: 4819,
        level: 4,
    },
    Bars {
        name: "BRONZE_WIRE",
        bar_type: 0,
        kind: TYPE_WIRE,
        product: 1794,
        level: 4,
    },
    Bars {
        name: "BRONZE_ARROW_TIPS",
        bar_type: 0,
        kind: TYPE_ARROW_TIP,
        product: 39,
        level: 5,
    },
    Bars {
        name: "BRONZE_SCIMITAR",
        bar_type: 0,
        kind: TYPE_SCIMITAR,
        product: 1321,
        level: 5,
    },
    Bars {
        name: "BRONZE_CROSSBOW_LIMBS",
        bar_type: 0,
        kind: TYPE_CROSSBOW_LIMB,
        product: 9420,
        level: 6,
    },
    Bars {
        name: "BRONZE_LONGSWORD",
        bar_type: 0,
        kind: TYPE_LONGSWORD,
        product: 1291,
        level: 6,
    },
    Bars {
        name: "BRONZE_THROWINGKNFIE",
        bar_type: 0,
        kind: TYPE_THROWING_KNIFE,
        product: 864,
        level: 7,
    },
    Bars {
        name: "BRONZE_FULL_HELM",
        bar_type: 0,
        kind: TYPE_FULL_HELM,
        product: 1155,
        level: 7,
    },
    Bars {
        name: "BRONZE_SQUARE_SHIELD",
        bar_type: 0,
        kind: TYPE_SQUARE_SHIELD,
        product: 1173,
        level: 8,
    },
    Bars {
        name: "BRONZE_WAR_HAMMER",
        bar_type: 0,
        kind: TYPE_WARHAMMER,
        product: 1337,
        level: 9,
    },
    Bars {
        name: "BRONZE_BATTLEAXE",
        bar_type: 0,
        kind: TYPE_BATTLE_AXE,
        product: 1375,
        level: 10,
    },
    Bars {
        name: "BRONZE_CHAINBODY",
        bar_type: 0,
        kind: TYPE_CHAINBODY,
        product: 1103,
        level: 11,
    },
    Bars {
        name: "BRONZE_KITESHIELD",
        bar_type: 0,
        kind: TYPE_KITE_SHIELD,
        product: 1189,
        level: 12,
    },
    Bars {
        name: "BRONZE_CLAWS",
        bar_type: 0,
        kind: TYPE_CLAWS,
        product: 3095,
        level: 13,
    },
    Bars {
        name: "BRONZE_TWO_HANDED",
        bar_type: 0,
        kind: TYPE_TWO_HAND_SWORD,
        product: 1307,
        level: 14,
    },
    Bars {
        name: "BRONZE_PLATE_SKIRT",
        bar_type: 0,
        kind: TYPE_PLATE_SKIRT,
        product: 1087,
        level: 16,
    },
    Bars {
        name: "BRONZE_PLATELEGS",
        bar_type: 0,
        kind: TYPE_PLATELEG,
        product: 1075,
        level: 16,
    },
    Bars {
        name: "BRONZE_PLATEBODY",
        bar_type: 0,
        kind: TYPE_PLATEBODY,
        product: 1117,
        level: 18,
    },
    Bars {
        name: "BRONZE_PICKAXE",
        bar_type: 0,
        kind: TYPE_PICKAXE,
        product: 1265,
        level: 5,
    },
    Bars {
        name: "IRON_DAGGER",
        bar_type: 2,
        kind: TYPE_DAGGER,
        product: 1203,
        level: 15,
    },
    Bars {
        name: "IRON_AXE",
        bar_type: 2,
        kind: TYPE_AXE,
        product: 1349,
        level: 16,
    },
    Bars {
        name: "IRON_MACE",
        bar_type: 2,
        kind: TYPE_MACE,
        product: 1420,
        level: 17,
    },
    Bars {
        name: "IRON_MED_HELM",
        bar_type: 2,
        kind: TYPE_MEDIUM_HELM,
        product: 1137,
        level: 18,
    },
    Bars {
        name: "IRON_BOLT",
        bar_type: 2,
        kind: TYPE_CROSSBOW_BOLT,
        product: 9377,
        level: 18,
    },
    Bars {
        name: "IRON_SWORD",
        bar_type: 2,
        kind: TYPE_SWORD,
        product: 1279,
        level: 19,
    },
    Bars {
        name: "IRON_DART_TIPS",
        bar_type: 2,
        kind: TYPE_DART_TIP,
        product: 820,
        level: 19,
    },
    Bars {
        name: "IRON_NAILS",
        bar_type: 2,
        kind: TYPE_NAIL,
        product: 4820,
        level: 19,
    },
    Bars {
        name: "IRON_SPIT",
        bar_type: 2,
        kind: TYPE_SPIT_IRON,
        product: 7225,
        level: 16,
    },
    Bars {
        name: "IRON_ARROW_TIPS",
        bar_type: 2,
        kind: TYPE_ARROW_TIP,
        product: 40,
        level: 20,
    },
    Bars {
        name: "IRON_SCIMITAR",
        bar_type: 2,
        kind: TYPE_SCIMITAR,
        product: 1323,
        level: 20,
    },
    Bars {
        name: "IRON_CROSSBOW_LimbS",
        bar_type: 2,
        kind: TYPE_CROSSBOW_LIMB,
        product: 9423,
        level: 23,
    },
    Bars {
        name: "IRON_LONGSWORD",
        bar_type: 2,
        kind: TYPE_LONGSWORD,
        product: 1293,
        level: 21,
    },
    Bars {
        name: "IRON_KNIFE",
        bar_type: 2,
        kind: TYPE_THROWING_KNIFE,
        product: 863,
        level: 22,
    },
    Bars {
        name: "IRON_FULL_HELM",
        bar_type: 2,
        kind: TYPE_FULL_HELM,
        product: 1153,
        level: 22,
    },
    Bars {
        name: "IRON_SQUARE_SHIELD",
        bar_type: 2,
        kind: TYPE_SQUARE_SHIELD,
        product: 1175,
        level: 23,
    },
    Bars {
        name: "OIL_LANTERN_FRAME",
        bar_type: 2,
        kind: TYPE_OIL_LANTERN,
        product: 4540,
        level: 26,
    },
    Bars {
        name: "IRON_WARHAMMER",
        bar_type: 2,
        kind: TYPE_WARHAMMER,
        product: 1335,
        level: 24,
    },
    Bars {
        name: "IRON_BATTLEAXE",
        bar_type: 2,
        kind: TYPE_BATTLE_AXE,
        product: 1363,
        level: 25,
    },
    Bars {
        name: "IRON_CHAINBODY",
        bar_type: 2,
        kind: TYPE_CHAINBODY,
        product: 1101,
        level: 26,
    },
    Bars {
        name: "IRON_KITE_SHIELD",
        bar_type: 2,
        kind: TYPE_KITE_SHIELD,
        product: 1191,
        level: 27,
    },
    Bars {
        name: "IRON_CLAWS",
        bar_type: 2,
        kind: TYPE_CLAWS,
        product: 3096,
        level: 28,
    },
    Bars {
        name: "IRON_TWO_HANDED_SWORD",
        bar_type: 2,
        kind: TYPE_TWO_HAND_SWORD,
        product: 1309,
        level: 29,
    },
    Bars {
        name: "IRON_PLATESKIRT",
        bar_type: 2,
        kind: TYPE_PLATE_SKIRT,
        product: 1081,
        level: 31,
    },
    Bars {
        name: "IRON_PLATELEGS",
        bar_type: 2,
        kind: TYPE_PLATELEG,
        product: 1067,
        level: 31,
    },
    Bars {
        name: "IRON_PLATEBODY",
        bar_type: 2,
        kind: TYPE_PLATEBODY,
        product: 1115,
        level: 33,
    },
    Bars {
        name: "IRON_PICKAXE",
        bar_type: 2,
        kind: TYPE_PICKAXE,
        product: 1267,
        level: 20,
    },
    Bars {
        name: "STEEL_DAGGER",
        bar_type: 3,
        kind: TYPE_DAGGER,
        product: 1207,
        level: 30,
    },
    Bars {
        name: "STEEL_AXE",
        bar_type: 3,
        kind: TYPE_AXE,
        product: 1353,
        level: 31,
    },
    Bars {
        name: "STEEL_MACE",
        bar_type: 3,
        kind: TYPE_MACE,
        product: 1424,
        level: 32,
    },
    Bars {
        name: "STEEL_MED_HELM",
        bar_type: 3,
        kind: TYPE_MEDIUM_HELM,
        product: 1141,
        level: 33,
    },
    Bars {
        name: "STEEL_CROSSBOW_BOLT",
        bar_type: 3,
        kind: TYPE_CROSSBOW_BOLT,
        product: 9378,
        level: 33,
    },
    Bars {
        name: "STEEL_SWORD",
        bar_type: 3,
        kind: TYPE_SWORD,
        product: 1281,
        level: 34,
    },
    Bars {
        name: "STEEL_DART_TIPS",
        bar_type: 3,
        kind: TYPE_DART_TIP,
        product: 821,
        level: 34,
    },
    Bars {
        name: "STEEL_NAILS",
        bar_type: 3,
        kind: TYPE_NAIL,
        product: 1539,
        level: 34,
    },
    Bars {
        name: "STEEL_ARROW_TIPS",
        bar_type: 3,
        kind: TYPE_ARROW_TIP,
        product: 41,
        level: 35,
    },
    Bars {
        name: "STEEL_SCIMITAR",
        bar_type: 3,
        kind: TYPE_SCIMITAR,
        product: 1325,
        level: 35,
    },
    Bars {
        name: "STEEL_CROSSBOW_LIMBS",
        bar_type: 3,
        kind: TYPE_CROSSBOW_LIMB,
        product: 9425,
        level: 36,
    },
    Bars {
        name: "STEEL_LONGSWORD",
        bar_type: 3,
        kind: TYPE_LONGSWORD,
        product: 1295,
        level: 36,
    },
    Bars {
        name: "STEEL_THROWING_KNIFE",
        bar_type: 3,
        kind: TYPE_THROWING_KNIFE,
        product: 865,
        level: 37,
    },
    Bars {
        name: "STEEL_FULL_HELM",
        bar_type: 3,
        kind: TYPE_FULL_HELM,
        product: 1157,
        level: 37,
    },
    Bars {
        name: "STEEL_STUDS",
        bar_type: 3,
        kind: TYPE_STUDS,
        product: 2370,
        level: 36,
    },
    Bars {
        name: "STEEL_SQUARE_SHIELD",
        bar_type: 3,
        kind: TYPE_SQUARE_SHIELD,
        product: 1177,
        level: 38,
    },
    Bars {
        name: "STEEL_BULLSEYE",
        bar_type: 3,
        kind: TYPE_BULLSEYE,
        product: 4544,
        level: 49,
    },
    Bars {
        name: "STEEL_WARHAMMER",
        bar_type: 3,
        kind: TYPE_WARHAMMER,
        product: 1339,
        level: 39,
    },
    Bars {
        name: "STEEL_BATTLE_AXE",
        bar_type: 3,
        kind: TYPE_BATTLE_AXE,
        product: 1365,
        level: 40,
    },
    Bars {
        name: "STEEL_CHAINBODY",
        bar_type: 3,
        kind: TYPE_CHAINBODY,
        product: 1105,
        level: 41,
    },
    Bars {
        name: "STEEL_KITE_SHIELD",
        bar_type: 3,
        kind: TYPE_KITE_SHIELD,
        product: 1193,
        level: 42,
    },
    Bars {
        name: "STEEL_CLAWS",
        bar_type: 3,
        kind: TYPE_CLAWS,
        product: 3097,
        level: 43,
    },
    Bars {
        name: "STEEL_TWO_HANDED_SWORD",
        bar_type: 3,
        kind: TYPE_TWO_HAND_SWORD,
        product: 1311,
        level: 44,
    },
    Bars {
        name: "STEEL_PLATE_SKIRT",
        bar_type: 3,
        kind: TYPE_PLATE_SKIRT,
        product: 1083,
        level: 46,
    },
    Bars {
        name: "STEEL_PLATELEGS",
        bar_type: 3,
        kind: TYPE_PLATELEG,
        product: 1069,
        level: 46,
    },
    Bars {
        name: "STEEL_PLATEBODY",
        bar_type: 3,
        kind: TYPE_PLATEBODY,
        product: 1119,
        level: 48,
    },
    Bars {
        name: "STEEL_PICKAXE",
        bar_type: 3,
        kind: TYPE_PICKAXE,
        product: 1269,
        level: 35,
    },
    Bars {
        name: "MITHRIL_DAGGER",
        bar_type: 4,
        kind: TYPE_DAGGER,
        product: 1209,
        level: 50,
    },
    Bars {
        name: "MITHRIL_HATCHET",
        bar_type: 4,
        kind: TYPE_AXE,
        product: 1355,
        level: 51,
    },
    Bars {
        name: "MITHRIL_MACE",
        bar_type: 4,
        kind: TYPE_MACE,
        product: 1428,
        level: 52,
    },
    Bars {
        name: "MITHRIL_MED_HELM",
        bar_type: 4,
        kind: TYPE_MEDIUM_HELM,
        product: 1143,
        level: 53,
    },
    Bars {
        name: "MITHRIL_CROSSBOW_BOLT",
        bar_type: 4,
        kind: TYPE_CROSSBOW_BOLT,
        product: 9379,
        level: 53,
    },
    Bars {
        name: "MITHRIL_SWORD",
        bar_type: 4,
        kind: TYPE_SWORD,
        product: 1285,
        level: 54,
    },
    Bars {
        name: "MITHRIL_DART_TIPS",
        bar_type: 4,
        kind: TYPE_DART_TIP,
        product: 822,
        level: 54,
    },
    Bars {
        name: "MITHRIL_NAILS",
        bar_type: 4,
        kind: TYPE_NAIL,
        product: 4822,
        level: 54,
    },
    Bars {
        name: "MITHRIL_ARROW_TIPS",
        bar_type: 4,
        kind: TYPE_ARROW_TIP,
        product: 42,
        level: 55,
    },
    Bars {
        name: "MITHRIL_SCIMITAR",
        bar_type: 4,
        kind: TYPE_SCIMITAR,
        product: 1329,
        level: 55,
    },
    Bars {
        name: "MITHRIL_CROSSBOW_LIMBS",
        bar_type: 4,
        kind: TYPE_CROSSBOW_LIMB,
        product: 9427,
        level: 56,
    },
    Bars {
        name: "MITHRIL_LONGSWORD",
        bar_type: 4,
        kind: TYPE_LONGSWORD,
        product: 1299,
        level: 56,
    },
    Bars {
        name: "MITHRIL_KNIFE",
        bar_type: 4,
        kind: TYPE_THROWING_KNIFE,
        product: 866,
        level: 57,
    },
    Bars {
        name: "MITHRIL_FULL_HELM",
        bar_type: 4,
        kind: TYPE_FULL_HELM,
        product: 1159,
        level: 57,
    },
    Bars {
        name: "MITHRIL_SQUARE_SHIELD",
        bar_type: 4,
        kind: TYPE_SQUARE_SHIELD,
        product: 1181,
        level: 58,
    },
    Bars {
        name: "MITHRIL_GRAPPLE_TIPS",
        bar_type: 4,
        kind: TYPE_GRAPPLE_TIP,
        product: 9416,
        level: 59,
    },
    Bars {
        name: "MITHRIL_WARHAMMER",
        bar_type: 4,
        kind: TYPE_WARHAMMER,
        product: 1343,
        level: 59,
    },
    Bars {
        name: "MITHRIL_BATTLEAXE",
        bar_type: 4,
        kind: TYPE_BATTLE_AXE,
        product: 1369,
        level: 60,
    },
    Bars {
        name: "MITHRIL_CHAINBODY",
        bar_type: 4,
        kind: TYPE_CHAINBODY,
        product: 1109,
        level: 61,
    },
    Bars {
        name: "MITHRIL_KITE_SHIELD",
        bar_type: 4,
        kind: TYPE_KITE_SHIELD,
        product: 1197,
        level: 62,
    },
    Bars {
        name: "MITHRIL_CLAWS",
        bar_type: 4,
        kind: TYPE_CLAWS,
        product: 3099,
        level: 63,
    },
    Bars {
        name: "MITHRIL_TWO_HANDED_SWORD",
        bar_type: 4,
        kind: TYPE_TWO_HAND_SWORD,
        product: 1315,
        level: 64,
    },
    Bars {
        name: "MITHRIL_PLATESKIRT",
        bar_type: 4,
        kind: TYPE_PLATE_SKIRT,
        product: 1085,
        level: 66,
    },
    Bars {
        name: "MITHRIL_PLATELEGS",
        bar_type: 4,
        kind: TYPE_PLATELEG,
        product: 1071,
        level: 66,
    },
    Bars {
        name: "MITHRIL_PLATEBODY",
        bar_type: 4,
        kind: TYPE_PLATEBODY,
        product: 1121,
        level: 68,
    },
    Bars {
        name: "MITHRIL_PICKAXE",
        bar_type: 4,
        kind: TYPE_PICKAXE,
        product: 1273,
        level: 55,
    },
    Bars {
        name: "ADAMANT_DAGGER",
        bar_type: 5,
        kind: TYPE_DAGGER,
        product: 1211,
        level: 70,
    },
    Bars {
        name: "ADAMANT_AXE",
        bar_type: 5,
        kind: TYPE_AXE,
        product: 1357,
        level: 71,
    },
    Bars {
        name: "ADAMANT_MACE",
        bar_type: 5,
        kind: TYPE_MACE,
        product: 1430,
        level: 72,
    },
    Bars {
        name: "ADAMANT_MEDIUM_HELM",
        bar_type: 5,
        kind: TYPE_MEDIUM_HELM,
        product: 1145,
        level: 73,
    },
    Bars {
        name: "ADAMANT_BOLT",
        bar_type: 5,
        kind: TYPE_CROSSBOW_BOLT,
        product: 9380,
        level: 73,
    },
    Bars {
        name: "ADAMANT_SWORD",
        bar_type: 5,
        kind: TYPE_SWORD,
        product: 1287,
        level: 74,
    },
    Bars {
        name: "ADAMANT_DART_TIPS",
        bar_type: 5,
        kind: TYPE_DART_TIP,
        product: 823,
        level: 74,
    },
    Bars {
        name: "ADAMANT_NAILS",
        bar_type: 5,
        kind: TYPE_NAIL,
        product: 4823,
        level: 74,
    },
    Bars {
        name: "ADAMANT_ARROW_TIPS",
        bar_type: 5,
        kind: TYPE_ARROW_TIP,
        product: 43,
        level: 75,
    },
    Bars {
        name: "ADAMANT_SCIMITAR",
        bar_type: 5,
        kind: TYPE_SCIMITAR,
        product: 1331,
        level: 75,
    },
    Bars {
        name: "ADAMANT_LIMBS",
        bar_type: 5,
        kind: TYPE_CROSSBOW_LIMB,
        product: 9429,
        level: 76,
    },
    Bars {
        name: "ADAMANT_LONGSWORD",
        bar_type: 5,
        kind: TYPE_LONGSWORD,
        product: 1301,
        level: 76,
    },
    Bars {
        name: "ADAMANT_KNIFE",
        bar_type: 5,
        kind: TYPE_THROWING_KNIFE,
        product: 867,
        level: 77,
    },
    Bars {
        name: "ADAMANT_FULL_HELM",
        bar_type: 5,
        kind: TYPE_FULL_HELM,
        product: 1161,
        level: 77,
    },
    Bars {
        name: "ADAMANT_SQUARE_SHIELD",
        bar_type: 5,
        kind: TYPE_SQUARE_SHIELD,
        product: 1183,
        level: 78,
    },
    Bars {
        name: "ADAMANT_WARHAMMER",
        bar_type: 5,
        kind: TYPE_WARHAMMER,
        product: 1345,
        level: 79,
    },
    Bars {
        name: "ADAMANT_BATTLEAXE",
        bar_type: 5,
        kind: TYPE_BATTLE_AXE,
        product: 1371,
        level: 80,
    },
    Bars {
        name: "ADAMANT_CHAINBODY",
        bar_type: 5,
        kind: TYPE_CHAINBODY,
        product: 1111,
        level: 81,
    },
    Bars {
        name: "ADAMANT_KITESHIELD",
        bar_type: 5,
        kind: TYPE_KITE_SHIELD,
        product: 1199,
        level: 82,
    },
    Bars {
        name: "ADAMANT_CLAWS",
        bar_type: 5,
        kind: TYPE_CLAWS,
        product: 3100,
        level: 83,
    },
    Bars {
        name: "ADAMANT_TWO_HANDED_SWORD",
        bar_type: 5,
        kind: TYPE_TWO_HAND_SWORD,
        product: 1317,
        level: 84,
    },
    Bars {
        name: "ADAMANT_PLATE_SKIRT",
        bar_type: 5,
        kind: TYPE_PLATE_SKIRT,
        product: 1091,
        level: 86,
    },
    Bars {
        name: "ADAMANT_PLATE_LEGS",
        bar_type: 5,
        kind: TYPE_PLATELEG,
        product: 1073,
        level: 86,
    },
    Bars {
        name: "ADAMANT_PLATE_BODY",
        bar_type: 5,
        kind: TYPE_PLATEBODY,
        product: 1123,
        level: 88,
    },
    Bars {
        name: "ADAMANT_PICKAXE",
        bar_type: 5,
        kind: TYPE_PICKAXE,
        product: 1271,
        level: 75,
    },
    Bars {
        name: "RUNE_DAGGER",
        bar_type: 6,
        kind: TYPE_DAGGER,
        product: 1213,
        level: 85,
    },
    Bars {
        name: "RUNITE_AXE",
        bar_type: 6,
        kind: TYPE_AXE,
        product: 1359,
        level: 86,
    },
    Bars {
        name: "RUNITE_MACE",
        bar_type: 6,
        kind: TYPE_MACE,
        product: 1432,
        level: 87,
    },
    Bars {
        name: "RUNITE_MEDIUM_HELM",
        bar_type: 6,
        kind: TYPE_MEDIUM_HELM,
        product: 1147,
        level: 88,
    },
    Bars {
        name: "RUNITE_BOLT",
        bar_type: 6,
        kind: TYPE_CROSSBOW_BOLT,
        product: 9381,
        level: 88,
    },
    Bars {
        name: "RUNITE_SWORD",
        bar_type: 6,
        kind: TYPE_SWORD,
        product: 1289,
        level: 89,
    },
    Bars {
        name: "RUNITE_DART_TIPS",
        bar_type: 6,
        kind: TYPE_DART_TIP,
        product: 824,
        level: 89,
    },
    Bars {
        name: "RUNITE_NAILS",
        bar_type: 6,
        kind: TYPE_NAIL,
        product: 4824,
        level: 89,
    },
    Bars {
        name: "RUNITE_ARROW_TIPS",
        bar_type: 6,
        kind: TYPE_ARROW_TIP,
        product: 44,
        level: 90,
    },
    Bars {
        name: "RUNITE_SCIMITAR",
        bar_type: 6,
        kind: TYPE_SCIMITAR,
        product: 1333,
        level: 90,
    },
    Bars {
        name: "RUNITE_LIMBS",
        bar_type: 6,
        kind: TYPE_CROSSBOW_LIMB,
        product: 9431,
        level: 91,
    },
    Bars {
        name: "RUNITE_LONGSWORD",
        bar_type: 6,
        kind: TYPE_LONGSWORD,
        product: 1303,
        level: 91,
    },
    Bars {
        name: "RUNITE_KNIFE",
        bar_type: 6,
        kind: TYPE_THROWING_KNIFE,
        product: 868,
        level: 92,
    },
    Bars {
        name: "RUNITE_FULL_HELM",
        bar_type: 6,
        kind: TYPE_FULL_HELM,
        product: 1163,
        level: 92,
    },
    Bars {
        name: "RUNITE_SQUARE_SHIELD",
        bar_type: 6,
        kind: TYPE_SQUARE_SHIELD,
        product: 1185,
        level: 93,
    },
    Bars {
        name: "RUNITE_WARHAMMER",
        bar_type: 6,
        kind: TYPE_WARHAMMER,
        product: 1347,
        level: 94,
    },
    Bars {
        name: "RUNITE_BATTLEAXE",
        bar_type: 6,
        kind: TYPE_BATTLE_AXE,
        product: 1373,
        level: 95,
    },
    Bars {
        name: "RUNITE_CHAINBODY",
        bar_type: 6,
        kind: TYPE_CHAINBODY,
        product: 1113,
        level: 96,
    },
    Bars {
        name: "RUNITE_KITESHIELD",
        bar_type: 6,
        kind: TYPE_KITE_SHIELD,
        product: 1201,
        level: 97,
    },
    Bars {
        name: "RUNITE_CLAWS",
        bar_type: 6,
        kind: TYPE_CLAWS,
        product: 3101,
        level: 98,
    },
    Bars {
        name: "RUNITE_TWO_HANDED_SWORD",
        bar_type: 6,
        kind: TYPE_TWO_HAND_SWORD,
        product: 1319,
        level: 99,
    },
    Bars {
        name: "RUNITE_PLATE_SKIRT",
        bar_type: 6,
        kind: TYPE_PLATE_SKIRT,
        product: 1093,
        level: 99,
    },
    Bars {
        name: "RUNITE_PLATE_LEGS",
        bar_type: 6,
        kind: TYPE_PLATELEG,
        product: 1079,
        level: 99,
    },
    Bars {
        name: "RUNITE_PLATE_BODY",
        bar_type: 6,
        kind: TYPE_PLATEBODY,
        product: 1127,
        level: 99,
    },
    Bars {
        name: "RUNITE_PICKAXE",
        bar_type: 6,
        kind: TYPE_PICKAXE,
        product: 1275,
        level: 90,
    },
    Bars {
        name: "BLURITE_CROSSBOW_BOLT",
        bar_type: 1,
        kind: TYPE_CROSSBOW_BOLT,
        product: 9376,
        level: 8,
    },
    Bars {
        name: "BLURITE_CROSSBOW_LIMBS",
        bar_type: 1,
        kind: TYPE_CROSSBOW_LIMB,
        product: 9422,
        level: 13,
    },
];
pub const BLURITE_CROSSBOW_LIMBS: usize = 157;
pub const STEEL_LONGSWORD: usize = 64;
pub const ADAMANT_MEDIUM_HELM: usize = 109;

/// The children `SmithingBuilder.build` hides for blurite.
pub const BLURITE_HIDDEN: [i32; 21] = [
    17, 25, 33, 41, 57, 65, 73, 105, 113, 129, 137, 145, 153, 177, 185, 193, 201, 217, 225, 233,
    241,
];

/// `Bars.forId(item)`: the ordinal of the first row with this product.
pub fn bars_for_id(item: i32) -> Option<usize> {
    BARS.iter().position(|bar| bar.product == item)
}

/// `Bars.getBars(type)`: the ordinals of the type's rows.
pub fn bars_of(kind: usize) -> Vec<usize> {
    (0..BARS.len())
        .filter(|at| BARS[*at].bar_type == kind)
        .collect()
}

/// `Bars.getItemId(buttonId, type)`: the product of the first row of the
/// type one of whose buttons this is, or -1 (a null type matches no row).
pub fn item_id(button: i32, kind: Option<usize>) -> i32 {
    BARS.iter()
        .find(|bar| Some(bar.bar_type) == kind && TYPES[bar.kind].buttons.contains(&button))
        .map_or(-1, |bar| bar.product)
}

/// `Location.withinDistance(other, dist)`.
pub fn within(at: [i32; 3], other: [i32; 3], distance: f64) -> bool {
    if at[2] != other[2] {
        return false;
    }
    let (dx, dy) = (at[0] - other[0], at[1] - other[1]);
    f64::from(dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy))).sqrt() <= distance
}

/// `StringUtils.isPlusN(word)` of a lower-cased, non-null word (an empty
/// one is `charAt(0)`'s StringIndexOutOfBoundsException).
pub fn is_plus_n(word: &str) -> Result<bool> {
    let mut chars = word.chars();
    let first = chars.next().ok_or(Error::Index)?;
    Ok(matches!(first, 'a' | 'e' | 'i' | 'o' | 'u')
        || (first == 'h' && chars.next().is_some_and(|second| second != 'e')))
}

/// The diaries `SmithingPulse.reward` finishes tasks of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Diary {
    Falador,
    Lumbridge,
    Varrock,
}

/// What the amount prompt answered.
pub use smelting::Input;

/// Everything the smithing classes call outside themselves.
pub trait Host {
    type Node: Clone;

    /// `QuestRepository.isComplete(name)`.
    fn quest_complete(&mut self, player: &Self::Node, quest: &str) -> Result<bool>;
    /// ContentAPI `hasRequirement(player, quest, false)`.
    fn has_requirement(&mut self, player: &Self::Node, quest: &str) -> Result<bool>;
    /// `SkillcapePerks.isActive(BAREFISTED_SMITHING, player)`.
    fn barefisted(&mut self, player: &Self::Node) -> Result<bool>;
    /// `DialogueInterpreter.sendDialogue(text)`.
    fn send_dialogue(&mut self, player: &Self::Node, text: &str) -> Result<()>;
    /// `DialogueInterpreter.open(key, args...)`.
    fn open_dialogue(&mut self, player: &Self::Node, key: i32, args: &[i32]) -> Result<()>;
    /// `Skills.getLevel(skill)`.
    fn level(&mut self, player: &Self::Node, skill: i32) -> Result<i32>;
    /// `inventory.contains(id, amount)`.
    fn inventory_contains(&mut self, player: &Self::Node, id: i32, amount: i32) -> Result<bool>;
    /// `inventory.getAmount(new Item(id))`.
    fn inventory_amount(&mut self, player: &Self::Node, id: i32) -> Result<i32>;
    /// `inventory.toArray()`: the ids, slot by slot.
    fn inventory_ids(&mut self, player: &Self::Node) -> Result<Vec<Option<i32>>>;
    /// `inventory.remove(new Item(id, amount))`.
    fn inventory_remove(&mut self, player: &Self::Node, id: i32, amount: i32) -> Result<bool>;
    /// `inventory.add(new Item(id, amount))`.
    fn inventory_add(&mut self, player: &Self::Node, id: i32, amount: i32) -> Result<bool>;
    /// `ItemDefinition.forId(id).getName()`.
    fn item_name(&mut self, id: i32) -> Result<String>;
    /// `String.toLowerCase()`.
    fn lowercase(&self, text: &str) -> String;
    /// `gameAttributes.removeAttribute("smith-type")`.
    fn remove_smith_type(&mut self, player: &Self::Node) -> Result<()>;
    /// `gameAttributes.setAttribute("smith-type", type)`.
    fn set_smith_type(&mut self, player: &Self::Node, kind: Option<usize>) -> Result<()>;
    /// `(BarType) gameAttributes.getAttribute("smith-type")`.
    fn smith_type(&mut self, player: &Self::Node) -> Result<Option<usize>>;
    /// `gameAttributes.setAttribute("smith-bar", bar)`.
    fn set_smith_bar(&mut self, player: &Self::Node, bar: usize) -> Result<()>;
    /// `(Bars) gameAttributes.getAttribute("smith-bar")`.
    fn smith_bar(&mut self, player: &Self::Node) -> Result<Option<usize>>;
    /// `gameAttributes.setAttribute("smith-item", item)`.
    fn set_smith_item(&mut self, player: &Self::Node, item: i32) -> Result<()>;
    /// `(int) gameAttributes.getAttribute("smith-item")`.
    fn smith_item(&mut self, player: &Self::Node) -> Result<i32>;
    /// `packetDispatch.sendInterfaceConfig(interface, child, hidden)`.
    fn send_interface_config(
        &mut self,
        player: &Self::Node,
        interface: i32,
        child: i32,
        hidden: bool,
    ) -> Result<()>;
    /// `packetDispatch.sendString(text, interface, child)`.
    fn send_string(
        &mut self,
        player: &Self::Node,
        text: &str,
        interface: i32,
        child: i32,
    ) -> Result<()>;
    /// `InterfaceContainer.generateItems(player, { new Item(id, amount) },
    /// { "" }, interface, child)`.
    fn generate_item(
        &mut self,
        player: &Self::Node,
        id: i32,
        amount: i32,
        interface: i32,
        child: i32,
    ) -> Result<()>;
    /// `interfaceManager.open(new Component(id))`.
    fn open_interface(&mut self, player: &Self::Node, id: i32) -> Result<()>;
    /// `interfaceManager.close()`.
    fn close_interface(&mut self, player: &Self::Node) -> Result<()>;
    /// `player.animate(new Animation(id))`.
    fn animate(&mut self, player: &Self::Node, id: i32) -> Result<()>;
    /// `player.animate(new Animation(-1, Priority.HIGH))`.
    fn reset_animation(&mut self, player: &Self::Node) -> Result<()>;
    /// `player.dispatch(ResourceProducedEvent(item, 1, player, source))`.
    fn resource_produced(&mut self, player: &Self::Node, item: i32, source: i32) -> Result<()>;
    /// `Skills.addExperience(skill, experience, true)`.
    fn add_experience(&mut self, player: &Self::Node, skill: i32, experience: f64) -> Result<()>;
    /// `packetDispatch.sendMessage(text)`.
    fn send_message(&mut self, player: &Self::Node, text: &str) -> Result<()>;
    /// `player.getLocation()`.
    fn location(&mut self, player: &Self::Node) -> Result<[i32; 3]>;
    /// `achievementDiaryManager.finishTask(player, type, level, index)`.
    fn diary_finish_task(
        &mut self,
        player: &Self::Node,
        diary: Diary,
        level: i32,
        index: i32,
    ) -> Result<()>;
    /// `sendInputDialogue(player, true, prompt, handler)`.
    fn send_input(&mut self, player: &Self::Node, prompt: &str, handler: AmountInput)
    -> Result<()>;
    /// `pulseManager.run(pulse)`.
    fn run_pulse(&mut self, player: &Self::Node, pulse: SmithingPulse<Self::Node>) -> Result<()>;
}

/// What `SmithingPlugin.newInstance` registers.
pub trait Registrar {
    /// `new SmithingPlugin()`: `UseWithHandler(allowedNodes)`.
    fn new_use_handler(&mut self, allowed: Vec<i32>) -> Result<()>;
    /// `addHandler(id, type, this)`.
    fn add_use_handler(&mut self, id: i32, kind: i32) -> Result<()>;
    /// `ClassScanner.definePlugin(new OptionHandler() {...})`, whose
    /// `newInstance` is
    /// `SceneryDefinition.forId(id).getHandlers().put(key, this)`.
    fn define_option_plugin(&mut self, id: i32, key: &str) -> Result<()>;
}

/// `new SmithingPlugin().newInstance(arg)`.
pub fn new_instance<R: Registrar + ?Sized>(r: &mut R) -> Result<()> {
    r.new_use_handler(ALLOWED.to_vec())?;
    for id in ANVILS {
        r.add_use_handler(id, OBJECT_TYPE)?;
    }
    r.define_option_plugin(OPTION_ANVIL, "option:smith")
}

/// The anonymous OptionHandler's `handle(player, node, option)`:
/// `SmithingPlugin.this.handle(new NodeUsageEvent(player, 10, HAMMER,
/// node))`.
pub fn option_handle<H: Host + ?Sized>(h: &mut H, player: &H::Node, anvil: i32) -> Result<bool> {
    handle(h, player, HAMMER, anvil)?;
    Ok(true)
}

/// `SmithingPlugin.handle(event)`: `used` is the used item's id and `anvil`
/// the id of the scenery it was used with.
pub fn handle<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    used: i32,
    anvil: i32,
) -> Result<bool> {
    if anvil == DORICS_ANVIL && !h.quest_complete(player, DORICS_QUEST)? {
        h.send_dialogue(player, "Property of Doric the Dwarf.")?;
        return Ok(true);
    }
    if !h.inventory_contains(player, HAMMER, 1)? && !h.barefisted(player)? {
        h.send_dialogue(player, "You need a hammer to work the metal with.")?;
        return Ok(true);
    }
    if used == 2366 || used == 2368 {
        if h.level(player, SMITHING)? < 60 {
            h.send_dialogue(player, "You need a smithing level of 60 to smith this.")?;
            return Ok(true);
        }
        if !h.inventory_contains(player, 2366, 1)? && !h.inventory_contains(player, 2368, 1)? {
            h.send_dialogue(
                player,
                "You need a shield left half and a shield right half.",
            )?;
            return Ok(true);
        }
        h.open_dialogue(player, SHIELD_DIALOGUE, &[1, used])?;
        return Ok(true);
    }
    if used == 11286 || used == 1540 {
        if h.level(player, SMITHING)? < 90 {
            h.send_dialogue(player, "You need a smithing level of 90 to smith this.")?;
            return Ok(true);
        }
        if !h.inventory_contains(player, 1540, 1)? && !h.inventory_contains(player, 11286, 1)? {
            h.send_dialogue(
                player,
                "You need a draconic visage and a anti-dragon shield.",
            )?;
            return Ok(true);
        }
        h.open_dialogue(player, SHIELD_DIALOGUE, &[2, used])?;
        return Ok(true);
    }
    if GODSWORD_PIECES.contains(&used) {
        if h.level(player, SMITHING)? < 80 {
            h.send_dialogue(player, "You need a smithing level of 80 to smith this.")?;
            return Ok(true);
        }
        h.open_dialogue(player, GODSWORD_DIALOGUE, &[used])?;
        return Ok(true);
    }
    let mut bar = smelting::for_id(used);
    let mut item = used;
    if used == HAMMER {
        // The first inventory item that is a smelting bar; `bar` is
        // reassigned for every slot, so it ends null without one.
        for id in h.inventory_ids(player)?.into_iter().flatten() {
            bar = smelting::for_id(id);
            if bar.is_some() {
                item = id;
                break;
            }
        }
    }
    let Some(bar) = bar else {
        return Ok(true);
    };
    let bar = &smelting::BARS[bar];
    if h.level(player, SMITHING)? < bar.level {
        let name = h.item_name(bar.product)?;
        let name = h.lowercase(&name);
        h.send_dialogue(
            player,
            &format!(
                "You need a smithing level of at least {} to work {name}s.",
                bar.level
            ),
        )?;
        return Ok(true);
    }
    build(h, player, item)?;
    Ok(true)
}

/// `new SmithingBuilder(item).build(player)`. A bar without a `BarType`
/// (silver, gold, found by the hammer branch) is `type.name()`'s
/// NullPointerException, after the attribute was set to null.
pub fn build<H: Host + ?Sized>(h: &mut H, player: &H::Node, item: i32) -> Result<()> {
    let kind = bar_type_for_id(item);
    h.remove_smith_type(player)?;
    h.set_smith_type(player, kind)?;
    let kind = kind.ok_or(Error::Null)?;
    if kind == BLURITE {
        for child in &BLURITE_HIDDEN {
            h.send_interface_config(player, INTERFACE, *child, true)?;
        }
    } else {
        // The pickaxe.
        h.send_interface_config(player, INTERFACE, 267, false)?;
    }
    for at in bars_of(kind) {
        let bar = &BARS[at];
        let smithing = &TYPES[bar.kind];
        for (which, child) in [
            (TYPE_GRAPPLE_TIP, 169),
            (TYPE_DART_TIP, 65),
            (TYPE_WIRE, 81),
            (TYPE_SPIT_IRON, 89),
            (TYPE_BULLSEYE, 161),
            (TYPE_CLAWS, 209),
            (TYPE_OIL_LANTERN, 161),
            (TYPE_STUDS, 97),
        ] {
            if bar.kind == which {
                h.send_interface_config(player, INTERFACE, child, false)?;
            }
        }
        let color = if h.level(player, SMITHING)? < bar.level {
            ""
        } else {
            "<col=FFFFFF>"
        };
        let name = format_display_name(&smithing.name.replace("TYPE_", ""));
        h.send_string(
            player,
            &format!("{color}{name}"),
            INTERFACE,
            smithing.name_child,
        )?;
        if h.inventory_contains(player, BAR_TYPES[kind].bar, smithing.required)? {
            let plural = if smithing.required > 1 { "s" } else { "" };
            h.send_string(
                player,
                &format!("<col=2DE120>{} Bar{plural}", smithing.required),
                INTERFACE,
                smithing.name_child + 1,
            )?;
        }
        h.generate_item(
            player,
            bar.product,
            smithing.product_amount,
            INTERFACE,
            smithing.child - 1,
        )?;
    }
    h.send_string(player, BAR_TYPES[kind].bar_name, INTERFACE, 14)?;
    h.open_interface(player, INTERFACE)
}

/// `SmithingType.forButton(player, bar, button, item)`: 1, 5, -1 (X) or
/// the inventory's amount of the bar by the button's place among the
/// type's buttons; a fifth button (the studs') answers its count, 4.
pub fn for_button<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    bar: usize,
    button: i32,
    item: i32,
) -> Result<i32> {
    let buttons = TYPES[BARS[bar].kind].buttons;
    let Some(count) = buttons.iter().position(|b| *b == button) else {
        return Ok(-1);
    };
    Ok(match count {
        0 => 1,
        1 => 5,
        2 => -1,
        3 => h.inventory_amount(player, item)?,
        other => other as i32,
    })
}

/// The amount prompt's handler: a pulse for the `smith-item` and
/// `smith-bar` attributes as they are when the answer comes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AmountInput;
impl AmountInput {
    /// `(int) value`: a String is a ClassCastException.
    pub fn run<H: Host + ?Sized>(&self, h: &mut H, player: &H::Node, value: &Input) -> Result<()> {
        let Input::Int(amount) = value else {
            return Err(Error::Cast);
        };
        let item = h.smith_item(player)?;
        let bar = h.smith_bar(player)?;
        h.run_pulse(
            player,
            SmithingPulse::new(player.clone(), item, bar, *amount),
        )
    }
}

/// `SmithingInterface.handle(player, component, opcode, button, slot,
/// itemId)`.
pub fn interface_button<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    button: i32,
) -> Result<bool> {
    let kind = h.smith_type(player)?;
    let item = item_id(button, kind);
    let Some(bar) = bars_for_id(item) else {
        return Ok(true);
    };
    let amount = for_button(h, player, bar, button, BAR_TYPES[BARS[bar].bar_type].bar)?;
    h.set_smith_bar(player, bar)?;
    h.set_smith_item(player, item)?;
    if amount == -1 {
        h.send_input(player, "Enter the amount:", AmountInput)?;
        return Ok(true);
    }
    h.run_pulse(
        player,
        SmithingPulse::new(player.clone(), item, bars_for_id(item), amount),
    )?;
    Ok(true)
}

/// `SmithingPulse` over `SkillPulse<Item>`: a `Pulse(1, player, node)`
/// that `SkillPulse`'s constructor stops, so it runs only once `start()`
/// passed the requirements.
#[derive(Clone, Debug, PartialEq)]
pub struct SmithingPulse<N> {
    pub player: N,
    /// `node.getId()`: the product.
    pub item: i32,
    /// The `Bars` ordinal (null from a prompt answered without the
    /// attribute).
    pub bar: Option<usize>,
    pub amount: i32,
    /// `Pulse.delay`.
    pub delay: i32,
    pub running: bool,
}

impl<N: Clone> SmithingPulse<N> {
    /// `SmithingPulse(player, item, bar, amount)`.
    pub fn new(player: N, item: i32, bar: Option<usize>, amount: i32) -> Self {
        Self {
            player,
            item,
            bar,
            amount,
            delay: 1,
            running: false,
        }
    }

    /// `SkillPulse.start()` (`message(0)` is empty here).
    pub fn start<H: Host<Node = N> + ?Sized>(&mut self, h: &mut H) -> Result<()> {
        if self.check_requirements(h)? {
            self.running = true;
        }
        Ok(())
    }

    /// `SkillPulse.stop()`: the animation reset, then `Pulse.stop()`.
    pub fn stop<H: Host<Node = N> + ?Sized>(&mut self, h: &mut H) -> Result<()> {
        let player = self.player.clone();
        h.reset_animation(&player)?;
        self.running = false;
        Ok(())
    }

    /// `SkillPulse.pulse()`.
    pub fn pulse<H: Host<Node = N> + ?Sized>(&mut self, h: &mut H) -> Result<bool> {
        if !self.check_requirements(h)? {
            return Ok(true);
        }
        self.animate(h)?;
        self.reward(h)
    }

    /// `Pulse.update()` once the pulse is due: the body, `stop()` when it
    /// finished or threw, and whether the runner drops the pulse. The
    /// caller counts `delay` down.
    pub fn update<H: Host<Node = N> + ?Sized>(&mut self, h: &mut H) -> Result<bool> {
        if !self.running {
            return Ok(true);
        }
        match self.pulse(h) {
            Ok(true) => {
                self.stop(h)?;
                Ok(true)
            }
            Ok(false) => Ok(!self.running),
            Err(error) => {
                self.stop(h)?;
                Err(error)
            }
        }
    }

    /// `checkRequirements()`.
    pub fn check_requirements<H: Host<Node = N> + ?Sized>(&mut self, h: &mut H) -> Result<bool> {
        let player = self.player.clone();
        let bar = &BARS[self.bar.ok_or(Error::Null)?];
        let smithing = &TYPES[bar.kind];
        let metal = BAR_TYPES[bar.bar_type].bar;
        if !h.inventory_contains(&player, metal, smithing.required.wrapping_mul(self.amount))? {
            // The count of bars, not of products they make.
            self.amount = h.inventory_amount(&player, metal)?;
        }
        h.close_interface(&player)?;
        if h.level(&player, SMITHING)? < bar.level {
            let name = h.item_name(bar.product)?;
            h.send_dialogue(
                &player,
                &format!(
                    "You need a Smithing level of {} to make a {name}.",
                    bar.level
                ),
            )?;
            return Ok(false);
        }
        if !h.inventory_contains(&player, metal, smithing.required)? {
            let metal_name = h.item_name(metal)?;
            let metal_name = h.lowercase(&metal_name);
            let product = h.lowercase(&smithing.name.replace("TYPE_", "").replace('_', " "));
            h.send_dialogue(
                &player,
                &format!("You don't have enough {metal_name}s to make a {product}."),
            )?;
            return Ok(false);
        }
        if !h.inventory_contains(&player, HAMMER, 1)? && !h.barefisted(&player)? {
            h.send_dialogue(&player, "You need a hammer to work the metal with.")?;
            return Ok(false);
        }
        if !h.quest_complete(&player, THE_TOURIST_TRAP)? && bar.kind == TYPE_DART_TIP {
            h.send_dialogue(
                &player,
                "You need to complete Tourist Trap to smith dart tips.",
            )?;
            return Ok(false);
        }
        if !h.has_requirement(&player, DEATH_PLATEAU)? && bar.kind == TYPE_CLAWS {
            h.send_dialogue(
                &player,
                "You need to complete Death Plateau to smith claws.",
            )?;
            return Ok(false);
        }
        Ok(true)
    }

    /// `animate()`.
    pub fn animate<H: Host<Node = N> + ?Sized>(&mut self, h: &mut H) -> Result<()> {
        let player = self.player.clone();
        if h.barefisted(&player)? {
            return h.animate(&player, BAREFISTED_ANIMATION);
        }
        h.animate(&player, ANIMATION)
    }

    /// `reward()`: the first run only sets the delay to 4.
    pub fn reward<H: Host<Node = N> + ?Sized>(&mut self, h: &mut H) -> Result<bool> {
        if self.delay == 1 {
            self.delay = 4;
            return Ok(false);
        }
        let player = self.player.clone();
        let at = self.bar.ok_or(Error::Null)?;
        let bar = &BARS[at];
        let smithing = &TYPES[bar.kind];
        let metal = &BAR_TYPES[bar.bar_type];
        h.inventory_remove(&player, metal.bar, smithing.required)?;
        h.inventory_add(&player, self.item, smithing.product_amount)?;
        h.resource_produced(&player, self.item, metal.bar)?;
        h.add_experience(
            &player,
            SMITHING,
            metal.experience * f64::from(smithing.required),
        )?;
        let name = h.item_name(bar.product)?;
        let name = h.lowercase(&name);
        let article = if is_plus_n(&name)? { "an" } else { "a" };
        let metal_name = h.lowercase(metal.bar_name).replace("smithing", "");
        h.send_message(
            &player,
            &format!("You hammer the {metal_name}and make {article} {name}."),
        )?;
        // Near Thurgo's anvil.
        if at == BLURITE_CROSSBOW_LIMBS && within(h.location(&player)?, [3000, 3145, 0], 10.0) {
            h.diary_finish_task(&player, Diary::Falador, 1, 9)?;
        }
        // The anvil in the Draynor jailhouse sewers.
        if at == STEEL_LONGSWORD
            && within(h.location(&player)?, [3112, 9688, 0], smelting::RENDERING)
        {
            h.diary_finish_task(&player, Diary::Lumbridge, 2, 0)?;
        }
        // The south-east anvil in Varrock.
        if at == ADAMANT_MEDIUM_HELM
            && within(h.location(&player)?, [3247, 3404, 0], smelting::RENDERING)
        {
            h.diary_finish_task(&player, Diary::Varrock, 2, 3)?;
        }
        self.amount = self.amount.wrapping_sub(1);
        Ok(self.amount < 1)
    }
}
