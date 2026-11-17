//! `EmoteManager` and `Emotes`: the emote tab's varps, unlocking and locking,
//! the emote button handler and playing an emote.
//!
//! The unlocked list is the player's persisted
//! [`Manager`](crate::player::persisted::emotes::Manager) (ordinals in list
//! order). The SALUTE, GOBLIN_BOW and GIVE_THANKS overrides of `play` are
//! content (an achievement diary task, a quest dialogue, a world pulse) and
//! are the required [`Host::content_play`] service.
use super::{Result, Value};
use crate::player::persisted::emotes::Manager;

/// `Emotes`, in declaration order (the ordinal is the discriminant).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Emote {
    Yes,
    No,
    Bow,
    Angry,
    Think,
    Wave,
    Shrug,
    Cheer,
    Beckon,
    JumpForJoy,
    Laugh,
    Yawn,
    Dance,
    Jig,
    Spin,
    Headbang,
    Cry,
    BlowKiss,
    Panic,
    Raspberry,
    Clap,
    Salute,
    GoblinBow,
    GoblinSalute,
    GlassBox,
    ClimbRope,
    Lean,
    GlassWall,
    Idea,
    Stomp,
    Flap,
    SlapHead,
    ZombieWalk,
    ZombieDance,
    ZombieHand,
    Scared,
    BunnyHop,
    Skillcape,
    SnowmanDance,
    AirGuitar,
    SafetyFirst,
    Explore,
    Trick,
    Freeze,
    GiveThanks,
}

const STRONGHOLD: &str =
    "You can't use this emote yet. <br>Visit the Stronghold of Security to unlock it.";
const LOST_TRIBE: &str = "This emote can be unlocked during the Lost Tribe quest.";
const MIME: &str = "This emote can be unlocked during the Mime random event.";
const GRAVEDIGGER: &str = "This emote can be unlocked during the Gravedigger random event.";
const HALLOWEEN: &str = "This emote can be unlocked by playing a Halloween holiday event.";
const CHRISTMAS: &str = "This emote can be unlocked by playing a Christmas holiday event.";

/// One constant's constructor values: button, animation, graphics, locked message.
pub struct Data {
    pub button: i32,
    pub animation: Option<i32>,
    pub graphics: Option<i32>,
    pub locked_message: Option<&'static str>,
}

const fn d(button: i32, animation: i32) -> Data {
    Data {
        button,
        animation: Some(animation),
        graphics: None,
        locked_message: None,
    }
}
const fn dl(button: i32, animation: i32, message: &'static str) -> Data {
    Data {
        button,
        animation: Some(animation),
        graphics: None,
        locked_message: Some(message),
    }
}
const fn dg(button: i32, animation: i32, graphics: i32, message: Option<&'static str>) -> Data {
    Data {
        button,
        animation: Some(animation),
        graphics: Some(graphics),
        locked_message: message,
    }
}

/// The constructor values of every constant, by ordinal.
pub const DATA: [Data; 45] = [
    d(2, 855),
    d(3, 856),
    d(4, 858),
    d(5, 859),
    d(6, 857),
    d(7, 863),
    d(8, 2113),
    d(9, 862),
    d(10, 864),
    d(11, 2109),
    d(12, 861),
    d(13, 2111),
    d(14, 866),
    d(15, 2106),
    d(16, 2107),
    d(17, 2108),
    d(18, 860),
    dg(19, 1368, 574, None),
    d(20, 2105),
    d(21, 2110),
    d(22, 865),
    d(23, 2112),
    dl(24, 2127, LOST_TRIBE),
    dl(25, 2128, LOST_TRIBE),
    dl(26, 1131, MIME),
    dl(27, 1130, MIME),
    dl(28, 1129, MIME),
    dl(29, 1128, MIME),
    dg(33, 4276, 712, Some(STRONGHOLD)),
    dl(31, 4278, STRONGHOLD),
    dl(
        32,
        4280,
        "You can't use this emote yet. <br>Visit the Strongshold of Security to unlock it.",
    ),
    dl(30, 4275, STRONGHOLD),
    dl(34, 3544, GRAVEDIGGER),
    dl(35, 3543, GRAVEDIGGER),
    dg(36, 7272, 1244, Some(GRAVEDIGGER)),
    dl(37, 2836, HALLOWEEN),
    dl(
        38,
        6111,
        "This emote can be unlocked by playing an Easter holiday event.",
    ),
    Data {
        button: 39,
        animation: None,
        graphics: None,
        locked_message: None,
    },
    dl(40, 7531, CHRISTMAS),
    dg(
        41,
        2414,
        1537,
        Some("This emote can be accessed by unlocking 200 pieces of music."),
    ),
    dg(
        42,
        8770,
        1553,
        Some(
            "You can't use this emote yet. Visit the Stronghold of Player safety to<br>unlock it.",
        ),
    ),
    dg(
        43,
        9990,
        1734,
        Some(
            "You can't use this emote yet. You must complete all the Lumbridge <br>and Draynor beginner tasks to unlock it.",
        ),
    ),
    dg(44, 10530, 1863, Some(HALLOWEEN)),
    dg(45, 11044, 1973, Some(CHRISTMAS)),
    Data {
        button: 46,
        animation: None,
        graphics: None,
        locked_message: Some("This emote can be unlocked by playing a Thanksgiving holiday event."),
    },
];

/// `SKILLCAPE_INFO`: cape, trimmed cape, graphic, animation.
pub const SKILLCAPE_INFO: [[i32; 4]; 25] = [
    [9747, 9748, 823, 4959],
    [9750, 9751, 828, 4981],
    [9753, 9754, 824, 4961],
    [9756, 9757, 832, 4973],
    [9759, 9760, 829, 4979],
    [9762, 9763, 813, 4939],
    [9765, 9766, 817, 4947],
    [9768, 9769, 833, 4971],
    [9771, 9772, 830, 4977],
    [9774, 9775, 835, 4969],
    [9777, 9778, 826, 4965],
    [9780, 9781, 818, 4949],
    [9783, 9784, 812, 4937],
    [9786, 9787, 1656, 4967],
    [9789, 9790, 820, 4953],
    [9792, 9793, 814, 4941],
    [9795, 9796, 815, 4943],
    [9798, 9799, 819, 4951],
    [9801, 9802, 821, 4955],
    [9804, 9805, 831, 4975],
    [9807, 9808, 822, 4957],
    [9810, 9811, 825, 4963],
    [12169, 12170, 1515, 8525],
    [9813, -1, 816, 4945],
    [9948, 9949, 907, 5158],
];

/// `EquipmentContainer` slots the overrides read.
const SLOT_HAT: i32 = 0;
const SLOT_CAPE: i32 = 1;
const SLOT_CHEST: i32 = 4;
const SLOT_LEGS: i32 = 7;
const SLOT_FEET: i32 = 10;
/// `Rights.ADMINISTRATOR.ordinal()`.
pub const ADMINISTRATOR: i32 = 2;
const NEEDS_CAPE: &str = "You need to be wearing a skillcape in order to perform this emote.";

impl Emote {
    pub const ALL: [Self; 45] = [
        Self::Yes,
        Self::No,
        Self::Bow,
        Self::Angry,
        Self::Think,
        Self::Wave,
        Self::Shrug,
        Self::Cheer,
        Self::Beckon,
        Self::JumpForJoy,
        Self::Laugh,
        Self::Yawn,
        Self::Dance,
        Self::Jig,
        Self::Spin,
        Self::Headbang,
        Self::Cry,
        Self::BlowKiss,
        Self::Panic,
        Self::Raspberry,
        Self::Clap,
        Self::Salute,
        Self::GoblinBow,
        Self::GoblinSalute,
        Self::GlassBox,
        Self::ClimbRope,
        Self::Lean,
        Self::GlassWall,
        Self::Idea,
        Self::Stomp,
        Self::Flap,
        Self::SlapHead,
        Self::ZombieWalk,
        Self::ZombieDance,
        Self::ZombieHand,
        Self::Scared,
        Self::BunnyHop,
        Self::Skillcape,
        Self::SnowmanDance,
        Self::AirGuitar,
        Self::SafetyFirst,
        Self::Explore,
        Self::Trick,
        Self::Freeze,
        Self::GiveThanks,
    ];

    pub fn ordinal(self) -> i32 {
        self as i32
    }

    pub fn from_ordinal(ordinal: i32) -> Option<Self> {
        usize::try_from(ordinal)
            .ok()
            .and_then(|i| Self::ALL.get(i).copied())
    }

    pub fn data(self) -> &'static Data {
        &DATA[self as usize]
    }

    /// `Emotes.forId(buttonId)`: the first constant with that button.
    pub fn for_id(button: i32) -> Option<Self> {
        Self::ALL.into_iter().find(|e| e.data().button == button)
    }
}

/// Every call `EmoteManager` and `Emotes` make outside their classes, for the
/// owning player.
pub trait Host {
    /// `ContentAPI.setVarp(player, id, value, save)`.
    fn varp(&mut self, id: i32, value: i32, save: bool) -> Result<()>;
    /// `player.getLocks().isLocked(key)`.
    fn is_locked(&mut self, key: &str) -> Result<bool>;
    /// `player.getLocks().lock(key, ticks)`.
    fn lock(&mut self, key: &str, ticks: i32) -> Result<()>;
    /// `player.getLocks().lock(ticks)`.
    fn lock_all(&mut self, ticks: i32) -> Result<()>;
    /// `properties.getCombatPulse().isAttacking()`.
    fn is_attacking(&mut self) -> Result<bool>;
    /// `player.inCombat()`.
    fn in_combat(&mut self) -> Result<bool>;
    /// `player.debug(text)`.
    fn debug(&mut self, text: &str) -> Result<()>;
    /// `player.getRights()`, as its ordinal.
    fn rights(&mut self) -> Result<i32>;
    /// `player.getPulseManager().clear()`.
    fn pulse_clear(&mut self) -> Result<()>;
    /// `player.getDialogueInterpreter().sendDialogue(messages)`.
    fn dialogue(&mut self, messages: &[&str]) -> Result<()>;
    /// `getAchievementDiaryManager().getDiary(VARROCK).isComplete(level, index)`.
    fn varrock_diary_complete(&mut self, level: i32, index: i32) -> Result<bool>;
    /// `getDiary(VARROCK).updateTask(player, level, index, complete)`.
    fn varrock_diary_update(&mut self, level: i32, index: i32, complete: bool) -> Result<()>;
    /// `entity.getAttribute(key, default)`.
    fn get_attribute(&mut self, key: &str, default: Value) -> Result<Value>;
    /// `entity.setAttribute(key, value)`.
    fn set_attribute(&mut self, key: &str, value: Value) -> Result<()>;
    /// `player.getEquipment().get(slot)`: the item id, `None` for null.
    fn equipment(&mut self, slot: i32) -> Result<Option<i32>>;
    /// `player.getAnimator().animate(Animation.create(id), graphics)`.
    fn animate(&mut self, animation: i32, graphics: Option<i32>) -> Result<()>;
    /// `Animation.getDuration()` of that animation id.
    fn animation_duration(&mut self, animation: i32) -> Result<i32>;
    /// `packetDispatch.sendGraphic(id)`.
    fn graphic(&mut self, id: i32) -> Result<()>;
    /// `packetDispatch.sendAnimation(id)`.
    fn animation(&mut self, id: i32) -> Result<()>;
    /// `packetDispatch.sendMessage(text)`.
    fn message(&mut self, text: &str) -> Result<()>;
    /// `ContentAPI.playJingle(player, id)`.
    fn jingle(&mut self, id: i32) -> Result<()>;
    /// The `play` override of SALUTE, GOBLIN_BOW or GIVE_THANKS: content.
    fn content_play(&mut self, emote: Emote) -> Result<()>;
}

/// `EmoteManager.refresh()`.
pub fn refresh<H: Host>(h: &mut H, manager: &mut Manager) -> Result<()> {
    let has = |m: &Manager, e: Emote| m.is_unlocked(e.ordinal());
    let mut value1 = 0;
    if has(manager, Emote::Idea) {
        value1 += 4;
    }
    if has(manager, Emote::Flap) {
        value1 += 1;
    }
    if has(manager, Emote::Stomp) {
        value1 += 8;
    }
    if has(manager, Emote::SlapHead) {
        value1 += 2;
    }
    h.varp(802, value1, false)?;
    let mut value2 = 0;
    for (emote, bit) in [
        (Emote::GlassBox, 2),
        (Emote::ClimbRope, 4),
        (Emote::Lean, 8),
        (Emote::GlassWall, 1),
        (Emote::Scared, 16),
        (Emote::ZombieDance, 32),
        (Emote::ZombieWalk, 64),
        (Emote::BunnyHop, 128),
    ] {
        if has(manager, emote) {
            value2 += bit;
        }
    }
    if !has(manager, Emote::Skillcape) {
        unlock(h, manager, Emote::Skillcape)?;
    }
    value2 += 256;
    for (emote, bit) in [
        (Emote::SnowmanDance, 512),
        (Emote::AirGuitar, 1024),
        (Emote::SafetyFirst, 2048),
        (Emote::Trick, 8192),
        (Emote::Explore, 4096),
        (Emote::Freeze, 32768),
        (Emote::GiveThanks, 16384),
    ] {
        if has(manager, emote) {
            value2 += bit;
        }
    }
    h.varp(313, value2, false)?;
    let mut value3 = 0;
    if has(manager, Emote::ZombieHand) {
        value3 = 12;
    }
    h.varp(1085, value3, false)
}

/// `EmoteManager.lock(emote)`: the first 23 constants never lock; the list
/// refreshes whether or not the emote was in it.
pub fn lock<H: Host>(h: &mut H, manager: &mut Manager, emote: Emote) -> Result<bool> {
    if emote.ordinal() <= 22 {
        return Ok(false);
    }
    let locked = manager.remove(emote.ordinal());
    refresh(h, manager)?;
    Ok(locked)
}

/// `EmoteManager.unlock(emote)`.
pub fn unlock<H: Host>(h: &mut H, manager: &mut Manager, emote: Emote) -> Result<bool> {
    if manager.is_unlocked(emote.ordinal()) {
        return Ok(true);
    }
    manager.emotes.push(emote.ordinal());
    refresh(h, manager)?;
    Ok(true)
}

fn attribute_bool<H: Host>(h: &mut H, key: &str) -> Result<bool> {
    h.get_attribute(key, Value::Bool(false))?.boolean()
}

/// `Emotes.handle(player, buttonId)`.
pub fn handle<H: Host>(h: &mut H, manager: &mut Manager, button: i32) -> Result<()> {
    if h.is_locked("emote")? {
        return h.message("You're already doing an emote!");
    }
    if h.is_attacking()? || h.in_combat()? {
        return h.message("You can't perform an emote while being in combat.");
    }
    let Some(emote) = Emote::for_id(button) else {
        return h.debug(&format!("Unhandled emote for button id - {button}"));
    };
    if !manager.is_unlocked(emote.ordinal()) {
        if h.rights()? == ADMINISTRATOR {
            unlock(h, manager, emote)?;
            h.pulse_clear()?;
            return play(h, emote);
        }
        let message = emote
            .data()
            .locked_message
            .unwrap_or("You can't use this emote.");
        return h.dialogue(&[message]);
    }
    if !h.varrock_diary_complete(1, 6)? && (30..=33).contains(&button) {
        let key = format!("emote-{button}");
        if !attribute_bool(h, &key)? {
            h.set_attribute(&key, Value::Bool(true))?;
        }
        let mut good = true;
        for i in 30..=33 {
            if !attribute_bool(h, &format!("emote-{i}"))? {
                good = false;
                break;
            }
        }
        h.varrock_diary_update(1, 6, good)?;
    }
    h.pulse_clear()?;
    play(h, emote)
}

/// `forceEmote(player, animation, graphic)`.
fn force_emote<H: Host>(h: &mut H, animation: Option<i32>, graphics: Option<i32>) -> Result<()> {
    if let Some(animation) = animation {
        h.animate(animation, graphics)?;
        let duration = h.animation_duration(animation)?;
        h.lock("emote", duration)?;
    }
    Ok(())
}

/// `emote.play(player)`, with each constant's override.
pub fn play<H: Host>(h: &mut H, emote: Emote) -> Result<()> {
    let worn =
        |h: &mut H, slot: i32, id: i32| -> Result<bool> { Ok(h.equipment(slot)? == Some(id)) };
    match emote {
        Emote::Bow => {
            if worn(h, SLOT_LEGS, 10396)? {
                return force_emote(h, Some(5312), None);
            }
        }
        Emote::Angry => {
            if worn(h, SLOT_HAT, 10392)? {
                return force_emote(h, Some(5315), None);
            }
        }
        Emote::Yawn => {
            if worn(h, SLOT_HAT, 10398)? {
                return force_emote(h, Some(5313), None);
            }
        }
        Emote::Dance => {
            if worn(h, SLOT_LEGS, 10394)? {
                return force_emote(h, Some(5316), None);
            }
        }
        Emote::Flap => {
            let head = h.equipment(SLOT_HAT)?;
            let wings = h.equipment(SLOT_CHEST)?;
            let legs = h.equipment(SLOT_LEGS)?;
            let feet = h.equipment(SLOT_FEET)?;
            if let (Some(head), Some(wings), Some(legs), Some(feet)) = (head, wings, legs, feet)
                && head == 11021
                && wings == 11020
                && legs == 11022
                && feet == 11019
            {
                return force_emote(h, Some(3859), None);
            }
        }
        Emote::Skillcape => {
            let Some(cape) = h.equipment(SLOT_CAPE)? else {
                return h.message(NEEDS_CAPE);
            };
            for info in SKILLCAPE_INFO {
                if cape == info[0] || cape == info[1] {
                    h.graphic(info[2])?;
                    h.animation(info[3])?;
                    let duration = h.animation_duration(info[3])?;
                    h.lock("emote", duration)?;
                    return h.lock_all(duration);
                }
            }
            return h.message(NEEDS_CAPE);
        }
        Emote::AirGuitar => {
            h.jingle(302)?;
        }
        Emote::Salute | Emote::GoblinBow | Emote::GiveThanks => return h.content_play(emote),
        _ => {}
    }
    let data = emote.data();
    force_emote(h, data.animation, data.graphics)
}
