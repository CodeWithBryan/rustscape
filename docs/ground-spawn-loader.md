# Ground spawn configuration

`world/ground_spawn_loader.rs` implements the complete loader's parsing and
publication flow using the actual native GroundSpawn and GroundItemManager.
The pinned 22,826-byte configuration contains 177 rows and initializes 717
spawns. The joined oracle compares every resulting item, subtype, manager/chunk
list, flag and service-call observation with the original unchanged Kotlin.
Region lookup, definitions/plugins, player services and logging remain explicit
world dependencies; these tests do not populate a running production world.

The entire JSON document is parsed before initialization starts. Each row has
its own exception boundary. A later invalid location leaves earlier items in
that row initialized, skips the rest of that row and continues with the next
row. Exceptions in the error logger itself escape. The final success count is
logged after all rows, including when some rows failed.

`loc_data` splits on every hyphen, including minus signs. Braces are removed,
extra comma-separated tokens are ignored and integer tokens are not trimmed.
The respawn rate is parsed first, then the amount and input Item are constructed,
then x/y/plane are parsed and the subtype is initialized. Integer grammar and
JSON-simple coercion/stringification use the shared JDK-compatible libraries.

The loader retains a shared reader field. Failed opens leave the old reader;
parse failures leave the new one. Reentrant loads share that field while the
outer call continues over its already parsed local document. `load_config`
concatenates the prefix literally, including the original `null` prefix.
File loading is bounded to 64 MiB and currently supports valid UTF-8 input, as
selected in the JVM oracle. Other default charsets, malformed UTF-8 replacement
and parser read-ahead after large malformed input remain explicit I/O gaps.

The diagnostic GroundSpawn string now reads its cached Node/Item name, current
rate and location, including nulls. It does not read a later definition name.
The loader oracle's constructor/name dependency is corrected to capture the
original constructor name before callbacks; the prior subtype corpus remains
unchanged. JVM log observations normalize exception categories and omit JVM
stack frames; native diagnostic errors retain their own details.

The corpus includes the full pinned data, argument evaluation order, malformed
rows and roots, Unicode integers, logging failures, nested loads, cached names
and EOF behavior on the covered inputs. A native filesystem regression verifies
reader publication, missing/directory open failures, retained parse failures and
that later loads do not reset existing manager items.

```powershell
cargo test -p rs09-server --test ground_spawn_loader --test ground_spawn --test ground_manager --test chunk_sync
cargo xtask ground-spawn-loader-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
```

Owning-world/session/plugin services, actual region population, Take/inventory
and the broader I/O gaps above remain incomplete. No player capacity, whole
gameplay replacement or runtime JVM dependency is implied by this component.
