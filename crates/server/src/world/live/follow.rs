//! The player options in the live world: `Option._P_ATTACK` and
//! `Option._P_FOLLOW` bound to the global handlers' AttackOptionPlugin and
//! FollowOptionPlugin (`setPlayerOption`), their dispatch from the player
//! option's InteractPlugin through the legacy runtime (the walk to the
//! player, the arrival), and FollowOptionPlugin's
//! `MovementPulse(player, target, FOLLOW_ENTITY)`, a movement of the
//! canonical state whose body (`global_handlers::player::follow_pulse`:
//! face the target, never finish) runs with the whole world right after the
//! walking call that reached it. "Trade with" and "Req Assist" are the
//! global handlers' RequestOption listeners (super::trading).
use super::{
    P,
    global::{Absent, GHost, GNode},
    host::Live,
};
use crate::{
    pulses,
    world::{
        global_handlers::{self as gh, Task, registry::OptionPlugin},
        interaction::{Approach, ApproachKind, OptionData, OptionHandle},
        movement::{self, Movement},
        player_walk::NodeHandle,
    },
};

/// The follow state of the live world.
#[derive(Default)]
pub struct FollowWorld {
    /// The movement pulses the handlers made (`movement_pulse`), by their
    /// base pulse, with their body's captures.
    pub movements: Vec<(
        pulses::Handle,
        Movement,
        OptionHandle,
        NodeHandle,
        NodeHandle,
    )>,
}

/// A player option handler reached by the legacy arrival
/// (`OptionHandler.handle(player, node, option)`), run with the whole world.
#[derive(Clone)]
pub struct PlayerHandlerCall {
    pub plugin: OptionPlugin,
    pub player: NodeHandle,
    pub target: NodeHandle,
    pub option: String,
}

impl Live {
    /// `Option._P_ATTACK.setHandler` and `_P_FOLLOW.setHandler`: the handler
    /// ids are positions in the walk state's player handler arena.
    pub fn bind_player_options(
        &mut self,
        options: &[(String, crate::definitions::option_handler::Handle)],
    ) {
        for (name, handler) in options {
            let Some(plugin) = handler.read().value::<OptionPlugin>().copied() else {
                continue;
            };
            let id = self.walk.player_handlers.len();
            self.walk.player_handlers.push(plugin);
            let constant = match name.as_str() {
                "Attack" => &self.inner.options.attack,
                "Follow" => &self.inner.options.follow,
                _ => continue,
            };
            constant.write().handler = Some(id);
        }
    }

    /// A player option handler's `handle(player, node, option)`.
    pub fn run_player_handler(&mut self, call: PlayerHandlerCall) {
        let p = P(call.player.clone());
        if !self.inner.players.contains_key(&p.node())
            || !self.inner.players.contains_key(&call.target.node)
        {
            return;
        }
        let player = GNode::Entity(p.clone());
        let target = GNode::Entity(P(call.target.clone()));
        let result = gh::registry::run_handler(
            &mut GHost { live: self },
            call.plugin,
            &player,
            Some(&target),
            Some(&call.option),
        );
        match result {
            Ok(false) => {
                let _ = self
                    .inner
                    .message(&p, crate::world::legacy_interaction::NOTHING);
            }
            Ok(true) => {}
            Err(e) => self
                .inner
                .log_line(format!("{} failed: {e:?}", call.plugin.class())),
        }
        self.inner.drain(&p);
    }

    /// `object : MovementPulse(player, target, DestinationFlag.FOLLOW_ENTITY)`
    /// of FollowOptionPlugin: constructed, not yet run.
    pub fn follow_movement(
        &mut self,
        mover: &P,
        target: &P,
        body: &Task<GNode, Absent>,
    ) -> gh::Result<pulses::Handle> {
        let Task::Follow { .. } = body else {
            return Err(gh::Error::Binding(Some(
                "not ported in the live world: a movement pulse body other than FollowOptionPlugin's".into(),
            )));
        };
        let option = crate::definitions::item::Shared::new(OptionData::new(None, -1));
        let approach = Approach {
            player: mover.node(),
            target: Some(target.node()),
            option: Some(option.clone()),
            kind: ApproachKind::Option,
        };
        let motion = self
            .with_walk(|world| {
                world.listener_motion(approach, Some(movement::Flag::FollowEntity), None)
            })
            .map_err(|e| gh::Error::Binding(Some(format!("{e:?}"))))?;
        let base = motion.base();
        // A player follows one target: its earlier follow pulse's record goes.
        self.round
            .follow
            .movements
            .retain(|(_, _, _, player, _)| player.node != mover.node());
        self.walk.follows.retain(|(_, node)| *node != mover.node());
        self.walk.follows.push((option.clone(), mover.node()));
        self.round.follow.movements.push((
            base.clone(),
            motion,
            option,
            mover.0.clone(),
            target.0.clone(),
        ));
        Ok(base)
    }
    /// `player.pulseManager.run(pulse, STANDARD)` of a follow pulse.
    pub fn run_follow(&mut self, pulse: &pulses::Handle) -> gh::Result<()> {
        let Some((_, motion, _, player, _)) = self
            .round
            .follow
            .movements
            .iter()
            .find(|(base, _, _, _, _)| base.same(pulse))
            .cloned()
        else {
            return Err(gh::Error::Binding(Some(
                "a pulse this world's handlers did not make".into(),
            )));
        };
        self.with_walk(|world| world.run_motion(player.node, motion))
            .map_err(|e| gh::Error::Binding(Some(format!("{e:?}"))))
    }
    /// The follow pulse's `pulse()` (reached when the mover stands at the
    /// follow tile): `player.face(target)`, never finished.
    pub fn follow_arrival(&mut self, option: &OptionHandle) {
        let Some((_, _, _, player, target)) = self
            .round
            .follow
            .movements
            .iter()
            .find(|(_, _, o, _, _)| o.same(option))
            .cloned()
        else {
            return;
        };
        if !self.inner.players.contains_key(&player.node)
            || !self.inner.players.contains_key(&target.node)
        {
            return;
        }
        let (p, t) = (GNode::Entity(P(player.clone())), GNode::Entity(P(target)));
        if let Err(e) = gh::player::follow_pulse(&mut GHost { live: self }, &p, Some(&t)) {
            self.inner
                .log_line(format!("FollowOptionPlugin's pulse failed: {e:?}"));
        }
        self.inner.drain(&P(player));
    }
    /// `entity.face(target)` of a player.
    pub fn face_entity(&mut self, player: &P, target: Option<&P>) -> Result<(), String> {
        use crate::world::prepared_masks::FaceTarget;
        let masks = self.inner.canonical(player).player.masks();
        match target {
            None => masks
                .clear_face_entity()
                .map(|_| ())
                .map_err(|e| format!("{e:?}")),
            Some(target) => {
                let face = match self.inner.players.get(&target.node()) {
                    Some(slot) => {
                        FaceTarget::Player(target.0.clone(), slot.canonical.render.clone())
                    }
                    None => {
                        let view = self
                            .npc_views
                            .get(&target.node())
                            .cloned()
                            .ok_or("facing an NPC without an NPC render view")?;
                        FaceTarget::Npc(target.0.clone(), view)
                    }
                };
                masks
                    .face_entity(Some(face))
                    .map(|_| ())
                    .map_err(|e| format!("{e:?}"))
            }
        }
    }
}
