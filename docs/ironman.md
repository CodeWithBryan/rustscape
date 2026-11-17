# Ironman manager and canonical player values

`player/ironman.rs` ports the complete pinned IronmanManager and IronmanMode
behavior. Modes keep the original ordinal and icon order. A null mode is still
different from NONE; isIronman therefore returns true, while a restriction check
can fail when reading its ordinal. NONE short-circuits a null threshold. Messages
retain their actual player receiver and execute without a manager guard held.
Reentrant mode changes and failures retain their completed prefixes.

The 81-case JVM corpus compiles both complete unchanged original classes. The
Player adapter forwards only sendMessage to the explicit recording callback.
It compares defaults, nulls, every threshold, icons, aliases, message reentry and
failure. Run `cargo xtask ironman-oracle --check --python PYTHON --java-home JDK11`
and `cargo test -p rs09-server --test ironman --test owned_player`.

Each canonical player owns its actual manager. The manager's final player field
is an internal graph edge counted once per native object. An exposed manager
retains the original disconnected player rather than a newly reused wire index;
dropping it releases that graph. Attribute views similarly retain the actual
player and access its existing GameAttributes implementation. Null and false
keys both count as present; integer reads preserve null defaults and type errors.

The current canonical attribute API exposes typed integer/boolean/null mutation
needed by this integration. General object-valued attributes and their complete
ownership metadata, profile parsing/saving and other Entity services remain open.
No untraced arbitrary object setter is exposed on the canonical field.
