//! The global handlers' interface and dialogue services in the live world
//! (`world::global_handlers`, docs/live-world.md).
//!
//! Interfaces: ContentAPI `openInterface`, `openSingleTab`,
//! `closeTabInterface` and the InterfaceManager calls the handlers make run
//! the native InterfaceManager (`world::interfaces`) over
//! `trade::ui::Ui<TradeHost>`, the interface context with the whole world
//! (the one the Close packet and a walk's close already use). That context
//! runs the global handlers' open and close listeners
//! (`InterfaceListeners.runOpen`/`runClose`: the equipment screen 667 and
//! its inventory side 670, items kept on death 102, the item select and
//! rules listeners) through `registry::run_open` where the interface
//! manager calls them, before the interface packet, as the original does.
//!
//! Dialogue: the senders (`sendDialogue`, `sendNPCDialogue`,
//! `sendItemDialogue`, `sendDoubleItemDialogue`, `interpreter.sendOptions`)
//! run the native DialogueInterpreter of the player (`world::dialogue`,
//! [`super::talk`]); `ClimbActionHandler.ClimbDialogue` is a native
//! DialoguePlugin ([`ClimbPlugin`]) whose `handle` is
//! `climb::climb_dialogue_handle`.
use super::{
    P,
    global::{GHost, GNode},
    host::Live,
    trading::TradeHost,
};
use crate::world::{
    dialogue::{self as d, Arg, Plugin, PluginBody},
    global_handlers::{self as gh, climb, registry},
    interface_listeners::{self, Button, Callback},
    interfaces::{self, Handle as Component, Manager},
    player_walk::NodeHandle,
    trade,
};
use std::any::Any;

/// `InterfaceContainer.index`'s initial value. The index is one value per
/// world (`GlobalState::container_index`); InterfaceContainer is not
/// otherwise installed in the live world (the Grand Exchange library keeps
/// its own copy and is not installed either).
pub const CONTAINER_INDEX_START: i32 = 600;

impl Live {
    /// Runs an InterfaceManager operation with the whole world as its
    /// interface context.
    pub fn with_world_ui<T>(
        &mut self,
        p: &P,
        f: impl FnOnce(&mut trade::ui::Ui<'_, TradeHost<'_>>, &Manager) -> interfaces::Result<T>,
    ) -> interfaces::Result<T> {
        let manager = self.inner.canonical(p).player.manager();
        let owner = p.0.clone();
        let mut host = TradeHost { live: self };
        let result = f(&mut trade::ui::Ui::new(&mut host, &owner), &manager);
        self.drain_all();
        result
    }

    /// `InterfaceListeners.runOpen` (`close` false) or `runClose` (`close`
    /// true) when the interface's listener is one of the global handlers';
    /// `None` when it is not (the caller's context answers).
    pub fn global_interface_listener(
        &mut self,
        p: &NodeHandle,
        component: &Component,
        close: bool,
    ) -> Option<interfaces::Result<bool>> {
        // InterfaceListeners.runOpen/runClose (the registry's own lookup);
        // the callback is run here only when it is a global handler's.
        let registry = self.listeners.clone();
        let mut ctx = GlobalListeners {
            live: self,
            handled: false,
        };
        let result = if close {
            registry.run_close(&mut ctx, Some(p), Some(component))
        } else {
            registry.run_open(&mut ctx, Some(p), Some(component))
        };
        // A failed lookup (a listener of the wrong arity) fails here too.
        if ctx.handled || result.is_err() {
            Some(result)
        } else {
            None
        }
    }

    /// `player.getDialogueInterpreter().setDialogue(dialogue)` for the climb
    /// dialogue: a new [`ClimbPlugin`] made with `DialoguePlugin(player)`.
    pub fn set_climb_dialogue(
        &mut self,
        p: &P,
        dialogue: climb::ClimbDialogue<GNode>,
    ) -> d::Result<()> {
        let plugin = Plugin::new(self, Some(p.0.clone()), ClimbPlugin(Some(dialogue)))?;
        self.interpreter_of(p).set_dialogue(Some(plugin));
        Ok(())
    }
}

/// The InterfaceListeners context of [`Live::global_interface_listener`]:
/// runs a global handler's open or close listener and marks it handled;
/// any other listener is left to the caller's interface context.
struct GlobalListeners<'a> {
    live: &'a mut Live,
    handled: bool,
}
impl interface_listeners::Context for GlobalListeners<'_> {
    fn invoke_open(
        &mut self,
        callback: Callback,
        player: &NodeHandle,
        component: &Component,
    ) -> interfaces::Result<bool> {
        let Some(listener) = callback.value::<registry::Listener>().map(|l| *l) else {
            return Ok(false);
        };
        self.handled = true;
        let id = component.read().id;
        self.live
            .global_open(listener, player, id)
            .map_err(|e| interfaces::Error::Binding(format!("{e:?}")))
    }
    fn invoke_button(
        &mut self,
        _: Callback,
        _: &NodeHandle,
        _: &Component,
        _: Button,
    ) -> interfaces::Result<bool> {
        Err(interfaces::Error::Binding(
            "a button listener through the open/close listener context".into(),
        ))
    }
}

/// `ClimbActionHandler.ClimbDialogue`: its player and object are the
/// library's [`climb::ClimbDialogue`]; `None` is the prototype the
/// no-argument constructor makes.
pub struct ClimbPlugin(pub Option<climb::ClimbDialogue<GNode>>);

/// `ClimbDialogue.ID`: `8 << 16`.
pub const CLIMB_DIALOGUE_ID: i32 = 8 << 16;

impl PluginBody<Live> for ClimbPlugin {
    fn class_name(&self) -> &str {
        "core.game.global.action.ClimbActionHandler$ClimbDialogue"
    }
    fn ids(&self, _: &mut Live, _: &Plugin<Live>) -> d::Result<Vec<i32>> {
        Ok(vec![CLIMB_DIALOGUE_ID])
    }
    fn new_instance(
        &self,
        host: &mut Live,
        _: &Plugin<Live>,
        player: Option<NodeHandle>,
    ) -> d::Result<Option<Plugin<Live>>> {
        // new ClimbDialogue(player): the object is set by open(args).
        let dialogue = player.clone().map(|p| climb::ClimbDialogue {
            player: GNode::Entity(P(p)),
            object: None,
        });
        Ok(Some(Plugin::new(host, player, ClimbPlugin(dialogue))?))
    }
    fn handle(
        &self,
        host: &mut Live,
        this: &Plugin<Live>,
        _interface_id: i32,
        button_id: i32,
    ) -> d::Result<bool> {
        // `switch (stage) { case 0: ... }`: any other stage answers true.
        if this.stage() != 0 {
            return Ok(true);
        }
        let dialogue = self.0.as_ref().ok_or(d::Error::Null)?;
        climb::climb_dialogue_handle(&mut GHost { live: host }, dialogue, button_id)
            .map_err(|e| d::Error::Binding(format!("{e:?}")))
    }
    fn open(&self, _: &mut Live, _: &Plugin<Live>, _: &[Arg<Live>]) -> d::Result<bool> {
        // ClimbActionHandler opens the dialogue itself (climb.rs sends the
        // options before setDialogue); the live world never opens this
        // plugin through the interpreter.
        Err(d::Error::Binding(
            "ClimbDialogue.open through the interpreter is not reached".into(),
        ))
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// `InterfaceContainer.generateItems(player, items, options, iface, child,
/// x, y, key)`: script 150, the interface settings, the container packet
/// with the key, then `increment()` of the world's index.
#[allow(clippy::too_many_arguments)]
pub fn generate_items(
    live: &mut Live,
    p: &P,
    items: &[Option<GNode>],
    options: &[&str],
    iface: i32,
    child: i32,
    x: i32,
    y: i32,
    key: i32,
) -> gh::Result<i32> {
    use rs09_protocol::game::updates::{self, ScriptArg, WireItem};
    fn bind(e: impl std::fmt::Debug) -> gh::Error {
        gh::Error::Binding(Some(format!("{e:?}")))
    }
    // generateScriptArguments(options.length) and populateScript.
    let mut types = String::from("IviiiI");
    types.push_str(&"s".repeat(options.len()));
    let mut objects: Vec<ScriptArg> = options
        .iter()
        .map(|o| ScriptArg::Text(o.as_bytes().to_vec()))
        .collect();
    objects.extend([
        ScriptArg::Int(-1),
        ScriptArg::Int(0),
        ScriptArg::Int(x),
        ScriptArg::Int(y),
        ScriptArg::Int(key),
        ScriptArg::Int(iface.wrapping_shl(16) | child),
    ]);
    let counter = live.inner.canonical(p).player.manager().packet_count(1);
    let frame =
        updates::run_client_script(150, types.as_bytes(), &objects, counter).map_err(bind)?;
    live.inner.queue_native(p, frame);
    // sendIfaceSettings(IfaceSettingsBuilder().enableAllOptions().build(),
    // child, iface, 0, itemArray.length).
    let counter = live.inner.canonical(p).player.manager().packet_count(1);
    live.inner.queue_native(
        p,
        updates::interface_events(
            iface.wrapping_shl(16) | child,
            0,
            items.len() as i32,
            gh::ALL_OPTIONS,
            counter,
        ),
    );
    // ContainerPacket(ContainerContext(player, -1, -2, key, items,
    // items.length, false)).
    let mut wire = Vec::with_capacity(items.len());
    for item in items {
        wire.push(match item {
            None => None,
            Some(GNode::Item(i)) => {
                let i = i.read();
                Some(WireItem {
                    id: i.id(),
                    amount: i.amount(),
                })
            }
            Some(_) => return Err(gh::Error::Cast),
        });
    }
    let pointer = (-1i32).wrapping_shl(16) | (-2i32 & 0xFFFF);
    let frame = updates::inventory_full(pointer, key & 0xFFFF, &wire).map_err(bind)?;
    live.inner.queue_native(p, frame);
    // increment(): the previous index, the index wrapped at 6999.
    let index = &mut live.global.container_index;
    if *index == 6999 {
        *index = CONTAINER_INDEX_START;
    }
    let previous = *index;
    *index = index.wrapping_add(1);
    Ok(previous)
}
