//! Native component metadata, InterfaceManager lifecycle and gameframe methods.
//! Listeners, virtual component plugins, player settings and login content are
//! explicit owning-world services; this does not complete player initialization.
pub use super::component_objects::{Close as CloseEvent, Plugin};
use super::player_walk::NodeHandle;
use crate::definitions::item::Shared;
use rs09_game_core::{
    java_json::{self, Json},
    java_string::parse_i32,
};
use rs09_protocol::game::{Frame, updates};
use std::collections::HashMap;
#[derive(Debug)]
pub enum Error {
    Null,
    Index,
    Number,
    Type,
    InvalidAttack,
    Limit,
    Json(java_json::Error),
    Binding(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    Default,
    Overlay,
    Tab,
    SingleTab,
    Dialogue,
    WindowPane,
    CsChatbox,
    Chatbox,
    OverlayB,
}
impl Type {
    pub const ALL: [Self; 9] = [
        Self::Default,
        Self::Overlay,
        Self::Tab,
        Self::SingleTab,
        Self::Dialogue,
        Self::WindowPane,
        Self::CsChatbox,
        Self::Chatbox,
        Self::OverlayB,
    ];
    pub fn pane(self, resizable: bool) -> i32 {
        match self {
            Self::Dialogue | Self::CsChatbox | Self::Chatbox => 752,
            _ => {
                if resizable {
                    746
                } else {
                    548
                }
            }
        }
    }
    pub fn child(self, resizable: bool) -> i32 {
        let (a, b) = match self {
            Self::Default => (11, 6),
            Self::Overlay => (4, 5),
            Self::Tab => (83, 93),
            Self::SingleTab => (80, 76),
            Self::Dialogue => (12, 12),
            Self::WindowPane => (0, 0),
            Self::CsChatbox => (6, 6),
            Self::Chatbox => (8, 8),
            Self::OverlayB => (11, 3),
        };
        if resizable { b } else { a }
    }
}
pub struct Definition {
    pub kind: Option<Type>,
    pub walkable: bool,
    pub tab_index: i32,
    pub plugin: Option<Plugin>,
}
impl Default for Definition {
    fn default() -> Self {
        Self {
            kind: Some(Type::Default),
            walkable: false,
            tab_index: -1,
            plugin: None,
        }
    }
}
pub type Def = Shared<Definition>;
pub type Handle = Shared<Component>;
pub struct Component {
    pub id: i32,
    pub definition: Option<Def>,
    pub plugin: Option<Plugin>,
    pub close_event: Option<CloseEvent>,
    pub hidden: bool,
    pub weapon: bool,
}
#[derive(Default)]
pub struct Registry {
    pub definitions: HashMap<i32, Def>,
}
impl Registry {
    pub fn for_id(&mut self, id: i32) -> Def {
        self.definitions
            .entry(id)
            .or_insert_with(|| Shared::new(Definition::default()))
            .clone()
    }
    pub fn component(&mut self, id: i32) -> Handle {
        let d = self.for_id(id);
        let plugin = d.read().plugin.clone();
        Shared::new(Component {
            id,
            definition: Some(d),
            plugin,
            close_event: None,
            hidden: false,
            weapon: false,
        })
    }
    pub fn parse_definition(d: &Def, kind: &str, walkable: &str, index: &str) -> Result<()> {
        let n = parse_i32(kind).ok_or(Error::Number)?;
        let t = *Type::ALL.get(n as usize).ok_or(Error::Index)?;
        d.write().kind = Some(t);
        d.write().walkable = walkable.eq_ignore_ascii_case("true");
        let n = parse_i32(index).ok_or(Error::Number)?;
        d.write().tab_index = n;
        Ok(())
    }
    /// InterfaceConfigParser also replaces a successfully parsed existing record.
    /// Old component aliases retain the first parse and its plugin identity.
    pub fn load(&mut self, text: &str) -> Result<i32> {
        let data = java_json::parse(text).map_err(Error::Json)?;
        let Json::Array(rows) = data else {
            return Err(if matches!(data, Json::Null) {
                Error::Null
            } else {
                Error::Type
            });
        };
        let mut count = 0i32;
        for row in rows {
            if !matches!(row, Json::Object(_)) {
                return Err(if matches!(row, Json::Null) {
                    Error::Null
                } else {
                    Error::Type
                });
            }
            let value = |k: &str| row.get(k).unwrap_or(&Json::Null).text();
            let id = parse_i32(&value("id")).ok_or(Error::Number)?;
            if let Some(d) = self.definitions.get(&id) {
                Self::parse_definition(
                    d,
                    &value("interfaceType"),
                    &value("walkable"),
                    &value("tabIndex"),
                )?;
            }
            let d = Shared::new(Definition::default());
            Self::parse_definition(
                &d,
                &value("interfaceType"),
                &value("walkable"),
                &value("tabIndex"),
            )?;
            self.definitions.insert(id, d);
            count = count.wrapping_add(1);
        }
        Ok(count)
    }
    pub fn plugin(&mut self, c: &Handle) -> Option<Plugin> {
        let current = c.read().plugin.clone();
        if current.is_some() {
            return current;
        }
        let id = c.read().id;
        let plugin = self.for_id(id).read().plugin.clone();
        c.write().plugin = plugin.clone();
        plugin
    }
}
pub struct Fields {
    player: Option<NodeHandle>,
    pub windows_pane: Option<Handle>,
    pub opened: Option<Handle>,
    pub chatbox: Option<Handle>,
    pub tabs: Option<Shared<Vec<Option<Handle>>>>,
    pub single_tab: Option<Handle>,
    pub overlay: Option<Handle>,
    pub wildy_overlay: Option<Handle>,
    pub current_tab_index: i32,
    pub packet_count: i32,
}
#[derive(Clone)]
pub struct Manager {
    pub fields: Shared<Fields>,
}
impl Default for Manager {
    fn default() -> Self {
        Self {
            fields: Shared::new(Fields {
                player: None,
                windows_pane: None,
                opened: None,
                chatbox: None,
                tabs: Some(Shared::new(vec![None; 15])),
                single_tab: None,
                overlay: None,
                wildy_overlay: None,
                current_tab_index: 3,
                packet_count: 0,
            }),
        }
    }
}
impl Manager {
    pub fn new(player: Option<NodeHandle>) -> Self {
        let manager = Self::default();
        manager.fields.write().player = player;
        manager
    }
    pub fn player(&self) -> Option<NodeHandle> {
        self.fields.read().player.clone()
    }
    /// Canonical startup fixes the constructor receiver before publishing fields.
    pub(crate) fn bind_player(&self, player: NodeHandle) -> Result<()> {
        let mut fields = self.fields.write();
        if fields.player.as_ref().is_some_and(|old| old != &player) {
            return Err(Error::Binding(
                "interface manager already belongs to another player".into(),
            ));
        }
        fields.player = Some(player);
        Ok(())
    }
    pub fn packet_count(&self, increment: i32) -> i32 {
        let mut f = self.fields.write();
        let n = f.packet_count;
        f.packet_count = n.wrapping_add(increment);
        n
    }
    pub fn pane_id(&self) -> i32 {
        self.fields
            .read()
            .windows_pane
            .as_ref()
            .map_or(548, |p| p.read().id)
    }
}
pub const DEFAULT_TABS: [i32; 15] = [
    92, 320, 274, 149, 387, 271, 192, 662, 550, 551, 589, 261, 464, 187, 182,
];
#[derive(Clone, Copy)]
pub enum Open {
    Overlap(bool),
    Typed(i32),
}
pub trait Context {
    fn validate_manager(&mut self, manager: &Manager) -> Result<()> {
        if manager.player().is_some() {
            return Err(Error::Binding(
                "canonical interface owner service is unavailable".into(),
            ));
        }
        Ok(())
    }
    fn component(&mut self, id: i32) -> Result<Handle>;
    fn resizable(&mut self) -> Result<bool>;
    fn send(&mut self, frame: Frame) -> Result<()>;
    fn log(&mut self, message: String) -> Result<()>;
    fn listener_open(&mut self, component: &Handle) -> Result<()>;
    fn listener_close(&mut self, component: &Handle) -> Result<bool>;
    fn plugin_open(&mut self, plugin: Plugin, component: &Handle) -> Result<()>;
    fn close_event(&mut self, event: CloseEvent, component: &Handle) -> Result<bool>;
    fn open_component(&mut self, manager: &Manager, component: &Handle) -> Result<()>;
    fn dispatch_open(&mut self, component: Handle) -> Result<()>;
    fn set_varp(&mut self, id: i32, value: i32) -> Result<()>;
    fn weapon_extension(&mut self) -> Result<Option<Handle>>;
    fn create_weapon(&mut self) -> Result<Handle>;
    fn add_weapon(&mut self, component: Handle) -> Result<()>;
    fn spellbook(&mut self) -> Result<i32>;
    fn has_familiar(&mut self) -> Result<bool>;
    fn friends_component(&mut self) -> Result<Handle>;
    fn autocast(&mut self) -> Result<Option<usize>>;
    fn auto_spell_id(&mut self, weapon: &Handle, spell: usize) -> Result<i32>;
    fn select_auto_spell(&mut self, weapon: &Handle, id: i32) -> Result<()>;
    fn window_mode(&mut self) -> Result<i32>;
    fn set_window_mode(&mut self, mode: i32) -> Result<()>;
    fn tutorial_complete(&mut self) -> Result<bool>;
    fn tutorial_tabs(&mut self) -> Result<()>;
    fn has_runscript(&mut self) -> Result<bool> {
        Err(Error::Binding(
            "runscript attribute service is unavailable".into(),
        ))
    }
    fn remove_runscript(&mut self) -> Result<()> {
        Err(Error::Binding(
            "runscript attribute service is unavailable".into(),
        ))
    }
    fn dispatch_close(&mut self, _: Option<Handle>) -> Result<()> {
        Err(Error::Binding(
            "interface close event service is unavailable".into(),
        ))
    }
    fn run_script(&mut self, m: &Manager, id: i32, value: Option<i32>) -> Result<()> {
        let args = value
            .map(updates::ScriptArg::Int)
            .into_iter()
            .collect::<Vec<_>>();
        let frame = updates::run_client_script(
            id,
            if value.is_some() { b"i" } else { b"" },
            &args,
            m.packet_count(1),
        )
        .map_err(|e| Error::Binding(e.to_string()))?;
        self.send(frame)
    }
    fn interface_config(&mut self, m: &Manager, pane: i32, child: i32, hidden: bool) -> Result<()> {
        self.send(updates::interface_hidden(
            hidden,
            m.packet_count(1),
            pane.wrapping_shl(16) | child,
        ))
    }
    fn set_unclosable_attribute(&mut self, _: bool) -> Result<()> {
        Err(Error::Binding(
            "component attribute service is unavailable".into(),
        ))
    }
    fn unclosable_attribute(&mut self) -> Result<bool> {
        Err(Error::Binding(
            "component attribute service is unavailable".into(),
        ))
    }
}
pub fn set_unclosable(ctx: &mut impl Context, c: Option<&Handle>) -> Result<()> {
    ctx.set_unclosable_attribute(true)?;
    c.ok_or(Error::Null)?.write().close_event =
        Some(CloseEvent::new(super::component_objects::Unclosable));
    Ok(())
}
fn def(c: &Handle) -> Result<Def> {
    c.read().definition.clone().ok_or(Error::Null)
}
fn kind(c: &Handle) -> Result<Option<Type>> {
    Ok(def(c)?.read().kind)
}
fn log(ctx: &mut impl Context, text: String) -> Result<()> {
    ctx.log(text)
}
pub fn send_interface(
    ctx: &mut impl Context,
    m: &Manager,
    pane: i32,
    child: i32,
    id: i32,
    walkable: bool,
) -> Result<()> {
    ctx.validate_manager(m)?;
    let pointer = pane.wrapping_shl(16) | child;
    let count = m.packet_count(1);
    ctx.send(updates::open_interface(
        i32::from(walkable),
        pointer,
        count,
        id,
    ))
}
pub fn component_open(ctx: &mut impl Context, m: &Manager, c: &Handle) -> Result<()> {
    ctx.validate_manager(m)?;
    ctx.listener_open(c)?;
    if c.read().definition.is_none() {
        let pane = m.pane_id();
        let child = if ctx.resizable()? { 6 } else { 11 };
        let id = c.read().id;
        send_interface(ctx, m, pane, child, id, false)?;
        let plugin = c.read().plugin.clone();
        if let Some(p) = plugin {
            ctx.plugin_open(p, c)?;
        }
        return Ok(());
    }
    if kind(c)? == Some(Type::WindowPane) {
        return Ok(());
    }
    let tab = kind(c)? == Some(Type::Tab);
    let d = def(c)?;
    let r = ctx.resizable()?;
    let pane = d.read().kind.ok_or(Error::Null)?.pane(r);
    let d = def(c)?;
    let r = ctx.resizable()?;
    let mut child = d.read().kind.ok_or(Error::Null)?.child(r);
    if tab {
        child = child.wrapping_add(def(c)?.read().tab_index);
    }
    let id = c.read().id;
    let walkable = def(c)?.read().walkable;
    send_interface(ctx, m, pane, child, id, walkable)?;
    let plugin = c.read().plugin.clone();
    if let Some(p) = plugin {
        ctx.plugin_open(p, c)?;
    }
    Ok(())
}
pub fn component_close(ctx: &mut impl Context, c: &Handle) -> Result<bool> {
    let event = c.read().close_event.clone();
    if let Some(e) = event {
        let allowed = if e
            .value::<super::component_objects::SingleTabClose>()
            .is_some()
        {
            true
        } else {
            ctx.close_event(e, c)?
        };
        if !allowed {
            return Ok(false);
        }
    }
    ctx.listener_close(c)
}
fn tabs(m: &Manager) -> Result<Shared<Vec<Option<Handle>>>> {
    m.fields.read().tabs.clone().ok_or(Error::Null)
}
fn tab(m: &Manager, index: i32) -> Result<Option<Handle>> {
    tabs(m)?
        .read()
        .get(index as usize)
        .cloned()
        .ok_or(Error::Index)
}
fn store_tab(m: &Manager, index: i32, c: Option<Handle>) -> Result<()> {
    *tabs(m)?
        .write()
        .get_mut(index as usize)
        .ok_or(Error::Index)? = c;
    Ok(())
}
fn close_packet(ctx: &mut impl Context, m: &Manager, c: &Handle, tab: bool) -> Result<()> {
    let d = def(c)?;
    let r = ctx.resizable()?;
    let pane = d.read().kind.ok_or(Error::Null)?.pane(r);
    let d = def(c)?;
    let r = ctx.resizable()?;
    let mut child = d.read().kind.ok_or(Error::Null)?.child(r);
    if tab {
        child = child.wrapping_add(def(c)?.read().tab_index);
    }
    // The original context evaluates these even though CloseInterface ignores them.
    let _id = c.read().id;
    let _walkable = def(c)?.read().walkable;
    let pointer = pane.wrapping_shl(16) | (child & 65535);
    ctx.send(updates::close_interface(m.packet_count(1), pointer))
}
pub fn open(ctx: &mut impl Context, m: &Manager, c: Option<Handle>) -> Result<Option<Handle>> {
    ctx.validate_manager(m)?;
    if !close(ctx, m)? {
        return Ok(None);
    }
    let c = c.ok_or(Error::Null)?;
    ctx.open_component(m, &c)?;
    m.fields.write().opened = Some(c);
    let opened = m.fields.read().opened.clone().ok_or(Error::Null)?;
    ctx.dispatch_open(opened)?;
    Ok(m.fields.read().opened.clone())
}
pub fn open_id(ctx: &mut impl Context, m: &Manager, id: i32) -> Result<Option<Handle>> {
    ctx.validate_manager(m)?;
    let c = ctx.component(id)?;
    open(ctx, m, Some(c))
}
pub fn is_opened(m: &Manager) -> bool {
    m.fields.read().opened.is_some()
}
pub fn has_chatbox(m: &Manager) -> bool {
    m.fields
        .read()
        .chatbox
        .as_ref()
        .is_some_and(|c| c.read().id != 137)
}
pub fn has_main_component(m: &Manager, id: i32) -> bool {
    m.fields
        .read()
        .opened
        .as_ref()
        .is_some_and(|c| c.read().id == id)
}
pub fn is_walkable(m: &Manager) -> Result<bool> {
    if let Some(c) = m.fields.read().opened.clone() {
        if c.read().id == 389 {
            return Ok(false);
        }
        // Preserve the original otherwise-true result, including non-walkable definitions.
        let _walkable = def(&c)?.read().walkable;
    }
    Ok(true)
}
pub fn close(ctx: &mut impl Context, m: &Manager) -> Result<bool> {
    ctx.validate_manager(m)?;
    if ctx.has_runscript()? {
        ctx.remove_runscript()?;
        ctx.run_script(m, 101, None)?;
    }
    let overlay = m.fields.read().overlay.clone();
    if overlay.is_some_and(|c| c.read().id == 333) {
        close_overlay(ctx, m)?;
    }
    let opened = m.fields.read().opened.clone();
    if let Some(c) = opened {
        if component_close(ctx, &c)? {
            // Callback mutations change which current component receives the packet/event.
            let opened = m.fields.read().opened.clone();
            if let Some(c) = opened {
                if !def(&c)?.read().walkable || c.read().id == 14 {
                    close_packet(ctx, m, &c, false)?;
                    let opened = m.fields.read().opened.clone();
                    ctx.dispatch_close(opened)?;
                }
            }
            m.fields.write().opened = None;
        }
    }
    Ok(m.fields.read().opened.is_none())
}
pub fn close_component(ctx: &mut impl Context, m: &Manager, c: &Handle) -> Result<bool> {
    ctx.validate_manager(m)?;
    if !component_close(ctx, c)? {
        return Ok(false);
    }
    if c.read().id != 137 {
        close_packet(ctx, m, c, kind(c)? == Some(Type::Tab))?;
    }
    Ok(true)
}
pub fn close_chatbox(ctx: &mut impl Context, m: &Manager) -> Result<()> {
    ctx.validate_manager(m)?;
    let chat = m.fields.read().chatbox.clone();
    if let Some(c) = chat {
        if c.read().id != 137 && close_component(ctx, m, &c)? {
            let c = ctx.component(137)?;
            open_chatbox(ctx, m, &c)?;
            ctx.run_script(m, 101, None)?;
        }
    }
    Ok(())
}
pub fn open_single_tab(ctx: &mut impl Context, m: &Manager, c: &Handle) -> Result<Handle> {
    ctx.validate_manager(m)?;
    if kind(c)? != Some(Type::SingleTab) {
        let id = c.read().id;
        log(
            ctx,
            format!(
                "Set interface type to SINGLE_TAB for component {id}, definition requires updating!"
            ),
        )?;
        def(c)?.write().kind = Some(Type::SingleTab);
    }
    ctx.open_component(m, c)?;
    if c.read().close_event.is_none() {
        c.write().close_event = Some(super::component_objects::single_tab_close());
    }
    m.fields.write().single_tab = Some(c.clone());
    Ok(c.clone())
}
pub fn close_single_tab(ctx: &mut impl Context, m: &Manager) -> Result<bool> {
    ctx.validate_manager(m)?;
    let single = m.fields.read().single_tab.clone();
    if let Some(c) = single {
        if close_component(ctx, m, &c)? {
            m.fields.write().single_tab = None;
        }
    }
    Ok(true)
}
pub fn set_viewed_tab(ctx: &mut impl Context, m: &Manager, mut index: i32) -> Result<()> {
    ctx.validate_manager(m)?;
    if tab(m, index)?.is_none() {
        return Ok(());
    }
    m.fields.write().current_tab_index = index;
    if (0..=2).contains(&index) {
        index += 1;
    }
    if index > 9 {
        index -= 1;
    }
    ctx.run_script(m, 115, Some(index))
}
pub fn remove_tabs(ctx: &mut impl Context, m: &Manager, indices: &[i32]) -> Result<()> {
    ctx.validate_manager(m)?;
    let mut change = false;
    for &slot in indices {
        if slot == m.fields.read().current_tab_index {
            change = true;
        }
        if let Some(c) = tab(m, slot)? {
            close_component(ctx, m, &c)?;
            store_tab(m, slot, None)?;
        }
    }
    if change {
        let mut index = -1;
        if tab(m, 3)?.is_none() {
            let mut i = 0;
            while i < tabs(m)?.read().len() {
                if i >= 65536 {
                    return Err(Error::Limit);
                }
                if tab(m, i as i32)?.is_some() {
                    index = i as i32;
                    break;
                }
                i += 1;
            }
        } else {
            index = 3;
        }
        if index > -1 {
            set_viewed_tab(ctx, m, index)?;
        }
    }
    Ok(())
}
pub fn restore_tabs(ctx: &mut impl Context, m: &Manager) -> Result<()> {
    ctx.validate_manager(m)?;
    let mut i = 0;
    while i < tabs(m)?.read().len() {
        if i >= 65536 {
            return Err(Error::Limit);
        }
        match tab(m, i as i32)? {
            None => match i {
                0 => {
                    let mut weapon = ctx.weapon_extension()?;
                    if weapon.is_none() {
                        let c = ctx.create_weapon()?;
                        weapon = Some(c.clone());
                        ctx.add_weapon(c)?;
                    }
                    open_tab(ctx, m, i as i32, &weapon.ok_or(Error::Null)?)?;
                }
                6 => {
                    let id = ctx.spellbook()?;
                    let c = ctx.component(id)?;
                    open_tab(ctx, m, i as i32, &c)?;
                }
                7 => {
                    if ctx.has_familiar()? {
                        let c = ctx.component(662)?;
                        open_tab(ctx, m, i as i32, &c)?;
                    }
                }
                8 => {
                    let c = ctx.friends_component()?;
                    open_tab(ctx, m, i as i32, &c)?;
                }
                _ => {
                    let id = *DEFAULT_TABS.get(i).ok_or(Error::Index)?;
                    let c = ctx.component(id)?;
                    open_tab(ctx, m, i as i32, &c)?;
                }
            },
            Some(c) => {
                if c.read().hidden {
                    let child = if i < 7 { 38 } else { 13 } + i as i32;
                    let pane = m.pane_id();
                    ctx.interface_config(m, pane, child, false)?;
                    let pane = m.pane_id();
                    ctx.interface_config(m, pane, child + 7, false)?;
                    tab(m, i as i32)?.ok_or(Error::Null)?.write().hidden = false;
                }
            }
        }
        i += 1;
    }
    Ok(())
}
pub fn close_default_tabs(ctx: &mut impl Context, m: &Manager) -> Result<()> {
    ctx.validate_manager(m)?;
    let mut i = 0;
    while i < tabs(m)?.read().len() {
        if i >= 65536 {
            return Err(Error::Limit);
        }
        if let Some(c) = tab(m, i as i32)? {
            if c.read().id != 182 {
                close_component(ctx, m, &c)?;
                store_tab(m, i as i32, None)?;
            }
        }
        i += 1;
    }
    Ok(())
}
pub fn get_component(ctx: &mut impl Context, m: &Manager, id: i32) -> Result<Option<Handle>> {
    ctx.validate_manager(m)?;
    let candidates = {
        let f = m.fields.read();
        [
            f.opened.clone(),
            f.chatbox.clone(),
            f.single_tab.clone(),
            f.overlay.clone(),
        ]
    };
    for c in candidates.into_iter().flatten() {
        if c.read().id == id {
            return Ok(Some(c));
        }
    }
    let pane = m.fields.read().windows_pane.clone().ok_or(Error::Null)?;
    if pane.read().id == id {
        return Ok(Some(pane));
    }
    // Enhanced-for captures the array, unlike the index-based mutating loops.
    let array = tabs(m)?;
    let len = array.read().len();
    for i in 0..len {
        let c = array.read()[i].clone();
        if let Some(c) = c {
            if c.read().id == id {
                return Ok(Some(c));
            }
        }
    }
    if [751, 750, 747].contains(&id) {
        return Ok(Some(ctx.component(id)?));
    }
    Ok(None)
}
pub fn open_overlay(ctx: &mut impl Context, m: &Manager, c: Option<Handle>) -> Result<()> {
    ctx.validate_manager(m)?;
    let old = m.fields.read().overlay.clone();
    if let Some(old) = old {
        if !component_close(ctx, &old)? {
            return Ok(());
        }
    }
    m.fields.write().overlay = c.clone();
    let overlay = c.ok_or(Error::Null)?;
    if kind(&overlay)? != Some(Type::Overlay) && kind(&overlay)? != Some(Type::OverlayB) {
        let id = overlay.read().id;
        log(
            ctx,
            format!(
                "Set interface type to OVERLAY for component {id}, definition requires updating!"
            ),
        )?;
        let overlay = m.fields.read().overlay.clone().ok_or(Error::Null)?;
        def(&overlay)?.write().kind = Some(Type::Overlay);
        let overlay = m.fields.read().overlay.clone().ok_or(Error::Null)?;
        def(&overlay)?.write().walkable = true;
    }
    let overlay = m.fields.read().overlay.clone().ok_or(Error::Null)?;
    ctx.open_component(m, &overlay)
}
pub fn close_overlay(ctx: &mut impl Context, m: &Manager) -> Result<()> {
    ctx.validate_manager(m)?;
    let overlay = m.fields.read().overlay.clone();
    if let Some(c) = overlay {
        if close_component(ctx, m, &c)? {
            m.fields.write().overlay = None;
        }
    }
    Ok(())
}
pub fn open_windows(
    ctx: &mut impl Context,
    m: &Manager,
    c: Option<Handle>,
    open: Open,
) -> Result<()> {
    ctx.validate_manager(m)?;
    m.fields.write().windows_pane = c.clone();
    let c = c.ok_or(Error::Null)?;
    if kind(&c)? != Some(Type::WindowPane) {
        let id = c.read().id;
        log(
            ctx,
            format!(
                "Set interface type to WINDOW_PANE for component {id}, definition requires updating!"
            ),
        )?;
        def(&c)?.write().kind = Some(match open {
            Open::Typed(_) => Type::SingleTab,
            Open::Overlap(_) => Type::WindowPane,
        });
    }
    let id = c.read().id;
    let reset = match open {
        Open::Overlap(v) => i32::from(v),
        Open::Typed(v) => v,
    };
    let count = m.packet_count(1);
    ctx.send(updates::windows_pane(id, reset, count))?;
    ctx.open_component(m, &c)?;
    let opened = m.fields.read().opened.clone();
    if let Some(c) = opened {
        ctx.dispatch_open(c)?;
    }
    Ok(())
}
pub fn open_tab(ctx: &mut impl Context, m: &Manager, slot: i32, c: &Handle) -> Result<()> {
    ctx.validate_manager(m)?;
    if c.read().id == 92 && !c.read().weapon {
        return Err(Error::InvalidAttack);
    }
    if def(c)?.read().tab_index != slot {
        let id = c.read().id;
        log(
            ctx,
            format!("Set tab index to {slot} for component {id}, definition requires updating!"),
        )?;
        def(c)?.write().tab_index = slot;
    }
    if kind(c)? != Some(Type::Tab) {
        let id = c.read().id;
        log(
            ctx,
            format!("Set interface type to TAB for component {id}, definition requires updating!"),
        )?;
        def(c)?.write().kind = Some(Type::Tab);
    }
    ctx.open_component(m, c)?;
    let tabs = tabs(m)?;
    let mut t = tabs.write();
    *t.get_mut(slot as usize).ok_or(Error::Index)? = Some(c.clone());
    Ok(())
}
pub fn open_tab_auto(ctx: &mut impl Context, m: &Manager, c: &Handle) -> Result<()> {
    ctx.validate_manager(m)?;
    if def(c)?.read().tab_index < 0 {
        let id = c.read().id;
        log(ctx, format!("No component definitions found for tab {id}!"))?;
        return Ok(());
    }
    let slot = def(c)?.read().tab_index;
    open_tab(ctx, m, slot, c)
}
pub fn open_default_tabs(ctx: &mut impl Context, m: &Manager) -> Result<()> {
    ctx.validate_manager(m)?;
    let mut inter = ctx.weapon_extension()?;
    if inter.is_none() {
        let c = ctx.create_weapon()?;
        inter = Some(c.clone());
        ctx.add_weapon(c)?;
    }
    let inter = inter.ok_or(Error::Null)?;
    let mut i = 0usize;
    while i < tabs(m)?.read().len() {
        if i >= 65_536 {
            return Err(Error::Limit);
        }
        match i {
            0 => open_tab(ctx, m, 0, &inter)?,
            6 => {
                let id = ctx.spellbook()?;
                let c = ctx.component(id)?;
                open_tab(ctx, m, 6, &c)?;
            }
            7 => {
                if ctx.has_familiar()? {
                    let c = ctx.component(662)?;
                    open_tab(ctx, m, 7, &c)?;
                }
            }
            8 => {
                let t = tabs(m)?;
                let empty = t.read().get(i).ok_or(Error::Index)?.is_none();
                if empty {
                    let c = ctx.friends_component()?;
                    open_tab(ctx, m, 8, &c)?;
                }
            }
            _ => {
                let id = *DEFAULT_TABS.get(i).ok_or(Error::Index)?;
                let c = ctx.component(id)?;
                open_tab(ctx, m, i as i32, &c)?;
            }
        }
        i += 1;
    }
    if ctx.autocast()?.is_some() {
        let spell = ctx.autocast()?.ok_or(Error::Null)?;
        let id = ctx.auto_spell_id(&inter, spell)?;
        ctx.select_auto_spell(&inter, id)?;
    }
    Ok(())
}
pub fn open_info_bars(ctx: &mut impl Context, m: &Manager) -> Result<()> {
    ctx.validate_manager(m)?;
    for (fixed, resize, id) in [
        (70, 13, 748),
        (71, 14, 749),
        (72, 15, 750),
        (73, 16, 747),
        (10, 71, 754),
    ] {
        let pane = m.pane_id();
        let child = if ctx.resizable()? { resize } else { fixed };
        send_interface(ctx, m, pane, child, id, true)?;
    }
    Ok(())
}
pub fn open_chatbox(ctx: &mut impl Context, m: &Manager, c: &Handle) -> Result<()> {
    ctx.validate_manager(m)?;
    if c.read().id == 137 {
        let chat = m.fields.read().chatbox.clone();
        let open = match chat {
            None => true,
            Some(chat) => chat.read().id != 137 && kind(&chat)? == Some(Type::Chatbox),
        };
        if open {
            let pane = m.pane_id();
            let child = if ctx.resizable()? { 23 } else { 14 };
            send_interface(ctx, m, pane, child, 751, true)?;
            let pane = m.pane_id();
            let child = if ctx.resizable()? { 70 } else { 75 };
            send_interface(ctx, m, pane, child, 752, true)?;
            send_interface(ctx, m, 752, 8, 137, true)?;
        }
        m.fields.write().chatbox = Some(c.clone());
        ctx.set_varp(334, 1)?;
    } else {
        m.fields.write().chatbox = Some(c.clone());
        if kind(c)? != Some(Type::Dialogue)
            && kind(c)? != Some(Type::Chatbox)
            && kind(c)? != Some(Type::CsChatbox)
        {
            let id = c.read().id;
            log(
                ctx,
                format!(
                    "Set interface type to CHATBOX for component {id}, definition requires updating!"
                ),
            )?;
            def(c)?.write().kind = Some(Type::Dialogue);
        }
        ctx.open_component(m, c)?;
    }
    Ok(())
}
pub fn switch_window_mode(ctx: &mut impl Context, m: &Manager, mode: i32) -> Result<()> {
    ctx.validate_manager(m)?;
    if mode != ctx.window_mode()? {
        ctx.set_window_mode(mode)?;
        let id = if ctx.resizable()? { 746 } else { 548 };
        let c = ctx.component(id)?;
        open_windows(ctx, m, Some(c), Open::Overlap(false))?;
        if !ctx.tutorial_complete()? {
            ctx.tutorial_tabs()?;
        } else {
            open_default_tabs(ctx, m)?;
        }
        open_info_bars(ctx, m)?;
        let pane = m.pane_id();
        let child = if ctx.resizable()? { 23 } else { 14 };
        send_interface(ctx, m, pane, child, 751, true)?;
        let pane = m.pane_id();
        let child = if ctx.resizable()? { 70 } else { 75 };
        send_interface(ctx, m, pane, child, 752, true)?;
    }
    Ok(())
}
/// Prefix and ordering of LoginConfiguration.sendGameConfiguration; welcome,
/// config, login plugins and appearance synchronization are owning-world calls.
pub trait LoginContext: Context {
    fn welcome(&mut self) -> Result<()>;
    fn configure(&mut self, m: &Manager) -> Result<()>;
    fn login_plugins(&mut self) -> Result<()>;
    fn appearance_sync(&mut self) -> Result<()>;
}
pub fn send_game_configuration(ctx: &mut impl LoginContext, m: &Manager) -> Result<()> {
    ctx.validate_manager(m)?;
    let id = if ctx.resizable()? { 746 } else { 548 };
    let c = ctx.component(id)?;
    open_windows(ctx, m, Some(c), Open::Overlap(false))?;
    let c = ctx.component(137)?;
    open_chatbox(ctx, m, &c)?;
    open_default_tabs(ctx, m)?;
    ctx.welcome()?;
    ctx.configure(m)?;
    ctx.login_plugins()?;
    ctx.appearance_sync()
}
