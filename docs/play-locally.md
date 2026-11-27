# Playing the native live world locally

One command starts the native server (`rs09-server --world`) and the visual
client, and logs you in to the world at the title screen. Windows with
PowerShell is the primary target. You need what [Build and check](../README.md#build-and-check)
lists, plus Python 3.10+ (the world folder is written by the world-trace
oracle's own Python code).

```powershell
pwsh tools/play.ps1
```

Log in at the title screen with any name and password. The world folder's
configuration has `persist_accounts = false` (accounts are kept in memory,
so any password works) and `noauth_default_admin = true`.

## What the script does

Each step is skipped when its result is already there, so later runs go
straight to the build check and start-up:

1. `cargo xtask bootstrap`: the pinned `reference/server` and
   `reference/client` partial clones (only when they are missing).
2. `cargo xtask cache-fetch`: the pinned cache into `assets/cache` (only
   when `main_file_cache.dat2` is missing).
3. `cargo xtask play-world`: the world folder `runs/play/world-data`, the
   pinned `Server/data` files and `worldprops/default.conf` with
   `enable_bots = false`. It calls `world_template` of
   `oracle/world-trace/generate.py` through `tools/play_world.py`, writing to
   its own folder; git fetches the `Server/data` blobs of the pinned commit
   into the partial clone. `--force` rewrites an existing folder.
4. `cargo build --release -p rs09-server -p rs09-client` (`-SkipBuild` skips
   it).
5. The server, in the background, with its log in `runs/play/server.log`:
   `rs09-server --world --cache assets/cache --saves runs/play/saves --data runs/play/world-data --tcp 127.0.0.1:43594`.
6. Once the port answers, the client in the foreground:
   `rs09-client --connect 127.0.0.1:43594 --cache assets/cache --world-keys runs/play/world-data/data/configs/xteas.json --mute`.
   Closing the client stops the server.

Options: `-Port N` (when 43594 is taken), `-Sound` (start with audio on;
by default the client gets `--mute`, which sets music, effects and ambient
sound to volume 0 and leaves the saved volumes as they are), `-KeepServer`
(leave the server running after the client closes), `-ServerOnly` (no
client; Ctrl+C stops it), `-Data`, `-Saves`, `-Cache`, `-Python PATH`.

The region keys matter: without them the client streams the title screen's
regions with no key and prints `JS5 length exceeds limit` and `negative JS5
length` for their locations. When `--world-keys` is not given the client now
reads `runs/player-session/xteas.json`, else
`runs/play/world-data/data/configs/xteas.json`.

## What is playable

[docs/live-world.md](live-world.md) lists what the native world runs, what it
answers by name without a port, and what it refuses. In short: walking,
other players, NPCs, combat, prayer, combat magic and three teleports, ground
items, equipment, banking at bankers, shops, trading, eating and drinking run;
quests and most content do not.

**New characters start on Tutorial Island** (`new_player_location =
"3094,3107,0"`). The tutorial is not ported, so there is no way off the
island in game, and until the tutorial is complete the original caps each
skill's experience at 175.

To start somewhere else, log out (or stop the server), then edit
`runs/play/saves/players/<name>.json`:

- `core_data.location`: `"3222,3218,0"` is Lumbridge.
- To count the tutorial as done (the original's lobby screen then shows at
  login, and the experience cap no longer applies), add to the top-level
  `attributes` list (create the list if it is missing):
  `{"key": "tutorial:complete", "type": "bool", "value": true}`.

Edit the file only while that character is logged out: the server writes it
at logout.
