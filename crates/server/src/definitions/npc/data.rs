// Authored by tools/port_npc_schema.py; source field names are retained in observations.
use super::{Property, Shared};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct NpcDefinition {
    #[serde(rename = "id")]
    pub id: i32,
    #[serde(rename = "name")]
    pub name: Option<String>,
    #[serde(rename = "examine")]
    pub examine: Option<String>,
    #[serde(rename = "options")]
    pub options: Option<Shared<Vec<Option<String>>>>,
    #[serde(rename = "handlers")]
    pub handlers: Shared<BTreeMap<String, Property>>,
    #[serde(rename = "size")]
    pub size: i32,
    #[serde(rename = "combatLevel")]
    pub combat_level: i32,
    #[serde(rename = "headIcons")]
    pub head_icons: i32,
    #[serde(rename = "isVisibleOnMap")]
    pub is_visible_on_map: bool,
    #[serde(rename = "anInt833")]
    pub an_int833: i32,
    #[serde(rename = "anInt836")]
    pub an_int836: i32,
    #[serde(rename = "anInt837")]
    pub an_int837: i32,
    #[serde(rename = "aBoolean841")]
    pub a_boolean841: bool,
    #[serde(rename = "anInt842")]
    pub an_int842: i32,
    #[serde(rename = "configFileId")]
    pub config_file_id: i32,
    #[serde(rename = "childNPCIds")]
    pub child_npc_ids: Option<Shared<Vec<i32>>>,
    #[serde(rename = "anInt846")]
    pub an_int846: i32,
    #[serde(rename = "anInt850")]
    pub an_int850: i32,
    #[serde(rename = "aByte851")]
    pub a_byte851: i8,
    #[serde(rename = "aBoolean852")]
    pub a_boolean852: bool,
    #[serde(rename = "anInt853")]
    pub an_int853: i32,
    #[serde(rename = "aByte854")]
    pub a_byte854: i8,
    #[serde(rename = "aBoolean856")]
    pub a_boolean856: bool,
    #[serde(rename = "aBoolean857")]
    pub a_boolean857: bool,
    #[serde(rename = "aShortArray859")]
    pub a_short_array859: Option<Shared<Vec<i16>>>,
    #[serde(rename = "aByteArray861")]
    pub a_byte_array861: Option<Shared<Vec<i8>>>,
    #[serde(rename = "aShort862")]
    pub a_short862: i16,
    #[serde(rename = "aBoolean863")]
    pub a_boolean863: bool,
    #[serde(rename = "anInt864")]
    pub an_int864: i32,
    #[serde(rename = "aShortArray866")]
    pub a_short_array866: Option<Shared<Vec<i16>>>,
    #[serde(rename = "anIntArray868")]
    pub an_int_array868: Option<Shared<Vec<i32>>>,
    #[serde(rename = "anInt869")]
    pub an_int869: i32,
    #[serde(rename = "anInt870")]
    pub an_int870: i32,
    #[serde(rename = "anInt871")]
    pub an_int871: i32,
    #[serde(rename = "anInt872")]
    pub an_int872: i32,
    #[serde(rename = "anInt874")]
    pub an_int874: i32,
    #[serde(rename = "anInt875")]
    pub an_int875: i32,
    #[serde(rename = "anInt876")]
    pub an_int876: i32,
    #[serde(rename = "anInt879")]
    pub an_int879: i32,
    #[serde(rename = "aShortArray880")]
    pub a_short_array880: Option<Shared<Vec<i16>>>,
    #[serde(rename = "anInt884")]
    pub an_int884: i32,
    #[serde(rename = "configId")]
    pub config_id: i32,
    #[serde(rename = "anInt889")]
    pub an_int889: i32,
    #[serde(rename = "anIntArray892")]
    pub an_int_array892: Option<Shared<Vec<i32>>>,
    #[serde(rename = "aShort894")]
    pub a_short894: i16,
    #[serde(rename = "aShortArray896")]
    pub a_short_array896: Option<Shared<Vec<i16>>>,
    #[serde(rename = "anInt897")]
    pub an_int897: i32,
    #[serde(rename = "anInt899")]
    pub an_int899: i32,
    #[serde(rename = "anInt901")]
    pub an_int901: i32,
    #[serde(rename = "standAnimation")]
    pub stand_animation: i32,
    #[serde(rename = "walkAnimation")]
    pub walk_animation: i32,
    #[serde(rename = "renderAnimationId")]
    pub render_animation_id: i32,
    #[serde(rename = "combatDistance")]
    pub combat_distance: i32,
    #[serde(rename = "turnAnimation")]
    pub turn_animation: i32,
    #[serde(rename = "turn180Animation")]
    pub turn180_animation: i32,
    #[serde(rename = "turnCWAnimation")]
    pub turn_cw_animation: i32,
    #[serde(rename = "turnCCWAnimation")]
    pub turn_ccw_animation: i32,
    #[serde(rename = "combatGraphics")]
    pub combat_graphics: Option<Shared<Vec<Option<Shared<super::Graphics>>>>>,
    /// `dropTables`: never null in a new definition. It is not a cache or
    /// configuration field, so observations leave it out.
    #[serde(skip)]
    pub drop_tables: Option<crate::world::drop_tables::NpcDropTables>,
}
impl Default for NpcDefinition {
    fn default() -> Self {
        Self {
            id: 0,
            name: Some("null".into()),
            examine: None,
            options: Some(Shared::new(vec![None; 5])),
            handlers: Shared::new(BTreeMap::new()),
            size: 1,
            combat_level: 0,
            head_icons: -1,
            is_visible_on_map: true,
            an_int833: -1,
            an_int836: -1,
            an_int837: -1,
            a_boolean841: true,
            an_int842: -1,
            config_file_id: -1,
            child_npc_ids: None,
            an_int846: -1,
            an_int850: 255,
            a_byte851: -96,
            a_boolean852: true,
            an_int853: 32,
            a_byte854: -16,
            a_boolean856: false,
            a_boolean857: true,
            a_short_array859: None,
            a_byte_array861: None,
            a_short862: 0,
            a_boolean863: false,
            an_int864: 128,
            a_short_array866: None,
            an_int_array868: Some(Shared::new(vec![])),
            an_int869: 0,
            an_int870: -1,
            an_int871: -1,
            an_int872: -1,
            an_int874: -1,
            an_int875: 0,
            an_int876: -1,
            an_int879: -1,
            a_short_array880: None,
            an_int884: -1,
            config_id: -1,
            an_int889: -1,
            an_int_array892: None,
            a_short894: 0,
            a_short_array896: None,
            an_int897: 0,
            an_int899: 128,
            an_int901: -1,
            stand_animation: -1,
            walk_animation: -1,
            render_animation_id: 0,
            combat_distance: 0,
            turn_animation: 0,
            turn180_animation: 0,
            turn_cw_animation: 0,
            turn_ccw_animation: 0,
            combat_graphics: Some(Shared::new(vec![None; 3])),
            drop_tables: Some(crate::world::drop_tables::NpcDropTables::new()),
        }
    }
}
