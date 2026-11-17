//! ContentAPI's interface, message and dialogue functions: interfaces and
//! tabs, messages and chat, dialogue boxes, input prompts, `runcs2`, the
//! item select prompt, the skill dialogue builder, logout listeners and hint
//! icons.
use super::{
    Arg, Error, GameTab, Host, InputType, Kind, Member as M, Result, ScriptBody, SkillHandler,
    Value, api_files, b, i, nd, new_item, state, unit,
};
use crate::world::content_api::services::{LogLevel, log};

/// `splitLines(message, perLineLimit = 54)` (DialUtils, ported in
/// [`crate::world::plain_dialogue::split_lines`]).
pub fn split_lines(message: &str, limit: i32) -> Result<Vec<String>> {
    let utf16: Vec<u16> = message.encode_utf16().collect();
    let lines = crate::world::plain_dialogue::split_lines(&utf16, limit)
        .map_err(|e| Error::Binding(Some(format!("{e:?}"))))?;
    Ok(lines.iter().map(|l| String::from_utf16_lossy(l)).collect())
}

/// `openInterface(player, id)`.
pub fn open_interface<H: Host + ?Sized>(h: &mut H, p: &H::Node, id: i32) -> Result<()> {
    unit(h, M::InterfaceOpen, vec![nd(p), Arg::component(id)])
}

/// `openOverlay(player, id)`.
pub fn open_overlay<H: Host + ?Sized>(h: &mut H, p: &H::Node, id: i32) -> Result<()> {
    unit(h, M::InterfaceOpenOverlay, vec![nd(p), Arg::component(id)])
}

/// `closeOverlay(player)`.
pub fn close_overlay<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<()> {
    unit(h, M::InterfaceCloseOverlay, vec![nd(p)])
}

/// `openSingleTab(player, component)`.
pub fn open_single_tab<H: Host + ?Sized>(h: &mut H, p: &H::Node, component: i32) -> Result<()> {
    unit(
        h,
        M::InterfaceOpenSingleTab,
        vec![nd(p), Arg::component(component)],
    )
}

/// `closeInterface(player)`.
pub fn close_interface<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<()> {
    b(h, M::InterfaceClose, vec![nd(p)]).map(|_| ())
}

/// `closeTabInterface(player)`.
pub fn close_tab_interface<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<()> {
    b(h, M::InterfaceCloseSingleTab, vec![nd(p)]).map(|_| ())
}

/// `closeAllInterfaces(player)`.
pub fn close_all_interfaces<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<()> {
    b(h, M::InterfaceClose, vec![nd(p)])?;
    unit(h, M::InterfaceCloseChatbox, vec![nd(p)])?;
    b(h, M::DialogueClose, vec![nd(p)]).map(|_| ())
}

/// `setInterfaceText(player, string, iface, child)` and `sendString(...)`.
pub fn set_interface_text<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    text: &str,
    iface: i32,
    child: i32,
) -> Result<()> {
    unit(
        h,
        M::SendString,
        vec![nd(p), Arg::text(text), Arg::Int(iface), Arg::Int(child)],
    )
}

/// `setComponentVisibility(player, iface, child, hide)`.
pub fn set_component_visibility<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    iface: i32,
    child: i32,
    hide: bool,
) -> Result<()> {
    unit(
        h,
        M::SendInterfaceConfig,
        vec![nd(p), Arg::Int(iface), Arg::Int(child), Arg::Bool(hide)],
    )
}

/// `setTitle(player, options)`: the check for more than five options comes
/// after both packets.
pub fn set_title<H: Host + ?Sized>(h: &mut H, p: &H::Node, options: i32) -> Result<()> {
    let iface = match options {
        5 => 234,
        4 => 232,
        3 => 230,
        _ => 228,
    };
    set_component_visibility(h, p, iface, 4i32.wrapping_add(options), true)?;
    set_component_visibility(
        h,
        p,
        iface,
        if options == 2 || options == 4 { 9 } else { 10 },
        false,
    )?;
    if options > 5 {
        return Err(Error::Argument(
            "Expected option value between 2 and 5, got Integer.".into(),
        ));
    }
    Ok(())
}

/// `removeTabs(player, tabs...)`.
pub fn remove_tabs<H: Host + ?Sized>(h: &mut H, p: &H::Node, tabs: &[GameTab]) -> Result<()> {
    let indices: Vec<i32> = tabs
        .iter()
        .flat_map(|t| t.indices().iter().copied())
        .collect();
    unit(h, M::InterfaceRemoveTabs, vec![nd(p), Arg::ints(&indices)])
}

/// `restoreTabs(player)`.
pub fn restore_tabs<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<()> {
    unit(h, M::InterfaceRestoreTabs, vec![nd(p)])
}

/// `sendPlayerOnInterface(player, iface, child)`.
pub fn send_player_on_interface<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    iface: i32,
    child: i32,
) -> Result<()> {
    unit(
        h,
        M::SendPlayerOnInterface,
        vec![nd(p), Arg::Int(iface), Arg::Int(child)],
    )
}

/// `sendAnimationOnInterface(player, anim, iface, child)`.
pub fn send_animation_on_interface<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    anim: i32,
    iface: i32,
    child: i32,
) -> Result<()> {
    unit(
        h,
        M::SendAnimationInterface,
        vec![nd(p), Arg::Int(anim), Arg::Int(iface), Arg::Int(child)],
    )
}

/// `sendItemOnInterface(player, iface, child, item, amount = 1)`.
pub fn send_item_on_interface<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    iface: i32,
    child: i32,
    item: i32,
    amount: i32,
) -> Result<()> {
    unit(
        h,
        M::SendItemOnInterface,
        vec![
            nd(p),
            Arg::Int(item),
            Arg::Int(amount),
            Arg::Int(iface),
            Arg::Int(child),
        ],
    )
}

/// `sendItemZoomOnInterface(player, iface, child, item, zoom = 230)`.
pub fn send_item_zoom_on_interface<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    iface: i32,
    child: i32,
    item: i32,
    zoom: i32,
) -> Result<()> {
    unit(
        h,
        M::SendItemZoomOnInterface,
        vec![
            nd(p),
            Arg::Int(item),
            Arg::Int(zoom),
            Arg::Int(iface),
            Arg::Int(child),
        ],
    )
}

/// `sendMessage(player, message)`: split at 86 characters.
pub fn send_message<H: Host + ?Sized>(h: &mut H, p: &H::Node, message: &str) -> Result<()> {
    let lines = split_lines(message, 86)?;
    unit(h, M::PlayerSendMessages, vec![nd(p), Arg::texts(&lines)])
}

/// `sendChat(entity, message, delay = -1)`: with a delay, a SOFT script.
pub fn send_chat<H: Host + ?Sized>(
    h: &mut H,
    e: &H::Node,
    message: &str,
    delay: i32,
) -> Result<()> {
    if delay > -1 {
        let body = ScriptBody::SendChat {
            entity: e.clone(),
            message: message.into(),
        };
        state::queue_script(h, e, delay, state::QueueStrength::Soft, false, body)
    } else {
        b(h, M::EntitySendChat, vec![nd(e), Arg::text(message)]).map(|_| ())
    }
}

/// `sendNews(message)`: icon 12, colour CC6600.
pub fn send_news<H: Host + ?Sized>(h: &mut H, message: &str) -> Result<()> {
    unit(
        h,
        M::SendNews,
        vec![Arg::text(message), Arg::Int(12), Arg::text("CC6600")],
    )
}

/// `sendDialogue(player, message)`.
pub fn send_dialogue<H: Host + ?Sized>(h: &mut H, p: &H::Node, message: &str) -> Result<()> {
    let lines = split_lines(message, 54)?;
    h.call(M::DialogueSendDialogue, vec![nd(p), Arg::texts(&lines)])
        .map(|_| ())
}

/// `sendDialogueLines(player, lines...)`.
pub fn send_dialogue_lines<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    lines: &[String],
) -> Result<()> {
    h.call(M::DialogueSendDialogue, vec![nd(p), Arg::texts(lines)])
        .map(|_| ())
}

/// `sendDialogueOptions(player, title, options...)`.
pub fn send_dialogue_options<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    title: &str,
    options: &[String],
) -> Result<()> {
    h.call(
        M::DialogueSendOptions,
        vec![nd(p), Arg::text(title), Arg::texts(options)],
    )
    .map(|_| ())
}

/// `sendPlayerDialogue(player, msg, expr = NEUTRAL, hide = false)`.
pub fn send_player_dialogue<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    msg: &str,
    expr: &'static str,
    hide: bool,
) -> Result<()> {
    let lines = split_lines(msg, 54)?;
    h.call(
        M::DialogueSendDialogues,
        vec![
            nd(p),
            nd(p),
            Arg::Enum(expr),
            Arg::Bool(hide),
            Arg::texts(&lines),
        ],
    )
    .map(|_| ())
}

/// `sendNPCDialogue(player, npc, msg, expr = NEUTRAL, hide = false)`.
pub fn send_npc_dialogue<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    npc: i32,
    msg: &str,
    expr: &'static str,
    hide: bool,
) -> Result<()> {
    let lines = split_lines(msg, 54)?;
    h.call(
        M::DialogueSendDialogues,
        vec![
            nd(p),
            Arg::Int(npc),
            Arg::Enum(expr),
            Arg::Bool(hide),
            Arg::texts(&lines),
        ],
    )
    .map(|_| ())
}

/// `sendNPCDialogueLines(player, npc, expr, hideContinue, msgs...)`.
pub fn send_npc_dialogue_lines<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    npc: i32,
    expr: &'static str,
    hide: bool,
    msgs: &[String],
) -> Result<()> {
    let component = h
        .call(
            M::DialogueSendDialogues,
            vec![nd(p), Arg::Int(npc), Arg::Enum(expr), Arg::texts(msgs)],
        )?
        .component()?
        .ok_or(Error::Null)?;
    set_component_visibility(h, p, component, (msgs.len() as i32).wrapping_add(4), hide)
}

/// The class name of a value in a message (`javaClass.simpleName`).
pub fn simple_name<H: Host + ?Sized>(h: &H, v: &Value<H::Node>) -> &'static str {
    match v {
        Value::Null => "Void",
        Value::Int(_) => "Integer",
        Value::Long(_) => "Long",
        Value::Double(_) => "Double",
        Value::Bool(_) => "Boolean",
        Value::Text(_) => "String",
        Value::Loc(_) => "Location",
        Value::Node(n) => h.kind(n).simple_name(),
        Value::Anim(_) => "Animation",
        Value::Gfx(_) => "Graphics",
        Value::Enum(_) => "Enum",
    }
}

/// `sendItemDialogue(player, item, message)`: an Item or an Int.
pub fn send_item_dialogue<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    item: &Value<H::Node>,
    message: &str,
) -> Result<()> {
    let shown = match item {
        Value::Node(n) if h.kind(n).is_item() => n.clone(),
        Value::Int(id) => new_item(h, *id, 1, 1000)?,
        other => {
            return Err(Error::Argument(format!(
                "Expected an Item or an Int, got {}.",
                simple_name(h, other)
            )));
        }
    };
    let lines = split_lines(message, 54)?;
    h.call(
        M::DialogueSendItem,
        vec![nd(p), nd(&shown), Arg::texts(&lines)],
    )
    .map(|_| ())
}

/// `sendDoubleItemDialogue(player, item1, item2, message)`: two Items or
/// two Ints (the second is cast to the first's type); anything else does
/// nothing.
pub fn send_double_item_dialogue<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    first: &Value<H::Node>,
    second: &Value<H::Node>,
    message: &str,
) -> Result<()> {
    let (a, b2) = match first {
        Value::Node(n) if h.kind(n).is_item() => match second {
            Value::Node(m) if h.kind(m).is_item() => (nd(n), nd(m)),
            Value::Null => return Err(Error::Null),
            _ => return Err(Error::Cast),
        },
        Value::Int(x) => match second {
            Value::Int(y) => (Arg::Int(*x), Arg::Int(*y)),
            Value::Null => return Err(Error::Null),
            _ => return Err(Error::Cast),
        },
        _ => return Ok(()),
    };
    let lines = split_lines(message, 54)?;
    h.call(
        M::DialogueSendDoubleItem,
        vec![nd(p), a, b2, Arg::texts(&lines)],
    )
    .map(|_| ())
}

/// `sendInputDialogue(player, numeric, prompt, handler)`.
pub fn send_input_dialogue<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    numeric: bool,
    prompt: &str,
    handler: &H::Node,
) -> Result<()> {
    let kind = if numeric {
        InputType::Numeric
    } else {
        InputType::StringShort
    };
    send_input_dialogue_typed(h, p, kind, prompt, handler)
}

/// `sendInputDialogue(player, type, prompt, handler)`.
pub fn send_input_dialogue_typed<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    kind: InputType,
    prompt: &str,
    handler: &H::Node,
) -> Result<()> {
    let prompt_list = Arg::List(vec![Arg::text(prompt)]);
    match kind {
        InputType::Amount => {
            state::set_attribute(h, p, "parseamount", &Value::Bool(true))?;
            unit(
                h,
                M::DialogueSendInput,
                vec![nd(p), Arg::Bool(true), prompt_list],
            )?;
        }
        InputType::Numeric | InputType::StringShort => {
            unit(
                h,
                M::DialogueSendInput,
                vec![nd(p), Arg::Bool(kind != InputType::Numeric), prompt_list],
            )?;
        }
        InputType::StringLong => unit(h, M::DialogueSendLongInput, vec![nd(p), prompt_list])?,
        InputType::Message => unit(
            h,
            M::DialogueSendMessageInput,
            vec![nd(p), Arg::text(prompt)],
        )?,
    }
    state::set_attribute(h, p, "runscript", &Value::Node(handler.clone()))?;
    state::set_attribute(h, p, "input-type", &Value::Enum(kind.name()))
}

/// `openDialogue(player, dialogue, args...)`: closes the current dialogue,
/// then opens a key, a file or a skill handler; another object is logged.
pub fn open_dialogue<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    dialogue: &Value<H::Node>,
    args: &[Value<H::Node>],
) -> Result<()> {
    b(h, M::DialogueClose, vec![nd(p)])?;
    let list = Arg::List(args.iter().map(Arg::value).collect());
    match dialogue {
        Value::Int(key) => b(h, M::DialogueOpenKey, vec![nd(p), Arg::Int(*key), list]).map(|_| ()),
        Value::Node(f) if h.kind(f) == Kind::DialogueFile => {
            unit(h, M::DialogueOpenFile, vec![nd(p), nd(f), list])
        }
        Value::Node(f) if h.kind(f) == Kind::SkillHandler => Err(Error::Binding(Some(
            "a SkillDialogueHandler object is opened by its owner".into(),
        ))),
        other => {
            let name = simple_name(h, other);
            log(
                h,
                "ContentAPI",
                LogLevel::Err,
                &format!("Invalid object type passed to openDialogue() -> {name}"),
            )
        }
    }
}

/// `closeDialogue(player)`.
pub fn close_dialogue<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<()> {
    b(h, M::DialogueClose, vec![nd(p)])?;
    unit(h, M::InterfaceCloseChatbox, vec![nd(p)])
}

/// `addDialogueAction(player, action)`.
pub fn add_dialogue_action<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    action: &H::Node,
) -> Result<()> {
    unit(h, M::DialogueAddAction, vec![nd(p), nd(action)])
}

/// A `toString` of a runcs2 argument in its message.
fn display<H: Host + ?Sized>(h: &H, v: &Value<H::Node>) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Int(i) => i.to_string(),
        Value::Long(l) => l.to_string(),
        Value::Double(d) => crate::world::content_api::api_files::java_double_string(*d),
        Value::Bool(b) => b.to_string(),
        Value::Text(t) => t.clone(),
        Value::Loc(l) => l.to_string(),
        Value::Node(n) => h.name(n).unwrap_or_default(),
        Value::Anim(a) => format!("Animation {}", a.id),
        Value::Gfx(g) => format!("Graphics {}", g.id),
        Value::Enum(e) => (*e).into(),
    }
}

/// `runcs2(player, scriptId, arguments...)`: Ints and Strings only, passed
/// in reverse; any failure is printed and swallowed.
pub fn runcs2<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    script: i32,
    args: &[Value<H::Node>],
) -> Result<()> {
    let attempt = (|| -> Result<()> {
        let mut types = String::new();
        let mut finals = vec![Arg::Null; args.len()];
        for (index, arg) in args.iter().enumerate() {
            match arg {
                Value::Int(_) => types.push('i'),
                Value::Text(_) => types.push('s'),
                other => {
                    return Err(Error::Argument(format!(
                        "Argument at index {index} ({}) is not an acceptable type! Only string and int are accepted.",
                        display(h, other)
                    )));
                }
            }
            finals[args.len() - index - 1] = Arg::value(arg);
        }
        unit(
            h,
            M::SendRunScript,
            vec![nd(p), Arg::Int(script), Arg::Text(types), Arg::List(finals)],
        )
    })();
    match attempt {
        Ok(()) => Ok(()),
        Err(e) => h.print_stack_trace(&e),
    }
}

/// `sendItemSelect(player, options..., keepAlive = false, callback)`.
pub fn send_item_select<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    options: &[String],
    keep_alive: bool,
    callback: &H::Node,
) -> Result<()> {
    open_single_tab(h, p, 12)?;
    let mut script: Vec<Value<H::Node>> = vec![
        Value::Int((12 << 16) + 18),
        Value::Int(93),
        Value::Int(4),
        Value::Int(7),
        Value::Int(0),
        Value::Int(-1),
    ];
    script.extend((0..9).map(|_| Value::Text(String::new())));
    for (index, option) in options.iter().take(9).enumerate() {
        script[6 + index] = Value::Text(option.clone());
    }
    runcs2(h, p, 150, &script)?;
    let settings = api_files::IfaceSettingsBuilder::default()
        .enable_options_range(0, 9)?
        .build();
    unit(
        h,
        M::SendIfaceSettings,
        vec![
            nd(p),
            Arg::Int(settings),
            Arg::Int(18),
            Arg::Int(12),
            Arg::Int(0),
            Arg::Int(28),
        ],
    )?;
    state::set_attribute(h, p, "itemselect-callback", &Value::Node(callback.clone()))?;
    state::set_attribute(h, p, "itemselect-keepalive", &Value::Bool(keep_alive))
}

/// What `sendSkillDialogue`'s `init` configures on its builder.
#[derive(Clone, Debug, PartialEq)]
pub enum SkillItems<N> {
    None,
    /// `withItems(vararg item: Item)`.
    Items(Vec<N>),
    /// `withItems(vararg item: Int)`: `Item(id)` each.
    Ids(Vec<i32>),
}

/// `SkillDialogueHandler.SkillDialogue` by item count.
pub const SKILL_DIALOGUES: [&str; 5] = [
    "ONE_OPTION",
    "TWO_OPTION",
    "THREE_OPTION",
    "FOUR_OPTION",
    "FIVE_OPTION",
];

/// `sendSkillDialogue(player) { ... }`: one to five items.
pub fn send_skill_dialogue<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    items: SkillItems<H::Node>,
    create: Option<&H::Node>,
    max: Option<&H::Node>,
) -> Result<()> {
    let items = match items {
        SkillItems::None => Vec::new(),
        SkillItems::Items(i) => i,
        SkillItems::Ids(ids) => {
            let mut out = Vec::new();
            for id in ids {
                out.push(new_item(h, id, 1, 1000)?);
            }
            out
        }
    };
    if !(1..=5).contains(&items.len()) {
        return Err(Error::State(format!(
            "Invalid number of items passed to skill dialogue (min 1, max 5): {}",
            items.len()
        )));
    }
    let kind = SKILL_DIALOGUES[items.len() - 1];
    let handler = SkillHandler {
        player: p.clone(),
        items,
        create: create.cloned(),
        max: max.cloned(),
    };
    h.open_skill_dialogue(handler, kind)
}

/// The handler's `create(amount, index)`.
pub fn skill_create<H: Host + ?Sized>(
    h: &mut H,
    handler: &SkillHandler<H::Node>,
    amount: i32,
    index: i32,
) -> Result<()> {
    let item = handler
        .items
        .get(index as usize)
        .filter(|_| index >= 0)
        .ok_or(Error::Index)?
        .clone();
    let id = h.id(&item);
    match &handler.create {
        Some(f) => unit(h, M::Callback, vec![nd(f), Arg::Int(id), Arg::Int(amount)]),
        None => Ok(()),
    }
}

/// The handler's `getAll(index)`.
pub fn skill_get_all<H: Host + ?Sized>(
    h: &mut H,
    handler: &SkillHandler<H::Node>,
    index: i32,
) -> Result<i32> {
    let item = handler
        .items
        .get(index as usize)
        .filter(|_| index >= 0)
        .ok_or(Error::Index)?
        .clone();
    let id = h.id(&item);
    match &handler.max {
        Some(f) => i(h, M::CallbackInt, vec![nd(f), Arg::Int(id)]),
        None => {
            let p = handler.player.clone();
            crate::world::content_api::items::amount_in_inventory(h, &p, id)
        }
    }
}

/// `registerLogoutListener(player, key, handler)`.
pub fn register_logout_listener<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    key: &str,
    handler: &H::Node,
) {
    h.set_logout_listener(p, key, Some(handler.clone()));
}

/// `clearLogoutListener(player, key)`.
pub fn clear_logout_listener<H: Host + ?Sized>(h: &mut H, p: &H::Node, key: &str) {
    h.set_logout_listener(p, key, None);
}

/// `registerHintIcon(player, location, height)`.
pub fn register_hint_icon_at<H: Host + ?Sized>(
    h: &mut H,
    p: &H::Node,
    at: super::Location,
    height: i32,
) -> Result<()> {
    let slot = i(h, M::HintFreeSlot, vec![nd(p)])?;
    let args = vec![
        nd(p),
        Arg::Loc(at),
        Arg::Int(1),
        Arg::Int(-1),
        Arg::Int(slot),
        Arg::Int(height),
        Arg::Int(3),
    ];
    let icon = i(h, M::HintRegister, args)?;
    state::set_attribute(h, p, "hinticon", &Value::Int(icon))
}

/// `registerHintIcon(player, node)`: only without an active ContentAPI icon.
pub fn register_hint_icon<H: Host + ?Sized>(h: &mut H, p: &H::Node, node: &H::Node) -> Result<()> {
    if state::get_attribute(h, p, "hinticon", Value::Null)? != Value::Null {
        return Ok(());
    }
    let icon = i(h, M::HintRegister, vec![nd(p), nd(node)])?;
    state::set_attribute(h, p, "hinticon", &Value::Int(icon))
}

/// `clearHintIcon(player)`.
pub fn clear_hint_icon<H: Host + ?Sized>(h: &mut H, p: &H::Node) -> Result<()> {
    let slot = match state::get_attribute(h, p, "hinticon", Value::Int(-1))? {
        Value::Int(s) => s,
        _ => return Err(Error::Cast),
    };
    unit(
        h,
        M::EntityRemoveAttribute,
        vec![nd(p), Arg::text("hinticon")],
    )?;
    unit(h, M::HintRemove, vec![nd(p), Arg::Int(slot)])
}
