//! `InterfaceList.processComponents` (628-1071): the per-cycle walk over the
//! open interface tree that queues IF3 hook requests, plus the event sources
//! around it (`calculateSize`'s `onResize`, `resetAnimations`).
//!
//! The walk follows the component tree with the geometry the last layout
//! stored, exactly as the original does, rather than a draw list:
//!
//! * a component is visited only when its array and overlayer match, so the
//!   children of a skipped layer are never reached;
//! * an IF3 component whose clipped rectangle is empty is skipped with its
//!   whole subtree (line 661), and so is a hidden one (632);
//! * an IF1 layer that is hidden is entered only while it is the hovered
//!   component, judged by the hover found so far in this same walk (663);
//! * every request of one component is queued before the next component is
//!   looked at, in the original's order (721-1040).
//!
//! Component key bindings (687-709) click through [`KeyInput`] at their place
//! in the walk; the slot rules are `input::bindings`. The world map
//! components 1400 and 1401 (677, 731, 750-789) hand their mouse to
//! `world_map_host::Host`. Of the drag bookkeeping only the
//! start is tracked (710-712), because a click that starts a drag takes the
//! mouse away from every component after it; the drag controller makes the
//! same decision from the draw list with [`drag_source`]. The IF1 tooltip
//! and scrollbars are applied by `state_legacy.rs`, which walks the same tree
//! first; this walk re-derives the IF1 hover target, which line 663 needs
//! as it stands part-way through the walk.
use super::{Hook, Key, PointerInput, State};
use crate::ui::HookArg;

/// The three request lists of one walk. `low` remembers the original's
/// `cancelOnMouseExit` flag, which a `noClickThrough` layer under the mouse
/// uses to throw requests away again.
#[derive(Default)]
struct Queues<'a> {
    /// The keyboard for component key bindings, when the caller has one.
    keys: Option<KeyInput<'a>>,
    high: Vec<Hook>,
    medium: Vec<Hook>,
    low: Vec<(Hook, bool)>,
    /// `InterfaceList.hoveredComponent` as this walk has set it so far.
    hover: Option<Key>,
    /// `Cs1ScriptRunner.draggedComponent` when a click in this walk began
    /// the drag (`dragElapsedTicks == 0`).
    dragged: Option<Key>,
}

/// What component key bindings (687-709) read and do: `Keyboard.pressedKeys`,
/// `client.loop`, and `ClientProt.sendButtonClick("", -1, op, id)`, which
/// runs during the walk, before any request the walk queues.
pub struct KeyInput<'a> {
    pub pressed: &'a [bool; 112],
    pub cycle: i32,
    pub click: &'a mut dyn FnMut(&mut State, Key, i32),
}

/// What the mouse does this cycle, in the original's terms.
#[derive(Clone, Copy)]
struct Mouse {
    /// `Mouse.lastMouseX/Y`.
    point: [i32; 2],
    /// `Mouse.clickX/Y`: a click is tested where it was (684), and its
    /// `onClickRepeat` request is made relative to it (803-804).
    click: [i32; 2],
    /// `Mouse.pressedButton == 1`.
    down: bool,
    /// `Mouse.clickButton == 1`.
    clicked: bool,
    /// `MouseWheel.wheelRotation`.
    wheel: i32,
    /// A component or inventory drag is in progress, or the menu is open
    /// (line 731).
    suppressed: bool,
    /// Whether the IF1 hover rules run at all (line 1043).
    legacy: bool,
}

impl State {
    /// `InterfaceList.resetAnimations`: every static component of the group
    /// starts its model animation again.
    pub(super) fn reset_animations(&mut self, group: i32) {
        for (key, c) in self
            .components
            .range_mut(Key::component(group << 16)..Key::component((group + 1) << 16))
        {
            if key.child == -1 {
                c.seq_next_frame = 1;
                c.seq_frame = 0;
                c.seq_cycle = 0;
            }
        }
    }

    /// The tail of `calculateSize` (536-541): a forced layout that changes a
    /// component's size queues its `onResize` hook at low priority.
    pub(super) fn queue_resize(&mut self, key: Key, old: [i32; 2]) {
        let Some(c) = self.components.get(&key) else {
            return;
        };
        if old == [c.width, c.height] {
            return;
        }
        if let Some(arguments) = c.on_resize.clone() {
            let mut hook = Hook::new(key, arguments);
            if key.child >= 0 {
                hook.lease = Some(self.handles.retain(key));
            }
            self.resize_hooks.push(hook);
        }
    }

    /// Whether a request may still run (`Protocol.loop` 2994-3017): one made
    /// for a created component is dropped once that slot holds a different
    /// component or none.
    pub fn hook_is_current(&self, hook: &Hook) -> bool {
        hook.lease
            .as_ref()
            .is_none_or(|lease| self.handles.is_current(lease))
    }

    /// The low-priority requests queued by forced layouts and not yet handed
    /// to the script queue.
    pub fn pending_resize_hooks(&self) -> &[Hook] {
        &self.resize_hooks
    }

    /// One cycle's `processSubInterface(0, 0, 0, canvasWidth, topLevel, 0,
    /// canvasHeight)` followed by `transmitTimer++`. The requests go to the
    /// script queue timers first, then releases and mouse-leaves, then the
    /// rest, with the resize requests of earlier forced layouts ahead of
    /// this walk's low-priority ones, as in the original's shared list.
    pub(super) fn walk_hooks(&mut self, input: Option<PointerInput>, keys: Option<KeyInput<'_>>) {
        let mouse = match input {
            Some(input) => Mouse {
                point: input.point,
                click: input.click.unwrap_or(input.point),
                down: input.down,
                clicked: input.clicked,
                wheel: input.scroll,
                suppressed: input.suppressed,
                legacy: !input.suppressed,
            },
            // No pointer: the original's mouse outside the window, at (-1, -1).
            None => Mouse {
                point: [-1, -1],
                click: [-1, -1],
                down: false,
                clicked: false,
                wheel: 0,
                suppressed: false,
                legacy: false,
            },
        };
        let mut queues = Queues {
            keys,
            ..Queues::default()
        };
        if self.root >= 0 {
            let clip = [0, 0, self.viewport[0], self.viewport[1]];
            self.walk_interface(self.root, [0, 0], clip, mouse, &mut queues, 0);
        }
        self.drag_candidate = queues.dragged;
        if mouse.legacy {
            // The legacy walk does not know about a drag begun part-way
            // through; the hover found here does (line 1043).
            self.legacy_hover = queues.hover.map(|key| self.handles.retain(key));
        }
        self.key_events.clear();
        self.transmit_timer = self.transmit_timer.wrapping_add(1);
        self.hooks.extend(queues.high);
        self.hooks.extend(queues.medium);
        let resized = std::mem::take(&mut self.resize_hooks);
        self.hooks.extend(resized);
        self.hooks
            .extend(queues.low.into_iter().map(|(hook, _)| hook));
    }

    /// `processComponents(components[interfaceId], -1, ...)`.
    fn walk_interface(
        &mut self,
        group: i32,
        origin: [i32; 2],
        clip: [i32; 4],
        mouse: Mouse,
        queues: &mut Queues<'_>,
        depth: usize,
    ) {
        if let Err(error) = self.load_group(group) {
            self.diagnostic(error);
            return;
        }
        let roots: Vec<Key> = self
            .components
            .range(Key::component(group << 16)..Key::component((group + 1) << 16))
            .filter(|(key, c)| key.child == -1 && c.overlayer == -1)
            .map(|(&key, _)| key)
            .collect();
        for key in roots {
            self.walk_component(key, origin, clip, mouse, queues, depth);
        }
    }

    fn walk_component(
        &mut self,
        key: Key,
        origin: [i32; 2],
        clip: [i32; 4],
        mouse: Mouse,
        queues: &mut Queues<'_>,
        depth: usize,
    ) {
        if depth > 100 {
            self.diagnostic("Cyclic interface tree".into());
            return;
        }
        let lease = (key.child >= 0).then(|| self.handles.retain(key));
        let events = self.properties.get(&key).map(|p| p.events);
        // Cs1ScriptRunner.getDragParent, needed only on a click.
        let draggable = mouse.clicked
            && (self.drag_render_parent(key).is_some() || self.drag_parents.contains_key(&key));
        let Some(c) = self.components.get_mut(&key) else {
            return;
        };
        let events = events.unwrap_or(c.properties.events);
        // 632: IF3 components without anything to process are not visited.
        if c.if3 && !(c.kind == 0 || c.has_event_handlers || events != 0 || c.client_code == 1338) {
            return;
        }
        if c.if3 && c.hidden {
            return;
        }
        let x = c.x.wrapping_add(origin[0]);
        let y = c.y.wrapping_add(origin[1]);
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
        // 661: an IF3 component clipped to nothing ends here, children too.
        if c.if3 && !(bounds[0] < bounds[2] && bounds[1] < bounds[3]) {
            return;
        }
        let within = |point: [i32; 2]| {
            point[0] >= bounds[0]
                && point[1] >= bounds[1]
                && point[0] < bounds[2]
                && point[1] < bounds[3]
        };
        let inside = within(mouse.point);
        if c.kind == 0 {
            if !c.if3 && c.hidden && queues.hover != Some(key) {
                return;
            }
            if c.no_click_through && inside {
                // 667-672: requests that only make sense under the mouse are
                // withdrawn, and their components forget the mouse silently.
                let (cancelled, kept): (Vec<_>, Vec<_>) =
                    queues.low.drain(..).partition(|(_, cancel)| *cancel);
                queues.low = kept;
                for (hook, _) in cancelled {
                    if let Some(source) = self.components.get_mut(&hook.source) {
                        source.mouse_over = false;
                    }
                }
                // 673-676: a drag that began earlier in this walk is undone.
                queues.dragged = None;
                // 677: and so is the world map's.
                self.world_map.blocked_by_layer();
            }
        }
        // 687-709: an IF3 component's key bindings click before anything
        // else about it is decided. A click's script may replace or delete
        // the component; the original carries on with the object it holds,
        // this walk with whatever the slot holds now, and stops if it holds
        // nothing.
        let c = &self.components[&key];
        if c.if3
            && c.key_codes.is_some()
            && let Some(keys) = queues.keys.as_mut()
        {
            crate::input::bindings::component(self, key, keys.pressed, keys.cycle, keys.click);
        }
        let Some(c) = self.components.get_mut(&key) else {
            return;
        };
        if c.if3 {
            let mut hovered = inside;
            let mut pressed = mouse.down && hovered;
            let mut clicked = mouse.clicked && within(mouse.click);
            // 710-712 with startComponentDrag's own conditions: no drag in
            // progress, the menu closed, and a drag parent to move within.
            if clicked && draggable && !mouse.suppressed && queues.dragged.is_none() {
                queues.dragged = Some(key);
            }
            if c.has_event_handlers || c.client_code != 0 {
                let relative = [
                    mouse.point[0].wrapping_sub(x),
                    mouse.point[1].wrapping_sub(y),
                ];
                let request = |arguments: &Option<Vec<HookArg>>, at: [i32; 2]| {
                    arguments.as_ref().map(|arguments| {
                        let mut hook = Hook::new(key, arguments.clone());
                        hook.mouse = at;
                        hook.lease = lease.clone();
                        hook
                    })
                };
                if hovered && mouse.wheel != 0 {
                    // 723-730: the wheel request is made before the mouse
                    // is taken away from the component below.
                    if let Some(hook) = request(&c.on_scroll, [0, mouse.wheel]) {
                        queues.low.push((hook, true));
                    }
                }
                // 731: a component or inventory drag, the open menu, or the
                // world map's own drag (for every component but the map).
                if mouse.suppressed
                    || queues.dragged.is_some()
                    || c.client_code != 1400 && self.world_map.dragging()
                {
                    clicked = false;
                    pressed = false;
                    hovered = false;
                }
                // 737-796: these components have their own handling and none
                // of the generic hooks; `continue` also skips their children.
                match c.client_code {
                    1400 => {
                        self.world_map.map_input(
                            [x, y],
                            [c.width, c.height],
                            mouse.point,
                            clicked,
                            pressed,
                        );
                        return;
                    }
                    1401 => {
                        self.world_map.overview_input(
                            [x, y],
                            [c.width, c.height],
                            mouse.point,
                            pressed,
                        );
                        return;
                    }
                    1337 | 1338 | 1402 => return,
                    _ => {}
                }
                if !c.dragging && clicked {
                    c.dragging = true;
                    let at = [
                        mouse.click[0].wrapping_sub(x),
                        mouse.click[1].wrapping_sub(y),
                    ];
                    if let Some(hook) = request(&c.on_click_repeat, at) {
                        queues.low.push((hook, true));
                    }
                }
                if c.dragging
                    && pressed
                    && let Some(hook) = request(&c.on_drag, relative)
                {
                    queues.low.push((hook, true));
                }
                if c.dragging && !pressed {
                    c.dragging = false;
                    if let Some(hook) = request(&c.on_release, relative) {
                        queues.medium.push(hook);
                    }
                }
                if pressed && let Some(hook) = request(&c.on_hold, relative) {
                    queues.low.push((hook, true));
                }
                if !c.mouse_over && hovered {
                    c.mouse_over = true;
                    if let Some(hook) = request(&c.on_mouse_over, relative) {
                        queues.low.push((hook, true));
                    }
                }
                if c.mouse_over
                    && hovered
                    && let Some(hook) = request(&c.on_mouse_repeat, relative)
                {
                    queues.low.push((hook, true));
                }
                if c.mouse_over && !hovered {
                    c.mouse_over = false;
                    if let Some(hook) = request(&c.on_mouse_leave, relative) {
                        queues.medium.push(hook);
                    }
                }
                if let Some(hook) = request(&c.on_timer, [0, 0]) {
                    queues.high.push(hook);
                }
                // 879-993: a transmit hook runs when the shared 32-entry ring
                // moved past this component's reader and either it has no
                // trigger list, more than 32 entries went by, or one of the
                // new entries is on its list.
                for (arguments, triggers, reader, ring, writer) in [
                    (
                        &c.on_varc_transmit,
                        &c.varc_triggers,
                        &mut c.updated_varcs_reader_index,
                        &self.updated_varcs,
                        self.varc_writer,
                    ),
                    (
                        &c.on_varcstr_transmit,
                        &c.varcstr_triggers,
                        &mut c.updated_varcstrs_reader_index,
                        &self.updated_varcstrs,
                        self.varcstr_writer,
                    ),
                    (
                        &c.on_varp_transmit,
                        &c.varp_triggers,
                        &mut c.updated_varps_reader_index,
                        &self.updated_varps,
                        self.varp_writer,
                    ),
                    (
                        &c.on_inv_transmit,
                        &c.inventory_triggers,
                        &mut c.updated_inventories_reader_index,
                        &self.updated_inventories,
                        self.inventory_writer,
                    ),
                    (
                        &c.on_stat_transmit,
                        &c.stat_triggers,
                        &mut c.updated_stats_reader_index,
                        &self.updated_stats,
                        self.stat_writer,
                    ),
                ] {
                    if arguments.is_some() && writer > *reader {
                        let changed = triggers.as_ref().is_none_or(|triggers| {
                            writer.wrapping_sub(*reader) > 32
                                || (*reader..writer)
                                    .any(|i| triggers.contains(&ring[(i & 31) as usize]))
                        });
                        if changed && let Some(hook) = request(arguments, [0, 0]) {
                            queues.low.push((hook, false));
                        }
                        *reader = writer;
                    }
                }
                for (stamp, arguments) in [
                    (self.chat_transmit_at, &c.on_msg),
                    (self.friend_transmit_at, &c.on_friend_transmit),
                    (self.clan_transmit_at, &c.on_clan_transmit),
                    (self.stock_transmit_at, &c.on_stock_transmit),
                    (self.misc_transmit_at, &c.on_misc_transmit),
                ] {
                    if stamp > c.last_transmit_timer
                        && let Some(hook) = request(arguments, [0, 0])
                    {
                        queues.low.push((hook, false));
                    }
                }
                c.last_transmit_timer = self.transmit_timer;
                for &[key_code, key_char] in &self.key_events {
                    if let Some(mut hook) = request(&c.on_key, [0, 0]) {
                        hook.key_code = key_code;
                        hook.key_char = key_char;
                        queues.low.push((hook, false));
                    }
                }
                if self.spline_just_finished
                    && let Some(hook) = request(&c.on_minimap_unlock, [0, 0])
                {
                    queues.low.push((hook, false));
                }
            }
        }
        // 1043-1050: the IF1 hover target, tracked here only because a
        // hidden IF1 layer later in this walk depends on it.
        if !c.if3
            && mouse.legacy
            && queues.dragged.is_none()
            && (c.hover_overlayer >= 0 || c.over_color != 0)
            && inside
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
            queues.hover = self.components.contains_key(&target).then_some(target);
        }
        let c = &self.components[&key];
        if c.kind != 0 {
            return;
        }
        let child_origin = [x.wrapping_sub(c.scroll_x), y.wrapping_sub(c.scroll_y)];
        // A created layer would recurse into its own array in the original
        // and never return; no script creates one.
        if key.child == -1 {
            let group = key.id >> 16;
            let statics: Vec<Key> = self
                .components
                .range(Key::component(group << 16)..Key::component((group + 1) << 16))
                .filter(|(k, child)| k.child == -1 && child.overlayer == key.id && **k != key)
                .map(|(&k, _)| k)
                .collect();
            for child in statics {
                self.walk_component(child, child_origin, bounds, mouse, queues, depth + 1);
            }
            let created: Vec<Key> = self
                .components
                .range(
                    Key {
                        id: key.id,
                        child: 0,
                    }..=Key {
                        id: key.id,
                        child: i32::MAX,
                    },
                )
                .filter(|(_, child)| child.overlayer == key.id)
                .map(|(&k, _)| k)
                .collect();
            for child in created {
                self.walk_component(child, child_origin, bounds, mouse, queues, depth + 1);
            }
            if let Some(&(sub, _)) = self.subinterfaces.get(&key.id) {
                self.walk_interface(sub, [x, y], bounds, mouse, queues, depth + 1);
            }
        }
    }
}

/// Which component a click at `point` starts dragging, decided from the draw
/// list the way the walk decides it (`processComponents` 666-676, 710-712):
/// the first clicked IF3 component the walk processes that has a drag
/// parent, except that a `noClickThrough` layer under the mouse later in
/// the walk undoes a drag begun before it. Returns the component and its
/// drag parent.
pub fn drag_source(
    state: &State,
    nodes: &[super::DrawNode],
    point: [i32; 2],
) -> Option<(Key, Key, super::Rect)> {
    let mut dragged = None;
    let mut index = 0;
    while index < nodes.len() {
        let node = &nodes[index];
        index += 1;
        let Some(c) = state.components.get(&node.key) else {
            continue;
        };
        // The walk handles these components itself and never enters them
        // (737-796), so nothing below them can be clicked.
        if c.if3 && matches!(c.client_code, 1337 | 1338 | 1400 | 1401 | 1402) {
            index = node.subtree_end.max(index);
        }
        if !node.clip.contains(point) {
            continue;
        }
        if c.kind == 0 && c.no_click_through {
            dragged = None;
        }
        if dragged.is_some() || !c.if3 {
            continue;
        }
        // 632: an IF3 component with nothing to process is not visited.
        let events = state
            .properties
            .get(&node.key)
            .map_or(c.properties.events, |p| p.events);
        if !(c.kind == 0 || c.has_event_handlers || events != 0 || c.client_code == 1338) {
            continue;
        }
        if let Some(parent) = state
            .drag_render_parent(node.key)
            .or_else(|| state.drag_parents.get(&node.key).copied())
        {
            dragged = Some((node.key, parent, node.rect));
        }
    }
    dragged
}
#[cfg(test)]
#[path = "hook_walk_tests.rs"]
mod tests;
