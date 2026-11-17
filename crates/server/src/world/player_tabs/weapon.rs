//! `WeaponInterface`: the weapon tab (tab 0). Opening it, the refresh after an
//! equipment change (`updateInterface`), attack style selection, the special
//! attack bar's visibility, the staff's autocast members and restoring a
//! saved autocast at login.
//!
//! The object's own fields are [`WeaponInterface`]: the component it is (its
//! mutable `id` lives in the shared component handle, which the interface
//! manager's tab array also holds), the current `WeaponInterfaces` constant,
//! the special bar flag and the attack animations. Everything else is a
//! [`Host`] service.
//!
//! The autocast members (`getAutospellId`, `selectAutoSpell`, `canAutocast`)
//! are also ported in `world::spells::autocast` against the spell library's
//! host; this module carries the same text against its own host, so that the
//! weapon tab's calls between its members stay inside one object, as they do
//! in the original. Both are replayed against their own corpora.
use super::{Error, Result, Value, remove_attributes};
use crate::world::animation::Priority;
use crate::world::combat_swing::{AttackStyle, WeaponInterfaces};
use crate::world::interfaces::{self, Type};
use crate::world::spells::SpellBook;
use rs09_game_core::java_string::equals_ignore_case;

pub const MODERN_SPELL_IDS: [i32; 16] =
    [1, 4, 6, 8, 10, 14, 17, 20, 24, 27, 33, 38, 45, 48, 52, 55];
pub const ANCIENT_SPELL_IDS: [i32; 20] = [
    8, 12, 4, 0, 10, 14, 6, 2, 9, 13, 5, 1, 11, 15, 7, 3, 16, 17, 18, 19,
];
pub const SLAYER_STAFF_SPELL_IDS: [i32; 6] = [22, 31, 45, 48, 52, 55];
pub const VOID_STAFF_SPELL_IDS: [i32; 6] = [22, 42, 45, 48, 52, 55];
/// `ATTRIBUTE_AUTOCAST_BUTTON_ID`.
pub const ATTRIBUTE_AUTOCAST_BUTTON_ID: &str = "/save:autocast:buttonid";
/// `ATTRIBUTE_AUTOCAST_STYLE`.
pub const ATTRIBUTE_AUTOCAST_STYLE: &str = "/save:autocast:style";
/// `Components.WEAPON_FISTS_SEL_92`: the component the constructor passes to
/// `super` and whose definition `open()` reads.
pub const WEAPON_FISTS_SEL: i32 = 92;
/// `Components.WEAPON_STAFF_SEL_90`.
pub const WEAPON_STAFF_SEL: i32 = 90;
/// `ItemConfigParser` keys.
pub const WEAPON_INTERFACE: &str = "weapon_interface";
pub const HAS_SPECIAL: &str = "has_special";
pub const ATTACK_ANIMS: &str = "attack_anims";
/// `EquipmentContainer.SLOT_WEAPON`.
pub const SLOT_WEAPON: i32 = 3;
/// `BONUS_RANGE`.
const BONUS_RANGE: i32 = 4;

/// An `Animation` as the weapon tab hands it to `Properties.setAttackAnimation`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Animation {
    pub id: i32,
    pub delay: i32,
    pub priority: Priority,
}

const fn high(id: i32) -> Animation {
    Animation {
        id,
        delay: 0,
        priority: Priority::High,
    }
}

/// `DEFAULT_ANIMS`.
pub const DEFAULT_ANIMS: [Animation; 4] = [high(422), high(423), high(422), high(422)];

/// What `SpellBook.X.getSpell(id)` returned, before the `(CombatSpell)` cast.
pub enum Lookup<S> {
    /// null.
    None,
    /// A `CombatSpell`.
    Combat(S),
    /// A spell that is not a `CombatSpell`: the cast fails.
    Other,
}

/// Every call the weapon tab makes outside its own object. The host is the
/// owning player's: each method acts on that player.
pub trait Host {
    /// The host's handle of a combat spell (the autocast spell).
    type Spell: Clone;
    /// `player.addExtension(WeaponInterface.class, this)` in the constructor.
    fn add_extension(&mut self) -> Result<()>;
    /// `player.getInterfaceManager().getTabs()[slot] = component`.
    fn set_tab(&mut self, slot: usize, component: &interfaces::Handle) -> Result<()>;
    /// `player.getEquipment().get(slot)`: the item id, `None` for null.
    fn equipment(&mut self, slot: i32) -> Result<Option<i32>>;
    /// `player.getEquipment().getNew(slot)`: the item id, 0 for an empty slot.
    fn equipment_new(&mut self, slot: i32) -> Result<i32>;
    /// `item.getName()` / `item.getDefinition().getName()`; `None` is null.
    fn item_name(&mut self, id: i32) -> Option<String>;
    /// `item.getDefinition().getConfiguration(key, default)` for an int.
    fn item_config_int(&mut self, id: i32, key: &str, default: i32) -> Result<i32>;
    /// `item.getDefinition().getConfiguration(key, default)` for a boolean.
    fn item_config_bool(&mut self, id: i32, key: &str, default: bool) -> Result<bool>;
    /// `item.getDefinition().getConfiguration(ATTACK_ANIMS, DEFAULT_ANIMS)`:
    /// the configured array, `None` when the key is absent.
    fn item_config_animations(&mut self, id: i32, key: &str) -> Result<Option<Vec<Animation>>>;
    /// `ComponentDefinition.forId(id)`.
    fn component_definition(&mut self, id: i32) -> interfaces::Def;
    /// `new Component(id)`.
    fn create_component(&mut self, id: i32) -> interfaces::Handle;
    /// `player.getInterfaceManager().isResizable()`.
    fn resizable(&mut self) -> Result<bool>;
    /// The Interface packet: `InterfaceContext(player, pane, child, id, walkable)`.
    fn send_interface(&mut self, pane: i32, child: i32, id: i32, walkable: bool) -> Result<()>;
    /// The InterfaceConfig packet (`PacketRepository.send(InterfaceConfig...)`,
    /// `PacketDispatch.sendInterfaceConfig`, `setComponentVisibility`).
    fn interface_config(&mut self, interface: i32, child: i32, hide: bool) -> Result<()>;
    /// The String packet.
    fn send_string(&mut self, text: &str, interface: i32, child: i32) -> Result<()>;
    /// `player.getPacketDispatch().sendMessage(text)`.
    fn message(&mut self, text: &str) -> Result<()>;
    /// `settings.getAttackStyleIndex()`.
    fn attack_style_index(&mut self) -> Result<i32>;
    /// `settings.toggleAttackStyleIndex(index)`.
    fn toggle_attack_style_index(&mut self, index: i32) -> Result<()>;
    /// `settings.isSpecialToggled()`.
    fn special_toggled(&mut self) -> Result<bool>;
    /// `settings.toggleSpecialBar()`.
    fn toggle_special_bar(&mut self) -> Result<()>;
    /// `properties.getAttackStyle()`.
    fn attack_style(&mut self) -> Result<Option<AttackStyle>>;
    /// `properties.setAttackStyle(style)`.
    fn set_attack_style(&mut self, style: AttackStyle) -> Result<()>;
    /// `properties.setAttackAnimation(animation)`.
    fn set_attack_animation(&mut self, animation: Animation) -> Result<()>;
    /// `properties.getCombatPulse().updateStyle()`.
    fn update_style(&mut self) -> Result<()>;
    /// `player.getAppearance().isNpc()`.
    fn is_npc(&mut self) -> Result<bool>;
    /// `player.getSpellBookManager().getSpellBook()`.
    fn spell_book(&mut self) -> Result<i32>;
    /// `properties.getAutocastSpell()`.
    fn autocast(&mut self) -> Result<Option<Self::Spell>>;
    /// `spell.getSpellId()`.
    fn spell_id(&mut self, spell: &Self::Spell) -> i32;
    /// `properties.setAutocastSpell(spell)`.
    fn set_autocast(&mut self, spell: Option<&Self::Spell>) -> Result<()>;
    /// `SpellBook.<book>.getSpell(id)`.
    fn spell_lookup(&mut self, book: SpellBook, id: i32) -> Result<Lookup<Self::Spell>>;
    /// `entity.getAttribute(key, default)`.
    fn get_attribute(&mut self, key: &str, default: Value) -> Result<Value>;
    /// `entity.setAttribute(key, value)`.
    fn set_attribute(&mut self, key: &str, value: Value) -> Result<()>;
    /// `entity.removeAttribute(key)`.
    fn remove_attribute(&mut self, key: &str) -> Result<()>;
    /// `ContentAPI.setVarp(player, id, value, save)`.
    fn varp(&mut self, id: i32, value: i32, save: bool) -> Result<()>;
    /// `player.getInterfaceManager().openTab(component)`.
    fn open_tab(&mut self, component: &interfaces::Handle) -> Result<()>;
}

/// The `WeaponInterface` object.
pub struct WeaponInterface {
    /// The component this object is: `id` is its field, set to the current
    /// interface's id when the interface changes.
    pub component: interfaces::Handle,
    /// `current`; null until the first `updateInterface`.
    pub current: Option<WeaponInterfaces>,
    /// `specialBar`.
    pub special_bar: bool,
    /// `attackAnimations`; null until the first `updateInterface`.
    pub attack_animations: Option<Vec<Animation>>,
}

/// `WeaponInterface.getWeaponInterface(weapon)`.
pub fn weapon_interface_for<H: Host>(h: &mut H, weapon: Option<i32>) -> Result<WeaponInterfaces> {
    let Some(id) = weapon else {
        return Ok(WeaponInterfaces::ALL[0]);
    };
    let slot = h.item_config_int(id, WEAPON_INTERFACE, 0)?;
    usize::try_from(slot)
        .ok()
        .and_then(|slot| WeaponInterfaces::ALL.get(slot).copied())
        .ok_or(Error::Index)
}

/// `getConfig(buttons, interfaceId)`: the special bar's child.
pub fn special_bar_child(buttons: usize, interface: i32) -> i32 {
    if interface == WEAPON_STAFF_SEL {
        return 87;
    }
    if ![93, 76, 79, 84, 91].contains(&interface) {
        match buttons {
            3 => 13,
            4 => 12,
            _ => 10,
        }
    } else {
        10
    }
}

/// `getAttribute(key, false)` read as a boolean.
fn attribute_bool<H: Host>(h: &mut H, key: &str) -> Result<bool> {
    h.get_attribute(key, Value::Bool(false))?.boolean()
}

/// `getNew(3).getName().equalsIgnoreCase(name)`: one `getNew`.
fn weapon_named<H: Host>(h: &mut H, name: &str) -> Result<bool> {
    let id = h.equipment_new(SLOT_WEAPON)?;
    let item = h.item_name(id).ok_or(Error::Null)?;
    Ok(equals_ignore_case(&item, name))
}

/// The table `getAutospellId` and `selectAutoSpell` pick.
fn table<H: Host>(h: &mut H, modern: bool) -> Result<&'static [i32]> {
    let mut data: &'static [i32] = if modern {
        &MODERN_SPELL_IDS
    } else {
        &ANCIENT_SPELL_IDS
    };
    if modern && weapon_named(h, "Slayer's staff")? {
        data = &SLAYER_STAFF_SPELL_IDS;
    }
    if modern && weapon_named(h, "Void knight mace")? {
        data = &VOID_STAFF_SPELL_IDS;
    }
    Ok(data)
}

fn styles(current: WeaponInterfaces) -> &'static [AttackStyle] {
    current.attack_styles()
}

fn style_at(current: WeaponInterfaces, slot: i32) -> Result<AttackStyle> {
    usize::try_from(slot)
        .ok()
        .and_then(|slot| styles(current).get(slot).copied())
        .ok_or(Error::Index)
}

impl WeaponInterface {
    /// `new WeaponInterface(player)`: `super(92)` is the caller's component
    /// (a new `Component(92)`), then the player's extension.
    pub fn new<H: Host>(h: &mut H, component: interfaces::Handle) -> Result<Self> {
        let me = Self {
            component,
            current: None,
            special_bar: false,
            attack_animations: None,
        };
        h.add_extension()?;
        Ok(me)
    }

    /// The component's current `id` field.
    pub fn id(&self) -> i32 {
        self.component.read().id
    }

    /// `open(Player)`: the argument is not read.
    pub fn open<H: Host>(&mut self, h: &mut H) -> Result<()> {
        self.current = None;
        self.update_interface(h)
    }

    /// The private `open()`.
    fn open_tab<H: Host>(&mut self, h: &mut H) -> Result<()> {
        let definition = h.component_definition(WEAPON_FISTS_SEL);
        let resizable = h.resizable()?;
        let kind = definition.read().kind;
        let pane = kind.ok_or(Error::Null)?.pane(resizable);
        let child = kind.ok_or(Error::Null)?.child(resizable);
        let walkable = definition.read().walkable;
        h.send_interface(pane, child, self.id(), walkable)?;
        let index = h.attack_style_index()?;
        let slot = self.ensure_style_index(h, index)?;
        if slot != h.attack_style_index()? {
            h.toggle_attack_style_index(slot)?;
        }
        let current = self.current.ok_or(Error::Null)?;
        h.set_attack_style(style_at(current, slot)?)?;
        self.check_staff_configs(h, slot)
    }

    /// `ensureStyleIndex(player, slot)`.
    fn ensure_style_index<H: Host>(&mut self, h: &mut H, slot: i32) -> Result<i32> {
        let style = h.attack_style()?;
        let current = self.current.ok_or(Error::Null)?;
        let list = styles(current);
        let length = list.len() as i32;
        if slot >= length {
            let slot = length - 1;
            if let Some(style) = style {
                for i in (0..=slot).rev() {
                    if list[i as usize].style == style.style {
                        return Ok(i);
                    }
                }
            }
            return Ok(slot);
        }
        if let Some(style) = style
            && style_at(current, slot)?.style != style.style
        {
            for i in (0..length).rev() {
                if list[i as usize].style == style.style {
                    return Ok(i);
                }
            }
        }
        Ok(slot)
    }

    /// `updateInterface()`.
    pub fn update_interface<H: Host>(&mut self, h: &mut H) -> Result<()> {
        h.set_tab(0, &self.component)?;
        let weapon = h.equipment(SLOT_WEAPON)?;
        let inter = weapon_interface_for(h, weapon)?;
        let name;
        if let Some(id) = weapon {
            name = h.item_name(id);
            self.special_bar = h.item_config_bool(id, HAS_SPECIAL, false)?;
            self.attack_animations = Some(
                h.item_config_animations(id, ATTACK_ANIMS)?
                    .unwrap_or_else(|| DEFAULT_ANIMS.to_vec()),
            );
        } else {
            name = Some("Unarmed".to_owned());
            self.special_bar = false;
            self.attack_animations = Some(DEFAULT_ANIMS.to_vec());
        }
        if Some(inter) != self.current {
            self.component.write().id = inter.interface_id();
            self.current = Some(inter);
            self.open_tab(h)?;
            h.update_style()?;
        }
        let length = self.attack_animations.as_ref().ok_or(Error::Null)?.len() as i32;
        if h.attack_style_index()? < length && !h.is_npc()? {
            let index = h.attack_style_index()?;
            let animation = self.animation_at(index)?;
            h.set_attack_animation(animation)?;
        }
        let current = self.current.ok_or(Error::Null)?;
        if current != WeaponInterfaces::Staff {
            self.select_auto_spell(h, -1, false, false)?;
            let child = special_bar_child(styles(current).len(), current.interface_id());
            h.interface_config(self.id(), child, !self.special_bar)?;
        } else {
            h.interface_config(self.id(), 87, !self.special_bar)?;
        }
        if !self.can_autocast(h, false)? {
            if self.current == Some(WeaponInterfaces::Staff)
                && attribute_bool(h, "autocast_select")?
            {
                self.open_tab(h)?;
            }
            self.select_auto_spell(h, -1, true, false)?;
        }
        // A null name fails where the String packet writes it (not exercised).
        let name = name.ok_or(Error::Null)?;
        h.send_string(&name, self.id(), 0)?;
        if h.special_toggled()? {
            h.toggle_special_bar()?;
        }
        Ok(())
    }

    fn animation_at(&self, index: i32) -> Result<Animation> {
        let animations = self.attack_animations.as_ref().ok_or(Error::Null)?;
        usize::try_from(index)
            .ok()
            .and_then(|i| animations.get(i).copied())
            .ok_or(Error::Index)
    }

    /// `setAttackStyle(button)`.
    pub fn set_attack_style<H: Host>(&mut self, h: &mut H, button: i32) -> Result<bool> {
        let mut slot = button.wrapping_sub(1);
        let current = self.current;
        if current != Some(WeaponInterfaces::Staff) {
            slot = slot.wrapping_sub(1);
        }
        let ranged = current == Some(WeaponInterfaces::WarhammerMaul) || {
            let c = current.ok_or(Error::Null)?;
            let list = styles(c);
            list.len() > 2 && list[2].bonus_type == BONUS_RANGE && c.interface_id() != 91
        };
        if ranged {
            slot = match button {
                4 => 1,
                3 => 2,
                _ => 0,
            };
        } else if current == Some(WeaponInterfaces::Claws) {
            slot = match button {
                5 => 1,
                3 => 3,
                _ => slot,
            };
        }
        if current == Some(WeaponInterfaces::Axe) {
            if button == 5 {
                slot = 1;
            } else if button == 3 {
                slot = 3;
            }
        }
        if current == Some(WeaponInterfaces::Staff) && slot <= 2 {
            remove_attributes(
                &[ATTRIBUTE_AUTOCAST_BUTTON_ID, ATTRIBUTE_AUTOCAST_STYLE],
                |key| h.remove_attribute(key),
            )?;
        }
        let current = current.ok_or(Error::Null)?;
        if slot < 0 || slot >= styles(current).len() as i32 {
            return Ok(false);
        }
        let style = style_at(current, slot)?;
        h.set_attack_style(style)?;
        h.toggle_attack_style_index(slot)?;
        let length = self.attack_animations.as_ref().ok_or(Error::Null)?.len() as i32;
        if slot < length && !h.is_npc()? {
            let animation = self.animation_at(slot)?;
            h.set_attack_animation(animation)?;
        }
        self.check_staff_configs(h, button.wrapping_sub(1))?;
        Ok(true)
    }

    /// `checkStaffConfigs(slot)`.
    fn check_staff_configs<H: Host>(&mut self, h: &mut H, slot: i32) -> Result<()> {
        if self.current != Some(WeaponInterfaces::Staff) {
            return self.select_auto_spell(h, -1, false, false);
        }
        let defensive = slot == 3;
        h.varp(439, if defensive { -5 } else { 0 }, false)?;
        if slot > 2 {
            h.varp(43, if defensive { -1 } else { 3 }, false)?;
        }
        Ok(())
    }

    /// `getAutospellId(spellId)`.
    pub fn autospell_id<H: Host>(&mut self, h: &mut H, spell_id: i32) -> Result<i32> {
        let modern = h.spell_book()? == 192;
        let data = table(h, modern)?;
        Ok(data
            .iter()
            .position(|id| *id == spell_id)
            .map_or(-1, |i| i as i32))
    }

    /// `selectAutoSpell(buttonId, adjustAttackStyle, forgetAutocast)`.
    pub fn select_auto_spell<H: Host>(
        &mut self,
        h: &mut H,
        button: i32,
        adjust_attack_style: bool,
        forget_autocast: bool,
    ) -> Result<()> {
        let standard_child = 83;
        let defensive_child = 183;
        let modern = h.spell_book()? == 192;
        let data = table(h, modern)?;
        let current = h.autocast()?;
        if button >= data.len() as i32 {
            return Ok(());
        }
        let config_start = if modern { 45 } else { 13 };
        if let Some(spell) = &current {
            for (index, id) in data.iter().enumerate() {
                if *id == h.spell_id(spell) {
                    let child = config_start + 2 * index as i32;
                    h.interface_config(WEAPON_STAFF_SEL, child, true)?;
                    h.interface_config(WEAPON_STAFF_SEL, 100 + child, true)?;
                }
            }
        }
        if forget_autocast {
            remove_attributes(
                &[ATTRIBUTE_AUTOCAST_BUTTON_ID, ATTRIBUTE_AUTOCAST_STYLE],
                |key| h.remove_attribute(key),
            )?;
        }
        if button < 0 {
            h.set_autocast(None)?;
            h.interface_config(WEAPON_STAFF_SEL, defensive_child, false)?;
            h.interface_config(WEAPON_STAFF_SEL, standard_child, false)?;
            if adjust_attack_style && current.is_some() {
                self.set_attack_style(h, 3)?;
                h.update_style()?;
            }
            return Ok(());
        }
        let defensive = h.attack_style_index()? == 3;
        h.interface_config(WEAPON_STAFF_SEL, defensive_child, defensive)?;
        h.interface_config(WEAPON_STAFF_SEL, standard_child, !defensive)?;
        let book = if modern {
            SpellBook::Modern
        } else {
            SpellBook::Ancient
        };
        let spell = match h.spell_lookup(book, data[button as usize])? {
            Lookup::None => None,
            Lookup::Combat(spell) => Some(spell),
            Lookup::Other => return Err(Error::Cast),
        };
        h.set_autocast(spell.as_ref())?;
        h.set_attribute(ATTRIBUTE_AUTOCAST_BUTTON_ID, Value::Int(button))?;
        let index = h.attack_style_index()?;
        h.set_attribute(ATTRIBUTE_AUTOCAST_STYLE, Value::Int(index))?;
        let mut config = config_start + 2 * button;
        let special = (modern && weapon_named(h, "Slayer's staff")?)
            || (modern && weapon_named(h, "Void knight mace")?);
        if special {
            let slayer = weapon_named(h, "Slayer's staff")?;
            config = match button {
                0 => 77,
                1 if slayer => 79,
                1 => 81,
                2 => 69,
                3 => 71,
                4 => 73,
                5 => 75,
                _ => config,
            };
            if config >= 85 || config <= 65 {
                return Ok(());
            }
            h.interface_config(
                WEAPON_STAFF_SEL,
                if defensive { 100 } else { 0 } + config,
                false,
            )?;
        } else {
            config += if defensive { 100 } else { 0 };
            config = match button {
                16 => {
                    if defensive {
                        265
                    } else {
                        253
                    }
                }
                17 => {
                    if defensive {
                        268
                    } else {
                        256
                    }
                }
                18 => {
                    if defensive {
                        271
                    } else {
                        259
                    }
                }
                19 => {
                    if defensive {
                        274
                    } else {
                        262
                    }
                }
                _ => config,
            };
            h.interface_config(WEAPON_STAFF_SEL, config, false)?;
        }
        Ok(())
    }

    /// `openAutocastSelect()`.
    pub fn open_autocast_select<H: Host>(&mut self, h: &mut H) -> Result<()> {
        if self.current != Some(WeaponInterfaces::Staff) {
            return Ok(());
        }
        if !self.can_autocast(h, true)? {
            self.set_attack_style(h, 3)?;
            return Ok(());
        }
        h.set_attribute("autocast_select", Value::Bool(true))?;
        let mut id = if h.spell_book()? == 193 { 797 } else { 319 };
        let slayer = weapon_named(h, "Slayer's staff")?;
        let mace = weapon_named(h, "Void knight mace")?;
        if slayer {
            id = 310;
        }
        if mace {
            id = 406;
        }
        let component = h.create_component(id);
        let definition = component.read().definition.clone().ok_or(Error::Null)?;
        definition.write().tab_index = 0;
        definition.write().kind = Some(Type::Tab);
        h.set_attribute("autocast_component", Value::Component(component.clone()))?;
        h.open_tab(&component)
    }

    /// `canAutocast(message)`.
    pub fn can_autocast<H: Host>(&mut self, h: &mut H, message: bool) -> Result<bool> {
        if self.current != Some(WeaponInterfaces::Staff) {
            return Ok(false);
        }
        if h.spell_book()? == SpellBook::Lunar.interface_id() {
            if message {
                h.message("You can't autocast Lunar magic.")?;
            }
            return Ok(false);
        }
        let first = h.equipment_new(SLOT_WEAPON)?;
        let ancient_staff = h
            .item_name(first)
            .ok_or(Error::Null)?
            .contains("ncient staff")
            || {
                let second = h.equipment_new(SLOT_WEAPON)?;
                h.item_name(second)
                    .ok_or(Error::Null)?
                    .contains("uriel's staff")
            };
        let book = h.spell_book()?;
        let refused = (book == 192 && ancient_staff) || {
            let book = h.spell_book()?;
            book == 193 && !ancient_staff
        };
        if refused {
            if message {
                h.message(
                    "You can only autocast ancient magicks with an Ancient or Zuriel's staff.",
                )?;
            }
            return Ok(false);
        }
        Ok(true)
    }

    /// `restoreAutocast()`.
    pub fn restore_autocast<H: Host>(&mut self, h: &mut H) -> Result<bool> {
        let button = h
            .get_attribute(ATTRIBUTE_AUTOCAST_BUTTON_ID, Value::Int(-1))?
            .int()?;
        let style = h
            .get_attribute(ATTRIBUTE_AUTOCAST_STYLE, Value::Int(-1))?
            .int()?;
        if button < 0 || style < 0 || !self.can_autocast(h, false)? {
            remove_attributes(
                &[ATTRIBUTE_AUTOCAST_BUTTON_ID, ATTRIBUTE_AUTOCAST_STYLE],
                |key| h.remove_attribute(key),
            )?;
            return Ok(false);
        }
        self.restore_attack_style(h, style)?;
        self.check_staff_configs(h, style)?;
        self.select_auto_spell(h, button, false, false)?;
        h.update_style()?;
        Ok(true)
    }

    /// `restoreAttackStyle(index)`.
    pub fn restore_attack_style<H: Host>(&mut self, h: &mut H, index: i32) -> Result<()> {
        if index < 0 {
            return Ok(());
        }
        let current = self.current.ok_or(Error::Null)?;
        if index >= styles(current).len() as i32 {
            return Ok(());
        }
        h.toggle_attack_style_index(index)?;
        h.set_attack_style(style_at(current, index)?)?;
        let length = self.attack_animations.as_ref().ok_or(Error::Null)?.len() as i32;
        if index < length && !h.is_npc()? {
            let animation = self.animation_at(index)?;
            h.set_attack_animation(animation)?;
        }
        h.update_style()
    }

    /// `isSpecialBar()`.
    pub fn is_special_bar(&self) -> bool {
        self.special_bar
    }

    /// `getWeaponInterface()`.
    pub fn weapon_interface(&self) -> Option<WeaponInterfaces> {
        self.current
    }
}
