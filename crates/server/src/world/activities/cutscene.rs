//! `core.game.activity.Cutscene` (Kotlin): a cutscene's region and base,
//! its stages driven by the "cutscene:stage" attribute, dialogue updates
//! whose continue action advances the stage, camera, NPC and teleport
//! helpers, the timed and ending pulses, the death hook and the logout
//! listener.
use super::{Body, Error, Expression, Host, Level, Result, copy, location_of, transform};
use crate::player::attributes::AttributeValue;
use crate::world::{
    doors::{Handle as Scenery, Location},
    dynamic_region,
    force_movement::Direction,
    player_walk::NodeHandle,
    region, zones,
};
use std::sync::{Arc, RwLock};

/// `Cutscene.ATTRIBUTE_CUTSCENE`.
pub const ATTRIBUTE_CUTSCENE: &str = "cutscene";
/// `Cutscene.ATTRIBUTE_CUTSCENE_STAGE`.
pub const ATTRIBUTE_CUTSCENE_STAGE: &str = "cutscene:stage";
/// `Components.FADE_TO_BLACK_120`.
pub const FADE_TO_BLACK: i32 = 120;
/// `Components.FADE_FROM_BLACK_170`.
pub const FADE_FROM_BLACK: i32 = 170;
/// The tabs `start` removes.
pub const START_TABS: [i32; 15] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14];

/// A content cutscene: the two abstract members, the content's own
/// callbacks (continue and end actions it passes as lambdas) and its class.
pub trait Script<H: Host>: Send + Sync + 'static {
    /// `setup()`.
    fn setup(&self, host: &mut H, cutscene: &Cutscene<H>) -> Result<()>;
    /// `runStage(stage)`.
    fn run_stage(&self, host: &mut H, cutscene: &Cutscene<H>, stage: i32) -> Result<()>;
    /// A lambda the content passed as `onContinue` or `endActions`.
    fn callback(&self, host: &mut H, cutscene: &Cutscene<H>, id: u32) -> Result<()>;
    /// `this::class.java`: the content class's binary name.
    fn class_name(&self) -> &str;
}

/// `addedNPCs`: a `HashMap<Int, ArrayList<NPC>>` whose iteration order
/// (`clearNPCs`) is Java's: bucket `(h ^ h >>> 16) & (capacity - 1)`, chains
/// in insertion order (splits keep it), the table kept by `clear()`.
pub struct NpcMap {
    capacity: usize,
    entries: Vec<(i32, Vec<NodeHandle>)>,
}
/// A native bound: a bin that would become a tree bin (eight colliding ids
/// in a table of 64 or more) is refused.
pub const TREEIFY_THRESHOLD: usize = 8;
impl NpcMap {
    fn new() -> Self {
        Self {
            capacity: 0,
            entries: Vec::new(),
        }
    }
    fn bucket(&self, key: i32) -> usize {
        let h = key as u32;
        ((h ^ (h >> 16)) as usize) & (self.capacity - 1)
    }
    pub fn get(&self, key: i32) -> Option<&Vec<NodeHandle>> {
        self.entries.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
    }
    fn put(&mut self, key: i32, value: Vec<NodeHandle>) -> Result<()> {
        if let Some(entry) = self.entries.iter_mut().find(|(k, _)| *k == key) {
            entry.1 = value;
            return Ok(());
        }
        if self.capacity == 0 {
            self.capacity = 16;
        }
        let bucket = self.bucket(key);
        let bin = self
            .entries
            .iter()
            .filter(|(k, _)| self.bucket(*k) == bucket)
            .count();
        self.entries.push((key, value));
        if bin >= TREEIFY_THRESHOLD {
            if self.capacity < 64 {
                self.capacity *= 2;
            } else {
                return Err(Error::Binding(
                    "a tree bin of addedNPCs (not ported)".into(),
                ));
            }
        }
        if self.entries.len() > self.capacity * 3 / 4 {
            self.capacity *= 2;
        }
        Ok(())
    }
    /// The entries in the map's iteration order.
    pub fn entries(&self) -> Vec<(i32, Vec<NodeHandle>)> {
        if self.capacity == 0 {
            return Vec::new();
        }
        let mut order: Vec<usize> = (0..self.entries.len()).collect();
        order.sort_by_key(|&i| (self.bucket(self.entries[i].0), i));
        order.into_iter().map(|i| self.entries[i].clone()).collect()
    }
    fn clear(&mut self) {
        self.entries.clear();
    }
}

/// A cutscene's fields.
pub struct Fields {
    /// `lateinit var region`.
    pub region: Option<region::Handle>,
    /// `lateinit var base`.
    pub base: Option<Location>,
    pub exit_location: Location,
    pub ended: bool,
    pub added_npcs: NpcMap,
}
pub struct Object<H: Host> {
    pub player: NodeHandle,
    pub fields: RwLock<Fields>,
    pub script: Box<dyn Script<H>>,
}
/// A Cutscene object.
pub struct Cutscene<H: Host>(Arc<Object<H>>);
impl<H: Host> Clone for Cutscene<H> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<H: Host> Cutscene<H> {
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn identity(&self) -> usize {
        Arc::as_ptr(&self.0) as *const () as usize
    }
    pub fn player(&self) -> &NodeHandle {
        &self.0.player
    }
    pub fn script(&self) -> &dyn Script<H> {
        &*self.0.script
    }
    fn read(&self) -> std::sync::RwLockReadGuard<'_, Fields> {
        self.0.fields.read().unwrap_or_else(|e| e.into_inner())
    }
    fn write(&self) -> std::sync::RwLockWriteGuard<'_, Fields> {
        self.0.fields.write().unwrap_or_else(|e| e.into_inner())
    }
    pub fn ended(&self) -> bool {
        self.read().ended
    }
    pub fn exit_location(&self) -> Location {
        self.read().exit_location.clone()
    }
    /// The `region` backing field (null while uninitialised).
    pub fn region_field(&self) -> Option<region::Handle> {
        self.read().region.clone()
    }
    /// The `base` backing field (null while uninitialised).
    pub fn base_field(&self) -> Option<Location> {
        self.read().base.clone()
    }
    /// `base`: a lateinit read.
    pub fn base(&self) -> Result<Location> {
        self.base_field().ok_or(Error::Uninitialized("base"))
    }
    /// `region`: a lateinit read.
    pub fn region(&self) -> Result<region::Handle> {
        self.region_field().ok_or(Error::Uninitialized("region"))
    }
    /// The `addedNPCs` entries in iteration order.
    pub fn added_npcs(&self) -> Vec<(i32, Vec<NodeHandle>)> {
        self.read().added_npcs.entries()
    }
}

/// A dialogue's continue action: `{ _, _ -> onContinue.invoke() }` with the
/// default `{ incrementStage() }` or a content lambda.
pub enum Continue<H: Host> {
    Increment(Cutscene<H>),
    Content(Cutscene<H>, u32),
}
impl<H: Host> Clone for Continue<H> {
    fn clone(&self) -> Self {
        match self {
            Self::Increment(c) => Self::Increment(c.clone()),
            Self::Content(c, id) => Self::Content(c.clone(), *id),
        }
    }
}
/// The `endActions` lambda of `end`/`endWithoutFade`.
pub struct EndActions<H: Host>(pub Cutscene<H>, pub u32);
impl<H: Host> Clone for EndActions<H> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1)
    }
}

/// The "cutscene" attribute's value: the cutscene object.
pub struct CutsceneRef<H: Host>(pub Cutscene<H>);
impl<H: Host> std::fmt::Display for CutsceneRef<H> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.script().class_name())
    }
}

/// `Cutscene(player)`: `exitLocation` is a copy of the player's location,
/// then the PlayerCamera is constructed.
pub fn new<H: Host>(
    host: &mut H,
    player: &NodeHandle,
    script: Box<dyn Script<H>>,
) -> Result<Cutscene<H>> {
    let here = location_of(host, player)?;
    let exit = transform(&here, 0, 0, 0);
    host.camera_new(player)?;
    Ok(Cutscene(Arc::new(Object {
        player: player.clone(),
        fields: RwLock::new(Fields {
            region: None,
            base: None,
            exit_location: exit,
            ended: false,
            added_npcs: NpcMap::new(),
        }),
        script,
    })))
}

/// `player.getCutsceneStage()`: the stage attribute, 0 when absent.
pub fn stage<H: Host>(host: &mut H, player: &NodeHandle) -> Result<i32> {
    match host.get_attribute(player, ATTRIBUTE_CUTSCENE_STAGE)? {
        None => Ok(0),
        Some(AttributeValue::Int(stage)) => Ok(stage),
        Some(_) => Err(Error::ClassCast),
    }
}
/// `player.getCutscene()`: the cutscene attribute, null when absent.
pub fn get_cutscene<H: Host>(host: &mut H, player: &NodeHandle) -> Result<Option<Cutscene<H>>> {
    match host.get_attribute(player, ATTRIBUTE_CUTSCENE)? {
        None => Ok(None),
        Some(AttributeValue::Object(o)) => o
            .as_any()
            .downcast_ref::<CutsceneRef<H>>()
            .map(|r| Some(r.0.clone()))
            .ok_or(Error::ClassCast),
        Some(_) => Err(Error::ClassCast),
    }
}

/// `logCutscene(message)`: logged at FINE with the content's class when
/// ServerConstants.LOG_CUTSCENE is set.
pub fn log_cutscene<H: Host>(host: &mut H, cutscene: &Cutscene<H>, message: &str) -> Result<()> {
    if host.log_cutscene()? {
        let class = cutscene.script().class_name().to_owned();
        host.log(&class, Level::Fine, message)?;
    }
    Ok(())
}
fn username<H: Host>(host: &mut H, cutscene: &Cutscene<H>) -> Result<String> {
    host.username(cutscene.player())
}

/// `loadRegion(regionId)`: the cutscene's NPCs cleared, a new instance of
/// the region (DynamicRegion.create) and its base location.
pub fn load_region<H: Host>(host: &mut H, cutscene: &Cutscene<H>, region_id: i32) -> Result<()> {
    clear_npcs(host, cutscene)?;
    let name = username(host, cutscene)?;
    log_cutscene(
        host,
        cutscene,
        &format!("Creating new instance of region {region_id} for {name}."),
    )?;
    let region = host.create_instance(region_id)?;
    cutscene.write().region = Some(region.clone());
    let name = username(host, cutscene)?;
    let base = dynamic_region::base_location(&region);
    log_cutscene(
        host,
        cutscene,
        &format!("Dynamic region instantiated for {name}. Global coordinates: {base}."),
    )?;
    cutscene.write().base = Some(dynamic_region::base_location(&region));
    Ok(())
}
/// `closeOverlay()`.
pub fn close_overlay<H: Host>(host: &mut H, cutscene: &Cutscene<H>) -> Result<()> {
    let name = username(host, cutscene)?;
    log_cutscene(host, cutscene, &format!("Close {name}'s overlay."))?;
    host.close_overlay(cutscene.player())
}
/// `fadeToBlack()`.
pub fn fade_to_black<H: Host>(host: &mut H, cutscene: &Cutscene<H>) -> Result<()> {
    let name = username(host, cutscene)?;
    log_cutscene(host, cutscene, &format!("Fading {name}'s screen to black."))?;
    host.close_overlay(cutscene.player())?;
    host.open_overlay(cutscene.player(), FADE_TO_BLACK)
}
/// `fadeFromBlack()`.
pub fn fade_from_black<H: Host>(host: &mut H, cutscene: &Cutscene<H>) -> Result<()> {
    let name = username(host, cutscene)?;
    log_cutscene(
        host,
        cutscene,
        &format!("Fading {name}'s screen from black to normal."),
    )?;
    host.close_overlay(cutscene.player())?;
    host.open_overlay(cutscene.player(), FADE_FROM_BLACK)
}
/// `teleport(entity, regionX, regionY, plane)`.
pub fn teleport<H: Host>(
    host: &mut H,
    cutscene: &Cutscene<H>,
    entity: &NodeHandle,
    x: i32,
    y: i32,
    plane: i32,
) -> Result<()> {
    let to = transform(&cutscene.base()?, x, y, plane);
    let name = host.username(entity)?;
    let [gx, gy, _] = to.coordinates();
    log_cutscene(
        host,
        cutscene,
        &format!(
            "Teleporting {name} to coordinates: LOCAL[{x},{y},{plane}] GLOBAL[{gx},{gy},{plane}]."
        ),
    )?;
    host.set_teleport_location(entity, Some(&to))
}
/// `move(entity, regionX, regionY)`: a movement pulse whose arrival does
/// nothing.
pub fn move_entity<H: Host>(
    host: &mut H,
    cutscene: &Cutscene<H>,
    entity: &NodeHandle,
    x: i32,
    y: i32,
) -> Result<()> {
    let name = host.username(entity)?;
    log_cutscene(host, cutscene, &format!("Moving {name} to LOCAL[{x},{y}]."))?;
    let to = transform(&cutscene.base()?, x, y, 0);
    host.run_movement(entity, &to)
}

fn add_action<H: Host>(
    host: &mut H,
    cutscene: &Cutscene<H>,
    on_continue: Option<u32>,
) -> Result<()> {
    let action = match on_continue {
        None => Continue::Increment(cutscene.clone()),
        Some(id) => Continue::Content(cutscene.clone(), id),
    };
    host.add_dialogue_action(cutscene.player(), action)
}
/// `dialogueUpdate(npcId, expression, message, onContinue, hide)`;
/// `on_continue` names a content lambda, `None` the default increment.
pub fn npc_dialogue<H: Host>(
    host: &mut H,
    cutscene: &Cutscene<H>,
    npc: i32,
    expression: Expression,
    message: &str,
    on_continue: Option<u32>,
    hide: bool,
) -> Result<()> {
    log_cutscene(host, cutscene, "Sending NPC dialogue update.")?;
    host.send_npc_dialogue(cutscene.player(), npc, message, expression, hide)?;
    add_action(host, cutscene, on_continue)
}
/// `dialogueLinesUpdate(npcId, expression, messages..., onContinue)`.
pub fn npc_dialogue_lines<H: Host>(
    host: &mut H,
    cutscene: &Cutscene<H>,
    npc: i32,
    expression: Expression,
    lines: &[String],
    on_continue: Option<u32>,
) -> Result<()> {
    log_cutscene(host, cutscene, "Sending NPC dialogue lines update.")?;
    host.send_npc_dialogue_lines(cutscene.player(), npc, expression, true, lines)?;
    add_action(host, cutscene, on_continue)
}
/// `dialogueClose()`.
pub fn dialogue_close<H: Host>(host: &mut H, cutscene: &Cutscene<H>) -> Result<()> {
    log_cutscene(host, cutscene, "Sending dialogue close.")?;
    host.close_dialogue(cutscene.player())
}
/// `dialogueUpdate(message, onContinue)`.
pub fn dialogue<H: Host>(
    host: &mut H,
    cutscene: &Cutscene<H>,
    message: &str,
    on_continue: Option<u32>,
) -> Result<()> {
    log_cutscene(host, cutscene, "Sending standard dialogue update.")?;
    host.send_dialogue(cutscene.player(), message)?;
    add_action(host, cutscene, on_continue)
}
/// `dialogueLinesUpdate(messages..., onContinue)`.
pub fn dialogue_lines<H: Host>(
    host: &mut H,
    cutscene: &Cutscene<H>,
    lines: &[String],
    on_continue: Option<u32>,
) -> Result<()> {
    log_cutscene(host, cutscene, "Sending standard dialogue lines update.")?;
    host.send_dialogue_lines(cutscene.player(), lines)?;
    add_action(host, cutscene, on_continue)
}
/// `playerDialogueUpdate(expression, message, onContinue)`.
pub fn player_dialogue<H: Host>(
    host: &mut H,
    cutscene: &Cutscene<H>,
    expression: Expression,
    message: &str,
    on_continue: Option<u32>,
) -> Result<()> {
    log_cutscene(host, cutscene, "Sending player dialogue update")?;
    host.send_player_dialogue(cutscene.player(), message, expression, false)?;
    add_action(host, cutscene, on_continue)
}
/// `timedUpdate(ticks, newStage)`: a pulse of `ticks` delay that increments
/// the stage (`newStage == -1`) or sets it.
pub fn timed_update<H: Host>(
    host: &mut H,
    cutscene: &Cutscene<H>,
    ticks: i32,
    new_stage: i32,
) -> Result<()> {
    log_cutscene(
        host,
        cutscene,
        &format!("Executing timed updated for {ticks} ticks."),
    )?;
    let pulse =
        host.activity_pulses()
            .create(ticks, None, Body::Timed(cutscene.clone(), new_stage));
    host.submit_pulse(&pulse)?;
    Ok(())
}
pub(crate) fn timed_pulse<H: Host>(
    host: &mut H,
    cutscene: &Cutscene<H>,
    new_stage: i32,
) -> Result<bool> {
    if new_stage == -1 {
        increment_stage(host, cutscene)?;
    } else {
        update_stage(host, cutscene, new_stage)?;
    }
    Ok(true)
}
/// `getNPC(id)`.
pub fn get_npc<H: Host>(cutscene: &Cutscene<H>, id: i32) -> Option<NodeHandle> {
    cutscene
        .read()
        .added_npcs
        .get(id)
        .and_then(|l| l.first().cloned())
}
/// `getNPCs(id)`.
pub fn get_npcs<H: Host>(cutscene: &Cutscene<H>, id: i32) -> Vec<NodeHandle> {
    cutscene
        .read()
        .added_npcs
        .get(id)
        .cloned()
        .unwrap_or_default()
}
/// `getObject(regionX, regionY, plane)`.
pub fn get_object<H: Host>(
    host: &mut H,
    cutscene: &Cutscene<H>,
    x: i32,
    y: i32,
    plane: i32,
) -> Result<Option<Scenery>> {
    let at = transform(&cutscene.base()?, x, y, plane);
    let object = host.scenery(&at)?;
    let name = match &object {
        Some(o) => host.scenery_name(o)?,
        None => None,
    };
    log_cutscene(
        host,
        cutscene,
        &format!(
            "Retrieving object at LOCAL[{x},{y}], GOT: {}.",
            name.as_deref().unwrap_or("null")
        ),
    )?;
    Ok(object)
}
fn steps(direction: Direction) -> (i32, i32) {
    match direction {
        Direction::NorthWest => (-1, 1),
        Direction::North => (0, 1),
        Direction::NorthEast => (1, 1),
        Direction::West => (-1, 0),
        Direction::East => (1, 0),
        Direction::SouthWest => (-1, -1),
        Direction::South => (0, -1),
        Direction::SouthEast => (1, -1),
    }
}
/// `addNPC(id, regionX, regionY, direction, plane)`: a new NPC that neither
/// respawns, aggresses nor walks, placed, initialised and facing one step in
/// the direction, appended to the id's list.
pub fn add_npc<H: Host>(
    host: &mut H,
    cutscene: &Cutscene<H>,
    id: i32,
    x: i32,
    y: i32,
    direction: Direction,
    plane: i32,
) -> Result<()> {
    let npc = host.npc_new(id)?;
    host.npc_set_respawn(&npc, false)?;
    host.npc_set_aggressive(&npc, false)?;
    host.npc_set_walks(&npc, false)?;
    let at = transform(&cutscene.base()?, x, y, plane);
    host.set_location(&npc, &at)?;
    host.npc_init(&npc)?;
    let here = location_of(host, &npc)?;
    let (dx, dy) = steps(direction);
    let [hx, hy, hz] = here.coordinates();
    host.face_location(
        &npc,
        &Location::new(hx.wrapping_add(dx), hy.wrapping_add(dy), hz),
    )?;
    {
        let mut f = cutscene.write();
        let mut list = f.added_npcs.get(id).cloned().unwrap_or_default();
        list.push(npc.clone());
        f.added_npcs.put(id, list)?;
    }
    let [gx, gy, _] = location_of(host, &npc)?.coordinates();
    log_cutscene(
        host,
        cutscene,
        &format!("Added NPC {id} at location LOCAL[{x},{y},{plane}] GLOBAL[{gx},{gy},{plane}]"),
    )
}

/// `start(hideMiniMap)`: the player's own region and its base, the content
/// set up, the minimap hidden, the stored stage run, the cutscene
/// attributes, the tabs removed, the safe flags, the lock, the death hook,
/// the logout listener and the AntiMacro pause.
pub fn start<H: Host>(host: &mut H, cutscene: &Cutscene<H>, hide_minimap: bool) -> Result<()> {
    let player = cutscene.player().clone();
    let name = username(host, cutscene)?;
    log_cutscene(host, cutscene, &format!("Starting cutscene for {name}."))?;
    let here = location_of(host, &player)?;
    let region = zones::Host::region(host, zones::region_id(&here))?;
    cutscene.write().region = Some(region);
    let here = location_of(host, &player)?;
    let region = zones::Host::region(host, zones::region_id(&here))?;
    cutscene.write().base = Some(dynamic_region::base_location(&region));
    cutscene.script().setup(host, cutscene)?;
    if hide_minimap {
        host.minimap_state(&player, 2)?;
    }
    let stage = stage(host, &player)?;
    cutscene.script().run_stage(host, cutscene, stage)?;
    host.set_attribute(
        &player,
        ATTRIBUTE_CUTSCENE,
        AttributeValue::Object(Arc::new(CutsceneRef(cutscene.clone()))),
    )?;
    host.set_attribute(&player, ATTRIBUTE_CUTSCENE_STAGE, AttributeValue::Int(0))?;
    host.remove_tabs(&player, &START_TABS)?;
    host.set_safe_zone(&player, true)?;
    let here = location_of(host, &player)?;
    host.set_safe_respawn(&player, Some(&here))?;
    host.lock(&player)?;
    host.hook_self_death(&player)?;
    host.put_logout_listener(&player, ATTRIBUTE_CUTSCENE, cutscene.clone())?;
    host.anti_macro_pause(&player)
}

/// `endWithoutFade(endActions)`: ended, then a pulse that teleports to the
/// exit on its first run and finishes on its second.
pub fn end_without_fade<H: Host>(
    host: &mut H,
    cutscene: &Cutscene<H>,
    actions: Option<u32>,
) -> Result<()> {
    cutscene.write().ended = true;
    let body = Body::End2 {
        cutscene: cutscene.clone(),
        fade: None,
        tick: 0,
        actions: actions.map(|id| EndActions(cutscene.clone(), id)),
    };
    let pulse = host.activity_pulses().create(1, None, body);
    host.submit_pulse(&pulse)?;
    Ok(())
}
/// `end(fade, endActions)`: ended, faded to black when fading, then a pulse
/// that (fading) runs nine times and teleports to the exit on the ninth,
/// where its `when` falls through to `return true` (the fade from black and
/// the sixteenth run are never reached), or (not fading) teleports and
/// finishes at once.
pub fn end<H: Host>(
    host: &mut H,
    cutscene: &Cutscene<H>,
    fade: bool,
    actions: Option<u32>,
) -> Result<()> {
    cutscene.write().ended = true;
    if fade {
        fade_to_black(host, cutscene)?;
    }
    let body = Body::End2 {
        cutscene: cutscene.clone(),
        fade: Some(fade),
        tick: 0,
        actions: actions.map(|id| EndActions(cutscene.clone(), id)),
    };
    let pulse = host.activity_pulses().create(1, None, body);
    host.submit_pulse(&pulse)?;
    Ok(())
}
/// The `pulse()` of the ending pulses; `tick` is the value `tick++` read.
pub(crate) fn end_pulse<H: Host>(
    host: &mut H,
    cutscene: &Cutscene<H>,
    fade: Option<bool>,
    tick: i32,
) -> Result<bool> {
    let player = cutscene.player().clone();
    match fade {
        // endWithoutFade
        None => match tick {
            0 => {
                let exit = cutscene.exit_location();
                host.set_teleport_location(&player, Some(&exit))?;
                Ok(false)
            }
            1 => Ok(true),
            _ => Ok(false),
        },
        Some(true) => match tick {
            8 => {
                let exit = cutscene.exit_location();
                host.set_teleport_location(&player, Some(&exit))?;
                Ok(true)
            }
            9 => {
                fade_from_black(host, cutscene)?;
                Ok(true)
            }
            16 => Ok(true),
            _ => Ok(false),
        },
        Some(false) => {
            let exit = cutscene.exit_location();
            host.set_teleport_location(&player, Some(&exit))?;
            Ok(true)
        }
    }
}
/// The ending pulses' `stop()` override, after Pulse.stop: the cutscene
/// attributes removed, the safe flags back home, the tabs restored, the
/// player unlocked, the NPCs cleared, the hook and listener removed, the
/// AntiMacro unpaused, the minimap shown, then the end actions (a failure
/// is logged with the anonymous pulse class's empty simple name and
/// printed).
pub(crate) fn end_stop<H: Host>(
    host: &mut H,
    cutscene: &Cutscene<H>,
    class: &str,
    actions: Option<&EndActions<H>>,
) -> Result<()> {
    let player = cutscene.player().clone();
    host.remove_attribute(&player, ATTRIBUTE_CUTSCENE)?;
    host.remove_attribute(&player, ATTRIBUTE_CUTSCENE_STAGE)?;
    host.set_safe_zone(&player, false)?;
    let home = host.home_location()?;
    host.set_safe_respawn(&player, home.as_ref())?;
    host.restore_tabs(&player)?;
    host.unlock(&player)?;
    clear_npcs(host, cutscene)?;
    host.unhook_self_death(&player)?;
    host.remove_logout_listener(&player, ATTRIBUTE_CUTSCENE)?;
    host.anti_macro_unpause(&player)?;
    host.minimap_state(&player, 0)?;
    if let Some(EndActions(c, id)) = actions {
        if let Err(error) = c.script().callback(host, c, *id) {
            host.log(
                class,
                Level::Err,
                "There's some bad nasty code in  end actions!",
            )?;
            host.print_stack_trace(&error)?;
        }
    }
    Ok(())
}
/// `moveCamera(regionX, regionY, height, speed)`.
pub fn move_camera<H: Host>(
    host: &mut H,
    cutscene: &Cutscene<H>,
    x: i32,
    y: i32,
    height: i32,
    speed: i32,
) -> Result<()> {
    let [gx, gy, _] = transform(&cutscene.base()?, x, y, 0).coordinates();
    host.camera_pan_to(cutscene.player(), gx, gy, height, speed)
}
/// `rotateCamera(regionX, regionY, height, speed)`.
pub fn rotate_camera<H: Host>(
    host: &mut H,
    cutscene: &Cutscene<H>,
    x: i32,
    y: i32,
    height: i32,
    speed: i32,
) -> Result<()> {
    let [gx, gy, _] = transform(&cutscene.base()?, x, y, 0).coordinates();
    host.camera_rotate_to(cutscene.player(), gx, gy, height, speed)
}
/// `rotateCameraBy(diffX, diffY, diffHeight, diffSpeed)`.
pub fn rotate_camera_by<H: Host>(
    host: &mut H,
    cutscene: &Cutscene<H>,
    x: i32,
    y: i32,
    height: i32,
    speed: i32,
) -> Result<()> {
    host.camera_rotate_by(cutscene.player(), x, y, height, speed)
}
/// `shakeCamera(type, jitter, amplitude, frequency, speed)`; `kind` is the
/// CameraShakeType ordinal (TRUCK, PEDESTAL, DOLLY, PAN, TILT).
pub fn shake_camera<H: Host>(
    host: &mut H,
    cutscene: &Cutscene<H>,
    kind: i32,
    jitter: i32,
    amplitude: i32,
    frequency: i32,
    speed: i32,
) -> Result<()> {
    host.camera_shake(cutscene.player(), kind, jitter, amplitude, frequency, speed)
}
/// `resetCamera()`.
pub fn reset_camera<H: Host>(host: &mut H, cutscene: &Cutscene<H>) -> Result<()> {
    host.camera_reset(cutscene.player())
}
/// `setExit(location)`.
pub fn set_exit<H: Host>(cutscene: &Cutscene<H>, location: &Location) {
    cutscene.write().exit_location = copy(location);
}
fn load_current_stage<H: Host>(host: &mut H, cutscene: &Cutscene<H>) -> Result<()> {
    if cutscene.ended() {
        return Ok(());
    }
    let stage = stage(host, cutscene.player())?;
    cutscene.script().run_stage(host, cutscene, stage)
}
/// `incrementStage()`.
pub fn increment_stage<H: Host>(host: &mut H, cutscene: &Cutscene<H>) -> Result<()> {
    let next = stage(host, cutscene.player())?.wrapping_add(1);
    host.set_attribute(
        cutscene.player(),
        ATTRIBUTE_CUTSCENE_STAGE,
        AttributeValue::Int(next),
    )?;
    load_current_stage(host, cutscene)
}
/// `updateStage(newStage)`.
pub fn update_stage<H: Host>(host: &mut H, cutscene: &Cutscene<H>, stage: i32) -> Result<()> {
    host.set_attribute(
        cutscene.player(),
        ATTRIBUTE_CUTSCENE_STAGE,
        AttributeValue::Int(stage),
    )?;
    load_current_stage(host, cutscene)
}
/// `clearNPCs()`: each id's NPCs cleared in the map's order, then the map
/// emptied (a failure leaves it as it was).
pub fn clear_npcs<H: Host>(host: &mut H, cutscene: &Cutscene<H>) -> Result<()> {
    for (id, npcs) in cutscene.added_npcs() {
        let name = username(host, cutscene)?;
        log_cutscene(
            host,
            cutscene,
            &format!("Clearing {} NPCs with ID {id} for {name}.", npcs.len()),
        )?;
        for npc in npcs {
            host.clear_npc(&npc)?;
        }
    }
    cutscene.write().added_npcs.clear();
    Ok(())
}

/// A dialogue continue: the action's lambda.
pub fn run_continue<H: Host>(host: &mut H, action: &Continue<H>) -> Result<()> {
    match action {
        Continue::Increment(c) => increment_stage(host, c),
        Continue::Content(c, id) => c.script().callback(host, c, *id),
    }
}
/// CUTSCENE_DEATH_HOOK.process: a player's cutscene ended (with the fade),
/// or the hook removed when there is none.
pub fn death_hook<H: Host>(host: &mut H, entity: &NodeHandle) -> Result<()> {
    if !super::is_player(host, entity)? {
        return Ok(());
    }
    match get_cutscene(host, entity)? {
        Some(c) => end(host, &c, true, None),
        None => host.unhook_self_death(entity),
    }
}
/// The "cutscene" logout listener of `cutscene`: the logging-out player put
/// at the exit location, then the player's current cutscene ended.
pub fn logout_listener<H: Host>(
    host: &mut H,
    cutscene: &Cutscene<H>,
    player: &NodeHandle,
) -> Result<()> {
    let exit = cutscene.exit_location();
    host.set_location(player, &exit)?;
    if let Some(c) = get_cutscene(host, player)? {
        end(host, &c, true, None)?;
    }
    Ok(())
}
