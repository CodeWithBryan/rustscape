//! `Player.init`, `Entity.init` and `LoginConfiguration`: the lobby or the
//! game world, and every packet the original queues while configuring the
//! client, in the original order.
use super::{Error, Host, Level, Result, username};
use crate::startup::generators::CountingRandom;
use crate::world::interfaces::Context as _;
use crate::world::{
    interfaces::{self, Open},
    repository::Repository,
};
use std::hash::Hash;

/// The class whose name `log(LoginConfiguration.class, ...)` reports.
pub const ORIGIN: &str = "core.game.node.entity.player.info.login.LoginConfiguration";
/// The class whose name `log(this.getClass(), ...)` reports in `Player.init`.
pub const PLAYER_ORIGIN: &str = "core.game.node.entity.player.Player";
/// `LOBBY_PANE` and `LOBBY_INTERFACE`.
pub const LOBBY_PANE: i32 = 549;
pub const LOBBY_INTERFACE: i32 = 378;
/// `MESSAGE_MODEL`.
pub const MESSAGE_MODEL: [i32; 16] = [
    15, 16, 17, 18, 19, 20, 21, 22, 23, 405, 447, 622, 623, 679, 715, 800,
];
/// `SpellBook.forInterface`: the modern, ancient and lunar books.
pub const MODERN: i32 = 192;
pub const ANCIENT: i32 = 193;
pub const LUNAR: i32 = 430;

/// `Player.init()`.
pub fn player_init<P: Clone + Eq + Hash, H: Host<P>>(
    host: &mut H,
    repository: &Repository<P>,
    player: &P,
) -> Result<()> {
    if !host.is_artificial(player) {
        let name = username(host, player)?;
        host.log_line(
            PLAYER_ORIGIN,
            Level::Info,
            &format!("{name} initialising..."),
        )?;
        let session = host
            .session(player)
            .ok_or_else(|| Error::Exception("NullPointerException: details.session".into()))?;
        host.set_session_object(&session, Some(player.clone()));
    }
    entity_init(host, player)?;
    configure_lobby(host, repository, player)?;
    host.set_attribute_bool(player, "logged-in-fully", true)
}

/// `Entity.init()`: active, then the automatic timers.
pub fn entity_init<P, H: Host<P>>(host: &mut H, player: &P) -> Result<()> {
    host.set_active(player, true);
    host.add_auto_timers(player)
}

/// `LoginConfiguration.configureLobby(player)`.
pub fn configure_lobby<P: Clone + Eq + Hash, H: Host<P>>(
    host: &mut H,
    repository: &Repository<P>,
    player: &P,
) -> Result<()> {
    host.update_scene_graph(player, true)?;
    if !host.is_artificial(player)
        && host.attribute_bool(player, "tutorial:complete", false)?
        && host.login_type_not_reconnect(player)?
    {
        send_lobby_screen(host, repository, player)
    } else {
        configure_game_world(host, repository, player)
    }
}

/// `LoginConfiguration.sendLobbyScreen(player)`.
pub fn send_lobby_screen<P: Clone + Eq + Hash, H: Host<P>>(
    host: &mut H,
    repository: &Repository<P>,
    player: &P,
) -> Result<()> {
    let constants = host.constants();
    let model = auto_select(host);
    let lobby = repository.lobby_players();
    let name = host.name(player);
    for other in lobby.values() {
        if host.name(&other) == name {
            host.player_clear(repository, &other)?;
            lobby.remove(&other);
            break;
        }
    }
    lobby.add(player.clone());
    let server = constants.server_name.clone();
    // Each text is evaluated when its call is reached: getLastLogin sends its
    // packet between the first and second text.
    host.set_interface_text(player, &format!("Welcome to {server}"), 378, 115)?;
    let last_login = last_login_text(host, player)?;
    host.set_interface_text(player, &last_login, 378, 116)?;
    for (text, interface, child) in [
        ("", 378, 37),
        (
            "Want to stay up to date with the latest news and updates? Join our discord by using the link on our website!",
            378,
            38,
        ),
        ("", 378, 39),
        ("Discord Invite", 378, 14),
        ("Discord Invite", 378, 129),
        (
            "You can gain more credits by testing new features and other methods of contribution.",
            378,
            93,
        ),
    ] {
        host.set_interface_text(player, text, interface, child)?;
    }
    let credits = host.with_details(player, |details| {
        crate::auth::storage::lock(&details.account).credits
    })?;
    host.set_interface_text(player, &credits.to_string(), 378, 96)?;
    for (text, interface, child) in [("Credits", 378, 94), ("", 378, 229)] {
        host.set_interface_text(player, text, interface, child)?;
    }
    let contribute =
        format!("Want to contribute to {server}? Visit the GitLab using the link on our website!");
    host.set_interface_text(player, &contribute, 378, 230)?;
    for (text, interface, child) in [("", 378, 231), ("Github", 378, 240)] {
        host.set_interface_text(player, text, interface, child)?;
    }
    host.set_interface_text(
        player,
        &constants.message_string,
        model,
        message_child(model),
    )?;
    let (ui, manager) = host.ui(player)?;
    let pane = ui.component(LOBBY_PANE)?;
    interfaces::open_windows(ui, &manager, Some(pane), Open::Overlap(false))?;
    let opened = ui.component(LOBBY_INTERFACE)?;
    manager.fields.write().opened = Some(opened);
    interfaces::send_interface(ui, &manager, LOBBY_PANE, 2, LOBBY_INTERFACE, true)?;
    interfaces::send_interface(ui, &manager, LOBBY_PANE, 3, model, true)?;
    let now = host.now_millis();
    host.set_last_login(player, now)
}

/// `LoginConfiguration.configureGameWorld(player)`.
pub fn configure_game_world<P: Clone + Eq + Hash, H: Host<P>>(
    host: &mut H,
    repository: &Repository<P>,
    player: &P,
) -> Result<()> {
    send_game_configuration(host, player)?;
    repository.lobby_players().remove(player);
    host.set_playing(player, true);
    host.renderable_players_add(player)?;
    host.region_move(player)?;
    host.music_init(player)?;
    host.update_appearance(player)?;
    host.set_update_scene_graph_flag(player, true)?;
    {
        let (ui, manager) = host.ui(player)?;
        ui.interface_config(&manager, 226, 1, true)?;
    }
    if host.test_stage(player)? == 3 && !host.safety_first_unlocked(player)? {
        host.unlock_safety_first(player)?;
    }
    let saved_attack_style = host.attack_style_index(player)?;
    let items = host.equipment_items(player)?;
    for (slot, item) in items.iter().enumerate() {
        if item.is_none() {
            continue;
        }
        host.rerun_equip_hooks(player, slot, saved_attack_style)?;
    }
    let book = host.spell_book(player)?;
    let current = spell_book_for_interface(book);
    if current == Some(ANCIENT) && !host.has_requirement(player, "DESERT_TREASURE", true)? {
        let text = colorize_red(
            "As you can no longer use Ancient Magic, you have been set back to Modern.",
        );
        host.player_message(player, &text)?;
        host.set_spell_book(player, MODERN)?;
    } else if current == Some(LUNAR) && !host.has_requirement(player, "LUNAR_DIPLOMACY", true)? {
        let text =
            colorize_red("As you can no longer use Lunar Magic, you have been set back to Modern.");
        host.player_message(player, &text)?;
        host.set_spell_book(player, MODERN)?;
    }
    spell_book_update(host, player)?;
    if host.has_requirement(player, "KINGS_RANSOM", false)? {
        host.set_varbit(player, 3909, 8, false)?;
    }
    if host.constants().rules_and_info_enabled {
        host.rules_and_info(player)?;
    }
    Ok(())
}

/// `SpellBook.forInterface(id)`: the book's interface when it is one of the
/// three, else null.
pub fn spell_book_for_interface(id: i32) -> Option<i32> {
    [MODERN, ANCIENT, LUNAR]
        .into_iter()
        .find(|&book| book == id)
}

/// `SpellBookManager.update(player)`: opens the book's tab, then varbit 357
/// to the book's ordinal. A book that is none of the three fails as
/// `Objects.requireNonNull` does, after the tab was opened.
pub fn spell_book_update<P, H: Host<P>>(host: &mut H, player: &P) -> Result<()> {
    let book = host.spell_book(player)?;
    {
        let (ui, manager) = host.ui(player)?;
        let tab = ui.component(book)?;
        interfaces::open_tab_auto(ui, &manager, &tab)?;
    }
    let ordinal = match spell_book_for_interface(book) {
        Some(MODERN) => 0,
        Some(ANCIENT) => 1,
        Some(_) => 2,
        None => {
            return Err(Error::Exception(
                "NullPointerException: SpellBook.forInterface".into(),
            ));
        }
    };
    host.set_varbit(player, 357, ordinal, false)
}

/// `LoginConfiguration.sendGameConfiguration(player)`: the windows pane of the
/// client's mode, the chatbox, the default tabs, the welcome, the
/// configuration, the login plugins (each failure printed and passed over),
/// then the appearance's synchronisation. The interface steps are those of
/// [`interfaces::send_game_configuration`].
pub fn send_game_configuration<P: Clone + Eq + Hash, H: Host<P>>(
    host: &mut H,
    player: &P,
) -> Result<()> {
    {
        let (ui, manager) = host.ui(player)?;
        let id = if ui.resizable()? { 746 } else { 548 };
        let pane = ui.component(id)?;
        interfaces::open_windows(ui, &manager, Some(pane), Open::Overlap(false))?;
        let chatbox = ui.component(137)?;
        interfaces::open_chatbox(ui, &manager, &chatbox)?;
        interfaces::open_default_tabs(ui, &manager)?;
    }
    welcome(host, player)?;
    config(host, player)?;
    for plugin in host.login_plugins() {
        match host.login_plugin(&plugin, player) {
            Ok(()) => {}
            Err(error) if error.is_throwable() => host.print_stack(&error),
            Err(error) => return Err(error),
        }
    }
    host.appearance_sync(player)
}

/// `LoginConfiguration.welcome(player)`.
pub fn welcome<P, H: Host<P>>(host: &mut H, player: &P) -> Result<()> {
    if host.is_artificial(player) {
        return Ok(());
    }
    let server = host.constants().server_name;
    host.dispatch_message(player, &format!("Welcome to {server}."))?;
    if host.with_details(player, |details| details.is_muted())? {
        host.dispatch_message(player, "You are muted.")?;
        host.dispatch_message(player, "To prevent further mutes please read the rules.")?;
    }
    Ok(())
}

/// `LoginConfiguration.config(player)`.
pub fn config<P: Clone + Eq + Hash, H: Host<P>>(host: &mut H, player: &P) -> Result<()> {
    if !host.is_artificial(player) {
        let name = username(host, player)?;
        host.log_line(ORIGIN, Level::Info, &format!("configuring player {name}"))?;
    }
    host.inventory_refresh(player)?;
    host.equipment_refresh(player)?;
    host.skills_refresh(player)?;
    host.skills_configure(player)?;
    host.settings_update(player)?;
    host.interaction_set_default(player)?;
    host.send_run_energy(player)?;
    host.familiar_login(player)?;
    {
        let (ui, manager) = host.ui(player)?;
        interfaces::open_default_tabs(ui, &manager)?;
    }
    let constants = host.constants();
    let friends = format!(
        "Friends List - {} {}",
        constants.server_name, constants.world_id
    );
    host.set_interface_text(player, &friends, 550, 3)?;
    let logout = format!(
        "When you have finished playing {}, always use the button below to logout safely.",
        constants.server_name
    );
    host.set_interface_text(player, &logout, 182, 0)?;
    host.quest_synchronize_tab(player)?;
    {
        let (ui, manager) = host.ui(player)?;
        interfaces::close(ui, &manager)?;
    }
    host.emote_refresh(player)?;
    {
        let (ui, manager) = host.ui(player)?;
        interfaces::open_info_bars(ui, &manager)?;
    }
    if !host.is_artificial(player) {
        let name = username(host, player)?;
        host.log_line(
            ORIGIN,
            Level::Info,
            &format!("finished configuring player {name}"),
        )?;
    }
    Ok(())
}

/// `LoginConfiguration.getMessageChild(interfaceId)`.
pub fn message_child(interface: i32) -> i32 {
    match interface {
        622 => 8,
        16 => 6,
        20 | 623 => 5,
        15 | 18 | 19 | 21 | 22 | 447 | 405 => 4,
        17 | 23 | 800 => 3,
        715 => 2,
        679 => 1,
        _ => 0,
    }
}

/// `LoginConfiguration.autoSelect()`: the configured model when it is one of
/// `MESSAGE_MODEL`, otherwise one drawn by an unseeded `new Random()`.
pub fn auto_select<P, H: Host<P>>(host: &mut H) -> i32 {
    let configured = host.constants().message_model;
    if MESSAGE_MODEL.contains(&configured) {
        return configured;
    }
    let seed = host.random_seed();
    let index = CountingRandom::new(seed)
        .next_bound(MESSAGE_MODEL.len() as i32)
        .unwrap_or(0);
    MESSAGE_MODEL[index as usize]
}

/// `LoginConfiguration.getLastLogin(player)`: sends LastLoginInfo, then the
/// days since the stored last login, truncated toward zero.
pub fn last_login_text<P, H: Host<P>>(host: &mut H, player: &P) -> Result<String> {
    host.send_last_login_info(player)?;
    let time = host.with_details(player, |details| {
        crate::auth::storage::lock(&details.account).last_login
    })?;
    let current = host.now_millis();
    let diff = current.wrapping_sub(time);
    let days = (diff / 86_400_000) as i32;
    let ago = if days < 1 {
        "earlier today".to_owned()
    } else if days == 1 {
        "yesterday".to_owned()
    } else {
        format!("{days} days ago")
    };
    Ok(format!("You last logged in {ago} from:"))
}

/// `colorize("%R" + text)` for a text without other markers: `RED`, the text,
/// `"</col>"` and the trailing space `colorize` appends.
pub fn colorize_red(text: &str) -> String {
    format!("<col=ff0000>{text}</col> ")
}
