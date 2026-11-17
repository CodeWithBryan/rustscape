# Shared containers and captured player inventory

The production Container now retains one shared state object. Cloning a handle
means another reference to the same container. Array replacement, event reset,
kind/capacity/sort and listener changes stay attached to that object. Holding
only the old item array is a different operation: it keeps that array after the
container replaces it, just as the original Java reference does.

The item and event arrays remain shared native objects. Current getters read
the current container fields; they do not reuse a snapshot taken at capture.
copy_from uses actual receiver identity, so copying from another reference to
the same container retains the original self-copy behavior of clearing it.
The methods never retain a state or list lock across a listener/world callback.

listener_list returns the actual exposed list. Add/remove/clear update its
structural modification count, including clearing an empty list. Set replaces
the selected reference without changing that count. Callback traversal captures
the list and expected count, checks the live length and reads each next listener
late. It preserves the original early exit when the cursor equals the new length.
listeners is explicitly a diagnostic value snapshot and is unsuitable for live
listener mutation. Each list element now retains an actual native listener object;
equal payloads do not make distinct objects equal. Test-only labels never select
runtime callbacks. The immutable object header uses one Arc allocation and typed
receiver projection preserves that same allocation, without locking callbacks.

The new oracle compiles the complete unchanged Item, Container, ContainerEvent,
ContainerListener and enum classes using the existing inventory oracle assembly.
Only the recording driver gains alias/reentry/list-view operations. Its207 cases
and 5,172 command snapshots compare object/array/event aliases, nested calls through
different references, list mutation, retirement/reuse and partial failures.
The existing 389 inventory cases and their provenance remain byte-identical.
The extended native replay also keeps real container handles in its table instead
of removing a value during a callback or recognizing self-reference by numeric ID.

The subsequent native-object corpus adds 104 original JVM scenarios / 2,066
snapshots for distinct listener objects with equal recording labels, aliases,
retirement/rebinding, direct refresh and exposed-list callback edits. Both prior
389/207 corpora stay unchanged. See container-listeners.md for that scope.

Player inventory integration now has an explicit Capture component containing
the exact canonical owner, inventory, equipment, Settings, NULL_ITEM and listener.
The canonical actor now owns one record retaining the actual definition table,
body masks, familiar flag and packet counter; UI metadata holds a view of it.
Settings/equipment views retain their owner. A base inventory/list follows its
actual listener edges, so clearing the list can release the player while the
empty container remains observable. The internally stored fields use unowned
edges; external views do not form an unconditional parent/child Arc cycle.
Callbacks use these retained receivers after the UI metadata table removes them;
they do not look up a new player by a recycled wire index. Captures check their
actual world domain before performing any operation.
Standalone retained listener objects and typed receivers can recover these fields
from the original canonical actor. See owned-player.md for the graph and the
distinct-player packet/weight callback comparisons.

The live-example regression removes a player, creates a replacement with the
same wire index, and mutates the old inventory through its original listener.
Only the old weight, familiar/body state and counter change. The delivery phase
also checks that the canonical node still occupies the wire index, so old frames
cannot reach the replacement's session. All old fields and nodes retire after
the last captured inventory/Settings reference drops. A second-world check rejects
matching numeric IDs before entering the operation.

Run the container_ownership, inventory, inventory_listener, player_inventory,
settings and core_data tests plus all three example targets, then reproduce
container-ownership-oracle and the affected existing JVM corpora. The new oracle
also checks the complete existing inventory corpus before running its new cases.
Use the local Cargo environment settings and related gates in AGENTS.md.

This completes neither full Player/profile construction and saving nor bank/
equipment subtype actions/listeners, familiar gameplay, full listener-object
graph collection, content callbacks or packet-to-Pickup acceptance. Actual native
listener values retain their own receiver fields, but the complete Player and
general plugin backreference graph is not yet bound. The UI is a bounded
development binding and its core capture is a
component rather than a complete profile. There is no measured speedup, capacity
or parallel simulation claim. All source inventory rows remain not_replaced.
