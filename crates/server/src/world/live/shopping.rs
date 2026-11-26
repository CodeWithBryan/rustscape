//! Shops in the live world (`world::shops`, docs/shops.md): `shops::World`
//! built by `Shops.startup` at boot from `shops.json`, the 620/621
//! InterfaceListeners, the NPC "trade"/"shop" listeners with their
//! destination override in the native InteractionListeners, `Shops.tick` as a
//! tick listener (after GraveController, in the scan's order), Buy-X/Sell-X
//! through the input prompts and ShopNPC's shops.
//!
//! [`ShopHost`] is `shops::Host` over [`Inner`] (the canonical players, the
//! interface manager, packets, definitions), so the shop's open and close
//! listeners run wherever the interface manager is used. The three services
//! that need the dialogue interpreter (`sendDialogue`, `sendNPCDialogue`,
//! `sendInputDialogue`) are recorded and made by the live world right after
//! the shop call returns ([`Pending`]).
use super::{P, host::Live, inner::Inner, now_millis};
use crate::{
    definitions::item::Registry as Items,
    inventory::{
        self as inv,
        container::{Container, Event, Factory},
        container_listener::Handle as Listener,
        item::Handle as Item,
    },
    player::ironman,
    world::drop_tables::LogLevel,
    world::{
        interface_listeners::{self, Button, Callback},
        interfaces::{self, CloseEvent, Handle as Component, Manager, Plugin},
        player_walk::NodeHandle,
        shops::{
            self, Error, Quest, Result, ShopEvent,
            controller::{self, NpcListener},
            shop::{AmountScript, Shop},
        },
    },
};
use rs09_protocol::game::Frame;
use std::{collections::HashMap, sync::Arc};

/// The shop state the interface context reaches (in [`Inner`]).
#[derive(Default)]
pub struct ShopState {
    /// The companion tables (`Shops.shopsById`, ...), made at boot.
    pub world: Option<shops::World>,
    /// `getServerConfig().getBoolean("world.personalized_shops", false)`:
    /// the configured value, if the configuration has one.
    pub personalized: Option<bool>,
    /// `ServerConstants.CONFIG_PATH`.
    pub config_path: Option<String>,
    /// `ServerConstants.PLAYER_STOCK_CLEAR_INTERVAL`.
    pub clear_interval: i32,
    /// `ServerConstants.PLAYER_STOCK_RECIRCULATE`.
    pub recirculate: bool,
    /// The "shop", "shop-cont" and "shop-main" attributes, by player node.
    pub open: HashMap<usize, Open>,
}
#[derive(Default)]
pub struct Open {
    shop: Option<Shop>,
    container: Option<Container>,
    main: Option<bool>,
}

/// A service that needs the dialogue interpreter, made after the shop call.
pub enum Pending {
    Dialogue(P, String),
    NpcDialogue(P, i32, String, &'static str),
    Input(P, String, AmountScript),
}

/// The NPC "trade"/"shop" listeners and their destination override in the
/// InteractionListeners registry.
#[derive(Clone, Copy)]
pub struct ShopListener(pub NpcListener);
impl crate::world::component_objects::NativeObject for ShopListener {}
pub struct ShopDestination;
impl crate::world::component_objects::NativeObject for ShopDestination {}
const SHOPS: &str = "core.game.shops.Shops";

pub struct ShopHost<'a> {
    pub inner: &'a mut Inner,
    pub pending: Vec<Pending>,
    /// The NPC of an NPC listener or destination call, for its attributes.
    pub npc: Option<usize>,
}

fn service(e: impl std::fmt::Debug) -> Error {
    Error::Service(format!("{e:?}"))
}
fn refuse<T>(what: &str) -> Result<T> {
    Err(Error::Service(format!(
        "not ported in the live world: {what}"
    )))
}

impl<'a> ShopHost<'a> {
    pub fn new(inner: &'a mut Inner) -> Self {
        Self {
            inner,
            pending: Vec::new(),
            npc: None,
        }
    }
    fn p(&self, owner: &NodeHandle) -> Result<P> {
        if self.inner.players.contains_key(&owner.node) {
            Ok(P(owner.clone()))
        } else {
            Err(Error::ClassCast)
        }
    }
    /// An interface manager service with the player as the context's current.
    fn ui<T>(
        &mut self,
        owner: &NodeHandle,
        f: impl FnOnce(&mut Inner) -> interfaces::Result<T>,
    ) -> Result<T> {
        let p = self.p(owner)?;
        let previous = self.inner.current.replace(p);
        let result = f(self.inner);
        self.inner.current = previous;
        result.map_err(service)
    }
}

impl shops::Host for ShopHost<'_> {
    fn world(&mut self) -> Result<shops::World> {
        self.inner
            .shops
            .world
            .clone()
            .ok_or_else(|| Error::Service("the shops are not started".into()))
    }
    fn ticks(&mut self) -> Result<i32> {
        Ok(self.inner.ticks)
    }
    fn definitions(&mut self) -> Result<Arc<Items>> {
        Ok(self.inner.items.clone())
    }
    fn factory(&mut self) -> Result<Factory> {
        Ok(Factory {
            null_item: self.inner.factory.null_item.clone(),
        })
    }
    fn config_bool(&mut self, key: &str, default: bool) -> Result<bool> {
        if key == shops::PERSONALIZED_SHOPS {
            return Ok(self.inner.shops.personalized.unwrap_or(default));
        }
        refuse(&format!("getServerConfig().getBoolean({key})"))
    }
    fn server_name(&mut self) -> Result<String> {
        Ok(self.inner.constants.server_name.clone())
    }
    fn config_path(&mut self) -> Result<Option<String>> {
        Ok(self.inner.shops.config_path.clone())
    }
    fn stock_clear_interval(&mut self) -> Result<i32> {
        Ok(self.inner.shops.clear_interval)
    }
    fn stock_recirculate(&mut self) -> Result<bool> {
        Ok(self.inner.shops.recirculate)
    }
    fn read_file(&mut self, path: &str) -> Result<String> {
        std::fs::read_to_string(path).map_err(|_| Error::File)
    }
    fn log(&mut self, level: LogLevel, message: &str) -> Result<()> {
        self.inner.log_line(format!("[Shops] {level:?} {message}"));
        Ok(())
    }
    fn interface_listeners(&mut self) -> Result<interface_listeners::Registry> {
        Ok(self.inner.interface_listeners.clone())
    }
    fn uid(&mut self, p: &NodeHandle) -> Result<i32> {
        let p = self.p(p)?;
        Ok(self.inner.slot(&p).uid)
    }
    fn username(&mut self, p: &NodeHandle) -> Result<String> {
        let p = self.p(p)?;
        Ok(self.inner.slot(&p).name.clone())
    }
    fn inventory(&mut self, p: &NodeHandle) -> Result<Container> {
        let p = self.p(p)?;
        Ok(self.inner.canonical(&p).player.inventory())
    }
    fn ironman(&mut self, p: &NodeHandle) -> Result<ironman::Manager> {
        let p = self.p(p)?;
        Ok(self.inner.canonical(&p).player.ironman())
    }
    fn interface_manager(&mut self, p: &NodeHandle) -> Result<Manager> {
        let p = self.p(p)?;
        Ok(self.inner.canonical(&p).player.manager())
    }
    fn shop_attribute(&mut self, p: &NodeHandle) -> Result<Option<Shop>> {
        Ok(self
            .inner
            .shops
            .open
            .get(&p.node)
            .and_then(|o| o.shop.clone()))
    }
    fn set_shop_attribute(&mut self, p: &NodeHandle, shop: Shop) -> Result<()> {
        self.p(p)?;
        self.inner.shops.open.entry(p.node).or_default().shop = Some(shop);
        Ok(())
    }
    fn shop_container(&mut self, p: &NodeHandle) -> Result<Option<Container>> {
        Ok(self
            .inner
            .shops
            .open
            .get(&p.node)
            .and_then(|o| o.container.clone()))
    }
    fn set_shop_container(&mut self, p: &NodeHandle, container: Container) -> Result<()> {
        self.p(p)?;
        self.inner.shops.open.entry(p.node).or_default().container = Some(container);
        Ok(())
    }
    fn shop_main(&mut self, p: &NodeHandle, default: bool) -> Result<bool> {
        Ok(self
            .inner
            .shops
            .open
            .get(&p.node)
            .and_then(|o| o.main)
            .unwrap_or(default))
    }
    fn set_shop_main(&mut self, p: &NodeHandle, main: bool) -> Result<()> {
        self.p(p)?;
        self.inner.shops.open.entry(p.node).or_default().main = Some(main);
        Ok(())
    }
    fn message(&mut self, p: &NodeHandle, text: &str) -> Result<()> {
        let p = self.p(p)?;
        self.inner.message(&p, text).map_err(service)
    }
    fn content_message(&mut self, p: &NodeHandle, text: &str) -> Result<()> {
        // ContentAPI.sendMessage: player.sendMessages(*splitLines(message, 86)).
        let p = self.p(p)?;
        let units: Vec<u16> = text.encode_utf16().collect();
        let lines = crate::world::plain_dialogue::split_lines(&units, 86).map_err(service)?;
        for line in lines {
            self.inner
                .message(&p, &String::from_utf16_lossy(&line))
                .map_err(service)?;
        }
        Ok(())
    }
    fn dialogue(&mut self, p: &NodeHandle, text: &str) -> Result<()> {
        let p = self.p(p)?;
        self.pending.push(Pending::Dialogue(p, text.into()));
        Ok(())
    }
    fn debug(&mut self, p: &NodeHandle, text: &str) -> Result<()> {
        let p = self.p(p)?;
        let debug = self
            .inner
            .canonical(&p)
            .player
            .attributes()
            .boolean("debug", false)
            .unwrap_or(false);
        if debug {
            self.inner.message(&p, text).map_err(service)?;
        }
        Ok(())
    }
    fn send(&mut self, p: &NodeHandle, frame: Frame) -> Result<()> {
        let p = self.p(p)?;
        self.inner.queue_native(&p, frame);
        Ok(())
    }
    fn examine(&mut self, id: i32) -> Result<String> {
        let definition = self.inner.items.for_id(id);
        crate::world::packet_handlers::examine::item_examine_text(&definition, |s| s.to_lowercase())
            .map_err(service)
    }
    fn shop_event(&mut self, _: &NodeHandle, _: ShopEvent, _: i32, _: i32, _: &Item) -> Result<()> {
        self.inner.not_ported.skip(
            "ItemShopPurchaseEvent/ItemShopSellEvent dispatch (entity event hooks not ported)",
        );
        Ok(())
    }
    fn view(&mut self, _: &NodeHandle, _: &Item, _: &Plugin) -> Result<Option<Item>> {
        refuse("a content ShopItemView")
    }
    fn input_dialogue(&mut self, p: &NodeHandle, prompt: &str, script: AmountScript) -> Result<()> {
        let p = self.p(p)?;
        self.pending.push(Pending::Input(p, prompt.into(), script));
        Ok(())
    }
    fn ge_offer(&mut self, _: i32, _: i32) -> Result<bool> {
        // GrandExchange.addBotOffer(id, amount) of an item the general
        // stores' player stock is about to drop: the Grand Exchange is not
        // installed, so the item is not offered (the stock is still
        // cleared, as the original does after offering).
        self.inner.not_ported.skip(
            "GrandExchange.addBotOffer from the shops' player stock clear (the Grand Exchange is not installed)",
        );
        Ok(false)
    }
    fn tanning(&mut self, _: &NodeHandle, _: i32) -> Result<()> {
        refuse("TanningProduct.open (content)")
    }
    fn quest_points(&mut self, _: &NodeHandle) -> Result<i32> {
        refuse("getQuestPoints for Siegfried Erkle's shop (quests are content)")
    }
    fn quest_stage(&mut self, _: &NodeHandle, quest: Quest) -> Result<i32> {
        refuse(&format!(
            "getQuestStage({}) for a shop (quests are content)",
            quest.name()
        ))
    }
    fn quest_complete(&mut self, _: &NodeHandle, quest: Quest) -> Result<bool> {
        refuse(&format!(
            "isQuestComplete({}) for a shop (quests are content)",
            quest.name()
        ))
    }
    fn npc_dialogue(
        &mut self,
        p: &NodeHandle,
        npc: i32,
        text: &str,
        expression: &'static str,
    ) -> Result<()> {
        let p = self.p(p)?;
        self.pending
            .push(Pending::NpcDialogue(p, npc, text.into(), expression));
        Ok(())
    }
    fn npc_facing_booth(&mut self, _: &shops::Npc) -> Result<bool> {
        // npc.getAttribute("facing_booth", false).
        let node = self.npc.ok_or(Error::Null)?;
        match self.inner.attribute(node, "facing_booth") {
            None => Ok(false),
            Some(super::combat::Attr::Bool(b)) => Ok(b),
            Some(_) => Err(Error::ClassCast),
        }
    }
    fn listener_update(
        &mut self,
        listener: Listener,
        container: &mut Container,
        event: Event,
    ) -> Result<()> {
        let Some(receiver) = listener.value::<inv::listener::Listener>() else {
            return refuse("a container listener other than the inventory's and the shop's");
        };
        let owner = receiver.owner().ok_or(Error::Null)?;
        let p = self.p(&owner)?;
        let player = self.inner.canonical(&p).player.clone();
        let inner = &mut *self.inner;
        let result = player.apply(
            &mut inner.state,
            &mut inner.frames,
            now_millis,
            |_, _, context| receiver.update_event(context, container, Some(&event)),
        );
        inner.drain(&p);
        result.map_err(Into::into)
    }
    fn listener_refresh(&mut self, listener: Listener, container: &mut Container) -> Result<()> {
        let Some(receiver) = listener.value::<inv::listener::Listener>() else {
            return refuse("a container listener other than the inventory's and the shop's");
        };
        let owner = receiver.owner().ok_or(Error::Null)?;
        let p = self.p(&owner)?;
        let player = self.inner.canonical(&p).player.clone();
        let inner = &mut *self.inner;
        let result = player.apply(
            &mut inner.state,
            &mut inner.frames,
            now_millis,
            |_, _, context| receiver.refresh(context, container),
        );
        inner.drain(&p);
        result.map_err(Into::into)
    }
    fn other_listener(
        &mut self,
        p: &NodeHandle,
        callback: Callback,
        component: &Component,
        _: Option<Button>,
    ) -> Result<bool> {
        // An open or close listener of another class reached through the
        // shop's interface context: BankInterface's runs; the global
        // handlers' stay skipped (super::inner's listener_open).
        let result = self.ui(p, |inner| {
            match inner.bank_open_listener(&callback, component) {
                Some(result) => result,
                None => {
                    inner
                        .not_ported
                        .skip("InterfaceListeners.runOpen (content interface listeners)");
                    Ok(true)
                }
            }
        });
        result
    }
    fn ui_validate(&mut self, p: &NodeHandle, m: &Manager) -> Result<()> {
        self.ui(p, |inner| interfaces::Context::validate_manager(inner, m))
    }
    fn ui_component(&mut self, p: &NodeHandle, id: i32) -> Result<Component> {
        self.ui(p, |inner| interfaces::Context::component(inner, id))
    }
    fn ui_resizable(&mut self, p: &NodeHandle) -> Result<bool> {
        self.ui(p, |inner| interfaces::Context::resizable(inner))
    }
    fn ui_log(&mut self, p: &NodeHandle, text: String) -> Result<()> {
        self.ui(p, |inner| interfaces::Context::log(inner, text))
    }
    fn ui_plugin_open(&mut self, p: &NodeHandle, plugin: Plugin, c: &Component) -> Result<()> {
        self.ui(p, |inner| {
            interfaces::Context::plugin_open(inner, plugin, c)
        })
    }
    fn ui_close_event(&mut self, p: &NodeHandle, event: CloseEvent, c: &Component) -> Result<bool> {
        self.ui(p, |inner| interfaces::Context::close_event(inner, event, c))
    }
    fn ui_dispatch_open(&mut self, p: &NodeHandle, c: Component) -> Result<()> {
        self.ui(p, |inner| interfaces::Context::dispatch_open(inner, c))
    }
    fn ui_dispatch_close(&mut self, p: &NodeHandle, c: Option<Component>) -> Result<()> {
        self.ui(p, |inner| interfaces::Context::dispatch_close(inner, c))
    }
    fn ui_set_varp(&mut self, p: &NodeHandle, id: i32, value: i32) -> Result<()> {
        self.ui(p, |inner| interfaces::Context::set_varp(inner, id, value))
    }
    fn ui_weapon_extension(&mut self, p: &NodeHandle) -> Result<Option<Component>> {
        self.ui(p, |inner| interfaces::Context::weapon_extension(inner))
    }
    fn ui_create_weapon(&mut self, p: &NodeHandle) -> Result<Component> {
        self.ui(p, |inner| interfaces::Context::create_weapon(inner))
    }
    fn ui_add_weapon(&mut self, p: &NodeHandle, c: Component) -> Result<()> {
        self.ui(p, |inner| interfaces::Context::add_weapon(inner, c))
    }
    fn ui_spellbook(&mut self, p: &NodeHandle) -> Result<i32> {
        self.ui(p, |inner| interfaces::Context::spellbook(inner))
    }
    fn ui_has_familiar(&mut self, p: &NodeHandle) -> Result<bool> {
        self.ui(p, |inner| interfaces::Context::has_familiar(inner))
    }
    fn ui_friends_component(&mut self, p: &NodeHandle) -> Result<Component> {
        self.ui(p, |inner| interfaces::Context::friends_component(inner))
    }
    fn ui_autocast(&mut self, p: &NodeHandle) -> Result<Option<usize>> {
        self.ui(p, |inner| interfaces::Context::autocast(inner))
    }
    fn ui_auto_spell_id(
        &mut self,
        p: &NodeHandle,
        weapon: &Component,
        spell: usize,
    ) -> Result<i32> {
        self.ui(p, |inner| {
            interfaces::Context::auto_spell_id(inner, weapon, spell)
        })
    }
    fn ui_select_auto_spell(&mut self, p: &NodeHandle, weapon: &Component, id: i32) -> Result<()> {
        self.ui(p, |inner| {
            interfaces::Context::select_auto_spell(inner, weapon, id)
        })
    }
    fn ui_window_mode(&mut self, p: &NodeHandle) -> Result<i32> {
        self.ui(p, |inner| interfaces::Context::window_mode(inner))
    }
    fn ui_set_window_mode(&mut self, p: &NodeHandle, mode: i32) -> Result<()> {
        self.ui(p, |inner| interfaces::Context::set_window_mode(inner, mode))
    }
    fn ui_tutorial_complete(&mut self, p: &NodeHandle) -> Result<bool> {
        self.ui(p, |inner| interfaces::Context::tutorial_complete(inner))
    }
    fn ui_tutorial_tabs(&mut self, p: &NodeHandle) -> Result<()> {
        self.ui(p, |inner| interfaces::Context::tutorial_tabs(inner))
    }
    fn ui_has_runscript(&mut self, p: &NodeHandle) -> Result<bool> {
        self.ui(p, |inner| interfaces::Context::has_runscript(inner))
    }
    fn ui_remove_runscript(&mut self, p: &NodeHandle) -> Result<()> {
        self.ui(p, |inner| interfaces::Context::remove_runscript(inner))
    }
    fn ui_set_unclosable(&mut self, p: &NodeHandle, value: bool) -> Result<()> {
        self.ui(p, |inner| {
            interfaces::Context::set_unclosable_attribute(inner, value)
        })
    }
    fn ui_unclosable(&mut self, p: &NodeHandle) -> Result<bool> {
        self.ui(p, |inner| interfaces::Context::unclosable_attribute(inner))
    }
}

impl Inner {
    /// `InterfaceListeners.runOpen`/`runClose` for one of the shop's
    /// callbacks (`onOpen(621)`, `onClose(620)`); `None` for another's.
    pub fn shop_open_listener(&mut self, callback: &Callback) -> Option<interfaces::Result<bool>> {
        let side = callback.value::<shops::SideOpen>().is_some();
        let close = callback.value::<shops::TemplateClose>().is_some();
        if !side && !close {
            return None;
        }
        let p = self.current.clone()?;
        let owner = p.0.clone();
        let mut host = ShopHost::new(self);
        let result = if side {
            controller::side_open(&mut host, &owner)
        } else {
            controller::template_close(&mut host, &owner)
        };
        if !host.pending.is_empty() {
            return Some(Err(interfaces::Error::Binding(
                "a shop open or close listener asked for a dialogue".into(),
            )));
        }
        Some(result.map_err(Into::into))
    }
}

impl Live {
    /// A shop call with the live world, then what it asked of the dialogue
    /// interpreter.
    pub fn shop_call<T>(
        &mut self,
        npc: Option<usize>,
        f: impl FnOnce(&mut ShopHost<'_>) -> Result<T>,
    ) -> Result<T> {
        let mut host = ShopHost::new(&mut self.inner);
        host.npc = npc;
        let result = f(&mut host);
        let pending = std::mem::take(&mut host.pending);
        for node in self.inner.frames.keys().copied().collect::<Vec<_>>() {
            if let Some(p) = self.player_of(node) {
                self.inner.drain(&p);
            }
        }
        let value = result?;
        for request in pending {
            self.shop_pending(request)?;
        }
        Ok(value)
    }
    fn shop_pending(&mut self, request: Pending) -> Result<()> {
        use crate::world::dialogue as d;
        match request {
            Pending::Dialogue(p, text) => {
                // ContentAPI.sendDialogue: sendDialogue(*splitLines(message)).
                let lines = crate::world::dialogue_dsl::split(&d::text(&text)).map_err(service)?;
                let interpreter = self.interpreter_of(&p);
                interpreter
                    .send_dialogue(self, Some(&lines))
                    .map_err(service)?;
                self.inner.drain(&p);
            }
            Pending::NpcDialogue(p, npc, text, expression) => {
                // sendNPCDialogue(player, npc, msg, expr): sendDialogues(npc,
                // expr, *splitLines(msg)).
                let expression = match expression {
                    "ANNOYED" => d::FacialExpression::Annoyed,
                    _ => d::FacialExpression::Neutral,
                };
                let lines = crate::world::dialogue_dsl::split(&d::text(&text)).map_err(service)?;
                let interpreter = self.interpreter_of(&p);
                interpreter
                    .send_dialogues(self, npc, expression.animation_id(), false, &lines)
                    .map_err(service)?;
                self.inner.drain(&p);
            }
            Pending::Input(p, prompt, script) => {
                self.shop_prompt(&p, &prompt, script).map_err(service)?;
            }
        }
        Ok(())
    }
    /// A 620 or 621 button (`on(620)`, `on(621)`).
    pub fn shop_button(
        &mut self,
        callback: &Callback,
        owner: &NodeHandle,
        button: Button,
    ) -> Option<interfaces::Result<bool>> {
        let template = callback.value::<shops::TemplateButtons>().is_some();
        let side = callback.value::<shops::SideButtons>().is_some();
        if !template && !side {
            return None;
        }
        let result = self.shop_call(None, |host| {
            if template {
                controller::template_button(host, owner, button)
            } else {
                controller::side_button(host, owner, button)
            }
        });
        Some(result.map_err(Into::into))
    }
    /// An NPC "trade"/"shop" listener.
    pub fn shop_listener(&mut self, which: NpcListener, player: &P, npc: &P) -> Result<bool> {
        let view = self.shop_npc(npc)?;
        self.shop_call(Some(npc.node()), |host| {
            controller::npc_option(host, which, &player.0, &view)
        })
    }
    /// `setDest(IntType.NPC, "trade", "shop")`'s override.
    pub fn shop_destination(&mut self, npc: &P) -> Result<[i32; 3]> {
        let view = self.shop_npc(npc)?;
        let at = self.shop_call(Some(npc.node()), |host| {
            controller::destination(host, &view)
        })?;
        Ok([at.x, at.y, at.z])
    }
    fn shop_npc(&mut self, npc: &P) -> Result<shops::Npc> {
        let id = self
            .inner
            .npcs
            .rows
            .get(&npc.node())
            .map(|r| r.id)
            .ok_or(Error::Null)?;
        let actor = self.inner.state.actor(npc.node()).map_err(service)?;
        let entity = actor.queue.entity();
        let [x, y, z] = entity.location.as_ref().ok_or(Error::Null)?.coordinates();
        let step = entity.direction.ok_or(Error::Null)?.step();
        Ok(shops::Npc {
            id,
            step,
            location: shops::Location { x, y, z },
        })
    }
    /// `Shops.tick()` (a tick listener).
    pub fn shops_tick(&mut self) -> Result<()> {
        self.shop_call(None, |host| controller::tick(host))
    }
    /// `Shops.startup()` with `defineInterfaceListeners` and the NPC
    /// listeners (`defineListeners`, `defineDestinationOverrides`).
    pub fn install_shops(&mut self) -> std::result::Result<(), String> {
        let world = shops::World::new(&self.inner.factory).map_err(|e| format!("{e:?}"))?;
        self.inner.shops.world = Some(world);
        controller::define_interface_listeners(&self.listeners).map_err(|e| format!("{e:?}"))?;
        use crate::world::interaction_listeners::{Function, FunctionKind, Kind, Text};
        let registry = self.item_actions.listeners.clone();
        let texts =
            |o: &[&str]| -> Vec<Text> { o.iter().map(|s| s.encode_utf16().collect()).collect() };
        let kind = Kind::Npc.ordinal();
        for ((id, options), which) in controller::NPC_LISTENERS.iter().zip([
            NpcListener::Generic,
            NpcListener::Siegfried,
            NpcListener::FurTrader,
            NpcListener::CandleMaker,
        ]) {
            let function = Function::new(FunctionKind::On, SHOPS, ShopListener(which));
            let ctx = super::items::Ctx(self);
            match id {
                Some(id) => registry.add_ids(&ctx, &[*id], kind, &texts(options), function),
                None => registry.add_generics(&ctx, &texts(options), kind, function),
            }
            .map_err(|e| format!("{e:?}"))?;
        }
        let function = Function::new(FunctionKind::Destination, SHOPS, ShopDestination);
        registry
            .add_destinations(
                &super::items::Ctx(self),
                kind,
                None,
                &texts(&controller::DESTINATION_OPTIONS),
                function,
            )
            .map_err(|e| format!("{e:?}"))?;
        self.shop_call(None, |host| controller::startup(host))
            .map_err(|e| format!("Shops.startup: {e:?}"))
    }
}
