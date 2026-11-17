//! `SpawnCommandSet`: npc, item, items, object, objectgrid, removeobject,
//! objects.
use super::{Host, Level, Scenery};
use crate::world::commands::kotlin::{to_int, to_int_or_null};
use crate::world::commands::{Container, Error, Loc, Result, arg, reject, to_integer};

/// The class `this::class.java` names in the set's log calls.
pub const ORIGIN: &str = "core.game.system.command.sets.SpawnCommandSet";

/// The set's commands, in definition order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Command {
    Npc,
    Item,
    Items,
    Object,
    ObjectGrid,
    RemoveObject,
    Objects,
}

pub fn run<H: Host + ?Sized>(
    host: &mut H,
    command: Command,
    player: &H::Player,
    args: &[String],
) -> Result<()> {
    match command {
        Command::Npc => npc(host, player, args),
        Command::Item => item(host, player, args),
        Command::Items => items(host, player, args),
        Command::Object => object(host, player, args),
        Command::ObjectGrid => object_grid(host, player, args),
        Command::RemoveObject => remove_object(host, player, args),
        Command::Objects => objects(host, player, args),
    }
}

fn npc<H: Host + ?Sized>(host: &mut H, player: &H::Player, args: &[String]) -> Result<()> {
    if args.len() < 2 {
        return reject(
            host,
            player,
            &["syntax: id (required) amount (optional) isWalks (optional)"],
        );
    }
    let amount = if args.len() > 2 {
        to_integer(arg(args, 2)?)
    } else {
        1
    };
    if amount < 1 {
        return reject(host, player, &["Invalid amount"]);
    }
    if amount > 900 {
        return reject(
            host,
            player,
            &["Based on experience, spawning that many NPCs at once is a bad idea"],
        );
    }
    let mut walks = false;
    if args.len() > 3 {
        let flag = arg(args, 3)?;
        if flag == "true" {
            walks = true;
        } else if !flag.is_empty() && flag != "false" {
            return reject(
                host,
                player,
                &["The \"isWalks\" argument only accepts \"true\" and \"false\""],
            );
        }
    }
    let mut text = String::new();
    for _ in 1..=amount {
        let location = host.location(player)?;
        let npc = host.new_npc(to_integer(arg(args, 1)?), location)?;
        host.npc_set_attribute(npc, "spawned:npc", true)?;
        host.npc_set_respawn(npc, false)?;
        let direction = host.direction(player);
        host.npc_set_direction(npc, direction)?;
        host.npc_init(npc)?;
        host.npc_set_walks(npc, walks)?;
        let [x, y, z] = host.npc_location(npc).ok_or(Error::Null)?;
        let walking = if host.npc_walks(npc) { "1" } else { "0" };
        let facing = host.npc_direction(npc).ordinal();
        text = format!("{{{x},{y},{z},{walking},{facing}}}");
        host.println(&text);
    }
    host.set_clipboard(&text)
}

/// `(args.getOrNull(index) ?: "1").toInt()`.
fn amount_arg(args: &[String], index: usize) -> Result<i32> {
    to_int(args.get(index).map_or("1", String::as_str))
}

/// The out-of-range check, `getMaximumAdd` clamp and `add` of one id.
fn spawn_one<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Player,
    id: i32,
    amount: &mut i32,
) -> Result<()> {
    if id > host.cache_item_definitions_size()? {
        return reject(host, player, &[&format!("Item ID '{id}' out of range.")]);
    }
    let max = host.maximum_add(player, Container::Inventory, id, *amount)?;
    if *amount > max {
        *amount = max;
    }
    host.add(player, Container::Inventory, id, *amount)
        .map(|_| ())
}

fn item<H: Host + ?Sized>(host: &mut H, player: &H::Player, args: &[String]) -> Result<()> {
    if args.len() < 2 {
        return reject(host, player, &["You must specify an item ID"]);
    }
    let Some(id) = to_int_or_null(arg(args, 1)?) else {
        return Ok(());
    };
    let mut amount = amount_arg(args, 2)?;
    spawn_one(host, player, id, &mut amount)
}

fn items<H: Host + ?Sized>(host: &mut H, player: &H::Player, args: &[String]) -> Result<()> {
    if args.len() < 3 {
        return reject(host, player, &["You must specify start and end item ids."]);
    }
    let Some(start) = to_int_or_null(arg(args, 1)?) else {
        return Ok(());
    };
    let Some(end) = to_int_or_null(arg(args, 2)?) else {
        return Ok(());
    };
    let mut amount = amount_arg(args, 3)?;
    if start <= end {
        for id in start..=end {
            spawn_one(host, player, id, &mut amount)?;
        }
    }
    Ok(())
}

/// `Location.transform(dx, dy, dz)`.
fn transform(location: Loc, dx: i32, dy: i32, dz: i32) -> Loc {
    let [x, y, z] = location;
    [
        x.wrapping_add(dx),
        y.wrapping_add(dy),
        z.wrapping_add(dz) % 4,
    ]
}

fn object<H: Host + ?Sized>(host: &mut H, player: &H::Player, args: &[String]) -> Result<()> {
    if args.len() < 2 {
        return reject(
            host,
            player,
            &["syntax error: id (optional) type rotation or rotation"],
        );
    }
    let location = host.location(player)?;
    let scenery = if args.len() > 3 {
        let id = to_integer(arg(args, 1)?);
        let kind = to_integer(arg(args, 2)?);
        let rotation = to_integer(arg(args, 3)?);
        host.new_scenery(id, location, kind, rotation)?
    } else if args.len() == 3 {
        // Scenery(id, location, rotation): type 10.
        let id = to_integer(arg(args, 1)?);
        let rotation = to_integer(arg(args, 2)?);
        host.new_scenery(id, location, 10, rotation)?
    } else {
        // Scenery(id, location): type 10, rotation 0.
        host.new_scenery(to_integer(arg(args, 1)?), location, 10, 0)?
    };
    host.scenery_add(&scenery)?;
    host.log(
        ORIGIN,
        Level::Fine,
        &format!("object = {}", scenery.java_string()),
    )
}

fn object_grid<H: Host + ?Sized>(host: &mut H, player: &H::Player, args: &[String]) -> Result<()> {
    if args.len() != 5 {
        return reject(
            host,
            player,
            &["Usage: objectgrid beginId endId type rotation"],
        );
    }
    let Some(begin) = to_int_or_null(arg(args, 1)?) else {
        return Ok(());
    };
    let Some(end) = to_int_or_null(arg(args, 2)?) else {
        return Ok(());
    };
    let Some(kind) = to_int_or_null(arg(args, 3)?) else {
        return Ok(());
    };
    let Some(rotation) = to_int_or_null(arg(args, 4)?) else {
        return Ok(());
    };
    let here = |host: &mut H| -> Result<Loc> { host.location(player)?.ok_or(Error::Null) };
    for i in 0..=10 {
        let at = transform(here(host)?, i, -1, 0);
        let marker = host.new_scenery(29447 + i, Some(at), 10, 0)?;
        host.scenery_add(&marker)?;
    }
    if begin <= end {
        for i in begin..=end {
            let j = i.wrapping_sub(begin);
            let at = transform(here(host)?, j % 10, j / 10, 0);
            let scenery: Scenery = host.new_scenery(i, Some(at), kind, rotation)?;
            host.scenery_add(&scenery)?;
            if j % 10 == 0 {
                let at = transform(here(host)?, -1, j / 10, 0);
                let marker = host.new_scenery(29447 + (j / 10) % 10, Some(at), 10, 0)?;
                host.scenery_add(&marker)?;
            }
        }
    }
    Ok(())
}

fn remove_object<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Player,
    args: &[String],
) -> Result<()> {
    if args.len() != 1 {
        reject::<_, ()>(host, player, &["::removeobject doesn't support arguments"])?;
    }
    let location = host.location(player)?.ok_or(Error::Null)?;
    let Some(object) = host.region_object(location)? else {
        return host.api_message(player, "All four objects on the tile were null.");
    };
    host.api_message(
        player,
        &format!(
            "The first object found on the tile was {}; it will now be removed.",
            object.id
        ),
    )?;
    host.scenery_remove(&object).map(|_| ())
}

/// `objects`' local `dump(label, objects)`.
fn dump<H: Host + ?Sized>(
    host: &mut H,
    player: &H::Player,
    label: &str,
    objects: &[Option<Scenery>],
) -> Result<()> {
    host.api_message(player, &format!("--- {label} ---"))?;
    let mut nulls = Vec::new();
    for (i, object) in objects.iter().enumerate() {
        let Some(o) = object else {
            nulls.push(i.to_string());
            continue;
        };
        let mut props = String::new();
        if o.constructed {
            props.push('C');
        }
        if o.renderable {
            props.push('R');
        }
        if o.active {
            props.push('A');
        }
        if !props.is_empty() {
            props = format!(" *{props}");
        }
        host.api_message(player, &format!("    {i}: {}{props}", o.java_string()))?;
    }
    if !nulls.is_empty() {
        host.api_message(player, &format!("    {}: null", nulls.join(", ")))?;
    }
    Ok(())
}

fn objects<H: Host + ?Sized>(host: &mut H, player: &H::Player, args: &[String]) -> Result<()> {
    if args.len() != 1 {
        reject::<_, ()>(host, player, &["::objects doesn't support arguments"])?;
    }
    let location = host.location(player)?.ok_or(Error::Null)?;
    let chunk = host.region_chunk(location)?;
    let (x, y) = ((location[0] & 63) & 7, (location[1] & 63) & 7);
    let mut stat = Vec::new();
    for i in 0..4 {
        if let Some(Some(object)) = host.chunk_static_objects(chunk, x, y).get(i).cloned() {
            stat.push(Some(object));
        }
    }
    dump(host, player, "Chunk static objects", &stat)?;
    let dynamic = host.chunk_objects(chunk, x, y)?;
    dump(host, player, "Chunk dynamic objects", &dynamic)?;
    let here = host.region_object(location)?;
    dump(host, player, "RegionManager", &[here])?;
    host.api_message(player, "--- That's all, folks! ---")
}
