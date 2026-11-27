# Client smoke test

`cargo xtask client-smoke` drives the real visual client (`rs09-client`)
against a real `rs09-server --world`, with no person at the controls, and
checks a few key screens and interactions from the client's structured
capture output (JSON state, pick rows, menu entries), not pixels.

It exists because unit tests can all pass while the running client is
broken: live NPCs could not be clicked (CodeWithBryan/rustscape#11) and only
a person clicking found it. This test finds that bug (below).

**It opens a window.** Run it only when you mean to. The client is always
started with `--mute` (and `--no-music`). On Windows the command starts it
with CreateProcess and `STARTUPINFO.wShowWindow = SW_SHOWMINNOACTIVE`, which
the first `ShowWindow` of the process (miniquad's) takes in place of its
own: the window should start minimized and never be activated, showing only
as a taskbar button. The runs so far passed that way (the scripted pointer
does not need the system cursor), but nobody has watched the screen during
one to confirm it. `--show` starts it with `SW_SHOWNOACTIVATE` instead (a
normal window that does not take focus). Other systems start it normally
(untested there). The client (held by its process handle, so no other
process can be reached) and the world the command started are stopped when
it ends, on failure and on a time-out too.

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
| `--npc-types ID,...` or `any` | keep to those NPC types; default for `Goblin`, the types the live world fights (1769 to 1776), and none after `--npc` |
| `--skip-welcome` | seed the character without `tutorial:complete`, so the login skips the welcome screen (see below) |
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
   - on the welcome screen it clicks Play (378, child 140), a second after
     it is laid out, and again every five seconds, four times at most;
   - in the world, once a goblin is in the frame's NPC list, it asks for the
     pick probe (`--pick-probe`'s grid of picks), taken in `build_menu` right
     after the frame's own mouse pick and from the same inputs, and chooses
     the probe's hit on a goblin nearest the middle of its hits (the probe's
     new `middle_pixel`);
   - it moves the mouse there and keeps the menu that frame builds once it
     has an Attack for a wanted goblin; when the goblin walked off the point
     it searches again, five times at most;
   - it right-clicks there and keeps the menu that opened, with its bounds;
   - it left-clicks that menu's `Attack` row for a wanted goblin, keeps the
     packets the client queued, waits up to 30 seconds for the server's hit
     on that goblin (its health bar) and combat experience, and ends the
     capture.

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
| e | choosing it queued an NPC option packet (opcodes 78, 3, 148, 30 or 218) and a hit on the NPC the Attack entry names (its health bar) then reached the client |

## Catching the unclickable NPCs

Before #11 the live NPC list was rebuilt every frame with empty pick
triangles, so the frame's pick skipped every live NPC. On 2026-11-27, with
`--skip-welcome` (next section):

| client | result |
| --- | --- |
| this branch (with #13, without #11) | FAIL at search: 19 goblins in the frame's list, every one with 0 pick triangles and `enters_pick: false`; the probe hit no NPC. a and b pass; c, d and e fail |
| the same with #11 merged | PASS: the probe hit goblin 6204, the hover and right-click menus had Attack for it, the click sent NPC option 1 (opcode 3) for 6204, and its hit and hitpoints experience came back |

The probe has to be taken at the frame's pick, as #13 does: taken after the
draw, it saw the meshes the draw had just made and called the NPCs
pickable.

## Server gaps it found

- **The welcome screen's Play does nothing for the visual client.** The
  client, like the original, sends `CLOSE_IFACE` (184) and then the button
  (155, 378:140). The original's `LoginInterface.kt` refuses that close with
  `onClose(378) { player.locks.isLocked("login") }`; the live world answers
  `InterfaceListeners.runClose` with the "no content listener: true" default,
  so the close removes the welcome screen on the server and the button that
  follows is refused. The headless agent passes only because it sends 155
  alone (with a 184 first, its login times out too). Until that listener is
  ported the default run fails check a; `--skip-welcome` avoids the screen.
- **Goblins of type 2278** (also in Lumbridge) accept an Attack and are never
  swung at, from the visual client and the headless agent alike, with or
  without `tutorial:complete`. The default `--npc-types` keeps to the types
  that fight.
- A character without `tutorial:complete` (`--skip-welcome`) is under the
  tutorial's rules: its attack experience dropped from 4,470 to 175 at the
  first hit. Check e needs only the hit.
- Lobby players are not reset by the update sequence, so their per-tick
  packet counts never clear: after about ten keep-alives on the welcome
  screen, reads that hold one more are cut short ("Skipping packet 93
  because already received more than 10"). Whether the original does the
  same was not checked.

## Scope

This checks the running client against the running server for one path:
login (through the welcome screen, or past it with `--skip-welcome`), the
Lumbridge scene, pointing at an NPC, its menus and an attack. It says nothing of rendering quality (no pixels are
compared), of input from the window system (the scripted events start at
the translator), of other screens or interactions, or of other systems than
Windows, where it was developed. The goblins wander: a goblin that walks off
the probe's point is searched for again, and a run can still fail c if that
happens five times. Check e waits 30 seconds for the hit.
