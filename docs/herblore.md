# Herblore: cleaning herbs and mixing potions (`world::herblore`)

Scope: the pinned `content/global/skill/herblore/HerbCleanListener.kt`,
`PotionMixingListener.kt`, `Herbs.java`, `UnfinishedPotion.kt`,
`FinishedPotion.kt`, `GenericPotion.java` and `HerblorePulse.java`, with
`SkillPulse`'s `start`, `pulse` and `stop`.

| original | here |
|---|---|
| `Herbs` (20 rows), `Herbs.forItem` | `HERBS`, `herb_for_item` |
| `UnfinishedPotion` (21 rows), `forItem` | `UNFINISHED`, `unfinished_for_item` |
| `FinishedPotion` (54 rows), `getPotion`, `canMake` | `FINISHED`, `finished_potion`, `can_make` |
| `GenericPotion.transform` (both) | `Potion::of_unfinished`, `Potion::of_finished` |
| HerbCleanListener's "clean" lambda | `clean` |
| PotionMixingListener's two `onUseWith` lambdas, `mixPotion` | `Listener`, `use_with`, `mix` |
| the skill dialogue's `calculateMaxAmount` and `create` | `get_all`, `create` |
| `HerblorePulse` (`checkRequirements`, `reward`, `handleUnfinished`, `handleFinished`) | `Pulse` |
| `StringUtils.formatDisplayName` | `format_display_name` |

The bodies are functions over `herblore::Host`; every call the original
makes outside these classes is a required method with no default.

Elsewhere: grinding (`GrindItemPlugin`, `GrindingItem`), herb tar
(`HerbTarPlugin`, `HerbTarPulse`, `Tars`) and barbarian mixes
(`BarbarianMixListener`, `BarbarianPotion`) are docs/herblore-tools.md.
Not ported: swamp toads (`SwampToadPlugin`). Drinking a potion is
docs/consumables.md.

## Evidence, and what it does not show

- `fixtures/herblore/oracle.jsonl` (1,457 cases, about 9,500 operations)
  is recorded from the unmodified sources above with the unmodified
  `SkillPulse`, `Pulse`, `GrindingItem`, `Quests` and `IntType`, compiled
  with Kotlin 1.8.20/JDK 11 (`oracle/herblore/generate.py`, `cargo xtask
  herblore-oracle`), and `cargo test -p rs09-server --test herblore`
  replays every case over this module and compares each result and each
  recorded call in order. One operation compares the three tables row by
  row and another the three registrations with their ids.
- What the corpus does not establish (see `scope` in
  `fixtures/herblore/provenance.json`): the player, item names, the
  inventory, the level, quest stages, attributes, messages, `replaceSlot`,
  the experience, the sound, the animation, `hasRequirement`,
  `Consumables.getConsumableById` and `PulseManager.run` are recording
  adapters answered from declared case state, and `RANDOM.nextInt` is a
  declared draw. The driver calls `SkillPulse.pulse()` and `stop()`
  itself, not `Pulse.update()`: the corpus shows the delay a pulse leaves,
  not the ticks it runs on. The use-with lookup is a declared model and
  the skill dialogue an adapter that records `open()`. The generator is
  not in `oracle/ci.toml`: CI replays the committed fixture but does not
  regenerate it.
- `cargo test -p rs09-server --lib herblore`: two hand-read checks.

## Kept behaviour worth knowing

- Cleaning asks `requireQuest` (Druidic Ritual at stage 100 exactly);
  the pulse asks `QuestRepository.isComplete` (stage 100 or more) and has
  its own message.
- Cleaning locks the player for a tick before any check, and an item that
  is no dirty herb answers `true` with nothing done.
- With one of either item the pulse starts at once; otherwise the skill
  dialogue opens with the product, and "All" is the amount of the base.
- An unfinished potion made with a vial of water is one a tick with no
  experience, except a single one, which waits three ticks. Everything
  else, the four coconut milk bases included, goes through the finished
  branch: three ticks each, `addExperience` (0.0 for coconut milk) and the
  "You mix the ... into your potion." message.
- A finished potion's own requirement is asked before `mixPotion`, with
  "You're not sure what effect this might have." when it fails.
- The skillcape perk (`cape_perks:brewmaster`) draws `random(100) < 15`
  and then gives the first id of the product's consumable, which for a
  three-dose potion is the four-dose one; the message is sent even when
  the product has no consumable.
- The pulse never checks free inventory space, and its animation resets
  when it stops.

## Live world

`world/live/items/herblore.rs` registers HerbCleanListener's generic
"clean" option and PotionMixingListener's two use-with registrations when
the class scan names them, opens the skill dialogue through
`dialogue_dsl::skill` and runs HerblorePulse on the player's pulse
manager, writing the delay the pulse sets back to its handle. The binding
has no JVM corpus.

- Quest stages are the ones the save holds (Druidic Ritual, Zogre Flesh
  Eaters and Heroes' Quest by their repository index). Nothing in this
  world advances a quest, so a character without a saved Druidic Ritual
  cannot use Herblore here.
- `hasRequirement` answers false by name (QuestRequirements is not
  ported, no quest is registered): Serum 207, the super fishing explosive,
  Shrink-me-quick, magic essence and the eight Guthix balance steps are
  always refused with the original's message.
- Nothing in this world sets the skillcape perk's attribute.
- `replaceSlot`'s second attempt (the herb gone from its slot: the dupe
  alert and the removal from the equipment) is refused by name.

The live world's `gather` scenario covers it with two characters at
Lumbridge. `herbnovice`, without the quest, is told to complete the
Druidic Ritual and keeps the grimy guam. `herbman`, with the quest saved
and Herblore 3, is refused a grimy ranarr by its level, cleans two grimy
guams (5 experience), uses one on a vial for the dialogue and "All" (two
guam potions (unf), the third vial left, no experience), mixes one attack
potion with "Make 1" and, with one of each left, the second without a
dialogue (50 experience). No live check covers a coconut milk base, a
potion with a quest requirement, the skillcape perk, a level refusal in
the pulse or a pulse another action interrupts.
