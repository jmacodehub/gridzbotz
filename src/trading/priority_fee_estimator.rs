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
//!                        (RPC impl | Mock impl)
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
///   - `RpcFeeSource` (Commit 6): calls `getRecentPrioritizationFees`
///   - `MockFeeSource` (below): deterministic data for tests
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
    /// The computed priority fee in microlamports.
    value: u64,
    /// When this value was computed.
    computed_at: Instant,
}

// ═══════════════════════════════════════════════════════════════════════════
// PRIORITY FEE ESTIMATOR
// ═══════════════════════════════════════════════════════════════════════════

/// Dynamic priority fee estimator with caching and safety bounds.
///
/// Thread-safe — can be shared across bot instances via `Arc`.
///
/// ## Usage
///
/// ```ignore
/// let estimator = PriorityFeeEstimator::new(config.priority_fees.clone(), source);
/// let fee = estimator.get_priority_fee().await; // cached, fast
/// ```
pub struct PriorityFeeEstimator {
    config: PriorityFeeConfig,
    source: Arc<dyn FeeDataSource>,
    cache: RwLock<Option<CachedFee>>,
}

impl PriorityFeeEstimator {
    /// Create a new estimator with the given config and data source.
    pub fn new(config: PriorityFeeConfig, source: Arc<dyn FeeDataSource>) -> Self {
        Self {
            config,
            source,
            cache: RwLock::new(None),
        }
    }

    /// Get the current priority fee in microlamports.
    ///
    /// Returns cached value if still within TTL, otherwise recomputes.
    /// Falls back to `fallback_microlamports` if estimation fails.
    pub async fn get_priority_fee(&self) -> u64 {
        // Fast path: check cache under read lock
        {
            let cache = self.cache.read().await;
            if let Some(ref cached) = *cache {
                let ttl = Duration::from_secs(self.config.cache_ttl_secs);
                if cached.computed_at.elapsed() < ttl {
                    debug!(
                        "priority fee cache hit: fee={}, age_ms={}",
                        cached.value,
                        cached.computed_at.elapsed().as_millis()
                    );
                    return cached.value;
                }
            }
        }

        // Slow path: recompute under write lock
        let mut cache = self.cache.write().await;

        // Double-check: another task may have refreshed while we waited
        if let Some(ref cached) = *cache {
            let ttl = Duration::from_secs(self.config.cache_ttl_secs);
            if cached.computed_at.elapsed() < ttl {
                return cached.value;
            }
        }

        let fee = self.estimate_fee().await;
        *cache = Some(CachedFee {
            value: fee,
            computed_at: Instant::now(),
        });

        debug!("priority fee recomputed: fee={}", fee);
        fee
    }

    /// Core estimation logic — fetch samples, compute percentile, apply bounds.
    async fn estimate_fee(&self) -> u64 {
        // Fixed strategy: skip RPC entirely
        if self.config.strategy == "fixed" {
            debug!("using fixed priority fee: fee={}", self.config.fallback_microlamports);
            return self.config.fallback_microlamports;
        }

        // Fetch raw fee samples
        let samples = self.source.fetch_recent_fees().await;

        if samples.is_empty() {
            warn!(
                "no priority fee samples — using fallback: {}",
                self.config.fallback_microlamports
            );
            return self.config.fallback_microlamports;
        }

        // Compute percentile
        let raw = Self::percentile(&samples, self.config.percentile);

        // Apply multiplier
        let adjusted = (raw as f64 * self.config.multiplier) as u64;

        // Clamp to safety bounds
        let clamped = adjusted
            .max(self.config.min_microlamports)
            .min(self.config.max_microlamports);

        debug!(
            "priority fee estimated: samples={}, raw={}, adjusted={}, clamped={}, P{}, mult={}",
            samples.len(),
            raw,
            adjusted,
            clamped,
            self.config.percentile,
            self.config.multiplier,
        );

        clamped
    }

    /// Compute the Nth percentile from a set of fee samples.
    ///
    /// Uses nearest-rank method (standard for discrete fee data).
    /// Percentile range: 0-100 (clamped internally).
    fn percentile(samples: &[u64], pct: u8) -> u64 {
        if samples.is_empty() {
            return 0;
        }

        let mut sorted: Vec<u64> = samples.to_vec();
        sorted.sort_unstable();

        let pct = pct.min(100) as f64 / 100.0;
        let idx = ((sorted.len() as f64 * pct).ceil() as usize).saturating_sub(1);
        let idx = idx.min(sorted.len() - 1);

        sorted[idx]
    }

    /// Force-expire the cache (useful for testing or config hot-reload).
    #[allow(dead_code)]
    pub async fn invalidate_cache(&self) {
        let mut cache = self.cache.write().await;
        *cache = None;
    }

    /// Get config reference (for logging/diagnostics).
    pub fn config(&self) -> &PriorityFeeConfig {
        &self.config
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// MOCK FEE SOURCE — deterministic testing
// ═══════════════════════════════════════════════════════════════════════════

/// Mock fee data source for deterministic tests.
///
/// Returns a fixed set of fee samples, or an empty Vec to simulate RPC failure.
#[derive(Debug)]
pub struct MockFeeSource {
    samples: Vec<u64>,
}

impl MockFeeSource {
    /// Create a mock source that returns the given samples.
    pub fn new(samples: Vec<u64>) -> Self {
        Self { samples }
    }

    /// Create a mock source that simulates RPC failure (empty samples).
    pub fn failing() -> Self {
        Self { samples: vec![] }
    }
}

#[async_trait]
impl FeeDataSource for MockFeeSource {
    async fn fetch_recent_fees(&self) -> Vec<u64> {
        self.samples.clone()
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
            enable_dynamic: true,
            strategy: "percentile".to_string(),
            percentile: 50,
            multiplier: 1.0, // no multiplier for predictable math
            min_microlamports: 100,
            max_microlamports: 1_000_000,
            fallback_microlamports: 5_000,
            cache_ttl_secs: 10,
            sample_blocks: 150,
        }
    }

    fn make_estimator(config: PriorityFeeConfig, samples: Vec<u64>) -> PriorityFeeEstimator {
        let source = Arc::new(MockFeeSource::new(samples));
        PriorityFeeEstimator::new(config, source)
    }

    // ── Percentile math ───────────────────────────────────────────────────────────────────

    #[test]
    fn test_percentile_median_odd() {
        // [100, 200, 300, 400, 500] → P50 = 300
        let samples = vec![500, 100, 300, 200, 400];
        assert_eq!(PriorityFeeEstimator::percentile(&samples, 50), 300);
    }

    #[test]
    fn test_percentile_median_even() {
        // [100, 200, 300, 400] → P50 = 200 (nearest-rank, floor)
        let samples = vec![400, 100, 300, 200];
        assert_eq!(PriorityFeeEstimator::percentile(&samples, 50), 200);
    }

    #[test]
    fn test_percentile_p75() {
        // [100, 200, 300, 400, 500, 600, 700, 800] → P75 = 600
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

    // ── Estimator integration ───────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_basic_estimation() {
        let samples = vec![1000, 2000, 3000, 4000, 5000];
        let estimator = make_estimator(test_config(), samples);
        let fee = estimator.get_priority_fee().await;
        assert_eq!(fee, 3000); // P50, multiplier=1.0
    }

    #[tokio::test]
    async fn test_multiplier_applied() {
        let mut config = test_config();
        config.multiplier = 1.5;
        let samples = vec![1000, 2000, 3000, 4000, 5000];
        let estimator = make_estimator(config, samples);
        let fee = estimator.get_priority_fee().await;
        assert_eq!(fee, 4500); // 3000 * 1.5
    }

    #[tokio::test]
    async fn test_clamp_min() {
        let mut config = test_config();
        config.min_microlamports = 10_000;
        let samples = vec![100, 200, 300]; // P50 = 200, way below min
        let estimator = make_estimator(config, samples);
        let fee = estimator.get_priority_fee().await;
        assert_eq!(fee, 10_000); // clamped to floor
    }

    #[tokio::test]
    async fn test_clamp_max() {
        let mut config = test_config();
        config.max_microlamports = 500;
        let samples = vec![1000, 2000, 3000]; // P50 = 2000, above max
        let estimator = make_estimator(config, samples);
        let fee = estimator.get_priority_fee().await;
        assert_eq!(fee, 500); // clamped to ceiling
    }

    #[tokio::test]
    async fn test_fallback_on_empty_samples() {
        // Generic empty-vec path (e.g. RPC returns 0 fee entries for the block window).
        // See also: test_priority_fee_fallback_on_rpc_error for the RPC-error-specific contract.
        let config = test_config();
        let estimator = make_estimator(config.clone(), vec![]);
        let fee = estimator.get_priority_fee().await;
        assert_eq!(fee, config.fallback_microlamports);
    }

    /// GAP-4 (PR #97): RPC failure → fallback_microlamports contract.
    ///
    /// `MockFeeSource::failing()` simulates `getRecentPrioritizationFees` returning
    /// an error (or an empty response due to connection failure). The estimator MUST
    /// return `fallback_microlamports` — never 0, never panic.
    ///
    /// This test explicitly documents the RPC-error safety contract and closes
    /// GAP-4 in the V3 audit trail.
    #[tokio::test]
    async fn test_priority_fee_fallback_on_rpc_error() {
        let config = test_config(); // fallback_microlamports = 5_000
        let source = Arc::new(MockFeeSource::failing()); // simulates RPC error
        let estimator = PriorityFeeEstimator::new(config.clone(), source);

        let fee = estimator.get_priority_fee().await;

        assert_eq!(
            fee,
            config.fallback_microlamports,
            "RPC failure must yield fallback_microlamports={}, got {}",
            config.fallback_microlamports,
            fee,
        );
    }

    #[tokio::test]
    async fn test_fixed_strategy_ignores_samples() {
        let mut config = test_config();
        config.strategy = "fixed".to_string();
        config.fallback_microlamports = 7_777;
        let samples = vec![1000, 2000, 3000]; // should be ignored
        let estimator = make_estimator(config, samples);
        let fee = estimator.get_priority_fee().await;
        assert_eq!(fee, 7_777);
    }

    #[tokio::test]
    async fn test_cache_returns_same_value() {
        let samples = vec![1000, 2000, 3000, 4000, 5000];
        let estimator = make_estimator(test_config(), samples);

        let fee1 = estimator.get_priority_fee().await;
        let fee2 = estimator.get_priority_fee().await;
        assert_eq!(fee1, fee2); // second call hits cache
    }

    #[tokio::test]
    async fn test_invalidate_cache() {
        let samples = vec![1000, 2000, 3000];
        let estimator = make_estimator(test_config(), samples);

        let fee1 = estimator.get_priority_fee().await;
        estimator.invalidate_cache().await;

        // After invalidation, re-fetches (same mock data, same result)
        let fee2 = estimator.get_priority_fee().await;
        assert_eq!(fee1, fee2);
    }

    #[tokio::test]
    async fn test_realistic_mainnet_distribution() {
        // Simulates real mainnet fee distribution:
        // Many low fees, some medium, few high (long tail)
        let mut samples = vec![1_000u64; 50];        // 50% at 1K
        samples.extend(vec![5_000u64; 25]);           // 25% at 5K
        samples.extend(vec![20_000u64; 15]);          // 15% at 20K
        samples.extend(vec![100_000u64; 8]);          // 8% at 100K
        samples.extend(vec![500_000u64; 2]);          // 2% at 500K (spam)

        let mut config = test_config();
        config.percentile = 50;
        config.multiplier = 1.2;

        let estimator = make_estimator(config, samples);
        let fee = estimator.get_priority_fee().await;

        // P50 of this distribution = 1000 (median is in the big cluster)
        // × 1.2 = 1200
        assert_eq!(fee, 1200);
    }
}
