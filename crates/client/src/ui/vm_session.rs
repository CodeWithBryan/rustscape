//! The script VM's share of logging in and out: the facts a login reply
//! carries, and the statics `client.resetGameSessionState`,
//! `LoginManager.processLogout` and `LoginManager.reconnect` clear.
use super::{State, Vm};
use crate::session::WorldLogin;

impl Vm {
    /// `LoginManager.loop` step 8 (LoginManager.java:502-511) for the fields
    /// scripts read, with the two launch facts that travel with them:
    ///
    /// | original | here | read by |
    /// |---|---|---|
    /// | `staffModLevel` | `rights` | CS2 3316, chat filters |
    /// | `blackmarks` | `blackmarks` | CS2 3323/3324 |
    /// | `playerUnderage && !parentalChatConsent` | `chat_restricted` | CS2 3328 |
    /// | `mapQuickChat` | `quick_chat` | CS2 3329 |
    /// | `playerMember` | `site_members` | CS2 3325 |
    /// | `mapMembers` | `members` | CS2 3312, CS1 item checks |
    /// | `Player.worldId` | `world` | CS2 3318, friend list |
    /// | `Player.usernameInput` | `player_name` | CS2 5015 until the appearance arrives |
    ///
    /// `LoginManager.reply` is 2 at this point. `parentalAdvertConsent` only
    /// drives the applet's advert frame; `MouseRecorder.enabled` and
    /// `PlayerList.selfId` are kept by [`crate::lifecycle::Lifecycle`].
    pub fn apply_login(&mut self, world: &WorldLogin, world_id: i32, username: &str) {
        self.rights = i32::from(world.staff_level);
        self.blackmarks = i32::from(world.blackmarks);
        self.chat_restricted = world.underage && !world.parental_chat_consent;
        self.quick_chat = world.quick_chat;
        self.site_members = world.player_member;
        self.members = world.map_members;
        self.world = world_id;
        self.player_name = username.to_owned();
        self.login_reply = Some(2);
    }

    /// The local player's appearance block arrived (`Player.decodeAppearance`,
    /// Player.java:361): `username = Base37.decode37(name37).toTitleCase()`,
    /// which CS2 5015 and the clan and friend reducers then use. A value the
    /// original's decoder rejects leaves the name as it was.
    pub fn apply_self_name(&mut self, name37: i64) {
        if let Some(name) = crate::player_interaction::username(name37) {
            self.player_name = name;
        }
    }

    /// `VarpDomain.reset`: both tables go back to 0 for every variable whose
    /// type has client code 0, and all pending timers are dropped. Variables
    /// with a client code (brightness, volumes, mouse buttons, chat effects
    /// and the like) keep their values. Without interface assets the types
    /// are unknown and every variable is treated as code 0.
    pub fn reset_varps(&mut self, state: Option<&State>) {
        let keep = |id: i32| {
            state.is_some_and(|state| Self::varp_client_code(state, id).is_ok_and(|code| code != 0))
        };
        self.varps.retain(|&id, _| keep(id));
        let mut values = std::mem::take(&mut self.pending_varps.values);
        values.retain(|&id, _| keep(id));
        self.pending_varps = super::super::varps::Pending::default();
        self.pending_varps.values = values;
    }

    /// The statics of `client.resetGameSessionState` (client.java:631-755)
    /// this VM holds. `username` is `Player.usernameInput`: with
    /// `PlayerList.self` replaced by a new `Player`, CS2 5015 falls back to it
    /// and the combat level reads 0.
    ///
    /// Run energy, weight, the three chat filters, the ignore list and the
    /// string variables are not touched by the original and are kept.
    pub fn reset_session(&mut self, state: Option<&State>, username: &str) {
        self.logout_on_disconnect_timer = 0;
        self.reboot_timer = 0;
        self.outgoing.clear();
        self.minimap.hints = Default::default();
        self.messages.clear();
        self.minimap.state = 0;
        self.effects.clear();
        self.chat_lines = 0;
        self.player_name = username.to_owned();
        self.combat_level = 0;
        self.social.reset_session();
        self.reset_varps(state);
        // `VarcDomain.varcs` is `int[2000]`; every element becomes -1.
        for id in 0..2000 {
            self.varcs.insert(id, -1);
        }
        self.player_options.labels = Default::default();
        self.player_options.secondary = [false; 8];
        self.player_options.cursors = [-1; 8];
        self.boosted_levels = [0; 25];
        self.base_levels = [0; 25];
        self.experience = [0; 25];
        self.walk_text = None;
        self.preferences.never_remove_roofs = false;
        self.menu_style = None;
    }

    /// The statics of `LoginManager.processLogout` (LoginManager.java:649-695)
    /// and of the `InterfaceList.resetToLoginScreen(true)` it ends with.
    pub fn process_logout(&mut self, state: Option<&State>, username: &str) {
        self.minimap.hints = Default::default();
        self.reset_varps(state);
        self.player_name = username.to_owned();
        self.combat_level = 0;
    }

    /// The statics of `LoginManager.reconnect` (LoginManager.java:830-862).
    pub fn reconnect(&mut self) {
        self.outgoing.clear();
        self.minimap.state = 0;
        self.reboot_timer = 0;
    }
}

/// Transcription tests: the expected values are read off
/// `ScriptRunner.java` (the CS2 commands), `LoginManager.java:502-511`,
/// `Protocol.java:1195-1202` (clan rank) and `VarpDomain.java:55-64`; the
/// original classes are not executed.
#[cfg(test)]
mod tests {
    use super::super::Stack;
    use super::*;
    use crate::pack::Pack;
    use rs09_protocol::game::events::{ClanMember, ServerEvent};

    fn pack(files: &[(&str, Vec<u8>)]) -> Pack {
        let mut body = Vec::new();
        let mut entries = serde_json::Map::new();
        for (name, bytes) in files {
            entries.insert(
                (*name).to_owned(),
                serde_json::json!([body.len(), bytes.len()]),
            );
            body.extend_from_slice(bytes);
        }
        let manifest =
            serde_json::to_vec(&serde_json::json!({"source":"session tests","files":entries}))
                .unwrap();
        let mut bytes = b"RS09VIS1".to_vec();
        bytes.extend((manifest.len() as u32).to_le_bytes());
        bytes.extend(manifest);
        bytes.extend(body);
        Pack::decode(bytes).unwrap()
    }
    fn setup() -> (State, Vm) {
        // Variable 3 has client code 6 (chat effects); 4 and 5 have none.
        let files = [
            ("varptype/3", vec![5, 0, 6, 0]),
            ("varptype/4", vec![0]),
            ("varptype/5", vec![0]),
        ];
        (State::new(pack(&files)), Vm::default())
    }
    fn int(vm: &mut Vm, state: &mut State, op: i32) -> i32 {
        let mut stack = Stack::default();
        vm.operation(state, op, false, &mut stack)
            .unwrap_or_else(|e| panic!("opcode {op}: {e}"));
        assert_eq!(stack.ints.len(), 1, "opcode {op} leaves one integer");
        stack.ints[0]
    }
    fn text(vm: &mut Vm, state: &mut State, op: i32) -> String {
        let mut stack = Stack::default();
        vm.operation(state, op, false, &mut stack)
            .unwrap_or_else(|e| panic!("opcode {op}: {e}"));
        stack.strings.pop().unwrap().unwrap()
    }
    fn world() -> WorldLogin {
        WorldLogin {
            staff_level: 2,
            blackmarks: 7,
            underage: true,
            parental_chat_consent: false,
            parental_ad_consent: true,
            quick_chat: true,
            mouse_recording: true,
            player_index: 513,
            player_member: true,
            map_members: false,
        }
    }

    #[test]
    fn scripts_read_the_login_reply_facts() {
        let (mut state, mut vm) = setup();
        let (st, vm) = (&mut state, &mut vm);
        // Before any login: the Java statics' initial values.
        assert_eq!(
            [3312, 3316, 3318, 3323, 3324, 3325, 3328, 3329].map(|op| int(vm, st, op)),
            [0; 8]
        );
        assert_eq!(text(vm, st, 5015), "");
        assert_eq!(vm.login_reply, None);

        vm.apply_login(&world(), 3, "a_new_name");
        assert_eq!(int(vm, st, 3312), 0, "isMember is mapMembers");
        assert_eq!(int(vm, st, 3316), 2, "staffModLevel 2 and up is reported");
        assert_eq!(int(vm, st, 3318), 3, "Player.worldId");
        assert_eq!(int(vm, st, 3323), 1, "5..=9 blackmarks");
        assert_eq!(int(vm, st, 3324), 7);
        assert_eq!(int(vm, st, 3325), 1, "playerMember");
        assert_eq!(int(vm, st, 3328), 1, "underage without chat consent");
        assert_eq!(int(vm, st, 3329), 1, "mapQuickChat");
        // PlayerList.self has no username yet: Player.usernameInput as typed.
        assert_eq!(text(vm, st, 5015), "a_new_name");
        assert_eq!(vm.login_reply, Some(2));

        let mut other = world();
        other.staff_level = 1;
        other.blackmarks = 10;
        other.parental_chat_consent = true;
        other.quick_chat = false;
        other.player_member = false;
        other.map_members = true;
        vm.apply_login(&other, 1, "a_new_name");
        assert_eq!(int(vm, st, 3312), 1);
        assert_eq!(int(vm, st, 3316), 0, "staffModLevel below 2 reads 0");
        assert_eq!([3323, 3324].map(|op| int(vm, st, op)), [0, 0]);
        assert_eq!([3325, 3328, 3329].map(|op| int(vm, st, op)), [0, 0, 0]);

        // Player.decodeAppearance: decode37(name37).toTitleCase().
        vm.apply_self_name(crate::ui::social::encode("a_new_name"));
        assert_eq!(text(vm, st, 5015), "A New Name");
        // A value Base37.decode37 rejects leaves the name alone.
        vm.apply_self_name(0);
        assert_eq!(text(vm, st, 5015), "A New Name");
    }

    #[test]
    fn joining_a_clan_channel_takes_the_rank_of_the_logged_in_name() {
        let (mut state, mut vm) = setup();
        let member = |name: &str, rank| ClanMember {
            name: crate::ui::social::encode(name),
            world: 1,
            rank,
            world_name: b"World 1".to_vec(),
        };
        let channel = ServerEvent::ClanChannel {
            owner: crate::ui::social::encode("owner"),
            name: crate::ui::social::encode("the_clan"),
            min_kick: 3,
            members: Some(vec![member("zed", 1), member("a_new_name", 5)]),
        };
        // Not logged in: Player.name37 matches nobody.
        assert!(vm.apply_event(&mut state, &channel).unwrap());
        assert_eq!(int(&mut vm, &mut state, 3618), 0);
        // Player.name37 is the encoded login name, before any appearance.
        vm.apply_login(&world(), 1, "A New Name");
        assert!(vm.apply_event(&mut state, &channel).unwrap());
        assert_eq!(int(&mut vm, &mut state, 3618), 5);
    }

    #[test]
    fn varp_reset_keeps_variables_that_have_a_client_code() {
        let (state, mut vm) = setup();
        for id in [3, 4, 5] {
            vm.varps.insert(id, 10 + id);
            vm.pending_varps.server(id, 20 + id, 0);
        }
        vm.reset_varps(Some(&state));
        assert_eq!(vm.varps, [(3, 13)].into());
        assert_eq!(vm.pending_varps.values, [(3, 23)].into());
        // pendingVarpTimers is a new table: nothing becomes active later.
        assert!(vm.pending_varps.poll(&mut vm.varps, i64::MAX).is_empty());
        // Without interface assets no type is known.
        vm.reset_varps(None);
        assert!(vm.varps.is_empty() && vm.pending_varps.values.is_empty());
    }
}
