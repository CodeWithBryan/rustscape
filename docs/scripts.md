# Native entity scripts and interactions

`server::scripts` supplies Script state, all four concrete script forms,
ScriptProcessor, and the ContentAPI delay/range/queue/stop/restart helpers.
Every canonical actor, including bare players and NPCs, now owns one shared
processor with actual retained native function objects and node handles. Full
Player admission reuses that exact object. `world::script_runtime` joins the
existing tick's script phases, ticks/delays, actual entity clocks and walking,
destination/pathfinding, and typed native callbacks. UI, animator, geometry,
projectile and content effects use explicit owning-host bindings. The controlled
examples bind player modal queries, real animator reset, and projectile
line of sight against the owning scene. Complete content, events, and
closeAllInterfaces/dialogue remain required; those missing services return errors.
Canonical ActionLocks now bind delayEntity and the same walking, NPC, interaction,
input and button admission state; see [action-locks.md](action-locks.md).

## State and execution

Scripts retain executor identity, persist, state and nextExecution, initially
zero. Persist means restarting the state when execution finishes; it does not
save a callback or queue to disk. Interaction and UseWithInteraction carry
distance; QueuedScript and QueuedUseWith carry queue strength. Use-with forms
retain the used/with node references. `Runtime` and `RuntimeScript` use actual
`interaction_listeners::Function` objects and owning-world `NodeHandle`s. Numeric
executor/node parameters exist only in the independent recording adapters; no
observation ID selects native gameplay and no Java execution occurs at runtime.

The processor owns approach/operable/target/current handles, its indexed queue,
delay and interaction/range/persistence flags, and a shared destination location.
Cloning a processor aliases its actual fields. Its immutable entity and world
identity belong to that object; multiple processors can retain the same entity
without aliasing each other. Every core operation uses short field guards and
releases them before host callbacks or native payload destruction. Reentry uses
the same receiver while ContentAPI calls re-read the entity's canonical processor.
Active script means current, otherwise operable, otherwise approach. Successful
execution returns directly and leaves current pointing to the last script,
including when that script has left the queue. Callbacks that reset it can clear
that pointer. Non-player/absent-target early returns preserve the same state.

Executor failures log before reset. Reset clears interaction/current/range fields
and resets a player's animator, including AI players. It retains the queue and
global delay. Earlier effects remain visible if logging or animator reset fails.
The callback formatter is an explicit binding: the oracle uses deterministic
exception text to check framing and order, not native reproduction of JVM frames.

## Queues and movement phases

The queue supports WEAK, NORMAL, STRONG and SOFT. A queued strong script closes
player interfaces and removes weak scripts before execution, even if it is not
yet due. AddToQueue's strength argument controls its immediate interface effect;
the script's own strength controls later processing. They are not conflated.
Delayed entities skip non-soft scripts. Non-player queued use-withs are removed
and logged after the delay check but before their due-time check.

Adding schedules at max(world tick + 1, existing nextExecution). Processing
captures the array length, then fetches each current index. Callback removal can
skip a shifted entry or cause a later bounds error; appends are deferred past the
captured end. This is not iterator fail-fast behavior. Each due script is
scheduled for tick + 1 before execution, then increments state with i32 wrapping.
Finished non-persistent references are removed; persistent ones reset state.
Weak/normal removal and aliases retain the original shared-handle behavior.

Pre-movement drains queue passes until none execute, then checks stun, delay and
player modality before interaction handling. AI players bypass the modality
check. Post-movement first updates MOVEMENT to tick plus zero when running or one
when walking, then applies its own interface/chatbox gates. It handles failed
reach, normal interaction reset and inactive targets in source order. Non-player
returns occur at the original positions. Script callbacks observe every clock
sample and partially changed fields.

Interaction deadlines use nextExecution < tick; queue deadlines permit equality.
An inactive interaction target triggers reset, but the original method can still
continue executing the supplied script afterward. Facing occurs before the
processor reloads the script field, so callbacks can change which script runs.
Approach distance comes from the script's constructor distance, independently of
the mutable apRange flag. Operable distance uses the four cardinal neighbor tiles,
including plane, then line of sight. Approach distance uses the original 2D
distance arithmetic. Packed signed 15-bit x/y and wrapped plane data preserve
location aliases and boundary behavior.

SetInteractionScript compares executor equality before resetting. Equality is a
binding, allowing native function-reference equivalence rather than forcing
pointer identity. Equal execution can retain the earlier target. NPC/item
destination selection and scenery path results use world bindings. The scenery
move-near branch's early return and missing-path reset are preserved, including
partial state and callback effects before destination assignment.

All arithmetic retains signed wrapping. As in the source, repeating scripts can
keep preMovement running if clock changes or wrapped deadlines make every pass
due. No new game-rule budget is imposed here. The oracle uses bounded scenarios
and tests extreme deadlines with individual queue calls rather than hanging on
unbounded repeats. Complete world startup and content scheduling remain pending.

## Verification

```powershell
cargo test -p rs09-server --test scripts --test script_objects --test script_runtime --test owned_player
cargo xtask scripts-oracle --check --python <python> --java-home <jdk11>
cargo xtask script-runtime-oracle --check --python <python> --java-home <jdk11>
```

328 scenarios compile complete unchanged Script.kt, ScriptProcessor.kt and
Clocks.kt with verbatim ContentAPI helpers. Original location hash, distance,
equality and cardinal-tile algorithms run in a small location adapter. Entity,
node, path, LOS, interface, animator, lock and executor operations are explicit
bindings, not claims of their full implementations. Reference checkouts remain
unmodified, and fixtures pin source/adapter and output hashes. The independent
initial lock and native pin test preserve the first 295 observations byte-for-byte.
The additional 33 cases compare shared processor aliases, independent processors
for one entity, callback reentry, captured receiver identity and late API reads.

Tests compare script identities and fields, queue/current/approach/operable
handles, targets, destination/actor/shared coordinates, clocks, movement/lock
effects, callbacks, messages and partial errors after each action. Coverage
includes players/AI/non-players, every strength/form, overflow, queue aliases and
mutations, equality and reentrant reset/facing/path callbacks, API-created scripts,
modal differences and missing/inactive targets. The native port does not depend
on JVM reflection, JSON fixture executors or test-only adapters at runtime.

## Canonical ownership and remaining joins

The actor and canonical Player expose the same retained processor on every access.
A weak index keyed by actual entity allocation preserves identity across Player
admission and cannot root a retired actor. Admission performs an indexed lookup;
expired weak entries are swept once per collection. Each actor's ten integer
clocks are shared with its actual prepared animator; retaining the primitive
clock array does not retain its former actor. Its
physical owner/target/script fields and each script's actual function/use-node
fields participate in the existing owning-world graph. Each native object is
inspected once, while duplicate physical references are counted separately.
Externally retained processor, script or typed payload views keep their exact
captured players alive after disconnect. A detached stateless script does not
implicitly retain its former storing player. Unreachable typed processor/script
cycles are severed after node marking, with payload destruction outside guards.
Foreign or unscoped script objects are conservatively rooted and never cleared
by another world's collector. Arbitrary opaque-only cycles and concurrent GC
remain open.

Runtime admission helpers validate the actual world identity and node allocation
before callbacks or mutation; matching numeric node IDs do not establish identity.
Native dispatch regressions exercise real non-Copy functions and nodes, captured
execution across replacement, queued Void/use arguments, and callback/destructor
reentry. These tests use explicit failing bindings for unused world services.

The separate1064-case JVM host-helper corpus compares original Entity.delayed,
Player.hasModalOpen, InterfaceManager.isOpened/hasChatbox, Component.getId,
isStunned/finishedMoving and Node/Scenery center methods, including signed
boundaries, differing UI gates, packed coordinates and array-error prefixes.
The original selected bodies and complete Clocks.kt are unchanged; constructors,
field inputs and world clock remain declared adapters.

Four composed regressions per controlled movement example execute real queued
native functions through actual Entity ticks, nest an NPC phase during a player
callback, preserve captured stage/deadline/current handles, and test weak/normal/
reset events, exact animator clocks, delay/clock inequalities,
and foreign nested captures/arguments before body effects. Native callback bodies
retain actual capture fields and Rust function pointers; no observer IDs dispatch
them. Bare/NPC collection and foreign bank/PIN/processor/script captures have
additional canonical ownership regressions.

Concrete content handlers, complete interfaces/events, arbitrary Java
object APIs, modern movement arrivals and natural banker/booth admission remain
pending. closeAllInterfaces/dialogue is an explicit required failure in the
current host. The script line-of-sight binding uses the canonical actor size and
location and the original projectile traversal for the script destination tile.
See [projectile.md](projectile.md). Full retained animator lifecycle
after fixture metadata retirement and native method-reference equality beyond
installed anonymous function objects are also separate joins. No performance or gameplay-capacity claim follows from
these tests. Source-file replacement statuses remain `not_replaced` until runtime
acceptance of their complete responsibilities.
