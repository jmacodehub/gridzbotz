//! 📈 Enhanced Momentum Strategy with MACD
//! 
//! ## Upgrades from Basic Momentum:
//! 
//! 1. **MACD Instead of Simple MA:**
//!    - MACD Line: EMA(12) - EMA(26)
//!    - Signal Line: EMA(9) of MACD
//!    - Histogram: MACD - Signal
//! 
//! 2. **More Precise Signals:**
//!    - MACD crossover = Trend change
//!    - Histogram = Momentum strength
//!    - Zero-line = Trend direction
//! 
//! 3. **Better Performance:**
//!    - Fewer false signals
//!    - Earlier trend detection
//!    - Stronger confidence scoring
//! 
//! ## Note on MACD signal semantics:
//! MACD excels at detecting TREND TRANSITIONS (crossovers), not
//! steady-state trends. In a monotonic linear trend the histogram
//! converges to ~0 as signal_ema catches up to macd_line. The
//! strategy fires on the GROWTH PHASE of the histogram (when
//! momentum is accelerating), not on the plateau.
//! 
//! ## Example:
//! ```text
//! MACD Line: 1.5
//! Signal Line: 0.8
//! Histogram: 0.7 (positive and growing)
//! 
//! Analysis: MACD > Signal + Positive Histogram
//! Signal: STRONG BUY 🟢 (Confidence: 0.85)
//! ```

use super::{Strategy, Signal, StrategyStats};
// MACDState::update() returns MACDValues (pub fields: macd_line, signal_line, histogram).
// macd::MACD is the object-based calculator — NOT what we need here.
use crate::indicators::{MACDState, MACDValues};
use async_trait::async_trait;
use anyhow::Result;

// ══════════════════════════════════════════════════════════════════════
// DEFAULTS (module-private — callers use MomentumMACDConfig)
// ══════════════════════════════════════════════════════════════════════

const DEFAULT_MIN_CONFIDENCE: f64 = 0.65;
const DEFAULT_STRONG_HISTOGRAM_THRESHOLD: f64 = 0.5;
const DEFAULT_MIN_WARMUP_PERIODS: usize = 26;

// ══════════════════════════════════════════════════════════════════════
// MOMENTUM MACD CONFIG
// ══════════════════════════════════════════════════════════════════════

/// Runtime-tunable parameters for the Momentum MACD strategy.
/// Sourced from TOML at startup — zero hardcoded decisions in the hot path.
#[derive(Debug, Clone)]
pub struct MomentumMACDConfig {
    /// Minimum confidence threshold for trend-following signals (default: 0.65)
    pub min_confidence: f64,
    /// Strong signal threshold — histogram magnitude for strong signals (default: 0.5)
    pub strong_histogram_threshold: f64,
    /// Minimum periods before generating signals (default: 26 = slow EMA period)
    pub min_warmup_periods: usize,
}

impl Default for MomentumMACDConfig {
    fn default() -> Self {
        Self {
            min_confidence: DEFAULT_MIN_CONFIDENCE,
            strong_histogram_threshold: DEFAULT_STRONG_HISTOGRAM_THRESHOLD,
            min_warmup_periods: DEFAULT_MIN_WARMUP_PERIODS,
        }
    }
}

// ══════════════════════════════════════════════════════════════════════
// MOMENTUM MACD STRATEGY
// ══════════════════════════════════════════════════════════════════════

/// Enhanced momentum strategy using MACD indicator
pub struct MomentumMACDStrategy {
    /// Strategy name
    name: String,
    
    /// MACD incremental calculator
    macd: MACDState,
    
    /// Previous MACD values for crossover detection (MACDValues, not macd::MACD)
    prev_macd: Option<MACDValues>,
    
    /// Number of periods processed
    periods: usize,
    
    /// Strategy statistics
    stats: StrategyStats,
    
    /// Last signal
    last_signal: Option<Signal>,

    // ── Config (captured at construction, immutable in hot path) ──────────
    min_confidence: f64,
    strong_histogram_threshold: f64,
    min_warmup_periods: usize,
}

impl MomentumMACDStrategy {
    /// Create from explicit config — preferred in production.
    pub fn new_from_config(cfg: &MomentumMACDConfig) -> Self {
        Self {
            name: "Momentum (MACD 12,26,9)".to_string(),
            macd: MACDState::new(),
            prev_macd: None,
            periods: 0,
            stats: StrategyStats::default(),
            last_signal: None,
            min_confidence: cfg.min_confidence,
            strong_histogram_threshold: cfg.strong_histogram_threshold,
            min_warmup_periods: cfg.min_warmup_periods,
        }
    }

    /// Create new MACD-based momentum strategy with default parameters.
    pub fn new() -> Self {
        Self::new_from_config(&MomentumMACDConfig::default())
    }
    
    /// Detect MACD crossover by comparing current vs previous MACDValues
    fn detect_crossover(&self, current: &MACDValues) -> Option<MACDCrossover> {
        if let Some(prev) = self.prev_macd {
            if prev.macd_line <= prev.signal_line && current.macd_line > current.signal_line {
                return Some(MACDCrossover::Bullish);
            }
            if prev.macd_line >= prev.signal_line && current.macd_line < current.signal_line {
                return Some(MACDCrossover::Bearish);
            }
        }
        None
    }
    
    /// Calculate momentum strength from histogram magnitude
    fn calculate_momentum_strength(&self, macd: &MACDValues) -> f64 {
        let magnitude = macd.histogram.abs();
        (magnitude / self.strong_histogram_threshold).min(1.0)
    }
    
    /// Check if histogram is expanding (momentum increasing)
    fn is_histogram_expanding(&self, current: &MACDValues) -> bool {
        if let Some(prev) = self.prev_macd {
            current.histogram.abs() > prev.histogram.abs()
        } else {
            false
        }
    }
    
    /// Calculate confidence based on multiple MACD factors
    fn calculate_confidence(&self, macd: &MACDValues, price: f64) -> f64 {
        let mut confidence: f64 = 0.0;
        
        let strength = self.calculate_momentum_strength(macd);
        confidence += strength * 0.4;
        
        let separation = (macd.macd_line - macd.signal_line).abs() / price * 100.0;
        confidence += (separation / 2.0).min(1.0) * 0.3;
        
        if (macd.macd_line > 0.0 && macd.histogram > 0.0) || 
           (macd.macd_line < 0.0 && macd.histogram < 0.0) {
            confidence += 0.2;
        }
        
        if self.is_histogram_expanding(macd) {
            confidence += 0.1;
        }
        
        confidence.min(1.0)
    }
    
    /// Determine trend from MACD position (uses MACDValues fields directly)
    fn get_trend(&self, macd: &MACDValues) -> Trend {
        if macd.macd_line > 0.0 {
            if macd.histogram > 0.0 {
                Trend::StrongBullish
            } else {
                Trend::WeakBullish
            }
        } else if macd.macd_line < 0.0 {
            if macd.histogram < 0.0 {
                Trend::StrongBearish
            } else {
                Trend::WeakBearish
            }
        } else {
            Trend::Neutral
        }
    }
}

/// MACD crossover types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MACDCrossover {
    Bullish,
    Bearish,
}

/// Market trend based on MACD
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Trend {
    StrongBullish,
    WeakBullish,
    Neutral,
    WeakBearish,
    StrongBearish,
}

// ══════════════════════════════════════════════════════════════════════
// STRATEGY TRAIT IMPLEMENTATION
// ══════════════════════════════════════════════════════════════════════

#[async_trait]
impl Strategy for MomentumMACDStrategy {
    fn name(&self) -> &str {
        &self.name
    }
    
    async fn analyze(&mut self, price: f64, _timestamp: i64) -> Result<Signal> {
        self.periods += 1;
        
        // STEP 1: Update MACD with new price.
        // MACDState::update() always returns Some(MACDValues).
        let macd = self.macd.update(price).expect("MACDState::update returned None");
        
        // STEP 2: Need warmup period for accurate MACD
        if self.periods < self.min_warmup_periods {
            self.prev_macd = Some(macd);
            self.stats.signals_generated += 1;
            self.stats.hold_signals += 1;
            return Ok(Signal::Hold { reason: None });
        }
        
        // STEP 3: Detect crossovers
        let crossover = self.detect_crossover(&macd);
        
        // STEP 4: Calculate confidence and trend
        let confidence = self.calculate_confidence(&macd, price);
        let trend = self.get_trend(&macd);
        let momentum_strength = self.calculate_momentum_strength(&macd);
        
        // STEP 5: Generate trading signal
        let signal = match crossover {
            Some(MACDCrossover::Bullish) => {
                self.stats.buy_signals += 1;
                Signal::StrongBuy {
                    price,
                    size: 1.0,
                    confidence,
                    reason: format!(
                        "MACD Bullish Cross! MACD: {:.4} > Signal: {:.4} | Histogram: {:.4} | Strength: {:.0}%",
                        macd.macd_line, macd.signal_line, macd.histogram, momentum_strength * 100.0
                    ),
                    level_id: None,
                }
            },
            Some(MACDCrossover::Bearish) => {
                self.stats.sell_signals += 1;
                Signal::StrongSell {
                    price,
                    size: 1.0,
                    confidence,
                    reason: format!(
                        "MACD Bearish Cross! MACD: {:.4} < Signal: {:.4} | Histogram: {:.4} | Strength: {:.0}%",
                        macd.macd_line, macd.signal_line, macd.histogram, momentum_strength * 100.0
                    ),
                    level_id: None,
                }
            },
            None => {
                match trend {
                    Trend::StrongBullish if confidence >= self.min_confidence => {
                        self.stats.buy_signals += 1;
                        Signal::Buy {
                            price,
                            size: 0.6,
                            confidence,
                            reason: format!(
                                "Strong Uptrend: MACD {:.4} above zero, Histogram {:.4} positive",
                                macd.macd_line, macd.histogram
                            ),
                            level_id: None,
                        }
                    },
                    Trend::WeakBullish if confidence >= self.min_confidence && macd.histogram > 0.0 => {
                        self.stats.buy_signals += 1;
                        Signal::Buy {
                            price,
                            size: 0.4,
                            confidence,
                            reason: format!(
                                "Weak Uptrend: MACD {:.4}, Histogram {:.4} (recovering)",
                                macd.macd_line, macd.histogram
                            ),
                            level_id: None,
                        }
                    },
                    Trend::StrongBearish if confidence >= self.min_confidence => {
                        self.stats.sell_signals += 1;
                        Signal::Sell {
                            price,
                            size: 0.6,
                            confidence,
                            reason: format!(
                                "Strong Downtrend: MACD {:.4} below zero, Histogram {:.4} negative",
                                macd.macd_line, macd.histogram
                            ),
                            level_id: None,
                        }
                    },
                    Trend::WeakBearish if confidence >= self.min_confidence && macd.histogram < 0.0 => {
                        self.stats.sell_signals += 1;
                        Signal::Sell {
                            price,
                            size: 0.4,
                            confidence,
                            reason: format!(
                                "Weak Downtrend: MACD {:.4}, Histogram {:.4} (weakening)",
                                macd.macd_line, macd.histogram
                            ),
                            level_id: None,
                        }
                    },
                    _ => {
                        self.stats.hold_signals += 1;
                        Signal::Hold { reason: None }
                    }
                }
            }
        };
        
        // STEP 6: Update state
        self.prev_macd = Some(macd);
        self.last_signal = Some(signal.clone());
        self.stats.signals_generated += 1;
        
        Ok(signal)
    }
    
    fn stats(&self) -> StrategyStats {
        self.stats.clone()
    }
    
    fn reset(&mut self) {
        self.macd.reset();
        self.prev_macd = None;
        self.periods = 0;
        self.stats = StrategyStats::default();
        self.last_signal = None;
    }
}

impl Default for MomentumMACDStrategy {
    fn default() -> Self {
        Self::new()
    }
}

// ══════════════════════════════════════════════════════════════════════
// TESTS
// ══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_macd_creation() {
        let strategy = MomentumMACDStrategy::new();
        assert_eq!(strategy.name(), "Momentum (MACD 12,26,9)");
    }

    #[tokio::test]
    async fn test_config_driven_creation() {
        let cfg = MomentumMACDConfig {
            min_confidence: 0.75,
            strong_histogram_threshold: 0.7,
            min_warmup_periods: 20,
        };
        let s = MomentumMACDStrategy::new_from_config(&cfg);
        assert!((s.min_confidence - 0.75).abs() < f64::EPSILON);
        assert!((s.strong_histogram_threshold - 0.7).abs() < f64::EPSILON);
        assert_eq!(s.min_warmup_periods, 20);
    }
    
    /// MACD fires signals during the HISTOGRAM GROWTH PHASE — when fast_ema
    /// diverges from slow_ema and the histogram is still expanding.
    /// In a monotonic linear trend the histogram eventually converges to ~0
    /// (signal_ema catches up), so checking the LAST signal gives a false
    /// negative. Instead we assert that at least one bullish signal was
    /// emitted somewhere during the 100-price uptrend, which is the
    /// semantically correct invariant.
    #[tokio::test]
    async fn test_uptrend_detection() {
        let mut strategy = MomentumMACDStrategy::new();
        
        // 100 prices ensures EMA convergence and a clear histogram peak
        let prices: Vec<f64> = (100..200)
            .map(|x| x as f64)
            .collect();
        
        for price in prices {
            let _ = strategy.analyze(price, 0).await.unwrap();
        }
        
        assert!(
            strategy.stats().buy_signals > 0,
            "Expected at least one buy signal during 100-price uptrend; got buy={} sell={}",
            strategy.stats().buy_signals,
            strategy.stats().sell_signals,
        );
    }
    
    #[tokio::test]
    async fn test_downtrend_detection() {
        let mut strategy = MomentumMACDStrategy::new();
        
        // 100 prices in a strong downtrend
        let prices: Vec<f64> = (100..200)
            .rev()
            .map(|x| x as f64)
            .collect();
        
        for price in prices {
            let _ = strategy.analyze(price, 0).await.unwrap();
        }
        
        assert!(
            strategy.stats().sell_signals > 0,
            "Expected at least one sell signal during 100-price downtrend; got buy={} sell={}",
            strategy.stats().buy_signals,
            strategy.stats().sell_signals,
        );
    }
    
    #[tokio::test]
    async fn test_crossover_detection() {
        let mut strategy = MomentumMACDStrategy::new();
        
        let mut prices: Vec<f64> = (50..75).rev().map(|x| x as f64).collect();
        prices.extend((75..100).map(|x| x as f64));
        
        let mut found_strong_buy = false;
        
        for price in prices {
            let signal = strategy.analyze(price, 0).await.unwrap();
            if matches!(signal, Signal::StrongBuy { .. }) {
                found_strong_buy = true;
            }
        }
        
        assert!(found_strong_buy, "Should detect strong buy signal on bullish crossover");
    }
}
