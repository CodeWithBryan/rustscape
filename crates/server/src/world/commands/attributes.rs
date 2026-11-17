//! `PlayerAttributeCommandSet`: getattribute and setattribute, on the
//! player or a named one.
use super::kotlin::{to_float_or_null, to_int_or_null};
use super::{Host, Result, Value, arg, notify, reject};

/// `::getattribute [player] <attribute>`.
pub fn get<H: Host + ?Sized>(host: &mut H, player: &H::Player, args: &[String]) -> Result<()> {
    if args.len() < 2 {
        reject::<_, ()>(
            host,
            player,
            &["Usage ::getattribute [playername] attributename"],
        )?;
    }
    let mut name = String::new();
    let mut query = None;
    if args.len() == 2 {
        query = Some(player.clone());
        name = arg(args, 1)?.to_owned();
    }
    if args.len() == 3 {
        query = host.player_by_name(Some(arg(args, 1)?))?;
        name = arg(args, 2)?.to_owned();
    }
    let Some(query) = query else {
        return reject(host, player, &["Invalid player or player not online."]);
    };
    let value = host.api_attribute(&query, &name, Value::Text(String::new()))?;
    let text = value.map_or("null".into(), |v| v.java_string());
    notify(host, player, &format!("Attribute: {name} Value: {text}"))
}

/// `::setattribute [player] <attribute> <value>`: an integer, a float,
/// `true` or `false`; anything else sets nothing.
pub fn set<H: Host + ?Sized>(host: &mut H, player: &H::Player, args: &[String]) -> Result<()> {
    if args.len() < 2 {
        reject::<_, ()>(
            host,
            player,
            &["Usage ::setattribute [playername] attributename value"],
        )?;
    }
    let mut name = String::new();
    let mut value = String::new();
    let mut query = None;
    if args.len() == 3 {
        query = Some(player.clone());
        name = arg(args, 1)?.to_owned();
        value = arg(args, 2)?.to_owned();
    }
    if args.len() == 4 {
        query = host.player_by_name(Some(arg(args, 1)?))?;
        name = arg(args, 2)?.to_owned();
        value = arg(args, 3)?.to_owned();
    }
    let Some(query) = query else {
        return reject(host, player, &["Invalid player or player not online."]);
    };
    if let Some(int) = to_int_or_null(&value) {
        return host.set_attribute(&query, &name, Some(Value::Int(int)));
    }
    if let Some(float) = to_float_or_null(&value) {
        return host.set_attribute(&query, &name, Some(Value::Float(float)));
    }
    if value == "true" {
        return host.set_attribute(&query, &name, Some(Value::Bool(true)));
    }
    if value == "false" {
        return host.set_attribute(&query, &name, Some(Value::Bool(false)));
    }
    Ok(())
}
