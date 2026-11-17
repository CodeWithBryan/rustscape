//! Actual BankContainer services on canonical fields, variables and UI objects.
//! PIN entry and deposit-box item-select rendering remain required boundaries.
use super::{Context as InterfaceContext, Handle, Manager, UiContext, ui};
#[path = "ui_bank_content.rs"]
mod content;
pub(super) use content::define_player_buttons;
#[path = "ui_input.rs"]
mod input;
use rs09_protocol::game::{Frame, updates};
use rs09_server::{
    definitions::item::Registry,
    inventory::{
        self as inv, bank,
        container::{self, Container, Event, Items},
        container_listener::Handle as Listener,
    },
    player::{bank_pin, ironman, owned::Player, variables},
    world::{
        component_objects::{BankClose, Close},
        player_walk::NodeHandle,
    },
};
fn error(e: impl std::fmt::Debug) -> inv::Error {
    inv::Error::Binding(format!("{e:?}"))
}
impl UiContext<'_> {
    fn bank_player(&self, owner: &NodeHandle) -> inv::Result<Player> {
        self.state.player_fields(owner).map_err(error)
    }
    fn bank_send(&mut self, player: &Player, frame: Frame) -> inv::Result<()> {
        let owner = player.owner();
        if !self.bank_player(&owner)?.same(player) {
            return Err(inv::Error::Binding(
                "bank recipient fields were replaced".into(),
            ));
        }
        if !self.state.actor(owner.node).map_err(error)?.connected {
            return Err(inv::Error::Binding("bank recipient is disconnected".into()));
        }
        self.host.send(owner.node, frame);
        Ok(())
    }
    fn bank_manager<T>(
        &mut self,
        manager: Manager,
        operation: impl FnOnce(&mut UiContext<'_>, &Manager) -> ui::Result<T>,
    ) -> inv::Result<T> {
        let owner = manager.player().ok_or(inv::Error::Null)?;
        let player = self.bank_player(&owner)?;
        if !player.manager().fields.same(&manager.fields) {
            return Err(inv::Error::Binding(
                "bank interface receiver was replaced".into(),
            ));
        }
        let mut context = UiContext {
            state: &mut *self.state,
            host: &mut *self.host,
            node: owner.node,
        };
        context.validate_manager(&manager).map_err(error)?;
        operation(&mut context, &manager).map_err(error)
    }
    fn bank_text(&mut self, player: &Player, text: &str) -> inv::Result<()> {
        let mut payload = text.as_bytes().to_vec();
        payload.push(0);
        self.bank_send(
            player,
            Frame {
                opcode: 70,
                payload,
            },
        )
    }
}
impl container::Context for UiContext<'_> {
    fn update(
        &mut self,
        listener: Listener,
        container: &mut Container,
        event: Event,
        _: &Registry,
    ) -> inv::Result<()> {
        if let Some(receiver) = listener.value::<bank::Listener>() {
            self.bank_player(&receiver.owner().ok_or(inv::Error::Null)?)?;
            return receiver.update(container, Some(&event), self);
        }
        if let Some(receiver) = listener.value::<inv::listener::Listener>() {
            let player = self.bank_player(&receiver.owner().ok_or(inv::Error::Null)?)?;
            return player.apply(
                self.state,
                &mut self.host.frames,
                super::super::now,
                |_, _, context| receiver.update_event(context, container, Some(&event)),
            );
        }
        Err(inv::Error::Binding("unbound bank mutation listener".into()))
    }
    fn refresh(
        &mut self,
        listener: Listener,
        container: &mut Container,
        _: &Registry,
    ) -> inv::Result<()> {
        if let Some(receiver) = listener.value::<bank::Listener>() {
            self.bank_player(&receiver.owner().ok_or(inv::Error::Null)?)?;
            return receiver.refresh(container, self);
        }
        if let Some(receiver) = listener.value::<inv::listener::Listener>() {
            let player = self.bank_player(&receiver.owner().ok_or(inv::Error::Null)?)?;
            return player.apply(
                self.state,
                &mut self.host.frames,
                super::super::now,
                |_, _, context| receiver.refresh(context, container),
            );
        }
        Err(inv::Error::Binding("unbound bank refresh listener".into()))
    }
    fn ground(
        &mut self,
        _: &mut Container,
        _: Option<rs09_server::inventory::item::Handle>,
        _: &Registry,
    ) -> inv::Result<()> {
        Err(inv::Error::Binding(
            "bank overflow requires the owning-world ground service".into(),
        ))
    }
}
impl ironman::Context for UiContext<'_> {
    fn message(&mut self, owner: Option<&NodeHandle>, text: &str) -> ironman::Result<()> {
        let player = self
            .bank_player(owner.ok_or(ironman::Error::Null)?)
            .map_err(|e| ironman::Error::Binding(format!("{e:?}")))?;
        self.bank_text(&player, text)
            .map_err(|e| ironman::Error::Binding(format!("{e:?}")))
    }
}
impl bank::Context for UiContext<'_> {
    type Ironman = ironman::Manager;
    type Pin = bank_pin::Manager;
    type Interfaces = Manager;
    type Component = Option<Handle>;
    type Dispatch = Player;
    fn inventory(&mut self, owner: &NodeHandle) -> inv::Result<Container> {
        Ok(self.bank_player(owner)?.inventory())
    }
    fn bank(&mut self, owner: &NodeHandle) -> inv::Result<bank::Bank> {
        Ok(self.bank_player(owner)?.bank())
    }
    fn ironman(&mut self, owner: &NodeHandle) -> inv::Result<Self::Ironman> {
        Ok(self.bank_player(owner)?.ironman())
    }
    fn ultimate_restriction(&mut self, manager: Self::Ironman) -> inv::Result<bool> {
        self.bank_player(&manager.player().ok_or(inv::Error::Null)?)?;
        manager
            .check(self, Some(ironman::Mode::Ultimate))
            .map_err(error)
    }
    fn pin(&mut self, owner: &NodeHandle) -> inv::Result<Self::Pin> {
        Ok(self.bank_player(owner)?.bank_pin())
    }
    fn unlocked(&mut self, pin: Self::Pin) -> inv::Result<bool> {
        self.bank_player(&pin.player().ok_or(inv::Error::Null)?)?;
        Ok(pin.is_unlocked())
    }
    fn dev_mode(&mut self) -> inv::Result<bool> {
        Ok(false)
    }
    fn open_pin(&mut self, _: Self::Pin, _: i32) -> inv::Result<()> {
        Err(inv::Error::Binding(
            "bank PIN entry needs full PIN runtime services".into(),
        ))
    }
    fn interfaces(&mut self, owner: &NodeHandle) -> inv::Result<Manager> {
        Ok(self.bank_player(owner)?.manager())
    }
    fn component(&mut self, id: i32) -> inv::Result<Option<Handle>> {
        InterfaceContext::component(self, id)
            .map(Some)
            .map_err(error)
    }
    fn open(&mut self, manager: Manager, component: Option<Handle>) -> inv::Result<Option<Handle>> {
        self.bank_manager(manager, |ctx, m| ui::open(ctx, m, component))
    }
    fn open_component(&mut self, manager: Manager, id: i32) -> inv::Result<Option<Handle>> {
        self.bank_manager(manager, |ctx, m| ui::open_id(ctx, m, id))
    }
    fn close_event(
        &mut self,
        component: Option<Handle>,
        event: bank::CloseEvent,
    ) -> inv::Result<()> {
        component.ok_or(inv::Error::Null)?.write().close_event = Some(Close::new(BankClose(event)));
        Ok(())
    }
    fn remove_tabs(&mut self, manager: Manager, tabs: &[i32]) -> inv::Result<()> {
        self.bank_manager(manager, |ctx, m| ui::remove_tabs(ctx, m, tabs))
    }
    fn open_default_tabs(&mut self, manager: Manager) -> inv::Result<()> {
        self.bank_manager(manager, |ctx, m| ui::open_default_tabs(ctx, m))
    }
    fn open_single_tab(&mut self, manager: Manager, component: Option<Handle>) -> inv::Result<()> {
        self.bank_manager(manager, |ctx, m| {
            ui::open_single_tab(ctx, m, &component.ok_or(ui::Error::Null)?).map(|_| ())
        })
    }
    fn close_single_tab(&mut self, manager: Manager) -> inv::Result<()> {
        self.bank_manager(manager, |ctx, m| ui::close_single_tab(ctx, m).map(|_| ()))
    }
    fn dispatch(&mut self, owner: &NodeHandle) -> inv::Result<Player> {
        self.bank_player(owner)
    }
    fn iface_settings(
        &mut self,
        player: Player,
        [mask, child, pane, start, end]: [i32; 5],
    ) -> inv::Result<()> {
        self.bank_player(&player.owner())?;
        let frame = updates::interface_events(
            pane.wrapping_shl(16) | child,
            start,
            end,
            mask,
            player.manager().packet_count(1),
        );
        self.bank_send(&player, frame)
    }
    fn script(&mut self, player: Player, id: i32) -> inv::Result<()> {
        self.bank_player(&player.owner())?;
        let frame = updates::run_client_script(id, b"", &[], player.manager().packet_count(1))
            .map_err(error)?;
        self.bank_send(&player, frame)
    }
    fn message(&mut self, owner: &NodeHandle, text: &str) -> inv::Result<()> {
        let player = self.bank_player(owner)?;
        self.bank_text(&player, text)
    }
    fn packet_message(&mut self, player: Player, text: &str) -> inv::Result<()> {
        self.bank_text(&player, text)
    }
    fn deposit_items(
        &mut self,
        _: &NodeHandle,
        _: Items,
        _: &[&str],
        _: [i32; 4],
    ) -> inv::Result<()> {
        Err(inv::Error::Binding(
            "deposit box needs native item-select interface rendering".into(),
        ))
    }
    fn get_varbit(&mut self, owner: &NodeHandle, id: i32) -> inv::Result<i32> {
        let player = self.bank_player(owner)?;
        player
            .apply_world_variables(self.state, &mut self.host.frames, |ctx| {
                variables::get_varbit(ctx, Some(owner), id)
            })
            .map_err(error)
    }
    fn set_varbit(
        &mut self,
        owner: &NodeHandle,
        id: i32,
        value: i32,
        save: bool,
    ) -> inv::Result<()> {
        let player = self.bank_player(owner)?;
        player
            .apply_world_variables(self.state, &mut self.host.frames, |ctx| {
                variables::set_varbit(ctx, Some(owner), id, value, save)
            })
            .map_err(error)
    }
    fn set_varp(&mut self, owner: &NodeHandle, id: i32, value: i32) -> inv::Result<()> {
        let player = self.bank_player(owner)?;
        player
            .apply_world_variables(self.state, &mut self.host.frames, |ctx| {
                variables::set_varp(ctx, Some(owner), id, value, false)
            })
            .map_err(error)
    }
    fn set_varc(&mut self, owner: &NodeHandle, id: i32, value: i32) -> inv::Result<()> {
        let player = self.bank_player(owner)?;
        player
            .apply_world_variables(self.state, &mut self.host.frames, |ctx| {
                variables::set_varc(ctx, Some(owner), id, value)
            })
            .map_err(error)
    }
    fn set_attribute(&mut self, owner: &NodeHandle, key: &str, value: i32) -> inv::Result<()> {
        self.bank_player(owner)?
            .attributes()
            .set_integer(key, value);
        Ok(())
    }
    fn remove_attribute(&mut self, owner: &NodeHandle, key: &str) -> inv::Result<()> {
        self.bank_player(owner)?.attributes().remove(key);
        Ok(())
    }
    fn packet(&mut self, packet: bank::Packet) -> inv::Result<()> {
        let player = self.bank_player(packet.player.as_ref().ok_or(inv::Error::Null)?)?;
        let frame = packet.frame(
            &self
                .host
                .ui
                .as_ref()
                .ok_or(inv::Error::Null)?
                .factory
                .null_item,
        )?;
        self.bank_send(&player, frame)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rs09_server::{
        definitions::{
            item::{ItemDefinition, Shared},
            property::Property,
            varbit,
        },
        inventory::item::Item,
        player::details::ClientInfo,
    };
    pub(super) fn probe() -> (
        super::super::super::Probe,
        super::super::super::Node,
        Player,
    ) {
        let (mut p, node, _) = super::super::super::tests::probe();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/interfaces/interface_configs.json");
        let mut ui = super::super::Ui::load(&path).unwrap();
        let mut defs = Registry::default();
        let mut item = ItemDefinition {
            id: 1351,
            ..Default::default()
        };
        item.handlers
            .insert("weight".into(), Property::Double(1.75));
        defs.insert(1351, Shared::new(item));
        ui.use_definitions(std::sync::Arc::new(defs)).unwrap();
        p.host.ui = Some(ui);
        for i in 0..9 {
            p.state.identity().varbits().create(800 + i, 4885 + i, 0, 7);
        }
        varbit::Loader::load_text(
            &p.state.identity().varbits(),
            include_str!("../../../../fixtures/variables/varbit_definitions.json"),
        )
        .unwrap();
        super::super::admit(
            &mut p,
            node,
            ClientInfo {
                display_mode: 0,
                window_mode: 2,
                screen_width: 1280,
                screen_height: 720,
            },
        )
        .unwrap();
        let owner = p.state.retain_node(node).unwrap();
        let player = p.state.player_fields(&owner).unwrap();
        p.host.frames.clear();
        (p, node, player)
    }
    #[test]
    fn game_configuration_uses_world_fields_without_a_visual_actor_lookup() {
        use rs09_protocol::game::updates::ServerUpdate;
        let (mut p, node, player) = probe();
        let actors = std::mem::take(&mut p.actors);
        let manager = player.manager();
        let count = manager.packet_count(0);
        p.host.frames.clear();
        ui::send_game_configuration(
            &mut UiContext::world(&mut p.state, &mut p.host, node),
            &manager,
        )
        .unwrap();
        assert!(p.actors.is_empty());
        assert!(p.host.frames[&node].iter().any(|f| matches!(
            ServerUpdate::decode(f).unwrap(),
            Some(ServerUpdate::InventoryFull { container: 93, .. })
        )));
        assert!(manager.packet_count(0) > count);
        assert!(
            p.state
                .player_fields(&player.owner())
                .unwrap()
                .manager()
                .fields
                .same(&manager.fields)
        );
        assert!(p.host.masks[&node].body().same(player.masks().body()));
        p.actors = actors;
    }
    #[test]
    fn actual_bank_open_deposit_withdraw_and_captured_close_share_inventory_variables_and_ui() {
        let (mut p, node, player) = probe();
        player
            .apply(
                &mut p.state,
                &mut p.host.frames,
                super::super::super::now,
                |inventory, defs, ctx| {
                    inventory.add_two(
                        Some(&Shared::new(Item::new(1351, 2, 0, defs))),
                        true,
                        defs,
                        ctx,
                    )
                },
            )
            .unwrap();
        assert_eq!(player.inventory().amount_id(1351, &player.definitions()), 2);
        let bank = player.bank();
        {
            let mut ctx = UiContext::new(&mut p, node);
            bank.open_bank(&mut ctx).unwrap();
            let count = ctx.host.frames[&node].len();
            bank.open_bank(&mut ctx).unwrap();
            assert_eq!(ctx.host.frames[&node].len(), count);
            assert!(bank.is_open());
            assert_eq!(
                player
                    .manager()
                    .fields
                    .read()
                    .opened
                    .as_ref()
                    .unwrap()
                    .read()
                    .id,
                762
            );
            assert_eq!(
                player
                    .manager()
                    .fields
                    .read()
                    .single_tab
                    .as_ref()
                    .unwrap()
                    .read()
                    .id,
                763
            );
            bank.add_item(0, 2, &mut ctx).unwrap();
            assert_eq!(player.inventory().amount_id(1351, &player.definitions()), 0);
            assert_eq!(bank.container().amount_id(1351, &player.definitions()), 2);
            bank.take_item(0, 1, &mut ctx).unwrap();
        }
        assert_eq!(player.inventory().amount_id(1351, &player.definitions()), 1);
        assert_eq!(bank.container().amount_id(1351, &player.definitions()), 1);
        assert_eq!(player.inventory().listener_list().len(), 2);
        assert_eq!(player.settings().fields.read().weight, 1.75);
        assert_eq!(player.variables().get_varp(1249), Ok(50));
        assert!(p.host.frames[&node].iter().any(|frame| matches!(updates::ServerUpdate::decode(frame), Ok(Some(updates::ServerUpdate::InventoryFull { pointer, container:95, .. })) if pointer == (762 << 16) | 64000)));
        // Ordinary canonical inventory operations also dispatch the installed bank listener.
        player
            .apply(
                &mut p.state,
                &mut p.host.frames,
                super::super::super::now,
                |inventory, defs, ctx| inventory.refresh(None, defs, ctx),
            )
            .unwrap();
        player.attributes().set_boolean("search", true);
        p.host.frames.clear();
        let manager = player.manager();
        let count = manager.packet_count(0);
        assert!(ui::close(&mut UiContext::new(&mut p, node), &manager).unwrap());
        assert!(!bank.is_open());
        assert_eq!(player.inventory().listener_list().len(), 1);
        assert!(manager.fields.read().opened.is_none());
        assert!(manager.fields.read().single_tab.is_none());
        assert!(!player.attributes().contains_key("search"));
        assert_eq!(
            p.host.frames[&node],
            vec![
                updates::close_interface(count, (746 << 16) | 76),
                updates::run_client_script(571, b"", &[], count.wrapping_add(1)).unwrap(),
                updates::close_interface(count.wrapping_add(2), (746 << 16) | 6),
            ]
        );
    }
    #[test]
    fn bank_restrictions_and_missing_pin_entry_do_not_fabricate_a_bank_open() {
        let (mut p, node, player) = probe();
        player.ironman().set_mode(Some(ironman::Mode::Ultimate));
        player
            .bank()
            .open_bank(&mut UiContext::new(&mut p, node))
            .unwrap();
        assert!(!player.bank().is_open());
        assert!(player.manager().fields.read().opened.is_none());
        assert_eq!(p.host.frames[&node].len(), 1);
        assert_eq!(p.host.frames[&node][0].opcode, 70);
        player.ironman().set_mode(Some(ironman::Mode::None));
        player.bank_pin().fields().write().pin = Some(bank_pin::text("1234"));
        player.bank_pin().fields().write().unlocked = false;
        p.host.frames.clear();
        assert!(
            player
                .bank()
                .open_bank(&mut UiContext::new(&mut p, node))
                .is_err()
        );
        assert!(!player.bank().is_open());
        assert!(player.manager().fields.read().opened.is_none());
        assert!(p.host.frames.is_empty());
    }
    #[test]
    fn foreign_bank_close_callback_fails_before_touching_its_real_open_bank() {
        let (mut p, node, _) = probe();
        let (mut foreign, foreign_node, player) = probe();
        player
            .bank()
            .open_bank(&mut UiContext::new(&mut foreign, foreign_node))
            .unwrap();
        let event = player
            .manager()
            .fields
            .read()
            .opened
            .as_ref()
            .unwrap()
            .read()
            .close_event
            .clone()
            .unwrap();
        let component =
            InterfaceContext::component(&mut UiContext::new(&mut p, node), 762).unwrap();
        component.write().close_event = Some(event);
        assert!(ui::component_close(&mut UiContext::new(&mut p, node), &component).is_err());
        assert!(player.bank().is_open());
        assert_eq!(player.inventory().listener_list().len(), 2);
        assert!(p.host.frames.is_empty());
    }
    #[test]
    fn disconnected_open_bank_ui_and_inventory_listener_cycles_collect_without_external_roots() {
        let (mut p, node, player) = probe();
        player
            .bank()
            .open_bank(&mut UiContext::new(&mut p, node))
            .unwrap();
        let weak = player.downgrade();
        drop(player);
        p.remove(1, node).unwrap();
        for _ in 0..2 {
            super::super::super::tick::handle(&mut p, false).unwrap();
            super::super::reap(&mut p.host, &p.state);
            p.state.collect();
        }
        assert!(weak.upgrade().is_none());
        assert!(p.state.actor(node).is_err());
        assert!(p.host.ui.as_ref().unwrap().players.is_empty());
    }
}
