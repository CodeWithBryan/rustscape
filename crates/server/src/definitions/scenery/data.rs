// Authored by tools/port_scenery_schema.py; source field names are retained in observations.
use super::{Property, Shared};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct SceneryDefinition {
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
    #[serde(rename = "originalColors")]
    pub original_colors: Option<Shared<Vec<i16>>>,
    #[serde(rename = "childrenIds")]
    pub children_ids: Option<Shared<Vec<i32>>>,
    #[serde(rename = "modelIds")]
    pub model_ids: Option<Shared<Vec<i32>>>,
    #[serde(rename = "modelConfiguration")]
    pub model_configuration: Option<Shared<Vec<i32>>>,
    #[serde(rename = "anIntArray3833")]
    pub an_int_array3833: Option<Shared<Vec<i32>>>,
    #[serde(rename = "anInt3834")]
    pub an_int3834: i32,
    #[serde(rename = "anInt3835")]
    pub an_int3835: i32,
    #[serde(rename = "aByte3837")]
    pub a_byte3837: i8,
    #[serde(rename = "anInt3838")]
    pub an_int3838: i32,
    #[serde(rename = "aBoolean3839")]
    pub a_boolean3839: bool,
    #[serde(rename = "anInt3840")]
    pub an_int3840: i32,
    #[serde(rename = "anInt3841")]
    pub an_int3841: i32,
    #[serde(rename = "anInt3844")]
    pub an_int3844: i32,
    #[serde(rename = "aBoolean3845")]
    pub a_boolean3845: bool,
    #[serde(rename = "aByte3847")]
    pub a_byte3847: i8,
    #[serde(rename = "aByte3849")]
    pub a_byte3849: i8,
    #[serde(rename = "anInt3850")]
    pub an_int3850: i32,
    #[serde(rename = "anInt3851")]
    pub an_int3851: i32,
    #[serde(rename = "secondBool")]
    pub second_bool: bool,
    #[serde(rename = "aBoolean3853")]
    pub a_boolean3853: bool,
    #[serde(rename = "anInt3855")]
    pub an_int3855: i32,
    #[serde(rename = "notClipped")]
    pub not_clipped: bool,
    #[serde(rename = "anInt3857")]
    pub an_int3857: i32,
    #[serde(rename = "aByteArray3858")]
    pub a_byte_array3858: Option<Shared<Vec<i8>>>,
    #[serde(rename = "anIntArray3859")]
    pub an_int_array3859: Option<Shared<Vec<i32>>>,
    #[serde(rename = "anInt3860")]
    pub an_int3860: i32,
    #[serde(rename = "configFileId")]
    pub config_file_id: i32,
    #[serde(rename = "modifiedColors")]
    pub modified_colors: Option<Shared<Vec<i16>>>,
    #[serde(rename = "anInt3865")]
    pub an_int3865: i32,
    #[serde(rename = "aBoolean3866")]
    pub a_boolean3866: bool,
    #[serde(rename = "aBoolean3867")]
    pub a_boolean3867: bool,
    #[serde(rename = "projectileClipped")]
    pub projectile_clipped: bool,
    #[serde(rename = "anIntArray3869")]
    pub an_int_array3869: Option<Shared<Vec<i32>>>,
    #[serde(rename = "aBoolean3870")]
    pub a_boolean3870: bool,
    #[serde(rename = "sizeY")]
    pub size_y: i32,
    #[serde(rename = "aBoolean3872")]
    pub a_boolean3872: bool,
    #[serde(rename = "membersOnly")]
    pub members_only: bool,
    #[serde(rename = "boolean1")]
    pub boolean1: bool,
    #[serde(rename = "anInt3875")]
    pub an_int3875: i32,
    #[serde(rename = "animationId")]
    pub animation_id: i32,
    #[serde(rename = "anInt3877")]
    pub an_int3877: i32,
    #[serde(rename = "anInt3878")]
    pub an_int3878: i32,
    #[serde(rename = "clipType")]
    pub clip_type: i32,
    #[serde(rename = "anInt3881")]
    pub an_int3881: i32,
    #[serde(rename = "anInt3882")]
    pub an_int3882: i32,
    #[serde(rename = "anInt3883")]
    pub an_int3883: i32,
    #[serde(rename = "loader")]
    pub loader: Option<serde_json::Value>,
    #[serde(rename = "anInt3889")]
    pub an_int3889: i32,
    #[serde(rename = "sizeX")]
    pub size_x: i32,
    #[serde(rename = "aBoolean3891")]
    pub a_boolean3891: bool,
    #[serde(rename = "anInt3892")]
    pub an_int3892: i32,
    #[serde(rename = "interactable")]
    pub interactable: i32,
    #[serde(rename = "aBoolean3894")]
    pub a_boolean3894: bool,
    #[serde(rename = "aBoolean3895")]
    pub a_boolean3895: bool,
    #[serde(rename = "anInt3896")]
    pub an_int3896: i32,
    #[serde(rename = "configId")]
    pub config_id: i32,
    #[serde(rename = "aByteArray3899")]
    pub a_byte_array3899: Option<Shared<Vec<i8>>>,
    #[serde(rename = "anInt3900")]
    pub an_int3900: i32,
    #[serde(rename = "anInt3902")]
    pub an_int3902: i32,
    #[serde(rename = "anInt3904")]
    pub an_int3904: i32,
    #[serde(rename = "anInt3905")]
    pub an_int3905: i32,
    #[serde(rename = "aBoolean3906")]
    pub a_boolean3906: bool,
    #[serde(rename = "anIntArray3908")]
    pub an_int_array3908: Option<Shared<Vec<i32>>>,
    #[serde(rename = "aByte3912")]
    pub a_byte3912: i8,
    #[serde(rename = "anInt3913")]
    pub an_int3913: i32,
    #[serde(rename = "aByte3914")]
    pub a_byte3914: i8,
    #[serde(rename = "anInt3915")]
    pub an_int3915: i32,
    #[serde(rename = "anIntArrayArray3916")]
    pub an_int_array_array3916: crate::definitions::item::IntegerRows,
    #[serde(rename = "anInt3917")]
    pub an_int3917: i32,
    #[serde(rename = "aShortArray3919")]
    pub a_short_array3919: Option<Shared<Vec<i16>>>,
    #[serde(rename = "aShortArray3920")]
    pub a_short_array3920: Option<Shared<Vec<i16>>>,
    #[serde(rename = "anInt3921")]
    pub an_int3921: i32,
    #[serde(rename = "aClass194_3922")]
    pub a_class194_3922: Option<serde_json::Value>,
    #[serde(rename = "aBoolean3923")]
    pub a_boolean3923: bool,
    #[serde(rename = "aBoolean3924")]
    pub a_boolean3924: bool,
    #[serde(rename = "walkingFlag")]
    pub walking_flag: i32,
    #[serde(rename = "hasHiddenOptions")]
    pub has_hidden_options: bool,
    #[serde(rename = "mapIcon")]
    pub map_icon: i16,
}
impl Default for SceneryDefinition {
    fn default() -> Self {
        Self {
            id: 0,
            name: Some("null".into()),
            examine: None,
            options: Some(Shared::new(vec![None; 5])),
            handlers: Shared::new(BTreeMap::new()),
            original_colors: None,
            children_ids: None,
            model_ids: None,
            model_configuration: None,
            an_int_array3833: None,
            an_int3834: 0,
            an_int3835: -1,
            a_byte3837: 0,
            an_int3838: -1,
            a_boolean3839: false,
            an_int3840: 0,
            an_int3841: 128,
            an_int3844: -1,
            a_boolean3845: false,
            a_byte3847: 0,
            a_byte3849: 0,
            an_int3850: 0,
            an_int3851: -1,
            second_bool: false,
            a_boolean3853: true,
            an_int3855: -1,
            not_clipped: false,
            an_int3857: -1,
            a_byte_array3858: None,
            an_int_array3859: None,
            an_int3860: -1,
            config_file_id: -1,
            modified_colors: None,
            an_int3865: 255,
            a_boolean3866: false,
            a_boolean3867: false,
            projectile_clipped: true,
            an_int_array3869: None,
            a_boolean3870: false,
            size_y: 1,
            a_boolean3872: true,
            members_only: false,
            boolean1: false,
            an_int3875: 0,
            animation_id: -1,
            an_int3877: 0,
            an_int3878: 0,
            clip_type: 2,
            an_int3881: 0,
            an_int3882: -1,
            an_int3883: 0,
            loader: None,
            an_int3889: 0,
            size_x: 1,
            a_boolean3891: false,
            an_int3892: 64,
            interactable: -1,
            a_boolean3894: false,
            a_boolean3895: true,
            an_int3896: 0,
            config_id: -1,
            a_byte_array3899: None,
            an_int3900: 0,
            an_int3902: 128,
            an_int3904: 0,
            an_int3905: 0,
            a_boolean3906: false,
            an_int_array3908: None,
            a_byte3912: 0,
            an_int3913: -1,
            a_byte3914: 0,
            an_int3915: 0,
            an_int_array_array3916: None,
            an_int3917: 128,
            a_short_array3919: None,
            a_short_array3920: None,
            an_int3921: 0,
            a_class194_3922: None,
            a_boolean3923: false,
            a_boolean3924: false,
            walking_flag: 0,
            has_hidden_options: false,
            map_icon: -1,
        }
    }
}
