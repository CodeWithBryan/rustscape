//! `ItemDefinition.hasRequirement(player, wield, message)`: the skill levels
//! an item needs to be worn or used. Replayed against
//! `fixtures/wield-requirements` (the unchanged original method,
//! `cargo xtask wield-requirements-oracle`).
//!
//! The requirement map is walked in JDK `HashMap` key order (the item
//! configuration's insertion order, `Requirements::order`); keys outside
//! `Skills.SKILL_NAME` (24 names) are skipped; the first skill whose static
//! level is below the requirement fails the check, with its message when
//! asked. A configuration of another class fails as the original's cast.
use super::{Error, Result};
use crate::definitions::property::Property;
use crate::world::player_tabs::level_up::SKILL_NAME;

/// `hasRequirement`. `static_level(skill)` is `getStaticLevel(skill)`;
/// `send(text)` is `PacketDispatch.sendMessage(text)`.
pub fn has_requirement(
    configuration: Option<&Property>,
    mut static_level: impl FnMut(usize) -> Result<i32>,
    wield: bool,
    message: bool,
    mut send: impl FnMut(&str) -> Result<()>,
    lowercase: impl Fn(&str) -> String,
) -> Result<bool> {
    let requirements = match configuration {
        None | Some(Property::Null) => return Ok(true),
        Some(Property::Requirements(r)) => r.clone(),
        Some(_) => return Err(Error::Cast),
    };
    let order = requirements.read().order();
    for skill in order {
        if skill < 0 || skill >= SKILL_NAME.len() as i32 {
            continue;
        }
        let level = requirements.read().get(skill).ok_or(Error::Null)?;
        if static_level(skill as usize)? < level {
            if message {
                let name = SKILL_NAME[skill as usize];
                let n = super::examine::is_plus_n(Some(name), &lowercase)?;
                let text = format!(
                    "You need a{}{name} level of {level} to {}this.",
                    if n { "n " } else { " " },
                    if wield { "wear " } else { "use " }
                );
                send(&text)?;
            }
            return Ok(false);
        }
    }
    Ok(true)
}
