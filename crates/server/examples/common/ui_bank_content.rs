//! Original bank listeners and IfAction/CloseIface joined to canonical services.
//! Fresh controlled admission has no active dialogue, component lock or zones;
//! complete dialogue/input/login/world-map services remain required.
use super::*;
use rs09_server::{
    definitions::item::Handle as Definition,
    inventory::item::Handle as Item,
    world::{
        bank_interface::{self as content, Program},
        interface_listeners::{self as listeners, Button, Callback},
        interface_packets::{self as packets, Action, LoginPulse},
    },
};
fn ui_error(e: impl std::fmt::Debug) -> ui::Error {
    ui::Error::Binding(format!("{e:?}"))
}

enum PlayerButton {
    ToggleRun,
    CombatStyle,
    RemoveEquipment,
    Shop,
}
impl rs09_server::world::component_objects::NativeObject for PlayerButton {}

pub(in super::super) fn define_player_buttons(registry: &listeners::Registry) -> ui::Result<()> {
    registry.add(750, None, Callback::button(PlayerButton::ToggleRun))?;
    registry.add(261, Some(3), Callback::button(PlayerButton::ToggleRun))?;
    for interface in super::super::super::weapons::INTERFACES {
        registry.add(
            interface.id,
            None,
            Callback::button(PlayerButton::CombatStyle),
        )?;
    }
    registry.add(
        387,
        Some(28),
        Callback::button(PlayerButton::RemoveEquipment),
    )?;
    for group in [620, 621] {
        registry.add(group, None, Callback::button(PlayerButton::Shop))?;
    }
    Ok(())
}

#[derive(Clone)]
pub(crate) enum Items {
    Bank(bank::Bank),
    Inventory(Container),
}
impl Items {
    fn container(&self) -> Container {
        match self {
            Self::Bank(b) => b.container(),
            Self::Inventory(c) => c.clone(),
        }
    }
    fn bank(self) -> ui::Result<bank::Bank> {
        match self {
            Self::Bank(b) => Ok(b),
            _ => Err(ui::Error::Type),
        }
    }
}
impl listeners::Context for UiContext<'_> {
    fn invoke_open(&mut self, callback: Callback, p: &NodeHandle, c: &Handle) -> ui::Result<bool> {
        self.bank_player(p).map_err(ui_error)?;
        if Program::kind(&callback) != Some(content::Kind::Open) {
            return Err(ui::Error::Type);
        }
        Program::invoke(
            callback,
            self,
            p,
            c,
            Button {
                opcode: 0,
                button: 0,
                slot: 0,
                item: 0,
            },
        )
    }
    fn invoke_button(
        &mut self,
        callback: Callback,
        p: &NodeHandle,
        c: &Handle,
        b: Button,
    ) -> ui::Result<bool> {
        let player = self.bank_player(p).map_err(ui_error)?;
        if let Some(button) = callback.value::<PlayerButton>() {
            match *button {
                PlayerButton::ToggleRun => {
                    let queue = &self.state.actor(p.node).map_err(ui_error)?.queue;
                    let enabled = {
                        let mut state = queue.state.write();
                        state.entity.run_toggled = !state.entity.run_toggled;
                        state.entity.run_toggled
                    };
                    self.send(updates::varp(173, i32::from(enabled)))?;
                }
                PlayerButton::CombatStyle => {
                    let group = c.read().id;
                    super::super::equipment_binding::select_style(self, group, b.button)?;
                }
                PlayerButton::RemoveEquipment => {
                    if b.opcode == 81 {
                        super::super::equipment_binding::change(
                            self, &player, b.slot, b.item, false,
                        )?;
                    }
                }
                PlayerButton::Shop => {
                    let group = c.read().id;
                    return super::super::shops::button(self, &player, group, b);
                }
            }
            return Ok(true);
        }
        if Program::kind(&callback) == Some(content::Kind::Open) {
            return Err(ui::Error::Type);
        }
        Program::invoke(callback, self, p, c, b)
    }
}
impl content::Context for UiContext<'_> {
    type Container = Items;
    type Item = Item;
    type Definition = Definition;
    type Dispatch = Player;
    type Dialogue = ();
    fn bank_size(&mut self) -> ui::Result<i32> {
        Ok(bank::SIZE)
    }
    fn bank(&mut self, p: &NodeHandle) -> ui::Result<Items> {
        self.bank_player(p)
            .map(|p| Items::Bank(p.bank()))
            .map_err(ui_error)
    }
    fn inventory(&mut self, p: &NodeHandle) -> ui::Result<Items> {
        self.bank_player(p)
            .map(|p| Items::Inventory(p.inventory()))
            .map_err(ui_error)
    }
    fn item(&mut self, c: Items, slot: i32) -> ui::Result<Option<Item>> {
        Ok(c.container().get(slot))
    }
    fn amount(&mut self, c: Items, item: Item) -> ui::Result<i32> {
        Ok(c.container().amount(Some(&item)))
    }
    fn amount_id(&mut self, c: Items, id: i32) -> ui::Result<i32> {
        let defs = self
            .host
            .ui
            .as_ref()
            .ok_or(ui::Error::Null)?
            .definitions
            .clone();
        Ok(c.container().amount_id(id, &defs))
    }
    fn last_amount_x(&mut self, c: Items) -> ui::Result<i32> {
        Ok(c.bank()?.last_amount_x())
    }
    fn take(&mut self, c: Items, slot: i32, amount: i32) -> ui::Result<()> {
        c.bank()?.take_item(slot, amount, self).map_err(ui_error)
    }
    fn deposit(&mut self, c: Items, slot: i32, amount: i32) -> ui::Result<()> {
        c.bank()?.add_item(slot, amount, self).map_err(ui_error)
    }
    fn transfer_x(&mut self, player: &NodeHandle, slot: i32, withdraw: bool) -> ui::Result<()> {
        self.input_transfer_x(player, slot, withdraw)
    }
    fn tab_index(&mut self, c: Items) -> ui::Result<i32> {
        Ok(c.bank()?.tab_index())
    }
    fn set_tab_index(&mut self, c: Items, index: i32) -> ui::Result<()> {
        c.bank()?.set_tab_index(index, self).map_err(ui_error)
    }
    fn collapse_tab(&mut self, c: Items, index: i32) -> ui::Result<()> {
        c.bank()?.collapse_tab(index, self).map_err(ui_error)
    }
    fn insert(&mut self, c: Items) -> ui::Result<bool> {
        c.bank()?.is_insert_items(self).map_err(ui_error)
    }
    fn set_insert(&mut self, c: Items, value: bool) -> ui::Result<()> {
        c.bank()?.set_insert_items(value, self).map_err(ui_error)
    }
    fn note(&mut self, c: Items) -> ui::Result<bool> {
        c.bank()?.is_note_items(self).map_err(ui_error)
    }
    fn set_note(&mut self, c: Items, value: bool) -> ui::Result<()> {
        c.bank()?.set_note_items(value, self).map_err(ui_error)
    }
    fn last_tab(&mut self, p: &NodeHandle) -> ui::Result<i32> {
        use rs09_server::player::attributes::AttributeValue;
        match self
            .bank_player(p)
            .map_err(ui_error)?
            .attributes()
            .get("bank:lasttab")
        {
            None => Ok(0),
            Some(AttributeValue::Int(n)) => Ok(n),
            Some(AttributeValue::Long(n)) => Ok(n as i32),
            Some(AttributeValue::Short(n)) => Ok(i32::from(n)),
            Some(AttributeValue::Byte(n)) => Ok(i32::from(n)),
            Some(AttributeValue::Object(n)) => {
                if let Some(n) = n.as_any().downcast_ref::<f64>() {
                    Ok(*n as i32)
                } else if let Some(n) = n.as_any().downcast_ref::<f32>() {
                    Ok(*n as i32)
                } else {
                    Err(ui::Error::Type)
                }
            }
            _ => Err(ui::Error::Type),
        }
    }
    fn varc(&mut self, p: &NodeHandle, id: i32, n: i32) -> ui::Result<()> {
        bank::Context::set_varc(self, p, id, n).map_err(ui_error)
    }
    fn search(&mut self, p: &NodeHandle) -> ui::Result<()> {
        self.bank_player(p)
            .map_err(ui_error)?
            .attributes()
            .set_boolean("search", true);
        Ok(())
    }
    fn definition(&mut self, item: Item) -> ui::Result<Definition> {
        item.read().definition.clone().ok_or(ui::Error::Null)
    }
    fn examine(&mut self, d: Definition) -> ui::Result<Option<content::Text>> {
        Ok(d.read()
            .examine
            .as_ref()
            .map(|s| s.encode_utf16().collect()))
    }
    fn id(&mut self, d: Definition) -> ui::Result<i32> {
        Ok(d.read().id)
    }
    fn name(&mut self, d: Definition) -> ui::Result<Option<content::Text>> {
        Ok(d.read().name.as_ref().map(|s| s.encode_utf16().collect()))
    }
    fn message(&mut self, p: &NodeHandle, text: Option<content::Text>) -> ui::Result<()> {
        let text =
            String::from_utf16(&text.ok_or(ui::Error::Null)?).map_err(|_| ui::Error::Type)?;
        let player = self.bank_player(p).map_err(ui_error)?;
        self.bank_text(&player, &text).map_err(ui_error)
    }
    fn debug(&mut self, p: &NodeHandle, _: content::Text) -> ui::Result<()> {
        self.bank_player(p).map_err(ui_error)?;
        Ok(())
    }
    fn dispatch(&mut self, p: &NodeHandle) -> ui::Result<Player> {
        self.bank_player(p).map_err(ui_error)
    }
    fn settings(&mut self, p: Player, fields: [i32; 5]) -> ui::Result<()> {
        bank::Context::iface_settings(self, p, fields).map_err(ui_error)
    }
    fn create_dialogue(&mut self, _: content::Dialogue) -> ui::Result<()> {
        Err(ui::Error::Binding(
            "bank help/deposit dialogues are not installed".into(),
        ))
    }
    fn open_dialogue(&mut self, _: &NodeHandle, _: ()) -> ui::Result<()> {
        Err(ui::Error::Binding(
            "bank dialogue runtime is not installed".into(),
        ))
    }
    fn open_bank(&mut self, p: &NodeHandle) -> ui::Result<()> {
        self.bank_player(p)
            .map_err(ui_error)?
            .bank()
            .open_bank(self)
            .map_err(ui_error)
    }
}
impl packets::Context for UiContext<'_> {
    type Dialogue = Player;
    type Locks = Player;
    type Zone = Player;
    type Manager = Manager;
    type Dispatch = Player;
    fn debug(&mut self, p: &NodeHandle, _: String) -> ui::Result<()> {
        self.bank_player(p).map_err(ui_error)?;
        Ok(())
    }
    fn dialogue(&mut self, p: &NodeHandle) -> ui::Result<Player> {
        self.bank_player(p).map_err(ui_error)
    }
    fn has_dialogue(&mut self, _: Player) -> ui::Result<bool> {
        Ok(false)
    }
    fn close_dialogue(&mut self, _: Player) -> ui::Result<()> {
        Err(ui::Error::Binding(
            "dialogue interpreter is not installed".into(),
        ))
    }
    fn locks(&mut self, p: &NodeHandle) -> ui::Result<Player> {
        self.bank_player(p).map_err(ui_error)
    }
    fn component_locked(&mut self, player: Player) -> ui::Result<bool> {
        let owner = player.owner();
        self.bank_player(&owner).map_err(ui_error)?;
        super::super::super::script_services::locked(
            self.state,
            owner.node,
            rs09_server::world::action_locks::Kind::Component,
        )
        .map_err(ui_error)
    }
    fn zone(&mut self, p: &NodeHandle) -> ui::Result<Player> {
        self.bank_player(p).map_err(ui_error)
    }
    fn click_zone(&mut self, _: Player, _: Action) -> ui::Result<bool> {
        Ok(false)
    }
    fn manager(&mut self, p: &NodeHandle) -> ui::Result<Manager> {
        self.bank_player(p).map(|p| p.manager()).map_err(ui_error)
    }
    fn component(&mut self, m: Manager, id: i32) -> ui::Result<Option<Handle>> {
        self.bank_manager(m, |ctx, m| ui::get_component(ctx, m, id))
            .map_err(ui_error)
    }
    fn hidden(&mut self, c: &Handle) -> ui::Result<bool> {
        Ok(c.read().hidden)
    }
    fn plugin(&mut self, c: &Handle) -> ui::Result<Option<ui::Plugin>> {
        Ok(self
            .host
            .ui
            .as_mut()
            .ok_or(ui::Error::Null)?
            .registry
            .plugin(c))
    }
    fn id(&mut self, c: &Handle) -> ui::Result<i32> {
        Ok(c.read().id)
    }
    fn button_event(&mut self, p: &NodeHandle, _: i32, _: i32) -> ui::Result<()> {
        self.bank_player(p).map_err(ui_error)?;
        Ok(())
    }
    fn run_listener(&mut self, p: &NodeHandle, c: &Handle, b: Button) -> ui::Result<bool> {
        let registry = self
            .host
            .ui
            .as_ref()
            .ok_or(ui::Error::Null)?
            .listeners
            .clone();
        registry.run(self, Some(p), Some(c), b)
    }
    fn handle_plugin(
        &mut self,
        _: ui::Plugin,
        _: &NodeHandle,
        _: &Handle,
        _: Button,
    ) -> ui::Result<bool> {
        Err(ui::Error::Binding(
            "component button plugin runtime is not installed".into(),
        ))
    }
    fn close(&mut self, m: Manager) -> ui::Result<bool> {
        self.bank_manager(m, |ctx, m| ui::close(ctx, m))
            .map_err(ui_error)
    }
    fn logging_present(&mut self, p: &NodeHandle) -> ui::Result<bool> {
        Ok(self
            .bank_player(p)
            .map_err(ui_error)?
            .attributes()
            .get("logging_in")
            .is_some())
    }
    fn submit_login(&mut self, _: LoginPulse) -> ui::Result<()> {
        Err(ui::Error::Binding(
            "deferred login/world configuration is not installed".into(),
        ))
    }
    fn map_present(&mut self, p: &NodeHandle) -> ui::Result<bool> {
        Ok(self
            .bank_player(p)
            .map_err(ui_error)?
            .attributes()
            .get("worldMap:viewing")
            .is_some())
    }
    fn remove_map(&mut self, p: &NodeHandle) -> ui::Result<()> {
        self.bank_player(p)
            .map_err(ui_error)?
            .attributes()
            .remove("worldMap:viewing");
        Ok(())
    }
    fn dispatch(&mut self, p: &NodeHandle) -> ui::Result<Player> {
        self.bank_player(p).map_err(ui_error)
    }
    fn resizable(&mut self, m: Manager) -> ui::Result<bool> {
        self.bank_manager(m, |ctx, _| InterfaceContext::resizable(ctx))
            .map_err(ui_error)
    }
    fn windows_pane(&mut self, p: Player, id: i32, kind: i32) -> ui::Result<()> {
        let frame = updates::windows_pane(id, kind, p.manager().packet_count(1));
        self.bank_send(&p, frame).map_err(ui_error)
    }
    fn unlock(&mut self, _: &NodeHandle) -> ui::Result<()> {
        Err(ui::Error::Binding(
            "world map lock runtime is not installed".into(),
        ))
    }
    fn remove_logging(&mut self, p: &NodeHandle) -> ui::Result<()> {
        self.bank_player(p)
            .map_err(ui_error)?
            .attributes()
            .remove("logging_in");
        Ok(())
    }
    fn configure_world(&mut self, _: &NodeHandle) -> ui::Result<()> {
        Err(ui::Error::Binding(
            "complete login world configuration is not installed".into(),
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use rs09_protocol::{
        buffer::{Reader, Writer},
        game::actions::ClientAction,
    };
    use rs09_server::{definitions::item::Shared, inventory::item::Item};
    fn button(opcode: u8, iface: i32, child: i32, slot: i32) -> ClientAction {
        let mut w = Writer::new();
        w.p4(iface.wrapping_shl(16) | child);
        w.p2(slot);
        ClientAction::decode(opcode, &mut Reader::new(&w.into_vec()))
            .unwrap()
            .unwrap()
    }
    #[test]
    fn actual_decoded_bank_buttons_deposit_withdraw_toggle_and_close_with_late_button() {
        let (mut p, node, player) = super::super::tests::probe();
        player
            .apply(
                &mut p.state,
                &mut p.host.frames,
                super::super::super::super::now,
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
        let count = player.manager().packet_count(0);
        p.host.frames.clear();
        player
            .bank()
            .open_bank(&mut UiContext::new(&mut p, node))
            .unwrap();
        assert_eq!(
            p.host.frames[&node][0],
            updates::interface_events((762 << 16) | 73, 0, 496, content::OPEN_SETTINGS, count)
        );
        assert_eq!(player.variables().get_varp(808), Ok(1));
        assert_eq!(player.attributes().integer("bank:lasttab", -1), Ok(0));
        // Client-generated side763:0 uses actual slot, with item_id=-1.
        assert!(super::super::super::process(&mut p, node, &button(155, 763, 0, 0)).unwrap());
        assert_eq!(
            player
                .bank()
                .container()
                .amount_id(1351, &player.definitions()),
            1
        );
        assert_eq!(player.inventory().amount_id(1351, &player.definitions()), 1);
        assert!(super::super::super::process(&mut p, node, &button(155, 762, 73, 0)).unwrap());
        assert_eq!(
            player
                .bank()
                .container()
                .amount_id(1351, &player.definitions()),
            0
        );
        assert_eq!(player.inventory().amount_id(1351, &player.definitions()), 2);
        for child in [14, 16, 20] {
            assert!(
                super::super::super::process(&mut p, node, &button(155, 762, child, -1)).unwrap()
            );
        }
        assert_eq!(player.variables().get_varp(304), Ok(1));
        assert_eq!(player.variables().get_varp(115), Ok(1));
        assert_eq!(player.attributes().boolean("search", false), Ok(true));
        let counter = player.manager().packet_count(0);
        p.host.frames.clear();
        let close = ClientAction::decode(184, &mut Reader::new(&[]))
            .unwrap()
            .unwrap();
        assert!(super::super::super::process(&mut p, node, &close).unwrap());
        assert_eq!(
            p.host.frames[&node],
            vec![
                updates::close_interface(counter, (746 << 16) | 76),
                updates::run_client_script(571, b"", &[], counter.wrapping_add(1)).unwrap(),
                updates::close_interface(counter.wrapping_add(2), (746 << 16) | 6)
            ]
        );
        let after = p.host.frames[&node].clone();
        assert!(super::super::super::process(&mut p, node, &button(155, 762, 22, -1)).unwrap());
        assert_eq!(p.host.frames[&node], after);
        assert!(!player.bank().is_open());
        assert!(player.manager().fields.read().opened.is_none());
        assert!(!player.attributes().contains_key("search"));
    }
    #[test]
    fn canonical_component_lock_gates_actual_bank_button_until_the_expiration_tick() {
        use rs09_server::world::action_locks::{Clock, Kind};
        let (mut p, node, player) = super::super::tests::probe();
        player
            .apply(
                &mut p.state,
                &mut p.host.frames,
                super::super::super::super::now,
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
        player
            .bank()
            .open_bank(&mut UiContext::new(&mut p, node))
            .unwrap();
        p.state.ticks = 20;
        p.state
            .locks(&player.owner())
            .unwrap()
            .lock_kind(Kind::Component, 10, &mut p.state, &mut Clock)
            .unwrap();
        p.host.frames.clear();
        let click = button(155, 763, 0, 0);
        assert!(super::super::super::process(&mut p, node, &click).unwrap());
        assert_eq!(player.inventory().amount_id(1351, &player.definitions()), 2);
        assert_eq!(
            player
                .bank()
                .container()
                .amount_id(1351, &player.definitions()),
            0
        );
        assert!(p.host.frames.is_empty());
        p.state.ticks = 30;
        assert!(super::super::super::process(&mut p, node, &click).unwrap());
        assert_eq!(player.inventory().amount_id(1351, &player.definitions()), 1);
        assert_eq!(
            player
                .bank()
                .container()
                .amount_id(1351, &player.definitions()),
            1
        );
    }
    #[test]
    fn hidden_and_missing_component_buttons_leave_real_bank_items_unchanged() {
        let (mut p, node, player) = super::super::tests::probe();
        player
            .bank()
            .open_bank(&mut UiContext::new(&mut p, node))
            .unwrap();
        let manager = player.manager();
        manager
            .fields
            .read()
            .opened
            .as_ref()
            .unwrap()
            .write()
            .hidden = true;
        p.host.frames.clear();
        let count = manager.packet_count(0);
        assert!(super::super::super::process(&mut p, node, &button(155, 762, 16, -1)).unwrap());
        assert_eq!(player.variables().get_varp(115), Ok(0));
        assert_eq!(manager.packet_count(0), count);
        assert!(p.host.frames.is_empty());
        assert!(super::super::super::process(&mut p, node, &button(155, 999, 73, 0)).unwrap());
        assert!(p.host.frames.is_empty());
    }
    #[test]
    fn cached_side_widget_slot8_deposits_that_exact_real_inventory_item() {
        let (mut p, node, player) = super::super::tests::probe();
        player
            .apply(
                &mut p.state,
                &mut p.host.frames,
                super::super::super::super::now,
                |inventory, defs, ctx| {
                    for slot in 0..9 {
                        inventory.replace(
                            Some(&Shared::new(Item::new(
                                1351,
                                1,
                                if slot == 8 { 9 } else { 1 },
                                defs,
                            ))),
                            slot,
                            false,
                            defs,
                            ctx,
                        )?;
                    }
                    inventory.update(None, defs, ctx)
                },
            )
            .unwrap();
        player
            .bank()
            .open_bank(&mut UiContext::new(&mut p, node))
            .unwrap();
        assert!(super::super::super::process(&mut p, node, &button(155, 763, 0, 8)).unwrap());
        assert!(player.inventory().get(8).is_none());
        assert_eq!(player.inventory().get(0).unwrap().read().charge(), 1);
        assert_eq!(player.bank().container().get(0).unwrap().read().charge(), 9);
        assert_eq!(player.inventory().amount_id(1351, &player.definitions()), 8);
    }
    #[test]
    fn explicit_probe_waits_for_real_inventory_and_never_reopens_after_close() {
        let (mut p, node, player) = super::super::tests::probe();
        assert!(p.host.ui.as_mut().unwrap().enable_bank_probe().is_err());
        assert!(!p.host.ui.as_ref().unwrap().bank_probe);
        p.host.ui.as_mut().unwrap().bank_probe = true;
        super::super::super::bank_probe(&mut p).unwrap();
        assert!(!player.bank().is_open());
        player
            .apply(
                &mut p.state,
                &mut p.host.frames,
                super::super::super::super::now,
                |inventory, defs, ctx| {
                    inventory.add_two(
                        Some(&Shared::new(Item::new(1351, 1, 0, defs))),
                        true,
                        defs,
                        ctx,
                    )
                },
            )
            .unwrap();
        super::super::super::bank_probe(&mut p).unwrap();
        assert!(player.bank().is_open());
        super::super::super::process(&mut p, node, &ClientAction::CloseIface).unwrap();
        p.host.frames.clear();
        super::super::super::bank_probe(&mut p).unwrap();
        assert!(!player.bank().is_open());
        assert!(p.host.frames.is_empty());
        assert_eq!(player.inventory().amount_id(1351, &player.definitions()), 1);
    }
}
