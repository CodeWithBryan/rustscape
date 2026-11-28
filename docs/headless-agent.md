# Headless agent actions and world view

`rs09_headless::agent` lets an agent fight, talk and use items in the live
world (`rs09-server --world`) without a renderer. It has three parts:

- `Action` is what the agent wants to do. `frame()` builds the original
  client's bytes: walk, scenery options, the NPC options (`Attack`, `TalkTo`,
  `NpcOption` for any of the five slots), inventory item options
  (`ItemOption`), use-with on an item, NPC, scenery or player, `Resume`
  (Continue or a dialogue option) and `CloseInterface`.
- `View` folds the server's frames into what the agent reads back: its
  position, skills and hitpoints, inventory and equipment, local NPCs with
  the health bar the server last drew on them, the open interfaces, the
  dialogue in the chatbox, and an ordered event list (game messages, hits on
  NPCs and on the player, an NPC leaving after an empty health bar, experience
  and level changes, item slot changes, dialogues opened and closed).
  `snapshot()` gives the same as JSON. Events are bounded to 512 between
  drains; drops are counted, never silent.
- `Agent` couples a `ManagedSession` with a `View`: `act`, `next_frame`
  (applies the frame first) and `until(limit, what, predicate)`.

```rust,ignore
let session = Session::connect(address, false, "agent", password).await?;
let mut agent = Agent::new(session.into_managed());
agent.until(limit, "the world", |v| v.position().is_some()).await?;

// Fight: attack the nearest chicken and wait for it to die.
let chicken = agent.view.nearest_npc(41).unwrap();
agent.act(&Action::Attack { index: chicken.index }).await?;
agent.until(limit, "the kill", |v| v.events().any(|e| matches!(e,
    Event::NpcRemoved { index, died: true, .. } if *index == chicken.index))).await?;

// Talk: read the dialogue, continue, pick an option by its text.
agent.act(&Action::TalkTo { index: banker }).await?;
agent.until(limit, "a line", |v| v.dialogue().is_some()).await?;
let d = agent.view.dialogue().unwrap();
agent.act(&d.resume().unwrap()).await?;
// ... later: agent.act(&options.choose_text("access my bank").unwrap())

// Use: an inventory item on another.
let (dagger, bowl) = (agent.view.held(1205).unwrap(), agent.view.held(1923).unwrap());
agent.act(&Action::UseOnItem { used: dagger, target: bowl }).await?;
```

`Sender::act` and `Session::act` send an `Action` without a view.

## Shared writers

The NPC option, item option, use-with, Continue and close packets are in
`rs09_protocol::game::menu`. The visual client re-exports them
(`npc_interaction::packet`, `ui::actions::item_packet`, `SelectedItem`'s
targets), so both clients publish the same bytes. Unit tests decode every
writer with the server's `ClientAction::decode` and check the fields
round trip; the Continue bytes equal the live world test's hand-written ones.

## Dialogue layout

The chatbox interfaces the pinned DialogueInterpreter opens:

| interface | dialogue | title | lines | Continue |
| --- | --- | --- | --- | --- |
| 241..244 (245..248 hidden) | NPC | child 3 | children 4.. | lines + 4 |
| 64..67 (68..71 hidden) | player | child 3 | children 4.. | lines + 4 |
| 210..213 | plain message | none | children 1.. | lines + 1 |
| 228, 230, 232, 234 | 2..5 options | child 1 | children 2.. | none; option *i* is child *i* + 2 |

The NPC's id comes from the head model packet on child 2. The dialogue is the
one whose interface is open in a component; a close of that component or
another interface opened there ends it.

## Live proof

`crates/headless/tests/live_agent.rs` boots the live world in-process (as
`crates/server/tests/live_world.rs` does) and drives three characters only
through `Action` and `View`:

- with attack and strength 40 at Lumbridge's chicken pen, attacks the nearest
  reachable chicken: hit marks with health bars, the NPC removed after an
  empty bar, hitpoints experience;
- at Draynor bank, talks to the banker (494): the NPC's greeting, Continue,
  the four options, "access my bank account" chosen by text, the player's
  line, Continue, the bank (762) opens and the chatbox closes;
- in Lumbridge castle's kitchen, uses a dagger on a bowl ("Nothing
  interesting happens.") and wields the dagger (equipment slot 3).

```sh
cargo xtask cache-fetch
cargo test --release -p rs09-headless --test live_agent -- --nocapture
```

It needs `assets/cache` and the world data (`runs/startup-loaders-oracle/template`
or `RS09_STARTUP_WORLD`, e.g. `runs/world-trace-oracle/template`); without them
it prints SKIPPED. On Linux the live world now writes world-relative paths
into its configuration, because `parsePath` drops a leading `/`.

## Scripted runs

[test-world.md](test-world.md) runs these actions from a JSON script against
a disposable `rs09-server --world` (the `agent_run` example), with characters
seeded before login and waits on the view.

## Not covered

- Use on an NPC, scenery or player sends the right bytes (unit tests). The
  live world now walks to the target and answers "Nothing interesting
  happens." (`UseWithHandler`'s MovementPulse, checked with raw packets by
  the server's `live_world` test), but `live_agent` does not drive it yet.
- The view has no cache definitions: no NPC or item names, and no option
  labels. `Attack` is option slot 1 and `TalkTo` slot 0, as on ordinary
  monsters and humans; other NPCs need `NpcOption` with the right slot.
- Ground items, scenery, other players' state and projectiles are not in the
  view (`ground::Observer` and `scenery::Observer` remain separate).
- Hits are read from NPC and player update masks; the view does not simulate
  combat or predict death.
