use rs09_client::{audio::SoundEffect, pcm::presentation::Presenter};
use std::sync::Arc;
fn sound(samples: Vec<i8>, start: usize, end: usize) -> Arc<SoundEffect> {
    Arc::new(SoundEffect {
        samples,
        loop_start: start,
        loop_end: end,
    })
}
#[test]
fn presentation_handles_follow_completion_stop_and_owner_release() {
    let mut output = Presenter::new(false);
    assert!(output.play(sound(vec![], 0, 0), 0, 127).is_none());
    let finite = output.play(sound(vec![1, -1], 0, 0), 0, 127).unwrap();
    let looping = output.play(sound(vec![1, 2, 3], 1, 2), -1, 127).unwrap();
    assert!(finite.linked() && looping.linked());
    output.read_block();
    assert!(!finite.linked() && looping.linked());
    output.stop(looping.id);
    assert!(!looping.linked());
    assert!(output.read_block().iter().all(|&s| s == 0));
    let detached = output.play(sound(vec![3], 0, 1), -1, 127).unwrap();
    drop(output);
    assert!(!detached.linked());
}
#[test]
fn diagnostic_capture_is_bounded_without_stopping_live_voice_control() {
    let mut output = Presenter::new(true);
    let voice = output.play(sound(vec![7], 0, 1), -1, 127).unwrap();
    for volume in 0..9000 {
        output.volume(voice.id, volume);
    }
    assert_eq!(output.status()["capture_complete"], false);
    assert!(output.read_block().iter().any(|&s| s != 0));
    assert!(voice.linked());
    output.stop(voice.id);
    assert!(!voice.linked());
    assert_eq!(output.status()["active_voices"], 0);
    let capture = output.capture();
    assert_eq!(capture["complete"], false);
    assert_eq!(
        capture["commands"]["commands"].as_array().unwrap().len(),
        8192
    );
}
