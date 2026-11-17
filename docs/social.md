# Social systems: friends, ignores, private messages, clans, chat

`crates/server/src/world/social/` ports, as a library, the pinned social
code a login and a playable world need:

| Original | Native |
| --- | --- |
| `CommunicationInfo` statics: `add`, `remove`, `block`, `updateClanRank`, `hasContact`, both `showActive`, `sendMessage`, `toggleLootshare` with its pulse, `setClan` | `social/communication.rs`, `Social::set_clan`, `Social::run_pulse` |
| `ClanRepository`, `ClanEntry`, `ClanRank` (already native in `player::communication`) | `social/clan.rs` |
| `ManagementEvents` (queue, every handler, the clan-information round trip, `initializeClanFrom/With`, `handleLoggingFor`), `Login.sendMSEvents`, `Player.sendLogoutEvents` | `social/management.rs` |
| The social branches of `PacketProcessor.process`, `splitChatMessage`, the ten social decoders of `Decoders530` and its dispatch, `GlobalChat`, `ChatMessage` | `social/chat.rs` |
| `PacketRepository.send`, `PacketWriteQueue` push/flush, `ContactPackets`, `CommunicationMessage`, `UpdateClanChat`, `MSPacketRepository.sendContactUpdate/sendChatSetting` | `social/packets.rs` |
| `StringUtils.stringToLong/longToString/formatDisplayName/encryptPlayerChat/decryptPlayerChat`, `CP1252` | `social/names.rs` (tables transcribed from `StringUtils.java`) |

It is joined to what already exists: player state is the native
`player::details::PlayerDetails` (account, rights, `isMuted` with its own
clock) and `player::communication::CommunicationInfo`; contacts now keep the
JDK 11 `HashMap` table of `world::repository::JavaHashMap`, so put, remove,
clear and the walk (and therefore `getContactString`, the clan ranks taken
from contacts and the contact lists sent at login) follow the original;
`parse_contacts`/`parse_clan_requirements` are the player-metadata parsers;
`UserAccountInfo` (with its equality and `create_default`) is the auth
record; the private-chat setting and `updateChatSettings(pub, priv, trade)`
are `player::settings`. Nothing installs the library: no session, login or
tick driver calls it yet.

## Behaviour worth knowing (all verified)

- The pinned `Server.kt` never connects the management server, so
  `WorldCommunicator.isEnabled()` is false and `getSession()` is null. Every
  chat-settings packet therefore tells the player "Privacy settings
  unavailable at the moment..." before applying the public and trade
  settings; the private setting argument is ignored, so the private chat
  setting only changes through a loaded save.
- Friend names from packets are base-37 longs, so they arrive in lower case
  with underscores. `Repository.getPlayerByName` lower-cases and turns spaces
  into underscores; contact keys are kept as given, so "Bob" and "bob" are two
  contacts. Lists hold 200 entries; duplicates are refused with the display
  name (`formatDisplayName`).
- `showActive` hides a player from itself, from players it ignores, from
  non-contacts when its private chat is "friends" and from everyone when
  "off". Adding a friend shows the friend to the adder and, only when the
  adder's private chat is "friends", the adder to the friend.
- Private messages are delivered with no privacy or ignore check: the sender
  gets the echo and an online receiver the message, whatever their settings.
  The rank field is the sender's rights ordinal, not the chat icon.
- Login publishes the online status, a contact-list request and, when the
  saved current clan is set and no clan object is held, a clan join request.
  The status reaches every online player who has the name as a contact, in
  name-map order, regardless of their privacy or ignore lists. The contact
  request reloads the player's lists from account storage (Kotlin `split`
  keeps empty ignore entries). Logout publishes world 0 and the clan leave.
- Game packets are queued when sent and built at the end-of-tick flush:
  contact ranks, the ignore list, the player's clan, the clan member list and
  `pm_index` are read then. A failing packet is logged with every failure of
  that flush so far and the next one is written.
- Clans: `ClanRepository.get` builds a registered clan from an online owner's
  details; an offline owner's details are a fresh `PlayerDetails` (one clock
  read) with no clan name, so offline owners' clans come only through the
  management round trip (`RequestClanInfo` → `SendClanInfo` → account storage).
  Kotlin assignments of a clan's join and kick requirements call the Java
  setters, which clean and update the channel. The default "Global" clan is
  created with requirements "-1,-1,7,7", which parse to null join and message
  requirements: entering it fails with a NullPointerException that the
  management handler swallows, so nobody can join it.
- `clean` and `kick` fail with a NullPointerException on an entry of another
  world; `kick` has queued the notice for the entries before it.
- `getSmart` in the pinned `IoBuffer` always reads one byte (the signed peek
  never exceeds its unsigned value), so a public message of 128 characters or
  more, whose count the client writes as two bytes, is decoded from the wrong
  offset.

## Evidence

`fixtures/social/` holds **166 original JVM cases** (1,081 actions, 4,056
recorded calls, 20 injected failures; 475 session writes with their bytes,
538 management events taken in order) and the source hashes. The generator
`oracle/social/generate.py` (run by `cargo xtask social-oracle [--check]`)
compiles the unmodified classes listed in its `SCOPE` with Kotlin 1.8.20 on
JDK 11, inserts the original text of the selected members into the adapters
of `oracle/social/adapters.py`, and passes the compiled classes through
`oracle/social/tool/Rewrite.java`, which redirects the four wall-clock reads,
one `new Date()` (in `Util.nextMidnight`, never reached) and the two unseeded
`new Random()` of `CommunicationMessage` to recorded case inputs (census in
the provenance). Every case runs in a fresh class loader. After every action
the harness records the calls in order (each a recorded adapter), the result,
the exception, and the state: each player's contacts in map order, ignores,
clan fields, requirements, loot share, settings and the three strings
`PlayerDetails.save` writes back; the clan registry and every clan object
seen (owner, name, requirements, ranks, bans, members); the pending and
waiting management events and the clan-information requests; the queued
packets.

The corpus covers list limits (199/200), duplicates, empty names, name case
and spaces, all 36 combinations of adder/target private setting, contact and
ignore; removal from both lists under each private setting; eight packet
ranks from −2 to 9 (−2 and 9 fail); private messages online, offline,
ignored, hidden, muted, to self, to artificial players, bots and players
without a session, text forms and decoding failures; login and logout
fan-out (three of five online players hold the name; in a second world 16 of
23 do), login order, missing accounts, saved current clans with
online and offline owners; the clan-information round trip (queued events,
default clan with and without an account, disabled default clan); join
requirement 0..9 for five players including an administrator; full channels
and the server's unlimited one; bans and their expiry; messages under the
message requirement and the lock; kick rules; leave, rank changes, cleaning,
settings, deletion, entries from other worlds, logout; loot share; public,
global and clan chat, global mute, chat settings, message splitting; the
management server available, unavailable and without a session; cipher
values; and injected failures in messages, writes, the cipher, attributes,
logs, the pulse queue, varps, account storage and the mask registry.

```powershell
cargo test -p rs09-server --test social
cargo xtask social-oracle --check --java-home PATH_TO_JDK_11
```

The native test replays every action through `world::social` with a
recording host and compares every call, result, failure and snapshot.

## Required services

Everything not ported is a method of `social::Host`, with no defaults: game
messages (three kinds), interface strings and opening, attributes, action
locks, the chat-mask registration (`EntityFlag.Chat`; the mask encoding is
world/update_masks), the session write and outgoing opcode cipher, the
management-server session and write, varps, the world pulse queue (the
loot-share pulse body is `Social::run_pulse`), logging, `PlayerMonitor`,
account storage and the authenticator, command registration, the private
chat setting and `updateChatSettings`, `Repository.getPlayerByName`, the name
map's keys and the player list, the clock, the seed of an unseeded
`java.util.Random`, the bot chat hook and the world settings and server
constants. A binding must answer them from the real systems.

The repository port (`world::repository`, `world::disconnection`) already
publishes the logout events and makes a disconnecting player leave its clan
through its own host: a binding answers its `publish_player_status`,
`publish_clan_leave`, `clan`, `clan_owner` and `clan_leave` with
`Social::publish`, `Social::clan_of`, `Social::clan` and
`Social::clan_leave_default`, rather than calling `Social::logout_events` a
second time.

## Differences from the original

- The management queue is drained when the host calls `Social::pump`, on the
  caller's thread; the original drains it on a coroutine concurrently with
  the world, so the tick at which an event is handled is a host decision.
- Host calls cannot reenter the library during a call (it is borrowed
  mutably), so the original's fail-fast iterator failures on reentrant list
  changes cannot occur.
- Exception messages in logs carry the exception's first line only; JVM
  stack frames are not reproduced (the oracle drops them when recording).
- `formatDisplayName` upper-cases one unit with Rust's Unicode tables; JDK 11
  tables may differ outside ASCII. Names reaching it from packets are ASCII.
- `IoBuffer.putString` encodes with the default charset; native code writes
  UTF-8 (the oracle runs with `-Dfile.encoding=UTF-8`).
- `ClanRepository.clanWar` is never set by anything here, so the clan-wars
  hook in `leave` has no native form.

## Not ported

Management-server networking between worlds (the `WorldCommunicator`
reactor, `MSPacketRepository`'s other packets, `MSEventHandler`, world
statistics); quick chat (`QCRepository`); `ClanTabInterface` and
`ClanRepository.openSettings` callers; moderation tools (`PlayerMonitor`'s
database, abuse reports, mutes); `AIPlayer.handleIncomingChat` behaviour;
clan wars; `ClanMember` (no code uses it); and `MessageLog`, which no chat
path uses in the pinned source (its only subclass, `DuplicationLog`, is
referenced nowhere). The legacy `CommunicationInfo.save/parse(SQLTable)`
column paths stay with the player-metadata port.
