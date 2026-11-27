//! `world::item_on_object` against a recording host written from the pinned
//! `SapCollectListener.kt`, `PoisonFountainListener.kt`,
//! `HairdresserCheeseListener.kt`, `FishEasterEgg.kt` and
//! `GWDEntranceRopeListener.kt`: the calls each lambda and pulse makes, in
//! order, for every branch, and what `defineListeners` registers. There is
//! no JVM corpus for these classes (docs/item-on-object.md): this checks the
//! port against a reading of the sources, not against the original classes
//! running.
use rs09_server::world::{
    interaction_listeners::{self as interactions, Casing, Entry, Text},
    item_on_object::{self, Class, Error, Host, Interaction, Kind, Pulse, Result},
};

#[derive(Clone, Debug, PartialEq)]
enum Node {
    Player,
    Scenery(i32),
    /// An item with its id and slot.
    Item(i32, i32),
    /// A coal truck (2114) at x, y on plane 0.
    Truck(i32, i32),
}

#[derive(Default)]
struct Recorder {
    calls: Vec<String>,
    pulses: Vec<(Pulse<Node>, i32)>,
    /// The ids `inventory.contains` and `removeItem` find.
    held: Vec<i32>,
    /// `addItem` answers false.
    full: bool,
    killed: bool,
    /// The coal truck attribute, free slots and coal held.
    truck: i32,
    free: i32,
    coal: i32,
    /// The Seers task is complete; its requirement attribute is set.
    task_complete: bool,
    truck_full: bool,
    /// GlobalData: Luthas' task, the bananas in the crate, the swamp rope.
    luthas: bool,
    bananas: i32,
    rope: bool,
}

impl Host for Recorder {
    type Node = Node;
    fn kind(&self, node: &Node) -> Kind {
        match node {
            Node::Player => Kind::Player,
            Node::Scenery(_) | Node::Truck(..) => Kind::Scenery,
            Node::Item(..) => Kind::Item,
        }
    }
    fn slot(&mut self, item: &Node) -> Result<i32> {
        match item {
            Node::Item(_, slot) => Ok(*slot),
            _ => Err(Error::Cast),
        }
    }
    fn in_inventory(&mut self, _: &Node, id: i32) -> Result<bool> {
        Ok(self.held.contains(&id))
    }
    fn send_message(&mut self, _: &Node, text: &str) -> Result<()> {
        self.calls.push(format!("message {text}"));
        Ok(())
    }
    fn remove_item_id(&mut self, _: &Node, id: i32) -> Result<bool> {
        self.calls.push(format!("remove id {id}"));
        Ok(self.held.contains(&id))
    }
    fn remove_item(&mut self, _: &Node, item: &Node) -> Result<bool> {
        self.calls.push(format!("remove {item:?}"));
        match item {
            Node::Item(id, _) => Ok(self.held.contains(id)),
            // `removeItem<T>` of something that is neither an Item nor an Int.
            _ => Err(Error::State),
        }
    }
    fn add_item(&mut self, _: &Node, id: i32) -> Result<bool> {
        self.calls.push(format!("add {id}"));
        Ok(!self.full)
    }
    fn replace_slot(&mut self, _: &Node, slot: i32, id: i32) -> Result<()> {
        self.calls.push(format!("replace slot {slot} with {id}"));
        Ok(())
    }
    fn animate(&mut self, _: &Node, animation: i32) -> Result<()> {
        self.calls.push(format!("animate {animation}"));
        Ok(())
    }
    fn attribute(&mut self, _: &Node, key: &str, default: bool) -> Result<bool> {
        self.calls.push(format!("attribute {key} {default}"));
        Ok(if key == item_on_object::SEERS_COAL_TRUCK_FULL {
            self.truck_full
        } else {
            self.killed
        })
    }
    fn id(&mut self, node: &Node) -> Result<i32> {
        match node {
            Node::Scenery(id) | Node::Item(id, _) => Ok(*id),
            Node::Truck(..) => Ok(2114),
            Node::Player => Err(Error::Cast),
        }
    }
    fn location(&mut self, node: &Node) -> Result<[i32; 3]> {
        match node {
            Node::Truck(x, y) => Ok([*x, *y, 0]),
            _ => Ok([0, 0, 0]),
        }
    }
    fn attribute_int(&mut self, _: &Node, key: &str, default: i32) -> Result<i32> {
        self.calls.push(format!("attribute {key} {default}"));
        Ok(self.truck)
    }
    fn set_attribute_int(&mut self, _: &Node, key: &str, value: i32) -> Result<()> {
        self.calls.push(format!("set attribute {key} {value}"));
        Ok(())
    }
    fn remove_attribute(&mut self, _: &Node, key: &str) -> Result<()> {
        self.calls.push(format!("remove attribute {key}"));
        Ok(())
    }
    fn send_dialogue(&mut self, _: &Node, text: &str) -> Result<()> {
        self.calls.push(format!("dialogue {text}"));
        Ok(())
    }
    fn free_slots(&mut self, _: &Node) -> Result<i32> {
        Ok(self.free)
    }
    fn amount_in_inventory(&mut self, _: &Node, id: i32) -> Result<i32> {
        self.calls.push(format!("amount {id}"));
        Ok(self.coal)
    }
    fn add_items(&mut self, _: &Node, id: i32, amount: i32) -> Result<bool> {
        self.calls.push(format!("add {id} x{amount}"));
        Ok(!self.full)
    }
    fn remove_items(&mut self, _: &Node, id: i32, amount: i32) -> Result<bool> {
        self.calls.push(format!("remove {id} x{amount}"));
        Ok(!self.full)
    }
    fn seers_task_complete(&mut self, _: &Node, level: i32, index: i32) -> Result<bool> {
        self.calls.push(format!("seers complete {level} {index}"));
        Ok(self.task_complete)
    }
    fn seers_update_task(&mut self, _: &Node, level: i32, index: i32, done: bool) -> Result<()> {
        self.calls
            .push(format!("seers update {level} {index} {done}"));
        Ok(())
    }
    fn luthas_task(&mut self, _: &Node) -> Result<bool> {
        self.calls.push("luthas task".to_owned());
        Ok(self.luthas)
    }
    fn karamja_bananas(&mut self, _: &Node) -> Result<i32> {
        self.calls.push("bananas".to_owned());
        Ok(self.bananas)
    }
    fn set_karamja_bananas(&mut self, _: &Node, value: i32) -> Result<()> {
        self.calls.push(format!("set bananas {value}"));
        self.bananas = value;
        Ok(())
    }
    fn lumbridge_rope(&mut self, _: &Node) -> Result<bool> {
        self.calls.push("rope".to_owned());
        Ok(self.rope)
    }
    fn set_lumbridge_rope(&mut self, _: &Node, value: bool) -> Result<()> {
        self.calls.push(format!("set rope {value}"));
        Ok(())
    }
    fn send_item_dialogue(&mut self, _: &Node, item: &Node, text: &str) -> Result<()> {
        self.calls.push(format!("item dialogue {item:?} {text}"));
        Ok(())
    }
    fn finish_seers_task(&mut self, _: &Node, level: i32, index: i32) -> Result<()> {
        self.calls.push(format!("finish seers {level} {index}"));
        Ok(())
    }
    fn set_attribute(&mut self, _: &Node, key: &str, value: bool) -> Result<()> {
        self.calls.push(format!("set attribute {key} {value}"));
        Ok(())
    }
    fn lock(&mut self, _: &Node, ticks: i32) -> Result<()> {
        self.calls.push(format!("lock {ticks}"));
        Ok(())
    }
    fn unlock(&mut self, _: &Node) -> Result<()> {
        self.calls.push("unlock".to_owned());
        Ok(())
    }
    fn finish_falador_task(&mut self, _: &Node, level: i32, index: i32) -> Result<()> {
        self.calls.push(format!("finish falador {level} {index}"));
        Ok(())
    }
    fn set_varbit(&mut self, _: &Node, id: i32, value: i32, save: bool) -> Result<()> {
        self.calls.push(format!("varbit {id} {value} {save}"));
        Ok(())
    }
    fn run_woodcutting_pulse(&mut self, _: &Node, scenery: &Node) -> Result<()> {
        self.calls.push(format!("woodcutting pulse {scenery:?}"));
        Ok(())
    }
    fn run_pulse(&mut self, pulse: Pulse<Node>, delay: i32) -> Result<()> {
        self.calls.push(format!("pulse {} {delay}", pulse.class()));
        self.pulses.push((pulse, delay));
        Ok(())
    }
}

fn recorder(held: &[i32]) -> Recorder {
    Recorder {
        held: held.to_vec(),
        ..Recorder::default()
    }
}
fn take(h: &mut Recorder) -> Vec<String> {
    std::mem::take(&mut h.calls)
}

const SAP_PULSE: &str =
    "pulse content.global.handlers.item.withobject.SapCollectListener$defineListeners$1$1 2";

#[test]
fn sap_collecting_messages_without_a_bucket_and_always_submits_the_pulse() {
    let knife = Node::Item(946, 0);
    let tree = Node::Scenery(1276);
    // No bucket: the message, and the pulse all the same.
    let mut h = recorder(&[]);
    let run = item_on_object::run(
        &mut h,
        Interaction::SapCollect,
        &Node::Player,
        &knife,
        &tree,
    );
    assert_eq!(run, Ok(true));
    assert_eq!(
        take(&mut h),
        ["message You need a bucket to do that.", SAP_PULSE]
    );
    // Without a bucket the pulse never finishes: one removal per run.
    let (mut pulse, delay) = h.pulses.remove(0);
    assert_eq!(delay, 2);
    assert_eq!(item_on_object::pulse(&mut h, &mut pulse), Ok(false));
    assert_eq!(item_on_object::pulse(&mut h, &mut pulse), Ok(false));
    assert_eq!(take(&mut h), ["remove id 1925", "remove id 1925"]);

    // With a bucket: only the pulse; its run swaps the bucket.
    let mut h = recorder(&[1925]);
    let run = item_on_object::run(
        &mut h,
        Interaction::SapCollect,
        &Node::Player,
        &knife,
        &tree,
    );
    assert_eq!(run, Ok(true));
    assert_eq!(take(&mut h), [SAP_PULSE]);
    let (mut pulse, _) = h.pulses.remove(0);
    assert_eq!(
        pulse,
        Pulse::Sap {
            player: Node::Player
        }
    );
    assert_eq!(item_on_object::pulse(&mut h, &mut pulse), Ok(true));
    assert_eq!(
        take(&mut h),
        [
            "remove id 1925",
            "animate 2009",
            "add 4687",
            "message You cut the tree and allow its sap to drip down into your bucket.",
        ]
    );
    // A refused add keeps the animation, drops the message and still ends.
    h.full = true;
    assert_eq!(item_on_object::pulse(&mut h, &mut pulse), Ok(true));
    assert_eq!(take(&mut h), ["remove id 1925", "animate 2009", "add 4687"]);
}

#[test]
fn emptying_sap_replaces_the_slot() {
    let mut h = recorder(&[]);
    let sap = Node::Item(4687, 3);
    let run = item_on_object::run_option(&mut h, Interaction::SapEmpty, &Node::Player, &sap);
    assert_eq!(run, Ok(true));
    assert_eq!(
        take(&mut h),
        [
            "replace slot 3 with 1925",
            "message You empty the contents of the bucket on the floor.",
        ]
    );
    // A negative slot answers false with no call.
    let loose = Node::Item(4687, -1);
    let run = item_on_object::run_option(&mut h, Interaction::SapEmpty, &Node::Player, &loose);
    assert_eq!(run, Ok(false));
    assert!(take(&mut h).is_empty());
    // `node as Item`.
    let run =
        item_on_object::run_option(&mut h, Interaction::SapEmpty, &Node::Player, &Node::Player);
    assert_eq!(run, Err(Error::Cast));
    // The use-with entry point does not run an `on` lambda, nor the reverse.
    let run = item_on_object::run(&mut h, Interaction::SapEmpty, &Node::Player, &sap, &sap);
    assert_eq!(run, Err(Error::Cast));
    let run = item_on_object::run_option(&mut h, Interaction::GwdRope, &Node::Player, &sap);
    assert_eq!(run, Err(Error::Cast));
}

#[test]
fn the_fountain_is_poisoned_once() {
    let food = Node::Item(274, 0);
    let fountain = Node::Scenery(153);
    let run = |h: &mut Recorder| {
        item_on_object::run(
            h,
            Interaction::PoisonFountain,
            &Node::Player,
            &food,
            &fountain,
        )
    };
    // Already dead.
    let mut h = recorder(&[274]);
    h.killed = true;
    assert_eq!(run(&mut h), Ok(true));
    assert_eq!(
        take(&mut h),
        [
            "attribute piranhas-killed false",
            "message The piranhas are dead already.",
        ]
    );
    // The removal fails: false, nothing else.
    let mut h = recorder(&[]);
    assert_eq!(run(&mut h), Ok(false));
    assert_eq!(
        take(&mut h),
        [
            "attribute piranhas-killed false".to_owned(),
            format!("remove {food:?}"),
        ]
    );
    // The pour.
    let mut h = recorder(&[274]);
    assert_eq!(run(&mut h), Ok(true));
    assert_eq!(
        take(&mut h),
        [
            "attribute piranhas-killed false".to_owned(),
            format!("remove {food:?}"),
            "lock 5".to_owned(),
            "animate 881".to_owned(),
            "message You pour the poisoned fish food into the fountain.".to_owned(),
            "set attribute /save:piranhas-killed true".to_owned(),
            "pulse content.global.handlers.item.withobject.PoisonFountainListener$defineListeners$1$1 1"
                .to_owned(),
        ]
    );
    // counter 0: nothing; 1: the first line; 2: the second, the unlock, done.
    let (mut pulse, delay) = h.pulses.remove(0);
    assert_eq!(delay, 1);
    assert_eq!(item_on_object::pulse(&mut h, &mut pulse), Ok(false));
    assert!(take(&mut h).is_empty());
    assert_eq!(item_on_object::pulse(&mut h, &mut pulse), Ok(false));
    assert_eq!(
        take(&mut h),
        ["message The piranhas start eating the food..."]
    );
    assert_eq!(item_on_object::pulse(&mut h, &mut pulse), Ok(true));
    assert_eq!(
        take(&mut h),
        ["message ... then die and float to the surface.", "unlock"]
    );
}

#[test]
fn the_cheese_lambda_answers_false_and_its_task_runs_once() {
    let cheese = Node::Item(1985, 0);
    let treadmill = Node::Scenery(11677);
    let mut h = recorder(&[1985]);
    let run = item_on_object::run(
        &mut h,
        Interaction::HairdresserCheese,
        &Node::Player,
        &cheese,
        &treadmill,
    );
    // `isTaskSuccessful` is read before the task runs.
    assert_eq!(run, Ok(false));
    assert_eq!(
        take(&mut h),
        ["lock 5", "pulse core.api.ContentAPIKt$runTask$1 3"]
    );
    let (mut pulse, delay) = h.pulses.remove(0);
    assert_eq!(delay, 3);
    assert_eq!(item_on_object::pulse(&mut h, &mut pulse), Ok(true));
    assert_eq!(
        take(&mut h),
        [
            format!("remove {cheese:?}"),
            "message You throw the cheese to Ridgeley, for which he appears grateful.".to_owned(),
            "finish falador 0 6".to_owned(),
            "unlock".to_owned(),
        ]
    );
    // The cheese is gone when the task runs: the task returns early (no
    // unlock: the five-tick lock runs out by itself) and the pulse ends.
    let mut h = recorder(&[]);
    let mut pulse = Pulse::Cheese {
        player: Node::Player,
        used: cheese.clone(),
        cycles: 1,
    };
    assert_eq!(item_on_object::pulse(&mut h, &mut pulse), Ok(true));
    assert_eq!(take(&mut h), [format!("remove {cheese:?}")]);
}

#[test]
fn herring_jokes_and_the_god_wars_rope() {
    let herring = Node::Item(347, 0);
    let mut h = recorder(&[954]);
    let run = item_on_object::run(
        &mut h,
        Interaction::FishOnTree,
        &Node::Player,
        &herring,
        &Node::Scenery(1276),
    );
    assert_eq!(run, Ok(true));
    assert_eq!(
        take(&mut h),
        ["message This is not the mightiest tree in the forest."]
    );
    let run = item_on_object::run(
        &mut h,
        Interaction::FishOnDoor,
        &Node::Player,
        &herring,
        &Node::Scenery(1967),
    );
    assert_eq!(run, Ok(true));
    assert_eq!(take(&mut h), ["message It can't be done!"]);

    let rope = Node::Item(954, 5);
    let hole = Node::Scenery(26340);
    let run = item_on_object::run(&mut h, Interaction::GwdRope, &Node::Player, &rope, &hole);
    assert_eq!(run, Ok(true));
    assert_eq!(
        take(&mut h),
        [format!("remove {rope:?}"), "varbit 3932 1 true".to_owned()]
    );
    let mut h = recorder(&[]);
    let run = item_on_object::run(&mut h, Interaction::GwdRope, &Node::Player, &rope, &hole);
    assert_eq!(run, Ok(false));
    assert_eq!(take(&mut h), [format!("remove {rope:?}")]);
}

#[test]
fn an_axe_on_a_tree_starts_the_woodcutting_pulse() {
    let axe = Node::Item(1351, 0);
    let tree = Node::Scenery(1276);
    let mut h = recorder(&[]);
    let run = item_on_object::run(&mut h, Interaction::AxeOnTree, &Node::Player, &axe, &tree);
    assert_eq!(run, Ok(true));
    assert_eq!(take(&mut h), [format!("woodcutting pulse {tree:?}")]);
    // `with.asScenery()`.
    let run = item_on_object::run(&mut h, Interaction::AxeOnTree, &Node::Player, &axe, &axe);
    assert_eq!(run, Err(Error::Cast));
    assert!(take(&mut h).is_empty());
}

#[test]
fn coal_goes_into_the_truck_up_to_its_capacity() {
    let truck = Node::Truck(2696, 3504);
    let coal = Node::Item(453, 0);
    let deposit = |h: &mut Recorder| {
        item_on_object::run(h, Interaction::CoalDeposit, &Node::Player, &coal, &truck)
    };
    // 10 in the truck, 5 held: all five, no message.
    let mut h = recorder(&[]);
    (h.truck, h.coal) = (10, 5);
    assert_eq!(deposit(&mut h), Ok(true));
    assert_eq!(
        take(&mut h),
        [
            "attribute coal-truck-inventory 0",
            "amount 453",
            "remove 453 x5",
            "set attribute /save:coal-truck-inventory 15",
        ]
    );
    // 110 in the truck, 27 held: ten, with the message first.
    (h.truck, h.coal) = (110, 27);
    assert_eq!(deposit(&mut h), Ok(true));
    assert_eq!(
        take(&mut h),
        [
            "attribute coal-truck-inventory 0",
            "amount 453",
            "message You have filled up the coal truck.",
            "remove 453 x10",
            "set attribute /save:coal-truck-inventory 120",
        ]
    );
    // A refused removal leaves the attribute.
    (h.truck, h.coal, h.full) = (0, 3, true);
    assert_eq!(deposit(&mut h), Ok(true));
    assert_eq!(
        take(&mut h),
        [
            "attribute coal-truck-inventory 0",
            "amount 453",
            "remove 453 x3"
        ]
    );
}

#[test]
fn coal_comes_out_of_the_truck_and_the_truck_reports_its_load() {
    let seers = Node::Truck(2698, 3507);
    let mine = Node::Truck(2581, 3481);
    let remove = |h: &mut Recorder, truck: &Node| {
        item_on_object::run_option(h, Interaction::CoalRemove, &Node::Player, truck)
    };
    // Empty.
    let mut h = recorder(&[]);
    assert_eq!(remove(&mut h, &seers), Ok(true));
    assert_eq!(
        take(&mut h),
        [
            "attribute coal-truck-inventory 0",
            "dialogue The coal truck is empty."
        ]
    );
    // 30 in the truck, 28 free slots: 28 leave; the truck was not full, so
    // the diary reads nothing.
    (h.truck, h.free) = (30, 28);
    assert_eq!(remove(&mut h, &seers), Ok(true));
    assert_eq!(
        take(&mut h),
        [
            "attribute coal-truck-inventory 0",
            "add 453 x28",
            "set attribute /save:coal-truck-inventory 2",
        ]
    );
    // A full truck at Seers' Village with the requirement set: the task.
    (h.truck, h.free, h.truck_full) = (120, 5, true);
    assert_eq!(remove(&mut h, &seers), Ok(true));
    assert_eq!(
        take(&mut h),
        [
            "attribute coal-truck-inventory 0",
            "add 453 x5",
            "set attribute /save:coal-truck-inventory 115",
            "attribute diary:seers:coal-truck-full false",
            "finish seers 1 2",
            "remove attribute diary:seers:coal-truck-full",
        ]
    );
    // The same at the mine's truck: not a Seers' Village truck.
    assert_eq!(remove(&mut h, &mine), Ok(true));
    assert_eq!(
        take(&mut h),
        [
            "attribute coal-truck-inventory 0",
            "add 453 x5",
            "set attribute /save:coal-truck-inventory 115",
        ]
    );
    // No free slot: `addItem` of nothing still writes the attribute, and
    // the diary is not asked.
    (h.truck, h.free) = (120, 0);
    assert_eq!(remove(&mut h, &seers), Ok(true));
    assert_eq!(
        take(&mut h),
        [
            "attribute coal-truck-inventory 0",
            "add 453 x0",
            "set attribute /save:coal-truck-inventory 120",
        ]
    );
    // Investigate.
    h.truck = 37;
    let run =
        item_on_object::run_option(&mut h, Interaction::CoalInvestigate, &Node::Player, &seers);
    assert_eq!(run, Ok(true));
    assert_eq!(
        take(&mut h),
        [
            "attribute coal-truck-inventory 0",
            "dialogue There is currently 37 coal in the truck. The truck has space for 83 more coal.",
        ]
    );
}

#[test]
fn a_full_truck_fulfils_the_seers_requirement_once() {
    let set = |h: &mut Recorder, key: &str, value: i32| {
        item_on_object::seers_attribute_set(h, &Node::Player, key, value)
    };
    let mut h = recorder(&[]);
    // Another key, or fewer than 120: nothing.
    assert_eq!(set(&mut h, "/save:piranhas-killed", 120), Ok(()));
    assert_eq!(set(&mut h, "/save:coal-truck-inventory", 119), Ok(()));
    assert!(take(&mut h).is_empty());
    assert_eq!(set(&mut h, "/save:coal-truck-inventory", 120), Ok(()));
    assert_eq!(
        take(&mut h),
        [
            "seers complete 1 2",
            "attribute diary:seers:coal-truck-full false",
            "seers update 1 2 false",
            "set attribute /save:diary:seers:coal-truck-full true",
        ]
    );
    // Already fulfilled, or the task complete: nothing more.
    h.truck_full = true;
    assert_eq!(set(&mut h, "/save:coal-truck-inventory", 120), Ok(()));
    assert_eq!(
        take(&mut h),
        [
            "seers complete 1 2",
            "attribute diary:seers:coal-truck-full false"
        ]
    );
    h.task_complete = true;
    assert_eq!(set(&mut h, "/save:coal-truck-inventory", 120), Ok(()));
    assert_eq!(take(&mut h), ["seers complete 1 2"]);
}

#[test]
fn the_banana_crate_needs_luthas_task_and_holds_ten() {
    let crate_ = Node::Scenery(2072);
    let rum = Node::Item(431, 0);
    let banana = Node::Item(1963, 1);
    let use_on = |h: &mut Recorder, f: Interaction, item: &Node| {
        item_on_object::run(h, f, &Node::Player, item, &crate_)
    };
    let option =
        |h: &mut Recorder, f: Interaction| item_on_object::run_option(h, f, &Node::Player, &crate_);
    const NO_TASK: [&str; 2] = ["luthas task", "message I don't know what goes in there."];

    // Without the task every lambda says the same and answers true.
    let mut h = recorder(&[431, 1963]);
    assert_eq!(use_on(&mut h, Interaction::CrateRum, &rum), Ok(true));
    assert_eq!(take(&mut h), NO_TASK);
    assert_eq!(use_on(&mut h, Interaction::CrateBanana, &banana), Ok(true));
    assert_eq!(take(&mut h), NO_TASK);
    assert_eq!(option(&mut h, Interaction::CrateSearch), Ok(true));
    assert_eq!(take(&mut h), NO_TASK);
    assert_eq!(option(&mut h, Interaction::CrateFill), Ok(true));
    assert_eq!(take(&mut h), NO_TASK);

    // The rum.
    h.luthas = true;
    assert_eq!(use_on(&mut h, Interaction::CrateRum, &rum), Ok(true));
    assert_eq!(
        take(&mut h),
        [
            "luthas task",
            "attribute stashed-rum false",
            "remove Item(431, 0)",
            "lock 2",
            "animate 832",
            "dialogue You stash the rum in the crate.",
            "set attribute /save:stashed-rum true",
        ]
    );
    h.killed = true; // the recorder's boolean attribute
    assert_eq!(use_on(&mut h, Interaction::CrateRum, &rum), Ok(true));
    assert_eq!(
        take(&mut h),
        [
            "luthas task",
            "attribute stashed-rum false",
            "message There's already some rum in here...",
        ]
    );
    h.killed = false;
    h.held.clear();
    assert_eq!(use_on(&mut h, Interaction::CrateRum, &rum), Ok(false));
    assert_eq!(
        take(&mut h),
        [
            "luthas task",
            "attribute stashed-rum false",
            "remove Item(431, 0)"
        ]
    );

    // One banana.
    h.held = vec![1963];
    assert_eq!(use_on(&mut h, Interaction::CrateBanana, &banana), Ok(true));
    assert_eq!(
        take(&mut h),
        [
            "luthas task",
            "bananas",
            "remove Item(1963, 1)",
            "lock 2",
            "animate 832",
            "dialogue You pack a banana into the crate.",
            "set bananas 1",
        ]
    );
    assert_eq!(option(&mut h, Interaction::CrateSearch), Ok(true));
    assert_eq!(
        take(&mut h),
        [
            "luthas task",
            "bananas",
            "message The crate has 1 banana inside."
        ]
    );
    h.bananas = 4;
    assert_eq!(option(&mut h, Interaction::CrateSearch), Ok(true));
    assert_eq!(
        take(&mut h),
        [
            "luthas task",
            "bananas",
            "message The crate has 4 bananas inside."
        ]
    );
    h.bananas = 0;
    assert_eq!(option(&mut h, Interaction::CrateSearch), Ok(true));
    assert_eq!(
        take(&mut h),
        [
            "luthas task",
            "bananas",
            "message The crate is completely empty."
        ]
    );

    // Fill: seven held and four in the crate packs six.
    (h.bananas, h.coal) = (4, 7); // `coal` is the recorder's amount held
    assert_eq!(option(&mut h, Interaction::CrateFill), Ok(true));
    assert_eq!(
        take(&mut h),
        [
            "luthas task",
            "amount 1963",
            "bananas",
            "bananas",
            "remove 1963 x6",
            "lock 2",
            "animate 832",
            "message You pack all your bananas into the crate.",
            "bananas",
            "set bananas 10",
        ]
    );
    // Full.
    assert_eq!(option(&mut h, Interaction::CrateFill), Ok(true));
    assert_eq!(
        take(&mut h),
        [
            "luthas task",
            "amount 1963",
            "bananas",
            "bananas",
            "message The crate is already full.",
        ]
    );
    assert_eq!(option(&mut h, Interaction::CrateSearch), Ok(true));
    assert_eq!(
        take(&mut h),
        [
            "luthas task",
            "bananas",
            "message The crate is full of bananas."
        ]
    );
    assert_eq!(use_on(&mut h, Interaction::CrateBanana, &banana), Ok(true));
    assert_eq!(
        take(&mut h),
        [
            "luthas task",
            "bananas",
            "message The crate is already full."
        ]
    );
    // No banana held: nothing is said, even for a full crate.
    h.coal = 0;
    assert_eq!(option(&mut h, Interaction::CrateFill), Ok(true));
    assert_eq!(
        take(&mut h),
        ["luthas task", "amount 1963", "bananas", "bananas"]
    );
    // A refused removal answers false.
    (h.bananas, h.coal, h.full) = (0, 3, true);
    assert_eq!(option(&mut h, Interaction::CrateFill), Ok(false));
    assert_eq!(
        take(&mut h),
        [
            "luthas task",
            "amount 1963",
            "bananas",
            "bananas",
            "remove 1963 x3"
        ]
    );
}

#[test]
fn the_swamp_hole_takes_one_rope() {
    let hole = Node::Scenery(5947);
    let rope = Node::Item(954, 3);
    let tie = |h: &mut Recorder| {
        item_on_object::run(h, Interaction::SwampRope, &Node::Player, &rope, &hole)
    };
    let mut h = recorder(&[954]);
    assert_eq!(tie(&mut h), Ok(true));
    assert_eq!(
        take(&mut h),
        [
            "rope",
            "remove Item(954, 3)",
            "item dialogue Item(954, 3) You tie the rope to the top of the entrance and throw it down.",
            "set rope true",
        ]
    );
    h.held.clear();
    assert_eq!(tie(&mut h), Ok(false));
    assert_eq!(take(&mut h), ["rope", "remove Item(954, 3)"]);
    h.rope = true;
    assert_eq!(tie(&mut h), Ok(true));
    assert_eq!(
        take(&mut h),
        [
            "rope",
            "dialogue There is already a rope tied to the entrance."
        ]
    );
}

struct Lower;
impl Casing for Lower {
    fn lowercase_default(&self, s: &[u16]) -> Text {
        String::from_utf16_lossy(s)
            .to_lowercase()
            .encode_utf16()
            .collect()
    }
    fn lowercase_root(&self, s: &[u16]) -> Text {
        self.lowercase_default(s)
    }
}

#[test]
fn define_listeners_registers_the_rows() {
    let registry = interactions::Registry::default();
    for class in Class::ALL {
        assert_eq!(Class::named(class.name()), Some(class));
        item_on_object::define(&registry, &Lower, class).unwrap();
    }
    assert_eq!(
        Class::named("content.global.handlers.item.withobject.SpiritShieldBlessListener"),
        None
    );
    let tables = registry.inspect();
    let table = |name: &str| -> Vec<(String, Interaction)> {
        tables
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, rows)| {
                rows.iter()
                    .map(|(key, entry)| {
                        let Entry::Function(f) = entry else {
                            panic!("a function row");
                        };
                        (
                            String::from_utf16_lossy(key),
                            *f.value::<Interaction>().unwrap(),
                        )
                    })
                    .collect()
            })
            .unwrap()
    };
    let scenery = interactions::Kind::Scenery.ordinal();
    let item = interactions::Kind::Item.ordinal();
    // on(BUCKET_OF_SAP_4687, ITEM, "empty"): "id:type:option".
    let mut options = table("listeners");
    options.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        options,
        [
            (format!("2072:{scenery}:fill"), Interaction::CrateFill),
            (format!("2072:{scenery}:search"), Interaction::CrateSearch),
            (
                format!("2114:{scenery}:investigate"),
                Interaction::CoalInvestigate
            ),
            (
                format!("2114:{scenery}:remove-coal"),
                Interaction::CoalRemove
            ),
            (format!("4687:{item}:empty"), Interaction::SapEmpty),
        ]
    );
    let uses = table("useWithListeners");
    let class_of = |used: i32, with: i32| -> Option<Interaction> {
        let key = format!("{used}:{with}:{scenery}");
        uses.iter().find(|(k, _)| *k == key).map(|(_, c)| *c)
    };
    for tree in item_on_object::SAP_TREES {
        assert_eq!(class_of(946, tree), Some(Interaction::SapCollect));
    }
    assert_eq!(class_of(274, 153), Some(Interaction::PoisonFountain));
    assert_eq!(class_of(1985, 11677), Some(Interaction::HairdresserCheese));
    assert_eq!(class_of(954, 26340), Some(Interaction::GwdRope));
    assert_eq!(class_of(453, 2114), Some(Interaction::CoalDeposit));
    assert_eq!(class_of(431, 2072), Some(Interaction::CrateRum));
    assert_eq!(class_of(1963, 2072), Some(Interaction::CrateBanana));
    assert_eq!(class_of(954, 5947), Some(Interaction::SwampRope));
    let trees = item_on_object::woodcutting_ids();
    assert!(trees.contains(&1276) && trees.len() > 50);
    for axe in item_on_object::AXES {
        for tree in &trees {
            assert_eq!(class_of(axe, *tree), Some(Interaction::AxeOnTree));
        }
    }
    for fish in item_on_object::HERRINGS {
        for tree in &trees {
            assert_eq!(class_of(fish, *tree), Some(Interaction::FishOnTree));
        }
        for door in item_on_object::DOORS {
            assert_eq!(class_of(fish, door), Some(Interaction::FishOnDoor));
        }
    }
    let mut distinct = trees.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(
        uses.len(),
        27 + 7 + 2 * (distinct.len() + 2) + 9 * distinct.len()
    );
    // A second SapCollectListener is the original's IllegalStateException.
    assert!(matches!(
        item_on_object::define(&registry, &Lower, Class::SapCollect),
        Err(Error::Binding(_))
    ));
}

/// The sap trees against the pinned source, where the reference checkout is
/// present (it is not committed).
#[test]
fn sap_trees_match_the_source() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../reference/server/Server/src/main/content/global/handlers/item/withobject/SapCollectListener.kt"
    );
    let Ok(source) = std::fs::read_to_string(path) else {
        return;
    };
    let ids: Vec<i32> = source
        .lines()
        .filter_map(|line| line.trim().strip_prefix("Scenery."))
        .filter_map(|name| name.trim_end_matches(',').rsplit('_').next()?.parse().ok())
        .collect();
    assert_eq!(ids, item_on_object::SAP_TREES);
}
