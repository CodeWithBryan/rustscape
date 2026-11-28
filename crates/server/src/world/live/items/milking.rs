//! CowMilkingPlugin and its BucketHandler in the live world
//! (`world::creatures::cow_milking`, replayed against the
//! equipment-registers oracle): the dairy cows' "milk" option, the
//! "steal-cowbell" option handler and a bucket used on a cow.
//!
//! The live world is `equipment_registers::Host` and `creatures::Host` for
//! this class only. `newInstance` puts the plugin on the two cow
//! definitions and on scenery's option handlers, and its BucketHandler in
//! `UseWithHandler.HANDLERS` (the first handler class there). The milking
//! pulse is a `CreaturePulse` the player's pulse manager runs (STANDARD):
//! its `update` runs every tick from the world's runner and its `stop`
//! (the animation reset) also when the manager stops it. The services of
//! the other classes of `world::creatures` and of the equipment registers
//! (salamanders, chinchompas, the hasta listener, sheep, the stray dog's
//! behaviour, decanting) are refused by name: none of them is installed.
//! The two dialogues the plugin opens (3807 without a bucket, 70099 for
//! "steal-cowbell") are dialogue plugins without a port: refused by name.
use super::{super::host::Live, Ctx, Node, P};
use crate::{
    definitions::{item::Shared, option_handler},
    inventory::item::Item as NativeItem,
    pulses,
    world::{
        combat_swing::{CombatStyle, Error, Graphics, Result},
        creatures::{self as cr, CreaturePulse, cow_milking},
        dragonfire::ItemValue,
        equipment_registers as er,
        packet_handlers::{
            Container as Held,
            use_with::{Event, HandlerData, OBJECT_TYPE, UseHandler},
        },
    },
};

/// The plugin's class.
pub const CLASS: &str = "content.global.handlers.npc.CowMilkingPlugin";
/// `BucketHandler`'s id among this world's use-with handlers.
pub const BUCKET_HANDLER: usize = 1;

/// The CowMilkingPlugin object on the scenery definitions.
#[derive(Clone, Copy, Debug)]
pub struct CowPlugin;
impl option_handler::NativeOptionHandler for CowPlugin {}

/// The plugin's objects and running pulses.
#[derive(Default)]
pub struct MilkingWorld {
    /// The plugin (`this` of `newInstance`).
    pub plugin: Option<option_handler::Handle>,
    /// `new BucketHandler()`.
    pub bucket: Option<UseHandler>,
    /// The milking pulses, by their pulse object.
    pub pulses: Vec<(pulses::Handle, CreaturePulse<Live>)>,
    /// CowMilkingPlugin was registered at boot.
    pub registered: bool,
}

fn refuse<T>(what: &str) -> Result<T> {
    Err(Error::Binding(format!(
        "not ported in the live world: {what}"
    )))
}
fn bind(e: impl std::fmt::Debug) -> Error {
    Error::Binding(format!("{e:?}"))
}

impl Live {
    fn milking_plugin(&mut self) -> option_handler::Handle {
        self.round
            .milking
            .plugin
            .get_or_insert_with(|| option_handler::Handler::new(CLASS, CowPlugin))
            .clone()
    }
    fn native_item(&self, (id, amount, charge): ItemValue) -> crate::inventory::item::Handle {
        Shared::new(NativeItem::new(id, amount, charge, &self.inner.items))
    }

    /// `CowMilkingPlugin.newInstance(null)` if the class scan names the
    /// plugin.
    pub fn install_cow_milking(&mut self) -> std::result::Result<(), String> {
        if !crate::startup::scan::section("scan.initializable").contains(&CLASS) {
            return Ok(());
        }
        cow_milking::new_instance(self).map_err(|e| format!("{CLASS}: {e:?}"))?;
        self.round.milking.registered = true;
        Ok(())
    }

    /// `UseWithHandler.handle(event)` of a handler of this world.
    pub fn use_handler_handle(
        &mut self,
        handler: usize,
        player: &P,
        event: &Event<Node>,
    ) -> Option<Result<bool>> {
        match handler {
            // BucketHandler.handle: `(Scenery) event.getUsedWith()` holds,
            // the handler is keyed by OBJECT_TYPE.
            BUCKET_HANDLER => Some(cow_milking::bucket_handle(self, player)),
            // SmeltUseWithHandler.handle: FurnaceOptionPlugin's handle.
            super::smelting::SMELT_HANDLER => {
                Some(self.furnace_handle(player, &event.with).map_err(bind))
            }
            // SmithingPlugin.handle.
            super::smithing::SMITH_HANDLER => Some(self.anvil_use(player, event).map_err(bind)),
            _ => None,
        }
    }

    /// A milking pulse by identity: `Some` with whether the runner drops it.
    pub fn milking_pulse_body(
        &mut self,
        pulse: &pulses::Handle,
    ) -> Option<std::result::Result<bool, String>> {
        let pulses = &mut self.round.milking.pulses;
        let at = pulses.iter().position(|(h, _)| h.same(pulse))?;
        let (handle, mut task) = pulses.remove(at);
        let result = task.update(self);
        if matches!(result, Ok(false)) {
            self.round.milking.pulses.push((handle, task));
        }
        Some(result.map_err(|e| format!("{e:?}")))
    }

    /// A content pulse a pulse manager stopped: the milking pulse's `stop()`
    /// override.
    pub fn content_pulse_stopped(&mut self, pulse: &pulses::Handle) -> Result<()> {
        let pulses = &mut self.round.milking.pulses;
        let Some(at) = pulses.iter().position(|(h, _)| h.same(pulse)) else {
            // SmeltingPulse: SkillPulse.stop().
            if let Some(result) = self.smelting_pulse_stopped(pulse) {
                return result.map_err(bind);
            }
            // SmithingPulse: SkillPulse.stop().
            if let Some(result) = self.smithing_pulse_stopped(pulse) {
                return result.map_err(bind);
            }
            // BarbFishingPulse: SkillPulse.stop().
            if let Some(result) = self.barbarian_pulse_stopped(pulse) {
                return result.map_err(bind);
            }
            // WoodcuttingSkillPulse.stop().
            return match self.woodcutting_pulse_stopped(pulse) {
                Some(result) => result.map_err(bind),
                None => Ok(()),
            };
        };
        let (_, mut task) = pulses.remove(at);
        task.stop(self)
    }

    /// The milking pulses of a player who left stop with it.
    pub fn milking_logout(&mut self, node: usize) {
        self.round.milking.pulses.retain(|(handle, task)| {
            let owned = task
                .checks
                .iter()
                .flatten()
                .any(|player| player.node() == node);
            if owned {
                pulses::stop_base(handle);
            }
            !owned
        });
    }
}

impl er::Host for Live {
    fn equipment_subclass(&self, _: &usize) -> Option<er::Subclass> {
        // No chinchompa or salamander handler instance is made here.
        None
    }
    fn set_salamander_style(&mut self, _: &usize, _: CombatStyle) {
        // No salamander handler instance exists (`equipment_subclass`).
    }
    fn equipment_remove(&mut self, _: &P, _: &Self::Item) -> Result<bool> {
        refuse("the equipment registers' equipment.remove (chinchompas, salamanders)")
    }
    fn equipment_remove_value(&mut self, _: &P, _: ItemValue) -> Result<bool> {
        refuse("the equipment registers' equipment.remove (chinchompas, salamanders)")
    }
    fn equipment_add(&mut self, _: &P, _: ItemValue, _: i32, _: bool, _: bool) -> Result<bool> {
        refuse("the equipment registers' equipment.add (HastaEquipListener)")
    }
    fn equipment_refresh(&mut self, _: &P) -> Result<()> {
        refuse("the equipment registers' equipment.refresh")
    }
    fn add_item_or_drop(&mut self, _: &P, _: i32, _: i32) -> Result<()> {
        refuse("the equipment registers' addItemOrDrop")
    }
    fn attribute_int(&mut self, _: &P, _: &str, _: i32) -> Result<i32> {
        refuse("the equipment registers' attributes (chinchompas)")
    }
    fn send_dialogue(&mut self, _: &P, _: &str) -> Result<()> {
        refuse("the equipment registers' sendDialogue (HastaEquipListener)")
    }
    fn on_equip(&mut self, _: &[i32]) -> Result<()> {
        refuse("HastaEquipListener.defineListeners")
    }
}

impl cr::Host for Live {
    fn potion(&self, _: i32) -> Option<cr::Potion> {
        // DecantListener is not installed (`on_use_with` refuses): nothing
        // asks.
        None
    }
    fn potion_ids(&self) -> Vec<i32> {
        Vec::new()
    }
    fn keg_ids(&self) -> Vec<i32> {
        Vec::new()
    }
    fn api_chat(&mut self, _: &P, _: &str) -> Result<()> {
        refuse("world::creatures' sendChat (sheep, the stray dog)")
    }
    fn api_animate(&mut self, _: &P, _: i32) -> Result<()> {
        refuse("world::creatures' animate (sheep, the stray dog)")
    }
    fn lock(&mut self, _: &P, _: i32) -> Result<()> {
        refuse("world::creatures' lock (sheep)")
    }
    fn unlock(&mut self, _: &P) -> Result<()> {
        refuse("world::creatures' unlock (sheep)")
    }
    fn force_walk(&mut self, _: &P, _: [i32; 3], _: &str) -> Result<()> {
        refuse("world::creatures' forceWalk (sheep)")
    }
    fn quest_started(&mut self, _: &P, _: &str) -> Result<bool> {
        refuse("world::creatures' quest stage (sheep)")
    }
    fn inventory_contains(&mut self, player: &P, id: i32, amount: i32) -> Result<bool> {
        // Container.contains(id, amount): getAmount(id) >= amount.
        let node = Node::Player(player.0.clone());
        let inventory = Ctx(self).container(&node, Held::Inventory).map_err(bind)?;
        Ok(inventory.amount_id(id, &self.inner.items) >= amount)
    }
    fn inventory_remove(&mut self, player: &P, item: ItemValue) -> Result<bool> {
        let node = Node::Player(player.0.clone());
        let item = self.native_item(item);
        Ctx(self)
            .with_container(&node, Held::Inventory, |c, defs, ctx| {
                c.remove(Some(&item), None, true, defs, ctx)
            })
            .map_err(bind)
    }
    fn inventory_add(&mut self, player: &P, item: ItemValue) -> Result<bool> {
        let node = Node::Player(player.0.clone());
        let item = self.native_item(item);
        Ctx(self)
            .with_container(&node, Held::Inventory, |c, defs, ctx| {
                c.add(Some(&item), true, -1, defs, ctx)
            })
            .map_err(bind)
    }
    fn free_slots(&mut self, _: &P) -> Result<i32> {
        refuse("world::creatures' freeSlots (sheep)")
    }
    fn has_option(&mut self, _: &P, _: &str) -> Result<bool> {
        refuse("world::creatures' hasOption (sheep)")
    }
    fn npc_transform(&mut self, _: &P, _: i32) -> Result<()> {
        refuse("world::creatures' NPC.transform (sheep)")
    }
    fn npc_re_transform(&mut self, _: &P) -> Result<()> {
        refuse("world::creatures' NPC.reTransform (sheep)")
    }
    fn api_add_item(&mut self, _: &P, _: i32, _: i32) -> Result<bool> {
        refuse("world::creatures' addItem (sheep, decanting)")
    }
    fn api_remove_item(&mut self, _: &P, _: ItemValue) -> Result<bool> {
        refuse("world::creatures' removeItem (decanting)")
    }
    fn send_graphics(&mut self, _: Graphics, _: [i32; 3]) -> Result<()> {
        refuse("world::creatures' sendGraphics (chinchompa deaths)")
    }
    fn submit_creature(&mut self, _: CreaturePulse<Self>) -> Result<()> {
        refuse("world::creatures' GameWorld.Pulser.submit (sheep)")
    }
    fn run_creature_pulse(&mut self, player: &P, mut pulse: CreaturePulse<Self>) -> Result<()> {
        // player.getPulseManager().run(pulse): STANDARD. The pulse object
        // here runs every tick and `CreaturePulse.update` keeps the
        // original's own delay, checks and ticks.
        let handle = Shared::new(pulses::Pulse::new(
            1,
            Some(Shared::new(Vec::new())),
            pulses::Class::Other,
            format!("{CLASS}$1"),
        ));
        let submitted = self
            .run_content_pulse(player, handle.clone(), |_| {
                pulse.start();
                pulse.running
            })
            .map_err(Error::Binding)?;
        if submitted {
            self.walk.stopping_pulses.push(handle.clone());
            self.round.milking.pulses.push((handle, pulse));
        }
        Ok(())
    }
    fn open_dialogue(&mut self, _: &P, id: i32, _: &[cr::DialogueArg]) -> Result<()> {
        refuse(&format!(
            "dialogue plugin {id} opened by CowMilkingPlugin (3807: no bucket; 70099: steal-cowbell)"
        ))
    }
    fn open_decanting_dialogue(&mut self, _: &P, _: &P) -> Result<()> {
        refuse("DecantingDialogue")
    }
    fn dialogue_npc(&mut self, _: &P, _: &str) -> Result<()> {
        refuse("DecantingDialogue")
    }
    fn dialogue_player(&mut self, _: &P, _: &str) -> Result<()> {
        refuse("DecantingDialogue")
    }
    fn decant_container(&mut self, _: &P) -> Result<(Vec<ItemValue>, Vec<ItemValue>)> {
        refuse("decantContainer")
    }
    fn decant_calquat(&mut self, _: &P) -> Result<(Vec<ItemValue>, Vec<ItemValue>)> {
        refuse("CalquatDecant.decantCalquat")
    }
    fn is_keg(&mut self, _: i32) -> Result<bool> {
        refuse("CalquatDecant.isKeg")
    }
    fn combine_kegs(&mut self, _: &P, _: &Self::Item, _: &Self::Item) -> Result<bool> {
        refuse("CalquatDecant.combineKegs")
    }
    fn pour_into_glass(&mut self, _: &P, _: &Self::Item, _: &Self::Item) -> Result<bool> {
        refuse("CalquatDecant.pourIntoGlass")
    }
    fn on_npc(&mut self, _: Option<&[i32]>, _: &[&str]) -> Result<()> {
        refuse("world::creatures' NPC listeners (sheep, decanting)")
    }
    fn on_use_with(&mut self, _: &[i32], _: &[i32]) -> Result<()> {
        refuse("DecantListener.defineListeners")
    }
    fn put_scenery_handler(&mut self, id: i32, key: &str) -> Result<()> {
        // SceneryDefinition.forId(id).getHandlers().put(key, this).
        let plugin = self.milking_plugin();
        let definition = self.walk.scene.definitions.for_id(id);
        let handlers = definition.read().handlers.clone();
        handlers.write().insert(
            key.to_owned(),
            crate::definitions::property::Property::OptionHandler(plugin),
        );
        Ok(())
    }
    fn scenery_option_handler(&mut self, name: &str) -> Result<()> {
        // SceneryDefinition.setOptionHandler(name, this).
        let plugin = self.milking_plugin();
        option_handler::set_option_handler(
            &self.walk.scenery.defaults(),
            Some(name.into()),
            Some(plugin),
        );
        Ok(())
    }
    fn define_bucket_plugin(&mut self) -> Result<()> {
        // new BucketHandler(): UseWithHandler(1925).
        self.round.milking.bucket = Some(Shared::new(HandlerData {
            name: "BucketHandler".into(),
            allowed_nodes: Some(vec![cow_milking::BUCKET]),
            dynamic: false,
            handler: BUCKET_HANDLER,
        }));
        Ok(())
    }
    fn add_use_with_handler(&mut self, id: i32, kind: i32) -> Result<()> {
        debug_assert_eq!(kind, OBJECT_TYPE);
        let bucket = self.round.milking.bucket.clone().ok_or(Error::Null)?;
        self.item_actions
            .use_handlers
            .add_handler(id, kind, &bucket);
        Ok(())
    }
}
