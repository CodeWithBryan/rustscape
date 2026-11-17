//! `ConsumableEffect` and every class of content/data/consumables/effects:
//! `activate` and `getHealthEffectValue`, in the original call order.
use super::{
    Attr, ContainerKind, Error, Host, Result, RumPulse, level, lifepoints, maximum_lifepoints,
    random, random_between, skills_call, static_level,
};
use crate::world::combat_pulse::HitsplatType;

/// `Skills` slots the effects name.
pub const ATTACK: i32 = 0;
pub const DEFENCE: i32 = 1;
pub const STRENGTH: i32 = 2;
pub const HITPOINTS: i32 = 3;
pub const RANGE: i32 = 4;
pub const PRAYER: i32 = 5;
pub const MAGIC: i32 = 6;
pub const SUMMONING: i32 = 23;
/// `Skills.NUM_SKILLS`.
pub const NUM_SKILLS: i32 = 24;
/// `Items.HOLY_WRENCH_6714`.
pub const HOLY_WRENCH: i32 = 6714;

/// `DiaryType`, by constant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Diary {
    Karamja,
    Varrock,
    Lumbridge,
    Falador,
    Fremennik,
    SeersVillage,
}

impl Diary {
    pub const ALL: [Self; 6] = [
        Self::Karamja,
        Self::Varrock,
        Self::Lumbridge,
        Self::Falador,
        Self::Fremennik,
        Self::SeersVillage,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Karamja => "KARAMJA",
            Self::Varrock => "VARROCK",
            Self::Lumbridge => "LUMBRIDGE",
            Self::Falador => "FALADOR",
            Self::Fremennik => "FREMENNIK",
            Self::SeersVillage => "SEERS_VILLAGE",
        }
    }

    /// `DiaryType.valueOf(name)`.
    pub fn value_of(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.name() == name)
    }
}

/// One `vararg args: Any` value of `AddTimerEffect` (a boxed `Int` or `Boolean`).
#[derive(Clone, Debug, PartialEq)]
pub enum TimerArg {
    Int(i32),
    Bool(bool),
}

/// One effect object. Fields are the original constructors' stored values.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// `AchievementEffect(diary, level, task)`.
    Achievement { diary: Diary, level: i32, task: i32 },
    /// `AddTimerEffect(identifier, *args)`.
    AddTimer {
        identifier: String,
        args: Vec<TimerArg>,
    },
    /// `CureDiseaseEffect()`.
    CureDisease,
    /// `DamageEffect(amt, isPercent, baseAmount)`.
    Damage {
        amt: f64,
        is_percent: bool,
        base_amount: i32,
    },
    /// `DraynorCabbageEffect()`.
    DraynorCabbage,
    /// `DwarvenRockCakeEffect()`.
    DwarvenRockCake,
    /// `EnergyEffect(int)`: the `double amt` field.
    Energy(f64),
    /// `HealingEffect(amount)`.
    Healing(i32),
    /// `KebabEffect()`.
    Kebab,
    /// `KegOfBeerEffect()`.
    KegOfBeer,
    /// `MatureWmbEffect()`.
    MatureWmb,
    /// `MultiEffect(*effects)`.
    Multi(Vec<Effect>),
    /// `PercentageHealthEffect(int)`: the stored `percentage * 0.01`.
    PercentageHealth(f64),
    /// `PercentHeal(base, percent)`.
    PercentHeal { base: i32, percent: f64 },
    /// `PoisonEffect(amount)`.
    Poison(i32),
    /// `PoisonKarambwanEffect()` (its own `PoisonEffect(5)`).
    PoisonKarambwan,
    /// `PrayerEffect(base, bonus, roundUp)`.
    Prayer {
        base: f64,
        bonus: f64,
        round_up: bool,
    },
    /// `RandomEnergyEffect(a, b)`.
    RandomEnergy(i32, i32),
    /// `RandomHealthEffect(a, b)`.
    RandomHealth(i32, i32),
    /// `RandomPrayerEffect(a, b)`.
    RandomPrayer(i32, i32),
    /// `RandomSkillEffect(skillSlot, a, b)`.
    RandomSkill { slot: i32, a: i32, b: i32 },
    /// `RemoveTimerEffect(identifier)`.
    RemoveTimer(String),
    /// `RestoreEffect(base, bonus, all_skills)`.
    Restore {
        base: f64,
        bonus: f64,
        all_skills: bool,
    },
    /// `RestoreSummoningSpecial()`.
    RestoreSummoningSpecial,
    /// `RockCakeEffect()`.
    RockCake,
    /// `SetAttributeEffect(attr, value, isTicks)`.
    SetAttribute {
        key: String,
        value: Attr,
        is_ticks: bool,
    },
    /// `SkillEffect(skill_slot, base, bonus)`.
    Skill { slot: i32, base: f64, bonus: f64 },
    /// `SmellingUgthankiKebabEffect()`.
    SmellingUgthankiKebab,
    /// `SummoningEffect(base, bonus)`.
    Summoning { base: f64, bonus: f64 },
    /// `SuperKebabEffect()`.
    SuperKebab,
    /// `TroubleBrewingRumEffect(forceChatMessage)`.
    TroubleBrewingRum(String),
    /// `UgthankiKebabEffect()`.
    UgthankiKebab,
    /// `WizardsMindBombEffect()`.
    WizardsMindBomb,
    /// `ZamorakMixEffect()`.
    ZamorakMix,
}

/// RestoreEffect's `SKILLS`.
pub const RESTORE_SKILLS: [i32; 5] = [DEFENCE, ATTACK, STRENGTH, MAGIC, RANGE];
/// RestoreEffect's `ALL_SKILLS` (every skill but hitpoints).
pub const RESTORE_ALL_SKILLS: [i32; 23] = [
    0, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
];

impl Effect {
    /// `EnergyEffect(int amt)`.
    pub fn energy(amount: i32) -> Self {
        Self::Energy(f64::from(amount))
    }

    /// `PercentageHealthEffect(int percentage)`: stores `percentage * 0.01`.
    pub fn percentage_health(percentage: i32) -> Self {
        Self::PercentageHealth(f64::from(percentage) * 0.01)
    }

    /// `DamageEffect(amt, isPercent)`.
    pub fn damage(amt: f64, is_percent: bool) -> Self {
        Self::Damage {
            amt,
            is_percent,
            base_amount: 0,
        }
    }

    /// `SetAttributeEffect(attr, value)`: ticks when the value is an `Integer`.
    pub fn set_attribute(key: &str, value: Attr) -> Self {
        let is_ticks = matches!(value, Attr::Int(_));
        Self::SetAttribute {
            key: key.into(),
            value,
            is_ticks,
        }
    }

    /// `effect.activate(player)`.
    pub fn activate<H: Host>(&self, host: &mut H) -> Result<()> {
        match self {
            Self::Achievement { diary, level, task } => host.finish_task(*diary, *level, *task),
            Self::AddTimer { identifier, args } => {
                host.remove_timer(identifier)?;
                let Some(timer) = host.spawn_timer(identifier, args)? else {
                    return Ok(());
                };
                host.register_timer(Some(&timer))
            }
            Self::CureDisease => {
                let Some(timer) = host.disease()? else {
                    return Ok(());
                };
                let left = host.disease_hits_left(&timer)?.wrapping_sub(9);
                host.set_disease_hits_left(&timer, left)?;
                if left <= 0 {
                    host.send_message("The disease has been cured.")?;
                    host.remove_disease()
                } else {
                    host.send_message("You feel slightly better.")
                }
            }
            Self::Damage { .. } => {
                let value = self.health_value(host)?;
                host.manual_hit(value.wrapping_neg(), HitsplatType::Normal)
            }
            Self::DraynorCabbage => {
                let value = self.health_value(host)?;
                Self::Healing(value).activate(host)
            }
            Self::DwarvenRockCake => {
                if lifepoints(host) > 2 {
                    Self::damage(1.0, false).activate(host)?;
                }
                Ok(())
            }
            Self::Energy(amount) => {
                let (settings, ctx) = host.settings();
                settings
                    .update_run_energy(ctx, -*amount)
                    .map_err(super::settings_error)
            }
            Self::Healing(amount) => skills_call(host, |s, c| s.heal(*amount, c)).map(|_| ()),
            Self::Kebab => kebab(host),
            Self::KegOfBeer => Self::Multi(vec![
                Self::Healing(15),
                Self::Skill {
                    slot: STRENGTH,
                    base: 10.0,
                    bonus: 0.0,
                },
                Self::Skill {
                    slot: ATTACK,
                    base: -40.0,
                    bonus: 0.0,
                },
            ])
            .activate(host),
            Self::MatureWmb => {
                let boost = if level(host, MAGIC)? > 50 { 4.0 } else { 3.0 };
                Self::Multi(vec![
                    Self::Skill {
                        slot: MAGIC,
                        base: boost,
                        bonus: 0.0,
                    },
                    Self::Healing(1),
                    Self::Skill {
                        slot: ATTACK,
                        base: -5.0,
                        bonus: 0.0,
                    },
                    Self::Skill {
                        slot: STRENGTH,
                        base: -5.0,
                        bonus: 0.0,
                    },
                    Self::Skill {
                        slot: DEFENCE,
                        base: -5.0,
                        bonus: 0.0,
                    },
                ])
                .activate(host)
            }
            Self::Multi(effects) => {
                for effect in effects {
                    effect.activate(host)?;
                }
                Ok(())
            }
            Self::PercentageHealth(_) => {
                let value = self.health_value(host)?;
                Self::Healing(value).activate(host)
            }
            Self::PercentHeal { base, percent } => {
                let max = maximum_lifepoints(host)?;
                let current = lifepoints(host);
                let amount = (f64::from(max) * percent).floor() as i32;
                let cap = ((1.0 + percent) * f64::from(max) - f64::from(current)) as i32;
                let amount = base.wrapping_add(amount.min(cap));
                let (s, _) = host.skills();
                s.heal_no_restrictions(amount);
                Ok(())
            }
            Self::Poison(amount) => host.manual_hit(*amount, HitsplatType::Poison),
            Self::PoisonKarambwan => {
                if lifepoints(host) > 5 {
                    Self::Poison(5).activate(host)?;
                }
                Ok(())
            }
            Self::Prayer {
                base,
                bonus,
                round_up,
            } => {
                let prayer = static_level(host, PRAYER)?;
                let mut b = *bonus;
                if host.contains(ContainerKind::Inventory, HOLY_WRENCH, 1)? {
                    b += 0.02;
                }
                let raw = base + f64::from(prayer) * b;
                let amount = if *round_up { raw.ceil() } else { raw.floor() };
                if amount > 0.0 {
                    skills_call(host, |s, c| s.increment_prayer(amount, c))
                } else if amount < 0.0 {
                    skills_call(host, |s, c| s.decrement_prayer(amount.abs(), c))
                } else {
                    Ok(())
                }
            }
            Self::RandomEnergy(a, b) => {
                let amount = random_between(host, *a, *b)?;
                Self::energy(amount).activate(host)
            }
            Self::RandomHealth(..) => {
                let value = self.health_value(host)?;
                if value > 0 {
                    Self::Healing(value).activate(host)
                } else {
                    Self::damage(f64::from(value), false).activate(host)
                }
            }
            Self::RandomPrayer(a, b) => {
                let amount = random_between(host, *a, *b)?;
                Self::Prayer {
                    base: f64::from(amount),
                    bonus: 0.0,
                    round_up: false,
                }
                .activate(host)
            }
            Self::RandomSkill { slot, a, b } => {
                let amount = random_between(host, *a, *b)?;
                Self::Skill {
                    slot: *slot,
                    base: f64::from(amount),
                    bonus: 0.0,
                }
                .activate(host)
            }
            Self::RemoveTimer(identifier) => host.remove_timer(identifier),
            Self::Restore {
                base,
                bonus,
                all_skills,
            } => {
                let list: &[i32] = if *all_skills {
                    &RESTORE_ALL_SKILLS
                } else {
                    &RESTORE_SKILLS
                };
                for &skill in list {
                    let stat = static_level(host, skill)?;
                    let boost = (base + f64::from(stat) * bonus) as i32;
                    let current = level(host, skill)?;
                    if current < stat {
                        skills_call(host, |s, c| s.update_level(skill, boost, stat, c))?;
                    }
                    if skill == PRAYER {
                        skills_call(host, |s, c| s.increment_prayer(f64::from(boost), c))?;
                    }
                }
                Ok(())
            }
            Self::RestoreSummoningSpecial => {
                if let Some(familiar) = host.familiar()? {
                    host.update_special_points(&familiar, -15)?;
                }
                Ok(())
            }
            Self::RockCake => {
                if lifepoints(host) > 1 {
                    Self::damage(10.0, true).activate(host)?;
                }
                Ok(())
            }
            Self::SetAttribute {
                key,
                value,
                is_ticks,
            } => {
                if *is_ticks {
                    let base = match value {
                        Attr::Int(v) => *v,
                        Attr::Null => return Err(Error::Null),
                        _ => return Err(Error::Type),
                    };
                    let ticks = host.ticks()?;
                    host.set_attribute(key, Attr::Int(base.wrapping_add(ticks)))
                } else {
                    host.set_attribute(key, value.clone())
                }
            }
            Self::Skill { slot, base, bonus } => {
                let stat = static_level(host, *slot)?;
                let delta = (base + bonus * f64::from(stat)) as i32;
                let maximum = if delta >= 0 {
                    stat.wrapping_add(delta)
                } else {
                    0
                };
                skills_call(host, |s, c| s.update_level(*slot, delta, maximum, c)).map(|_| ())
            }
            Self::SmellingUgthankiKebab => {
                if random(host, 100)? < 10 {
                    Self::Healing(9).activate(host)?;
                }
                Ok(())
            }
            Self::Summoning { base, bonus } => {
                let stat = static_level(host, SUMMONING)?;
                let amount = base + f64::from(stat) * bonus;
                skills_call(host, |s, c| {
                    s.update_level(SUMMONING, amount as i32, stat, c)
                })
                .map(|_| ())
            }
            Self::SuperKebab => {
                if random(host, 8)? < 5 {
                    Self::Multi(vec![Self::Healing(3), Self::percentage_health(7)])
                        .activate(host)?;
                }
                if random(host, 32)? < 1 {
                    let slot = random(host, NUM_SKILLS)?;
                    Self::Skill {
                        slot,
                        base: -1.0,
                        bonus: 0.0,
                    }
                    .activate(host)?;
                }
                Ok(())
            }
            Self::TroubleBrewingRum(chat) => host.run_pulse(RumPulse::main(chat)),
            Self::UgthankiKebab => {
                if lifepoints(host) < maximum_lifepoints(host)? {
                    host.send_chat("Yum!")?;
                }
                Self::Healing(19).activate(host)
            }
            Self::WizardsMindBomb => {
                let boost = if level(host, MAGIC)? > 50 { 3.0 } else { 2.0 };
                Self::Multi(vec![
                    Self::Skill {
                        slot: MAGIC,
                        base: boost,
                        bonus: 0.0,
                    },
                    Self::Healing(1),
                    Self::Skill {
                        slot: ATTACK,
                        base: -3.0,
                        bonus: 0.0,
                    },
                    Self::Skill {
                        slot: STRENGTH,
                        base: -4.0,
                        bonus: 0.0,
                    },
                    Self::Skill {
                        slot: DEFENCE,
                        base: -4.0,
                        bonus: 0.0,
                    },
                ])
                .activate(host)
            }
            Self::ZamorakMix => {
                let damage = (f64::from(lifepoints(host)) * 0.10 + 2.0) as i32;
                skills_call(host, |s, c| s.heal(6, c))?;
                host.manual_hit(damage, HitsplatType::Normal)
            }
        }
    }

    /// `effect.getHealthEffectValue(player)` (0 for the classes without one).
    pub fn health_value<H: Host>(&self, host: &mut H) -> Result<i32> {
        Ok(match self {
            Self::Damage {
                amt,
                is_percent,
                base_amount,
            } => {
                if *is_percent {
                    let amount = amt / 100.0;
                    (-(amount * f64::from(lifepoints(host)) + f64::from(*base_amount))) as i32
                } else {
                    (-(amt + f64::from(*base_amount))) as i32
                }
            }
            Self::DraynorCabbage => {
                if level(host, DEFENCE)? > 50 {
                    3
                } else {
                    4
                }
            }
            Self::DwarvenRockCake => {
                if lifepoints(host) > 2 {
                    -1
                } else {
                    0
                }
            }
            Self::Healing(amount) => *amount,
            Self::MatureWmb | Self::WizardsMindBomb => 1,
            Self::Multi(effects) => {
                let mut healing = 0i32;
                for effect in effects {
                    healing = healing.wrapping_add(effect.health_value(host)?);
                }
                healing
            }
            Self::PercentageHealth(percentage) => {
                (f64::from(maximum_lifepoints(host)?) * percentage) as i32
            }
            Self::Poison(amount) => amount.wrapping_neg(),
            Self::PoisonKarambwan => -5,
            Self::RandomHealth(a, b) => random_between(host, *a, *b)?,
            Self::RockCake => (f64::from(lifepoints(host)) * -0.1) as i32,
            Self::SmellingUgthankiKebab => {
                if random(host, 100)? < 10 {
                    9
                } else {
                    0
                }
            }
            Self::SuperKebab => {
                if random(host, 8)? < 5 {
                    (3.0 + f64::from(maximum_lifepoints(host)?) * 0.07) as i32
                } else {
                    0
                }
            }
            Self::UgthankiKebab => 19,
            Self::ZamorakMix => {
                6i32.wrapping_sub((f64::from(lifepoints(host)) * 0.10 + 2.0) as i32)
            }
            _ => 0,
        })
    }
}

/// KebabEffect.activate: one `nextInt(100)` picks the outcome.
fn kebab<H: Host>(host: &mut H) -> Result<()> {
    let roll = random(host, 100)?;
    if roll < 66 {
        let initial = lifepoints(host);
        Effect::percentage_health(10).activate(host)?;
        if lifepoints(host) > initial {
            host.packet_message("It heals some health.")?;
        }
    } else if roll < 87 {
        Effect::RandomHealth(10, 20).activate(host)?;
        host.packet_message("That was a good kebab. You feel a lot better.")?;
    } else if roll < 96 {
        if random(host, 100)? < 50 {
            let slot = random(host, NUM_SKILLS - 1)?;
            let effect = match slot {
                ATTACK | DEFENCE | STRENGTH => Effect::Multi(vec![
                    Effect::Skill {
                        slot: ATTACK,
                        base: -3.0,
                        bonus: 0.0,
                    },
                    Effect::Skill {
                        slot: DEFENCE,
                        base: -3.0,
                        bonus: 0.0,
                    },
                    Effect::Skill {
                        slot: STRENGTH,
                        base: -3.0,
                        bonus: 0.0,
                    },
                ]),
                _ => Effect::Skill {
                    slot,
                    base: -3.0,
                    bonus: 0.0,
                },
            };
            host.packet_message("That tasted a bit dodgy. You feel a bit ill.")?;
            effect.activate(host)?;
        } else {
            host.packet_message("That kebab didn't seem to do a lot.")?;
        }
    } else {
        Effect::Multi(vec![
            Effect::Healing(30),
            Effect::RandomSkill {
                slot: ATTACK,
                a: 1,
                b: 3,
            },
            Effect::RandomSkill {
                slot: DEFENCE,
                a: 1,
                b: 3,
            },
            Effect::RandomSkill {
                slot: STRENGTH,
                a: 1,
                b: 3,
            },
        ])
        .activate(host)?;
        host.packet_message("Wow, that was an amazing kebab! You feel really invigorated.")?;
    }
    Ok(())
}
