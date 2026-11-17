# 2009scape Rust replacement

The objective is full replacement of the pinned server and RT4 client, including
Java and Kotlin, while preserving observable gameplay and improving performance.

- For modern InteractionListeners changes, reproduce interaction-listeners-oracle
  and affected scripts/legacy-interaction/interaction-runtime/player-walk/pulses
  corpora; run interaction_listeners and those native suites plus all three examples.
  Preserve initial1168 observations and all1278 current cases. Compile the complete
  unchanged registry/interface/script sources with the declared reflection artifact.
  Retain actual native function identity/captures; no observer IDs select gameplay.
  Preserve scalar validation/bulk overwrite, partial registration, case/locale
  differences, reversed use fallback, wildcard modCount, result-discard differences,
  script/manager capture order and late IDs/zone/callback reads. No guard spans
  gameplay callbacks. Private sorted views do not establish HashMap iteration.
  Array identity hashes are explicit platform inputs. Canonical shared processor
  ownership/native objects and a real actor-clock/tick phase bridge exist;
  remaining close/dialogue and modern context services, movement arrivals,
  events/locks/zones and natural banker/booth content are
  still required runtime joins. See docs/interaction-listeners.md.

- For ActionLocks/Lock/LockElapse changes, run action_locks/owned_player and all
  affected script/pulse/player/NPC/world/ownership/input/interface suites plus all
  three examples. Reproduce action-locks-oracle and affected JVM corpora using
  JDK11. Compile all three unchanged source files; preserve initial309 observations.
  Preserve expiration-before-clock evaluation, distinct conditional assignment
  samples, signed wrap, strict deadlines, movement-then-interaction default order
  (not teleport), nullable equipment/messages/events/keys and exact shared aliases.
  Custom unlock clears first, reloads the event, calls with the selected object,
  then removes the current key after reentry; failures skip that removal. Do not
  invent automatic expiry callbacks. Actual runtime fields/bodies use typed native
  objects; diagnostic IDs never dispatch gameplay. Trace physical action/lock/map/
  callback fields once and count incoming aliases separately. Stateless exposed
  lock/map views do not anchor their storing player. Collect typed cycles, preserve
  foreign/unscoped state and drop payloads outside guards. Validate actual scopes
  before native body effects. Shared actor locks now bind script/walk/NPC/legacy/
  input/button admission; do not reintroduce the old NPC-only lock_until shadow.
  Broader content/world, arbitrary opaque cycles and concurrent GC remain open.
  See docs/action-locks.md.

- For projectile path or script line-of-sight changes, reproduce
  `cargo xtask projectile-oracle --check` with JDK 11, run the projectile,
  region_queries and script suites, and run the three controlled examples.
  Preserve the original query order/short circuits, source plane, the stop on
  a blocked diagonal, multi-tile footprint loop order, and partial path outcomes.
  Admission uses canonical actor coordinates and both scene flag layers.
  The script binding does not establish full combat or content parity.

- For combat swing calculation or BattleState changes, reproduce
  `cargo xtask combat-swing-oracle --check` with Python/JDK 11 and run
  `cargo test -p rs09-server --test combat_swing`. Preserve the 4,461 cases and
  20,762 operations. The hit decision draws Math.random; damage, Verac and
  bolt rolls draw RandomFunction.RANDOM: keep them separate host services and
  keep the draw count (none for a nonpositive bound, magic rolls `max` not
  `max + 1`). Keep every repeated attackStyle/lifepoints/equipment read, the
  float step order and int wrapping, the swallowed familiar failure, Weapon's
  ignored type, the dark-bow fallback and copied second hit, and the
  IGNORE_STAT_REDUCTION flag left behind by a failed diamond calculation.
  Prayer, perk, slayer, familiar, equipment, spell, audio/graphics/message and
  pulse members are required services with no defaults. What runs around and
  after a swing is world/combat_pulse (next entry). Other bolt effects and
  RangedConfigLoader are not ported; the library is not bound to a session and
  the alpha stand-in in examples/common/combat.rs is not this port. See
  docs/combat-swing.md.

- For CombatPulse, ImpactHandler or the handler members around a swing
  (world/combat_pulse: canSwing/isAttackable/canStepTowards/canMelee,
  adjustBattleState, getFormattedHit, addExperience, useAmmo, visuals, impact,
  onImpact, ArmourSet effects, special registration), reproduce
  `cargo xtask combat-pulse-oracle --check` with Python/JDK 11 and run
  `cargo test -p rs09-server --test combat_pulse --test combat_swing --test
  projectile`; reproduce combat-swing-oracle and projectile-oracle too.
  Preserve the 7,377 cases and 24,619 operations. Keep pulse()'s return order
  (victim, both deaths, both regions, interactable, then nextAttack), the
  time-out counter compared before it is incremented and before combatTimeOut
  is read, the path request for any result but STILL_INTERACT, the re-npc
  exception, and updateStyle after a refused swing. Speed: 5 for a casting
  player, minus one for rapid or salamander-accurate, times 1.5 truncated for
  miasmic and never for magic. The impact pulse is submitted with delay - 1:
  zero delivers on its first update, anything else shows the impact visuals,
  sets delay 1 and delivers a tick later; a second arrow lands one tick after
  the first. Pulse.update catches a failing body, prints it and stops, and the
  stop after a finished body is inside that try. EXPERIENCE_MOD is one world
  value: adjustBattleState resets it and addExperience multiplies it in place.
  handleImpact reads neither state nor secondary; the range class delivers a
  hit of -1. Kill credit follows HashMap iteration: uid keys, entity identity
  hashes (a host input), ties to the first key, a null source key, table size
  kept across clear. State is reached through host accessors and no borrow
  spans a service call; every service is required and has no default. The
  stand-ins' world model, tick runner and retaliating onImpact are oracle
  inputs, not ports. Not ported: removeFor, bolt-effect overrides (their two
  services are never driven by the corpus), special-attack and other handler
  subclasses, MultiSwingHandler, spell effects, DeathTask, graves, prayer
  drain, poison, degradation, Entity.onImpact/isAttackable and
  auto-retaliation. Nothing is bound to canonical actors, server::pulses,
  skills, equipment or a session. See docs/combat-pulse.md.

- For Prayer/PrayerType/PrayerCategory, the prayer tab handler, Player
  commenceDeath/fullRestore or the combat prayer reads (world/prayer), run
  `cargo test -p rs09-server --test prayer` and `cargo xtask prayer-oracle
  --check` with Python/JDK 11, plus the combat_swing, combat_pulse, skills
  and variables suites and combat-swing-oracle/combat-pulse-oracle. Preserve
  the 3,145 cases. Keep the varp write before flag, a prayer switched off once
  per conflicting bonus pair, permitted() also on switching off, the float
  `(1/30f) * bonuses[12]` read once per active prayer, the zero check before
  the drain, the decrement even of 0, reset leaving the tick counter and
  clearing the head icon a tick later, and getFormattedHit's truncating int
  multiply. Points, levels and healing are native Skills; every other call is
  a required service. ConstLib 1.4.28 supplies the sound names. Training,
  SkillRestore hooks, protect-item death and every session binding are not
  ported. See docs/prayer.md.

- For the spell framework (world/spells: Runes, CombinationRune, MagicStaff,
  SpellUtils.hasRunes, MagicSpell/CombatSpell/DefaultCombatSpell, SpellType,
  SpellBookManager, SpellBlocks, WeaponInterface's autocast members and the
  53 combat spell registrations), run `cargo test -p rs09-server --test
  spells` and `cargo xtask spells-oracle --check` with Python/JDK 11, plus
  the combat_swing, combat_pulse and prayer suites and their oracles.
  Preserve the 8,600 cases. Keep hasRunes' JDK HashMap walk (resizes
  included), combination runes first in declaration order with zero-usage
  entries, the stale spell:runes attribute, the staff-of-light draw before
  any rune check, the autocast-staff check against the autocast spell, the
  binding delay through Player.sendMessage, every repeated location/weapon/
  god-cast/NPC-id read, the God spells' three failure sounds and delay-0
  impact sound, Crumble undead's null NPC and the double requirement checks
  of Teleblock and the miasmic spells. Spell objects are host-owned values
  (spell_id and DefaultCombatSpell's projectile mutate); every outside call
  is a required service with no default; fireEffect is content and stays the
  combat host's service. spells::combat answers the combat slices' spell
  services and is replayed against the same corpus; no corpus runs the real
  MagicSwingHandler with these spells and nothing installs the library.
  Non-combat and lunar spells are not ported. See docs/spells.md.
- For the player tab systems (world/player_tabs: WeaponInterface, LevelUp and
  getSkillJingle, EmoteManager/Emotes, MusicPlayer/MusicZone.enter,
  QuestRepository.syncronizeTab/forButtonId/forIndex and the base
  Quest.getConfig), run `cargo test -p rs09-server --test player_tabs --test
  world_login` and `cargo xtask player-tabs-oracle --check` with Python/JDK
  11, plus the combat_swing, combat_pulse, spells, prayer and player_state
  suites. Preserve the 5,041 cases. Keep definition 92's type for the weapon
  tab's pane and child, the attack style read first in ensureStyleIndex, the
  special bar set before the animations, every repeated style index, spell
  book and getNew(3) read, the non-combat spell cast failure, the global
  definition mutation of openAutocastSelect, the 24 flashing-icon reads, the
  per-milestone combat level read, the nested skillcape unlock inside
  EmoteManager.refresh, lock's refresh without a removal, music key 62 as a
  list index, play's early null read when looping, tick's swallowed send and
  the varbit-or-varp choice by config length. Emote, song and quest state is
  the persisted state in player/persisted, joined, not copied. Every outside
  call is a required service; quest getConfig overrides and updateVarps,
  Skillcape.trim and the SALUTE, GOBLIN_BOW and GIVE_THANKS plays are
  content services. world_login joins these libraries to the canonical
  player (only the quest tab stays replayed); world/live/tabs.rs installs
  them in `rs09-server --world` (next to the live-world bullet). See
  docs/player-tabs.md.
- For the engine timers (timers/content: SkillRestore, Poison, PoisonImmunity,
  Disease, Frozen, FrozenImmunity, Teleblock, Skulled, HealOverTime, Miasmic,
  MiasmicImmunity, DragonFireImmunity, SpellbookSwap, AntiMacro's timer and the
  ContentAPI spawn/getOrStart/hasTimerActive/applyPoison/curePoison glue), run
  `cargo test -p rs09-server --test content_timers` and `cargo xtask
  content-timers-oracle --check` with Python/JDK 11, plus the timers,
  timer_registry, core_data, player_save, player_state, canonical_player and
  prayer suites. Preserve the 2,311 cases. Keep Miasmic's 100-tick immunity
  (the entity is getTimer's first argument), Poison's post-decrement damage,
  lateinit source failures and numeric source-uid, applyPoison's order, the
  second Disease removing itself, Frozen's boolean-only applyImmunity, the
  double Skulled reset, SkillRestore's increment-before-compare counters and
  its fixed 51-tick special energy (varp written at 100), SpellbookSwap's
  null book failure before the dispatch and AntiMacro's draw order. Timer
  fields live on the timer (Timer::content); hooks, dispatch, logout
  listeners and every other outside call are required Host services with no
  default; skills and special energy are native. The command, random event
  content and every session binding are not ported. See docs/content-timers.md.
- For the item, NPC, player, use-with, component use-with, slot switch,
  examine and track-finished packets (world/packet_handlers:
  processItemAction with InteractPlugin.handleItemOption, processNpcAction,
  processPlayerAction, processUseWith with NodeUsageEvent and
  UseWithHandler, processComponentUseWith, processSlotSwitch/switchItem, the
  examine getters, EquipHandler, DropListener) or their driver routes, run
  `cargo test -p rs09-server --test packet_handlers --test tick_driver` and
  `cargo xtask packet-handlers-oracle --check` with Python/JDK 11, plus the
  interaction_listeners, legacy_interaction, inventory, equipment, trade,
  spells, player_tabs and world_login suites. Preserve the 3,314 cases. Keep
  the second item.interaction read, the pulse's own-node target and its
  logging catch, the NPC listeners' NPC with the shown id, the ROOT player
  option name, the use-with flip and which misses clear the minimap, run's
  swallowed exceptions against the pulses' propagated ones, the live handler
  list and the PLAYER_TYPE array growth, "Too late!" after the clear, the
  strict magic delay, the tab drag's remove-and-add and `secondSlot - 1`,
  the examine cache with the note overwrite and "It's an Hans.", the casts
  of the equip slot/sound and unequip's plugin answer, and the drop's lock
  presence test and identity checks. Oracle adapter fields must not share a
  name with a getter's Kotlin property (the original reads getters). Every
  outside call is a required service with no default; EquipmentContainer.add
  is a service here (its body is inventory/equipment, not run with
  EquipHandler by any corpus); content listeners, plugins and
  UseWithHandler subclasses stay content. The live world binds the item,
  NPC, player, examine and track-finished handlers (docs/live-world.md);
  use-with, component use-with and slot switch are not bound there yet. See
  docs/packet-handlers.md.
- For deferred input/BankUtils or object attributes, reproduce input-prompt-oracle
  and affected ownership/attribute/bank/UI/world corpora; run input_prompt,
  owned_player, plain_dialogue and all three controlled examples. Preserve initial790
  and1131 observations; the current1137 cases include244/65/23 wire decoding.
  Modern RunScript.kt is the reference, not the old Java player/link class.
  Keep raw Int<=0 early returns, ASCII suffix regex after ROOT lowercasing,
  discarded callback result, preparation versus callback failures, two distinct
  finally cleanups, captured player and late bank lookups. Attribute set/remove
  event failures retain the completed map mutation. Object captures report each
  actual physical node/PIN/container/native-header/attribute-object field once;
  aliases are not duplicate object fields. No owner/attribute guard spans metadata
  or gameplay callbacks; exposed object values do not anchor their former owner.
  New opaque field kinds require tracing before storage. Arbitrary opaque-only
  object cycles/concurrent GC remain open. Kernels and recording services do not
  establish full gameplay parity. The controlled live binding independently verifies
 244/65/23, actual callbacks and210/132 error/Continue. Retained callbacks can still
  transfer after bank Close: original addItem/takeItem have no bank-open guard;
  late packets lose the current script and preserve stale parseamount. Keep fresh
  fresh action queue/content dialogue boundaries explicit; actual lock queries
  use canonical actor state. No natural banker/
  PIN/profile/capacity claim. See docs/input-prompt.md and docs/plain-dialogue.md.

- For DialogueInterpreter/DialoguePlugin/DialogueFile/EmptyPlugin/Topic changes,
  reproduce dialogue-oracle and plain-dialogue-oracle with Python/JDK11; run
  dialogue, plain_dialogue, input_prompt and interfaces. Preserve all1586 initial
  observations. world/dialogue is a library: no example or alpha installs it and
  no NPC/quest/skill content, DialogueBuilder, DialogueLabeller or
  SkillDialogueHandler is ported. Compile the complete unchanged classes; scripted
  oracle content may only call inherited original methods. Keep the END check on
  the player's current interpreter, the file captured before a topic runs,
  re-read dialogue fields after events, close's early true with a chatbox close
  event, the cleared dialogue before plugin close, overwrite-without-close in
  open and the single caught queue/pulse block. Entity forms without hide use the
  shown NPC, forms with hide use getId. Substitution keeps appendReplacement
  escapes and group references. Plugin helpers pass the caller's message array;
  file helpers pass a copy. Plain sending and splitting stay in plain_dialogue.
  The registry is an explicit world handle, not process-global state. Host
  services have no defaults and must not fabricate success; labels never select
  behaviour and no guard spans a host or content call. Plugin/file/interpreter
  back-references are strong and not yet traced by the canonical capture graph.
  Packet bytes, real UI/definition/NPC bindings, Continue and modern-context
  joins and all content remain required. See docs/dialogue.md.
- For plain dialogue or Continue changes, reproduce plain-dialogue-oracle and the
  affected input/interface/bank corpora; run plain_dialogue, input_prompt, interfaces,
  bank_interface and all three controlled examples. Preserve the initial424 cases
  and all430 current observations. Compare unchanged DialUtils plus exact selected
  sender/Continue bodies; retain mutable message arrays, getter order, late
  interpreter/queue/manager/listener/plugin reads and completed failure prefixes.
  Never expose Java adapter fields that bypass original Kotlin getter calls.
  Positive finite limits preserve UTF16/ASCII tags, float sizing, signed accumulator
  wrap, duplicate-tag removal and the original four-line truncation. Nonpositive
  limits and nonprogressing tagged resource-failure cases are explicit rejected
  boundaries, not full failure parity. No guard spans a callback. Controlled empty
  action queues do not establish full content dialogue. See docs/plain-dialogue.md.

- For BankInterface, InterfaceListeners or IfAction/CloseIface changes, reproduce
  bank-interface-oracle and all affected bank/ownership/UI/world corpora; run
  bank_interface/owned_player and all three controlled examples. Preserve the
  first336 and first559 observations byte-for-byte. Runtime callbacks are actual
  typed objects, including the shared stateless help function, never observation
  labels. Preserve specific-before-generic lookup, null versus false, function
  arity casts, late bank/component/definition reads, captured plugin/dispatch and
  completed failure prefixes. Count actual captured player fields once and do
  not span callbacks with guards. CloseIface must ignore close's boolean and use
  non-null attribute presence, including false/zero. Fresh fixture empty dialogue,
  locks/zones/events and one-shot --bank-probe admission remain explicit test
  boundaries. Banker/booth admission, PIN, help/deposit dialogue,
  full login/map/profile/content still need real bindings. Browser pointer and
  native protocol-driven rendering proofs must be labelled separately; neither
  proves native pointer input. See docs/bank-interface.md.

- For canonical bank runtime/listener/variable-archive bindings, run bank,
  owned_player, variables, interfaces and all ownership/player/world/ground suites
  plus all three examples; reproduce bank/variables/interfaces and affected JVM
  corpora. BankListener uses its actual captured player and passed container;
  bank-only tab/space reads resolve that player's current active bank. Keep packet
  and completed variable prefixes on failure; false varbit flags mean save=false,
  not suppressed sends. Ordinary inventory operations must dispatch installed
  bank listeners too. Foreign callback fields reject before target mutation.
  Bind immutable archive22 before canonical admission; exact-handle reuse is
  idempotent, replacements/late first binding fail, and missing service differs
  from an available archive's missing file. Real BankClose retains its bank and
  may differ from the caller. No guards span UI/listener/variable callbacks.
  The later BankInterface stage joins registration and basic live transactions;
  banker/booth admission, PIN entry, deposit rendering and full input services
  remain required. See docs/bank-runtime.md and docs/bank-interface.md.

- For InterfaceManager lifecycle/ownership changes, run interfaces/owned_player,
  all ownership/player/container/world/ground suites and all three examples;
  reproduce interfaces-oracle and affected JVM corpora. The complete original
  Manager class is the oracle, preserving both initial238 and typed351 cases.
  Preserve late opened/overlay/tab-array reads, veto/discard differences,
  reset-script attribute prefixes, nullable shared tabs, lookup priority, hidden
  fixed child offsets and signed packet counters. Each actual manager owns one
  fixed physical player field; exposed views must not add owner anchors. Count
  its field once even when shared, and trace single/overlay/wilderness fields.
  Reject foreign/rebound receiver contexts before any observable mutation.
  Full content/listener/bank/PIN/profile and close-packet admission services are
  still required; controlled empty event listeners do not establish game dispatch.
  See docs/interface-manager.md.

- For typed ComponentPlugin/CloseEvent or component capture graph changes, run
  interfaces/bank_pin/owned_player, all ownership/container/player/world/ground
  suites and all three examples; reproduce interfaces-oracle and affected existing
  JVM corpora. Preserve the first238 observations byte-for-byte and compare actual
  equal-label distinct objects, aliases, late lookup/replacement, callback reentry
  and failed prefixes. Runtime dispatch uses captured typed objects, never observer
  IDs. Count physical manager/tab/component/definition/callback/PIN/container fields
  once, distinguish aliases from separate fields, and preserve global-definition
  roots. Exposed components retain only players actually captured by their fields;
  stateless unclosable events use the close caller's attributes. Foreign PIN events
  fail before mutation. No guards span callbacks or side-effect-free capture
  metadata. NativeObject.captures must report all supported strong fields; new
  opaque field kinds need explicit tracing before use. Do not introduce owner
  anchors into their own graphs. Full listener/content/PIN/profile/bank
  runtime and arbitrary plugin GC remain pending; docs/component-objects.md.

- For BankPinManager/PinStatus/BankPinInterface changes, run bank_pin/owned_player
  and all existing ownership/player/container/world/ground suites plus all three
  examples; reproduce bank-pin-oracle and affected JVM corpora. Preserve the855
  original cases and independently pinned initial836 observations, UTF16 text,
  captured dispatch/interface/close receivers, clock-call order/int narrowing,
  shuffle RNG state, try/recovery quirks and failed/reentrant prefixes. Each actual
  manager owns one player field; each distinct close event owns one manager field,
  while aliases keep the same event object. Count these physical edges correctly
  when joining real component graphs; internal UI cycles must not become roots.
  No guards span callbacks. Required UI/packet/audio/GE/world services must not
  fabricate success. Full close/plugin/profile/content runtime and live PIN/bank
  acceptance remain open until independently joined and verified; docs/bank-pin.md.

- For player variables, varbit definitions/loaders or canonical variable bindings,
  run variables/owned_player, portable SQL integer-map tests, all ownership/player/
  world/ground suites and all three examples. Reproduce variables-oracle and the
  affected existing JVM corpora. Preserve actual public primitive maps, sticky
  save flags, mutation-before-send failures, signed varc narrowing/shared counter,
  reinit's captured map/order/modCount and retained clear capacity. Definitions
  publish only after successful decode; old aliases survive replacement and
  getValue resolves the current ID mapping. Custom loader failures preserve reader
  and completed-row prefixes. No guards span callbacks. VarpManager's real final
  player field is one graph edge; raw maps do not retain the player. Reject foreign
  worlds before entry and never deliver old captures to a reused wire slot. Full
  profiles, AIPlayer/session/cipher behavior, FileReader cursor/default charset,
  arbitrary exposed registry iteration and live banking remain open; variables.md.

- For BankContainer or player item ownership changes, run bank/owned_player and
  all existing ownership/container/listener/equipment/player/ground suites plus
  both controlled examples; reproduce bank-oracle and affected existing JVM
  corpora. Preserve the initial488 bank observations, open overload/listener
  differences, captured close targets, charge/note conversion, virtual maximum
  add, tab arithmetic and failed/reentrant prefixes. Banks own a real player
  field and final listener separate from the exposed list: trace both physical
  fields once even after clearing listeners. Primitive tab arrays do not retain
  players. No guards span callbacks. Use the actual active primary/secondary
  bank in inventory/bank/equipment ownership queries; exclude drop log. Preserve
  the explicit external ConstLib range-resolution provenance. Full bank runtime
  services/PIN/profile/UI/content/live admission remain pending; docs/bank.md.

- The existing parent project is separate. Keep changes in this workspace.
- `upstream.lock.json` defines the behavioral reference. Never silently update it.
- Keep reference source unmodified. JVM oracle adapters belong under `oracle/`.
- Describe the exact scope of every compatibility claim. Never call a partial
  subsystem a complete server, visual client, or gameplay replacement.
- Add JVM differential fixtures and regression tests for ported behavior. Generated
  fixtures record source hashes; reference changes require explicit review.
- Preserve valid-input quirks, integer wrapping, update order, and default settings.
  Bounds checks may reject malformed inputs, with the difference documented.
- Do not change XP rates, timing, drops, economy, or game rules for performance.
- Profile actual workloads. Benchmark reports must identify reference/native build,
  corpus, repetitions, checksums, and what costs they include.
- Runtime game logic must use Rust. Java is permitted only as the test oracle during
  migration. Do not hide an embedded JVM behind a native-server completion claim.
- Keep simulation independent of graphics/network/model inference and deterministic
  under controlled inputs. Verify parallel behavior against the serial reference.
- Run `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, and `cargo xtask oracle --check` before reporting a
  verified milestone. Record unavailable checks and remaining parity work.
  The RT4 primitive `cargo xtask oracle --check` uses the existing JDK25 toolchain;
  server Kotlin/JVM corpora select JDK11 explicitly. JDK11's generated GZIP test
  inputs differ in ten OS-header bytes. Preserve the committed fixtures; never
  regenerate them to work around choosing the wrong oracle toolchain.
- For local Windows verification set CARGO_INCREMENTAL=0, CARGO_BUILD_JOBS=2,
  CARGO_PROFILE_DEV_DEBUG=0 and CARGO_PROFILE_TEST_DEBUG=0. Full default debug
  symbols previously exhausted the system volume. Explicit symbol debugging may
  override these defaults when there is sufficient space. Preserve interrupted
  verification logs and existing recovery artifacts; do not delete them for space.
- For cache changes, also run cargo xtask cache-oracle --check; when assets are
  available, run cargo run --release -p xtask -- cache-audit --jvm. Client fallback
  behavior and strict server errors are distinct contracts. Expose fallback
  diagnostics rather than claiming undecipherable data decoded successfully.
- For transport changes, run cargo xtask network-oracle --check and the native
  transport tests. With pinned cache assets, also run network-interop and
  network-smoke (build rs09-headless in the same profile first). Cache connection
  counts do not establish gameplay player capacity.
  Respect the user's stopped500-client run: local transport smoke must pass
  `network-smoke --clients 4 --output NEW_EMPTY_DIRECTORY`. The configurable
  cache-only workload is separate from gameplay; never restart500 game clients.
- For login changes, run the login differential/transport tests and
  `cargo xtask login-oracle --check` with Python 3.10+ and JDK 11. Also run
  `cargo xtask login-interop`; it uses portable committed cache fixtures and
  verifies original/native codec sessions in both directions. Compiler artifacts
  are pinned under oracle/login/kotlin.lock.json and never enter runtime packaging.
- For account/password changes, also run the server auth tests,
  `cargo xtask auth-oracle --check` and `cargo xtask auth-interop`. Use the same
  Python/JDK 11 selection as the login oracle. Account clock/entropy adapters
  must remain explicit; do not infer SQL or world-entry parity from these tests.
- For SQL storage changes, also run `cargo test -p xtask --test sql` and
  `cargo xtask sql-oracle --check` with Python/JDK 11 and OpenSSL on Windows.
  This starts owned, temporary MariaDB instances and compares the complete
  original provider against Rust over plaintext and TLS. Never redirect its
  schema-reset commands to a user database or infer world-entry parity from it.
- For configuration changes, also run `cargo test -p rs09-server --test config`
  and `cargo xtask config-oracle --check` with Python/JDK 11 on Windows. Preserve
  initial-vs-parsed defaults, ordered partial assignments, packed coordinates,
  original path and TOML string quirks. JVM exits/version/logging are explicit
  oracle adapters; the native runtime must never invoke Java to load settings.
- For player metadata changes, run `cargo test -p rs09-server --test player`
  and `cargo xtask player-oracle --check` with Python/JDK 11. Preserve account
  aliasing, Java contact iteration, nullable ranks, partial parse/save mutations
  and clock boundaries. The oracle's world adapters do not prove playable login.
- For saved-data or Java numeric compatibility changes, run
  `cargo test -p rs09-server --test saved_data` and
  `cargo xtask saved-data-oracle --check` with Python/JDK 11. Preserve record
  defaults, parse mutation order, signed widths, nullable items and legacy double
  output. Record adapters do not establish complete profile or item gameplay parity.
- For attribute persistence changes, run `cargo test -p rs09-server --test attributes`
  and `cargo xtask attributes-oracle --check` with Python/JDK 11. Preserve prefix
  quirks, nullable values, duplicate saved keys, expiration sampling and failed-load
  mutation order. XML fixtures use original DOM-produced observations and file
  bytes in isolated temporary directories, never user save files. This does not
  establish complete profile loading, Entity event dispatch or general XML parity.
- For skill changes, run `cargo test -p rs09-server --test skills` and
  `cargo xtask skills-oracle --check` with Python/JDK 11. Preserve fractional XP
  boundaries, integer overflow/saturation, callback/error ordering, selective copy
  and row-position-based profile loading. Entity/content bindings are explicit
  adapters, not proof of a complete assist/perk/gear/world or packet subsystem.
  The reference XP-by-level loop is unbounded for huge positive levels; use the
  verified saturation prefix rather than sending billion-iteration oracle cases.
- For gathering tables, tools, woodcutting, mining, fishing or the gathering
  binding (world/gathering), run `cargo test -p rs09-server --test gathering`,
  `cargo xtask gathering-oracle --check` with Python/JDK 11 (it reproduces both
  corpora) and `python tools/port_gathering.py --check`; for binding.rs also
  run the skills, inventory, container_ownership, container_listeners and
  item_definitions suites. Preserve all 10,504 cases; the first 9,281 lines
  are the woodcutting/mining corpus byte for byte (the test pins their
  SHA-256), so new cases are appended, never inserted. Preserve the 362
  binding cases. Every outside call is a required service in the
  original order; never add defaults or perform one inside the library. Keep
  the four generators distinct (shared RANDOM, Math.random, thread-local,
  Collections.shuffle) and their draw counts; the pinned JDK 11 thread-local
  nextDouble can return its bound. The listener and the legacy pulse differ
  (sound draw order, adze roll, bark XP, felling identifiers, Varrock
  thresholds, essence shortcut, message channel): keep each as its source has
  it. The gem-rock table is shared state shuffled by every roll. Fishing:
  FishingListener and FishingPulse differ too (hasSpaceFor on the API versus
  the container, roll(20) after versus random(100) <= 5 before the catch,
  incrementAttribute versus get/set, rewardXP versus the three-argument
  addExperience, barehand rules, the debug line, skill tasks, Location
  identity). Keep rollFish's draw order (randomize(100) for the big net, one
  shared nextDouble per eligible fish, `<`), the stripy-feather question per
  lure fish, hasBait's short circuit, removeBait from the last bait, the
  repeated name lookups in the two dialogue texts, the pulse's first reward
  that only sets the delay, and the never-finishing barehand Pulse(1) whose
  random(1) is never one. Item names, the en-US default locale, the adapter
  world state, player direction as a step vector and the modelled per-tick
  driver are oracle inputs, not ports. binding::Host answers skill, inventory,
  equipment-slot and item-name requests from native player::skills,
  inventory::container and the definition registry with the pinned ContentAPI
  bodies; its corpus runs the unmodified sources over the original Skills,
  Container and Item. Do not answer any other service there: scripts/clocks,
  pulses, the listener registry, attributes, messages, scenery, inEquipment,
  bank/beast-of-burden containers and EquipmentContainer's own overrides
  remain required services, and removeItem removes one default-charge unit
  because the service carries only the id. Barbarian fishing, pickaxe repair,
  spot movement and the FishingPulse option handlers are not ported; nothing is
  installed in a live world and the examples' bronze-axe stand-in is
  unrelated. See docs/gathering.md.
- For item-definition changes, run `cargo test -p rs09-server --test item_definitions`
  and `cargo xtask item-oracle --check` with Python/JDK 11. Preserve decoder stop
  positions, signed-byte strings, missing defaults, ascending template order,
  self-reference effects, shared arrays/maps and cloned action arrays. With pinned
  assets also run `cargo run --release -p xtask -- item-audit`; this compares every
  field and alias from the strict native archive loader with original methods.
  The pre-content cache definitions do not establish complete items, configuration,
  inventory/equipment or gameplay. `item-samples` intentionally changes inputs;
  regenerate/review fixtures only after checking the pinned cache provenance.
  JDK case-comparison data is authored with `tools/port_jdk_case.py`; use its
  `--check` mode to validate it. Java casing compares UTF-16 code units, not Rust's
  current Unicode scalar mappings.
- For item-instance/container changes, run `cargo test -p rs09-server --test inventory`
  and `cargo xtask inventory-oracle --check` with Python/JDK 11. Preserve Java
  wrapping, constructor/setter differences, shared items/events/arrays, ascending
  slot order, ID-vs-hash differences, partial loads/mutations, listener reentrancy,
  callback failures and ArrayList iterator checks. The factory owns one mutable
  NULL_ITEM per world. Do not replace deferred event handles with value snapshots.
  The complete original classes compile in this oracle, but Node/world, concrete
  item plugins, degradation and ground spawning are explicit external adapters;
  the tests do not establish live inventory/bank/equipment or player integration.
  Changes to shared integer/String map ordering also require the portable SQL
  collection tests; use original JVM collision/resize/tree-bin observations.
- Container is a shared object handle. Clone must retain current/replaced arrays,
  events and the same live listener list; never snapshot its fields for a retained
  receiver. Use listener_list for the real exposed ArrayList, listeners only for
  diagnostic values. Structural edits change modCount; set does not. Preserve
  captured iterators, late element/event reads and reset/failure prefixes. Run
  container_ownership and fresh container-ownership-oracle as well as the 389-case
  inventory baseline, whose bytes must remain unchanged. Containers retain real
  native ContainerListener objects, never labels or registry observation IDs.
  Preserve identity even for equal payloads, aliases and nonstructural replacement.
  Clone the actual typed receiver before invoking it; no guards span callbacks.
  Also run container_listeners and container-listeners-oracle, preserving both
  prior389/207 corpora. Full Player/backreference graph collection remains open;
  typed objects and weak receiver retirement alone do not establish complete GC.
- Canonical actors own the currently bound player fields in player/owned.rs.
  Bind once and reject foreign domains/leases before entering operations. Never
  resolve retained Player/InventoryListener receivers through UI metadata or a
  recycled wire index. Base inventory views follow actual listener edges; clearing
  their list can release the player. Settings/interface/mask views retain their
  actual owner. Count each physical node field once and deduplicate shared lists,
  objects and mask contexts when tracing roots; detached listener cycles must free.
  NativeListener.retained_nodes is side-effect-free ownership metadata and must
  report every stored strong canonical node field. No field guards span it.
  Do not store an exposed view's owner anchor inside that owner's graph: the
  stored appearance payload uses an unowned internal edge to the same body data.
  Same-world InventoryListeners can update another container: packet/event/pouch
  reads use that container, while weight uses the listener's actual player's own
  inventory/equipment/counter/body. Foreign worlds fail before mutation. Run
  owned_player, all ownership/actor/ground/mask suites, headless and all examples;
  reproduce owned-player/player-inventory/inventory-listener/container-listeners/
  container-ownership/core-profile/settings and affected world/mask oracles. The
  88-case original distinct-player corpus uses actual original writers and keeps
  the 184-case baseline unchanged. Full Player/profile/save, remaining Settings/
  skills/Animator joins, equipment/bank/content and concurrent GC remain open.
- For captured player inventory/Settings bindings, also run player_inventory,
  inventory_listener/settings/core_data, shared_ownership, prepared_masks/animator,
  player_walk/world_update and ground suites, headless and all three examples.
  Reproduce player-inventory/inventory-listener/settings/core-profile/inventory/
  container-ownership and affected world/ground oracles. The actual captured
  listener, containers, settings, body, familiar flag and packet counter must
  survive metadata retirement without rebinding by wire index. Reject foreign
  worlds before mutating fields. Frame delivery must verify the canonical node
  still occupies that wire index. No row or field guard spans callbacks. Keep
  bank/equipment subclasses, full profiles/saves/content, complete listener GC
  and authoritative Take explicit until they are independently bound.
- For InventoryListener changes, run inventory_listener/inventory/settings tests
  and inventory-listener-oracle, inventory-oracle and game-packets-oracle checks.
  Also check tools/port_summoning_pouch_ids.py. Preserve send-before-weight order,
  retained packet arrays versus captured ascending event slots, world NULL_ITEM
  reference identity, live container scans after callbacks, the two distinct
  familiar-manager lookups and appearance sync only on a captured-state change.
  Callback errors retain completed prefixes; no guards span service callbacks.
  Pouch key membership is compared with the unchanged full enum over all65536
  representable Item IDs. Player/weight/familiar/appearance/session services and
  complete pouch gameplay remain required world bindings, not live Take proof.
- For EquipmentContainer/EquipmentListener or generic two-argument add dispatch,
  run equipment, inventory/container_ownership/container_listeners, owned_player,
  player_inventory/inventory_listener, settings/core_data/shared_ownership and
  affected world/mask/headless/examples. Reproduce equipment-oracle and the prior
  inventory/ownership/listener/player/Settings/core-profile/item/config/writer
  corpora. Preserve the base one/three-argument versus virtual two-argument
  overloads, removal/hook/rollback mutations, forced stack refresh, original
  input index/name, live cached definitions, slots captured before packets,
  missing-extension break and bonus narrowing. Equipment's own player field
  remains a traced edge after clearing listeners; count it once per object.
  Native plugin payloads retain object identity and never dispatch by labels.
  No guards span callbacks. Full player weapon/combat/appearance/requirements/
  perk services, plugin GC and live Equip remain explicit until independently
  bound; development admission still uses base equipment data. See docs/equipment.md.
- For player inventory binding, run player_inventory/inventory_listener/settings/
  inventory/interfaces/player_render/rebuild tests, both live examples, headless
  tests and fresh player-inventory/inventory-listener/settings/interfaces/game-
  packets/player-render/appearance oracles. Ground-world definition ownership
  changes also require its ground/chunk/viewport/pulse suites and prior gates.
  Use actual cached Item definitions, not a lookup by the item's current ID;
  charge changes can leave a different cached definition. Weight ignores amount,
  skips nonpositive inventory weights and includes signed equipment weights only
  in slots0..10. Preserve send-before-weight, integer-cast text and shared UI
  counter order, event/array aliases, callback reentry and failed prefixes.
  No registry/field guard may span a host callback. The loaded definition table
  is shared; mutable per-definition handles remain live. The fresh-player probe
  drops its own listener lease on disconnect and exposes no retained old profile
  captures. This does not complete equipment/familiar/profile/content lifecycle,
  late render-phase masks or authoritative Take/Drop. Headless histories retain
  decoded wire events and label bytes; do not invent precise weight or UI state.
- For item-configuration/property changes, run `cargo test -p rs09-server --test item_config`
  and `cargo xtask item-config-oracle --check` with Python/JDK 11. Preserve typed
  arrays/maps/constructor objects, JSON-simple grammar and coercions, HashMap field
  order, partial requirement writes and failed-reload state. With pinned assets,
  also run release `item-config-audit` and `item-audit`; these verify all original
  configuration rows and raw definitions. Native metadata/constructor states do
  not establish full animation, content/plugin, equipment or world behavior.
  Check authored constants with `tools/port_item_config.py --check`. Audit JSON
  contains literal UTF-8 names; Python readers must select UTF-8 explicitly.
- For timer/core-profile changes, run `cargo test -p rs09-server --test timers
  --test core_data` and `cargo xtask core-profile-oracle --check` with Python/JDK 11.
  The composite oracle also reproduces timer and inventory corpora. Preserve
  queued activation/removal, clock-call order, fail-fast iterators, callback
  reentrancy, remaining-time overflow, partial loads and detached save publication.
  Concrete content timers, registry/entity-startup wiring, actual inventory/bank/
  equipment listeners, full profiles/file I/O and world scheduling are separate
  work. Constructor/clock/registry adapters never establish playable world entry.
- For the whole character save file (player/save.rs and save/parser.rs, order.rs)
  or the state behind save::Host (player/persisted), run
  `cargo test -p rs09-server --test player_save --test player_state` and
  `cargo xtask player-save-oracle --check` with Python/JDK 11; the composite also
  reproduces the timer and inventory corpora. Also run the suites of the section
  libraries it joins: core_data/timers, attributes, skills, settings, saved_data,
  player_appearance, bank_pin and ironman. Preserve root and nested put order
  (a JSONObject is a HashMap, so put order is part of the bytes), the integer
  `version` and familiar `lifepoints` beside otherwise stringified numbers,
  section order in both directions, casts before mutation, completed prefixes on
  failure, Settings.isAcceptAid's ironman rule, one clock read per PIN delay
  comparison, per-hook `saveFile!!`, and the JDK's single replacement of an
  encoded surrogate. Systems without a native port stay behind save::Host as
  required services: never give them success defaults and never model them in
  this module. The 2,637 original cases compile PlayerSaver.kt and
  PlayerSaveParser.kt unchanged, but Skills, Settings, Appearance and
  BankPinManager contribute verbatim selected members there, and
  kotlinx.coroutines is a same-thread stand-in. 267 appended state cases
  compile both files again over the whole quest, music, emote, diary, pouch
  and POH-storage classes; persisted::host::Native answers those Host
  sections from native state, while house/familiar parsing, content hooks,
  events and logging stay required services. Keep the first 2,637 fixture
  lines byte for byte (player_state pins their SHA-256), append new cases,
  and keep the brand-new-character file byte for byte. Not covered: the Java-below-11
  script-engine output, default-charset files, unpaired UTF-16 escapes (rejected
  natively), a null save path, most I/O failures, concurrent saves, and every
  login, logout and playable-alpha call site; the alpha still writes its own
  snapshot. See docs/player-save.md.
- For the canonical player's profile state (player/owned/profile.rs: details,
  skills, saved data, timers, Properties in player/properties.rs, spell book,
  persisted state, Player's scalar fields) or its save/load binding, run
  `cargo test -p rs09-server --test canonical_player --test owned_player
  --test player_save --test player_state`, all ownership suites and all three
  examples, and reproduce `cargo xtask canonical-player-oracle --check` and
  player-save-oracle with Python/JDK 11. The oracle reuses the world-trace
  build and is accepted only with a byte-identical boot record; never edit the
  world-trace harness for it. Keep: profile values store no node handle (the
  original's entity back-references are implicit), operations take the profile
  out and hold no player lock across services (reentry gets InUse), save.rs
  and parser.rs unchanged with core_data projected from the player's own
  containers, tabs, selector and variable store, and native answers only for
  what the player owns (retaliating, combat levels, Settings' Properties and
  ironman). Every NOT_HELD field and KNOWN difference in the test is listed in
  docs/canonical-player.md; a new field must be compared or listed. Entity
  location stays the actor's (save argument, load result). Login, Player.init,
  LoginConfiguration and the tick driver are not ported here.
- For timer-registry or ROOT string-case changes, also run the timer_registry and
  xtask string_case tests, `cargo xtask timer-registry-oracle --check`,
  `cargo xtask string-case-oracle --check` with Python/JDK 11, and
  `python tools/port_jdk_lower.py --check`. Preserve prototype identity, first type
  match in HashMap order, nullable/typed spawn differences, argument dispatch,
  duplicate registration logs, automatic-list fail-fast/reentrancy, and JDK 11
  contextual ROOT lowercasing. Factory/argument/subtype bindings are native content
  boundaries; default-locale casing and complete world entity startup are separate.
- For pulses, entity pulse manager or world pulse queue changes, run
  `cargo test -p rs09-server --test pulses` and `cargo xtask pulses-oracle --check`
  with Python/JDK 11. Preserve base virtual dispatch, wrapping ticks, check-array
  aliases, stopped-slot priority, script-removal order, strong interruptions,
  HashMap.forEach's late modification check, fixed queue prefix, exception and
  telemetry ordering. Enum identity hashes and wall clock are explicit oracle/
  replay inputs. Parallel producer tests do not establish parallel game simulation
  or player capacity. Concrete movement/combat/skill pulses, scripts, world startup,
  the 600ms tick driver, telemetry storage and JVM thread interruption remain separate.
- For entity scripts, run scripts/script_objects/script_runtime/owned_player and affected
  ownership/player/world suites plus all three controlled examples; reproduce
  scripts-oracle/script-runtime-oracle and affected interaction/pulse/player-walk corpora with Python/
  JDK11. Preserve initial295 observations and all328 current cases. Retain the
  actual shared processor identity and immutable owner/world, actual native
  function/node fields, capture-before-invoke and alias/reentry behavior. Numeric
  executor IDs belong only to recording adapters. Trace physical fields once,
  count incoming aliases separately and collect detached typed script cycles;
  external/foreign views retain their exact local captures. Never clear foreign
  or unscoped objects, and drop payloads outside guards. Validate actual world/
  allocation before runtime admission; equal numeric IDs are insufficient.
  All actors, including bare players and NPCs, share Entity.scripts with later
  Player admission through a weak allocation-keyed index. Trace the actor and
  Player's separate physical references to that same processor. Expired weak
  entries are swept during collection, not on each admission. Retained primitive
  clocks do not anchor actors; prepared animators share the actual entity array.
  Preserve1064 original modal/clock/center helper observations and real nested
  native queue/tick regressions. Callback bodies use actual capture fields and
  Rust function pointers; validate nested node/PIN/container/header/attribute/
  processor/script captures and arguments before body effects. No guards span
  metadata, callbacks or destructors. Keep required UI closure/dialogue, combat
  actions and modern arrival/content services explicit.
  Preserve retained
  current-script handles, callback execution/equality, indexed queue mutation,
  strength-argument versus stored-strength differences, strict interaction versus
  inclusive queue timing, movement-phase short circuits, shared locations and
  partial resets/failures. Remaining interface/content bindings
  are explicit; do not infer concrete content, event dispatch, full pathfinding,
  player/world startup or packet delivery. Avoid unbounded oracle preMovement
  loops when ticking clocks or wrapped deadlines keep repeating scripts due.
- For gameplay packet changes, run `cargo test -p xtask --test game_packets` and
  `cargo xtask game-packets-oracle --check` with Python/JDK 11. Preserve signed
  plain shorts versus unsigned transformed fields, final walk offsets, ignored
  fields/tails, original packet lengths, counter overflow, window reset-byte
  differences, inventory sentinel/amount formats and chunk-relative locations.
  The bounded framer may reject malformed/oversized messages. Numeric action
  decoding and field reads do not establish authoritative action handling,
  collision changes, UI reducers, login/world startup or playable sessions.
- For door rules/configuration, run `cargo test -p rs09-server --test doors` and
  `cargo xtask doors-oracle --check` with Python/JDK 11. Preserve ordered forward/
  reverse configuration overwrites, retained handles and partial load failures;
  west/east/south/north lookup priority, wrapper/constructed differences,
  diagonal/fence/special-ID geometry, signed remainder, captured logout locations,
  charge/pulse/audio/diary ordering and callback failures. Scene replacements are
  explicit bindings: this corpus does not establish SceneryBuilder, collision,
  walking, audio delivery, live world sessions or a full Scenery/Node replacement.
- For scenery replacement/restoration, run `cargo test -p rs09-server --test scenery`
  and `cargo xtask scenery-oracle --check` with Python/JDK 11. Preserve wrapper
  identity, constructed/replaced handles, stop/submit order, partial failures,
  destruction callbacks, captured versus live pulse references and late object
  field reads when serializing updates. Landscape storage/chunk selection and
  ground spawning are explicit adapters; this is not live world integration.
- For server collision flags, run `cargo test -p rs09-server --test clipping`
  and `cargo xtask clipping-oracle --check` with Python/JDK 11. Preserve default
  movement/projectile values, signed wrapping, flat array indexing, border calls,
  movement-before-projectile order and partial callback failures. Server flags
  differ from RT4 CollisionMap. Region loading/routing and mapscape application
  remain separate; malformed object rectangles are bounded at 65,536 writes.
- For chunk scenery storage, also run `cargo test -p rs09-server --test chunk`
  and `cargo xtask chunk-oracle --check` with Python/JDK 11. Preserve four-slot
  capacity, static/dynamic object identity, duplicate rules, retained locations,
  copy/rebase order and the static write before constructor failure. Item/entity
  lists, dynamic-region rotation and chunk synchronization remain separate work.
- For landscape placement/removal/decoding, run `cargo test -p rs09-server
  --test landscape` and `cargo xtask landscape-oracle --check` with Python/JDK 11.
  Preserve constructor-retained definitions, storage-before-clipping order,
  argument-vs-current definition use on removal, floor markers, projectile phases,
  bridge-plane adjustment, object-count mutation and smart-stream read positions.
  Region load/storage/routing, child-definition actions and full cache definition
  decoding are explicit oracle bindings; live world sessions remain separate.
- For server scenery definitions, run `cargo test -p rs09-server --test
  scenery_definitions`, `cargo xtask scenery-definition-oracle --check` with
  Python/JDK 11, and `python tools/port_scenery_schema.py --check`. With pinned
  assets also run release `scenery-definition-audit`. Preserve signed-byte
  strings, original decoder stop/log behavior, child placeholder/reload identity,
  configuration order, missing-file versus lookup defaults, array/map aliases,
  manual ID changes and child/action selection. Player values, logs and concrete
  plugins are explicit bindings. `scenery-definition-samples` changes authored
  inputs; regenerate/review fixtures only after verifying the pinned cache.
- For terrain/mapscape parsing and floor clipping, run `cargo test -p
  rs09-server --test mapscape` and `cargo xtask mapscape-oracle --check` with
  Python/JDK 11. Preserve z/x/y parsing, z/chunk-x/chunk-y/x/y clipping, unknown
  flag normalization without clearing existing wall/object bits, bridge-plane
  changes, separate source/destination chunk tables, null chunk consumption,
  captured landscape references and partial errors. Canonical hashes include
  every map byte, both chunk tables/landscape arrays and allocated movement
  flags; getter/flag call counts are also compared. Region loading/routing and
  dynamic-region ownership are explicit adapters, not a live world proof.
- For scenery construction/morph ownership, run the scenery_node tests and
  `cargo xtask scenery-node-oracle --check` with Python/JDK 11. Preserve retained
  definitions/names/sizes, frozen direction, child/wrapper identity, shared child
  arrays, default/config selection and live callback order. Child views keep the
  family alive without parent/child Arc cycles; test release after traversal.
  Node interaction dispatch, mutable/reflection child arrays and malformed cyclic
  graphs remain separate. Shared handle changes also require item/attribute tests.
- For Region.load ordering, run region_load tests and
  `cargo xtask region-load-oracle --check` with Python/JDK 11. Preserve early
  returns, loaded/hasFlags publication, walking-only marker resets, nested loads,
  landscape exception recovery and enhanced-for list modification checks. Cache/
  parser/service adapters in this corpus do not establish integrated world entry.
- For native scene integration, run scene tests and
  `cargo xtask scene-oracle --check` with Python/JDK 11. With pinned assets also
  run release `scene-audit`. Preserve original static/dynamic object identity,
  region-border collision, late packet fields, stopped/removed pulse behavior
  and 500-tick restoration. The portable joined corpus uses synthetic cache
  terrain/land streams; the whole-data audit uses real pinned Lumbridge bytes.
  Player movement/path/audio services, chunk fanout, live world delivery and
  600ms scheduling remain separate. Exported native packets are trace evidence,
  not live world-entry or player-capacity evidence.
- For player render/rebuild changes, run player_render and rebuild tests and
  `cargo xtask player-render-oracle --check` with Python/JDK 11. Preserve ordered
  local membership, LinkedList iterator checks, the nine-addition limit, maximum
  255 locals, cached appearance stamps, mask ordering, partial callbacks and the
  original normal-rebuild key order/location publication. Mask contents/spatial
  enumeration in that corpus are explicit services, not a full Player claim.
- For appearance field encoding, run appearance tests and
  `cargo xtask appearance-oracle --check` with Python/JDK 11. Preserve transformed
  length, prepared equipment indices versus item IDs, NPC/gender/size/color fields,
  UTF-16 Base37 names and PvP/summoning level branches. Naked defaults are compared
  with original body preparation; equipped-body/content bindings remain separate.
- For full Appearance/BodyPart/Gender or configured/runtime Animation identity,
  run player_appearance, owned_player/equipment/container ownership/listeners,
  item_config/NPC definitions/init, animator/prepared_masks/appearance/player_render,
  affected world/headless and all examples. Reproduce player-appearance-oracle,
  animator/item-config/NPC-definition and affected existing JVM corpora. Preserve
  real per-domain defaults and animation objects/cached definitions; nullable
  array elements, raw array identity, part aliases and getters; parse/copy/failed
  constructor prefixes; separate captured render/property/attribute/appearance
  lookups; and prepared=true before all equipment queries. Appearance's player
  field is a counted graph edge. Detached parts/arrays do not retain the player;
  the production render_info view does. No guards span callbacks. The 448-case
  whole-class corpus uses recording services and retained observation identities.
  The late field writer is player/appearance_writer.rs. Changes require its
  appearance_writer tests, fresh appearance-writer-oracle and appearance-oracle,
  player_appearance/prepared_masks/update_masks/player_render/animator, headless,
  all examples and the workspace checks. Preserve the Kotlin buffer-null check
  before context dispatch, one captured Appearance, distinct skull/size/world/
  properties/familiar lookups, five captured color references, actual render
  override callbacks, reentry and failed bytes/cursor/header patches. No guards
  span callbacks. Its 604 original cases retain the first 468 byte-for-byte and
  cross-check 127 older original field encodings. Recording Player adapters must
  not expose a field with the same name as a synthetic Kotlin getter property;
  validate the real reference layout and require the getter events. Always use
  explicit UTF-8 for fixture/source edits on Windows. The growing byte-output
  subset is not a full IoBuffer replacement. Actual Properties/skull/familiar/
  skills/player/provider bindings remain required before replacing the prepared
  transport publisher or claiming accepted Equip. See docs/appearance-writer.md.
  Late writer runtime, properties/attributes/skills/familiar/profile/content bindings
  and accepted Equip remain open; do not substitute a cached DTO projection for
  original late getter/encoding order. See docs/player-appearance.md.
- For world-session stream changes, run world_session and existing login/transport
  tests plus the network/login oracle/interop gates above. Success requires an
  explicit ready native world; default startup still reports unavailable. Preserve
  coalesced post-login bytes, independent bounded directions, early online checks,
  original nonce detail, disconnect cancellation and queue-budget release. The
  stationary player_session example is a transport fixture, not full profile,
  Player.init, movement, NPC/content, bank/shop or capacity evidence.
- For walking-queue changes, run walking tests and `cargo xtask walking-oracle
  --check` with Python/JDK 11. Preserve point/Location aliases, origin consumption,
  walk/run ordinal versus direction value, run-disable/depletion behavior, energy
  and perk/timer order, teleport/scene publication, zone vetoes and partial callback
  failures. Route cleanup retains the captured ArrayList iterator across callback
  replacement/mutation. Explicit Entity/Settings/timer/zone/item/packet adapters
  do not establish pathfinding, collision-aware routing, original world phases or
  playable live movement. Malformed addPath expansion is bounded to 65,536 points.
- For server pathfinding/reach changes, run pathfinding tests and
  `cargo xtask pathfinding-oracle --check` with Python/JDK 11, plus
  `python tools/port_reach.py --check`. Preserve A*/BFS selection, Java heap tie
  order, ring/via/cost mutations, callback query ordering, caught supplier errors,
  fallback/truncation/backtracking quirks, mutable Location reads and the sized
  reach helper's direct RegionManager access. Generated predicates use wrapping
  i32 arithmetic. Node destination dispatch, Path.walk, MovementPulse and actual
  scene/world integration remain separate; native malformed iteration is bounded.
- For player Animate/SpotAnim payload or AudioPacket.write field changes, also run
  appearance tests and appearance-oracle. That corpus now includes animation and
  spot payloads and global/area sound wire bytes. Preserve wrapping addition in
  `(height << 16) + delay`, transformed short/int bytes, the extended mask header
  and Animate-before-Appearance-before-SpotAnim field order. The explicit
  animation_probe example is controlled
  rendering/audio input, not an emote handler or gameplay proof. Its composition
  regression runs with `cargo test -p rs09-server --example player_session`.
  Shared fixture actor changes also require player_render/rebuild tests and both
  movement_session/npc_session example targets. Headless spot observations retain
  exact decoded slots/packed words and bounded raw PLAYER_INFO payload history;
  run rs09-headless tests and verify TCP/WebSocket self and nearby masks. Clearing
  a transient server mask is distinct from sending the explicit ID65535 clear.
- For MovementPulse/DestinationFlag/truncateLoc changes, run movement tests and
  `cargo xtask movement-oracle --check` with Python/JDK 11. Preserve live pulse
  handles/Location aliases, constructor replacement, base checks versus current
  destination, early region gating, weak/normal script order, callback reentrancy,
  moving/combat targets, exact truncation equality, near/interaction exception
  paths and partial path consumption. Node IDs must remain bound to the same
  nodes for the lifetime of a pulse and its captured checks. Entity, queue,
  pathfinder, content and packet services are explicit oracle boundaries; this
  does not establish live movement or complete world integration.
- For Node destination dispatch or native navigation bindings, run path_dispatch,
  navigation, walking, movement, pathfinding and scene tests, plus fresh
  `cargo xtask path-dispatch-oracle --check` and `cargo xtask navigation-oracle
  --check` with Python/JDK 11. Preserve selected-versus-global collision suppliers,
  late destination reads, Java flag rotation, canonical queue/Location aliases,
  energy/direction publication and reentrant mutations. The joined replay uses
  explicit command phases and external entity/script/zone/content services; it
  does not prove complete world phases or live walk handling. SMART is currently
  executable; remaining finders must report an explicit error rather than fall
  back. Same-workspace recursive search behavior remains unverified.
- For walk requests, player-walk assembly or tick phase changes, run walk_request,
  tick, player_walk, pulses, walking, movement, pathfinding, navigation and scene
  tests plus `cargo test -p rs09-server --example movement_session`. Reproduce
  walk-request-oracle, tick-oracle and player-walk-oracle with Python/JDK 11.
  Preserve handler short circuits, strict request versus inclusive pulse distance
  checks, arrival capture, fixed runner prefixes, script/queue phase order and
  catch/finally flush behavior. Retained movement/base/check/queue/manager/node
  handles keep original node identities alive; detached internal edges must not
  retain cycles. Shared ownership helpers also require inventory/attribute tests.
  The controlled movement_session uses real pinned cache collision and native
  walking, with explicitly empty content services. It is not a complete Player,
  profile, UpdateSequence, region-activity, script/zone/content or production world
  replacement. Keep ordinary/animation endpoints unchanged during its probes.
- For packet-queue, initializing-list or update-sequence changes, run world_update,
  tick and player_walk tests, the movement_session example tests, and fresh
  `cargo xtask world-update-oracle --check` with Python/JDK 11. Preserve both
  shared packet buffers, nested processing, global NoProcess, accumulated error
  logs, live initialization queues, duplicate/early-removal behavior, captured
  list aliases, Kotlin iterator checks and partial failures. Keep node leases
  through deferred removal; stale wire indices must not rebind old input. The
  probe's empty lobby/NPC/chunk/ground-item services and partial Player methods
  do not establish full repository, Player, content, disconnect or world parity.
  Repository NPC lists use CopyOnWriteArrayList iterator snapshots; lobby and
  initializing player lists use live ArrayList iterators. Ordinary list add/remove
  applies immediately; only initializing player membership uses deferred entries.
- For NPC tick/spawn/render/queue changes, run npc_tick, npc_spawns, npc_render,
  npc_walk, dumb_path, navigation and player_walk tests, the movement_session
  example tests, and fresh npc-tick-oracle, npc-spawns-oracle, npc-render-oracle,
  dumb-path-oracle and navigation-oracle checks with Python/JDK11. Also check
  tools/port_dumb.py --check. Preserve original RNG draws, strict tick boundaries,
  initialization/failure order, DUMB singleton fields, sized traversal and
  clipping order, LinkedList membership and callback identity. Return attributes
  are internal owner-to-motion graph edges, not roots retaining detached cycles.
  Numeric NPC reader changes also require protocol npcs and client npc_updates
  tests plus python oracle/players/generate.py --check. The pinned server omits
  the NPC running selector expected by RT4; keep that discrepancy explicit.
  Joined native movement tests do not establish complete NPC definitions/config,
  subclasses, combat/content, repository/region startup or live NPC sessions.
- For NPC definition/configuration changes, run npc_definitions tests,
  npc-definition-oracle --check with Python/JDK11 and tools/port_npc_schema.py
  --check. With pinned assets, run release npc-definition-audit. Preserve lazy
  lookup/publication and retained aliases, partially populated arrays and reader
  positions, signed bytes/shorts, opcode126 fall-through, unknown-opcode behavior,
  constructor defaults, child fallback and missing-varbit failures. Configuration
  uses original JSONObject iteration, typed enums/animation constructors, strict
  comma entries, Boolean parsing, captured handlers and partial failed loads.
  Shared Property changes also require existing item_config tests and original
  item-config-oracle, item-config-audit and item-audit gates. The NPC metadata
  audit excludes opaque dropTables and does not establish complete NPC init,
  drop/combat/content/behavior, general plugin invocation or live world startup.
  The definition's `drop_tables` field is authored by port_npc_schema.py with
  `#[serde(skip)]`: keep it out of the tool's stored-field list, the
  observations and the audit. Changing that tool changes its hash in
  fixtures/npc-definitions/provenance.json; update only that line and prove it
  with npc-definition-oracle --check, whose 1,749 cases must stay byte-identical.
- For NPC drop tables, the shared rare/Chaos Elemental/herb/gem/seed tables or
  the drop random sources, run `cargo test -p rs09-server --test drop_tables`
  and `cargo xtask drop-tables-oracle --check` with Python/JDK 11. Preserve the
  three separate generators (RandomFunction.RANDOM, the one private to
  Collections.shuffle, the thread's ThreadLocalRandom) and their draw order:
  insertion swaps while loading, all selection before any amount, one double
  per non-empty table per time, pick before shuffle in the weighted chance
  tables, double before shuffle in the rare table. Keep the half-open amount
  range, `<= 0` selection, totals summed in insertion order, herb and
  uncommon-seed slots repeating by amount, the herb coin before the receiver
  check, nothing entries counted in the rare total but passed over in its walk,
  in-place shuffles that persist, `rand <= weight`, rows refused only for a
  minimum above the maximum, and the pinned JDK's uncorrected
  nextDouble(origin, bound). Looter, clue registry, definition lookup,
  inventory and logging are explicit inputs; do not answer them from native
  state before those systems are bound. The 444 pinned rows/2,152 NPC ids and
  seven shared files are in the portable corpus. Behaviour content and loader
  start-up order are not ported, and a rolled list is not a drop: what turns
  it into one is the next bullet. See docs/drop-tables.md.
- For NPCDropTables.drop/createDrop/getLooter/sendDropMessage/handleBoneCrusher,
  announceIfRare, DeathTask, kill credit or NPC finalizeDeath/handleDrops
  (world/drop_tables/drop.rs, definitions.rs, world/npc_death.rs), run
  `cargo test -p rs09-server --test npc_death --test drop_tables` and
  `cargo xtask npc-death-oracle --check` with Python/JDK 11, plus the suites
  of what they join: ground_manager/chunk_sync, pulses, npc_tick,
  npc_definitions, item_definitions/item_config and inventory. Preserve all
  474 cases and the 510 drop-table cases. Keep every service call in the
  original order with no default answers: the roll before the null-looter
  failure and the behaviour callback; the drop location read per item; bone
  crusher before plugins; three plugin lookups; unstackable stacks split for
  the killer without announcement, message or loot share; the looter chain's
  short circuits, its `random(10)` only for a member above the highest points
  so far, and the price read after the `true`/`false` message. DeathTask.start
  runs twice per death, a held strong pulse leaves the entity dead, and a
  failure while the death finishes makes Pulse.update finish it again (kill
  and roll counted twice): keep all three. Kill credit walks both damage logs
  in HashMap order; non-player sources need the binding's identity hash, and
  equal hashes are refused, not guessed. Host::roll must call the native table
  roll with the server's generators. The adapters of this oracle must not
  expose a public field named like a Kotlin getter property (`username` did
  and bypassed getUsername). A player's own death and DeathTask.getContainers
  are the next bullet's; the rest of handleImpact, NPC.clear, real Properties/attributes/zones/
  behaviours/plugins/clans/prices and any session or alpha binding are not
  ported; the en-US locale, entity hashes and the tick driver are oracle
  inputs. See docs/npc-death.md.
- For a player's death, kept items or graves (world/player_death:
  getContainers, Player.commenceDeath/finalizeDeath, Grave, GraveController,
  GraveType, BarrowsEquipment's grave rules, DeathInterface, the active-graves
  server store), run `cargo test -p rs09-server --test player_death` and
  `cargo xtask player-death-oracle --check` with Python/JDK 11, plus the
  npc_death, drop_tables, ground_manager, chunk_sync, pulses, inventory,
  container_ownership, container_listeners, item_definitions, item_config,
  ironman, player_save and player_state suites. Preserve all 938 cases. Keep
  getContainers' repeated pass per kept slot, `<=` (a later equal value wins),
  the push-out back into the worn container and the null-mode-is-not-ultimate
  rule; finalizeDeath's order, the permadeath return that skips every reset,
  bones for `k`, coins only for a player killer, all non-broken Barrows
  dropping broken, the second plugin lookup in checkTransform, grave items
  private to the owner's uid for good, Kotlin's direct `location` field
  access, the controller's 150/50 versus addTime's 90/30 thresholds, the -1
  grave that ends the whole tick, `String.valueOf` text for missing store
  fields and map orders. Every outside call is a required service; the
  driver order of the controller, ground pulse and grave NPC ticks is a case
  input. Permadeath.kt, GravePurchaseInterface and the forcegravedeath
  command are not ported; nothing installs the library. See
  docs/player-death.md.
- For NodeList, Repository, DisconnectionQueue or Player.clear/finishClear/
  allowRemoval (world/repository, world/disconnection), run
  `cargo test -p rs09-server --test repository` and `cargo xtask
  repository-oracle --check` with Python/JDK 11, plus the tick, world_update,
  world_session, pulses, owned_player and player_walk suites. Preserve all 625
  cases. Keep the uid scan over slots 1..size-1 only, remove(null) lowering the
  size, iterator copies, the last put winning in both maps, the Kotlin
  null-name failure after the uid map is written, HashMap iteration order
  (tree bins included) for the queue and names, wait counters kept after a
  finished entry and after clear(), the forced clear's finally order, and
  Throwable versus Exception catches. Every outside call is a required
  service; never answer one from native state. A library: nothing installs
  it, and the example's index search is not this port. See docs/repository.md.
- For friends/ignores, private messages, clans, ManagementEvents, GlobalChat,
  public ChatMessage state or the social packets (world/social), run
  `cargo test -p rs09-server --test social` and `cargo xtask social-oracle
  --check` with Python/JDK 11, plus the player, player_save, repository,
  settings and update_masks suites. Preserve all 166 cases. Packets are
  queued when sent and built at the flush (late contact/clan/pm_index reads);
  the management queue drains only when the host pumps it, in queue order;
  Kotlin assignments of clan join/kick requirements run the cleaning/updating
  Java setters; private messages ignore privacy and ignores; the pinned world
  has no management server (chat settings always send the "unavailable"
  message and ignore the private setting). Contacts are a JavaHashMap: keep
  put/remove/clear order. Clock reads and unseeded Random seeds are recorded
  host inputs; every other system is a required service with no default.
  A library: nothing installs it; MessageLog, quick chat and inter-world
  networking are not ported. See docs/social.md.
- For the world-trace oracle (oracle/world-trace, fixtures/world-trace), run
  `cargo test -p rs09-server --test world_trace` and `cargo xtask
  world-trace-oracle --check` with Python/JDK 11 and the pinned cache. It
  compiles all 3,487 original server sources unchanged, boots them headless
  through the original ServerConfigParser.parse and GameWorld.prompt(false)
  and drives ticks by hand: four scenarios (start-up, a new character, two
  characters, a returning character), 111 records. No native world replays
  it. The fixture describes the original only and clears no acceptance gate.
  Keep every host input explicit: the clock and unseeded Random constructions
  are redirected call sites (census in provenance), the three shared
  generators and the world thread's ThreadLocalRandom are replaced or seeded,
  and Dispatchers.Default and the management-event queue run on the world
  thread when the harness pumps. The generator must find two fresh JVM passes
  identical before it writes or accepts a fixture; never mask a differing
  field to get there. The original's world depends on identity hash codes
  (measured: one extra hash handed out before start-up changed NPC state from
  tick 3), so the JVM runs with -XX:hashCode=2 and identity-keyed collections
  iterate by insertion history; keep that flag, and do not present the trace
  as the only valid execution. Class-scan order and directory order are
  observed to repeat on this JDK build and operating system, not pinned.
  Not covered: the socket, the 600 ms worker loop with its timeout and restart
  checks, Server.main's networking, shutdown, bots (enable_bots=false is the
  one configuration change), any client input after login, a clean logout,
  reconnect, the lobby and the 50-tick autosave. Bump the schema number when
  records change. The run directory is guarded: a write outside it, a socket
  or a process fails the run. It must never open a database other than the
  SQLite files the original creates inside that directory. See
  docs/world-trace.md.
- For the native start-up (crates/server/src/startup/: loaders, generators,
  regions, sequence), run `cargo test -p rs09-server --test startup` and
  `cargo xtask startup-loaders-oracle --check` with Python/JDK 11 and the
  pinned cache; also the suites of the loaders the sequence joins (item/npc/
  scenery definitions and configs, interfaces, drop_tables, doors, variables).
  The oracle calls GameWorld.prompt's statements one at a time on the
  world-trace build and is accepted only if its boot record is byte-identical
  to the world-trace fixture's; never edit the world-trace harness for it.
  Preserve the 52 phases (per-phase draws and log lines), 6 tables and 47
  authored cases. Keep each loader's evaluation order, exception points,
  Kotlin split/trim/toBoolean rules, the ordinal rotation of overrides, the
  signed-byte DataMap strings and HashMap song order. Generators are one
  WorldGenerators per world with explicit Seeds, no defaults. A refused step
  is logged and makes the start-up incomplete, never skipped or reported as
  done; the whole-start-up test asserts its mismatches equal KNOWN_GAPS. The
  World it builds is not playable and nothing installs it. See docs/startup.md.
  The class scan is a native manifest (startup/scan_order.txt, checked against
  the boot record): register a class only with a native port, in the original
  order, and name every other one as content not ported; never draw to fake a
  generator position. For NPCSpawner, GroundSpawnLoader, the store, the scan
  steps or the timers there, also run chunk_sync, ground_spawn_loader,
  npc_spawns, npc_init, repository and timer_registry, and reproduce
  `cargo xtask startup-draws-oracle --check`: it attributes every start-up
  draw to its class and is accepted only if its phase lines equal the
  startup-loaders fixture's. Start-up writes go through Inputs::writes.
- For login into the world (world/login: Login.proceedWith and the rest of
  Login.kt after decoding, LoginParser, PlayerParser.parse, LoginWriteEvent,
  Player.init/Entity.init, LoginConfiguration), run `cargo test -p rs09-server
  --test world_login` and `cargo xtask world-login-oracle --check` with
  Python/JDK 11 and the pinned cache, plus the repository, social,
  interfaces, canonical_player and world_trace suites. The oracle reruns the
  three world-trace login scenarios on the world-trace build with
  PacketWriteQueue's lists replaced by lists that record who queued each
  packet; it is accepted only if every record is byte-identical to the
  world-trace fixture's and two passes agree; never edit the world-trace
  harness for it. Keep the original order and catch categories: the uid
  check before anything else, the parse failure that still submits the pulse
  with an unset parser, the pulse's Throwable recovery, the reply's rights
  and index read through session.getPlayer(). Login listeners and plugins
  are an input list by class name; every call without a native library is a
  required Host service with no default. The test's replay stand-ins (the
  original's packets for a not-ported call, found by its stack, with the
  counter and varps they carry) are inputs, not ports: it pins 282 natively
  written entries per new character's login (258 returning; the player tab
  calls are world/player_tabs) and every replayed segment, and a closed gap or
  a new difference must fail it. Nothing installs the library; the tick
  driver is world/driver (next bullet). See docs/world-login.md.
- For the tick driver (world/driver: handleTickActions' phases with the
  verified libraries installed, GameReadEvent.read, Decoders530's table,
  PacketProcessor's routes, PacketWriteQueue, GameWorld.pulse, Player.update
  and Player.reset, the logout path), run `cargo test -p rs09-server --test
  world_login --test tick_driver` plus the tick, world_update, repository,
  pulses, social, world_trace and canonical_player suites, and reproduce
  world-login-oracle. The world-trace comparison is world_login's
  tick_driver module on that binding: every entry of the three scenarios
  (bytes and dropped flag), the characters, lobby and counter after every
  step, the events and the saves. Keep the phase order and tick::handle's
  catch/finally, the read event's size -1 path and its byte[255] limit that
  wraps and returns, refusals that are logged and recorded (never a
  success), the five immediate write classes, the flush's one writer, the
  wrapping counter and the 50-tick hand-off. Every replayed segment, every
  differing entry (eleven player updates) and every stand-in kind is pinned;
  a closed gap or a new difference must fail it. The test clock is per
  thread. `rs09-server --world` installs the driver (next bullet); the
  scenarios reach no client input, NPCs or lobby. See docs/tick-driver.md.
- For the live world (world/live: `rs09-server --world`, its boot, the
  production login/driver/repository/social hosts, walking, the player
  update with prepared masks, the player tabs and their buttons, NPCs,
  entity regions, zone monitors and music zones, region activation, the map
  chunk renderer, ground items and pickup, the item/NPC/player/examine
  packets with DropListener and EquipHandler, EquipmentContainer and the
  appearance body, the region zones of ZoneBuilder.startup with SkullManager,
  the worker loop and the saver), run
  `cargo test -p rs09-server --test live_world` (needs assets/cache and
  runs/startup-loaders-oracle/template; it prints SKIPPED without them, and a
  skip establishes nothing) plus world_login, tick_driver, tick, world_update,
  repository, pulses, social, world_trace, canonical_player, startup,
  world_session, player_tabs, npc_walk, npc_behaviour, npc_render, zones,
  entity_regions, chunk_sync, map_chunk_render, player_walk, action_locks,
  scene, packet_handlers, ground_manager, ground_spawn, ground_spawn_loader,
  ground_options, ground_target, ground_runtime, pickup, wilderness,
  player_managers, equipment, inventory and interaction_listeners. Keep the
  libraries unchanged: a host answers through them.
  `zones::Host` is `Live` itself (a monitor per entity); NPCs keep the
  start-up's slots, store and generators; regions are never unloaded and
  NPC update masks are not written (both named).
  Every call into a system without a port is a named answer in
  world/live/not_ported.rs (skipped, or the original's empty answer when that
  is what it does with no content) or a refusal; list each in
  docs/live-world.md and never answer one with a fabricated success. Keep the
  login-tick comparison with the world-trace new-character tick (removed
  segments pinned by marker and count, entries before reinitVarps in order,
  counter-carrying packets by opcode and length, reinitVarps as a subset); a
  segment that becomes native must be taken off the list. Saves are built on
  the worker thread and written by the saver thread; nothing blocks the tick
  on file I/O. Ground items join the start-up's manager, spawns and option
  table; DropListener's soft script runs at the player's next tick; the
  appearance body is the canonical prepareBodyData copied into the prepared
  data. Content, content timers (SkillRestore and the rest), combat,
  use-with/component/slot-switch/scenery/continue/input handlers and region
  unloading remain open. See docs/live-world.md.
- For NPC factory/init/configuration application, run npc_init and player_walk
  tests, both movement_session/npc_session example tests, and fresh
  npc-init-oracle --check with Python/JDK11. Preserve prototype/null fallback,
  captured property/skill handles, configuration and partial mutation order,
  virtual callback reentrancy, child-array aliases and late parent index/size.
  Child arrays are traced internal graph edges; exposed arrays retain their
  elements without making detached parent/child cycles permanent roots.
  The npc_session endpoint is geographically bounded and uses empty subclass,
  combat, content, timer and partial skill services plus preactivated regions.
  Live packet/visual evidence does not complete NPC/Entity or world startup.
- For entity-region membership, move/local queries or tolerance changes, run
  entity_regions/chunk/region_load/scene/player_walk/world_update tests, both
  live example tests and fresh entity-regions-oracle/chunk-oracle/region-load-
  oracle/scene-oracle checks with Python/JDK11. Preserve chunk identity versus
  coordinates, duplicate/null list entries, x/y and player-before-NPC order,
  repeated location reads, captured lists/maps, partial callback failures and
  both clock draws for isTolerated. Chunk lists and query results retain canonical
  node leases; numeric IDs from another world are different identities. Native
  query work is bounded to65,536 visited chunks. Region activity/zone/cache/
  dynamic routing remain explicit services until independently implemented.
- For zones (world/zones.rs, zone_impls.rs) or region activity
  (world/region_activity.rs), run `cargo test -p rs09-server --test zones` and
  `cargo xtask zones-oracle --check` with Python/JDK 11, plus the
  entity_regions, region_load, region_queries, chunk, chunk_sync, scene,
  map_chunk_render, player_walk, walking and pulses suites and both live
  example targets. Preserve all 1,019 cases. Keep ZoneMonitor's live
  ArrayList walks (leave in list order, enter in registration order,
  locationUpdate after each, completed prefixes on refusal or failure),
  getUid from the name hash versus hashCode from the stored uid, the
  exception snapshot, the tele-block before zones, the duplicate pulse
  submission while still queued, unload's refusals that flag the region
  active again, and Region.add skipping only null players. Music, messages,
  combat state, NPCs, repository and every other outside system are required
  services with no defaults; a DynamicRegion fails explicitly. getRandomLoc's
  unseeded draws are compared only by draw-independent facts. cleanItems is
  not ported (WildernessZone, DarkZone and ZoneBuilder.startup: see the
  wilderness entry below), and nothing
  binds these libraries to a session: the examples' check_inactive,
  zone_update and ZoneMove answers are still fabricated. See docs/zones.md.
- For TeleportManager, TeleportType, TeleportPulse or Entity.isTeleporting
  (world/teleport.rs), run `cargo test -p rs09-server --test teleport` and
  `cargo xtask teleport-oracle --check` with Python/JDK 11, plus the pulses,
  animator, prepared_masks, update_masks, action_locks, walking, zones,
  scripts and attributes suites. Preserve all 1,027 cases. Keep send's order
  (WILDY_TELEPORT/OBELISK check only the teleblock timer; otherwise
  ZoneMonitor.teleport(type, null) then a second isTeleBlocked, both skipped
  for -1), the null node that makes type 1 fail while tele-blocked, delay and
  dialogue only for a non-forced player, tablet-spell after them, getPulse
  after the walking reset, lock 12 and disabled ticks 12 or 5 before the
  submission. Each type keeps its step ticks, sounds, priorities and stop
  override; FAIRY_RING keeps `++delay`, MID animate and the uncopied
  destination; every other type copies it at its step. A second send leaves
  the first pulse running, PulseManager.clear never reaches a teleport and
  send never calls start(). Pulses are native pulses::Pulse handles the world
  dispatches by identity; animations go through the native Animator, the
  final move through walking::Queue, the checks through zones::Monitor and
  zones::is_tele_blocked. Every other call is a required service with no
  default; the per-tick driver is an oracle input. No content reaches send
  and nothing installs the library. See docs/teleport.md.
- For WildernessZone, DarkZone, LightSource or ZoneBuilder.startup
  (world/wilderness_zone.rs, world/dark_zone.rs), run `cargo test -p
  rs09-server --test wilderness` and `cargo xtask wilderness-oracle --check`
  with Python/JDK 11, plus the zones, teleport, combat_pulse, combat_swing
  and npc_behaviour suites and zones-oracle. Preserve all 696 cases. Keep
  the level formula (truncating division, the 6400 split), isInZone's unused
  first location read over the INSTANCE borders the world keeps, show's
  order (overlay, level, line read back, attack option only with PvP, flag),
  leave closing a fresh component 381 (the manager's overlay field stays),
  the artificial player's flag-then-level, the lower skull level read twice,
  wrapping range tests, the revenant name test only for NPC targets, the
  teleport item name read twice for slaying, the loot's id/name re-reads,
  draw order (glove roll, byte-cast random(1, 14), seventeen gear rolls,
  random(15, 50)), the deep halving, the glove name lower-cased but a null
  gear name printed, and death returning false. DarkZone: the light source
  read before the perk, no hook for a perk player, the running-pulse early
  return before the component opens, the pulse stopped only after a close,
  updateOverlay's two overlay reads and SOFT glow script, the first DarkZone
  only in checkDarkArea. SkullManager, the InterfaceManager overlay, familiars,
  aggression, names, ground items, news, scripts, tasks, hooks and the impact
  handler are required services with no defaults; the insect pulse is a
  native pulse dispatched by identity. Nothing installs the libraries. See
  docs/wilderness.md.
- For SkullManager, HintIconManager (HintIcon packet), WarningMessages,
  AssistSession or AchievementDiaryManager/AchievementDiary/DiaryType
  (world/player_managers), run `cargo test -p rs09-server --test
  player_managers` and `cargo xtask player-managers-oracle --check` with
  Python/JDK 11, plus the wilderness, zones, trade, teleport, player_tabs,
  canonical_player and player_save suites; after a DiaryType change run
  `oracle/player-managers/port_diary_data.py --check`. Preserve all 865
  cases and their frame bytes. Keep setLevel's 49/48 deep-wilderness
  boundaries and the lock above 20, the risk skull only with the enhanced
  setting (protect item subtracted, the alchemy value read twice above 1,
  wrapping longs), the listener installed once and the cast failure on a
  foreign attribute, checkSkull's cause added before the Skulled timer is
  restarted (its removal resets and clears it), HintIcon frames sent before
  the slot store, the null target's 0, removal's shared context with type 0,
  AssistSession's zero window written only to GlobalData, the two clock reads
  of the pulse, the saved experience array shared with GlobalData, the close
  event's parameter player against the session's, the cap's float order, and
  the diaries' 25-slot completedLevels rule, Lumbridge's beginner level and
  open's child arithmetic. Diary state is persisted::diaries (joined, not
  copied); completedLevels is a world value. The Skulled timer is a required
  service whose bodies call back skull::set_skull_icon, answers::set_skulled
  and skull::reset; skull::answers answers the wilderness, zones, combat,
  NPC and death skull services. Every other call is a required service with
  no default; nothing installs the libraries. See docs/player-managers.md.
- For aggression, NPCBehavior hooks or the NPC/Entity/Player combat members
  (world/npc_behaviour: AggressiveHandler/AggressiveBehavior, setDefaultBehavior,
  configureBossData, isInvisible/isHidden, isAttackable, continueAttack,
  checkImpact, onImpact retaliation, sendImpact, inCombat, getSwingHandler,
  onRegionInactivity, fullRestore, the tick join), run `cargo test -p
  rs09-server --test npc_behaviour` and `cargo xtask npc-behaviour-oracle
  --check` with Python/JDK 11, plus the npc_tick, npc_init, npc_walk,
  combat_pulse, combat_swing, npc_death, zones, pulses and entity_regions
  suites. Preserve all 1,243 cases. Keep selectTarget's order (pause, lock,
  switching/attacking, death, one RANDOM nextInt(10) against the ratio, local
  players, canSelectTarget, priority flag plus one Math.random each with
  `<=`), the can_tolerate read before the allowTolerance field, tolerance
  strictly after ten minutes through the native entity_regions::is_tolerated,
  the wrapping level shift, the safespot 10/40 tick rule, the player-only
  path/moving-pulse retaliation condition, Kotlin's null refusal before a
  hook body, and attribute writes before their record. The tick kernel's
  four return/talk attributes stay world::npc fields. Every outside call
  (the combat pulse included) is a required service with no default. No
  corpus runs a whole fight through the real CombatPulse/ImpactHandler/
  DeathTask with these members; AbstractNPC and IdleAbstractNPC are not
  ported; nothing installs the library. See docs/npc-behaviour.md.
- For requests or trading (world/trade: RequestManager, RequestType,
  TradeModule, TradeContainer, TradeCloseEvent, TradeInterfacePlugin), run
  `cargo test -p rs09-server --test trade` and `cargo xtask trade-oracle
  --check` with Python/JDK 11, plus the inventory, container_ownership,
  container_listeners, interfaces, input_prompt, ironman, owned_player and
  bank_interface suites. Preserve all 373 cases and their frame bytes. The
  trade runs on native containers, the native InterfaceManager with the typed
  TradeClose event (its reentrant partner close included), native Ironman and
  the native input prompt with AmountScript; keep those joins, and keep every
  other call a required Host service in the original order with no default.
  Keep the quirks: no request timeout, the stale lastRequest accepted, the
  requester's own interface closed, Int-wrapping distance, the placeholder's
  stage 0, update's re-read stage, the in-place clamp at the exchange with
  amount-0 ground items, stack overflow lost, the unconditional bot lookups,
  Offer-X without m. The alert pulse body is ported; its scheduling, the
  real InventoryListener and every content service are not. Nothing installs
  the library in a session. See docs/trade.md.
- For shops (world/shops: Shop, ShopItem, ShopListener, Shops' start-up,
  tick, NPC, destination and 620/621 interface listeners), run
  `cargo test -p rs09-server --test shops` and `cargo xtask shops-oracle
  --check` with Python/JDK 11, plus the inventory, container_ownership,
  container_listeners, interfaces, input_prompt, ironman, owned_player,
  bank_interface, item_definitions, item_config, startup and trade suites.
  Preserve all 634 cases and their frame bytes; the 245 pinned rows are
  compared in 12 of them. Keep the joins (native containers and listener
  objects, InterfaceManager, typed callbacks in the InterfaceListeners
  registry, input prompt with AmountScript, Ironman, definitions) and every
  other call a required Host service in the original order with no default.
  Keep the quirks: the per-use personalized read, needsUpdate never cleared,
  duplicate listeners from repeated tabs, the buy price computed on every 620
  button, last-match getStockSlot and the note slot beyond the stock array,
  4067 sellable but refused by the value message, the shared general stock's
  "too full" check for every shop, wrapping and clamping amounts, tick 0
  clearing the player stock and an interval of 0 failing every tick. Item
  definitions in the corpus are authored inputs. Nothing installs the
  library; ShoppingPlugin.java is commented out in the pinned source. See
  docs/shops.md.
- For interface metadata, component open/close/plugin lookup, gameframe/tab/chat
  orchestration or window-mode changes, run interfaces and game_packets tests,
  both movement_session/npc_session examples and fresh interfaces-oracle and
  game-packets-oracle checks with Python/JDK11. Preserve global definition versus
  retained component aliases, the parser's double parse/replacement, callback
  order and reentrancy, partial failures, odd typed pane behavior, signed wrapping
  packet counters and original fixed/resizable child IDs. Keep tutorial, weapon,
  friends, plugins and full login configuration explicit until implemented.
  --gameframe uses real pinned metadata, an empty native inventory and explicit
  post-tutorial input; live UI evidence does not establish full Player or content.
- For player Settings changes, run settings and skills tests and reproduce
  settings-oracle with Python/JDK11. Preserve the ignored private-chat and
  special-toggle arguments, wrapping integer fields, IEEE NaN/signed-zero energy,
  ordered partial parse mutations, captured service receivers and callback/error
  order. Inventory weight ignores nonpositive entries; equipment includes signed
  weights and scans only11 slots, with Java compound-assignment LHS capture.
  Fields/services are shared owning-player state; do not hold locks across calls.
  Packet/container/item/property adapters do not establish full Player, profile,
  content integration or original client volume-preference interpretation.
- For scenery click dispatch, run scenery_action and xtask game_packets tests and
  also scenery_options and both live examples when editing the new owning-world
  bindings. Reproduce scenery-options-oracle, ground-options-oracle and affected
  interaction/door/scene/ground/ownership corpora. Keep cached definition actions
  separate from current-ID handler lookup, actual typed programs, foreign world
  rejection, old array identity and failed initialization prefixes. Stored table
  targets are internal edges; exposed node/scenery/table views retain the exact
  morph family. Raw arrays retain handlers without rooting dead families. Never
  hold guards across callbacks. See docs/live-doors.md for the live probe scope.
  An accepted ordinary door does not complete action locks, morph player values,
  autowalk/diaries/trapdoor Properties or full content/world startup. Continue to
  reproduce scenery-action-oracle/game-packets-oracle with Python/JDK11. Preserve
  lookup before lever construction, the second player-plane read, morph versus
  original object identity, captured wrapper status/options/interaction receivers,
  debug and varbit fallback reads, scripts/listener/plugin order and failure
  prefixes. The selected Kotlin method is unchanged in the oracle; option
  initialization, invalid/legacy approach, content registries, door rules and
  live scene delivery remain explicit services until independently bound.
  The same corpus compiles the unchanged DoorManagingPlugin. Changes to its
  native door_option port also require doors/scenery/scenery_node tests and fresh
  doors-oracle reproduction. Preserve case-sensitive cupboard distance guards,
  default-locale lowering (an explicit world service), switch fall-through,
  wrapping transform IDs, animation/audio order, captured teleport properties
  and registration order. Its transform/replace/player/door dependencies remain
  recording oracle adapters; neither these nor a decoded Open prove live doors.
- Scenery approach ownership also requires player_walk's retained-capture,
  duplicate-coordinate, repeated-replacement and foreign-world regressions.
  Native targets retain exact Scenery references; immutable arrival captures
  survive mutation of MovementPulse's live mover/destination/checks. Option
  handlers used for routing are captured at construction, while arrival reads
  the shared selected option. Handler registry IDs must remain bound for every
  movement/task that references them. Do not substitute a current option or tile
  lookup for a captured reference. The joined player-walk corpus records approach
  arrivals and normalizes the harness's class identity for telemetry; the full
  original InteractPlugin body and live scene delivery remain separate work.
- For legacy interaction orchestration/arrival changes, run legacy_interaction,
  player_walk and scenery_action tests and reproduce legacy-interaction-oracle
  and scenery-action-oracle with Python/JDK11. Preserve cached hasHandler/walk
  decisions versus repeated handler reads, manager receiver capture before
  construction, immutable pulse captures, face-before-lock ordering, short
  circuits and dispatch after helper returns. Walk/invalid arrivals catch and
  log; delayed default arrivals propagate. Print precedes virtual error-message
  lookup and logging. Default locale and concrete profile/content/mask bindings
  remain explicit. Face geometry must pack the intermediate top-right location.
  Logical-state oracle transport uses UTF-8 and must verify scenario round trips;
  test-console encoding is distinct from original gameplay string wire formats.
- For canonical plain tasks or the legacy_runtime binding, run interaction_runtime,
  player_walk, legacy_interaction and pulses tests and reproduce interaction-runtime-
  oracle/player-walk-oracle/pulses-oracle. Keep original movement and plain tasks
  on the same runner with its fixed queue prefix; ordinary delayed/unhandled
  pulses do not replace Entity.currentMovement. Capture managers before
  construction, retain body nodes independently of mutable checks, and preserve
  exposed old checks without retaining detached task/manager cycles forever.
  Verify the prior 190 player-walk observations remain byte-for-byte unchanged
  when changing shared harness assembly. Profile, handler, zone, face-mask and
  live scene bindings are explicit; joined scheduling is not live door parity.
- For a shared entity/scenery runner, run world_pulses, interaction_runtime,
  player_walk, scene and both live-example regression targets; reproduce
  world-pulses-oracle/interaction-runtime-oracle/player-walk-oracle/scene-oracle
  plus pulses-oracle. Keep one global fixed-prefix update/catch/telemetry phase;
  route foreign checks in their owning domain and validate pulse identity before
  dispatch or stop. Scene.collect_pulses only reaps metadata, never advances the
  queue or clears viewer flags. Foreign captures of canonical actor IDs require
  retained node leases. Preserve the existing 190/323/37 observation records.
  Full original MapChunkRenderer viewport synchronization and flag fanout still
  require their own implementation; do not substitute a radius-only broadcast.
- For map viewport traversal, run map_chunk_render, player_render, rebuild, scene,
  entity_regions and world_update tests and reproduce map-chunk-render-oracle
  with Python/JDK11. Preserve retained outer/row arrays, exact chunk identity,
  original x/y traversal and insertion-ordered region sets, last-scene x/y with
  current player plane, callbacks before slot publication and partial failures.
  RegionChunk synchronization/flag serialization/reset and region activity are
  separate services until bound; a recording viewport oracle is not live fanout.
- For chunk synchronization/flag queues or ground-item visibility, run chunk_sync,
  map_chunk_render, scene, world_pulses, player_walk, entity_regions, chunk,
  region_load, world_update, inventory/attributes and both live example targets.
  Reproduce chunk-sync-oracle, map-chunk-render-oracle, scene-oracle, world-pulses-
  oracle and game-packets-oracle; with pinned assets also run isolated release
  scene-audit. Preserve per-chunk identity, flag-array snapshots, live object/item
  fields, owner UID rebinding, clock capture/short-circuit order, constructor
  partial states and failed writer prefixes. Reset only after all viewer flushes.
  Queued/exposed flags and ground droppers retain canonical node leases; foreign
  registry IDs require capture owners, not numeric-node guesses. Scene's weak
  chunk bindings must not retain replaced chunks after dirty reset. Ground item
  managers, player/activity services, all foreign flag payloads, shared client
  item reducers and live fanout remain separate until independently integrated.
- For shared client ground item packets/lists/selection, run protocol ground_items
  and ground unit tests, xtask game_packets, fresh ground-items-oracle and
  game-packets-oracle, plus native/WASM client checks. Preserve duplicate IDs,
  first-match masked count/remove, actual network owner slot, signed value
  wrapping and strict ties, head promotion, shared iterator cursor, retained
  selected values/list/node ownership, callback order and failed prefixes.
  Apply top-level envelopes once. Rebase logical lists separately from scene
  teardown/rebuild. Never guess missing item metadata or terrain. A recording
  reducer oracle does not establish live Take, model placement or server drops.
- The shared GPU-independent pack/item decoder is in crates/client-data; visual
  client world rebuild/ChangeLocRequest types and queue now also re-export its
  scenery module. Keep the actual shared implementation and public aliases;
  camera picking remains visual. The original location writer is shared from
  protocol/game/actions.rs and client interaction re-exports it. Changes require
  the48 original locaction vectors, original Scene/rebuild/interaction cases,
  client/headless/protocol tests, fresh full UI oracle and native/WASM checks.
  Headless Session is a reusable data-only transport; preserve pending login
  bytes, fragment handling, frame validation before writes and caller deadlines.
  Scenery observations are ordered wire deltas/private sound fields, not complete
  static maps, collision, asset readiness or local success acknowledgements.
  Run the headless Session/scenery tests, game-packets and affected login/network/
  world-session checks; with the owned probe exercise adaptive and CLI real TCP/
  WS actions, restoring the door before other visual probes. See
  docs/headless-scenery.md. The previous item-sharing rules still apply:
  client public paths re-export the same types. Changes require client/headless
  tests, fresh full UI oracle (JDK25), port_client_items.py --check and native/WASM
  checks. Headless --ground-pack preloads immutable metadata once per process.
  Preserve RT4 missing-definition defaults with explicit fallback diagnostics;
  this is distinct from strict server cache errors. Never report the headless
  rendering-height placeholder as measured terrain or infer live drops/Take
  behavior from an empty old-server smoke. Keep original self teleport queue
  behavior during rebuilds, including the non-discard near-distance branch.
- For GroundItemManager/chunk item mutation or tradeability, run ground_manager,
  chunk_sync, map_chunk_render, scene, world_pulses, player_walk, entity_regions,
  chunk, region_load, world_update, player_render/rebuild, inventory/attributes,
  item_definitions/item_config and both live example targets. Reproduce ground-
  manager/chunk-sync/map-chunk-render/scene/world-pulses/game-packets/inventory/
  item/item-config oracles; use an isolated --output for item-audit and scene-audit
  with pinned assets. Preserve first-reference removal, ArrayList modCount versus
  slot replacement and captured lists, fixed pulse snapshots, late dropper reads,
  Euclidean distance of 15 tiles, plugin receiver/argument order and failed prefixes.
  Private sends are fixed 26 plus a separate item frame in one byte stream; use
  direct_frames for native framing. Ground active/private subtype dispatch is
  shared by manager and viewers. Exposed Item views use ground_item::as_item to
  retain the owner without cycles. Preserve all prior 259 chunk-sync and 1,831 item
  records when extending oracle assembly. GroundSpawn scheduling, owning-world
  service bindings, Take/inventory and live fanout remain separate until bound.
- For GroundSpawn subtype/rates/respawn changes, run ground_spawn, ground_manager,
  chunk_sync and pulses tests; reproduce ground-spawn/ground-manager/chunk-sync/
  pulses oracles with Python/JDK11. Preserve Kotlin's left-associative infix
  `(min | max) << 16`, double population divisions and casts, runner receiver
  capture before duration reads, and one canonical fixed-prefix queue. Register
  the subtype before the base constructor exposes it: failed constructors retain
  virtual dispatch with default fields. Exposed ground/Item/task handles retain
  owners without cycles; weak metadata must release them. Keep all previous405
  manager and259 chunk-sync observations byte-identical. Full owning-world
  bindings and authoritative Take remain separate.
- For ground-spawn configuration changes, run ground_spawn_loader, ground_spawn,
  ground_manager, chunk_sync and pulses tests and reproduce all five JVM corpora.
  Preserve full pinned177rows/717spawns, per-row catch and completed prefixes,
  rate-before-Item-before-coordinates evaluation, no whitespace trimming, minus
  splitting, literal/null config prefixes, reader publication after open and
  reentrant reader aliasing. Preserve cached Node names in diagnostic strings.
  Log categories omit JVM stack frames; bounded UTF8 readers, alternate charset
  and large malformed-input read-ahead remain explicit limits, not full I/O parity.
- For owning-world ground assembly or ClearRegionChunk changes, also run the
  clear_chunk differential target and both movement_session/npc_session examples;
  reproduce clear-chunk-oracle with Python/JDK11. Keep Scene/actors on the same
  canonical Runner and reject foreign handles. Use actual account UID/name and
  retained player viewport/rebuild state. Clear flags only after all viewers and
  run the manager after reset. Test rebuild/plane clear-before-sync, UID rebinding
  and disconnected-owner release; metadata must not retain actor leases itself.
  The opt-in probe's missing Take/Drop/default interaction, item plugins, region
  inactivity and full profiles remain explicit. Live item visibility does not
  prove those handlers, complete startup, render fidelity or capacity.
- For pickup handler/plugin or cape/pouch lookup changes, run pickup, inventory,
  ground_manager, ground_spawn, ground_spawn_loader, chunk_sync, clear_chunk and
  pulses tests plus both live examples. Reproduce pickup/ground-manager/ground-
  spawn/ground-spawn-loader/chunk-sync/clear-chunk/pulses/inventory/attributes/item/
  item-config oracles. Preserve subtype versus virtual-auto distinctions,
  manager identity membership, cached UID rebinding, captured new Items versus
  late ground fields, each actual inventory/plugin receiver and failed prefixes.
  Destroy does not set removed itself. Pickup attribute cleanup is not finally.
  Native container regressions do not establish InventoryListener/profile, ground
  packet approach/arrival, content callbacks or live Take. Keep the pinned bundled
  ConstLib evidence and explicit missing PICK2_2582 dependency boundary visible.
- Canonical players now own a value-attribute store and a real Ironman manager.
  Capture the actual player for attribute views; raw key presence includes null
  and false. Typed integer reads preserve wrong-type failures and original null
  defaults. Generic object-valued attributes/profile parsing remain separate;
  do not introduce untraced object-to-player cycles through that store. Trace the
  Ironman manager's actual final player field once per object, including exposed
  manager roots after disconnect; it must release after its last external view.
  Run ironman/owned_player, all ownership/container/listener/world/ground suites
  and fresh ironman-oracle plus affected existing corpora. The81 complete original
  cases preserve nullable modes, ordinal thresholds, aliases and message callback
  reentry/failure. No manager/field guard spans message or other host callbacks.
- pickup_runtime joins the real ground-world context to captured canonical
  inventory receivers/listeners, attributes, Ironman, shared cape/pouch objects
  and actual Scene permission. Use the original virtual two-argument add path,
  never a replacement vector mutation or success stub. Required content/bank/
  dialogue/plugin/audit/animation/audio/event services stay explicit. Controlled
  endpoint Take composes actual66 packet/option/movement/arrival and two-viewer
  ground removal; test it with both example targets and finite real TCP/WS
  actor/spectator runs. Respect private inventory/audio delivery and original
  failure prefixes. Settings.updateWeight sends171 text at667:32, not a new159
  packet there; preserve the double separately from its integer label. See
  docs/live-take.md. Full content/startup/profile/Drop and missing original sound
  constant provenance remain open even after an accepted ordinary Take.
- For interaction option-table or ground-click dispatch changes, run
  interaction_options, ground_action, legacy_interaction, scenery_action,
  player_walk, pickup, ground_manager, ground_spawn, ground_spawn_loader,
  chunk_sync and clear_chunk tests and reproduce their JVM corpora plus
  game-packets-oracle. Preserve canonical static Option identity, old exposed
  arrays, live table loop bounds, retained input names, item location versus
  subtype selection and partial initialization. Definition/locale/handler/packet
  services remain explicit; never hold locks across them. Ground lookup precedes
  scripts; plugin precedes GROUNDITEM then ITEM listeners. Re-read the interaction
  receiver after callbacks while retaining the selected option. Runtime node and
  handler IDs must retain their owning-world identity while captured. Recording
  invalid/approach callbacks do not establish authoritative live Take or complete
  content/profile/default-registry bindings. Oracle Kotlin properties must use
  the original getters rather than public adapter fields; snapshots copy arrays.
- Ground packet Lookup now exposes only the actual player-location and native
  GroundItemManager.get operations it uses. Keep the original manager bound by
  owning-world adapters. ground_runtime joins this packet algorithm to captured
  ground tables and the canonical legacy scheduler. Invalid clicks preserve the
  world Option.NULL object without a second interaction read. Valid dispatch
  re-reads the table after plugins/listeners but retains the selected option.
  Reject foreign/nonplayer request handles and foreign target/table captures
  before entering legacy scheduling. Run ground_runtime and the existing ground-
  action/options/target, interaction/runtime/player-walk/pickup and ground suites,
  fresh affected original corpora and workspace checks. Its composition tests
  record typed handler arrival, not full Pickup/profile/container or live Take
  integration. See docs/ground-runtime.md.
- For canonical ground targets or weak exposed-view ownership, run ground_target,
  player_walk, interaction_runtime, world_pulses, ground_action, pickup, inventory,
  attributes, ground_manager, ground_spawn, ground_spawn_loader, chunk_sync,
  clear_chunk, pulses, scene, navigation/path_dispatch and both live examples;
  reproduce their JVM corpora. Typed targets retain the exact ordinary ground or
  GroundSpawn owner after list removal. Count internal dropper leases separately
  from external roots, then trace ground/dropper/current-motion edges together.
  Exposed ground/Item/subtype views and detached checks are roots; disconnected
  cycles without external roots must release. Weak upgrades retain the actual
  subtype owner, including after a particular anchor retires. Reject foreign
  subtype registries/droppers; never infer subtype from coordinates or auto-spawn.
  Preserve inherited node size, original removed-or-before-decay active behavior,
  unconditional spawn activity and GroundItem path-dispatch kind. Ground target
  walking uses the native clock and required content/arrival services; it does
  not establish default registries, packet-to-inventory binding or live Take.
- For typed native option registries or Pickup Program registration, run
  item_option_registry/shared_ownership, pickup, interaction_options, ground_action,
  ground_target and the shared ownership/ground suites above; reproduce
  item-option-registry-oracle and all affected corpora. Item-local option:name
  configuration precedes the exact case-sensitive global key. A null name forms
  option:null locally but remains a null global key. Wrong local types fail
  before global fallback. setOptionHandler reports whether the previous value
  was nonnull, not whether a key existed. Preserve real handler object identity,
  exposed map aliases and nonnull forId fallback. Only an explicit null lookup
  at22937 logs the original missing-definition error. Iterator/view semantics
  remain separate; authored sorted snapshots never establish HashMap iteration.
  Native program payloads bind through owning-world dispatch, never decoded
  numeric/JSON observation IDs. Keep callbacks outside handler/map locks.
  Shared-handle layout probes measure inline storage only; record exact source,
  binary and toolchain inputs, preserving weak owner/anchor retirement behavior.
- For owning-world ground option bindings, run ground_options, interaction_options,
  item_option_registry, ground_action/ground_target, shared ownership, player_walk,
  interaction_runtime/world_pulses, pickup and all ground/chunk/scene tests plus
  both live examples; reproduce ground-options-oracle and the affected existing
  corpora. Ground tables use cached Item definition arrays but current-ID handler
  lookup. Preserve location-based array selection, local type failures, partial
  initialization and old exposed arrays. Bind actual native handler objects;
  reject foreign actor domains even when they share a runner or numeric IDs.
  Exposed table views retain the exact subtype; old arrays retain native handlers
  without rooting dead ground/actor graphs. Arena lifetime is conservative, not
  proof of complete plugin GC. No guards span the default-locale operation.
  Constructor defaults are not packet/navigation/arrival or live Take proof.
- For RegionManager query or Scene predicate changes, run region_queries, clipping,
  mapscape, region_load, scene, navigation/path_dispatch, movement/player_walk,
  ground_target and both live examples, plus fresh region-queries/clipping/mapscape/
  region-load/scene/navigation/path-dispatch/movement/player-walk oracles and
  isolated release scene-audit. Preserve load-before-hasFlags, repeated chunk and
  landscape reads, retained arrays across callbacks, absolute flag keys independent
  of mutable ChunkFlags bases and differing walking/projectile defaults. Teleport
  permitted and clipped are not complements. Water captures its first flag before
  its independent landscape/clipping query. No guards span host callbacks. The
  Host now uses actual Scene permission for ground reach; this is not live Take,
  full RegionManager startup, water-NPC supplier or parallel simulation proof.
- For UpdateMasks lifecycle changes, run update_masks, player_render, appearance,
  world_update and both live example tests, and reproduce update-masks-oracle,
  player-render-oracle, appearance-oracle and world-update-oracle. Keep actual
  context/provider identities and separate current/synced arrays and presence
  fields. Reset must preserve synced slots and stamp; unregisterSynced must not
  clear current slots. Preserve signed extension-header checks, late slot reads,
  captured writeSynced flags before the appearance lookup, prepare's captured
  handler and two separate peek/poll calls, late updating=true and failed byte
  prefixes. Registry/log/clock/impact/encoder services are explicit and required;
  no guards span callbacks. Core lifecycle tests do not establish concrete flag
  encoders, full registry startup, actual Actor integration or live Take.
- For prepared-player mask/Actor binding changes, run prepared_masks/update_masks,
  player_render/rebuild/appearance, player_inventory/inventory_listener/settings/
  skills/interfaces, world_update/player_walk/tick, shared ownership/entity-region/
  NPC/ground suites, headless tests and all three live example targets. Reproduce
  prepared-masks/update-masks/player-render/appearance/player-inventory/inventory-
  listener/settings/skills/interfaces/world-update/player-walk/tick/entity-regions/
  map-chunk-render and affected ground/NPC oracles. Keep the actual shared body,
  owner and canonical face target; never resolve a retained context by wire index.
  Player client indices OR 0x8000. Preserve clear-during-updating behavior,
  prepare-before-render, reset after all viewers, and source-only synced facing
  for later viewers. Private host views must retire even with self-facing; no
  guards span clock/encoding callbacks. Controlled previews explicitly register
  appearance. Full profile/body preparation, combat/hit providers, other concrete
  flags, mutable registry startup and authoritative Take remain unbound.
- For server Animation/Graphics/Animator or animation-definition changes, run
  animator/prepared_masks/update_masks/player_render/appearance and all three
  live-example targets; reproduce animator/prepared-masks/update-masks/appearance/
  player-render oracles. With pinned assets run release animator-audit with an
  isolated --output directory. Preserve the server Java high-word precedence
  expressions; these differ from the RT4/client sequence decoder. Duration ticks
  truncate milliseconds/600, skip only durations above100 and ignore delay.
  Preserve mutable reset/context identity, stale cached definitions after ID
  changes, captured deadlines/ticks before callbacks, ignored mask return values,
  reset's clock clear after rejection, signed overflow and failed prefixes.
  Actual prepared owners retain their exact clocks/masks and one reset factory
  per world, never rebind by wire index. No locks span world/cache/mask callbacks.
  This does not complete full Entity/profile/combat/content integration, chunk
  Graphics.send delivery or authoritative Take. Do not use raw mask registration
  as a replacement for original Animator priority/timing in gameplay handlers.
- For music changes (rs09-audio midi/vorbis/sound_stream/music, client music.rs
  or the PCM output's music mix), run `cargo test -p rs09-audio` and
  `cargo xtask music-oracle --check` with JDK 25 and the pinned cache. The
  fixture holds hashes over real cache data, so the Rust test skips without
  assets/cache; a pass without the cache establishes nothing. Preserve integer
  wrapping, the 22050-byte loading budget per isSongReady call, exclusive-group
  slots that keep naming a note after it left the list (group 0 is never
  freed), the limiter reaching only voices a note still owns, and separate
  clipping of music and effects. The oracle opens no device: do not infer
  hardware output, latency or pacing from it. See
  docs/client-protocol-and-cache.md.
- For the visual client's JVM-checked parts, reproduce the matching corpus with
  JDK 25 (`cargo run -p xtask -- NAME --check`) and run the named rs09-client
  test; each compiles unchanged RT4 classes (or method text copied verbatim at
  generation time, recorded in the fixture's provenance file) with adapters
  under `oracle/<topic>/`:
  `input-oracle` (`tests/input.rs`: Keyboard/Mouse state after every call),
  `preferences-oracle` (lib tests in `preferences_file.rs` and `cheat.rs`:
  preference file read/decode/write, `toInt`, `::` command effects),
  `menus-oracle` (`tests/menus.rs`: hover-text glyph layout, CursorType),
  `minimap-oracle` (`tests/minimap_features.rs`: drift, floor jitter, player
  dots, label layout),
  `camera-oracle` (`tests/camera_reference.rs`: follow camera, viewport bounds,
  projectToScreen), `roof-oracle` (`tests/roofs.rs`: roofVisibility and tile
  arrays), `materials-oracle` (`tests/materials.rs`: noise volumes and the
  liquid table; `tests/underwater.rs`: water/underwater material GL traces),
  `picking-oracle` (`tests/picking.rs`: hover test, walk tile, draw order),
  `ui-fidelity-oracle` (`tests/ui_fidelity.rs`: hook walk, layout, drag start,
  kerned widths, tag numbers, item count text),
  `actors-oracle` (`tests/actors_oracle.rs`: BAS decode, acceleration, yaw and
  lean, crowd thinning, ProjAnim flights, rgbToHsl),
  `loc-oracle` (`tests/loc_placement.rs`: LocType placement fields, GL models
  by shape and rotation, hill skew, getMultiLoc),
  `lighting-oracle` (`tests/lighting_oracle.rs`: light selection, light meshes,
  floor shadows, isObjectInShadow),
  `display-oracle` (`tests/display.rs`: DisplayMode.setWindowMode calls, state
  and letterbox after every step),
  `title-oracle` (`tests/title_scene.rs`: title region set-up, NPC files,
  wandering and the title camera; `tests/title_cache.rs` needs the cache),
  `occlusion-oracle` (`tests/occlusion.rs`: occluder construction, active
  occluders, the visibility queries and renderTile with real occlusion).
  The terrain corpus is a client example, not an xtask command:
  `cargo run -p rs09-client --example terrain_oracle -- --check`
  (`tests/terrain.rs`: HD floor mesh, tile shade, merged normals;
  `tests/underwater.rs`: underwater maps, water data, floor and locations). It
  also needs `runs/player-session/xteas.json`.
  JVM-derived but without a regenerating command here: `tests/title.rs`
  (LoadingBar layout; javac line in `oracle/title/TitleOracle.java`),
  `tests/login_flow.rs` (login, create and world-list trace) and
  `tests/world_map.rs` (world map hashes); the last two harnesses are not in
  the repository.
  These establish the named algorithms only. Input event translation from the
  window system, GPU shaders, draw-call output and anything a report under
  docs/ lists as a transcription test are not JVM-verified; say so when
  describing them. The fixtures hold hashes of pinned sources and, for some,
  of real cache data: a pass without `assets/cache` establishes nothing there.
  See docs/client-protocol-and-cache.md for what each client area rests on,
  its known differences and the "Still missing" list.
  Shift-click (`Cheat.shiftClick`, a 2009scape addition) is ported but starts
  off; the pinned client starts with it on. Do not change the default without
  the owner's decision.
