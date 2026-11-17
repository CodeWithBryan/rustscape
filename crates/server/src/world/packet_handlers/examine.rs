//! The `ItemExamine`, `SceneryExamine`, `NpcExamine` and `TrackFinished`
//! branches of `PacketProcessor.process`, and the examine texts they send:
//! `Definition.getExamine`, `ItemDefinition.getExamine`,
//! `NPCDefinition.getExamine` and `StringUtils.isPlusN`, over the native
//! definitions.
use super::{Base, Error, Result};
use crate::definitions::{item, npc, property::Property, scenery};
use crate::world::player_tabs::{self, music::MusicPlayer};

/// `ItemDefinition.getExamine`'s text for a noted item.
pub const NOTE_EXAMINE: &str = "Swap this note at any bank for the equivalent item.";

pub trait Context: Base {
    /// `ItemDefinition.forId(id)`.
    fn item_definition(&mut self, id: i32) -> Result<Option<item::Handle>>;
    /// `SceneryDefinition.forId(id)`.
    fn scenery_definition(&mut self, id: i32) -> Result<Option<scenery::Handle>>;
    /// `NPCDefinition.forId(id)`.
    fn npc_definition(&mut self, id: i32) -> Result<Option<npc::Handle>>;
    /// `VarbitDefinition.forObjectID(id)`: the definition's own id.
    fn varbit(&mut self, id: i32) -> Result<Option<i32>>;
    /// `player.musicPlayer.isLooping()`.
    fn music_looping(&mut self, player: &Self::Node) -> Result<bool>;
    /// `player.musicPlayer.replay()`.
    fn music_replay(&mut self, player: &Self::Node) -> Result<()>;
    /// `player.musicPlayer.setPlaying(playing)`.
    fn music_set_playing(&mut self, player: &Self::Node, playing: bool) -> Result<()>;
}

/// `StringUtils.isPlusN(word)` (`toLowerCase()` in the default locale).
pub fn is_plus_n(word: Option<&str>, lowercase: impl Fn(&str) -> String) -> Result<bool> {
    let Some(word) = word else {
        return Ok(false);
    };
    let s: Vec<u16> = lowercase(word).encode_utf16().collect();
    let at = |i: usize| -> Result<u16> {
        s.get(i)
            .copied()
            .ok_or_else(|| Error::Index(Some(format!("String index out of range: {i}"))))
    };
    let c = at(0)?;
    Ok(b"aeiou".iter().any(|v| c == u16::from(*v))
        || (c == u16::from(b'h') && s.len() > 1 && at(1)? != u16::from(b'e')))
}

/// `Object.toString()` of a handler value.
fn property_text(value: &Property) -> Result<String> {
    match value {
        Property::Text(text) => Ok(text.clone()),
        Property::Int(value) => Ok(value.to_string()),
        Property::Long(value) => Ok(value.to_string()),
        Property::Bool(value) => Ok(value.to_string()),
        other => Err(Error::Binding(Some(format!(
            "examine handler {other:?} has no native toString"
        )))),
    }
}

/// `Definition.getExamine()` over a definition's fields: the cached text,
/// else the `examine` handler's `toString()`, else "It's a(n) NAME." or
/// "null" for an empty name; the result is cached in `examine`.
pub fn definition_examine(
    examine: &mut Option<String>,
    handler: Option<&Property>,
    name: Option<&str>,
    lowercase: impl Fn(&str) -> String,
) -> Result<String> {
    if examine.is_none() {
        // `handlers.get("examine") != null`; the try only covers toString.
        if let Some(value) = handler.filter(|v| !v.is_null()) {
            *examine = Some(property_text(value)?);
        }
        if examine.is_none() {
            let name = name.ok_or(Error::Null)?;
            *examine = Some(if !name.is_empty() {
                let n = if is_plus_n(Some(name), &lowercase)? {
                    "n "
                } else {
                    " "
                };
                format!("It's a{n}{name}.")
            } else {
                "null".into()
            });
        }
    }
    Ok(examine.clone().unwrap_or_default())
}

/// `ItemDefinition.getExamine()`: the field is overwritten with the note
/// text for a noted item.
pub fn item_examine_text(
    definition: &item::Handle,
    lowercase: impl Fn(&str) -> String,
) -> Result<String> {
    let mut d = definition.write();
    let d = &mut *d;
    let text = definition_examine(
        &mut d.examine,
        d.handlers.get("examine"),
        d.name.as_deref(),
        lowercase,
    )?;
    d.examine = Some(text);
    if !d.unnoted {
        d.examine = Some(NOTE_EXAMINE.into());
    }
    Ok(d.examine.clone().unwrap_or_default())
}

/// `SceneryDefinition`'s inherited `getExamine()`.
pub fn scenery_examine_text(
    definition: &scenery::Handle,
    lowercase: impl Fn(&str) -> String,
) -> Result<String> {
    let handler = definition.read().handlers.read().get("examine").cloned();
    let mut d = definition.write();
    let d = &mut *d;
    definition_examine(
        &mut d.examine,
        handler.as_ref(),
        d.name.as_deref(),
        lowercase,
    )
}

/// `NPCDefinition.getExamine()`: not cached; a name of at most one UTF-16
/// unit gives null.
pub fn npc_examine_text(
    definition: &npc::Handle,
    lowercase: impl Fn(&str) -> String,
) -> Result<Option<String>> {
    let d = definition.read();
    let configured = match d.handlers.read().get("examine") {
        None | Some(Property::Null) => d.examine.clone(),
        Some(Property::Text(text)) => Some(text.clone()),
        Some(_) => return Err(Error::Cast),
    };
    if configured.is_some() {
        return Ok(configured);
    }
    let name = d.name.as_deref().ok_or(Error::Null)?;
    if name.encode_utf16().count() <= 1 {
        return Ok(None);
    }
    let n = if is_plus_n(Some(name), lowercase)? {
        "n "
    } else {
        " "
    };
    Ok(Some(format!("It's a{n}{name}.")))
}

/// The `ItemExamine` branch.
pub fn item_examine<C: Context>(ctx: &mut C, player: &C::Node, id: i32) -> Result<()> {
    let Some(definition) = ctx.item_definition(id)? else {
        return Ok(());
    };
    let (value, model) = {
        let d = definition.read();
        (d.value, d.interface_model_id)
    };
    ctx.debug(
        player,
        &format!("[ITEM] ID: {id} Value: {value} Model: {model}"),
    )?;
    let text = item_examine_text(&definition, |s| ctx.lowercase_default(s))?;
    ctx.message(player, Some(&text))
}

/// The `SceneryExamine` branch: `def.configFile` is looked up twice.
pub fn scenery_examine<C: Context>(ctx: &mut C, player: &C::Node, id: i32) -> Result<()> {
    let Some(definition) = ctx.scenery_definition(id)? else {
        return Ok(());
    };
    ctx.debug(player, "[SCENERY]---------------------")?;
    ctx.debug(player, &format!("ID: {id}"))?;
    let file = definition.read().config_file_id;
    let found = if file != -1 { ctx.varbit(file)? } else { None };
    if found.is_some() {
        let file = definition.read().config_file_id;
        let again = if file != -1 { ctx.varbit(file)? } else { None };
        let varbit = again.ok_or(Error::Null)?;
        ctx.debug(player, &format!("Varbit: {varbit}"))?;
    }
    ctx.debug(player, "------------------------------")?;
    let text = scenery_examine_text(&definition, |s| ctx.lowercase_default(s))?;
    ctx.message(player, Some(&text))
}

/// The `NpcExamine` branch (no null check on the definition).
pub fn npc_examine<C: Context>(ctx: &mut C, player: &C::Node, id: i32) -> Result<()> {
    let definition = ctx.npc_definition(id)?;
    ctx.debug(player, "[NPC]-------------------------")?;
    ctx.debug(player, &format!("ID: {id}"))?;
    let definition = definition.ok_or(Error::Null)?;
    let file = definition.read().config_file_id;
    if file != -1 {
        ctx.debug(player, &format!("Varbit: {file}"))?;
    }
    ctx.debug(player, "------------------------------")?;
    let text = npc_examine_text(&definition, |s| ctx.lowercase_default(s))?;
    ctx.message(player, text.as_deref())
}

/// The `TrackFinished` branch.
pub fn track_finished<C: Context>(ctx: &mut C, player: &C::Node) -> Result<()> {
    if ctx.music_looping(player)? {
        ctx.music_replay(player)
    } else {
        ctx.music_set_playing(player, false)
    }
}

/// The `TrackFinished` branch on the native music player: the binding of
/// [`Context::music_looping`], [`Context::music_replay`] and
/// [`Context::music_set_playing`].
pub fn track_finished_native<H: player_tabs::music::Host>(
    music: &mut MusicPlayer<'_>,
    host: &mut H,
) -> player_tabs::Result<()> {
    if music.playback.looping {
        music.replay(host)
    } else {
        music.playback.playing = false;
        Ok(())
    }
}
