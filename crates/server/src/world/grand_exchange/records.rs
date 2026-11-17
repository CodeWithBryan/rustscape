//! GrandExchangeRecords.kt: a player's six offer slots (uid and slot of each
//! offer row), the five-entry history, the update notification flag, the
//! login/save/parse hooks, the collection box (109), the history log (643)
//! and the ContentAPI `hasAwaitingGrandExchangeCollections`.
use super::{
    Error, Host, JINGLE, Offer, OfferState, Result,
    components::{HISTORY_643, STOCKCOLLECT_109},
    offer, ui,
};
use crate::world::{interfaces, player_walk::NodeHandle};
use rs09_game_core::java_json::Json;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, MutexGuard},
};

/// `data class OfferRecord(val uid: Long, val slot: Int)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OfferRecord {
    pub uid: i64,
    pub slot: i32,
}

pub struct Fields {
    pub player: Option<NodeHandle>,
    pub history: [Option<Offer>; 5],
    pub offer_records: [Option<OfferRecord>; 6],
    pub update_notification: bool,
}
/// One GrandExchangeRecords object. Clones are the same object.
#[derive(Clone)]
pub struct Records(Arc<Mutex<Fields>>);
impl Records {
    /// `GrandExchangeRecords(player)`.
    pub fn new(player: Option<NodeHandle>) -> Self {
        Self(Arc::new(Mutex::new(Fields {
            player,
            history: [None, None, None, None, None],
            offer_records: [None; 6],
            update_notification: false,
        })))
    }
    pub fn fields(&self) -> MutexGuard<'_, Fields> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn identity(&self) -> usize {
        Arc::as_ptr(&self.0) as *const () as usize
    }
    pub fn player(&self) -> Option<NodeHandle> {
        self.fields().player.clone()
    }
    pub fn records(&self) -> [Option<OfferRecord>; 6] {
        self.fields().offer_records
    }
    /// `offerRecords[index] = record` (ArrayIndexOutOfBounds outside 0..6).
    pub fn set_record(&self, index: i32, record: Option<OfferRecord>) -> Result<()> {
        let slot = usize::try_from(index)
            .ok()
            .filter(|i| *i < 6)
            .ok_or(Error::Bounds)?;
        self.fields().offer_records[slot] = record;
        Ok(())
    }
    fn record(&self, index: i32) -> Result<Option<OfferRecord>> {
        let slot = usize::try_from(index)
            .ok()
            .filter(|i| *i < 6)
            .ok_or(Error::Bounds)?;
        Ok(self.fields().offer_records[slot])
    }
    pub fn history(&self) -> [Option<Offer>; 5] {
        self.fields().history.clone()
    }
}

/// `GrandExchangeRecords.getInstance(player)`: the "ge-records" attribute,
/// else a new detached instance (never stored) without a player.
pub fn get_instance<H: Host + ?Sized>(h: &mut H, player: Option<&NodeHandle>) -> Result<Records> {
    let Some(player) = player else {
        return Ok(Records::new(None));
    };
    Ok(h.records(player)?.unwrap_or_else(|| Records::new(None)))
}

/// The LoginListener body: a new instance for the player in "ge-records".
pub fn login<H: Host + ?Sized>(h: &mut H, player: &NodeHandle) -> Result<()> {
    let instance = Records::new(Some(player.clone()));
    h.set_records(player, instance)
}

fn json_int(value: Option<&Json>) -> Result<i32> {
    let text = value.map_or_else(|| "null".to_string(), Json::text);
    rs09_game_core::java_string::parse_i32(&text).ok_or(Error::Number)
}

/// `parsePlayer(player, data)` on the hook instance `this`: the saved
/// history, then the player's rows with `offer_state < 6`. Rows saved with
/// slot -1 (converted from JSON) take the slots that are free in `this`'s
/// own records (the class-scan instance's), popping the newest first.
pub fn parse_player<H: Host + ?Sized>(
    h: &mut H,
    this: &Records,
    player: &NodeHandle,
    data: &Json,
) -> Result<()> {
    if let Some(raw) = data.get("ge-history")
        && !matches!(raw, Json::Null)
    {
        let Json::Array(entries) = raw else {
            return Err(Error::ClassCast);
        };
        for (i, entry) in entries.iter().enumerate() {
            let entry = match entry {
                Json::Object(_) => entry,
                Json::Null => return Err(Error::Null),
                _ => return Err(Error::ClassCast),
            };
            let o = Offer::new();
            let item = json_int(entry.get("itemId"))?;
            o.fields().item_id = item;
            let sell = match entry.get("isSell") {
                Some(Json::Bool(b)) => *b,
                None | Some(Json::Null) => return Err(Error::Null),
                Some(_) => return Err(Error::ClassCast),
            };
            o.fields().sell = sell;
            let coins = json_int(entry.get("totalCoinExchange"))?;
            o.fields().total_coin_exchange = coins;
            let completed = json_int(entry.get("completedAmount"))?;
            o.fields().completed_amount = completed;
            let instance = get_instance(h, Some(player))?;
            if i >= 5 {
                return Err(Error::Bounds);
            }
            instance.fields().history[i] = Some(o);
        }
    }
    let mut needs_index: VecDeque<Offer> = VecDeque::new();
    let instance = get_instance(h, Some(player))?;
    let db = h.exchange()?.db()?;
    let uid = h.uid(player)?;
    let rows = db.query(
        h,
        &format!("SELECT * from player_offers where player_uid = {uid} AND offer_state < 6"),
    )?;
    for row in 0..rows.len() {
        let o = offer::from_query(h, &rows, row)?;
        if o.index() == -1 {
            needs_index.push_front(o);
        } else {
            instance.set_record(
                o.index(),
                Some(OfferRecord {
                    uid: o.uid(),
                    slot: o.index(),
                }),
            )?;
        }
    }
    if !needs_index.is_empty() {
        for index in 0..6 {
            let current = this.fields().offer_records[index];
            if current.is_none() {
                let o = needs_index.pop_front().ok_or(Error::NoSuchElement)?;
                o.fields().index = index as i32;
                instance.set_record(
                    index as i32,
                    Some(OfferRecord {
                        uid: o.uid(),
                        slot: index as i32,
                    }),
                )?;
                offer::update(h, &o)?;
            }
        }
        while let Some(o) = needs_index.pop_front() {
            h.log_ge(&format!(
                "[WARN] PLAYER HAD EXTRA OFFER - RECOMMEND IMMEDIATE REFUND OF CONTENTS -> OFFER UID: {}",
                o.uid()
            ))?;
            h.log_ge("[WARN] AS PER ABOVE MESSAGE, REFUND CONTENTS OF OFFER AND MANUALLY SET offer_state = 6")?;
        }
    }
    init(h, &instance)
}

/// `savePlayer(player, save)`: "ge-history" as an array of the non-null
/// entries (`isSell`, and `itemId`, `totalCoinExchange`, `completedAmount`
/// as strings).
pub fn save_player<H: Host + ?Sized>(
    h: &mut H,
    player: &NodeHandle,
    save: &mut Json,
) -> Result<()> {
    let mut history = Vec::new();
    let instance = get_instance(h, Some(player))?;
    for entry in instance.history().iter().flatten() {
        let f = entry.fields();
        let mut object = Json::object();
        let _ = object.put("isSell", Json::Bool(f.sell));
        let _ = object.put("itemId", Json::Text(f.item_id.to_string()));
        let _ = object.put(
            "totalCoinExchange",
            Json::Text(f.total_coin_exchange.to_string()),
        );
        let _ = object.put(
            "completedAmount",
            Json::Text(f.completed_amount.to_string()),
        );
        history.push(object);
    }
    save.put("ge-history", Json::Array(history))
        .map_err(|_| Error::ClassCast)?;
    Ok(())
}

/// `openCollectionBox()`: the bank PIN first when locked, else interface 109
/// with its six withdraw children's settings and the offers.
pub fn open_collection_box<H: Host + ?Sized>(h: &mut H, records: &Records) -> Result<()> {
    let player = records.player().ok_or(Error::Null)?;
    if !h.pin_unlocked(&player)? {
        return h.pin_open(&player, 3);
    }
    let m = h.interface_manager(&player)?;
    interfaces::open_id(&mut ui::Ui::new(h, &player), &m, STOCKCOLLECT_109)?;
    for child in [18, 23, 28, 36, 44, 52] {
        ui::send_iface_settings(h, &player, 6, child, STOCKCOLLECT_109, 0, 2)?;
    }
    visualize_records(h, records)
}

/// `visualizeRecords()`: every recorded offer that is not REMOVED, shown at
/// its record's slot.
pub fn visualize_records<H: Host + ?Sized>(h: &mut H, records: &Records) -> Result<()> {
    let db = h.exchange()?.db()?;
    for index in 0..6 {
        let Some(record) = records.fields().offer_records[index] else {
            continue;
        };
        let rows = db.query(
            h,
            &format!("select * from player_offers where uid = {}", record.uid),
        )?;
        if !rows.is_empty() {
            let o = offer::from_query(h, &rows, 0)?;
            if o.state() == OfferState::Removed {
                continue;
            }
            o.fields().index = record.slot;
            let player = records.player();
            offer::visualize(h, &o, player.as_ref())?;
        }
    }
    Ok(())
}

/// `getOffer(index)`: -1 is no offer.
pub fn get_offer<H: Host + ?Sized>(
    h: &mut H,
    records: &Records,
    index: i32,
) -> Result<Option<Offer>> {
    if index == -1 {
        return get_offer_record(h, None);
    }
    let record = records.record(index)?;
    get_offer_record(h, record)
}

/// `getOffer(record)`: the row of the record's uid, at the record's slot.
pub fn get_offer_record<H: Host + ?Sized>(
    h: &mut H,
    record: Option<OfferRecord>,
) -> Result<Option<Offer>> {
    let Some(record) = record else {
        return Ok(None);
    };
    let db = h.exchange()?.db()?;
    let rows = db.query(
        h,
        &format!("select * from player_offers where uid = {}", record.uid),
    )?;
    if rows.is_empty() {
        return Ok(None);
    }
    let o = offer::from_query(h, &rows, 0)?;
    o.fields().index = record.slot;
    Ok(Some(o))
}

/// `NumberFormat.getNumberInstance(Locale.US).format(value)`.
pub fn format_us(value: i64) -> String {
    let digits = value.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if value < 0 { format!("-{out}") } else { out }
}

/// `openHistoryLog(p)`: interface 643 with the five history entries.
pub fn open_history_log<H: Host + ?Sized>(
    h: &mut H,
    records: &Records,
    p: &NodeHandle,
) -> Result<()> {
    let m = h.interface_manager(p)?;
    let component = h.ui_component(p, HISTORY_643)?;
    interfaces::open(&mut ui::Ui::new(h, p), &m, Some(component))?;
    for i in 0..5 {
        let entry = records.fields().history[i].clone();
        let Some(o) = entry else {
            for j in 0..=3 {
                ui::send_string(h, p, "-", HISTORY_643, 25 + i as i32 + j * 5)?;
            }
            continue;
        };
        let sell = o.sell();
        ui::send_string(
            h,
            p,
            if sell { "You sold" } else { "You bought" },
            HISTORY_643,
            25 + i as i32,
        )?;
        let completed = o.completed_amount();
        ui::send_string(
            h,
            p,
            &format_us(i64::from(completed)),
            HISTORY_643,
            30 + i as i32,
        )?;
        // `ItemDefinition.forId(id).name`: a null name fails in the string writer.
        let name = h
            .definitions()?
            .for_id(o.item_id())
            .read()
            .name
            .clone()
            .ok_or(Error::Null)?;
        ui::send_string(h, p, &name, HISTORY_643, 35 + i as i32)?;
        let coins = o.fields().total_coin_exchange;
        ui::send_string(
            h,
            p,
            &format!("{} gp", format_us(i64::from(coins))),
            HISTORY_643,
            40 + i as i32,
        )?;
    }
    Ok(())
}

/// `init()`: every recorded offer is shown; when any has something to
/// collect, the message and jingle.
pub fn init<H: Host + ?Sized>(h: &mut H, records: &Records) -> Result<()> {
    let mut updated = false;
    for index in 0..6 {
        let record = records.fields().offer_records[index];
        if record.is_some() {
            let Some(o) = get_offer_record(h, record)? else {
                continue;
            };
            let withdraw = o.withdraw();
            if !updated && (withdraw[0].is_some() || withdraw[1].is_some()) {
                updated = true;
            }
            let player = records.player();
            offer::visualize(h, &o, player.as_ref())?;
        }
    }
    if updated {
        let player = records.player().ok_or(Error::Null)?;
        h.content_message(
            &player,
            "You have items waiting in your Grand Exchange collection box!",
        )?;
        let player = records.player().ok_or(Error::Null)?;
        h.jingle(&player, JINGLE)?;
    }
    Ok(())
}

/// `hasActiveOffer()`.
pub fn has_active_offer(records: &Records) -> bool {
    records.fields().offer_records.iter().any(Option::is_some)
}

/// `format()`: `itemId,amount,sell` per recorded offer, joined with `|`.
pub fn format<H: Host + ?Sized>(h: &mut H, records: &Records) -> Result<String> {
    let mut log = String::new();
    for index in 0..6 {
        let record = records.fields().offer_records[index];
        if record.is_some() {
            let Some(o) = get_offer_record(h, record)? else {
                continue;
            };
            let total = offer::amount(h, &o)?;
            log.push_str(&format!("{},{},{}|", o.item_id(), total, o.sell()));
        }
    }
    if log.ends_with('|') {
        log.pop();
    }
    Ok(log)
}

/// ContentAPI `hasAwaitingGrandExchangeCollections(player)`: the loop
/// returns at its first slot, so only slot 0 is ever looked at.
pub fn has_awaiting_collections<H: Host + ?Sized>(h: &mut H, player: &NodeHandle) -> Result<bool> {
    let records = get_instance(h, Some(player))?;
    let record = records.fields().offer_records[0];
    let o = get_offer_record(h, record)?;
    Ok(o.is_some_and(|o| o.withdraw()[0].is_some()))
}
