//! `NPCFlags530`: the revision 530 NPC update-mask providers (presence bit,
//! ordinal, flag) and their `writeTo` bodies, with `EFlagProvider`'s
//! `logInvalidType`.
//!
//! Each body is the pinned `NPCFlags530.kt` member over `IoBuffer`'s
//! writers (`p1`, `p1neg`, `p1sub`, `p2`, `p2add`, `ip2`, `ip4`,
//! `putString`), written here with the original's Java byte arithmetic. The
//! context is the object the mask registered (`HitMark`, `Animation`,
//! `Graphics`, an `Entity` or null, `Integer`, `String`, `Location`, or an
//! object of another class); the reads the bodies make of the hit mark's
//! entity (`skills.maximumLifepoints`, `skills.lifepoints` twice when it is
//! below the maximum) and of a face target (`getClientIndex()`) are [`Host`]
//! services, called in the original order. A context of another type is
//! logged (`logWithStack(ERR, ...)` with the original message) and nothing
//! is written. Verified by `cargo xtask npc-masks-oracle` (docs/npc-masks.md).
use super::update_masks::Flag;

/// `NPCFlags530`'s constructor arguments in declaration order: flag, presence
/// flag, ordinal (the provider class's simple name is [`class_name`]).
pub const PROVIDERS: [(Flag, i32, i32); 9] = [
    (Flag::PrimaryHit, 0x40, 0),
    (Flag::SecondaryHit, 0x2, 1),
    (Flag::Animate, 0x10, 2),
    (Flag::FaceEntity, 0x4, 3),
    (Flag::SpotAnim, 0x80, 4),
    (Flag::TypeSwap, 0x1, 5),
    (Flag::ForceChat, 0x20, 6),
    (Flag::AnimSeq, 0x100, 7),
    (Flag::FaceLocation, 0x200, 8),
];

/// The nested provider class of a flag (`this::class.simpleName`).
pub fn class_name(flag: Flag) -> Option<&'static str> {
    Some(match flag {
        Flag::PrimaryHit => "PrimaryHit",
        Flag::SecondaryHit => "SecondaryHit",
        Flag::Animate => "Animate",
        Flag::FaceEntity => "FaceEntity",
        Flag::SpotAnim => "SpotAnim",
        Flag::TypeSwap => "TypeSwap",
        Flag::ForceChat => "ForceChat",
        Flag::AnimSeq => "AnimationSequence",
        Flag::FaceLocation => "FaceLocation",
        _ => return None,
    })
}

/// A mask's context object. `E` is the host's entity handle.
#[derive(Clone, Debug, PartialEq)]
pub enum Context<E> {
    /// `HitMark(damage, type, entity)`; the entity may be null.
    HitMark {
        damage: i32,
        kind: i32,
        entity: Option<E>,
    },
    /// `Animation`: `getId()`, `getDelay()`.
    Animation { id: i32, delay: i32 },
    /// `Graphics`: `getId()`, `getHeight()`, `getDelay()`.
    Graphics { id: i32, height: i32, delay: i32 },
    /// An `Entity` (a `Player` or an `NPC`, by its class simple name).
    Entity { entity: E, class: &'static str },
    /// A boxed `Integer`.
    Int(i32),
    /// A `String`.
    Text(String),
    /// `Location`: `getX()`, `getY()`.
    Location { x: i32, y: i32 },
    /// An object of another class, by its simple name.
    Other(&'static str),
}

impl<E> Context<E> {
    /// `context::class.java.simpleName`.
    pub fn simple_name(&self) -> &'static str {
        match self {
            Self::HitMark { .. } => "HitMark",
            Self::Animation { .. } => "Animation",
            Self::Graphics { .. } => "Graphics",
            Self::Entity { class, .. } => class,
            Self::Int(_) => "Integer",
            Self::Text(_) => "String",
            Self::Location { .. } => "Location",
            Self::Other(name) => name,
        }
    }
}

/// What the original throws, by category.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// `NullPointerException`.
    Null,
    /// `ArithmeticException` (division by a zero maximum).
    Arithmetic,
    /// A required service failed.
    Binding(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

/// The reads and the log the bodies make outside themselves. None has a
/// default.
pub trait Host<E> {
    /// `e.skills.maximumLifepoints`.
    fn maximum_lifepoints(&mut self, entity: &E) -> Result<i32>;
    /// `e.skills.lifepoints`.
    fn lifepoints(&mut self, entity: &E) -> Result<i32>;
    /// `entity.getClientIndex()`.
    fn client_index(&mut self, entity: &E) -> Result<i32>;
    /// `logWithStack(provider class, Log.ERR, message)`.
    fn log_invalid(&mut self, provider: &'static str, message: &str) -> Result<()>;
}

fn p1(out: &mut Vec<u8>, value: i32) {
    out.push(value as u8);
}
fn p1neg(out: &mut Vec<u8>, value: i32) {
    out.push(value.wrapping_neg() as u8);
}
fn p1sub(out: &mut Vec<u8>, value: i32) {
    out.push(128i32.wrapping_sub(value) as u8);
}
fn p2(out: &mut Vec<u8>, value: i32) {
    out.push((value >> 8) as u8);
    out.push(value as u8);
}
fn p2add(out: &mut Vec<u8>, value: i32) {
    out.push((value >> 8) as u8);
    out.push(value.wrapping_add(128) as u8);
}
fn ip2(out: &mut Vec<u8>, value: i32) {
    out.push(value as u8);
    out.push((value >> 8) as u8);
}
fn ip4(out: &mut Vec<u8>, value: i32) {
    out.push(value as u8);
    out.push((value >> 8) as u8);
    out.push((value >> 16) as u8);
    out.push((value >> 24) as u8);
}
/// `putString`: `String.getBytes()` (the JVM's default charset; UTF-8 here
/// and in the oracle), then 0.
fn put_string(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(value.as_bytes());
    out.push(0);
}

/// `EFlagProvider.logInvalidType(context, expected)`.
fn log_invalid<E, H: Host<E>>(
    h: &mut H,
    provider: &'static str,
    context: Option<&Context<E>>,
    expected: &str,
) -> Result<()> {
    let found = context.map_or("null", Context::simple_name);
    h.log_invalid(
        provider,
        &format!(
            "Invalid context of type {found} passed to {provider} flag which expects {expected}."
        ),
    )
}

/// `KType.toString()` of the `typeOf<T>()` each body names.
const HIT_MARK: &str = "core.game.world.update.flag.context.HitMark";
const ANIMATION: &str = "core.game.world.update.flag.context.Animation";
const ENTITY: &str = "core.game.node.entity.Entity";
const GRAPHICS: &str = "core.game.world.update.flag.context.Graphics";
const STRING: &str = "kotlin.String";
const LOCATION: &str = "core.game.world.map.Location";

/// `provider.writeTo(buffer, context)` of the `NPCFlags530` class of `flag`.
/// On a failure the bytes already written stay in `out`, as they stay in
/// the original's buffer.
pub fn write_to<E, H: Host<E>>(
    h: &mut H,
    flag: Flag,
    context: Option<&Context<E>>,
    out: &mut Vec<u8>,
) -> Result<()> {
    let class = class_name(flag)
        .ok_or_else(|| Error::Binding(format!("{flag:?} has no NPCFlags530 provider")))?;
    match flag {
        Flag::PrimaryHit => {
            let Some(Context::HitMark {
                damage,
                kind,
                entity,
            }) = context
            else {
                return log_invalid(h, class, context, HIT_MARK);
            };
            p1(out, *damage);
            p1neg(out, *kind);
            let entity = entity.as_ref().ok_or(Error::Null)?;
            let mut ratio = 255;
            let max = h.maximum_lifepoints(entity)?;
            if h.lifepoints(entity)? < max {
                let lifepoints = h.lifepoints(entity)?;
                if max == 0 {
                    return Err(Error::Arithmetic);
                }
                ratio = lifepoints.wrapping_mul(255).wrapping_div(max);
            }
            p1sub(out, ratio);
        }
        Flag::SecondaryHit => {
            let Some(Context::HitMark { damage, kind, .. }) = context else {
                return log_invalid(h, class, context, HIT_MARK);
            };
            p1neg(out, *damage);
            p1sub(out, *kind);
        }
        Flag::Animate => {
            let Some(Context::Animation { id, delay }) = context else {
                return log_invalid(h, class, context, ANIMATION);
            };
            p2(out, *id);
            p1(out, *delay);
        }
        Flag::FaceEntity => match context {
            None => p2add(out, -1),
            Some(Context::Entity { entity, .. }) => {
                let index = h.client_index(entity)?;
                p2add(out, index);
            }
            Some(_) => return log_invalid(h, class, context, ENTITY),
        },
        Flag::SpotAnim => {
            let Some(Context::Graphics { id, height, delay }) = context else {
                return log_invalid(h, class, context, GRAPHICS);
            };
            p2add(out, *id);
            ip4(out, height.wrapping_shl(16).wrapping_add(*delay));
        }
        Flag::TypeSwap => {
            // The original names Graphics as the expected type.
            let Some(Context::Int(id)) = context else {
                return log_invalid(h, class, context, GRAPHICS);
            };
            ip2(out, *id);
        }
        Flag::ForceChat => {
            let Some(Context::Text(text)) = context else {
                return log_invalid(h, class, context, STRING);
            };
            put_string(out, text);
        }
        // AnimationSequence: EFlagProvider's empty writeTo (a TODO).
        Flag::AnimSeq => {}
        Flag::FaceLocation => {
            let Some(Context::Location { x, y }) = context else {
                return log_invalid(h, class, context, LOCATION);
            };
            p2add(out, x.wrapping_shl(1).wrapping_add(1));
            p2(out, y.wrapping_shl(1).wrapping_add(1));
        }
        _ => unreachable!("class_name answered None"),
    }
    Ok(())
}
