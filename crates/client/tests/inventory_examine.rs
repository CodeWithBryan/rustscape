use rs09_client::ui::inventory_examine as examine;
use serde_json::{Value, json};
struct Ctx {
    counts: Vec<Option<Vec<i32>>>,
    args: Vec<i32>,
    name: Option<String>,
    calls: usize,
    log: Vec<Value>,
    wire: Vec<u8>,
    pressed: i32,
    slot: i32,
    tick: i32,
    size: i32,
    transmit: i32,
    message: Option<Value>,
}
impl examine::Context for Ctx {
    type Component = usize;
    type Error = &'static str;
    fn lookup(&mut self, id: i32) -> Result<Option<usize>, Self::Error> {
        self.log.push(json!([0, id]));
        let n = self.args[6 + usize::from(self.calls > 0)];
        self.calls += 1;
        if n == -2 {
            Err("IllegalStateException")
        } else {
            Ok((n >= 0).then_some(n as usize))
        }
    }
    fn count(&mut self, c: &usize, slot: i32) -> Result<i32, Self::Error> {
        self.counts[*c]
            .as_ref()
            .ok_or("NullPointerException")?
            .get(slot as usize)
            .copied()
            .ok_or("ArrayIndexOutOfBoundsException")
    }
    fn packet(&mut self, id: i32) -> Result<(), Self::Error> {
        self.wire
            .extend([92, id.wrapping_add(128) as u8, (id >> 8) as u8]);
        Ok(())
    }
    fn name(&mut self, id: i32) -> Result<Option<String>, Self::Error> {
        self.log.push(json!([1, id]));
        if self.args[8] != 0 {
            Err("IllegalStateException")
        } else {
            Ok(self.name.clone())
        }
    }
    fn message(&mut self, text: String) -> Result<(), Self::Error> {
        let hex: String = text.chars().map(|c| format!("{:02x}", c as u32)).collect();
        self.message = Some(json!([0, -1, hex, "", ""]));
        self.size = self.size.wrapping_add(1);
        self.transmit = self.args[9];
        Ok(())
    }
    fn reset_tick(&mut self) {
        self.tick = 0;
    }
    fn set_pressed(&mut self, c: Option<usize>) {
        self.pressed = c.map_or(-1, |i| i as i32);
    }
    fn set_slot(&mut self, n: i32) {
        self.slot = n;
    }
}
#[test]
fn original_complete_inventory_examine_and_failure_prefixes_match() {
    use sha2::{Digest, Sha256};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let f: Value = serde_json::from_slice(
        &std::fs::read(root.join("fixtures/ui/inventory-examine.json")).unwrap(),
    )
    .unwrap();
    for (name, hash) in f["sources"].as_object().unwrap() {
        let text = std::fs::read_to_string(
            root.join("reference/client/client/src/main/java/rt4")
                .join(name),
        )
        .unwrap()
        .replace("\r\n", "\n");
        assert_eq!(
            format!("{:x}", Sha256::digest(text.as_bytes())),
            hash.as_str().unwrap()
        );
    }
    assert_eq!(f["cases"].as_array().unwrap().len(), 2048);
    for (i, row) in f["cases"].as_array().unwrap().iter().enumerate() {
        let a: Vec<i32> = serde_json::from_value(row["arguments"].clone()).unwrap();
        let name = row["name"].as_str().map(|s| {
            (0..s.len())
                .step_by(2)
                .map(|i| char::from(u8::from_str_radix(&s[i..i + 2], 16).unwrap()))
                .collect()
        });
        let mut c = Ctx {
            counts: serde_json::from_value(row["counts"].clone()).unwrap(),
            args: a.clone(),
            name,
            calls: 0,
            log: vec![],
            wire: vec![],
            pressed: a[3],
            slot: a[4],
            tick: a[5],
            size: a[10],
            transmit: 0,
            message: None,
        };
        let result = examine::run(&mut c, a[1], a[0], a[2]);
        let wire: String = c.wire.iter().map(|x| format!("{x:02x}")).collect();
        let actual = json!([
            c.log,
            wire,
            c.pressed,
            c.slot,
            c.tick,
            c.size,
            c.transmit,
            c.message,
            result.err()
        ]);
        assert_eq!(actual, row["expected"], "case{i}");
    }
}
