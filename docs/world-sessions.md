# Native world-session transport and player updates

The server transport can now hand authenticated native player metadata to an
owning world through `world::session::World`. A ready `Joined` supplies rights,
network player index, and bounded inbound/outbound frame channels. The transport
sends the original success header only after that admission succeeds. Default
startup has no complete world provider and still returns world unavailable.

Account/password work remains in the bounded authentication workers. World
admission is asynchronous and cancellation-aware: disconnects release the pending
connection, and bytes arriving during admission remain bounded. Online checks
run at the original decoder boundary, before account/profile work. Rejected
profile admission preserves the original low-byte server-nonce detail. An error
incorrectly marked Success cannot manufacture a successful world header.

Game ingress and egress are polled independently, so two full bounded directions
cannot block one another. TCP chunks and binary WebSocket messages use the same
incremental game framer. Bytes coalesced after the login request/reply are retained.
Disconnect, shutdown and failed writes drop channels and release byte budgets.
The pinned profile disables opcode ISAAC.

`world::player_render` ports PlayerRenderer ordering over shared mutable native
player/render state, and `world::rebuild` ports UpdateSceneGraph's normal 162
writer. Their 207 JVM scenarios compare exact wire bytes, membership/retained list
views, appearance stamps, callback mutation/failure order and last-scene state.
RenderInfo cases also compare first-cycle/appearance preparation, retained last
locations and post-incremented mask counters on bounds failures. The nine-player
addition limit is intentional: the original increments count and
breaks when it reaches ten. Local membership is limited to 255. Self slot 2047 is
separate from the network player index in the login reply.

The normal client field decoder is shared in `protocol::game::rebuild`. It is also
checked against 32 original server coordinate/plane frames, signed coordinate
packing, transformed fields and key order. It decodes fields; full scene/cache
loading and the original blacklist/dynamic-region behavior belong to the client.

`world::appearance::Data` writes the original appearance field after equipment/body
preparation. Its 127 JVM cases use the unchanged original flag subclass and
verbatim body helpers. They verify empty-equipment male/female defaults and
prepared slot/color/NPC/PvP/summoning/name/size variants. Equipped-body preparation,
item-team/color/BAS resolution, complete masks/Appearance APIs and live content
still need integration. Appearance bytes use equipment indices, not item IDs.

The visual and headless clients share `protocol::game::players::State` for
incremental 225 membership, movement queues, appearance cache and mask fields.
The decoder does not run authoritative player simulation or infer completed
animation/chat/audio effects from field parsing.

```powershell
cargo test -p rs09-server --test player_render --test rebuild --test appearance --test world_session
cargo xtask player-render-oracle --check --python <python> --java-home <jdk11>
cargo xtask appearance-oracle --check --python <python> --java-home <jdk11>
```

## Controlled stationary-player endpoint

`crates/server/examples/player_session.rs` is a client/transport integration
fixture. It uses native development authentication, pinned cache serving/XTEAs,
normal scene rebuilds, the verified player renderer and naked appearance encoder.
It accepts connections and publishes stationary player data at (3207,3216,0).
It does not perform full save/profile loading, Player.init, NPCs, movement,
interaction/content dispatch, economy or bank/shop actions. It is separate from
the pending production world provider and does not clear those acceptance gates.

Prepare the pinned XTEA JSON once from the locked source and pass its path:

```powershell
git -C reference/server show 9921b98d49308edd0489dd50ee98011d53914a57:Server/data/configs/xteas.json > runs/player-session/xteas.json
cargo run -p rs09-server --example player_session -- --keys runs/player-session/xteas.json --tcp 127.0.0.1:43596 --websocket 127.0.0.1:43597
cargo run -p rs09-headless -- --world --address 127.0.0.1:43596 --username agent --password testing --rounds 3
cargo run -p rs09-headless -- --world --websocket --address 127.0.0.1:43597 --username agentws --password testing --rounds 3
```

The native example needs the cache and JSON file, not Java/Python at runtime.
Use an existing writable `runs/player-session` directory for the preparation
command. Development authentication accepts the supplied test name/password under
the original development rules; this is not a production account setup.

Real Windows TCP and browser WebSocket connections have consumed this endpoint's
success/rebuild/player/appearance stream and rendered the supplied body. Both
native headless transports also output the same structured player observations.
The browser has sent a real door action over the connection; the fixture logs
and ignores it, so the door stays closed. This demonstrates frame delivery while
keeping authoritative interaction dispatch explicitly pending.

Headless `--world` outputs structured self/nearby coordinates, appearances, network
index, region IDs and packet counts. Rounds count received player-update packets,
not world ticks or capacity. The observations preserve queued movement data;
client animation/queue consumption remains separate. The cache probe stays the
default mode. Actual AI action handling and full game-playing agents are pending.

The complete game replacement still requires profile/startup hooks, full entity
and NPC state, original movement/path/interaction processing, authoritative UI/
economy/content, the full tick sequence, persistence, serial/parallel replay and
measured performance. This fixture's periodic loop is not that world driver.

The original walking queue is now ported separately with 284 JVM comparisons;
see [walking.md](walking.md). It is not yet connected to this fixture. Movement
pulses and original action/world sequencing must be in place before enabling
live movement.

SmartPathfinder and its reach rules now have a separate native port with 5,287
JVM comparisons and native scene/door collision integration; see
[pathfinding.md](pathfinding.md). MovementPulse/destination geometry now has its
own 626-case port ([movement.md](movement.md)); joining its entity/queue/path/
content services and authoritative dispatch remains pending.

An explicit rendering/audio probe can run alongside the default fixture:

```powershell
cargo run -p rs09-server --example animation_probe -- --keys runs/player-session/xteas.json --tcp 127.0.0.1:43598 --websocket 127.0.0.1:43599 --animation-probe 863 --sound-probe 127
```

Every ten fixture ticks (approximately six seconds), the optional probe sends
animation 863/delay 0 in original player-mask order and SYNTH_SOUND 127/loops 1/
delay 0. These are controlled rendering/audio inputs, not gameplay or emote-action
handlers. Omitting the options preserves stationary behavior. Animation fields
and global/area AudioPacket.write bytes add 45 and 60 JVM comparisons to the
existing 127 appearance cases (232 before the spot extension). A fixture composition regression also
decodes its emitted 225 frame and checks animation before appearance.

The separate [PLAYER_INFO spot probe](player-spot-probe.md) adds 880 original
SpotAnim payload comparisons (1,112 appearance/animation/audio/spot cases total)
and an admission-relative repeating timeline for graphics111/369, replacement,
height, delay and explicit clears. It uses the same native TCP/WebSocket session
and player renderer, with bounded data-only observations in the headless client.
