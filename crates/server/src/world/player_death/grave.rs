//! `Grave`, `GraveController` and `GraveType`.
use super::{
    Error, GRAVE_TYPE, Host, ORIGINAL_LOCATION, PRAYER, Result, barrows, colorize, index, null,
    seconds_to_ticks, ticks_to_seconds,
};
use crate::{
    definitions::item::{Registry, Shared},
    inventory::item,
    player::ironman::Mode,
    world::{
        doors::Location, ground_item as ground, ground_manager as gm, npc_death::Attribute,
        player_walk::NodeHandle,
    },
};
use rs09_game_core::{java_lower::lowercase_root, java_map::int_key_layout};

/// `GraveType`, in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum GraveType {
    MemPlaque,
    Flag,
    SmallGs,
    OrnateGs,
    FontOfLife,
    Stele,
    SaraSymbol,
    ZamSymbol,
    GuthSymbol,
    BandSymbol,
    ArmaSymbol,
    ZaroSymbol,
    AngelDeath,
}

const MEMORIAL: &str = "In memory of @name,<br>who died here.";
const LOVING: &str =
    "In loving memory of our dear friend @name,<br>who died in this place @mins ago.";
const TRAVELS: &str =
    "In your travels,<br>pause awhile to remember @name,<br>who passed away at this spot.";

impl GraveType {
    /// `GraveType.values()`.
    pub const ALL: [Self; 13] = [
        Self::MemPlaque,
        Self::Flag,
        Self::SmallGs,
        Self::OrnateGs,
        Self::FontOfLife,
        Self::Stele,
        Self::SaraSymbol,
        Self::ZamSymbol,
        Self::GuthSymbol,
        Self::BandSymbol,
        Self::ArmaSymbol,
        Self::ZaroSymbol,
        Self::AngelDeath,
    ];
    /// `GraveType.values()[ordinal]`.
    pub fn from_ordinal(ordinal: i32) -> Option<Self> {
        usize::try_from(ordinal)
            .ok()
            .and_then(|index| Self::ALL.get(index))
            .copied()
    }
    pub fn ordinal(self) -> i32 {
        self as i32
    }
    pub fn name(self) -> &'static str {
        [
            "MEM_PLAQUE",
            "FLAG",
            "SMALL_GS",
            "ORNATE_GS",
            "FONT_OF_LIFE",
            "STELE",
            "SARA_SYMBOL",
            "ZAM_SYMBOL",
            "GUTH_SYMBOL",
            "BAND_SYMBOL",
            "ARMA_SYMBOL",
            "ZARO_SYMBOL",
            "ANGEL_DEATH",
        ][self as usize]
    }
    /// `npcId`: the NPC ids of `org.rs09.consts.NPCs` the original names.
    pub fn npc_id(self) -> i32 {
        6565 + 3 * self as i32
    }
    pub fn cost(self) -> i32 {
        match self {
            Self::MemPlaque => 0,
            Self::Flag => 50,
            Self::SmallGs => 500,
            Self::OrnateGs => 5000,
            Self::AngelDeath => 500_000,
            _ => 50_000,
        }
    }
    pub fn duration_minutes(self) -> i32 {
        match self {
            Self::MemPlaque | Self::Flag | Self::SmallGs => 2,
            Self::OrnateGs => 3,
            Self::AngelDeath => 5,
            _ => 4,
        }
    }
    pub fn members(self) -> bool {
        self >= Self::FontOfLife
    }
    /// `requiredQuest`, as `Quests.toString()` gives it.
    pub fn required_quest(self) -> Option<&'static str> {
        match self {
            Self::BandSymbol => Some("Land of the Goblins"),
            Self::ArmaSymbol => Some("Temple of Ikov"),
            Self::ZaroSymbol => Some("Desert Treasure"),
            _ => None,
        }
    }
    pub fn text(self) -> &'static str {
        match self {
            Self::MemPlaque | Self::Flag => MEMORIAL,
            Self::SmallGs | Self::OrnateGs => LOVING,
            Self::FontOfLife | Self::Stele => TRAVELS,
            Self::SaraSymbol => {
                "@name,<br>an enlightened servant of Saradomin,<br>perished in this place."
            }
            Self::ZamSymbol => {
                "@name,<br>a most bloodthirsty follower of Zamorak,<br>perished in this place."
            }
            Self::GuthSymbol => {
                "@name,<br>who walked with the Balance of Guthix,<br>perished in this place."
            }
            Self::BandSymbol => {
                "@name,<br>a vicious warrior dedicated to Bandos,<br>perished in this place. "
            }
            Self::ArmaSymbol => {
                "@name,<br>a follower of the Law of Armadyl,<br>perished in this place."
            }
            Self::ZaroSymbol => {
                "@name,<br>servant of the Unknown Power,<br>perished in this place."
            }
            Self::AngelDeath => {
                "Ye frail mortals who gaze upon this sight,<br>forget not the fate of @name, once mighty, now<br>surrendered to the inescapable grasp of destiny.<br><i>Requiescat in pace.</i>"
            }
        }
    }
    /// `GraveType.ids`: every type's NPC id and the two after it.
    pub fn ids() -> Vec<i32> {
        Self::ALL
            .iter()
            .flat_map(|kind| [kind.npc_id(), kind.npc_id() + 1, kind.npc_id() + 2])
            .collect()
    }
}

/// A grave. Its NPC (`AbstractNPC`) is the binding's: id, location, walking
/// flags, `init` and `clear` are services.
pub struct Grave {
    pub npc: NodeHandle,
    /// `lateinit var type`.
    kind: Option<GraveType>,
    items: Vec<ground::Handle>,
    pub owner_username: String,
    pub owner_uid: i32,
    pub ticks_remaining: i32,
    pub ticks_since_death: i32,
}
pub type Handle = Shared<Grave>;

impl Grave {
    /// The fields `new Grave()` leaves: no type yet, no owner (`-1`, an empty
    /// username), `ticksRemaining` -1. The binding calls this for the NPC it
    /// constructs in [`Host::new_grave`].
    pub fn new(npc: NodeHandle) -> Self {
        Self {
            npc,
            kind: None,
            items: Vec::new(),
            owner_username: String::new(),
            owner_uid: -1,
            ticks_remaining: -1,
            ticks_since_death: 0,
        }
    }
    /// `type`; reading it before `configureType` fails as Kotlin's
    /// `lateinit` does.
    pub fn kind(&self) -> Result<GraveType> {
        self.kind.ok_or(Error::Uninitialized)
    }
    /// `getItems()`.
    pub fn items(&self) -> Vec<ground::Handle> {
        self.items.clone()
    }
}

/// `GraveController.activeGraves`, a `HashMap<Int, Grave>`: kept in its
/// iteration order, with the table size kept across removals.
#[derive(Default)]
pub struct Controller {
    graves: Vec<(i32, Handle)>,
    capacity: usize,
}
impl Controller {
    /// `activeGraves[uid]`.
    pub fn get(&self, uid: i32) -> Option<Handle> {
        self.graves
            .iter()
            .find(|(key, _)| *key == uid)
            .map(|(_, grave)| grave.clone())
    }
    /// `activeGraves[uid] = grave`: a present key keeps its place.
    pub fn put(&mut self, uid: i32, grave: Handle) {
        if let Some(slot) = self.graves.iter_mut().find(|(key, _)| *key == uid) {
            slot.1 = grave;
            return;
        }
        self.graves.push((uid, grave));
        let keys = self.graves.iter().map(|(key, _)| *key);
        self.capacity = int_key_layout(keys, self.capacity.max(16)).1;
    }
    /// `activeGraves.remove(uid)`.
    pub fn remove(&mut self, uid: i32) -> Option<Handle> {
        let index = self.graves.iter().position(|(key, _)| *key == uid)?;
        Some(self.graves.remove(index).1)
    }
    /// The entries in `HashMap` iteration order.
    pub fn entries(&self) -> Vec<(i32, Handle)> {
        let keys = self.graves.iter().map(|(key, _)| *key);
        int_key_layout(keys, self.capacity.max(16))
            .0
            .into_iter()
            .filter_map(|uid| self.get(uid).map(|grave| (uid, grave)))
            .collect()
    }
    /// `activeGraves.clear()`.
    pub fn clear(&mut self) {
        self.graves.clear();
    }

    /// `GraveController.tick()`, once per server tick. A grave at -1 ends the
    /// whole tick; at 150 and 50 ticks left a grave turns into the next NPC
    /// id; at 0 it collapses (and is still counted down).
    pub fn tick<H: Host>(&mut self, host: &mut H) -> Result<()> {
        for (_, grave) in self.entries() {
            if grave.read().ticks_remaining == -1 {
                return Ok(());
            }
            let remaining = grave.read().ticks_remaining;
            if remaining == seconds_to_ticks(30) || remaining == seconds_to_ticks(90) {
                let npc = grave.read().npc.clone();
                let id = host.npc_id(&npc)?;
                host.transform(&npc, id.wrapping_add(1))?;
            }
            if grave.read().ticks_remaining == 0 {
                collapse(host, self, &grave)?;
            }
            let mut grave = grave.write();
            grave.ticks_remaining = grave.ticks_remaining.wrapping_sub(1);
            grave.ticks_since_death = grave.ticks_since_death.wrapping_add(1);
        }
        Ok(())
    }
}

/// `GraveController.allowGenerate(player)`: no grave when skulled, in the
/// wilderness by skull or zone, a hardcore ironman, or where graves are
/// restricted.
pub fn allow_generate<H: Host>(host: &mut H, player: &NodeHandle) -> Result<bool> {
    if host.skulled(player)? {
        return Ok(false);
    }
    if host.skull_wilderness(player)? {
        return Ok(false);
    }
    if host.in_wilderness_zone(player)? {
        return Ok(false);
    }
    if host.ironman_mode(player)? == Some(Mode::Hardcore) {
        return Ok(false);
    }
    if host.graves_restricted(player)? {
        return Ok(false);
    }
    Ok(true)
}

/// `GraveController.getGraveType(player)`: the saved attribute, 0 when absent.
pub fn grave_type<H: Host>(host: &mut H, player: &NodeHandle) -> Result<GraveType> {
    let ordinal = host.attribute_int(player, GRAVE_TYPE, 0)?;
    GraveType::from_ordinal(ordinal).ok_or_else(index)
}

/// `GraveController.shouldCrumble(item)`: the ectophial, the essence pouches
/// and anything with a destroy option.
pub fn should_crumble<H: Host>(host: &mut H, id: i32) -> Result<bool> {
    if id == 4251 || (5509..=5515).contains(&id) {
        return Ok(true);
    }
    let definition = host.item_definition(id)?;
    let destroy = definition.read().has_action(Some("destroy"));
    Ok(destroy)
}

/// `GraveController.shouldRelease(item)`: chinchompas and unnoted impling jars.
pub fn should_release<H: Host>(host: &mut H, id: i32) -> Result<bool> {
    match id {
        9976 | 10033 => Ok(true),
        11238..=11257 => {
            let definition = host.item_definition(id)?;
            let unnoted = definition.read().unnoted;
            Ok(unnoted)
        }
        _ => Ok(false),
    }
}

/// `GraveController.checkTransform(item)`: an item plugin's death item. The
/// plugin is looked up twice, as `hasItemPlugin()` and `item.plugin` do; a
/// plugin that gives no item fails Kotlin's non-null return.
pub fn check_transform<H: Host>(host: &mut H, item: &item::Handle) -> Result<item::Handle> {
    let definition = item.read().definition.clone();
    let has = match definition {
        Some(definition) => host.plugin(&definition)?.is_some(),
        None => false,
    };
    if !has {
        return Ok(item.clone());
    }
    let definition = item.read().definition.clone();
    let plugin = match definition {
        Some(definition) => host.plugin(&definition)?,
        None => None,
    };
    let plugin = plugin.ok_or_else(null)?;
    host.plugin_death_item(plugin, item)?.ok_or_else(null)
}

/// `GraveController.produceGrave(type)`: `new Grave()` and `configureType`.
pub fn produce_grave<H: Host>(host: &mut H, kind: GraveType) -> Result<Handle> {
    let grave = host.new_grave()?;
    grave.write().kind = Some(kind);
    let npc = grave.read().npc.clone();
    host.transform(&npc, kind.npc_id())?;
    grave.write().ticks_remaining = seconds_to_ticks(kind.duration_minutes().wrapping_mul(60));
    Ok(grave)
}

/// `Grave.initialize(player, location, inventory)`.
///
/// Nothing when no grave is allowed. The grave takes the player's uid and
/// username and stands at `/save:original-loc` or the given location. Each
/// item is released (with a message), crumbles (with a message), loses a
/// fifth of its durability when it is a damaged Barrows piece, or becomes its
/// plugin's death item; it is then a ground item for the player that stays
/// private and decays when the grave would. A grave with no items is cleared
/// and not registered. Otherwise its NPC is registered, the owner's previous
/// grave collapses, this one replaces it and the owner is told how long it
/// lasts.
#[allow(clippy::too_many_arguments)]
pub fn initialize<H: Host>(
    host: &mut H,
    controller: &mut Controller,
    grave: &Handle,
    player: &NodeHandle,
    location: Option<Location>,
    items: &[item::Handle],
    definitions: &Registry,
) -> Result<()> {
    if !allow_generate(host, player)? {
        return Ok(());
    }
    let uid = host.uid(player)?;
    grave.write().owner_uid = uid;
    let username = host.username(player)?.ok_or_else(null)?;
    grave.write().owner_username = username;
    let npc = grave.read().npc.clone();
    let at = match host.attribute_location(player, ORIGINAL_LOCATION)? {
        Some(at) => at,
        None => location,
    };
    host.set_location_field(&npc, at)?;
    host.set_respawn(&npc, false)?;
    host.set_walks(&npc, false)?;
    host.set_never_walks(&npc, true)?;
    let manager = host.manager();
    let kind = grave.read().kind()?;
    for item in items {
        let id = item.read().id();
        if should_release(host, id)? {
            let name = item.read().name.clone().ok_or_else(null)?;
            let name = lowercase_root(&name).replace("jar", "");
            host.api_message(player, &format!("Your {name} has escaped."))?;
            continue;
        }
        if should_crumble(host, id)? {
            let name = item.read().name.clone().ok_or_else(null)?;
            let name = lowercase_root(&name);
            host.api_message(player, &format!("Your {name} has crumbled to dust."))?;
            continue;
        }
        let final_item = if barrows::is_barrows(id) && !barrows::is_fully_repaired(id) {
            barrows::grave_death_durability_reduction(item, definitions)
                .map_err(|_| index())?
                .unwrap_or_else(|| item.clone())
        } else {
            check_transform(host, item)?
        };
        let at = host.location_field(&npc)?;
        let ground = gm::create_at(host, &manager, Some(final_item), at, Some(player.clone()))?;
        ground.write().remain_private = true;
        ground::set_decay_time(
            host,
            &ground,
            seconds_to_ticks(kind.duration_minutes().wrapping_mul(60)),
        )?;
        grave.write().items.push(ground);
    }
    if grave.read().items.is_empty() {
        host.npc_clear(&npc)?;
        return Ok(());
    }
    host.npc_init(&npc)?;
    let owner = grave.read().owner_uid;
    if controller.get(owner).is_some()
        && let Some(old) = controller.get(owner)
    {
        collapse(host, controller, &old)?;
    }
    controller.put(owner, grave.clone());
    host.api_message(
        player,
        &colorize(&format!(
            "%RBecause of your current gravestone, you have {} minutes to get your items back.",
            kind.duration_minutes()
        )),
    )?;
    Ok(())
}

/// `Grave.setupFromJsonParams(uid, ticks, location, items, username)`: a
/// grave read back from the server store. Its items are ground items for the
/// owner's uid that decay `ticks` from now and stay private.
pub fn setup_from_json<H: Host>(
    host: &mut H,
    grave: &Handle,
    uid: i32,
    ticks: i32,
    location: Location,
    items: &[item::Handle],
    username: String,
) -> Result<()> {
    let npc = grave.read().npc.clone();
    grave.write().owner_uid = uid;
    grave.write().ticks_remaining = ticks;
    host.set_location_field(&npc, Some(location.clone()))?;
    host.set_respawn(&npc, false)?;
    host.set_walks(&npc, false)?;
    host.set_never_walks(&npc, true)?;
    grave.write().owner_username = username;
    let manager = host.manager();
    for item in items {
        let until = host.ticks()?.wrapping_add(grave.read().ticks_remaining);
        let ground = gm::create_uid(
            host,
            &manager,
            Some(item.clone()),
            Some(location.clone()),
            uid,
            until,
        )?;
        ground.write().remain_private = true;
        grave.write().items.push(ground);
    }
    let kind = grave.read().kind()?;
    host.transform(&npc, kind.npc_id())?;
    host.npc_init(&npc)?;
    Ok(())
}

/// `Grave.tick()`: the owner, when online, gets the hint icon every tick.
pub fn npc_tick<H: Host>(host: &mut H, grave: &Handle) -> Result<()> {
    let uid = grave.read().owner_uid;
    if host.uid_map(uid)?.is_some() {
        let Some(player) = host.uid_map(uid)? else {
            return Ok(());
        };
        let npc = grave.read().npc.clone();
        host.register_hint_icon(&player, &npc)?;
    }
    Ok(())
}

/// `Grave.addTime(ticks)`: every item decays when the grave would, counted
/// from now, and the NPC shows the grave's state (thresholds of 30 and 90
/// ticks, unlike the controller's 50 and 150).
pub fn add_time<H: Host>(host: &mut H, grave: &Handle, ticks: i32) -> Result<()> {
    {
        let mut grave = grave.write();
        grave.ticks_remaining = grave.ticks_remaining.wrapping_add(ticks);
    }
    for item in grave.read().items() {
        let remaining = grave.read().ticks_remaining;
        ground::set_decay_time(host, &item, remaining)?;
    }
    let (npc, kind, remaining) = {
        let grave = grave.read();
        (grave.npc.clone(), grave.kind()?, grave.ticks_remaining)
    };
    let id = if remaining < 30 {
        kind.npc_id() + 2
    } else if remaining < 90 {
        kind.npc_id() + 1
    } else {
        kind.npc_id()
    };
    host.transform(&npc, id)?;
    Ok(())
}

/// `Grave.collapse()`: its items are removed from the ground, the NPC is
/// cleared, the grave is no longer active and an online owner's hint icon is
/// cleared.
pub fn collapse<H: Host>(host: &mut H, controller: &mut Controller, grave: &Handle) -> Result<()> {
    let manager = host.manager();
    for item in grave.read().items() {
        gm::destroy(host, &manager, Some(item))?;
    }
    let (npc, uid) = {
        let grave = grave.read();
        (grave.npc.clone(), grave.owner_uid)
    };
    host.npc_clear(&npc)?;
    controller.remove(uid);
    if host.uid_map(uid)?.is_some() {
        let Some(player) = host.uid_map(uid)? else {
            return Ok(());
        };
        host.clear_hint_icon(&player)?;
    }
    Ok(())
}

/// `Grave.demolish()`: only with the owner online. Items not yet removed
/// decay 45 seconds from now (still private), the NPC is cleared and the
/// grave is no longer active.
pub fn demolish<H: Host>(host: &mut H, controller: &mut Controller, grave: &Handle) -> Result<()> {
    let uid = grave.read().owner_uid;
    let Some(owner) = host.uid_map(uid)? else {
        return Ok(());
    };
    for item in grave.read().items() {
        let removed = item.read().removed;
        if !removed {
            ground::set_decay_time(host, &item, seconds_to_ticks(45))?;
        }
    }
    let npc = grave.read().npc.clone();
    host.npc_clear(&npc)?;
    let remaining = formatted_time_remaining(&grave.read());
    host.api_message(
        &owner,
        &format!("It looks like it'll last another {remaining}."),
    )?;
    host.api_message(&owner, "You demolish it anyway.")?;
    controller.remove(uid);
    host.clear_hint_icon(&owner)?;
    Ok(())
}

/// `Grave.formatDuration(ticks)`.
pub fn format_duration(ticks: i32) -> String {
    let seconds = ticks_to_seconds(ticks);
    let minutes = seconds / 60 > 0;
    let quantity = if minutes { seconds / 60 } else { seconds };
    let unit = if minutes { "minute" } else { "second" };
    let plural = if quantity > 1 { "s" } else { "" };
    format!("{quantity} {unit}{plural}")
}
/// `getFormattedTimeRemaining()`.
pub fn formatted_time_remaining(grave: &Grave) -> String {
    format_duration(grave.ticks_remaining)
}
/// `retrieveFormattedText()`.
pub fn retrieve_formatted_text(grave: &Grave) -> Result<String> {
    Ok(grave
        .kind()?
        .text()
        .replace("@name", &grave.owner_username)
        .replace("@mins", &format_duration(grave.ticks_since_death)))
}

/// The "read" option: the gravestone interface with its text and how long it
/// will last.
pub fn on_read<H: Host>(host: &mut H, player: &NodeHandle, grave: Option<&Handle>) -> Result<bool> {
    let Some(grave) = grave else {
        return Ok(false);
    };
    let granite = (GraveType::SmallGs..=GraveType::AngelDeath).contains(&grave.read().kind()?);
    host.set_varbit(player, 4191, i32::from(granite))?;
    host.open_interface(player, 266)?;
    let text = retrieve_formatted_text(&grave.read())?;
    host.interface_text(player, &text, 266, 23)?;
    let remaining = formatted_time_remaining(&grave.read());
    host.api_message(
        player,
        &format!("It looks like it'll survive another {remaining}."),
    )?;
    if host.uid(player)? == grave.read().owner_uid {
        host.api_message(
            player,
            "Isn't there something a bit odd about reading your own gravestone?",
        )?;
    }
    Ok(true)
}

/// The "bless" option: once per grave, not one's own, not an online
/// ironman's, Prayer 70; adds up to 60 minutes for prayer points above 10.
pub fn on_bless<H: Host>(
    host: &mut H,
    player: &NodeHandle,
    grave: Option<&Handle>,
) -> Result<bool> {
    let Some(grave) = grave else {
        return Ok(false);
    };
    let npc = grave.read().npc.clone();
    if host.attribute_bool(&npc, "blessed", false)? {
        host.api_message(player, "This grave has already been blessed.")?;
        return Ok(true);
    }
    let uid = grave.read().owner_uid;
    if host.uid(player)? == uid {
        host.api_message(
            player,
            "The gods don't seem to approve of people attempting to bless their own gravestones.",
        )?;
        return Ok(true);
    }
    let owner = host.uid_map(uid)?;
    if let Some(owner) = &owner
        && host.ironman(owner)?
    {
        host.api_message(player, "This grave belongs to an Ironman.")?;
        return Ok(true);
    }
    if host.stat_level(player, PRAYER)? < 70 {
        host.api_message(player, "You need a Prayer level of 70 to bless a grave.")?;
        return Ok(true);
    }
    let amount = 60.min((host.prayer_points(player)? as i32).wrapping_sub(10));
    if amount <= 0 {
        host.api_message(player, "You do not have enough prayer points to do that.")?;
        return Ok(true);
    }
    add_time(host, grave, seconds_to_ticks(amount.wrapping_mul(60)))?;
    let points = host.prayer_points(player)?;
    host.set_prayer_points(player, points - f64::from(amount))?;
    host.set_attribute(&npc, "blessed", Attribute::Bool(true))?;
    host.play_audio(player, 2674)?;
    host.animate(player, 645)?;
    if let Some(owner) = &owner {
        host.api_message(owner, &colorize("%RYour grave has been blessed."))?;
    }
    Ok(true)
}

/// The "repair" option: once per grave, Prayer 2 and a prayer point; adds up
/// to five minutes.
pub fn on_repair<H: Host>(
    host: &mut H,
    player: &NodeHandle,
    grave: Option<&Handle>,
) -> Result<bool> {
    let Some(grave) = grave else {
        return Ok(false);
    };
    let npc = grave.read().npc.clone();
    if host.attribute_bool(&npc, "repaired", false)? {
        host.api_message(player, "This grave has already been repaired.")?;
        return Ok(true);
    }
    if host.stat_level(player, PRAYER)? < 2 {
        host.api_message(player, "You need a Prayer level of 2 to bless a grave.")?;
        return Ok(true);
    }
    if host.prayer_points(player)? < 1.0 {
        host.api_message(player, "You do not have enough prayer points to do that.")?;
        return Ok(true);
    }
    let amount = 5.min(host.prayer_points(player)? as i32);
    add_time(host, grave, seconds_to_ticks(amount.wrapping_mul(60)))?;
    let points = host.prayer_points(player)?;
    host.set_prayer_points(player, points - f64::from(amount))?;
    host.set_attribute(&npc, "repaired", Attribute::Bool(true))?;
    host.play_audio(player, 2674)?;
    host.animate(player, 645)?;
    Ok(true)
}

/// The "demolish" option: the owner's only.
pub fn on_demolish<H: Host>(
    host: &mut H,
    controller: &mut Controller,
    player: &NodeHandle,
    grave: Option<&Handle>,
) -> Result<bool> {
    let Some(grave) = grave else {
        return Ok(false);
    };
    if host.uid(player)? != grave.read().owner_uid {
        host.api_message(player, "You cannot demolish someone else's gravestone!")?;
        return Ok(true);
    }
    demolish(host, controller, grave)?;
    Ok(true)
}
