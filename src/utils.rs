use std::collections::VecDeque;
use std::fmt::{Display, Formatter};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use chrono::{DateTime, TimeDelta};
use comparable::Comparable;
use regex::Regex;
use reqwest::Client;
use serde_derive::{Deserialize, Serialize};

static LOGS: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();
static TUI_MODE: AtomicBool = AtomicBool::new(false);
static LOG_FILE: OnceLock<Mutex<Option<std::fs::File>>> = OnceLock::new();
const LOG_CAP: usize = 1000;

pub fn set_tui_mode(on: bool) {
    TUI_MODE.store(on, Ordering::Relaxed);
}

pub fn tui_mode() -> bool {
    TUI_MODE.load(Ordering::Relaxed)
}

pub fn set_log_file(path: &str) {
    let f = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .ok();
    let cell = LOG_FILE.get_or_init(|| Mutex::new(None));
    *cell.lock().unwrap() = f;
}

fn log_ring() -> &'static Mutex<VecDeque<String>> {
    LOGS.get_or_init(|| Mutex::new(VecDeque::with_capacity(LOG_CAP + 16)))
}

pub fn emit_log(tag: &str, color: &str, msg: &str) {
    let ts = chrono::Local::now().format("%H:%M:%S");
    let line = format!("{ts} [{tag}] {msg}");
    {
        let mut ring = log_ring().lock().unwrap();
        if ring.len() >= LOG_CAP {
            ring.pop_front();
        }
        ring.push_back(line.clone());
    }
    if let Some(cell) = LOG_FILE.get() {
        if let Some(f) = cell.lock().unwrap().as_mut() {
            use std::io::Write as _;
            let _ = writeln!(f, "{line}");
            let _ = f.flush();
        }
    }
    if !tui_mode() {
        eprintln!("{color}[{tag}] {msg}\x1B[0m");
    }
}

pub fn recent_logs(max: usize) -> Vec<String> {
    let ring = log_ring().lock().unwrap();
    let skip = ring.len().saturating_sub(max);
    ring.iter().skip(skip).cloned().collect()
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        $crate::utils::emit_log("Erreur", "\x1B[31m", &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        $crate::utils::emit_log("Avertissement", "\x1B[33m", &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        $crate::utils::emit_log("Information", "\x1B[32m", &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! my_msg {
    ($($arg:tt)*) => {
        $crate::utils::emit_log("MyMsg", "\x1B[38;5;201m", &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! msg {
    ($($arg:tt)*) => {
        $crate::utils::emit_log("Msg", "\x1b[36m", &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! serv_msg {
    ($($arg:tt)*) => {
        $crate::utils::emit_log("Server", "\x1b[34m", &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! dm_msg {
    ($($arg:tt)*) => {
        $crate::utils::emit_log("DM", "\x1B[35m", &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! dm_msg_delete {
    ($($arg:tt)*) => {
        $crate::utils::emit_log("Delete", "\x1B[31m", &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! voice_connect {
    ($($arg:tt)*) => {
        $crate::utils::emit_log("Voice+", "\x1B[38;5;10m", &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! voice_disconnect {
    ($($arg:tt)*) => {
        $crate::utils::emit_log("Voice-", "\x1B[38;5;9m", &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! login_state {
    ($client:expr, $status:expr) => {
        match $status {
            "online" | "offline" | "idle" | "dnd" | "invisible" => {
                $crate::utils::emit_log("Login", "\x1B[92m", &format!("{}: {}", $client, $status))
            }
            _ => {}
        }
    };
}

#[macro_export]
macro_rules! bool_state {
    ($status:expr) => {
        match $status {
            true => "\x1B[38;5;10mtrue\x1B[0m",
            false => "\x1B[38;5;9mfalse\x1B[0m",
        }
    };
}

pub fn parse_timestamp(time: &str) -> String {
    match DateTime::parse_from_rfc3339(time) {
        Ok(dt) => {
            let local = dt
                .naive_local()
                .checked_add_signed(TimeDelta::hours(2))
                .unwrap_or_else(|| dt.naive_local());
            local.format("%d/%m/%Y %H:%M:%S").to_string()
        }
        Err(e) => format!("Error parsing date: {}", e),
    }
}

pub fn timestamp_to_date(timestamp_ns: u64) -> String {
    use chrono::TimeZone;
    let secs = (timestamp_ns / 1_000_000_000) as i64;
    match chrono::Local.timestamp_opt(secs, 0).single() {
        Some(dt) => dt.format("%Y_%m_%d_%H_%M_%S").to_string(),
        None => format!("invalid_ts_{}", secs),
    }
}

static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();

pub fn http_client() -> &'static Client {
    HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
            .build()
            .unwrap_or_else(|_| Client::new())
    })
}

static REG_MEDIA: OnceLock<Regex> = OnceLock::new();
static REG_CONTENT: OnceLock<Regex> = OnceLock::new();
static REG_PARSE_ERR: OnceLock<Regex> = OnceLock::new();

pub fn reg_media() -> &'static Regex {
    REG_MEDIA.get_or_init(|| Regex::new(r"/([^/]*)\?").expect("static regex"))
}

pub fn reg_content() -> &'static Regex {
    REG_CONTENT
        .get_or_init(|| Regex::new(r"https://.*.discordapp.*.*/([^/]*)\?.*").expect("static regex"))
}

pub fn reg_parse_err() -> &'static Regex {
    REG_PARSE_ERR.get_or_init(|| Regex::new(r"line (\d+) column (\d+)").expect("static regex"))
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash, Copy, Comparable)]
#[serde(transparent)]
pub struct SnowflakeID(#[serde(with = "snowflake")] u64);

pub mod snowflake {
    use serde::Deserialize;

    pub fn serialize<S>(n: &u64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&n.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: &str = Deserialize::deserialize(deserializer)?;
        s.parse::<u64>().map_err(serde::de::Error::custom)
    }
}

impl Default for SnowflakeID {
    fn default() -> Self {
        Self(0)
    }
}

impl SnowflakeID {
    pub const fn const_zero() -> Self {
        Self(0)
    }
    pub const fn from_u64(n: u64) -> Self {
        Self(n)
    }
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl Display for SnowflakeID {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl PartialEq<u64> for SnowflakeID {
    fn eq(&self, other: &u64) -> bool {
        &self.0 == other
    }
}

impl From<SnowflakeID> for u64 {
    fn from(value: SnowflakeID) -> Self {
        value.0
    }
}
