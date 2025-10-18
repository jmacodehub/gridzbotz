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
//! ```
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
// CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

/// Fast moving average period (shorter = more responsive)
const FAST_MA_PERIOD: usize = 9;

/// Slow moving average period (longer = smoother trend)
const SLOW_MA_PERIOD: usize = 21;

/// Minimum confidence threshold for signals (0.0 - 1.0)
const MIN_CONFIDENCE: f64 = 0.6;

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
}

impl MomentumStrategy {
    /// Create a new momentum strategy
    pub fn new() -> Self {
        Self {
            name: "Momentum (MA Crossover)".to_string(),
            fast_prices: VecDeque::with_capacity(FAST_MA_PERIOD),
            slow_prices: VecDeque::with_capacity(SLOW_MA_PERIOD),
            prev_fast_ma: None,
            prev_slow_ma: None,
            stats: StrategyStats::default(),
            last_signal: None,
        }
    }
    
    /// Calculate Simple Moving Average (SMA)
    /// 
    /// # How it works:
    /// ```
    /// Prices: 
    /// Sum: 520
    /// Count: 5
    /// Average: 520 / 5 = 104
    /// ```
    fn calculate_sma(prices: &VecDeque<f64>) -> Option<f64> {
        if prices.is_empty() {
            return None;
        }
        
        let sum: f64 = prices.iter().sum();
        Some(sum / prices.len() as f64)
    }
    
    /// Calculate Exponential Moving Average (EMA) - More responsive!
    /// 
    /// # Why EMA?
    /// EMA gives more weight to recent prices, making it react faster to changes.
    /// 
    /// Formula: EMA = (Price * Multiplier) + (Previous EMA * (1 - Multiplier))
    fn calculate_ema(prices: &VecDeque<f64>, prev_ema: Option<f64>) -> Option<f64> {
        if prices.is_empty() {
            return None;
        }
        
        let multiplier = 2.0 / (prices.len() as f64 + 1.0);
        
        // If we have a previous EMA, use it
        if let Some(prev) = prev_ema {
            let latest_price = prices.back().unwrap();
            Some((latest_price * multiplier) + (prev * (1.0 - multiplier)))
        } else {
            // First time: use SMA as starting point
            Self::calculate_sma(prices)
        }
    }
    
    /// Detect crossover between Fast and Slow moving averages
    /// 
    /// # Crossover Types:
    /// - Golden Cross: Fast crosses ABOVE Slow = BULLISH 📈
    /// - Death Cross: Fast crosses BELOW Slow = BEARISH 📉
    fn detect_crossover(&self, fast_ma: f64, slow_ma: f64) -> Option<Crossover> {
        if let (Some(prev_fast), Some(prev_slow)) = (self.prev_fast_ma, self.prev_slow_ma) {
            // Golden Cross: Fast was below, now above
            if prev_fast <= prev_slow && fast_ma > slow_ma {
                return Some(Crossover::Golden);
            }
            
            // Death Cross: Fast was above, now below
            if prev_fast >= prev_slow && fast_ma < slow_ma {
                return Some(Crossover::Death);
            }
        }
        
        None
    }
    
    /// Calculate trend strength (0.0 - 1.0)
    /// 
    /// # How it works:
    /// Measures the distance between Fast MA and Slow MA.
    /// Larger distance = Stronger trend
    fn calculate_trend_strength(&self, fast_ma: f64, slow_ma: f64) -> f64 {
        let diff_percent = ((fast_ma - slow_ma).abs() / slow_ma) * 100.0;
        
        // Normalize to 0.0 - 1.0 range
        // 0.5% difference = weak (0.5)
        // 2.0%+ difference = strong (1.0)
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
        
        // Bonus confidence if price confirms the trend
        if fast_ma > slow_ma && price > fast_ma {
            // Uptrend confirmed by price
            confidence += 0.2;
        } else if fast_ma < slow_ma && price < fast_ma {
            // Downtrend confirmed by price
            confidence += 0.2;
        }
        
        // Bonus for strong separation between MAs
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
        if self.fast_prices.len() > FAST_MA_PERIOD {
            self.fast_prices.pop_front();
        }
        if self.slow_prices.len() > SLOW_MA_PERIOD {
            self.slow_prices.pop_front();
        }
        
        // STEP 2: Need enough data to calculate MAs
        if self.slow_prices.len() < SLOW_MA_PERIOD {
            self.stats.signals_generated += 1;
            self.stats.hold_signals += 1;
            return Ok(Signal::Hold);
        }
        
        // STEP 3: Calculate moving averages
        let fast_ma = Self::calculate_ema(
            &self.fast_prices, 
            self.prev_fast_ma
        ).unwrap();
        
        let slow_ma = Self::calculate_ema(
            &self.slow_prices,
            self.prev_slow_ma
        ).unwrap();
        
        // STEP 4: Detect crossovers
        let crossover = self.detect_crossover(fast_ma, slow_ma);
        
        // STEP 5: Calculate trend metrics
        let trend_strength = self.calculate_trend_strength(fast_ma, slow_ma);
        let confidence = self.calculate_confidence(price, fast_ma, slow_ma, trend_strength);
        
        // STEP 6: Generate trading signal
        let signal = if let Some(Crossover::Golden) = crossover {
            // 🟢 GOLDEN CROSS - Strong Buy Signal!
            self.stats.buy_signals += 1;
            Signal::StrongBuy {
                price,
                size: 1.0,
                confidence,
                reason: format!(
                    "Golden Cross! Fast MA ${:.2} > Slow MA ${:.2} | Strength: {:.0}%",
                    fast_ma, slow_ma, trend_strength * 100.0
                ),
            }
        } else if let Some(Crossover::Death) = crossover {
            // 🔴 DEATH CROSS - Strong Sell Signal!
            self.stats.sell_signals += 1;
            Signal::StrongSell {
                price,
                size: 1.0,
                confidence,
                reason: format!(
                    "Death Cross! Fast MA ${:.2} < Slow MA ${:.2} | Strength: {:.0}%",
                    fast_ma, slow_ma, trend_strength * 100.0
                ),
            }
        } else if fast_ma > slow_ma && confidence >= MIN_CONFIDENCE {
            // 📈 Uptrend continues - Regular Buy
            self.stats.buy_signals += 1;
            Signal::Buy {
                price,
                size: 0.5,
                confidence,
                reason: format!(
                    "Uptrend: Fast MA ${:.2} > Slow MA ${:.2}",
                    fast_ma, slow_ma
                ),
            }
        } else if fast_ma < slow_ma && confidence >= MIN_CONFIDENCE {
            // 📉 Downtrend continues - Regular Sell
            self.stats.sell_signals += 1;
            Signal::Sell {
                price,
                size: 0.5,
                confidence,
                reason: format!(
                    "Downtrend: Fast MA ${:.2} < Slow MA ${:.2}",
                    fast_ma, slow_ma
                ),
            }
        } else {
            // ⏸️ No clear trend - Hold
            self.stats.hold_signals += 1;
            Signal::Hold
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
    async fn test_uptrend_detection() {
        let mut strategy = MomentumStrategy::new();
        
        // Simulate uptrend: prices going up
        let prices = vec![
            100.0, 101.0, 102.0, 103.0, 104.0,
            105.0, 106.0, 107.0, 108.0, 109.0,
            110.0, 111.0, 112.0, 113.0, 114.0,
            115.0, 116.0, 117.0, 118.0, 119.0,
            120.0, 121.0,
        ];
        
        let mut last_signal = Signal::Hold;
        
        for price in prices {
            let signal = strategy.analyze(price, 0).await.unwrap();
            last_signal = signal;
        }
        
        // Should detect uptrend
        assert!(last_signal.is_bullish());
    }
    
    #[tokio::test]
    async fn test_downtrend_detection() {
        let mut strategy = MomentumStrategy::new();
        
        // Simulate downtrend: prices going down
        let prices = vec![
            120.0, 119.0, 118.0, 117.0, 116.0,
            115.0, 114.0, 113.0, 112.0, 111.0,
            110.0, 109.0, 108.0, 107.0, 106.0,
            105.0, 104.0, 103.0, 102.0, 101.0,
            100.0, 99.0,
        ];
        
        let mut last_signal = Signal::Hold;
        
        for price in prices {
            let signal = strategy.analyze(price, 0).await.unwrap();
            last_signal = signal;
        }
        
        // Should detect downtrend
        assert!(last_signal.is_bearish());
    }
}
