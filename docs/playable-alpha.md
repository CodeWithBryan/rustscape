# Playable multiplayer alpha

The `npc_session` alpha joins the Windows client, the original packet decoders,
authoritative NPC melee, ordinary tree chopping, and character snapshots. The
local and configured private-network launchers are under `runs/melee-alpha`.

**Start world.cmd** and **Join world.cmd** open one persistent world. New
characters join in Lumbridge. To reach the mountain trolls, travel west and
north, open the Taverley gate at about (2935, 3450), then go through Burthorpe
and follow the mountain path to Death Plateau. The account, session and
character save are the same throughout. Players elsewhere on the map remain in
the same world and become visible to one another when in range.

The launcher uses `--world-npcs`. The server indexes all 13,709 placements
from the pinned spawn file. It creates NPCs once when players approach their
regions, on every authored plane and with the original placement, walking
flag, facing, and definition. Opened regions stay active for the lifetime of
the world. The Windows client uses the Lumbridge pack at startup, then reads
normal map regions on demand from the local pinned cache. It uses rebuild keys
from the server and the matching local keys for prefetch, prepares map geometry
and NPC assets on a worker, and uploads the next window across render frames.
Normal map rebuilds preserve the player's absolute presentation position and
walking animation. The previous window stays visible until its replacement is
ready. Instanced maps, quest-gated movement and broader interaction parity
remain outside this build.

Combat accepts the real `NpcAction` option slot only if the NPC supplied that
player with a local view and its definition has `Attack` in that slot. The
initial mode supports unarmed accurate melee against ordinary unmorphed NPCs.
The default animation, speed and base formulas come from `Properties.java`,
`MeleeSwingHandler.kt`, and `CombatSwingHandler.kt`. The NPC side uses the
loaded original hitpoints, bonuses, defence, animations and respawn delay.
Attack and Hitpoints XP use the native Skills implementation. The damage and
death updates reach all local viewers while XP stays with the player who hit.
NPC loot, specialized AI, fully animated player death, weapon styles, gear
bonuses, ranged/magic combat and protections are outside this slice.

Woodcutting accepts a normal scenery option on an ordinary cached tree. It
checks for a bronze axe, walks a bounded route to a reachable side, awards a
log and 25 XP using the canonical inventory and Skills, and publishes a shared
temporary stump. Constants come from `WoodcuttingListener.kt`, `WoodcuttingNode.java`,
`SkillingTool.java`, and `RandomFunction.java`. The membership, perk, alternate
tool, diary, and bonus resource paths have not been ported here. The launcher
supplies bronze axes near the initial Lumbridge tile. The tree is from the
actual Lumbridge scene.

The versioned alpha snapshot preserves location, skills/XP and health,
inventory and equipment slots, both banks and bank tabs. It loads from a
dedicated directory, keeps filesystem work off the world loop, and publishes
a complete file via a synced temporary and rename. Logout, periodic saves,
and graceful stop are separate entry points. Character snapshots must not be
shared by two simultaneously running worlds. This is not a full original
profile or an import/export format for the pinned server.

For private hosting use a configured account list; the upstream development
auth mode is restricted to loopback. The native client uses TCP, and both
TCP and WebSocket sessions join one world. Do not route this alpha directly
to the public internet; it is designed for a private test network.

## Rebuilding the Windows client and startup pack

Use the pinned cache under `assets/cache`. The RT4 terrain packs from other
regions are unnecessary for the Windows live launcher. The browser build
continues to use prepared map packs. Combat sequences come from the existing
`assets/visual/animations.pack`.

```powershell
python tools/prepare_visual.py --cache assets/cache --center 3207 3216 --region-bounds 49 49 51 51 --region-npcs --npc-types 86 278 519 520 521 708 2281 2342 3021 3384 3385 3777 4899 4942 7969 --output runs/melee-alpha/lumbridge-populated-wide-base.pack
python tools/prepare_playable_pack.py runs/melee-alpha/lumbridge-populated-wide.pack --base runs/melee-alpha/lumbridge-populated-wide-base.pack
cargo build --release -p rs09-client
Copy-Item target/release/rs09-client.exe runs/melee-alpha/rs09-world-client.exe
```
