//! TradeModule.kt, TradeContainer.java (with TradeListener) and
//! TradeCloseEvent.java. Each function keeps the original call order; field
//! reads that the original repeats are repeated. No module lock is held while
//! a host, container or interface call runs.
use super::{
    Error, Host, Result, TradePlayer,
    ui::{Items, Ui},
};
use crate::definitions::item::Shared;
use crate::{
    inventory::{
        container::{Container, ContainerType, SortType},
        container_listener::{Handle as Listener, NativeListener},
        item::{Handle as Item, Item as ItemValue},
    },
    world::{
        interfaces::{self, Handle as Component},
        player_walk::NodeHandle,
    },
};
use rs09_protocol::game::updates::{self, ScriptArg, WireItem};
use std::sync::{Arc, Mutex, MutexGuard};

struct Fields<P> {
    retained: bool,
    player: Option<P>,
    target: Option<P>,
    container: Option<Container>,
    accepted: bool,
    modified: bool,
    stage: i32,
}
/// One TradeModule object. Clones are the same object.
pub struct Module<P>(Arc<Mutex<Fields<P>>>);
impl<P> Clone for Module<P> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<P: TradePlayer> Module<P> {
    fn fields(&self) -> MutexGuard<'_, Fields<P>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn identity(&self) -> usize {
        Arc::as_ptr(&self.0) as *const () as usize
    }
    pub fn player(&self) -> Option<P> {
        self.fields().player.clone()
    }
    pub fn target(&self) -> Option<P> {
        self.fields().target.clone()
    }
    pub fn container(&self) -> Option<Container> {
        self.fields().container.clone()
    }
    pub fn is_accepted(&self) -> bool {
        self.fields().accepted
    }
    pub fn is_modified(&self) -> bool {
        self.fields().modified
    }
    pub fn set_modified(&self, value: bool) {
        self.fields().modified = value;
    }
    pub fn is_retained(&self) -> bool {
        self.fields().retained
    }
    pub fn set_retained(&self, value: bool) {
        self.fields().retained = value;
    }
    pub fn stage(&self) -> i32 {
        self.fields().stage
    }
    /// `incrementStage()`.
    pub fn increment_stage(&self) {
        let mut f = self.fields();
        f.stage = f.stage.wrapping_add(1);
    }
    fn set_accepted_field(&self, value: bool) {
        self.fields().accepted = value;
    }
    fn player_req(&self) -> Result<P> {
        self.player().ok_or(Error::Null)
    }
    fn target_req(&self) -> Result<P> {
        self.target().ok_or(Error::Null)
    }
    fn container_req(&self) -> Result<Container> {
        self.container().ok_or(Error::Null)
    }
    /// Retained canonical nodes of the module's two player fields.
    pub fn retained_nodes(&self) -> Vec<NodeHandle> {
        let f = self.fields();
        f.player
            .iter()
            .chain(f.target.iter())
            .filter_map(TradePlayer::node)
            .collect()
    }
}

/// TradeContainer.TradeListener. It reads its container's final `player` field.
pub struct TradeListener<P> {
    pub player: Option<P>,
}
impl<P: TradePlayer> NativeListener for TradeListener<P> {
    fn retained_nodes(&self) -> Vec<NodeHandle> {
        self.player.iter().filter_map(TradePlayer::node).collect()
    }
}

/// `new TradeModule(player, target)`: its `init` makes `TradeContainer(player)`,
/// a DEFAULT/ID container of 28 with one TradeListener.
pub fn new_module<H: Host>(
    h: &mut H,
    player: Option<&H::Player>,
    target: Option<&H::Player>,
) -> Result<Module<H::Player>> {
    let factory = h.factory()?;
    let mut container = factory.create(28, ContainerType::Default, SortType::Id)?;
    container.register(Some(Listener::new(TradeListener {
        player: player.cloned(),
    })));
    Ok(Module(Arc::new(Mutex::new(Fields {
        retained: false,
        player: player.cloned(),
        target: target.cloned(),
        container: Some(container),
        accepted: false,
        modified: false,
        stage: 0,
    }))))
}

/// `TradeModule.getExtension(player)`.
pub fn extension<H: Host>(h: &mut H, p: Option<&H::Player>) -> Result<Option<Module<H::Player>>> {
    h.trade_module(p.ok_or(Error::Null)?)
}
fn extension_req<H: Host>(h: &mut H, p: Option<&H::Player>) -> Result<Module<H::Player>> {
    extension(h, p)?.ok_or(Error::Null)
}

pub(crate) fn pointer(interface: i32, child: i32) -> i32 {
    interface.wrapping_shl(16) | (child & 0xFFFF)
}
fn wire(items: &[Option<Item>]) -> Vec<Option<WireItem>> {
    items
        .iter()
        .map(|item| {
            item.as_ref().map(|item| {
                let item = item.read();
                WireItem {
                    id: item.id(),
                    amount: item.amount(),
                }
            })
        })
        .collect()
}
fn protocol<T>(r: std::result::Result<T, rs09_protocol::Error>) -> Result<T> {
    r.map_err(|e| Error::Service(format!("protocol: {e}")))
}
/// `PacketRepository.send(ContainerPacket.class, new ContainerContext(...))` for a
/// whole array (the length argument is not read by the full writer).
fn send_container<H: Host>(
    h: &mut H,
    p: &H::Player,
    interface: i32,
    child: i32,
    id: i32,
    items: &[Option<Item>],
) -> Result<()> {
    let frame = protocol(updates::inventory_full(
        pointer(interface, child),
        id,
        &wire(items),
    ))?;
    h.send(p, frame)
}
fn send_string<H: Host>(
    h: &mut H,
    p: &H::Player,
    text: &str,
    interface: i32,
    line: i32,
) -> Result<()> {
    let m = h.interface_manager(p)?;
    let frame = protocol(updates::interface_text(
        pointer(interface, line),
        text.as_bytes(),
        m.packet_count(1),
    ))?;
    h.send(p, frame)
}
fn send_iface_settings<H: Host>(
    h: &mut H,
    p: &H::Player,
    hash: i32,
    child: i32,
    interface: i32,
    offset: i32,
    length: i32,
) -> Result<()> {
    let m = h.interface_manager(p)?;
    let frame = updates::interface_events(
        interface.wrapping_shl(16) | child,
        offset,
        length,
        hash,
        m.packet_count(1),
    );
    h.send(p, frame)
}
pub(crate) fn send_run_script<H: Host>(
    h: &mut H,
    p: &H::Player,
    id: i32,
    types: &str,
    args: &[ScriptArg],
) -> Result<()> {
    let m = h.interface_manager(p)?;
    let frame = protocol(updates::run_client_script(
        id,
        types.as_bytes(),
        args,
        m.packet_count(1),
    ))?;
    h.send(p, frame)
}
fn send_interface_config<H: Host>(
    h: &mut H,
    p: &H::Player,
    interface: i32,
    child: i32,
    hidden: bool,
) -> Result<()> {
    let m = h.interface_manager(p)?;
    let frame = updates::interface_hidden(
        hidden,
        m.packet_count(1),
        interface.wrapping_shl(16) | child,
    );
    h.send(p, frame)
}

fn text(value: &str) -> ScriptArg {
    ScriptArg::Text(value.as_bytes().to_vec())
}
fn params(strings: [&str; 9], ints: [i32; 6]) -> Vec<ScriptArg> {
    strings
        .iter()
        .map(|s| text(s))
        .chain(ints.iter().map(|i| ScriptArg::Int(*i)))
        .collect()
}
fn inventory_params() -> Vec<ScriptArg> {
    params(
        [
            "",
            "",
            "",
            "Lend",
            "Offer-X",
            "Offer-All",
            "Offer-10",
            "Offer-5",
            "Offer",
        ],
        [-1, 0, 7, 4, 93, 336 << 16],
    )
}
fn trade_params() -> Vec<ScriptArg> {
    params(
        [
            "",
            "",
            "",
            "",
            "Remove-X",
            "Remove-All",
            "Remove-10",
            "Remove-5",
            "Remove",
        ],
        [-1, 0, 7, 4, 90, (335 << 16) | 30],
    )
}
fn partner_params() -> Vec<ScriptArg> {
    params(
        ["", "", "", "", "", "", "", "", ""],
        [-1, 0, 7, 4, 91, (335 << 16) | 32],
    )
}
/// `HIDDEN_CHILDS`, with its repeated entries.
pub const HIDDEN_CHILDS: [i32; 7] = [42, 43, 44, 42, 44, 40, 41];

// ---------------------------------------------------------------- TradeModule

/// `TradeModule.extend(player, target)`.
pub fn extend<H: Host>(h: &mut H, player: &H::Player, target: &H::Player) -> Result<()> {
    let module = new_module(h, Some(player), Some(target))?;
    h.add_trade_module(player, module)?;
    let module = new_module(h, Some(target), Some(player))?;
    h.add_trade_module(target, module)
}

/// `TradeModule.open(player, target)` (RequestModule.open of RequestType.TRADE).
pub fn open<H: Host>(h: &mut H, player: &H::Player, target: &H::Player) -> Result<()> {
    extend(h, player, target)?;
    if extension(h, Some(target))?.is_none() || extension(h, Some(player))?.is_none() {
        return Ok(());
    }
    // `getInterface(stage)` and `display(stage)` read the stage of the module
    // `open` is called on: RequestType.TRADE's placeholder, which stays 0.
    let module = extension_req(h, Some(player))?;
    let component = interface_for(h, &module, 0)?;
    open_interface(h, &module, &component)?;
    display(h, &module, 0)?;
    if extension(h, Some(target))?.is_none() || extension(h, Some(player))?.is_none() {
        return Ok(());
    }
    let module = extension_req(h, Some(target))?;
    let component = interface_for(h, &module, 0)?;
    open_interface(h, &module, &component)?;
    display(h, &module, 0)?;
    h.dialogue_close(player)?;
    h.dialogue_close(target)
}

/// `getInterface(stage)`.
pub fn interface_for<H: Host>(
    h: &mut H,
    _module: &Module<H::Player>,
    stage: i32,
) -> Result<Component> {
    let c = h.components()?;
    Ok(if stage == 0 { c.main } else { c.accept })
}

/// `update()`: this module's display, then the target's with this stage.
pub fn update<H: Host>(h: &mut H, module: &Module<H::Player>) -> Result<()> {
    display(h, module, module.stage())?;
    let target = module.target();
    let other = extension_req(h, target.as_ref())?;
    display(h, &other, module.stage())
}

fn open_interface<H: Host>(
    h: &mut H,
    module: &Module<H::Player>,
    component: &Component,
) -> Result<()> {
    let main = h.components()?.main;
    if component.same(&main) {
        open_main(h, module)
    } else {
        open_second(h, module)
    }
}

/// The `acceptMessage` getter.
fn accept_message<H: Host>(h: &mut H, module: &Module<H::Player>) -> Result<&'static str> {
    let target = module.target();
    let other = extension_req(h, target.as_ref())?.is_accepted();
    let accepted = module.is_accepted();
    Ok(if !other && !accepted {
        ""
    } else if other {
        "Other player has accepted."
    } else {
        "Waiting for other player..."
    })
}

/// Kotlin `substring(0, 1).uppercase(Locale.getDefault()) + substring(1)`.
fn capitalised(name: &str) -> Result<String> {
    let mut chars = name.chars();
    let first = chars.next().ok_or(Error::StringBounds)?;
    Ok(first.to_uppercase().chain(chars).collect())
}

fn display<H: Host>(h: &mut H, module: &Module<H::Player>, stage: i32) -> Result<()> {
    let player = module.player_req()?;
    match stage {
        0 => {
            for i in HIDDEN_CHILDS {
                send_string(h, &player, "", 335, i)?;
            }
            let target = module.target_req()?;
            let name = h.username(&target)?;
            send_string(h, &player, &format!("Trading With: {name}"), 335, 15)?;
            let name = h.username(&target)?;
            let free = h.inventory(&target)?.free_slots();
            let count = if free == 0 {
                "no".to_string()
            } else {
                h.inventory(&target)?.free_slots().to_string()
            };
            send_string(
                h,
                &player,
                &format!("{name} has {count} free inventory slots."),
                335,
                21,
            )?;
            let message = accept_message(h, module)?;
            send_string(h, &player, message, 335, 36)?;
        }
        1 => {
            let target = module.target_req()?;
            let name = h.username(&target)?;
            let shown = capitalised(&name)?;
            send_string(
                h,
                &player,
                &format!("<col=00FFFF>Trading with:<br><col=00FFFF>{shown}"),
                334,
                2,
            )?;
            let message = accept_message(h, module)?;
            if !message.is_empty() {
                send_string(h, &player, message, 334, 33)?;
            }
            let m = h.interface_manager(&player)?;
            interfaces::restore_tabs(&mut Ui::new(h, &player), &m)?;
        }
        _ => {}
    }
    display_modified(h, module, stage)?;
    let defs = h.definitions()?;
    let mut container = module.container_req()?;
    container.update(Some(true), &defs, &mut Items { host: h })?;
    Ok(())
}

fn display_modified<H: Host>(h: &mut H, module: &Module<H::Player>, stage: i32) -> Result<()> {
    let target = module.target();
    let other = extension_req(h, target.as_ref())?.is_modified();
    let player = module.player_req()?;
    if stage == 0 {
        if other {
            h.set_varp(&player, 1043, 1)?;
        }
        if module.is_modified() {
            h.set_varp(&player, 1042, 1)?;
        }
    } else {
        let modified = module.is_modified();
        send_interface_config(h, &player, 334, 45, !modified)?;
        send_interface_config(h, &player, 334, 46, !other)?;
    }
    Ok(())
}

/// `alert(slot)`: the flashing icon on both screens.
pub fn alert<H: Host>(h: &mut H, module: &Module<H::Player>, slot: i32) -> Result<()> {
    let player = module.player_req()?;
    send_run_script(
        h,
        &player,
        143,
        "Iiii",
        &[
            ScriptArg::Int(slot),
            ScriptArg::Int(7),
            ScriptArg::Int(4),
            ScriptArg::Int(21954594),
        ],
    )?;
    let target = module.target_req()?;
    send_run_script(
        h,
        &target,
        143,
        "Iiii",
        &[
            ScriptArg::Int(slot),
            ScriptArg::Int(7),
            ScriptArg::Int(4),
            ScriptArg::Int(21954593),
        ],
    )
}

/// `decline()`.
pub fn decline<H: Host>(h: &mut H, module: &Module<H::Player>) -> Result<()> {
    let player = module.player_req()?;
    let m = h.interface_manager(&player)?;
    interfaces::close(&mut Ui::new(h, &player), &m)?;
    let target = module.target_req()?;
    h.message(
        &target,
        "<col=FF0000>Other player has declined trade!</col>",
    )
}

fn next_stage<H: Host>(h: &mut H, module: &Module<H::Player>) -> Result<()> {
    if module.is_accepted() && extension_req(h, module.target().as_ref())?.is_accepted() {
        if module.stage() == 0 {
            if !has_space(h, module)? {
                return Ok(());
            }
            module.increment_stage();
            let component = interface_for(h, module, module.stage())?;
            open_interface(h, module, &component)?;
            extension_req(h, module.target().as_ref())?.increment_stage();
            let other = extension_req(h, module.target().as_ref())?;
            let component = interface_for(h, module, module.stage())?;
            open_interface(h, &other, &component)?;
            let other = extension_req(h, module.target().as_ref())?;
            set_accepted(h, &other, false, false)?;
            set_accepted(h, module, false, false)?;
        } else {
            give_containers(h, module)?;
            module.increment_stage();
            extension_req(h, module.target().as_ref())?.increment_stage();
            let other = extension_req(h, module.target().as_ref())?;
            set_accepted(h, &other, false, false)?;
            set_accepted(h, module, false, false)?;
            let player = module.player_req()?;
            let m = h.interface_manager(&player)?;
            interfaces::close(&mut Ui::new(h, &player), &m)?;
            return Ok(());
        }
    }
    update(h, module)
}

fn open_main<H: Host>(h: &mut H, module: &Module<H::Player>) -> Result<()> {
    let player = module.player_req()?;
    let c = h.components()?;
    let m = h.interface_manager(&player)?;
    interfaces::close_default_tabs(&mut Ui::new(h, &player), &m)?;
    let m = h.interface_manager(&player)?;
    interfaces::open(&mut Ui::new(h, &player), &m, Some(c.main.clone()))?;
    let m = h.interface_manager(&player)?;
    interfaces::open_single_tab(&mut Ui::new(h, &player), &m, &c.overlay)?;
    let defs = h.definitions()?;
    let mut inventory = h.inventory(&player)?;
    inventory.refresh(None, &defs, &mut Items { host: h })?;
    send_iface_settings(h, &player, 1278, 30, 335, 0, 27)?;
    send_iface_settings(h, &player, 1026, 32, 335, 0, 27)?;
    send_iface_settings(h, &player, 1278, 0, 336, 0, 27)?;
    send_iface_settings(h, &player, 2360446, 0, 335, 0, 27)?;
    send_run_script(h, &player, 150, "IviiiIsssssssss", &inventory_params())?;
    send_run_script(h, &player, 150, "IviiiIsssssssss", &trade_params())?;
    send_run_script(h, &player, 695, "IviiiIsssssssss", &partner_params())
}

fn open_second<H: Host>(h: &mut H, module: &Module<H::Player>) -> Result<()> {
    let player = module.player_req()?;
    let c = h.components()?;
    let m = h.interface_manager(&player)?;
    interfaces::open(&mut Ui::new(h, &player), &m, Some(c.accept.clone()))?;
    let m = h.interface_manager(&player)?;
    interfaces::close_single_tab(&mut Ui::new(h, &player), &m)?;
    let mine = module.container();
    display_offer(h, &player, mine.as_ref(), 37)?;
    let theirs = extension_req(h, module.target().as_ref())?.container();
    display_offer(h, &player, theirs.as_ref(), 41)
}

/// `setAccepted(accept, update)`.
pub fn set_accepted<H: Host>(
    h: &mut H,
    module: &Module<H::Player>,
    accept: bool,
    update: bool,
) -> Result<()> {
    module.set_accepted_field(accept);
    if update {
        next_stage(h, module)?;
    }
    Ok(())
}

fn display_offer<H: Host>(
    h: &mut H,
    player: &H::Player,
    container: Option<&Container>,
    child: i32,
) -> Result<()> {
    let container = container.ok_or(Error::Null)?;
    let split = container.count() > 14;
    if split {
        send_interface_config(h, player, 334, child + 1, false)?;
        send_interface_config(h, player, 334, child + 2, false)?;
        let items = container.items().read().clone();
        let first = display_message(&split_list(&items, 0, 14)?)?;
        let items = container.items().read().clone();
        let length = container.items().read().len() as i32;
        let second = display_message(&split_list(&items, 14, length)?)?;
        send_string(h, player, &first, 334, child + 1)?;
        send_string(h, player, &second, 334, child + 2)?;
    } else {
        send_interface_config(h, player, 334, child, false)?;
        let items = container.items().read().clone();
        let message = display_message(&items)?;
        send_string(h, player, &message, 334, child)?;
    }
    Ok(())
}

/// `getDisplayMessage(items)`.
pub fn display_message(items: &[Option<Item>]) -> Result<String> {
    let mut message = String::from("Absolutely nothing!");
    if !items.is_empty() {
        message.clear();
        for item in items.iter().flatten() {
            let item = item.read();
            let name = item.name.clone().unwrap_or_else(|| "null".into());
            message.push_str("<col=FF9040>");
            message.push_str(&name);
            if item.amount() > 1 {
                message.push_str("<col=FFFFFF> x ");
                message.push_str("<col=FFFF00>");
                message.push_str(&formatted_number(item.amount()));
                message.push_str("<br>");
            } else {
                message.push_str("<br>");
            }
        }
    }
    Ok(message)
}

/// `DecimalFormat("#,###,##0").format(amount.toLong())` in en-US.
pub fn formatted_number(amount: i32) -> String {
    let digits = (amount as i64).unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if amount < 0 { format!("-{out}") } else { out }
}

fn has_space<H: Host>(h: &mut H, module: &Module<H::Player>) -> Result<bool> {
    let mut space = true;
    let player = module.player_req()?;
    let theirs = extension_req(h, module.target().as_ref())?.container();
    let defs = h.definitions()?;
    let inventory = h.inventory(&player)?;
    let target = module.target_req()?;
    if !inventory.has_space_for_container(
        theirs.map(|c| c.items()).as_ref(),
        &defs,
        &mut Items { host: h },
    )? {
        h.message(
            &target,
            "Other player doesn't have enough space in their inventory for this trade.",
        )?;
        h.message(&player, "You don't have enough inventory space for this.")?;
        space = false;
    } else {
        let inventory = h.inventory(&target)?;
        let mine = module.container();
        if !inventory.has_space_for_container(
            mine.map(|c| c.items()).as_ref(),
            &defs,
            &mut Items { host: h },
        )? {
            h.message(
                &player,
                "Other player doesn't have enough space in their inventory for this trade.",
            )?;
            h.message(&target, "You don't have enough inventory space for this.")?;
            space = false;
        }
    }
    if !space {
        set_accepted(h, module, false, true)?;
        let other = extension_req(h, module.target().as_ref())?;
        set_accepted(h, &other, false, true)?;
    }
    Ok(space)
}

fn give_containers<H: Host>(h: &mut H, module: &Module<H::Player>) -> Result<()> {
    let Some(mine) = module.container() else {
        return Ok(());
    };
    let Some(theirs) = extension_req(h, module.target().as_ref())?.container() else {
        return Ok(());
    };
    let player = module.player_req()?;
    let target = module.target_req()?;
    h.log_trade(&player, &target, &mine, &theirs)?;
    let name = super::lowercase(&h.username(&player)?);
    bot_received(h, &name, &target, &theirs)?;
    let name = super::lowercase(&h.username(&target)?);
    bot_received(h, &name, &player, &mine)?;
    add_container(h, &player, &theirs)?;
    add_container(h, &target, &mine)?;
    h.message(&target, "Accepted trade.")?;
    h.message(&player, "Accepted trade.")
}

/// `(AIRepository.PulseRepository[name]?.botScript as DoublingMoney?)?.itemsReceived(...)`.
fn bot_received<H: Host>(h: &mut H, name: &str, from: &H::Player, items: &Container) -> Result<()> {
    match h.bot(name)? {
        None => Ok(()),
        Some(bot) if !bot.doubling => Err(Error::ClassCast),
        Some(_) => h.bot_items_received(name, from, items),
    }
}

/// `addContainer(player, container)`: the receiver's maximum add clamps each
/// traded item in place, and what the inventory refuses goes to the ground.
fn add_container<H: Host>(h: &mut H, player: &H::Player, container: &Container) -> Result<()> {
    let defs = h.definitions()?;
    let factory = h.factory()?;
    // `Container c = new Container(itemCount, ALWAYS_STACK); c.addAll(container)`: unused copy.
    let mut copy = factory.create(container.count(), ContainerType::AlwaysStack, SortType::Id)?;
    copy.add_many(&container.items(), &defs, &mut Items { host: h })?;
    let items = container.items().read().clone();
    for item in items.into_iter().flatten() {
        let inventory = h.inventory(player)?;
        if item.read().amount() > inventory.maximum_add(Some(&item))? {
            let maximum = inventory.maximum_add(Some(&item))?;
            item.write().set_amount(maximum);
        }
        let mut inventory = h.inventory(player)?;
        if !inventory.add(Some(&item), true, -1, &defs, &mut Items { host: h })? {
            h.ground(&item, player)?;
        }
    }
    Ok(())
}

fn split_list(items: &[Option<Item>], min: i32, max: i32) -> Result<Vec<Option<Item>>> {
    let mut list = Vec::new();
    for i in min..max {
        let item = items.get(i as usize).ok_or(Error::Bounds)?;
        if item.is_none() {
            continue;
        }
        list.push(item.clone());
    }
    Ok(list)
}

// ------------------------------------------------------------- TradeContainer

fn can_use<H: Host>(h: &mut H, player: &H::Player) -> Result<bool> {
    let m = h.interface_manager(player)?;
    if !interfaces::is_opened(&m) {
        return Ok(false);
    }
    let Some(module) = extension(h, Some(player))? else {
        return Ok(false);
    };
    if extension(h, module.target().as_ref())?.is_none() {
        return Ok(false);
    }
    Ok(extension_req(h, Some(player))?.stage() == 0)
}

/// `validatedItem`: the slot bound is the player's inventory capacity for both
/// containers, and `slot == capacity` passes.
fn validated(item: Option<&Item>, slot: i32, amount: i32, player_capacity: i32) -> bool {
    if item.is_none() {
        return false;
    }
    !(slot < 0 || slot > player_capacity || amount < 1)
}

/// `ext(player).isAccepted() || ext(ext(player).getTarget()).isAccepted()`.
fn either_accepted<H: Host>(h: &mut H, player: &H::Player) -> Result<bool> {
    if extension_req(h, Some(player))?.is_accepted() {
        return Ok(true);
    }
    let own = extension_req(h, Some(player))?;
    Ok(extension_req(h, own.target().as_ref())?.is_accepted())
}
/// `ext(player).setAccepted(false, true); ext(ext(player).getTarget()).setAccepted(false, true)`.
fn reset_both<H: Host>(h: &mut H, player: &H::Player) -> Result<()> {
    let own = extension_req(h, Some(player))?;
    set_accepted(h, &own, false, true)?;
    let own = extension_req(h, Some(player))?;
    let other = extension_req(h, own.target().as_ref())?;
    set_accepted(h, &other, false, true)
}

fn stabalize(item: &Item, amount: i32, container: &Container) -> i32 {
    let maximum = container.amount(Some(item));
    if amount > maximum { maximum } else { amount }
}

fn check_capacity(item: &Item, container: &Container) -> Result<bool> {
    if item.read().amount() > container.maximum_add(Some(item))? {
        let maximum = container.maximum_add(Some(item))?;
        item.write().set_amount(maximum);
        if item.read().amount() < 1 {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Ids that ironmen may trade (Shield of Arrav and Heroes' Quest).
pub const IRONMAN_EXCEPTIONS: [i32; 5] = [11174, 11173, 759, 1586, 1577];
/// `colorize("%RYou can not trade items with your own bot accounts.")`.
pub const OWN_BOT: &str = "<col=ff0000>You can not trade items with your own bot accounts.</col> ";

fn tradeable<H: Host>(h: &mut H, player: &H::Player, item: &Item) -> Result<bool> {
    let definition = {
        let id = item.read().id();
        h.definitions()?.for_id(id)
    };
    let target = extension_req(h, Some(player))?.target();
    let player_bot = h.attribute_bool(player, "not_on_highscores", false)?;
    let target_ref = target.as_ref().ok_or(Error::Null)?;
    let target_bot = h.attribute_bool(target_ref, "not_on_highscores", false)?;
    let player_ip = h.ip_address(player)?;
    let target_ip = h.ip_address(target_ref)?;
    let player_mac = h.mac_address(player)?;
    let target_mac = h.mac_address(target_ref)?;
    let player_host = h.computer_name(player)?;
    let target_host = h.computer_name(target_ref)?;
    let id = item.read().id();
    if IRONMAN_EXCEPTIONS.contains(&id) {
        return Ok(true);
    }
    if h.ironman(player)?.is_ironman() || h.ironman(target_ref)?.is_ironman() {
        return Ok(false);
    }
    if (player_bot || target_bot)
        && (player_ip == target_ip || player_mac == target_mac || player_host == target_host)
    {
        h.content_message(player, OWN_BOT)?;
        return Ok(false);
    }
    let name = item.read().name.clone().ok_or(Error::Null)?;
    if name == "Coins" && id != 995 {
        return Ok(false);
    }
    Ok(crate::inventory::item::definition_tradeable(&definition)?)
}

/// `TradeContainer.offer(slot, amount)`, on the container of `module`.
pub fn offer<H: Host>(h: &mut H, module: &Module<H::Player>, slot: i32, amount: i32) -> Result<()> {
    let player = module_owner(module)?;
    let inventory = h.inventory(&player)?;
    let item = inventory.get(slot);
    if !can_use(h, &player)? {
        return Ok(());
    }
    let capacity = h.inventory(&player)?.capacity();
    if !validated(item.as_ref(), slot, amount, capacity) {
        return Ok(());
    }
    let item = item.ok_or(Error::Null)?;
    if !tradeable(h, &player, &item)? && !h.dev_mode()? {
        h.message(&player, "You can't trade this item.")?;
        return Ok(());
    }
    let defs = h.definitions()?;
    let remove: Item = Shared::new(ItemValue::new(item.read().id(), amount, 1000, &defs));
    let inventory = h.inventory(&player)?;
    let stable = stabalize(&remove, amount, &inventory);
    remove.write().set_amount(stable);
    let mut this = module.container_req()?;
    if !check_capacity(&remove, &this)? {
        h.message(
            &player,
            "You must accept the current items before adding any more.",
        )?;
        return Ok(());
    }
    let mut inventory = h.inventory(&player)?;
    if inventory.remove(
        Some(&remove),
        Some(slot),
        true,
        &defs,
        &mut Items { host: h },
    )? && this.add(Some(&remove), true, -1, &defs, &mut Items { host: h })?
    {
        if either_accepted(h, &player)? {
            reset_both(h, &player)?;
        }
        let own = extension_req(h, Some(&player))?;
        update(h, &own)?;
        check_alert(h, &player)?;
    }
    Ok(())
}

/// `TradeContainer.withdraw(slot, amount)`, on the container of `module`.
pub fn withdraw<H: Host>(
    h: &mut H,
    module: &Module<H::Player>,
    slot: i32,
    amount: i32,
) -> Result<()> {
    let player = module_owner(module)?;
    let mut this = module.container_req()?;
    let item = this.get(slot);
    if !can_use(h, &player)? {
        return Ok(());
    }
    let capacity = h.inventory(&player)?.capacity();
    if !validated(item.as_ref(), slot, amount, capacity) {
        return Ok(());
    }
    let item = item.ok_or(Error::Null)?;
    let defs = h.definitions()?;
    let remove: Item = Shared::new(ItemValue::new(item.read().id(), amount, 1000, &defs));
    let stable = stabalize(&remove, amount, &this);
    remove.write().set_amount(stable);
    let inventory = h.inventory(&player)?;
    if !check_capacity(&remove, &inventory)? {
        h.message(&player, "You don't have enough space in your inventory.")?;
        return Ok(());
    }
    if this.remove(
        Some(&remove),
        Some(slot),
        true,
        &defs,
        &mut Items { host: h },
    )? && {
        let mut inventory = h.inventory(&player)?;
        inventory.add(Some(&remove), true, -1, &defs, &mut Items { host: h })?
    } {
        this.shift(&defs, &mut Items { host: h })?;
        extension_req(h, Some(&player))?.set_modified(true);
        if either_accepted(h, &player)? {
            reset_both(h, &player)?;
            let own = extension_req(h, Some(&player))?;
            update(h, &own)?;
        } else {
            let own = extension_req(h, Some(&player))?;
            update(h, &own)?;
        }
    }
    if extension_req(h, Some(&player))?.is_modified() {
        alert_submit(h, &player, slot, true)?;
    }
    Ok(())
}

/// TradeContainer's final `player` field (the module's own player).
fn module_owner<P: TradePlayer>(module: &Module<P>) -> Result<P> {
    module.player().ok_or(Error::Null)
}

/// `alert(slot, save)`: submits the one-tick pulse and, when saving, stores
/// the deadline and slot.
fn alert_submit<H: Host>(h: &mut H, player: &H::Player, slot: i32, save: bool) -> Result<()> {
    h.submit_alert(player, slot)?;
    if save {
        let ticks = h.ticks()?;
        h.set_attribute_int(player, "alert", ticks.wrapping_add(8))?;
        h.set_attribute_int(player, "alertSlot", slot)?;
    }
    Ok(())
}

/// `checkAlert()`.
fn check_alert<H: Host>(h: &mut H, player: &H::Player) -> Result<()> {
    if h.attribute_int(player, "alert", 0)? > h.ticks()? {
        let slot = h.attribute_int(player, "alertSlot", 0)?;
        alert_submit(h, player, slot, false)?;
    }
    Ok(())
}

/// The body of the pulse `alert` submits: `pulse()` returns true.
pub fn alert_pulse<H: Host>(h: &mut H, player: &H::Player, slot: i32) -> Result<bool> {
    if extension(h, Some(player))?.is_some() {
        let module = extension_req(h, Some(player))?;
        alert(h, &module, slot)?;
    }
    Ok(true)
}

/// TradeListener.update(c, event).
pub fn listener_update<H: Host>(
    h: &mut H,
    listener: &TradeListener<H::Player>,
    container: &Container,
) -> Result<()> {
    let player = listener.player.clone().ok_or(Error::Null)?;
    let items = container.items().read().clone();
    send_container(h, &player, -1, 64212, 90, &items)?;
    if let Some(own) = extension(h, Some(&player))?
        && extension(h, own.target().as_ref())?.is_some()
    {
        let own = extension_req(h, Some(&player))?;
        let other = extension_req(h, own.target().as_ref())?;
        let items = other.container_req()?.items().read().clone();
        send_container(h, &player, 60981, -2, 91, &items)?;
    }
    check_alert(h, &player)
}

/// TradeListener.refresh(c): sends the player's inventory, not `c`.
pub fn listener_refresh<H: Host>(h: &mut H, listener: &TradeListener<H::Player>) -> Result<()> {
    let player = listener.player.clone().ok_or(Error::Null)?;
    let items = h.inventory(&player)?.items().read().clone();
    send_container(h, &player, -1, 64209, 93, &items)
}

// ------------------------------------------------------------ TradeCloseEvent

/// `TradeCloseEvent.close(player, c)`; always true.
pub fn close_event<H: Host>(h: &mut H, player: &H::Player, _component: &Component) -> Result<bool> {
    let module = extension(h, Some(player))?;
    send_run_script(h, player, 101, "", &[])?;
    let Some(module) = module else {
        return Ok(true);
    };
    let target = module.target();
    let Some(other) = extension(h, target.as_ref())? else {
        return Ok(true);
    };
    if module.is_accepted() && other.is_accepted() {
        return Ok(true);
    }
    let target = target.ok_or(Error::Null)?;
    if module.stage() != 2 {
        retain(h, player)?;
        retain(h, &target)?;
    }
    close_interfaces(h, player)?;
    close_interfaces(h, &target)?;
    let m = h.interface_manager(&target)?;
    interfaces::close(&mut Ui::new(h, &target), &m)?;
    end(h, player)?;
    let m = h.interface_manager(player)?;
    interfaces::open_default_tabs(&mut Ui::new(h, player), &m)?;
    end(h, &target)?;
    let m = h.interface_manager(&target)?;
    interfaces::open_default_tabs(&mut Ui::new(h, &target), &m)?;
    Ok(true)
}

fn close_interfaces<H: Host>(h: &mut H, player: &H::Player) -> Result<()> {
    h.remove_trade_module(player)?;
    let m = h.interface_manager(player)?;
    interfaces::close_single_tab(&mut Ui::new(h, player), &m)?;
    send_container(h, player, -1, 2, 24, &[])?;
    send_container(h, player, -1, 2, 23, &[])?;
    send_run_script(h, player, 101, "", &[])
}

fn end<H: Host>(h: &mut H, player: &H::Player) -> Result<()> {
    h.set_varp(player, 1043, 0)?;
    h.set_varp(player, 1042, 0)
}

fn retain<H: Host>(h: &mut H, player: &H::Player) -> Result<()> {
    let Some(module) = extension(h, Some(player))? else {
        return Ok(());
    };
    if module.is_retained() {
        return Ok(());
    }
    module.set_retained(true);
    let defs = h.definitions()?;
    let items = module.container_req()?.items();
    let mut inventory = h.inventory(player)?;
    inventory.add_many(&items, &defs, &mut Items { host: h })?;
    Ok(())
}
