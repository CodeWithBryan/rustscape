//! `CowMilkingPlugin` and its `BucketHandler`: the dairy cows' "milk" and
//! "steal-cowbell" options and a bucket used on a cow.
use super::{CreaturePulse, CreatureTask, DialogueArg, Host};
use crate::world::combat_swing::{Error, Result};
use crate::world::special_attacks::Priority;

/// The two dairy cow sceneries.
pub const COWS: [i32; 2] = [8689, 12111];
pub const BUCKET: i32 = 1925;
pub const PAIL: i32 = 3727;
pub const BUCKET_OF_MILK: i32 = 1927;
/// `ANIMATION`.
pub const MILKING: i32 = 2305;
/// `Sounds.MILK_COW_372`.
pub const MILK_SOUND: i32 = 372;
/// `UseWithHandler.OBJECT_TYPE`.
pub const OBJECT_TYPE: i32 = 2;

/// `newInstance(arg)`: "option:milk" on both cows, the "steal-cowbell"
/// option handler, then `ClassScanner.definePlugin(new BucketHandler())`,
/// whose try prints and swallows any failure of the plugin's `newInstance`.
pub fn new_instance<H: Host>(h: &mut H) -> Result<()> {
    for id in COWS {
        h.put_scenery_handler(id, "option:milk")?;
    }
    h.scenery_option_handler("steal-cowbell")?;
    let defined = h
        .define_bucket_plugin()
        .and_then(|_| bucket_new_instance(h));
    if let Err(error) = defined {
        h.print_stack_trace(&error)?;
    }
    Ok(())
}

/// `BucketHandler.newInstance(arg)`.
pub fn bucket_new_instance<H: Host>(h: &mut H) -> Result<()> {
    for id in COWS {
        h.add_use_with_handler(id, OBJECT_TYPE)?;
    }
    Ok(())
}

/// `handle(player, node, option)`: the Cold War message for
/// "steal-cowbell" (any case), otherwise milking. `scenery` is false for a
/// node that is neither null nor a `Scenery` (the cast fails).
pub fn handle<H: Host>(
    h: &mut H,
    player: &H::Entity,
    scenery: bool,
    option: Option<&str>,
) -> Result<bool> {
    let option = option.ok_or(Error::Null)?;
    if rs09_game_core::java_string::equals_ignore_case(option, "steal-cowbell") {
        h.open_dialogue(
            player,
            70099,
            &[DialogueArg::Str(
                "You need to have started the Cold War quest to attempt this.",
            )],
        )?;
        return Ok(true);
    }
    if !scenery {
        return Err(Error::Binding(
            "ClassCastException: (Scenery) node of another node".into(),
        ));
    }
    milk(h, player)?;
    Ok(true)
}

/// `BucketHandler.handle(event)`.
pub fn bucket_handle<H: Host>(h: &mut H, player: &H::Entity) -> Result<bool> {
    milk(h, player)?;
    Ok(true)
}

/// `milk(player, object)`: without a bucket or a pail the dialogue 3807,
/// otherwise the animation, the sound and the 8-tick milking pulse.
pub fn milk<H: Host>(h: &mut H, player: &H::Entity) -> Result<bool> {
    if !h.inventory_contains(player, BUCKET, 1)? && !h.inventory_contains(player, PAIL, 1)? {
        h.open_dialogue(
            player,
            3807,
            &[DialogueArg::Bool(true), DialogueArg::Bool(true)],
        )?;
        return Ok(true);
    }
    let animation = h.new_animation(MILKING, Priority::Mid);
    h.animate(player, Some(&animation))?;
    h.play_audio(player, MILK_SOUND, 0)?;
    h.run_creature_pulse(
        player,
        CreaturePulse::new(
            8,
            vec![Some(player.clone())],
            CreatureTask::Milk {
                player: player.clone(),
            },
        ),
    )?;
    Ok(true)
}

/// The milking pulse's `pulse()`: a bucket (or else a pail) becomes a
/// bucket of milk; it goes on while one is left.
pub fn milk_pulse<H: Host>(h: &mut H, player: &H::Entity) -> Result<bool> {
    if h.inventory_remove(player, (BUCKET, 1, 1000))?
        || h.inventory_remove(player, (PAIL, 1, 1000))?
    {
        h.inventory_add(player, (BUCKET_OF_MILK, 1, 1000))?;
        h.send_message(player, "You milk the cow.")?;
    }
    if h.inventory_contains(player, BUCKET, 1)? || h.inventory_contains(player, PAIL, 1)? {
        let animation = h.new_animation(MILKING, Priority::Mid);
        h.animate(player, Some(&animation))?;
        h.play_audio(player, MILK_SOUND, 0)?;
        return Ok(false);
    }
    Ok(true)
}
