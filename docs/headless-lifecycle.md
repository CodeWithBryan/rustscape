# Managed data-only game connections

`Session::into_managed()` starts one Tokio task for an already logged-in native
TCP/WebSocket session. Its read and write futures progress independently. A slow
write has a15second deadline; it does not stop receiving or processing the idle
clock. No renderer, model inference or gameplay callback runs inside this task.

The driver uses the original20ms client cycles: a complete received packet
resets the read counter, the751st unread cycle closes, and the51st idle write
cycle queues opcode93. A successful transport write resets the write counter.
Partial headers/payloads do not reset reads. Scheduling stalls delay subsequent
cycles rather than emitting bursts of historical pings. Socket scheduling is
native async I/O; it is not a claim of identical JVM thread scheduling.

```rust,ignore
let mut game = Session::connect(address, false, "agent", password).await?
    .into_managed();
let commands = game.sender();
// The transport continues while your model is thinking elsewhere.
commands.walk([3207, 3216], false).await?;
let frame = game.next_frame().await?; // apply your shared authoritative reducers
game.close().await?;
```

The owner retains login metadata but no credentials. `sender()` is cloneable;
it exposes `send_frame`, `walk`, `scenery`, status and sampled statistics. A send
acknowledges completed transport publication, not gameplay acceptance. Cancelling
a sender future after admission does not retract its command. Dropping the
ManagedSession owner closes the entire connection, including cloned senders.
Explicit `close()` also waits for bounded transport cleanup. There is no automatic
reconnect, fabricated activity/AFK input, or swallowed server logout.

Queued observations are bounded to256 frames and1MiB including a conservative
three-byte header charge per frame. Overflow closes with ObservationOverflow;
previously accepted frames remain drainable in order before the terminal error.
Commands are bounded to64 queued messages and1MiB of encoded bytes, with async
admission backpressure. Caller-owned buffers/futures and returned frames are
outside these queue budgets. Decoder and native transport bounds still apply.
Stats count only accepted frames and successfully written commands/keepalives;
their independently sampled counters are not an atomic world snapshot.

The136-case JVM corpus compiles unchanged selected Protocol blocks, including
wrapping counters, timeout-before-write order, pending output, null sockets and
failed write prefixes. Recording socket/connection-lost adapters are explicit;
this is not a full Protocol class port. Reproduce with JDK25:

```powershell
cargo xtask headless-lifecycle-oracle --check
cargo test -p rs09-headless
cargo run --release -p rs09-headless --example lifecycle_agent -- `
  --address 127.0.0.1:43640 --username idleagent --seconds 70
```

Use `--websocket --address 127.0.0.1:43641` for WebSocket. The finite example
pauses the consumer for2seconds, checks an actual completed keepalive, then
observes authoritative player updates without gameplay commands. Raw Session
remains available for applications that deliberately supply their own lifecycle.
This work does not complete profiles/content, public action coverage, idle AFK
game rules, session reconnection, full shared world state or measured capacity.
