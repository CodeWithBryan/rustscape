//! Original MiniMenu legacy buttons and magic-varp refresh, with explicit services.
pub trait Context {
    type Component;
    type Error;
    fn packet(&mut self, component: i32) -> Result<(), Self::Error>;
    fn lookup(&mut self, id: i32) -> Result<Self::Component, Self::Error>;
    fn client_code(&mut self, component: &Self::Component) -> i32;
    fn script_opcode(&mut self, component: &Self::Component) -> Result<Option<i32>, Self::Error>;
    fn script_varp(&mut self, component: &Self::Component) -> Result<i32, Self::Error>;
    fn operand(&mut self, component: &Self::Component) -> Result<i32, Self::Error>;
    fn value(&mut self, id: i32) -> Result<i32, Self::Error>;
    fn set_value(&mut self, id: i32, value: i32) -> Result<(), Self::Error>;
    fn redraw_active(&mut self) -> Result<(), Self::Error>;
    fn ambient_multi(&mut self) -> Result<(), Self::Error>;
    fn varp_code(&mut self, id: i32) -> Result<i32, Self::Error>;
    fn set_mode(&mut self, code: i32, value: i32);
    fn logout_timer(&mut self) -> &mut i32;
}
pub fn special(code: i32, timer: &mut i32) -> bool {
    if code == 205 {
        *timer = 250;
        true
    } else {
        false
    }
}
pub fn advance_logout_timer(timer: &mut i32) {
    if *timer > 0 {
        *timer -= 1;
    }
}
pub fn refresh<C: Context>(ctx: &mut C, id: i32) -> Result<(), C::Error> {
    ctx.redraw_active()?;
    ctx.ambient_multi()?;
    let code = ctx.varp_code(id)?;
    if code == 0 {
        return Ok(());
    }
    let value = ctx.value(id)?;
    ctx.set_mode(code, value);
    Ok(())
}
pub fn run<C: Context>(ctx: &mut C, action: i32, id: i32) -> Result<(), C::Error> {
    if matches!(action, 51 | 59) {
        ctx.packet(id)?;
        let component = ctx.lookup(id)?;
        if ctx.script_opcode(&component)? == Some(5) {
            let index = ctx.script_varp(&component)?;
            if action == 51 {
                let old = ctx.value(index)?;
                let operand = ctx.operand(&component)?;
                if old != operand {
                    ctx.set_value(index, operand)?;
                    refresh(ctx, index)?;
                }
            } else {
                let value = 1i32.wrapping_sub(ctx.value(index)?);
                ctx.set_value(index, value)?;
                refresh(ctx, index)?;
            }
        }
    }
    if action == 8 {
        let component = ctx.lookup(id)?;
        let approved = if ctx.client_code(&component) > 0 {
            let code = ctx.client_code(&component);
            special(code, ctx.logout_timer())
        } else {
            true
        };
        if approved {
            ctx.packet(id)?;
        }
    }
    Ok(())
}

struct Native<'a, F> {
    state: &'a mut super::state::State,
    vm: &'a mut super::vm::Vm,
    emit: F,
}
impl<F: FnMut(rs09_protocol::game::Frame) -> Result<(), String>> Context for Native<'_, F> {
    type Component = super::component_handles::Handle;
    type Error = String;
    fn packet(&mut self, id: i32) -> Result<(), String> {
        (self.emit)(
            super::actions::item_packet(8, 0, 0, id).expect("Original legacy button writer"),
        )
    }
    fn lookup(&mut self, id: i32) -> Result<Self::Component, String> {
        self.state
            .lookup_reference(super::state::Key::component(id))?
            .ok_or_else(|| "Null legacy button component".into())
    }
    fn client_code(&mut self, c: &Self::Component) -> i32 {
        self.state.component_ref(c).client_code
    }
    fn script_opcode(&mut self, c: &Self::Component) -> Result<Option<i32>, String> {
        let Some(scripts) = &self.state.component_ref(c).cs1_scripts else {
            return Ok(None);
        };
        scripts
            .first()
            .and_then(|s| s.first())
            .copied()
            .map(Some)
            .ok_or_else(|| "Legacy button CS1 opcode bounds".into())
    }
    fn script_varp(&mut self, c: &Self::Component) -> Result<i32, String> {
        self.state
            .component_ref(c)
            .cs1_scripts
            .as_ref()
            .and_then(|s| s.first())
            .and_then(|s| s.get(1))
            .copied()
            .ok_or_else(|| "Legacy button CS1 varp bounds".into())
    }
    fn operand(&mut self, c: &Self::Component) -> Result<i32, String> {
        self.state
            .component_ref(c)
            .cs1_comparison_operands
            .as_ref()
            .and_then(|v| v.first())
            .copied()
            .ok_or_else(|| "Legacy button operand bounds".into())
    }
    fn value(&mut self, id: i32) -> Result<i32, String> {
        if !(0..3500).contains(&id) {
            return Err("Legacy button active varp bounds".into());
        }
        Ok(self.vm.varps.get(&id).copied().unwrap_or(0))
    }
    fn set_value(&mut self, id: i32, value: i32) -> Result<(), String> {
        if !(0..3500).contains(&id) {
            return Err("Legacy button active varp bounds".into());
        }
        self.vm.varps.insert(id, value);
        self.vm.last_legacy_varp = Some(id);
        self.vm.legacy_varp_writes = self.vm.legacy_varp_writes.wrapping_add(1);
        Ok(())
    }
    fn redraw_active(&mut self) -> Result<(), String> {
        // The current GPU backend redraws the complete frame. Keep its request
        // observable without claiming the original dirty-rectangle scheduler.
        self.state.legacy_button_redraws = self.state.legacy_button_redraws.wrapping_add(1);
        Ok(())
    }
    fn ambient_multi(&mut self) -> Result<(), String> {
        #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
        self.vm.ambient_updates.push(self.vm.varps.clone());
        Ok(())
    }
    fn varp_code(&mut self, id: i32) -> Result<i32, String> {
        super::vm::Vm::varp_client_code(self.state, id)
    }
    fn set_mode(&mut self, code: i32, value: i32) {
        match code {
            5 => self.vm.mouse_buttons = value,
            6 => self.vm.chat_effects_disabled = value,
            9 => self.vm.inserting = value,
            _ => {}
        }
    }
    fn logout_timer(&mut self) -> &mut i32 {
        &mut self.vm.logout_on_disconnect_timer
    }
}
impl super::vm::Vm {
    pub fn legacy_button(
        &mut self,
        state: &mut super::state::State,
        action: i32,
        id: i32,
        emit: impl FnMut(rs09_protocol::game::Frame) -> Result<(), String>,
    ) -> Result<(), String> {
        run(
            &mut Native {
                state,
                vm: self,
                emit,
            },
            action,
            id,
        )
    }
    pub fn legacy_button_snapshot(&self, state: &super::state::State) -> serde_json::Value {
        serde_json::json!({"logout_timer":self.logout_on_disconnect_timer,"mouse_buttons":self.mouse_buttons,"chat_effects_disabled":self.chat_effects_disabled,"inserting":self.inserting,"redraws":state.legacy_button_redraws,"varp_writes":self.legacy_varp_writes,"last_varp":self.last_legacy_varp.map(|id|serde_json::json!({"id":id,"value":self.varps.get(&id).copied().unwrap_or(0)}))})
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        pack::Pack,
        ui::{
            Component, Properties,
            state::{Key, State},
            vm::Vm,
        },
    };
    fn setup() -> (State, Vm, Key) {
        let manifest=serde_json::to_vec(&serde_json::json!({"source":"Native legacy button binding","files":{"varptype/0":[0,4],"varptype/1":[4,4],"varptype/2":[8,1],"varptype/3":[9,4]}})).unwrap();
        let mut b = b"RS09VIS1".to_vec();
        b.extend((manifest.len() as u32).to_le_bytes());
        b.extend(manifest);
        b.extend([5, 0, 5, 0, 5, 0, 6, 0, 5, 5, 0, 9, 0]);
        let mut state = State::new(Pack::decode(b).unwrap());
        state.loaded.insert(1);
        let key = Key::component(1 << 16);
        state.components.insert(
            key,
            Component {
                id: key.id,
                cs1_scripts: Some(vec![vec![5, 0, 0]]),
                cs1_comparison_operands: Some(vec![1]),
                ..Default::default()
            },
        );
        (state, Vm::default(), key)
    }
    #[test]
    fn native_legacy_controls_update_modes_without_a_pending_varp_timer() {
        let (mut state, mut vm, key) = setup();
        let mut frames = vec![];
        for _ in 0..2 {
            vm.legacy_button(&mut state, 59, key.id, |f| {
                frames.push(f);
                Ok(())
            })
            .unwrap();
        }
        assert_eq!(vm.varps[&0], 0);
        assert_eq!(vm.mouse_buttons, 0);
        assert_eq!(state.legacy_button_redraws, 2);
        let c = state.get_mut(key).unwrap();
        c.cs1_scripts = Some(vec![vec![5, 1, 0]]);
        c.cs1_comparison_operands = Some(vec![7]);
        for _ in 0..2 {
            vm.legacy_button(&mut state, 51, key.id, |f| {
                frames.push(f);
                Ok(())
            })
            .unwrap();
        }
        assert_eq!(vm.varps[&1], 7);
        assert_eq!(vm.chat_effects_disabled, 7);
        assert_eq!(state.legacy_button_redraws, 3);
        assert_eq!(vm.legacy_varp_writes, 3);
        state.get_mut(key).unwrap().cs1_scripts = Some(vec![vec![5, 3, 0]]);
        vm.legacy_button(&mut state, 59, key.id, |f| {
            frames.push(f);
            Ok(())
        })
        .unwrap();
        assert_eq!(vm.inserting, 1);
        assert_eq!(vm.legacy_varp_writes, 4);
        assert_eq!(state.legacy_button_redraws, 4);
        assert!(vm.pending_varps.poll(&mut vm.varps, 10000).is_empty());
        assert_eq!(vm.varps[&1], 7);
        assert_eq!(state.varp_writer, 0);
        assert_eq!(frames.len(), 5);
        assert!(
            frames
                .iter()
                .all(|f| f.opcode == 10 && f.payload == key.id.to_be_bytes())
        );
        #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
        {
            assert_eq!(vm.ambient_updates.len(), 4);
            assert_eq!(vm.ambient_updates[0][&0], 1);
            assert_eq!(vm.ambient_updates[1][&0], 0);
            assert_eq!(vm.ambient_updates[2][&1], 7);
        }
    }
    #[test]
    fn native_button_errors_keep_the_completed_packet_and_varp_prefix() {
        let (mut state, mut vm, key) = setup();
        let mut frames = vec![];
        let missing = key.id + 1;
        assert!(
            vm.legacy_button(&mut state, 8, missing, |f| {
                frames.push(f);
                Ok(())
            })
            .is_err()
        );
        assert!(frames.is_empty());
        assert!(
            vm.legacy_button(&mut state, 51, missing, |f| {
                frames.push(f);
                Ok(())
            })
            .is_err()
        );
        assert_eq!(frames.len(), 1);
        assert!(
            vm.legacy_button(&mut state, 59, key.id, |_| Err("Emitter failed".into()))
                .is_err()
        );
        assert!(vm.varps.is_empty());
        state.get_mut(key).unwrap().cs1_scripts = Some(vec![vec![5, 2]]);
        assert!(
            vm.legacy_button(&mut state, 59, key.id, |f| {
                frames.push(f);
                Ok(())
            })
            .is_err()
        );
        assert_eq!(vm.varps[&2], 1);
        assert_eq!(state.legacy_button_redraws, 1);
        assert_eq!(vm.mouse_buttons, 0);
        #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
        assert_eq!(vm.ambient_updates[0][&2], 1);
        state.get_mut(key).unwrap().cs1_scripts = Some(vec![vec![5, 3500]]);
        assert!(
            vm.legacy_button(&mut state, 59, key.id, |f| {
                frames.push(f);
                Ok(())
            })
            .is_err()
        );
        assert!(!vm.varps.contains_key(&3500));
        state.get_mut(key).unwrap().client_code = 205;
        vm.logout_on_disconnect_timer = 17;
        assert!(
            vm.legacy_button(&mut state, 8, key.id, |_| Err("Emitter failed".into()))
                .is_err()
        );
        assert_eq!(vm.logout_on_disconnect_timer, 250);
    }
    #[test]
    fn native_if3_special205_runs_before_permission_and_shares_the_legacy_handler() {
        let (mut state, mut vm, key) = setup();
        let c = state.get_mut(key).unwrap();
        c.client_code = 205;
        c.properties = Properties {
            events: 0,
            target_param: -1,
        };
        vm.click_button(&mut state, key, 1, "Logout").unwrap();
        assert_eq!(vm.logout_on_disconnect_timer, 250);
        assert!(vm.outgoing.is_empty());
        state.get_mut(key).unwrap().properties.events = 2;
        vm.click_button(&mut state, key, 1, "Logout").unwrap();
        assert_eq!(vm.outgoing[0].opcode, 155);
        vm.outgoing.clear();
        state.get_mut(key).unwrap().client_code = 206;
        vm.logout_on_disconnect_timer = 17;
        vm.click_button(&mut state, key, 1, "Unknown special")
            .unwrap();
        assert!(vm.outgoing.is_empty());
        assert_eq!(vm.logout_on_disconnect_timer, 17);
        let mut frames = vec![];
        vm.legacy_button(&mut state, 8, key.id, |f| {
            frames.push(f);
            Ok(())
        })
        .unwrap();
        assert!(frames.is_empty());
        state.get_mut(key).unwrap().client_code = 205;
        vm.legacy_button(&mut state, 8, key.id, |f| {
            frames.push(f);
            Ok(())
        })
        .unwrap();
        assert_eq!(vm.logout_on_disconnect_timer, 250);
        assert_eq!(frames[0].opcode, 10);
    }
}
