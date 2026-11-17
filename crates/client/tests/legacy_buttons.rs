use rs09_client::ui::legacy_buttons::{self as buttons, Context};
use serde_json::{Value, json};
struct Ctx {
    a: Vec<i32>,
    scripts: Option<Vec<Option<Vec<i32>>>>,
    operands: Option<Vec<i32>>,
    values: Option<Vec<i32>>,
    wire: Vec<u8>,
    log: Vec<Value>,
    timer: i32,
    mouse: i32,
    chat: i32,
    inserting: i32,
}
type R<T> = Result<T, &'static str>;
impl Ctx {
    fn script(&self) -> R<&Vec<i32>> {
        self.scripts
            .as_ref()
            .ok_or("NullPointerException")?
            .first()
            .ok_or("ArrayIndexOutOfBoundsException")?
            .as_ref()
            .ok_or("NullPointerException")
    }
}
impl Context for Ctx {
    type Component = ();
    type Error = &'static str;
    fn packet(&mut self, id: i32) -> R<()> {
        self.wire.push(10);
        self.wire.extend(id.to_be_bytes());
        Ok(())
    }
    fn lookup(&mut self, id: i32) -> R<()> {
        self.log.push(json!([0, id]));
        match self.a[2] {
            -2 => Err("IllegalStateException"),
            -1 => Err("NullPointerException"),
            _ => Ok(()),
        }
    }
    fn client_code(&mut self, _: &()) -> i32 {
        self.a[3]
    }
    fn script_opcode(&mut self, _: &()) -> R<Option<i32>> {
        if self.scripts.is_none() {
            Ok(None)
        } else {
            self.script()?
                .first()
                .copied()
                .map(Some)
                .ok_or("ArrayIndexOutOfBoundsException")
        }
    }
    fn script_varp(&mut self, _: &()) -> R<i32> {
        self.script()?
            .get(1)
            .copied()
            .ok_or("ArrayIndexOutOfBoundsException")
    }
    fn operand(&mut self, _: &()) -> R<i32> {
        self.operands
            .as_ref()
            .ok_or("NullPointerException")?
            .first()
            .copied()
            .ok_or("ArrayIndexOutOfBoundsException")
    }
    fn value(&mut self, id: i32) -> R<i32> {
        self.values
            .as_ref()
            .ok_or("NullPointerException")?
            .get(id as usize)
            .copied()
            .ok_or("ArrayIndexOutOfBoundsException")
    }
    fn set_value(&mut self, id: i32, value: i32) -> R<()> {
        *self
            .values
            .as_mut()
            .ok_or("NullPointerException")?
            .get_mut(id as usize)
            .ok_or("ArrayIndexOutOfBoundsException")? = value;
        Ok(())
    }
    fn redraw_active(&mut self) -> R<()> {
        self.log.push(json!([1]));
        if self.a[8] != 0 {
            Err("IllegalStateException")
        } else {
            Ok(())
        }
    }
    fn ambient_multi(&mut self) -> R<()> {
        self.log.push(json!([2]));
        if self.a[9] != 0 {
            Err("IllegalStateException")
        } else {
            Ok(())
        }
    }
    fn varp_code(&mut self, id: i32) -> R<i32> {
        self.log.push(json!([3, id]));
        match self.a[10] {
            -2 => Err("IllegalStateException"),
            -1 => Err("NullPointerException"),
            code => Ok(code),
        }
    }
    fn set_mode(&mut self, code: i32, value: i32) {
        match code {
            5 => self.mouse = value,
            6 => self.chat = value,
            9 => self.inserting = value,
            _ => {}
        }
    }
    fn logout_timer(&mut self) -> &mut i32 {
        &mut self.timer
    }
}
fn ints(v: &Value) -> Option<Vec<i32>> {
    v.as_array()
        .map(|a| a.iter().map(|v| v.as_i64().unwrap() as i32).collect())
}
#[test]
fn complete_original_legacy_buttons_refresh_and_timer_sequences_match() {
    use sha2::{Digest, Sha256};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixture: Value = serde_json::from_slice(
        &std::fs::read(root.join("fixtures/ui/legacy-buttons.json")).unwrap(),
    )
    .unwrap();
    for (name, hash) in fixture["sources"].as_object().unwrap() {
        let raw = std::fs::read_to_string(
            root.join("reference/client/client/src/main/java/rt4")
                .join(name),
        )
        .unwrap()
        .replace("\r\n", "\n");
        assert_eq!(
            format!("{:x}", Sha256::digest(raw.as_bytes())),
            hash.as_str().unwrap(),
            "source{name}"
        );
    }
    for (i, row) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let a = ints(&row["arguments"]).unwrap();
        let mut ctx = Ctx {
            timer: a[4],
            mouse: a[5],
            chat: a[6],
            inserting: a[7],
            scripts: row["scripts"]
                .as_array()
                .map(|s| s.iter().map(ints).collect()),
            operands: ints(&row["operands"]),
            values: ints(&row["values"]),
            a,
            wire: vec![],
            log: vec![],
        };
        let (action, id) = (ctx.a[0], ctx.a[1]);
        let error = buttons::run(&mut ctx, action, id).err();
        let hex = ctx
            .wire
            .iter()
            .map(|v| format!("{v:02x}"))
            .collect::<String>();
        let actual = json!([
            hex,
            ctx.values,
            ctx.timer,
            ctx.mouse,
            ctx.chat,
            ctx.inserting,
            ctx.log,
            error
        ]);
        assert_eq!(actual, row["expected"], "case{i}");
    }
    for row in fixture["timerCases"].as_array().unwrap() {
        let mut timer = row["initial"].as_i64().unwrap() as i32;
        for _ in 0..row["ticks"].as_u64().unwrap() {
            buttons::advance_logout_timer(&mut timer);
        }
        assert_eq!(timer, row["expected"].as_i64().unwrap() as i32);
    }
}
