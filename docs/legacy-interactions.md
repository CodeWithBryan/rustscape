# Legacy interaction dispatch and arrival

`world/legacy_interaction.rs` ports the complete selected InteractPlugin.handle,
handleInvalidInteraction, handleWalkOption, handleDefaultOption and the actual
anonymous walk, invalid and delayed arrival bodies. `Task::from(&Approach)`
retains the canonical movement assembly's original player, target and option.
Item actions and option table/default initialization remain separate.

The port keeps the initial hasHandler/walk decisions while re-reading the live
handler at its later source points. The manager receiver is captured before
constructing a pulse, and a null manager still allows constructor argument
effects before the original null invocation fails. Routing receives the handler
evaluated before construction; arrival retains the selected option, whose handler
may subsequently change. Canonical World exposes captured-handler construction
and captured-manager scheduling so bindings need not look these values up again.
A native regression replaces both live fields and verifies the original routing
handler and original manager are used.

Walking/invalid arrivals face the captured target before checking interaction
locks or zone interception. The default delayed arrival does not face and does
not catch exceptions. Walking/invalid bodies catch, print and log exceptions,
then finish; outer handle has its own catch. Printing precedes virtual exception
message lookup. Immediate default handling checks the zone, stops combat, invokes
the handler and schedules an unhandled action only when it returns false. The
outer interaction event still follows a helper's ordinary early return.

The production node_face_location helper ports Node's reflected mathematical
center. Intermediate top-right coordinates are packed before center arithmetic;
the result uses the from-location's plane. Integer arithmetic preserves the
original finite half-tile calculation and packed boundaries.

The JVM harness compiles the complete unchanged InteractPlugin, Option and
OptionHandler classes and invokes the selected methods and real anonymous
bodies. Original Node center/face methods and Location/Vector geometry are used.
Recording managers/constructors, locks, zones, plugin APIs, combat, dispatch,
facing and exception/log services allow controlled mutation and reentry. The
default locale is explicitly ROOT. All 4,865 cases include 769 orchestration
scenarios and 4,096 geometry boundaries. They compare state and ordered effects
after every command, including nullable options/players/locations/managers,
retained captures, changed handlers, replaced receivers, failure prefixes,
Unicode and delayed-versus-caught errors.

Both this harness and scenery-action now explicitly use UTF-8 for logical JSON
transport and assert every input scenario round-trips. The original JDK11
Windows output encoding had collapsed five scenery oracle input names to
question marks. Correcting that adapter preserves the original logical strings;
it does not change gameplay packet string encoding or pinned reference sources.
Historical reports retain their exact earlier source/fixture archives.

```powershell
cargo test -p rs09-server --test legacy_interaction --test player_walk --test scenery_action
cargo xtask legacy-interaction-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
cargo xtask scenery-action-oracle --check --python PATH_TO_PYTHON --java-home PATH_TO_JDK11
```

The optional [canonical runtime binding](interaction-runtime.md) now connects
these bodies to native movement and plain-task scheduling. Concrete Player
locks/properties/zones, handler registries, real facing update masks, complete
option initialization, native scene fanout and live click-to-door delivery
remain to be bound. No existing live
endpoint has changed. Complete classes, game/content/headless/profile and
performance/capacity acceptance remain open; inventory statuses stay not_replaced.
