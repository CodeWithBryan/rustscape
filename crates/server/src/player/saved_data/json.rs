use super::{Error, ItemData, Result};
use serde_json::{Map, Value};
pub fn required<'a>(data: &'a Map<String, Value>, key: &str) -> Result<&'a Value> {
    data.get(key)
        .filter(|value| !value.is_null())
        .ok_or(Error::Null)
}
pub fn object(value: &Value) -> Result<&Map<String, Value>> {
    if value.is_null() {
        Err(Error::Null)
    } else {
        value.as_object().ok_or(Error::Type)
    }
}
pub fn section(value: Option<&Value>) -> Result<&Value> {
    match value {
        None | Some(Value::Null) => Err(Error::Null),
        Some(value) if value.is_object() => Ok(value),
        _ => Err(Error::Type),
    }
}
fn text(value: &Value) -> Result<String> {
    match value {
        Value::Null => Err(Error::Null),
        Value::String(value) => Ok(value.clone()),
        Value::Number(value) if value.is_f64() => {
            Ok(rs09_game_core::java_double::format(value.as_f64().unwrap()))
        }
        _ => Ok(value.to_string()),
    }
}
pub fn int(value: &Value) -> Result<i32> {
    rs09_game_core::java_string::parse_i32(&text(value)?).ok_or(Error::Number)
}
pub fn long(value: &Value) -> Result<i64> {
    rs09_game_core::java_string::parse_i64(&text(value)?).ok_or(Error::Number)
}
pub fn byte(value: &Value) -> Result<i8> {
    i8::try_from(int(value)?).map_err(|_| Error::Number)
}
pub fn double(value: &Value) -> Result<f64> {
    rs09_game_core::java_double::parse(&text(value)?).ok_or(Error::Number)
}
pub fn boolean(value: &Value) -> Result<bool> {
    if value.is_null() {
        Err(Error::Null)
    } else {
        value.as_bool().ok_or(Error::Type)
    }
}
pub fn array<T>(
    data: &Map<String, Value>,
    key: &str,
    output: &mut [T],
    parse: fn(&Value) -> Result<T>,
) -> Result<()> {
    let values = required(data, key)?.as_array().ok_or(Error::Type)?;
    for (index, value) in values.iter().enumerate() {
        let value = parse(value)?;
        *output.get_mut(index).ok_or(Error::Index)? = value;
    }
    Ok(())
}
pub fn items(data: &Map<String, Value>, key: &str, output: &mut [Option<ItemData>]) -> Result<()> {
    let values = required(data, key)?.as_array().ok_or(Error::Type)?;
    for (index, value) in values.iter().enumerate() {
        let item = object(value)?;
        let item = ItemData::new(int(required(item, "id")?)?, int(required(item, "amount")?)?);
        *output.get_mut(index).ok_or(Error::Index)? = Some(item);
    }
    Ok(())
}
