# Client: status, verification and what is still missing

This is the status record of the visual client (`crates/client`, package
`rs09-client`), the audio crate, and the client side of the protocol, net and
cache crates. It says what the client does, area by area; how each part is
checked; where it knowingly differs from the pinned RT4 rev-530 client; and
what is still missing. It is not a claim of a complete client: the
[Still missing](#still-missing) list is long. How to run the client, its
launch options and its controls are in [visual-client.md](visual-client.md).

State as of 2026-11-17.

## Verification scope

The areas below rest on four kinds of evidence, strongest first. Each area
says which.

**1. JVM differentials.** A command compiles original classes unchanged (or
method text copied verbatim from the pinned sources at generation time, with
the source hashes in the fixture's provenance file) beside small adapters
under `oracle/<topic>/`, runs them, and writes a fixture under `fixtures/`; a
Rust test must reproduce every observation. `cargo run -p xtask -- NAME
--check` (JDK 25) regenerates and compares. All of the following reproduced in
this tree on 2026-11-16:

| Command | Rust test | What the JVM ran | Reproduced |
| --- | --- | --- | --- |
| `music-oracle` | `rs09-audio` `tests/music_oracle.rs`, `tests/stream_oracle.rs` | see [Music](#music) | 2,126 observations |
| `preferences-oracle` | unit tests in `preferences_file.rs` and `cheat.rs` | `Preferences` (read, decode of versions 0 to 255, write, `toInt`) and `Cheat.execute`, whose effects land on recording adapters | 1,562 |
| `input-oracle` | `tests/input.rs` | `Keyboard` and `Mouse` driven with real AWT events; whole state after every call | 20,772 keyboard, 4,677 mouse |
| `menus-oracle` | `tests/menus.rs` | `Font` hover-text ("wavy") layout over unkerned fonts, `CursorType.decode`, `java.util.Random` seeds | 1,208 |
| `minimap-oracle` | `tests/minimap_features.rs` | statements copied from `MiniMap`, `Protocol`, `client` and `SceneGraph` (drift, floor jitter, dot choice); `Font` and `MapElementList` (label layout) | 486 |
| `camera-oracle` | `tests/camera_reference.rs` | `Camera`, `MathUtils`; copied `getTileHeight`, the opcode-129 layout, the follow branch, the key loop, viewport bounds, `projectToScreen` | 700 |
| `roof-oracle` | `tests/roofs.rs` | copied `ScriptRunner` and `SceneGraph` roof methods over three real scenes and one invented one; `addLoc` is a transcription inside the oracle | 2,564 |
| `materials-oracle` | `tests/materials.rs`, `tests/underwater.rs` | the two noise volumes and the liquid table; `UnderwaterMaterialRenderer`, `WaterMaterialRenderer`, `MaterialManager` and `GlTile` against a recording GL | 4 tables, 11 traces |
| `picking-oracle` | `tests/picking.rs` | copied `GlModel.render` hover test and bounds, the walk-tile test, the frustum map, `renderTile` order, location keys | 11,102 |
| `ui-fidelity-oracle` | `tests/ui_fidelity.rs` | `InterfaceList` hook walk, layout and drag start over staged trees; number parsing; kerned widths; count-text sprites | 865 |
| `actors-oracle` | `tests/actors_oracle.rs` | `BasType`, `NpcList`, `PathingEntity`, `ProjAnim`, `ColorUtils`; copied crowd-thinning methods | 2,432 |
| `loc-oracle` | `tests/loc_placement.rs` | `LocType`, `LocTypeList`, `GlModel`, `RawModel`: all definitions, GL models by shape and rotation, hill skew, `getMultiLoc` | 5,205 |
| `lighting-oracle` | `tests/lighting_oracle.rs` | `LightingManager`, `LightMesh`, `ShadowManager`; copied `buildGlLighting` | 104 |
| `display-oracle` | `tests/display.rs` | `DisplayMode` over recording adapters | 600 cases, 5,410 steps |
| `occlusion-oracle` | `tests/occlusion.rs` | copied `SceneGraph` occluder marks and construction, active occluders, `isOccluded` and the four visibility queries in frame order, `renderTile` with real occlusion; `Occluder`, `Tile` and the tile entity classes unchanged | 1,424 |
| `title-oracle` | `tests/title_scene.rs` | `Camera`, `MathUtils`, `CollisionMap` unchanged; copied `setupLoadingScreenRegion`, `loadRegion`, `decodeNpcFiles` and the NPC and camera part of `client.mainUpdate`: title region set-up, NPC files, wandering, the title camera | 1,720 |
| `cargo run -p rs09-client --example terrain_oracle -- --check` | `tests/terrain.rs`, `tests/underwater.rs` | `GlTile`, `FluType`, `FloType` and others unchanged, with methods extracted from `SceneGraph`, `GlModel` and `Rasteriser`: floor mesh, tile shade, merged normals; then the underwater scene | 73 and 77 sections |

What these do not observe is the same everywhere: what the real renderer,
canvas, sockets or devices did with the calls, and any Rust code between the
ported function and the screen. Fixtures that hash real cache data (music,
roofs, minimap, actors, locations, terrain, underwater) establish nothing
without `assets/cache`; the terrain tests also need
`runs/player-session/xteas.json`.

Four more tests compare with output of the original classes but have no
regenerating command in this workspace:

- `tests/title.rs`: 576 loading-bar cases from `oracle/title/TitleOracle.java`
  (unchanged `LoadingBar`); the `javac` command is in that file's header.
- `tests/login_flow.rs`: 31 scenarios, 2,068 trace lines of the original
  `LoginManager` loop methods, `CreateManager` and `WorldList` over scripted
  sockets. The harness that wrote `tests/login_flow/java_trace.txt` is not in
  the repository.
- `tests/world_map.rs`: hashes from a probe that ran the unchanged `WorldMap`,
  `MapList`, `WorldMapFont` and their dependencies. The probe is not in the
  repository.
- `crates/cache/tests/store_write_oracle.rs`: the original `Cache` driven by
  `oracle/StoreWriteOracle.java`.

The self-sent packet encoders (`rs09_protocol::game::outgoing`) are tested
against bytes the original `Buffer` wrote, embedded in the tests; that harness
is not in the repository either. The older Python-driven corpora under
`oracle/visual`, `oracle/ui`, `oracle/players`, `oracle/minimap` and
`oracle/audio` are described in [visual-client.md](visual-client.md); they
were not re-run for this record.

**2. Transcription tests.** Expected values worked out by hand from the
pinned sources. They do not run the original classes, so they are weaker
evidence. `crates/client/tests/server_events.rs` pins the SHA-256 of the
reference files transcribed from (`tests/actors_oracle.rs` and
`tests/lighting_oracle.rs` pin their own); a change to one fails the test.

**3. Tests against scripted peers.** Real sockets against a peer the test
scripts (login, reconnect, JS5). The peer's behaviour is transcribed.

**4. Captures and live runs.** Screenshots looked at by whoever ported the
part, and runs of the release client against the Rust `player_session`
example or `rs09-server`, some through a recording proxy. These were single
runs, not automated, and were not repeated for this record. Before the frame
comparison below (5) no frame of the original client had been compared.

Three checks are independent of the transcription:

- An instanced scene that copies every chunk unrotated from its own position
  is compared, on real map files, with what the ordinary region decoder gives
  (`world_cache::tests::an_identity_instance_reproduces_the_normal_scene`).
- Fixed-length messages are tested against `SERVER_PACKET_SIZES`, the
  original `PACKET_LENGTHS` table, which is separate from the read order.
- The cache-backed asset source is compared byte for byte with every entry of
  eight packs that the original Python packers wrote from the same cache.

**5. Frame comparison with the original client** (2026-11-17, one manual
run; the scripts are in `oracle/refcompare/`). The original rev-530 client is
compiled with `javac` alone from the unchanged `reference/client` sources
(`build-original.ps1`; the JOGL and gson jars come out of the reference
repository's own git objects, the one Kotlin interface is replaced by a Java
transcription in `oracle/refcompare/adapter/plugin`). It is started by
`oracle/refcompare/adapter/refcompare/Driver.java`, which calls
`rt4.client.main` unchanged and, from a second thread, runs commands from a
file: input sent straight to the canvas's AWT listeners, and reads and writes
of the client's static fields by reflection (`run-original.ps1`; its cache,
preferences and temporary files stay under `runs/refcompare/`). Both clients
run against the same example server (`npc_session --gameframe`, one instance
each with the same arguments and `--spawn`, so neither sees the other's
player), with the same preference bytes (window mode, brightness, lighting
detail). The Rust client runs with `--pack <missing file>` (the cache-built
scene; at the time the default `assets/visual/demo.pack` dropped the session
when an NPC outside its catalog appeared, which "NPC and other types the
server names" under Actors has since fixed). Each pair is taken at the same camera: the Rust
client writes the pose it draws with when a file `pose` appears in its
`RS09_FRAME_DUMP` folder (`crates/client/src/game/pose_dump.rs`, native only,
inert without the variable and the request), and the driver holds the
original's follow-camera fields (targets, drift, offsets, orbit point,
`Camera.ZOOM`) or, on the title screen, its spline segment and progress with
both speeds 0, or its locked camera with every speed 0, to those values. The
original's own `Camera.render*`, `cameraPitch` and `cameraYaw` are then read
back and logged; they were equal in every in-game and spline pair. The Rust
client is driven with posted window messages; `RS09_POSTED_POINTER` stops it
from replacing the posted pointer with the real one each frame
(`input/platform.rs`), without which hover text, menus and the click cross
never appear in a driven run. `launch.ps1` starts servers and clients and
stops exactly the processes it started; `session.ps1` has `Pair-Now`
(in game), `Pair-Title` (title scene) and `Pair-Ui` (no camera), which write
original | Rust | difference ×4 images and the statistics of the canvas and
the 512×334 viewport; `title-match.ps1` restarts the Rust title until its
first title place is the one the original shows. Compared: the title menu
layer, title scene (pinned), login form empty, typed and failed, the login
sequence, the kitchen at several pitches and yaws, all four gameframe tabs,
typed chat, hover text, object and floor menus, the click cross, walking and
running (unpinned sequences), the castle courtyard, the river by the bridge,
Draynor Village with world NPCs, standard-detail lighting, brightness 1 with
`Camera.ZOOM` 400, and resizable mode at 1024×768. Not compared: underwater
scenes, fog over long distances, health bars and overhead names (the example
servers send none), full screen, the software renderer, sound. The evidence
and the ranked list of differences are outside the repository (the
coordinator's scratchpad `refcompare-evidence/`). To repeat: build the
original once (`build-original.ps1 -Copy <root>`), build
`rs09-client` and the `npc_session` example in release, then dot-source
`session.ps1`, `image.ps1` and `launch.ps1` with `$Copy` and `$Evidence` set,
start servers and clients with `Start-RcServer`, `Start-RcOrig`,
`Login-RcOrig` and `Start-RcRust -Login -Extra @('--pack','none.pack')`, and
call `Pair-Now NAME NOTE`.

A second run (2026-11-17, pairs `v3-*`) checked the fixes for what the first
one found. The driver gained `oget` (array indices and instance fields from
a static field, an object printed with its fields) and `call` (a static
method taking ints, e.g. `LocTypeList.get`), with which the original's
`roofVisibility`, tile walls and location types were read back; its roof
mask was equal to this client's at every tile of four levels around the
player, so roof removal was not the cause of the missing wall tops. They
were faces of texture 147 whose texture coordinate is NaN (see
`texcoord.rs`): 714 location types of the pinned cache have such faces, all
of which were undrawn. After the fix the kitchen pairs' viewport mean
difference fell from 6.5 to 3.6 (default camera) and from 6.5 to 4.0
(pitch max) per channel, and the per-cell darkening of 1 to 14 levels in the
kitchen disappeared (cells now differ by 0 to 1). Character shadows are now
drawn before the scenery, and the title scene's unseen player has none. The
cache-built pack now names the `floorshadows` sprites. Still open: the
character shadow is darker in this client (its innermost ring darkens the
floor to about 0.6 of its value, the original's to 0.77, which is what the
model's alpha of 82/255 with its colour of about 19 gives); the model itself
matches the JVM (`tests/oracle.rs`), and the extra darkness was not traced.

## Start-up and cache download

- The JS5 update client is ported: request queue, connect and back-off rules
  (`rs09_net::js5_client`, `js5_tcp`), master index and group verification
  (`js5_provider`), the store writer (`rs09_cache::disk`), and the 19
  `mainLoad` stages (`cache_update.rs`).
- The desktop client fetches or repairs its cache before loading when a
  server is named (`--update-server HOST:PORT`, else `--connect`) and either
  `--update-cache` is given or the cache directory holds no cache.
  `cargo run --release -p rs09-client --example fetch_cache -- HOST:PORT DIR`
  does the same without the client.
- The login block carries the CRC of each archive index on disk.

Verified by: the store-write differential; transcription tests of the queue,
connect, provider and stage sequence against a scripted socket; the
`fixtures/network` reply vectors through the queue; and runs against
`rs09-server` in which a fresh directory matched the pinned cache group for
group and the unchanged Java `Cache`/`Js5` read it back identically.

What `client.mainLoad` waits for before its login screen (`client.java`
1385-1688): every archive index (state 40); the font, title and fifteen
named sprite groups of archive 8 and the font groups of 13 (50, 60, 80);
archives 2, 16-22, 24, 25 and 27 whole (70, `fetchAll`); 26 whole (90); the
`runes` group of 8 (100); the `huffman` group of 10 (120); 3, 12 and 13 whole
and the `details` group of 23 (130); the world list (135). Everything else
(frames 0, bases 1, sounds 4, maps 5, models 7, the rest of 8, textures 9,
jingles 11, instruments 15, the rest of 23) is prefetched in the background
and read on demand, `Js5.fetchFile`/`isFileReady` answering null until a
group is in; songs (6) and Vorbis samples (14) are only fetched on demand.

The client now does the same (the start gate): the bar goes once the
start-up sequence has finished and `cache_update::start_archives()` are
whole, every prefetched archive except `DEFERRED_AT_START` (0, 1, 4, 5, 7,
11 and 15), which keep arriving behind the title. So beyond the original it
also holds 8, 9, 10 and 23 whole before its title, because some reader of
them cannot answer "not yet" (sprites and textures are read synchronously).
After the gate the update's own store is read live (`Store::live_recording`;
on the desktop `rs09_client::cache_live` stands in for the cache directory),
and a group read before it has arrived is recorded and asked for at urgent
priority on the next cycle, as `Js5.fetchGroup` does. The readers of the
deferred archives wait for it: map streaming builds the scene again until it
read nothing still missing (`world_cache::build_when_present`, the original
`LoginManager` waits for its files the same way), the starting scene built
from the cache likewise (`cache_update_screen::when_present`, the browser's
`cache_host::world_pack`), NPC and spot bodies, changed locations, interface
models and animation frames are asked for again on later cycles, a queued
sound effect stays queued (`ui_audio`), and the music player takes the group
as not there yet. `--update-whole-cache` keeps the old behaviour (wait for
everything) and `--update-original-pace` reads the update connection once a
cycle, as the original does, instead of in between too.

Measured on localhost against `npc_session`, launch to the title (game
state 10) with an empty cache directory: at the original pace 137.2 s
waiting for the whole cache (`--update-whole-cache`) against 18.1 s with the
gate (open at 14.7 s, 10,984 of 68,499 groups verified, title scene built
0.8 s later from groups fetched on demand); with the default fast reading
8.7 s before (the previous client) against 7.1 s (everything arrives in about
4 s on localhost anyway). Logged in at the original pace, the session was in
game in the kitchen with the cook drawn while maps and models were still
arriving. In Chromium (`?update=`, live world, first visit, the previous
build against this one on the same server): the cache bar went at 23.1 s
against 6.6 s (25,980 of 67,337 background groups verified), the title menu
was up at about 25 s against about 16 s and its 3D scene by about 28 s
against about 18 s (screenshots every 2 s); a login then drew the kitchen and
the cook.

Differences: the download draws its own bar in macroquad's default font; of
`mainLoad`'s stages only the world list shows on the original bar. A bad
block marker or an over-long reply is an immediate I/O error, and an
oversize cache file is an error instead of being deleted.

## Title screen and login

- A live client (`--connect`) starts on the original title screen, interface
  744, run by its own scripts: main menu, login form, world select, options,
  and the reply messages. The world list is fetched behind the original
  loading bar first.
- `login_flow.rs` and `world_list.rs` are the login, account-creation and
  world-list state machines; CS2 5600 to 5611 and 6500 to 6507 reach them
  through `Vm::login_host`. `--username` starts the same login for the player.
- Entering the title starts the title song; leaving it fades the music.
- With a cache to stream maps from (`--cache`, or the browser's downloaded
  cache), the component with client code 1403 is the original's 3D title
  scene (`title_scene.rs`, `title_world.rs`): one of the 13 scripted camera
  paths, the regions around it that have an `n{X}_{Y}` group, their NPCs
  wandering as `client.mainUpdate` moves them, rebuilt when the camera leaves
  scene tiles 14 to 89. Game state 28 shows the last frame under the loading
  box, as does the state 25 that follows a login. With prepared packs only,
  the component shows the static title background instead.
- The title's own controls: a press on a component with a drag parent (the
  volume sliders' handles) starts its drag in the interface walk, as
  `startComponentDrag` does, and the title's cycle updates it after the
  hooks (`client.mainUpdate` has no mini menu). Quit (CS2 5411) ends the
  desktop client, as `System.exit(0)` ends the original application; the
  browser, an applet's page, records the settings-page request instead.
  Input that arrives while the title scene is being rebuilt waits for the
  first cycle after the build (`game_cycle::reads_devices`).

Verified by: the login-flow trace and the loading-bar cases (above);
`tests/login_seam.rs` against a scripted peer; and one live run in which a
typed login, world select, a duplicate-login message, a killed server and a
second login all worked. The title scene's rules are a JVM differential
(`title-oracle`); its building, drawing and the interface scripts are not
observed on the JVM and were looked at in live runs only. Every control of
the main menu, the login form, account creation (to its second page), world
select, Graphics Options and Audio Options was clicked in this client and in
the original against the same example server, with frames compared by eye
(2026-11-17); the walk-started drag, Quit and the held input are
transcription tests.

Differences: the title scene keeps whole regions where the original cuts the
104 by 104 tile scene from them, builds on the streaming thread (state 28
lasts a fraction of a second instead of one blocking call; two clicks during
one build count as the later one, where the original's build cycle takes the
first and the next cycle the second), and does not
carry out the rebuild the original starts when a display setting changes. A
live session started with `--cache` streams its maps from the cache in game
as well. `--connect` is treated as the original
treats `config.json`: one address for the world list and every world. A
world-list failure is fatal, as in the original, so a client started while
the server is down stays on the loading bar. The pinned client's world hop
is broken (the request overflows its buffer) and is reproduced as it is.
The SD buttons (main menu and Graphics Options) end on the scripts' "unable
to enter the chosen display mode" message, because mode 0 is carried out as
mode 1 (see Display modes), and the full-screen list offers the desktop's own
size only, where the original lists every screen mode from 800 by 600 up.

## Session lifecycle

- Game states 10 (title), 25 (loading), 30 (in game) and 40 (connection lost)
  with the original's transitions (`lifecycle.rs`): logout, a lost connection
  retried as a type-18 login, reply 15 resuming the session, two attempts and
  then the title.
- `resetGameSessionState`, `processLogout` and `reconnect` are ported
  statement by statement with a table of what each clears and keeps.
- Login facts (staff level, member flags, quick-chat flag, own name, world)
  reach the script VM.
- "Loading - please wait." and "Connection lost" boxes; in state 40 the last
  frame is kept frozen under the box.
- An unknown opcode or a failing packet handler logs out, as the original's
  T1 and T2 reports do.

Verified by: transcription tests of every transition and of the three reset
functions; sockets against a scripted peer (`tests/lifecycle_live.rs`,
`tests/login_seam.rs`); one applicable case of `fixtures/headless-lifecycle`;
and live runs: 10, 25, 30; server killed, 40, then 10; server restarted in
time, 40, 25, 30 through a type-18 login answered with reply 2.

Not verified: reply 15 against a real server (neither server sends it) and a
server LOGOUT in the running client (the example never sends 86).

## Main loop and self-sent packets

- One 20 ms logic clock (`clock.rs`, `NanoTimer.count`: at least 1 and at most
  10 cycles per drawn frame, backlog dropped). A drawn frame runs the cycles
  it owes one after another, each a whole `client.mainLoop` in the original's
  order (`game_cycle.rs`, whose module text lists the order with the Java
  lines; `game.rs` and `game/` are the client's state and the steps): the
  devices and the music player; in game state 25 the rebuild and its
  keep-alives and nothing else; in state 30 the timers, at most 100 server
  packets, the loop's own packets, the world (location changes, players,
  NPCs, overhead timers), a pressed inventory item, the interface walk, the
  hooks, the dragged component, the tile a walk was waiting for, the click
  on the menu the last redraw built, the camera, and the cycle's one write;
  on the title screen `mainUpdate`'s order (walk, NPCs, camera, hooks).
  `GameShell.resetTimer` ends the burst, so the cycle that ends a rebuild is
  its frame's last. Then, once per frame, `mainInputLoop` and the redraw,
  which builds the menu.
- A click acts on the menu a redraw built for the point of the click
  (`MiniMenu.clickProcessingState`): at once if the last redraw built it
  there, otherwise in the first cycle after the redraw that does. "Walk here"
  in the scene and a spell cast on the world remember the click's point; the
  next redraw finds the tile under it and the cycle after that walks or
  casts.
- `client_events.rs` writes what `Protocol.loop` writes by itself, in its
  order: window status (243) at session start, on a mode change and 500 ms
  after a resize; rebuild markers (20, 110); mouse recorder (123); click (75);
  camera (21); focus (22); preferences (98); idle logout (245) after 15,000
  idle cycles; keep-alive (93) after 50 silent cycles; verification id (177).
- Game state 25 runs nothing but the rebuild: `ping(false)` every cycle (a
  keep-alive after 50 silent cycles, with whatever was waiting in the buffer
  in front of it), and in the cycle the scene is built four forced
  keep-alives, seven with an underwater map, each a write of its own; the
  rebuild marker and 110 go out with the write of the `Protocol.loop` that
  runs in that same cycle. Music keeps playing. A cycle's script packets are
  written before its menu action; the release of a pressed inventory item
  comes before both.

Verified by: transcription tests; a test that feeds the JVM's recorded
`Mouse` statics from `fixtures/input` and compares the 75 and 123 frames;
encoder bytes from the original `Buffer`; transcription tests of the cycle's
order, of the click and walk state, of the hundred packets a cycle reads and
of a rebuild's writes through the real session encoder (`game_cycle.rs`,
`client_events.rs`, `tests/session.rs`); and proxy logs of live runs at the
display's rate and with every frame held back 60 ms (`--frame-sleep`): `243
93` first; the rebuild's keep-alives, then `20 110 21`; `75` for a click and
the walk's `215` in a later write, after the redraw between them; `21` about
every 400 ms with an arrow held; `22` on focus change; `93` about once a
second; `245` at 304.6 s of no input (login at 4.5 s) and every 10 s after.

Differences: see "Main loop" under Still missing.

## Input

`crates/client/src/input` is the original `Keyboard` and `Mouse`: the full key
table, 112 held-key flags, the 128-entry typed queue with auto-repeat, the
per-cycle click and pointer latch, and (-1,-1) outside the window. Component
key bindings (F1 to F8 switch the gameframe tabs) run inside the interface
walk. Positions are relative to the canvas; a press on the letterbox is
dropped with its release.

Verified by `input-oracle`. Not JVM-verified, and written down as platform
policy in `input/platform.rs`: the table from window-system keys to AWT key
codes, the characters AWT typed for Enter, Delete, Backspace, Tab and Escape,
and how the pointer leaving the window is detected (five user32 calls on
Windows, a `mouseleave` listener in the browser, not at all elsewhere).
An IF3 click is tested where it was (`Mouse.clickX/clickY`), not where the
pointer is by the cycle that takes it (`ui-fidelity-oracle` includes clicks
away from the pointer). A development aid: with `RS09_POSTED_INPUT` set the
desktop client does not ask Windows where the system cursor is, so a harness
can drive it with posted mouse messages while the cursor is elsewhere.

## Interfaces and the script VM

- The hook walk is a port of `InterfaceList.processComponents`: one pass,
  original request order, empty-clip and hidden skips, the `noClickThrough`
  cancel, drag-start suppression, requests for replaced components dropped.
- `onResize`, `onMinimapUnlock` and the animation reset on open are raised.
- Client codes: 324, 325, 327, 328 (character design), 1337 (scene viewport),
  1338 and 1339 (minimap, compass), 1400 and 1401 (world map), 1403 (title
  background stand-in), 1405 (FPS line), 1406 (hover-text host).
- Item icons use the software rasteriser's brightness (0.7), bake the count
  text into the 36 x 32 sprite, and are cached 100 at a time.
- Rectangle outlines and the menu frame are one pixel.
- Preferences are read and written in the original's binary file (version
  11); `::` commands run locally before they are forwarded.

Verified by `ui-fidelity-oracle` (walk, layout, drag start, kerning, number
parsing, count text) and `preferences-oracle`. The rest is transcription
tests and captures, including: client codes, the item-icon cache, the
outline fix (checked against a model of GL lines, not GL), and `Protocol`'s
stale-request discard.

### Server messages

`rs09_protocol::game::events::ServerEvent` decodes the messages that
`updates::ServerUpdate` and the older single-purpose decoders did not own. No
opcode has two decoders: 192 and 217 stay with the minimap reducer, 16 and 17
and the spatial branch of 56 with the world-spot reducer, 44 with player
options.

| Message | Opcodes | Applied by |
| --- | --- | --- |
| Private, clan and echo chat | 0, 54, 71 | `ui::social`, into the chat log |
| Private and clan quick chat | 81, 141, 247 | `ui::quickchat` for the phrase text, `ui::social` for the line |
| Game message | 70 | `ui::game_message`: the ten request suffixes (`:tradereq:`, `:clan:` and the rest) in the original's test order, with the ignore and Tutorial Island rules |
| Public and forced chat in PLAYER_INFO | 225 | `ui::public_chat`: typed and quick chat, staff and tutorial rules, "Cabbage" for undecodable text |
| Friends, ignores, clan channel | 55, 62, 126, 196, 197 | `ui::social` |
| Grand Exchange offer | 116 | `ui::social` |
| Interface colour, scroll, rotation, body head, target parameter, move, varc string, top-level scripts | 2, 9, 48, 111, 123, 176, 207, 209, 220 | `ui::state` through the delayed-change queue |
| Top-level interface with reset 2 | 145 | `ui::state`; resets the world map |
| Inventory delete, reboot timer, var cache reset and refresh | 85, 89, 128, 191 | `ui::vm`; the reboot timer counts down each cycle; a reset keeps varps that have a client code |
| Walk text, settings string, last login, URL request | 42, 142, 160, 164 | `ui::vm` |
| Logout | 86 | `lifecycle` |
| Self teleport, actor spot effect, NPC animation, reset animations | 13, 56, 102, 131 | client loop |
| Half-tile projectiles, area sound, location animation at a coordinate | 97, 104, 121, 235 | `world_spot`, `sound_queue`, client loop; 97, 104 and 121 also as children of the zone envelope 230 (`zone_events`) |
| Camera look-at, move-to, shake, forced angle, reset | 24, 27, 125, 154, 187 | `camera` |
| Attached location, reflection check, UID | 114, 169, 202 | read and discarded |
| Song and jingle | 4, 208 | `music` on Windows and in the browser; dropped elsewhere |
| Scene rebuild, instanced scene rebuild | 162, 214 | `scene_rebuild`, `world_cache`; 214 needs map streaming |

Points that differ from a naive reading of the original:

- `PACKET_LENGTHS[104]` is zero in the pinned client, so a top-level
  MAP_PROJANIM carries no fields. Only the zone envelope (230) can deliver it.
- In 104 the first actor launches the projectile and the second is tracked;
  in 16 and 121 the single actor is the tracked one.
- LOC_ADD_CHANGE (202) is read and discarded, as the HD renderer does.
- A rebuild for the zone the scene is already centred on is ignored. In HD
  the plane is not compared.
- `:chalreq:` and `:allyreq:` without a colon before the suffix make the
  original throw and log out; here the message is an error after the ignore
  and tutorial tests.
- A URL request is recorded in `Vm::requested_urls`. The client does not open
  server-supplied URLs by itself.
- The last-login address is stored without a reverse-DNS lookup.
- A camera shake on an axis outside 0 to 4 is ignored; the original raises an
  array bounds exception there.
- A quick-chat phrase that cannot be decoded is a diagnostic, not a
  disconnect.

Quick chat messages need the phrase definitions, which only the cache-backed
pack (`--cache`) carries as `qcphrase/ID` and `qccat/ID`; with a prepared UI
pack they fail with a diagnostic. All phrases in the pinned cache decode, and
each has one more text segment than it has dynamic values.

### Script commands

`ui/vm_ops.rs` and its neighbours add the commands below to the base set.
Stack order follows the original.

- Containers: 3313 to 3315, 3330 to 3332.
- Account and client facts: 3323, 3324, 3327, 3333, 3335, 3336, 3337.
- Enums: 3409, 3410, 3411.
- Arithmetic: 4004, 4005, 4006, 4012, 4013, 4018.
- Strings: 4102, 4104, 4105, 4110, 4119, 4120, 4124.
- Definition parameters: 4208, 4300, 4400, 4500.
- Item search: 4210, 4211, 4212, with the original quicksort kept verbatim.
- Friends, ignores, clan and Grand Exchange: 3600 to 3629, 3903 to 3913.
- Requests: 3101, 3107, 3109, 3110, 3201, 3202. A scripted drag (3109) starts
  only for a component that has a drag parent and is laid out. 3107 queues
  its route request before the option packet.
- Quick chat menu and sending: 5050 to 5073. Bug report and private
  message packets: 5002, 5009. 5008 runs a `::` command locally first.
- Character design: 403, 404, 410. They edit the local player's appearance
  (`RawAppearance::design`), and the next player update rebuilds the body.
- Keyboard: 5100 to 5102 (Alt, Ctrl, Shift held), 6600 and 6601 (the
  keyboard-camera preference, which the follow camera reads).
- World map: 5200 to 5220 (`ui/vm_world_map.rs`).
- Display and window: 5300 to 5309, acting on `display.rs` at once.
- Client: 5400, 5401 (the recolour palette), 5405 to 5407, 5411, 5419 to 5427.
- Camera: 5500 to 5506. The spline camera (5502) advances every cycle in a
  live session as well, and the end of a fly-through raises the
  `onMinimapUnlock` hook.
- Account creation and login replies: 5600 to 5611.
- Preferences: 6001 to 6028 and 6101 to 6128.
- Viewport and clock: 6200 to 6205, 6300 to 6304, 6405, 6406.
- World list: 6500 to 6507.

`cargo run -p rs09-client --example script_ops` tallies every opcode in the
cache's scripts: 2,063 scripts, none failing to decode, 420 distinct opcodes.
A text comparison of that tally with the VM's match arms on 2026-11-16 found
an arm for each of the 420. That is not an execution test: no test runs every
cached script.

Known differences:

- 4004 and 4005 use a fixed xorshift generator rather than `Math.random`.
- 4104 formats in UTC; the original uses the JVM's default time zone.
- 5302, 5303 and 5305 list one full-screen mode, the screen's own size, on
  Windows and in the browser, and none elsewhere.
- 6121 answers 1 on the desktop and 0 in the browser.
- Preferences. These take effect: brightness, fog, flickering effects,
  character shadows, scenery shadows (0, 1, 2), ground decorations, the two
  roof settings, high water detail, the three volumes, the cursors switch,
  the keyboard camera, the remembered window mode and safe mode. Anti-aliasing
  applies from the next start. High-detail lighting only turns the light-mesh
  pass off. Stored, written and reported but without effect: high-detail
  textures, idle animations, ground textures (the original reads it only in
  the software renderer), all-levels-visible (the original ignores it in HD),
  particles (`ParticleSystem` is an empty stub in this revision), stereo,
  build area and `hdr`.
- The file is `%APPDATA%\rs09-client\jagex_runescape_preferences.dat`, or
  `--preferences PATH`. An earlier `preferences.json` is read once when no
  binary file exists and never written again. Capture runs neither read nor
  write preferences. The browser keeps the same bytes in the page's local
  storage (`rs09:jagex_runescape_preferences.dat`, base64), read at start
  and written at the same points.
- `::` commands: `fpson`, `fpsoff`, `wm0` to `wm3`, `setparticles`,
  `shiftclick`, `tween`, `noclip`, `clientdrop`, `rebuild` and `errortest`
  act. `rect_debug`, `qa_op_test` and `fps N` are parsed and stored only.
  `gc`, `mm`, `pcachesize`, `cardmem`, `clientjs5drop`, `serverjs5drop`,
  `breakcon`, `replacecanvas`, `reloadplugins` and plugin dispatch have no
  counterpart.

## Menus, hover text, cursors and picking

- World picking follows the original's rules (`picking.rs`): the integer
  camera-space hover test of `GlModel.render`, every model under the pointer
  offered in scene draw order, consecutive equal keys dropped, a location
  offered only where `isLocValid` finds it on the player's level, the walk
  tile from the drawn tile triangles. `scene_order.rs` ports `renderTile` to
  supply the order, since this renderer has none of its own.
- World entries are added only while the pointer is inside the 1337
  component, at that component's place in the draw walk; a hovered
  `noClickThrough` layer or modal sub-interface resets the list.
- The menu opens on right click whenever it has an entry, closes more than
  10 px away, ignores a right click while open, and is trimmed when a
  sub-interface opens or closes, with the original's stale-slot quirk
  (`context_menu.rs`).
- Hover text ("Walk here / 2 more options") with the original's jitter and
  alpha (`ui/action_tooltip.rs`).
- Cursors: definitions, selection and presentation (`cursor.rs`,
  `cursor_view.rs`).
- Dispatch: multiloc children in menu and Examine, the morphed NPC id for
  Examine, ground-item option 2 (packet 109), spell-on-tile from the world
  and the minimap, shift-click as one rule (off by default).

Verified by `picking-oracle` (hover test, walk test, frustum map, draw order,
keys) and `menus-oracle` (hover text layout, `CursorType.decode`). Not
JVM-verified: `drawShapedTile` (transcribed); the menu open state and the
tooltip text and refresh (transcription tests; `MiniMenu` cannot be compiled
alone); menu gating (three transcription tests). `world_pick.rs`, which
mirrors the workbench into the ported structures, and the dispatch edits have
no automated test; one capture with `--pick-probe` exercises the former.

## Camera and viewport

- The follow camera is the original's (`follow_camera.rs`): easing, pitch
  128 to 383, the terrain minimum pitch, distance `ZOOM + pitch * 3`, arrow
  keys at the original's rates, the keyboard-camera preference, the login
  randomisation and the 500-cycle drift, the `mod(yaw, 2047.0)` quirk.
- The projection is the original's frustum (near 50, far 7,168) inside the
  1337 component's rectangle (`projection.rs`, `scene_camera.rs`); overheads
  and tile hints use the integer `projectToScreen`.
- `camera::Script` is the locked camera and its five shake channels; while
  the server or a script holds the lock, or a shake is active, the renderer
  builds its view from the scripted pose.
- A rebuild to another base rebases the destination flag and releases a
  locked or spline camera.

Verified by `camera-oracle`. The GL matrix agrees with `projectToScreen`
within 2.5 px in a unit test; the original's two paths differ by that much
themselves. The camera's wiring in the main loop was checked by captures and
live packet 21 values only.

Differences: with an interface open that has no 1337 component the original
draws no scene; this client falls back to the whole window. The offline
workbench keeps a fixed starting view without the login randomisation or the
camera drift.

## Display modes

`display.rs` follows `DisplayMode.setWindowMode` statement by statement. In
the fixed modes the canvas is 765 x 503, centred across the window at its
top, with black around it; in the resizable mode it is the window. A live
client opens a 1024 x 768 window and opens the title in the mode its
preference file remembers. Full screen exists on Windows only, as a
borderless window at the desktop's size.

Verified by `display-oracle`. Captures cover the fixed and resizable frames
at several sizes; one live run recorded each 243. Not run: a 243 in mode 3
and the 6016 canvas replacement. In a browser the fixed and resizable modes
were looked at, not recorded on the wire (see "The browser build as run").
In the browser, full screen is the canvas element in the browser's own full
screen (the Fullscreen API), again at the screen's size only.

Differences: there is one renderer, so the original's software mode 0 is
carried out, and reported to the server and to scripts, as mode 1. Full
screen never changes the screen's mode, so only the desktop's size is
listed, and `::wm3` (1024 x 768) falls back to the remembered mode on any
other desktop. In the browser, full screen is granted only shortly after a
click or key press, and the answer arrives after `setWindowMode` has
returned: the client takes mode 3 at once (CS2 5300 pushes 1, the size is
saved, packet 243 reports mode 3) and, if the browser refuses or later ends
full screen (Escape, its own controls), leaves for the remembered mode as the
original leaves a full-screen frame that loses the focus
(`display_host::full_screen_lost`). The original's frame could not be taken
away like that.

## Scene

**Terrain.** The HD floor mesh is the original's (`gl_tile.rs`): per-corner
texture layers with vertex alpha, the underlay diagonal choice, overlay blend
fans, groups in key order, blended faces drawn nearer with depth writes off.
Tiles are shaded by the walls and centrepieces on them (`tile_shade.rs`), and
mergeable models share normals at coincident vertices (`normal_merge.rs`).
Verified by the terrain example over four real scenes. The oracle's scenes
place one piece per location; the client's two-piece and turned models reach
the merge through code checked by captures only.

**Locations.** `loc_placement.rs` holds `addLoc`'s placement rules: corner
walls as two pieces, wall decorations offset by the wall's `walloff`, diagonal
pieces, hill skew types 1 to 5, the ground-decoration rule, types hidden in
HD (opcode 82) not drawn. Multiloc scenery follows its varbit or varp every
frame. LOC_ANIM keeps the replaced location's clock under the original's
condition. Verified by `loc-oracle` for definitions, models, skew and
`getMultiLoc`; the per-shape table and `Plan` are transcription tests, since
`SceneGraph.addLoc` is not run.

**Roofs.** Every level is drawn and only the tiles the original's flood fill
marks are hidden (`roof.rs`), in all three modes. Verified by `roof-oracle`.
The renderer side (index filtering per chunk) has a unit test and captures.

**Lighting.** Per-object selection of up to four lights with slot retention
(`light_select.rs`), light meshes on the terrain with the own-level,
no-floor and diagonal-wall rules (`light_mesh.rs`), floor shadows from upper
levels, deferred animated-scenery shadows, the sun direction re-read at each
scene, fog and flicker preferences, instant fades. Verified by
`lighting-oracle` for the selection state machine, the meshes and the floor
shadows. The shader carries a hand copy of the selection rule, checked only
by captures. With the pinned light placement (the default) 2 of 68 Lumbridge
lights get a terrain mesh; `--correct-lights` gives 78 of 78.

**Standard-detail lighting.** With the high detail lighting preference off
(`sd_light.rs`, `scene_detail.rs`) the renderer lights nothing: no sun or
ambient term, no point lights, no light meshes, no specular highlight. Each
packed model vertex carries a lightness baked from its normal and the scene's
sun (`GlModel.packInterleavedVertexData`), the floor takes `buildScene`'s
slope-and-shade lightness, and the palette follows the brightness preference
(0.9 to 0.6) instead of staying at 0.7. A change of the preference (CS2 6012,
or 6001 while it is off) rebuilds the scene: through the map stream in a
session, from the prepared pack in the offline workbench. Verified by
`loc-oracle` (the colours of 5,232 packed models under three sun positions),
by the terrain example (the lightness of every tile of four scenes under two
sun positions, the floor mesh it gives, and the palette at the three other
brightnesses) and by captures; there is no reference screenshot of the
original. The high detail textures preference generates every texture at 64
by 64 (`Js5GlTextureProvider.setLowDetail`); the generator was already
verified at that size for all 680 textures.

**Materials.** Routing by texture; water (4) over the ridged noise volume;
liquid (2); waterfall (5); specular (1), evaluated analytically; unlit (6);
mipmap mode 2. Verified by `materials-oracle` for every byte of the noise
volumes and the liquid table; the formulas are transcription tests; the GLSL
is checked by captures, on desktop GL only. No capture shows the specular
highlight.

**Underwater.** Regions with `um` and `ul` map files (597 in the pinned
cache) get the one-level scene below the surface: its floor, its locations,
the backdrop, material 3, and the translucent surface over it. Verified by
the terrain example's second pass over seven scenes and by `materials-oracle`
traces; the shader, pass order, depth biases and pack path by captures only.

## Actors

- BAS acceleration, accelerated yaw and body lean (`actor_turning.rs`).
- Crowd thinning, NPC top render priority and low detail above 50 players
  (`crowd.rs`).
- Hint arrows: the sprite over a hinted tile and the arrow model at the
  player's feet (`hint_arrow.rs`).
- NPC palette recolours and CS2 5401 (`recolor.rs`).
- Projectiles step and aim as `ProjAnim` does, with the integer pitch and the
  next frame for tweening.
- The shadow's sequence choice and the previous body kept while a new one
  cannot be built.

Verified by `actors-oracle` for BAS decoding and movement, lean, crowd
planning, projectile flights and `rgbToHsl`. Hint arrows have transcription
tests only; the shadow choice and the previous-body fallback have none.

NPC and other types the server names. The original reads every type on
demand (`NpcTypeList.get`, `NpcTypeList.java` 25-38, and its siblings) and
never meets one it does not have: an id without a file keeps the type's
defaults. The client follows one rule (`rs09_client::on_demand::find`): a
held definition; else, with a cache open (`--cache`, the browser's
downloaded cache), the file read from it, the defaults when the archive has
no such file, or "not yet" while its group is arriving; else, prepared packs
only, the defaults when the pack's catalog says it is complete; otherwise
the type is not in hand: the entity stays in its update state, nothing is
drawn for it, one diagnostic per id is said (also on standard error), and
the session goes on. Where it is applied:

- NPC types, with the types a morph may become (`npc_types.rs`): the update
  decoder's size, BAS, turn speed and area sound, the NPC views, the bodies
  (models from the cache too). Before, an NPC outside the prepared catalog
  ended the session ("NPC definition absent from prepared catalog").
- Players shown as an NPC (appearance `npcId`): the type is read the same
  way; one not to be had leaves the position unadjusted (size 1) instead of
  ending the session.
- Spot animations: with a cache open the whole archive 21 is read when the
  cache is attached (as the original holds it after start-up) and their
  models come from the cache; a world spot (SPOTANIM_SPECIFIC) whose type or
  sequence a prepared pack lacks places nothing and is reported once, instead
  of ending the session.
- Locations added by the server (LOC_ADD_CHANGE): a type without a file has
  the defaults; one or a model still arriving is asked for again until the
  change can be applied. Before, a type missing from the cache ended the
  session.
- Sequences: `cancel_on_move` with a sequence not to be had acts as the
  default type (`movetype` -1, `SeqType.java` 39) instead of failing the NPC
  or player update.
- Items already followed the rule (`ObjTypeList` defaults); BAS and kits for
  players come from the cache-backed interface pack or the world pack and a
  failure there was already a diagnostic.

Verified live against `npc_session --world-npcs` with the default
`demo.pack` (evidence in the stream's `npcpack-evidence` folder): with
`--cache` in the kitchen, the courtyard and Draynor the session stays in game
for 45-50 s with NPCs drawn and walking (the courtyard's rats, imp, man and
woman, the Draynor bankers); without a cache in the courtyard it stays in
game, draws the NPCs `demo.pack` has and reports the eight types it lacks
once each. The previous client dropped back to the title within seconds of
the kitchen login. Tests: `on_demand` (the rule and the morph walk against a
cache double), transcription tests citing the Java lines above.

Earlier work (player and NPC bodies, animation, spot effects, overheads,
ground items, sound effects and ambience) is described, with its corpora, in
[visual-client.md](visual-client.md) and
[client-part-animations.md](client-part-animations.md).

## Minimap

The raster, icons, compass, dots, flag and hint markers were there before.
Added: the six session offsets and the 500-cycle drift of rotation and zoom,
friend and clan dots, the seeded floor jitter, re-rastering only on a scene
rebuild or plane change, and text labels. Verified by `minimap-oracle`.
The pinned cache flags none of its 211 labels for the minimap, so the label
path draws nothing with it; `--minimap-all-labels` shows it.

Where its cache data comes from (`minimap/source.rs`): the sprite group IDs
(`minimap/assets`: `mapfunction`, `mapdots`, `mapflag`, `compass`,
`hint_mapmarkers`, `hint_mapedge`, plus `floorshadows`), the map scene
definitions (config group 34) and the floors (groups 1 and 4) are each read
from the first of

1. the cache, whenever the client has one open (`--cache`, the default cache
   a run without a UI pack falls back to, the browser's downloaded cache);
2. the starting-scene pack (the cache-built `world_pack` scene and packs from
   the current `prepare_visual.py` carry them);
3. tables compiled in from the pinned cache (`PINNED_SPRITES`,
   `PINNED_MAPSCENES`, checked against `assets/cache` by a test), for a
   prepared pack whose manifest names the pinned cache but which predates
   those entries: the recorded `demo.pack` and the tour packs.

The run prints which one it used (`Minimap: sprite names from ...`). Before
this, the renderer read only the starting-scene pack, so with the recorded
`demo.pack` (the default whenever it exists, live `--cache` runs included)
the minimap was black and the compass missing. Sprites themselves come from
the starting-scene pack, which holds every sprite in all three cases.
Locations: a streamed region's types (and their morph children) come from
the cache with the scene and are kept for the icon pass; morph children an
older prepared pack lacks are read from the cache when one is open. Without
a cache those children are unknown: their map functions are not drawn and a
diagnostic names them (166 in the recorded `demo.pack`; at the Lumbridge
start the minimap is pixel-identical with and without them).

Drawing: the map, its icons and the compass go through one material whose
`MaskTexture` macroquad binds when the batch is drawn, not per queued call.
Until 2026-11-17 the compass's 33 x 33 mask, set last, therefore cut the map
too (a 132-pixel circle in the map's top-left, with four-pixel steps).
`minimap_render::Renderer::quad` now draws what is queued before it binds a
different mask. Compared with the original in the same pose (the original's
camera, `MiniMap.compassAngleOffset` and `zoomOffset` pinned to the Rust
client's, and the original's floor and map-scene colour jitters handed to the
Rust client): walls, floors, map scenes, orientation, zoom and the compass
match to within a pixel; map-function icons differ in place because both
clients walk each one up to ten random steps (`Math.random`), and their
colours by the unmatched `Sprites.load` jitter.

## World map

Interface 755 opens: `world_map/` is the map itself (lists, pan and zoom,
labels, the software renderer), `ui/world_map_host.rs` and
`world_map_render.rs` wire components 1400 and 1401, CS2 5200 to 5220, the
per-cycle easing, the drag, the staff Ctrl-click teleport, the load's ten
forced keep-alives, and the reset and rebuild on close.

Verified by: the core's JVM comparison (pixels, layers, labels, state after
each operation; see `tests/world_map.rs`); transcription tests of the
commands and the host; one integration test that opens the real interface by
packet and clicks its buttons. Not exercised in the running client: dragging,
the teleport, and a live session (no local server opens interface 755).

Differences: label glyphs come from a system TrueType face (bold Arial on
Windows, or `--world-map-font`), not AWT's, so they differ from the original
and by machine; with no face, no labels. In the browser the face is the
browser's own text, drawn on a 2D canvas in `bold Npx Helvetica` (the page's
sans-serif behind it; `world_map::Face`, `web/visual/display_glue.js`),
thresholded at half coverage like the TrueType glyphs; the canvas reports no
leading, so the line height there is ascent plus descent. The browser map
reads archive 23 and the rest from the downloaded cache (with the prepared
packs there is no map). The palette is fixed at 0.7. The scene is not
cleared while the map is open; the rebuild on close still runs.

## Music

`crates/audio` holds the original music path:

| Module | Original classes |
| --- | --- |
| `midi` | `Song`, `MidiDecoder` |
| `vorbis` | `VorbisSound`, `VorbisCodebook`, `VorbisFloor`, `VorbisResidue`, `VorbisMapping` |
| `sound_stream` | `SoundPcmStream`, `PcmSound`, `PcmResampler` |
| `music` | `MidiInstrument`, `SoundBank`, `MidiNote`, `MidiNoteStream`, `MidiPcmStream`, `MidiPlayer`, `MusicPlayer`, and the block reader of `AudioChannel` |
| `limiter` | the voice limiter in `AudioChannel.read` |

`cargo xtask music-oracle` compiles those classes unchanged (with empty
adapters for `SignLink`, `AudioThread` and the two device channel classes),
runs them over the pinned cache and writes `fixtures/music`. The fixture holds
2,126 observations:

- all 1,058 songs and jingles expanded to MIDI;
- all 266 instrument slots (148 present);
- all 479 sounds the instruments use, synthesized and Vorbis;
- 83 playback scripts run through `MidiPlayer`, `MidiPcmStream` and
  `AudioChannel.read`, hashed every 50 client cycles: songs, song openings,
  mono output, jingles, a fade into another song, a fade to silence, volume
  changes, stop and restart, and a jingle over a song;
- 240 runs of the `AudioChannel` voice limiter over seeded sets of up to 95
  voices, under a mixer and under the two-level shape of the music tree. Real
  songs seldom reach the 32-channel limit, so the playback scripts alone would
  not show what the limiter does; 149 of these cases turn voices away.

`crates/audio/tests/music_oracle.rs` reproduces every observation, including
each playback hash, so the mixed samples are identical to the original's. The
test needs the cache; without it, it reports that it was skipped. It does not
cover device output or real-time pacing.

The same command writes `fixtures/music/stream.txt`: 4,000 seeded scenarios
and 600 resampler cases through the unchanged `SoundPcmStream`, `PcmSound`
and `PcmResampler`, covering what real songs may not reach (backward and
ping-pong play, mono output, every fade and rate change, and inputs on which
the original throws). `crates/audio/tests/stream_oracle.rs` replays it without
the cache and compares the state and output buffer after every step.

While porting, each piece was also compared with the JVM on larger inputs
that are not kept as fixtures: the MIDI decoder's full event trace for every
song and jingle plus 40,000 generated files, every Vorbis sound decoded whole
and in budget slices, and 60,000 stream scenarios.

In the client (`music.rs`; played on Windows and in the browser):

- MIDI_SONG, MIDI_JINGLE and script commands 3201 and 3202 go to the player;
  the fade, the loading step and the resume after a jingle run once per
  20 ms client cycle.
- The title song starts when the title screen opens; leaving the title only
  fades it (`Music::fade_out`), as `client.setGameState` does.
- Loading is budgeted as in the original, so a song starts after about a
  second of cycles rather than in one long frame.
- The audio thread mixes the music into each 256-frame block. Music and sound
  effects are each clipped as their own device line would clip them, then
  added (`rs09_audio::pcm::add_music`, on both devices). The PCM capture still
  records sound effects only.
- On Windows the WinMM thread reads the sequencer itself. In the browser the
  audio thread is a separate module with its own memory, so the client reads
  the blocks once per client cycle, keeps 16 of them (186 ms) ahead of
  playback and hands them to the worklet, which queues them and adds one to
  each effect block (`pcm/feed.rs`, `pcm/browser.rs`, the `rs09-audio-worklet`
  crate). A frame longer than the lead is heard as a gap; a hidden tab, whose
  frames stop, goes silent and resumes. The worklet counts both.
- In the browser the songs come from the downloaded cache (see "The cache in
  the browser"): songs and Vorbis samples are requested from the update
  server when a song first needs them, as the original fetches archives 6
  and 14.
- SOUND_SONGEND (137) is sent when the area's song has played through.
- `--no-music` turns it off; capture runs are always silent. The M key mutes
  music with the effects.
- `cargo run --release -p rs09-client --example play_song -- 76` plays a song
  on the default device, or renders it with `--wav PATH`.

Differences and limits:

- The original closes the music line when a jingle starts, to cut the buffered
  audio; here a jingle starts after the blocks already queued (about 50 ms).
- The device-side timing of `AudioChannel` (buffer sizing, skipping when the
  device falls behind) is not ported; WinMM buffers are refilled as they
  empty.
- The sound-effect mixer runs the same limiter (`limiter::admit`). Its voices
  use the older integer-rate `pcm::Voice`, whose limiter inputs are tested
  equal to the full stream port's; every play counts as its own sound, as in
  the original, where each play builds a new `PcmSound`.
- No music, location ambience or NPC ambience on desktop systems other than
  Windows. The player and its cache source build everywhere; what is missing
  there is a PCM output device in this client (Windows has WinMM, the browser
  the worklet), and none was written untested.
- Nobody has listened to the music; the evidence is the sample comparison
  and, in the browser, non-silent worklet output.

## Assets from the cache

`cache_pack::cache_pack` presents the whole JS5 cache as one lazily resolved
pack. Every interface, script, definition, model, animation, sprite, texture
and sound is listed under the name the prepared packs use and is read the
first time it is asked for. Loaded assets stay in memory for the run.

- `--cache DIR` makes the client take its interface, model, animation and
  sound assets from the cache. It then needs no `ui*.pack` or
  `animations.pack`.
- When neither `--cache` nor `--ui-pack` is given and the default UI pack is
  missing, `assets/cache` is used if it opens.
- `cache_pack::ui_pack` writes the same file `tools/prepare_ui.py` does:
  `cargo run --release -p rs09-client --example build_packs -- ui`.

The recorded `assets/visual/ui.pack` predates three additions to that script
(parameter definitions, the `p12_full` font and the scrollbar sprite). The
native pack contains all of its entries, identical and in the same order, plus
those three and the title screen's asset ids.

### The starting scene

`world_pack::world_pack` is the native replacement for
`tools/prepare_visual.py`: it writes the starting-scene pack (terrain,
locations and their definitions, models, animations, the default player,
preview NPCs, floors, textures, sprites and `scene.json`).

- When the `--pack` file does not exist, the desktop client builds the scene
  from the cache at start (`--cache DIR`, else `assets/cache`). `--start X,Y`
  chooses the tile.
- Location files are encrypted per region. Keys are read from `--world-keys`
  (default `runs/player-session/xteas.json`); a region without a key keeps its
  terrain and has no locations, with a note on standard error, until the
  server's first rebuild streams the real scene.
- A scene built this way turns map streaming on from the same cache, as
  `--world-cache` would, because no prepared pack covers anywhere else. The
  key file may then be missing altogether: the server's rebuild messages carry
  the keys of the regions they name. Started without keys, Lumbridge shows the
  castle region (which is not encrypted) and bare terrain in the other three.
- `cargo run --release -p rs09-client --example build_packs -- world OUT.pack
  --keys xteas.json [--doors door_configs.json] [--spawns npc_spawns.json]`
  writes the pack to a file. The three JSON files are the pinned server's.

`crates/client/tests/world_pack.rs` builds the pack with the pinned server's
three files (taken from the reference checkout with `git show`) and compares
it with the recorded `assets/visual/demo.pack`: every recorded entry is
present and byte-identical, in the same order apart from re-written movement
sets, and `scene.json` describes the same scene. The native pack adds what
came after that pack was recorded: every raw NPC definition, map scenes, the
minimap sprite ids (among them `floorshadows`), morph children, placement
rules, the world map's label lists and the underwater map files.

The recorded `demo.pack` has none of those later entries. A client started
from it (the default when the file exists) therefore draws no floor shadows
and no underwater scene (the water keeps a plain stand-in backdrop), and
draws multiloc children only where the cache can supply their models. Its
minimap takes the missing sprite IDs and map scenes from the cache or the
pinned tables (see Minimap).
Starting with `--pack` naming a file that does not exist uses the cache-built
scene instead.

At run time the door and spawn files are not read, so door replacement
definitions arrive with streaming and the offline preview has no NPCs unless a
pack built with `--spawns` is supplied.

Map files are not in the lazy cache pack; `--world-cache` streams them.

## Instanced scenes

REBUILD_REGION (214) gives a 4 x 13 x 13 table of source chunks, each with its
own level and rotation. `instance.rs` composes the 104 x 104 scene from the
source map files: terrain (`readDynamicTerrain`, `readTile`,
`clearTerrainRegion`), locations with the rotation of their footprint
(`readDynamicLocs`), and each chunk's atmosphere from its source region.
`world_cache` builds the scene from that when map streaming is on
(`--world-cache`, or a scene started from the cache); without streaming the
message is an error.

- Ground with no stored height takes its noise at the scene tile, not the
  source tile. The original does the same (`readTile(0, 0, ...)`), so an
  instanced copy of an area is not height-identical to the area itself.
- A location whose rotated corner lands on the scene's outer ring is dropped,
  as in the original.
- Point lights follow their chunks as `SceneGraph.readDynamicTerrain` places
  them (`instance_lights.rs`): only lights standing in the copied chunk on its
  source level, turned with the chunk about its 1024-unit extent, keeping
  their source level and unturned span profile. They are stored on the scene's
  first region in the original's insertion order. Unit tests only.
- NPC models are preloaded for the source regions' recorded spawns only.
- An instanced scene has no underwater scene and no occlusion heights; its
  water keeps the stand-in backdrop.
- This was not exercised against a server sending 214. The composition has
  unit tests and the identity check above; the streaming switch has none.

## The cache in the browser

The original applet downloads its cache from the game server over JS5 and
keeps it on disk. The browser build does the same when the start page names
an update server (`ws://host:port/`; the Rust server speaks JS5 on its
WebSocket listener exactly as on TCP). Without one it uses the prepared packs
as before.

- `rs09_net::js5_stream` carries the update connection over a transport that
  delivers whole messages and cannot wait (a `WebSocket`). The ported
  `Js5NetQueue`, connect logic and resource providers run unchanged over it;
  the one difference is that the handshake reply is read when it arrives
  instead of being waited for.
- `web_cache.rs` runs the ported start-up sequence (`cache_update::Updater`)
  into a `DiskStore` held in memory, behind the original loading bar.
- The store is mirrored to IndexedDB in the pieces that changed (the data
  file in 1 MiB pieces, each index file whole), every two seconds while
  loading and every five afterwards. The next visit restores it and the usual
  verification runs: a complete store costs one request, the master index.
  A page closed mid-download, or damaged storage, is repaired group by group.
- `cache_pack`, `world_pack`, the music source and map streaming read that
  store through `rs09_cache::store::Store::live`. Map streaming has no worker
  thread there: since 2026-11-17 a scene is prepared a slice of about 12 ms
  per frame (`rs09_client::slice`; the build is the desktop's own code with
  yield points in its loops, run to the end at once on the desktop's worker
  thread). `?slice=MS` sets the slice, `?slice=0` builds in one frame.
- The login block carries the index CRCs the update server sent.

What differs from the original:

- The cache is in memory, about 92 MB when complete, as well as in IndexedDB.
- The game starts once the start-up sequence has run and the start archives
  are whole (see "Start-up and cache download"); maps, models, animations,
  sound effects, jingles and instruments arrive behind it, and the starting
  scene, the title scene and later scenes wait for the groups they need,
  which are asked for as they are met. Until 2026-11-17 it started only
  once every prefetched archive had arrived (about 35 s on localhost).
- Offline the starting scene has locations only for regions whose keys the
  page serves as `assets/visual/xteas.json`.

`web/visual/cache_storage.js` and the additions to `game_transport.js` are
the only JavaScript: sockets and key/value storage.

What is checked, and how:

- Native tests against the scripted update server
  (`rs09_net::js5_client::scripted`), none of them a JVM differential:
  `js5_stream` (messages cut at any size give the same stream; the
  `fixtures/network` reply vectors reassemble over it at four message sizes;
  the handshake completes without waiting; the original response codes), and
  `web_cache` (an empty store fills and storage holds the same files; a
  reload downloads nothing; a page closed mid-download resumes and a damaged
  piece is repaired; a song is fetched on demand and kept; slow, absent and
  failing storage).
- One run in Chromium (driven by Playwright, 2026-11-16) against
  `rs09-server --cache assets/cache --websocket`: the first visit received
  63.9 MB in 67,421 replies on one connection in about half a minute (the
  page can answer at most the original's 20 background requests per drawn
  frame) and drew the Lumbridge scene and the gameframe from it; IndexedDB
  then held the 83.9 MB data file in 81 pieces and 29 index files. A reload
  received 240 bytes (the master index) and was in the scene about three
  seconds after launch. With `scape main` named on the start page, 41 groups
  were fetched on demand and the worklet reported 86 music blocks a second
  with no gaps and non-silent output; nobody listened to it. Logged in to the
  `player_session` example, the scene was streamed from the same store with
  the server's keys.
- Not checked: other browsers, a slow or failing network in a real browser,
  storage quota being refused, and the sound itself by ear.

## The browser build as run

A second run in Chromium (Chrome 157 driven by Playwright, WebGL 1 through
ANGLE on Direct3D 11, 2026-11-17) went through the client as it stands after
the work this document describes, against `rs09-server --cache --websocket`
for the cache and the `movement_session --gameframe` and `player_session`
examples for a world. Nothing was heard and no other browser was used.

What the page does:

- The canvas element fills the page and is the original applet's container:
  `GameShell.startApplet` and `DisplayMode.setWindowMode` measure the applet,
  put the 765 x 503 canvas at `(width - 765) / 2` in the fixed modes and paint
  the rest black. So a browser client with no saved preferences opens the
  title on the fixed canvas, centred at the top of the page, and `::wm2` or
  the options make the canvas the page. Resizing the browser window is the
  container changing size.
- The WebGL context is made without an alpha channel
  (`graphics_lifecycle.js`). Blended draws leave destination alpha below one
  (18,716 pixels of one in-game frame, read back before the change), and a
  canvas with alpha is composited over the page with it. No pass reads
  destination alpha. A frame copied for game states 25, 28 and 40 is
  therefore an RGB texture there (`screen_frame.rs`); WebGL refuses to copy
  an opaque buffer into an RGBA one.
- The page reports the quote, slash and back-quote keys itself
  (`game_transport.js`): miniquad's page script sends numbers for the first
  two that its Rust side does not know, and for the third the quote key's.
- The start page refuses to launch, with a message, where WebGL offers fewer
  than three texture units in the vertex shader or textures narrower than
  4096 pixels; the world shader needs both and WebGL 1 guarantees neither.
- The status badge over the canvas corner shows for four seconds after the
  status changes, and the client clears it when there is no connection.
- `?cursor=ID` is the desktop's `--script-cursor`.

What was seen, with the cache downloaded (63.9 MB in 23 s on the first
visit, 240 bytes and about 3.5 s on a reload) and with the prepared packs:

- The loading bar; the title screen with its 3D scene (cache) or the static
  background (packs); the login form opened by Enter and by a click, filled
  in from the keyboard; game state 25's box over the frozen title frame; the
  scene with the fixed gameframe (548), minimap, chat and tabs.
- In the scene: blended floors, walls and wall decorations, the roof of the
  room the player stands in removed, actors with names and shadows offline,
  the river with the underwater pass (the water's mean colour within about
  one level in 255 of a desktop capture from the same tile). No shader
  failed to compile (the world shader with the material tables and the mip
  atlas, the underwater pass, the light meshes, widget sprites, the minimap
  mask) and the console stayed free of WebGL warnings. Whether a light mesh
  was among what was drawn was not checked.
- Typing in the login form and in chat, `?` and `"` included; the chat line
  came back from the server as typed. Hover text, the right-click menu at
  the pointer, a walk click with its cross and packet 215, tab and button
  clicks (packet 155), the cursor sprite at the pointer with the system
  cursor hidden (`?cursor=1`).
- The fixed canvas in pages of 700 x 420, 900 x 620, 1100 x 700 and
  1300 x 800, clicks landing where drawn; the resizable frame (746) after
  `::wm2`; `::wm3` falling back, since a browser lists no screen mode.
- At a device pixel ratio of 2 the interface was clipped to a quarter of the
  canvas until the scissor and the interface models' viewport were given in
  surface pixels (`display_host::surface_rect`). After that the title, the
  gameframe and the menu were right at 2 (packs) and at 1.25 (cache, both
  window modes). The ratios were emulated, not a real display's.
- A connection that stops answering: game state 40's box over the last game
  frame for 75 s, then the title, with the title song sounding again 1.3 s
  later. A refused reconnect returns to the title at once; a reconnect that
  succeeds returns to the game.
- Music: the audio context running at 22050 Hz, 83 to 89 blocks a second,
  non-zero samples and rising output energy at the title and for a song
  named on the start page. Blocks are missed while the page is busy: 45 to
  127 at start-up and 260 to 535 around the scene build at login (2.5 to
  3.3 s in one frame, while the title song fades), none otherwise.

Not seen or not working there:

- A logout by the server (the examples do not answer the logout button),
  account creation, a hidden tab, a slow network, refused storage, other
  browsers and real high-DPI hardware.
- At the title, a click on Graphics Options or Audio Options was reported to
  change nothing, after which the menu no longer answered the mouse. Rerun on
  2026-11-17 with Playwright clicks (the cache from the update server): both
  open; in 60 timed clicks on the earlier build 3 were lost, the cause being
  clicks taken while the title scene was rebuilt; on the fixed build 0 of 40.
  The lasting deafness was not reproduced. On the desktop the same report
  came from clicks posted while the system cursor was elsewhere, which took
  the pointer away after each event; such clicks now count where they were,
  as in the original.
- With the prepared packs the minimap was black and there was no compass,
  on the desktop as well; fixed (see Minimap): the recorded packs now get
  the minimap's sprite IDs and map scenes from the pinned tables.

A third run (Chromium via Playwright, 2026-11-17, release wasm, the cache
from `player_session`'s update listener, `player_session` and
`movement_session --gameframe` for a world; evidence in the coordinator's
scratchpad `browser2-evidence/`) covered four gaps closed that day:

- Scene builds. Before: the login's build held one frame for 5.7 to 6.5 s,
  the game socket was silent for 5.7 to 8.2 s and the worklet found its
  queue empty 671 to 830 times in 25 s (three logins each, preferences
  cleared, the cache already downloaded). After (`world_cache::Building`,
  `rs09_client::slice`): the same build took 3.7 to 4.2 s of work over 258
  to 290 frames, the longest frame during it was 159 to 197 ms, the socket
  was never silent for more than 1.07 s and 5 to 17 music blocks were
  missed. The login scene's digest (every chunk's mesh fingerprint, the
  counts, the shadow tiles, the slots) was the same, 0cc444baed3af769, in
  every run, sliced and with `?slice=0`. Still one frame of 2.9 to 3.2 s at
  start-up (the starting scene and the texture generation, before the
  title) and one of 0.35 to 0.42 s before the title scene, neither a scene
  build.
- Preferences. Music volume, brightness and the high-detail resizable mode
  chosen in the title's options were in local storage at once and were in
  force after a reload (title opened resizable, sliders where they were
  left).
- Full screen. The Graphics Options list `1100 x 700` (the emulated
  screen). This Chromium refuses every `requestFullscreen`, even one made
  inside a `pointerup` handler, so the real grant was not seen; the refusal
  path was: mode 3 for a moment, the script's `keep this setting?` box, and
  the remembered mode again. With the API stood in for by the page (a
  1024 x 768 screen and a grant), `::wm3` asked for full screen with the
  key press's activation still active, packet 243 carried mode 3 at
  1024 x 768, and when the stand-in browser ended full screen the client went
  back to mode 2 at 1100 x 700 (243 again).
- World map. Interface 755 opened offline from a one-packet replay
  (`91 73 02 80 80 00` as `assets/visual/ui-gameframe.bin`): loading,
  then the map with its labels in the browser's own font, dragging, the
  overview, the zoom menu (37 %), all drawn as on the desktop. Not opened
  from the orb (no example server opens it).

## Still missing

Each item is marked:

- **(a)** behaviour a player would notice;
- **(b)** a fidelity detail;
- **(c)** dormant with the pinned cache or server, with the evidence;
- **(d)** needs the project owner's decision (the options are collected under
  [Decisions for the owner](#decisions-for-the-owner)).

Evidence quoted for (c) comes from the cache sweeps of the stream that ported
the part; the sweeps were not repeated for this record.

### Depth order and occlusion

The scene is drawn on the original's depth layers (`depth_layers.rs`), and
the original's occluders (`occlusion.rs`, JVM-verified) decide which map
locations are left undrawn and unpicked. Camera-side wall and decoration
"culling" in the original decides when a piece is drawn, not whether; the one
real cull, a diagonal decoration on the camera's own tile, is reproduced.

- (b) Reproduced by depth bias and pass order, not by per-tile draw order:
  ties on one layer go to chunk order, not to the original's later draw;
  character shadows and hint arrows are drawn after every floor and light
  mesh and before all scenery (`game/redraw.rs`, `deferred_scenery`), so
  scenery covers them wherever it is in front, as in the original, but a
  shadow does not cover scenery the original's tile walk draws before the
  actor within the shadow's 150-unit bias (the same tile's earlier
  scenery); a translucent face does not hide things the original draws
  later behind it; translucent faces of actors and animated locations are
  drawn in the dynamic pass, not interleaved with the map's.
- (b)(d) The level used for a layer is the level a tile is drawn on, not
  `Tile.level`; the 201 and 200.5 floor layers are not told apart; the shadow
  map is a shader multiply, not a pass on layer 101.5.
- (b)(d) Actors, item piles, projectiles and spot animations are
  occlusion-culled for picking only, not for drawing.
- (b) Occluders are built from the location map at the first frame after a
  scene build and not rebuilt when a location changes.
- (b) `scene_cull.rs` and the picking changes that use it have no automated
  test; level-1 flat decoration and the diagonal-decoration cull have no
  capture.
### Start-up, cache and login

- (b)(d) The client opens its title once the start-up sequence has run and
  the start archives are whole (see "Start-up and cache download"); beyond
  the original it also waits for archives 8, 9, 10 and 23 whole (about 2,400
  groups), because sprites and textures are read with no "not yet" answer.
  Interface models, animation frames and item icons met before their group
  arrives are drawn missing for those frames (each retries every cycle); the
  original draws nothing for them either until `isFileReady`.
- (a) Account creation was never clicked through, and the Rust server has no
  create opcodes (147, 186 and 36 disconnect; the client then reports reply
  -1, as the original does at end of stream).
- (a)(d) A world-list failure at start is fatal, as in the original. A failed
  launch-file login (`--username`) is reported on standard error only; the
  message appears once the form is opened.
- (b) Of `mainLoad`'s progress stages only the world list is shown on the
  original bar.
- (b) The loading box's percentage line is never shown: nothing counts missing
  map files or models. An in-game rebuild (state 30 to 25) keeps drawing the
  old scene under the box; the original freezes the frame, as this client does
  only for state 28 and the 25 that follows a login.
- (b)(d) The title scene is not cropped to 104 by 104 tiles, title NPCs are
  drawn in index order rather than placement order, and the settings-change
  rebuild (state 10 to 28) is not carried out. The start-up bar stops at the
  world-list stage; the original reaches 100% before state 28.
- (b) After a title login the client sends opcode 137; whether the original
  does was not checked.
- (b) Only English text: the status strings, chat prefixes and social
  strings of `LocalizedText`'s other three languages are not ported. (c) The
  pinned launcher is English.
- (b) Not done, by design so far: the reflection check reply (114 / 163), the
  UID file (169), reverse DNS for the last-login address (164), keeping and
  echoing the settings string (142), opening server-supplied URLs (42), and
  posting error reports to `clienterror.ws`. (c) The pinned server never
  sends 114.
- (c) The software renderer's flames (client code 1402). The title scripts
  hide them under the hardware renderer.
- (c) World hop is the pinned client's broken one. `SELECT_DEFAULT_WORLD` is
  true, so the pinned client never reaches it, and neither server reads the
  hop request (210).
- (c) Reply 15 (resume after reconnect) has only met scripted peers: both
  servers answer a type-18 login with 2.

### Session lifecycle

- (b) Upper levels work with and without map streaming: a rebuild onto
  level 1 to 3 (login onto it), a level change by PLAYER_INFO or
  TELEPORT_LOCAL_PLAYER inside the scene, drawing with roofs removed per
  level, picking the level's objects, and walking with its collision. Seen
  live in Lumbridge castle on levels 1, 2 and 3 (`movement_session --spawn
  X,Y,PLANE`, streamed and prepared-pack scenes). No example server moves a
  player between levels, so the climb itself was checked by injecting the
  server's self-teleport (the native development aid `RS09_FRAME_DUMP/inject`,
  `game/inject.rs`); the click on "Climb-up" is sent but nothing answers it.
  The minimap was black in those live runs on every level, level 0 included.
- (b) The workbench's status box (a 440 x 32 rectangle at the top left
  holding `route_status`) is drawn in live frames only with
  `--workbench-keys`; a frame kept from such a run shows it behind the
  connection-lost box.
- (b) Without `--cache`, a UI pack lacking `p12_full` gives the status box
  plain text instead of the original font.
- (b) Sub-interfaces in one hash bucket close in key order; the original
  closes them in insertion order.
- (b) On a session reset, pending scenery changes are reverted through the
  ordinary path rather than by rebuilding from map files.
- (b) A connection lost in state 25 goes to state 40 at once; the original
  notices at the next `Protocol.loop`.
- (b) A handler error logs out, as the original's does, but here that also
  covers limits of this port: more than 1,024 scene spot animations or
  projectiles (the original has no cap) and an instanced rebuild without
  streaming. Server varp ids are not bounded to
  the original's 3,500 entries.

### Main loop

- (b) The login state machine (`LoginManager.loop` in game states 10 and 40)
  runs on a 20 ms count of its own at the top of the frame, before the
  frame's cycles, not after each cycle's `mainUpdate`. The rebuild packet a
  login reads itself is handled in the next cycle.
- (b) The menu's world entries are picked against the meshes of the previous
  drawn frame with this frame's camera; the original picks while it draws.
- (b) A press that starts an inventory drag is judged by the entry the click
  would run, and its release runs that entry; the original reads the menu as
  it stands at each of the two moments. A component drag starts with the
  menu click, not in the interface walk, so none starts on the title screen.
- (b) The read time-out (750 cycles without a packet) is counted at the
  cycle's write, not before the players move.
- (b) In a capture run (`--capture`, frame-driven) the world's cycle number
  stays one behind the interface's and the first frame moves nothing, as
  before the restructure, so that captures compare with earlier ones.
- (b) While the title scene is built (state 28) the interfaces stop, as in
  the original, but only once the title interface's load scripts have run a
  cycle to place the camera.
- (b) A drawn frame longer than 200 ms loses its backlog, as `NanoTimer`
  does, and writes nothing extra (the keep-alive that used to follow such a
  frame is gone; a rebuild now writes the original's own).
- (c) A connection lost in state 25 is noticed at once; the original notices
  at the next `Protocol.loop`.
- (b) The cycles run inside drawn frames, not on a thread of their own as
  `GameShell.run` does. On Windows frames do not stop: miniquad drives
  frames from a 10 ms timer inside the move/size and menu loops, and a
  minimised window keeps drawing; a 35 s keyboard move loop and a 35 s
  minimise kept the keep-alive every ~1 s on the wire. A press on the title
  bar before the move loop starts could not be produced with posted messages
  and is not measured. In the browser a page that is not painted gets no
  animation frames or only about one a second; `graphics_lifecycle.js` then
  runs each frame the page asked for from a worker's 20 ms timer once it is
  100 ms late (at once while `document.hidden`), and from the page's own
  timer where no worker can be made. Checked in Chrome with the tab in the
  background and the window minimised (40 s: keep-alive gaps 1.1 s, against
  6 s before); a tab reporting `hidden` with frames stopped outright was not
  reproduced (Playwright never reports hidden). Such timer frames draw too,
  into a canvas nobody sees.
- (b) The children 97, 104 and 121 of a zone envelope are applied after the
  envelope's other children instead of interleaved; the only shared state is
  the projectile list order.
- (b) The reboot countdown's first decrement can come one cycle early. The
  location that drives the Tutorial Island tests is refreshed once per drawn
  frame.
- (b) The world map's Ctrl reads the keyboard per frame, not the per-cycle
  port. (Run and the staff teleport read `Keyboard.pressedKeys`.)
- (b) Whether all 29 handlers that mark the verification id also store it was
  not audited.
- (b) The minimap and camera offsets are drawn once at process start as well
  as at login; the original has zeros until the first login.
- (b) With shift-click on and Shift held, the "open the menu instead" test
  reads the chosen entry; the original reads the last entry whatever Shift
  does.
- (c)(d) The mouse recorder (123) samples at most once per drawn frame and
  does not catch up after a long one. The pinned server sends recorder flag
  0, so 123 is never sent to it.
- (c) Menu action 36 and its packet 179 (face a tile) exist only when
  `client.game == 1`, which this game is not.
- (b) The offline replay path (`--replay`) does not run the 97, 104 and 121
  handlers.

### Input

- (b) In the browser a character typed while Ctrl is reported held does not
  arrive (the loader's rule). Whether that loses the AltGr characters of
  layouts that have them was not checked; only the US layout was used. The
  earlier entry here, that `?` and `"` cannot be typed in the browser, was
  wrong:
  both always arrived, because the loader's special case for the quote and
  slash keys never matched its own key numbers. What was wrong was those
  keys' key codes, and the back-quote key's, which the page now reports.
- (b) Keys are identified by position, so letter and punctuation key codes
  follow the US layout on other layouts. Typed characters are unaffected. The
  number pad is treated as NumLock on.
- (b) The IF1 walk and the in-game drag start still test a click at one
  point: the drag controller tests `noClickThrough` layers at the click
  where the walk tests them at the pointer. All clicks in one frame share a
  timestamp.
- (b) In game a component drag begins at the mini menu click, a cycle after
  the original's (whose walk begins it), so its first update is that
  cycle's second; on the title it begins in the walk's cycle.
- (b) No mouse capture on Windows: a button released outside the window is
  not seen until the next event. The pointer leaving the window is not
  reported on desktop systems other than Windows.
- (b)(d) Extra mouse buttons are ignored.
- (b) The hook walk omits one arm of the original's visit condition: the
  drag parent of a dragged component is not visited for that reason alone.
- (b) A component key binding with a key code outside 0 to 111 counts as not
  held; the original throws.
- (b) The S key held during loading does not trigger safe mode.
- (b) macroquad's typed-character queue is no longer drained and grows by one
  entry per typed character.

### Interfaces and the script VM

- (b) Client code 1405 draws only the "FPS" line; the memory, GPU and cache
  lines have no source here.
- (b)(d) Item icons use brightness 0.7 exactly; the original adds a random
  amount within 0.015 at start.
- (b) Thin lines drawn through `legacy_geometry` (type-9 lines of width 1,
  scrollbar edges) land at about half strength under multisampling, as the
  rectangle outlines did before they were fixed.
- (b) On a scaled display the interface's scissor and its models' viewport
  are now given in surface pixels. Checked in a browser at emulated device
  pixel ratios of 1.25 and 2 only; a scaled desktop display was not tried,
  and thin lines and sprite edges at a fractional scale were not looked at
  closely.
- (b) `BitmapFont::load` takes the bottom padding only from glyphs with a
  height; `offset_line` does not consume a glyph offset for an `<img=N>` with
  a bad index and underlines one pixel low. The hover text does not use that
  function.
- (b) `Targeting::default()` and the player options default their cursors to
  0 where the original has -1; both are overwritten before use.
- (b) A forced camera angle on a locked camera assigns clamped values; the
  original assigns the raw pitch and yaw before clamping.
- (b) `Region::decode` accepts map opcode 129 after an environment entry;
  the original throws there.
- (b) Reading the preferences does not create an empty file as the
  original's read-write open does.
- (b) The friends-list limit is taken from the map's member flag
  (`Vm::members`); the lifecycle work read the original as using
  `playerMember` (`Vm::site_members`). Reported, not changed, not re-read.
- (b) `::rebuild` reaches the scene rebuild through the lifecycle; that path
  and the world map's close have tests only.
- (c) The styled context menu (CS2 5424 and 5425 store its colours; `drawB`
  is not drawn). No cached script runs 5424 to 5427.
- (c) Client codes 324, 325 and 327 are ported from the code alone: only 328
  and 1405 occur in the cache. 327 without a model reports "default
  appearance service pending" in live mode, and the design appearance is only
  ever reset to male.
- (c) The hover-text host (client code 1406) is covered by the differential
  only: no cached interface has that code. The menu-entry walk does not skip
  such a component.
- (c) IF3 type-5 components showing an item go through the sprite router; no
  offline interface shows one, so there is no capture.

### Menus, cursors and picking

- (b) One mesh per thing in the pick: an actor's body and its spot animation,
  and a pile's three items, are each tested as one model. Actor meshes are
  float and are rounded back to fine units.
- (b) Ignored by the pick's scene: the stretching-sequence tile extension,
  projectiles and spot animations as scenery, hill skew on animated
  locations, colourless plain tiles. The own player is not inserted as a
  scene actor.
- (b) Picking stops at the original's 56 tiles; the renderer draws to 70.
- (b) The 400-entry NPC cap counts world entries only. The walk tile is
  resolved when the menu is built, not on the frame after the click. Menus
  use the multiloc child resolved on the previous frame.
- (b) `isRectVisible` depends on vertex order under 32-bit overflow; the
  face order of `tile_mesh` was not checked against the original for that
  case.
- (b) The hover text's seed uses local time on Windows and UTC elsewhere and
  in the browser. With the interface hidden the workbench draws the text
  under its header, a placement of its own. (d)
- (c)(d) Cursors never change in normal play: the pinned cache has no cursor
  definitions (archive 2 has no group 33), no location, NPC, item or
  component names one, no script runs 5426 or 5427, and the pinned server
  sends -1 for player options. An id without a definition draws sprite 0, as
  the original's default type does. AWT's rescaling of cursor images is not
  reproduced.
- (c)(d) Locations with opcode 96 are never pickable, reproducing a
  sign-extension in the original's key (JVM-verified). The same corruption's
  effect on `getWallKey` (a default `walloff` for 11 wall types) is not
  reproduced.

### Camera and display

- (b)(d) The mouse wheel and middle-drag move the camera only in the
  workbench with the interface hidden. The reference core has neither; wheel
  zoom is a plugin feature there.
- (b) Offline, moving the workbench's origin does not rebase the destination
  flag or release the camera; only a live rebuild does.
- (b) The spline camera is drawn from 2048-step angles, not the original's
  radians.
- (b) A frame's interface walks all run before its camera updates, so a
  second walk in the frame where a script unlocks the camera reads the
  spline-finished flag one cycle late.
- (a)(d) Full screen is Windows and browser only, never changes the screen's
  mode and offers one size. Nothing corrects a surface that differs from the
  listed size. In the browser it needs a click or key press just before
  and is entered optimistically (see Display modes); a real grant was not
  seen in the test browser.
- (b) A first browser visit opens the title on the fixed 765 x 503 canvas at
  the top centre of the page, as the applet does in a page larger than its
  canvas; later visits open in the remembered mode.
- (b)(d) Mode 0 is reported as mode 1. With the title in mode 1 neither the
  SD nor the HD button is ticked; whether the original does the same was not
  checked.
- (c) `buildArea` and `hdr` are stored only, as in the original: nothing
  there reads `hdr`, and `buildArea` is only zeroed after a safe-mode start.
- (c) Occlusion heights (map opcode 129) are read and used for the camera's
  minimum pitch, but none of the 1,226 map files in the pinned cache carries
  the opcode, and they are not laid out for instanced scenes.

### Scene

- (a)(d) With the pinned light placement, the default, almost nothing is lit
  by point lights: 2 of 68 Lumbridge lights get a terrain mesh.
- (a)(d) The recorded prepared packs lack the later entries: no floor
  shadows, no underwater scene, no minimap label lists, and multiloc children
  without models (not drawn in the browser or from a pack alone).
- (b) Standard-detail lighting: a model's colours are baked from its normals
  as they are when it is built, with the scene's sun. The original bakes them
  when a model is first drawn and keeps the result in the model cache, so
  three history-dependent cases differ: under a sequence that turns normals
  (opcode 14) the colours stay the rest pose's here, where the original keeps
  those of the pose it drew first (a player's are always the rest pose's
  there too); a location model built from a cached model whose colours were
  already packed inherits that model's lightness in the original; an
  interface model takes the default sun here, the scene's in the original if
  the scene drew that cached model first. A zero light direction or contrast
  divides by zero in the original and gives the ambient term alone here.
  Between a change of the preference and the rebuilt scene's arrival (well
  under a second) the old scene is drawn with the new lighting; the original
  shows its loading box. With `--cpu-static-mesh` the underwater backdrop is
  drawn unlit while the preference is off.
- (b) The high detail textures preference regenerates every texture at once
  when it changes; the original empties its cache and regenerates each
  texture when it is next bound.
- (b) A location changed at run time does not update tile shade, merged
  normals, or the light meshes' diagonal-wall cuts. A wall added later with
  `walloff` other than 16 does not rescale a decoration already there.
- (b)(d) Navigation still flags collision for types hidden in HD (opcode
  82); the original's `addLoc` returns before flagging them (370 placements
  in Varrock).
- (b) LOC_ANIM uses the stored location's shape and rotation; the original
  uses the packet's.
- (b) `world.locations` keeps one location per plane, tile and layer, where
  the original's tiles hold up to five sceneries; collision may lose
  same-layer duplicates after the first dynamic change. Not investigated.
- (b) A shapeless location type placed as a non-centrepiece can receive a
  cached centrepiece model in the original (`getStaticEntity`'s cache key).
  Not reproduced.
- (b) Dynamic locations on bridge columns: the original uses the tile slot
  of the protocol plane with the heights of the plane above; the workbench
  treats the plane as physical.
- (b) Edges. The original's rules stop at its 104-tile scene; here they stop
  at the loaded area's edge: `addLoc`'s special case, the last lightness
  row, the light-mesh rules, shade next to a region with no terrain file.
  Offline, the workbench loads the 2 x 2 regions around the start, not the
  original's 104 x 104 window. (d)
- (b) Floor texture coordinates, the liquid and waterfall coordinates and
  the noise phases use world-absolute coordinates, not scene-local ones: the
  same for texture scales dividing 1,024, a phase shift otherwise.
- (b) Lighting: `Light.onBridge` is not applied; "tile exists" counts a
  location on its south-west tile only; two history-dependent selection
  cases are not reproduced (a wall piece whose neighbour is outside the
  scene, a repeated region query after a prune); footprints over 4 x 4 scan
  the first 4 x 4 tiles; players are taken as size 1; ground items, spot
  animations and projectiles carry no light key; lights enter the grid in
  this client's region order, not the original's map order, which changes
  which win the four-per-tile and 255 caps; the shadow projection direction
  comes from the scene's start region, not the player's chunk.
- (b) Materials: the specular cube map's bilinear filtering and the
  no-3D-texture fallback are not reproduced; noise is filtered in float;
  mipmap levels use a box filter where the original leaves it to the driver;
  interface models take no materials or mipmaps; without the
  standard-derivatives extension the base level is sampled as before; the
  liquid fog coordinate omits the depth-layer term.
- (b) Underwater: scenery spanning several tiles takes its water colour,
  range and surface height from its south-west tile; the pass stops the
  moment high water detail is turned off (the original decides at rebuild);
  no per-tile frustum visibility; no underwater area sounds.
- (b)(d) Where a scene has water but no underwater map, the water is blended
  over the fog colour. That follows the Java by reading; no such place was
  captured. Packs and instanced scenes without underwater data keep a
  stand-in backdrop in the raw water colour; the original's is lit.
- (b) Roofs: where the original throws (a camera outside the scene, or on
  the player's tile below pitch 310) the tile reads as unroofed; the
  workbench's free camera reaches both. Mode-1 groups are rebuilt on
  location changes as well as when the original rebuilds them. The streamed
  scene swap and live play are compile-checked only.
- (b)(d) Captures save the window's alpha, and blended draws leave it below
  255. The browser's drawing buffer has no alpha channel, so the page does
  not show through there. On the desktop a frame kept for game state 25 or
  28 still carries that alpha and is drawn blended over the frame beneath.
- (b) In the cache-built Lumbridge scene the minimap showed black (or the
  scene) over part of its circle. Captures since the mask fix (see Minimap)
  show the whole circle; tiles outside the loaded regions stay black.
- (c) Mipmap mode 1 (674 textures are mode 2, 6 are mode 0, none mode 1); a
  model or underlay with a water texture (none exists); opcode 98 `dynamic`
  locations (0 of 42,004 types); animated or multiloc underwater locations
  (none).
- (c)(d) Underwater scenes for instanced scenes (`readDynamicTerrain(true)`
  and `readDynamicLocs(true)`). No server here sends 214.

### Actors

- (b) Lean state advances once per built frame for every listed actor; the
  original advances it only for an entity actually drawn. A lean record with
  zero acceleration divides by zero in the original; here it is an error and
  the actor is drawn upright. (c)(d) No cached BAS record uses opcodes 29 to
  37, so acceleration, accelerated yaw and lean never run with this cache.
- (b)(d) A projectile frame that spans several cycles moves by all of them
  and can overshoot at the end, as in the original. Its coordinates are
  absolute, its creation-time aim is deferred to the first step, and a
  zero-delay replay loop that hangs the original stops after 65,536 frames.
- (b) Hint arrows: the foot arrow does not use the original's light
  direction and is drawn through the character-shadow material instead of the
  original's depth bias.
- (b) Attached models (`attachment`) are not ported; an NPC counts as visible
  when it has a render id.
- (b) NPC overheads are drawn only for NPCs with pick triangles, so
  distance-culled NPCs lose theirs.
- (b) The previous-body fallback keeps the body shown; the original also
  depends on its cache still holding the old model.
- (b) Location palettes are not wired to the recolour palette (the original
  shares one array across NPC, object and location types). Item sprites
  reading it is untested beyond a counter assertion.
- (c)(d) Player area sounds. The pinned server always writes sound radius 0.
- (c) NPC palette recolours: 0 of 8,590 NPC types have opcode 42.

### Minimap and world map

- (c) Minimap labels: `<img=N>` icons in names are not drawn (no cache label
  uses one); strike and underline lines are drawn unmasked on a reading of
  `GlRaster` that was not verified; the oracle observes draw calls, not
  pixels or masks.
- (b)(d) Capture runs show the seed-0 drift (minimap rotated 11 units, zoom
  -4).
- (b)(d) World map: system typeface for labels; fixed 0.7 palette (the
  original uses 0.9 to 0.6 by brightness in SD and adds a small random
  offset); the scene is not cleared while the map is open.
- (b) Offline, with no session, the world map's scene requests are not
  consumed.
- (b) The browser's world-map labels are the browser's own text (canvas 2D),
  without leading, and there is no map with the prepared packs.

### Audio

- (a) No music or ambience on desktop systems other than Windows.
- (b) The device-side timing of `AudioChannel` and the jingle's 50 ms late
  start (see Music).
- (b) The stereo preference is stored only: both mixers are fixed stereo.
  The original mixes one channel when it is off (`AudioChannel.stereo`, read
  by every PCM stream) and reopens both devices (`client.reinitAudio`). The
  audio crate's sound stream has the mono mixers; the music sequencer, the
  block readers and the two devices assume two channels.

### Browser build

What was run, and how, is under "The browser build as run".

- (b)(d) Start waits for the start archives only (see "Start-up and cache
  download"); the rest arrives behind the title, with a few extra network
  passes per frame (3 ms) beyond the original's one read window a cycle.
- (a) Never run: browsers other than Chromium, a real high-DPI display, a
  slow or failing network, refused storage quota, a hidden tab, a logout
  sent by the server, account creation.
- (b) The title's lasting deafness to the mouse once reported after Graphics
  or Audio Options was not reproduced (see "The browser build as run").
- (a) Start-up still holds one frame for about three seconds before the
  title (the starting scene and its textures) and one for about 0.4 s before
  the title scene. Scene builds are spread over frames (about 12 ms each),
  but a frame during one can still last up to about 0.2 s (one step of the
  build, such as a dense chunk's locations or a floor pass, plus the rest of
  the frame), and a few music blocks are missed then. Music also goes silent
  in a hidden tab or on any frame longer than 186 ms. (d)
- (b) A sliced build reads the client's global switches (lighting detail,
  ground decorations, the sun) when each step runs, as the desktop's worker
  thread does; a preference changed while a scene is being built can reach
  part of it.
- (a) A browser whose WebGL lacks three vertex texture units or 4096-pixel
  textures is refused at the start page. The check was exercised by
  overriding the reported limit, not on such a device; other limits the
  shaders may meet on weaker devices were not looked for.
- (b) Characters typed with Ctrl reported held are dropped (see Input).
- (b) Every connection, the world list included, goes to the launch
  WebSocket address. The update connection keeps reporting "title screen"
  after start; the Rust server ignores it. With a server that accepts a
  reconnect and never answers, game state 40 lasted 75 s before the title
  returned; the desktop was not timed.
- (b) `tools/build_visual_web.py` was updated for the cache and music work
  but not run; the runs staged the same files with a script outside the
  repository. Its version hash covers a fixed list of page scripts.
- (b)(d) `THIRD_PARTY_NOTICES.md` does not list the pure-Rust bzip2
  implementation the browser build uses.

### Verification gaps

- No regenerating command for the title loading-bar cases, the login-flow
  trace, the world-map core hashes, the store-write cases or the outgoing
  encoder bytes; three of those harnesses are not in the repository.
- No automated test: `world_pick.rs` and the picking insertion order, the
  dispatch edits, the GPU presenters (world map, item-icon cache, cursor),
  the atmosphere and preference wiring, the unlit material, the shadow
  sequence choice.
- Captures only: every shader (light selection, materials, material 3), the
  pass order, the depth biases, the pack path of the underwater scene.
- One mutation of the actors port survives its corpus (`braking < squared`
  against `<=`).
- No frame of the original client has been compared with this one, apart
  from the title's controls (by eye, under "Title screen and login").
- The in-game Graphics and Audio settings (interfaces 742 and 743) were run
  only headless (`examples/title_probe.rs`): neither example server opens
  them from the options tab, as the original server's
  `SettingsTabInterface` does.

### Decisions for the owner

Collected from every stream. None blocks the others.

1. **Shift-click** (a 2009scape addition; ported, off by default). Keep off;
   turn on by default as the pinned client has it; or remove.
2. **Wheel zoom in live play.** Leave it to the workbench (the reference core
   has none), or adopt the plugin's `UpdateCameraZoom` step.
3. **Offline workbench camera.** Keep the fixed starting view without login
   randomisation or drift, so captures repeat; or give it the camera drift,
   which shifts every offline capture.
4. **Light placement default.** Keep the pinned loader's placement (2 of 68
   Lumbridge lights reach the terrain) or make `--correct-lights` the default
   (78 of 78, departing from the pinned loader).
5. **Opcode-82 collision.** Keep flagging collision for types hidden in HD,
   or skip them as the original's HD path does.
6. **Opcode-96 key quirk.** Keep "never pickable" as verified against the
   JVM, or drop it as a decompilation artefact.
7. **Diagnostic launch aids.** Keep or drop each of `--script-cursor`,
   `--minimap-all-labels`, `--loc-varbit`, `--fpson`, `--showcase-npcs`,
   `--window-mode`, `RS09_FRAME_DUMP` (its `take`, `command` and `inject` files),
   `RS09_POSTED_POINTER`, the
   browser start page's song field and its `?cursor=ID`.
8. **World map.** (i) Leave the scene in place while the map is open, or
   clear it as the original does. (ii) Label font: a system face, or ship
   one. (iii) Palette: fixed 0.7, or follow the brightness preference and the
   random offset as the original's SD mode does.
9. **Projectile overshoot at low frame rates.** Keep the original's
   behaviour or restore the earlier Rust behaviour that avoided it.
10. **Spline camera in live play.** Keep advancing it every cycle
    (`Protocol` 2895 to 2901) or advance it only on the login screen.
11. **Key bindings and the destination flag.** Bindings now click inside the
    walk rather than before it, and the flag is cleared on arrival by the
    crowd pass; both follow the Java's place and are a few lines to reverse.
12. **Title screen.** World-list failure: fatal as in the original, or
    retry. Failed launch-file login: standard error only, or open the form
    (which ties the host to script ids).
13. **Fixture sizes.** Keep the large fixtures (folders in this tree: actors
    5.9 MB, roofs 4.4 MB, picking 3.9 MB, ui-fidelity 2.9 MB, input 1.8 MB,
    camera 1.2 MB, display 1.1 MB) or reduce the case counts.
14. **Unused code.** Delete or keep `floor_blend.rs`; `LoginLoop`, `LiveDial`
    and `Credentials` (used only by `tests/lifecycle_live.rs`);
    `roof::has_entity` beside `loc_placement`'s own decision;
    `login_flow::preferences_to_int` beside `Preferences::to_int`.
15. **Mouse.** Count extra buttons as left clicks, as Java 7 and later do, or
    ignore them. (IF3 clicks are now tested at `clickX/clickY` through
    `PointerInput::click`; revert to the pointer if that is not wanted.)
16. **Hover text.** Add a local-time source for the seed outside Windows or
    accept UTC there. Show the text in the hidden-interface workbench, and
    where, or not at all.
17. **Cursors.** Keep sprite 0 for an id without a definition (the
    original's quirk) or not.
18. **Brightness jitter.** Keep the fixed 0.7 for item icons and palettes or
    reproduce the random offset.
19. **Player area sounds.** Port them although the pinned server never sends
    a radius, or leave them.
20. **Lean state.** Move it onto the canonical movement state and tie it to
    drawn actors, or leave it (no cached record uses it).
21. **Roofs.** Keep upper-level shadows always on (as the Java) or restore
    the earlier level-0-only default. Let the workbench's free camera drive
    the camera-line rule or not. Load the full 104 x 104 region set offline
    or keep 2 x 2 regions.
22. **Minimap captures.** Keep the seed-0 drift in captures or pin it to
    zero.
23. **Prepared packs.** Rebuild `demo.pack` and the web packs to carry floor
    shadows, underwater maps, world-map labels, multiloc children's models
    and `p12_full`; whether `floorshadows` stays in `minimap/assets` or gets
    its own entry. A rebuilt `demo.pack` (from `world_pack`) also carries
    `minimap/assets`, the map scenes and every morph child, after which the
    minimap's pinned tables are no longer used by it. Adding models needs the `world_pack` test to allow new
    `model/` entries.
24. **Capture alpha.** Write captures opaque, or make the world pipelines
    keep destination alpha. The browser's drawing buffer is now made without
    alpha; the alternative there is an alpha-keeping blend in every pass.
25. **Underwater.** Port it for instanced scenes or leave the stand-in. Light
    the stand-in's colour or drop the stand-in once packs are rebuilt. Accept
    water over the bare fog colour where there is no underwater map.
26. **Materials.** Accept eight bound texture units and three samplers in the
    vertex stage (WebGL1 guarantees eight units and no vertex samplers) and
    about 32 MB more GPU memory for the mip atlas, or not. Apply materials to
    interface models (their eye space is unchecked) or not.
27. **Display.** Report mode 0 as mode 1 (now) or as a nominal mode 0, which
    scripts would treat as SD. Keep the new live defaults (1024 x 768, fixed,
    samples from the preference) or the old (1280 x 800, resizable, 4x). In
    the browser the page is the applet's container and the title opens
    fixed, as the applet's does; the alternatives are a page that sizes the
    element to 765 x 503, or a resizable default there.
28. **Terrain test keys.** Keep the test's dependence on the git-ignored
    `runs/player-session/xteas.json` or embed the region keys in the fixture.
29. **Keep-alives.** Done: a rebuild writes the original's four or seven.
    Open: whether a frame longer than 200 ms should write one of its own,
    as the client did before (the original does not).
30. **Mouse recorder.** Catch up during long frames (changing a pinned test)
    or not.
31. **Per-cycle restructure.** Done (see "Main loop"). Open: whether a
    capture run should keep its world cycle one behind its interface cycle
    (kept, so captures compare with earlier ones), and whether the login
    state machine should move into the cycle as well.
32. **Browser.** Schedule the work to start before the download finishes
    (every pack reader must accept "not yet") or keep the wait. Accept the
    pure-Rust bzip2 dependency and the `--import-undefined` linker flag;
    update the notices file. Read-ahead for music: 186 ms, lower or higher.
    Scenes are now built over several frames (12 ms a frame; `?slice`),
    preferences are kept in local storage and full screen goes through the
    browser's own: keep these, change the slice, or move the build to a
    worker (it would need the cache and the prepared scene to cross between
    modules). Whether `::wm3` should, in a browser, scale a 1024 x 768
    canvas to the screen (the nearest thing to the original's mode change)
    instead of falling back.
33. **Styled menu.** Leave CS2 5424's menu undrawn (no cached script uses
    it) or port `drawB`.
34. **Error reports.** Post the original's reports to `clienterror.ws` or
    keep them local.
35. **Source pins.** Move the reference hashes kept in `tests/actors_oracle.rs`
    and `tests/lighting_oracle.rs` into `tests/server_events.rs` or leave
    them.
