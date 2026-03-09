//! ═══════════════════════════════════════════════════════════════════════════
//! ⚡ RPC FEE SOURCE — getRecentPrioritizationFees Provider
//!
//! Implements FeeSource trait for the PriorityFeeEstimator.
//! Samples recent priority fees from the Solana RPC (up to 150 slots).
//!
//! Design:
//! - Owns a lightweight RpcClient (not shared with HornetProductionRpc)
//! - Sync call — matches FeeSource trait contract
//! - Empty account list = global network fees (correct for grid bot)
//! - Called infrequently (cache_ttl_secs in PriorityFeeEstimator)
//!
//! PR #79 — Dynamic Priority Fees V1.0
//! ═══════════════════════════════════════════════════════════════════════════

use anyhow::{Result, Context};
use log::{debug, warn};
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::time::Duration;

use crate::config::priority_fees::FeeSource;

/// RPC-based fee source that calls `getRecentPrioritizationFees`.
///
/// Creates its own `RpcClient` — decoupled from the main transaction pipeline.
/// Fee sampling is low-frequency (governed by `PriorityFeeEstimator` cache TTL),
/// so a separate client adds negligible overhead.
///
/// # Usage
/// ```ignore
/// let source = RpcFeeSource::new("https://api.mainnet-beta.solana.com", Duration::from_secs(5));
/// let estimator = PriorityFeeEstimator::new(config, Box::new(source));
/// ```
pub struct RpcFeeSource {
    client: RpcClient,
    /// Optional account keys to scope fee sampling.
    /// Empty = global fees (recommended for grid bot CU estimation).
    accounts: Vec<Pubkey>,
    /// Stored for diagnostics/logging only.
    rpc_url: String,
}

impl RpcFeeSource {
    /// Create a new RPC fee source pointing at the given endpoint.
    ///
    /// Uses a dedicated timeout (separate from main RPC pipeline).
    /// Recommended: 5s timeout — fee sampling is best-effort, not critical path.
    pub fn new(rpc_url: &str, timeout: Duration) -> Self {
        debug!("RpcFeeSource: targeting {}", rpc_url);
        Self {
            client: RpcClient::new_with_timeout(rpc_url.to_string(), timeout),
            accounts: Vec::new(),
            rpc_url: rpc_url.to_string(),
        }
    }

    /// Scope fee sampling to specific accounts (e.g., Jupiter program ID).
    ///
    /// When accounts are provided, the RPC returns fees paid by transactions
    /// that touched those accounts — useful for program-specific fee estimation.
    /// For general grid bot use, leave empty (global fees).
    pub fn with_accounts(mut self, accounts: Vec<Pubkey>) -> Self {
        debug!("RpcFeeSource: scoped to {} accounts", accounts.len());
        self.accounts = accounts;
        self
    }
}

impl FeeSource for RpcFeeSource {
    fn get_recent_fees(&self) -> Result<Vec<u64>> {
        let result = self
            .client
            .get_recent_prioritization_fees(&self.accounts)
            .context("RPC getRecentPrioritizationFees failed")?;

        // Extract raw fee values — estimator handles percentile/clamping
        let fees: Vec<u64> = result
            .iter()
            .map(|entry| entry.prioritization_fee)
            .collect();

        if fees.is_empty() {
            warn!(
                "RpcFeeSource: RPC returned 0 fee samples from {} — estimator will use fallback",
                self.rpc_url
            );
        } else {
            let min_fee = fees.iter().min().copied().unwrap_or(0);
            let max_fee = fees.iter().max().copied().unwrap_or(0);
            let non_zero = fees.iter().filter(|&&f| f > 0).count();
            debug!(
                "RpcFeeSource: {} slots sampled, {} non-zero, range {}-{} µL",
                fees.len(),
                non_zero,
                min_fee,
                max_fee,
            );
        }

        Ok(fees)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::priority_fees::FeeSource;

    /// Mock fee source for unit tests (no real RPC needed)
    pub struct MockFeeSource {
        fees: Vec<u64>,
    }

    impl MockFeeSource {
        pub fn new(fees: Vec<u64>) -> Self {
            Self { fees }
        }
    }

    impl FeeSource for MockFeeSource {
        fn get_recent_fees(&self) -> Result<Vec<u64>> {
            Ok(self.fees.clone())
        }
    }

    #[test]
    fn test_rpc_fee_source_construction() {
        let source = RpcFeeSource::new(
            "https://api.devnet.solana.com",
            Duration::from_secs(5),
        );
        assert!(source.accounts.is_empty());
        assert_eq!(source.rpc_url, "https://api.devnet.solana.com");
    }

    #[test]
    fn test_rpc_fee_source_with_accounts() {
        let source = RpcFeeSource::new(
            "https://api.devnet.solana.com",
            Duration::from_secs(5),
        )
        .with_accounts(vec![Pubkey::default()]);
        assert_eq!(source.accounts.len(), 1);
    }

    #[test]
    fn test_mock_fee_source_returns_fees() {
        let source = MockFeeSource::new(vec![100, 200, 300, 150, 250]);
        let fees = source.get_recent_fees().unwrap();
        assert_eq!(fees.len(), 5);
        assert_eq!(*fees.iter().min().unwrap(), 100);
        assert_eq!(*fees.iter().max().unwrap(), 300);
    }

    #[test]
    fn test_mock_fee_source_empty() {
        let source = MockFeeSource::new(vec![]);
        let fees = source.get_recent_fees().unwrap();
        assert!(fees.is_empty());
    }
}
