//! `Cheat.execute` (Cheat.java:111-240): the commands the client acts on
//! itself before forwarding them, and the debug switches they set.
//!
//! The original runs every text that starts with `::` (CS2 5008) and the
//! command built by `Cheat.teleport` through one list of independent tests,
//! then sends it to the server as CLIENT_CHEAT. The staff-level guard around
//! the list is commented out in the pinned client, so every player reaches
//! every command; only `::fps` asks for `client.modeWhere != 0` and only
//! `::cardmem` for the HD renderer. A command that throws (a bad number,
//! `::errortest`) leaves the script before the packet is written.
//!
//! [`execute`] is that list as data: it returns what the original does, in
//! order, and the caller applies it. `PluginRepository.ProcessCommand`, which
//! the original calls first for every command, has no counterpart here: this
//! client loads no plugins.
use crate::ui::vm::Preferences;
use rs09_game_core::collision::CollisionMap;

/// What the tests in `Cheat.execute` read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Context {
    /// `client.modeWhere`.
    pub mode_where: i32,
    /// `GlRenderer.enabled`.
    pub gl: bool,
    /// `SeqType.applyTweening`.
    pub tweening: bool,
    /// `Cheat.shiftClick`.
    pub shift_click: bool,
}

/// A part of a command this client has nothing to apply to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Missing {
    /// `::gc`: `client.unloadSoft()`, ten `System.gc()` and the chat line
    /// `mem=N MB`. There is no JVM heap to collect or measure.
    MemoryReport,
    /// `::mm`: the same before and after `Player.invalidateGlModels()`.
    MemoryCleanup,
    /// `::pcachesize`: the chat line with `PlayerAppearance`'s model cache
    /// size. Player models are not kept in such a cache.
    ModelCacheSize,
    /// `::cardmem`: `GlCleaner`'s three byte counters on standard output.
    CardMemory,
    /// `::clientjs5drop`: `client.js5NetQueue.quit()`.
    Js5Quit,
    /// `::serverjs5drop`: `client.js5NetQueue.drop()`.
    Js5Drop,
    /// `::breakcon`: `breakConnection()` on the sign link, the game socket
    /// and the JS5 queue, which makes their next operations fail.
    BreakConnection,
    /// `::replacecanvas`: `GameShell.replaceCanvas = true`.
    ReplaceCanvas,
    /// `::reloadplugins`: `PluginRepository.reloadPlugins()`.
    ReloadPlugins,
}

/// The exception that ends a command before it is forwarded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Failure {
    /// `::setparticles` with nothing after it: `substring(15)` of a
    /// 14-character string allocates a negative length.
    NegativeArraySize,
    /// `JagString.parseInt` of something that is not a decimal `int`.
    NumberFormat,
    /// `::errortest` throws a `RuntimeException` on purpose.
    ErrorTest,
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NegativeArraySize => "Client command: NegativeArraySizeException",
            Self::NumberFormat => "Client command: NumberFormatException",
            Self::ErrorTest => "Client command: RuntimeException",
        })
    }
}

/// One thing `Cheat.execute` does, in the original's order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Effect {
    NoCounterpart(Missing),
    /// `Chat.add(null, 0, text)`: a game message in the chat box.
    Message(String),
    /// `JagString.print()`: a line on standard output.
    Print(String),
    /// `::clientdrop`: `Protocol.handleConnectionLost()`.
    ConnectionLost,
    /// `::rebuild`: `client.setGameState(25)`, which builds the scene again.
    RebuildScene,
    /// `::fpson` and `::fpsoff`: `Cheat.displayFps`.
    DisplayFps(bool),
    /// `::wm0` to `::wm3`: `DisplayMode.setWindowMode(false, mode, width,
    /// height)`; the size is -1 except for full screen (mode 3, 1024x768).
    WindowMode {
        mode: i32,
        width: i32,
        height: i32,
    },
    /// `::noclip`: zero the flags of tiles 1..=102 on all four planes.
    ClearCollision,
    /// `::setparticlesN`: `Preferences.setParticles`, write the file, and
    /// clear `sentToServer`. The value is not clamped.
    Particles(i32),
    /// `::fps N`: `GameShell.setFpsTarget(N)`.
    FpsTarget(i32),
    /// `::rect_debugN`: `Cheat.rectDebug`.
    RectDebug(i32),
    /// `::qa_op_test`: `Cheat.qaOpTest = true`.
    QaOpTest,
    /// `::tween`: the new `SeqType.applyTweening`.
    Tweening(bool),
    /// `::shiftclick`: the new `Cheat.shiftClick`.
    ShiftClick(bool),
    /// The command threw: nothing after this, and no packet.
    Fail(Failure),
    /// `sendCheatPacket`: the command goes to the server.
    Forward,
}

/// The bytes of a `JagString`: one per character, as the chat code sends them.
fn bytes(text: &str) -> Vec<u8> {
    text.chars().map(|c| c as u32 as u8).collect()
}

/// `JagString.equalsIgnoreCase`: A-Z and the Latin-1 capitals 0xC0-0xDE
/// (without the multiplication sign) fold to lower case.
fn equals_ignore_case(a: &[u8], b: &[u8]) -> bool {
    let fold = |c: u8| match c {
        65..=90 | 0xc0..=0xd6 | 0xd8..=0xde => c + 32,
        _ => c,
    };
    a.len() == b.len() && a.iter().zip(b).all(|(&x, &y)| fold(x) == fold(y))
}

/// `JagString.parseInt` (`parseHexString(10)`): an optional sign, then
/// decimal digits that fit an `int`.
fn parse_int(text: &[u8]) -> Result<i32, Failure> {
    let mut negative = false;
    let mut has_digit = false;
    let mut result = 0i32;
    for (i, &c) in text.iter().enumerate() {
        if i == 0 {
            if c == b'-' {
                negative = true;
                continue;
            }
            if c == b'+' {
                continue;
            }
        }
        let mut digit = match c {
            b'0'..=b'9' => i32::from(c - b'0'),
            b'A'..=b'Z' => i32::from(c) - 55,
            b'a'..=b'z' => i32::from(c) - 87,
            _ => return Err(Failure::NumberFormat),
        };
        if digit >= 10 {
            return Err(Failure::NumberFormat);
        }
        if negative {
            digit = -digit;
        }
        let next = digit.wrapping_add(result.wrapping_mul(10));
        if next / 10 != result {
            return Err(Failure::NumberFormat);
        }
        result = next;
        has_digit = true;
    }
    if has_digit {
        Ok(result)
    } else {
        Err(Failure::NumberFormat)
    }
}

/// `JagString.substring(from)`: a start past the end is a negative array
/// size in the original.
fn substring(text: &[u8], from: usize) -> Result<&[u8], Failure> {
    text.get(from..).ok_or(Failure::NegativeArraySize)
}

/// `JagString.trim`: bytes 0 to 32 and the no-break space come off both ends.
fn trim(mut text: &[u8]) -> &[u8] {
    let blank = |c: u8| c <= 32 || c == 160;
    while let [first, rest @ ..] = text {
        if !blank(*first) {
            break;
        }
        text = rest;
    }
    while let [rest @ .., last] = text {
        if !blank(*last) {
            break;
        }
        text = rest;
    }
    text
}

/// `Cheat.execute(command)`. The command keeps its `::`; exact names match
/// in any case, the three prefixes (`::setparticles`, `::fps `,
/// `::rect_debug`) only as written.
pub fn execute(command: &str, context: &Context) -> Vec<Effect> {
    let mut effects = Vec::new();
    if let Err(failure) = run(&bytes(command), context, &mut effects) {
        effects.push(Effect::Fail(failure));
    } else {
        effects.push(Effect::Forward);
    }
    effects
}

fn run(command: &[u8], context: &Context, effects: &mut Vec<Effect>) -> Result<(), Failure> {
    let is = |name: &str| equals_ignore_case(command, name.as_bytes());
    let missing = [
        ("::gc", Missing::MemoryReport),
        ("::mm", Missing::MemoryCleanup),
        ("::pcachesize", Missing::ModelCacheSize),
    ];
    for (name, what) in missing {
        if is(name) {
            effects.push(Effect::NoCounterpart(what));
        }
    }
    if context.gl && is("::cardmem") {
        effects.push(Effect::NoCounterpart(Missing::CardMemory));
    }
    if is("::clientdrop") {
        effects.push(Effect::ConnectionLost);
    }
    let missing = [
        ("::clientjs5drop", Missing::Js5Quit),
        ("::serverjs5drop", Missing::Js5Drop),
        ("::breakcon", Missing::BreakConnection),
        ("::replacecanvas", Missing::ReplaceCanvas),
    ];
    for (name, what) in missing {
        if is(name) {
            effects.push(Effect::NoCounterpart(what));
        }
    }
    if is("::rebuild") {
        effects.push(Effect::RebuildScene);
    }
    if is("::fpson") {
        effects.push(Effect::DisplayFps(true));
    }
    if is("::fpsoff") {
        effects.push(Effect::DisplayFps(false));
    }
    for (name, mode, size) in [
        ("::wm0", 0, [-1, -1]),
        ("::wm1", 1, [-1, -1]),
        ("::wm2", 2, [-1, -1]),
        ("::wm3", 3, [1024, 768]),
    ] {
        if is(name) {
            effects.push(Effect::WindowMode {
                mode,
                width: size[0],
                height: size[1],
            });
        }
    }
    if is("::noclip") {
        effects.push(Effect::ClearCollision);
    }
    if command.starts_with(b"::setparticles") {
        effects.push(Effect::Particles(parse_int(substring(command, 15)?)?));
    }
    if command.starts_with(b"::fps ") && context.mode_where != 0 {
        effects.push(Effect::FpsTarget(parse_int(substring(command, 6)?)?));
    }
    if is("::errortest") {
        return Err(Failure::ErrorTest);
    }
    if command.starts_with(b"::rect_debug") {
        let value = parse_int(trim(substring(command, 12)?))?;
        effects.push(Effect::RectDebug(value));
        effects.push(Effect::Message(format!("rect_debug={value}")));
    }
    if is("::qa_op_test") {
        effects.push(Effect::QaOpTest);
    }
    if is("::tween") {
        effects.push(Effect::Tweening(!context.tweening));
        effects.push(Effect::Message(
            if context.tweening {
                "Forced tweening disabled."
            } else {
                "Forced tweening ENABLED!"
            }
            .into(),
        ));
    }
    if is("::shiftclick") {
        effects.push(Effect::Print(
            if context.shift_click {
                "Shift-click disabled."
            } else {
                "Shift-click ENABLED!"
            }
            .into(),
        ));
        effects.push(Effect::ShiftClick(!context.shift_click));
    }
    if is("::reloadplugins") {
        effects.push(Effect::NoCounterpart(Missing::ReloadPlugins));
    }
    Ok(())
}

/// `Cheat.teleport(tileX, tileY, plane)`: the command is printed, then run
/// like any other. Coordinates are absolute tiles.
pub fn teleport(x: i32, y: i32, plane: i32, context: &Context) -> (String, Vec<Effect>) {
    let command = rs09_protocol::game::outgoing::teleport_command(x, y, plane);
    let mut effects = vec![Effect::Print(command.clone())];
    effects.extend(execute(&command, context));
    (command, effects)
}

/// Protocol.java:2818 and 2828: a staff member holding Ctrl and Shift
/// teleports with the mouse wheel and with a walk click.
pub fn staff_teleport(staff_level: i32, ctrl: bool, shift: bool) -> bool {
    staff_level > 0 && ctrl && shift
}

/// Protocol.java:2819-2824: the plane a wheel teleport goes to.
pub fn wheel_plane(plane: i32, wheel_rotation: i32) -> i32 {
    plane.wrapping_sub(wheel_rotation).clamp(0, 3)
}

/// `::noclip` (Cheat.java:189-197): tiles 1..=102 of every plane lose all
/// their flags; the border keeps its own. A later scene build restores them.
pub fn clear_collision(maps: &mut [CollisionMap]) {
    for map in maps {
        for x in 1..103 {
            for y in 1..103 {
                // A map smaller than the original's 104 tiles has no such tile.
                let _ = map.set(x, y, 0);
            }
        }
    }
}

/// The switches `Cheat` and `SeqType` keep, with the standalone client's
/// starting values, and the effects waiting for the main loop.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct State {
    /// `client.modeWhere`: 2 when started through `client.main`.
    pub mode_where: i32,
    /// `Cheat.displayFps`.
    pub display_fps: bool,
    /// `Cheat.shiftClick`. The pinned client starts with it on
    /// (`GlobalConfig.USE_SHIFT_CLICK`, a 2009scape addition); here it starts
    /// off until the project owner decides, and `::shiftclick` turns it on.
    pub shift_click: bool,
    /// `Cheat.rectDebug`.
    pub rect_debug: i32,
    /// `Cheat.qaOpTest`.
    pub qa_op_test: bool,
    /// `SeqType.applyTweening`, from `GlobalConfig.USE_TWEENING`.
    pub tweening: bool,
    /// Effects of the commands run since the main loop last looked, oldest
    /// first. The switches above and the particle preference are already
    /// applied when an effect is queued.
    pub pending: Vec<Effect>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            mode_where: 2,
            display_fps: false,
            shift_click: false,
            rect_debug: 0,
            qa_op_test: false,
            tweening: true,
            pending: Vec::new(),
        }
    }
}
impl State {
    pub fn context(&self, gl: bool) -> Context {
        Context {
            mode_where: self.mode_where,
            gl,
            tweening: self.tweening,
            shift_click: self.shift_click,
        }
    }

    /// Apply the effects that only touch these switches or the preferences,
    /// and queue all of them. Ok when the command is to be forwarded.
    pub fn apply(
        &mut self,
        effects: Vec<Effect>,
        preferences: &mut Preferences,
    ) -> Result<(), Failure> {
        let mut result = Ok(());
        for effect in &effects {
            match *effect {
                Effect::DisplayFps(on) => self.display_fps = on,
                Effect::RectDebug(value) => self.rect_debug = value,
                Effect::QaOpTest => self.qa_op_test = true,
                Effect::Tweening(on) => self.tweening = on,
                Effect::ShiftClick(on) => self.shift_click = on,
                Effect::Particles(value) => {
                    preferences.particles = value;
                    preferences.writes = preferences.writes.wrapping_add(1);
                    preferences.sent_to_server = false;
                }
                Effect::Fail(failure) => result = Err(failure),
                _ => {}
            }
        }
        self.pending.extend(effects);
        result
    }

    /// `Cheat.execute(command)` from a script (CS2 5008).
    pub fn run(
        &mut self,
        command: &str,
        gl: bool,
        preferences: &mut Preferences,
    ) -> Result<(), Failure> {
        let effects = execute(command, &self.context(gl));
        self.apply(effects, preferences)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HD: Context = Context {
        mode_where: 2,
        gl: true,
        tweening: true,
        shift_click: true,
    };

    fn only(command: &str) -> Vec<Effect> {
        let mut effects = execute(command, &HD);
        assert_eq!(effects.pop(), Some(Effect::Forward), "{command}");
        effects
    }

    #[test]
    fn memory_and_cache_commands_have_no_counterpart() {
        assert_eq!(only("::gc"), [Effect::NoCounterpart(Missing::MemoryReport)]);
        assert_eq!(
            only("::mm"),
            [Effect::NoCounterpart(Missing::MemoryCleanup)]
        );
        assert_eq!(
            only("::pcachesize"),
            [Effect::NoCounterpart(Missing::ModelCacheSize)]
        );
        assert_eq!(
            only("::cardmem"),
            [Effect::NoCounterpart(Missing::CardMemory)]
        );
        // ::cardmem is tested only with the HD renderer on.
        let sd = Context { gl: false, ..HD };
        assert_eq!(execute("::cardmem", &sd), [Effect::Forward]);
    }

    #[test]
    fn connection_commands() {
        assert_eq!(only("::clientdrop"), [Effect::ConnectionLost]);
        assert_eq!(
            only("::clientjs5drop"),
            [Effect::NoCounterpart(Missing::Js5Quit)]
        );
        assert_eq!(
            only("::serverjs5drop"),
            [Effect::NoCounterpart(Missing::Js5Drop)]
        );
        assert_eq!(
            only("::breakcon"),
            [Effect::NoCounterpart(Missing::BreakConnection)]
        );
    }

    #[test]
    fn display_commands() {
        assert_eq!(
            only("::replacecanvas"),
            [Effect::NoCounterpart(Missing::ReplaceCanvas)]
        );
        assert_eq!(only("::rebuild"), [Effect::RebuildScene]);
        assert_eq!(only("::fpson"), [Effect::DisplayFps(true)]);
        assert_eq!(only("::fpsoff"), [Effect::DisplayFps(false)]);
        for (mode, size) in [
            (0, [-1, -1]),
            (1, [-1, -1]),
            (2, [-1, -1]),
            (3, [1024, 768]),
        ] {
            assert_eq!(
                only(&format!("::wm{mode}")),
                [Effect::WindowMode {
                    mode,
                    width: size[0],
                    height: size[1]
                }]
            );
        }
        assert_eq!(only("::wm4"), []);
    }

    #[test]
    fn names_match_in_any_case_but_not_with_anything_added() {
        assert_eq!(only("::NoClip"), [Effect::ClearCollision]);
        assert_eq!(only("::noclip "), []);
        assert_eq!(only("::REBUILD"), [Effect::RebuildScene]);
        assert_eq!(only("::tweening"), []);
        assert_eq!(only("::unknown"), []);
        assert_eq!(only("::"), []);
    }

    #[test]
    fn particles_take_any_integer_after_one_skipped_character() {
        assert_eq!(only("::setparticles 0"), [Effect::Particles(0)]);
        assert_eq!(only("::setparticles 300"), [Effect::Particles(300)]);
        assert_eq!(only("::setparticles -1"), [Effect::Particles(-1)]);
        assert_eq!(only("::setparticles +2"), [Effect::Particles(2)]);
        assert_eq!(only("::setparticles=7"), [Effect::Particles(7)]);
        assert_eq!(only("::setparticles 0002"), [Effect::Particles(2)]);
        // The prefix is matched as written.
        assert_eq!(only("::SETPARTICLES 1"), []);
        assert_eq!(
            execute("::setparticles", &HD),
            [Effect::Fail(Failure::NegativeArraySize)]
        );
        for bad in [
            "::setparticlesX",
            "::setparticles  2",
            "::setparticles 2 ",
            "::setparticles 2x",
            "::setparticles 99999999999",
            "::setparticles -",
            "::setparticles a",
        ] {
            assert_eq!(
                execute(bad, &HD),
                [Effect::Fail(Failure::NumberFormat)],
                "{bad}"
            );
        }
    }

    #[test]
    fn the_frame_rate_target_needs_a_mode_other_than_live_applet() {
        assert_eq!(only("::fps 30"), [Effect::FpsTarget(30)]);
        assert_eq!(only("::fps -5"), [Effect::FpsTarget(-5)]);
        assert_eq!(only("::fps"), []);
        assert_eq!(only("::FPS 30"), []);
        assert_eq!(
            execute("::fps x", &HD),
            [Effect::Fail(Failure::NumberFormat)]
        );
        assert_eq!(
            execute("::fps ", &HD),
            [Effect::Fail(Failure::NumberFormat)]
        );
        // With modeWhere 0 the number is never read.
        let applet = Context {
            mode_where: 0,
            ..HD
        };
        assert_eq!(execute("::fps 30", &applet), [Effect::Forward]);
        assert_eq!(execute("::fps x", &applet), [Effect::Forward]);
    }

    #[test]
    fn error_test_throws_and_is_not_forwarded() {
        assert_eq!(
            execute("::errortest", &HD),
            [Effect::Fail(Failure::ErrorTest)]
        );
        assert_eq!(
            execute("::ErrorTest", &HD),
            [Effect::Fail(Failure::ErrorTest)]
        );
        assert_eq!(only("::errortest "), []);
    }

    #[test]
    fn rect_debug_trims_its_number_and_reports_it() {
        let set = |n: i32| {
            vec![
                Effect::RectDebug(n),
                Effect::Message(format!("rect_debug={n}")),
            ]
        };
        assert_eq!(only("::rect_debug 2"), set(2));
        assert_eq!(only("::rect_debug2"), set(2));
        assert_eq!(only("::rect_debug   3  "), set(3));
        assert_eq!(only("::rect_debug -1"), set(-1));
        assert_eq!(only("::rect_debug\u{a0}1\u{a0}"), set(1));
        assert_eq!(only("::RECT_DEBUG 1"), []);
        for bad in ["::rect_debug", "::rect_debugx", "::rect_debug 1 2"] {
            assert_eq!(
                execute(bad, &HD),
                [Effect::Fail(Failure::NumberFormat)],
                "{bad}"
            );
        }
    }

    #[test]
    fn toggles_report_their_new_state() {
        assert_eq!(only("::qa_op_test"), [Effect::QaOpTest]);
        assert_eq!(
            only("::tween"),
            [
                Effect::Tweening(false),
                Effect::Message("Forced tweening disabled.".into())
            ]
        );
        assert_eq!(
            only("::shiftclick"),
            [
                Effect::Print("Shift-click disabled.".into()),
                Effect::ShiftClick(false)
            ]
        );
        let off = Context {
            tweening: false,
            shift_click: false,
            ..HD
        };
        assert_eq!(
            execute("::tween", &off)[..2],
            [
                Effect::Tweening(true),
                Effect::Message("Forced tweening ENABLED!".into())
            ]
        );
        assert_eq!(
            execute("::shiftclick", &off)[..2],
            [
                Effect::Print("Shift-click ENABLED!".into()),
                Effect::ShiftClick(true)
            ]
        );
        assert_eq!(
            only("::reloadplugins"),
            [Effect::NoCounterpart(Missing::ReloadPlugins)]
        );
    }

    #[test]
    fn teleport_prints_and_forwards_its_command() {
        let (command, effects) = teleport(3222, 3218, 0, &HD);
        assert_eq!(command, "::tele 0,50,50,22,18");
        assert_eq!(effects, [Effect::Print(command), Effect::Forward]);
        assert!(staff_teleport(1, true, true));
        assert!(!staff_teleport(0, true, true));
        assert!(!staff_teleport(2, true, false));
        assert!(!staff_teleport(2, false, true));
        assert_eq!(wheel_plane(0, 1), 0);
        assert_eq!(wheel_plane(1, 1), 0);
        assert_eq!(wheel_plane(2, -1), 3);
        assert_eq!(wheel_plane(3, -3), 3);
    }

    #[test]
    fn noclip_clears_the_inner_tiles_of_every_plane() {
        let mut maps: Vec<CollisionMap> = (0..4)
            .map(|_| CollisionMap::new(104, 104).unwrap())
            .collect();
        for map in &mut maps {
            for x in 0..104 {
                for y in 0..104 {
                    map.set(x, y, 7).unwrap();
                }
            }
        }
        clear_collision(&mut maps);
        for map in &maps {
            for x in 0..104 {
                for y in 0..104 {
                    let inner = (1..103).contains(&x) && (1..103).contains(&y);
                    assert_eq!(map.get(x, y), Some(if inner { 0 } else { 7 }));
                }
            }
        }
        // Smaller maps are cleared as far as they reach.
        clear_collision(&mut [CollisionMap::new(8, 8).unwrap()]);
    }

    #[test]
    fn state_applies_switches_and_queues_everything() {
        // The pinned client's starting switches: shift-click is on there.
        let mut state = State {
            shift_click: true,
            ..State::default()
        };
        assert!(!State::default().shift_click);
        let mut preferences = Preferences::default();
        assert_eq!(state.context(true), HD);
        for command in ["::fpson", "::tween", "::shiftclick", "::qa_op_test"] {
            state.run(command, true, &mut preferences).unwrap();
        }
        state.run("::rect_debug 3", true, &mut preferences).unwrap();
        assert!(state.display_fps && state.qa_op_test);
        assert!(!state.tweening && !state.shift_click);
        assert_eq!(state.rect_debug, 3);
        assert!(preferences.sent_to_server);
        state
            .run("::setparticles 1", true, &mut preferences)
            .unwrap();
        assert_eq!(
            (
                preferences.particles,
                preferences.writes,
                preferences.sent_to_server
            ),
            (1, 1, false)
        );
        assert_eq!(
            state.run("::errortest", true, &mut preferences),
            Err(Failure::ErrorTest)
        );
        assert_eq!(
            state.pending.last(),
            Some(&Effect::Fail(Failure::ErrorTest))
        );
        assert_eq!(
            state
                .pending
                .iter()
                .filter(|e| **e == Effect::Forward)
                .count(),
            6
        );
    }

    /// The oracle's adapter calls for one effect, digits replaced as there.
    fn events(effect: &Effect, command: &str, out: &mut Vec<String>) {
        let chat = |text: &str| {
            let mut line = String::from("chat null 0 ");
            let mut digits = false;
            for c in text.chars() {
                if c.is_ascii_digit() {
                    if !digits {
                        line.push('#');
                    }
                    digits = true;
                } else {
                    digits = false;
                    line.push(c);
                }
            }
            line
        };
        match effect {
            Effect::NoCounterpart(Missing::MemoryReport) => {
                out.extend(["unloadSoft".into(), chat("mem=0 MB")]);
            }
            Effect::NoCounterpart(Missing::MemoryCleanup) => out.extend([
                "unloadSoft".into(),
                chat("Memory before cleanup=0 MB"),
                "invalidateGlModels".into(),
                "unloadSoft".into(),
                chat("Memory after cleanup=0 MB"),
            ]),
            Effect::NoCounterpart(Missing::ModelCacheSize) => {
                out.push(chat("Number of player models in cache:0"));
            }
            Effect::NoCounterpart(Missing::CardMemory) => out.extend([
                "stdout oncard_geometry:0".into(),
                "stdout oncard_2d:0".into(),
                "stdout oncard_texture:0".into(),
            ]),
            Effect::NoCounterpart(Missing::Js5Quit) => out.push("js5.quit".into()),
            Effect::NoCounterpart(Missing::Js5Drop) => out.push("js5.drop".into()),
            Effect::NoCounterpart(Missing::BreakConnection) => out.extend([
                "signLink.breakConnection".into(),
                "socket.breakConnection".into(),
                "js5.breakConnection".into(),
            ]),
            Effect::NoCounterpart(Missing::ReloadPlugins) => out.push("reloadPlugins".into()),
            Effect::Message(text) => out.push(chat(text)),
            Effect::Print(text) => out.push(format!("stdout {text}")),
            Effect::ConnectionLost => out.push("handleConnectionLost".into()),
            Effect::RebuildScene => out.push("gameState 25".into()),
            Effect::WindowMode {
                mode,
                width,
                height,
            } => out.push(format!("windowMode false {mode} {width} {height}")),
            Effect::FpsTarget(value) => out.push(format!("fpsTarget {value}")),
            Effect::Forward => {
                let rest: String = command.chars().skip(2).collect();
                out.extend([
                    "p1isaac 44".into(),
                    format!("p1 {}", command.chars().count() as i32 - 1),
                    format!("pjstr {rest}"),
                ]);
            }
            // Fields of the original, compared after the command instead.
            Effect::NoCounterpart(Missing::ReplaceCanvas)
            | Effect::DisplayFps(_)
            | Effect::ClearCollision
            | Effect::Particles(_)
            | Effect::RectDebug(_)
            | Effect::QaOpTest
            | Effect::Tweening(_)
            | Effect::ShiftClick(_)
            | Effect::Fail(_) => {}
        }
    }

    /// Every `cheat` line of the JVM fixture: the unchanged `Cheat.java`
    /// over recording adapters. Compared are the adapter calls in order, the
    /// exception, the switches, the collision maps and the preferences file.
    #[test]
    fn execute_reproduces_the_unchanged_cheat_class() {
        let fixture = crate::preferences_file::tests::fixture();
        let mut cases = 0;
        for line in fixture.lines() {
            let case: serde_json::Value = serde_json::from_str(line).unwrap();
            if case["case"] != "cheat" {
                continue;
            }
            cases += 1;
            let flag = |name: &str| case[name].as_bool().unwrap();
            let mut state = State {
                mode_where: case["modeWhere"].as_i64().unwrap() as i32,
                tweening: flag("tween"),
                shift_click: flag("shift"),
                ..State::default()
            };
            let context = state.context(flag("gl"));
            let (command, effects) = match case["teleport"].as_array() {
                Some(tile) => {
                    let [x, y, plane] = [0, 1, 2].map(|i| tile[i].as_i64().unwrap() as i32);
                    teleport(x, y, plane, &context)
                }
                None => {
                    let command = case["command"].as_str().unwrap().to_owned();
                    let effects = execute(&command, &context);
                    (command, effects)
                }
            };
            let mut expected = vec![format!("plugins {command}")];
            for effect in &effects {
                events(effect, &command, &mut expected);
            }
            let observed: Vec<&str> = case["events"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e.as_str().unwrap())
                .collect();
            // Cheat.teleport prints before Cheat.execute tells the plugins.
            if case["teleport"].is_array() {
                expected.swap(0, 1);
            }
            assert_eq!(observed, expected, "{line}");
            let failure = effects.iter().find_map(|e| match e {
                Effect::Fail(failure) => Some(match failure {
                    Failure::NegativeArraySize => "java.lang.NegativeArraySizeException",
                    Failure::NumberFormat => "java.lang.NumberFormatException",
                    Failure::ErrorTest => "java.lang.RuntimeException",
                }),
                _ => None,
            });
            assert_eq!(case["error"].as_str(), failure, "{line}");
            assert_eq!(
                failure.is_none(),
                effects.last() == Some(&Effect::Forward),
                "{line}"
            );
            let mut preferences = crate::preferences_file::defaults(128);
            let replace = effects.contains(&Effect::NoCounterpart(Missing::ReplaceCanvas));
            let cleared = effects.contains(&Effect::ClearCollision);
            let written = effects.iter().any(|e| matches!(e, Effect::Particles(_)));
            let _ = state.apply(effects, &mut preferences);
            assert_eq!(flag("displayFps"), state.display_fps, "{line}");
            assert_eq!(flag("shiftClick"), state.shift_click, "{line}");
            assert_eq!(flag("qaOpTest"), state.qa_op_test, "{line}");
            assert_eq!(flag("tweening"), state.tweening, "{line}");
            assert_eq!(flag("replaceCanvas"), replace, "{line}");
            assert_eq!(flag("sentToServer"), preferences.sent_to_server, "{line}");
            assert_eq!(case["rectDebug"], state.rect_debug, "{line}");
            assert_eq!(case["particles"], preferences.particles, "{line}");
            assert_eq!(
                case["collision"],
                if cleared {
                    "inner 1..102 cleared"
                } else {
                    "kept"
                },
                "{line}"
            );
            // The file is empty (just opened by the read) unless written.
            let file = if written {
                crate::preferences_file::tests::hex(&crate::preferences_file::encode(&preferences))
            } else {
                String::new()
            };
            assert_eq!(case["preferencesFile"], file, "{line}");
        }
        assert_eq!(cases, 320, "cheat cases in the fixture");
    }
}
