//! Original inventory action feedback fields and Protocol's fifteen-tick expiry.
#[derive(Debug)]
pub struct Press<T> {
    pub component: Option<T>,
    pub slot: i32,
    pub tick: i32,
}
impl<T> Default for Press<T> {
    fn default() -> Self {
        Self {
            component: None,
            slot: 0,
            tick: 0,
        }
    }
}
impl<T> Press<T> {
    /// Assignment order matters: a failed lookup preserves the old reference
    /// and slot, but has already reset the shared interaction counter.
    pub fn assign<E>(
        &mut self,
        slot: i32,
        lookup: impl FnOnce() -> Result<Option<T>, E>,
    ) -> Result<(), E> {
        self.tick = 0;
        self.component = lookup()?;
        self.slot = slot;
        Ok(())
    }
    pub fn after_action<E>(&self, mut redraw: impl FnMut(&T) -> Result<(), E>) -> Result<(), E> {
        if let Some(c) = &self.component {
            if self.tick == 0 {
                redraw(c)?;
            }
        }
        Ok(())
    }
    pub fn advance<E>(&mut self, mut redraw: impl FnMut(&T) -> Result<(), E>) -> Result<(), E> {
        if let Some(c) = &self.component {
            self.tick = self.tick.wrapping_add(1);
            if self.tick >= 15 {
                redraw(c)?;
                self.component = None;
            }
        }
        Ok(())
    }
    pub fn released(&mut self) {
        self.tick = 10;
    }
}

/// The caller has already applied MiniMenu's one-time action normalization.
pub fn handles_action(action: i32) -> bool {
    matches!(
        action,
        3 | 5 | 7 | 13 | 23 | 25 | 35 | 40 | 43 | 47 | 48 | 58 | 1006
    )
}

impl super::state::State {
    pub fn capture_inventory_press(
        &mut self,
        action: i32,
        id: i32,
        slot: i32,
    ) -> Result<(), String> {
        if handles_action(action) {
            // Do not move this state out while lookup may load component groups.
            self.inventory_press.tick = 0;
            self.inventory_press.component =
                self.lookup_reference(super::state::Key::component(id))?;
            self.inventory_press.slot = slot;
        }
        Ok(())
    }
    pub fn finish_inventory_action(&mut self) {
        let redraws = &mut self.inventory_redraws;
        self.inventory_press
            .after_action(|_| {
                *redraws = redraws.wrapping_add(1);
                Ok::<_, std::convert::Infallible>(())
            })
            .unwrap();
    }
    pub fn advance_inventory_press(&mut self) {
        let redraws = &mut self.inventory_redraws;
        self.inventory_press
            .advance(|_| {
                *redraws = redraws.wrapping_add(1);
                Ok::<_, std::convert::Infallible>(())
            })
            .unwrap();
    }
    pub fn released_inventory(&mut self) {
        self.inventory_press.released();
    }
    pub fn inventory_press_active(&self) -> bool {
        self.inventory_press.component.is_some()
    }
    pub fn inventory_pressed_slot(&self, key: super::state::Key) -> Option<i32> {
        self.inventory_press
            .component
            .as_ref()
            .filter(|h| h.key() == key && self.component_is_current(h))
            .map(|_| self.inventory_press.slot)
    }
    pub fn inventory_press_snapshot(&self) -> serde_json::Value {
        serde_json::json!({"active":self.inventory_press_active(),"component":self.inventory_press.component.as_ref().map(|h|h.key()),"current":self.inventory_press.component.as_ref().is_some_and(|h|self.component_is_current(h)),"slot":self.inventory_press.slot,"tick":self.inventory_press.tick,"redraws":self.inventory_redraws})
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        pack::Pack,
        ui::{
            Component,
            state::{Key, State},
        },
    };
    fn state() -> (State, Key) {
        let m = br#"{"source":"pressed component identity test","files":{}}"#;
        let mut raw = b"RS09VIS1".to_vec();
        raw.extend((m.len() as u32).to_le_bytes());
        raw.extend(m);
        let mut s = State::new(Pack::decode(raw).unwrap());
        let key = Key::component(1 << 16);
        s.loaded.insert(1);
        s.components.insert(
            key,
            Component {
                id: key.id,
                obj_counts: Some(vec![1]),
                ..Default::default()
            },
        );
        (s, key)
    }
    #[test]
    fn replaced_component_does_not_inherit_feedback_and_expiry_redraws_old_object() {
        let (mut s, key) = state();
        s.capture_inventory_press(47, key.id, 0).unwrap();
        s.finish_inventory_action();
        let old = s.inventory_press.component.clone().unwrap();
        assert_eq!(s.inventory_pressed_slot(key), Some(0));
        s.replace_component(
            key,
            Component {
                id: key.id,
                obj_counts: Some(vec![99]),
                ..Default::default()
            },
        );
        assert_eq!(s.inventory_pressed_slot(key), None);
        assert_eq!(s.component_ref(&old).obj_counts, Some(vec![1]));
        for _ in 0..14 {
            s.advance_inventory_press();
        }
        assert!(s.inventory_press_active());
        s.advance_inventory_press();
        assert!(!s.inventory_press_active());
        assert_eq!(s.inventory_redraws, 2);
        assert_eq!(s.get(key).unwrap().obj_counts, Some(vec![99]));
    }
    #[test]
    fn failed_lookup_keeps_old_lease_and_slot_after_counter_reset() {
        let (mut s, key) = state();
        s.capture_inventory_press(5, key.id, 3).unwrap();
        for _ in 0..7 {
            s.advance_inventory_press();
        }
        assert!(s.capture_inventory_press(5, 2 << 16, 9).is_err());
        assert_eq!(s.inventory_press.tick, 0);
        assert_eq!(s.inventory_pressed_slot(key), Some(3));
        s.released_inventory();
        for _ in 0..4 {
            s.advance_inventory_press();
        }
        assert!(s.inventory_press_active());
        s.advance_inventory_press();
        assert!(!s.inventory_press_active());
    }
}
