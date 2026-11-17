# Native door rules

`server::world::doors` ports DoorActionHandler's door decisions, geometry and
anonymous autowalk pulse bodies, together with DoorConfigLoader's load/lookup
behavior. Scene replacement, collision, walking, entity locks, player services,
audio delivery and scheduler submission remain explicit native bindings. This
library is not connected to live login/world startup yet.

## Configuration

The loader preserves JSON-simple coercions, Kotlin integer/boolean conversion,
defaults and row order. Each row creates distinct forward and reverse records.
Later rows overwrite either key, including repeated replacement IDs and self
replacements. Loading retains unrelated existing keys. Earlier writes survive a
later row failure; a logging failure happens after all rows have committed.
Returned records are shared handles, so references retained before a reload keep
their old objects. Registry clones share the registry itself.

The complete 514-row pinned configuration is retained in
`fixtures/doors/door_configs.json` and checked against the original Git object
when reproducing fixtures. Every resulting record is compared with the JVM.
The original game code consumes this registry through lookups, not iteration;
arbitrary Java HashMap APIs/iteration are outside this native registry's claim.
Native file loading accepts UTF-8 and bounds JSON at 64 MiB. The oracle selects
UTF-8 for the original platform-dependent FileReader; the pinned data is ASCII.
General charset/reader-reflection behavior is not established by these checks.

## Decisions, geometry and pulses

Second-door search uses west, east, south, north priority and exact name matching.
The original ID exceptions for castle/maze doors, already constructed replacements,
close actions, missing configuration, metal/fence audio and autowalk are preserved.
Ordinary opening requests a 500-tick restoration; the autowalk pulse requests a
client-side two-tick replacement. The replacement binding must implement the
original SceneryBuilder behavior before those requests affect the world.

The native rules preserve straight/diagonal/double-door offsets, fence ID swaps,
the special 4577/4578 and 36917/36919 paths, wrapper references and the direction
stored at construction even when rotation changes later. Destination selection
retains move-near path queries, reference locations and the original short circuits.
Arithmetic uses Java wrapping and signed remainder.

Autowalk doors and fences differ in when they set the in-use charge of 88.
Their first pulse opens and queues walking; the second clears the listener and
charge and performs the original diary checks. The 35549/35551 condition retains
its original operator precedence. Logout restores the captured location handle.
Callbacks can change state or fail between effects; those partial changes remain.
The two sounds of a selected autowalk audio branch stay paired if a callback
changes the configuration between them.

`DoorPulse` contains the existing native base Pulse plus native captured state.
The world adapter submits its base handle to the native runner and calls
`pulse_body` from its pulse binding. No JVM, reflection or JSON executor runs in
production door code. The regression harness drives base update behavior against
the complete original Pulse; the live world runner/startup binding remains pending.

The exported Scenery state is the metadata consumed by these rules, with shared
locations, replacement and wrapper references. Definition creation is a required
factory binding. This does not replace all Scenery/Constructed/Node methods,
attributes, definition decoding, child configuration or destruction/restore pulses.

## Verification and remaining world integration

```powershell
cargo test -p rs09-server --test doors
cargo xtask doors-oracle --check --python <python> --java-home <jdk11>
```

463 scenarios contain 2,841 commands. They compile unchanged DoorActionHandler,
DoorConfigLoader, Scenery, Constructed and Pulse with explicit world adapters.
Original location hash/distance/equality and direction methods run in small
adapters. Tests compare configurations, retained records, node and replacement
identities/fields, requests, callback order, locations, charges, listeners, path
requests, pulse state, diary calls and partial errors after every command.

Coverage includes all 514 original configuration rows, reload aliases/failures,
every rotation, both path-result values, diagonal/double/fence behavior, special
IDs, player/non-player paths, reentrant mutations and callback exceptions. JVM
exception classes are normalized for comparison; JVM stack-frame text is outside
the claim. Source/adapter/data hashes are pinned and the references stay unchanged.

Replacement requests in this corpus use an explicit SceneryBuilder adapter.
Separate native LandscapeParser, chunk-storage, ChunkFlags and builder-restoration
corpora are now described in [scenery.md](scenery.md). Joining those kernels with
this handler for an actual collision/restoration trace still requires world
integration. Movement and full entity services need further ports and tests.
A constructed packet is not a real gameplay trace.
The selected acceptance target is the actual cache door 12348 at (3207,3217,0),
shape 0, rotation 1, with replacement 10327. Its rule/collision/packet replay and
live visual/headless sessions remain pending. No throughput/player-capacity or
whole-game completion claim follows from this milestone.
