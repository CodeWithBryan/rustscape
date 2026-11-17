//! The interface side of `client.resetGameSessionState`,
//! `InterfaceList.resetToLoginScreen(true)` and `LoginManager.reconnect`.
use super::{Key, State};

impl State {
    /// `InterfaceList.unload`: drop a group's components, except that a group
    /// holding a type 2 (inventory) component keeps those.
    fn unload_group(&mut self, group: i32) {
        if group == -1 || !self.loaded.contains(&group) {
            return;
        }
        let removed: Vec<Key> = self
            .components
            .iter()
            .filter(|(key, c)| key.id >> 16 == group && !(key.child == -1 && c.kind == 2))
            .map(|(&key, _)| key)
            .collect();
        for key in removed {
            self.remove_component(key);
        }
        self.created_lengths.retain(|id, _| id >> 16 != group);
        self.drag_parents.retain(|key, _| key.id >> 16 != group);
        self.loaded.remove(&group);
    }

    /// The open sub-interface pointers in the order `HashTable(8)` yields
    /// them: by bucket (`key & 7`). Inside one bucket the original keeps
    /// insertion order, which this state does not record; keys ascend here.
    fn open_pointers(&self) -> Vec<i32> {
        let mut pointers: Vec<i32> = self.subinterfaces.keys().copied().collect();
        pointers.sort_by_key(|&pointer| (pointer & 7, pointer));
        pointers
    }

    /// The interface block shared by `client.resetGameSessionState`
    /// (client.java:709-718) and `InterfaceList.resetToLoginScreen(true)`
    /// (InterfaceList.java:1088-1097):
    ///
    /// ```text
    /// if (topLevelInterface != -1) unload(topLevelInterface);
    /// for (each open pointer) closeInterface(true, pointer);
    /// topLevelInterface = -1;
    /// openInterfaces = new HashTable(8);
    /// InterfaceList.reset();
    /// ```
    ///
    /// `closeInterface` ends with `runScripts(1, topLevelInterface)` while the
    /// top level is still set, and `runScripts` loads that interface again, so
    /// the open/close hooks of a freshly decoded top level run once per closed
    /// sub-interface before everything is discarded. `run_hooks` is called
    /// after each close to run what was queued (`Vm::drain`).
    ///
    /// The server-set properties of the top level's components are not
    /// removed: the original only removes those of the closed sub-interfaces.
    pub fn close_everything(&mut self, mut run_hooks: impl FnMut(&mut State)) {
        let root = self.root;
        if root != -1 {
            self.unload_group(root);
        }
        for pointer in self.open_pointers() {
            if root != -1 {
                // `runScripts` -> `load(topLevelInterface)`; a top level the
                // cache cannot supply runs nothing, as in the original.
                let _ = self.load_group(root);
            }
            self.close_sub(pointer, true);
            run_hooks(self);
        }
        self.root = -1;
        self.subinterfaces.clear();
        // InterfaceList.reset(): every component and loaded flag is dropped.
        let keys: Vec<Key> = self.components.keys().copied().collect();
        for key in keys {
            self.remove_component(key);
        }
        self.loaded.clear();
        self.created_lengths.clear();
        self.drag_parents.clear();
        self.hooks.clear();
    }

    /// The interface statics `client.resetGameSessionState` clears after it
    /// has closed the interfaces: `Cs1ScriptRunner.pleaseWaitComponent =
    /// null`, `Inv.clear()` and `Protocol.verifyId = 0`.
    pub fn reset_session_statics(&mut self) {
        self.clear_waiting();
        self.containers.clear();
        self.verify_id = 0;
    }
}
