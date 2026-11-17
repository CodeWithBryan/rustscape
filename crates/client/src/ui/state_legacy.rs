//! Legacy hover hierarchy and the original shared Continue waiting reference.
use super::{Key, PointerInput, State};
use rs09_protocol::game::Frame;

struct Node {
    if3: bool,
    hidden: bool,
    kind: i32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    scroll_x: i32,
    scroll_y: i32,
    scroll_max_v: i32,
    hover_overlayer: i32,
    over_color: i32,
}

impl State {
    /// Bounded public diagnostics; this does not drive input or rendering.
    pub fn scrollbar_snapshot(&self) -> serde_json::Value {
        let components: Vec<_> = self.components.iter()
            .filter(|(_, c)| !c.if3 && c.scroll_max_v > c.height)
            .take(128)
            .map(|(key,c)| serde_json::json!({"key":key,"scroll_y":c.scroll_y,"range":c.scroll_max_v,"height":c.height}))
            .collect();
        serde_json::json!({"dragging":self.scrollbar_drag.dragging,"margin":self.scrollbar_drag.margin,"components":components})
    }
    pub(crate) fn lookup_reference(
        &mut self,
        key: Key,
    ) -> Result<Option<crate::ui::component_handles::Handle>, String> {
        self.load_group((key.id as u32 >> 16) as i32)?;
        if key.child < -1 {
            return Err("Invalid widget child".into());
        }
        let parent_exists = self.components.contains_key(&Key::component(key.id));
        let in_array = key.child == -1
            || self
                .created_lengths
                .get(&key.id)
                .is_some_and(|&length| (key.child as usize) < length);
        Ok(
            (parent_exists && in_array && self.components.contains_key(&key))
                .then(|| self.handles.retain(key)),
        )
    }
    pub fn is_hovered(&self, key: Key) -> bool {
        self.legacy_hover
            .as_ref()
            .is_some_and(|h| h.key() == key && self.handles.is_current(h))
    }
    pub fn is_waiting(&self, key: Key) -> bool {
        self.please_wait
            .as_ref()
            .is_some_and(|h| h.key() == key && self.handles.is_current(h))
    }
    pub fn hovered_key(&self) -> Option<Key> {
        self.legacy_hover
            .as_ref()
            .filter(|h| self.handles.is_current(h))
            .map(|h| h.key())
    }
    pub fn waiting_key(&self) -> Option<Key> {
        self.please_wait.as_ref().map(|h| h.key())
    }
    pub fn clear_waiting(&mut self) {
        self.please_wait = None;
    }
    pub fn tooltip_key(&self) -> Option<Key> {
        self.legacy_tooltip
            .as_ref()
            .filter(|h| self.handles.is_current(h))
            .map(|h| h.key())
    }
    pub fn tooltip_timer(&self) -> i32 {
        self.tooltip_timer
    }
    pub fn is_tooltip(&self, key: Key) -> bool {
        self.tooltip_timer == super::super::widget_legacy::TOOLTIP_DELAY
            && self.tooltip_key() == Some(key)
    }
    /// Called after the cycle's input hooks, matching Protocol's timer phase.
    /// Moving directly between tooltip objects keeps the shared delay counter.
    pub fn finish_input_events(&mut self) {
        self.tooltip_timer = super::super::widget_legacy::tooltip_tick(
            self.tooltip_timer,
            self.legacy_tooltip.is_some(),
        );
        self.world_map.end_cycle();
    }
    /// MiniMenu sends before resolving the component. Missing native assets are
    /// diagnosed after preserving that wire prefix; a null target does not wait.
    pub fn request_continue(&mut self, key: Key) -> Option<Frame> {
        if self.please_wait.is_some() {
            return None;
        }
        let packet = crate::ui::actions::item_packet(41, 0, key.child, key.id);
        match self.lookup_reference(key) {
            Ok(handle) => self.please_wait = handle,
            Err(e) => self.diagnostic(e),
        }
        packet
    }
    /// This legacy-only walk uses the current hover reference while traversing
    /// the complete already-laid-out hierarchy. The prior visible draw list can
    /// omit a newly hovered hidden pane or retain one from the previous tick.
    pub(super) fn process_legacy_input(
        &mut self,
        input: Option<PointerInput>,
    ) -> Result<(), String> {
        self.legacy_hover = None;
        self.legacy_tooltip = None;
        if let Some(input) = input.filter(|p| !p.suppressed) {
            if self.root >= 0 {
                self.legacy_group(
                    self.root,
                    [0, 0],
                    [0, 0, self.viewport[0], self.viewport[1]],
                    input,
                    0,
                )?;
            }
        }
        Ok(())
    }
    fn legacy_group(
        &mut self,
        group: i32,
        origin: [i32; 2],
        clip: [i32; 4],
        input: PointerInput,
        depth: usize,
    ) -> Result<(), String> {
        self.load_group(group)?;
        let roots: Vec<_> = self
            .components
            .iter()
            .filter(|(k, c)| k.child == -1 && k.id >> 16 == group && c.overlayer == -1)
            .map(|(&k, _)| k)
            .collect();
        for key in roots {
            self.legacy_node(key, origin, clip, input, depth)?;
        }
        Ok(())
    }
    fn legacy_node(
        &mut self,
        key: Key,
        origin: [i32; 2],
        clip: [i32; 4],
        input: PointerInput,
        depth: usize,
    ) -> Result<(), String> {
        if depth > 100 {
            return Err("Cyclic legacy interface tree".into());
        }
        let mut c = {
            let c = &self.components[&key];
            Node {
                if3: c.if3,
                hidden: c.hidden,
                kind: c.kind,
                x: c.x,
                y: c.y,
                width: c.width,
                height: c.height,
                scroll_x: c.scroll_x,
                scroll_y: c.scroll_y,
                scroll_max_v: c.scroll_max_v,
                hover_overlayer: c.hover_overlayer,
                over_color: c.over_color,
            }
        };
        let point = input.point;
        if c.if3 && c.hidden {
            return Ok(());
        }
        if !c.if3 && c.kind == 0 && c.hidden && !self.is_hovered(key) {
            return Ok(());
        }
        let x = origin[0].wrapping_add(c.x);
        let y = origin[1].wrapping_add(c.y);
        let bounds = if c.kind == 2 {
            clip
        } else {
            let extra = i32::from(c.kind == 9);
            [
                clip[0].max(x),
                clip[1].max(y),
                clip[2].min(x.wrapping_add(c.width).wrapping_add(extra)),
                clip[3].min(y.wrapping_add(c.height).wrapping_add(extra)),
            ]
        };
        if c.if3 && (bounds[0] >= bounds[2] || bounds[1] >= bounds[3]) {
            return Ok(());
        }
        if !c.if3
            && (c.hover_overlayer >= 0 || c.over_color != 0)
            && point[0] >= bounds[0]
            && point[0] < bounds[2]
            && point[1] >= bounds[1]
            && point[1] < bounds[3]
        {
            let target = if c.hover_overlayer < 0 {
                key
            } else if key.child >= 0 {
                Key {
                    id: key.id,
                    child: c.hover_overlayer,
                }
            } else {
                Key::component((key.id & -65536).wrapping_add(c.hover_overlayer))
            };
            self.legacy_hover = if self.components.contains_key(&target) {
                Some(self.retain_component(target)?)
            } else {
                None
            };
        }
        if !c.if3
            && c.kind == 8
            && point[0] >= bounds[0]
            && point[0] < bounds[2]
            && point[1] >= bounds[1]
            && point[1] < bounds[3]
        {
            self.legacy_tooltip = Some(self.retain_component(key)?);
        }
        if !c.if3 && c.scroll_max_v > c.height {
            let target = self.components.get_mut(&key).unwrap();
            self.scrollbar_drag.handle(
                &mut target.scroll_y,
                super::super::scrollbar::Input {
                    point,
                    bar: [x.wrapping_add(c.width), y],
                    height: c.height,
                    range: c.scroll_max_v,
                    width: c.width,
                    pressed: input.any_down,
                    wheel: input.scroll,
                },
                || {}, // This presentation redraws every frame.
            )?;
            c.scroll_y = target.scroll_y;
        }
        if c.kind == 0 {
            let mut children: Vec<_> = self
                .components
                .iter()
                .filter(|(k, child)| {
                    key.child == -1
                        && k.id >> 16 == key.id >> 16
                        && child.overlayer == key.id
                        && (k.child == -1 || k.id == key.id)
                })
                .map(|(&k, _)| k)
                .filter(|&k| k != key)
                .collect();
            // RT4 visits the static array before the created-component array.
            children.sort_by_key(|k| (k.child >= 0, k.id, k.child));
            let child_origin = [x.wrapping_sub(c.scroll_x), y.wrapping_sub(c.scroll_y)];
            for child in children {
                self.legacy_node(child, child_origin, bounds, input, depth + 1)?;
            }
            if key.child == -1 {
                if let Some(&(group, _)) = self.subinterfaces.get(&key.id) {
                    self.legacy_group(group, [x, y], bounds, input, depth + 1)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "state_legacy_tests.rs"]
pub(super) mod legacy_tests;
#[cfg(test)]
#[path = "state_legacy_wait_tests.rs"]
mod wait_tests;
