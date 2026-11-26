# Bank content and interface packet dispatch

The Rust BankInterface program now registers real open and button callbacks in
the controlled movement/NPC server. InterfaceManager calls that same registry,
and decoded IfAction packets run the selected original PacketProcessor algorithm
before reaching the bank. Deposit and withdraw change the canonical bank and
inventory through the existing BankContainer and installed listeners. Closing
uses the actual captured BankClose object and the original CloseIface order.

The new differential corpus has 572 scenarios. It compiles the complete unchanged
BankInterface, InterfaceListener, InterfaceListeners and IfaceSettingsBuilder,
plus the exact selected PacketProcessor IfAction/CloseIface bodies and anonymous
login pulse. Its independent locks preserve the initial336 bank/registry cases
and the first559 packet cases byte-for-byte. Player, containers, attributes,
definitions, dialogue, locks, zones, plugins, events, scheduling and login are
explicit mutable recording services. Bank item mutation is independently checked
by the existing546-case original BankContainer comparison and composed native
tests; this is not a whole-server JVM comparison.

The port preserves:

- On-open access mask2623486, current configured bank size, reset-search tab
  selection and varc190. The original open callback returns false.
- Menu opcodes155/196/124/199/234/168/166/9, inventory-specific behavior, modes,
  tabs, search and help/deposit dialogue service calls. Slot contents determine
  the item; the wire item ID is not a new validation condition.
- Bank/container receivers captured before later getters and callbacks, including
  last-X and all-item amounts. Examine reads the actual cached definition and
  preserves the100000-coin boundary, second amount read and UTF16/null text.
- Real function identity, aliases and replacement, the stateless help singleton,
  nullable raw-map values and arity errors at the original lookup boundary.
  A specific callback returning false does not cause a generic retry.
- IfAction dialogue/lock/zone ordering, hidden/missing components, plugin capture
  before ButtonClickEvent, current registry lookup and original plugin fallback.
- CloseIface's ignored close result, non-null logging/map attribute checks,
  captured deferred login player, map removal before window type2, and completed
  prefixes on reentry or failure. False and zero attributes still count as
  present. LoginPulse reports its actual strong player capture.

Runtime Callback wraps a real NativeObject header. Global registry aliases retain
only players actually captured by the callback. The canonical graph regression
removes aliases separately and verifies collection after the final external
capture disappears. The bank callbacks themselves capture no player. Function
metadata and cloned receivers are inspected without holding callback guards.
General public-map iteration/null keys and arbitrary opaque object fields are
not established by this registry port.

Both controlled example targets test actual155 decode/dispatch, main and side
panes, transfer amounts, modes/search, hidden and missing components,184 close
and a late155 after close. A cached763:0 slot8 fixture proves that charged items
are selected by the actual slot without transferring different charge instances.
The opt-in `--bank-probe` controller waits for a real1351 inventory acquisition,
opens the bank once per player, and never reopens it after close. It does not
seed inventory or claim banker/booth admission. It requires `--gameframe` and
must be enabled before admission.

The shared headless `pickup_agent --banking` keeps both actor and spectator
readers active. On real TCP and WebSocket connections it takes the ground axe,
waits for bank and inventory packets, deposits one, waits for authoritative95/93
container changes, withdraws one, and sends184 followed by a late155. Success
requires both panes closed and at least two subsequent225 player updates.
Private bank traffic must not reach the spectator. Raw requests, quantities and
observed stages are recorded rather than inferred from elapsed time.

Live visual QA uses the same frozen candidate server with actual server-owned
bank data. Windows rendering driven by an explicit protocol actor and browser
mouse input have separate receipts. The former does not establish Windows OS
pointer behavior. Final gate, transport, visual and source receipts are under
`runs/bank-interface-investigation`; candidate binaries and later rebuilds retain
their own hashes so the visual candidate is not silently relabelled.

Fresh controlled admission currently supplies no active dialogue, component lock,
zone interception or game event listeners. Those are explicit fixture conditions,
not complete implementations. Banker/booth registration and admission, live PIN,
help/deposit dialogues, login scheduling/world configuration,
world-map unlocking, complete profiles and content remain required. Unsupported
services return explicit binding errors. The later [input stage](input-prompt.md)
independently joins controlled X/error/Continue with finite TCP/WS and browser
acceptance; it preserves this checkpoint as historical evidence. No whole-game replacement, performance
gain or player capacity is established here. All4042 whole-source rows remain
`not_replaced`.

`rs09-server --world` installs the program (`world/live/bank.rs`, [the live world](live-world.md)): the 762 open listener from `InterfaceListeners.runOpen`, the 762/763/767 buttons from IfAction, the canonical bank opened by a banker (`BankerNPC`'s listeners and the banker conversation) and the captured BankClose. Booths, chests, deposit boxes, the PIN and the help/deposit dialogues are refused there.

```powershell
cargo test -p rs09-server --test bank_interface --test bank --test owned_player --test interfaces
cargo test -p rs09-server --example movement_session --example npc_session --example player_session
cargo xtask bank-interface-oracle --check --java-home runs/jdk11/jdk-11.0.32.1+1
```
