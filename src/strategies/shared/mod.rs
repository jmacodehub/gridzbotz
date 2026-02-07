//! ═══════════════════════════════════════════════════════════════════════════
//! SHARED MODULE - PROJECT FLASH V5.4 (Phase 3 Stable / Phase 4 Ready)
//! ═══════════════════════════════════════════════════════════════════════════
//!
//! Purpose:
//!   Central hub for unified shared modules powering all strategies.
//!   Integrates analytics core and modular signal engines under one namespace.
//!
//! Highlights:
//!   ✅ Unified export layer (analytics + signals)
//!   ✅ Thread-safe context factory for bots & strategy managers
//!   ✅ RegimeDetector public getter for config-driven regime access
//!   ✅ Clean testing suite for CI/CD integration
//!   ✅ Future-ready for Signal Fusion Bus Phase 4
//!
//! ═══════════════════════════════════════════════════════════════════════════


pub mod analytics;
pub mod signals;


// ──────────────────────────────
// Analytics re-exports for global strategy access
// ──────────────────────────────
pub use analytics::{
    atr_dynamic::{ATRConfig, ATRDynamic},
    fee_filter::{FeeDecision, FeeFilter, FeeFilterConfig},
    regime_detection::{RegimeConfig, RegimeDetector},
    volatility_calc::{VolatilityCalculator, VolatilityConfig, VolatilityStats},
    AnalyticsContext, SharedAnalytics,
};


// ──────────────────────────────
// Analytics re-exports for single strategy access
// ──────────────────────────────
pub use self::analytics::{atr_dynamic, fee_filter, regime_detection, volatility_calc};


// ──────────────────────────────
// Signal Exports - Phase 4 FusionBus Ready
// ──────────────────────────────
pub use signals::{MeanSignal, MomentumSignal, RsiSignal, SignalModule};


// ──────────────────────────────
// Thread-safe handles and context builder
// ──────────────────────────────
use std::sync::Arc;
use tokio::sync::Mutex;


/// Shared volatility object for any strategy's dynamic volatility logic
pub type SharedVolatility = Arc<Mutex<VolatilityCalculator>>;


/// Factory for a pre-configured shared analytics context
pub fn build_shared_context() -> AnalyticsContext {
    AnalyticsContext::default()
}


// ═══════════════════════════════════════════════════════════════════════════
// ✅ PRODUCTION-GRADE GETTER - RegimeDetector Access
// ═══════════════════════════════════════════════════════════════════════════
// This module extends RegimeDetector with a public accessor for internal state.
// Uses the public getter method already defined in RegimeDetector impl.
// ═══════════════════════════════════════════════════════════════════════════


/// Helper function to safely get regime display string
/// This is the idiomatic way to access regime state from RegimeDetector
pub fn get_regime_display(detector: &RegimeDetector) -> String {
    format!("{:?}", detector.get_current_regime())
}


// ═══════════════════════════════════════════════════════════════════════════
// TEST SUITE - SANITY CHECKS FOR MODULAR LAYOUT
// ═══════════════════════════════════════════════════════════════════════════


#[cfg(test)]
mod tests {
    use super::*;
    use tokio::runtime::Runtime;

    #[test]
    fn test_shared_initialization() {
        // Context sanity
        let ctx = build_shared_context();
        assert!(
            ctx.fees.config.enabled,
            "FeeFilter configuration should default to enabled"
        );

        // Async signal compute sanity
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            use crate::strategies::shared::signals::SignalModule;

            let mut rsi = RsiSignal::default();
            let mut mom = MomentumSignal::default();
            let mut mean = MeanSignal::default();

            let a = rsi.compute(101.0).await;
            let b = mom.compute(99.5).await;
            let c = mean.compute(100.2).await;

            assert!(
                (a + b + c).is_finite(),
                "Aggregated signal outputs must remain valid floats"
            );
        });
    }

    #[test]
    fn test_regime_detector_getter_safe_access() {
        // Test that the public getter method works
        let regime_cfg = regime_detection::RegimeConfig::default();
        let detector = RegimeDetector::new(regime_cfg);

        // Using the public getter method on RegimeDetector
        let regime = detector.get_current_regime();

        // Should return some regime variant
        assert!(
            !format!("{:?}", regime).is_empty(),
            "Regime should be debuggable"
        );

        // Test the helper function
        let regime_display = get_regime_display(&detector);
        assert!(!regime_display.is_empty(), "Display string should not be empty");
    }

    #[test]
    fn test_context_regime_access() {
        let ctx = build_shared_context();

        // Should be able to get regime through context
        let regime = ctx.regime.get_current_regime();
        assert!(!format!("{:?}", regime).is_empty());
    }
}
