# Player managers: skull, hint icons, warnings, assist, diaries

`world/player_managers/` ports the remaining core player managers of
`core.game.node.entity.player.link`:

| Original | Native |
| --- | --- |
| `SkullManager` with `SkullIcon` and its anonymous deep-wilderness `ContainerListener` | `skull.rs` (`SkullManager`, `SkullIcon`, `DeepWildListener`, `answers`) |
| `HintIconManager`, `HintIconContext`, the `HintIcon` packet writer | `hint_icons.rs` |
| `WarningMessages` with `WarningMessage` | `warnings.rs` |
| `request/assist/AssistSession` (the ASSIST request module, its close event and pulse) | `assist.rs` |
| `diary/AchievementDiaryManager`, `diary/AchievementDiary` | `diary.rs` over `player/persisted/diaries.rs` |
| `diary/DiaryType` (task tables, rewards, NPCs, info) | `diary_data.rs` (data, written by `oracle/player-managers/port_diary_data.py`) |

These are libraries. Nothing installs them in a session: no example, alpha,
`--world` server or login path calls them yet. Every system they reach
outside these classes is a required service of the module's `Host`, called
in the original order with the original arguments; none has a default and no
borrow of manager state is held across a service call. This is not a
complete wilderness, PvP, assist or diary system.

## What is ported

SkullManager:

- The fields (wilderness, wilderness disabled, level, skull causes, skulled,
  skull check disabled, deep wilderness) are `SkullManager`, one per player,
  reached through `skull::Host::skull`. Getters and setters are the fields;
  `isSkulled` is skulled or deep wilderness; `hasWildernessProtection` is
  level below 49.
- `setLevel`: deep wilderness is entered from 49 and left below 48 (48 keeps
  it), then the teleport lock (`lockTeleport(1000000)` above 20,
  `unlockTeleport` otherwise), then the level is stored.
- `setDeepWilderness(deep)`: with `ServerConstants.ENHANCED_DEEP_WILDERNESS`
  and `deep`, `updateDWSkullIcon`; in every other case `removeDWSkullIcon`
  (also when entering without the setting); then the skull check flag and
  the deep flag take `deep`.
- `updateDWSkullIcon`: the listener is installed once (attribute, then the
  inventory's list, then the equipment's); the risked value is the sum over
  the inventory then the equipment of `max(1, alchemy) * amount` (the
  definition is read a second time when above 1; long arithmetic that wraps)
  minus the largest single value (protect item); it is stored as
  "deepwild-value-risk"; the red skull at 100,000 or more, the brown one below.
  An attribute holding anything else stops the installation, and makes
  `removeDWSkullIcon` fail its cast after the icon was set.
- `removeDWSkullIcon`: the white skull when skulled, none otherwise; the
  attribute's listener leaves both lists; both attributes are removed.
- `checkSkull(other)`: only a Player other, only in the wilderness and with
  the check enabled; not when the other already skulled on this player or
  this player already on the other. The cause is added before the Skulled
  timer is restarted (`removeTimer("skulled")`, `spawnTimer("skulled",
  2000)`, `registerTimer` when the registry has one). Removing a running
  Skulled timer resets the manager, so the cause just added is cleared again
  (and the skulled player only keeps the new timer's skull).
- `setSkullIcon` (appearance icon, then `updateAppearance`) and `reset`
  (causes cleared, icon -1, not skulled, appearance synced).
- `SkullIcon.forId`: 0-7, anything else NONE; the ids -1..7.

HintIconManager: eight slots; `freeSlot`; the seven register overloads (the
free-slot ones, the five-argument one that returns -1 for a negative slot and
0, not the slot, for a null target, and the typed ones whose context
constructor fails on a null target); every register builds the context (an
entity's index, otherwise the node's location), writes the HintIcon frame to
the context's player and only then stores it, so slot 8 and above fail after
their frame was sent. `removeHintIcon` sets the stored context's target type
to 0 and sends it; contexts are shared objects (`Shared`), so changes made
through `getIcon` show in that frame. `clear` walks the slots as it reaches
them and removes each through its context's player. The frame: opcode 217,
`slot << 6 | type` and the arrow as bytes, and when the arrow is positive the
index (types 1 and 10), the location and height, or zeros, then the model,
all truncated as IoBuffer does.

WarningMessages: `open` (component 583, then varp 1045 = 0 saved);
`getMessage` (first match, otherwise appended); `toggle` refuses with its two
messages (no message is ever active as pinned; the unreachable remainder is
kept as written).

AssistSession: `open(player, target)` as the request module (both ironman
checks, both face, a new session extended to the player and opened);
`open()` (the partner's extension, lock, component 301, `load`, three
messages, two strings, animation 7299/graphic 1247 and the partner's
animation, sound 4010 to both, icons 79-80 of 548 hidden on both, the pulse
submitted, `refresh`); the close event (save, both extensions removed,
unlock, icons back, tabs restored, both told, killed: the closing player is
the event's parameter, the saving player the session's field);
`addExperience` (restricted or total at 30,000, the per-skill cap, the
unknown-skill index failure); `refresh` (each value clamped, then sent as an
int, toggled skills in varp 1087, restriction from the total and its message
with `getTimeLeft`, varp 1088 = int total times 10); `toggleButton` (a byte
index); `isToggled`; `getSkillIndex`; `getTimeLeft` (hours above one hour,
minutes above one minute, else seconds, truncating); `load` (a zero window
end is only written to GlobalData, so the session's own time stays 0 and its
first pulse starts a new window, zeroing the experience it just loaded);
`save` (skills refreshed, GlobalData takes the session's time and its own
experience array, not a copy); `reset`; `pulse` (closed through the interface
manager beyond 20 tiles, on another plane or when either player is inactive;
the clock read twice for a new window; refresh; finished once killed);
`translateExperience` (the cap formula in its original float order, the
assister's skill experience, then the session's experience times `mod`).

AchievementDiaryManager and AchievementDiary: `parse` (the persisted parser
joined, with the world's shared `completedLevels`), `openTab` (tab 259, then
each started diary's status lines), `updateTask` (level started, the task
message or the completion messages with Lumbridge's beginner level, the
congratulation with the NPC's name, the status), `finishTask` (not for an
artificial player; an unfinished task completed; the shared list gains the
level only when all 25 slots are set, otherwise loses its first entry for the
level), `resetTask`, `open` (311 lines cleared, title, status line, start
info for a diary not started, each level's status line and tasks, split on
`<br><br>` with Java's trailing-empty rule, struck through when done, the
interface when nothing is open, run script 1207), `drawStatus`, the started
and completion queries (cumulative included), `getLevel`, `getReward`,
`getStatus`, the reward checks (`hasGlove`, `getKaramjaGlove`, `hasArmour`,
`getArmour`, `checkMiningReward`, `checkSmithReward` with Bar ordinals),
`isComplete(type)`, `resetRewards`, and the static helpers
(`removeRewardsFor`, `addRewardsFor`, `flagRewarded`, `canReplaceReward`,
`grantReplacement`, `hasCompletedLevel`, `hasClaimedLevelRewards`,
`canClaimLevelRewards`, `getRewards`). DiaryType: the six constants as data,
`forChild`, `getNpc`, `hasRewardEquipment`.

## Verification

```powershell
cargo xtask player-managers-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
cargo test -p rs09-server --test player_managers
python oracle/player-managers/port_diary_data.py --check
```

The oracle compiles the unchanged SkullManager, HintIconManager,
WarningMessages, AssistSession, RequestModule, AchievementDiaryManager,
AchievementDiary, DiaryType, DiaryLevel, Bar, HintIcon, HintIconContext,
IoBuffer, PacketHeader, Context, OutgoingPacket, CloseEvent,
ContainerListener, MapDistance and the timer framework the Skulled timer runs
on (RSTimer, PersistTimer, TimerFlag, TimerManager, Skulled) on JDK 11 with
Kotlin 1.8.20, kotlin-reflect 1.6.10 (compile classpath only), json-simple
1.1.1 and ConstLib 1.4.28. Original text inserted at generation time:
Player.debug and hasItem, Item.getAlchemyValue(), the GlobalData assist
accessors, EquipmentContainer's slot constants, WildernessZone's protection
attribute, the Skills ids, Location.withinDistance/getDistance and ContentAPI
registerTimer/spawnTimer/removeTimer. Everything else is a recording adapter
under `oracle/player-managers/adapters/`. AssistSession's `System` resolves to
a same-package clock class of the oracle (Java's same-package rule shadows
`java.lang`), so its source text is unchanged and every clock read is a
recorded case input.

The designed corpus, `oracle/player-managers/cases.py`, has 865 cases, 7,838
actions and 25,267 recorded calls (536 HintIcon frames); 333 operations fail
(126 injected failures reached, 114 index, 69 null, 24 cast); 195 failures are
injected in total (the rest are caught by TimerManager's try or never
reached):

| Group | Cases | What it drives |
| --- | ---: | --- |
| skull | 219 | the icon table and forId; every getter and setter; setLevel at both sides of 20/21, 47/48/49/50 and the int extremes from inside and outside deep wilderness, with and without the enhanced setting; setDeepWilderness in every skulled/enhanced combination; the risk over 14 holdings (protect item, the threshold at 99,999/100,000/100,001, zero and negative alchemy values, negative amounts, long wrapping); the listener installed, refreshed by container events and removed; foreign, missing and wrong-type attributes; reentrant listener refreshes and level changes during an update; checkSkull's early returns, mutual and self causes, an empty registry, the real Skulled timer (onRegister, run, onRemoval through removeTimer before and after processTimers) and its reset clearing the new cause; every recorded call failing at its first and second occurrence in three flows |
| hint | 189 | every register overload over players, NPCs, nodes with and without a location and null; arrows -1..2^31-1 and models -1..70,000; target types -1..64; slots -5..100; heights; filling all eight slots, the ninth refusal, removal and clear; mutations of the stored context before removal; other players' managers; reentrant registers/removals from the send; send, index and location failing at each of their first three calls |
| warn | 10 | open, getMessage (repeats, negatives, 25 messages), toggle for each player, a null and an NPC, failures, a reentrant lookup |
| assist | 186 | the skill index table; an empty session; the request module with each ironman answer; load at window ends around now and 24 hours; short and long stored arrays; addExperience and the caps; the time left at every unit boundary; buttons; translateExperience over toggles, skills, amounts at the cap and fractional values; the pulse at 20/21 tiles, planes, wrapping coordinates, inactive players and the window end; closing through the manager and the event, then pulsing and adding (the GlobalData alias); reentrant closes; every recorded call of the opening and of the close failing |
| diary | 261 | the DiaryType table, forChild, rewards, NPCs and tasks per level; hasRewardEquipment over every container; parse of valid, renamed, oversized, missing, wrong-type and empty data with the shared list; openTab; updateTask/finishTask over every level of three diaries (Lumbridge's beginner level, completion messages); the 25-slot rule of the shared list; null types and bad indices; gloves and armour (36 pairings); mining and smithing doubles per armour level; whole-diary completion; open in every state; every query at levels -1..3; the reward helpers with full, near-full and holding inventories, replacement and claiming; debug messages; injected failures |

`cargo test -p rs09-server --test player_managers` replays every case
through the native libraries and compares results, the ordered call record
(HintIcon frames as bytes), every player's skull fields, hint slots,
attributes, extensions, container contents and listeners, opened component,
GlobalData assist values and which session's array they share, diaries,
warning messages and timer lists, every session's fields, the shared
completed-level list and the spawned timers. A second test pins the fixture
and source hashes.

The comparison is not vacuous: twenty deliberate breaks (the 49/48/20
boundaries, protect item, the HintIcon index bytes, the removal's target
type, the null-target return, frame-before-store, varp 1045, varp 1088, the
zero window end, the second clock read, the cap formula's float order, the
close event's extension order, the GlobalData copy, the open's child
arithmetic, Lumbridge's level, the hands slot, the 25-slot rule, the
cause-before-timer order, the white-skull choice) each fail between 3 and 163
cases; the 25-slot rule and the float order were undetected at first and the
cases that now catch them were added.

The oracle needs `json-simple-1.1.1.jar` and `ConstLib-1.4.28.jar` in
`runs/player-managers-oracle/` (or any `runs/*-oracle/`), checked against
their pins, and the Kotlin jars in `runs/kotlin-1.8.20/`; it never downloads
them.

## How the other systems join

Wilderness and the other skull services. Every SkullManager service another
library declares is answered by the native manager (`skull::answers` and the
functions with effects), through a host that implements `skull::Host` for the
same players:

| Service | Declared in | Native answer |
| --- | --- | --- |
| `skull_wilderness` | `zones::Host`, `npc_behaviour::Host`, `player_death::Host` | `answers::is_wilderness` |
| `is_wilderness` | `combat_pulse::Host` | `answers::is_wilderness` |
| `skull_level` | `zones::Host` | `answers::level` |
| `set_skull_level` | `wilderness_zone::Host` | `skull::set_level` (deep wilderness, the risk skull, the teleport lock) |
| `set_skull_wilderness` | `wilderness_zone::Host` | `answers::set_wilderness` |
| `skull_wilderness_disabled` | `wilderness_zone::Host` | `answers::is_wilderness_disabled` |
| `skull_deep_wilderness` | `wilderness_zone::Host` | `answers::is_deep_wilderness` |
| `deepwild_value_risk` | `wilderness_zone::Host` | the "deepwild-value-risk" attribute `skull::update_dw_skull_icon` stores (`set_risk_attribute`) |
| `has_wilderness_protection` | `npc_behaviour::Host` | `answers::has_wilderness_protection` |
| `check_skull(entity, victim)` | `combat_pulse::Host` | `skull::check_skull(h, player, victim as a player or None)` |
| `skulled` / `set_skulled` | `player_death::Host` | `answers::is_skulled` / `answers::set_skulled` |

So `WildernessZone.show` reaching `set_skull_level` now produces the
deep-wilderness skull icons and the level-20 teleport lock that the wilderness
corpus could not record (its SkullManager is a recording adapter), and the
lock reaches the zones' `teleport_locked` service through ActionLocks. The
wilderness library is unchanged; the binding is the owning world's host (one
object implementing both traits), which nothing installs yet.

The Skulled timer. `checkSkull` calls three `skull::Host` services that the
timers port answers: `remove_timer(player, "skulled")` is
`TimerManager.removeTimer(identifier)`, `spawn_timer("skulled", 2000)` is
`TimerRegistry.getTimerInstance("skulled", 2000)` (Skulled with run interval
2000, None when the registry lacks it) and `register_timer(player, timer)` is
`TimerManager.registerTimer`. The timers port's Skulled must call back:
onRegister `skull::set_skull_icon(h, player, 0)` then
`answers::set_skulled(h, player, true)`; run and onRemoval `skull::reset(h,
player)` (run returns false). The corpus runs the unchanged TimerManager and
Skulled; the native replay's stand-in (`oracle/player-managers/native.rs`,
`skulled_on_register`, `skulled_reset`, `remove_timer_named`,
`process_timers`) is written only with these public functions, including
TimerManager's caught-and-printed onRemoval failures and its list walks.

Requests. `trade::Host::assist_open(player, target)` (RequestType.ASSIST's
`module.open`) is answered by `assist::module_open`.

The interface manager. `assist::Host::open_component` opens the session's
component 301; a binding to the native `interfaces` manager must make it one
component whose typed close event calls `assist::close_event(h, session,
player)` (as trade's TradeClose does) and dispatch `close_interface` to that
manager. Diary and warning components are fresh components without events.

Pulses. `assist::Host::submit` hands the session to the world pulser; the
world's pulse context must dispatch the session handle by identity to
`assist::pulse`.

Container listeners. `DeepWildListener` is a native `container_listener`
object (it reports its player as a retained node); the owning world's
`container::Context` must dispatch it by type to `skull::listener_update` and
`skull::listener_refresh`, as trade's `ui::Items` does for its listener.

Saved state. Diary flags and tasks are `persisted::diaries::Manager` (the
save file's state; `diary::parse` is its parser with the shared list). The
assist window end and experience are GlobalData's; `assist::Host` reaches
them through `global_data` and four accessors. The native GlobalData record
(`player/saved_data/records.rs`) holds the experience as a value, so a
binding must keep the session's array as the stored one after `save` (the
corpus shows the alias: a pulse after closing zeroes GlobalData's values) or
pin the difference.

## Required services

`skull::Host`: the manager accessor, the three timer calls, appearance icon,
update and sync, lock and unlock teleport, the enhanced setting, the two
attributes and removal, both listener lists, the container snapshots and the
alchemy value. `hint_icons::Host`: the manager accessor, the entity index,
the node location and the session write. `warnings::Host`: open, setVarp and
sendMessage. `assist::Host`: ironman restriction, face, the extension get,
add and remove, lock, unlock, the component open, the manager close, restore
tabs, messages, strings, interface configs, username, animation, graphics,
sound, the pulser, setVarp, the clock, GlobalData and its accessors, skills
refresh/level/addExperience, location and activity. `diary::Host`: the
persisted diaries, the shared list, strings, messages, NPC names, the tab,
the opened check and open, the run script, the artificial flag, the debug
attribute and message, container contains/remove/add/freeSlots/get and item
names.

## Deviations

- Managers are values the host keeps per player; the original's back
  references (`SkullManager.player`, the listener's manager) are the player
  handle.
- A diary type is its DiaryType ordinal; an item is (id, amount).
- `completedLevels` is a world value passed by the host, not a JVM static.

## Harness limits

The containers, the interface manager (one opened field), the attribute
store and the timer manager of the replay are case models, the same on both
sides; their real classes are other ports. No case reaches a due timer in
`processTimers` (world tick 0). Content that calls these classes (combat's
checkSkull, assist buttons and Skills' translation, diary task triggers and
reward dialogues, warning screens) is not driven.

## Not ported

The content around these managers: AssistInterfacePlugin, Skills' calls into
translateExperience, the diary task hooks and reward NPC dialogues, the
warning screens' areas, and every session binding. The Skulled timer itself
belongs to the timers port.
