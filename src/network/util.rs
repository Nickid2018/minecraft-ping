use std::io::ErrorKind;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn parse_i64(str: &str) -> std::io::Result<i64> {
    Ok(i64::from_str(str).map_err(|o| std::io::Error::new(ErrorKind::InvalidData, o))?)
}