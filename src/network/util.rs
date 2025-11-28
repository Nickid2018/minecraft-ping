use anyhow::{Result, anyhow};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::timeout;

pub fn now_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub async fn generic_timeout<F, O>(
    time: Duration,
    future: F,
    timeout_message: impl ToString,
) -> Result<O>
where
    F: Future<Output = Result<O>>,
{
    Ok(timeout(time, future)
        .await
        .map_err(|_| anyhow!("{} timed out", timeout_message.to_string()))??)
}

pub async fn io_timeout<F, O>(
    time: Duration,
    future: F,
    timeout_message: impl ToString,
) -> Result<O>
where
    F: Future<Output = std::io::Result<O>>,
{
    Ok(timeout(time, future)
        .await
        .map_err(|_| anyhow!("{} timed out", timeout_message.to_string()))??)
}
