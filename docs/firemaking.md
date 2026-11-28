# Firemaking (`world::firemaking`)

Scope: the pinned `content/global/skill/firemaking`.

| original | here |
|---|---|
| `Log` (the 18 constants, `forId`) | `LOG_VALUES`, `log_for_id` |
| `FireMakingPulse` (a tinderbox) and `BarbFirePulse` (a bow), with `SkillPulse`'s `start`, `pulse` and `stop` | `Pulse` with `Kind::Tinderbox` and `Kind::Bow` |
| `FiremakingListener`'s `logs` and `bows` | `LOGS`, `BOWS` |

The bodies are functions over `firemaking::Host`; every call the original
makes outside these classes is a required method with no default.

## Evidence, and what it does not show

**There is no JVM differential corpus for this module.** It was ported by
reading the pinned sources.

- `cargo test -p rs09-server --lib firemaking`: a recording host written
  from the same sources checks the calls of the main branches in order (an
  inventory log lit on the third tick, a log lit at once after a recent
  fire, a failed roll, each refusal's message, a log already on the ground,
  the bow's training and level checks and its training progress, the bow's
  silent refusal of cursed magic logs and jogre bones). It shows agreement
  with one reading of the sources, not with the original classes running.
- The live world's `gather` scenario (`firelog`) lights two logs with a
  tinderbox and waits for the first fire's ashes. Nothing live uses a bow.

## Kept quirks

- `SkillPulse.stop()` sends "The fire catches and the logs begin to burn."
  whenever the pulse stops, including when a requirement fails mid-pulse.
- `BarbFirePulse.createFire`'s `face(player, fireObject)` does nothing (a
  Scenery is neither of that function's cases), so only the tinderbox pulse
  turns the player to the fire.
- Pyre logs and the other listed logs that are no `Log` constant start a
  pulse that fails its first check without a message.
- The success roll divides by the log's level (or barbarian level), so a
  jogre bone or cursed log's barbarian level of -1 never reaches it.

## In the live world

`world/live/items/firemaking.rs` installs the listener's two inventory
registrations (a tinderbox or a bow used with logs) and runs the pulses.

- The fire is `SceneryBuilder.add(object, life, ash)` over the scene. The
  ashes are created by a content tick listener when the scene's pulse hands
  the ash back, on the tick the fire goes out or the next one.
- **Not installed:** the listener's three ground item registrations (a
  tinderbox or bow used on logs on the ground, and the logs' "light"
  option). The live world routes no ground item listener.
- `LitFireEvent` is skipped by name, so no diary or tutorial hook sees a
  fire. `replaceSlot` is joined only for an unstacked log.
- Cooking on a lit fire is not checked.
