//! ═══════════════════════════════════════════════════════════════════════════
//! ⚡ HELIUS FEE SOURCE V1.0 — Enhanced getPriorityFeeEstimate
//!
//! Implements FeeDataSource using Helius's proprietary priority fee API.
//! Helius V2 algorithm uses max(global_percentile, per_account_percentile)
//! for each writable account — significantly more accurate for Jupiter swaps
//! than global network sampling.
//!
//! HELIUS ONLY — requires `GRIDZBOTZ_HELIUS_RPC_URL` in .env.
//! Falls back gracefully to `fallback_microlamports` if not configured.
//!
//! API: POST {helius_rpc_url} with method `getPriorityFeeEstimate`
//! Ref: https://docs.helius.dev/solana-rpc-nodes/priority-fee-api
//!
//! March 2026 — V1.0 ⚡
//! ═══════════════════════════════════════════════════════════════════════════

use async_trait::async_trait;
use log::{debug, warn};
use serde::{Deserialize, Serialize};

use crate::trading::priority_fee_estimator::FeeDataSource;
use crate::config::PriorityFeeConfig;

// ═══════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════

/// Jupiter V6 aggregator program — most contested account in Jupiter swaps.
const JUP_V6_PROGRAM: &str = "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4";
const WSOL_MINT:       &str = "So11111111111111111111111111111111111111112";
const USDC_MINT:       &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

// ═══════════════════════════════════════════════════════════════════════════
// REQUEST / RESPONSE TYPES
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Serialize)]
struct HeliusRequest {
    jsonrpc: &'static str,
    id:      &'static str,
    method:  &'static str,
    params:  Vec<HeliusParams>,
}

#[derive(Debug, Serialize)]
struct HeliusParams {
    #[serde(rename = "accountKeys")]
    account_keys: Vec<&'static str>,
    options:      HeliusOptions,
}

#[derive(Debug, Serialize)]
struct HeliusOptions {
    /// Use Helius recommended percentile (P75 of local + global max).
    recommended: bool,
}

#[derive(Debug, Deserialize)]
struct HeliusResponse {
    result: Option<HeliusResult>,
}

#[derive(Debug, Deserialize)]
struct HeliusResult {
    #[serde(rename = "priorityFeeEstimate")]
    priority_fee_estimate: f64,
}

// ═══════════════════════════════════════════════════════════════════════════
// HELIUS FEE SOURCE
// ═══════════════════════════════════════════════════════════════════════════

/// Priority fee source using Helius `getPriorityFeeEstimate` API.
///
/// Returns a single-element Vec containing the Helius-recommended fee.
/// The value is already percentile-computed by Helius (V2 algorithm),
/// so PriorityFeeEstimator will compute "percentile of one value" = that value,
/// then apply multiplier + clamp bounds from PriorityFeeConfig.
///
/// ## Why single-element Vec?
///
/// `FeeDataSource::fetch_recent_fees()` returns `Vec<u64>` — the contract
/// expects raw samples. Wrapping the Helius result as a single sample is
/// intentional: it passes through percentile math (P50 of [x] = x), then
/// gets multiplier + bounds applied. This gives consistent behaviour with
/// RpcFeeSource while using Helius's more accurate baseline.
pub struct HeliusFeeSource {
    helius_url: String,
    fallback:   u64,
    client:     reqwest::Client,
}

impl HeliusFeeSource {
    /// Create a new HeliusFeeSource.
    ///
    /// `helius_url` — full Helius RPC URL (from `GRIDZBOTZ_HELIUS_RPC_URL`).
    /// `config`     — used for fallback_microlamports on API failure.
    pub fn new(helius_url: impl Into<String>, config: &PriorityFeeConfig) -> Self {
        Self {
            helius_url: helius_url.into(),
            fallback:   config.fallback_microlamports,
            client:     reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl FeeDataSource for HeliusFeeSource {
    async fn fetch_recent_fees(&self) -> Vec<u64> {
        let request = HeliusRequest {
            jsonrpc: "2.0",
            id:      "gridzbotz-fee",
            method:  "getPriorityFeeEstimate",
            params:  vec![HeliusParams {
                account_keys: vec![JUP_V6_PROGRAM, WSOL_MINT, USDC_MINT],
                options:      HeliusOptions { recommended: true },
            }],
        };

        let resp = match self
            .client
            .post(&self.helius_url)
            .json(&request)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(r)  => r,
            Err(e) => {
                warn!("HeliusFeeSource: HTTP error: {} — using fallback {}", e, self.fallback);
                return vec![self.fallback];
            }
        };

        let helius_resp: HeliusResponse = match resp.json().await {
            Ok(r)  => r,
            Err(e) => {
                warn!("HeliusFeeSource: parse error: {} — using fallback {}", e, self.fallback);
                return vec![self.fallback];
            }
        };

        match helius_resp.result {
            Some(r) => {
                // Helius returns f64 — convert to u64 (round up for safety)
                let fee = r.priority_fee_estimate.ceil() as u64;
                debug!(
                    "HeliusFeeSource: recommended fee = {} µLCU (JUP local market, V2 algo)",
                    fee
                );
                vec![fee]
            }
            None => {
                warn!(
                    "HeliusFeeSource: null result from getPriorityFeeEstimate — using fallback {}",
                    self.fallback
                );
                vec![self.fallback]
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> PriorityFeeConfig {
        PriorityFeeConfig {
            enable_dynamic:        true,
            strategy:              "percentile".to_string(),
            percentile:            75,
            multiplier:            1.2,
            min_microlamports:     1_000,
            max_microlamports:     500_000,
            fallback_microlamports: 50_000,
            cache_ttl_secs:        10,
            sample_blocks:         150,
        }
    }

    #[test]
    fn test_helius_fee_source_construction() {
        let cfg = test_config();
        let src = HeliusFeeSource::new("https://mainnet.helius-rpc.com/?api-key=test", &cfg);
        assert_eq!(src.fallback, 50_000);
        assert!(src.helius_url.contains("helius-rpc"));
    }

    #[test]
    fn test_fee_ceil_conversion() {
        // Helius returns f64 — we ceil() to u64 for safety
        let raw: f64 = 12_345.7;
        let fee = raw.ceil() as u64;
        assert_eq!(fee, 12_346);
    }

    #[test]
    fn test_fallback_on_construction() {
        let cfg = test_config();
        let src = HeliusFeeSource::new("https://example.com", &cfg);
        // Fallback wired from config
        assert_eq!(src.fallback, cfg.fallback_microlamports);
    }
}
