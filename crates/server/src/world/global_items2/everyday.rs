//! The everyday item options: emptying, dropping a lit light source,
//! caskets, bird nests, experience lamps, the starter pack, the imbued
//! rings, rune packs, the herb box, the newspaper and flyer, the cadava
//! potion and blamish oil.
use super::random::{self, ChanceItem};
use super::tables::{self, BirdNest};
use super::{
    Container, Error, Expression, Host, ItemArg, Kind, Object, Result, Store, Task, Value, as_item,
    is_plus_n,
};
use crate::world::dark_zone::LightSource;
use crate::world::drop_tables::{self, Context, Looter, WeightBasedTable, WeightedItem};

// ---------------------------------------------------------------------- EmptyOptionListener

/// EmptyOptionListener's `empty` lambda: anything whose name looks like a
/// potion becomes a vial; otherwise the EmptyItem row of the id. Always true.
pub fn empty<H: Host + ?Sized>(h: &mut H, player: &H::Node, node: &H::Node) -> Result<bool> {
    let name = h.name(node)?.ok_or(Error::Null)?;
    let lower = h.lowercase_default(&name);
    if name.contains("brew")
        || name.contains("potion")
        || lower.contains("poison")
        || lower.contains("serum")
        || name.contains("cure")
        || name.contains("mix")
        || name.contains("balm")
        || name.contains("Super ")
    {
        let slot = h.slot(as_item(h, node)?)?;
        let empty = tables::empty_item(195).ok_or(Error::Null)?;
        let vial = h.new_item(empty.empty, 1)?;
        let current = as_item(h, node)?.clone();
        h.replace_slot(player, slot, &vial, Some(&current), Store::Inventory)?;
        h.play_audio(player, empty.audio, 0, 1, None, 8)?;
        return Ok(true);
    }
    let id = h.id(node)?;
    if let Some(row) = tables::empty_item(id) {
        let slot = h.slot(as_item(h, node)?)?;
        let emptied = h.new_item(row.empty, 1)?;
        let current = as_item(h, node)?.clone();
        h.replace_slot(player, slot, &emptied, Some(&current), Store::Inventory)?;
        if row.audio != -1 {
            h.play_audio(player, row.audio, 0, 1, None, 8)?;
        }
        h.send_message(player, row.message)?;
    }
    Ok(true)
}

// ---------------------------------------------------------------------- LightSourceListeners

/// `Items.MINING_HELMET_5013` (lit) and `_5014` (unlit).
pub const LIT_HELMET: i32 = 5013;
pub const UNLIT_HELMET: i32 = 5014;
/// The Temple of Ikov dungeon's region.
pub const IKOV_REGION: i32 = 10648;

/// `LightSource.getActiveLightSource(player)`: the inventory's array first,
/// then the equipment's; the first lit product wins.
pub fn active_light_source<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
) -> Result<Option<LightSource>> {
    for container in [Container::Inventory, Container::Equipment] {
        for item in h
            .container_to_array(player, container)?
            .into_iter()
            .flatten()
        {
            if let Some(source) = LightSource::for_product_id(h.id(&item)?) {
                return Ok(Some(source));
            }
        }
    }
    Ok(None)
}

/// LightSourceListeners' `drop` lambda: the light lighting the Temple of
/// Ikov dungeon stays; otherwise it is removed and dropped (a lit mining
/// helmet goes out). True unless it stays.
pub fn drop_light<H: Host + ?Sized>(h: &mut H, player: &H::Node, light: &H::Node) -> Result<bool> {
    let active = active_light_source(h, player)?
        .ok_or(Error::Null)?
        .product();
    let [x, y, _] = h.location(player)?.ok_or(Error::Null)?;
    if (x >> 6 << 8 | (y >> 6)) == IKOV_REGION && h.id(light)? == active {
        let product = active_light_source(h, player)?
            .ok_or(Error::Null)?
            .product();
        let name = h.item_name(product).ok_or(Error::Null)?;
        let text = format!(
            "Dropping the {} would leave you without a light source.",
            h.lowercase_root(&name)
        );
        h.send_message(player, &text)?;
        return Ok(false);
    }
    let id = h.id(light)?;
    if h.remove_item(player, ItemArg::Id(id), Store::Inventory)? {
        let mut dropped = h.id(light)?;
        if h.id(light)? == LIT_HELMET {
            dropped = UNLIT_HELMET;
            h.send_message(player, "The helmet goes out as you drop it.")?;
        }
        h.produce_ground_item(player, dropped)?;
    }
    Ok(true)
}

// ---------------------------------------------------------------------- CasketListener

/// `Items.CASKET_405`.
pub const CASKET: i32 = 405;

/// `CasketListener.loot`, built (with its insertion draws) when the class
/// initialises.
pub fn casket_table(r: &mut dyn random::Random) -> WeightBasedTable {
    let rows = [
        WeightedItem::new(995, 20, 640, 55.0, false),
        WeightedItem::new(1623, 1, 1, 32.0, false),
        WeightedItem::new(1621, 1, 1, 16.0, false),
        WeightedItem::new(1619, 1, 1, 9.0, false),
        WeightedItem::new(1617, 1, 1, 2.0, false),
        WeightedItem::new(1454, 1, 1, 8.0, false),
        WeightedItem::new(987, 1, 1, 1.0, false),
        WeightedItem::new(985, 1, 1, 1.0, false),
    ];
    WeightBasedTable::create(&mut random::AsSource(r), rows)
}

/// The drop host the casket's roll needs: its table has none of the shared
/// table slots, so neither member is reached.
struct NoSharedTables;
impl drop_tables::Host for NoSharedTables {
    fn clue_scrolls(&self, _: drop_tables::ClueLevel) -> Option<&[i32]> {
        None
    }
    fn log(&mut self, _: &'static str, _: LogLevelOf, _: &str) {}
    fn stack_trace(&mut self, _: &'static str, _: drop_tables::shared::ParseFailure) {}
}
type LogLevelOf = crate::world::drop_tables::LogLevel;

/// CasketListener's `open` lambda: the casket goes, one roll of the table
/// lands in the inventory and the first item is announced. Always true.
pub fn casket<H: Host + ?Sized>(h: &mut H, player: &H::Node, node: &H::Node) -> Result<bool> {
    let casket = as_item(h, node)?.clone();
    if h.remove_item(player, ItemArg::Item(casket), Store::Inventory)? {
        let table = h.statics().casket.clone().ok_or(Error::Null)?;
        let rolled = {
            let mut shared = drop_tables::shared::SharedTables::default();
            let mut none = NoSharedTables;
            let mut source = random::AsSource(h.random());
            let mut ctx = Context::new(&mut source, &mut shared, &mut none);
            table
                .roll(&mut ctx, Looter::None, 1)
                .map_err(|e| Error::Binding(Some(format!("{e:?}"))))?
        };
        let mut loot = Vec::new();
        for item in rolled {
            loot.push(h.new_item(item.id, item.amount)?);
        }
        for item in &loot {
            h.container_add(player, Container::Inventory, item)?;
        }
        let first = loot.first().ok_or(Error::Index)?.clone();
        let amount = h.amount(&first)?;
        let name = h.name(&first)?;
        let article = if amount > 1 {
            "some"
        } else if is_plus_n(h, name.as_deref())? {
            "an"
        } else {
            "a"
        };
        let name = name.ok_or(Error::Null)?;
        let text = format!(
            "You open the casket. Inside you find {article} {}.",
            h.lowercase_default(&name)
        );
        h.item_message(player, &first, &[&text])?;
    }
    Ok(true)
}

// ---------------------------------------------------------------------- BirdNestScript, BirdNestPlugin, BirdNest

/// `Sounds.CUCKOO_1_1997`.
pub const CUCKOO: i32 = 1997;

/// BirdNestScript's `search` (`::handleNest`): another class answers false;
/// an item that is no nest fails at the search.
pub fn nest_script<H: Host + ?Sized>(h: &mut H, player: &H::Node, node: &H::Node) -> Result<bool> {
    if h.kind(node) != Kind::Item {
        return Ok(false);
    }
    let nest = BirdNest::for_nest(h.id(node)?).ok_or(Error::Null)?;
    search(h, nest, player, node)?;
    Ok(true)
}

/// BirdNestPlugin's `handle` (the class is not `@Initializable`: nothing
/// registers it at start-up).
pub fn nest_plugin<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    node: Option<&H::Node>,
) -> Result<bool> {
    let item = node.ok_or(Error::Null)?;
    let item = as_item(h, item)?;
    let nest = BirdNest::for_nest(h.id(item)?).ok_or(Error::Null)?;
    search(h, nest, player, item)?;
    Ok(true)
}

/// `BirdNest.search(player, item)`: the egg and raven nests give their one
/// loot, the others (and Wyson's) draw it; the nest becomes an empty one.
pub fn search<H: Host + ?Sized>(
    h: &mut H,
    nest: BirdNest,
    player: &H::Node,
    item: &H::Node,
) -> Result<()> {
    if h.container_free_slots(player, Container::Inventory)? < 1 {
        h.send_message(player, "You don't have enough inventory space.")?;
        return Ok(());
    }
    let loot = nest.loot();
    let chosen: ChanceItem = if nest.ordinal() > 1 && nest != BirdNest::Wyson {
        *loot.first().ok_or(Error::Index)?
    } else {
        let index = random::get_chance_item(h.random(), &loot).ok_or(Error::Null)?;
        loot[index]
    };
    let name = h.item_name(chosen.id).ok_or(Error::Null)?;
    let name = h.lowercase_default(&name);
    let input = format!(
        "{} {name}",
        if is_plus_n(h, Some(&name))? {
            "an"
        } else {
            "a"
        }
    );
    h.lock(player, 1)?;
    let loot_item = h.constant_item(chosen.id, chosen.minimum);
    h.container_add(player, Container::Inventory, &loot_item)?;
    let empty = h.constant_item(tables::EMPTY_NEST, 1);
    let slot = h.slot(item)?;
    h.container_replace(player, Container::Inventory, &empty, slot)?;
    h.send_message(player, &format!("You take {input} out of the bird's nest."))?;
    Ok(())
}

/// `BirdNest.getRandomNest(wyson)`: one of the first six nests by their
/// rates; the seed nest is Wyson's when asked for.
pub fn random_nest(r: &mut dyn random::Random, wyson: bool) -> Option<BirdNest> {
    let nests: Vec<ChanceItem> = BirdNest::ALL[..6].iter().map(|n| n.nest()).collect();
    let index = random::get_chance_item(r, &nests)?;
    let nest = BirdNest::ALL[index];
    Some(if wyson && nest == BirdNest::Seed {
        BirdNest::Wyson
    } else {
        nest
    })
}

/// `BirdNest.drop(player)`: a random nest falls to the ground.
pub fn drop_nest<H: Host + ?Sized>(h: &mut H, player: &H::Node) -> Result<()> {
    let nest = random_nest(h.random(), false);
    h.play_audio(player, CUCKOO, 0, 1, None, 8)?;
    let nest = nest.ok_or(Error::Null)?.nest();
    let item = h.constant_item(nest.id, nest.minimum);
    h.ground_item(player, &item)?;
    h.send_message(player, "<col=FF0000>A bird's nest falls out of the tree.")
}

// ---------------------------------------------------------------------- LampPlugin

/// The skill choice interface.
pub const LAMP_INTERFACE: i32 = 134;
/// The tabs the lamp hides.
pub const LAMP_TABS: [i32; 13] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 12, 13];
/// `Items.LAMP_2528` (the genie lamp).
pub const GENIE_LAMP: i32 = 2528;

/// LampPlugin's `handle`: the lamp and the plugin go into attributes, the
/// skill interface opens with its close event and the tabs go. Always true.
pub fn lamp<H: Host + ?Sized>(
    h: &mut H,
    identity: usize,
    player: &H::Node,
    node: Option<&H::Node>,
) -> Result<bool> {
    let value = node.map_or(Value::Null, |n| Value::Node(n.clone()));
    h.set_attribute(player, "lamp", value)?;
    h.set_attribute(
        player,
        "caller",
        Value::Object(Object::Handler {
            plugin: super::registry::OptionPlugin::Lamp,
            identity,
        }),
    )?;
    h.open_component(
        player,
        LAMP_INTERFACE,
        Some(Task::LampClose {
            player: player.clone(),
        }),
    )?;
    h.remove_tabs(player, &LAMP_TABS)?;
    Ok(true)
}

/// The lamp interface's close event: the tabs come back, the lamp attribute
/// goes and the player unlocks. True.
pub fn lamp_close<H: Host + ?Sized>(h: &mut H, player: &H::Node) -> Result<bool> {
    h.open_default_tabs(player)?;
    h.remove_attribute(player, "lamp")?;
    h.unlock(player)?;
    Ok(true)
}

/// `LampPlugin.handleSelectionCallback(skill, player)`: the lamp (a genie
/// lamp by default, built first) must meet its level; then the lamp is
/// removed and the experience given (a genie lamp gives ten times the level).
pub fn lamp_select<H: Host + ?Sized>(h: &mut H, skill: i32, player: &H::Node) -> Result<()> {
    let default = h.new_item(GENIE_LAMP, 1)?;
    let item = match h.get_attribute(player, "lamp")? {
        None | Some(Value::Null) => default,
        Some(Value::Node(n)) if h.kind(&n) == Kind::Item => n,
        Some(_) => return Err(Error::Cast),
    };
    let lamp = tables::lamp_for_item(h.id(&item)?);
    let level = h.static_level(player, skill)?;
    let (lamp_id, experience, requirement) = tables::LAMPS[lamp.ok_or(Error::Null)?];
    if level < requirement {
        let skill_name = usize::try_from(skill)
            .ok()
            .and_then(|s| tables::SKILL_NAME.get(s))
            .ok_or(Error::Index)?;
        h.send_message(
            player,
            &format!("You need at least {requirement} {skill_name} to do this."),
        )?;
        return Ok(());
    }
    let removed = match h.get_attribute(player, "lamp")? {
        None | Some(Value::Null) => None,
        Some(Value::Node(n)) if h.kind(&n) == Kind::Item => Some(n),
        Some(_) => return Err(Error::Cast),
    };
    if h.container_remove(player, Container::Inventory, removed.as_ref())? {
        if lamp_id == GENIE_LAMP {
            let level = h.static_level(player, skill)?;
            h.add_experience(player, skill, f64::from(level.wrapping_mul(10)))?;
        } else {
            h.add_experience(player, skill, f64::from(experience))?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------- StarterPackPlugin, RingUnchargePlugin, RunePackPlugin

/// `Items` id of the starter pack.
pub const STARTER_PACK: i32 = 14775;

/// StarterPackPlugin's `handle`: with eighteen free slots the box becomes
/// its items; otherwise how many more slots it needs. Always true.
pub fn starter_pack<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    node: Option<&H::Node>,
    option: Option<&str>,
) -> Result<bool> {
    if option.ok_or(Error::Null)? == "open" {
        let count = tables::STARTER_ITEMS.len() as i32;
        if h.container_free_slots(player, Container::Inventory)? >= count {
            if let Some(n) = node {
                as_item(h, n)?;
            }
            if h.container_remove(player, Container::Inventory, node)? {
                let items: Vec<Option<H::Node>> = tables::STARTER_ITEMS
                    .iter()
                    .map(|&(id, amount)| Some(h.constant_item(id, amount)))
                    .collect();
                h.container_add_all(player, Container::Inventory, &items)?;
                return Ok(true);
            }
        } else {
            let free = h.container_free_slots(player, Container::Inventory)?;
            h.send_message(
                player,
                &format!(
                    "You need {} more free inventory slot(s) to open the box.",
                    count.wrapping_sub(free)
                ),
            )?;
            return Ok(true);
        }
    }
    Ok(true)
}

/// RingUnchargePlugin's `handle`: the imbued ring becomes its plain ring.
pub fn ring_uncharge<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    node: Option<&H::Node>,
) -> Result<bool> {
    let Some(node) = node else {
        return Ok(true);
    };
    let id = h.id(as_item(h, node)?)?;
    let ring = h.new_item(id, 1)?;
    if h.container_remove(player, Container::Inventory, Some(&ring))? {
        let id = h.id(as_item(h, node)?)?;
        let reward = h.new_item(tables::ring_reward(id), 1)?;
        h.container_add(player, Container::Inventory, &reward)?;
    }
    Ok(true)
}

/// RunePackPlugin's `handle`: a lock, then the pack becomes 100 runes.
pub fn rune_pack<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    node: Option<&H::Node>,
) -> Result<bool> {
    let Some(node) = node else {
        return Ok(true);
    };
    h.lock(player, 1)?;
    let id = h.id(as_item(h, node)?)?;
    let pack = h.new_item(id, 1)?;
    if h.container_remove(player, Container::Inventory, Some(&pack))? {
        let id = h.id(as_item(h, node)?)?;
        let runes = h.new_item(tables::rune_pack_reward(id), 100)?;
        h.container_add(player, Container::Inventory, &runes)?;
    }
    Ok(true)
}

// ---------------------------------------------------------------------- HerbBoxPlugin

/// The herb box.
pub const HERB_BOX: i32 = 14810;

/// HerbBoxPlugin's `handle`: the herbs left are `10 - (1000 - charge)`; a
/// box outside 0..=10 is removed. Then a lock and the option: bank all,
/// take one (the box goes once its charge falls to 990), or check. True.
pub fn herb_box<H: Host + ?Sized>(
    h: &mut H,
    player: &H::Node,
    node: Option<&H::Node>,
    option: Option<&str>,
) -> Result<bool> {
    let herb_box = as_item(h, node.ok_or(Error::Null)?)?.clone();
    let herbs = 10i32.wrapping_sub(1000i32.wrapping_sub(h.charge(&herb_box)?));
    if !(0..=10).contains(&herbs) {
        let slot = h.slot(&herb_box)?;
        h.container_remove_slot(player, Container::Inventory, &herb_box, slot, true)?;
        return Ok(true);
    }
    h.lock(player, 1)?;
    match option.ok_or(Error::Null)? {
        "bank-all" => {
            if h.container_free_slots(player, Container::Bank)? < herbs {
                h.send_message(
                    player,
                    "You don't have enough bank space to bank all those herbs.",
                )?;
                return Ok(true);
            }
            let mut list = Vec::new();
            for _ in 0..herbs {
                list.push(random::get_chance_item(h.random(), &tables::HERBS));
            }
            let slot = h.slot(&herb_box)?;
            if h.container_remove_slot(player, Container::Inventory, &herb_box, slot, true)? {
                let items: Vec<Option<H::Node>> = list
                    .into_iter()
                    .map(|index| {
                        index.map(|i| {
                            let herb = tables::HERBS[i];
                            h.constant_item(herb.id, herb.minimum)
                        })
                    })
                    .collect();
                h.container_add_all(player, Container::Bank, &items)?;
                h.send_message(
                    player,
                    &format!("{herbs} herbs have been deposited into your bank."),
                )?;
            }
        }
        "take-one" => {
            if h.container_free_slots(player, Container::Inventory)? < 1 {
                h.send_message(player, "You don't have enough space in your inventory.")?;
                return Ok(true);
            }
            let index = random::get_chance_item(h.random(), &tables::HERBS).ok_or(Error::Null)?;
            let herb = tables::HERBS[index];
            let amount = herb.random_amount(h.random());
            let item = h.new_item(herb.id, amount)?;
            h.container_add(player, Container::Inventory, &item)?;
            let charge = h.charge(&herb_box)?;
            h.set_charge(&herb_box, charge.wrapping_sub(1))?;
            if h.charge(&herb_box)? <= 990 {
                let slot = h.slot(&herb_box)?;
                h.container_remove_slot(player, Container::Inventory, &herb_box, slot, true)?;
            }
            h.send_message(player, "You take a herb from the box.")?;
        }
        "check" => {
            h.send_message(player, &format!("The herb box contains {herbs} herbs."))?;
        }
        _ => {}
    }
    Ok(true)
}

// ---------------------------------------------------------------------- NewspaperListener, CadavaPotionListener, DrinkBlamishOilListener

/// The newspaper interface.
pub const NEWSPAPER_INTERFACE: i32 = 530;
/// `NewspaperListener.leftPage`.
pub const LEFT_PAGE: &str = "Varrock gets Makeover<br><br>The city of Varrock is the latest recipient of a complete makeover. When interviewed, King Roald said, 'In order to keep visitors coming to see the sights of our beautiful capital, we felt that tidying-up the city would be more effective than just issuing a decree - make sure you visit the new museum while you are here.'";
/// `NewspaperListener.rightPage`.
pub const RIGHT_PAGE: &str = "Obituaries <br><br>Goblin-Died<br>Giant Rat-Died<br>Unicorn-Died<br>Varrock Guard-Died<br>Varrock Guard-Died<br>Varrock Guard-Died<br>Bear-Died.<br><br>Classifieds<br><br>Lowe's Archery Emporium for the finest ranging weapons in town!<br><br>Time to party! Visit the Fancy Dress Shop for all your party outfits.<br><br>The Dancing Donkey - cold beer always in stock.";

/// NewspaperListener's newspaper `read`.
pub fn newspaper<H: Host + ?Sized>(h: &mut H, player: &H::Node) -> Result<bool> {
    h.open_interface(player, NEWSPAPER_INTERFACE)?;
    h.interface_text(player, LEFT_PAGE, NEWSPAPER_INTERFACE, 7)?;
    h.interface_text(player, RIGHT_PAGE, NEWSPAPER_INTERFACE, 8)?;
    Ok(true)
}

/// NewspaperListener's Al Kharid flyer `read`.
pub fn flyer<H: Host + ?Sized>(h: &mut H, player: &H::Node) -> Result<bool> {
    h.dialogue_lines(
        player,
        &[
            "Come to the Al Kharid Market place! High quality",
            "produce at low, low prices! Show this flyer to a",
            "merchant for money off your next purchase,",
            "courtesy of Ali Morrisane!",
        ],
    )?;
    Ok(true)
}

/// `Items.CADAVA_POTION_756`.
pub const CADAVA_POTION: i32 = 756;

/// CadavaPotionListener's `drink` (`look` false) and `look-at`.
pub fn cadava<H: Host + ?Sized>(h: &mut H, player: &H::Node, look: bool) -> Result<bool> {
    let text = if look {
        "This looks very colourful."
    } else {
        "You dare not drink."
    };
    h.item_dialogue(player, ItemArg::Id(CADAVA_POTION), text)?;
    Ok(true)
}

/// DrinkBlamishOilListener's `drink`.
pub fn blamish_oil<H: Host + ?Sized>(h: &mut H, player: &H::Node) -> Result<bool> {
    h.player_dialogue(
        player,
        "You know... I'd really rather not.",
        Expression::Neutral,
        false,
    )?;
    Ok(true)
}
