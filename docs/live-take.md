# Native Take through the actual game stream

The controlled movement/NPC endpoint now joins the original ground packet body,
real native manager lookup and option table, retained movement/arrival, typed
Pickup Program, canonical player inventory/listeners/attributes/Ironman and Scene.
`pickup_runtime` uses the real captured container receiver and virtual two-argument
add dispatch. It does not synthesize inventory success or bypass approach.

Three integration regressions run in both movement_session and npc_session:
actual opcode66 through arrival and two viewer removals; existing pickup keys
with false/null values; and full inventory/future drop deadlines. The success
case checks actual container state, recalculated weight, 667:32 text and private
sound. Its portable cache uses an explicit bounded walkable terrain patch.
Production uses the supplied pinned cache instead of that test patch.

A separate owned development server53212 runs at TCP43640/WS43641 with one
authored bronze-axe1351 spawn at(3207,3216,0). The prior door demo10220 at
43632/43633 remains running. Finite real TCP and WebSocket actor/spectator pairs
both observe the item before sending Take; both observe removal afterward.
The actor receives 1351x1, server weight becomes1.35, and the original integer
label is `1 kg`. The spectator receives neither that inventory nor private
pickup sound. Exact request:42870c0547100c. Raw receipts and accepted handler
snapshots are under runs/live-take-investigation/data.

```powershell
cargo run --release -p rs09-headless --example pickup_agent -- `
  --address 127.0.0.1:43640 --username pickupactor --spectator pickupspec
```

Use `--websocket --address 127.0.0.1:43641` for WebSocket. Both actors use the
shared original ground reducer and preloaded item metadata, without a renderer.
The example reports actual observations, applies a finite deadline and sends
through the public Session frame API. It is neither a capacity benchmark nor
idle-session longevity proof. The typed public action API/lifecycle driver is
still incomplete.

Original Settings.updateWeight updates the server double and sends only text to
interface667/child32 on this path. WeightUpdate registration is commented out in
the pinned PacketRepository. A default client RunWeight0 with no159 frame is
not measured weight; no new159 send is added to make the diagnostics look newer.

The current canonical player now owns primary/secondary/drop-log banks; cape and
pouch checks use real inventory/active-bank/equipment item ownership. Portable
66/arrival regressions cover those stores. Full startup/content, saved profiles,
Drop, bank UI/PIN/content, guild dialogue, concrete item plugins and drop-trade
audit logging are not established. Missing controlled-host services fail explicitly if reached;
the original kernel can already have inserted an item before a later service
failure, and that prefix is not rolled back. The controlled content-event
registry is empty. The existing PICK2_2582 dependency gap remains documented in
docs/pickup.md; id2582 is the existing explicit numeric adapter, not proof that
the missing external ConstLib symbol was recovered. Full game replacement and
measured performance/capacity remain open.
