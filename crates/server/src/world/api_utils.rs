//! `core/api/utils`: `WeightedTable`, `PlayerCamera` and `CameraUtils`
//! (`CameraShakeType`), with `CameraContext`'s camera types.
//!
//! The other files of the package live elsewhere: `WeightBasedTable` and
//! `WeightedItem` in `world::drop_tables` (drop-tables-oracle), `Vector` in
//! `world::content_api::api_files` (content-api-oracle). `PlayerStatsCounter`
//! and `Permadeath` are not ported.
//!
//! Differentially tested by api-utils-oracle (docs/api-utils.md).

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// An `IndexOutOfBoundsException` of the original.
    Index,
    /// A failure of a host service.
    Binding(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

// -------------------------------------------------------------- WeightedTable

/// The two generators `WeightedTable.roll` draws from. They are different
/// objects in the original and stay separate services.
pub trait Draws {
    /// `rnd.nextInt(bound)` of `Collections.shuffle(list)`: the JDK's shared
    /// shuffle generator (Kotlin's `shuffled()`), not `RandomFunction.RANDOM`.
    fn shuffle_int(&mut self, bound: i32) -> Result<i32>;
    /// `RandomFunction.RANDOM.nextDouble()`.
    fn next_double(&mut self) -> Result<f64>;
}

/// `Math.min(a, b)`.
fn java_min(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        a
    } else if a == 0.0 && b == 0.0 {
        if a.is_sign_negative() { a } else { b }
    } else if a <= b {
        a
    } else {
        b
    }
}

/// `Math.max(a, b)`.
fn java_max(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        a
    } else if a == 0.0 && b == 0.0 {
        if a.is_sign_negative() { b } else { a }
    } else if a >= b {
        a
    } else {
        b
    }
}

/// `RandomFunction.random(a, b)`: `min + (max - min) * RANDOM.nextDouble()`.
fn random_between(draws: &mut impl Draws, a: f64, b: f64) -> Result<f64> {
    let min = java_min(a, b);
    let max = java_max(a, b);
    Ok(min + (max - min) * draws.next_double()?)
}

/// `WeightedTable<T>`: an `ArrayList<Pair<T?, Double>>` with a running total.
/// `entries` is the list itself: the inherited `ArrayList` members (`size`,
/// `get`, a raw `add(Pair)`, `clear()`) work on it and leave `total_weight`
/// as it was, as they do in the original.
#[derive(Clone, Debug)]
pub struct WeightedTable<T> {
    pub entries: Vec<(Option<T>, f64)>,
    pub total_weight: f64,
}

impl<T> Default for WeightedTable<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            total_weight: 0.0,
        }
    }
}

impl<T: Clone + PartialEq> WeightedTable<T> {
    /// `WeightedTable<T>()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// `WeightedTable.create(vararg elements)`.
    pub fn create(elements: impl IntoIterator<Item = (Option<T>, f64)>) -> Self {
        let mut table = Self::new();
        for (element, weight) in elements {
            table.add(element, weight);
        }
        table
    }

    /// `add(element, weight)`: the total first, then the entry.
    pub fn add(&mut self, element: Option<T>, weight: f64) -> bool {
        self.total_weight += weight;
        self.entries.push((element, weight));
        true
    }

    /// `remove(element)`: the first entry whose element equals it.
    pub fn remove(&mut self, element: Option<&T>) -> bool {
        let Some(index) = self
            .entries
            .iter()
            .position(|(candidate, _)| candidate.as_ref() == element)
        else {
            return false;
        };
        // this.removeAt(index): the weight leaves the total.
        let weight = self.entries[index].1;
        self.total_weight -= weight;
        self.entries.remove(index);
        true
    }

    /// `removeAt(index)`: the entry's weight leaves the total.
    pub fn remove_at(&mut self, index: i32) -> Result<(Option<T>, f64)> {
        let at = usize::try_from(index)
            .ok()
            .filter(|i| *i < self.entries.len())
            .ok_or(Error::Index)?;
        let weight = self.entries[at].1;
        self.total_weight -= weight;
        Ok(self.entries.remove(at))
    }

    /// `roll()`: the only element of a table of one and null for an empty
    /// table, without a draw; otherwise a shuffled copy (the list itself
    /// keeps its order), a weight in `[0, total)`, and the first shuffled
    /// entry that brings it to zero or below. Null when none does.
    pub fn roll(&self, draws: &mut impl Draws) -> Result<Option<T>> {
        if self.entries.len() == 1 {
            return Ok(self.entries[0].0.clone());
        } else if self.entries.is_empty() {
            return Ok(None);
        }
        let mut shuffled = self.entries.clone();
        // Collections.shuffle: for (i = size; i > 1; i--) swap(i - 1, nextInt(i)).
        let mut i = shuffled.len();
        while i > 1 {
            let bound = i32::try_from(i).map_err(|_| Error::Index)?;
            let other = usize::try_from(draws.shuffle_int(bound)?).map_err(|_| Error::Index)?;
            if other >= i {
                return Err(Error::Index);
            }
            shuffled.swap(i - 1, other);
            i -= 1;
        }
        let mut rand_weight = random_between(draws, 0.0, self.total_weight)?;
        for (element, weight) in shuffled {
            rand_weight -= weight;
            if rand_weight <= 0.0 {
                return Ok(element);
            }
        }
        Ok(None)
    }
}

// --------------------------------------------------------------- PlayerCamera

/// `CameraContext.CameraType`, with `opcode()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraType {
    Position,
    Rotation,
    Set,
    Shake,
    Reset,
}

impl CameraType {
    pub const ALL: [Self; 5] = [
        Self::Position,
        Self::Rotation,
        Self::Set,
        Self::Shake,
        Self::Reset,
    ];
    /// `opcode()`.
    pub const fn opcode(self) -> i32 {
        match self {
            Self::Position => 154,
            Self::Rotation => 125,
            Self::Set => 187,
            Self::Shake => 27,
            Self::Reset => 24,
        }
    }
    /// The constant's name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Position => "POSITION",
            Self::Rotation => "ROTATION",
            Self::Set => "SET",
            Self::Shake => "SHAKE",
            Self::Reset => "RESET",
        }
    }
}

/// The numbers of a `CameraContext` (its player is the camera's).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CameraContext {
    pub kind: CameraType,
    pub x: i32,
    pub y: i32,
    pub height: i32,
    pub speed: i32,
    pub zoom_speed: i32,
}

pub trait CameraHost {
    type Player;
    /// `PacketRepository.send(CameraViewPacket::class.java, ctx)`.
    fn send_camera(&mut self, player: &Self::Player, context: &CameraContext) -> Result<()>;
}

/// `PlayerCamera(player)`: every member does nothing without a player.
#[derive(Clone, Debug)]
pub struct PlayerCamera<P> {
    pub player: Option<P>,
    /// The last context made (kept when its packet fails).
    pub ctx: Option<CameraContext>,
}

impl<P> PlayerCamera<P> {
    pub fn new(player: Option<P>) -> Self {
        Self { player, ctx: None }
    }

    fn send<H: CameraHost<Player = P>>(
        &mut self,
        h: &mut H,
        kind: CameraType,
        numbers: [i32; 5],
    ) -> Result<()> {
        let Some(player) = &self.player else {
            return Ok(());
        };
        let [x, y, height, speed, zoom_speed] = numbers;
        let context = CameraContext {
            kind,
            x,
            y,
            height,
            speed,
            zoom_speed,
        };
        self.ctx = Some(context);
        h.send_camera(player, &context)
    }

    /// `setPosition(x, y, height)`.
    pub fn set_position<H: CameraHost<Player = P>>(
        &mut self,
        h: &mut H,
        x: i32,
        y: i32,
        height: i32,
    ) -> Result<()> {
        self.send(h, CameraType::Set, [x, y, height, 0, 0])
    }

    /// `rotateTo(x, y, height, speed)`.
    pub fn rotate_to<H: CameraHost<Player = P>>(
        &mut self,
        h: &mut H,
        x: i32,
        y: i32,
        height: i32,
        speed: i32,
    ) -> Result<()> {
        self.send(h, CameraType::Rotation, [x, y, height, speed, 1])
    }

    /// `rotateBy(diffX, diffY, diffHeight, speed)`: from the last context of
    /// any type; nothing without one.
    pub fn rotate_by<H: CameraHost<Player = P>>(
        &mut self,
        h: &mut H,
        diff_x: i32,
        diff_y: i32,
        diff_height: i32,
        speed: i32,
    ) -> Result<()> {
        if self.player.is_none() {
            return Ok(());
        }
        let Some(last) = self.ctx else {
            return Ok(());
        };
        self.send(
            h,
            CameraType::Rotation,
            [
                last.x.wrapping_add(diff_x),
                last.y.wrapping_add(diff_y),
                last.height.wrapping_add(diff_height),
                speed,
                1,
            ],
        )
    }

    /// `panTo(x, y, height, speed)`.
    pub fn pan_to<H: CameraHost<Player = P>>(
        &mut self,
        h: &mut H,
        x: i32,
        y: i32,
        height: i32,
        speed: i32,
    ) -> Result<()> {
        self.send(h, CameraType::Position, [x, y, height, speed, 1])
    }

    /// `shake(cameraType, jitter, amplitude, frequency, speed)`: the five
    /// numbers fill the context's x, y, height, speed and zoom speed.
    pub fn shake<H: CameraHost<Player = P>>(
        &mut self,
        h: &mut H,
        camera_type: i32,
        jitter: i32,
        amplitude: i32,
        frequency: i32,
        speed: i32,
    ) -> Result<()> {
        self.send(
            h,
            CameraType::Shake,
            [camera_type, jitter, amplitude, frequency, speed],
        )
    }

    /// `reset()`.
    pub fn reset<H: CameraHost<Player = P>>(&mut self, h: &mut H) -> Result<()> {
        self.send(h, CameraType::Reset, [-1, -1, -1, -1, -1])
    }
}

/// `CameraShakeType` (CameraUtils.kt; the `CameraUtils` object is empty).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraShakeType {
    Truck,
    Pedestal,
    Dolly,
    Pan,
    Tilt,
}

impl CameraShakeType {
    pub const ALL: [Self; 5] = [
        Self::Truck,
        Self::Pedestal,
        Self::Dolly,
        Self::Pan,
        Self::Tilt,
    ];
    /// `ordinal`.
    pub const fn ordinal(self) -> i32 {
        self as i32
    }
    /// The constant's name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Truck => "TRUCK",
            Self::Pedestal => "PEDESTAL",
            Self::Dolly => "DOLLY",
            Self::Pan => "PAN",
            Self::Tilt => "TILT",
        }
    }
}
