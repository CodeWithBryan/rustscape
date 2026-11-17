//! The six random offsets of a game session and their slow drift, once for
//! both of their users.
//!
//! `client.resetGameSessionState` (client.java:658-671) draws six values
//! from `Math.random()` at every successful login, in this order:
//! `Camera.cameraOffsetX`, `Camera.yawTarget`, `Camera.cameraOffsetY`,
//! `MiniMap.zoomOffset`, `MiniMap.compassAngleOffset`, `Camera.yawDrift`.
//! The tail of `Protocol.loop` (Protocol.java:2920-2975) then moves five of
//! them, every 501st cycle, with one more draw for the camera's three and
//! one for the minimap's two, in that order.
//!
//! [`crate::minimap::Drift`] holds all of that state and is the port of
//! both blocks that is compared with the original on a JVM (`cargo xtask
//! minimap-oracle`). [`crate::lifecycle::reset_session`] makes the six
//! draws with it and returns them as a [`Jitter`]. The functions here hand
//! the camera's half on to [`Follow`], whose pose reads them, so that the
//! minimap and the follow camera share one draw and one drift step.
//! [`Follow::login`] and [`Follow::drift`] are the same statements for a
//! camera on its own (they are what `cargo xtask camera-oracle` runs);
//! the client does not call them, because their draws would not be in the
//! original's order next to the minimap's.
use crate::{follow_camera::Follow, lifecycle::Jitter, minimap::Drift};

/// The six assignments of `client.resetGameSessionState`, from the values
/// its draws produced. The steps and timers of the drift are not reset
/// there and are left as they are. `follow` is None where no follow camera
/// is driven by a session (the offline workbench keeps a steady view).
pub fn reset(jitter: &Jitter, drift: &mut Drift, follow: Option<&mut Follow>) {
    drift.camera_offset = [jitter.camera_offset_x, jitter.camera_offset_y];
    drift.zoom_offset = jitter.zoom_offset;
    drift.angle_offset = jitter.compass_angle_offset;
    drift.yaw_drift = jitter.yaw_drift;
    if let Some(follow) = follow {
        follow.yaw_target = f64::from(jitter.yaw_target);
        share(drift, follow);
    }
}

/// One 20 ms cycle of the drift (Protocol.java:2920-2975) for the minimap
/// and, if given, the follow camera. `random` is `Math.random()`; it is
/// called twice on every 501st cycle and not at all otherwise.
pub fn cycle(drift: &mut Drift, follow: Option<&mut Follow>, random: impl FnMut() -> f64) {
    drift.cycle(random);
    if let Some(follow) = follow {
        share(drift, follow);
    }
}

/// `Camera.cameraOffsetX/Y`, `Camera.offsetXDelta`,
/// `Protocol.cameraYOffsetDelta`, `Camera.yawDrift`, `Protocol.yawDriftDelta`
/// and `Protocol.cameraJitterTimer` are single statics in the original.
fn share(drift: &Drift, follow: &mut Follow) {
    follow.offset = drift.camera_offset;
    follow.offset_delta = drift.camera_step;
    follow.yaw_drift = drift.yaw_drift;
    follow.yaw_drift_delta = drift.yaw_step;
    follow.jitter_timer = drift.camera_timer;
}

#[cfg(test)]
mod tests {
    use super::*;
    use rs09_game_core::random::JavaRandom;

    #[test]
    fn one_draw_of_six_in_the_original_order_feeds_the_minimap_and_the_camera() {
        // Transcription of client.java:658-671 with chosen draws.
        let mut draws = [0.0, 0.2, 0.999, 0.25, 0.75, 0.1].into_iter();
        let jitter = Jitter::draw(|| draws.next().unwrap());
        assert!(draws.next().is_none());
        assert_eq!(
            jitter,
            Jitter {
                // (int) (0.0 * 100) - 50
                camera_offset_x: -50,
                // (int) (0.2 * 20) - 10 & 0x7FF = -6 & 0x7FF
                yaw_target: 2042,
                // (int) (0.999 * 110) - 55
                camera_offset_y: 54,
                // (int) (0.25 * 30) - 20
                zoom_offset: -13,
                // (int) (0.75 * 120) - 60
                compass_angle_offset: 30,
                // (int) (0.1 * 80) - 40
                yaw_drift: -32,
            }
        );
        let mut drift = Drift::default();
        let mut follow = Follow {
            pitch_target: 200.0,
            yaw_target: 77.5,
            ..Follow::default()
        };
        reset(&jitter, &mut drift, Some(&mut follow));
        assert_eq!(drift.camera_offset, [-50, 54]);
        assert_eq!(drift.yaw_drift, -32);
        assert_eq!([drift.zoom_offset, drift.angle_offset], [-13, 30]);
        assert_eq!(follow.offset, [-50, 54]);
        assert_eq!(follow.yaw_drift, -32);
        assert_eq!(follow.yaw_target, 2042.0);
        // The pitch target is not among the six.
        assert_eq!(follow.pitch_target, 200.0);
        // Without a camera only the drift state is set.
        let mut alone = Drift::default();
        reset(&jitter, &mut alone, None);
        assert_eq!(alone, drift);
    }

    #[test]
    fn the_joined_camera_half_is_follow_login_and_drift_on_the_cameras_own_draws() {
        // `Follow::login` and `Follow::drift` are the camera's statements
        // alone, as the camera oracle runs them against the JVM. Next to
        // the minimap the same statements see the 1st, 2nd, 3rd and 6th of
        // the login draws and the first of each later pair.
        for seed in [0, 1, 530, 0x5DEE_CE66D] {
            let mut random = JavaRandom::new(seed);
            let mut drawn = Vec::new();
            let mut next = || {
                let value = random.next_double();
                drawn.push(value);
                value
            };
            let mut drift = Drift::default();
            let mut follow = Follow::default();
            let jitter = Jitter::draw(&mut next);
            reset(&jitter, &mut drift, Some(&mut follow));
            let mut states = vec![follow.clone()];
            for _ in 0..40_000 {
                cycle(&mut drift, Some(&mut follow), &mut next);
                states.push(follow.clone());
            }
            assert_eq!(drawn.len(), 6 + 2 * (40_000 / 501));

            let mut alone = Follow::default();
            let mut login = [drawn[0], drawn[1], drawn[2], drawn[5]].into_iter();
            alone.login(|| login.next().unwrap());
            let mut camera = drawn[6..].iter().step_by(2).copied();
            assert_eq!(states[0], alone);
            for state in &states[1..] {
                alone.drift(|| camera.next().unwrap());
                assert_eq!(*state, alone);
            }
            assert!(camera.next().is_none());
            // The minimap's half stayed inside its bounds while it moved.
            assert!((-62..=62).contains(&drift.angle_offset));
            assert!((-21..=11).contains(&drift.zoom_offset));
        }
    }

    #[test]
    fn a_second_session_draws_again_and_keeps_the_steps_and_timers() {
        // resetGameSessionState assigns the six values only: the deltas and
        // the two jitter timers are statics no login touches.
        let mut random = JavaRandom::new(7);
        let mut drift = Drift::default();
        let mut follow = Follow::default();
        let first = Jitter::draw(|| random.next_double());
        reset(&first, &mut drift, Some(&mut follow));
        for _ in 0..30_300 {
            cycle(&mut drift, Some(&mut follow), || random.next_double());
        }
        let before = drift;
        let second = Jitter::draw(|| random.next_double());
        assert_ne!(first, second);
        reset(&second, &mut drift, Some(&mut follow));
        assert_eq!(drift.camera_step, before.camera_step);
        assert_eq!(drift.yaw_step, before.yaw_step);
        assert_eq!(drift.angle_step, before.angle_step);
        assert_eq!(drift.zoom_step, before.zoom_step);
        assert_eq!(drift.camera_timer, before.camera_timer);
        assert_eq!(drift.minimap_timer, before.minimap_timer);
        assert_eq!(drift.camera_timer, 30_300 % 501);
        assert_eq!(
            drift.camera_offset,
            [second.camera_offset_x, second.camera_offset_y]
        );
        assert_eq!(follow.offset, drift.camera_offset);
        assert_eq!(follow.offset_delta, before.camera_step);
        assert_eq!(follow.jitter_timer, before.camera_timer);
        assert_eq!(follow.yaw_target, f64::from(second.yaw_target));
        // The same seed gives the same session: capture runs use a fixed one.
        let mut again = JavaRandom::new(7);
        assert_eq!(Jitter::draw(|| again.next_double()), first);
    }
}
