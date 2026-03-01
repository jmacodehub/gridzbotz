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
//! ```text
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
// DEFAULTS (module-private — callers use MeanReversionConfig)
// ═══════════════════════════════════════════════════════════════════════════

const DEFAULT_MEAN_PERIOD: usize = 20;
const DEFAULT_STRONG_BUY_THRESHOLD: f64 = 5.0;
const DEFAULT_BUY_THRESHOLD: f64 = 2.5;
const DEFAULT_STRONG_SELL_THRESHOLD: f64 = 5.0;
const DEFAULT_SELL_THRESHOLD: f64 = 2.5;
const DEFAULT_MIN_CONFIDENCE: f64 = 0.6;

// ═══════════════════════════════════════════════════════════════════════════
// MEAN REVERSION CONFIG
// ═══════════════════════════════════════════════════════════════════════════

/// Runtime-tunable parameters for the Mean Reversion strategy.
/// Sourced from TOML at startup — zero hardcoded decisions in the hot path.
#[derive(Debug, Clone)]
pub struct MeanReversionConfig {
    /// Period for calculating the mean (moving average) (default: 20)
    pub mean_period: usize,
    /// Strong buy threshold — % below mean triggers StrongBuy (default: 5.0)
    pub strong_buy_threshold: f64,
    /// Regular buy threshold — % below mean triggers Buy (default: 2.5)
    pub buy_threshold: f64,
    /// Strong sell threshold — % above mean triggers StrongSell (default: 5.0)
    pub strong_sell_threshold: f64,
    /// Regular sell threshold — % above mean triggers Sell (default: 2.5)
    pub sell_threshold: f64,
    /// Minimum confidence for signals (default: 0.6)
    pub min_confidence: f64,
}

impl Default for MeanReversionConfig {
    fn default() -> Self {
        Self {
            mean_period: DEFAULT_MEAN_PERIOD,
            strong_buy_threshold: DEFAULT_STRONG_BUY_THRESHOLD,
            buy_threshold: DEFAULT_BUY_THRESHOLD,
            strong_sell_threshold: DEFAULT_STRONG_SELL_THRESHOLD,
            sell_threshold: DEFAULT_SELL_THRESHOLD,
            min_confidence: DEFAULT_MIN_CONFIDENCE,
        }
    }
}

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

    // ── Config (captured at construction, immutable in hot path) ──────────
    mean_period: usize,
    strong_buy_threshold: f64,
    buy_threshold: f64,
    strong_sell_threshold: f64,
    sell_threshold: f64,
    min_confidence: f64,
}

impl MeanReversionStrategy {
    /// Create from explicit config — preferred in production.
    pub fn new_from_config(cfg: &MeanReversionConfig) -> Self {
        Self {
            name: "Mean Reversion".to_string(),
            price_history: VecDeque::with_capacity(cfg.mean_period),
            current_mean: None,
            stats: StrategyStats::default(),
            last_signal: None,
            mean_period: cfg.mean_period,
            strong_buy_threshold: cfg.strong_buy_threshold,
            buy_threshold: cfg.buy_threshold,
            strong_sell_threshold: cfg.strong_sell_threshold,
            sell_threshold: cfg.sell_threshold,
            min_confidence: cfg.min_confidence,
        }
    }

    /// Create new mean reversion strategy with default parameters.
    pub fn new() -> Self {
        Self::new_from_config(&MeanReversionConfig::default())
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
    fn calculate_confidence(&self, deviation: f64) -> f64 {
        let abs_dev = deviation.abs();
        (abs_dev / self.strong_buy_threshold).min(1.0)
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
        if self.price_history.len() > self.mean_period {
            self.price_history.pop_front();
        }
        
        // STEP 2: Need enough data to calculate mean
        if self.price_history.len() < self.mean_period {
            self.stats.signals_generated += 1;
            self.stats.hold_signals += 1;
            return Ok(Signal::Hold { reason: None });
        }
        
        // STEP 3: Calculate mean price
        let mean = Self::calculate_mean(&self.price_history);
        self.current_mean = Some(mean);
        
        // STEP 4: Calculate deviation from mean
        let deviation = Self::calculate_deviation(price, mean);
        
        // STEP 5: Calculate confidence
        let confidence = self.calculate_confidence(deviation);
        
        // STEP 6: Generate trading signal based on deviation
        let signal = if deviation <= -self.strong_buy_threshold {
            // 🟢 Price way below mean - STRONG BUY!
            self.stats.buy_signals += 1;
            Signal::StrongBuy {
                price,
                size: 1.0,
                confidence: confidence.max(self.min_confidence),
                reason: format!(
                    "Price {:.1}% below mean (${:.2}) - Strong reversion expected",
                    deviation.abs(), mean
                ),
                level_id: None,
            }
        } else if deviation <= -self.buy_threshold {
            // 🟩 Price below mean - BUY
            self.stats.buy_signals += 1;
            Signal::Buy {
                price,
                size: 0.5,
                confidence: confidence.max(self.min_confidence * 0.7),
                reason: format!(
                    "Price {:.1}% below mean (${:.2})",
                    deviation.abs(), mean
                ),
                level_id: None,
            }
        } else if deviation >= self.strong_sell_threshold {
            // 🔴 Price way above mean - STRONG SELL!
            self.stats.sell_signals += 1;
            Signal::StrongSell {
                price,
                size: 1.0,
                confidence: confidence.max(self.min_confidence),
                reason: format!(
                    "Price {:.1}% above mean (${:.2}) - Strong reversion expected",
                    deviation, mean
                ),
                level_id: None,
            }
        } else if deviation >= self.sell_threshold {
            // 🟥 Price above mean - SELL
            self.stats.sell_signals += 1;
            Signal::Sell {
                price,
                size: 0.5,
                confidence: confidence.max(self.min_confidence * 0.7),
                reason: format!(
                    "Price {:.1}% above mean (${:.2})",
                    deviation, mean
                ),
                level_id: None,
            }
        } else {
            // ⏸️ Price near mean - HOLD
            self.stats.hold_signals += 1;
            Signal::Hold { reason: None }
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
    async fn test_config_driven_creation() {
        let cfg = MeanReversionConfig {
            mean_period: 15,
            strong_buy_threshold: 7.0,
            buy_threshold: 3.5,
            strong_sell_threshold: 7.0,
            sell_threshold: 3.5,
            min_confidence: 0.7,
        };
        let s = MeanReversionStrategy::new_from_config(&cfg);
        assert_eq!(s.mean_period, 15);
        assert!((s.strong_buy_threshold - 7.0).abs() < f64::EPSILON);
        assert!((s.min_confidence - 0.7).abs() < f64::EPSILON);
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
        assert!(matches!(signal, Signal::Hold { .. }), "Should hold when price is near mean");
    }
}
