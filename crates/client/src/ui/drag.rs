//! IF1 inventory rearrangement and IF3 hook-driven drag gestures.
use super::state::{DrawNode, Hook, Key, Rect, State};
use crate::interaction::MenuEntry;
use rs09_protocol::{buffer::Writer, game::Frame};

pub fn inventory_packet(component: i32, source: i32, target: i32, inserting: i32) -> Frame {
    let mut w = Writer::new();
    w.p2(target);
    w.ip4(component);
    w.p2add(source);
    w.p1sub(inserting);
    Frame {
        opcode: 231,
        payload: w.into_vec(),
    }
}
pub fn component_packet(source: Key, target: Key) -> Frame {
    let mut w = Writer::new();
    w.mp4(source.id);
    w.ip2(target.child);
    w.p4(target.id);
    w.ip2(source.child);
    Frame {
        opcode: 79,
        payload: w.into_vec(),
    }
}
#[derive(Debug)]
pub enum Release {
    Inventory(Frame),
    Component { source: Key, target: Key },
}
impl Release {
    /// The callback can change access masks or remove the drag parent.
    pub fn resolve(self, state: &State) -> Option<Frame> {
        match self {
            Self::Inventory(frame) => Some(frame),
            Self::Component { source, target } => state
                .drag_render_parent(source)
                .map(|_| component_packet(source, target)),
        }
    }
}
/// These edits affect the legacy widget arrays only, as in Protocol's drag path.
/// The authoritative item container is updated only by server packets.
pub fn rearrange(
    state: &mut State,
    key: Key,
    source: i32,
    target: i32,
    inserting: bool,
) -> Result<Frame, String> {
    let replace = state.properties(key)?.events & (1 << 29) != 0;
    let c = state.get_mut(key)?;
    let ids = c.obj_types.as_mut().ok_or("Missing inventory item IDs")?;
    let counts = c
        .obj_counts
        .as_mut()
        .ok_or("Missing inventory item counts")?;
    if source < 0
        || target < 0
        || source as usize >= ids.len()
        || target as usize >= ids.len()
        || counts.len() != ids.len()
    {
        return Err("Invalid inventory drag slots".into());
    }
    let mode = i32::from(inserting && c.client_code == 206 && ids[source as usize] > 0);
    let (source, target) = (source as usize, target as usize);
    if replace {
        ids[source] = ids[target];
        counts[source] = counts[target];
        ids[target] = -1;
        counts[target] = 0;
    } else if mode == 1 {
        let mut current = source;
        while current != target {
            let next = if current > target {
                current - 1
            } else {
                current + 1
            };
            ids.swap(current, next);
            counts.swap(current, next);
            current = next;
        }
    } else {
        ids.swap(source, target);
        counts.swap(source, target);
    }
    Ok(inventory_packet(key.id, source as i32, target as i32, mode))
}
struct Gesture {
    key: Key,
    parent: Option<Key>,
    slot: Option<i32>,
    rect: Rect,
    press: [i32; 2],
    start: i32,
    last_tick: i32,
    dragging: bool,
    click: Option<MenuEntry>,
}
#[derive(Default)]
pub struct Controller {
    gesture: Option<Gesture>,
    pub visual: Option<DrawNode>,
    pub inventory_visual: Option<InventoryVisual>,
    last_inventory_presented: Option<InventoryVisual>,
    inventory_released: bool,
}
#[derive(Clone, Copy)]
pub struct InventoryVisual {
    pub key: Key,
    pub slot: i32,
    pub point: [i32; 2],
    pub press: [i32; 2],
    pub cycles: i32,
    pub source_at: Option<[i32; 2]>,
    pub drawn_at: Option<[i32; 2]>,
    pub scroll_delta: i32,
}
impl Controller {
    /// Protocol assigns10 after the released inventory's menu action finishes.
    pub fn take_inventory_release(&mut self) -> bool {
        std::mem::take(&mut self.inventory_released)
    }
    /// Rendering may scroll the parent after drawing the icon; compensate the
    /// captured press position before the next layout/input iteration.
    pub fn inventory_presented(&mut self, visual: Option<InventoryVisual>) {
        if let (Some(g), Some(v)) = (&mut self.gesture, visual)
            && g.key == v.key
            && g.slot == Some(v.slot)
        {
            g.press = v.press;
            self.last_inventory_presented = Some(v);
        }
    }
    /// Public bounded diagnostics used by native captures and browser review.
    pub fn snapshot(&self, state: Option<&State>) -> serde_json::Value {
        let last = self.last_inventory_presented.map(|v|serde_json::json!({"key":v.key,"slot":v.slot,"point":v.point,"press":v.press,"cycles":v.cycles,"source_at":v.source_at,"drawn_at":v.drawn_at,"scroll_delta":v.scroll_delta}));
        let key = self
            .gesture
            .as_ref()
            .map(|g| g.key)
            .or_else(|| self.last_inventory_presented.map(|v| v.key));
        let widget=key.and_then(|key|state?.components.get(&key).map(|c|serde_json::json!({"key":key,"ids":c.obj_types.as_ref().map(|a|a.iter().take(128).collect::<Vec<_>>()),"counts":c.obj_counts.as_ref().map(|a|a.iter().take(128).collect::<Vec<_>>()),"truncated":c.obj_types.as_ref().is_some_and(|a|a.len()>128)})));
        serde_json::json!({"active":self.active(),"press":self.gesture.as_ref().map(|g|g.press),"slot":self.gesture.as_ref().and_then(|g|g.slot),"last_presentation":last,"widget":widget})
    }
    pub fn active(&self) -> bool {
        self.gesture.is_some()
    }
    /// The drag under way is of an inventory item (`InterfaceList.
    /// clickedInventoryComponent`), not of a component (`Cs1ScriptRunner.
    /// draggedComponent`).
    pub fn inventory(&self) -> bool {
        self.gesture
            .as_ref()
            .is_some_and(|gesture| gesture.slot.is_some())
    }
    /// `Cs1ScriptRunner.startComponentDrag` called by a script (CS2 3109):
    /// drag `key` as if it had been pressed at `offset` inside it. Nothing
    /// happens while another drag or the menu is active, or when the
    /// component has no drag parent or is not laid out.
    pub fn begin_scripted(
        &mut self,
        state: &State,
        nodes: &[DrawNode],
        key: Key,
        offset: [i32; 2],
        cycle: i32,
        menu_open: bool,
    ) -> bool {
        if self.active() || menu_open {
            return false;
        }
        let Some(parent) = state
            .drag_render_parent(key)
            .or_else(|| state.drag_parents.get(&key).copied())
        else {
            return false;
        };
        let Some(node) = nodes.iter().find(|node| node.key == key) else {
            return false;
        };
        self.gesture = Some(Gesture {
            key,
            parent: Some(parent),
            slot: None,
            rect: node.rect,
            press: [node.rect.x + offset[0], node.rect.y + offset[1]],
            start: cycle,
            last_tick: cycle,
            dragging: false,
            click: None,
        });
        true
    }
    /// `Cs1ScriptRunner.startComponentDrag` as the interface walk calls it
    /// for a clicked component (InterfaceList.java:710-712), where nothing
    /// else claims the click (the title's `client.mainUpdate` has no
    /// `processDefaultAction`): drag `key`, pressed at `click`. The step of
    /// the same cycle is its first update (`dragElapsedTicks` 1).
    pub fn begin_walked(
        &mut self,
        state: &State,
        nodes: &[DrawNode],
        key: Key,
        click: [i32; 2],
        cycle: i32,
    ) -> bool {
        let Some(node) = nodes.iter().find(|node| node.key == key) else {
            return false;
        };
        let offset = [click[0] - node.rect.x, click[1] - node.rect.y];
        if !self.begin_scripted(state, nodes, key, offset, cycle, false) {
            return false;
        }
        if let Some(gesture) = &mut self.gesture {
            gesture.last_tick = cycle.wrapping_sub(1);
        }
        true
    }
    pub fn begin(
        &mut self,
        state: &State,
        nodes: &[DrawNode],
        point: [i32; 2],
        cycle: i32,
        click: Option<MenuEntry>,
    ) -> bool {
        if self.active() {
            return true;
        }
        let inventory = click
            .as_ref()
            .filter(|e| {
                matches!(
                    e.action,
                    25 | 23 | 48 | 7 | 13 | 47 | 5 | 43 | 35 | 58 | 22 | 1006
                )
            })
            .and_then(|entry| {
                let key = Key::component(entry.args[1]);
                (state.properties(key).ok()?.events & (3 << 28) != 0).then(|| (key, entry.args[0]))
            });
        let candidate = if let Some((key, slot)) = inventory {
            state
                .inventory_slot_at(nodes, point)
                .filter(|v| v.0 == key && v.1 == slot)
                .map(|(_, _, rect)| (key, None, Some(slot), rect))
        } else {
            // The component the original's walk would start dragging,
            // including its noClickThrough rule (InterfaceList 666-676).
            super::state::drag_source(state, nodes, point)
                .map(|(key, parent, rect)| (key, Some(parent), None, rect))
        };
        if let Some((key, parent, slot, rect)) = candidate {
            self.gesture = Some(Gesture {
                key,
                parent,
                slot,
                rect,
                press: point,
                start: cycle,
                last_tick: cycle,
                dragging: false,
                click,
            });
            true
        } else {
            false
        }
    }
    /// Hooks are returned in dispatch order so their script side effects run before
    /// the caller builds the post-release packet.
    pub fn step(
        &mut self,
        state: &mut State,
        nodes: &[DrawNode],
        point: [i32; 2],
        cycle: i32,
        down: bool,
        inserting: bool,
    ) -> (Vec<Hook>, Option<MenuEntry>, Option<Release>) {
        self.inventory_released = false;
        self.visual = None;
        self.inventory_visual = None;
        let Some(g) = self.gesture.as_mut() else {
            return (vec![], None, None);
        };
        let mut hooks = Vec::new();
        let mut packet = None;
        let delta = [point[0] - g.press[0], point[1] - g.press[1]];
        if let Some(slot) = g.slot {
            g.dragging |= delta.iter().any(|v| v.abs() > 5);
            if down {
                self.inventory_visual = Some(InventoryVisual {
                    key: g.key,
                    slot,
                    point,
                    press: g.press,
                    cycles: cycle.wrapping_sub(g.start),
                    source_at: None,
                    drawn_at: None,
                    scroll_delta: 0,
                });
                return (hooks, None, None);
            }
            self.inventory_released = true;
            if g.dragging && cycle.wrapping_sub(g.start) >= 5 {
                if let Some((key, target, _)) = state.inventory_slot_at(nodes, point)
                    && key == g.key
                    && target != slot
                {
                    // Original helper order is hover slot then pressed slot.
                    match rearrange(state, key, target, slot, inserting) {
                        Ok(frame) => packet = Some(Release::Inventory(frame)),
                        Err(e) => state.diagnostic(e),
                    }
                }
            } else {
                let click = g.click.clone();
                self.gesture = None;
                return (hooks, click, None);
            }
        } else {
            let Some(parent) = g.parent.and_then(|p| nodes.iter().find(|n| n.key == p)) else {
                self.gesture = None;
                return (hooks, None, None);
            };
            let Some(source) = nodes.iter().find(|n| n.key == g.key) else {
                self.gesture = None;
                return (hooks, None, None);
            };
            let Ok(c) = state.get(g.key) else {
                self.gesture = None;
                return (hooks, None, None);
            };
            let mut x = g.rect.x + delta[0];
            let mut y = g.rect.y + delta[1];
            if x < parent.rect.x {
                x = parent.rect.x;
            }
            if parent.rect.x + parent.rect.w < x + source.rect.w {
                x = parent.rect.x + parent.rect.w - source.rect.w;
            }
            if y < parent.rect.y {
                y = parent.rect.y;
            }
            if parent.rect.y + parent.rect.h < y + source.rect.h {
                y = parent.rect.y + parent.rect.h - source.rect.h;
            }
            let movement = [x - source.rect.x, y - source.rect.y];
            let moved = movement.iter().any(|v| v.abs() > c.drag_deadzone);
            let p = &state.components[&parent.key];
            let mouse = [
                x + p.scroll_x - parent.rect.x,
                y + p.scroll_y - parent.rect.y,
            ];
            let target = nodes
                .iter()
                .rfind(|n| {
                    n.key != g.key
                        && n.rect.contains(point)
                        && n.clip.contains(point)
                        && state
                            .properties(n.key)
                            .is_ok_and(|p| p.events & (1 << 21) != 0)
                })
                .map(|n| n.key);
            for tick in g.last_tick.wrapping_add(1)..=cycle {
                g.dragging |= tick.wrapping_sub(g.start).wrapping_add(1) > c.drag_deadtime && moved;
                if g.dragging
                    && let Some(args) = &c.on_drag_start
                {
                    let mut hook = Hook::new(g.key, args.clone());
                    hook.mouse = mouse;
                    hooks.push(hook);
                }
            }
            if g.dragging {
                self.visual = Some(DrawNode {
                    key: g.key,
                    rect: Rect {
                        x,
                        y,
                        ..source.rect
                    },
                    clip: parent.clip,
                    paint_clip: parent.clip,
                    subtree_end: 0,
                });
            }
            g.last_tick = cycle;
            if down {
                return (hooks, None, None);
            }
            if g.dragging {
                if let Some(args) = &c.on_drag_release {
                    let mut hook = Hook::new(g.key, args.clone());
                    hook.mouse = mouse;
                    hook.target = target;
                    hooks.push(hook);
                }
                if let Some(target) = target {
                    packet = Some(Release::Component {
                        source: g.key,
                        target,
                    });
                }
            } else {
                let click = g.click.clone();
                self.gesture = None;
                return (hooks, click, None);
            }
        }
        self.gesture = None;
        self.visual = None;
        (hooks, None, packet)
    }
}
