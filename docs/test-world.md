# Testing against a disposable world

The test-world kit lets a session (a person or an agent, with no one
watching) check a change against a real `rs09-server --world`: boot a
throwaway world, put a character where the test needs it, drive it with the
headless agent and assert on what the server sent.

It has three parts:

| part | command | what it does |
| --- | --- | --- |
| disposable world | `cargo xtask test-world start/stop/list` | its own copy of the release server on a free loopback port, its own saves folder |
| character placement | the `character` of a script | a save written before login: location, items, equipment, skill levels |
| agent runner | `cargo run --release -p rs09-headless --example agent_run` | logs the character in, runs JSON steps, prints the `View` and events as JSON |

**Scope.** The kit only drives existing behaviour; it adds no server or
agent code beyond these entry points. What it establishes is server
behaviour: the packets the server sent in answer to the original client's
packets. It does not show rendering, click picking or menus (the headless
agent has no renderer and no cache definitions; see
[headless-agent.md](headless-agent.md)).

**Test runs never open a window.** The kit is the server plus the
`rs09-headless` agent only; nothing in it starts `rs09-client`, and it does
not use `tools/play.ps1` (whose server-only mode is `-NoClient`). A
visual-client check is a separate, deliberate step that someone chooses to
take, and it runs the client with `--mute`.

**It never drives the play world.** `test-world start` refuses port 43594
(the port `cargo xtask play` uses) and never picks it. `agent_run` and the
runner refuse an address on port 43594, and refuse to seed unless the saves
folder belongs to a test world: under `runs/test-world/`, or in a folder
that has a `world.json` (seeding replaces `players/<name>.json`). `stop`
kills only a process this command started (below).

## 1. A disposable world

```powershell
cargo xtask test-world start --name goblins
```

This:

1. refuses a world of that name that is still running;
2. checks the cache (`assets/cache`, or `--cache DIR`) and the world folder:
   `--data DIR`, else `runs/play/world-data` written by
   `cargo xtask play-world` ([play-locally.md](play-locally.md); that needs
   the `reference/` clones of `cargo xtask bootstrap`). A copy of
   `runs/world-trace-oracle/template` works as `--data` too;
3. replaces the folder `runs/test-world/<name>/` only when it is one this
   command made (it holds `world.json`), so every start has empty saves;
4. builds the release server (`cargo build --release -p rs09-server`;
   `--no-build` skips it);
5. copies `target/release/rs09-server` into `runs/test-world/<name>/` and
   runs that copy, so a running world never locks the build's exe (Windows
   locks a running exe) and worlds do not share a binary across rebuilds;
6. picks a free port on 127.0.0.1 (never 43594, which `--port` refuses
   too) and starts, detached,
   `runs/test-world/<name>/rs09-server --world --cache C --saves runs/test-world/<name>/saves --data D --tcp 127.0.0.1:<port> --verbose`
   with its output in `runs/test-world/<name>/server.log` and
   `server.err.log`;
7. waits for the server's `Native world listening on TCP` line (a server
   that exits first, or listens elsewhere, is an error; `--timeout SECS`,
   default 300) and prints `world.json`. Any failure after the spawn
   (including writing `world.json`) stops the child before the command
   returns its error:

```json
{
  "address": "127.0.0.1:65244",
  "name": "goblins",
  "pid": 35872,
  "saves": "...runs/test-world/goblins/saves",
  "log": "...runs/test-world/goblins/server.log",
  ...
}
```

The command returns once the server listens; the server keeps running. On
Windows the server is started without any of this command's inheritable
handles, so a caller reading the command's output to its end is not held
until the server exits.

```powershell
cargo xtask test-world list
cargo xtask test-world stop --name goblins
cargo xtask test-world stop --name goblins --clean
cargo xtask test-world stop --all
```

`--log-every-refusal` starts the server with `RS09_NOT_PORTED_LOG=every`,
which logs every named not-ported call instead of only the first
([refusal-census.md](refusal-census.md)).

`stop` kills only the pid in that world's `world.json`, and only while that
process is still `rs09-server` with that world's saves folder on its command
line; anything else is reported as not running and left alone. A failure to
read a process's command line (PowerShell's `Get-CimInstance` on Windows,
`ps` on Unix) is an error, never "not running", so `start` does not replace
the folder of a world it could not check. A process still running 10 seconds
after the kill is an error. `--clean` also removes the folder (logs, saves
and the exe copy); without it they stay for reading. With several worlds
(`--all`, or `--name` repeated), a folder with a bad name or `world.json`, a
failed kill or a failed removal is reported and the others are still
stopped; the command then exits with an error naming each. `runs/` is
git-ignored.

Only the Windows path has been run. The Unix path (`ps -o args=` for the
command line, `kill` sending SIGTERM, `process_group(0)` for detaching)
compiles but has not been tested.

## 2. A script

A script is one JSON file: the character, then the steps.

```json
{
  "character": {
    "name": "kitgoblin",
    "location": [3250, 3235, 0],
    "skills": { "attack": 20, "strength": 20, "defence": 20, "hitpoints": 20 },
    "equipment": [{ "slot": 3, "id": 1277 }],
    "inventory": [{ "id": 1925 }]
  },
  "steps": [
    { "do": "assert", "that": { "wearing": 1277 } },
    { "do": "attack", "npc": { "types": [1769, 1770, 1771, 1772, 1773, 1774, 1775, 1776] } },
    { "do": "wait", "until": { "npcHit": "target" }, "timeout": 20 },
    { "do": "wait", "until": { "npcDied": "target" }, "timeout": 90 },
    { "do": "wait", "until": { "all": [{ "xpGained": "attack" }, { "xpGained": "hitpoints" }] }, "timeout": 5 },
    { "do": "snapshot" }
  ]
}
```

### Character placement

`character` fields:

| field | meaning |
| --- | --- |
| `name` | 1..12 letters, digits or `_` (lower-cased); not `kitseed` |
| `password` | default `test` (the world folder keeps accounts in memory; any password logs in) |
| `location` | `[x, y, plane]` |
| `inventory` | `[{ "id", "amount"?, "slot"? }]`; slots default to the list order, amounts to 1 |
| `equipment` | `[{ "slot", "id", "amount"? }]`; slot 3 is the weapon, 13 the ammunition |
| `skills` | `{ "attack": 20, ... }` (names or indices); each sets the static and dynamic level and that level's experience (hitpoints also the current hitpoints) |
| `tutorialComplete` | default `true`: the `tutorial:complete` attribute, so the character is off Tutorial Island's rules (no 175-experience cap) and gets the welcome screen at login, which the runner answers with its Play button (378, child 140) |

The save is made the way the `live_world` and `live_agent` tests make
theirs: the first time a saves folder is used, a template character
(`kitseed`) logs in and out, so the server writes a new character's save with
its own writer. Every seeded character is a copy of that file with the fields
above replaced in the writer's own form (numbers as strings, experience as a
double), checked with the native save parser
(`player::save::parser::parse_text`) and written to
`saves/players/<name>.json` before the login. At logout the server rewrites
the file itself. A character seeded this way starts where the script says,
not on Tutorial Island.

Seed a character only while it is logged out (the server writes its file at
logout). A run ends only after the server has written the character's
logout save, so the next run can re-seed and log in the same character at
once; a login refused as "already logged in" (code 5) is retried for up to
15 seconds. `--no-seed` logs in a character whose save is already there.

### Steps

Each step is an object with `"do"`:

| `do` | fields | sends |
| --- | --- | --- |
| `walk` | `to: [x, y]`, `run`? | `Action::Walk` |
| `attack` | `npc` | `Action::Attack` (option slot 1) |
| `talkTo` | `npc` | `Action::TalkTo` (option slot 0) |
| `npcOption` | `option` 0..4, `npc` | `Action::NpcOption` |
| `itemOption` | `option` 0..4, `item` (id) | `Action::ItemOption` on the first slot holding it |
| `useOnItem` | `item`, `target` (ids) | `Action::UseOnItem` |
| `useOnNpc` | `item`, `npc` | `Action::UseOnNpc` |
| `useOnScenery` | `item`, `scenery` (id), `tile: [x, y]` | `Action::UseOnScenery` |
| `useOnPlayer` | `item`, `player` (index) | `Action::UseOnPlayer` |
| `scenery` | `option` 0..4, `scenery`, `tile` | `Action::Scenery` |
| `continue` | `timeout`? | the open dialogue's Continue (`Action::Resume`) |
| `choose` | `text`, `timeout`? | the first dialogue option containing `text` |
| `close` | | `Action::CloseInterface` |
| `dialogue` | `max`? (12), `option`? (0), `timeout`? (4 s for the first) | Continue, else that option, until no dialogue is open; closes one still open after `max` |
| `groundItem` | `option` (2 Take, or 3), `item`, `tile: [x, y]` | the ground item option (opcode 66 or 33) |
| `button` | `interface`, `child` | IF_BUTTON (155), as the original client sends a button |
| `wait` | `until`, `timeout`? (seconds, default 20) | nothing: reads frames until the condition holds |
| `assert` | `that` | nothing: the condition must hold now |
| `pump` | `seconds`? | nothing: reads frames for that long |
| `snapshot` | | nothing: the `View` snapshot goes into the step's report |

An `npc` is `{"types": [ids]}` or `{"type": id}` (the nearest local NPC of
those definitions whose health bar is not empty; `"skip": n` takes the
n+1-th nearest), `{"index": n}`, or `"target"` (the NPC the last NPC step
picked). Steps that need an NPC, an item or a dialogue read frames for a
few seconds until it is there.

Conditions (`until`, `that`):

| condition | holds when |
| --- | --- |
| `{"position": [x, y, z]}` | the player is on that tile |
| `{"near": {"tile": [x, y], "within": n}}` | Chebyshev distance at most n |
| `{"npcHit": npc}` | a hit mark on that NPC since the last action step |
| `{"npcDied": npc}` | it left the NPC list after an empty health bar, since the last action step |
| `{"dialogue": true}` / `false` / `"text"` | a chatbox dialogue is open / is not / shows that text |
| `{"message": "text"}` | a game message containing it since the last action step |
| `{"holding": id}` or `{"holding": {"id", "amount"}}` | the inventory holds at least that many |
| `{"wearing": id}` | likewise in the equipment |
| `{"interface": id}` | that interface is open |
| `{"xpGained": skill}` or `{"xpGained": {"skill", "atLeast"}}` | experience since login (default at least 1) |
| `{"level": {"skill", "atLeast"}}` | the level the server last sent |
| `{"all": [...]}`, `{"any": [...]}`, `{"not": c}` | combinations |

Everything a condition reads comes from frames the server sent (the
agent's `View`); a sent action is never taken as its outcome. Event
conditions read the events since the last step that sent something, so a
`wait` after an `attack` sees that fight's hits and death.

## 3. Running a script

```powershell
cargo run --release -p rs09-headless --example agent_run -- --world runs/test-world/goblins crates/headless/scenarios/goblins.json
```

`--world DIR` reads the address and saves folder from `world.json`;
`--address IP:PORT --saves DIR` gives them directly (without `--saves`, use
`--no-seed`). Progress and every event go to stderr; the report goes to
stdout (and to `--output FILE`):

- `ok`, `error` (null, or what went wrong besides a failed wait), `character`;
- `steps`: each step's spec, `ok`, seconds and detail (the action sent, the
  NPC index picked, the condition, or the snapshot);
- `experienceGained` per skill since login;
- `events`: every event of the run in order (messages, hits on NPCs and the
  player, NPCs leaving, experience, levels, item slots, dialogues,
  interfaces);
- `start` and `view`: the `View` snapshot right after login and at the end.

The run stops at the first failed step, unless the script sets
`"keepGoing": true`: then every step runs, and a step that cannot be sent
(no such NPC or item in view) is only a failed step, not an error. Each
step's report also has `startedMs` and `endedMs` (Unix milliseconds), and
keys a step has beyond its `do` fields (such as a `label`) are ignored and
kept in the report. The refusal census ([refusal-census.md](refusal-census.md))
uses all three. A step that errors (a bad step, a
lost connection) is a failed step with its error in `detail` and in
`error`; the report and the final view are still printed, and the character
is always logged out. The run then waits up to 20 seconds for the server to
write the character's logout save; if it does not, that is an error too.
Exit status: 0 when every step passed, 1 when a wait or assertion failed (or
a `continue`/`choose` found no dialogue), 2 on an error (a bad script, no
server, a failed login, a step's error, a failed logout or a missing logout
save). Every login attempt has a 20-second limit. With `--world`, a run that
does not exit 0 also prints the last 40 lines of the world's `server.log` to
stderr.

## Worked example: a Lumbridge goblin

The script above is `crates/headless/scenarios/goblins.json`: a character
with attack, strength, defence and hitpoints 20 and a bronze sword (1277)
next to the goblins east of Lumbridge (ids 1769..1776 spawn around
3247..3264, 3222..3254) attacks the nearest one and waits for a hit mark,
its death, and attack and hitpoints experience.

```powershell
cargo xtask test-world start --name goblins --data runs/world-trace-oracle/template
cargo run --release -p rs09-headless --example agent_run -- --world runs/test-world/goblins crates/headless/scenarios/goblins.json
cargo xtask test-world stop --name goblins
```

A run (2026-11-27, Windows, release build), stderr without the event lines:

```text
seeded kitgoblin at [3250, 3235, 0]
kitgoblin logged in at Some([3250, 3235, 0])
step 0 assert ok (0.0 s)
step 1 attack ok (0.0 s)
step 2 wait ok (2.4 s)
step 3 wait ok (9.0 s)
step 4 wait ok (0.0 s)
step 5 snapshot ok (0.0 s)
```

The report had `"experienceGained": {"attack": 20, "hitpoints": 6}`, and its
events the goblin's hit marks (`npcHit` with damage and health bar) and its
`npcRemoved` with `died: true`.

The same script is a test that boots the world in-process (as
`live_agent` does) and needs no server process or visual client:

```powershell
cargo test --release -p rs09-headless --test test_world_kit -- --nocapture
```

It runs in debug and release builds. When it fails it prints the error, the
run's events and the final view (the player's tile, the target's tile,
health bar and facing); `RS09_LIVE_VERBOSE=1` adds the server's own log.

**Open issue: a stall, unexplained.** It was seen once, in the first debug
run (2026-11-27). The goblin (type 1771) took one hit from the character
(damage 1, health bar 204 of 255, 4 attack experience), then neither side
hit again (no further hits, no retaliation, no experience) until the
90-second wait ran out, while frames kept arriving. What is known:

- It is not the wait being too short for a debug build: fights took 1 to 16
  seconds, and it did not come back in about 75 later attacks (six more
  debug and seven release runs of the test, and 60 kills in a row from one
  character in a release `--world` server).
- The `attack` step picked an NPC that could be attacked: that goblin took
  the character's hit. The selector does check the type and that the health
  bar is not empty, but not the option label (the agent has no
  definitions), so a script that lists a type with no Attack in slot 1 gets
  whatever the server does with that option.
- The cause is unknown. The run kept no server log, so nothing shows whether
  the server, the client side of the kit or the goblin's movement stopped the
  fight. A hand-driven client was not
  tried (test runs open no window). Until a recurrence is read, treat it
  as unexplained, not as server behaviour.

It needs `assets/cache` and a world folder (`RS09_STARTUP_WORLD`, else
`runs/startup-loaders-oracle/template`, `runs/world-trace-oracle/template` or
`runs/play/world-data`); without them it prints SKIPPED and checks nothing,
unless `RS09_REQUIRE_LIVE=1` is set, which makes a missing input fail the
test. Set it wherever the inputs should be present, so a skip cannot pass
as a check.

## Limits

- The view has no cache definitions: NPCs and items are named by id, and
  `attack`/`talkTo` are the conventional option slots
  ([headless-agent.md](headless-agent.md)).
- Use on an NPC, scenery or player sends the original bytes, and the live
  world walks to the target and answers "Nothing interesting happens."
  ([live-world.md](live-world.md)). The kit can drive it; the worked example
  does not check it.
- What the live world runs, answers by name or refuses is listed in
  [live-world.md](live-world.md); a script reaching a refused system sees
  whatever the server sends then, often nothing.
- One character per script run; run several `agent_run` processes against
  one world for more players.
- Goblins wander and respawn; the example attacks whichever is nearest when
  the step runs.
