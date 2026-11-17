//! The game-frame and global interface listeners
//! (`content/global/handlers/iface` and `iface/bank`).
use super::{
    Error, Expression, Extension, Host, LogLevel, ObjectKind, Quest, Result, ScriptArg, Store,
    Task, TeleportType, Toggle, Value, tabs::java_weapon,
};
use crate::player::details::Rights;
use crate::world::combat_swing::WeaponInterfaces;
use crate::world::interface_listeners::Button;

// ------------------------------------------------------------------ MainGameInterface

/// `on(FILTERBUTTONS_751)`.
pub fn filter_buttons<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    if b.button == 27 {
        open_report(h, p)?;
    }
    Ok(true)
}

/// `MainGameInterface.openReport`.
pub fn open_report<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<()> {
    if !h.open(p, 553)? {
        return Err(Error::Null);
    }
    h.set_close_event(p, 553, Task::ReportClose)?;
    h.run_script(p, 508, "", Vec::new())?;
    if h.rights(p)? != Rights::RegularPlayer {
        for i in 0..=17 {
            h.interface_config(p, 553, i, false)?;
        }
    }
    Ok(())
}

/// The report-abuse close event.
pub fn report_close<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<bool> {
    h.run_script(p, 80, "", Vec::new())?;
    h.run_script(p, 137, "", Vec::new())?;
    Ok(true)
}

/// `on(TOPSTAT_RUN_750)`.
pub fn run_button<H: Host + ?Sized>(h: &mut H, p: &H::Node, _: Button) -> Result<bool> {
    h.toggle(p, Toggle::Run)?;
    Ok(true)
}

/// `on(TOPLEVEL_FULLSCREEN_746)`.
pub fn top_level_fullscreen<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    if b.button == 110 {
        configure_world_map(h, p)?;
    }
    Ok(true)
}

/// `on(TOPLEVEL_548)`.
pub fn top_level<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    match b.button {
        38 => {
            // getExtension<Any>(WeaponInterface::class.java) === WeaponInterfaces.STAFF
            if h.weapon_interface(p)? == Some(Extension::StaffConstant) {
                h.open_tab(p, 0, WeaponInterfaces::Staff.interface_id())?;
                let weapon = h.weapon_interface(p)?;
                match weapon {
                    Some(Extension::Weapon(w)) => h.update_weapon_interface(&w)?,
                    Some(Extension::StaffConstant) => return Err(Error::Cast),
                    None => return Err(Error::Null),
                }
            }
        }
        40 => h.quest_sync_tab(p)?,
        41 => {
            let inventory = h.inventory(p)?;
            h.refresh(&inventory)?;
        }
        66 => configure_world_map(h, p)?,
        _ => {}
    }
    Ok(true)
}

/// `MainGameInterface.configureWorldMap`.
pub fn configure_world_map<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<()> {
    if h.in_combat(p)? {
        return h.api_message(
            p,
            "It wouldn't be very wise opening the world map during combat.",
        );
    }
    if h.interaction_locked(p)? || h.movement_locked(p)? {
        return h.api_message(p, "You can't do this right now.");
    }
    h.close_manager(p)?;
    h.open_windows_pane(p, 755, None)?;
    let z = h.location(p)?.ok_or(Error::Null)?[2];
    let x = h.location(p)?.ok_or(Error::Null)?[0];
    let y = h.location(p)?.ok_or(Error::Null)?[1];
    let hash = (z << 28) | (x << 14) | y;
    h.script_configs(p, 622, hash, "", vec![ScriptArg::Int(0)])?;
    h.script_configs(p, 674, hash, "", vec![ScriptArg::Int(0)])?;
    Ok(())
}

// ------------------------------------------------------------------ LoginInterface, DeathInterface

/// `on(WELCOME_SCREEN_378)`.
pub fn welcome<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    if b.button == 140 {
        h.lock_key(p, "login", 2)?;
        h.close_interface(p)?;
        h.run_task(p, 1, 0, Task::ConfigureWorld { player: p.clone() })?;
    }
    Ok(true)
}

/// `onClose(WELCOME_SCREEN_378)`.
pub fn welcome_close<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<bool> {
    h.is_locked(p, "login")
}

/// `on(AIDE_DEATH_153)`.
pub fn death<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    if b.button == 1 {
        h.set_disable_death_screen(p, true)?;
        h.close_interface(p)?;
    }
    Ok(true)
}

// ------------------------------------------------------------------ component plugins

/// `CharacterDesignInterface.handle`.
pub fn character_design<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    if b.opcode == 155 {
        h.character_design(p, b.button)?;
    }
    Ok(true)
}

/// `on(WORLDMAP_755)`.
pub fn world_map<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    if b.button == 3 {
        let pane = if h.is_resizable(p)? { 746 } else { 548 };
        h.open_windows_pane(p, pane, Some(2))?;
        h.run_script(p, 1187, "ii", vec![ScriptArg::Int(0), ScriptArg::Int(0)])?;
        h.update_scene_graph(p, true)?;
    }
    Ok(true)
}

/// `Sounds.TBCU_FINDGEM_1270`.
pub const EXPERIENCE_SOUND: i32 = 1270;

/// ExperienceInterface's skill buttons.
pub fn experience_skill(button: i32) -> Option<i32> {
    Some(match button {
        29 => 0,
        30 => 2,
        31 => 1,
        32 => 4,
        35 => 6,
        39 => 12,
        34 => 3,
        33 => 5,
        36 => 16,
        37 => 15,
        38 => 17,
        43 => 10,
        47 => 20,
        48 => 18,
        50 => 19,
        41 => 14,
        42 => 13,
        49 => 21,
        52 => 23,
        45 => 7,
        44 => 11,
        46 => 8,
        40 => 9,
        51 => 22,
        _ => return None,
    })
}

/// `ExperienceInterface.handle`.
pub fn experience<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    if b.button == 2 {
        let attribute = h.get_attribute(p, "exp_interface:skill")?;
        let skill = Value::int(attribute, -1)?;
        if skill == -1 {
            h.message(p, "You must first select a skill.")?;
            return Ok(true);
        }
        h.remove_attribute(p, "exp_interface:skill")?;
        let required = match skill {
            15 => Some((Quest::DruidicRitual, "Druidic Ritual")),
            20 => Some((Quest::RuneMysteries, "Rune Mysteries")),
            23 => Some((Quest::WolfWhistle, "Wolf Whistle")),
            _ => None,
        };
        if let Some((quest, name)) = required {
            if !h.quest_complete(p, quest)? {
                h.message(p, &format!("You need to have completed {name} for this."))?;
                return Ok(true);
            }
        }
        let Some(caller) = h.attributes_get(p, "caller")? else {
            return Ok(true);
        };
        match caller {
            Value::Object(o) => match h.object_kind(&o) {
                ObjectKind::Plugin => h.selection_callback(&o, skill, p)?,
                ObjectKind::Function => h.caller_invoke(&o, skill, p)?,
                ObjectKind::Other => return Err(Error::Cast),
            },
            _ => return Err(Error::Cast),
        }
        h.play_audio(p, EXPERIENCE_SOUND, 0, 1)?;
        h.close_interface(p)?;
    } else {
        let skill = match experience_skill(b.button) {
            Some(skill) => skill,
            None => {
                h.log(
                    LogLevel::Warn,
                    &format!("EXP_INTERFACE: Invalid SKILL CHOICE BUTTON: {}", b.button),
                )?;
                18
            }
        };
        h.set_attribute(p, "exp_interface:skill", Value::Int(skill))?;
    }
    Ok(true)
}

/// `FriendsListInterface.open(player)`.
pub fn friends_list_open<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<()> {
    h.component_open(p, 550)?;
    let server = h.server_name();
    let world = h.world_id()?;
    h.set_interface_text(p, &format!("Friends List - {server} {world}"), 550, 3)
}

// ------------------------------------------------------------------ RulesAndInfo

/// `RulesAndInfo.RULES`.
pub const RULES: [&str; 13] = [
    "<col=ffffff>1. Be respectful to your fellow players.</col>",
    "   -No harassment, etc.",
    "   -Keep arguments private.",
    "<col=ffffff>2. Do not exploit bugs.</col>",
    "   -Zero tolerance. Can result in account deletion.",
    "   -If you discover a bug, report it on our Gitlab.",
    "<col=ffffff>3. Do not discuss or advertise other servers.</col>",
    "   -Discussion of the live jagex games is fine.",
    "   -Discussion of open source projects is fine.",
    "<col=ffffff>4. No unfair advantages.</col>",
    "   -This includes autoclickers, autohotkey, etc.",
    "   -Exception: 1-to-1 inputs, such as mousekeys.",
    "   -Includes training methods that bypass AFK limits.",
];
/// `RulesAndInfo.SEPARATOR`.
pub const SEPARATOR: &str = "<str>                                             </str>";
/// `RulesAndInfo.INFO`.
pub const INFO: [&str; 1] = ["<col=6bcdfa>Join the forums! forum.2009scape.org </col>"];

/// `RulesListener`'s `onClose(384)`.
pub fn rules_close<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<bool> {
    let confirmed = h.api_get_attribute(p, "rules:confirmed")?;
    if !Value::bool(confirmed, false)? {
        h.run_task(p, 1, 1, Task::Rules { player: p.clone() })?;
    }
    Ok(true)
}

/// The rules close event's task: `openFor(player)`, then the dialogue.
pub fn rules_task<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<()> {
    rules_open_for(h, p)?;
    h.dialogue(p, "Please read the rules.")
}

/// `RulesAndInfo.openFor(player)`.
pub fn rules_open_for<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<()> {
    let confirmed = h.api_get_attribute(p, "rules:confirmed")?;
    if Value::bool(confirmed, false)? {
        return Ok(());
    }
    let tutorial = h.api_get_attribute(p, "tutorial:complete")?;
    if !Value::bool(tutorial, false)? {
        return Ok(());
    }
    // The default is drawn before the attribute is read.
    let fallback = h.random(1000, 9999)?;
    let pin = h.api_get_attribute(p, "rules:pin")?;
    let pin = Value::int(pin, fallback)?;
    h.api_set_attribute(p, "/save:rules:pin", Value::Int(pin))?;
    let mut line = rules_base(h, p)?;
    h.set_interface_text(
        p,
        &format!("<col=ffffff>If you agree to the above, type ::confirmrules {pin}"),
        384,
        line,
    )?;
    line += 1;
    h.set_interface_text(p, "", 384, line)?;
    h.interface_config(p, 384, 17, true)?;
    h.open_interface(p, 384)?;
    h.lock_all(p)
}

/// `RulesAndInfo.setBaseRulesAndInfo(player)`.
pub fn rules_base<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<i32> {
    let mut line = 1;
    for _ in INFO {
        h.set_interface_text(p, INFO[(line - 1) as usize], 384, line)?;
        line += 1;
    }
    h.set_interface_text(p, SEPARATOR, 384, line)?;
    line += 1;
    let first = line;
    for _ in RULES {
        h.set_interface_text(p, RULES[(line - first) as usize], 384, line)?;
        line += 1;
    }
    h.set_interface_text(p, "", 384, line)?;
    Ok(line)
}

/// `ScrollInterface.scrollSetup(player, component, contents)`.
pub fn scroll_setup<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    component: i32,
    lines: &[(String, i32)],
) -> Result<()> {
    h.close_interface(p)?;
    if component == 220 {
        h.open_interface(p, 220)?;
        for (message, child) in lines {
            if (1..=15).contains(child) {
                h.packet_string(p, message, 220, *child)?;
            }
        }
    }
    Ok(())
}

// ------------------------------------------------------------------ GenericItemSelect

/// `onOpen(12)`: a pulse that never finishes and closes the tab when stopped.
pub fn item_select_open<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<bool> {
    let pulse = h.new_pulse(Task::ItemSelect { player: p.clone() })?;
    h.run_pulse(p, &pulse, None)?;
    Ok(true)
}

/// `on(12)`: `processResponse(player, opcode, slot)`.
pub fn item_select<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    let callback = match h.api_get_attribute(p, "itemselect-callback")? {
        None => None,
        Some(Value::Object(o)) if h.object_kind(&o) == ObjectKind::Function => Some(o),
        Some(_) => return Err(Error::Cast),
    };
    let Some(callback) = callback else {
        let name = h.name(p)?;
        h.log(
            LogLevel::Warn,
            &format!(
                "{} is trying to use an item select prompt with no callback!",
                super::or_null(name)
            ),
        )?;
        return Ok(true);
    };
    let option = match b.opcode {
        155 => 0,
        196 => 1,
        124 => 2,
        199 => 3,
        234 => 4,
        9 => 10,
        _ => -1,
    };
    if option == -1 {
        let name = h.name(p)?;
        h.log(
            LogLevel::Warn,
            &format!(
                "{} is clicking a right-click index that we don't know the opcode for yet, lol. Here's the opcode: {}",
                super::or_null(name),
                b.opcode
            ),
        )?;
        return Ok(true);
    }
    h.item_select_invoke(&callback, b.slot, option)?;
    let keep = h.api_get_attribute(p, "itemselect-keepalive")?;
    if !Value::bool(keep, false)? {
        h.api_remove_attribute(p, "itemselect-callback")?;
        h.api_remove_attribute(p, "itemselect-keepalive")?;
        h.close_single_tab(p)?;
    }
    Ok(true)
}

/// `onClose(12)`.
pub fn item_select_close<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<bool> {
    h.api_remove_attribute(p, "itemselect-callback")?;
    h.api_remove_attribute(p, "itemselect-keepalive")?;
    Ok(true)
}

// ------------------------------------------------------------------ AutocastSelectPlugin, TeleotherInterface, RequestAssistInterface

/// `AutocastSelectPlugin.handle`.
pub fn autocast_select<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    let selecting = h.get_attribute(p, "autocast_select")?;
    if !Value::bool(selecting, false)? {
        return Ok(true);
    }
    if h.has_timer(p, "spellbook:swap")? {
        h.packet_message(p, "You cannot autocast while using Spellbook Swap.")?;
        h.remove_attribute(p, "autocast_select")?;
        h.remove_attribute(p, "autocast_component")?;
        if let Some(w) = java_weapon(h, p)? {
            h.set_attack_style(&w, 3)?;
            h.open_tab_weapon(p, &w)?;
        }
        return Ok(true);
    }
    h.remove_attribute(p, "autocast_select")?;
    if let Some(w) = java_weapon(h, p)? {
        h.select_auto_spell(&w, b.button, true, false)?;
        h.open_tab_weapon(p, &w)?;
    }
    Ok(true)
}

/// `on(TELEPORT_OTHER_326)`: the listener answers the close's result.
pub fn teleother<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    if b.button == 5 {
        h.lock_ticks(p, 2)?;
        let own = h.location(p)?;
        let location = match h.get_attribute(p, "t-o_location")? {
            None => own,
            Some(Value::Location(l)) => Some(l),
            Some(_) => return Err(Error::Cast),
        };
        if h.teleport(p, location, TeleportType::TeleOther)? {
            h.visualize(p, 1816, 342)?;
        }
    }
    h.close_manager(p)
}

/// `RequestAssistInterface.handle`.
pub fn request_assist<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    let Some(session) = h.assist_session(p)? else {
        return Ok(true);
    };
    if h.assist_player(&session)?.as_ref() != Some(p) {
        return Ok(true);
    }
    let index = match b.button {
        15 => Some(0),
        20 => Some(1),
        25 => Some(2),
        30 => Some(3),
        35 => Some(4),
        40 => Some(5),
        45 => Some(6),
        50 => Some(7),
        55 => Some(8),
        _ => None,
    };
    if let Some(index) = index {
        h.assist_toggle(&session, index)?;
    }
    h.assist_refresh(&session)?;
    Ok(true)
}

// ------------------------------------------------------------------ BankDepositBoxInterface, BankChargeInterface

/// `Animations.HUMAN_BANK_DEPOSIT_BOX_834`.
pub const DEPOSIT_ANIMATION: i32 = 834;

/// `onOpen(BANK_DEPOSIT_BOX_11)`.
pub fn deposit_box_open<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<bool> {
    let server = h.server_name();
    h.set_interface_text(p, &format!("The Bank of {server} - Deposit Box"), 11, 12)?;
    Ok(true)
}

/// `on(BANK_DEPOSIT_BOX_11, ::handleDepositBoxMenu)`.
pub fn deposit_box_menu<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    let inventory = h.inventory(p)?;
    let Some(item) = h.get(&inventory, b.slot)? else {
        return Ok(true);
    };
    if b.opcode == 168 {
        let text = h.examine(&item)?.ok_or(Error::Null)?;
        h.api_message(p, &text)?;
        return Ok(true);
    }
    h.run_world_task(Task::Deposit {
        player: p.clone(),
        item,
        opcode: b.opcode,
        slot: b.slot,
    })?;
    Ok(true)
}

/// The deposit box's world task.
pub fn deposit_task<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    item: &H::Node,
    opcode: i32,
    slot: i32,
) -> Result<()> {
    match opcode {
        155 => h.bank_add_item(p, slot, 1)?,
        196 => h.bank_add_item(p, slot, 5)?,
        124 => h.bank_add_item(p, slot, 10)?,
        234 => h.transfer_x(
            p,
            slot,
            false,
            Some(Task::RefreshDepositBox { player: p.clone() }),
        )?,
        199 => {
            let inventory = h.inventory(p)?;
            let amount = h.get_amount(&inventory, item)?;
            h.bank_add_item(p, slot, amount)?;
        }
        _ => h.debug(p, &format!("Unknown deposit box menu opcode {opcode}"))?,
    }
    h.refresh_deposit_box(p)?;
    h.api_animate(p, DEPOSIT_ANIMATION, false)
}

/// `on(BANK_DEPOSIT_BOX_11, BUTTON_DEPOSIT_BOB)`.
pub fn deposit_box_beast<H: Host + ?Sized>(h: &mut H, p: &H::Node, _: Button) -> Result<bool> {
    h.dump_beast_of_burden(p)?;
    Ok(true)
}

/// `NPCs.ENIOLA_6362`.
pub const ENIOLA: i32 = 6362;

/// `BankChargeInterface.BUTTON_TO_RUNE`.
pub fn charge_rune(button: i32) -> Option<i32> {
    Some(match button {
        28 => 556,
        29 => 558,
        30 => 555,
        31 => 557,
        32 => 554,
        33 => 559,
        34 => 564,
        35 => 562,
        36 => 9075,
        37 => 563,
        38 => 560,
        39 => 565,
        40 => 561,
        41 => 566,
        _ => return None,
    })
}

/// `BankChargeInterface.handleButtonClick`.
pub fn bank_charge<H: Host + ?Sized>(h: &mut H, p: &H::Node, b: Button) -> Result<bool> {
    let Some(rune) = charge_rune(b.button) else {
        return Ok(true);
    };
    if h.amount_in_inventory(p, rune)? < 20 {
        h.npc_dialogue(
            p,
            ENIOLA,
            "I'm afraid you don't have the necessary runes with you at this time, so I can't allow you to access your account. Please bring 20 runes of one type and you'll be able to open your bank.",
            Expression::Neutral,
            false,
        )?;
        return Ok(true);
    }
    if h.remove_item(p, rune, 20, Store::Inventory)? {
        let action = h.api_get_attribute(p, "zmi:bankaction")?;
        match Value::text(action, "")?.as_str() {
            "open" => h.open_bank(p)?,
            "collect" => h.open_collection_box(p)?,
            _ => {}
        }
    }
    Ok(true)
}
