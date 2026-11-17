//! CS2 commands outside the component and core sets: container totals,
//! enum queries, arithmetic, string formatting and definition parameters.
//! Each arm is a transcription of the pinned RT4 `ScriptRunner` branch for
//! that opcode; stack order follows the original's `isp`/`ssp` arithmetic.
use super::{Result, Stack, Vm};
use crate::ui::{HookArg, latin1, state::State, targeting::ParamType};
use rs09_protocol::buffer::Reader;
use std::collections::BTreeMap;

/// A request the script VM cannot carry out itself; the client drains these.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MusicRequest {
    Song(i32),
    Jingle { id: i32, delay: i32 },
}

/// `Preferences`, with the original defaults. Scripts read and write these;
/// the renderer and audio output decide what each one changes.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub brightness: i32,
    pub all_levels_visible: bool,
    pub remove_roofs_selectively: bool,
    pub never_remove_roofs: bool,
    pub show_ground_decorations: bool,
    pub high_detail_textures: bool,
    pub many_idle_animations: bool,
    pub flickering_effects: bool,
    pub many_ground_textures: bool,
    pub character_shadows: bool,
    pub scenery_shadows: i32,
    pub high_detail_lighting: bool,
    pub high_water_detail: bool,
    pub fog: bool,
    pub anti_aliasing: i32,
    pub stereo: bool,
    pub sound_volume: i32,
    pub music_volume: i32,
    pub ambient_volume: i32,
    pub particles: i32,
    pub window_mode: i32,
    pub cursors: bool,
    pub keyboard_camera: bool,
    /// The original's misnamed `favoriteWorlds`: the remembered window mode.
    pub remembered_mode: i32,
    pub last_world: i32,
    /// The display mode of the last full-screen session; 0 when none.
    pub full_screen_width: i32,
    pub full_screen_height: i32,
    /// Set while the client loads and cleared at the title screen, so a
    /// start that fails leaves it in the file.
    pub safe_mode: bool,
    pub build_area: i32,
    pub hdr: bool,
    /// Cleared by every change the original reports to the server.
    #[serde(skip, default = "sent")]
    pub sent_to_server: bool,
    /// Bumped wherever the original calls `Preferences.write`, so the client
    /// can notice and write the file.
    #[serde(skip)]
    pub writes: u32,
}
fn sent() -> bool {
    true
}
impl Preferences {
    /// A saved file is untrusted: every field is brought back into the range
    /// its setter allows.
    pub fn sanitized(mut self) -> Self {
        let mode = |value: i32| if (0..=2).contains(&value) { value } else { 0 };
        self.brightness = self.brightness.clamp(1, 4);
        self.scenery_shadows = mode(self.scenery_shadows);
        self.anti_aliasing = mode(self.anti_aliasing);
        self.sound_volume = self.sound_volume.clamp(0, 127);
        self.music_volume = self.music_volume.clamp(0, 255);
        self.ambient_volume = self.ambient_volume.clamp(0, 127);
        self.particles = self.particles.clamp(0, 2);
        self.window_mode = mode(self.window_mode);
        self.remembered_mode = mode(self.remembered_mode);
        self
    }
    /// Missing or unknown fields fall back to the original defaults.
    pub fn from_json(bytes: &[u8]) -> std::result::Result<Self, String> {
        serde_json::from_slice::<Self>(bytes)
            .map(Self::sanitized)
            .map_err(|e| e.to_string())
    }
    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(self).expect("preferences are plain values")
    }
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            brightness: 3,
            all_levels_visible: true,
            remove_roofs_selectively: true,
            never_remove_roofs: false,
            show_ground_decorations: true,
            high_detail_textures: true,
            many_idle_animations: true,
            flickering_effects: true,
            many_ground_textures: true,
            character_shadows: true,
            scenery_shadows: 2,
            high_detail_lighting: true,
            high_water_detail: true,
            fog: true,
            anti_aliasing: 0,
            stereo: true,
            sound_volume: 127,
            music_volume: 255,
            ambient_volume: 127,
            particles: 2,
            window_mode: 0,
            cursors: true,
            keyboard_camera: false,
            remembered_mode: 0,
            last_world: 0,
            full_screen_width: 0,
            full_screen_height: 0,
            safe_mode: false,
            build_area: 0,
            hdr: false,
            sent_to_server: true,
            writes: 0,
        }
    }
}

/// Script-set viewport limits (CS2 6200 to 6202), with the original defaults
/// applied to non-positive arguments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Viewport {
    pub near_zoom: i32,
    pub far_zoom: i32,
    pub min_view_distance: i32,
    pub max_view_distance: i32,
    pub min_zoom: i32,
    pub max_zoom: i32,
    pub min_fov: i32,
    pub max_fov: i32,
}
impl Default for Viewport {
    fn default() -> Self {
        Self {
            near_zoom: 256,
            far_zoom: 205,
            min_view_distance: 256,
            max_view_distance: 320,
            min_zoom: 1,
            max_zoom: 32767,
            min_fov: 1,
            max_fov: 32767,
        }
    }
}

/// Camera commands from scripts. Coordinates are absolute tiles; the client
/// converts them to the scene and applies them to its camera.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CameraRequest {
    MoveTo {
        tile: [i32; 2],
        height: i32,
        min_step: i32,
        speed: i32,
    },
    LookAt {
        tile: [i32; 2],
        height: i32,
        min_step: i32,
        speed: i32,
    },
    Unlock,
    Angle {
        pitch: i32,
        yaw: i32,
    },
    /// 5502: fly along one stored path while looking along another. Each is
    /// the four control points `[x, height, y]` (absolute fine units) of the
    /// chosen segment, taken when the command runs.
    Spline {
        position: [[i32; 3]; 4],
        look: [[i32; 3]; 4],
        speed_start: i32,
        speed_end: i32,
    },
}

/// `JagString.toLowerCase`: ASCII and Latin-1 capitals except the
/// multiplication sign.
fn lower(text: &str) -> String {
    text.chars()
        .map(|c| match c as u32 {
            code @ (65..=90 | 192..=214 | 216..=222) => char::from_u32(code + 32).unwrap_or(c),
            _ => c,
        })
        .collect()
}
/// `JagString.compareTo`: unsigned bytes, then the shorter string first.
fn compare_to(a: &str, b: &str) -> std::cmp::Ordering {
    a.chars().map(|c| c as u32).cmp(b.chars().map(|c| c as u32))
}
/// `Find.quicksort`, kept verbatim: its comparison alternates between `<` and
/// `<=` on odd indices, so equal names do not land where a stable sort puts them.
fn find_sort(ids: &mut [i32], names: &mut [String], left: isize, right: isize) {
    if right <= left {
        return;
    }
    let (l, r) = (left as usize, right as usize);
    let mut store = l;
    let mid = (l + r) / 2;
    names.swap(mid, r);
    ids.swap(mid, r);
    let pivot = names[r].clone();
    for i in l..r {
        let order = match compare_to(&names[i], &pivot) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        };
        if order < (i & 1) as i32 {
            names.swap(i, store);
            ids.swap(i, store);
            store += 1;
        }
    }
    names.swap(r, store);
    ids.swap(r, store);
    find_sort(ids, names, left, store as isize - 1);
    find_sort(ids, names, store as isize + 1, right);
}

/// `QuickChatPhraseType.getDynamicCommand`: -1 with no commands or an index
/// outside 0..=length; an index equal to the length throws in the original.
fn dynamic_command(phrase: &crate::ui::quickchat::PhraseType, index: i32) -> Result<i32> {
    match &phrase.commands {
        Some(list) if index >= 0 && index as usize <= list.len() => list
            .get(index as usize)
            .map(|(kind, _)| *kind)
            .ok_or_else(|| "Quick chat command index equals the list length".to_string()),
        _ => Ok(-1),
    }
}

/// Days from 1970-01-01 to a proleptic Gregorian date.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year.rem_euclid(400);
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn inv_size(state: &State, id: i32) -> Result<i32> {
    let mut r = Reader::new(state.pack.get(&format!("invtype/{id}"))?);
    let mut size = 0;
    loop {
        match r.g1().map_err(|e| e.to_string())? {
            0 => return Ok(size),
            2 => size = r.g2().map_err(|e| e.to_string())?,
            _ => {}
        }
    }
}

/// An enum's declared value type and every key/value pair, in file order.
fn enum_table(state: &State, id: i32) -> Result<(i32, Vec<(i32, HookArg)>)> {
    let mut r = Reader::new(state.pack.get(&format!("enum/{id}"))?);
    let mut value_type = 0;
    let mut table = vec![];
    let e = |e: rs09_protocol::Error| e.to_string();
    loop {
        match r.g1().map_err(e)? {
            0 => return Ok((value_type, table)),
            1 => {
                r.g1().map_err(e)?;
            }
            2 => value_type = r.g1().map_err(e)?,
            3 => {
                r.gjstr().map_err(e)?;
            }
            4 => {
                r.g4().map_err(e)?;
            }
            op @ (5 | 6) => {
                // Each table opcode replaces the previous table.
                table.clear();
                for _ in 0..r.g2().map_err(e)? {
                    let key = r.g4().map_err(e)?;
                    table.push((
                        key,
                        if op == 5 {
                            HookArg::Text(latin1(r.gjstr().map_err(e)?))
                        } else {
                            HookArg::Int(r.g4().map_err(e)?)
                        },
                    ));
                }
            }
            _ => {}
        }
    }
}

fn param_type(state: &State, id: i32) -> Result<ParamType> {
    let name = format!("param/{id}");
    if state.pack.manifest.files.contains_key(&name) {
        ParamType::decode(state.pack.get(&name)?).map_err(|e| e.to_string())
    } else if state
        .pack
        .manifest
        .files
        .contains_key("param-catalog-complete")
    {
        Ok(ParamType::default())
    } else {
        Err(format!("Missing parameter metadata {id}"))
    }
}

/// A definition's raw bytes, or None where the complete catalogue has no such
/// ID and the original would hand back a default definition.
fn definition<'a>(state: &'a State, family: &str, id: i32) -> Result<Option<&'a [u8]>> {
    let name = format!("{family}/{id}");
    if state.pack.manifest.files.contains_key(&name) {
        return state.pack.get(&name).map(Some);
    }
    let complete = match family {
        "npc" => "npc-catalog-complete",
        _ => "param-catalog-complete",
    };
    if state.pack.manifest.files.contains_key(complete) {
        Ok(None)
    } else {
        Err(format!("Missing {family} definition {id}"))
    }
}

fn struct_params(data: &[u8]) -> Result<BTreeMap<i32, HookArg>> {
    let mut r = Reader::new(data);
    let mut params = BTreeMap::new();
    loop {
        match r.g1().map_err(|e| e.to_string())? {
            0 => return Ok(params),
            249 => {
                params.extend(crate::ui::targeting::read_params(&mut r).map_err(|e| e.to_string())?)
            }
            // StructType.decode ignores every other opcode without operands.
            _ => {}
        }
    }
}

/// `JagString.parseInt(showSign, value)`.
fn signed(value: i32) -> String {
    if value < 0 {
        value.to_string()
    } else {
        format!("+{value}")
    }
}

/// `StringUtils.formatNumber(language, grouping, 0, value)`.
fn format_number(language: i32, grouping: bool, value: i64) -> String {
    let separator = match language {
        1 | 3 => '.',
        2 => '\u{a0}',
        _ => ',',
    };
    let digits = value.unsigned_abs().to_string();
    let mut out = String::new();
    if value < 0 {
        out.push('-');
    }
    for (i, c) in digits.chars().enumerate() {
        if grouping && i != 0 && (digits.len() - i) % 3 == 0 {
            out.push(separator);
        }
        out.push(c);
    }
    out
}

/// Day/month/year of a count of days since 27 February 2002, the original's
/// epoch of 1014768000000 ms. The original formats in the JVM's default time
/// zone; this uses UTC, which that epoch is midnight in.
fn script_date(days: i32) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    // Days from 1970-01-01 to 2002-02-27, then the civil-from-days algorithm.
    let z = i64::from(days) + 11_745 + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!("{day}-{}-{year}", MONTHS[(month - 1) as usize])
}

impl Vm {
    /// The quick chat menu: category and phrase queries, the phrase being
    /// composed, phrase search and the three send packets.
    fn quick_chat_operation(&mut self, state: &mut State, op: i32, s: &mut Stack) -> Result<()> {
        use crate::ui::quickchat::{self, ENCODE_BYTES};
        // An array read past its end is an exception in the original.
        fn at<T: Copy>(list: Option<&Vec<T>>, index: i32) -> Result<T> {
            list.and_then(|list| usize::try_from(index).ok().and_then(|i| list.get(i)))
                .copied()
                .ok_or_else(|| "Quick chat index outside its list".to_string())
        }
        match op {
            5050 => {
                let id = s.pop()?;
                s.strings.push(quickchat::category(state, id)?.description);
            }
            5051 | 5053 => {
                let category = quickchat::category(state, s.pop()?)?;
                let list = if op == 5051 {
                    category.subcategories
                } else {
                    category.phrases
                };
                s.ints.push(list.map_or(0, |list| list.len() as i32));
            }
            5052 | 5054 | 5062 | 5063 => {
                let [id, index] = s.take()?;
                let category = quickchat::category(state, id)?;
                let (entry, shortcut) = at(
                    if matches!(op, 5052 | 5062) {
                        category.subcategories.as_ref()
                    } else {
                        category.phrases.as_ref()
                    },
                    index,
                )?;
                s.ints.push(if matches!(op, 5052 | 5054) {
                    entry
                } else {
                    shortcut
                });
            }
            5055 => {
                let id = s.pop()?;
                s.strings.push(Some(quickchat::phrase(state, id)?.text()));
            }
            5056 => {
                let phrase = quickchat::phrase(state, s.pop()?)?;
                s.ints.push(
                    phrase
                        .automatic_responses
                        .map_or(0, |list| list.len() as i32),
                );
            }
            5057 => {
                let [id, index] = s.take()?;
                let phrase = quickchat::phrase(state, id)?;
                s.ints.push(at(phrase.automatic_responses.as_ref(), index)?);
            }
            5058 => {
                let id = s.pop()?;
                let phrase = quickchat::phrase(state, id)?;
                let values = vec![0; phrase.commands.as_ref().map_or(0, Vec::len)];
                self.active_phrase = Some((id, phrase, values));
            }
            5059..=5061 => {
                let mut w = rs09_protocol::buffer::Writer::new();
                let opcode = if op == 5060 {
                    w.p8(crate::ui::social::encode(&s.text()?));
                    178
                } else {
                    // 0 is public quick chat and 1 is clan quick chat.
                    w.p1(i32::from(op == 5061));
                    167
                };
                let (id, phrase, values) = self
                    .active_phrase
                    .as_ref()
                    .ok_or("No quick chat phrase is being composed")?;
                w.p2(*id);
                for ((kind, _), value) in phrase.commands.iter().flatten().zip(values) {
                    let bytes = ENCODE_BYTES[*kind as usize];
                    if bytes > 0 {
                        w.p_var_long(bytes, i64::from(*value))
                            .map_err(|e| e.to_string())?;
                    }
                }
                self.outgoing.push(rs09_protocol::game::Frame {
                    opcode,
                    payload: w.into_vec(),
                });
            }
            5064 | 5065 => {
                let [id, shortcut] = s.take()?;
                s.ints.push(if shortcut == -1 {
                    -1
                } else {
                    let category = quickchat::category(state, id)?;
                    if op == 5064 {
                        category.subcategory_by_shortcut(shortcut)
                    } else {
                        category.phrase_by_shortcut(shortcut)
                    }
                });
            }
            5066 => {
                let phrase = quickchat::phrase(state, s.pop()?)?;
                s.ints
                    .push(phrase.commands.map_or(0, |list| list.len() as i32));
            }
            5067 => {
                let [id, index] = s.take()?;
                let phrase = quickchat::phrase(state, id)?;
                s.ints.push(dynamic_command(&phrase, index)?);
            }
            5068 | 5069 => {
                let [index, value] = s.take()?;
                let slot = self
                    .active_phrase
                    .as_mut()
                    .and_then(|(_, _, values)| {
                        usize::try_from(index).ok().and_then(|i| values.get_mut(i))
                    })
                    .ok_or("Quick chat value outside the composed phrase")?;
                *slot = value;
            }
            5070 => {
                let [id, command, param] = s.take()?;
                let phrase = quickchat::phrase(state, id)?;
                if dynamic_command(&phrase, command)? != 0 {
                    return Err("bad command".into());
                }
                // getDynamicCommandParam: -1 outside either list; an index equal
                // to a list's length passes the original's check and then throws.
                let list = phrase
                    .commands
                    .as_ref()
                    .filter(|list| command >= 0 && command as usize <= list.len());
                s.ints.push(match list {
                    None => -1,
                    Some(list) => {
                        let (_, params) = list
                            .get(command as usize)
                            .ok_or("Quick chat command index equals the list length")?;
                        if param < 0 || param as usize > params.len() {
                            -1
                        } else {
                            *params
                                .get(param as usize)
                                .ok_or("Quick chat parameter index equals the list length")?
                        }
                    }
                });
            }
            5071 => {
                let query = lower(&s.text()?);
                let global = s.pop()? == 1;
                let counts: Vec<i32> = serde_json::from_slice(state.pack.get("quickchat/counts")?)
                    .map_err(|e| e.to_string())?;
                let start = if global { 32768 } else { 0 };
                let count = *counts.get(usize::from(global)).ok_or("Quick chat counts")?;
                let (mut ids, mut names, mut overflow) = (vec![], vec![], false);
                for id in start..start + count {
                    let phrase = quickchat::phrase(state, id)?;
                    let text = phrase.text();
                    if phrase.searchable && lower(&text).contains(&query) {
                        if ids.len() >= 50 {
                            overflow = true;
                            break;
                        }
                        ids.push(id);
                        names.push(text);
                    }
                }
                if overflow {
                    // The original leaves the cursor untouched on overflow.
                    self.find_results = None;
                    self.find_index = -1;
                } else {
                    let last = ids.len() as isize - 1;
                    find_sort(&mut ids, &mut names, 0, last);
                    self.find_index = ids.len() as i32;
                    self.find_size = 0;
                    self.find_results = Some(ids);
                }
                s.ints.push(self.find_index);
            }
            5072 => {
                let next = self
                    .find_results
                    .as_ref()
                    .filter(|_| self.find_size < self.find_index)
                    .and_then(|ids| ids.get(self.find_size as usize))
                    .copied();
                s.ints.push(match next {
                    Some(id) => {
                        self.find_size += 1;
                        id & 0xffff
                    }
                    None => -1,
                });
            }
            5073 => self.find_size = 0,
            _ => return Err(format!("Unsupported CS2 opcode {op}")),
        }
        Ok(())
    }

    /// `Math.random()`. A fixed-algorithm generator keeps captures repeatable;
    /// `random_state` may be seeded, and zero seeds from the client cycle.
    pub(super) fn random(&mut self) -> f64 {
        let mut x = if self.random_state == 0 {
            0x9e37_79b9_7f4a_7c15 ^ (self.cycle as u32 as u64)
        } else {
            self.random_state
        };
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.random_state = x;
        (x.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 11) as f64 / (1u64 << 53) as f64
    }

    pub(super) fn extended_operation(
        &mut self,
        state: &mut State,
        op: i32,
        s: &mut Stack,
    ) -> Result<()> {
        match op {
            403 => {
                let [feature, kit] = s.take()?;
                self.design_edits
                    .push(rs09_protocol::game::players::DesignEdit::Kit { feature, kit });
            }
            404 => {
                let [index, colour] = s.take()?;
                self.design_edits
                    .push(rs09_protocol::game::players::DesignEdit::Colour { index, colour });
            }
            410 => {
                let female = s.pop()? != 0;
                self.design_edits
                    .push(rs09_protocol::game::players::DesignEdit::Female(female));
            }
            3101 => {
                let [sequence, delay] = s.take()?;
                self.self_animations.push([sequence, delay]);
            }
            3107 => {
                let option = s.pop()?;
                let name = s.text()?;
                self.player_option_requests.push((option, name));
            }
            3110 => {
                let value = s.pop()?;
                self.outgoing.push(rs09_protocol::game::Frame {
                    opcode: 111,
                    payload: (value as u16).to_be_bytes().to_vec(),
                });
            }
            3201 => {
                let id = s.pop()?;
                self.music.push(MusicRequest::Song(id));
            }
            3202 => {
                let [id, delay] = s.take()?;
                self.music.push(MusicRequest::Jingle { id, delay });
            }
            3313 | 3314 => {
                let [inv, slot] = s.take()?;
                let item = state
                    .containers
                    .get(&inv.wrapping_add(32768))
                    .and_then(|v| usize::try_from(slot).ok().and_then(|slot| v.get(slot)));
                s.ints.push(if op == 3313 {
                    item.map_or(-1, |i| i.id)
                } else {
                    item.map_or(0, |i| i.amount)
                });
            }
            3315 => {
                let [inv, id] = s.take()?;
                s.ints.push(if id == -1 {
                    0
                } else {
                    state
                        .containers
                        .get(&inv.wrapping_add(32768))
                        .map_or(0, |v| {
                            v.iter()
                                .filter(|i| i.id == id)
                                .fold(0i32, |n, i| n.wrapping_add(i.amount))
                        })
                });
            }
            3323 | 3324 => {
                let marked = (5..=9).contains(&self.blackmarks);
                s.ints.push(match (op, marked) {
                    (_, false) => 0,
                    (3323, true) => 1,
                    _ => self.blackmarks,
                });
            }
            3327 => s.ints.push(i32::from(self.female)),
            3330 => {
                let inv = s.pop()?;
                s.ints.push(if inv < 0 {
                    0
                } else {
                    let size = inv_size(state, inv)?;
                    state.containers.get(&inv).map_or(size, |items| {
                        let free = items.iter().filter(|i| i.id == -1).count() as i32;
                        free.wrapping_add(size).wrapping_sub(items.len() as i32)
                    })
                });
            }
            3331 | 3332 => {
                let [inv, param] = s.take()?;
                let items = state.containers.get(&inv).cloned().unwrap_or_default();
                let mut total = 0i32;
                for item in items.iter().filter(|item| item.id >= 0) {
                    let Ok(definition) = state.items.get(&state.pack, item.id) else {
                        // IDs past the item table are skipped by the original.
                        continue;
                    };
                    if let Some(HookArg::Int(value)) =
                        definition.params.as_ref().and_then(|p| p.get(&param))
                    {
                        total = total.wrapping_add(if op == 3332 {
                            item.amount.wrapping_mul(*value)
                        } else {
                            *value
                        });
                    }
                }
                s.ints.push(total);
            }
            3333 => s.ints.push(self.login_type),
            3335 => s.ints.push(self.language),
            3336 => {
                let [coord, x, plane, y] = s.take()?;
                s.ints.push(
                    coord
                        .wrapping_add(x.wrapping_shl(14))
                        .wrapping_add(plane.wrapping_shl(28))
                        .wrapping_add(y),
                );
            }
            3337 => s.ints.push(self.affiliate),
            3409 => {
                let [value_type, id, value] = s.take()?;
                if id == -1 {
                    return Err("C3409-2".into());
                }
                let (declared, table) = enum_table(state, id)?;
                if declared != value_type {
                    return Err("C3409-1".into());
                }
                s.ints.push(i32::from(
                    table
                        .iter()
                        .any(|(_, v)| matches!(v, HookArg::Int(n) if *n == value)),
                ));
            }
            3410 => {
                let id = s.pop()?;
                let value = s.text()?;
                if id == -1 {
                    return Err("C3410-2".into());
                }
                let (declared, table) = enum_table(state, id)?;
                if declared != 115 {
                    return Err("C3410-1".into());
                }
                s.ints.push(i32::from(
                    table
                        .iter()
                        .any(|(_, v)| matches!(v, HookArg::Text(t) if *t == value)),
                ));
            }
            3411 => {
                let id = s.pop()?;
                let (_, table) = enum_table(state, id)?;
                // The original hash table keeps one value per key.
                let keys: std::collections::BTreeSet<i32> = table.iter().map(|(k, _)| *k).collect();
                s.ints.push(keys.len() as i32);
            }
            4004 => {
                let n = s.pop()?;
                s.ints.push((f64::from(n) * self.random()) as i32);
            }
            4005 => {
                let n = s.pop()?;
                s.ints
                    .push((self.random() * f64::from(n.wrapping_add(1))) as i32);
            }
            4006 => {
                let [y0, y1, x0, x1, x] = s.take()?;
                let divisor = x1.wrapping_sub(x0);
                if divisor == 0 {
                    return Err("CS2 interpolation by zero".into());
                }
                s.ints.push(
                    y1.wrapping_sub(y0)
                        .wrapping_mul(x.wrapping_sub(x0))
                        .wrapping_div(divisor)
                        .wrapping_add(y0),
                );
            }
            4012 => {
                let [base, exponent] = s.take()?;
                s.ints.push(if base == 0 {
                    0
                } else {
                    f64::from(base).powf(f64::from(exponent)) as i32
                });
            }
            4013 => {
                let [base, root] = s.take()?;
                s.ints.push(if base == 0 {
                    0
                } else if root == 0 {
                    i32::MAX
                } else {
                    f64::from(base).powf(1.0 / f64::from(root)) as i32
                });
            }
            4018 => {
                let [a, b, c] = s.take()?;
                if b == 0 {
                    return Err("CS2 division by zero".into());
                }
                s.ints
                    .push((i64::from(a) * i64::from(c) / i64::from(b)) as i32);
            }
            4102 => {
                let n = s.pop()?;
                let text = s.text()?;
                s.strings.push(Some(format!("{text}{}", signed(n))));
            }
            4104 => {
                let days = s.pop()?;
                s.strings.push(Some(script_date(days)));
            }
            4105 => {
                let female = s.string()?;
                let male = s.string()?;
                s.strings.push(if self.female { female } else { male });
            }
            4110 => {
                let second = s.string()?;
                let first = s.string()?;
                let chosen = if s.pop()? == 1 { first } else { second };
                s.strings.push(chosen);
            }
            4119 => {
                let text = s.text()?;
                let mut in_tag = false;
                let mut out = String::with_capacity(text.len());
                for c in text.chars() {
                    match c {
                        '<' => in_tag = true,
                        '>' => in_tag = false,
                        c if !in_tag => out.push(c),
                        _ => {}
                    }
                }
                s.strings.push(Some(out));
            }
            4120 => {
                let [character, from] = s.take()?;
                let text = s.text()?;
                let from = usize::try_from(from).map_err(|_| "Negative string index")?;
                // The original compares the low byte of the character.
                let wanted = character as u8;
                s.ints.push(
                    text.chars()
                        .enumerate()
                        .skip(from)
                        .find(|(_, c)| *c as u32 as u8 == wanted)
                        .map_or(-1, |(i, _)| i as i32),
                );
            }
            4124 => {
                let grouping = s.pop()? != 0;
                let value = s.pop()?;
                s.strings.push(Some(format_number(
                    self.language,
                    grouping,
                    i64::from(value),
                )));
            }
            5002 => {
                // BUG_REPORT: name, then the two selectors as bytes.
                let name = s.text()?;
                let [first, second] = s.take()?;
                let mut payload = crate::ui::social::encode(&name).to_be_bytes().to_vec();
                payload.extend([first.wrapping_sub(1) as u8, second as u8]);
                self.outgoing.push(rs09_protocol::game::Frame {
                    opcode: 99,
                    payload,
                });
            }
            5009 => {
                let message = s.text()?;
                let name = s.text()?;
                if self.rights != 0 || !(self.chat_restricted || self.quick_chat) {
                    let codec = rs09_protocol::huffman::Huffman::new(state.pack.get("huffman")?)
                        .map_err(|e| e.to_string())?;
                    let bytes: Vec<u8> = message.chars().map(|c| c as u32 as u8).collect();
                    let mut w = rs09_protocol::buffer::Writer::new();
                    w.p8(crate::ui::social::encode(&name));
                    w.psmarts(bytes.len() as i32).map_err(|e| e.to_string())?;
                    let mut payload = w.into_vec();
                    payload.extend(codec.encode(&bytes).map_err(|e| e.to_string())?);
                    if payload.len() > 255 {
                        return Err("Private message exceeds the byte packet bound".into());
                    }
                    self.outgoing.push(rs09_protocol::game::Frame {
                        opcode: 201,
                        payload,
                    });
                }
            }
            5050..=5073 => self.quick_chat_operation(state, op, s)?,
            5200..=5220 => self.world_map_operation(state, op, s)?,
            // ScriptRunner.java:4649-4704, on `crate::display::Display`.
            5300 => {
                let [width, height] = s.take()?;
                let entered = self
                    .display
                    .script_full_screen(width, height, &mut self.preferences);
                self.window_mode = self.display.window_mode();
                s.ints.push(entered);
            }
            5301 => {
                self.display.leave_full_screen(&mut self.preferences);
                self.window_mode = self.display.window_mode();
            }
            5302 => s.ints.push(self.display.display_modes().len() as i32),
            5303 => {
                let index = s.pop()?;
                let [width, height] = self
                    .display
                    .mode_size(index)
                    .ok_or("CS2 5303 display mode out of range")?;
                s.ints.extend([width, height]);
            }
            5305 => s.ints.push(self.display.preferred_mode(&self.preferences)),
            5307 => {
                let mode = s.pop()?;
                self.display.script_window_mode(mode, &mut self.preferences);
                self.window_mode = self.display.window_mode();
            }
            5308 => s.ints.push(self.preferences.remembered_mode),
            5309 => {
                let mode = s.pop()?;
                self.preferences.remembered_mode = if (0..=2).contains(&mode) { mode } else { 0 };
                self.preferences.writes = self.preferences.writes.wrapping_add(1);
            }
            5400 => {
                let second = s.text()?;
                let first = s.text()?;
                let value = s.pop()?;
                let mut payload = Vec::new();
                for text in [&first, &second] {
                    if text.chars().any(|c| c == '\0' || c as u32 > 255) {
                        return Err("CS2 5400 text is not Latin-1".into());
                    }
                    payload.extend(text.chars().map(|c| c as u32 as u8));
                    payload.push(0);
                }
                payload.push(value as u8);
                if payload.len() > 255 {
                    return Err("CS2 5400 packet too long".into());
                }
                self.outgoing.push(rs09_protocol::game::Frame {
                    opcode: 117,
                    payload,
                });
            }
            5401 => {
                let [index, rgb] = s.take()?;
                self.recolor.set(index, rgb)?;
                // ObjTypeList.clearSprites(): item sprites are redrawn. (The
                // model caches follow `recolor.revision`.)
                self.item_sprite_resets = self.item_sprite_resets.wrapping_add(1);
            }
            5405 => {
                let [path, points] = s.take()?;
                if (0..2).contains(&path) {
                    if !(0..=4096).contains(&points) {
                        return Err("Invalid camera path length".into());
                    }
                    self.camera_paths[path as usize] = Some(vec![[0; 4]; (points as usize) << 1]);
                }
            }
            5406 => {
                let [path, point, position, height, look, look_height, speed] = s.take()?;
                let index = point.wrapping_shl(1);
                if let Some(points) = usize::try_from(path)
                    .ok()
                    .and_then(|path| self.camera_paths.get_mut(path))
                    .and_then(Option::as_mut)
                {
                    if index >= 0 && (index as usize) + 1 < points.len() + 1 {
                        let index = index as usize;
                        if index + 1 < points.len() {
                            points[index] = [
                                (position >> 14 & 0x3fff) * 128,
                                height,
                                (position & 0x3fff) * 128,
                                speed,
                            ];
                            points[index + 1] = [
                                (look >> 14 & 0x3fff) * 128,
                                look_height,
                                (look & 0x3fff) * 128,
                                0,
                            ];
                        }
                    }
                }
            }
            5407 => {
                let path = s.pop()?;
                let points = usize::try_from(path)
                    .ok()
                    .and_then(|path| self.camera_paths.get(path))
                    .and_then(Option::as_ref)
                    .ok_or("Camera path is not allocated")?;
                s.ints.push((points.len() >> 1) as i32);
            }
            5411 => {
                // ScriptRunner.java:4774-4782: the applet opens the settings
                // page; the application exits.
                self.display.leave_full_screen(&mut self.preferences);
                self.window_mode = self.display.window_mode();
                if self.application {
                    self.exit_requested = true;
                } else if self.requested_urls.len() < 16 {
                    self.requested_urls.push(String::new());
                }
            }
            5419 => s.strings.push(Some(match &state.last_login_address {
                Some((_, Some(name))) => name.clone(),
                Some((address, None)) => {
                    let [a, b, c, d] = address.to_be_bytes();
                    format!("{a}.{b}.{c}.{d}")
                }
                None => String::new(),
            })),
            // SignLink.clientMode 3 is the unsigned applet.
            5420 => s.ints.push(0),
            5421 => {
                // ScriptRunner.java:4801-4803.
                self.display.leave_full_screen(&mut self.preferences);
                self.window_mode = self.display.window_mode();
                s.pop()?;
                let suffix = s.text()?;
                if self.requested_urls.len() < 16 {
                    self.requested_urls.push(suffix);
                }
            }
            5423 => {
                s.string()?;
            }
            5424 => self.menu_style = Some(s.take()?),
            5425 => self.menu_style = None,
            5426 => self.script_cursor = crate::cursor::Id(s.pop()?),
            5427 => self.minimap_cursors = s.take::<2>()?.map(crate::cursor::Id),
            5500 | 5501 => {
                let [coord, height, min_step, speed] = s.take()?;
                let tile = [coord >> 14 & 0x3fff, coord & 0x3fff];
                self.camera_requests.push(if op == 5500 {
                    CameraRequest::MoveTo {
                        tile,
                        height,
                        min_step,
                        speed,
                    }
                } else {
                    CameraRequest::LookAt {
                        tile,
                        height,
                        min_step,
                        speed,
                    }
                });
            }
            5502 => {
                let [path, index, speed_start, speed_end, look_path, look_index] = s.take()?;
                // The original throws for a path above 1 or a segment that
                // does not have its four points.
                let segment = |path: i32, index: i32| -> Result<[[i32; 3]; 4]> {
                    let points = usize::try_from(path)
                        .ok()
                        .and_then(|path| self.camera_paths.get(path))
                        .and_then(Option::as_ref)
                        .ok_or("Camera path is not allocated")?;
                    let first = usize::try_from(index)
                        .ok()
                        .filter(|&index| (points.len() >> 1) > index + 1)
                        .ok_or("Camera path segment out of range")?
                        * 2;
                    Ok(std::array::from_fn(|i| {
                        let [x, height, y, _] = points[first + i];
                        [x, height, y]
                    }))
                };
                let request = CameraRequest::Spline {
                    position: segment(path, index)?,
                    look: segment(look_path, look_index)?,
                    speed_start,
                    speed_end,
                };
                self.camera_requests.push(request);
            }
            5503 => self.camera_requests.push(CameraRequest::Unlock),
            5504 => {
                let [pitch, yaw] = s.take()?;
                self.camera_requests
                    .push(CameraRequest::Angle { pitch, yaw });
            }
            5505 => s.ints.push(self.camera_angles[0]),
            5506 => s.ints.push(self.camera_angles[1]),
            6001..=6028 => {
                let value = s.pop()?;
                let on = value == 1;
                let p = &mut self.preferences;
                let mut reported = true;
                // 6016 and 6021 neither write the file nor report.
                let mut written = true;
                match op {
                    6001 => p.brightness = value.clamp(1, 4),
                    6002 => p.all_levels_visible = on,
                    6003 => p.remove_roofs_selectively = on,
                    6005 => p.show_ground_decorations = on,
                    6006 => p.high_detail_textures = on,
                    6007 => p.many_idle_animations = on,
                    6008 => p.flickering_effects = on,
                    6009 => p.many_ground_textures = on,
                    6010 => p.character_shadows = on,
                    6011 => p.scenery_shadows = if (0..=2).contains(&value) { value } else { 0 },
                    6012 => p.high_detail_lighting = on,
                    6014 => p.high_water_detail = on,
                    6015 => p.fog = on,
                    6016 => {
                        // ScriptRunner.java:5182-5184: with the hardware
                        // renderer on, the canvas is replaced at the next
                        // redraw, which restarts the renderer and reports
                        // the window with the new value.
                        if self.display.renderer() {
                            self.display.replace_canvas = true;
                        }
                        p.anti_aliasing = if (0..=2).contains(&value) { value } else { 0 };
                        (reported, written) = (false, false);
                    }
                    6017 => p.stereo = on,
                    6018 => p.sound_volume = value.clamp(0, 127),
                    6019 => p.music_volume = value.clamp(0, 255),
                    6020 => p.ambient_volume = value.clamp(0, 127),
                    6021 => {
                        p.never_remove_roofs = on;
                        (reported, written) = (false, false);
                    }
                    6023 => {
                        p.particles = value.clamp(0, 2);
                        // 1 unless the original's low-memory guard forced 0.
                        s.ints.push(1);
                    }
                    6024 => {
                        p.window_mode = if (0..=2).contains(&value) { value } else { 0 };
                        reported = false;
                    }
                    6028 => {
                        p.cursors = value != 0;
                        reported = false;
                    }
                    _ => return Err(format!("Unsupported CS2 opcode {op}")),
                }
                if written {
                    p.writes = p.writes.wrapping_add(1);
                }
                if reported {
                    p.sent_to_server = false;
                }
                // FogManager.setInstantFade, which both do in HD.
                if matches!(op, 6001 | 6015) {
                    self.instant_fade = true;
                }
                if op == 6001 {
                    // ObjTypeList.clearSprites(): item sprites are redrawn.
                    self.item_sprite_resets = self.item_sprite_resets.wrapping_add(1);
                }
            }
            6101..=6128 => {
                let p = &self.preferences;
                s.ints.push(match op {
                    6101 => p.brightness,
                    // SceneGraph.allLevelsAreVisible: always in HD.
                    6102 => i32::from(self.window_mode != 0 || p.all_levels_visible),
                    6103 => i32::from(p.remove_roofs_selectively),
                    6105 => i32::from(p.show_ground_decorations),
                    6106 => i32::from(p.high_detail_textures),
                    6107 => i32::from(p.many_idle_animations),
                    6108 => i32::from(p.flickering_effects),
                    6109 => i32::from(p.many_ground_textures),
                    6110 => i32::from(p.character_shadows),
                    6111 => p.scenery_shadows,
                    6112 => i32::from(p.high_detail_lighting),
                    6114 => i32::from(p.high_water_detail),
                    6115 => i32::from(p.fog),
                    6116 => p.anti_aliasing,
                    6117 => i32::from(p.stereo),
                    6118 => p.sound_volume,
                    6119 => p.music_volume,
                    6120 => p.ambient_volume,
                    // GlRenderer.arbMultisampleSupported with the hardware
                    // renderer on, else 0 (ScriptRunner.java:5367-5374). The
                    // window is multisampled as the saved preference says;
                    // the browser's drawing buffer is the page's.
                    6121 => i32::from(cfg!(not(target_arch = "wasm32")) && self.display.renderer()),
                    6123 => p.particles,
                    6124 => p.window_mode,
                    6128 => i32::from(p.cursors),
                    _ => return Err(format!("Unsupported CS2 opcode {op}")),
                });
            }
            6200 => {
                let [near, far] = s.take()?.map(|n| i32::from(n as i16));
                self.viewport.near_zoom = if near <= 0 { 256 } else { near };
                self.viewport.far_zoom = if far <= 0 { 205 } else { far };
            }
            6201 => {
                let [min, max] = s.take()?.map(|n| i32::from(n as i16));
                self.viewport.min_view_distance = if min <= 0 { 256 } else { min };
                self.viewport.max_view_distance = if max <= 0 { 320 } else { max };
            }
            6202 => {
                let [min_zoom, max_zoom, min_fov, max_fov] = s.take()?.map(|n| i32::from(n as i16));
                let v = &mut self.viewport;
                v.min_zoom = if min_zoom <= 0 { 1 } else { min_zoom };
                v.max_zoom = if max_zoom <= 0 {
                    32767
                } else {
                    max_zoom.max(v.min_zoom)
                };
                v.min_fov = if min_fov <= 0 { 1 } else { min_fov };
                v.max_fov = if max_fov <= 0 {
                    32767
                } else {
                    max_fov.max(v.min_fov)
                };
            }
            6204 => s.ints.extend([
                self.viewport.min_view_distance,
                self.viewport.max_view_distance,
            ]),
            6205 => s
                .ints
                .extend([self.viewport.near_zoom, self.viewport.far_zoom]),
            6300 => s.ints.push((self.now_ms / 60_000) as i32),
            6301 => s.ints.push((self.now_ms / 86_400_000) as i32 - 11_745),
            6302 => {
                // Day, zero-based month, year; noon avoids any day boundary.
                let [day, month, year] = s.take()?;
                s.ints.push(
                    (days_from_civil(i64::from(year), i64::from(month) + 1, i64::from(day))
                        - 11_745) as i32,
                );
            }
            6303 => {
                let days = self.now_ms.div_euclid(86_400_000);
                // The year of `days` since 1970, by searching the civil calendar.
                let mut year = 1970 + days / 366;
                while days_from_civil(year + 1, 1, 1) <= days {
                    year += 1;
                }
                s.ints.push(year as i32);
            }
            6304 => {
                let year = s.pop()?;
                s.ints.push(i32::from(if year < 0 {
                    (year + 1) % 4 == 0
                } else if year < 1582 {
                    year % 4 == 0
                } else if year % 4 != 0 {
                    false
                } else if year % 100 != 0 {
                    true
                } else {
                    year % 400 == 0
                }));
            }
            // The login, account-creation and world-list commands act only at
            // the original title screen (game state 10). This client logs in
            // from its launcher and is never in that state, so each takes the
            // original's "not at the title screen" branch.
            //
            // With a login host (see vm_login.rs) the title screen exists and
            // the host carries them out instead.
            5600..=5611 | 6500..=6503 | 6507 if self.login_operation(state, op, s)? => {}
            5600 => {
                s.text()?;
                s.text()?;
                s.pop()?;
            }
            5601 => {}
            5602 => self.login_reply = None,
            5603 => {
                let [_, _, _, _] = s.take()?;
            }
            5604 => {
                s.text()?;
            }
            5605 => {
                let [_, _, _, _] = s.take()?;
                s.text()?;
                s.text()?;
            }
            5606 => self.create_reply = None,
            5607 => s.ints.push(self.login_reply.unwrap_or(-2)),
            // LoginManager.hopTime: no world hop is ever in progress.
            5608 => s.ints.push(0),
            5609 => s.ints.push(self.create_reply.unwrap_or(-2)),
            5610 => s.strings.extend((0..5).map(|_| Some(String::new()))),
            5611 => s.ints.push(self.disallow_result.unwrap_or(-1)),
            6500 => s.ints.push(1),
            // The world list is empty: no first or next world.
            6501 | 6502 => {
                s.ints.extend([-1, 0, 0, 0]);
                s.strings.extend([Some(String::new()), Some(String::new())]);
            }
            6503 => {
                s.pop()?;
                s.ints.push(0);
            }
            6504 => {
                self.preferences.last_world = s.pop()?;
                self.preferences.writes = self.preferences.writes.wrapping_add(1);
            }
            6505 => s.ints.push(self.preferences.last_world),
            6600 => {
                self.preferences.keyboard_camera = s.pop()? == 1;
                self.preferences.writes = self.preferences.writes.wrapping_add(1);
            }
            6601 => s.ints.push(i32::from(self.preferences.keyboard_camera)),
            6507 => {
                let [_, _, _, _] = s.take()?;
            }
            // No video advertisements exist in this client.
            6405 | 6406 => s.ints.push(0),
            // Whether Alt (5100), Ctrl (5101) or Shift (5102) is held.
            5100..=5102 => s
                .ints
                .push(i32::from(self.modifier_keys[(op - 5100) as usize])),
            4210 => {
                let query = lower(&s.text()?);
                let stock_only = s.pop()? == 1;
                // ObjTypeList.capacity is one past the highest item ID.
                let capacity = state
                    .pack
                    .manifest
                    .files
                    .keys()
                    .filter_map(|name| name.strip_prefix("item/")?.parse::<i32>().ok())
                    .max()
                    .map_or(0, |id| id + 1);
                let mut ids = vec![];
                let mut names = vec![];
                let mut overflow = false;
                for id in 0..capacity {
                    // A missing definition is the original's default item, "null".
                    let (name, allowed) = match state.items.get(&state.pack, id) {
                        Ok(item) => (
                            item.name.clone(),
                            (!stock_only || item.stockMarket)
                                && item.certtemplate == -1
                                && item.lentTemplate == -1
                                && item.dummyItem == 0,
                        ),
                        Err(_) => ("null".to_owned(), !stock_only),
                    };
                    if allowed && lower(&name).contains(&query) {
                        if ids.len() >= 250 {
                            overflow = true;
                            break;
                        }
                        ids.push(id);
                        names.push(name);
                    }
                }
                self.find_size = 0;
                if overflow {
                    self.find_results = None;
                    self.find_index = -1;
                } else {
                    let last = ids.len() as isize - 1;
                    find_sort(&mut ids, &mut names, 0, last);
                    self.find_index = ids.len() as i32;
                    self.find_results = Some(ids);
                }
                s.ints.push(self.find_index);
            }
            4211 => {
                let next = self
                    .find_results
                    .as_ref()
                    .filter(|_| self.find_size < self.find_index)
                    .and_then(|ids| ids.get(self.find_size as usize))
                    .copied();
                s.ints.push(match next {
                    Some(id) => {
                        self.find_size += 1;
                        id & 0xffff
                    }
                    None => -1,
                });
            }
            4212 => self.find_size = 0,
            4208 | 4300 | 4400 | 4500 => {
                let [id, param] = s.take()?;
                let kind = param_type(state, param)?;
                let params: Option<BTreeMap<i32, HookArg>> = match op {
                    4208 => state.items.get(&state.pack, id)?.params,
                    4300 => definition(state, "npc", id)?
                        .map(crate::npc::Definition::decode)
                        .transpose()
                        .map_err(|e| e.to_string())?
                        .map(|d| d.params),
                    4400 => definition(state, "loc", id)?
                        .map(crate::interaction::LocationType::decode)
                        .transpose()
                        .map_err(|e| e.to_string())?
                        .map(|d| d.params),
                    _ => definition(state, "struct", id)?
                        .map(struct_params)
                        .transpose()?,
                };
                let value = params.as_ref().and_then(|p| p.get(&param));
                if kind.kind == 115 {
                    s.strings.push(match value {
                        Some(HookArg::Text(text)) => Some(text.clone()),
                        Some(_) => return Err(format!("Parameter {param} is not text")),
                        None => kind.default_string,
                    });
                } else {
                    s.ints.push(match value {
                        Some(HookArg::Int(n)) => *n,
                        Some(_) => return Err(format!("Parameter {param} is not an integer")),
                        None => kind.default_int,
                    });
                }
            }
            _ => return Err(format!("Unsupported CS2 opcode {op}")),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pack::Pack;
    use rs09_protocol::game::updates::WireItem;

    fn pack(files: &[(&str, Vec<u8>)]) -> Pack {
        let mut spans = serde_json::Map::new();
        let mut blob = Vec::<u8>::new();
        for (name, data) in files {
            spans.insert((*name).into(), serde_json::json!([blob.len(), data.len()]));
            blob.extend(data);
        }
        let manifest =
            serde_json::to_vec(&serde_json::json!({"source":"ops","files":spans})).unwrap();
        let mut bytes = b"RS09VIS1".to_vec();
        bytes.extend((manifest.len() as u32).to_le_bytes());
        bytes.extend(manifest);
        bytes.extend(blob);
        Pack::decode(bytes).unwrap()
    }
    fn params(entries: &[(i32, std::result::Result<i32, &str>)]) -> Vec<u8> {
        let mut out = vec![249, entries.len() as u8];
        for (id, value) in entries {
            out.push(u8::from(value.is_err()));
            out.extend(&id.to_be_bytes()[1..]);
            match value {
                Ok(n) => out.extend(n.to_be_bytes()),
                Err(text) => {
                    out.extend(text.as_bytes());
                    out.push(0);
                }
            }
        }
        out
    }
    fn setup() -> (State, Vm) {
        let mut item = vec![2, b'A', 0];
        item.extend(params(&[(7, Ok(3)), (8, Err("text"))]));
        item.push(0);
        let mut structure = params(&[(7, Ok(42))]);
        structure.push(0);
        let files = vec![
            // Inventory type 93 holds 28 slots.
            ("invtype/93", vec![2, 0, 28, 0]),
            // Enum 5: int keys, int values 10 and 20 (key 1 listed twice).
            (
                "enum/5",
                [
                    &[1, b'i', 2, b'i', 6, 0, 3][..],
                    &[0, 0, 0, 1, 0, 0, 0, 10],
                    &[0, 0, 0, 2, 0, 0, 0, 20],
                    &[0, 0, 0, 1, 0, 0, 0, 10],
                    &[0],
                ]
                .concat(),
            ),
            // Enum 6: string values.
            (
                "enum/6",
                [&[2, 115, 5, 0, 1][..], &[0, 0, 0, 9], b"word\0", &[0]].concat(),
            ),
            ("item/100", item),
            ("item/101", vec![2, b'B', 0, 0]),
            // Parameter 7 is an integer defaulting to -1; 8 is text; 9 is absent.
            ("param/7", vec![1, b'i', 2, 255, 255, 255, 255, 0]),
            ("param/8", [&[1, 115, 5][..], b"none\0", &[0]].concat()),
            ("param-catalog-complete", vec![]),
            ("struct/3", structure),
        ];
        (State::new(pack(&files)), Vm::default())
    }
    fn run(vm: &mut Vm, state: &mut State, op: i32, ints: &[i32], strings: &[&str]) -> Stack {
        let mut s = Stack {
            ints: ints.to_vec(),
            strings: strings.iter().map(|t| Some((*t).to_owned())).collect(),
        };
        vm.extended_operation(state, op, &mut s)
            .unwrap_or_else(|e| panic!("opcode {op}: {e}"));
        s
    }
    fn int(vm: &mut Vm, state: &mut State, op: i32, ints: &[i32], strings: &[&str]) -> i32 {
        let s = run(vm, state, op, ints, strings);
        assert_eq!(s.ints.len(), 1, "opcode {op} leaves one integer");
        s.ints[0]
    }
    fn text(vm: &mut Vm, state: &mut State, op: i32, ints: &[i32], strings: &[&str]) -> String {
        let s = run(vm, state, op, ints, strings);
        assert_eq!((s.ints.len(), s.strings.len()), (0, 1), "opcode {op}");
        s.strings[0].clone().unwrap()
    }

    #[test]
    fn arithmetic_uses_java_integer_and_long_rules() {
        let (mut state, mut vm) = setup();
        let (st, vm) = (&mut state, &mut vm);
        // (y1 - y0) * (x - x0) / (x1 - x0) + y0, truncating toward zero.
        assert_eq!(int(vm, st, 4006, &[10, 20, 0, 3, 1], &[]), 13);
        assert_eq!(int(vm, st, 4006, &[20, 10, 0, 3, 1], &[]), 17);
        assert_eq!(int(vm, st, 4012, &[2, 10], &[]), 1024);
        assert_eq!(int(vm, st, 4012, &[0, 5], &[]), 0);
        // (int) of a double saturates at Integer.MAX_VALUE.
        assert_eq!(int(vm, st, 4012, &[10, 20], &[]), i32::MAX);
        assert_eq!(int(vm, st, 4013, &[27, 3], &[]), 3);
        assert_eq!(int(vm, st, 4013, &[0, 3], &[]), 0);
        assert_eq!(int(vm, st, 4013, &[5, 0], &[]), i32::MAX);
        // The product is formed in 64 bits before the division.
        assert_eq!(
            int(vm, st, 4018, &[2_000_000_000, 4, 2], &[]),
            1_000_000_000
        );
        assert_eq!(int(vm, st, 4018, &[-7, 2, 1], &[]), -3);
        for (op, ints) in [(4006, vec![1, 2, 5, 5, 9]), (4018, vec![1, 0, 1])] {
            let mut s = Stack {
                ints,
                strings: vec![],
            };
            assert!(vm.extended_operation(st, op, &mut s).is_err(), "{op}");
        }
        // coord + (x << 14) + (plane << 28) + y.
        assert_eq!(
            int(
                vm,
                st,
                3336,
                &[(1 << 28) | (3200 << 14) | 3200, 2, 1, -3],
                &[]
            ),
            (2 << 28) | (3202 << 14) | 3197
        );
    }

    #[test]
    fn random_draws_stay_inside_their_original_ranges() {
        let (mut state, mut vm) = setup();
        vm.random_state = 12345;
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..2000 {
            let exclusive = int(&mut vm, &mut state, 4004, &[6], &[]);
            let inclusive = int(&mut vm, &mut state, 4005, &[6], &[]);
            assert!((0..6).contains(&exclusive) && (0..=6).contains(&inclusive));
            seen.insert((false, exclusive));
            seen.insert((true, inclusive));
        }
        // Every value of both ranges turns up: 6 exclusive and 7 inclusive.
        assert_eq!(seen.len(), 13);
        assert_eq!(int(&mut vm, &mut state, 4004, &[0], &[]), 0);
    }

    #[test]
    fn string_commands() {
        let (mut state, mut vm) = setup();
        let (st, vm) = (&mut state, &mut vm);
        assert_eq!(text(vm, st, 4102, &[5], &["x"]), "x+5");
        assert_eq!(text(vm, st, 4102, &[0], &["x"]), "x+0");
        assert_eq!(text(vm, st, 4102, &[-5], &["x"]), "x-5");
        assert_eq!(text(vm, st, 4110, &[1], &["yes", "no"]), "yes");
        assert_eq!(text(vm, st, 4110, &[0], &["yes", "no"]), "no");
        assert_eq!(
            text(vm, st, 4119, &[], &["<col=ff0000>Red</col> text<br>"]),
            "Red text"
        );
        assert_eq!(int(vm, st, 4120, &[i32::from(b'l'), 0], &["hello"]), 2);
        assert_eq!(int(vm, st, 4120, &[i32::from(b'l'), 3], &["hello"]), 3);
        assert_eq!(int(vm, st, 4120, &[i32::from(b'z'), 0], &["hello"]), -1);
        assert_eq!(text(vm, st, 4124, &[1234567, 1], &[]), "1,234,567");
        assert_eq!(text(vm, st, 4124, &[1234567, 0], &[]), "1234567");
        assert_eq!(text(vm, st, 4124, &[-1000, 1], &[]), "-1,000");
        assert_eq!(text(vm, st, 4124, &[999, 1], &[]), "999");
        assert_eq!(text(vm, st, 4124, &[0, 1], &[]), "0");
        assert_eq!(text(vm, st, 4124, &[i32::MIN, 1], &[]), "-2,147,483,648");
        vm.language = 1;
        assert_eq!(text(vm, st, 4124, &[1234567, 1], &[]), "1.234.567");
        vm.language = 0;
        assert_eq!(text(vm, st, 4105, &[], &["his", "her"]), "his");
        vm.female = true;
        assert_eq!(text(vm, st, 4105, &[], &["his", "her"]), "her");
        assert_eq!(int(vm, st, 3327, &[], &[]), 1);
    }

    #[test]
    fn script_dates_count_days_from_the_original_epoch() {
        let (mut state, mut vm) = setup();
        let (st, vm) = (&mut state, &mut vm);
        assert_eq!(text(vm, st, 4104, &[0], &[]), "27-Feb-2002");
        assert_eq!(text(vm, st, 4104, &[1], &[]), "28-Feb-2002");
        assert_eq!(text(vm, st, 4104, &[2], &[]), "1-Mar-2002");
        // 2004 is a leap year: 27 Feb 2002 + 732 days is 29 Feb 2004.
        assert_eq!(text(vm, st, 4104, &[732], &[]), "29-Feb-2004");
        assert_eq!(text(vm, st, 4104, &[-58], &[]), "31-Dec-2001");
        assert_eq!(text(vm, st, 4104, &[2865], &[]), "1-Jan-2010");
    }

    #[test]
    fn container_totals_and_free_space() {
        let (mut state, mut vm) = setup();
        // A container the server never sent is entirely free; a negative ID has none.
        assert_eq!(int(&mut vm, &mut state, 3330, &[93], &[]), 28);
        assert_eq!(int(&mut vm, &mut state, 3330, &[-1], &[]), 0);
        state.containers.insert(
            93,
            vec![
                WireItem { id: 100, amount: 5 },
                WireItem { id: -1, amount: 0 },
                WireItem { id: 101, amount: 2 },
                WireItem { id: 100, amount: 1 },
            ],
        );
        state.containers.insert(
            93 + 32768,
            vec![
                WireItem { id: 100, amount: 9 },
                WireItem { id: 101, amount: 4 },
            ],
        );
        let (st, vm) = (&mut state, &mut vm);
        // One empty slot among four, plus the 24 slots never sent.
        assert_eq!(int(vm, st, 3330, &[93], &[]), 25);

        // Parameter 7 is 3 on item 100 only: two stacks, six items.
        assert_eq!(int(vm, st, 3331, &[93, 7], &[]), 6);
        assert_eq!(int(vm, st, 3332, &[93, 7], &[]), 18);
        assert_eq!(int(vm, st, 3331, &[94, 7], &[]), 0);
        // The inspecting variants read container ID + 32768.
        assert_eq!(int(vm, st, 3313, &[93, 1], &[]), 101);
        assert_eq!(int(vm, st, 3314, &[93, 0], &[]), 9);
        assert_eq!(int(vm, st, 3313, &[93, 7], &[]), -1);
        assert_eq!(int(vm, st, 3314, &[93, 7], &[]), 0);
        assert_eq!(int(vm, st, 3315, &[93, 100], &[]), 9);
        assert_eq!(int(vm, st, 3315, &[93, -1], &[]), 0);
    }

    #[test]
    fn enum_queries_check_the_declared_type() {
        let (mut state, mut vm) = setup();
        let (st, vm) = (&mut state, &mut vm);
        assert_eq!(int(vm, st, 3409, &[i32::from(b'i'), 5, 20], &[]), 1);
        assert_eq!(int(vm, st, 3409, &[i32::from(b'i'), 5, 30], &[]), 0);
        assert_eq!(int(vm, st, 3410, &[6], &["word"]), 1);
        assert_eq!(int(vm, st, 3410, &[6], &["Word"]), 0);
        // Two distinct keys; the repeated key counts once.
        assert_eq!(int(vm, st, 3411, &[5], &[]), 2);
        assert_eq!(int(vm, st, 3411, &[6], &[]), 1);
        for (op, ints, strings, message) in [
            (3409, vec![115, 5, 1], Vec::<&str>::new(), "C3409-1"),
            (3409, vec![105, -1, 1], vec![], "C3409-2"),
            (3410, vec![5], vec!["x"], "C3410-1"),
            (3410, vec![-1], vec!["x"], "C3410-2"),
        ] {
            let mut s = Stack {
                ints,
                strings: strings
                    .into_iter()
                    .map(|t: &str| Some(t.to_owned()))
                    .collect(),
            };
            assert_eq!(vm.extended_operation(st, op, &mut s).unwrap_err(), message);
        }
    }

    #[test]
    fn definition_parameters_fall_back_to_the_parameter_default() {
        let (mut state, mut vm) = setup();
        let (st, vm) = (&mut state, &mut vm);
        assert_eq!(int(vm, st, 4208, &[100, 7], &[]), 3);
        assert_eq!(text(vm, st, 4208, &[100, 8], &[]), "text");
        // Item 101 has no parameters: the parameter type's defaults apply.
        assert_eq!(int(vm, st, 4208, &[101, 7], &[]), -1);
        assert_eq!(text(vm, st, 4208, &[101, 8], &[]), "none");
        // An undefined parameter in a complete catalogue is an integer 0.
        assert_eq!(int(vm, st, 4208, &[100, 9], &[]), 0);
        assert_eq!(int(vm, st, 4500, &[3, 7], &[]), 42);
        // A struct the complete catalogue lacks behaves as an empty one.
        assert_eq!(int(vm, st, 4500, &[4, 7], &[]), -1);
    }

    #[test]
    fn requests_and_account_flags() {
        let (mut state, mut vm) = setup();
        let (st, vm) = (&mut state, &mut vm);
        run(vm, st, 3101, &[808, 4], &[]);
        assert_eq!(vm.self_animations, [[808, 4]]);
        run(vm, st, 3201, &[76], &[]);
        run(vm, st, 3202, &[90, 150], &[]);
        assert_eq!(
            vm.music,
            [
                MusicRequest::Song(76),
                MusicRequest::Jingle { id: 90, delay: 150 }
            ]
        );
        run(vm, st, 403, &[2, 20], &[]);
        run(vm, st, 404, &[4, 7], &[]);
        run(vm, st, 410, &[1], &[]);
        {
            use rs09_protocol::game::players::DesignEdit;
            assert_eq!(
                vm.design_edits,
                [
                    DesignEdit::Kit {
                        feature: 2,
                        kit: 20
                    },
                    DesignEdit::Colour {
                        index: 4,
                        colour: 7
                    },
                    DesignEdit::Female(true),
                ]
            );
        }
        run(vm, st, 3107, &[4], &["Bob"]);
        assert_eq!(vm.player_option_requests, [(4, "Bob".to_owned())]);
        run(vm, st, 3110, &[0x1234], &[]);
        assert_eq!(
            (vm.outgoing[0].opcode, vm.outgoing[0].payload.as_slice()),
            (111, &[0x12, 0x34][..])
        );
        assert_eq!(
            rs09_protocol::game::Direction::ClientToServer.size(111),
            Some(rs09_protocol::game::Size::Fixed(2))
        );
        // Black marks are reported only in the original's 5..=9 band.
        for (marks, flag, count) in [(0, 0, 0), (4, 0, 0), (5, 1, 5), (9, 1, 9), (10, 0, 0)] {
            vm.blackmarks = marks;
            assert_eq!(int(vm, st, 3323, &[], &[]), flag, "{marks}");
            assert_eq!(int(vm, st, 3324, &[], &[]), count, "{marks}");
        }
        vm.login_type = 2;
        vm.affiliate = 7;
        assert_eq!(int(vm, st, 3333, &[], &[]), 2);
        assert_eq!(int(vm, st, 3335, &[], &[]), 0);
        assert_eq!(int(vm, st, 3337, &[], &[]), 7);
        let mut s = Stack::default();
        assert!(vm.extended_operation(st, 9999, &mut s).is_err());
    }
}

#[cfg(test)]
mod settings_tests {
    use super::*;
    use crate::pack::Pack;

    fn setup() -> (State, Vm) {
        let manifest = br#"{"source":"settings","files":{}}"#;
        let mut bytes = b"RS09VIS1".to_vec();
        bytes.extend((manifest.len() as u32).to_le_bytes());
        bytes.extend(manifest);
        (State::new(Pack::decode(bytes).unwrap()), Vm::default())
    }
    fn run(vm: &mut Vm, state: &mut State, op: i32, ints: &[i32], strings: &[&str]) -> Stack {
        let mut s = Stack {
            ints: ints.to_vec(),
            strings: strings.iter().map(|t| Some((*t).to_owned())).collect(),
        };
        vm.extended_operation(state, op, &mut s)
            .unwrap_or_else(|e| panic!("opcode {op}: {e}"));
        s
    }

    #[test]
    fn preferences_start_at_the_original_defaults_and_clamp_like_the_original() {
        let (mut state, mut vm) = setup();
        let (st, vm) = (&mut state, &mut vm);
        let get = |vm: &mut Vm, st: &mut State, op| run(vm, st, op, &[], &[]).ints[0];
        for (op, default) in [
            (6101, 3),
            (6102, 1),
            (6103, 1),
            (6105, 1),
            (6106, 1),
            (6107, 1),
            (6108, 1),
            (6109, 1),
            (6110, 1),
            (6111, 2),
            (6112, 1),
            (6114, 1),
            (6115, 1),
            (6116, 0),
            (6117, 1),
            (6118, 127),
            (6119, 255),
            (6120, 127),
            (6121, 0),
            (6123, 2),
            (6124, 0),
            (6128, 1),
        ] {
            assert_eq!(get(vm, st, op), default, "{op}");
        }
        // Each setter with an out-of-range value, then the getter that reads it.
        for (set, value, read, expected) in [
            (6001, 0, 6101, 1),
            (6001, 9, 6101, 4),
            (6002, 0, 6102, 0),
            (6003, 2, 6103, 0),
            (6011, 3, 6111, 0),
            (6011, 1, 6111, 1),
            (6016, -1, 6116, 0),
            (6016, 2, 6116, 2),
            (6018, 200, 6118, 127),
            (6018, -5, 6118, 0),
            (6019, 300, 6119, 255),
            (6020, 64, 6120, 64),
            (6024, 5, 6124, 0),
            (6024, 2, 6124, 2),
            (6028, 0, 6128, 0),
            (6028, 7, 6128, 1),
        ] {
            assert!(run(vm, st, set, &[value], &[]).ints.is_empty(), "{set}");
            assert_eq!(get(vm, st, read), expected, "{set} {value}");
        }
        // Particle quality answers 1 and clamps into 0..=2.
        assert_eq!(run(vm, st, 6023, &[9], &[]).ints, [1]);
        assert_eq!(get(vm, st, 6123), 2);
        // Anti-aliasing (6016, twice above) is the one setter that does not
        // write the file.
        assert_eq!(vm.preferences.writes, 15);
        // In HD every level is visible whatever the preference says.
        vm.window_mode = 2;
        assert_eq!(get(vm, st, 6102), 1);
        assert!(!vm.preferences.all_levels_visible);
        // ScriptRunner.java:5179-5190 and 5367-5374: with the hardware
        // renderer on, multisampling is on offer and a new anti-aliasing
        // value replaces the canvas at the next redraw. (6016 above ran
        // with the renderer off and asked for nothing.)
        assert!(!vm.display.replace_canvas);
        vm.display.script_window_mode(2, &mut vm.preferences);
        assert_eq!(get(vm, st, 6121), 1);
        let writes = vm.preferences.writes;
        assert!(run(vm, st, 6016, &[1], &[]).ints.is_empty());
        assert!(vm.display.replace_canvas);
        assert_eq!(
            (vm.preferences.anti_aliasing, vm.preferences.writes),
            (1, writes)
        );
        // The keyboard camera switch is written but not reported.
        vm.preferences.sent_to_server = true;
        assert_eq!(get(vm, st, 6601), 0);
        assert!(run(vm, st, 6600, &[1], &[]).ints.is_empty());
        assert_eq!(get(vm, st, 6601), 1);
        assert!(run(vm, st, 6600, &[2], &[]).ints.is_empty());
        assert_eq!(get(vm, st, 6601), 0);
        assert_eq!(vm.preferences.writes, 17);
        assert!(vm.preferences.sent_to_server);
    }

    #[test]
    fn saved_preferences_round_trip_and_are_clamped_on_load() {
        let mut saved = Preferences {
            brightness: 2,
            fog: false,
            music_volume: 40,
            writes: 9,
            sent_to_server: false,
            ..Preferences::default()
        };
        let loaded = Preferences::from_json(&saved.to_json()).unwrap();
        // The bookkeeping fields are not saved and restart at their defaults.
        saved.writes = 0;
        saved.sent_to_server = true;
        assert_eq!(loaded, saved);
        // A partial or hand-edited file keeps defaults and clamps the rest.
        let edited = br#"{"brightness":99,"sound_volume":-4,"scenery_shadows":7,"unknown":1}"#;
        let loaded = Preferences::from_json(edited).unwrap();
        assert_eq!(
            (
                loaded.brightness,
                loaded.sound_volume,
                loaded.scenery_shadows
            ),
            (4, 0, 0)
        );
        assert_eq!(loaded.music_volume, 255);
        assert!(loaded.sent_to_server && loaded.fog);
        assert!(Preferences::from_json(b"not json").is_err());
        assert!(Preferences::from_json(br#"{"brightness":"high"}"#).is_err());
    }

    #[test]
    fn commands_run_in_the_client_before_they_are_forwarded() {
        use crate::cheat::{Effect, Failure};
        let (mut state, mut vm) = setup();
        let mut send = |vm: &mut Vm, text: &str| {
            let mut s = Stack {
                ints: vec![],
                strings: vec![Some(text.to_owned())],
            };
            vm.operation(&mut state, 5008, false, &mut s)
        };
        send(&mut vm, "::fpson").unwrap();
        assert!(vm.cheat.display_fps);
        assert_eq!(
            vm.cheat.pending,
            [Effect::DisplayFps(true), Effect::Forward]
        );
        assert_eq!(
            vm.outgoing.last().map(|f| (f.opcode, f.payload.as_slice())),
            Some((44, b"fpson\0".as_slice()))
        );
        // A command that throws ends the script and is not sent.
        assert!(send(&mut vm, "::errortest").is_err());
        assert!(send(&mut vm, "::setparticles").is_err());
        assert_eq!(vm.outgoing.len(), 1);
        assert_eq!(
            vm.cheat.pending[2..],
            [
                Effect::Fail(Failure::ErrorTest),
                Effect::Fail(Failure::NegativeArraySize)
            ]
        );
        // The particle command writes the preference like CS2 6023 does.
        send(&mut vm, "::setparticles 1").unwrap();
        let p = &vm.preferences;
        assert_eq!((p.particles, p.writes, p.sent_to_server), (1, 1, false));
        // Commands the client knows nothing about are forwarded untouched.
        send(&mut vm, "::item 995 100").unwrap();
        assert_eq!(vm.outgoing.len(), 3);
        assert_eq!(vm.outgoing[2].payload, b"item 995 100\0");
    }

    #[test]
    fn brightness_and_fog_ask_for_an_instant_fade() {
        let (mut state, mut vm) = setup();
        for (op, expected) in [(6005, false), (6001, true), (6015, true), (6019, false)] {
            vm.instant_fade = false;
            run(&mut vm, &mut state, op, &[1], &[]);
            assert_eq!(vm.instant_fade, expected, "{op}");
        }
    }

    #[test]
    fn only_server_reported_preferences_clear_the_sent_flag() {
        let (mut state, mut vm) = setup();
        for op in [6016, 6021, 6024, 6028] {
            run(&mut vm, &mut state, op, &[1], &[]);
            assert!(vm.preferences.sent_to_server, "{op}");
        }
        assert!(vm.preferences.never_remove_roofs);
        // Of those, only 6024 and 6028 write the file.
        assert_eq!(vm.preferences.writes, 2);
        run(&mut vm, &mut state, 6008, &[0], &[]);
        assert!(!vm.preferences.sent_to_server && !vm.preferences.flickering_effects);
    }

    #[test]
    fn viewport_limits_take_shorts_and_default_non_positive_values() {
        let (mut state, mut vm) = setup();
        let (st, vm) = (&mut state, &mut vm);
        assert_eq!(run(vm, st, 6204, &[], &[]).ints, [256, 320]);
        assert_eq!(run(vm, st, 6205, &[], &[]).ints, [256, 205]);
        run(vm, st, 6200, &[300, 0], &[]);
        run(vm, st, 6201, &[-1, 65536 + 400], &[]);
        assert_eq!(run(vm, st, 6205, &[], &[]).ints, [300, 205]);
        // 65936 is 400 after the short cast.
        assert_eq!(run(vm, st, 6204, &[], &[]).ints, [256, 400]);
        run(vm, st, 6202, &[500, 100, 0, 0], &[]);
        assert_eq!(
            (vm.viewport.min_zoom, vm.viewport.max_zoom),
            (500, 500),
            "a maximum below the minimum is raised to it"
        );
        assert_eq!((vm.viewport.min_fov, vm.viewport.max_fov), (1, 32767));
    }

    #[test]
    fn clock_commands_use_the_supplied_time() {
        let (mut state, mut vm) = setup();
        let (st, vm) = (&mut state, &mut vm);
        // 2010-01-01T00:00:00Z.
        vm.now_ms = 1_262_304_000_000;
        assert_eq!(run(vm, st, 6300, &[], &[]).ints, [21_038_400]);
        assert_eq!(run(vm, st, 6301, &[], &[]).ints, [2865]);
        assert_eq!(run(vm, st, 6303, &[], &[]).ints, [2010]);
        // The last millisecond of 2009.
        vm.now_ms -= 1;
        assert_eq!(run(vm, st, 6303, &[], &[]).ints, [2009]);
        // Day, zero-based month, year: 1 January 2010 and the epoch itself.
        assert_eq!(run(vm, st, 6302, &[1, 0, 2010], &[]).ints, [2865]);
        assert_eq!(run(vm, st, 6302, &[27, 1, 2002], &[]).ints, [0]);
        for (year, leap) in [
            (2000, 1),
            (1900, 0),
            (2004, 1),
            (2001, 0),
            (1500, 1),
            (1581, 0),
            (-1, 1),
            (-2, 0),
        ] {
            assert_eq!(run(vm, st, 6304, &[year], &[]).ints, [leap], "{year}");
        }
        assert_eq!(run(vm, st, 6405, &[], &[]).ints, [0]);
        assert_eq!(run(vm, st, 6406, &[], &[]).ints, [0]);
    }

    // Transcription test: ScriptRunner 4451 to 4474 and 5606 to 5615.
    #[test]
    fn modifier_keys_and_the_keyboard_camera_preference() {
        let (mut state, mut vm) = setup();
        let (st, vm) = (&mut state, &mut vm);
        let held = |vm: &mut Vm, st: &mut State| {
            [5100, 5101, 5102].map(|op| run(vm, st, op, &[], &[]).ints[0])
        };
        assert_eq!(held(vm, st), [0, 0, 0]);
        // Alt, Ctrl, Shift.
        vm.modifier_keys = [false, true, false];
        assert_eq!(held(vm, st), [0, 1, 0]);
        vm.modifier_keys = [true, false, true];
        assert_eq!(held(vm, st), [1, 0, 1]);
        assert_eq!(run(vm, st, 5101, &[7], &[]).ints, [7, 0]);

        assert_eq!(run(vm, st, 6601, &[], &[]).ints, [0]);
        let writes = vm.preferences.writes;
        assert_eq!(run(vm, st, 6600, &[9, 1], &[]).ints, [9]);
        assert_eq!(run(vm, st, 6601, &[], &[]).ints, [1]);
        assert_eq!(vm.preferences.writes, writes + 1);
        assert!(vm.preferences.sent_to_server, "not a reported preference");
        // Only 1 switches it on.
        for (value, on) in [(2, 0), (1, 1), (0, 0), (-1, 0)] {
            run(vm, st, 6600, &[value], &[]);
            assert_eq!(run(vm, st, 6601, &[], &[]).ints, [on], "{value}");
        }
    }

    #[test]
    fn title_screen_commands_take_the_in_game_branch() {
        let (mut state, mut vm) = setup();
        let (st, vm) = (&mut state, &mut vm);
        // Each consumes exactly its arguments and starts nothing.
        for (op, ints, strings) in [
            (5600, vec![1], vec!["name", "password"]),
            (5601, vec![], vec![]),
            (5603, vec![1, 2, 3, 4], vec![]),
            (5604, vec![], vec!["name"]),
            (5605, vec![1, 2, 3, 4], vec!["name", "password"]),
            (6507, vec![1, 0, 2, 1], vec![]),
        ] {
            let s = run(vm, st, op, &ints, &strings);
            assert!(s.ints.is_empty() && s.strings.is_empty(), "{op}");
        }
        assert!(vm.outgoing.is_empty());
        // Reply codes start at the original's -2, -2 and -1.
        assert_eq!(run(vm, st, 5607, &[], &[]).ints, [-2]);
        assert_eq!(run(vm, st, 5609, &[], &[]).ints, [-2]);
        assert_eq!(run(vm, st, 5611, &[], &[]).ints, [-1]);
        vm.login_reply = Some(3);
        vm.create_reply = Some(7);
        assert_eq!(run(vm, st, 5607, &[], &[]).ints, [3]);
        assert_eq!(run(vm, st, 5609, &[], &[]).ints, [7]);
        run(vm, st, 5602, &[], &[]);
        run(vm, st, 5606, &[], &[]);
        assert_eq!(run(vm, st, 5607, &[], &[]).ints, [-2]);
        assert_eq!(run(vm, st, 5609, &[], &[]).ints, [-2]);
        assert_eq!(run(vm, st, 5608, &[], &[]).ints, [0]);
        assert_eq!(
            run(vm, st, 5610, &[], &[]).strings,
            vec![Some(String::new()); 5]
        );

        assert_eq!(run(vm, st, 6500, &[], &[]).ints, [1]);
        for op in [6501, 6502] {
            let s = run(vm, st, op, &[], &[]);
            assert_eq!(s.ints, [-1, 0, 0, 0], "{op}");
            assert_eq!(s.strings, vec![Some(String::new()); 2], "{op}");
        }
        assert_eq!(run(vm, st, 6503, &[5], &[]).ints, [0]);
        run(vm, st, 6504, &[12], &[]);
        assert_eq!(run(vm, st, 6505, &[], &[]).ints, [12]);
        assert_eq!(vm.preferences.writes, 1);
    }

    #[test]
    fn display_and_window_requests() {
        let (mut state, mut vm) = setup();
        let (st, vm) = (&mut state, &mut vm);
        // Transcription test of ScriptRunner.java:4649-4718. `window_mode`
        // is what CS2 5306 pushes. A screen with no modes (full screen
        // unsupported), as on a default `Vm`.
        assert_eq!(vm.window_mode, 0);
        assert_eq!(run(vm, st, 5300, &[1024, 768], &[]).ints, [0]);
        // The failed request ended in the remembered mode (0), which this
        // client carries out as the fixed hardware mode.
        assert_eq!(vm.window_mode, 1);
        assert_eq!(run(vm, st, 5302, &[], &[]).ints, [0]);
        assert_eq!(run(vm, st, 5305, &[], &[]).ints, [-1]);
        let mut s = Stack {
            ints: vec![0],
            strings: vec![],
        };
        assert!(vm.extended_operation(st, 5303, &mut s).is_err());
        // 5306 answers at once, within the script that set the mode.
        for (mode, answer) in [(2, 2), (7, 1), (1, 1), (2, 2), (0, 1), (-1, 1)] {
            assert!(run(vm, st, 5307, &[mode], &[]).ints.is_empty());
            assert_eq!(vm.window_mode, answer, "{mode}");
        }
        assert_eq!(vm.display.canvas, crate::display::FIXED);
        // 5301 without a full-screen frame does nothing.
        vm.display.effects.clear();
        run(vm, st, 5301, &[], &[]);
        assert!(vm.display.effects.is_empty());

        // A screen with modes: the list, its sizes and full screen.
        let mode = |width, height| crate::display::Mode {
            width,
            height,
            bit_depth: 32,
            refresh_rate: 60,
        };
        vm.display = crate::display::Display::new(
            [1024, 768],
            crate::display::System::client(vec![mode(1920, 1080), mode(1280, 720)]),
        );
        vm.preferences.remembered_mode = 2;
        assert_eq!(run(vm, st, 5302, &[], &[]).ints, [2]);
        assert_eq!(run(vm, st, 5303, &[0], &[]).ints, [1280, 720]);
        assert_eq!(run(vm, st, 5303, &[1], &[]).ints, [1920, 1080]);
        assert_eq!(run(vm, st, 5305, &[], &[]).ints, [-1]);
        // A size the screen does not have fails into the remembered mode.
        assert_eq!(run(vm, st, 5300, &[1024, 768], &[]).ints, [0]);
        assert_eq!(vm.window_mode, 2);
        let writes = vm.preferences.writes;
        assert_eq!(run(vm, st, 5300, &[1920, 1080], &[]).ints, [1]);
        assert_eq!(vm.window_mode, 3);
        assert_eq!(vm.preferences.writes, writes + 1);
        assert_eq!(
            [
                vm.preferences.full_screen_width,
                vm.preferences.full_screen_height
            ],
            [1920, 1080]
        );
        assert_eq!(run(vm, st, 5305, &[], &[]).ints, [1]);
        run(vm, st, 5301, &[], &[]);
        assert_eq!(vm.window_mode, 2);
        // 5411 and 5421 leave full screen before they open a page.
        for (op, ints, strings) in [(5411, &[][..], &[][..]), (5421, &[1][..], &["x"][..])] {
            assert_eq!(run(vm, st, 5300, &[1920, 1080], &[]).ints, [1]);
            run(vm, st, op, ints, strings);
            assert_eq!(vm.window_mode, 2, "{op}");
        }
        run(vm, st, 5309, &[2], &[]);
        assert_eq!(run(vm, st, 5308, &[], &[]).ints, [2]);
        run(vm, st, 5309, &[3], &[]);
        assert_eq!(run(vm, st, 5308, &[], &[]).ints, [0]);
        assert_eq!(run(vm, st, 5420, &[], &[]).ints, [0]);
    }

    #[test]
    fn quit_exits_an_application_and_sends_an_applet_to_its_page() {
        // Transcribed from ScriptRunner.java:4774-4782 (CS2 5411, the
        // title's Quit): full screen is left first; then with no frame
        // (an applet) the settings page is opened, with one System.exit(0).
        let (mut state, mut vm) = setup();
        let (st, vm) = (&mut state, &mut vm);
        run(vm, st, 5411, &[], &[]);
        assert!(!vm.exit_requested);
        assert_eq!(vm.requested_urls, [""]);
        vm.requested_urls.clear();
        vm.application = true;
        run(vm, st, 5411, &[], &[]);
        assert!(vm.exit_requested);
        assert!(vm.requested_urls.is_empty());
    }

    #[test]
    fn recolour_palette_entries_are_set_by_5401() {
        let (mut state, mut vm) = setup();
        let (st, vm) = (&mut state, &mut vm);
        assert_eq!(vm.recolor.colors, [0; 256]);
        // The stack holds the palette index, then the RGB colour.
        assert!(run(vm, st, 5401, &[7, 0xff0000], &[]).ints.is_empty());
        assert_eq!(
            i32::from(vm.recolor.colors[7]),
            crate::recolor::rgb_to_hsl(0xff0000)
        );
        assert_eq!(vm.recolor.revision, 1);
        assert_eq!(vm.item_sprite_resets, 1, "ObjTypeList.clearSprites");
        run(vm, st, 5401, &[255, 0x123456], &[]);
        assert_eq!(vm.recolor.revision, 2);
        // An index outside the 256 entries is the original's array exception.
        for index in [256, -1] {
            let mut s = Stack {
                ints: vec![index, 0],
                strings: vec![],
            };
            assert!(vm.extended_operation(st, 5401, &mut s).is_err());
        }
        assert_eq!(vm.recolor.revision, 2);
    }

    #[test]
    fn camera_paths_requests_and_angles() {
        let (mut state, mut vm) = setup();
        let (st, vm) = (&mut state, &mut vm);
        run(vm, st, 5405, &[1, 3], &[]);
        // A path index outside 0..2 is ignored.
        run(vm, st, 5405, &[2, 3], &[]);
        assert_eq!(run(vm, st, 5407, &[1], &[]).ints, [3]);
        let position = (3200 << 14) | 3400;
        let look = (3210 << 14) | 3410;
        run(vm, st, 5406, &[1, 2, position, 500, look, 20, 7], &[]);
        let path = vm.camera_paths[1].as_ref().unwrap();
        assert_eq!(path[4], [3200 * 128, 500, 3400 * 128, 7]);
        assert_eq!(path[5], [3210 * 128, 20, 3410 * 128, 0]);
        // A point past the end, or an unallocated path, changes nothing.
        run(vm, st, 5406, &[1, 3, position, 1, look, 1, 1], &[]);
        run(vm, st, 5406, &[0, 0, position, 1, look, 1, 1], &[]);
        assert!(vm.camera_paths[0].is_none());
        let mut s = Stack {
            ints: vec![0],
            strings: vec![],
        };
        assert!(vm.extended_operation(st, 5407, &mut s).is_err());

        run(vm, st, 5500, &[position, 300, 2, 10], &[]);
        run(vm, st, 5501, &[look, 0, 1, 20], &[]);
        run(vm, st, 5504, &[200, 1024], &[]);
        run(vm, st, 5503, &[], &[]);
        assert_eq!(
            vm.camera_requests,
            [
                CameraRequest::MoveTo {
                    tile: [3200, 3400],
                    height: 300,
                    min_step: 2,
                    speed: 10
                },
                CameraRequest::LookAt {
                    tile: [3210, 3410],
                    height: 0,
                    min_step: 1,
                    speed: 20
                },
                CameraRequest::Angle {
                    pitch: 200,
                    yaw: 1024
                },
                CameraRequest::Unlock,
            ]
        );
        vm.camera_angles = [250, 77];
        // 5502 takes the four control points of a segment from each path.
        run(vm, st, 5405, &[0, 2], &[]);
        for point in 0..2 {
            let position = (3200 + point) << 14 | 3210;
            let look = (3300 + point) << 14 | 3310;
            run(
                vm,
                st,
                5406,
                &[0, point, position, 100 + point, look, 50, 7],
                &[],
            );
            run(vm, st, 5406, &[1, point, look, 20, position, 30, 9], &[]);
        }
        run(vm, st, 5502, &[0, 0, 400, 800, 1, 0], &[]);
        assert_eq!(
            vm.camera_requests.pop(),
            Some(CameraRequest::Spline {
                position: [
                    [3200 * 128, 100, 3210 * 128],
                    [3300 * 128, 50, 3310 * 128],
                    [3201 * 128, 101, 3210 * 128],
                    [3301 * 128, 50, 3310 * 128],
                ],
                look: [
                    [3300 * 128, 20, 3310 * 128],
                    [3200 * 128, 30, 3210 * 128],
                    [3301 * 128, 20, 3310 * 128],
                    [3201 * 128, 30, 3210 * 128],
                ],
                speed_start: 400,
                speed_end: 800,
            })
        );
        // A segment needs the point after it; a path above 1 does not exist.
        for bad in [[0, 1, 1, 1, 1, 0], [2, 0, 1, 1, 1, 0]] {
            let mut s = Stack {
                ints: bad.to_vec(),
                strings: vec![],
            };
            assert!(vm.extended_operation(st, 5502, &mut s).is_err());
        }
        assert_eq!(run(vm, st, 5505, &[], &[]).ints, [250]);
        assert_eq!(run(vm, st, 5506, &[], &[]).ints, [77]);
    }

    #[test]
    fn packets_urls_and_menu_style() {
        let (mut state, mut vm) = setup();
        let (st, vm) = (&mut state, &mut vm);
        run(vm, st, 5400, &[3], &["ab", "c"]);
        assert_eq!(
            (vm.outgoing[0].opcode, vm.outgoing[0].payload.as_slice()),
            (117, &b"ab\0c\0\x03"[..])
        );
        assert_eq!(
            rs09_protocol::game::Direction::ClientToServer.size(117),
            Some(rs09_protocol::game::Size::Byte)
        );
        run(vm, st, 5421, &[1], &["page.ws"]);
        run(vm, st, 5411, &[], &[]);
        assert_eq!(vm.requested_urls, ["page.ws", ""]);
        assert_eq!(run(vm, st, 5419, &[], &[]).strings, [Some(String::new())]);
        st.last_login_address = Some((0x7f00_0001, None));
        assert_eq!(
            run(vm, st, 5419, &[], &[]).strings,
            [Some("127.0.0.1".to_owned())]
        );
        st.last_login_address = Some((0x7f00_0001, Some("host.example".into())));
        assert_eq!(
            run(vm, st, 5419, &[], &[]).strings,
            [Some("host.example".to_owned())]
        );
        let style: Vec<i32> = (1..=11).collect();
        run(vm, st, 5424, &style, &[]);
        assert_eq!(vm.menu_style.unwrap()[10], 11);
        run(vm, st, 5425, &[], &[]);
        assert!(vm.menu_style.is_none());
        run(vm, st, 5426, &[4], &[]);
        run(vm, st, 5427, &[5, 6], &[]);
        use crate::cursor::Id;
        assert_eq!(
            (vm.script_cursor, vm.minimap_cursors),
            (Id(4), [Id(5), Id(6)])
        );
        assert!(run(vm, st, 5423, &[], &["printed"]).strings.is_empty());
    }
}

#[cfg(test)]
mod find_tests {
    use super::*;
    use crate::pack::Pack;

    fn item(name: &str, extra: &[u8]) -> Vec<u8> {
        let mut out = vec![2];
        out.extend(name.as_bytes());
        out.push(0);
        out.extend(extra);
        out.push(0);
        out
    }
    fn setup() -> (State, Vm) {
        // 65 marks an item tradeable on the Grand Exchange; 97/98 make a note;
        // 121/122 make a lent copy.
        let items: Vec<(i32, Vec<u8>)> = vec![
            (0, item("Rune sword", &[65])),
            (1, item("rune axe", &[65])),
            (2, item("Rune sword", &[97, 0, 0, 98, 0, 9])),
            (3, item("Bronze sword", &[])),
            (5, item("RUNE bar", &[65])),
            (9, item("Swordfish", &[65])),
        ];
        let mut spans = serde_json::Map::new();
        let mut blob = Vec::<u8>::new();
        for (id, data) in &items {
            spans.insert(
                format!("item/{id}"),
                serde_json::json!([blob.len(), data.len()]),
            );
            blob.extend(data);
        }
        let manifest =
            serde_json::to_vec(&serde_json::json!({"source":"find","files":spans})).unwrap();
        let mut bytes = b"RS09VIS1".to_vec();
        bytes.extend((manifest.len() as u32).to_le_bytes());
        bytes.extend(manifest);
        bytes.extend(blob);
        (State::new(Pack::decode(bytes).unwrap()), Vm::default())
    }
    fn search(vm: &mut Vm, state: &mut State, stock: i32, query: &str) -> (i32, Vec<i32>) {
        let mut s = Stack {
            ints: vec![stock],
            strings: vec![Some(query.to_owned())],
        };
        vm.extended_operation(state, 4210, &mut s).unwrap();
        let count = s.ints[0];
        let mut found = vec![];
        loop {
            let mut s = Stack::default();
            vm.extended_operation(state, 4211, &mut s).unwrap();
            if s.ints[0] == -1 {
                return (count, found);
            }
            found.push(s.ints[0]);
        }
    }

    #[test]
    fn search_matches_case_insensitively_and_orders_by_raw_bytes() {
        let (mut state, mut vm) = setup();
        // "RUNE bar" sorts first (capitals precede lower case), then the sword,
        // then the lower-case axe. The noted copy (item 2) is excluded.
        assert_eq!(search(&mut vm, &mut state, 0, "RuNe"), (3, vec![5, 0, 1]));
        // Without the stock-market filter the untradeable bronze sword appears.
        assert_eq!(search(&mut vm, &mut state, 0, "sword"), (3, vec![3, 0, 9]));
        assert_eq!(search(&mut vm, &mut state, 1, "sword"), (2, vec![0, 9]));
        assert_eq!(search(&mut vm, &mut state, 1, "zzz"), (0, vec![]));
        // IDs with no definition are the default item named "null".
        assert_eq!(search(&mut vm, &mut state, 0, "null").0, 4);
        assert_eq!(search(&mut vm, &mut state, 1, "null").0, 0);
    }

    #[test]
    fn rewinding_restarts_the_result_cursor() {
        let (mut state, mut vm) = setup();
        let mut s = Stack {
            ints: vec![0],
            strings: vec![Some("rune".into())],
        };
        vm.extended_operation(&mut state, 4210, &mut s).unwrap();
        let next = |vm: &mut Vm, state: &mut State| {
            let mut s = Stack::default();
            vm.extended_operation(state, 4211, &mut s).unwrap();
            s.ints[0]
        };
        assert_eq!(next(&mut vm, &mut state), 5);
        assert_eq!(next(&mut vm, &mut state), 0);
        vm.extended_operation(&mut state, 4212, &mut Stack::default())
            .unwrap();
        assert_eq!(next(&mut vm, &mut state), 5);
    }

    #[test]
    fn original_quicksort_orders_equal_names_by_its_alternating_comparison() {
        let mut ids: Vec<i32> = (0..6).collect();
        let mut names: Vec<String> = ["b", "a", "b", "c", "a", "b"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        let last = ids.len() as isize - 1;
        find_sort(&mut ids, &mut names, 0, last);
        assert_eq!(names, ["a", "a", "b", "b", "b", "c"]);
        // Each name keeps its own IDs; only the order among equals is the
        // original's, which this pins against an accidental stable sort.
        let mut a: Vec<i32> = ids[..2].to_vec();
        let mut b: Vec<i32> = ids[2..5].to_vec();
        a.sort_unstable();
        b.sort_unstable();
        assert_eq!((a, b, ids[5]), (vec![1, 4], vec![0, 2, 5], 3));
    }
}

#[cfg(test)]
mod quick_chat_tests {
    use super::*;
    use crate::pack::Pack;
    use rs09_protocol::game::{Direction, Size};

    fn setup() -> (State, Vm) {
        // Category 0: two subcategories and two phrases with shortcut keys.
        let category = [
            &[1][..],
            b"General\0",
            &[2, 2, 0, 3, b'a', 0, 4, b'b'],
            &[3, 2, 0, 0, b'h', 0, 1, b'm'],
            &[0],
        ]
        .concat();
        // Phrase 0: "Hello." with one automatic response.
        let hello = [&[1][..], b"Hello.\0", &[2, 1, 0, 1], &[0]].concat();
        // Phrase 1: "My level is <." with command 4 (one byte, one parameter).
        let level = [&[1][..], b"My level is <.\0", &[3, 1, 0, 4, 0, 9], &[0]].concat();
        // Phrase 2 is hidden from search; phrase 3: an enum command 0.
        let hidden = [&[1][..], b"Hello hidden\0", &[4, 0]].concat();
        let skill = [&[1][..], b"I like <!\0", &[3, 1, 0, 0, 0, 173], &[0]].concat();
        let files: Vec<(&str, Vec<u8>)> = vec![
            ("qccat/0", category),
            ("qcphrase/0", hello),
            ("qcphrase/1", level),
            ("qcphrase/2", hidden),
            ("qcphrase/3", skill),
            ("quickchat/counts", b"[4,0]".to_vec()),
            (
                "huffman",
                include_bytes!("../../../../fixtures/public-chat/huffman.bin").to_vec(),
            ),
        ];
        let mut spans = serde_json::Map::new();
        let mut blob = Vec::<u8>::new();
        for (name, data) in &files {
            spans.insert((*name).into(), serde_json::json!([blob.len(), data.len()]));
            blob.extend(data);
        }
        let manifest =
            serde_json::to_vec(&serde_json::json!({"source":"qc","files":spans})).unwrap();
        let mut bytes = b"RS09VIS1".to_vec();
        bytes.extend((manifest.len() as u32).to_le_bytes());
        bytes.extend(manifest);
        bytes.extend(blob);
        (State::new(Pack::decode(bytes).unwrap()), Vm::default())
    }
    fn run(vm: &mut Vm, state: &mut State, op: i32, ints: &[i32], strings: &[&str]) -> Stack {
        let mut s = Stack {
            ints: ints.to_vec(),
            strings: strings.iter().map(|t| Some((*t).to_owned())).collect(),
        };
        vm.extended_operation(state, op, &mut s)
            .unwrap_or_else(|e| panic!("opcode {op}: {e}"));
        s
    }
    fn fails(vm: &mut Vm, state: &mut State, op: i32, ints: &[i32]) -> bool {
        let mut s = Stack {
            ints: ints.to_vec(),
            strings: vec![],
        };
        vm.extended_operation(state, op, &mut s).is_err()
    }

    #[test]
    fn category_and_phrase_queries() {
        let (mut state, mut vm) = setup();
        let (st, vm) = (&mut state, &mut vm);
        assert_eq!(
            run(vm, st, 5050, &[0], &[]).strings,
            [Some("General".to_owned())]
        );
        // A category the pack lacks is the empty default: no description.
        assert_eq!(run(vm, st, 5050, &[9], &[]).strings, [None]);
        assert_eq!(run(vm, st, 5051, &[0], &[]).ints, [2]);
        assert_eq!(run(vm, st, 5051, &[9], &[]).ints, [0]);
        assert_eq!(run(vm, st, 5052, &[0, 1], &[]).ints, [4]);
        assert_eq!(run(vm, st, 5062, &[0, 1], &[]).ints, [i32::from(b'b')]);
        assert_eq!(run(vm, st, 5053, &[0], &[]).ints, [2]);
        assert_eq!(run(vm, st, 5054, &[0, 1], &[]).ints, [1]);
        assert_eq!(run(vm, st, 5063, &[0, 0], &[]).ints, [i32::from(b'h')]);
        assert_eq!(run(vm, st, 5064, &[0, i32::from(b'a')], &[]).ints, [3]);
        assert_eq!(run(vm, st, 5065, &[0, i32::from(b'm')], &[]).ints, [1]);
        assert_eq!(run(vm, st, 5064, &[0, -1], &[]).ints, [-1]);
        assert_eq!(run(vm, st, 5065, &[0, 1], &[]).ints, [-1]);
        assert!(fails(vm, st, 5052, &[0, 2]) && fails(vm, st, 5054, &[9, 0]));

        assert_eq!(
            run(vm, st, 5055, &[1], &[]).strings,
            [Some("My level is ....".to_owned())]
        );
        assert_eq!(run(vm, st, 5056, &[0], &[]).ints, [1]);
        assert_eq!(run(vm, st, 5056, &[1], &[]).ints, [0]);
        assert_eq!(run(vm, st, 5057, &[0, 0], &[]).ints, [1]);
        assert!(fails(vm, st, 5057, &[0, 1]));
        assert_eq!(run(vm, st, 5066, &[1], &[]).ints, [1]);
        assert_eq!(run(vm, st, 5066, &[0], &[]).ints, [0]);
        assert_eq!(run(vm, st, 5067, &[1, 0], &[]).ints, [4]);
        // No commands, or an index past the length, is -1; the length itself throws.
        assert_eq!(run(vm, st, 5067, &[0, 0], &[]).ints, [-1]);
        assert_eq!(run(vm, st, 5067, &[1, 2], &[]).ints, [-1]);
        assert_eq!(run(vm, st, 5067, &[1, -1], &[]).ints, [-1]);
        assert!(fails(vm, st, 5067, &[1, 1]));
        // 5070 reads a parameter of an enum command only.
        assert_eq!(run(vm, st, 5070, &[3, 0, 0], &[]).ints, [173]);
        assert_eq!(run(vm, st, 5070, &[3, 0, 2], &[]).ints, [-1]);
        assert!(
            fails(vm, st, 5070, &[1, 0, 0]),
            "command 4 is a bad command"
        );
    }

    #[test]
    fn composing_and_sending_a_phrase() {
        let (mut state, mut vm) = setup();
        let (st, vm) = (&mut state, &mut vm);
        assert!(fails(vm, st, 5059, &[]), "nothing is being composed");
        run(vm, st, 5058, &[1], &[]);
        run(vm, st, 5068, &[0, 99], &[]);
        assert!(fails(vm, st, 5069, &[1, 5]));
        run(vm, st, 5059, &[], &[]);
        run(vm, st, 5061, &[], &[]);
        run(vm, st, 5060, &[], &["Bob"]);
        // Command 4 sends one byte.
        assert_eq!(
            (vm.outgoing[0].opcode, vm.outgoing[0].payload.as_slice()),
            (167, &[0, 0, 1, 99][..])
        );
        assert_eq!(vm.outgoing[1].payload, [1, 0, 1, 99]);
        let mut private = crate::ui::social::encode("bob").to_be_bytes().to_vec();
        private.extend([0, 1, 99]);
        assert_eq!(
            (vm.outgoing[2].opcode, &vm.outgoing[2].payload),
            (178, &private)
        );
        // Phrase 0 has no dynamic values: only the ID follows.
        run(vm, st, 5058, &[0], &[]);
        run(vm, st, 5059, &[], &[]);
        assert_eq!(vm.outgoing[3].payload, [0, 0, 0]);
        for opcode in [167, 178, 201] {
            assert_eq!(Direction::ClientToServer.size(opcode), Some(Size::Byte));
        }
    }

    #[test]
    fn phrase_search_skips_unsearchable_phrases_and_sorts() {
        let (mut state, mut vm) = setup();
        let (st, vm) = (&mut state, &mut vm);
        assert_eq!(run(vm, st, 5071, &[0], &["HELLO"]).ints, [1]);
        assert_eq!(run(vm, st, 5072, &[], &[]).ints, [0]);
        assert_eq!(run(vm, st, 5072, &[], &[]).ints, [-1]);
        run(vm, st, 5073, &[], &[]);
        assert_eq!(run(vm, st, 5072, &[], &[]).ints, [0]);
        // "I like ...!" sorts before "My level is ....".
        assert_eq!(run(vm, st, 5071, &[0], &["l"]).ints, [3]);
        let found: Vec<i32> = (0..3)
            .map(|_| run(vm, st, 5072, &[], &[]).ints[0])
            .collect();
        assert_eq!(found, [0, 3, 1]);
        // The global archive has no phrases in this pack.
        assert_eq!(run(vm, st, 5071, &[1], &["hello"]).ints, [0]);
    }

    #[test]
    fn bug_report_and_private_message_packets() {
        let (mut state, mut vm) = setup();
        let (st, vm) = (&mut state, &mut vm);
        run(vm, st, 5002, &[3, 7], &["Bob"]);
        let mut expected = crate::ui::social::encode("bob").to_be_bytes().to_vec();
        expected.extend([2, 7]);
        assert_eq!(
            (vm.outgoing[0].opcode, &vm.outgoing[0].payload),
            (99, &expected)
        );
        assert_eq!(Direction::ClientToServer.size(99), Some(Size::Fixed(10)));
        // A restricted, non-staff account sends no private message.
        vm.chat_restricted = true;
        run(vm, st, 5009, &[], &["Bob", "hi there"]);
        assert_eq!(vm.outgoing.len(), 1);
        // Staff rights override the restriction; otherwise it must be lifted.
        vm.rights = 1;
        run(vm, st, 5009, &[], &["Bob", "hi there"]);
        vm.rights = 0;
        vm.chat_restricted = false;
        run(vm, st, 5009, &[], &["Bob", "hi there"]);
        assert_eq!(vm.outgoing.len(), 3);
        assert_eq!(vm.outgoing[1], vm.outgoing[2]);
        let frame = &vm.outgoing[1];
        assert_eq!(frame.opcode, 201);
        // The name, then the word-packed text exactly as the receiver decodes it.
        assert_eq!(
            frame.payload[..8],
            crate::ui::social::encode("bob").to_be_bytes()
        );
        assert_eq!(frame.payload[8], 8, "the smart-encoded text length");
        assert_eq!(
            rs09_protocol::game::chat::public_text(&frame.payload[8..]).unwrap(),
            b"Hi there"
        );
    }
}
