# Shared client ground items

`rs09_protocol::game::ground` is the shared RT4 packet reducer and pile selector
for the visual and headless clients. It does not simulate ground-item ownership,
decay, drops or taking items; the authoritative server owns those rules.

The reducer implements ground packets 33, 135, 240 and 14, zone origins 26/112
and enclosed ground updates in 230. Packet 135 excludes the actual network player
slot. It must not compare a profile UID or the internal self slot 2047. Additions
append distinct objects without merging. Count and removal updates change only
the first matching entry; those two readers mask the item ID with `0x7fff`,
whereas additions retain all 16 bits.

Pile selection follows the original `Protocol.spawnGroundObject`: stackable
value is wrapping `cost * (amount + 1)`, ties retain the earlier candidate, and
the best entry is moved to the list head before selecting two further distinct
item types. Values at or below the original `-99999999` initial threshold do not
produce a pile. The selected objects remain live references, including when
their amounts later change. A failed height query leaves the head mutation in
place and preserves the previous rendered pile.

`Context` supplies item cost/stackability, terrain height and publication. No
internal lock crosses a callback. Captured lists, the original single iterator
cursor, reentrant refresh, plane changes and partial failures are preserved.
Externally retained nodes lease their current owner; transferring a node updates
the shared lease. Lists hold bare nodes and weak back references to exposed
handles, so dropping all owners and handles releases the graph without cycles.
Structural edits run in the serialized client update phase.

The state stores nonnull cells in a sparse ordered map rather than allocating
43,264 empty references per client. Empty but nonnull lists still exist as
distinct cells. Rebasing moves only nonnull cells while preserving the original
array-shift results and list aliases, including negative/large deltas.

Consumers apply each top-level frame once. `apply_frame` already handles a zone
envelope's ground children; feeding them again duplicates additions. Scenery,
projectile and sound reducers still need their own handling of those children.
The consumer synchronizes `plane` and `self_id` from shared player/login state.
`rebase_stacks` shifts the logical lists only. Scene teardown, `clear_rendered`,
then terrain reconstruction and `refresh` are separate phases. `take_dirty`
reports scene publication/removal, not every logical edit. Metadata must come
from the actual cache. RT4's missing-definition default (cost 1, not stackable)
must be reported explicitly rather than silently treating it as a real record.

The existing pack reader, complete item decoder, generated item fields and
template transfers now live in `rs09-client-data`. The visual client re-exports
the same public paths and types. `tools/port_client_items.py --check` checks the
new generated-file location; the original source hashes and UI fixtures remain
unchanged. This crate has no graphics dependencies.

The headless observer opts in with:

```powershell
cargo run -p rs09-headless -- --world --address 127.0.0.1:43612 --rounds 2 --username agent --ground-pack runs/windows-client-frustum/assets/visual/ui-players.pack
```

Use an appropriate running fixture/server address and prepared pack. The decoder
preloads cost/stackability once, including note/lend transfers, drops the pack and
model bytes, then shares immutable metadata among every client in that process.
`groundItems` reports remembered local scene tiles, duplicate entries in original
order, selected object types, metadata hash/source and explicit fallback IDs.
Headless height publication is deliberately omitted; the internal placeholder
never appears as measured terrain data and cannot change item ordering/selection.
This observer makes no drop/Take/inventory predictions.

An owned ephemeral TCP regression sends actual login/rebuild/ground/player frames
through the real headless transport. It checks coalesced post-login bytes, network
owner filtering and original self queue initialization during rebuild. A separate
local smoke loaded all 14,654 definitions from the pinned UI-players pack with no
fallbacks and observed two updates on the preserved server. That old endpoint
sent no ground tiles; the smoke is not evidence of live world item management.

`cargo xtask ground-items-oracle --check` compiles unchanged pinned Java branches,
the full selection method and original intrusive-list classes. It compares 475
scenarios and 6,614 command snapshots, with explicit metadata/height/publication
adapters. The corpus covers raw and truncated readers, duplicate IDs, wrapping
values, strict ties, owner filtering, retained references, nested refresh,
discarded/transferred lists, callback failures, zone clearing and array rebasing.
It records the client commit and normalized source hashes. JDK 17+ is required;
CI uses 25. Rust tests additionally check object reclamation, dirty publication,
writer agreement with canonical original reader inputs and every frame split.

Malformed-frame safety is deliberately bounded: enclosed framing is validated
before mutation, so a truncated later child rejects the whole envelope. The
original streaming reader can mutate earlier children first. Full-zone tile
bounds fail after any already-cleared prefix instead of clamping the rectangle.
Invalid/truncated origin readers also decode before publishing their new origin.

Visual integration/rendering, SceneGraph support heights, minimap and picking are
being validated separately. Remaining gameplay work includes original Take and
inventory/content behavior, complete scene rebuild phases, terrain observations
for headless agents and live server ground-item management/fanout. Native manager
and chunk mutation bodies are documented in `ground-manager.md`.
This reducer and its recording oracle do not establish complete gameplay or full
original class replacement. See the parity ledger for the broader migration.
