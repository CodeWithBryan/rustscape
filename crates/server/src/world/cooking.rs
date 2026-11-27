//! Cooking on ranges and fires: `CookingRewrite`'s use-with listener, the
//! four cooking pulses it runs (`StandardCookingPulse`, `PieCookingPulse`,
//! `PizzaCookingPulse`, `IntentionalBurnPulse`), `CookingDialogue` and the
//! `CookableItems` table, from the pinned `content/global/skill/cooking`.
//!
//! Every body is a function over [`Host`]; each call the original makes
//! outside the class is a required method with no default. Ported from the
//! pinned sources **without a JVM differential corpus**:
//! `tests/cooking.rs` checks the bodies against a recording host written
//! from the same sources. See docs/cooking.md for what is kept (the
//! shadowed fields of the pulse subclasses, the pulse that is submitted even
//! when `start()` refuses, the burn roll drawn on every check) and what is
//! not ported (the tutorial branch, the recipes, dairy, fermenting).
use crate::world::interaction_listeners::{self as interactions, FunctionKind};
pub use crate::world::{
    gathering::random::skill_success_chance,
    global_items2::{Error, Result},
};

/// `Skills.COOKING`.
pub const COOKING: i32 = 7;
/// `Sounds.FRY_2577`.
pub const FRY: i32 = 2577;
/// `StandardCookingPulse.RANGE_ANIMATION` and `FIRE_ANIMATION` (both
/// `Animator.Priority.HIGH`).
pub const RANGE_ANIMATION: i32 = 883;
pub const FIRE_ANIMATION: i32 = 897;
/// `StandardCookingPulse.LUMBRIDGE_RANGE`.
pub const LUMBRIDGE_RANGE: i32 = 114;
/// `Quests.COOKS_ASSISTANT`.
pub const COOKS_ASSISTANT: &str = "Cook's Assistant";

pub const COOKED_MEAT: i32 = 2142;
pub const RAW_BEEF: i32 = 2132;
pub const RAW_BEAR_MEAT: i32 = 2136;
pub const BARLEY: i32 = 6006;
pub const BARLEY_MALT: i32 = 6008;
pub const SEAWEED: i32 = 401;
pub const SODA_ASH: i32 = 1781;
pub const BOWL_OF_WATER: i32 = 1921;
pub const BOWL_OF_HOT_WATER: i32 = 4456;
pub const BREAD_DOUGH: i32 = 2307;
pub const UNCOOKED_CAKE: i32 = 1889;
pub const CAKE_TIN: i32 = 1887;
pub const SINEW: i32 = 9436;
pub const IRON_SPIT: i32 = 7225;
pub const RAW_OOMLIE: i32 = 2337;
pub const COOKING_GAUNTLETS: i32 = 775;
/// The skewered foods of `cook`'s switch (with the iron spit itself).
pub const SPIT_FOODS: [i32; 5] = [7230, 7224, 9984, 9992, IRON_SPIT];

/// A `CookableItems` constant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cookable {
    pub name: &'static str,
    pub cooked: i32,
    pub raw: i32,
    pub burnt: i32,
    pub level: i32,
    pub experience: f64,
    pub low: i32,
    pub high: i32,
    pub low_range: i32,
    pub high_range: i32,
}

macro_rules! cookables {
    ($($name:ident($cooked:expr, $raw:expr, $burnt:expr, $level:expr, $xp:expr, $low:expr, $high:expr, $low_range:expr, $high_range:expr),)*) => {
        /// `CookableItems.values()`, in declaration order.
        pub const COOKABLES: &[Cookable] = &[$(Cookable {
            name: stringify!($name),
            cooked: $cooked,
            raw: $raw,
            burnt: $burnt,
            level: $level,
            experience: $xp as f64,
            low: $low,
            high: $high,
            low_range: $low_range,
            high_range: $high_range,
        },)*];
    };
}
cookables! {
    CHICKEN(2140, 2138, 2144, 1, 30, 128, 512, 128, 512),
    UGTHANKI(2140, 2138, 2144, 1, 40, 40, 252, 30, 253),
    RABBIT(3228, 3226, 7222, 1, 30, 128, 512, 128, 512),
    CRAB(7521, 7518, 7520, 21, 100, 57, 377, 57, 377),
    CHOMPY(2878, 2876, 2880, 30, 100, 200, 255, 200, 255),
    JUBBLY(7568, 7566, 7570, 41, 160, 195, 250, 195, 250),
    CRAYFISH(13433, 13435, 13437, 1, 30, 128, 512, 128, 512),
    SHRIMP(315, 317, 7954, 1, 30, 128, 512, 128, 512),
    KARAMBWANJI(3151, 3150, 592, 1, 10, 200, 400, 200, 400),
    SARDINE(325, 327, 369, 1, 40, 118, 492, 118, 492),
    ANCHOVIES(319, 321, 323, 1, 30, 128, 512, 128, 512),
    HERRING(347, 345, 357, 5, 50, 108, 472, 108, 472),
    MACKEREL(355, 353, 357, 10, 60, 98, 452, 98, 452),
    TROUT(333, 335, 343, 15, 70, 88, 432, 88, 432),
    COD(339, 341, 343, 18, 75, 83, 422, 88, 432),
    PIKE(351, 349, 343, 20, 80, 78, 412, 78, 412),
    SALMON(329, 331, 343, 25, 90, 68, 392, 68, 392),
    SLIMY_EEL(3381, 3379, 3383, 28, 95, 63, 382, 63, 382),
    TUNA(361, 359, 367, 30, 100, 58, 372, 58, 372),
    RAINBOW_FISH(10136, 10138, 10140, 35, 110, 56, 370, 56, 370),
    CAVE_EEL(5003, 5001, 5002, 38, 115, 38, 332, 38, 332),
    LOBSTER(379, 377, 381, 40, 120, 38, 332, 38, 332),
    BASS(365, 363, 367, 43, 130, 33, 312, 33, 312),
    SWORDFISH(373, 371, 375, 45, 140, 18, 292, 30, 310),
    LAVA_EEL(2149, 2148, 3383, 53, 30, 256, 256, 256, 256),
    MONKFISH(7946, 7944, 7948, 62, 150, 11, 275, 13, 280),
    SHARK(385, 383, 387, 80, 210, 1, 202, 1, 232),
    SEA_TURTLE(397, 395, 399, 82, 212, 1, 202, 1, 222),
    MANTA_RAY(391, 389, 393, 91, 216, 1, 202, 1, 222),
    KARAMBWAN(3144, 3142, 3146, 30, 190, 70, 255, 70, 255),
    THIN_SNAIL(3369, 3363, 3375, 12, 70, 93, 444, 93, 444),
    LEAN_SNAIL(3371, 3365, 3375, 17, 80, 85, 428, 93, 444),
    FAT_SNAIL(3373, 3367, 3375, 22, 95, 73, 402, 73, 402),
    BREAD(2309, 2307, 2311, 1, 40, 118, 492, 118, 492),
    PITTA_BREAD(1865, 1863, 1867, 58, 40, 118, 492, 118, 492),
    CAKE(1891, 1889, 1903, 40, 180, 0, 0, 38, 332),
    BEEF(2142, 2132, 2146, 1, 30, 128, 512, 128, 512),
    RAT_MEAT(2142, 2134, 2146, 1, 30, 128, 512, 128, 512),
    BEAR_MEAT(2142, 2136, 2146, 1, 30, 128, 512, 128, 512),
    YAK_MEAT(2142, 10816, 2146, 1, 30, 128, 512, 128, 512),
    SKEWER_CHOMPY(2878, 7230, 2880, 30, 100, 200, 255, 200, 255),
    SKEWER_ROAST_RABBIT(7223, 7224, 7222, 16, 72, 160, 255, 160, 255),
    SKEWER_ROAST_BIRD(9980, 9984, 9982, 11, 62, 155, 255, 155, 255),
    SKEWER_ROAST_BEAST(9988, 9992, 9990, 21, 82.5, 180, 255, 180, 255),
    REDBERRY_PIE(2325, 2321, 2329, 10, 78, 0, 0, 98, 452),
    MEAT_PIE(2327, 2319, 2329, 20, 110, 0, 0, 78, 412),
    MUD_PIE(7170, 7168, 2329, 29, 128, 0, 0, 58, 372),
    APPLE_PIE(2323, 2317, 2329, 30, 130, 0, 0, 58, 372),
    GARDEN_PIE(7178, 7176, 2329, 34, 138, 0, 0, 48, 352),
    FISH_PIE(7188, 7186, 2329, 47, 164, 0, 0, 38, 332),
    ADMIRAL_PIE(7198, 7196, 2329, 70, 210, 0, 0, 15, 270),
    WILD_PIE(7208, 7206, 2329, 85, 240, 0, 0, 1, 222),
    SUMMER_PIE(7218, 7216, 2329, 95, 260, 0, 0, 1, 212),
    PIZZA_PLAIN(2289, 2287, 2305, 35, 143, 0, 0, 48, 352),
    BOWL_STEW(2003, 2001, 2005, 25, 117, 68, 392, 68, 392),
    BOWL_CURRY(2011, 2009, 2013, 60, 280, 38, 332, 38, 332),
    BOWL_NETTLE(4239, 4237, 4239, 20, 53, 78, 412, 78, 412),
    BOWL_EGG(7078, 7076, 7090, 13, 50, 0, 0, 90, 438),
    BOWL_ONION(7084, 1871, 7092, 43, 60, 36, 322, 36, 322),
    BOWL_MUSHROOM(7082, 7080, 7094, 46, 60, 16, 282, 16, 282),
    BAKED_POTATO(6701, 1942, 6699, 7, 15, 0, 0, 108, 472),
    SWEETCORN(5988, 5986, 5990, 28, 104, 78, 412, 78, 412),
    RAW_OOMLIE(2337, 0, 2426, 50, 0, 0, 0, 0, 0),
    OOMLIE_WRAP(2343, 2341, 2345, 50, 30, 106, 450, 112, 476),
    SEAWEED(401, 0, 1781, 0, 0, 0, 0, 0, 0),
    BARLEY(6006, 0, 6008, 0, 0, 0, 0, 0, 0),
    SINEW(9436, 2132, 9436, 0, 3, 0, 0, 0, 0),
}

/// `CookableItems.gauntletValues`.
pub const GAUNTLET_VALUES: [(i32, [i32; 2]); 4] = [
    (377, [55, 368]),
    (371, [30, 310]),
    (7944, [24, 290]),
    (383, [15, 270]),
];
/// `CookableItems.lumbridgeRangeValues`.
pub const LUMBRIDGE_RANGE_VALUES: [(i32, [i32; 2]); 21] = [
    (2307, [128, 512]),
    (2132, [138, 532]),
    (2134, [138, 532]),
    (2136, [138, 532]),
    (10816, [138, 532]),
    (2138, [138, 532]),
    (317, [138, 532]),
    (321, [138, 532]),
    (327, [128, 512]),
    (345, [118, 492]),
    (353, [108, 472]),
    (2321, [108, 462]),
    (3363, [103, 464]),
    (335, [98, 452]),
    (3365, [95, 448]),
    (341, [93, 442]),
    (349, [88, 432]),
    (2319, [88, 432]),
    (3367, [83, 422]),
    (2001, [78, 412]),
    (331, [78, 402]),
];

/// `CookableItems.forId(id)`: `cookingMap` is filled with `putIfAbsent` by
/// raw id, so the first constant with that raw id answers (raw beef is BEEF,
/// never SINEW; raw id 0 is RAW_OOMLIE).
pub fn for_id(id: i32) -> Option<&'static Cookable> {
    COOKABLES.iter().find(|c| c.raw == id)
}
/// `intentionalBurnMap.get(id)`: the first constant with that cooked id.
fn intentional(id: i32) -> Option<&'static Cookable> {
    COOKABLES.iter().find(|c| c.cooked == id)
}
/// `CookableItems.intentionalBurn(id)`.
pub fn intentional_burn(id: i32) -> bool {
    intentional(id).is_some()
}
/// `CookableItems.getIntentionalBurn(id).id` (a NullPointerException with no
/// row).
pub fn intentional_burn_product(id: i32) -> Result<i32> {
    intentional(id).map(|c| c.burnt).ok_or(Error::Null)
}
/// `CookableItems.getBurnt(id).id`.
pub fn burnt(id: i32) -> Result<i32> {
    for_id(id).map(|c| c.burnt).ok_or(Error::Null)
}

/// `CookingRewrite.COOKING_OBJs` (114 appears twice, as in the original).
pub const COOKING_OBJECTS: [i32; 110] = [
    24313, 21302, 13528, 13529, 13533, 13531, 13536, 13539, 13542, 2728, 2729, 2730, 2731, 2732,
    2859, 3038, 3769, 3775, 4265, 4266, 5249, 5499, 5631, 5632, 5981, 9682, 10433, 11404, 11405,
    11406, 12102, 12796, 13337, 13881, 14169, 14919, 15156, 20000, 20001, 21620, 21792, 22713,
    22714, 23046, 24283, 24284, 25155, 25156, 25465, 25730, 27297, 29139, 30017, 32099, 33500,
    34495, 34546, 36973, 37597, 37629, 37726, 114, 4172, 5275, 8750, 16893, 22154, 34410, 34565,
    114, 9085, 9086, 9087, 12269, 15398, 25440, 25441, 2724, 2725, 2726, 4618, 4650, 5165, 6093,
    6094, 6095, 6096, 8712, 9374, 9439, 9440, 9441, 10824, 17640, 17641, 17642, 17643, 18039,
    18170, 21795, 24285, 24329, 27251, 33498, 35449, 36815, 36816, 37426, 40110, 10377,
];

/// `CookingRewrite.RAW_FOODS`: every constant's raw id in declaration order
/// (with its repeats and its zeros), then the six added ids.
pub fn raw_foods() -> Vec<i32> {
    let mut list: Vec<i32> = COOKABLES.iter().map(|c| c.raw).collect();
    list.extend([
        COOKED_MEAT,
        RAW_BEEF,
        RAW_BEAR_MEAT,
        BARLEY,
        SEAWEED,
        BOWL_OF_WATER,
    ]);
    list
}

/// Java's `String.toLowerCase()` for the names read here (item and scenery
/// names are ASCII in the pinned cache; the host lower-cases).
pub trait Host {
    /// A player, an inventory item or a scene object.
    type Node: Clone;

    /// `node.getId()`.
    fn id(&mut self, node: &Self::Node) -> Result<i32>;
    /// `scenery.getName()`.
    fn scenery_name(&mut self, scenery: &Self::Node) -> Result<String>;
    /// `scenery.isActive()`.
    fn scenery_active(&mut self, scenery: &Self::Node) -> Result<bool>;
    /// `new Item(id).getName()` / `ItemDefinition.forId(id).getName()`.
    fn item_name(&mut self, id: i32) -> Result<String>;
    /// `String.toLowerCase()`.
    fn lowercase(&self, text: &str) -> String;

    /// `getAttribute(player, "/save:tutorial:complete", false)`.
    fn tutorial_complete(&mut self, player: &Self::Node) -> Result<bool>;
    /// The tutorial island branch of the listener (its queued script,
    /// `TutorialStage.load`): content.
    fn tutorial_cook(&mut self, player: &Self::Node, used: &Self::Node) -> Result<()>;
    /// ContentAPI `amountInInventory(player, id)`.
    fn amount_in_inventory(&mut self, player: &Self::Node, id: i32) -> Result<i32>;
    /// `player.getInventory().getAmount(id)`.
    fn inventory_amount(&mut self, player: &Self::Node, id: i32) -> Result<i32>;
    /// `player.getInventory().add(new Item(id))`.
    fn inventory_add(&mut self, player: &Self::Node, id: i32) -> Result<bool>;
    /// `player.getInventory().remove(new Item(id))`.
    fn inventory_remove(&mut self, player: &Self::Node, id: i32) -> Result<bool>;
    /// `GroundItemManager.create(new Item(id), player.getLocation(), player)`
    /// (`at_location`) or `GroundItemManager.create(new Item(id), player)`.
    fn ground_create(&mut self, player: &Self::Node, id: i32, at_location: bool) -> Result<()>;
    /// `player.getEquipment().containsItem(new Item(id))`.
    fn equipment_contains(&mut self, player: &Self::Node, id: i32) -> Result<bool>;
    /// `player.getPacketDispatch().sendMessage(message)` (null sends nothing).
    fn send_message(&mut self, player: &Self::Node, text: Option<&str>) -> Result<()>;
    /// `player.getDialogueInterpreter().sendDialogue(message)`.
    fn send_dialogue(&mut self, player: &Self::Node, text: &str) -> Result<()>;
    /// `player.getQuestRepository().isComplete(quest)`.
    fn quest_complete(&mut self, player: &Self::Node, quest: &str) -> Result<bool>;
    /// `player.getSkills().getLevel(skill)`.
    fn level(&mut self, player: &Self::Node, skill: i32) -> Result<i32>;
    /// `SkillcapePerks.isActive(SkillcapePerks.HASTY_COOKING, player)`.
    fn hasty_cooking(&mut self, player: &Self::Node) -> Result<bool>;
    /// `RandomFunction.randomDouble(max)`.
    fn random_double(&mut self, max: f64) -> Result<f64>;
    /// `RandomFunction.random(max)`.
    fn random(&mut self, max: i32) -> Result<i32>;
    /// `player.dispatch(new ResourceProducedEvent(item, amount, source, original))`.
    fn resource_produced(
        &mut self,
        player: &Self::Node,
        item: i32,
        amount: i32,
        source: &Self::Node,
        original: i32,
    ) -> Result<()>;
    /// `player.getSkills().addExperience(skill, experience, true)`.
    fn add_experience(&mut self, player: &Self::Node, skill: i32, experience: f64) -> Result<()>;
    /// `PlayerMonitor.log(player, LogType.DUPE_ALERT, message)`.
    fn dupe_alert(&mut self, player: &Self::Node, message: &str) -> Result<()>;
    /// `player.getName()`.
    fn player_name(&mut self, player: &Self::Node) -> Result<String>;
    /// `player.incrementAttribute(key, amount)`.
    fn increment_attribute(&mut self, player: &Self::Node, key: &str, amount: i32) -> Result<()>;
    /// ContentAPI `playAudio(player, id)`.
    fn play_audio(&mut self, player: &Self::Node, id: i32) -> Result<()>;
    /// `player.animate(new Animation(id, Animator.Priority.HIGH))`.
    fn animate_high(&mut self, player: &Self::Node, id: i32) -> Result<()>;
    /// `player.pulseManager.run(pulse)`: `start()` runs inside
    /// ([`CookingPulse::start`]); the pulse is submitted while it is running.
    fn run_pulse(&mut self, player: &Self::Node, pulse: CookingPulse<Self::Node>) -> Result<()>;
    /// `player.dialogueInterpreter.open(CookingDialogue(*args))`.
    fn open_dialogue(&mut self, player: &Self::Node, dialogue: Dialogue<Self::Node>) -> Result<()>;

    // CookingDialogue (a DialogueFile).
    /// `options(*lines)`.
    fn dialogue_options(&mut self, player: &Self::Node, lines: &[&str]) -> Result<()>;
    /// `end()`.
    fn dialogue_end(&mut self, player: &Self::Node) -> Result<()>;
    /// `packetDispatch.sendItemZoomOnInterface(item, zoom, interface, child)`.
    fn send_item_zoom(
        &mut self,
        player: &Self::Node,
        item: i32,
        zoom: i32,
        interface: i32,
        child: i32,
    ) -> Result<()>;
    /// `packetDispatch.sendString(text, interface, child)`.
    fn send_string(
        &mut self,
        player: &Self::Node,
        text: &str,
        interface: i32,
        child: i32,
    ) -> Result<()>;
    /// `PacketRepository.send(RepositionChild, ChildPositionContext(player,
    /// interface, child, x, y))`.
    fn reposition_child(
        &mut self,
        player: &Self::Node,
        interface: i32,
        child: i32,
        x: i32,
        y: i32,
    ) -> Result<()>;
    /// `interfaceManager.openChatbox(interface)`.
    fn open_chatbox(&mut self, player: &Self::Node, interface: i32) -> Result<()>;
    /// ContentAPI `sendInputDialogue(player, true, prompt) { value -> cook(...) }`.
    fn send_input(
        &mut self,
        player: &Self::Node,
        prompt: &str,
        handler: AmountInput<Self::Node>,
    ) -> Result<()>;
}

// ------------------------------------------------------------ CookingRewrite

/// CookingRewrite's use-with lambda.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Interaction;
impl crate::world::component_objects::NativeObject for Interaction {}
/// The lambda's class and the listener's.
pub const LAMBDA: &str = "content.global.skill.cooking.CookingRewrite$defineListeners$1";
pub const CLASS: &str = "content.global.skill.cooking.CookingRewrite";

/// `CookingRewrite.defineListeners`: `onUseWith(IntType.SCENERY, RAW_FOODS,
/// *COOKING_OBJs)`.
pub fn define(registry: &interactions::Registry) -> Result<()> {
    let function = interactions::Function::new(FunctionKind::UseWith, LAMBDA, Interaction);
    registry
        .add_use_arrays(
            interactions::Kind::Scenery.ordinal(),
            &raw_foods(),
            &COOKING_OBJECTS,
            function,
        )
        .map_err(|e| Error::Binding(Some(format!("{e:?}"))))
}

/// The use-with lambda: the food `used` on the range or fire `with`.
pub fn use_with<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    used: &H::Node,
    with: &H::Node,
) -> Result<bool> {
    if !h.tutorial_complete(player)? {
        h.tutorial_cook(player, used)?;
        return Ok(true);
    }
    let item = h.id(used)?;
    let name = h.scenery_name(with)?;
    let range = h.lowercase(&name).contains("range");
    match item {
        RAW_BEEF | RAW_BEAR_MEAT if range => {
            h.open_dialogue(player, Dialogue::sinew(item, with.clone()))?;
            return Ok(true);
        }
        BREAD_DOUGH | UNCOOKED_CAKE | BARLEY if !range => {
            h.send_message(player, Some("You need to cook this on a range."))?;
            return Ok(false);
        }
        BOWL_OF_WATER => {
            cook(h, player, with, BOWL_OF_WATER, BOWL_OF_HOT_WATER, 1)?;
            return Ok(true);
        }
        _ => {}
    }
    if h.amount_in_inventory(player, item)? > 1 {
        h.open_dialogue(player, Dialogue::standard(item, with.clone()))?;
    } else {
        // One of the item: no dialogue.
        let product = if intentional_burn(item) {
            intentional_burn_product(item)?
        } else {
            for_id(item).ok_or(Error::Null)?.cooked
        };
        cook(h, player, with, item, product, 1)?;
    }
    Ok(true)
}

/// `CookingRewrite.cook(player, object, initial, product, amount)`: the
/// pulse by the food's name and table rows.
pub fn cook<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    object: &H::Node,
    initial: i32,
    product: i32,
    amount: i32,
) -> Result<()> {
    let name = h.item_name(initial)?;
    let name = h.lowercase(&name);
    let kind = if name.contains("pizza") {
        Kind::Pizza
    } else if name.contains("pie") {
        Kind::Pie
    } else if intentional_burn(initial) {
        Kind::IntentionalBurn
    } else {
        Kind::Standard
    };
    let pulse = CookingPulse::new(h, kind, player, object, initial, product, amount)?;
    h.run_pulse(player, pulse)
}

// ------------------------------------------------------------ the pulses

/// The pulse's class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Standard,
    Pizza,
    Pie,
    IntentionalBurn,
}
impl Kind {
    pub fn class(self) -> &'static str {
        match self {
            Self::Standard => "content.global.skill.cooking.StandardCookingPulse",
            Self::Pizza => "content.global.skill.cooking.PizzaCookingPulse",
            Self::Pie => "content.global.skill.cooking.PieCookingPulse",
            Self::IntentionalBurn => "content.global.skill.cooking.IntentionalBurnPulse",
        }
    }
}

/// A `StandardCookingPulse` or one of its subclasses.
#[derive(Clone, Debug, PartialEq)]
pub struct CookingPulse<N> {
    pub kind: Kind,
    pub player: N,
    pub object: N,
    pub initial: i32,
    pub product: i32,
    /// `StandardCookingPulse.amount`.
    pub amount: i32,
    /// `IntentionalBurnPulse.amount`: the subclass's own field, which
    /// shadows the base one in its `reward`.
    pub own_amount: i32,
    pub experience: f64,
    pub burned: bool,
    /// `properties` (set by `start`).
    pub properties: Option<&'static Cookable>,
    pub initial_amount: i32,
    pub processed_amount: i32,
    /// `Pulse.delay` (1 from `Pulse()`; `reward` sets it once).
    pub delay: i32,
    /// `Pulse.running`: true from construction, so `PulseManager.run`
    /// submits the pulse even when `start()` refused.
    pub running: bool,
}

impl<N: Clone> CookingPulse<N> {
    /// The constructor (the base one reads the inventory).
    pub fn new<H: Host<Node = N> + ?Sized>(
        h: &mut H,
        kind: Kind,
        player: &N,
        object: &N,
        initial: i32,
        product: i32,
        amount: i32,
    ) -> Result<Self> {
        let initial_amount = h.amount_in_inventory(player, initial)?;
        Ok(Self {
            kind,
            player: player.clone(),
            object: object.clone(),
            initial,
            product,
            amount,
            own_amount: amount,
            experience: 0.0,
            burned: false,
            properties: None,
            initial_amount,
            processed_amount: 0,
            delay: 1,
            running: true,
        })
    }

    /// `start()`: the first item is cooked at once when the requirements
    /// hold.
    pub fn start<H: Host<Node = N> + ?Sized>(&mut self, h: &mut H) -> Result<()> {
        self.properties = for_id(self.initial);
        if self.check_requirements(h)? {
            // super.start().
            self.running = true;
            let burned = self.properties.is_some() && self.burned;
            self.cook(h, burned)?;
            self.amount = self.amount.wrapping_sub(1);
        }
        Ok(())
    }

    /// `pulse()`: whether the pulse finished.
    pub fn pulse<H: Host<Node = N> + ?Sized>(&mut self, h: &mut H) -> Result<bool> {
        if self.amount < 1 || !self.check_requirements(h)? {
            return Ok(true);
        }
        self.reward(h)
    }

    /// `checkRequirements()` with the subclasses' overrides.
    pub fn check_requirements<H: Host<Node = N> + ?Sized>(&mut self, h: &mut H) -> Result<bool> {
        match self.kind {
            Kind::IntentionalBurn => return h.scenery_active(&self.object),
            Kind::Pie | Kind::Pizza => {
                let name = h.scenery_name(&self.object)?;
                if !h.lowercase(&name).contains("range") {
                    h.send_message(&self.player, Some("This can only be cooked on a range."))?;
                    return Ok(false);
                }
            }
            Kind::Standard => {}
        }
        self.experience = 0.0;
        if let Some(properties) = self.properties {
            if h.id(&self.object)? == LUMBRIDGE_RANGE
                && !h.quest_complete(&self.player, COOKS_ASSISTANT)?
            {
                h.send_message(
                    &self.player,
                    Some(
                        "You need to have completed the Cook's Assistant quest in order to use that range.",
                    ),
                )?;
                return Ok(false);
            }
            if h.level(&self.player, COOKING)? < properties.level {
                h.send_dialogue(
                    &self.player,
                    &format!(
                        "You need a cooking level of {} to cook this.",
                        properties.level
                    ),
                )?;
                return Ok(false);
            }
            self.experience = properties.experience;
            self.burned = self.is_burned(h)?;
        }
        if self.amount < 1 {
            return Ok(false);
        }
        h.scenery_active(&self.object)
    }

    /// `reward()`: the first run only sets the delay.
    pub fn reward<H: Host<Node = N> + ?Sized>(&mut self, h: &mut H) -> Result<bool> {
        if self.kind == Kind::IntentionalBurn {
            if self.delay == 1 {
                // `equals("range")`, not `contains`.
                let name = h.scenery_name(&self.object)?;
                self.delay = if h.lowercase(&name) == "range" { 5 } else { 4 };
                return Ok(false);
            }
            if self.cook(h, false)? {
                self.own_amount = self.own_amount.wrapping_sub(1);
            } else {
                return Ok(true);
            }
            return Ok(self.own_amount <= 1);
        }
        if self.delay == 1 {
            let name = h.scenery_name(&self.object)?;
            let mut delay = if h.lowercase(&name).contains("range") {
                5
            } else {
                4
            };
            if h.hasty_cooking(&self.player)? {
                delay -= 1;
            }
            self.delay = delay;
            return Ok(false);
        }
        let burned = self.burned;
        if self.cook(h, burned)? {
            self.amount = self.amount.wrapping_sub(1);
        } else {
            return Ok(true);
        }
        Ok(self.amount < 1)
    }

    /// `isBurned(player, object, food)`: one `randomDouble(100.0)` draw
    /// against the success chance for the level.
    pub fn is_burned<H: Host<Node = N> + ?Sized>(&mut self, h: &mut H) -> Result<bool> {
        let food = self.initial;
        let gauntlets = h.equipment_contains(&self.player, COOKING_GAUNTLETS)?;
        let mut level = h.level(&self.player, COOKING)?;
        if h.hasty_cooking(&self.player)? {
            level = level.wrapping_sub(5);
        }
        let item = for_id(food);
        let gauntlet = GAUNTLET_VALUES.iter().find(|(id, _)| *id == food);
        let (low, high) = if let (true, Some((_, values))) = (gauntlets, gauntlet) {
            (values[0], values[1])
        } else if h.id(&self.object)? == LUMBRIDGE_RANGE {
            match LUMBRIDGE_RANGE_VALUES.iter().find(|(id, _)| *id == food) {
                Some((_, values)) => (values[0], values[1]),
                None => {
                    // The default array is built before the lookup.
                    let item = item.ok_or(Error::Null)?;
                    (item.low_range, item.high_range)
                }
            }
        } else {
            let name = h.scenery_name(&self.object)?;
            let fire = h.lowercase(&name).contains("fire");
            let item = item.ok_or(Error::Null)?;
            if fire {
                (item.low, item.high)
            } else {
                (item.low_range, item.high_range)
            }
        };
        let host_ratio = h.random_double(100.0)?;
        let client_ratio = skill_success_chance(f64::from(low), f64::from(high), level);
        Ok(host_ratio > client_ratio)
    }

    /// `cook(player, object, burned, initial, product)`: whether the food
    /// was removed.
    pub fn cook<H: Host<Node = N> + ?Sized>(&mut self, h: &mut H, burned: bool) -> Result<bool> {
        let (initial, product) = (self.initial, self.product);
        let player = self.player.clone();
        if self.kind == Kind::IntentionalBurn {
            if h.inventory_remove(&player, initial)? {
                h.inventory_add(&player, product)?;
                let message = self.message(h, burned)?;
                h.send_message(&player, message.as_deref())?;
                h.play_audio(&player, FRY)?;
                self.animate(h)?;
                return Ok(true);
            }
            return Ok(false);
        }
        if SPIT_FOODS.contains(&initial) {
            if h.random(15)? == 5 {
                h.send_message(
                    &player,
                    Some("Your iron spit seems to have broken in the process."),
                )?;
            } else if !h.inventory_add(&player, IRON_SPIT)? {
                h.ground_create(&player, IRON_SPIT, true)?;
            }
        } else if initial == UNCOOKED_CAKE && !h.inventory_add(&player, CAKE_TIN)? {
            h.ground_create(&player, CAKE_TIN, false)?;
        }
        if h.inventory_remove(&player, initial)? {
            if !burned {
                h.inventory_add(&player, product)?;
                h.resource_produced(&player, product, 1, &self.object, initial)?;
                h.add_experience(&player, COOKING, self.experience)?;
                self.processed_amount = self.processed_amount.wrapping_add(1);
                if self.processed_amount > self.initial_amount {
                    let message = format!(
                        "cooked item ({}, {}): initialAmount {}, processedAmount {}",
                        h.player_name(&player)?,
                        h.item_name(initial)?,
                        self.initial_amount,
                        self.processed_amount
                    );
                    h.dupe_alert(&player, &message)?;
                }
                h.increment_attribute(&player, "/save:stats_manager:food_cooked", 1)?;
            } else {
                let burnt = burnt(initial)?;
                h.resource_produced(&player, burnt, 1, &self.object, initial)?;
                // `CookableItems.getBurnt(initial)` again for the add.
                h.inventory_add(&player, burnt)?;
            }
            let message = self.message(h, burned)?;
            h.send_message(&player, message.as_deref())?;
            h.play_audio(&player, FRY)?;
            self.animate(h)?;
            return Ok(true);
        }
        Ok(false)
    }

    /// `animate()`: the fire animation only at an object named exactly
    /// "fire" (any case).
    pub fn animate<H: Host<Node = N> + ?Sized>(&mut self, h: &mut H) -> Result<()> {
        let name = h.scenery_name(&self.object)?;
        let id = if name.eq_ignore_ascii_case("fire") {
            FIRE_ANIMATION
        } else {
            RANGE_ANIMATION
        };
        h.animate_high(&self.player, id)
    }

    /// `getMessage(food, product, burned)` with the pie and pizza
    /// overrides.
    pub fn message<H: Host<Node = N> + ?Sized>(
        &self,
        h: &mut H,
        burned: bool,
    ) -> Result<Option<String>> {
        match self.kind {
            Kind::Pie => {
                return Ok(Some(if burned {
                    "You accidentally burn the pie.".into()
                } else {
                    let name = h.item_name(self.product)?;
                    format!("You successfully bake a delicious {}.", h.lowercase(&name))
                }));
            }
            Kind::Pizza => {
                return Ok(Some(
                    if burned {
                        "You accidentally burn the pizza."
                    } else {
                        "You cook a delicious looking pizza."
                    }
                    .into(),
                ));
            }
            Kind::Standard | Kind::IntentionalBurn => {}
        }
        if self.initial == RAW_OOMLIE {
            return Ok(Some("The meat is far too delicate to cook like this. Perhaps you should wrap something around it to protect it from the heat.".into()));
        }
        if self.product == SODA_ASH {
            return Ok(Some("You burn the seaweed into soda ash.".into()));
        }
        if self.product == BARLEY_MALT {
            return Ok(None);
        }
        if intentional_burn(self.initial) {
            return Ok(Some(
                "You deliberately burn the perfectly good piece of meat.".into(),
            ));
        }
        let food = h.item_name(self.initial)?;
        for (prefix, strip) in [("Raw", "Raw "), ("Uncooked", "Uncooked ")] {
            if food.starts_with(prefix) {
                let what = food.replace(strip, "");
                return Ok(Some(if burned {
                    format!("You accidentally burn some {what}.")
                } else {
                    format!("You manage to cook some {what}.")
                }));
            }
        }
        Ok(None)
    }
}

// ------------------------------------------------------------ CookingDialogue

/// `CookingDialogue`'s fields (a DialogueFile; `stage` is its own).
#[derive(Clone, Debug, PartialEq)]
pub struct Dialogue<N> {
    /// Five arguments (`initial, product, sinew, object, itemid`) rather
    /// than two (`initial, object`).
    pub five: bool,
    pub initial: i32,
    pub product: i32,
    pub object: N,
    pub sinew: bool,
    pub stage: i32,
}
/// `CookingDialogue`'s class.
pub const DIALOGUE_CLASS: &str = "content.global.skill.cooking.CookingDialogue";

/// The amount prompt's handler (`cook(player, object, initial, product,
/// value)`).
#[derive(Clone, Debug, PartialEq)]
pub struct AmountInput<N> {
    pub object: N,
    pub initial: i32,
    pub product: i32,
}

/// What the prompt answered.
#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    Int(i32),
    Text(String),
}

impl<N: Clone> AmountInput<N> {
    /// The handler: a String is parsed with `toInt()` (a
    /// NumberFormatException otherwise).
    pub fn run<H: Host<Node = N> + ?Sized>(
        &self,
        h: &mut H,
        player: &N,
        value: &Input,
    ) -> Result<()> {
        let amount = match value {
            Input::Int(n) => *n,
            Input::Text(text) => text
                .parse::<i32>()
                .map_err(|_| Error::Binding(Some("NumberFormatException".into())))?,
        };
        cook(h, player, &self.object, self.initial, self.product, amount)
    }
}

impl<N: Clone> Dialogue<N> {
    /// `CookingDialogue(item, object)`.
    pub fn standard(initial: i32, object: N) -> Self {
        Self {
            five: false,
            initial,
            product: 0,
            object,
            sinew: false,
            stage: 0,
        }
    }
    /// `CookingDialogue(item, 9436, true, object, item)`.
    pub fn sinew(initial: i32, object: N) -> Self {
        Self {
            five: true,
            initial,
            product: SINEW,
            object,
            sinew: true,
            stage: 0,
        }
    }

    /// `handle(componentID, buttonID)`.
    pub fn handle<H: Host<Node = N> + ?Sized>(
        &mut self,
        h: &mut H,
        player: &N,
        button: i32,
    ) -> Result<()> {
        match self.stage {
            0 => {
                if !self.five {
                    self.product = if intentional_burn(self.initial) {
                        intentional_burn_product(self.initial)?
                    } else {
                        for_id(self.initial).ok_or(Error::Null)?.cooked
                    };
                } else if self.sinew {
                    h.dialogue_options(player, &["Dry the meat into sinew", "Cook the meat"])?;
                    self.stage = if h.amount_in_inventory(player, self.initial)? > 1 {
                        100
                    } else {
                        101
                    };
                    return Ok(());
                }
                self.display(h, player)
            }
            1 => {
                h.dialogue_end(player)?;
                let amount = match button {
                    5 => 1,
                    4 => 5,
                    2 => h.inventory_amount(player, self.initial)?,
                    _ => -1,
                };
                if amount == -1 {
                    h.send_input(
                        player,
                        "Enter the amount:",
                        AmountInput {
                            object: self.object.clone(),
                            initial: self.initial,
                            product: self.product,
                        },
                    )
                } else {
                    h.dialogue_end(player)?;
                    cook(h, player, &self.object, self.initial, self.product, amount)
                }
            }
            100 => {
                match button {
                    1 => {
                        self.product = SINEW;
                        self.display(h, player)?;
                    }
                    2 => {
                        self.product = for_id(self.initial).ok_or(Error::Null)?.cooked;
                        self.display(h, player)?;
                    }
                    _ => {}
                }
                Ok(())
            }
            101 => {
                match button {
                    1 => {
                        h.dialogue_end(player)?;
                        cook(h, player, &self.object, self.initial, SINEW, 1)?;
                    }
                    2 => {
                        h.dialogue_end(player)?;
                        let product = for_id(self.initial).ok_or(Error::Null)?.cooked;
                        cook(h, player, &self.object, self.initial, product, 1)?;
                    }
                    _ => {}
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// `display()`: the "How many would you like to cook?" chatbox (307).
    pub fn display<H: Host<Node = N> + ?Sized>(&mut self, h: &mut H, player: &N) -> Result<()> {
        h.send_item_zoom(player, self.initial, 160, 307, 2)?;
        let name = h.item_name(self.initial)?;
        h.send_string(player, &format!("<br><br><br><br>{name}"), 307, 6)?;
        for (child, x, y) in [
            (0, 12, 15),
            (1, 431, 15),
            (7, 0, 12),
            (3, 58, 27),
            (4, 58, 27),
            (5, 58, 27),
            (6, 58, 27),
        ] {
            h.reposition_child(player, 307, child, x, y)?;
        }
        h.open_chatbox(player, 307)?;
        self.stage = 1;
        Ok(())
    }
}
