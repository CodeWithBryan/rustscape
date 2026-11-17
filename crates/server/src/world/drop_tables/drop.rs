//! From a rolled list to items on the ground: `NPCDropTables.drop`,
//! `createDrop`, `getLooter`, `sendDropMessage`, `handleBoneCrusher` and
//! `core.api.announceIfRare`.
//!
//! The ground item is made by the native [`ground_manager`](gm) and the item
//! and its definition are the native ones. Everything else the original
//! reaches (the NPC, its behaviour, item plugins, clans and loot share, the
//! Grand Exchange price, skills, messages, news, statistics, bots) is a
//! required service of [`Host`]: one method per original call, in the original
//! order, with no default answers.
use super::{
    NpcDropTable, NpcDropTables, RollError, RolledItem,
    random::{self, Source},
};
use crate::{
    definitions::property::Property,
    inventory::item,
    world::{
        doors::Location, ground_item as ground, ground_manager as gm, player_walk::NodeHandle,
    },
};
use rs09_game_core::java_lower::lowercase_root;

/// `Skills.PRAYER`.
pub const PRAYER: i32 = 5;
/// The item whose drop tells the player that a mystery box fell.
pub const MYSTERY_BOX: i32 = 6199;
pub const MYSTERY_BOX_MESSAGE: &str = "<col=990000>A mystery box has fallen on the ground.</col>";
/// `ItemConfigParser.RARE_ITEM`.
pub const RARE_ITEM: &str = "rare_item";
/// Native bound on one unstackable drop split into single items. The original
/// has none: an amount of two billion makes two billion ground items.
pub const MAX_SPLIT: i32 = 1 << 16;

/// `content.global.skill.prayer.Bones`: item id and burying experience, in
/// declaration order.
pub const BONES: [(i32, f64); 31] = [
    (526, 4.5),
    (2530, 4.5),
    (3187, 4.5),
    (2859, 4.5),
    (528, 4.5),
    (3179, 16.0),
    (3180, 18.0),
    (3181, 18.0),
    (3182, 18.0),
    (3183, 5.0),
    (3185, 5.0),
    (3186, 5.0),
    (530, 5.3),
    (532, 15.0),
    (3125, 15.0),
    (4812, 22.5),
    (3123, 25.0),
    (534, 30.0),
    (6812, 50.0),
    (536, 72.0),
    (4830, 84.0),
    (4832, 96.0),
    (6729, 125.0),
    (4834, 140.0),
    (3127, 16.0),
    (3128, 17.0),
    (3129, 17.0),
    (3130, 18.0),
    (3131, 17.0),
    (3132, 17.0),
    (3133, 18.0),
];

/// `Bones.forId(itemId)?.experience`. The original fills a map in declaration
/// order, so a later constant would replace an earlier one with the same id.
pub fn bone_experience(item_id: i32) -> Option<f64> {
    BONES
        .iter()
        .rev()
        .find(|(id, _)| *id == item_id)
        .map(|(_, experience)| *experience)
}

#[derive(Debug)]
pub enum Error {
    /// `NullPointerException`.
    Null,
    /// `ClassCastException`: a configuration value of another type.
    Type,
    /// `ArrayIndexOutOfBoundsException`.
    Index,
    Roll(RollError),
    Ground(gm::Error),
    /// Native bound, see [`MAX_SPLIT`].
    TooManyItems,
    /// A required service failed.
    Binding(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
impl From<RollError> for Error {
    fn from(error: RollError) -> Self {
        Self::Roll(error)
    }
}
impl From<gm::Error> for Error {
    fn from(error: gm::Error) -> Self {
        Self::Ground(error)
    }
}
impl From<ground::Error> for Error {
    fn from(error: ground::Error) -> Self {
        Self::Ground(gm::Error::Ground(error))
    }
}
pub type Result<T> = std::result::Result<T, Error>;

/// The services `drop` and `createDrop` need from systems that are not ported,
/// on top of the ground manager's own. Players and NPCs are canonical nodes.
///
/// Each method stands for the original call named in its comment. None has a
/// default: a binding must not answer for a system it does not have.
pub trait Host: gm::Context {
    /// `clanRepository` identity, compared with `==`.
    type Clan: PartialEq;

    /// `entity instanceof Player`.
    fn is_player(&mut self, entity: &NodeHandle) -> bool;
    /// `player instanceof AIPlayer`.
    fn is_ai_player(&mut self, entity: &NodeHandle) -> bool;
    /// `RandomFunction.RANDOM`.
    fn random(&mut self) -> &mut dyn Source;
    /// `table.roll(looter, 1)`. The binding owns the three generators, the
    /// shared tables, the clue registry and the looter's equipment answers,
    /// and calls [`NpcDropTable::roll`] with them.
    fn roll(
        &mut self,
        table: &NpcDropTable,
        looter: Option<&NodeHandle>,
    ) -> std::result::Result<Vec<RolledItem>, RollError>;
    /// `GroundItemManager`'s list.
    fn manager(&mut self) -> gm::Manager;

    /// `npc.behavior.onDropTableRolled(npc, looter, drops)`.
    fn on_drop_table_rolled(
        &mut self,
        npc: &NodeHandle,
        looter: &NodeHandle,
        drops: &mut Vec<RolledItem>,
    ) -> Result<()>;
    /// `npc.getId()`.
    fn npc_id(&mut self, npc: &NodeHandle) -> Result<i32>;
    /// `npc.getDropLocation()`, which subclasses override.
    fn npc_drop_location(&mut self, npc: &NodeHandle) -> Result<Option<Location>>;
    /// `npc.getLocation()`.
    fn npc_location(&mut self, npc: &NodeHandle) -> Result<Option<Location>>;
    /// `npc.getProperties().isMultiZone()`.
    fn npc_multi_zone(&mut self, npc: &NodeHandle) -> Result<bool>;

    /// `plugin.createDrop(item, player, npc, location)`.
    fn plugin_create_drop(
        &mut self,
        plugin: Self::Plugin,
        item: &item::Handle,
        player: &NodeHandle,
        npc: &NodeHandle,
        location: &Location,
    ) -> Result<bool>;
    /// `plugin.getItem(item, npc)`.
    fn plugin_get_item(
        &mut self,
        plugin: Self::Plugin,
        item: &item::Handle,
        npc: &NodeHandle,
    ) -> Result<Option<item::Handle>>;

    /// `player.sendMessage(message)`.
    fn send_message(&mut self, player: &NodeHandle, message: &str) -> Result<()>;
    /// `player.getInterfaceManager().isResizable()`.
    fn resizable(&mut self, player: &NodeHandle) -> Result<bool>;
    /// `player.getGlobalData().isEnableBoneCrusher()`.
    fn bone_crusher_enabled(&mut self, player: &NodeHandle) -> Result<bool>;
    /// `player.getSkills().addExperience(skill, experience)`.
    fn add_experience(&mut self, player: &NodeHandle, skill: i32, experience: f64) -> Result<()>;
    /// `player.getCommunication().getClan()`.
    fn clan(&mut self, player: &NodeHandle) -> Result<Option<Self::Clan>>;
    /// `clan.getLootRequirement().ordinal()`.
    fn clan_loot_requirement(&mut self, clan: &Self::Clan) -> Result<i32>;
    /// `player.getCommunication().isLootShare()`.
    fn loot_share(&mut self, player: &NodeHandle) -> Result<bool>;
    /// `player.getCommunication().getLootRequirement().ordinal()`.
    fn loot_requirement(&mut self, player: &NodeHandle) -> Result<i32>;
    /// `player.getIronmanManager().isIronman()`.
    fn ironman(&mut self, player: &NodeHandle) -> Result<bool>;
    /// `player.getGlobalData().getLootSharePoints()`.
    fn loot_share_points(&mut self, player: &NodeHandle) -> Result<i32>;
    /// `player.getGlobalData().setLootSharePoints(points)`.
    fn set_loot_share_points(&mut self, player: &NodeHandle, points: i32) -> Result<()>;
    /// `RegionManager.getLocalPlayers(location, distance)`, in its order.
    fn local_players(
        &mut self,
        location: Option<&Location>,
        distance: i32,
    ) -> Result<Vec<Option<NodeHandle>>>;
    /// `GrandExchange.getRecommendedPrice(itemId, from)`.
    fn recommended_price(&mut self, item_id: i32, from: bool) -> Result<i32>;
    /// `Repository.getPlayers()`, in iteration order.
    fn repository_players(&mut self) -> Result<Vec<NodeHandle>>;
    /// `core.api.sendNews(message)`.
    fn send_news(&mut self, message: &str) -> Result<()>;
    /// `PlayerStatsCounter.incrementRareDrop(player, item)`.
    fn increment_rare_drop(&mut self, player: &NodeHandle, item: &item::Handle) -> Result<()>;
    /// `AIRepository.addItem(groundItem)`.
    fn ai_add_item(&mut self, item: &ground::Handle) -> Result<()>;
    /// `player.getAttribute("botting:script", null)` is a pulse that is running.
    fn bot_script_running(&mut self, player: &NodeHandle) -> Result<bool>;
    /// The three statements that append to `botting:drops`.
    fn bot_add_drop(&mut self, player: &NodeHandle, item: &ground::Handle) -> Result<()>;
}

fn name(item: &item::Handle) -> Result<String> {
    item.read().name.clone().ok_or(Error::Null)
}

/// `item.getPlugin()`.
fn plugin<H: Host>(host: &mut H, item: &item::Handle) -> Result<Option<H::Plugin>> {
    let definition = item.read().definition.clone();
    Ok(match definition {
        Some(definition) => host.plugin(&definition)?,
        None => None,
    })
}

/// `NPCDropTables.drop(npc, looter)`: one roll of the table, the behaviour
/// callback, then `createDrop` for every item left in the list.
///
/// A looter that is not a player still rolls, and spends the draws, but
/// nothing is dropped.
pub fn drop<H: Host>(
    host: &mut H,
    tables: &NpcDropTables,
    npc: &NodeHandle,
    looter: Option<&NodeHandle>,
) -> Result<()> {
    let player = match looter {
        Some(looter) if host.is_player(looter) => Some(looter),
        _ => None,
    };
    let mut drops = host.roll(&tables.table, looter)?;
    // The behaviour's parameter is non-null: a missing looter fails here,
    // after the roll.
    let killer = looter.ok_or(RollError::NullLooter)?;
    host.on_drop_table_rolled(npc, killer, &mut drops)?;
    for rolled in drops {
        let item = host.copy_item(rolled.id, rolled.amount, 1000)?;
        let location = host.npc_drop_location(npc)?;
        create_drop(host, Some(item), player, npc, location)?;
    }
    Ok(())
}

/// `createDrop(item, player, npc, l)`.
pub fn create_drop<H: Host>(
    host: &mut H,
    item: Option<item::Handle>,
    player: Option<&NodeHandle>,
    npc: &NodeHandle,
    location: Option<Location>,
) -> Result<()> {
    let Some(mut item) = item else {
        return Ok(());
    };
    if item.read().id() == 0 {
        return Ok(());
    }
    let Some(location) = location else {
        return Ok(());
    };
    if name(&item)? == "null" {
        return Ok(());
    }
    let Some(player) = player else {
        return Ok(());
    };
    if handle_bone_crusher(host, player, &item)? {
        return Ok(());
    }
    if plugin(host, &item)?.is_some() {
        let found = plugin(host, &item)?.ok_or(Error::Null)?;
        if !host.plugin_create_drop(found, &item, player, npc, &location)? {
            return Ok(());
        }
        let found = plugin(host, &item)?.ok_or(Error::Null)?;
        item = host
            .plugin_get_item(found, &item, npc)?
            .ok_or(Error::Null)?;
    }
    let definition = item.read().definition.clone().ok_or(Error::Null)?;
    let stackable = definition.read().is_stackable();
    let manager = host.manager();
    if !stackable && item.read().amount() > 1 {
        // Each single item goes to the killer, not to the loot-share winner,
        // and is neither announced nor reported.
        if item.read().amount() > MAX_SPLIT {
            return Err(Error::TooManyItems);
        }
        let mut index = 0;
        while index < item.read().amount() {
            let id = item.read().id();
            let single = host.copy_item(id, 1, 1000)?;
            gm::create_at(
                host,
                &manager,
                Some(single),
                Some(location.clone()),
                Some(player.clone()),
            )?;
            index += 1;
        }
        return Ok(());
    }
    announce_if_rare(host, player, &item)?;
    if item.read().id() == MYSTERY_BOX {
        host.send_message(player, MYSTERY_BOX_MESSAGE)?;
    }
    let npc_id = host.npc_id(npc)?;
    send_drop_message(host, player, npc_id, &item)?;
    let looter = get_looter(host, player, npc, &item)?;
    let created = gm::create_at(host, &manager, Some(item), Some(location), Some(looter))?;
    if host.is_ai_player(player) {
        host.ai_add_item(&created)?;
    }
    if host.bot_script_running(player)? {
        host.bot_add_drop(player, &created)?;
    }
    Ok(())
}

/// `handleBoneCrusher(player, item)`: bones turn into prayer experience
/// instead of a drop when the player has the perk switched on.
pub fn handle_bone_crusher<H: Host>(
    host: &mut H,
    player: &NodeHandle,
    item: &item::Handle,
) -> Result<bool> {
    let Some(experience) = bone_experience(item.read().id()) else {
        return Ok(false);
    };
    if !host.bone_crusher_enabled(player)? {
        return Ok(false);
    }
    let amount = item.read().amount();
    host.add_experience(player, PRAYER, f64::from(amount) * experience)?;
    Ok(true)
}

/// `core.api.announceIfRare(player, item)`.
pub fn announce_if_rare<H: Host>(
    host: &mut H,
    player: &NodeHandle,
    item: &item::Handle,
) -> Result<()> {
    let definition = item.read().definition.clone().ok_or(Error::Null)?;
    let rare = match definition.read().handlers.get(RARE_ITEM) {
        None | Some(Property::Null) => false,
        Some(Property::Bool(value)) => *value,
        Some(_) => return Err(Error::Type),
    };
    if rare {
        let username = host.username(player)?;
        let (amount, item_name) = {
            let item = item.read();
            (item.amount(), item.name.clone())
        };
        host.send_news(&format!(
            "{} has just received: {amount} x {}.",
            username.as_deref().unwrap_or("null"),
            item_name.as_deref().unwrap_or("null")
        ))?;
        host.increment_rare_drop(player, item)?;
    }
    Ok(())
}

/// `Location.getDistance`: planar, the plane is not compared.
fn distance(first: &Location, second: &Location) -> f64 {
    let [x, y, _] = first.coordinates();
    let [other_x, other_y, _] = second.coordinates();
    let x = x.wrapping_sub(other_x);
    let y = y.wrapping_sub(other_y);
    f64::from(x.wrapping_mul(x).wrapping_add(y.wrapping_mul(y))).sqrt()
}

/// `new DecimalFormat("#,###,##0").format(amount)` in the en-US locale.
pub fn formatted_number(amount: i32) -> String {
    let digits = i64::from(amount).unsigned_abs().to_string();
    let mut out = String::new();
    if amount < 0 {
        out.push('-');
    }
    for (index, digit) in digits.chars().enumerate() {
        if index != 0 && (digits.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// `sendDropMessage(killer, npcId, item)`: for the listed NPCs, every player
/// within ten tiles of the killer is told what dropped, unless the item's name
/// contains "bone" or "ashes".
pub fn send_drop_message<H: Host>(
    host: &mut H,
    killer: &NodeHandle,
    npc_id: i32,
    item: &item::Handle,
) -> Result<()> {
    let item_name = name(item)?;
    let lower = lowercase_root(&item_name);
    if lower.contains("bone") || lower.contains("ashes") {
        return Ok(());
    }
    for id in NpcDropTables::MESSAGE_NPCS {
        if npc_id != id {
            continue;
        }
        for player in host.repository_players()? {
            let location = host.player_location(&player)?.ok_or(Error::Null)?;
            let other = host.player_location(killer)?.ok_or(Error::Null)?;
            if distance(&location, &other) > 10.0 {
                continue;
            }
            let mut message = String::from(if host.resizable(&player)? {
                "<col=32CD32>"
            } else {
                "<col=005F00>"
            });
            if player == *killer {
                message.push_str("You received: ");
            } else {
                let username = host.username(killer)?;
                message.push_str(username.as_deref().unwrap_or("null"));
                message.push_str(" received a drop: ");
            }
            let amount = item.read().amount();
            if amount > 1 {
                message.push_str(&formatted_number(amount));
                message.push_str(" x ");
            }
            message.push_str(&name(item)?);
            message.push_str("</col>");
            host.send_message(&player, &message)?;
        }
    }
    Ok(())
}

/// The clan and loot-share conditions `getLooter` asks of one player, after
/// the first clan lookup.
fn shares_loot<H: Host>(host: &mut H, player: &NodeHandle) -> Result<bool> {
    if !host.loot_share(player)? {
        return Ok(false);
    }
    let own = host.loot_requirement(player)?;
    let clan = host.clan(player)?.ok_or(Error::Null)?;
    Ok(own >= host.clan_loot_requirement(&clan)?)
}

/// `getLooter(player, npc, item)`: who the ground item belongs to.
///
/// Loot share applies in a multi-combat zone, to a tradeable item or a charm,
/// for a killer who is in a clan with loot share on, meets the clan's rank
/// requirement and is not an ironman. Of the clan members within 16 tiles of
/// the NPC who also share loot, each one with more points than the highest so
/// far takes the drop on a draw of 2 to 9 out of `random(10)`.
pub fn get_looter<H: Host>(
    host: &mut H,
    player: &NodeHandle,
    npc: &NodeHandle,
    item: &item::Handle,
) -> Result<NodeHandle> {
    let definition = item.read().definition.clone().ok_or(Error::Null)?;
    let item_id = if definition.read().unnoted {
        item.read().id()
    } else {
        item.read().note_change().map_err(|_| Error::Null)?
    };
    if !host.npc_multi_zone(npc)? {
        return Ok(player.clone());
    }
    let tradeable = host.tradeable(&definition)?;
    if !tradeable && !name(item)?.ends_with("charm") {
        return Ok(player.clone());
    }
    if host.clan(player)?.is_none() || !shares_loot(host, player)? || host.ironman(player)? {
        return Ok(player.clone());
    }
    let mut looter = player.clone();
    let location = host.npc_location(npc)?;
    let players = host.local_players(location.as_ref(), 16)?;
    let mut looters = Vec::new();
    for candidate in players.into_iter().flatten() {
        if host.clan(&candidate)?.is_none() {
            continue;
        }
        let theirs = host.clan(&candidate)?;
        let ours = host.clan(player)?;
        if theirs != ours || !shares_loot(host, &candidate)? {
            continue;
        }
        looters.push(candidate);
    }
    if looters.len() > 1 {
        let size = looters.len().min(i32::MAX as usize) as i32;
        let mut highest = 0;
        for candidate in &looters {
            if host.loot_share_points(candidate)? > highest
                && random::random(host.random(), 10) >= 2
            {
                highest = host.loot_share_points(candidate)?;
                looter = candidate.clone();
            }
        }
        let resizable = host.resizable(player)?;
        host.send_message(player, if resizable { "true" } else { "false" })?;
        let item_name = name(item)?;
        let price = if item_name.ends_with("charm") {
            100
        } else {
            host.recommended_price(item_id, false)?
        };
        let share = price / size;
        let points = host.loot_share_points(&looter)?;
        host.set_loot_share_points(&looter, points.wrapping_sub(price).wrapping_add(share))?;
        let amount = item.read().amount();
        let colour = |resizable| {
            if resizable {
                "<col=32CD32>"
            } else {
                "<col=009900>"
            }
        };
        let prefix = colour(host.resizable(player)?);
        host.send_message(
            &looter,
            &format!("{prefix}You received: {amount} {}", name(item)?),
        )?;
        for candidate in &looters {
            if *candidate == looter {
                continue;
            }
            let prefix = colour(host.resizable(player)?);
            let username = host.username(&looter)?;
            host.send_message(
                candidate,
                &format!(
                    "{prefix}{} received: {} {}",
                    username.as_deref().unwrap_or("null"),
                    item.read().amount(),
                    name(item)?
                ),
            )?;
            let points = host.loot_share_points(candidate)?;
            host.set_loot_share_points(candidate, points.wrapping_add(price / size))?;
        }
    }
    Ok(looter)
}
