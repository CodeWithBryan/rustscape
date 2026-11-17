# The canonical player's profile state and its save file

`player::owned::Player` is the one native object for a player admitted to a
canonical world. Before this step it held the containers, banks, Settings,
appearance, value attributes, ironman, variables, PIN, scripts and prepared
masks (see [owned-player.md](owned-player.md)). It now also holds the rest of
a player's own state that has a verified native library, and the character
save file is written from it and loaded into it.

This is a library step. Nothing calls it from login or logout yet;
`Player.init`, `LoginConfiguration`, the tick driver and the playable alpha are
untouched. It is not a complete Player.

## What the canonical player now owns

`player::owned::profile::Profile`, bound once with `Player::bind_profile`:

| Field | Original member | Native library (verified by) |
| --- | --- | --- |
| `name` | `Node.name` | — |
| `version` | `Player.version` (6) | player-save corpus |
| `details` | `Player.details`: `UserAccountInfo`, rights, `UIDInfo` (ip, computer name, mac, serial), session address, `saveParsed`, `CommunicationInfo` (contacts, blocked list, clan name/current clan, clan requirements, loot share) | `player::details`, `player::communication` (player-oracle) |
| `skills` | `Entity.skills` | `player::skills` (skills-oracle) |
| `saved` | `Player.savedData` | `player::saved_data` (saved-data-oracle) |
| `timers` | `Entity.timers` | `timers` (core-profile/timers oracles) |
| `properties` | `Entity.properties` | **new**: `player::properties` (this oracle) |
| `spell_book` | `SpellBookManager.spellBook` (192) | player-save corpus |
| `persisted` | quests, music, emotes, diaries, autocast spell (`Properties.autocastSpell`), pouches, POH storage, house and familiar values | `player::persisted` (player-save state corpus) |
| `summoning_combat_level` | `FamiliarManager.summoningCombatLevel` | this oracle |
| `flags` | Player's scalar fields: `newPlayer`, `inWardrobe`, `startLocation`, `isAfkLogout`, `playing`, both `invisible` fields (Player's and Entity's), `artificial`, `customState`, `archeryTargets`, `archeryTotal`, `opCounts`, `invalidPacketCount` | this oracle |

`Profile::new(details, Fresh { world, ticks }, factory)` is what `new
Player(details)` sets for these members, in the original's initialisation
order: Properties, then Skills (whose constructor sets the combat level to 3,
answered here by a field write), then `newPlayer = totalLevel < 50`, the
persisted managers (quests take every quest registered at that moment) and
`version`. The caller constructs `PlayerDetails` first, as the original's caller
does.

`player::properties::Properties` is a port of `Properties.java`'s fields and
initial values (retaliating, teleporting, combat level, attack style,
teleport/spawn locations, the 15 bonuses, attack speed 4, last animation end,
the attack/defence/death animations as actual `Animation` objects with their
priorities, the death graphics, range/magic animations, spell, armour set,
multi/safe zone, safe respawn, combat time-out 10, NPC-walkable, protect
style) and the accessors that only touch them, `getCombatAnimation` and
`getCurrentCombatLevel` (world settings, wilderness and familiar values as
explicit inputs). Not ported: `combatPulse` (CombatPulse is a library not bound
to actors), `updateDefenceAnimation` (configuration lookups), and `setSpell`'s
`combatPulse.updateStyle()` call, which `set_spell` reports instead of making.

### Ownership

No profile value stores a canonical node handle. The original's `entity` and
`player` back-references of Properties, Skills, TimerManager and the persisted
managers are implicit in being fields of this player, so they add no edge to
the ownership graph and `owned::trace` is unchanged. Pouch and
beast-of-burden containers have no listeners. A disconnected player with a
bound profile is collected exactly like one without
(`profile_binding_is_once_and_adds_no_ownership_edge`).

### No lock across a service

`Player::with_profile`, save and load take the profile out of the player for
their duration and put it back when they end, also after a failure. No player
lock is held across a service, so a service that reaches back into the same
player's profile meanwhile gets `profile::Error::InUse` instead of a deadlock
or a stand-in value. Attributes keep their existing infallible views: a save
reads a copy taken when the save starts; a load works on a copy and publishes
it when the load ends, so a view used *during* a load sees the attributes from
before the load and its writes are replaced when the load publishes.

## The save file from and into the canonical player

`Player::save_text`, `save_file`, `load_text`, `load_file` and
`run_content_hooks` run the existing `save::populate`/`save::Parser` over the
canonical player. Nothing in `save.rs` or `parser.rs` changed.

- `save::Profile` borrows the profile's fields; `Settings`, `Appearance`, the
  PIN and ironman managers are the player's own handles.
- `core_data` is projected: the four containers are the player's own
  (`Container` is a shared handle); the bank tab arrays, the bank selector and
  the varp and save-flag maps of the player's variable store are copied in and,
  after a load, written back (keys already in the store keep their place; new
  keys are put in the order the projected map iterates them). This joins the
  save file's varps to the canonical variable store, which player-save.md listed
  as open.
- `save::Host` is `persisted::host::Native` over the profile's persisted state.
  What the canonical player owns is answered from it and never asked of the
  services: `Properties.isRetaliating` (save) and Settings' Properties and
  ironman queries (load), the combat levels skills read and the
  `Combat`/`SummoningCombat` effects they produce. Everything else stays a
  required `persisted::host::Remaining` service: logging, the AttributeSetEvent,
  container listeners, other skill effects and packets, FamiliarManager.parse,
  HouseManager.parse and the seven content hooks. Timer content is the
  caller's `timers::Context`.
- **Entity.location** is the canonical actor's, which `State` holds. A save takes
  it as an argument; a load returns it (`LoadOutcome::location`). Placing the
  player there is login's work and is not done here.

## Verification

```powershell
cargo test -p rs09-server --test canonical_player
cargo xtask canonical-player-oracle --check --python <python> --java-home <jdk11>
```

### The oracle

`oracle/canonical-player` builds on the world-trace oracle as
startup-loaders does: the same unchanged compiled server (3,487 files), the same
pinned call sites and world directory. `oracle.worldtrace.FreshPlayer` boots
the world as WorldTrace does and is accepted only when its boot record is
byte-identical to the world-trace fixture's and two fresh JVM runs agree. It
then constructs `new Player(PlayerDetails.getDetails("traceone"))` and records,
by reflection over the original classes, every instance field of Node, Entity,
Player, Properties, Skills, PlayerDetails, UserAccountInfo, UIDInfo,
CommunicationInfo, SpellBookManager, FamiliarManager and Settings; the
registered quests (68), songs (612), start location, save version and content
hooks; generator positions and clock reads around each step; and
`PlayerSaver.populate()`'s text with each content hook's own output. It then
loads each distinct character file of the world-trace fixture (three) into a
new Player with `PlayerSaveParser.parse` and `runContentHooks` and records the
same again. `fixtures/canonical-player/oracle.jsonl` holds 6 records (world, 1
fresh, 3 loaded, end).

### What matches

- **Fresh player** (`fresh_canonical_player_matches_the_original_constructor_and_first_save`):
  every field the canonical player holds equals the original's, and its first
  save is **byte for byte** the original's populate text. The test lists the
  original fields the canonical player does not hold (`NOT_HELD`) and fails on
  any field that is neither compared nor listed.
- **Clock and generators**: the original reads the clock twice for the first
  player of a JVM (UserAccountInfo's static `default` plus `joinDate`) and once
  for each later one; constructing and loading draw nothing from any
  generator; a fresh save reads the clock once. Natively construction reads it
  once and a save once.
- **World-trace character files** (`world_trace_character_files_load_into_the_canonical_player`):
  all four save files (the new character after 26 ticks, both of the two
  characters, the returning character) load into a new canonical player whose
  held fields equal the original's after the same load, and are written back
  **byte for byte** — equal both to the original's save after its load and to
  the file itself.
- **Original reloads** (`files_the_original_reloaded_into_a_new_player_reload_identically`):
  34 files the player-save corpus's state cases wrote, loaded by the original
  into a new Player and written again, are written identically by the
  canonical player.
- **core_data projection** (`canonical_core_data_matches_the_plain_library_for_every_corpus_file`):
  for all 229 corpus files that load, the canonical player's `core_data` section
  is identical to the plain core_data library's.
- **All whole files of the player-save corpus**
  (`player_save_corpus_whole_files_round_trip_through_the_canonical_player`):
  237 distinct files the original wrote, each loaded into a new canonical player
  and saved: 38 come back byte for byte. The other 199, by category:
  - 156 earlier-corpus files whose quest, music, emote, diary, house,
    familiar, pouch and POH-storage sections were written by that corpus's
    recording adapters, not by the original classes; the canonical player's
    ported classes write their own state there (e.g. four default pouches for
    an adapter's `[]`). Of these, 18 also differ in `core_data`, where the
    canonical player writes exactly what the plain core_data library writes
    after the same load (the cross-check above): a persistent timer's
    `ticksLeft` is written back as 0 because the save happens before a tick,
    which the original does too (player-save.md), or varps whose order in the
    text came from a different put history are laid out as a load into a new
    map lays them out (no corpus holds the original's reload of these
    particular files). 3 differ in `attributes`: values of a type the parser
    does not know were saved as type `null` and load as null, as in the
    original's parser.
  - 35 later-corpus files: the load recomputes quest points (`syncPoints`) and
    skips an emote already unlocked; the 34 original reloads above show the
    original writing the same values after its own load.
  - 8 files fail to load: three where the original's own load of the same file
    failed with the same category in the corpus (`Null`, `Number`, `Null`);
    five the original never loaded in the corpus, failing as its parser does:
    an expiration time of `null` (`toLong`), an inventory slot of -1 (twice),
    a `version` a content hook overwrote with text, and an adapter's pouch
    array of numbers (cast in the real PouchManager).

### Stand-ins in these tests

They are inputs, not ports: the seven content hooks write what the original's
hooks wrote (recorded by the oracle, or scripted in the player-save corpus);
FamiliarManager.parse and HouseManager.parse do nothing; the timer service
gives `antimacro` AntiMacro's two persistence methods (`ticksRemaining`
against world tick 0) and any other identifier RSTimer's base methods; the
quest and song registries are the ones the oracle recorded after boot.

## Deviations from the original

- `Node.active` is false until `Player.init`; `State.add` creates an active
  actor. Pinned in the fresh-player test, left to login.
- `POHStorageState` is created lazily by the original (`getPOHStorageState`);
  natively it exists from construction. No saved byte depends on it.
- Profile access during a running save or load fails (`InUse`) instead of
  reading the live object; attribute views read a copy (see above).
- `Player.name`'s file stem, the Java-below-11 output, default charsets and the
  other save-file limits are those of player-save.md.
- A varp store cleared since construction keeps its larger table in the
  original; the save's projection lays its keys out at the size their count
  needs. Not exercised by any corpus. A null varp value is refused as `Null`.

## What remains outside the canonical player

The other objects a Player constructs: interface manager and packet dispatch
(partly bound), dialogue interpreter, prayer, skull manager, request manager,
warning messages, hint icons, house/familiar runtime, emote/music players as
gameplay, skiller tasks, equipment degrader, render info and player flags
(partly in the actor), update masks, walking queue, pulse manager, impact
handler, animator, teleporter, zone monitor, action locks (actor), Entity's
event hooks and extensions, the CombatPulse, logout listeners and the session.

## What login and the tick driver need from this

- Login: construct `PlayerDetails` (with the account), create the actor,
  `owned::Player::new`, `Profile::new` with the world's registries and tick,
  `bind_profile`; `load_file` then `run_content_hooks`; place the actor at
  `LoadOutcome::location`; set `active` (Entity.init), `playing`, the session
  address; then `LoginConfiguration` (lobby/tutorial, the
  `logged-in-fully` attribute, varp sends through the variable store).
- Logout and the 50-tick autosave: `save_file` with the actor's location after
  `PlayerDetails.save` rules.
- Tick driver: `with_profile` for skills pulse, timers `process`
  (`timers::Manager` in the profile), `Properties.teleporting` reset, the
  `opCounts` reset, `lifepointsUpdate`; services for all content.
