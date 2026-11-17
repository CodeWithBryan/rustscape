//! The parts of Region.java that region.rs does not hold: the active flag,
//! the viewing players, the activity pulse and its unload decision, the
//! region-zone list with Region.add/remove, the music fields, and
//! RegionChunk.clear as unloading uses it.
//!
//! Activation joins the native `region::load`; the pulse is a native
//! `pulses::Pulse` that the owning world submits to its runner and whose body
//! is `pulse_body`. NPC, repository and log effects are required `Host`
//! services. The functions without a suffix treat every region as a plain
//! Region and refuse a DynamicRegion with `Error::Dynamic`; the `_in`
//! versions take the region class (`Kind`), which is how
//! `dynamic_region` reaches DynamicRegion's `flagInactive` override and the
//! `instanceof DynamicRegion` branches of `unload` and `RegionChunk.clear`.
use super::{
    chunk::{self, HandleChunk},
    player_walk::NodeHandle,
    region, update,
    zones::{self, List, MusicZone, RegionZone},
};
use crate::{definitions::item::Shared, pulses};
use std::{any::Any, sync::Arc};

pub use zones::{Error, Result};

pub const CHUNKS: usize = 8;
pub const PLANES: usize = 4;
/// `new Pulse(50)`: fifty runner updates.
pub const ACTIVITY_DELAY: i32 = 50;

impl From<region::Error> for Error {
    fn from(error: region::Error) -> Self {
        match error {
            region::Error::Null => Self::Null,
            region::Error::ConcurrentModification => Self::ConcurrentModification,
            region::Error::Binding(text) => Self::Binding(text),
        }
    }
}

/// The per-region fields. The zone list is stored behind `Any` because zones
/// are typed by the owning world's host; `region_zones` gives it its type.
pub struct Activity {
    pub active: bool,
    pub viewing_players: Vec<String>,
    /// The anonymous `Region$1` pulse, created stopped.
    pub pulse: pulses::Handle,
    pub music: i32,
    pub music_zones: List<MusicZone>,
    zones: Option<Arc<dyn Any + Send + Sync>>,
    /// DynamicRegion's own fields; `None` for a plain Region.
    pub dynamic: Option<Shared<super::dynamic_region::Instance>>,
}
impl Default for Activity {
    fn default() -> Self {
        let mut pulse = pulses::Pulse::new(
            ACTIVITY_DELAY,
            None,
            pulses::Class::Other,
            "core.game.world.map.Region$1".into(),
        );
        pulse.running = false;
        Self {
            active: false,
            viewing_players: Vec::new(),
            pulse: Shared::new(pulse),
            music: -1,
            music_zones: List::new(),
            zones: None,
            dynamic: None,
        }
    }
}

/// `getRegionZones()`: the live list. A region whose list was created for a
/// different host type is a binding error, never a second list.
pub fn region_zones<H: zones::Host>(region: &region::Handle) -> Result<List<RegionZone<H>>> {
    let activity = region.read().activity.clone();
    let mut activity = activity.write();
    if let Some(list) = &activity.zones {
        return list
            .clone()
            .downcast::<List<RegionZone<H>>>()
            .map(|list| (*list).clone())
            .map_err(|_| Error::Binding("region zones belong to another host type".into()));
    }
    let list = List::<RegionZone<H>>::new();
    activity.zones = Some(Arc::new(list.clone()));
    Ok(list)
}
fn activity(region: &region::Handle) -> Shared<Activity> {
    region.read().activity.clone()
}
fn is_dynamic(region: &region::Handle) -> bool {
    region.read().dynamic_template.is_some()
}
fn refuse_dynamic<K: Kind<H>, H: Host>(region: &region::Handle) -> Result<()> {
    if !K::DYNAMIC && is_dynamic(region) {
        return Err(Error::Dynamic);
    }
    Ok(())
}
/// The class of the region objects a host has: what Region's methods
/// dispatch on. `Static` knows only Region; `dynamic_region::Dynamic` also
/// DynamicRegion.
pub trait Kind<H: Host> {
    /// Whether DynamicRegion objects are accepted rather than refused.
    const DYNAMIC: bool;
    /// `region.flagInactive(force)`, a virtual call.
    fn flag_inactive(host: &mut H, region: &region::Handle, force: bool) -> Result<bool>;
}
/// Plain regions only; a DynamicRegion is `Error::Dynamic`.
pub struct Static;
impl<H: Host> Kind<H> for Static {
    const DYNAMIC: bool = false;
    fn flag_inactive(host: &mut H, region: &region::Handle, force: bool) -> Result<bool> {
        base_flag_inactive::<Self, H>(host, region, force)
    }
}
/// `chunks[x][y][z]`, read again on every use as the original does.
fn chunk_at(region: &region::Handle, x: usize, y: usize, z: usize) -> Result<HandleChunk> {
    let chunks = region.read().chunks.clone().ok_or(Error::Null)?;
    let chunk = chunks.read()[x][y][z].clone().ok_or(Error::Null)?;
    Ok(chunk)
}
type Entities = update::List<Option<NodeHandle>>;
/// The enhanced for loop over a chunk's live entity list.
fn each_entity(
    list: &Entities,
    mut body: impl FnMut(Option<NodeHandle>) -> Result<()>,
) -> Result<()> {
    let mut failure = None;
    let result = list.each(|node| {
        body(node).map_err(|error| {
            failure = Some(error);
            update::Error::Binding(String::new())
        })
    });
    result.map_err(|error| {
        failure.unwrap_or(match error {
            update::Error::ConcurrentModification => Error::ConcurrentModification,
            update::Error::Null => Error::Null,
            other => Error::Binding(other.to_string()),
        })
    })
}
fn update_entities<H: zones::Host>(
    host: &mut H,
    region: &region::Handle,
    skip_null_players: bool,
) -> Result<()> {
    for x in 0..CHUNKS {
        for y in 0..CHUNKS {
            for z in 0..PLANES {
                let npcs = chunk_at(region, x, y, z)?.read().npcs.clone();
                each_entity(&npcs, |npc| {
                    let npc = npc.ok_or(Error::Null)?;
                    let monitor = host.monitor(&npc)?;
                    let location = host.location(&npc)?;
                    monitor.update_location(host, location.as_ref())?;
                    Ok(())
                })?;
                let players = chunk_at(region, x, y, z)?.read().players.clone();
                each_entity(&players, |player| {
                    let Some(player) = player else {
                        return if skip_null_players {
                            Ok(())
                        } else {
                            Err(Error::Null)
                        };
                    };
                    let monitor = host.monitor(&player)?;
                    let location = host.location(&player)?;
                    monitor.update_location(host, location.as_ref())?;
                    Ok(())
                })?;
            }
        }
    }
    Ok(())
}
/// Region.add(RegionZone): append, then every NPC and player of the region
/// has its monitor updated at its own location, chunk x, y, plane order,
/// NPCs before players. The monitors' results are ignored.
pub fn add_zone<H: zones::Host>(
    host: &mut H,
    region: &region::Handle,
    zone: RegionZone<H>,
) -> Result<()> {
    region_zones::<H>(region)?.add(zone);
    update_entities(host, region, true)
}
/// Region.remove(RegionZone): the first equal entry goes, then the same
/// walk, here without the null-player check.
pub fn remove_zone<H: zones::Host>(
    host: &mut H,
    region: &region::Handle,
    zone: &RegionZone<H>,
) -> Result<()> {
    let list = region_zones::<H>(region)?;
    for (index, candidate) in list.values().iter().enumerate() {
        if zone.equals(candidate)? {
            list.remove_at(index)?;
            break;
        }
    }
    update_entities(host, region, false)
}

/// Required services of activation and unloading; `region::Context` is what
/// the native Region.load needs.
pub trait Host: region::Context {
    /// GameWorld.getPulser().submit(activityPulse).
    fn submit_pulse(&mut self, pulse: &pulses::Handle) -> Result<()>;
    fn npc_active(&mut self, npc: &NodeHandle) -> Result<bool>;
    /// Repository.addRenderableNPC(npc).
    fn add_renderable_npc(&mut self, npc: &NodeHandle) -> Result<()>;
    /// NPC.onRegionInactivity().
    fn npc_region_inactivity(&mut self, npc: &NodeHandle) -> Result<()>;
    /// `log(CommunicationInfo.class, Log.ERR, message)`.
    fn log_error(&mut self, message: &str) -> Result<()>;
    /// RegionManager.forId, as RegionChunk.clear calls it.
    fn chunk_region(&mut self, region_id: i32) -> Result<region::Handle>;
}

pub fn is_active(region: &region::Handle) -> bool {
    activity(region).read().active
}
pub fn is_viewed(region: &region::Handle) -> bool {
    !activity(region).read().viewing_players.is_empty()
}
pub fn viewing_players(region: &region::Handle) -> Vec<String> {
    activity(region).read().viewing_players.clone()
}
pub fn increment_view_amount(region: &region::Handle, player_name: &str) -> i32 {
    let activity = activity(region);
    let mut activity = activity.write();
    if !activity.viewing_players.iter().any(|n| n == player_name) {
        activity.viewing_players.push(player_name.to_owned());
    }
    activity.viewing_players.len() as i32
}
pub fn decrement_view_amount(region: &region::Handle, player_name: &str) -> i32 {
    let activity = activity(region);
    let mut activity = activity.write();
    if let Some(index) = activity
        .viewing_players
        .iter()
        .position(|n| n == player_name)
    {
        activity.viewing_players.remove(index);
    }
    activity.viewing_players.len() as i32
}
/// `setRegionTimeOut(ticks)`.
pub fn set_region_time_out(region: &region::Handle, ticks: i32) {
    activity(region).read().pulse.write().delay = ticks;
}
pub fn activity_pulse(region: &region::Handle) -> pulses::Handle {
    activity(region).read().pulse.clone()
}

pub fn check_inactive<H: Host>(host: &mut H, region: &region::Handle) -> Result<bool> {
    is_inactive(host, region, true)
}
pub fn is_inactive<H: Host>(
    host: &mut H,
    region: &region::Handle,
    run_pulse: bool,
) -> Result<bool> {
    is_inactive_in::<Static, H>(host, region, run_pulse)
}
pub fn flag_active<H: Host>(host: &mut H, region: &region::Handle) -> Result<()> {
    flag_active_in::<Static, H>(host, region)
}
pub fn flag_inactive<H: Host>(host: &mut H, region: &region::Handle, force: bool) -> Result<bool> {
    <Static as Kind<H>>::flag_inactive(host, region, force)
}
pub fn unload<H: Host>(host: &mut H, region: &region::Handle, force: bool) -> Result<bool> {
    unload_in::<Static, H>(host, region, force)
}
pub fn pulse_body<H: Host>(host: &mut H, region: &region::Handle) -> Result<bool> {
    pulse_body_in::<Static, H>(host, region)
}
pub fn clear_chunk<H: Host>(host: &mut H, chunk: &HandleChunk) -> Result<()> {
    clear_chunk_in::<Static, H>(host, chunk)
}
/// `isInactive(runPulse)`: viewed or populated regions are not inactive;
/// otherwise a stopped pulse is rewound, started and submitted again, even
/// while its earlier submission is still queued.
pub fn is_inactive_in<K: Kind<H>, H: Host>(
    host: &mut H,
    region: &region::Handle,
    run_pulse: bool,
) -> Result<bool> {
    refuse_dynamic::<K, H>(region)?;
    if is_viewed(region) {
        return Ok(false);
    }
    for x in 0..CHUNKS {
        for y in 0..CHUNKS {
            for z in 0..PLANES {
                if !chunk_at(region, x, y, z)?
                    .read()
                    .players
                    .values()
                    .is_empty()
                {
                    return Ok(false);
                }
            }
        }
    }
    if run_pulse {
        let pulse = activity_pulse(region);
        if !pulses::is_running_base(&pulse) {
            pulse.write().restart();
            pulses::start_base(&pulse);
            host.submit_pulse(&pulse)?;
        }
    }
    Ok(true)
}
/// `flagActive()`: the pulse stops; a region that was not active loads and
/// hands its active NPCs to the repository.
pub fn flag_active_in<K: Kind<H>, H: Host>(host: &mut H, region: &region::Handle) -> Result<()> {
    refuse_dynamic::<K, H>(region)?;
    pulses::stop_base(&activity_pulse(region));
    let activity = activity(region);
    if activity.read().active {
        return Ok(());
    }
    activity.write().active = true;
    region::load(host, region, None)?;
    for x in 0..CHUNKS {
        for y in 0..CHUNKS {
            for z in 0..PLANES {
                let npcs = chunk_at(region, x, y, z)?.read().npcs.clone();
                each_entity(&npcs, |npc| {
                    let npc = npc.ok_or(Error::Null)?;
                    if host.npc_active(&npc)? {
                        host.add_renderable_npc(&npc)?;
                    }
                    Ok(())
                })?;
            }
        }
    }
    Ok(())
}
/// Region's own `flagInactive(force)` (DynamicRegion's `super` call).
pub fn base_flag_inactive<K: Kind<H>, H: Host>(
    host: &mut H,
    region: &region::Handle,
    force: bool,
) -> Result<bool> {
    if unload_in::<K, H>(host, region, force)? {
        activity(region).write().active = false;
        return Ok(true);
    }
    Ok(false)
}
/// `unload(force)`: refused, with the region flagged active again, while it
/// is viewed or holds a player; otherwise every NPC of a plain Region is
/// told (a DynamicRegion's are not) and every chunk cleared, x, y, plane
/// order.
pub fn unload_in<K: Kind<H>, H: Host>(
    host: &mut H,
    region: &region::Handle,
    force: bool,
) -> Result<bool> {
    refuse_dynamic::<K, H>(region)?;
    if !force && is_viewed(region) {
        host.log_error("Players viewing region!")?;
        flag_active_in::<K, H>(host, region)?;
        return Ok(false);
    }
    for x in 0..CHUNKS {
        for y in 0..CHUNKS {
            for z in 0..PLANES {
                if !force
                    && !chunk_at(region, x, y, z)?
                        .read()
                        .players
                        .values()
                        .is_empty()
                {
                    let id = region.read().id();
                    let count = chunk_at(region, x, y, z)?.read().players.values().len();
                    host.log_error(&format!(
                        "Players still in region! (region id {id}; {count} players on a chunk)"
                    ))?;
                    flag_active_in::<K, H>(host, region)?;
                    return Ok(false);
                }
            }
        }
    }
    for x in 0..CHUNKS {
        for y in 0..CHUNKS {
            for z in 0..PLANES {
                if !is_dynamic(region) {
                    // `new ArrayList<>(getNpcs())`: a copy.
                    let npcs = chunk_at(region, x, y, z)?.read().npcs.values();
                    for npc in npcs {
                        host.npc_region_inactivity(&npc.ok_or(Error::Null)?)?;
                    }
                }
                clear_chunk_in::<K, H>(host, &chunk_at(region, x, y, z)?)?;
            }
        }
    }
    region.write().loaded = false;
    pulses::stop_base(&activity_pulse(region));
    Ok(true)
}
/// The activity pulse's `pulse()`: `flagInactive()`, then finished.
pub fn pulse_body_in<K: Kind<H>, H: Host>(host: &mut H, region: &region::Handle) -> Result<bool> {
    K::flag_inactive(host, region, false)?;
    Ok(true)
}
/// RegionChunk.clear(): the update flags, a DynamicRegion chunk's item list
/// (emptied and dropped, so `getItems` makes a new one), then both object
/// layers. The entity lists stay.
pub fn clear_chunk_in<K: Kind<H>, H: Host>(host: &mut H, chunk: &HandleChunk) -> Result<()> {
    let flags = chunk.read().update_flags.clone();
    flags.write().clear();
    let base = chunk.read().current_base.clone();
    let region = host.chunk_region(zones::region_id(&base))?;
    refuse_dynamic::<K, H>(&region)?;
    let items = chunk.read().items.clone();
    if let Some(items) = items.filter(|_| is_dynamic(&region)) {
        items.write().clear();
        chunk.write().items = None;
    }
    let (statics, objects) = {
        let chunk = chunk.read();
        (chunk.static_objects.clone(), chunk.objects.clone())
    };
    for x in 0..chunk::SIZE {
        for y in 0..chunk::SIZE {
            for i in 0..chunk::ARRAY_SIZE {
                objects.write()[x][y][i] = None;
                statics.write()[x][y][i] = None;
            }
        }
    }
    Ok(())
}
