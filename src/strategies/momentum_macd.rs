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
//! ## Example:
//! ```
//! MACD Line: 1.5
//! Signal Line: 0.8
//! Histogram: 0.7 (positive and growing)
//! 
//! Analysis: MACD > Signal + Positive Histogram
//! Signal: STRONG BUY 🟢 (Confidence: 0.85)
//! ```

use super::{Strategy, Signal, StrategyStats};
use crate::indicators::{MACDState, MACD};
use async_trait::async_trait;
use anyhow::Result;

// ═══════════════════════════════════════════════════════════════════════════
// CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

/// Minimum confidence threshold for signals
const MIN_CONFIDENCE: f64 = 0.65;

/// Strong signal threshold (histogram magnitude)
const STRONG_HISTOGRAM_THRESHOLD: f64 = 0.5;

/// Minimum periods before generating signals
const MIN_WARMUP_PERIODS: usize = 26; // Slow EMA period

// ═══════════════════════════════════════════════════════════════════════════
// MOMENTUM MACD STRATEGY
// ═══════════════════════════════════════════════════════════════════════════

/// Enhanced momentum strategy using MACD indicator
pub struct MomentumMACDStrategy {
    /// Strategy name
    name: String,
    
    /// MACD calculator state
    macd: MACDState,
    
    /// Previous MACD values for crossover detection
    prev_macd: Option<MACD>,
    
    /// Number of periods processed
    periods: usize,
    
    /// Strategy statistics
    stats: StrategyStats,
    
    /// Last signal
    last_signal: Option<Signal>,
}

impl MomentumMACDStrategy {
    /// Create new MACD-based momentum strategy
    pub fn new() -> Self {
        Self {
            name: "Momentum (MACD 12,26,9)".to_string(),
            macd: MACDState::new(), // Standard 12, 26, 9
            prev_macd: None,
            periods: 0,
            stats: StrategyStats::default(),
            last_signal: None,
        }
    }
    
    /// Detect MACD crossover
    /// 
    /// # Crossover Types:
    /// - **Bullish**: MACD crosses above Signal line
    /// - **Bearish**: MACD crosses below Signal line
    fn detect_crossover(&self, current: &MACD) -> Option<MACDCrossover> {
        if let Some(prev) = self.prev_macd {
            // Bullish crossover: MACD was below Signal, now above
            if prev.macd_line <= prev.signal_line && current.macd_line > current.signal_line {
                return Some(MACDCrossover::Bullish);
            }
            
            // Bearish crossover: MACD was above Signal, now below
            if prev.macd_line >= prev.signal_line && current.macd_line < current.signal_line {
                return Some(MACDCrossover::Bearish);
            }
        }
        
        None
    }
    
    /// Calculate momentum strength from histogram
    /// 
    /// # Logic:
    /// - Larger histogram = Stronger momentum
    /// - Positive histogram = Bullish momentum
    /// - Negative histogram = Bearish momentum
    fn calculate_momentum_strength(&self, macd: &MACD) -> f64 {
        // Normalize histogram magnitude to 0.0 - 1.0
        let magnitude = macd.histogram.abs();
        
        // Scale: 0.5 = moderate, 1.0+ = strong
        (magnitude / STRONG_HISTOGRAM_THRESHOLD).min(1.0)
    }
    
    /// Check if histogram is expanding (momentum increasing)
    fn is_histogram_expanding(&self, current: &MACD) -> bool {
        if let Some(prev) = self.prev_macd {
            current.histogram.abs() > prev.histogram.abs()
        } else {
            false
        }
    }
    
    /// Calculate confidence based on multiple MACD factors
    fn calculate_confidence(&self, macd: &MACD, price: f64) -> f64 {
        let mut confidence = 0.0;
        
        // Factor 1: Momentum strength (40% weight)
        let strength = self.calculate_momentum_strength(macd);
        confidence += strength * 0.4;
        
        // Factor 2: MACD-Signal separation (30% weight)
        let separation = (macd.macd_line - macd.signal_line).abs() / price * 100.0;
        confidence += (separation / 2.0).min(1.0) * 0.3;
        
        // Factor 3: Zero-line position (20% weight)
        // MACD above zero = bullish, below = bearish
        if (macd.macd_line > 0.0 && macd.histogram > 0.0) || 
           (macd.macd_line < 0.0 && macd.histogram < 0.0) {
            confidence += 0.2; // Aligned with trend
        }
        
        // Factor 4: Histogram expansion bonus (10% weight)
        if self.is_histogram_expanding(macd) {
            confidence += 0.1;
        }
        
        confidence.min(1.0)
    }
    
    /// Determine trend from MACD position
    fn get_trend(&self, macd: &MACD) -> Trend {
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
    /// Bullish crossover (MACD crosses above Signal)
    Bullish,
    
    /// Bearish crossover (MACD crosses below Signal)
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

// ═══════════════════════════════════════════════════════════════════════════
// STRATEGY TRAIT IMPLEMENTATION
// ═══════════════════════════════════════════════════════════════════════════

#[async_trait]
impl Strategy for MomentumMACDStrategy {
    fn name(&self) -> &str {
        &self.name
    }
    
    async fn analyze(&mut self, price: f64, _timestamp: i64) -> Result<Signal> {
        self.periods += 1;
        
        // STEP 1: Update MACD with new price
        let macd = self.macd.update(price).unwrap();
        
        // STEP 2: Need warmup period for accurate MACD
        if self.periods < MIN_WARMUP_PERIODS {
            self.prev_macd = Some(macd);
            self.stats.signals_generated += 1;
            self.stats.hold_signals += 1;
            return Ok(Signal::Hold);
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
                // 🟢 BULLISH CROSSOVER - Strong Buy!
                self.stats.buy_signals += 1;
                Signal::StrongBuy {
                    price,
                    size: 1.0,
                    confidence,
                    reason: format!(
                        "MACD Bullish Cross! MACD: {:.4} > Signal: {:.4} | Histogram: {:.4} | Strength: {:.0}%",
                        macd.macd_line, macd.signal_line, macd.histogram, momentum_strength * 100.0
                    ),
                }
            },
            Some(MACDCrossover::Bearish) => {
                // 🔴 BEARISH CROSSOVER - Strong Sell!
                self.stats.sell_signals += 1;
                Signal::StrongSell {
                    price,
                    size: 1.0,
                    confidence,
                    reason: format!(
                        "MACD Bearish Cross! MACD: {:.4} < Signal: {:.4} | Histogram: {:.4} | Strength: {:.0}%",
                        macd.macd_line, macd.signal_line, macd.histogram, momentum_strength * 100.0
                    ),
                }
            },
            None => {
                // No crossover - check trend continuation
                match trend {
                    Trend::StrongBullish if confidence >= MIN_CONFIDENCE => {
                        self.stats.buy_signals += 1;
                        Signal::Buy {
                            price,
                            size: 0.6,
                            confidence,
                            reason: format!(
                                "Strong Uptrend: MACD {:.4} above zero, Histogram {:.4} positive",
                                macd.macd_line, macd.histogram
                            ),
                        }
                    },
                    Trend::WeakBullish if confidence >= MIN_CONFIDENCE && macd.histogram > 0.0 => {
                        self.stats.buy_signals += 1;
                        Signal::Buy {
                            price,
                            size: 0.4,
                            confidence,
                            reason: format!(
                                "Weak Uptrend: MACD {:.4}, Histogram {:.4} (recovering)",
                                macd.macd_line, macd.histogram
                            ),
                        }
                    },
                    Trend::StrongBearish if confidence >= MIN_CONFIDENCE => {
                        self.stats.sell_signals += 1;
                        Signal::Sell {
                            price,
                            size: 0.6,
                            confidence,
                            reason: format!(
                                "Strong Downtrend: MACD {:.4} below zero, Histogram {:.4} negative",
                                macd.macd_line, macd.histogram
                            ),
                        }
                    },
                    Trend::WeakBearish if confidence >= MIN_CONFIDENCE && macd.histogram < 0.0 => {
                        self.stats.sell_signals += 1;
                        Signal::Sell {
                            price,
                            size: 0.4,
                            confidence,
                            reason: format!(
                                "Weak Downtrend: MACD {:.4}, Histogram {:.4} (weakening)",
                                macd.macd_line, macd.histogram
                            ),
                        }
                    },
                    _ => {
                        // Low confidence or neutral - Hold
                        self.stats.hold_signals += 1;
                        Signal::Hold
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

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_macd_creation() {
        let strategy = MomentumMACDStrategy::new();
        assert_eq!(strategy.name(), "Momentum (MACD 12,26,9)");
    }
    
    #[tokio::test]
    async fn test_uptrend_detection() {
        let mut strategy = MomentumMACDStrategy::new();
        
        // Simulate strong uptrend
        let prices: Vec<f64> = (100..150)
            .map(|x| x as f64)
            .collect();
        
        let mut last_signal = Signal::Hold;
        
        for price in prices {
            let signal = strategy.analyze(price, 0).await.unwrap();
            last_signal = signal;
        }
        
        // Should detect bullish trend
        assert!(last_signal.is_bullish());
    }
    
    #[tokio::test]
    async fn test_downtrend_detection() {
        let mut strategy = MomentumMACDStrategy::new();
        
        // Simulate strong downtrend
        let prices: Vec<f64> = (50..100)
            .rev()
            .map(|x| x as f64)
            .collect();
        
        let mut last_signal = Signal::Hold;
        
        for price in prices {
            let signal = strategy.analyze(price, 0).await.unwrap();
            last_signal = signal;
        }
        
        // Should detect bearish trend
        assert!(last_signal.is_bearish());
    }
    
    #[tokio::test]
    async fn test_crossover_detection() {
        let mut strategy = MomentumMACDStrategy::new();
        
        // Create reversal: down then up
        let mut prices: Vec<f64> = (50..75).rev().map(|x| x as f64).collect();
        prices.extend((75..100).map(|x| x as f64));
        
        let mut found_strong_buy = false;
        
        for price in prices {
            let signal = strategy.analyze(price, 0).await.unwrap();
            if matches!(signal, Signal::StrongBuy { .. }) {
                found_strong_buy = true;
            }
        }
        
        // Should detect bullish crossover after reversal
        assert!(found_strong_buy, "Should detect strong buy signal on bullish crossover");
    }
}
