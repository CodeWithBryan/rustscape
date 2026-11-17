//! ContentAPI's animation, graphics, audio, projectile and impact functions,
//! scenery and ground items, definitions and charges.
use super::{
    Anim, Arg, Error, Gfx, Host, Kind, Location, Member as M, Priority, Result, Value, b, i,
    loc_of, n, nd, new_item, ns, unit,
};
use crate::world::content_api::{api_files::Direction, services::LogLevel, services::log};

/// `getAnimation(id)`: `Animation(id)`.
pub fn get_animation<N>(id: i32) -> Anim<N> {
    Anim::new(id)
}

/// `getAnimationWithPriority(id, priority)`.
pub fn get_animation_with_priority<N>(id: i32, priority: Priority) -> Anim<N> {
    Anim::with(id, priority)
}

/// `resetAnimator(player)`: `Animation(-1, VERY_HIGH)`.
pub fn reset_animator<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<()> {
    b(
        h,
        M::AnimatorAnimate,
        vec![
            nd(p),
            Arg::anim(&Anim::<H::Node>::with(-1, Priority::VeryHigh)),
        ],
    )
    .map(|_| ())
}

/// `renderAnimation(player, animation)`.
pub fn render_animation<H: Host + ?Sized>(h: &mut H, p: &H::Node, animation: i32) -> Result<()> {
    unit(
        h,
        M::AppearanceSetAnimations,
        vec![nd(p), Arg::anim(&Anim::<H::Node>::new(animation))],
    )?;
    unit(h, M::AppearanceSync, vec![nd(p)])
}

/// `resetRenderAnimation(player)`.
pub fn reset_render_animation<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<()> {
    unit(h, M::AppearanceDefaultAnimations, vec![nd(p)])?;
    unit(h, M::AppearanceSetAnimations, vec![nd(p)])?;
    unit(h, M::AppearanceSync, vec![nd(p)])
}

/// `animationDuration(animation)`: its definition's `durationTicks`.
pub fn animation_duration<H: Host + ?Sized>(h: &mut H, animation: &Anim<H::Node>) -> Result<i32> {
    i(h, M::AnimationDurationTicks, vec![Arg::Int(animation.id)])
}

/// `animationCycles(animation)`.
pub fn animation_cycles<H: Host + ?Sized>(h: &mut H, animation: i32) -> Result<i32> {
    i(h, M::AnimationCycles, vec![Arg::Int(animation)])
}

/// The `Animation` of `animate<T>`/`visualize<A, G>`: an Int or an Animation.
fn anim_arg<N: Clone>(v: &Value<N>, message: &str) -> Result<Anim<N>> {
    match v {
        Value::Int(id) => Ok(Anim::new(*id)),
        Value::Anim(a) => Ok(a.clone()),
        _ => Err(Error::State(message.into())),
    }
}

/// `animate(entity, anim, forced = false)`.
pub fn animate<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    anim: &Value<H::Node>,
    forced: bool,
) -> Result<()> {
    let animation = anim_arg(anim, "Invalid value passed for anim")?;
    if forced {
        unit(h, M::AnimatorForce, vec![nd(e), Arg::anim(&animation)])
    } else {
        b(h, M::AnimatorAnimate, vec![nd(e), Arg::anim(&animation)]).map(|_| ())
    }
}

/// `visualize(entity, anim, gfx)`.
pub fn visualize<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    anim: &Value<H::Node>,
    gfx: &Value<H::Node>,
) -> Result<()> {
    let animation = anim_arg(anim, "Invalid parameter passed for animation.")?;
    let graphics = match gfx {
        Value::Int(id) => Gfx::new(*id),
        Value::Gfx(g) => *g,
        _ => {
            return Err(Error::State(
                "Invalid parameter passed for graphics.".into(),
            ));
        }
    };
    b(
        h,
        M::EntityVisualize,
        vec![nd(e), Arg::anim(&animation), Arg::gfx(graphics)],
    )
    .map(|_| ())
}

/// `sendGraphics(gfx, location)`: an Int or a Graphics; anything else does nothing.
pub fn send_graphics<H: Host + ?Sized>(
    h: &mut H,
    gfx: &Value<H::Node>,
    location: Location,
) -> Result<()> {
    let g = match gfx {
        Value::Int(id) => Gfx::new(*id),
        Value::Gfx(g) => *g,
        _ => return Ok(()),
    };
    unit(h, M::GraphicsSend, vec![Arg::gfx(g), Arg::Loc(location)])
}

/// `emote(entity, emote)`: `entity.animate(emote.animation)`.
pub fn emote<H: Host + ?Sized>(h: &mut H, e: &H::Node, animation: &Anim<H::Node>) -> Result<()> {
    b(h, M::EntityAnimate, vec![nd(e), Arg::anim(animation)]).map(|_| ())
}

/// `animateScenery(player, obj, animationId, global = false)`.
pub fn animate_scenery_for<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    obj: &H::Node,
    animation: i32,
    global: bool,
) -> Result<()> {
    unit(
        h,
        M::SendSceneryAnimation,
        vec![
            nd(p),
            nd(obj),
            Arg::anim(&Anim::<H::Node>::new(animation)),
            Arg::Bool(global),
        ],
    )
}

/// `animateScenery(obj, animationId)`: the chunk's AnimateObjectUpdateFlag.
pub fn animate_scenery<H: Host + ?Sized>(h: &mut H, obj: &H::Node, animation: i32) -> Result<()> {
    let mut anim = Anim::new(animation);
    anim.object = Some(obj.clone());
    let at = loc_of(h, obj)?;
    let chunk = n(h, M::RegionChunkOf, vec![Arg::Loc(at)])?.ok_or(Error::Null)?;
    let flag = Arg::List(vec![Arg::text("AnimateObjectUpdateFlag"), Arg::anim(&anim)]);
    unit(h, M::ChunkFlag, vec![nd(&chunk), flag])
}

/// `animateInterface(player, iface, child, anim)`.
pub fn animate_interface<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    iface: i32,
    child: i32,
    anim: i32,
) -> Result<()> {
    unit(
        h,
        M::SendAnimationInterface,
        vec![nd(p), Arg::Int(anim), Arg::Int(iface), Arg::Int(child)],
    )
}

/// `spawnProjectile(source, dest, projectileId)`.
pub fn spawn_projectile<H: Host + ?Sized>(
    h: &mut H,
    source: &H::Node,
    dest: &H::Node,
    id: i32,
) -> Result<()> {
    let p = n(
        h,
        M::ProjectileCreate,
        vec![nd(source), nd(dest), Arg::Int(id)],
    )?
    .ok_or(Error::Null)?;
    unit(h, M::ProjectileSend, vec![nd(&p)])
}

/// `spawnProjectile(source, dest, projectile, startHeight, endHeight, delay,
/// speed, angle)`: the distance is `source.getDistance(dest).toInt()`.
#[allow(clippy::too_many_arguments)]
pub fn spawn_projectile_at<H: Host + ?Sized>(
    h: &mut H,
    source: Location,
    dest: Location,
    projectile: i32,
    start_height: i32,
    end_height: i32,
    delay: i32,
    speed: i32,
    angle: i32,
) -> Result<()> {
    let distance = super::java_double_to_int(source.distance(dest));
    let args = vec![
        Arg::Loc(source),
        Arg::Loc(dest),
        Arg::Int(projectile),
        Arg::Int(start_height),
        Arg::Int(end_height),
        Arg::Int(delay),
        Arg::Int(speed),
        Arg::Int(angle),
        Arg::Int(distance),
    ];
    let p = n(h, M::ProjectileCreate, args)?.ok_or(Error::Null)?;
    unit(h, M::ProjectileSend, vec![nd(&p)])
}

/// `Audio(id, delay, loops, radius)`.
pub fn audio<N>(id: i32, delay: i32, loops: i32, radius: i32) -> Arg<N> {
    Arg::List(vec![
        Arg::text("Audio"),
        Arg::Int(id),
        Arg::Int(delay),
        Arg::Int(loops),
        Arg::Int(radius),
    ])
}

/// `Audio.defaultAudioRadius`.
pub const DEFAULT_AUDIO_RADIUS: i32 = 8;

/// `playJingle(player, jingleId)`: a MusicPacket with the secondary flag.
pub fn play_jingle<H: Host + ?Sized>(h: &mut H, p: &H::Node, jingle: i32) -> Result<()> {
    let ctx = Arg::List(vec![
        Arg::text("MusicContext"),
        nd(p),
        Arg::Int(jingle),
        Arg::Bool(true),
    ]);
    unit(h, M::PacketSend, vec![Arg::text("MusicPacket"), ctx])
}

/// `playAudio(player, id, delay = 0, loops = 1, location = null, radius = 8)`.
pub fn play_audio<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    id: i32,
    delay: i32,
    loops: i32,
    location: Option<Location>,
    radius: i32,
) -> Result<()> {
    let ctx = Arg::List(vec![
        Arg::text("DefaultContext"),
        nd(p),
        audio(id, delay, loops, radius),
        Arg::loc(location),
    ]);
    unit(h, M::PacketSend, vec![Arg::text("AudioPacket"), ctx])
}

/// `playGlobalAudio(location, id, delay = 0, loops = 1, radius = 8)`.
pub fn play_global_audio<H: Host + ?Sized>(
    h: &mut H,
    location: Location,
    id: i32,
    delay: i32,
    loops: i32,
    radius: i32,
) -> Result<()> {
    for p in ns(
        h,
        M::LocalPlayers,
        vec![Arg::Loc(location), Arg::Int(radius)],
    )? {
        let p = p.ok_or(Error::Null)?;
        play_audio(h, &p, id, delay, loops, Some(location), radius)?;
    }
    Ok(())
}

/// `playHurtAudio`'s sounds (HUMAN_HIT4_516, HUMAN_HIT5_517, HUMAN_HIT_518,
/// HUMAN_HIT_6_522 and FEMALE_HIT_506, FEMALE_HIT_507, FEMALE_HIT2_508,
/// FEMALE_HIT_2_510).
pub const MALE_HURT: [i32; 4] = [516, 517, 518, 522];
pub const FEMALE_HURT: [i32; 4] = [506, 507, 508, 510];

/// `playHurtAudio(player, delay = 0)`: Kotlin's `random()` of the gender's sounds.
pub fn play_hurt_audio<H: Host + ?Sized>(h: &mut H, p: &H::Node, delay: i32) -> Result<()> {
    let sounds = if h.is_male(p) { MALE_HURT } else { FEMALE_HURT };
    let id = sounds[h.kotlin_random_int(sounds.len() as i32) as usize];
    play_audio(h, p, id, delay, 1, None, DEFAULT_AUDIO_RADIUS)
}

/// `ImpactHandler.HitsplatType`.
pub const HITSPLATS: [&str; 6] = ["MISS", "NORMAL", "POISON", "DISEASE", "NORMAL_1", "VENOM"];

/// `impact(entity, amount, type = NORMAL)`: a manual hit, and a player's
/// hurt sound.
pub fn impact<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    amount: i32,
    hitsplat: &'static str,
) -> Result<()> {
    unit(
        h,
        M::ManualHit,
        vec![nd(e), nd(e), Arg::Int(amount), Arg::Enum(hitsplat)],
    )?;
    if h.kind(e) == Kind::Player {
        play_hurt_audio(h, e, 0)?;
    }
    Ok(())
}

/// `toReplace.transform(with, rotation, location)` with
/// `SceneryBuilder.replace` (permanent for -1), then `isActive = false`.
fn replace_with<H: Host + ?Sized>(
    h: &mut H,
    to: &H::Node,
    with: i32,
    ticks: i32,
    rotation: i32,
    loc: Option<Location>,
) -> Result<()> {
    let at = match loc {
        None => h.location(to),
        some => some,
    };
    let built = n(
        h,
        M::SceneryTransform,
        vec![nd(to), Arg::Int(with), Arg::Int(rotation), Arg::loc(at)],
    )?;
    let built = Arg::opt_node(built.as_ref());
    if ticks == -1 {
        b(h, M::SceneryReplace, vec![nd(to), built])?;
    } else {
        b(h, M::SceneryReplace, vec![nd(to), built, Arg::Int(ticks)])?;
    }
    unit(h, M::ScenerySetActive, vec![nd(to), Arg::Bool(false)])
}

/// `replaceScenery(toReplace, with, forTicks, loc = null)`: the scenery's own rotation.
pub fn replace_scenery<H: Host + ?Sized>(
    h: &mut H,
    to: &H::Node,
    with: i32,
    ticks: i32,
    loc: Option<Location>,
) -> Result<()> {
    let rotation = h.rotation(to);
    replace_with(h, to, with, ticks, rotation, loc)
}

/// `replaceScenery(toReplace, with, forTicks, rotation, loc = null)`.
pub fn replace_scenery_facing<H: Host + ?Sized>(
    h: &mut H,
    to: &H::Node,
    with: i32,
    ticks: i32,
    rotation: Direction,
    loc: Option<Location>,
) -> Result<()> {
    let rot = match rotation {
        Direction::NorthWest => 0,
        Direction::North => 1,
        Direction::NorthEast => 2,
        Direction::East => 4,
        Direction::SouthEast => 7,
        Direction::South => 6,
        Direction::SouthWest => 5,
        Direction::West => 3,
    };
    replace_with(h, to, with, ticks, rot, loc)
}

/// `addScenery(sceneryId, location, rotation = 0, type = 22)`.
pub fn add_scenery<H: Host + ?Sized>(
    h: &mut H,
    id: i32,
    location: Location,
    rotation: i32,
    kind: i32,
) -> Result<H::Node> {
    let scenery = n(
        h,
        M::NewScenery,
        vec![
            Arg::Int(id),
            Arg::Loc(location),
            Arg::Int(kind),
            Arg::Int(rotation),
        ],
    )?
    .ok_or(Error::Null)?;
    unit(h, M::SceneryAdd, vec![nd(&scenery)])?;
    Ok(scenery)
}

/// `addScenery(scenery)`.
pub fn add_existing_scenery<H: Host + ?Sized>(h: &mut H, scenery: &H::Node) -> Result<()> {
    unit(h, M::SceneryAdd, vec![nd(scenery)])
}

/// `removeScenery(scenery)`.
pub fn remove_scenery<H: Host + ?Sized>(h: &mut H, scenery: &H::Node) -> Result<()> {
    b(h, M::SceneryRemove, vec![nd(scenery)]).map(|_| ())
}

/// `getScenery(x, y, z)`.
pub fn get_scenery<H: Host + ?Sized>(h: &mut H, x: i32, y: i32, z: i32) -> Result<Option<H::Node>> {
    n(h, M::GetObject, vec![Arg::Int(x), Arg::Int(y), Arg::Int(z)])
}

/// `getScenery(loc)`.
pub fn get_scenery_at<H: Host + ?Sized>(h: &mut H, loc: Location) -> Result<Option<H::Node>> {
    n(h, M::GetObject, vec![Arg::Loc(loc)])
}

/// `SceneryDefinition.forId(id)` with Kotlin's non-null use.
fn scenery_definition_of<H: Host + ?Sized>(h: &mut H, id: i32) -> Result<()> {
    n(h, M::SceneryDefinition, vec![Arg::Int(id)])?
        .ok_or(Error::Null)
        .map(|_| ())
}

/// `getChildren(scenery)`: the child ids other than -1.
pub fn get_children<H: Host + ?Sized>(h: &mut H, scenery: i32) -> Result<Vec<i32>> {
    scenery_definition_of(h, scenery)?;
    let ids = h
        .call(M::SceneryChildren, vec![Arg::Int(scenery)])?
        .ints()?
        .ok_or(Error::Null)?;
    Ok(ids.into_iter().filter(|i| *i != -1).collect())
}

/// `sceneryDefinition(id)` (the definition's id).
pub fn scenery_definition<H: Host + ?Sized>(h: &mut H, id: i32) -> Result<i32> {
    scenery_definition_of(h, id)?;
    Ok(id)
}

/// `getItemName(id)`.
pub fn get_item_name<H: Host + ?Sized>(h: &mut H, id: i32) -> Result<String> {
    h.item_definition(id)
        .ok_or(Error::Null)?
        .name
        .ok_or(Error::Null)
}

/// `itemDefinition(id)` (Kotlin's non-null return).
pub fn item_definition<H: Host + ?Sized>(h: &mut H, id: i32) -> Result<super::ItemDef<H::Node>> {
    h.item_definition(id).ok_or(Error::Null)
}

/// `hasOption(node, option)`: the NPC, scenery or item definition's action.
pub fn has_option<H: Host + ?Sized>(h: &mut H, node: &H::Node, option: &str) -> Result<bool> {
    let kind = h.kind(node);
    let id = h.id(node);
    if kind.is_npc() {
        b(h, M::NpcHasAction, vec![Arg::Int(id), Arg::text(option)])
    } else if kind == Kind::Scenery {
        scenery_definition_of(h, id)?;
        b(
            h,
            M::SceneryHasAction,
            vec![Arg::Int(id), Arg::text(option)],
        )
    } else if kind.is_item() {
        h.item_definition(id).ok_or(Error::Null)?;
        b(h, M::ItemHasAction, vec![Arg::Int(id), Arg::text(option)])
    } else {
        Err(Error::Argument(format!(
            "Expected an NPC, Scenery or an Item, got {}.",
            kind.simple_name()
        )))
    }
}

/// `removeGroundItem(node)`.
pub fn remove_ground_item<H: Host + ?Sized>(h: &mut H, node: &H::Node) -> Result<()> {
    n(h, M::GroundDestroy, vec![nd(node)]).map(|_| ())
}

/// `isValidGroundItem(node)`.
pub fn is_valid_ground_item<H: Host + ?Sized>(h: &mut H, node: &H::Node) -> Result<bool> {
    Ok(ns(h, M::GroundItems, vec![])?
        .iter()
        .any(|i| i.as_ref() == Some(node)))
}

/// `produceGroundItem(player, item)`.
pub fn produce_ground_item<H: Host + ?Sized>(h: &mut H, p: &H::Node, id: i32) -> Result<()> {
    let item = new_item(h, id, 1, 1000)?;
    n(h, M::GroundCreate, vec![nd(&item), nd(p)]).map(|_| ())
}

/// `produceGroundItem(owner, id, amount, location)`.
pub fn produce_ground_item_at<H: Host + ?Sized>(
    h: &mut H,
    owner: Option<&H::Node>,
    id: i32,
    amount: i32,
    at: Location,
) -> Result<H::Node> {
    let item = new_item(h, id, amount, 1000)?;
    n(
        h,
        M::GroundCreate,
        vec![nd(&item), Arg::Loc(at), Arg::opt_node(owner)],
    )?
    .ok_or(Error::Null)
}

fn set_charge_of<H: Host + ?Sized>(h: &mut H, node: &H::Node, charge: i32) -> Result<()> {
    let m = if h.kind(node).is_item() {
        M::ItemSetCharge
    } else {
        M::ScenerySetCharge
    };
    unit(h, m, vec![nd(node), Arg::Int(charge)])
}

/// `adjustCharge(node, amount)`.
pub fn adjust_charge<H: Host + ?Sized>(h: &mut H, node: &H::Node, amount: i32) -> Result<()> {
    let kind = h.kind(node);
    if kind.is_item() || kind == Kind::Scenery {
        let charge = h.item_charge(node).wrapping_add(amount);
        set_charge_of(h, node, charge)
    } else {
        log(
            h,
            "ContentAPI",
            LogLevel::Err,
            &format!(
                "Attempt to adjust the charge of invalid type: {}",
                kind.simple_name()
            ),
        )
    }
}

/// `getCharge(node)`: -1 for a node without charges.
pub fn get_charge<H: Host + ?Sized>(h: &mut H, node: &H::Node) -> Result<i32> {
    let kind = h.kind(node);
    if kind.is_item() || kind == Kind::Scenery {
        Ok(h.item_charge(node))
    } else {
        log(
            h,
            "ContentAPI",
            LogLevel::Err,
            &format!(
                "Attempt to get charge of invalid type: {}",
                kind.simple_name()
            ),
        )?;
        Ok(-1)
    }
}

/// `setCharge(node, charge)`.
pub fn set_charge<H: Host + ?Sized>(h: &mut H, node: &H::Node, charge: i32) -> Result<()> {
    let kind = h.kind(node);
    if kind.is_item() || kind == Kind::Scenery {
        set_charge_of(h, node, charge)
    } else {
        log(
            h,
            "ContentAPI",
            LogLevel::Err,
            &format!(
                "Attempt to set the charge of invalid type: {}",
                kind.simple_name()
            ),
        )
    }
}
