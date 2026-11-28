# Pickpocketing (`world::thieving_pickpocket`)

Scope: the pinned `content/global/skill/thieving/Pickpockets.kt` and the
"pickpocket" / "pick-pocket" NPC listener of `ThievingListeners.kt` with its
`pickpocketRoll`.

| original | here |
|---|---|
| `Pickpockets` (18 constants: ids, level, low, high, experience, stun damage and time, the entries its table is given) | `PICKPOCKETS`, `Pickpocket::entries` |
| `Pickpockets.forID` | `for_id` |
| `ThievingListeners.pickpocketRoll` | `pickpocket_roll` |
| the listener's lambda | `pickpocket` |

The bodies are functions over `thieving_pickpocket::Host`; every call the
original makes outside these two classes is a required method with no
default. The loot tables are `drop_tables::WeightBasedTable`'s, reached
through `Host::can_roll`, `Host::exclude_clues` and `Host::roll`.

Stalls (`ThievingOptionPlugin`, `Stall`, `StallThiefPulse`) are
docs/thieving-stalls.md and chests (`ThievableChestPlugin`) are
docs/thieving-chests.md; doors (`PickableDoorHandler`) are
docs/thieving-doors.md. Not ported: `Pickpockets.getSuccessChance` (nothing calls it) and the Dig Site's two
calls of `pickpocketRoll`.

## Evidence, and what it does not show

- `fixtures/thieving/oracle.jsonl` (1,018 cases, about 11,400 operations) is
  recorded from the unmodified Kotlin sources compiled with Kotlin
  1.8.20/JDK 11 (`oracle/thieving/generate.py`, `cargo xtask
  thieving-oracle`), and `cargo test -p rs09-server --test thieving` replays
  every case over this module and compares each result and each recorded
  call in order. One operation compares every constant's fields and the
  entries its table is given, in order; another asks `forID` for every
  listed id and its neighbours.
- What the corpus does not establish (see `scope` in
  `fixtures/thieving/provenance.json`): the loot table is a recording
  adapter, so `canRoll`, `excluding` and `roll` are logged and answered from
  declared case state. Table positions, weights, slots and the inventory
  questions are the drop-tables corpus's (`fixtures/drop-tables`). The
  player, levels, combat state, animator, equipment, inventory, manual hit,
  NPC facing and attack animation, the skillcape perk, the treasure trail
  manager, `animate`, `playAudio`, `playHurtAudio` and `stun` are recording
  adapters too, and `randomDouble(1.0, 100.0)` returns the case's declared
  chance. The driver calls the registered lambda with an NPC of the id a
  case names; InteractionListeners is not run. The generator is not in
  `oracle/ci.toml`: CI replays the committed fixture but does not
  regenerate it.
- `cargo test -p rs09-server --lib thieving_pickpocket`: one table check.
- The live world's `gather` scenario (`pickman`) uses a Lumbridge man's
  "Pickpocket" option until an attempt succeeds, and then holds three coins
  and 8 Thieving experience. A failed attempt is waited out by its "You
  have been stunned!" message. No live check asserts the hit, the NPC's
  animation, a level or space refusal, the combat refusal, a table with
  weighted entries, a clue holder, the gloves or the perk.

## Kept behaviour worth knowing

- The level is read twice: for the requirement and again inside
  `pickpocketRoll`, after the chance is drawn.
- The chance is drawn from 1.0 to 100.0 and fails when it is above
  `getSkillSuccessChance(low, high, level)` plus 25 for smooth hands and 3
  for the gloves of silence.
- A success may hand over nothing (an empty roll) and still rewards the
  experience and locks the player for two ticks. `inventory.add`'s answer
  is not read.
- A clue holder's table is rebuilt by `excluding` on every attempt, which
  draws new positions for its weighted entries.
- A gnome answers with its own attack animation; every other NPC with 422.
- The damage is `RandomFunction.random(min, max)`: equal ends draw nothing,
  and the upper end is exclusive (a H.A.M. member hits 1 or 2, never 3).
- `GUARD` lists six ids twice; no id is in two constants.
- `DESERT_BANDIT`'s coins are `WeightedItem(COINS, 50, 1, 3.0)`: the
  amounts are given the wrong way round and kept.

## Live world

`world/live/items/pickpocket.rs` registers the listener for both options at
boot when `ThievingListeners` is in the scan. The 18 tables are made from
the shared generator the first time the lambda runs, in declaration order
and before any of its checks (also for an NPC id no constant lists), as the
enum's class initialiser makes them on the first `forID`; nothing
else in this world touches `Pickpockets` earlier (the original's
`VarrockGuardSignpost` and `GlobalStats`, which also read it, are not
ported). `roll()` has no receiver. No clue scroll is registered in this
world, so an easy clue slot rolls nothing.

`TreasureTrailManager.hasClue()` is answered from the containers: a casket
in the inventory, the bank in use or the equipment, or one of the
manager's 329 clue ids in the inventory or that bank. The manager itself is
not ported. `SkillcapePerks` is not ported either: the binding reads the
`cape_perks:smooth-hands` attribute, which nothing here sets. `stun` is
composed in the binding (walking reset, pulses cleared, movement lock, the
STUN clock, graphics 80, sound 2727, animation 424 and the message). The
binding has no JVM corpus.
