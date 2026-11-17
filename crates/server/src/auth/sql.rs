//! Pinned SQLStorageProvider behavior with a bounded native MySQL connection pool.
use super::{
    AuthError, Clock, Result,
    account::{Account, FieldValue, Timestamp, UserAccountInfo},
    storage::{AccountStorage, lock},
};
use chrono::{DateTime, Datelike, NaiveDate, Timelike};
use mysql::{
    DriverError, Opts, OptsBuilder, Pool, PoolConstraints, PoolOpts, PooledConn, Row, SslOpts,
    Value, prelude::Queryable,
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

pub const USERNAME_QUERY: &str = "SELECT username FROM members WHERE username = ?;";
pub const REMOVE_QUERY: &str = "DELETE FROM members WHERE username = ?;";
pub const IP_QUERY: &str = "SELECT username FROM members WHERE lastGameIp = ?;";
pub const FRIEND_QUERY: &str = "SELECT contacts FROM players WHERE username = ?;";
pub const ACCOUNT_QUERY: &str = "SELECT username,password,UID,rights,credits,ip,lastGameIp,muteTime,banTime,contacts,blocked,clanName,currentClan,clanReqs,timePlayed,lastLogin,online,joined_date FROM members WHERE username = ?;";
pub const INSERT_QUERY: &str = "INSERT INTO members (username,password,rights,credits,ip,lastGameIp,muteTime,banTime,contacts,blocked,clanName,currentClan,clanReqs,timePlayed,lastLogin,online,joined_date) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?);";

pub struct SqlConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub max_connections: usize,
    pub timeout: Duration,
    pub encryption: Encryption,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Encryption {
    Preferred,
    Required,
    Disabled,
}
impl SqlConfig {
    pub fn new(
        host: String,
        port: u16,
        database: String,
        username: String,
        password: String,
    ) -> Self {
        Self {
            host,
            port,
            database,
            username,
            password,
            max_connections: 4,
            timeout: Duration::from_secs(10),
            encryption: Encryption::Preferred,
        }
    }
}
pub struct SqlStorage {
    pool: Mutex<Option<Pool>>,
    options: Result<OptsBuilder>,
    reference: Option<ReferenceConnection>,
    encryption: Encryption,
    last_connection_error: Mutex<Option<String>>,
    timeout: Duration,
    online: Arc<dyn Fn(&str) -> bool + Send + Sync>,
}
/// The original provider's connection fields. Contains credentials; do not log.
pub struct ReferenceConnection {
    pub connection_string: String,
    pub username: String,
    pub password: String,
}
#[derive(Clone)]
pub struct Statement {
    pub query: String,
    pub parameters: Vec<FieldValue>,
}
pub fn build_update_query(indices: &[usize]) -> String {
    let fields: Vec<_> = indices
        .iter()
        .filter_map(|index| match index {
            0 => Some("username"),
            1 => Some("password"),
            3 => Some("rights"),
            4 => Some("credits"),
            6 => Some("lastGameIp"),
            7 => Some("muteTime"),
            8 => Some("banTime"),
            9 => Some("contacts"),
            10 => Some("blocked"),
            11 => Some("clanName"),
            12 => Some("currentClan"),
            13 => Some("clanReqs"),
            14 => Some("timePlayed"),
            15 => Some("lastLogin"),
            16 => Some("online"),
            _ => None,
        })
        .map(|name| format!("{name} = ?"))
        .collect();
    format!("UPDATE members SET {} WHERE uid = ?;", fields.join(","))
}
pub fn insert_statement(info: &UserAccountInfo) -> Statement {
    let values = info.values();
    let indices = [0, 1, 3, 4, 5, 5, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17];
    Statement {
        query: INSERT_QUERY.to_owned(),
        parameters: indices.into_iter().map(|i| values[i].clone()).collect(),
    }
}
pub fn update_statement(indices: &[usize], values: &[FieldValue], uid: i32) -> Result<Statement> {
    let mut parameters = Vec::new();
    for &index in indices {
        let value = values.get(index).ok_or(AuthError::Storage)?;
        if !matches!(value, FieldValue::Timestamp(_)) {
            parameters.push(value.clone());
        }
    }
    // Preserve original binding of all changed scalar fields, including fields
    // omitted by the SQL column map. Resulting invalid plans fail at execution.
    parameters.push(FieldValue::Int(uid));
    Ok(Statement {
        query: build_update_query(indices),
        parameters,
    })
}
pub fn parameter(value: &FieldValue) -> Result<Value> {
    Ok(match value {
        FieldValue::Text(value) => Value::Bytes(value.as_bytes().to_vec()),
        FieldValue::Int(value) => Value::Int(i64::from(*value)),
        FieldValue::Long(value) => Value::Int(*value),
        FieldValue::Bool(value) => Value::Int(i64::from(*value)),
        FieldValue::Timestamp(stamp) => {
            let date = DateTime::from_timestamp_millis(stamp.millis()).ok_or(AuthError::Storage)?;
            let micros = stamp.value().nanos / 1000;
            Value::Date(
                u16::try_from(date.year()).map_err(|_| AuthError::Storage)?,
                date.month() as u8,
                date.day() as u8,
                date.hour() as u8,
                date.minute() as u8,
                date.second() as u8,
                micros % 1_000_000,
            )
        }
    })
}
fn integer(value: &Value) -> Result<i64> {
    match value {
        Value::NULL => Ok(0),
        Value::Int(value) => Ok(*value),
        Value::UInt(value) => i64::try_from(*value).map_err(|_| AuthError::Storage),
        Value::Bytes(value) => std::str::from_utf8(value)
            .map_err(|_| AuthError::Storage)?
            .parse()
            .map_err(|_| AuthError::Storage),
        _ => Err(AuthError::Storage),
    }
}
fn text(value: &Value) -> Result<Option<String>> {
    match value {
        Value::NULL => Ok(None),
        Value::Bytes(value) => String::from_utf8(value.clone())
            .map(Some)
            .map_err(|_| AuthError::Storage),
        _ => Err(AuthError::Storage),
    }
}
pub fn decode_row(username: &str, row: &[Value], now: i64) -> Result<UserAccountInfo> {
    if row.len() != 18 {
        return Err(AuthError::Storage);
    }
    let mut info = UserAccountInfo::create_default(now);
    info.username = username.to_owned();
    for (column, field) in [
        (1, 1),
        (5, 5),
        (6, 6),
        (9, 9),
        (10, 10),
        (11, 11),
        (12, 12),
        (13, 13),
    ] {
        if let Some(value) = text(&row[column])? {
            info.set_value(field, FieldValue::Text(value))?;
        }
    }
    for (column, field) in [(2, 2), (3, 3), (4, 4)] {
        info.set_value(
            field,
            FieldValue::Int(i32::try_from(integer(&row[column])?).map_err(|_| AuthError::Storage)?),
        )?;
    }
    for (column, field) in [(7, 7), (8, 8), (14, 14), (15, 15)] {
        info.set_value(field, FieldValue::Long(integer(&row[column])?.max(0)))?;
    }
    let online = integer(&row[16])?;
    info.online = online == -1 || online > 0;
    match row[17] {
        Value::NULL => {}
        Value::Date(year, month, day, hour, minute, second, micros) => {
            let date = NaiveDate::from_ymd_opt(i32::from(year), u32::from(month), u32::from(day))
                .and_then(|date| {
                    date.and_hms_micro_opt(
                        u32::from(hour),
                        u32::from(minute),
                        u32::from(second),
                        micros,
                    )
                })
                .ok_or(AuthError::Storage)?
                .and_utc();
            info.join_date = Timestamp::new(date.timestamp_millis());
            info.join_date.set_nanos(micros * 1000)?;
        }
        _ => return Err(AuthError::Storage),
    }
    info.set_initial_reference_values();
    Ok(info)
}

fn parse_int(text: &str) -> Result<i32> {
    const ZEROES: &[u32] = &[
        0x30, 0x660, 0x6f0, 0x7c0, 0x966, 0x9e6, 0xa66, 0xae6, 0xb66, 0xbe6, 0xc66, 0xce6, 0xd66,
        0xde6, 0xe50, 0xed0, 0xf20, 0x1040, 0x1090, 0x17e0, 0x1810, 0x1946, 0x19d0, 0x1a80, 0x1a90,
        0x1b50, 0x1bb0, 0x1c40, 0x1c50, 0xa620, 0xa8d0, 0xa900, 0xa9d0, 0xa9f0, 0xaa50, 0xabf0,
        0xff10,
    ];
    let mut chars = text.chars();
    let first = chars.next().ok_or(AuthError::Storage)?;
    let negative = first == '-';
    let mut digits = String::new();
    for ch in (if first == '+' || first == '-' {
        None
    } else {
        Some(first)
    })
    .into_iter()
    .chain(chars)
    {
        let cp = ch as u32;
        let digit = ZEROES
            .iter()
            .find_map(|&zero| (cp >= zero && cp < zero + 10).then_some(cp.wrapping_sub(zero)))
            .ok_or(AuthError::Storage)?;
        digits.push((b'0' + digit as u8) as char);
    }
    if negative {
        digits.insert(0, '-');
    }
    digits.parse().map_err(|_| AuthError::Storage)
}
pub fn parse_contact_names(contacts: &str) -> Result<Vec<String>> {
    if contacts.is_empty() {
        return Ok(Vec::new());
    }
    let mut entries: Vec<_> = contacts.split('~').collect();
    while entries.last() == Some(&"") {
        entries.pop();
    }
    let mut names = Vec::new();
    for entry in entries {
        let cleaned = entry.replace(['{', '}'], "");
        let mut tokens: Vec<_> = cleaned.split(',').collect();
        if !cleaned.is_empty() {
            while tokens.last() == Some(&"") {
                tokens.pop();
            }
        }
        if tokens.is_empty() {
            continue;
        }
        if tokens.len() < 2 {
            return Err(AuthError::Storage);
        }
        let rank = parse_int(tokens[1])?;
        if !(0..10).contains(&rank) {
            return Err(AuthError::Storage);
        }
        names.push(tokens[0].to_owned());
    }
    Ok(rs09_game_core::java_map::string_key_order(names))
}
impl SqlStorage {
    pub fn new(
        config: SqlConfig,
        online: impl Fn(&str) -> bool + Send + Sync + 'static,
    ) -> Result<Self> {
        let constraints =
            PoolConstraints::new(1, config.max_connections).ok_or(AuthError::Storage)?;
        let options = OptsBuilder::default()
            .ip_or_hostname(Some(config.host))
            .tcp_port(config.port)
            .db_name(Some(config.database))
            .user(Some(config.username))
            .pass(Some(config.password))
            .prefer_socket(false)
            .read_timeout(Some(config.timeout))
            .write_timeout(Some(config.timeout))
            .tcp_connect_timeout(Some(config.timeout))
            .pool_opts(
                PoolOpts::default()
                    .with_constraints(constraints)
                    .with_reset_connection(true),
            );
        Ok(Self {
            pool: Mutex::new(None),
            options: Ok(options),
            reference: None,
            encryption: config.encryption,
            last_connection_error: Mutex::new(None),
            timeout: config.timeout,
            online: Arc::new(online),
        })
    }
    /// Auth.configure constructs this even when development mode never configures it.
    pub fn unconfigured(online: impl Fn(&str) -> bool + Send + Sync + 'static) -> Self {
        Self {
            pool: Mutex::new(None),
            options: Err(AuthError::Storage),
            reference: Some(ReferenceConnection {
                connection_string: String::new(),
                username: String::new(),
                password: String::new(),
            }),
            encryption: Encryption::Preferred,
            last_connection_error: Mutex::new(None),
            timeout: Duration::from_secs(10),
            online: Arc::new(online),
        }
    }
    /// Preserve configure's lazy connection validation and its unused DATABASE_PORT.
    pub fn configure_reference(
        &mut self,
        host: &str,
        database: &str,
        username: &str,
        password: &str,
    ) {
        self.reference = Some(ReferenceConnection {
            connection_string: format!(
                "jdbc:mysql://{host}/{database}?useTimezone=true&serverTimezone=UTC"
            ),
            username: username.to_owned(),
            password: password.to_owned(),
        });
        self.options = Opts::from_url(&format!("mysql://{host}/{database}"))
            .map(|options| {
                OptsBuilder::from_opts(options)
                    .user(Some(username))
                    .pass(Some(password))
                    .prefer_socket(false)
                    .read_timeout(Some(self.timeout))
                    .write_timeout(Some(self.timeout))
                    .tcp_connect_timeout(Some(self.timeout))
                    .pool_opts(
                        PoolOpts::default()
                            .with_constraints(PoolConstraints::new(1, 4).unwrap())
                            .with_reset_connection(true),
                    )
            })
            .map_err(|_| AuthError::Storage);
        *lock(&self.pool) = None;
    }
    pub fn reference_connection(&self) -> Option<&ReferenceConnection> {
        self.reference.as_ref()
    }
    fn connection(&self) -> Result<PooledConn> {
        let pool = {
            let mut pool = lock(&self.pool);
            if pool.is_none() {
                let options = self.options.as_ref().map_err(|_| {
                    *lock(&self.last_connection_error) =
                        Some("SQL provider is unconfigured or its address is invalid".into());
                    AuthError::Storage
                })?;
                let selected = if self.encryption == Encryption::Disabled {
                    Pool::new(options.clone())
                } else {
                    // Match Connector/J's default PREFERRED transport policy.
                    // Only an absent TLS capability permits plaintext fallback.
                    let tls = SslOpts::default()
                        .with_danger_skip_domain_validation(true)
                        .with_danger_accept_invalid_certs(true);
                    match Pool::new(options.clone().ssl_opts(tls)) {
                        Err(mysql::Error::DriverError(DriverError::TlsNotSupported))
                            if self.encryption == Encryption::Preferred =>
                        {
                            Pool::new(options.clone())
                        }
                        result => result,
                    }
                }
                .map_err(|error| {
                    *lock(&self.last_connection_error) = Some(error.to_string());
                    AuthError::Storage
                })?;
                *pool = Some(selected);
            }
            pool.as_ref().unwrap().clone()
        };
        let mut conn = pool.try_get_conn(self.timeout).map_err(|error| {
            *lock(&self.last_connection_error) = Some(error.to_string());
            AuthError::Storage
        })?;
        conn.query_drop("SET time_zone = '+00:00'")
            .map_err(|error| {
                *lock(&self.last_connection_error) = Some(error.to_string());
                AuthError::Storage
            })?;
        *lock(&self.last_connection_error) = None;
        Ok(conn)
    }
    pub fn last_connection_error(&self) -> Option<String> {
        lock(&self.last_connection_error).clone()
    }
    pub fn transport_encrypted(&self) -> Result<bool> {
        let value: Option<(String, String)> = self
            .connection()?
            .query_first("SHOW SESSION STATUS LIKE 'Ssl_cipher'")
            .map_err(|_| AuthError::Storage)?;
        Ok(value.is_some_and(|(_, cipher)| !cipher.is_empty()))
    }
    fn execute(conn: &mut PooledConn, plan: Statement) -> Result<()> {
        let parameters = plan
            .parameters
            .iter()
            .map(parameter)
            .collect::<Result<Vec<_>>>()?;
        conn.exec_drop(plan.query, parameters)
            .map_err(|_| AuthError::Storage)
    }
}
impl AccountStorage for SqlStorage {
    fn username_taken(&self, username: &str) -> Result<bool> {
        Ok(self
            .connection()?
            .exec_first::<Row, _, _>(USERNAME_QUERY, (username.to_lowercase(),))
            .map_err(|_| AuthError::Storage)?
            .is_some())
    }
    fn get_account_info(&self, username: &str, now: i64) -> Result<Account> {
        self.get_account_info_with_clock(username, &move || now)
    }
    fn get_account_info_with_clock(&self, username: &str, clock: &Clock) -> Result<Account> {
        let row = self
            .connection()?
            .exec_first::<Row, _, _>(ACCOUNT_QUERY, (username.to_lowercase(),))
            .map_err(|_| AuthError::Storage)?;
        // Original UserAccountInfo.createDefault executes after the query.
        let now = clock();
        Ok(match row {
            Some(row) => decode_row(username, &row.unwrap(), now)?,
            None => UserAccountInfo::create_default(now),
        }
        .shared())
    }
    fn store(&self, account: Account) -> Result<()> {
        let mut connection = self.connection()?;
        let plan = {
            let info = lock(&account);
            let mut empty = UserAccountInfo::create_default(0);
            if *info == empty {
                return Err(AuthError::IllegalState);
            }
            empty.username = info.username.clone();
            if *info == empty {
                return Err(AuthError::IllegalState);
            }
            insert_statement(&info)
        };
        let username = match &plan.parameters[0] {
            FieldValue::Text(name) => name.clone(),
            _ => unreachable!(),
        };
        // Reuse this lease for the original pre-insert existence query: no nested
        // pool acquisition or transaction changes, and the same query ordering.
        if connection
            .exec_first::<Row, _, _>(USERNAME_QUERY, (username.to_lowercase(),))
            .map_err(|_| AuthError::Storage)?
            .is_some()
        {
            return Err(AuthError::Storage);
        }
        Self::execute(&mut connection, plan)?;
        let uid = i32::try_from(connection.last_insert_id()).map_err(|_| AuthError::Storage)?;
        let mut info = lock(&account);
        info.uid = uid;
        info.set_initial_reference_values();
        Ok(())
    }
    fn update(&self, account: Account) -> Result<()> {
        let (indices, values) = lock(&account).changed_fields()?;
        let mut connection = self.connection()?;
        let uid = {
            let info = lock(&account);
            let mut empty = UserAccountInfo::create_default(0);
            if *info == empty {
                return Err(AuthError::IllegalState);
            }
            empty.username = info.username.clone();
            if *info == empty {
                return Err(AuthError::IllegalState);
            }
            empty.password = info.password.clone();
            if *info == empty {
                return Err(AuthError::IllegalState);
            }
            info.uid
        };
        if indices.is_empty() {
            return Ok(());
        }
        Self::execute(&mut connection, update_statement(&indices, &values, uid)?)?;
        lock(&account).initial_values = Some(values);
        Ok(())
    }
    fn remove(&self, account: &Account) -> Result<()> {
        let username = lock(account).username.clone();
        self.connection()?
            .exec_drop(REMOVE_QUERY, (username,))
            .map_err(|_| AuthError::Storage)
    }
    fn online_friends(&self, username: &str) -> Result<Vec<String>> {
        let row = self
            .connection()?
            .exec_first::<Row, _, _>(FRIEND_QUERY, (username,))
            .map_err(|_| AuthError::Storage)?;
        let contacts = match row {
            None => String::new(),
            Some(row) => {
                text(row.as_ref(0).ok_or(AuthError::Storage)?)?.ok_or(AuthError::Storage)?
            }
        };
        Ok(parse_contact_names(&contacts)?
            .into_iter()
            .filter(|name| (self.online)(name))
            .collect())
    }
    fn usernames_with_ip(&self, ip: &str) -> Result<Vec<Option<String>>> {
        let rows: Vec<Row> = self
            .connection()?
            .exec(IP_QUERY, (ip,))
            .map_err(|_| AuthError::Storage)?;
        rows.iter()
            .map(|row| text(row.as_ref(0).ok_or(AuthError::Storage)?))
            .collect()
    }
}
