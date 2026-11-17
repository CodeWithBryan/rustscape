# Canonical bank runtime binding

The existing Rust BankContainer kernel now uses actual canonical player fields,
variables, interface components and captured close callbacks in the controlled
movement/NPC composition. `examples/common/ui_bank.rs` provides its UI-facing
services. `player/owned/inventory.rs` also dispatches the real private BankListener
from ordinary inventory operations; banking is not isolated to a separate item
array or an interface-only simulation.

The BankListener's four actual dependencies are exposed as `bank::ListenerContext`:
the current bank, its packet sink, varbit writes and varc writes. Existing complete
Bank contexts implement that subset automatically. Refresh/update retain the same
original algorithm and order; the 546-case bank oracle still compares the complete
original BankContainer/private BankListener, including the initial488 baseline.
The 628 interface, 474 variable and 855 PIN corpora remain separate comparisons;
they are not a combined whole-server JVM oracle.

Each callback dispatches its actual typed listener header and captured player.
When attached to another player's inventory, the BankListener sends that passed
container's data to its own player. For bank containers, it subsequently resolves
that player's actual current primary/secondary bank for tab configurations and
bank-space varc192. An ordinary inventory callback sends the side-panel inventory
without the bank-only variable updates. Foreign domains fail before the callback
changes their state. Completed outer/container and earlier-listener prefixes are
not rolled back.

The world now retains the immutable variable cache archive configured before
player admission. Reusing the exact archive handle is idempotent; replacing it or
first binding it after canonical admission fails. `apply_world_variables` resolves
the actual world's archive and definition registry. No archive is treated as an
unavailable service when an uncached definition needs loading. A real available
archive with a missing file retains the original default-definition behavior.
Cached definitions remain usable without additional I/O. The controlled startup
uses the same real archive22 already supplied to its UI variable services.

Four new canonical tests cover active-bank selection, signed shared counters,
inventory-side listener dispatch, real captured recipients, foreign-world rejection,
partial failed variable prefixes, and the archive's binding/lifetime rules.

Both controlled examples also exercise:

- Opening the real bank and single side tab, including idempotent second open.
- Depositing two real inventory items, withdrawing one, and updating the same
  bank/inventory arrays, listener list, weight and variable state. The original
  initial last-X amount remains50.
- Ordinary player inventory operations while the exact bank listener is installed.
- Closing through the actual captured BankClose object: close the side tab,
  remove the listener/search attribute, send script571, then close the main pane.
- Ultimate Ironman refusal, explicit unavailable PIN entry, foreign BankClose
  rejection before changing its captured open bank, and collection of disconnected
  open bank/UI/listener cycles without external roots.

This checkpoint established callable runtime bindings with regression evidence.
The later [bank content stage](bank-interface.md) installs BankInterface's open
and button handlers and joins decoded packets to basic live transactions.
Ordinary banker/booth admission, PIN entry, deposit-box item-select rendering,
other content dialogues, complete profiles and remaining content services
remain required. The later [input stage](input-prompt.md) binds controlled X and
plain error/Continue. Required services must be ported, not replaced with successful placeholders.
No full playable banking, full replacement, performance improvement or capacity
claim is made here.

```powershell
cargo test -p rs09-server --test bank --test owned_player --test variables --test interfaces
cargo test -p rs09-server --example movement_session --example npc_session --example player_session
cargo xtask bank-oracle --check --java-home runs/jdk11/jdk-11.0.32.1+1
```
