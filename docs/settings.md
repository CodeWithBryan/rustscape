# Native player settings and fresh skill startup

`player/settings.rs` ports the pinned Settings state and operations. The complete
unchanged Java class runs against explicit owning-player, property, container,
item configuration and packet-service adapters. The current corpus contains
871 scenarios, covering every operation, partial failed loads, callback
reentrancy, receiver capture, numeric extremes and emitted values/bytes.

The original behavior includes several unusual details. The three-argument chat
update ignores its private-chat argument; the special-attack setter toggles the
flag regardless of its argument. Special energy wraps signed arithmetic without
clamping. Run energy preserves NaN and signed zero while applying the original
finite bounds. The saved-settings parser assigns fields in source order, keeping
earlier writes when a later key is missing or has an invalid value/type.

Weight calculation counts each occupied inventory slot once, ignores nonpositive
inventory weights, and includes signed equipment weights from only the first
eleven slots. Equipment's compound assignment captures the previous weight before
configuration callbacks, while inventory reads it after the weight lookup. The
native port retains this distinction. Session/dispatcher/property handles are
captured before argument evaluation, including null receivers and callback
mutations. Shared settings fields remain unlocked during external calls.

```powershell
cargo test -p rs09-server --test settings --test skills
cargo xtask settings-oracle --check --python PYTHON --java-home JDK11
cargo xtask skills-oracle --check --python PYTHON --java-home JDK11
cargo test -p rs09-server --example movement_session --example npc_session
```

This is a Settings library with explicit owning-world services, rather than a
complete Player/settings-button/profile integration. Runtime use of the full
Settings update and its chat/control packets remains pending. Server varps are
not substitutes for the original client's separate preference-script operations;
the Rust client must preserve the original volume/preference boundary.

The opt-in gameframe fixture now constructs the existing native Skills state,
refreshes its 24 skill packets and configures combat through the original native
methods. Fresh values are hitpoints 10/experience 1154 and level 1/experience 0 for
other skills, with prayer 1. A packet regression checks each slot. The session
still supplies a fresh player, no assist session or saved level-up attributes,
and empty XP/perk/equipment/content services; unsupported skill effects return
an explicit error. This does not load a complete saved profile or implement XP
gameplay. The headless observer exposes each received skill's boosted level and
wire experience, leaving unreceived slots null.

The combined held checkpoint is `runs/settings-verification/report.json`
(SHA256 b583011a98f9443177236af2b17c776d620bd59e6a5729b2faf663277febbdda).
It passes 317 workspace tests, 22 example tests and 37 checks with exact source,
fixture, oracle and pack hashes. The current Windows and WASM clients show the
fresh skill values; headless data matches canonical state. Current Windows
looping/finite PCM and location ambient have separate original comparisons and
actual recorded-output replay proof in the same checkpoint. Full saved profiles,
XP/content and live Settings controls remain separate work.

`runs/settings-verification/source-and-test-inputs.zip` preserves 4,755 exact
source, fixture, oracle, pack and supporting build/documentation files for this
checkpoint. `source-bundle.json` records every entry hash and the archive hash;
reference checkouts, toolchains and full cache assets are retrieved separately.
