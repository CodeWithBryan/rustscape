//! Finite authoritative X-input controller. Readiness comes from actual decoded
//! packets; it never seeds inventory or replaces server outcomes with timers.
use rs09_headless::protocol::{
    buffer::Writer,
    game::{Frame, updates::ServerUpdate},
    login,
};
use serde_json::{Value, json};
use std::io;
#[derive(Default)]
pub struct Probe {
    stage: u8,
    prompts: usize,
    last_x: Option<i32>,
    chat_open: bool,
    error_ready: bool,
    error_text: Vec<u8>,
    private_seen: bool,
    closed_tick: Option<i32>,
    steps: Vec<Value>,
    pub wires: Vec<Vec<u8>>,
    varps: Vec<i32>,
}
pub struct Snapshot {
    pub carried: i64,
    pub banked: i64,
    pub inventory_slot: Option<i32>,
    pub bank_slot: Option<i32>,
    pub main_open: bool,
    pub side_open: bool,
    pub bank_full: bool,
    pub removed: bool,
    pub single_closed: bool,
    pub main_closed: bool,
    pub updates: i32,
    pub opcode: u8,
}
fn button(iface: i32, child: i32, slot: i32) -> Frame {
    let mut w = Writer::new();
    w.p4(iface << 16 | child);
    w.p2(slot);
    Frame {
        opcode: 234,
        payload: w.into_vec(),
    }
}
fn response(text: &[u8]) -> Frame {
    Frame {
        opcode: 244,
        payload: login::encode_name(text).to_be_bytes().to_vec(),
    }
}
fn continue_() -> Frame {
    let mut w = Writer::new();
    w.imp4(210 << 16 | 1);
    w.ip2(-1);
    Frame {
        opcode: 132,
        payload: w.into_vec(),
    }
}
impl Probe {
    pub fn observe(&mut self, update: Option<&ServerUpdate>) -> io::Result<()> {
        match update {
            Some(ServerUpdate::RunClientScript {
                id: 109,
                types,
                arguments,
                ..
            }) => {
                if types != b"s" || arguments.len() != 1 {
                    return Err(io::Error::other("unexpected amount prompt signature"));
                }
                self.prompts += 1;
                self.error_ready = false;
                self.private_seen = true;
            }
            Some(ServerUpdate::Varp { id: 1249, value }) => {
                self.last_x = Some(*value);
                self.varps.push(*value);
                if *value != 50 {
                    self.private_seen = true;
                }
            }
            Some(ServerUpdate::OpenInterface { interface: 210, .. }) => {
                self.chat_open = true;
                self.error_ready = false;
                self.private_seen = true;
            }
            Some(ServerUpdate::InterfaceText { pointer, text, .. })
                if *pointer == (210 << 16 | 1) =>
            {
                self.error_text = text.clone();
                self.private_seen = true;
            }
            Some(ServerUpdate::InterfaceHidden {
                pointer, hidden: 0, ..
            }) if *pointer == (210 << 16 | 1) => self.error_ready = true,
            Some(ServerUpdate::CloseInterface { pointer, .. }) if *pointer == (752 << 16 | 12) => {
                self.chat_open = false;
                self.error_ready = false;
            }
            _ => {}
        }
        Ok(())
    }
    pub fn closing(&self) -> bool {
        self.stage >= 11
    }
    pub fn done(&self) -> bool {
        self.stage == 12
    }
    pub fn private_seen(&self) -> bool {
        self.private_seen
    }
    pub fn advance(&mut self, s: Snapshot) -> io::Result<Vec<Frame>> {
        let mut out = vec![];
        match self.stage {
            0 if s.main_open && s.side_open && s.bank_full && s.removed => {
                if s.carried != 1 || s.banked != 0 {
                    return Err(io::Error::other(
                        "X probe requires one actual pickup and empty matching bank",
                    ));
                }
                self.steps.push(
                    json!({"stage":"open","inventoryQuantity":s.carried,"bankQuantity":s.banked}),
                );
                out.push(button(
                    763,
                    0,
                    s.inventory_slot
                        .ok_or_else(|| io::Error::other("missing real inventory slot"))?,
                ));
                self.stage = 1;
            }
            1 if self.prompts == 1 => {
                out.push(response(b"1k"));
                self.stage = 2;
            }
            2 if s.carried == 0 && s.banked == 1 && self.last_x == Some(1000) => {
                self.steps.push(json!({"stage":"deposit-x-confirmed","inventoryQuantity":s.carried,"bankQuantity":s.banked,"lastX":self.last_x}));
                out.push(button(
                    762,
                    73,
                    s.bank_slot
                        .ok_or_else(|| io::Error::other("missing actual bank slot"))?,
                ));
                self.stage = 3;
            }
            3 if self.prompts == 2 => {
                out.push(response(b"1"));
                self.stage = 4;
            }
            4 if s.carried == 1 && s.banked == 0 && self.last_x == Some(1) => {
                self.steps.push(json!({"stage":"withdraw-x-confirmed","inventoryQuantity":s.carried,"bankQuantity":s.banked,"lastX":self.last_x}));
                out.push(button(763, 0, s.inventory_slot.unwrap()));
                self.stage = 5;
            }
            5 if self.prompts == 3 => {
                out.push(response(b"oops"));
                self.stage = 6;
            }
            6 if self.chat_open
                && self.error_ready
                && self.error_text == b"That doesn't look right. Please try again." =>
            {
                if s.carried != 1 || s.banked != 0 {
                    return Err(io::Error::other(
                        "invalid amount changed authoritative quantities",
                    ));
                }
                self.steps.push(json!({"stage":"format-error-confirmed","text":self.error_text,"inventoryQuantity":s.carried,"bankQuantity":s.banked}));
                out.push(continue_());
                self.stage = 7;
            }
            7 if !self.chat_open && s.main_open && s.side_open => {
                self.steps
                    .push(json!({"stage":"format-continue-confirmed","bankOpen":true}));
                out.push(button(763, 0, s.inventory_slot.unwrap()));
                self.stage = 8;
            }
            8 if self.prompts == 4 => {
                out.push(response(b"2147483648"));
                self.stage = 9;
            }
            9 if self.chat_open
                && self.error_ready
                && self.error_text == b"That number's a bit large, don't you think?" =>
            {
                if s.carried != 1 || s.banked != 0 || self.last_x != Some(1) {
                    return Err(io::Error::other("overflow changed items or last-X"));
                }
                self.steps.push(json!({"stage":"overflow-error-confirmed","text":self.error_text,"inventoryQuantity":s.carried,"bankQuantity":s.banked,"lastX":self.last_x}));
                out.push(continue_());
                self.stage = 10;
            }
            10 if !self.chat_open && s.main_open && s.side_open => {
                self.steps
                    .push(json!({"stage":"overflow-continue-confirmed","bankOpen":true}));
                out.push(Frame {
                    opcode: 184,
                    payload: vec![],
                });
                let mut w = Writer::new();
                w.p4(762 << 16 | 22);
                w.p2(-1);
                out.push(Frame {
                    opcode: 155,
                    payload: w.into_vec(),
                });
                self.stage = 11;
            }
            11 if s.single_closed && s.main_closed && !s.main_open && !s.side_open => {
                let first = *self.closed_tick.get_or_insert(s.updates);
                if s.opcode == 225 && s.updates - first >= 2 {
                    self.steps.push(
                        json!({"stage":"close-confirmed","continuedPlayerUpdates":s.updates-first}),
                    );
                    self.stage = 12;
                }
            }
            _ => {}
        }
        Ok(out)
    }
    pub fn report(&self) -> Value {
        json!({"mode":"input","stage":self.stage,"steps":self.steps,"requests":self.wires,"prompts":self.prompts,"lastX":self.last_x,"varp1249":self.varps,"errorChatboxOpen":self.chat_open,"privateInputSeen":self.private_seen})
    }
}
