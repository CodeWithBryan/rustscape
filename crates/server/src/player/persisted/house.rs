//! HouseManager's persisted values, as ordinals: location, style, crest,
//! servant and the 4 x 8 x 8 rooms with each hotspot's decoration index.
//! Only the saving side is native. HouseManager.parse builds rooms from
//! RoomProperties, configures them for the style and reads hotspots by
//! index; it stays a required service, as do region building and the zone.
use crate::player::save::House;

/// HouseLocation.NOWHERE, HousingStyle.BASIC_WOOD, CrestType.NULL.
pub const FRESH_LOCATION: i32 = 0;
pub const FRESH_STYLE: i32 = 0;
pub const FRESH_CREST: i32 = 16;

/// A new character's house: no servant and every room empty.
pub fn fresh() -> House {
    House {
        location: FRESH_LOCATION,
        style: FRESH_STYLE,
        crest: FRESH_CREST,
        servant: None,
        rooms: vec![vec![vec![None; 8]; 8]; 4],
    }
}
