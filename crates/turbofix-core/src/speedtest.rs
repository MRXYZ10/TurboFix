//! Explicit, bounded internet throughput test using Cloudflare's public speed-test endpoints.
//! A test runs only after the user clicks Start; it transfers 5 MB down + 1 MB up.

use crate::error::{CoreError, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DOWNLOAD_BYTES: usize = 5_000_000;
const UPLOAD_BYTES: usize = 1_000_000;
const ENDPOINT: &str = "https://speed.cloudflare.com";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpeedTestResult {
    pub id: Option<i64>,
    pub started_at: String,
    pub duration_ms: u64,
    pub download_mbps: f64,
    pub upload_mbps: f64,
    pub download_bytes: u64,
    pub upload_bytes: u64,
    pub download_duration_ms: u64,
    pub upload_duration_ms: u64,
    pub endpoint_label: String,
}

fn throughput_mbps(bytes: usize, elapsed: Duration) -> f64 {
    (bytes as f64 * 8.0) / elapsed.as_secs_f64().max(0.001) / 1_000_000.0
}

/// Performs bounded download/upload requests against Cloudflare's public speed test.
/// The test sends no user files, credentials, or arbitrary URLs.
pub async fn run_speed_test() -> Result<SpeedTestResult> {
    let started = Instant::now();
    let started_at = chrono::Utc::now().to_rfc3339();
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(6))
        .timeout(Duration::from_secs(60))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            let url = attempt.url();
            if url.scheme() == "https"
                && url.host_str() == Some("speed.cloudflare.com")
                && attempt.previous().len() < 3
            {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }))
        .user_agent(concat!(
            "TurboFix/",
            env!("CARGO_PKG_VERSION"),
            " network diagnostics"
        ))
        .build()
        .map_err(|e| CoreError::Internal(format!("could not initialise speed test: {e}")))?;

    let cache_key = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_string();
    // Warm the TLS connection with a tiny request so the reported phase rate is
    // less dominated by DNS/TCP/TLS setup, especially on fast broadband links.
    let warmup = client
        .get(format!("{ENDPOINT}/__down"))
        .query(&[
            ("bytes", "1000".to_string()),
            ("cacheBust", format!("{cache_key}-warmup")),
        ])
        .header(reqwest::header::CACHE_CONTROL, "no-cache")
        .send()
        .await
        .map_err(|e| CoreError::Internal(format!("speed-test warm-up failed: {e}")))?;
    if !warmup.status().is_success() {
        return Err(CoreError::Internal(format!(
            "speed-test warm-up returned HTTP {}",
            warmup.status()
        )));
    }
    if warmup.content_length().is_some_and(|n| n > 1000) {
        return Err(CoreError::Internal(
            "speed-test warm-up returned an unexpectedly large response".into(),
        ));
    }
    let mut warmup_stream = warmup.bytes_stream();
    let mut warmup_bytes = 0_usize;
    while let Some(chunk) = warmup_stream.next().await {
        let chunk = chunk
            .map_err(|e| CoreError::Internal(format!("could not read speed-test warm-up: {e}")))?;
        warmup_bytes = warmup_bytes.saturating_add(chunk.len());
        if warmup_bytes > 1000 {
            return Err(CoreError::Internal(
                "speed-test warm-up returned more data than requested".into(),
            ));
        }
    }
    if warmup_bytes != 1000 {
        return Err(CoreError::Internal(
            "speed-test warm-up returned an unexpected payload size".into(),
        ));
    }

    let download_started = Instant::now();
    let download_response = client
        .get(format!("{ENDPOINT}/__down"))
        .query(&[
            ("bytes", DOWNLOAD_BYTES.to_string()),
            ("cacheBust", cache_key),
        ])
        .header(reqwest::header::CACHE_CONTROL, "no-cache")
        .send()
        .await
        .map_err(|e| CoreError::Internal(format!("download test failed: {e}")))?
        .error_for_status()
        .map_err(|e| CoreError::Internal(format!("download endpoint returned an error: {e}")))?;
    if !download_response.status().is_success() {
        return Err(CoreError::Internal(format!(
            "download endpoint returned HTTP {}; expected a 2xx response",
            download_response.status()
        )));
    }
    if download_response
        .content_length()
        .is_some_and(|n| n > DOWNLOAD_BYTES as u64)
    {
        return Err(CoreError::Internal(
            "download endpoint returned an unexpectedly large response; test stopped".into(),
        ));
    }
    // Stream and count bytes rather than buffering the whole response. This keeps memory
    // use bounded even if a proxy or unexpected server ignores the requested byte count.
    let mut response_stream = download_response.bytes_stream();
    let mut download_bytes = 0_usize;
    while let Some(chunk) = response_stream.next().await {
        let chunk = chunk
            .map_err(|e| CoreError::Internal(format!("could not read download test data: {e}")))?;
        download_bytes = download_bytes.saturating_add(chunk.len());
        if download_bytes > DOWNLOAD_BYTES {
            return Err(CoreError::Internal(
                "download endpoint returned more data than requested; test stopped".into(),
            ));
        }
    }
    let download_elapsed = download_started.elapsed();
    if download_bytes == 0 {
        return Err(CoreError::Internal("download test returned no data".into()));
    }
    if download_bytes != DOWNLOAD_BYTES {
        return Err(CoreError::Internal(format!(
            "download test returned {} of {} expected bytes; result discarded",
            download_bytes, DOWNLOAD_BYTES
        )));
    }
    let download_mbps = throughput_mbps(download_bytes, download_elapsed);

    // A fixed-size synthetic payload is sent to the test endpoint; no local files are read.
    let upload_payload = vec![0_u8; UPLOAD_BYTES];
    let upload_started = Instant::now();
    let upload_response = client
        .post(format!("{ENDPOINT}/__up"))
        .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
        .body(upload_payload)
        .send()
        .await
        .map_err(|e| CoreError::Internal(format!("upload test failed: {e}")))?;
    if !upload_response.status().is_success() {
        return Err(CoreError::Internal(format!(
            "upload endpoint returned HTTP {}; expected a 2xx response",
            upload_response.status()
        )));
    }
    let upload_elapsed = upload_started.elapsed();
    let upload_mbps = throughput_mbps(UPLOAD_BYTES, upload_elapsed);

    Ok(SpeedTestResult {
        id: None,
        started_at,
        duration_ms: started.elapsed().as_millis().min(u64::MAX as u128) as u64,
        download_mbps,
        upload_mbps,
        download_bytes: download_bytes as u64,
        upload_bytes: UPLOAD_BYTES as u64,
        download_duration_ms: download_elapsed.as_millis().min(u64::MAX as u128) as u64,
        upload_duration_ms: upload_elapsed.as_millis().min(u64::MAX as u128) as u64,
        endpoint_label: "Cloudflare speed-test edge".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn throughput_formula_uses_decimal_megabits_per_second() {
        assert!((throughput_mbps(1_000_000, Duration::from_secs(1)) - 8.0).abs() < 0.001);
        assert!((throughput_mbps(500_000, Duration::from_secs(1)) - 4.0).abs() < 0.001);
    }

    #[test]
    fn test_sizes_are_bounded() {
        assert_eq!(DOWNLOAD_BYTES, 5_000_000);
        assert_eq!(UPLOAD_BYTES, 1_000_000);
    }
}
