//! `world::smelting` against a recording host written from the pinned
//! `FurnaceOptionPlugin.java`, `SmeltingInterface.java`, `SmeltingPulse.java`,
//! `SkillPulse.java` and `Bar.java`. There is no JVM corpus
//! (docs/smelting.md): this checks the port against a reading of the
//! sources. The tables are also compared with the text of `Bar.java`,
//! `SmeltingInterface.java` and `FurnaceOptionPlugin.java` when the
//! reference checkout is present.
use rs09_server::world::smelting::{
    self, AmountInput, Diary, Error, Host, Input, Registrar, Result, SmeltingPulse,
};
use std::collections::{HashMap, VecDeque};

#[derive(Clone, Debug, PartialEq)]
enum Node {
    Player,
    Scenery(i32),
}

struct Recorder {
    calls: Vec<String>,
    inventory: HashMap<i32, i32>,
    slots: i32,
    level: i32,
    quest: bool,
    at: [i32; 3],
    varrock_level: i32,
    varrock_easy: bool,
    smith_reward: bool,
    ints: VecDeque<i32>,
    bools: VecDeque<bool>,
    hands: Option<i32>,
    ring: bool,
    ring_removable: bool,
    charges: Option<i32>,
    zone: bool,
    pulses: Vec<SmeltingPulse<Node>>,
    inputs: Vec<AmountInput>,
}

impl Recorder {
    fn new(items: &[(i32, i32)]) -> Self {
        Self {
            calls: Vec::new(),
            inventory: items.iter().copied().collect(),
            slots: 10,
            level: 99,
            quest: false,
            at: [3226, 3255, 0],
            varrock_level: -1,
            varrock_easy: false,
            smith_reward: false,
            ints: VecDeque::new(),
            bools: VecDeque::new(),
            hands: None,
            ring: false,
            ring_removable: true,
            charges: None,
            zone: false,
            pulses: Vec::new(),
            inputs: Vec::new(),
        }
    }
    fn held(&self, id: i32) -> i32 {
        self.inventory.get(&id).copied().unwrap_or(0)
    }
    fn take(&mut self) -> Vec<String> {
        std::mem::take(&mut self.calls)
    }
    fn messages(&self) -> Vec<&str> {
        self.calls
            .iter()
            .filter_map(|c| c.strip_prefix("message "))
            .collect()
    }
}

fn name(id: i32) -> &'static str {
    match id {
        2349 => "Bronze bar",
        9467 => "Blurite bar",
        2351 => "Iron bar",
        2355 => "Silver bar",
        2353 => "Steel bar",
        2357 => "Gold bar",
        2359 => "Mithril bar",
        2361 => "Adamantite bar",
        2363 => "Runite bar",
        _ => "Item",
    }
}

impl Host for Recorder {
    type Node = Node;
    fn id(&mut self, node: &Node) -> Result<i32> {
        match node {
            Node::Player => Err(Error::Cast),
            Node::Scenery(id) => Ok(*id),
        }
    }
    fn item_name(&mut self, id: i32) -> Result<String> {
        Ok(name(id).to_owned())
    }
    fn lowercase(&self, text: &str) -> String {
        text.to_lowercase()
    }
    fn close_chatbox(&mut self, _: &Node) -> Result<()> {
        self.calls.push("closeChatbox".into());
        Ok(())
    }
    fn open_chatbox(&mut self, _: &Node, interface: i32) -> Result<()> {
        self.calls.push(format!("openChatbox {interface}"));
        Ok(())
    }
    fn send_item_zoom(&mut self, _: &Node, item: i32, zoom: i32, i: i32, c: i32) -> Result<()> {
        self.calls.push(format!("zoom {item} {zoom} {i}:{c}"));
        Ok(())
    }
    fn send_string(&mut self, _: &Node, text: &str, i: i32, c: i32) -> Result<()> {
        self.calls.push(format!("string {text} {i}:{c}"));
        Ok(())
    }
    fn quest_complete(&mut self, _: &Node, quest: &str) -> Result<bool> {
        assert_eq!(quest, "The Knight's Sword");
        Ok(self.quest)
    }
    fn level(&mut self, _: &Node, skill: i32) -> Result<i32> {
        assert_eq!(skill, 13);
        Ok(self.level)
    }
    fn send_message(&mut self, _: &Node, text: &str) -> Result<()> {
        self.calls.push(format!("message {text}"));
        Ok(())
    }
    fn inventory_contains(&mut self, _: &Node, id: i32, amount: i32) -> Result<bool> {
        Ok(self.held(id) >= amount)
    }
    fn inventory_remove(&mut self, _: &Node, id: i32, amount: i32) -> Result<bool> {
        if self.held(id) < amount {
            self.calls.push(format!("remove {id}x{amount} failed"));
            return Ok(false);
        }
        *self.inventory.get_mut(&id).unwrap() -= amount;
        self.calls.push(format!("remove {id}x{amount}"));
        Ok(true)
    }
    fn inventory_remove_all(&mut self, player: &Node, items: &[(i32, i32)]) -> Result<bool> {
        let mut all = true;
        for (id, amount) in items {
            all &= self.inventory_remove(player, *id, *amount)?;
        }
        Ok(all)
    }
    fn inventory_add(&mut self, _: &Node, id: i32, amount: i32) -> Result<bool> {
        *self.inventory.entry(id).or_default() += amount;
        self.calls.push(format!("add {id}x{amount}"));
        Ok(true)
    }
    fn free_slots(&mut self, _: &Node) -> Result<i32> {
        Ok(self.slots)
    }
    fn location(&mut self, _: &Node) -> Result<[i32; 3]> {
        Ok(self.at)
    }
    fn diary_level(&mut self, _: &Node, diary: Diary) -> Result<i32> {
        assert_eq!(diary, Diary::Varrock);
        Ok(self.varrock_level)
    }
    fn diary_complete(&mut self, _: &Node, diary: Diary, level: i32) -> Result<bool> {
        assert_eq!((diary, level), (Diary::Varrock, 0));
        Ok(self.varrock_easy)
    }
    fn check_smith_reward(&mut self, _: &Node, bar: usize) -> Result<bool> {
        self.calls.push(format!("checkSmithReward {bar}"));
        Ok(self.smith_reward)
    }
    fn diary_finish_task(&mut self, _: &Node, diary: Diary, level: i32, index: i32) -> Result<()> {
        self.calls
            .push(format!("finishTask {diary:?} {level} {index}"));
        Ok(())
    }
    fn random(&mut self, max: i32) -> Result<i32> {
        assert_eq!(max, 100);
        Ok(self.ints.pop_front().expect("a random draw"))
    }
    fn next_bool(&mut self) -> Result<bool> {
        Ok(self.bools.pop_front().expect("a nextBool draw"))
    }
    fn resource_produced(&mut self, _: &Node, item: i32, amount: i32) -> Result<()> {
        self.calls.push(format!("produced {item}x{amount}"));
        Ok(())
    }
    fn hands(&mut self, _: &Node) -> Result<Option<i32>> {
        Ok(self.hands)
    }
    fn add_experience(&mut self, _: &Node, skill: i32, experience: f64) -> Result<()> {
        self.calls.push(format!("xp {skill} {experience}"));
        Ok(())
    }
    fn in_equipment(&mut self, _: &Node, id: i32, amount: i32) -> Result<bool> {
        assert_eq!((id, amount), (2568, 1));
        Ok(self.ring)
    }
    fn attribute_int(&mut self, _: &Node, key: &str, default: i32) -> Result<i32> {
        assert_eq!(key, "ringOfForgingCharges");
        Ok(self.charges.unwrap_or(default))
    }
    fn set_attribute_int(&mut self, _: &Node, key: &str, value: i32) -> Result<()> {
        self.calls.push(format!("set {key} {value}"));
        self.charges = Some(value);
        Ok(())
    }
    fn remove_equipment(&mut self, _: &Node, id: i32) -> Result<bool> {
        self.calls.push(format!("unequip {id}"));
        if self.ring_removable {
            self.ring = false;
        }
        Ok(self.ring_removable)
    }
    fn log_error(&mut self, message: &str) -> Result<()> {
        self.calls.push(format!("log {message}"));
        Ok(())
    }
    fn player_name(&mut self, _: &Node) -> Result<String> {
        Ok("smith".into())
    }
    fn animate(&mut self, _: &Node, id: i32) -> Result<()> {
        self.calls.push(format!("animate {id}"));
        Ok(())
    }
    fn visualize(&mut self, _: &Node, animation: i32, graphics: (i32, i32)) -> Result<()> {
        self.calls
            .push(format!("visualize {animation} {graphics:?}"));
        Ok(())
    }
    fn reset_animation(&mut self, _: &Node) -> Result<()> {
        self.calls.push("resetAnimation".into());
        Ok(())
    }
    fn play_audio(&mut self, _: &Node, id: i32) -> Result<()> {
        self.calls.push(format!("audio {id}"));
        Ok(())
    }
    fn run_pulse(&mut self, _: &Node, mut pulse: SmeltingPulse<Node>) -> Result<()> {
        // PulseManager.run: start(), submitted while running.
        self.calls.push("run".into());
        pulse.start(self)?;
        self.pulses.push(pulse);
        Ok(())
    }
    fn send_input(&mut self, _: &Node, prompt: &str, handler: AmountInput) -> Result<()> {
        self.calls.push(format!("input {prompt}"));
        self.inputs.push(handler);
        Ok(())
    }
    fn in_zone(&mut self, _: &Node, name: &str) -> Result<bool> {
        assert_eq!(name, "Donator Zone");
        Ok(self.zone)
    }
}

/// Runs a started pulse until the runner drops it; the ticks it took.
fn run(host: &mut Recorder, pulse: &mut SmeltingPulse<Node>) -> i32 {
    let mut ticks = 0;
    loop {
        ticks += 1;
        assert!(ticks < 500, "the pulse never finished");
        if pulse.update(host).unwrap() {
            return ticks;
        }
    }
}

#[test]
fn the_furnace_shows_the_chatbox() {
    let mut host = Recorder::new(&[]);
    assert_eq!(
        smelting::handle(&mut host, &Node::Player, &Node::Scenery(11666)),
        Ok(true)
    );
    assert_eq!(
        host.take(),
        [
            "openChatbox 311",
            "zoom 2349 150 311:4",
            "zoom 9467 150 311:5",
            "zoom 2351 150 311:6",
            "zoom 2355 150 311:7",
            "zoom 2353 150 311:8",
            "zoom 2357 150 311:9",
            "zoom 2359 150 311:10",
            "zoom 2361 150 311:11",
            "zoom 2363 150 311:12",
        ]
    );
    // The Knight's Sword names the blurite bar.
    host.quest = true;
    smelting::show(&mut host, &Node::Player).unwrap();
    assert_eq!(
        host.take()[2],
        "string <br><br><br><br><col=000000>Blurite 311:20"
    );
}

#[test]
fn the_edgeville_furnace_asks_for_the_varrock_diary() {
    let mut host = Recorder::new(&[]);
    let furnace = Node::Scenery(26814);
    assert_eq!(
        smelting::handle(&mut host, &Node::Player, &furnace),
        Ok(true)
    );
    assert_eq!(
        host.take(),
        [
            "message You need to have completed the easy tasks in the Varrock Diary in order to use this."
        ]
    );
    host.varrock_easy = true;
    smelting::handle(&mut host, &Node::Player, &furnace).unwrap();
    assert_eq!(host.take()[0], "openChatbox 311");
    // `node.getId()` of something that is not a scenery.
    assert_eq!(
        smelting::handle(&mut host, &Node::Player, &Node::Player),
        Err(Error::Cast)
    );
}

#[test]
fn the_destination_is_null_outside_the_donator_zone() {
    let mut host = Recorder::new(&[]);
    assert_eq!(
        smelting::destination(&mut host, &Node::Player, [3000, 3000, 0]),
        Ok(None)
    );
    host.zone = true;
    assert_eq!(
        smelting::destination(&mut host, &Node::Player, [3000, 3000, 0]),
        Ok(Some([3001, 3002, 0]))
    );
}

#[test]
fn a_bar_button_runs_the_pulse_or_asks_the_amount() {
    // Bronze, one: button 16.
    let mut host = Recorder::new(&[(436, 1), (438, 1)]);
    assert_eq!(
        smelting::interface_button(&mut host, &Node::Player, 16),
        Ok(true)
    );
    assert_eq!(host.take(), ["run", "closeChatbox"]);
    assert_eq!(host.pulses.len(), 1);
    assert_eq!(
        host.pulses[0],
        SmeltingPulse {
            player: Node::Player,
            bar: Some(smelting::BRONZE),
            super_heat: false,
            ticks: 0,
            amount: 1,
            reset_animation: true,
            running: true,
        }
    );
    // Steel, X: button 29 closes the chatbox and asks.
    smelting::interface_button(&mut host, &Node::Player, 29).unwrap();
    assert_eq!(host.take(), ["closeChatbox", "input Enter the amount:"]);
    assert_eq!(
        host.inputs,
        [AmountInput {
            bar: smelting::STEEL
        }]
    );
    // The handler: an Int, or a String parsed with Integer.parseInt.
    let input = host.inputs.remove(0);
    input.run(&mut host, &Node::Player, &Input::Int(7)).unwrap();
    assert_eq!(host.pulses[1].amount, 7);
    // No steel ores: start() refused, the pulse is not running.
    assert!(!host.pulses[1].running);
    input
        .run(&mut host, &Node::Player, &Input::Text("12".into()))
        .unwrap();
    assert_eq!(host.pulses[2].amount, 12);
    assert_eq!(
        input.run(&mut host, &Node::Player, &Input::Text("x".into())),
        Err(Error::Binding(Some("NumberFormatException".into())))
    );
    // Any other button does nothing.
    host.take();
    for button in [0, 12, 49, 100] {
        assert_eq!(
            smelting::interface_button(&mut host, &Node::Player, button),
            Ok(true)
        );
    }
    assert!(host.take().is_empty());
}

#[test]
fn bronze_takes_five_ticks_a_bar() {
    let mut host = Recorder::new(&[(436, 2), (438, 2)]);
    let mut pulse = SmeltingPulse::new(Node::Player, Some(smelting::BRONZE), 2);
    pulse.start(&mut host).unwrap();
    assert!(pulse.running);
    assert_eq!(host.take(), ["closeChatbox"]);
    // The first tick animates (ticks is 0), the next four only check.
    assert_eq!(pulse.update(&mut host), Ok(false));
    assert_eq!(host.take(), ["closeChatbox", "animate 3243", "audio 2725"]);
    for _ in 0..3 {
        assert_eq!(pulse.update(&mut host), Ok(false));
        assert_eq!(host.take(), ["closeChatbox"]);
    }
    // The fifth tick smelts.
    assert_eq!(pulse.update(&mut host), Ok(false));
    assert_eq!(
        host.take(),
        [
            "closeChatbox",
            "message You place the required ores and attempt to create a bar of Bronze.",
            "remove 436x1",
            "remove 438x1",
            "add 2349x1",
            "produced 2349x1",
            "xp 13 6.2",
            "message You retrieve a bar of bronze.",
        ]
    );
    // The sixth tick animates again (ticks is 5); the tenth smelts the
    // second bar and ends the pulse with the animation reset.
    assert_eq!(pulse.update(&mut host), Ok(false));
    assert_eq!(host.take(), ["closeChatbox", "animate 3243", "audio 2725"]);
    assert_eq!(run(&mut host, &mut pulse), 4);
    assert_eq!(host.calls.last().unwrap(), "resetAnimation");
    assert_eq!(host.held(2349), 2);
    assert_eq!((host.held(436), host.held(438)), (0, 0));
    assert!(!pulse.running);
}

#[test]
fn the_pulse_ends_when_the_ores_run_out() {
    let mut host = Recorder::new(&[(436, 1), (438, 1)]);
    let mut pulse = SmeltingPulse::new(Node::Player, Some(smelting::BRONZE), 5);
    pulse.start(&mut host).unwrap();
    // Five ticks for the bar, then the next check fails.
    assert_eq!(run(&mut host, &mut pulse), 6);
    assert_eq!(host.held(2349), 1);
    assert_eq!(
        host.messages().last().unwrap(),
        &"You do not have the required ores to make this bar."
    );
    assert_eq!(host.calls.last().unwrap(), "resetAnimation");
}

#[test]
fn the_requirements_refuse_by_level_ores_and_quest() {
    // The level message keeps the space the removed "bar" leaves.
    let mut host = Recorder::new(&[(451, 1), (453, 8)]);
    host.level = 84;
    let mut pulse = SmeltingPulse::new(Node::Player, Some(8), 1);
    pulse.start(&mut host).unwrap();
    assert!(!pulse.running);
    assert_eq!(
        host.take(),
        [
            "closeChatbox",
            "message You need a Smithing level of at least 85 in order to smelt runite .",
            "closeChatbox",
        ]
    );
    // A pulse that never started is dropped by its first update.
    assert_eq!(pulse.update(&mut host), Ok(true));
    assert!(host.take().is_empty());
    // Steel needs two coal and one iron ore.
    let mut host = Recorder::new(&[(453, 1), (440, 1)]);
    let mut pulse = SmeltingPulse::new(Node::Player, Some(smelting::STEEL), 1);
    pulse.start(&mut host).unwrap();
    assert!(!pulse.running);
    assert_eq!(
        host.take(),
        [
            "closeChatbox",
            "message You do not have the required ores to make this bar."
        ]
    );
    // Blurite without The Knight's Sword: no message.
    let mut host = Recorder::new(&[(668, 1)]);
    let mut pulse = SmeltingPulse::new(Node::Player, Some(smelting::BLURITE), 1);
    pulse.start(&mut host).unwrap();
    assert!(!pulse.running);
    assert_eq!(host.take(), ["closeChatbox"]);
    host.quest = true;
    pulse.start(&mut host).unwrap();
    assert!(pulse.running);
    // A null bar.
    let mut pulse = SmeltingPulse::new(Node::Player, None, 1);
    pulse.start(&mut host).unwrap();
    assert!(!pulse.running);
}

#[test]
fn iron_is_a_coin_toss_without_a_ring() {
    let mut host = Recorder::new(&[(440, 2)]);
    host.bools = VecDeque::from([false, true]);
    let mut pulse = SmeltingPulse::new(Node::Player, Some(smelting::IRON), 2);
    pulse.start(&mut host).unwrap();
    assert_eq!(run(&mut host, &mut pulse), 10);
    assert_eq!(host.held(2351), 1);
    assert_eq!(host.held(440), 0);
    assert_eq!(
        host.messages(),
        [
            "You place the required ores and attempt to create a bar of Iron.",
            "The ore is too impure and you fail to refine it.",
            "You place the required ores and attempt to create a bar of Iron.",
            "You retrieve a bar of iron.",
        ]
    );
}

#[test]
fn the_ring_of_forging_counts_down_and_disintegrates() {
    let mut host = Recorder::new(&[(440, 3)]);
    host.ring = true;
    host.charges = Some(2);
    let mut pulse = SmeltingPulse::new(Node::Player, Some(smelting::IRON), 3);
    pulse.start(&mut host).unwrap();
    // The first bar: one charge left.
    for _ in 0..5 {
        assert_eq!(pulse.update(&mut host), Ok(false));
    }
    assert!(
        host.take()
            .contains(&"set /save:ringOfForgingCharges 1".to_owned())
    );
    // The second bar: the ring goes, the charges restart and `stop()` from
    // inside `success` ends the pulse after this bar.
    for _ in 0..4 {
        assert_eq!(pulse.update(&mut host), Ok(false));
    }
    assert_eq!(pulse.update(&mut host), Ok(true));
    let calls = host.take();
    let from = calls.iter().position(|c| c == "unequip 2568").unwrap();
    assert_eq!(
        calls[from..],
        [
            "unequip 2568",
            "message Your Ring of forging uses up its last charge and disintegrates.",
            "resetAnimation",
            "set /save:ringOfForgingCharges 140",
            "add 2351x1",
            "produced 2351x1",
            "xp 13 12.5",
            "message You retrieve a bar of iron.",
        ]
    );
    assert_eq!(host.held(2351), 2);
    assert_eq!(host.held(440), 1);
    // The ring that cannot be removed: logged, the ore is lost.
    let mut host = Recorder::new(&[(440, 1)]);
    host.ring = true;
    host.ring_removable = false;
    host.charges = Some(1);
    let mut pulse = SmeltingPulse::new(Node::Player, Some(smelting::IRON), 1);
    pulse.start(&mut host).unwrap();
    run(&mut host, &mut pulse);
    assert!(
        host.calls
            .contains(&"log Failed to delete empty ring of forging for player smith".to_owned())
    );
    assert_eq!(host.held(2351), 0);
    assert_eq!(
        host.messages().last().unwrap(),
        &"The ore is too impure and you fail to refine it."
    );
}

#[test]
fn the_goldsmith_gauntlets_and_the_lumbridge_diary() {
    let mut host = Recorder::new(&[(444, 1)]);
    host.hands = Some(776);
    let mut pulse = SmeltingPulse::new(Node::Player, Some(5), 1);
    pulse.start(&mut host).unwrap();
    run(&mut host, &mut pulse);
    assert!(host.calls.contains(&"xp 13 56.2".to_owned()));
    // Steel and silver near the Lumbridge furnace finish diary tasks.
    let mut host = Recorder::new(&[(453, 2), (440, 1), (442, 1)]);
    let mut pulse = SmeltingPulse::new(Node::Player, Some(smelting::STEEL), 1);
    pulse.start(&mut host).unwrap();
    run(&mut host, &mut pulse);
    assert!(host.calls.contains(&"finishTask Lumbridge 1 5".to_owned()));
    let mut pulse = SmeltingPulse::new(Node::Player, Some(smelting::SILVER), 1);
    pulse.start(&mut host).unwrap();
    run(&mut host, &mut pulse);
    assert!(host.calls.contains(&"finishTask Lumbridge 2 7".to_owned()));
    // Not from another plane or sixteen tiles away.
    let mut host = Recorder::new(&[(442, 1)]);
    host.at = [3226, 3270, 0];
    let mut pulse = SmeltingPulse::new(Node::Player, Some(smelting::SILVER), 1);
    pulse.start(&mut host).unwrap();
    run(&mut host, &mut pulse);
    assert!(!host.calls.iter().any(|c| c.starts_with("finishTask")));
    assert!(smelting::within_distance(
        [3226, 3269, 0],
        smelting::LUMBRIDGE
    ));
    assert!(!smelting::within_distance(
        [3226, 3254, 1],
        smelting::LUMBRIDGE
    ));
}

#[test]
fn the_varrock_armour_smelts_a_second_bar_in_edgeville() {
    let edgeville = |host: &mut Recorder| {
        host.at = [3108, 3500, 0];
        host.varrock_level = 0;
        host.smith_reward = true;
    };
    let mut host = Recorder::new(&[(436, 2), (438, 2)]);
    edgeville(&mut host);
    host.ints = VecDeque::from([10]);
    let mut pulse = SmeltingPulse::new(Node::Player, Some(smelting::BRONZE), 1);
    pulse.start(&mut host).unwrap();
    run(&mut host, &mut pulse);
    assert_eq!(host.held(2349), 2);
    assert_eq!((host.held(436), host.held(438)), (0, 0));
    assert!(host.calls.contains(&"xp 13 12.4".to_owned()));
    assert!(host.messages().contains(
        &"The magic of the Varrock armour enables you to smelt 2 bars at the same time."
    ));
    // A roll of 11 is one bar; so is a second set of ores that is missing
    // (no roll is drawn then).
    let mut host = Recorder::new(&[(436, 2), (438, 2)]);
    edgeville(&mut host);
    host.ints = VecDeque::from([11]);
    let mut pulse = SmeltingPulse::new(Node::Player, Some(smelting::BRONZE), 1);
    pulse.start(&mut host).unwrap();
    run(&mut host, &mut pulse);
    assert_eq!(host.held(2349), 1);
    let mut host = Recorder::new(&[(436, 1), (438, 1)]);
    edgeville(&mut host);
    let mut pulse = SmeltingPulse::new(Node::Player, Some(smelting::BRONZE), 1);
    pulse.start(&mut host).unwrap();
    run(&mut host, &mut pulse);
    assert_eq!(host.held(2349), 1);
    assert!(!host.calls.iter().any(|c| c.starts_with("checkSmithReward")));
    // No free slot, or no diary level: the reward is not asked.
    let mut host = Recorder::new(&[(436, 2), (438, 2)]);
    edgeville(&mut host);
    host.slots = 0;
    let mut pulse = SmeltingPulse::new(Node::Player, Some(smelting::BRONZE), 1);
    pulse.start(&mut host).unwrap();
    run(&mut host, &mut pulse);
    assert_eq!(host.held(2349), 1);
    assert!(!host.calls.iter().any(|c| c.starts_with("checkSmithReward")));
}

#[test]
fn superheat_smelts_every_tick_without_messages() {
    let mut host = Recorder::new(&[(440, 1)]);
    let mut pulse = SmeltingPulse::with_heat(Node::Player, Some(smelting::IRON), 1, true);
    assert!(!pulse.reset_animation);
    pulse.start(&mut host).unwrap();
    host.take();
    assert_eq!(pulse.update(&mut host), Ok(true));
    assert_eq!(
        host.take(),
        [
            "closeChatbox",
            "visualize 725 (148, 96)",
            "remove 440x1",
            "add 2351x1",
            "produced 2351x1",
            "xp 13 12.5",
        ]
    );
}

#[test]
fn display_names_and_tables() {
    assert_eq!(smelting::format_display_name("runite"), "Runite");
    assert_eq!(
        smelting::format_display_name("RING_OF_forging"),
        "Ring Of Forging"
    );
    assert_eq!(smelting::for_id(2353), Some(smelting::STEEL));
    assert_eq!(smelting::for_id(1), None);
    // Coal's first bar is steel, iron ore's is iron.
    assert_eq!(smelting::for_ore(453), Some(smelting::STEEL));
    assert_eq!(smelting::for_ore(440), Some(smelting::IRON));
    assert_eq!(
        smelting::ore_ids(),
        [
            436, 438, 668, 440, 442, 453, 440, 444, 447, 453, 449, 453, 451, 453
        ]
    );
    assert_eq!(smelting::bar_button(13), Some((0, -1)));
    assert_eq!(smelting::bar_button(16), Some((0, 1)));
    assert_eq!(smelting::bar_button(47), Some((8, 5)));
    assert_eq!(smelting::bar_button(48), Some((8, 1)));
    assert_eq!(smelting::bar_button(12), None);
    assert_eq!(smelting::bar_button(49), None);
}

#[derive(Default)]
struct Registered(Vec<String>);
impl Registrar for Registered {
    fn set_option_handler(&mut self, name: &str) -> Result<()> {
        self.0.push(format!("option {name}"));
        Ok(())
    }
    fn put_handler(&mut self, id: i32, key: &str) -> Result<()> {
        self.0.push(format!("put {id} {key}"));
        Ok(())
    }
    fn new_use_handler(&mut self, allowed: Vec<i32>) -> Result<()> {
        self.0.push(format!("handler {}", allowed.len()));
        Ok(())
    }
    fn add_use_handler(&mut self, id: i32, kind: i32) -> Result<()> {
        self.0.push(format!("add {id} {kind}"));
        Ok(())
    }
}

#[test]
fn new_instance_registers_the_plugin_and_its_handler() {
    let mut r = Registered::default();
    smelting::new_instance(&mut r).unwrap();
    assert_eq!(
        r.0[..5],
        [
            "option smelt",
            "option smelt-ore",
            "put 3044 option:use",
            "put 21303 option:use",
            "handler 14",
        ]
    );
    assert_eq!(r.0.len(), 5 + smelting::FURNACES.len());
    assert_eq!(r.0[5], "add 4304 2");
    assert_eq!(r.0.last().unwrap(), "add 37651 2");
}

#[test]
fn tables_match_the_pinned_source_text() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference/server/Server/src/main/content/global");
    let Ok(java) = std::fs::read_to_string(root.join("skill/smithing/smelting/Bar.java")) else {
        eprintln!("no reference checkout: Bar.java not compared");
        return;
    };
    // `NAME(level, experience, new Item(product, 1), new Item(ore[, n])...)`.
    let numbers = |text: &str| -> Vec<f64> {
        text.split(|c: char| !(c.is_ascii_digit() || c == '.'))
            .filter(|t| !t.is_empty())
            .map(|t| t.parse().unwrap())
            .collect()
    };
    let body = &java[java.find('{').unwrap()..java.find("private final Item[] ores").unwrap()];
    let mut rows = 0;
    for line in body.lines() {
        let line = line.trim();
        let Some(open) = line.find('(') else { continue };
        let name = &line[..open];
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_uppercase()) {
            continue;
        }
        let bar = &smelting::BARS[rows];
        rows += 1;
        assert_eq!(name, bar.name);
        let mut items = line[open + 1..].split("new Item(");
        let head = numbers(items.next().unwrap());
        assert_eq!(head, [f64::from(bar.level), bar.experience], "{name}");
        let items: Vec<(i32, i32)> = items
            .map(|item| {
                let n = numbers(&item[..item.find(')').unwrap()]);
                (n[0] as i32, n.get(1).map_or(1, |a| *a as i32))
            })
            .collect();
        assert_eq!(items[0], (bar.product, 1), "{name}");
        assert_eq!(&items[1..], bar.ores, "{name}");
    }
    assert_eq!(rows, smelting::BARS.len());

    // BarButton: `NAME_N(button, Bar.BAR, amount)`.
    let java = std::fs::read_to_string(root.join("handlers/iface/SmeltingInterface.java")).unwrap();
    let line = java
        .lines()
        .find(|line| line.contains("BRONZE_1(16"))
        .unwrap();
    let mut buttons = 0;
    for entry in line.split("),") {
        let Some(open) = entry.find('(') else {
            continue;
        };
        let mut fields = entry[open + 1..].split(',').map(str::trim);
        let button: i32 = fields.next().unwrap().parse().unwrap();
        let bar = fields.next().unwrap().trim_start_matches("Bar.");
        let amount: i32 = fields
            .next()
            .unwrap()
            .trim_end_matches([')', ';'])
            .parse()
            .unwrap();
        let (ordinal, ours) = smelting::bar_button(button).expect("a bar button");
        assert_eq!(smelting::BARS[ordinal].name, bar, "button {button}");
        assert_eq!(ours, amount, "button {button}");
        buttons += 1;
    }
    assert_eq!(buttons, 36);

    // furnaceIDS.
    let java =
        std::fs::read_to_string(root.join("skill/smithing/FurnaceOptionPlugin.java")).unwrap();
    let line = java
        .lines()
        .find(|line| line.contains("furnaceIDS = new int[]"))
        .unwrap();
    let ids: Vec<i32> = numbers(&line[line.find('{').unwrap()..])
        .into_iter()
        .map(|n| n as i32)
        .collect();
    assert_eq!(ids, smelting::FURNACES);
}
