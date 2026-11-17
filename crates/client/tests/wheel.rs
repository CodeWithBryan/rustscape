use rs09_client::wheel::Wheel;
use serde_json::{Value, json};
#[test]
fn original_awt_wrapping_previous_rotation_and_poll_match() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../fixtures/players/wheel.json")).unwrap();
    let mut wheel = Wheel::default();
    for row in fixture["cases"].as_array().unwrap() {
        let c = &row["command"];
        let mut out = None;
        match c[0].as_str().unwrap() {
            "R" => wheel = Wheel::default(),
            "M" => wheel.add(c[1].as_i64().unwrap() as i32),
            "P" => out = Some(wheel.poll()),
            _ => unreachable!(),
        }
        assert_eq!(json!([wheel.current, wheel.previous, out]), row["expected"]);
    }
}
#[test]
fn browser_units_fractional_accumulation_and_native_sign_are_explicit() {
    let mut wheel = Wheel::default();
    for _ in 0..10 {
        wheel.browser(10.0, 0, f64::NAN).unwrap();
    }
    assert_eq!(wheel.poll(), 1);
    assert_eq!(wheel.poll(), 0);
    wheel.browser(-100.0, 0, 120.0).unwrap();
    assert_eq!(wheel.poll(), -1);
    wheel.browser(300.0, 0, -120.0).unwrap();
    assert_eq!(wheel.poll(), 1);
    wheel.browser(1.5, 1, f64::NAN).unwrap();
    assert_eq!(wheel.poll(), 0);
    wheel.browser(1.5, 1, f64::NAN).unwrap();
    assert_eq!(wheel.poll(), 1);
    wheel.browser(-2.0, 2, f64::NAN).unwrap();
    assert_eq!(wheel.poll(), -2);
    wheel.native(1.0).unwrap();
    assert_eq!(wheel.poll(), -1);
    wheel.browser(25.0, 0, f64::NAN).unwrap();
    wheel.browser(-25.0, 0, f64::NAN).unwrap();
    assert_eq!(wheel.poll(), 0);
    let previous = (wheel.current, wheel.previous, wheel.fraction);
    for (delta, mode) in [(f64::NAN, 0), (f64::INFINITY, 0), (1.0, 3), (2e10, 0)] {
        assert!(wheel.browser(delta, mode, f64::NAN).is_err());
    }
    assert_eq!((wheel.current, wheel.previous, wheel.fraction), previous);
}
