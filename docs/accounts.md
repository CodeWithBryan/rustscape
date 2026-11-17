# Native account library

`rs09-server::auth` implements the pinned account records, in-memory and SQL
storage, development/production authentication rules and BCrypt password format in Rust.
Configured network endpoints use it after packet validation, through separate
bounded workers. Successful account checks stop at the unavailable world-entry
boundary; character ownership, saves and map/entity streaming remain pending.

## Preserved behavior

`UserAccountInfo` preserves all eighteen ordered fields, initial-reference values,
change indices, Java UTF-16 string/long/Boolean hash codes, default detection and
equality. Equality/hash exclude join date, while change tracking includes it.
Mutable Timestamp handles retain the original reference alias: mutating the
current timestamp also mutates the initial snapshot when they share that object.
Replacing it with another value follows Timestamp equality, including nanos.

In-memory storage keeps exact string keys and shared account identities. A missing
lookup creates a default record under the requested key, sets its UID from that
key's Java string hash, and leaves its username empty. Store recomputes UID from
the record's username; update retains UID. Renaming and updating can leave older
keys referencing the same record. Removing uses the record's current username.
Hash collisions such as Aa/BB preserve separate map records with equal UIDs.
The original empty online-friend/IP query results are retained for this provider.

Map and record locks are separate and short. Password work does not hold either
lock. Hash/equality/default checks avoid per-call record/string copies. Snapshot
handle enumeration releases the map lock before inspecting records; callers need
a stable world phase for a consistent deep snapshot. Tests compare serial and
parallel results for 128 distinct accounts and shared missing-record creation.
This does not prove parallel simulation or world-admission ordering.

Development mode preserves password-ignoring login, optional administrator rights,
case normalization on creation, and its final lookup using the original input
case. Production mode preserves lookup/password/ban/online ordering and response
codes, including invalid stored hashes producing CouldNotLogin. Creation and
password-update mutation side effects remain visible when storage fails.

The pinned default.conf selects development auth and in-memory storage, with
default administrator rights enabled. These also match the initial constants.
The TOML parser's fallback values differ (production/persistence enabled and
default administrator disabled); the separate native configuration library
preserves them and supplies configured endpoints.
Native policy selection is available through `Mode` or `factory::Registry`.
The game's Base37 login names are ASCII. Full default-locale and JDK-version
Unicode lowercasing outside that name path is not yet claimed.

## Password compatibility

The implementation uses RustCrypto Blowfish primitives with the original BCrypt
framing and parsing rules. It retains `$2a$` verification, UTF-8 encoding, embedded
NUL handling and the effective 72-byte key boundary. The original code's signed
`1 << 31` overflow makes cost 31 skip the repeated expansion loop; the native
compatibility path deliberately matches it. Default hashing remains cost 12.
Malformed input returns typed errors rather than reproducing array exceptions.

`Passwords::default` obtains a fresh sixteen-byte salt from the OS. Explicit salt
sources support deterministic experiments/tests. Hashing is synchronous CPU work;
configured endpoints use bounded worker admission outside the socket threads.
No JVM is part of the account runtime.

## Reproduce checks

```powershell
cargo test -p rs09-server --test auth
cargo xtask auth-oracle --check --java-home PATH_TO_JDK_11
cargo xtask auth-interop --java-home PATH_TO_JDK_11
```

Oracle commands require Python 3.10+ and JDK 11. They accept the login oracle's
`--python PATH`, `RS09_PYTHON` and `RS09_ORACLE_JAVA_HOME` options. Kotlin/compiler
and JSON-simple test artifacts are downloaded with pinned size/hash checks.
Runtime account/password APIs use neither Python nor Java.

The 368 cases execute complete original Auth.configure, UserAccountInfo,
in-memory provider, development/production auth and EncryptionManager/BCrypt classes. Explicit
adapters control wall-clock time, hash entropy/cost, player password access and
storage exceptions. SQL configuration fields are observed through an adapter;
SQL execution deliberately throws in this account harness. Tests compare
every observed step: returned values/errors, metadata, hashes, initial/current
arrays, changed fields, provider call order, stored entries and handle aliases.
They also execute a verbatim LoginReadEvent prefix through its authentication
branch, using explicit decoder/world-boundary adapters. Communication and world
entry are outside that prefix.

The `*_with_clock` APIs sample an explicit clock at original operation boundaries.
Creation assigns join date after hashing; production login samples ban expiry
after password verification. Storage samples timestamp defaults when a record is
actually created/read, after SQL query completion. Fixed-time APIs delegate to a
constant clock for deterministic scenarios. Forty elapsed-work traces and 82
admission traces validate expiry, errors, ordering and mutation effects.

`auth-interop` additionally exercises real OS salts at default cost 12: four
native hashes are checked by the original Java implementation and four original
Java hashes are checked by Rust. The credentials are synthetic test data. New
reports go to `runs/auth-interop`; the recorded snapshot is
`parity/auth-interop.json`. Timings are not a game-server capacity measurement.

## Provider factory

`factory::Registry::configure` implements Auth.configure's selection and assignment
order from explicit `Settings`. Each call creates fresh memory or SQL storage.
Production mode configures SQL; development mode leaves it unconfigured even
when persistence is requested, preserving the original error behavior. Missing
production SQL settings fail in argument order after replacing the selected
provider but before replacing the authenticator. An earlier authenticator retains
its earlier storage. Its development administrator policy follows the current
setting, including after failed reconfiguration.

Thirty-three original factory traces compare provider/authenticator identities,
raw connection fields, missing nullable settings, subsequent login/create results
and record mutations. Database port is taken from the address string; the separate
DATABASE_PORT value is unused by this original path. The real SQL comparison
also constructs a factory-selected provider and checks its connection and query
over both transport modes. Credentials are synthetic in fixtures; configuration
types deliberately have no Debug implementation that would print passwords.
General Connector/J URL extensions are not established by these checks. Native
server configuration coverage is described in [configuration.md](configuration.md).

## Remaining work

The SQL provider's database behavior is covered separately in
[sql-storage.md](sql-storage.md). Atomic character ownership/admission,
the daily-account policy, profile save/load,
social communication actions and world entry remain pending. Account-field and
communication metadata setup is covered in [player-metadata.md](player-metadata.md). The native network
server does not accept playable logins yet. These components must be completed
against their original implementations before full login parity is claimed.
