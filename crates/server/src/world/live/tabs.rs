//! The player tab systems of `world::player_tabs` installed for live
//! players: `WeaponInterface`, `LevelUp.sendFlashingIcons`, `EmoteManager`
//! and `Emotes`, `MusicPlayer` and `QuestRepository.syncronizeTab`.
//!
//! [`Tabs`] is the libraries' host for one canonical player, the shape of
//! the world-login test's binding (`tests/world_login.rs`): packets through
//! the session queue and the interface manager's counter, varps through the
//! canonical variable store, Settings, Properties, persisted state, the
//! attribute store, the equipment container and the animator of that
//! player. A call into a system without a port is a named answer
//! ([`super::not_ported`]) or a refusal; none is a fabricated success.
use super::{
    P,
    inner::{Inner, SettingsContext},
    walk::{Deferred, Walk, WalkHost},
};
use crate::{
    definitions::property::Property,
    player::{
        owned::profile::Profile, persisted::emotes::Manager as Emotes, save, settings::Settings,
    },
    world::{
        action_locks,
        animation::{Animation, Graphics},
        interfaces,
        player_tabs::{
            self, Value, emotes as tab_emotes, level_up, music as tab_music, quests,
            weapon as tab_weapon,
        },
    },
};
use rs09_protocol::{
    buffer::Writer,
    game::{Frame, updates},
};

fn binding<E: std::fmt::Debug>(e: E) -> player_tabs::Error {
    player_tabs::Error::Binding(format!("{e:?}"))
}
fn refused<T>(what: &str) -> player_tabs::Result<T> {
    Err(player_tabs::Error::Binding(format!(
        "not ported in the live world: {what}"
    )))
}

/// MusicPacket, as the original writes it: opcode 4 with `putLEShortA(id)`,
/// or for a jingle opcode 208 with `putTri(255)` and `putLEShort(id)`.
pub fn music_frame(id: i32, secondary: bool) -> Frame {
    let mut w = Writer::new();
    if secondary {
        w.p1(0);
        w.p1(0);
        w.p1(255);
        w.ip2(id);
        Frame {
            opcode: 208,
            payload: w.into_vec(),
        }
    } else {
        w.p1(id.wrapping_add(128));
        w.p1(id >> 8);
        Frame {
            opcode: 4,
            payload: w.into_vec(),
        }
    }
}

/// The host of `world::player_tabs` for one live player. `walk` is the
/// scene, the animation definitions and the walk services' deferred calls,
/// present when the caller holds the whole live world (button handlers);
/// the login's interface context does not, and nothing it reaches needs them.
pub struct Tabs<'a> {
    pub inner: &'a mut Inner,
    pub p: P,
    pub walk: Option<(&'a mut Walk, &'a mut Vec<Deferred>)>,
}

impl<'a> Tabs<'a> {
    pub fn new(inner: &'a mut Inner, p: P) -> Self {
        Self {
            inner,
            p,
            walk: None,
        }
    }

    /// Runs a WeaponInterface member on the player's native extension
    /// object (`player.getExtension(WeaponInterface.class)`); taken out while
    /// it runs, so a nested call finds none (null).
    pub fn weapon<T>(
        &mut self,
        f: impl FnOnce(&mut Tabs<'_>, &mut tab_weapon::WeaponInterface) -> player_tabs::Result<T>,
    ) -> player_tabs::Result<T> {
        let mut weapon = self
            .inner
            .slot_mut(&self.p)
            .canonical
            .weapon_tab
            .take()
            .ok_or(player_tabs::Error::Null)?;
        let result = f(self, &mut weapon);
        self.inner.slot_mut(&self.p).canonical.weapon_tab = Some(weapon);
        result
    }

    /// Runs an EmoteManager member on the profile's persisted emotes; no
    /// profile lock is held while it runs.
    pub fn emotes<T>(
        &mut self,
        f: impl FnOnce(&mut Tabs<'_>, &mut Emotes) -> player_tabs::Result<T>,
    ) -> player_tabs::Result<T> {
        let mut manager = self.profile(|profile| profile.persisted.emotes.clone())?;
        let result = f(self, &mut manager);
        self.profile(|profile| profile.persisted.emotes = manager)?;
        result
    }

    /// Runs a MusicPlayer member on the profile's persisted songs, the
    /// player's playback fields and the world's song table.
    pub fn music<T>(
        &mut self,
        f: impl FnOnce(&mut Tabs<'_>, &mut tab_music::MusicPlayer<'_>) -> player_tabs::Result<T>,
    ) -> player_tabs::Result<T> {
        let mut unlocked = self.profile(|profile| profile.persisted.music.clone())?;
        let mut playback = self.inner.canonical(&self.p).music.clone();
        let songs = self.inner.world.songs.clone();
        let result = {
            let mut player = tab_music::MusicPlayer {
                unlocked: &mut unlocked,
                playback: &mut playback,
                songs: &songs,
            };
            f(self, &mut player)
        };
        self.profile(|profile| profile.persisted.music = unlocked)?;
        self.inner.slot_mut(&self.p).canonical.music = playback;
        result
    }

    /// `QuestRepository.syncronizeTab(player)` over the profile's quest
    /// stages and the world's quest registry.
    pub fn quest_tab(&mut self) -> player_tabs::Result<()> {
        let repository = self.profile(|profile| profile.persisted.quests.clone())?;
        let registry = self.inner.world.quests.clone();
        quests::syncronize_tab(self, &repository, &registry)
    }

    fn varp(&mut self, id: i32, value: i32, save: bool) -> player_tabs::Result<()> {
        self.inner
            .variables(&self.p, |ctx, owner| {
                crate::player::variables::set_varp(ctx, Some(owner), id, value, save)
            })
            .map_err(binding)
    }
    fn get_attribute(&mut self, key: &str, default: Value) -> player_tabs::Result<Value> {
        let attributes = self.inner.canonical(&self.p).player.attributes();
        match default {
            Value::Bool(d) => attributes
                .boolean(key, d)
                .map(Value::Bool)
                .map_err(|_| player_tabs::Error::Cast),
            Value::Int(d) => attributes
                .integer(key, d)
                .map(Value::Int)
                .map_err(|_| player_tabs::Error::Cast),
            Value::Component(_) => refused("a Component-valued attribute default"),
        }
    }
    fn set_attribute(&mut self, key: &str, value: Value) -> player_tabs::Result<()> {
        let attributes = self.inner.canonical(&self.p).player.attributes();
        match value {
            Value::Bool(v) => attributes.set_boolean(key, v),
            Value::Int(v) => attributes.set_integer(key, v),
            Value::Component(_) => return refused("a Component-valued attribute"),
        }
        Ok(())
    }
    fn remove_attribute(&mut self, key: &str) {
        self.inner
            .canonical(&self.p)
            .player
            .attributes()
            .remove(key);
    }
    fn message(&mut self, text: &str) -> player_tabs::Result<()> {
        self.inner.message(&self.p, text).map_err(binding)
    }
    fn send_string(&mut self, text: &str, interface: i32, child: i32) -> player_tabs::Result<()> {
        self.inner
            .text(&self.p, text, interface, child)
            .map_err(binding)
    }
    fn settings(&self) -> Settings {
        self.inner.canonical(&self.p).player.settings()
    }
    fn profile<T>(&self, f: impl FnOnce(&mut Profile) -> T) -> player_tabs::Result<T> {
        self.inner.with_profile(&self.p, f).map_err(binding)
    }
    fn manager(&mut self) -> interfaces::Manager {
        self.inner.current = Some(self.p.clone());
        self.inner.canonical(&self.p).player.manager()
    }
    fn equipment_id(&mut self, slot: i32) -> Option<i32> {
        let item = self.inner.canonical(&self.p).player.equipment().get(slot);
        item.map(|i| i.read().id())
    }
    /// `item.getDefinition().getConfiguration(key)`: the stored property, or
    /// none when the key is absent (or stored as null).
    // The binding drops the definition's read guard before the definition.
    #[allow(clippy::let_and_return)]
    fn item_property(&mut self, id: i32, key: &str) -> Option<Property> {
        let definition = self.inner.items.for_id(id);
        let property = definition
            .read()
            .handlers
            .get(key)
            .filter(|p| !p.is_null())
            .cloned();
        property
    }
    /// `Player.isDebug()`: an administrator with the "debug" attribute.
    fn is_debug(&mut self) -> player_tabs::Result<bool> {
        let admin = self
            .profile(|profile| profile.details.rights())?
            .map_err(binding)?
            == crate::player::details::Rights::Administrator;
        if !admin {
            return Ok(false);
        }
        self.get_attribute("debug", Value::Bool(false))?.boolean()
    }
    /// `player.getLocks()`: the canonical actor's ActionLocks.
    fn locks(&mut self) -> player_tabs::Result<action_locks::ActionLocks> {
        let owner = self.inner.canonical(&self.p).owner.clone();
        self.inner.state.locks(&owner).map_err(binding)
    }
    fn key(key: &str) -> action_locks::Key {
        Some(key.encode_utf16().collect())
    }
    /// The animator over the player's masks and the animation definitions.
    fn animate(
        &mut self,
        animation: Option<Animation>,
        graphics: Option<Graphics>,
    ) -> player_tabs::Result<()> {
        let animator = self.inner.canonical(&self.p).animator.clone();
        let Some((walk, _)) = self.walk.as_mut() else {
            return refused("the animator outside a world call");
        };
        animator
            .animate(&mut walk.animations, animation, graphics)
            .map(|_| ())
            .map_err(binding)
    }
    fn duration(&mut self, id: i32) -> player_tabs::Result<i32> {
        let Some((walk, _)) = self.walk.as_mut() else {
            return refused("animation definitions outside a world call");
        };
        Ok(Animation::new(id).duration(&mut walk.animations))
    }
    /// `player.getPulseManager().clear()` on the canonical actor's manager.
    fn clear_pulses(&mut self) -> player_tabs::Result<()> {
        let node = self.p.node();
        let Some((walk, deferred)) = self.walk.as_mut() else {
            return refused("the pulse manager outside a world call");
        };
        let Inner {
            state,
            frames,
            players,
            not_ported,
            npcs,
            generators,
            ..
        } = &mut *self.inner;
        let mut lines = Vec::new();
        let mut host = WalkHost {
            walk,
            frames,
            players,
            not_ported,
            deferred,
            log: &mut lines,
            npcs,
            random: generators,
        };
        let result = state.context(&mut host).clear_pulses(node).map_err(binding);
        for line in lines {
            self.inner.log_line(line);
        }
        self.inner.drain(&self.p);
        result
    }
}

// ---------------------------------------------------- WeaponInterface

impl tab_weapon::Host for Tabs<'_> {
    type Spell = save::Autocast;
    fn add_extension(&mut self) -> player_tabs::Result<()> {
        // `player.addExtension(WeaponInterface.class, this)`: the caller
        // stores the object it constructs as the player's weapon tab.
        Ok(())
    }
    fn set_tab(&mut self, slot: usize, component: &interfaces::Handle) -> player_tabs::Result<()> {
        let manager = self.manager();
        let tabs = manager
            .fields
            .read()
            .tabs
            .clone()
            .ok_or(player_tabs::Error::Null)?;
        let mut tabs = tabs.write();
        *tabs.get_mut(slot).ok_or(player_tabs::Error::Index)? = Some(component.clone());
        Ok(())
    }
    fn equipment(&mut self, slot: i32) -> player_tabs::Result<Option<i32>> {
        Ok(self.equipment_id(slot))
    }
    fn equipment_new(&mut self, slot: i32) -> player_tabs::Result<i32> {
        Ok(self.equipment_id(slot).unwrap_or(0))
    }
    fn item_name(&mut self, id: i32) -> Option<String> {
        self.inner.items.for_id(id).read().name.clone()
    }
    fn item_config_int(&mut self, id: i32, key: &str, default: i32) -> player_tabs::Result<i32> {
        match self.item_property(id, key) {
            None => Ok(default),
            Some(Property::Int(v)) => Ok(v),
            Some(_) => Err(player_tabs::Error::Cast),
        }
    }
    fn item_config_bool(&mut self, id: i32, key: &str, default: bool) -> player_tabs::Result<bool> {
        match self.item_property(id, key) {
            None => Ok(default),
            Some(Property::Bool(v)) => Ok(v),
            Some(_) => Err(player_tabs::Error::Cast),
        }
    }
    fn item_config_animations(
        &mut self,
        id: i32,
        key: &str,
    ) -> player_tabs::Result<Option<Vec<tab_weapon::Animation>>> {
        match self.item_property(id, key) {
            None => Ok(None),
            Some(Property::Animations(list)) => {
                let list = list.read();
                let mut out = Vec::with_capacity(list.len());
                for animation in list.iter() {
                    let Some(animation) = animation else {
                        return refused("a null element of a weapon's attack_anims");
                    };
                    let a = animation.read();
                    let Some(priority) = a.priority else {
                        return refused("an attack animation without a priority");
                    };
                    out.push(tab_weapon::Animation {
                        id: a.id,
                        delay: a.delay,
                        priority,
                    });
                }
                Ok(Some(out))
            }
            Some(_) => Err(player_tabs::Error::Cast),
        }
    }
    fn component_definition(&mut self, id: i32) -> interfaces::Def {
        self.inner.registry.for_id(id)
    }
    fn create_component(&mut self, id: i32) -> interfaces::Handle {
        self.inner.registry.component(id)
    }
    fn resizable(&mut self) -> player_tabs::Result<bool> {
        Ok(self.inner.canonical(&self.p).client.is_resizable())
    }
    fn send_interface(
        &mut self,
        pane: i32,
        child: i32,
        id: i32,
        walkable: bool,
    ) -> player_tabs::Result<()> {
        let manager = self.manager();
        interfaces::send_interface(&mut *self.inner, &manager, pane, child, id, walkable)
            .map_err(binding)
    }
    fn interface_config(
        &mut self,
        interface: i32,
        child: i32,
        hide: bool,
    ) -> player_tabs::Result<()> {
        let manager = self.manager();
        interfaces::Context::interface_config(&mut *self.inner, &manager, interface, child, hide)
            .map_err(binding)
    }
    fn send_string(&mut self, text: &str, interface: i32, child: i32) -> player_tabs::Result<()> {
        Tabs::send_string(self, text, interface, child)
    }
    fn message(&mut self, text: &str) -> player_tabs::Result<()> {
        Tabs::message(self, text)
    }
    fn attack_style_index(&mut self) -> player_tabs::Result<i32> {
        Ok(self.settings().fields.read().attack_style_index)
    }
    fn toggle_attack_style_index(&mut self, index: i32) -> player_tabs::Result<()> {
        let settings = self.settings();
        settings
            .toggle_attack_style_index(
                &mut SettingsContext {
                    inner: &mut *self.inner,
                    p: self.p.clone(),
                },
                index,
            )
            .map_err(binding)
    }
    fn special_toggled(&mut self) -> player_tabs::Result<bool> {
        Ok(self.settings().fields.read().special_toggled)
    }
    fn toggle_special_bar(&mut self) -> player_tabs::Result<()> {
        let settings = self.settings();
        settings
            .toggle_special_bar(&mut SettingsContext {
                inner: &mut *self.inner,
                p: self.p.clone(),
            })
            .map_err(binding)
    }
    fn attack_style(
        &mut self,
    ) -> player_tabs::Result<Option<crate::world::combat_swing::AttackStyle>> {
        self.profile(|profile| profile.properties.attack_style)
    }
    fn set_attack_style(
        &mut self,
        style: crate::world::combat_swing::AttackStyle,
    ) -> player_tabs::Result<()> {
        self.profile(|profile| profile.properties.attack_style = Some(style))
    }
    fn set_attack_animation(
        &mut self,
        animation: tab_weapon::Animation,
    ) -> player_tabs::Result<()> {
        let animation = Animation::with(animation.id, animation.delay, Some(animation.priority));
        self.profile(|profile| profile.properties.attack_animation = Some(animation))
    }
    fn update_style(&mut self) -> player_tabs::Result<()> {
        // CombatPulse.updateStyle sets the combat pulse's style field; the
        // combat pulse is not installed and nothing reads it here.
        self.inner
            .not_ported
            .skip("CombatPulse.updateStyle (combat pulse not installed)");
        Ok(())
    }
    fn is_npc(&mut self) -> player_tabs::Result<bool> {
        let masks = self.inner.canonical(&self.p).player.masks();
        let npc = masks.body().read().npc_id != -1;
        Ok(npc)
    }
    fn spell_book(&mut self) -> player_tabs::Result<i32> {
        self.profile(|profile| profile.spell_book)
    }
    fn autocast(&mut self) -> player_tabs::Result<Option<save::Autocast>> {
        self.profile(|profile| profile.persisted.spells.autocast)
    }
    fn spell_id(&mut self, spell: &save::Autocast) -> i32 {
        spell.spell_id
    }
    fn set_autocast(&mut self, spell: Option<&save::Autocast>) -> player_tabs::Result<()> {
        let spell = spell.copied();
        self.profile(|profile| profile.persisted.spells.autocast = spell)
    }
    fn spell_lookup(
        &mut self,
        _: crate::world::spells::SpellBook,
        _: i32,
    ) -> player_tabs::Result<tab_weapon::Lookup<save::Autocast>> {
        refused("SpellBook.getSpell for autocast (spells are not installed)")
    }
    fn get_attribute(&mut self, key: &str, default: Value) -> player_tabs::Result<Value> {
        Tabs::get_attribute(self, key, default)
    }
    fn set_attribute(&mut self, key: &str, value: Value) -> player_tabs::Result<()> {
        Tabs::set_attribute(self, key, value)
    }
    fn remove_attribute(&mut self, key: &str) -> player_tabs::Result<()> {
        Tabs::remove_attribute(self, key);
        Ok(())
    }
    fn varp(&mut self, id: i32, value: i32, save: bool) -> player_tabs::Result<()> {
        Tabs::varp(self, id, value, save)
    }
    fn open_tab(&mut self, _: &interfaces::Handle) -> player_tabs::Result<()> {
        refused("the autocast selection tab (spells are not installed)")
    }
}

// ---------------------------------------------------- LevelUp

impl level_up::Host for Tabs<'_> {
    fn get_attribute(&mut self, key: &str, default: Value) -> player_tabs::Result<Value> {
        Tabs::get_attribute(self, key, default)
    }
    fn set_attribute(&mut self, key: &str, value: Value) -> player_tabs::Result<()> {
        Tabs::set_attribute(self, key, value)
    }
    fn static_level(&mut self, _: i32) -> player_tabs::Result<i32> {
        refused("LevelUp.levelup (no experience is gained in this world)")
    }
    fn increment_prayer_points(&mut self, _: f64) -> player_tabs::Result<()> {
        refused("LevelUp.levelup (no experience is gained in this world)")
    }
    fn total_level(&mut self) -> player_tabs::Result<i32> {
        refused("LevelUp milestones (no experience is gained in this world)")
    }
    fn combat_milestone(&mut self) -> player_tabs::Result<i32> {
        refused("LevelUp milestones (no experience is gained in this world)")
    }
    fn set_combat_milestone(&mut self, _: i32) -> player_tabs::Result<()> {
        refused("LevelUp milestones (no experience is gained in this world)")
    }
    fn skill_milestone(&mut self) -> player_tabs::Result<i32> {
        refused("LevelUp milestones (no experience is gained in this world)")
    }
    fn set_skill_milestone(&mut self, _: i32) -> player_tabs::Result<()> {
        refused("LevelUp milestones (no experience is gained in this world)")
    }
    fn combat_level(&mut self) -> player_tabs::Result<i32> {
        self.profile(|profile| profile.properties.combat_level)
    }
    fn jingle(&mut self, _: i32) -> player_tabs::Result<()> {
        refused("level-up jingles (no experience is gained in this world)")
    }
    fn varp(&mut self, id: i32, value: i32, save: bool) -> player_tabs::Result<()> {
        Tabs::varp(self, id, value, save)
    }
    fn graphic(&mut self, _: i32) -> player_tabs::Result<()> {
        refused("level-up graphics (no experience is gained in this world)")
    }
    fn send_string(&mut self, text: &str, interface: i32, child: i32) -> player_tabs::Result<()> {
        Tabs::send_string(self, text, interface, child)
    }
    fn message(&mut self, text: &str) -> player_tabs::Result<()> {
        Tabs::message(self, text)
    }
    fn is_artificial(&mut self) -> bool {
        false
    }
    fn username(&mut self) -> String {
        self.inner.slot(&self.p).name.clone()
    }
    fn news(&mut self, _: &str, _: Option<(i32, &str)>) -> player_tabs::Result<()> {
        refused("Repository.sendNews (level-up news)")
    }
    fn skillcape_trim(&mut self) -> player_tabs::Result<()> {
        refused("Skillcape.trim (content)")
    }
    fn emote_unlock(&mut self, emote: tab_emotes::Emote) -> player_tabs::Result<bool> {
        self.emotes(|tabs, manager| tab_emotes::unlock(tabs, manager, emote))
    }
    fn open_chatbox(&mut self, id: i32) -> player_tabs::Result<()> {
        let manager = self.manager();
        let component = self.inner.registry.component(id);
        interfaces::open_chatbox(&mut *self.inner, &manager, &component).map_err(binding)
    }
}

// ---------------------------------------------------- EmoteManager, Emotes

impl tab_emotes::Host for Tabs<'_> {
    fn varp(&mut self, id: i32, value: i32, save: bool) -> player_tabs::Result<()> {
        Tabs::varp(self, id, value, save)
    }
    fn is_locked(&mut self, key: &str) -> player_tabs::Result<bool> {
        let locks = self.locks()?;
        locks
            .is_locked(
                &Self::key(key),
                &mut self.inner.state,
                &mut action_locks::Clock,
            )
            .map_err(binding)
    }
    fn lock(&mut self, key: &str, ticks: i32) -> player_tabs::Result<()> {
        let locks = self.locks()?;
        locks
            .lock_custom(
                Self::key(key),
                ticks,
                None,
                &mut self.inner.state,
                &mut action_locks::Clock,
            )
            .map(|_| ())
            .map_err(binding)
    }
    fn lock_all(&mut self, ticks: i32) -> player_tabs::Result<()> {
        let locks = self.locks()?;
        locks
            .lock(ticks, &mut self.inner.state, &mut action_locks::Clock)
            .map_err(binding)
    }
    fn is_attacking(&mut self) -> player_tabs::Result<bool> {
        // CombatPulse.isAttacking: the combat pulse is not installed, so no
        // player is ever attacking.
        self.inner
            .not_ported
            .default_answer("CombatPulse.isAttacking (combat pulse not installed: false)");
        Ok(false)
    }
    fn in_combat(&mut self) -> player_tabs::Result<bool> {
        // Entity.inCombat: the "combat-time" attribute against the clock.
        let time = match self
            .inner
            .canonical(&self.p)
            .player
            .attributes()
            .get("combat-time")
        {
            None => 0,
            Some(crate::player::attributes::AttributeValue::Long(v)) => v,
            Some(_) => return Err(player_tabs::Error::Cast),
        };
        Ok(time > super::now_millis())
    }
    fn debug(&mut self, text: &str) -> player_tabs::Result<()> {
        // Player.debug: a message for a player in debug mode.
        if self.is_debug()? {
            return Tabs::message(self, text);
        }
        Ok(())
    }
    fn rights(&mut self) -> player_tabs::Result<i32> {
        Ok(self
            .profile(|profile| profile.details.rights())?
            .map_err(binding)?
            .ordinal())
    }
    fn pulse_clear(&mut self) -> player_tabs::Result<()> {
        self.clear_pulses()
    }
    fn dialogue(&mut self, _: &[&str]) -> player_tabs::Result<()> {
        refused("DialogueInterpreter.sendDialogue (the dialogue interpreter is not installed)")
    }
    fn varrock_diary_complete(&mut self, level: i32, index: i32) -> player_tabs::Result<bool> {
        // AchievementDiary.isComplete(level, index) on the persisted
        // Varrock diary (DiaryType ordinal 1).
        self.profile(|profile| {
            let diary = profile.persisted.diaries.diaries.get(1)?;
            let level = diary.completed.get(usize::try_from(level).ok()?)?;
            level.get(usize::try_from(index).ok()?).copied()
        })?
        .ok_or(player_tabs::Error::Index)
    }
    fn varrock_diary_update(&mut self, _: i32, _: i32, _: bool) -> player_tabs::Result<()> {
        refused("AchievementDiary.updateTask (diary task updates are not ported)")
    }
    fn get_attribute(&mut self, key: &str, default: Value) -> player_tabs::Result<Value> {
        Tabs::get_attribute(self, key, default)
    }
    fn set_attribute(&mut self, key: &str, value: Value) -> player_tabs::Result<()> {
        Tabs::set_attribute(self, key, value)
    }
    fn equipment(&mut self, slot: i32) -> player_tabs::Result<Option<i32>> {
        Ok(self.equipment_id(slot))
    }
    fn animate(&mut self, animation: i32, graphics: Option<i32>) -> player_tabs::Result<()> {
        // Animation.create(id) and `new Graphics(id)`.
        Tabs::animate(
            self,
            Some(Animation::new(animation)),
            graphics.map(|id| Graphics::new(id, 0, 0)),
        )
    }
    fn animation_duration(&mut self, animation: i32) -> player_tabs::Result<i32> {
        self.duration(animation)
    }
    fn graphic(&mut self, id: i32) -> player_tabs::Result<()> {
        // packetDispatch.sendGraphic(id): the animator's graphics.
        Tabs::animate(self, None, Some(Graphics::new(id, 0, 0)))
    }
    fn animation(&mut self, id: i32) -> player_tabs::Result<()> {
        // packetDispatch.sendAnimation(id): the animator's animation.
        Tabs::animate(self, Some(Animation::new(id)), None)
    }
    fn message(&mut self, text: &str) -> player_tabs::Result<()> {
        Tabs::message(self, text)
    }
    fn jingle(&mut self, id: i32) -> player_tabs::Result<()> {
        // ContentAPI.playJingle: the secondary MusicPacket.
        self.inner.queue_native(&self.p, music_frame(id, true));
        Ok(())
    }
    fn content_play(&mut self, _: tab_emotes::Emote) -> player_tabs::Result<()> {
        refused("the SALUTE, GOBLIN_BOW and GIVE_THANKS emotes (content)")
    }
}

// ---------------------------------------------------- MusicPlayer

impl tab_music::Host for Tabs<'_> {
    fn varp(&mut self, id: i32, value: i32, save: bool) -> player_tabs::Result<()> {
        Tabs::varp(self, id, value, save)
    }
    fn access_mask(
        &mut self,
        hash: i32,
        child: i32,
        interface: i32,
        offset: i32,
        length: i32,
    ) -> player_tabs::Result<()> {
        // AccessMask (165): the interface manager's counter, then the fields.
        let counter = self
            .inner
            .canonical(&self.p)
            .player
            .manager()
            .packet_count(1);
        let frame = updates::interface_events(
            interface.wrapping_shl(16) | child,
            offset,
            length,
            hash,
            counter,
        );
        self.inner.queue_native(&self.p, frame);
        Ok(())
    }
    fn message(&mut self, text: &str) -> player_tabs::Result<()> {
        Tabs::message(self, text)
    }
    fn music(&mut self, id: i32, secondary: bool) -> player_tabs::Result<()> {
        self.inner.queue_native(&self.p, music_frame(id, secondary));
        Ok(())
    }
    fn send_string(&mut self, text: &str, interface: i32, child: i32) -> player_tabs::Result<()> {
        Tabs::send_string(self, text, interface, child)
    }
    fn is_debug(&mut self) -> player_tabs::Result<bool> {
        Tabs::is_debug(self)
    }
    fn ticks(&mut self) -> player_tabs::Result<i32> {
        Ok(self.inner.ticks)
    }
    fn emote_unlocked(&mut self, emote: tab_emotes::Emote) -> player_tabs::Result<bool> {
        self.profile(|profile| profile.persisted.emotes.is_unlocked(emote.ordinal()))
    }
    fn emote_unlock(&mut self, emote: tab_emotes::Emote) -> player_tabs::Result<bool> {
        self.emotes(|tabs, manager| tab_emotes::unlock(tabs, manager, emote))
    }
    fn emote_lock(&mut self, emote: tab_emotes::Emote) -> player_tabs::Result<bool> {
        self.emotes(|tabs, manager| tab_emotes::lock(tabs, manager, emote))
    }
}

// ---------------------------------------------------- QuestRepository

impl quests::Host for Tabs<'_> {
    fn varp(&mut self, id: i32, value: i32, save: bool) -> player_tabs::Result<()> {
        Tabs::varp(self, id, value, save)
    }
    fn varbit(&mut self, id: i32, value: i32, save: bool) -> player_tabs::Result<()> {
        self.inner
            .variables(&self.p, |ctx, owner| {
                crate::player::variables::set_varbit(ctx, Some(owner), id, value, save)
            })
            .map_err(binding)
    }
    fn quest_config(
        &mut self,
        _: &crate::player::persisted::quests::Quest,
        _: i32,
    ) -> player_tabs::Result<Vec<i32>> {
        // The registry is empty in this world (quest classes are content).
        refused("Quest.getConfig of a registered quest (quests are content)")
    }
    fn update_varps(
        &mut self,
        _: &crate::player::persisted::quests::Quest,
    ) -> player_tabs::Result<()> {
        refused("Quest.updateVarps (quests are content)")
    }
}
