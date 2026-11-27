//! Smelting at furnaces: `FurnaceOptionPlugin` with its
//! `SmeltUseWithHandler`, `SmeltingInterface` (the chatbox 311 and its
//! `BarButton`s), `SmeltingPulse` over `SkillPulse`, and the `Bar` table,
//! from the pinned `content/global/skill/smithing` and
//! `content/global/handlers/iface`.
//!
//! Every body is a function over [`Host`]; each call the original makes
//! outside the class is a required method with no default. Ported from the
//! pinned sources **without a JVM differential corpus**:
//! `tests/smelting.rs` checks the bodies against a recording host written
//! from the same sources, and the tables against the source text. See
//! docs/smelting.md for what is kept (the level message's "smelt bronze ."
//! spacing, the chatbox closed on every requirement check, the ring of
//! forging stopping the pulse from inside `success`) and what is not ported
//! (anvils, the superheat spell's caller, the bots).
pub use crate::world::global_items2::{Error, Result};

/// `Skills.SMITHING`.
pub const SMITHING: i32 = 13;
/// The smelting chatbox.
pub const INTERFACE: i32 = 311;
/// `Sounds.FURNACE_2725`.
pub const FURNACE_SOUND: i32 = 2725;
/// The furnace animation (`Animation.create(3243)`, MID priority).
pub const ANIMATION: i32 = 3243;
/// Superheat's animation and graphics (`new Graphics(148, 96)`).
pub const SUPERHEAT_ANIMATION: i32 = 725;
pub const SUPERHEAT_GRAPHICS: (i32, i32) = (148, 96);
pub const RING_OF_FORGING: i32 = 2568;
pub const GOLDSMITH_GAUNTLETS: i32 = 776;
pub const GOLD_BAR: i32 = 2357;
/// The Edgeville furnace 26814 asks for the Varrock diary's easy tasks.
pub const EDGEVILLE_FURNACE: i32 = 26814;
/// `Location.create(3107, 3500, 0)`: the Varrock armour's furnace.
pub const EDGEVILLE: [i32; 3] = [3107, 3500, 0];
/// `Location.create(3226, 3254, 0)`: the Lumbridge furnace of the diary.
pub const LUMBRIDGE: [i32; 3] = [3226, 3254, 0];
/// `MapDistance.RENDERING.distance`.
pub const RENDERING: f64 = 15.0;
pub const KNIGHTS_SWORD: &str = "The Knight's Sword";
pub const PLUGIN_CLASS: &str = "content.global.skill.smithing.FurnaceOptionPlugin";
pub const INTERFACE_CLASS: &str = "content.global.handlers.iface.SmeltingInterface";
pub const PULSE_CLASS: &str = "content.global.skill.smithing.smelting.SmeltingPulse";
/// `UseWithHandler.OBJECT_TYPE`.
pub const OBJECT_TYPE: i32 = 2;

/// One `Bar` constant: its name, level, experience, product and ores
/// (`new Item(id)` is an amount of one).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bar {
    pub name: &'static str,
    pub level: i32,
    pub experience: f64,
    pub product: i32,
    pub ores: &'static [(i32, i32)],
}

/// `Bar.values()`, in declaration order (the ordinal is the index).
pub const BARS: [Bar; 9] = [
    Bar {
        name: "BRONZE",
        level: 1,
        experience: 6.2,
        product: 2349,
        ores: &[(436, 1), (438, 1)],
    },
    Bar {
        name: "BLURITE",
        level: 8,
        experience: 8.0,
        product: 9467,
        ores: &[(668, 1)],
    },
    Bar {
        name: "IRON",
        level: 15,
        experience: 12.5,
        product: 2351,
        ores: &[(440, 1)],
    },
    Bar {
        name: "SILVER",
        level: 20,
        experience: 13.7,
        product: 2355,
        ores: &[(442, 1)],
    },
    Bar {
        name: "STEEL",
        level: 30,
        experience: 17.5,
        product: 2353,
        ores: &[(453, 2), (440, 1)],
    },
    Bar {
        name: "GOLD",
        level: 40,
        experience: 22.5,
        product: 2357,
        ores: &[(444, 1)],
    },
    Bar {
        name: "MITHRIL",
        level: 50,
        experience: 30.0,
        product: 2359,
        ores: &[(447, 1), (453, 4)],
    },
    Bar {
        name: "ADAMANT",
        level: 70,
        experience: 37.5,
        product: 2361,
        ores: &[(449, 1), (453, 6)],
    },
    Bar {
        name: "RUNITE",
        level: 85,
        experience: 50.0,
        product: 2363,
        ores: &[(451, 1), (453, 8)],
    },
];
pub const BRONZE: usize = 0;
pub const BLURITE: usize = 1;
pub const IRON: usize = 2;
pub const SILVER: usize = 3;
pub const STEEL: usize = 4;

/// `Bar.forId(id)`: the ordinal of the bar with this product.
pub fn for_id(id: i32) -> Option<usize> {
    BARS.iter().position(|bar| bar.product == id)
}

/// `Bar.forOre(id)`: the ordinal of the first bar with this ore.
pub fn for_ore(id: i32) -> Option<usize> {
    BARS.iter()
        .position(|bar| bar.ores.iter().any(|(ore, _)| *ore == id))
}

/// `SmeltUseWithHandler.furnaceIDS`.
pub const FURNACES: [i32; 13] = [
    4304, 6189, 11010, 11666, 12100, 12809, 14921, 18497, 26814, 30021, 30510, 36956, 37651,
];
/// The two sceneries whose "use" option is the plugin's.
pub const USE_FURNACES: [i32; 2] = [3044, 21303];
/// The option names `SceneryDefinition.setOptionHandler` gets.
pub const OPTIONS: [&str; 2] = ["smelt", "smelt-ore"];

/// `SmeltUseWithHandler.getIds()`: every bar's ores in order, repeats kept
/// (the handler's `allowedNodes`).
pub fn ore_ids() -> Vec<i32> {
    BARS.iter()
        .flat_map(|bar| bar.ores.iter().map(|(id, _)| *id))
        .collect()
}

/// `BarButton.forId(button)`: the bar's ordinal and the amount (-1 for X).
/// Each bar has four buttons from 13: X, 10, 5 and 1.
pub fn bar_button(button: i32) -> Option<(usize, i32)> {
    if !(13..=48).contains(&button) {
        return None;
    }
    let offset = button - 13;
    let amount = [-1, 10, 5, 1][(offset % 4) as usize];
    Some(((offset / 4) as usize, amount))
}

/// `StringUtils.formatDisplayName(name)`: underscores to spaces, lower
/// case, then the first letter and every letter after a space in upper
/// case. The names here are ASCII.
pub fn format_display_name(name: &str) -> String {
    let name = name.replace('_', " ").to_lowercase();
    let mut formatted = String::with_capacity(name.len());
    let mut was_space = true;
    for c in name.chars() {
        if was_space {
            formatted.extend(c.to_uppercase());
            was_space = false;
        } else {
            formatted.push(c);
        }
        if c == ' ' {
            was_space = true;
        }
    }
    formatted
}

/// `Location.withinDistance(other)` with the default distance.
pub fn within_distance(at: [i32; 3], other: [i32; 3]) -> bool {
    if at[2] != other[2] {
        return false;
    }
    let (dx, dy) = (at[0] - other[0], at[1] - other[1]);
    f64::from(dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy))).sqrt() <= RENDERING
}

/// The diaries these classes read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Diary {
    Varrock,
    Lumbridge,
}

/// What the amount prompt answered.
#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    Int(i32),
    Text(String),
}

/// Everything these classes reach outside themselves. Each method is one
/// original call; none has a default.
pub trait Host {
    type Node: Clone;

    /// `node.getId()` of a scenery.
    fn id(&mut self, node: &Self::Node) -> Result<i32>;
    /// `ItemDefinition.forId(id).getName()` (`Item.getName()`).
    fn item_name(&mut self, id: i32) -> Result<String>;
    /// `String.toLowerCase()`.
    fn lowercase(&self, text: &str) -> String;
    /// `interfaceManager.closeChatbox()`.
    fn close_chatbox(&mut self, player: &Self::Node) -> Result<()>;
    /// `interfaceManager.openChatbox(id)`.
    fn open_chatbox(&mut self, player: &Self::Node, interface: i32) -> Result<()>;
    /// `packetDispatch.sendItemZoomOnInterface(item, zoom, interface, child)`.
    fn send_item_zoom(
        &mut self,
        player: &Self::Node,
        item: i32,
        zoom: i32,
        interface: i32,
        child: i32,
    ) -> Result<()>;
    /// `packetDispatch.sendString(text, interface, child)`.
    fn send_string(
        &mut self,
        player: &Self::Node,
        text: &str,
        interface: i32,
        child: i32,
    ) -> Result<()>;
    /// `questRepository.isComplete(quest)`.
    fn quest_complete(&mut self, player: &Self::Node, quest: &str) -> Result<bool>;
    /// `skills.getLevel(skill)`.
    fn level(&mut self, player: &Self::Node, skill: i32) -> Result<i32>;
    /// `packetDispatch.sendMessage(text)` (`player.sendMessage` too).
    fn send_message(&mut self, player: &Self::Node, text: &str) -> Result<()>;
    /// `inventory.contains(id, amount)` (`containsItem` of an item too).
    fn inventory_contains(&mut self, player: &Self::Node, id: i32, amount: i32) -> Result<bool>;
    /// `inventory.remove(new Item(id, amount))`.
    fn inventory_remove(&mut self, player: &Self::Node, id: i32, amount: i32) -> Result<bool>;
    /// `inventory.remove(Item...)`: every item is tried, then one update;
    /// whether all were removed.
    fn inventory_remove_all(&mut self, player: &Self::Node, items: &[(i32, i32)]) -> Result<bool>;
    /// `inventory.add(new Item(id, amount))`.
    fn inventory_add(&mut self, player: &Self::Node, id: i32, amount: i32) -> Result<bool>;
    /// `inventory.freeSlots()`.
    fn free_slots(&mut self, player: &Self::Node) -> Result<i32>;
    /// `player.getLocation()`.
    fn location(&mut self, player: &Self::Node) -> Result<[i32; 3]>;
    /// `achievementDiaryManager.getDiary(type).getLevel()`.
    fn diary_level(&mut self, player: &Self::Node, diary: Diary) -> Result<i32>;
    /// `achievementDiaryManager.getDiary(type).isComplete(level)`.
    fn diary_complete(&mut self, player: &Self::Node, diary: Diary, level: i32) -> Result<bool>;
    /// `achievementDiaryManager.checkSmithReward(bar)`, by the bar's ordinal.
    fn check_smith_reward(&mut self, player: &Self::Node, bar: usize) -> Result<bool>;
    /// `achievementDiaryManager.finishTask(player, type, level, index)`.
    fn diary_finish_task(
        &mut self,
        player: &Self::Node,
        diary: Diary,
        level: i32,
        index: i32,
    ) -> Result<()>;
    /// `RandomFunction.random(max)`.
    fn random(&mut self, max: i32) -> Result<i32>;
    /// `RandomFunction.nextBool()`.
    fn next_bool(&mut self) -> Result<bool>;
    /// `player.dispatch(ResourceProducedEvent(item, amount, player, -1))`.
    fn resource_produced(&mut self, player: &Self::Node, item: i32, amount: i32) -> Result<()>;
    /// `equipment.get(SLOT_HANDS)`'s id, if the slot holds an item.
    fn hands(&mut self, player: &Self::Node) -> Result<Option<i32>>;
    /// `skills.addExperience(skill, experience, true)`.
    fn add_experience(&mut self, player: &Self::Node, skill: i32, experience: f64) -> Result<()>;
    /// ContentAPI `inEquipment(player, id, amount)`.
    fn in_equipment(&mut self, player: &Self::Node, id: i32, amount: i32) -> Result<bool>;
    /// ContentAPI `getAttribute(player, key, default)` of an Int.
    fn attribute_int(&mut self, player: &Self::Node, key: &str, default: i32) -> Result<i32>;
    /// ContentAPI `setAttribute(player, key, value)` of an Int.
    fn set_attribute_int(&mut self, player: &Self::Node, key: &str, value: i32) -> Result<()>;
    /// ContentAPI `removeItem(player, id, Container.EQUIPMENT)`.
    fn remove_equipment(&mut self, player: &Self::Node, id: i32) -> Result<bool>;
    /// `log(class, Log.ERR, message)`.
    fn log_error(&mut self, message: &str) -> Result<()>;
    /// `player.getName()`.
    fn player_name(&mut self, player: &Self::Node) -> Result<String>;
    /// `player.animate(Animation.create(id))`.
    fn animate(&mut self, player: &Self::Node, id: i32) -> Result<()>;
    /// `player.visualize(Animation.create(animation), new Graphics(id, height))`.
    fn visualize(
        &mut self,
        player: &Self::Node,
        animation: i32,
        graphics: (i32, i32),
    ) -> Result<()>;
    /// `player.animate(new Animation(-1, Priority.HIGH))` of `SkillPulse.stop`.
    fn reset_animation(&mut self, player: &Self::Node) -> Result<()>;
    /// ContentAPI `playAudio(player, id)`.
    fn play_audio(&mut self, player: &Self::Node, id: i32) -> Result<()>;
    /// `pulseManager.run(pulse)` (`submitIndividualPulse` too): the manager
    /// starts the pulse and submits it while it is running.
    fn run_pulse(&mut self, player: &Self::Node, pulse: SmeltingPulse<Self::Node>) -> Result<()>;
    /// ContentAPI `sendInputDialogue(player, true, prompt, handler)`.
    fn send_input(&mut self, player: &Self::Node, prompt: &str, handler: AmountInput)
    -> Result<()>;
    /// `zoneMonitor.isInZone(name)`.
    fn in_zone(&mut self, player: &Self::Node, name: &str) -> Result<bool>;
}

/// What `FurnaceOptionPlugin.newInstance` and its handler's register.
pub trait Registrar {
    /// `SceneryDefinition.setOptionHandler(name, plugin)`.
    fn set_option_handler(&mut self, name: &str) -> Result<()>;
    /// `SceneryDefinition.forId(id).getHandlers().put(key, plugin)`.
    fn put_handler(&mut self, id: i32, key: &str) -> Result<()>;
    /// `new SmeltUseWithHandler()`: `UseWithHandler(allowedNodes)`.
    fn new_use_handler(&mut self, allowed: Vec<i32>) -> Result<()>;
    /// `addHandler(id, type, handler)` of that handler.
    fn add_use_handler(&mut self, id: i32, kind: i32) -> Result<()>;
}

/// `FurnaceOptionPlugin.newInstance(arg)`, then
/// `new SmeltUseWithHandler().newInstance(arg)`.
pub fn new_instance<R: Registrar + ?Sized>(r: &mut R) -> Result<()> {
    for name in OPTIONS {
        r.set_option_handler(name)?;
    }
    for id in USE_FURNACES {
        r.put_handler(id, "option:use")?;
    }
    r.new_use_handler(ore_ids())?;
    for id in FURNACES {
        r.add_use_handler(id, OBJECT_TYPE)?;
    }
    Ok(())
}

/// `FurnaceOptionPlugin.handle(player, node, option)` and
/// `SmeltUseWithHandler.handle(event)` (the furnace is `usedWith`): the
/// Edgeville furnace's diary check, then the chatbox.
pub fn handle<H: Host + ?Sized>(h: &mut H, player: &H::Node, furnace: &H::Node) -> Result<bool> {
    if h.id(furnace)? == EDGEVILLE_FURNACE && !h.diary_complete(player, Diary::Varrock, 0)? {
        h.send_message(
            player,
            "You need to have completed the easy tasks in the Varrock Diary in order to use this.",
        )?;
        return Ok(true);
    }
    show(h, player)?;
    Ok(true)
}

/// `FurnaceOptionPlugin.show(player)`.
pub fn show<H: Host + ?Sized>(h: &mut H, player: &H::Node) -> Result<()> {
    h.open_chatbox(player, INTERFACE)?;
    h.send_item_zoom(player, 2349, 150, INTERFACE, 4)?;
    if h.quest_complete(player, KNIGHTS_SWORD)? {
        h.send_string(player, "<br><br><br><br><col=000000>Blurite", INTERFACE, 20)?;
    }
    for (item, child) in [
        (BARS[BLURITE].product, 5),
        (BARS[IRON].product, 6),
        (2355, 7),
        (2353, 8),
        (2357, 9),
        (2359, 10),
        (2361, 11),
        (2363, 12),
    ] {
        h.send_item_zoom(player, item, 150, INTERFACE, child)?;
    }
    Ok(())
}

/// `FurnaceOptionPlugin.getDestination(node, n)` for a player: a tile
/// beside the furnace in the "Donator Zone", otherwise null.
pub fn destination<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    furnace: [i32; 3],
) -> Result<Option<[i32; 3]>> {
    if h.in_zone(player, "Donator Zone")? {
        return Ok(Some([furnace[0] + 1, furnace[1] + 2, furnace[2]]));
    }
    Ok(None)
}

/// The amount prompt's handler: `submitIndividualPulse(player,
/// SmeltingPulse(player, null, bar, value))`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AmountInput {
    pub bar: usize,
}
impl AmountInput {
    /// A String is parsed with `Integer.parseInt` (a NumberFormatException
    /// otherwise).
    pub fn run<H: Host + ?Sized>(&self, h: &mut H, player: &H::Node, value: &Input) -> Result<()> {
        let amount = match value {
            Input::Int(n) => *n,
            Input::Text(text) => text
                .parse::<i32>()
                .map_err(|_| Error::Binding(Some("NumberFormatException".into())))?,
        };
        let pulse = SmeltingPulse::new(player.clone(), Some(self.bar), amount);
        h.run_pulse(player, pulse)
    }
}

/// `SmeltingInterface.handle(player, component, opcode, button, slot,
/// itemId)`: a bar button runs the pulse, or asks the amount first.
pub fn interface_button<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    button: i32,
) -> Result<bool> {
    let Some((bar, amount)) = bar_button(button) else {
        return Ok(true);
    };
    if amount == -1 {
        h.close_chatbox(player)?;
        h.send_input(player, "Enter the amount:", AmountInput { bar })?;
    } else {
        let pulse = SmeltingPulse::new(player.clone(), Some(bar), amount);
        h.run_pulse(player, pulse)?;
    }
    Ok(true)
}

/// `SmeltingPulse` over `SkillPulse<Item>`: a `Pulse(1, player, node)`
/// that `SkillPulse`'s constructor stops, so it runs only once `start()`
/// passed the requirements.
#[derive(Clone, Debug, PartialEq)]
pub struct SmeltingPulse<N> {
    pub player: N,
    /// The bar's ordinal (null for a caller that found none).
    pub bar: Option<usize>,
    pub super_heat: bool,
    pub ticks: i32,
    pub amount: i32,
    /// `SkillPulse.resetAnimation`.
    pub reset_animation: bool,
    /// `Pulse.running`.
    pub running: bool,
}

impl<N: Clone> SmeltingPulse<N> {
    /// `SmeltingPulse(player, node, bar, amount)`.
    pub fn new(player: N, bar: Option<usize>, amount: i32) -> Self {
        Self {
            player,
            bar,
            super_heat: false,
            ticks: 0,
            amount,
            reset_animation: true,
            running: false,
        }
    }
    /// `SmeltingPulse(player, node, bar, amount, heat)`: `resetAnimation`
    /// is off whatever `heat` is.
    pub fn with_heat(player: N, bar: Option<usize>, amount: i32, heat: bool) -> Self {
        Self {
            super_heat: heat,
            reset_animation: false,
            ..Self::new(player, bar, amount)
        }
    }

    /// `SkillPulse.start()`.
    pub fn start<H: Host<Node = N> + ?Sized>(&mut self, h: &mut H) -> Result<()> {
        if self.check_requirements(h)? {
            self.running = true;
        }
        Ok(())
    }

    /// `SkillPulse.stop()`: the animation reset, then `Pulse.stop()`.
    pub fn stop<H: Host<Node = N> + ?Sized>(&mut self, h: &mut H) -> Result<()> {
        if self.reset_animation {
            let player = self.player.clone();
            h.reset_animation(&player)?;
        }
        self.running = false;
        Ok(())
    }

    /// `SkillPulse.pulse()`.
    pub fn pulse<H: Host<Node = N> + ?Sized>(&mut self, h: &mut H) -> Result<bool> {
        if !self.check_requirements(h)? {
            return Ok(true);
        }
        self.animate(h)?;
        self.reward(h)
    }

    /// `Pulse.update()` once the pulse is due (its delay is one tick): the
    /// body, `stop()` when it finished or threw, and whether the runner
    /// drops the pulse. A body that threw is the error (the original prints
    /// it and drops the pulse). The checked nodes are the caller's.
    pub fn update<H: Host<Node = N> + ?Sized>(&mut self, h: &mut H) -> Result<bool> {
        if !self.running {
            return Ok(true);
        }
        match self.pulse(h) {
            Ok(true) => {
                self.stop(h)?;
                Ok(true)
            }
            Ok(false) => Ok(!self.running),
            Err(error) => {
                self.stop(h)?;
                Err(error)
            }
        }
    }

    /// `checkRequirements()`.
    pub fn check_requirements<H: Host<Node = N> + ?Sized>(&mut self, h: &mut H) -> Result<bool> {
        let player = self.player.clone();
        h.close_chatbox(&player)?;
        let Some(ordinal) = self.bar else {
            return Ok(false);
        };
        let bar = BARS.get(ordinal).ok_or(Error::Index)?;
        if ordinal == BLURITE && !h.quest_complete(&player, KNIGHTS_SWORD)? {
            return Ok(false);
        }
        if h.level(&player, SMITHING)? < bar.level {
            let name = h.item_name(bar.product)?;
            let name = h.lowercase(&name).replace("bar", "");
            h.send_message(
                &player,
                &format!(
                    "You need a Smithing level of at least {} in order to smelt {name}.",
                    bar.level
                ),
            )?;
            h.close_chatbox(&player)?;
            return Ok(false);
        }
        for (id, amount) in bar.ores {
            if !h.inventory_contains(&player, *id, *amount)? {
                h.send_message(
                    &player,
                    "You do not have the required ores to make this bar.",
                )?;
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// `animate()`.
    pub fn animate<H: Host<Node = N> + ?Sized>(&mut self, h: &mut H) -> Result<()> {
        if self.ticks == 0 || self.ticks.wrapping_rem(5) == 0 {
            let player = self.player.clone();
            if self.super_heat {
                h.visualize(&player, SUPERHEAT_ANIMATION, SUPERHEAT_GRAPHICS)?;
            } else {
                h.animate(&player, ANIMATION)?;
                h.play_audio(&player, FURNACE_SOUND)?;
            }
        }
        Ok(())
    }

    /// `reward()`: every fifth tick at a furnace (every tick for superheat)
    /// the ores become a bar, or are lost to an impure iron ore.
    pub fn reward<H: Host<Node = N> + ?Sized>(&mut self, h: &mut H) -> Result<bool> {
        if !self.super_heat {
            self.ticks = self.ticks.wrapping_add(1);
            if self.ticks.wrapping_rem(5) != 0 {
                return Ok(false);
            }
        }
        let player = self.player.clone();
        let ordinal = self.bar.ok_or(Error::Null)?;
        let bar = BARS.get(ordinal).ok_or(Error::Index)?;
        if !self.super_heat {
            let name = format_display_name(&h.lowercase(bar.name));
            h.send_message(
                &player,
                &format!("You place the required ores and attempt to create a bar of {name}."),
            )?;
        }
        for (id, amount) in bar.ores {
            if !h.inventory_remove(&player, *id, *amount)? {
                return Ok(true);
            }
        }
        if self.success(h)? {
            // The Varrock armour's second bar.
            let mut amount = if h.free_slots(&player)? != 0
                && !self.super_heat
                && within_distance(h.location(&player)?, EDGEVILLE)
                && contains_items(h, &player, bar.ores)?
                && h.diary_level(&player, Diary::Varrock)? != -1
                && h.check_smith_reward(&player, ordinal)?
                && h.random(100)? <= 10
            {
                2
            } else {
                1
            };
            if amount != 1 {
                if !h.inventory_remove_all(&player, bar.ores)? {
                    amount = 1;
                } else {
                    h.send_message(
                        &player,
                        "The magic of the Varrock armour enables you to smelt 2 bars at the same time.",
                    )?;
                }
            }
            h.inventory_add(&player, bar.product, amount)?;
            h.resource_produced(&player, bar.product, 1)?;
            let mut experience = bar.experience * f64::from(amount);
            if h.hands(&player)? == Some(GOLDSMITH_GAUNTLETS) && bar.product == GOLD_BAR {
                experience = 56.2 * f64::from(amount);
            }
            h.add_experience(&player, SMITHING, experience)?;
            if !self.super_heat {
                let name = h.item_name(bar.product)?;
                let name = h.lowercase(&name).replace(" bar", "");
                h.send_message(&player, &format!("You retrieve a bar of {name}."))?;
            }
            if ordinal == STEEL && within_distance(h.location(&player)?, LUMBRIDGE) {
                h.diary_finish_task(&player, Diary::Lumbridge, 1, 5)?;
            }
            if ordinal == SILVER && within_distance(h.location(&player)?, LUMBRIDGE) {
                h.diary_finish_task(&player, Diary::Lumbridge, 2, 7)?;
            }
        } else {
            h.send_message(&player, "The ore is too impure and you fail to refine it.")?;
        }
        self.amount = self.amount.wrapping_sub(1);
        Ok(self.amount < 1)
    }

    /// `success(player)`: iron at a furnace is a coin toss without a ring
    /// of forging, whose last charge takes the ring and stops the pulse.
    pub fn success<H: Host<Node = N> + ?Sized>(&mut self, h: &mut H) -> Result<bool> {
        if self.bar == Some(IRON) && !self.super_heat {
            let player = self.player.clone();
            if h.in_equipment(&player, RING_OF_FORGING, 1)? {
                let mut charges = h
                    .attribute_int(&player, "ringOfForgingCharges", 140)?
                    .wrapping_sub(1);
                if charges <= 0 {
                    if h.remove_equipment(&player, RING_OF_FORGING)? {
                        charges = 140;
                        h.send_message(
                            &player,
                            "Your Ring of forging uses up its last charge and disintegrates.",
                        )?;
                        self.stop(h)?;
                    } else {
                        let name = h.player_name(&player)?;
                        h.log_error(&format!(
                            "Failed to delete empty ring of forging for player {name}"
                        ))?;
                        return Ok(false);
                    }
                }
                h.set_attribute_int(&player, "/save:ringOfForgingCharges", charges)?;
                return Ok(true);
            }
            return h.next_bool();
        }
        Ok(true)
    }
}

/// `inventory.containsItems(Item...)`.
fn contains_items<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    items: &[(i32, i32)],
) -> Result<bool> {
    for (id, amount) in items {
        if !h.inventory_contains(player, *id, *amount)? {
            return Ok(false);
        }
    }
    Ok(true)
}
