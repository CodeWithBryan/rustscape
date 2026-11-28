//! Mining in the live world: `MiningListener`, the rocks' "Mine" option as
//! an interaction script of the player's ScriptProcessor
//! (super::super::scripts), and `ProspectListener`, the "Prospect" option
//! with its three-tick message pulse. Both bodies are
//! `world::gathering::mining`'s, verified by the gathering corpus
//! (docs/gathering.md); their `service::Context` over the live world is
//! super::woodcutting's `Gather`, written without a corpus of its own.
//!
//! Answered through the native systems, beyond what woodcutting already
//! asks: the rock's location, type and rotation and a new `Scenery` for
//! the depleted rock, the animation clock and `animate`, `delayScript` and
//! `scripts.reset()`, the worn item of a slot, `inEquipment`,
//! `removeItem` from the equipment (the bracelet of clay), `addItemOrDrop`,
//! ContentAPI `sendDialogue`, `inBorders`, the player's StarBonus timer,
//! the Varrock diary's level and armour, `Math.random()`,
//! `Collections.shuffle`'s generator and `ThreadLocalRandom.nextInt`.
//! `MiningNode.gemRockGems`, a static the gem rock's roll shuffles in
//! place, is this world's ([`MiningWorld`]).
//!
//! Not ported, and answered by name (docs/live-world.md): the familiar's
//! boost (0: no familiar), SkillcapePerks (inactive) and the
//! ResourceProducedEvent (skipped). The legacy MiningSkillPulse has no
//! caller in the pinned source's rock options and is not installed.
use super::{
    super::{host::Live, scripts::Interaction},
    Node, P,
    woodcutting::{Bound, NODE, PLAYER, bind},
};
use crate::{
    definitions::item::Shared,
    pulses, scripts,
    world::{
        doors,
        gathering::{Settings, mining, nodes::MiningNode, service::Task},
        interaction_listeners::{self as listeners, Function, FunctionKind},
        player_walk::NodeHandle,
    },
};

/// The listeners' classes (`scan.contentInterfaces`).
pub const LISTENER_CLASS: &str = "content.global.skill.gather.mining.MiningListener";
pub const PROSPECT_CLASS: &str = "content.global.skill.gather.mining.ProspectListener";
/// ProspectListener's `on` lambda.
const PROSPECT_LAMBDA: &str =
    "content.global.skill.gather.mining.ProspectListener$defineListeners$1";

/// ProspectListener's `on(IntType.SCENERY, "prospect")` lambda.
pub struct Prospect;
impl crate::world::component_objects::NativeObject for Prospect {}

/// Mining's state in this world.
#[derive(Default)]
pub struct MiningWorld {
    /// `MiningNode.gemRockGems`.
    pub gems: mining::State,
    /// The running prospect pulses: the pulse object, the player's node,
    /// the rock and the message.
    pub prospects: Vec<(pulses::Handle, usize, doors::Handle, Task)>,
}

fn settings() -> Settings {
    Settings {
        max_players: crate::world::repository::MAX_PLAYERS,
    }
}

impl Live {
    /// `MiningListener.defineListeners`: every rock's "mine" as one SCENERY
    /// interaction (persistent, distance 1) whose handler is
    /// [`Interaction::Mining`].
    pub fn define_mining_listener(&mut self) {
        let ids: Vec<i32> = MiningNode::values().iter().map(|n| n.full).collect();
        self.item_actions.listeners.add_metadata(
            &super::super::global::RootCasing,
            &ids,
            listeners::Kind::Scenery,
            &["mine".encode_utf16().collect()],
            Shared::new(listeners::Metadata {
                function: Interaction::Mining.function(),
                distance: 1,
                persist: true,
            }),
        );
    }

    /// `ProspectListener.defineListeners`: `on(IntType.SCENERY, "prospect")`.
    pub fn define_prospect_listener(&mut self) -> std::result::Result<(), String> {
        let option: Vec<u16> = "prospect".encode_utf16().collect();
        self.item_actions
            .listeners
            .add_generic(
                &super::super::global::RootCasing,
                &option,
                listeners::Kind::Scenery.ordinal(),
                Function::new(FunctionKind::On, PROSPECT_LAMBDA, Prospect),
            )
            .map_err(|e| format!("{PROSPECT_CLASS}: {e:?}"))
    }

    /// `handleMining(player, node, state)`: one execution of the
    /// interaction script, with the processor's target as the rock.
    pub fn mining_script(
        &mut self,
        p: &P,
        target: &NodeHandle,
        stage: i32,
    ) -> scripts::Result<bool> {
        let rock = self
            .inner
            .state
            .scenery(target.node)
            .map_err(crate::world::script_runtime::error)?;
        let mut gems = std::mem::take(&mut self.round.mining.gems);
        let done = self.gather(p, &rock, "mining", |host| {
            mining::handle_mining(host, &settings(), &mut gems, PLAYER, NODE, stage)
        });
        self.round.mining.gems = gems;
        done.map_err(crate::world::script_runtime::error)
    }

    /// ProspectListener's lambda.
    pub fn prospect_option(&mut self, player: &Node, rock: &Node) -> Bound<bool> {
        let (Node::Player(player), Node::Scenery(rock)) = (player, rock) else {
            return Err(bind("the prospect option of a node that is not scenery"));
        };
        self.gather(&P(player.clone()), rock, "mining", |host| {
            mining::prospect(host, PLAYER, NODE)
        })
    }

    /// `player.pulseManager.run(object : Pulse(delay) { ... })` of the
    /// prospect message.
    pub(super) fn run_prospect_pulse(
        &mut self,
        p: &P,
        rock: &doors::Handle,
        delay: i32,
        task: Task,
    ) -> std::result::Result<(), String> {
        // The pulses a manager stopped never reach their body.
        self.round
            .mining
            .prospects
            .retain(|(pulse, ..)| pulse.read().running);
        let handle = Shared::new(pulses::Pulse::new(
            delay,
            None,
            pulses::Class::Other,
            format!("{PROSPECT_LAMBDA}$1"),
        ));
        if self.run_standard_pulse(p, handle.clone())? {
            self.round
                .mining
                .prospects
                .push((handle, p.node(), rock.clone(), task));
        }
        Ok(())
    }

    /// A prospect pulse by identity: `Some` with whether the runner drops
    /// it (the body always finishes).
    pub fn prospect_pulse_body(&mut self, pulse: &pulses::Handle) -> Option<Bound<bool>> {
        let pulses = &mut self.round.mining.prospects;
        let at = pulses.iter().position(|(h, ..)| h.same(pulse))?;
        let (_, owner, rock, task) = pulses.remove(at);
        if !self.inner.players.contains_key(&owner) {
            // The player left: the pulse ends.
            return Some(Ok(true));
        }
        let p = match self.inner.state.retain_node(owner) {
            Ok(h) => P(h),
            Err(e) => return Some(Err(bind(e))),
        };
        Some(self.gather(&p, &rock, "mining", |host| mining::run_task(host, &task)))
    }
}
