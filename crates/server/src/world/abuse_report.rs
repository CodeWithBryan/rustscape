//! Abuse reports: the `REPORT_ABUSE` decoder of `Decoders530`, the
//! `ReportAbuse` branch of `PacketProcessor.process`, `Rule` and
//! `AbuseReport.construct`. The optional mute is the registry's own `mute`
//! command run through `Command.attemptHandling` (world/commands), so it
//! keeps the reporter's rights check and the command's account writes.
//! See docs/commands.md.
use crate::world::commands::{self, Host as CommandHost, Registry};
use crate::world::social::names::long_to_string;

/// `core.game.global.report.Rule`, by its id.
pub const RULES: [&str; 13] = [
    "OFFENSIVE_LANGUAGE",
    "ITEM_SCAMMING",
    "PASSWORD_SCAMMING",
    "BUG_ABUSE",
    "KELDAGRIM_STAFF_IMPERSONATION",
    "ACCCOUNT_SHARING",
    "MACROING",
    "MULTIPLE_LOGGING",
    "ENCOURAGING_TO_BREAK_TULES",
    "MISUSE_OF_CUSTOMER_SUPPORT",
    "ADVERISTING",
    "REAL_WORLD_ITEM_TRADING",
    "ASKING_PERSONAL_DETAILS",
];

/// `Rule.forId(id)`: the constant whose rule id is `id`.
pub fn rule_for_id(id: i32) -> Option<&'static str> {
    usize::try_from(id).ok().and_then(|i| RULES.get(i).copied())
}

/// `Packet.ReportAbuse`'s fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportAbuse {
    pub target: String,
    pub rule: i32,
    pub mod_mute: bool,
}

/// `Decoders530.REPORT_ABUSE.decode`: a name long, the rule byte and the
/// mute byte; a short payload is `BufferUnderflowException`.
pub fn decode(payload: &[u8]) -> Result<ReportAbuse, String> {
    if payload.len() < 10 {
        return Err(
            "Error decoding opcode 99/REPORT_ABUSE: java.nio.BufferUnderflowException".into(),
        );
    }
    let long = i64::from_be_bytes(payload[..8].try_into().expect("eight bytes"));
    Ok(ReportAbuse {
        target: long_to_string(long),
        rule: i32::from(payload[8] as i8),
        mod_mute: payload[9] as i8 == 1,
    })
}

/// What the report needs beyond the command host.
pub trait Host: CommandHost {
    /// `player.getName()`.
    fn name(&mut self, player: &Self::Player) -> String;
    /// `player.getPacketDispatch().sendMessage(text)`.
    fn packet_message(&mut self, player: &Self::Player, text: &str) -> commands::Result<()>;
    /// `Discord.postPlayerAlert(player, type)`.
    fn post_player_alert(&mut self, player: &str, kind: &str) -> commands::Result<()>;
}

/// "Invalid player name."
pub const INVALID: &str = "Invalid player name.";

/// The `ReportAbuse` branch: the target must be a taken user name; a
/// report on oneself is told so and still filed.
pub fn process<H: Host + ?Sized>(
    host: &mut H,
    registry: &Registry,
    player: &H::Player,
    report: &ReportAbuse,
) -> commands::Result<()> {
    if report.target.is_empty() {
        return host.api_message(player, INVALID);
    }
    let lower = rs09_game_core::java_lower::lowercase_root(&report.target);
    if !host.username_taken(&lower)? {
        return host.api_message(player, INVALID);
    }
    let username = host.username(player);
    if rs09_game_core::java_string::equals_ignore_case(&report.target, &username) {
        host.message(player, "You can't report yourself!")?;
    }
    let reporter = host.name(player);
    construct(
        host,
        registry,
        player,
        &reporter,
        &report.target,
        rule_for_id(report.rule),
        report.mod_mute,
    )
}

/// `new AbuseReport(reporter, victim, rule).construct(player, mute)`.
pub fn construct<H: Host + ?Sized>(
    host: &mut H,
    registry: &Registry,
    player: &H::Player,
    reporter: &str,
    victim: &str,
    rule: Option<&'static str>,
    mute: bool,
) -> commands::Result<()> {
    let _ = reporter;
    if mute {
        let command = registry.get("mute").ok_or(commands::Error::Null)?.clone();
        let args = ["mute".to_owned(), victim.to_owned(), "48h".to_owned()];
        commands::attempt_handling(host, registry, &command, player, &args)?;
    }
    host.packet_message(player, "Thank-you, your abuse report has been received.")?;
    let rule = rule.ok_or(commands::Error::Null)?;
    host.post_player_alert(victim, &format!("Abuse Report - {rule}"))
}

/// The decoder and the branch together, as the packet reaches the world.
pub fn handle_packet<H: Host + ?Sized>(
    host: &mut H,
    registry: &Registry,
    player: &H::Player,
    payload: &[u8],
) -> Result<commands::Result<()>, String> {
    let report = decode(payload)?;
    Ok(process(host, registry, player, &report))
}
