//! Firemaking: `Log`, `FireMakingPulse` (a tinderbox on logs) and
//! `BarbFirePulse` (a bow on logs), with `SkillPulse`'s `start`, `pulse`
//! and `stop` around them. Ported from the Java and Kotlin sources without
//! a JVM corpus; the unit tests below check the control flow against a
//! recording host only.
//!
//! FiremakingListener's five registrations are the binding's
//! (`world::live::items::firemaking`); each one constructs a pulse with
//! [`Pulse::tinderbox`] or [`Pulse::bow`] and hands it to the player's pulse
//! manager.

/// `Items.TINDERBOX_590`.
pub const TINDERBOX: i32 = 590;
/// `Items.ASHES_592`.
pub const ASHES: i32 = 592;
/// `Items.OAK_LOGS_1521`.
pub const OAK_LOGS: i32 = 1521;
/// `Items.CURSED_MAGIC_LOGS_13567`.
pub const CURSED_MAGIC_LOGS: i32 = 13567;
/// `Items.JOGRE_BONES_3125`.
pub const JOGRE_BONES: i32 = 3125;
/// `Skills.FIREMAKING`.
pub const FIREMAKING: i32 = 11;
/// `FireMakingPulse.ANIMATION`.
pub const ANIMATION: i32 = 733;
/// `BarbarianTraining.attributeBowFire`.
pub const ATTRIBUTE_BOW_FIRE: &str = "/save:barbtraining:bowfire";
const ATTRIBUTE_REMOVE_LOG: &str = "remove-log";
const ATTRIBUTE_LAST_FIREMAKE: &str = "last-firemake";

/// The listener class in the start-up scan.
pub const LISTENER_CLASS: &str = "content.global.skill.firemaking.FiremakingListener";

/// FiremakingListener's `logs`, in source order.
pub const LOGS: [i32; 31] = [
    1511, 1521, 1513, 1515, 1517, 1519, 2862, 3438, 3440, 3442, 3444, 3446, 3448, 6211, 6213, 6332,
    6333, 7404, 7405, 7406, 8934, 9067, 10328, 10329, 10808, 10810, 10812, 11035, 12581, 12583,
    3125,
];
/// FiremakingListener's `bows`, in source order.
pub const BOWS: [i32; 12] = [841, 839, 843, 845, 849, 847, 853, 851, 857, 855, 861, 859];
/// BarbFirePulse's `bowsAndAnimations`, in the map's insertion order.
pub const BOWS_AND_ANIMATIONS: [(i32, i32); 12] = [
    (841, 6714),
    (839, 6714),
    (843, 6715),
    (845, 6715),
    (849, 6716),
    (847, 6716),
    (853, 6717),
    (851, 6717),
    (857, 6718),
    (855, 6718),
    (861, 6719),
    (859, 6719),
];

/// A `Log` constant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Log {
    pub name: &'static str,
    pub log_id: i32,
    pub level: i32,
    pub life: i32,
    pub fire_id: i32,
    pub xp: f64,
    pub barb_level: i32,
}

const fn log(
    name: &'static str,
    log_id: i32,
    level: i32,
    life: i32,
    fire_id: i32,
    xp: f64,
    barb_level: i32,
) -> Log {
    Log {
        name,
        log_id,
        level,
        life,
        fire_id,
        xp,
        barb_level,
    }
}

/// `Log.values()`.
pub const LOG_VALUES: [Log; 18] = [
    log("NORMAL", 1511, 1, 180, 2732, 40.0, 21),
    log("ACHEY", 2862, 1, 180, 2732, 40.0, 21),
    log("OAK", 1521, 15, 200, 2732, 60.0, 35),
    log("WILLOW", 1519, 30, 250, 2732, 90.0, 50),
    log("TEAK", 6333, 35, 300, 2732, 105.0, 55),
    log("ARCTIC_PINE", 10810, 42, 500, 2732, 125.0, 62),
    log("MAPLE", 1517, 45, 300, 2732, 135.0, 65),
    log("MAHOGANY", 6332, 50, 300, 2732, 157.5, 70),
    log("EUCALYPTUS", 12581, 58, 300, 2732, 193.5, 78),
    log("YEW", 1515, 60, 400, 2732, 202.5, 80),
    log("MAGIC", 1513, 75, 450, 2732, 303.8, 95),
    log("CURSED_MAGIC", 13567, 82, 650, 2732, 303.8, -1),
    log("PURPLE", 10329, 1, 200, 20001, 50.0, 21),
    log("WHITE", 10328, 1, 200, 20000, 50.0, 21),
    log("BLUE", 7406, 1, 200, 11406, 50.0, 21),
    log("GREEN", 7405, 1, 200, 11405, 50.0, 21),
    log("RED", 7404, 1, 200, 11404, 50.0, 21),
    log("JOGRE", 3125, 1, 200, 3862, 50.0, -1),
];

/// `Log.forId(id)`: the first constant with that log (`putIfAbsent`).
pub fn log_for_id(id: i32) -> Option<&'static Log> {
    LOG_VALUES.iter().find(|log| log.log_id == id)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// NullPointerException.
    Null,
    /// ClassCastException.
    Cast,
    /// A host service failed or is not joined.
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;

/// The world calls the two pulses make, for one player.
pub trait Host {
    /// A `GroundItem`.
    type Ground: Clone;
    /// A `Scenery`.
    type Scenery;

    /// `RegionManager.getObject(player.getLocation()) != null`.
    fn object_at_player(&mut self) -> Result<bool>;
    /// `player.getZoneMonitor().isInZone(name)`.
    fn in_zone(&mut self, name: &str) -> Result<bool>;
    /// `player.getPacketDispatch().sendMessage(message)`.
    fn message(&mut self, message: &str) -> Result<()>;
    /// ContentAPI `sendDialogue(player, message)`.
    fn dialogue(&mut self, message: &str) -> Result<()>;
    /// `inInventory(player, id, 1)` (and `containsItem(new Item(id))`).
    fn in_inventory(&mut self, id: i32) -> Result<bool>;
    /// `player.getSkills().getLevel(Skills.FIREMAKING)`.
    fn level(&mut self) -> Result<i32>;
    /// `player.getAttribute(key, default)` read as a Boolean.
    fn attribute_bool(&mut self, key: &str, default: bool) -> Result<bool>;
    /// `player.getAttribute(key, default)` read as an Int.
    fn attribute_int(&mut self, key: &str, default: i32) -> Result<i32>;
    fn set_attribute_bool(&mut self, key: &str, value: bool) -> Result<()>;
    fn set_attribute_int(&mut self, key: &str, value: i32) -> Result<()>;
    /// `player.removeAttribute(key)`.
    fn remove_attribute(&mut self, key: &str) -> Result<()>;
    /// `replaceSlot(player, node.slot, Item(node.id, amount), node,
    /// Container.INVENTORY)`.
    fn replace_slot(&mut self, node: Node, amount: i32) -> Result<()>;
    /// `new GroundItem(node, player.getLocation(), player)`: not yet in the
    /// world.
    fn new_ground(&mut self, node: Node) -> Result<Self::Ground>;
    /// `GroundItemManager.create(groundItem)`.
    fn create_ground(&mut self, ground: &Self::Ground) -> Result<()>;
    /// `produceGroundItem(player, groundItem.id, groundItem.amount,
    /// groundItem.location)`.
    fn produce_ground(&mut self, like: &Self::Ground) -> Result<Self::Ground>;
    /// `groundItem.isActive()`.
    fn ground_active(&mut self, ground: &Self::Ground) -> Result<bool>;
    /// `GroundItemManager.destroy(groundItem)`.
    fn destroy_ground(&mut self, ground: &Self::Ground) -> Result<()>;
    /// `new Scenery(fireId, player.getLocation())`, the ash
    /// (`getAsh`: a GroundItem of ashes at the object's location for the
    /// player, with `setDecayTime(life + 200)`) and
    /// `SceneryBuilder.add(object, life, ash)`.
    fn add_fire(&mut self, fire_id: i32, life: i32) -> Result<Self::Scenery>;
    /// `player.moveStep()`.
    fn move_step(&mut self) -> Result<()>;
    /// `player.faceLocation(object.getFaceLocation(player.getLocation()))`.
    fn face_fire(&mut self, fire: &Self::Scenery) -> Result<()>;
    /// `player.getSkills().addExperience(Skills.FIREMAKING, xp)`.
    fn add_experience(&mut self, xp: f64) -> Result<()>;
    /// `GameWorld.getTicks()`.
    fn world_ticks(&mut self) -> Result<i32>;
    /// `player.dispatch(new LitFireEvent(logId))`.
    fn dispatch_lit_fire(&mut self, log_id: i32) -> Result<()>;
    /// `player.animate(new Animation(id))`.
    fn animate(&mut self, id: i32) -> Result<()>;
    /// `player.animate(new Animation(-1, Priority.HIGH))`.
    fn reset_animation(&mut self) -> Result<()>;
    /// `RandomFunction.random(max)`.
    fn random(&mut self, max: i32) -> Result<i32>;
}

/// The pulse's `node`: the log as an Item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node {
    pub id: i32,
    pub amount: i32,
    pub slot: i32,
}

/// Which of the two pulses this is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `FireMakingPulse`.
    Tinderbox,
    /// `BarbFirePulse` with its `usedBow`.
    Bow { used: Option<i32> },
}

/// A FireMakingPulse or a BarbFirePulse.
pub struct Pulse<G> {
    pub kind: Kind,
    pub node: Node,
    pub fire: Option<&'static Log>,
    pub ground: G,
    pub ticks: i32,
    /// `Pulse.running`.
    pub running: bool,
}

impl<G: Clone> Pulse<G> {
    /// `new FireMakingPulse(player, node, groundItem)`.
    pub fn tinderbox<H: Host<Ground = G>>(
        host: &mut H,
        node: Node,
        ground: Option<G>,
    ) -> Result<Self> {
        Self::new(host, Kind::Tinderbox, node, ground)
    }
    /// `BarbFirePulse(player, node, groundItem, usedBow)`.
    pub fn bow<H: Host<Ground = G>>(
        host: &mut H,
        node: Node,
        ground: Option<G>,
        used: Option<i32>,
    ) -> Result<Self> {
        Self::new(host, Kind::Bow { used }, node, ground)
    }
    fn new<H: Host<Ground = G>>(
        host: &mut H,
        kind: Kind,
        node: Node,
        ground: Option<G>,
    ) -> Result<Self> {
        // SkillPulse's constructor: `super.stop()`.
        let fire = log_for_id(node.id);
        let ground = match ground {
            None => {
                // The two sources order these differently; neither call
                // reads what the other writes.
                let ground = host.new_ground(node)?;
                host.set_attribute_bool(ATTRIBUTE_REMOVE_LOG, true)?;
                ground
            }
            Some(ground) => {
                host.remove_attribute(ATTRIBUTE_REMOVE_LOG)?;
                ground
            }
        };
        Ok(Self {
            kind,
            node,
            fire,
            ground,
            ticks: 0,
            running: false,
        })
    }

    /// `SkillPulse.start()`.
    pub fn start<H: Host<Ground = G>>(&mut self, host: &mut H) -> Result<()> {
        if self.check_requirements(host)? {
            self.running = true;
            self.message(host, 0)?;
        }
        Ok(())
    }

    /// `SkillPulse.pulse()`: whether the pulse is done.
    pub fn pulse<H: Host<Ground = G>>(&mut self, host: &mut H) -> Result<bool> {
        if !self.check_requirements(host)? {
            return Ok(true);
        }
        // animate(): empty in both pulses.
        self.reward(host)
    }

    /// `SkillPulse.stop()`.
    pub fn stop<H: Host<Ground = G>>(&mut self, host: &mut H) -> Result<()> {
        host.reset_animation()?;
        self.running = false;
        self.message(host, 1)
    }

    /// `checkRequirements()`.
    pub fn check_requirements<H: Host<Ground = G>>(&mut self, host: &mut H) -> Result<bool> {
        let Some(fire) = self.fire else {
            return Ok(false);
        };
        let bow = matches!(self.kind, Kind::Bow { .. });
        if bow && (fire.log_id == CURSED_MAGIC_LOGS || fire.log_id == JOGRE_BONES) {
            return Ok(false);
        }
        if host.object_at_player()? || host.in_zone("bank")? {
            host.message("You can't light a fire here.")?;
            return Ok(false);
        }
        let tool = if bow {
            let mut any = false;
            for (id, _) in BOWS_AND_ANIMATIONS {
                if host.in_inventory(id)? {
                    any = true;
                    break;
                }
            }
            any
        } else {
            host.in_inventory(TINDERBOX)?
        };
        if !tool {
            host.message("You do not have the required items to light this.")?;
            return Ok(false);
        }
        if bow {
            if host.attribute_int(ATTRIBUTE_BOW_FIRE, 0)? == 0 {
                host.dialogue(
                    "You must begin the relevant section of Otto Godblessed's barbarian training.",
                )?;
                return Ok(false);
            }
            if host.level()? < fire.barb_level {
                host.message(&format!(
                    "You need a firemaking level of {} to light this log using barbarian firemaking.",
                    fire.barb_level
                ))?;
                return Ok(false);
            }
        } else if host.level()? < fire.level {
            host.message(&format!(
                "You need a firemaking level of {} to light this log.",
                fire.level
            ))?;
            return Ok(false);
        }
        if host.attribute_bool(ATTRIBUTE_REMOVE_LOG, false)? {
            host.remove_attribute(ATTRIBUTE_REMOVE_LOG)?;
            if host.in_inventory(self.node.id)? {
                host.replace_slot(self.node, self.node.amount.wrapping_sub(1))?;
                if bow {
                    self.ground = host.produce_ground(&self.ground)?;
                } else {
                    host.create_ground(&self.ground)?;
                }
            }
        }
        Ok(true)
    }

    /// `getBowAnimation()`.
    fn bow_animation<H: Host<Ground = G>>(&self, host: &mut H) -> Result<i32> {
        if let Kind::Bow { used: Some(used) } = self.kind
            && let Some((_, animation)) = BOWS_AND_ANIMATIONS.iter().find(|(bow, _)| *bow == used)
        {
            return Ok(*animation);
        }
        for (bow, animation) in BOWS_AND_ANIMATIONS {
            if host.in_inventory(bow)? {
                return Ok(animation);
            }
        }
        Ok(BOWS_AND_ANIMATIONS[0].1)
    }

    /// `reward()`.
    pub fn reward<H: Host<Ground = G>>(&mut self, host: &mut H) -> Result<bool> {
        let fire = self.fire.ok_or(Error::Null)?;
        if host.attribute_int(ATTRIBUTE_LAST_FIREMAKE, 0)? >= host.world_ticks()? {
            self.create_fire(host, fire)?;
            return Ok(true);
        }
        match self.kind {
            Kind::Tinderbox => {
                if self.ticks == 0 {
                    host.animate(ANIMATION)?;
                }
                self.ticks = self.ticks.wrapping_add(1);
                if self.ticks % 3 != 0 {
                    return Ok(false);
                }
                if self.ticks % 12 == 0 {
                    host.animate(ANIMATION)?;
                }
                if !self.success(host, fire.level)? {
                    return Ok(false);
                }
                self.create_fire(host, fire)?;
            }
            Kind::Bow { .. } => {
                if self.ticks % 12 == 0 {
                    let animation = self.bow_animation(host)?;
                    host.animate(animation)?;
                }
                self.ticks = self.ticks.wrapping_add(1);
                if self.ticks % 3 != 0 {
                    return Ok(false);
                }
                if !self.success(host, fire.barb_level)? {
                    return Ok(false);
                }
                self.create_fire(host, fire)?;
                // Progress Barbarian training if this is your first oak log
                // lit with a bow.
                if host.attribute_int(ATTRIBUTE_BOW_FIRE, 0)? == 1 && fire.log_id == OAK_LOGS {
                    host.set_attribute_int(ATTRIBUTE_BOW_FIRE, 2)?;
                    host.dialogue(
                        "You feel you have learned more of barbarian ways. Otto might wish to talk to you more.",
                    )?;
                }
            }
        }
        Ok(true)
    }

    /// `createFire()`.
    fn create_fire<H: Host<Ground = G>>(&mut self, host: &mut H, fire: &Log) -> Result<()> {
        if !host.ground_active(&self.ground)? {
            return Ok(());
        }
        let object = host.add_fire(fire.fire_id, fire.life)?;
        host.destroy_ground(&self.ground)?;
        host.move_step()?;
        if self.kind == Kind::Tinderbox {
            host.face_fire(&object)?;
        }
        // BarbFirePulse's `face(player, fireObject)` does nothing: a Scenery
        // is neither of the function's two cases.
        host.add_experience(fire.xp)?;
        let ticks = host.world_ticks()?;
        host.set_attribute_int(ATTRIBUTE_LAST_FIREMAKE, ticks.wrapping_add(2))?;
        host.dispatch_lit_fire(fire.log_id)
    }

    /// `message(type)`.
    fn message<H: Host<Ground = G>>(&self, host: &mut H, kind: i32) -> Result<()> {
        let name = if self.node.id == JOGRE_BONES {
            "bones"
        } else {
            "logs"
        };
        match kind {
            0 => host.message(&format!("You attempt to light the {name}..")),
            1 => host.message(&format!("The fire catches and the {name} begin to burn.")),
            _ => Ok(()),
        }
    }

    /// `success()`: `req` is the log's level, or its barbarian level.
    fn success<H: Host<Ground = G>>(&self, host: &mut H, req: i32) -> Result<bool> {
        let level = 1i32.wrapping_add(host.level()?);
        let req = f64::from(req);
        // `(level * 50 - req * 15) / req / 3 * 4`: the Int product widens
        // for the subtraction.
        let chance = ((f64::from(level.wrapping_mul(50)) - req * 15.0) / req / 3.0 * 4.0).ceil();
        let roll = host.random(99)?;
        Ok(chance >= f64::from(roll))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct World {
        calls: Vec<String>,
        inventory: Vec<i32>,
        level: i32,
        bools: BTreeMap<String, bool>,
        ints: BTreeMap<String, i32>,
        object: bool,
        bank: bool,
        ticks: i32,
        rolls: Vec<i32>,
        grounds: i32,
        active: bool,
    }
    impl World {
        fn messages(&self) -> Vec<&str> {
            self.calls
                .iter()
                .filter_map(|c| c.strip_prefix("message "))
                .collect()
        }
    }
    impl Host for World {
        type Ground = i32;
        type Scenery = i32;
        fn object_at_player(&mut self) -> Result<bool> {
            Ok(self.object)
        }
        fn in_zone(&mut self, name: &str) -> Result<bool> {
            Ok(name == "bank" && self.bank)
        }
        fn message(&mut self, message: &str) -> Result<()> {
            self.calls.push(format!("message {message}"));
            Ok(())
        }
        fn dialogue(&mut self, message: &str) -> Result<()> {
            self.calls.push(format!("dialogue {message}"));
            Ok(())
        }
        fn in_inventory(&mut self, id: i32) -> Result<bool> {
            Ok(self.inventory.contains(&id))
        }
        fn level(&mut self) -> Result<i32> {
            Ok(self.level)
        }
        fn attribute_bool(&mut self, key: &str, default: bool) -> Result<bool> {
            Ok(self.bools.get(key).copied().unwrap_or(default))
        }
        fn attribute_int(&mut self, key: &str, default: i32) -> Result<i32> {
            Ok(self.ints.get(key).copied().unwrap_or(default))
        }
        fn set_attribute_bool(&mut self, key: &str, value: bool) -> Result<()> {
            self.bools.insert(key.into(), value);
            Ok(())
        }
        fn set_attribute_int(&mut self, key: &str, value: i32) -> Result<()> {
            self.ints.insert(key.into(), value);
            Ok(())
        }
        fn remove_attribute(&mut self, key: &str) -> Result<()> {
            self.bools.remove(key);
            Ok(())
        }
        fn replace_slot(&mut self, node: Node, amount: i32) -> Result<()> {
            self.calls
                .push(format!("replaceSlot {} {} x{amount}", node.slot, node.id));
            if amount <= 0 {
                self.inventory.retain(|id| *id != node.id);
            }
            Ok(())
        }
        fn new_ground(&mut self, _: Node) -> Result<i32> {
            self.grounds += 1;
            Ok(self.grounds)
        }
        fn create_ground(&mut self, ground: &i32) -> Result<()> {
            self.calls.push(format!("create ground {ground}"));
            self.active = true;
            Ok(())
        }
        fn produce_ground(&mut self, like: &i32) -> Result<i32> {
            self.grounds += 1;
            self.calls
                .push(format!("produce ground {} like {like}", self.grounds));
            self.active = true;
            Ok(self.grounds)
        }
        fn ground_active(&mut self, _: &i32) -> Result<bool> {
            Ok(self.active)
        }
        fn destroy_ground(&mut self, ground: &i32) -> Result<()> {
            self.calls.push(format!("destroy ground {ground}"));
            self.active = false;
            Ok(())
        }
        fn add_fire(&mut self, fire_id: i32, life: i32) -> Result<i32> {
            self.calls.push(format!("fire {fire_id} life {life}"));
            Ok(fire_id)
        }
        fn move_step(&mut self) -> Result<()> {
            self.calls.push("moveStep".into());
            Ok(())
        }
        fn face_fire(&mut self, _: &i32) -> Result<()> {
            self.calls.push("face".into());
            Ok(())
        }
        fn add_experience(&mut self, xp: f64) -> Result<()> {
            self.calls.push(format!("xp {xp}"));
            Ok(())
        }
        fn world_ticks(&mut self) -> Result<i32> {
            Ok(self.ticks)
        }
        fn dispatch_lit_fire(&mut self, log_id: i32) -> Result<()> {
            self.calls.push(format!("lit {log_id}"));
            Ok(())
        }
        fn animate(&mut self, id: i32) -> Result<()> {
            self.calls.push(format!("animate {id}"));
            Ok(())
        }
        fn reset_animation(&mut self) -> Result<()> {
            self.calls.push("animate -1".into());
            Ok(())
        }
        fn random(&mut self, max: i32) -> Result<i32> {
            assert_eq!(max, 99);
            Ok(self.rolls.remove(0))
        }
    }

    const LOG: Node = Node {
        id: 1511,
        amount: 1,
        slot: 3,
    };

    fn world() -> World {
        World {
            inventory: vec![TINDERBOX, 1511],
            level: 1,
            ticks: 100,
            ..World::default()
        }
    }

    #[test]
    fn log_table_matches_the_source() {
        assert_eq!(LOG_VALUES.len(), 18);
        let magic = log_for_id(1513).unwrap();
        assert_eq!((magic.level, magic.life, magic.barb_level), (75, 450, 95));
        assert_eq!(magic.xp, 303.8);
        assert_eq!(log_for_id(3125).unwrap().fire_id, 3862);
        // Pyre logs are in the listener's list but are no Log constant.
        assert!(LOGS.contains(&3438) && log_for_id(3438).is_none());
        assert_eq!(BOWS.len(), BOWS_AND_ANIMATIONS.len());
    }

    #[test]
    fn tinderbox_lights_an_inventory_log_on_the_third_tick() {
        let mut w = world();
        // Level 1 against level 1: ceil((100 - 15) / 1 / 3 * 4) = 114, so
        // any roll succeeds.
        w.rolls = vec![98];
        let mut pulse = Pulse::tinderbox(&mut w, LOG, None).unwrap();
        assert_eq!(w.bools.get("remove-log"), Some(&true));
        pulse.start(&mut w).unwrap();
        assert!(pulse.running);
        assert_eq!(
            w.calls,
            [
                "replaceSlot 3 1511 x0",
                "create ground 1",
                "message You attempt to light the logs.."
            ]
        );
        assert!(!w.bools.contains_key("remove-log"));
        w.calls.clear();
        assert!(!pulse.pulse(&mut w).unwrap());
        assert!(!pulse.pulse(&mut w).unwrap());
        assert_eq!(w.calls, ["animate 733"]);
        assert!(pulse.pulse(&mut w).unwrap());
        pulse.stop(&mut w).unwrap();
        assert_eq!(
            w.calls,
            [
                "animate 733",
                "fire 2732 life 180",
                "destroy ground 1",
                "moveStep",
                "face",
                "xp 40",
                "lit 1511",
                "animate -1",
                "message The fire catches and the logs begin to burn."
            ]
        );
        assert_eq!(w.ints.get("last-firemake"), Some(&102));
    }

    #[test]
    fn a_recent_fire_lights_the_next_log_at_once() {
        let mut w = world();
        w.ints.insert("last-firemake".into(), 100);
        let mut pulse = Pulse::tinderbox(&mut w, LOG, None).unwrap();
        pulse.start(&mut w).unwrap();
        assert!(pulse.pulse(&mut w).unwrap());
        assert!(w.calls.contains(&"fire 2732 life 180".to_string()));
        assert!(!w.calls.contains(&"animate 733".to_string()));
    }

    #[test]
    fn a_failed_roll_keeps_the_pulse_going() {
        let mut w = world();
        // Yew at level 60: ceil((61 * 50 - 900) / 60 / 3 * 4) = 48.
        w.level = 60;
        w.inventory = vec![TINDERBOX, 1515];
        w.rolls = vec![49, 48];
        let yew = Node {
            id: 1515,
            amount: 1,
            slot: 0,
        };
        let mut pulse = Pulse::tinderbox(&mut w, yew, None).unwrap();
        pulse.start(&mut w).unwrap();
        let done: Vec<bool> = (0..6).map(|_| pulse.pulse(&mut w).unwrap()).collect();
        assert_eq!(done, [false, false, false, false, false, true]);
    }

    #[test]
    fn requirements_refuse_with_the_source_messages() {
        let mut w = world();
        w.object = true;
        let mut pulse = Pulse::tinderbox(&mut w, LOG, None).unwrap();
        pulse.start(&mut w).unwrap();
        assert!(!pulse.running);
        assert_eq!(w.messages(), ["You can't light a fire here."]);

        let mut w = world();
        w.inventory = vec![1511];
        let mut pulse = Pulse::tinderbox(&mut w, LOG, Some(7)).unwrap();
        pulse.start(&mut w).unwrap();
        assert_eq!(
            w.messages(),
            ["You do not have the required items to light this."]
        );

        let mut w = world();
        w.inventory = vec![TINDERBOX, 1513];
        let magic = Node {
            id: 1513,
            amount: 1,
            slot: 0,
        };
        let mut pulse = Pulse::tinderbox(&mut w, magic, None).unwrap();
        pulse.start(&mut w).unwrap();
        assert_eq!(
            w.messages(),
            ["You need a firemaking level of 75 to light this log."]
        );
        // The log stays in the inventory and the attribute stays set.
        assert_eq!(w.bools.get("remove-log"), Some(&true));

        // A pyre log is no Log: nothing is said.
        let mut w = world();
        let pyre = Node {
            id: 3438,
            amount: 1,
            slot: 0,
        };
        let mut pulse = Pulse::tinderbox(&mut w, pyre, None).unwrap();
        pulse.start(&mut w).unwrap();
        assert!(!pulse.running && w.messages().is_empty());
    }

    #[test]
    fn a_ground_log_is_lit_where_it_lies() {
        let mut w = world();
        w.inventory = vec![TINDERBOX];
        w.bools.insert("remove-log".into(), true);
        w.active = true;
        w.rolls = vec![0];
        let mut pulse = Pulse::tinderbox(&mut w, LOG, Some(9)).unwrap();
        assert!(!w.bools.contains_key("remove-log"));
        pulse.start(&mut w).unwrap();
        for _ in 0..3 {
            pulse.pulse(&mut w).unwrap();
        }
        assert!(w.calls.contains(&"destroy ground 9".to_string()));
        assert!(!w.calls.iter().any(|c| c.starts_with("replaceSlot")));
    }

    #[test]
    fn a_bow_needs_ottos_training_and_the_barbarian_level() {
        let oak = Node {
            id: OAK_LOGS,
            amount: 1,
            slot: 1,
        };
        let mut w = world();
        w.inventory = vec![843, OAK_LOGS];
        w.level = 35;
        let mut pulse = Pulse::bow(&mut w, oak, None, Some(843)).unwrap();
        pulse.start(&mut w).unwrap();
        assert!(!pulse.running);
        assert_eq!(
            w.calls,
            [
                "dialogue You must begin the relevant section of Otto Godblessed's barbarian training."
            ]
        );

        let mut w = world();
        w.inventory = vec![843, OAK_LOGS];
        w.level = 34;
        w.ints.insert(ATTRIBUTE_BOW_FIRE.into(), 1);
        let mut pulse = Pulse::bow(&mut w, oak, None, Some(843)).unwrap();
        pulse.start(&mut w).unwrap();
        assert_eq!(
            w.messages(),
            ["You need a firemaking level of 35 to light this log using barbarian firemaking."]
        );

        let mut w = world();
        w.inventory = vec![843, OAK_LOGS];
        w.level = 35;
        w.ints.insert(ATTRIBUTE_BOW_FIRE.into(), 1);
        w.rolls = vec![0];
        let mut pulse = Pulse::bow(&mut w, oak, None, Some(843)).unwrap();
        pulse.start(&mut w).unwrap();
        assert!(pulse.running);
        assert!(w.calls.contains(&"produce ground 2 like 1".to_string()));
        w.calls.clear();
        let done: Vec<bool> = (0..3).map(|_| pulse.pulse(&mut w).unwrap()).collect();
        assert_eq!(done, [false, false, true]);
        assert_eq!(
            w.calls,
            [
                "animate 6715",
                "fire 2732 life 200",
                "destroy ground 2",
                "moveStep",
                "xp 60",
                "lit 1521",
                "dialogue You feel you have learned more of barbarian ways. Otto might wish to talk to you more."
            ]
        );
        assert_eq!(w.ints.get(ATTRIBUTE_BOW_FIRE), Some(&2));
    }

    #[test]
    fn a_bow_refuses_cursed_logs_and_bones_silently() {
        let mut w = world();
        w.inventory = vec![841, JOGRE_BONES];
        w.ints.insert(ATTRIBUTE_BOW_FIRE.into(), 3);
        let bones = Node {
            id: JOGRE_BONES,
            amount: 1,
            slot: 0,
        };
        let mut pulse = Pulse::bow(&mut w, bones, None, None).unwrap();
        pulse.start(&mut w).unwrap();
        assert!(!pulse.running && w.calls.is_empty());
    }
}
