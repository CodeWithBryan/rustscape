# Player appearance state and body preparation

`player/appearance.rs` ports the entire pinned Appearance, BodyPart and Gender
API, including constructors, parsing, copying, exposed arrays/defaults, equipped
body preparation, human/NPC animations, transforms, render overrides, carts and
copters. Required player/render/properties/attribute/animation services are typed
receiver lookups. No field or array guard spans a service callback.

The real appearance object retains its canonical player. Canonical owned fields
construct it with the actor domain's one mutable default factory, rather than
independent defaults for each character. The actor collector counts that physical
owner field once and traces retained appearance receivers. Parts, body and
animation arrays contain no Player backreference: detached views remain usable
without retaining the player. The production `render_info` facade retains the
original owner, as the original RenderInfo.player field does. Low-level renderer
DTOs and complete foreign/local renderer graph ownership remain separate work.

Body preparation resolves actual equipment and RenderInfo through the canonical
owner. It performs the separate render-info lookups, sets prepared=true before
the nine equipment lookups, captures those items, and then reads their cached
definitions when drawing. Failures retain completed body writes and the prepared
flag. A second call in that cycle returns early. Actual updateInformation resets
the flag. This binding does not resolve a recycled wire index or UI row.

Preserved details include the 14-entry body array, seven animations and parts,
skin/wrists alias, female facial-hair1000, default colors ignored during Gender
generation, parse changing gender without regenerating parts, and ordered partial
look/color writes. Copy preserves the existing target arrays, aliases source
BodyPart references, and copies animation values/render ID; it does not copy NPC,
icons, body or minecart fields. Null getters, failed generation after gender
assignment, mutable static animation defaults and the original hat/hood/sleeve
rules remain observable.

Transform and cart/copter methods preserve separate player/property/appearance
lookups, captured animation arrays, nullable entries, index clamping, repeated
NPC combat lookups, default-priority constructors, failed prefixes and callback
reentry. Body preparation uses the supplied player argument even when it differs
from the appearance's retained player. Render override reads remain two separate
attribute calls; a callback can change the cached default between them.

Item/NPC configuration Animation objects now use the actual runtime Animation
fields. Configuration, appearance, NPC properties and Animator can share one
object, including mutations, nullable priority, scenery target and cached
definition. `Animation::from_shared` preserves identity. Configuration observation
still serializes the original constructor fields. Animation arrays permit null
elements, while ordinary parser output stays byte-for-byte identical. The prior
server duration/high-word quirks are unchanged; the RT4 decoder is separate.

448 JVM scenarios compile the complete unchanged Appearance/BodyPart/Gender and
reuse freshly reproduced complete original Container/Item/Animation classes.
Definition getters and weapon defaults are verbatim; required player/world
services are explicit recording adapters. Snapshots compare state, object/array
aliases, constructors/defaults, nullable getters, callbacks and failed prefixes.
Recording identity maps retain their objects on both sides; native addresses
cannot be recycled into false identity matches. Native regressions additionally
bind actual canonical equipment/render flags, retired appearance/render-info
receivers, detached arrays, per-world defaults and actual parser/runtime animation
identity/cached definitions.

Run player_appearance and affected ownership, inventory/configuration, NPC,
Animator/mask, world/render/headless and all example targets. Reproduce
player-appearance-oracle, animator/item-config/NPC-definition and affected existing
JVM gates. Full cache item/configuration/NPC/animation checks remain required when
their corresponding fields or bindings change. Use the Cargo settings in AGENTS.

The transport publisher still reads explicit prepared `world::appearance::Data`.
The native late writer kernel is now covered in docs/appearance-writer.md; its
full Properties/skills/attributes/familiar/profile/provider runtime bindings are
not complete. This stage does not silently project cached render values into that
writer, enable live Equip, or prove full player initialization, saving, gameplay,
parallel collection, performance or visual parity. All4,042 source rows remain
not_replaced; the complete Rust replacement objective remains active.
