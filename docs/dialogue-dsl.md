# Native dialogue DSLs and the banker conversation

`world/dialogue_dsl/` ports the rest of `core/game/dialogue/`: the DSLs most
content dialogues are written in, on top of the native interpreter in
`world/dialogue/` (see [dialogue](dialogue.md)).

- `labeller.rs`: DialogueLabeller (labels, `goto`/`loadLabel`, `exec`,
  `manual`, the NPC/player/item/line stages, `options` with DialogueOption,
  `input`, `afterClose`, the `open` stage and `openDialogue`), DialogueOption
  and the companion `DialogueLabeller.open(player, dialogue, npc)`.
- `builder.rs`: DialogueBuilderFile, DialogueBuilder, DialogueClause, every
  node class (NpcL, Npc, Item, PlayerL, Player, BetweenStage, ManualStage,
  ManualStageWithGoto, Placeholder, Goto, Options, OptionsDispatch, Branch,
  EndWith), DialogueOptionsBuilder and DialogueBranchBuilder.
- `skill.rs`: SkillDialogueHandler with its six SkillDialogue types
  (display, getAmount, getIndex, forLength) and SkillDialoguePlugin.
- `gem.rs`: ActivateEnchantedGem.
- `api.rs`: the ContentAPI functions these call (`openDialogue`, both
  `sendInputDialogue` overloads) and `Component.setUnclosable`;
  `face(npc, location)` is inlined in `DialogueLabeller.open`.
- `banker.rs`: BankerNPC's `BankerDialogueLabellerFile` and the companion's
  `canAccessBank`, written against the native labeller, plus `talk_to`, the
  "talk-to" listener body.

DialogueTopic.kt (Topic/IfTopic) was already ported in `world/dialogue`.

## Original comparison

`cargo xtask dialogue-dsl-oracle --check` compiles, with JDK 11 and Kotlin
1.8.20, the unchanged DialogueLabeller.kt, DialogueBuilder.kt,
SkillDialogueHandler.kt, SkillDialoguePlugin.java, ActivateEnchantedGem.java,
the dialogue core they run on (DialogueInterpreter, DialoguePlugin,
DialogueFile, EmptyPlugin, DialogueTopic, FacialExpression, DialogueAction,
SimpleDialoguePlugin), DialUtils, DialogueConst, core.api.InputType, Event,
EventHook, CloseEvent, IronmanMode and the plugin annotations, with the
published ConstLib 1.4.28. Original text is inserted into adapters at
generation time: ContentAPI's `face`, `openDialogue`, both
`sendInputDialogue` overloads and SecondaryBankAccountActivationResult;
`Event.DialogueClosed`; Entity's hook map, `dispatch`, `unhook` and both
`hook` overloads; `Component.setUnclosable`; `StringUtils.formatDisplayName`;
and BankerNPC's `ODOVACAR_FEE`, `canAccessBank` and the whole
`BankerDialogueLabellerFile`. Player, NPC (with `new NPC(id)`),
PacketDispatch, InterfaceManager, Component, Item, ItemDefinition,
NPCDefinition, GameWorld, PacketRepository, the bank/ironman/Grand
Exchange/message ContentAPI members, SlayerManager, OptionHandler, Quests,
logging, `System.out.printf` and `System.err` are recording stand-ins.
`ServerConstants.SECOND_BANK` is an unrecorded field, as in the original.

Scripted DialogueLabeller, DialogueBuilderFile and SkillDialogueHandler
subclasses (`oracle/dialogue-dsl/Script.kt`) interpret JSON scripts by
calling only original DSL members; `native.rs` interprets the same scripts
against the port and drives the real native interpreter, so labels never
select behaviour.

The corpus has 3,600 cases: 154 designed scenarios (35 banker, 45 labeller,
30 builder, 41 skill, 3 gem) and 3,446 injected-failure cases (961, 1,284,
713, 481, 7): in each marked action
every recorded call fails once in turn. A failure case keeps only the swept
action's snapshot (its prefix is the designed case). 11,372 actions, 355,964
recorded calls. 33 deliberate breaks of the port (jump limit, stage guards,
option button ranges and jumps, afterClose's jump check, input scripts,
formatting, builder dispatch/branch/between/option limits and END, skill
index/amount/order/parse, openDialogue's close and log, setUnclosable,
hook order, banker conditions, fee and activation mapping, the gem message)
all fail the replay; four first survived and gained cases (afterClose
with a jump, an exec after a pending jump, the single builder option's
button, amount prompts reaching the parse). Each snapshot holds the ordered calls, the result and error
class, the interpreter's dialogue/key/topic count, every file's
DialogueFile stage, player/NPC/interpreter references and either the
labeller fields (own stage, counter, starting stage, stage hit, jump,
button, option button, input, filtered option labels, label map) or the
builder's clause structure (node classes and their target stages), the
plugins seen, attributes, chatbox, components and close events, the number
of dialogue-close hooks and the secondary bank flag.

Covered: every banker branch (Sirsal with and without a seal, ultimate iron
man for Jade and others, Jade, the bank tutor's three explanations, the
Bounty Hunter banker with and without SECOND_BANK, Odovacar paying, unable
to pay, paying later and refusing, Grand Exchange collections, access, PIN,
collect, buying a secondary account with all four activation results,
switching accounts both ways, a female player's `@g[...]`), closing
mid-conversation, a null location, no NPC, out-of-range buttons; every
labeller stage form with and without `unclosable` and its close event,
labels with nesting, missing labels, a self jump and the eleven-pass
ping-pong limit, options with expression/spoken/skip/conditions, one or no
visible option, six options, every input type with numeric and text
answers, afterClose, the `open` stage to a file and to a key,
`openDialogue` from `exec`, a failing exec; every builder node, quest-stage
and attribute predicates, a clause that never matches, options with
recorded attributes and filtered/one/zero/six entries, placeholder loops
and an unset placeholder, Int and Boolean branches with a wrong type,
`betweenStage`, `manualStage`, `manualStageWithGoto`, `endWith`, a chain
before any predicate; each SkillDialogue type opened directly and through
`openDialogue`, the amount/index table of every type for buttons 0..26,
amount prompts answered with numbers, numeric text and bad text, getAll and
getName overrides, a failing create, a null type, too few items, a sixth
item for FIVE_OPTION, null item names, an unregistered key, the key opened
without a handler and handled before opening; the gem with and without a
slayer task and its registration.

## Preserved behaviour

- DialogueLabeller keeps its own `stage` (the pass being run) beside
  DialogueFile's field (`super.stage`, the next stage). The first `handle`
  runs `addConversation` with stage -1 to record labels and the starting
  stage (1 when a label comes before any stage, else 0); then up to eleven
  passes follow jumps, stopping when no jump is set or a jump targets the
  stage just run. A pass that hits no stage ends the file. A missing label
  leaves no jump.
- A stage runs only when the pass's stage equals the counter and no jump is
  pending; it increments `super.stage`, sets `stageHit` and, when
  unclosable and a component came back, calls `setUnclosable` (attribute
  `close_c_` then the close event). One message is split at 54 units, more
  are sent as given; `line` never splits; `item` splits `message` when no
  messages are given.
- `options` filters on each pass that runs it (conditions read the player
  then the NPC, both required), jumps directly with one visible option and
  sends nothing for zero or more than five. The spoken stage uses the button
  of the current pass (`buttonID in 1..size`), a skipping option jumps to
  `stage + 1`, and the following pass jumps to the label of `optButton`
  without checking for a pending jump. Kotlin evaluates `open`'s arguments
  (including a new file) on every pass, not only the active one.
- `input` sends script 109 for AMOUNT and STRING_SHORT, 108 for NUMERIC, 110
  for STRING_LONG and 107 for MESSAGE, sets `parseamount` for AMOUNT, then
  a runscript that stores `optInput` and calls the interpreter with the
  chatbox id and button 2, then `input-type`.
- `afterClose` hooks only right after the stage hit in this pass with no
  jump pending; the hook unhooks itself before its callback, and the
  player dispatches a close event over a copy of its hooks.
- `DialogueLabeller.open` faces the player's location (null fails the
  Kotlin parameter), sets the dialogue player, resets the walking queue,
  clears the pulses, then `openDialogue`, which closes the player's
  interpreter first and logs any object it cannot open.
- DialogueBuilderFile builds every clause in its constructor. `handle` runs
  the first clause whose predicate holds (the player is required for each
  predicate) and ends on END_DIALOGUE, so a node that re-enters `handle`
  can end twice. A negative stage fails the node lookup; a stage past the
  end is END. Options read every entry's predicate; one visible entry
  re-filters and re-enters with button 0; more than five prints and ends;
  the dispatch stores `buttonID - 1` before indexing the re-filtered list.
  A branch without the value ends. `betweenStage` returns `stage + 2`.
  `iteml` splits while building, `linel` when the stage runs.
- SkillDialoguePlugin casts its first argument, displays it, then reads the
  interface manager before the type (a null type fails after that read);
  `handle` reads amount and index, ends, then creates or prompts for an
  amount whose answer is parsed with Integer.parseInt. Each type's display
  order, zooms, child positions and the null name printed as "null" are
  kept; TWO_OPTION and FIVE_OPTION open chatbox 306 first and call the
  overridable `getName`.
- The banker reads `npc.id` once per comparison, checks the seal before the
  iron man restriction, evaluates option conditions in declaration order and
  short-circuits like Kotlin; Odovacar builds the 100-coin Item before the
  removal.

## Required services

`dialogue_dsl::Host` extends `dialogue::Host` with: the player's location
and an NPC's `faceLocation`; `new NPC(id)` and `new Item(id, amount)`;
typed attribute writes (Boolean, Int, InputType, runscript) and a Boolean
read; a component's close event; hooking and unhooking the dialogue-close
hook; quest stages; `System.out.printf`; `player.debug`; item names; item
zoom and child repositioning packets; inventory amounts; PacketDispatch
messages; the slayer task flag; and the gem's item option registration.
`banker::BankHost` adds the ContentAPI bank, iron man, Grand Exchange and
message calls and `ServerConstants.SECOND_BANK`. None has a default.

## Differences from the original

- Content implements `labeller::Conversation`, `builder::file`'s create
  closure and `skill::HandlerBody`; DSL calls take `&mut H` explicitly.
  Kotlin default arguments become explicit parameters (`npc(chatAnim, npcId,
  ...)`'s `npc!!.id` default is the caller's to evaluate).
- The deprecated throwing members (`playerl`, `npcl`, `iteml`, `linel` and
  `options(String...)` of DialogueLabeller) are not ported; they only throw.
- The debug graph printer (`DEBUG_DIALOGUE` is false) and the node
  `toString`s are not ported; `builder::structure` is an observer.
- `ManualStageWithGotoNode`'s unused second function is not taken.
- The runscript value is `input_prompt::Response` (Int or Text); the
  original's cast failure for any other type cannot arise.
- A skill handler travels to its plugin as `dialogue::Arg::Object` (a new
  variant of the existing open-argument enum).
- `getName`'s display form works on the name as a Rust string (lossy for
  unpaired surrogates).

## Remaining work

Nothing installs these libraries in `rs09-server --world`. The banker
conversation replaces npc_classes' `banker_conversation` service: a live
binding answers `npc_classes::Host::banker_conversation(player, npc)` with
`dialogue_dsl::banker::talk_to(host, player, npc)` once the world host
implements `dialogue::Host`, `dialogue_dsl::Host` and `banker::BankHost`
over the canonical player, interfaces, packets, attributes, events and
bank. The input op of the corpus invokes the stored runscript directly, not
through RunScript.processInput (`input_prompt`); no other content dialogue
is ported. The activity and cutscene framework (ActivityManager,
ActivityPlugin, CutscenePlugin, Cutscene) is not ported: it needs
DynamicRegion/RegionBuilder (not native), pulses submitted to the world,
MinimapState, tab removal/restore, locks, camera packets, logout listeners
and AntiMacro pause.

```powershell
cargo test -p rs09-server --test dialogue_dsl --test dialogue --test npc_classes
cargo xtask dialogue-dsl-oracle --check --java-home runs/jdk11/jdk-11.0.32.1+1
```
