//! `ImpactHandler`: applies a hit to its entity, queues the hitsplat and
//! keeps the damage logs that later decide who gets credit for a kill.
use super::{Host, Prayer, Task, WorldPulse, pulse, skill, slot};
use crate::world::combat_swing::{CombatStyle, Error, Result};
use rs09_game_core::java_map::int_key_layout;
use std::collections::VecDeque;

/// `ImpactHandler.HitsplatType`, in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitsplatType {
    Miss,
    Normal,
    Poison,
    Disease,
    Normal1,
    Venom,
}

impl HitsplatType {
    pub const ALL: [Self; 6] = [
        Self::Miss,
        Self::Normal,
        Self::Poison,
        Self::Disease,
        Self::Normal1,
        Self::Venom,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Miss => "MISS",
            Self::Normal => "NORMAL",
            Self::Poison => "POISON",
            Self::Disease => "DISEASE",
            Self::Normal1 => "NORMAL_1",
            Self::Venom => "VENOM",
        }
    }
}

/// `ImpactHandler.Impact`.
pub struct Impact<H: Host> {
    pub source: Option<H::Entity>,
    pub amount: i32,
    pub style: Option<CombatStyle>,
    pub kind: Option<HitsplatType>,
}

impl<H: Host> Clone for Impact<H> {
    fn clone(&self) -> Self {
        Self {
            source: self.source.clone(),
            amount: self.amount,
            style: self.style,
            kind: self.kind,
        }
    }
}

/// The fields of one `ImpactHandler`.
pub struct ImpactState<H: Host> {
    /// The tick impacts are disabled until.
    pub disabled_ticks: i32,
    /// `npcImpactLog`: damage by source entity, a null source included, in
    /// insertion order with each key's identity hash.
    pub npc_log: Vec<(Option<H::Entity>, i32, i32)>,
    /// The table size `npcImpactLog` had when it was last cleared.
    pub npc_capacity: usize,
    /// `playerImpactLog`: damage by player uid, in insertion order.
    pub player_log: Vec<(i32, i32)>,
    /// The table size `playerImpactLog` had when it was last cleared.
    pub player_capacity: usize,
    /// `impactQueue`.
    pub queue: VecDeque<Impact<H>>,
}

impl<H: Host> Default for ImpactState<H> {
    fn default() -> Self {
        Self {
            disabled_ticks: 0,
            npc_log: Vec::new(),
            npc_capacity: 16,
            player_log: Vec::new(),
            player_capacity: 16,
            queue: VecDeque::new(),
        }
    }
}

/// `HashMap` iteration order of keys with the given hash codes, inserted in
/// slice order into a table of `capacity` buckets. Answers the positions in
/// iteration order and the final table size. A bucket that the JDK would turn
/// into a tree is refused: its order depends on the tree's rotations.
fn hashed_order(hashes: &[i32], mut capacity: usize) -> Result<(Vec<usize>, usize)> {
    let spread = |hash: i32| (hash ^ ((hash as u32) >> 16) as i32) as u32 as usize;
    let mut table: Vec<Vec<usize>> = vec![Vec::new(); capacity];
    let grow = |table: &mut Vec<Vec<usize>>, capacity: &mut usize| {
        *capacity *= 2;
        let mut next: Vec<Vec<usize>> = vec![Vec::new(); *capacity];
        for bucket in table.iter() {
            for &at in bucket {
                next[spread(hashes[at]) & (*capacity - 1)].push(at);
            }
        }
        *table = next;
    };
    for (at, &hash) in hashes.iter().enumerate() {
        let bucket = spread(hash) & (capacity - 1);
        table[bucket].push(at);
        if table[bucket].len() > 8 {
            if capacity >= 64 {
                return Err(Error::Binding("impact log tree bin".into()));
            }
            grow(&mut table, &mut capacity);
        }
        if (at + 1) * 4 > capacity * 3 {
            grow(&mut table, &mut capacity);
        }
    }
    Ok((table.into_iter().flatten().collect(), capacity))
}

impl<H: Host> ImpactState<H> {
    /// `isHitUpdate()`.
    pub fn is_hit_update(&self) -> bool {
        !self.queue.is_empty()
    }

    /// The NPC log's entries in `HashMap` iteration order.
    pub fn npc_order(&self) -> Result<Vec<usize>> {
        let hashes: Vec<i32> = self.npc_log.iter().map(|entry| entry.1).collect();
        Ok(hashed_order(&hashes, self.npc_capacity)?.0)
    }

    /// The player log's uids in `HashMap` iteration order.
    pub fn player_order(&self) -> Vec<i32> {
        let keys = self.player_log.iter().map(|entry| entry.0);
        int_key_layout(keys, self.player_capacity).0
    }

    /// `getNpcImpactLog().clear()`: the table keeps its size.
    pub fn clear_npc_log(&mut self) -> Result<()> {
        let hashes: Vec<i32> = self.npc_log.iter().map(|entry| entry.1).collect();
        self.npc_capacity = hashed_order(&hashes, self.npc_capacity)?.1;
        self.npc_log.clear();
        Ok(())
    }

    /// `getPlayerImpactLog().clear()`: the table keeps its size.
    pub fn clear_player_log(&mut self) {
        let keys = self.player_log.iter().map(|entry| entry.0);
        self.player_capacity = int_key_layout(keys, self.player_capacity).1;
        self.player_log.clear();
    }
}

/// `setDisabledTicks(ticks)`.
pub fn set_disabled_ticks<H: Host>(h: &mut H, owner: &H::Entity, ticks: i32) -> Result<()> {
    let now = h.ticks()?;
    h.impact_handler(owner).disabled_ticks = now.wrapping_add(ticks);
    Ok(())
}

/// `manualHit(source, hit, type)`.
pub fn manual_hit<H: Host>(
    h: &mut H,
    owner: &H::Entity,
    source: Option<&H::Entity>,
    hit: i32,
    kind: Option<HitsplatType>,
) -> Result<Option<Impact<H>>> {
    let mut hit = hit;
    if hit > h.lifepoints(owner)? {
        hit = h.lifepoints(owner)?;
    }
    handle_impact(h, owner, source, hit, None, kind, false)
}

/// `manualHit(source, hit, type, ticks)`: a positive delay submits a pulse
/// and answers nothing.
pub fn manual_hit_delayed<H: Host>(
    h: &mut H,
    owner: &H::Entity,
    source: Option<&H::Entity>,
    hit: i32,
    kind: Option<HitsplatType>,
    ticks: i32,
) -> Result<Option<Impact<H>>> {
    if ticks > 0 {
        h.submit(WorldPulse::new(
            ticks,
            vec![Some(owner.clone())],
            Task::ManualHit {
                owner: owner.clone(),
                source: source.cloned(),
                hit,
                kind,
            },
        ))?;
        return Ok(None);
    }
    manual_hit(h, owner, source, hit, kind)
}

/// `handleImpact(source, hit, style, state, type, secondary)`. The original
/// reads neither `state` nor `secondary`.
pub fn handle_impact<H: Host>(
    h: &mut H,
    owner: &H::Entity,
    source: Option<&H::Entity>,
    hit: i32,
    style: Option<CombatStyle>,
    kind: Option<HitsplatType>,
    _secondary: bool,
) -> Result<Option<Impact<H>>> {
    let (mut hit, mut kind) = (hit, kind);
    if h.is_dead(owner)? {
        pulse::set_victim(h, owner, None)?;
        pulse::stop(h, owner)?;
        return Ok(None);
    }
    let mut source = source.cloned();
    let fam = source.as_ref().is_some_and(|s| pulse::is_familiar(h, s));
    if fam && let Some(familiar) = source.clone() {
        source = h.familiar_owner(&familiar)?;
    }
    let disabled = h.impact_handler(owner).disabled_ticks;
    if disabled > h.ticks()? {
        return Ok(None);
    }
    if h.kind(owner).is_player()
        && !h.is_ai_player(owner)
        && !h.attribute_flag(owner, "tutorial:complete")?
    {
        let impact = Impact {
            source,
            amount: 0,
            style,
            kind: Some(HitsplatType::Miss),
        };
        h.impact_handler(owner).queue.push_back(impact.clone());
        return Ok(Some(impact));
    }
    hit = hit.wrapping_sub(h.skills_hit(owner, hit)?);
    if kind.is_none() || kind == Some(HitsplatType::Normal) {
        kind = Some(if hit == 0 {
            HitsplatType::Miss
        } else {
            HitsplatType::Normal
        });
    } else if hit == 0 && kind == Some(HitsplatType::Poison) {
        return Ok(None);
    }
    let source_player = source.clone().filter(|s| h.kind(s).is_player());
    if hit > 0 {
        if let Some(player) = &source_player {
            let uid = h.uid(player)?;
            let log = &mut h.impact_handler(owner).player_log;
            match log.iter_mut().find(|entry| entry.0 == uid) {
                Some(entry) => entry.1 = hit.wrapping_add(entry.1),
                None => log.push((uid, hit)),
            }
        } else {
            let found = {
                let keys: Vec<Option<H::Entity>> = h
                    .impact_handler(owner)
                    .npc_log
                    .iter()
                    .map(|entry| entry.0.clone())
                    .collect();
                keys.iter().position(|key| match (key, &source) {
                    (Some(a), Some(b)) => h.same_entity(a, b),
                    (None, None) => true,
                    _ => false,
                })
            };
            match found {
                Some(at) => {
                    let entry = &mut h.impact_handler(owner).npc_log[at];
                    entry.2 = hit.wrapping_add(entry.2);
                }
                None => {
                    let hash = source.as_ref().map_or(0, |s| h.identity_hash(s));
                    h.impact_handler(owner)
                        .npc_log
                        .push((source.clone(), hash, hit));
                }
            }
        }
    }
    if style.is_some()
        && let Some(player) = &source_player
    {
        if fam && h.has_familiar(player)? && !h.familiar_is_pet(player)? {
            h.set_fam_exp(player)?;
        }
        h.remove_attribute(player, "fam-exp")?;
    }
    let mut dead = false;
    if h.lifepoints(owner)? < 1 {
        pulse::stop(h, owner)?;
        h.face(owner, source.as_ref())?;
        let killer = most_damage_entity(h, owner, source.as_ref())?;
        h.start_death(owner, killer.as_ref())?;
        dead = true;
    } else if f64::from(h.lifepoints(owner)?) < f64::from(h.maximum_lifepoints(owner)?) * 0.1
        && h.kind(owner).is_player()
        && h.prayer_active(owner, Prayer::Redemption)?
    {
        h.start_redemption(owner)?;
    }
    if h.kind(owner).is_player() && h.attribute_flag(owner, "godMode")? {
        h.heal(owner, 10000)?;
    }
    let impact = Impact {
        source,
        amount: hit,
        style,
        kind,
    };
    h.impact_handler(owner).queue.push_back(impact.clone());
    if h.kind(owner).is_player()
        && !dead
        && h.zone_type(owner)? != 1
        && h.equipment_contains(owner, 2570, 1)?
    {
        let percentage = (f64::from(h.static_level(owner, skill::HITPOINTS)?) * 0.10) as i32;
        if h.lifepoints(owner)? <= percentage && h.ring_of_life_teleport(owner)? {
            h.player_send_message(
                owner,
                "Your ring of life saves you and in the process is destroyed.",
            )?;
        }
    }
    Ok(Some(impact))
}

/// `handleRecoilEffect(attacker, hit)`.
pub fn handle_recoil_effect<H: Host>(
    h: &mut H,
    owner: &H::Entity,
    attacker: &H::Entity,
    hit: i32,
) -> Result<()> {
    let mut damage = (f64::from(hit) * 0.1).ceil() as i32;
    if h.kind(owner).is_player() {
        let current = h.recoil_damage(owner)?;
        if damage >= current {
            damage = current;
            h.send_message(owner, "Your Ring of Recoil has shattered.")?;
            h.equipment_replace(owner, None, slot::RING)?;
            h.set_recoil_damage(owner, 40)?;
        } else {
            h.set_recoil_damage(owner, current.wrapping_sub(damage))?;
        }
    }
    if damage > 0 {
        manual_hit(h, attacker, Some(owner), damage, Some(HitsplatType::Normal))?;
    }
    Ok(())
}

/// `getMostDamageEntity(killer)`: who a kill is credited to.
pub fn most_damage_entity<H: Host>(
    h: &mut H,
    owner: &H::Entity,
    killer: Option<&H::Entity>,
) -> Result<Option<H::Entity>> {
    if h.kind(owner).is_player() {
        return Ok(killer.cloned());
    }
    if killer.is_some_and(|k| h.is_ai_player(k)) {
        return Ok(killer.cloned());
    }
    let mut entity = Some(owner.clone());
    let mut damage = -1;
    if h.impact_handler(owner).player_log.is_empty() {
        for at in h.impact_handler(owner).npc_order()? {
            let (key, _, amount) = h.impact_handler(owner).npc_log[at].clone();
            if key.as_ref().is_some_and(|e| h.same_entity(e, owner)) {
                continue;
            }
            if amount > damage {
                damage = amount;
                entity = key;
            }
        }
        return Ok(entity);
    }
    let mut player = 0;
    for uid in h.impact_handler(owner).player_order() {
        let log = &h.impact_handler(owner).player_log;
        let amount = log.iter().find(|entry| entry.0 == uid).map_or(0, |e| e.1);
        if amount > damage {
            damage = amount;
            player = uid;
        }
    }
    h.player_by_uid(player)
}
