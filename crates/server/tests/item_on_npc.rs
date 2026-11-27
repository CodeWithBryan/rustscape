//! `world::item_on_npc` against a recording host written from the pinned
//! `FeedOnStrayDog.kt` and `FeedFaladorSheepdog.kt`: the calls each lambda
//! makes, in order, for every branch, and what `defineListeners` registers.
//! There is no JVM corpus for these two classes (docs/item-on-npc.md): this
//! checks the port against a reading of the sources, not against the
//! original classes running.
use rs09_server::world::{
    interaction_listeners::{self as interactions, Entry},
    item_on_npc::{self, Class, Error, Host, Interaction, Kind, Result},
};

#[derive(Clone, Debug, PartialEq)]
enum Node {
    Player,
    Npc(i32, &'static str),
    Item(i32, &'static str),
}

#[derive(Default)]
struct Recorder {
    calls: Vec<String>,
    /// `removeItem` answers false.
    refuse_removal: bool,
}

impl Host for Recorder {
    type Node = Node;
    fn kind(&self, node: &Node) -> Kind {
        match node {
            Node::Player => Kind::Player,
            Node::Npc(..) => Kind::Npc,
            Node::Item(..) => Kind::Item,
        }
    }
    fn id(&mut self, node: &Node) -> Result<i32> {
        match node {
            Node::Player => Err(Error::Cast),
            Node::Npc(id, _) | Node::Item(id, _) => Ok(*id),
        }
    }
    fn name(&mut self, node: &Node) -> Result<Option<String>> {
        self.definition_name(node)
    }
    fn definition_name(&mut self, node: &Node) -> Result<Option<String>> {
        match node {
            Node::Player => Err(Error::Cast),
            Node::Npc(_, name) | Node::Item(_, name) => Ok(Some((*name).to_owned())),
        }
    }
    fn lowercase_default(&self, text: &str) -> String {
        text.to_lowercase()
    }
    fn lowercase_root(&self, text: &str) -> String {
        text.to_lowercase()
    }
    fn remove_item(&mut self, _: &Node, item: &Node) -> Result<bool> {
        self.calls.push(format!("remove {item:?}"));
        Ok(!self.refuse_removal)
    }
    fn send_message(&mut self, _: &Node, text: &str) -> Result<()> {
        self.calls.push(format!("message {text}"));
        Ok(())
    }
    fn send_dialogue(&mut self, _: &Node, text: &str) -> Result<()> {
        self.calls.push(format!("dialogue {text}"));
        Ok(())
    }
    fn animate(&mut self, entity: &Node, animation: i32) -> Result<()> {
        let who = match entity {
            Node::Player => "player",
            _ => "npc",
        };
        self.calls.push(format!("animate {who} {animation}"));
        Ok(())
    }
    fn send_chat(&mut self, _: &Node, text: &str) -> Result<()> {
        self.calls.push(format!("chat {text}"));
        Ok(())
    }
}

fn run(
    f: Interaction,
    used: Node,
    with: Node,
    refuse_removal: bool,
) -> (Result<bool>, Vec<String>) {
    let mut host = Recorder {
        refuse_removal,
        ..Default::default()
    };
    let result = item_on_npc::run(&mut host, f, &Node::Player, &used, &with);
    (result, host.calls)
}

const DOG: Node = Node::Npc(5917, "Stray dog");
const SHEEPDOG: Node = Node::Npc(2311, "Sheepdog");

#[test]
fn stray_dog_takes_bones_by_kind() {
    let cases: [(i32, &str, Option<&str>, &str); 6] = [
        (526, "Bones", None, "Woof"),
        (
            528,
            "Burnt bones",
            Some("The dog looks at you, disappointed, but takes the bones anyway."),
            "Woof...",
        ),
        (
            4832,
            "Raurg bones",
            Some("The dog looks at you, confused, but takes the bones anyway."),
            "Woof..?",
        ),
        (
            532,
            "Big bones",
            Some("The dog seems to be very happy."),
            "Woof!",
        ),
        (
            536,
            "Dragon bones",
            Some("The dog seems to be extremely overjoyed."),
            "WOOF!",
        ),
        (530, "Bat bones", None, "Woof"),
    ];
    for (id, name, line, woof) in cases {
        let item = Node::Item(id, name);
        let (result, calls) = run(Interaction::StrayDog, item.clone(), DOG, false);
        assert_eq!(result, Ok(true), "{name}");
        let mut expected = vec![
            format!("remove {item:?}"),
            format!(
                "message You feed the stray dog your {}.",
                name.to_lowercase()
            ),
            "animate player 827".to_owned(),
        ];
        expected.extend(line.map(|l| format!("message {l}")));
        expected.push(format!("chat {woof}"));
        assert_eq!(calls, expected, "{name}");
    }
}

#[test]
fn stray_dog_wags_at_meat_and_ignores_a_failed_removal() {
    let meat = Node::Item(2142, "Cooked meat");
    let (result, calls) = run(Interaction::StrayDog, meat.clone(), DOG, false);
    assert_eq!(result, Ok(true));
    assert_eq!(
        calls,
        [
            format!("remove {meat:?}"),
            "message You feed the stray dog your cooked meat.".to_owned(),
            "animate player 827".to_owned(),
            "message The dog wagged its tail happily.".to_owned(),
            "animate npc 4777".to_owned(),
            "chat Woof woof!".to_owned(),
        ]
    );
    // The removal fails: nothing else happens, and the lambda still answers
    // true.
    let (result, calls) = run(Interaction::StrayDog, meat.clone(), DOG, true);
    assert_eq!(result, Ok(true));
    assert_eq!(calls, [format!("remove {meat:?}")]);
    // `used as Item` and `with as NPC` are casts.
    let (result, calls) = run(Interaction::StrayDog, DOG, DOG, false);
    assert_eq!(result, Err(Error::Cast));
    assert!(calls.is_empty());
}

#[test]
fn sheepdog_eats_bones_and_meat_and_growls_at_the_rest() {
    let bones = Node::Item(526, "Bones");
    let (result, calls) = run(Interaction::Sheepdog, bones.clone(), SHEEPDOG, false);
    assert_eq!(result, Ok(true));
    assert_eq!(
        calls,
        [
            format!("remove {bones:?}"),
            "dialogue You give the dog some nice bones. It happily gnaws on them.".to_owned(),
            "animate player 827".to_owned(),
            "chat Woof woof!".to_owned(),
        ]
    );
    let meat = Node::Item(2132, "Raw beef");
    let (result, calls) = run(Interaction::Sheepdog, meat.clone(), SHEEPDOG, false);
    assert_eq!(result, Ok(true));
    assert_eq!(
        calls,
        [
            format!("remove {meat:?}"),
            "dialogue You give the dog a nice piece of meat. It gobbles it up.".to_owned(),
            "animate player 827".to_owned(),
            "chat Woof woof!".to_owned(),
        ]
    );
    // Burnt bones are not food, nor is anything else.
    for other in [
        Node::Item(528, "Burnt bones"),
        Node::Item(1205, "Bronze dagger"),
    ] {
        let (result, calls) = run(Interaction::Sheepdog, other, SHEEPDOG, false);
        assert_eq!(result, Ok(true));
        assert_eq!(
            calls,
            [
                "message The dog doesn't seem interested in that.",
                "chat Grrrr!"
            ]
        );
    }
    // A failed removal ends the lambda before the animation.
    let (result, calls) = run(Interaction::Sheepdog, bones.clone(), SHEEPDOG, true);
    assert_eq!(result, Ok(true));
    assert_eq!(calls, [format!("remove {bones:?}")]);
}

#[test]
fn tables_follow_the_sources() {
    // Bones.values(): 31 constants, no id twice.
    let mut bones = item_on_npc::BONES.to_vec();
    bones.sort_unstable();
    bones.dedup();
    assert_eq!(bones.len(), 31);
    assert_eq!(item_on_npc::bones_array().len(), 31);
    // Meat: INEDIBLE_RAW and EDIBLE_COOKED rows, in declaration order.
    let meats = item_on_npc::consumable_meats();
    assert_eq!(meats.len(), 30);
    assert_eq!(&meats[..4], [1859, 1861, 2132, 2134]);
    // The special and burnt rows stay out (2140 is both cooked and burnt in
    // the original's table: the cooked row keeps it in).
    for id in [522, 2146, 7520, 9990, 12546] {
        assert!(!meats.contains(&id), "{id}");
    }
    assert!(meats.contains(&2140));
}

#[test]
fn define_registers_both_classes() {
    let registry = interactions::Registry::default();
    for name in [
        "content.global.handlers.item.withnpc.FeedFaladorSheepdog",
        "content.global.handlers.item.withnpc.FeedOnStrayDog",
    ] {
        let class = Class::named(name).expect(name);
        assert_eq!(class.name(), name);
        item_on_npc::define(&registry, class).unwrap();
    }
    let key = |entry: &(Vec<u16>, Entry)| String::from_utf16_lossy(&entry.0);
    let tables = registry.inspect();
    let table = |name: &str| -> Vec<String> {
        let mut keys: Vec<String> = tables
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, rows)| rows.iter().map(key).collect())
            .unwrap();
        keys.sort();
        keys
    };
    // onUseAnyWith(IntType.NPC, SHEEPDOG_2311): one row, "with:type".
    let npc = interactions::Kind::Npc.ordinal();
    assert_eq!(table("useAnyWithListeners"), [format!("2311:{npc}")]);
    // onUseWith(IntType.NPC, bones + meats, *dogs): a row per pair
    // ("used:with:type"); the meats' repeated ids share a row.
    let mut used = item_on_npc::bones_array();
    used.extend(item_on_npc::consumable_meats());
    used.sort_unstable();
    used.dedup();
    let rows = table("useWithListeners");
    assert_eq!(rows.len(), used.len() * item_on_npc::STRAY_DOGS.len());
    assert!(rows.contains(&format!("526:5917:{npc}")));
    assert!(rows.contains(&format!("2142:4766:{npc}")));
    assert!(!rows.contains(&format!("1205:5917:{npc}")));
}
