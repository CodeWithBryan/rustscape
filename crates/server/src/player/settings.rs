//! Original player Settings state and operations. Player properties, containers,
//! item configuration and packet dispatch are explicit owning-world services.
use crate::definitions::item::Shared;
use rs09_game_core::{java_double, java_json::Json, java_string::parse_i32};
use rs09_protocol::game::Frame;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Null,
    Type,
    Number,
    Index,
    Binding,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Copy, Debug)]
pub enum ContainerKind {
    Inventory,
    Equipment,
}
/// Opaque container/item identities are resolved by the same owning player.
/// Callbacks may reenter Settings; no field guard is held across these calls.
pub trait Context {
    fn varp(&mut self, state: &Settings, id: i32, value: i32) -> Result<()>;
    fn session(&mut self, state: &Settings) -> Result<Option<usize>>;
    fn send(&mut self, state: &Settings, session: Option<usize>, frame: Frame) -> Result<()>;
    fn dispatcher(&mut self, state: &Settings) -> Result<Option<usize>>;
    fn run_energy(&mut self, state: &Settings, dispatcher: Option<usize>) -> Result<()>;
    fn message(&mut self, state: &Settings, dispatcher: Option<usize>, text: &str) -> Result<()>;
    fn string(
        &mut self,
        state: &Settings,
        dispatcher: Option<usize>,
        text: String,
        interface: i32,
        child: i32,
    ) -> Result<()>;
    fn properties(&mut self, state: &Settings) -> Result<Option<usize>>;
    fn retaliating(&mut self, state: &Settings, properties: Option<usize>) -> Result<bool>;
    fn set_retaliating(
        &mut self,
        state: &Settings,
        properties: Option<usize>,
        value: bool,
    ) -> Result<()>;
    fn ironman(&mut self, state: &Settings) -> Result<bool>;
    fn container(&mut self, state: &Settings, kind: ContainerKind) -> Result<usize>;
    fn item(&mut self, state: &Settings, container: usize, slot: i32) -> Result<Option<usize>>;
    fn weight(&mut self, state: &Settings, item: usize) -> Result<f64>;
}
#[derive(Clone, Debug)]
pub struct Fields {
    pub run_energy: f64,
    pub weight: f64,
    pub brightness: i32,
    pub music_volume: i32,
    pub sound_effect_volume: i32,
    pub area_sound_volume: i32,
    pub single_mouse_button: bool,
    pub disable_chat_effects: bool,
    pub split_private_chat: bool,
    pub accept_aid: bool,
    pub run_toggled: bool,
    pub public_chat_setting: i32,
    pub private_chat_setting: i32,
    pub clan_chat_setting: i32,
    pub trade_setting: i32,
    pub assist_setting: i32,
    pub special_toggled: bool,
    pub special_energy: i32,
    pub attack_style_index: i32,
}
impl Default for Fields {
    fn default() -> Self {
        Self {
            run_energy: 100.0,
            weight: 0.0,
            brightness: 2,
            music_volume: 0,
            sound_effect_volume: 0,
            area_sound_volume: 0,
            single_mouse_button: false,
            disable_chat_effects: false,
            split_private_chat: false,
            accept_aid: false,
            run_toggled: false,
            public_chat_setting: 0,
            private_chat_setting: 0,
            clan_chat_setting: 0,
            trade_setting: 0,
            assist_setting: 0,
            special_toggled: false,
            special_energy: 100,
            attack_style_index: 0,
        }
    }
}
#[derive(Clone)]
pub struct Settings {
    pub fields: Shared<Fields>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            fields: Shared::new(Fields::default()),
        }
    }
}
macro_rules! int_setter {
    ($name:ident, $field:ident, $varp:literal) => {
        pub fn $name(&self, ctx: &mut impl Context, value: i32) -> Result<()> {
            self.fields.write().$field = value;
            let value = self.fields.read().$field;
            ctx.varp(self, $varp, value)
        }
    };
}
macro_rules! flag_toggle {
    ($name:ident, $field:ident, $varp:literal) => {
        pub fn $name(&self, ctx: &mut impl Context) -> Result<()> {
            let value = !self.fields.read().$field;
            self.fields.write().$field = value;
            let value = i32::from(self.fields.read().$field);
            ctx.varp(self, $varp, value)
        }
    };
}
impl Settings {
    pub fn update(&self, ctx: &mut impl Context) -> Result<()> {
        let value = self.fields.read().brightness.wrapping_add(1);
        ctx.varp(self, 166, value)?;
        let value = self.fields.read().music_volume;
        ctx.varp(self, 168, value)?;
        let value = self.fields.read().sound_effect_volume;
        ctx.varp(self, 169, value)?;
        let value = self.fields.read().area_sound_volume;
        ctx.varp(self, 872, value)?;
        let value = i32::from(self.fields.read().single_mouse_button);
        ctx.varp(self, 170, value)?;
        let value = i32::from(self.fields.read().disable_chat_effects);
        ctx.varp(self, 171, value)?;
        let value = i32::from(self.fields.read().split_private_chat);
        ctx.varp(self, 287, value)?;
        let value = i32::from(self.fields.read().accept_aid);
        ctx.varp(self, 427, value)?;
        let properties = ctx.properties(self)?;
        let value = i32::from(!ctx.retaliating(self, properties)?);
        ctx.varp(self, 172, value)?;
        let value = i32::from(self.fields.read().run_toggled);
        ctx.varp(self, 173, value)?;
        let value = self.fields.read().clan_chat_setting;
        ctx.varp(self, 1054, value)?;
        let value = self.fields.read().assist_setting;
        ctx.varp(self, 1055, value)?;
        let value = self.fields.read().special_energy.wrapping_mul(10);
        ctx.varp(self, 300, value)?;
        let value = self.fields.read().attack_style_index;
        ctx.varp(self, 43, value)?;
        let dispatcher = ctx.dispatcher(self)?;
        ctx.run_energy(self, dispatcher)?;
        self.update_chat_settings(ctx)
    }
    pub fn update_chat_settings(&self, ctx: &mut impl Context) -> Result<()> {
        let session = ctx.session(self)?;
        let f = self.fields.read();
        let frame = Frame {
            opcode: 232,
            payload: vec![
                f.public_chat_setting as u8,
                f.private_chat_setting as u8,
                f.trade_setting as u8,
            ],
        };
        drop(f);
        ctx.send(self, session, frame)
    }
    /// The original overload ignores its private-chat argument.
    pub fn update_chat_values(
        &self,
        ctx: &mut impl Context,
        public: i32,
        _private: i32,
        trade: i32,
    ) -> Result<()> {
        let mut update = false;
        if self.fields.read().public_chat_setting != public {
            self.fields.write().public_chat_setting = public;
            update = true;
        }
        if self.fields.read().trade_setting != trade {
            self.fields.write().trade_setting = trade;
            update = true;
        }
        if update {
            self.update_chat_settings(ctx)?;
        }
        Ok(())
    }
    pub fn set_chat_settings(&self, public: i32, private: i32, trade: i32) {
        self.fields.write().public_chat_setting = public;
        self.fields.write().private_chat_setting = private;
        self.fields.write().trade_setting = trade;
    }
    pub fn parse(&self, ctx: &mut impl Context, data: &Json) -> Result<()> {
        fn field<'a>(data: &'a Json, key: &str) -> Result<&'a Json> {
            match data {
                Json::Null => Err(Error::Null),
                Json::Object(_) => match data.get(key) {
                    None | Some(Json::Null) => Err(Error::Null),
                    Some(v) => Ok(v),
                },
                _ => Err(Error::Type),
            }
        }
        fn integer(data: &Json, key: &str) -> Result<i32> {
            parse_i32(&field(data, key)?.text()).ok_or(Error::Number)
        }
        fn boolean(data: &Json, key: &str) -> Result<bool> {
            match field(data, key)? {
                Json::Bool(v) => Ok(*v),
                _ => Err(Error::Type),
            }
        }
        self.fields.write().brightness = integer(data, "brightness")?;
        self.fields.write().music_volume = integer(data, "musicVolume")?;
        self.fields.write().sound_effect_volume = integer(data, "soundEffectVolume")?;
        self.fields.write().area_sound_volume = integer(data, "areaSoundVolume")?;
        self.fields.write().single_mouse_button = boolean(data, "singleMouse")?;
        self.fields.write().disable_chat_effects = boolean(data, "disableChatEffects")?;
        self.fields.write().split_private_chat = boolean(data, "splitPrivate")?;
        self.fields.write().accept_aid = boolean(data, "acceptAid")?;
        self.fields.write().run_toggled = boolean(data, "runToggled")?;
        self.fields.write().public_chat_setting = integer(data, "publicChatSetting")?;
        self.fields.write().private_chat_setting = integer(data, "privateChatSetting")?;
        self.fields.write().clan_chat_setting = integer(data, "clanChatSetting")?;
        self.fields.write().trade_setting = integer(data, "tradeSetting")?;
        self.fields.write().assist_setting = integer(data, "assistSetting")?;
        let energy = java_double::parse(&field(data, "runEnergy")?.text()).ok_or(Error::Number)?;
        self.fields.write().run_energy = energy;
        self.fields.write().special_energy = integer(data, "specialEnergy")?;
        self.fields.write().attack_style_index = integer(data, "attackStyle")?;
        let properties = ctx.properties(self)?;
        let value = boolean(data, "retaliation")?;
        ctx.set_retaliating(self, properties, value)
    }
    pub fn toggle_special_bar(&self, ctx: &mut impl Context) -> Result<()> {
        let value = !self.fields.read().special_toggled;
        self.set_special_toggled(ctx, value)
    }
    /// Preserves the original toggle despite its setter name/unused argument.
    pub fn set_special_toggled(&self, ctx: &mut impl Context, _enabled: bool) -> Result<()> {
        let value = !self.fields.read().special_toggled;
        self.fields.write().special_toggled = value;
        let value = i32::from(self.fields.read().special_toggled);
        ctx.varp(self, 301, value)
    }
    pub fn drain_special(&self, ctx: &mut impl Context, amount: i32) -> Result<bool> {
        if !self.fields.read().special_toggled {
            return Ok(false);
        }
        self.set_special_toggled(ctx, false)?;
        if amount > self.fields.read().special_energy {
            let dispatcher = ctx.dispatcher(self)?;
            ctx.message(
                self,
                dispatcher,
                "You do not have enough special attack energy left.",
            )?;
            return Ok(false);
        }
        let value = self.fields.read().special_energy.wrapping_sub(amount);
        self.set_special_energy(ctx, value)?;
        Ok(true)
    }
    pub fn set_special_energy(&self, ctx: &mut impl Context, value: i32) -> Result<()> {
        self.fields.write().special_energy = value;
        let value = self.fields.read().special_energy.wrapping_mul(10);
        ctx.varp(self, 300, value)
    }
    pub fn toggle_retaliating(&self, ctx: &mut impl Context) -> Result<()> {
        let destination = ctx.properties(self)?;
        let properties = ctx.properties(self)?;
        let value = !ctx.retaliating(self, properties)?;
        ctx.set_retaliating(self, destination, value)?;
        let properties = ctx.properties(self)?;
        let value = i32::from(!ctx.retaliating(self, properties)?);
        ctx.varp(self, 172, value)
    }
    flag_toggle!(toggle_mouse_button, single_mouse_button, 170);
    flag_toggle!(toggle_chat_effects, disable_chat_effects, 171);
    flag_toggle!(toggle_split_private_chat, split_private_chat, 287);
    flag_toggle!(toggle_accept_aid, accept_aid, 427);
    pub fn toggle_run(&self, ctx: &mut impl Context) -> Result<()> {
        let value = !self.fields.read().run_toggled;
        self.set_run_toggled(ctx, value)
    }
    pub fn set_run_toggled(&self, ctx: &mut impl Context, enabled: bool) -> Result<()> {
        self.fields.write().run_toggled = enabled;
        let value = i32::from(self.fields.read().run_toggled);
        ctx.varp(self, 173, value)
    }
    pub fn update_run_energy(&self, ctx: &mut impl Context, drain: f64) -> Result<()> {
        let value = self.fields.read().run_energy - drain;
        // Fixed finite bounds preserve Java's NaN and signed-zero comparisons.
        self.fields.write().run_energy = value.clamp(0.0, 100.0);
        let dispatcher = ctx.dispatcher(self)?;
        ctx.run_energy(self, dispatcher)
    }
    pub fn update_weight(&self, ctx: &mut impl Context) -> Result<()> {
        self.fields.write().weight = 0.0;
        for slot in 0..28 {
            let container = ctx.container(self, ContainerKind::Inventory)?;
            if let Some(item) = ctx.item(self, container, slot)? {
                let value = ctx.weight(self, item)?;
                if value > 0.0 {
                    self.fields.write().weight += value;
                }
            }
        }
        for slot in 0..11 {
            let container = ctx.container(self, ContainerKind::Equipment)?;
            if let Some(item) = ctx.item(self, container, slot)? {
                // Java compound field assignment captures the LHS before the
                // configuration getter, including reentrant mutations there.
                let weight = self.fields.read().weight;
                let value = ctx.weight(self, item)?;
                self.fields.write().weight = weight + value;
            }
        }
        let dispatcher = ctx.dispatcher(self)?;
        let text = format!("{} kg", self.fields.read().weight as i32);
        ctx.string(self, dispatcher, text, 667, 32)
    }
    pub fn set_brightness(&self, ctx: &mut impl Context, value: i32) -> Result<()> {
        self.fields.write().brightness = value;
        let value = self.fields.read().brightness.wrapping_add(1);
        ctx.varp(self, 166, value)
    }
    int_setter!(set_music_volume, music_volume, 168);
    int_setter!(set_sound_effect_volume, sound_effect_volume, 169);
    int_setter!(set_area_sound_volume, area_sound_volume, 872);
    int_setter!(set_clan_chat_setting, clan_chat_setting, 1054);
    int_setter!(set_assist_setting, assist_setting, 1055);
    int_setter!(toggle_attack_style_index, attack_style_index, 43);
    pub fn set_public_chat_setting(&self, ctx: &mut impl Context, value: i32) -> Result<()> {
        self.fields.write().public_chat_setting = value;
        self.update_chat_settings(ctx)
    }
    pub fn set_private_chat_setting(&self, ctx: &mut impl Context, value: i32) -> Result<()> {
        self.fields.write().private_chat_setting = value;
        self.update_chat_settings(ctx)
    }
    pub fn set_trade_setting(&self, ctx: &mut impl Context, value: i32) -> Result<()> {
        self.fields.write().trade_setting = value;
        self.update_chat_settings(ctx)
    }
    pub fn accept_aid(&self, ctx: &mut impl Context) -> Result<bool> {
        if ctx.ironman(self)? {
            return Ok(false);
        }
        Ok(self.fields.read().accept_aid)
    }
}
