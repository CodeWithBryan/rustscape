//! Interface buttons in the live world: `PacketProcessor`'s IfAction and
//! CloseIface bodies (`world::interface_packets`) over the player's
//! interface manager, its locks and its zone monitor, with the
//! InterfaceListeners registry (`world::interface_listeners`) and the
//! component plugins the original registers for the player tabs:
//!
//! - `CombatTabInterface` (ComponentPlugin of interface 92 and every weapon
//!   interface): the weapon tab's attack styles through `WeaponInterface`,
//!   the retaliate and special bar toggles in one-tick world pulses.
//! - `EmoteTabInterface` (listener of 464): `Emotes.handle`.
//! - `MusicTabInterface` (listener of 187): play, loop and the admin unlock.
//!
//! The listener and plugin bodies are the content classes' few lines,
//! written over the ported libraries (they are not differentially tested
//! themselves). Everything else a button reaches without a port is a named
//! answer or a refusal.
use super::{
    P,
    host::{Live, fail},
    tabs::Tabs,
};
use crate::{
    pulses,
    world::{
        action_locks::{self, ActionLocks},
        combat_swing::WeaponInterfaces,
        component_objects::{NativeObject, Plugin},
        driver,
        interface_listeners::{self, Button, Callback},
        interface_packets::{self, Action, LoginPulse},
        interfaces::{self, Handle},
        player_tabs::{self, emotes as tab_emotes},
        player_walk::NodeHandle,
        zones::Monitor,
    },
};
use rs09_protocol::game::updates;

/// `CombatTabInterface`, the ComponentPlugin of the weapon interfaces.
pub struct CombatTab;
impl NativeObject for CombatTab {}

/// The InterfaceListeners callbacks of the tab content classes.
#[derive(Clone, Copy)]
pub enum TabListener {
    /// `EmoteTabInterface`: `on(EMOTES_464)`.
    Emotes,
    /// `MusicTabInterface`: `on(MUSIC_V3_187)`.
    Music,
}
impl NativeObject for TabListener {}

/// The one-tick world pulses `CombatTabInterface` submits.
pub enum TabPulse {
    Retaliate(P),
    SpecialBar(P),
}

/// Registers the tab content at start-up, as `ComponentDefinition.put` and
/// `defineInterfaceListeners` do.
pub fn register(live: &mut Live) -> Result<(), String> {
    let plugin = Plugin::new(CombatTab);
    let mut ids: Vec<i32> = WeaponInterfaces::ALL
        .iter()
        .map(|w| w.interface_id())
        .collect();
    ids.push(92);
    for id in ids {
        live.inner.registry.for_id(id).write().plugin = Some(plugin.clone());
    }
    live.listeners
        .add(464, None, Callback::button(TabListener::Emotes))
        .map_err(|e| format!("{e:?}"))?;
    live.listeners
        .add(187, None, Callback::button(TabListener::Music))
        .map_err(|e| format!("{e:?}"))?;
    Ok(())
}

fn ui<E: std::fmt::Debug>(e: E) -> interfaces::Error {
    interfaces::Error::Binding(format!("{e:?}"))
}

struct Buttons<'a> {
    live: &'a mut Live,
}

impl Buttons<'_> {
    fn p(node: &NodeHandle) -> P {
        P(node.clone())
    }
    fn tabs(&mut self, p: &NodeHandle) -> Tabs<'_> {
        let live = &mut *self.live;
        Tabs {
            inner: &mut live.inner,
            p: P(p.clone()),
            walk: Some((&mut live.walk, &mut live.deferred)),
        }
    }
}

impl interface_listeners::Context for Buttons<'_> {
    fn invoke_open(&mut self, _: Callback, _: &NodeHandle, _: &Handle) -> interfaces::Result<bool> {
        Err(interfaces::Error::Binding(
            "not ported in the live world: open listeners of content interfaces".into(),
        ))
    }
    fn invoke_button(
        &mut self,
        callback: Callback,
        player: &NodeHandle,
        _: &Handle,
        button: Button,
    ) -> interfaces::Result<bool> {
        let Some(listener) = callback.value::<TabListener>() else {
            return Err(interfaces::Error::Binding(
                "a button listener this world did not register".into(),
            ));
        };
        match *listener {
            TabListener::Emotes => {
                // Emotes.handle(player, buttonID); return true.
                self.tabs(player)
                    .emotes(|tabs, manager| tab_emotes::handle(tabs, manager, button.button))
                    .map_err(ui)?;
                Ok(true)
            }
            TabListener::Music => {
                music_button(&mut self.tabs(player), button).map_err(ui)?;
                Ok(true)
            }
        }
    }
}

/// `MusicTabInterface`'s listener body.
fn music_button(tabs: &mut Tabs<'_>, b: Button) -> player_tabs::Result<()> {
    if b.opcode != 155 {
        return Ok(());
    }
    if b.button == 11 {
        return tabs.music(|tabs, music| music.toggle_looping(tabs));
    }
    if b.button == 1 {
        let id = tabs.music(|_, music| Ok(music.unlocked.unlocked.get(b.slot).copied()))?;
        if let Some(id) = id {
            return tabs.music(|tabs, music| music.play(tabs, id));
        }
        let admin = tabs
            .inner
            .with_profile(&tabs.p, |profile| profile.details.rights())
            .map_err(|e| player_tabs::Error::Binding(format!("{e:?}")))?
            .map_err(|e| player_tabs::Error::Binding(format!("{e:?}")))?
            == crate::player::details::Rights::Administrator;
        if admin {
            // MusicEntry.getSongs().values: every song at that list index.
            let ids: Vec<i32> = tabs
                .inner
                .world
                .songs
                .songs
                .iter()
                .filter(|(_, song)| song.index == b.slot)
                .map(|(id, _)| *id)
                .collect();
            for id in ids {
                tabs.music(|tabs, music| music.unlock(tabs, id, true))?;
            }
        } else {
            tabs.inner
                .message(&tabs.p, "You have not unlocked this piece of music yet!")
                .map_err(|e| player_tabs::Error::Binding(format!("{e:?}")))?;
        }
    }
    Ok(())
}

impl interface_packets::Context for Buttons<'_> {
    type Dialogue = ();
    type Locks = ActionLocks;
    type Zone = Monitor<Live>;
    type Manager = interfaces::Manager;
    type Dispatch = P;
    fn debug(&mut self, p: &NodeHandle, text: String) -> interfaces::Result<()> {
        tab_emotes::Host::debug(&mut self.tabs(p), &text).map_err(ui)
    }
    fn dialogue(&mut self, _: &NodeHandle) -> interfaces::Result<()> {
        Ok(())
    }
    fn has_dialogue(&mut self, _: ()) -> interfaces::Result<bool> {
        // DialogueInterpreter.getDialogue(): nothing opens a dialogue in
        // this world (the interpreter and its content are not installed).
        self.live
            .inner
            .not_ported
            .default_answer("DialogueInterpreter.getDialogue (no dialogue content: null)");
        Ok(false)
    }
    fn close_dialogue(&mut self, _: ()) -> interfaces::Result<()> {
        Err(interfaces::Error::Binding(
            "not ported in the live world: DialogueInterpreter.close".into(),
        ))
    }
    fn locks(&mut self, p: &NodeHandle) -> interfaces::Result<ActionLocks> {
        self.live.inner.state.locks(p).map_err(ui)
    }
    fn component_locked(&mut self, l: ActionLocks) -> interfaces::Result<bool> {
        l.is_kind_locked(
            action_locks::Kind::Component,
            &mut self.live.inner.state,
            &mut action_locks::Clock,
        )
        .map_err(ui)
    }
    fn zone(&mut self, p: &NodeHandle) -> interfaces::Result<Monitor<Live>> {
        Ok(self.live.monitor(p))
    }
    fn click_zone(&mut self, z: Monitor<Live>, a: Action) -> interfaces::Result<bool> {
        z.click_button(self.live, a.iface, a.child, a.slot, a.item, a.opcode)
            .map_err(ui)
    }
    fn manager(&mut self, p: &NodeHandle) -> interfaces::Result<interfaces::Manager> {
        self.live.inner.current = Some(Self::p(p));
        Ok(self.live.inner.canonical(&Self::p(p)).player.manager())
    }
    fn component(&mut self, m: interfaces::Manager, id: i32) -> interfaces::Result<Option<Handle>> {
        interfaces::get_component(&mut self.live.inner, &m, id)
    }
    fn hidden(&mut self, c: &Handle) -> interfaces::Result<bool> {
        Ok(c.read().hidden)
    }
    fn plugin(&mut self, c: &Handle) -> interfaces::Result<Option<Plugin>> {
        Ok(self.live.inner.registry.plugin(c))
    }
    fn id(&mut self, c: &Handle) -> interfaces::Result<i32> {
        Ok(c.read().id)
    }
    fn button_event(&mut self, _: &NodeHandle, _: i32, _: i32) -> interfaces::Result<()> {
        self.live
            .inner
            .not_ported
            .skip("ButtonClickEvent dispatch (entity event hooks not ported)");
        Ok(())
    }
    fn run_listener(&mut self, p: &NodeHandle, c: &Handle, b: Button) -> interfaces::Result<bool> {
        let registry = self.live.listeners.clone();
        registry.run(self, Some(p), Some(c), b)
    }
    fn handle_plugin(
        &mut self,
        plugin: Plugin,
        p: &NodeHandle,
        _: &Handle,
        b: Button,
    ) -> interfaces::Result<bool> {
        if plugin.value::<CombatTab>().is_none() {
            return Err(interfaces::Error::Binding(
                "a component plugin this world did not register".into(),
            ));
        }
        combat_tab(self.live, p, b)
    }
    fn close(&mut self, m: interfaces::Manager) -> interfaces::Result<bool> {
        interfaces::close(&mut self.live.inner, &m)
    }
    fn logging_present(&mut self, p: &NodeHandle) -> interfaces::Result<bool> {
        Ok(self
            .live
            .inner
            .canonical(&Self::p(p))
            .player
            .attributes()
            .get("logging_in")
            .is_some())
    }
    fn submit_login(&mut self, _: LoginPulse) -> interfaces::Result<()> {
        Err(interfaces::Error::Binding(
            "not ported in the live world: the deferred login world configuration".into(),
        ))
    }
    fn map_present(&mut self, p: &NodeHandle) -> interfaces::Result<bool> {
        Ok(self
            .live
            .inner
            .canonical(&Self::p(p))
            .player
            .attributes()
            .get("worldMap:viewing")
            .is_some())
    }
    fn remove_map(&mut self, p: &NodeHandle) -> interfaces::Result<()> {
        self.live
            .inner
            .canonical(&Self::p(p))
            .player
            .attributes()
            .remove("worldMap:viewing");
        Ok(())
    }
    fn dispatch(&mut self, p: &NodeHandle) -> interfaces::Result<P> {
        Ok(Self::p(p))
    }
    fn resizable(&mut self, m: interfaces::Manager) -> interfaces::Result<bool> {
        let p = self
            .live
            .inner
            .current
            .clone()
            .ok_or(interfaces::Error::Null)?;
        let _ = m;
        Ok(self.live.inner.canonical(&p).client.is_resizable())
    }
    fn windows_pane(&mut self, p: P, id: i32, kind: i32) -> interfaces::Result<()> {
        let counter = self
            .live
            .inner
            .canonical(&p)
            .player
            .manager()
            .packet_count(1);
        self.live
            .inner
            .queue_native(&p, updates::windows_pane(id, kind, counter));
        Ok(())
    }
    fn unlock(&mut self, p: &NodeHandle) -> interfaces::Result<()> {
        let locks = self.live.inner.state.locks(p).map_err(ui)?;
        locks.unlock();
        Ok(())
    }
    fn remove_logging(&mut self, p: &NodeHandle) -> interfaces::Result<()> {
        self.live
            .inner
            .canonical(&Self::p(p))
            .player
            .attributes()
            .remove("logging_in");
        Ok(())
    }
    fn configure_world(&mut self, _: &NodeHandle) -> interfaces::Result<()> {
        Err(interfaces::Error::Binding(
            "not ported in the live world: the deferred login world configuration".into(),
        ))
    }
}

/// `CombatTabInterface.handle(p, component, opcode, button, slot, itemId)`.
fn combat_tab(live: &mut Live, p: &NodeHandle, b: Button) -> interfaces::Result<bool> {
    let player = P(p.clone());
    let pulse = match b.button {
        7 | 9 | 24 | 26 | 27 => Some(TabPulse::Retaliate(player)),
        11 | 10 | 8 | 85 => Some(TabPulse::SpecialBar(player)),
        _ => None,
    };
    if let Some(body) = pulse {
        // GameWorld.getPulser().submit(new Pulse(1, p) { ... }).
        let handle = crate::definitions::item::Shared::new(pulses::Pulse::new(
            1,
            Some(crate::definitions::item::Shared::new(vec![Some(p.node)])),
            pulses::Class::Other,
            "content.global.handlers.iface.tabs.CombatTabInterface$1".into(),
        ));
        live.driver
            .pulser()
            .submit(Some(handle.clone()))
            .map_err(ui)?;
        live.tab_pulses.push((handle, body));
        return Ok(true);
    }
    // default: the weapon tab's attack style.
    let mut tabs = Tabs {
        inner: &mut live.inner,
        p: P(p.clone()),
        walk: Some((&mut live.walk, &mut live.deferred)),
    };
    let present = tabs.inner.canonical(&tabs.p).weapon_tab.is_some();
    if !present {
        return Ok(false);
    }
    let handled = tabs
        .weapon(|tabs, weapon| {
            if !weapon.set_attack_style(tabs, b.button)? {
                return Ok(false);
            }
            if b.button == 4 || b.button == 5 {
                weapon.open_autocast_select(tabs)?;
            } else if tab_weapon_autocast(tabs)? {
                weapon.select_auto_spell(tabs, -1, false, true)?;
            }
            Ok(true)
        })
        .map_err(ui)?;
    Ok(handled)
}

fn tab_weapon_autocast(tabs: &mut Tabs<'_>) -> player_tabs::Result<bool> {
    use crate::world::player_tabs::weapon::Host;
    Ok(tabs.autocast()?.is_some())
}

impl Live {
    /// IfAction (`processIfAction`).
    pub fn if_action(&mut self, p: &P, a: Action) -> driver::Result<()> {
        let owner = p.0.clone();
        interface_packets::if_action(&mut Buttons { live: self }, &owner, a).map_err(fail)?;
        self.inner.drain(p);
        Ok(())
    }
    /// CloseIface (`processCloseIface`).
    pub fn close_iface(&mut self, p: &P) -> driver::Result<()> {
        let owner = p.0.clone();
        interface_packets::close_iface(&mut Buttons { live: self }, &owner).map_err(fail)?;
        self.inner.drain(p);
        Ok(())
    }
    /// A tab pulse's body: `true` when it finished (all of them run once).
    pub fn tab_pulse_body(&mut self, at: usize) -> driver::Result<bool> {
        let (_, body) = self.tab_pulses.remove(at);
        let p = match &body {
            TabPulse::Retaliate(p) | TabPulse::SpecialBar(p) => p.clone(),
        };
        if !self.inner.players.contains_key(&p.node()) {
            return Ok(true);
        }
        let settings = self.inner.canonical(&p).player.settings();
        let mut ctx = super::inner::SettingsContext {
            inner: &mut self.inner,
            p: p.clone(),
        };
        match body {
            TabPulse::Retaliate(_) => {
                // p.getSettings().toggleRetaliating().
                settings.toggle_retaliating(&mut ctx).map_err(fail)?;
            }
            TabPulse::SpecialBar(_) => {
                let special_bar = self
                    .inner
                    .canonical(&p)
                    .weapon_tab
                    .as_ref()
                    .is_some_and(|w| w.is_special_bar());
                if special_bar {
                    settings
                        .toggle_special_bar(&mut super::inner::SettingsContext {
                            inner: &mut self.inner,
                            p: p.clone(),
                        })
                        .map_err(fail)?;
                    if settings.fields.read().special_toggled {
                        return Err(fail(
                            "not ported in the live world: special attack handlers (instant_spec)",
                        ));
                    }
                }
            }
        }
        self.inner.drain(&p);
        Ok(true)
    }
}
