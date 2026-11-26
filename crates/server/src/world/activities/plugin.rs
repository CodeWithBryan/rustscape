//! ActivityPlugin and CutscenePlugin: a MapZone subclass (a native
//! [`Zone`] whose behaviour is [`Activity`]) with the activity's fields, its
//! enter/leave hooks, region bases over native instances, and the cutscene
//! plugin's start, leave, stop/end and its StartPulse and EndPulse bodies.
use super::{Body, Error, Host, Level, Result, copy, is_player, location_of};
use crate::player::attributes::AttributeValue;
use crate::pulses;
use crate::world::{
    doors::Location,
    dynamic_region,
    player_walk::NodeHandle,
    region, region_activity,
    zones::{self, Borders, RegionZone, Restriction, Zone, ZoneType},
};
use std::sync::{Arc, RwLock};

/// CutscenePlugin.TABS: the tabs a cutscene plugin removes by default.
pub const TABS: [i32; 9] = [0, 1, 2, 3, 4, 5, 6, 11, 12];
/// The dim overlay of the start and end pulses (`new Component(115)`).
pub const DIM_OVERLAY: i32 = 115;
/// `getLocks().lockMovement(1000000)`.
pub const MOVEMENT_LOCK: i32 = 1_000_000;

/// The members a content activity overrides. Each default is the
/// original's base body (ActivityPlugin's, or CutscenePlugin's where it
/// overrides). Content receives the plugin object (`this`) and calls back
/// into the base members of this module.
pub trait Content<H: Host>: Send + Sync + 'static {
    /// `newInstance(player)` (abstract): the content's new plugin object, or
    /// `None` for a null answer.
    fn new_instance(
        &self,
        host: &mut H,
        plugin: &Zone<H>,
        player: Option<&NodeHandle>,
    ) -> Result<Option<Zone<H>>>;
    /// `getSpawnLocation()` (abstract).
    fn spawn_location(&self, host: &mut H, core: &Arc<Core<H>>) -> Result<Option<Location>>;
    /// `register()`: ActivityPlugin's does nothing.
    fn register(&self, _host: &mut H, _plugin: &Zone<H>) -> Result<()> {
        Ok(())
    }
    /// `configure()`: MapZone's does nothing.
    fn configure(&self, _host: &mut H, _plugin: &Zone<H>) -> Result<()> {
        Ok(())
    }
    /// `start(player, login, args)`.
    fn start(
        &self,
        host: &mut H,
        plugin: &Zone<H>,
        player: &NodeHandle,
        login: bool,
        args: &[AttributeValue],
    ) -> Result<bool> {
        start_base(host, plugin, player, login, args)
    }
    /// `enter(entity)`.
    fn enter(&self, host: &mut H, plugin: &Zone<H>, entity: &NodeHandle) -> Result<bool> {
        enter_base(host, plugin, entity)
    }
    /// `leave(entity, logout)`.
    fn leave(
        &self,
        host: &mut H,
        plugin: &Zone<H>,
        entity: &NodeHandle,
        logout: bool,
    ) -> Result<bool> {
        leave_base(host, plugin, entity, logout)
    }
    /// `fireEvent(identifier, args)`: both classes answer null.
    fn fire_event(
        &self,
        _host: &mut H,
        _plugin: &Zone<H>,
        _identifier: &str,
    ) -> Result<Option<AttributeValue>> {
        Ok(None)
    }
    /// CutscenePlugin.open(): nothing.
    fn open(&self, _host: &mut H, _core: &Arc<Core<H>>) -> Result<()> {
        Ok(())
    }
    /// CutscenePlugin.fade(): nothing.
    fn fade(&self, _host: &mut H, _core: &Arc<Core<H>>) -> Result<()> {
        Ok(())
    }
    /// CutscenePlugin.getMapState(): 2.
    fn map_state(&self, _host: &mut H, _core: &Arc<Core<H>>) -> Result<i32> {
        Ok(2)
    }
    /// CutscenePlugin.getRemovedTabs(): [`TABS`].
    fn removed_tabs(&self, _host: &mut H, _core: &Arc<Core<H>>) -> Result<Vec<i32>> {
        Ok(TABS.to_vec())
    }
    /// CutscenePlugin.getStartLocation(): `getBase()`.
    fn start_location(&self, _host: &mut H, core: &Arc<Core<H>>) -> Result<Option<Location>> {
        Ok(core.base())
    }
}

/// ActivityPlugin's own fields.
pub struct Fields {
    pub instanced: bool,
    pub multicombat: bool,
    pub safe: bool,
    /// The protected `region` (a DynamicRegion).
    pub region: Option<region::Handle>,
    pub base: Option<Location>,
    /// `safeRespawn`: ServerConstants.HOME_LOCATION at construction.
    pub safe_respawn: Option<Location>,
    pub player: Option<NodeHandle>,
}
/// CutscenePlugin's own fields.
pub struct CutsceneFields {
    pub fade: bool,
    /// The protected `npcs` list (never filled by the framework).
    pub npcs: zones::List<NodeHandle>,
    /// The `startPulse` and `endPulse` objects and their `counter` fields.
    pub start_pulse: RwLock<Option<pulses::Handle>>,
    pub end_pulse: RwLock<Option<pulses::Handle>>,
    pub start_counter: RwLock<i32>,
    pub end_counter: RwLock<i32>,
}
/// The plugin object's state: its fields, the cutscene plugin's, and the
/// content class.
pub struct Core<H: Host> {
    pub fields: RwLock<Fields>,
    pub cutscene: Option<CutsceneFields>,
    pub content: Box<dyn Content<H>>,
}
impl<H: Host> Core<H> {
    fn read(&self) -> std::sync::RwLockReadGuard<'_, Fields> {
        self.fields.read().unwrap_or_else(|e| e.into_inner())
    }
    fn write(&self) -> std::sync::RwLockWriteGuard<'_, Fields> {
        self.fields.write().unwrap_or_else(|e| e.into_inner())
    }
    pub fn is_instanced(&self) -> bool {
        self.read().instanced
    }
    pub fn set_instanced(&self, value: bool) {
        self.write().instanced = value;
    }
    pub fn is_multicombat(&self) -> bool {
        self.read().multicombat
    }
    pub fn set_multicombat(&self, value: bool) {
        self.write().multicombat = value;
    }
    pub fn is_safe(&self) -> bool {
        self.read().safe
    }
    pub fn set_safe(&self, value: bool) {
        self.write().safe = value;
    }
    pub fn region(&self) -> Option<region::Handle> {
        self.read().region.clone()
    }
    /// The protected `region` field written by content.
    pub fn set_region(&self, region: Option<region::Handle>) {
        self.write().region = region;
    }
    pub fn base(&self) -> Option<Location> {
        self.read().base.clone()
    }
    pub fn set_base(&self, base: Option<Location>) {
        self.write().base = base;
    }
    pub fn safe_respawn(&self) -> Option<Location> {
        self.read().safe_respawn.clone()
    }
    pub fn set_safe_respawn(&self, location: Option<Location>) {
        self.write().safe_respawn = location;
    }
    pub fn player(&self) -> Option<NodeHandle> {
        self.read().player.clone()
    }
    pub fn is_fade(&self) -> Result<bool> {
        Ok(self.cutscene.as_ref().ok_or(Error::ClassCast)?.fade)
    }
    pub fn npcs(&self) -> Result<zones::List<NodeHandle>> {
        Ok(self.cutscene.as_ref().ok_or(Error::ClassCast)?.npcs.clone())
    }
    /// `getStartPulse()`: the plugin's StartPulse object.
    pub fn start_pulse(&self) -> Result<pulses::Handle> {
        let c = self.cutscene.as_ref().ok_or(Error::ClassCast)?;
        let p = c
            .start_pulse
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        p.ok_or(Error::Null)
    }
    /// The `endPulse` field.
    pub fn end_pulse(&self) -> Result<pulses::Handle> {
        let c = self.cutscene.as_ref().ok_or(Error::ClassCast)?;
        let p = c
            .end_pulse
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        p.ok_or(Error::Null)
    }
    /// The pulse objects the plugin's fields hold.
    pub(crate) fn held_pulses(&self) -> Vec<pulses::Handle> {
        let Some(c) = &self.cutscene else {
            return Vec::new();
        };
        [&c.start_pulse, &c.end_pulse]
            .into_iter()
            .filter_map(|p| p.read().unwrap_or_else(|e| e.into_inner()).clone())
            .collect()
    }
    fn next(counter: &RwLock<i32>) -> i32 {
        let mut c = counter.write().unwrap_or_else(|e| e.into_inner());
        let value = *c;
        *c = c.wrapping_add(1);
        value
    }
    /// StartPulse's `switch (counter++)`.
    pub(crate) fn next_start_counter(&self) -> i32 {
        self.cutscene
            .as_ref()
            .map_or(0, |c| Self::next(&c.start_counter))
    }
    /// The plugin's EndPulse's `switch (counter++)`.
    pub(crate) fn next_end_counter(&self) -> i32 {
        self.cutscene
            .as_ref()
            .map_or(0, |c| Self::next(&c.end_counter))
    }
}

/// The zone behaviour of an activity plugin.
pub struct Activity<H: Host> {
    pub core: Arc<Core<H>>,
}
impl<H: Host> zones::Behavior<H> for Activity<H> {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn enter(&self, zone: &Zone<H>, host: &mut H, entity: &NodeHandle) -> zones::Result<bool> {
        Ok(self.core.content.enter(host, zone, entity)?)
    }
    fn leave(
        &self,
        zone: &Zone<H>,
        host: &mut H,
        entity: &NodeHandle,
        logout: bool,
    ) -> zones::Result<bool> {
        Ok(self.core.content.leave(host, zone, entity, logout)?)
    }
    fn configure(&self, zone: &Zone<H>, host: &mut H) -> zones::Result<()> {
        Ok(self.core.content.configure(host, zone)?)
    }
}

/// The plugin's core: `ClassCast` when the zone is not an activity plugin.
pub fn core<H: Host>(plugin: &Zone<H>) -> Result<Arc<Core<H>>> {
    plugin
        .behavior::<Activity<H>>()
        .map(|a| a.core.clone())
        .ok_or(Error::ClassCast)
}

/// `new ActivityPlugin(name, instanced, multicombat, safe, restrictions...)`
/// for a content class: MapZone(name, true, RANDOM_EVENTS), the field
/// initialiser `safeRespawn = HOME_LOCATION`, then each restriction, the
/// flags and the SAFE zone type.
pub fn new_activity<H: Host>(
    host: &mut H,
    name: Option<&str>,
    instanced: bool,
    multicombat: bool,
    safe: bool,
    restrictions: &[Restriction],
    content: Box<dyn Content<H>>,
) -> Result<Zone<H>> {
    construct(
        host,
        name,
        instanced,
        multicombat,
        safe,
        restrictions,
        content,
        None,
    )
}
/// `new CutscenePlugin(name, fade)`: ActivityPlugin(name, true, false,
/// true), then the field initialisers: the npcs list, a StartPulse and an
/// EndPulse (each `super(1, player)` while `player` is still null).
pub fn new_cutscene<H: Host>(
    host: &mut H,
    name: Option<&str>,
    fade: bool,
    content: Box<dyn Content<H>>,
) -> Result<Zone<H>> {
    construct(host, name, true, false, true, &[], content, Some(fade))
}
#[allow(clippy::too_many_arguments)]
fn construct<H: Host>(
    host: &mut H,
    name: Option<&str>,
    instanced: bool,
    multicombat: bool,
    safe: bool,
    restrictions: &[Restriction],
    content: Box<dyn Content<H>>,
    fade: Option<bool>,
) -> Result<Zone<H>> {
    let home = host.home_location()?;
    let core = Arc::new(Core {
        fields: RwLock::new(Fields {
            instanced,
            multicombat,
            safe,
            region: None,
            base: None,
            safe_respawn: home,
            player: None,
        }),
        cutscene: fade.map(|fade| CutsceneFields {
            fade,
            npcs: zones::List::new(),
            start_pulse: RwLock::new(None),
            end_pulse: RwLock::new(None),
            start_counter: RwLock::new(0),
            end_counter: RwLock::new(0),
        }),
        content,
    });
    let zone = Zone::new(
        name,
        true,
        &[Restriction::RandomEvents],
        Activity { core: core.clone() },
    );
    for r in restrictions {
        zone.add_restriction(r.flag());
    }
    if safe {
        zone.set_zone_type(ZoneType::Safe.id());
    }
    if let Some(c) = &core.cutscene {
        let start = host
            .activity_pulses()
            .create(1, Some(vec![None]), Body::Start(core.clone()));
        *c.start_pulse.write().unwrap_or_else(|e| e.into_inner()) = Some(start);
        let end = host
            .activity_pulses()
            .create(1, Some(vec![None]), Body::End(core.clone(), None));
        *c.end_pulse.write().unwrap_or_else(|e| e.into_inner()) = Some(end);
    }
    Ok(zone)
}

/// `getEndPulse()`: a new EndPulse (`super(1, player)` with the current
/// player field).
pub fn new_end_pulse<H: Host>(host: &mut H, core: &Arc<Core<H>>) -> Result<pulses::Handle> {
    if core.cutscene.is_none() {
        return Err(Error::ClassCast);
    }
    let checks = vec![core.player().map(|p| p.node)];
    Ok(host
        .activity_pulses()
        .create(1, Some(checks), Body::End(core.clone(), Some(0))))
}

/// `region.getX()`, `region.getY()`.
fn region_xy(region: &region::Handle) -> (i32, i32) {
    let r = region.read();
    let d = r.data.read();
    (d.x, d.y)
}

/// ActivityPlugin.register(ZoneBorders): the multiway zone first when
/// multicombat, then MapZone.register.
pub fn register_borders<H: Host>(host: &mut H, plugin: &Zone<H>, borders: &Borders) -> Result<()> {
    let core = core(plugin)?;
    if core.is_multicombat() {
        let multiway = host.multiway_zone()?;
        multiway.register(host, borders)?;
    }
    plugin.register(host, borders)?;
    Ok(())
}

/// `setRegionBase()`: with a region, multicombat toggled on it and the base
/// at the south-west corner of its borders (plane 0).
pub fn set_region_base<H: Host>(host: &mut H, plugin: &Zone<H>) -> Result<()> {
    let core = core(plugin)?;
    let Some(region) = core.region() else {
        return Ok(());
    };
    if core.is_multicombat() {
        dynamic_region::toggle_multicombat(host, &region)?;
    }
    let borders = dynamic_region::borders(&region)?.ok_or(Error::Null)?;
    core.set_base(Some(Location::new(
        borders.south_west_x(),
        borders.south_west_y(),
        0,
    )));
    Ok(())
}

/// `setRegionBase(DynamicRegion[])`: the first region becomes the
/// activity's; the base location of the last region whose region x or y
/// exceeds the running location's tile x or y (the original compares the
/// two units) fixes the borders' north-east corner; one RegionZone of this
/// plugin (and one of the multiway zone when multicombat) is added to every
/// region's list directly, then the base is set.
pub fn set_region_base_regions<H: Host>(
    host: &mut H,
    plugin: &Zone<H>,
    regions: &[region::Handle],
) -> Result<()> {
    let core = core(plugin)?;
    let first = regions.first().ok_or(Error::Index)?.clone();
    core.set_region(Some(first.clone()));
    let mut l = dynamic_region::base_location(&first);
    for r in regions {
        let (x, y) = region_xy(r);
        let [lx, ly, _] = l.coordinates();
        if x > lx || y > ly {
            l = dynamic_region::base_location(r);
        }
    }
    let (rx, ry) = region_xy(&first);
    let [lx, ly, _] = l.coordinates();
    let borders = Borders::new(
        rx.wrapping_shl(6),
        ry.wrapping_shl(6),
        lx.wrapping_add(64),
        ly.wrapping_add(64),
    );
    let multicombat = core.is_multicombat();
    let multi_zone = if multicombat {
        Some(RegionZone::new(host.multiway_zone()?, borders.clone()))
    } else {
        None
    };
    let zone = RegionZone::new(plugin.clone(), borders.clone());
    for r in regions {
        if let Some(multi_zone) = &multi_zone {
            dynamic_region::set_multicombat(r, true)?;
            region_activity::region_zones::<H>(r)?.add(multi_zone.clone());
        }
        region_activity::region_zones::<H>(r)?.add(zone.clone());
    }
    core.set_base(Some(Location::new(
        borders.south_west_x(),
        borders.south_west_y(),
        0,
    )));
    Ok(())
}

/// ActivityPlugin.start: the player field, true.
pub fn activity_start<H: Host>(plugin: &Zone<H>, player: &NodeHandle) -> Result<bool> {
    core(plugin)?.write().player = Some(player.clone());
    Ok(true)
}
/// The class's own `start`: CutscenePlugin's for a cutscene plugin,
/// ActivityPlugin's otherwise (what `super.start` reaches from content).
pub fn start_base<H: Host>(
    host: &mut H,
    plugin: &Zone<H>,
    player: &NodeHandle,
    login: bool,
    args: &[AttributeValue],
) -> Result<bool> {
    let core = core(plugin)?;
    if core.cutscene.is_some() {
        cutscene_start(host, plugin, &core, player, login, args)
    } else {
        activity_start(plugin, player)
    }
}
/// `plugin.start(player, login, args)`: the content's override.
pub fn start<H: Host>(
    host: &mut H,
    plugin: &Zone<H>,
    player: &NodeHandle,
    login: bool,
    args: &[AttributeValue],
) -> Result<bool> {
    let core = core(plugin)?;
    core.content.start(host, plugin, player, login, args)
}

/// ActivityPlugin.enter: a player's spawn location from the content, the
/// safe flags, the "activity" attribute, then MapZone.enter.
pub fn activity_enter<H: Host>(
    host: &mut H,
    plugin: &Zone<H>,
    entity: &NodeHandle,
) -> Result<bool> {
    let core = core(plugin)?;
    if is_player(host, entity)? {
        if let Some(l) = core.content.spawn_location(host, &core)? {
            host.set_spawn_location(entity, Some(&l))?;
        }
    }
    host.set_safe_zone(entity, core.is_safe())?;
    host.set_safe_respawn(entity, core.safe_respawn().as_ref())?;
    host.set_attribute(
        entity,
        "activity",
        AttributeValue::Object(Arc::new(PluginRef(plugin.clone()))),
    )?;
    Ok(zones::base_enter(plugin, host, entity)?)
}
/// The base `enter` (CutscenePlugin does not override it).
pub fn enter_base<H: Host>(host: &mut H, plugin: &Zone<H>, entity: &NodeHandle) -> Result<bool> {
    activity_enter(host, plugin, entity)
}
/// ActivityPlugin.leave: a player's spawn location back home, an instance's
/// spawn location on logout, the safe flags cleared and the attribute
/// removed; MapZone.leave answers true.
pub fn activity_leave<H: Host>(
    host: &mut H,
    plugin: &Zone<H>,
    entity: &NodeHandle,
    logout: bool,
) -> Result<bool> {
    let core = core(plugin)?;
    if is_player(host, entity)? {
        let home = host.home_location()?;
        host.set_spawn_location(entity, home.as_ref())?;
    }
    if core.is_instanced() && logout {
        if let Some(l) = core.content.spawn_location(host, &core)? {
            host.set_location(entity, &l)?;
        }
    }
    host.set_safe_zone(entity, false)?;
    let home = host.home_location()?;
    host.set_safe_respawn(entity, home.as_ref())?;
    host.remove_attribute(entity, "activity")?;
    Ok(true)
}
/// The class's own `leave`: CutscenePlugin's for a cutscene plugin.
pub fn leave_base<H: Host>(
    host: &mut H,
    plugin: &Zone<H>,
    entity: &NodeHandle,
    logout: bool,
) -> Result<bool> {
    let core = core(plugin)?;
    if core.cutscene.is_some() {
        cutscene_leave(host, plugin, &core, entity, logout)
    } else {
        activity_leave(host, plugin, entity, logout)
    }
}

/// The "activity" attribute's value: the plugin object.
pub struct PluginRef<H: Host>(pub Zone<H>);
impl<H: Host> std::fmt::Display for PluginRef<H> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.name().unwrap_or_else(|| "null".into()))
    }
}

// ---------------------------------------------------------------- CutscenePlugin

/// CutscenePlugin.start: the original location stored twice ("real-end"
/// removed first), then the fade-in pulse submitted or, without a fade,
/// everything it would do at once; then the lock and ActivityPlugin.start.
pub fn cutscene_start<H: Host>(
    host: &mut H,
    plugin: &Zone<H>,
    core: &Arc<Core<H>>,
    player: &NodeHandle,
    _login: bool,
    _args: &[AttributeValue],
) -> Result<bool> {
    let here = location_of(host, player)?;
    host.set_attribute(
        player,
        "cutscene:original-loc",
        AttributeValue::Location(here),
    )?;
    host.remove_attribute(player, "real-end")?;
    let here = location_of(host, player)?;
    host.set_attribute(player, "real-end", AttributeValue::Location(here))?;
    if core.is_fade()? {
        let pulse = core.start_pulse()?;
        host.submit_pulse(&pulse)?;
    } else {
        let state = core.content.map_state(host, core)?;
        host.minimap_state(player, state)?;
        let tabs = core.content.removed_tabs(host, core)?;
        host.remove_tabs(player, &tabs)?;
        let start = core.content.start_location(host, core)?;
        host.set_teleport_location(player, start.as_ref())?;
        host.unlock(player)?;
        host.walking_reset(player)?;
        host.lock_movement(player, MOVEMENT_LOCK)?;
        host.close_interfaces(player)?;
        core.content.open(host, core)?;
    }
    host.lock(player)?;
    activity_start(plugin, player)
}

/// CutscenePlugin.leave: with a started player, a logout puts the player
/// back at the stored original location and ends the cutscene, anything
/// else fades it out (`unpause`); then the player is unlocked, the default
/// tabs opened, the walking queue reset and movement unlocked; then
/// ActivityPlugin.leave.
pub fn cutscene_leave<H: Host>(
    host: &mut H,
    plugin: &Zone<H>,
    core: &Arc<Core<H>>,
    entity: &NodeHandle,
    logout: bool,
) -> Result<bool> {
    if let Some(player) = core.player() {
        if logout {
            let here = location_of(host, &player)?;
            let back = match host.get_attribute(&player, "cutscene:original-loc")? {
                None => here,
                Some(AttributeValue::Location(l)) => l,
                Some(_) => return Err(Error::ClassCast),
            };
            host.set_location(&player, &back)?;
            end(host, core)?;
        } else {
            unpause(host, core)?;
        }
        host.unlock(&player)?;
        host.open_default_tabs(&player)?;
        host.walking_reset(&player)?;
        host.unlock_movement(&player)?;
    }
    activity_leave(host, plugin, entity, logout)
}

/// `stop(fade)`: the plugin's EndPulse submitted, or `end()` at once.
pub fn stop<H: Host>(host: &mut H, core: &Arc<Core<H>>, fade: bool) -> Result<()> {
    if fade {
        let pulse = core.end_pulse()?;
        host.submit_pulse(&pulse)?;
        Ok(())
    } else {
        end(host, core)
    }
}
/// `unpause()`: `stop(true)`.
pub fn unpause<H: Host>(host: &mut H, core: &Arc<Core<H>>) -> Result<()> {
    stop(host, core, true)
}
fn player_of<H: Host>(core: &Arc<Core<H>>) -> Result<NodeHandle> {
    core.player().ok_or(Error::Null)
}
/// `end()`: every NPC of the instance cleared (planes 0 to 3, each list
/// assembled first), the minimap back to 0, the tabs restored, the player
/// unlocked and the walking queue reset.
pub fn end<H: Host>(host: &mut H, core: &Arc<Core<H>>) -> Result<()> {
    if let Some(region) = core.region() {
        for z in 0..dynamic_region::PLANES {
            for npc in dynamic_region::assemble_npc_list(&region, z)? {
                let npc = npc.ok_or(Error::Null)?;
                host.clear_npc(&npc)?;
            }
        }
    }
    let player = player_of(core)?;
    host.minimap_state(&player, 0)?;
    host.restore_tabs(&player)?;
    host.unlock(&player)?;
    host.walking_reset(&player)?;
    Ok(())
}

/// StartPulse.pulse(): `switch (counter++)`: 1 locks and dims, 3 sends the
/// map state and removes the tabs, 4 teleports to the start location, 5
/// undims, closes, unlocks, resets, locks movement and opens.
pub(crate) fn start_pulse<H: Host>(host: &mut H, core: &Arc<Core<H>>) -> Result<bool> {
    match core.next_start_counter() {
        1 => {
            let player = player_of(core)?;
            host.lock(&player)?;
            host.open_overlay(&player, DIM_OVERLAY)?;
        }
        3 => {
            let state = core.content.map_state(host, core)?;
            let player = player_of(core)?;
            host.minimap_state(&player, state)?;
            let tabs = core.content.removed_tabs(host, core)?;
            host.remove_tabs(&player, &tabs)?;
        }
        4 => {
            let player = player_of(core)?;
            let start = core.content.start_location(host, core)?;
            host.set_teleport_location(&player, start.as_ref())?;
        }
        5 => {
            let player = player_of(core)?;
            host.close_overlay(&player)?;
            host.close_interfaces(&player)?;
            host.unlock(&player)?;
            host.walking_reset(&player)?;
            host.lock_movement(&player, MOVEMENT_LOCK)?;
            core.content.open(host, core)?;
            return Ok(true);
        }
        _ => {}
    }
    Ok(false)
}

/// EndPulse.pulse(): 1 locks and dims, 3 sends the map state and removes
/// the tabs, 4 teleports to "real-end" (else the original location, else
/// where the player stands; one cast of the value read), 5 ends, stops this
/// pulse, fades, resets the minimap and closes while the session is active,
/// undims and clears the safe zone.
pub(crate) fn end_pulse<H: Host>(
    host: &mut H,
    core: &Arc<Core<H>>,
    pulse: &pulses::Handle,
    counter: i32,
) -> Result<bool> {
    match counter {
        1 => {
            let player = player_of(core)?;
            host.lock(&player)?;
            host.open_overlay(&player, DIM_OVERLAY)?;
        }
        3 => {
            let state = core.content.map_state(host, core)?;
            let player = player_of(core)?;
            host.minimap_state(&player, state)?;
            let tabs = core.content.removed_tabs(host, core)?;
            host.remove_tabs(&player, &tabs)?;
        }
        4 => {
            let player = player_of(core)?;
            let here = location_of(host, &player)?;
            let original = host.get_attribute(&player, "cutscene:original-loc")?;
            let real = host.get_attribute(&player, "real-end")?;
            let value = real.or(original);
            let loc = match value {
                None => Some(copy(&here)),
                Some(AttributeValue::Location(l)) => Some(l),
                Some(_) => return Err(Error::ClassCast),
            };
            host.set_teleport_location(&player, loc.as_ref())?;
        }
        5 => {
            end(host, core)?;
            pulses::stop_base(pulse);
            core.content.fade(host, core)?;
            let player = player_of(core)?;
            if host.session_active(&player)? {
                host.minimap_state(&player, 0)?;
            }
            host.close_overlay(&player)?;
            if host.session_active(&player)? {
                host.close_interfaces(&player)?;
            }
            host.set_safe_zone(&player, false)?;
            return Ok(true);
        }
        _ => {}
    }
    Ok(false)
}

/// Writes the log line of a failing activity start.
pub(crate) fn log_class() -> &'static str {
    "core.game.activity.ActivityManager"
}
pub(crate) fn log_unhandled<H: Host>(host: &mut H, name: Option<&str>) -> Result<()> {
    host.log(
        log_class(),
        Level::Err,
        &format!("Unhandled activity - {}!", name.unwrap_or("null")),
    )
}
