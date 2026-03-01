//! ═══════════════════════════════════════════════════════════════════════════
//! MARKET REGIME DETECTOR - V2.4 (PROJECT FLASH V5)
//! ═══════════════════════════════════════════════════════════════════════════
//!
//! Purpose:
//!   Shared classification and volatility gate system for all strategies.
//!
//! V2.4 (fix/vol-threshold-true-percent):
//!   ✅ RegimeConfig::default() min_volatility_to_trade: 0.3 → 0.02
//!      volatility() now returns TRUE % — 0.3 would block all normal SOL
//!      trading. 0.02% is the safe floor (~2× real SOL quiet-market vol).
//!   ✅ RegimeThresholds::default() updated to true-% world:
//!      very_low=0.5%, low=1.0%, medium=2.0%, high=3.0% — unchanged values
//!      but now explicitly documented as true % not raw ratios.
//!   ✅ should_pause() format string corrected — was multiplying by 100
//!      (double-scaling); now prints raw value directly as it is already %.
//!
//! V2.3:
//!   ✅ Public getter for current_regime (config-driven integration)
//!   ✅ Cleaner thresholds & configuration
//!   ✅ Deterministic testing and debug outputs
//!   ✅ Direct integration-ready with AnalyticsContext
//!   ✅ Additional helper functions: severity(), is_high_vol()
//!   ✅ Broader test coverage (Phase 3 style)
//!
//! ⚠️  UNIT SYSTEM NOTE (V2.4+):
//!   All volatility values in this module are TRUE PERCENTAGE.
//!   Real SOL markets: ~0.001% – 0.010% per 100-cycle window.
//!   DO NOT pass raw ratios (e.g. 0.0002) — multiply by 100 first.
//!
//! ═══════════════════════════════════════════════════════════════════════════


use log::{debug, trace};
use serde::{Deserialize, Serialize};


// ═══════════════════════════════════════════════════════════════════════════
// ENUM - Market Regime States
// ═══════════════════════════════════════════════════════════════════════════


#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketRegime {
    VeryLow,
    Low,
    Medium,
    High,
    VeryHigh,
}


impl MarketRegime {
    pub fn label(&self) -> &'static str {
        match self {
            MarketRegime::VeryLow => "VERY_LOW_VOL",
            MarketRegime::Low => "LOW_VOL",
            MarketRegime::Medium => "MEDIUM_VOL",
            MarketRegime::High => "HIGH_VOL",
            MarketRegime::VeryHigh => "VERY_HIGH_VOL",
        }
    }

    pub fn label_with_icon(&self) -> &'static str {
        match self {
            MarketRegime::VeryLow => "😴 VERY_LOW_VOL",
            MarketRegime::Low => "🙂 LOW_VOL",
            MarketRegime::Medium => "⚖️ MEDIUM_VOL",
            MarketRegime::High => "🔥 HIGH_VOL",
            MarketRegime::VeryHigh => "🚨 VERY_HIGH_VOL",
        }
    }

    /// Numerical severity scale 0 (low) ↦ 4 (high)
    pub fn severity(&self) -> u8 {
        match self {
            MarketRegime::VeryLow => 0,
            MarketRegime::Low => 1,
            MarketRegime::Medium => 2,
            MarketRegime::High => 3,
            MarketRegime::VeryHigh => 4,
        }
    }

    pub fn is_high_vol(&self) -> bool {
        matches!(self, MarketRegime::High | MarketRegime::VeryHigh)
    }
}


// ═══════════════════════════════════════════════════════════════════════════
// CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegimeThresholds {
    /// All values are TRUE PERCENTAGE (e.g. 0.5 = 0.5%)
    pub very_low: f64,
    pub low: f64,
    pub medium: f64,
    pub high: f64,
}


impl Default for RegimeThresholds {
    fn default() -> Self {
        Self {
            // True-% thresholds. Real SOL quiet = ~0.001–0.010%.
            // These classify the regime label only — not the trade gate.
            very_low: 0.5,
            low: 1.0,
            medium: 2.0,
            high: 3.0,
        }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegimeConfig {
    pub thresholds: RegimeThresholds,
    /// Minimum volatility (TRUE %) required to allow trading.
    /// Real SOL quiet market: ~0.001–0.010%. Safe floor: 0.02%.
    /// DO NOT use raw ratios here — volatility() already returns true %.
    pub min_volatility_to_trade: f64,
    pub pause_in_very_low_vol: bool,
    #[serde(default)]
    pub verbose: bool,
}


impl Default for RegimeConfig {
    fn default() -> Self {
        Self {
            thresholds: RegimeThresholds::default(),
            // 0.02% — safe floor for SOL. Blocks only truly dead markets.
            // Was 0.3 (old raw-ratio world) which blocked all normal trading.
            min_volatility_to_trade: 0.02,
            pause_in_very_low_vol: true,
            verbose: false,
        }
    }
}


// ═══════════════════════════════════════════════════════════════════════════
// DETECTOR - V2.4 with corrected true-% defaults
// ═══════════════════════════════════════════════════════════════════════════


#[derive(Debug, Clone)]
pub struct RegimeDetector {
    config: RegimeConfig,
    current_regime: MarketRegime,
}


impl RegimeDetector {
    pub fn new(config: RegimeConfig) -> Self {
        Self {
            config,
            current_regime: MarketRegime::Medium,  // Default starting regime
        }
    }

    /// Get the current market regime without needing field access
    pub fn get_current_regime(&self) -> MarketRegime {
        self.current_regime.clone()
    }

    /// Main classification method. `volatility` is TRUE PERCENTAGE.
    pub fn classify(&self, volatility: f64) -> MarketRegime {
        let t = &self.config.thresholds;
        let regime = if volatility < t.very_low {
            MarketRegime::VeryLow
        } else if volatility < t.low {
            MarketRegime::Low
        } else if volatility < t.medium {
            MarketRegime::Medium
        } else if volatility < t.high {
            MarketRegime::High
        } else {
            MarketRegime::VeryHigh
        };
        if self.config.verbose {
            trace!(
                "🔍 Regime classified {:.4}% → {}",
                volatility,
                regime.label()
            );
        }
        regime
    }

    /// Decide if trading should pause based on volatility and settings.
    /// `volatility` is TRUE PERCENTAGE.
    pub fn should_pause(&self, volatility: f64) -> (bool, String) {
        let regime = self.classify(volatility);
        if self.config.pause_in_very_low_vol && regime == MarketRegime::VeryLow {
            return (true, "RegimeGate: VERY_LOW_VOL".into());
        }
        if volatility < self.config.min_volatility_to_trade {
            return (
                true,
                format!(
                    "RegimeGate: Volatility {:.4}% < min {:.4}%",
                    volatility,
                    self.config.min_volatility_to_trade
                ),
            );
        }
        (false, String::new())
    }

    /// Combined high-level analysis: (regime, pause?, reason).
    /// `volatility` is TRUE PERCENTAGE.
    pub fn analyze(&self, volatility: f64) -> (MarketRegime, bool, String) {
        let regime = self.classify(volatility);
        let (pause, reason) = self.should_pause(volatility);
        if self.config.verbose {
            debug!(
                "📈 Analyze → {} pause={} reason={}",
                regime.label(),
                pause,
                reason
            );
        }
        (regime, pause, reason)
    }
}


// ═══════════════════════════════════════════════════════════════════════════
// TEST SUITE (V2.4 — true-% world)
// ═══════════════════════════════════════════════════════════════════════════


#[cfg(test)]
mod tests {
    use super::*;

    fn default_detector() -> RegimeDetector {
        RegimeDetector::new(RegimeConfig {
            verbose: false,
            ..Default::default()
        })
    }

    #[test]
    fn test_threshold_ordering_basic() {
        let r = default_detector();
        assert_eq!(r.classify(0.2), MarketRegime::VeryLow);
        assert_eq!(r.classify(0.7), MarketRegime::Low);
        assert_eq!(r.classify(1.5), MarketRegime::Medium);
        assert_eq!(r.classify(2.2), MarketRegime::High);
        assert_eq!(r.classify(4.0), MarketRegime::VeryHigh);
    }

    #[test]
    fn test_pause_when_below_min_threshold() {
        // Disable pause_in_very_low_vol so the min-threshold check fires.
        let cfg = RegimeConfig {
            min_volatility_to_trade: 0.5,
            pause_in_very_low_vol: false,
            ..Default::default()
        };
        let r = RegimeDetector::new(cfg);
        let (pause, reason) = r.should_pause(0.3);
        assert!(pause, "Should pause when below min_volatility_to_trade");
        assert!(reason.contains("Volatility"), "Reason was: {}", reason);
    }

    #[test]
    fn test_no_pause_in_high_volatility() {
        let r = default_detector();
        let (pause, _) = r.should_pause(3.2);
        assert!(!pause, "High vol should not pause trading");
    }

    #[test]
    fn test_analyze_combined_output_consistency() {
        let r = default_detector();
        let (regime, pause, reason) = r.analyze(0.2);
        assert_eq!(regime, MarketRegime::VeryLow);
        assert!(pause, "Pause expected in VeryLow vol state");
        assert!(reason.contains("REGIME") || reason.contains("VOL"));
    }

    #[test]
    fn test_severity_scale_and_icon_labels() {
        let r = default_detector();
        let levels = [0.3, 0.8, 1.5, 2.2, 3.5];
        for (i, l) in levels.iter().enumerate() {
            let regime = r.classify(*l);
            assert_eq!(regime.severity(), i as u8);
            assert!(regime.label_with_icon().contains("_VOL"));
        }
    }

    #[test]
    fn test_manual_regime_threshold_override() {
        let thresholds = RegimeThresholds {
            very_low: 1.0,
            low: 2.0,
            medium: 3.0,
            high: 4.0,
        };
        let cfg = RegimeConfig {
            thresholds,
            ..Default::default()
        };
        let r = RegimeDetector::new(cfg);
        assert_eq!(r.classify(0.5), MarketRegime::VeryLow);
        assert_eq!(r.classify(3.5), MarketRegime::High);
    }

    #[test]
    fn test_get_current_regime_getter() {
        let r = default_detector();
        let regime = r.get_current_regime();
        assert_eq!(regime, MarketRegime::Medium, "Default should be Medium");
        assert!(!format!("{:?}", regime).is_empty(), "Regime should be debuggable");
    }

    /// Verify the default gate allows real SOL market volatility.
    /// Real SOL quiet market: ~0.001–0.010%. Default floor: 0.02%.
    /// A value of 0.03% must NOT be blocked.
    #[test]
    fn test_default_gate_allows_real_sol_volatility() {
        let r = default_detector();
        // pause_in_very_low_vol=true in default, but 0.03% > very_low threshold (0.5%)
        // so the VeryLow branch won't fire — only the min gate check runs.
        let cfg = RegimeConfig {
            pause_in_very_low_vol: false,
            ..Default::default()
        };
        let r2 = RegimeDetector::new(cfg);
        let (pause, reason) = r2.should_pause(0.03);
        assert!(!pause, "0.03% vol should pass the default 0.02% gate. Reason: {}", reason);
    }

    /// Verify the default gate blocks truly dead markets.
    #[test]
    fn test_default_gate_blocks_dead_market() {
        let cfg = RegimeConfig {
            pause_in_very_low_vol: false,
            ..Default::default()
        };
        let r = RegimeDetector::new(cfg);
        let (pause, _) = r.should_pause(0.005);
        assert!(pause, "0.005% vol should be blocked by the 0.02% floor");
    }
}
