//! `NPCTalkListener`: the barcrawl "talk-to" handler and the catch-all NPC
//! "talk-to"/"talk"/"talk to" handler (random event and holiday NPCs,
//! facing, the combat refusals, gnome cooking deliveries, then the NPC's
//! dialogue).
use super::{Arg, Error, Host, Kind, Result, Value};

/// `NPCTalkListener.barCrawlNPCs`.
pub const BARCRAWL_NPCS: [i32; 11] = [733, 848, 735, 739, 737, 738, 731, 568, 3217, 736, 734];

/// `BarcrawlType`'s constants in ordinal order with their NPC ids.
pub const BARCRAWL_TYPES: [(&str, &[i32]); 10] = [
    ("BLUE_MOON", &[733]),
    ("BLUEBERRY_BAR", &[848]),
    ("DEADMAN_CHEST", &[735]),
    ("DRAGON_INN", &[739]),
    ("FLYING_HORSE_INN", &[737]),
    ("FORESTERS_ARMS", &[738]),
    ("JOLLY_BOAR", &[731]),
    ("KARAMJA_SPIRITS", &[568]),
    ("RISING_SUNN_INN", &[3217, 736]),
    ("RUSTY_ANCHOR_INN", &[734]),
];

/// `GnomeCookingJob`'s constants in ordinal order with their `npc_id`.
pub const GNOME_COOKING_JOBS: [(&str, i32); 15] = [
    ("CPT_ERRDO", 3811),
    ("DALILAH", 4588),
    ("GULLUCK", 602),
    ("ROMETTI", 601),
    ("NARNODE", 670),
    ("MEEGLE", 4597),
    ("PERRDUR", 4587),
    ("SARBLE", 4599),
    ("GIMLEWAP", 4580),
    ("BLEEMADGE", 3810),
    ("DALBUR", 3809),
    ("BOLREN", 469),
    ("SCHEPBUR", 3817),
    ("IMBLEWYN", 4586),
    ("ONGLEWIP", 4585),
];

/// `"$GC_BASE_ATTRIBUTE:$GC_JOB_ORDINAL"`.
pub const GC_JOB_ORDINAL: &str = "gnome_cooking:job:job_ordinal";
/// `"$GC_BASE_ATTRIBUTE:$GC_JOB_COMPLETE"`.
pub const GC_JOB_COMPLETE: &str = "gnome_cooking:job:job_complete";
/// `NPCs.SANDWICH_LADY_3117`.
pub const SANDWICH_LADY: i32 = 3117;

/// `BarcrawlType.forId(id)`: the ordinal and name of the first constant
/// listing the id.
pub fn barcrawl_type(id: i32) -> Option<(i32, &'static str)> {
    BARCRAWL_TYPES
        .iter()
        .enumerate()
        .find(|(_, (_, ids))| ids.contains(&id))
        .map(|(ordinal, (name, _))| (ordinal as i32, *name))
}

/// The barcrawl NPCs' "talk-to"/"talk" handler.
pub fn barcrawl<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Npc,
    node: Option<&H::Npc>,
) -> Result<bool> {
    let node = node.ok_or(Error::Null)?;
    let kind = barcrawl_type(h.id(node)?);
    h.barcrawl_instance(player)?;
    let not_now = h.barcrawl_finished(player)? || !h.barcrawl_started(player)? || {
        let (ordinal, _) = kind.ok_or(Error::Null)?;
        h.barcrawl_completed(player, ordinal)?
    };
    if not_now {
        let id = h.id(node)?;
        h.open_dialogue_id(player, id, &[Arg::Node(Some(node.clone()))])?;
    } else {
        let id = h.id(node)?;
        let (_, name) = kind.ok_or(Error::Null)?;
        h.open_dialogue_name(
            player,
            "barcrawl dialogue",
            &[Arg::Int(id), Arg::Barcrawl(name)],
        )?;
    }
    Ok(true)
}

/// `getEventNpc(player) == null || getEventNpc(player) != npc ||
/// getEventNpc(player)?.finalized == true`.
#[allow(clippy::type_complexity)]
fn not_this_event<H: Host + ?Sized>(
    h: &mut H,
    npc: &H::Npc,
    lookup: fn(&mut H, &H::Npc) -> Result<Option<H::Npc>>,
    player: &H::Npc,
) -> Result<bool> {
    if lookup(h, player)?.is_none() {
        return Ok(true);
    }
    if !lookup(h, player)?.is_some_and(|e| h.same(&e, npc)) {
        return Ok(true);
    }
    match lookup(h, player)? {
        Some(e) => h.finalized(&e),
        None => Ok(false),
    }
}

/// The catch-all NPC "talk-to"/"talk"/"talk to" handler.
pub fn talk<H: Host + ?Sized>(h: &mut H, player: &H::Npc, node: Option<&H::Npc>) -> Result<bool> {
    let node = node.ok_or(Error::Null)?;
    // node.asNpc()
    if !h.kind(node).is_npc() {
        return Err(Error::Cast);
    }
    let npc = node;
    let id = h.id(node)?;
    if h.random_event_ids()?.contains(&id) {
        if not_this_event(h, npc, H::anti_macro_event_npc, player)? {
            if h.id(npc)? == SANDWICH_LADY {
                h.send_dialogue(
                    player,
                    &[
                        "The sandwich lady doesn't seem interested in selling you any",
                        "refreshments.",
                    ],
                )?;
            } else {
                h.api_message(player, "They aren't interested in talking to you.")?;
            }
        } else if let Some(event) = h.anti_macro_event_npc(player)? {
            h.talk_to(&event, node)?;
        }
        return Ok(true);
    }
    let id = h.id(node)?;
    if h.holiday_random_ids()?.contains(&id) && h.kind(node) == Kind::HolidayRandomEventNpc {
        if not_this_event(h, npc, H::holiday_event_npc, player)? {
            h.player_message(player, "They aren't interested in talking to you.")?;
        } else if let Some(event) = h.holiday_event_npc(player)? {
            h.talk_to(&event, node)?;
        }
        return Ok(true);
    }
    let extra = h.api_get_attribute(npc, "holiday_random_extra_npc")?;
    if Value::bool(extra, false)? && h.holiday_event_npc(player)?.is_some() {
        if let Some(event) = h.holiday_event_npc(player)? {
            h.talk_to(&event, node)?;
        }
        return Ok(true);
    }
    let facing = h.get_attribute(npc, "facing_booth")?;
    if !Value::bool(facing, false)? {
        let at = h.location(player)?;
        h.face_location(npc, at)?;
    }
    // ---------------- THESE DIALOGUES ARE INAUTHENTIC BUT ARE COPING FOR A CORE ISSUE
    if h.combat_victim(player)?.is_some_and(|v| h.same(&v, npc)) {
        h.api_message(
            player,
            "I don't think they have any interest in talking to me right now!",
        )?;
        return Ok(true);
    }
    if h.in_combat(npc)? {
        h.api_message(player, "They look a bit busy at the moment.")?;
        return Ok(true);
    }
    let ordinal = h.get_attribute(player, GC_JOB_ORDINAL)?;
    if Value::int(ordinal, -1)? != -1 {
        let ordinal = h.get_attribute(player, GC_JOB_ORDINAL)?;
        let ordinal = Value::int(ordinal, -1)?;
        let (job, npc_id) = *usize::try_from(ordinal)
            .ok()
            .and_then(|i| GNOME_COOKING_JOBS.get(i))
            .ok_or(Error::Index)?;
        if h.id(node)? == npc_id {
            let complete = h.get_attribute(player, GC_JOB_COMPLETE)?;
            if !Value::bool(complete, false)? {
                let file = h.gnome_completion_dialogue(job)?;
                h.open_file(player, file)?;
                return Ok(true);
            }
        }
    }
    let id = h.id(npc)?;
    h.open_dialogue_id(player, id, &[Arg::Node(Some(npc.clone()))])
}
