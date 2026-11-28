# Native dialogue interpreter, plugin and file contracts

`world/dialogue/` ports DialogueInterpreter, the DialoguePlugin and DialogueFile
base classes, EmptyPlugin, Topic/IfTopic, SimpleDialoguePlugin, FacialExpression
and the DialogueAction interface. It is a library. `rs09-server --world`
installs it (`world/live/talk.rs`: the live world is the `Host`, one
interpreter per player, Continue through `interface_packets`; see
[the live world](live-world.md)); the controlled examples do not, and no
NPC, quest or skill dialogue content is ported here. DialogueBuilder, DialogueLabeller,
SkillDialogueHandler, SkillDialoguePlugin, ActivateEnchantedGem and the banker
conversation are ported in `world/dialogue_dsl` (see
[dialogue DSLs](dialogue-dsl.md)). Whole-source parity rows remain `not_replaced`.

Plain `sendDialogue` and the 54-unit line splitter are not reimplemented: the
interpreter calls the verified `plain_dialogue::send_lines` and
`plain_dialogue::split_lines`. See [plain dialogue](plain-dialogue.md).

## Original comparison

The 1586-case JVM corpus (5406 per-action snapshots) compiles the complete
unchanged DialogueInterpreter.java, DialoguePlugin.java, DialogueFile.kt,
EmptyPlugin.kt, DialogueTopic.kt, FacialExpression.java, DialogueAction.java,
SimpleDialoguePlugin.java, DialUtils.kt, DialogueConst.kt, Event.kt and the
plugin annotations/interface with JDK 11 and Kotlin 1.8.20. The three dialogue
event classes are copied verbatim from Events.kt at generation time.
`oracle/dialogue/initial.lock.json` pins all 1586 initial observations.

Player, NPC, Entity, Component, InterfaceManager, PacketDispatch, Item,
ItemDefinition, NPCDefinition, GameWorld settings, PacketRepository, logging
and System.err are recording stand-ins in `oracle/dialogue/Standins.java`.
Scripted plugin and file classes in `Harness.java` extend the unmodified base
classes and call only inherited methods. Reflection observes private fields and
empties the static PLUGINS map between scenarios. Each snapshot holds the
ordered service calls of that action, every interpreter's dialogue, key,
actions and active topics, every plugin's stage/finished/file/NPC/player/
interpreter/option lists, every file's stage and back-references, attributes
and chatbox components. Native replay drives real typed `PluginBody`/`FileBody`
objects; labels are diagnostic output and never select behaviour.

This corpus does not establish packet bytes, real interface lifecycle, real
definitions, concrete content or a live session. Those are the stand-ins.

## Preserved behaviour

- `handle` stores `chatbox-buttonid`, then tests END_DIALOGUE through the
  player's current interpreter, not the receiver: the file's virtual
  `getCurrentStage` first, then the plugin stage. A missing dialogue fails after
  the attribute is stored. The end branch closes and reopens chatbox 137, marks
  that dialogue finished and calls `close`.
- With active topics and a button of two or more, the topic at `button - 2` is
  spoken unless it skips the player; a file target is loaded through the current
  dialogue, a stage target goes to the file captured before the topic ran or to
  the plugin; the topic list is cleared and a skipping topic re-enters `handle`.
- Otherwise the selection event carries `button - 1` and the file's virtual
  stage or the plugin stage; the plugin is re-read after the event is dispatched.
- `close` clears queued actions, then returns true without closing when the
  chatbox component has a close event, leaving the dialogue and active topics in
  place. Otherwise the dialogue field is cleared before the plugin's `close` and
  the close event; a failing plugin close leaves no dialogue and no event.
- `open(key)` records the key first. An NPC first argument gets the dialogue
  player, then queue reset and pulse clear inside one caught block: a failed
  reset also skips the clear and only prints. Empty arguments become the key.
  The previous dialogue is overwritten, not closed, and survives a failing
  `newInstance`. A plugin that ends itself during `open` still reports success.
- `sendDialogues` uses interface 240 plus the count for an NPC id above -1 and 63
  plus the count for a player, plus four when Continue is hidden; expression -1
  becomes HALF_GUILTY. The Entity forms without `hide` use the NPC shown to the
  interpreter's player; the forms with `hide` use `entity.getId()`. Counts
  outside 1..4 print and return nothing. The previous chatbox's child 3 is
  unhidden before the new chatbox opens.
- `doSubstitutions` replaces `@name`, then `@servername`, then `@g[male,female]`
  with Matcher.appendReplacement semantics: the chosen group is a replacement
  string, so a backslash escapes and `$n` references a group. `isMale` is read
  even without a gendered form.
- Item messages do not range check the count. Single items halve the zoom with
  integer division; double items divide by 1.5 and truncate. Definition fields
  are read late, after the item packet. `sendOptions` accepts two to five
  options; `sendDestroyItem` breaks a message longer than 200 units at the
  middle word with Java `split(" ")` rules.
- DialoguePlugin helpers read `getIds()` twice when no NPC is set; `showTopics`
  with one valid topic follows the first entry of the interpreter's active list,
  which can be older. DialogueFile's form instead takes the first argument whose
  text equals it, which can be a filtered-out IfTopic, and its helpers pass a
  copy of the message array where the Java plugin helpers pass the array itself.
- No field guard spans a host or content call. Reentrant close, open and
  replacement of the dialogue from callbacks match the JVM.

## Required services

`dialogue::Host` lists every call the classes make outside their package:
player getters (interpreter, packet dispatch, interface manager, username,
gender, debug, node id), messages, attributes and event dispatch; each
PacketDispatch method used; chatbox open/close/lookup, component id and close
event presence; NPC id, shown NPC, dialogue player, walking queue and pulse
manager; NPC and item definitions, Item construction and fields; the world
name; and the log, System.err and printStackTrace diagnostics. None has a
default. Content supplies `PluginBody` and `FileBody` objects.

## Differences from the original

- The static PLUGINS map is an explicit `Registry` handle the owning world
  creates and passes to each interpreter, not process-global state.
- The reflective base `newInstance` is not ported. Content implements
  `PluginBody::new_instance`; `Ok(None)` is the null the reflective body returns
  after printing a failed construction.
- Only `handle`, `ids`, `new_instance`, `open` and `close` are overridable on a
  plugin, and `handle`, the stage property, `getCurrentStage` and `end` on a
  file. The other helper methods, overridable in Java/Kotlin, are not virtual.
  `fireEvent`, `handleSelectionCallback` and the RED/BLUE/option-interface
  constants are absent.
- FacialExpression and int overload pairs are one function taking the animation
  id; callers convert with `FacialExpression::or_unset`. `sendOptions` takes the
  already converted title. `sendDestroyItem`'s unused message is dropped.
- Message arrays are shared `Lines` handles. A null array is representable only
  for `send_dialogue`; the other forms take a non-null array.
- An open argument that is neither null, Integer, Boolean, String, NPC nor a
  file carries only its class simple name.
- IllegalArgumentException messages from `${...}` group references are not
  reproduced exactly; the failure class is. Line splitting inherits the
  explicit rejected boundaries of [plain dialogue](plain-dialogue.md).
- The caught block in `open` catches every native error; Java `Error`
  subclasses that escape `catch (Exception)` are not modelled.

## Remaining parity work

Concrete dialogue content other than the banker conversation and Otto
Godblessed's (the DSLs are
[dialogue DSLs](dialogue-dsl.md)). Binding `Host` to the
canonical player, InterfaceManager, real packet writers, definitions, NPC
queue/pulse state, attributes and events. Joining Continue (`interface_packets`)
and the modern context services to this interpreter. Plugin, file and
interpreter objects hold strong back-references like the original fields;
they are not yet traced by the canonical capture graph, so detached
plugin/file cycles are not collected. No live session, performance or
capacity claim is made.

```powershell
cargo test -p rs09-server --test dialogue --test plain_dialogue --test input_prompt --test interfaces
cargo xtask dialogue-oracle --check --java-home runs/jdk11/jdk-11.0.32.1+1
```
