# Player tab systems (`world/player_tabs`)

`crates/server/src/world/player_tabs/` ports the player's core tab systems:
the weapon tab, the level-up reward and flashing skill icons, the emote tab,
the music player and the quest tab's synchronisation. Each is a library over
the player's own state with a `Host` trait for everything else; no service
has a default. They are engine, not content: the content they reach (quest
journals and varps, three emote overrides, skillcape trimming) stays behind
named services.

## What is ported

| Original | Rust | Scope |
| --- | --- | --- |
| `WeaponInterface` | `weapon.rs` | the constructor, `open(Player)`, the private `open()`, `ensureStyleIndex`, `getConfig`, `updateInterface` (the equipment-change refresh), `setAttackStyle`, `checkStaffConfigs`, `getAutospellId`, `selectAutoSpell`, `openAutocastSelect`, `canAutocast`, `getWeaponInterface(Item)`, `restoreAutocast`, `restoreAttackStyle`; the special bar flag and attack animations |
| `LevelUp`, `LevelUpJinglesKt.getSkillJingle` | `level_up.rs` | `levelup`, `handleMilestones`, `sendFlashingIcons`, every table (skill icons, flash icons, advance configs, milestones, the twenty jingle tables) |
| `EmoteManager`, `Emotes` | `emotes.rs` | `refresh` (with its nested skillcape unlock), `lock`, `unlock`, `Emotes.handle`, `forId`, `play` and `forceEmote` with the BOW, ANGRY, YAWN, DANCE, FLAP, SKILLCAPE and AIR_GUITAR overrides; the 45 constants' buttons, animations, graphics and locked messages |
| `MusicPlayer`, `MusicZone.enter` | `music.rs` | `init`, `refreshList`, `unlock` (both overloads), `play`, `playDefault`, `replay`, `tick`, `toggleLooping`, `setLooping`, `hasUnlocked`, `hasAirGuitar`; the zone's player check |
| `QuestRepository.syncronizeTab`, `forButtonId`, `forIndex`, `Quest.getConfig` (base) | `quests.rs` | the points varp, each quest's varp or varbit in the registry's order, `updateVarps`; the base `getConfig` over a quest's constructor configs |

State joins, rather than copies: the emote list is
`player::persisted::emotes::Manager`, the unlocked songs are
`persisted::music::Player` against the world's `persisted::music::Songs`,
the quest stages and points are `persisted::quests::Repository` against the
world's `Registry` (whose other members, `setStage`, `syncPoints`,
`isComplete`, are ported there and are replayed by this corpus too). The
weapon tab's own fields are `weapon::WeaponInterface`; its `id` is the field
of the shared component handle the interface manager's tab array holds. The
music player's current track, playing and looping flags are
`music::Playback` (the original does not save them).

## Behaviour that is easy to get wrong

- `WeaponInterface.open()` reads definition 92's type for the pane and child
  (no tab index added), whatever the current interface; a null type fails
  there. `ensureStyleIndex` reads the attack style before anything else and
  searches downwards. `updateInterface` sets the special bar before reading
  the attack animations, re-reads the style index for the animation, calls
  `selectAutoSpell(-1, false, false)` for every interface but the staff, and
  for an unusable staff reopens the tab when `autocast_select` is set.
- `setAttackStyle` on an interface that was never updated fails at
  `current.attackStyles` (the warhammer test short-circuits first); the staff
  forgets its saved autocast for a slot of 2 or less, before the range check.
- The autocast members read the spell book and `getNew(3)` once per use, so
  `canAutocast` may read the book three times; `selectAutoSpell` reads the
  current autocast spell's id for each table entry and casts the looked-up
  spell (a non-combat spell is a `ClassCastException`, null passes).
- `openAutocastSelect` mutates the global definition of the select
  component (tab index 0, type TAB).
- `LevelUp.levelup` returns before anything without `tutorial:complete`;
  the skill name is read when the texts are built; `getStaticLevel` is read
  three times; the level-99 news uses the one-argument `Repository.sendNews`
  and the all-skills news `ContentAPI.sendNews` (icon 12, colour CC6600).
  `handleMilestones` reads the combat level once per milestone it compares;
  `amount` is never read. `sendFlashingIcons` reads 24 `levelup:` attributes
  (`SKILL_NAME` has 24 entries, `FLASH_ICONS` 25).
- `EmoteManager.refresh` unlocks the skillcape emote when it is missing,
  which refreshes again inside the first refresh: six varps for a new
  character, three after. `lock` never locks the first 23 constants and
  refreshes even when nothing was removed.
- `MusicPlayer.init` checks the unlocked map for the key 62, which is a list
  index, not music id 62. `play` with looping on reads the entry's id at once
  (a null entry fails there), otherwise at the packet. `tick`'s send is inside
  `catch (Exception)`: a failing send is dropped. `refreshList` skips list
  indexes from 640.
- `syncronizeTab` treats a three-value config as a varbit and anything else
  as `config[0], config[1]` (short arrays fail there); a quest registered
  after the repository was built has no stage and fails unboxing.

## Required services

Every call outside these classes is a method of the system's `Host`, in the
original order: packets (Interface, InterfaceConfig, String, Music, access
mask), varps and varbits, attributes (with the original `/save:` handling in
the host), equipment and item configuration, Settings (attack style index,
special bar), Properties (attack style, attack animation, autocast spell,
combat level, the combat pulse's `updateStyle`), appearance (`isNpc`), the
spell book and spell lookups, the interface manager (tabs, `openTab`,
`openChatbox`, resizable), skills, news, locks, the animator and animation
durations, rights, the pulse manager, the dialogue interpreter, the Varrock
diary, debug output and ticks. Content: each quest's `getConfig` (virtual:
seven quests override it) and `updateVarps`; `Skillcape.trim`; the SALUTE,
GOBLIN_BOW and GIVE_THANKS `play` overrides (`emotes::Host::content_play`).

Region music zones reach `music::zone_enter` through the zones library's
music service (`world::zones`, "MusicPlayer.unlock(id)"); ZoneMonitor's
`checkMusicZones` and the music zone registration are not joined to a
session yet, so that service is a binding a world host must provide.

## Reference tests

```powershell
cargo test -p rs09-server --test player_tabs
cargo xtask player-tabs-oracle --check --java-home <jdk11>
```

`oracle/player-tabs` compiles the unchanged `WeaponInterface`,
`ComponentDefinition`, `InterfaceType`, `LevelUp`, `LevelUpJingles`,
`EmoteManager`, `Emotes`, `MusicPlayer`, `MusicEntry`, `MusicZone`,
`QuestRepository`, `Quest` and `Quests` on JDK 11 with Kotlin 1.8.20 and the
pinned ConstLib 1.4.28 (Items and Components constants checked against their
names). Original text inserted into adapters at generation time: the
EquipmentContainer slots, the Skills constants and `SKILL_NAME`, the
ItemConfigParser keys, the Log and Animator.Priority constants, and the
ContentAPI attribute functions, `setComponentVisibility`, `getStatLevel` and
`sendNews`. Everything else is a recording adapter answering from the case.
Registered quests are a case subclass of the unchanged `Quest` whose
`getConfig` override (when marked custom) and `updateVarps` are recorded;
the item, spell and full `Quests` tables are in the provenance
(`tables`), checked against the compiled enum.

`fixtures/player-tabs/oracle.jsonl`: 5,041 cases (3,288 designed, 1,753 with
one recorded call failing in turn), 27,385 operations, 228,671 recorded
calls. Designed groups: every weapon interface opened from six style indexes
and three attack styles, every button on every interface, staves of every
kind under all three books with autocast spells and saved autocasts,
broken and missing configuration, a definition of another or no type, random
sequences; `getSkillJingle` for every slot and level 0..100, milestones
around every threshold, level-ups of every skill with and without the
tutorial, at 99 and at the all-skills total, flashing icons; emote refreshes
over random unlocked sets, unlock/lock of every constant, every button
through the handler in six situations, every non-content emote with each
relevant item, the Varrock diary task; music init over unlocked/current/
looping/air-guitar states, unlocks of unknown, unnamed, duplicate-index and
out-of-range songs, the all-songs and 200-song air guitar, the 20-tick
replay, zones for a player and an NPC, random sequences; quest tab
synchronisation over random stages, a late quest, short and custom configs,
`getConfig` directly and random repository sequences.

`crates/server/tests/player_tabs.rs` replays every operation through the
native libraries with a recording host and compares the result, the error,
every recorded call and the whole state after each operation (attributes,
Settings, Properties, tab 0, the weapon object, emotes, unlocked songs in
the JDK HashMap's order, playback, quest points and stages, milestones and
the watched component definitions). There are no pinned gaps: every case
matches.

## Joined to the login (`tests/world_login.rs`)

The world-login test binds these libraries to the canonical player
(`Tabs` in the test): packets go through the interface manager's counter and
the session queue, varps through the canonical variable store, Settings,
Properties, the persisted emotes and songs, the spell book and the attribute
store are the canonical player's. Native there, byte for byte against the
original: `WeaponInterface.open` twice and its equipment refresh, the
flashing icons, `EmoteManager.refresh`, `MusicPlayer.init`, and the music
zone entered from `RegionManager.move` (its zone, the music id, is the one
stand-in input there). `QuestRepository.syncronizeTab` stays replayed: the
quests' configs and their content `updateVarps` are not inputs of that test,
and its varbits need the varbit archive. See docs/world-login.md.

## Differences from the original

- `Item.getName()` returning null is modelled as a failing name read; the
  weapon name of a null-named item fails where the String packet is written.
  Neither is exercised.
- Errors carry a category, not a Java stack trace.

## Not ported

The content overrides named above; `Quest.start`, `finish`, journals and
rewards; `MusicPlayer.clearUnlocked` (permadeath); `QuestRepository.parse`
and the other persisted members are in `player/persisted`. Nothing installs
these libraries in a live session; the world-login test is their only join.
