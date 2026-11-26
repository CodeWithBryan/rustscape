# Activities and cutscenes

`world/activities` ports the activity and cutscene framework of
`core.game.activity`: ActivityManager, ActivityPlugin, CutscenePlugin and the
Kotlin Cutscene utility. Content activities and cutscenes (Pest Control, the
quests' cutscenes, ...) are not ported; they are [`plugin::Content`] and
[`cutscene::Script`] implementations the framework calls.

- `manager.rs` is ActivityManager: the world's map of plugins by name
  (`Host::activities`; only `get` and `put` reach it, so its iteration order
  is never observed), `register` (the content's `register()`, the map entry,
  then `configure()` for a plugin that is not instanced) and `start`: an
  unknown name logs `Unhandled activity - <name>!` in development mode (null
  settings fail with Null, as the original's `getSettings().isDevMode()`);
  an instanced activity is replaced by the content's `newInstance(player)`
  and configured (the variable is reassigned first, so a failing configure
  reports the new instance and a null instance reports "null"); every
  failure is printed, reported to the player in development mode as
  `Error starting activity <name>!` and answers false.
- `plugin.rs` is ActivityPlugin and CutscenePlugin. A plugin is a native
  `zones::Zone` (MapZone(name, true, RANDOM_EVENTS), then each restriction,
  the flags and the SAFE zone type) whose behaviour is `Activity`; its own
  fields live in a `Core` (instanced, multicombat, safe, region, base,
  `safeRespawn` read from ServerConstants.HOME_LOCATION at construction, the
  player) with the cutscene plugin's (fade, npcs, the StartPulse and EndPulse
  objects made at construction with `checks = [null]`, since `player` is
  still null then, and their counters). `enter`/`leave` are the
  original's (a player's spawn location from the content, the safe flags,
  the "activity" attribute holding the plugin; on leave the spawn back home,
  an instance's spawn location on logout, the flags cleared, the attribute
  removed). `register_borders` is the ZoneBorders override (the multiway
  zone first when multicombat), `set_region_base` toggles multicombat on the
  plugin's instance and takes the south-west corner of its borders,
  `set_region_base_regions` keeps the original's comparison of region units
  with tile coordinates and adds one RegionZone of the plugin (and of the
  multiway zone) to every region's list directly. CutscenePlugin's `start`
  stores the original location twice ("real-end" removed first) and either
  submits the fade-in StartPulse or does at once what it would (minimap
  state, tabs, teleport to the start location, unlock, walking reset, the
  1,000,000-tick movement lock, close, `open()`), then locks; `leave` with a
  started player puts a logging-out player back and ends, or fades out
  (`unpause`), then unlocks, opens the default tabs, resets and unlocks
  movement; `end` clears every NPC of the instance plane by plane, shows the
  minimap, restores the tabs, unlocks and resets. The pulses keep
  `switch (counter++)`: 1 dims, 3 sends the map state and removes the tabs,
  4 teleports (the EndPulse reads "cutscene:original-loc" then "real-end"
  and casts the value it picks once), 5 finishes (the EndPulse ends, stops
  itself, fades, sends minimap 0 and closes only while the session is
  active).
- `cutscene.rs` is Cutscene.kt: `exitLocation` copied at construction, then
  the PlayerCamera; `start` (the player's own region twice through
  RegionManager.forId, `setup`, minimap 2, the stored stage run before the
  stage attribute is reset, the attributes, the 15 tabs removed, the safe
  flags, lock, the SelfDeath hook, the "cutscene" logout listener, the
  AntiMacro pause); `loadRegion` (NPCs cleared, a new instance, base);
  teleport/move/camera helpers on the lateinit `base` (an uninitialised read
  fails as kotlin's UninitializedPropertyAccessException); the dialogue
  updates whose continue action is the default `incrementStage` or a content
  lambda; `timedUpdate`; `addNPC` (`new NPC(id)`, respawn/aggressive/walks
  off, placed, initialised, facing one step in the direction);
  `addedNPCs` with Java HashMap iteration (`NpcMap`: bucket order, chains in
  insertion order, the table kept by `clear()`); `incrementStage`,
  `updateStage` and the ended guard; `clearNPCs`; the ending pulses: the
  fading `end` runs nine times and its `when` falls through to `return true`
  right after the teleport on the ninth, so the fade from black is never
  reached; `endWithoutFade` teleports then finishes on its second run; their
  `stop()` override clears everything and runs the end actions, logging a
  failure with the anonymous class's empty simple name
  ("There's some bad nasty code in  end actions!") and printing it. The
  death hook ends the player's cutscene (with the fade) or unhooks itself;
  the logout listener moves the player to the exit location and ends the
  player's current cutscene.
- Pulses are native `pulses::Pulse` handles. `Pulses` maps each activity
  pulse object to its body (`Body`: StartPulse, EndPulse, the timed update,
  the ending pulses); a host's `pulses::Context::pulse` and `stop` call
  `activities::run_pulse` and `stop_pulse` by identity. `Pulses::sweep`
  drops what Java would collect (a pulse nobody holds; a plugin core held by
  nothing but its own pulse entries while none of those pulses is held
  elsewhere); the replay sweeps after every operation, but no corpus case
  observes collection.

Every outside call is a required `Host` service with no default: attributes
(values are `AttributeValue`, the plugin and cutscene objects as
`PluginRef`/`CutsceneRef`), properties, locks, the walking queue, the
interface manager, the minimap packet, the session, logout listeners, the
dialogue interpreter's actions, the ContentAPI dialogue functions,
PlayerCamera, AntiMacro, NPC construction, logging, stack traces,
`DynamicRegion.create` and RegionManager.getObject. The host is a
`zones::Host` and a `dynamic_region::Host` whose zone host is itself, so the
plugin's RegionZones share region lists with the multiway zone; multiway
toggles, borders, base locations, region NPC lists and NPC clearing are
`dynamic_region`'s. No lock is held across a host or content call; diagnostic
ids never select behaviour.

## Verification

`cargo xtask activities-oracle --check` (Python and JDK 11) compiles the
unchanged ActivityManager, ActivityPlugin, CutscenePlugin and Cutscene.kt
with MapZone, Zone, ZoneRestriction, ZoneType, RegionZone, FacialExpression,
CameraShakeType, EventHook/Event, the core.plugin interfaces and the whole
dynamic-regions set (DynamicRegion, Region, RegionChunk, RegionManager, the
parsers, Scenery, SceneryBuilder, SceneryDefinition, the MinimapState writer)
over the real cache regions of `fixtures/dynamic-regions/cache.json` (its
hash is checked against that fixture's provenance; no new export). Original
text inserted: the dynamic-regions markers, Location.transform(Direction),
ContentAPI getAttribute/setAttribute/removeAttribute/openInterface/
closeInterface, Player.getCutscene and getCutsceneStage. The players, NPCs,
interfaces, dialogue functions, camera, AntiMacro, session, multiway zone
object and GameWorld are recording adapters (`oracle/activities/adapters.py`);
the content (TestActivity, TestCutscenePlugin, TestCutscene) is scripted
oracle content calling only inherited original members. Every case runs in a
class loader of its own.

The corpus (`oracle/activities/cases.py`) has 513 cases (68 designed, 445
single injected failures: in a marked operation each recorded call fails once
in turn) and 1,830 operations: plain, safe, multicombat and instanced
activities over Lumbridge (12850), the Fight Caves (9551), Pest Control
(10536) and the Clan Wars pair (13114/13115); unknown names with and
without development mode and without settings; failing content
configuration; re-registration; NPCs entering and leaving; no home
location; cutscene plugins with and without the fade, every tick of the
fade-in and fade-out, failures at every tick, inactive sessions, missing and
mistyped "real-end", leaving with and without logout and before starting,
NPCs in the instance, custom map states, tabs and start locations, their own
end pulses and inactive players, a start pulse submitted twice; Kotlin
cutscenes through every stage (NPCs, teleports, moves, every camera call,
every dialogue form, timed updates, getNPC/getNPCs/getObject over real
objects, exits, the fading and non-fading ends and endWithoutFade, end
actions that run and fail), logout and death, the hook without a cutscene,
mistyped cutscene and stage attributes, a stored stage, use before start,
two region loads, 29 NPC ids, logging off, custom continue actions and stage
updates after the end. After every operation the replay compares the result
or error, every recorded call with its arguments in order, the written
packet bytes, each plugin's and cutscene's fields, the registry, every
instance and every region holding zones (zones by name, object and borders),
the reserved areas, the pulse queue (class, owner, delay, ticks, running)
and the player's location, attributes, properties, logout listeners, hooks
and dialogue actions.

`cargo test -p rs09-server --test activities` replays it against the port
with the real native scene, Region.load and `dynamic_region` over the same
cache. Fourteen deliberate breaks of the port were run against the replay
(the instanced logout spawn, the unlock/reset order, the "real-end"
preference, the region-base comparison, the logout end, the ninth-run
teleport, the NPC map order, the stored stage, every NPC of an id cleared,
the NPCs cleared on stop, the printed stack trace, the safe respawn, the
NPC map's growth threshold, the death hook's unhook); all fourteen fail it
(between 1 and 266 cases each).

## Not ported or not joined

- Nothing installs the framework in `rs09-server --world`, and no content
  activity or cutscene is ported. Installing means: the live host implements
  `activities::Host` (it already is the zones host; it must become
  `dynamic_region::Host`, see docs/dynamic-regions.md), answers
  `create_instance` with `dynamic_region::create` on its scene, the minimap
  with `dynamic_scene::minimap_state`, the tab, overlay and close services
  with `world/interfaces` (`remove_tabs`, `restore_tabs`,
  `open_default_tabs`, `open_overlay`, `close_overlay`, `close`), attributes
  with the canonical player's attribute store, AntiMacro with
  `timers::content::antimacro::{pause, unpause}`, the dialogue services with
  `world/dialogue` (an added action runs `cutscene::run_continue`), the
  logout listener map with `cutscene::logout_listener`, the SelfDeath event
  with `cutscene::death_hook`; its `pulses::Context::pulse`/`stop` dispatch
  `activities::run_pulse`/`stop_pulse` and it calls `Pulses::sweep`; the
  start-up registers content activities through `manager::register`.
- PlayerCamera and the ContentAPI dialogue functions belong to the
  ContentAPI port (core/api); here they are services. CameraViewPacket
  itself is native (world/command_packets.rs).
- MapZone.equals compares classes: every native activity plugin has the
  behaviour type `Activity`, so two plugins of different content classes and
  equal names compare equal natively but not in Java (reached only by
  Region.remove(RegionZone); no corpus case does).
- The activity zone's `uid` field is not compared: ActivityManager never runs
  ZoneBuilder.configure, which is what sets it.
- The log origin class (`this::class.java`) is a host argument the adapters
  do not record; the level and message are compared.
- `NpcMap` refuses a ninth colliding NPC id in a table of 64 or more buckets
  (Java would make a tree bin) with a Binding error.
- `Pulses::sweep`'s collection is native accounting; the corpus never
  observes when Java's garbage collector frees an object.
