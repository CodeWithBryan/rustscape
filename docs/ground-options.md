# Owning-world ground option tables

`world::ground_options` joins actual GroundItem state, canonical actor-domain
identities, cached Item definitions, InteractPlugin option initialization and
ItemDefinition's native local/global registry. Ground constructors in both live
examples now install these real tables instead of discarding default interaction.
The registry installs the actual native `pickup::Program` under `take`.

Names come from the Item's cached definition. Ground versus inventory actions
depend on its current location, independently of subtype. Handler lookup uses
the current item ID and the definition registry's original nonnull fallback.
This difference matters when unmasked charge changes an ID without changing its
cached definition. Local `option:name` configuration precedes the global key;
wrong types fail with the already-created table and completed prefix intact.
Repeated default initialization leaves an initialized table untouched. Old
exposed arrays retain their original options after reinitialization.

Each binding records a stable actor-domain identity, distinct even for two State
values sharing one scheduler. Captures retain the exact canonical node and ground
table. Exposed table views retain the actual GroundSpawn subtype owner when
applicable. Internal edges contain no node leases or self-owning ground anchors.
Foreign-world tables and foreign native handler IDs are rejected.

Native handler IDs are process-unique, never reused and never reconstructed from
labels or observations. The registry conservatively retains actual handler
objects in an arena. Ground bindings and exposed old option arrays retain that
arena; dropping an array does not keep a dead ground item or actor domain alive.
Arena reclamation per individual handler and general plugin-owned reference
cycles remain separate work. No speed or total-memory improvement is claimed by
this binding. No registry, definition, item or table lock spans locale callbacks.

The new JVM corpus joins complete unchanged InteractPlugin/Option/OptionHandler
and PickupPlugin classes with original ItemDefinition lookup/registration,
Definition.getConfiguration, action-array getters and Item ID/charge methods.
112 scenarios compare real option/handler identities, cached versus current
definition behavior, location branches, ROOT lowercasing, retained arrays and
failed prefixes. Runtime node construction and metadata fields are explicit
adapters; PickupPlugin.handle is not executed by this corpus. Native regressions
add actual object dispatch types, cross-world rejection, handler lifetime and
exact GroundSpawn retention/release. The live example integration regression
uses its actual ground construction service and canonical actor state.

Run `cargo test -p rs09-server --test ground_options` and
`cargo xtask ground-options-oracle --check --java-home runs/jdk11/jdk-11.0.32.1+1`.
Also reproduce existing option, registry, ground, player-walk and scheduler gates
listed in AGENTS.md. The current examples explicitly use the pinned JDK ROOT
lowercase implementation; another server default locale requires a real binding.

The next integration remains authoritative packet dispatch through legacy
navigation/arrival, then Pickup's real profile/container/content services.
These tables do not enable Take/Drop, complete server startup or a playable full
game. Existing isolated server processes and checkpoint artifacts stay unchanged.
