# Core player profiles and entity timers

`server::player::core_data` now holds inventory, equipment, two banks, bank tabs,
the bank selector, location, varps and their save flags. Its native parse/save
paths reproduce the original core profile methods. `server::timers` supplies
timer state, manager operations and base persistent-timer serialization.

This is supporting library work. Startup/login does not yet construct a complete
Player, load a complete character file or enter a running world. Concrete bank/
equipment actions, packet listeners, gameplay timers, registry/entity startup,
the world clock/scheduler, and other profile sections still
need porting and integration. All source inventory entries remain `not_replaced`.

## Character state and persistence

Inventory/equipment dimensions are 28/14, with default ID sorting. Banks use
ALWAYS_STACK and HASH, 11 tab indices and an explicitly supplied capacity; the
reference default is 496. World startup supplies listener identities and the
initial location. This is the core field initialization boundary, not the complete
original Player constructor or concrete listeners. `bank()`/`bank_mut()` select the
same bank as the original `useSecondaryBank` flag.

Parsing performs the original upfront casts before mutating tabs, then selects
the bank and loads inventory, primary bank, secondary bank and equipment in that
order. A missing secondary bank is an empty array. Existing containers are not
cleared by a load. Location parsing converts every comma-separated token before
using the first three, then preserves signed 15-bit x/y packing and plane wrapping.
Varps and save flags update next; timer parsing runs last. Failures leave all
earlier changes visible, including individual tab/container/varp writes.

Saving uses each item's own slot/index rather than its physical array index.
Both banks and all tabs are saved. Nonzero varps with a true save flag are written
in original integer HashMap iteration order. The new core object remains detached
until every section and timer callback succeeds; failure preserves any previously
published `core_data`. Callback changes can affect live state without retroactively
changing sections already serialized. The shared legacy JSON DOM reproduces
JSON-simple stringification and HashMap field order, including replacement keys.

## Timer behavior

Registering invokes `beforeRegister`, queues the same timer handle, then invokes
`onRegister`. Processing first removes queued removals. Player modal/script-delay
checks run once per processing pass; NPCs skip them. Due soft timers can run while
a player is busy. Each due timer samples the clock in the original order.

Pending timers become active after the active pass, with their first execution
set to initial delay plus the current tick. Removing a timer sets its execution
to `Int.MAX_VALUE`, queues removal and catches removal-hook failures. Death only
examines active timers with ClearOnDeath. Duplicate handles/identifiers remain
possible; lookup selects the last match and returns absent if that match is
queued for removal. Mutable-list iteration preserves ArrayList fail-fast behavior,
including mutations that shorten the list exactly to the iterator cursor.

The original manager does not update `lastExecution`; the native manager retains
that behavior. Scheduling and remaining-time arithmetic wrap as signed i32.
Persistent base parse changes `runInterval` from `ticksLeft`; activation sets the
next execution later. Saves visit active then pending timers, including timers
queued for removal. Duplicate saved identifiers overwrite earlier values.

All clock, entity, registry/factory, content, logging and error hooks are explicit
`Context<S>` bindings. The state parameter lets a callback mutate the owning
character and its live manager without swapping out the manager. Class identity
and persistence are factory metadata; the manager does not perform reflection or
load a JVM. Identifier queries are case sensitive. Registry lookup policy and
subtype predicates belong to the native content registry when that is ported.
The [native registry](timer-registry.md) now implements registration, factories,
automatic timer installation and spawn/get-or-start wrappers; concrete timer
content and world startup remain pending. Default timer equality here is handle identity; arbitrary content equality
overrides and mutable JSON aliases held by content are outside this claim.

## Verification

```powershell
cargo test -p rs09-server --test timers --test core_data
cargo xtask timers-oracle --check --python <python> --java-home <jdk11>
cargo xtask core-profile-oracle --check --python <python> --java-home <jdk11>
```

The composite check reproduces all 242 timer scenarios and 389 item/container
scenarios before generating the 302 core profile scenarios. The original complete
TimerManager, RSTimer, PersistTimer, TimerFlag, Item, Container and event classes
execute unchanged. Core profile and location parsing bodies are verbatim selected
methods, with the original packed LocationHash. Selected Player core fields and
getters and the BankContainer/EquipmentContainer constructors are verbatim;
their concrete listeners are fail-on-use adapters. JSON-simple 1.1.1 performs actual
DOM parsing and output, and original Kotlin reflection constructs fresh test
timer instances. Fixture provenance pins source/adapters and fixture hashes.

World/content/registry factories are controlled adapters. Core constructors use
the original dimensions/default sorting and fail-on-use listeners: an unexpected
packet callback fails the oracle. The suite observes sparse container contents,
event slots, tabs, selectors, location, varp order, flags, timer identities/lists,
callbacks, every clock sample, output bytes and partial failures after each step.
Tests include callback edits to character state during timer parse/save/run,
overflow, busy players, queued activation/death/removal, duplicate IDs, malformed
fields, missing secondary-bank migration and failed save publication.

The shared JSON parser retains the existing documented 64 MiB/depth limits and
unpaired UTF-16 rejection. Filesystem encoding and complete save-file I/O are not
exercised here. No player-capacity, tick-throughput or full-profile parity claim
follows from this suite; native performance must be measured after world wiring.
