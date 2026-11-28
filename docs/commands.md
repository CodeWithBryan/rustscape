# Commands, quick chat and abuse reports

Native ports of three `PacketProcessor` branches that the tick driver used to
refuse: `Command` (`CommandSystem.parse`), `QuickChat` (`QCRepository.sendQC`)
and `ReportAbuse` (`AbuseReport`). The driver now routes them to
`Handler::Command`, `Handler::QuickChat` and `Handler::ReportAbuse`
(world/driver/dispatch.rs); the libraries are `world::commands`,
`world::quick_chat` and `world::abuse_report`. `rs09-server --world` installs
the command system (world/live/commands.rs: the COMMAND packet, the registry
and `commands::Host`/`sets::Host` over the live world; what it answers and
refuses is in docs/live-world.md); quick chat and abuse reports are still not
installed there.

## What is ported

| Original | Native |
| --- | --- |
| `CommandSystem.parse`, `Command.attemptHandling`, `CommandMapping` (a `HashMap`, walked in its own order), `getPageIndices` | `commands::parse`, `attempt_handling`, `Registry` (over `world::repository::hash_map::JavaHashMap`) |
| legacy `core.game.system.command.CommandSet` enum: `validate`, `interpret`, linked `CommandPlugin`s | `commands::interpret`, `LegacySet`, `Registry::link` |
| `CommandPlugin.toInteger`, `getArgumentLine`; `sets.CommandSet.reject`/`notify`/`define` | `commands::to_integer`, `argument_line`, `reject`, `notify`, `registration` |
| the class scan's `defineCommands()`/`newInstance` calls | `registration::register_all` over `startup::scan`'s manifest |
| `ModerationCommandSet` (kick, ban, ipban, mute, jail, modcr, csvmodcr, its getattribute) | `commands::moderation`, `commands::deferred` (the pulses and the coroutine) |
| `SystemCommandSet` (update, cancelupdate, resetpassword, setpasswordother, giveitem, removeitem, removeitemall, potato, shutdown, charge) | `commands::system`, `commands::deferred` (task and input handlers) |
| `TeleportCommandSet` (to, tele, teleto, teletome, home) with `ServerConstants.TELEPORT_DESTINATIONS` | `commands::teleport` |
| `PlayerAttributeCommandSet` (getattribute, setattribute) | `commands::attributes` |
| the `Command` branch and the `COMMAND` decoder | `commands::process`, `commands::decode`, `commands::handle_packet` |
| `QCRepository` (getIndex, the phrase lookups, sendQC, its pulse), `QuickChatPacketHandler`, the `QUICKCHAT` decoder and branch | `world::quick_chat` |
| `AbuseReport`, `Rule.forId`, the `REPORT_ABUSE` decoder and branch | `world::abuse_report` |

The registry holds typed bodies. A `NativeBody` (the 24 commands of the four
ported sets that survive registration, and the 57 of MiscCommandSet,
SpawnCommandSet and StatsCommandSet, `NativeBody::Set`, run through
`Host::sets`: docs/command-sets.md) runs in `commands::run`; every other
command (121 of the 202 the boot registers) is a `Body::Content` naming its
class and handler lambda, run by the required service
`Host::content_command`. Dispatch goes through the registry as the original's
does; no name is matched outside it. `Command::origin` (the
`Class$defineCommands$N` that defined it) is metadata for records only.

Every outside call is a required `Host` method with no default. The account
and authenticator calls (checkUsernameTaken, getAccountInfo, update,
getUsernamesWithIP, checkPassword, updatePassword) are answered by
`commands::accounts::Accounts` over the native `crate::auth` storage and
authenticator, and the bodies write ban ends, mute ends and credits into the
native `UserAccountInfo` records (`Host::account`), so a ban or mute lands in
the record login reads. Quick chat builds the native
`world::social::ChatMessage` (with `isQuickChat`) and publishes the native
`world::social::Event::ClanMessage`; its chat icon, mask registration and
management queue are host services the world answers with
`world::social::chat::chat_icon`, the update masks and `Social::publish`.
Skill names and the experience curve are the native `player::skills::Skills`.

## Evidence

`cargo xtask commands-oracle --check` (Python, JDK 11) reproduces
fixtures/commands/:

- registry.json: the whole unchanged server booted as oracle/world-trace boots
  it (accepted only when the boot record is byte-identical to the world-trace
  fixture's and two fresh JVM runs agree), then CommandMapping in
  `getNames()` order with every command's name, privilege, usage, description
  and handler class, the legacy CommandSet plugin lists and
  `getPageIndices(0..3)`. 202 commands; ADMINISTRATOR holds MapDumpCommand,
  SimpleDumpingCommands and VisualCommand.
- oracle.jsonl: the command framework, the four sets, AbuseReport/Rule and
  QCRepository/QuickChatPacketHandler compiled unchanged (with the decoders,
  the three branches, the destination table, getRights, the Skills members and
  the Log enum inserted verbatim) over recording adapters, each case in a
  fresh class loader, wall-clock reads redirected to case inputs. Every
  recorded call of the marked operations is then made to fail once in turn.
  The counts are in fixtures/commands/provenance.json.

`cargo test -p rs09-server --test commands` replays every case natively and
compares every recorded call, result, error and the account state after each
operation; it checks that `registration::register_all` builds exactly the
booted registry (order, fields, defining class and lambda, legacy lists, page
indices) and that the registry's name set matches the world-trace boot
record's CommandMapping digest.

The comparison is not vacuous: 19 deliberate breaks of the port were tried
(the deprecated-command message, the privilege comparison, the duration
token, the castle-wars row, getIndex, the self-report return, the full
inventory's bank, the singleton-list message, replaceFirst, the boot's
active world events, the jail release comparison, a container read in
removeitemall, the argument split, a boolean attribute, isQuickChat, the
csv comment prefix, the mute field, the slayer amount check and the item
phrase's `<` check). The replay or the registration test failed for 14 of
the first 16; the other two (replaceFirst in direction phrases, the jail
release at exactly its time) were unobserved, so cases were added (clan
quick chat publishes the text, a public message's pulse registers it, a
release at the exact clock) and both breaks then failed, as did the last
three.

## Differences and limits

- The three decoders run when the packet is processed, not when it is read:
  dispatch.rs keeps COMMAND, QUICKCHAT and REPORT_ABUSE as
  `Packet::Undecoded` (the route edits are this slice's only change there).
  A payload the original drops at read time (`NoProcess`, `DecodingError`) is
  queued and dropped by the handler instead, the COMMAND decoder's "Something
  went wrong there! Please try again." is sent in the packet phase, and the
  quick chat decoder's log lines are written then. Within a tick the original
  reads before it processes, so only the order of those messages and log
  lines relative to other packets of the same tick differs.
- `handle_packet` of quick chat and abuse reports answers a decoding failure
  as the `DecodingError` text (without the stack trace the original appends).
- `kotlin::to_int_or_null`/`to_int` accept ASCII digits only. Kotlin and the
  JDK also accept other Unicode decimal digits, which the client cannot send
  (the decoder widens bytes to U+0001..U+007F and U+FF80..U+FFFF).
- `setattribute` parses floats as doubles and narrows them; `Float.parseFloat`
  rounds once. A decimal lying exactly between two floats after the double
  rounding can differ by one unit. `Float.toString` is JDK 11's
  `FloatingDecimal` (not always the shortest digits, e.g. "2.14748365E9"):
  `java_double::format_float` runs the shared `dtoa` with the float's
  significant-bit count.
- `TELEPORT_DESTINATIONS`' last row (castle wars) holds the location
  `CastleWars.lobbyBankArea.randomLoc` drew when `ServerConstants` was
  initialised: `Host::castle_wars_destination` is that start-up input.
- `ModerationCommandSet`'s `getattribute` is ported but unreachable, as in the
  original: `PlayerAttributeCommandSet` registers the same name later.
- `::shutdown` is `Host::exit_process(0)` and no case drives it (the oracle
  JVM would exit). `Rule.canRequest` is not called by any original code and is
  not ported.
- `EasterEvent` registers `eggspawntest` only when the boot finds it active;
  `register_all` takes that answer as an input (`pinned_boot_active` is the
  world-trace boot's: inactive).
- The remaining sets are registered with their exact names, privileges,
  usages and descriptions, but their bodies are content services (Spawn,
  Stats and Misc are native: docs/command-sets.md): Animation, Music
  (AudioCommandSet), Botting, Camera,
  CameraViewer, Config, Development, Fun, If, ModelViewer, Quest, Slayer,
  MimeTestCommands and the commands of ShootingStarPlugin, ImplingController,
  LunarListeners, BloatedToadListeners, ElementalWorkshopQuest,
  RevenantController, GrandExchange, GraveController, Shops, GlobalChat,
  ConfigParser, AntiMacro, HolidayRandoms, AprilFoolsEvent, Giftmas and
  EasterEvent; the legacy MapDumpCommand, SimpleDumpingCommands and
  VisualCommand `parse` bodies are `Host::legacy_parse`.
- Not bound: the live world (world/live/commands.rs) installs the command
  handler but not quick chat or abuse reports, and refuses by name what
  docs/live-world.md lists. There and in any other world, the content services, the zone `parseCommand` hooks, PlayerMonitor, the
  ServerStore archive, Discord, the system manager, containers and items,
  ChargedItem, the cache index 24 and data map readers and the slayer
  answers are services the world must still bind.
