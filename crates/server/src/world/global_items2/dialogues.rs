//! The dialogue files the everyday items open: EnchantedGemListener's
//! `EnchantedGemDialogue` (the slayer master through the gem) and
//! PetRockListener's `PetRockDialogue`, with the DialogueFile helpers they
//! call (`npc`, `npcl`, `player`, `options`, `end`, `showTopics`).
use super::{
    Anim, Dialogue, Error, Expression, Gfx, Host, QueueStrength, Result, Speaker, Task, animate,
    visualize,
};
use crate::world::global_items::jewellery::pluralise;

/// `DialogueConst.END_DIALOGUE`.
pub const END_DIALOGUE: i32 = 1_000_000;
/// The item that opens the slayer master's dialogue.
pub const ENCHANTED_GEM: i32 = 4155;
/// `Items.PET_ROCK_3695`.
pub const PET_ROCK: i32 = 3695;

// ---------------------------------------------------------------------- DialogueFile helpers

/// `DialUtils.splitLines(message)` (54 units a line).
pub fn split_lines(message: &str) -> Result<Vec<String>> {
    let text: Vec<u16> = message.encode_utf16().collect();
    let lines = crate::world::plain_dialogue::split_lines(&text, 54)
        .map_err(|e| Error::Binding(Some(e.to_string())))?;
    Ok(lines.iter().map(|l| String::from_utf16_lossy(l)).collect())
}

/// `DialogueFile.npc(expression, messages...)`: the file's NPC, or id 0
/// without one.
pub fn npc<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    npc: Option<&H::Node>,
    expression: Expression,
    lines: &[String],
) -> Result<()> {
    let speaker = match npc {
        Some(n) => Speaker::Node(n.clone()),
        None => Speaker::Id(0),
    };
    h.send_dialogues(player, speaker, Some(expression), lines)
}

/// `DialogueFile.npcl(expression, message)`: the message split into lines.
pub fn npcl<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    npc_node: Option<&H::Node>,
    expression: Expression,
    message: &str,
) -> Result<()> {
    let lines = split_lines(message)?;
    npc(h, player, npc_node, expression, &lines)
}

/// `DialogueFile.player(expression, messages...)` (`None`: the overload
/// without an expression).
pub fn player<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    expression: Option<Expression>,
    lines: &[String],
) -> Result<()> {
    h.send_dialogues(player, Speaker::Node(player.clone()), expression, lines)
}

/// `DialogueFile.options(options..., title)`.
pub fn options<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    options: &[&str],
    title: &str,
) -> Result<()> {
    h.send_options(player, title, options)
}

/// `DialogueFile.end()`: the interpreter closes.
pub fn end<H: Host + ?Sized>(h: &mut H, player: &H::Node) -> Result<()> {
    h.interpreter_close(player)?;
    Ok(())
}

/// A `Topic` or `IfTopic` with an Int stage (`show` is an IfTopic's
/// `showCondition`; a plain Topic always shows).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Topic {
    pub expression: Expression,
    pub text: &'static str,
    pub stage: i32,
    pub show: bool,
}

/// `DialogueFile.showTopics(topics..., title)`: the shown topics become the
/// interpreter's active topics; none ends (true); one is taken at once (its
/// stage, the player says it, the topics clear); more are offered.
pub fn show_topics<H: Host + ?Sized>(
    h: &mut H,
    player_node: &H::Node,
    stage: &mut i32,
    topics: &[Topic],
    title: &str,
) -> Result<bool> {
    let mut valid = Vec::new();
    for topic in topics.iter().filter(|t| t.show) {
        h.add_topic(player_node, topic.expression, topic.text, topic.stage)?;
        valid.push(topic.text);
    }
    match valid.len() {
        0 => Ok(true),
        1 => {
            let topic = topics
                .iter()
                .find(|t| t.text == valid[0])
                .ok_or(Error::Index)?;
            *stage = topic.stage;
            player(h, player_node, None, &[topic.text.to_owned()])?;
            h.clear_topics(player_node)?;
            Ok(false)
        }
        _ => {
            options(h, player_node, &valid, title)?;
            Ok(false)
        }
    }
}

// ---------------------------------------------------------------------- EnchantedGemListener

/// EnchantedGemListener's `activate`: opens a new EnchantedGemDialogue.
pub fn gem_activate<H: Host + ?Sized>(h: &mut H, player: &H::Node) -> Result<bool> {
    h.open_dialogue(
        player,
        Dialogue::Gem {
            first_run: true,
            stage: 0,
            npc: None,
        },
    )?;
    Ok(true)
}

/// ContentAPI `getSlayerMasterLocation(player)`.
pub fn slayer_master_location<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
) -> Result<&'static str> {
    let master = h.slayer_master(player)?.ok_or(Error::Null)?;
    Ok(match h.id(&master)? {
        1598 => "Zanaris",
        8275 => "Shilo Village",
        8274 => "Canifis",
        8273 => "Taverley",
        1597 => "Edgeville Dungeon",
        _ => "The Backrooms",
    })
}

/// `EnchantedGemDialogue.handle(interfaceId, buttonId)`: the file's NPC is
/// the slayer master every time.
pub fn gem_handle<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    first_run: &mut bool,
    stage: &mut i32,
    npc_field: &mut Option<H::Node>,
) -> Result<()> {
    let master = h.slayer_master(player)?.ok_or(Error::Null)?;
    *npc_field = Some(master);
    let npc_node = npc_field.clone();
    let npc_node = npc_node.as_ref();
    match *stage {
        0 => {
            let name = h.username(player)?;
            npcl(
                h,
                player,
                npc_node,
                Expression::Friendly,
                &format!("Hello there {name}, what can I help you with?"),
            )?;
            *stage += 1;
        }
        1 => {
            let topics = [
                Topic {
                    expression: Expression::Asking,
                    text: "How am I doing so far?",
                    stage: 100,
                    show: true,
                },
                Topic {
                    expression: Expression::HalfAsking,
                    text: "Who are you?",
                    stage: 200,
                    show: true,
                },
                Topic {
                    expression: Expression::HalfAsking,
                    text: "Where are you?",
                    stage: 300,
                    show: true,
                },
                Topic {
                    expression: Expression::Friendly,
                    text: "Got any tips for me?",
                    stage: 400,
                    show: true,
                },
                Topic {
                    expression: Expression::Friendly,
                    text: "Nothing really.",
                    stage: END_DIALOGUE,
                    show: *first_run,
                },
                Topic {
                    expression: Expression::Happy,
                    text: "That's all thanks.",
                    stage: END_DIALOGUE,
                    show: !*first_run,
                },
            ];
            show_topics(h, player, stage, &topics, "Select an Option")?;
        }
        100 => {
            *first_run = false;
            if !h.has_slayer_task(player)? {
                npcl(
                    h,
                    player,
                    npc_node,
                    Expression::HalfThinking,
                    "You need something new to hunt. Come and see me when you can and I'll give you a new task.",
                )?;
                *stage = 1;
            } else {
                let task = pluralise(&h.slayer_task_name(player)?);
                let kills = h.slayer_kills_remaining(player)?;
                npcl(
                    h,
                    player,
                    npc_node,
                    Expression::Friendly,
                    &format!("You're currently assigned to kill {task}; only {kills} more to go."),
                )?;
                let flags = h.slayer_task_flags(player)?;
                h.set_varp(player, 2502, flags >> 4, false)?;
                *stage = 1;
            }
        }
        200 => {
            *first_run = false;
            let master = h.slayer_master(player)?.ok_or(Error::Null)?;
            let name = h.name(&master)?;
            npcl(
                h,
                player,
                npc_node,
                Expression::Happy,
                &format!(
                    "My name's {}, I'm the Slayer Master best able to train you.",
                    name.as_deref().unwrap_or("null")
                ),
            )?;
            *stage = 1;
        }
        300 => {
            *first_run = false;
            let location = slayer_master_location(h, player)?;
            npcl(
                h,
                player,
                npc_node,
                Expression::Happy,
                &format!("You'll find me in {location}, I'll be here when you need a new task."),
            )?;
            *stage = 1;
        }
        400 => {
            *first_run = false;
            let tips = h.slayer_tip(player)?;
            npc(h, player, npc_node, Expression::Friendly, &tips)?;
            *stage += 1;
        }
        401 => {
            self::player(
                h,
                player,
                Some(Expression::Happy),
                &["Great, thanks!".to_owned()],
            )?;
            *stage = 1;
        }
        _ => {}
    }
    Ok(())
}

// ---------------------------------------------------------------------- PetRockListener

/// PetRockListener's `interact`: opens a new PetRockDialogue.
pub fn pet_rock_interact<H: Host + ?Sized>(h: &mut H, player: &H::Node) -> Result<bool> {
    h.open_dialogue(player, Dialogue::PetRock { stage: 0 })?;
    Ok(true)
}

/// ContentAPI `face(entity, location)` with a random location around the
/// player (`Location.getRandomLocation(getLocation(player), radius, true)`).
fn face_random<H: Host + ?Sized>(h: &mut H, player: &H::Node, radius: i32) -> Result<()> {
    let at = h.projectile_location(player)?.ok_or(Error::Null)?;
    let target = h.random_location(at, radius, true)?.ok_or(Error::Null)?;
    h.face_location(player, target)
}

/// `PetRockDialogue.handle(componentID, buttonID)`.
pub fn pet_rock_handle<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    stage: &mut i32,
    button: i32,
) -> Result<()> {
    match *stage {
        0 => {
            options(
                h,
                player,
                &["Talk", "Stroke", "Feed", "Fetch", "Stay"],
                "Select an Option",
            )?;
            *stage += 1;
        }
        1 => match button {
            1 => {
                h.player_dialogue(
                    player,
                    "Who's a good rock, then? Yes, you are! You're such a good rock. Ooga booga googa.",
                    Expression::Friendly,
                    false,
                )?;
                h.send_message(player, "Your rock seems a little happier.")?;
                *stage = END_DIALOGUE;
            }
            2 => {
                end(h, player)?;
                h.send_message(player, "You stroke your pet rock.")?;
                animate(h, player, Some(Anim::new(1333)))?;
                let duration = h.animation_duration(1333)?;
                h.queue_script(
                    player,
                    duration,
                    QueueStrength::Soft,
                    false,
                    Task::PetRockStroke {
                        player: player.clone(),
                    },
                )?;
            }
            3 => {
                end(h, player)?;
                h.send_message(player, "You try and feed the rock.")?;
                h.send_message(player, "Your rock doesn't seem hungry.")?;
            }
            4 => {
                h.player_dialogue(
                    player,
                    "Want to fetch the stick, rock? Of course you do.",
                    Expression::Friendly,
                    false,
                )?;
                face_random(h, player, 2)?;
                visualize(h, player, Anim::new(6665), Gfx::new(1157))?;
                let source = h.projectile_location(player)?.ok_or(Error::Null)?;
                let around = h.projectile_location(player)?.ok_or(Error::Null)?;
                let dest = h.random_location(around, 5, true)?.ok_or(Error::Null)?;
                h.spawn_projectile(source, dest, 1158, 40, 0, 150, 250, 25)?;
                *stage = END_DIALOGUE;
            }
            5 => {
                face_random(h, player, 2)?;
                h.player_dialogue(player, "Be a good rock...", Expression::Friendly, false)?;
                h.send_message(
                    player,
                    "You wait a few seconds and pick your rock back up and pet it.",
                )?;
                visualize(h, player, Anim::new(6664), Gfx::new(1156))?;
                *stage = END_DIALOGUE;
            }
            _ => {}
        },
        _ => {}
    }
    Ok(())
}

/// The pet rock's queued script: a message, then `stopExecuting`.
pub fn pet_rock_script<H: Host + ?Sized>(h: &mut H, player: &H::Node) -> Result<bool> {
    h.send_message(player, "Your rock seems much happier.")?;
    h.stop_executing(player)
}
