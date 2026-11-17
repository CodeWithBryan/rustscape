# Revision-530 gameplay packet contract

`rs09_protocol::game` is the shared wire boundary for the Rust visual client,
headless client and server. It contains both pinned 256-entry packet-size tables,
`Frame { opcode, payload }`, a bounded incremental `Decoder`, numeric client
actions, and typed server update fields. It does not implement game rules.

The pinned profile disables ISAAC. The original server reads plain inbound
opcodes, so enabling a client cipher alone does not produce a compatible server.
The framer accepts an explicit cipher for controlled callers/tests, advances it
once per opcode across fragmented headers, and validates writes before consuming
a cipher word. Socket/WebSocket read boundaries have no packet meaning.

## Client actions

`ClientAction::decode(opcode, &mut Reader)` returns `Ok(None)` without consuming
bytes for an unported opcode. It covers 69 original numeric opcodes: all scenery,
item, NPC and supported player actions; ground-item/use-with/component actions;
ten interface buttons, item options, examine, close/continue, slot switches; three
walking forms; focus/camera/display/mouse tracking; numeric prompt, preferences,
chat settings, packet counts and keepalive. Text/chat/social payloads remain raw
frames for later decoders. Decoding does not authorize or execute an action.

Ordinary IoBuffer shorts are signed; little-endian/transformed shorts are
unsigned. Interface-hash high halves use arithmetic shifts. Walking reads all
reported offset pairs but adds only the last pair to the initial destination.
Minimap data retains its 14-byte trailer. Ignored fields and leftover bytes are
preserved as original decoder behavior. Boundary tests also compare partial read
positions against the JVM for truncated numeric payloads.

## Server updates

`game::updates::ServerUpdate::decode(&Frame)` supplies these typed messages:

- Chunk origins (112/26) and enclosed zone frames (230).
- Scenery add/remove/animation (179/195/20), with coordinates relative to the
  active chunk. The client adds its origin and current plane, then applies the
  original scene bounds, layer mapping, placement and collision rules.
- Interface open (155), Windows pane (145), close (149), hide (21) and text (171).
  Original RT4 constant names for 155/145 can be misleading; use the methods in
  `Interface.java` and `WindowsPane.java` and the pinned decoder behavior.
- Inventory full/partial/clear (105/22/144). Decoded item IDs already subtract one
  from the wire ID. Container IDs already include the original adjustment for
  component pointers below -70000. Raw negative/large amounts remain observable.
- Client integer variables (65/69), client-script invocation (115) and inclusive
  interface event/access-mask ranges (165). Counter fields wrap to 16 bits;
  event range endpoints 65535 decode to -1.
- Varps (60/226) and varbits (37/84), without interface counters. Small varps
  retain signed bytes; large values retain signed 32-bit fields.

Outgoing helpers create original server payloads. The caller owns the interface
counter, its pre-increment sampling, scene origins and authoritative containers.
For partial inventory updates it resolves slot references and NULL_ITEM identity
before serializing. A null full entry writes amount zero and wire ID zero. Full
updates transform amount bytes; partial updates do not. Amounts outside 0..254
use a 255 marker and signed four-byte amount. Smart slots outside 0..32767 and
oversized payloads are rejected by the native codec.

WindowsPane writes `p1sub(kind)` while RT4 reads `g1add()`. The native decoder
preserves the resulting reset value, including 2 becoming 254. Text helpers accept
already encoded bytes; the test oracle selects UTF-8 for the original platform
charset-dependent writer. This does not establish general JVM text/locale parity.

`varc(id, value, counter)` preserves the original small-packet selection for every
value <=255, including negative values. The client reads that byte unsigned.
`run_client_script(id, types, objects, counter)` consumes objects in original
server context order: the signature is traversed right-to-left, consuming objects
forward. `ScriptArg::Text` supplies encoded bytes for `s`; other signature bytes
use `ScriptArg::Int`. Extra objects are ignored. The decoded `arguments` vector
is in client signature order, with script ID separate. Native writers reject
missing/mistyped arguments, NUL strings, non-ASCII signatures and oversized
payloads. These bounded failures do not claim arbitrary malformed JVM behavior.

`varp(id, value)` uses the original -128..127 small-packet range. Artificial-player
suppression remains the server caller's policy; the oracle binds that flag false.
`varbit(id, value)` deliberately preserves Varbit.java's lossy writer: small
values decode as value&127; large values write only `(128|value)&255` and an
untransformed little-endian ID although the client reads a transformed ID. For
example, requested (4893,256) decodes as (5021,128). The typed decoder follows
the client reads and can represent full values from other raw messages. Tests
cover raw reads separately from writer output. Runtime parity must preserve the
source call paths rather than silently correcting these transformations.

## Verification and integration

```powershell
cargo test -p xtask --test game_packets
cargo xtask game-packets-oracle --check --python <python> --java-home <jdk11>
```

The 3,989-case corpus compiles complete unchanged Decoders530, Packet, IoBuffer,
GameWriteEvent, fourteen outgoing classes and their original contexts. It checks
numeric actions/read positions, outgoing bytes/counters and client field reads.
The latter compile original RT4 Buffer/Packet, verbatim selected Protocol field
prefixes, client-script argument loops, JagString.charAt and inventory loops.
World/entity/item/interface-counter/ISAAC-zero and
scene-coordinate adapters are explicit. Layer lookup is a placeholder for these
field reads; scene mutation, collision and UI side effects are outside this oracle.
Fixtures pin source and adapter hashes. Both reference checkouts remain unchanged.

Stream tests cover every sized opcode, fixed/one-byte/two-byte headers, empty and
maximum payload lengths, and chunk sizes from one byte through a whole stream.
They also check bounded ingress, unsupported-size failures and optional cipher
advancement. Enclosed zone messages contain original fixed-size zone packets;
unknown or variable nested opcodes are rejected instead of risking a stalled loop.

The visual agent owns scene/UI reducers and consumes this shared contract. The
server still returns world-unavailable after successful account validation.
Authoritative scenery/door rules, dynamic collision, walking, animations, entity
updates, initial map/player synchronization, action dispatch and live session
integration remain required before a playable door/walk-through acceptance test.
No constructed packet is presented as a real Lumbridge door gameplay trace.
Performance and gameplay capacity have not been established by these codec tests.
