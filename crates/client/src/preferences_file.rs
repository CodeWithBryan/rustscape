//! The original preferences file: `Preferences.read`, `decode`, `encode`,
//! `write` and `toInt` (Preferences.java:71-327).
//!
//! The file is `jagex_runescape_preferences.dat` (SignLink.java:109-121,
//! opened with the name "runescape"): a version byte, 11 when written, then
//! the fields in `encode` order. `decode` accepts versions 0 to 11, each with
//! its own length, and leaves everything untouched when the version is
//! unknown or the file is shorter than its version needs. Values are clamped
//! as the original clamps them and no further: the particle setting and the
//! music volume are whatever byte the file holds.
//!
//! The original looks for the file in `c:/rscache/`, `/rscache/`, the
//! per-user `2009scape` folder and five more places. This client keeps it in
//! its own per-user settings folder instead, next to the JSON file it used
//! before, and still reads that JSON when no binary file exists yet.
use crate::ui::vm::Preferences;
use rs09_protocol::game::outgoing::PreferenceFlags;

/// `"jagex_" + "runescape" + "_preferences.dat"`.
pub const FILE_NAME: &str = "jagex_runescape_preferences.dat";
/// The version `encode` writes.
pub const VERSION: u8 = 11;
/// `GameShell.maxMemory` for this client. The original measures the JVM's
/// heap limit in MiB and turns particles off below 96; a native process has
/// no such limit, so this is the smallest value that is not low memory.
pub const MAX_MEMORY: i32 = 96;

/// The values `Preferences.read` assigns before it opens the file. Particles
/// start at 0 with less than 96 MiB.
pub fn defaults(max_memory: i32) -> Preferences {
    Preferences {
        particles: if max_memory >= 96 { 2 } else { 0 },
        ..Preferences::default()
    }
}

/// `Preferences.read`: the defaults, then the file if there is one.
pub fn read(file: Option<&[u8]>, max_memory: i32) -> Preferences {
    let mut preferences = defaults(max_memory);
    if let Some(data) = file {
        decode(&mut preferences, data, max_memory);
    }
    preferences
}

/// `Preferences.decode`: `data` is the whole file. Fields a version does not
/// carry keep their current values.
pub fn decode(p: &mut Preferences, data: &[u8], max_memory: i32) {
    let Some((&version, rest)) = data.split_first() else {
        return;
    };
    let length = match version {
        11 => 33,
        10 => 32,
        9 => 31,
        8 => 30,
        7 => 29,
        5 | 6 => 28,
        4 => 24,
        1 | 3 => 23,
        2 => 22,
        0 => 19,
        _ => return,
    };
    if rest.len() < length {
        return;
    }
    let mut bytes = rest.iter().map(|&b| i32::from(b));
    // The length check above covers every read below.
    let mut g1 = || bytes.next().unwrap_or(0);
    p.brightness = g1().clamp(1, 4);
    p.all_levels_visible = g1() == 1;
    p.remove_roofs_selectively = g1() == 1;
    p.show_ground_decorations = g1() == 1;
    p.high_detail_textures = g1() == 1;
    p.many_idle_animations = g1() == 1;
    p.flickering_effects = g1() == 1;
    p.many_ground_textures = g1() == 1;
    p.character_shadows = g1() == 1;
    p.scenery_shadows = g1().min(2);
    p.high_detail_lighting = g1() == 1;
    if version < 2 {
        g1();
    }
    p.high_water_detail = g1() == 1;
    p.fog = g1() == 1;
    // The byte the original calls windowMode is also its anti-aliasing mode.
    p.window_mode = g1().min(2);
    p.anti_aliasing = p.window_mode;
    p.stereo = g1() == 1;
    p.sound_volume = g1().min(127);
    p.music_volume = g1();
    p.ambient_volume = g1().min(127);
    if version >= 1 {
        p.full_screen_width = (g1() << 8) + g1();
        p.full_screen_height = (g1() << 8) + g1();
    }
    if (3..6).contains(&version) {
        g1();
    }
    if version >= 4 {
        let particles = g1();
        p.particles = if max_memory < 96 { 0 } else { particles };
    }
    if version >= 5 {
        p.last_world = (g1() << 24) + (g1() << 16) + (g1() << 8) + g1();
    }
    if version >= 6 {
        p.remembered_mode = g1();
    }
    if version >= 7 {
        p.safe_mode = g1() == 1;
    }
    if version >= 8 {
        p.keyboard_camera = g1() == 1;
    }
    if version >= 9 {
        p.build_area = g1();
    }
    if version >= 10 {
        p.hdr = g1() != 0;
    }
    if version >= 11 {
        p.cursors = g1() != 0;
    }
}

/// `Preferences.encode`: version 11 and the fields, each cut to its width
/// (`p1` keeps the low byte, `p2` the low 16 bits).
pub fn encode(p: &Preferences) -> [u8; 34] {
    let width = p.full_screen_width;
    let height = p.full_screen_height;
    let world = p.last_world.to_be_bytes().map(i32::from);
    [
        i32::from(VERSION),
        p.brightness,
        i32::from(p.all_levels_visible),
        i32::from(p.remove_roofs_selectively),
        i32::from(p.show_ground_decorations),
        i32::from(p.high_detail_textures),
        i32::from(p.many_idle_animations),
        i32::from(p.flickering_effects),
        i32::from(p.many_ground_textures),
        i32::from(p.character_shadows),
        p.scenery_shadows,
        i32::from(p.high_detail_lighting),
        i32::from(p.high_water_detail),
        i32::from(p.fog),
        p.window_mode,
        i32::from(p.stereo),
        p.sound_volume,
        p.music_volume,
        p.ambient_volume,
        width >> 8,
        width,
        height >> 8,
        height,
        p.particles,
        world[0],
        world[1],
        world[2],
        world[3],
        p.remembered_mode,
        i32::from(p.safe_mode),
        i32::from(p.keyboard_camera),
        p.build_area,
        i32::from(p.hdr),
        i32::from(p.cursors),
    ]
    .map(|value| value as u8)
}

impl Preferences {
    /// The inputs of `Preferences.toInt`.
    pub fn flags(&self) -> PreferenceFlags {
        PreferenceFlags {
            brightness: self.brightness,
            all_levels_visible: self.all_levels_visible,
            remove_roofs_selectively: self.remove_roofs_selectively,
            show_ground_decorations: self.show_ground_decorations,
            high_detail_textures: self.high_detail_textures,
            many_idle_animations: self.many_idle_animations,
            flickering_effects: self.flickering_effects,
            many_ground_textures: self.many_ground_textures,
            character_shadows: self.character_shadows,
            scenery_shadows: self.scenery_shadows,
            high_detail_lighting: self.high_detail_lighting,
            high_water_detail: self.high_water_detail,
            fog: self.fog,
            stereo: self.stereo,
            sound_volume: self.sound_volume,
            music_volume: self.music_volume,
            ambient_volume: self.ambient_volume,
            particles: self.particles,
        }
    }

    /// `Preferences.toInt()`: the value the login block and packet 98 carry.
    pub fn to_int(&self) -> i32 {
        self.flags().to_int()
    }
}

/// `client.mainLoad` at state 150 (client.java:1674-1681): a safe mode still
/// set from the previous run means that run never reached the title screen,
/// so the display choices go back to their safest values. Safe mode is then
/// set until [`title_shown`] clears it, and the file is written.
pub fn main_load(p: &mut Preferences) {
    if p.safe_mode {
        p.window_mode = 0;
        p.anti_aliasing = 0;
        p.remembered_mode = 0;
        p.build_area = 0;
    }
    p.safe_mode = true;
    p.writes = p.writes.wrapping_add(1);
}

/// `client.mainRedraw` (client.java:926-929), for a frame drawn at the title
/// screen with a top-level interface open: safe mode ends and the file is
/// written. True when that happened.
pub fn title_shown(p: &mut Preferences) -> bool {
    if !p.safe_mode {
        return false;
    }
    p.safe_mode = false;
    p.writes = p.writes.wrapping_add(1);
    true
}

/// `Preferences.write` over a file that holds `existing`: the encoded bytes
/// at its start, and whatever lay past them kept (a `RandomAccessFile` is
/// written in place and never shortened).
pub fn written_over(existing: &[u8], preferences: &Preferences) -> Vec<u8> {
    let encoded = encode(preferences);
    let mut file = encoded.to_vec();
    file.extend(existing.iter().skip(encoded.len()));
    file
}

/// Where the preferences are kept on disk.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Store {
    /// The original binary file; the only one written.
    pub binary: std::path::PathBuf,
    /// This client's earlier JSON file, read when `binary` holds nothing.
    pub json: Option<std::path::PathBuf>,
}

/// What [`Store::load`] found.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Source {
    Binary,
    Json,
    /// Neither file exists or holds anything: the defaults.
    Defaults,
}

#[cfg(not(target_arch = "wasm32"))]
impl Store {
    /// `--preferences PATH` names the binary file, or the earlier JSON file
    /// when it ends in `.json` (the binary file is then its neighbour).
    /// Without it both files are in `rs09-client` under the per-user
    /// application data folder; None when that folder is unknown too.
    pub fn locate(
        option: Option<std::path::PathBuf>,
        app_data: Option<std::ffi::OsString>,
    ) -> Option<Self> {
        match option {
            Some(path) if path.extension().is_some_and(|e| e == "json") => Some(Self {
                binary: path.with_file_name(FILE_NAME),
                json: Some(path),
            }),
            Some(path) => Some(Self {
                binary: path,
                json: None,
            }),
            None => app_data.map(|folder| {
                let folder = std::path::PathBuf::from(folder).join("rs09-client");
                Self {
                    binary: folder.join(FILE_NAME),
                    json: Some(folder.join("preferences.json")),
                }
            }),
        }
    }

    /// `Preferences.read`. A missing, empty or unreadable binary file leaves
    /// the defaults, as in the original; only then is the JSON file tried,
    /// and a JSON file that does not parse is reported and ignored.
    pub fn load(&self, max_memory: i32) -> (Preferences, Source, Option<String>) {
        match std::fs::read(&self.binary) {
            Ok(bytes) if !bytes.is_empty() => {
                return (read(Some(&bytes), max_memory), Source::Binary, None);
            }
            _ => {}
        }
        let json = self.json.as_ref().and_then(|path| std::fs::read(path).ok());
        match json.as_deref().map(Preferences::from_json) {
            Some(Ok(preferences)) => (preferences, Source::Json, None),
            Some(Err(error)) => (defaults(max_memory), Source::Defaults, Some(error)),
            None => (defaults(max_memory), Source::Defaults, None),
        }
    }

    /// `Preferences.write`: the 34 bytes go to the start of the file. Like
    /// the original's `RandomAccessFile`, this does not shorten a longer
    /// file; `decode` never reads past what its version needs.
    pub fn save(&self, preferences: &Preferences) -> std::io::Result<()> {
        use std::io::Write;
        if let Some(folder) = self.binary.parent() {
            std::fs::create_dir_all(folder)?;
        }
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.binary)?
            .write_all(&encode(preferences))
    }
}

/// The browser's stand-in for the cache directory's file: the same bytes,
/// kept under the file's name in the page's local storage
/// (`web/visual/cache_storage.js`). The applet keeps them in its cache
/// directory; a page has no files.
#[cfg(target_arch = "wasm32")]
mod browser {
    unsafe extern "C" {
        fn rs09_prefs_length() -> i32;
        fn rs09_prefs_read(ptr: *mut u8, len: usize) -> i32;
        fn rs09_prefs_write(ptr: *const u8, len: usize) -> i32;
    }

    /// The stored bytes; None when there are none or storage is refused.
    pub fn read() -> Option<Vec<u8>> {
        // SAFETY: plain calls into the page glue, which copies at most `len`
        // bytes into the buffer it is given.
        unsafe {
            let length = usize::try_from(rs09_prefs_length()).ok()?;
            let mut bytes = vec![0; length];
            (rs09_prefs_read(bytes.as_mut_ptr(), length) == 0).then_some(bytes)
        }
    }

    pub fn write(bytes: &[u8]) -> bool {
        // SAFETY: the glue copies the bytes out before returning.
        unsafe { rs09_prefs_write(bytes.as_ptr(), bytes.len()) == 0 }
    }
}

/// Where the browser keeps the preferences: [`browser`].
#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Store {
    /// Names the stored file in messages.
    pub binary: std::path::PathBuf,
}

#[cfg(target_arch = "wasm32")]
impl Store {
    /// A page has one place: its own storage. The arguments the desktop's
    /// form takes mean nothing there.
    pub fn locate(
        _option: Option<std::path::PathBuf>,
        _app_data: Option<std::ffi::OsString>,
    ) -> Option<Self> {
        Some(Self {
            binary: format!("the browser's local storage ({FILE_NAME})").into(),
        })
    }

    /// `Preferences.read`: the defaults, then the stored file if there is
    /// one. There is no earlier JSON file in a browser.
    pub fn load(&self, max_memory: i32) -> (Preferences, Source, Option<String>) {
        match browser::read() {
            Some(bytes) if !bytes.is_empty() => {
                (read(Some(&bytes), max_memory), Source::Binary, None)
            }
            _ => (defaults(max_memory), Source::Defaults, None),
        }
    }

    /// `Preferences.write`, as the desktop writes the file.
    pub fn save(&self, preferences: &Preferences) -> std::io::Result<()> {
        let file = written_over(&browser::read().unwrap_or_default(), preferences);
        if browser::write(&file) {
            Ok(())
        } else {
            Err(std::io::Error::other("the browser refused to store them"))
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::Value;

    /// `fixtures/preferences/oracle.jsonl`, written by
    /// `cargo xtask preferences-oracle` from the unchanged Java classes.
    pub(crate) fn fixture() -> String {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/preferences/oracle.jsonl"
        );
        std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("{path}: {e}; run cargo xtask preferences-oracle"))
            .replace('\r', "")
    }

    pub(crate) fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    fn unhex(text: &str) -> Vec<u8> {
        (0..text.len() / 2)
            .map(|i| u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).unwrap())
            .collect()
    }

    /// The fixture's state order, as its `layout` line names it.
    const STATE: &str = "brightness,allLevelsVisible,removeRoofsSelectively,showGroundDecorations,\
        highDetailTextures,manyIdleAnimations,flickeringEffectsOn,manyGroundTextures,\
        characterShadowsOn,sceneryShadowsType,highDetailLighting,highWaterDetail,fogEnabled,\
        windowMode,antiAliasingMode,stereo,soundEffectVolume,musicVolume,ambientSoundsVolume,\
        fullScreenWidth,fullScreenHeight,particles,lastWorldId,favoriteWorlds,safeMode,\
        keyboardCameraEnabled,buildArea,hdr,cursorsEnabled";

    fn state(p: &Preferences) -> Vec<i32> {
        vec![
            p.brightness,
            i32::from(p.all_levels_visible),
            i32::from(p.remove_roofs_selectively),
            i32::from(p.show_ground_decorations),
            i32::from(p.high_detail_textures),
            i32::from(p.many_idle_animations),
            i32::from(p.flickering_effects),
            i32::from(p.many_ground_textures),
            i32::from(p.character_shadows),
            p.scenery_shadows,
            i32::from(p.high_detail_lighting),
            i32::from(p.high_water_detail),
            i32::from(p.fog),
            p.window_mode,
            p.anti_aliasing,
            i32::from(p.stereo),
            p.sound_volume,
            p.music_volume,
            p.ambient_volume,
            p.full_screen_width,
            p.full_screen_height,
            p.particles,
            p.last_world,
            p.remembered_mode,
            i32::from(p.safe_mode),
            i32::from(p.keyboard_camera),
            p.build_area,
            i32::from(p.hdr),
            i32::from(p.cursors),
        ]
    }

    fn from_state(value: &Value) -> Preferences {
        let s: Vec<i32> = value
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_i64().unwrap() as i32)
            .collect();
        assert_eq!(s.len(), 29);
        Preferences {
            brightness: s[0],
            all_levels_visible: s[1] == 1,
            remove_roofs_selectively: s[2] == 1,
            show_ground_decorations: s[3] == 1,
            high_detail_textures: s[4] == 1,
            many_idle_animations: s[5] == 1,
            flickering_effects: s[6] == 1,
            many_ground_textures: s[7] == 1,
            character_shadows: s[8] == 1,
            scenery_shadows: s[9],
            high_detail_lighting: s[10] == 1,
            high_water_detail: s[11] == 1,
            fog: s[12] == 1,
            window_mode: s[13],
            anti_aliasing: s[14],
            stereo: s[15] == 1,
            sound_volume: s[16],
            music_volume: s[17],
            ambient_volume: s[18],
            full_screen_width: s[19],
            full_screen_height: s[20],
            particles: s[21],
            last_world: s[22],
            remembered_mode: s[23],
            safe_mode: s[24] == 1,
            keyboard_camera: s[25] == 1,
            build_area: s[26],
            hdr: s[27] == 1,
            cursors: s[28] == 1,
            ..Preferences::default()
        }
    }

    fn check(case: &Value, p: &Preferences, line: &str) {
        let expected: Vec<i32> = case["after"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_i64().unwrap() as i32)
            .collect();
        assert_eq!(state(p), expected, "{line}");
        assert_eq!(hex(&encode(p)), case["encoded"].as_str().unwrap(), "{line}");
        assert_eq!(i64::from(p.to_int()), case["toInt"], "{line}");
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let folder =
            std::env::temp_dir().join(format!("rs09-preferences-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        folder
    }

    /// Every preference line of the JVM fixture: the unchanged
    /// `Preferences.java` reading real files, decoding seeded bytes of every
    /// version over default and scrambled states, and writing over files of
    /// other lengths, with `encode` and `toInt` observed after each.
    #[test]
    fn the_unchanged_preferences_class_is_reproduced() {
        let folder = scratch("oracle");
        let store = Store {
            binary: folder.join(FILE_NAME),
            json: None,
        };
        let mut counts = std::collections::BTreeMap::<String, usize>::new();
        let mut versions = [0usize; 12];
        for line in fixture().lines() {
            let case: Value = serde_json::from_str(line).unwrap();
            let kind = case["case"].as_str().unwrap();
            *counts.entry(kind.to_owned()).or_default() += 1;
            let memory = case["maxMemory"].as_i64().unwrap_or(0) as i32;
            match kind {
                "layout" => assert_eq!(case["state"], STATE.replace(' ', "")),
                "initial" => {
                    // The class's own initial values are the defaults.
                    check(&case, &Preferences::default(), line);
                    assert_eq!(case["sentToServer"], Preferences::default().sent_to_server);
                }
                "read" => {
                    let file = case["file"].as_str().map(unhex);
                    let _ = std::fs::remove_file(&store.binary);
                    if let Some(bytes) = &file {
                        std::fs::write(&store.binary, bytes).unwrap();
                    }
                    check(&case, &read(file.as_deref(), memory), line);
                    // Through the file system: an absent or empty file, or a
                    // location that cannot be opened, leaves the defaults.
                    let (loaded, source, warning) = store.load(memory);
                    check(&case, &loaded, line);
                    assert_eq!(warning, None);
                    assert_eq!(
                        source == Source::Binary,
                        file.is_some_and(|f| !f.is_empty()),
                        "{line}"
                    );
                }
                "decode" => {
                    let mut p = from_state(&case["before"]);
                    let data = unhex(case["data"].as_str().unwrap());
                    if let Some(&version) = data.first()
                        && usize::from(version) < 12
                        && state(&p) != from_after(&case)
                    {
                        versions[usize::from(version)] += 1;
                    }
                    decode(&mut p, &data, memory);
                    check(&case, &p, line);
                }
                "write" => {
                    let _ = std::fs::remove_file(&store.binary);
                    if let Some(before) = case["file"].as_str() {
                        std::fs::write(&store.binary, unhex(before)).unwrap();
                    }
                    let p = from_state(&case["state"]);
                    store.save(&p).unwrap();
                    assert_eq!(
                        hex(&std::fs::read(&store.binary).unwrap()),
                        case["fileAfter"].as_str().unwrap(),
                        "{line}"
                    );
                }
                "cheat" => {}
                other => panic!("unknown fixture case {other}"),
            }
        }
        let _ = std::fs::remove_dir_all(&folder);
        assert_eq!(counts["layout"], 1);
        assert_eq!(counts["initial"], 1);
        assert_eq!(counts["read"], 192);
        assert_eq!(counts["decode"], 1000);
        assert_eq!(counts["write"], 48);
        // Every version changed the state in several cases.
        assert!(versions.iter().all(|&n| n >= 10), "{versions:?}");
    }

    fn from_after(case: &Value) -> Vec<i32> {
        case["after"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_i64().unwrap() as i32)
            .collect()
    }

    #[test]
    fn each_version_reads_its_own_fields() {
        // Version 0: 19 bytes, with a spare byte after the lighting flag.
        let mut v0 = vec![
            0, 9, 0, 0, 0, 0, 0, 0, 0, 0, 5, 1, 77, 0, 0, 3, 0, 200, 9, 200,
        ];
        let p = read(Some(&v0), 128);
        assert_eq!((p.brightness, p.scenery_shadows), (4, 2));
        assert!(p.high_detail_lighting && !p.high_water_detail && !p.fog);
        assert_eq!((p.window_mode, p.anti_aliasing), (2, 2));
        assert_eq!(
            (p.sound_volume, p.music_volume, p.ambient_volume),
            (127, 9, 127)
        );
        // What version 0 does not carry stays at its default.
        assert_eq!((p.particles, p.cursors, p.last_world), (2, true, 0));
        // One byte short: nothing is read.
        v0.pop();
        assert_eq!(read(Some(&v0), 128), defaults(128));
        // Version 12 and an empty file are ignored.
        assert_eq!(read(Some(&[12; 40]), 128), defaults(128));
        assert_eq!(read(Some(&[]), 128), defaults(128));
        assert_eq!(read(None, 64).particles, 0);
        // Version 11 round trip of non-default values.
        let saved = Preferences {
            brightness: 1,
            fog: false,
            full_screen_width: 1920,
            full_screen_height: 1080,
            particles: 1,
            last_world: 0x0102_0304,
            remembered_mode: 2,
            safe_mode: true,
            keyboard_camera: true,
            build_area: 3,
            hdr: true,
            cursors: false,
            window_mode: 1,
            anti_aliasing: 1,
            ..Preferences::default()
        };
        let bytes = encode(&saved);
        assert_eq!(bytes.len(), 34);
        assert_eq!(bytes[0], VERSION);
        assert_eq!(read(Some(&bytes), 128), saved);
        // Low memory forces particles off when the file is read.
        assert_eq!(read(Some(&bytes), 95).particles, 0);
    }

    #[test]
    fn safe_mode_resets_display_choices_after_a_failed_start() {
        let mut p = Preferences {
            window_mode: 2,
            anti_aliasing: 2,
            remembered_mode: 2,
            build_area: 4,
            ..Preferences::default()
        };
        // A normal start: safe mode is set while loading and the file written.
        main_load(&mut p);
        assert!(p.safe_mode);
        assert_eq!((p.window_mode, p.remembered_mode, p.build_area), (2, 2, 4));
        assert_eq!(p.writes, 1);
        // The title screen appears: cleared and written once.
        assert!(title_shown(&mut p));
        assert!(!title_shown(&mut p));
        assert!(!p.safe_mode);
        assert_eq!(p.writes, 2);
        // A start that never got that far leaves the flag in the file.
        main_load(&mut p);
        let mut next = read(Some(&encode(&p)), 128);
        main_load(&mut next);
        assert_eq!(
            (
                next.window_mode,
                next.anti_aliasing,
                next.remembered_mode,
                next.build_area
            ),
            (0, 0, 0, 0)
        );
        assert!(next.safe_mode);
    }

    #[test]
    fn the_store_prefers_the_binary_file_and_falls_back_to_json() {
        let folder = scratch("store");
        let store = Store::locate(None, Some(folder.clone().into_os_string())).unwrap();
        assert_eq!(store.binary, folder.join("rs09-client").join(FILE_NAME));
        assert_eq!(
            store.json,
            Some(folder.join("rs09-client").join("preferences.json"))
        );
        assert_eq!(Store::locate(None, None), None);
        let named = Store::locate(Some(folder.join("old.json")), None).unwrap();
        assert_eq!(named.binary, folder.join(FILE_NAME));
        assert_eq!(named.json, Some(folder.join("old.json")));
        let direct = Store::locate(Some(folder.join("p.dat")), None).unwrap();
        assert_eq!((direct.binary, direct.json), (folder.join("p.dat"), None));
        // Nothing on disk: the defaults.
        assert_eq!(store.load(128), (defaults(128), Source::Defaults, None));
        // Only the earlier JSON file: its settings are kept.
        std::fs::create_dir_all(folder.join("rs09-client")).unwrap();
        let old = Preferences {
            brightness: 2,
            music_volume: 40,
            ..Preferences::default()
        };
        std::fs::write(store.json.as_ref().unwrap(), old.to_json()).unwrap();
        assert_eq!(store.load(128), (old.clone(), Source::Json, None));
        // Saving writes the binary file, which is read from then on.
        let mut changed = old.clone();
        changed.fog = false;
        store.save(&changed).unwrap();
        assert_eq!(std::fs::read(&store.binary).unwrap(), encode(&changed));
        assert_eq!(store.load(128), (changed, Source::Binary, None));
        // A broken JSON file is reported, not fatal.
        std::fs::remove_file(&store.binary).unwrap();
        std::fs::write(store.json.as_ref().unwrap(), b"{").unwrap();
        let (loaded, source, warning) = store.load(128);
        assert_eq!((loaded, source), (defaults(128), Source::Defaults));
        assert!(warning.is_some());
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// The browser keeps the bytes the desktop's file would hold after the
    /// same writes: `written_over` against `Store::save` on a real file,
    /// for no file, a file shorter than the 34 bytes and one longer.
    #[test]
    fn the_browser_bytes_are_the_files_bytes() {
        let folder = scratch("browser-bytes");
        std::fs::create_dir_all(&folder).unwrap();
        let store = Store::locate(Some(folder.join("p.dat")), None).unwrap();
        let mut preferences = defaults(128);
        preferences.brightness = 4;
        preferences.music_volume = 64;
        preferences.remembered_mode = 2;
        for existing in [vec![], vec![7; 5], (0..60).collect::<Vec<u8>>()] {
            if existing.is_empty() {
                let _ = std::fs::remove_file(&store.binary);
            } else {
                std::fs::write(&store.binary, &existing).unwrap();
            }
            store.save(&preferences).unwrap();
            let file = std::fs::read(&store.binary).unwrap();
            assert_eq!(written_over(&existing, &preferences), file);
            assert_eq!(file.len(), existing.len().max(34));
            // And they read back as what was written.
            assert_eq!(read(Some(&file), 128), preferences);
        }
        let _ = std::fs::remove_dir_all(&folder);
    }
}
