# Stringing bows and assembling arrows (`world::fletching_assembly`)

Scope: the pinned `content/global/skill/fletching/stringing` (three files)
and `content/global/skill/fletching/arrow` (five files) with the `Feathers`
object they read.

| original | here |
|---|---|
| `StringableCraftInfo` (20 constants), `unstrungBowIds`, `stringItemIds`, `forUnstrungBowItemId`, `applicableStringId` | `STRINGABLES`, `unstrung_ids`, `STRINGS`, `for_unstrung`, `Stringable::string` |
| `ArrowCraftInfo` (8 constants), `arrowTipIds`, `fromTipId` | `ARROWS`, `tip_ids`, `from_tip` |
| `Feathers.all` | `FEATHERS` |
| the stringing lambda's checks before its handler | `string_use` |
| the four handlers' `create` and `getAll` | `create`, `get_all` |
| `StringItemScript`, `HeadlessArrowCraftScript`, `TippedArrowCraftScript`, `FlightedOgreArrowCraftScript` queued lambdas | `script` over a `Job` |

The bodies are functions over `fletching_assembly::Host`; every call the
original makes outside these classes is a required method with no default.
The `SkillDialogueHandler`s are `dialogue_dsl::skill`'s (`ONE_OPTION` for
stringing, `MAKE_SET_ONE_OPTION` for the arrows).

Bolts, darts, crossbow limbs, gem tips and the grapple are
`world::fletching_ammo` (docs/fletching-ammo.md). Not ported: the Zogre
Flesh Eaters fletching listeners.

## Evidence, and what it does not show

- `fixtures/fletching/oracle.jsonl` (2,593 cases, about 15,200 operations) is
  recorded from the unmodified Kotlin sources compiled with Kotlin
  1.8.20/JDK 11 (`oracle/fletching/generate.py`, `cargo xtask
  fletching-oracle`), and `cargo test -p rs09-server --test fletching`
  replays every case over this module and compares each result and each
  recorded call in order. The corpus covers logs, stringing, arrows and `fletching_ammo`.
- What the corpus does not establish (see `scope` in
  `fixtures/fletching/provenance.json`): the player, inventory, levels,
  quest stages, clock, attributes, diary, slayer flags, messages and script
  queue are recording adapters answering from declared case state, the
  SkillDialogueHandler is an adapter that records `open()` only, and the
  driver calls a handler through a declared use-with lookup and runs a
  script at the stage a case names. The original Container, Skills,
  InteractionListeners, ScriptProcessor and dialogue interpreter are not
  run. The generator is not in `oracle/ci.toml`: CI replays the committed
  fixture but does not regenerate it.
- `cargo test -p rs09-server --lib fletching_assembly`: a recording host
  written from the same sources checks both tables, the string and quest
  refusals, each `create`'s checks in order, `getAll`, each script's calls
  in order with its set sizes and delays, and the Seers' Village task. It shows agreement with one reading of the sources.
- The live world's `gather` scenario (`fletcharrow`) is refused a crossbow
  string on a shortbow (u), strings it with a bow string through the
  one-option dialogue (interface 309), makes one set of 15 headless arrows
  through the make-sets dialogue (582), and asks for five sets of bronze
  arrows, which stop after the first when the headless arrows run out. No
  live check reaches a crossbow, the ogre arrows, the comp bow, broad
  arrows, "Make X"/"Make All" or the diary task.

## Kept behaviour worth knowing

- The headless arrow handler's `getAll` counts headless arrows, not shafts
  or feathers.
- `removeItemsIfPlayerHasEnough` only asks for one of each item before it
  removes the amounts. An ogre arrow shaft with one to three feathers
  passes that check with a set of zero: the script reports "0 feathers to 0
  arrow shafts" and rewards nothing.
- The scripts ignore `addItem`'s answer; only `create` asks `hasSpaceFor`.
- Broad arrows ask the slayer unlock before the level.
- The stringing task reads the attribute the log script set, so the magic
  shortbow has to be both cut and strung inside the Seers' Village zones.
- The ogre composite bow's animation is `Animation(-1)`.

## Live world

`world/live/items/fletching_assembly.rs` registers the four listeners at
boot when `StringingListeners` and `ArrowListeners` are in the scan, over
the same host as the logs (`items/fletching.rs`). `SlayerFlags` is not
ported: `isBroadsUnlocked()` answers false and is counted by name
(`SlayerFlags.isBroadsUnlocked`), so broad arrows are always refused with
the original's dialogue. The Zogre Flesh Eaters stage is the one the save
holds; nothing in this world advances it.
