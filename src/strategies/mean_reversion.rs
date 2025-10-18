//! 📊 Mean Reversion Trading Strategy
//! 
//! ## Concept:
//! Prices tend to revert to their historical average (mean).
//! When price deviates significantly from the mean, it's likely to bounce back.
//! 
//! ## How It Works:
//! 1. Calculate Simple Moving Average (SMA) as the "mean"
//! 2. Measure how far current price is from the mean (deviation)
//! 3. When price is too far below mean → BUY (expect bounce up)
//! 4. When price is too far above mean → SELL (expect drop down)
//! 
//! ## Example:
//! ```
//! SMA (20): $195.00
//! Current:  $180.00
//! 
//! Deviation: -7.7% below mean
//! Signal: STRONG BUY 🟢
//! Logic: Price will likely revert back to $195
//! ```

use super::{Strategy, Signal, StrategyStats};
use async_trait::async_trait;
use anyhow::Result;
use std::collections::VecDeque;

// ═══════════════════════════════════════════════════════════════════════════
// CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

/// Period for calculating the mean (moving average)
const MEAN_PERIOD: usize = 20;

/// Strong buy threshold (% below mean)
const STRONG_BUY_THRESHOLD: f64 = 5.0;

/// Regular buy threshold
const BUY_THRESHOLD: f64 = 2.5;

/// Strong sell threshold (% above mean)
const STRONG_SELL_THRESHOLD: f64 = 5.0;

/// Regular sell threshold
const SELL_THRESHOLD: f64 = 2.5;

/// Minimum confidence for signals
const MIN_CONFIDENCE: f64 = 0.6;

// ═══════════════════════════════════════════════════════════════════════════
// MEAN REVERSION STRATEGY
// ═══════════════════════════════════════════════════════════════════════════

/// Mean reversion strategy - buys dips, sells pumps
pub struct MeanReversionStrategy {
    /// Strategy name
    name: String,
    
    /// Price history for mean calculation
    price_history: VecDeque<f64>,
    
    /// Current mean (average) price
    current_mean: Option<f64>,
    
    /// Strategy statistics
    stats: StrategyStats,
    
    /// Last signal
    last_signal: Option<Signal>,
}

impl MeanReversionStrategy {
    /// Create new mean reversion strategy
    pub fn new() -> Self {
        Self {
            name: "Mean Reversion".to_string(),
            price_history: VecDeque::with_capacity(MEAN_PERIOD),
            current_mean: None,
            stats: StrategyStats::default(),
            last_signal: None,
        }
    }
    
    /// Calculate Simple Moving Average (mean price)
    fn calculate_mean(prices: &VecDeque<f64>) -> f64 {
        if prices.is_empty() {
            return 0.0;
        }
        
        let sum: f64 = prices.iter().sum();
        sum / prices.len() as f64
    }
    
    /// Calculate deviation from mean (as percentage)
    /// 
    /// Positive = price above mean
    /// Negative = price below mean
    fn calculate_deviation(price: f64, mean: f64) -> f64 {
        if mean == 0.0 {
            return 0.0;
        }
        
        ((price - mean) / mean) * 100.0
    }
    
    /// Calculate confidence based on deviation magnitude
    /// 
    /// Larger deviation = Higher confidence in reversion
    fn calculate_confidence(deviation: f64) -> f64 {
        let abs_dev = deviation.abs();
        
        // Scale confidence based on deviation
        // 0% deviation = 0% confidence
        // 5%+ deviation = 100% confidence
        (abs_dev / 5.0).min(1.0)
    }
    
    /// Calculate standard deviation (volatility measure)
    #[allow(dead_code)]
    fn calculate_std_dev(prices: &VecDeque<f64>, mean: f64) -> f64 {
        if prices.is_empty() {
            return 0.0;
        }
        
        let variance: f64 = prices
            .iter()
            .map(|&price| {
                let diff = price - mean;
                diff * diff
            })
            .sum::<f64>() / prices.len() as f64;
        
        variance.sqrt()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// STRATEGY TRAIT IMPLEMENTATION
// ═══════════════════════════════════════════════════════════════════════════

#[async_trait]
impl Strategy for MeanReversionStrategy {
    fn name(&self) -> &str {
        &self.name
    }
    
    async fn analyze(&mut self, price: f64, _timestamp: i64) -> Result<Signal> {
        // STEP 1: Add price to history
        self.price_history.push_back(price);
        
        // Keep only required number of prices
        if self.price_history.len() > MEAN_PERIOD {
            self.price_history.pop_front();
        }
        
        // STEP 2: Need enough data to calculate mean
        if self.price_history.len() < MEAN_PERIOD {
            self.stats.signals_generated += 1;
            self.stats.hold_signals += 1;
            return Ok(Signal::Hold);
        }
        
        // STEP 3: Calculate mean price
        let mean = Self::calculate_mean(&self.price_history);
        self.current_mean = Some(mean);
        
        // STEP 4: Calculate deviation from mean
        let deviation = Self::calculate_deviation(price, mean);
        
        // STEP 5: Calculate confidence
        let confidence = Self::calculate_confidence(deviation);
        
        // STEP 6: Generate trading signal based on deviation
        let signal = if deviation <= -STRONG_BUY_THRESHOLD {
            // 🟢 Price way below mean - STRONG BUY!
            self.stats.buy_signals += 1;
            Signal::StrongBuy {
                price,
                size: 1.0,
                confidence: confidence.max(MIN_CONFIDENCE),
                reason: format!(
                    "Price {:.1}% below mean (${:.2}) - Strong reversion expected",
                    deviation.abs(), mean
                ),
            }
        } else if deviation <= -BUY_THRESHOLD {
            // 🟩 Price below mean - BUY
            self.stats.buy_signals += 1;
            Signal::Buy {
                price,
                size: 0.5,
                confidence: confidence.max(MIN_CONFIDENCE * 0.7),
                reason: format!(
                    "Price {:.1}% below mean (${:.2})",
                    deviation.abs(), mean
                ),
            }
        } else if deviation >= STRONG_SELL_THRESHOLD {
            // 🔴 Price way above mean - STRONG SELL!
            self.stats.sell_signals += 1;
            Signal::StrongSell {
                price,
                size: 1.0,
                confidence: confidence.max(MIN_CONFIDENCE),
                reason: format!(
                    "Price {:.1}% above mean (${:.2}) - Strong reversion expected",
                    deviation, mean
                ),
            }
        } else if deviation >= SELL_THRESHOLD {
            // 🟥 Price above mean - SELL
            self.stats.sell_signals += 1;
            Signal::Sell {
                price,
                size: 0.5,
                confidence: confidence.max(MIN_CONFIDENCE * 0.7),
                reason: format!(
                    "Price {:.1}% above mean (${:.2})",
                    deviation, mean
                ),
            }
        } else {
            // ⏸️ Price near mean - HOLD
            self.stats.hold_signals += 1;
            Signal::Hold
        };
        
        // STEP 7: Update stats
        self.last_signal = Some(signal.clone());
        self.stats.signals_generated += 1;
        
        Ok(signal)
    }
    
    fn stats(&self) -> StrategyStats {
        self.stats.clone()
    }
    
    fn reset(&mut self) {
        self.price_history.clear();
        self.current_mean = None;
        self.stats = StrategyStats::default();
        self.last_signal = None;
    }
}

impl Default for MeanReversionStrategy {
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
    async fn test_mean_reversion_creation() {
        let strategy = MeanReversionStrategy::new();
        assert_eq!(strategy.name(), "Mean Reversion");
    }
    
    #[tokio::test]
    async fn test_buy_signal_below_mean() {
        let mut strategy = MeanReversionStrategy::new();
        
        // Build price history around $200
        let base_prices: Vec<f64> = (0..20).map(|_| 200.0).collect();
        
        for price in base_prices {
            strategy.analyze(price, 0).await.unwrap();
        }
        
        // Price drops to $185 (7.5% below mean)
        let signal = strategy.analyze(185.0, 0).await.unwrap();
        
        // Should generate buy signal
        assert!(signal.is_bullish(), "Should generate buy signal when price drops below mean");
    }
    
    #[tokio::test]
    async fn test_sell_signal_above_mean() {
        let mut strategy = MeanReversionStrategy::new();
        
        // Build price history around $200
        let base_prices: Vec<f64> = (0..20).map(|_| 200.0).collect();
        
        for price in base_prices {
            strategy.analyze(price, 0).await.unwrap();
        }
        
        // Price jumps to $215 (7.5% above mean)
        let signal = strategy.analyze(215.0, 0).await.unwrap();
        
        // Should generate sell signal
        assert!(signal.is_bearish(), "Should generate sell signal when price rises above mean");
    }
    
    #[tokio::test]
    async fn test_hold_near_mean() {
        let mut strategy = MeanReversionStrategy::new();
        
        // Build price history around $200
        let base_prices: Vec<f64> = (0..20).map(|_| 200.0).collect();
        
        for price in base_prices {
            strategy.analyze(price, 0).await.unwrap();
        }
        
        // Price stays near mean ($202)
        let signal = strategy.analyze(202.0, 0).await.unwrap();
        
        // Should hold (price too close to mean)
        assert!(matches!(signal, Signal::Hold), "Should hold when price is near mean");
    }
}
