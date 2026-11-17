//! Pinned server ItemDefinition cache decoder and ordered note/lend/recolour transfers.
//!
//! Handles retain Java's shared definition/array identity. Mutation is explicit; no
//! lock is held while another definition is read, including self-reference cases.
use super::property::Property;
use rs09_cache::{CacheError, CacheResult, archive::Archive};
use rs09_protocol::{Error, Result, buffer::Reader};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::{
    any::Any,
    collections::{BTreeMap, HashMap},
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard, Weak},
};

struct KeepAlive(Box<dyn OwnerValue>);
struct WeakKeepAlive(Box<dyn WeakOwner>);
trait OwnerValue: Send + Sync {
    fn downgrade(&self) -> WeakKeepAlive;
}
trait WeakOwner: Send + Sync {
    fn upgrade(&self) -> Option<KeepAlive>;
}
struct Owner<T>(Arc<RwLock<T>>);
struct WeakOwnerValue<T>(Weak<RwLock<T>>);
impl<T: Any + Send + Sync> OwnerValue for Owner<T> {
    fn downgrade(&self) -> WeakKeepAlive {
        WeakKeepAlive(Box::new(WeakOwnerValue(Arc::downgrade(&self.0))))
    }
}
impl<T: Any + Send + Sync> WeakOwner for WeakOwnerValue<T> {
    fn upgrade(&self) -> Option<KeepAlive> {
        self.0
            .upgrade()
            .map(|owner| KeepAlive(Box::new(Owner(owner))))
    }
}
/// Shared value identity, optionally retaining an owning graph for an exposed
/// child view. The owner is never part of serialization or value identity.
pub struct Shared<T>(Arc<RwLock<T>>, Option<Arc<KeepAlive>>);
pub struct WeakShared<T>(Weak<RwLock<T>>, Option<Arc<WeakKeepAlive>>);
impl<T> Clone for WeakShared<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1.clone())
    }
}
impl<T> WeakShared<T> {
    pub fn upgrade(&self) -> Option<Shared<T>> {
        let value = self.0.upgrade()?;
        let owner = match &self.1 {
            Some(owner) => Some(Arc::new(owner.0.upgrade()?)),
            None => None,
        };
        Some(Shared(value, owner))
    }
}
impl<T: std::fmt::Debug> std::fmt::Debug for Shared<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0), self.1.clone())
    }
}
impl<T> Shared<T> {
    pub fn new(value: T) -> Self {
        Self(Arc::new(RwLock::new(value)), None)
    }
    pub fn downgrade(&self) -> WeakShared<T> {
        WeakShared(
            Arc::downgrade(&self.0),
            self.1.as_ref().map(|owner| Arc::new(owner.0.downgrade())),
        )
    }
    /// Native ownership accounting, not a Java-visible gameplay value. Owners
    /// subtract their known graph edges before detecting externally retained
    /// handles; use this only while the graph's owner serializes its mutation.
    pub fn strong_count(&self) -> usize {
        Arc::strong_count(&self.0)
    }
    pub(crate) fn has_owner(&self) -> bool {
        self.1.is_some()
    }
    /// Store an internal graph edge without copying a returned view's owner
    /// anchor back into that owner. The owning graph traces this edge itself.
    pub(crate) fn unowned(&self) -> Self {
        Self(self.0.clone(), None)
    }
    /// Attach ownership to a returned view, not to the internally stored edge.
    pub fn keeping_from<U: Any + Send + Sync>(&self, owner: &Shared<U>) -> Self {
        // Reuse a family's existing anchor. Repeated child/wrapper traversal
        // must not grow a chain of view owners.
        let anchor = owner
            .1
            .clone()
            .unwrap_or_else(|| Arc::new(KeepAlive(Box::new(Owner(owner.0.clone())))));
        Self(self.0.clone(), Some(anchor))
    }
    pub fn read(&self) -> RwLockReadGuard<'_, T> {
        self.0
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    pub fn write(&self) -> RwLockWriteGuard<'_, T> {
        self.0
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    /// Process-local identity for alias diagnostics; never persist this address.
    pub fn identity(&self) -> usize {
        Arc::as_ptr(&self.0) as usize
    }
}
impl<T: Serialize> Serialize for Shared<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        self.read().serialize(serializer)
    }
}
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Shared<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        Ok(Self::new(T::deserialize(deserializer)?))
    }
}

pub type Actions = Option<Shared<Vec<Option<String>>>>;
pub type Shorts = Option<Shared<Vec<i16>>>;
pub type Integers = Option<Shared<Vec<i32>>>;
pub type IntegerRows = Option<Shared<Vec<Option<Shared<Vec<i32>>>>>>;
pub type Handle = Shared<ItemDefinition>;

/// All original stored fields. Content values retain their native scalar, array,
/// requirement, animation and audio types. Plugin objects, shop policy and
/// equipment checks are separate ports.
#[derive(Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ItemDefinition {
    pub id: i32,
    pub name: Option<String>,
    pub examine: Option<String>,
    pub options: Actions,
    pub handlers: BTreeMap<String, Property>,
    pub interface_model_id: i32,
    pub model_zoom: i32,
    pub model_rotation_x: i32,
    pub model_rotation_y: i32,
    pub model_offset_x: i32,
    pub model_offset_y: i32,
    pub stackable: bool,
    pub value: i32,
    pub members_only: bool,
    pub male_worn_model_id1: i32,
    pub female_worn_model_id1: i32,
    pub male_worn_model_id2: i32,
    pub female_worn_model_id2: i32,
    pub male_worn_model_id3: i32,
    pub female_worn_model_id3: i32,
    pub male_worn_model_id4: i32,
    pub female_worn_model_id4: i32,
    pub ground_actions: Actions,
    pub original_model_colors: Shorts,
    pub modified_model_colors: Shorts,
    pub texture_colour1: Shorts,
    pub texture_colour2: Shorts,
    pub unknown_array1: Option<Shared<Vec<i8>>>,
    pub unknown_array2: Integers,
    pub unknown_array3: IntegerRows,
    pub unnoted: bool,
    pub colour_equip1: i32,
    pub colour_equip2: i32,
    pub note_id: i32,
    pub note_template_id: i32,
    pub stack_ids: Integers,
    pub stack_amounts: Integers,
    pub team_id: i32,
    pub lend_id: i32,
    pub lend_template_id: i32,
    pub recolour_id: i32,
    pub recolour_template_id: i32,
    pub equip_id: i32,
    pub item_requirements: Option<Shared<BTreeMap<i32, i32>>>,
    pub client_script_data: Option<Shared<BTreeMap<i32, Value>>>,
    pub item_type: i32,
}
impl Default for ItemDefinition {
    fn default() -> Self {
        Self {
            id: 0,
            name: Some("null".into()),
            examine: None,
            options: Some(Shared::new(vec![
                None,
                None,
                None,
                None,
                Some("drop".into()),
            ])),
            handlers: BTreeMap::new(),
            interface_model_id: 0,
            model_zoom: 0,
            model_rotation_x: 0,
            model_rotation_y: 0,
            model_offset_x: 0,
            model_offset_y: 0,
            stackable: false,
            value: 1,
            members_only: false,
            male_worn_model_id1: -1,
            female_worn_model_id1: -1,
            male_worn_model_id2: -1,
            female_worn_model_id2: -1,
            male_worn_model_id3: -1,
            female_worn_model_id3: -1,
            male_worn_model_id4: -1,
            female_worn_model_id4: -1,
            ground_actions: Some(Shared::new(vec![
                None,
                None,
                Some("take".into()),
                None,
                None,
            ])),
            original_model_colors: None,
            modified_model_colors: None,
            texture_colour1: None,
            texture_colour2: None,
            unknown_array1: None,
            unknown_array2: None,
            unknown_array3: None,
            unnoted: true,
            colour_equip1: -1,
            colour_equip2: 0,
            note_id: -1,
            note_template_id: -1,
            stack_ids: None,
            stack_amounts: None,
            team_id: 0,
            lend_id: -1,
            lend_template_id: -1,
            recolour_id: -1,
            recolour_template_id: -1,
            equip_id: 0,
            item_requirements: None,
            client_script_data: None,
            item_type: 0,
        }
    }
}

impl ItemDefinition {
    pub fn decode(id: i32, reader: &mut Reader<'_>) -> Result<Self> {
        let mut def = Self {
            id,
            ..Self::default()
        };
        loop {
            match reader.g1()? {
                0 => break,
                1 => def.interface_model_id = reader.g2()?,
                2 => def.name = Some(string(reader)?),
                3 => {
                    def.handlers
                        .insert("examine".into(), string(reader)?.into());
                }
                4 => def.model_zoom = reader.g2()?,
                5 => def.model_rotation_x = reader.g2()?,
                6 => def.model_rotation_y = reader.g2()?,
                7 => def.model_offset_x = reader.g2b()?,
                8 => def.model_offset_y = reader.g2b()?,
                10 => {} // Original unused opcode consumes no operands.
                11 => def.stackable = true,
                12 => {
                    def.value = (reader.g1()? << 24)
                        | (reader.g1()? << 16)
                        | (reader.g1()? << 8)
                        | reader.g1()?
                }
                16 => def.members_only = true,
                23 => def.male_worn_model_id1 = reader.g2()?,
                24 => def.female_worn_model_id1 = reader.g2()?,
                25 => def.male_worn_model_id2 = reader.g2()?,
                26 => def.female_worn_model_id2 = reader.g2()?,
                opcode @ 30..=34 => {
                    def.ground_actions.as_ref().unwrap().write()[opcode as usize - 30] =
                        Some(string(reader)?)
                }
                opcode @ 35..=39 => {
                    def.options.as_ref().unwrap().write()[opcode as usize - 35] =
                        Some(string(reader)?)
                }
                opcode @ (40 | 41) => {
                    let length = reader.g1()? as usize;
                    let mut first = Vec::with_capacity(length);
                    let mut second = Vec::with_capacity(length);
                    for _ in 0..length {
                        first.push(reader.g2b()? as i16);
                        second.push(reader.g2b()? as i16);
                    }
                    let pair = (Some(Shared::new(first)), Some(Shared::new(second)));
                    if opcode == 40 {
                        (def.original_model_colors, def.modified_model_colors) = pair;
                    } else {
                        (def.texture_colour1, def.texture_colour2) = pair;
                    }
                }
                42 => {
                    let length = reader.g1()?;
                    let mut values = Vec::with_capacity(length as usize);
                    for _ in 0..length {
                        values.push(reader.g1b()? as i8);
                    }
                    def.unknown_array1 = Some(Shared::new(values));
                }
                65 => def.unnoted = true,
                78 => def.colour_equip1 = reader.g2()?,
                79 => def.colour_equip2 = reader.g2()?,
                90 => def.male_worn_model_id3 = reader.g2b()?,
                91 => def.female_worn_model_id3 = reader.g2b()?,
                92 => def.male_worn_model_id4 = reader.g2b()?,
                93 => def.female_worn_model_id4 = reader.g2b()?,
                95 | 110..=112 => {
                    reader.g2b()?;
                }
                96 => def.item_type = reader.g1b()?,
                97 => def.note_id = reader.g2()?,
                98 => def.note_template_id = reader.g2()?,
                opcode @ 100..=109 => {
                    if def.stack_ids.is_none() {
                        def.stack_ids = Some(Shared::new(vec![0; 10]));
                        def.stack_amounts = Some(Shared::new(vec![0; 10]));
                    }
                    def.stack_ids.as_ref().unwrap().write()[opcode as usize - 100] = reader.g2()?;
                    def.stack_amounts.as_ref().unwrap().write()[opcode as usize - 100] =
                        reader.g2()?;
                }
                113 | 114 => {
                    reader.g1b()?;
                }
                115 => def.team_id = reader.g1b()?,
                121 => def.lend_id = reader.g2()?,
                122 => def.lend_template_id = reader.g2()?,
                125 | 126 => {
                    reader.g1()?;
                    reader.g1()?;
                    reader.g1()?;
                }
                127..=130 => {
                    reader.g1()?;
                    reader.g2()?;
                }
                249 => {
                    let length = reader.g1()?;
                    let params = def
                        .client_script_data
                        .get_or_insert_with(|| Shared::new(BTreeMap::new()));
                    for _ in 0..length {
                        let is_string = reader.g1()? == 1;
                        // Kotlin getMedium performs three individual relative reads.
                        let key = (reader.g1()? << 16) | (reader.g1()? << 8) | reader.g1()?;
                        let value = if is_string {
                            string(reader)?.into()
                        } else {
                            reader.g4()?.into()
                        };
                        params.write().insert(key, value);
                    }
                }
                _ => break, // Preserve the original early return, including trailing bytes.
            }
        }
        Ok(def)
    }
    pub fn is_stackable(&self) -> bool {
        self.stackable || !self.unnoted
    }
    pub fn is_player_type(&self) -> bool {
        self.item_type == 0
    }
    pub fn archive_id(&self) -> u32 {
        (self.id as u32) >> 8
    }
    pub fn file_id(&self) -> i32 {
        self.id & 255
    }
    pub fn has_action(&self, name: Option<&str>) -> bool {
        let Some(name) = name else {
            return false;
        };
        self.options.as_ref().is_some_and(|options| {
            options
                .read()
                .iter()
                .flatten()
                .any(|action| rs09_game_core::java_string::equals_ignore_case(action, name))
        })
    }
    pub fn has_destroy_action(&self) -> bool {
        self.has_action(Some("destroy")) || self.has_action(Some("dissolve"))
    }
    pub fn has_wear_action(&self) -> bool {
        ["wield", "wear", "equip"]
            .into_iter()
            .any(|name| self.has_action(Some(name)))
    }
    pub fn has_special_bar(&self) -> bool {
        self.parameter(686).as_ref().and_then(Value::as_i64) == Some(1)
    }
    pub fn quest_id(&self) -> i32 {
        self.parameter(861)
            .as_ref()
            .and_then(Value::as_i64)
            .map_or(-1, |id| id as i32)
    }
    fn parameter(&self, id: i32) -> Option<Value> {
        self.client_script_data
            .as_ref()
            .and_then(|params| params.read().get(&id).cloned())
    }
}
fn string(reader: &mut Reader<'_>) -> Result<String> {
    let mut result = String::new();
    loop {
        let byte = reader.g1b()?;
        if byte == 0 {
            return Ok(result);
        }
        result.push(char::from_u32(u32::from(byte as u16)).unwrap());
    }
}
#[derive(Default)]
pub struct Registry {
    definitions: HashMap<i32, Handle>,
}
impl Registry {
    pub fn definitions(&self) -> &HashMap<i32, Handle> {
        &self.definitions
    }
    pub fn insert(&mut self, id: i32, definition: Handle) -> Option<Handle> {
        self.definitions.insert(id, definition)
    }
    pub fn for_id(&self, id: i32) -> Handle {
        self.definitions
            .get(&id)
            .cloned()
            .unwrap_or_else(|| Shared::new(ItemDefinition::default()))
    }
    /// Source parse() publishes each completed record immediately. A later failure
    /// keeps earlier writes and never inserts the failing definition.
    pub fn parse_files(
        &mut self,
        count: i32,
        mut file: impl FnMut(i32) -> CacheResult<Option<Vec<u8>>>,
    ) -> CacheResult<()> {
        for id in 0..count {
            let definition = if let Some(data) = file(id)? {
                let mut definition = ItemDefinition::decode(id, &mut Reader::new(&data))?;
                if id == 14958 {
                    definition.stackable = true;
                }
                definition
            } else {
                ItemDefinition::default()
            };
            self.insert(id, Shared::new(definition));
        }
        self.define_templates(count);
        Ok(())
    }
    /// Server archive 19, strict decoding. A caller-supplied record cap bounds
    /// malformed sparse indices before loading; there is no client fallback here.
    pub fn load_archive(
        &mut self,
        archive: &Archive,
        max_definitions: usize,
    ) -> CacheResult<usize> {
        let count = archive
            .index
            .capacity
            .checked_sub(1)
            .and_then(|last| {
                last.checked_mul(256).and_then(|base| {
                    base.checked_add(archive.index.group(last).map_or(0, |g| g.capacity))
                })
            })
            .ok_or_else(|| CacheError::Format(Error::new(0, "invalid item archive capacity")))?;
        if count as usize > max_definitions || count > i32::MAX as u32 {
            return Err(Error::new(0, "item definition count exceeds limit").into());
        }
        self.parse_files(count as i32, |id| {
            archive
                .file((id as u32) >> 8, (id & 255) as u32, [0; 4])
                .map(|file| file.map(|file| file.as_ref().clone()))
        })?;
        Ok(count as usize)
    }
    pub fn define_templates(&self, count: i32) {
        let mut equip_id = 0i32;
        for id in 0..count {
            let def = self.for_id(id);
            let (base, template) = {
                let d = def.read();
                (d.note_id, d.note_template_id)
            };
            if template != -1 {
                transfer_note(&def, &self.for_id(base), &self.for_id(template));
            }
            let (base, template) = {
                let d = def.read();
                (d.lend_id, d.lend_template_id)
            };
            if template != -1 {
                transfer_lend(&def, &self.for_id(base), &self.for_id(template));
            }
            let (base, template) = {
                let d = def.read();
                (d.recolour_id, d.recolour_template_id)
            };
            if template != -1 {
                transfer_recolour(&def, &self.for_id(base), &self.for_id(template));
            }
            let wears = {
                let d = def.read();
                d.male_worn_model_id1 >= 0 || d.male_worn_model_id2 >= 0
            };
            if wears {
                def.write().equip_id = equip_id;
                equip_id = equip_id.wrapping_add(1);
            }
        }
        let replacement = self.for_id(3140).read().equip_id;
        self.for_id(2513).write().equip_id = replacement;
    }
}

// Read, release, then assign each field. Copying one initial snapshot would change
// the result when target/base/template refer to the same definition.
macro_rules! assign {
    ($to:ident, $from:ident; $($field:ident),+ $(,)?) => { $( { let value = cloned(&$from.read().$field); $to.write().$field = value; } )+ };
}
fn cloned<T: Clone>(value: &T) -> T {
    value.clone()
}
pub fn transfer_note(to: &Handle, base: &Handle, template: &Handle) {
    assign!(to, base; members_only);
    assign!(to, template; interface_model_id, original_model_colors);
    assign!(to, base; name);
    assign!(to, template; model_offset_y, texture_colour1);
    assign!(to, base; value);
    assign!(to, template; model_rotation_y);
    {
        let mut d = to.write();
        d.stackable = true;
        d.unnoted = false;
    }
    assign!(to, template; modified_model_colors, model_rotation_x, model_zoom, texture_colour1);
    to.write().handlers.insert("tradeable".into(), true.into());
}
pub fn transfer_lend(to: &Handle, base: &Handle, template: &Handle) {
    assign!(to, base; female_worn_model_id1, male_worn_model_id2, members_only);
    assign!(to, template; interface_model_id);
    assign!(to, base; texture_colour2, ground_actions, unknown_array1);
    assign!(to, template; model_rotation_x, model_rotation_y);
    assign!(to, base; original_model_colors, name, male_worn_model_id1, colour_equip1, team_id);
    assign!(to, template; model_offset_y);
    assign!(to, base; client_script_data, modified_model_colors, colour_equip2);
    assign!(to, template; model_offset_x);
    assign!(to, base; texture_colour1);
    to.write().value = 0;
    assign!(to, template; model_zoom);
    to.write().options = Some(Shared::new(vec![None; 5]));
    assign!(to, base; female_worn_model_id2);
    clone_options(to, base);
}
pub fn transfer_recolour(to: &Handle, base: &Handle, template: &Handle) {
    assign!(to, base; female_worn_model_id2);
    to.write().options = Some(Shared::new(vec![None; 5]));
    assign!(to, template; model_rotation_y);
    assign!(to, base; name, male_worn_model_id1);
    assign!(to, template; model_offset_y);
    assign!(to, base; female_worn_model_id1, male_worn_model_id2);
    assign!(to, template; model_offset_x);
    assign!(to, base; unknown_array1, stackable);
    assign!(to, template; model_rotation_x);
    assign!(to, base; texture_colour1, colour_equip1, texture_colour2, modified_model_colors);
    assign!(to, template; model_zoom);
    assign!(to, base; colour_equip2, team_id);
    to.write().value = 0;
    assign!(to, base; ground_actions, original_model_colors, members_only, client_script_data);
    assign!(to, template; interface_model_id);
    clone_options(to, base);
}
fn clone_options(to: &Handle, base: &Handle) {
    let options = base
        .read()
        .options
        .as_ref()
        .map(|array| Shared::new(array.read().clone()));
    if let Some(options) = options {
        to.write().options = Some(options);
    }
}
