//! ═══════════════════════════════════════════════════════════════════════════
//! ⚡ RPC FEE SOURCE V1.0 — Standard getRecentPrioritizationFees
//!
//! Implements FeeDataSource using Solana's standard RPC method.
//! Compatible with any RPC provider (Chainstack, Helius, QuickNode, etc.)
//!
//! Passes the Jupiter V6 program address + SOL + USDC mints as account keys
//! to bias sampling toward the local fee market for Jupiter swaps.
//!
//! Response field: `prioritizationFee` (u64, microlamports per compute unit)
//! — already in the correct unit for PriorityFeeEstimator.
//!
//! March 2026 — V1.0 ⚡
//! ═══════════════════════════════════════════════════════════════════════════

use async_trait::async_trait;
use log::{debug, warn};
use serde::Deserialize;

use crate::trading::priority_fee_estimator::FeeDataSource;

// ═══════════════════════════════════════════════════════════════════════════
// CONSTANTS — Jupiter V6 + major mints for local fee market accuracy
// ═══════════════════════════════════════════════════════════════════════════

/// Jupiter V6 aggregator program — most contested account in swap transactions.
const JUP_V6_PROGRAM: &str = "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4";

/// Wrapped SOL mint (same as SOL_MINT in dex module).
const WSOL_MINT: &str = "So11111111111111111111111111111111111111112";

/// USDC mint on Solana mainnet.
const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

// ═══════════════════════════════════════════════════════════════════════════
// RPC RESPONSE TYPES
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
struct RpcResponse {
    result: Option<Vec<PrioritizationFeeEntry>>,
}

#[derive(Debug, Deserialize)]
struct PrioritizationFeeEntry {
    #[serde(rename = "prioritizationFee")]
    prioritization_fee: u64,
    // slot: u64  — present in response but not needed for estimation
}

// ═══════════════════════════════════════════════════════════════════════════
// RPC FEE SOURCE
// ═══════════════════════════════════════════════════════════════════════════

/// Fetches priority fee samples via standard `getRecentPrioritizationFees` RPC.
///
/// Returns raw `prioritizationFee` values (µLamports/CU) from recent blocks.
/// On any RPC error, returns an empty Vec — PriorityFeeEstimator falls back
/// to `fallback_microlamports` automatically.
///
/// ## Account Key Strategy
///
/// Passing the Jupiter V6 program + SOL + USDC mints as account keys causes
/// the RPC to filter fee history to slots where those accounts were active,
/// giving a fee distribution representative of actual Jupiter swap conditions
/// rather than global network noise.
pub struct RpcFeeSource {
    rpc_url: String,
    client:  reqwest::Client,
}

impl RpcFeeSource {
    /// Create a new RpcFeeSource targeting the given RPC endpoint.
    pub fn new(rpc_url: impl Into<String>) -> Self {
        Self {
            rpc_url: rpc_url.into(),
            client:  reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl FeeDataSource for RpcFeeSource {
    async fn fetch_recent_fees(&self) -> Vec<u64> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "id":      1,
            "method":  "getRecentPrioritizationFees",
            "params":  [[
                JUP_V6_PROGRAM,
                WSOL_MINT,
                USDC_MINT,
            ]]
        });

        let resp = match self
            .client
            .post(&self.rpc_url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(r)  => r,
            Err(e) => {
                warn!("RpcFeeSource: HTTP error fetching prioritization fees: {}", e);
                return vec![];
            }
        };

        let rpc_resp: RpcResponse = match resp.json().await {
            Ok(r)  => r,
            Err(e) => {
                warn!("RpcFeeSource: failed to parse prioritization fee response: {}", e);
                return vec![];
            }
        };

        let entries = match rpc_resp.result {
            Some(e) => e,
            None    => {
                warn!("RpcFeeSource: RPC returned null result for getRecentPrioritizationFees");
                return vec![];
            }
        };

        // Filter out zero-fee slots (network idle — not representative)
        let fees: Vec<u64> = entries
            .into_iter()
            .map(|e| e.prioritization_fee)
            .filter(|&f| f > 0)
            .collect();

        debug!(
            "RpcFeeSource: fetched {} non-zero fee samples (JUP local market)",
            fees.len()
        );

        fees
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_fee_source_construction() {
        let src = RpcFeeSource::new("https://solana-mainnet.core.chainstack.com/test");
        assert!(src.rpc_url.contains("chainstack"));
    }

    #[test]
    fn test_zero_fee_filtering() {
        // Simulate what the filter closure does
        let raw = vec![0u64, 1000, 0, 5000, 200, 0, 300];
        let filtered: Vec<u64> = raw.into_iter().filter(|&f| f > 0).collect();
        assert_eq!(filtered, vec![1000, 5000, 200, 300]);
        assert!(!filtered.contains(&0));
    }

    #[test]
    fn test_all_zero_returns_empty() {
        let raw = vec![0u64, 0, 0];
        let filtered: Vec<u64> = raw.into_iter().filter(|&f| f > 0).collect();
        assert!(filtered.is_empty());
    }
}
