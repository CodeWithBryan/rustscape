# Books

`world/books` ports `content.global.handlers.iface.BookInterface` and the
global books of `content.global.handlers.item.book`: GeneralRuleBook (the
rules a Town Crier shows), GoblinBook, GrimDiary, SecurityBookPlugin and
StrongholdNotes.

- `mod.rs` is BookInterface: the three book components (26 and 27 with 15
  lines a page, 27 with index and clickable rows, 49 with 11 lines) and
  their line, button and image ids; `open_book` (close, page 0, the
  callback attribute, the component opened when it is one of the three,
  then the callback for page 0); `page_setup` (the current page read first;
  every line blanked with the component opened again, every button hidden,
  the title, the pagination of the current page set, whose
  `contents[currentPage]` fails with an index error outside the book, then
  the set's lines: a line child is sent, a button child shown and sent);
  `set_pagination`, `set_page_content`, `set_model_on_page`,
  `set_item_on_page`, `clear_models_on_page` (book 27's enabling layers on
  the given component), `is_last_page`; `change_page_and_callback` (the
  callback attribute cast to the Kotlin function type, the page moved, then
  the callback with the new page when there is one) and the listeners'
  `button` (left, right, anything else keeps the page; always true). The
  books' `display` functions (GeneralRuleBook's index jumps on page 0 and
  its "Index" button elsewhere, shown away from the first page) and `read`
  (the book opened, then stored in the house bookcase for all but the Grim
  Reaper's diary).
- `contents.rs` holds the five titles and page tables, written by
  `oracle/books/port_contents.py` from the unchanged sources (`--check`
  compares the committed file). The world's names the books concatenate
  (`GameWorld.settings!!.name`, `ServerConstants.SERVER_NAME` for the goblin
  book) are filled in once per book class (`Tables`).
- `registry.rs` is the registration: BookInterface's three component
  listeners and the item books' "read" listeners as typed values with their
  Kotlin lambda classes, the `::display` references stored as the callback
  (`callback`), `define`, `register_scanned` (the start-up manifest's order)
  and `Native`, the registrar over the native InteractionListeners and
  InterfaceListeners registries.

Every outside call (attributes, `closeInterface`/`openInterface`, the packet
dispatch's strings, configs, models, angles and items, `storeBookInHouse`,
the world's names) is a required `Host` service with no default.

## Verification

`cargo xtask books-oracle --check` (Python and JDK 11) compiles the unchanged
BookInterface.kt, the five book classes and IntType with ConstLib 1.4.28 (the
Items values checked against their names' suffixes); ContentAPI's
getAttribute, setAttribute, openInterface and closeInterface are inserted
verbatim; the listener registries, the player and storeBookInHouse are
recording adapters (`oracle/books/adapters.py`). Every case runs in a class
loader of its own, so each book class initialises with the case's names.
`python oracle/books/port_contents.py --check` reproduces the page tables.

The corpus (`oracle/books/cases.py`) has 1,239 cases (49 designed, 1,190
single injected failures) and 5,036 operations: every registration; each item
book read, paged forward past its last set and back past the first, other
buttons, a second read, not stored in the house, a failing page turn, every
component's buttons with the book's callback; the rules book through every
page, every index button on the first page and the index button elsewhere,
its other buttons; other world names; a missing callback; mistyped page and
callback attributes; page sets far outside the book; models, items and
cleared models on pages and `isLastPage`. After every operation the replay
compares the result or error, every recorded call with its arguments in order
and the player's attributes (the callback by its Kotlin class).

`cargo test -p rs09-server --test books` replays it and checks that
`register_scanned` installs the listeners into the native registries in the
manifest's order. Of twelve deliberate breaks (the left arrow's hiding, the
single-page test, an index row, the house store of the diary, the callback's
page number, the cleared image layers, the component reopened per setup,
the close before opening, a callback class, a page line, the item amount,
the model zoom), eleven fail the replay; the callback's page number is
equivalent for these five books (only GeneralRuleBook reads it, and only an
arrow changes it, which is never an index or "Index" button).

## Not ported or not joined

- Nothing installs the books in `rs09-server --world`. Installing means:
  `register_scanned` with `registry::Native` over the live registries, the
  item listener dispatching `registry::run_interaction` and the component
  listeners `registry::run_interface`, and the host answering attributes from
  the canonical player, the interface services from `world/interfaces`, the
  packet dispatch from the interface packets and `storeBookInHouse` from the
  POH storage port (ContentAPI; not ported).
- The Town Crier's dialogue (the caller of `open_general_rules`) and the
  other BookInterface users (quest books such as VarmensNotes) are content
  and not ported.
- The item books' tables are filled on first display; Java fills them when
  the listener class initialises (at registration). The difference is
  observable only with null settings or names that change in between,
  which no case does.
- ScrollInterface is world/global_handlers (docs/global-handlers.md).
