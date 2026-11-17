# Native scenery interactions and live door probe

The controlled movement/NPC endpoint can now bind scenery clicks to the native
original option table, InteractPlugin scheduling/arrival, DoorManagingPlugin and
Scene replacement/collision implementations. The complete game and production
world startup remain in progress. Source inventory rows remain `not_replaced`.

`world/scenery_options.rs` installs each actual scenery's table before running
default initialization. Action names use its cached definition; option handlers
use its current ID. Local typed `option:name` configuration precedes the exact
global key. Initialized tables and old arrays retain their original option and
handler objects. Wrong local types fail before fallback. Registration binds the
actual native Door Program; labels and fixture observation numbers never select
executable content. Item and scenery registries share the same arena mechanism
while retaining distinct IDs and registry identities.

`world/scenery_runtime.rs` joins the existing scenery packet algorithm to those
captured tables and the canonical legacy runtime. It preserves lookup/morph,
scripts/listener/plugin order and the second interaction receiver lookup. The
selected option survives that second lookup. Walking retains the exact target
and routing handler; arrival reads the selected shared option as the original
does. `world/door_runtime.rs` binds the content program to actual Scene transforms,
collision replacement, flags and restoration tasks. No independent scene tick is
introduced: actor and scenery pulses share the original fixed-prefix runner.

Stored scenery/table node edges are unowned internal references. Exposed canonical
nodes, scenery and table views retain the actual morph family. Collection counts
the table's physical internal lease separately, keeps scene-owned nodes stable,
and can release detached families after external views drop. Raw option arrays
keep handlers alive without retaining a dead scenery/actor graph. Handler arena
lifetime remains conservative; this is not complete plugin graph collection.

The probe enables this path with `--door-config FILE`, loading every original
configuration row. It also initializes the existing original 13-by-13 viewport
synchronization to deliver scenery flags to all viewers before reset. It does
not use a distance-only broadcast. Example:

```powershell
cargo run --release -p rs09-server --example npc_session -- `
  --keys runs/player-session/xteas.json `
  --tcp 127.0.0.1:43632 --websocket 127.0.0.1:43633 `
  --gameframe --door-config fixtures/doors/door_configs.json `
  --item-config runs/item-config-oracle/item_configs.json `
  --scenery-trace runs/my-door-session/handlers.jsonl
```

Create the trace's parent directory first. Trace files contain accepted handler
results/errors, queued scenery updates, emitted sound frames and allocated
walking/projectile arrays around the selected object before/after the handler.
Array observation performs no region loads or gameplay callbacks. Null arrays
are explicitly missing observations. Existing processes/endpoints are independent.

The ordinary Lumbridge door is ID12348 at `(3207,3217,0)`, shape0/rotation1. The
original rules replace it with ID10327 at `(3207,3218,0)`, rotation2, with a
500-pulse restore and original sound62. Close emits sound60 and restores collision.
RegionChunk.add wraps restored plain scenery in Constructed; value equivalence
does not imply the original plain object reference is reused.

Regression coverage includes the full request/walk/arrival path, collision
changes/restoration, exact sound frames, two viewer deliveries including one
20 tiles away, and one-runner 500-pulse restoration. Portable terrain is an
explicit bounded walkable fixture; live/cache validation uses pinned map data.
The new scenery-options oracle has 77 original cases using unchanged full
InteractPlugin/Option/OptionHandler classes, original scenery definition methods,
and the original door registration method. It checks cached/current definitions,
local/global handler identity, retained arrays, ROOT casing, inherited walk/delay
defaults and partial failures. Full DoorManagingPlugin execution remains covered
by the existing scenery-action oracle, and Scene/door behavior by their existing
joined original corpora and full-cache audit.

The probe still has explicit empty content/listener/zone/combat/event services.
Morphs requiring player varps/varbits, full ActionLocks, autowalk logout listeners,
diaries, profile Properties/trapdoor teleports and complete content startup remain
unbound and report required-service failures when reached. There is no object-ID
whitelist, fake successful trapdoor, complete game, live Take/Equip/bank/shop or
capacity claim. The general Rust library requires the real host services.

The headless probe still needs a public scenery-action request API and structured
scenery/collision observations. Finite TCP/WebSocket login/player/NPC observers
verify that the door-enabled server continues to serve data without graphics;
they do not by themselves prove headless door manipulation.
