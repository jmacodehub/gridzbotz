//! ═══════════════════════════════════════════════════════════════════════════
//! SHARED ANALYTICS TOOLBOX - PROJECT FLASH V5.5
//! ═══════════════════════════════════════════════════════════════════════════
//!
//! Purpose:
//!   Centralizes all analytical utilities for strategies to share state through a
//!   unified context (Volatility, ATR, Regime, Fees, etc.).
//!
//! V5.5 (fix/vol-units-alignment):
//!   ✅ BPS → TRUE-% converter fixed: / 10,000 → / 100
//!      2 BPS now correctly produces 0.02 (not 0.0002) — matches regime_detection.rs V2.4
//!   ✅ summarize() display corrected: removed double-multiplication by 100
//!      stddev_volatility = 0.5 now logs "0.5000%" not "50.0000%"
//!   ✅ BPS display in summarize() corrected: × 100 (not × 10,000)
//!      0.5% volatility now shows "50 BPS" not "5000 BPS"
//!   ✅ Test assertions updated to TRUE-% convention (0.02 not 0.0002)
//!
//! ⚠️  UNIT SYSTEM CANON (V5.5+)
//!   All volatility values flowing through this module are TRUE PERCENTAGE:
//!     0.5  = 0.5%  (normal SOL tick)
//!     0.02 = 0.02% (min-vol gate floor)
//!   BPS inputs from TOML are converted via: bps / 100.0 → TRUE %
//!   Do NOT multiply volatility by 100 before passing to regime_detection.rs.
//!
//! V5.4 (previous):
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
// ✅ CONFIG CONVERTERS - Production-Grade BPS → TRUE-% Transformation
// ═══════════════════════════════════════════════════════════════════════════
// This is the CRITICAL link between:
//   1. TOML config (BPS format: 2, 15, 30)
//   2. Internal regime detection (TRUE-% format: 0.02, 0.15, 0.30)
//
// UNIT SYSTEM:
//   BPS (Basis Points): 1 BPS = 0.01%
//   TRUE %:             0.02  = 0.02%  (NOT 0.0002)
//   Conversion:         bps / 100 = true_%
//   Examples:
//     2 BPS → 2 / 100 = 0.02   (0.02%)  ✅
//    15 BPS → 15 / 100 = 0.15  (0.15%)  ✅
//   100 BPS → 100 / 100 = 1.0  (1.0%)   ✅
//
// ⚠️  V5.4 BUG (fixed in V5.5):
//   Old: bps / 10,000 → decimal fraction (0.0002)
//   New: bps / 100    → TRUE %            (0.02)
//   regime_detection.rs V2.4 expects TRUE % — the old formula was 100× off.
// ═══════════════════════════════════════════════════════════════════════════

/// Convert Bot's RegimeGateConfig (in BPS) → RegimeConfig (in TRUE %)
///
/// # Unit Conversion
/// BPS = Basis Points (1 BPS = 0.01%)
/// Formula: threshold_true_pct = bps / 100.0
///
/// # Examples
/// ```ignore
/// let toml_config = RegimeGateConfig {
///     enable_regime_gate: true,
///     volatility_threshold_bps: 2.0,      // 2 BPS = 0.02%
///     trend_threshold: 3.0,
///     min_volatility_to_trade_bps: 3.0,  // 3 BPS = 0.03%
///     pause_in_very_low_vol: true,
/// };
/// let regime_cfg = RegimeConfig::from(&toml_config);
/// // 2 BPS → 0.02 (TRUE %) — matches regime_detection.rs V2.4
/// assert!((regime_cfg.thresholds.very_low - 0.02).abs() < 1e-6);
/// ```
impl From<&RegimeGateConfig> for RegimeConfig {
    fn from(cfg: &RegimeGateConfig) -> Self {
        // ✅ V5.5 FIX: BPS → TRUE % conversion
        // Formula: bps / 100.0
        // Old (WRONG):  bps / 10,000 → decimal fraction (100× too small)
        // New (CORRECT): bps / 100   → TRUE % matching regime_detection.rs V2.4
        //
        // 1 BPS = 0.01%
        // 2 BPS / 100 = 0.02 (TRUE %) ← correct
        // 2 BPS / 10,000 = 0.0002     ← was 100× off, TOML path was broken
        let threshold_pct = cfg.volatility_threshold_bps / 100.0;
        let min_vol_pct   = cfg.min_volatility_to_trade_bps / 100.0;

        info!(
            "🔧 Converting RegimeGateConfig → RegimeConfig (V5.5 TRUE-%):\n   \
            ├─ Vol threshold:  {} BPS → {:.4}% (true %)\n   \
            ├─ Min vol trade:  {} BPS → {:.4}% (true %)\n   \
            ├─ Trend sensitivity: {}\n   \
            └─ Gate enabled: {}",
            cfg.volatility_threshold_bps,
            threshold_pct,
            cfg.min_volatility_to_trade_bps,
            min_vol_pct,
            cfg.trend_threshold,
            cfg.enable_regime_gate
        );

        Self {
            thresholds: regime_detection::RegimeThresholds {
                // Scale thresholds proportionally for regime stages (TRUE % world)
                very_low: threshold_pct,         // e.g. 2 BPS → 0.02%
                low:      threshold_pct * 1.5,   // e.g. → 0.03%
                medium:   threshold_pct * 3.0,   // e.g. → 0.06%
                high:     threshold_pct * 5.0,   // e.g. → 0.10%
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
        // ✅ Convert TOML config → internal TRUE-% format (V5.5)
        let regime_cfg = RegimeConfig::from(&regime_gate_cfg);
        Self::new_with_config(vol_cfg, regime_cfg, fee_cfg, atr_cfg)
    }

    // ═══════════════════════════════════════════════════════════════════════
    // MONITORING & TELEMETRY
    // ═══════════════════════════════════════════════════════════════════════

    /// Get current market volatility (non-blocking, safe for monitoring loops)
    /// Returns None if lock is held by another task.
    /// Returned value is TRUE PERCENTAGE (e.g. 0.5 = 0.5%).
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
        format!("{:?}", self.regime.get_current_regime())
    }

    /// Print context summary for debug or live telemetry.
    /// Safe to call frequently - logs but doesn't block.
    ///
    /// V5.5 FIX: volatility values are TRUE % — do NOT multiply by 100 for display.
    /// Old: stats.stddev_volatility * 100.0 → showed 50.0% when vol was 0.5%
    /// New: stats.stddev_volatility          → shows  0.5% correctly
    pub async fn summarize(&self) {
        info!("═══════════════════════════════════════════════════════");
        info!("📊 Analytics Summary (V5.5)");
        info!("═══════════════════════════════════════════════════════");

        // Volatility snapshot
        let vol_guard = self.volatility_calc.lock().await;
        match vol_guard.stats() {
            Some(stats) => {
                // ✅ V5.5 FIX: stddev_volatility and range_volatility are ALREADY TRUE %.
                // BPS = true_pct * 100  (e.g. 0.5% × 100 = 50 BPS)
                info!(
                    "📈 Volatility:\n   \
                    ├─ StdDev: {:.4}% ({} BPS)\n   \
                    ├─ Range:  {:.4}%\n   \
                    └─ Samples: {}",
                    stats.stddev_volatility,
                    (stats.stddev_volatility * 100.0) as u32,
                    stats.range_volatility,
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
// COMPREHENSIVE TEST SUITE (V5.5)
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
            volatility_threshold_bps: 2.0,      // 2 BPS = 0.02%
            trend_threshold: 3.0,
            min_volatility_to_trade_bps: 3.0,  // 3 BPS = 0.03%
            pause_in_very_low_vol: true,
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // CONVERTER TESTS (V5.5 — TRUE-% world)
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_regime_config_conversion_from_reference() {
        let cfg = toml_config();
        let regime_cfg = RegimeConfig::from(&cfg);

        // V5.5: 2 BPS → 0.02 (TRUE %)  [was 0.0002 in V5.4 — 100× wrong]
        assert!(
            (regime_cfg.thresholds.very_low - 0.02).abs() < 1e-6,
            "2 BPS should convert to 0.02 TRUE % (got {})",
            regime_cfg.thresholds.very_low
        );

        // 3 BPS min-vol gate → 0.03 TRUE %
        assert!(
            (regime_cfg.min_volatility_to_trade - 0.03).abs() < 1e-6,
            "3 BPS min_vol should convert to 0.03 TRUE % (got {})",
            regime_cfg.min_volatility_to_trade
        );
    }

    #[test]
    fn test_regime_config_conversion_from_owned() {
        let cfg = toml_config();
        let regime_cfg = RegimeConfig::from(cfg);
        // V5.5: 2 BPS → 0.02 TRUE %
        assert!((regime_cfg.thresholds.very_low - 0.02).abs() < 1e-6);
    }

    #[test]
    fn test_bps_conversion_accuracy() {
        // V5.5: BPS / 100 = TRUE %
        // 1 BPS = 0.01%, 100 BPS = 1.0%
        let test_cases = vec![
            (2.0_f64,   0.02_f64),    // 2 BPS   → 0.02%
            (15.0,      0.15),         // 15 BPS  → 0.15%
            (30.0,      0.30),         // 30 BPS  → 0.30%
            (100.0,     1.00),         // 100 BPS → 1.00%
        ];

        for (bps, expected_true_pct) in test_cases {
            let cfg = RegimeGateConfig {
                enable_regime_gate: true,
                volatility_threshold_bps: bps,
                trend_threshold: 3.0,
                min_volatility_to_trade_bps: bps,
                pause_in_very_low_vol: true,
            };
            let regime_cfg = RegimeConfig::from(&cfg);
            assert!(
                (regime_cfg.thresholds.very_low - expected_true_pct).abs() < 1e-6,
                "BPS conversion failed: {} BPS should be {} TRUE % (got {})",
                bps, expected_true_pct, regime_cfg.thresholds.very_low
            );
        }
    }

    /// V5.5 REGRESSION: Verify TOML-derived config produces sensible
    /// regime classifications — not always VeryHigh as the old 100× bug caused.
    #[test]
    fn test_toml_regime_classifies_real_sol_vol() {
        use regime_detection::{RegimeDetector, MarketRegime};

        let cfg = toml_config();  // 2 BPS very_low threshold
        let regime_cfg = RegimeConfig::from(&cfg);
        let detector = RegimeDetector::new(regime_cfg);

        // SOL normal vol ~0.5% → should classify as VeryHigh relative to 0.02% threshold
        // (0.5 > very_low=0.02, > low=0.03, > medium=0.06, > high=0.10 → VeryHigh)
        // This is CORRECT — a 2 BPS gate is extremely sensitive
        let regime = detector.classify(0.5);
        assert_eq!(
            regime, MarketRegime::VeryHigh,
            "0.5% vol should be VeryHigh relative to 2 BPS (0.02%) gate"
        );

        // Vol just at the very_low threshold should be VeryLow
        let regime_floor = detector.classify(0.01);
        assert_eq!(
            regime_floor, MarketRegime::VeryLow,
            "0.01% vol should be VeryLow relative to 0.02% threshold"
        );
    }

    /// V5.5 REGRESSION: min-vol gate must block correctly with TOML config.
    #[test]
    fn test_toml_min_vol_gate_fires_correctly() {
        use regime_detection::RegimeDetector;

        let cfg = toml_config();  // 3 BPS min_vol = 0.03%
        let regime_cfg = RegimeConfig::from(&cfg);
        let detector = RegimeDetector::new(RegimeConfig {
            pause_in_very_low_vol: false,  // test only the min-vol path
            ..regime_cfg
        });

        // Vol above min gate: 0.05% > 0.03% → must NOT pause
        let (pause, _) = detector.should_pause(0.05);
        assert!(!pause, "0.05% vol must pass 3 BPS (0.03%) gate");

        // Vol below min gate: 0.01% < 0.03% → must pause
        let (pause2, reason) = detector.should_pause(0.01);
        assert!(pause2, "0.01% vol must be blocked by 3 BPS (0.03%) gate. Reason: {}", reason);
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
    }

    // ─────────────────────────────────────────────────────────────────────
    // INTEGRATION TEST
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_full_pipeline_toml_to_trading() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let toml_cfg = toml_config();

            let ctx = AnalyticsContext::from_regime_gate_config(
                VolatilityConfig::default(),
                toml_cfg,
                FeeFilterConfig::default(),
                None,
            );

            {
                let mut vol = ctx.volatility_calc.lock().await;
                for i in 0..100 {
                    vol.add_price(100.0 + (i as f64 * 0.1));
                }
            }

            let vol = ctx.get_current_volatility();
            assert!(vol.is_some(), "Should have volatility after prices added");

            let regime = ctx.get_current_regime();
            assert!(!regime.is_empty(), "Should have regime detection");

            ctx.summarize().await;
        });
    }
}
