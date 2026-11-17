//! Original door rules and captured autowalk pulse bodies. Scene replacement,
//! collision, path queues, player services and scheduler submission are bindings.
use crate::{definitions::item::Shared, player::attributes::AttributeLocation, pulses};
use rs09_game_core::{
    direction::Direction,
    java_json::{self, Json},
    java_string,
};
use std::{
    collections::HashMap,
    fs::File,
    io::{Cursor, Read},
    path::Path,
};
pub type Location = AttributeLocation;
pub type Entity = usize;
#[derive(Debug)]
pub enum Error {
    Null,
    Type,
    Number,
    Index,
    Direction,
    Json(java_json::Error),
    Io(std::io::Error),
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Debug)]
pub struct DoorConfig {
    pub id: i32,
    pub replace_id: i32,
    pub fence: bool,
    pub metal: bool,
    pub autowalk: bool,
    pub quest_requirement: String,
}
pub type Door = Shared<DoorConfig>;
impl DoorConfig {
    pub fn new(id: i32) -> Self {
        Self {
            id,
            replace_id: 0,
            fence: false,
            metal: false,
            autowalk: false,
            quest_requirement: String::new(),
        }
    }
}
#[derive(Clone)]
pub struct Registry {
    pub values: Shared<HashMap<i32, Door>>,
}
impl Default for Registry {
    fn default() -> Self {
        Self {
            values: Shared::new(HashMap::new()),
        }
    }
}
impl Registry {
    pub fn get(&self, id: i32) -> Option<Door> {
        self.values.read().get(&id).cloned()
    }
    pub fn apply(&self, data: &Json, log: &mut impl FnMut(i32) -> Result<()>) -> Result<i32> {
        let Json::Array(rows) = data else {
            return Err(if matches!(data, Json::Null) {
                Error::Null
            } else {
                Error::Type
            });
        };
        let mut count = 0i32;
        for row in rows {
            if !matches!(row, Json::Object(_)) {
                return Err(if matches!(row, Json::Null) {
                    Error::Null
                } else {
                    Error::Type
                });
            }
            let text = |key| row.get(key).unwrap_or(&Json::Null).text();
            let id = java_string::parse_i32(&text("id")).ok_or(Error::Number)?;
            let mut door = DoorConfig::new(id);
            door.replace_id = java_string::parse_i32(&text("replaceId")).ok_or(Error::Number)?;
            door.fence = java_string::equals_ignore_case(&text("fence"), "true");
            door.metal = java_string::equals_ignore_case(&text("metal"), "true");
            door.autowalk = java_string::equals_ignore_case(&text("autowalk"), "true");
            let id2 = door.replace_id;
            let reverse = DoorConfig {
                id: id2,
                replace_id: id,
                fence: door.fence,
                metal: door.metal,
                autowalk: door.autowalk,
                quest_requirement: String::new(),
            };
            self.values.write().insert(id, Shared::new(door));
            self.values.write().insert(id2, Shared::new(reverse));
            count = count.wrapping_add(1);
        }
        log(count)?;
        Ok(count)
    }
}
pub enum ConfigReader {
    File(File),
    Text(Cursor<Vec<u8>>),
}
impl Read for ConfigReader {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::File(f) => f.read(b),
            Self::Text(c) => c.read(b),
        }
    }
}
#[derive(Default)]
pub struct Loader {
    pub reader: Option<ConfigReader>,
}
impl Loader {
    pub fn load_file(
        &mut self,
        path: &Path,
        registry: &Registry,
        log: &mut impl FnMut(i32) -> Result<()>,
    ) -> Result<i32> {
        let f = File::open(path).map_err(Error::Io)?;
        if f.metadata().map_err(Error::Io)?.is_dir() {
            return Err(Error::Io(std::io::ErrorKind::IsADirectory.into()));
        }
        self.reader = Some(ConfigReader::File(f));
        self.load(registry, log)
    }
    pub fn load_text(
        &mut self,
        text: &str,
        registry: &Registry,
        log: &mut impl FnMut(i32) -> Result<()>,
    ) -> Result<i32> {
        self.reader = Some(ConfigReader::Text(Cursor::new(text.as_bytes().to_vec())));
        self.load(registry, log)
    }
    fn load(
        &mut self,
        registry: &Registry,
        log: &mut impl FnMut(i32) -> Result<()>,
    ) -> Result<i32> {
        let mut text = String::new();
        self.reader
            .as_mut()
            .ok_or(Error::Null)?
            .take(64 * 1024 * 1024 + 1)
            .read_to_string(&mut text)
            .map_err(Error::Io)?;
        let data = java_json::parse(&text).map_err(Error::Json)?;
        registry.apply(&data, log)
    }
}

/// State consumed by these rules. Complete definition/Node/scenery APIs remain
/// separate; definition creation and transforms use the caller's native factory.
pub struct Scenery {
    pub id: i32,
    pub shape: i32,
    pub rotation: i32,
    pub direction: Direction,
    pub location: Location,
    pub name: Option<String>,
    pub close: bool,
    pub charge: i32,
    pub constructed: bool,
    pub replaced: Option<Handle>,
    pub wrapper: Option<Handle>,
    pub active: bool,
    pub renderable: bool,
    pub restore_pulse: Option<pulses::Handle>,
    pub destruction_pulse: Option<pulses::Handle>,
    pub definition: Option<crate::definitions::scenery::Handle>,
    pub node_size: i32,
    pub attributes: Shared<crate::player::attributes::GameAttributes>,
    pub(crate) interaction: Option<super::scenery_options::Binding>,
    pub(crate) children: Option<Shared<Vec<Handle>>>,
    pub(crate) parent_wrapper: Option<crate::definitions::item::WeakShared<Scenery>>,
}
pub type Handle = Shared<Scenery>;
impl Scenery {
    pub fn new(
        id: i32,
        location: Location,
        shape: i32,
        mut rotation: i32,
        name: Option<String>,
        close: bool,
    ) -> Result<Self> {
        if rotation < 0 {
            rotation = rotation.wrapping_add(4);
        }
        Ok(Self {
            id,
            shape: if id < 1 { 22 } else { shape },
            rotation,
            direction: Direction::from_code(rotation).ok_or(Error::Direction)?,
            location,
            name,
            close,
            charge: 1000,
            constructed: false,
            replaced: None,
            wrapper: None,
            active: true,
            renderable: true,
            restore_pulse: None,
            destruction_pulse: None,
            definition: None,
            node_size: 1,
            attributes: Shared::new(crate::player::attributes::GameAttributes::default()),
            interaction: None,
            children: None,
            parent_wrapper: None,
        })
    }
}
fn id(h: &Handle) -> i32 {
    h.read().id
}
fn rot(h: &Handle) -> i32 {
    h.read().rotation
}
fn shape(h: &Handle) -> i32 {
    h.read().shape
}
fn loc(h: &Handle) -> Location {
    h.read().location.clone()
}
pub fn wrapper(h: &Handle) -> Handle {
    let (explicit, parent) = {
        let h = h.read();
        (h.wrapper.clone(), h.parent_wrapper.clone())
    };
    explicit
        .or_else(|| parent.and_then(|p| p.upgrade()).map(|p| p.keeping_from(h)))
        .unwrap_or_else(|| h.clone())
}
fn close_action(h: &Handle) -> bool {
    let (definition, close) = {
        let h = h.read();
        (h.definition.clone(), h.close)
    };
    definition.map_or(close, |d| d.read().has_action(Some("close")))
}
fn transform(l: &Location, x: i32, y: i32) -> Location {
    let [a, b, z] = l.coordinates();
    Location::new(a.wrapping_add(x), b.wrapping_add(y), z)
}
fn rotation_add(v: i32, d: i32) -> i32 {
    v.wrapping_add(d) % 4
}
#[derive(Clone)]
pub enum Action {
    Autowalk {
        entity: Entity,
        object: Handle,
        second: Option<Handle>,
        end: Location,
        opened: bool,
    },
    Fence {
        entity: Entity,
        object: Handle,
        second: Handle,
        replace_id: i32,
        second_replace_id: i32,
        opened: bool,
    },
}
#[derive(Clone)]
pub struct DoorPulse {
    pub pulse: pulses::Handle,
    pub action: Shared<Action>,
}
impl DoorPulse {
    fn new(action: Action, name: &str) -> Self {
        Self {
            pulse: Shared::new(pulses::Pulse::new(
                1,
                None,
                pulses::Class::Other,
                name.into(),
            )),
            action: Shared::new(action),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Diary {
    Falador,
    Lumbridge,
}
pub trait Context {
    fn config(&mut self, id: i32) -> Result<Option<Door>>;
    fn object(&mut self, location: Location, shape: i32) -> Result<Option<Handle>>;
    fn construct(
        &mut self,
        id: i32,
        location: Location,
        shape: i32,
        rotation: i32,
    ) -> Result<Handle>;
    /// None is replace(remove,construct); Some(ticks) uses its temporary overload.
    fn replace(
        &mut self,
        remove: &Handle,
        construct: Handle,
        ticks: Option<i32>,
        clip: bool,
    ) -> Result<bool>;
    fn is_player(&mut self, entity: Entity) -> bool;
    fn entity_location(&mut self, entity: Entity) -> Result<Location>;
    fn lock(&mut self, entity: Entity, ticks: i32) -> Result<()>;
    fn audio(&mut self, entity: Entity, id: i32, delay: Option<i32>) -> Result<()>;
    fn message(&mut self, entity: Entity, text: &str) -> Result<()>;
    /// Install/remove the original "autowalk" logout listener. The location
    /// handle is shared, so logout restores the captured reference, not a copy.
    fn logout_listener(&mut self, entity: Entity, location: Option<Location>) -> Result<()>;
    fn reset_path(&mut self, entity: Entity) -> Result<()>;
    fn add_path(&mut self, entity: Entity, x: i32, y: i32) -> Result<()>;
    fn path_move_near(&mut self, entity: Entity, location: Location) -> Result<bool>;
    fn diary(&mut self, entity: Entity, diary: Diary, level: i32, task: i32) -> Result<()>;
    fn submit(&mut self, pulse: DoorPulse) -> Result<()>;
}
fn new_from(
    ctx: &mut impl Context,
    h: &Handle,
    id: i32,
    rotation: i32,
    location: Location,
) -> Result<Handle> {
    ctx.construct(id, location, shape(h), rotation)
}
pub fn rotation_point(rotation: i32) -> Option<[i32; 2]> {
    match rotation {
        0 => Some([-1, 0]),
        1 => Some([0, 1]),
        2 => Some([1, 0]),
        3 => Some([0, -1]),
        _ => None,
    }
}
fn close_rotation(h: &Handle) -> [i32; 2] {
    match rot(h) {
        0 => [0, 1],
        1 => [1, 0],
        2 => [0, -1],
        3 => [-1, 0],
        _ => [0, 0],
    }
}
pub fn second_door(ctx: &mut impl Context, object: &Handle) -> Result<Option<Handle>> {
    let l = loc(object);
    for [x, y] in [[-1, 0], [1, 0], [0, -1], [0, 1]] {
        if let Some(o) = ctx.object(transform(&l, x, y), shape(object))? {
            let name = o.read().name.clone().ok_or(Error::Null)?;
            if Some(name) == object.read().name {
                return Ok(Some(o));
            }
        }
    }
    Ok(None)
}
pub fn handle(ctx: &mut impl Context, player: Entity, object: &Handle) -> Result<()> {
    let ignore = matches!(id(object), 1512 | 1515);
    let second = if matches!(id(object), 1530 | 1531) || ignore {
        None
    } else {
        second_door(ctx, object)?
    };
    let previous = if object.read().constructed {
        object.read().replaced.clone()
    } else {
        None
    };
    if let Some(previous) = previous {
        let d = ctx.config(id(object))?;
        let sound = d.as_ref().map_or(60, |d| {
            let d = d.read();
            if d.metal {
                70
            } else if d.fence {
                66
            } else {
                60
            }
        });
        ctx.audio(player, sound, None)?;
        ctx.replace(object, previous, None, true)?;
        if let Some(second) = second {
            let previous = if second.read().constructed {
                second.read().replaced.clone()
            } else {
                None
            };
            if let Some(previous) = previous {
                ctx.replace(&second, previous, None, true)?;
            }
        }
        return Ok(());
    }
    if close_action(object) {
        if second.is_some() {
            ctx.message(player, "The doors appear to be stuck.")?;
            ctx.audio(player, 61, None)?;
            return Ok(());
        }
        let d = ctx.config(id(object))?;
        play_open(ctx, player, d.as_ref(), None)?;
        let Some(d) = d else {
            ctx.message(player, "The door appears to be stuck.")?;
            ctx.audio(player, 61, None)?;
            return Ok(());
        };
        let first_dir = rotation_add(rot(object), 3);
        let [x, y] = close_rotation(object);
        let first = transform(&loc(object), x, y);
        let replace_id = d.read().replace_id;
        let new = new_from(ctx, object, replace_id, first_dir, first)?;
        ctx.replace(object, new, None, true)?;
        return Ok(());
    }
    let d = ctx.config(id(object))?;
    if d.as_ref().is_none_or(|d| d.read().autowalk) {
        let end = end_location(ctx, player, object)?;
        autowalk(ctx, player, object, end)?;
        return Ok(());
    }
    play_open(ctx, player, d.as_ref(), None)?;
    let d = d.unwrap();
    if let Some(second) = second {
        let other = ctx.config(id(&second))?;
        let second_id = other
            .as_ref()
            .map_or_else(|| id(&second), |s| s.read().replace_id);
        let (replace_id, fence) = {
            let d = d.read();
            (d.replace_id, d.fence)
        };
        open(
            ctx,
            object,
            Some(&second),
            replace_id,
            second_id,
            true,
            500,
            fence,
        )?;
    } else {
        let (replace_id, fence) = {
            let d = d.read();
            (d.replace_id, d.fence)
        };
        open(ctx, object, None, replace_id, -1, true, 500, fence)?;
    }
    Ok(())
}
fn play_open(
    ctx: &mut impl Context,
    entity: Entity,
    d: Option<&Door>,
    delay: Option<i32>,
) -> Result<()> {
    let sound = d.map_or(62, |d| {
        let d = d.read();
        if d.metal {
            71
        } else if d.fence {
            67
        } else {
            62
        }
    });
    ctx.audio(entity, sound, delay)
}
pub fn end_location(ctx: &mut impl Context, entity: Entity, object: &Handle) -> Result<Location> {
    let mut l = loc(object);
    match rot(object) {
        0 => {
            if ctx.entity_location(entity)?.coordinates()[0] >= l.coordinates()[0] {
                l = transform(&l, -1, 0);
            }
        }
        1 => {
            if ctx.entity_location(entity)?.coordinates()[1] <= l.coordinates()[1] {
                l = transform(&l, 0, 1);
            }
        }
        2 => {
            if ctx.entity_location(entity)?.coordinates()[0] <= l.coordinates()[0] {
                l = transform(&l, 1, 0);
            }
        }
        _ => {
            if ctx.entity_location(entity)?.coordinates()[1] >= l.coordinates()[1] {
                l = transform(&l, 0, -1);
            }
        }
    }
    Ok(l)
}
pub fn destination(ctx: &mut impl Context, entity: Entity, door: &Handle) -> Result<Location> {
    let mut l = loc(door);
    let mut rotation = rot(door);
    if door.read().constructed && close_action(door) {
        if let Some(o) = door.read().replaced.clone() {
            l = loc(&o);
            rotation = rot(&o);
        }
    }
    if shape(door) == 9 {
        match rotation {
            0 | 2 => {
                if ctx.entity_location(entity)?.coordinates()[1] < l.coordinates()[1]
                    || ctx.entity_location(entity)?.coordinates()[0] < l.coordinates()[0]
                {
                    return Ok(transform(&l, 0, -1));
                }
                return Ok(transform(&l, 0, 1));
            }
            1 | 3 => {
                if ctx.entity_location(entity)?.coordinates()[0] > l.coordinates()[0]
                    || ctx.entity_location(entity)?.coordinates()[1] > l.coordinates()[1]
                {
                    return Ok(transform(&l, 1, 0));
                }
                return Ok(transform(&l, -1, 0));
            }
            _ => {}
        }
    }
    let offset = match rotation {
        0 if ctx.entity_location(entity)?.coordinates()[0] < l.coordinates()[0] => Some([-1, 0]),
        1 if ctx.entity_location(entity)?.coordinates()[1] > l.coordinates()[1] => Some([0, 1]),
        2 if ctx.entity_location(entity)?.coordinates()[0] > l.coordinates()[0] => Some([1, 0]),
        3 if ctx.entity_location(entity)?.coordinates()[1] < l.coordinates()[1] => Some([0, -1]),
        _ => None,
    };
    if let Some([x, y]) = offset {
        if ctx.path_move_near(entity, transform(&l, x, y))? {
            return Ok(transform(&l, 0, 0));
        }
        return Ok(transform(&l, x, y));
    }
    Ok(l)
}
#[allow(clippy::too_many_arguments)]
pub fn open(
    ctx: &mut impl Context,
    object: &Handle,
    second: Option<&Handle>,
    mut replace_id: i32,
    second_replace_id: i32,
    clip: bool,
    restore_ticks: i32,
    fence: bool,
) -> Result<()> {
    let object = wrapper(object);
    let modifier = if shape(&object) == 9 { -1 } else { 1 };
    let mut first_dir = rotation_add(rot(&object), (modifier + 4) % 4);
    let [x, y] = rotation_point(rot(&object)).ok_or(Error::Null)?;
    let mut first_loc = transform(&loc(&object), x * modifier, y * modifier);
    let Some(second) = second else {
        if replace_id == 4577 {
            replace_id = 4578;
            first_dir = 3;
            first_loc = transform(&first_loc, 0, 1);
        }
        let new = new_from(ctx, &object, replace_id, first_dir, first_loc)?;
        ctx.replace(&object, new, Some(restore_ticks), clip)?;
        return Ok(());
    };
    let second = wrapper(second);
    if fence {
        return open_fence(
            ctx,
            &object,
            &second,
            replace_id,
            second_replace_id,
            clip,
            restore_ticks,
        );
    }
    let a = loc(&object).coordinates();
    let b = loc(&second).coordinates();
    let offset = Direction::from_delta(b[0].wrapping_sub(a[0]), b[1].wrapping_sub(a[1]));
    let mut second_dir = rotation_add(rot(&second), modifier);
    first_dir = match (first_dir, offset) {
        (1, Direction::North) => 3,
        (2, Direction::East) => 0,
        (3, Direction::South) => 1,
        (0, Direction::West) => 2,
        _ => first_dir,
    };
    if first_dir == second_dir {
        second_dir = rotation_add(second_dir, 2);
    }
    let second_loc = transform(&loc(&second), x, y);
    let new = new_from(ctx, &object, replace_id, first_dir, first_loc)?;
    ctx.replace(&object, new, Some(restore_ticks), clip)?;
    let new = new_from(ctx, &second, second_replace_id, second_dir, second_loc)?;
    ctx.replace(&second, new, Some(restore_ticks), clip)?;
    Ok(())
}
fn open_fence(
    ctx: &mut impl Context,
    object: &Handle,
    second: &Handle,
    mut replace_id: i32,
    mut second_replace_id: i32,
    clip: bool,
    restore_ticks: i32,
) -> Result<()> {
    let a = loc(object).coordinates();
    let b = loc(second).coordinates();
    let offset = Direction::from_delta(b[0].wrapping_sub(a[0]), b[1].wrapping_sub(a[1]));
    let mut first_dir = rotation_add(rot(object), 3);
    let [x, y] = rotation_point(rot(object)).ok_or(Error::Null)?;
    let mut second_dir = rotation_add(rot(second), 3);
    let first_loc = if matches!(offset, Direction::West | Direction::South) {
        std::mem::swap(&mut replace_id, &mut second_replace_id);
        transform(&loc(second), x, y)
    } else {
        transform(&loc(object), x, y)
    };
    if matches!(rot(object), 3 | 2) {
        first_dir = rotation_add(first_dir, 2);
        second_dir = rotation_add(second_dir, 2);
    }
    let second_loc = transform(&first_loc, x, y);
    let special = matches!(id(object), 36917 | 36919);
    if special {
        (replace_id, second_replace_id) = if object.read().direction == Direction::East {
            (36917, 36919)
        } else {
            (36919, 36917)
        };
    }
    let new = new_from(ctx, object, replace_id, first_dir, first_loc)?;
    ctx.replace(object, new, Some(restore_ticks), clip || special)?;
    let new = new_from(ctx, second, second_replace_id, second_dir, second_loc)?;
    ctx.replace(second, new, Some(restore_ticks), clip || special)?;
    Ok(())
}
pub fn rotations(object: &Handle, second: Option<&Handle>, point: [i32; 2]) -> Vec<i32> {
    let Some(second) = second else {
        return vec![rotation_add(rot(object), 1)];
    };
    let a = loc(object).coordinates();
    let b = loc(second).coordinates();
    let mut result = vec![3, 1];
    if a[0] > b[0] {
        result = vec![2, 0];
    }
    if a[0] < b[0] {
        result = vec![0, 2];
    }
    if a[1] > b[1] {
        result = vec![1, 3];
    }
    if point[1] > 0 || point[0] > 0 {
        result.swap(0, 1);
    }
    result
}
pub fn autowalk(
    ctx: &mut impl Context,
    entity: Entity,
    object: &Handle,
    end: Location,
) -> Result<bool> {
    if object.read().charge == 88 {
        return Ok(false);
    }
    let second = if matches!(
        id(object),
        3 | 3628 | 3629 | 3630 | 3631 | 3632 | 4545 | 4546
    ) {
        None
    } else {
        second_door(ctx, object)?
    };
    ctx.lock(entity, 4)?;
    let location = ctx.entity_location(entity)?;
    if ctx.is_player(entity) {
        let d = ctx.config(id(object))?;
        let (open_sound, close_sound) = d.as_ref().map_or((62, 60), |d| {
            let d = d.read();
            if d.metal {
                (71, 70)
            } else if d.fence {
                (67, 66)
            } else {
                (62, 60)
            }
        });
        ctx.audio(entity, open_sound, None)?;
        ctx.audio(entity, close_sound, Some(60))?;
        ctx.logout_listener(entity, Some(location))?;
    }
    ctx.submit(DoorPulse::new(
        Action::Autowalk {
            entity,
            object: object.clone(),
            second,
            end,
            opened: false,
        },
        "core.game.global.action.DoorActionHandler$1",
    ))?;
    Ok(true)
}
pub fn autowalk_fence(
    ctx: &mut impl Context,
    entity: Entity,
    object: &Handle,
    replace_id: i32,
    second_replace_id: i32,
) -> Result<bool> {
    let second = second_door(ctx, object)?;
    if object.read().charge == 88 || second.is_none() {
        return Ok(false);
    }
    let second = second.unwrap();
    ctx.lock(entity, 4)?;
    let location = ctx.entity_location(entity)?;
    if ctx.is_player(entity) {
        ctx.logout_listener(entity, Some(location))?;
    }
    object.write().charge = 88;
    second.write().charge = 88;
    ctx.submit(DoorPulse::new(
        Action::Fence {
            entity,
            object: object.clone(),
            second,
            replace_id,
            second_replace_id,
            opened: false,
        },
        "core.game.global.action.DoorActionHandler$2",
    ))?;
    Ok(true)
}
fn within(a: &Location, b: &Location, d: i32) -> bool {
    let [x, y, z] = a.coordinates();
    let [u, v, w] = b.coordinates();
    if z != w {
        return false;
    }
    let dx = x.wrapping_sub(u);
    let dy = y.wrapping_sub(v);
    let squared = dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy));
    f64::from(squared).sqrt() <= f64::from(d)
}
/// The anonymous pulse's body; schedule its base Pulse with the existing native
/// runner. A caller invokes this from Context::pulse, preserving runner ordering.
pub fn pulse_body(ctx: &mut impl Context, task: &DoorPulse) -> Result<bool> {
    let action = task.action.read().clone();
    match action {
        Action::Autowalk {
            entity,
            object,
            second,
            end,
            opened,
        } => {
            if !opened {
                open(
                    ctx,
                    &object,
                    second.as_ref(),
                    id(&object),
                    second.as_ref().map_or(-1, id),
                    false,
                    2,
                    false,
                )?;
                ctx.reset_path(entity)?;
                let [x, y, _] = end.coordinates();
                ctx.add_path(entity, x, y)?;
                if let Action::Autowalk { opened, .. } = &mut *task.action.write() {
                    *opened = true;
                }
                object.write().charge = 88;
                if let Some(second) = second {
                    second.write().charge = 88;
                }
                return Ok(false);
            }
            if ctx.is_player(entity) {
                if id(&object) == 2112
                    && within(
                        &ctx.entity_location(entity)?,
                        &Location::new(3046, 9756, 0),
                        10,
                    )
                {
                    ctx.diary(entity, Diary::Falador, 2, 6)?;
                }
                if id(&object) == 35549
                    || (id(&object) == 35551 && {
                        let [x, y, _] = ctx.entity_location(entity)?.coordinates();
                        ((x >> 6) << 8) | (y >> 6) == 13106
                    })
                {
                    ctx.diary(entity, Diary::Lumbridge, 0, 4)?;
                }
                if id(&object) == 2406
                    && within(
                        &ctx.entity_location(entity)?,
                        &Location::new(3202, 3169, 0),
                        15,
                    )
                {
                    ctx.diary(entity, Diary::Lumbridge, 1, 6)?;
                }
                ctx.logout_listener(entity, None)?;
            }
            object.write().charge = 1000;
            if let Some(second) = second {
                second.write().charge = 1000;
            }
            Ok(true)
        }
        Action::Fence {
            entity,
            object,
            second,
            replace_id,
            second_replace_id,
            opened,
        } => {
            if !opened {
                open_fence(
                    ctx,
                    &object,
                    &second,
                    replace_id,
                    second_replace_id,
                    false,
                    2,
                )?;
                let end = end_location(ctx, entity, &object)?;
                ctx.reset_path(entity)?;
                let [x, y, _] = end.coordinates();
                ctx.add_path(entity, x, y)?;
                if let Action::Fence { opened, .. } = &mut *task.action.write() {
                    *opened = true;
                }
                return Ok(false);
            }
            if ctx.is_player(entity) {
                ctx.logout_listener(entity, None)?;
            }
            object.write().charge = 1000;
            second.write().charge = 1000;
            Ok(true)
        }
    }
}
