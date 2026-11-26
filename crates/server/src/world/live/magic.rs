//! Spells and teleports in the live world.
//!
//! - [`SpellHost`] is `world::spells::Host` over the live world: the combat
//!   spells of the standard and ancient books (`spells::table`) in a
//!   registry this world owns, answered from the canonical player (skills,
//!   inventory, equipment, attributes, the autocast spell, the spell book),
//!   the NPC rows, the animators, the live projectiles and the combat pulse.
//!   The combat hosts' spell services (`combat_swing::Host`,
//!   `combat_pulse::Host`) answer through `spells::combat`.
//! - [`TeleHost`] is `world::teleport::Host`: one `TeleportManager` per
//!   entity, its pulses submitted to the world runner and dispatched here by
//!   identity (body, then the stop override), the destination written to the
//!   walking queue's teleport location, the zone monitor's teleport check and
//!   `isTeleBlocked`.
//! - [`spell_listeners`]: `SpellListeners.run` with the standard book's
//!   ported listeners (the Varrock, Lumbridge and Falador teleports, ported
//!   from the pinned `ModernListeners.kt` and `SpellListener.kt`); every
//!   other listener the original registers is refused by name, and a key the
//!   original does not register does nothing, as there.
//!
//! What is refused by name (see docs/live-world.md): the `fireEffect` of the
//! spells with effects (binds, curses, god spells, teleblock and the ancient
//! spells), NPC casters' `DefaultCombatSpell`, multi-target ancient spells,
//! the non-combat `MagicSpell` plugins (enchanting, charge, teleother,
//! telekinetic grab, bounty locate, the ancient teleports, the lunar
//! plugins), the other `SpellListener` listeners (home teleport, the
//! diary- and quest-gated teleports, house teleport, alchemy, superheat,
//! bones to bananas/peaches, charge orb, the whole lunar book), and the
//! events (`SpellCastEvent`, `TeleportEvent`: entity event hooks are not
//! ported; a named skip).
use super::{P, host::Live, inner::Inner};
use crate::{
    definitions::item::Shared,
    inventory::item::{Handle as ItemHandle, Item},
    player::attributes::{AttributeObject, AttributeValue},
    pulses,
    world::{
        action_locks,
        animation::{self as anim, Animation as NativeAnimation, Graphics as NativeGraphics},
        combat_pulse as cp, combat_swing as cs,
        doors::Location,
        spells::{
            self as sp, Animation, Graphics, NodeKind, Priority, Projectile, ProjectileTarget,
            Spell, SpellBook, SpellClass, Value,
        },
        teleport::{self as tp, Manager, TeleportPulse, TeleportType},
        zones,
    },
};
use rs09_game_core::java_lower::lowercase_root;
use std::{collections::HashMap, sync::Arc};

/// A registered spell: the book it was registered in and its button.
/// Equal keys are the same spell object (the registry holds one per key).
pub type SpellKey = (SpellBook, i32);

/// The spells and teleports of the live world.
pub struct MagicWorld {
    /// `SpellBook.register`: the combat spells of the standard and ancient
    /// books, by book and button.
    pub registry: HashMap<SpellKey, Spell>,
    /// Each entity's `TeleportManager` (the Entity's final `teleporter`).
    pub teleporters: HashMap<usize, Manager<P>>,
    /// The teleport pulses submitted to the world runner.
    pub teleports: Vec<TeleportPulse<P>>,
}

impl Default for MagicWorld {
    fn default() -> Self {
        Self {
            registry: sp::table::combat_spells()
                .into_iter()
                .map(|(book, button, spell)| ((book, button), spell))
                .collect(),
            teleporters: HashMap::new(),
            teleports: Vec::new(),
        }
    }
}

fn serr(e: impl std::fmt::Debug) -> sp::Error {
    sp::Error::Binding(format!("{e:?}"))
}
fn srefuse<T>(what: &str) -> sp::Result<T> {
    Err(sp::Error::Binding(format!(
        "not ported in the live world: {what}"
    )))
}
fn from_cs(e: cs::Error) -> sp::Error {
    match e {
        cs::Error::Null => sp::Error::Null,
        cs::Error::Index => sp::Error::Index,
        cs::Error::Binding(text) => sp::Error::Binding(text),
    }
}
fn terr(e: impl std::fmt::Debug) -> tp::Error {
    tp::Error::Binding(format!("{e:?}"))
}
fn trefuse<T>(what: &str) -> tp::Result<T> {
    Err(tp::Error::Binding(format!(
        "not ported in the live world: {what}"
    )))
}

/// The `spell:runes` list (`ArrayList<Item>`) as an attribute object.
#[derive(Clone, Debug)]
pub struct RuneList(pub Vec<(i32, i32)>);
impl std::fmt::Display for RuneList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // ArrayList.toString of Items: [Item(id, amount), ...]; the list's
        // text is never read by these classes.
        let parts: Vec<String> = self.0.iter().map(|(i, a)| format!("{i}x{a}")).collect();
        write!(f, "[{}]", parts.join(", "))
    }
}

/// The `tele-pulse` attribute: the running TeleportPulse object.
pub struct TelePulse(pub TeleportPulse<P>);
impl std::fmt::Display for TelePulse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.kind().class_name())
    }
}

fn native_priority(p: Priority) -> anim::Priority {
    match p {
        Priority::Low => anim::Priority::Low,
        Priority::Mid => anim::Priority::Mid,
        Priority::High => anim::Priority::High,
        Priority::VeryHigh => anim::Priority::VeryHigh,
    }
}
fn spell_priority(p: Option<anim::Priority>) -> Priority {
    match p {
        Some(anim::Priority::Low) => Priority::Low,
        Some(anim::Priority::High) => Priority::High,
        Some(anim::Priority::VeryHigh) => Priority::VeryHigh,
        _ => Priority::Mid,
    }
}
fn native_animation(a: Animation) -> NativeAnimation {
    NativeAnimation::with(a.id, a.delay, Some(native_priority(a.priority)))
}
fn spell_animation(a: &NativeAnimation) -> Animation {
    Animation {
        id: a.id(),
        delay: a.delay(),
        priority: spell_priority(a.priority()),
    }
}

/// The classes of the registered combat spells, by key (the registry's
/// shape, for callers without the world: the weapon tab's lookup).
pub fn combat_classes() -> &'static HashMap<SpellKey, SpellClass> {
    static CLASSES: std::sync::OnceLock<HashMap<SpellKey, SpellClass>> = std::sync::OnceLock::new();
    CLASSES.get_or_init(|| {
        sp::table::combat_spells()
            .into_iter()
            .map(|(book, button, spell)| ((book, button), spell.class))
            .collect()
    })
}

/// `SpellBook.<book>.getSpell(id)` for the weapon tab's autocast selection:
/// a combat spell, another registered spell (the cast fails), or null. A
/// combat spell whose `fireEffect` is not ported is refused by name.
pub fn autocast_lookup(
    book: SpellBook,
    id: i32,
) -> Result<crate::world::player_tabs::weapon::Lookup<crate::player::save::Autocast>, String> {
    use crate::world::player_tabs::weapon::Lookup;
    if let Some(class) = combat_classes().get(&(book, id)) {
        if let Err(name) = Live::fire_effect_ported(*class) {
            return Err(format!(
                "not ported in the live world: {name}.fireEffect (an autocast spell with an effect)"
            ));
        }
        return Ok(Lookup::Combat(crate::player::save::Autocast {
            book: book.ordinal(),
            spell_id: id,
        }));
    }
    Ok(if refused_registration(book, id).is_some() {
        Lookup::Other
    } else {
        Lookup::None
    })
}

impl Live {
    /// The player's weapon and side tabs.
    pub(crate) fn tabs(&mut self, p: &P) -> super::tabs::Tabs<'_> {
        super::tabs::Tabs {
            inner: &mut self.inner,
            p: p.clone(),
            walk: Some((&mut self.walk, &mut self.deferred)),
        }
    }

    /// A spell class whose `fireEffect` is the empty `CombatSpell.fireEffect`
    /// (ported: nothing to do), or the name of the class whose effect is not.
    pub(crate) fn fire_effect_ported(class: SpellClass) -> Result<(), &'static str> {
        match class {
            SpellClass::Air
            | SpellClass::Water
            | SpellClass::Earth
            | SpellClass::Fire
            | SpellClass::CrumbleUndead
            | SpellClass::IbanBlast
            | SpellClass::MagicDart
            | SpellClass::SonicWave => Ok(()),
            other => Err(other.name()),
        }
    }

    /// The registered spell at a persisted (book ordinal, id) pair.
    pub(crate) fn spell_key(&self, book: i32, id: i32) -> Option<SpellKey> {
        let book = *SpellBook::ALL.get(usize::try_from(book).ok()?)?;
        self.round
            .magic
            .registry
            .contains_key(&(book, id))
            .then_some((book, id))
    }

    /// `properties.getSpell()` of a player.
    pub(crate) fn selected_spell(&mut self, p: &P) -> cs::Result<Option<SpellKey>> {
        if !self.inner.is_player_node(p.node()) {
            return Ok(None);
        }
        let spell = self
            .inner
            .with_profile(p, |profile| profile.properties.spell)
            .map_err(|e| cs::Error::Binding(format!("{e:?}")))?;
        match spell {
            None => Ok(None),
            Some((book, id)) => self
                .spell_key(book, id)
                .map(Some)
                .ok_or_else(|| cs::Error::Binding(format!("an unregistered spell {book}:{id}"))),
        }
    }

    /// `properties.getAutocastSpell()` of a player (the persisted autocast).
    pub(crate) fn autocast_key(&mut self, p: &P) -> cs::Result<Option<SpellKey>> {
        if !self.inner.is_player_node(p.node()) {
            return Ok(None);
        }
        let autocast = self
            .inner
            .with_profile(p, |profile| profile.persisted.spells.autocast)
            .map_err(|e| cs::Error::Binding(format!("{e:?}")))?;
        match autocast {
            None => Ok(None),
            Some(a) => {
                let key = self.spell_key(a.book, a.spell_id).ok_or_else(|| {
                    cs::Error::Binding(format!(
                        "an unregistered autocast spell {}:{}",
                        a.book, a.spell_id
                    ))
                })?;
                let class = self.round.magic.registry[&key].class;
                if let Err(name) = Self::fire_effect_ported(class) {
                    return Err(cs::Error::Binding(format!(
                        "not ported in the live world: {name}.fireEffect (an autocast spell with an effect)"
                    )));
                }
                Ok(Some(key))
            }
        }
    }

    /// `playGlobalAudio(location, id, delay, loops, radius)`: the zone
    /// origin (26) and the area sound (97) for every player within radius.
    pub(crate) fn global_audio_full(
        &mut self,
        at: Location,
        id: i32,
        delay: i32,
        loops: i32,
        radius: i32,
    ) -> cs::Result<()> {
        let players = self
            .local(
                at.clone(),
                radius,
                crate::world::entity_regions::Select::Players,
            )
            .map_err(|e| cs::Error::Binding(format!("{e:?}")))?;
        let [x, y, z] = at.coordinates();
        let base = Location::new(x & !7, y & !7, z);
        for player in players {
            let Some(p) = self.player_of(player.node) else {
                continue;
            };
            let offset = super::ground_items::World::with(self, |w| {
                crate::world::chunk_sync::scene_offset(w, &player, &base)
            })
            .map_err(|e| cs::Error::Binding(format!("{e:?}")))?;
            let mut origin = rs09_protocol::buffer::Writer::new();
            origin.p1neg(offset[0]);
            origin.p1(offset[1]);
            self.inner.queue_native(
                &p,
                rs09_protocol::game::Frame {
                    opcode: 26,
                    payload: origin.into_vec(),
                },
            );
            self.inner.queue_native(
                &p,
                rs09_protocol::game::updates::area_sound(x, y, id, radius, loops, delay),
            );
        }
        Ok(())
    }

    /// The spell services of the combat hosts, run with a [`SpellHost`].
    pub(crate) fn with_spells<T>(&mut self, f: impl FnOnce(&mut SpellHost<'_>) -> T) -> T {
        let mut host = SpellHost {
            live: self,
            book: None,
        };
        f(&mut host)
    }

    /// A teleport operation with a [`TeleHost`].
    pub(crate) fn with_teleports<T>(&mut self, f: impl FnOnce(&mut TeleHost<'_>) -> T) -> T {
        f(&mut TeleHost { live: self })
    }

    /// A world pulse this module submitted (a teleport), by identity: its
    /// body.
    pub fn magic_pulse_body(&mut self, pulse: &pulses::Handle) -> Option<Result<bool, String>> {
        let teleport = self
            .round
            .magic
            .teleports
            .iter()
            .find(|t| t.handle().same(pulse))
            .cloned()?;
        let node = teleport.entity().node();
        if !self.inner.players.contains_key(&node) && !self.inner.is_npc_node(node) {
            // The entity left the world: finished, untracked first so the
            // runner's stop is the base stop.
            self.round
                .magic
                .teleports
                .retain(|t| !t.handle().same(pulse));
            return Some(Ok(true));
        }
        let result = self
            .with_teleports(|h| teleport.pulse(h))
            .map_err(|e| format!("{e:?}"));
        self.drain_all();
        Some(result)
    }

    /// `Pulse.stop()` of a world pulse this module submitted: the teleport
    /// pulse's stop override; `None` for another pulse.
    pub fn magic_pulse_stop(&mut self, pulse: &pulses::Handle) -> Option<Result<(), String>> {
        let at = self
            .round
            .magic
            .teleports
            .iter()
            .position(|t| t.handle().same(pulse))?;
        let teleport = self.round.magic.teleports.remove(at);
        let result = self
            .with_teleports(|h| teleport.stop(h))
            .map_err(|e| format!("{e:?}"));
        self.drain_all();
        Some(result)
    }

    /// `Entity.isTeleporting()`.
    pub fn entity_is_teleporting(&mut self, p: &P) -> Result<bool, String> {
        self.with_teleports(|h| tp::is_teleporting(h, p))
            .map_err(|e| format!("{e:?}"))
    }

    /// A player left: its teleporter and the spell state this world keeps.
    pub fn magic_logout(&mut self, node: usize) {
        self.round.magic.teleporters.remove(&node);
        // A teleport still running for the player (the logout can come
        // before the move, and the pulse checks no node): stopped with the
        // base stop and untracked, so the runner drops it without running
        // its body or the stop override against a removed player.
        self.round.magic.teleports.retain(|t| {
            if t.entity().node() == node {
                pulses::stop_base(t.handle());
                false
            } else {
                true
            }
        });
    }
}

// ------------------------------------------------------------------ spells

/// `world::spells::Host` over the live world. The spell book field is held
/// here while a library call runs and written back when the host is dropped.
pub struct SpellHost<'a> {
    pub live: &'a mut Live,
    book: Option<(P, i32)>,
}

impl Drop for SpellHost<'_> {
    fn drop(&mut self) {
        if let Some((p, book)) = self.book.take() {
            if self.live.inner.is_player_node(p.node()) {
                let _ = self
                    .live
                    .inner
                    .with_profile(&p, |profile| profile.spell_book = book);
            }
        }
    }
}

impl SpellHost<'_> {
    fn player_only(&self, node: &P, what: &str) -> sp::Result<()> {
        if self.live.inner.is_player_node(node.node()) {
            Ok(())
        } else {
            srefuse(&format!("{what} of an NPC"))
        }
    }
    fn attribute_value(&self, p: &P, key: &str) -> Option<AttributeValue> {
        self.live.inner.canonical(p).player.attributes().get(key)
    }
    /// `inventory.remove(items...)`: each `remove(item, false)`, then
    /// `update()`.
    pub fn inventory_remove_all(&mut self, p: &P, items: &[(i32, i32)]) -> sp::Result<bool> {
        let canonical = self.live.inner.canonical(p).player.clone();
        let mut inventory = canonical.inventory();
        let handles: Vec<ItemHandle> = items
            .iter()
            .map(|(id, amount)| Shared::new(Item::new(*id, *amount, 1000, &self.live.inner.items)))
            .collect();
        let inner: &mut Inner = &mut self.live.inner;
        let result = canonical.apply(
            &mut inner.state,
            &mut inner.frames,
            super::now_millis,
            |_, defs, ctx| {
                let mut all = true;
                for item in &handles {
                    if !inventory.remove(Some(item), None, false, defs, ctx)? {
                        all = false;
                    }
                }
                inventory.update(None, defs, ctx)?;
                Ok(all)
            },
        );
        inner.drain(p);
        result.map_err(serr)
    }
}

impl sp::Host for SpellHost<'_> {
    type Node = P;
    type Spell = SpellKey;

    fn kind(&self, node: &P) -> NodeKind {
        match self.live.ckind(node) {
            cs::Kind::Player => NodeKind::Player,
            cs::Kind::Npc => NodeKind::Npc,
            _ => NodeKind::Entity,
        }
    }
    fn same_node(&self, a: &P, b: &P) -> bool {
        a == b
    }
    fn same_spell(&self, a: &SpellKey, b: &SpellKey) -> bool {
        a == b
    }
    fn spell(&mut self, spell: &SpellKey) -> &mut Spell {
        // Keys are only made from registered entries.
        self.live
            .round
            .magic
            .registry
            .get_mut(spell)
            .expect("a registered spell")
    }
    fn registered(&self, book: SpellBook, button: i32) -> Option<SpellKey> {
        self.live
            .round
            .magic
            .registry
            .contains_key(&(book, button))
            .then_some((book, button))
    }
    fn item_name(&self, id: i32) -> String {
        self.live
            .inner
            .items
            .for_id(id)
            .read()
            .name
            .clone()
            .unwrap_or_else(|| "null".into())
    }
    fn experience_mod(&mut self) -> f64 {
        self.live.inner.combat.experience_mod
    }
    fn spell_book(&mut self, player: &P) -> &mut i32 {
        let held = self.book.as_ref().is_some_and(|(p, _)| p == player);
        if !held {
            if let Some((p, book)) = self.book.take() {
                let _ = self
                    .live
                    .inner
                    .with_profile(&p, |profile| profile.spell_book = book);
            }
            let book = self
                .live
                .inner
                .with_profile(player, |profile| profile.spell_book)
                .unwrap_or(0);
            self.book = Some((player.clone(), book));
        }
        &mut self.book.as_mut().expect("held").1
    }
    fn staff_interface(&mut self, player: &P) -> bool {
        // WeaponInterface.current == WeaponInterfaces.STAFF (id 90).
        self.live.inner.is_player_node(player.node())
            && self
                .live
                .inner
                .canonical(player)
                .weapon_tab
                .as_ref()
                .is_some_and(|w| w.current == Some(cs::WeaponInterfaces::Staff))
    }
    fn set_god_cast(&mut self, player: &P, index: usize, value: i32) {
        let _ = self.live.inner.with_profile(player, |profile| {
            if let Some(slot) = profile.saved.activity.god_casts.get_mut(index) {
                *slot = value;
            }
        });
    }

    fn ticks(&mut self) -> sp::Result<i32> {
        Ok(self.live.inner.ticks)
    }
    fn dev_mode(&mut self) -> sp::Result<bool> {
        Ok(self.live.inner.settings.dev_mode)
    }
    fn name(&mut self, node: &P) -> sp::Result<Option<String>> {
        cs::Host::name(self.live, node).map_err(from_cs)
    }
    fn location(&mut self, node: &P) -> sp::Result<Option<[i32; 3]>> {
        Ok(self.live.cloc(node).map(|l| l.coordinates()))
    }
    fn level(&mut self, entity: &P, slot: i32) -> sp::Result<i32> {
        cs::Host::level_with(self.live, entity, slot, false).map_err(from_cs)
    }
    fn level_with(&mut self, entity: &P, slot: i32, discard_assist: bool) -> sp::Result<i32> {
        cs::Host::level_with(self.live, entity, slot, discard_assist).map_err(from_cs)
    }
    fn add_experience(
        &mut self,
        entity: &P,
        slot: i32,
        experience: f64,
        player_mod: bool,
    ) -> sp::Result<()> {
        cp::Host::add_experience(self.live, entity, slot, experience, player_mod).map_err(from_cs)
    }
    fn add_experience_default(&mut self, entity: &P, slot: i32, experience: f64) -> sp::Result<()> {
        // Skills.addExperience(slot, experience): playerMod false.
        cp::Host::add_experience(self.live, entity, slot, experience, false).map_err(from_cs)
    }
    fn equipment(&mut self, player: &P, slot: i32) -> sp::Result<Option<i32>> {
        self.player_only(player, "equipment")?;
        Ok(self
            .live
            .equipment_slot(player, slot)
            .map(|i| i.read().id()))
    }
    fn equipment_new(&mut self, player: &P, slot: i32) -> sp::Result<i32> {
        self.player_only(player, "equipment")?;
        Ok(self
            .live
            .equipment_slot(player, slot)
            .map_or(0, |i| i.read().id()))
    }
    fn inventory_amount(&mut self, player: &P, id: i32) -> sp::Result<i32> {
        self.player_only(player, "inventory")?;
        let inventory = self.live.inner.canonical(player).player.inventory();
        Ok(inventory.amount_id(id, &self.live.inner.items))
    }
    fn inventory_remove(&mut self, player: &P, id: i32, amount: i32) -> sp::Result<bool> {
        self.player_only(player, "inventory")?;
        // inventory.remove(new Item(id, amount)): remove(item, true).
        let canonical = self.live.inner.canonical(player).player.clone();
        let mut inventory = canonical.inventory();
        let item: ItemHandle = Shared::new(Item::new(id, amount, 1000, &self.live.inner.items));
        let inner: &mut Inner = &mut self.live.inner;
        let result = canonical.apply(
            &mut inner.state,
            &mut inner.frames,
            super::now_millis,
            |_, defs, ctx| inventory.remove(Some(&item), None, true, defs, ctx),
        );
        inner.drain(player);
        result.map_err(serr)
    }
    fn send_message(&mut self, player: &P, text: &str) -> sp::Result<()> {
        if self.live.inner.is_player_node(player.node()) {
            self.live.inner.message(player, text).map_err(serr)?;
        }
        Ok(())
    }
    fn packet_message(&mut self, player: &P, text: &str) -> sp::Result<()> {
        self.player_only(player, "the packet dispatch")?;
        self.live.inner.message(player, text).map_err(serr)
    }
    fn interface_config(&mut self, _: &P, _: i32, _: i32, _: bool) -> sp::Result<()> {
        // Only the autocast selection reaches it; the live world selects
        // autocast spells through the weapon tab (super::tabs).
        srefuse("PacketDispatch.sendInterfaceConfig from the spell classes")
    }
    fn dispatch_cast(&mut self, _: &P, _: SpellBook, _: i32, _: Option<&P>) -> sp::Result<()> {
        self.live
            .inner
            .not_ported
            .skip("SpellCastEvent dispatch (entity event hooks not ported)");
        Ok(())
    }
    fn face_location(&mut self, entity: &P, location: Option<[i32; 3]>) -> sp::Result<()> {
        let location = location.map(|[x, y, z]| Location::new(x, y, z));
        if self.live.inner.is_npc_node(entity.node()) {
            self.live
                .npc_face_location(entity.node(), location)
                .map_err(serr)?;
            return Ok(());
        }
        self.live
            .inner
            .canonical(entity)
            .player
            .masks()
            .face_location(location)
            .map_err(serr)?;
        Ok(())
    }
    fn face_temporary(&mut self, entity: &P, target: Option<&P>, ticks: i32) -> sp::Result<()> {
        cp::Host::face_temporary(self.live, entity, target, ticks).map_err(from_cs)
    }
    fn face(&mut self, entity: &P, target: Option<&P>) -> sp::Result<()> {
        self.live.entity_face(entity, target).map_err(from_cs)
    }
    fn graphics(&mut self, entity: &P, graphics: Option<Graphics>) -> sp::Result<()> {
        // Entity.graphics(null): Animator.animate(null, null) does nothing.
        let Some(g) = graphics else {
            return Ok(());
        };
        self.live
            .entity_visualize(
                entity,
                None,
                Some(NativeGraphics::new(g.id, g.height, g.delay)),
            )
            .map_err(from_cs)
    }
    fn animate(&mut self, entity: &P, animation: Option<Animation>) -> sp::Result<()> {
        let Some(a) = animation else {
            return Ok(());
        };
        self.live
            .entity_visualize(entity, Some(native_animation(a)), None)
            .map_err(from_cs)
    }
    fn force_animation(&mut self, entity: &P, animation: Option<Animation>) -> sp::Result<()> {
        let animation = animation.map(native_animation);
        if self.live.inner.is_npc_node(entity.node()) {
            return self
                .live
                .npc_force_animation(entity.node(), animation)
                .map_err(serr);
        }
        let animator = self.live.inner.canonical(entity).animator.clone();
        animator
            .force_animation(&mut self.live.walk.animations, animation)
            .map_err(serr)
    }
    fn global_audio(
        &mut self,
        location: [i32; 3],
        id: i32,
        delay: i32,
        loops: i32,
        radius: i32,
    ) -> sp::Result<()> {
        let [x, y, z] = location;
        self.live
            .global_audio_full(Location::new(x, y, z), id, delay, loops, radius)
            .map_err(from_cs)
    }
    fn projectile_magic(
        &mut self,
        _: &P,
        _: Option<&P>,
        _: i32,
        _: i32,
        _: i32,
        _: i32,
        _: i32,
    ) -> sp::Result<Projectile> {
        // Only DefaultCombatSpell (an NPC's own spell) builds one.
        srefuse("Projectile.magic of DefaultCombatSpell (NPC autocast spells)")
    }
    fn send_projectile(
        &mut self,
        projectile: &Projectile,
        source: &P,
        target: ProjectileTarget<'_, P>,
        npc: bool,
        base_speed: i32,
        modified_speed: i32,
    ) -> sp::Result<()> {
        // projectile.transform(source, target, npc, baseSpeed, modifiedSpeed)
        // .send(): the template's numbers with the source's centre, the
        // speed over the distance to the target, the distance `size << 5`
        // for an NPC source (written into the shared template, as the
        // original mutates it).
        let (victim, end) = match target {
            ProjectileTarget::Entity(victim) => {
                let victim = victim.ok_or(sp::Error::Null)?;
                (Some(victim.node()), None)
            }
            ProjectileTarget::Location(location) => {
                let [x, y, z] = location.ok_or(sp::Error::Null)?;
                (None, Some(Location::new(x, y, z)))
            }
        };
        let mut sent =
            self.live
                .projectile_create(source, None, projectile.projectile_id, 0, 0, 0, 0, 0);
        let from = sent.source_location.clone().ok_or(sp::Error::Null)?;
        let to = match (&end, victim) {
            (Some(end), _) => end.clone(),
            (None, Some(node)) => self
                .live
                .inner
                .state
                .node_location(node)
                .map_err(serr)?
                .ok_or(sp::Error::Null)?,
            (None, None) => return Err(sp::Error::Null),
        };
        let speed = (f64::from(base_speed)
            + cs::distance(from.coordinates(), to.coordinates()) * f64::from(modified_speed))
            as i32;
        let distance = if npc {
            let size = sent.distance; // projectile_create's `size << 5`
            for spell in self.live.round.magic.registry.values_mut() {
                if let Some(template) = spell.projectile.as_mut() {
                    if template.label == projectile.label {
                        template.distance = size;
                    }
                }
            }
            size
        } else {
            projectile.distance
        };
        sent.victim = victim;
        sent.end_location = end;
        sent.start_height = projectile.start_height;
        sent.end_height = projectile.end_height;
        sent.start_delay = projectile.start_delay;
        sent.speed = speed;
        sent.angle = projectile.angle;
        sent.distance = distance;
        self.live.projectile_send(sent).map_err(from_cs)
    }
    fn attribute(&mut self, entity: &P, key: &str, default: Value) -> sp::Result<Value> {
        self.player_only(entity, "attributes")?;
        Ok(match self.attribute_value(entity, key) {
            None => default,
            Some(AttributeValue::Int(v)) => Value::Int(v),
            Some(AttributeValue::Object(o)) => match o.as_any().downcast_ref::<RuneList>() {
                Some(list) => Value::Items(list.0.clone()),
                None => return Err(sp::Error::Cast),
            },
            Some(_) => return Err(sp::Error::Cast),
        })
    }
    fn set_attribute(&mut self, entity: &P, key: &str, value: Value) -> sp::Result<()> {
        self.player_only(entity, "attributes")?;
        let value = match value {
            Value::Int(v) => AttributeValue::Int(v),
            Value::Items(items) => {
                AttributeValue::Object(Arc::new(RuneList(items)) as Arc<dyn AttributeObject>)
            }
        };
        self.live
            .inner
            .canonical(entity)
            .player
            .attributes()
            .set_value(key, Some(value));
        Ok(())
    }
    fn remove_attribute(&mut self, entity: &P, key: &str) -> sp::Result<()> {
        self.player_only(entity, "attributes")?;
        self.live
            .inner
            .canonical(entity)
            .player
            .attributes()
            .remove(key);
        Ok(())
    }
    fn autocast(&mut self, entity: &P) -> sp::Result<Option<SpellKey>> {
        self.live.autocast_key(entity).map_err(from_cs)
    }
    fn set_autocast(&mut self, entity: &P, spell: Option<&SpellKey>) -> sp::Result<()> {
        self.player_only(entity, "the autocast spell")?;
        let value = spell.map(|key| crate::player::save::Autocast {
            book: self.live.round.magic.registry[key].book.ordinal(),
            spell_id: key.1,
        });
        self.live
            .inner
            .with_profile(entity, |profile| profile.persisted.spells.autocast = value)
            .map_err(serr)
    }
    fn set_spell(&mut self, entity: &P, spell: &SpellKey) -> sp::Result<()> {
        self.player_only(entity, "properties.setSpell")?;
        let key = (spell.0.ordinal(), spell.1);
        self.live
            .inner
            .with_profile(entity, |profile| profile.properties.spell = Some(key))
            .map_err(serr)?;
        // setSpell(non-null): combatPulse.updateStyle().
        cp::pulse::update_style(self.live, entity).map_err(from_cs)
    }
    fn is_attacking(&mut self, entity: &P) -> sp::Result<bool> {
        cp::pulse::is_attacking(self.live, entity).map_err(from_cs)
    }
    fn set_victim(&mut self, entity: &P, victim: Option<&P>) -> sp::Result<()> {
        cp::pulse::set_victim(self.live, entity, victim).map_err(from_cs)
    }
    fn update_style(&mut self, entity: &P) -> sp::Result<()> {
        cp::pulse::update_style(self.live, entity).map_err(from_cs)
    }
    fn pulse_start(&mut self, entity: &P) -> sp::Result<()> {
        cp::pulse::start(self.live, entity).map_err(from_cs)
    }
    fn pulse_attack(&mut self, entity: &P, victim: Option<&P>) -> sp::Result<()> {
        cp::pulse::attack(self.live, entity, victim).map_err(from_cs)
    }
    fn attack_style(&mut self, entity: &P) -> sp::Result<Option<i32>> {
        Ok(cs::Host::attack_style(self.live, entity)
            .map_err(from_cs)?
            .map(|s| s.style))
    }
    fn attack_style_index(&mut self, player: &P) -> sp::Result<i32> {
        cp::Host::attack_style_index(self.live, player).map_err(from_cs)
    }
    fn magic_animation(&mut self, entity: &P) -> sp::Result<Option<Animation>> {
        // Properties.getMagicAnimation (combat animation index 1).
        let animation = cp::Host::combat_animation(self.live, entity, 1).map_err(from_cs)?;
        Ok(animation.as_ref().map(spell_animation))
    }
    fn attack_animation(&mut self, entity: &P) -> sp::Result<Option<Animation>> {
        let animation = cp::Host::attack_animation(self.live, entity).map_err(from_cs)?;
        Ok(animation.as_ref().map(spell_animation))
    }
    fn multi_zone(&mut self, entity: &P) -> sp::Result<bool> {
        if self.live.inner.is_player_node(entity.node()) {
            return self
                .live
                .inner
                .with_profile(entity, |p| p.properties.multi_zone)
                .map_err(serr);
        }
        Ok(self.live.zones.multi.contains(&entity.node()))
    }
    fn bonuses(&mut self, entity: &P) -> sp::Result<Option<Vec<i32>>> {
        cs::Host::bonuses(self.live, entity).map_err(from_cs)
    }
    fn in_zone(&mut self, entity: &P, name: &str) -> sp::Result<bool> {
        let monitor = self.live.monitor(&entity.0);
        monitor.is_in_zone(Some(name)).map_err(serr)
    }
    fn timer_active(&mut self, entity: &P, name: &str) -> sp::Result<bool> {
        self.live
            .has_timer_of(entity.node(), name)
            .map_err(|e| sp::Error::Binding(format!("{e:?}")))
    }
    fn god_casts(&mut self, player: &P) -> sp::Result<Option<Vec<i32>>> {
        self.player_only(player, "god casts")?;
        self.live
            .inner
            .with_profile(player, |profile| {
                Some(profile.saved.activity.god_casts.clone())
            })
            .map_err(serr)
    }
    fn npc_id(&mut self, npc: &P) -> sp::Result<i32> {
        self.live
            .npc_row(npc)
            .map(|row| row.id)
            .ok_or(sp::Error::Cast)
    }
    fn npc_task(&mut self, npc: &P) -> sp::Result<Option<bool>> {
        // NPC.getTask(): Tasks.forId is not ported, so no NPC has a task.
        cs::Host::npc_task(self.live, npc).map_err(from_cs)?;
        Ok(None)
    }
    fn combat_graphics(&mut self, npc: &P) -> sp::Result<Option<Vec<Option<Graphics>>>> {
        cp::Host::npc_combat_graphics(self.live, npc).map_err(from_cs)
    }
    fn multi_targets(&mut self, _: &P, _: &P) -> sp::Result<Vec<P>> {
        srefuse("findMultihitTargets (multi-target ancient spells)")
    }
    fn open_tab(&mut self, _: &P, _: i32) -> sp::Result<()> {
        srefuse("interfaceManager.openTab from the spell classes")
    }
    fn set_varbit(&mut self, _: &P, _: i32, _: i32, _: bool) -> sp::Result<()> {
        srefuse("setVarbit from the spell classes")
    }
    fn component_visibility(&mut self, _: &P, _: i32, _: i32, _: bool) -> sp::Result<()> {
        srefuse("setComponentVisibility from the spell classes")
    }
    fn set_attack_style(&mut self, _: &P, _: i32) -> sp::Result<bool> {
        srefuse("WeaponInterface.setAttackStyle from the spell classes")
    }
    fn math_random(&mut self) -> sp::Result<f64> {
        Ok(self.live.inner.generators.math_random())
    }
}

/// `MagicSpell.castSpell(player, book, spellId, target)` for a target that
/// is an entity, or `None` for another node (an item, a ground item): the
/// checks before the target is read, then a refusal if a registered combat
/// spell would be cast on it. A registered spell whose `fireEffect` is not
/// ported is refused before anything runs.
pub fn cast_spell(
    live: &mut Live,
    player: &P,
    book: SpellBook,
    spell_id: i32,
    target: Option<&P>,
) -> Result<bool, String> {
    // The checks before the spell runs (magic delay, registration, book),
    // so a spell that is not ported is refused only where the original
    // would cast it.
    let delay = live
        .inner
        .canonical(player)
        .player
        .attributes()
        .integer("magic-delay", 0)
        .map_err(|e| format!("{e:?}"))?;
    let player_book = live
        .inner
        .with_profile(player, |profile| profile.spell_book)
        .map_err(|e| format!("{e:?}"))?;
    if delay > live.inner.ticks || player_book != book.interface_id() {
        return Ok(false);
    }
    if let Some(refused) = refused_registration(book, spell_id) {
        return Err(format!("not ported in the live world: {refused}"));
    }
    if let Some(spell) = live.round.magic.registry.get(&(book, spell_id)) {
        if let Err(name) = Live::fire_effect_ported(spell.class) {
            return Err(format!(
                "not ported in the live world: {name}.fireEffect (a combat spell with an effect)"
            ));
        }
    }
    match target {
        Some(target) => live
            .with_spells(|h| sp::cast::cast_spell(h, player, book, spell_id, Some(target)))
            .map_err(|e| format!("{e:?}")),
        None => {
            if !live.round.magic.registry.contains_key(&(book, spell_id)) {
                return Ok(false);
            }
            Err(
                "not ported in the live world: a combat spell cast on a node that is not an entity"
                    .into(),
            )
        }
    }
}

/// The non-combat `MagicSpell` plugins the original registers in the spell
/// books (by plugin scan), by book and button: not ported here.
pub fn refused_registration(book: SpellBook, button: i32) -> Option<&'static str> {
    Some(match (book, button) {
        (SpellBook::Modern, 3) => "EnchantCrossbowSpell",
        (SpellBook::Modern, 5 | 16 | 28 | 36 | 51 | 61) => "EnchantSpell",
        (SpellBook::Modern, 19) => "TelekineticGrabSpell",
        (SpellBook::Modern, 54 | 59 | 62) => "TeleotherSpells",
        (SpellBook::Modern, 58) => "ChargeSpell",
        (SpellBook::Modern, 60) => "BountyLocateSpell",
        (SpellBook::Ancient, 20..=28) => "AncientTeleportPlugin (the ancient teleports)",
        (SpellBook::Lunar, 13) => "MagicImbueSpell",
        (SpellBook::Lunar, 14 | 19) => "VengeanceSpell",
        (SpellBook::Lunar, 26) => "StatBoostSpell",
        (SpellBook::Lunar, 27) => "StatRestoreSpell",
        (SpellBook::Lunar, 29 | 30) => "HealSpell",
        _ => return None,
    })
}

// ------------------------------------------------------------------ listeners

/// `SpellListener.NPC`, `OBJECT`, `ITEM`, `PLAYER`, `NONE`, `GROUND_ITEM`.
pub mod kind {
    pub const NPC: i32 = -1;
    pub const OBJECT: i32 = -2;
    pub const ITEM: i32 = -3;
    pub const PLAYER: i32 = -4;
    pub const NONE: i32 = -5;
    pub const GROUND_ITEM: i32 = -6;
}

/// A listener the original registers with `onCast`.
enum Listener {
    /// A standard-book teleport: level, runes, experience and destination.
    Teleport(ModernTeleport),
    /// Registered by the original, not ported here.
    Refused(&'static str),
}

#[derive(Clone, Copy)]
struct ModernTeleport {
    level: i32,
    runes: &'static [(i32, i32)],
    experience: f64,
    destination: [i32; 3],
    /// Varrock's alternative destination with `diaries:varrock:alttele`.
    alternate: Option<[i32; 3]>,
}

/// `Items` ids of the runes the teleports name.
const FIRE: i32 = 554;
const WATER: i32 = 555;
const AIR: i32 = 556;
const EARTH: i32 = 557;
const LAW: i32 = 563;

/// `SpellListeners.get(spellID, type, [id,] book)`: the listener the
/// original's maps hold for a key (the id-specific map first).
fn listener(book: &str, button: i32, kind: i32, id: i32) -> Option<Listener> {
    use Listener::{Refused, Teleport};
    const MTA_ALCH: [i32; 5] = [6893, 6894, 6895, 6896, 6897];
    const ORBS: [(i32, i32); 4] = [(35, 2151), (39, 29415), (46, 2153), (49, 2152)];
    match book {
        "modern" => {
            // MTAListeners' id-specific alchemy of the Alchemists' Playground items.
            if kind == kind::ITEM && (button == 13 || button == 34) && MTA_ALCH.contains(&id) {
                return Some(Refused("MTAListeners alchemy (Mage Training Arena)"));
            }
            if kind == kind::OBJECT {
                return ORBS
                    .iter()
                    .any(|(b, scenery)| *b == button && *scenery == id)
                    .then_some(Refused("ModernListeners.chargeOrb"));
            }
            Some(match (button, kind) {
                (0, kind::NONE) => {
                    Refused("ModernListeners home teleport (HomeTeleportHelper's pulse)")
                }
                (15, kind::NONE) => Teleport(ModernTeleport {
                    level: 25,
                    runes: &[(FIRE, 1), (AIR, 3), (LAW, 1)],
                    experience: 35.0,
                    destination: [3213, 3424, 0],
                    alternate: Some([3165, 3472, 0]),
                }),
                (18, kind::NONE) => Teleport(ModernTeleport {
                    level: 31,
                    runes: &[(EARTH, 1), (AIR, 3), (LAW, 1)],
                    experience: 41.0,
                    destination: [3221, 3219, 0],
                    alternate: None,
                }),
                (21, kind::NONE) => Teleport(ModernTeleport {
                    level: 37,
                    runes: &[(WATER, 1), (AIR, 3), (LAW, 1)],
                    experience: 47.0,
                    destination: [2965, 3378, 0],
                    alternate: None,
                }),
                (26, kind::NONE) => Refused(
                    "ModernListeners Camelot teleport (AchievementDiaryManager.finishTask: diaries not installed)",
                ),
                (32, kind::NONE) => Refused(
                    "ModernListeners Ardougne teleport (Plague City's teleport attribute and sendDialogue)",
                ),
                (37, kind::NONE) => Refused(
                    "ModernListeners Watchtower teleport (quest requirements not installed)",
                ),
                (44, kind::NONE) => {
                    Refused("ModernListeners Trollheim teleport (quest requirements not installed)")
                }
                (47, kind::NONE) => {
                    Refused("ModernListeners Ape Atoll teleport (quest requirements not installed)")
                }
                (23, kind::NONE) => Refused("ModernListeners house teleport (player-owned houses)"),
                (13, kind::ITEM) | (34, kind::ITEM) => Refused("ModernListeners alchemy"),
                (25, kind::ITEM) => Refused("ModernListeners superheat"),
                (9, kind::NONE) | (40, kind::NONE) => {
                    Refused("ModernListeners bones to bananas/peaches")
                }
                _ => return None,
            })
        }
        "lunar" => {
            const NONE: [i32; 28] = [
                16, 15, 4, 7, 20, 33, 31, 23, 8, 24, 34, 3, 0, 35, 25, 18, 36, 10, 22, 17, 37, 21,
                38, 28, 39, 12, 2, 32,
            ];
            let registered = match kind {
                kind::NONE => NONE.contains(&button) && button != 2 && button != 32,
                kind::OBJECT => button == 32 || button == 2,
                kind::NPC => button == 6,
                kind::PLAYER => matches!(button, 1 | 9 | 5),
                kind::ITEM => button == 11,
                _ => false,
            };
            registered.then_some(Refused("LunarListeners (the lunar book is not ported)"))
        }
        _ => None,
    }
}

/// `SpellListeners.run(button, type, book, player, node)`.
pub fn spell_listeners(
    live: &mut Live,
    button: i32,
    kind: i32,
    book: &str,
    player: &P,
    node_id: i32,
) -> Result<(), String> {
    let Some(found) = listener(book, button, kind, node_id) else {
        return Ok(());
    };
    let result = run_listener(live, found, player);
    live.drain_all();
    result
}

fn run_listener(live: &mut Live, found: Listener, player: &P) -> Result<(), String> {
    // player.scripts.removeWeakScripts(): only content queues weak scripts.
    live.inner.not_ported.skip(
        "Entity.scripts.removeWeakScripts in SpellListeners.run (only content queues weak scripts)",
    );
    match found {
        Listener::Refused(what) => Err(format!("not ported in the live world: {what}")),
        Listener::Teleport(teleport) => {
            // The NONE branch: the method, then the SpellCastEvent; an
            // IllegalStateException from `requires` removes "spell:runes".
            match modern_teleport(live, player, teleport)? {
                Cast::Done => {
                    live.inner
                        .not_ported
                        .skip("SpellCastEvent dispatch (entity event hooks not ported)");
                    Ok(())
                }
                Cast::IllegalState => {
                    live.inner
                        .canonical(player)
                        .player
                        .attributes()
                        .remove(sp::runes::SPELL_RUNES);
                    Ok(())
                }
            }
        }
    }
}

enum Cast {
    Done,
    /// `requires` threw `IllegalStateException`.
    IllegalState,
}

/// One standard-book teleport listener: `requires`, then `sendTeleport`.
fn modern_teleport(live: &mut Live, player: &P, t: ModernTeleport) -> Result<Cast, String> {
    if !requires(live, player, t.level, t.runes)? {
        return Ok(Cast::IllegalState);
    }
    let destination = match t.alternate {
        Some(alternate) => {
            let alt = live
                .inner
                .canonical(player)
                .player
                .attributes()
                .boolean("diaries:varrock:alttele", false)
                .map_err(|e| format!("{e:?}"))?;
            if alt { alternate } else { t.destination }
        }
        None => t.destination,
    };
    send_teleport(live, player, t.experience, destination)?;
    Ok(Cast::Done)
}

/// `SpellListener.requires(player, magicLevel, runes)`: false where the
/// original throws `IllegalStateException`.
fn requires(live: &mut Live, player: &P, level: i32, runes: &[(i32, i32)]) -> Result<bool, String> {
    let attributes = live.inner.canonical(player).player.attributes();
    let delay = attributes
        .integer("magic-delay", 0)
        .map_err(|e| format!("{e:?}"))?;
    if delay > live.inner.ticks {
        return Ok(false);
    }
    if attributes
        .boolean("tablet-spell", false)
        .map_err(|e| format!("{e:?}"))?
    {
        return Ok(true);
    }
    let magic = cs::Host::level_with(live, player, sp::skill::MAGIC, false)
        .map_err(|e| format!("{e:?}"))?;
    if magic < level {
        live.inner
            .message(
                player,
                &format!("You need a magic level of {level} to cast this spell."),
            )
            .map_err(|e| format!("{e:?}"))?;
        return Ok(false);
    }
    let missing = live
        .with_spells(|h| sp::runes::has_runes(h, player, runes))
        .map_err(|e| format!("{e:?}"))?;
    if let Some((id, _)) = missing {
        let name = live
            .inner
            .items
            .for_id(id)
            .read()
            .name
            .clone()
            .unwrap_or_else(|| "null".into());
        live.inner
            .message(
                player,
                &format!(
                    "You don't have enough {}s to cast this spell.",
                    lowercase_root(&name)
                ),
            )
            .map_err(|e| format!("{e:?}"))?;
        return Ok(false);
    }
    // No special equipment for these listeners.
    Ok(true)
}

/// `ModernListeners.sendTeleport(player, xp, location)`.
fn send_teleport(live: &mut Live, player: &P, experience: f64, at: [i32; 3]) -> Result<(), String> {
    let blocked = zones::is_tele_blocked(live, &player.0).map_err(|e| format!("{e:?}"))?;
    if blocked {
        live.inner
            .canonical(player)
            .player
            .attributes()
            .remove(sp::runes::SPELL_RUNES);
        live.inner
            .message(player, "A magical force prevents you from teleporting.")
            .map_err(|e| format!("{e:?}"))?;
        return Ok(());
    }
    let [x, y, z] = at;
    let location = Location::new(x, y, z);
    let sent = live
        .with_teleports(|h| {
            let manager = tp::Host::teleporter(h, player)?;
            manager.send_type(h, Some(&location), Some(TeleportType::Normal))
        })
        .map_err(|e| format!("{e:?}"))?;
    if sent {
        live.inner
            .not_ported
            .skip("TeleportEvent dispatch (entity event hooks not ported)");
        // removeRunes(player): inventory.remove(*spell:runes), then the two
        // attributes go.
        let attributes = live.inner.canonical(player).player.attributes();
        let runes = match attributes.get(sp::runes::SPELL_RUNES) {
            None => Vec::new(),
            Some(AttributeValue::Object(o)) => match o.as_any().downcast_ref::<RuneList>() {
                Some(list) => list.0.clone(),
                None => return Err("ClassCastException: spell:runes".into()),
            },
            Some(_) => return Err("ClassCastException: spell:runes".into()),
        };
        live.with_spells(|h| h.inventory_remove_all(player, &runes))
            .map_err(|e| format!("{e:?}"))?;
        attributes.remove(sp::runes::SPELL_RUNES);
        let tablet = attributes
            .boolean("tablet-spell", false)
            .map_err(|e| format!("{e:?}"))?;
        attributes.remove("tablet-spell");
        // addXP(player, xp): nothing for a tablet; the attribute was just
        // removed, so the read answers false.
        let _ = tablet;
        cp::Host::add_experience(live, player, sp::skill::MAGIC, experience, false)
            .map_err(|e| format!("{e:?}"))?;
        // setDelay(player, true): magic-delay = ticks + 5.
        let ticks = live.inner.ticks;
        attributes.set_integer("magic-delay", ticks.wrapping_add(5));
    }
    Ok(())
}

// ------------------------------------------------------------------ teleports

/// `world::teleport::Host` over the live world.
pub struct TeleHost<'a> {
    pub live: &'a mut Live,
}

impl TeleHost<'_> {
    fn player(&self, entity: &P, what: &str) -> tp::Result<()> {
        if self.live.inner.is_player_node(entity.node()) {
            Ok(())
        } else {
            trefuse(&format!("{what} of an NPC"))
        }
    }
}

impl tp::Host for TeleHost<'_> {
    type Entity = P;

    fn is_player(&mut self, entity: &P) -> tp::Result<bool> {
        Ok(self.live.inner.is_player_node(entity.node()))
    }
    fn teleporter(&mut self, entity: &P) -> tp::Result<Manager<P>> {
        Ok(self
            .live
            .round
            .magic
            .teleporters
            .entry(entity.node())
            .or_insert_with(|| Manager::new(entity.clone()))
            .clone())
    }
    fn spell_book(&mut self, player: &P) -> tp::Result<i32> {
        self.live
            .inner
            .with_profile(player, |profile| profile.spell_book)
            .map_err(terr)
    }
    fn has_timer(&mut self, entity: &P, name: &str) -> tp::Result<bool> {
        self.live.has_timer_of(entity.node(), name).map_err(terr)
    }
    fn zone_teleport(&mut self, entity: &P, teleport_type: i32) -> tp::Result<bool> {
        let monitor = self.live.monitor(&entity.0);
        monitor
            .teleport(self.live, teleport_type, None)
            .map_err(terr)
    }
    fn is_tele_blocked(&mut self, entity: &P) -> tp::Result<bool> {
        zones::is_tele_blocked(self.live, &entity.0).map_err(terr)
    }
    fn send_message(&mut self, player: &P, text: &str) -> tp::Result<()> {
        self.live.inner.message(player, text).map_err(terr)
    }
    fn world_ticks(&mut self) -> tp::Result<i32> {
        Ok(self.live.inner.ticks)
    }
    fn set_script_delay(&mut self, player: &P, value: i32) -> tp::Result<()> {
        self.player(player, "the script processor")?;
        self.live
            .inner
            .canonical(player)
            .player
            .scripts()
            .set_delay(value);
        Ok(())
    }
    fn close_dialogue(&mut self, player: &P) -> tp::Result<()> {
        self.live.close_dialogue_of(player).map_err(terr)?;
        Ok(())
    }
    fn tablet_spell(&mut self, entity: &P) -> tp::Result<bool> {
        if !self.live.inner.is_player_node(entity.node()) {
            // An NPC's attributes hold no "tablet-spell".
            return Ok(false);
        }
        self.live
            .inner
            .canonical(entity)
            .player
            .attributes()
            .boolean("tablet-spell", false)
            .map_err(|_| tp::Error::ClassCast)
    }
    fn has_attribute(&mut self, entity: &P, key: &str) -> tp::Result<bool> {
        if !self.live.inner.is_player_node(entity.node()) {
            return Ok(false);
        }
        Ok(self
            .live
            .inner
            .canonical(entity)
            .player
            .attributes()
            .contains_key(key))
    }
    fn set_tele_pulse(&mut self, entity: &P, pulse: &TeleportPulse<P>) -> tp::Result<()> {
        self.player(entity, "attributes")?;
        self.live
            .inner
            .canonical(entity)
            .player
            .attributes()
            .set_value(
                "tele-pulse",
                Some(AttributeValue::Object(Arc::new(TelePulse(pulse.clone())))),
            );
        self.live
            .inner
            .not_ported
            .skip("AttributeSetEvent dispatch (entity event hooks not ported)");
        Ok(())
    }
    fn remove_attribute(&mut self, entity: &P, key: &str) -> tp::Result<()> {
        if !self.live.inner.is_player_node(entity.node()) {
            return Ok(());
        }
        self.live
            .inner
            .canonical(entity)
            .player
            .attributes()
            .remove(key);
        self.live
            .inner
            .not_ported
            .skip("AttributeRemoveEvent dispatch (entity event hooks not ported)");
        Ok(())
    }
    fn reset_walking(&mut self, entity: &P) -> tp::Result<()> {
        self.live.walk_reset(entity).map_err(terr)
    }
    fn clear_pulses(&mut self, entity: &P) -> tp::Result<()> {
        cp::Host::pulse_manager_clear(self.live, entity).map_err(terr)
    }
    fn lock(&mut self, entity: &P, ticks: i32) -> tp::Result<()> {
        let state = &mut self.live.inner.state;
        let locks = state.locks(&entity.0).map_err(terr)?;
        locks
            .lock(ticks, state, &mut action_locks::Clock)
            .map_err(terr)
    }
    fn lock_forever(&mut self, entity: &P) -> tp::Result<()> {
        let state = &mut self.live.inner.state;
        let locks = state.locks(&entity.0).map_err(terr)?;
        locks
            .lock_forever(state, &mut action_locks::Clock)
            .map_err(terr)
    }
    fn unlock(&mut self, entity: &P) -> tp::Result<()> {
        self.live
            .inner
            .state
            .locks(&entity.0)
            .map_err(terr)?
            .unlock();
        Ok(())
    }
    fn set_disabled_ticks(&mut self, entity: &P, ticks: i32) -> tp::Result<()> {
        cp::impact::set_disabled_ticks(self.live, entity, ticks).map_err(terr)
    }
    fn submit(&mut self, pulse: &TeleportPulse<P>) -> tp::Result<()> {
        // Pulses stopped outside the runner (a newer teleport stops the
        // current one) are no longer tracked.
        self.live
            .round
            .magic
            .teleports
            .retain(|t| pulses::is_running_base(t.handle()));
        self.live
            .driver
            .pulser()
            .submit(Some(pulse.handle().clone()))
            .map_err(terr)?;
        self.live.round.magic.teleports.push(pulse.clone());
        Ok(())
    }
    fn close_interfaces(&mut self, player: &P) -> tp::Result<()> {
        crate::world::player_death::Host::close_interface(self.live, &player.0).map_err(terr)
    }
    fn location(&mut self, entity: &P) -> tp::Result<Option<Location>> {
        Ok(self.live.cloc(entity))
    }
    fn teleport_location(&mut self, entity: &P) -> tp::Result<Option<Location>> {
        let actor = self.live.inner.state.actor(entity.node()).map_err(terr)?;
        let location = actor.queue.state.read().entity.teleport_location.clone();
        Ok(location)
    }
    fn set_teleport_location(&mut self, entity: &P, location: Option<Location>) -> tp::Result<()> {
        self.live
            .inner
            .state
            .actor(entity.node())
            .map_err(terr)?
            .queue
            .state
            .write()
            .entity
            .teleport_location = location;
        Ok(())
    }
    fn play_global_audio(
        &mut self,
        location: &Location,
        id: i32,
        delay: i32,
        loops: i32,
        radius: i32,
    ) -> tp::Result<()> {
        self.live
            .global_audio_full(location.clone(), id, delay, loops, radius)
            .map_err(terr)
    }
    fn play_audio(
        &mut self,
        player: &P,
        id: i32,
        delay: i32,
        loops: i32,
        location: Option<&Location>,
        _: i32,
    ) -> tp::Result<()> {
        if location.is_some() {
            return trefuse("playAudio with a location");
        }
        self.player(player, "playAudio")?;
        self.live.inner.queue_native(
            player,
            rs09_protocol::game::updates::synth_sound(id, loops, delay),
        );
        Ok(())
    }
    fn force_animation(&mut self, entity: &P, animation: NativeAnimation) -> tp::Result<()> {
        if self.live.inner.is_npc_node(entity.node()) {
            return self
                .live
                .npc_force_animation(entity.node(), Some(animation))
                .map_err(terr);
        }
        let animator = self.live.inner.canonical(entity).animator.clone();
        animator
            .force_animation(&mut self.live.walk.animations, Some(animation))
            .map_err(terr)
    }
    fn animate(&mut self, entity: &P, animation: NativeAnimation) -> tp::Result<bool> {
        if self.live.inner.is_npc_node(entity.node()) {
            return self
                .live
                .npc_animate(entity.node(), Some(animation), None)
                .map_err(terr);
        }
        let animator = self.live.inner.canonical(entity).animator.clone();
        animator
            .animate(&mut self.live.walk.animations, Some(animation), None)
            .map_err(terr)
    }
    fn graphics(&mut self, entity: &P, graphics: NativeGraphics) -> tp::Result<bool> {
        if self.live.inner.is_npc_node(entity.node()) {
            return self
                .live
                .npc_animate(entity.node(), None, Some(graphics))
                .map_err(terr);
        }
        let animator = self.live.inner.canonical(entity).animator.clone();
        animator
            .animate(&mut self.live.walk.animations, None, Some(graphics))
            .map_err(terr)
    }
    fn animation_reset(&mut self) -> NativeAnimation {
        self.live.walk.animations.factory.resets().animation_reset()
    }
}
