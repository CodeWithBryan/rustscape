# Native action locks

`world::action_locks` ports the complete unchanged ActionLocks, Lock and LockElapse
algorithms. Every canonical actor owns actual shared lock state. Script delayEntity,
walking, NPC movement, legacy interaction, bank buttons and deferred input use
that same state. Callback bodies are actual typed native objects with retained
capture fields and Rust function pointers; numeric executors exist only in the
independent recording adapter.

Each Lock begins with expiration0, nullable message and nullable elapse event.
Messages retain immutable UTF16 data and identity. lock(ticks) reads expiration
before sampling the world clock for its signed wrapping comparison, and samples
the clock again only when assigning the new deadline. Shorter requests do not
blindly overwrite an existing longer lock. Indefinite lock computes MAX_VALUE
minus its own clock sample before entering that same method. isLocked uses strict
expiration > world tick; unlock sets only expiration0.

ActionLocks creates separate movement, teleport, component and interaction locks;
equipment initially remains null. Its default lock/unlock touch movement followed
by interaction. The original comment mentioning teleport does not match the code.
Equipment can retain another exact Lock object. Private custom keys preserve
nullable UTF16 equality; diagnostic map snapshots do not establish HashMap order.

Custom lock finds or inserts the actual object, sets its event even when null,
then applies the timing method. The two-argument overload clears the previous
event. Custom unlock captures the selected object, clears its expiration, checks
and reloads its current event, invokes it with the actual nullable Node and Lock,
then optionally removes the current key. A callback can remove/recreate the key;
the outer call still removes the replacement. Failure skips that final removal
and preserves all earlier effects. Merely passing the expiration does not invoke
LockElapse automatically.

No field/map guard spans clocks, callback invocation, native metadata or payload
destruction. Reentry retains exact receiver identity. Primitive views and stateless
action-lock/map objects do not implicitly anchor their storing actor. Actual
lock/callback/action/map fields are traced once per physical object and incoming
aliases counted separately. External lock, payload or map views preserve exact
captured actors; detached typed callback cycles are collected. Foreign/unscoped
objects are conservative roots and are never cleared by a local world's collector.
Native admission validates supported capture fields and typed Node/Lock arguments
before body effects. Arbitrary opaque-only cycles and concurrent GC remain open.

The309-case JVM corpus compiles all three original files unchanged. It compares
default/custom/equipment behavior, nullable keys/messages/events, aliases,
callbacks, clock sampling, signed boundaries, reentry and completed failure
prefixes after every operation. Reflection is a private-field observer. GameWorld
clock samples/failures and callback effects are declared bindings. The independent
initial309 lock prevents silent rewriting of those observations.

Native regressions additionally cover actual actor ownership/collection, foreign
capture rejection and safe destructor reentry. Composed controlled-example tests
exercise shared script/walking/interaction deadlines, actual native LockElapse
replacement/failure prefixes, and bank amount/button packets blocked until the
original expiration tick. Blocked input preserves the same pending callback and
attributes. The old NPC-only `lock_until` shadow is removed.

The UI context now borrows State and Host directly. Skill appearance reads/writes
use prepared masks keyed by the canonical node; inventory refresh uses the same
canonical Player receiver and outgoing frame store. A composed regression removes
the visual actor lookup table and still performs actual game configuration, skill
and container packet generation. This prepares real UI closure during a script
tick; DialogueInterpreter and ordered closeAllInterfaces remain required joins.

```powershell
cargo test -p rs09-server --test action_locks --test owned_player
cargo test -p rs09-server --example movement_session --example npc_session --example player_session
cargo xtask action-locks-oracle --check --python <python> --java-home <jdk11>
```

Full content registration, modern arrivals, UI/dialogue closure, combat actions,
complete profiles/world startup and performance remain separate work. Script
projectile line of sight is now bound; see [projectile.md](projectile.md).
This port does not establish complete game replacement or player capacity.
