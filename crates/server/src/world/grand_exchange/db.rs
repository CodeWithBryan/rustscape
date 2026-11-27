//! GEDB.kt and the parts of SQLiteProvider.kt it uses: the database file, the
//! three tables with the original `CREATE TABLE` text, `initTables` in the
//! iteration order of the original `hashMapOf`, and `populateInitialPriceIndex`.
//!
//! Every statement the original executes goes through [`Db::query`],
//! [`Db::update`] or [`Db::execute`] with the original SQL text and the
//! parameters bound at that point; [`super::Host::statement`] sees it first.
//! Column reads follow the xerial JDBC driver over `sqlite3_column_*`: an SQL
//! NULL read as an int or long is 0, read as a string is Java null.
use super::{Error, Host, Result};
use crate::world::repository::hash_map::JavaHashMap;
use rusqlite::types::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

static SYNCHRONOUS_OFF: AtomicBool = AtomicBool::new(false);

/// A switch for tests and tools, not part of the ported classes: when on,
/// databases opened afterwards use `PRAGMA synchronous = OFF`, so a commit
/// does not wait for the disk. Rows and statement results are unchanged;
/// only durability after a crash is lost. Off by default, which is what the
/// original's JDBC driver uses. The replay tests turn it on: on Windows every
/// autocommitted statement otherwise waits on a flush.
pub fn set_synchronous_off(on: bool) {
    SYNCHRONOUS_OFF.store(on, Ordering::Relaxed);
}

/// `TABLE_CHECK` of SQLiteProvider.
pub const TABLE_CHECK: &str = "SELECT name FROM sqlite_master WHERE type='table' AND name=?;";

/// `GEDB.expectedTables`, as written (`hashMapOf` of the three tables).
pub const EXPECTED_TABLES: [(&str, &str); 3] = [
    (
        "player_offers",
        "CREATE TABLE player_offers(uid INTEGER PRIMARY KEY ASC,player_uid      INTEGER,item_id         INTEGER,amount_total    INTEGER,amount_complete INTEGER,offered_value   INTEGER,time_stamp      INTEGER,offer_state     INTEGER,is_sale         INTEGER,withdraw_items  STRING ,slot_index      INTEGER,total_coin_xc   INTEGER)",
    ),
    (
        "bot_offers",
        "CREATE TABLE bot_offers(item_id         INTEGER,amount          INTEGER)",
    ),
    (
        "price_index",
        "CREATE TABLE price_index(item_id         INTEGER,value           INTEGER,total_value     INTEGER,unique_trades   INTEGER,last_update     INTEGER)",
    ),
];

/// The tables in `HashMap` iteration order (the order `initTables` checks
/// and creates them). Kotlin's `hashMapOf` of three pairs is
/// `HashMap(mapCapacity(3))`, `HashMap(5)`: a table of eight buckets.
pub fn table_order() -> Vec<(&'static str, &'static str)> {
    let mut map: JavaHashMap<Option<String>, usize> =
        JavaHashMap::with_capacity(5).expect("a valid capacity");
    for (index, (name, _)) in EXPECTED_TABLES.iter().enumerate() {
        map.put(Some((*name).to_string()), index);
    }
    map.iter()
        .map(|(_, index)| EXPECTED_TABLES[*index])
        .collect()
}

/// A JDBC parameter as the original binds it.
#[derive(Clone, Debug, PartialEq)]
pub enum Param {
    /// `setInt`.
    Int(i32),
    /// `setLong`.
    Long(i64),
    /// `setBoolean` (bound as 1 or 0).
    Bool(bool),
    /// `setString` (`None` is Java null).
    Text(Option<String>),
}
impl rusqlite::ToSql for Param {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        use rusqlite::types::ToSqlOutput;
        Ok(match self {
            Self::Int(v) => ToSqlOutput::Owned(Value::Integer(i64::from(*v))),
            Self::Long(v) => ToSqlOutput::Owned(Value::Integer(*v)),
            Self::Bool(v) => ToSqlOutput::Owned(Value::Integer(i64::from(*v))),
            Self::Text(Some(v)) => ToSqlOutput::Owned(Value::Text(v.clone())),
            Self::Text(None) => ToSqlOutput::Owned(Value::Null),
        })
    }
}

/// Which JDBC call runs a statement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `executeQuery`.
    Query,
    /// `executeUpdate`.
    Update,
    /// `execute`.
    Execute,
}
impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Query => "query",
            Self::Update => "update",
            Self::Execute => "execute",
        }
    }
}

/// A result set read in full (no statement in these classes writes a table
/// while it still reads another row of the same result).
#[derive(Clone, Debug, Default)]
pub struct Rows {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}
impl Rows {
    pub fn len(&self) -> usize {
        self.rows.len()
    }
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    fn column(&self, name: &str) -> Result<usize> {
        // xerial's findColumn compares names case-insensitively.
        self.columns
            .iter()
            .position(|c| c.eq_ignore_ascii_case(name))
            .ok_or_else(|| Error::Sql(format!("no such column: '{name}'")))
    }
    fn value(&self, row: usize, column: usize) -> Result<&Value> {
        self.rows
            .get(row)
            .and_then(|r| r.get(column))
            .ok_or_else(|| Error::Sql("column index out of bounds".into()))
    }
    /// `getInt(name)`.
    pub fn int(&self, row: usize, name: &str) -> Result<i32> {
        let column = self.column(name)?;
        Ok(column_long(self.value(row, column)?) as i32)
    }
    /// `getInt(index)` (1-based).
    pub fn int_at(&self, row: usize, index: usize) -> Result<i32> {
        Ok(column_long(self.value(row, index - 1)?) as i32)
    }
    /// `getLong(name)`.
    pub fn long(&self, row: usize, name: &str) -> Result<i64> {
        let column = self.column(name)?;
        Ok(column_long(self.value(row, column)?))
    }
    /// `getLong(index)` (1-based).
    pub fn long_at(&self, row: usize, index: usize) -> Result<i64> {
        Ok(column_long(self.value(row, index - 1)?))
    }
    /// `getString(name)`; `None` is Java null.
    pub fn string(&self, row: usize, name: &str) -> Result<Option<String>> {
        let column = self.column(name)?;
        Ok(match self.value(row, column)? {
            Value::Null => None,
            Value::Integer(v) => Some(v.to_string()),
            Value::Real(v) => Some(format_real(*v)),
            Value::Text(t) => Some(t.clone()),
            Value::Blob(b) => Some(String::from_utf8_lossy(b).into_owned()),
        })
    }
}

/// `sqlite3_column_int64` of a stored value (`sqlite3_column_int` is its low
/// 32 bits).
fn column_long(value: &Value) -> i64 {
    match value {
        Value::Null => 0,
        Value::Integer(v) => *v,
        Value::Real(v) => real_to_long(*v),
        Value::Text(t) => text_to_long(t.as_bytes()),
        Value::Blob(b) => text_to_long(b),
    }
}
/// SQLite's `doubleToInt64`: saturating, NaN is 0.
fn real_to_long(v: f64) -> i64 {
    if v.is_nan() {
        0
    } else if v <= i64::MIN as f64 {
        i64::MIN
    } else if v >= i64::MAX as f64 {
        i64::MAX
    } else {
        v as i64
    }
}
/// The integer prefix SQLite reads from text (leading spaces, a sign, digits;
/// saturating).
fn text_to_long(bytes: &[u8]) -> i64 {
    let mut i = 0;
    while i < bytes.len() && matches!(bytes[i], b' ' | b'\t' | b'\n' | b'\x0c' | b'\r') {
        i += 1;
    }
    let negative = i < bytes.len() && bytes[i] == b'-';
    if i < bytes.len() && matches!(bytes[i], b'-' | b'+') {
        i += 1;
    }
    let mut value: i128 = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        value = (value * 10 + i128::from(bytes[i] - b'0')).min(i128::from(u64::MAX));
        i += 1;
    }
    let value = if negative { -value } else { value };
    value.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}
/// SQLite's text of a REAL (`%!.15g`), for the integral values these
/// classes could meet.
fn format_real(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{v:.1}")
    } else {
        format!("{v}")
    }
}

/// The database of one world: `GEDB.db` (the `lateinit` provider) with its
/// connection. Clones are the same database.
#[derive(Clone)]
pub struct Db(Arc<Inner>);
struct Inner {
    path: String,
    connection: Mutex<rusqlite::Connection>,
}
fn sql_error(e: rusqlite::Error) -> Error {
    Error::Sql(e.to_string())
}
impl Db {
    /// `SQLiteProvider(path)` and the directory part of `initTables`: the
    /// parent directory of `path` is created when missing; the file opens
    /// (and is created) on first use as the JDBC driver does.
    pub fn open(path: &str) -> Result<Self> {
        let file = std::path::Path::new(path);
        let absolute = if file.is_absolute() {
            file.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|e| Error::Sql(e.to_string()))?
                .join(file)
        };
        if let Some(parent) = absolute.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::Sql(e.to_string()))?;
        }
        let connection = rusqlite::Connection::open(&absolute).map_err(sql_error)?;
        if SYNCHRONOUS_OFF.load(Ordering::Relaxed) {
            connection
                .execute_batch("PRAGMA synchronous = OFF;")
                .map_err(sql_error)?;
        }
        Ok(Self(Arc::new(Inner {
            path: absolute.to_string_lossy().into_owned(),
            connection: Mutex::new(connection),
        })))
    }
    pub fn path(&self) -> &str {
        &self.0.path
    }
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    fn connection(&self) -> MutexGuard<'_, rusqlite::Connection> {
        self.0.connection.lock().unwrap_or_else(|e| e.into_inner())
    }
    /// `conn.prepareStatement(sql)`: SQLite compiles the statement here, so a
    /// statement on a missing table fails before anything executes.
    pub fn prepare<'a>(&'a self, sql: &'a str) -> Result<Prepared<'a>> {
        self.connection().prepare(sql).map_err(sql_error)?;
        Ok(Prepared { db: self, sql })
    }
    /// `createStatement().executeQuery(sql)`.
    pub fn query<H: Host + ?Sized>(&self, h: &mut H, sql: &str) -> Result<Rows> {
        h.statement(Kind::Query, sql, &[])?;
        self.read(sql, &[])
    }
    /// `createStatement().executeUpdate(sql)`: the changed row count.
    pub fn update<H: Host + ?Sized>(&self, h: &mut H, sql: &str) -> Result<usize> {
        h.statement(Kind::Update, sql, &[])?;
        self.write(sql, &[])
    }
    /// `createStatement().execute(sql)` of a statement that returns no rows.
    pub fn execute<H: Host + ?Sized>(&self, h: &mut H, sql: &str) -> Result<()> {
        h.statement(Kind::Execute, sql, &[])?;
        self.run(sql, &[])
    }
    fn run(&self, sql: &str, params: &[Param]) -> Result<()> {
        let connection = self.connection();
        let mut statement = connection.prepare(sql).map_err(sql_error)?;
        if statement.column_count() > 0 {
            let mut rows = statement
                .query(rusqlite::params_from_iter(params.iter()))
                .map_err(sql_error)?;
            rows.next().map_err(sql_error)?;
        } else {
            statement
                .execute(rusqlite::params_from_iter(params.iter()))
                .map_err(sql_error)?;
        }
        Ok(())
    }
    fn read(&self, sql: &str, params: &[Param]) -> Result<Rows> {
        let connection = self.connection();
        let mut statement = connection.prepare(sql).map_err(sql_error)?;
        let columns: Vec<String> = statement
            .column_names()
            .iter()
            .map(|c| (*c).to_string())
            .collect();
        let count = columns.len();
        let mut rows = statement
            .query(rusqlite::params_from_iter(params.iter()))
            .map_err(sql_error)?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().map_err(sql_error)? {
            let mut values = Vec::with_capacity(count);
            for i in 0..count {
                values.push(row.get::<_, Value>(i).map_err(sql_error)?);
            }
            out.push(values);
        }
        Ok(Rows { columns, rows: out })
    }
    fn write(&self, sql: &str, params: &[Param]) -> Result<usize> {
        let connection = self.connection();
        connection
            .execute(sql, rusqlite::params_from_iter(params.iter()))
            .map_err(sql_error)
    }
    /// An unrecorded read for tests and tools (`quote()`d rows); it is not
    /// part of the ported classes.
    pub fn inspect(&self, sql: &str) -> Result<Vec<Vec<String>>> {
        let rows = self.read(sql, &[])?;
        Ok(rows
            .rows
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|v| match v {
                        Value::Null => "NULL".into(),
                        Value::Integer(i) => i.to_string(),
                        Value::Real(r) => format_real(r),
                        Value::Text(t) => t,
                        Value::Blob(b) => String::from_utf8_lossy(&b).into_owned(),
                    })
                    .collect()
            })
            .collect())
    }
    /// An unrecorded statement for tests and tools (seeding a database the
    /// way an older server left it); it is not part of the ported classes.
    pub fn seed(&self, sql: &str) -> Result<()> {
        self.connection().execute_batch(sql).map_err(sql_error)
    }
}

/// A prepared statement (compiled by [`Db::prepare`]); each execution binds
/// the parameters given.
pub struct Prepared<'a> {
    db: &'a Db,
    sql: &'a str,
}
impl Prepared<'_> {
    /// `executeQuery()`.
    pub fn query<H: Host + ?Sized>(&self, h: &mut H, params: &[Param]) -> Result<Rows> {
        h.statement(Kind::Query, self.sql, params)?;
        self.db.read(self.sql, params)
    }
    /// `executeUpdate()`.
    pub fn update<H: Host + ?Sized>(&self, h: &mut H, params: &[Param]) -> Result<usize> {
        h.statement(Kind::Update, self.sql, params)?;
        self.db.write(self.sql, params)
    }
    /// `execute()`.
    pub fn execute<H: Host + ?Sized>(&self, h: &mut H, params: &[Param]) -> Result<()> {
        h.statement(Kind::Execute, self.sql, params)?;
        self.db.run(self.sql, params)
    }
}

/// `GEDB.init(path)`: the provider for `path`, then `initTables` with the
/// callback that fills a newly created `price_index`.
pub fn init<H: Host + ?Sized>(h: &mut H, path: &str) -> Result<Db> {
    let db = Db::open(path)?;
    h.exchange()?.set_db(db.clone());
    let check = db.prepare(TABLE_CHECK)?;
    for (table, create) in table_order() {
        let rows = check.query(h, &[Param::Text(Some(table.into()))])?;
        if rows.is_empty() {
            db.execute(h, create)?;
            if table == "price_index" {
                populate_initial_price_index(h, &db)?;
            }
        }
    }
    Ok(db)
}

/// `GEDB.init()`: `GRAND_EXCHANGE_DATA_PATH + "grandexchange.db"`.
pub fn init_default<H: Host + ?Sized>(h: &mut H) -> Result<Db> {
    let prefix = h.data_path()?;
    let path = format!("{}grandexchange.db", prefix.as_deref().unwrap_or("null"));
    init(h, &path)
}

/// `populateInitialPriceIndex`: one prepared INSERT executed for every
/// tradeable definition in `ItemDefinition.getDefinitions().values` order,
/// value and total value the high alchemy value.
fn populate_initial_price_index<H: Host + ?Sized>(h: &mut H, db: &Db) -> Result<()> {
    let definitions = h.definitions()?;
    let order = h.definition_order()?;
    let insert = db.prepare(super::price_index::INSERT_QUERY)?;
    let mut params = vec![Param::Int(0); 5];
    for id in order {
        let definition = definitions.for_id(id);
        if crate::inventory::item::definition_tradeable(&definition)? {
            let id = definition.read().id;
            let alchemy =
                crate::inventory::item::definition_alchemy(&definition, true, &definitions)?;
            params[0] = Param::Int(id);
            params[1] = Param::Int(alchemy);
            params[2] = Param::Int(alchemy);
            params[3] = Param::Int(0);
            params[4] = Param::Int(0);
            insert.execute(h, &params)?;
        }
    }
    Ok(())
}

/// `ItemDefinition.getDefinitions().values` order for definitions inserted
/// in ascending id order (as the cache parse inserts them): Java `HashMap`
/// iteration over `Integer` keys.
pub fn definition_map_order(mut ids: Vec<i32>) -> Vec<i32> {
    ids.sort_unstable();
    let mut map: JavaHashMap<i32, ()> = JavaHashMap::new();
    for id in ids {
        map.put(id, ());
    }
    map.iter().map(|(k, _)| *k).collect()
}
