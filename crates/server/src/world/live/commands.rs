//! The command system (`world::commands`, with the Misc, Spawn and Stats
//! sets of `world::commands::sets`) installed in the live world: the
//! `COMMAND` packet (opcode 44) reaches `commands::handle_packet`, which
//! decodes it, logs it through `PlayerMonitor.log` and runs
//! `CommandSystem.parse` over the registry the class scan leaves
//! ([`crate::world::commands::registration::register_all`], built at boot).
//!
//! [`CommandHost`] is `commands::Host` and `commands::sets::Host` over the
//! live world. The rights rules are the library's: `Command.attemptHandling`
//! reads `player.getRights()` from the account record the login loaded
//! (`details.accountInfo.rights`) and the `PLAYER_COMMANDS`,
//! `I_AM_A_CHEATER` and dev-mode settings of the configuration.
//!
//! What it answers natively: messages, the account record and the account
//! storage (bans, mutes, rights), the player's location, its attributes,
//! `TeleportManager.send` (`world::teleport`, the live world's teleporters),
//! `properties.teleportLocation`, the zone monitor's `parseCommand`, the
//! canonical inventory, equipment and banks (`add`, `remove`, `getAmount`,
//! `replace`, `getMaximumAdd`, ...), item definitions, the repository's
//! name lookup and online list, and the saved-data fields the sets read.
//!
//! One content body is ported here, from the pinned
//! `DevelopmentCommandSet.kt` (the `setlevel` lambda, with its
//! `getPlayerFromArgs`), over the native Skills. No JVM corpus covers it.
//!
//! Every other call is refused by name (an `Error::Binding` that starts
//! with "not ported in the live world") and listed in docs/live-world.md:
//! the not-ported command bodies (`Body::Content`), the legacy
//! `CommandPlugin`s, pulses, tasks and coroutines the bodies submit, input
//! dialogues, the system manager (update, shutdown), `player.clear()`, the
//! server store, NPC spawning, scenery, regions, interfaces, varps, books,
//! the Grand Exchange, timers and every content call.
//!
//! A field read the sets make without a `Result` (`direction`,
//! `chunk_static_objects`, ...) that this world does not answer marks the
//! command refused: the read gives a placeholder, every later call of the
//! command fails with the refusal before it can act on it, and the command
//! ends with the refusal.
use super::{
    P,
    host::Live,
    inner::{SkillsContext, placeholder_skills},
};
use crate::auth::account::Account;
use crate::inventory::{
    container::Container as InvContainer,
    item::{Handle as ItemHandle, Item},
};
use crate::player::attributes::AttributeValue;
use crate::player::skills::Skills;
use crate::world::commands::{
    self as cmd, Body, Container, ContentBody, Error, InputType, ItemRef, LegacyPlugin, Loc,
    NativeBody, Registry, Result, TeleportType, Value,
    deferred::{InputHandler, Job, Pulse, Task},
    sets::{
        self, Actor, Book, Chunk, Direction, Handler, Input, IronmanMode, Level, Lock, Npc,
        NpcDefinition, Offer, PageSet, Scenery, Timer,
    },
};
use crate::world::{doors::Location, teleport as tp, zones};
use std::sync::Arc;

/// The command system's state in the live world.
pub struct LiveCommands {
    /// `CommandMapping` as the class scan left it.
    pub registry: Arc<Registry>,
    /// `ServerConstants.PLAYER_COMMANDS`.
    pub player_commands: bool,
    /// `ServerConstants.I_AM_A_CHEATER`.
    pub i_am_a_cheater: bool,
    /// `GameWorld.settings.isBeta`.
    pub beta: bool,
    /// `ServerConstants.DATA_PATH`.
    pub data_path: Option<String>,
    /// `ServerConstants.BANK_SIZE`.
    pub bank_size: i32,
    /// `ServerConstants.NEW_PLAYER_ANNOUNCEMENT`.
    pub new_player_announcement: bool,
    /// Ported bodies this world does not run: their registry entries carry
    /// their own `origin` as a content body, so the rights check runs and
    /// the body is refused before any of it.
    pub withheld: Vec<(ContentBody, &'static str)>,
}

/// Native bodies withheld from the live world, with the reason. Each writes
/// part of its effect (an account field, a message) before reaching a call
/// this world refuses, so running it would leave a half-applied command.
const WITHHELD: &[(NativeBody, &str)] = &[
    (
        NativeBody::Ban,
        "::ban (Player.clear and the ban store pulse are not bound; the body          would write the online account's ban end before the refusal)",
    ),
    (
        NativeBody::IpBan,
        "::ipban (Player.clear, the ban store pulse and ServerStore are not          bound; the body would write ban ends before the refusal)",
    ),
    (
        NativeBody::Jail,
        "::jail (the jail pulse is not bound; the body would send both          jailing messages before the refusal)",
    ),
];

/// The class whose `setlevel` lambda is ported here.
const DEVELOPMENT: &str = "core.game.system.command.sets.DevelopmentCommandSet";
/// `setlevel`'s position in `DevelopmentCommandSet.defineCommands`.
const SETLEVEL: u16 = 38;

/// The names each legacy `CommandPlugin`'s `parse` handles (the cases of
/// its `when (name)`, pinned sources); every other name answers false.
const LEGACY_NAMES: &[(&str, &[&str])] = &[
    (
        "core.game.system.command.MapDumpCommand",
        &["mapredo", "findobj"],
    ),
    (
        "core.game.system.command.oldsys.SimpleDumpingCommands",
        &["make"],
    ),
    (
        "core.game.system.command.oldsys.VisualCommand",
        &[
            "invisible",
            "invis",
            "seti",
            "maxkc",
            "1hko",
            "anim",
            "emote",
            "render",
            "remote",
            "normalwalk",
            "gfx",
            "graphic",
            "graphics",
            "sync",
            "visual",
            "pos_graphic",
            "position_gfx",
            "pos_gfx",
            "lgfx",
            "npc",
            "npcsquad",
            "teleallowed",
            "oib",
            "char",
            "savenpc",
            "objwithanim",
            "oa",
            "object_anim",
            "obj_anim",
            "objectanim",
            "objanim",
            "inter",
            "component",
            "interface",
            "ti",
            "iconfig",
            "inter_config",
            "loop_varposition",
            "loop_anim_on_i",
            "send_i_anim",
            "loop_inter",
            "loop_iconfig",
            "loop_itemoni",
            "loop_config",
            "config_loop",
            "string",
            "loop_string",
            "string_loop",
            "loop_oa",
            "loop_anim",
            "loop_gfx",
            "removenpc",
            "pnpc",
            "itemoni",
            "hit",
            "noclip",
            "disabledisease",
            "rake",
            "full",
            "toreg",
            "testcut",
            "tryinfinity",
            "clearpatches",
        ],
    ),
];

fn refused(what: &str) -> Error {
    Error::Binding(Some(format!("not ported in the live world: {what}")))
}
fn refuse<T>(what: &str) -> Result<T> {
    Err(refused(what))
}
fn bind(e: impl std::fmt::Debug) -> Error {
    Error::Binding(Some(format!("{e:?}")))
}

/// `Calendar.MONTH == APRIL` at the start-up moment, in UTC (the original
/// reads the JVM's default time zone).
fn april(millis: i64) -> bool {
    // Days since 1970-01-01, then the civil month (the days-to-civil
    // algorithm of Howard Hinnant).
    let days = millis.div_euclid(86_400_000);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    month == 4
}

impl LiveCommands {
    /// The registry with the class scan's order; `EasterEvent` (the one
    /// `WorldEvent` that defines commands) registers only when its
    /// `checkActive` answers true at the start-up moment: April or
    /// `FORCE_EASTER_EVENTS`.
    pub fn boot(
        constants: &std::collections::BTreeMap<String, serde_json::Value>,
        values: &std::collections::BTreeMap<String, serde_json::Value>,
        start_millis: i64,
    ) -> std::result::Result<Self, String> {
        let flag = |key: &str| -> std::result::Result<bool, String> {
            constants
                .get(key)
                .and_then(serde_json::Value::as_bool)
                .ok_or_else(|| format!("configuration has no {key}"))
        };
        // `month == APRIL || FORCE_EASTER_EVENTS`: reading the static field
        // has no effect, so it is read in every month and a configuration
        // without it fails the boot whatever the date.
        let force_easter = flag("FORCE_EASTER_EVENTS")?;
        let easter = april(start_millis) || force_easter;
        let mut registry = cmd::registration::register_all(&|class| {
            class == "core.game.worldevents.holiday.easter.EasterEvent" && easter
        });
        // Re-registering a name keeps its place in the map's walk order.
        let mut withheld = Vec::new();
        for command in registry.commands().into_iter().cloned().collect::<Vec<_>>() {
            let Body::Native(native) = command.body else {
                continue;
            };
            if let Some((_, why)) = WITHHELD.iter().find(|(body, _)| *body == native) {
                withheld.push((command.origin, *why));
                registry.register(cmd::Command {
                    body: Body::Content(command.origin),
                    ..command
                });
            }
        }
        Ok(Self {
            registry: Arc::new(registry),
            player_commands: flag("PLAYER_COMMANDS")?,
            i_am_a_cheater: flag("I_AM_A_CHEATER")?,
            beta: values
                .get("isBeta")
                .and_then(serde_json::Value::as_bool)
                .ok_or("configuration has no isBeta")?,
            data_path: constants
                .get("DATA_PATH")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            // A constant of ServerConstants the configuration never sets.
            bank_size: crate::player::core_data::DEFAULT_BANK_SIZE,
            new_player_announcement: flag("NEW_PLAYER_ANNOUNCEMENT")?,
            withheld,
        })
    }
}

impl Live {
    /// The `Command` branch of `PacketProcessor.process`: the payload as
    /// the `COMMAND` decoder reads it, then `CommandSystem.parse`.
    pub fn command_packet(&mut self, player: &P, payload: &[u8]) -> Result<()> {
        let registry = self.commands.registry.clone();
        let mut host = CommandHost {
            live: self,
            poisoned: None,
        };
        let result = cmd::handle_packet(&mut host, &registry, player, payload);
        let poisoned = host.poisoned;
        let result = match (result, poisoned) {
            (Ok(()), Some(what)) => Err(refused(what)),
            (other, _) => other,
        };
        if let Err(error) = &result {
            self.inner
                .log_line(format!("Command refused or failed: {error}"));
        }
        result
    }
}

/// `commands::Host` and `commands::sets::Host` over the live world.
pub struct CommandHost<'a> {
    pub live: &'a mut Live,
    /// A field read this world does not answer: the command is refused.
    poisoned: Option<&'static str>,
}

type Services<'a> = crate::player::owned::Services<'a>;
type ItemContext<'a, 'b, 'c> = crate::inventory::player_binding::Context<'a, 'b, Services<'c>>;

impl CommandHost<'_> {
    fn gate(&self) -> Result<()> {
        match self.poisoned {
            Some(what) => refuse(what),
            None => Ok(()),
        }
    }
    fn poison(&mut self, what: &'static str) {
        self.live.inner.not_ported.skip(what);
        self.poisoned.get_or_insert(what);
    }
    fn player_only(&self, p: &P) -> Result<()> {
        if self.live.inner.players.contains_key(&p.node()) {
            Ok(())
        } else {
            refuse("a command service for an entity that is not a live player")
        }
    }
    fn container(&self, p: &P, which: Container) -> Result<InvContainer> {
        self.player_only(p)?;
        let player = &self.live.inner.canonical(p).player;
        Ok(match which {
            Container::Inventory => player.inventory(),
            Container::Equipment => player.equipment(),
            Container::Bank => player.bank().container(),
            Container::BankPrimary => player.bank_primary().container(),
            Container::BankSecondary => player.bank_secondary().container(),
        })
    }
    /// A container operation with the player's inventory services; what it
    /// queued is moved to the session.
    fn with_container<T>(
        &mut self,
        p: &P,
        which: Container,
        f: impl FnOnce(
            &mut InvContainer,
            &crate::definitions::item::Registry,
            &mut ItemContext<'_, '_, '_>,
        ) -> crate::inventory::Result<T>,
    ) -> Result<T> {
        let mut target = self.container(p, which)?;
        let canonical = self.live.inner.canonical(p).player.clone();
        let inner = &mut self.live.inner;
        let result = canonical.apply(
            &mut inner.state,
            &mut inner.frames,
            super::now_millis,
            |_, defs, ctx| f(&mut target, defs, ctx),
        );
        inner.drain(p);
        result.map_err(bind)
    }
    /// `new Item(id, amount)`.
    fn item(&self, id: i32, amount: i32) -> ItemHandle {
        crate::definitions::item::Shared::new(Item::new(id, amount, 1000, &self.live.inner.items))
    }
    fn item_ref(handle: &ItemHandle) -> ItemRef {
        let item = handle.read();
        ItemRef {
            id: item.id(),
            slot: item.index,
            charge: item.charge(),
        }
    }
    fn attribute_value(value: AttributeValue) -> Value {
        match value {
            AttributeValue::Int(v) => Value::Int(v),
            AttributeValue::Bool(v) => Value::Bool(v),
            AttributeValue::Text(v) => Value::Text(v),
            other => Value::Other(other.java_string()),
        }
    }
    fn set_value(value: Option<sets::Value<P>>) -> Result<Option<AttributeValue>> {
        Ok(match value {
            None => None,
            Some(sets::Value::Int(v)) => Some(AttributeValue::Int(v)),
            Some(sets::Value::Long(v)) => Some(AttributeValue::Long(v)),
            Some(sets::Value::Bool(v)) => Some(AttributeValue::Bool(v)),
            Some(sets::Value::Text(v)) => Some(AttributeValue::Text(v)),
            Some(_) => {
                return refuse(
                    "an attribute value of another class (rights, lists, players) from a command",
                );
            }
        })
    }
    fn get_value(value: AttributeValue) -> Result<Option<sets::Value<P>>> {
        Ok(Some(match value {
            AttributeValue::Int(v) => sets::Value::Int(v),
            AttributeValue::Long(v) => sets::Value::Long(v),
            AttributeValue::Bool(v) => sets::Value::Bool(v),
            AttributeValue::Text(v) => sets::Value::Text(v),
            _ => return refuse("an attribute of another class read by a command set"),
        }))
    }
    fn profile<T>(
        &self,
        p: &P,
        f: impl FnOnce(&mut crate::player::owned::profile::Profile) -> T,
    ) -> Result<T> {
        self.player_only(p)?;
        self.live.inner.with_profile(p, f).map_err(bind)
    }

    /// `DevelopmentCommandSet`'s `setlevel` lambda (pinned source, lines
    /// 477-491) with `getPlayerFromArgs(player, args, 3)`.
    fn setlevel(&mut self, player: &P, args: &[String]) -> Result<()> {
        if args.len() < 3 {
            return cmd::reject(self, player, &["Usage: ::setlevel skillname level"]);
        }
        let skillname = cmd::arg(args, 1)?.to_owned();
        let Some(desired) = cmd::kotlin::to_int_or_null(cmd::arg(args, 2)?) else {
            return cmd::reject(self, player, &["Level must be an integer."]);
        };
        if desired > 99 {
            return cmd::reject(self, player, &["Level must be 99 or lower."]);
        }
        let skill = Skills::skill_by_name(Some(&skillname));
        if skill < 0 {
            return cmd::reject(self, player, &["Must use a valid skill name!"]);
        }
        let n = args.get(3..).unwrap_or(&[]).join("_");
        let target = if n.is_empty() {
            player.clone()
        } else {
            match <Self as cmd::Host>::player_by_name(self, Some(&n))? {
                Some(target) => target,
                None => {
                    return cmd::reject(
                        self,
                        player,
                        &[&format!("Could not find a player named '{n}'")],
                    );
                }
            }
        };
        self.player_only(&target)?;
        let inner = &mut self.live.inner;
        let mut skills = inner
            .with_profile(&target, |profile| {
                std::mem::replace(&mut profile.skills, placeholder_skills())
            })
            .map_err(bind)?;
        let result = {
            let mut ctx = SkillsContext {
                inner: &mut *inner,
                p: target.clone(),
            };
            skills
                .set_static_level(skill, desired, &mut ctx)
                .and_then(|()| skills.set_level(skill, desired, &mut ctx))
                .and_then(|()| skills.update_combat_level(&mut ctx).map(|_| ()))
        };
        inner
            .with_profile(&target, |profile| profile.skills = skills)
            .map_err(bind)?;
        inner.drain(&target);
        result.map_err(bind)
    }
}

impl cmd::Host for CommandHost<'_> {
    type Player = P;

    fn player_commands(&mut self) -> bool {
        self.live.commands.player_commands
    }
    fn i_am_a_cheater(&mut self) -> bool {
        self.live.commands.i_am_a_cheater
    }
    fn dev_mode(&mut self) -> Option<bool> {
        Some(self.live.inner.settings.dev_mode)
    }
    fn beta(&mut self) -> Option<bool> {
        Some(self.live.commands.beta)
    }
    fn home_location(&mut self) -> Option<Loc> {
        self.live
            .inner
            .constants
            .home_location
            .as_ref()
            .map(Location::coordinates)
    }
    fn castle_wars_destination(&mut self) -> Loc {
        // CastleWars.lobbyBankArea.randomLoc, drawn when the constants were
        // initialised: Castle Wars is not ported. The command is refused
        // before the teleport can use this placeholder.
        self.poison(
            "ServerConstants.TELEPORT_DESTINATIONS' Castle Wars row (CastleWars not ported)",
        );
        [0, 0, 0]
    }
    fn data_path(&mut self) -> Option<String> {
        self.live.commands.data_path.clone()
    }
    fn now(&mut self) -> i64 {
        (self.live.inner.clock)()
    }

    fn account(&mut self, player: &P) -> Account {
        // Every player this packet reaches has a profile.
        self.live
            .inner
            .with_profile(player, |profile| profile.details.account.clone())
            .expect("a live player's profile")
    }
    fn username(&mut self, player: &P) -> String {
        self.live.inner.slot(player).name.clone()
    }
    fn location(&mut self, player: &P) -> Result<Option<Loc>> {
        self.gate()?;
        Ok(self.live.cloc(player).map(|l| l.coordinates()))
    }

    fn message(&mut self, player: &P, text: &str) -> Result<()> {
        self.gate()?;
        self.live.inner.message(player, text).map_err(bind)
    }
    fn api_message(&mut self, player: &P, text: &str) -> Result<()> {
        // core.api.sendMessage: player.sendMessage(message).
        self.gate()?;
        self.live.inner.message(player, text).map_err(bind)
    }
    fn send_chat(&mut self, _: &P, _: &str) -> Result<()> {
        refuse("Entity.sendChat from a command (forced chat)")
    }
    fn dialogue(&mut self, _: &P, _: &str) -> Result<()> {
        refuse("core.api.sendDialogue from a command")
    }
    fn input_dialogue(&mut self, _: &P, _: InputType, _: &str, _: InputHandler<P>) -> Result<()> {
        refuse("core.api.sendInputDialogue from a command")
    }
    fn run_task(&mut self, _: &P, _: i32, _: i32, _: Task<P>) -> Result<()> {
        refuse("core.api.runTask from a command")
    }

    fn monitor_log(&mut self, _: &P, _: &str) -> Result<()> {
        self.gate()?;
        self.live
            .inner
            .not_ported
            .skip("PlayerMonitor.log (LogType.COMMAND)");
        Ok(())
    }
    fn zone_parse_command(&mut self, player: &P, name: &str, arguments: &[String]) -> Result<bool> {
        self.gate()?;
        let monitor = self.live.monitor(&player.0);
        monitor
            .parse_command(self.live, Some(&player.0), Some(name), arguments)
            .map_err(|e: zones::Error| bind(e))
    }
    fn legacy_parse(
        &mut self,
        plugin: LegacyPlugin,
        _: &P,
        name: &str,
        _: &[String],
    ) -> Result<bool> {
        self.gate()?;
        // Each plugin's parse is `when (name) { ... }` then `return false`:
        // a name outside its cases answers false with nothing done. The
        // bodies of its cases are not ported.
        let Some((_, names)) = LEGACY_NAMES.iter().find(|(class, _)| *class == plugin) else {
            return Err(refused(&format!("the legacy CommandPlugin {plugin}")));
        };
        if !names.contains(&name) {
            return Ok(false);
        }
        self.live
            .inner
            .not_ported
            .skip("a legacy CommandPlugin's command body (MapDumpCommand, oldsys)");
        Err(refused(&format!(
            "the legacy CommandPlugin {plugin}'s \"{name}\""
        )))
    }
    fn content_command(&mut self, body: ContentBody, player: &P, args: &[String]) -> Result<()> {
        self.gate()?;
        if let Some((_, why)) = self.live.commands.withheld.iter().find(|(b, _)| *b == body) {
            self.live
                .inner
                .not_ported
                .skip("a ported command body withheld from the live world");
            return refuse(why);
        }
        if body.class == DEVELOPMENT && body.lambda == SETLEVEL {
            return self.setlevel(player, args);
        }
        self.live
            .inner
            .not_ported
            .skip("a command body without a port (Body::Content)");
        Err(refused(&format!(
            "the command body {}$defineCommands${}",
            body.class, body.lambda
        )))
    }

    fn player_by_name(&mut self, name: Option<&str>) -> Result<Option<P>> {
        self.gate()?;
        let repository = self.live.repository.clone();
        Ok(repository.get_player_by_name(self.live, name))
    }
    fn clear(&mut self, _: &P) -> Result<()> {
        refuse("Player.clear from a command (kick)")
    }
    fn attribute(&mut self, player: &P, key: &str) -> Result<Option<Value>> {
        self.gate()?;
        self.player_only(player)?;
        Ok(self
            .live
            .inner
            .canonical(player)
            .player
            .attributes()
            .get(key)
            .map(Self::attribute_value))
    }
    fn api_attribute(&mut self, player: &P, key: &str, default: Value) -> Result<Option<Value>> {
        self.gate()?;
        self.player_only(player)?;
        // GameAttributes.getAttribute(key, fail): "/save:" removed wherever it
        // occurs, and `fail` for an absent key or a stored null (the native
        // `get` does both). The commands oracle's adapter answers a stored
        // null instead; see docs/live-world.md.
        let attributes = self.live.inner.canonical(player).player.attributes();
        Ok(Some(
            attributes
                .get(key)
                .map(Self::attribute_value)
                .unwrap_or(default),
        ))
    }
    fn set_attribute(&mut self, player: &P, key: &str, value: Option<Value>) -> Result<()> {
        self.gate()?;
        self.player_only(player)?;
        let value = match value {
            None => None,
            Some(Value::Int(v)) => Some(AttributeValue::Int(v)),
            Some(Value::Bool(v)) => Some(AttributeValue::Bool(v)),
            Some(Value::Text(v)) => Some(AttributeValue::Text(v)),
            Some(Value::Float(_) | Value::Other(_)) => {
                return refuse("a float or object attribute value from a command");
            }
        };
        self.live
            .inner
            .canonical(player)
            .player
            .attributes()
            .set_value(key, value);
        Ok(())
    }
    fn teleport(&mut self, player: &P, location: Loc, kind: TeleportType) -> Result<bool> {
        self.gate()?;
        self.player_only(player)?;
        let TeleportType::Normal = kind;
        let [x, y, z] = location;
        let location = Location::new(x, y, z);
        self.live
            .with_teleports(|h| {
                let manager = tp::Host::teleporter(h, player)?;
                manager.send_type(h, Some(&location), Some(tp::TeleportType::Normal))
            })
            .map_err(bind)
    }
    fn set_teleport_location(&mut self, player: &P, location: Option<Loc>) -> Result<()> {
        self.gate()?;
        self.player_only(player)?;
        self.live
            .inner
            .state
            .actor(player.node())
            .map_err(bind)?
            .queue
            .state
            .write()
            .entity
            .teleport_location = location.map(|[x, y, z]| Location::new(x, y, z));
        Ok(())
    }

    fn username_taken(&mut self, name: &str) -> Result<bool> {
        self.gate()?;
        self.live.inner.storage.username_taken(name).map_err(bind)
    }
    fn account_info(&mut self, name: Option<&str>) -> Result<Account> {
        self.gate()?;
        let name = name.ok_or(Error::Null)?;
        let clock = self.live.inner.clock.clone();
        self.live
            .inner
            .storage
            .get_account_info_with_clock(name, &*clock)
            .map_err(bind)
    }
    fn account_update(&mut self, info: &Account) -> Result<()> {
        self.gate()?;
        self.live
            .inner
            .storage
            .update(Arc::clone(info))
            .map_err(bind)
    }
    fn usernames_with_ip(&mut self, ip: &str) -> Result<Vec<Option<String>>> {
        self.gate()?;
        self.live.inner.storage.usernames_with_ip(ip).map_err(bind)
    }
    fn check_password(&mut self, _: &P, _: &str) -> Result<bool> {
        refuse("GameWorld.authenticator.checkPassword (the live world keeps no authenticator)")
    }
    fn update_password(&mut self, _: &str, _: &str) -> Result<()> {
        refuse("GameWorld.authenticator.updatePassword (the live world keeps no authenticator)")
    }
    fn store_add_to_list(&mut self, _: &str, _: &str, _: &str) -> Result<()> {
        refuse("ServerStore.addToList")
    }

    fn submit_pulse(&mut self, pulse: Pulse<P>) -> Result<()> {
        Err(refused(&format!("a command's pulse ({})", pulse.label())))
    }
    fn launch(&mut self, _: Job<P>) -> Result<()> {
        refuse("GlobalScope.launch from a command")
    }
    fn file_lines(&mut self, _: Option<&str>, _: &str) -> Result<Option<Vec<String>>> {
        refuse("a data file read by a command (modcr's list)")
    }

    fn set_update_countdown(&mut self, _: i32) -> Result<()> {
        refuse("SystemManager.getUpdater (the system manager is not installed)")
    }
    fn flag_updating(&mut self) -> Result<()> {
        refuse("SystemManager.flag (the system manager is not installed)")
    }
    fn cancel_update(&mut self) -> Result<()> {
        refuse("SystemManager.getUpdater().cancel (the system manager is not installed)")
    }
    fn exit_process(&mut self, _: i32) -> Result<()> {
        refuse("exitProcess from ::shutdown (the live world stops with Ctrl-C)")
    }

    fn item_definitions_size(&mut self) -> Result<i32> {
        self.gate()?;
        Ok(self.live.inner.items.definitions().len() as i32)
    }
    fn item_name(&mut self, id: i32) -> Result<String> {
        self.gate()?;
        Ok(sets::Host::item_definition_name(self, id))
    }
    fn note_change(&mut self, id: i32) -> Result<i32> {
        self.gate()?;
        self.item(id, 1).read().note_change().map_err(bind)
    }
    fn is_full(&mut self, player: &P, container: Container) -> Result<bool> {
        self.gate()?;
        Ok(self.container(player, container)?.full())
    }
    fn add(&mut self, player: &P, container: Container, id: i32, amount: i32) -> Result<bool> {
        self.gate()?;
        let item = self.item(id, amount);
        self.with_container(player, container, |c, defs, ctx| {
            c.add(Some(&item), true, -1, defs, ctx)
        })
    }
    fn remove(&mut self, player: &P, container: Container, id: i32, amount: i32) -> Result<bool> {
        self.gate()?;
        let item = self.item(id, amount);
        self.with_container(player, container, |c, defs, ctx| {
            c.remove(Some(&item), None, true, defs, ctx)
        })
    }
    fn item_amount(
        &mut self,
        player: &P,
        container: Container,
        id: i32,
        amount: i32,
    ) -> Result<Option<i32>> {
        self.gate()?;
        let item = self.item(id, amount);
        Ok(self
            .container(player, container)?
            .get_item(Some(&item))
            .map(|i| i.read().amount()))
    }
    fn amount(&mut self, player: &P, container: Container, id: i32) -> Result<i32> {
        self.gate()?;
        let c = self.container(player, container)?;
        Ok(c.amount_id(id, &self.live.inner.items))
    }
    fn amount_item(&mut self, player: &P, container: Container, id: i32) -> Result<i32> {
        self.gate()?;
        let item = self.item(id, 1);
        Ok(self.container(player, container)?.amount(Some(&item)))
    }
    fn slots(&mut self, player: &P, container: Container) -> Result<Vec<Option<ItemRef>>> {
        self.gate()?;
        let items = self.container(player, container)?.items();
        let slots = items
            .read()
            .iter()
            .map(|slot| slot.as_ref().map(Self::item_ref))
            .collect();
        Ok(slots)
    }
    fn get(&mut self, player: &P, container: Container, slot: i32) -> Result<Option<ItemRef>> {
        self.gate()?;
        Ok(self
            .container(player, container)?
            .get(slot)
            .as_ref()
            .map(Self::item_ref))
    }
    fn set_charge(&mut self, _: &P, _: Container, _: &ItemRef, _: i32) -> Result<()> {
        refuse("Item.setCharge from ::charge (ChargedItem is not ported)")
    }
    fn replace(&mut self, player: &P, container: Container, id: i32, slot: i32) -> Result<()> {
        self.gate()?;
        let item = self.item(id, 1);
        self.with_container(player, container, |c, defs, ctx| {
            c.replace(Some(&item), slot, true, defs, ctx).map(|_| ())
        })
    }
    fn charged_item(&mut self, _: i32) -> Result<bool> {
        refuse("ChargedItem.forId")
    }
    fn charged_for_charge(&mut self, _: i32, _: i32) -> Result<i32> {
        refuse("ChargedItem.forCharge")
    }
    fn charged_charge(&mut self, _: i32) -> Result<Option<i32>> {
        refuse("ChargedItem.getCharge")
    }

    fn sets(&mut self) -> Result<&mut dyn sets::Host<Player = P>> {
        self.gate()?;
        Ok(self)
    }
}

impl sets::Host for CommandHost<'_> {
    fn bank_size(&mut self) -> i32 {
        self.live.commands.bank_size
    }
    fn new_player_announcement(&mut self) -> bool {
        self.live.commands.new_player_announcement
    }
    fn online_players(&mut self) -> Vec<P> {
        self.live
            .repository
            .players()
            .occupied()
            .into_iter()
            .map(|(_, p)| p)
            .collect()
    }

    fn direction(&mut self, _: &P) -> Direction {
        self.poison("Entity.direction read by a command set");
        Direction::South
    }
    fn version(&mut self, player: &P) -> i32 {
        match self.profile(player, |profile| profile.version) {
            Ok(version) => version,
            Err(_) => {
                self.poison("Player.version of an entity that is not a live player");
                0
            }
        }
    }
    fn last_scene_graph(&mut self, player: &P) -> Option<Loc> {
        match self.live.inner.state.actor(player.node()) {
            Ok(actor) => actor
                .queue
                .state
                .read()
                .entity
                .last_scene
                .as_ref()
                .map(Location::coordinates),
            Err(_) => {
                self.poison("playerFlags.lastSceneGraph of an entity that is not a live player");
                None
            }
        }
    }
    fn diaries(&mut self, _: &P) -> Vec<Vec<i32>> {
        self.poison("AchievementDiaryManager's diaries read by a command set");
        Vec::new()
    }
    fn test_stage(&mut self, player: &P) -> i32 {
        match self.profile(player, |profile| profile.saved.global.player_test_stage) {
            Ok(stage) => stage,
            Err(_) => {
                self.poison("savedData.globalData.playerTestStage");
                0
            }
        }
    }
    fn set_test_stage(&mut self, player: &P, stage: i32) {
        if self.poisoned.is_some() {
            return;
        }
        if self
            .profile(player, |profile| {
                profile.saved.global.player_test_stage = stage;
            })
            .is_err()
        {
            self.poison("savedData.globalData.playerTestStage");
        }
    }
    fn read_plaques(&mut self, player: &P) -> Vec<bool> {
        match self.profile(player, |profile| profile.saved.global.read_plaques.clone()) {
            Ok(read) => read,
            Err(_) => {
                self.poison("savedData.globalData.readPlaques");
                Vec::new()
            }
        }
    }
    fn set_read_plaque(&mut self, player: &P, index: usize, read: bool) {
        if self.poisoned.is_some() {
            return;
        }
        let set = self.profile(player, |profile| {
            match profile.saved.global.read_plaques.get_mut(index) {
                Some(slot) => {
                    *slot = read;
                    true
                }
                None => false,
            }
        });
        if !matches!(set, Ok(true)) {
            self.poison("savedData.globalData.readPlaques");
        }
    }
    fn boss_counters(&mut self, player: &P) -> Vec<i32> {
        match self.profile(player, |profile| profile.saved.global.boss_counters.clone()) {
            Ok(counters) => counters,
            Err(_) => {
                self.poison("savedData.globalData.bossCounters");
                Vec::new()
            }
        }
    }
    fn barrows_loots(&mut self, player: &P) -> i32 {
        match self.profile(player, |profile| profile.saved.global.barrows_loots) {
            Ok(loots) => loots,
            Err(_) => {
                self.poison("savedData.globalData.barrowsLoots");
                0
            }
        }
    }

    fn entity_attribute(
        &mut self,
        player: &P,
        key: &str,
        fail: Option<sets::Value<P>>,
    ) -> Result<Option<sets::Value<P>>> {
        self.gate()?;
        self.player_only(player)?;
        let attributes = self.live.inner.canonical(player).player.attributes();
        match attributes.get(key) {
            Some(value) => Self::get_value(value),
            None => Ok(fail),
        }
    }
    fn entity_set_attribute(
        &mut self,
        player: &P,
        key: &str,
        value: Option<sets::Value<P>>,
    ) -> Result<()> {
        self.gate()?;
        self.player_only(player)?;
        let value = Self::set_value(value)?;
        self.live
            .inner
            .canonical(player)
            .player
            .attributes()
            .set_value(key, value);
        Ok(())
    }
    fn entity_remove_attribute(&mut self, player: &P, key: &str) -> Result<()> {
        self.gate()?;
        self.player_only(player)?;
        self.live
            .inner
            .canonical(player)
            .player
            .attributes()
            .remove(key);
        Ok(())
    }
    fn has_attribute(&mut self, player: &P, key: &str) -> Result<bool> {
        self.gate()?;
        self.player_only(player)?;
        Ok(self
            .live
            .inner
            .canonical(player)
            .player
            .attributes()
            .contains_key(key))
    }

    fn dispatch_message(&mut self, player: &P, text: &str) -> Result<()> {
        self.gate()?;
        self.live.inner.message(player, text).map_err(bind)
    }
    fn send_string(&mut self, player: &P, text: &str, interface: i32, child: i32) -> Result<()> {
        self.gate()?;
        self.player_only(player)?;
        self.live
            .inner
            .text(player, text, interface, child)
            .map_err(bind)
    }
    fn send_varc(&mut self, _: &P, _: i16, _: i32) -> Result<()> {
        refuse("PacketDispatch.sendVarcUpdate from a command set")
    }
    fn send_interface_config(&mut self, _: &P, _: i32, _: i32, _: bool) -> Result<()> {
        refuse("PacketDispatch.sendInterfaceConfig from a command set")
    }
    fn interface_opened(&mut self, _: &P) -> Result<bool> {
        refuse("InterfaceManager.isOpened from a command set")
    }
    fn opened_component(&mut self, _: &P) -> Result<Option<i32>> {
        refuse("InterfaceManager.getOpened from a command set")
    }
    fn open_component(&mut self, _: &P, _: i32) -> Result<()> {
        refuse("InterfaceManager.open from a command set")
    }
    fn close_interfaces(&mut self, _: &P) -> Result<bool> {
        refuse("InterfaceManager.close from a command set")
    }
    fn close_dialogue(&mut self, _: &P) -> Result<bool> {
        refuse("DialogueInterpreter.close from a command set")
    }
    fn locked(&mut self, _: &P, _: Lock) -> Result<bool> {
        refuse("the action locks read by a command set")
    }
    fn is_artificial(&mut self, player: &P) -> Result<bool> {
        self.gate()?;
        self.player_only(player)?;
        // Every player of this world is a session's player.
        Ok(false)
    }
    fn swing_handler(&mut self, _: &P, _: bool) -> Result<Handler> {
        refuse("Player.getSwingHandler from a command set")
    }
    fn level(&mut self, _: &Actor<P>, _: i32) -> Result<i32> {
        refuse("Skills.getLevel from a command set")
    }
    fn set_version(&mut self, player: &P, version: i32) -> Result<()> {
        self.gate()?;
        self.profile(player, |profile| profile.version = version)
    }
    fn unlock(&mut self, _: &P) -> Result<()> {
        refuse("Entity.unlock from a command set")
    }
    fn ip_address(&mut self, _: &P) -> Result<String> {
        refuse("PlayerDetails.ipAddress from a command set")
    }
    fn comp_name(&mut self, _: &P) -> Result<String> {
        refuse("PlayerDetails.compName from a command set")
    }
    fn finish_task(&mut self, _: &P, _: i32, _: i32, _: i32) -> Result<()> {
        refuse("AchievementDiary.finishTask from a command set")
    }
    fn quest_points(&mut self, _: &P) -> Result<i32> {
        refuse("QuestRepository.getPoints from a command set")
    }
    fn ironman_mode(&mut self, _: &P) -> Result<IronmanMode> {
        refuse("IronmanManager.getMode from a command set")
    }
    fn slayer_completed_tasks(&mut self, _: &P) -> Result<i32> {
        refuse("SlayerManager from a command set")
    }
    fn completed_clues(&mut self, _: &P) -> Result<Vec<i32>> {
        refuse("the clue counters from a command set")
    }
    fn stat_kills(&mut self, _: &P, _: &[i32]) -> Result<i64> {
        refuse("PlayerStatsCounter.getKills from a command set")
    }
    fn stat_rare_drops(&mut self, _: &P, _: i32) -> Result<i64> {
        refuse("PlayerStatsCounter.getRareDrops from a command set")
    }

    fn container_clear(&mut self, player: &P, container: Container) -> Result<()> {
        self.gate()?;
        self.with_container(player, container, |c, defs, ctx| c.clear(true, defs, ctx))
    }
    fn container_refresh(&mut self, player: &P, container: Container) -> Result<()> {
        self.gate()?;
        self.with_container(player, container, |c, defs, ctx| c.refresh(None, defs, ctx))
    }
    fn container_update(&mut self, player: &P, container: Container) -> Result<()> {
        self.gate()?;
        self.with_container(player, container, |c, defs, ctx| c.update(None, defs, ctx))
    }
    fn maximum_add(
        &mut self,
        player: &P,
        container: Container,
        id: i32,
        amount: i32,
    ) -> Result<i32> {
        self.gate()?;
        let item = self.item(id, amount);
        self.container(player, container)?
            .maximum_add(Some(&item))
            .map_err(bind)
    }

    fn send_news(&mut self, _: &str, _: i32, _: &str) -> Result<()> {
        refuse("core.api.sendNews from a command set")
    }
    fn item_definition_name(&mut self, id: i32) -> String {
        self.live
            .inner
            .items
            .for_id(id)
            .read()
            .name
            .clone()
            .unwrap_or_else(|| "null".into())
    }
    fn add_item_or_drop(&mut self, _: &P, _: i32, _: i32) -> Result<()> {
        refuse("core.api.addItemOrDrop from a command set")
    }
    fn set_varp(&mut self, _: &P, _: i32, _: i32, _: bool) -> Result<()> {
        refuse("core.api.setVarp from a command set")
    }
    fn set_varbit(&mut self, _: &P, _: i32, _: i32, _: bool) -> Result<()> {
        refuse("core.api.setVarbit from a command set")
    }
    fn get_varbit(&mut self, _: &P, _: i32) -> Result<i32> {
        refuse("core.api.getVarbit from a command set")
    }
    fn start_timer(&mut self, _: &P, _: Timer) -> Result<()> {
        refuse("getOrStartTimer (farming and brewing timers) from a command set")
    }
    fn patches(&mut self, _: &P) -> Result<i32> {
        refuse("farming patches from a command set")
    }
    fn set_next_growth(&mut self, _: &P, _: i32, _: i64) -> Result<()> {
        refuse("farming patches from a command set")
    }
    fn set_disease_mod(&mut self, _: &P, _: i32, _: i32) -> Result<()> {
        refuse("farming patches from a command set")
    }
    fn crop_run(&mut self, _: &P) -> Result<bool> {
        refuse("farming patches from a command set")
    }
    fn bins(&mut self, _: &P) -> Result<i32> {
        refuse("compost bins from a command set")
    }
    fn bin_finished(&mut self, _: &P, _: i32) -> Result<bool> {
        refuse("compost bins from a command set")
    }
    fn bin_closed(&mut self, _: &P, _: i32) -> Result<bool> {
        refuse("compost bins from a command set")
    }
    fn bin_finish(&mut self, _: &P, _: i32) -> Result<()> {
        refuse("compost bins from a command set")
    }
    fn bin_reset(&mut self, _: &P, _: i32) -> Result<()> {
        refuse("compost bins from a command set")
    }
    fn vats(&mut self, _: &P) -> Result<i32> {
        refuse("brewing vats from a command set")
    }
    fn brew(&mut self, _: &P, _: i32, _: [bool; 4]) -> Result<()> {
        refuse("brewing vats from a command set")
    }
    fn log(&mut self, origin: &str, level: Level, message: &str) -> Result<()> {
        self.gate()?;
        self.live
            .inner
            .log_line(format!("[{}] {origin}: {message}", level.name()));
        Ok(())
    }
    fn input(&mut self, _: &P, _: InputType, _: &str, _: Input<P>) -> Result<()> {
        refuse("core.api.sendInputDialogue from a command set")
    }
    fn launch_job(&mut self, _: sets::Job<P>) -> Result<()> {
        refuse("GlobalScope.launch from a command set")
    }

    fn start_cutscene(&mut self, _: &P, _: &str) -> Result<()> {
        refuse("cutscenes from a command set")
    }
    fn permadeath(&mut self, _: &P) -> Result<()> {
        refuse("Permadeath from a command set")
    }
    fn trawler_loot(&mut self, _: &P, _: i32, _: i32, _: bool) -> Result<()> {
        refuse("TrawlerLoot from a command set")
    }
    fn rules_and_info(&mut self, _: &P) -> Result<i32> {
        refuse("RulesAndInfo.setBaseRulesAndInfo from a command set")
    }
    fn open_book(&mut self, _: &P, _: i32, _: Book) -> Result<()> {
        refuse("BookInterface.openBook from a command set")
    }
    fn page_setup(&mut self, _: &P, _: i32, _: &str, _: &[PageSet], _: bool) -> Result<()> {
        refuse("BookInterface.pageSetup from a command set")
    }
    fn private_message(&mut self, _: &P, _: &str, _: &str) -> Result<()> {
        refuse("CommunicationInfo.sendMessage from a command set")
    }
    fn bot_script(&mut self, _: &str) -> Result<Option<String>> {
        refuse("AIRepository.PulseRepository from a command set")
    }
    fn valid_offers(&mut self) -> Result<Vec<Offer>> {
        refuse("the Grand Exchange offers from a command set")
    }
    fn bot_offers(&mut self) -> Result<Vec<Offer>> {
        refuse("the Grand Exchange bot offers from a command set")
    }

    fn cache_item_definitions_size(&mut self) -> Result<i32> {
        // Cache.getItemDefinitionsSize(): the registry holds one definition
        // per file of the cache's item archive.
        self.gate()?;
        Ok(self.live.inner.items.definitions().len() as i32)
    }
    fn scenery_definition(&mut self, _: i32) -> Result<()> {
        refuse("SceneryDefinition.forId from a command set")
    }
    fn scenery_varbit_id(&mut self, _: i32) -> Result<i32> {
        refuse("SceneryDefinition.varbitID from a command set")
    }
    fn scenery_config_file(&mut self, _: i32) -> Result<Option<i32>> {
        refuse("SceneryDefinition.getConfigFile from a command set")
    }
    fn varbit_for_object(&mut self, _: i32) -> Result<Option<i32>> {
        refuse("VarbitDefinition.forObjectID from a command set")
    }
    fn varbit_for_id(&mut self, _: i32) -> Result<Option<i32>> {
        refuse("VarbitDefinition.forId from a command set")
    }
    fn varbit_varp(&mut self, _: i32) -> Result<i32> {
        refuse("VarbitDefinition.varpId from a command set")
    }
    fn varbit_start_bit(&mut self, _: i32) -> Result<i32> {
        refuse("VarbitDefinition.startBit from a command set")
    }
    fn varbit_string(&mut self, _: i32) -> Result<String> {
        refuse("VarbitDefinition.toString from a command set")
    }
    fn npc_definitions(&mut self) -> Result<Vec<Option<NpcDefinition>>> {
        refuse("NPCDefinition.getDefinitions from a command set")
    }

    fn region(&mut self, _: i32) -> Result<bool> {
        refuse("RegionManager.forId from a command set")
    }
    fn region_active(&mut self, _: i32) -> Result<bool> {
        refuse("Region.isActive from a command set")
    }
    fn teleport_permitted(&mut self, _: Loc) -> Result<bool> {
        refuse("RegionManager.isTeleportPermitted from a command set")
    }
    fn clipping_flag(&mut self, _: Loc) -> Result<i32> {
        refuse("RegionManager.getClippingFlag from a command set")
    }
    fn is_landscape(&mut self, _: Loc) -> Result<bool> {
        refuse("RegionManager.isLandscape from a command set")
    }
    fn region_object(&mut self, _: Loc) -> Result<Option<Scenery>> {
        refuse("RegionManager.getObject from a command set")
    }
    fn region_chunk(&mut self, _: Loc) -> Result<Chunk> {
        refuse("RegionManager.getRegionChunk from a command set")
    }
    fn chunk_static_objects(&mut self, _: Chunk, _: i32, _: i32) -> Vec<Option<Scenery>> {
        self.poison("RegionChunk.statObjects read by a command set");
        Vec::new()
    }
    fn chunk_objects(&mut self, _: Chunk, _: i32, _: i32) -> Result<Vec<Option<Scenery>>> {
        refuse("RegionChunk.getObjects from a command set")
    }
    fn new_scenery(&mut self, _: i32, _: Option<Loc>, _: i32, _: i32) -> Result<Scenery> {
        refuse("new Scenery from a command set")
    }
    fn scenery_add(&mut self, _: &Scenery) -> Result<()> {
        refuse("SceneryBuilder.add from a command set")
    }
    fn scenery_remove(&mut self, _: &Scenery) -> Result<bool> {
        refuse("SceneryBuilder.remove from a command set")
    }

    fn new_npc(&mut self, _: i32, _: Option<Loc>) -> Result<Npc> {
        refuse("NPC.create from a command set (::npc)")
    }
    fn npc_init_config(&mut self, _: Npc) -> Result<()> {
        refuse("NPC.initConfig from a command set")
    }
    fn npc_name(&mut self, _: Npc) -> Result<String> {
        refuse("NPC.getName from a command set")
    }
    fn npc_bonuses(&mut self, _: Npc) -> Result<Vec<i32>> {
        refuse("NPC bonuses from a command set")
    }
    fn npc_set_attack_style(&mut self, _: Npc, _: i32, _: i32) -> Result<()> {
        refuse("NPC.setAttackStyle from a command set")
    }
    fn npc_set_attribute(&mut self, _: Npc, _: &str, _: bool) -> Result<()> {
        refuse("NPC.setAttribute from a command set")
    }
    fn npc_set_respawn(&mut self, _: Npc, _: bool) -> Result<()> {
        refuse("NPC.setRespawn from a command set")
    }
    fn npc_set_direction(&mut self, _: Npc, _: Direction) -> Result<()> {
        refuse("NPC.setDirection from a command set")
    }
    fn npc_init(&mut self, _: Npc) -> Result<()> {
        refuse("NPC.init from a command set")
    }
    fn npc_set_walks(&mut self, _: Npc, _: bool) -> Result<()> {
        refuse("NPC.setWalks from a command set")
    }
    fn npc_location(&mut self, _: Npc) -> Option<Loc> {
        self.poison("NPC.location read by a command set");
        None
    }
    fn npc_direction(&mut self, _: Npc) -> Direction {
        self.poison("NPC.direction read by a command set");
        Direction::South
    }
    fn npc_walks(&mut self, _: Npc) -> bool {
        self.poison("NPC.isWalks read by a command set");
        false
    }

    fn calculate_accuracy(&mut self, _: Handler, _: &Actor<P>) -> Result<i32> {
        refuse("the swing handlers' calculateAccuracy from a command set")
    }
    fn calculate_defence(&mut self, _: Handler, _: &Actor<P>, _: &Actor<P>) -> Result<i32> {
        refuse("the swing handlers' calculateDefence from a command set")
    }
    fn calculate_hit(&mut self, _: Handler, _: &Actor<P>, _: &Actor<P>, _: f64) -> Result<i32> {
        refuse("the swing handlers' calculateHit from a command set")
    }

    fn println(&mut self, text: &str) {
        if self.poisoned.is_none() {
            println!("{text}");
        }
    }
    fn set_clipboard(&mut self, _: &str) -> Result<()> {
        // The server runs headless: the AWT clipboard throws.
        self.gate()?;
        Err(sets::headless())
    }
    fn clock(&mut self) -> i64 {
        (self.live.inner.clock)()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn april_is_the_fourth_civil_month() {
        // 2026-04-01T00:00Z and the millisecond before it.
        assert!(super::april(1_775_001_600_000));
        assert!(!super::april(1_775_001_599_999));
        // 2026-11-27T00:00Z.
        assert!(!super::april(1_795_737_600_000));
    }
}
