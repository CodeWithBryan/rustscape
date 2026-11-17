# Native player metadata

`crates/server/src/player` implements the data setup between authentication and
`Login.proceedWith`. It retains the authenticated account handle, parses its
communication fields, and carries the packet's client modes, dimensions,
username, opcode and ISAAC seeds. The native endpoint executes this work inside
the existing bounded account worker, before any world-entry response.

The port covers communication account parsing and the older SQLTable column
parse/save methods; contact/rank data; client modes; UID normalization and data
translation; player account-field access, ban/mute checks and account saving.
The column trait models those legacy method calls and is independent of the
actual SQL account provider. It does not perform database I/O.

Compatibility includes these observable details:

- Account parsing appends blocked entries, including one empty entry when the
  saved string is empty. It replaces contacts only after a complete successful
  contact parse. The column parser merges contacts and keeps earlier mutations
  when a later entry fails.
- JDK String HashMap iteration, resize and tree-bin order determine contact
  serialization. Native lookup and stable node indices retain that order.
- Invalid or missing clan requirement ordinals can leave null ranks. The account
  and legacy column paths treat invalid kick requirements differently. Empty
  clan requirements in the column path retain Java's string-identity distinction.
- Save mutations and column writes occur in source order. Null-rank/name errors
  preserve earlier updates. PlayerDetails.save ignores IllegalStateException
  from storage, while other storage failures propagate; the native SQL provider
  distinguishes its five empty-data guards from ordinary query failures.
- Ban/mute comparisons use strict greater-than. Permanent mute uses signed long
  subtraction with wrapping and truncation toward zero when converting to days.
- A present session address overrides UID IP, even when the address is null.
  Failed UID translation preserves the original method's partial assignments.

Parsing uses borrowed delimiter iterators. Serialization builds each output
directly, without allocating a string for each contact or rebuilding the map's
iteration order. These implementation choices have no standalone performance
claim; gameplay workload benchmarks still need a complete world.

## Evidence

`fixtures/player/` contains **285 original JVM cases** and source hashes.
`oracle/player/generate.py` compiles complete, unmodified CommunicationInfo,
Contact, ClanRank, PlayerDetails, UIDInfo, ClientInfo and Rights classes, plus
UserAccountInfo and AccountStorageProvider. It also compiles the verbatim
LoginReadEvent suffix that performs metadata setup. Clock, column, storage and
session adapters are explicit. Unused world operations throw. The
Login.proceedWith adapter records its arguments and returns offline.

Run the native differential tests and reproduce the reference output with
Python 3.10+ and JDK 11:

```powershell
cargo test -p rs09-server --test player --test admission
cargo xtask player-oracle --check --java-home PATH_TO_JDK_11
```

The admission tests also send valid and malformed account metadata through real
TCP and WebSocket sessions, checking error codes and worker accounting. JVMs are
test tools only; runtime metadata loading is Rust.

## Remaining work

Social actions and their packets/management messages, clan repositories,
loot-share pulses, Rights player attributes, UID debug messages and arbitrary
Java Map mutations remain pending. Actual player ownership, duplicate admission,
daily-account limits, profile parsing/saving, region streaming, tick ordering and
world entry remain pending. Accepted, well-formed account metadata still receives
LoginServerOffline (8); malformed metadata receives UnexpectedError (0), matching
the original LoginReadEvent exception branch. This component does not establish
a playable login or replace an entire source file in the parity inventory.
