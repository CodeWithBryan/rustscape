//! The constant tables the handlers read, in the originals' declaration
//! order.
use super::random::ChanceItem;

// ---------------------------------------------------------------------- EmptyOptionListener.EmptyItem

/// `Sounds.LIQUID_2401`.
pub const LIQUID: i32 = 2401;
/// `Items.VIAL_229`.
pub const VIAL: i32 = 229;

/// `EmptyItem` rows: (full id, empty id, message, audio id or -1).
pub const EMPTY_ITEMS: [(i32, i32, &str, i32); 51] = [
    (
        1933,
        1931,
        "You empty the contents of the pot onto the floor.",
        -1,
    ),
    (
        7468,
        1931,
        "You empty the contents of the pot onto the floor.",
        -1,
    ),
    (4255, 1931, "You empty the pot of crushed bones.", -1),
    (
        1783,
        1925,
        "You empty the contents of the bucket onto the floor.",
        2584,
    ),
    (
        1927,
        1925,
        "You empty the contents of the bucket onto the floor.",
        LIQUID,
    ),
    (
        1929,
        1925,
        "You empty the contents of the bucket on the floor.",
        LIQUID,
    ),
    (6032, 1925, "You empty the bucket of compost.", -1),
    (6034, 1925, "You empty the bucket of supercompost.", -1),
    (
        4286,
        1925,
        "You empty the contents of the bucket on the floor.",
        LIQUID,
    ),
    (5992, 1925, "You empty the bucket of apple mush.", -1),
    (227, VIAL, "You empty the vial.", LIQUID),
    (
        1921,
        1923,
        "You empty the contents of the bowl onto the floor.",
        LIQUID,
    ),
    (
        4456,
        1923,
        "You empty the contents of the bowl onto the floor.",
        LIQUID,
    ),
    (
        1937,
        1935,
        "You empty the contents of the jug onto the floor.",
        LIQUID,
    ),
    (2329, 2313, "You empty the pie dish.", -1),
    (195, VIAL, "You empty the vial.", LIQUID),
    (
        2005,
        1923,
        "You empty the contents of the bowl onto the floor.",
        LIQUID,
    ),
    (
        4239,
        1923,
        "You empty the contents of the bowl onto the floor.",
        LIQUID,
    ),
    (4242, 1980, "You empty the cup of tea.", LIQUID),
    (
        4243,
        1980,
        "You empty the contents of the cup onto the floor.",
        LIQUID,
    ),
    (
        4245,
        4244,
        "You empty the contents of the cup onto the floor.",
        LIQUID,
    ),
    (
        4246,
        4244,
        "You empty the contents of the cup onto the floor.",
        LIQUID,
    ),
    (
        4237,
        1923,
        "You empty the contents of the bowl onto the floor.",
        LIQUID,
    ),
    (
        4240,
        1923,
        "You empty the contents of the bowl onto the floor.",
        LIQUID,
    ),
    (
        4458,
        1980,
        "You empty the contents of the cup onto the floor.",
        LIQUID,
    ),
    (
        4460,
        1980,
        "You empty the contents of the cup onto the floor.",
        LIQUID,
    ),
    (
        7730,
        7728,
        "You empty the contents of the cup onto the floor.",
        LIQUID,
    ),
    (
        7731,
        7728,
        "You empty the contents of the cup onto the floor.",
        LIQUID,
    ),
    (
        7733,
        7732,
        "You empty the contents of the cup onto the floor.",
        LIQUID,
    ),
    (
        7734,
        7732,
        "You empty the contents of the cup onto the floor.",
        LIQUID,
    ),
    (
        7736,
        7735,
        "You empty the contents of the cup onto the floor.",
        LIQUID,
    ),
    (
        7737,
        7735,
        "You empty the contents of the cup onto the floor.",
        LIQUID,
    ),
    (
        7692,
        7702,
        "You empty the contents of the pot onto the floor",
        LIQUID,
    ),
    (
        7694,
        7702,
        "You empty the contents of the pot onto the floor",
        LIQUID,
    ),
    (
        7696,
        7702,
        "You empty the contents of the pot onto the floor",
        LIQUID,
    ),
    (
        7698,
        7702,
        "You empty the contents of the pot onto the floor",
        LIQUID,
    ),
    (
        7704,
        7714,
        "You empty the contents of the pot onto the floor",
        LIQUID,
    ),
    (
        7706,
        7714,
        "You empty the contents of the pot onto the floor",
        LIQUID,
    ),
    (
        7708,
        7714,
        "You empty the contents of the pot onto the floor",
        LIQUID,
    ),
    (
        7710,
        7714,
        "You empty the contents of the pot onto the floor",
        LIQUID,
    ),
    (
        7716,
        7726,
        "You empty the contents of the pot onto the floor",
        LIQUID,
    ),
    (
        7718,
        7726,
        "You empty the contents of the pot onto the floor",
        LIQUID,
    ),
    (
        7720,
        7726,
        "You empty the contents of the pot onto the floor",
        LIQUID,
    ),
    (
        7722,
        7726,
        "You empty the contents of the pot onto the floor",
        LIQUID,
    ),
    (
        2013,
        1923,
        "You empty the contents of the bowl onto the floor.",
        LIQUID,
    ),
    (
        2175,
        2166,
        "You empty the contents of the gnomebowl onto the floor.",
        -1,
    ),
    (
        7090,
        1923,
        "You empty the contents of the bowl onto the floor.",
        -1,
    ),
    (
        7092,
        1923,
        "You empty the contents of the bowl onto the floor.",
        -1,
    ),
    (
        7094,
        1923,
        "You empty the contents of the bowl onto the floor.",
        -1,
    ),
    (5935, VIAL, "You empty the vial.", LIQUID),
    (11262, VIAL, "You empty the vial.", LIQUID),
];

/// `FinishedPotion.values().map { it.potion.id }`.
pub const FINISHED_POTIONS: [i32; 54] = [
    121, 175, 115, 127, 3010, 133, 3034, 9741, 139, 12142, 145, 181, 151, 3018, 10000, 157, 187,
    3026, 163, 2454, 169, 3042, 189, 6687, 5937, 5940, 5945, 5954, 4844, 3410, 1582, 12633, 11204,
    9022, 10915, 10913, 10911, 10909, 10923, 10921, 10919, 10917, 10931, 10929, 10927, 10925, 7658,
    7656, 7654, 7652, 7666, 7664, 7662, 7660,
];

/// `UnfinishedPotion.values().map { it.potion.id }`.
pub const UNFINISHED_POTIONS: [i32; 21] = [
    91, 93, 4840, 95, 97, 99, 3002, 12181, 101, 103, 105, 9019, 3004, 107, 2483, 109, 111, 5936,
    5939, 5942, 5951,
];

/// One `EmptyItem` map entry: the empty id, the message and the audio id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Empty {
    pub empty: i32,
    pub message: &'static str,
    pub audio: i32,
}

/// `EmptyItem.emptyItemMap[id]` with its message and audio maps: the rows
/// first, then the finished and unfinished potion products, each
/// `putIfAbsent`.
pub fn empty_item(id: i32) -> Option<Empty> {
    if let Some(&(_, empty, message, audio)) = EMPTY_ITEMS.iter().find(|row| row.0 == id) {
        return Some(Empty {
            empty,
            message,
            audio,
        });
    }
    potion_products().contains(&id).then_some(Empty {
        empty: VIAL,
        message: "You empty the vial.",
        audio: LIQUID,
    })
}

/// The finished then unfinished potion products, in order.
pub fn potion_products() -> Vec<i32> {
    FINISHED_POTIONS
        .iter()
        .chain(&UNFINISHED_POTIONS)
        .copied()
        .collect()
}

/// `EmptyItem.emptyItemList` once its companion has initialised: the rows,
/// the potion products not already listed, then each definition (in the
/// definitions table's order) whose name matches the legacy scan. `lower`
/// is `lowercase(Locale.getDefault())`.
pub fn empty_item_list(
    definitions: &[super::Definition],
    lower: impl Fn(&str) -> String,
) -> super::Result<Vec<i32>> {
    let mut list: Vec<i32> = EMPTY_ITEMS.iter().map(|row| row.0).collect();
    for id in potion_products() {
        if !list.contains(&id) {
            list.push(id);
        }
    }
    for definition in definitions {
        let name = definition.name.as_deref().ok_or(super::Error::Null)?;
        let lower = lower(name);
        if (name.contains("potion")
            || name.contains("brew")
            || name.contains("poison")
            || lower.contains("serum")
            || name.contains("cure")
            || name.contains("mix")
            || name.contains("balm")
            || name.contains("Super "))
            && !list.contains(&definition.id)
        {
            list.push(definition.id);
        }
    }
    Ok(list)
}

// ---------------------------------------------------------------------- content.data.Lamps

/// `Lamps` rows: (item id, experience, level requirement).
pub const LAMPS: [(i32, i32, i32); 23] = [
    (2528, 10, 0),
    (12627, 500, 0),
    (12628, 500, 0),
    (13227, 10000, 30),
    (4447, 10000, 30),
    (11137, 1000, 30),
    (11139, 5000, 40),
    (11141, 10000, 50),
    (11753, 1000, 30),
    (11754, 5000, 40),
    (11755, 10000, 50),
    (11185, 500, 1),
    (11186, 1000, 30),
    (11187, 1500, 35),
    (14580, 1000, 30),
    (14581, 5000, 40),
    (14582, 10000, 50),
    (14574, 5000, 30),
    (14575, 10000, 40),
    (14576, 15000, 50),
    (14633, 1000, 30),
    (14634, 5000, 40),
    (14635, 10000, 50),
];
/// `Lamps.forItem(item)`: the index of the first lamp with that id.
pub fn lamp_for_item(id: i32) -> Option<usize> {
    LAMPS.iter().position(|lamp| lamp.0 == id)
}

/// `Skills.SKILL_NAME`.
pub const SKILL_NAME: [&str; 24] = [
    "Attack",
    "Defence",
    "Strength",
    "Hitpoints",
    "Ranged",
    "Prayer",
    "Magic",
    "Cooking",
    "Woodcutting",
    "Fletching",
    "Fishing",
    "Firemaking",
    "Crafting",
    "Smithing",
    "Mining",
    "Herblore",
    "Agility",
    "Thieving",
    "Slayer",
    "Farming",
    "Runecrafting",
    "Hunter",
    "Construction",
    "Summoning",
];

// ---------------------------------------------------------------------- content.data.tables.BirdNest

/// `NPCDropTables.DROP_RATES`.
pub const DROP_RATES: [i32; 4] = [750, 150, 15, 5];

/// `BirdNest`, in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BirdNest {
    Seed,
    Ring,
    Red,
    Green,
    Blue,
    Raven,
    Wyson,
}
impl BirdNest {
    pub const ALL: [Self; 7] = [
        Self::Seed,
        Self::Ring,
        Self::Red,
        Self::Green,
        Self::Blue,
        Self::Raven,
        Self::Wyson,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Seed => "SEED",
            Self::Ring => "RING",
            Self::Red => "RED",
            Self::Green => "GREEN",
            Self::Blue => "BLUE",
            Self::Raven => "RAVEN",
            Self::Wyson => "WYSON",
        }
    }
    pub fn ordinal(self) -> usize {
        Self::ALL.iter().position(|n| *n == self).unwrap_or(0)
    }
    /// The nest item.
    pub fn nest(self) -> ChanceItem {
        match self {
            Self::Seed => ChanceItem::new(5073, 1, 65),
            Self::Ring => ChanceItem::new(5074, 1, 30),
            Self::Red => ChanceItem::new(5070, 1, 5),
            Self::Green => ChanceItem::new(5071, 1, 5),
            Self::Blue => ChanceItem::new(5072, 1, 5),
            Self::Raven => ChanceItem::new(11966, 1, 5),
            Self::Wyson => ChanceItem::new(7413, 1, 1),
        }
    }
    /// The loot.
    pub fn loot(self) -> Vec<ChanceItem> {
        let r = DROP_RATES;
        let c = ChanceItem::new;
        match self {
            Self::Seed => vec![
                c(5312, 1, r[0]),
                c(5283, 1, r[0]),
                c(5313, 1, r[0]),
                c(5284, 1, r[0]),
                c(5285, 1, r[0]),
                c(5286, 1, r[0]),
                c(5314, 1, r[1]),
                c(5287, 1, r[1]),
                c(5288, 1, r[1]),
                c(5315, 1, r[2]),
                c(5289, 1, r[2]),
                c(5290, 1, r[2]),
                c(5317, 1, r[3]),
                c(5316, 1, r[3]),
            ],
            Self::Ring => vec![
                c(1635, 1, r[0]),
                c(1637, 1, r[0]),
                c(1639, 1, r[0]),
                c(1641, 1, r[0]),
                c(1643, 1, r[2]),
            ],
            Self::Red => vec![ChanceItem::single(5076)],
            Self::Green => vec![ChanceItem::single(5078)],
            Self::Blue => vec![ChanceItem::single(5077)],
            Self::Raven => vec![ChanceItem::single(11964)],
            Self::Wyson => vec![
                c(5318, 14, r[0]),
                c(5320, 3, r[0]),
                c(5322, 6, r[0]),
                c(5324, 9, r[0]),
                c(5100, 2, r[0]),
                c(5319, 11, r[0]),
                c(5323, 3, r[0]),
                c(5321, 2, r[0]),
                c(5312, 1, r[2]),
                c(5295, 1, r[2]),
                c(5313, 1, r[2]),
                c(5314, 1, r[2]),
                c(5315, 1, r[2]),
                c(5316, 1, r[3]),
                c(5317, 1, r[3]),
            ],
        }
    }
    /// `BirdNest.forNest(item)`: the first nest with that item id.
    pub fn for_nest(id: i32) -> Option<Self> {
        Self::ALL.into_iter().find(|n| n.nest().id == id)
    }
}
/// `BirdNest`'s empty nest, `Item(5075)`.
pub const EMPTY_NEST: i32 = 5075;

// ---------------------------------------------------------------------- HerbBoxPlugin, StarterPackPlugin, rings and packs

/// `HerbBoxPlugin.HERBS`.
pub const HERBS: [ChanceItem; 11] = [
    ChanceItem::new(199, 1, 25),
    ChanceItem::new(201, 1, 18),
    ChanceItem::new(203, 1, 13),
    ChanceItem::new(205, 1, 11),
    ChanceItem::new(207, 1, 8),
    ChanceItem::new(209, 1, 6),
    ChanceItem::new(211, 1, 4),
    ChanceItem::new(213, 1, 5),
    ChanceItem::new(215, 1, 3),
    ChanceItem::new(2485, 1, 3),
    ChanceItem::new(217, 1, 3),
];

/// `StarterPackPlugin.ITEMS`: (id, amount).
pub const STARTER_ITEMS: [(i32, i32); 18] = [
    (558, 50),
    (556, 50),
    (1379, 1),
    (740, 1),
    (882, 100),
    (1167, 1),
    (1129, 1),
    (1095, 1),
    (1063, 1),
    (1363, 1),
    (1323, 1),
    (1267, 1),
    (1349, 1),
    (1153, 1),
    (1101, 1),
    (1067, 1),
    (1175, 1),
    (841, 1),
];

/// `RingUnchargePlugin.getReward(id)`.
pub fn ring_reward(id: i32) -> i32 {
    match id {
        14810 => 6737,
        14808 => 6733,
        14809 => 6735,
        14807 => 6731,
        _ => -1,
    }
}
/// The imbued rings RingUnchargePlugin registers.
pub const IMBUED_RINGS: [i32; 4] = [14807, 14808, 14809, 14810];

/// `RunePackPlugin.getReward(id)`.
pub fn rune_pack_reward(id: i32) -> i32 {
    match id {
        14730 => 556,
        14732 => 555,
        14734 => 557,
        14736 => 554,
        14738 => 558,
        14740 => 562,
        _ => 556,
    }
}
/// The rune packs RunePackPlugin registers.
pub const RUNE_PACKS: [i32; 6] = [14730, 14732, 14734, 14736, 14738, 14740];

// ---------------------------------------------------------------------- ItemQuestRequirementListener

/// The `Quests` constant each `onEquip` lambda asks for, with its ids, in
/// registration order; `None` is the quest cape's own check.
pub fn quest_equipment() -> Vec<(Option<&'static str>, Vec<i32>)> {
    let fremennik_trials = vec![
        3751, 13408, 12674, 12675, 3748, 3753, 13409, 12677, 12676, 3749, 13407, 12673, 12672,
        6128, 10613, 13411, 6130, 13413, 6129, 13412, 6145, 13421, 6151, 13424, 6131, 10614, 13414,
        6133, 13415, 6135, 13416, 6149, 13423, 6143, 13420, 6137, 10604, 13417, 6139, 13418, 6141,
        13419, 6153, 13425, 6147, 13422, 3798, 3793, 3795, 3799, 3791, 3759, 3761, 3763, 3765,
        3777, 3779, 3781, 3783, 3785, 3787, 3789, 3767, 3769, 3771, 3773, 3775, 3757, 3758,
    ];
    let crystal: Vec<i32> = (4212..=4234).filter(|id| *id != 4213).collect();
    vec![
        (Some("THE_FREMENNIK_TRIALS"), fremennik_trials),
        (Some("THE_FREMENNIK_ISLES"), vec![10828, 12680, 12681]),
        (Some("ANIMAL_MAGNETISM"), vec![10499, 10498]),
        (Some("LOST_CITY"), vec![1215, 1231, 5680, 5698, 1305]),
        (Some("LEGENDS_QUEST"), vec![1052]),
        (None, vec![9813, 9814]),
        (Some("PRIEST_IN_PERIL"), vec![2952]),
        (Some("ANOTHER_SLICE_OF_HAM"), vec![11061]),
        (Some("DESERT_TREASURE"), vec![4675]),
        (Some("ELEMENTAL_WORKSHOP_I"), vec![2890]),
        (Some("ROVING_ELVES"), crystal),
        (
            Some("DRAGON_SLAYER"),
            vec![
                1127, 10798, 2615, 13800, 10800, 13805, 2623, 2653, 2661, 2669, 1135, 7370, 7372,
            ],
        ),
        (Some("MONKEY_MADNESS"), vec![4587]),
        (Some("RECIPE_FOR_DISASTER"), vec![7462]),
        (Some("SMOKING_KILLS"), vec![13263]),
        (Some("REGICIDE"), vec![3204]),
        (Some("DEATH_PLATEAU"), vec![3105]),
        (
            Some("HORROR_FROM_THE_DEEP"),
            vec![3840, 3842, 3844, 3839, 3841, 3843],
        ),
        (Some("ICTHLARINS_LITTLE_HELPER"), (9044..=9051).collect()),
        (Some("LEGENDS_QUEST"), vec![1187]),
        (Some("RECRUITMENT_DRIVE"), (5574..=5576).collect()),
        (Some("THE_SLUG_MENACE"), (9672..=9678).collect()),
        (Some("SUMMERS_END"), (13734..=13745).collect()),
        (Some("HEROES_QUEST"), vec![1434]),
        (Some("HEROES_QUEST"), vec![1377]),
        (Some("SHADOW_OF_THE_STORM"), vec![6746]),
    ]
}

// ---------------------------------------------------------------------- MorphItemListener, ToyListeners, ToyHorseListener

/// `MorphItemListener.morphItems`.
pub const MORPH_ITEMS: [i32; 2] = [6583, 7927];
/// `MorphItemListener.easterEggs` (`NPCs.EGG_3689..3694`).
pub const EASTER_EGGS: [i32; 6] = [3689, 3690, 3691, 3692, 3693, 3694];
/// `MorphItemListener.stone` (`NPCs.ROCKS_2626`).
pub const STONE: [i32; 1] = [2626];
/// The "unmorph" interface.
pub const MORPH_INTERFACE: i32 = 375;

/// `ToyListeners.MARIONETTES`.
pub const MARIONETTES: [i32; 3] = [6867, 6866, 6865];
/// `ToyListeners.MARIONETTE_GFX`.
pub const MARIONETTE_GFX: [[i32; 4]; 3] = [
    [507, 508, 509, 510],
    [511, 512, 513, 514],
    [515, 516, 517, 518],
];

/// `ToyHorseListener.HORSEY_MAP` (item to animation).
pub const HORSEY_MAP: [(i32, i32); 4] = [(2520, 918), (2522, 919), (2524, 920), (2526, 921)];
/// `ToyHorseListener.PHRASES`.
pub const PHRASES: [&str; 3] = [
    "Come-on Dobbin, we can win the race!",
    "Hi-ho Silver, and away",
    "Neaahhhyyy! Giddy-up horsey!",
];

// ---------------------------------------------------------------------- withobject

/// `WaterSourceListener.WaterVessel`, in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WaterVessel {
    Bucket,
    Vial,
    Jug,
    Bowl,
    WateringCan,
    WaterSkin,
    Fishbowl,
    Kettle,
    Cup,
}
impl WaterVessel {
    pub const ALL: [Self; 9] = [
        Self::Bucket,
        Self::Vial,
        Self::Jug,
        Self::Bowl,
        Self::WateringCan,
        Self::WaterSkin,
        Self::Fishbowl,
        Self::Kettle,
        Self::Cup,
    ];
    pub fn inputs(self) -> &'static [i32] {
        match self {
            Self::Bucket => &[1925],
            Self::Vial => &[229],
            Self::Jug => &[1935],
            Self::Bowl => &[1923],
            Self::WateringCan => &[5331, 5333, 5334, 5335, 5336, 5337, 5338, 5339],
            Self::WaterSkin => &[1831, 1829, 1827, 1825],
            Self::Fishbowl => &[6667],
            Self::Kettle => &[7688],
            Self::Cup => &[1980],
        }
    }
    pub fn output(self) -> i32 {
        match self {
            Self::Bucket => 1929,
            Self::Vial => 227,
            Self::Jug => 1937,
            Self::Bowl => 1921,
            Self::WateringCan => 5340,
            Self::WaterSkin => 1823,
            Self::Fishbowl => 6668,
            Self::Kettle => 7690,
            Self::Cup => 4458,
        }
    }
    /// Only the bucket can be filled at a well.
    pub fn wellable(self) -> bool {
        self == Self::Bucket
    }
    /// Every vessel keeps filling.
    pub fn autofill(self) -> bool {
        true
    }
    pub fn fill_message(self) -> &'static str {
        "You fill the @."
    }
    /// `WaterVessel.forId(id)` (the item map: later rows replace earlier).
    pub fn for_id(id: i32) -> Option<Self> {
        Self::ALL
            .into_iter()
            .rev()
            .find(|v| v.inputs().contains(&id))
    }
    /// `WaterVessel.getInputs()`: `itemMap.keys` in its HashMap's order.
    pub fn input_keys() -> Vec<i32> {
        let mut keys = Vec::new();
        for v in Self::ALL {
            for &id in v.inputs() {
                if !keys.contains(&id) {
                    keys.push(id);
                }
            }
        }
        java_hash_order(&keys)
    }
}

/// `WaterSourceListener.waterSources` (with its repeated ids).
pub const WATER_SOURCES: [i32; 95] = [
    21355, 16302, 6827, 11661, 24160, 34577, 15936, 15937, 15938, 23920, 35469, 24265, 153, 879,
    880, 2864, 6232, 10436, 10437, 10827, 11007, 11759, 21764, 22973, 24161, 24214, 24265, 28662,
    30223, 30820, 34579, 36781, 873, 874, 4063, 6151, 8699, 9143, 9684, 10175, 12279, 12974, 13563,
    13564, 14868, 14917, 14918, 15678, 16704, 16705, 20358, 22715, 24112, 24314, 25729, 25929,
    26966, 29105, 33458, 34082, 34411, 34496, 34547, 34566, 35762, 36971, 37154, 37155, 878, 884,
    3264, 3305, 3359, 4004, 4005, 6097, 6249, 6549, 8747, 8927, 11793, 12201, 12897, 24166, 26945,
    31359, 32023, 32024, 34576, 35671, 40063, 13561, 13563, 13559, 12089,
];
/// `WaterSourceListener.nonWellableMsg`.
pub const NON_WELLABLE: &str =
    "If I drop my @ down there, I don't think I'm likely to get it back.";

/// `SandSourceListener.SAND_PITS` (`Scenery.SAND_PIT_2645`, `SAND_PIT_4373`, `SANDPIT_10814`).
pub const SAND_PITS: [i32; 3] = [2645, 4373, 10814];
/// `SackOnHay.HAY`.
pub const HAY: [i32; 7] = [36892, 36894, 36896, 300, 34593, 298, 299];

/// The iteration order of a JDK `HashMap<Integer, _>` filled with these
/// keys one `put` at a time: the table grows from 16 by doubling whenever
/// the size passes three quarters of it; buckets are walked in index order,
/// each in insertion order (a resize keeps the relative order of a bucket).
pub fn java_hash_order(keys: &[i32]) -> Vec<i32> {
    let mut capacity = 16usize;
    while keys.len() > capacity * 3 / 4 {
        capacity *= 2;
    }
    java_bucket_order(keys, capacity)
}

/// The walk of a JDK HashMap of this capacity holding these Integer keys
/// (in insertion order).
pub fn java_bucket_order(keys: &[i32], capacity: usize) -> Vec<i32> {
    let mut ordered: Vec<(usize, usize, i32)> = keys
        .iter()
        .enumerate()
        .map(|(index, &key)| {
            let h = key as u32;
            let spread = h ^ (h >> 16);
            (spread as usize & (capacity - 1), index, key)
        })
        .collect();
    ordered.sort();
    ordered.into_iter().map(|(_, _, key)| key).collect()
}

// ---------------------------------------------------------------------- gloves

/// `FOGGlovesListener.FOG_GLOVES` (`IRIT_GLOVES_12856..EARTH_RUNECRAFTING_GLOVES_12865`).
pub const FOG_FIRST: i32 = 12856;
/// `MAX_CHARGES` of the Fist of Guthix gloves, by `id - 12856`.
pub const FOG_MAX_CHARGES: [i32; 10] = [100, 100, 100, 100, 1000, 1000, 1000, 1000, 1000, 1000];

/// `EquipmentSlot.HANDS.ordinal`.
pub const HANDS: i32 = 9;
/// `EquipmentSlot.HEAD.ordinal`.
pub const HEAD: i32 = 0;
