//! NPC update masks in the live world: `UpdateMasks` (`world::update_masks`)
//! of each NPC with the `NPCFlags530` providers and encoders, the NPC's
//! `Animator` over them, and `UpdateMasks.prepare`'s impact poll from the
//! NPC's `ImpactHandler` queue.
//!
//! The encoders are the library `world::npc_masks` (`NPCFlags530.writeTo`,
//! replayed against npc-masks-oracle); this module maps the live payloads
//! to its contexts and answers its reads (the hit mark owner's lifepoints,
//! a face target's client index).
use super::super::{P, host::Live};
use crate::{
    definitions::{animation::CacheSource, item::Shared},
    world::{
        animation::{self, Animation, Animator, Graphics},
        doors::Location,
        npc_masks,
        player_walk::{Node, NodeHandle},
        prepared_animation,
        update_masks::{self as um, Flag, Kind, Provider, ProviderHandle},
    },
};
use std::sync::{Arc, LazyLock};

/// What a face-entity mask retains: the target's node, read when written.
#[derive(Clone)]
pub enum FaceTarget {
    Player(NodeHandle),
    Npc(NodeHandle),
}

/// A context object of an NPC mask (`HitMark`, `Animation`, `Graphics`,
/// `Entity`, `String`, `Integer`, `Location`).
#[derive(Clone)]
pub enum Payload {
    /// `HitMark(amount, type.ordinal, entity)`: the ratio is read from the
    /// owner's skills when the mask is written.
    Hit {
        owner: Node,
        damage: i32,
        kind: i32,
    },
    Animation(Animation),
    Graphics(Graphics),
    Face(FaceTarget),
    FaceLocation(Location),
    Chat(String),
    TypeSwap(i32),
}

/// One NPC's `UpdateMasks`.
pub type NpcMasks = um::Masks<Node, Payload>;

/// `NPCFlags530`: flag, presence bit, ordinal.
static PROVIDERS: LazyLock<Vec<ProviderHandle>> = LazyLock::new(|| {
    npc_masks::PROVIDERS
        .into_iter()
        .map(|(flag, presence, ordinal)| {
            Arc::new(Provider {
                revision: 530,
                kind: Kind::Npc,
                presence,
                ordinal,
                flag,
            })
        })
        .collect()
});

/// The provider registry (`EntityFlags`) for NPC masks.
pub struct Registry;
impl um::Registry<Node, Payload> for Registry {
    fn provider(
        &mut self,
        _: &NpcMasks,
        kind: Kind,
        flag: Flag,
    ) -> um::Result<Option<ProviderHandle>> {
        Ok(PROVIDERS
            .iter()
            .find(|p| p.kind == kind && p.flag == flag)
            .cloned())
    }
    fn missing(&mut self, _: &NpcMasks, kind: Kind, flag: Flag) -> um::Result<()> {
        Err(um::Error::Binding(format!(
            "no {kind:?} provider for {flag:?} in revision 530"
        )))
    }
    fn now_millis(&mut self, _: &NpcMasks) -> um::Result<i64> {
        Ok(super::super::now_millis())
    }
}

/// `UpdateMasks.prepare`: the impact poll over the NPC's impact queue.
pub struct Prepare<'a> {
    pub live: &'a mut Live,
}
impl um::Registry<Node, Payload> for Prepare<'_> {
    fn provider(
        &mut self,
        masks: &NpcMasks,
        kind: Kind,
        flag: Flag,
    ) -> um::Result<Option<ProviderHandle>> {
        Registry.provider(masks, kind, flag)
    }
    fn missing(&mut self, masks: &NpcMasks, kind: Kind, flag: Flag) -> um::Result<()> {
        Registry.missing(masks, kind, flag)
    }
    fn now_millis(&mut self, masks: &NpcMasks) -> um::Result<i64> {
        Registry.now_millis(masks)
    }
}
impl um::Impacts<Node, Payload> for Prepare<'_> {
    type Handler = Node;
    type Impact = (i32, i32);
    fn handler(&mut self, _: &NpcMasks, entity: &Node) -> um::Result<Node> {
        Ok(*entity)
    }
    fn peek(&mut self, _: &NpcMasks, handler: &Node) -> um::Result<bool> {
        Ok(self
            .live
            .inner
            .combat
            .entities
            .get(handler)
            .is_some_and(|e| !e.impact.queue.is_empty()))
    }
    fn poll(&mut self, _: &NpcMasks, handler: &Node) -> um::Result<Option<(i32, i32)>> {
        let Some(entry) = self.live.inner.combat.entities.get_mut(handler) else {
            return Ok(None);
        };
        Ok(entry
            .impact
            .queue
            .pop_front()
            .map(|impact| (impact.amount, impact.kind.map_or(0, |k| k as i32))))
    }
    fn hit_payload(&self, entity: Node, (damage, kind): (i32, i32)) -> Payload {
        Payload::Hit {
            owner: entity,
            damage,
            kind,
        }
    }
}

/// The NPC mask encoders (`world::npc_masks`, `NPCFlags530.writeTo`) over the
/// live world.
pub struct Encode<'a> {
    pub live: &'a Live,
    /// `logInvalidType`'s lines, logged when the write returns.
    pub logs: Vec<String>,
}

/// The entities an NPC mask's context names: the hit mark's owner, or a
/// face target.
#[derive(Clone)]
pub enum MaskEntity {
    Owner(Node),
    Target(FaceTarget),
}

impl npc_masks::Host<MaskEntity> for Encode<'_> {
    fn maximum_lifepoints(&mut self, entity: &MaskEntity) -> npc_masks::Result<i32> {
        match entity {
            MaskEntity::Owner(node) => Ok(self.live.health_of(*node).1),
            MaskEntity::Target(_) => Err(npc_masks::Error::Binding(
                "the skills of a face target".into(),
            )),
        }
    }
    fn lifepoints(&mut self, entity: &MaskEntity) -> npc_masks::Result<i32> {
        match entity {
            MaskEntity::Owner(node) => Ok(self.live.health_of(*node).0),
            MaskEntity::Target(_) => Err(npc_masks::Error::Binding(
                "the skills of a face target".into(),
            )),
        }
    }
    fn client_index(&mut self, entity: &MaskEntity) -> npc_masks::Result<i32> {
        match entity {
            MaskEntity::Target(target) => Ok(self.live.client_index(target)),
            MaskEntity::Owner(_) => Err(npc_masks::Error::Binding(
                "the client index of a hit mark's owner".into(),
            )),
        }
    }
    fn log_invalid(&mut self, provider: &'static str, message: &str) -> npc_masks::Result<()> {
        self.logs.push(format!("{provider} ERR {message}"));
        Ok(())
    }
}

impl um::Writer<Node, Payload> for Encode<'_> {
    fn is_player(&self, _: Option<&Node>) -> bool {
        false
    }
    fn appearance_presence(&mut self, _: &NpcMasks, _: Kind) -> um::Result<i32> {
        // NPCs have no appearance provider; writeSynced is only called with
        // appearance true for NPCs, so the flag is never consulted.
        Ok(0)
    }
    fn encode(
        &mut self,
        _: &NpcMasks,
        provider: ProviderHandle,
        context: Option<Payload>,
        _: Node,
        buffer: &mut Vec<u8>,
    ) -> um::Result<()> {
        let context = context.map(|payload| match payload {
            Payload::Hit {
                owner,
                damage,
                kind,
            } => npc_masks::Context::HitMark {
                damage,
                kind,
                entity: Some(MaskEntity::Owner(owner)),
            },
            Payload::Animation(animation) => npc_masks::Context::Animation {
                id: animation.id(),
                delay: animation.delay(),
            },
            Payload::Graphics(graphics) => npc_masks::Context::Graphics {
                id: graphics.id(),
                height: graphics.height(),
                delay: graphics.delay(),
            },
            Payload::Face(target) => {
                let class = match target {
                    FaceTarget::Player(_) => "Player",
                    FaceTarget::Npc(_) => "NPC",
                };
                npc_masks::Context::Entity {
                    entity: MaskEntity::Target(target),
                    class,
                }
            }
            Payload::FaceLocation(location) => {
                let [x, y, _] = location.coordinates();
                npc_masks::Context::Location { x, y }
            }
            Payload::Chat(text) => npc_masks::Context::Text(text),
            Payload::TypeSwap(id) => npc_masks::Context::Int(id),
        });
        npc_masks::write_to(self, provider.flag, context.as_ref(), buffer)
            .map_err(|e| um::Error::Binding(format!("NPCFlags530 {:?}: {e:?}", provider.flag)))
    }
}
/// `Entity.animator` of an NPC: its masks and its ten clocks.
#[derive(Clone)]
pub struct NpcOwner {
    pub node: Node,
    pub masks: NpcMasks,
    pub clocks: Shared<Vec<i32>>,
}

/// The animator's world for NPCs: the cache's animation definitions and the
/// world tick of the live animation world.
pub struct Animations<'a> {
    pub world: &'a mut prepared_animation::World<CacheSource>,
}
impl animation::Definitions for Animations<'_> {
    fn definition(
        &mut self,
        animation: &Animation,
        id: i32,
    ) -> Option<crate::definitions::animation::Handle> {
        animation::Definitions::definition(self.world, animation, id)
    }
}
impl animation::Context<NpcOwner> for Animations<'_> {
    fn world_ticks(&mut self, _: &Animator<NpcOwner>) -> animation::Result<i32> {
        Ok(self.world.ticks)
    }
    fn clocks(
        &mut self,
        _: &Animator<NpcOwner>,
        owner: Option<&NpcOwner>,
    ) -> animation::Result<Option<Shared<Vec<i32>>>> {
        Ok(Some(owner.ok_or(animation::Error::Null)?.clocks.clone()))
    }
    fn register_animation(
        &mut self,
        _: &Animator<NpcOwner>,
        owner: Option<&NpcOwner>,
        animation: Animation,
    ) -> animation::Result<bool> {
        let owner = owner.ok_or(animation::Error::Null)?;
        owner
            .masks
            .register(
                &mut Registry,
                Flag::Animate,
                Some(Payload::Animation(animation)),
                false,
            )
            .map_err(|e| animation::Error::Binding(e.to_string()))
    }
    fn register_graphics(
        &mut self,
        _: &Animator<NpcOwner>,
        owner: Option<&NpcOwner>,
        graphics: Graphics,
    ) -> animation::Result<bool> {
        let owner = owner.ok_or(animation::Error::Null)?;
        owner
            .masks
            .register(
                &mut Registry,
                Flag::SpotAnim,
                Some(Payload::Graphics(graphics)),
                false,
            )
            .map_err(|e| animation::Error::Binding(e.to_string()))
    }
}

impl Live {
    /// The NPC's masks, made the first time they are asked for.
    pub fn npc_masks(&mut self, node: Node) -> NpcMasks {
        self.inner
            .combat
            .npc_masks
            .entry(node)
            .or_insert_with(|| um::Masks::new(node, Kind::Npc))
            .clone()
    }
    /// The NPC's `Animator`.
    pub fn npc_animator(&mut self, node: Node) -> Animator<NpcOwner> {
        if let Some(animator) = self.inner.combat.npc_animators.get(&node) {
            return animator.clone();
        }
        let masks = self.npc_masks(node);
        let animator = Animator::new(
            Some(NpcOwner {
                node,
                masks,
                clocks: Shared::new(vec![0; 10]),
            }),
            self.walk.animations.factory.resets().clone(),
        );
        self.inner
            .combat
            .npc_animators
            .insert(node, animator.clone());
        animator
    }
    /// `Entity.animate`/`visualize` of an NPC through its animator.
    pub fn npc_animate(
        &mut self,
        node: Node,
        animation: Option<Animation>,
        graphics: Option<Graphics>,
    ) -> animation::Result<bool> {
        let animator = self.npc_animator(node);
        animator.animate(
            &mut Animations {
                world: &mut self.walk.animations,
            },
            animation,
            graphics,
        )
    }
    /// `Animator.forceAnimation` of an NPC.
    pub fn npc_force_animation(
        &mut self,
        node: Node,
        animation: Option<Animation>,
    ) -> animation::Result<()> {
        let animator = self.npc_animator(node);
        animator.force_animation(
            &mut Animations {
                world: &mut self.walk.animations,
            },
            animation,
        )
    }
    /// `Animator.reset` of an NPC.
    pub fn npc_animator_reset(&mut self, node: Node) -> animation::Result<()> {
        let animator = self.npc_animator(node);
        animator.reset(&mut Animations {
            world: &mut self.walk.animations,
        })
    }
    /// `Entity.face(entity)` of an NPC.
    pub fn npc_face(&mut self, node: Node, target: Option<FaceTarget>) -> um::Result<bool> {
        let masks = self.npc_masks(node);
        match target {
            None => {
                if masks.unregister_synced(3)? {
                    masks.register(&mut Registry, Flag::FaceEntity, None, false)
                } else {
                    Ok(true)
                }
            }
            Some(target) => masks.register(
                &mut Registry,
                Flag::FaceEntity,
                Some(Payload::Face(target)),
                true,
            ),
        }
    }
    /// `Entity.faceLocation(location)` of an NPC.
    pub fn npc_face_location(
        &mut self,
        node: Node,
        location: Option<Location>,
    ) -> um::Result<bool> {
        let masks = self.npc_masks(node);
        match location {
            None => {
                masks.unregister_synced(8)?;
                Ok(true)
            }
            Some(location) => masks.register(
                &mut Registry,
                Flag::FaceLocation,
                Some(Payload::FaceLocation(location)),
                true,
            ),
        }
    }
    /// `Entity.sendChat(text)` of an NPC.
    pub fn npc_chat(&mut self, node: Node, text: String) -> um::Result<bool> {
        let masks = self.npc_masks(node);
        masks.register(
            &mut Registry,
            Flag::ForceChat,
            Some(Payload::Chat(text)),
            false,
        )
    }
    /// `updateMasks.prepare(npc)` at the end of the NPC's `Entity.tick`.
    pub fn npc_prepare_masks(&mut self, node: Node) -> um::Result<()> {
        let masks = self.npc_masks(node);
        masks.prepare(&mut Prepare { live: self }, node)
    }
    /// `updateMasks.reset()` of an NPC.
    pub fn npc_reset_masks(&mut self, node: Node) {
        if let Some(masks) = self.inner.combat.npc_masks.get(&node) {
            masks.reset();
        }
    }
    /// Whether an NPC's masks need writing (`isUpdateRequired`).
    pub fn npc_masks_required(&self, node: Node) -> bool {
        self.inner
            .combat
            .npc_masks
            .get(&node)
            .is_some_and(|m| m.status().update_required())
    }
    /// `NPCRenderer.writeMaskUpdates`: `writeSynced(player, npc, buffer,
    /// true)` for a new NPC, `write` otherwise.
    pub fn write_npc_masks(
        &mut self,
        node: Node,
        viewer: Node,
        buffer: &mut Vec<u8>,
        sync: bool,
    ) -> um::Result<()> {
        let masks = self
            .inner
            .combat
            .npc_masks
            .get(&node)
            .cloned()
            .ok_or(um::Error::Null)?;
        let mut encode = Encode {
            live: self,
            logs: Vec::new(),
        };
        let result = if sync {
            masks.write_synced(&mut encode, Some(viewer), Some(&node), true, buffer)
        } else {
            masks.write(&mut encode, Some(viewer), Some(&node), buffer)
        };
        let logs = encode.logs;
        for line in logs {
            self.inner.log_line(line);
        }
        result
    }
    /// The client index of a face target: a player's index with 0x8000, an
    /// NPC's index.
    pub fn client_index(&self, target: &FaceTarget) -> i32 {
        match target {
            FaceTarget::Player(h) => self
                .inner
                .players
                .get(&h.node)
                .map_or(-1, |slot| slot.index | 0x8000),
            FaceTarget::Npc(h) => self
                .inner
                .state
                .actor(h.node)
                .map_or(-1, |actor| actor.wire_index),
        }
    }
    /// `(skills.lifepoints, skills.maximumLifepoints)` of an entity, for the
    /// hit mark's ratio.
    pub fn health_of(&self, node: Node) -> (i32, i32) {
        if let Some(row) = self.inner.npcs.rows.get(&node) {
            return (row.lifepoints, row.static_levels[3]);
        }
        if let Some(p) = self.player_of(node) {
            if let Ok(pair) = self.inner.with_profile(&p, |profile| {
                (
                    profile.skills.lifepoints,
                    profile.skills.static_levels[3]
                        .wrapping_add(profile.skills.lifepoints_increase),
                )
            }) {
                return pair;
            }
        }
        (0, 1)
    }
    /// The face target of an entity.
    pub fn face_target(&self, p: &P) -> FaceTarget {
        if self.inner.npcs.rows.contains_key(&p.node()) {
            FaceTarget::Npc(p.0.clone())
        } else {
            FaceTarget::Player(p.0.clone())
        }
    }
}
