//! `content.global.skill.prayer.PrayerAltarListener`: the altars' "pray"
//! and "pray-at" options.
//!
//! Ported from the pinned source without a JVM corpus: the unit tests below
//! check the order of the calls against the Kotlin by reading, not against a
//! recorded run. Every outside call is a [`Host`] method.
//!
//! Covered: the first `on` lambda ([`pray_at`]) with the private `pray` and
//! the chaos altar's queued script ([`trap`]). Not covered: the second
//! registration, the chaos altar 61's "check" option (Merlin's Crystal's
//! quest stage).

/// `Skills.HITPOINTS` and `Skills.PRAYER`.
pub const HITPOINTS: i32 = 3;
pub const PRAYER: i32 = 5;
/// The praying animation.
pub const ANIMATION: i32 = 645;
/// `Sounds.PRAYER_RECHARGE_2674`.
pub const SOUND: i32 = 2674;
pub const TRIBAL_STATUE: i32 = 3863;
pub const ELIDINIS_STATUETTE: i32 = 10439;
/// `Scenery.CHAOS_ALTAR_412`, the trapped altar.
pub const CHAOS_ALTAR_TRAP: i32 = 412;
/// Where the trap sends the player.
pub const TRAP_DESTINATION: [i32; 3] = [2583, 9576, 0];
/// `Quests.TAI_BWO_WANNAI_TRIO`'s name.
pub const TAI_BWO_WANNAI_TRIO: &str = "Tai Bwo Wannai Trio";
/// The listener's class (`scan.contentInterfaces`).
pub const LISTENER_CLASS: &str = "content.global.skill.prayer.PrayerAltarListener";
/// The `on(altars, IntType.SCENERY, "pray", "pray-at")` lambda.
pub const LAMBDA: &str = "content.global.skill.prayer.PrayerAltarListener$defineListeners$1";
/// The lambda's `queueScript` lambda.
pub const QUEUED_LAMBDA: &str =
    "content.global.skill.prayer.PrayerAltarListener$defineListeners$1$1";
/// The registration's options.
pub const OPTIONS: [&str; 2] = ["pray", "pray-at"];

/// The companion's `altars`, in order.
pub const ALTARS: [i32; 87] = [
    409, 2478, 2479, 2480, 2481, 2482, 2483, 2484, 2485, 2486, 2487, 2488, 2489, 2640, 4008, 8749,
    10639, 10640, 13179, 13180, 13181, 13182, 13183, 13184, 13185, 13186, 13187, 13188, 13189,
    13190, 13191, 13192, 13193, 13194, 13195, 13196, 13197, 13198, 13199, 15050, 15051, 18254,
    19145, 20377, 20378, 20379, 24343, 27306, 27307, 27308, 27309, 27334, 27338, 27339, 27661,
    30624, 30726, 34616, 36972, 37630, 37901, 37902, 37903, 37904, 37905, 37906, 37907, 37908,
    37909, 37910, 37911, 37912, 39547, 39842, 61, 411, 412, 32079, 37990, 4858, 4859, 410, 28698,
    3521, 3863, 10439, 37985,
];
/// The companion's `boostedAltars`: two prayer points above the level.
pub const BOOSTED_ALTARS: [i32; 2] = [2640, 3521];

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// A NullPointerException.
    Null,
    /// A failure of the binding, not of the original.
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;

/// What the listener asks of the world, for its one player.
pub trait Host {
    /// `hasRequirement(player, quest)`.
    fn has_requirement(&mut self, quest: &str) -> Result<bool>;
    /// `getStatLevel(player, skill)`.
    fn static_level(&mut self, skill: i32) -> Result<i32>;
    /// `player.skills.prayerPoints`.
    fn prayer_points(&mut self) -> Result<f64>;
    /// `sendMessage(player, message)`.
    fn message(&mut self, message: &str) -> Result<()>;
    /// `lock(player, ticks)`.
    fn lock(&mut self, ticks: i32) -> Result<()>;
    /// `animate(player, id)`.
    fn animate(&mut self, id: i32) -> Result<()>;
    /// `playAudio(player, id)`.
    fn play_audio(&mut self, id: i32) -> Result<()>;
    /// `setTempLevel(player, skill, level)`: `Skills.setLevel`.
    fn set_level(&mut self, skill: i32, level: i32) -> Result<()>;
    /// `player.dispatch(PrayerPointsRechargeEvent(node))`.
    fn dispatch_recharge(&mut self) -> Result<()>;
    /// `queueScript(player, delay, QueueStrength.STRONG) { ... }`; the
    /// script is [`trap`].
    fn queue_trap(&mut self, delay: i32) -> Result<()>;
    /// `teleport(player, location, TeleportType.INSTANT)`.
    fn teleport(&mut self, to: [i32; 3]) -> Result<()>;
    /// `stopExecuting(player)`.
    fn stop_executing(&mut self) -> Result<bool>;
}

/// The private `pray(player, node)`.
fn pray<H: Host>(host: &mut H, altar: i32) -> Result<bool> {
    let boost = if BOOSTED_ALTARS.contains(&altar) {
        2
    } else {
        0
    };
    let level = host.static_level(PRAYER)?.wrapping_add(boost);
    if host.prayer_points()? >= f64::from(level) {
        host.message("You already have full prayer points.")?;
        return Ok(false);
    }
    host.lock(3)?;
    host.animate(ANIMATION)?;
    host.play_audio(SOUND)?;
    host.set_level(PRAYER, level)?;
    host.message("You recharge your prayer points.")?;
    host.dispatch_recharge()?;
    Ok(true)
}

/// The `on(altars, IntType.SCENERY, "pray", "pray-at")` lambda for the
/// scenery id `altar`.
pub fn pray_at<H: Host>(host: &mut H, altar: i32) -> Result<bool> {
    if altar == TRIBAL_STATUE && !host.has_requirement(TAI_BWO_WANNAI_TRIO)? {
        return Ok(true);
    }
    if !pray(host, altar)? && altar != ELIDINIS_STATUETTE {
        return Ok(true);
    }
    if altar == ELIDINIS_STATUETTE {
        let level = host.static_level(HITPOINTS)?.wrapping_add(7);
        host.set_level(HITPOINTS, level)?;
        host.message("You feel much healthier after praying in the shrine.")?;
    }
    if altar == CHAOS_ALTAR_TRAP {
        host.lock(4)?;
        host.queue_trap(4)?;
    }
    Ok(true)
}

/// The chaos altar's queued script.
pub fn trap<H: Host>(host: &mut H) -> Result<bool> {
    host.message("It's a trap!")?;
    host.teleport(TRAP_DESTINATION)?;
    host.stop_executing()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct World {
        calls: Vec<String>,
        prayer_level: i32,
        points: f64,
        quest: bool,
    }
    fn world(prayer_level: i32, points: f64) -> World {
        World {
            calls: Vec::new(),
            prayer_level,
            points,
            quest: false,
        }
    }
    impl Host for World {
        fn has_requirement(&mut self, quest: &str) -> Result<bool> {
            self.calls.push(format!("requirement {quest}"));
            Ok(self.quest)
        }
        fn static_level(&mut self, skill: i32) -> Result<i32> {
            Ok(if skill == PRAYER {
                self.prayer_level
            } else {
                10
            })
        }
        fn prayer_points(&mut self) -> Result<f64> {
            Ok(self.points)
        }
        fn message(&mut self, message: &str) -> Result<()> {
            self.calls.push(format!("message {message}"));
            Ok(())
        }
        fn lock(&mut self, ticks: i32) -> Result<()> {
            self.calls.push(format!("lock {ticks}"));
            Ok(())
        }
        fn animate(&mut self, id: i32) -> Result<()> {
            self.calls.push(format!("animate {id}"));
            Ok(())
        }
        fn play_audio(&mut self, id: i32) -> Result<()> {
            self.calls.push(format!("audio {id}"));
            Ok(())
        }
        fn set_level(&mut self, skill: i32, level: i32) -> Result<()> {
            self.calls.push(format!("level {skill} {level}"));
            Ok(())
        }
        fn dispatch_recharge(&mut self) -> Result<()> {
            self.calls.push("event".into());
            Ok(())
        }
        fn queue_trap(&mut self, delay: i32) -> Result<()> {
            self.calls.push(format!("queue {delay}"));
            Ok(())
        }
        fn teleport(&mut self, to: [i32; 3]) -> Result<()> {
            self.calls.push(format!("teleport {to:?}"));
            Ok(())
        }
        fn stop_executing(&mut self) -> Result<bool> {
            self.calls.push("stopExecuting".into());
            Ok(true)
        }
    }

    const RECHARGE: [&str; 6] = [
        "lock 3",
        "animate 645",
        "audio 2674",
        "level 5 43",
        "message You recharge your prayer points.",
        "event",
    ];

    #[test]
    fn the_altars_are_distinct() {
        let mut ids = ALTARS.to_vec();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 87);
        assert!(BOOSTED_ALTARS.iter().all(|id| ALTARS.contains(id)));
    }

    #[test]
    fn an_altar_recharges() {
        let mut w = world(43, 12.5);
        assert_eq!(pray_at(&mut w, 409), Ok(true));
        assert_eq!(w.calls, RECHARGE);
    }

    #[test]
    fn full_points_only_say_so() {
        let mut w = world(43, 43.0);
        assert_eq!(pray_at(&mut w, 409), Ok(true));
        assert_eq!(w.calls, ["message You already have full prayer points."]);
        // The trapped altar springs nothing on a full player.
        let mut w = world(43, 43.0);
        assert_eq!(pray_at(&mut w, CHAOS_ALTAR_TRAP), Ok(true));
        assert_eq!(w.calls.len(), 1);
    }

    #[test]
    fn a_boosted_altar_gives_two_more() {
        let mut w = world(43, 43.0);
        assert_eq!(pray_at(&mut w, 2640), Ok(true));
        assert!(w.calls.contains(&"level 5 45".to_string()));
        let mut w = world(43, 45.0);
        assert_eq!(pray_at(&mut w, 3521), Ok(true));
        assert_eq!(w.calls, ["message You already have full prayer points."]);
    }

    #[test]
    fn the_tribal_statue_needs_its_quest() {
        let mut w = world(43, 0.0);
        assert_eq!(pray_at(&mut w, TRIBAL_STATUE), Ok(true));
        assert_eq!(w.calls, ["requirement Tai Bwo Wannai Trio"]);
        let mut w = world(43, 0.0);
        w.quest = true;
        assert_eq!(pray_at(&mut w, TRIBAL_STATUE), Ok(true));
        assert_eq!(w.calls[1..], RECHARGE);
    }

    #[test]
    fn the_statuette_heals_even_at_full_points() {
        let mut w = world(43, 43.0);
        assert_eq!(pray_at(&mut w, ELIDINIS_STATUETTE), Ok(true));
        assert_eq!(
            w.calls,
            [
                "message You already have full prayer points.",
                "level 3 17",
                "message You feel much healthier after praying in the shrine.",
            ]
        );
    }

    #[test]
    fn the_chaos_altar_is_a_trap() {
        let mut w = world(43, 1.0);
        assert_eq!(pray_at(&mut w, CHAOS_ALTAR_TRAP), Ok(true));
        assert_eq!(w.calls[..6], RECHARGE);
        assert_eq!(w.calls[6..], ["lock 4", "queue 4"]);
        let mut w = world(43, 1.0);
        assert_eq!(trap(&mut w), Ok(true));
        assert_eq!(
            w.calls,
            [
                "message It's a trap!",
                "teleport [2583, 9576, 0]",
                "stopExecuting"
            ]
        );
    }
}
