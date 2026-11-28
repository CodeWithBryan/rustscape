//! Bank booths, bank chests and deposit boxes in the live world: the global
//! handlers' `BankBoothListener`, `BankChestListener` and
//! `BankDepositBoxListener` (`world::global_handlers::scenery`), registered
//! with `defineInteraction` (their metadata in `InteractionListeners`).
//!
//! A scenery option reaches `InteractionListeners.run` through the live
//! world's scenery listener run (super::scenery, super::items): for
//! metadata, `Script(handler, distance, persist)` is installed with
//! `ScriptProcessor.setInteractionScript` on the player's canonical
//! processor, then the empty `MovementPulse(player, node, null)` runs. The
//! processor's approach/operable checks run in the player's script phases
//! with super::scripts' `ScriptHost`, which executes the handler here
//! (`Live::bank_script`) inside the phase, as the original invokes the
//! lambda.
use super::{
    global::{GHost, GNode},
    host::Live,
};
use crate::world::{
    doors,
    global_handlers::registry::{self, Interaction},
    player_walk::NodeHandle,
    script_runtime,
};

impl Live {
    /// A `defineInteraction` script's handler, executed by the player's
    /// script processor inside its phase (super::scripts' `content_script`):
    /// the global handler's booth, chest or deposit box body with the whole
    /// world. Its answer is the executor's; a failure goes back to
    /// `ScriptProcessor.executeScript`, which logs it and resets.
    pub fn bank_script(
        &mut self,
        p: &super::P,
        interaction: Interaction,
        target: &NodeHandle,
        // The booth, chest and deposit box bodies do not read the stage.
        _stage: i32,
    ) -> crate::scripts::Result<bool> {
        let object = self
            .inner
            .state
            .scenery(target.node)
            .map_err(script_runtime::error)?;
        registry::run_interaction(
            &mut GHost { live: self },
            interaction,
            &GNode::Entity(p.clone()),
            Some(&GNode::Scenery(object)),
            None,
        )
        .map_err(|e| crate::scripts::Error::Binding(format!("{e:?}")))
    }

    /// Whether a handler is one of the bank scenery bodies this world runs.
    pub fn bank_interaction(
        f: &crate::world::interaction_listeners::Function,
    ) -> Option<Interaction> {
        f.value::<Interaction>().map(|i| *i).filter(|i| {
            matches!(
                i,
                Interaction::BoothQuick
                    | Interaction::BoothRegular
                    | Interaction::BoothCollect
                    | Interaction::Chest
                    | Interaction::DepositBox
            )
        })
    }
}

impl Live {
    /// `player.ironmanManager.checkRestriction(mode)`: the canonical
    /// manager's check, its message through the bank's interface context.
    pub fn ironman_check(
        &mut self,
        p: &super::P,
        mode: crate::world::global_handlers::IronmanMode,
    ) -> crate::player::ironman::Result<bool> {
        use crate::{player::ironman::Mode, world::global_handlers::IronmanMode as M};
        let minimum = match mode {
            M::None => Mode::None,
            M::Standard => Mode::Standard,
            M::Hardcore => Mode::Hardcore,
            M::Ultimate => Mode::Ultimate,
        };
        let manager = self.inner.canonical(p).player.ironman();
        let restricted = manager.check(
            &mut super::bank::BankUi::new(&mut self.inner),
            Some(minimum),
        );
        self.inner.drain(p);
        restricted
    }
}

// ------------------------------------------------------------ the deposit box

/// `InterfaceContainer.generateItems(player, items, options, iface, child,
/// x, y)`: `world::grand_exchange::sets_interface::generate_items` over the
/// live frames (super::interface_container), with the world's
/// `InterfaceContainer.index`.
#[allow(clippy::too_many_arguments)]
pub fn generate_items(
    inner: &mut super::inner::Inner,
    p: &super::P,
    items: &[Option<crate::inventory::item::Handle>],
    options: &[&str],
    iface: i32,
    child: i32,
    x: i32,
    y: i32,
) -> Result<i32, String> {
    use rs09_protocol::game::updates::WireItem;
    let wire: Vec<Option<WireItem>> = items
        .iter()
        .map(|item| {
            item.as_ref().map(|item| {
                let item = item.read();
                WireItem {
                    id: item.id(),
                    amount: item.amount(),
                }
            })
        })
        .collect();
    let exchange = inner.interface_container.exchange.clone();
    let result = crate::world::grand_exchange::sets_interface::generate_items(
        &mut super::interface_container::Frames { inner, exchange },
        &p.0,
        &wire,
        options,
        iface,
        child,
        x,
        y,
    );
    inner.drain(p);
    result.map_err(|e| format!("{e:?}"))
}

/// BankDepositBoxListener's stateless close event
/// (`CloseEvent { p, _ -> p.interfaceManager.openDefaultTabs(); true }`).
pub struct DepositBoxClose;
impl crate::world::component_objects::NativeObject for DepositBoxClose {}

impl super::inner::Inner {
    /// `CloseEvent.close` of BankDepositBoxListener's event; `None` for
    /// another kind. The player is the closing player.
    pub fn deposit_box_close_event(
        &mut self,
        event: &crate::world::interfaces::CloseEvent,
    ) -> Option<crate::world::interfaces::Result<bool>> {
        event.value::<DepositBoxClose>()?;
        Some((|| {
            let p = self
                .current
                .clone()
                .ok_or(crate::world::interfaces::Error::Null)?;
            let manager = self.canonical(&p).player.manager();
            crate::world::interfaces::open_default_tabs(self, &manager)?;
            Ok(true)
        })())
    }
}

/// `BankUtils.transferX`'s `after` of the deposit box's Deposit-X
/// (`{ player.bank.refreshDepositBoxInterface() }`), capturing the player.
pub struct DepositRefresh(pub NodeHandle);
impl crate::world::component_objects::NativeObject for DepositRefresh {
    fn captures(&self) -> crate::world::component_objects::Captures {
        crate::world::component_objects::Captures {
            nodes: vec![self.0.clone()],
            ..Default::default()
        }
    }
}

impl Live {
    fn ui_call<T>(
        &mut self,
        p: &super::P,
        f: impl FnOnce(
            &mut super::bank::BankUi<'_>,
            crate::world::interfaces::Manager,
        ) -> crate::inventory::Result<T>,
    ) -> Result<T, String> {
        let manager = self.inner.canonical(p).player.manager();
        let result = f(&mut super::bank::BankUi::new(&mut self.inner), manager);
        self.inner.drain(p);
        result.map_err(|e| format!("{e:?}"))
    }
    /// `interfaceManager.open(Component(id))`: false for a null answer.
    pub fn deposit_open(&mut self, p: &super::P, id: i32) -> Result<bool, String> {
        use crate::inventory::bank::Context as _;
        let opened = self.ui_call(p, |ui, manager| {
            let component = ui.component(id)?;
            Ok(ui.open(manager, component)?.is_some())
        });
        // InterfaceListeners.runOpen inside open: BankDepositBoxInterface's
        // onOpen(11) (the title), with the global handlers' host, before
        // open returns to the listener body.
        if let Some((player, component)) = self.inner.interface_container.pending_open.take() {
            self.global_open(
                crate::world::global_handlers::registry::Listener::DepositBoxOpen,
                &player.0,
                component,
            )
            .map_err(|e| format!("BankDepositBoxInterface onOpen: {e:?}"))?;
            self.inner.drain(&player);
        }
        opened
    }
    /// `component.closeEvent = event` on the component just opened: only
    /// BankDepositBoxListener's event has a body here.
    pub fn deposit_close_event(&mut self, p: &super::P, id: i32) -> Result<(), String> {
        let manager = self.inner.canonical(p).player.manager();
        let opened = manager.fields.read().opened.clone();
        let component = opened
            .filter(|c| c.read().id == id)
            .ok_or_else(|| format!("no opened component {id} for its close event"))?;
        component.write().close_event =
            Some(crate::world::component_objects::Close::new(DepositBoxClose));
        Ok(())
    }
    /// `interfaceManager.removeTabs(tabs...)`.
    pub fn deposit_remove_tabs(&mut self, p: &super::P, tabs: &[i32]) -> Result<(), String> {
        use crate::inventory::bank::Context as _;
        self.ui_call(p, |ui, manager| ui.remove_tabs(manager, tabs))
    }
    /// `interfaceManager.openDefaultTabs()`.
    pub fn deposit_default_tabs(&mut self, p: &super::P) -> Result<(), String> {
        use crate::inventory::bank::Context as _;
        self.ui_call(p, |ui, manager| ui.open_default_tabs(manager))
    }
    /// `player.bank.addItem(slot, amount)`.
    pub fn deposit_add(&mut self, p: &super::P, slot: i32, amount: i32) -> Result<(), String> {
        let bank = self.inner.canonical(p).player.bank();
        self.bank_deposit(p, &bank, slot, amount)
            .map_err(|e| format!("{e:?}"))
    }
    /// `player.bank.refreshDepositBoxInterface()`.
    pub fn deposit_refresh(&mut self, p: &super::P) -> Result<(), String> {
        let bank = self.inner.canonical(p).player.bank();
        self.ui_call(p, |ui, _| bank.refresh_deposit_box(ui))
    }
    /// `item.definition.examine` of the item's cached definition.
    pub fn item_examine_of(
        &mut self,
        item: &crate::inventory::item::Handle,
    ) -> Result<Option<String>, String> {
        let definition = item
            .read()
            .definition
            .clone()
            .ok_or("an item without a definition")?;
        crate::world::packet_handlers::examine::item_examine_text(&definition, |s| {
            rs09_game_core::java_lower::lowercase_root(s)
        })
        .map(Some)
        .map_err(|e| format!("{e:?}"))
    }
}

impl Live {
    /// `Repository.findNPC(l)`: the first NPC of the location's region
    /// chunk list (in list order) whose location equals it. A null entry
    /// fails as Kotlin's `n.location` does (NullPointerException).
    pub fn find_npc_at(&mut self, at: [i32; 3]) -> Result<Option<NodeHandle>, String> {
        let location = doors::Location::new(at[0], at[1], at[2]);
        // RegionManager.getRegionChunk(l): the region (with its
        // configuration) first, then its chunk.
        let chunk = crate::world::entity_regions::Context::chunk(self, location)
            .map_err(|e| format!("{e:?}"))?;
        let npcs = chunk.read().npcs.values();
        for npc in npcs {
            let npc = npc.ok_or("NullPointerException: a null NPC in the region chunk's list")?;
            let here = self
                .inner
                .state
                .node_location(npc.node)
                .map_err(|e| format!("{e:?}"))?;
            if here.is_some_and(|l| l.coordinates() == at) {
                return Ok(Some(npc));
            }
        }
        Ok(None)
    }
}
