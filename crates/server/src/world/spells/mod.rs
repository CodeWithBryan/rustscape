//! The magic spell framework: rune requirements with combination runes and
//! elemental staves, the cast pipeline, spell books and their registered
//! combat spells, spell blocks and autocast selection.
//!
//! Ports the pinned `MagicSpell`, `CombatSpell`, `DefaultCombatSpell`,
//! `SpellType`, `Runes`, `CombinationRune`, `MagicStaff`, `SpellBlocks`,
//! `SpellBookManager`, `SpellUtils`, the autocast members of
//! `WeaponInterface` (`getAutospellId`, `selectAutoSpell`, `canAutocast`) and
//! the data and overridden members of the seventeen combat spell classes of
//! the standard and ancient books.
//!
//! This is a library. A spell is an object the host owns ([`Spell`]) and names
//! by a handle; the library reads it through [`Host::spell`] and never holds
//! that borrow across a service call. Everything the original reaches outside
//! these classes is a required [`Host`] service, called in the original order
//! with the original arguments. None has a default. Each spell's `fireEffect`
//! (freeze, drain, heal, poison, bind, teleblock, miasmic) is content and not
//! ported. Nothing installs this library in a session. See docs/spells.md.
pub mod autocast;
pub mod book;
pub mod cast;
pub mod combat;
pub mod runes;
pub mod table;

pub use book::{Registry, SpellBlocks, SpellBook, SpellBookManager};
pub use cast::{
    add_experience, cast, cast_spell, check_level_requirement, default_combat_spell, delay,
    experience, get_targets, maximum_impact, meets_requirements, visualize, visualize_impact,
};
pub use runes::{CombinationRune, MagicStaff, Rune, has_runes, is_infinite, using_staff};

/// A failure of the original, or of a service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// A null dereference in the original.
    Null,
    /// An array or list index outside its bounds in the original.
    Index,
    /// A failing cast (`ClassCastException`) in the original.
    Cast,
    /// A required service failed; preceding effects are kept.
    Binding(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<Error> for crate::world::combat_swing::Error {
    /// The combat slices model no `ClassCastException`; it travels as a
    /// binding failure naming the original exception.
    fn from(error: Error) -> Self {
        match error {
            Error::Null => Self::Null,
            Error::Index => Self::Index,
            Error::Cast => Self::Binding("java.lang.ClassCastException".into()),
            Error::Binding(text) => Self::Binding(text),
        }
    }
}

/// `Skills` slot numbers read here.
pub mod skill {
    pub const DEFENCE: i32 = 1;
    pub const HITPOINTS: i32 = 3;
    pub const MAGIC: i32 = 6;
    pub const SLAYER: i32 = 18;
}

/// `EquipmentContainer` slot numbers read here.
pub mod slot {
    pub const CAPE: i32 = 1;
    pub const WEAPON: i32 = 3;
    pub const HANDS: i32 = 9;
}

/// `WeaponInterface.STYLE_DEFENSIVE_CAST`.
pub const STYLE_DEFENSIVE_CAST: i32 = 7;
/// `Sounds.SPELLFAIL_227`.
pub const SPELL_FAIL_AUDIO: i32 = 227;
/// `CombatSpell.SPLASH_GRAPHIC`.
pub const SPLASH_GRAPHIC: Graphics = Graphics {
    id: 85,
    height: 96,
    delay: 0,
};
/// `CombatSpell.AUTOCAST_ANIMATION`.
pub const AUTOCAST_ANIMATION: Animation = Animation {
    id: 1162,
    delay: 0,
    priority: Priority::High,
};
/// `Audio.defaultAudioRadius`.
pub const DEFAULT_AUDIO_RADIUS: i32 = 8;

/// What a node is, as the `instanceof` tests here distinguish it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Player,
    Npc,
    /// An entity that is neither a player nor an NPC.
    Entity,
    /// A node that is not an entity.
    Node,
}

impl NodeKind {
    pub fn is_entity(self) -> bool {
        self != Self::Node
    }
}

/// `Animator.Priority`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Priority {
    Low,
    Mid,
    High,
    VeryHigh,
}

impl Priority {
    pub fn name(self) -> &'static str {
        match self {
            Self::Low => "LOW",
            Self::Mid => "MID",
            Self::High => "HIGH",
            Self::VeryHigh => "VERY_HIGH",
        }
    }
}

/// An `Animation` as these classes build it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Animation {
    pub id: i32,
    pub delay: i32,
    pub priority: Priority,
}

impl Animation {
    /// `new Animation(id)`.
    pub const fn new(id: i32) -> Self {
        Self {
            id,
            delay: 0,
            priority: Priority::Mid,
        }
    }

    /// `new Animation(id, Priority.HIGH)`.
    pub const fn high(id: i32) -> Self {
        Self {
            id,
            delay: 0,
            priority: Priority::High,
        }
    }
}

pub use crate::world::combat_swing::Graphics;

/// `Graphics(id, height, delay)`.
pub const fn graphics(id: i32, height: i32, delay: i32) -> Graphics {
    Graphics { id, height, delay }
}

/// An `Audio` record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Audio {
    pub id: i32,
    pub delay: i32,
    pub loops: i32,
    pub radius: i32,
}

/// A `Projectile` template a spell holds. `label` names the field (or the
/// constructor) that holds it in the original, because the original's
/// templates are shared mutable objects: `transform` rewrites them in place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Projectile {
    pub label: String,
    pub projectile_id: i32,
    pub start_height: i32,
    pub end_height: i32,
    pub start_delay: i32,
    pub speed: i32,
    pub angle: i32,
    pub distance: i32,
}

/// Where `projectile.transform(...)` aims.
pub enum ProjectileTarget<'a, N> {
    /// `transform(source, victim, ...)`; the victim may be null.
    Entity(Option<&'a N>),
    /// `transform(source, location, ...)`.
    Location(Option<[i32; 3]>),
}

/// `SpellType`, in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpellType {
    Strike,
    Bolt,
    CrumbleUndead,
    Blast,
    Wave,
    Rush,
    Burst,
    Blitz,
    Barrage,
    Confuse,
    Weaken,
    Curse,
    Vulnerability,
    Enfeeble,
    Stun,
    GodStrike,
    Bind,
    Snare,
    Entangle,
    MagicDart,
    IbansBlast,
    TridentOfSeas,
    Teleblock,
    Null,
}

impl SpellType {
    pub const ALL: [Self; 24] = [
        Self::Strike,
        Self::Bolt,
        Self::CrumbleUndead,
        Self::Blast,
        Self::Wave,
        Self::Rush,
        Self::Burst,
        Self::Blitz,
        Self::Barrage,
        Self::Confuse,
        Self::Weaken,
        Self::Curse,
        Self::Vulnerability,
        Self::Enfeeble,
        Self::Stun,
        Self::GodStrike,
        Self::Bind,
        Self::Snare,
        Self::Entangle,
        Self::MagicDart,
        Self::IbansBlast,
        Self::TridentOfSeas,
        Self::Teleblock,
        Self::Null,
    ];

    pub fn ordinal(self) -> i32 {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0) as i32
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Strike => "STRIKE",
            Self::Bolt => "BOLT",
            Self::CrumbleUndead => "CRUMBLE_UNDEAD",
            Self::Blast => "BLAST",
            Self::Wave => "WAVE",
            Self::Rush => "RUSH",
            Self::Burst => "BURST",
            Self::Blitz => "BLITZ",
            Self::Barrage => "BARRAGE",
            Self::Confuse => "CONFUSE",
            Self::Weaken => "WEAKEN",
            Self::Curse => "CURSE",
            Self::Vulnerability => "VULNERABILITY",
            Self::Enfeeble => "ENFEEBLE",
            Self::Stun => "STUN",
            Self::GodStrike => "GOD_STRIKE",
            Self::Bind => "BIND",
            Self::Snare => "SNARE",
            Self::Entangle => "ENTANGLE",
            Self::MagicDart => "MAGIC_DART",
            Self::IbansBlast => "IBANS_BLAST",
            Self::TridentOfSeas => "TRIDENT_OF_SEAS",
            Self::Teleblock => "TELEBLOCK",
            Self::Null => "NULL",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|t| t.name() == name)
    }

    /// `getAccuracyMod()`: the constructor argument.
    pub fn accuracy_mod(self) -> f64 {
        match self {
            Self::Strike => 1.0,
            Self::Bolt | Self::Rush | Self::Bind => 1.1,
            Self::CrumbleUndead | Self::Blast | Self::Burst | Self::GodStrike | Self::Snare => 1.2,
            Self::Wave | Self::Blitz | Self::Entangle | Self::Teleblock => 1.3,
            Self::Barrage | Self::IbansBlast | Self::TridentOfSeas => 1.4,
            Self::Confuse | Self::Weaken | Self::Curse | Self::MagicDart => 1.15,
            Self::Vulnerability | Self::Enfeeble | Self::Stun => 1.25,
            Self::Null => 0.0,
        }
    }

    /// `getImpactAmount(e, victim, base)`.
    pub fn impact_amount<H: Host>(
        self,
        h: &mut H,
        e: Option<&H::Node>,
        victim: Option<&H::Node>,
        base: i32,
    ) -> Result<i32> {
        let player = |h: &H, n: Option<&H::Node>| n.is_some_and(|n| h.kind(n) == NodeKind::Player);
        Ok(match self {
            Self::Strike => {
                if let Some(v) = victim.filter(|v| h.kind(v) == NodeKind::Npc)
                    && h.npc_id(v)? == 205
                {
                    return Ok(8i32.wrapping_add(base));
                }
                2i32.wrapping_mul(base)
            }
            Self::Bolt => {
                if player(h, e) && h.equipment_new(e.ok_or(Error::Null)?, slot::HANDS)? == 777 {
                    return Ok(11i32.wrapping_add(base));
                }
                8i32.wrapping_add(base)
            }
            Self::CrumbleUndead => 15,
            Self::Blast => 12i32.wrapping_add(base),
            Self::Wave => 16i32.wrapping_add(base),
            Self::Rush => 14i32.wrapping_add(base),
            Self::Burst => 18i32.wrapping_add(base),
            Self::Blitz => 22i32.wrapping_add(base),
            Self::Barrage => 26i32.wrapping_add(base),
            Self::GodStrike => {
                if !player(h, e) {
                    return Ok(20);
                }
                let e = e.ok_or(Error::Null)?;
                if h.timer_active(e, "magic:spellcharge")? {
                    let cape = h.equipment_new(e, slot::CAPE)?;
                    if cape == 2412 || cape == 2413 || cape == 2414 {
                        return Ok(30);
                    }
                }
                20
            }
            Self::MagicDart => {
                let e = e.ok_or(Error::Null)?;
                10i32.wrapping_add(h.level(e, skill::MAGIC)? / 10)
            }
            Self::IbansBlast => 25,
            Self::TridentOfSeas => {
                let e = e.ok_or(Error::Null)?;
                if h.kind(e) != NodeKind::Player {
                    return Err(Error::Cast);
                }
                let level = h.level(e, skill::MAGIC)?;
                (level.wrapping_sub(75) / 3).wrapping_add(base)
            }
            _ => 2,
        })
    }
}

/// The concrete class of a spell object: which overridden members it has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpellClass {
    Air,
    Water,
    Earth,
    Fire,
    Bind,
    Curse,
    God,
    CrumbleUndead,
    IbanBlast,
    MagicDart,
    Teleblock,
    SonicWave,
    Ice,
    Blood,
    Shadow,
    Smoke,
    Miasmic,
    /// `DefaultCombatSpell`, built for one NPC.
    Default,
}

impl SpellClass {
    /// The class's simple name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Air => "AirSpell",
            Self::Water => "WaterSpell",
            Self::Earth => "EarthSpell",
            Self::Fire => "FireSpell",
            Self::Bind => "BindSpell",
            Self::Curse => "CurseSpells",
            Self::God => "GodSpells",
            Self::CrumbleUndead => "CrumbleUndead",
            Self::IbanBlast => "IbanBlast",
            Self::MagicDart => "MagicDart",
            Self::Teleblock => "TeleblockSpell",
            Self::SonicWave => "SonicWaveSpell",
            Self::Ice => "IceSpells",
            Self::Blood => "BloodSpells",
            Self::Shadow => "ShadowSpells",
            Self::Smoke => "SmokeSpells",
            Self::Miasmic => "MiasmicSpells",
            Self::Default => "DefaultCombatSpell",
        }
    }
}

/// One combat spell object: the `MagicSpell` and `CombatSpell` fields and
/// the class whose members it runs. `spell_id` and `projectile` are the two
/// fields the original mutates after construction (`register` and
/// `DefaultCombatSpell.visualize`).
#[derive(Clone, Debug, PartialEq)]
pub struct Spell {
    pub class: SpellClass,
    pub book: SpellBook,
    pub level: i32,
    pub experience: f64,
    pub animation: Option<Animation>,
    pub graphic: Option<Graphics>,
    pub audio: Audio,
    /// `runes`; `None` is a null array. Elements are `(id, amount)`.
    pub runes: Option<Vec<(i32, i32)>>,
    pub spell_id: i32,
    pub kind: SpellType,
    pub projectile: Option<Projectile>,
    pub end_graphic: Option<Graphics>,
    pub impact_audio: i32,
    /// `DefaultCombatSpell`'s `projectileId` and `startHeight`.
    pub default_projectile: Option<(i32, i32)>,
}

/// An attribute value as these classes store and read them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Int(i32),
    /// An `ArrayList<Item>` of `(id, amount)`.
    Items(Vec<(i32, i32)>),
}

/// The `BattleState` fields the spell members read: the estimated hit and the
/// frozen flag.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StateView {
    pub estimated_hit: i32,
    pub frozen: bool,
}

/// Everything the ported classes reach outside themselves. Each fallible
/// method is one original call; a failure leaves earlier effects in place.
/// The first group are plain accessors to state and class identity: they are
/// not service calls.
pub trait Host: Sized {
    /// A node: an entity or another node.
    type Node: Clone;
    /// One spell object.
    type Spell: Clone;

    fn kind(&self, node: &Self::Node) -> NodeKind;
    /// Reference identity of two nodes.
    fn same_node(&self, a: &Self::Node, b: &Self::Node) -> bool;
    /// Reference identity of two spell objects.
    fn same_spell(&self, a: &Self::Spell, b: &Self::Spell) -> bool;
    /// The spell object's fields.
    fn spell(&mut self, spell: &Self::Spell) -> &mut Spell;
    /// `book.getSpell(button)`: the world's registry.
    fn registered(&self, book: SpellBook, button: i32) -> Option<Self::Spell>;
    /// `ItemDefinition.forId(id).getName()`.
    fn item_name(&self, id: i32) -> String;
    /// The static `CombatSwingHandler.EXPERIENCE_MOD`.
    fn experience_mod(&mut self) -> f64;
    /// `SpellBookManager.spellBook`: the player's own field.
    fn spell_book(&mut self, player: &Self::Node) -> &mut i32;
    /// `WeaponInterface.current == WeaponInterfaces.STAFF`.
    fn staff_interface(&mut self, player: &Self::Node) -> bool;
    /// `getGodCasts()[index] = value`: a write into the array the service
    /// returned, not a call.
    fn set_god_cast(&mut self, player: &Self::Node, index: usize, value: i32);

    /// `GameWorld.getTicks()`.
    fn ticks(&mut self) -> Result<i32>;
    /// `GameWorld.getSettings().isDevMode()`.
    fn dev_mode(&mut self) -> Result<bool>;
    /// `Node.getName()`; `None` is a null name.
    fn name(&mut self, node: &Self::Node) -> Result<Option<String>>;
    /// `Node.getLocation()`.
    fn location(&mut self, node: &Self::Node) -> Result<Option<[i32; 3]>>;
    /// `skills.getLevel(slot)`.
    fn level(&mut self, entity: &Self::Node, slot: i32) -> Result<i32>;
    /// `skills.getLevel(slot, discardAssist)`.
    fn level_with(&mut self, entity: &Self::Node, slot: i32, discard_assist: bool) -> Result<i32>;
    /// `skills.addExperience(slot, experience, playerMod)`.
    fn add_experience(
        &mut self,
        entity: &Self::Node,
        slot: i32,
        experience: f64,
        player_mod: bool,
    ) -> Result<()>;
    /// `skills.addExperience(slot, experience)`.
    fn add_experience_default(
        &mut self,
        entity: &Self::Node,
        slot: i32,
        experience: f64,
    ) -> Result<()>;
    /// `equipment.get(slot)`: the worn item's id, or `None` for null.
    fn equipment(&mut self, player: &Self::Node, slot: i32) -> Result<Option<i32>>;
    /// `equipment.getNew(slot)`: the worn item's id, or 0 for the placeholder.
    fn equipment_new(&mut self, player: &Self::Node, slot: i32) -> Result<i32>;
    /// `inventory.getAmount(id)`.
    fn inventory_amount(&mut self, player: &Self::Node, id: i32) -> Result<i32>;
    /// `inventory.remove(new Item(id, amount))`.
    fn inventory_remove(&mut self, player: &Self::Node, id: i32, amount: i32) -> Result<bool>;
    /// `Player.sendMessage(text)`.
    fn send_message(&mut self, player: &Self::Node, text: &str) -> Result<()>;
    /// `getPacketDispatch().sendMessage(text)`.
    fn packet_message(&mut self, player: &Self::Node, text: &str) -> Result<()>;
    /// `getPacketDispatch().sendInterfaceConfig(interface, child, hide)`.
    fn interface_config(
        &mut self,
        player: &Self::Node,
        interface: i32,
        child: i32,
        hide: bool,
    ) -> Result<()>;
    /// `dispatch(new SpellCastEvent(book, spellId, target))`.
    fn dispatch_cast(
        &mut self,
        player: &Self::Node,
        book: SpellBook,
        spell_id: i32,
        target: Option<&Self::Node>,
    ) -> Result<()>;
    fn face_location(&mut self, entity: &Self::Node, location: Option<[i32; 3]>) -> Result<()>;
    fn face_temporary(
        &mut self,
        entity: &Self::Node,
        target: Option<&Self::Node>,
        ticks: i32,
    ) -> Result<()>;
    fn face(&mut self, entity: &Self::Node, target: Option<&Self::Node>) -> Result<()>;
    fn graphics(&mut self, entity: &Self::Node, graphics: Option<Graphics>) -> Result<()>;
    fn animate(&mut self, entity: &Self::Node, animation: Option<Animation>) -> Result<()>;
    /// `getAnimator().forceAnimation(animation)`.
    fn force_animation(&mut self, entity: &Self::Node, animation: Option<Animation>) -> Result<()>;
    /// `playGlobalAudio(location, id, delay, loops, radius)`.
    fn global_audio(
        &mut self,
        location: [i32; 3],
        id: i32,
        delay: i32,
        loops: i32,
        radius: i32,
    ) -> Result<()>;
    /// `Projectile.magic(source, victim, id, startHeight, endHeight, startDelay, angle)`.
    #[allow(clippy::too_many_arguments)]
    fn projectile_magic(
        &mut self,
        source: &Self::Node,
        victim: Option<&Self::Node>,
        id: i32,
        start_height: i32,
        end_height: i32,
        start_delay: i32,
        angle: i32,
    ) -> Result<Projectile>;
    /// `projectile.transform(source, target, npc, baseSpeed, modifiedSpeed).send()`.
    fn send_projectile(
        &mut self,
        projectile: &Projectile,
        source: &Self::Node,
        target: ProjectileTarget<'_, Self::Node>,
        npc: bool,
        base_speed: i32,
        modified_speed: i32,
    ) -> Result<()>;
    /// `getAttribute(key, default)`.
    fn attribute(&mut self, entity: &Self::Node, key: &str, default: Value) -> Result<Value>;
    /// `setAttribute(key, value)`.
    fn set_attribute(&mut self, entity: &Self::Node, key: &str, value: Value) -> Result<()>;
    /// `removeAttribute(key)`.
    fn remove_attribute(&mut self, entity: &Self::Node, key: &str) -> Result<()>;
    /// `properties.getAutocastSpell()`.
    fn autocast(&mut self, entity: &Self::Node) -> Result<Option<Self::Spell>>;
    /// `properties.setAutocastSpell(spell)`.
    fn set_autocast(&mut self, entity: &Self::Node, spell: Option<&Self::Spell>) -> Result<()>;
    /// `properties.setSpell(spell)`.
    fn set_spell(&mut self, entity: &Self::Node, spell: &Self::Spell) -> Result<()>;
    /// `combatPulse.isAttacking()`.
    fn is_attacking(&mut self, entity: &Self::Node) -> Result<bool>;
    /// `combatPulse.setVictim(victim)`.
    fn set_victim(&mut self, entity: &Self::Node, victim: Option<&Self::Node>) -> Result<()>;
    /// `combatPulse.updateStyle()`.
    fn update_style(&mut self, entity: &Self::Node) -> Result<()>;
    /// `combatPulse.start()`.
    fn pulse_start(&mut self, entity: &Self::Node) -> Result<()>;
    /// `combatPulse.attack(victim)`.
    fn pulse_attack(&mut self, entity: &Self::Node, victim: Option<&Self::Node>) -> Result<()>;
    /// `properties.getAttackStyle()`'s style; `None` is a null style.
    fn attack_style(&mut self, entity: &Self::Node) -> Result<Option<i32>>;
    /// `settings.getAttackStyleIndex()`.
    fn attack_style_index(&mut self, player: &Self::Node) -> Result<i32>;
    fn magic_animation(&mut self, entity: &Self::Node) -> Result<Option<Animation>>;
    fn attack_animation(&mut self, entity: &Self::Node) -> Result<Option<Animation>>;
    /// `properties.isMultiZone()`.
    fn multi_zone(&mut self, entity: &Self::Node) -> Result<bool>;
    /// `properties.getBonuses()`; `None` is a null array.
    fn bonuses(&mut self, entity: &Self::Node) -> Result<Option<Vec<i32>>>;
    /// `zoneMonitor.isInZone(name)`.
    fn in_zone(&mut self, entity: &Self::Node, name: &str) -> Result<bool>;
    /// `hasTimerActive(entity, name)`.
    fn timer_active(&mut self, entity: &Self::Node, name: &str) -> Result<bool>;
    /// `savedData.activityData.getGodCasts()`; `None` is a null array.
    fn god_casts(&mut self, player: &Self::Node) -> Result<Option<Vec<i32>>>;
    /// `NPC.getId()`.
    fn npc_id(&mut self, npc: &Self::Node) -> Result<i32>;
    /// `NPC.getTask()`'s `undead`; `None` is a null task.
    fn npc_task(&mut self, npc: &Self::Node) -> Result<Option<bool>>;
    /// `npc.getDefinition().getCombatGraphics()`; `None` is a null array.
    fn combat_graphics(&mut self, npc: &Self::Node) -> Result<Option<Vec<Option<Graphics>>>>;
    /// `findMultihitTargets(target, attacker, CombatStyle.MAGIC)`.
    fn multi_targets(
        &mut self,
        target: &Self::Node,
        attacker: &Self::Node,
    ) -> Result<Vec<Self::Node>>;
    /// `interfaceManager.openTab(new Component(id))`.
    fn open_tab(&mut self, player: &Self::Node, id: i32) -> Result<()>;
    /// `setVarbit(player, id, value, save)`.
    fn set_varbit(&mut self, player: &Self::Node, id: i32, value: i32, save: bool) -> Result<()>;
    /// `setComponentVisibility(player, interface, child, hide)`.
    fn component_visibility(
        &mut self,
        player: &Self::Node,
        interface: i32,
        child: i32,
        hide: bool,
    ) -> Result<()>;
    /// `WeaponInterface.setAttackStyle(button)`.
    fn set_attack_style(&mut self, player: &Self::Node, button: i32) -> Result<bool>;
    /// One `Math.random()` draw.
    fn math_random(&mut self) -> Result<f64>;
}
