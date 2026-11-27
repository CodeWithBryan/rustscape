//! `world::smithing` against a recording host written from the pinned
//! `SmithingPlugin.java`, `SmithingBuilder.java`, `SmithingInterface.java`,
//! `SmithingPulse.java`, `SkillPulse.java`, `SmithingType.java`,
//! `BarType.java` and `Bars.java`. There is no JVM corpus
//! (docs/smithing.md): this checks the port against a reading of the
//! sources. The three tables are also compared with the source text when
//! the reference checkout is present.
use rs09_server::world::smithing::{
    self, AmountInput, Diary, Error, Host, Input, Registrar, Result, SmithingPulse,
};
use std::collections::HashMap;

type Node = ();

struct Recorder {
    calls: Vec<String>,
    /// The inventory, slot by slot.
    slots: Vec<Option<(i32, i32)>>,
    level: i32,
    quests: Vec<&'static str>,
    requirements: Vec<&'static str>,
    barefisted: bool,
    at: [i32; 3],
    smith_type: Option<Option<usize>>,
    smith_bar: Option<usize>,
    smith_item: Option<i32>,
    pulses: Vec<SmithingPulse<Node>>,
    inputs: Vec<AmountInput>,
}

impl Recorder {
    fn new(items: &[(i32, i32)]) -> Self {
        Self {
            calls: Vec::new(),
            slots: items.iter().map(|item| Some(*item)).collect(),
            level: 99,
            quests: Vec::new(),
            requirements: Vec::new(),
            barefisted: false,
            at: [3188, 3425, 0],
            smith_type: None,
            smith_bar: None,
            smith_item: None,
            pulses: Vec::new(),
            inputs: Vec::new(),
        }
    }
    fn held(&self, id: i32) -> i32 {
        self.slots
            .iter()
            .flatten()
            .filter(|(item, _)| *item == id)
            .map(|(_, amount)| *amount)
            .sum()
    }
    fn take(&mut self) -> Vec<String> {
        std::mem::take(&mut self.calls)
    }
    fn dialogues(&self) -> Vec<&str> {
        self.calls
            .iter()
            .filter_map(|c| c.strip_prefix("dialogue "))
            .collect()
    }
}

fn names() -> HashMap<i32, &'static str> {
    HashMap::from([
        (2349, "Bronze bar"),
        (9467, "Blurite bar"),
        (2351, "Iron bar"),
        (2353, "Steel bar"),
        (2355, "Silver bar"),
        (2361, "Adamantite bar"),
        (2363, "Runite bar"),
        (1205, "Bronze dagger"),
        (1351, "Bronze axe"),
        (1117, "Bronze platebody"),
        (1295, "Steel longsword"),
        (1145, "Adamant med helm"),
        (9422, "Blurite limbs"),
        (819, "Bronze dart tip"),
        (3095, "Bronze claws"),
        (2370, "Steel studs"),
        (4819, "Bronze nails"),
    ])
}

impl Host for Recorder {
    type Node = Node;
    fn quest_complete(&mut self, _: &Node, quest: &str) -> Result<bool> {
        Ok(self.quests.contains(&quest))
    }
    fn has_requirement(&mut self, _: &Node, quest: &str) -> Result<bool> {
        Ok(self.requirements.contains(&quest))
    }
    fn barefisted(&mut self, _: &Node) -> Result<bool> {
        Ok(self.barefisted)
    }
    fn send_dialogue(&mut self, _: &Node, text: &str) -> Result<()> {
        self.calls.push(format!("dialogue {text}"));
        Ok(())
    }
    fn open_dialogue(&mut self, _: &Node, key: i32, args: &[i32]) -> Result<()> {
        self.calls.push(format!("open {key} {args:?}"));
        Ok(())
    }
    fn level(&mut self, _: &Node, skill: i32) -> Result<i32> {
        assert_eq!(skill, 13);
        Ok(self.level)
    }
    fn inventory_contains(&mut self, _: &Node, id: i32, amount: i32) -> Result<bool> {
        Ok(self.held(id) >= amount)
    }
    fn inventory_amount(&mut self, _: &Node, id: i32) -> Result<i32> {
        Ok(self.held(id))
    }
    fn inventory_ids(&mut self, _: &Node) -> Result<Vec<Option<i32>>> {
        Ok(self
            .slots
            .iter()
            .map(|slot| slot.map(|(id, _)| id))
            .collect())
    }
    fn inventory_remove(&mut self, _: &Node, id: i32, amount: i32) -> Result<bool> {
        self.calls.push(format!("remove {id}x{amount}"));
        let mut left = amount;
        for slot in &mut self.slots {
            if let Some((item, held)) = slot
                && *item == id
                && left > 0
            {
                let taken = left.min(*held);
                *held -= taken;
                left -= taken;
                if *held == 0 {
                    *slot = None;
                }
            }
        }
        Ok(left == 0)
    }
    fn inventory_add(&mut self, _: &Node, id: i32, amount: i32) -> Result<bool> {
        self.calls.push(format!("add {id}x{amount}"));
        self.slots.push(Some((id, amount)));
        Ok(true)
    }
    fn item_name(&mut self, id: i32) -> Result<String> {
        names()
            .get(&id)
            .map(|name| (*name).to_owned())
            .ok_or(Error::Null)
    }
    fn lowercase(&self, text: &str) -> String {
        text.to_lowercase()
    }
    fn remove_smith_type(&mut self, _: &Node) -> Result<()> {
        self.calls.push("removeAttribute smith-type".into());
        self.smith_type = None;
        Ok(())
    }
    fn set_smith_type(&mut self, _: &Node, kind: Option<usize>) -> Result<()> {
        self.calls.push(format!("smith-type {kind:?}"));
        self.smith_type = Some(kind);
        Ok(())
    }
    fn smith_type(&mut self, _: &Node) -> Result<Option<usize>> {
        Ok(self.smith_type.flatten())
    }
    fn set_smith_bar(&mut self, _: &Node, bar: usize) -> Result<()> {
        self.calls
            .push(format!("smith-bar {}", smithing::BARS[bar].name));
        self.smith_bar = Some(bar);
        Ok(())
    }
    fn smith_bar(&mut self, _: &Node) -> Result<Option<usize>> {
        Ok(self.smith_bar)
    }
    fn set_smith_item(&mut self, _: &Node, item: i32) -> Result<()> {
        self.calls.push(format!("smith-item {item}"));
        self.smith_item = Some(item);
        Ok(())
    }
    fn smith_item(&mut self, _: &Node) -> Result<i32> {
        self.smith_item.ok_or(Error::Null)
    }
    fn send_interface_config(&mut self, _: &Node, i: i32, c: i32, hidden: bool) -> Result<()> {
        self.calls.push(format!("config {i}:{c} {hidden}"));
        Ok(())
    }
    fn send_string(&mut self, _: &Node, text: &str, i: i32, c: i32) -> Result<()> {
        self.calls.push(format!("string {text} {i}:{c}"));
        Ok(())
    }
    fn generate_item(&mut self, _: &Node, id: i32, amount: i32, i: i32, c: i32) -> Result<()> {
        self.calls.push(format!("items {id}x{amount} {i}:{c}"));
        Ok(())
    }
    fn open_interface(&mut self, _: &Node, id: i32) -> Result<()> {
        self.calls.push(format!("openInterface {id}"));
        Ok(())
    }
    fn close_interface(&mut self, _: &Node) -> Result<()> {
        self.calls.push("close".into());
        Ok(())
    }
    fn animate(&mut self, _: &Node, id: i32) -> Result<()> {
        self.calls.push(format!("animate {id}"));
        Ok(())
    }
    fn reset_animation(&mut self, _: &Node) -> Result<()> {
        self.calls.push("resetAnimation".into());
        Ok(())
    }
    fn resource_produced(&mut self, _: &Node, item: i32, source: i32) -> Result<()> {
        self.calls.push(format!("produced {item} from {source}"));
        Ok(())
    }
    fn add_experience(&mut self, _: &Node, skill: i32, experience: f64) -> Result<()> {
        self.calls.push(format!("xp {skill} {experience}"));
        Ok(())
    }
    fn send_message(&mut self, _: &Node, text: &str) -> Result<()> {
        self.calls.push(format!("message {text}"));
        Ok(())
    }
    fn location(&mut self, _: &Node) -> Result<[i32; 3]> {
        Ok(self.at)
    }
    fn diary_finish_task(&mut self, _: &Node, diary: Diary, level: i32, index: i32) -> Result<()> {
        self.calls
            .push(format!("finishTask {diary:?} {level} {index}"));
        Ok(())
    }
    fn send_input(&mut self, _: &Node, prompt: &str, handler: AmountInput) -> Result<()> {
        self.calls.push(format!("input {prompt}"));
        self.inputs.push(handler);
        Ok(())
    }
    fn run_pulse(&mut self, _: &Node, mut pulse: SmithingPulse<Node>) -> Result<()> {
        // PulseManager.run: start(), submitted while running.
        self.calls.push("run".into());
        pulse.start(self)?;
        self.pulses.push(pulse);
        Ok(())
    }
}

/// Runs a started pulse as the runner does (the delay counted down between
/// runs) until it is dropped; the ticks it took.
fn run(host: &mut Recorder, pulse: &mut SmithingPulse<Node>) -> i32 {
    let mut ticks = 0;
    let mut wait = pulse.delay;
    loop {
        ticks += 1;
        assert!(ticks < 500, "the pulse never finished");
        wait -= 1;
        if wait > 0 {
            continue;
        }
        if pulse.update(host).unwrap() {
            return ticks;
        }
        wait = pulse.delay;
    }
}

fn bar(name: &str) -> usize {
    smithing::BARS
        .iter()
        .position(|bar| bar.name == name)
        .unwrap()
}

#[test]
fn dorics_anvil_and_the_hammer_refuse() {
    let mut h = Recorder::new(&[(2347, 1), (2349, 1)]);
    assert!(smithing::handle(&mut h, &(), 2349, 2782).unwrap());
    assert_eq!(h.take(), ["dialogue Property of Doric the Dwarf."]);
    h.quests.push("Doric's Quest");
    assert!(smithing::handle(&mut h, &(), 2349, 2782).unwrap());
    assert_eq!(h.take().last().unwrap(), "openInterface 300");

    let mut h = Recorder::new(&[(2349, 1)]);
    assert!(smithing::handle(&mut h, &(), 2349, 2783).unwrap());
    assert_eq!(
        h.take(),
        ["dialogue You need a hammer to work the metal with."]
    );
    h.barefisted = true;
    assert!(smithing::handle(&mut h, &(), 2349, 2783).unwrap());
    assert_eq!(h.take().last().unwrap(), "openInterface 300");
}

#[test]
fn the_shield_and_godsword_branches() {
    let mut h = Recorder::new(&[(2347, 1), (2366, 1)]);
    h.level = 59;
    smithing::handle(&mut h, &(), 2366, 2783).unwrap();
    assert_eq!(
        h.take(),
        ["dialogue You need a smithing level of 60 to smith this."]
    );
    h.level = 60;
    // One half is enough for the check as written (`&&`).
    smithing::handle(&mut h, &(), 2368, 2783).unwrap();
    assert_eq!(h.take(), ["open 82127843 [1, 2368]"]);
    let mut none = Recorder::new(&[(2347, 1)]);
    smithing::handle(&mut none, &(), 2366, 2783).unwrap();
    assert_eq!(
        none.take(),
        ["dialogue You need a shield left half and a shield right half."]
    );

    let mut h = Recorder::new(&[(2347, 1), (1540, 1)]);
    h.level = 89;
    smithing::handle(&mut h, &(), 11286, 2783).unwrap();
    assert_eq!(
        h.take(),
        ["dialogue You need a smithing level of 90 to smith this."]
    );
    h.level = 90;
    smithing::handle(&mut h, &(), 11286, 2783).unwrap();
    assert_eq!(h.take(), ["open 82127843 [2, 11286]"]);
    let mut none = Recorder::new(&[(2347, 1)]);
    smithing::handle(&mut none, &(), 1540, 2783).unwrap();
    assert_eq!(
        none.take(),
        ["dialogue You need a draconic visage and a anti-dragon shield."]
    );

    let mut h = Recorder::new(&[(2347, 1)]);
    h.level = 79;
    smithing::handle(&mut h, &(), 11710, 2783).unwrap();
    assert_eq!(
        h.take(),
        ["dialogue You need a smithing level of 80 to smith this."]
    );
    h.level = 80;
    smithing::handle(&mut h, &(), 11692, 2783).unwrap();
    assert_eq!(h.take(), ["open 62362 [11692]"]);
}

#[test]
fn a_bar_builds_the_interface() {
    let mut h = Recorder::new(&[(2347, 1), (2349, 2)]);
    h.level = 4;
    assert!(smithing::handle(&mut h, &(), 2349, 2783).unwrap());
    let calls = h.take();
    assert_eq!(
        &calls[..3],
        [
            "removeAttribute smith-type",
            "smith-type Some(0)",
            "config 300:267 false"
        ]
    );
    // The dagger: level 1, one bar held of two.
    assert_eq!(
        &calls[3..6],
        [
            "string <col=FFFFFF>Dagger 300:19",
            "string <col=2DE120>1 Bar 300:20",
            "items 1205x1 300:17"
        ]
    );
    // The dart tips show their child first.
    assert!(calls.contains(&"config 300:65 false".to_owned()));
    assert!(calls.contains(&"items 819x10 300:65".to_owned()));
    // A level-5 scimitar at level 4 has no colour; two bars are held.
    assert!(calls.contains(&"string Scimitar 300:115".to_owned()));
    assert!(calls.contains(&"string <col=2DE120>2 Bars 300:116".to_owned()));
    // The platebody's five bars are not held: no bar line.
    assert!(calls.contains(&"string Platebody 300:243".to_owned()));
    assert!(!calls.iter().any(|c| c.ends_with("300:244")));
    assert_eq!(
        &calls[calls.len() - 2..],
        ["string Bronze Smithing 300:14", "openInterface 300"]
    );
    let items = calls.iter().filter(|c| c.starts_with("items ")).count();
    assert_eq!(items, smithing::bars_of(0).len());
}

#[test]
fn blurite_hides_the_other_products() {
    let mut h = Recorder::new(&[(2347, 1), (9467, 1)]);
    smithing::handle(&mut h, &(), 9467, 2783).unwrap();
    let calls = h.take();
    let hidden: Vec<&String> = calls.iter().filter(|c| c.ends_with(" true")).collect();
    assert_eq!(hidden.len(), 21);
    assert_eq!(hidden[0], "config 300:17 true");
    assert_eq!(hidden[20], "config 300:241 true");
    assert!(!calls.contains(&"config 300:267 false".to_owned()));
    assert_eq!(calls.iter().filter(|c| c.starts_with("items ")).count(), 2);
    assert!(calls.contains(&"string Blurite Smithing 300:14".to_owned()));
}

#[test]
fn the_level_refusal_and_the_hammer_option() {
    let mut h = Recorder::new(&[(2347, 1), (2353, 1)]);
    h.level = 29;
    smithing::handle(&mut h, &(), 2353, 2783).unwrap();
    assert_eq!(
        h.take(),
        ["dialogue You need a smithing level of at least 30 to work steel bars."]
    );

    // The option handler: the hammer, then the first bar in the inventory.
    let mut h = Recorder::new(&[(2347, 1), (995, 5), (2351, 1), (2349, 1)]);
    assert!(smithing::option_handle(&mut h, &(), 42027).unwrap());
    assert_eq!(h.take()[1], "smith-type Some(2)");
    // No bar: nothing.
    let mut h = Recorder::new(&[(2347, 1), (995, 5)]);
    assert!(smithing::option_handle(&mut h, &(), 42027).unwrap());
    assert!(h.take().is_empty());
    // A silver bar is a smelting bar without a BarType: the attribute is
    // set to null and `type.name()` throws.
    let mut h = Recorder::new(&[(2347, 1), (2355, 1)]);
    assert_eq!(
        smithing::option_handle(&mut h, &(), 42027),
        Err(Error::Null)
    );
    assert_eq!(h.take(), ["removeAttribute smith-type", "smith-type None"]);
    // An item that is no bar at all (the allowed nodes keep others out).
    let mut h = Recorder::new(&[(2347, 1)]);
    assert!(smithing::handle(&mut h, &(), 995, 2783).unwrap());
    assert!(h.take().is_empty());
}

#[test]
fn the_buttons_pick_the_amount() {
    let dagger = bar("BRONZE_DAGGER");
    let mut h = Recorder::new(&[(2347, 1), (2349, 7)]);
    for (button, amount) in [(24, 1), (23, 5), (22, -1), (21, 7), (25, -1)] {
        assert_eq!(
            smithing::for_button(&mut h, &(), dagger, button, 2349).unwrap(),
            amount,
            "button {button}"
        );
    }
    // The studs have a fifth button, which answers its place.
    let studs = bar("STEEL_STUDS");
    assert_eq!(
        smithing::for_button(&mut h, &(), studs, 100, 2353).unwrap(),
        4
    );
    // Bullseye and oil lantern share buttons: the first row of the type wins.
    let steel = smithing::bar_type_for_id(2353);
    let first = smithing::BARS
        .iter()
        .find(|b| Some(b.bar_type) == steel && smithing::TYPES[b.kind].buttons.contains(&168))
        .unwrap();
    assert_eq!(smithing::item_id(168, steel), first.product);
    assert_eq!(smithing::item_id(24, None), -1);
    assert_eq!(smithing::item_id(24, Some(0)), 1205);
    assert_eq!(smithing::item_id(9999, Some(0)), -1);
}

#[test]
fn a_button_runs_the_pulse_or_asks() {
    let mut h = Recorder::new(&[(2347, 1), (2349, 3)]);
    // Without the attribute no row matches.
    assert!(smithing::interface_button(&mut h, &(), 24).unwrap());
    assert!(h.take().is_empty());
    h.smith_type = Some(Some(0));
    assert!(smithing::interface_button(&mut h, &(), 23).unwrap());
    assert_eq!(
        h.take(),
        ["smith-bar BRONZE_DAGGER", "smith-item 1205", "run", "close"]
    );
    let pulse = h.pulses.pop().unwrap();
    // Five asked, three bars held: the amount becomes the bar count.
    assert_eq!((pulse.item, pulse.amount, pulse.running), (1205, 3, true));

    assert!(smithing::interface_button(&mut h, &(), 22).unwrap());
    assert_eq!(
        h.take(),
        [
            "smith-bar BRONZE_DAGGER",
            "smith-item 1205",
            "input Enter the amount:"
        ]
    );
    let input = h.inputs.pop().unwrap();
    assert_eq!(
        input.run(&mut h, &(), &Input::Text("2".into())),
        Err(Error::Cast)
    );
    input.run(&mut h, &(), &Input::Int(2)).unwrap();
    let pulse = h.pulses.pop().unwrap();
    assert_eq!(
        (pulse.item, pulse.bar, pulse.amount),
        (1205, Some(bar("BRONZE_DAGGER")), 2)
    );
}

#[test]
fn a_dagger_takes_four_ticks_after_the_first_run() {
    let mut h = Recorder::new(&[(2347, 1), (2349, 2)]);
    let mut pulse = SmithingPulse::new((), 1205, Some(bar("BRONZE_DAGGER")), 2);
    pulse.start(&mut h).unwrap();
    assert_eq!(h.take(), ["close"]);
    // The first run only sets the delay.
    assert!(!pulse.update(&mut h).unwrap());
    assert_eq!(h.take(), ["close", "animate 898"]);
    assert_eq!(pulse.delay, 4);
    assert!(!pulse.update(&mut h).unwrap());
    assert_eq!(
        h.take(),
        [
            "close",
            "animate 898",
            "remove 2349x1",
            "add 1205x1",
            "produced 1205 from 2349",
            "xp 13 12.5",
            "message You hammer the bronze and make a bronze dagger.",
        ]
    );
    assert!(pulse.update(&mut h).unwrap());
    assert_eq!(h.calls.last().unwrap(), "resetAnimation");
    assert_eq!((h.held(1205), h.held(2349)), (2, 0));

    // The runner's view: one tick, then four a product.
    let mut h = Recorder::new(&[(2347, 1), (2349, 2)]);
    let mut pulse = SmithingPulse::new((), 1205, Some(bar("BRONZE_DAGGER")), 2);
    pulse.start(&mut h).unwrap();
    assert_eq!(run(&mut h, &mut pulse), 9);
}

#[test]
fn products_take_their_bars_and_amounts() {
    let mut h = Recorder::new(&[(2347, 1), (2349, 5)]);
    let mut pulse = SmithingPulse::new((), 1117, Some(bar("BRONZE_PLATEBODY")), 1);
    pulse.start(&mut h).unwrap();
    run(&mut h, &mut pulse);
    assert!(h.calls.contains(&"remove 2349x5".to_owned()));
    assert!(h.calls.contains(&"add 1117x1".to_owned()));
    assert!(h.calls.contains(&"xp 13 62.5".to_owned()));

    let mut h = Recorder::new(&[(2347, 1), (2349, 1)]);
    let mut pulse = SmithingPulse::new((), 4819, Some(bar("BRONZE_NAILS")), 1);
    pulse.start(&mut h).unwrap();
    run(&mut h, &mut pulse);
    assert!(h.calls.contains(&"add 4819x15".to_owned()));
    // "an" before a vowel.
    let mut h = Recorder::new(&[(2347, 1), (2361, 1)]);
    let mut pulse = SmithingPulse::new((), 1145, Some(bar("ADAMANT_MEDIUM_HELM")), 1);
    pulse.start(&mut h).unwrap();
    run(&mut h, &mut pulse);
    assert!(
        h.calls
            .contains(&"message You hammer the adamant and make an adamant med helm.".to_owned())
    );
}

#[test]
fn the_requirements_refuse() {
    let dagger = Some(bar("BRONZE_DAGGER"));
    let mut h = Recorder::new(&[(2347, 1), (2349, 1)]);
    h.level = 0;
    let mut pulse = SmithingPulse::new((), 1205, dagger, 1);
    pulse.start(&mut h).unwrap();
    assert!(!pulse.running);
    assert_eq!(
        h.dialogues(),
        ["You need a Smithing level of 1 to make a Bronze dagger."]
    );

    let mut h = Recorder::new(&[(2347, 1)]);
    let mut pulse = SmithingPulse::new((), 1205, dagger, 1);
    pulse.start(&mut h).unwrap();
    assert_eq!(pulse.amount, 0);
    assert_eq!(
        h.dialogues(),
        ["You don't have enough bronze bars to make a dagger."]
    );

    let mut h = Recorder::new(&[(2349, 1)]);
    let mut pulse = SmithingPulse::new((), 1205, dagger, 1);
    pulse.start(&mut h).unwrap();
    assert_eq!(h.dialogues(), ["You need a hammer to work the metal with."]);

    let mut h = Recorder::new(&[(2347, 1), (2349, 2)]);
    let mut pulse = SmithingPulse::new((), 819, Some(bar("BRONZE_DART_TIPS")), 1);
    pulse.start(&mut h).unwrap();
    assert_eq!(
        h.dialogues(),
        ["You need to complete Tourist Trap to smith dart tips."]
    );
    h.quests.push("The Tourist Trap");
    h.take();
    pulse.start(&mut h).unwrap();
    assert!(pulse.running);

    let mut pulse = SmithingPulse::new((), 3095, Some(bar("BRONZE_CLAWS")), 1);
    pulse.start(&mut h).unwrap();
    assert_eq!(
        h.dialogues(),
        ["You need to complete Death Plateau to smith claws."]
    );
    h.requirements.push("Death Plateau");
    pulse.start(&mut h).unwrap();
    assert!(pulse.running);

    // A pulse without its row (the prompt without the attribute).
    let mut pulse = SmithingPulse::new((), 1205, None, 1);
    assert_eq!(pulse.start(&mut h), Err(Error::Null));
}

#[test]
fn the_pulse_stops_when_the_bars_run_out_mid_way() {
    // Two platebodies asked with seven bars: the amount stays 2 until the
    // second check, where ten are not held and it becomes the bar count.
    let mut h = Recorder::new(&[(2347, 1), (2349, 7)]);
    let mut pulse = SmithingPulse::new((), 1117, Some(bar("BRONZE_PLATEBODY")), 2);
    pulse.start(&mut h).unwrap();
    assert_eq!(pulse.amount, 7);
    run(&mut h, &mut pulse);
    assert_eq!((h.held(1117), h.held(2349)), (1, 2));
    assert_eq!(
        h.dialogues(),
        ["You don't have enough bronze bars to make a platebody."]
    );
}

#[test]
fn the_barefisted_perk_and_the_diaries() {
    let mut h = Recorder::new(&[(2349, 1)]);
    h.barefisted = true;
    let mut pulse = SmithingPulse::new((), 1205, Some(bar("BRONZE_DAGGER")), 1);
    pulse.start(&mut h).unwrap();
    run(&mut h, &mut pulse);
    assert!(h.calls.contains(&"animate 2068".to_owned()));
    assert!(!h.calls.contains(&"animate 898".to_owned()));

    for (name, product, metal, at, away, task) in [
        (
            "BLURITE_CROSSBOW_LIMBS",
            9422,
            9467,
            [3000, 3155, 0],
            [3000, 3156, 0],
            "finishTask Falador 1 9",
        ),
        (
            "STEEL_LONGSWORD",
            1295,
            2353,
            [3112, 9703, 0],
            [3112, 9704, 0],
            "finishTask Lumbridge 2 0",
        ),
        (
            "ADAMANT_MEDIUM_HELM",
            1145,
            2361,
            [3247, 3419, 0],
            [3247, 3404, 1],
            "finishTask Varrock 2 3",
        ),
    ] {
        for (place, expected) in [(at, true), (away, false)] {
            let mut h = Recorder::new(&[(2347, 1), (metal, 2)]);
            h.at = place;
            let mut pulse = SmithingPulse::new((), product, Some(bar(name)), 1);
            pulse.start(&mut h).unwrap();
            run(&mut h, &mut pulse);
            assert_eq!(
                h.calls.contains(&task.to_owned()),
                expected,
                "{name} at {place:?}"
            );
        }
    }
}

#[test]
fn is_plus_n_and_distances() {
    assert!(smithing::is_plus_n("adamant med helm").unwrap());
    assert!(smithing::is_plus_n("hatchet").unwrap());
    assert!(!smithing::is_plus_n("helm").unwrap());
    assert!(!smithing::is_plus_n("h").unwrap());
    assert!(!smithing::is_plus_n("bronze dagger").unwrap());
    assert_eq!(smithing::is_plus_n(""), Err(Error::Index));
    assert!(smithing::within([0, 0, 0], [6, 8, 0], 10.0));
    assert!(!smithing::within([0, 0, 0], [6, 9, 0], 10.0));
    assert!(!smithing::within([0, 0, 0], [0, 0, 1], 10.0));
}

struct Registered(Vec<String>);
impl Registrar for Registered {
    fn new_use_handler(&mut self, allowed: Vec<i32>) -> Result<()> {
        self.0.push(format!("handler {allowed:?}"));
        Ok(())
    }
    fn add_use_handler(&mut self, id: i32, kind: i32) -> Result<()> {
        self.0.push(format!("add {id} {kind}"));
        Ok(())
    }
    fn define_option_plugin(&mut self, id: i32, key: &str) -> Result<()> {
        self.0.push(format!("define {id} {key}"));
        Ok(())
    }
}

#[test]
fn new_instance_registers_the_handler_and_the_option() {
    let mut r = Registered(Vec::new());
    smithing::new_instance(&mut r).unwrap();
    assert_eq!(r.0.len(), 11);
    assert!(r.0[0].starts_with("handler [2349, 2351, 2353, 2359, 2361, 2363, 2366, 2368, 9467,"));
    assert_eq!(r.0[1], "add 2782 2");
    assert_eq!(r.0[9], "add 42027 2");
    assert_eq!(r.0[10], "define 42027 option:smith");
}

#[test]
fn tables_match_the_pinned_source_text() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference/server/Server/src/main/content/global/skill/smithing");
    let Ok(java) = std::fs::read_to_string(root.join("Bars.java")) else {
        eprintln!("no reference checkout: the smithing tables are not compared");
        return;
    };
    let numbers = |text: &str| -> Vec<f64> {
        text.split(|c: char| !(c.is_ascii_digit() || c == '.'))
            .filter(|t| !t.is_empty() && *t != ".")
            .map(|t| t.parse().unwrap())
            .collect()
    };
    let constant = |line: &str| -> Option<(String, String)> {
        let line = line.trim();
        let open = line.find('(')?;
        let name = &line[..open];
        let valid = !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        valid.then(|| (name.to_owned(), line[open + 1..].to_owned()))
    };

    // `NAME(BarType.B, SmithingType.T, product, level)`.
    let mut rows = 0;
    for (name, rest) in java.lines().filter_map(constant) {
        if !rest.starts_with("BarType.") {
            continue;
        }
        let row = &smithing::BARS[rows];
        rows += 1;
        assert_eq!(name, row.name);
        let mut fields = rest.split(',').map(str::trim);
        assert_eq!(
            fields.next().unwrap(),
            format!("BarType.{}", smithing::BAR_TYPES[row.bar_type].name)
        );
        assert_eq!(
            fields.next().unwrap(),
            format!("SmithingType.{}", smithing::TYPES[row.kind].name)
        );
        let tail = numbers(&fields.collect::<Vec<_>>().join(","));
        assert_eq!(
            tail,
            [f64::from(row.product), f64::from(row.level)],
            "{name}"
        );
    }
    assert_eq!(rows, smithing::BARS.len());
    for name in [
        "BLURITE_CROSSBOW_LIMBS",
        "STEEL_LONGSWORD",
        "ADAMANT_MEDIUM_HELM",
    ] {
        let at = bar(name);
        let expected = match name {
            "BLURITE_CROSSBOW_LIMBS" => smithing::BLURITE_CROSSBOW_LIMBS,
            "STEEL_LONGSWORD" => smithing::STEEL_LONGSWORD,
            _ => smithing::ADAMANT_MEDIUM_HELM,
        };
        assert_eq!(at, expected);
    }

    // `TYPE_X(required, child, name, new int[] { buttons }, amount)`.
    let java = std::fs::read_to_string(root.join("SmithingType.java")).unwrap();
    let mut rows = 0;
    for (name, rest) in java.lines().filter_map(constant) {
        if !name.starts_with("TYPE_") {
            continue;
        }
        let row = &smithing::TYPES[rows];
        rows += 1;
        assert_eq!(name, row.name);
        let n: Vec<i32> = numbers(&rest).into_iter().map(|n| n as i32).collect();
        assert_eq!(&n[..3], [row.required, row.child, row.name_child], "{name}");
        assert_eq!(&n[3..n.len() - 1], row.buttons, "{name}");
        assert_eq!(n[n.len() - 1], row.product_amount, "{name}");
    }
    assert_eq!(rows, smithing::TYPES.len());
    assert_eq!(
        smithing::TYPES[smithing::TYPE_DART_TIP].name,
        "TYPE_DART_TIP"
    );
    assert_eq!(smithing::TYPES[smithing::TYPE_CLAWS].name, "TYPE_CLAWS");

    // `NAME(bar, experience, "Name Smithing")`.
    let java = std::fs::read_to_string(root.join("BarType.java")).unwrap();
    let mut rows = 0;
    for (name, rest) in java.lines().filter_map(constant) {
        let Some(quote) = rest.find('"') else {
            continue;
        };
        let row = &smithing::BAR_TYPES[rows];
        rows += 1;
        assert_eq!(name, row.name);
        assert_eq!(
            numbers(&rest[..quote]),
            [f64::from(row.bar), row.experience]
        );
        assert_eq!(rest[quote + 1..].split('"').next().unwrap(), row.bar_name);
    }
    assert_eq!(rows, smithing::BAR_TYPES.len());
    assert_eq!(smithing::BAR_TYPES[smithing::BLURITE].name, "BLURITE");

    // The plugin's allowed nodes and anvils.
    let java =
        std::fs::read_to_string(root.join("../../handlers/item/withobject/SmithingPlugin.java"))
            .unwrap();
    let line = java
        .lines()
        .find(|l| l.trim().starts_with("super("))
        .unwrap();
    let allowed: Vec<i32> = numbers(line).into_iter().map(|n| n as i32).collect();
    assert_eq!(allowed, smithing::ALLOWED);
    let anvils: Vec<i32> = java
        .lines()
        .filter(|l| l.contains("addHandler("))
        .map(|l| numbers(l)[0] as i32)
        .collect();
    assert_eq!(anvils, smithing::ANVILS);
}
