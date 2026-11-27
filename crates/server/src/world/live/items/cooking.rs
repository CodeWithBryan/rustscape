//! Cooking on ranges and fires in the live world (`world::cooking`,
//! docs/cooking.md): CookingRewrite's use-with listener registered in the
//! native InteractionListeners at boot, its pulses as STANDARD pulses of the
//! player's canonical pulse manager on `GameWorld.Pulser`, and
//! CookingDialogue as a native DialogueFile of the player's interpreter
//! (the "How many would you like to cook?" chatbox 307 and the sinew
//! options), with the amount prompt through `world::input_prompt`.
//!
//! [`Cook`] is `cooking::Host` over the live world. Content the world does
//! not have answers by name (docs/live-world.md): the quest repository
//! (Cook's Assistant is not complete, so the Lumbridge range 114 refuses),
//! the Hasty Cooking skillcape perk (off) and the ResourceProducedEvent
//! hooks (skipped); the tutorial island branch is refused.
use super::{Ctx, Node, P};
use crate::{
    definitions::item::Shared,
    inventory::item::Item as NativeItem,
    pulses,
    world::{
        animation::{Animation, Priority},
        cooking::{self, AmountInput, CookingPulse, Dialogue, Error, Input, Result},
        dialogue::{self as d, File, FileBody},
        packet_handlers::{Base, Container as Held},
    },
};
use rs09_game_core::java_lower::lowercase_root;
use rs09_protocol::game::updates;
use std::sync::Mutex;

/// The cooking pulses the world runs, with their player's node.
#[derive(Default)]
pub struct CookingWorld {
    pub pulses: Vec<(pulses::Handle, usize, CookingPulse<Node>)>,
    /// CookingRewrite was registered at boot.
    pub registered: bool,
}

fn refuse<T>(what: &str) -> Result<T> {
    Err(Error::Binding(Some(format!(
        "not ported in the live world: {what}"
    ))))
}
fn bind(e: impl std::fmt::Debug) -> Error {
    Error::Binding(Some(format!("{e:?}")))
}
fn utf16(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

/// `cooking::Host` over the live world; `file` is the CookingDialogue being
/// handled, for its `options` and `end`.
pub struct Cook<'a> {
    pub ctx: Ctx<'a>,
    pub file: Option<File<super::super::host::Live>>,
}

impl Cook<'_> {
    fn player(&self, node: &Node) -> Result<P> {
        self.ctx.player_of(node).map_err(|_| Error::Cast)
    }
    fn native(&self, id: i32) -> crate::inventory::item::Handle {
        Shared::new(NativeItem::new(id, 1, 1000, &self.ctx.0.inner.items))
    }
    fn file(&self) -> Result<File<super::super::host::Live>> {
        self.file.clone().ok_or(Error::Null)
    }
}

impl cooking::Host for Cook<'_> {
    type Node = Node;

    fn id(&mut self, node: &Node) -> Result<i32> {
        Base::id(&mut self.ctx, node).map_err(bind)
    }
    fn scenery_name(&mut self, scenery: &Node) -> Result<String> {
        let Node::Scenery(object) = scenery else {
            return Err(Error::Cast);
        };
        let object = object.read();
        if let Some(name) = &object.name {
            return Ok(name.clone());
        }
        let definition = object.definition.clone().ok_or(Error::Null)?;
        let name = definition.read().name.clone();
        name.ok_or(Error::Null)
    }
    fn scenery_active(&mut self, scenery: &Node) -> Result<bool> {
        let Node::Scenery(object) = scenery else {
            return Err(Error::Cast);
        };
        Ok(object.read().active)
    }
    fn item_name(&mut self, id: i32) -> Result<String> {
        let name = self.ctx.0.inner.items.for_id(id).read().name.clone();
        name.ok_or(Error::Null)
    }
    fn lowercase(&self, text: &str) -> String {
        lowercase_root(text)
    }
    fn tutorial_complete(&mut self, player: &Node) -> Result<bool> {
        let p = self.player(player)?;
        self.ctx
            .0
            .inner
            .canonical(&p)
            .player
            .attributes()
            .boolean("tutorial:complete", false)
            .map_err(bind)
    }
    fn tutorial_cook(&mut self, _: &Node, _: &Node) -> Result<()> {
        refuse("cooking before the tutorial is complete (tutorial island content)")
    }
    fn amount_in_inventory(&mut self, player: &Node, id: i32) -> Result<i32> {
        self.inventory_amount(player, id)
    }
    fn inventory_amount(&mut self, player: &Node, id: i32) -> Result<i32> {
        let inventory = self.ctx.container(player, Held::Inventory).map_err(bind)?;
        Ok(inventory.amount_id(id, &self.ctx.0.inner.items))
    }
    fn inventory_add(&mut self, player: &Node, id: i32) -> Result<bool> {
        let item = self.native(id);
        self.ctx
            .with_container(player, Held::Inventory, |c, defs, ctx| {
                c.add(Some(&item), true, -1, defs, ctx)
            })
            .map_err(bind)
    }
    fn inventory_remove(&mut self, player: &Node, id: i32) -> Result<bool> {
        let item = self.native(id);
        self.ctx
            .with_container(player, Held::Inventory, |c, defs, ctx| {
                c.remove(Some(&item), None, true, defs, ctx)
            })
            .map_err(bind)
    }
    fn ground_create(&mut self, player: &Node, id: i32, _at_location: bool) -> Result<()> {
        // Both overloads drop at the player's location for the player.
        let p = self.player(player)?;
        let at = self
            .ctx
            .0
            .inner
            .state
            .node_location(p.node())
            .map_err(bind)?;
        let item = self.native(id);
        self.ctx
            .0
            .ground_create(item, at, Some(p.0.clone()))
            .map(|_| ())
            .map_err(bind)
    }
    fn equipment_contains(&mut self, player: &Node, id: i32) -> Result<bool> {
        let p = self.player(player)?;
        let items = self.ctx.0.inner.canonical(&p).player.equipment().items();
        let found = items
            .read()
            .iter()
            .flatten()
            .any(|item| item.read().id() == id);
        Ok(found)
    }
    fn send_message(&mut self, player: &Node, text: Option<&str>) -> Result<()> {
        let Some(text) = text else { return Ok(()) };
        let p = self.player(player)?;
        self.ctx.0.inner.message(&p, text).map_err(bind)
    }
    fn send_dialogue(&mut self, player: &Node, text: &str) -> Result<()> {
        let p = self.player(player)?;
        let interpreter = self.ctx.0.interpreter_of(&p);
        let sent = interpreter
            .send_dialogue(self.ctx.0, Some(&d::line(&utf16(text))))
            .map(|_| ())
            .map_err(bind);
        self.ctx.0.inner.drain(&p);
        sent
    }
    fn quest_complete(&mut self, _: &Node, _: &str) -> Result<bool> {
        self.ctx
            .0
            .inner
            .not_ported
            .default_answer("QuestRepository.isComplete for cooking (quests not ported: false)");
        Ok(false)
    }
    fn level(&mut self, player: &Node, skill: i32) -> Result<i32> {
        let p = self.player(player)?;
        crate::world::combat_swing::Host::level(self.ctx.0, &p, skill).map_err(bind)
    }
    fn hasty_cooking(&mut self, _: &Node) -> Result<bool> {
        self.ctx
            .0
            .inner
            .not_ported
            .default_answer("SkillcapePerks.isActive(HASTY_COOKING) (skillcape perks off: false)");
        Ok(false)
    }
    fn random_double(&mut self, max: f64) -> Result<f64> {
        crate::world::drop_tables::random::random_double(&mut self.ctx.0.inner.generators, max)
            .map_err(bind)
    }
    fn random(&mut self, max: i32) -> Result<i32> {
        Ok(crate::world::drop_tables::random::random(
            &mut self.ctx.0.inner.generators,
            max,
        ))
    }
    fn resource_produced(&mut self, _: &Node, _: i32, _: i32, _: &Node, _: i32) -> Result<()> {
        self.ctx
            .0
            .inner
            .not_ported
            .skip("ResourceProducedEvent dispatch (entity event hooks not ported)");
        Ok(())
    }
    fn add_experience(&mut self, player: &Node, skill: i32, experience: f64) -> Result<()> {
        let p = self.player(player)?;
        let added = crate::world::combat_pulse::Host::add_experience(
            self.ctx.0, &p, skill, experience, true,
        )
        .map_err(bind);
        self.ctx.0.inner.drain(&p);
        added
    }
    fn dupe_alert(&mut self, _: &Node, message: &str) -> Result<()> {
        self.ctx
            .0
            .inner
            .log_line(format!("[PlayerMonitor] DUPE_ALERT {message}"));
        Ok(())
    }
    fn player_name(&mut self, player: &Node) -> Result<String> {
        let p = self.player(player)?;
        Ok(self.ctx.0.inner.slot(&p).name.clone())
    }
    fn increment_attribute(&mut self, player: &Node, key: &str, amount: i32) -> Result<()> {
        // attributes.setAttribute(key, attributes.getAttribute(key, 0) + amount).
        let p = self.player(player)?;
        let attributes = self.ctx.0.inner.canonical(&p).player.attributes();
        let read = key.strip_prefix("/save:").unwrap_or(key);
        let current = attributes.integer(read, 0).map_err(|_| Error::Cast)?;
        attributes.set_value(
            key,
            Some(crate::player::attributes::AttributeValue::Int(
                current.wrapping_add(amount),
            )),
        );
        Ok(())
    }
    fn play_audio(&mut self, player: &Node, id: i32) -> Result<()> {
        let p = self.player(player)?;
        self.ctx
            .0
            .inner
            .queue_native(&p, updates::synth_sound(id, 1, 0));
        Ok(())
    }
    fn animate_high(&mut self, player: &Node, id: i32) -> Result<()> {
        let p = self.player(player)?;
        self.ctx
            .0
            .entity_visualize(&p, Some(Animation::with(id, 0, Some(Priority::High))), None)
            .map_err(bind)
    }
    fn run_pulse(&mut self, player: &Node, mut pulse: CookingPulse<Node>) -> Result<()> {
        let p = self.player(player)?;
        // `Pulse()`: delay 1, no checked nodes.
        let handle = Shared::new(pulses::Pulse::new(
            1,
            Some(Shared::new(Vec::new())),
            pulses::Class::Other,
            pulse.kind.class().into(),
        ));
        let live = &mut *self.ctx.0;
        let mut started = Ok(());
        let submitted = live
            .run_content_pulse(&p, handle.clone(), |live| {
                started = pulse.start(&mut Cook {
                    ctx: Ctx(live),
                    file: None,
                });
                started.is_ok() && pulse.running
            })
            .map_err(bind)?;
        if let Err(error) = started {
            // `start()` threw inside PulseManager.run: nothing is submitted.
            pulses::stop_base(&handle);
            return Err(error);
        }
        if submitted {
            handle.write().delay = pulse.delay;
            let world = &mut self.ctx.0.round.cooking;
            world.pulses.retain(|(h, _, _)| h.read().running);
            world.pulses.push((handle, p.node(), pulse));
        }
        Ok(())
    }
    fn open_dialogue(&mut self, player: &Node, dialogue: Dialogue<Node>) -> Result<()> {
        let p = self.player(player)?;
        let file = File::new(CookFile(Mutex::new(dialogue)));
        let interpreter = self.ctx.0.interpreter_of(&p);
        let opened = interpreter
            .open_file(self.ctx.0, Some(&file), &[])
            .map_err(bind);
        self.ctx.0.inner.drain(&p);
        opened
    }
    fn dialogue_options(&mut self, player: &Node, lines: &[&str]) -> Result<()> {
        let p = self.player(player)?;
        let lines: Vec<Vec<u16>> = lines.iter().map(|line| utf16(line)).collect();
        let sent = self
            .file()?
            .options(self.ctx.0, &crate::world::dialogue_dsl::lines(&lines), None)
            .map_err(bind);
        self.ctx.0.inner.drain(&p);
        sent
    }
    fn dialogue_end(&mut self, player: &Node) -> Result<()> {
        let p = self.player(player)?;
        let ended = self.file()?.end(self.ctx.0).map_err(bind);
        self.ctx.0.inner.drain(&p);
        ended
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
            self.ctx.0, p, item, zoom, interface, child,
        )
        .map_err(bind)
    }
    fn send_string(&mut self, player: &Node, text: &str, interface: i32, child: i32) -> Result<()> {
        let p = self.player(player)?;
        self.ctx
            .0
            .inner
            .text(&p, text, interface, child)
            .map_err(bind)
    }
    fn reposition_child(
        &mut self,
        player: &Node,
        interface: i32,
        child: i32,
        x: i32,
        y: i32,
    ) -> Result<()> {
        let p = self.player(player)?;
        d::Host::send_reposition_on_interface(self.ctx.0, p, interface, child, x, y).map_err(bind)
    }
    fn open_chatbox(&mut self, player: &Node, interface: i32) -> Result<()> {
        let p = self.player(player)?;
        let manager = self.ctx.0.inner.canonical(&p).player.manager();
        d::Host::open_chatbox(self.ctx.0, manager, interface).map_err(bind)
    }
    fn send_input(
        &mut self,
        player: &Node,
        prompt: &str,
        handler: AmountInput<Node>,
    ) -> Result<()> {
        let p = self.player(player)?;
        self.ctx
            .0
            .cook_prompt(&p, prompt, CookScript(p.clone(), handler))
            .map_err(bind)
    }
}

/// CookingDialogue as a DialogueFile of the native interpreter.
pub struct CookFile(Mutex<Dialogue<Node>>);

impl FileBody<super::super::host::Live> for CookFile {
    fn class_name(&self) -> &str {
        cooking::DIALOGUE_CLASS
    }
    fn handle(
        &self,
        host: &mut super::super::host::Live,
        this: &File<super::super::host::Live>,
        _component: i32,
        button: i32,
    ) -> d::Result<()> {
        let player = this.player().ok_or(d::Error::Null)?;
        let node = Node::Player(player);
        let mut state = self.0.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let result = state.handle(
            &mut Cook {
                ctx: Ctx(host),
                file: Some(this.clone()),
            },
            &node,
            button,
        );
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = state;
        result.map_err(|e| d::Error::Binding(format!("{e:?}")))
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// The amount prompt's handler with its player.
#[derive(Clone)]
pub struct CookScript(pub P, pub AmountInput<Node>);

impl super::super::host::Live {
    /// CookingRewrite's use-with lambda.
    pub fn cooking_use(&mut self, player: &Node, used: &Node, with: &Node) -> Result<bool> {
        cooking::use_with(
            &mut Cook {
                ctx: Ctx(self),
                file: None,
            },
            player,
            used,
            with,
        )
    }
    /// The amount prompt answered: `cook(player, object, initial, product,
    /// value)`.
    pub fn cooking_amount(&mut self, script: &CookScript, value: &Input) -> Result<()> {
        let node = Node::Player(script.0.0.clone());
        script.1.run(
            &mut Cook {
                ctx: Ctx(self),
                file: None,
            },
            &node,
            value,
        )
    }
    /// A cooking pulse by identity: `Some` with whether it finished.
    pub fn cooking_pulse_body(&mut self, pulse: &pulses::Handle) -> Option<Result<bool>> {
        let pulses = &mut self.round.cooking.pulses;
        let at = pulses.iter().position(|(h, _, _)| h.same(pulse))?;
        let (handle, owner, mut task) = pulses.remove(at);
        if !self.inner.players.contains_key(&owner) {
            // The player left: the pulse ends.
            return Some(Ok(true));
        }
        let result = task.pulse(&mut Cook {
            ctx: Ctx(self),
            file: None,
        });
        if matches!(result, Ok(false)) {
            // `setDelay` in `reward`.
            handle.write().delay = task.delay;
            self.round.cooking.pulses.push((handle, owner, task));
        }
        Some(result)
    }
}
