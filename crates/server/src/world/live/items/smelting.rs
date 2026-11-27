//! Smelting at furnaces in the live world (`world::smelting`, ported
//! without a JVM corpus: docs/smelting.md): FurnaceOptionPlugin on
//! scenery's "smelt" and "smelt-ore" options and on the "use" option of
//! 3044 and 21303, its SmeltUseWithHandler for the thirteen furnaces in
//! `UseWithHandler.HANDLERS`, SmeltingInterface as the component plugin of
//! the chatbox 311, and SmeltingPulse as a STANDARD pulse of the player's
//! pulse manager whose `SkillPulse.stop()` (the animation reset) also runs
//! when the manager stops it.
//!
//! `Smelt` is the library's `Host` over the live world: the canonical
//! inventory and equipment, the native Skills, the persisted diaries
//! (`world::player_managers::diary` through super::super::subclasses),
//! `RandomFunction.RANDOM`, the animator, the synth sound, the attributes
//! and the input prompt. The Knight's Sword answers false by name (no quest
//! is ported), so blurite is never smelted; the ResourceProducedEvent is a
//! named skip.
use super::{super::host::Live, Ctx, Node, P};
use crate::{
    definitions::{item::Shared, option_handler},
    inventory::item::Item as NativeItem,
    player::attributes::AttributeValue,
    pulses,
    world::{
        animation::{Animation, Graphics, Priority},
        component_objects::{NativeObject, Plugin},
        dialogue as d,
        packet_handlers::{
            Base, Container as Held,
            use_with::{HandlerData, UseHandler},
        },
        player_managers::diary,
        smelting::{self, AmountInput, Diary, Error, Input, Result, SmeltingPulse},
    },
};
use rs09_game_core::java_lower::lowercase_root;
use rs09_protocol::game::updates;

/// `SmeltUseWithHandler`'s id among this world's use-with handlers.
pub const SMELT_HANDLER: usize = 2;

/// The FurnaceOptionPlugin object on the scenery definitions.
#[derive(Clone, Copy, Debug)]
pub struct FurnacePlugin;
impl option_handler::NativeOptionHandler for FurnacePlugin {}

/// SmeltingInterface, the component plugin of 311.
pub struct SmeltPlugin;
impl NativeObject for SmeltPlugin {}

/// The amount prompt's script: the player and the handler.
#[derive(Clone)]
pub struct SmeltScript(pub P, pub AmountInput);

/// The plugin's objects and running pulses.
#[derive(Default)]
pub struct SmeltingWorld {
    /// The plugin (`this` of `newInstance`).
    pub plugin: Option<option_handler::Handle>,
    /// `new SmeltUseWithHandler()`.
    pub handler: Option<UseHandler>,
    /// The smelting pulses, by their pulse object, with the player's node.
    pub pulses: Vec<(pulses::Handle, usize, SmeltingPulse<Node>)>,
    /// FurnaceOptionPlugin and SmeltingInterface were registered at boot.
    pub registered: (bool, bool),
}

fn bind(e: impl std::fmt::Debug) -> Error {
    Error::Binding(Some(format!("{e:?}")))
}

/// `DiaryType`'s ordinal.
fn kind(diary: Diary) -> usize {
    match diary {
        Diary::Varrock => 1,
        Diary::Lumbridge => 2,
    }
}

/// `world::smelting`'s host over the live world.
pub struct Smelt<'a>(pub Ctx<'a>);

impl Smelt<'_> {
    fn player(&self, node: &Node) -> Result<P> {
        self.0.player_of(node).map_err(|_| Error::Cast)
    }
    fn native(&self, id: i32, amount: i32) -> crate::inventory::item::Handle {
        Shared::new(NativeItem::new(id, amount, 1000, &self.0.0.inner.items))
    }
    fn diaries<T>(
        &mut self,
        player: &Node,
        f: impl FnOnce(
            &mut super::super::subclasses::Diaries<'_>,
            &P,
        ) -> crate::world::player_managers::Result<T>,
    ) -> Result<T> {
        let p = self.player(player)?;
        super::super::subclasses::with_diaries(self.0.0, &p, f).map_err(bind)
    }
}

impl smelting::Host for Smelt<'_> {
    type Node = Node;

    fn id(&mut self, node: &Node) -> Result<i32> {
        Base::id(&mut self.0, node).map_err(bind)
    }
    fn item_name(&mut self, id: i32) -> Result<String> {
        let name = self.0.0.inner.items.for_id(id).read().name.clone();
        name.ok_or(Error::Null)
    }
    fn lowercase(&self, text: &str) -> String {
        lowercase_root(text)
    }
    fn close_chatbox(&mut self, player: &Node) -> Result<()> {
        let p = self.player(player)?;
        self.0.0.inner.current = Some(p.clone());
        let manager = self.0.0.inner.canonical(&p).player.manager();
        let closed =
            crate::world::interfaces::close_chatbox(&mut self.0.0.inner, &manager).map_err(bind);
        self.0.0.inner.drain(&p);
        closed
    }
    fn open_chatbox(&mut self, player: &Node, interface: i32) -> Result<()> {
        let p = self.player(player)?;
        let manager = self.0.0.inner.canonical(&p).player.manager();
        d::Host::open_chatbox(self.0.0, manager, interface).map_err(bind)
    }
    fn send_item_zoom(
        &mut self,
        player: &Node,
        item: i32,
        zoom: i32,
        interface: i32,
        child: i32,
    ) -> Result<()> {
        let p = self.player(player)?;
        crate::world::dialogue_dsl::Host::send_item_zoom_on_interface(
            self.0.0, p, item, zoom, interface, child,
        )
        .map_err(bind)
    }
    fn send_string(&mut self, player: &Node, text: &str, interface: i32, child: i32) -> Result<()> {
        let p = self.player(player)?;
        self.0
            .0
            .inner
            .text(&p, text, interface, child)
            .map_err(bind)
    }
    fn quest_complete(&mut self, _: &Node, _: &str) -> Result<bool> {
        self.0.0.inner.not_ported.default_answer(
            "QuestRepository.isComplete for smelting (quests not ported: false, no blurite)",
        );
        Ok(false)
    }
    fn level(&mut self, player: &Node, skill: i32) -> Result<i32> {
        let p = self.player(player)?;
        crate::world::combat_swing::Host::level(self.0.0, &p, skill).map_err(bind)
    }
    fn send_message(&mut self, player: &Node, text: &str) -> Result<()> {
        let p = self.player(player)?;
        self.0.0.inner.message(&p, text).map_err(bind)
    }
    fn inventory_contains(&mut self, player: &Node, id: i32, amount: i32) -> Result<bool> {
        let inventory = self.0.container(player, Held::Inventory).map_err(bind)?;
        Ok(inventory.contains(id, amount))
    }
    fn inventory_remove(&mut self, player: &Node, id: i32, amount: i32) -> Result<bool> {
        let item = self.native(id, amount);
        self.0
            .with_container(player, Held::Inventory, |c, defs, ctx| {
                c.remove(Some(&item), None, true, defs, ctx)
            })
            .map_err(bind)
    }
    fn inventory_remove_all(&mut self, player: &Node, items: &[(i32, i32)]) -> Result<bool> {
        let items = Shared::new(
            items
                .iter()
                .map(|(id, amount)| Some(self.native(*id, *amount)))
                .collect::<Vec<_>>(),
        );
        self.0
            .with_container(player, Held::Inventory, |c, defs, ctx| {
                c.remove_many(&items, defs, ctx)
            })
            .map_err(bind)
    }
    fn inventory_add(&mut self, player: &Node, id: i32, amount: i32) -> Result<bool> {
        let item = self.native(id, amount);
        self.0
            .with_container(player, Held::Inventory, |c, defs, ctx| {
                c.add(Some(&item), true, -1, defs, ctx)
            })
            .map_err(bind)
    }
    fn free_slots(&mut self, player: &Node) -> Result<i32> {
        let inventory = self.0.container(player, Held::Inventory).map_err(bind)?;
        Ok(inventory.free_slots())
    }
    fn location(&mut self, player: &Node) -> Result<[i32; 3]> {
        let p = self.player(player)?;
        self.0
            .0
            .inner
            .state
            .node_location(p.node())
            .map_err(bind)?
            .map(|l| l.coordinates())
            .ok_or(Error::Null)
    }
    fn diary_level(&mut self, player: &Node, which: Diary) -> Result<i32> {
        self.diaries(player, |h, owner| {
            let manager = diary::Host::diaries(h, owner)?;
            let found = manager.diaries.iter().find(|d| d.kind == kind(which));
            diary::completed_level(found.ok_or(crate::world::player_managers::Error::Null)?)
        })
    }
    fn diary_complete(&mut self, player: &Node, which: Diary, level: i32) -> Result<bool> {
        self.diaries(player, |h, owner| {
            let manager = diary::Host::diaries(h, owner)?;
            let found = manager.diaries.iter().find(|d| d.kind == kind(which));
            diary::level_complete(
                found.ok_or(crate::world::player_managers::Error::Null)?,
                level,
            )
        })
    }
    fn check_smith_reward(&mut self, player: &Node, bar: usize) -> Result<bool> {
        self.diaries(player, |h, owner| {
            diary::check_smith_reward(h, owner, Some(bar))
        })
    }
    fn diary_finish_task(
        &mut self,
        player: &Node,
        which: Diary,
        level: i32,
        index: i32,
    ) -> Result<()> {
        self.diaries(player, |h, owner| {
            diary::finish_task(h, owner, owner, Some(kind(which)), level, index)
        })
    }
    fn random(&mut self, max: i32) -> Result<i32> {
        Ok(crate::world::drop_tables::random::random(
            &mut self.0.0.inner.generators,
            max,
        ))
    }
    fn next_bool(&mut self) -> Result<bool> {
        Ok(crate::world::drop_tables::random::Source::next_boolean(
            &mut self.0.0.inner.generators,
        ))
    }
    fn resource_produced(&mut self, _: &Node, _: i32, _: i32) -> Result<()> {
        self.0
            .0
            .inner
            .not_ported
            .skip("ResourceProducedEvent dispatch (entity event hooks not ported)");
        Ok(())
    }
    fn hands(&mut self, player: &Node) -> Result<Option<i32>> {
        // equipment.get(EquipmentContainer.SLOT_HANDS).
        let equipment = self.0.container(player, Held::Equipment).map_err(bind)?;
        Ok(equipment.get(9).map(|item| item.read().id()))
    }
    fn add_experience(&mut self, player: &Node, skill: i32, experience: f64) -> Result<()> {
        let p = self.player(player)?;
        let added =
            crate::world::combat_pulse::Host::add_experience(self.0.0, &p, skill, experience, true)
                .map_err(bind);
        self.0.0.inner.drain(&p);
        added
    }
    fn in_equipment(&mut self, player: &Node, id: i32, amount: i32) -> Result<bool> {
        // amountInEquipment(player, id) >= amount.
        let equipment = self.0.container(player, Held::Equipment).map_err(bind)?;
        Ok(equipment.amount_id(id, &self.0.0.inner.items) >= amount)
    }
    fn attribute_int(&mut self, player: &Node, key: &str, default: i32) -> Result<i32> {
        let p = self.player(player)?;
        let attributes = self.0.0.inner.canonical(&p).player.attributes();
        attributes.integer(key, default).map_err(|_| Error::Cast)
    }
    fn set_attribute_int(&mut self, player: &Node, key: &str, value: i32) -> Result<()> {
        let p = self.player(player)?;
        let attributes = self.0.0.inner.canonical(&p).player.attributes();
        attributes.set_value(key, Some(AttributeValue::Int(value)));
        Ok(())
    }
    fn remove_equipment(&mut self, player: &Node, id: i32) -> Result<bool> {
        // player.equipment.remove(Item(id)).
        let item = self.native(id, 1);
        self.0
            .with_container(player, Held::Equipment, |c, defs, ctx| {
                c.remove(Some(&item), None, true, defs, ctx)
            })
            .map_err(bind)
    }
    fn log_error(&mut self, message: &str) -> Result<()> {
        self.0
            .0
            .inner
            .log_line(format!("{} ERR {message}", smelting::PULSE_CLASS));
        Ok(())
    }
    fn player_name(&mut self, player: &Node) -> Result<String> {
        let p = self.player(player)?;
        Ok(self.0.0.inner.slot(&p).name.clone())
    }
    fn animate(&mut self, player: &Node, id: i32) -> Result<()> {
        let p = self.player(player)?;
        self.0
            .0
            .entity_visualize(&p, Some(Animation::new(id)), None)
            .map_err(bind)
    }
    fn visualize(&mut self, player: &Node, animation: i32, graphics: (i32, i32)) -> Result<()> {
        let p = self.player(player)?;
        self.0
            .0
            .entity_visualize(
                &p,
                Some(Animation::new(animation)),
                Some(Graphics::new(graphics.0, graphics.1, 0)),
            )
            .map_err(bind)
    }
    fn reset_animation(&mut self, player: &Node) -> Result<()> {
        let p = self.player(player)?;
        self.0
            .0
            .entity_visualize(&p, Some(Animation::with(-1, 0, Some(Priority::High))), None)
            .map_err(bind)
    }
    fn play_audio(&mut self, player: &Node, id: i32) -> Result<()> {
        let p = self.player(player)?;
        self.0
            .0
            .inner
            .queue_native(&p, updates::synth_sound(id, 1, 0));
        Ok(())
    }
    fn run_pulse(&mut self, player: &Node, mut pulse: SmeltingPulse<Node>) -> Result<()> {
        let p = self.player(player)?;
        // `SkillPulse(player, node)`: `Pulse(1, player, node)`. The player
        // leaving stops the pulse (super::super::round).
        let handle = Shared::new(pulses::Pulse::new(
            1,
            Some(Shared::new(Vec::new())),
            pulses::Class::Other,
            smelting::PULSE_CLASS.into(),
        ));
        let live = &mut *self.0.0;
        let mut started = Ok(());
        let submitted = live
            .run_content_pulse(&p, handle.clone(), |live| {
                started = pulse.start(&mut Smelt(Ctx(live)));
                started.is_ok() && pulse.running
            })
            .map_err(bind)?;
        if let Err(error) = started {
            // `start()` threw inside PulseManager.run: nothing is submitted.
            pulses::stop_base(&handle);
            return Err(error);
        }
        if submitted {
            live.walk.stopping_pulses.push(handle.clone());
            live.round.smelting.pulses.push((handle, p.node(), pulse));
        }
        Ok(())
    }
    fn send_input(&mut self, player: &Node, prompt: &str, handler: AmountInput) -> Result<()> {
        let p = self.player(player)?;
        self.0
            .0
            .smelt_prompt(&p, prompt, SmeltScript(p.clone(), handler))
            .map_err(bind)
    }
    fn in_zone(&mut self, player: &Node, name: &str) -> Result<bool> {
        let p = self.player(player)?;
        let monitor = self.0.0.monitor(&p.0);
        monitor.is_in_zone(Some(name)).map_err(bind)
    }
}

/// `FurnaceOptionPlugin.newInstance`'s registrations over the live world.
struct Install<'a>(&'a mut Live);
impl Install<'_> {
    fn plugin(&mut self) -> option_handler::Handle {
        self.0
            .round
            .smelting
            .plugin
            .get_or_insert_with(|| {
                option_handler::Handler::new(smelting::PLUGIN_CLASS, FurnacePlugin)
            })
            .clone()
    }
}
impl smelting::Registrar for Install<'_> {
    fn set_option_handler(&mut self, name: &str) -> Result<()> {
        let plugin = self.plugin();
        option_handler::set_option_handler(
            &self.0.walk.scenery.defaults(),
            Some(name.into()),
            Some(plugin),
        );
        Ok(())
    }
    fn put_handler(&mut self, id: i32, key: &str) -> Result<()> {
        let plugin = self.plugin();
        let definition = self.0.walk.scene.definitions.for_id(id);
        let handlers = definition.read().handlers.clone();
        handlers.write().insert(
            key.to_owned(),
            crate::definitions::property::Property::OptionHandler(plugin),
        );
        Ok(())
    }
    fn new_use_handler(&mut self, allowed: Vec<i32>) -> Result<()> {
        self.0.round.smelting.handler = Some(Shared::new(HandlerData {
            name: "SmeltUseWithHandler".into(),
            allowed_nodes: Some(allowed),
            dynamic: false,
            handler: SMELT_HANDLER,
        }));
        Ok(())
    }
    fn add_use_handler(&mut self, id: i32, kind: i32) -> Result<()> {
        let handler = self.0.round.smelting.handler.clone().ok_or(Error::Null)?;
        self.0
            .item_actions
            .use_handlers
            .add_handler(id, kind, &handler);
        Ok(())
    }
}

impl Live {
    /// `FurnaceOptionPlugin.newInstance(null)` and
    /// `SmeltingInterface.newInstance(null)` (`ComponentDefinition.put(311,
    /// plugin)`), each if the class scan names it.
    pub fn install_smelting(&mut self) -> std::result::Result<(), String> {
        let scanned = crate::startup::scan::section("scan.initializable");
        if scanned.contains(&smelting::INTERFACE_CLASS) {
            self.inner
                .registry
                .for_id(smelting::INTERFACE)
                .write()
                .plugin = Some(Plugin::new(SmeltPlugin));
            self.round.smelting.registered.1 = true;
        }
        if scanned.contains(&smelting::PLUGIN_CLASS) {
            smelting::new_instance(&mut Install(self))
                .map_err(|e| format!("{}: {e:?}", smelting::PLUGIN_CLASS))?;
            self.round.smelting.registered.0 = true;
        }
        Ok(())
    }

    /// `FurnaceOptionPlugin.handle(player, node, option)` and
    /// `SmeltUseWithHandler.handle(event)`.
    pub fn furnace_handle(&mut self, player: &P, furnace: &Node) -> Result<bool> {
        let node = Node::Player(player.0.clone());
        let handled = smelting::handle(&mut Smelt(Ctx(self)), &node, furnace);
        self.inner.drain(player);
        handled
    }

    /// `SmeltingInterface.handle`: a button of 311.
    pub fn smelting_button(&mut self, player: &P, button: i32) -> Result<bool> {
        let node = Node::Player(player.0.clone());
        let handled = smelting::interface_button(&mut Smelt(Ctx(self)), &node, button);
        self.inner.drain(player);
        handled
    }

    /// The amount prompt answered.
    pub fn smelting_amount(&mut self, script: &SmeltScript, value: &Input) -> Result<()> {
        let node = Node::Player(script.0.0.clone());
        script.1.run(&mut Smelt(Ctx(self)), &node, value)
    }

    /// A smelting pulse by identity: `Some` with whether the runner drops
    /// it.
    pub fn smelting_pulse_body(&mut self, pulse: &pulses::Handle) -> Option<Result<bool>> {
        let pulses = &mut self.round.smelting.pulses;
        let at = pulses.iter().position(|(h, _, _)| h.same(pulse))?;
        let (handle, owner, mut task) = pulses.remove(at);
        if !self.inner.players.contains_key(&owner) {
            // The player left: the pulse ends.
            return Some(Ok(true));
        }
        let result = task.update(&mut Smelt(Ctx(self)));
        if matches!(result, Ok(false)) {
            self.round.smelting.pulses.push((handle, owner, task));
        }
        Some(result)
    }

    /// A smelting pulse a pulse manager stopped: `SkillPulse.stop()`.
    pub fn smelting_pulse_stopped(&mut self, pulse: &pulses::Handle) -> Option<Result<()>> {
        let pulses = &mut self.round.smelting.pulses;
        let at = pulses.iter().position(|(h, _, _)| h.same(pulse))?;
        let (_, owner, mut task) = pulses.remove(at);
        if !self.inner.players.contains_key(&owner) {
            return Some(Ok(()));
        }
        Some(task.stop(&mut Smelt(Ctx(self))))
    }
}
