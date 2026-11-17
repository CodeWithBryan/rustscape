# Modern interaction listener dispatch

`world/interaction_listeners.rs` ports the complete InteractionListeners registry
and its three run paths. This is a required dependency for natural banker and bank
booth interactions. It is not yet installed in the canonical live player/scene
runtime; the current live examples still expose their explicitly empty listener
and script services. Existing legacy InteractPlugin scheduling remains separate.

The1278-case JVM comparison compiles complete unchanged InteractionListeners.kt,
InteractionListener.kt, IntType.kt and Script.kt. Entity, scripts, locks, event,
zone, movement construction/run and callback bodies are mutable recording services.
The recorder invokes captured pulse bodies explicitly; those calls are not a
combined ScriptProcessor, collision-aware movement or full-world proof. The first
1168 observations are independently pinned. The declared kotlin-reflect1.5.20
dependency is pinned separately; a discarded stdlib-only draft produced different
duplicate-registration class text and is preserved in the investigation folder.

Registrations retain actual native function objects through the shared opaque
object header. Their typed payloads select behavior; observer labels never do.
Aliases retain identity, and headers expose their actual capture metadata for
later owning-world graph integration. New payloads with strong fields must trace
those fields. The registration's class name is real program metadata required by
the original instant-class rules, rather than a callback lookup key.

Preserved behavior includes scalar registration validation versus bulk overwrite,
partial registration on duplicate options/IDs, reversed item-use fallback, special
player-use lookup, specific/any/wildcard priority and retained wildcard iterator
modCount checks. A matching predicate may add a row and return its captured handler;
a false result reaches the next iterator check and fails. Private map views are
sorted diagnostic observations; their order does not establish JVM HashMap order.

Generic interaction metadata registration stores option casing unchanged even
though lookup uses ROOT lowercase. Legacy lookup and destination registration use
the current default locale. Generic destination lookup does not lowercase. The
IntArray use-metadata overload interpolates the array identity rather than its loop
element. That platform-specific identity hash is an explicit native API input;
only its private snapshot text is normalized in the oracle.

Normal actions remove weak scripts, check interaction locks, resolve a callback,
publish the ROOT option attribute and dispatch the default-locale event. Metadata
lookup follows that event and can observe newly registered handlers. It constructs
a script, captures the current script receiver for installation, then captures the
current pulse manager before movement construction. Its scenery destination flag
is null and it does not perform the legacy movement-lock check.

Legacy callbacks retain their chosen function and destination override. Arrival
reads the current zone, applies its veto, faces the current target location and
invokes the captured callback. Normal actions ignore callback booleans; direct
item-use and equip calls return them. Deferred use actions retain the original
used/with objects and flip flag, check the used-item cast after obtaining the zone,
then perform late ID reads for dispatch. Failure prefixes and reentrant receiver,
registration, result and ID changes match the JVM. No registry or wildcard guard
spans a gameplay callback.

Canonical Player ScriptProcessor ownership and actual native function/node
objects now exist; see [scripts.md](scripts.md). Its real tick/pulse phase bridge
now uses actor clocks, queues and typed native functions. Script projectile line
of sight now uses the owning scene. Required next joins are remaining
close/dialogue and modern-context services, MovementPulse arrival on
the existing world runner, real event/zone/lock
services and bank booth/banker content registration. PIN, profiles, dialogue trees,
use-with content, full game replacement and measured performance remain open.

```powershell
cargo test -p rs09-server --test interaction_listeners --test scripts --test interaction_runtime
cargo xtask interaction-listeners-oracle --check --java-home runs/jdk11/jdk-11.0.32.1+1
```
