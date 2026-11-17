//! Original cached IF1/IF3 components. GPU, transport, and script execution are separate.
pub mod action_tooltip;
pub mod actions;
pub mod chat;
pub mod client_code;
mod component_fields;
mod component_handles;
pub mod cs1;
pub mod delayed;
pub mod drag;
pub mod font;
pub mod game_message;
pub mod icon;
pub mod icon_text;
pub mod inventory_examine;
pub mod inventory_press;
pub mod inventory_render;
pub mod inventory_scroll;
pub mod item;
pub mod legacy_buttons;
pub mod model;
pub mod model_animation;
pub mod model_assets;
pub mod outline;
pub mod public_chat;
pub mod quickchat;
pub mod raster;
pub mod script;
pub mod scrollbar;
pub mod skills;
pub mod small_text;
pub mod social;
pub mod sprite;
pub mod state;
pub mod targeting;
pub mod text;
mod text_order;
pub mod varps;
pub mod viewport;
pub mod vm;
pub mod widget_legacy;
pub mod widget_sprite;
pub mod widget_style;
pub mod world_map_host;
pub use component_fields::Component;
use rs09_protocol::{Error, Result, buffer::Reader};
use serde::Serialize;

pub use rs09_client_data::{HookArg, latin1};
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Properties {
    pub events: i32,
    #[serde(rename = "targetParam")]
    pub target_param: i32,
}
impl Default for Properties {
    fn default() -> Self {
        Self {
            events: 0,
            target_param: -1,
        }
    }
}
impl Properties {
    pub fn target_mask(self) -> i32 {
        (self.events >> 11) & 127
    }
    pub fn button_enabled(self, button: i32) -> bool {
        (0..31).contains(&button) && self.events.wrapping_shr((button + 1) as u32) & 1 != 0
    }
}
fn text(r: &mut Reader<'_>) -> Result<String> {
    Ok(latin1(r.gjstr()?))
}
fn id(r: &mut Reader<'_>) -> Result<i32> {
    let n = r.g2()?;
    Ok(if n == 65535 { -1 } else { n })
}
fn triggers(r: &mut Reader<'_>) -> Result<Option<Vec<i32>>> {
    let n = r.g1()?;
    if n == 0 {
        return Ok(None);
    }
    Ok(Some((0..n).map(|_| r.g4()).collect::<Result<_>>()?))
}
fn hook(r: &mut Reader<'_>, has: &mut bool) -> Result<Option<Vec<HookArg>>> {
    let n = r.g1()?;
    if n == 0 {
        return Ok(None);
    }
    let mut v = Vec::new();
    for _ in 0..n {
        v.push(match r.g1()? {
            0 => HookArg::Int(r.g4()?),
            1 => HookArg::Text(text(r)?),
            _ => HookArg::Null,
        });
    }
    *has = true;
    Ok(Some(v))
}
impl Component {
    pub fn decode(component_id: i32, data: &[u8]) -> Result<(Self, usize)> {
        let mut c = Self {
            id: component_id,
            ..Self::default()
        };
        let mut r = Reader::new(data);
        if data.first() == Some(&255) {
            c.decode_if3(&mut r)?
        } else {
            c.decode_if1(&mut r)?
        }
        Ok((c, r.position()))
    }
    fn parent(&self, r: &mut Reader<'_>) -> Result<i32> {
        let n = id(r)?;
        Ok(if n < 0 {
            n
        } else {
            n.wrapping_add(self.id & 0xffff0000u32 as i32)
        })
    }
    fn inventory_options(&mut self, r: &mut Reader<'_>, flags: &mut i32) -> Result<()> {
        let mut options = vec![None; 5];
        for (i, option) in options.iter_mut().enumerate() {
            let s = text(r)?;
            if !s.is_empty() {
                *option = Some(s);
                *flags |= 1 << (i + 23);
            }
        }
        self.inv_options = Some(options);
        Ok(())
    }
    fn inventory_arrays(&mut self) -> Result<()> {
        let n = (self.base_width as usize)
            .checked_mul(self.base_height as usize)
            .filter(|&n| n <= 1_000_000)
            .ok_or(Error::new(0, "oversized widget inventory"))?;
        self.dynamic_width_value = 3;
        self.dynamic_height_value = 3;
        self.obj_counts = Some(vec![0; n]);
        self.obj_types = Some(vec![0; n]);
        Ok(())
    }
    fn decode_if1(&mut self, r: &mut Reader<'_>) -> Result<()> {
        self.if3 = false;
        self.kind = r.g1()?;
        self.button_type = r.g1()?;
        self.client_code = r.g2()?;
        self.base_x = r.g2b()?;
        self.base_y = r.g2b()?;
        self.base_width = r.g2()?;
        self.base_height = r.g2()?;
        self.dynamic_width_value = 0;
        self.dynamic_height_value = 0;
        self.y_mode = 0;
        self.x_mode = 0;
        self.alpha = r.g1()?;
        self.overlayer = self.parent(r)?;
        self.hover_overlayer = id(r)?;
        let n = r.g1()?;
        if n > 0 {
            let mut ops = Vec::new();
            let mut operands = Vec::new();
            for _ in 0..n {
                ops.push(r.g1()?);
                operands.push(r.g2()?);
            }
            self.cs1_comparison_opcodes = Some(ops);
            self.cs1_comparison_operands = Some(operands);
        }
        let n = r.g1()?;
        if n > 0 {
            let mut scripts = Vec::new();
            for _ in 0..n {
                let n = r.g2()?;
                scripts.push((0..n).map(|_| id(r)).collect::<Result<_>>()?);
            }
            self.cs1_scripts = Some(scripts);
        }
        if self.kind == 0 {
            self.scroll_max_v = r.g2()?;
            self.hidden = r.g1()? == 1;
        }
        if self.kind == 1 {
            r.g2()?;
            r.g1()?;
        }
        let mut flags = 0;
        if self.kind == 2 {
            self.inventory_arrays()?;
            for bit in [28, 30, 31, 29] {
                if r.g1()? == 1 {
                    flags |= 1 << bit;
                }
            }
            self.inv_margin_x = r.g1()?;
            self.inv_margin_y = r.g1()?;
            let mut xs = vec![0; 20];
            let mut ys = vec![0; 20];
            let mut sprites = vec![-1; 20];
            for i in 0..20 {
                if r.g1()? == 1 {
                    xs[i] = r.g2b()?;
                    ys[i] = r.g2b()?;
                    sprites[i] = r.g4()?;
                }
            }
            self.inv_offset_x = Some(xs);
            self.inv_offset_y = Some(ys);
            self.inv_sprite = Some(sprites);
            self.inventory_options(r, &mut flags)?;
        }
        if self.kind == 3 {
            self.filled = r.g1()? == 1;
        }
        if matches!(self.kind, 1 | 4) {
            self.halign = r.g1()?;
            self.valign = r.g1()?;
            self.vpadding = r.g1()?;
            self.font = id(r)?;
            self.shadowed = r.g1()? == 1;
        }
        if self.kind == 4 {
            self.text = text(r)?;
            self.active_text = text(r)?;
        }
        if matches!(self.kind, 1 | 3 | 4) {
            self.color = r.g4()?;
        }
        if matches!(self.kind, 3 | 4) {
            self.active_color = r.g4()?;
            self.over_color = r.g4()?;
            self.active_over_color = r.g4()?;
        }
        if self.kind == 5 {
            self.sprite_id = r.g4()?;
            self.active_sprite_id = r.g4()?;
        }
        if self.kind == 6 {
            self.model_type = 1;
            self.model_id = id(r)?;
            self.active_model_type = 1;
            self.active_model_id = id(r)?;
            self.model_seq_id = id(r)?;
            self.active_model_seq_id = id(r)?;
            self.model_zoom = r.g2()?;
            self.model_x_angle = r.g2()?;
            self.model_y_angle = r.g2()?;
        }
        if self.kind == 7 {
            self.inventory_arrays()?;
            self.halign = r.g1()?;
            self.font = id(r)?;
            self.shadowed = r.g1()? == 1;
            self.color = r.g4()?;
            self.inv_margin_x = r.g2b()?;
            self.inv_margin_y = r.g2b()?;
            if r.g1()? == 1 {
                flags |= 1 << 30;
            }
            self.inventory_options(r, &mut flags)?;
        }
        if self.kind == 8 {
            self.text = text(r)?;
        }
        if self.button_type == 2 || self.kind == 2 {
            self.option_circumfix = text(r)?;
            self.option_suffix = text(r)?;
            flags |= (r.g2()? & 63) << 11;
        }
        if matches!(self.button_type, 1 | 4 | 5 | 6) {
            self.option = text(r)?;
            if self.option.is_empty() {
                self.option = match self.button_type {
                    1 => "Ok",
                    4 | 5 => "Select",
                    _ => "Continue",
                }
                .into();
            }
        }
        if matches!(self.button_type, 1 | 4 | 5) {
            flags |= 1 << 22;
        }
        if self.button_type == 6 {
            flags |= 1;
        }
        self.properties = Properties {
            events: flags,
            target_param: -1,
        };
        Ok(())
    }
    fn decode_if3(&mut self, r: &mut Reader<'_>) -> Result<()> {
        self.if3 = true;
        r.g1()?;
        self.kind = r.g1()?;
        if self.kind & 128 != 0 {
            self.kind &= 127;
            text(r)?;
        }
        self.client_code = r.g2()?;
        self.base_x = r.g2b()?;
        self.base_y = r.g2b()?;
        self.base_width = r.g2()?;
        self.base_height = r.g2()?;
        self.dynamic_width_value = r.g1b()?;
        self.dynamic_height_value = r.g1b()?;
        self.y_mode = r.g1b()?;
        self.x_mode = r.g1b()?;
        self.overlayer = self.parent(r)?;
        self.hidden = r.g1()? == 1;
        match self.kind {
            0 => {
                self.scroll_max_h = r.g2()?;
                self.scroll_max_v = r.g2()?;
                self.no_click_through = r.g1()? == 1;
            }
            5 => {
                self.sprite_id = r.g4()?;
                self.angle2d = r.g2()?;
                let flags = r.g1()?;
                self.has_alpha = flags & 2 != 0;
                self.sprite_tiling = flags & 1 != 0;
                self.alpha = r.g1()?;
                self.outline_thickness = r.g1()?;
                self.shadow_color = r.g4()?;
                self.v_flip = r.g1()? == 1;
                self.h_flip = r.g1()? == 1;
            }
            6 => {
                self.model_type = 1;
                self.model_id = id(r)?;
                self.model_origin_x = r.g2b()?;
                self.model_origin_y = r.g2b()?;
                self.model_x_angle = r.g2()?;
                self.model_y_angle = r.g2()?;
                self.model_y_offset = r.g2()?;
                self.model_zoom = r.g2()?;
                self.model_seq_id = id(r)?;
                self.model_ortho = r.g1()? == 1;
                self.model_view_angle = r.g2b()?;
                self.model_near_clip = r.g2b()?;
                self.model_transparent = r.g1()? == 1;
                if self.dynamic_width_value != 0 {
                    self.model_viewport_width = r.g2()?;
                }
                if self.dynamic_height_value != 0 {
                    self.model_viewport_height = r.g2()?;
                }
            }
            4 => {
                self.font = id(r)?;
                self.text = text(r)?;
                self.vpadding = r.g1()?;
                self.halign = r.g1()?;
                self.valign = r.g1()?;
                self.shadowed = r.g1()? == 1;
                self.color = r.g4()?;
            }
            3 => {
                self.color = r.g4()?;
                self.filled = r.g1()? == 1;
                self.alpha = r.g1()?;
            }
            9 => {
                self.line_width = r.g1()?;
                self.color = r.g4()?;
                self.line_flipped = r.g1()? == 1;
            }
            _ => {}
        }
        let flags = r.g3()?;
        let mut key = r.g1()?;
        if key != 0 {
            let (mut delays, mut codes, mut modifiers) = (vec![0; 10], vec![0; 10], vec![0; 10]);
            while key != 0 {
                let index = (key >> 4) - 1;
                if !(0..10).contains(&index) {
                    return Err(Error::new(r.position(), "invalid widget key index"));
                }
                key = (r.g1()? | key << 8) & 4095;
                delays[index as usize] = if key == 4095 { -1 } else { key };
                codes[index as usize] = r.g1b()?;
                modifiers[index as usize] = r.g1b()?;
                key = r.g1()?;
            }
            self.key_repeat_delays = Some(delays);
            self.key_codes = Some(codes);
            self.key_modifiers = Some(modifiers);
        }
        self.option_base = text(r)?;
        let ops = r.g1()?;
        let n = ops & 15;
        if n > 0 {
            self.ops = Some((0..n).map(|_| text(r).map(Some)).collect::<Result<_>>()?);
        }
        if ops >> 4 > 0 {
            let i = r.g1()? as usize;
            let mut targets = vec![-1; i + 1];
            targets[i] = r.g2()?;
            self.drag_targets = Some(targets);
        }
        if ops >> 4 > 1 {
            let i = r.g1()? as usize;
            let value = r.g2()?;
            let slot = self
                .drag_targets
                .as_mut()
                .and_then(|t| t.get_mut(i))
                .ok_or(Error::new(r.position(), "invalid widget drag target"))?;
            *slot = value;
        }
        self.drag_deadzone = r.g1()?;
        self.drag_deadtime = r.g1()?;
        self.drag_render_behavior = r.g1()? == 1;
        self.option_circumfix = text(r)?;
        let target = if (flags >> 11) & 127 != 0 {
            let value = id(r)?;
            self.target_cursor = id(r)?;
            self.default_target_cursor = id(r)?;
            value
        } else {
            -1
        };
        self.properties = Properties {
            events: flags,
            target_param: target,
        };
        self.on_load = hook(r, &mut self.has_event_handlers)?;
        self.on_mouse_over = hook(r, &mut self.has_event_handlers)?;
        self.on_mouse_leave = hook(r, &mut self.has_event_handlers)?;
        self.on_use_with = hook(r, &mut self.has_event_handlers)?;
        self.on_use = hook(r, &mut self.has_event_handlers)?;
        self.on_varp_transmit = hook(r, &mut self.has_event_handlers)?;
        self.on_inv_transmit = hook(r, &mut self.has_event_handlers)?;
        self.on_stat_transmit = hook(r, &mut self.has_event_handlers)?;
        self.on_timer = hook(r, &mut self.has_event_handlers)?;
        self.on_option_click = hook(r, &mut self.has_event_handlers)?;
        self.on_mouse_repeat = hook(r, &mut self.has_event_handlers)?;
        self.on_click_repeat = hook(r, &mut self.has_event_handlers)?;
        self.on_drag = hook(r, &mut self.has_event_handlers)?;
        self.on_release = hook(r, &mut self.has_event_handlers)?;
        self.on_hold = hook(r, &mut self.has_event_handlers)?;
        self.on_drag_start = hook(r, &mut self.has_event_handlers)?;
        self.on_drag_release = hook(r, &mut self.has_event_handlers)?;
        self.on_scroll = hook(r, &mut self.has_event_handlers)?;
        self.on_varc_transmit = hook(r, &mut self.has_event_handlers)?;
        self.on_varcstr_transmit = hook(r, &mut self.has_event_handlers)?;
        self.varp_triggers = triggers(r)?;
        self.inventory_triggers = triggers(r)?;
        self.stat_triggers = triggers(r)?;
        self.varc_triggers = triggers(r)?;
        self.varcstr_triggers = triggers(r)?;
        Ok(())
    }
    /// InterfaceList.calculateSize, with Java int arithmetic. QA layout clamping is disabled by default.
    pub fn calculate_size(&mut self, parent_width: i32, parent_height: i32) -> Result<()> {
        match self.dynamic_width_value {
            0 => self.width = self.base_width,
            1 => self.width = parent_width.wrapping_sub(self.base_width),
            2 => self.width = self.base_width.wrapping_mul(parent_width) >> 14,
            3 => match self.kind {
                2 => {
                    self.width = self.base_width.wrapping_mul(32).wrapping_add(
                        self.base_width
                            .wrapping_sub(1)
                            .wrapping_mul(self.inv_margin_x),
                    )
                }
                7 => {
                    self.width = self.base_width.wrapping_mul(115).wrapping_add(
                        self.base_width
                            .wrapping_sub(1)
                            .wrapping_mul(self.inv_margin_x),
                    )
                }
                _ => {}
            },
            _ => {}
        }
        match self.dynamic_height_value {
            0 => self.height = self.base_height,
            1 => self.height = parent_height.wrapping_sub(self.base_height),
            2 => self.height = self.base_height.wrapping_mul(parent_height) >> 14,
            3 => match self.kind {
                2 => {
                    self.height = self.base_height.wrapping_mul(32).wrapping_add(
                        self.base_height
                            .wrapping_sub(1)
                            .wrapping_mul(self.inv_margin_y),
                    )
                }
                7 => {
                    self.height = self.base_height.wrapping_mul(12).wrapping_add(
                        self.base_height
                            .wrapping_sub(1)
                            .wrapping_mul(self.inv_margin_y),
                    )
                }
                _ => {}
            },
            _ => {}
        }
        if self.dynamic_width_value == 4 {
            self.width = self
                .aspect_width
                .wrapping_mul(self.height)
                .checked_div(self.aspect_height)
                .ok_or(Error::new(0, "invalid widget aspect ratio"))?;
        }
        if self.dynamic_height_value == 4 {
            self.height = self
                .aspect_height
                .wrapping_mul(self.width)
                .checked_div(self.aspect_width)
                .ok_or(Error::new(0, "invalid widget aspect ratio"))?;
        }
        Ok(())
    }
    /// The reference's xMode selects the vertical axis and yMode the horizontal axis.
    pub fn calculate_position(&mut self, parent_width: i32, parent_height: i32) {
        fn pos(mode: i32, base: i32, parent: i32, size: i32) -> i32 {
            match mode {
                0 => base,
                1 => base.wrapping_add(parent.wrapping_sub(size) / 2),
                2 => parent.wrapping_sub(size).wrapping_sub(base),
                3 => base.wrapping_mul(parent) >> 14,
                4 => (base.wrapping_mul(parent) >> 14).wrapping_add(parent.wrapping_sub(size) / 2),
                _ => parent
                    .wrapping_sub(base.wrapping_mul(parent) >> 14)
                    .wrapping_sub(size),
            }
        }
        self.x = pos(self.y_mode, self.base_x, parent_width, self.width);
        self.y = pos(self.x_mode, self.base_y, parent_height, self.height);
    }
}
