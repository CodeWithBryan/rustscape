# Native cache/lobby transport

`rs09-server` serves the pinned revision-530 cache and lobby protocol using Rust
on Windows. It is a migration component, not yet a playable game server.
Login handshakes and the original packet/RSA parser work. Invalid packets receive
the reference parser's response. With `--config`, valid packets run the selected
native account backend and receive its rejection code. Accepted accounts then
run native player/communication metadata setup. Malformed metadata receives
UnexpectedError (0); prepared records receive LoginServerOffline (8) while world
entry remains pending. See [player-metadata.md](player-metadata.md). Without `--config`,
valid packets receive 8 without checking accounts. There is no world simulation or
simulated character state. See [login.md](login.md) for codec scope and evidence.
Native account rules, provider selection and memory/SQL storage now have separate
library tests and are now called by configured endpoints; see
[accounts.md](accounts.md) and [sql-storage.md](sql-storage.md).

## Run

From the workspace root, first acquire and verify the pinned cache:

```powershell
cargo xtask bootstrap
cargo xtask cache-fetch
cargo run --release -p rs09-server -- --tcp 127.0.0.1:43594 --websocket 127.0.0.1:53595
```

The TCP listener defaults to `127.0.0.1:43594`. WebSocket is optional; the example
uses the reference's world-1 WebSocket port. Both listeners share connection
slots and budgets. `--cache DIR` selects an existing sector cache. Stop with Ctrl+C.
The runtime uses no Java or Python and does not download assets itself.

To load the original settings and enable account validation:

```powershell
cargo run --release -p rs09-server -- --config fixtures/config/default.conf --cache assets/cache
```

This shipped configuration uses development authentication and in-memory accounts.
It selects TCP port 43595 (43594 + world ID 1). Configured WebSocket defaults use
53594 + world ID, unless a positive explicit configured port is present. CLI
`--cache`, `--tcp` and `--websocket` override those fields. Paths are relative to
the process working directory. TLS WebSockets remain unavailable; requesting a
TLS listener returns an error. Configuration/content data is retained for future
world loading, but definitions, characters and gameplay are not loaded yet.

In a second terminal, run a native cache probe:

```powershell
cargo run --release -p rs09-headless -- --address 127.0.0.1:43594 --clients 500 --rounds 10
cargo run --release -p rs09-headless -- --address 127.0.0.1:53595 --clients 500 --rounds 10 --websocket
```

The default request fetches master index 255/255. `--archive ID --group ID`
selects another group. Each connection fetches the requested container repeatedly,
hashes its bytes, and closes; all clients must agree. Counts describe cache
connections. They do not imply logged-in players, AI actions or game capacity.
The visual workbench remains offline and does not yet consume this endpoint.

## Execution and limits

Four Tokio I/O workers handle sockets. Cache reads, cold packet preparation and
XOR encoding and login RSA parsing run in bounded blocking workers, with admission
before dispatch.
Configured account/password/SQL checks use a separate four-job admission pool.
The CLI allows twelve total blocking threads for the eight cache/RSA and four
account jobs. Account-clock reads occur at the original storage/password
boundaries. Socket tasks await admission; they do not execute those checks.
Warm unencrypted packets share immutable `Bytes` buffers and use no blocking job.
Per-connection request processing preserves FIFO order. Master data is prepared
from the original reference tables at startup.

| Resource | Default |
| --- | ---: |
| Shared TCP/WebSocket connection slots | 2,048 |
| Queued requests per connection | 40 |
| Queued replies per connection | 8 |
| Global queued reply payload budget | 32 MiB |
| Concurrent blocking jobs | 8 |
| Concurrent account jobs | 4 |
| Session ingress buffer | 64 KiB |
| Read / write timeout | 60 / 15 seconds |
| Retained packet entries / payload | 4,096 / 64 MiB |

Limits are configurable through the Rust `Config` API; the CLI currently uses
defaults. The packet retention cache is shared. The sector store loads the
approximately 92 MB dat2 file once, plus index files. Queue byte accounting covers
payloads admitted to output, including a write in progress. It excludes socket
buffers, TLS/platform bookkeeping, packet preparation before admission, map
allocations and other process memory. It is not a process memory ceiling.

An overloaded connection awaits bounded queues and byte permits. Read/write
timeouts and dropped connections release their queued resources. Shutdown
cancels sessions, joins them and releases their connection/byte permits. A
blocking job already executing may finish after its requesting session is gone.
This also applies to account work. Its permit stays with the blocking job through
session cancellation; restarting listeners cannot exceed that bound. The outer
Tokio runtime waits for running blocking jobs when it is dropped. Profile saving
and complete world shutdown remain pending.

## Compatibility evidence

```powershell
cargo test -p rs09-server
cargo test -p xtask --test network
cargo xtask network-oracle --check
cargo xtask network-interop
cargo build -p rs09-headless
cargo xtask network-smoke
```

The last two probes require the complete pinned cache. `network-smoke` invokes
the headless executable from its own build-profile directory; build both in the
same profile. Reports go under `runs/network-interop` and `runs/network-smoke`.
Recorded snapshots are under `parity/`.

- 1,311 JVM cases exercise complete original cache packet formatters, handshake
  handlers, cache controls, world-list encoding and original client queue limits.
  Explicit adapters replace external I/O, queue scheduling and game metadata;
  they do not emulate authentication or a game world.
- A live original RT4 `Js5NetQueue` checks group/reference bytes over TCP with
  urgent/prefetch requests, pipelining and four XOR keys. The same run checks
  master bytes and both lobby stamp cases: 315 verified replies total.
- Native integration tests use both TCP and binary WebSocket, fragmentation,
  malformed controls, missing groups, admission limits, slow readers and a
  request flood. A synchronized 500-connection test verifies shared preparation
  and clean shutdown. A separate CLI smoke run exercises 500 connections and
  5,000 verified requests per transport.

Smoke timings include connection setup, handshakes, response hashing and close.
They use one run per backend, a small repeated file, and no CLI start barrier;
they are functional evidence, not a throughput or player-capacity benchmark.

The native encoder deliberately preserves the original master formatter's
separate block counter, its ignored XOR/priority settings, and compressed-group
padding when the cache version bytes are missing. Bounds checks reject malformed
lengths. The incremental reader safely retains fragmented handshakes, where the
original handler could underflow.

## Pending integration

Character/profile persistence, registration, game packets, entity updates, ticks,
gameplay/content, saves, client resource-provider retries/CRC checking, browser
network bindings, and TLS/wss remain pending. Lobby population/stamps are static
cache-only metadata. The pinned client sets `LOGIN_USE_STRINGS=true`, but its
actual login code still writes a Base37 username long; future login integration
must follow the executed code rather than inferring behavior from that flag.
