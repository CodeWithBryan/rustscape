# Player requests and trading

`world/trade` ports the request system and the trade: who may ask whom, how a
request is accepted by asking back, both trade screens, offering and removing
items, the acceptance reset and the "modified" warning, the space checks, the
final exchange, and declining, closing or logging out at any stage. It is a
library. `rs09-server --world` installs it (`world/live/trading.rs`, see docs/live-world.md): requests from the Trade with option, the plugin on 334/335/336, the close event and the alert pulse; AssistSession and the bots are refused there. Java is a test oracle; nothing here runs it.

| Original | Rust | Scope |
| --- | --- | --- |
| `RequestManager.kt` | `trade/request.rs` | `request`, `canRequest`, `acceptExisting`, `close`, `clear` |
| `RequestType.kt`, `RequestModule.kt` | `trade/mod.rs` (`RequestType`), `request.rs` | TRADE and ASSIST, their two messages, `getRequestMessage`, the base `canRequest`, `module.open` dispatch |
| `TradeModule.kt` | `trade/module.rs` | every member: `open`, `update`, `display`, `displayModified`, `alert`, `decline`, `nextStage`, `openMain`, `openSecond`, `setAccepted`, `displayOffer`, `getDisplayMessage`, `hasSpace`, `getFormattedNumber`, `giveContainers`, `addContainer`, `splitList`, `incrementStage`, `extend`, `getExtension`, the companion components and run-script parameters |
| `TradeContainer.java` | `module.rs` | `offer`, `withdraw`, `validatedItem`, `stabalizeAmount`, `tradeable`, `canUse`, `checkCapacity`, `alert`, `checkAlert`, the alert pulse body, `TradeListener.update`/`refresh` |
| `TradeCloseEvent.java` | `module.rs` (`close_event`) and `TradeClose` | every member |
| `TradeInterfacePlugin.java` (content/global/handlers/iface) | `trade/plugin.rs` | `handle` for 334, 335 and 336, and both Offer-X/Remove-X handler lambdas (`AmountScript`, `run_amount`) |
| `Location.withinDistance`/`getDistance` | `mod.rs` (`Location::within_distance`) | the Int arithmetic that wraps |
| `Globals.colorize` | `module.rs` (`OWN_BOT`) | the one string the trade colours |

`RequestOption.kt` (the "Trade with" player option) is one line that calls
`request(target, RequestType.TRADE)`; it is not ported as a listener.

## What is joined to native pieces

- Containers: each module's TradeContainer is a native `Container` (DEFAULT,
  ID sort, 28) carrying a native `TradeListener` object; every add, remove,
  shift, `hasSpaceFor`, `getMaximumAdd`, refresh and update runs the native
  container code, and the player's inventory is the native container the host
  hands over. Other listeners on those containers are host calls.
- InterfaceManager and close events: every `open`, `close`, `closeChatbox`,
  `openSingleTab`, `closeSingleTab`, `closeDefaultTabs`, `restoreTabs` and
  `openDefaultTabs` is the native `world::interfaces` code with a trade
  `interfaces::Context` (`trade/ui.rs`). 335 and 334 carry a native
  `TradeClose` close event; the manager's close calls back into
  `module::close_event`, including the reentrant close of the partner.
  336, 335 and 334 carry the native `TradePlugin`; its open is the inherited
  empty body. The services the manager needs besides these (component lookup,
  listeners, events, weapon/friends tabs, runscript attribute...) are host calls.
- Ironman: `tradeable` asks the native `ironman::Manager.is_ironman()`.
- Item definitions: tradeability is the native `definition_tradeable` on the
  native definition; names come from native items.
- Input prompts: Offer-X/Remove-X give the host an `AmountScript` (a native
  object capturing the module and slot); the replay hands it to the native
  `input_prompt::send` and answers the input with the native
  `input_prompt::receive`/`process`, whose `invoke` runs `plugin::run_amount`.
  A failed `Integer.parseInt` is `Error::Number`, which the native prompt turns
  into "That number's a bit large, don't you think?".
- Packets: the trade builds its frames with the native protocol writers
  (`interface_text`, `interface_events`, `run_client_script`,
  `interface_hidden`, `inventory_full`); the interface manager builds its own.

## Behaviour that matters for gameplay

- A request needs: another player, within 15 tiles on the same plane (the
  squares are Int arithmetic: 65,536 tiles away squares to zero and counts as
  near; 46,341 overflows and counts as far), an active target without a main
  interface open, neither side `busy` past the current tick, and the zone's
  consent. Failures say "Unable to find NAME." or "Other player is busy at the
  moment." The requester's own open interface does not refuse: it is closed.
- There is no timeout. `lastRequest` stays on the requester and the manager's
  target stays set until someone accepts, so a request from long ago is still
  accepted by asking back; asking someone else replaces the target. Asking
  back with the other type sends a new request. Acceptance closes the
  accepter's dialogue, interface and chatbox, clears both targets, makes both
  players `busy` for two ticks (`ticks + 2`, wrapping) and opens the trade (or
  hands ASSIST to the host).
- A request to an artificial player whose bot runs DoublingMoney tells the bot.
  The exchange asks `PulseRepository` for both names unconditionally, and a bot
  entry whose script is not DoublingMoney fails with ClassCastException
  before anything moves.
- Offering: the slot must hold an item, the slot may be 0..28 (28 passes the
  check and finds nothing), the amount at least 1. Ironmen on either side
  trade only ids 11174, 11173, 759, 1586 and 1577 (even when those are
  untradeable). If either side is marked not on the hiscores and IP, MAC or
  computer name match, the item is refused with the red bot message. An item
  named "Coins" that is not 995 is refused. Otherwise the definition decides
  (a destroy/dissolve option refuses, except impling jars). Dev mode lets any
  refused item through, after the checks have run. The amount is clamped to
  what the inventory holds and to what the trade container can take; zero
  room says "You must accept the current items before adding any more."
- Any change while either side has accepted resets both acceptances. Removing
  an item sets "modified" for good: varp 1042/1043 on the first screen, the
  red text on the second, and an alert pulse for 8 ticks after the removal.
- Offer-X takes k/K as thousands only; Remove-X also takes m/M. "1m" offered
  is a NumberFormatException, shown as "That number's a bit large". Closing
  the trade removes the pending prompt. The prompt captures the module of the
  moment, not the player's later one.
- Both accepting the first screen checks space both ways; the receiver's
  check copies its inventory and adds the offer, so a stack near the Int
  maximum passes and the overflow is clamped off at the exchange: coins over
  2,147,483,647 vanish.
- The exchange (both accepting the second screen) logs the trade, tells the
  bots, then gives each side the other's items in container order. Each item is
  first clamped to the receiver's maximum add, in place in the trade container;
  what the inventory then refuses becomes a ground item with that clamped
  amount, which is 0 when the receiver had no room at all. Nothing undoes a
  partial exchange: a failure part-way leaves what was added added.
- Declining, closing, logging out or opening another interface on either
  screen gives both sides their own offers back (once each), closes both
  sides, clears varps 1042/1043 and reopens the default tabs. After the
  exchange the screens close without returning anything.
- Examining an empty slot fails with NullPointerException.

## Evidence

`cargo xtask trade-oracle` compiles the six request/trade classes,
TradeInterfacePlugin, RunScript.kt, InputType, Globals, IronmanManager and
IronmanMode, MapDistance, Item, Definition, Container and its event/listener/
type/sort classes, Component, ComponentDefinition, ComponentPlugin,
CloseEvent, InterfaceType, Plugin, Initializable, the complete
InterfaceManager, ClientInfo, IoBuffer and nine packet writers with their
contexts, all unchanged, with Kotlin 1.8.20 and runs them on JDK 11, each case
in a fresh class loader so TradeModule's shared components start new. The
original text of the members listed in the fixture's `scope` is inserted into
recording adapters at generation time. The pinned interface_configs.json is
loaded in both runs.

The corpus has 373 cases with 2,823 actions (950 requests, 1,307 button
presses, 81 prompt answers), in which the original makes 106,813 recorded
calls, 67,157 of them frames compared byte for byte, and completes 86
exchanges, with 13 injected failures. After every action it records the
result or the exception class, the calls in order, and the state: each
inventory slot by slot, the interface manager's opened, single-tab, chatbox
and overlay components, the tab array and packet counter, the extension and
request target, the attributes (busy, lastRequest, alert, alertSlot,
runscript, input-type, parseamount), every trade module ever made with stage,
flags and container contents, pending alert pulses and the tick.
`crates/server/tests/trade.rs` replays every case on the native library and
compares all of it.

```powershell
cargo test -p rs09-server --test trade
cargo xtask trade-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
```

The generator never downloads. It needs the pinned Kotlin jars in
`runs/kotlin-1.8.20` and the pinned json-simple jar in `runs/trade-oracle` or
in any other `runs/*-oracle` folder (it checks the hash).

## Required services

`trade::Host` lists them, with no defaults: the tick, dev mode, definitions,
the container factory and the shared components; user name (the original
formats it with `formatDisplayName`), location, active and artificial flags,
inventory, Ironman manager, interface manager and the three login details;
the integer/boolean attributes, `lastRequest` and the request target; the
TradeModule extension; `PacketDispatch.sendMessage` (with its filter and
truncation), `ContentAPI.sendMessage`, `setVarp`, the session write;
`DialogueInterpreter.close`; `ZoneMonitor.canRequest`; the bot repository and
DoublingMoney; AssistSession (content); `PlayerMonitor.logTrade`;
`GroundItemManager.create`; the alert pulse's submission (its body is
`module::alert_pulse`); `ContentAPI.sendInputDialogue`; the examine text; other
container listeners; and the InterfaceManager services listed in `ui.rs`.

## Differences from the original

- Failures are categories (`Error`); JVM exception messages are not compared.
- `Container(itemCount, ALWAYS_STACK).addAll` in addContainer is performed and
  discarded, as in the original; the native container bound of one million
  slots applies.
- The display name's first letter is upper-cased with Rust's Unicode tables;
  only ASCII names are compared.
- `RequestType.TRADE`'s placeholder module (`TradeModule(null, null)`) is not
  built; its stage, always 0, is a constant.

## Not covered

Pulse scheduling of the alert (only its body), the real InventoryListener (a
recorder stands in), GameMessage bytes, examine text, interaction locks and
the zone/listener checks before a button reaches the plugin, the "Trade with"
option's walk to the player, AIPlayer bots, the assist session and the duel
arena (content), and concurrent trades on two threads.

## What the next steps need from this

The packet-dispatch step needs: PLAYER_ACTION "Trade with"/"Req Assist" to
call `request::request`; IF_ACTION on 334/335/336 to find the component with
`interfaces::get_component`, apply processIfAction's dialogue/lock/zone/hidden
checks and call `plugin::handle` when the component's plugin is `TradePlugin`;
the input-prompt response to go through `input_prompt::receive` with an
`invoke` that runs `plugin::run_amount` for an `AmountScript`; the close-
interface packet and Player.finishClear's `interfaceClose`/`closeSingleTab`
to use the trade-aware `ui::Ui` so the trade's close event runs. The world
must call `register_plugin` and `components` once after the interface
configuration loads. The tick-driver step needs to run `module::alert_pulse`
one tick after `Host::submit_alert` (Pulse(1, player), dropped if the player is
gone) and to keep the tick the trade reads equal to `GameWorld.ticks`.
