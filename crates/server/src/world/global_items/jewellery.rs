//! `content.data.EnchantedJewellery` and `EnchantedJewelleryListener` (with
//! its `EnchantedJewelleryDialogueFile`): rubbing or operating charged
//! jewellery, the destination choice, the teleport script and the charge.
use super::{
    Anim, Dialogue, Diary, Error, Expression, Gfx, Host, ItemArg, LogLevel, PulseType, Quest,
    QueueStrength, Result, Store, Task, TeleportMethod, TeleportType, as_item, reset_animator,
    teleport,
};

/// `Items.RING_OF_LIFE_2570`.
pub const RING_OF_LIFE: i32 = 2570;
/// `Items.ENCHANTED_GEM_4155`.
pub const ENCHANTED_GEM: i32 = 4155;
/// `this.javaClass` of the enum constants (the class `log` names).
pub const CLASS: &str = "content.data.EnchantedJewellery";

const ANIMATION: Anim = Anim::new(714);
const GRAPHICS: Gfx = Gfx {
    id: 308,
    height: 100,
    delay: 50,
};
const JEWELLERY_ANIMATION: Anim = Anim::new(9603);
const JEWELLERY_GRAPHICS: Gfx = Gfx::new(1684);

/// The `EnchantedJewellery` constants, in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Jewellery {
    RingOfSlaying,
    RingOfDueling,
    AmuletOfGlory,
    AmuletOfGloryT,
    GamesNecklace,
    DigsitePendant,
    CombatBracelet,
    SkillsNecklace,
    RingOfLife,
}

const GLORY_OPTIONS: [&str; 5] = [
    "Edgeville",
    "Karamja",
    "Draynor Village",
    "Al Kharid",
    "Nowhere",
];
const GLORY_LOCATIONS: [[i32; 3]; 4] = [
    [3087, 3495, 0],
    [2919, 3175, 0],
    [3104, 3249, 0],
    [3292, 3175, 0],
];

impl Jewellery {
    pub const ALL: [Self; 9] = [
        Self::RingOfSlaying,
        Self::RingOfDueling,
        Self::AmuletOfGlory,
        Self::AmuletOfGloryT,
        Self::GamesNecklace,
        Self::DigsitePendant,
        Self::CombatBracelet,
        Self::SkillsNecklace,
        Self::RingOfLife,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::RingOfSlaying => "RING_OF_SLAYING",
            Self::RingOfDueling => "RING_OF_DUELING",
            Self::AmuletOfGlory => "AMULET_OF_GLORY",
            Self::AmuletOfGloryT => "AMULET_OF_GLORY_T",
            Self::GamesNecklace => "GAMES_NECKLACE",
            Self::DigsitePendant => "DIGSITE_PENDANT",
            Self::CombatBracelet => "COMBAT_BRACELET",
            Self::SkillsNecklace => "SKILLS_NECKLACE",
            Self::RingOfLife => "RING_OF_LIFE",
        }
    }

    /// The dialogue options.
    pub fn options(self) -> &'static [&'static str] {
        match self {
            Self::RingOfSlaying => &[
                "Sumona in Pollnivneach.",
                "Morytania Slayer Tower.",
                "Rellekka Slayer Caves.",
                "Tarn's Lair.",
                "Nowhere. Give me a slayer update.",
            ],
            Self::RingOfDueling => &[
                "Al Kharid Duel Arena.",
                "Castle Wars Arena.",
                "Fist of Guthix",
                "Nowhere.",
            ],
            Self::AmuletOfGlory | Self::AmuletOfGloryT => &GLORY_OPTIONS,
            Self::GamesNecklace => &[
                "Burthorpe Games Room.",
                "Barbarian Outpost.",
                "Clan Wars.",
                "Wilderness Volcano.",
                "Corporeal Beast.",
            ],
            Self::DigsitePendant | Self::RingOfLife => &[],
            Self::CombatBracelet => &[
                "Champions' Guild.",
                "Monastery.",
                "Ranging Guild.",
                "Warriors' Guild.",
                "Nowhere.",
            ],
            Self::SkillsNecklace => &[
                "Fishing Guild.",
                "Mining Guild.",
                "Crafting Guild.",
                "Cooking Guild.",
                "Nowhere.",
            ],
        }
    }

    /// The teleport locations; RING_OF_LIFE's one location is
    /// `ServerConstants.HOME_LOCATION`, read by [`Self::location`].
    fn fixed_locations(self) -> &'static [[i32; 3]] {
        match self {
            Self::RingOfSlaying => &[
                [3361, 2994, 0],
                [3428, 3535, 0],
                [2792, 3615, 0],
                [3187, 4601, 0],
            ],
            Self::RingOfDueling => &[[3314, 3235, 0], [2442, 3089, 0], [1693, 5600, 0]],
            Self::AmuletOfGlory | Self::AmuletOfGloryT => &GLORY_LOCATIONS,
            Self::GamesNecklace => &[
                [2899, 3563, 0],
                [2520, 3571, 0],
                [3266, 3686, 0],
                [3179, 3685, 0],
                [2885, 4372, 2],
            ],
            Self::DigsitePendant => &[[3342, 3445, 0]],
            Self::CombatBracelet => &[
                [3191, 3365, 0],
                [3052, 3472, 0],
                [2657, 3439, 0],
                [2878, 3546, 0],
            ],
            Self::SkillsNecklace => &[
                [2611, 3392, 0],
                [3016, 3338, 0],
                [2933, 3290, 0],
                [3143, 3442, 0],
            ],
            Self::RingOfLife => &[],
        }
    }

    /// `locations.size`.
    pub fn location_count(self) -> i32 {
        match self {
            Self::RingOfLife => 1,
            _ => self.fixed_locations().len() as i32,
        }
    }

    /// The ordered item ids, most charges first.
    pub fn ids(self) -> &'static [i32] {
        match self {
            Self::RingOfSlaying => &[13281, 13282, 13283, 13284, 13285, 13286, 13287, 13288],
            Self::RingOfDueling => &[2552, 2554, 2556, 2558, 2560, 2562, 2564, 2566],
            Self::AmuletOfGlory => &[1712, 1710, 1708, 1706, 1704],
            Self::AmuletOfGloryT => &[10354, 10356, 10358, 10360, 10362],
            Self::GamesNecklace => &[3853, 3855, 3857, 3859, 3861, 3863, 3865, 3867],
            Self::DigsitePendant => &[11194, 11193, 11192, 11191, 11190],
            Self::CombatBracelet => &[11118, 11120, 11122, 11124, 11126],
            Self::SkillsNecklace => &[11105, 11107, 11109, 11111, 11113],
            Self::RingOfLife => &[RING_OF_LIFE],
        }
    }

    /// `isCrumble`.
    pub fn is_crumble(self) -> bool {
        !matches!(
            self,
            Self::AmuletOfGlory
                | Self::AmuletOfGloryT
                | Self::CombatBracelet
                | Self::SkillsNecklace
        )
    }

    /// `idMap[id]`.
    pub fn for_id(id: i32) -> Option<Self> {
        Self::ALL.into_iter().find(|j| j.ids().contains(&id))
    }

    /// `getItemIndex(item)`: `ids.indexOf(item.id)`.
    pub fn item_index(self, id: i32) -> i32 {
        self.ids()
            .iter()
            .position(|i| *i == id)
            .map_or(-1, |i| i as i32)
    }

    /// `isLastItemIndex(index)`.
    pub fn is_last_index(self, index: i32) -> bool {
        index == self.ids().len() as i32 - 1
    }

    /// `getNext(index)`: the next id, or the last one past the end.
    fn next(self, index: i32) -> i32 {
        let ids = self.ids();
        let i = index.wrapping_add(1);
        if i > ids.len() as i32 - 1 {
            ids[ids.len() - 1]
        } else {
            // index is at least -1 (indexOf), so i is never negative.
            ids[i as usize]
        }
    }

    /// `getLocation(index)`: the last location past the end, a negative
    /// index fails as the array read does.
    pub fn location<H: Host + ?Sized>(self, h: &mut H, index: i32) -> Result<[i32; 3]> {
        let count = self.location_count();
        let i = if index > count - 1 { count - 1 } else { index };
        if i < 0 {
            return Err(Error::Index);
        }
        match self {
            Self::RingOfLife => h.home_location().ok_or(Error::Null),
            _ => Ok(self.fixed_locations()[i as usize]),
        }
    }

    /// `getJewelleryType(item)`: a fixed word for four constants, otherwise
    /// the name's first word in the default locale's lower case.
    pub fn jewellery_type<H: Host + ?Sized>(self, h: &mut H, item: &H::Node) -> Result<String> {
        Ok(match self {
            Self::GamesNecklace => "games necklace".into(),
            Self::DigsitePendant | Self::SkillsNecklace => "necklace".into(),
            Self::CombatBracelet => "bracelet".into(),
            _ => {
                let name = h.name(item)?.ok_or(Error::Null)?;
                let first = name.split(' ').next().unwrap_or_default().to_owned();
                h.lowercase_default(&first)
            }
        })
    }
}

/// The keys of `EnchantedJewellery.idMap` in the order
/// `idMap.keys.toIntArray()` walks them: a JDK `HashMap<Integer, _>` filled
/// in declaration order (constants, then each constant's ids). Its table
/// doubles from 16 whenever the size passes three quarters of it; a key's
/// bucket is `(h ^ h >>> 16) & (n - 1)` with `h` the key itself, and a
/// bucket keeps insertion order (no bucket here reaches the tree threshold).
pub fn id_map_keys() -> Vec<i32> {
    let keys: Vec<i32> = Jewellery::ALL
        .iter()
        .flat_map(|j| j.ids().iter().copied())
        .collect();
    let mut capacity = 16usize;
    for size in 1..=keys.len() {
        if size > capacity * 3 / 4 {
            capacity *= 2;
        }
    }
    let bucket = |k: i32| {
        let h = k as u32;
        ((h ^ (h >> 16)) as usize) & (capacity - 1)
    };
    let mut ordered = keys;
    ordered.sort_by_key(|k| bucket(*k));
    ordered
}

/// `SlayerUtils.pluralise(str)`.
pub fn pluralise(text: &str) -> String {
    match text {
        "black bear" => "bears".into(),
        "cyclops" => "cyclopes".into(),
        "guard dog" => "dogs".into(),
        "dwarf" => "dwarves".into(),
        "elf warrior" => "elves".into(),
        "jelly" => "jellies".into(),
        "nechryael" => text.into(),
        "turoth" => text.into(),
        "tzhaar-mej" => "tzHaar".into(),
        "werewolf" => "werewolves".into(),
        "wolf" => "wolves".into(),
        "kalphite worker" => "kalphites".into(),
        "scarab swarm" => "scabarites".into(),
        _ => format!("{text}s"),
    }
}

/// What `attemptTeleport`'s queued script captured.
#[derive(Clone, Debug, PartialEq)]
pub struct Teleport<N> {
    pub jewellery: Jewellery,
    pub player: N,
    pub item: N,
    pub item_index: i32,
    pub next: N,
    pub location: [i32; 3],
    pub replace: bool,
    pub equipped: bool,
}

/// The listener's `handle(player, node, isEquipped)` (the `operate` lambda
/// passes true, `rub` false; both answer true).
pub fn handle<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    node: &H::Node,
    equipped: bool,
) -> Result<bool> {
    h.pulse_clear(p, PulseType::Standard)?;
    let item = as_item(h, node)?;
    if h.id(item)? == RING_OF_LIFE {
        h.send_message(p, "You can't operate that.")?;
        return Ok(true);
    }
    let id = h.id(item)?;
    let Some(j) = Jewellery::for_id(id) else {
        return Ok(true);
    };
    let index = j.item_index(h.id(item)?);
    if j.is_last_index(index) && !j.is_crumble() {
        let kind = j.jewellery_type(h, item)?;
        h.send_message(p, &format!("The {kind} has lost its charge."))?;
        h.send_message(
            p,
            "It will need to be recharged before you can use it again.",
        )?;
        return Ok(true);
    }
    let kind = j.jewellery_type(h, item)?;
    h.send_message(p, &format!("You rub the {kind}..."))?;
    if j.options().is_empty() {
        use_jewellery(h, j, p, item, 0, equipped)?;
        return Ok(true);
    }
    h.open_dialogue(
        p,
        Dialogue::Jewellery {
            player: p.clone(),
            jewellery: j,
            item: item.clone(),
            replace: true,
            equipped,
            stage: 0,
        },
    )?;
    Ok(true)
}

/// `EnchantedJewelleryDialogueFile.handle(componentID, buttonID)`.
pub fn dialogue<H: Host + ?Sized>(
    h: &mut H,
    dialogue: &mut Dialogue<H::Node>,
    button: i32,
) -> Result<()> {
    let Dialogue::Jewellery {
        player,
        jewellery,
        item,
        equipped,
        stage,
        ..
    } = dialogue
    else {
        return Err(Error::Binding(Some("not a jewellery dialogue".into())));
    };
    match *stage {
        0 => {
            h.send_options(
                player,
                "Where would you like to teleport to?",
                jewellery.options(),
            )?;
            *stage += 1;
        }
        1 => {
            // DialogueFile.end(): the loaded interpreter's close().
            h.interpreter_close(player)?;
            use_jewellery(
                h,
                *jewellery,
                &player.clone(),
                &item.clone(),
                button.wrapping_sub(1),
                *equipped,
            )?;
        }
        _ => {}
    }
    Ok(())
}

/// `EnchantedJewellery.use(player, item, buttonID, isEquipped)`.
pub fn use_jewellery<H: Host + ?Sized>(
    h: &mut H,
    j: Jewellery,
    p: &H::Node,
    item: &H::Node,
    button: i32,
    equipped: bool,
) -> Result<()> {
    if button > j.location_count() - 1 {
        if is_slayer_ring(h, item)? {
            slayer_progress(h, p)?;
        }
        return Ok(());
    }
    attempt_teleport(h, j, p, item, button, true, equipped)?;
    Ok(())
}

fn is_slayer_ring<H: Host + ?Sized>(h: &mut H, item: &H::Node) -> Result<bool> {
    Ok(Jewellery::RingOfSlaying.ids().contains(&h.id(item)?))
}

/// `EnchantedJewellery.attemptTeleport(player, item, buttonID, replace,
/// isEquipped)`: the jewellery zone rule, Tarn's Lair, the script delay and
/// the queued teleport script.
pub fn attempt_teleport<H: Host + ?Sized>(
    h: &mut H,
    j: Jewellery,
    p: &H::Node,
    item: &H::Node,
    button: i32,
    replace: bool,
    equipped: bool,
) -> Result<bool> {
    let item_index = j.item_index(h.id(item)?);
    let next = h.new_item(j.next(item_index), 1)?;
    if !h.zone_teleport(p, 1, &next)? {
        return Ok(false);
    }
    if j == Jewellery::RingOfSlaying
        && button == 3
        && !h.is_quest_complete(p, Quest::HauntedMine)?
    {
        h.send_message(
            p,
            "You must complete the Haunted Mine quest to access Tarn's Lair.",
        )?;
        return Ok(false);
    }
    let location = j.location(h, button)?;
    let delay = h.ticks().wrapping_add(4);
    h.set_script_delay(p, delay)?;
    h.queue_script(
        p,
        0,
        QueueStrength::Soft,
        false,
        Task::Jewellery(Teleport {
            jewellery: j,
            player: p.clone(),
            item: item.clone(),
            item_index,
            next,
            location,
            replace,
            equipped,
        }),
    )?;
    Ok(true)
}

/// The queued teleport script's body for one stage.
pub fn teleport_script<H: Host + ?Sized>(
    h: &mut H,
    t: &Teleport<H::Node>,
    stage: i32,
) -> Result<bool> {
    let p = &t.player;
    let animated = matches!(
        t.jewellery,
        Jewellery::RingOfDueling | Jewellery::GamesNecklace
    );
    match stage {
        0 => {
            h.lock(p, 4)?;
            if animated {
                h.visualize(p, JEWELLERY_ANIMATION, JEWELLERY_GRAPHICS)?;
            } else {
                h.visualize(p, ANIMATION, GRAPHICS)?;
            }
            let at = h.location(p)?.ok_or(Error::Null)?;
            h.play_global_audio(at, 200, 0, 1, 8)?;
            h.set_disabled_ticks(p, 4)?;
            h.close_interface(p)?;
            return h.delay_script(p, 3);
        }
        1 => {
            teleport(h, p, t.location, TeleportType::Instant)?;
            reset_animator(h, p)?;
            if animated {
                h.graphics(p, JEWELLERY_GRAPHICS)?;
            }
            h.unlock(p)?;
            h.teleport_event(
                p,
                TeleportType::Normal,
                TeleportMethod::Jewelry,
                &t.item,
                t.location,
            )?;
            if Jewellery::DigsitePendant.ids().contains(&h.id(&t.item)?) {
                h.finish_diary_task(p, Diary::Varrock, 1, 10)?;
            }
            if !t.replace {
                return h.stop_executing(p);
            }
            if t.jewellery.is_last_index(t.item_index) {
                if is_slayer_ring(h, &t.item)? {
                    if t.equipped {
                        // Removed now; the gem is added on the next stage.
                        if h.remove_item(p, ItemArg::Item(t.item.clone()), Store::Equipment)? {
                            return h.delay_script(p, 1);
                        }
                        h.log(
                            CLASS,
                            LogLevel::Err,
                            "Error replacing slayer ring with enchanted gem (removeItem from equipment)",
                        )?;
                    }
                    let slot = h.slot(&t.item)?;
                    let gem = h.new_item(ENCHANTED_GEM, 1)?;
                    if h.replace_slot(p, slot, &gem, Some(&t.item), Store::Inventory)?
                        .as_ref()
                        == Some(&t.item)
                    {
                        return h.stop_executing(p);
                    }
                    h.log(
                        CLASS,
                        LogLevel::Err,
                        "Error replacing slayer ring with enchanted gem (replaceSlot)",
                    )?;
                }
                if t.jewellery.is_crumble() {
                    let from = if t.equipped {
                        Store::Equipment
                    } else {
                        Store::Inventory
                    };
                    if h.remove_item(p, ItemArg::Item(t.item.clone()), from)? {
                        return h.stop_executing(p);
                    }
                    h.log(CLASS, LogLevel::Err, "Error crumbling jewelry")?;
                }
            } else {
                let slot = h.slot(&t.item)?;
                if t.equipped {
                    h.replace_slot(p, slot, &t.next, Some(&t.item), Store::Equipment)?;
                } else {
                    h.replace_slot(p, slot, &t.next, None, Store::Inventory)?;
                }
            }
        }
        2 => {
            h.add_item_or_drop(p, ENCHANTED_GEM, 1)?;
            h.send_message(
                p,
                "Your Ring of Slaying reverts back into a regular enchanted gem.",
            )?;
        }
        _ => {}
    }
    h.stop_executing(p)
}

/// `slayerProgressDialogue(player)`.
fn slayer_progress<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<()> {
    if !h.slayer_has_task(p)? {
        let npc = h.slayer_master_npc(p)?.ok_or(Error::Null)?;
        return h.npc_dialogue(
            p,
            npc,
            "You need something new to hunt. Come and see me when you can and I'll give you a new task.",
            Expression::HalfGuilty,
            false,
        );
    }
    let npc = h.slayer_master_npc(p)?.ok_or(Error::Null)?;
    let name = h.slayer_task_name(p)?;
    let plural = pluralise(&name);
    let kills = h.slayer_kills_remaining(p)?;
    h.npc_dialogue(
        p,
        npc,
        &format!("You're currently assigned to kill {plural} only {kills} more to go."),
        Expression::Friendly,
        false,
    )?;
    let flags = h.slayer_task_flags(p)?;
    h.set_varp(p, 2502, flags >> 4, false)
}
