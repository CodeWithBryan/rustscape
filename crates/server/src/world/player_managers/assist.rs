//! `AssistSession`: the ASSIST request module, the assisting session with its
//! interface 301 and close event, the per-skill experience caps, the 24-hour
//! window, translated experience and the session pulse.
//!
//! A session is a shared object ([`Session`]): the player's and the
//! partner's extension both hold it, the world pulser holds it as a pulse and
//! its component's close event captures it. The fields are [`AssistSession`];
//! no lock on them is held across a service call. The experience array is
//! itself shared ([`Exp`]): `save` stores the session's array in the
//! player's GlobalData, so later changes to the session's experience show
//! there too, as in the original. GlobalData's assist time and experience
//! are services ([`Host::assist_time`] and the rest).
//!
//! `trade::Host::assist_open` (RequestType.ASSIST's `module.open`) is
//! answered by [`module_open`].
use super::{Error, Result};
use crate::{definitions::item::Shared, world::trade::Location};

/// The interface.
pub const COMPONENT: i32 = 301;
/// `ANIMATION` and `GRAPHIC`.
pub const ANIMATION: i32 = 7299;
pub const GRAPHIC: i32 = 1247;
/// The assist sound played to both players.
pub const AUDIO: i32 = 4010;
/// `TIME_OUT`: 24 hours.
pub const TIME_OUT: i64 = 86_400_000;
/// `CONFIG_VALUES` and `CHILD_VALUES`.
pub const CONFIG_VALUES: [i32; 9] = [3, 4, 6, 8, 9, 11, 13, 14, 15];
pub const CHILD_VALUES: [i32; 9] = [46, 48, 50, 52, 54, 56, 58, 60, 62];
/// The experience cap per skill and in total.
pub const CAP: f64 = 30000.0;

/// A `double[]` shared between a session and GlobalData.
pub type Exp = Shared<Vec<f64>>;

/// The session's fields.
#[derive(Debug)]
pub struct AssistSession<P> {
    pub player: Option<P>,
    pub partener: Option<P>,
    pub skills: Vec<bool>,
    pub restricted: bool,
    pub exp: Exp,
    pub kill: bool,
    pub time: i64,
}

/// The session object.
pub type Session<P> = Shared<AssistSession<P>>;

/// `new AssistSession(player, partener)`; [`empty`] is the no-argument one.
pub fn new<P>(player: Option<P>, partener: Option<P>) -> Session<P> {
    Shared::new(AssistSession {
        player,
        partener,
        skills: vec![false; 9],
        restricted: false,
        exp: Shared::new(vec![0.0; 9]),
        kill: false,
        time: 0,
    })
}

/// `new AssistSession()`: RequestType.ASSIST's module object.
pub fn empty<P>() -> Session<P> {
    new(None, None)
}

/// Every call AssistSession makes outside its class.
pub trait Host {
    type Player: Clone + PartialEq;
    /// `getIronmanManager().checkRestriction()`.
    fn ironman_restricted(&mut self, player: &Self::Player) -> Result<bool>;
    /// `face(target)`.
    fn face(&mut self, player: &Self::Player, target: &Self::Player) -> Result<()>;
    /// `addExtension(AssistSession.class, session)`.
    fn add_extension(
        &mut self,
        player: &Self::Player,
        session: &Session<Self::Player>,
    ) -> Result<()>;
    /// `getExtension(AssistSession.class)`.
    fn extension(&mut self, player: &Self::Player) -> Result<Option<Session<Self::Player>>>;
    /// `removeExtension(AssistSession.class)`.
    fn remove_extension(&mut self, player: &Self::Player) -> Result<()>;
    fn lock(&mut self, player: &Self::Player) -> Result<()>;
    fn unlock(&mut self, player: &Self::Player) -> Result<()>;
    /// `getInterfaceManager().open(component)`: the session's component 301,
    /// whose close event is [`close_event`] of this session.
    fn open_component(
        &mut self,
        player: &Self::Player,
        session: &Session<Self::Player>,
    ) -> Result<()>;
    /// `getInterfaceManager().close()`.
    fn close_interface(&mut self, player: &Self::Player) -> Result<bool>;
    /// `getInterfaceManager().restoreTabs()`.
    fn restore_tabs(&mut self, player: &Self::Player) -> Result<()>;
    /// `getPacketDispatch().sendMessage(text)`.
    fn message(&mut self, player: &Self::Player, text: &str) -> Result<()>;
    /// `getPacketDispatch().sendString(text, interface, child)`.
    fn send_string(
        &mut self,
        player: &Self::Player,
        text: &str,
        interface: i32,
        child: i32,
    ) -> Result<()>;
    /// `getPacketDispatch().sendInterfaceConfig(interface, child, hide)`.
    fn interface_config(
        &mut self,
        player: &Self::Player,
        interface: i32,
        child: i32,
        hide: bool,
    ) -> Result<()>;
    /// `getUsername()`.
    fn username(&mut self, player: &Self::Player) -> Result<String>;
    /// `animate(Animation.create(id))`.
    fn animate(&mut self, player: &Self::Player, id: i32) -> Result<()>;
    /// `graphics(Graphics.create(id))`.
    fn graphics(&mut self, player: &Self::Player, id: i32) -> Result<()>;
    /// `ContentAPI.playAudio(player, id)` (delay 0, one loop).
    fn audio(&mut self, player: &Self::Player, id: i32) -> Result<()>;
    /// `GameWorld.getPulser().submit(session)`.
    fn submit(&mut self, session: &Session<Self::Player>) -> Result<()>;
    /// `ContentAPI.setVarp(player, id, value)` (not saved).
    fn set_varp(&mut self, player: &Self::Player, id: i32, value: i32) -> Result<()>;
    /// `System.currentTimeMillis()`.
    fn now(&mut self) -> Result<i64>;
    /// The player's GlobalData object, `getSavedData().getGlobalData()`.
    type GlobalData;
    /// `getSavedData().getGlobalData()`.
    fn global_data(&mut self, player: &Self::Player) -> Result<Self::GlobalData>;
    /// `getAssistTime()`.
    fn assist_time(&mut self, data: &Self::GlobalData) -> i64;
    /// `setAssistTime(time)`.
    fn set_assist_time(&mut self, data: &Self::GlobalData, time: i64);
    /// `getAssistExperience()`: the stored array itself.
    fn assist_experience(&mut self, data: &Self::GlobalData) -> Exp;
    /// `setAssistExperience(array)`: stores the array itself.
    fn set_assist_experience(&mut self, data: &Self::GlobalData, exp: Exp);
    /// `getSkills().refresh()`.
    fn skills_refresh(&mut self, player: &Self::Player) -> Result<()>;
    /// `getSkills().getLevel(slot)`.
    fn level(&mut self, player: &Self::Player, slot: i32) -> Result<i32>;
    /// `getSkills().addExperience(slot, experience)`.
    fn add_experience(&mut self, player: &Self::Player, slot: i32, experience: f64) -> Result<()>;
    /// `getLocation()`; None is null.
    fn location(&mut self, player: &Self::Player) -> Result<Option<Location>>;
    /// `isActive()`.
    fn is_active(&mut self, player: &Self::Player) -> Result<bool>;
}

fn player<P: Clone>(session: &Session<P>) -> Result<P> {
    session.read().player.clone().ok_or(Error::Null)
}
fn partener<P: Clone>(session: &Session<P>) -> Result<P> {
    session.read().partener.clone().ok_or(Error::Null)
}
fn exp<P>(session: &Session<P>) -> Exp {
    session.read().exp.clone()
}
fn element(exp: &Exp, index: i32) -> Result<f64> {
    usize::try_from(index)
        .ok()
        .and_then(|i| exp.read().get(i).copied())
        .ok_or(Error::Index)
}
fn store(exp: &Exp, index: i32, value: f64) -> Result<()> {
    let index = usize::try_from(index).map_err(|_| Error::Index)?;
    *exp.write().get_mut(index).ok_or(Error::Index)? = value;
    Ok(())
}

/// `getSkillIndex(skill)`.
pub fn skill_index(skill: i32) -> i32 {
    match skill {
        20 => 0,
        12 => 1,
        9 => 2,
        22 => 3,
        19 => 4,
        6 => 5,
        13 => 6,
        7 => 7,
        15 => 8,
        _ => -1,
    }
}

/// `extend(player, partener)`: a new session as the player's extension. A
/// null partner is kept (the session fails when it reaches it).
pub fn extend<H: Host>(
    h: &mut H,
    player: &H::Player,
    partener: Option<&H::Player>,
) -> Result<Session<H::Player>> {
    let session = new(Some(player.clone()), partener.cloned());
    h.add_extension(player, &session)?;
    Ok(session)
}

/// `open(player, target)`, the RequestModule method (ironman checks, the
/// players face each other, a new session extended and opened).
pub fn module_open<H: Host>(h: &mut H, player: &H::Player, target: &H::Player) -> Result<()> {
    if h.ironman_restricted(player)? || h.ironman_restricted(target)? {
        return Ok(());
    }
    h.face(player, target)?;
    h.face(target, player)?;
    extend(h, player, Some(target))?;
    let session = h.extension(player)?.ok_or(Error::Null)?;
    open(h, &session)
}

/// `toggleIcon(player, hide)`: children 79 and 80 of the game frame.
pub fn toggle_icon<H: Host>(h: &mut H, player: &H::Player, hide: bool) -> Result<()> {
    for child in 79..81 {
        h.interface_config(player, 548, child, hide)?;
    }
    Ok(())
}

/// The session's `open()`.
pub fn open<H: Host>(h: &mut H, session: &Session<H::Player>) -> Result<()> {
    // Each statement reads the player and partner fields when it runs.
    let p = |s: &Session<H::Player>| player(s);
    let pa = |s: &Session<H::Player>| partener(s);
    h.add_extension(&pa(session)?, session)?;
    h.lock(&p(session)?)?;
    h.open_component(&p(session)?, session)?;
    load(h, session)?;
    h.message(&p(session)?, "Sending assistance response.")?;
    let to = p(session)?;
    let name = h.username(&pa(session)?)?;
    h.message(&to, &format!("You are assisting {name}."))?;
    let to = pa(session)?;
    let name = h.username(&p(session)?)?;
    h.message(&to, &format!("You are being assisted by {name}."))?;
    let to = p(session)?;
    let name = h.username(&pa(session)?)?;
    h.send_string(
        &to,
        &format!("Assist System XP Display - You are assisting {name}"),
        COMPONENT,
        101,
    )?;
    h.send_string(&p(session)?, "", COMPONENT, 10)?;
    h.animate(&p(session)?, ANIMATION)?;
    h.graphics(&p(session)?, GRAPHIC)?;
    h.animate(&pa(session)?, ANIMATION)?;
    h.audio(&pa(session)?, AUDIO)?;
    h.audio(&p(session)?, AUDIO)?;
    toggle_icon(h, &p(session)?, false)?;
    toggle_icon(h, &pa(session)?, false)?;
    h.submit(session)?;
    refresh(h, session)
}

/// The component's close event, `close(player, component)`: the session is
/// saved for its own player, the extension removed from the closing player
/// and the partner, icons back, tabs restored, both told, the session killed.
pub fn close_event<H: Host>(
    h: &mut H,
    session: &Session<H::Player>,
    player: &H::Player,
) -> Result<bool> {
    save(h, session)?;
    h.remove_extension(player)?;
    h.remove_extension(&partener(session)?)?;
    h.unlock(player)?;
    toggle_icon(h, player, true)?;
    toggle_icon(h, &partener(session)?, true)?;
    h.restore_tabs(player)?;
    let name = h.username(&partener(session)?)?;
    h.message(player, &format!("You have stopped assisting {name}."))?;
    let to = partener(session)?;
    let name = h.username(&self::player(session)?)?;
    h.message(&to, &format!("{name} has stopped assisting you."))?;
    session.write().kill = true;
    Ok(true)
}
/// `getTotalExperience()`: the first nine entries.
pub fn total_experience<P>(session: &Session<P>) -> Result<f64> {
    let exp = exp(session);
    let mut xp = 0.0;
    for i in 0..9 {
        xp += element(&exp, i)?;
    }
    Ok(xp)
}

/// `addExperience(skill, experience)` for the assister.
pub fn add_experience<H: Host>(
    h: &mut H,
    session: &Session<H::Player>,
    skill: i32,
    experience: f64,
) -> Result<()> {
    let restricted = session.read().restricted;
    if restricted || total_experience(session)? >= CAP {
        session.write().restricted = true;
        return Ok(());
    }
    let index = skill_index(skill);
    let exp = exp(session);
    if element(&exp, index)? + experience >= CAP {
        store(&exp, index, CAP)?;
        session.write().restricted = true;
        return Ok(());
    }
    store(&exp, index, element(&exp, index)? + experience)?;
    if element(&exp, index)? >= CAP {
        store(&exp, index, CAP)?;
    }
    refresh(h, session)
}

/// `refresh()`: the nine experience lines, the toggled-skill varp 1087, the
/// restriction from the total, its message on child 10 and varp 1088.
pub fn refresh<H: Host>(h: &mut H, session: &Session<H::Player>) -> Result<()> {
    let mut value = 0i32;
    let mut total = 0.0;
    for i in 0..9i32 {
        let exp = exp(session);
        if element(&exp, i)? >= CAP {
            store(&exp, i, CAP)?;
        }
        let player = player(session)?;
        let text = (element(&exp, i)? as i32).to_string();
        h.send_string(&player, &text, COMPONENT, CHILD_VALUES[i as usize])?;
        if *session.read().skills.get(i as usize).ok_or(Error::Index)? {
            value |= 1i32.wrapping_shl(CONFIG_VALUES[i as usize] as u32);
        }
        total += element(&self::exp(session), i)?;
    }
    let restricted = total_experience(session)? >= CAP;
    session.write().restricted = restricted;
    let mut message = String::new();
    if session.read().restricted {
        message = format!(
            "You've earned the maximum XP from the Assist System within a 24-hour period. You can assist again in {}.",
            time_left(h, session)?
        );
    }
    h.send_string(&player(session)?, &message, COMPONENT, 10)?;
    h.set_varp(&player(session)?, 1087, value)?;
    h.set_varp(&player(session)?, 1088, (total as i32).wrapping_mul(10))
}

/// `toggleButton(id)`.
pub fn toggle_button<P>(session: &Session<P>, id: i8) -> Result<()> {
    let index = usize::try_from(id).map_err(|_| Error::Index)?;
    let mut fields = session.write();
    let skill = fields.skills.get_mut(index).ok_or(Error::Index)?;
    *skill = !*skill;
    Ok(())
}

/// `isToggled(skill)`.
pub fn is_toggled<P>(session: &Session<P>, skill: i32) -> Result<bool> {
    let index = usize::try_from(skill_index(skill)).map_err(|_| Error::Index)?;
    session
        .read()
        .skills
        .get(index)
        .copied()
        .ok_or(Error::Index)
}

/// `getTimeLeft()`: the time until the window ends in hours above one hour,
/// minutes above one minute, otherwise seconds (truncating divisions).
pub fn time_left<H: Host>(h: &mut H, session: &Session<H::Player>) -> Result<String> {
    let time = session.read().time;
    let now = h.now()?;
    let diff = time.wrapping_sub(now);
    let seconds = diff / 1000;
    let minutes = diff / (60 * 1000);
    let hours = diff / (60 * 60 * 1000);
    if hours > 1 {
        return Ok(format!("{hours} hours"));
    }
    if minutes > 1 {
        return Ok(format!("{minutes} minutes"));
    }
    Ok(format!("{seconds} seconds"))
}

/// `load()`: the window end from GlobalData (a zero one starts a new window
/// in GlobalData only), then the nine stored values copied.
pub fn load<H: Host>(h: &mut H, session: &Session<H::Player>) -> Result<()> {
    let data = h.global_data(&player(session)?)?;
    let time = h.assist_time(&data);
    session.write().time = time;
    if session.read().time == 0 {
        let data = h.global_data(&player(session)?)?;
        let now = h.now()?;
        h.set_assist_time(&data, now.wrapping_add(TIME_OUT));
    }
    for i in 0..9 {
        let data = h.global_data(&player(session)?)?;
        let stored = h.assist_experience(&data);
        let value = element(&stored, i)?;
        store(&exp(session), i, value)?;
    }
    Ok(())
}

/// `save()`: skills refreshed, the window end and the session's own
/// experience array (not a copy) stored in GlobalData.
pub fn save<H: Host>(h: &mut H, session: &Session<H::Player>) -> Result<()> {
    h.skills_refresh(&player(session)?)?;
    let data = h.global_data(&player(session)?)?;
    let time = session.read().time;
    h.set_assist_time(&data, time);
    let data = h.global_data(&player(session)?)?;
    h.set_assist_experience(&data, exp(session));
    Ok(())
}

/// `reset()`: GlobalData cleared to a new array, then loaded.
pub fn reset<H: Host>(h: &mut H, session: &Session<H::Player>) -> Result<()> {
    let data = h.global_data(&player(session)?)?;
    h.set_assist_time(&data, 0);
    let data = h.global_data(&player(session)?)?;
    h.set_assist_experience(&data, Shared::new(vec![0.0; 9]));
    load(h, session)
}
/// `pulse()`: closed when the players are more than 20 tiles apart, on other
/// planes or either is inactive; a new window (experience zeroed) once the
/// time has passed; refreshed; finished once killed.
pub fn pulse<H: Host>(h: &mut H, session: &Session<H::Player>) -> Result<bool> {
    let player = player(session)?;
    let here = h.location(&player)?;
    let partener = partener(session)?;
    let there = h.location(&partener)?;
    let near = here
        .ok_or(Error::Null)?
        .within_distance(there.ok_or(Error::Null)?, 20);
    let leave = !near
        || {
            let partener = self::partener(session)?;
            !h.is_active(&partener)?
        }
        || {
            let player = self::player(session)?;
            !h.is_active(&player)?
        };
    if leave {
        let player = self::player(session)?;
        h.close_interface(&player)?;
        return Ok(true);
    }
    let time = session.read().time;
    if time < h.now()? {
        let now = h.now()?;
        session.write().time = now.wrapping_add(TIME_OUT);
        let exp = exp(session);
        for i in 0..9 {
            store(&exp, i, 0.0)?;
        }
    }
    refresh(h, session)?;
    Ok(session.read().kill)
}

/// `translateExperience(p, slot, experience, mod)`: experience another
/// player earns in a toggled skill is given to the assister (capped at the
/// remaining allowance), who also gains it times `mod` in the session.
pub fn translate_experience<H: Host>(
    h: &mut H,
    session: &Session<H::Player>,
    p: &H::Player,
    slot: i32,
    experience: f64,
    modifier: f64,
) -> Result<bool> {
    let mut experience = experience;
    if session.read().player.as_ref() != Some(p) {
        let index = skill_index(slot);
        if index != -1 && !session.read().restricted {
            let level = h.level(p, slot)?;
            let player = player(session)?;
            let p_level = h.level(&player, slot)?;
            if p_level < level {
                return Ok(false);
            }
            let toggled = {
                let at = usize::try_from(skill_index(slot)).map_err(|_| Error::Index)?;
                *session.read().skills.get(at).ok_or(Error::Index)?
            };
            if toggled {
                let exp = exp(session);
                if element(&exp, index)? + experience >= CAP {
                    experience -= element(&exp, index)? + experience - CAP;
                }
                let player = self::player(session)?;
                h.add_experience(&player, slot, experience)?;
                add_experience(h, session, slot, experience * modifier)?;
                return Ok(true);
            }
        }
    }
    Ok(false)
}
