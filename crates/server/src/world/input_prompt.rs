//! Original RunScript.processInput, input packet finally, ContentAPI prompts and
//! BankUtils transfer closure. Attribute, dialogue and callback receivers remain
//! explicit services; none is replaced with a successful no-op.
use super::{
    component_objects::{Captures, NativeObject, Plugin},
    player_walk::NodeHandle,
};
pub type Text = Vec<u16>;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputType {
    Amount,
    Numeric,
    Short,
    Long,
    Message,
}
impl std::fmt::Display for InputType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Amount => "AMOUNT",
            Self::Numeric => "NUMERIC",
            Self::Short => "STRING_SHORT",
            Self::Long => "STRING_LONG",
            Self::Message => "MESSAGE",
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Response {
    Int(i32),
    Text(Text),
}
impl Value for Response {
    fn integer(&self) -> Option<i32> {
        if let Self::Int(n) = self {
            Some(*n)
        } else {
            None
        }
    }
    fn from_integer(value: i32) -> Self {
        Self::Int(value)
    }
    fn from_text(value: Text) -> Self {
        Self::Text(value)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Null,
    Type,
    Number,
    Binding(String),
}
pub type Result<T> = std::result::Result<T, Error>;
pub trait Value: Clone {
    fn integer(&self) -> Option<i32>;
    fn from_integer(value: i32) -> Self;
    fn from_text(value: Text) -> Self;
}
#[derive(Clone)]
pub struct Transfer {
    pub player: NodeHandle,
    pub slot: i32,
    pub withdraw: bool,
    pub after: Option<Plugin>,
}
impl NativeObject for Transfer {
    fn captures(&self) -> Captures {
        Captures {
            nodes: vec![self.player.clone()],
            objects: self.after.clone().into_iter().collect(),
            ..Default::default()
        }
    }
}
impl crate::player::attributes::AttributeObject for Transfer {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn java_string(&self) -> String {
        // Fresh original BankUtils lambda observation with the pinned POM's
        // kotlin-reflect1.5.20 and stdlib/compiler1.8.20, not fallback reflection.
        "(kotlin.Any) -> kotlin.Unit".into()
    }
    fn function_arity(&self) -> Option<u8> {
        Some(1)
    }
    fn captures(&self) -> Captures {
        NativeObject::captures(self)
    }
}
pub trait Context {
    type Value: Value;
    type Script;
    type Interpreter;
    type Bank;
    fn script(&mut self, p: &NodeHandle) -> Result<Option<Self::Script>>;
    fn interaction_locked(&mut self, p: &NodeHandle) -> Result<bool>;
    fn input_type(&mut self, p: &NodeHandle) -> Result<InputType>;
    fn parse_amount(&mut self, p: &NodeHandle) -> Result<bool>;
    fn string(&mut self, value: &Self::Value) -> Result<Text>;
    fn invoke(&mut self, script: Self::Script, value: Self::Value) -> Result<()>;
    fn dialogue(&mut self, p: &NodeHandle, text: Text) -> Result<()>;
    fn remove(&mut self, p: &NodeHandle, key: &str) -> Result<()>;
    fn set_parse_amount(&mut self, p: &NodeHandle, value: bool) -> Result<()>;
    fn interpreter(&mut self, p: &NodeHandle) -> Result<Self::Interpreter>;
    fn send_input(&mut self, d: Self::Interpreter, string: bool, prompt: Text) -> Result<()>;
    fn send_long(&mut self, d: Self::Interpreter, prompt: Text) -> Result<()>;
    fn send_message(&mut self, d: Self::Interpreter, prompt: Text) -> Result<()>;
    fn set_script(&mut self, p: &NodeHandle, script: Self::Script) -> Result<()>;
    fn set_type(&mut self, p: &NodeHandle, kind: InputType) -> Result<()>;
    fn make_transfer(&mut self, transfer: Transfer) -> Result<Self::Script>;
    fn bank(&mut self, p: &NodeHandle) -> Result<Self::Bank>;
    fn take(&mut self, bank: Self::Bank, slot: i32, amount: i32) -> Result<()>;
    fn deposit(&mut self, bank: Self::Bank, slot: i32, amount: i32) -> Result<()>;
    fn last_amount(&mut self, bank: Self::Bank, amount: i32) -> Result<()>;
    fn after(&mut self, callback: Plugin) -> Result<()>;
}
fn text(s: &str) -> Text {
    s.encode_utf16().collect()
}
fn number(value: &[u16]) -> Option<i32> {
    String::from_utf16(value)
        .ok()
        .and_then(|s| rs09_game_core::java_string::parse_i32(&s))
}
fn expanded_amount(value: &[u16]) -> Option<Text> {
    // Unpaired surrogates cannot match the reference's ASCII regex. Lowercase
    // valid Unicode before matching: the Kelvin sign can become suffix k.
    let s = String::from_utf16(value).ok()?;
    let lower = rs09_game_core::java_lower::lowercase_root(&s);
    let digits = lower.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let suffix = &lower.as_bytes()[digits..];
    if !suffix.is_empty() && !suffix.iter().all(|b| *b == suffix[0]) {
        return None;
    }
    let zeros = match suffix.first() {
        None => 0,
        Some(b'k') => 3,
        Some(b'm') => 6,
        _ => return None,
    };
    let mut out: Text = lower.as_bytes()[..digits]
        .iter()
        .map(|b| u16::from(*b))
        .collect();
    out.extend(std::iter::repeat_n(u16::from(b'0'), suffix.len() * zeros));
    Some(out)
}
pub fn process<C: Context>(
    ctx: &mut C,
    p: &NodeHandle,
    value: C::Value,
    script: C::Script,
) -> Result<()> {
    if value.integer().is_some_and(|v| v <= 0) {
        return Ok(());
    }
    let kind = ctx.input_type(p)?;
    let mut input = value;
    if ctx.parse_amount(p)? {
        let string = ctx.string(&input)?;
        let Some(expanded) = expanded_amount(&string) else {
            ctx.dialogue(p, text("That doesn't look right. Please try again."))?;
            return Ok(());
        };
        input = C::Value::from_text(expanded);
    }
    if matches!(kind, InputType::Numeric | InputType::Amount)
        && let Some(value) = number(&ctx.string(&input)?)
    {
        input = C::Value::from_integer(value);
    }
    let outcome = match ctx.invoke(script, input) {
        Err(Error::Number) => ctx.dialogue(p, text("That number's a bit large, don't you think?")),
        Err(Error::Type) => ctx.dialogue(p, text("Something went wrong here. Try again.")),
        result => result,
    };
    ctx.remove(p, "runscript")?;
    ctx.remove(p, "input-type")?;
    outcome
}
pub fn receive<C: Context>(ctx: &mut C, p: &NodeHandle, value: C::Value) -> Result<()> {
    let Some(script) = ctx.script(p)? else {
        return Ok(());
    };
    if ctx.interaction_locked(p)? {
        return Ok(());
    }
    let outcome = process(ctx, p, value, script);
    ctx.remove(p, "parseamount")?;
    ctx.remove(p, "runscript")?;
    outcome
}
pub fn send<C: Context>(
    ctx: &mut C,
    p: &NodeHandle,
    kind: InputType,
    prompt: Text,
    script: C::Script,
) -> Result<()> {
    match kind {
        InputType::Amount => {
            ctx.set_parse_amount(p, true)?;
            let interpreter = ctx.interpreter(p)?;
            ctx.send_input(interpreter, true, prompt)?;
        }
        InputType::Numeric | InputType::Short => {
            let interpreter = ctx.interpreter(p)?;
            ctx.send_input(interpreter, kind != InputType::Numeric, prompt)?;
        }
        InputType::Long => {
            let interpreter = ctx.interpreter(p)?;
            ctx.send_long(interpreter, prompt)?;
        }
        InputType::Message => {
            let interpreter = ctx.interpreter(p)?;
            ctx.send_message(interpreter, prompt)?;
        }
    }
    ctx.set_script(p, script)?;
    ctx.set_type(p, kind)
}
pub fn transfer_x<C: Context>(
    ctx: &mut C,
    p: &NodeHandle,
    slot: i32,
    withdraw: bool,
    after: Option<Plugin>,
) -> Result<()> {
    let script = ctx.make_transfer(Transfer {
        player: p.clone(),
        slot,
        withdraw,
        after,
    })?;
    send(ctx, p, InputType::Amount, text("Enter the amount:"), script)
}
pub fn invoke_transfer<C: Context>(
    ctx: &mut C,
    transfer: &Transfer,
    value: &C::Value,
) -> Result<()> {
    let amount = number(&ctx.string(value)?).ok_or(Error::Number)?;
    let bank = ctx.bank(&transfer.player)?;
    if transfer.withdraw {
        ctx.take(bank, transfer.slot, amount)?;
    } else {
        ctx.deposit(bank, transfer.slot, amount)?;
    }
    let bank = ctx.bank(&transfer.player)?;
    ctx.last_amount(bank, amount)?;
    if let Some(after) = transfer.after.clone() {
        ctx.after(after)?;
    }
    Ok(())
}
