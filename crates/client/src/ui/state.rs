//! Interface state driven by the original server updates. Item/economy changes are never predicted.
use super::{Component, HookArg, latin1};
use crate::pack::Pack;
use rs09_protocol::game::{
    Frame,
    updates::{ServerUpdate, WireItem},
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
#[cfg(test)]
#[path = "state_button_tests.rs"]
mod button_tests;
#[path = "hook_walk.rs"]
mod hook_walk;
#[path = "state_legacy.rs"]
mod legacy;
#[cfg(test)]
#[path = "state_menu_tests.rs"]
mod menu_tests;
#[path = "state_session.rs"]
mod session;
pub use hook_walk::{KeyInput, drag_source};
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, serde::Serialize)]
pub struct Key {
    pub id: i32,
    pub child: i32,
}
impl Key {
    pub fn component(id: i32) -> Self {
        Self { id, child: -1 }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}
impl Rect {
    pub fn contains(self, p: [i32; 2]) -> bool {
        p[0] >= self.x
            && p[1] >= self.y
            && p[0] < self.x.saturating_add(self.w)
            && p[1] < self.y.saturating_add(self.h)
    }
    pub fn intersect(self, other: Self) -> Self {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        Self {
            x,
            y,
            w: (self
                .x
                .saturating_add(self.w)
                .min(other.x.saturating_add(other.w))
                - x)
                .max(0),
            h: (self
                .y
                .saturating_add(self.h)
                .min(other.y.saturating_add(other.h))
                - y)
                .max(0),
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct DrawNode {
    pub key: Key,
    pub rect: Rect,
    pub clip: Rect,
    /// Drawing inherits the parent's raster clip; bounds still limit input hits.
    pub paint_clip: Rect,
    /// Exclusive end of this visible subtree, so container chrome paints last.
    pub subtree_end: usize,
}
#[derive(Clone, Debug)]
pub struct Hook {
    pub source: Key,
    pub arguments: Vec<HookArg>,
    pub op: i32,
    pub op_base: String,
    pub mouse: [i32; 2],
    pub target: Option<Key>,
    pub key_code: i32,
    pub key_char: i32,
    /// The created component a queued request was made for; the request is
    /// dropped when that slot has been replaced by the time it would run.
    pub(crate) lease: Option<super::component_handles::Handle>,
}
#[derive(Clone, Copy, Default)]
pub struct PointerInput {
    pub point: [i32; 2],
    pub clicked: bool,
    pub down: bool,
    /// IF1 scrollbars accept any pressed mouse button; IF3 hooks use `down`.
    pub any_down: bool,
    pub scroll: i32,
    pub suppressed: bool,
}
impl Hook {
    pub fn new(source: Key, arguments: Vec<HookArg>) -> Self {
        Self {
            source,
            arguments,
            op: 0,
            op_base: String::new(),
            mouse: [0; 2],
            target: None,
            key_code: 0,
            key_char: 0,
            lease: None,
        }
    }
}
pub struct State {
    pub pack: Pack,
    pub components: BTreeMap<Key, Component>,
    handles: super::component_handles::Handles,
    pub(crate) inventory_press: super::inventory_press::Press<super::component_handles::Handle>,
    pub(crate) inventory_redraws: i32,
    legacy_hover: Option<super::component_handles::Handle>,
    please_wait: Option<super::component_handles::Handle>,
    legacy_tooltip: Option<super::component_handles::Handle>,
    tooltip_timer: i32,
    scrollbar_drag: super::scrollbar::Drag,
    /// Last-login DNS result is a caller service; no DNS request runs in rendering.
    pub last_login_address: Option<(i32, Option<String>)>,
    pub loaded: BTreeSet<i32>,
    pub root: i32,
    pub subinterfaces: BTreeMap<i32, (i32, i32)>,
    /// How often a sub-interface was opened or closed. Each time, the original
    /// trims the context menu (`context_menu::ContextMenu::interface_changed`).
    pub subinterface_changes: u32,
    pub containers: BTreeMap<i32, Vec<WireItem>>,
    pub hooks: VecDeque<Hook>,
    pub verify_id: i32,
    pub updated_inventories: [i32; 32],
    pub inventory_writer: i32,
    pub diagnostics: VecDeque<String>,
    pub viewport: [i32; 2],
    pub items: super::item::Catalog,
    pub created_lengths: BTreeMap<i32, usize>,
    pub properties: BTreeMap<Key, super::Properties>,
    pub drag_parents: BTreeMap<Key, Key>,
    pub delayed: super::delayed::Queue,
    pub updated_varcs: [i32; 32],
    pub varc_writer: i32,
    pub updated_varcstrs: [i32; 32],
    pub varcstr_writer: i32,
    pub updated_varps: [i32; 32],
    pub varp_writer: i32,
    pub selected_item: Option<super::actions::SelectedItem>,
    pub targeting: super::targeting::Targeting,
    pub(crate) target_params: BTreeMap<i32, super::targeting::ParamType>,
    pub key_events: Vec<[i32; 2]>,
    pub transmit_timer: i32,
    pub chat_transmit_at: i32,
    pub friend_transmit_at: i32,
    pub clan_transmit_at: i32,
    pub stock_transmit_at: i32,
    pub misc_transmit_at: i32,
    pub updated_stats: [i32; 32],
    pub stat_writer: i32,
    /// Redraw callbacks requested by the model-update wire reducer (for QA).
    pub model_redraws: usize,
    pub legacy_button_redraws: i32,
    /// The world map (client codes 1400/1401, CS2 5200..=5220).
    pub world_map: super::world_map_host::Host,
    /// `Camera.splineJustFinished`, set by the caller before each cycle's
    /// walk; `onMinimapUnlock` hooks run on the cycle it is true.
    pub spline_just_finished: bool,
    /// The component whose drag a click began during the last walk, if no
    /// `noClickThrough` layer undid it (`Cs1ScriptRunner.draggedComponent`).
    pub drag_candidate: Option<Key>,
    /// `onResize` requests queued by forced layouts for the next walk.
    resize_hooks: Vec<Hook>,
    /// The layout in progress was forced by a new canvas size.
    resize_all: bool,
    /// A layout forced below one component (`layoutComponent(c, true)`).
    resize_scope: Option<i32>,
    resize_inside: u32,
}
impl State {
    pub fn new(pack: Pack) -> Self {
        Self {
            pack,
            components: BTreeMap::new(),
            handles: super::component_handles::Handles::default(),
            inventory_press: super::inventory_press::Press::default(),
            inventory_redraws: 0,
            legacy_hover: None,
            please_wait: None,
            legacy_tooltip: None,
            tooltip_timer: 0,
            scrollbar_drag: super::scrollbar::Drag::default(),
            last_login_address: None,
            loaded: BTreeSet::new(),
            root: -1,
            subinterfaces: BTreeMap::new(),
            subinterface_changes: 0,
            containers: BTreeMap::new(),
            hooks: VecDeque::new(),
            verify_id: 0,
            updated_inventories: [0; 32],
            inventory_writer: 0,
            diagnostics: VecDeque::new(),
            viewport: [765, 503],
            items: super::item::Catalog::default(),
            created_lengths: BTreeMap::new(),
            properties: BTreeMap::new(),
            drag_parents: BTreeMap::new(),
            delayed: super::delayed::Queue::default(),
            updated_varcs: [0; 32],
            varc_writer: 0,
            updated_varcstrs: [0; 32],
            varcstr_writer: 0,
            updated_varps: [0; 32],
            varp_writer: 0,
            selected_item: None,
            targeting: super::targeting::Targeting::default(),
            target_params: BTreeMap::new(),
            key_events: Vec::new(),
            transmit_timer: 1,
            chat_transmit_at: 0,
            friend_transmit_at: 0,
            clan_transmit_at: 0,
            stock_transmit_at: 0,
            misc_transmit_at: 0,
            updated_stats: [0; 32],
            stat_writer: 0,
            model_redraws: 0,
            legacy_button_redraws: 0,
            world_map: super::world_map_host::Host::default(),
            spline_just_finished: false,
            drag_candidate: None,
            resize_hooks: Vec::new(),
            resize_all: false,
            resize_scope: None,
            resize_inside: 0,
        }
    }
    pub fn load_group(&mut self, group: i32) -> Result<(), String> {
        if self.loaded.contains(&group) {
            return Ok(());
        }
        let prefix = format!("interface/{group}/");
        let mut pending = Vec::new();
        let mut found = false;
        for name in self
            .pack
            .manifest
            .files
            .keys()
            .filter(|s| s.starts_with(&prefix))
        {
            found = true;
            let child = name[prefix.len()..]
                .parse::<i32>()
                .map_err(|e| e.to_string())?;
            let id = group << 16 | child;
            if self.components.contains_key(&Key::component(id)) {
                continue;
            }
            let (component, _) =
                Component::decode(id, self.pack.get(name)?).map_err(|e| e.to_string())?;
            pending.push((Key::component(id), component));
        }
        if !found {
            return Err(format!("Missing cached interface {group}"));
        }
        self.components.extend(pending);
        self.loaded.insert(group);
        Ok(())
    }
    pub fn get(&self, key: Key) -> Result<&Component, String> {
        self.components
            .get(&key)
            .ok_or_else(|| format!("Missing widget {}:{}", key.id, key.child))
    }
    pub fn get_mut(&mut self, key: Key) -> Result<&mut Component, String> {
        self.load_group((key.id as u32 >> 16) as i32)?;
        self.components
            .get_mut(&key)
            .ok_or_else(|| format!("Missing widget {}:{}", key.id, key.child))
    }
    pub(crate) fn retain_component(
        &mut self,
        key: Key,
    ) -> Result<super::component_handles::Handle, String> {
        self.get(key)?;
        Ok(self.handles.retain(key))
    }
    pub(crate) fn component_is_current(&self, handle: &super::component_handles::Handle) -> bool {
        self.handles.is_current(handle)
    }
    pub(crate) fn component_ref(&self, handle: &super::component_handles::Handle) -> &Component {
        if self.handles.is_current(handle) {
            &self.components[&handle.key()]
        } else {
            self.handles.get(handle).expect("live component lease")
        }
    }
    pub(crate) fn component_ref_mut(
        &mut self,
        handle: &super::component_handles::Handle,
    ) -> &mut Component {
        if self.handles.is_current(handle) {
            self.components
                .get_mut(&handle.key())
                .expect("current component lease")
        } else {
            self.handles.get_mut(handle).expect("live component lease")
        }
    }
    pub(crate) fn replace_component(&mut self, key: Key, component: Component) {
        if let Some(old) = self.components.insert(key, component) {
            self.handles.retire(key, old);
        }
    }
    pub(crate) fn remove_component(&mut self, key: Key) {
        if let Some(old) = self.components.remove(&key) {
            self.handles.retire(key, old);
        }
    }
    pub(crate) fn reference_properties(
        &self,
        handle: &super::component_handles::Handle,
    ) -> super::Properties {
        self.properties
            .get(&handle.key())
            .copied()
            .unwrap_or(self.component_ref(handle).properties)
    }
    pub(crate) fn refresh_reference(
        &mut self,
        handle: &super::component_handles::Handle,
    ) -> Result<(), String> {
        let c = self.component_ref(handle);
        let parent_id = if c.overlayer != -1 {
            Some(c.overlayer)
        } else {
            self.subinterfaces
                .iter()
                .find(|(_, v)| v.0 == c.id >> 16)
                .map(|(&id, _)| id)
        };
        let size = parent_id
            .and_then(|id| self.components.get(&Key::component(id)))
            .map_or(self.viewport, |p| [p.width, p.height]);
        let c = self.component_ref_mut(handle);
        c.calculate_size(size[0], size[1])
            .map_err(|e| e.to_string())?;
        c.calculate_position(size[0], size[1]);
        Ok(())
    }
    pub fn refresh_component(&mut self, key: Key) -> Result<(), String> {
        let c = self.get(key)?;
        let parent_id = if c.overlayer != -1 {
            Some(c.overlayer)
        } else {
            self.subinterfaces
                .iter()
                .find(|(_, v)| v.0 == c.id >> 16)
                .map(|(&id, _)| id)
        };
        let size = parent_id
            .and_then(|id| self.components.get(&Key::component(id)))
            .map_or(self.viewport, |p| [p.width, p.height]);
        let c = self.get_mut(key)?;
        c.calculate_size(size[0], size[1])
            .map_err(|e| e.to_string())?;
        c.calculate_position(size[0], size[1]);
        Ok(())
    }
    pub fn drag_render_parent(&self, mut key: Key) -> Option<Key> {
        let depth = self.properties(key).ok()?.events >> 18 & 7;
        if depth == 0 {
            return None;
        }
        for _ in 0..depth {
            key = Key::component(self.components.get(&key)?.overlayer);
            self.components.get(&key)?;
        }
        Some(key)
    }
    pub fn inventory_slot_at(
        &self,
        nodes: &[DrawNode],
        point: [i32; 2],
    ) -> Option<(Key, i32, Rect)> {
        let mut hit = None;
        for node in nodes {
            let c = &self.components[&node.key];
            if c.kind != 2 || !node.clip.contains(point) {
                continue;
            }
            for slot in 0..c.obj_types.as_ref().map_or(0, Vec::len) {
                let rect = Rect {
                    x: node.rect.x
                        + (slot as i32 % c.base_width.max(1)) * (32 + c.inv_margin_x)
                        + c.inv_offset_x
                            .as_ref()
                            .and_then(|v| v.get(slot))
                            .copied()
                            .unwrap_or(0),
                    y: node.rect.y
                        + (slot as i32 / c.base_width.max(1)) * (32 + c.inv_margin_y)
                        + c.inv_offset_y
                            .as_ref()
                            .and_then(|v| v.get(slot))
                            .copied()
                            .unwrap_or(0),
                    w: 32,
                    h: 32,
                };
                if rect.contains(point) {
                    hit = Some((node.key, slot as i32, rect));
                }
            }
        }
        hit
    }
    pub fn menu_at(
        &mut self,
        nodes: &[DrawNode],
        point: [i32; 2],
        menu: &mut crate::interaction::Menu,
    ) -> bool {
        self.menu_at_with(nodes, point, menu, |_, _, _| None)
    }
    /// Special client-code components participate in the same draw-order clearing
    /// as ordinary controls, so an overlaid interface can block their actions.
    pub fn menu_at_with(
        &mut self,
        nodes: &[DrawNode],
        point: [i32; 2],
        menu: &mut crate::interaction::Menu,
        mut special: impl FnMut(&Component, &DrawNode, &mut crate::interaction::Menu) -> Option<bool>,
    ) -> bool {
        let mut hit = false;
        // Where a hovered layer's modal sub-interface starts in the drawing
        // order: Cs1ScriptRunner.renderComponent resets the menu to Cancel
        // there, after the layer's own children have added their entries.
        let mut modal_starts: Vec<usize> = Vec::new();
        for (index, node) in nodes.iter().enumerate() {
            if modal_starts.contains(&index) {
                menu.entries.retain(|e| e.action == 1005);
                hit = true;
            }
            if !node.clip.contains(point) || !node.rect.contains(point) {
                continue;
            }
            let c = self.components[&node.key].clone();
            let properties = self.properties(node.key).unwrap_or(c.properties);
            // Only a layer (type 0) that does not let clicks through resets
            // the menu; what was added before it, the world included, goes.
            if c.kind == 0 && c.no_click_through {
                menu.entries.retain(|e| e.action == 1005);
                hit = true;
            }
            if c.kind == 0 && node.key.child == -1 {
                if let Some(&(group, 0)) = self.subinterfaces.get(&node.key.id) {
                    let end = node.subtree_end.min(nodes.len());
                    modal_starts.push(
                        (index + 1..end)
                            .find(|&i| nodes[i].key.id >> 16 == group)
                            .unwrap_or(end),
                    );
                }
            }
            if let Some(special_hit) = special(&c, node, menu) {
                hit |= special_hit;
                continue;
            }
            if matches!(c.kind, 2 | 3 | 4 | 5 | 6 | 9) || c.button_type > 0 || c.ops.is_some() {
                hit = true;
            }
            // Set before adding an IF3 op (`MiniMenu.getOpCursor`); every other
            // entry added here has no cursor of its own.
            let op_cursor = std::cell::Cell::new(-1);
            let mut add = |action, op, key, target: &str| {
                menu.add(crate::interaction::MenuEntry {
                    option: op,
                    target: target.into(),
                    action,
                    key,
                    args: [node.key.child, c.id],
                    cursor: op_cursor.replace(-1),
                });
            };
            if !c.if3 {
                match c.button_type {
                    1 => add(8, c.option.clone(), 0, ""),
                    2 if !self.targeting.active => {
                        if let Some(verb) = super::targeting::target_verb(&c, properties) {
                            add(
                                32,
                                verb.into(),
                                0,
                                &format!("<col=00ff00>{}", c.option_suffix),
                            );
                        }
                    }
                    3 => add(28, "Close".into(), 0, ""),
                    4 => add(59, c.option.clone(), 0, ""),
                    5 => add(51, c.option.clone(), 0, ""),
                    6 if self.please_wait.is_none() => add(41, c.option.clone(), 0, ""),
                    _ => {}
                }
                if c.kind == 2 {
                    if let Some(ids) = &c.obj_types {
                        for (slot, &wire_id) in ids.iter().enumerate() {
                            if wire_id <= 0 {
                                continue;
                            }
                            let x = (slot as i32 % c.base_width.max(1)) * (32 + c.inv_margin_x)
                                + c.inv_offset_x
                                    .as_ref()
                                    .and_then(|a| a.get(slot))
                                    .copied()
                                    .unwrap_or(0);
                            let y = (slot as i32 / c.base_width.max(1)) * (32 + c.inv_margin_y)
                                + c.inv_offset_y
                                    .as_ref()
                                    .and_then(|a| a.get(slot))
                                    .copied()
                                    .unwrap_or(0);
                            let rect = Rect {
                                x: node.rect.x + x,
                                y: node.rect.y + y,
                                w: 32,
                                h: 32,
                            };
                            if !rect.contains(point) {
                                continue;
                            }
                            if let Ok(item) = self.items.get(&self.pack, wire_id - 1) {
                                if let Some(selected) = &self.selected_item {
                                    if properties.events >> 30 & 1 != 0 {
                                        if selected.component != c.id
                                            || selected.slot != slot as i32
                                        {
                                            menu.add(crate::interaction::MenuEntry {
                                                option: "Use".into(),
                                                target: format!(
                                                    "{} -> <col=ff9040>{}",
                                                    selected.text, item.name
                                                ),
                                                action: 40,
                                                key: item.id as u64,
                                                args: [slot as i32, c.id],
                                                cursor: -1,
                                            });
                                        }
                                        continue;
                                    }
                                }
                                if self.targeting.active && properties.events >> 30 & 1 != 0 {
                                    let entry = crate::interaction::MenuEntry {
                                        option: String::new(),
                                        target: format!("<col=ff9040>{}", item.name),
                                        action: 3,
                                        key: item.id as u64,
                                        args: [slot as i32, c.id],
                                        cursor: -1,
                                    };
                                    if let Err(error) =
                                        self.target_entry(menu, 16, item.params.as_ref(), entry)
                                    {
                                        self.diagnostic(error);
                                    }
                                    continue;
                                }
                                let target = format!("<col=ff9040>{}", item.name);
                                let mut add_item = |action, option: String| {
                                    menu.add(crate::interaction::MenuEntry {
                                        option,
                                        target: target.clone(),
                                        action,
                                        key: item.id as u64,
                                        args: [slot as i32, c.id],
                                        cursor: -1,
                                    });
                                };
                                if properties.events >> 30 & 1 != 0 {
                                    for i in (3..5).rev() {
                                        if let Some(op) = &item.iops[i] {
                                            add_item([47, 5, 43, 35, 58][i], op.clone());
                                        }
                                    }
                                }
                                if properties.events >> 31 & 1 != 0 {
                                    add_item(22, "Use".into());
                                }
                                if properties.events >> 30 & 1 != 0 {
                                    for i in (0..3).rev() {
                                        if let Some(op) = &item.iops[i] {
                                            add_item([47, 5, 43, 35, 58][i], op.clone());
                                        }
                                    }
                                }
                                if let Some(ops) = &c.inv_options {
                                    for i in (0..5).rev() {
                                        if let Some(op) = &ops[i] {
                                            add_item([25, 23, 48, 7, 13][i], op.clone());
                                        }
                                    }
                                }
                                add_item(1006, "Examine".into());
                            }
                        }
                    }
                }
                continue;
            }
            if self.targeting.active {
                if properties.events >> 22 & 1 != 0 && self.targeting.mask & 32 != 0 {
                    menu.add(crate::interaction::MenuEntry {
                        option: self.targeting.verb.clone(),
                        target: format!("{} -> {}", self.targeting.text, c.option_base),
                        action: 12,
                        key: 0,
                        args: [node.key.child, c.id],
                        cursor: self.targeting.cursor,
                    });
                }
                continue;
            }
            for i in (0..10).rev() {
                if i == 4 {
                    if let Some(verb) = super::targeting::target_verb(&c, properties) {
                        add(32, verb.into(), 0, &c.option_base);
                    }
                }
                if !properties.button_enabled(i as i32) && c.on_option_click.is_none() {
                    continue;
                }
                if let Some(Some(op)) = c.ops.as_ref().and_then(|ops| ops.get(i)) {
                    if !op.trim().is_empty() {
                        op_cursor.set(
                            c.drag_targets
                                .as_ref()
                                .and_then(|cursors| cursors.get(i))
                                .copied()
                                .unwrap_or(-1),
                        );
                        add(
                            if i >= 5 { 1003 } else { 9 },
                            op.clone(),
                            (i + 1) as u64,
                            &c.option_base,
                        );
                    }
                }
            }
            if properties.events & 1 != 0 {
                add(41, "Continue".into(), 0, "");
            }
        }
        if modal_starts.contains(&nodes.len()) {
            menu.entries.retain(|e| e.action == 1005);
            hit = true;
        }
        hit
    }
    /// ClientProt.sendButtonClick's wire phase; hooks must run before this is called.
    pub fn button_packet(&self, key: Key, op: i32) -> Result<Option<Frame>, String> {
        let c = self.get(key)?;
        if c.client_code > 0 {
            return Err(format!(
                "Special client button {} is not ported",
                c.client_code
            ));
        }
        if !(1..=10).contains(&op) || !self.properties(key)?.button_enabled(op - 1) {
            return Ok(None);
        }
        Ok(Self::button_frame(key, op))
    }
    pub(crate) fn button_with_hook(
        &mut self,
        key: Key,
        op: i32,
        hook: impl FnOnce(
            &mut Self,
            &super::component_handles::Handle,
            Vec<HookArg>,
        ) -> Result<(), String>,
        special: impl FnOnce(&mut Self, &super::component_handles::Handle) -> Result<bool, String>,
    ) -> Result<Option<Frame>, String> {
        let Some(source) = self.lookup_reference(key)? else {
            return Ok(None);
        };
        if let Some(arguments) = self.component_ref(&source).on_option_click.clone() {
            hook(self, &source, arguments)?;
        }
        if self.component_ref(&source).client_code > 0 && !special(self, &source)? {
            return Ok(None);
        }
        if !self
            .reference_properties(&source)
            .button_enabled(op.wrapping_sub(1))
        {
            return Ok(None);
        }
        Ok(Self::button_frame(key, op))
    }
    fn button_frame(key: Key, op: i32) -> Option<Frame> {
        let opcode =
            *[155, 196, 124, 199, 234, 168, 166, 64, 53, 9].get(op.wrapping_sub(1) as usize)?;
        let mut w = rs09_protocol::buffer::Writer::new();
        w.p4(key.id);
        w.p2(key.child);
        Some(Frame {
            opcode,
            payload: w.into_vec(),
        })
    }
    pub fn properties(&self, key: Key) -> Result<super::Properties, String> {
        Ok(self
            .properties
            .get(&key)
            .copied()
            .unwrap_or(self.get(key)?.properties))
    }
    pub fn set_events(
        &mut self,
        pointer: i32,
        start: i32,
        end: i32,
        events: i32,
    ) -> Result<(), String> {
        if end.wrapping_sub(start) > 65536 {
            return Err("Oversized component properties range".into());
        }
        for child in start..=end {
            let key = Key { id: pointer, child };
            let target_param = if let Some(prev) = self.properties.get(&key) {
                prev.target_param
            } else if child == -1 {
                self.get_mut(Key::component(pointer))?
                    .properties
                    .target_param
            } else {
                -1
            };
            self.properties.insert(
                key,
                super::Properties {
                    events,
                    target_param,
                },
            );
        }
        Ok(())
    }
    pub fn run_on_load(&mut self, group: i32) {
        for (&key, c) in self
            .components
            .range(Key::component(group << 16)..Key::component((group + 1) << 16))
        {
            if key.child == -1 {
                if let Some(args) = &c.on_load {
                    self.hooks.push_back(Hook::new(key, args.clone()));
                }
            }
        }
    }
    pub fn open_root(&mut self, group: i32) -> Result<(), String> {
        self.load_group(group)?;
        self.root = group;
        self.reset_animations(group);
        self.layout(self.viewport[0], self.viewport[1])?;
        self.run_on_load(group);
        Ok(())
    }
    pub fn open_sub(&mut self, pointer: i32, group: i32, kind: i32) -> Result<(), String> {
        self.load_group(group)?;
        self.subinterfaces.insert(pointer, (group, kind));
        self.subinterface_changes = self.subinterface_changes.wrapping_add(1);
        self.reset_animations(group);
        self.clear_waiting();
        self.layout(self.viewport[0], self.viewport[1])?;
        self.run_on_load(group);
        if self.root >= 0 {
            self.queue_open_close(self.root, 0);
        }
        Ok(())
    }
    pub fn close_sub(&mut self, pointer: i32, unload: bool) {
        if let Some((group, _)) = self.subinterfaces.remove(&pointer) {
            self.subinterface_changes = self.subinterface_changes.wrapping_add(1);
            if unload {
                let removed: Vec<_> = self
                    .components
                    .iter()
                    .filter(|(key, c)| key.id >> 16 == group && !(key.child == -1 && c.kind == 2))
                    .map(|(&key, _)| key)
                    .collect();
                for key in removed {
                    self.remove_component(key);
                }
                self.created_lengths.retain(|id, _| id >> 16 != group);
                self.drag_parents.retain(|key, _| key.id >> 16 != group);
                self.loaded.remove(&group);
            }
            self.properties.retain(|key, _| key.id >> 16 != group);
            if self.root >= 0 {
                self.queue_open_close(self.root, 0);
            }
        }
    }
    fn queue_open_close(&mut self, group: i32, depth: usize) {
        if depth > 100 {
            self.diagnostic("Cyclic widget open/close hooks".into());
            return;
        }
        let keys: Vec<_> = self
            .components
            .keys()
            .filter(|k| k.child == -1 && k.id >> 16 == group)
            .copied()
            .collect();
        for key in keys {
            if self.components[&key].kind == 0 {
                let created: Vec<_> = self
                    .components
                    .iter()
                    .filter(|(k, c)| {
                        k.id == key.id && k.child >= 0 && c.on_widgets_open_close.is_some()
                    })
                    .map(|(&k, c)| Hook::new(k, c.on_widgets_open_close.clone().unwrap()))
                    .collect();
                self.hooks.extend(created);
                if let Some(&(g, _)) = self.subinterfaces.get(&key.id) {
                    self.queue_open_close(g, depth + 1);
                }
            }
            if let Some(args) = &self.components[&key].on_widgets_open_close {
                self.hooks.push_back(Hook::new(key, args.clone()));
            }
        }
    }
    /// `InterfaceList.runScripts(0, ..)`: dialog-abort hooks through the open tree.
    pub fn queue_dialog_abort(&mut self, group: i32, depth: usize) {
        if depth > 100 {
            self.diagnostic("Cyclic dialog-abort hooks".into());
            return;
        }
        let keys: Vec<_> = self
            .components
            .keys()
            .filter(|k| k.child == -1 && k.id >> 16 == group)
            .copied()
            .collect();
        for key in keys {
            if self.components[&key].kind == 0 {
                let created: Vec<_> = self
                    .components
                    .iter()
                    .filter(|(k, c)| k.id == key.id && k.child >= 0 && c.on_dialog_abort.is_some())
                    .map(|(&k, c)| Hook::new(k, c.on_dialog_abort.clone().unwrap()))
                    .collect();
                self.hooks.extend(created);
                if let Some(&(g, _)) = self.subinterfaces.get(&key.id) {
                    self.queue_dialog_abort(g, depth + 1);
                }
            }
            if let Some(args) = &self.components[&key].on_dialog_abort {
                self.hooks.push_back(Hook::new(key, args.clone()));
            }
        }
    }
    /// WIDGETSTRUCT_SETTING: replaces the target parameter and keeps the events
    /// of an existing server entry (or the component's own for child -1).
    pub fn set_target_param(
        &mut self,
        pointer: i32,
        start: i32,
        end: i32,
        target_param: i32,
    ) -> Result<(), String> {
        if end.wrapping_sub(start) > 65536 {
            return Err("Oversized component properties range".into());
        }
        for child in start..=end {
            let key = Key { id: pointer, child };
            let events = if let Some(prev) = self.properties.get(&key) {
                prev.events
            } else if child == -1 {
                self.get_mut(Key::component(pointer))?.properties.events
            } else {
                0
            };
            self.properties.insert(
                key,
                super::Properties {
                    events,
                    target_param,
                },
            );
        }
        Ok(())
    }
    /// SWITCH_WIDGET: moves the interface open at `source` to `target`.
    pub fn move_sub(&mut self, source: i32, target: i32) -> Result<(), String> {
        let moved = self.subinterfaces.get(&source).copied();
        if let Some(&(group, _)) = self.subinterfaces.get(&target) {
            self.close_sub(target, moved.is_none_or(|(other, _)| other != group));
        }
        if let Some(entry) = moved {
            self.subinterfaces.remove(&source);
            self.subinterfaces.insert(target, entry);
        }
        // Protocol 1742: layoutComponent(target, true) forces the resize
        // hooks of everything below the target component.
        self.resize_scope = Some(target);
        let laid_out = self.layout(self.viewport[0], self.viewport[1]);
        self.resize_scope = None;
        laid_out?;
        if self.root >= 0 {
            self.queue_open_close(self.root, 0);
        }
        Ok(())
    }
    /// Interface messages decoded by `ServerEvent`. Counters update the
    /// verification ID before the change is queued, as `setVerifyId` does.
    pub fn apply_event(
        &mut self,
        event: &rs09_protocol::game::events::ServerEvent,
    ) -> Result<bool, String> {
        use rs09_protocol::game::events::ServerEvent;
        match *event {
            ServerEvent::InterfaceColour {
                pointer,
                colour,
                counter,
            } => {
                self.verify_id = (counter + 1) & 65535;
                self.delayed.server(6, pointer, colour, None);
            }
            ServerEvent::InterfaceScroll {
                pointer,
                position,
                counter,
            } => {
                self.verify_id = (counter + 1) & 65535;
                self.delayed.server(12, pointer, position, None);
            }
            ServerEvent::InterfaceRotation {
                pointer,
                pitch_step,
                yaw_step,
                counter,
            } => {
                self.verify_id = (counter + 1) & 65535;
                self.delayed.server(
                    13,
                    pointer,
                    yaw_step.wrapping_add(pitch_step.wrapping_shl(16)),
                    None,
                );
            }
            ServerEvent::InterfaceBodyHead {
                pointer,
                first,
                second,
                third,
                counter,
            } => {
                self.verify_id = (counter + 1) & 65535;
                self.delayed.server_values(
                    4,
                    pointer,
                    [7, first, second.wrapping_shl(16) | third],
                    None,
                );
            }
            ServerEvent::InterfaceTargetParam {
                pointer,
                start,
                end,
                value,
                counter,
            } => {
                self.verify_id = (counter + 1) & 65535;
                self.set_target_param(pointer, start, end, value)?;
            }
            ServerEvent::MoveInterface {
                source,
                target,
                counter,
            } => {
                self.verify_id = (counter + 1) & 65535;
                self.move_sub(source, target)?;
            }
            ServerEvent::VarcString {
                id,
                ref text,
                counter,
            } => {
                self.verify_id = (counter + 1) & 65535;
                self.delayed.server(2, id, 0, Some(latin1(text)));
            }
            ServerEvent::RunTopLevelScripts => {
                if self.root != -1 {
                    self.queue_dialog_abort(self.root, 0);
                }
            }
            ServerEvent::DeleteInventory { id } => {
                self.containers.remove(&id);
                self.changed_inventory(id);
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
    /// Event queues use the original inventory notification ring and prioritize timers.
    pub fn queue_events(&mut self, nodes: &[DrawNode]) {
        self.queue_input_events(nodes, None);
    }
    /// One cycle of `InterfaceList.processComponents` over the open tree; see
    /// `hook_walk.rs`. The walk follows the component tree itself, as the
    /// original does, so the draw list is not consulted.
    pub fn queue_input_events(&mut self, nodes: &[DrawNode], input: Option<PointerInput>) {
        let _ = nodes;
        self.queue_cycle_events(input, None);
    }
    /// The same cycle with the keyboard, so that component key bindings
    /// click at their place in the walk (`InterfaceList` 687-709).
    pub fn queue_cycle_events(&mut self, input: Option<PointerInput>, keys: Option<KeyInput<'_>>) {
        if let Err(e) = self.process_legacy_input(input) {
            self.diagnostic(e);
        }
        self.world_map.begin_cycle();
        self.walk_hooks(input, keys);
    }
    pub fn diagnostic(&mut self, text: String) {
        if self.diagnostics.back() != Some(&text) {
            self.diagnostics.push_back(text);
            while self.diagnostics.len() > 32 {
                self.diagnostics.pop_front();
            }
        }
    }
    fn changed_inventory(&mut self, id: i32) {
        self.updated_inventories[(self.inventory_writer & 31) as usize] = id & 32767;
        self.inventory_writer = self.inventory_writer.wrapping_add(1);
    }
    fn update_item(
        &mut self,
        pointer: i32,
        container: i32,
        slot: i32,
        item: WireItem,
    ) -> Result<(), String> {
        if !(0..1_000_000).contains(&slot) {
            return Err("Invalid inventory slot".into());
        }
        if pointer >= 0 {
            self.load_group(pointer >> 16)?;
            if let Some(c) = self.components.get_mut(&Key::component(pointer)) {
                if let (Some(ids), Some(amounts)) = (&mut c.obj_types, &mut c.obj_counts) {
                    if let (Some(id), Some(amount)) =
                        (ids.get_mut(slot as usize), amounts.get_mut(slot as usize))
                    {
                        *id = item.id.wrapping_add(1);
                        *amount = item.amount;
                    }
                }
            }
        }
        let items = self
            .containers
            .entry(container)
            .or_insert_with(|| vec![WireItem { id: -1, amount: 0 }]);
        if items.len() <= slot as usize {
            items.resize(slot as usize + 1, WireItem { id: -1, amount: 0 });
        }
        items[slot as usize] = item;
        Ok(())
    }
    pub fn apply_frame(&mut self, frame: &Frame) -> Result<bool, String> {
        let Some(update) = ServerUpdate::decode(frame).map_err(|e| e.to_string())? else {
            return Ok(false);
        };
        self.apply(update)
    }
    pub fn apply(&mut self, update: ServerUpdate) -> Result<bool, String> {
        match update {
            ServerUpdate::InterfaceAnimation {
                pointer,
                sequence,
                counter,
            } => {
                self.verify_id = (counter + 1) & 65535;
                self.delayed.server(5, pointer, sequence, None);
            }
            ServerUpdate::InterfaceModel {
                pointer,
                model,
                counter,
            } => {
                self.verify_id = (counter + 1) & 65535;
                self.delayed.server_values(4, pointer, [1, -1, model], None);
            }
            ServerUpdate::InterfaceNpcHead {
                pointer,
                npc,
                counter,
            } => {
                self.verify_id = (counter + 1) & 65535;
                self.delayed.server_values(4, pointer, [2, -1, npc], None);
            }
            ServerUpdate::InterfaceAngle {
                pointer,
                pitch,
                yaw,
                zoom,
                counter,
            } => {
                self.verify_id = (counter + 1) & 65535;
                self.delayed
                    .server_values(8, pointer, [pitch, zoom, yaw], None);
            }
            ServerUpdate::InterfacePosition {
                pointer,
                x,
                y,
                counter,
            } => {
                self.verify_id = (counter + 1) & 65535;
                self.delayed.server_values(11, pointer, [x, 0, y], None);
            }
            ServerUpdate::InterfaceObject {
                pointer,
                item,
                value,
                counter,
            } => {
                self.verify_id = (counter + 1) & 65535;
                let key = Key::component(pointer);
                if self.get_mut(key)?.if3 {
                    self.delayed
                        .server_values(9, pointer, [item, 0, value], None);
                    let item = self.items.get(&self.pack, item)?;
                    self.delayed.server_values(
                        8,
                        pointer,
                        [item.xAngle2D, item.zoom2d, item.yAngle2D],
                        None,
                    );
                    self.delayed.server_values(
                        10,
                        pointer,
                        [item.xOffset2D, item.zAngle2D, item.yOffset2D],
                        None,
                    );
                } else if item == -1 {
                    self.get_mut(key)?.model_type = 0;
                } else {
                    let item = self.items.get(&self.pack, item)?;
                    let c = self.get_mut(key)?;
                    c.model_x_angle = item.xAngle2D;
                    if value == 0 {
                        return Err("Zero legacy item-model zoom divisor".into());
                    }
                    c.model_zoom = item.zoom2d.wrapping_mul(100).wrapping_div(value);
                    c.model_type = 4;
                    c.model_id = item.id;
                    c.model_y_angle = item.yAngle2D;
                    self.model_redraws = self.model_redraws.wrapping_add(1);
                }
            }
            ServerUpdate::OpenInterface {
                kind,
                pointer,
                counter,
                interface,
            } => {
                self.verify_id = (counter + 1) & 65535;
                self.open_sub(pointer, interface, kind)?;
            }
            ServerUpdate::WindowsPane {
                interface,
                counter,
                reset,
            } => {
                self.verify_id = (counter + 1) & 65535;
                self.world_map.top_level_opened(reset);
                self.open_root(interface)?;
            }
            ServerUpdate::CloseInterface { counter, pointer } => {
                self.verify_id = (counter + 1) & 65535;
                self.close_sub(pointer, true);
                self.clear_waiting();
            }
            ServerUpdate::InterfaceHidden {
                hidden,
                counter,
                pointer,
            } => {
                self.verify_id = (counter + 1) & 65535;
                self.delayed.server(7, pointer, hidden, None);
            }
            ServerUpdate::InterfaceText {
                text,
                counter,
                pointer,
            } => {
                self.verify_id = (counter + 1) & 65535;
                self.delayed.server(3, pointer, 0, Some(latin1(&text)));
            }
            ServerUpdate::Varc { counter, id, value } => {
                self.verify_id = (counter + 1) & 65535;
                self.delayed.server(1, id, value, None);
            }
            ServerUpdate::InterfaceEvents {
                counter,
                pointer,
                start,
                end,
                mask,
            } => {
                self.verify_id = (counter + 1) & 65535;
                self.set_events(pointer, start, end, mask)?;
            }
            ServerUpdate::InventoryClear { pointer } => {
                if let Some(ids) = &mut self.get_mut(Key::component(pointer))?.obj_types {
                    ids.fill(0);
                }
            }
            ServerUpdate::InventoryFull {
                pointer,
                container,
                items,
            } => {
                if pointer >= 0 {
                    self.load_group(pointer >> 16)?;
                    if let Some(c) = self.components.get_mut(&Key::component(pointer)) {
                        if let Some(ids) = &mut c.obj_types {
                            ids.fill(0);
                        }
                        if let Some(amounts) = &mut c.obj_counts {
                            amounts.fill(0);
                        }
                    }
                }
                if let Some(items) = self.containers.get_mut(&container) {
                    items.fill(WireItem { id: -1, amount: 0 });
                }
                for (slot, item) in items.into_iter().enumerate() {
                    self.update_item(pointer, container, slot as i32, item)?;
                }
                self.changed_inventory(container);
            }
            ServerUpdate::InventoryPartial {
                pointer,
                container,
                slots,
            } => {
                for slot in slots {
                    self.update_item(pointer, container, slot.slot, slot.item)?;
                }
                self.changed_inventory(container);
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
    pub fn layout(&mut self, width: i32, height: i32) -> Result<Vec<DrawNode>, String> {
        // A new canvas size is the original's layoutTopLevel(true): sizes
        // that change queue their onResize hooks.
        self.resize_all = self.viewport != [width, height];
        self.resize_inside = 0;
        self.viewport = [width, height];
        let mut out = Vec::new();
        let rect = Rect {
            x: 0,
            y: 0,
            w: width,
            h: height,
        };
        if self.root >= 0 {
            self.layout_group(self.root, rect, rect, &mut out, 0, true)?;
        }
        Ok(out)
    }
    fn layout_group(
        &mut self,
        group: i32,
        parent: Rect,
        clip: Rect,
        out: &mut Vec<DrawNode>,
        depth: usize,
        visible: bool,
    ) -> Result<(), String> {
        let roots: Vec<_> = self
            .components
            .iter()
            .filter(|(k, c)| k.child == -1 && k.id >> 16 == group && c.overlayer == -1)
            .map(|(&k, _)| k)
            .collect();
        for key in roots {
            self.layout_node(key, parent, clip, out, depth, visible)?;
        }
        Ok(())
    }
    fn layout_node(
        &mut self,
        key: Key,
        parent: Rect,
        clip: Rect,
        out: &mut Vec<DrawNode>,
        depth: usize,
        visible: bool,
    ) -> Result<(), String> {
        if depth > 100 {
            return Err("Cyclic interface tree".into());
        }
        let hovered = self.is_hovered(key);
        let c = self.get_mut(key)?;
        // Hidden widgets still participate in InterfaceList.layoutComponents.
        let visible = visible
            && if c.if3 || c.kind == 0 {
                !c.hidden || !c.if3 && hovered
            } else {
                true
            };
        let old_size = [c.width, c.height];
        c.calculate_size(parent.w, parent.h)
            .map_err(|e| e.to_string())?;
        if self.resize_all || self.resize_inside > 0 {
            self.queue_resize(key, old_size);
        }
        let scoped = key.child == -1 && self.resize_scope == Some(key.id);
        let c = self.get_mut(key)?;
        c.calculate_position(parent.w, parent.h);
        c.scroll_x = c.scroll_x.min(c.scroll_max_h - c.width).max(0);
        c.scroll_y = c.scroll_y.min(c.scroll_max_v - c.height).max(0);
        let rect = Rect {
            x: parent.x.wrapping_add(c.x),
            y: parent.y.wrapping_add(c.y),
            w: c.width,
            h: c.height,
        };
        let node_clip = if c.kind == 2 {
            clip
        } else {
            let extra = i32::from(c.kind == 9);
            clip.intersect(Rect {
                w: rect.w.wrapping_add(extra),
                h: rect.h.wrapping_add(extra),
                ..rect
            })
        };
        let is_container = c.kind == 0;
        let scroll = [c.scroll_x, c.scroll_y];
        let content_size = [
            if c.scroll_max_h != 0 {
                c.scroll_max_h
            } else {
                c.width
            },
            if c.scroll_max_v != 0 {
                c.scroll_max_v
            } else {
                c.height
            },
        ];
        let node_index = out.len();
        if visible {
            out.push(DrawNode {
                key,
                rect,
                clip: node_clip,
                paint_clip: clip,
                subtree_end: node_index + 1,
            });
        }
        if is_container {
            let mut children: Vec<_> = self
                .components
                .iter()
                .filter(|(k, c)| {
                    if key.child == -1 {
                        k.id >> 16 == key.id >> 16
                            && c.overlayer == key.id
                            && (k.child == -1 || k.id == key.id)
                    } else {
                        false
                    }
                })
                .map(|(&k, _)| k)
                .filter(|&k| k != key)
                .collect();
            children.sort_by_key(|k| (k.child >= 0, k.id, k.child));
            let content = Rect {
                x: rect.x - scroll[0],
                y: rect.y - scroll[1],
                w: content_size[0],
                h: content_size[1],
            };
            self.resize_inside += u32::from(scoped);
            let mut laid_out = Ok(());
            for child in children {
                laid_out = self.layout_node(child, content, node_clip, out, depth + 1, visible);
                if laid_out.is_err() {
                    break;
                }
            }
            if laid_out.is_ok() && key.child == -1 {
                if let Some(&(group, _)) = self.subinterfaces.get(&key.id) {
                    // layoutComponent 330-339: the open interface is sized
                    // against the host's scroll size, and placed at the
                    // host's own position without its scroll offset.
                    let host = Rect {
                        w: content_size[0],
                        h: content_size[1],
                        ..rect
                    };
                    laid_out = self.layout_group(group, host, node_clip, out, depth + 1, visible);
                }
            }
            self.resize_inside -= u32::from(scoped);
            laid_out?;
        }
        if visible {
            out[node_index].subtree_end = out.len();
        }
        Ok(())
    }
}
