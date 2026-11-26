# Shops

`world/shops` ports the shop engine: building every shop from `shops.json`,
each shop's stock and the player's view of it, the two shop interfaces (620
stock, 621 inventory side), buying and selling with the original price
formulas, the general stores' shared player stock, restocking and decay over
ticks and the periodic clear of the general player stock. It is a library.
`rs09-server --world` installs it (`world/live/shopping.rs`,
[the live world](live-world.md)): the start-up at boot, the tick listener,
the NPC listeners and the destination override in the native
InteractionListeners, the 620/621 listeners and the amount prompts. Java is
a test oracle; nothing here runs it.

| Original | Rust | Scope |
| --- | --- | --- |
| `core/game/shops/Shop.kt` | `shops/shop.rs` | `ShopItem`; `ShopListener` (`view`, `update`, `refresh`); every `Shop` member: construction and `init`, `openFor`, `showTab`, `getContainer`, `getVisibleStockItem`, `generateStockContainer`, `restock`, `getBuyPrice`, `getSellPrice`, `getGPCost`, `getGPSell`, `buy`, `sell`, `getStockSlot`; the companion `generalPlayerStock`, `listenerInstances` and `fixedPriceItems`; `TransactionStatus` |
| `core/game/shops/Shops.kt` | `shops/controller.rs` | `startup` and `parseStock`, `tick`, `openId`, the four NPC "trade"/"shop" listener bodies, the destination override, the four interface listeners (`on(620)`, `onOpen(621)`, `onClose(620)`, `on(621)`) and the two lambdas given to `sendInputDialogue`; the companion `shopsById`/`shopsByNpc` |
| `ItemDefinition.hasShopCurrencyValue` (both forms) | `shop.rs` | the reference comparison `getOrDefault(key, "0") != "0"` (any stored entry answers true) |
| `IfaceSettingsBuilder` | constants | `enableOptions(0..9)` = 2046, `enableOptions(0 until 9)` = 1022 |
| `TickUtils.secondsToTicks` | `controller.rs` | `seconds * 1000 / 600` in Int arithmetic |
| `Kotlin roundToInt` / `Math.round` | `shop.rs` | NaN refused, saturation, the JDK 11 bit algorithm |

`content/global/handlers/iface/ShoppingPlugin.java` is commented out in
the pinned source (every line is inside `/* */`); the engine routing of the
shop buttons is Shops' InterfaceListeners, which this port follows.
`world::shops::World` holds the companion tables, one per world (the
original has one per JVM).

## What is joined to native pieces

- Containers: shop stock instances, each shop's player stock and the general
  player stock are native `Container`s (SHOP, ID sort, 40); the player's
  inventory is the native container the host hands over. Every add, remove,
  `getMaximumAdd`, `hasSpaceFor`, `getAmount`, update, refresh and clear runs
  the native container code. A `ShopListener` is a native listener object
  (`shop::ShopListener`) whose update/refresh run the ported bodies; other
  listeners are host calls.
- InterfaceManager: opening 620 (`openInterface`), the single tab 621, the
  close of 620 and `closeSingleTab` are the native `world::interfaces` code
  with a shop `interfaces::Context` (`shops/ui.rs`).
- InterfaceListeners: `controller::define_interface_listeners` registers four
  typed callbacks (`TemplateButtons`, `SideOpen`, `TemplateClose`,
  `SideButtons`) on the native `world::interface_listeners::Registry`; the
  interface context's `listener_open`/`listener_close` and the button path run
  the registry's `run_open`/`run_close`/`run`, which dispatch to the ported
  lambdas through `ui::Callbacks`.
- Input prompt: Buy-X and Sell-X hand the host an `AmountScript` (a native
  object capturing the shop, the player and the slot); the replay gives it to
  the native `input_prompt::send` and answers with `input_prompt::receive`,
  whose `invoke` runs `shop::run_amount`.
- Ironman: the player-stock and overstock refusals ask the native
  `ironman::Manager.is_ironman()`.
- Item definitions: values, alchemy values (`definition_alchemy`), notes,
  destroy options, tradeability (`definition_tradeable`) and the currency
  prices (`tokkul_price`, `archery_ticket_price`, `castle_wars_ticket_price`)
  are read from native definitions.
- Packets: the stock frames are the native `inventory_partial`/`inventory_full`
  writers (to -1/-1, container 92); access masks, run scripts, interface
  configs and strings use the native protocol writers.
- Start-up parsing uses the start-up loaders' JSON-simple helpers
  (`startup::loaders`).

## Behaviour that matters for gameplay

- Construction: each `shops.json` row's `stock` is split on `-` (so a
  negative number can never appear: `{1,5,-2}` fails as `2}`), braces are
  removed, a repeated item adds to the first row with a WARN line, `inf` is
  an amount of -1, an unparsable restock rate is 100, `npcs` is a comma list
  (a blank one is none), `general_store`/`force_shared` are case-insensitive
  `true`, `high_alch` is exactly `1`. A bad row logs MALFORMED STOCK and stops
  the start-up. The last row with an id wins in `shopsById`; an NPC listed by
  two shops opens the later one.
- Shared versus personal stock: without `world.personalized_shops` (or for a
  `force_shared` shop) one stock container per shop is keyed by
  `SERVER_NAME.hashCode()`; with it each player uid gets its own container on
  first open. The setting is read at every use, so switching it while shops
  exist changes which container a player gets; closing the shop after
  switching it off with no shared container fails with NullPointerException.
- General stores share one 40-slot player stock across every general store.
  The "too full" check reads that shared stock for every shop.
- Buying: price per unit is `ceil(value * mod / 100)` with mod from 130 (empty)
  down to 100 (at or above stock), at least 1; currencies 6529/1464/4067 use
  the item's configured price (default 1) times the amount; fixed prices
  (spirit shards 25, pouch 1, magic capes 99,000). The amount is clamped to the
  stock and the inventory's maximum add; out of stock, not enough money (the
  currency's lowercased name), no space (the money is given back) and both
  ironman refusals have their messages. Buying from the player stock removes
  the item when it reaches zero; the main stock keeps a 0 item. Every 620
  button computes the buy price first, so an empty slot says "That item
  doesn't appear to be there anymore" even for Examine.
- Selling: 995, 6529 and 1464 are refused as currency (4067 is not, but its
  value message says the shop will not buy it), destroyable and untradeable
  items are refused, a specialist shop refuses items it does not stock. The
  price is the alchemy value (high or low per the shop) less 3% of the value
  per unit of overstock, capped at 10 units, at least 1; tokkul shops pay a
  tenth of the tokkul price. Notes are sold as their unnoted item. Selling
  switches the shown tab to the container the item went to.
- `getStockSlot` keeps the last matching stock row (item or its note) and
  then searches the player stock; a stock row holding a note can receive the
  unnoted item in a new slot beyond the stock array, and buying from that
  slot fails with ArrayIndexOutOfBoundsException.
- Listeners: `openFor` installs a new ShopListener per player uid; `showTab`
  adds it to one container and removes it from the other, so repeating the
  same tab adds it twice (and the player gets each update twice); stale
  listeners left on a container keep receiving updates.
- Restock: every tick each stock instance flagged in `needsUpdate` moves each
  stock row one unit toward its stock amount when `ticks % restockRate == 0`
  (rate 0 never; Java remainder for negative ticks); rows beyond the stock
  array are left alone. `needsUpdate` is never cleared. Under personalized
  shops a `force_shared` shop's flag is keyed by uid while its container is
  keyed by the server name, so it never restocks.
- Player stock clear: when `ticks % secondsToTicks(minutes * 60) == 0`
  (tick 0 included) each general player stock item is offered to the Grand
  Exchange (when recirculation is on) and the stock is cleared. An interval
  of 0 throws ArithmeticException every tick.

## Evidence

`cargo xtask shops-oracle` compiles Shop.kt, Shops.kt, InterfaceListener,
InterfaceListeners, IntType, ContentInterface, StartupListener, TickListener,
IfaceSettingsBuilder, InputType, RunScript, TickUtils, DialogueConst,
IronmanManager/IronmanMode, Definition, Item, Container with its
event/listener/type/sort classes, Component, ComponentDefinition,
ComponentPlugin, CloseEvent, InterfaceType, Plugin, Initializable, the complete
InterfaceManager, ClientInfo, IoBuffer and nine packet writers with their
contexts, all unchanged, with Kotlin 1.8.20 and runs them on JDK 11, each case
in a fresh class loader. The original text of the members listed in the
fixture's `scope` is inserted into recording adapters at generation time. The
pinned interface_configs.json is loaded in both runs.

The corpus has 634 cases with 5,563 actions (2,793 interface button presses,
831 shop opens, 96 prompt answers, 143 tick runs totalling 23,099 ticks, 193
price queries), in which the original makes 59,852 recorded calls (25,276
packet frames compared byte for byte), completes 724 purchases and 578 sales,
constructs 12,545 shops and records 18 injected failures. 12 cases build all
245 pinned rows of `fixtures/startup-loaders/shops.json` (every shop compared
field by field: title, flags, currency, stock rows, stock containers, restock
rates, NPC map) and open, value, buy from, sell to and close every pinned shop,
then run 501 ticks; item definitions for those rows are authored inputs (the
real item cache is not used). The other cases cover 61 start-up and loader cases,
every NPC listener and the destination override, every amount option on every
kind of shop and currency, price levels across stock levels, overstock beyond
the cap, full inventories, notes, untradeables, currency, item views, ironmen,
two players, uid collisions, personalized and shared stock with the setting
switched, restock and decay over runs of up to 5,000 ticks and the player
stock clear at several intervals. After every action it records the result or
the exception class, the calls in order, and the state: each player's
inventory and interface manager (opened, single tab, chatbox, overlay, tabs,
packet counter), the shop attributes, prompt attributes and ironman mode, the
shops whose state changed (stock instances with their listeners, needsUpdate,
restock rates, player stock), the general player stock, listenerInstances, an
FNV-1a digest of every shop's stock and the tick.
`crates/server/tests/shops.rs` replays every case on the native library and
compares all of it.

```powershell
cargo test -p rs09-server --test shops
cargo xtask shops-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
```

The generator never downloads. It needs the pinned Kotlin jars in
`runs/kotlin-1.8.20` and the pinned json-simple jar in `runs/shops-oracle` or
in any other `runs/*-oracle` folder (it checks the hash). It writes the case
files it reads under `runs/shops-oracle/cases`.

## Required services

`shops::Host` lists them, with no defaults: the world's tables, ticks,
definitions, container factory, `getServerConfig().getBoolean`,
`SERVER_NAME`, `CONFIG_PATH`, `PLAYER_STOCK_CLEAR_INTERVAL`,
`PLAYER_STOCK_RECIRCULATE`, the file read of `shops.json`, logging and the
InterfaceListeners registry; the player's uid, name, inventory, Ironman
manager and interface manager and the `shop`, `shop-cont` and `shop-main`
attributes; `Player.sendMessage`, `ContentAPI.sendMessage`, `sendDialogue`,
`Player.debug`, the session write, `getExamine`, `player.dispatch` of the two
shop events, content item views, `sendInputDialogue`,
`GrandExchange.addBotOffer`, `TanningProduct.open`, the quest reads,
`sendNPCDialogue`, the NPC's `facing_booth` attribute, other container
listeners, other InterfaceListeners callbacks and the InterfaceManager services
listed in `ui.rs`.

## Differences from the original

- Failures are categories (`Error`); JVM exception messages are not compared.
- `ContentAPI.removeItem`/`addItem` are ported for the inventory, the only
  container the shop passes.
- The companion tables are per world, not per JVM.

## Not covered

The registration of the NPC listeners into the InteractionListeners registry
and its dispatch (the replay picks the id-specific listener before the
option-only one), walking to the NPC, the two command bodies (`::openshop`,
`::shopscript`; their definitions are compared), the real InventoryListener (a
recorder stands in), GameMessage bytes, examine text, interaction locks and
the zone/dialogue checks before a button reaches the listener, tanning and
interface 732 content, the content shops (CreditShopInterface, MTAShop,
CulinomancerShop and the shopkeeper dialogues), real item cache values,
concurrent shops on two threads. The examples' controlled shop
(`examples/common/ui_shop.rs`) is a separate stand-in, not this port.

## What the next steps need from this

The start-up step needs to create `shops::World` once per world after
definitions (`World::new(factory)`), register `define_interface_listeners`
on the world's InterfaceListeners registry during the class scan, register the
NPC listeners (`controller::NPC_LISTENERS`, bodies `controller::npc_option`)
and the destination override (`controller::destination`) on the
InteractionListeners registry, and run `controller::startup` at the start-up
listener step with `CONFIG_PATH`. The packet-dispatch step needs IF_ACTION on
620/621 to apply processIfAction's dialogue/lock/zone/hidden checks and call
the registry's `run`; the input-prompt response to go through
`input_prompt::receive` with an `invoke` that runs `shop::run_amount` for an
`AmountScript`; the close-interface packet to use `interfaces::close` with the
shop context so `onClose(620)` runs. The tick driver needs to call
`controller::tick` with the tick listeners (after the pulses, before the
update sequence), increment `GameWorld.ticks` afterwards, and on a failure
skip the rest of that tick (the increment included) as `handleTickActions`'
catch does.
