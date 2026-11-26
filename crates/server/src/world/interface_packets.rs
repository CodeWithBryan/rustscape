//! Pinned PacketProcessor IfAction/CloseIface bodies and their captured login
//! pulse. Dialogue, locks, zones, events, scheduling and login remain services.
use super::{
    component_objects::{Captures, NativeObject, Plugin},
    interface_listeners::Button,
    interfaces::{Handle, Result},
    player_walk::NodeHandle,
};
#[derive(Clone, Copy, Debug)]
pub struct Action {
    pub iface: i32,
    pub child: i32,
    pub slot: i32,
    pub item: i32,
    pub opcode: i32,
    pub opt_index: i32,
}
impl Action {
    pub fn button(self) -> Button {
        Button {
            opcode: self.opcode,
            button: self.child,
            slot: self.slot,
            item: self.item,
        }
    }
}
#[derive(Clone)]
pub struct LoginPulse {
    player: NodeHandle,
}
impl NativeObject for LoginPulse {
    fn captures(&self) -> Captures {
        Captures {
            nodes: vec![self.player.clone()],
            ..Default::default()
        }
    }
}
impl LoginPulse {
    pub fn player(&self) -> NodeHandle {
        self.player.clone()
    }
    pub fn pulse(&self, ctx: &mut impl Context) -> Result<bool> {
        ctx.remove_logging(&self.player)?;
        ctx.configure_world(&self.player)?;
        Ok(true)
    }
}
pub trait Context {
    type Dialogue;
    type Locks;
    type Zone;
    type Manager;
    type Dispatch;
    fn debug(&mut self, p: &NodeHandle, text: String) -> Result<()>;
    fn dialogue(&mut self, p: &NodeHandle) -> Result<Self::Dialogue>;
    fn has_dialogue(&mut self, d: Self::Dialogue) -> Result<bool>;
    fn close_dialogue(&mut self, d: Self::Dialogue) -> Result<()>;
    fn locks(&mut self, p: &NodeHandle) -> Result<Self::Locks>;
    fn component_locked(&mut self, l: Self::Locks) -> Result<bool>;
    fn zone(&mut self, p: &NodeHandle) -> Result<Self::Zone>;
    fn click_zone(&mut self, z: Self::Zone, a: Action) -> Result<bool>;
    fn manager(&mut self, p: &NodeHandle) -> Result<Self::Manager>;
    fn component(&mut self, m: Self::Manager, id: i32) -> Result<Option<Handle>>;
    fn hidden(&mut self, c: &Handle) -> Result<bool>;
    fn plugin(&mut self, c: &Handle) -> Result<Option<Plugin>>;
    fn id(&mut self, c: &Handle) -> Result<i32>;
    fn button_event(&mut self, p: &NodeHandle, id: i32, child: i32) -> Result<()>;
    fn run_listener(&mut self, p: &NodeHandle, c: &Handle, b: Button) -> Result<bool>;
    fn handle_plugin(
        &mut self,
        plugin: Plugin,
        p: &NodeHandle,
        c: &Handle,
        b: Button,
    ) -> Result<bool>;
    fn close(&mut self, m: Self::Manager) -> Result<bool>;
    fn logging_present(&mut self, p: &NodeHandle) -> Result<bool>;
    fn submit_login(&mut self, pulse: LoginPulse) -> Result<()>;
    fn map_present(&mut self, p: &NodeHandle) -> Result<bool>;
    fn remove_map(&mut self, p: &NodeHandle) -> Result<()>;
    fn dispatch(&mut self, p: &NodeHandle) -> Result<Self::Dispatch>;
    fn resizable(&mut self, m: Self::Manager) -> Result<bool>;
    fn windows_pane(&mut self, d: Self::Dispatch, id: i32, kind: i32) -> Result<()>;
    fn unlock(&mut self, p: &NodeHandle) -> Result<()>;
    fn remove_logging(&mut self, p: &NodeHandle) -> Result<()>;
    fn configure_world(&mut self, p: &NodeHandle) -> Result<()>;
}
pub fn if_action(ctx: &mut impl Context, p: &NodeHandle, a: Action) -> Result<()> {
    for text in [
        "[IF ACTION]--------------------------".to_owned(),
        format!("Iface: {}, Button: {}", a.iface, a.child),
        format!("Slot: {}, ItemID: {}", a.slot, a.item),
        format!("RCM Index: {}, Op: {}", a.opt_index, a.opcode),
        "-------------------------------------".to_owned(),
    ] {
        ctx.debug(p, text)?;
    }
    let d = ctx.dialogue(p)?;
    if ctx.has_dialogue(d)? && a.opcode != 132 && a.iface != 64 && a.iface != 746 {
        let d = ctx.dialogue(p)?;
        ctx.close_dialogue(d)?;
    }
    let l = ctx.locks(p)?;
    if ctx.component_locked(l)? {
        return Ok(());
    }
    let z = ctx.zone(p)?;
    if ctx.click_zone(z, a)? {
        return Ok(());
    }
    let m = ctx.manager(p)?;
    let Some(c) = ctx.component(m, a.iface)? else {
        return Ok(());
    };
    if ctx.hidden(&c)? {
        return Ok(());
    }
    let plugin = ctx.plugin(&c)?;
    let id = ctx.id(&c)?;
    ctx.button_event(p, id, a.child)?;
    if !ctx.run_listener(p, &c, a.button())?
        && let Some(plugin) = plugin
    {
        ctx.handle_plugin(plugin, p, &c, a.button())?;
    }
    Ok(())
}
pub fn close_iface(ctx: &mut impl Context, p: &NodeHandle) -> Result<()> {
    let m = ctx.manager(p)?;
    ctx.close(m)?;
    if ctx.logging_present(p)? {
        ctx.submit_login(LoginPulse { player: p.clone() })?;
    }
    if ctx.map_present(p)? {
        ctx.remove_map(p)?;
        let dispatch = ctx.dispatch(p)?;
        let m = ctx.manager(p)?;
        let id = if ctx.resizable(m)? { 746 } else { 548 };
        ctx.windows_pane(dispatch, id, 2)?;
        ctx.unlock(p)?;
    }
    Ok(())
}
pub trait ContinueContext: Context {
    type DialogueAction;
    fn close_chatbox(&mut self, m: Self::Manager) -> Result<()>;
    fn pop_action(&mut self, d: Self::Dialogue) -> Result<Option<Self::DialogueAction>>;
    fn handle_action(&mut self, a: Self::DialogueAction, p: &NodeHandle, child: i32) -> Result<()>;
    fn continue_dialogue(&mut self, d: Self::Dialogue, iface: i32, child: i32) -> Result<()>;
}
pub fn continue_option(ctx: &mut impl ContinueContext, p: &NodeHandle, a: Action) -> Result<()> {
    for text in [
        "[CONTINUE OPT]----------".into(),
        format!("Iface: {}", a.iface),
        format!("Child: {}", a.child),
        format!("Slot: {}", a.slot),
        "------------------------".into(),
    ] {
        ctx.debug(p, text)?;
    }
    let d = ctx.dialogue(p)?;
    if !ctx.has_dialogue(d)? {
        let m = ctx.manager(p)?;
        ctx.close_chatbox(m)?;
        let d = ctx.dialogue(p)?;
        if let Some(action) = ctx.pop_action(d)? {
            ctx.handle_action(action, p, a.child)?;
        }
        let m = ctx.manager(p)?;
        let Some(c) = ctx.component(m, a.iface)? else {
            return Ok(());
        };
        let b = Button {
            opcode: a.opcode,
            button: a.child,
            slot: a.slot,
            item: -1,
        };
        if !ctx.run_listener(p, &c, b)?
            && let Some(plugin) = ctx.plugin(&c)?
        {
            ctx.handle_plugin(plugin, p, &c, b)?;
        }
        return Ok(());
    }
    let d = ctx.dialogue(p)?;
    ctx.continue_dialogue(d, a.iface, a.child)
}
