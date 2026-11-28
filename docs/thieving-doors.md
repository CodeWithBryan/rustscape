# Lock-picked doors (`world::thieving_door`)

Scope: the pinned `content/global/skill/thieving/PickableDoorHandler.java`
with its `PickableDoor` class.

| original | here |
|---|---|
| `DOORS` (28 scenery ids) and the two handler keys | `DOORS`, `KEYS` |
| `pickableDoors` (12 doors: tile, level, experience, lockpick, flipped) | `PICKABLE` |
| `forDoor` | `for_door` |
| `PickableDoorHandler.handle`, `PickableDoor.open`, `pickLock`, `isInside` | `handle` |
| `PickableDoorHandler.getDestination` | `destination` |

The bodies are functions over `thieving_door::Host`; every call the
original makes outside this class is a required method with no default.

Pickpocketing is docs/thieving-pickpocket.md, stalls are
docs/thieving-stalls.md and chests are docs/thieving-chests.md. Of
`content.global.skill.thieving` only the Dig Site's two `pickpocketRoll`
calls (quest content elsewhere) are not installed.

## Evidence, and what it does not show

- `fixtures/thieving-doors/oracle.jsonl` (2,059 cases, about 7,300
  operations) is recorded from the unmodified Java source, with the
  unmodified `Direction`, compiled with Kotlin 1.8.20/JDK 11
  (`oracle/thieving-doors/generate.py`, `cargo xtask
  thieving-doors-oracle`), and `cargo test -p rs09-server --test
  thieving_doors` replays every case over this module and compares each
  result and each recorded call in order. One operation compares every
  door's fields and another the handler keys `newInstance` puts on each
  definition.
- What the corpus does not establish (see `scope` in
  `fixtures/thieving-doors/provenance.json`): the player, level,
  inventory, messages, impact handler, scenery, `Location`,
  `handleAutowalkDoor` and `DoorActionHandler.getDestination` are
  recording adapters answered from declared case state, and
  `RANDOM.nextInt` is a declared draw. No option dispatch or door walk is
  run. The generator is not in `oracle/ci.toml`: CI replays the committed
  fixture but does not regenerate it.
- `cargo test -p rs09-server --lib thieving_door`: one table check.

## Kept behaviour worth knowing

- A door is found by its tile, not its id: a listed id on another tile
  says "The door is locked." to "open" and "This door cannot be
  unlocked." to "pick-lock".
- Which side is locked comes from the door's direction and where the
  player stands (`isInside`), turned around for the three flipped doors.
  From the unlocked side "open" walks through with "You go through the
  door." and "pick-lock" says "The door is already unlocked."
- The success draw (`random(12) >= 4`) is made first, even when nothing
  uses it.
- Below the level the attempt always hurts: a hit of 1 or 2 and one of two
  messages by a second draw. Nothing is gained.
- The lockpick is only asked for, never used up. A failed attempt gives
  no experience.
- Two of the doors give 0.0 experience.

## Live world

`world/live/items/pick_doors.rs` puts the handler on the 28 definitions'
two keys at boot when `PickableDoorHandler` is in the scan; a handler on a
definition comes before DoorManagingPlugin's default
(docs/live-world-doors.md). `handleAutowalkDoor` is DoorActionHandler's
end location and walk-through over the door binding. `getDestination` is
run in the walk host, where the movement pulse asks it: that copy of the
two-line body is not the library function the corpus replays. The binding
has no JVM corpus.

The live world's `gather` scenario (`lockman`) uses the level 1 door 34812
at 2672,3308 in East Ardougne: from its west side "Open" says "The door is
locked.", "Pick-lock" is clicked until it succeeds (the walk to the east
side and 3.8 experience), and from the east side "Pick-lock" says "The
door is already unlocked." and "Open" walks back through. No live check
covers a level refusal with its hit, a lockpick door, a flipped door or a
listed id on another tile.
