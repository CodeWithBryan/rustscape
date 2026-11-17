//! The parts of Region.java that region.rs does not hold: the active flag,
//! the viewing players, the activity pulse and its unload decision, the
//! region-zone list with Region.add/remove, the music fields, and
//! RegionChunk.clear as unloading uses it.
//!
//! Activation joins the native `region::load`; the pulse is a native
//! `pulses::Pulse` that the owning world submits to its runner and whose body
//! is `pulse_body`. NPC, repository and log effects are required `Host`
//! services. A DynamicRegion is refused with `Error::Dynamic`.
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
fn refuse_dynamic(region: &region::Handle) -> Result<()> {
    if region.read().dynamic_template.is_some() {
        return Err(Error::Dynamic);
    }
    Ok(())
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
/// `isInactive(runPulse)`: viewed or populated regions are not inactive;
/// otherwise a stopped pulse is rewound, started and submitted again, even
/// while its earlier submission is still queued.
pub fn is_inactive<H: Host>(
    host: &mut H,
    region: &region::Handle,
    run_pulse: bool,
) -> Result<bool> {
    refuse_dynamic(region)?;
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
pub fn flag_active<H: Host>(host: &mut H, region: &region::Handle) -> Result<()> {
    refuse_dynamic(region)?;
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
/// `flagInactive(force)`.
pub fn flag_inactive<H: Host>(host: &mut H, region: &region::Handle, force: bool) -> Result<bool> {
    if unload(host, region, force)? {
        activity(region).write().active = false;
        return Ok(true);
    }
    Ok(false)
}
/// `unload(force)`: refused, with the region flagged active again, while it
/// is viewed or holds a player; otherwise every NPC is told and every chunk
/// cleared, x, y, plane order.
pub fn unload<H: Host>(host: &mut H, region: &region::Handle, force: bool) -> Result<bool> {
    refuse_dynamic(region)?;
    if !force && is_viewed(region) {
        host.log_error("Players viewing region!")?;
        flag_active(host, region)?;
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
                    flag_active(host, region)?;
                    return Ok(false);
                }
            }
        }
    }
    for x in 0..CHUNKS {
        for y in 0..CHUNKS {
            for z in 0..PLANES {
                // `new ArrayList<>(getNpcs())`: a copy.
                let npcs = chunk_at(region, x, y, z)?.read().npcs.values();
                for npc in npcs {
                    host.npc_region_inactivity(&npc.ok_or(Error::Null)?)?;
                }
                clear_chunk(host, &chunk_at(region, x, y, z)?)?;
            }
        }
    }
    region.write().loaded = false;
    pulses::stop_base(&activity_pulse(region));
    Ok(true)
}
/// The activity pulse's `pulse()`: `flagInactive()`, then finished.
pub fn pulse_body<H: Host>(host: &mut H, region: &region::Handle) -> Result<bool> {
    flag_inactive(host, region, false)?;
    Ok(true)
}
/// RegionChunk.clear(): the update flags and both object layers. The entity
/// lists stay. A chunk of a DynamicRegion is refused.
pub fn clear_chunk<H: Host>(host: &mut H, chunk: &HandleChunk) -> Result<()> {
    let flags = chunk.read().update_flags.clone();
    flags.write().clear();
    let base = chunk.read().current_base.clone();
    let region = host.chunk_region(zones::region_id(&base))?;
    refuse_dynamic(&region)?;
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
