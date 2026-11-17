# Scenery click dispatch and door options

`world/scenery_action.rs` ports the complete selected
`PacketProcessor.processSceneryAction` method. It accepts shared protocol
`ClientAction::SceneryAction` values and retains native scenery handles across
object lookup, lever construction, morph selection, option selection, debugging,
script reset, listeners, plugins and legacy interaction dispatch.

Lookup still runs before the hardcoded lever exceptions. Family-crest levers
read the player's plane again when constructing the odd variant. A missing morph
option passes the original scenery to invalid-interaction handling. Wrapper
status is captured before debug callbacks; IDs, locations, configuration and
listener arguments are read at their original later points. A null varbit
definition triggers a fresh definition read for the varp debug text. The selected
option survives callback replacement of its array, while the final legacy call
obtains the child's interaction receiver again.

`world/door_option.rs` ports DoorManagingPlugin's registration, handle and
destination decisions. This preserves the cupboard guard's case-sensitive name
test and plane-independent cardinal adjacency, furniture opening fall-through,
active-only opening, unconditional closing, wrapping next/previous IDs, wardrobe
animation-before-audio ordering, 80-tick furniture replacement requests, trapdoor
teleports, the mithril-door exception and wall-shape destination selection.
String.toLowerCase's default locale is an explicit owning-world service; the
oracle selects Locale.ROOT and uses the existing native JDK ROOT implementation.
The destination API accepts a player after the caller performs the original cast.

`oracle/scenery-action/generate.py` compiles the unchanged full selected Kotlin
method and unchanged complete DoorManagingPlugin Java class. Original packed
Location, equality, isNextTo, transform and string methods are used as well.
1,229 scenarios compare every command's state and ordered callbacks, including
nulls, indices, all cardinal/diagonal debug directions, lever exceptions,
listener/plugin short circuits, furniture/trapdoor decisions, wrapping IDs,
retained receivers, nested dispatch and callback failure prefixes. Fixture and
source hashes are pinned; `--check` never rewrites them. A separate regression
decodes the actual RT4 Open packet for object 12348 at (3207,3217,0).

```powershell
cargo test -p rs09-server --test scenery_action --test doors --test scenery --test scenery_node
cargo test -p xtask --test game_packets
cargo xtask scenery-action-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
cargo xtask doors-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
cargo xtask game-packets-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
```

Object construction, morphs, retained interaction/option handles, invalid and
legacy interactions, listener/plugin registries and player APIs are explicit
dispatch services. The door-option oracle records transforms, replacements,
audio, animation, teleport and door-rule/destination calls. Their separate native
kernels already have independent corpora; their runtime composition is pending.
This suite does not establish live click-to-approach/arrival, full option
initialization/content registration, original-locale selection, scene tick/fanout,
audio delivery or full Node/Player/PacketProcessor replacement. The existing live
endpoints are unchanged. Source inventory entries remain `not_replaced`.

The canonical player-walking assembly now supports native scenery targets and
interaction approach captures. A reference index reuses a live node binding for
the exact same object; equal coordinates do not merge different objects. The
target's shape, rotation, retained definition dimensions/walking flags and
frozen node size/direction feed the original destination and path dispatch.
Native walking, managers, the runner and queue phases are shared with ordinary
player movement. The owning world supplies content/door destinations, permitted
tiles and the final legacy arrival callback.

`world/interaction.rs` retains Option's final name/index, mutable handler and the
anonymous pulse's player/target/option captures. Movement construction captures
the handler used for routing; later arrival retains the selected option even if
the live option array changes. Collection follows both the mutable movement
fields and immutable arrival/check captures, releasing detached cycles after
their final external handle is dropped. Scheduling a foreign world's movement
is rejected instead of rebinding matching numeric IDs. A target without a
retained definition returns an error when its view is requested; ordinary native
cache constructors always carry the definition.

The separate joined player-walk JVM corpus grows from 111 to 190 scenarios.
Its 79 scenery additions cover wall/decor/rectangle shapes, rotations and sizes,
walking flags, custom and fallback destinations, movement, inactive/moved
targets, retained/replaced options, run flags, reentrancy and failures. Complete
unchanged Option is compiled along with the existing original movement classes;
default-handler lookups and arrival content are recording adapters. The
harness's approach class has the original InteractPlugin$3 telemetry identity
explicitly normalized. Thirteen native assembly tests include four new ownership
regressions and 192 successive scenery approach replacements.

This is runtime movement assembly support. The live packet dispatcher still
needs native option initialization, InteractPlugin's lock/zone/facing/content
arrival behavior, and scene fanout before the door can be called playable.

The [legacy interaction bodies](legacy-interactions.md) now provide the original
orchestration and actual captured arrival decisions as a native library, with
4,865 additional JVM cases. Their owning-world profile, zone, handler and facing
mask services are still required before live door delivery is complete.
