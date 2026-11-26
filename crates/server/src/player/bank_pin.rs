//! Pinned BankPinManager and PinStatus algorithms. Text remains Java UTF-16;
//! clocks, RNG and actual player/interface/packet services are required inputs.
//! No field guard spans those services or a captured close callback.
use crate::world::component_objects::{self as objects, NativeObject};
use crate::{definitions::item::Shared, world::player_walk::NodeHandle};
use rs09_game_core::{
    java_json::Json,
    java_string::{parse_i32, parse_i64},
    random::JavaRandom,
};
pub type Text = Vec<u16>;
pub fn text(s: &str) -> Text {
    s.encode_utf16().collect()
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Null,
    Type,
    Number,
    Index,
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Status {
    NoPin = 0,
    Pending = 1,
    Active = 2,
}
pub const SET_PIN: i32 = 60;
pub const CHANGE_RECOVERY: i32 = 61;
pub const CHANGE_PIN: i32 = 62;
pub const DELETE_PIN: i32 = 63;
pub const CANCEL_PENDING: i32 = 65;
pub const PENDING_AUDIO: i32 = 1040;
pub const BEEP_AUDIO: i32 = 1041;
pub const CANCEL_AUDIO: i32 = 1042;
pub const WRONG_AUDIO: i32 = 2277;
pub const SUCCESS_AUDIO: i32 = 2274;
const SETTINGS: [Option<&str>; 13] = [
    Some("Customers are reminded that"),
    Some("they should NEVER tell"),
    Some("anyone their Bank PINs or"),
    Some("passwords, nor should they"),
    Some("ever enter their PINs on any"),
    Some("website form."),
    None,
    Some("Have you read the PIN guide"),
    Some("on the website?"),
    None,
    None,
    None,
    None,
];
pub struct Fields {
    player: Option<NodeHandle>,
    pub digits: Vec<i32>,
    pub pin: Option<Text>,
    pub unlocked: bool,
    pub long_recovery: bool,
    pub status: Option<Status>,
    pub temp_pin: Option<Text>,
    pub pending_delay: i64,
    pub open_id: i32,
    pub tries: i32,
    pub change_state: i32,
    pub deleting: bool,
    pub try_delay: i64,
}
#[derive(Clone)]
pub struct Manager(Shared<Fields>);
/// The real close event captures its manager, not the callback's player argument.
#[derive(Clone)]
pub struct CloseEvent(objects::Close);
struct CloseFields {
    manager: Manager,
}
impl NativeObject for CloseFields {
    fn captures(&self) -> objects::Captures {
        objects::Captures {
            pins: vec![self.manager.clone()],
            ..Default::default()
        }
    }
}
impl CloseEvent {
    pub fn new(manager: Manager) -> Self {
        Self(objects::Close::new(CloseFields { manager }))
    }
    pub fn manager(&self) -> Manager {
        self.0.value::<CloseFields>().unwrap().manager.clone()
    }
    pub fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
    pub fn component_event(&self) -> objects::Close {
        self.0.clone()
    }
    pub fn from_component_event(event: &objects::Close) -> Option<Self> {
        event.value::<CloseFields>().map(|_| Self(event.clone()))
    }
    pub fn close(&self) -> bool {
        let manager = self.manager();
        let mut f = manager.0.write();
        if f.change_state != 0 {
            f.change_state = 0;
        }
        if f.deleting {
            f.deleting = false;
        }
        true
    }
}
pub trait Context {
    type Interfaces;
    type Dispatch;
    type Component;
    type Dialogue;
    type Bank;
    type Exchange;
    type Settings;
    fn now(&mut self) -> Result<i64>;
    fn shuffle(&mut self, digits: &mut [i32]) -> Result<()>;
    fn random(&mut self, min: i32, max: i32) -> Result<i32>;
    fn interfaces(&mut self, player: Option<&NodeHandle>) -> Result<Self::Interfaces>;
    fn close(&mut self, manager: Self::Interfaces) -> Result<()>;
    fn component(&mut self, id: i32) -> Result<Self::Component>;
    fn close_event(
        &mut self,
        component: Self::Component,
        event: CloseEvent,
    ) -> Result<Self::Component>;
    fn open(&mut self, manager: Self::Interfaces, component: Self::Component) -> Result<()>;
    fn dispatch(&mut self, player: Option<&NodeHandle>) -> Result<Self::Dispatch>;
    fn dispatch_string(
        &mut self,
        dispatch: Self::Dispatch,
        value: &[u16],
        interface: i32,
        child: i32,
    ) -> Result<()>;
    fn dispatch_config(
        &mut self,
        dispatch: Self::Dispatch,
        interface: i32,
        child: i32,
        hidden: bool,
    ) -> Result<()>;
    fn dispatch_message(&mut self, dispatch: Self::Dispatch, value: &[u16]) -> Result<()>;
    fn string_packet(
        &mut self,
        player: Option<&NodeHandle>,
        value: &[u16],
        interface: i32,
        child: i32,
    ) -> Result<()>;
    fn reposition(
        &mut self,
        player: Option<&NodeHandle>,
        interface: i32,
        child: i32,
        x: i32,
        y: i32,
    ) -> Result<()>;
    fn audio(&mut self, player: Option<&NodeHandle>, id: i32) -> Result<()>;
    fn message(&mut self, player: Option<&NodeHandle>, value: &[u16]) -> Result<()>;
    fn dialogue(&mut self, player: Option<&NodeHandle>) -> Result<Self::Dialogue>;
    fn send_dialogue(&mut self, dialogue: Self::Dialogue, value: &[u16]) -> Result<()>;
    fn bank(&mut self, player: Option<&NodeHandle>) -> Result<Self::Bank>;
    fn open_bank(&mut self, bank: Self::Bank) -> Result<()>;
    fn exchange(&mut self, player: Option<&NodeHandle>) -> Result<Self::Exchange>;
    fn collection(&mut self, exchange: Self::Exchange) -> Result<()>;
    fn market(&mut self, player: Option<&NodeHandle>) -> Result<()>;
    fn settings(&mut self) -> Result<Self::Settings>;
    fn world_name(&mut self, settings: Self::Settings) -> Result<Option<Text>>;
    fn dev_mode(&mut self, settings: Self::Settings) -> Result<bool>;
    fn varp(&mut self, player: Option<&NodeHandle>, id: i32, value: i32) -> Result<()>;
}
/// Collections.shuffle's RandomAccess list path, with the actual Java RNG.
/// The caller owns RNG lifetime/seed rather than reseeding each keypad shuffle.
pub fn shuffle(digits: &mut [i32], random: &mut JavaRandom) -> Result<()> {
    if digits.len() > i32::MAX as usize {
        return Err(Error::Index);
    }
    for i in (2..=digits.len()).rev() {
        let j = random.next_bound(i as i32).ok_or(Error::Index)? as usize;
        digits.swap(i - 1, j);
    }
    Ok(())
}
/// Concrete stateless BankPinInterface instance. Registries retain this actual
/// object; observation labels do not select the native program.
#[derive(Clone)]
pub struct Program(objects::Plugin);
struct PinProgram;
impl NativeObject for PinProgram {}
impl Default for Program {
    fn default() -> Self {
        Self(objects::Plugin::new(PinProgram))
    }
}
pub trait InterfaceContext: Context {
    type InputComponent;
    type Definition;
    fn player_manager(&mut self, player: Option<&NodeHandle>) -> Result<Option<Manager>>;
    fn component_id(&mut self, component: &Self::InputComponent) -> Result<i32>;
    fn definition(&mut self, id: i32) -> Result<Self::Definition>;
    fn set_plugin(&mut self, definition: Self::Definition, program: Program) -> Result<()>;
}
impl Program {
    pub fn component_plugin(&self) -> objects::Plugin {
        self.0.clone()
    }
    pub fn from_component_plugin(plugin: &objects::Plugin) -> Option<Self> {
        plugin.value::<PinProgram>().map(|_| Self(plugin.clone()))
    }
    pub fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
    pub fn register(&self, ctx: &mut impl InterfaceContext) -> Result<Self> {
        let definition = ctx.definition(13)?;
        ctx.set_plugin(definition, self.clone())?;
        let definition = ctx.definition(14)?;
        ctx.set_plugin(definition, self.clone())?;
        Ok(self.clone())
    }
    pub fn handle<C: InterfaceContext>(
        &self,
        ctx: &mut C,
        player: Option<&NodeHandle>,
        component: &C::InputComponent,
        button: i32,
    ) -> Result<bool> {
        let manager = ctx.player_manager(player)?;
        match ctx.component_id(component)? {
            14 => match button {
                60 | 62 => {
                    let manager = manager.ok_or(Error::Null)?;
                    if !manager.has_pin() {
                        manager.toggle_confirm(ctx, button == 60)?;
                    } else {
                        manager.0.write().change_state = 1;
                        manager.open_pin(ctx)?;
                    }
                }
                63 => manager.ok_or(Error::Null)?.toggle_confirm(ctx, true)?,
                65 => manager.ok_or(Error::Null)?.cancel(
                    ctx,
                    &[
                        "The PIN has been cancelled",
                        "and will NOT be set.",
                        "",
                        "You still do not have a Bank",
                        "PIN.",
                    ],
                )?,
                89 | 91 => manager.ok_or(Error::Null)?.confirm(ctx, button)?,
                61 | 64 => manager.ok_or(Error::Null)?.switch_recovery(ctx)?,
                _ => {}
            },
            13 if button != 30 => {
                let manager = ctx.player_manager(player)?.ok_or(Error::Null)?;
                manager.update_temp(ctx, button.wrapping_sub(1))?;
            }
            _ => {}
        }
        Ok(true)
    }
}
impl Manager {
    pub fn new(player: Option<NodeHandle>) -> Self {
        Self(Shared::new(Fields {
            player,
            digits: (0..10).collect(),
            pin: None,
            unlocked: false,
            long_recovery: false,
            status: Some(Status::NoPin),
            temp_pin: None,
            pending_delay: -1,
            open_id: 0,
            tries: 0,
            change_state: 0,
            deleting: false,
            try_delay: 0,
        }))
    }
    pub fn fields(&self) -> Shared<Fields> {
        self.0.clone()
    }
    pub fn player(&self) -> Option<NodeHandle> {
        self.0.read().player.clone()
    }
    pub fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
    pub(crate) fn ownership(&self) -> (usize, usize, Option<NodeHandle>) {
        (self.0.identity(), self.0.strong_count(), self.player())
    }
    pub fn has_pin(&self) -> bool {
        self.0.read().pin.is_some()
    }
    pub fn is_unlocked(&self) -> bool {
        !self.has_pin() || self.0.read().unlocked
    }
    pub fn recovery_delay(&self) -> i32 {
        if self.0.read().long_recovery { 7 } else { 3 }
    }
    pub fn stage(&self) -> i32 {
        self.0
            .read()
            .temp_pin
            .as_ref()
            .map_or(0, |s| s.len() as i32)
    }
    pub fn pin_seconds(&self) -> i32 {
        match self.0.read().tries {
            2 => 10,
            3 => 15,
            n if n > 3 => 1000,
            _ => 0,
        }
    }
    pub fn set_tries(&self, tries: i32) -> i32 {
        self.0.write().tries = tries;
        tries
    }
    pub fn parse(&self, data: Option<&Json>) -> Result<()> {
        let data = data.ok_or(Error::Null)?;
        if !matches!(data, Json::Object(_)) {
            return Err(if matches!(data, Json::Null) {
                Error::Null
            } else {
                Error::Type
            });
        }
        if let Some(pin) = data.get("pin") {
            if matches!(pin, Json::Null) {
                return Err(Error::Null);
            }
            self.0.write().pin = Some(text(&pin.text()));
        }
        if let Some(status) = data.get("status") {
            if matches!(status, Json::Null) {
                return Err(Error::Null);
            }
            let n = parse_i32(&status.text()).ok_or(Error::Number)?;
            self.0.write().status = Some(match n {
                0 => Status::NoPin,
                1 => Status::Pending,
                2 => Status::Active,
                _ => return Err(Error::Index),
            });
        }
        for (key, pending) in [("pendingDelay", true), ("tryDelay", false)] {
            if let Some(v) = data.get(key) {
                if matches!(v, Json::Null) {
                    return Err(Error::Null);
                }
                let n = parse_i64(&v.text()).ok_or(Error::Number)?;
                if pending {
                    self.0.write().pending_delay = n;
                } else {
                    self.0.write().try_delay = n;
                }
            }
        }
        match data.get("longRecovery") {
            Some(Json::Bool(v)) => self.0.write().long_recovery = *v,
            None | Some(Json::Null) => return Err(Error::Null),
            _ => return Err(Error::Type),
        }
        Ok(())
    }
    fn string(&self, ctx: &mut impl Context, s: &[u16], interface: i32, child: i32) -> Result<()> {
        let player = self.player();
        let dispatch = ctx.dispatch(player.as_ref())?;
        ctx.dispatch_string(dispatch, s, interface, child)
    }
    fn draw_string(&self, ctx: &mut impl Context, s: &str, child: i32) -> Result<()> {
        self.string(ctx, &text(s), 14, child)
    }
    fn config(
        &self,
        ctx: &mut impl Context,
        interface: i32,
        child: i32,
        hidden: bool,
    ) -> Result<()> {
        let player = self.player();
        let dispatch = ctx.dispatch(player.as_ref())?;
        ctx.dispatch_config(dispatch, interface, child, hidden)
    }
    fn audio(&self, ctx: &mut impl Context, id: i32) -> Result<()> {
        ctx.audio(self.player().as_ref(), id)
    }
    fn close(&self, ctx: &mut impl Context) -> Result<()> {
        let manager = ctx.interfaces(self.player().as_ref())?;
        ctx.close(manager)
    }
    fn check_pending(&self, ctx: &mut impl Context) -> Result<()> {
        if self.0.read().status == Some(Status::Pending) {
            let delay = self.0.read().pending_delay;
            if delay < ctx.now()? {
                let mut f = self.0.write();
                f.pending_delay = -1;
                f.status = Some(Status::Active);
            }
        }
        Ok(())
    }
    pub fn pending_days(&self, ctx: &mut impl Context) -> Result<i32> {
        let delay = self.0.read().pending_delay;
        if delay < ctx.now()? {
            return Ok(0);
        }
        let delay = self.0.read().pending_delay;
        let now = ctx.now()?;
        let days = delay.wrapping_sub(now) as i32;
        let settings = ctx.settings()?;
        let dev = ctx.dev_mode(settings)?;
        Ok(days / if dev { 1000 } else { 86_400_000 })
    }
    pub fn has_pending_pin(&self, ctx: &mut impl Context) -> Result<bool> {
        Ok(self.pending_days(ctx)? > 0)
    }
    pub fn blocked(&self, ctx: &mut impl Context) -> Result<bool> {
        let delay = self.0.read().try_delay;
        Ok(delay > ctx.now()?)
    }
    pub fn open_settings(&self, ctx: &mut impl Context) -> Result<()> {
        self.check_pending(ctx)?;
        let status = self.0.read().status;
        let messages = self.messages(ctx, status)?;
        self.open_settings_messages(ctx, Some(&messages))
    }
    pub fn open_settings_messages(
        &self,
        ctx: &mut impl Context,
        messages: Option<&[Option<Text>]>,
    ) -> Result<()> {
        self.close(ctx)?;
        self.0.write().temp_pin = None;
        self.send_messages(ctx, messages)?;
        self.audio(ctx, PENDING_AUDIO)?;
        let interfaces = ctx.interfaces(self.player().as_ref())?;
        let component = ctx.component(14)?;
        ctx.open(interfaces, component)
    }
    fn send_messages(
        &self,
        ctx: &mut impl Context,
        messages: Option<&[Option<Text>]>,
    ) -> Result<()> {
        for (i, message) in messages.ok_or(Error::Null)?.iter().enumerate() {
            if let Some(message) = message {
                ctx.string_packet(
                    self.player().as_ref(),
                    message,
                    14,
                    42i32.wrapping_add(i as i32),
                )?;
            }
        }
        let status = self.0.read().status;
        self.draw_status(ctx, status)?;
        self.draw_string(ctx, "Messages", 41)?;
        self.draw_string(ctx, &format!("{} days", self.recovery_delay()), 71)?;
        self.draw_string(ctx, "Use the buttons below to change your PIN settings", 86)
    }
    pub fn open_type(&self, ctx: &mut impl Context, button: i32) -> Result<()> {
        self.check_pending(ctx)?;
        if button == 2 {
            return self.open_settings(ctx);
        }
        self.0.write().open_id = button;
        if self.has_pin() && !self.0.read().unlocked {
            if self.has_pending_pin(ctx)? {
                self.open_settings(ctx)?;
                self.toggle_confirm(ctx, true)?;
                let days = self.pending_days(ctx)?;
                let plural = if self.pending_days(ctx)? > 1 { "s" } else { "" };
                self.draw_string(
                    ctx,
                    &format!(
                        "A PIN will be set on your bank account in another {days} day{plural}."
                    ),
                    73,
                )?;
                self.draw_string(ctx, "Yes, I asked for this. I want this PIN.", 89)?;
                return self.draw_string(ctx, "No, I didn't ask for this. Cancel it.", 91);
            }
            return self.open_pin(ctx);
        }
        self.open_for_id(ctx, button)
    }
    fn open_for_id(&self, ctx: &mut impl Context, button: i32) -> Result<()> {
        match button {
            1 => {
                let bank = ctx.bank(self.player().as_ref())?;
                ctx.open_bank(bank)
            }
            3 => {
                let exchange = ctx.exchange(self.player().as_ref())?;
                ctx.collection(exchange)
            }
            4 => ctx.market(self.player().as_ref()),
            _ => Ok(()),
        }
    }
    pub fn confirm(&self, ctx: &mut impl Context, button: i32) -> Result<()> {
        let confirm = button != 91;
        let status = self.0.read().status.ok_or(Error::Null)?;
        match status {
            Status::NoPin => {
                if !confirm {
                    self.toggle_confirm(ctx, false)?;
                    self.close(ctx)?;
                    return self.settings_strings(ctx, &["No changes made."]);
                }
                self.open_pin(ctx)
            }
            Status::Pending => {
                if confirm {
                    self.unlock(ctx)
                } else {
                    self.cancel(
                        ctx,
                        &[
                            "The PIN has been cancelled",
                            "and will NOT be set.",
                            "",
                            "You still do not have a Bank",
                            "PIN.",
                        ],
                    )
                }
            }
            Status::Active => {
                if confirm {
                    if self.0.read().unlocked {
                        self.cancel(
                            ctx,
                            &[
                                "Your Bank PIN has now been",
                                "deleted.",
                                "",
                                "This means that there is no",
                                "PIN protection on your bank",
                                "account.",
                            ],
                        )
                    } else {
                        self.0.write().deleting = true;
                        self.open_pin(ctx)
                    }
                } else {
                    self.settings_strings(ctx, &["No changes made."])
                }
            }
        }
    }
    fn unlock(&self, ctx: &mut impl Context) -> Result<()> {
        self.0.write().unlocked = true;
        let id = self.0.read().open_id;
        self.open_for_id(ctx, id)
    }
    pub fn toggle_confirm(&self, ctx: &mut impl Context, show: bool) -> Result<()> {
        for i in 60..66 {
            self.config(ctx, 14, i, show)?;
        }
        self.audio(ctx, PENDING_AUDIO)?;
        for i in [89, 91, 87] {
            self.config(ctx, 14, i, !show)?;
        }
        if show && self.0.read().status == Some(Status::NoPin) {
            self.draw_string(
                ctx,
                "Do you really wish to set a PIN on your bank account?",
                73,
            )?;
            self.draw_string(
                ctx,
                "Yes, I really want a Bank PIN. I will never forget it!",
                89,
            )?;
            self.draw_string(ctx, "No, I might forget it!", 91)?;
        } else if show && self.0.read().status == Some(Status::Active) {
            self.draw_string(ctx, "Do you really wish to delete your Bank PIN?", 73)?;
            self.draw_string(ctx, "Yes, I don't need a PIN anymore.", 89)?;
            self.draw_string(ctx, "No thanks, I'd rather keep the extra security.", 91)?;
        }
        Ok(())
    }
    pub fn update_temp(&self, ctx: &mut impl Context, button: i32) -> Result<()> {
        // Java evaluates the old string before the List.get RHS.
        let mut pin = self
            .0
            .read()
            .temp_pin
            .clone()
            .unwrap_or_else(|| text("null"));
        let value = *self
            .0
            .read()
            .digits
            .get(usize::try_from(button).map_err(|_| Error::Index)?)
            .ok_or(Error::Index)?;
        pin.extend(text(&value.to_string()));
        self.0.write().temp_pin = Some(pin);
        self.handle_stage(ctx, self.stage())
    }
    pub fn open_pin(&self, ctx: &mut impl Context) -> Result<()> {
        if self.blocked(ctx)? {
            return self.try_dialogue(ctx);
        }
        self.0.write().temp_pin = Some(vec![]);
        let dispatch = ctx.dispatch(self.player().as_ref())?;
        let settings = ctx.settings()?;
        let name = ctx.world_name(settings)?.unwrap_or_else(|| text("null"));
        let mut heading = text("Bank of ");
        heading.extend(name);
        ctx.dispatch_string(dispatch, &heading, 13, 31)?;
        if !self.has_pin() {
            self.config(ctx, 13, 29, true)?;
            self.string(
                ctx,
                &text("Please choose a new FOUR DIGIT PIN using the buttons below."),
                13,
                28,
            )?;
        } else {
            self.string(
                ctx,
                &text("Please enter your PIN using the buttons below."),
                13,
                28,
            )?;
        }
        let interfaces = ctx.interfaces(self.player().as_ref())?;
        let component = ctx.component(13)?;
        let component = ctx.close_event(component, CloseEvent::new(self.clone()))?;
        ctx.open(interfaces, component)?;
        self.handle_stage(ctx, self.stage())
    }
    pub fn handle_stage(&self, ctx: &mut impl Context, stage: i32) -> Result<()> {
        if stage == 4 {
            if !self.has_pin() || self.0.read().change_state == 2 {
                self.audio(ctx, PENDING_AUDIO)?;
                self.string(ctx, &text("Now please enter that number again!"), 13, 28)?;
                self.shuffle_digits(ctx)?;
                return Ok(());
            }
            let pin = self.0.read().temp_pin.clone().ok_or(Error::Null)?;
            if Some(pin) != self.0.read().pin {
                self.audio(ctx, WRONG_AUDIO)?;
                self.close(ctx)?;
                ctx.message(
                    self.player().as_ref(),
                    &text("The PIN you entered is incorrect."),
                )?;
                let tries = self.0.read().tries.wrapping_add(1);
                if self.set_tries(tries) >= 2 {
                    self.try_lock(ctx)?;
                }
                return Ok(());
            }
            if self.0.read().change_state == 1 {
                self.0.write().temp_pin = Some(vec![]);
                self.string(
                    ctx,
                    &text("Please choose a new FOUR DIGIT PIN using the buttons below."),
                    13,
                    28,
                )?;
                self.0.write().change_state = 2;
                self.shuffle_digits(ctx)?;
                return Ok(());
            }
            if self.0.read().deleting {
                return self.cancel(
                    ctx,
                    &[
                        "Your Bank PIN has now been",
                        "deleted.",
                        "",
                        "This means that there is no",
                        "PIN protection on your bank",
                        "account.",
                    ],
                );
            }
            self.unlock(ctx)?;
            self.audio(ctx, SUCCESS_AUDIO)?;
            ctx.message(
                self.player().as_ref(),
                &text("You have correctly entered your PIN."),
            )?;
            return Ok(());
        } else if stage == 8 {
            let success = self.check_temp()?;
            if self.easy_guess()? {
                return self.settings_strings(
                    ctx,
                    &[
                        "That number wouldn't be very",
                        "hard to guess. Please try",
                        "something different!",
                    ],
                );
            }
            if self.0.read().change_state == 2 && success {
                let pin = self.0.read().pin.clone().ok_or(Error::Null)?;
                let current = self.first_four()?;
                if pin == current {
                    let dispatch = ctx.dispatch(self.player().as_ref())?;
                    ctx.dispatch_message(
                        dispatch,
                        &text("Your current PIN matches your new one."),
                    )?;
                } else {
                    self.0.write().pin = Some(current);
                    let dispatch = ctx.dispatch(self.player().as_ref())?;
                    ctx.dispatch_message(dispatch, &text("You have changed your PIN."))?;
                }
                return self.open_settings(ctx);
            }
            if success {
                self.set_new_pin(ctx)?;
            }
            return if success {
                self.settings_strings(
                    ctx,
                    &[
                        "You have requested that a",
                        "PIN be set on your bank",
                        "account. This will take effect",
                        &format!("in another {} days.", self.recovery_delay()),
                        "",
                        "If you wish to cancel this PIN,",
                        "please use the button",
                        "on the left.",
                    ],
                )
            } else {
                self.settings_strings(
                    ctx,
                    &[
                        "Those numbers did not",
                        "match.",
                        "",
                        "Your PIN has not been set;",
                        "please try again if you wish",
                        "to set a new PIN.",
                    ],
                )
            };
        }
        self.shuffle_digits(ctx)?;
        if stage != 0 {
            self.audio(ctx, BEEP_AUDIO)?;
        }
        Ok(())
    }
    fn first_four(&self) -> Result<Text> {
        let f = self.0.read();
        let pin = f.temp_pin.as_ref().ok_or(Error::Null)?;
        Ok(pin.get(..4).ok_or(Error::Index)?.to_vec())
    }
    fn copy_four(&self) -> Result<Text> {
        let f = self.0.read();
        let pin = f.temp_pin.as_ref().ok_or(Error::Null)?;
        let mut out = vec![0; 4];
        for (dst, src) in out.iter_mut().zip(pin) {
            *dst = *src;
        }
        Ok(out)
    }
    fn check_temp(&self) -> Result<bool> {
        let f = self.0.read();
        let pin = f.temp_pin.as_ref().ok_or(Error::Null)?;
        for i in 0..4 {
            if pin.get(i).ok_or(Error::Index)? != pin.get(4 + i).ok_or(Error::Index)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
    fn easy_guess(&self) -> Result<bool> {
        let f = self.0.read();
        let pin = f.temp_pin.as_ref().ok_or(Error::Null)?;
        let mut bad = true;
        for i in 0..4 {
            if pin.first().ok_or(Error::Index)? != pin.get(i).ok_or(Error::Index)? {
                bad = false;
                break;
            }
        }
        if !bad {
            bad = true;
            for i in 0..3 {
                let value = i32::from(*pin.get(i).ok_or(Error::Index)? as i8);
                let next = i32::from(*pin.get(i + 1).ok_or(Error::Index)? as i8);
                if !(i32::from(value.wrapping_add(1) as u16) == next
                    || i32::from(value.wrapping_sub(1) as u16) == next)
                {
                    bad = false;
                    break;
                }
            }
        }
        Ok(bad)
    }
    fn try_lock(&self, ctx: &mut impl Context) -> Result<()> {
        let now = ctx.now()?;
        let delay = now.wrapping_add(i64::from(self.pin_seconds().wrapping_mul(1000)));
        self.0.write().try_delay = delay;
        self.try_dialogue(ctx)
    }
    fn try_dialogue(&self, ctx: &mut impl Context) -> Result<()> {
        let delay = self.0.read().try_delay;
        let mut ticks = delay.wrapping_sub(ctx.now()?) / 1000;
        let original = self.pin_seconds();
        let mut suffix = if original > 15 { "minutes" } else { "seconds" };
        self.close(ctx)?;
        if suffix == "minutes" || ticks > 15 {
            ticks /= 100;
            suffix = "minutes";
        }
        let message = if suffix == "minutes" && ticks <= 1 {
            "You will be able to access your bank in less than 1 minute.".into()
        } else {
            format!("You will be able to access your bank pin in {ticks} {suffix}.")
        };
        let dialogue = ctx.dialogue(self.player().as_ref())?;
        ctx.send_dialogue(dialogue, &text(&message))
    }
    fn shuffle_digits(&self, ctx: &mut impl Context) -> Result<()> {
        let mut digits = self.0.read().digits.clone();
        ctx.shuffle(&mut digits)?;
        self.0.write().digits = digits;
        let mut bit_value = 0i32;
        for i in 0..8 {
            let digit = *self.0.read().digits.get(i).ok_or(Error::Index)?;
            bit_value |= digit.wrapping_shl((i * 4) as u32);
        }
        let mut stage = self.stage();
        if stage >= 4 {
            stage = stage.wrapping_sub(4);
        }
        ctx.varp(self.player().as_ref(), 562, bit_value)?;
        let value = {
            let f = self.0.read();
            let digits = &f.digits;
            *digits.get(8).ok_or(Error::Index)?
                | digits.get(9).ok_or(Error::Index)?.wrapping_shl(4)
                | stage.wrapping_shl(26)
        };
        ctx.varp(self.player().as_ref(), 563, value)?;
        for i in 0i32..9 {
            let child = (if i > 2 { i + 1 } else { i }) + 11;
            let x = (37 + (i % 3) * 95).wrapping_add(ctx.random(2, 45)?);
            let y = (157 + (i / 3) * 70).wrapping_sub(ctx.random(3, 48)?);
            ctx.reposition(self.player().as_ref(), 13, child, x, y)?;
        }
        let x = 308i32.wrapping_add(ctx.random(2, 45)?);
        let y = 155i32.wrapping_sub(ctx.random(3, 45)?);
        ctx.reposition(self.player().as_ref(), 13, 14, x, y)
    }
    fn set_new_pin(&self, ctx: &mut impl Context) -> Result<()> {
        self.0.write().status = Some(Status::Pending);
        let now = ctx.now()?;
        let settings = ctx.settings()?;
        let dev = ctx.dev_mode(settings)?;
        let delay = if dev {
            30_000
        } else {
            i64::from(self.recovery_delay()) * 86_400_000
        };
        self.0.write().pending_delay = now.wrapping_add(delay);
        let pin = self.copy_four()?;
        self.0.write().pin = Some(pin);
        Ok(())
    }
    pub fn do_cancel(&self) {
        let mut f = self.0.write();
        f.status = Some(Status::NoPin);
        f.pending_delay = -1;
        f.pin = None;
        f.unlocked = false;
    }
    pub fn cancel(&self, ctx: &mut impl Context, messages: &[&str]) -> Result<()> {
        self.do_cancel();
        self.audio(ctx, CANCEL_AUDIO)?;
        self.settings_strings(ctx, messages)
    }
    pub fn cancel_messages(
        &self,
        ctx: &mut impl Context,
        messages: Option<&[Option<Text>]>,
    ) -> Result<()> {
        self.do_cancel();
        self.audio(ctx, CANCEL_AUDIO)?;
        self.open_settings_messages(ctx, messages)
    }
    fn settings_strings(&self, ctx: &mut impl Context, messages: &[&str]) -> Result<()> {
        let messages: Vec<_> = messages.iter().map(|s| Some(text(s))).collect();
        self.open_settings_messages(ctx, Some(&messages))
    }
    pub fn draw_login(&self, ctx: &mut impl Context) -> Result<()> {
        let dispatch = ctx.dispatch(self.player().as_ref())?;
        let status = self.0.read().status;
        let message = self.login_message(ctx, status)?;
        ctx.dispatch_string(dispatch, &message, 378, 62)?;
        self.check_pending(ctx)
    }
    pub fn switch_recovery(&self, ctx: &mut impl Context) -> Result<()> {
        let long = !self.0.read().long_recovery;
        self.0.write().long_recovery = long;
        self.draw_string(ctx, &format!("{} days", self.recovery_delay()), 71)?;
        self.draw_string(ctx, "Your recovery delay has", 42)?;
        self.draw_string(
            ctx,
            &format!("now been set to {} days.", self.recovery_delay()),
            43,
        )?;
        for (child, s) in [
            (44, ""),
            (45, "You would have to wait this"),
            (46, "long to delete your PIN if"),
            (47, "you forgot it. But you"),
            (48, "haven't got one..."),
        ] {
            self.draw_string(ctx, s, child)?;
        }
        Ok(())
    }
    pub fn messages(
        &self,
        ctx: &mut impl Context,
        status: Option<Status>,
    ) -> Result<Vec<Option<Text>>> {
        if status.ok_or(Error::Null)? == Status::Pending {
            let days = self.pending_days(ctx)?;
            let plural = if days > 1 { "s" } else { "" };
            Ok([
                "You have requested that a".into(),
                "PIN be set on your bank".into(),
                "account. This will take effect".into(),
                format!("in another {days} day{plural}."),
                "".into(),
                "If you wish to cancel this".into(),
                "PIN, please use the button".into(),
                "on the left.".into(),
            ]
            .iter()
            .map(|s| Some(text(s)))
            .collect())
        } else {
            Ok(SETTINGS.iter().map(|s| s.map(text)).collect())
        }
    }
    pub fn login_message(&self, ctx: &mut impl Context, status: Option<Status>) -> Result<Text> {
        Ok(match status.ok_or(Error::Null)? {
            Status::NoPin => {
                text("You do not have a Bank PIN. Please visit a bank if you would like one.")
            }
            Status::Active => text("You you currently have a Bank PIN."),
            Status::Pending => {
                let days = self.pending_days(ctx)?;
                if days < 1 {
                    text("Your Bank PIN has just become active.")
                } else {
                    text(&format!(
                        "Your Bank PIN will become active in another {days} day{}.",
                        if days > 1 { "s" } else { "" }
                    ))
                }
            }
        })
    }
    pub fn draw_status(&self, ctx: &mut impl Context, status: Option<Status>) -> Result<()> {
        let (label, lines): (&str, &[i32]) = match status.ok_or(Error::Null)? {
            Status::NoPin => ("NO PIN set", &[89, 55, 56, 57, 91, 64, 65, 63, 62]),
            Status::Pending => ("PIN coming soon", &[89, 55, 56, 57, 91, 64, 63, 62, 60, 61]),
            Status::Active => (
                "You have a PIN",
                &[89, 55, 56, 57, 91, CANCEL_PENDING, CHANGE_RECOVERY, SET_PIN],
            ),
        };
        self.draw_string(ctx, label, 69)?;
        for line in lines {
            self.config(ctx, 14, *line, true)?;
        }
        Ok(())
    }
}
