# Native SQL account storage

`rs09-server::auth::sql::SqlStorage` implements the pinned SQLStorageProvider's
queries, field conversions and mutation behavior using the native `mysql` crate.
It can be constructed programmatically with `SqlConfig` and an online-player
lookup, or selected by `auth::factory::Registry` using reference configuration
fields. Configured network endpoints execute it on bounded account workers.
Playable world entry remains pending. No Java process is used by the runtime provider.

The original `Server/db_exports/global.sql` is preserved under `fixtures/sql`
with its Git blob and SHA-256. It defines the members/worlds tables and their
original defaults, nullability, collation and timestamp precision. Tests reset
only two reserved schemas inside a fresh, owned database process.

## Behavior and execution

The provider preserves the original eighteen-column record mapping, lowercase
lookup, raw requested username in returned records, missing-record defaults,
generated UID assignment, null text/numeric handling, JDBC Boolean conversion,
negative time clamping, timestamp precision and initial-value updates. Nullable
names from the IP query remain `Option<String>`. Inserts bind the account IP
twice, as the original does. A generated UID outside signed Int range fails
after insertion, preserving the already committed database row.

The update-column map omits UID, original IP and join date while the original
binder still processes changed scalar fields. The resulting invalid statements
remain errors. The friends query targets `players`, which is absent from the
canonical dump; that error is preserved. Tests explicitly create a players
table to exercise the successful branch too. Contact splitting, Unicode rank
parsing, duplicate handling and JDK String HashMap iteration are compared with
the original. The shared map helper models insertion/iteration, including
resize/tree bins; it is not a general Java map implementation.

Connections use a bounded pool (four maximum by default), ten-second connect,
read, write and lease timeouts, and UTC sessions. Existence checks before an
insert reuse the same lease; a one-connection pool is tested to catch nested
acquisition deadlocks. Calls are synchronous and must run on bounded storage
workers when integrated with the asynchronous server. This does not establish
atomic character ownership or parallel world execution.

Encryption defaults to Connector/J's PREFERRED behavior: use TLS when advertised,
and permit plaintext only when the server lacks TLS capability. Required and
disabled policies are also available. The compatibility policy does not verify
server certificates/hostnames, matching the pinned JDBC URL. Verified-certificate
configuration and changing server TLS capability during a provider's lifetime
are not covered. Windows uses OS SChannel through `native-tls`; game TCP/wss TLS
is a separate, unfinished feature. Driver 26 is pinned for Rust 1.86 compatibility.

## Verification

```powershell
cargo test -p xtask --test sql
cargo xtask sql-oracle --check --java-home PATH_TO_JDK_11
```

The integration command also accepts `--python PATH` and the login oracle's
environment options. It requires Windows, Python 3.10+, JDK 11 and OpenSSL on
PATH. It verifies downloaded archive/JAR hashes, compiles the complete original
SQLStorageProvider/UserAccountInfo classes with explicit clock/online-lookup
adapters, and runs 139 cases against MariaDB 10.4.14. Native and original record,
error, lookup-order and database-row observations must match before fixtures can
be written. The full matrix runs over plaintext and TLS; actual session cipher
checks prove transport selection, and required TLS must reject the plaintext
instance. Portable Rust tests exercise the recorded data without starting a DB.

The pinned portable database runs under `runs/sql-db` on an ephemeral loopback
port. Before schema resets and shutdown, the harness checks the running owned
process and its reported data directory. Successful runs remove only that run's
data directory after shutdown; failed runs retain it for inspection. Logs and
downloaded tools are ignored. No system database service is installed or used.
Reports include tool versions, source hashes, fixture hashes, transport modes
and shutdown results; see `parity/sql-oracle.json`. This is storage compatibility
evidence, not a game-player capacity measurement.

The native error type distinguishes the five original empty-data guards
(`IllegalStateException`) from query/storage errors. Player metadata saving
ignores only those guards, matching PlayerDetails.save; see
[player-metadata.md](player-metadata.md). Duplicate-account insertion is a query
error, and is not swallowed by that save rule.
