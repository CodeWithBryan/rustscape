//! `world::cooking` against a recording host written from the pinned
//! `CookingRewrite.kt`, `StandardCookingPulse.java` and its subclasses,
//! `CookingDialogue.kt` and `CookableItems.java`. There is no JVM corpus
//! (docs/cooking.md): this checks the port against a reading of the sources.
//! The table is also compared row by row with the text of
//! `CookableItems.java` when the reference checkout is present.
use rs09_server::world::cooking::{
    self, AmountInput, CookingPulse, Dialogue, Error, Host, Input, Kind, Result,
};
use std::collections::{HashMap, VecDeque};

#[derive(Clone, Debug, PartialEq)]
enum Node {
    Player,
    Item(i32),
    Scenery(i32, &'static str),
}

const FIRE: Node = Node::Scenery(2732, "Fire");
const RANGE: Node = Node::Scenery(2728, "Range");
const LUMBRIDGE: Node = Node::Scenery(114, "Cooking range");

struct Recorder {
    calls: Vec<String>,
    inventory: HashMap<i32, i32>,
    level: i32,
    tutorial: bool,
    quest: bool,
    gauntlets: bool,
    doubles: VecDeque<f64>,
    ints: VecDeque<i32>,
    full: bool,
    pulses: Vec<CookingPulse<Node>>,
    dialogues: Vec<Dialogue<Node>>,
    inputs: Vec<AmountInput<Node>>,
}

impl Recorder {
    fn new(items: &[(i32, i32)]) -> Self {
        Self {
            calls: Vec::new(),
            inventory: items.iter().copied().collect(),
            level: 99,
            tutorial: true,
            quest: false,
            gauntlets: false,
            doubles: VecDeque::new(),
            ints: VecDeque::new(),
            full: false,
            pulses: Vec::new(),
            dialogues: Vec::new(),
            inputs: Vec::new(),
        }
    }
    fn held(&self, id: i32) -> i32 {
        self.inventory.get(&id).copied().unwrap_or(0)
    }
    fn take(&mut self) -> Vec<String> {
        std::mem::take(&mut self.calls)
    }
}

fn name(id: i32) -> &'static str {
    match id {
        317 => "Raw shrimps",
        315 => "Shrimps",
        7954 => "Burnt shrimp",
        377 => "Raw lobster",
        2142 => "Cooked meat",
        2146 => "Burnt meat",
        2132 => "Raw beef",
        9436 => "Sinew",
        2321 => "Uncooked berry pie",
        2325 => "Redberry pie",
        2287 => "Uncooked pizza",
        2289 => "Plain pizza",
        2307 => "Bread dough",
        401 => "Seaweed",
        1781 => "Soda ash",
        1921 => "Bowl of water",
        4456 => "Bowl of hot water",
        7224 => "Skewered rabbit",
        7223 => "Roast rabbit",
        1889 => "Uncooked cake",
        1891 => "Cake",
        6006 => "Barley",
        6008 => "Barley malt",
        _ => "Item",
    }
}

impl Host for Recorder {
    type Node = Node;
    fn id(&mut self, node: &Node) -> Result<i32> {
        match node {
            Node::Player => Err(Error::Cast),
            Node::Item(id) | Node::Scenery(id, _) => Ok(*id),
        }
    }
    fn scenery_name(&mut self, scenery: &Node) -> Result<String> {
        match scenery {
            Node::Scenery(_, name) => Ok((*name).to_owned()),
            _ => Err(Error::Cast),
        }
    }
    fn scenery_active(&mut self, _: &Node) -> Result<bool> {
        Ok(true)
    }
    fn item_name(&mut self, id: i32) -> Result<String> {
        Ok(name(id).to_owned())
    }
    fn lowercase(&self, text: &str) -> String {
        text.to_lowercase()
    }
    fn tutorial_complete(&mut self, _: &Node) -> Result<bool> {
        Ok(self.tutorial)
    }
    fn tutorial_cook(&mut self, _: &Node, used: &Node) -> Result<()> {
        self.calls.push(format!("tutorial {used:?}"));
        Ok(())
    }
    fn amount_in_inventory(&mut self, _: &Node, id: i32) -> Result<i32> {
        Ok(self.held(id))
    }
    fn inventory_amount(&mut self, _: &Node, id: i32) -> Result<i32> {
        Ok(self.held(id))
    }
    fn inventory_add(&mut self, _: &Node, id: i32) -> Result<bool> {
        self.calls.push(format!("add {id}"));
        if self.full {
            return Ok(false);
        }
        *self.inventory.entry(id).or_default() += 1;
        Ok(true)
    }
    fn inventory_remove(&mut self, _: &Node, id: i32) -> Result<bool> {
        self.calls.push(format!("remove {id}"));
        if self.held(id) == 0 {
            return Ok(false);
        }
        *self.inventory.get_mut(&id).unwrap() -= 1;
        Ok(true)
    }
    fn ground_create(&mut self, _: &Node, id: i32, at_location: bool) -> Result<()> {
        self.calls.push(format!("ground {id} {at_location}"));
        Ok(())
    }
    fn equipment_contains(&mut self, _: &Node, id: i32) -> Result<bool> {
        Ok(self.gauntlets && id == 775)
    }
    fn send_message(&mut self, _: &Node, text: Option<&str>) -> Result<()> {
        self.calls.push(format!("message {text:?}"));
        Ok(())
    }
    fn send_dialogue(&mut self, _: &Node, text: &str) -> Result<()> {
        self.calls.push(format!("dialogue {text}"));
        Ok(())
    }
    fn quest_complete(&mut self, _: &Node, quest: &str) -> Result<bool> {
        assert_eq!(quest, "Cook's Assistant");
        Ok(self.quest)
    }
    fn level(&mut self, _: &Node, skill: i32) -> Result<i32> {
        assert_eq!(skill, 7);
        Ok(self.level)
    }
    fn hasty_cooking(&mut self, _: &Node) -> Result<bool> {
        Ok(false)
    }
    fn random_double(&mut self, max: f64) -> Result<f64> {
        assert_eq!(max, 100.0);
        self.calls.push("randomDouble".into());
        Ok(self.doubles.pop_front().unwrap_or(0.0))
    }
    fn random(&mut self, max: i32) -> Result<i32> {
        self.calls.push(format!("random {max}"));
        Ok(self.ints.pop_front().unwrap_or(0))
    }
    fn resource_produced(
        &mut self,
        _: &Node,
        item: i32,
        amount: i32,
        _: &Node,
        original: i32,
    ) -> Result<()> {
        self.calls
            .push(format!("produced {item} {amount} from {original}"));
        Ok(())
    }
    fn add_experience(&mut self, _: &Node, skill: i32, experience: f64) -> Result<()> {
        self.calls.push(format!("xp {skill} {experience}"));
        Ok(())
    }
    fn dupe_alert(&mut self, _: &Node, message: &str) -> Result<()> {
        self.calls.push(format!("dupe {message}"));
        Ok(())
    }
    fn player_name(&mut self, _: &Node) -> Result<String> {
        Ok("cook".into())
    }
    fn increment_attribute(&mut self, _: &Node, key: &str, amount: i32) -> Result<()> {
        self.calls.push(format!("increment {key} {amount}"));
        Ok(())
    }
    fn play_audio(&mut self, _: &Node, id: i32) -> Result<()> {
        self.calls.push(format!("audio {id}"));
        Ok(())
    }
    fn animate_high(&mut self, _: &Node, id: i32) -> Result<()> {
        self.calls.push(format!("animate {id}"));
        Ok(())
    }
    fn run_pulse(&mut self, _: &Node, mut pulse: CookingPulse<Node>) -> Result<()> {
        self.calls.push(format!("run {:?}", pulse.kind));
        pulse.start(self)?;
        // PulseManager.run submits while the pulse is running.
        if pulse.running {
            self.pulses.push(pulse);
        }
        Ok(())
    }
    fn open_dialogue(&mut self, player: &Node, mut dialogue: Dialogue<Node>) -> Result<()> {
        self.calls.push("open CookingDialogue".into());
        // DialogueInterpreter.open(file): the first handle.
        dialogue.handle(self, player, 0)?;
        self.dialogues.push(dialogue);
        Ok(())
    }
    fn dialogue_options(&mut self, _: &Node, lines: &[&str]) -> Result<()> {
        self.calls.push(format!("options {lines:?}"));
        Ok(())
    }
    fn dialogue_end(&mut self, _: &Node) -> Result<()> {
        self.calls.push("end".into());
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
    fn reposition_child(&mut self, _: &Node, i: i32, c: i32, x: i32, y: i32) -> Result<()> {
        self.calls.push(format!("reposition {i}:{c} {x},{y}"));
        Ok(())
    }
    fn open_chatbox(&mut self, _: &Node, interface: i32) -> Result<()> {
        self.calls.push(format!("chatbox {interface}"));
        Ok(())
    }
    fn send_input(&mut self, _: &Node, prompt: &str, handler: AmountInput<Node>) -> Result<()> {
        self.calls.push(format!("input {prompt}"));
        self.inputs.push(handler);
        Ok(())
    }
}

/// Runs the submitted pulse until it finishes: how many `pulse()` calls.
fn drain(h: &mut Recorder) -> usize {
    let mut pulse = h.pulses.pop().expect("a submitted pulse");
    for ticks in 1..100 {
        if pulse.pulse(h).unwrap() {
            return ticks;
        }
    }
    panic!("the pulse never finished");
}

fn cooked(xp: &str, raw: i32, product: i32, message: &str, animation: i32) -> Vec<String> {
    vec![
        format!("remove {raw}"),
        format!("add {product}"),
        format!("produced {product} 1 from {raw}"),
        format!("xp 7 {xp}"),
        "increment /save:stats_manager:food_cooked 1".into(),
        format!("message Some({message:?})"),
        "audio 2577".into(),
        format!("animate {animation}"),
    ]
}

#[test]
fn one_shrimp_on_a_fire_cooks_at_once() {
    let mut h = Recorder::new(&[(317, 1)]);
    let result = cooking::use_with(&mut h, &Node::Player, &Node::Item(317), &FIRE);
    assert_eq!(result, Ok(true));
    let mut expected = vec!["run Standard".to_owned(), "randomDouble".to_owned()];
    expected.extend(cooked(
        "30",
        317,
        315,
        "You manage to cook some shrimps.",
        897,
    ));
    assert_eq!(h.take(), expected);
    assert_eq!((h.held(317), h.held(315)), (0, 1));
    // The pulse was submitted; its first run finds nothing left to cook.
    assert_eq!(drain(&mut h), 1);
    assert!(h.take().is_empty());
}

#[test]
fn a_failed_roll_burns_the_food() {
    let mut h = Recorder::new(&[(317, 1)]);
    h.level = 1;
    // Level 1 on a fire: (128 + 0 + 1) / 256 = 50.39%.
    h.doubles.push_back(50.4);
    cooking::use_with(&mut h, &Node::Player, &Node::Item(317), &FIRE).unwrap();
    assert_eq!(
        h.take(),
        [
            "run Standard",
            "randomDouble",
            "remove 317",
            "produced 7954 1 from 317",
            "add 7954",
            "message Some(\"You accidentally burn some shrimps.\")",
            "audio 2577",
            "animate 897",
        ]
    );
    let mut h = Recorder::new(&[(317, 1)]);
    h.level = 1;
    h.doubles.push_back(50.3);
    cooking::use_with(&mut h, &Node::Player, &Node::Item(317), &FIRE).unwrap();
    assert_eq!(h.held(315), 1);
}

#[test]
fn several_open_the_dialogue_and_cook_all() {
    let mut h = Recorder::new(&[(317, 3)]);
    let result = cooking::use_with(&mut h, &Node::Player, &Node::Item(317), &RANGE);
    assert_eq!(result, Ok(true));
    assert_eq!(
        h.take(),
        [
            "open CookingDialogue",
            "zoom 317 160 307:2",
            "string <br><br><br><br>Raw shrimps 307:6",
            "reposition 307:0 12,15",
            "reposition 307:1 431,15",
            "reposition 307:7 0,12",
            "reposition 307:3 58,27",
            "reposition 307:4 58,27",
            "reposition 307:5 58,27",
            "reposition 307:6 58,27",
            "chatbox 307",
        ]
    );
    // Button 2: all of them.
    let mut dialogue = h.dialogues.pop().unwrap();
    assert_eq!((dialogue.stage, dialogue.product), (1, 315));
    dialogue.handle(&mut h, &Node::Player, 2).unwrap();
    let calls = h.take();
    assert_eq!(&calls[..4], ["end", "end", "run Standard", "randomDouble"]);
    assert_eq!(calls[calls.len() - 1], "animate 883");
    assert_eq!(h.held(315), 1);
    // The first run sets the delay (5 at a range), then one item a run.
    let mut pulse = h.pulses.pop().unwrap();
    assert_eq!((pulse.amount, pulse.delay), (2, 1));
    assert_eq!(pulse.pulse(&mut h), Ok(false));
    assert_eq!(pulse.delay, 5);
    assert_eq!(h.take(), ["randomDouble"]);
    assert_eq!(pulse.pulse(&mut h), Ok(false));
    assert_eq!(pulse.pulse(&mut h), Ok(true));
    assert_eq!((h.held(317), h.held(315)), (0, 3));
    assert_eq!(pulse.processed_amount, 3);
}

#[test]
fn dialogue_buttons_choose_the_amount() {
    for (button, amount) in [(5, 1), (4, 5)] {
        let mut h = Recorder::new(&[(317, 9)]);
        cooking::use_with(&mut h, &Node::Player, &Node::Item(317), &FIRE).unwrap();
        let mut dialogue = h.dialogues.pop().unwrap();
        dialogue.handle(&mut h, &Node::Player, button).unwrap();
        let pulse = h.pulses.pop().unwrap();
        assert_eq!(pulse.amount, amount - 1, "button {button}");
    }
    // Button 3 (and anything unknown) asks for the amount.
    let mut h = Recorder::new(&[(317, 9)]);
    cooking::use_with(&mut h, &Node::Player, &Node::Item(317), &FIRE).unwrap();
    let mut dialogue = h.dialogues.pop().unwrap();
    h.take();
    dialogue.handle(&mut h, &Node::Player, 3).unwrap();
    assert_eq!(h.take(), ["end", "input Enter the amount:"]);
    let input = h.inputs.pop().unwrap();
    assert!(
        input
            .run(&mut h, &Node::Player, &Input::Text("many".into()))
            .is_err()
    );
    input
        .run(&mut h, &Node::Player, &Input::Text("4".into()))
        .unwrap();
    assert_eq!(h.pulses.pop().unwrap().amount, 3);
    input.run(&mut h, &Node::Player, &Input::Int(2)).unwrap();
    assert_eq!(h.pulses.pop().unwrap().amount, 1);
}

#[test]
fn the_lumbridge_range_needs_the_quest_and_says_so_twice() {
    let mut h = Recorder::new(&[(317, 1)]);
    cooking::use_with(&mut h, &Node::Player, &Node::Item(317), &LUMBRIDGE).unwrap();
    let refusal = "message Some(\"You need to have completed the Cook's Assistant quest in order to use that range.\")";
    assert_eq!(h.take(), ["run Standard", refusal]);
    // `start()` did not run `super.start()`, but a new Pulse is running:
    // PulseManager.run submits it and its first run checks again.
    assert_eq!(drain(&mut h), 1);
    assert_eq!(h.take(), [refusal]);
    assert_eq!(h.held(317), 1);
    // With the quest the range has its own success values.
    let mut h = Recorder::new(&[(317, 1)]);
    h.quest = true;
    h.level = 1;
    // (138 + 0 + 1) / 256 = 54.29% at the Lumbridge range.
    h.doubles.push_back(54.2);
    cooking::use_with(&mut h, &Node::Player, &Node::Item(317), &LUMBRIDGE).unwrap();
    assert_eq!(h.held(315), 1);
}

#[test]
fn a_level_too_low_sends_a_dialogue() {
    let mut h = Recorder::new(&[(377, 1)]);
    h.level = 39;
    cooking::use_with(&mut h, &Node::Player, &Node::Item(377), &FIRE).unwrap();
    assert_eq!(
        h.take(),
        [
            "run Standard",
            "dialogue You need a cooking level of 40 to cook this."
        ]
    );
    assert_eq!(h.held(377), 1);
}

#[test]
fn gauntlets_change_the_lobster_roll() {
    // Level 40 lobster: 38..332 gives floor(38*59/98) + floor(332*39/98) + 1
    // = 22 + 132 + 1 = 155 -> 60.55%; with gauntlets 55..368 gives
    // 33 + 146 + 1 = 180 -> 70.31%.
    for (gauntlets, cooked) in [(false, 0), (true, 1)] {
        let mut h = Recorder::new(&[(377, 1)]);
        h.level = 40;
        h.gauntlets = gauntlets;
        h.doubles.push_back(65.0);
        cooking::use_with(&mut h, &Node::Player, &Node::Item(377), &FIRE).unwrap();
        assert_eq!(h.held(379), cooked, "gauntlets {gauntlets}");
    }
}

#[test]
fn cooked_meat_is_burnt_on_purpose() {
    let mut h = Recorder::new(&[(2142, 1)]);
    cooking::use_with(&mut h, &Node::Player, &Node::Item(2142), &FIRE).unwrap();
    assert_eq!(
        h.take(),
        [
            "run IntentionalBurn",
            "remove 2142",
            "add 2146",
            "message Some(\"You deliberately burn the perfectly good piece of meat.\")",
            "audio 2577",
            "animate 897",
        ]
    );
    assert_eq!(drain(&mut h), 1);
    // Three through the dialogue: the subclass counts with its own amount.
    let mut h = Recorder::new(&[(2142, 3)]);
    cooking::use_with(&mut h, &Node::Player, &Node::Item(2142), &FIRE).unwrap();
    let mut dialogue = h.dialogues.pop().unwrap();
    assert_eq!(dialogue.product, 2146);
    dialogue.handle(&mut h, &Node::Player, 2).unwrap();
    let pulse = h.pulses.last().unwrap();
    assert_eq!(
        (pulse.kind, pulse.amount, pulse.own_amount),
        (Kind::IntentionalBurn, 2, 3)
    );
    assert_eq!(drain(&mut h), 3);
    assert_eq!((h.held(2142), h.held(2146)), (0, 3));
}

#[test]
fn pies_and_pizzas_need_a_range() {
    for (raw, kind) in [(2321, "Pie"), (2287, "Pizza")] {
        let mut h = Recorder::new(&[(raw, 1)]);
        cooking::use_with(&mut h, &Node::Player, &Node::Item(raw), &FIRE).unwrap();
        let refusal = "message Some(\"This can only be cooked on a range.\")";
        assert_eq!(h.take(), [format!("run {kind}"), refusal.to_owned()]);
        assert_eq!(drain(&mut h), 1);
        assert_eq!(h.take(), [refusal]);
    }
    let mut h = Recorder::new(&[(2321, 1)]);
    cooking::use_with(&mut h, &Node::Player, &Node::Item(2321), &RANGE).unwrap();
    assert!(
        h.take()
            .contains(&"message Some(\"You successfully bake a delicious redberry pie.\")".into())
    );
    let mut h = Recorder::new(&[(2287, 1)]);
    h.level = 35;
    h.doubles.push_back(99.0);
    cooking::use_with(&mut h, &Node::Player, &Node::Item(2287), &RANGE).unwrap();
    assert!(
        h.take()
            .contains(&"message Some(\"You accidentally burn the pizza.\")".into())
    );
}

#[test]
fn range_only_foods_and_the_special_rows() {
    // Bread dough, cake and barley are refused away from a range: false.
    let mut h = Recorder::new(&[(2307, 1)]);
    let result = cooking::use_with(&mut h, &Node::Player, &Node::Item(2307), &FIRE);
    assert_eq!(result, Ok(false));
    assert_eq!(
        h.take(),
        ["message Some(\"You need to cook this on a range.\")"]
    );
    // Seaweed has no cooking row (its raw id is 0): no level, no roll.
    let mut h = Recorder::new(&[(401, 1)]);
    cooking::use_with(&mut h, &Node::Player, &Node::Item(401), &FIRE).unwrap();
    assert_eq!(
        h.take(),
        [
            "run IntentionalBurn",
            "remove 401",
            "add 1781",
            "message Some(\"You burn the seaweed into soda ash.\")",
            "audio 2577",
            "animate 897",
        ]
    );
    // Barley malt has no message; a bowl of water has no row and no xp.
    let mut h = Recorder::new(&[(6006, 1)]);
    cooking::use_with(&mut h, &Node::Player, &Node::Item(6006), &RANGE).unwrap();
    assert!(h.take().contains(&"message None".into()));
    let mut h = Recorder::new(&[(1921, 5)]);
    cooking::use_with(&mut h, &Node::Player, &Node::Item(1921), &FIRE).unwrap();
    assert_eq!(
        h.take(),
        [
            "run Standard",
            "remove 1921",
            "add 4456",
            "produced 4456 1 from 1921",
            "xp 7 0",
            "increment /save:stats_manager:food_cooked 1",
            "message None",
            "audio 2577",
            "animate 897",
        ]
    );
}

#[test]
fn raw_beef_at_a_range_offers_sinew() {
    let mut h = Recorder::new(&[(2132, 1)]);
    cooking::use_with(&mut h, &Node::Player, &Node::Item(2132), &RANGE).unwrap();
    assert_eq!(
        h.take(),
        [
            "open CookingDialogue",
            "options [\"Dry the meat into sinew\", \"Cook the meat\"]"
        ]
    );
    let mut dialogue = h.dialogues.pop().unwrap();
    assert_eq!(dialogue.stage, 101);
    dialogue.handle(&mut h, &Node::Player, 1).unwrap();
    // The pinned source runs a StandardCookingPulse with BEEF's row (raw
    // beef looks up BEEF, never SINEW; SinewCookingPulse is not used).
    let mut expected = vec![
        "end".to_owned(),
        "run Standard".to_owned(),
        "randomDouble".to_owned(),
    ];
    expected.extend(cooked(
        "30",
        2132,
        9436,
        "You manage to cook some beef.",
        883,
    ));
    assert_eq!(h.take(), expected);
    // Several: stage 100 shows the amount chatbox after the choice.
    let mut h = Recorder::new(&[(2132, 2)]);
    cooking::use_with(&mut h, &Node::Player, &Node::Item(2132), &RANGE).unwrap();
    let mut dialogue = h.dialogues.pop().unwrap();
    assert_eq!(dialogue.stage, 100);
    dialogue.handle(&mut h, &Node::Player, 2).unwrap();
    assert_eq!((dialogue.stage, dialogue.product), (1, 2142));
    // On a fire raw beef is ordinary food.
    let mut h = Recorder::new(&[(2132, 1)]);
    cooking::use_with(&mut h, &Node::Player, &Node::Item(2132), &FIRE).unwrap();
    assert_eq!(h.held(2142), 1);
}

#[test]
fn spits_and_cake_tins_come_back() {
    let mut h = Recorder::new(&[(7224, 1)]);
    h.ints.push_back(5);
    cooking::use_with(&mut h, &Node::Player, &Node::Item(7224), &FIRE).unwrap();
    let calls = h.take();
    assert_eq!(
        &calls[2..5],
        [
            "random 15",
            "message Some(\"Your iron spit seems to have broken in the process.\")",
            "remove 7224"
        ]
    );
    let mut h = Recorder::new(&[(7224, 1)]);
    h.ints.push_back(4);
    cooking::use_with(&mut h, &Node::Player, &Node::Item(7224), &FIRE).unwrap();
    assert_eq!(h.held(7225), 1);
    // A full inventory drops the cake tin for the player.
    let mut h = Recorder::new(&[(1889, 1)]);
    h.full = true;
    cooking::use_with(&mut h, &Node::Player, &Node::Item(1889), &RANGE).unwrap();
    let calls = h.take();
    assert_eq!(&calls[2..4], ["add 1887", "ground 1887 false"]);
}

#[test]
fn the_tutorial_branch_is_a_service() {
    let mut h = Recorder::new(&[(317, 1)]);
    h.tutorial = false;
    let result = cooking::use_with(&mut h, &Node::Player, &Node::Item(317), &FIRE);
    assert_eq!(result, Ok(true));
    assert_eq!(h.take(), ["tutorial Item(317)"]);
}

#[test]
fn table_lookups_follow_put_if_absent() {
    assert_eq!(cooking::COOKABLES.len(), 67);
    assert_eq!(cooking::for_id(2132).unwrap().name, "BEEF");
    assert_eq!(cooking::for_id(2138).unwrap().name, "CHICKEN");
    assert_eq!(cooking::for_id(0).unwrap().name, "RAW_OOMLIE");
    assert!(cooking::for_id(401).is_none());
    assert!(cooking::intentional_burn(2142));
    assert_eq!(cooking::intentional_burn_product(2142), Ok(2146));
    assert_eq!(cooking::intentional_burn_product(401), Ok(1781));
    assert!(!cooking::intentional_burn(317));
    assert_eq!(cooking::burnt(317), Ok(7954));
    assert_eq!(cooking::raw_foods().len(), 67 + 6);
    assert_eq!(cooking::COOKING_OBJECTS.len(), 110);
}

/// Every row against the text of the pinned `CookableItems.java` and
/// `CookingRewrite.kt` (skipped without the reference checkout).
#[test]
fn table_matches_the_pinned_source_text() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference/server/Server/src/main/content/global/skill/cooking");
    let Ok(java) = std::fs::read_to_string(root.join("CookableItems.java")) else {
        eprintln!("no reference checkout: CookableItems.java not compared");
        return;
    };
    // `Items.NAME_123` is 123.
    let number = |token: &str| -> f64 {
        let token = token.trim();
        match token.rsplit_once('_') {
            Some((head, id)) if head.starts_with("Items.") => id.parse().unwrap(),
            _ => token.parse().unwrap(),
        }
    };
    let body = &java[java.find('{').unwrap()..java.find("public final static").unwrap()];
    let mut rows = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        let Some(open) = line.find('(') else { continue };
        let name = &line[..open];
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_uppercase() || c == '_') {
            continue;
        }
        let close = line.rfind(')').unwrap();
        let values: Vec<f64> = line[open + 1..close].split(',').map(number).collect();
        rows.push((name.to_owned(), values));
    }
    assert_eq!(rows.len(), cooking::COOKABLES.len());
    for ((name, values), row) in rows.iter().zip(cooking::COOKABLES) {
        assert_eq!(name, row.name);
        let ours = [
            f64::from(row.cooked),
            f64::from(row.raw),
            f64::from(row.burnt),
            f64::from(row.level),
            row.experience,
            f64::from(row.low),
            f64::from(row.high),
            f64::from(row.low_range),
            f64::from(row.high_range),
        ];
        assert_eq!(values.as_slice(), ours, "{name}");
    }
    let table = |marker: &str| -> Vec<(i32, [i32; 2])> {
        java.lines()
            .filter(|line| line.contains(marker))
            .map(|line| {
                let inner = &line[line.find('(').unwrap() + 1..];
                let (id, rest) = inner.split_once(',').unwrap();
                let rest = &rest[rest.find('{').unwrap() + 1..rest.find('}').unwrap()];
                let (low, high) = rest.split_once(',').unwrap();
                (number(id) as i32, [number(low) as i32, number(high) as i32])
            })
            .collect()
    };
    assert_eq!(table("gauntletValues.put"), cooking::GAUNTLET_VALUES);
    assert_eq!(
        table("lumbridgeRangeValues.put"),
        cooking::LUMBRIDGE_RANGE_VALUES
    );
    let kotlin = std::fs::read_to_string(root.join("CookingRewrite.kt")).unwrap();
    let line = kotlin
        .lines()
        .find(|line| line.contains("val COOKING_OBJs"))
        .unwrap();
    let objects: Vec<i32> = line[line.find('(').unwrap() + 1..line.rfind(')').unwrap()]
        .split(',')
        .map(|n| n.trim().parse().unwrap())
        .collect();
    assert_eq!(objects, cooking::COOKING_OBJECTS);
}

#[test]
fn define_registers_every_pair() {
    use rs09_server::world::interaction_listeners as interactions;
    let registry = interactions::Registry::default();
    cooking::define(&registry).unwrap();
    let tables = registry.inspect();
    let rows = &tables
        .iter()
        .find(|(name, _)| *name == "useWithListeners")
        .unwrap()
        .1;
    let mut foods = cooking::raw_foods();
    foods.sort_unstable();
    foods.dedup();
    let mut objects = cooking::COOKING_OBJECTS.to_vec();
    objects.sort_unstable();
    objects.dedup();
    assert_eq!(rows.len(), foods.len() * objects.len());
    let scenery = interactions::Kind::Scenery.ordinal();
    let keys: Vec<String> = rows
        .iter()
        .map(|(k, _)| String::from_utf16_lossy(k))
        .collect();
    assert!(keys.contains(&format!("317:2732:{scenery}")));
    assert!(keys.contains(&format!("1921:114:{scenery}")));
}
