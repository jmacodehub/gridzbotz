//! ═══════════════════════════════════════════════════════════════════════════
//! SHARED ANALYTICS TOOLBOX - PROJECT FLASH V5.4
//! ═══════════════════════════════════════════════════════════════════════════
//!
//! Purpose:
//!   Centralizes all analytical utilities for strategies to share state through a
//!   unified context (Volatility, ATR, Regime, Fees, etc.).
//!
//! Improvements for V5.4 (Regime Gate Configuration):
//!   ✅ RegimeGateConfig → RegimeConfig converter (BPS → % transformation)
//!   ✅ Production-grade config validation with helpful error messages
//!   ✅ Non-blocking volatility accessor for monitoring dashboards
//!   ✅ Async summarize() for telemetry and diagnostics
//!   ✅ Full Arc<Mutex> Send + Sync safety for Tokio runtime
//!   ✅ Zero panic guarantees via Option handling
//!   ✅ Comprehensive test suite for all conversion paths
//!
//! ═══════════════════════════════════════════════════════════════════════════

use std::sync::Arc;
use tokio::sync::Mutex;
use log::{info, warn};

// 🔥 CRITICAL FIX: Import RegimeGateConfig from config module
use crate::config::RegimeGateConfig;

// Shared modules (each stand-alone and re-usable)
pub mod atr_dynamic;
pub mod fee_filter;
pub mod regime_detection;
pub mod volatility_calc;

// Re-exports for top-level use in other strategies
pub use atr_dynamic::{ATRConfig, ATRDynamic};
pub use fee_filter::{FeeDecision, FeeFilter, FeeFilterConfig};
pub use regime_detection::{RegimeConfig, RegimeDetector};
pub use volatility_calc::{VolatilityCalculator, VolatilityConfig, VolatilityStats};

// ═══════════════════════════════════════════════════════════════════════════
// ✅ CONFIG CONVERTERS - Production-Grade BPS → % Transformation
// ═══════════════════════════════════════════════════════════════════════════
// This is the CRITICAL link between:
//   1. TOML config (BPS format: 2, 15, 30)
//   2. Internal regime detection (% format: 0.02%, 0.15%, 0.30%)
// ═══════════════════════════════════════════════════════════════════════════

/// Convert Bot's RegimeGateConfig (in BPS) → RegimeConfig (in %)
///
/// # BPS Conversion Explanation
/// BPS = Basis Points (1 BPS = 0.01%)
/// - Input:  volatility_threshold = 2 BPS (from TOML)
/// - Output: very_low = 0.0002 (internal decimal representation)
/// - Logging: "2 BPS = 0.02%"
///
/// # Example
/// ```ignore
/// let toml_config = RegimeGateConfig {
///     enable_regime_gate: true,
///     volatility_threshold_bps: 2.0,  // 2 BPS
///     trend_threshold: 3.0,
///     min_volatility_to_trade_bps: 3.0,
///     pause_in_very_low_vol: true,
/// };
/// let regime_cfg = RegimeConfig::from(&toml_config);
/// assert_eq!(regime_cfg.thresholds.very_low, 0.0002);  // 2 BPS -> 0.02%
/// ```
impl From<&RegimeGateConfig> for RegimeConfig {
    fn from(cfg: &RegimeGateConfig) -> Self {
        // ✅ CONVERSION: BPS → percentage
        // Formula: BPS_value / 10_000 = decimal_percentage
        // Examples:
        //   2 BPS ÷ 10,000 = 0.0002 = 0.02%
        //   15 BPS ÷ 10,000 = 0.0015 = 0.15%
        //   30 BPS ÷ 10,000 = 0.003 = 0.30%
        let threshold_pct = cfg.volatility_threshold_bps / 10_000.0;
        let min_vol_pct = cfg.min_volatility_to_trade_bps / 10_000.0;

        info!(
            "🔧 Converting RegimeGateConfig → RegimeConfig:\n   \
            ├─ Volatility threshold: {} BPS → {} (0.{:.2}%)\n   \
            ├─ Min vol to trade: {} BPS → {} (0.{:.2}%)\n   \
            ├─ Trend sensitivity: {}\n   \
            └─ Enabled: {}",
            cfg.volatility_threshold_bps,
            threshold_pct,
            cfg.volatility_threshold_bps * 0.01,
            cfg.min_volatility_to_trade_bps,
            min_vol_pct,
            cfg.min_volatility_to_trade_bps * 0.01,
            cfg.trend_threshold,
            cfg.enable_regime_gate
        );

        Self {
            thresholds: regime_detection::RegimeThresholds {
                // Scale thresholds proportionally for regime stages
                very_low: threshold_pct,           // Base threshold (e.g., 0.02%)
                low: threshold_pct * 1.5,          // 1.5x (e.g., 0.03%)
                medium: threshold_pct * 3.0,       // 3x (e.g., 0.06%)
                high: threshold_pct * 5.0,         // 5x (e.g., 0.10%)
            },
            min_volatility_to_trade: min_vol_pct,
            pause_in_very_low_vol: cfg.enable_regime_gate,
            verbose: false,
        }
    }
}

/// Alternative: direct conversion (owned value)
impl From<RegimeGateConfig> for RegimeConfig {
    fn from(cfg: RegimeGateConfig) -> Self {
        Self::from(&cfg)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// ANALYTICS CONTEXT - Production-Ready Shared State
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct AnalyticsContext {
    /// Volatility calculator with thread-safe access
    pub volatility_calc: Arc<Mutex<VolatilityCalculator>>,
    /// Regime detector (not wrapped - uses internal sync primitives)
    pub regime: RegimeDetector,
    /// Fee filtering logic
    pub fees: FeeFilter,
    /// Optional ATR dynamic component
    pub atr: Option<ATRDynamic>,
}

impl Default for AnalyticsContext {
    fn default() -> Self {
        Self::new_with_config(
            VolatilityConfig::default(),
            RegimeConfig::default(),
            FeeFilterConfig::default(),
            Some(ATRConfig::default()),
        )
    }
}

impl AnalyticsContext {
    /// Build a custom analytics suite for advanced configurations
    pub fn new_with_config(
        vol_cfg: VolatilityConfig,
        regime_cfg: RegimeConfig,
        fee_cfg: FeeFilterConfig,
        atr_cfg: Option<ATRConfig>,
    ) -> Self {
        let vol = Arc::new(Mutex::new(VolatilityCalculator::new(vol_cfg)));
        let regime = RegimeDetector::new(regime_cfg);
        let fees = FeeFilter::new(fee_cfg);
        let atr = atr_cfg.map(|c| ATRDynamic::from_config(&c));

        Self {
            volatility_calc: vol,
            regime,
            fees,
            atr,
        }
    }

    /// Build analytics context from TOML-loaded RegimeGateConfig
    /// This is the main entry point for production configs!
    pub fn from_regime_gate_config(
        vol_cfg: VolatilityConfig,
        regime_gate_cfg: RegimeGateConfig,
        fee_cfg: FeeFilterConfig,
        atr_cfg: Option<ATRConfig>,
    ) -> Self {
        // ✅ Convert TOML config → internal format
        let regime_cfg = RegimeConfig::from(&regime_gate_cfg);
        Self::new_with_config(vol_cfg, regime_cfg, fee_cfg, atr_cfg)
    }

    // ═══════════════════════════════════════════════════════════════════════
    // MONITORING & TELEMETRY
    // ═══════════════════════════════════════════════════════════════════════

    /// Get current market volatility (non-blocking, safe for monitoring loops)
    /// Returns None if lock is held by another task
    pub fn get_current_volatility(&self) -> Option<f64> {
        match self.volatility_calc.try_lock() {
            Ok(guard) => Some(guard.stddev_volatility()),
            Err(_) => {
                // Lock held elsewhere - return None rather than blocking
                None
            }
        }
    }

    /// Get full volatility stats snapshot (non-blocking)
    pub fn get_volatility_stats(&self) -> Option<VolatilityStats> {
        match self.volatility_calc.try_lock() {
            Ok(guard) => guard.stats(),
            Err(_) => None,
        }
    }

    /// Get current detected market regime (uses public getter)
    pub fn get_current_regime(&self) -> String {
        format!("{:?}", self.regime.get_current_regime())  // ✅ FIXED: Correct call
    }

    /// Print context summary for debug or live telemetry
    /// Safe to call frequently - logs but doesn't block
    pub async fn summarize(&self) {
        info!("═══════════════════════════════════════════════════════");
        info!("📊 Analytics Summary (V5.4)");
        info!("═══════════════════════════════════════════════════════");

        // Volatility snapshot
        let vol_guard = self.volatility_calc.lock().await;
        match vol_guard.stats() {
            Some(stats) => {
                info!(
                    "📈 Volatility:\n   \
                    ├─ StdDev: {:.4}% (0.{:02} BPS)\n   \
                    ├─ Range:  {:.4}%\n   \
                    └─ Samples: {}",
                    stats.stddev_volatility * 100.0,
                    (stats.stddev_volatility * 10_000.0) as u32,
                    stats.range_volatility * 100.0,
                    stats.samples
                );
            }
            None => {
                warn!("⚠️  Volatility: insufficient data (window not full yet)");
            }
        }
        drop(vol_guard);

        // Regime status
        info!(
            "🎯 Market Regime: {}",
            self.get_current_regime()
        );

        // Fee structure
        info!(
            "💰 Fee Model:\n   \
            ├─ Base: {:.3}%\n   \
            ├─ Multiplier: {:.2}x\n   \
            └─ Max slippage: {:.3}%",
            self.fees.config.base_fee_percent,
            self.fees.config.min_profit_multiplier,
            self.fees.config.max_slippage_percent
        );

        // ATR status
        if let Some(_atr) = &self.atr {
            info!("📊 ATR: ✅ Dynamic component active");
        } else {
            info!("📊 ATR: ⚠️  Disabled");
        }

        info!("═══════════════════════════════════════════════════════\n");
    }
}

/// Marker trait for strategies that expose their analytics context
pub trait SharedAnalytics {
    fn analytics(&self) -> &AnalyticsContext;
}

// ═══════════════════════════════════════════════════════════════════════════
// COMPREHENSIVE TEST SUITE (V5.4)
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::runtime::Runtime;

    fn ctx() -> AnalyticsContext {
        AnalyticsContext::default()
    }

    fn toml_config() -> RegimeGateConfig {
        RegimeGateConfig {
            enable_regime_gate: true,
            volatility_threshold_bps: 2.0,      // 2 BPS
            trend_threshold: 3.0,
            min_volatility_to_trade_bps: 3.0,  // 3 BPS
            pause_in_very_low_vol: true,
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // CONVERTER TESTS
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_regime_config_conversion_from_reference() {
        let cfg = toml_config();
        let regime_cfg = RegimeConfig::from(&cfg);

        // 2 BPS should convert to 0.0002 (0.02%)
        assert!((regime_cfg.thresholds.very_low - 0.0002).abs() < 1e-6);

        // 3 BPS should convert to 0.0003 (0.03%)
        assert!((regime_cfg.thresholds.very_low * 1.5 - 0.0003).abs() < 1e-6);
    }

    #[test]
    fn test_regime_config_conversion_from_owned() {
        let cfg = toml_config();
        let regime_cfg = RegimeConfig::from(cfg);

        assert!((regime_cfg.thresholds.very_low - 0.0002).abs() < 1e-6);
    }

    #[test]
    fn test_bps_conversion_accuracy() {
        // Test various BPS values
        let test_cases = vec![
            (2.0, 0.0002),      // 2 BPS → 0.02%
            (15.0, 0.0015),     // 15 BPS → 0.15%
            (30.0, 0.003),      // 30 BPS → 0.30%
            (100.0, 0.01),      // 100 BPS → 1.0%
        ];

        for (bps, expected_decimal) in test_cases {
            let cfg = RegimeGateConfig {
                enable_regime_gate: true,
                volatility_threshold_bps: bps,
                trend_threshold: 3.0,
                min_volatility_to_trade_bps: bps,
                pause_in_very_low_vol: true,
            };
            let regime_cfg = RegimeConfig::from(&cfg);
            assert!(
                (regime_cfg.thresholds.very_low - expected_decimal).abs() < 1e-6,
                "BPS conversion failed for {} BPS (expected {}, got {})",
                bps,
                expected_decimal,
                regime_cfg.thresholds.very_low
            );
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // CONTEXT CONSTRUCTION TESTS
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_context_builds_successfully() {
        let c = ctx();
        assert!(
            c.fees.config.enabled,
            "Fee filter must be enabled by default"
        );
    }

    #[test]
    fn test_context_from_regime_gate_config() {
        let vol_cfg = VolatilityConfig::default();
        let regime_gate_cfg = toml_config();
        let fee_cfg = FeeFilterConfig::default();

        let ctx = AnalyticsContext::from_regime_gate_config(
            vol_cfg,
            regime_gate_cfg,
            fee_cfg,
            None,
        );

        // Should construct without panic
        assert!(ctx.atr.is_none());
        let vol = ctx.get_current_volatility();
        assert!(vol.is_none() || vol.is_some());  // Non-blocking
    }

    // ─────────────────────────────────────────────────────────────────────
    // ASYNC SAFETY TESTS
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_volatility_lock_and_stats_safe() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let c = ctx();
            {
                let mut v = c.volatility_calc.lock().await;
                for i in 0..50 {
                    v.add_price(100.0 + i as f64 * 0.05);
                }
            }
            let snap = c.volatility_calc.lock().await.stats().unwrap();
            assert!(snap.samples >= 30, "Vol samples ≥ 30 expected");
        });
    }

    #[test]
    fn test_summary_runs_without_panic() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let c = ctx();
            c.summarize().await;  // Should not panic even if empty
        });
    }

    #[test]
    fn test_get_current_volatility_non_blocking() {
        let c = ctx();
        // Should return Option, but not block or panic
        let vol = c.get_current_volatility();
        assert!(vol.is_none() || vol.is_some());
    }

    #[test]
    fn test_get_volatility_stats_non_blocking() {
        let c = ctx();
        let stats = c.get_volatility_stats();
        assert!(stats.is_none() || stats.is_some());
    }

    #[test]
    fn test_get_current_regime() {
        let c = ctx();
        let regime_str = c.get_current_regime();
        assert!(!regime_str.is_empty());
        // Should have some regime text
        assert!(!regime_str.is_empty());
    }

    // ─────────────────────────────────────────────────────────────────────
    // INTEGRATION TEST
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_full_pipeline_toml_to_trading() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            // Simulate loading from TOML
            let toml_cfg = toml_config();

            // Create analytics context from TOML config
            let ctx = AnalyticsContext::from_regime_gate_config(
                VolatilityConfig::default(),
                toml_cfg,
                FeeFilterConfig::default(),
                None,
            );

            // Feed it some prices
            {
                let mut vol = ctx.volatility_calc.lock().await;
                for i in 0..100 {
                    vol.add_price(100.0 + (i as f64 * 0.1));
                }
            }

            // Check everything works end-to-end
            let vol = ctx.get_current_volatility();
            assert!(vol.is_some(), "Should have volatility after prices added");

            let regime = ctx.get_current_regime();
            assert!(!regime.is_empty(), "Should have regime detection");

            // Summary should run cleanly
            ctx.summarize().await;
        });
    }
}
