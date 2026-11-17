//! The title screen's script commands (`ScriptRunner` 5600 to 5611, 6500 to
//! 6503, 6506 and 6507) carried out by the login state machine.
//!
//! [`Vm::login_host`] is the object behind them: the client's
//! [`crate::login_flow::LoginFlow`]. Without one (offline previews and the
//! unit tests of other modules) the commands keep the answers of
//! `vm_ops.rs`, which are the original's "not at the title screen" branches.
//!
//! Stack order is the original's: 5600 reads the username below the
//! password; 6501, 6502 and 6506 push their integers and strings in the
//! order `ScriptRunner` does, each on its own stack.
use super::{Result, Stack, Vm};
use crate::{
    ui::{latin1, state::State},
    world_list::Entry,
};

/// A script string as the original's `JagString` bytes (Latin-1; a character
/// outside it cannot come from a script and becomes `?`).
fn bytes(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| u8::try_from(c as u32).unwrap_or(b'?'))
        .collect()
}

impl Vm {
    /// One of the title-screen commands when a login host is installed.
    /// False: there is none, and the caller's own arm applies.
    pub(super) fn login_operation(
        &mut self,
        _: &mut State,
        op: i32,
        s: &mut Stack,
    ) -> Result<bool> {
        let Some(host) = self.login_host.clone() else {
            return Ok(false);
        };
        let mut host = host.borrow_mut();
        match op {
            5600 => {
                let password = s.text()?;
                let username = s.text()?;
                let login_type = s.pop()?;
                host.op5600(&bytes(&username), &bytes(&password), login_type)
                    .map_err(|e| format!("CS2 5600: {e}"))?;
            }
            5601 => host.op5601(),
            5602 => host.op5602(),
            5603 => host.op5603(s.take()?),
            5604 => {
                let name = s.text()?;
                host.op5604(&bytes(&name))
                    .map_err(|e| format!("CS2 5604: {e}"))?;
            }
            5605 => {
                let ints = s.take()?;
                let password = s.text()?;
                let name = s.text()?;
                host.op5605(ints, &bytes(&name), &bytes(&password), self.affiliate)
                    .map_err(|e| format!("CS2 5605: {e}"))?;
            }
            5606 => host.op5606(),
            5607 => s.ints.push(host.op5607()),
            5608 => s.ints.push(host.op5608()),
            5609 => s.ints.push(host.op5609()),
            5610 => {
                let names = host.op5610().map_err(|e| format!("CS2 5610: {e}"))?;
                s.strings
                    .extend(names.iter().map(|name| Some(latin1(name))));
            }
            5611 => s.ints.push(host.op5611()),
            6500 => s.ints.push(host.op6500(self.now_ms)),
            6501 | 6502 => {
                let entry = if op == 6501 {
                    host.op6501()
                } else {
                    host.op6502()
                };
                match entry {
                    Some(Entry {
                        id,
                        flags,
                        activity,
                        country_flag,
                        country_name,
                        players,
                    }) => {
                        s.ints.extend([id, flags, country_flag, players]);
                        s.strings
                            .extend([Some(latin1(&activity)), Some(latin1(&country_name))]);
                    }
                    None => {
                        s.ints.extend([-1, 0, 0, 0]);
                        s.strings.extend([Some(String::new()), Some(String::new())]);
                    }
                }
            }
            6503 => {
                let world = s.pop()?;
                s.ints.push(host.op6503(world));
            }
            6506 => {
                let world = s.pop()?;
                match host.op6506(world) {
                    Some(entry) => {
                        s.ints
                            .extend([entry.flags, entry.country_flag, entry.players]);
                        s.strings.extend([
                            Some(latin1(&entry.activity)),
                            Some(latin1(&entry.country_name)),
                        ]);
                    }
                    None => {
                        s.ints.extend([-1, 0, 0]);
                        s.strings.extend([Some(String::new()), Some(String::new())]);
                    }
                }
            }
            6507 => host
                .op6507(s.take()?)
                .map_err(|_| "CS2 6507: the world list has an empty slot".to_owned())?,
            _ => return Ok(false),
        }
        Ok(true)
    }
}

/// Transcription tests: the expected stack traffic is read off
/// `ScriptRunner.java` (the `opcode < 5700` and `opcode < 6600` blocks); the
/// Java was not run.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        login_flow::{Commands, ScriptError},
        pack::Pack,
        world_list::EmptySlot,
    };
    use std::{cell::RefCell, rc::Rc};

    /// Records every call and answers with fixed values.
    #[derive(Default)]
    struct Host {
        calls: Vec<String>,
        world: Option<Entry>,
        fail: bool,
    }
    fn text(bytes: &[u8]) -> String {
        format!("{bytes:?}")
    }
    impl Commands for Host {
        fn op5600(&mut self, name: &[u8], password: &[u8], kind: i32) -> Result5600 {
            self.calls
                .push(format!("5600 {} {} {kind}", text(name), text(password)));
            Ok(())
        }
        fn op5601(&mut self) {
            self.calls.push("5601".into());
        }
        fn op5602(&mut self) {
            self.calls.push("5602".into());
        }
        fn op5603(&mut self, ints: [i32; 4]) {
            self.calls.push(format!("5603 {ints:?}"));
        }
        fn op5604(&mut self, name: &[u8]) -> Result5600 {
            self.calls.push(format!("5604 {}", text(name)));
            if self.fail {
                Err(ScriptError::InvalidName)
            } else {
                Ok(())
            }
        }
        fn op5605(&mut self, ints: [i32; 4], name: &[u8], password: &[u8], aff: i32) -> Result5600 {
            self.calls.push(format!(
                "5605 {ints:?} {} {} {aff}",
                text(name),
                text(password)
            ));
            Ok(())
        }
        fn op5606(&mut self) {
            self.calls.push("5606".into());
        }
        fn op5607(&self) -> i32 {
            -3
        }
        fn op5608(&self) -> i32 {
            480
        }
        fn op5609(&self) -> i32 {
            21
        }
        fn op5610(&mut self) -> std::result::Result<[Vec<u8>; 5], ScriptError> {
            if self.fail {
                return Err(ScriptError::NoSuggestions);
            }
            Ok([
                b"One".to_vec(),
                b"Two".to_vec(),
                vec![],
                vec![],
                b"Caf\xe9".to_vec(),
            ])
        }
        fn op5611(&self) -> i32 {
            4
        }
        fn op6500(&mut self, now_ms: i64) -> i32 {
            self.calls.push(format!("6500 {now_ms}"));
            0
        }
        fn op6501(&mut self) -> Option<Entry> {
            self.calls.push("6501".into());
            self.world.clone()
        }
        fn op6502(&mut self) -> Option<Entry> {
            self.calls.push("6502".into());
            self.world.clone()
        }
        fn op6503(&mut self, world_id: i32) -> i32 {
            self.calls.push(format!("6503 {world_id}"));
            1
        }
        fn op6506(&self, world_id: i32) -> Option<Entry> {
            self.world.clone().filter(|world| world.id == world_id)
        }
        fn op6507(&mut self, ints: [i32; 4]) -> std::result::Result<(), EmptySlot> {
            self.calls.push(format!("6507 {ints:?}"));
            if self.fail { Err(EmptySlot) } else { Ok(()) }
        }
        fn world_id(&self) -> i32 {
            1
        }
        fn username_input(&self) -> &[u8] {
            b""
        }
    }
    type Result5600 = std::result::Result<(), ScriptError>;

    fn setup() -> (State, Vm, Rc<RefCell<Host>>) {
        let pack = Pack::lazy(
            "login".into(),
            Vec::new(),
            Vec::new(),
            Box::new(|name: &str| Err(format!("{name} absent"))),
        );
        let host = Rc::new(RefCell::new(Host {
            world: Some(Entry {
                id: 12,
                flags: 9,
                activity: b"Trade".to_vec(),
                country_flag: 225,
                country_name: b"Caf\xe9".to_vec(),
                players: 345,
            }),
            ..Host::default()
        }));
        let vm = Vm {
            login_host: Some(Rc::clone(&host) as _),
            affiliate: 9,
            now_ms: 1_234_567,
            ..Vm::default()
        };
        (State::new(pack), vm, host)
    }
    fn run(vm: &mut Vm, state: &mut State, op: i32, ints: &[i32], strings: &[&str]) -> Stack {
        let mut s = Stack {
            ints: ints.to_vec(),
            strings: strings.iter().map(|t| Some((*t).to_owned())).collect(),
        };
        vm.operation(state, op, true, &mut s)
            .unwrap_or_else(|e| panic!("opcode {op}: {e}"));
        s
    }
    fn strings(s: &Stack) -> Vec<&str> {
        s.strings.iter().map(|t| t.as_deref().unwrap()).collect()
    }

    #[test]
    fn login_and_account_commands_reach_the_host_in_the_original_argument_order() {
        let (mut state, mut vm, host) = setup();
        let (st, vm) = (&mut state, &mut vm);
        // `string = stack[ssp]` is the username, `stack[ssp + 1]` the
        // password; an untouched entry below both stays.
        let s = run(vm, st, 5600, &[5, 2], &["below", "Zezima", "pass\u{e9}"]);
        assert_eq!((&s.ints[..], strings(&s)), (&[5][..], vec!["below"]));
        run(vm, st, 5601, &[], &[]);
        run(vm, st, 5602, &[], &[]);
        // isp -= 4: day, month, year, country as pushed.
        let s = run(vm, st, 5603, &[8, 1, 2, 1990, 77], &[]);
        assert_eq!(s.ints, [8]);
        run(vm, st, 5604, &[], &["new name"]);
        let s = run(
            vm,
            st,
            5605,
            &[1, 2, 1990, 77],
            &["x", "new name", "secret"],
        );
        assert_eq!(strings(&s), ["x"]);
        run(vm, st, 5606, &[], &[]);
        assert_eq!(
            host.borrow().calls,
            [
                "5600 [90, 101, 122, 105, 109, 97] [112, 97, 115, 115, 233] 2",
                "5601",
                "5602",
                "5603 [1, 2, 1990, 77]",
                "5604 [110, 101, 119, 32, 110, 97, 109, 101]",
                "5605 [1, 2, 1990, 77] [110, 101, 119, 32, 110, 97, 109, 101] \
                 [115, 101, 99, 114, 101, 116] 9",
                "5606",
            ]
        );
        assert_eq!(run(vm, st, 5607, &[], &[]).ints, [-3]);
        assert_eq!(run(vm, st, 5608, &[], &[]).ints, [480]);
        assert_eq!(run(vm, st, 5609, &[], &[]).ints, [21]);
        assert_eq!(run(vm, st, 5611, &[], &[]).ints, [4]);
        // Five strings in order, Latin-1 bytes as characters.
        let s = run(vm, st, 5610, &[], &["below"]);
        assert_eq!(strings(&s), ["below", "One", "Two", "", "", "Caf\u{e9}"]);

        // What throws in the original aborts the script here.
        host.borrow_mut().fail = true;
        let mut s = Stack::default();
        assert!(vm.operation(st, 5610, true, &mut s).is_err());
        s.strings.push(Some("_".into()));
        assert!(vm.operation(st, 5604, true, &mut s).is_err());
        s.ints.extend([0, 0, 0, 0]);
        assert!(vm.operation(st, 6507, true, &mut s).is_err());
    }

    #[test]
    fn world_list_commands_push_in_the_original_order() {
        let (mut state, mut vm, host) = setup();
        let (st, vm) = (&mut state, &mut vm);
        // The clock the fetch throttles on is the VM's.
        assert_eq!(run(vm, st, 6500, &[], &[]).ints, [0]);
        // world.id, world.flags, (activity), info.flag, (info.name),
        // world.players: the integers and the strings on their own stacks.
        for op in [6501, 6502] {
            let s = run(vm, st, op, &[4], &["below"]);
            assert_eq!(s.ints, [4, 12, 9, 225, 345]);
            assert_eq!(strings(&s), ["below", "Trade", "Caf\u{e9}"]);
        }
        assert_eq!(run(vm, st, 6503, &[4, 12], &[]).ints, [4, 1]);
        // 6506 is 6501 without the id.
        let s = run(vm, st, 6506, &[4, 12], &[]);
        assert_eq!(s.ints, [4, 9, 225, 345]);
        assert_eq!(strings(&s), ["Trade", "Caf\u{e9}"]);
        let s = run(vm, st, 6506, &[13], &[]);
        assert_eq!(s.ints, [-1, 0, 0]);
        assert_eq!(strings(&s), ["", ""]);
        // isp -= 4, in push order.
        assert_eq!(run(vm, st, 6507, &[4, 0, 1, 2, 0], &[]).ints, [4]);
        host.borrow_mut().world = None;
        for op in [6501, 6502] {
            let s = run(vm, st, op, &[], &[]);
            assert_eq!(s.ints, [-1, 0, 0, 0]);
            assert_eq!(strings(&s), ["", ""]);
        }
        assert_eq!(
            host.borrow().calls,
            [
                "6500 1234567",
                "6501",
                "6502",
                "6503 12",
                "6507 [0, 1, 2, 0]",
                "6501",
                "6502"
            ]
        );
    }

    /// Without a host the commands keep the answers of `vm_ops.rs`.
    #[test]
    fn without_a_host_the_title_commands_take_the_logged_in_branches() {
        let (mut state, mut vm, _) = setup();
        vm.login_host = None;
        let (st, vm) = (&mut state, &mut vm);
        assert_eq!(run(vm, st, 5607, &[], &[]).ints, [-2]);
        assert_eq!(run(vm, st, 6500, &[], &[]).ints, [1]);
        assert_eq!(run(vm, st, 6506, &[1], &[]).ints, [-1, 0, 0]);
        let s = run(vm, st, 5600, &[0], &["name", "password"]);
        assert!(s.ints.is_empty() && s.strings.is_empty());
    }
}
