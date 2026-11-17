//! UpdateMasks binding for the prepared-player transport world. Appearance
//! fields are supplied after body preparation; full content/impact encoders and
//! registry startup remain separate. Every unsupported encoder returns an error.
use super::{
    animation::{Animation, Graphics},
    appearance,
    doors::Location,
    player_render::{Handle, MaskState},
    update_masks::{self as masks, Flag, Kind, Provider, ProviderHandle},
};
use crate::definitions::item::Shared;
use std::{
    collections::VecDeque,
    sync::{Arc, LazyLock},
};
#[derive(Clone)]
enum Payload {
    Chat {
        effects: u16,
        rights: u8,
        bytes: Arc<[u8]>,
    },
    Appearance(Shared<appearance::Data>),
    Animation(Animation),
    Spot(Graphics),
    FaceLocation(Location),
    FaceEntity(FaceTarget),
    Hit(Handle, i32, i32),
    MeleeHit {
        damage: i32,
        kind: i32,
        ratio: i32,
    },
}
type Core = masks::Masks<Handle, Payload>;
/// Retains the actual canonical target and render view. Encoding never resolves
/// a later occupant of the same wire index.
#[derive(Clone)]
pub enum FaceTarget {
    Player(super::player_walk::NodeHandle, Handle),
    Npc(super::player_walk::NodeHandle, super::npc_render::Handle),
}
impl FaceTarget {
    fn client_index(&self) -> i32 {
        match self {
            Self::Player(_, p) => p.read().index | 0x8000,
            Self::Npc(_, n) => n.read().index,
        }
    }
}
/// Actual queued impact fields for this owned prepared-player instance. The
/// combat producer and HitMark encoders must be bound before impacts are enabled.
#[derive(Clone, Copy)]
struct Impact {
    damage: i32,
    kind: i32,
}
#[derive(Clone)]
pub struct PreparedMasks {
    core: Core,
    body: Shared<appearance::Data>,
    impacts: Shared<VecDeque<Impact>>,
}
static PROVIDERS: LazyLock<Vec<ProviderHandle>> = LazyLock::new(|| {
    [
        (Flag::Chat, 0x80, 0),
        (Flag::PrimaryHit, 1, 1),
        (Flag::Animate, 8, 2),
        (Flag::Appearance, 4, 3),
        (Flag::FaceEntity, 2, 4),
        (Flag::ForceMove, 0x400, 5),
        (Flag::ForceChat, 0x20, 6),
        (Flag::SecondaryHit, 0x200, 7),
        (Flag::AnimSeq, 0x800, 8),
        (Flag::SpotAnim, 0x100, 9),
        (Flag::FaceLocation, 0x40, 10),
    ]
    .into_iter()
    .map(|(flag, presence, ordinal)| {
        Arc::new(Provider {
            revision: 530,
            kind: Kind::Player,
            presence,
            ordinal,
            flag,
        })
    })
    .collect()
});
impl PreparedMasks {
    pub fn chat(&self, effects: u16, rights: u8, bytes: Vec<u8>) -> masks::Result<bool> {
        if bytes.len() > usize::from(u8::MAX) {
            return Err(masks::Error::Binding(
                "Public chat mask exceeds its byte length".into(),
            ));
        }
        self.register(
            Flag::Chat,
            Some(Payload::Chat {
                effects,
                rights,
                bytes: bytes.into(),
            }),
            false,
            || 0,
        )
    }
    pub(crate) fn same(&self, other: &Self) -> bool {
        self.core.same(&other.core)
    }
    pub(crate) fn keeping_from<T: std::any::Any + Send + Sync>(&self, owner: &Shared<T>) -> Self {
        Self {
            core: self.core.keeping_from(owner),
            body: self.body.keeping_from(owner),
            impacts: self.impacts.keeping_from(owner),
        }
    }
    pub(crate) fn ownership(&self) -> (usize, usize, Vec<super::player_walk::NodeHandle>) {
        let f = self.core.snapshot();
        let nodes = f
            .current
            .into_iter()
            .chain(f.synced)
            .flatten()
            .filter_map(|e| match e.context {
                Some(Payload::FaceEntity(
                    FaceTarget::Player(node, _) | FaceTarget::Npc(node, _),
                )) => Some(node),
                _ => None,
            })
            .collect();
        (self.core.identity(), self.core.strong_count(), nodes)
    }
    pub fn new(player: Handle, body: Shared<appearance::Data>) -> Self {
        Self {
            core: Core::new(player, Kind::Player),
            body,
            impacts: Shared::new(VecDeque::new()),
        }
    }
    pub fn owner(&self) -> Handle {
        self.core.owner()
    }
    pub fn body(&self) -> &Shared<appearance::Data> {
        &self.body
    }
    pub fn status(&self) -> masks::Status {
        self.core.status()
    }
    fn publish(&self) {
        let status = self.status();
        self.owner().write().masks = MaskState {
            required: status.update_required(),
            synced: status.has_synced(),
            appearance_stamp: status.appearance_stamp,
        };
    }
    fn register(
        &self,
        flag: Flag,
        payload: Option<Payload>,
        sync: bool,
        clock: impl FnMut() -> i64,
    ) -> masks::Result<bool> {
        let result = self
            .core
            .register(&mut Context { state: self, clock }, flag, payload, sync);
        self.publish();
        result
    }
    pub fn sync_appearance(&self, clock: impl FnMut() -> i64) -> masks::Result<bool> {
        self.register(
            Flag::Appearance,
            Some(Payload::Appearance(self.body.unowned())),
            true,
            clock,
        )
    }
    pub fn animate(&self, id: i32, delay: i32) -> masks::Result<bool> {
        self.animate_handle(Animation::with(
            id,
            delay,
            Some(super::animation::Priority::Mid),
        ))
    }
    /// Register one impact against canonical health after the swing applies.
    pub fn melee_hit(&self, damage: i32, ratio: i32) -> masks::Result<bool> {
        self.register(
            Flag::PrimaryHit,
            Some(Payload::MeleeHit {
                damage,
                kind: i32::from(damage > 0),
                ratio,
            }),
            false,
            || 0,
        )
    }
    /// Retain the actual context. IDs can change after registration and before
    /// the original encoder reads them; copying numeric fields here is wrong.
    pub fn animate_handle(&self, animation: Animation) -> masks::Result<bool> {
        self.register(
            Flag::Animate,
            Some(Payload::Animation(animation)),
            false,
            || 0,
        )
    }
    pub fn spot(&self, graphic: appearance::SpotAnimation) -> masks::Result<bool> {
        self.graphics_handle(Graphics::new(graphic.id, graphic.height, graphic.delay))
    }
    pub fn graphics_handle(&self, graphic: Graphics) -> masks::Result<bool> {
        self.register(Flag::SpotAnim, Some(Payload::Spot(graphic)), false, || 0)
    }
    pub fn face_location(&self, location: Option<Location>) -> masks::Result<bool> {
        if let Some(location) = location {
            self.register(
                Flag::FaceLocation,
                Some(Payload::FaceLocation(location)),
                true,
                || 0,
            )
        } else {
            let result = self.core.unregister_synced(10);
            self.publish();
            result.map(|_| true)
        }
    }
    /// Entity.face(null) registers a clear only after removing a persistent face.
    pub fn clear_face_entity(&self) -> masks::Result<bool> {
        let removed = self.core.unregister_synced(4)?;
        self.publish();
        if removed {
            self.register(Flag::FaceEntity, None, false, || 0)
        } else {
            Ok(true)
        }
    }
    pub fn face_entity(&self, target: Option<FaceTarget>) -> masks::Result<bool> {
        if let Some(target) = target {
            self.register(
                Flag::FaceEntity,
                Some(Payload::FaceEntity(target)),
                true,
                || 0,
            )
        } else {
            self.clear_face_entity()
        }
    }
    pub fn prepare(&self) -> masks::Result<()> {
        let result = self.core.prepare(
            &mut Context {
                state: self,
                clock: || 0,
            },
            self.owner(),
        );
        self.publish();
        result
    }
    pub fn reset(&self) {
        self.core.reset();
        self.publish();
    }
    pub fn write(
        &self,
        viewer: Handle,
        buffer: &mut Vec<u8>,
        appearance: bool,
        synced: bool,
    ) -> masks::Result<()> {
        let mut context = Context {
            state: self,
            clock: || 0,
        };
        let owner = self.owner();
        if synced {
            self.core
                .write_synced(&mut context, Some(viewer), Some(&owner), appearance, buffer)
        } else {
            self.core
                .write(&mut context, Some(viewer), Some(&owner), buffer)
        }
    }
}
struct Context<'a, F> {
    state: &'a PreparedMasks,
    clock: F,
}
impl<F: FnMut() -> i64> masks::Registry<Handle, Payload> for Context<'_, F> {
    fn provider(
        &mut self,
        _: &Core,
        kind: Kind,
        flag: Flag,
    ) -> masks::Result<Option<ProviderHandle>> {
        Ok(PROVIDERS
            .iter()
            .find(|p| p.kind == kind && p.flag == flag)
            .cloned())
    }
    fn missing(&mut self, _: &Core, _: Kind, flag: Flag) -> masks::Result<()> {
        Err(masks::Error::Binding(format!(
            "prepared player has no provider for {flag:?}"
        )))
    }
    fn now_millis(&mut self, _: &Core) -> masks::Result<i64> {
        Ok((self.clock)())
    }
}
impl<F: FnMut() -> i64> masks::Writer<Handle, Payload> for Context<'_, F> {
    fn is_player(&self, entity: Option<&Handle>) -> bool {
        entity.is_some()
    }
    fn appearance_presence(&mut self, _: &Core, _: Kind) -> masks::Result<i32> {
        Ok(4)
    }
    fn encode(
        &mut self,
        _: &Core,
        provider: ProviderHandle,
        context: Option<Payload>,
        _: Handle,
        buffer: &mut Vec<u8>,
    ) -> masks::Result<()> {
        match (provider.flag, context) {
            (
                Flag::Chat,
                Some(Payload::Chat {
                    effects,
                    rights,
                    bytes,
                }),
            ) => {
                buffer.extend(effects.to_le_bytes());
                buffer.extend([rights, bytes.len() as u8]);
                buffer.extend(bytes.iter().rev().copied());
            }
            (Flag::Appearance, Some(Payload::Appearance(body))) => {
                buffer.extend(body.read().encode())
            }
            (Flag::Animate, Some(Payload::Animation(animation))) => {
                buffer.extend(appearance::animation(animation.id(), animation.delay()))
            }
            (Flag::SpotAnim, Some(Payload::Spot(graphic))) => buffer.extend(
                appearance::SpotAnimation {
                    id: graphic.id(),
                    height: graphic.height(),
                    delay: graphic.delay(),
                }
                .encode(),
            ),
            (Flag::FaceLocation, Some(Payload::FaceLocation(location))) => {
                let [x, y, _] = location.coordinates();
                let x = x.wrapping_shl(1).wrapping_add(1);
                let y = y.wrapping_shl(1).wrapping_add(1);
                buffer.extend([
                    (x >> 8) as u8,
                    x as u8,
                    (y as u8).wrapping_add(128),
                    (y >> 8) as u8,
                ]);
            }
            (Flag::FaceEntity, None) => buffer.extend([255, 127]),
            (Flag::FaceEntity, Some(Payload::FaceEntity(target))) => {
                let index = target.client_index();
                buffer.extend([(index >> 8) as u8, (index as u8).wrapping_add(128)]);
            }
            // The original AnimationSequence provider itself writes no payload.
            (Flag::AnimSeq, _) => {}
            (
                Flag::PrimaryHit,
                Some(Payload::MeleeHit {
                    damage,
                    kind,
                    ratio,
                }),
            ) => {
                let mut out = rs09_protocol::buffer::Writer::default();
                out.psmarts(damage)
                    .map_err(|e| masks::Error::Binding(e.to_string()))?;
                out.p1add(kind);
                out.p1sub(ratio);
                buffer.extend(out.into_vec());
            }
            (flag, Some(Payload::Hit(entity, damage, kind))) => {
                return Err(masks::Error::Binding(format!(
                    "prepared {flag:?} encoder requires actual combat skills: owner={} damage={damage} type={kind}",
                    entity.read().index
                )));
            }
            (flag, _) => {
                return Err(masks::Error::Binding(format!(
                    "prepared player encoder/context is unbound: {flag:?}"
                )));
            }
        }
        Ok(())
    }
}
impl<F: FnMut() -> i64> masks::Impacts<Handle, Payload> for Context<'_, F> {
    type Handler = Shared<VecDeque<Impact>>;
    type Impact = Impact;
    fn handler(&mut self, _: &Core, _: &Handle) -> masks::Result<Self::Handler> {
        Ok(self.state.impacts.clone())
    }
    fn peek(&mut self, _: &Core, handler: &Self::Handler) -> masks::Result<bool> {
        Ok(!handler.read().is_empty())
    }
    fn poll(&mut self, _: &Core, handler: &Self::Handler) -> masks::Result<Option<Impact>> {
        Ok(handler.write().pop_front())
    }
    fn hit_payload(&self, entity: Handle, impact: Impact) -> Payload {
        Payload::Hit(entity, impact.damage, impact.kind)
    }
}
