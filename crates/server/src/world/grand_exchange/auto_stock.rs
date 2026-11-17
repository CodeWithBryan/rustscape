//! GEAutoStock.kt: at start-up, when `GE_AUTOSTOCK_ENABLED`, every entry of
//! `data/eco/autostock.json`'s "offers" becomes a bot offer.
use super::{Error, Host, Result, exchange};
use rs09_game_core::java_json::{self, Json};

/// `"data" + File.separator + "eco" + File.separator + "autostock.json"`
/// with the separator of the platform the server runs on.
pub fn path() -> String {
    let sep = std::path::MAIN_SEPARATOR;
    format!("data{sep}eco{sep}autostock.json")
}

/// `autostock()`.
pub fn autostock<H: Host + ?Sized>(h: &mut H) -> Result<()> {
    if !h.autostock_enabled()? {
        return Ok(());
    }
    let text = h.read_file(&path())?;
    let save = match java_json::parse(&text) {
        Ok(Json::Object(entries)) => Json::Object(entries),
        Ok(Json::Null) => return Err(Error::Null),
        Ok(_) => return Err(Error::ClassCast),
        Err(_) => return Err(Error::Parse),
    };
    let Some(offers) = save.get("offers") else {
        return Ok(());
    };
    let offers = match offers {
        Json::Array(items) => items.clone(),
        Json::Null => return Err(Error::Null),
        _ => return Err(Error::ClassCast),
    };
    for entry in offers {
        let o = match &entry {
            Json::Object(_) => entry.clone(),
            Json::Null => return Err(Error::Null),
            _ => return Err(Error::ClassCast),
        };
        let int = |key: &str| -> Result<i32> {
            let text = o.get(key).map_or_else(|| "null".to_string(), Json::text);
            rs09_game_core::java_string::parse_i32(&text).ok_or(Error::Number)
        };
        let item = int("item")?;
        let qty = int("qty")?;
        exchange::add_bot_offer(h, item, qty)?;
    }
    Ok(())
}
