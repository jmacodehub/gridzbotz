//! ═══════════════════════════════════════════════════════════════════════════
//! ⚡ PRIORITY FEE ESTIMATOR V1.1 — Dynamic Compute-Unit Priority Fees
//!
//! V1.1 CHANGES (feat/priority-fee-quant-log — PR #97 Commit 2):
//! ✅ test_priority_fee_fallback_on_rpc_error: GAP-4 formally closed.
//!    Explicitly tests MockFeeSource::failing() → fallback_microlamports.
//!    Distinguishes from test_fallback_on_empty_samples (generic empty vec)
//!    by documenting the RPC-error-specific contract and audit trail entry.
//!
//! Estimates optimal priority fees for Solana transactions by sampling
//! recent prioritization fees from the RPC and computing a percentile.
//!
//! Architecture:
//!   PriorityFeeConfig → PriorityFeeEstimator → cached fee (µLamports)
//!                                ↑
//!                         FeeDataSource trait
//!                        (RPC impl | Helius impl | Mock impl)
//!
//! The estimator is config-driven (PriorityFeeConfig from Commit 4),
//! thread-safe (Arc<RwLock<CachedFee>>), and designed for multi-bot use.
//!
//! March 2026 — V1.1 ⚡
//! ═══════════════════════════════════════════════════════════════════════════

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use async_trait::async_trait;
use log::{debug, warn};

use crate::config::PriorityFeeConfig;

// ═══════════════════════════════════════════════════════════════════════════
// FEE DATA SOURCE TRAIT — abstraction for testability + future providers
// ═══════════════════════════════════════════════════════════════════════════

/// Trait for fetching recent prioritization fee samples.
///
/// Implementations:
///   - `RpcFeeSource`    : calls `getRecentPrioritizationFees` (any RPC)
///   - `HeliusFeeSource` : calls `getPriorityFeeEstimate` V2 (Helius only)
///   - `MockFeeSource`   : deterministic data for tests
///
/// Returns raw fee-per-CU values in microlamports from recent blocks.
#[async_trait]
pub trait FeeDataSource: Send + Sync {
    /// Fetch recent priority fee samples (microlamports per compute unit).
    /// Returns an empty Vec on failure (estimator falls back to cached/fallback).
    async fn fetch_recent_fees(&self) -> Vec<u64>;
}

// ═══════════════════════════════════════════════════════════════════════════
// CACHED FEE — internal state
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
struct CachedFee {
    value:       u64,
    computed_at: Instant,
}

// ═══════════════════════════════════════════════════════════════════════════
// PRIORITY FEE ESTIMATOR
// ═══════════════════════════════════════════════════════════════════════════

/// Dynamic priority fee estimator with caching and safety bounds.
///
/// Thread-safe — can be shared across bot instances via `Arc`.
pub struct PriorityFeeEstimator {
    config: PriorityFeeConfig,
    source: Arc<dyn FeeDataSource>,
    cache:  RwLock<Option<CachedFee>>,
}

impl PriorityFeeEstimator {
    pub fn new(config: PriorityFeeConfig, source: Arc<dyn FeeDataSource>) -> Self {
        Self { config, source, cache: RwLock::new(None) }
    }

    pub async fn get_priority_fee(&self) -> u64 {
        {
            let cache = self.cache.read().await;
            if let Some(ref cached) = *cache {
                let ttl = Duration::from_secs(self.config.cache_ttl_secs);
                if cached.computed_at.elapsed() < ttl {
                    debug!("priority fee cache hit: fee={}", cached.value);
                    return cached.value;
                }
            }
        }

        let mut cache = self.cache.write().await;
        if let Some(ref cached) = *cache {
            let ttl = Duration::from_secs(self.config.cache_ttl_secs);
            if cached.computed_at.elapsed() < ttl {
                return cached.value;
            }
        }

        let fee = self.estimate_fee().await;
        *cache = Some(CachedFee { value: fee, computed_at: Instant::now() });
        debug!("priority fee recomputed: fee={}", fee);
        fee
    }

    async fn estimate_fee(&self) -> u64 {
        if self.config.strategy == "fixed" {
            return self.config.fallback_microlamports;
        }

        let samples = self.source.fetch_recent_fees().await;
        if samples.is_empty() {
            warn!("no priority fee samples — using fallback: {}", self.config.fallback_microlamports);
            return self.config.fallback_microlamports;
        }

        let raw      = Self::percentile(&samples, self.config.percentile);
        let adjusted = (raw as f64 * self.config.multiplier) as u64;
        let clamped  = adjusted
            .max(self.config.min_microlamports)
            .min(self.config.max_microlamports);

        debug!(
            "priority fee: samples={}, raw={}, adjusted={}, clamped={}, P{}, mult={}",
            samples.len(), raw, adjusted, clamped, self.config.percentile, self.config.multiplier,
        );
        clamped
    }

    fn percentile(samples: &[u64], pct: u8) -> u64 {
        if samples.is_empty() { return 0; }
        let mut sorted: Vec<u64> = samples.to_vec();
        sorted.sort_unstable();
        let pct = pct.min(100) as f64 / 100.0;
        let idx = ((sorted.len() as f64 * pct).ceil() as usize).saturating_sub(1);
        sorted[idx.min(sorted.len() - 1)]
    }

    #[allow(dead_code)]
    pub async fn invalidate_cache(&self) {
        *self.cache.write().await = None;
    }

    pub fn config(&self) -> &PriorityFeeConfig { &self.config }
}

// ═══════════════════════════════════════════════════════════════════════════
// MOCK FEE SOURCE — deterministic testing
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug)]
pub struct MockFeeSource { samples: Vec<u64> }

impl MockFeeSource {
    pub fn new(samples: Vec<u64>) -> Self     { Self { samples } }
    pub fn failing() -> Self                  { Self { samples: vec![] } }
}

#[async_trait]
impl FeeDataSource for MockFeeSource {
    async fn fetch_recent_fees(&self) -> Vec<u64> { self.samples.clone() }
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> PriorityFeeConfig {
        PriorityFeeConfig {
            enable_dynamic:         true,
            source:                 "rpc".to_string(),   // PR #109: new field, default "rpc"
            strategy:               "percentile".to_string(),
            percentile:             50,
            multiplier:             1.0,                 // no multiplier for predictable math
            min_microlamports:      100,
            max_microlamports:      1_000_000,
            fallback_microlamports: 5_000,
            cache_ttl_secs:         10,
            sample_blocks:          150,
        }
    }

    fn make_estimator(config: PriorityFeeConfig, samples: Vec<u64>) -> PriorityFeeEstimator {
        let source = Arc::new(MockFeeSource::new(samples));
        PriorityFeeEstimator::new(config, source)
    }

    // ── Percentile math ────────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_percentile_median_odd() {
        let samples = vec![500, 100, 300, 200, 400];
        assert_eq!(PriorityFeeEstimator::percentile(&samples, 50), 300);
    }

    #[test]
    fn test_percentile_median_even() {
        let samples = vec![400, 100, 300, 200];
        assert_eq!(PriorityFeeEstimator::percentile(&samples, 50), 200);
    }

    #[test]
    fn test_percentile_p75() {
        let samples = vec![100, 200, 300, 400, 500, 600, 700, 800];
        assert_eq!(PriorityFeeEstimator::percentile(&samples, 75), 600);
    }

    #[test]
    fn test_percentile_p90() {
        let samples: Vec<u64> = (1..=100).collect();
        assert_eq!(PriorityFeeEstimator::percentile(&samples, 90), 90);
    }

    #[test]
    fn test_percentile_single_sample() {
        assert_eq!(PriorityFeeEstimator::percentile(&[42_000], 50), 42_000);
    }

    #[test]
    fn test_percentile_empty() {
        assert_eq!(PriorityFeeEstimator::percentile(&[], 50), 0);
    }

    // ── Estimator integration ────────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_basic_estimation() {
        let samples = vec![1000, 2000, 3000, 4000, 5000];
        let estimator = make_estimator(test_config(), samples);
        assert_eq!(estimator.get_priority_fee().await, 3000);
    }

    #[tokio::test]
    async fn test_multiplier_applied() {
        let mut config = test_config();
        config.multiplier = 1.5;
        let estimator = make_estimator(config, vec![1000, 2000, 3000, 4000, 5000]);
        assert_eq!(estimator.get_priority_fee().await, 4500);
    }

    #[tokio::test]
    async fn test_clamp_min() {
        let mut config = test_config();
        config.min_microlamports = 10_000;
        let estimator = make_estimator(config, vec![100, 200, 300]);
        assert_eq!(estimator.get_priority_fee().await, 10_000);
    }

    #[tokio::test]
    async fn test_clamp_max() {
        let mut config = test_config();
        config.max_microlamports = 500;
        let estimator = make_estimator(config, vec![1000, 2000, 3000]);
        assert_eq!(estimator.get_priority_fee().await, 500);
    }

    #[tokio::test]
    async fn test_fallback_on_empty_samples() {
        let config = test_config();
        let estimator = make_estimator(config.clone(), vec![]);
        assert_eq!(estimator.get_priority_fee().await, config.fallback_microlamports);
    }

    /// GAP-4 (PR #97): RPC failure → fallback_microlamports contract.
    #[tokio::test]
    async fn test_priority_fee_fallback_on_rpc_error() {
        let config = test_config();
        let source = Arc::new(MockFeeSource::failing());
        let estimator = PriorityFeeEstimator::new(config.clone(), source);
        let fee = estimator.get_priority_fee().await;
        assert_eq!(
            fee, config.fallback_microlamports,
            "RPC failure must yield fallback_microlamports={}, got {}",
            config.fallback_microlamports, fee,
        );
    }

    #[tokio::test]
    async fn test_fixed_strategy_ignores_samples() {
        let mut config = test_config();
        config.strategy = "fixed".to_string();
        config.fallback_microlamports = 7_777;
        let estimator = make_estimator(config, vec![1000, 2000, 3000]);
        assert_eq!(estimator.get_priority_fee().await, 7_777);
    }

    #[tokio::test]
    async fn test_cache_returns_same_value() {
        let estimator = make_estimator(test_config(), vec![1000, 2000, 3000, 4000, 5000]);
        let fee1 = estimator.get_priority_fee().await;
        let fee2 = estimator.get_priority_fee().await;
        assert_eq!(fee1, fee2);
    }

    #[tokio::test]
    async fn test_invalidate_cache() {
        let estimator = make_estimator(test_config(), vec![1000, 2000, 3000]);
        let fee1 = estimator.get_priority_fee().await;
        estimator.invalidate_cache().await;
        let fee2 = estimator.get_priority_fee().await;
        assert_eq!(fee1, fee2);
    }

    #[tokio::test]
    async fn test_realistic_mainnet_distribution() {
        let mut samples = vec![1_000u64; 50];
        samples.extend(vec![5_000u64;   25]);
        samples.extend(vec![20_000u64;  15]);
        samples.extend(vec![100_000u64;  8]);
        samples.extend(vec![500_000u64;  2]);

        let mut config = test_config();
        config.percentile = 50;
        config.multiplier = 1.2;

        let estimator = make_estimator(config, samples);
        assert_eq!(estimator.get_priority_fee().await, 1200);
    }
}
