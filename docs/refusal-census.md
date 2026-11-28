# The refusal census

The census measures what a player hits in the live world that is not
ported. Headless agents tour busy areas, try every option they can reach,
and the server's log is collected while they do. Every refusal and error
the server logs is counted and ranked by how many distinct player actions
hit it. The result is a measured priority list for what to port next.

```powershell
cargo xtask census run
```

The command builds the release server and the kit's `agent_run`, starts a
disposable world, runs the tours one after another, stops the world, and
writes `runs/census/report.md` and `runs/census/report.json`. The twelve
tours take about 50 minutes of tour time (the 2026-11-27 runs: 48 and 50
tour minutes), plus the builds and the world's start-up. `--only lumbridge_castle,draynor`
runs only some tours, `--no-build` skips the builds, `--output DIR` writes
somewhere else, `--tour-timeout SECS` (default 1500) kills a tour's agent
that runs longer, and `--data DIR` picks the world folder (by default
`runs/world-trace-oracle/template`, else `runs/play/world-data`). The server
only reads the world folder: its configuration copy, saves and server store
go under the test world's `saves/` folder, and its start-up refuses writes
outside it. It needs
`assets/cache` and a world folder, the same inputs as the test-world kit
([test-world.md](test-world.md)).

**Scope.** The census reports what the server logs while scripted agents
send the original client's packets. It is not a coverage measure. Content
that answers with nothing and logs nothing does not appear. Neither does
content the tours do not reach: other areas, quests, skilling loops, combat
past the first swings, and anything behind an interface the agent cannot
see. A refusal's rank says how often these tours hit it, not how much it
matters to a real player. Credit is by time window, not causation (see
[How a line is attributed](#how-a-line-is-attributed)).

**When a run does not count.** The census exits with an error, and the
report opens with a warning, when a tour's agent exited with an error or
was killed at the time limit, a tour reported no steps or failed more than
half of them, or the server logged no not-ported line at all. The report
is still written, so the cause can be read.

## What runs

| part | where |
| --- | --- |
| tours | `crates/headless/scenarios/census/*.json`, written by `cargo xtask census tours` |
| runner | the kit's `agent_run` (`crates/headless/examples/common/kit.rs`) |
| world | `cargo xtask test-world start --name census --log-every-refusal` |
| log mode | `RS09_NOT_PORTED_LOG=every` (`crates/server/src/world/live/not_ported.rs`) |
| census | `xtask/src/census.rs` |

### The log mode

The live world names every call into a system without a port
(`world/live/not_ported.rs`). Normally `--verbose` prints only the first
call of each name. With the environment variable
`RS09_NOT_PORTED_LOG=every`, every call is printed with its running count:

```text
[live] not ported, skipped: InteractionEvent dispatch (entity event hooks not ported) [#14]
```

That is the only server change. It changes what is logged, nothing else,
except that writing every line to standard error takes time on the world
thread, which slightly perturbs tick timing during a census. The prefixes
are constants in `not_ported.rs` (`SKIPPED_PREFIX`, `DEFAULT_PREFIX`,
`line`), which the census parses with, so the two cannot drift apart.
`test-world start --log-every-refusal` sets the variable for its server.

### The tours

`cargo xtask census tours` writes one tour per area from the pinned cache
and the world folder's spawn files. A tour is an ordinary kit script, so any
one of them can also be run by hand against a test world
([test-world.md](test-world.md)).

| tour | area | centre | radius |
| --- | --- | --- | --- |
| lumbridge_castle | Lumbridge | 3215, 3220, 0 | 14 |
| lumbridge_store | Lumbridge | 3212, 3247, 0 | 9 |
| lumbridge_bank | Lumbridge | 3208, 3220, 2 | 10 |
| draynor | Draynor | 3093, 3247, 0 | 14 |
| varrock_square | Varrock | 3212, 3428, 0 | 12 |
| varrock_west_bank | Varrock | 3185, 3438, 0 | 8 |
| varrock_shops | Varrock | 3213, 3410, 0 | 11 |
| varrock_east | Varrock | 3253, 3415, 0 | 12 |
| al_kharid_bank | Al Kharid | 3271, 3168, 0 | 10 |
| al_kharid_town | Al Kharid | 3298, 3186, 0 | 14 |
| falador_square | Falador | 2965, 3380, 0 | 12 |
| falador_banks | Falador | 2946, 3370, 0 | 9 |

Each tour seeds its own character at the centre, with every skill at 60 and
an inventory of tools, food, runes and coins. The steps are, in order:

1. **Idle baseline:** 20 seconds standing still.
2. **Tab buttons** (first tour only): combat styles, music, emotes, quests,
   skill guides, prayers, settings.
3. **NPCs:** every named NPC type spawned within the radius, every option
   its cache definition shows except Examine and Attack (Talk-to,
   Pickpocket, Trade, Bank and so on), then Use bread on it.
4. **Ground items:** Take on each ground spawn within the radius.
5. **Scenery:** for every scenery definition placed within the radius, the
   nearest object, with every option its definition shows except Examine
   and options marked hidden. Varbit-morphed objects take their first
   child's options. Options that usually move the character away (climb,
   enter, cross and similar) come last. A bucket is used on water sources,
   raw shrimps on ranges and fires.
6. **Inventory:** every item option (Wield, Eat, Bury and so on), then four
   use-with pairs (knife, tinderbox and chisel on logs, needle on thread).
7. **Attacks:** Attack on each NPC type that has it, six seconds of
   fighting, then a run back to the centre.
8. One drop (the empty pot).

After each action the tour waits for the walk (scenery and ground items),
pumps frames for a second or three, walks any dialogue it opened to the end
(Continue, else the first option), and closes whatever interface is open.
Tours set `keepGoing`, so a step that fails (an NPC that wandered out of
view, a tile it could not reach) is reported and the tour goes on.

Regenerate the tours after changing `AREAS`, the inventory or the step
template in `xtask/src/census.rs`. `cargo xtask census tours --check`
reports tours that differ from what would be generated.

### What the kit gained

The census added these to the kit's runner (see [test-world.md](test-world.md)):

- `"keepGoing": true` on a script: run every step, even after a failure.
- `"dialogue"` step: answer the open dialogue until it ends.
- `"groundItem"` step: a ground item's option (2 is Take), through the
  shared writer `rs09_protocol::game::menu::ground_item_option`.
- Each step's report has `startedMs` and `endedMs` (Unix milliseconds).
- An extra key on a step, such as the census's `label`, is ignored by the
  runner and kept in the report.

## Reading the report

`runs/census/report.md` opens with a warning when the run does not count
(above), then has four tables.

**Tours:** the agent's exit (1 only means some steps failed; the tours keep
going), steps that passed and failed, and how long each tour took. A
tour with many failed steps reached less, so its area is under-counted.
The agent's own report and stderr for each tour are in `runs/census/agents/`.

**Ranking:** every not-ported refusal the tours hit.

| column | meaning |
| --- | --- |
| kind | `skipped`: a not-ported call that did nothing; `empty answer`: a not-ported query answered with the original's no-content answer; `refused`: an error naming something "not ported in the live world" (the packet or service failed) |
| refusal | the name the server logged. Numbers in error text are replaced by `#` so hits group together |
| credited actions | (tour, action) pairs credited with it (below). This is the rank |
| seen | (tour, action) pairs during which it was logged at all |
| hits | every log line, which favours things called many times per action |
| baseline/min | its rate while the characters stood still; empty when the baselines never saw it |
| e.g. credited to | up to three of the credited actions |

**Every tick:** refusals logged during the idle baseline of at least half
the tours, by their idle rate. (One baseline alone can catch the previous
character's logout, which waits until its last fight is over.) They stay in
the ranking, where an action is credited only for hits beyond that rate.

**Errors:** packet errors (`Error Processing ...`) that name no not-ported
system, service failures (`... failed: ...`) and panics, with the same
columns. Only those two forms and panics are counted; a line that merely
contains the word "error" is not.

`report.json` has the same entries, with the credited actions, every
action that logged each one and how often (`hitBy`), up to three raw log
lines (`examples`), the tour summary and the `problems` behind a warning.

### How a line is attributed

The census follows `server.err.log` and `server.log` while the tours run
and stamps each line with the time it read it (every 40 ms). The runner
reports when each step started. A line belongs to the step running when it
was read, and to that step's action (the label shared by an action and its
follow-up wait, pump, dialogue and close steps). The server answers a
packet on its next tick, up to 600 ms later, and the follow-up steps keep a
line inside its action. A line can still land on the neighbouring action
when it comes from something slower, such as a respawn or a pulse that runs
for several ticks. Lines during the login, before the first step, are
`(login)`. Tours run one at a time, so a line is never another tour's.

So credit is by time window, not causation. Timer- and tick-driven
refusals (the autosave hook, aggression tolerance) are logged during
whatever action was running. To keep them from outranking real gaps, an
action is credited with a refusal only when it was logged then more often
than the idle baseline explains. With the baseline's rate *r* per second
(the higher of that tour's own baseline and all baselines together) and
the action lasting *d* seconds, the expectation is *e* = *r*·*d*, and the
action needs more than 1.5·*e* + 2√*e* + 1 hits; a refusal the baselines
never logged needs one. The factor 1.5 is there because some rates follow
how many NPCs are near (the entity tick hooks, `ZoneMonitor.move`), which
changes as the character walks. Such refusals can still be credited to
each tour's `(login)`, when the previous tour's character is often still
online (its logout waits for its last fight to end). A periodic refusal slower than the baseline (once
every 30 s, say) can still be missed by every baseline and credited to
whatever action it lands in; its *seen* and *hits* against *credited
actions* show that pattern.

`cargo xtask census report` rebuilds the report from a run's saved files
(`census.json`, `server-texts.json`, `server-events.tsv` and the agents'
reports) without
running anything.

## Rerunning as content lands

The census is meant to be rerun as systems are ported: the doors, the bank
booths and chests, commands and interface services. Run it again on the new
branch and compare the rankings. When a refusal leaves the list, the code
that named it is gone or no longer reached by the tours. Add areas or items
in `xtask/src/census.rs` and regenerate the tours when the tours should
reach something new.
