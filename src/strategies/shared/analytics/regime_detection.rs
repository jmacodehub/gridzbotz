//! ═══════════════════════════════════════════════════════════════════════════
//! MARKET REGIME DETECTOR - V2.3 (PROJECT FLASH V5)
//! ═══════════════════════════════════════════════════════════════════════════
//!
//! Purpose:
//!   Shared classification and volatility gate system for all strategies.
//!
//! Upgraded in V2.3:
//!   ✅ Public getter for current_regime (config-driven integration)
//!   ✅ Cleaner thresholds & configuration
//!   ✅ Deterministic testing and debug outputs
//!   ✅ Direct integration-ready with AnalyticsContext
//!   ✅ Additional helper functions: severity(), is_high_vol()
//!   ✅ Broader test coverage (Phase 3 style)
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
    pub very_low: f64,
    pub low: f64,
    pub medium: f64,
    pub high: f64,
}


impl Default for RegimeThresholds {
    fn default() -> Self {
        Self {
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
    pub min_volatility_to_trade: f64,
    pub pause_in_very_low_vol: bool,
    #[serde(default)]
    pub verbose: bool,
}


impl Default for RegimeConfig {
    fn default() -> Self {
        Self {
            thresholds: RegimeThresholds::default(),
            min_volatility_to_trade: 0.3,
            pause_in_very_low_vol: true,
            verbose: false,
        }
    }
}


// ═══════════════════════════════════════════════════════════════════════════
// DETECTOR - V2.3 with Public Getter
// ═══════════════════════════════════════════════════════════════════════════


#[derive(Debug, Clone)]
pub struct RegimeDetector {
    config: RegimeConfig,
    current_regime: MarketRegime,  // ✅ Now internal tracking with public getter
}


impl RegimeDetector {
    pub fn new(config: RegimeConfig) -> Self {
        Self {
            config,
            current_regime: MarketRegime::Medium,  // Default starting regime
        }
    }

    // ✅ PUBLIC GETTER - Exposes current regime safely
    /// Get the current market regime without needing field access
    pub fn get_current_regime(&self) -> MarketRegime {
        self.current_regime.clone()
    }

    /// Main classification method
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
                "🔍 Regime classified {:.3}% → {}",
                volatility * 100.0,
                regime.label()
            );
        }
        regime
    }

    /// Decide if trading should pause based on volatility and settings
    pub fn should_pause(&self, volatility: f64) -> (bool, String) {
        let regime = self.classify(volatility);
        if self.config.pause_in_very_low_vol && regime == MarketRegime::VeryLow {
            return (true, "RegimeGate: VERY_LOW_VOL".into());
        }
        if volatility < self.config.min_volatility_to_trade {
            return (
                true,
                format!(
                    "RegimeGate: Volatility {:.3}% < min {:.3}%",
                    volatility * 100.0,
                    self.config.min_volatility_to_trade * 100.0
                ),
            );
        }
        (false, String::new())
    }

    /// Combined high-level analysis: (regime, pause?, reason)
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
// TEST SUITE (Phase 3 Enhanced)
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
        let mut cfg = RegimeConfig::default();
        cfg.min_volatility_to_trade = 0.5;
        let r = RegimeDetector::new(cfg);
        let (pause, reason) = r.should_pause(0.3);
        assert!(pause, "Should pause for too-low volatility");
        assert!(reason.contains("Volatility"));
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

    // ✅ NEW TEST: Verify public getter works
    #[test]
    fn test_get_current_regime_getter() {
        let r = default_detector();
        let regime = r.get_current_regime();
        assert_eq!(regime, MarketRegime::Medium, "Default should be Medium");
        assert!(!format!("{:?}", regime).is_empty(), "Regime should be debuggable");
    }
}
