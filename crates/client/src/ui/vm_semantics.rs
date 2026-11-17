//! Server-message semantics that need the VM's session state: typed game
//! messages, the chat blocks of PLAYER_INFO, the system update countdown, the
//! varc default of a game session and the two varp cache messages.
//!
//! Sources: `Protocol.readPacketInternal`, `Protocol.readExtendedPlayerInfo`,
//! `Protocol.loop`, `client.resetGameSessionState` and `VarpDomain`.
use super::*;
use crate::ui::{game_message, public_chat};

/// `VarpDomain.varp.length`: FORCE_VARP_REFRESH compares this many entries.
const VARP_CAPACITY: i32 = 3500;

/// `VarpTypeList.count`: the capacity of config group 16, one more than its
/// highest file id. The pack lists every file of the group as `varptype/ID`.
fn varp_type_count(state: &State) -> i32 {
    const FAMILY: &str = "varptype/";
    state
        .pack
        .manifest
        .files
        .range::<str, _>((
            std::ops::Bound::Included(FAMILY),
            std::ops::Bound::Unbounded,
        ))
        .map_while(|(name, _)| name.strip_prefix(FAMILY))
        .filter_map(|id| id.parse::<i32>().ok())
        .max()
        .map_or(0, |highest| highest.saturating_add(1))
}

impl Vm {
    /// MESSAGE_GAME: a request suffix turns the text into a typed chat line,
    /// subject to the ignore list and Tutorial Island.
    pub(super) fn game_message(&mut self, state: &mut State, text: &[u8]) -> Result<()> {
        let line = game_message::classify(text, self.language, self.in_tutorial_island(), |key| {
            self.social.ignores.iter().any(|ignore| ignore.key == key)
        })?;
        if let Some(line) = line {
            self.add_message(state, line);
        }
        Ok(())
    }

    /// The public chat block of PLAYER_INFO (flag 0x80) for a player whose
    /// name is known. Returns the overhead text when the message is shown and
    /// adds its chat-box line when there is an interface state to add it to.
    ///
    /// Quick chat needs the phrase definitions of the interface asset pack;
    /// without an interface state it is not shown. A phrase that cannot be
    /// decoded is an error for the caller to report.
    pub fn public_chat(
        &mut self,
        mut state: Option<&mut State>,
        name37: i64,
        name: &str,
        color_effect: i32,
        rights: i32,
        payload: &[u8],
    ) -> Result<Option<crate::overheads::Chat>> {
        let context = public_chat::Context {
            restricted: self.chat_restricted || self.quick_chat,
            tutorial: self.in_tutorial_island(),
            ignored: self.social.ignores.iter().any(|i| i.key == name37),
        };
        if !public_chat::wanted(color_effect, rights, context) {
            return Ok(None);
        }
        if color_effect & public_chat::QUICK_CHAT != 0 && state.is_none() {
            return Ok(None);
        }
        let shown = public_chat::show(
            color_effect,
            rights,
            name,
            payload,
            crate::ui::social::message,
            |id, body| match state.as_deref_mut() {
                Some(state) => crate::ui::quickchat::message(state, id, body),
                None => Err("Quick chat needs the interface assets".into()),
            },
        )?;
        if let Some(state) = state {
            self.add_message(state, shown.line);
        }
        Ok(Some(crate::overheads::Chat {
            text: shown.overhead,
            color: shown.color,
            effect: shown.effect,
            loops: 150,
        }))
    }

    /// The forced chat block of PLAYER_INFO (flag 0x20): always shown
    /// overhead, and logged as a type-2 line when it starts with `~` or the
    /// speaker is the local player.
    pub fn forced_chat(
        &mut self,
        state: Option<&mut State>,
        name: &str,
        is_self: bool,
        text: &[u8],
    ) -> crate::overheads::Chat {
        let (text, line) = public_chat::forced(text, name, is_self);
        if let (Some(state), Some(line)) = (state, line) {
            self.add_message(state, line);
        }
        crate::overheads::Chat {
            text,
            color: 0,
            effect: 0,
            loops: 150,
        }
    }

    /// One client cycle of the system update countdown (`Protocol.loop`): above
    /// one it counts down and stamps the misc transmit, so scripts that listen
    /// for it redraw the clock; it stops at one.
    pub fn advance_reboot_timer(&mut self, state: &mut State) {
        if self.reboot_timer > 1 {
            self.reboot_timer -= 1;
            state.misc_transmit_at = state.transmit_timer;
        }
    }

    /// The varc part of `client.resetGameSessionState`, which runs when a login
    /// succeeds: every varc becomes -1, including ones set before the login.
    /// The rest of that reset belongs to the session's other owners.
    pub fn reset_session_varcs(&mut self) {
        self.varcs.clear();
        self.varc_default = -1;
    }

    /// `VarcDomain.varcs[id]`: zero before the first game session, -1 once a
    /// session has reset them, until something sets the varc.
    pub(super) fn varc(&self, id: i32) -> i32 {
        self.varcs.get(&id).copied().unwrap_or(self.varc_default)
    }

    /// RESET_CLIENT_VARCACHE (`VarpDomain.reset`): both tables are zeroed for
    /// the varps whose type has client code 0, the others keep their values,
    /// and every pending timer is dropped. Varps at or above the type count
    /// are not visited by the original loop and are left alone.
    pub(super) fn reset_varp_cache(&mut self, state: &mut State) -> Result<()> {
        let count = varp_type_count(state);
        let mut cleared = Vec::new();
        for &id in self.varps.keys().chain(self.pending_varps.values.keys()) {
            if (0..count).contains(&id) && Self::varp_client_code(state, id)? == 0 {
                cleared.push(id);
            }
        }
        for id in cleared {
            self.varps.remove(&id);
            self.pending_varps.values.remove(&id);
        }
        let values = std::mem::take(&mut self.pending_varps.values);
        self.pending_varps = crate::ui::varps::Pending::default();
        self.pending_varps.values = values;
        state.varp_writer = state.varp_writer.wrapping_add(32);
        Ok(())
    }

    /// FORCE_VARP_REFRESH: every varp whose active value differs from the
    /// server's takes the server's, in ascending id order, with the same
    /// refresh and listener notice as a polled change. This includes a varp a
    /// script set that the server never sent, which goes back to zero.
    pub(super) fn force_varp_refresh(&mut self, state: &mut State) -> Result<()> {
        let ids: std::collections::BTreeSet<i32> = self
            .varps
            .keys()
            .chain(self.pending_varps.values.keys())
            .copied()
            .filter(|id| (0..VARP_CAPACITY).contains(id))
            .collect();
        for id in ids {
            let server = self.pending_varps.values.get(&id).copied().unwrap_or(0);
            if self.varps.get(&id).copied().unwrap_or(0) != server {
                self.varps.insert(id, server);
                // refreshMagicVarp runs AreaSoundManager.updateMulti first.
                #[cfg(any(target_os = "windows", target_arch = "wasm32"))]
                self.ambient_updates.push(self.varps.clone());
                self.refresh_varp(state, id)?;
                state.updated_varps[(state.varp_writer & 31) as usize] = id;
                state.varp_writer = state.varp_writer.wrapping_add(1);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! Driven through the real VM with server frames written in the original
    //! byte orders. Expected effects are worked out by hand from the Java named
    //! in the module comment (transcription tests); the original classes are
    //! not executed.
    use super::*;
    use crate::{pack::Pack, ui::social};
    use rs09_protocol::{buffer::Writer, game::Frame as Wire};

    /// Varp types: 1 and 2 have client code 0 (2 by an empty definition), 3
    /// has client code 5 (mouse buttons) and 6 has client code 9, so the type
    /// count is 7 with 0, 4 and 5 undefined. Script 900 copies varc 5 to 6.
    /// Quick chat phrase 7 is "Level <!" with one single-byte value.
    fn setup() -> (State, Vm) {
        let script = [
            &[0][..],
            &[0, 42, 0, 0, 0, 5],
            &[0, 43, 0, 0, 0, 6],
            &[0, 21, 0],
            // Three instructions, no locals or arguments, no switch tables.
            &[0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            &[0, 1],
        ]
        .concat();
        let files = vec![
            ("varptype/1".to_owned(), vec![5, 0, 0, 0]),
            ("varptype/2".to_owned(), vec![0]),
            ("varptype/3".to_owned(), vec![5, 0, 5, 0]),
            ("varptype/6".to_owned(), vec![5, 0, 9, 0]),
            ("script/900".to_owned(), script),
            (
                "qcphrase/7".to_owned(),
                [&[1][..], b"Level <!\0", &[3, 1, 0, 4, 0, 1, 0]].concat(),
            ),
            ("quickchat/counts".to_owned(), vec![]),
        ];
        let pack = Pack::lazy(
            "Server semantics tests".into(),
            Vec::<String>::new(),
            files,
            Box::new(|name| Err(format!("{name} is not in the test pack"))),
        );
        (State::new(pack), Vm::default())
    }
    fn send(vm: &mut Vm, state: &mut State, opcode: u8, w: Writer) -> Result<bool> {
        vm.apply_frame(
            state,
            &Wire {
                opcode,
                payload: w.into_vec(),
            },
        )
    }
    fn message(vm: &mut Vm, state: &mut State, text: &[u8]) -> Result<bool> {
        let mut w = Writer::new();
        w.pjstr(text).unwrap();
        send(vm, state, 70, w)
    }
    fn latest(vm: &Vm) -> (i32, &str, &str) {
        let line = &vm.messages[0];
        (line.kind, line.name.as_str(), line.text.as_str())
    }
    fn ignore(vm: &mut Vm, name: &str) {
        vm.social.ignores.push(social::Ignore {
            key: social::encode(name),
            name: name.into(),
        });
    }

    #[test]
    fn game_messages_become_typed_lines_through_the_real_packet() {
        let (mut state, mut vm) = setup();
        state.transmit_timer = 4;
        assert!(message(&mut vm, &mut state, b"Bob:tradereq:").unwrap());
        assert_eq!(latest(&vm), (4, "Bob", "wishes to trade with you."));
        assert_eq!((vm.chat_lines, state.chat_transmit_at), (1, 4));
        assert!(message(&mut vm, &mut state, b"Bob:9 a side:chalreq:").unwrap());
        assert_eq!(latest(&vm), (8, "Bob", "9 a side"));
        assert!(message(&mut vm, &mut state, b"Plain").unwrap());
        assert_eq!(latest(&vm), (0, "", "Plain"));
        assert_eq!(vm.messages.len(), 3);

        // An ignored name is dropped, compared on its base-37 key.
        ignore(&mut vm, "bob");
        assert!(message(&mut vm, &mut state, b"BOB:duelfriend:").unwrap());
        assert_eq!(vm.messages.len(), 3);
        assert!(message(&mut vm, &mut state, b"Bobby:duelfriend:").unwrap());
        assert_eq!(latest(&vm), (15, "Bobby", ""));

        // On Tutorial Island only clan notices and plain text get through.
        vm.location = [3100, 3100, 0];
        for text in [&b"Ann:tradereq:"[..], b"Sending...:trade:", b"Ann:clanreq:"] {
            assert!(message(&mut vm, &mut state, text).unwrap());
        }
        assert_eq!(vm.messages.len(), 4);
        assert!(message(&mut vm, &mut state, b"Joined.:clan:").unwrap());
        assert_eq!(latest(&vm), (11, "", "Joined."));
        assert!(message(&mut vm, &mut state, b"Plain").unwrap());
        assert_eq!(vm.messages.len(), 6);

        // The request the original cannot cut fails the packet, off the island.
        vm.location = [3200, 3200, 0];
        assert!(message(&mut vm, &mut state, b"Ann:chalreq:").is_err());
        assert_eq!(vm.messages.len(), 6);
    }

    #[test]
    fn public_chat_shows_typed_and_quick_messages_and_honours_the_filters() {
        let (mut state, mut vm) = setup();
        let bob = social::encode("bob");
        let typed = rs09_protocol::game::chat::WORDPACK
            .encode(b"hi <there>")
            .unwrap();
        let typed = [&[10][..], &typed[..]].concat();
        let chat = vm
            .public_chat(Some(&mut state), bob, "Bob", 0x0201, 1, &typed)
            .unwrap()
            .unwrap();
        assert_eq!(
            (chat.text.as_str(), chat.color, chat.effect, chat.loops),
            ("Hi <lt>there<gt>", 2, 1, 150)
        );
        assert_eq!(latest(&vm), (1, "<img=0>Bob", "Hi <lt>there<gt>"));
        assert_eq!(vm.messages[0].phrase, -1);

        // Quick chat: phrase 7 with the value 99, colour bits under the flag.
        let quick = [0, 7, 99];
        let chat = vm
            .public_chat(Some(&mut state), bob, "Bob", 0x8300, 0, &quick)
            .unwrap()
            .unwrap();
        assert_eq!(
            (chat.text.as_str(), chat.color, chat.effect),
            ("Level 99!", 3, 0)
        );
        assert_eq!(latest(&vm), (17, "Bob", "Level 99!"));
        assert_eq!(vm.messages[0].phrase, 7);
        // Without an interface state the phrase cannot be read and nothing shows.
        assert!(
            vm.public_chat(None, bob, "Bob", 0x8000, 0, &quick)
                .unwrap()
                .is_none()
        );
        // Typed chat is still shown overhead without one.
        assert!(
            vm.public_chat(None, bob, "Bob", 0, 0, &typed)
                .unwrap()
                .is_some()
        );
        assert_eq!(vm.messages.len(), 2);
        // A truncated value is an error and adds nothing.
        assert!(
            vm.public_chat(Some(&mut state), bob, "Bob", 0x8000, 0, &[0, 7])
                .is_err()
        );
        assert_eq!(vm.messages.len(), 2);

        // The chat restriction drops typed chat from ordinary players only.
        vm.chat_restricted = true;
        let mut shown = |vm: &mut Vm, word, rights, payload: &[u8]| {
            vm.public_chat(Some(&mut state), bob, "Bob", word, rights, payload)
                .unwrap()
                .is_some()
        };
        assert!(!shown(&mut vm, 0, 0, &typed));
        assert!(!shown(&mut vm, 0, 1, &typed));
        assert!(shown(&mut vm, 0, 2, &typed));
        assert!(shown(&mut vm, 0x8000, 0, &quick));
        vm.chat_restricted = false;
        // The ignore list drops both kinds, again except from staff.
        ignore(&mut vm, "bob");
        assert!(!shown(&mut vm, 0, 0, &typed));
        assert!(!shown(&mut vm, 0x8000, 1, &quick));
        assert!(shown(&mut vm, 0x8000, 2, &quick));
        vm.social.ignores.clear();
        // Tutorial Island drops everything.
        vm.location = [3100, 9500, 0];
        assert!(!shown(&mut vm, 0, 2, &typed));
        assert!(!shown(&mut vm, 0x8000, 2, &quick));
        assert_eq!(vm.messages.len(), 5);
    }

    #[test]
    fn forced_chat_is_logged_for_a_tilde_or_the_local_player_even_on_the_island() {
        let (mut state, mut vm) = setup();
        vm.location = [3100, 3100, 0];
        ignore(&mut vm, "bob");
        let chat = vm.forced_chat(Some(&mut state), "Bob", false, b"Hello");
        assert_eq!(
            (chat.text.as_str(), chat.color, chat.effect, chat.loops),
            ("Hello", 0, 0, 150)
        );
        assert!(vm.messages.is_empty());
        let chat = vm.forced_chat(Some(&mut state), "Bob", false, b"~Hello");
        assert_eq!(chat.text, "Hello");
        assert_eq!(latest(&vm), (2, "Bob", "Hello"));
        vm.forced_chat(Some(&mut state), "Me", true, b"Mine");
        assert_eq!(latest(&vm), (2, "Me", "Mine"));
        assert_eq!(vm.messages.len(), 2);
        // Without an interface state there is no chat box to log to.
        assert_eq!(vm.forced_chat(None, "Me", true, b"~Mine").text, "Mine");
        assert_eq!(vm.messages.len(), 2);
    }

    #[test]
    fn the_system_update_countdown_runs_each_cycle_and_stops_at_one() {
        let (mut state, mut vm) = setup();
        let read = |vm: &mut Vm, state: &mut State| {
            let mut stack = Stack::default();
            vm.operation(state, 3317, false, &mut stack).unwrap();
            stack.ints[0]
        };
        // Nothing counts while no update is announced.
        state.transmit_timer = 7;
        vm.advance_reboot_timer(&mut state);
        assert_eq!((read(&mut vm, &mut state), state.misc_transmit_at), (0, 0));

        // UPDATE_REBOOT_TIMER carries 600 ms ticks: thirty client cycles each.
        let mut w = Writer::new();
        w.p2(2);
        assert!(send(&mut vm, &mut state, 85, w).unwrap());
        assert_eq!((read(&mut vm, &mut state), state.misc_transmit_at), (60, 7));
        for cycle in 1..=59 {
            state.transmit_timer = 7 + cycle;
            vm.advance_reboot_timer(&mut state);
            assert_eq!(read(&mut vm, &mut state), 60 - cycle);
            assert_eq!(state.misc_transmit_at, 7 + cycle);
        }
        // At one the countdown holds and no longer stamps the transmit.
        state.transmit_timer = 500;
        vm.advance_reboot_timer(&mut state);
        assert_eq!((read(&mut vm, &mut state), state.misc_transmit_at), (1, 66));
        // A timer of zero cancels the announcement.
        let mut w = Writer::new();
        w.p2(0);
        assert!(send(&mut vm, &mut state, 85, w).unwrap());
        vm.advance_reboot_timer(&mut state);
        assert_eq!(read(&mut vm, &mut state), 0);
    }

    #[test]
    fn unset_varcs_read_zero_before_a_session_and_minus_one_after() {
        let (mut state, mut vm) = setup();
        let copy = |vm: &mut Vm, state: &mut State| {
            let hook = Hook::new(Key::component(-1), vec![HookArg::Int(900)]);
            vm.run(state, &hook, 100).unwrap();
            vm.varcs[&6]
        };
        // Script 900 stores varc 5 in varc 6 through the real interpreter.
        assert_eq!(copy(&mut vm, &mut state), 0);
        vm.varcs.insert(5, 44);
        assert_eq!(copy(&mut vm, &mut state), 44);
        // The login reset forgets values set on the login screen too.
        vm.reset_session_varcs();
        assert!(vm.varcs.is_empty());
        assert_eq!(copy(&mut vm, &mut state), -1);
        vm.varcs.insert(5, 0);
        assert_eq!(copy(&mut vm, &mut state), 0);
    }

    #[test]
    fn the_varp_cache_reset_spares_client_coded_and_untyped_varps() {
        let (mut state, mut vm) = setup();
        // Server values for client code 0 (1, 2), client codes 5 and 9 (3, 6),
        // an undefined id below the type count (4) and one at it (7).
        for id in [1, 2, 3, 4, 6, 7] {
            let frame = rs09_protocol::game::updates::varp(id, id * 10);
            assert!(vm.apply_frame(&mut state, &frame).unwrap());
        }
        vm.poll_updates(&mut state, 2).unwrap();
        assert_eq!(vm.varps.len(), 6);
        assert_eq!((vm.mouse_buttons, vm.inserting), (30, 60));
        // A script-side value with a pending timer, and a queued server value.
        vm.varps.insert(2, 99);
        vm.pending_varps.client(2, 0);
        let frame = rs09_protocol::game::updates::varp(6, 61);
        assert!(vm.apply_frame(&mut state, &frame).unwrap());

        let writer = state.varp_writer;
        assert!(send(&mut vm, &mut state, 89, Writer::new()).unwrap());
        assert_eq!(state.varp_writer, writer + 32);
        let read = |vm: &Vm, id: i32| {
            (
                vm.varps.get(&id).copied().unwrap_or(0),
                vm.pending_varps.values.get(&id).copied().unwrap_or(0),
            )
        };
        // Client code 0, defined or not, is zeroed in both tables.
        assert_eq!(
            (read(&vm, 1), read(&vm, 2), read(&vm, 4)),
            ((0, 0), (0, 0), (0, 0))
        );
        // Client-coded varps and ids past the type count keep both values.
        assert_eq!(
            (read(&vm, 3), read(&vm, 6), read(&vm, 7)),
            ((30, 30), (60, 61), (70, 70))
        );
        // Every timer is gone: the queued value for varp 6 never arrives.
        let ring = state.varp_writer;
        vm.poll_updates(&mut state, 1_000_000).unwrap();
        assert_eq!((read(&vm, 6), state.varp_writer), ((60, 61), ring));
        assert_eq!(vm.inserting, 60);
    }

    #[test]
    fn a_forced_refresh_visits_every_differing_varp_in_id_order() {
        let (mut state, mut vm) = setup();
        // Queued server values for 6 and 3, and a script-set varp 2 the server
        // never sent, which the refresh puts back to the server's zero.
        for (id, value) in [(6, 1), (3, 4)] {
            let frame = rs09_protocol::game::updates::varp(id, value);
            assert!(vm.apply_frame(&mut state, &frame).unwrap());
        }
        vm.varps.insert(2, 9);
        // Equal values, and ids outside the original table, are not touched.
        vm.varps.insert(1, 5);
        vm.pending_varps.values.insert(1, 5);
        vm.varps.insert(3500, 8);
        vm.varps.insert(-1, 8);
        let writer = state.varp_writer;
        assert!(send(&mut vm, &mut state, 128, Writer::new()).unwrap());
        assert_eq!(state.varp_writer, writer + 3);
        let ring: Vec<i32> = (0..3)
            .map(|i| state.updated_varps[((writer + i) & 31) as usize])
            .collect();
        assert_eq!(ring, [2, 3, 6]);
        assert_eq!(
            [1, 2, 3, 6, 3500, -1].map(|id| vm.varps.get(&id).copied().unwrap_or(0)),
            [5, 0, 4, 1, 8, 8]
        );
        // The client-coded varps were refreshed as they changed.
        assert_eq!((vm.mouse_buttons, vm.inserting), (4, 1));
        // Nothing differs any more.
        assert!(send(&mut vm, &mut state, 128, Writer::new()).unwrap());
        assert_eq!(state.varp_writer, writer + 3);
    }

    #[test]
    fn the_varp_type_count_is_one_past_the_highest_listed_file() {
        let (state, _) = setup();
        assert_eq!(varp_type_count(&state), 7);
        let empty = Pack::lazy(
            "Empty".into(),
            ["varpish/9".to_owned(), "varptype".to_owned()],
            vec![],
            Box::new(|_| Err("empty".into())),
        );
        assert_eq!(varp_type_count(&State::new(empty)), 0);
    }
}
