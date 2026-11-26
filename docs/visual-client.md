# Rust visual client workbench

**How to read this document.** The current state of the client, how each part
is verified and the full list of what is still missing are in
[client-protocol-and-cache.md](client-protocol-and-cache.md). This document
holds the run instructions, the launch options and the controls, which are
kept current, and below them a record of the earlier stages: what each built
and how it was checked at the time. In those stage sections a sentence saying
something is "pending", "separate" or "open" was true when it was written;
many such items have since been done. Where the two documents disagree, the
status document is right.

Bridge terrain now uses the original integer SceneGraph height interpolation
for actors, attached spot effects, character shadows, overheads, camera target
and the ground-pick height adapter. Region lookup retains the current104-tile
scene origin and cross-region corner samples. Original force-ground/bridge
render levels expose prepared bridge floors and static/animated scenery while
retaining their physical plane for geometry. The Catherby configured NPC at
(2804,3427) consequently stands on the dock instead of the lower water surface.
The existing original1024 height cases are replayed through region lookup at
positive/negative origins, and512 new original render-level observations cover
flag priority and the plane1 shared flag cell. This was a renderer integration
slice. Roof removal and picking on upper floors have since been ported (see
the status document); dynamic locations on bridge columns still treat the
plane as physical.

Normal NPC body assembly now consumes its selected definition's BAS per-part
transforms before model combination, animation and final resize. The original
signed six-field values and complete duplicate-slot replacement are preserved.
`fixtures/ui/npc-bas.json` records256 decoder observations from the unchanged
original BasType; the existing original NPC raw/GL corpus also checks this normal
assembly path. All1431 pinned cached BAS records lack these transforms, so this
extends valid-input compatibility without claiming a visible cache correction.
Invalid body slots are rejected. (Accelerated movement, slot animations,
script-controlled palettes and reading assets from the cache at run time,
listed here as separate work when this was written, have since been ported.)

This is a runnable revision-530 visual client and offline compatibility workbench
for Windows and WebAssembly. A live client starts at the original title
screen, logs in, and plays in a streamed scene with the original interfaces.
It is not a complete playable replacement: see "Still missing" in the status
document.
The visual target is the pinned RT4 **HD client**, with the same assets, appearance,
animation tick, camera and brightness. Component agreement is required; it does not
by itself establish agreement of complete rendered frames.
The same Rust decoder, animator, synthesizer, and renderer run on both targets.
No Java, Kotlin, browser engine, or server is embedded in the desktop executable.

Reading assets directly from the JS5 cache (`--cache DIR`) removes the need
for every Python-built pack on the desktop, the starting scene included.

Browser launches show the available gameframe by default, matching Windows game
launchers. F9 hides/shows interfaces for the camera workbench. The hidden-UI debug
HUD's `test sound` field reports only manual sound audition; automatic NPC,
location, UI and sequence effects use their own audio backend status and traces.
The default offline browser launch applies its packaged gameframe preview before
the first draw. Live sessions receive their server interfaces, and explicit
world-packet replays keep their existing playback timing.

The browser host reports WebGL context interruption with a reload/reconnect
control, stops scheduled render frames and blocks further canvas input. Reloading starts a fresh renderer and
login flow; credentials are not retained. Automatic GPU resource restoration is
not implemented. A blank frame observed with many review tabs remains a separate
unconfirmed context/resource-loss investigation; the event handler does not
establish its cause.

Native capture and browser status snapshots include a bounded read-only view of
the server-fed inventory container93, preserving slot indices, raw item IDs and
amounts. Missing storage is distinct from an empty slot; the total slot count and
an explicit truncation flag accompany at most128 slots. The weight field is the
last decoded integer RunWeight value, not a recomputed or precise server weight.
These diagnostics do not predict item transfers or expose bank/shop contents.

The browser host's hidden `transport-proof` output retains only the latest
connection phase, queue bound and actual WebSocket close code/reason/wasClean,
plus a local close initiator where known. It stores no URL, credential or packet
contents. Queue limits, packet handling and timeout behavior are unchanged.
The earlier long-session disconnect has no retained close reason and remains
unexplained; the new diagnostic cannot reconstruct that past event.

## Run on Windows

With the pinned cache in `assets/cache` (`cargo xtask cache-fetch` downloads
and verifies it), from the `2009scape-rs` workspace root with Rust 1.86+:

```powershell
cargo run --release -p rs09-client
```

That starts the offline workbench at Lumbridge: no server, the scene, the
gameframe hidden until F9 or `--ui show`, and the workbench controls below.
Use `--release`: two of the work streams found that a debug build overflows
its stack in live mode.

Where the assets come from:

- The starting scene is `assets/visual/demo.pack` (or `--pack FILE`) when
  that file exists. When it does not, the client builds the scene from the
  cache (`--cache DIR`, else `assets/cache`) and streams the map from it as
  the player moves. A scene built from the cache has what the recorded pack
  lacks: floor shadows and the underwater scene. To force it, name a pack
  that does not exist: `--pack none`.
- Interfaces, models, animations and sounds come from the cache with
  `--cache DIR`. Without it the prepared UI pack is used
  (`assets/visual/ui-players.pack`, or `--ui-pack FILE`), and `assets/cache`
  if that pack is missing.
- Location files are encrypted per region; the keys are read from
  `--world-keys FILE` (default `runs/player-session/xteas.json`). A region
  without a key shows terrain only until a server's rebuild supplies it.

For example, everything from the cache, with the interface shown:

```powershell
cargo run --release -p rs09-client -- --cache assets/cache --pack none --ui show
```

A live session needs a server (see "Live world transport" below):

```powershell
cargo run --release -p rs09-client -- --connect 127.0.0.1:43596 --cache assets/cache --world-cache assets/cache
```

It opens a 1024 x 768 window on the original title screen; type the account
there. `--username NAME` with the password in `RS09_CLIENT_PASSWORD` logs in
without typing. `--world-cache` streams the map, so the scene follows the
player anywhere; without it (and with a prepared starting pack) only the
pack's regions exist and levels above the ground are an error. When the cache
directory is empty the client first downloads the cache from that server.

### Launch options

Every option below is read in `crates/client/src/main.rs` or a module it
calls. Options that take a value take it as the next argument.

Connection and data:

| Option | Effect |
| --- | --- |
| `--connect HOST:PORT` | Live client. The address is the world list and every world, and the update server unless `--update-server` names another. Starts at the title screen. |
| `--username NAME` | With `--connect`: log in as NAME once the title is up. The password is read from the environment variable `RS09_CLIENT_PASSWORD`, which must then be set. |
| `--world N` | The world id the client reports (default 1). |
| `--cache DIR` | Read interfaces, definitions, models, animations, sounds, music and the world map from this JS5 cache. Where a cache is needed and none is named, `assets/cache`. |
| `--world-keys FILE` | Region keys (JSON). Default `runs/player-session/xteas.json`. |
| `--world-cache DIR` | Stream map files from this cache. On by itself when the starting scene was built from the cache. |
| `--pack FILE` | Prepared starting-scene pack. Default `assets/visual/demo.pack`; if the file is missing the scene is built from the cache. |
| `--ui-pack FILE` | Prepared interface pack. Default `assets/visual/ui-players.pack`. |
| `--start X,Y` | Starting tile of a cache-built scene. |
| `--update-server HOST:PORT` | Where to fetch or repair the cache from before loading (default: the `--connect` address). Runs when the cache directory has no cache. |
| `--update-cache` | Run that update even when the directory has a cache. |
| `--update-original-archives` | Download only what the original prefetches; songs and Vorbis samples (archives 6 and 14) are then only verified. |
| `--preferences PATH` | The preference file (the original's binary format). Default `%APPDATA%\rs09-client\jagex_runescape_preferences.dat`. A `.json` path names the earlier JSON file; the binary file is then its neighbour. |
| `--no-music` | No music. |
| `--world-map-font PATH.ttf` | The typeface for world-map labels (default: bold Arial from the Windows fonts folder, then common faces elsewhere). |

Window and view:

| Option | Effect |
| --- | --- |
| `--window WIDTHxHEIGHT` | Window size. Default 1024x768 live, 1280x800 offline; 765x503 is the original's fixed size. |
| `--root 548` | Offline: open the fixed game frame instead of the resizable one (746), on the fixed 765 x 503 canvas. |
| `--window-mode 1\|2\|3` | Offline: the window mode (fixed, resizable, full screen) whatever the frame. |
| `--ui VALUE` | Offline: show the interface from the start. A number also opens that interface in the game frame's main slot (`--ui 320`); any other word (`--ui show`) only shows it. |
| `--yaw R`, `--pitch R` | Offline: the starting camera angles, in radians. |
| `--zoom N` | `Camera.ZOOM`, 1 to 2000 (default 600). |
| `--plane N` | The level the workbench player stands on. |
| `--roofs` | Start with roof removal off: every level drawn whole. |
| `--no-shadows`, `--no-fog`, `--no-flicker` | Start with ground shadows off; ignore the fog preference; ignore the flicker preference. |
| `--pinned-lights` | Place point lights where the pinned loader puts them instead of where their records say, which is the default (see "Local-light placement comparison"). `--correct-lights`, the old switch for what is now the default, is accepted and does nothing. |
| `--sd-lighting` | Start with high detail lighting off whatever the saved preference says: baked vertex colours, no light meshes, no point lights. |
| `--low-detail-textures` | Start with high detail textures off: every texture generated at 64 by 64. |
| `--brightness N` | Start at brightness 1 to 4 (the `B` key cycles it). |
| `--lighting-switch FRAME`, `--texture-switch FRAME` | From that frame on the lighting or texture detail is the other way round, as if a script had changed the preference (CS2 6012, 6006): for capturing a change at run time. |
| `--workbench-keys` | In a live session, enable F9, F10 and what F9 unlocks. |
| `--cpu-static-mesh` | Draw static scenery through the streaming path instead of persistent GPU buffers. |
| `--no-frustum-culling` | Submit every static chunk. |

Offline inputs and diagnostics (none has an original counterpart):

| Option | Effect |
| --- | --- |
| `--replay FILE` | Play a recorded server-to-client packet stream, one packet every 50 frames. |
| `--replay-now 1` | Apply the whole replay before the first frame. |
| `--frame-sleep MS` | Development aid: hold every frame back by MS milliseconds, so that a frame owes several 20 ms cycles. |
| `--npc-replay FILE` | Play an NPC update fixture. |
| `--appearance-preview FILE` | Dress the workbench player from a raw appearance block. |
| `--sequence ID` | Play this animation on the workbench player. |
| `--showcase-npcs "ID,X,Y;..."` | Replace the showcase NPCs. |
| `--loc-varbit ID=VALUE` | Give multiloc scenery a varbit value. |
| `--script-cursor ID` | Stand in for CS2 5426 (no cached script runs it). |
| `--minimap-all-labels` | Draw every map label on the minimap (the cache flags none). |
| `--fpson` | Start as if `::fpson` had been typed. |

Capture (the client draws a fixed number of frames on a controlled clock,
writes its files and exits; it reads no preferences and plays no music):

| Option | Effect |
| --- | --- |
| `--capture` | Capture. Writes a PNG and state files (`camera-state.json`, `ui-state.json`, `menu-state.json`, `minimap-state.json` and others) under `runs/visual/`. |
| `--output FILE` | The PNG's path. |
| `--capture-frame N` | The frame to capture at. |
| `--capture-realtime` | Keep the real 50 Hz clock while capturing. |
| `--capture-npcs`, `--capture-animation`, `--capture-ui-models` | Capture a series of frames of the NPC showcase, the player's animation, or interface models. |
| `--portrait` | A close view of the player playing the wave. |
| `--capture-route`, `--capture-route-run`, `--capture-route-target X,Y` | Walk (or run) to a tile during the capture. |
| `--capture-travel FILE` | Live capture: follow a recorded route. |
| `--pick-probe` | Also write `pick-state.json`: what the picking offers across the view. |

Environment variables: `RS09_CLIENT_PASSWORD` (above). With
`RS09_FRAME_DUMP=DIR` set, creating a file named `take` in that folder makes
the running client write the frame it just drew as `frame-N.png` there,
opaque as the window shows it (`frame_dump.rs`; `take-25` waits for a frame
of the loading state), a file named
`command` is run as a typed `::` command without its packet, and the lines
`OPCODE HEX` of a file named `inject` are taken by the next cycle as server
packets received (`game/inject.rs`). `RS09_POSTED_POINTER` set makes the
client take the pointer from window messages alone, for runs that post them.

### Prepared packs (Python)

No Python step is needed to run the client. The Python route below still
works and produces the prepared pack the regression captures were recorded
with:

```powershell
python tools/prepare_visual.py
cargo run --release -p rs09-client
```

The preparation command downloads the pinned server's Git LFS cache objects,
verifies their SHA-256 hashes against the pinned commit, reads the original cache,
and packages a focused visual corpus under the ignored `assets/visual/` directory.
It leaves the reference checkout untouched. The download is approximately 92 MB
and is reused after verification on subsequent runs. The prepared pack is about
9.1 MB. `--cache PATH` accepts an existing cache only if its files match the pin.

Python performs corpus extraction and location metadata export. Rust decodes the
original models, identity kits, NPC body definitions, frames/bases, sequences, the selected NPC and default player BAS types,
location collision fields, terrain, floor definitions,
materials, texture graphs, sprites and sound-effect bytes at runtime. An
already downloaded native corpus can be packaged with
`python tools/prepare_visual.py --cache assets/cache`. The same packs can be
written without Python: `cargo run --release -p rs09-client --example
build_packs -- ui` and `-- world OUT.pack --keys xteas.json` (see the status
document, "Assets from the cache").

If Python is available only through the Codex bundled runtime, invoke that Python
executable explicitly in place of `python`.

After preparing both packs, the live window below and the UI fixtures, `python
tools/package_visual_windows.py` copies the native release executable, raw inputs,
fixtures and license notices to `runs/windows-client/`. Double-click
`Launch client.cmd` there to start the inventory/game-frame preview. The packaged
runtime needs neither Java nor Python. Its README identifies the offline inputs
and remaining integration work. `Launch live client.cmd` prompts for the TCP world,
username and a hidden password, then runs the same executable in live mode.

## Run in a browser

```powershell
rustup target add wasm32-unknown-unknown
python tools/build_visual_web.py --serve
```

Open <http://127.0.0.1:8099> and click **Launch client**. The script builds
the Rust client, copies the JavaScript loader from the exact resolved Macroquad
package, and serves the staged artifact on loopback. The loader and WebAudio
integration are standard platform glue; models, animation and sound synthesis run
in the WebAssembly module. Audio starts in response to user interaction.

The build output is under `runs/visual-web/`; no external site is published.
WASM and the game-transport glue use content-versioned URLs so a refreshed launch
cannot silently reuse an older executable from the browser cache.

The start page can also take the game cache from a server instead of the
prepared packs, as the original applet does: tick **Download the game cache
from the update server** and give the server's WebSocket address (for example
`cargo run --release -p rs09-server -- --cache assets/cache --tcp
127.0.0.1:43594 --websocket 127.0.0.1:43597`, or the `player_session` example,
which serves the cache on the same listener as its world). The cache is kept
in the browser (IndexedDB) and verified on the next visit; music plays from it.
`?update=ws://127.0.0.1:43597/&song=scape%20main` fills the two fields in.
Details and limits are in
[client-protocol-and-cache.md](client-protocol-and-cache.md#the-cache-in-the-browser).

Toolchains newer than the pinned 1.86 no longer turn the client's undefined
`extern` functions into imports by themselves; `.cargo/config.toml` passes
`--import-undefined` to the linker for the browser target. Cargo reads that
file from the directory it is run in.

State of the browser build (what was run and seen is in the status document,
[The browser build as run](client-protocol-and-cache.md#the-browser-build-as-run)):

- `tools/build_visual_web.py` needs Python. It was updated for the cache
  download and music but has not been run since; the browser runs staged the
  same files with a script outside the repository.
- Run in Chromium twice. 2026-11-16: the cache download and its IndexedDB
  copy, the scene and gameframe built from it, music, a live login.
  2026-11-17, with the client as it is now, from the downloaded cache and
  from the prepared packs: loading bar, title screen (3D scene with the
  cache), typed login, the scene with the fixed and the resizable gameframe,
  menus, hover text, chat, the cursor presentation, window sizes and device
  pixel ratios 1.25 and 2, a lost connection and the return to the title,
  the title song. No other browser, and nobody listened.
- The page is the applet's container: the canvas element fills it, and a
  client in a fixed window mode draws its 765 x 503 canvas at the top
  centre with black around it, as the original does in an applet larger
  than its canvas. `::wm2` or the options make the canvas the page.
- Saved preferences (since 2026-11-17): the original preferences file's
  bytes are kept in the page's local storage under
  `rs09:jagex_runescape_preferences.dat` (`web/visual/cache_storage.js`),
  read at start and written whenever the desktop writes its file
  (`preferences_file::Store`). Brightness, volumes, window mode and the rest
  survive a reload; a first visit opens the title fixed, later visits in the
  remembered mode.
- Full screen (since 2026-11-17): the options' full-screen mode, CS2
  5300/5301 and `::wm3` put the canvas element in full screen with the
  browser's Fullscreen API (`web/visual/display_glue.js`). The one mode
  offered is the screen's own size; `::wm3` asks for 1024 x 768 and so
  enters full screen only on a screen of that size, as on Windows. The
  browser grants it only just after a click or key press and may end it by
  itself (Escape); either way the client returns to the remembered mode.
- The world map (since 2026-11-17) is installed when the cache came from the
  update server; its labels are drawn by the browser's own text on a 2D
  canvas in "Helvetica" (the page's sans-serif behind it), standing in for
  AWT's font. With the prepared packs there is no map.
- Scene builds (since 2026-11-17) are spread over frames, about 12 ms of a
  frame each (`?slice=MS` sets another amount, `?slice=0` builds in one
  frame as before), so the game, the keep-alives and the music feed go on
  while a scene is built.
- The drawing buffer has no alpha channel, so the page never shows through
  the frame.
- Limits: start-up (before the title) still takes one frame of about three
  seconds (the starting scene and its textures); music stops in a hidden
  tab; WebGL must offer three vertex texture units and 4096-pixel textures
  (the start page says so when it does not).
- To start a live browser client at the title screen, leave both credential
  fields empty. `?cursor=ID` on the page's address is the desktop's
  `--script-cursor`.

## Live world transport and initial player rendering

Prepare a separate pack containing all regions requested by the original normal
rebuild around the current development fixture's (3207,3216) spawn:

```powershell
python tools/prepare_visual.py --cache assets/cache --center 3207 3216 --label "Lumbridge live window" --output assets/visual/live/lumbridge.pack --login-window
```

The pack contains six pinned regions, 806 models and 17,775 location placements,
with no missing model dependencies. Its SHA-256 is
`e3fcc1ade7ccf438705a72b6c694019c233eb2f213167c432e1917f3bb6a58b4`.
This is prepared asset coverage of the initial window, not runtime JS5 streaming.

The server's `player_session` example is a controlled stationary-player transport
fixture. Start it separately using the server documentation. Its development
authenticator accepts username `visual` and password `testing`; simultaneous
clients must use distinct names. The current endpoints are native TCP
`127.0.0.1:43596` and binary WebSocket `ws://127.0.0.1:43597/`.

Native launch takes `--connect IP:port --username NAME` and reads the password
from `RS09_CLIENT_PASSWORD`; the packaged live launcher obtains it without echoing
or saving it. Use `--pack assets/visual/live/lumbridge.pack` with that fixture,
or stream the map from the cache instead (`--world-cache assets/cache`, or a
starting scene built from the cache), which needs no prepared pack.

`--connect HOST:port` alone starts at the original title screen (the
`loginscreen` interface, group 744): the world list is fetched behind the
original loading bar, then the player types a username and password, may pick a
world, and logs in; logout and a failed reconnect return there. With
`--username` the same login is started for the player as soon as the title
screen is up. The address is used as the original uses a `config.json`: the
world list and every world are that one address, and `--world N` (default 1) is
the world the client reports. In the browser, leave both credential fields
empty to start at the title screen.

Behind the title interface is the original's 3D scene (game state 28 builds
it, under "Loading - please wait." over the last frame): the map regions that
have an NPC file around a camera path the interface's scripts pick at random,
the path flown by the scripted camera, and the regions' NPCs wandering by the
client's own rule. It needs maps streamed from a cache, so a session started
with `--cache DIR` (or in the browser with a downloaded cache) streams its
scenes from it, as `--world-cache DIR` does. With prepared packs only, the
interface's scene component shows the title background instead. Not ported:
the software renderer's flames (the hardware renderer never draws them) and
the rest of `client.mainLoad`'s progress stages.

For browser login, stage that same pack:

```powershell
python tools/build_visual_web.py --pack assets/visual/live/lumbridge.pack --serve
```

Select **Connect to a development world** and enter the endpoint and credentials.
The HTML fields and transient launch strings are cleared after transfer to Rust.
The UI never stores them in local storage. Native TCP uses a bounded worker off
the render thread. Browser glue delivers binary WebSocket bytes to the same Rust
session/framing code; neither transport defines a separate gameplay protocol.
Partial writes, fragmented headers and coalesced login/rebuild/game data retain
their original byte order. The success path waits for all eleven metadata fields
and the first complete original rebuild payload before entering the game.

Both GPU clients have consumed the actual native fixture's login2, rebuild162 and
player 225 packets, assembled the original default body/BAS1426 from the cache and
rendered the self actor at (3207,3216,0). The packaged Windows client also observed
the browser player using its actual network index. Evidence is under
`runs/visual/live-tcp.png`, `live-tcp-state.json`, and `live-websocket.png`.
Live mode disables synthetic NPC spawns, manual walking/animation previews,
synthetic stock/inventory replay shortcuts and local route execution. Interaction
requests leave through the original client packet codecs, in order; state changes
depend on received server updates.
An actual browser **Open Door** click on 12348 at (3207,3217) sent interaction-walk
opcode77 and option-one opcode254 over WebSocket. The native fixture log records
both requests. `live-door-menu.png` and `live-door-request.png` preserve the menu
and request readback; the door stays closed because this fixture ignores actions.

The shared player decoder compares 2,304 incremental original reader packets and
1,536 complete appearance reads. Another 256 original LoginManager success streams
exercise the header and complete first-payload gate. Source hashes and explicit
chat/cache/actor/UI/socket bindings are recorded in `fixtures/players/`.
Native tests additionally cover both cipher settings, confirmation, timeout and
keepalive boundaries, bounded ingress and fragmented TCP byte ordering.

This established a stationary live player/scene transport slice. What it
listed as pending has since been ported: the login retry and reconnect rules
(reply 15 has only met scripted peers), the world list, map streaming from
the cache, levels above the ground (with streaming), NPC updates, public
chat, spot and slot graphics, worn and NPC-transformed bodies and the other
movement sets. World hop is the pinned client's own broken one. Every player
mask field is parsed; unavailable appearance assets produce diagnostics, and
a body that cannot be built keeps the one already shown.
The fixture does not run authoritative movement, door, bank, shop or agility
handlers, saved-profile initialization or complete original world startup.
The combined checkpoint passes all 228 workspace tests, workspace formatting,
all-target Clippy and 6,827 primitive oracle cases. Network/login/game-packet
oracles and TCP/WebSocket interop/smoke checks also pass; cache smoke client counts
do not establish gameplay capacity. The cache oracle's generated gzip bytes use
the normal JDK25 selection; JDK11 emits a different gzip OS header and dependent
CRCs. Login/game-packet reproduction uses the required JDK11 selection.
`--port PORT` changes the local server port. Stop the server with Ctrl+C.

To capture another map location without replacing the Lumbridge regression corpus:

```powershell
python tools/prepare_visual.py --cache assets/visual/cache --center 3212 3426 --label Varrock --output assets/visual/tour/varrock.pack
cargo run --release -p rs09-client -- --pack assets/visual/tour/varrock.pack --capture --output runs/visual/varrock.png --zoom 2000 --yaw 2.6 --pitch 0.8 --roofs
```

The view is the original's follow camera (`follow_camera.rs`): `--yaw` and
`--pitch` give the workbench's starting angles in radians and are converted to
the original's 2048-unit targets (pitch is held to 128..383), and `--zoom` sets
`Camera.ZOOM` (1..2000, default 600); the earlier free `--distance` is gone.
`--window WIDTHxHEIGHT` sizes the window (765x503 is the original's fixed
size) and `--root 548` previews the fixed game frame, where the scene is
confined to the 512 x 334 viewport component. Each capture also writes
`runs/visual/camera-state.json`. `cargo xtask camera-oracle --check` reproduces
the JVM observations behind `crates/client/tests/camera_reference.rs`.

Window modes follow `DisplayMode` (`crates/client/src/display.rs`, replayed
against the unchanged Java class by `cargo xtask display-oracle --check` and
`crates/client/tests/display.rs`). In the fixed modes the canvas is 765 x 503,
centred across the window at its top with black around it
(`leftMargin = (frameWidth - 765) / 2`); interfaces, the scene viewport and
mouse positions are the canvas's. In the resizable mode the canvas is the
window. A live client opens a 1024 x 768 window, shows the loading bar over
all of it and opens the title in the mode its preference file remembers (the
default file gives the fixed mode); the offline workbench stays resizable, or
fixed with `--root 548`, and `--window-mode 1|2|3` chooses otherwise
(`--root 746 --window-mode 1`, `--root 548 --window-mode 2`). This client has
only the hardware renderer, so the original's mode 0 is carried out, and
reported to the server, as mode 1. Full screen (mode 3, CS2 5300, or
`--window-mode 3` offline) is the borderless window over the primary screen at
the desktop's size on Windows; the screen's mode is never changed, so that
size is the only one listed and `::wm3` (1024 x 768) falls back to the
remembered mode unless the desktop is that size. Other systems and the browser
list no mode. A live window is multisampled as the saved anti-aliasing
preference says (none, 2x, 4x); miniquad fixes that when it makes the window,
so a change in the game's options applies from the next start. Offline and
capture windows keep four samples. With `RS09_FRAME_DUMP` set, a file named
`command` in that folder is run as a typed `::` command (without the packet).

Alternate packs load four surrounding cache regions, center the player/navigation
window on the requested tile, and select that region's environment and shadow
direction. Native `--pack` also opens these scenes interactively. Camera options
control the initial view; `--roofs` starts with roof removal off. The output
folder must exist. Varrock, Falador, Catherby, East Ardougne and Yanille have been
captured with this path. These captures exercise existing renderer behavior on
additional assets; they do not establish complete original-client frame parity.

The scene is drawn on the original's depth layers (`rs09_client::depth_layers`:
floors fifty layers behind what stands on them, flat ground decoration just in
front of its floor, wall decoration half a layer in front of its wall), and the
translucent faces of map locations are drawn after everything opaque, in the
original's drawing order. The original's occluders (`rs09_client::occlusion`,
JVM-verified by `cargo xtask occlusion-oracle`) decide what is left undrawn and
unpicked behind long walls and upper floors. `--no-occlusion` draws and picks as
if the scene had none, for comparison; with `--pick-probe` the capture's
`pick-state.json` also reports the occluders and what they left out.

## Controls

The game's own controls work live and offline, with the interface shown or
hidden:

| Input | Effect |
| --- | --- |
| Left click | The top menu entry: walk to the ground tile, or the first option of what is under the pointer. Live, the request goes to the server; offline, the workbench walks a local route |
| Right click | Open the menu of everything under the pointer |
| Ctrl | Run while moving (the original's `Keyboard.KEY_CTRL`; either Ctrl key) |
| Arrow keys | Turn and tilt the follow camera at the original's rates |
| F1 to F8 | Switch the gameframe's side-panel tabs (the interface's own key bindings) |
| Typed keys | Go to the interface's scripts (chat, dialogs, the title screen's form) |

The workbench's controls work offline while the interface is hidden. In a
live session they need `--workbench-keys`, and then F9 first:

| Input | Effect |
| --- | --- |
| F9 | Hide or show the interface |
| F10 | Show queued action bytes and script and asset diagnostics |
| F5 / F6 / F7 / F8 | Offline only: load the inventory, bank, shop or equipment preview fixture |
| WASD | Offline only: walk in world coordinates, respecting static collision |
| Middle mouse drag | Turn and tilt the camera (the original ignores the button) |
| Mouse wheel | Step `Camera.ZOOM` by 50 (a plugin feature in the reference, not the core client's) |
| 1 / 2 / 3 / 4 / 5 | Offline only: request idle / walking / running / wave / dance using the original action clock |
| T | Toggle tweening; enabled by default, matching the pinned client |
| R | Turn roof removal off (every level drawn whole) or back on |
| H | Toggle scenery and character ground shadows together |
| B | Cycle the original's brightness settings 1 to 4 (default 3) |
| P | Compare pinned and corrected local-light placement (pinned default) |
| N | Switch between the nearby eight-type NPC showcase and 33 pinned ground-plane spawns (not while NPCs come from packets) |
| Space | Offline only: pause the preview clock |
| F / G / M | Play the selected original effect (offline only) / select the next effect / mute effects and music |
| Escape | Exit |

With the interface shown, Escape and the letter and number keys belong to the
interface's scripts. Shift-click (a 2009scape addition) starts on, as in the
pinned client: with shift held and more than two entries in the menu, a left
click runs the entry below the top one. `::shiftclick` toggles it.

Offline, local routes feed one validated tile at a time into the
default-player movement controller, ticking every 20 ms; the ground-click
packet bytes are generated and kept locally. Live, the same clicks are sent
and the server's player updates move the actor. Picking follows the
original's rules (see the status document, "Menus, hover text, cursors and
picking").

## Live action animation and synthesized sound

Original player mask8 requests now drive the native actor clock: delay, priority,
same-animation restart modes, replay offsets/counts, next-frame selection and
sound-trigger ordering. Action poses combine with movement using the original
frame-group masks and retain fractional GL coordinates until both passes finish.
Live appearance meshes prepare their bind normals before animation. The same
implementation compiles for WebAssembly. Slot/spot clocks are ported, but slot
part transforms and spot graphics still need rendering/assets. Default-player
movement now applies the original action cancellation/blocking rules, catchup
speed and animation step accounting. Forced movement preserves the original
start/destination interpolation, action-frame gate and facing. Tile/player facing,
teleport queue preservation and local bounds resets are wired into live masks.
(Listed here as separate work at the time and since ported: slot part
transforms and spot graphics, NPC face targets, BAS acceleration and
accelerated yaw, weapon overrides.)

`fixtures/players/motion.json` adds 48 scenarios / 9,216 ticks against the complete
unchanged original `NpcList.processEntity` phases, `PathingEntity` queue methods
and `Player.animate`. This compares positions, queues, catchup, action/movement
clocks, forced intervals, facing, bounds resets and ordered sound triggers. The
explicit bindings use size-one players, synthetic sequences/default-player BAS,
target arrays and scene origins. Live actors tick and render in original local
membership order; retained actors outside that list do not continue appearing.
These client comparisons do not establish authoritative server walking or agility.
The rebuilt Windows package was exercised against the unchanged animation/audio
probe: `motion-regression-frames.jsonl` records 81 GPU frames over 14.51 seconds,
23 active wave frames plus idle, two server sound submissions and no diagnostics.
Reproduce the evidence checks with `python tools/check_actor_capture.py --prefix
motion-regression`. An additional packaged bank replay renders 327 visible nodes
with no diagnostics or item fallbacks (`motion-ui-bank.png` and its state JSON).
The rebuilt browser also preserves wave/idle transitions and empty diagnostics;
`motion-browser-{0..8}.png` span a full interval. The current active browser pose
is available at <https://files.catbox.moe/dqrzjg.webp>. These rendering checks use
the stationary probe; forced motion itself is covered by the JVM corpus rather
than claimed as an authoritative live-world result.
The combined movement checkpoint passes all 240 workspace tests through doctests,
workspace formatting, all-target Clippy with warnings denied and 6,827 primitive
JVM cases. The complete player oracle also reproduces its pinned fixtures. See
`runs/client-motion-verification.json` and `runs/movement-verification/` for the
scoped evidence and shared gate logs.

SYNTH_SOUND172 now reads its original ID/loop/delay fields and joins the same queue
as UI-script and player/NPC/scenery sequence sound triggers. The queue preserves
the fifty-entry limit, delayed cache readiness/start trimming, expiry, sequence
alternatives and original Manhattan-distance attenuation. The platform binding
uses the original Rust-synthesized finite-loop PCM on Windows and WebAudio.
Music/MIDI, continuous ambient emitters, infinite sample-region loops, resampling/
mixing equivalence, exact whole-client event order and physical device output
measurement remain unverified or pending. Unsupported loop counts produce a
diagnostic rather than a fabricated sound.

Differential coverage adds 8,192 original actor-clock ticks, 768 complete SeqType
metadata records, 4,096 full SoundPlayer queue ticks including SYNTH_SOUND reads,
and 512 actual-cache GL dual poses comparing every vertex/normal/color/alpha.
Run `python oracle/players/generate.py --check` and the client `actor_animation`,
`sound_queue`, `dual_pose` and `players` test targets to reproduce these checks.

The actual packaged Windows release was connected to the explicit server visual/
audio probe on TCP43598. That fixture emits wave863 and sound127/loops1 every ten
600ms fixture ticks. It provides rendering/audio requests, without gameplay
handlers. The captured run received one162, 24 player 225 packets and three172 sound
packets. All three sounds reached the native audio backend, and 81 GPU frames show
waves advancing and returning to idle with zero diagnostics. Evidence:
`runs/visual/live-animation.png`, `live-animation-state.json`,
`live-animation-network.json`, `live-animation-frames.jsonl` and
`python tools/check_actor_capture.py`. The raw frames are under the packaged
client's `runs/visual/actor-frames/`; their timestamps record actual elapsed time.
The packaged `Launch animation and audio.cmd` selects that probe by default and
prompts for a username/password without saving them. Ordinary live startup keeps
the stationary fixture's 43596 default. The verified Windows capture is also
available as a silent, actual-timestamp animated image at
<https://files.catbox.moe/d1w3jq.webp>; audio evidence is the native readback rather
than an audio track in that image.

For an explicit offline animation input, pass `--sequence 863` (wave) or `866`
(dance). `--capture-animation` records GPU frames every sixth render frame;
`--capture-frame N` selects the end. Offline captures use a controlled logical
clock. Live captures retain real 50Hz elapsed-time scheduling and never inject the
offline capture's test sound.

The final browser build also connected to the separate WebSocket43599 probe and
displayed the same wave/idle transitions in a close-up WebGL view. Its runtime
diagnostic panel showed no errors after the gesture that resumes WebAudio.
`runs/visual/live-browser-wave.png` preserves an active raised-arm pose;
`live-browser-animation.png` records idle, and `browser-wave-{0..8}.png` span a
complete probe interval. The shared checkpoint passes 237 workspace tests,
formatting and all-target Clippy; the server example's two additional fixture
lifetime regressions also pass. These checks do not establish complete gameplay,
music/mixer or whole-frame visual parity.
At that stage picking was a camera ray against rendered triangles. It has
since been replaced by a port of the original's hover test, scene draw order
and stacked-actor expansion; the other movement sets and slot and spot
rendering have been ported too.

## Live server walking and running

The separate controlled movement world on TCP 43600 / WS 43601 now accepts the
client's real ground-click 215 requests and returns authoritative player 225 steps.
The packaged Windows client and browser both reach real-cache reachable targets
with original walk 819/run 824 animation, smooth fine-coordinate movement and
return to idle 808. This is integrated native server/client movement, while full
Player/profile initialization, NPC AI, door/agility/bank/shop content, world
streaming/planes and general original packet dispatch remain incomplete services.
The ordinary stationary 43596/97 and animation/audio 43598/99 probes are unchanged.

Use **Launch movement probe.cmd** in the Windows package, or select **Connect to a
development world, Use controlled movement probe** in the browser. Both preserve
the existing credential handling. The probe must already be running; use a fresh
development username and the development password `testing`. Browser clicks walk;
Ctrl-click requests running (the captures below were taken while the run key was
still Shift and have not been repeated with Ctrl). This probe emits no audio requests; existing audio
validation remains the separate animation/audio/NPC slice.

Actual Windows captures against the final probe PID 12756 reach (3211,3215) from
(3207,3216). Walking records 46 GPU samples over 8.31 seconds, five authoritative
tile samples and 14 walk poses. Running records 46 samples over 12.02 seconds,
three authoritative tile samples, four run poses and six energy 234 updates.
Both end at fine coordinates (411072,411584), idle 808 and destination flag (0,47),
with zero diagnostics. The click input is an explicit capture adapter after
successful login/appearance loading; actual requests and returned server movement
drive the rendered result. Normal live mode never executes local preview routes.
Use `--capture-route --capture-route-target 3211,3215` and optionally
`--capture-route-run` with the live endpoint to reproduce it. The native desktop
automation hook was unavailable, so Windows proof uses that input adapter.

Actual browser pointer input is separately verified: a ground click reaches
(3208,3215) with walk 819, and a Shift-click reaches (3211,3215) with run 824,
the final odd single step using walk 819, then idle 808. The final F10 readback
shows run-enabled opcode 215 / payload 810c8b0c0f and no diagnostics. Screenshots
are `live-walk-browser-{0..8}.png`, `live-run-browser-{0..12}.png` and
`live-movement-browser-final.png`. Complete minimap drawing remains separate
parity work; the later wheel checkpoint below covers browser camera normalization.

The previously unhandled zero-payload clear 153 now preserves the original reducer:
X becomes 0 and Y is retained. Route selection sets both fields from the original
compressed destination. Another 128 unchanged JVM branch observations cover clear
and unrelated opcodes. `python tools/check_live_movement_capture.py` checks the
native request bits, authoritative trajectory, locomotion poses, final position,
energy updates and cleared flag. Actual-timestamp silent animations are available
as <https://files.catbox.moe/058ig6.webp> (walk) and
<https://files.catbox.moe/jfa1bd.webp> (run). Original PNG samples and JSON readbacks
are preserved under `runs/visual/live-{walk,run}-native-*`.
The combined checkpoint passes 258 workspace tests across 75 suites and four
movement example tests, with formatting, all-target Clippy, doctests and fresh JVM
checks passing. `runs/client-live-movement-verification.json` links the immutable
`runs/player-walk-verification/report.json`, records the final probe/binary hashes,
and preserves the exact Windows/browser evidence and verified public media.

## NPC update rendering and original indices

NPC_INFO32 now drives original local membership, walking/teleport commands, type
changes, size/turn speed, facing and primary/slot/spot animation clocks. The raw
reader preserves every mask field and the original short-sentinel guard, mask
order, same-cycle retention and type-change behavior (including retained turn
speed). Live rendering and picking follow local-list order and use original NPC
indices rather than positions in the rendered vector. Actor poses share the
player action/movement blend, with NPC resize and movement-first shadow selection.
The later overhead slice adds hit/chat graphics, and later stages slot part
transforms, spot models and ambient emitters. Animation and spot assets absent
from a prepared pack are diagnosed; with `--cache` they are read on demand.

The original complete Protocol NPC readers, requests and NpcList entity phases
match 64 scenarios / 1,536 packets / 1,536 ticks. Synthetic type/sequence/spot
catalogs cover sizes 1 to 3, rotation speeds 0/16/32/64 and both rotate-on-move modes;
hit/area/plugin callbacks are explicit observations. A separate whole-cache corpus
matches metadata from all 8,590 original NpcType streams, including morph children,
palette indices, sound IDs and raw names. All raw NPC definitions now accompany
the live pack. In a prepared pack, body models, BAS and frame assets are there
for the selected eight types only and other bodies stay hidden with
diagnostics; with `--cache` every body is read from the cache on demand.
Cache extraction still matches original bytes across all 29 archives.

`Launch NPC update preview.cmd` runs an explicit offline sequence of 19 original
NPC32/SYNTH_SOUND packets. Its three wire indices 10/20/30 exercise spawn, walking,
removal/readdition, type change, wave 863 and dance 866 with four sound 127 requests.
This fixture demonstrates the client update/render/audio path, without NPC AI,
combat or authoritative server outcomes. Use `--npc-replay
assets/visual/npc-render-replay.json` with the live pack; `--capture-realtime`
retains real 50Hz scheduling in capture mode. The browser start screen provides
the extended **NPC animation and overhead preview (offline)** choice described below.

The packaged Windows release recorded 81 GPU samples over 15.48 seconds, including
31 wave and 21 dance samples, local-count transitions 2/3 and four sound backend
submissions. `runs/visual/npc-update-{state,frames,loaded}.*` and
`python tools/check_npc_capture.py` preserve/check the readbacks; the original PNG
samples are under `runs/visual/npc-update-frames/`. The actual-timestamp silent
animation is available at <https://files.catbox.moe/anie3c.webp>. Audio is verified
through original PCM/queue comparisons and backend submissions, not an audio
track in that image or a physical device recording.

The rebuilt browser also renders those NPC updates. Twelve screenshots under
`npc-browser-{0..11}.png` span the replay. Its original cow menu and attack request
are captured in `npc-browser-menu.png` and `npc-browser-action.png`; the latter's
F10 readback shows opcode 3 / payload 9e00 for wire index 30 after the local list was
reordered. This is an offline outbox request, without combat execution.
The combined checkpoint passes 246 workspace tests across 72 result suites,
including doctests, workspace formatting, all-target Clippy and fresh original
JVM checks. `runs/client-npc-verification.json` records scoped coverage, native
readbacks, browser evidence, artifact hashes and the verified public animation.

The default scene includes Hans, man, woman, guard, goblin, cow, chicken and rat
in a labeled nearby showcase. N switches to the 33 matching ground-plane entries
in the pinned server's `npc_spawns.json`. NPC bodies decode their original model
parts, recolors/retextures, lighting, sizes and idle/walk sequences. Animation runs
every 20 ms with original HD transforms, tweening and animation-before-resize order.
Footprint-sized ground shadows use the original shadow colors and alpha. Local
wander commands respect static clipping and exercise default-rotation movement;
they are a controlled visual adapter, not server AI or network NPC updates.
Dynamic actor collision and authoritative NPC outcomes are not part of this
client. (Morphs, palette recolours, slot part animations, BAS part transforms
and acceleration, and NPC terrain tilt, pending when this was written, have
since been ported.)
The unmorphed showcase now has source-defined talk/trade/attack/examine menus and
item targeting. Its preview indices are offline actor handles; NPC32-driven actors
instead retain their original wire indices.

`cargo run --release -p rs09-client -- --capture-npcs` exports 60 real GPU frames
at 100 ms intervals to `runs/visual/npc-frames/`, plus `runs/visual/npcs.png`.

## Actor overhead text, health and hits

NPC text/health/hit mask fields and callbacks now drive Rust presentation state.
Player hit masks and forced overhead text use the same state and drawing path.
The first expired hit slot is reused, a fifth simultaneous hit is discarded,
hits expire after 70 logical cycles, health bars after 300, and direct NPC/player
text after 100/150 cycles. Removal clears the presentation state for an old wire
index. Drawing preserves self/player/NPC order, the 50-message cap, text overlap
stacking, original colors/effects, four hit offsets, centered damage and health
fill clipping. Artwork and bitmap fonts come from the pinned cache; no replacement
fonts, painted health rectangles or generated hit sprites are used.

The source-hashed corpus executes unchanged `ScriptRunner.drawOverheads`,
`PathingEntity.addHit`, `OverheadChat.loop` and the three original wave/shake font
methods: 512 scenes, 3,136 state commands and 512 glyph-offset cases. Projection,
font metrics, sprite availability/dimensions, friend membership and plugin hooks
are explicit oracle services. The existing 8,590-definition comparison now also
checks NPC headicon, custom hitbar and icon-height fields. Cached player/NPC head
icons are presented, and hint markers are bound to live actors.

Use **Launch NPC overhead preview.cmd** in the Windows package or the browser's
offline NPC preview checkbox. The separate 25-packet fixture extends the preserved
19-packet animation/audio input with chat, partial/full health and four hit slots.
It is controlled presentation input, without combat rules, damage calculation or
NPC AI. `tools/prepare_npc_replay.py --overheads --check` reproduces it; another
test decodes its complete lifecycle and checks valid MISS/NORMAL hit types.

The Windows build records 81 GPU samples over 15.95 seconds, up to 22 overhead
commands, all five chat messages, a discarded fifth hit, eventual chat/health/hit
expiry, four sound submissions and zero UI/NPC diagnostics. Evidence is under
`runs/overheads-validation/native-valid/`; run
`python tools/check_overheads_capture.py` to check those readbacks. Browser samples
`runs/visual/overheads-valid-browser-{0..23}.png` show the same cached yellow text,
red damage, blue misses, health bars and existing wave/dance poses.
The public [sampled animation](https://files.catbox.moe/5jpm58.webp) preserves the
capture timestamps, omitting startup samples 0 to 2; it is silent. A public
[still image](https://files.catbox.moe/oi3hu5.webp) shows all four hit slots.

Client tests and all-target Clippy pass; the 129-case cache oracle and all-29-archive
real-cache JVM audit also pass after adding named overhead asset IDs to the UI
pack. `runs/client-overheads-verification.json` records exact artifacts and the
coordinated workspace gate when available. Full-frame font compositing and the
workbench camera projection are not established by the drawing-command oracle.
(Public chat with its history and filters, live hint markers and slot and
spot models were separate work then and have since been ported; overheads now
use the original's integer `projectToScreen`.)

## What is implemented

This list describes the renderer and the prepared Lumbridge corpus it was
first built on. The areas ported since (interfaces, login, lifecycle, input,
picking, camera, roofs, terrain, locations, lighting, materials, actors,
minimap, world map, music) are in the status document.

- The prepared pack holds four real cache regions around Lumbridge: map
  squares 49/49, 49/50, 50/49 and 50/50. The scene is no longer limited to
  them: `world_cache.rs` streams any region from the cache. All four levels
  are drawn, with the original's roof removal; R turns the removal off.
- The shared Rust CollisionMap supplies original wall/scenery flags. Terrain,
  bridge-plane collision projection and native decoded location collision fields
  populate a 104-tile navigation window. The single-tile ground-click pathfinder
  preserves BFS neighbor/tie order, diagonal checks, nearest reachable fallback,
  compressed turns and movement-packet truncation. Default-player BAS 1426 drives
  original fine-coordinate movement, turning and locomotion animation state.
  The other BAS movement modes, sized routes and plane changes came later.
  NPC collision and the server's walking queues are the server's.
- 706 real model assets in the prepared corpus and approximately 12,000 placed
  meshes, including castle walls, interiors, trees and other scenery.
- Both original model geometry encodings, vertex and face animation labels,
  recoloring/retexturing, per-face alpha, and basic object orientation, scale and
  placement. Old-format redundant texture projections are discarded as in RT4.
- Terrain opcode decoding, original procedural heights and all 13 shaped-tile
  patterns with four rotations each. Floor definitions, ten-tile underlay HSL
  blending, floor textures and height-derived lighting normals are decoded natively.
- Original skeletal bases/frames, implicit transform origins, integer transform
  rounding, rotation wrapping, alpha/color transforms, sequence delays and tween
  flags. The HD path retains four fractional coordinate bits until each complete
  pose finishes, and transforms normals when requested by the sequence. The corpus
  contains 3,894 frames. The avatar demonstrates sequences 808, 819, 824, 863 and
  866 using welded identity-kit parts in PlayerAppearance slot order, with kit and
  body/skin recoloring applied before rendering. Missing parts are load errors.
- Scenery animation is active for the 119 animated locations with meshes in this
  area (two additional animated cache entries have empty model definitions).
  Multipart locations are combined before normals and animation. The location
  clock follows original frame boundaries, replay tails, randomized starts and
  long offscreen gaps; its GL animation preserves the original orientation,
  scale/offset and normal-transform order. Random inputs are controlled from
  location coordinates for reproducible Windows/browser captures. Scenery and
  scrolling textures keep their own clock when changing the avatar preview.
- All 680 texture graphs in the pinned cache generate natively. The row interpreter
  preserves shared-buffer aliasing, seeded randomness, fixed-point rounding,
  dependency brightness changes, transposition and the separate opacity graph.
  The renderer uses actual 64/128-pixel material sizes, repeating/clamped sampling,
  material tint/brightness, blend modes, UV scrolling and alpha cutouts. There are
  no average-color texture fallbacks; unsupported graphs fail loading explicitly.
- Bind-pose UVs for planar vertex bases, cylindrical, cuboid and spherical
  projections, including seam wrapping. Animation preserves these UVs. Smooth and
  flat normals and the GL constructor's filtering/vertex-sharing behavior follow
  RT4. The shader replaces the former absolute face-normal approximation with
  RT4's default HD ambient and opposing directional lights. Location ambient and
  contrast settings, player-body contrast 850 and character normal animation are
  respected. Mesh faces retain the GL constructor's material/priority order from
  the bind pose, including during alpha animation.
- Map atmosphere extensions provide light tint, ambient/diffuse intensity,
  direction and fog settings. FogManager's integer color blending and float
  transitions are ported. Brightness 1 to 4 is live from the B key and CS2
  6001 (default 3), and the fog preference is honoured. Fog uses original
  cache-unit start/end distances and the map's fog color as the clear color.
  The sun direction is re-read each time a scene is activated, as in RT4; the
  shadow projection direction still comes from the scene's start region.
  Local lights decode the original
  span profiles, plane flags, palette colors, flicker presets and custom types.
  Their 2,048-value Perlin noise table, animation arithmetic, light-grid insertion
  order and terrain RGB calculations match original methods. Each object's
  vertices carry a key from which the shader repeats `LightingManager`'s choice
  of up to four lights for that object: its own tile, the first four lights
  over a multi-tile footprint (up to 4 x 4 tiles), the tile across a wall piece
  on the camera's side, and the camera tests of diagonal walls. Actors are lit
  on the level they are drawn on. Terrain is lit by the original's light
  meshes (`light_mesh.rs`), drawn as a multiplying pass after every level's
  floors and before any scenery, as the original orders them. The selection state
  machine, the meshes and floor shadows are compared with the JVM
  (`cargo xtask lighting-oracle --check`); the shader's copy of the rule is not,
  and the two history-dependent cases named in `light_select.rs` are not
  reproduced (see also the placement comparison below).
- Scenery casts projected ground shadows using the original one-pixel-per-eight-
  cache-units masks, signed alpha check, byte-wrapping overlap counts, clipping,
  dirty-tile hash and 68/17 alpha filter. Animated locations remove and replace
  their masks. Only changed 128-square texture tiles upload. The player uses the
  original three-ring, nine-segment ground shadow, animated-body bounds and
  shadow animation flags, including the original HD transform rounding. Every
  level's static scenery casts, whatever the roof state, as the original's
  `addLoc` does. Upper-level tiles add the original's floor shadows when the
  pack has the `floorshadows` sprites (the recorded `demo.pack` does not).
  The character-shadow and scenery-shadow preferences are separate: scenery
  mode 0 hides the map, mode 1 keeps animated scenery's first shadow, mode 2
  rebuilds it as the animation runs. The original's occluders are not ported.
- Original 22,050 Hz sound-effect synthesis: envelopes, oscillators, seeded noise,
  phase/amplitude modulation, gating, reverb, filters and clipped PCM mixing.
  Eighteen original effects are loaded for selectable playback through Windows
  WASAPI or browser WebAudio.
- The underwater scene of regions with `um`/`ul` map files (597 regions of the
  pinned cache): one level of tiles below the surface, its floor
  (`buildGlPlainTiles` with the water arguments), its locations, and the
  backdrop `applyFogFade` paints under the surface's water, drawn before the
  surface under material 3; the water is then blended over it. The map
  decoding, water data, floor mesh and location placement have a JVM
  differential (`terrain_oracle`, second pass), the material's GL state a
  recorded trace (`materials-oracle`); the shader that evaluates that state
  is checked by captures only. Animated underwater locations (none in the
  pinned cache), instanced scenes and packs made before `underwater/` entries
  existed have no underwater scene; the last two keep the water's stand-in
  backdrop.

Full-frame visual parity is not established. Of the differences this section
once listed, these have since been ported: merged scenery normals, per-object
point-light selection, floor shadows from upper levels, the water, liquid,
waterfall, specular and unlit materials, mipmaps, the HD floor mesh with its
transition blending, and location placement with all hill-skew modes. Still
open: depth layers and camera-side wall culling, occluders and non-HD
lighting (see "Still missing" in the status document). Brightness is
controlled at 0.7; RT4's random startup palette jitter is not reproduced.
There is no synchronized original HD frame/pixel comparison yet. A successful
load or attractive screenshot must not be treated as a 1:1 client acceptance
result.

Login, NPC updates, equipment and appearance state, interfaces, client
scripts, map streaming and music, listed here as pending when this section
was written, exist now and are described in the status document. Browser
support has been exercised locally in Chromium; other browsers and low-end
GPUs are unverified.

## Regression evidence

```powershell
cargo test -p rs09-client
python oracle/visual/generate.py --check
python oracle/visual/textures.py --check
```

The client has 13,375 visual/audio/navigation/lighting JVM cases and 1,360 texture-image cases under
`fixtures/visual/`. The HD constructor is checked across the original 703-model constructor corpus:
face visibility, corner normals and material RGBA values match exactly; projected
UVs match within a relative/absolute tolerance of 0.00001; face order is exact.
The 594 avatar HD pose cases
cover every preview tick across complete idle, walk, run, turning, wave and dance cycles,
comparing assembled vertex positions and animated corner normals exactly.
Another 1,800 scenery pose cases compare vertices, normals, colors and alpha
across animated scenery definitions, four quadrants and both normal-transform
modes. The 234 location-clock traces also check emitted sound-trigger indices,
offscreen optimization, replay behavior, randomized starts and loop-counter
overflow. Sound triggers are not yet connected to positional playback.
Environment comparisons cover all 256 flag combinations and the default
constructor. Three atmosphere traces compare every float bit and integer across
1,587 transition steps, including the light/fog values submitted to the original
GL boundary. The Rust shader implements those lighting and linear fog parameters;
its pixels have not yet been compared to an original HD capture.
Scenery shadow masks match across all 703 models in three light directions
(2,109 cases). Six mask-filter cases and eight clipping/overlap/removal traces
compare original texture alpha and byte-count behavior. Another 594 character
shadow cases cover the existing avatar cycles, several headings, stretch flags
and cold-cache fine-coordinate seeds. Their terrain-height boundary is controlled
flat in the oracle; the workbench follows its existing terrain interpolation.
Twenty-four projection cases compare original shadow depth-bias coefficients.
The renderer applies their relative depth change to its workbench camera so the
player shadow remains visible over thin ground decorations such as rugs.
Point-light tests cover all 78 map records and 256 preset/phase combinations,
including negative/wrapping clocks, flicker suppression, custom parameters and
terrain RGB output. Five original grid cases check clipping, plane spans, the
four-light cap and original insertion order with more than 255 scene lights.
The noise table is generated by the original Perlin implementation
in the separate texture oracle and compared byte-for-byte with Rust.
Another 234 cases execute the original map loader's coordinate placement and
104-tile clipping boundary for every cached light at three controlled heights.
Collision-definition checks cover all 780 location types in this scene plus four
opcode-order cases. Another 144 cases compare route results and ground-click packet
bytes, including routes longer than the 25-turn packet limit. Nineteen default-player
movement traces compare 7,980 ticks of positions, facing, animation state, pending
steps and sound-trigger indices against the original movement/facing methods.
Another 24 movement traces cover all six selected NPC BAS types, including the
two-tile cow footprint. Eight definition/BAS cases and eight assembled-body cases
check the selected NPCs. Their 2,094 pose cases cover every idle/walk frame with
and without tweening, comparing positions, corner normals, colors, alpha and
the animated footprint-sized shadow exactly. The original NPC decoder and body
assembly branches are extracted into oracle adapters; morphs and BAS part
transforms are rejected by this declared corpus.

Every one of the cache's 680 texture graphs is compared byte-for-byte against
original RT4 RGBA output at both 64x64 and 128x128. Portable fixtures contain raw
cache graph/sprite bytes, not pre-rendered JVM textures. Other checks cover both
model formats, individual-part/software poses, appearance assembly with three
palette selections, floor definitions and the original scene-builder color window,
procedural heights, all tile shapes/rotations, palette sprites and 18 effects.
Sound comparisons cover every PCM sample and loop boundary.

`oracle/visual/generate.py` extracts the original decoding and math methods from
the pinned source into a separate Java adapter build under `runs/visual-oracle/`.
It does not edit the reference. Fixture provenance records the client commit,
source hashes normalized for Git line endings, and input hashes. Java is used
only to generate/check test expectations. Ordinary `cargo test` uses committed
fixtures and requires neither Java nor the game cache. Reproduction with
`--check` additionally requires Java 17+, Python and the prepared corpus.

These tests validate the stated corpus and data/math paths, not every animation
combination or complete renderer output. Texture operations and raster shapes
absent from the pinned cache are not claimed compatible.

For Windows GPU captures with deterministic preview ticks:

```powershell
cargo run --release -p rs09-client -- --capture
cargo run --release -p rs09-client -- --capture --portrait
cargo run --release -p rs09-client -- --capture --capture-route
cargo run --release -p rs09-client -- --capture --no-shadows
cargo run --release -p rs09-client -- --capture --pinned-lights
```

The program saves `runs/visual/lumbridge.png`, `runs/visual/wave.png` or
`runs/visual/route.png`, and a load
report in `runs/visual/loaded.json`, then exits. Capture mode also submits an
original effect to the audio backend. This confirms the playback path was
exercised; it is not an acoustic loopback measurement or a performance benchmark.

## Local-light placement comparison

The pinned `SceneGraph.java:5293` assigns terrain height minus horizontal Y back
to `lightObj.y`. `LightingManager` subsequently uses that Y as a horizontal grid
coordinate and uses Z as height. It can displace lights outside their intended
footprints. This is a reference behavior, not a decoder failure.

The client's default is the corrected placement (the owner's decision of
2026-11-26): the record's first two coordinates remain horizontal and its third
coordinate becomes height above the terrain. All 78 prepared Lumbridge lights
are available in it. This default changes the pinned loader's behavior and must
not be counted as 1:1 parity. Launch with `--pinned-lights`, or press **P** in
the workbench, for the pinned loader's placement; every frame comparison with
the original client uses it (`oracle/refcompare/launch.ps1` passes the option).
The HUD identifies the active placement. With the
pinned placement 2 of 68 Lumbridge lights get a terrain light mesh; with the
corrected one, 78 of 78. Per-object and wall light selection and the light
meshes have since been ported (status document, "Scene"); `Light.onBridge`
and a GPU frame comparison are still missing in both modes.

The workspace's required formatting, Clippy, tests and primitive-oracle checks
remain required alongside these client-specific checks. Fontdue is pinned to
0.9.3 because 0.9.5 uses an API unavailable on Rust 1.86.


## Cached interfaces and interaction requests

Run `python tools/prepare_ui.py --cache assets/cache` after the pinned cache is
available. It verifies the additional index files and packages the original
31,622 components, 2,063 CS2 scripts, 18 fonts, 14,654 client item definitions,
varbits, enums, inventory/varp types, 4,359 inventory model dependencies, all
6,750 synthesized effect records, the Huffman table and name-icon sheet.
Definitions, scripts and rendering rules execute in Rust; the pack contains raw
cache inputs rather than pre-rendered interface screenshots.

Generate the explicit offline wire fixtures, then launch a native preview:

```powershell
cargo run -p rs09-client --example ui_preview -- gameframe
cargo run -p rs09-client --example ui_preview
cargo run -p rs09-client --example ui_preview -- shop
cargo run --release -p rs09-client -- --ui game --replay runs/visual/ui-gameframe.bin --replay-now 1
cargo run --release -p rs09-client -- --ui game --replay runs/visual/ui-preview.bin --replay-now 1 --capture --output runs/visual/bank-ui.png
```

F5 loads the normal inventory/game-frame fixture; F6/F7 load populated bank/shop
fixtures. F8 opens the cached equipment pane for inspection. F9 toggles interface
presentation and workbench controls; F10 displays queued action bytes and explicit
script/asset diagnostics. The browser build stages the same packs and fixture
files. Letter/number animation and WASD shortcuts are disabled while interfaces
are visible so they do not consume text input.

Implemented client paths include:

- IF1/IF3 decoding, original dimensions/position modes, hidden-widget layout,
  server roots/subinterfaces, texts, visibility, access masks and item containers.
- A bounded native CS2 runner, cached creation/hooks, variables/varbits, server
  script calls, timers, inventory/variable notifications and original delayed
  update protection. Unsupported instructions remain explicit diagnostics.
- Cached inventory, bank, shop, equipment-slot, tab, orb and chat-pane layouts;
  original bitmap fonts, markup, inline name icons, paragraph alignment and
  palette/alpha sprites. Item art uses native integer projection, lighting and
  verified software triangle kernels, including stack variants, notes and lends.
- Item equip/use/drop/examine requests, original ten component button opcodes,
  item-on-item/object/NPC targeting, IF1 rearrangement and IF3 drag hooks/requests.
  Drag ghosts and selection outlines use the original presentation rules.
- NPC menu order/attack priorities and action bytes; the live menu and the
  Examine id use the morphed definition. Object clicks route
  toward the original footprint/side constraints. Zone/location packet replay
  drives replacement, animation, collision rebuild and restoration queues.
- Key and character events for cached input hooks (since replaced by the full
  `Keyboard` port: every key code, auto-repeat, component key bindings), original
  numeric/name/string dialog frames, English public-chat prefixes/Huffman packets,
  local chat history/message notifications, and cached UI sound requests with
  trimmed PCM and finite loop playback. Bank search, text editing and confirmation
  run the cached scripts and update the displayed search results.

The fixtures provide synthetic stock and inventories and explicit empty social/chat
state. They deliver 24 skill packets (level 1/zero XP, except hitpoints 10/1,154 XP)
and run energy 100 through the shared wire decoder. They do not read player saves,
execute purchases/deposits or establish a live session. The normal bank fixture
includes `BankInterface.onOpen`'s mask 2623486 and the source open/update cascade;
`BankContainer.open(Player)` is a distinct overload whose later mask 1278 removes
its generic drag permission. Containers remain authoritative server state; IF1
rearrangement modifies only the original client widget arrays.

Additional reproduction:

```powershell
cargo test -p rs09-client --test ui
python oracle/ui/generate.py --check
python tools/port_component_fields.py --check
python tools/port_client_items.py --check
python tools/port_ui_text.py --check
cargo run -p rs09-client --example ui_models
cargo run -p rs09-client --example ui_run -- runs/visual/ui-preview.bin --drag
```

The UI corpus checks every cached component/item/script decoder, 528 source widget
operation probes, 512 layout probes, 8,192 complete triangle pixel buffers, 148
item/button request cases, 256 delayed/varp scenarios, 512 reorder/drag wire cases,
512 NPC metadata/menu/action scenarios, 4,096 collation cases, 1,024 paragraph
wrapping/alignment cases, 1,069 chat/dialog frames, 645 character boundaries,
all 1,709 sprite sheets and 34 queue-trimmed effect/finite-loop cases. Another 1,669 immutable
text-editing cases and 64 chat history/notification scenarios cover the input path.
Another 96 scenarios compare XP thresholds, skill state and stat/misc notifications;
the shared packet corpus compares 4,313 original writer/reader cases. Skill updates,
run energy and signed run-weight reads drive the native UI reducer. The server
caller still resolves prayer/lifepoint/current-level selection before the skill
writer. The pinned server's disabled `WeightUpdate` writer is not replaced here.
The IF3 release mask is evaluated after callbacks; native and browser bank drags
produce opcode 79 for slot 0 to slot 1 without changing the server container.
Adapters and normalized source hashes are recorded in each fixture. Font pixel compositing,
complete item-icon frames, full CS2 execution/state/reentrancy and pointer event
ordering are not established by these component comparisons.

### Wheel input and interface scrolling

The original `JavaMouseWheel` accumulator now runs in Rust, preserving signed
wrapping, previous rotation and destructive polling. Another 4,128 observations
execute the unchanged pinned AWT listener with actual `MouseWheelEvent` inputs;
`cargo test -p rs09-client --test wheel` compares the complete state and poll
results. `python oracle/players/generate.py --check` reproduces that corpus along
with the other player/NPC/animation fixtures.

The browser forwards raw wheel fields through a small platform bridge, then Rust
normalizes and accumulates fractional events. A supplied legacy detent takes
priority; the explicit fallback sensitivity is 100 pixels, three lines or one page
per detent. These constants are a platform policy, not original hardware parity:
[WheelEvent units and physical-device mapping vary by environment](https://www.w3.org/TR/pointerevents4/#interface-wheelevent).
The bridge suppresses the vendor canvas handler to prevent duplicate raw input.
Native input retains Miniquad's detents with the original down-positive sign.
Input stays pending through render frames without a logical UI cycle, and each
logical catch-up consumes it only once. Hidden interfaces receive no wheel input.

### Keyboard and mouse as the original sees them

`crates/client/src/input` holds the original `Keyboard` and `Mouse` classes.
Window events are taken once per rendered frame, in arrival order, and handed to
the same listener methods AWT called; `Keyboard.loop` and `Mouse.loop` then run
once per 20 ms logic cycle, so held keys, typed keys, the pointer and a click are
fixed for the whole cycle and a click is seen by exactly one cycle.

- Keys: the complete code table (function keys, letters, digits, navigation and
  number pad), the 112 held-key flags, and the 128-entry queue of key codes and
  characters with the system's auto-repeat. Losing focus releases every key.
- Mouse: click button, position and time; held button and pointer position;
  (-1,-1) once the pointer leaves the window (Windows and the browser).
- Component key bindings run in the interface walk: F1 to F8 switch the
  gameframe's side-panel tabs. CS2 5100 to 5102 answer whether Alt, Ctrl or
  Shift is held.

`cargo xtask input-oracle --check` (JDK only, no cache) runs the unchanged
`rt4.Keyboard` and `rt4.Mouse` with real AWT event objects on a headless canvas;
`cargo test -p rs09-client --test input` compares 20,772 keyboard and 4,677 mouse
observations of their complete state. The table from window-system keys to AWT
key codes, the characters AWT typed for Enter, Delete, Backspace, Tab and Escape,
and the detection of the pointer leaving the window are platform policy written
down in `input/platform.rs`; no JVM run covers them. Keys are identified by
position, so letter and punctuation key codes follow the US layout on other
layouts (typed characters are unaffected).

In a live session F9 (hide the interface), F10 (diagnostics) and what F9
unlocks (Escape to quit and the letter toggles) are off unless the client is
started with `--workbench-keys`; offline they work as before. The top and bottom
click bands apply only while the workbench HUD they belong to is drawn.

Browser GPU screenshots in `runs/visual/wheel-*.png` show camera distance
14.0, 15.5, then 14.0 for equal down/up input (the free camera distance of
that stage; the wheel now steps `Camera.ZOOM`), and bank contents advancing one
45-pixel row then returning. A separate hidden-bank check changes the camera
while retaining the first bank row. The long bank input is an explicit synthetic
280-slot scroll fixture (`cargo run -p rs09-client --example ui_preview -- large-bank`),
with the normal staged input restored after QA; it proves no bank transactions.
Native physical-device input and sensitivity across other browsers/devices remain
unverified. `runs/client-wheel-verification.json` records the exact build and
evidence hashes and the associated combined workspace gate.

That combined gate passes 265 workspace tests across 77 suites and seven movement
example tests, with no failures/ignored tests, formatting, all-target Clippy,
doctests and 6,827 primitive JVM cases passing. All 283 Rust sources remained
unchanged. Its immutable report is `runs/world-update-cow-verification/report.json`
(SHA-256 `046302143db1982a5c00360fb7c99caa88f4dae13a11264a5e164a0acf6d167f`).
It includes 257 original world-update scenarios / 1,667 commands with actual
copy-on-write NPC iteration. The unchanged transport/login baseline is linked
explicitly from the preceding immutable report.

An additional actual browser check uses the corrected server artifact on the
separate WS 43603 endpoint: walking reaches (3208,3215) from (3207,3216), and a
later Shift-click reaches (3208,3213) from (3211,3215), visibly using run 824,
the final odd walk 819, then idle 808. F10 records three actions, the last opcode
215 with payload `810c8b0c0d7d80`, and no diagnostic lines. The client disconnects
after capture. The exact server artifact uses empty NPC/content services.
These frozen-build screenshots are `runs/visual/world-update-cow-browser-*.png`;
the client report retains the preceding artifact's scoped captures separately.
The existing movement/stationary/animation endpoints remain unchanged. A public
image of the restored normal bank preview is <https://files.catbox.moe/lrygo8.webp>.

When this section was written the remaining client work it listed was: login
and reconnect behaviour, public-chat masks, slot and spot graphics, streamed
bodies, minimap social markers, model and head widgets, CS1 expressions,
spell targeting, special client buttons, chat and social server updates,
script opcodes and event sources, world streaming and planes, and the other
movement sets. Each of those has since been ported; the limits each still
has, and what else is missing, are in the status document. Server collision/door,
agility and other content outcomes must be validated through authoritative
responses. This is not 100% client or gameplay parity. Finite effect loops are bounded to 256; invalid cache loop bounds
are reported instead of reproducing the original mixer array exception.

## Verified native-server door trace

The joined server scene audit exports `runs/scene-oracle/lumbridge-door.s2c` and
tick/batch metadata. These are native server outputs byte-compared with the
original server over the pinned Lumbridge cache. The client consumes them without
synthesizing the door outcome. Door 12348 at (3207,3217), shape 0/rotation 1,
becomes 10327 at (3207,3218), shape 0/rotation 2, then restores at server tick 500.
Native GPU captures at receipt-playback frames 25/75/125 show closed/open/restored
geometry. Client collision values exactly match the server at all affected cells:

| Tile | Closed | Open | Restored |
| --- | ---: | ---: | ---: |
| 3207,3217 | 1026 | 0 | 1026 |
| 3207,3218 | 16416 | 4104 | 16416 |
| 3208,3218 | 16416 | 82080 | 16416 |

Use a separate view pack centered at (3207,3216). Native `--replay` accepts the
trace directly; `--capture --capture-frame 75` captures the open state. Save each
capture's `runs/visual/scene-state.json` as `server-door-closed.json`,
`server-door-opened.json`, or `server-door-restored.json`, then run
`python tools/check_door_replay.py`. This verifies complete nearby restoration too.
`--replay-now 1` consumes every packet before the first render.

For the same packet stream in WASM, `python tools/prepare_door_replay.py` embeds
the unchanged bytes as `replay.s2c` in a separate pack. Pass that pack to
`python tools/build_visual_web.py --pack assets/visual/tour/lumbridge-door-replay.pack`.
`--repeat N` on the preparation tool repeats the trace for inspection; it does not
change the server's verified restoration time. Replay packets are presented at
50-render-frame intervals. This is an integrated trace test with accelerated
receipt playback. The live stationary-player socket proof above is separate;
door actions have not yet been connected to an authoritative world handler.


## Nearby NPC assets and changing forms

The live Lumbridge pack now includes the original23 NPC types within25 tiles of
(3207,3216), the eight existing preview types, and morph child3385:28 asset types,
27 leaf bodies,864 models,17,775 scenery placements and103 pinned spawn rows.
There are no missing model files. The pack retains every raw NPC definition.
`tools/prepare_visual.py --npc-types ...` adds roots to the preview set and follows
morph children when gathering model/BAS/sequence/frame/base dependencies.

Morphs use original varbit-before-varp selection, the final-child fallback, and
invisibility for child-1. The logical NPC and parent BAS continue ticking while
invisible. Body assembly follows the selected leaf; overhead head icons/hints use
the immediate selected type, while health/hit sizing and shadows retain the raw
parent definition. Missing assets/variable definitions and cyclic morph graphs
produce diagnostics. The nearby Gypsy parent3384 is hidden at default varbit1850
value0 and renders child3385 at value4.

`Launch nearby NPC preview.cmd` and the browser's nearby-types preview send27
logical types in synthetic display positions, followed by original packet37
varbit changes. They are explicit offline rendering inputs. They do not run NPC
AI, apply authoritative spawn positions, or establish live NPC/content parity.
The client accepts the server's actual NPC32 stream through the same rendering
path without restricting membership to the old eight preview types.

Validation adds1,024 original morph-selection/validity cases and988 original
body/BAS/representative idle/walk pose comparisons across27 bodies and13 BAS
records. The pack audit matches287 original binary inputs. Existing512 overhead
scenes also cover distinct parent/morphed head styles. Native GPU capture records
six hidden/visible/fallback phases while preserving all27 logical NPCs, with no
client/UI diagnostics. `tools/check_nearby_npc_capture.py` checks those readbacks.
Evidence and exact artifact/source hashes are in
`runs/client-npc-assets-verification.json`.

These pose comparisons use explicit flat-terrain services. Full body terrain
alignment, arbitrary streamed body assets, palette overrides, slot/spot graphics,
ambient emitters and complete content/live-world behavior remain separate work.


## Item-script metadata queries

CS2 item opcodes4200 through4207 now implement original names, ground/inventory
options, value, stackability, noted/real-item conversion, and membership queries.
This includes the item-membership callback reached by the shop's Buy1 action.
Null/out-of-range options return the original empty string, stackability tests
exactly1, and certificate predicates retain their signed comparisons.
`oracle/ui/item_scripts.py` copies the unchanged original branches and compares
1,024 controlled resolved-item metadata cases through the native VM. Item cache
decoding, template resolution and world membership filtering remain separate
contracts. Item attributes/search and other unported script opcodes continue to
produce explicit diagnostics.

## Minimap and compass review

The shared Windows/WASM renderer now presents the original terrain raster,
mapscene sprites, map functions, compass, NPC/player dots, destination flag and
four-slot hint markers. The original circular click mask drives local routing;
minimap walking uses opcode39 and its fourteen-byte click trailer, including the
retained left-click coordinates when choosing a context-menu action. Special
component actions follow normal UI draw order, so overlaid controls can clear
them. Server state192 and hint217 feed the map; actor hints also feed overheads.

`oracle/minimap/check.py --pack assets/visual/live/lumbridge-minimap.pack`
reproduces six original-source corpora:3,744 raster/mask/click/UV cases,
1,736 indexed-sprite/scenery/bridge/color cases,1,555 real metadata records,
3,072 state/hint/morph/equipment/jitter-helper observations,1,024 complete
minimap movement packets and512 HD edge-arrow transforms. Seven native tests
include an overlay action-order regression. These are component comparisons with
explicit sprite/GL, metadata, RNG and path services; they do not compare a
complete original framebuffer.

Five actual Windows captures exercise hints on/off and minimap states2,3,5,
with no UI diagnostics or item fallbacks. Actual browser checks cover opcode39
from left clicks and context menus, the mask's excluded corner, retained raw
click coordinates, camera rotation, inventory, bank and shop presentation.
The item quantities/stock and offline NPC positions are synthetic. The archived
minimap component checkpoint predates the live gameframe binding described below.

`runs/client-minimap-verification.json` freezes the source, original-source,
fixture, pack, executable/WASM and capture hashes. The matching broad checkpoint
is `runs/entity-regions-verification/report.json` (SHA256
`9d78421b099040ff9cedecda545edad1bb175974bb07999ccad157c11752d49b`):
all37 validation groups pass, including308 workspace tests across90 suites,
18 examples, formatting/all-target Clippy,6,827 primitive observations, the
minimap reproducers,129 cache observations, all29 real-cache JVM archive audits
and transport/login checks. Its326 Rust and10 Cargo files match the component
report snapshot. A new native live-probe launch was blocked by automatic approval
review; this checkpoint claims actual Windows offline captures and browser live
NPC evidence separately.

The isolated Windows review is `runs/windows-client-minimap/Launch client.cmd`;
the browser review is `http://127.0.0.1:8099/minimap-review/`. Both use provisional
pack SHA256 `d799e6dc48363f48245686396737b8bc1857830d110200a7cf4042db40e45463`.
The canonical pack and normal stable staged packages remain unchanged. Actual
public captures show the [Windows minimap and hints](https://files.catbox.moe/yud6sp.webp)
and [browser bank with minimap](https://files.catbox.moe/v1t91s.webp); both remote
files were downloaded and hash-checked against their local captures.

Still missing for the minimap: a pixel comparison of the composed map with
the original. Since ported (`cargo xtask minimap-oracle --check`): the
session's random offsets and the periodic drift of rotation and zoom, friend
and clan dots, ground-item dots, the seeded floor jitter, re-rastering only
on a scene rebuild or plane change, and text labels. The pinned cache flags
none of its labels for the minimap, so none is drawn with it. Authoritative
doors, agility, bank/shop and other content outcomes are the server's.

## Live gameframe and initial skills

The corrected native session sends REBUILD_NORMAL162 before interface traffic.
Its original resizable gameframe746 opens241 visible nodes and a real empty
28-slot inventory. Native and browser captures demonstrate authoritative walking;
the browser sends original minimap39 requests, reaches3210,3212 and returns to
3207,3216. Local inventory and spellbook tabs work. This is a bounded native
session with explicit empty content/profile services, rather than a complete
login or gameplay implementation.

`runs/client-gameframe-verification.json` records exact archived Windows/WASM
builds, actual captures, server canonical rows and source hashes. The matching
combined checkpoint is `runs/gameframe-verification/report.json`, SHA256
`56d12b848624de9f204496d3c4b0bb4ac1b743e5834a6d9b235e9cfca59fc418`:
all41 validation groups pass, including310 workspace tests across91 suites.
The earlier failed startup capture is retained separately; it is excluded from
the successful proof.

The fresh-skills endpoint TCP43612/WS43613 adds24 original stat38 packets:
10HP with1154XP, other playable levels1/XP0, and100run energy. The actual Windows
capture in `runs/client-player-skills-verification.json` displays10HP/1prayer/100run
with no UI diagnostics. Server Settings and Skills differential tests are recorded
separately by the server agent; this does not imply that all Settings controls,
combat, profile loading or skill content are bound to the session.

## Windows location ambience and PCM output

The current Windows client owns a WinMM22050Hz stereo PCM16 output. Finite
server/UI/sequence effects and location ambience share the same wrapping mixer.
The port preserves forward integer-rate sample playback, original loop start/end
bounds, additional-repeat counts, volume scaling and24bit-to16bit saturation.
No JVM, external decoder or external audio mixer runs in the client. The
non-Windows finite-effect path also corrects its volume divisor from255 to256.

Location AreaSound metadata comes directly from raw prepared LocType records.
The runtime preserves rotated emitter rectangles, the64fine-unit margin,
plane/volume gating, primary loops, secondary countdowns, cache retries and
one-level morph selection. Server varp polling records each individual active
value prefix before the next assignment, as the original updateMulti callback
observes it. Direct CS2varp2 updates refresh morphs; opcode27 does not introduce
an extra refresh. The provisional map has120 location emitters and90 distinct
ambient sounds, all present in the existing UI pack.

`python oracle/audio/check.py` reproduces five original-source corpora:
3,072 stream observations plus128 clipping buffers;128 emitter constructions
plus4,096 redraw ticks;1,274 real location records;90 complete raw sound syntheses;
and2,048 varp polls with5,882 individual return snapshots. Original source hashes
and explicit service boundaries are recorded in `fixtures/audio/`.

`runs/client-audio-verification.json` freezes the current exact build, corpus and
actual Windows evidence. The live fresh-skills capture has101 GPU frames over
28seconds,25 observed NPC slots,22 moving/animating slots, authoritative walking
to3211,3215, the original gameframe and no UI/backend diagnostics. Its fireplace
loop gets louder as the player approaches. A separate controlled animation probe
has71 GPU frames, wave863 and three finite effects mixed with the ambient loop;
that older probe supplies no gameframe, skills or NPCs.

Each actual Windows audio command trace is replayed through the unchanged original
SoundPcmStream and JavaAudioChannel.write. The live589,056 stereo frames
(2,356,224PCM bytes) and mixed384,768 frames (1,539,072PCM bytes) match byte for
byte. These are app-owned samples submitted successfully to WinMM, rather than
device loopback recordings. Raw sample synthesis is compared independently;
the replay takes those samples as explicit input. The old workbench
`audio_playback_submitted=false` field describes its manual preview toggle;
`audio-state.json` records the finite/ambient backend.

The exact current binaries are archived under `runs/client-audio-validation/bin/`.
The older browser review and its immutable gameframe proof retain their previous
WASM build. A separate current-build browser regression is recorded in
`runs/client-audio-validation/browser-current-report.json`: actual inventory-tab
and minimap clicks display the real empty inventory, reach3210,3212 and return
to3207,3216, confirmed by the native server's canonical trace. The isolated page
is `http://127.0.0.1:8099/audio-review/`. Actual current Windows
[gameframe/skill-orb visuals](https://files.catbox.moe/lcjbuw.webp) and a
[silent NPC-animation excerpt](https://files.catbox.moe/fvb7ne.webp) are public;
both remote files were downloaded and hash-checked against the local captures.

Location ambient output was Windows-only at this checkpoint (the browser
worklet and music came later; other desktop systems still have neither).
Music/MIDI, NPC/player continuous ambience, fractional pitch, fades, pingpong, the complete
priority scheduler, exact device latency and all audio preference controls remain
separate work. RNG/controller oracles use explicit tapes; runtime replay uses
seeded JavaRandom0. Full original RNG order/world phases and complete visual,
audio, UI or gameplay parity are not established by these component checks.

The combined held checkpoint is `runs/settings-verification/report.json`, SHA256
`b583011a98f9443177236af2b17c776d620bd59e6a5729b2faf663277febbdda`.
All37 validation groups pass, including317 workspace tests across94 suites,
22 example tests, formatting/all-target Clippy, native/WASM builds, fresh UI and
minimap comparisons, both independent actual-PCM replays, all29 real-cache JVM
archive comparisons and transport/login interoperability. The340 Rust files,
10 Cargo files and4,311 recorded oracle/fixture/asset inputs stay unchanged through
the relevant checks. Across both actual audio captures,3,895,296 PCM bytes match
the original player.

The isolated current Windows review is
`runs/windows-client-audio/Launch live client.cmd` (default TCP43612).
`Launch animation and audio.cmd` selects the separate controlled43598probe;
`Launch client.cmd` opens offline UI previews with synthetic stock. The exact
captured executable, provisional pack, UI pack, launch scripts, smoke capture and
checkpoint links are hashed in `runs/windows-client-audio/build.json`.
The package requires neither Java nor Python to run. Its UI requests still need
the server's remaining native content handlers to produce full gameplay outcomes.

The current source also draws cached type6 model widgets: static models, NPC and
player heads, partial heads, item models and the already prepared body models.
It uses the original UI projection, lighting, inherited raster clip and animation
clocks. Live player-head updates sample the received appearance before subsequent
UI packets. Seven model/position packet readers retain the original delayed
updates, including the500ms client protection window. NPC dialogue preview uses
interface241 inside752:12 and the original Cook278 facial sequence9760.

The separate `assets/visual/ui-models.pack` is now the runtime default. It retains
the original UI inputs and adds raw head/static models, identity kits and original
facial/widget animation dependencies. The older `ui.pack`, world packs and frozen
Windows/browser reviews remain unchanged. Run `tools/prepare_ui_models.py` to
reproduce the new pack and `cargo run -p rs09-client --example ui_dialogue` to
generate explicitly offline NPC/player dialogue and item-message review packets.
These reuse the synthetic-stock gameframe fixture; they run no content handler.

Focused comparisons cover1,024 original camera/scissor cases,1,024 animation
clocks,38,590 head-assembly cases,2,048 entity-animation dispatches,2,048 exact
server packets and256 joined delayed-update scenarios. Another29,308 real item
and count-variant cases compare the complete original ObjType.getModel and GL
constructor/resize/bounds with Rust, including geometry, normals, UVs, palettes
and face order. Item normals/UVs are constructed before GL resizing; object-widget
centering preserves the original signed-short minimum height.

This does not establish pixel-for-pixel UI or full gameplay parity. Conditional
legacy text/sprite styling, full hover/hierarchy animation behavior, character-design
DEFAULT appearance, complete body animation/equipment overrides, palette-setting
services and unrestricted body/animation asset streaming were separate work
at this stage. Since ported: the character-design client codes, the recolour
palette (CS2 5401) and reading any body or animation from the cache.
Bank/shop transactions and live dialogue still require native server content
handlers; the cached layouts, item rendering and click packets are client work.

Later source work also binds original CS1 conditions to model selection and
animation clocks. The2,050 JVM expression/condition cases include the actual
casket varp261 states;512 complete Component.getModel dispatch cases preserve
active model/sequence choice and the partial-head primary modelId quirk.
The same existing model pack covers all239 conditional model widgets in the
cache. The original casket289 interface draws10 models at varp261=0 and15 at1
in the Windows review; the browser shows the corresponding highlight change
with an empty warning/error log. These are explicit offline states, with no
casket gameplay handler. Legacy text/sprite styling, hover/hierarchy behavior,
character design and full body/equipment services remain separate work.

Equipped-player body review now uses the separate `assets/visual/ui-players.pack`
prepared by `python tools/prepare_player_models.py`; the earlier UI/model and world
packs remain unchanged. `cargo run -p rs09-client --example player_preview` creates
explicit offline male/female rune equipment appearances. Native
`--appearance-preview runs/visual/player-male.bin` and the browser launch selector
exercise the same model assembly as live appearance updates, with cached equipment
index resolution, three-part gender-specific wearables, offsets, ordered colour/
texture replacement, BAS raw part transforms and primary-sequence slot5/3 overrides.
Type5 player-body widgets rebuild for their own sequence overrides before posing.
The full original wearable/kit body and raw player assembly comparison covers
33,130 cases, including all 14,654 items in both genders, 308 kits, 1,431 BAS records,
synthetic empty/repeated kit arrays and transforms, and composed bodies.

These are rendering/input fixtures, not equipment transactions. At this stage
runtime movement accepted only BAS records without acceleration; acceleration,
accelerated yaw and lean have since been ported (no cached record uses them).
The rest of this paragraph is the list as it stood then. NPC player transforms,
slot-source masks and slot animation overlays, body cache fallback identity, full
terrain/acceleration services, unrestricted asset streaming and complete gameplay
remain separate. Appearance previews cannot override a live actor.

Specular material type1 now preserves primary/vertex opacity, matching the original
material bind and display-list alpha rules for both lighting settings and cube-map
capabilities (336 JVM cases). The texture alpha remains a shine intensity mask.
This change fixed incorrectly translucent metal without claiming full material
or GPU pixel parity. The specular highlight itself was ported later,
evaluated analytically per fragment rather than through the cube map; no
capture shows it clearly.

The world renderer now omits static chunks entirely beyond a GPU side clip plane.
Bounds are created with each initial, inserted or reused chunk. Index-only scenery
removal may leave a larger conservative box. Picking, simulation, animation clocks,
shadow casting and near/far depth clipping retain their existing paths. Native
`--no-frustum-culling` disables the optimization for rendering comparisons.

The checked native comparison uses the same release binary for both modes, with
11 fixed views including a door replacement and five older tour packs. Visible
scene differences are at most one channel level; the older world-only tour views
explicitly disable UI and exclude the workbench's changing FPS header from the
comparison. Full uncropped screenshots and difference images remain in the report.
Two safety tests independently clip box triangles in double precision across
8,192 boxes and retain uncertain bounds and depth-biased geometry.

Three repeated 240-frame native workloads, alternating modes with 20 warmup frames,
reduced median render-window time from 9.956 to 5.953 seconds (close view),
9.849 to 6.971 (high view), and 12.793 to 6.919 (Varrock). These compare local Rust
rendering with submission culling disabled/enabled; they do not measure original
JVM performance, live world/player capacity, or browser performance. The measured
window includes GPU/presentation and other frame work, excludes loading/end
capture, and retained background test browsers. Static CPU submission costs are
recorded separately in `runs/client-frustum-validation/final/benchmark.json`.

Persistent static-world buffers now share the existing world GLSL, atlas,
lighting, fog, depth and blend settings. Each chunk owns its vertex/index buffers;
scenery removal invalidates them before changing CPU geometry, and replacement
chunks start without buffers even when their sizes match the previous mesh.
CPU geometry remains available for picking. Buffers, pipelines and shaders are
released on the rendering thread. `--cpu-static-mesh` selects the existing native
streaming path for comparison; a shader creation failure also uses that path.

Ground packets33/135/240/14 and zone envelopes feed
`rs09_protocol::game::ground` once per outer frame. The visual adapter supplies the
shared item catalog and original signed terrain interpolation, then renders the
selected primary/secondary/tertiary models at the tile centre. Item counts choose
the existing item-model variants. Minimap dots and menus observe the shared lists
without merging duplicate IDs. Take, Examine and item-on-ground actions use the
original wire order; walking tries the exact tile before a 1x1 access fallback.
Ground updates use the actual network self slot and current plane, including
PLAYER_INFO plane changes. Levels above the ground are drawn in a live
session with the map streamed from the cache or from prepared packs alone.

Pile support height is captured during publication, as in SceneGraph.setObjStack.
A later scenery change does not recalculate that height until the pile is
published again. Model availability and item-definition fallbacks remain visible
in diagnostics. Normal scene rebuilds shift the shared lists, rebuild their
published views and apply the original self teleport queue operation.
Logical rebuild publication now precedes the diagnostic for unavailable upper-floor
visuals. The raw-packet regression covers planes 0, 1 and 3, followed by a ground
update at the new base, retaining item identity and the original non-discard
teleport queue behavior. World and minimap clicks are disabled while the current
logical plane has no supported visual scene.

The persistent-buffer comparison used native binary `05719867` for three workloads,
three alternating repetitions and 240 frames with 20 warmup frames. Median render
windows improved by 2.57x (close view), 2.92x (high view) and 2.22x (Varrock).
These local Rust comparisons include GPU/presentation, actor, UI, light and shadow
work but exclude loading and end capture; background browser/development activity
remained. The later logical-rebuild correction has five fresh CPU/GPU comparisons
matching both each other and that prior native's screenshots within one RGBA level.
The 18 measurements themselves remain explicitly associated with the prior binary
in `runs/client-ground-validation/final/benchmark/report.json`.

`cargo run -p rs09-client --example ground_preview -- <output-directory>` creates
explicit offline spawn, count, duplicate removal, owner filtering, zone clearing
and table-support inputs. The support sequence removes/restores a real cached
table and republishes its coin pile separately. These are presentation and wire
fixtures; authoritative drops, decay, pickup/inventory transactions, bank/shop
handlers and full content remain server integration work. The ground menu and
geometry differential fixtures add1536 and2048 original-client cases respectively;
their adapters and exact scope are recorded beside the generated observations.

Non-accelerated BAS movement now preserves directional run and slow-walk sequence
selection, including the original full/CCW/CW ID equality priority. All 1,431
pinned cached BAS definitions decode, with terrain dimensions retained. Six
definitions previously rejected directional-running opcodes; Monkey132/BAS116
is captured as an actual native and browser model/animation review. Every cached
movement sequence is already present in the unchanged UI/player asset pack.
The differential corpus covers 1,527 cached/synthetic definitions and 23,040
original movement/facing ticks. A separate 46-case monkey corpus compares idle,
walk, run, tweened geometry and shadows with the original GL model methods.
The earlier 9,216 default-motion observations and 988 nearby-NPC pose cases
remain unchanged. Accelerated movement and per-part BAS transforms, separate
work at this stage, have since been ported; NPC AI and content are the
server's.

Windows NPC ambient playback now consumes the original NPC32 type/add/remove
events, resolves cached morph audio at the varp-refresh phase, and redraws after
actor motion. The portable collection preserves actor reference identity,
duplicate entries, first-reference removal, original movement-ID alias priority,
null-morph primitive defaults, radius staleness and distance gain. It shares the
existing native22050Hz stereo PCM backend with location and finite effects.
Only received/offline-replayed NPCs enter this lifecycle; the older locally
generated showcase is a separate visual preview.

`python oracle/audio/check.py` now also reproduces66 NPC scenarios with4,389
ordered commands, all8,590 cached NPC sound radii and all19 effects referenced by
the71 audible raw NPC types. Forty additional unchanged original GL cases cover
the cached Dwarven Miner2207/2208 bodies, movement poses and shadows. Existing
NPC metadata and988 nearby-NPC pose observations remain unchanged.

The actual native `debc6456` replay retains36 GPU frames, both NPC types, sound
IDs1605/2411 and emitter counts2/1/0 through replacement/removal. Cached BAS437
aliases idle and walking to2150, so both use the idle sound category. The initial
sequence-1 aliases missing run IDs. The real-time run starts/stops five voices;
its169,728 stereo frames (678,912 PCM bytes) match the unchanged original mixer
SHA256 `3f87f884cbfe0e693414f1f40c388d8d9a51f8fe3bf474dc406f6f2c279c3874`.
This records app-submitted PCM, not device loopback or synchronized video audio.

`runs/windows-client-npc-audio` retains earlier inventory, bank, shop, ground and
monkey previews and adds `Launch Dwarven NPC audio.cmd`. All40 packaged runtime
files remain unchanged after five package-directory capture runs. Current native
and WASM `d11c4f94` clients also display19 actual configured piles from the opt-in
owning-world ground session; a nearby browser walk retained all19. These live
visibility checks do not establish Take/Drop or authoritative bank/shop handlers.
The isolated browser review is `http://127.0.0.1:8099/npc-audio-review/`.

Continuous NPC/location ambient output remains Windows-only. Player ambient
emitters, music, resampling/pitch, priority scheduling, device latency, slot
rendering, higher-floor scenes, unrestricted streaming and complete visual/UI
and gameplay parity remain separate. The exact build, original-source checks,
runtime captures and scoped remaining work are recorded under
`runs/client-npc-audio-validation/` and its next combined checkpoint.

Actor-attached spot graphics now decode the pinned SpotAnimType definitions,
assemble original recolors/retextures and animate through the original GL pose
path. Scaling, quarter-turn rotation, height and optional terrain attachment
preserve the original transform order and unchanged normals. NPC32 and
PLAYER_INFO requests share the existing original priority/clock/clear rules.
Each effect draws immediately after its owning player or NPC and participates
in NPC picking. Missing required cache data remains an explicit diagnostic.

The new JVM corpus compares all1,993 cached and256 synthetic definitions,
304 original GL poses/attachments,512 terrain samples and36 extreme-coordinate
resize/rotation cases. Eight cached sequences retain original sound-cue
observations; graphic457/sequence2789 emits sound2574 at frame7. Original raw
synthesis and actual Windows submitted PCM are checked separately. The captured
252,160 stereo frames (1,008,640 PCM bytes) match the original mixer SHA256
`fef493f50d3d25dd9cefb16243333953c00c2066b789d56d7eb1a4797a10c1f4`.
This is app-submitted audio, not device loopback or synchronized video sound.

`runs/windows-client-spots` adds actor-effect and finite-sound launchers while
preserving earlier UI, ground and NPC previews. Its separate asset pack contains
all1,993 cached definitions,1,163 models,1,335 sequences,28,083 main frames and
895 bases with no missing dependencies. Native GPU-frame captures and browser
ice-effect frames show actual cached effects on animated actors. The browser
review is `http://127.0.0.1:8099/spot-review/`; its repeated replay is explicitly
offline and finite. Self requests in this replay are not PLAYER_INFO wire proof.
Particles, floor/projectile graphics, slot and per-part BAS transforms, full body
terrain alignment, complete material/camera fidelity, upper floors, unrestricted
streaming, complete interface targeting and authoritative gameplay remain open.
Bank/shop interfaces and packet generation do not establish live transactions;
door/agility content still requires the server's authoritative handlers.

The separate stationary server probe onTCP43616/WS43617 now sends actual
PLAYER_INFO225 spot masks. Native `b964d2c4` retained301 GPU frames across18.409
seconds, rendering graphics111/369 at heights0/64/256 and returning to no effect
after explicit clears. Its height128 delayed request is replaced before it can
become visible, as specified by the server timeline. WASM `a44c09e5` connects
cleanly and shows the cached effect under that same live input. The current live
browser status omits spot-pose metadata, so exact pose/height assertions use the
native trace and original-source fixtures. The server's byte-level/headless
proof remains separate in `runs/player-spot-session/report.json`. This isolated
probe supplies appearance and effects; it is not movement, combat or a complete
world session.

Browser location/NPC ambience and finite effects now use a dedicated Rust
AudioWorklet engine. The unchanged synthesis and Voice/Mixer/clip/WAV numeric
bodies are shared in `crates/audio`; the public client APIs re-export them.
`crates/audio-worklet` accepts prepared synth PCM and play/gain/stop commands,
then renders the same256-frame integer stereo blocks independently of graphics
and UI callbacks. The Windows WinMM device implementation remains unchanged.
`tools/build_visual_web.py` builds and stages both WASM modules and their small
platform bridges. This follows the browser's [Web Audio API](https://www.w3.org/TR/webaudio-1.0/)
with a22050Hz AudioContext; hardware rate conversion and latency remain device
services, not claims of original resampling/pitch or scheduler parity.

The isolated `http://127.0.0.1:8099/audio-worklet-review/` stage uses client
`28e508d4` and the79,124-byte standalone worker `ca694331`, which imports no
graphics or host functions. Three actual eight-second captures each retain
176,640 stereo frames (706,560 PCM bytes). NPC5-play/5-stop, continuous location
4-play and finite sequence1-play inputs match both the actual Float32 output
channel readback and the unchanged original JVM mixer, with separate original
raw-synthesis comparisons. Initial silent graph-construction callback skips
occur before every voice in two captures; no quantum gaps occur after PCM
begins. These are app-owned output samples, not hardware loopback recordings.

Cold bank loading preserves the context-minus-mixer frame offset exactly for
both NPC and sustained location workloads. The older render-thread prototype
recorded a397ms gap on cold UI loading and is preserved separately. Actual mute
cleanup now removes all worker voices; M is a workbench control when interfaces
are hidden with F9. Player ambience, music, pitch/fades, full audio scheduling,
all browser/device combinations and complete visual/interface/gameplay parity
remain open. Detailed source/runtime evidence and remaining work are under
`runs/client-audio-worklet-validation/`; earlier verified packages are preserved.

## Selected-component targeting

The Windows `33c03a54` and WASM `4c5239c6` client adds the original selected-spell
menu and action path. IF1/IF3 selection, target masks, typed parameter defaults,
`onUse`/`onUseWith`, item-mode cancellation and transformed packet fields are
ported. CS2 retains component identity when a child is deleted or replaced.
Ordinary script errors are diagnosed before MiniMenu continues its outer state
transition, matching the original ScriptRunner boundary. The UI pack adds all
670 original ParamType records; previous raw asset bytes are preserved.

The unchanged-JVM corpus covers 896 action writes, 512 widget menus, 20 parameter
decodes, 400 lifecycle scenarios with explicit callback adapters, and 1,440 typed
target conditions. Actual browser pointer tests select Wind Strike on a Cow,
alchemy on a Bronze sword, and Telekinetic Grab on the live world's Bowl. All
three emitted requests match independently reproduced original writes. The
ground observation also records the actual route77/spell73 WebSocket sends.
Cache item1923 is Bowl and1931 is Empty pot; an initial QA lookup assumption was
corrected without changing client code. These requests do not establish server
spell effects, item conversion or pickup outcomes.

The joint `runs/ground-options-verification-2/checkpoint.json` records461 workspace
tests, 46 root gates, 5,420 unchanged held inputs and both complete client proof
manifests. Independent parent/delta readback verifies5,942 file paths. The Windows
package `runs/windows-client-targeting/` preserves the previous previews and adds
`Launch spellbook.cmd`; all47 payload files pass eight package smoke scenarios.
Three Windows UI captures, two fresh native PCM/JVM comparisons and two full-frame
renderer regressions are retained under `runs/client-targeting-validation/`.
The browser review is `http://127.0.0.1:8099/targeting-review/`.

Nearby-player picking and targeting, open at this stage, are described in the
next section. Full visual parity remains open. The seven serialized target families are not a claim
that every target kind has an end-to-end scene/UI binding. Doors, agility,
Take/Drop, banking, shops and spell effects still require authoritative server
content and transaction bindings. The earlier sealed checkpoints and packages
remain immutable.

The follow-up `runs/client-pickup-assets-validation/` adds the missing raw sequence
535 to a separate UI pack. Its ten distinct frame records and base0 already match
the parent pack exactly. All63 direct/tween GL pose observations match original
positions and normals, and an unchanged Windows client captures15 GPU frames of
the equipped body's finite action followed by idle. Actual browser/live535 and
server pickup outcomes await the corresponding server input; sound2582 was
already present. This additive asset proof does not change the sealed47-file
package or the client executables.

## Nearby-player interaction path

The follow-up client adds body picking for rendered nearby players, original
SET_INTERACTION options, team/level attack priority, item use and selected-spell
targets. A player spell target uses mask bit8 without the ParamType filtering
used by NPCs, items and locations. The original 1x1 player approach precedes the
action request; a departed player is checked again before sending. Picking uses
the displayed body triangles and the workbench camera, rather than claiming the
complete original viewport/overlapping-entity selection algorithm.

Original name/title formatting and CS2 5422 preserve lazy title-array allocation,
empty assignments, null entries and completed writes before an error. The
original-backed corpus covers1,024 menus,120 action writes,128 eight-update
SET_INTERACTION sequences and128 name/title sequences. Six Rust tests include
real CS2 title callbacks. The new oracle is included in the full UI check.

Actual browser input on `player-interaction-review/` offers Wind Strike on three
connected players and sends route77 followed by player-spell195 for Targetinput
(index3). The bounded server currently sends no ordinary player options, so
Follow/Trade menus remain explicit packet/corpus evidence. Live item-use input
and authoritative follow/trade/combat/spell outcomes remain separate work;
transformed-player bodies and the cross-entity overlap menu were ported in
the sections below. Windows/browser
build, source and runtime receipts are under
`runs/client-player-interaction-validation/`; a joint workspace checkpoint is
required before treating this follow-up as a verified combined milestone.


### NPC-transformed player appearances

The integrated player renderer now delegates NPC-transformed appearances to the
selected NPC body. It keeps the player's packet BAS for movement, uses the base
NPC definition for actor size and shadow visibility, and uses the selected morph
child for model parts and post-animation scale. Active varp/varbit changes can
hide or replace the body without another appearance packet. Normal player names,
action options, item/spell targeting and body-triangle picking remain player
services. NPC bodies ignore human sequence equipment substitutions.

Raw NPC assembly accepts original per-part offsets, BAS rotate/translate data,
recolour palette entries and retextures before GL animation. Full player-body
widgets apply the NPC scale after their existing UI animation. Appearance
decoding preserves both original position mutations, including repeated NPC
appearances, raw flag size changes and NPC-to-human recentering. Cached
appearances are restored before an added actor's teleport.

`oracle/ui/player_forms.py` reproduces1,024 original player position/form cases,
five raw NPC assemblies and640 GL pose observations, including actual scaled
chicken geometry, a controlled multipart BAS/palette body, integer tween
fractions and both normal-transform modes. The three Rust regression tests also
require explicit errors for missing definitions/variable services and cyclic
morph data. The original reference remains unmodified.

Windows evidence under `runs/client-player-forms-validation/` includes walking
and idle transformations, a hidden-visible-hidden NPC morph, original bank/shop
and spellbook interfaces, a real bounded server session, two full-frame prior
build comparisons and an original-mixer PCM comparison. The browser stage is
`http://127.0.0.1:8099/player-forms-review/`; its appearance selectors use explicit
offline packets generated by `tools/prepare_player_forms.py`. Preview variable
packets and inventory/bank/shop contents do not establish authoritative server
transformation, doors, agility, pickup or transaction outcomes.

Larger transformed-self pathfinding still uses the existing size1 client route
kernel; actor motion/teleports use the actual form size. Complete overlapping
entity menus, per-slot animated body transforms, full body terrain alignment,
original unavailable-model cache fallback and every special player/NPC form
remain separate work. This follow-up does not establish complete visual parity.


### Sized client routes and interaction approaches

Client routing now selects the original size1/2/N frontier and reach rules for
the player's actual appearance size. Ground and minimap clicks, scenery, NPC,
player and selected item/spell approaches use the canonical movement queue head.
Offline keyboard steps test the complete footprint. A transformed player's
centre coordinate can differ from that head; dividing the fine coordinate by128
is not the original routing source for larger bodies or queued movement.

`oracle/ui/sized_routes.py` executes the complete pinned PathFinder dispatcher,
all three BFS routines, CollisionMap reach methods and ClientProt move writer.
The18,068 observations include individual edge/corner flag probes, rectangular
and wall/decor approaches, ground exact-then-adjacent fallback, nearest-tile
selection, compressed turns and the25-turn packet limit. Sizes1/2/3/4/8 have
mask cases; complete searches also exercise16/64. Four Rust tests compare those
observations. `tools/port_client_reach.py --check` pins the transcribed wall and
decoration branches to their original source and is part of the full UI oracle.

Original quirks remain observable: the larger wall/decor early condition omits
the actor Y coordinate, rectangle contact can exclude an actor that strictly
covers the target span, and the N frontier retains its original corner masks
and102 bounds. The bounded Rust API rejects invalid actor footprints and treats
out-of-map flag probes as blocked. The oracle uses loaded104-tile maps with the
original blocked five-tile outer margin; this does not establish malformed-map
exception identity or full scene construction parity.

Native captures now include the complete navigation flag input for independent
original-JVM route replay. The earlier1tile self-routing limitation is addressed
by this kernel and its normal client call sites. Authoritative movement/content
acceptance, complete scene streaming/higher floors, overlapping-entity selection
and all remaining visual/UI/audio boundaries remain separate contracts.

Actual Windows size2cow and size3gargoyle routes now match complete original
move writes on the captured navigation grid. Replaying those same cases with
fine-coordinate/128 sources produces different writes. The gargoyle's observed
idle/walk sequences also match256 original GL vertex/normal poses, including
negative, zero and interpolated clocks. These receipts live under
`runs/client-sized-routing-validation/`; browser/package checks and a combined
workspace checkpoint are separate verification steps.

### Original overlapping actor menu expansion

`entity_menu` ports the NPC/player branches of the pinned `MiniMenu.addEntries`:
NPC alignment depends on footprint parity, player alignment always requires
fine-coordinate remainders of 64, and containment uses the original signed shifted
differences and wrapping integer arithmetic. It preserves NPC-list then
player-list order, picked actor last, repeated local slots, repeated expansion
from distinct keys, and suppression of consecutive identical full64-bit keys.
Every expanded entry retains the original picked tile. Missing picked actors
are explicitly skipped instead of dereferencing stale/null renderer state.
Original `ScriptRunner` producer statements also establish that actor keys
carry zero tile coordinates and noninteractive NPC keys carry the sign bit.
Normal actor picking now follows these rules, including the base NPC type's
interactive flag before child-specific menu filtering.
The original GL picker accepts only positive signed keys: an NPC with its sign
bit set and player wire index0 cannot seed a pick. They may still be considered
when a different picked actor expands its containing local lists.

The unchanged original branches and picked-key dispatch execute with explicit
actor-field/getSize snapshots and recording menu callbacks in
`oracle/ui/entity_menu.py`. The corpus has 1,686 observations including alignment,
sizes1/2/3/4/8, ordering/repeated keys and signed overflow, plus 32 original actor
scene keys. Four native tests compare those observations. The main world menu
applies expansion to the existing ordered mesh hits
and routes entries through the existing NPC/player/item/spell menu functions.
NPC menu arguments now retain the picked tile as the original does.

At this stage the pick order came from mesh hits; the original's scene draw
order and hover test were ported later (`picking.rs`, `scene_order.rs`).
This section does not establish callback-mutating actor getters, every menu/script behavior or accepted server
interactions. Windows/browser reports under `runs/client-entity-menu-validation/`
use an explicit Cow2 footprint containing Hans1 and Man1. These are offline
original-format NPC32 packets; selecting Talk-to proves an outgoing request,
not a server conversation. Original cached world/UI asset packs are retained.
The earlier attempted Gargoyle NPC preview records its missing world-pack BAS
diagnostic; the separate player-transformation asset path does not close that
NPC asset-pack gap. Prior packaged sized-routing evidence remains immutable.

### Actor bodies on slopes

Normal NPC and player bodies now apply the original terrain alignment after
animation and resize. Shadow geometry and overhead height use the body bounds
captured before alignment, matching the original render order. NPCs use their
parent definition's BAS dimensions; transformed players retain their own packet
BAS. The existing integer scene-height sampler includes bridge promotion and
region seams. Original GL pitch/roll transforms change vertices while retaining
the existing normals.

`oracle/ui/player_forms.py` adds256 original GL body comparisons, including cached
Cow idle frames, transformed-player poses, nonuniform resize, zero dimensions,
sample order and bounds before/after alignment. The prior1,669 observations are
preserved. Of the1,431 pinned cached BAS definitions,121 have nonzero dimensions
on both axes. This does not establish material, camera or pixel fidelity or
accepted gameplay interactions. Accelerated body lean was ported later
(`actor_turning.rs`, `cargo xtask actors-oracle --check`).

### World-positioned effect graphics

World `SPOTANIM_SPECIFIC`17 and the spatial branch of `SPOTANIM_ENTITY`56 now
create finite effects independently of player/NPC attachments. The original
zone offsets, transformed packet fields,104tile bounds, tile centre, one-time
terrain/height capture, delayed start, strict frame-delay comparison and removal
before drawing are retained. Effects keep scene-local coordinates as the original
class does. They use the existing cached SpotAnimType model/recolour/retexture,
GL animation, lighting and normal renderer on Windows and WASM. Their next frame
is always-1, and their original lifecycle emits no actor sequence-sound events.

`oracle/ui/world_spots.py --check` executes the unchanged original packet branches,
SpotAnim constructor/advance and ScriptRunner scene traversal. Its640 scenarios
produce5,120 snapshots;128 use the eight existing pinned cached effect sequences.
Terrain and draw submissions are recording services. The existing actor-spot GL
pose and cache corpus remains unchanged. A separate offline packet review is
`fixtures/players/world-spots-replay.json`, selectable in the browser preview or
with `--npc-replay` on Windows. It does not exercise authoritative spells/combat.

Actor-targeted opcode 56, projectiles and scene streaming, open at this stage,
have since been ported (`ParticleSystem` is an empty stub in this revision,
so there are no particles to port). Full visual/gameplay parity remains open.
The native adapter rejects
more than1,024 simultaneous effects and malformed/trailing packet bytes with
explicit diagnostics, rather than unbounded allocation or original exceptions.
Missing sequence/model assets remain diagnostics; the review requires the
prepared world pack containing the eight spot-effect dependency closures.

### Cached widget sprite preparation and draws

Ordinary type5 interface art now follows original `Component.getSprite` selection
and preparation. Legacy CS1 state chooses its active sprite, while IF3 uses the
normal sprite. RGB loading and alpha loading remain distinct. Flips update the
trim offset before padding, black/white outlines and the diagonal shadow. Cache
keys retain the original wrapping64bit sum, including overlapping option bits.
Valid empty cached images stay empty. Uploads retain the original power-of-two
backing and repeat wrapping; prepared images are not redirected into an atlas.

The portable type5 GL draw router preserves natural legacy sizing, exact-size
nearest filtering, fixed-point resized bounds, alpha-specific filtering, tiled
power-of-two branches, rotation arguments/half-pixel pivots and inherited versus
explicit tile clipping. A dragged component's alpha overrides its configured
value as in the original. A dedicated sprite shader preserves the original
floating-point alpha fractions of256 instead of quantizing them through byte
vertex colors. Filter changes and eviction finish queued geometry
before changing or dropping the shared texture. The native cache keeps200strong
entries; original soft-reference demotion/GC remains separate.

`oracle/ui/widget_sprites.py --check` reproduces6,042 original preparation, alpha,
key, upload-byte and resize observations over the1,709 prepared cached sprite
sheets and authored boundary inputs. `oracle/ui/widget_draw.py --check` executes
the complete original type5 GL branch under recording services in2,176 scenarios.
Its plugin callback arguments are retained by the router; a concrete native
plugin host remains a separate boundary. Native capture adds
`widget-sprite-state.json` with prepared geometry and expected/actual GPU backing
checksums; normal rendering does not read those texture pixels back.

This does not complete full UI pixel fidelity, every interface/content handler
or live bank/shop transactions. Item sprites were finished later: the count
text is baked into the 36 x 32 sprite and IF3 item sprites go through this
router (`cargo xtask ui-fidelity-oracle --check` covers the count text). Invalid image/padding/backing sizes and excessive
non-power-of-two tile work return explicit native errors. Pixel/backing buffers
are bounded to4million pixels and tile work to131,072 draws per widget; ordinary
cached inputs remain covered by the original comparisons.

### Conditional widget text and legacy controls

Ordinary rectangles and text select the original CS1 active colors and legacy
hover colors. Text also selects the active string, IF3 item name/count formatting,
the shared waiting reference and IF1 numeric/DNS interpolation. The count formatter
retains signed grouping, leading spaces and the original length thresholds.
Ordinary font drawing ignores component opacity and inherits the parent's clip;
font markup retains its existing transparency behavior. English waiting text is
the current runtime default. Last-login DNS data is an explicit caller service;
rendering does not issue DNS requests, and its full wire/profile binding is pending.

The legacy hover walk visits the already laid out hierarchy in order, including
newly selected hidden panes. IF3 hidden widgets remain hidden; a legacy container
can draw through its hover reference, while other legacy types retain their
original hidden-flag behavior. Hover and waiting state hold object identities so
a replacement at the same numeric address does not inherit the old reference.
Continue sends its original132 packet before resolving the target and blocks
duplicate sends while waiting. A loaded null child remains an ordinary null
lookup. IF3 menus can still show Continue during that wait, as in the original.
Legacy Close uses the existing184 modal-close path and resets waiting after close
callbacks. Action-driven layout is refreshed before drawing removed controls.

The selected behavior has2,048 original rectangle/text observations,1,024 original
static hover/hidden/reference scenarios, and256 original waiting/close lifecycles
(8,192 snapshots). The hover corpus excludes IF3 event and noClickThrough services,
created children, subinterfaces, scrollbars and tooltips; its eligibility checks
do not establish a complete input hierarchy or framebuffer comparison. Font,
item metadata, CS1-expression values, DNS results, raster/plugin and zero/one-pane
close iteration are explicit oracle services. Existing CS1 and UI corpora cover
their separate decoded expressions and cached components. Runtime review belongs
to the separate `runs/client-widget-style-validation-v2/` receipt when it closes.

The complete original button-click function has another768 callback/reference/
wire scenarios, including deletion, replacement, nested clicks and failed service
prefixes. A button's component is captured before its hook, then the same object
and property fallback are used after the hook. Callback packets are queued before
the outer button packet. This corrects the stale lookup seen when closing the
cached bank interface. Hook execution, component/property providers, special
client-code handlers and ISAAC-disabled output are explicit oracle services;
unported special handlers still report their existing native boundary.

Native interpolation rejects cyclic/excessive service strings after262,144
replacements or4MiB output. Full glyph sampling/pixels, internationalization,
native plugin callbacks and complete live bank/shop/profile/content behavior
remain pending; item-sprite composition and the keyboard and mouse ports came
later.

Ordinary legacy type7 item text lists, type8 tooltips and type9 lines have a
separate1,536-case original drawing corpus plus14 tooltip-timer observations in
`fixtures/ui/widget-legacy-draw.json`. It executes the complete original drawing
branches, JagString operations and GL thin/thick line geometry. Font readiness,
string width/glyph drawing, item lookup, CS1 values, raster boxes and plugin calls
are recording services; matching those calls and GL vertices does not establish
whole-framebuffer pixel parity. The native renderer uses opaque text/lines,
original list baselines/alignment/count formatting, parent raster clips, the
original0.3f thin-line offset and integer thick-line fan geometry. Native item
lists reject more than1,048,576 slots rather than exhaust memory.

The selected legacy tooltip input path retains the component object and shares
the original50-cycle timer across controls. A replacement at the same numeric
address does not inherit the old selected object. Measuring and drawing perform
separate CS1 interpolation passes. Original delay arithmetic is covered by the
JVM fixture; an actual State test covers switching, replacement, suppression and
decay. Existing selected hierarchy fixtures remain separate from full IF3 event,
plugin and complete input parity. `prepare_ui.py` now declares the
original named `p12_full` font; pinned cache name lookup resolves495 and its
font/sprite bytes are independently compared. An older pack without that asset
declaration reports a missing-font diagnostic.

`runs/client-widget-legacy-validation-v2/` contains four static Windows GPU
reviews and actual WASM pointer checks for delayed tooltips, CS1 text, immediate
switching at the shared delay and leaving controls. The first missing-font pack
attempt is retained separately. Browser JPEGs are not lossless pixel comparisons;
the authored sheet is test content. Live bank/shop transactions, PIN/profile
composition and complete game/client parity remain open.

Ordinary IF1 scrollbars now use the pinned `InterfaceList.handleScrollbar` and
`Cs1ScriptRunner.renderScrollbar` behavior. Wheel input adds45 pixels per detent;
any held mouse button moves arrows by4 per client cycle or positions the thumb.
The shared drag flag and32-pixel margin reset on every eligible component call,
including a missed second scrollbar. Wheel input includes the bottom edge and
is gated by the original component-width region, independently of the hit clip.
Menu/inventory/component drag suppression remains in the existing input phase.
Scroll clamping follows input, and children use the updated scroll origin during
that same legacy traversal. IF3 controls retain their separate script hooks.

Container scrollbars paint after static children, created children and attached
subinterfaces, using the parent raster clip. The UI pack declares the original
named `scrollbar` indexed-arrow sheet. Track/thumb colors, minimum8-pixel thumb,
Java integer wrapping/division and GL axis-line float-operation order are kept.
Zero divisors produce diagnostics after preserving already completed calls.
The renderer presents every frame; redraw notifications are observed by the
oracle rather than introducing a separate dirty-rectangle renderer.

`fixtures/ui/scrollbars.json` records1,024 eight-call original input sequences,
1,024 original paint/failure prefixes and512 actual original GL geometry cases.
Redraw, mouse/wheel, indexed-arrow presentation and the GL backend are explicit
recording services. State tests also cover child-origin refresh, noncontainer
input, clipping boundaries, suppression, IF3 exclusion and subtree paint order.
This does not establish complete interface traversal, IF3 scrollbar scripting,
dirty-rectangle scheduling, native plugin hooks or whole-framebuffer parity.

The frozen client in `runs/client-live-bank-validation/` completed a separate
controlled live Take/Deposit-1/Withdraw-1/Close round-trip against the native bank
server. Windows inputs were protocol-driven; browser inputs used actual pointer
menus through an unmodified transport relay. The server test controller opens
the bank once after real pickup. Natural banker/booth admission, PIN/custom
amount callbacks and profiles remain open. `runs/windows-live-bank-review/`
provides a workspace-dependent launcher for manual Windows review; its short
login/GPU smoke is separate from actual Windows mouse acceptance.

Selected legacy inventory icons now draw in their original subtree position at
half opacity, including the initial press before the five-cycle movement delay.
Per-axis offsets strictly between−5 and5 are suppressed; exact±5 offsets survive.
After drawing, the renderer applies the original independent upper/lower edge
scroll branches using the active parent clip and logical UI cycles accumulated
since the preceding presentation. It compensates the captured pressY so the next
layout's parent movement cancels out of the selected icon's screen position.
Both edges can execute in a short clip, observing the first branch's mutation.
A null parent fails only when an edge condition actually reads its fields.

`fixtures/ui/inventory-scroll.json` executes the complete unchanged original
selected-icon drawing branch in4,096 cases, including integer overflow,
GL/software clip selection, completed draw/redraw prefixes and array/null parent
failures. Sprite presentation, mouse/raster state and redraw are recording
services. The native parent binding covers ordinary static widget trees; this
does not establish arbitrary created-array bounds or the complete drag lifecycle.

The existing512-case original inventory reorder corpus defines helper arguments
as hover slot followed by pressed slot. The actual drag controller now uses that
order on release, correcting a reversed call that default swaps could conceal.
Another test drives226 valid positive-item controller bindings from that same
unchanged corpus and compares actual packets and local widget arrays. Rendering
feedback tests preserve icon position while the parent scrolls and assert no
authoritative container mutations. Original insertion/replacement quirks remain
unchanged. Full IF3 script drag/scroll, callback identity across complete input
lifecycles, live reorder acceptance and complete framebuffer parity remain open.


Ordinary IF1 inventory rendering now traverses the original declared row/column dimensions, preserves Java wrapping slot positions and first20 offset reads, and requests positive item sprites only when their32-pixel slot intersects the clip or is the clicked drag slot. Item counts are read lazily after visibility. The original quantity-text flag and selected outline/shadow arguments reach the GPU adapter, and ordinary icons use full opacity independent of component alpha. Empty-slot background lookups retain their distinct unclipped first20-slot behavior. The complete original type2 body, with recording available sprite services and layer=-1, matches2048 cases/6791 request/draw observations/112 lazy-array failure prefixes. The selected edge-scroll branch keeps its separate4096-case proof. The request helper represents pressed half-opacity; real MiniMenu pressed-component lifetime is still a separate input binding, as are sprite availability/dirty/plugin callbacks, component replacement and full framebuffer parity.


Inventory action feedback now retains the actual component lease separately from its numeric address. The original13 post-packet action assignment prefixes reset the shared counter before lookup, then assign the returned component and slot; failed lookup preserves the old lease/slot after the reset. The original footer requests a redraw at counter0. Each logical50Hz UI tick increments with Java wrapping arithmetic, redraws at>=15, then clears the reference only after redraw succeeds. A released legacy inventory gesture assigns10 after its deferred menu action, including rearrangements without a new menu action. Replacement widgets at the same address do not inherit the old object's fade. The existing type2 request helper now consumes that current-object/slot binding and records bounded data-only successful pressed-icon submissions. Native capture snapshots include state/submissions; offline browser snapshots publish transient feedback after drawing while active and once on expiry.

The unchanged field/footer/tick/release statements match2048 original sequences/37888 observations, including aliases, same-ID distinct objects, nulls, failure prefixes and counter overflow. Additional real State/Controller regressions cover detached identity, failed lookup reset and post-action release ordering. These proofs do not establish the complete MiniMenu handler, all packet/target cleanup/large-count Examine chat behavior, original dirty-rectangle callbacks, full component input lifecycle or whole framebuffer/game parity. Redraws use the client's existing full-frame presentation with bounded request counts; native OS pointer/keyboard behavior still needs manual review.


Inventory Examine now executes the complete original branch through typed packet/chat/component services. It first resolves the actual component, sends92 if absent or its selected count is strictly below100000, otherwise reads the quantity before resolving the item name and appends the exact decimal `quantity x name` type0 chat message (empty sender, phrase-1). It then resets the interaction counter, performs a fresh component lookup and captures that returned object/slot. A later error keeps any completed packet/chat and mutation prefix. Native binding uses the existing normalized nullable chat-text representation and notification clock; it does not rewrite authoritative containers. Main dispatch no longer substitutes a debug status string for this message, and does not stamp feedback a second time. Latest visible chat diagnostics are bounded to one message for Windows/browser review.

The complete unchanged MiniMenu branch, real JagString decimal/concatenation/copy helpers and original Chat class match2048 cases, including threshold boundaries, null/count-array failures, first/late lookup errors, name-service failure, null name normalization, signed metadata and completed prefixes. Real State/VM tests cover packet versus chat, notification/phrase metadata, missing component and failure-before-feedback boundaries. This does not complete the surrounding MiniMenu/target lifecycle, raw nullable Chat storage, cipher/buffer-capacity failures or full UI/game/framebuffer parity.


Legacy button actions8/51/59 now execute their complete original MiniMenu branches through typed component/packet/varp/refresh services.51emits10before lookup and writes its first CS1varp operand only when unequal;59always assigns wrapping1-old for first-opcode5. Both write active state directly without creating a500ms pending-varp timer or transmit notification.8looks up before emission and invokes the original special handler for positive client codes.205assigns logoutOnDisconnectTimer250and approves; other positive codes suppress emission. The same handler now runs after the IF3captured onOptionClick hook and before its enabled/wire phase. The existing actual close-hook test also covers a detached205component, retaining callback184before outer155. Each logical50Hz UI cycle decrements only positive logout timers; disconnect/reconnect/login-screen behavior remains separately incomplete.

The unchanged three branches, special handler and full refreshMagicVarp body match2048cases, with recording lookup/redraw/ambient/varptype services, null/nested-array/bounds failures, overflow and completed packet/varp prefixes.512original timer sequences cover positive/zero/negative counters. Real State/VM tests cover direct writes/no pending timers, ordered ambient snapshots, mode5/6/9binding, emitter/late-definition failure, active-array3500bounds and special-before-permission behavior. Native components use decoded Vec<Vec<i32>>and cannot contain arbitrary Java null nested rows. Current GPU presentation redraws full frames; a bounded counter records redraw requests instead of reproducing the full original active-interface loader/dirty scheduler. Existing ordered ambient snapshots feed the actual audio runtime. Missing varptype files use the existing default-code0archive behavior. Raw cipher/buffer failures, full redraw/plugin services, all MiniMenu/target/input lifecycle and whole UI/game parity remain open.
# Current per-part body animation follow-up

The Windows/WASM body path now applies player/NPC slot tracks using retained
sparse source bits and original GL/BAS arithmetic. See
[client-part-animations.md](client-part-animations.md) for the exact comparison
corpora and remaining boundaries. Older dated checkpoints below retain their
historical pending lists.

