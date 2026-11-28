# Bolts, darts, crossbow limbs, gem bolt tips and the mithril grapple (`world::fletching_ammo`)

Scope: the pinned `content/global/skill/fletching/bolts`, `darts`,
`crossbow`, `gem` and `grapple` packages with the `Feathers` object they
read.

| original | here |
|---|---|
| `BoltCraftInfo` (9 constants), `DartCraftInfo` (7), `UnfinishedCrossbowCraftInfo` (7), `GemBoltsCraftInfo` (11) and their lookups | `BOLTS`, `DARTS`, `CROSSBOWS`, `GEMS`, `for_gem`, `for_tip`, `untipped_ids` |
| the seven `onUseWith` lambdas of `BoltListeners`, `DartListeners`, `LimbingListener`, `GemBoltListeners` and `GrappleListeners`, up to their handler or script | `use_with` over a `Listener` |
| the five handlers' `create` and `getAll` | `create`, `get_all` over a `Craft` |
| `BoltCraftScript`, `DartCraftScript`, `LimbingScript`, `AttachGemTipToBoltScript`, `CutGemsIntoBoltTipsScript` (its start and its two loops) and the two grapple scripts | `script` over a `Script` |

The bodies are functions over `fletching_ammo::Host`; every call the
original makes outside these classes is a required method with no default.
The `SkillDialogueHandler`s are `dialogue_dsl::skill`'s (`ONE_OPTION` for
limbs and gem cutting, `MAKE_SET_ONE_OPTION` for bolts, darts and gem
tips).

Not ported: the Zogre Flesh Eaters fletching listeners
(`ZogreFletchingListeners`).

## Evidence, and what it does not show

- `fixtures/fletching/oracle.jsonl` (2,593 cases, about 15,200 operations;
  1,160 of the cases are this module's) is recorded from the unmodified
  Kotlin sources compiled with Kotlin 1.8.20/JDK 11
  (`oracle/fletching/generate.py`, `cargo xtask fletching-oracle`), and
  `cargo test -p rs09-server --test fletching` replays every case over this
  module and compares each result and each recorded call in order.
- What the corpus does not establish (see `scope` in
  `fixtures/fletching/provenance.json`): everything docs/fletching-assembly.md
  lists, and when or in what order ScriptProcessor runs the two loops
  `CutGemsIntoBoltTipsScript` queues. The driver runs the script a case
  names at the stage the case names; the shared "finished" flag between the
  two loops is exercised only in the orders the cases declare.
- `cargo test -p rs09-server --lib fletching_ammo`: one table check written
  from the same sources.
- The live world's `gather` scenario (`fletchbolt`) makes one set of ten
  bronze bolts through the make-sets dialogue (582), is refused a set of
  bronze darts with the Tourist Trap dialogue, is refused oak limbs on a
  wooden stock, attaches bronze limbs through the one-option dialogue
  (309), cuts an opal into twelve bolt tips and asks for ten sets of opal
  bolts, which stop after the first when the bronze bolts run out. No live
  check reaches a made dart, the grapple, broad bolts, a level or space
  refusal, "Make X"/"Make All", or more than one gem in a cutting loop.

## Kept behaviour worth knowing

- A set is the smaller of the two stacks, at most ten; the check before
  the removal only asks for one of each item.
- Broad bolts ask the slayer unlock before the level; darts ask the level
  before the quest.
- The bolt script's message has no full stop ("You fletch 10 bolts"); the
  gem tip script's has.
- The limbing lambda answers the wrong limbs with a message; its handler's
  `getAll` counts stocks only.
- A chisel on a gem without a constant answers true and opens nothing.
- Cutting gives 12 tips, 6 for a pearl and 24 for pearls and onyx. The
  cutting loop stops on its amount by the script's stage, and tells the
  animation loop to stop through the flag they share.
- The grapple lambdas check level 59 with a message, not a dialogue, and
  queue their script without a handler.

## Live world

`world/live/items/fletching_ammo.rs` registers the seven listeners at boot
for each of the five classes in the scan, over the same host as the logs
(`items/fletching.rs`). `SlayerFlags` is not ported: `isBroadsUnlocked()`
answers false and is counted by name (`SlayerFlags.isBroadsUnlocked`), so
broad bolts are always refused with the original's dialogue. The Tourist
Trap stage is the one the save holds (quest index 123); nothing in this
world advances it, so darts are refused unless a save already holds stage
100. The binding has no JVM corpus.
