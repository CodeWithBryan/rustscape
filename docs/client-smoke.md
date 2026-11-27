# Client smoke test

`cargo xtask client-smoke` drives the real visual client (`rs09-client`)
against a real `rs09-server --world`, with no person at the controls, and
checks a few key screens and interactions from the client's structured
capture output (JSON state, pick rows, menu entries), not pixels.

It exists because unit tests can all pass while the running client is
broken: live NPCs could not be clicked (CodeWithBryan/rustscape#11) and only
a person clicking found it. This test finds that bug (below).

**It opens a window.** Run it only when you mean to. The client is always
started with `--mute` (and `--no-music`). On Windows the window is created
minimized and is never activated, so it does not take focus or cover what
you are doing: it shows only as a taskbar button for the run. `--show`
starts it in a normal window, still without taking focus. Other systems
start it normally (unverified there). The client and the world the command
started are stopped when it ends, on failure and on a time-out too.

## Running it

```powershell
cargo xtask client-smoke --data runs/world-trace-oracle/template
```

It needs `assets/cache` and a world folder (see
[test-world.md](test-world.md)); `--data` takes a copy of
`runs/world-trace-oracle/template`, and without it the command uses
`runs/play/world-data` from `cargo xtask play-world`.

| option | meaning |
| --- | --- |
| `--data DIR`, `--cache DIR` | the world folder and cache, as for `test-world start` |
| `--no-build` | skip `cargo build --release` of the server, the client and `agent_run` |
| `--show` | show the window (not activated) instead of minimizing it |
| `--npc NAME` | the NPC to point at; default `Goblin` |
| `--timeout SECS` | the limit for the client's whole run; default 300 |
| `--keep-world` | leave the test world running afterwards (`cargo xtask test-world stop --name ...`) |

The exit status is 0 on a pass. Everything is written to
`runs/client-smoke/client-smoke-<seconds>/`: `report.md` (one line per
check), `report.json` (the checks with their evidence), the client's
capture files, the client's and the server's logs, and the seeding run's
report.

## What it does

1. Starts a disposable world with the test-world kit (`test-world start`: a
   free loopback port, never 43594, and its own empty saves).
2. Seeds the character `smokegoblin` at 3250, 3235 (beside the goblins east
   of Lumbridge castle), with level 20 attack, strength, defence and
   hitpoints and a bronze sword, by running the kit's `agent_run` with a
   one-step script (it writes the save and logs in and out once).
3. Starts the client auto-logged-in (`--connect`, `--username`, the password
   in `RS09_CLIENT_PASSWORD`) as a live capture with
   `--capture-npc-menu Goblin`. That switch is a scripted pointer inside the
   client (`crates/client/src/game/npc_menu_capture.rs`). Its mouse events
   enter the client's input translator the way a window event on the canvas
   would, so the pick, the menus and the click handling are the frame's own:
   - on the welcome screen it clicks Play (378, child 140);
   - in the world, once a goblin is in the frame's NPC list, it asks for the
     pick probe (`--pick-probe`'s grid of picks), taken in `build_menu` right
     after the frame's own mouse pick and from the same inputs, and chooses a
     pixel where the probe hit a goblin;
   - it moves the mouse there and keeps the menu that frame builds;
   - it right-clicks there and keeps the menu that opened, with its bounds;
   - it left-clicks that menu's `Attack` row, keeps the packets the client
     queued, waits up to 30 seconds for the server's combat experience in the
     stats it sends, and ends the capture.

   Each stage is written to `npc-menu-capture.json` as it ends; a stage that
   cannot finish is recorded with its reason and the capture ends there.
4. Waits for the client to exit (or stops it at the time limit), stops the
   world and checks the files.

## The checks

| id | passes when |
| --- | --- |
| a | the script got past the welcome screen to the world view, and `live-state.json` says `in_game` |
| b | the player's tile is within 20 tiles of the seeded one, `loaded.json` has placed meshes, and the probe's NPC list has a goblin |
| c | the probe hit a goblin whose row says it `enters_pick` (a model and pick triangles at the frame's pick), and the menu built with the mouse on it has `Attack` naming it |
| d | the right-click menu opened there lists `Attack` naming it |
| e | choosing it queued an NPC option packet (opcodes 17, 16, 4, 19 or 2) and the client then received a rise in attack, strength, defence or hitpoints experience |

## Catching the unclickable NPCs

Before #11 the live NPC list was rebuilt every frame with empty pick
triangles, so the frame's pick skipped every live NPC. Without #11 the
probe's rows say `enters_pick: false` with zero triangles for every goblin,
the probe hits none, and the run fails at the search stage: check c (and d,
e) fail with "the pick probe never hit an NPC named Goblin". With #11 merged
the same run passes. (The probe has to be taken at the frame's pick, as
#13 does; taken after the draw, it saw the meshes the draw had just made
and called the NPCs pickable.)

## Scope

This checks the running client against the running server for one path:
login through the welcome screen, the Lumbridge scene, pointing at an NPC,
its menus and an attack. It says nothing of rendering quality (no pixels are
compared), of input from the window system (the scripted events start at
the translator), of other screens or interactions, or of other systems than
Windows, where it was developed. The goblins wander, and a miss gives no
experience, so check e waits for a hit within 30 seconds; a run where every
swing in that time misses fails e.
