//! Real non-Copy function/node objects through the shared processor. Unused
//! world services fail explicitly; these tests do not claim live tick bindings.
use rs09_server::{
    scripts::{
        Arguments, Context, Destination, EntityKind, Error, Form, Level, Location, NodeKind, Path,
        Processor, Result, Runtime, RuntimeScript, Strength,
    },
    world::{
        component_objects::{Captures, NativeObject},
        interaction_listeners::{Function, FunctionKind},
        player_walk::{NodeHandle, State},
    },
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
type P = Processor<Function, NodeHandle>;
struct Program {
    captured: NodeHandle,
    calls: Arc<AtomicUsize>,
    result: bool,
}
impl NativeObject for Program {
    fn captures(&self) -> Captures {
        Captures {
            nodes: vec![self.captured.clone()],
            ..Default::default()
        }
    }
}
struct NativeContext {
    owner: NodeHandle,
    observed: Vec<(NodeHandle, NodeHandle, Arguments<NodeHandle>)>,
    replace: Option<(RuntimeScript, Function)>,
}
macro_rules! unbound {
 ($name:ident($($arg:ident:$ty:ty),* $(,)?)->$ret:ty)=>{fn $name(&mut self,$($arg:$ty),*)->Result<$ret>{Err(Error::Binding(concat!("required test service: ",stringify!($name)).into()))}};
}
impl Context<State, Function, NodeHandle> for NativeContext {
    fn entity_kind(&self, _: &State) -> EntityKind {
        EntityKind::Player
    }
    fn ticks(&mut self, _: &P, state: &mut State) -> Result<i32> {
        Ok(state.ticks)
    }
    unbound!(delayed(_p: &P, _s: &mut State) -> bool);
    unbound!(stunned(_p: &P, _s: &mut State) -> bool);
    unbound!(modal_open(_p: &P, _s: &mut State) -> bool);
    unbound!(interface_open(_p: &P, _s: &mut State) -> bool);
    unbound!(chatbox_open(_p: &P, _s: &mut State) -> bool);
    unbound!(close_interfaces(_p: &P, _s: &mut State) -> ());
    unbound!(reset_animator(_p: &P, _s: &mut State) -> ());
    unbound!(running(_p: &P, _s: &mut State) -> bool);
    unbound!(set_movement_clock(_v: i32, _p: &P, _s: &mut State) -> ());
    unbound!(finished_moving(_p: &P, _s: &mut State) -> bool);
    unbound!(entity_location(_p: &P, _s: &mut State) -> Option<Location>);
    fn node_kind(&self, _: NodeHandle, _: &State) -> NodeKind {
        NodeKind::Other
    }
    unbound!(node_active(_n: NodeHandle, _p: &P, _s: &mut State) -> bool);
    unbound!(node_description(_n: Option<NodeHandle>, _p: &P, _s: &mut State) -> String);
    unbound!(node_location(_n: NodeHandle, _p: &P, _s: &mut State) -> Option<Location>);
    unbound!(
        face_location(
            _n: NodeHandle,
            _own: Option<Location>,
            _p: &P,
            _s: &mut State,
        ) -> Option<Location>
    );
    unbound!(center_location(_n: NodeHandle, _p: &P, _s: &mut State) -> Option<Location>);
    unbound!(
        destination(_d: Destination, _n: NodeHandle, _p: &P, _s: &mut State) -> Option<Location>
    );
    unbound!(pathfind(_n: Option<NodeHandle>, _p: &P, _s: &mut State) -> Path);
    unbound!(line_of_sight(_l: &Location, _p: &P, _s: &mut State) -> bool);
    unbound!(face(_l: Location, _p: &P, _s: &mut State) -> ());
    unbound!(message(_t: &str, _p: &P, _s: &mut State) -> ());
    fn execution_equals(
        &mut self,
        a: Option<Function>,
        b: Option<Function>,
        _: &P,
        _: &mut State,
    ) -> Result<bool> {
        Ok(match (a, b) {
            (Some(a), Some(b)) => a.same(&b),
            (None, None) => true,
            _ => false,
        })
    }
    fn execute(
        &mut self,
        f: Option<Function>,
        args: Arguments<NodeHandle>,
        p: &P,
        state: &mut State,
    ) -> Result<bool> {
        p.validate_in(state)?;
        let f = f.ok_or(Error::Null)?;
        let valid = matches!(
            (&args, f.kind()),
            (Arguments::Void { .. }, FunctionKind::Void)
                | (Arguments::Interaction { .. }, FunctionKind::Interaction)
                | (Arguments::UseWith { .. }, FunctionKind::UseInteraction)
        );
        if !valid {
            return Err(Error::Binding("native function arity mismatch".into()));
        }
        let body = f
            .value::<Program>()
            .ok_or_else(|| Error::Binding("unbound actual native function type".into()))?;
        if !state.contains_handle(&body.captured) {
            return Err(Error::Binding("foreign callback capture".into()));
        }
        if let Some((script, next)) = self.replace.take() {
            script.write().execution = Some(next);
        }
        // Reenter through another handle while the original invoke is still active.
        p.clone().set_delay(p.delay().wrapping_add(1));
        body.calls.fetch_add(1, Ordering::SeqCst);
        self.observed
            .push((self.owner.clone(), body.captured.clone(), args));
        Ok(body.result)
    }
    unbound!(exception_text(_e: &Error, _p: &P, _s: &mut State) -> String);
    unbound!(log(_owner: &str, _level: Level, _text: String, _p: &P, _s: &mut State) -> ());
    unbound!(lock_entity(_ticks: i32, _p: &P, _s: &mut State) -> ());
}
fn actor(state: &mut State, index: i32) -> NodeHandle {
    let n = state.add(index, Location::new(1, 1, 0), [1, 2, 3, 4]);
    state.retain_node(n).unwrap()
}
#[test]
fn runtime_dispatch_uses_captured_function_objects_and_native_nodes_across_reentry() {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut state = State::default();
        let owner = actor(&mut state, 1);
        let target = actor(&mut state, 2);
        let captured = actor(&mut state, 3);
        let p = Runtime::with_entity_in(owner.clone(), state.identity());
        p.set_target(Some(target.clone()));
        let first = Arc::new(AtomicUsize::new(0));
        let second = Arc::new(AtomicUsize::new(0));
        let make = |calls| {
            Function::new(
                FunctionKind::Interaction,
                "native.Script",
                Program {
                    captured: captured.clone(),
                    calls,
                    result: false,
                },
            )
        };
        let a = make(first.clone());
        let b = make(second.clone());
        assert!(!a.same(&b));
        let script = p
            .create_script(Form::Interaction { distance: 1 }, Some(a), false)
            .unwrap();
        let mut ctx = NativeContext {
            owner: owner.clone(),
            observed: vec![],
            replace: Some((script.clone(), b)),
        };
        assert!(
            !p.execute_script(Some(&script), &mut state, &mut ctx)
                .unwrap()
        );
        assert_eq!(first.load(Ordering::SeqCst), 1);
        assert_eq!(second.load(Ordering::SeqCst), 0);
        assert!(
            !p.execute_script(Some(&script), &mut state, &mut ctx)
                .unwrap()
        );
        assert_eq!(second.load(Ordering::SeqCst), 1);
        assert_eq!(p.delay(), 2);
        assert!(ctx.observed.iter().all(|(o, c, args)| o == &owner
            && c == &captured
            && matches!(args,Arguments::Interaction{target:t,stage:0}if t==&target)));
        assert!(p.current().is_some_and(|s| s.same(&script)));
        tx.send(()).unwrap();
    });
    rx.recv_timeout(std::time::Duration::from_secs(10))
        .expect("native script callback held a processor guard");
}
#[test]
fn queued_native_void_and_use_calls_keep_typed_arguments_and_domain() {
    let mut state = State::default();
    let owner = actor(&mut state, 1);
    let used = actor(&mut state, 2);
    let with = actor(&mut state, 3);
    let p = Runtime::with_entity_in(owner.clone(), state.identity());
    let mut ctx = NativeContext {
        owner: owner.clone(),
        observed: vec![],
        replace: None,
    };
    let counter = Arc::new(AtomicUsize::new(0));
    let f = Function::new(
        FunctionKind::Void,
        "native.Void",
        Program {
            captured: owner.clone(),
            calls: counter.clone(),
            result: true,
        },
    );
    p.queue_script(Some(f), 0, Strength::Normal, false, &mut state, &mut ctx)
        .unwrap();
    let script = p.queue().pop().unwrap();
    assert!(
        script
            .read()
            .domain()
            .is_some_and(|d| d.same(&state.identity()))
    );
    p.execute_script(Some(&script), &mut state, &mut ctx)
        .unwrap();
    assert!(matches!(ctx.observed[0].2, Arguments::Void { stage: 0 }));
    let f = Function::new(
        FunctionKind::UseInteraction,
        "native.Use",
        Program {
            captured: owner,
            calls: counter.clone(),
            result: true,
        },
    );
    let script = p
        .create_script(
            Form::QueuedUseWith {
                strength: Strength::Soft,
                used: used.clone(),
                with: with.clone(),
            },
            Some(f),
            false,
        )
        .unwrap();
    p.execute_script(Some(&script), &mut state, &mut ctx)
        .unwrap();
    assert!(
        matches!(&ctx.observed[1].2,Arguments::UseWith{used:u,with:w,stage:0}if u==&used&&w==&with)
    );
    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

struct InspectDrop {
    processor: Runtime,
    count: Arc<AtomicUsize>,
}
impl NativeObject for InspectDrop {
    fn captures(&self) -> Captures {
        Captures {
            processors: vec![self.processor.clone()],
            ..Default::default()
        }
    }
}
impl Drop for InspectDrop {
    fn drop(&mut self) {
        assert_eq!(self.processor.delay(), 91);
        self.count.fetch_add(1, Ordering::SeqCst);
    }
}
#[test]
fn replacing_fields_and_removing_queued_callbacks_drop_payloads_after_unlocking() {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut state = State::default();
        let owner = actor(&mut state, 1);
        let p = Runtime::with_entity_in(owner.clone(), state.identity());
        p.set_delay(91);
        let count = Arc::new(AtomicUsize::new(0));
        let make = || {
            Function::new(
                FunctionKind::Void,
                "native.Drop",
                InspectDrop {
                    processor: p.clone(),
                    count: count.clone(),
                },
            )
        };
        let s = p
            .create_script(
                Form::Queued {
                    strength: Strength::Weak,
                },
                Some(make()),
                false,
            )
            .unwrap();
        let mut ctx = NativeContext {
            owner,
            observed: vec![],
            replace: None,
        };
        p.add_in(&mut state, &mut ctx, Some(&s), Strength::Weak)
            .unwrap();
        drop(s);
        p.remove_weak();
        assert_eq!(count.load(Ordering::SeqCst), 1);
        let s = p
            .create_script(Form::Interaction { distance: 1 }, Some(make()), false)
            .unwrap();
        p.set_approach(Some(s));
        p.set_approach(None);
        assert_eq!(count.load(Ordering::SeqCst), 2);
        tx.send(()).unwrap();
    });
    rx.recv_timeout(std::time::Duration::from_secs(10))
        .expect("native payload destructor ran under a processor guard");
}
