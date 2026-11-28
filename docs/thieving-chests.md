# Thieving chests (`world::thieving_chest`)

Scope: the pinned `content/global/skill/thieving/ThievableChestPlugin.java`
with its `Chest` enum and the anonymous pulse of `searchTraps`.

| original | here |
|---|---|
| `Chest` (6 constants: object id, level, experience, rewards, respawn) | `CHESTS` |
| `Chest.forId` | `for_id` |
| each constant's `currentRespawn` | `Respawns` |
| `ThievableChestPlugin.handle`, `Chest.open`, `Chest.searchTraps` | `handle` |
| the anonymous `Pulse(1, player)` | `TrapPulse::pulse` |

The bodies are functions over `thieving_chest::Host`; every call the
original makes outside this class is a required method with no default.

Pickpocketing is docs/thieving-pickpocket.md, stalls are
docs/thieving-stalls.md and doors (`PickableDoorHandler`) are
docs/thieving-doors.md.

## Evidence, and what it does not show

- `fixtures/thieving-chests/oracle.jsonl` (271 cases, about 2,200
  operations) is recorded from the unmodified Java source compiled with
  Kotlin 1.8.20/JDK 11 (`oracle/thieving-chests/generate.py`, `cargo xtask
  thieving-chests-oracle`), and `cargo test -p rs09-server --test
  thieving_chests` replays every case over this module and compares each
  result and each recorded call in order. One operation compares every
  constant's fields and another the handler keys `newInstance` puts on each
  definition.
- What the corpus does not establish (see `scope` in
  `fixtures/thieving-chests/provenance.json`): the player, level,
  lifepoints, inventory, messages, lock, animator, face mask, impact
  handler, scenery, `SceneryBuilder.replace`, the world tick and the pulser
  are recording adapters answered from declared case state. The driver
  calls the plugin's `handle` and the submitted pulse's `update` itself and
  sets every constant's respawn tick back to 0 before a case; no pulse
  runner or option dispatch is run. The generator is not in
  `oracle/ci.toml`: CI replays the committed fixture but does not
  regenerate it.
- `cargo test -p rs09-server --lib thieving_chest`: one table check.
- The live world's `gather` scenario (`chestman`) uses the ten-coin chest
  2566 in East Ardougne at Thieving 40: "Search for traps" gives the four
  messages, 10 coins and 7.8 experience; "Open" then says the chest has
  been looted, and once eleven ticks have passed springs the trap and
  hands over nothing. No live check asserts the hit's amount, the level
  refusal with its delayed "You find nothing.", the full inventory
  refusal, a reward that does not fit, the looted chest's id or a second
  player.

## Kept behaviour worth knowing

- `currentRespawn` is a field of the enum constant: one looted chest makes
  every chest of that constant respawn for every player.
- "Open" never opens a chest. It springs the trap (a two tick lock and a
  hit of a twelfth of the lifepoints, at least 2) unless the constant is
  respawning.
- "Search for traps" on a scenery that is no chest throws (the plugin is
  only registered on the six ids).
- Below the level the search still animates, locks for two ticks and says
  "You find nothing." one tick later. The inventory is only asked for one
  free slot, whatever the constant rewards; what does not fit is dropped
  for the player.
- The pulse says its lines on its second, fourth and sixth runs. The sixth
  replaces the chest with 2574 for three ticks only while the chest is
  active, and sets the respawn to the tick plus `(int) (respawn / 0.6)`.
- A second search while a pulse runs starts a second pulse.

## Live world

`world/live/items/chests.rs` puts the plugin on the six definitions' two
handler keys at boot when `ThievableChestPlugin` is in the scan, and runs
the trap pulse as a world pulse that ends when its player is gone or
inactive. `player.sendMessage(message, ticks)` is run as a pulse of that
delay which then sends the message. The binding has no JVM corpus.
