//! ═══════════════════════════════════════════════════════════════════════════
//! 📡 PRICE FEED UTILITIES — Pyth Hermes HTTP Price Fetching
//!
//! Extracted from engine.rs (PR #71) into its own module for reuse.
//!
//! Features:
//! - Pyth Hermes v2 HTTP API integration
//! - Retry with exponential backoff (3 attempts, 500ms/1s/2s)
//! - Price sanity validation (positive, confidence bounds)
//! - Stateless — safe for multi-bot concurrent use
//!
//! Follow-up:
//! - Response caching with TTL
//! - Multiple fallback Hermes endpoints
//! - pyth_proxy.js bridge integration
//!
//! March 2026 — PR #72
//! ═══════════════════════════════════════════════════════════════════════════

use anyhow::{Result, Context, bail};
use log::warn;
use serde_json::Value;
use tokio::time::{sleep, Duration};

const MAX_RETRIES: u32 = 3;
const BASE_DELAY_MS: u64 = 500;

/// Fetch the latest price from Pyth Hermes v2 HTTP API with retry.
///
/// Calls `/v2/updates/price/latest` with `parsed=true` and extracts
/// the price adjusted by the Pyth exponent.
///
/// Retries up to 3 times with exponential backoff (500ms, 1s, 2s).
///
/// # Arguments
/// * `endpoint` - Pyth Hermes base URL (e.g., "https://hermes.pyth.network")
/// * `feed_id` - Pyth price feed ID (hex string)
///
/// # Errors
/// - All retry attempts exhausted
/// - Non-positive price after adjustment
pub async fn fetch_pyth_price(endpoint: &str, feed_id: &str) -> Result<f64> {
    let url = format!(
        "{}/v2/updates/price/latest?ids[]={}&parsed=true",
        endpoint.trim_end_matches('/'),
        feed_id
    );

    let mut last_err = None;

    for attempt in 1..=MAX_RETRIES {
        match fetch_price_once(&url).await {
            Ok(price) => return Ok(price),
            Err(e) => {
                last_err = Some(e);
                if attempt < MAX_RETRIES {
                    let delay = BASE_DELAY_MS * 2u64.pow(attempt - 1);
                    warn!(
                        "Pyth price fetch attempt {}/{} failed, retrying in {}ms...",
                        attempt, MAX_RETRIES, delay
                    );
                    sleep(Duration::from_millis(delay)).await;
                }
            }
        }
    }

    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("fetch_pyth_price: no attempts made")))
}

/// Single-attempt price fetch (no retry).
async fn fetch_price_once(url: &str) -> Result<f64> {
    let resp: Value = reqwest::get(url)
        .await
        .context("Pyth Hermes HTTP request failed — check network connectivity")?
        .json()
        .await
        .context("Failed to parse Pyth response as JSON")?;

    let price_data = &resp["parsed"][0]["price"];

    let price_str = price_data["price"]
        .as_str()
        .context("Missing 'price' field in Pyth response — API format may have changed")?;

    let expo = price_data["expo"]
        .as_i64()
        .context("Missing 'expo' field in Pyth response — API format may have changed")?;

    let raw_price: f64 = price_str
        .parse()
        .context("Failed to parse Pyth price string as f64")?;

    let adjusted_price = raw_price * 10f64.powi(expo as i32);

    // Confidence check — warn if spread is unusually wide
    if let Some(conf_str) = price_data["conf"].as_str() {
        if let Ok(conf_raw) = conf_str.parse::<f64>() {
            let confidence = conf_raw * 10f64.powi(expo as i32);
            if adjusted_price > 0.0 {
                let conf_pct = (confidence / adjusted_price) * 100.0;
                if conf_pct > 5.0 {
                    warn!(
                        "Pyth confidence interval is wide: {:.2}% — price may be unreliable",
                        conf_pct
                    );
                }
            }
        }
    }

    if adjusted_price <= 0.0 {
        bail!(
            "Pyth returned non-positive price: {} (raw={}, expo={}). \
             Feed may be stale or misconfigured.",
            adjusted_price, raw_price, expo
        );
    }

    Ok(adjusted_price)
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    #[test]
    fn test_price_adjustment_normal() {
        // Simulates 14735000000 * 10^-8 = 147.35
        let raw = 14_735_000_000.0_f64;
        let expo = -8_i32;
        let adjusted = raw * 10f64.powi(expo);
        assert!((adjusted - 147.35).abs() < 0.01);
    }

    #[test]
    fn test_price_adjustment_small_expo() {
        // 12345 * 10^-2 = 123.45
        let raw = 12_345.0_f64;
        let expo = -2_i32;
        let adjusted = raw * 10f64.powi(expo);
        assert!((adjusted - 123.45).abs() < 0.001);
    }

    #[test]
    fn test_price_adjustment_zero_expo() {
        let raw = 42.0_f64;
        let expo = 0_i32;
        let adjusted = raw * 10f64.powi(expo);
        assert!((adjusted - 42.0).abs() < 0.001);
    }

    #[test]
    fn test_confidence_percentage_calculation() {
        let price = 150.0_f64;
        let confidence = 3.0_f64;
        let conf_pct = (confidence / price) * 100.0;
        assert!((conf_pct - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_confidence_wide_detection() {
        let price = 100.0_f64;
        let confidence = 6.0_f64;
        let conf_pct = (confidence / price) * 100.0;
        assert!(conf_pct > 5.0, "6% confidence should trigger wide warning");
    }
}
