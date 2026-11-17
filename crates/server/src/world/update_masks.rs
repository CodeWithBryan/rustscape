//! Original UpdateMasks lifecycle and live write traversal. Providers, actual
//! entity/context handles, encoders, clock and impact queues are host bindings.
use crate::definitions::item::Shared;
use std::sync::Arc;
pub const SIZE: usize = 11;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Player,
    Npc,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Flag {
    Chat,
    ForceChat,
    PrimaryHit,
    SecondaryHit,
    Animate,
    SpotAnim,
    Appearance,
    FaceEntity,
    FaceLocation,
    ForceMove,
    AnimSeq,
    TypeSwap,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Null,
    Index,
    Binding(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
/// Original provider properties are immutable constructor values. The actual
/// native encoder is resolved by the host from this retained provider object.
pub struct Provider {
    pub revision: i32,
    pub kind: Kind,
    pub presence: i32,
    pub ordinal: i32,
    pub flag: Flag,
}
pub type ProviderHandle = Arc<Provider>;
#[derive(Clone)]
pub struct Element<C> {
    pub encoder: ProviderHandle,
    pub context: Option<C>,
}
struct Fields<E, C> {
    owner: E,
    kind: Kind,
    appearance_stamp: i64,
    updating: bool,
    presence: i32,
    synced_presence: i32,
    elements: [Option<Element<C>>; SIZE],
    synced: [Option<Element<C>>; SIZE],
}
pub struct Masks<E, C>(Shared<Fields<E, C>>);
impl<E, C> Clone for Masks<E, C> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
#[derive(Clone, Copy)]
pub struct Status {
    pub appearance_stamp: i64,
    pub updating: bool,
    pub presence: i32,
    pub synced_presence: i32,
}
impl Status {
    pub fn update_required(self) -> bool {
        self.presence != 0
    }
    pub fn has_synced(self) -> bool {
        self.synced_presence != 0
    }
}
pub struct Snapshot<C> {
    pub status: Status,
    pub current: Vec<Option<Element<C>>>,
    pub synced: Vec<Option<Element<C>>>,
}
pub trait Registry<E: Clone, C: Clone> {
    fn provider(
        &mut self,
        masks: &Masks<E, C>,
        kind: Kind,
        flag: Flag,
    ) -> Result<Option<ProviderHandle>>;
    fn missing(&mut self, masks: &Masks<E, C>, kind: Kind, flag: Flag) -> Result<()>;
    fn now_millis(&mut self, masks: &Masks<E, C>) -> Result<i64>;
}
pub trait Writer<E: Clone, C: Clone> {
    fn is_player(&self, entity: Option<&E>) -> bool;
    fn appearance_presence(&mut self, masks: &Masks<E, C>, kind: Kind) -> Result<i32>;
    fn encode(
        &mut self,
        masks: &Masks<E, C>,
        provider: ProviderHandle,
        context: Option<C>,
        viewer: E,
        buffer: &mut Vec<u8>,
    ) -> Result<()>;
}
pub trait Impacts<E: Clone, C: Clone>: Registry<E, C> {
    type Handler: Clone;
    type Impact;
    fn handler(&mut self, masks: &Masks<E, C>, entity: &E) -> Result<Self::Handler>;
    fn peek(&mut self, masks: &Masks<E, C>, handler: &Self::Handler) -> Result<bool>;
    fn poll(
        &mut self,
        masks: &Masks<E, C>,
        handler: &Self::Handler,
    ) -> Result<Option<Self::Impact>>;
    /// HitMark construction captures the actual entity and impact fields; it
    /// makes no game-service callback in the original constructor.
    fn hit_payload(&self, entity: E, impact: Self::Impact) -> C;
}
impl<E: Clone, C: Clone> Masks<E, C> {
    pub(crate) fn identity(&self) -> usize {
        self.0.identity()
    }
    pub(crate) fn strong_count(&self) -> usize {
        self.0.strong_count()
    }
    pub(crate) fn keeping_from<T: std::any::Any + Send + Sync>(&self, owner: &Shared<T>) -> Self {
        Self(self.0.keeping_from(owner))
    }
    pub fn new(owner: E, kind: Kind) -> Self {
        Self(Shared::new(Fields {
            owner,
            kind,
            appearance_stamp: 0,
            updating: false,
            presence: 0,
            synced_presence: 0,
            elements: std::array::from_fn(|_| None),
            synced: std::array::from_fn(|_| None),
        }))
    }
    pub fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
    pub fn owner(&self) -> E {
        self.0.read().owner.clone()
    }
    pub fn kind(&self) -> Kind {
        self.0.read().kind
    }
    /// The source exposes appearanceStamp as a mutable property.
    pub fn set_appearance_stamp(&self, stamp: i64) {
        self.0.write().appearance_stamp = stamp;
    }
    pub fn status(&self) -> Status {
        let f = self.0.read();
        Status {
            appearance_stamp: f.appearance_stamp,
            updating: f.updating,
            presence: f.presence,
            synced_presence: f.synced_presence,
        }
    }
    /// Native observation API. Context objects retain their actual identities;
    /// callers must serialize values now if they need immutable observations.
    pub fn snapshot(&self) -> Snapshot<C> {
        let f = self.0.read();
        Snapshot {
            status: Status {
                appearance_stamp: f.appearance_stamp,
                updating: f.updating,
                presence: f.presence,
                synced_presence: f.synced_presence,
            },
            current: f.elements.to_vec(),
            synced: f.synced.to_vec(),
        }
    }
    pub fn register(
        &self,
        ctx: &mut impl Registry<E, C>,
        flag: Flag,
        context: Option<C>,
        mut sync: bool,
    ) -> Result<bool> {
        let kind = self.kind();
        let Some(provider) = ctx.provider(self, kind, flag)? else {
            ctx.missing(self, kind, flag)?;
            return Ok(false);
        };
        if self.status().updating {
            return Ok(false);
        }
        if flag == Flag::Appearance {
            let stamp = ctx.now_millis(self)?;
            self.0.write().appearance_stamp = stamp;
            sync = true;
        }
        let slot = usize::try_from(provider.ordinal).map_err(|_| Error::Index)?;
        if sync {
            let mut f = self.0.write();
            *f.synced.get_mut(slot).ok_or(Error::Index)? = Some(Element {
                encoder: provider.clone(),
                context: context.clone(),
            });
            f.synced_presence |= provider.presence;
        }
        let mut f = self.0.write();
        *f.elements.get_mut(slot).ok_or(Error::Index)? = Some(Element {
            encoder: provider.clone(),
            context,
        });
        f.presence |= provider.presence;
        Ok(true)
    }
    pub fn unregister_synced(&self, ordinal: i32) -> Result<bool> {
        let slot = usize::try_from(ordinal).map_err(|_| Error::Index)?;
        let mut f = self.0.write();
        let value = f.synced.get(slot).ok_or(Error::Index)?.clone();
        if let Some(value) = value {
            f.synced_presence &= !value.encoder.presence;
            f.synced[slot] = None;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    fn header(
        ctx: &impl Writer<E, C>,
        entity: Option<&E>,
        mut presence: i32,
        buffer: &mut Vec<u8>,
    ) {
        if presence >= 0x100 {
            presence |= if ctx.is_player(entity) { 0x10 } else { 0x8 };
            buffer.extend([presence as u8, (presence >> 8) as u8]);
        } else {
            buffer.push(presence as u8)
        }
    }
    pub fn write(
        &self,
        ctx: &mut impl Writer<E, C>,
        viewer: Option<E>,
        entity: Option<&E>,
        buffer: &mut Vec<u8>,
    ) -> Result<()> {
        Self::header(ctx, entity, self.status().presence, buffer);
        for i in 0..SIZE {
            let element = self.0.read().elements[i].clone();
            if let Some(element) = element {
                ctx.encode(
                    self,
                    element.encoder,
                    element.context,
                    viewer.clone().ok_or(Error::Null)?,
                    buffer,
                )?;
            }
        }
        Ok(())
    }
    pub fn write_synced(
        &self,
        ctx: &mut impl Writer<E, C>,
        viewer: Option<E>,
        entity: Option<&E>,
        appearance: bool,
        buffer: &mut Vec<u8>,
    ) -> Result<()> {
        let status = self.status();
        let mut synced = status.synced_presence;
        let appearance_flag = ctx.appearance_presence(self, self.kind())?;
        if !appearance && synced & appearance_flag != 0 {
            synced &= !appearance_flag;
        }
        Self::header(ctx, entity, status.presence | synced, buffer);
        for i in 0..SIZE {
            let current = self.0.read().elements[i].clone();
            let element = if current.is_some() {
                current
            } else {
                let synced = self.0.read().synced[i].clone();
                if !appearance
                    && synced
                        .as_ref()
                        .is_some_and(|e| e.encoder.flag == Flag::Appearance)
                {
                    continue;
                }
                synced
            };
            if let Some(element) = element {
                ctx.encode(
                    self,
                    element.encoder,
                    element.context,
                    viewer.clone().ok_or(Error::Null)?,
                    buffer,
                )?;
            }
        }
        Ok(())
    }
    pub fn prepare(&self, ctx: &mut impl Impacts<E, C>, entity: E) -> Result<()> {
        let handler = ctx.handler(self, &entity)?;
        for i in 0..2 {
            if !ctx.peek(self, &handler)? {
                break;
            }
            let impact = ctx.poll(self, &handler)?.ok_or(Error::Null)?;
            let payload = ctx.hit_payload(entity.clone(), impact);
            self.register(
                ctx,
                if i == 1 {
                    Flag::SecondaryHit
                } else {
                    Flag::PrimaryHit
                },
                Some(payload),
                false,
            )?;
        }
        self.0.write().updating = true;
        Ok(())
    }
    pub fn reset(&self) {
        let mut f = self.0.write();
        f.elements.fill(None);
        f.presence = 0;
        f.updating = false;
    }
}
