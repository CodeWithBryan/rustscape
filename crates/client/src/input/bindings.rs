//! Component key bindings: the `keyCodes` block of
//! `InterfaceList.processComponents`.
//!
//! An IF3 component can bind up to ten keys, slot `j` standing for button
//! op `j + 1`. Each cycle, for every slot of every visited component:
//!
//! - key held, repeat timer expired and modifiers held: click the button,
//!   then arm the timer (`Integer.MAX_VALUE` for a repeat delay of 0, so the
//!   key fires once per press; otherwise the current cycle plus the delay);
//! - key held but the modifiers are not: nothing changes;
//! - key not held: the timer returns to 0 once any slot has ever fired.
//!
//! The pinned gameframe binds F1 to F8 to its side-panel tabs this way.
use super::keyboard::{KEY_ALT, KEY_CTRL, KEY_SHIFT};
use crate::ui::state::{Key, State};

/// One slot for one cycle. Returns whether the button is clicked and writes
/// the new timer. `timer` is None until the component's first click, as
/// `keyRepeatTimers` is null until then.
pub fn step(
    held: bool,
    timer: &mut Option<i32>,
    cycle: i32,
    modifier: i32,
    pressed: &[bool; 112],
    delay: i32,
) -> bool {
    if !held {
        if let Some(timer) = timer {
            *timer = 0;
        }
        return false;
    }
    if timer.is_some_and(|timer| cycle < timer) {
        return false;
    }
    let satisfied = modifier == 0
        || (modifier & 0x2 == 0 || pressed[KEY_ALT])
            && (modifier & 0x1 == 0 || pressed[KEY_CTRL])
            && (modifier & 0x4 == 0 || pressed[KEY_SHIFT]);
    if !satisfied {
        return false;
    }
    *timer = Some(if delay == 0 {
        i32::MAX
    } else {
        cycle.wrapping_add(delay)
    });
    true
}

/// The key bindings of one component the interface walk visits
/// (`ui::state::hook_walk`, which decides what is visited). `click` stands
/// for `ClientProt.sendButtonClick("", -1, op, id)`; it runs during the walk,
/// before any hook queued this cycle, as in the original.
pub fn component(
    state: &mut State,
    key: Key,
    pressed: &[bool; 112],
    cycle: i32,
    click: &mut dyn FnMut(&mut State, Key, i32),
) {
    let Ok(component) = state.get(key) else {
        return;
    };
    let (Some(codes), Some(modifiers), Some(delays)) = (
        component.key_codes.clone(),
        component.key_modifiers.clone(),
        component.key_repeat_delays.clone(),
    ) else {
        return;
    };
    // The original keeps working on the component object it started
    // with, even when a click's script replaces or deletes it.
    let mut timers = component.key_repeat_timers.clone();
    let mut current = true;
    for (slot, &code) in codes.iter().enumerate() {
        // A code outside the array is an exception in the original.
        let held = usize::try_from(code)
            .ok()
            .and_then(|code| pressed.get(code))
            .copied()
            .unwrap_or(false);
        let mut timer = timers.as_ref().map(|timers| timers[slot]);
        let fire = step(
            held,
            &mut timer,
            cycle,
            modifiers.get(slot).copied().unwrap_or(0),
            pressed,
            delays.get(slot).copied().unwrap_or(0),
        );
        if let Some(timer) = timer {
            timers.get_or_insert_with(|| vec![0; codes.len()])[slot] = timer;
        }
        if current {
            match state.get_mut(key) {
                Ok(component) => component.key_repeat_timers.clone_from(&timers),
                Err(_) => current = false,
            }
        }
        if fire {
            click(state, Key::component(key.id), slot as i32 + 1);
            current = current
                && state
                    .get(key)
                    .is_ok_and(|component| component.key_repeat_timers == timers);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(held: &[usize]) -> [bool; 112] {
        let mut pressed = [false; 112];
        for &key in held {
            pressed[key] = true;
        }
        pressed
    }

    // Transcription tests: expected values worked out by hand from
    // InterfaceList.processComponents lines 687 to 709.
    #[test]
    fn a_zero_delay_fires_once_per_press() {
        let (none, f4) = (keys(&[]), keys(&[4]));
        let mut timer = None;
        assert!(!step(false, &mut timer, 10, 0, &none, 0));
        assert_eq!(timer, None, "the timers stay null until a first click");
        assert!(step(true, &mut timer, 11, 0, &f4, 0));
        assert_eq!(timer, Some(i32::MAX));
        for cycle in 12..200 {
            assert!(!step(true, &mut timer, cycle, 0, &f4, 0));
        }
        assert!(!step(false, &mut timer, 200, 0, &none, 0));
        assert_eq!(timer, Some(0));
        assert!(step(true, &mut timer, 201, 0, &f4, 0));
    }

    #[test]
    fn a_delay_repeats_while_held() {
        let f4 = keys(&[4]);
        let mut timer = None;
        let fired: Vec<i32> = (100..130)
            .filter(|&cycle| step(true, &mut timer, cycle, 0, &f4, 10))
            .collect();
        assert_eq!(fired, [100, 110, 120]);
        // The decoded delay 4095 is stored as -1: due again on the next cycle.
        let mut timer = None;
        assert!((100..110).all(|cycle| step(true, &mut timer, cycle, 0, &f4, -1)));
        assert_eq!(timer, Some(108));
        // Cycle plus delay wraps as a Java int does.
        let mut timer = None;
        assert!(step(true, &mut timer, i32::MAX, 0, &f4, 5));
        assert_eq!(timer, Some(i32::MIN + 4));
    }

    #[test]
    fn modifiers_must_be_held_and_do_not_arm_the_timer() {
        for mask in 0..8 {
            for held in 0..8 {
                let mut down = vec![4];
                for (bit, key) in [(1, KEY_CTRL), (2, KEY_ALT), (4, KEY_SHIFT)] {
                    if held & bit != 0 {
                        down.push(key);
                    }
                }
                let mut timer = None;
                let fired = step(true, &mut timer, 50, mask, &keys(&down), 0);
                assert_eq!(fired, mask & held == mask, "mask {mask} held {held}");
                assert_eq!(timer.is_some(), fired);
            }
        }
        // Bits above the three known ones ask for nothing.
        let mut timer = None;
        assert!(step(true, &mut timer, 50, 0x78, &keys(&[4]), 0));
        // Holding the key first and the modifier later still clicks.
        let mut timer = None;
        assert!(!step(true, &mut timer, 50, 1, &keys(&[4]), 0));
        assert!(step(true, &mut timer, 51, 1, &keys(&[4, KEY_CTRL]), 0));
    }
}
