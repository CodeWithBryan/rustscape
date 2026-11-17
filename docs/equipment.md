# Equipment container and listener

`inventory/equipment.rs` ports the complete control flow of the pinned
EquipmentContainer, its private EquipmentListener, updateBonuses and stats
interface update. `equipment::create` constructs the actual 14-slot subtype and
native listener, both retaining their original canonical player. It shares the
existing Container identity, arrays, events and listener list. Clones, copy and
weak upgrades preserve that subtype; clearing its listener list does not remove
the class's own player field.

The original overloads differ. Container.add(item) and its three-argument
preferred-slot overload bypass the equipment override. The two-argument form,
array add, addAll and shift dispatch through it. Native `add` remains the base
preferred-slot form, `add_two` selects the override, and equipment::add/add_at
implement the equipment-specific overloads. An unbound world context returns an
explicit Binding error rather than a successful generic insertion.

Preserved behavior includes requirement checks, two-handed/shield selection,
original input versus copied item slots/names/cached definitions, removal before
space/hook checks, stack overflow and charge replacement, preferred inventory
slots and original rollback. A rejecting hook still permits the following
remove/add loops before rollback. Rollback uses the base one-argument add, and
can leave copies in both containers. The stack path forces listener updates even
when the outer fire flag is false. These quirks are reference behavior.

The actual native EquipmentListener captures event slots before sending a live
array, then updates appearance/weight/bonuses before processing cape, weapon and
shield slots. A missing weapon extension breaks that slot loop. Full refresh
sets attack speed and updates the weapon interface before sending inventory.
The container writer ignores the supplied context length for Item arrays and
writes their actual current length; copies can change that length. Exact
387:28/container94 full/partial frames use the owning world's NULL_ITEM identity.

Bonuses preserve wrapping sums, short/long arrays and failed prefixes, original
strength requirement and perk queries, double ceil followed by Java compound
assignment narrowing, DFS charge divisions and stat-label order. Stats capture
the bonuses array but read each element after preceding callbacks. No player,
item, definition, array or listener guards span a service callback.

`Property::Plugin` holds an actual native typed payload with object identity;
observation labels do not reconstruct or dispatch runtime plugins. Equipment
hooks pass the original current item. Arbitrary plugin graphs and their cycle
collection remain separate work.

415 JVM scenarios compile the entire unchanged EquipmentContainer class and
reuse freshly reproduced original Container/Item/Event and packet writers.
The original definition getRequirement is verbatim. Required hasRequirement,
Player, weapon/combat/appearance/Settings/perk, dispatch and interaction services
are recording adapters. Cases compare exact state and callback/failure order,
packet bytes, nullable owners, overflow, recursion, array replacement and
retained cached definitions. They preserve the prior 389 inventory cases.
Four additional native lifecycle regressions bind actual equipment to canonical
player fields and verify listener-only/list-only retention, the class's player
field after clearing listeners, and release of disconnected cycles.

Run equipment tests, inventory/ownership/listener/player binding suites, affected
world/mask/headless/example tests, fresh equipment-oracle and the affected existing
JVM gates. Use the local Cargo environment in AGENTS.md.

This is not live Equip integration. Full Player properties, weapon/combat pulse,
appearance body/animation preparation, requirement and content/perk services must
be independently implemented and bound before the game endpoint can instantiate
the subtype. The current development admission still uses base equipment data.
Full profiles/saves, bank behavior, accepted actions, complete plugin lifetime,
parallel collection and measured performance remain open. All 4,042 inventory
rows remain not_replaced.
