//! 📈 Momentum Trading Strategy
//!
//! ## How It Works:
//!
//! 1. **Tracks Two Moving Averages:**
//!    - Fast MA (9 periods) - Reacts quickly to price changes
//!    - Slow MA (21 periods) - Shows overall trend direction
//!
//! 2. **Generates Signals:**
//!    - BUY when Fast MA crosses above Slow MA (Golden Cross)
//!    - SELL when Fast MA crosses below Slow MA (Death Cross)
//!
//! 3. **Confirms with Price Action:**
//!    - Checks if price is above/below moving averages
//!    - Measures strength of the trend
//!
//! ## Example:
//! ```text
//! Price: $200
//! Fast MA (9): $198
//! Slow MA (21): $195
//!
//! Analysis: Fast > Slow AND Price > Both
//! Signal: STRONG BUY 🟢
//! ```

use super::{Strategy, Signal, StrategyStats};
use async_trait::async_trait;
use anyhow::Result;
use std::collections::VecDeque;

// ═══════════════════════════════════════════════════════════════════════════
// DEFAULTS (module-private — callers use MomentumConfig)
// ═══════════════════════════════════════════════════════════════════════════

const DEFAULT_FAST_PERIOD: usize = 9;
const DEFAULT_SLOW_PERIOD: usize = 21;
const DEFAULT_MIN_CONFIDENCE: f64 = 0.6;

// ═══════════════════════════════════════════════════════════════════════════
// MOMENTUM CONFIG
// ═══════════════════════════════════════════════════════════════════════════

/// Runtime-tunable parameters for the Momentum strategy.
/// Sourced from TOML at startup — zero hardcoded decisions in the hot path.
#[derive(Debug, Clone)]
pub struct MomentumConfig {
    /// Fast moving average period (default: 9)
    pub fast_period: usize,
    /// Slow moving average period (default: 21)
    pub slow_period: usize,
    /// Minimum confidence threshold to act on a signal (default: 0.6)
    pub min_confidence: f64,
}

impl Default for MomentumConfig {
    fn default() -> Self {
        Self {
            fast_period: DEFAULT_FAST_PERIOD,
            slow_period: DEFAULT_SLOW_PERIOD,
            min_confidence: DEFAULT_MIN_CONFIDENCE,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// MOMENTUM STRATEGY
// ═══════════════════════════════════════════════════════════════════════════

/// Momentum trading strategy using moving average crossovers
pub struct MomentumStrategy {
    /// Strategy name
    name: String,

    /// Price history for Fast MA calculation
    fast_prices: VecDeque<f64>,

    /// Price history for Slow MA calculation
    slow_prices: VecDeque<f64>,

    /// Previous Fast MA value (to detect crossovers)
    prev_fast_ma: Option<f64>,

    /// Previous Slow MA value
    prev_slow_ma: Option<f64>,

    /// Strategy statistics
    stats: StrategyStats,

    /// Last signal generated
    last_signal: Option<Signal>,

    // ── Config (captured at construction, immutable in hot path) ──────────
    fast_period: usize,
    slow_period: usize,
    min_confidence: f64,
}

impl MomentumStrategy {
    /// Create from explicit config — preferred in production.
    pub fn new_from_config(cfg: &MomentumConfig) -> Self {
        Self {
            name: "Momentum (MA Crossover)".to_string(),
            fast_prices: VecDeque::with_capacity(cfg.fast_period),
            slow_prices: VecDeque::with_capacity(cfg.slow_period),
            prev_fast_ma: None,
            prev_slow_ma: None,
            stats: StrategyStats::default(),
            last_signal: None,
            fast_period: cfg.fast_period,
            slow_period: cfg.slow_period,
            min_confidence: cfg.min_confidence,
        }
    }

    /// Create a new momentum strategy with default parameters.
    pub fn new() -> Self {
        Self::new_from_config(&MomentumConfig::default())
    }

    /// Calculate Simple Moving Average (SMA)
    fn calculate_sma(prices: &VecDeque<f64>) -> Option<f64> {
        if prices.is_empty() {
            return None;
        }
        let sum: f64 = prices.iter().sum();
        Some(sum / prices.len() as f64)
    }

    /// Calculate Exponential Moving Average (EMA) - More responsive!
    fn calculate_ema(prices: &VecDeque<f64>, prev_ema: Option<f64>) -> Option<f64> {
        if prices.is_empty() {
            return None;
        }
        let multiplier = 2.0 / (prices.len() as f64 + 1.0);
        if let Some(prev) = prev_ema {
            let latest_price = prices.back().unwrap();
            Some((latest_price * multiplier) + (prev * (1.0 - multiplier)))
        } else {
            Self::calculate_sma(prices)
        }
    }

    /// Detect crossover between Fast and Slow moving averages
    fn detect_crossover(&self, fast_ma: f64, slow_ma: f64) -> Option<Crossover> {
        if let (Some(prev_fast), Some(prev_slow)) = (self.prev_fast_ma, self.prev_slow_ma) {
            if prev_fast <= prev_slow && fast_ma > slow_ma {
                return Some(Crossover::Golden);
            }
            if prev_fast >= prev_slow && fast_ma < slow_ma {
                return Some(Crossover::Death);
            }
        }
        None
    }

    /// Calculate trend strength (0.0 - 1.0)
    fn calculate_trend_strength(&self, fast_ma: f64, slow_ma: f64) -> f64 {
        let diff_percent = ((fast_ma - slow_ma).abs() / slow_ma) * 100.0;
        (diff_percent / 2.0).min(1.0)
    }

    /// Calculate confidence based on multiple factors
    fn calculate_confidence(
        &self,
        price: f64,
        fast_ma: f64,
        slow_ma: f64,
        trend_strength: f64,
    ) -> f64 {
        let mut confidence = trend_strength;
        if fast_ma > slow_ma && price > fast_ma {
            confidence += 0.2;
        } else if fast_ma < slow_ma && price < fast_ma {
            confidence += 0.2;
        }
        let separation = ((fast_ma - slow_ma).abs() / slow_ma) * 100.0;
        if separation > 1.0 {
            confidence += 0.1;
        }
        confidence.min(1.0)
    }
}

/// Types of moving average crossovers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Crossover {
    /// Golden Cross: Bullish signal (Fast crosses above Slow)
    Golden,
    /// Death Cross: Bearish signal (Fast crosses below Slow)
    Death,
}

// ═══════════════════════════════════════════════════════════════════════════
// STRATEGY TRAIT IMPLEMENTATION
// ═══════════════════════════════════════════════════════════════════════════

#[async_trait]
impl Strategy for MomentumStrategy {
    fn name(&self) -> &str {
        &self.name
    }

    async fn analyze(&mut self, price: f64, _timestamp: i64) -> Result<Signal> {
        // STEP 1: Add price to our history
        self.fast_prices.push_back(price);
        self.slow_prices.push_back(price);

        // Keep only the required number of prices
        if self.fast_prices.len() > self.fast_period {
            self.fast_prices.pop_front();
        }
        if self.slow_prices.len() > self.slow_period {
            self.slow_prices.pop_front();
        }

        // STEP 2: Need enough data to calculate MAs
        if self.slow_prices.len() < self.slow_period {
            self.stats.signals_generated += 1;
            self.stats.hold_signals += 1;
            return Ok(Signal::Hold { reason: None });
        }

        // STEP 3: Calculate moving averages
        let fast_ma = Self::calculate_ema(&self.fast_prices, self.prev_fast_ma).unwrap();
        let slow_ma = Self::calculate_ema(&self.slow_prices, self.prev_slow_ma).unwrap();

        // STEP 4: Detect crossovers
        let crossover = self.detect_crossover(fast_ma, slow_ma);

        // STEP 5: Calculate trend metrics
        let trend_strength = self.calculate_trend_strength(fast_ma, slow_ma);
        let confidence = self.calculate_confidence(price, fast_ma, slow_ma, trend_strength);

        // STEP 6: Generate trading signal
        let signal = if let Some(Crossover::Golden) = crossover {
            self.stats.buy_signals += 1;
            Signal::StrongBuy {
                price,
                size: 1.0,
                confidence,
                reason: format!(
                    "Golden Cross! Fast MA ${:.2} > Slow MA ${:.2} | Strength: {:.0}%",
                    fast_ma, slow_ma, trend_strength * 100.0
                ),
                level_id: None,
            }
        } else if let Some(Crossover::Death) = crossover {
            self.stats.sell_signals += 1;
            Signal::StrongSell {
                price,
                size: 1.0,
                confidence,
                reason: format!(
                    "Death Cross! Fast MA ${:.2} < Slow MA ${:.2} | Strength: {:.0}%",
                    fast_ma, slow_ma, trend_strength * 100.0
                ),
                level_id: None,
            }
        } else if fast_ma > slow_ma && confidence >= self.min_confidence {
            self.stats.buy_signals += 1;
            Signal::Buy {
                price,
                size: 0.5,
                confidence,
                reason: format!(
                    "Uptrend: Fast MA ${:.2} > Slow MA ${:.2}",
                    fast_ma, slow_ma
                ),
                level_id: None,
            }
        } else if fast_ma < slow_ma && confidence >= self.min_confidence {
            self.stats.sell_signals += 1;
            Signal::Sell {
                price,
                size: 0.5,
                confidence,
                reason: format!(
                    "Downtrend: Fast MA ${:.2} < Slow MA ${:.2}",
                    fast_ma, slow_ma
                ),
                level_id: None,
            }
        } else {
            self.stats.hold_signals += 1;
            Signal::Hold { reason: None }
        };

        // STEP 7: Update state for next iteration
        self.prev_fast_ma = Some(fast_ma);
        self.prev_slow_ma = Some(slow_ma);
        self.last_signal = Some(signal.clone());
        self.stats.signals_generated += 1;

        Ok(signal)
    }

    fn stats(&self) -> StrategyStats {
        self.stats.clone()
    }

    fn reset(&mut self) {
        self.fast_prices.clear();
        self.slow_prices.clear();
        self.prev_fast_ma = None;
        self.prev_slow_ma = None;
        self.stats = StrategyStats::default();
        self.last_signal = None;
    }
}

impl Default for MomentumStrategy {
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
    async fn test_momentum_creation() {
        let strategy = MomentumStrategy::new();
        assert_eq!(strategy.name(), "Momentum (MA Crossover)");
    }

    #[tokio::test]
    async fn test_config_driven_creation() {
        let cfg = MomentumConfig { fast_period: 5, slow_period: 15, min_confidence: 0.75 };
        let s = MomentumStrategy::new_from_config(&cfg);
        assert_eq!(s.fast_period, 5);
        assert_eq!(s.slow_period, 15);
        assert!((s.min_confidence - 0.75).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_uptrend_detection() {
        let mut strategy = MomentumStrategy::new();
        let prices = vec![
            100.0, 101.0, 102.0, 103.0, 104.0,
            105.0, 106.0, 107.0, 108.0, 109.0,
            110.0, 111.0, 112.0, 113.0, 114.0,
            115.0, 116.0, 117.0, 118.0, 119.0,
            120.0, 121.0,
        ];
        let mut last_signal = Signal::Hold { reason: None };
        for price in prices {
            let signal = strategy.analyze(price, 0).await.unwrap();
            last_signal = signal;
        }
        assert!(last_signal.is_bullish());
    }

    #[tokio::test]
    async fn test_downtrend_detection() {
        let mut strategy = MomentumStrategy::new();
        let prices = vec![
            120.0, 119.0, 118.0, 117.0, 116.0,
            115.0, 114.0, 113.0, 112.0, 111.0,
            110.0, 109.0, 108.0, 107.0, 106.0,
            105.0, 104.0, 103.0, 102.0, 101.0,
            100.0, 99.0,
        ];
        let mut last_signal = Signal::Hold { reason: None };
        for price in prices {
            let signal = strategy.analyze(price, 0).await.unwrap();
            last_signal = signal;
        }
        assert!(last_signal.is_bearish());
    }
}
