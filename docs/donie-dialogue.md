# `DonieDialogue`, `DonieDialogueFile`, `DonieDialogueShoelaceFile`

The original Donie dialogue (`content/region/misthalin/lumbridge/dialogue/DonieDialogue.kt`), ported to
`world::content::lumbridge::donie_dialogue` and checked against its own JVM corpus (`oracle/donie-dialogue`,
`fixtures/donie-dialogue`). Laid out by `tools/content_scaffold.py dialogue`; the recipe is docs/content-porting.md.
It is the kit's first port.

## What is ported

- `DonieDialogue`, a DialoguePlugin for `NPCs.DONIE_2238`: its ids, `newInstance` and `handle`, which calls
  `openDialogue(player, DonieDialogueFile(), npc)` and answers false.
- `DonieDialogueFile`, a DialogueLabeller: the greeting, the three options ("Where am I?" with THINKING, "How are you
  today?", "Your shoe lace is untied." without the player's line), the "whereami" and "howareyou" sections and the
  "shoelace" stage that opens `DonieDialogueShoelaceFile` with `open(player!!, DonieDialogueShoelaceFile(), npc!!)`.
- `DonieDialogueShoelaceFile`, a DialogueLabeller of four lines.

The live world registers the plugin at boot (`Live::install_ported`, ClassScanner's DIALOGUE branch in the class
scan's order), so Talk-to on Donie (NPCTalkListener's `DialogueInterpreter.open(2238, npc)`) opens it instead of
answering false. The live_world `donie` scenario talks to her over the protocol: greeting (241), the options, the
player's line (64), her two-line answer (242) with the world's name.

## The corpus

`cargo xtask donie-dialogue-oracle --check` (Python, JDK 11, the pinned reference and ConstLib 1.4.28) compiles the
source unchanged over the dialogue content kit and reproduces `fixtures/donie-dialogue`; `cargo test -p rs09-server
--test content_donie_dialogue` replays it natively. 399 cases: 16 designed, 383 injected failures (every recorded call
of each swept action failing once in turn); 1,584 actions, 42,917 recorded calls.

The designed cases: the greeting; the options; each of the three branches to the end and past it; the weather line
with another world name and with no settings; a female player; Talk-to before registration (false); closing at the
greeting and at the options, then continuing; opening without an NPC; an out-of-range button; registration alone.

What the original does that the source does not make obvious, kept by the port:

- Labels end a section: "Where am I?" ends after Donie's answer, it does not fall into "How are you".
- `GameWorld.settings?.name ?: "2009Scape"` is evaluated on every labeller pass, but in a world with no settings the
  interpreter's own `getSettings().getName()` fails first, so the `"2009Scape"` default is never shown.
- `open(player!!, DonieDialogueShoelaceFile(), npc!!)` evaluates its arguments on every pass (a new file each time).
- Opened without an NPC, `handle` fails at the call site's null check on the platform-typed `npc` field, before
  `openDialogue` closes anything.

## Services

The calls the scaffold found outside the dialogue library:

- `GameWorld.settings?.name`: `content::Services::settings_name` (shared). The recording host answers it from the
  case (`serverName`, `settings: null`); the live world answers `SERVER_NAME` (it always has settings).

Donie's own `Host` trait is empty.

## Not covered

The corpus runs over the dialogue-dsl recording stand-ins: no packet bytes, real interfaces or definitions. The live
scenario checks one path (How are you) over the protocol, not every branch. NPCTalkListener's checks before the open
are world/npc_classes (npc-classes-oracle).
