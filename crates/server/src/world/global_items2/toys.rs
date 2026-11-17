//! ItemQuestRequirementListener's equip requirements, MorphItemListener's
//! morphing rings, ToyListeners and ToyHorseListener.
use super::random::kotlin_random;
use super::tables;
use super::{
    Anim, Error, Gfx, Host, Result, RunBody, animate, run_task, send_chat, used_option, visualize,
};

// ---------------------------------------------------------------------- ItemQuestRequirementListener

/// One of ItemQuestRequirementListener's `onEquip` lambdas: the quest's
/// `hasRequirement` (with its message), or the quest cape's points check.
pub fn quest_equip<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    quest: Option<&str>,
) -> Result<bool> {
    match quest {
        Some(quest) => h.has_requirement(player, quest, true),
        None => {
            let max = h.quests()?.into_iter().fold(0i32, i32::wrapping_add);
            if h.quest_points(player)? < max {
                h.send_message(
                    player,
                    "You cannot equip the Quest cape without completing all available quests.",
                )?;
                return Ok(false);
            }
            Ok(true)
        }
    }
}

// ---------------------------------------------------------------------- MorphItemListener

/// `Sounds.EASTER06_HUMAN_INTO_EGG_1520`.
pub const MORPH_SOUND: i32 = 1520;

/// MorphItemListener's `onEquip`: `morph(player, item.id)`, and the item is
/// not equipped (false).
pub fn morph_equip<H: Host + ?Sized>(h: &mut H, player: &H::Node, item: &H::Node) -> Result<bool> {
    let id = h.id(item)?;
    let morph = if id == tables::MORPH_ITEMS[0] {
        kotlin_random(h.random(), &tables::STONE)?
    } else {
        kotlin_random(h.random(), &tables::EASTER_EGGS)?
    };
    h.play_audio(player, MORPH_SOUND, 0, 1, None, 8)?;
    h.stop_walk(player)?;
    h.close_all_interfaces(player)?;
    h.transform_npc(player, morph)?;
    h.open_interface(player, tables::MORPH_INTERFACE)?;
    Ok(false)
}

/// The unmorph interface's button: button 3 closes every interface. True.
pub fn morph_button<H: Host + ?Sized>(h: &mut H, player: &H::Node, button: i32) -> Result<bool> {
    if button == 3 {
        h.close_all_interfaces(player)?;
    }
    Ok(true)
}

/// The unmorph interface's close: the player's own appearance returns. True.
pub fn morph_close<H: Host + ?Sized>(h: &mut H, player: &H::Node) -> Result<bool> {
    h.transform_npc(player, -1)?;
    Ok(true)
}

// ---------------------------------------------------------------------- ToyListeners

/// The ToyListeners lambdas, in registration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Toy {
    ChocatriceCape,
    Marionette,
    ReindeerHat,
    SnowGlobe,
    SpinningPlate,
    Kite,
    YoYo,
    ZombieHead,
    RubberChicken,
}
impl Toy {
    pub const ALL: [Self; 9] = [
        Self::ChocatriceCape,
        Self::Marionette,
        Self::ReindeerHat,
        Self::SnowGlobe,
        Self::SpinningPlate,
        Self::Kite,
        Self::YoYo,
        Self::ZombieHead,
        Self::RubberChicken,
    ];
    /// The ids and options each registers.
    pub fn registration(self) -> (&'static [i32], &'static [&'static str]) {
        match self {
            Self::ChocatriceCape => (&[12645], &["operate"]),
            Self::Marionette => (&tables::MARIONETTES, &["jump", "walk", "bow", "dance"]),
            Self::ReindeerHat => (&[10507], &["operate"]),
            Self::SnowGlobe => (&[11949], &["shake"]),
            Self::SpinningPlate => (&[4613], &["spin"]),
            Self::Kite => (&[12844], &["fly", "operate"]),
            Self::YoYo => (&[4079], &["play", "loop", "walk", "crazy"]),
            Self::ZombieHead => (&[6722], &["talk-at", "display", "question"]),
            Self::RubberChicken => (&[4566], &["operate", "Dance"]),
        }
    }
}

const SNOWGLOBE_SHAKE: Anim = Anim::new(7535);
const SNOWGLOBE_HOLDFACE: Anim = Anim::new(7536);
const SNOWGLOBE_INTERFACE: i32 = 659;
const SPINNING_PLATE_SPIN: Anim = Anim::new(1902);
const SPINNING_PLATE_PUT_DOWN: Anim = Anim::new(1904);

/// A ToyListeners lambda. Always true.
pub fn toy<H: Host + ?Sized>(
    h: &mut H,
    toy: Toy,
    player: &H::Node,
    node: &H::Node,
) -> Result<bool> {
    match toy {
        Toy::ChocatriceCape => {
            h.lock_interactions(player, 2)?;
            visualize(h, player, Anim::new(8903), Gfx::new(1566))?;
        }
        Toy::Marionette => {
            let id = h.id(node)?;
            let index = tables::MARIONETTES.iter().position(|m| *m == id);
            h.lock_interactions(player, 2)?;
            let option = used_option(h, player)?;
            let (animation, column) = match option.as_str() {
                "jump" => (3003, 0),
                "walk" => (3004, 1),
                "bow" => (3005, 2),
                "dance" => (3006, 3),
                _ => return Ok(true),
            };
            let gfx = tables::MARIONETTE_GFX[index.ok_or(Error::Index)?][column];
            visualize(h, player, Anim::new(animation), Gfx::new(gfx))?;
        }
        Toy::ReindeerHat => {
            h.lock_interactions(player, 2)?;
            visualize(h, player, Anim::new(5059), Gfx::new(859))?;
        }
        Toy::SnowGlobe => {
            h.lock_interactions(player, 2)?;
            animate(h, player, Some(SNOWGLOBE_SHAKE))?;
            run_task(
                h,
                player,
                3,
                1,
                RunBody::SnowGlobeHold {
                    player: player.clone(),
                },
            )?;
        }
        Toy::SpinningPlate => {
            h.lock_interactions(player, 2)?;
            animate(h, player, Some(SPINNING_PLATE_SPIN))?;
            run_task(
                h,
                player,
                2,
                1,
                RunBody::SpinningPlate {
                    player: player.clone(),
                },
            )?;
        }
        Toy::Kite => {
            h.lock_interactions(player, 2)?;
            animate(h, player, Some(Anim::new(8990)))?;
        }
        Toy::YoYo => {
            let option = used_option(h, player)?;
            h.lock_interactions(player, 2)?;
            let animation = match option.as_str() {
                "play" => 1457,
                "loop" => 1458,
                "walk" => 1459,
                "crazy" => 1460,
                _ => return Ok(true),
            };
            animate(h, player, Some(Anim::new(animation)))?;
        }
        Toy::ZombieHead => {
            let option = used_option(h, player)?;
            h.lock_interactions(player, 2)?;
            match option.as_str() {
                "talk-at" => {
                    animate(h, player, Some(Anim::new(2840)))?;
                    send_chat(h, player, "Alas!")?;
                }
                "display" => {
                    animate(h, player, Some(Anim::new(2844)))?;
                    send_chat(h, player, "MWAHAHAHAHAHAHAH")?;
                }
                _ => {}
            }
        }
        Toy::RubberChicken => {
            h.lock_interactions(player, 2)?;
            visualize(h, player, Anim::new(1835), Gfx::new(-1))?;
            h.play_jingle(player, 99)?;
            h.play_audio(player, 355, 100, 1, None, 8)?;
        }
    }
    Ok(true)
}

/// The body of a toy's `runTask`.
pub fn toy_task<H: Host + ?Sized>(h: &mut H, body: &RunBody<H::Node>) -> Result<()> {
    match body {
        RunBody::SnowGlobeHold { player } => {
            animate(h, player, Some(SNOWGLOBE_HOLDFACE))?;
            run_task(
                h,
                player,
                0,
                1,
                RunBody::SnowGlobeInterface {
                    player: player.clone(),
                },
            )
        }
        RunBody::SnowGlobeInterface { player } => h.open_interface(player, SNOWGLOBE_INTERFACE),
        RunBody::SpinningPlate { player } => animate(h, player, Some(SPINNING_PLATE_PUT_DOWN)),
        RunBody::Sand { .. } => Err(Error::Binding(Some("not a toy task".into()))),
    }
}

// ---------------------------------------------------------------------- ToyHorseListener

/// ToyHorseListener's `play-with`: the walk stops, the horse's animation
/// plays (an unknown id has none: `IllegalStateException`) and a random
/// phrase is said. True.
pub fn horsey<H: Host + ?Sized>(h: &mut H, player: &H::Node, node: &H::Node) -> Result<bool> {
    h.stop_walk(player)?;
    let id = h.id(node)?;
    let animation = tables::HORSEY_MAP
        .iter()
        .find(|(item, _)| *item == id)
        .map(|(_, anim)| Anim::new(*anim));
    animate(h, player, animation)?;
    let phrase = kotlin_random(h.random(), &tables::PHRASES)?;
    send_chat(h, player, phrase)?;
    Ok(true)
}
