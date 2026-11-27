# Grand Exchange

`world/grand_exchange` ports the Grand Exchange engine: the offer database
(GEDB over SQLite, with the original schema and statements), offers and their
states, matching between player offers and against bot stock, the price
index and guide prices, a player's offer records with the collection box and
history, the periodic notification timer, item sets, bot prices and the
start-up auto stock, and the interface handlers that drive it (StockMarket on
105/107/389, GrandExchangeInterface on 109/389/642/644/645, ExchangeItemSets)
plus PacketProcessor's `GESetOfferItem` branch. It is a library. Nothing
installs it in a live world yet (see the last section). Java is a test
oracle; nothing here runs it.

| Original | Rust | Scope |
| --- | --- | --- |
| `core/game/ge/GEDB.kt`, `integrations/sqlite/SQLiteProvider.kt` | `db.rs` | `init`/`initTables` in the `hashMapOf` iteration order (price_index, player_offers, bot_offers; a table of eight buckets), the three `CREATE TABLE` texts, `populateInitialPriceIndex` in `getDefinitions().values` order, xerial column reads (NULL as 0 or null), prepared versus direct statements |
| `GrandExchange.kt` | `exchange.rs` | `startup`, `boot` (`isRunning`), the worker loop body, `selectPotentialMatches`, `exchange`, `tryExchangeWithBots`, `dispatch` (including `I_AM_A_CHEATER`'s pulse), `addBotOffer`, `getRecommendedPrice`, `getOfferStats`, `getValidOffers`, `getBotOffers`, `getBotstockForId`, `getOfferByUid`, the three command definitions and bodies |
| `GrandExchangeOffer.kt`, `OfferState.java` | `offer.rs` | every member: the bot `amount`/`amountLeft` clamps, `addWithdrawItem`, `visualize`, `update`, `writeNew`, `encodeWithdraw`, `fromQuery`, `fromBotQuery`, `createBotOffer`, `cacheValue` |
| `GrandExchangeRecords.kt`, ContentAPI `hasAwaitingGrandExchangeCollections` | `records.rs` | `login`, `parsePlayer`, `savePlayer`, `openCollectionBox`, `visualizeRecords`, both `getOffer`, `openHistoryLog`, `init`, `hasActiveOffer`, `format`, `getInstance` |
| `PriceIndex.kt` | `price_index.rs` | every member and `PriceInfo` |
| `GrandExchangeTimer.kt` | `timer.rs` | `run` (the registration is `startup::timers`) |
| `GEGuidePrice.java`, `StringUtils.formatDisplayName` | `guide_price.rs` | `open`, `clear`, `GuideType` tables and `display` |
| `GEItemSet.java` | `item_sets.rs` | the 70 sets, `forId`, `getItemArray` |
| `BotPrices.kt`, `GEAutoStock.kt` | `bot_prices.rs`, `auto_stock.rs` | overrides and `autostock` |
| `StockMarket.kt`, PacketProcessor `GESetOfferItem` | `stock_market.rs` | the six listeners, `updateOfferValue/Amount`, `abortOffer`, `confirmOffer`, `getInventoryAmount`, `openFor`, `updateVarbits`, `withdraw`, `toMainInterface`, both input handlers |
| `GrandExchangeInterface.java` | `interface.rs` | `newInstance`, `handle` and its pulse body with the four handlers |
| `ExchangeItemSets.kt`, `InterfaceContainer.java` (`generateItems`, `generate`, `increment`) | `sets_interface.rs` | `onClose(645)`, `openFor`, the inventory listener |
| `GrandExchangePacket.java` | `ui.rs` | the opcode 116 writer |

## What is joined to native pieces

- Database: `Db` is a native SQLite file (`rusqlite` with its bundled
  SQLite). Every statement the original executes is executed with the same
  SQL text and bound parameters; `Host::statement` observes each one first.
  `Db::prepare` compiles where `prepareStatement` does, so a statement on a
  missing table fails before anything executes, as in the original.
- Containers: the player's inventory is the native container; removals,
  additions, `hasSpaceFor`, `getAmount`, `containItems`, `freeSlots` and
  `refresh` run the native code. ExchangeItemSets' listener is a native
  listener object (`SetsListener`).
- InterfaceManager: every open/close/chatbox/single-tab call is the native
  `world::interfaces` code with the GE's `interfaces::Context` (`ui.rs`).
- InterfaceListeners: `stock_market::define_interface_listeners` and
  `sets_interface::define_interface_listeners` register typed callbacks on
  the native registry; GrandExchangeInterface is a typed ComponentPlugin on
  the native component definitions (`interface::new_instance`).
- Input prompt: buttons 170 and 185 hand the host an `InputScript` for the
  native `input_prompt::send`; `stock_market::run_input` is the handler body.
- Ironman: `openFor`'s restriction is the native manager. Item definitions
  (values, alchemy values, notes, tradeability, names) are native.

## Behaviour that matters for gameplay

- Matching runs in the worker: an offer is written, read back by uid, then
  matched against the open player offers of the other side sorted best price
  first (stable: equal prices keep row order); a player's buy offer with
  anything left then meets the first bot row of the item. A trade happens
  only when the buyer pays at least the seller's price; the older offer's
  price is used (a seller newer than the buyer is paid the buyer's price,
  otherwise the buyer pays the seller's price and the difference is refunded).
  Equal time stamps count as the buyer being newer. Amounts and coins wrap.
- Bot stock is clamped to `BOTSTOCK_LIMIT` through the `amount` getter, so
  a bot row is written back as `limit - completed`. Bot offers are sold at
  110% of the larger of the guide price and the bot price. An artificial
  player's offers become bot offers of the shared "2009scape" account.
- The price index moves only when the two sides are different accounts and
  (unless `BOTS_INFLUENCE_PRICE_INDEX`) neither is a bot; its volume resets to
  a tenth of the threshold of the price band (500/1,000/2,500/10,000).
- `confirmOffer` returns Success for a buy offer whose dispatch fails (the
  coins are kept and nothing is queued); a sell offer is taken unnoted first,
  then as notes. `withdraw` with an option other than 155 or 196 empties the
  slot without giving anything. Ring of wealth (i) ids are withdrawn as 2572.
- `hasAwaitingGrandExchangeCollections` only ever looks at slot 0, so the
  timer's notice depends on slot 0 alone.
- `parsePlayer` gives rows saved with slot -1 the slots that are free in the
  class-scan instance's own records (always all six), so fewer than six such
  rows end in NoSuchElementException after the first ones are placed.
- InterfaceContainer: `generateItems` sends with one key and returns the
  next, which is what "container-key" holds.
- An exception inside the worker ends the thread: later offers stay queued.

## Evidence

`cargo xtask grand-exchange-oracle` compiles the eleven `core/game/ge`
sources, StockMarket, GrandExchangeInterface, ExchangeItemSets,
InterfaceContainer, GrandExchangePacket and the interface, container, input
and packet classes the shops oracle uses, all unchanged, with Kotlin 1.8.20,
and runs them on JDK 11 with the pinned sqlite-jdbc 3.36.0.3 over a real
database file per case, each case in a fresh class loader. SQLiteProvider is
its original text without the coroutine members, its connection wrapped by a
proxy that records each statement with its bound parameters. The fixture's
`scope` lists every inserted original text and recording adapter.

The corpus has 321 cases with 4,174 actions (1,550 button presses, 487
inputs, 377 worker runs, 82 pulse runs, 82 timer runs), in which the original
makes 52,104 recorded calls, 22,465 of them SQL statements of 28 kinds, and
records 61 injected failures. After every action it records the result or
exception class, the calls in order (frames byte for byte, statements with
their parameters, clock reads, Repository lookups) and the state: each
player's inventory and listeners, interface manager, GE attributes (the
temporary offer, records with slots and history, guide type, container key,
item-set listener, input prompt), every reachable offer with identity labels
(so aliasing is compared), the pending queue, the class-scan records, the
pulses, the worker state, InterfaceContainer.index and, when changed, every
row of every table (`quote()` of each column, in rowid order) and
`sqlite_master`. `crates/server/tests/grand_exchange.rs` replays every case
on the native library over its own database file and compares all of it.

```powershell
cargo test -p rs09-server --test grand_exchange
cargo xtask grand-exchange-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
```

The generator never downloads. It needs the pinned Kotlin jars in
`runs/kotlin-1.8.20`, json-simple and sqlite-jdbc in `runs/grand-exchange-oracle`
or any other `runs/*-oracle` folder (hashes checked against
`oracle/auth/json.lock.json` and `oracle/world-trace/artifacts.lock.json`).

## Required services

`grand_exchange::Host` lists them, with no defaults: the world's GE tables,
definitions and their map order, the container factory,
`System.currentTimeMillis`, the five ServerConstants, the file read of
`autostock.json`, logging, `SystemLogger.logGE`, the statement observer, the
InterfaceListeners registry, the worker start, `GameWorld.Pulser.submit`,
`sendGrandExchangeNews`, `Discord.postNewOffer`, `PlayerDetails.getDetails`,
`Repository.uid_map` and `Repository.players`, examine text, CS2 mapping 1089;
the player's uid, name, artificial flag, inventory, Ironman manager,
interface manager, bank PIN state and the six GE attributes; the messages
(`Player`/`PacketDispatch.sendMessage`, ContentAPI `sendMessage`, `notify`),
the session write, `setVarp`, `playAudio`, `playJingle`, the input dialogue,
other container listeners and InterfaceListeners callbacks, and the
InterfaceManager services listed in `ui.rs`.

## Differences from the original

- Failures are categories; JVM exception messages are not compared.
- The worker is a function the world calls (`exchange::run_worker`); the
  original runs it on its own thread concurrently with the world thread. The
  corpus runs it at chosen points on one thread.
- SQLiteProvider's lock and connection count are not modelled: after an
  exception inside `GEDB.run` the original never closes its connection again
  and, with two threads, a later `unlock` can fail. The corpus includes
  exceptions inside open result sets (the bot match, uid and stats reads);
  the database contents match there, but the cross-thread lock failure is
  not reproduced.
- The native SQLite is the one `rusqlite` 0.40.2 bundles, newer than the
  driver's 3.36.0; the statements used behave the same and the corpus
  compares every row.
- Column reads of REAL and TEXT values follow SQLite's conversions only for
  the values these classes can meet.
- The replay test turns on `db::set_synchronous_off`, so its databases use
  `PRAGMA synchronous = OFF` (a test/tool switch, off by default; the original
  and the native default keep SQLite's FULL). This changes durability after a
  crash only, not rows or statement results; on Windows it cuts the replay
  from about 123 to 16 seconds, since every autocommitted statement otherwise
  waits on a disk flush. The JVM side of the corpus is unchanged.

## Not covered

Concurrency between the worker and the world thread, the clerk/tutor
dialogues and other content callers (`openGrandExchange`, `GEGuidePrice.open`
callers, the Varrock shortcut, bots), the real InventoryListener, GameMessage
bytes, examine text, the real item cache, the Discord webhook, the
processIfAction checks before a button reaches the listeners, and command
dispatch (the three bodies are compared, not their registration in the
command system).

## What the next steps need from this

The start-up step needs one `Exchange` per world, `exchange::startup` at the
GrandExchange start-up listener (with `GRAND_EXCHANGE_DATA_PATH`),
`auto_stock::autostock` at GEAutoStock's, the two `define_interface_listeners`
and `interface::new_instance` during the class scan, and the three commands
in the command system. The worker needs a place to run `run_worker` (its own
thread with the world's host, or a fixed phase of the tick). The tick driver
needs `Handler::GrandExchange` (the `GESetOfferItem` route) to call
`stock_market::set_offer_item`, IF_ACTION on 105/107/109/389/642/644/645 to
reach the registry and plugin, pulses submitted by the GE to run in the
world's pulse runner (`run_pulse`), and the GE timer (`timer::run`). Login
needs `records::login` and `records::parse_player` on the class-scan
instance (`Exchange::hook`), and the saver `records::save_player`.
