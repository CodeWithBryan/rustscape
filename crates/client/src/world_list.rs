//! The RT4 world list: `WorldList`, `World`, `WorldInfo` and
//! `ScriptRunner.getWorld`, without a socket of its own.
//!
//! [`WorldList::fetch`] is `WorldList.fetch()` and runs over the connection
//! shared with the login and account-creation state machines
//! ([`crate::login_flow::Net`]). Strings stay in the original byte encoding.
//!
//! Differences from the original, all on input the original cannot use:
//! a payload that would raise an unchecked exception there (a truncated
//! field, a string version other than 0, a world or country index outside
//! its table, a negative table size) is rejected as error 1002 and leaves the
//! previous list untouched, where the original has already replaced part of
//! it; sorting a list that holds an empty slot reports an error before moving
//! anything, where the original fails part of the way through.
use crate::login_flow::{Dialer, Endpoints, Net, Opened};
use rs09_protocol::buffer::Reader;
use std::io;

/// `JagString.CHAR_SORT_ORDER`.
const CHAR_SORT_ORDER: [u8; 256] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49,
    50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 73, 74, 76, 78, 83, 84, 85, 86,
    91, 92, 93, 94, 95, 97, 103, 104, 105, 106, 107, 108, 113, 114, 115, 116, 118, 119, 120, 121,
    122, 123, 124, 125, 133, 134, 136, 138, 143, 144, 145, 146, 151, 152, 153, 154, 155, 157, 163,
    164, 165, 166, 168, 169, 174, 175, 176, 177, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189,
    190, 191, 192, 193, 194, 195, 196, 197, 97, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208,
    209, 210, 211, 212, 213, 157, 215, 216, 117, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227,
    228, 229, 230, 231, 232, 233, 234, 235, 236, 237, 238, 239, 240, 241, 242, 243, 244, 245, 246,
    247, 248, 249, 66, 66, 66, 66, 66, 66, 65, 75, 79, 79, 79, 79, 87, 87, 87, 87, 77, 96, 98, 98,
    98, 98, 98, 250, 251, 109, 109, 109, 109, 117, 252, 167, 126, 126, 126, 126, 126, 126, 125,
    135, 139, 139, 139, 139, 147, 147, 147, 147, 137, 156, 158, 158, 158, 158, 158, 253, 254, 170,
    170, 170, 170, 178, 255, 178,
];

/// `JagString.isLigature`.
fn is_ligature(c: i32) -> bool {
    matches!(c, 198 | 230 | 156 | 140 | 223)
}

/// `JagString.compare`: -1, 0 or 1 in the client's own collation, with the
/// ligatures (ae, oe, sharp s) expanded to two letters.
pub fn compare_strings(this: &[u8], other: &[u8]) -> i32 {
    let (mut c1, mut c2) = (0i32, 0i32);
    let mut other_len = other.len();
    let mut this_len = this.len();
    let mut this_total = this.len();
    let mut other_total = other.len();
    let (mut this_index, mut other_index) = (0usize, 0usize);
    while this_len != 0 && other_len != 0 {
        if c1 == 156 || c1 == 230 {
            c1 = 101;
        } else if c1 == 140 || c1 == 198 {
            c1 = 69;
        } else if c1 == 223 {
            c1 = 115;
        } else {
            c1 = i32::from(this[this_index]);
            this_index += 1;
        }
        if is_ligature(c1) {
            this_total += 1;
        } else {
            this_len -= 1;
        }
        if c2 == 156 || c2 == 230 {
            c2 = 101;
        } else if c2 == 140 || c2 == 198 {
            c2 = 69;
        } else if c2 == 223 {
            c2 = 115;
        } else {
            c2 = i32::from(other[other_index]);
            other_index += 1;
        }
        if is_ligature(c2) {
            other_total += 1;
        } else {
            other_len -= 1;
        }
        let (o1, o2) = (CHAR_SORT_ORDER[c1 as usize], CHAR_SORT_ORDER[c2 as usize]);
        if o1 < o2 {
            return -1;
        }
        if o2 < o1 {
            return 1;
        }
    }
    if other_total <= this_total {
        i32::from(this_total > other_total)
    } else {
        -1
    }
}

/// `WorldInfo`: a country's flag sprite index and its name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Country {
    pub flag: i32,
    pub name: Vec<u8>,
}

/// `World` with its `AbstractWorld` fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct World {
    pub id: i32,
    /// Index into [`WorldList::countries`].
    pub country: i32,
    pub flags: i32,
    /// -1 is the wire's 65535, an offline world. A world the server has not
    /// counted yet keeps the original's field default, 0.
    pub players: i32,
    pub activity: Vec<u8>,
    pub hostname: Vec<u8>,
}
impl World {
    pub fn is_members(&self) -> bool {
        self.flags & 1 != 0
    }
    pub fn is_quick_chat(&self) -> bool {
        self.flags & 2 != 0
    }
    pub fn is_pvp(&self) -> bool {
        self.flags & 4 != 0
    }
    pub fn is_loot_share(&self) -> bool {
        self.flags & 8 != 0
    }
}

/// What script commands 6501, 6502 and 6506 push for one world.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    pub id: i32,
    pub flags: i32,
    pub activity: Vec<u8>,
    pub country_flag: i32,
    pub country_name: Vec<u8>,
    pub players: i32,
}

/// `WorldList.NO_ACTIVITY`, the text "-".
pub const NO_ACTIVITY: &[u8] = b"-";

/// A list holding an empty slot cannot be sorted; the original throws.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmptySlot;

/// The statics of `WorldList`.
#[derive(Debug)]
pub struct WorldList {
    /// 0 idle, 1 connecting and requesting, 2 length, 3 payload.
    pub step: i32,
    pub open_time: i64,
    pub close_time: i64,
    pub loaded: bool,
    pub buffer_len: usize,
    pub buffer_off: usize,
    buffer: Option<Vec<u8>>,
    /// `sorted`: indices into [`Self::worlds`]. A None is a slot the original
    /// leaves null, which happens when the server names one world twice.
    pub sorted: Vec<Option<usize>>,
    pub size: i32,
    pub checksum: i32,
    pub countries: Vec<Country>,
    pub min_id: i32,
    pub max_id: i32,
    /// `worlds[id - min_id]`.
    pub worlds: Vec<Option<World>>,
    pub errors: i32,
    pub world_pos: usize,
    /// The request was written and its one-byte answer has not arrived: the
    /// point where the original sits in a blocking read.
    awaiting: bool,
}
impl Default for WorldList {
    fn default() -> Self {
        Self {
            step: 0,
            open_time: 0,
            close_time: 0,
            loaded: false,
            buffer_len: 0,
            buffer_off: 0,
            buffer: None,
            sorted: Vec::new(),
            size: 0,
            checksum: 0,
            countries: Vec::new(),
            min_id: 0,
            max_id: 0,
            worlds: Vec::new(),
            errors: 0,
            world_pos: 1,
            awaiting: false,
        }
    }
}

struct Decoded {
    countries: Vec<Country>,
    min_id: i32,
    max_id: i32,
    size: i32,
    worlds: Vec<Option<World>>,
    checksum: i32,
}

impl WorldList {
    /// `WorldList.fetch()`. Returns -1 while a request is in progress, 0 when
    /// a list has just been decoded (and also when called within five seconds
    /// of the last one, without fetching), or an error code once five
    /// attempts in a row have failed: 1000 no answer within 30 seconds, 1001
    /// could not connect, 1002 bad payload, 1003 connection error, or the
    /// server's own first byte when that is not 0.
    ///
    /// `now` is `MonotonicClock.currentTimeMillis()`, sampled once per call.
    pub fn fetch<D: Dialer>(
        &mut self,
        net: &mut Net<D>,
        endpoints: &mut Endpoints,
        now: i64,
    ) -> i32 {
        match self.fetch_steps(net, endpoints, now) {
            Ok(value) => value,
            Err(_) => self.close(net, endpoints, 1003),
        }
    }

    fn fetch_steps<D: Dialer>(
        &mut self,
        net: &mut Net<D>,
        endpoints: &mut Endpoints,
        now: i64,
    ) -> io::Result<i32> {
        if self.step == 0 {
            if now - 5000 < self.close_time {
                return Ok(0);
            }
            if let Some(json) = &endpoints.json {
                endpoints.world_list_hostname = json.ip_management.clone();
                endpoints.world_list_port = json.wl_port;
            }
            self.awaiting = false;
            net.open(&endpoints.world_list_hostname, endpoints.world_list_port);
            self.open_time = now;
            self.step = 1;
        }
        if self.open_time + 30000 < now {
            return Ok(self.close(net, endpoints, 1000));
        }
        if self.step == 1 {
            if !self.awaiting {
                match net.poll() {
                    Opened::Failed => return Ok(self.close(net, endpoints, 1001)),
                    Opened::Pending => return Ok(-1),
                    Opened::Ready(()) => {}
                }
                let checksum = if self.loaded { self.checksum } else { 0 };
                let mut request = vec![255];
                request.extend_from_slice(&checksum.to_be_bytes());
                net.outbound = request;
                let bytes = net.outbound.clone();
                net.write(&bytes)?;
                self.awaiting = true;
            }
            let Some(response) = net.read_blocking()? else {
                return Ok(-1);
            };
            self.awaiting = false;
            if response != 0 {
                return Ok(self.close(net, endpoints, response));
            }
            self.step = 2;
        }
        if self.step == 2 {
            if net.available()? < 2 {
                return Ok(-1);
            }
            let length = net.read(2)?;
            self.buffer_len = (usize::from(length[0]) << 8) + usize::from(length[1]);
            self.step = 3;
            self.buffer_off = 0;
            self.buffer = Some(vec![0; self.buffer_len]);
        }
        if self.step != 3 {
            return Ok(-1);
        }
        let mut available = net.available()?;
        if available < 1 {
            return Ok(-1);
        }
        available = available.min(self.buffer_len - self.buffer_off);
        let bytes = net.read(available)?;
        let buffer = self.buffer.get_or_insert_with(Vec::new);
        buffer[self.buffer_off..self.buffer_off + available].copy_from_slice(&bytes);
        self.buffer_off += available;
        if self.buffer_off < self.buffer_len {
            return Ok(-1);
        }
        let data = self.buffer.take().unwrap_or_default();
        if self.decode(&data) {
            self.rebuild_sorted();
            net.close();
            self.errors = 0;
            self.step = 0;
            self.close_time = now;
            Ok(0)
        } else {
            Ok(self.close(net, endpoints, 1002))
        }
    }

    /// `WorldList.close(errorCode)`: drop the connection and either report
    /// the error (fifth failure in a row) or switch to the other port and
    /// report "still trying".
    pub fn close<D: Dialer>(
        &mut self,
        net: &mut Net<D>,
        endpoints: &mut Endpoints,
        error_code: i32,
    ) -> i32 {
        net.close();
        self.awaiting = false;
        self.errors += 1;
        if self.errors > 4 {
            self.step = 0;
            self.errors = 0;
            return error_code;
        }
        self.step = 0;
        endpoints.world_list_port =
            if endpoints.world_list_port == endpoints.world_list_default_port {
                endpoints.world_list_alternate_port
            } else {
                endpoints.world_list_default_port
            };
        -1
    }

    /// `sorted = new World[size]`, filled in world-ID order.
    fn rebuild_sorted(&mut self) {
        let size = usize::try_from(self.size).unwrap_or(0);
        self.sorted = vec![None; size];
        let mut count = 0;
        for (index, world) in self.worlds.iter().enumerate() {
            if world.is_some() && count < size {
                self.sorted[count] = Some(index);
                count += 1;
            }
        }
    }

    /// `WorldList.decode`: false for a payload this client cannot use. On
    /// success the list (when the payload carries one) and the player counts
    /// are replaced.
    pub fn decode(&mut self, data: &[u8]) -> bool {
        self.decode_checked(data).unwrap_or(false)
    }

    fn decode_checked(&mut self, data: &[u8]) -> rs09_protocol::Result<bool> {
        let mut reader = Reader::new(data);
        if reader.g1()? != 1 {
            return Ok(false);
        }
        let decoded = if reader.g1()? == 1 {
            Some(Self::decode_worlds(&mut reader)?)
        } else {
            None
        };
        // decodePlayers, against the list as it will be after this payload.
        let (size, slots) = match &decoded {
            Some(list) => (list.size, list.worlds.len()),
            None => (self.size, self.worlds.len()),
        };
        let mut players = Vec::new();
        for _ in 0..size {
            let index = reader.gsmarts()? as usize;
            let mut count = reader.g2()?;
            if count == 65535 {
                count = -1;
            }
            if index >= slots {
                return Ok(false);
            }
            players.push((index, count));
        }
        if let Some(list) = decoded {
            self.countries = list.countries;
            self.min_id = list.min_id;
            self.max_id = list.max_id;
            self.size = list.size;
            self.worlds = list.worlds;
            self.checksum = list.checksum;
            self.loaded = true;
        }
        for (index, count) in players {
            if let Some(world) = &mut self.worlds[index] {
                world.players = count;
            }
        }
        Ok(true)
    }

    /// `WorldList.decodeWorlds`.
    fn decode_worlds(reader: &mut Reader<'_>) -> rs09_protocol::Result<Decoded> {
        let bad = |reason| rs09_protocol::Error::new(0, reason);
        let country_count = reader.gsmarts()?;
        let mut countries = Vec::new();
        for _ in 0..country_count {
            let flag = reader.gsmarts()?;
            let name = reader.gjstr2()?.to_vec();
            countries.push(Country { flag, name });
        }
        let min_id = reader.gsmarts()?;
        let max_id = reader.gsmarts()?;
        let size = reader.gsmarts()?;
        let slots =
            usize::try_from(max_id + 1 - min_id).map_err(|_| bad("negative world table size"))?;
        let mut worlds: Vec<Option<World>> = vec![None; slots];
        for _ in 0..size {
            let offset = reader.gsmarts()?;
            let country = reader.g1()?;
            let flags = reader.g4()?;
            let activity = reader.gjstr2()?.to_vec();
            let hostname = reader.gjstr2()?.to_vec();
            if country as usize >= countries.len() {
                return Err(bad("world country outside the country table"));
            }
            let slot = worlds
                .get_mut(offset as usize)
                .ok_or_else(|| bad("world outside the world table"))?;
            *slot = Some(World {
                id: offset + min_id,
                country,
                flags,
                players: 0,
                activity,
                hostname,
            });
        }
        let checksum = reader.g4()?;
        Ok(Decoded {
            countries,
            min_id,
            max_id,
            size,
            worlds,
            checksum,
        })
    }

    /// `ScriptRunner.getWorld`.
    pub fn world(&self, world_id: i32) -> Option<&World> {
        if self.loaded && world_id >= self.min_id && world_id <= self.max_id {
            self.worlds
                .get((world_id - self.min_id) as usize)
                .and_then(Option::as_ref)
        } else {
            None
        }
    }

    fn entry(&self, world: &World) -> Entry {
        let country = &self.countries[world.country as usize];
        Entry {
            id: world.id,
            flags: world.flags,
            activity: world.activity.clone(),
            country_flag: country.flag,
            country_name: country.name.clone(),
            players: world.players,
        }
    }

    /// Script command 6506: one world by ID.
    pub fn get(&self, world_id: i32) -> Option<Entry> {
        self.world(world_id).map(|world| self.entry(world))
    }

    /// `WorldList.getFirstWorld` (script command 6501).
    pub fn first_world(&mut self) -> Option<Entry> {
        self.world_pos = 0;
        self.next_world()
    }

    /// `WorldList.getNextWorld` (script command 6502). The position moves
    /// past an empty slot too, as the original's does.
    pub fn next_world(&mut self) -> Option<Entry> {
        if self.sorted.len() > self.world_pos {
            let slot = self.sorted[self.world_pos];
            self.world_pos += 1;
            let world = self.worlds[slot?].as_ref()?;
            Some(self.entry(world))
        } else {
            None
        }
    }

    /// `WorldList.sortWorldList` as script command 6507 calls it: the four
    /// integers in the order the script pushed them. The list is ordered by
    /// `field` first and, where that ties and `tie_field` is not -1, by
    /// `tie_field`. (The original's parameter names have these two swapped.)
    ///
    /// Fields: 1 players, 2 country name, 3 activity, 4 loot share, 5 quick
    /// chat, 6 PvP, 7 members, anything else the world ID.
    pub fn sort(
        &mut self,
        field: i32,
        reverse: bool,
        tie_field: i32,
        tie_reverse: bool,
    ) -> Result<(), EmptySlot> {
        if self.sorted.len() > 1 && self.sorted.iter().any(Option::is_none) {
            return Err(EmptySlot);
        }
        let high = self.sorted.len() as i32 - 1;
        self.quicksort(tie_field, field, high, tie_reverse, 0, reverse);
        Ok(())
    }

    /// `WorldList.quicksort`, with the original's argument order.
    fn quicksort(
        &mut self,
        primary_field: i32,
        secondary_field: i32,
        high: i32,
        secondary_reverse: bool,
        low: i32,
        primary_reverse: bool,
    ) {
        if high <= low {
            return;
        }
        let mid = ((high + low) / 2) as usize;
        let mut store_index = low as usize;
        self.sorted.swap(mid, high as usize);
        let pivot = self.sorted[high as usize];
        for i in low as usize..high as usize {
            if self.compare_worlds(
                pivot,
                self.sorted[i],
                primary_field,
                secondary_field,
                secondary_reverse,
                primary_reverse,
            ) <= 0
            {
                self.sorted.swap(i, store_index);
                store_index += 1;
            }
        }
        self.sorted[high as usize] = self.sorted[store_index];
        self.sorted[store_index] = pivot;
        self.quicksort(
            primary_field,
            secondary_field,
            store_index as i32 - 1,
            secondary_reverse,
            low,
            primary_reverse,
        );
        self.quicksort(
            primary_field,
            secondary_field,
            high,
            secondary_reverse,
            store_index as i32 + 1,
            primary_reverse,
        );
    }

    /// `WorldList.compareWorlds`.
    fn compare_worlds(
        &self,
        a: Option<usize>,
        b: Option<usize>,
        primary_field: i32,
        secondary_field: i32,
        secondary_reverse: bool,
        primary_reverse: bool,
    ) -> i32 {
        let (Some(a), Some(b)) = (
            a.and_then(|slot| self.worlds[slot].as_ref()),
            b.and_then(|slot| self.worlds[slot].as_ref()),
        ) else {
            return 0;
        };
        let result = self.compare_world_field(b, secondary_field, a, primary_reverse);
        if result != 0 {
            if primary_reverse {
                result.wrapping_neg()
            } else {
                result
            }
        } else if primary_field == -1 {
            0
        } else {
            let fallback = self.compare_world_field(b, primary_field, a, secondary_reverse);
            if secondary_reverse {
                fallback.wrapping_neg()
            } else {
                fallback
            }
        }
    }

    /// `WorldList.compareWorldField`.
    fn compare_world_field(&self, a: &World, field: i32, b: &World, reverse: bool) -> i32 {
        let flag = |a: bool, b: bool| {
            if a {
                i32::from(!b)
            } else if b {
                -1
            } else {
                0
            }
        };
        match field {
            1 => {
                let mut players_a = a.players;
                let mut players_b = b.players;
                if !reverse {
                    if players_b == -1 {
                        players_b = 2001;
                    }
                    if players_a == -1 {
                        players_a = 2001;
                    }
                }
                players_a.wrapping_sub(players_b)
            }
            2 => compare_strings(
                &self.countries[a.country as usize].name,
                &self.countries[b.country as usize].name,
            ),
            3 => {
                if a.activity == NO_ACTIVITY {
                    if b.activity == NO_ACTIVITY {
                        0
                    } else if reverse {
                        -1
                    } else {
                        1
                    }
                } else if b.activity == NO_ACTIVITY {
                    if reverse { 1 } else { -1 }
                } else {
                    compare_strings(&a.activity, &b.activity)
                }
            }
            4 => flag(a.is_loot_share(), b.is_loot_share()),
            5 => flag(a.is_quick_chat(), b.is_quick_chat()),
            6 => flag(a.is_pvp(), b.is_pvp()),
            7 => flag(a.is_members(), b.is_members()),
            _ => a.id.wrapping_sub(b.id),
        }
    }
}
