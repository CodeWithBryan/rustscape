# Player spot-animation input

The native `animation_probe` example can send actual PLAYER_INFO225 spot-animation
masks to the Windows, browser and headless clients. This provides controlled
rendering input while spell/combat handlers and complete gameplay integration
remain pending. The timeline does not execute a spell or infer animation state
on the server.

```powershell
cargo run -p rs09-server --example animation_probe -- --keys runs/player-session/xteas.json --tcp 127.0.0.1:43616 --websocket 127.0.0.1:43617 --spot-probe fixtures/player-spot-probe/timeline.json
```

The reference endpoint on those ports was launched separately with a copied
executable. Do not launch another process on occupied ports. Existing probe
endpoints retain their original arguments. Use distinct usernames of at most12
characters and password `testing` for this explicit stationary fixture.

Each admitted actor has its own600ms tick age. The committed timeline repeats
every16ticks; an actor joining later starts its own sequence:

| Actor age | Graphic | Height | Delay | Purpose |
| --- | --- | --- | --- | --- |
| 2 | 111 | 0 | 0 | Immediate graphic |
| 4 | 369 | 128 | 60 | Delayed request |
| 5 | 111 | 64 | 0 | Replace the pending graphic |
| 7 | -1 | 0 | 0 | Explicit clear |
| 9 | 369 | 256 | 30 | Delayed graphic at another height |
| 11 | -1 | 0 | 0 | Explicit clear |

The first delayed369 is replaced before becoming visible. Server tick ages and
the client's animation/delay clock are different units. Omitted masks between
events retain client state; they do not mean clear. A missing repeat period is
one-shot. Invalid zero periods, unordered/duplicate/zero event ticks and events
outside the repeat period are rejected before startup.

## Wire behavior

`world::appearance::SpotAnimation` writes the original
`PlayerFlags530.SpotAnim` payload: mask0x100, ordinal9, little-endian truncated
ID followed by `mp4((height << 16) + delay)`. Both arithmetic operations wrap as
Java/Kotlin signed32-bit operations; the addition can carry from delay into
height. ID-1 becomes the protocol's65535 clear sentinel. Encoding returns a
six-byte stack array.

The fixture's renderer emits the extended mask header and writes Animate
(ordinal2), Appearance(ordinal3), then SpotAnim(ordinal9). It retains its prior
forced appearance for non-sync self updates; a spot-only nearby update can have
a different mask. It sends each transient spot to all eligible viewers before
resetting the server-side pending field. That reset does not send a clear.

The original JVM corpus compiles the unchanged SpotAnim writer and full
Graphics class against explicit external adapters. The880 new cases cover
negative values, signed extremes, truncation, overflow and delay carry; the
prior232 observations remain byte-identical. The static Graphics.send chunk
path is not executed, and its external adapters throw if called. This proves
payload encoding, with separate native regressions for renderer composition
and scheduling.

## Transport and headless evidence

`runs/player-spot-verification/live-receipt/report.json` records the captured
four-observer run: two TCP and two WebSocket clients, each observing28 player
updates,12 self spot events and24 nearby spot events. Every self sequence
matches the timeline twice, including four clears and four delayed requests.
The audit compares each raw payload fragment to the exact original encoding.
Own network indices1–4 are distinct from the protocol's self slot2047.

Headless world results add `playerSpots` only when a spot field is observed.
Each retained update contains its player-update number, opcode, exact payload
hex and decoded slot/ID/packed-word fields. History is bounded to256
spot-bearing updates and reports the number omitted. These are data
observations; they do not invent client pose, terrain height or audio state.

The live daemon's logs can continue growing as visual QA connects. The receipt
contains frozen raw observer JSON, exact captured log prefixes, timeline hash
and copied executable hashes. Runtime rendering/audio evidence belongs to the
client's separately scoped reports in `docs/visual-client.md`. These transport
observations do not establish combat, content handlers, full Player startup,
world scalability or complete graphical parity.
