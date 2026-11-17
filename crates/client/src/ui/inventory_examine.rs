//! Original MiniMenu inventory Examine branch; the caller owns packet/chat services.
pub trait Context {
    type Component;
    type Error;
    fn lookup(&mut self, id: i32) -> Result<Option<Self::Component>, Self::Error>;
    fn count(&mut self, component: &Self::Component, slot: i32) -> Result<i32, Self::Error>;
    fn packet(&mut self, id: i32) -> Result<(), Self::Error>;
    fn name(&mut self, id: i32) -> Result<Option<String>, Self::Error>;
    fn message(&mut self, text: String) -> Result<(), Self::Error>;
    fn reset_tick(&mut self);
    fn set_pressed(&mut self, component: Option<Self::Component>);
    fn set_slot(&mut self, slot: i32);
}
pub fn run<C: Context>(ctx: &mut C, id: i32, slot: i32, component_id: i32) -> Result<(), C::Error> {
    let component = ctx.lookup(component_id)?;
    if match &component {
        None => true,
        Some(c) => ctx.count(c, slot)? < 100000,
    } {
        ctx.packet(id)?;
    } else {
        // The original evaluates the quantity string before the item definition.
        let count = ctx.count(component.as_ref().unwrap(), slot)?;
        let name = ctx.name(id)?.unwrap_or_else(|| "null".into());
        ctx.message(format!("{count} x {name}"))?;
    }
    ctx.reset_tick();
    let pressed = ctx.lookup(component_id)?;
    ctx.set_pressed(pressed);
    ctx.set_slot(slot);
    Ok(())
}

struct Native<'a, F> {
    state: &'a mut super::state::State,
    vm: &'a mut super::vm::Vm,
    emit: F,
    slot: i32,
    component: i32,
}
impl<F: FnMut(rs09_protocol::game::Frame) -> Result<(), String>> Context for Native<'_, F> {
    type Component = super::component_handles::Handle;
    type Error = String;
    fn lookup(&mut self, id: i32) -> Result<Option<Self::Component>, String> {
        self.state
            .lookup_reference(super::state::Key::component(id))
    }
    fn count(&mut self, c: &Self::Component, slot: i32) -> Result<i32, String> {
        self.state
            .component_ref(c)
            .obj_counts
            .as_ref()
            .ok_or("Missing inventory count array")?
            .get(slot as usize)
            .copied()
            .ok_or_else(|| "Invalid inventory Examine slot".into())
    }
    fn packet(&mut self, id: i32) -> Result<(), String> {
        (self.emit)(
            super::actions::item_packet(1006, id, self.slot, self.component)
                .expect("Original Examine writer"),
        )
    }
    fn name(&mut self, id: i32) -> Result<Option<String>, String> {
        Ok(Some(self.state.items.get(&self.state.pack, id)?.name))
    }
    fn message(&mut self, text: String) -> Result<(), String> {
        self.vm.add_message(
            self.state,
            super::vm::ChatLine {
                text,
                phrase: -1,
                ..Default::default()
            },
        );
        Ok(())
    }
    fn reset_tick(&mut self) {
        self.state.inventory_press.tick = 0;
    }
    fn set_pressed(&mut self, c: Option<Self::Component>) {
        self.state.inventory_press.component = c;
    }
    fn set_slot(&mut self, slot: i32) {
        self.state.inventory_press.slot = slot;
    }
}
impl super::vm::Vm {
    /// Complete inventory Examine branch. Emission happens before the feedback
    /// reset/late lookup, so a later failure retains the completed packet/chat.
    pub fn examine_inventory(
        &mut self,
        state: &mut super::state::State,
        id: i32,
        slot: i32,
        component: i32,
        emit: impl FnMut(rs09_protocol::game::Frame) -> Result<(), String>,
    ) -> Result<(), String> {
        run(
            &mut Native {
                state,
                vm: self,
                emit,
                slot,
                component,
            },
            id,
            slot,
            component,
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        pack::Pack,
        ui::{
            Component,
            state::{Key, State},
            vm::Vm,
        },
    };
    fn setup(count: Option<Vec<i32>>) -> (State, Vm, Key) {
        let item = b"\x02Coins\0\x0b\0";
        let manifest=serde_json::to_vec(&serde_json::json!({"source":"Native inventory Examine binding","files":{"item/995":[0,item.len()]}})).unwrap();
        let mut b = b"RS09VIS1".to_vec();
        b.extend((manifest.len() as u32).to_le_bytes());
        b.extend(manifest);
        b.extend(item);
        let mut state = State::new(Pack::decode(b).unwrap());
        state.loaded.insert(1);
        let key = Key::component(1 << 16);
        state.components.insert(
            key,
            Component {
                id: key.id,
                obj_counts: count,
                ..Default::default()
            },
        );
        (state, Vm::default(), key)
    }
    #[test]
    fn real_vm_threshold_chat_and_null_component_packet_bindings() {
        let (mut state, mut vm, key) = setup(Some(vec![99999]));
        let mut sent = vec![];
        vm.examine_inventory(&mut state, 995, 0, key.id, |f| {
            sent.push(f);
            Ok(())
        })
        .unwrap();
        assert_eq!(sent[0].opcode, 92);
        assert_eq!(sent[0].payload, [99, 3]);
        assert!(vm.messages.is_empty());
        state.get_mut(key).unwrap().obj_counts = Some(vec![100000]);
        state.transmit_timer = 777;
        sent.clear();
        vm.examine_inventory(&mut state, 995, 0, key.id, |f| {
            sent.push(f);
            Ok(())
        })
        .unwrap();
        assert!(sent.is_empty());
        assert_eq!(vm.messages[0].text, "100000 x Coins");
        assert_eq!((vm.messages[0].kind, vm.messages[0].phrase), (0, -1));
        assert!(vm.messages[0].name.is_empty());
        assert_eq!(state.chat_transmit_at, 777);
        assert_eq!(state.inventory_pressed_slot(key), Some(0));
        vm.examine_inventory(&mut state, 995, 5, key.id | 17, |f| {
            sent.push(f);
            Ok(())
        })
        .unwrap();
        assert_eq!(sent.len(), 1);
        assert!(!state.inventory_press_active());
        assert_eq!(state.inventory_press_snapshot()["slot"], 5);
    }
    #[test]
    fn failed_count_or_emit_keeps_feedback_prefix_unchanged() {
        let (mut state, mut vm, key) = setup(None);
        state.capture_inventory_press(47, key.id, 2).unwrap();
        for _ in 0..7 {
            state.advance_inventory_press();
        }
        let mut sent = vec![];
        assert!(
            vm.examine_inventory(&mut state, 995, 0, key.id, |f| {
                sent.push(f);
                Ok(())
            })
            .is_err()
        );
        assert!(sent.is_empty());
        assert!(vm.messages.is_empty());
        assert_eq!(state.inventory_press_snapshot()["tick"], 7);
        assert_eq!(state.inventory_pressed_slot(key), Some(2));
        state.get_mut(key).unwrap().obj_counts = Some(vec![1]);
        assert!(
            vm.examine_inventory(&mut state, 995, 0, key.id, |f| {
                sent.push(f);
                Err("Injected emitter failure".into())
            })
            .is_err()
        );
        assert_eq!(sent.len(), 1);
        assert_eq!(state.inventory_press_snapshot()["tick"], 7);
        assert_eq!(state.inventory_pressed_slot(key), Some(2));
    }
}
