//! `Pets` (content/global/skill/summoning/pet/Pets.java): the 85 constants
//! in declaration order, the three lookup maps `forId` reads (baby, then
//! grown, then overgrown item ids; a later constant replaces an earlier one
//! under the same key; grown and overgrown ids only when positive), the
//! stage and NPC lookups, the cat tests and `hasPet`. The `PetDetails`
//! objects are the persisted ones (`player::persisted::familiars`).
use super::{Host, LogLevel, Origin, Result};
use rs09_game_core::java_map::int_key_order;
use std::collections::HashMap;
use std::sync::OnceLock;

/// One `Pets` constant.
#[derive(Debug, PartialEq)]
pub struct Pet {
    pub name: &'static str,
    pub baby_item: i32,
    pub grown_item: i32,
    pub overgrown_item: i32,
    pub baby_npc: i32,
    pub grown_npc: i32,
    pub overgrown_npc: i32,
    pub growth_rate: f64,
    pub summoning_level: i32,
    pub food: &'static [i32],
}

/// `Pets.values()`, transcribed from the pinned Pets.java.
#[rustfmt::skip]
pub const PETS: &[Pet] = &[
    Pet { name: "CAT", baby_item: 1555, grown_item: 1561, overgrown_item: 1567, baby_npc: 761, grown_npc: 768, overgrown_npc: 774, growth_rate: 0.0154320987654321, summoning_level: 0, food: &[321, 319, 363, 365, 341, 339, 15264, 345, 347, 377, 379, 353, 355, 389, 391, 7944, 7946, 349, 351, 331, 329, 327, 325, 395, 397, 383, 385, 317, 315, 371, 373, 335, 333, 359, 361, 15264, 15270, 1927] },
    Pet { name: "CAT_1", baby_item: 1556, grown_item: 1562, overgrown_item: 1568, baby_npc: 762, grown_npc: 769, overgrown_npc: 775, growth_rate: 0.0154320987654321, summoning_level: 0, food: &[321, 319, 363, 365, 341, 339, 15264, 345, 347, 377, 379, 353, 355, 389, 391, 7944, 7946, 349, 351, 331, 329, 327, 325, 395, 397, 383, 385, 317, 315, 371, 373, 335, 333, 359, 361, 15264, 15270, 1927] },
    Pet { name: "CAT_2", baby_item: 1557, grown_item: 1563, overgrown_item: 1569, baby_npc: 763, grown_npc: 770, overgrown_npc: 776, growth_rate: 0.0154320987654321, summoning_level: 0, food: &[321, 319, 363, 365, 341, 339, 15264, 345, 347, 377, 379, 353, 355, 389, 391, 7944, 7946, 349, 351, 331, 329, 327, 325, 395, 397, 383, 385, 317, 315, 371, 373, 335, 333, 359, 361, 15264, 15270, 1927] },
    Pet { name: "CAT_3", baby_item: 1558, grown_item: 1564, overgrown_item: 1570, baby_npc: 764, grown_npc: 771, overgrown_npc: 777, growth_rate: 0.0154320987654321, summoning_level: 0, food: &[321, 319, 363, 365, 341, 339, 15264, 345, 347, 377, 379, 353, 355, 389, 391, 7944, 7946, 349, 351, 331, 329, 327, 325, 395, 397, 383, 385, 317, 315, 371, 373, 335, 333, 359, 361, 15264, 15270, 1927] },
    Pet { name: "CAT_4", baby_item: 1559, grown_item: 1565, overgrown_item: 1571, baby_npc: 765, grown_npc: 772, overgrown_npc: 778, growth_rate: 0.0154320987654321, summoning_level: 0, food: &[321, 319, 363, 365, 341, 339, 15264, 345, 347, 377, 379, 353, 355, 389, 391, 7944, 7946, 349, 351, 331, 329, 327, 325, 395, 397, 383, 385, 317, 315, 371, 373, 335, 333, 359, 361, 15264, 15270, 1927] },
    Pet { name: "CAT_5", baby_item: 1560, grown_item: 1566, overgrown_item: 1572, baby_npc: 766, grown_npc: 773, overgrown_npc: 779, growth_rate: 0.0154320987654321, summoning_level: 0, food: &[321, 319, 363, 365, 341, 339, 15264, 345, 347, 377, 379, 353, 355, 389, 391, 7944, 7946, 349, 351, 331, 329, 327, 325, 395, 397, 383, 385, 317, 315, 371, 373, 335, 333, 359, 361, 15264, 15270, 1927] },
    Pet { name: "HELLCAT", baby_item: 7583, grown_item: 7582, overgrown_item: 7581, baby_npc: 3505, grown_npc: 3504, overgrown_npc: 3503, growth_rate: 0.0154320987654321, summoning_level: 0, food: &[321, 319, 363, 365, 341, 339, 15264, 345, 347, 377, 379, 353, 355, 389, 391, 7944, 7946, 349, 351, 331, 329, 327, 325, 395, 397, 383, 385, 317, 315, 371, 373, 335, 333, 359, 361, 15264, 15270, 1927] },
    Pet { name: "CAT_6", baby_item: 14089, grown_item: 14090, overgrown_item: 15092, baby_npc: 8217, grown_npc: 8214, overgrown_npc: 8216, growth_rate: 0.0154320987654321, summoning_level: 0, food: &[321, 319, 363, 365, 341, 339, 15264, 345, 347, 377, 379, 353, 355, 389, 391, 7944, 7946, 349, 351, 331, 329, 327, 325, 395, 397, 383, 385, 317, 315, 371, 373, 335, 333, 359, 361, 15264, 15270, 1927] },
    Pet { name: "CLOCKWORK_CAT", baby_item: 7771, grown_item: 7772, overgrown_item: -1, baby_npc: 3598, grown_npc: -1, overgrown_npc: -1, growth_rate: 0.0, summoning_level: 0, food: &[] },
    Pet { name: "BULLDOG", baby_item: 12522, grown_item: 12523, overgrown_item: -1, baby_npc: 6969, grown_npc: 6968, overgrown_npc: -1, growth_rate: 0.0033333333333333, summoning_level: 4, food: &[2132, 2134, 2136, 2138, 10816, 9986, 9978, 526] },
    Pet { name: "BULLDOG_1", baby_item: 12720, grown_item: 12721, overgrown_item: -1, baby_npc: 7259, grown_npc: 7257, overgrown_npc: -1, growth_rate: 0.0033333333333333, summoning_level: 4, food: &[2132, 2134, 2136, 2138, 10816, 9986, 9978, 526] },
    Pet { name: "BULLDOG_2", baby_item: 12722, grown_item: 12723, overgrown_item: -1, baby_npc: 7260, grown_npc: 7258, overgrown_npc: -1, growth_rate: 0.0033333333333333, summoning_level: 4, food: &[2132, 2134, 2136, 2138, 10816, 9986, 9978, 526] },
    Pet { name: "DALMATIAN", baby_item: 12518, grown_item: 12519, overgrown_item: -1, baby_npc: 6964, grown_npc: 6965, overgrown_npc: -1, growth_rate: 0.0033333333333333, summoning_level: 4, food: &[2132, 2134, 2136, 2138, 10816, 9986, 9978, 526] },
    Pet { name: "DALMATIAN_1", baby_item: 12712, grown_item: 12713, overgrown_item: -1, baby_npc: 7249, grown_npc: 7250, overgrown_npc: -1, growth_rate: 0.0033333333333333, summoning_level: 4, food: &[2132, 2134, 2136, 2138, 10816, 9986, 9978, 526] },
    Pet { name: "DALMATIAN_2", baby_item: 12714, grown_item: 12715, overgrown_item: -1, baby_npc: 7251, grown_npc: 7252, overgrown_npc: -1, growth_rate: 0.0033333333333333, summoning_level: 4, food: &[2132, 2134, 2136, 2138, 10816, 9986, 9978, 526] },
    Pet { name: "GREYHOUND", baby_item: 12514, grown_item: 12515, overgrown_item: -1, baby_npc: 6960, grown_npc: 6961, overgrown_npc: -1, growth_rate: 0.0033333333333333, summoning_level: 4, food: &[2132, 2134, 2136, 2138, 10816, 9986, 9978, 526] },
    Pet { name: "GREYHOUND_1", baby_item: 12704, grown_item: 12705, overgrown_item: -1, baby_npc: 7241, grown_npc: 7242, overgrown_npc: -1, growth_rate: 0.0033333333333333, summoning_level: 4, food: &[2132, 2134, 2136, 2138, 10816, 9986, 9978, 526] },
    Pet { name: "GREYHOUND_2", baby_item: 12706, grown_item: 12707, overgrown_item: -1, baby_npc: 7243, grown_npc: 7244, overgrown_npc: -1, growth_rate: 0.0033333333333333, summoning_level: 4, food: &[2132, 2134, 2136, 2138, 10816, 9986, 9978, 526] },
    Pet { name: "LABRADOR", baby_item: 12516, grown_item: 12517, overgrown_item: -1, baby_npc: 6962, grown_npc: 6963, overgrown_npc: -1, growth_rate: 0.0033333333333333, summoning_level: 4, food: &[2132, 2134, 2136, 2138, 10816, 9986, 9978, 526] },
    Pet { name: "LABRADOR_1", baby_item: 12708, grown_item: 12709, overgrown_item: -1, baby_npc: 7245, grown_npc: 7246, overgrown_npc: -1, growth_rate: 0.0033333333333333, summoning_level: 4, food: &[2132, 2134, 2136, 2138, 10816, 9986, 9978, 526] },
    Pet { name: "LABRADOR_2", baby_item: 12710, grown_item: 12711, overgrown_item: -1, baby_npc: 7247, grown_npc: 7248, overgrown_npc: -1, growth_rate: 0.0033333333333333, summoning_level: 4, food: &[2132, 2134, 2136, 2138, 10816, 9986, 9978, 526] },
    Pet { name: "SHEEPDOG", baby_item: 12520, grown_item: 12521, overgrown_item: -1, baby_npc: 6966, grown_npc: 6967, overgrown_npc: -1, growth_rate: 0.0033333333333333, summoning_level: 4, food: &[2132, 2134, 2136, 2138, 10816, 9986, 9978, 526] },
    Pet { name: "SHEEPDOG_1", baby_item: 12716, grown_item: 12717, overgrown_item: -1, baby_npc: 7253, grown_npc: 7254, overgrown_npc: -1, growth_rate: 0.0033333333333333, summoning_level: 4, food: &[2132, 2134, 2136, 2138, 10816, 9986, 9978, 526] },
    Pet { name: "SHEEPDOG_2", baby_item: 12718, grown_item: 12719, overgrown_item: -1, baby_npc: 7255, grown_npc: 7256, overgrown_npc: -1, growth_rate: 0.0033333333333333, summoning_level: 4, food: &[2132, 2134, 2136, 2138, 10816, 9986, 9978, 526] },
    Pet { name: "TERRIER", baby_item: 12512, grown_item: 12513, overgrown_item: -1, baby_npc: 6958, grown_npc: 6959, overgrown_npc: -1, growth_rate: 0.0033333333333333, summoning_level: 4, food: &[2132, 2134, 2136, 2138, 10816, 9986, 9978, 526] },
    Pet { name: "TERRIER_1", baby_item: 12700, grown_item: 12701, overgrown_item: -1, baby_npc: 7237, grown_npc: 7238, overgrown_npc: -1, growth_rate: 0.0033333333333333, summoning_level: 4, food: &[2132, 2134, 2136, 2138, 10816, 9986, 9978, 526] },
    Pet { name: "TERRIER_2", baby_item: 12702, grown_item: 12703, overgrown_item: -1, baby_npc: 7239, grown_npc: 7240, overgrown_npc: -1, growth_rate: 0.0033333333333333, summoning_level: 4, food: &[2132, 2134, 2136, 2138, 10816, 9986, 9978, 526] },
    Pet { name: "GECKO", baby_item: 12488, grown_item: 12489, overgrown_item: -1, baby_npc: 6915, grown_npc: 6916, overgrown_npc: -1, growth_rate: 0.005, summoning_level: 10, food: &[12125, 12127] },
    Pet { name: "GECKO_1", baby_item: 12738, grown_item: 12742, overgrown_item: -1, baby_npc: 7277, grown_npc: 7281, overgrown_npc: -1, growth_rate: 0.005, summoning_level: 10, food: &[12125, 12127] },
    Pet { name: "GECKO_2", baby_item: 12739, grown_item: 12743, overgrown_item: -1, baby_npc: 7278, grown_npc: 7282, overgrown_npc: -1, growth_rate: 0.005, summoning_level: 10, food: &[12125, 12127] },
    Pet { name: "GECKO_3", baby_item: 12740, grown_item: 12744, overgrown_item: -1, baby_npc: 7279, grown_npc: 7283, overgrown_npc: -1, growth_rate: 0.005, summoning_level: 10, food: &[12125, 12127] },
    Pet { name: "GECKO_4", baby_item: 12741, grown_item: 12745, overgrown_item: -1, baby_npc: 7280, grown_npc: 7284, overgrown_npc: -1, growth_rate: 0.005, summoning_level: 10, food: &[12125, 12127] },
    Pet { name: "PLATYPUS", baby_item: 12551, grown_item: 12548, overgrown_item: -1, baby_npc: 7018, grown_npc: 7015, overgrown_npc: -1, growth_rate: 0.0046296296296296, summoning_level: 10, food: &[321, 363, 341, 15264, 345, 377, 353, 389, 7944, 349, 331, 327, 395, 383, 317, 371, 335, 359, 15264, 15270, 313, 12129] },
    Pet { name: "PLATYPUS_1", baby_item: 12552, grown_item: 12549, overgrown_item: -1, baby_npc: 7019, grown_npc: 7016, overgrown_npc: -1, growth_rate: 0.0046296296296296, summoning_level: 10, food: &[321, 363, 341, 15264, 345, 377, 353, 389, 7944, 349, 331, 327, 395, 383, 317, 371, 335, 359, 15264, 15270, 313, 12129] },
    Pet { name: "PLATYPUS_2", baby_item: 12553, grown_item: 12550, overgrown_item: -1, baby_npc: 7020, grown_npc: 7017, overgrown_npc: -1, growth_rate: 0.0046296296296296, summoning_level: 10, food: &[321, 363, 341, 15264, 345, 377, 353, 389, 7944, 349, 331, 327, 395, 383, 317, 371, 335, 359, 15264, 15270, 313, 12129] },
    Pet { name: "BROAV", baby_item: 14533, grown_item: -1, overgrown_item: -1, baby_npc: 8491, grown_npc: -1, overgrown_npc: -1, growth_rate: 0.0, summoning_level: 23, food: &[2970] },
    Pet { name: "PENGUIN", baby_item: 12481, grown_item: 12482, overgrown_item: -1, baby_npc: 6908, grown_npc: 6909, overgrown_npc: -1, growth_rate: 0.0046296296296296, summoning_level: 30, food: &[321, 363, 341, 15264, 345, 377, 353, 389, 7944, 349, 331, 327, 395, 383, 317, 371, 335, 359, 15264, 15270] },
    Pet { name: "PENGUIN_1", baby_item: 12763, grown_item: 12762, overgrown_item: -1, baby_npc: 7313, grown_npc: 7314, overgrown_npc: -1, growth_rate: 0.0046296296296296, summoning_level: 30, food: &[321, 363, 341, 15264, 345, 377, 353, 389, 7944, 349, 331, 327, 395, 383, 317, 371, 335, 359, 15264, 15270] },
    Pet { name: "PENGUIN_2", baby_item: 12765, grown_item: 12764, overgrown_item: -1, baby_npc: 7316, grown_npc: 7317, overgrown_npc: -1, growth_rate: 0.0046296296296296, summoning_level: 30, food: &[321, 363, 341, 15264, 345, 377, 353, 389, 7944, 349, 331, 327, 395, 383, 317, 371, 335, 359, 15264, 15270] },
    Pet { name: "GIANT_CRAB", baby_item: 12500, grown_item: 12501, overgrown_item: -1, baby_npc: 6947, grown_npc: 6948, overgrown_npc: -1, growth_rate: 0.0069444444444444, summoning_level: 40, food: &[321, 363, 341, 15264, 345, 377, 353, 389, 7944, 349, 331, 327, 395, 383, 317, 371, 335, 359, 15264, 15270] },
    Pet { name: "GIANT_CRAB_1", baby_item: 12746, grown_item: 12747, overgrown_item: -1, baby_npc: 7293, grown_npc: 7294, overgrown_npc: -1, growth_rate: 0.0069444444444444, summoning_level: 40, food: &[321, 363, 341, 15264, 345, 377, 353, 389, 7944, 349, 331, 327, 395, 383, 317, 371, 335, 359, 15264, 15270] },
    Pet { name: "GIANT_CRAB_2", baby_item: 12748, grown_item: 12749, overgrown_item: -1, baby_npc: 7295, grown_npc: 7296, overgrown_npc: -1, growth_rate: 0.0069444444444444, summoning_level: 40, food: &[321, 363, 341, 15264, 345, 377, 353, 389, 7944, 349, 331, 327, 395, 383, 317, 371, 335, 359, 15264, 15270] },
    Pet { name: "GIANT_CRAB_3", baby_item: 12750, grown_item: 12751, overgrown_item: -1, baby_npc: 7297, grown_npc: 7298, overgrown_npc: -1, growth_rate: 0.0069444444444444, summoning_level: 40, food: &[321, 363, 341, 15264, 345, 377, 353, 389, 7944, 349, 331, 327, 395, 383, 317, 371, 335, 359, 15264, 15270] },
    Pet { name: "GIANT_CRAB_4", baby_item: 12752, grown_item: 12753, overgrown_item: -1, baby_npc: 7299, grown_npc: 7300, overgrown_npc: -1, growth_rate: 0.0069444444444444, summoning_level: 40, food: &[321, 363, 341, 15264, 345, 377, 353, 389, 7944, 349, 331, 327, 395, 383, 317, 371, 335, 359, 15264, 15270] },
    Pet { name: "RAVEN", baby_item: 12484, grown_item: 12485, overgrown_item: -1, baby_npc: 6911, grown_npc: 6912, overgrown_npc: -1, growth_rate: 0.00698888, summoning_level: 50, food: &[313, 12129] },
    Pet { name: "RAVEN_1", baby_item: 12724, grown_item: 12725, overgrown_item: -1, baby_npc: 7261, grown_npc: 7262, overgrown_npc: -1, growth_rate: 0.00698888, summoning_level: 50, food: &[313, 12129] },
    Pet { name: "RAVEN_2", baby_item: 12726, grown_item: 12727, overgrown_item: -1, baby_npc: 7263, grown_npc: 7264, overgrown_npc: -1, growth_rate: 0.00698888, summoning_level: 50, food: &[313, 12129] },
    Pet { name: "RAVEN_3", baby_item: 12728, grown_item: 12729, overgrown_item: -1, baby_npc: 7265, grown_npc: 7266, overgrown_npc: -1, growth_rate: 0.00698888, summoning_level: 50, food: &[313, 12129] },
    Pet { name: "RAVEN_4", baby_item: 12730, grown_item: 12731, overgrown_item: -1, baby_npc: 7267, grown_npc: 7268, overgrown_npc: -1, growth_rate: 0.00698888, summoning_level: 50, food: &[313, 12129] },
    Pet { name: "RAVEN_5", baby_item: 12732, grown_item: 12733, overgrown_item: -1, baby_npc: 7269, grown_npc: 7270, overgrown_npc: -1, growth_rate: 0.00698888, summoning_level: 50, food: &[313, 12129] },
    Pet { name: "SQUIRREL", baby_item: 12490, grown_item: 12491, overgrown_item: -1, baby_npc: 6919, grown_npc: 6920, overgrown_npc: -1, growth_rate: 0.0071225071225071, summoning_level: 60, food: &[12130] },
    Pet { name: "SQUIRREL_1", baby_item: 12754, grown_item: 12755, overgrown_item: -1, baby_npc: 7301, grown_npc: 7302, overgrown_npc: -1, growth_rate: 0.0071225071225071, summoning_level: 60, food: &[12130] },
    Pet { name: "SQUIRREL_2", baby_item: 12756, grown_item: 12757, overgrown_item: -1, baby_npc: 7303, grown_npc: 7304, overgrown_npc: -1, growth_rate: 0.0071225071225071, summoning_level: 60, food: &[12130] },
    Pet { name: "SQUIRREL_3", baby_item: 12758, grown_item: 12759, overgrown_item: -1, baby_npc: 7305, grown_npc: 7306, overgrown_npc: -1, growth_rate: 0.0071225071225071, summoning_level: 60, food: &[12130] },
    Pet { name: "SQUIRREL_4", baby_item: 12760, grown_item: 12761, overgrown_item: -1, baby_npc: 7307, grown_npc: 7308, overgrown_npc: -1, growth_rate: 0.0071225071225071, summoning_level: 60, food: &[12130] },
    Pet { name: "SARADOMIN_OWL", baby_item: 12503, grown_item: 12504, overgrown_item: 12505, baby_npc: 6949, grown_npc: 6950, overgrown_npc: 6951, growth_rate: 0.0069444444444444, summoning_level: 70, food: &[313, 12129] },
    Pet { name: "ZAMORAK_HAWK", baby_item: 12506, grown_item: 12507, overgrown_item: 12508, baby_npc: 6952, grown_npc: 6953, overgrown_npc: 6954, growth_rate: 0.0069444444444444, summoning_level: 70, food: &[313, 12129] },
    Pet { name: "GUTHIX_RAPTOR", baby_item: 12509, grown_item: 12510, overgrown_item: 12511, baby_npc: 6955, grown_npc: 6956, overgrown_npc: 6957, growth_rate: 0.0069444444444444, summoning_level: 70, food: &[313, 12129] },
    Pet { name: "EX_EX_PARROT", baby_item: 13335, grown_item: -1, overgrown_item: -1, baby_npc: 7844, grown_npc: -1, overgrown_npc: -1, growth_rate: 0.0, summoning_level: 71, food: &[13379] },
    Pet { name: "CUTE_PHOENIX_EGGLING", baby_item: 14627, grown_item: -1, overgrown_item: -1, baby_npc: 8578, grown_npc: -1, overgrown_npc: -1, growth_rate: 0.0, summoning_level: 72, food: &[592] },
    Pet { name: "MEAN_PHOENIX_EGGLING", baby_item: 14626, grown_item: -1, overgrown_item: -1, baby_npc: 8577, grown_npc: -1, overgrown_npc: -1, growth_rate: 0.0, summoning_level: 72, food: &[592] },
    Pet { name: "RACCOON", baby_item: 12486, grown_item: 12487, overgrown_item: -1, baby_npc: 6913, grown_npc: 6914, overgrown_npc: -1, growth_rate: 0.0029444444444444, summoning_level: 80, food: &[321, 363, 341, 15264, 345, 377, 353, 389, 7944, 349, 331, 327, 395, 383, 317, 371, 335, 359, 15264, 15270, 2132, 2134, 2136, 2138, 10816, 9986, 9978] },
    Pet { name: "RACCOON_1", baby_item: 12734, grown_item: 12735, overgrown_item: -1, baby_npc: 7271, grown_npc: 7272, overgrown_npc: -1, growth_rate: 0.0029444444444444, summoning_level: 80, food: &[321, 363, 341, 15264, 345, 377, 353, 389, 7944, 349, 331, 327, 395, 383, 317, 371, 335, 359, 15264, 15270, 2132, 2134, 2136, 2138, 10816, 9986, 9978] },
    Pet { name: "RACCOON_2", baby_item: 12736, grown_item: 12737, overgrown_item: -1, baby_npc: 7273, grown_npc: 7274, overgrown_npc: -1, growth_rate: 0.0029444444444444, summoning_level: 80, food: &[321, 363, 341, 15264, 345, 377, 353, 389, 7944, 349, 331, 327, 395, 383, 317, 371, 335, 359, 15264, 15270, 2132, 2134, 2136, 2138, 10816, 9986, 9978] },
    Pet { name: "VULTURE", baby_item: 12498, grown_item: 12499, overgrown_item: -1, baby_npc: 6945, grown_npc: 6946, overgrown_npc: -1, growth_rate: 0.0078, summoning_level: 85, food: &[313, 12129] },
    Pet { name: "VULTURE_1", baby_item: 12766, grown_item: 12767, overgrown_item: -1, baby_npc: 7319, grown_npc: 7320, overgrown_npc: -1, growth_rate: 0.0078, summoning_level: 85, food: &[313, 12129] },
    Pet { name: "VULTURE_2", baby_item: 12768, grown_item: 12769, overgrown_item: -1, baby_npc: 7321, grown_npc: 7322, overgrown_npc: -1, growth_rate: 0.0078, summoning_level: 85, food: &[313, 12129] },
    Pet { name: "VULTURE_3", baby_item: 12770, grown_item: 12771, overgrown_item: -1, baby_npc: 7323, grown_npc: 7324, overgrown_npc: -1, growth_rate: 0.0078, summoning_level: 85, food: &[313, 12129] },
    Pet { name: "VULTURE_4", baby_item: 12772, grown_item: 12773, overgrown_item: -1, baby_npc: 7325, grown_npc: 7326, overgrown_npc: -1, growth_rate: 0.0078, summoning_level: 85, food: &[313, 12129] },
    Pet { name: "VULTURE_5", baby_item: 12774, grown_item: 12775, overgrown_item: -1, baby_npc: 7327, grown_npc: 7328, overgrown_npc: -1, growth_rate: 0.0078, summoning_level: 85, food: &[313, 12129] },
    Pet { name: "CHAMELEON", baby_item: 12492, grown_item: 12493, overgrown_item: -1, baby_npc: 6922, grown_npc: 6923, overgrown_npc: -1, growth_rate: 0.0069444444444444, summoning_level: 90, food: &[12125] },
    Pet { name: "MONKEY", baby_item: 12496, grown_item: 12497, overgrown_item: -1, baby_npc: 6942, grown_npc: 6943, overgrown_npc: -1, growth_rate: 0.0069444444444444, summoning_level: 95, food: &[1963] },
    Pet { name: "MONKEY_1", baby_item: 12682, grown_item: 12683, overgrown_item: -1, baby_npc: 7210, grown_npc: 7211, overgrown_npc: -1, growth_rate: 0.0069444444444444, summoning_level: 95, food: &[1963] },
    Pet { name: "MONKEY_2", baby_item: 12684, grown_item: 12685, overgrown_item: -1, baby_npc: 7212, grown_npc: 7213, overgrown_npc: -1, growth_rate: 0.0069444444444444, summoning_level: 95, food: &[1963] },
    Pet { name: "MONKEY_3", baby_item: 12686, grown_item: 12687, overgrown_item: -1, baby_npc: 7214, grown_npc: 7215, overgrown_npc: -1, growth_rate: 0.0069444444444444, summoning_level: 95, food: &[1963] },
    Pet { name: "MONKEY_4", baby_item: 12688, grown_item: 12689, overgrown_item: -1, baby_npc: 7216, grown_npc: 7217, overgrown_npc: -1, growth_rate: 0.0069444444444444, summoning_level: 95, food: &[1963] },
    Pet { name: "MONKEY_5", baby_item: 12690, grown_item: 12691, overgrown_item: -1, baby_npc: 7218, grown_npc: 7219, overgrown_npc: -1, growth_rate: 0.0069444444444444, summoning_level: 95, food: &[1963] },
    Pet { name: "MONKEY_6", baby_item: 12692, grown_item: 12693, overgrown_item: -1, baby_npc: 7220, grown_npc: 7221, overgrown_npc: -1, growth_rate: 0.0069444444444444, summoning_level: 95, food: &[1963] },
    Pet { name: "MONKEY_7", baby_item: 12694, grown_item: 12695, overgrown_item: -1, baby_npc: 7222, grown_npc: 7223, overgrown_npc: -1, growth_rate: 0.0069444444444444, summoning_level: 95, food: &[1963] },
    Pet { name: "MONKEY_8", baby_item: 12696, grown_item: 12697, overgrown_item: -1, baby_npc: 7224, grown_npc: 7225, overgrown_npc: -1, growth_rate: 0.0069444444444444, summoning_level: 95, food: &[1963] },
    Pet { name: "MONKEY_9", baby_item: 12698, grown_item: 12699, overgrown_item: -1, baby_npc: 7226, grown_npc: 7227, overgrown_npc: -1, growth_rate: 0.0069444444444444, summoning_level: 95, food: &[1963] },
    Pet { name: "BABY_DRAGON", baby_item: 12469, grown_item: 12470, overgrown_item: -1, baby_npc: 6900, grown_npc: 6901, overgrown_npc: -1, growth_rate: 0.0052, summoning_level: 99, food: &[2132, 2134, 2136, 2138, 10816, 9986, 9978, 321, 363, 341, 15264, 345, 377, 353, 389, 7944, 349, 331, 327, 395, 383, 317, 371, 335, 359, 15264, 15270] },
    Pet { name: "BABY_DRAGON_1", baby_item: 12471, grown_item: 12472, overgrown_item: -1, baby_npc: 6902, grown_npc: 6903, overgrown_npc: -1, growth_rate: 0.0052, summoning_level: 99, food: &[2132, 2134, 2136, 2138, 10816, 9986, 9978, 321, 363, 341, 15264, 345, 377, 353, 389, 7944, 349, 331, 327, 395, 383, 317, 371, 335, 359, 15264, 15270] },
    Pet { name: "BABY_DRAGON_2", baby_item: 12473, grown_item: 12474, overgrown_item: -1, baby_npc: 6904, grown_npc: 6905, overgrown_npc: -1, growth_rate: 0.0052, summoning_level: 99, food: &[2132, 2134, 2136, 2138, 10816, 9986, 9978, 321, 363, 341, 15264, 345, 377, 353, 389, 7944, 349, 331, 327, 395, 383, 317, 371, 335, 359, 15264, 15270] },
    Pet { name: "BABY_DRAGON_3", baby_item: 12475, grown_item: 12476, overgrown_item: -1, baby_npc: 6906, grown_npc: 6907, overgrown_npc: -1, growth_rate: 0.0052, summoning_level: 99, food: &[2132, 2134, 2136, 2138, 10816, 9986, 9978, 321, 363, 341, 15264, 345, 377, 353, 389, 7944, 349, 331, 327, 395, 383, 317, 371, 335, 359, 15264, 15270] },
];

/// The constants whose `isKitten`/`isGrownCat` switch matches.
const CATS: &[&str] = &[
    "CAT", "CAT_1", "CAT_2", "CAT_3", "CAT_4", "CAT_5", "CAT_6", "HELLCAT",
];

struct Maps {
    baby: HashMap<i32, usize>,
    grown: HashMap<i32, usize>,
    overgrown: HashMap<i32, usize>,
    /// Each map's keys in the JDK's iteration order (`hasPet` walks them).
    orders: [Vec<i32>; 3],
}

fn maps() -> &'static Maps {
    static MAPS: OnceLock<Maps> = OnceLock::new();
    MAPS.get_or_init(|| {
        let (mut baby, mut grown, mut overgrown) = (HashMap::new(), HashMap::new(), HashMap::new());
        let (mut baby_keys, mut grown_keys, mut overgrown_keys) =
            (Vec::new(), Vec::new(), Vec::new());
        for (index, pet) in PETS.iter().enumerate() {
            if baby.insert(pet.baby_item, index).is_none() {
                baby_keys.push(pet.baby_item);
            }
            if pet.grown_item > 0 {
                if grown.insert(pet.grown_item, index).is_none() {
                    grown_keys.push(pet.grown_item);
                }
                if pet.overgrown_item > 0 && overgrown.insert(pet.overgrown_item, index).is_none() {
                    overgrown_keys.push(pet.overgrown_item);
                }
            }
        }
        let orders = [
            int_key_order(baby_keys),
            int_key_order(grown_keys),
            int_key_order(overgrown_keys),
        ];
        Maps {
            baby,
            grown,
            overgrown,
            orders,
        }
    })
}

impl Pet {
    /// `Pets.forId(itemId)`.
    pub fn for_id(item_id: i32) -> Option<&'static Pet> {
        let m = maps();
        m.baby
            .get(&item_id)
            .or_else(|| m.grown.get(&item_id))
            .or_else(|| m.overgrown.get(&item_id))
            .map(|&i| &PETS[i])
    }
    /// The constant's ordinal.
    pub fn ordinal(&self) -> usize {
        PETS.iter()
            .position(|p| std::ptr::eq(p, self))
            .unwrap_or(usize::MAX)
    }
    /// `getNpcId(itemId)`: logs and answers -1 for an item of no stage.
    pub fn npc_id<H: Host>(&self, host: &mut H, item_id: i32) -> Result<i32> {
        if item_id == self.baby_item {
            return Ok(self.baby_npc);
        }
        if item_id == self.grown_item {
            return Ok(self.grown_npc);
        }
        if item_id == self.overgrown_item {
            return Ok(self.overgrown_npc);
        }
        host.log(
            Origin::Pets,
            LogLevel::Err,
            &format!("Could not locate NPC ID for pet item {item_id}"),
        )?;
        Ok(-1)
    }
    /// `getNextStageItemId(itemId)`.
    pub fn next_stage_item_id(&self, item_id: i32) -> i32 {
        if item_id == self.baby_item {
            return self.grown_item;
        }
        if item_id == self.grown_item {
            return self.overgrown_item;
        }
        -1
    }
    fn is_cat(&self) -> bool {
        CATS.contains(&self.name)
    }
    /// `isKitten(id)`.
    pub fn is_kitten(&self, id: i32) -> bool {
        self.is_cat() && id == self.baby_item
    }
    /// `isGrownCat(id)`.
    pub fn is_grown_cat(&self, id: i32) -> bool {
        self.is_cat() && (id == self.grown_item || id == self.overgrown_item)
    }
}

/// `Pets.hasPet(player)`: the baby, grown and overgrown keys in each map's
/// iteration order, until the inventory holds one.
pub fn has_pet<H: Host>(host: &mut H, player: &H::Node) -> Result<bool> {
    for keys in &maps().orders {
        for &id in keys {
            let inventory = host.inventory(player)?;
            if host.contains_one(&inventory, id)? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// `Pet.getIds()`: the NPC ids of the `Pet` class.
#[rustfmt::skip]
pub const PET_NPC_IDS: &[i32] = &[761, 762, 763, 764, 765, 766, 3505, 3598, 6969, 7259, 7260, 6964, 7249, 7251, 6960, 7241, 7243, 6962, 7245, 7247, 6966, 7253, 7255, 6958, 7237, 7239, 6915, 7277, 7278, 7279, 7280, 7018, 7019, 7020, 6908, 7313, 7316, 6947, 7293, 7295, 7297, 7299, 6911, 7261, 7263, 7265, 7267, 7269, 6919, 7301, 7303, 7305, 7307, 6949, 6952, 6955, 6913, 7271, 7273, 6945, 7319, 7321, 7323, 7325, 7327, 6922, 6942, 7210, 7212, 7214, 7216, 7218, 7220, 7222, 7224, 7226, 6900, 6902, 6904, 6906, 768, 769, 770, 771, 772, 773, 3504, 6968, 7257, 7258, 6965, 7250, 7252, 6961, 7242, 7244, 6963, 7246, 7248, 6967, 7254, 7256, 6859, 7238, 7240, 6916, 7281, 7282, 7283, 7284, 7015, 7016, 7017, 6909, 7314, 7317, 6948, 7294, 7296, 7298, 7300, 6912, 7262, 7264, 7266, 7268, 7270, 6920, 7302, 7304, 7306, 7308, 6950, 6953, 6956, 6914, 7272, 7274, 6946, 7320, 7322, 7324, 7326, 7328, 6923, 6943, 7211, 7213, 7215, 7217, 7219, 7221, 7223, 7225, 7227, 6901, 6903, 6905, 6907, 774, 775, 776, 777, 778, 779, 3503, 6951, 6954, 6957];
