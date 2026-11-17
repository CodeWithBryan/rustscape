# Native interfaces and gameframe startup

`world/interfaces.rs` implements the original component definitions, interface
types, configuration parser and component/InterfaceManager lifecycle methods.
The latest lifecycle and ownership evidence is in `interface-manager.md`.
The pinned server's 162 configuration rows supply fixed and resizable layout
positions. Runtime configuration and packet orchestration are Rust; Java/Kotlin
run only in the differential oracle.

The oracle executes 628 scenarios using the original component open/close/plugin
lookup and complete original InterfaceManager class,
and LoginConfiguration.sendGameConfiguration ordering. Complete original
ComponentDefinition, InterfaceType and ClientInfo and the original packet
writers run against explicit player, listener, weapon, familiar, friends,
tutorial, login-content and packet-sink adapters. These comparisons do not
establish complete Player, content/listener or login-content parity.

Definitions and components retain shared identity. The configuration loader
parses an existing definition, then parses and publishes a replacement even
after success; existing components retain the first definition and its plugin.
The Rust implementation preserves that behavior, partial failed writes,
callback ordering/reentrancy, default tab handling, signed wrapping packet
counters, and the original typed-window overload's unusual SingleTab assignment.
No lock remains held across external component plugin or close callbacks.

Run the portable comparisons and fresh JVM reproduction:

```powershell
cargo test -p rs09-server --test interfaces
cargo test -p xtask --test game_packets
cargo test -p rs09-server --example movement_session --example npc_session
cargo xtask interfaces-oracle --check --python PYTHON --java-home JDK11
cargo xtask game-packets-oracle --check --python PYTHON --java-home JDK11
```

The controlled movement/NPC sessions accept `--gameframe`, optionally with
`--interface-config PATH`. This binds original interface startup to the native
authenticated session's ClientInfo, opens root 548/746 and chat/default tabs,
and sends a real empty 28-slot native inventory and canonical run energy.
TrackingDisplayUpdate (243) changes dimensions before native mode switching;
Ping (93) updates keepalive time. Worldspace and minimap walks continue through
the existing original walk-request/movement kernels and cache collision.
The initial normal scene rebuild precedes all interface packets, matching the
first call in the original configureLobby. A regression checks the actual first
flush and subsequent player appearance. The fixture enters the game directly;
the complete lobby screen and lobby-to-game lifecycle remain pending.

This fixture supplies an explicit post-tutorial state, basic weapon/friends
component identities and empty content callbacks. Complete player profiles,
tutorial progression, weapon/friends plugins, skills/settings/equipment, welcome
content, quests, emotes, login plugins, persistence and interface actions remain
unimplemented here. Inventory quantities are not fabricated. Full gameplay
startup and full game replacement remain required.

With `--npc-trace NEW_FILE`, diagnostic records also include canonical self
position and UI root, mode, counter, dimensions and last handled walk. A handled
walk is not proof of accepted movement; compare subsequent canonical positions
and decoded client updates. Tracing uses synchronous diagnostic file writes and
is excluded from performance or player-capacity claims.

The immutable `runs/gameframe-verification/report.json` records 310 workspace
tests, 22 example tests and 41 successful checks with 330 Rust/ten Cargo inputs
held unchanged during final checks. Actual archived Windows and WASM clients
connect to the scene-first fixture. The browser's real minimap click (opcode 39) reaches
the server-confirmed destination; Windows captures show server-authored UI,
moving/animated NPCs, native destination clearing and finite effect submissions.
The failed first-frame case is retained alongside the regression and corrected
proof. This does not complete lobby/profile/content, renderer or audio parity.
