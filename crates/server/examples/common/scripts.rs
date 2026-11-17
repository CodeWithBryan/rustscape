//! Script phases use actual canonical actor state. Remaining content and
//! dialogue services fail explicitly until their real bindings are installed.
use super::{Host, Location, Node, State};
use rs09_server::{
    scripts::{Arguments, Error, Level, Result},
    world::{
        interaction_listeners::{Function, FunctionKind},
        interfaces, legacy_interaction, pathfinding,
        player_walk::{NodeHandle, Scripts},
        script_runtime::{self, Bindings, Native, error},
    },
};

struct ActorFootprint<'a>(&'a State, Node);
impl pathfinding::projectile::Footprint for ActorFootprint<'_> {
    fn size(&self) -> pathfinding::Result<i32> {
        self.0
            .node_size(self.1)
            .map_err(|e| pathfinding::Error::Binding(e.to_string()))
    }

    fn location(&self) -> pathfinding::Result<Option<Location>> {
        self.0
            .node_location(self.1)
            .map_err(|e| pathfinding::Error::Binding(e.to_string()))
    }
}

fn manager(state: &State, owner: &NodeHandle) -> Result<Option<interfaces::Manager>> {
    match state.player_fields(owner) {
        Ok(player) => Ok(Some(player.manager())),
        Err(rs09_server::world::player_walk::Error::Null) => Ok(None),
        Err(e) => Err(error(e)),
    }
}
impl Bindings for Host {
    fn modal_open(&mut self, state: &mut State, owner: &NodeHandle) -> Result<bool> {
        let Some(manager) = manager(state, owner)? else {
            return Ok(false);
        };
        // Player.hasModalOpen excludes plain messages, scrolling plain messages
        // and the default chatbox; postMovement has different interface gates.
        Ok(script_runtime::has_modal_open(&manager))
    }
    fn interface_open(&mut self, state: &mut State, owner: &NodeHandle) -> Result<bool> {
        Ok(manager(state, owner)?.is_some_and(|m| interfaces::is_opened(&m)))
    }
    fn chatbox_open(&mut self, state: &mut State, owner: &NodeHandle) -> Result<bool> {
        Ok(manager(state, owner)?.is_some_and(|m| interfaces::has_chatbox(&m)))
    }
    fn close_interfaces(&mut self, _: &mut State, _: &NodeHandle) -> Result<()> {
        Err(Error::Binding(
            "script closeAllInterfaces requires UI and dialogue host joins".into(),
        ))
    }
    fn reset_animator(&mut self, state: &mut State, owner: &NodeHandle) -> Result<()> {
        let animator = self.animators.get(&owner.node).ok_or(Error::Null)?.clone();
        let actual = state.clocks(owner).map_err(error)?;
        if !animator.owner().is_some_and(|o| o.clocks().same(&actual)) {
            return Err(Error::Binding(
                "script animator has different entity clocks".into(),
            ));
        }
        self.animations.ticks = state.ticks;
        animator.reset(&mut self.animations).map_err(error)
    }
    fn node_description(&mut self, _: &mut State, _: Option<NodeHandle>) -> Result<String> {
        Err(Error::Binding(
            "original node diagnostic formatter is unbound".into(),
        ))
    }
    fn face_location(
        &mut self,
        state: &mut State,
        target: NodeHandle,
        from: Option<Location>,
    ) -> Result<Option<Location>> {
        let location = state
            .node_location(target.node)
            .map_err(error)?
            .ok_or(Error::Null)?;
        let size = state.node_size(target.node).map_err(error)?;
        Ok(Some(legacy_interaction::node_face_location(
            &location,
            size,
            &from.ok_or(Error::Null)?,
        )))
    }
    fn center_location(
        &mut self,
        state: &mut State,
        target: NodeHandle,
    ) -> Result<Option<Location>> {
        Ok(Some(script_runtime::center_location(state, &target)?))
    }
    fn line_of_sight(
        &mut self,
        state: &mut State,
        owner: &NodeHandle,
        destination: &Location,
    ) -> Result<bool> {
        state.clocks(owner).map_err(error)?;
        pathfinding::projectile::can_reach(
            &mut self.scene.context(&mut self.maps),
            &ActorFootprint(state, owner.node),
            Some(destination),
            false,
        )
        .map_err(error)
    }
    fn face(&mut self, _: &mut State, owner: &NodeHandle, location: Location) -> Result<()> {
        self.masks
            .get(&owner.node)
            .ok_or(Error::Null)?
            .face_location(Some(location))
            .map_err(error)?;
        Ok(())
    }
    fn execution_equals(
        &mut self,
        _: &mut State,
        left: Option<Function>,
        right: Option<Function>,
    ) -> Result<bool> {
        Ok(match (left, right) {
            (Some(a), Some(b)) => a.same(&b),
            (None, None) => true,
            _ => false,
        })
    }
    fn invoke(
        &mut self,
        state: &mut State,
        owner: &NodeHandle,
        execution: Option<Function>,
        arguments: Arguments<NodeHandle>,
    ) -> Result<bool> {
        let function = execution.ok_or(Error::Null)?;
        if !matches!(
            (&arguments, function.kind()),
            (Arguments::Void { .. }, FunctionKind::Void)
                | (Arguments::Interaction { .. }, FunctionKind::Interaction)
                | (Arguments::UseWith { .. }, FunctionKind::UseInteraction)
        ) {
            return Err(Error::Binding(
                "native script function arity mismatch".into(),
            ));
        }
        let body = function
            .value::<Native<Host>>()
            .ok_or_else(|| Error::Binding("native script function type is unbound".into()))?;
        body.invoke(state, self, owner, arguments)
    }
    fn exception_text(&mut self, _: &mut State, error: &Error) -> Result<String> {
        Ok(format!("{error:?}"))
    }
    fn log(&mut self, _: &mut State, owner: &str, level: Level, text: String) -> Result<()> {
        if level == Level::Error {
            self.errors.push(format!("{owner}: {text}"));
        }
        Ok(())
    }
    fn lock_entity(&mut self, state: &mut State, owner: &NodeHandle, ticks: i32) -> Result<()> {
        state
            .locks(owner)
            .map_err(error)?
            .lock(ticks, state, &mut rs09_server::world::action_locks::Clock)
            .map_err(error)
    }
}
impl rs09_server::world::action_locks::Bindings for Host {
    fn elapse(
        &mut self,
        state: &mut State,
        event: Option<rs09_server::world::component_objects::Plugin>,
        node: Option<NodeHandle>,
        lock: &rs09_server::world::action_locks::Lock,
    ) -> rs09_server::world::action_locks::Result<()> {
        use rs09_server::world::action_locks::{Error as LockError, Native as LockBody};
        let event = event.ok_or(LockError::Null)?;
        let body = event
            .value::<LockBody<Host>>()
            .ok_or_else(|| LockError::Binding("actual LockElapse body is unbound".into()))?;
        body.invoke(state, self, node, lock.clone())
    }
}
pub(super) fn locked(
    state: &mut State,
    node: Node,
    kind: rs09_server::world::action_locks::Kind,
) -> rs09_server::world::action_locks::Result<bool> {
    use rs09_server::world::action_locks::{Clock, Error};
    let owner = state
        .retain_node(node)
        .map_err(|e| Error::Binding(e.to_string()))?;
    state
        .locks(&owner)
        .map_err(|e| Error::Binding(e.to_string()))?
        .is_kind_locked(kind, state, &mut Clock)
}
pub(super) fn lock(
    state: &mut State,
    node: Node,
    kind: rs09_server::world::action_locks::Kind,
    ticks: i32,
) -> rs09_server::world::action_locks::Result<()> {
    use rs09_server::world::action_locks::{Clock, Error};
    let owner = state
        .retain_node(node)
        .map_err(|e| Error::Binding(e.to_string()))?;
    state
        .locks(&owner)
        .map_err(|e| Error::Binding(e.to_string()))?
        .lock_kind(kind, ticks, state, &mut Clock)
}
pub(super) fn unlock(
    state: &mut State,
    node: Node,
    kind: rs09_server::world::action_locks::Kind,
) -> rs09_server::world::action_locks::Result<()> {
    use rs09_server::world::action_locks::Error;
    let owner = state
        .retain_node(node)
        .map_err(|e| Error::Binding(e.to_string()))?;
    state
        .locks(&owner)
        .map_err(|e| Error::Binding(e.to_string()))?
        .unlock_kind(kind)
}
pub(super) fn phase(state: &mut State, host: &mut Host, node: Node, phase: Scripts) -> Result<()> {
    let owner = state.retain_node(node).map_err(error)?;
    script_runtime::phase(state, host, &owner, phase)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rs09_protocol::game::Frame;
    use rs09_server::{
        scripts::{Context as _, Form, Runtime, Strength},
        world::{
            component_objects::{Captures, NativeObject, Plugin},
            player_walk::{Event, Services},
        },
    };
    fn callback(
        body: &Native<Host>,
        state: &mut State,
        host: &mut Host,
        owner: &NodeHandle,
        args: Arguments<NodeHandle>,
    ) -> Result<bool> {
        let Arguments::Void { stage } = args else {
            return Err(Error::Null);
        };
        // Nest another actual entity phase while this entity's callback is live.
        let other = &body.fields().nodes[0];
        script_runtime::phase(state, host, other, Scripts::Post(true))?;
        host.send(
            owner.node,
            Frame {
                opcode: 70,
                payload: format!("native script stage {stage}\0").into_bytes(),
            },
        );
        if stage == 0 {
            let processor = state.scripts(owner).map_err(error)?;
            let mut ctx = script_runtime::Context::new(state, host, &processor)?;
            processor.delay_script(2, state, &mut ctx)?;
            Ok(false)
        } else {
            Ok(true)
        }
    }

    fn ranged_callback(
        _: &Native<Host>,
        _: &mut State,
        host: &mut Host,
        owner: &NodeHandle,
        args: Arguments<NodeHandle>,
    ) -> Result<bool> {
        let Arguments::Interaction { stage: 0, .. } = args else {
            return Err(Error::Binding("unexpected interaction args".into()));
        };
        host.send(
            owner.node,
            Frame {
                opcode: 70,
                payload: b"target visible\0".to_vec(),
            },
        );
        Ok(true)
    }

    #[test]
    fn script_approach_waits_for_canonical_projectile_and_walking_flags() {
        let (mut p, node, _) = super::super::tests::probe();
        let owner = p.state.retain_node(node).unwrap();
        let destination = Location::new(3209, 3216, 0);
        let target = p.state.add(2, destination.clone(), [1, 2, 3, 4]);
        p.state.actor_mut(target).unwrap().view.active = true;
        let target = p.state.retain_node(target).unwrap();
        let blocked = Location::new(3208, 3216, 0);
        let tile = p.host.scene.chunk(&blocked).unwrap();
        let (walking, projectile) = {
            let tile = tile.read();
            (tile.flags.clone(), tile.projectile_flags.clone())
        };
        {
            let mut world = p.host.scene.context(&mut p.host.maps);
            world.load((50 << 8) | 50, None).unwrap().write().has_flags = true;
            walking.clear(&mut world, 0, 0).unwrap();
            walking.clear(&mut world, 1, 0).unwrap();
            projectile.clear(&mut world, 1, 0).unwrap();
            projectile.add_flag(&mut world, 0, 0, 0x80).unwrap();
        }
        p.state.ticks = 10;
        let processor = p.state.scripts(&owner).unwrap();
        let function = Function::new(
            FunctionKind::Interaction,
            "native.test.ranged",
            Native::<Host>::new(Captures::default(), ranged_callback),
        );
        let script = processor
            .create_script(Form::Interaction { distance: 2 }, Some(function), false)
            .unwrap();
        let mut ctx = script_runtime::Context::new(&p.state, &mut p.host, &processor).unwrap();
        processor
            .interact_in(&mut p.state, &mut ctx, Some(target), Some(&script))
            .unwrap();
        phase(&mut p.state, &mut p.host, node, Scripts::Pre).unwrap();
        assert_eq!(script.read().state, 0);

        {
            let mut world = p.host.scene.context(&mut p.host.maps);
            projectile.clear(&mut world, 0, 0).unwrap();
            walking.add_flag(&mut world, 0, 0, 0x20000).unwrap();
        }
        phase(&mut p.state, &mut p.host, node, Scripts::Pre).unwrap();
        assert_eq!(script.read().state, 0);

        // Ordinary walking bits do not block projectile line of sight.
        {
            let mut world = p.host.scene.context(&mut p.host.maps);
            walking.clear(&mut world, 0, 0).unwrap();
            walking.add_flag(&mut world, 0, 0, 0x80).unwrap();
        }
        phase(&mut p.state, &mut p.host, node, Scripts::Pre).unwrap();
        assert_eq!(script.read().state, 1);
        assert!(
            p.host.frames[&node]
                .iter()
                .any(|f| f.opcode == 70 && f.payload == b"target visible\0")
        );
        assert!(p.host.errors.is_empty());
    }

    #[test]
    fn actual_entity_tick_runs_native_queue_and_preserves_nested_owner_clocks_and_captured_state() {
        let (mut p, node, _) = super::super::tests::probe();
        let owner = p.state.retain_node(node).unwrap();
        let npc = p
            .state
            .add_npc(2, Location::new(3208, 3216, 0), [1, 2, 3, 4]);
        let other = p.state.retain_node(npc).unwrap();
        let processor = p.state.scripts(&owner).unwrap();
        p.state.ticks = 10;
        let function = Function::new(
            FunctionKind::Void,
            "native.test.queue",
            Native::<Host>::new(
                Captures {
                    nodes: vec![other.clone()],
                    ..Default::default()
                },
                callback,
            ),
        );
        let mut ctx = script_runtime::Context::new(&p.state, &mut p.host, &processor).unwrap();
        processor
            .queue_script(
                Some(function),
                0,
                Strength::Normal,
                false,
                &mut p.state,
                &mut ctx,
            )
            .unwrap();
        let script = processor.queue().pop().unwrap();
        assert_eq!(script.read().next_execution, 11);
        p.state.ticks = 11;
        p.state.context(&mut p.host).entity(node).unwrap();
        assert_eq!(script.read().state, 1);
        assert_eq!(script.read().next_execution, 13);
        assert!(processor.current().is_some_and(|s| s.same(&script)));
        assert_eq!(
            p.state.clocks(&owner).unwrap().read()[script_runtime::MOVEMENT],
            0
        );
        assert_eq!(
            p.state.clocks(&other).unwrap().read()[script_runtime::MOVEMENT],
            12
        );
        assert_eq!(
            p.host.frames[&node]
                .iter()
                .filter(|f| f.opcode == 70)
                .count(),
            1
        );
        p.state.ticks = 12;
        p.state.context(&mut p.host).entity(node).unwrap();
        assert_eq!(script.read().state, 1);
        p.state.ticks = 13;
        p.state.context(&mut p.host).entity(node).unwrap();
        assert_eq!(script.read().state, 2);
        assert!(processor.queue().is_empty());
        assert!(processor.current().is_some_and(|s| s.same(&script)));
        assert_eq!(
            p.state.clocks(&other).unwrap().read()[script_runtime::MOVEMENT],
            14
        );
        assert_eq!(
            p.host.frames[&node]
                .iter()
                .filter(|f| f.opcode == 70)
                .count(),
            2
        );
        assert!(p.host.errors.is_empty());
    }
    #[test]
    fn live_phase_removals_and_reset_use_the_same_processor_animator_and_clock_array() {
        let (mut p, node, _) = super::super::tests::probe();
        let owner = p.state.retain_node(node).unwrap();
        let processor = p.state.scripts(&owner).unwrap();
        let clocks = p.state.clocks(&owner).unwrap();
        assert!(
            p.host.animators[&node]
                .owner()
                .unwrap()
                .clocks()
                .same(&clocks)
        );
        p.state.ticks = 20;
        for strength in [Strength::Weak, Strength::Normal, Strength::Soft] {
            let script = processor
                .create_script(Form::Queued { strength }, None, false)
                .unwrap();
            let mut ctx = script_runtime::Context::new(&p.state, &mut p.host, &processor).unwrap();
            processor
                .add_in(&mut p.state, &mut ctx, Some(&script), strength)
                .unwrap();
        }
        Services::event(
            &mut p.host,
            &mut p.state,
            Event::Scripts(node, Scripts::Weak),
        )
        .unwrap();
        assert_eq!(processor.queue().len(), 2);
        Services::event(
            &mut p.host,
            &mut p.state,
            Event::Scripts(node, Scripts::Normal),
        )
        .unwrap();
        assert_eq!(processor.queue().len(), 1);
        processor.set_delay(55);
        {
            let mut c = clocks.write();
            c[0] = 77;
            c[1] = 99;
            c[6] = 88;
        }
        Services::event(
            &mut p.host,
            &mut p.state,
            Event::Scripts(node, Scripts::Reset),
        )
        .unwrap();
        assert_eq!(processor.delay(), 55);
        assert_eq!(processor.queue().len(), 1);
        assert_eq!(
            [clocks.read()[0], clocks.read()[1], clocks.read()[6]],
            [77, 0, 88]
        );
        let npc = p.state.add_npc(2, Location::new(1, 1, 0), [1, 2, 3, 4]);
        Services::event(
            &mut p.host,
            &mut p.state,
            Event::Scripts(npc, Scripts::Reset),
        )
        .unwrap();
    }
    #[test]
    fn retained_receiver_delay_and_clock_boundaries_read_the_actual_entity() {
        let (mut p, node, _) = super::super::tests::probe();
        let owner = p.state.retain_node(node).unwrap();
        let canonical = p.state.scripts(&owner).unwrap();
        let separate = Runtime::with_entity_in(owner.clone(), p.state.identity());
        separate.set_delay(i32::MAX);
        p.state.ticks = 30;
        let mut ctx = script_runtime::Context::new(&p.state, &mut p.host, &separate).unwrap();
        assert!(!ctx.delayed(&separate, &mut p.state).unwrap());
        canonical.set_delay(31);
        assert!(ctx.delayed(&separate, &mut p.state).unwrap());
        p.state.clocks(&owner).unwrap().write()[6] = 30;
        assert!(ctx.stunned(&separate, &mut p.state).unwrap());
        p.state.ticks = 31;
        assert!(!ctx.delayed(&separate, &mut p.state).unwrap());
        assert!(!ctx.stunned(&separate, &mut p.state).unwrap());
        p.state.clocks(&owner).unwrap().write()[0] = 31;
        assert!(!ctx.finished_moving(&separate, &mut p.state).unwrap());
        p.state.ticks = 32;
        assert!(ctx.finished_moving(&separate, &mut p.state).unwrap());
        separate.delay_entity(4, &mut p.state, &mut ctx).unwrap();
        assert_eq!(separate.delay(), 36);
        assert_eq!(canonical.delay(), 31);
    }
    struct Nested {
        node: NodeHandle,
    }
    impl NativeObject for Nested {
        fn captures(&self) -> Captures {
            Captures {
                nodes: vec![self.node.clone()],
                ..Default::default()
            }
        }
    }
    #[test]
    fn native_nested_foreign_captures_fail_before_the_body_can_emit_any_frame() {
        let (mut p, node, _) = super::super::tests::probe();
        let owner = p.state.retain_node(node).unwrap();
        let mut foreign = State::default();
        let n = foreign.add(1, Location::new(1, 1, 0), [1, 2, 3, 4]);
        let target = foreign.retain_node(n).unwrap();
        assert_eq!(owner.node, target.node);
        let body = Native::<Host>::new(
            Captures {
                objects: vec![Plugin::new(Nested {
                    node: target.clone(),
                })],
                ..Default::default()
            },
            |_, _, host, owner, _| {
                host.send(
                    owner.node,
                    Frame {
                        opcode: 70,
                        payload: vec![0],
                    },
                );
                Ok(true)
            },
        );
        let before = p.host.frames.clone();
        assert!(
            body.invoke(
                &mut p.state,
                &mut p.host,
                &owner,
                Arguments::Void { stage: 0 }
            )
            .is_err()
        );
        assert_eq!(p.host.frames, before);
        let body = Native::<Host>::new(Default::default(), |_, _, host, owner, _| {
            host.send(
                owner.node,
                Frame {
                    opcode: 70,
                    payload: vec![0],
                },
            );
            Ok(true)
        });
        assert!(
            body.invoke(
                &mut p.state,
                &mut p.host,
                &owner,
                Arguments::Interaction { target, stage: 0 }
            )
            .is_err()
        );
        assert_eq!(p.host.frames, before);
    }
    #[test]
    fn script_delay_entity_and_movement_interaction_queries_share_actual_default_locks() {
        use rs09_server::world::{
            action_locks::{Clock, Kind},
            legacy_runtime,
        };
        let (mut p, node, _) = super::super::tests::probe();
        let owner = p.state.retain_node(node).unwrap();
        let processor = p.state.scripts(&owner).unwrap();
        p.state.ticks = 20;
        let mut ctx = script_runtime::Context::new(&p.state, &mut p.host, &processor).unwrap();
        processor.delay_entity(4, &mut p.state, &mut ctx).unwrap();
        let locks = p.state.locks(&owner).unwrap();
        assert_eq!(processor.delay(), 24);
        assert_eq!(locks.get(Kind::Movement).unwrap().expiration(), 24);
        assert_eq!(locks.get(Kind::Interaction).unwrap().expiration(), 24);
        assert_eq!(locks.get(Kind::Teleport).unwrap().expiration(), 0);
        assert_eq!(locks.get(Kind::Component).unwrap().expiration(), 0);
        assert!(
            Services::query(&mut p.host, &mut p.state, super::super::Query::Locked(node)).unwrap()
        );
        assert!(
            legacy_runtime::Bindings::interaction_locked(&mut p.host, &mut p.state, Some(node))
                .unwrap()
        );
        p.state.ticks = 24;
        assert!(
            !Services::query(&mut p.host, &mut p.state, super::super::Query::Locked(node)).unwrap()
        );
        assert!(
            !locks
                .is_kind_locked(Kind::Interaction, &mut p.state, &mut Clock)
                .unwrap()
        );
    }
    #[test]
    fn actual_elapse_callback_receives_the_old_lock_and_outer_unlock_removes_reentrant_replacement()
    {
        use rs09_server::world::{
            action_locks::{self as locks, Clock, Native as Elapse},
            component_objects::{Captures, Plugin},
        };
        let (mut p, node, _) = super::super::tests::probe();
        let owner = p.state.retain_node(node).unwrap();
        let actions = p.state.locks(&owner).unwrap();
        p.state.ticks = 10;
        let key = Some(vec![97]);
        let old = actions
            .lock_custom(key.clone(), 5, None, &mut p.state, &mut Clock)
            .unwrap();
        let event = Plugin::new(Elapse::<Host>::new(
            Captures {
                action_locks: vec![actions.clone()],
                ..Default::default()
            },
            |payload, state, host, node, passed| {
                assert_eq!(passed.expiration(), 0);
                passed.set_elapse(None);
                let actions = &payload.fields().action_locks[0];
                actions.unlock_custom(&Some(vec![97]), true, node.clone(), state, &mut Clock)?;
                let new = actions.lock_custom(Some(vec![97]), 7, None, state, &mut Clock)?;
                assert!(!new.same(&passed));
                host.send(
                    node.unwrap().node,
                    Frame {
                        opcode: 70,
                        payload: b"native LockElapse\0".to_vec(),
                    },
                );
                Ok(())
            },
        ));
        old.set_elapse(Some(event));
        actions
            .unlock_custom(
                &key,
                true,
                Some(owner.clone()),
                &mut p.state,
                &mut locks::Runtime::new(&mut p.host),
            )
            .unwrap();
        assert!(actions.custom().read().is_empty());
        assert_eq!(old.expiration(), 0);
        assert!(
            p.host.frames[&node]
                .iter()
                .any(|f| f.opcode == 70 && f.payload == b"native LockElapse\0")
        );
        let event = Plugin::new(Elapse::<Host>::new(
            Default::default(),
            |_, _, host, node, passed| {
                assert_eq!(passed.expiration(), 0);
                host.send(
                    node.unwrap().node,
                    Frame {
                        opcode: 70,
                        payload: b"before elapse failure\0".to_vec(),
                    },
                );
                Err(locks::Error::Binding("elapse failure".into()))
            },
        ));
        let failed = actions
            .lock_custom(Some(vec![98]), 3, Some(event), &mut p.state, &mut Clock)
            .unwrap();
        assert!(
            actions
                .unlock_custom(
                    &Some(vec![98]),
                    true,
                    Some(owner),
                    &mut p.state,
                    &mut locks::Runtime::new(&mut p.host)
                )
                .is_err()
        );
        assert_eq!(failed.expiration(), 0);
        assert!(
            actions
                .custom()
                .read()
                .get(&Some(vec![98]))
                .is_some_and(|l| l.same(&failed))
        );
        assert!(
            p.host.frames[&node]
                .iter()
                .any(|f| f.payload == b"before elapse failure\0")
        );
    }
}
