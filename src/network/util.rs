use anyhow::{Result, anyhow};
use std::io::ErrorKind;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::timeout;

pub fn now_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn parse_i64(str: &str) -> Result<i64> {
    Ok(i64::from_str(str).map_err(|o| std::io::Error::new(ErrorKind::InvalidData, o))?)
}

pub async fn generic_timeout<F, O>(time: Duration, future: F, timeout_message: String) -> Result<O>
where
    F: Future<Output = Result<O>>,
{
    Ok(timeout(time, future)
        .await
        .map_err(|_| anyhow!("{} timed out", timeout_message))??)
}

pub async fn io_timeout<F, O>(time: Duration, future: F, timeout_message: &str) -> Result<O>
where
    F: Future<Output = std::io::Result<O>>,
{
    Ok(timeout(time, future)
        .await
        .map_err(|_| anyhow!("{} timed out", timeout_message))??)
}
