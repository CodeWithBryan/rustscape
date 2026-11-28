# Fletching logs with a knife (`world::fletching_logs`)

Scope: the pinned `content/global/skill/fletching/log` (four files) with the
`Zones` and `AchievementDiaryAttributeKeys` objects of
`content/global/skill/fletching` it reads.

| original | here |
|---|---|
| `LogCraftInfo` (22 constants), `logIds`, `forLogId` | `CRAFTS`, `log_ids`, `for_log_id` |
| `GrammarHelpers.makeFriendlyName`, `aOrAn` | `make_friendly_name`, `a_or_an` |
| `LogCraftableListeners.sendLevelCheckFailDialog` | `send_level_check_fail_dialog` |
| the handler's `create` and `playerMeetsInitialRequirements` | `create` |
| `CraftItemWithLogScript.invoke()`'s queued lambda | `script` |

The bodies are functions over `fletching_logs::Host`; every call the original
makes outside these classes is a required method with no default. The
`SkillDialogueHandler` the listener opens is `dialogue_dsl::skill`'s.

Stringing and arrows are `world::fletching_assembly`
(docs/fletching-assembly.md); bolts, darts, crossbow limbs, gem tips and
the grapple are `world::fletching_ammo` (docs/fletching-ammo.md).

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
- `cargo test -p rs09-server --lib fletching_logs`: a recording host written
  from the same sources checks the table's grouping, the grammar helpers,
  `create`'s level and quest checks, the script's calls in order, its clock
  wait, its level recheck, the ogre shaft roll, the comp bow's wolf bones
  and the Seers' Village attribute. It shows agreement with one reading of
  the sources.
- The live world's `gather` scenario (`fletchlog`) uses a knife on logs,
  reads the four names of the make-X dialogue (interface 305), is refused a
  shortbow at level 1, and cuts three logs into 45 arrow shafts with
  "Make 10". No live check reaches another log, the amount prompt ("Make
  X"), the ogre crafts or the diary attribute.

## Kept behaviour worth knowing

- `aOrAn` is `StringUtils.isPlusN`: "an" before a vowel and before an "h"
  not followed by "e".
- The comp bow's log is removed before its wolf bones are asked for; a
  player whose bones left in between loses the log.
- `addItem`'s answer is ignored, so a full inventory loses the product.
- `RandomFunction.random(2, 6)` gives 2 to 5 ogre arrow shafts.
- The third Seers' Village zone is written with its corners swapped;
  `ZoneBorders` orders them, and none of the three compares the plane.

## Live world

`world/live/items/fletching.rs` registers the listener at boot when
`LogCraftableListeners` is in the scan, and `SkillDialoguePlugin` under
`SKILL_DIALOGUE` when it is in `scan.initializable`. It is the first plugin
in the live dialogue registry; stringing and arrows open it too.
The quest stages are the ones the save holds for the quests' repository
indexes (Big Chompy Bird Hunting 35, Zogre Flesh Eaters 40); no quest is
registered in this world, so nothing advances them.
