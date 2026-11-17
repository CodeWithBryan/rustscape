// Authored by tools/port_npc_schema.py; differential observation only.
use rs09_server::definitions::npc::Handle;
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
        "size" => h.size = serde_json::from_value(value.clone()).unwrap(),
        "combatLevel" => h.combat_level = serde_json::from_value(value.clone()).unwrap(),
        "headIcons" => h.head_icons = serde_json::from_value(value.clone()).unwrap(),
        "isVisibleOnMap" => h.is_visible_on_map = serde_json::from_value(value.clone()).unwrap(),
        "anInt833" => h.an_int833 = serde_json::from_value(value.clone()).unwrap(),
        "anInt836" => h.an_int836 = serde_json::from_value(value.clone()).unwrap(),
        "anInt837" => h.an_int837 = serde_json::from_value(value.clone()).unwrap(),
        "aBoolean841" => h.a_boolean841 = serde_json::from_value(value.clone()).unwrap(),
        "anInt842" => h.an_int842 = serde_json::from_value(value.clone()).unwrap(),
        "configFileId" => h.config_file_id = serde_json::from_value(value.clone()).unwrap(),
        "childNPCIds" => h.child_npc_ids = serde_json::from_value(value.clone()).unwrap(),
        "anInt846" => h.an_int846 = serde_json::from_value(value.clone()).unwrap(),
        "anInt850" => h.an_int850 = serde_json::from_value(value.clone()).unwrap(),
        "aByte851" => h.a_byte851 = serde_json::from_value(value.clone()).unwrap(),
        "aBoolean852" => h.a_boolean852 = serde_json::from_value(value.clone()).unwrap(),
        "anInt853" => h.an_int853 = serde_json::from_value(value.clone()).unwrap(),
        "aByte854" => h.a_byte854 = serde_json::from_value(value.clone()).unwrap(),
        "aBoolean856" => h.a_boolean856 = serde_json::from_value(value.clone()).unwrap(),
        "aBoolean857" => h.a_boolean857 = serde_json::from_value(value.clone()).unwrap(),
        "aShortArray859" => h.a_short_array859 = serde_json::from_value(value.clone()).unwrap(),
        "aByteArray861" => h.a_byte_array861 = serde_json::from_value(value.clone()).unwrap(),
        "aShort862" => h.a_short862 = serde_json::from_value(value.clone()).unwrap(),
        "aBoolean863" => h.a_boolean863 = serde_json::from_value(value.clone()).unwrap(),
        "anInt864" => h.an_int864 = serde_json::from_value(value.clone()).unwrap(),
        "aShortArray866" => h.a_short_array866 = serde_json::from_value(value.clone()).unwrap(),
        "anIntArray868" => h.an_int_array868 = serde_json::from_value(value.clone()).unwrap(),
        "anInt869" => h.an_int869 = serde_json::from_value(value.clone()).unwrap(),
        "anInt870" => h.an_int870 = serde_json::from_value(value.clone()).unwrap(),
        "anInt871" => h.an_int871 = serde_json::from_value(value.clone()).unwrap(),
        "anInt872" => h.an_int872 = serde_json::from_value(value.clone()).unwrap(),
        "anInt874" => h.an_int874 = serde_json::from_value(value.clone()).unwrap(),
        "anInt875" => h.an_int875 = serde_json::from_value(value.clone()).unwrap(),
        "anInt876" => h.an_int876 = serde_json::from_value(value.clone()).unwrap(),
        "anInt879" => h.an_int879 = serde_json::from_value(value.clone()).unwrap(),
        "aShortArray880" => h.a_short_array880 = serde_json::from_value(value.clone()).unwrap(),
        "anInt884" => h.an_int884 = serde_json::from_value(value.clone()).unwrap(),
        "configId" => h.config_id = serde_json::from_value(value.clone()).unwrap(),
        "anInt889" => h.an_int889 = serde_json::from_value(value.clone()).unwrap(),
        "anIntArray892" => h.an_int_array892 = serde_json::from_value(value.clone()).unwrap(),
        "aShort894" => h.a_short894 = serde_json::from_value(value.clone()).unwrap(),
        "aShortArray896" => h.a_short_array896 = serde_json::from_value(value.clone()).unwrap(),
        "anInt897" => h.an_int897 = serde_json::from_value(value.clone()).unwrap(),
        "anInt899" => h.an_int899 = serde_json::from_value(value.clone()).unwrap(),
        "anInt901" => h.an_int901 = serde_json::from_value(value.clone()).unwrap(),
        "standAnimation" => h.stand_animation = serde_json::from_value(value.clone()).unwrap(),
        "walkAnimation" => h.walk_animation = serde_json::from_value(value.clone()).unwrap(),
        "renderAnimationId" => {
            h.render_animation_id = serde_json::from_value(value.clone()).unwrap()
        }
        "combatDistance" => h.combat_distance = serde_json::from_value(value.clone()).unwrap(),
        "turnAnimation" => h.turn_animation = serde_json::from_value(value.clone()).unwrap(),
        "turn180Animation" => h.turn180_animation = serde_json::from_value(value.clone()).unwrap(),
        "turnCWAnimation" => h.turn_cw_animation = serde_json::from_value(value.clone()).unwrap(),
        "turnCCWAnimation" => h.turn_ccw_animation = serde_json::from_value(value.clone()).unwrap(),
        "combatGraphics" => h.combat_graphics = serde_json::from_value(value.clone()).unwrap(),
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
        "aByteArray861",
        h.a_byte_array861
            .as_ref()
            .map_or(Value::Null, |v| tag(v, ids)),
    );
    out.insert(
        "aShortArray859",
        h.a_short_array859
            .as_ref()
            .map_or(Value::Null, |v| tag(v, ids)),
    );
    out.insert(
        "aShortArray866",
        h.a_short_array866
            .as_ref()
            .map_or(Value::Null, |v| tag(v, ids)),
    );
    out.insert(
        "aShortArray880",
        h.a_short_array880
            .as_ref()
            .map_or(Value::Null, |v| tag(v, ids)),
    );
    out.insert(
        "aShortArray896",
        h.a_short_array896
            .as_ref()
            .map_or(Value::Null, |v| tag(v, ids)),
    );
    out.insert(
        "anIntArray868",
        h.an_int_array868
            .as_ref()
            .map_or(Value::Null, |v| tag(v, ids)),
    );
    out.insert(
        "anIntArray892",
        h.an_int_array892
            .as_ref()
            .map_or(Value::Null, |v| tag(v, ids)),
    );
    out.insert(
        "childNPCIds",
        h.child_npc_ids
            .as_ref()
            .map_or(Value::Null, |v| tag(v, ids)),
    );
    out.insert(
        "combatGraphics",
        h.combat_graphics
            .as_ref()
            .map_or(Value::Null, |v| tag(v, ids)),
    );
    out.insert("handlers", tag(&h.handlers, ids));
    out.insert(
        "options",
        h.options.as_ref().map_or(Value::Null, |v| tag(v, ids)),
    );
    json!(out)
}
pub fn share(from: &Handle, to: &Handle, field: &str) {
    match field {
        "options" => {
            let a = from.read().options.clone();
            to.write().options = a;
        }
        "childNPCIds" => {
            let a = from.read().child_npc_ids.clone();
            to.write().child_npc_ids = a;
        }
        "aShortArray859" => {
            let a = from.read().a_short_array859.clone();
            to.write().a_short_array859 = a;
        }
        "aByteArray861" => {
            let a = from.read().a_byte_array861.clone();
            to.write().a_byte_array861 = a;
        }
        "aShortArray866" => {
            let a = from.read().a_short_array866.clone();
            to.write().a_short_array866 = a;
        }
        "anIntArray868" => {
            let a = from.read().an_int_array868.clone();
            to.write().an_int_array868 = a;
        }
        "aShortArray880" => {
            let a = from.read().a_short_array880.clone();
            to.write().a_short_array880 = a;
        }
        "anIntArray892" => {
            let a = from.read().an_int_array892.clone();
            to.write().an_int_array892 = a;
        }
        "aShortArray896" => {
            let a = from.read().a_short_array896.clone();
            to.write().a_short_array896 = a;
        }
        "combatGraphics" => {
            let a = from.read().combat_graphics.clone();
            to.write().combat_graphics = a;
        }
        _ => panic!("unknown shared field {field}"),
    }
}
pub fn mutate(
    h: &Handle,
    field: &str,
    index: usize,
    value: &Value,
) -> Result<(), rs09_server::definitions::npc::Error> {
    use rs09_server::definitions::npc::Error;
    let h = h.read();
    match field {
        "options" => {
            let a = h.options.as_ref().ok_or(Error::Null)?;
            *a.write().get_mut(index).ok_or(Error::Index)? = value.as_str().map(str::to_owned);
        }
        "childNPCIds" => {
            let a = h.child_npc_ids.as_ref().ok_or(Error::Null)?;
            *a.write().get_mut(index).ok_or(Error::Index)? = value.as_i64().unwrap() as i32;
        }
        "aShortArray859" => {
            let a = h.a_short_array859.as_ref().ok_or(Error::Null)?;
            *a.write().get_mut(index).ok_or(Error::Index)? = value.as_i64().unwrap() as i16;
        }
        "aByteArray861" => {
            let a = h.a_byte_array861.as_ref().ok_or(Error::Null)?;
            *a.write().get_mut(index).ok_or(Error::Index)? = value.as_i64().unwrap() as i8;
        }
        "aShortArray866" => {
            let a = h.a_short_array866.as_ref().ok_or(Error::Null)?;
            *a.write().get_mut(index).ok_or(Error::Index)? = value.as_i64().unwrap() as i16;
        }
        "anIntArray868" => {
            let a = h.an_int_array868.as_ref().ok_or(Error::Null)?;
            *a.write().get_mut(index).ok_or(Error::Index)? = value.as_i64().unwrap() as i32;
        }
        "aShortArray880" => {
            let a = h.a_short_array880.as_ref().ok_or(Error::Null)?;
            *a.write().get_mut(index).ok_or(Error::Index)? = value.as_i64().unwrap() as i16;
        }
        "anIntArray892" => {
            let a = h.an_int_array892.as_ref().ok_or(Error::Null)?;
            *a.write().get_mut(index).ok_or(Error::Index)? = value.as_i64().unwrap() as i32;
        }
        "aShortArray896" => {
            let a = h.a_short_array896.as_ref().ok_or(Error::Null)?;
            *a.write().get_mut(index).ok_or(Error::Index)? = value.as_i64().unwrap() as i16;
        }
        _ => panic!("unknown mutable field {field}"),
    }
    Ok(())
}
