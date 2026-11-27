# Item-on-NPC basics (`world::item_on_npc`)

Scope: two listener classes of the pinned
`content/global/handlers/item/withnpc` package, with the two tables they
read. Nothing else of the package is ported.

| original | here |
|---|---|
| `FeedOnStrayDog` (`onUseWith(IntType.NPC, bones + consumableMeats, *dogs)`) | `Class::StrayDog`, `Interaction::StrayDog`, `stray_dog` |
| `FeedFaladorSheepdog` (`onUseAnyWith(IntType.NPC, SHEEPDOG_2311)`) | `Class::Sheepdog`, `Interaction::Sheepdog`, `sheepdog` |
| `Bones.values()` ids and `Bones.array` | `BONES`, `bones_array()` (the HashMap walk of `global_items2::tables::java_hash_order`) |
| `content.data.Meat` ids and states, `INEDIBLE_RAW`/`EDIBLE_COOKED` filter | `MEAT`, `consumable_meats()` |

The bodies are functions over `item_on_npc::Host`, whose every method is a
call the original makes outside the lambda (ContentAPI `removeItem`,
`sendMessage`, `sendDialogue`, `animate`, `sendChat`; the nodes' ids and
names; the two `lowercase` flavours). `define` registers a class in the
native `InteractionListeners` registry as the original's `defineListeners`
does (`add_use_arrays` for the stray dogs, `add_any` for the sheepdog).

## Evidence, and what it does not show

**There is no JVM differential corpus for this module.** It was ported by
reading `FeedOnStrayDog.kt`, `FeedFaladorSheepdog.kt`, `Bones.kt` and
`Meat.kt`:

- `cargo test -p rs09-server --test item_on_npc`: a recording host written
  from the same sources checks each branch's calls in order (plain, burnt,
  ogre-family, big, dragon bones; meat; a failed removal; the casts), the
  two tables' sizes and filter, and the registry rows `define` writes. It
  shows the port agrees with one reading of the sources, not with the
  original classes running.
- `live_world`'s `usedog` (docs/live-world.md) feeds bones to a Varrock
  stray dog through a socket: the walk, the removal and the message.

Not shown: the order of `Bones.array` against a JVM (it only decides which
duplicate registry row is written last, and every row carries the same
function), the dogs' force chat and animation on a client, and the
sheepdog's dialogue box against the original interface packets.

## Kept from the sources

- A stray dog whose food cannot be removed gets nothing: no message, no
  animation, no chat; the lambda still answers true.
- The stray dog's `Bones.forId(used.id)` test comes before the meat test;
  burnt bones (528), the three ogre-family bones (4830, 4832, 4834), big and
  jogre bones (532, 534) and dragon and wyvern bones (536, 6812) have their
  own line and bark; other bones get the plain "Woof".
- The sheepdog refuses burnt bones with everything else ("The dog doesn't
  seem interested in that." and "Grrrr!"); its bones line lower-cases the
  item's name in the default locale, the stray dog's in `Locale.ROOT`.
- `Meat` lists 2140 and 9982 twice with different states; the filter keeps
  both rows, as the original's does.

## Live world

`rs09-server --world` registers both classes at boot
(`world/live/items/everyday.rs`, in the class scan's order) and runs the
lambdas from `InteractionListeners.run`'s use-with arrival. See
docs/live-world.md for the services bound and the ones refused.

## Not ported

The rest of `withnpc`: CatOnArdougneCivilian, CiderOnForester, GCItemOnCat,
GertrudeCatUsePlugin, HatEasterEgg, MistagEasterEgg,
PoisonChaliceOnKingArthur, RopeOnLadyKeli and ZooknockListener (quests,
cats and Easter eggs: each needs content this repository does not have).
