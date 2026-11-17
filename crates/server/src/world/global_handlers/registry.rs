//! The classes' registrations and the typed callbacks they register.
//!
//! Each Kotlin lambda, function reference, ComponentPlugin and OptionHandler
//! the original registers is a typed native value here: [`Listener`]
//! (InterfaceListeners), [`Interaction`] (InteractionListeners),
//! [`ComponentPlugin`] (ComponentDefinition plugins), [`OptionPlugin`]
//! (option handlers) and [`LoginPlugin`]. [`define`] performs a class's
//! registration in the original's order through a [`Registrar`];
//! [`register_scanned`] walks the start-up scan manifest (content interfaces,
//! then `@Initializable` plugins, as `ClassScanner` does) and registers every
//! class this module ports. [`Native`] is the registrar over the native
//! registries. The `run_*` functions dispatch a registered value to its body.
//! Class names are real program metadata (the registries' duplicate and
//! instant-class rules print them); they never select behaviour.
use super::{Error, Host, Result, SpellBook, ifaces, npc, player, scenery, tabs};
use crate::definitions::option_handler::{self, NativeOptionHandler};
use crate::world::combat_swing::WeaponInterfaces;
use crate::world::component_objects::{NativeObject, Plugin as Object};
use crate::world::interaction_listeners::{self as interactions, Kind as IntType};
use crate::world::interface_listeners::{self, Button, Callback};

/// An InterfaceListeners callback (one per Kotlin lambda or function reference).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Listener {
    Logout,
    Settings,
    Stats,
    SkillGuide,
    LevelUp,
    ItemsKeptOnDeath,
    WornItems,
    EquipScreenOpen,
    EquipScreen,
    EquipScreenClose,
    EquipInventoryOpen,
    EquipInventory,
    Magic(SpellBook),
    Prayer,
    QuestJournal,
    AreaTasks,
    ClanJoin,
    ClanSetup,
    Emotes,
    Music,
    FilterButtons,
    RunButton,
    TopLevelFullscreen,
    TopLevel,
    Welcome,
    WelcomeClose,
    Death,
    WorldMap,
    RulesClose,
    ItemSelectOpen,
    ItemSelect,
    ItemSelectClose,
    Teleother,
    DepositBoxOpen,
    DepositBoxMenu,
    DepositBoxBeast,
    BankCharge,
}
impl NativeObject for Listener {}

const IFACE: &str = "content.global.handlers.iface.";

impl Listener {
    /// The Kotlin class the original compiles the callback to.
    pub fn class(self) -> String {
        let (owner, n) = match self {
            Self::Logout => ("tabs.LogoutTabInterface", "1"),
            Self::Settings => ("tabs.SettingsTabInterface", "1"),
            Self::Stats => ("tabs.StatsTabInterface", "1"),
            Self::SkillGuide => ("tabs.StatsTabInterface", "2"),
            Self::LevelUp => ("tabs.StatsTabInterface", "3"),
            Self::ItemsKeptOnDeath => ("tabs.EquipmentTabInterface", "1"),
            Self::WornItems => ("tabs.EquipmentTabInterface", "2"),
            Self::EquipScreenOpen => ("tabs.EquipmentTabInterface", "3"),
            Self::EquipScreen => ("tabs.EquipmentTabInterface", "4"),
            Self::EquipScreenClose => ("tabs.EquipmentTabInterface", "5"),
            Self::EquipInventoryOpen => ("tabs.EquipmentTabInterface", "6"),
            Self::EquipInventory => ("tabs.EquipmentTabInterface", "7"),
            Self::Magic(_) => ("tabs.MagicTabInterface", "1$1"),
            Self::Prayer => ("tabs.PrayerTabInterface", "1"),
            Self::QuestJournal => ("tabs.QuestTabInterface", "1"),
            Self::AreaTasks => ("tabs.QuestTabInterface", "2"),
            Self::ClanJoin => ("tabs.ClanTabInterface", "1"),
            Self::ClanSetup => ("tabs.ClanTabInterface", "2"),
            Self::Emotes => ("tabs.EmoteTabInterface", "1"),
            Self::Music => ("tabs.MusicTabInterface", "1"),
            Self::FilterButtons => ("MainGameInterface", "1"),
            Self::RunButton => ("MainGameInterface", "2"),
            Self::TopLevelFullscreen => ("MainGameInterface", "3"),
            Self::TopLevel => ("MainGameInterface", "4"),
            Self::Welcome => ("LoginInterface", "1"),
            Self::WelcomeClose => ("LoginInterface", "2"),
            Self::Death => ("DeathInterface", "1"),
            Self::WorldMap => ("WorldMapInterface", "1"),
            Self::RulesClose => ("RulesListener", "1"),
            Self::ItemSelectOpen => ("GenericItemSelect", "1"),
            Self::ItemSelect => ("GenericItemSelect", "2"),
            Self::ItemSelectClose => ("GenericItemSelect", "3"),
            Self::Teleother => ("TeleotherInterface", "1"),
            Self::DepositBoxOpen => ("bank.BankDepositBoxInterface", "1"),
            Self::DepositBoxMenu => ("bank.BankDepositBoxInterface", "2"),
            Self::DepositBoxBeast => ("bank.BankDepositBoxInterface", "3"),
            Self::BankCharge => ("bank.BankChargeInterface", "1"),
        };
        format!("{IFACE}{owner}$defineInterfaceListeners${n}")
    }
}

/// An InteractionListeners function.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interaction {
    Trade,
    Assist,
    BoothQuick,
    BoothRegular,
    BoothCollect,
    BoothUse,
    Chest,
    DepositBox,
    PeerDeposit,
}
impl NativeObject for Interaction {}
impl Interaction {
    pub fn class(self) -> &'static str {
        match self {
            Self::Trade => "content.global.handlers.player.RequestOption$defineListeners$1",
            Self::Assist => "content.global.handlers.player.RequestOption$defineListeners$2",
            Self::BoothQuick => {
                "content.global.handlers.scenery.BankBoothListener$defineListeners$1"
            }
            Self::BoothRegular => {
                "content.global.handlers.scenery.BankBoothListener$defineListeners$2"
            }
            Self::BoothCollect => {
                "content.global.handlers.scenery.BankBoothListener$defineListeners$3"
            }
            Self::BoothUse => "content.global.handlers.scenery.BankBoothListener$defineListeners$4",
            Self::Chest => "content.global.handlers.scenery.BankChestListener$defineListeners$1",
            Self::DepositBox => {
                "content.global.handlers.scenery.BankDepositBoxListener$defineListeners$1"
            }
            Self::PeerDeposit => "content.global.handlers.npc.NPCDepositListener$defineListeners$1",
        }
    }
    /// The registry's function kind (`on`, `defineInteraction`, `onUseAnyWith`).
    pub fn kind(self) -> interactions::FunctionKind {
        match self {
            Self::Trade | Self::Assist | Self::PeerDeposit => interactions::FunctionKind::On,
            Self::BoothUse => interactions::FunctionKind::UseWith,
            _ => interactions::FunctionKind::Interaction,
        }
    }
}

/// A ComponentPlugin (one object per `newInstance`, shared by its ids).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentPlugin {
    CombatTab,
    CharacterDesign,
    Experience,
    AutocastSelect,
    RequestAssist,
}
impl NativeObject for ComponentPlugin {}
impl ComponentPlugin {
    pub fn class(self) -> &'static str {
        match self {
            Self::CombatTab => "content.global.handlers.iface.tabs.CombatTabInterface",
            Self::CharacterDesign => "content.global.handlers.iface.CharacterDesignInterface",
            Self::Experience => "content.global.handlers.iface.ExperienceInterface",
            Self::AutocastSelect => "content.global.handlers.iface.AutocastSelectPlugin",
            Self::RequestAssist => "content.global.handlers.iface.RequestAssistInterface",
        }
    }
}

/// An OptionHandler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptionPlugin {
    Attack,
    Follow,
    LookAt,
    Search,
    Ladder,
    Dummy,
}
impl NativeOptionHandler for OptionPlugin {}
impl OptionPlugin {
    pub fn class(self) -> &'static str {
        match self {
            Self::Attack => "content.global.handlers.player.AttackOptionPlugin",
            Self::Follow => "content.global.handlers.player.FollowOptionPlugin",
            Self::LookAt => "content.global.handlers.scenery.LookAtOptionPlugin",
            Self::Search => "content.global.handlers.scenery.SearchOptionPlugin",
            Self::Ladder => "content.global.handlers.scenery.LadderManagingPlugin",
            Self::Dummy => "content.global.handlers.npc.DummyAttackPlugin",
        }
    }
    /// `isWalk()`: false only for AttackOptionPlugin.
    pub fn is_walk(self) -> bool {
        self != Self::Attack
    }
    /// `isWalk(player, node)`: the base answer.
    pub fn is_walk_for(self) -> bool {
        false
    }
    /// `isDelayed(player)`: false only for AttackOptionPlugin.
    pub fn is_delayed(self) -> bool {
        self != Self::Attack
    }
    /// The native option handler object for the definition registries.
    pub fn native_handler(self) -> option_handler::Handle {
        option_handler::Handler::new(self.class(), self)
    }
}

/// A LOGIN-type plugin (`LoginConfiguration.getLoginPlugins().add`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginPlugin {
    Validation,
}

/// The ported classes, by their original names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    AttackOption,
    FollowOption,
    LoginValidation,
    RequestOption,
    LogoutTab,
    SettingsTab,
    StatsTab,
    EquipmentTab,
    CombatTab,
    MagicTab,
    PrayerTab,
    QuestTab,
    ClanTab,
    EmoteTab,
    MusicTab,
    MainGame,
    Login,
    Death,
    CharacterDesign,
    WorldMap,
    Experience,
    Rules,
    ItemSelect,
    AutocastSelect,
    Teleother,
    RequestAssist,
    DepositBoxInterface,
    BankCharge,
    BankBooth,
    BankChest,
    BankDepositBox,
    LookAt,
    Search,
    Ladder,
    NpcDeposit,
    DummyAttack,
}
impl Class {
    pub const ALL: [Self; 36] = [
        Self::AttackOption,
        Self::FollowOption,
        Self::LoginValidation,
        Self::RequestOption,
        Self::LogoutTab,
        Self::SettingsTab,
        Self::StatsTab,
        Self::EquipmentTab,
        Self::CombatTab,
        Self::MagicTab,
        Self::PrayerTab,
        Self::QuestTab,
        Self::ClanTab,
        Self::EmoteTab,
        Self::MusicTab,
        Self::MainGame,
        Self::Login,
        Self::Death,
        Self::CharacterDesign,
        Self::WorldMap,
        Self::Experience,
        Self::Rules,
        Self::ItemSelect,
        Self::AutocastSelect,
        Self::Teleother,
        Self::RequestAssist,
        Self::DepositBoxInterface,
        Self::BankCharge,
        Self::BankBooth,
        Self::BankChest,
        Self::BankDepositBox,
        Self::LookAt,
        Self::Search,
        Self::Ladder,
        Self::NpcDeposit,
        Self::DummyAttack,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::AttackOption => "content.global.handlers.player.AttackOptionPlugin",
            Self::FollowOption => "content.global.handlers.player.FollowOptionPlugin",
            Self::LoginValidation => "content.global.handlers.player.LoginValidationPlugin",
            Self::RequestOption => "content.global.handlers.player.RequestOption",
            Self::LogoutTab => "content.global.handlers.iface.tabs.LogoutTabInterface",
            Self::SettingsTab => "content.global.handlers.iface.tabs.SettingsTabInterface",
            Self::StatsTab => "content.global.handlers.iface.tabs.StatsTabInterface",
            Self::EquipmentTab => "content.global.handlers.iface.tabs.EquipmentTabInterface",
            Self::CombatTab => "content.global.handlers.iface.tabs.CombatTabInterface",
            Self::MagicTab => "content.global.handlers.iface.tabs.MagicTabInterface",
            Self::PrayerTab => "content.global.handlers.iface.tabs.PrayerTabInterface",
            Self::QuestTab => "content.global.handlers.iface.tabs.QuestTabInterface",
            Self::ClanTab => "content.global.handlers.iface.tabs.ClanTabInterface",
            Self::EmoteTab => "content.global.handlers.iface.tabs.EmoteTabInterface",
            Self::MusicTab => "content.global.handlers.iface.tabs.MusicTabInterface",
            Self::MainGame => "content.global.handlers.iface.MainGameInterface",
            Self::Login => "content.global.handlers.iface.LoginInterface",
            Self::Death => "content.global.handlers.iface.DeathInterface",
            Self::CharacterDesign => "content.global.handlers.iface.CharacterDesignInterface",
            Self::WorldMap => "content.global.handlers.iface.WorldMapInterface",
            Self::Experience => "content.global.handlers.iface.ExperienceInterface",
            Self::Rules => "content.global.handlers.iface.RulesListener",
            Self::ItemSelect => "content.global.handlers.iface.GenericItemSelect",
            Self::AutocastSelect => "content.global.handlers.iface.AutocastSelectPlugin",
            Self::Teleother => "content.global.handlers.iface.TeleotherInterface",
            Self::RequestAssist => "content.global.handlers.iface.RequestAssistInterface",
            Self::DepositBoxInterface => {
                "content.global.handlers.iface.bank.BankDepositBoxInterface"
            }
            Self::BankCharge => "content.global.handlers.iface.bank.BankChargeInterface",
            Self::BankBooth => "content.global.handlers.scenery.BankBoothListener",
            Self::BankChest => "content.global.handlers.scenery.BankChestListener",
            Self::BankDepositBox => "content.global.handlers.scenery.BankDepositBoxListener",
            Self::LookAt => "content.global.handlers.scenery.LookAtOptionPlugin",
            Self::Search => "content.global.handlers.scenery.SearchOptionPlugin",
            Self::Ladder => "content.global.handlers.scenery.LadderManagingPlugin",
            Self::NpcDeposit => "content.global.handlers.npc.NPCDepositListener",
            Self::DummyAttack => "content.global.handlers.npc.DummyAttackPlugin",
        }
    }
    /// The class of a scanned name, if this module ports it.
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.name() == name)
    }
}

/// The registries a class registers into, in the original's terms.
pub trait Registrar {
    /// A ComponentPlugin object (`new X()`).
    type PluginRef: Clone;
    /// An OptionHandler object (`new X()`).
    type HandlerRef: Clone;
    /// Constructs a class's ComponentPlugin object.
    fn plugin_object(&mut self, plugin: ComponentPlugin) -> Self::PluginRef;
    /// Constructs a class's OptionHandler object.
    fn handler_object(&mut self, handler: OptionPlugin) -> Self::HandlerRef;
    /// `InterfaceListeners.add(component[, button], handler)`.
    fn on_button(&mut self, component: i32, button: Option<i32>, listener: Listener) -> Result<()>;
    /// `InterfaceListeners.addOpenListener`.
    fn on_open(&mut self, component: i32, listener: Listener) -> Result<()>;
    /// `InterfaceListeners.addCloseListener`.
    fn on_close(&mut self, component: i32, listener: Listener) -> Result<()>;
    /// `ComponentDefinition.forId(id).setPlugin(plugin)` (`put` is the same).
    fn set_component_plugin(&mut self, id: i32, plugin: &Self::PluginRef) -> Result<()>;
    /// `Option._P_X.setHandler(handler)` (the option's name).
    fn set_player_option(&mut self, option: &str, handler: &Self::HandlerRef) -> Result<()>;
    /// `SceneryDefinition.setOptionHandler(name, handler)`.
    fn set_scenery_option(&mut self, option: &str, handler: &Self::HandlerRef) -> Result<()>;
    /// `SceneryDefinition.forId(id).getHandlers().put(key, handler)`.
    fn put_scenery_handler(&mut self, id: i32, key: &str, handler: &Self::HandlerRef)
    -> Result<()>;
    /// `InteractionListener.on(...)`: ids (none for a catch-all), type, options.
    fn on(
        &mut self,
        ids: Option<&[i32]>,
        kind: IntType,
        options: &[&str],
        f: Interaction,
    ) -> Result<()>;
    /// `defineInteraction(type, ids, options..., persistent, allowedDistance, handler)`.
    fn define_interaction(
        &mut self,
        kind: IntType,
        ids: &[i32],
        options: &[&str],
        persistent: bool,
        distance: i32,
        f: Interaction,
    ) -> Result<()>;
    /// `onUseAnyWith(type, ids..., handler)`.
    fn on_use_any_with(&mut self, kind: IntType, ids: &[i32], f: Interaction) -> Result<()>;
    /// `LoginConfiguration.getLoginPlugins().add(plugin)`.
    fn login_plugin(&mut self, plugin: LoginPlugin) -> Result<()>;
    /// `ServerConstants.BANK_BOOTH_NOTE_ENABLED` at registration.
    fn booth_note_enabled(&mut self) -> bool;
}

/// A class's `defineInterfaceListeners`/`defineListeners`/`newInstance(null)`
/// (LoginValidationPlugin: added to the login plugins, as `definePlugin` does
/// for a LOGIN manifest).
pub fn define<R: Registrar + ?Sized>(r: &mut R, class: Class) -> Result<()> {
    use Listener as L;
    match class {
        Class::AttackOption => {
            let h = r.handler_object(OptionPlugin::Attack);
            r.set_player_option("Attack", &h)
        }
        Class::FollowOption => {
            let h = r.handler_object(OptionPlugin::Follow);
            r.set_player_option("Follow", &h)
        }
        Class::LoginValidation => r.login_plugin(LoginPlugin::Validation),
        Class::RequestOption => {
            r.on(None, IntType::Player, &["Trade with"], Interaction::Trade)?;
            r.on(None, IntType::Player, &["Req Assist"], Interaction::Assist)
        }
        Class::LogoutTab => r.on_button(182, None, L::Logout),
        Class::SettingsTab => r.on_button(261, None, L::Settings),
        Class::StatsTab => {
            r.on_button(320, None, L::Stats)?;
            r.on_button(499, None, L::SkillGuide)?;
            r.on_button(740, None, L::LevelUp)
        }
        Class::EquipmentTab => {
            r.on_open(tabs::ITEMS_KEPT_ON_DEATH, L::ItemsKeptOnDeath)?;
            r.on_button(387, None, L::WornItems)?;
            r.on_open(667, L::EquipScreenOpen)?;
            r.on_button(667, None, L::EquipScreen)?;
            r.on_close(667, L::EquipScreenClose)?;
            r.on_open(670, L::EquipInventoryOpen)?;
            r.on_button(670, None, L::EquipInventory)
        }
        Class::CombatTab => {
            let plugin = r.plugin_object(ComponentPlugin::CombatTab);
            for w in WeaponInterfaces::ALL {
                r.set_component_plugin(w.interface_id(), &plugin)?;
            }
            r.set_component_plugin(92, &plugin)
        }
        Class::MagicTab => {
            for book in SpellBook::ALL {
                r.on_button(book.interface_id(), None, L::Magic(book))?;
            }
            Ok(())
        }
        Class::PrayerTab => r.on_button(271, None, L::Prayer),
        Class::QuestTab => {
            r.on_button(274, None, L::QuestJournal)?;
            r.on_button(259, None, L::AreaTasks)
        }
        Class::ClanTab => {
            r.on_button(589, None, L::ClanJoin)?;
            r.on_button(590, None, L::ClanSetup)
        }
        Class::EmoteTab => r.on_button(464, None, L::Emotes),
        Class::MusicTab => r.on_button(187, None, L::Music),
        Class::MainGame => {
            r.on_button(751, None, L::FilterButtons)?;
            r.on_button(750, None, L::RunButton)?;
            r.on_button(746, None, L::TopLevelFullscreen)?;
            r.on_button(548, None, L::TopLevel)
        }
        Class::Login => {
            r.on_button(378, None, L::Welcome)?;
            r.on_close(378, L::WelcomeClose)
        }
        Class::Death => r.on_button(153, None, L::Death),
        Class::CharacterDesign => {
            let plugin = r.plugin_object(ComponentPlugin::CharacterDesign);
            r.set_component_plugin(771, &plugin)
        }
        Class::WorldMap => r.on_button(755, None, L::WorldMap),
        Class::Experience => {
            let plugin = r.plugin_object(ComponentPlugin::Experience);
            r.set_component_plugin(134, &plugin)
        }
        Class::Rules => r.on_close(384, L::RulesClose),
        Class::ItemSelect => {
            r.on_open(12, L::ItemSelectOpen)?;
            r.on_button(12, None, L::ItemSelect)?;
            r.on_close(12, L::ItemSelectClose)
        }
        Class::AutocastSelect => {
            let plugin = r.plugin_object(ComponentPlugin::AutocastSelect);
            for id in [797, 319, 310, 406] {
                r.set_component_plugin(id, &plugin)?;
            }
            Ok(())
        }
        Class::Teleother => r.on_button(326, None, L::Teleother),
        Class::RequestAssist => {
            let plugin = r.plugin_object(ComponentPlugin::RequestAssist);
            r.set_component_plugin(301, &plugin)
        }
        Class::DepositBoxInterface => {
            r.on_open(11, L::DepositBoxOpen)?;
            r.on_button(11, None, L::DepositBoxMenu)?;
            r.on_button(11, Some(13), L::DepositBoxBeast)
        }
        Class::BankCharge => r.on_button(619, None, L::BankCharge),
        Class::BankBooth => {
            let booths = &scenery::BANK_BOOTHS;
            r.define_interaction(
                IntType::Scenery,
                booths,
                &["use-quickly", "bank"],
                false,
                1,
                Interaction::BoothQuick,
            )?;
            r.define_interaction(
                IntType::Scenery,
                booths,
                &["use"],
                false,
                1,
                Interaction::BoothRegular,
            )?;
            r.define_interaction(
                IntType::Scenery,
                booths,
                &["collect"],
                false,
                1,
                Interaction::BoothCollect,
            )?;
            if r.booth_note_enabled() {
                r.on_use_any_with(IntType::Scenery, booths, Interaction::BoothUse)?;
            }
            Ok(())
        }
        Class::BankChest => r.define_interaction(
            IntType::Scenery,
            &scenery::BANK_CHESTS,
            &["bank", "use"],
            false,
            1,
            Interaction::Chest,
        ),
        Class::BankDepositBox => r.define_interaction(
            IntType::Scenery,
            &scenery::BANK_DEPOSIT_BOXES,
            &["deposit"],
            false,
            1,
            Interaction::DepositBox,
        ),
        Class::LookAt => {
            let h = r.handler_object(OptionPlugin::LookAt);
            for id in 18877..=18900 {
                r.put_scenery_handler(id, "option:look at", &h)?;
            }
            Ok(())
        }
        Class::Search => {
            let h = r.handler_object(OptionPlugin::Search);
            r.set_scenery_option("search", &h)
        }
        Class::Ladder => {
            let h = r.handler_object(OptionPlugin::Ladder);
            for option in ["climb-up", "climb-down", "climb", "walk-up", "walk-down"] {
                r.set_scenery_option(option, &h)?;
            }
            Ok(())
        }
        Class::NpcDeposit => r.on(
            Some(&[npc::PEER_THE_SEER]),
            IntType::Npc,
            &["deposit"],
            Interaction::PeerDeposit,
        ),
        Class::DummyAttack => {
            let h = r.handler_object(OptionPlugin::Dummy);
            for id in npc::DUMMIES {
                r.put_scenery_handler(id, "option:attack", &h)?;
            }
            Ok(())
        }
    }
}

/// Registers every ported class in the original's order: the start-up
/// manifest's content interfaces (`loadPureInterfaces`), then its
/// `@Initializable` classes (`loadSideEffectfulPlugins`: only Plugins are
/// defined, so RequestOption's second entry does nothing). Returns the
/// classes registered.
pub fn register_scanned<R: Registrar + ?Sized>(r: &mut R) -> Result<Vec<Class>> {
    let mut done = Vec::new();
    for name in crate::startup::scan::section("scan.contentInterfaces") {
        if let Some(class) = Class::named(name) {
            define(r, class)?;
            done.push(class);
        }
    }
    for name in crate::startup::scan::section("scan.initializable") {
        match Class::named(name) {
            None | Some(Class::RequestOption) => {}
            Some(class) => {
                define(r, class)?;
                done.push(class);
            }
        }
    }
    Ok(done)
}

// ------------------------------------------------------------------ dispatch

/// Runs an InterfaceListeners button callback.
pub fn run_button<H: Host + ?Sized>(
    h: &mut H,
    listener: Listener,
    p: &H::Node,
    b: Button,
) -> Result<bool> {
    use Listener as L;
    match listener {
        L::Logout => tabs::logout(h, p, b),
        L::Settings => tabs::settings(h, p, b),
        L::Stats => tabs::stats(h, p, b),
        L::SkillGuide => tabs::skill_guide(h, p, b),
        L::LevelUp => tabs::level_up(h, p, b),
        L::WornItems => tabs::worn_items(h, p, b),
        L::EquipScreen => tabs::equip_screen(h, p, b),
        L::EquipInventory => tabs::equip_inventory(h, p, b),
        L::Magic(book) => tabs::magic(h, p, book, b),
        L::Prayer => tabs::prayer(h, p, b),
        L::QuestJournal => tabs::quest_journal(h, p, b),
        L::AreaTasks => tabs::area_tasks(h, p, b),
        L::ClanJoin => tabs::clan_join(h, p, b),
        L::ClanSetup => tabs::clan_setup(h, p, b),
        L::Emotes => tabs::emotes(h, p, b),
        L::Music => tabs::music(h, p, b),
        L::FilterButtons => ifaces::filter_buttons(h, p, b),
        L::RunButton => ifaces::run_button(h, p, b),
        L::TopLevelFullscreen => ifaces::top_level_fullscreen(h, p, b),
        L::TopLevel => ifaces::top_level(h, p, b),
        L::Welcome => ifaces::welcome(h, p, b),
        L::Death => ifaces::death(h, p, b),
        L::WorldMap => ifaces::world_map(h, p, b),
        L::ItemSelect => ifaces::item_select(h, p, b),
        L::Teleother => ifaces::teleother(h, p, b),
        L::DepositBoxMenu => ifaces::deposit_box_menu(h, p, b),
        L::DepositBoxBeast => ifaces::deposit_box_beast(h, p, b),
        L::BankCharge => ifaces::bank_charge(h, p, b),
        _ => Err(Error::Binding(Some(format!(
            "{listener:?} is no button listener"
        )))),
    }
}

/// Runs an open or close callback with the component's id.
pub fn run_open<H: Host + ?Sized>(
    h: &mut H,
    listener: Listener,
    p: &H::Node,
    component: i32,
) -> Result<bool> {
    use Listener as L;
    match listener {
        L::ItemsKeptOnDeath => tabs::items_kept_on_death(h, p, component),
        L::EquipScreenOpen => tabs::equip_screen_open(h, p, component),
        L::EquipScreenClose => tabs::equip_screen_close(h, p),
        L::EquipInventoryOpen => tabs::equip_inventory_open(h, p, component),
        L::WelcomeClose => ifaces::welcome_close(h, p),
        L::RulesClose => ifaces::rules_close(h, p),
        L::ItemSelectOpen => ifaces::item_select_open(h, p),
        L::ItemSelectClose => ifaces::item_select_close(h, p),
        L::DepositBoxOpen => ifaces::deposit_box_open(h, p),
        _ => Err(Error::Binding(Some(format!(
            "{listener:?} is no open or close listener"
        )))),
    }
}

/// Runs a ComponentPlugin's `handle`.
pub fn run_plugin<H: Host + ?Sized>(
    h: &mut H,
    plugin: ComponentPlugin,
    p: &H::Node,
    b: Button,
) -> Result<bool> {
    match plugin {
        ComponentPlugin::CombatTab => tabs::combat_tab(h, p, b),
        ComponentPlugin::CharacterDesign => ifaces::character_design(h, p, b),
        ComponentPlugin::Experience => ifaces::experience(h, p, b),
        ComponentPlugin::AutocastSelect => ifaces::autocast_select(h, p, b),
        ComponentPlugin::RequestAssist => ifaces::request_assist(h, p, b),
    }
}

/// Runs an OptionHandler's `handle(player, node, option)`.
pub fn run_handler<H: Host + ?Sized>(
    h: &mut H,
    handler: OptionPlugin,
    p: &H::Node,
    node: Option<&H::Node>,
    option: Option<&str>,
) -> Result<bool> {
    match handler {
        OptionPlugin::Attack => player::attack(h, p, node),
        OptionPlugin::Follow => player::follow(h, p, node),
        OptionPlugin::LookAt => scenery::look_at(h, p, node),
        OptionPlugin::Search => scenery::search(h, p, node),
        OptionPlugin::Ladder => scenery::ladder(h, p, node, option),
        OptionPlugin::Dummy => npc::dummy_attack(h, p),
    }
}

/// An OptionHandler's `getDestination(player, node)`.
pub fn handler_destination<H: Host + ?Sized>(
    h: &mut H,
    handler: OptionPlugin,
    node: Option<&H::Node>,
) -> Result<Option<[i32; 3]>> {
    match handler {
        OptionPlugin::Ladder => scenery::ladder_destination(h, node),
        _ => Ok(None),
    }
}

/// Runs an InteractionListeners function: `on` and `defineInteraction`
/// handlers get the node (and the state, unused by these), `onUseAnyWith`
/// handlers the used and the with nodes.
pub fn run_interaction<H: Host + ?Sized>(
    h: &mut H,
    f: Interaction,
    p: &H::Node,
    node: Option<&H::Node>,
    with: Option<&H::Node>,
) -> Result<bool> {
    let node_ref = || node.ok_or(Error::Null);
    match f {
        Interaction::Trade => player::trade(h, p, node),
        Interaction::Assist => player::assist(h, p, node),
        Interaction::BoothQuick => scenery::booth_quick(h, p, node_ref()?),
        Interaction::BoothRegular => scenery::booth_regular(h, p, node_ref()?),
        Interaction::BoothCollect => scenery::booth_collect(h, p, node_ref()?),
        Interaction::BoothUse => scenery::booth_use(h, p, node, with.ok_or(Error::Null)?),
        Interaction::Chest => scenery::chest(h, p),
        Interaction::DepositBox => scenery::deposit_box(h, p),
        Interaction::PeerDeposit => npc::peer_deposit(h, p),
    }
}

/// Runs a login plugin's `newInstance(player)`.
pub fn run_login<H: Host + ?Sized>(h: &mut H, plugin: LoginPlugin, p: &H::Node) -> Result<()> {
    match plugin {
        LoginPlugin::Validation => player::login_validation(h, p),
    }
}

// ------------------------------------------------------------------ the native registries

/// The registrar over the native registries: InterfaceListeners callbacks,
/// component definition plugins, the InteractionListeners registry and the
/// scenery option handler defaults. Player option handlers and per-definition
/// scenery handlers are collected for the world to bind into its option
/// tables and definitions; login plugins likewise.
pub struct Native<'a, C: interactions::Casing> {
    pub listeners: &'a interface_listeners::Registry,
    pub components: &'a mut crate::world::interfaces::Registry,
    pub interactions: &'a interactions::Registry,
    pub casing: &'a C,
    pub scenery_defaults: &'a option_handler::DefaultHandle,
    pub player_options: Vec<(String, option_handler::Handle)>,
    pub scenery_handlers: Vec<(i32, String, option_handler::Handle)>,
    pub login_plugins: Vec<LoginPlugin>,
    pub note_enabled: bool,
}

fn listener_error(e: crate::world::interfaces::Error) -> Error {
    Error::Binding(Some(format!("{e:?}")))
}
fn interaction_error(e: interactions::Error) -> Error {
    Error::Binding(Some(format!("{e:?}")))
}
fn text(s: &str) -> interactions::Text {
    s.encode_utf16().collect()
}

impl<C: interactions::Casing> Registrar for Native<'_, C> {
    type PluginRef = Object;
    type HandlerRef = option_handler::Handle;
    fn plugin_object(&mut self, plugin: ComponentPlugin) -> Object {
        Object::new(plugin)
    }
    fn handler_object(&mut self, handler: OptionPlugin) -> option_handler::Handle {
        handler.native_handler()
    }
    fn on_button(&mut self, component: i32, button: Option<i32>, listener: Listener) -> Result<()> {
        self.listeners
            .add(component, button, Callback::button(listener))
            .map_err(listener_error)
    }
    fn on_open(&mut self, component: i32, listener: Listener) -> Result<()> {
        self.listeners
            .add_open(component, Callback::open(listener))
            .map_err(listener_error)
    }
    fn on_close(&mut self, component: i32, listener: Listener) -> Result<()> {
        self.listeners
            .add_close(component, Callback::open(listener))
            .map_err(listener_error)
    }
    fn set_component_plugin(&mut self, id: i32, plugin: &Object) -> Result<()> {
        self.components.for_id(id).write().plugin = Some(plugin.clone());
        Ok(())
    }
    fn set_player_option(&mut self, option: &str, handler: &option_handler::Handle) -> Result<()> {
        self.player_options.push((option.into(), handler.clone()));
        Ok(())
    }
    fn set_scenery_option(&mut self, option: &str, handler: &option_handler::Handle) -> Result<()> {
        option_handler::set_option_handler(
            self.scenery_defaults,
            Some(option.into()),
            Some(handler.clone()),
        );
        Ok(())
    }
    fn put_scenery_handler(
        &mut self,
        id: i32,
        key: &str,
        handler: &option_handler::Handle,
    ) -> Result<()> {
        self.scenery_handlers
            .push((id, key.into(), handler.clone()));
        Ok(())
    }
    fn on(
        &mut self,
        ids: Option<&[i32]>,
        kind: IntType,
        options: &[&str],
        f: Interaction,
    ) -> Result<()> {
        let function = interactions::Function::new(f.kind(), f.class(), f);
        let options: Vec<interactions::Text> = options.iter().map(|o| text(o)).collect();
        match ids {
            Some(ids) => {
                self.interactions
                    .add_ids(self.casing, ids, kind.ordinal(), &options, function)
            }
            // The deprecated on(option, type): InteractionListeners.add(option, type, handler).
            None => options.iter().try_for_each(|option| {
                self.interactions
                    .add_generic(self.casing, option, kind.ordinal(), function.clone())
            }),
        }
        .map_err(interaction_error)
    }
    fn define_interaction(
        &mut self,
        kind: IntType,
        ids: &[i32],
        options: &[&str],
        persistent: bool,
        distance: i32,
        f: Interaction,
    ) -> Result<()> {
        let function = interactions::Function::new(f.kind(), f.class(), f);
        let meta = crate::definitions::item::Shared::new(interactions::Metadata {
            function,
            distance,
            persist: persistent,
        });
        let options: Vec<interactions::Text> = options.iter().map(|o| text(o)).collect();
        self.interactions
            .add_metadata(self.casing, ids, kind, &options, meta);
        Ok(())
    }
    fn on_use_any_with(&mut self, kind: IntType, ids: &[i32], f: Interaction) -> Result<()> {
        let function = interactions::Function::new(f.kind(), f.class(), f);
        self.interactions
            .add_any(kind.ordinal(), ids, function)
            .map_err(interaction_error)
    }
    fn login_plugin(&mut self, plugin: LoginPlugin) -> Result<()> {
        self.login_plugins.push(plugin);
        Ok(())
    }
    fn booth_note_enabled(&mut self) -> bool {
        self.note_enabled
    }
}
