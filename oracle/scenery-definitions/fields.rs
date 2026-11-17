// Authored by tools/port_scenery_schema.py; differential observation only.
use rs09_server::definitions::scenery::Handle;
use serde_json::{Value, json};
use std::collections::BTreeMap;
pub fn set(h: &Handle, field: &str, value: &Value) {
    let mut h = h.write();
    match field {
        "id" => h.id = serde_json::from_value(value.clone()).unwrap(),
        "name" => h.name = serde_json::from_value(value.clone()).unwrap(),
        "examine" => h.examine = serde_json::from_value(value.clone()).unwrap(),
        "options" => h.options = serde_json::from_value(value.clone()).unwrap(),
        "handlers" => *h.handlers.write() = serde_json::from_value(value.clone()).unwrap(),
        "originalColors" => h.original_colors = serde_json::from_value(value.clone()).unwrap(),
        "childrenIds" => h.children_ids = serde_json::from_value(value.clone()).unwrap(),
        "modelIds" => h.model_ids = serde_json::from_value(value.clone()).unwrap(),
        "modelConfiguration" => {
            h.model_configuration = serde_json::from_value(value.clone()).unwrap()
        }
        "anIntArray3833" => h.an_int_array3833 = serde_json::from_value(value.clone()).unwrap(),
        "anInt3834" => h.an_int3834 = serde_json::from_value(value.clone()).unwrap(),
        "anInt3835" => h.an_int3835 = serde_json::from_value(value.clone()).unwrap(),
        "aByte3837" => h.a_byte3837 = serde_json::from_value(value.clone()).unwrap(),
        "anInt3838" => h.an_int3838 = serde_json::from_value(value.clone()).unwrap(),
        "aBoolean3839" => h.a_boolean3839 = serde_json::from_value(value.clone()).unwrap(),
        "anInt3840" => h.an_int3840 = serde_json::from_value(value.clone()).unwrap(),
        "anInt3841" => h.an_int3841 = serde_json::from_value(value.clone()).unwrap(),
        "anInt3844" => h.an_int3844 = serde_json::from_value(value.clone()).unwrap(),
        "aBoolean3845" => h.a_boolean3845 = serde_json::from_value(value.clone()).unwrap(),
        "aByte3847" => h.a_byte3847 = serde_json::from_value(value.clone()).unwrap(),
        "aByte3849" => h.a_byte3849 = serde_json::from_value(value.clone()).unwrap(),
        "anInt3850" => h.an_int3850 = serde_json::from_value(value.clone()).unwrap(),
        "anInt3851" => h.an_int3851 = serde_json::from_value(value.clone()).unwrap(),
        "secondBool" => h.second_bool = serde_json::from_value(value.clone()).unwrap(),
        "aBoolean3853" => h.a_boolean3853 = serde_json::from_value(value.clone()).unwrap(),
        "anInt3855" => h.an_int3855 = serde_json::from_value(value.clone()).unwrap(),
        "notClipped" => h.not_clipped = serde_json::from_value(value.clone()).unwrap(),
        "anInt3857" => h.an_int3857 = serde_json::from_value(value.clone()).unwrap(),
        "aByteArray3858" => h.a_byte_array3858 = serde_json::from_value(value.clone()).unwrap(),
        "anIntArray3859" => h.an_int_array3859 = serde_json::from_value(value.clone()).unwrap(),
        "anInt3860" => h.an_int3860 = serde_json::from_value(value.clone()).unwrap(),
        "configFileId" => h.config_file_id = serde_json::from_value(value.clone()).unwrap(),
        "modifiedColors" => h.modified_colors = serde_json::from_value(value.clone()).unwrap(),
        "anInt3865" => h.an_int3865 = serde_json::from_value(value.clone()).unwrap(),
        "aBoolean3866" => h.a_boolean3866 = serde_json::from_value(value.clone()).unwrap(),
        "aBoolean3867" => h.a_boolean3867 = serde_json::from_value(value.clone()).unwrap(),
        "projectileClipped" => {
            h.projectile_clipped = serde_json::from_value(value.clone()).unwrap()
        }
        "anIntArray3869" => h.an_int_array3869 = serde_json::from_value(value.clone()).unwrap(),
        "aBoolean3870" => h.a_boolean3870 = serde_json::from_value(value.clone()).unwrap(),
        "sizeY" => h.size_y = serde_json::from_value(value.clone()).unwrap(),
        "aBoolean3872" => h.a_boolean3872 = serde_json::from_value(value.clone()).unwrap(),
        "membersOnly" => h.members_only = serde_json::from_value(value.clone()).unwrap(),
        "boolean1" => h.boolean1 = serde_json::from_value(value.clone()).unwrap(),
        "anInt3875" => h.an_int3875 = serde_json::from_value(value.clone()).unwrap(),
        "animationId" => h.animation_id = serde_json::from_value(value.clone()).unwrap(),
        "anInt3877" => h.an_int3877 = serde_json::from_value(value.clone()).unwrap(),
        "anInt3878" => h.an_int3878 = serde_json::from_value(value.clone()).unwrap(),
        "clipType" => h.clip_type = serde_json::from_value(value.clone()).unwrap(),
        "anInt3881" => h.an_int3881 = serde_json::from_value(value.clone()).unwrap(),
        "anInt3882" => h.an_int3882 = serde_json::from_value(value.clone()).unwrap(),
        "anInt3883" => h.an_int3883 = serde_json::from_value(value.clone()).unwrap(),
        "loader" => h.loader = serde_json::from_value(value.clone()).unwrap(),
        "anInt3889" => h.an_int3889 = serde_json::from_value(value.clone()).unwrap(),
        "sizeX" => h.size_x = serde_json::from_value(value.clone()).unwrap(),
        "aBoolean3891" => h.a_boolean3891 = serde_json::from_value(value.clone()).unwrap(),
        "anInt3892" => h.an_int3892 = serde_json::from_value(value.clone()).unwrap(),
        "interactable" => h.interactable = serde_json::from_value(value.clone()).unwrap(),
        "aBoolean3894" => h.a_boolean3894 = serde_json::from_value(value.clone()).unwrap(),
        "aBoolean3895" => h.a_boolean3895 = serde_json::from_value(value.clone()).unwrap(),
        "anInt3896" => h.an_int3896 = serde_json::from_value(value.clone()).unwrap(),
        "configId" => h.config_id = serde_json::from_value(value.clone()).unwrap(),
        "aByteArray3899" => h.a_byte_array3899 = serde_json::from_value(value.clone()).unwrap(),
        "anInt3900" => h.an_int3900 = serde_json::from_value(value.clone()).unwrap(),
        "anInt3902" => h.an_int3902 = serde_json::from_value(value.clone()).unwrap(),
        "anInt3904" => h.an_int3904 = serde_json::from_value(value.clone()).unwrap(),
        "anInt3905" => h.an_int3905 = serde_json::from_value(value.clone()).unwrap(),
        "aBoolean3906" => h.a_boolean3906 = serde_json::from_value(value.clone()).unwrap(),
        "anIntArray3908" => h.an_int_array3908 = serde_json::from_value(value.clone()).unwrap(),
        "aByte3912" => h.a_byte3912 = serde_json::from_value(value.clone()).unwrap(),
        "anInt3913" => h.an_int3913 = serde_json::from_value(value.clone()).unwrap(),
        "aByte3914" => h.a_byte3914 = serde_json::from_value(value.clone()).unwrap(),
        "anInt3915" => h.an_int3915 = serde_json::from_value(value.clone()).unwrap(),
        "anIntArrayArray3916" => {
            h.an_int_array_array3916 = serde_json::from_value(value.clone()).unwrap()
        }
        "anInt3917" => h.an_int3917 = serde_json::from_value(value.clone()).unwrap(),
        "aShortArray3919" => h.a_short_array3919 = serde_json::from_value(value.clone()).unwrap(),
        "aShortArray3920" => h.a_short_array3920 = serde_json::from_value(value.clone()).unwrap(),
        "anInt3921" => h.an_int3921 = serde_json::from_value(value.clone()).unwrap(),
        "aClass194_3922" => h.a_class194_3922 = serde_json::from_value(value.clone()).unwrap(),
        "aBoolean3923" => h.a_boolean3923 = serde_json::from_value(value.clone()).unwrap(),
        "aBoolean3924" => h.a_boolean3924 = serde_json::from_value(value.clone()).unwrap(),
        "walkingFlag" => h.walking_flag = serde_json::from_value(value.clone()).unwrap(),
        "hasHiddenOptions" => h.has_hidden_options = serde_json::from_value(value.clone()).unwrap(),
        "mapIcon" => h.map_icon = serde_json::from_value(value.clone()).unwrap(),
        _ => panic!("unknown field {field}"),
    }
}
pub fn references(h: &Handle, ids: &mut BTreeMap<usize, usize>) -> Value {
    fn tag<T>(
        v: &rs09_server::definitions::item::Shared<T>,
        ids: &mut BTreeMap<usize, usize>,
    ) -> Value {
        let next = ids.len();
        json!(*ids.entry(v.identity()).or_insert(next))
    }
    let mut out = BTreeMap::new();
    out.insert("self", tag(h, ids));
    let h = h.read();
    out.insert(
        "aByteArray3858",
        h.a_byte_array3858
            .as_ref()
            .map_or(Value::Null, |v| tag(v, ids)),
    );
    out.insert(
        "aByteArray3899",
        h.a_byte_array3899
            .as_ref()
            .map_or(Value::Null, |v| tag(v, ids)),
    );
    out.insert(
        "aShortArray3919",
        h.a_short_array3919
            .as_ref()
            .map_or(Value::Null, |v| tag(v, ids)),
    );
    out.insert(
        "aShortArray3920",
        h.a_short_array3920
            .as_ref()
            .map_or(Value::Null, |v| tag(v, ids)),
    );
    out.insert(
        "anIntArray3833",
        h.an_int_array3833
            .as_ref()
            .map_or(Value::Null, |v| tag(v, ids)),
    );
    out.insert(
        "anIntArray3859",
        h.an_int_array3859
            .as_ref()
            .map_or(Value::Null, |v| tag(v, ids)),
    );
    out.insert(
        "anIntArray3869",
        h.an_int_array3869
            .as_ref()
            .map_or(Value::Null, |v| tag(v, ids)),
    );
    out.insert(
        "anIntArray3908",
        h.an_int_array3908
            .as_ref()
            .map_or(Value::Null, |v| tag(v, ids)),
    );
    out.insert(
        "anIntArrayArray3916",
        h.an_int_array_array3916
            .as_ref()
            .map_or(Value::Null, |v| tag(v, ids)),
    );
    out.insert(
        "childrenIds",
        h.children_ids.as_ref().map_or(Value::Null, |v| tag(v, ids)),
    );
    out.insert("handlers", tag(&h.handlers, ids));
    out.insert(
        "modelConfiguration",
        h.model_configuration
            .as_ref()
            .map_or(Value::Null, |v| tag(v, ids)),
    );
    out.insert(
        "modelIds",
        h.model_ids.as_ref().map_or(Value::Null, |v| tag(v, ids)),
    );
    out.insert(
        "modifiedColors",
        h.modified_colors
            .as_ref()
            .map_or(Value::Null, |v| tag(v, ids)),
    );
    out.insert(
        "options",
        h.options.as_ref().map_or(Value::Null, |v| tag(v, ids)),
    );
    out.insert(
        "originalColors",
        h.original_colors
            .as_ref()
            .map_or(Value::Null, |v| tag(v, ids)),
    );
    json!(out)
}
pub fn share(from: &Handle, to: &Handle, field: &str) {
    match field {
        "options" => {
            let a = from.read().options.clone();
            to.write().options = a;
        }
        "originalColors" => {
            let a = from.read().original_colors.clone();
            to.write().original_colors = a;
        }
        "childrenIds" => {
            let a = from.read().children_ids.clone();
            to.write().children_ids = a;
        }
        "modelIds" => {
            let a = from.read().model_ids.clone();
            to.write().model_ids = a;
        }
        "modelConfiguration" => {
            let a = from.read().model_configuration.clone();
            to.write().model_configuration = a;
        }
        "anIntArray3833" => {
            let a = from.read().an_int_array3833.clone();
            to.write().an_int_array3833 = a;
        }
        "aByteArray3858" => {
            let a = from.read().a_byte_array3858.clone();
            to.write().a_byte_array3858 = a;
        }
        "anIntArray3859" => {
            let a = from.read().an_int_array3859.clone();
            to.write().an_int_array3859 = a;
        }
        "modifiedColors" => {
            let a = from.read().modified_colors.clone();
            to.write().modified_colors = a;
        }
        "anIntArray3869" => {
            let a = from.read().an_int_array3869.clone();
            to.write().an_int_array3869 = a;
        }
        "aByteArray3899" => {
            let a = from.read().a_byte_array3899.clone();
            to.write().a_byte_array3899 = a;
        }
        "anIntArray3908" => {
            let a = from.read().an_int_array3908.clone();
            to.write().an_int_array3908 = a;
        }
        "anIntArrayArray3916" => {
            let a = from.read().an_int_array_array3916.clone();
            to.write().an_int_array_array3916 = a;
        }
        "aShortArray3919" => {
            let a = from.read().a_short_array3919.clone();
            to.write().a_short_array3919 = a;
        }
        "aShortArray3920" => {
            let a = from.read().a_short_array3920.clone();
            to.write().a_short_array3920 = a;
        }
        _ => panic!("unknown shared field {field}"),
    }
}
pub fn mutate(
    h: &Handle,
    field: &str,
    index: usize,
    value: &Value,
) -> Result<(), rs09_server::definitions::scenery::Error> {
    use rs09_server::definitions::scenery::Error;
    let h = h.read();
    match field {
        "options" => {
            let a = h.options.as_ref().ok_or(Error::Null)?;
            *a.write().get_mut(index).ok_or(Error::Index)? = value.as_str().map(str::to_owned);
        }
        "originalColors" => {
            let a = h.original_colors.as_ref().ok_or(Error::Null)?;
            *a.write().get_mut(index).ok_or(Error::Index)? = value.as_i64().unwrap() as i16;
        }
        "childrenIds" => {
            let a = h.children_ids.as_ref().ok_or(Error::Null)?;
            *a.write().get_mut(index).ok_or(Error::Index)? = value.as_i64().unwrap() as i32;
        }
        "modelIds" => {
            let a = h.model_ids.as_ref().ok_or(Error::Null)?;
            *a.write().get_mut(index).ok_or(Error::Index)? = value.as_i64().unwrap() as i32;
        }
        "modelConfiguration" => {
            let a = h.model_configuration.as_ref().ok_or(Error::Null)?;
            *a.write().get_mut(index).ok_or(Error::Index)? = value.as_i64().unwrap() as i32;
        }
        "anIntArray3833" => {
            let a = h.an_int_array3833.as_ref().ok_or(Error::Null)?;
            *a.write().get_mut(index).ok_or(Error::Index)? = value.as_i64().unwrap() as i32;
        }
        "aByteArray3858" => {
            let a = h.a_byte_array3858.as_ref().ok_or(Error::Null)?;
            *a.write().get_mut(index).ok_or(Error::Index)? = value.as_i64().unwrap() as i8;
        }
        "anIntArray3859" => {
            let a = h.an_int_array3859.as_ref().ok_or(Error::Null)?;
            *a.write().get_mut(index).ok_or(Error::Index)? = value.as_i64().unwrap() as i32;
        }
        "modifiedColors" => {
            let a = h.modified_colors.as_ref().ok_or(Error::Null)?;
            *a.write().get_mut(index).ok_or(Error::Index)? = value.as_i64().unwrap() as i16;
        }
        "anIntArray3869" => {
            let a = h.an_int_array3869.as_ref().ok_or(Error::Null)?;
            *a.write().get_mut(index).ok_or(Error::Index)? = value.as_i64().unwrap() as i32;
        }
        "aByteArray3899" => {
            let a = h.a_byte_array3899.as_ref().ok_or(Error::Null)?;
            *a.write().get_mut(index).ok_or(Error::Index)? = value.as_i64().unwrap() as i8;
        }
        "anIntArray3908" => {
            let a = h.an_int_array3908.as_ref().ok_or(Error::Null)?;
            *a.write().get_mut(index).ok_or(Error::Index)? = value.as_i64().unwrap() as i32;
        }
        "aShortArray3919" => {
            let a = h.a_short_array3919.as_ref().ok_or(Error::Null)?;
            *a.write().get_mut(index).ok_or(Error::Index)? = value.as_i64().unwrap() as i16;
        }
        "aShortArray3920" => {
            let a = h.a_short_array3920.as_ref().ok_or(Error::Null)?;
            *a.write().get_mut(index).ok_or(Error::Index)? = value.as_i64().unwrap() as i16;
        }
        _ => panic!("unknown mutable field {field}"),
    }
    Ok(())
}
