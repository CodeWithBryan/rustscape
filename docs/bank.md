# Native bank containers and item ownership

The Rust Bank implements the complete pinned BankContainer and private
BankListener control flow over actual native Container objects. The546-case JVM
corpus compiles the unchanged original class, original IfaceSettingsBuilder,
Player.hasItem body, Item/Container/Event and ContainerContext/ContainerPacket.
The initial488 bank observations remain byte-for-byte unchanged. The additional
58 observations exercise item ownership and getter/callback short circuits.

The port preserves both open overloads, their different listeners and interface
settings, actual captured close callbacks, deposit box operations, note conversion,
charges, maximum-add virtual dispatch, tab arithmetic/collapse, last-X state,
private listener packets, retained item/event/array identities and failed/reentrant
prefixes. Bank packets retain their Item[] and captured slots. The original full
packet writer ignores context.length for Item[]; the native encoder does too.
Settings0-to10 tab mapping still sends/stores the original argument's values.

Player/interface/PIN/Ironman/world settings, var/attribute APIs and deposit UI
generation are explicit recording services in the corpus. This does not complete
those services or bank UI/content startup. Native Bank operations require their
owning services; missing services do not return fabricated success.

Canonical players now own three distinct real Bank objects: primary, secondary
and drop log. Active bank selection and Player.hasItem use the actual inventory,
current active bank and equipment in that order; the drop log is not searched.
Ground Pickup's cape/pouch restrictions now use that actual query. A portable
opcode66/arrival regression in both controlled examples checks sacred cape
rejection from primary/secondary/inventory/equipment, while inactive-bank and
drop-log capes permit Take. Its definitions/terrain are explicitly authored test
inputs, not real-cache visual fidelity evidence.

Each bank has a physical player field and a separate final listener reference.
The ownership tracer counts each object/field once, including the listener after
its exposed list is cleared. External bank/listener handles retain the exact
disconnected player across wire-index reuse; releasing them collects the cycle.
Detached primitive tab arrays do not retain that player. Bank metadata uses an
optional heap allocation so ordinary containers store only an optional pointer;
the shared definition table and mutable definition handles are retained. No field
guard spans a host callback. This is not a measured full-game capacity claim.

The default capacity496 is the pinned ServerConstants value. Bank mode symbols
come from the separately pinned official ConstLib1.4.28 artifact satisfying the
original POM's `[1.4.0,)` range: insert7000/note7001. The old bundled1.4 jar has
no Vars class. oracle/bank/constants.lock.json records artifact bytes/hash and
scope. It does not establish which exact dependency a historical build resolved;
upstream.lock.json and earlier fixture provenance are unchanged.

```powershell
cargo xtask bank-oracle --check --java-home runs/jdk11/jdk-11.0.32.1+1
cargo test -p rs09-server --test bank --test owned_player
```

The native PIN manager/status and button program now have a separate855-case
original comparison and canonical ownership regression; see docs/bank-pin.md.
Their actual component/packet/profile/content services remain separate work.

Actual BankListener/var/interface dispatch through the canonical player runtime,
live bank PIN behavior, bank interface content/scripts, complete profile persistence,
bank opening via bankers/booths and live bank frames remain pending. Development
admission still starts with empty banks. This is not a playable-banking or full
Player/server completion claim. Performance improvements need measured workloads.

Canonical BankListener/player-variable and controlled UI service bindings are now
joined; see [bank-runtime.md](bank-runtime.md). This does not complete bank content
registration, banker/booth admission, PIN entry or live transaction acceptance.
