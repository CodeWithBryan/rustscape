# Shared headless sessions and scenery observations

`rs09-headless` is now a library as well as a finite observer CLI. `Session`
connects through the existing native login transport, preserves coalesced world
bytes, frames the TCP/WebSocket stream, validates outbound frames and exposes
the original scenery/walk writers. Applications can keep a session alive and
alternate commands with `next_frame()` instead of launching a process per action.
The application chooses deadlines and owns its world reducer. Credentials are
not retained in Session. The current pinned transport profile disables ISAAC.

The visual and headless clients use the same `location_packet` implementation
in protocol/game/actions.rs. The client public path is a re-export. All48 original
RT4 location writer cases remain unchanged, including priority action normalization,
transformed shorts and integer narrowing. `SceneryAction.option` is the original
zero-based0..4 index. Sending returns the actual wire bytes; gameplay acceptance
is established by subsequent server observations, not a local success flag.

The original logical rebuild and ChangeLocRequest queue, Slot/Location and layer
mapping now live in client-data/scenery.rs; the visual public world path re-exports
them. Camera Ray/picking remains in the visual crate. These shared types introduce
no graphics or audio device dependency into the headless client.

`scenery::Observer` records ordered location additions/removals, full chunk-reset
signals and private SYNTH_SOUND fields. It uses the shared ServerUpdate decoder,
actual last rebuild base and player plane. Envelopes are expanded once; a malformed
later subframe preserves the already recorded prefix. Events retain decoded
subframe opcodes/payloads and the player-update ordinal at receipt. Previous
absolute observations remain unchanged across rebuilds. History is bounded256
with an omitted counter; streaming consumers can drain events after each frame.

These are authoritative wire changes. They do not invent a complete static map,
collision/terrain data, missing cache definitions, asset readiness or audio output.
Those data services still need their complete shared cache/world bindings.
The actual presentation queue remains available separately for consumers with
a real static scene and asset readiness provider.

Example of a finite real Open/Close sequence against the owned development probe:

```powershell
cargo run --release -p rs09-headless -- `
  --world --address 127.0.0.1:43632 --username aidoor --rounds 14 `
  --scenery 12348,3207,3217,0 --scenery-after 2 `
  --scenery 10327,3207,3218,0 --scenery-after 8
```

The door must initially be closed and the server must have the native door
configuration enabled. Add `--websocket` and use port43633 for the same game stream
over WebSocket. Existing `--walk`, `--npcs`, `--ground-pack` and multi-client
observation remain available. This probe is not a player-capacity benchmark.

For a long-lived agent, use `rs09_headless::session::Session` directly. Its
`scenery`, `walk`, `send_frame`, `next_frame`, `metadata` and `close` methods do
not create a renderer. The crate re-exports shared `protocol` and `data` modules.
The full game, public action coverage beyond currently bound methods, static
world/collision observations, profiles/content and complete gameplay remain open.
For autonomous liveness, convert it with `Session::into_managed()`. The managed
connection sends original idle keepalives independently of observation consumption
and provides bounded queues and explicit close reasons. Raw Session still requires
the caller to drive liveness. The earlier finite scenery proofs alone do not
establish idle-session longevity. See docs/headless-lifecycle.md for the new driver
and its separate bounded proof scope. For NPC, item, use-with and dialogue
actions with a reduced world view, see docs/headless-agent.md.

The runnable `scenery_agent` example demonstrates a data-driven loop: after its
first player update it sends Open, waits for the actual replacement object,
uses that received object's tile for Close, and waits for restoration. It uses
neither a renderer nor fixed sleeps or update counts to infer success. Real TCP
and WebSocket runs on the owned native probe each produce the exact original
Open/Close bytes, observe sounds62/60 and match two accepted server handler rows.
The finite scheduled CLI path also completes both round trips over both
transports. Final rebuilt-binary receipts: runs/headless-scenery-final/live/report.json and
live-cli/report.json. Earlier binaries and their initial observations are preserved
separately. The final held-source verification passes 556 workspace tests and
67 server-example tests, with no failures or ignored tests, plus the original
packet/login/network and 6,827-case primitive checks. These bounded observations
establish no player capacity.
