# Native item option registry

`definitions::option_handler` ports ItemDefinition.getOptionHandler,
setOptionHandler and the shared map returned by getOptionHandlers. Item-local
`option:name` configuration takes precedence over the global entry. Lookup is
case-sensitive here; Option.defaultHandler owns the separate locale-lowering
step. A null name produces `option:null` in configuration and a null key in the
global map. A wrong local value type fails before global fallback. Null values
remain distinguishable from missing entries, and setOptionHandler returns true
only when the previous value was nonnull.

The original forId normally returns a fresh default definition for an absent
ID, including22937. The missing-definition log branch is preserved for an
explicit null return at22937; it is not triggered by ordinary missing cache IDs.
The context's default configuration lookup uses the actual native definition
handlers map. Lookup and log services can reenter without held native locks.

Handler objects retain real native NativeOptionHandler payloads and shared
identity. `pickup::Program` invokes the existing native PickupPlugin control
flow, and native_handler plus pickup::register install that same object into
the default map. Definition overrides use typed Property::OptionHandler values.
The existing 800-scenario pickup replay executes this concrete Program. The
native registry regression checks startup registration, instance identity,
nonnull cache fallback, typed payload access and local override precedence.
The owning world must bind these objects to its existing dispatch IDs and
release read guards before invoking callbacks. Diagnostic JSON handler labels
are observations; they do not reconstruct executable programs.

The JVM corpus runs 134 scenarios using the unchanged lookup/registration/map
accessor and Definition.getConfiguration bodies. The original forId fallback
body runs under a recording lookup adapter. Null keys/values, aliases, local type
failures, late registry reads, reentry and failure prefixes are compared. Native
replay uses actual native definition and handler objects. Map content snapshots
use authored sorted keys; they do not claim Java iterator or map-view behavior.
The Rust map API currently exposes lookup, put, remove, clear, contains and size.
Full iterator/views, plugin startup, owning-world table/dispatch integration,
packet-to-approach-to-inventory binding and live Take remain pending.

The shared owner wrapper also preserves compact inline storage: the Windows
x86_64 probe measures Shared, WeakShared and Option<Shared> at16bytes each,
compared with24bytes before the wrapper change. Weak upgrades still reconstruct
the actual owner after an anchor retires. This measures inline handles only;
owner allocations, object memory, allocator costs, total RAM and throughput are
not measured. Exact probe source/toolchain/binary IDs are under runs/shared-layout.

Run `cargo test -p rs09-server --test item_option_registry --test shared_ownership`
and `cargo xtask item-option-registry-oracle --check` with Python3.10+/JDK11.
Shared regression gates are in AGENTS.md.
