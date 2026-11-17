use rs09_client::{animation::Sequence, sound_queue::Queue};
use serde_json::{Value, json};
#[test]
fn original_sound_selection_queue_capacity_expiry_and_area_volume_match() {
    let corpus: Value =
        serde_json::from_str(include_str!("../../../fixtures/players/sound.json")).unwrap();
    for (scenario, case) in corpus["cases"].as_array().unwrap().iter().enumerate() {
        let mut queue = Queue::default();
        let mut volumes = [127; 2];
        let mut listener = [0; 2];
        let mut tick = 0;
        for command in case["commands"].as_array().unwrap() {
            let c = command.as_array().unwrap();
            let n = |i: usize| c[i].as_i64().unwrap() as i32;
            match c[0].as_str().unwrap() {
                "V" => {
                    volumes = [n(1), n(2)];
                    listener = [n(3), n(4)];
                }
                "Q" => queue.play(n(1), n(2), n(3), volumes[0]),
                "B" => {
                    let hex = c[1].as_str().unwrap();
                    let bytes = (0..hex.len())
                        .step_by(2)
                        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                        .collect();
                    let [id, loops, delay] =
                        rs09_client::sound_queue::synth_frame(&rs09_protocol::game::Frame {
                            opcode: 172,
                            payload: bytes,
                        })
                        .unwrap()
                        .unwrap();
                    queue.play(loops, id, delay, volumes[0]);
                }
                "S" => {
                    let seq = Sequence {
                        sounds: vec![
                            c[1].as_array()
                                .unwrap()
                                .iter()
                                .map(|v| v.as_u64().unwrap() as u32)
                                .collect(),
                        ],
                        ..Default::default()
                    };
                    queue.sequence(
                        &seq,
                        n(5),
                        [n(2), n(3)],
                        c[4].as_bool().unwrap(),
                        volumes,
                        || c[6].as_f64().unwrap(),
                    );
                }
                "T" => {
                    let played = queue.tick(listener, volumes, |id| {
                        if id % 11 == 0 { None } else { Some(id % 6) }
                    });
                    let result = json!({"entries":queue.entries.iter().map(|e|json!([e.id,e.loops,e.delay,e.position,e.start])).collect::<Vec<_>>(),"played":played.iter().map(|p|[p.id,p.loops,p.volume]).collect::<Vec<_>>()});
                    assert_eq!(
                        result, case["expected"][tick],
                        "scenario{scenario} tick{tick}"
                    );
                    tick += 1;
                }
                _ => panic!("unknown sound command"),
            }
        }
    }
}
