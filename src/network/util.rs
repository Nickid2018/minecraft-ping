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

pub fn parse_i64(str: &str) -> std::io::Result<i64> {
    Ok(i64::from_str(str).map_err(|o| std::io::Error::new(ErrorKind::InvalidData, o))?)
}

pub async fn io_timeout<F>(
    time: Duration,
    future: F,
    timeout_message: &str,
) -> std::io::Result<F::Output>
where
    F: Future,
{
    timeout(time, future).await.map_err(|_| {
        std::io::Error::new(
            ErrorKind::TimedOut,
            format!("{} timed out", timeout_message),
        )
    })
}
