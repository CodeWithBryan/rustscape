//! Actual input response/callback bindings with canonical actor interaction
//! locks and the explicitly installed native function bodies below.
use super::*;
use rs09_server::{
    player::attributes::{AttributeObject, AttributeValue},
    world::{
        component_objects::Plugin,
        input_prompt::{self as input, InputType, Response, Transfer},
    },
};
use std::sync::Arc;
#[derive(Clone)]
pub(crate) struct Script(Arc<dyn AttributeObject>);
impl Script {
    fn new(value: Arc<dyn AttributeObject>) -> input::Result<Self> {
        if value.function_arity() != Some(1) {
            return Err(input::Error::Type);
        }
        Ok(Self(value))
    }
}
fn error(e: inv::Error) -> input::Error {
    match e {
        inv::Error::Null => input::Error::Null,
        inv::Error::Type => input::Error::Type,
        inv::Error::Number => input::Error::Number,
        inv::Error::Binding(s) => input::Error::Binding(s),
        other => input::Error::Binding(format!("{other:?}")),
    }
}
fn ui_error(e: input::Error) -> ui::Error {
    match e {
        input::Error::Null => ui::Error::Null,
        input::Error::Type => ui::Error::Type,
        other => ui::Error::Binding(format!("{other:?}")),
    }
}
fn utf8(text: &[u16]) -> Vec<u8> {
    std::char::decode_utf16(text.iter().copied())
        .map(|c| c.unwrap_or('?'))
        .collect::<String>()
        .into_bytes()
}
impl UiContext<'_> {
    pub(crate) fn input_response(&mut self, response: Response) -> ui::Result<()> {
        let player = self
            .state
            .retain_node(self.node)
            .map_err(|e| ui::Error::Binding(e.to_string()))?;
        input::receive(self, &player, response).map_err(ui_error)
    }
    pub(super) fn input_transfer_x(
        &mut self,
        player: &NodeHandle,
        slot: i32,
        withdraw: bool,
    ) -> ui::Result<()> {
        input::transfer_x(self, player, slot, withdraw, None).map_err(ui_error)
    }
    pub(in super::super) fn input_shop_x(
        &mut self,
        player: &NodeHandle,
        order: super::super::shops::Order,
        prompt: &str,
    ) -> ui::Result<()> {
        let script = Script::new(Arc::new(order)).map_err(ui_error)?;
        input::send(
            self,
            player,
            InputType::Amount,
            prompt.encode_utf16().collect(),
            script,
        )
        .map_err(ui_error)
    }
    fn input_script(&mut self, player: Player, id: i32, prompt: Vec<u16>) -> input::Result<()> {
        self.bank_player(&player.owner()).map_err(error)?;
        let frame = updates::run_client_script(
            id,
            b"s",
            &[updates::ScriptArg::Text(utf8(&prompt))],
            player.manager().packet_count(1),
        )
        .map_err(|e| input::Error::Binding(e.to_string()))?;
        self.bank_send(&player, frame).map_err(error)
    }
}
impl input::Context for UiContext<'_> {
    type Value = Response;
    type Script = Script;
    type Interpreter = Player;
    type Bank = bank::Bank;
    fn script(&mut self, p: &NodeHandle) -> input::Result<Option<Script>> {
        match self
            .bank_player(p)
            .map_err(error)?
            .attributes()
            .get("runscript")
        {
            None => Ok(None),
            Some(AttributeValue::Object(v)) => Script::new(v).map(Some),
            _ => Err(input::Error::Type),
        }
    }
    fn interaction_locked(&mut self, p: &NodeHandle) -> input::Result<bool> {
        self.bank_player(p).map_err(error)?;
        super::super::super::script_services::locked(
            self.state,
            p.node,
            rs09_server::world::action_locks::Kind::Interaction,
        )
        .map_err(|e| input::Error::Binding(e.to_string()))
    }
    fn input_type(&mut self, p: &NodeHandle) -> input::Result<InputType> {
        match self
            .bank_player(p)
            .map_err(error)?
            .attributes()
            .get("input-type")
        {
            None => Ok(InputType::Numeric),
            Some(AttributeValue::Object(v)) => v
                .as_any()
                .downcast_ref::<InputType>()
                .copied()
                .ok_or(input::Error::Type),
            _ => Err(input::Error::Type),
        }
    }
    fn parse_amount(&mut self, p: &NodeHandle) -> input::Result<bool> {
        self.bank_player(p)
            .map_err(error)?
            .attributes()
            .boolean("parseamount", false)
            .map_err(|_| input::Error::Type)
    }
    fn string(&mut self, value: &Response) -> input::Result<Vec<u16>> {
        Ok(match value {
            Response::Int(n) => n.to_string().encode_utf16().collect(),
            Response::Text(s) => s.clone(),
        })
    }
    fn invoke(&mut self, script: Script, value: Response) -> input::Result<()> {
        if let Some(&order) = script
            .0
            .as_any()
            .downcast_ref::<super::super::shops::Order>()
        {
            let Response::Int(amount) = value else {
                return Err(input::Error::Number);
            };
            let owner = self
                .state
                .retain_node(self.node)
                .map_err(|e| input::Error::Binding(e.to_string()))?;
            let player = self.bank_player(&owner).map_err(error)?;
            return super::super::shops::execute(self, &player, order, Some(amount))
                .map_err(|e| input::Error::Binding(e.to_string()));
        }
        let transfer = script
            .0
            .as_any()
            .downcast_ref::<Transfer>()
            .ok_or_else(|| {
                input::Error::Binding("native input function body is not installed".into())
            })?;
        // Resolve the actual captured player before entering its bank methods.
        self.bank_player(&transfer.player).map_err(error)?;
        input::invoke_transfer(self, transfer, &value)
    }
    fn dialogue(&mut self, player: &NodeHandle, text: Vec<u16>) -> input::Result<()> {
        rs09_server::world::plain_dialogue::send(self, player, &text)
            .map(|_| ())
            .map_err(|e| input::Error::Binding(format!("{e:?}")))
    }
    fn remove(&mut self, p: &NodeHandle, key: &str) -> input::Result<()> {
        self.bank_player(p).map_err(error)?.attributes().remove(key);
        Ok(())
    }
    fn set_parse_amount(&mut self, p: &NodeHandle, value: bool) -> input::Result<()> {
        self.bank_player(p)
            .map_err(error)?
            .attributes()
            .set_boolean("parseamount", value);
        Ok(())
    }
    fn interpreter(&mut self, p: &NodeHandle) -> input::Result<Player> {
        self.bank_player(p).map_err(error)
    }
    fn send_input(&mut self, d: Player, string: bool, prompt: Vec<u16>) -> input::Result<()> {
        self.input_script(d, if string { 109 } else { 108 }, prompt)
    }
    fn send_long(&mut self, d: Player, prompt: Vec<u16>) -> input::Result<()> {
        self.input_script(d, 110, prompt)
    }
    fn send_message(&mut self, d: Player, prompt: Vec<u16>) -> input::Result<()> {
        self.input_script(d, 107, prompt)
    }
    fn set_script(&mut self, p: &NodeHandle, script: Script) -> input::Result<()> {
        self.bank_player(p)
            .map_err(error)?
            .attributes()
            .set_value("runscript", Some(AttributeValue::Object(script.0)));
        Ok(())
    }
    fn set_type(&mut self, p: &NodeHandle, kind: InputType) -> input::Result<()> {
        self.bank_player(p)
            .map_err(error)?
            .attributes()
            .set_value("input-type", Some(AttributeValue::Object(Arc::new(kind))));
        Ok(())
    }
    fn make_transfer(&mut self, transfer: Transfer) -> input::Result<Script> {
        self.bank_player(&transfer.player).map_err(error)?;
        if transfer.after.is_some() {
            return Err(input::Error::Binding(
                "native after function binding is not installed".into(),
            ));
        }
        Script::new(Arc::new(transfer))
    }
    fn bank(&mut self, p: &NodeHandle) -> input::Result<bank::Bank> {
        Ok(self.bank_player(p).map_err(error)?.bank())
    }
    fn take(&mut self, b: bank::Bank, slot: i32, amount: i32) -> input::Result<()> {
        b.take_item(slot, amount, self).map_err(error)
    }
    fn deposit(&mut self, b: bank::Bank, slot: i32, amount: i32) -> input::Result<()> {
        b.add_item(slot, amount, self).map_err(error)
    }
    fn last_amount(&mut self, b: bank::Bank, amount: i32) -> input::Result<()> {
        b.update_last_amount_x(amount, self).map_err(error)
    }
    fn after(&mut self, _: Plugin) -> input::Result<()> {
        Err(input::Error::Binding(
            "native after function body is not installed".into(),
        ))
    }
}
impl rs09_server::world::plain_dialogue::Context for UiContext<'_> {
    type Interpreter = Player;
    type Dispatch = Player;
    type Manager = Manager;
    type Component = Handle;
    fn receiver(&mut self, d: &Player) -> ui::Result<()> {
        self.bank_player(&d.owner())
            .map_err(error)
            .map_err(ui_error)?;
        Ok(())
    }
    fn interpreter(&mut self, p: &NodeHandle) -> ui::Result<Player> {
        self.bank_player(p).map_err(error).map_err(ui_error)
    }
    fn player(&mut self, d: &Player) -> ui::Result<NodeHandle> {
        Ok(d.owner())
    }
    fn dispatch(&mut self, p: &NodeHandle) -> ui::Result<Player> {
        self.bank_player(p).map_err(error).map_err(ui_error)
    }
    fn text(
        &mut self,
        d: Player,
        text: Option<Vec<u16>>,
        iface: i32,
        child: i32,
    ) -> ui::Result<()> {
        let text = text.ok_or(ui::Error::Null)?;
        let frame = updates::interface_text(
            iface << 16 | child,
            &utf8(&text),
            d.manager().packet_count(1),
        )
        .map_err(|e| ui::Error::Binding(e.to_string()))?;
        self.bank_send(&d, frame).map_err(error).map_err(ui_error)
    }
    fn manager(&mut self, p: &NodeHandle) -> ui::Result<Manager> {
        self.bank_player(p)
            .map(|p| p.manager())
            .map_err(error)
            .map_err(ui_error)
    }
    fn open_chatbox(&mut self, m: Manager, id: i32) -> ui::Result<()> {
        self.bank_manager(m, |ctx, m| {
            let c = InterfaceContext::component(ctx, id)?;
            ui::open_chatbox(ctx, m, &c)
        })
        .map_err(error)
        .map_err(ui_error)
    }
    fn chatbox(&mut self, m: Manager) -> ui::Result<Option<Handle>> {
        self.bank_manager(m, |_, m| Ok(m.fields.read().chatbox.clone()))
            .map_err(error)
            .map_err(ui_error)
    }
    fn id(&mut self, c: Handle) -> ui::Result<i32> {
        Ok(c.read().id)
    }
    fn hidden(&mut self, d: Player, id: i32, child: i32, hidden: bool) -> ui::Result<()> {
        let frame =
            updates::interface_hidden(hidden, d.manager().packet_count(1), id << 16 | child);
        self.bank_send(&d, frame).map_err(error).map_err(ui_error)
    }
}
impl rs09_server::world::interface_packets::ContinueContext for UiContext<'_> {
    type DialogueAction = Plugin;
    fn close_chatbox(&mut self, m: Manager) -> ui::Result<()> {
        self.bank_manager(m, |ctx, m| ui::close_chatbox(ctx, m))
            .map_err(error)
            .map_err(ui_error)
    }
    fn pop_action(&mut self, d: Player) -> ui::Result<Option<Plugin>> {
        self.bank_player(&d.owner())
            .map_err(error)
            .map_err(ui_error)?;
        Ok(None)
    }
    fn handle_action(&mut self, _: Plugin, _: &NodeHandle, _: i32) -> ui::Result<()> {
        Err(ui::Error::Binding(
            "dialogue action queue bodies are not installed".into(),
        ))
    }
    fn continue_dialogue(&mut self, _: Player, _: i32, _: i32) -> ui::Result<()> {
        Err(ui::Error::Binding(
            "active content dialogue handling is not installed".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rs09_protocol::{
        buffer::{Reader, Writer},
        game::{actions::ClientAction, updates::ServerUpdate},
    };
    use rs09_server::{definitions::item::Shared, inventory::item::Item};
    fn action(
        p: &mut super::super::super::super::Probe,
        node: super::super::super::super::Node,
        opcode: u8,
        payload: Vec<u8>,
    ) -> ui::Result<()> {
        let action = ClientAction::decode(opcode, &mut Reader::new(&payload))
            .unwrap()
            .unwrap();
        assert!(super::super::super::process(p, node, &action)?);
        Ok(())
    }
    fn button(iface: i32, child: i32, slot: i32) -> Vec<u8> {
        let mut w = Writer::new();
        w.p4(iface << 16 | child);
        w.p2(slot);
        w.into_vec()
    }
    #[test]
    fn actual_x_button_and_short_response_move_canonical_items_update_last_x_and_clear_attributes()
    {
        let (mut p, node, player) = super::super::tests::probe();
        player
            .apply(
                &mut p.state,
                &mut p.host.frames,
                super::super::super::super::now,
                |inventory, defs, ctx| {
                    inventory.add_two(
                        Some(&Shared::new(Item::new(1351, 12, 9, defs))),
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
        p.host.frames.clear();
        action(&mut p, node, 234, button(763, 0, 0)).unwrap();
        let scripts = p.host.frames[&node]
            .iter()
            .filter_map(|f| ServerUpdate::decode(f).unwrap())
            .filter(|u| matches!(u, ServerUpdate::RunClientScript { .. }))
            .collect::<Vec<_>>();
        assert!(
            scripts
                .iter()
                .any(|u| matches!(u, ServerUpdate::RunClientScript { id: 109, .. }))
        );
        assert!(player.attributes().boolean("parseamount", false).unwrap());
        let AttributeValue::Object(callback) = player.attributes().get("runscript").unwrap() else {
            panic!()
        };
        assert_eq!(callback.function_arity(), Some(1));
        assert!(callback.as_any().downcast_ref::<Transfer>().unwrap().player == player.owner());
        action(
            &mut p,
            node,
            244,
            rs09_protocol::login::encode_name(b"5")
                .to_be_bytes()
                .to_vec(),
        )
        .unwrap();
        assert_eq!(
            player
                .bank()
                .container()
                .amount_id(1351, &player.definitions()),
            5
        );
        assert_eq!(player.inventory().amount_id(1351, &player.definitions()), 7);
        assert_eq!(player.bank().last_amount_x(), 5);
        assert_eq!(player.variables().get_varp(1249), Ok(5));
        for key in ["runscript", "parseamount", "input-type"] {
            assert!(!player.attributes().contains_key(key));
        }
        action(&mut p, node, 234, button(762, 73, 0)).unwrap();
        action(&mut p, node, 65, b"2\0".to_vec()).unwrap();
        assert_eq!(
            player
                .bank()
                .container()
                .amount_id(1351, &player.definitions()),
            3
        );
        assert_eq!(player.inventory().amount_id(1351, &player.definitions()), 9);
        assert_eq!(player.bank().last_amount_x(), 2);
        action(&mut p, node, 234, button(762, 73, 0)).unwrap();
        action(&mut p, node, 23, 1i32.to_be_bytes().to_vec()).unwrap();
        assert_eq!(
            player
                .bank()
                .container()
                .amount_id(1351, &player.definitions()),
            2
        );
        assert_eq!(
            player.inventory().amount_id(1351, &player.definitions()),
            10
        );
    }
    #[test]
    fn script_action_lock_defers_real_amount_response_without_consuming_the_pending_callback() {
        use rs09_server::{scripts, world::script_runtime};
        let (mut p, node, player) = super::super::tests::probe();
        player
            .apply(
                &mut p.state,
                &mut p.host.frames,
                super::super::super::super::now,
                |inventory, defs, ctx| {
                    inventory.add_two(
                        Some(&Shared::new(Item::new(1351, 12, 9, defs))),
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
        action(&mut p, node, 234, button(763, 0, 0)).unwrap();
        let owner = player.owner();
        let processor = player.scripts();
        p.state.ticks = 60;
        let mut ctx = script_runtime::Context::new(&p.state, &mut p.host, &processor).unwrap();
        processor.delay_entity(4, &mut p.state, &mut ctx).unwrap();
        let before = player.attributes().get("runscript").unwrap();
        let AttributeValue::Object(before) = before else {
            panic!()
        };
        p.host.frames.clear();
        let payload = rs09_protocol::login::encode_name(b"5")
            .to_be_bytes()
            .to_vec();
        action(&mut p, node, 244, payload.clone()).unwrap();
        assert_eq!(
            player.inventory().amount_id(1351, &player.definitions()),
            12
        );
        assert_eq!(
            player
                .bank()
                .container()
                .amount_id(1351, &player.definitions()),
            0
        );
        assert!(player.attributes().boolean("parseamount", false).unwrap());
        let AttributeValue::Object(after) = player.attributes().get("runscript").unwrap() else {
            panic!()
        };
        assert!(Arc::ptr_eq(&before, &after));
        assert!(p.host.frames.is_empty());
        p.state.ticks = 64;
        action(&mut p, node, 244, payload).unwrap();
        assert_eq!(player.inventory().amount_id(1351, &player.definitions()), 7);
        assert_eq!(
            player
                .bank()
                .container()
                .amount_id(1351, &player.definitions()),
            5
        );
        assert!(!player.attributes().contains_key("runscript"));
        assert_eq!(processor.delay(), 64);
        let mut ctx = script_runtime::Context::new(&p.state, &mut p.host, &processor).unwrap();
        assert!(!scripts::Context::delayed(&mut ctx, &processor, &mut p.state).unwrap());
        assert!(p.state.locks(&owner).is_ok());
    }
    #[test]
    fn invalid_amount_opens_original_plain_error_chatbox_and_continue_restores_default() {
        let (mut p, node, player) = super::super::tests::probe();
        player
            .apply(
                &mut p.state,
                &mut p.host.frames,
                super::super::super::super::now,
                |inventory, defs, ctx| {
                    inventory.add_two(
                        Some(&Shared::new(Item::new(1351, 1, 9, defs))),
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
        action(&mut p, node, 234, button(763, 0, 0)).unwrap();
        p.host.frames.clear();
        action(
            &mut p,
            node,
            244,
            rs09_protocol::login::encode_name(b"oops")
                .to_be_bytes()
                .to_vec(),
        )
        .unwrap();
        assert_eq!(
            player
                .manager()
                .fields
                .read()
                .chatbox
                .as_ref()
                .unwrap()
                .read()
                .id,
            210
        );
        assert_eq!(player.inventory().amount_id(1351, &player.definitions()), 1);
        assert_eq!(
            player
                .bank()
                .container()
                .amount_id(1351, &player.definitions()),
            0
        );
        assert!(player.attributes().get("runscript").is_none());
        assert!(player.attributes().get("parseamount").is_none());
        assert!(player.attributes().get("input-type").is_some());
        let updates = p.host.frames[&node]
            .iter()
            .filter_map(|f| ServerUpdate::decode(f).unwrap())
            .collect::<Vec<_>>();
        let text=updates.iter().position(|u|matches!(u,ServerUpdate::InterfaceText{pointer,text,..}if *pointer==(210<<16|1)&&text==b"That doesn't look right. Please try again.")).unwrap();
        let open = updates
            .iter()
            .position(|u| matches!(u, ServerUpdate::OpenInterface { interface: 210, .. }))
            .unwrap();
        let hidden=updates.iter().position(|u|matches!(u,ServerUpdate::InterfaceHidden{pointer,hidden:0,..}if *pointer==(210<<16|1))).unwrap();
        assert!(text < open && open < hidden);
        let continue_option = ClientAction::ContinueOption {
            opcode: 132,
            iface: 210,
            child: 1,
            slot: -1,
        };
        assert!(super::super::super::process(&mut p, node, &continue_option).unwrap());
        assert_eq!(
            player
                .manager()
                .fields
                .read()
                .chatbox
                .as_ref()
                .unwrap()
                .read()
                .id,
            137
        );
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
    }
    #[test]
    fn pending_actual_input_function_cycle_collects_and_external_alias_keeps_only_its_player() {
        for keep in [false, true] {
            let (mut p, node, player) = super::super::tests::probe();
            player
                .apply(
                    &mut p.state,
                    &mut p.host.frames,
                    super::super::super::super::now,
                    |inventory, defs, ctx| {
                        inventory.add_two(
                            Some(&Shared::new(Item::new(1351, 1, 9, defs))),
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
            action(&mut p, node, 234, button(763, 0, 0)).unwrap();
            let weak = player.downgrade();
            let AttributeValue::Object(callback) = player.attributes().get("runscript").unwrap()
            else {
                panic!()
            };
            let held = keep.then(|| callback.clone());
            drop(callback);
            drop(player);
            p.remove(1, node).unwrap();
            for _ in 0..2 {
                super::super::super::super::tick::handle(&mut p, false).unwrap();
                super::super::super::reap(&mut p.host, &p.state);
                p.state.collect();
            }
            assert_eq!(p.state.actor(node).is_ok(), keep);
            drop(held);
            p.state.collect();
            assert!(p.state.actor(node).is_err());
            assert!(weak.upgrade().is_none());
        }
    }
    #[test]
    fn raw_foreign_input_capture_rejects_before_bank_mutation_and_packet_delivery() {
        let (mut p, node, player) = super::super::tests::probe();
        let (other, _, foreign) = super::super::tests::probe();
        let owner = player.owner();
        let other_owner = foreign.owner();
        assert_eq!(owner.node, other_owner.node);
        assert!(owner != other_owner);
        player.attributes().set_value(
            "runscript",
            Some(AttributeValue::Object(Arc::new(Transfer {
                player: other_owner,
                slot: 0,
                withdraw: false,
                after: None,
            }))),
        );
        player.attributes().set_value(
            "input-type",
            Some(AttributeValue::Object(Arc::new(InputType::Amount))),
        );
        player.attributes().set_boolean("parseamount", true);
        p.host.frames.clear();
        let before = foreign.bank().last_amount_x();
        assert!(action(&mut p, node, 65, b"5\0".to_vec()).is_err());
        assert_eq!(foreign.bank().last_amount_x(), before);
        assert!(p.host.frames.is_empty());
        assert!(other.host.frames.values().all(Vec::is_empty));
        for key in ["runscript", "input-type", "parseamount"] {
            assert!(player.attributes().get(key).is_none());
        }
    }
    #[test]
    fn close_removes_current_script_but_retained_callback_uses_captured_closed_bank() {
        let (mut p, node, player) = super::super::tests::probe();
        player
            .apply(
                &mut p.state,
                &mut p.host.frames,
                super::super::super::super::now,
                |inventory, defs, ctx| {
                    inventory.add_two(
                        Some(&Shared::new(Item::new(1351, 1, 9, defs))),
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
        action(&mut p, node, 234, button(763, 0, 0)).unwrap();
        let AttributeValue::Object(retained) = player.attributes().get("runscript").unwrap() else {
            panic!()
        };
        action(&mut p, node, 184, vec![]).unwrap();
        assert!(!player.bank().is_open());
        assert!(player.attributes().get("runscript").is_none());
        let last = player.bank().last_amount_x();
        action(
            &mut p,
            node,
            244,
            rs09_protocol::login::encode_name(b"1k")
                .to_be_bytes()
                .to_vec(),
        )
        .unwrap();
        assert_eq!(player.bank().last_amount_x(), last);
        assert!(player.attributes().boolean("parseamount", false).unwrap());
        input::Context::invoke(
            &mut UiContext::new(&mut p, node),
            Script::new(retained).unwrap(),
            Response::Int(3),
        )
        .unwrap();
        assert_eq!(player.bank().last_amount_x(), 3);
        // Original BankContainer.addItem does not require an open interface;
        // only the ordinary late packet loses its current stored function.
        assert_eq!(player.inventory().amount_id(1351, &player.definitions()), 0);
        assert_eq!(
            player
                .bank()
                .container()
                .amount_id(1351, &player.definitions()),
            1
        );
    }
}
