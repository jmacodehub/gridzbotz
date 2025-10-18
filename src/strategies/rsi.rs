//! 📊 RSI (Relative Strength Index) Trading Strategy
//! 
//! ## What is RSI?
//! RSI measures momentum on a 0-100 scale to identify overbought/oversold conditions.
//! 
//! ## How It Works:
//! 1. Track gains and losses over 14 periods
//! 2. Calculate average gain vs average loss (RS)
//! 3. Convert to RSI: RSI = 100 - (100 / (1 + RS))
//! 4. Generate signals based on thresholds:
//!    - RSI < 30: Oversold → BUY
//!    - RSI > 70: Overbought → SELL
//! 
//! ## Example:
//! ```
//! RSI = 25 (Oversold)
//! Signal: STRONG BUY 🟢
//! Reason: "Price oversold, bounce expected"
//! ```

use super::{Strategy, Signal, StrategyStats};
use async_trait::async_trait;
use anyhow::Result;
use std::collections::VecDeque;

// ═══════════════════════════════════════════════════════════════════════════
// CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

/// RSI calculation period (standard is 14)
const RSI_PERIOD: usize = 14;

/// Oversold threshold (buy zone)
const OVERSOLD_THRESHOLD: f64 = 30.0;

/// Overbought threshold (sell zone)
const OVERBOUGHT_THRESHOLD: f64 = 70.0;

/// Extreme oversold (strong buy)
const EXTREME_OVERSOLD: f64 = 20.0;

/// Extreme overbought (strong sell)
const EXTREME_OVERBOUGHT: f64 = 80.0;

// ═══════════════════════════════════════════════════════════════════════════
// RSI STRATEGY
// ═══════════════════════════════════════════════════════════════════════════

/// RSI strategy for timing entry/exit points
pub struct RSIStrategy {
    /// Strategy name
    name: String,
    
    /// Price history for RSI calculation
    price_history: VecDeque<f64>,
    
    /// Average gain over period
    avg_gain: f64,
    
    /// Average loss over period
    avg_loss: f64,
    
    /// Current RSI value
    current_rsi: Option<f64>,
    
    /// Previous price for gain/loss calculation
    prev_price: Option<f64>,
    
    /// Strategy statistics
    stats: StrategyStats,
    
    /// Last signal
    last_signal: Option<Signal>,
}

impl RSIStrategy {
    /// Create new RSI strategy
    pub fn new() -> Self {
        Self {
            name: "RSI (14)".to_string(),
            price_history: VecDeque::with_capacity(RSI_PERIOD + 1),
            avg_gain: 0.0,
            avg_loss: 0.0,
            current_rsi: None,
            prev_price: None,
            stats: StrategyStats::default(),
            last_signal: None,
        }
    }
    
    /// Calculate RSI value
    /// 
    /// Formula: RSI = 100 - (100 / (1 + RS))
    /// Where RS = Average Gain / Average Loss
    fn calculate_rsi(&self) -> Option<f64> {
        if self.avg_loss == 0.0 {
            // If no losses, RSI is 100 (maximum)
            return Some(100.0);
        }
        
        let rs = self.avg_gain / self.avg_loss;
        let rsi = 100.0 - (100.0 / (1.0 + rs));
        
        Some(rsi)
    }
    
    /// Update average gain and loss with new price
    /// 
    /// Uses smoothed moving average (Wilder's method):
    /// New Average = ((Previous Average × 13) + Current Change) / 14
    fn update_averages(&mut self, price: f64) {
        if let Some(prev_price) = self.prev_price {
            let change = price - prev_price;
            
            // First RSI calculation (initial averages)
            if self.price_history.len() == RSI_PERIOD {
                let (gains, losses) = self.calculate_initial_averages();
                self.avg_gain = gains;
                self.avg_loss = losses;
            } else if self.price_history.len() > RSI_PERIOD {
                // Subsequent calculations (smoothed)
                if change > 0.0 {
                    self.avg_gain = ((self.avg_gain * 13.0) + change) / 14.0;
                    self.avg_loss = (self.avg_loss * 13.0) / 14.0;
                } else {
                    self.avg_gain = (self.avg_gain * 13.0) / 14.0;
                    self.avg_loss = ((self.avg_loss * 13.0) + change.abs()) / 14.0;
                }
            }
        }
        
        self.prev_price = Some(price);
    }
    
    /// Calculate initial averages for first RSI
    fn calculate_initial_averages(&self) -> (f64, f64) {
        let mut total_gain = 0.0;
        let mut total_loss = 0.0;
        
        for i in 1..self.price_history.len() {
            let change = self.price_history[i] - self.price_history[i - 1];
            
            if change > 0.0 {
                total_gain += change;
            } else {
                total_loss += change.abs();
            }
        }
        
        (total_gain / RSI_PERIOD as f64, total_loss / RSI_PERIOD as f64)
    }
    
    /// Calculate confidence based on RSI extremity
    /// 
    /// More extreme RSI = Higher confidence
    fn calculate_confidence(&self, rsi: f64) -> f64 {
        if rsi <= EXTREME_OVERSOLD {
            // Extremely oversold: 100% confidence
            1.0
        } else if rsi < OVERSOLD_THRESHOLD {
            // Oversold: Scale confidence (70-100%)
            0.7 + (0.3 * (OVERSOLD_THRESHOLD - rsi) / (OVERSOLD_THRESHOLD - EXTREME_OVERSOLD))
        } else if rsi >= EXTREME_OVERBOUGHT {
            // Extremely overbought: 100% confidence
            1.0
        } else if rsi > OVERBOUGHT_THRESHOLD {
            // Overbought: Scale confidence (70-100%)
            0.7 + (0.3 * (rsi - OVERBOUGHT_THRESHOLD) / (EXTREME_OVERBOUGHT - OVERBOUGHT_THRESHOLD))
        } else {
            // Neutral zone: Low confidence
            0.5
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// STRATEGY TRAIT IMPLEMENTATION
// ═══════════════════════════════════════════════════════════════════════════

#[async_trait]
impl Strategy for RSIStrategy {
    fn name(&self) -> &str {
        &self.name
    }
    
    async fn analyze(&mut self, price: f64, _timestamp: i64) -> Result<Signal> {
        // STEP 1: Add price to history
        self.price_history.push_back(price);
        
        // Keep only required prices
        if self.price_history.len() > RSI_PERIOD + 1 {
            self.price_history.pop_front();
        }
        
        // STEP 2: Update averages
        self.update_averages(price);
        
        // STEP 3: Need enough data to calculate RSI
        if self.price_history.len() < RSI_PERIOD {
            self.stats.signals_generated += 1;
            self.stats.hold_signals += 1;
            return Ok(Signal::Hold);
        }
        
        // STEP 4: Calculate RSI
        let rsi = self.calculate_rsi().unwrap_or(50.0);
        self.current_rsi = Some(rsi);
        
        // STEP 5: Calculate confidence
        let confidence = self.calculate_confidence(rsi);
        
        // STEP 6: Generate trading signal
        let signal = if rsi <= EXTREME_OVERSOLD {
            // 🟢 Extremely oversold - STRONG BUY!
            self.stats.buy_signals += 1;
            Signal::StrongBuy {
                price,
                size: 1.0,
                confidence,
                reason: format!("RSI {:.1} - Extremely oversold!", rsi),
            }
        } else if rsi < OVERSOLD_THRESHOLD {
            // 🟩 Oversold - BUY
            self.stats.buy_signals += 1;
            Signal::Buy {
                price,
                size: 0.5,
                confidence,
                reason: format!("RSI {:.1} - Oversold", rsi),
            }
        } else if rsi >= EXTREME_OVERBOUGHT {
            // 🔴 Extremely overbought - STRONG SELL!
            self.stats.sell_signals += 1;
            Signal::StrongSell {
                price,
                size: 1.0,
                confidence,
                reason: format!("RSI {:.1} - Extremely overbought!", rsi),
            }
        } else if rsi > OVERBOUGHT_THRESHOLD {
            // 🟥 Overbought - SELL
            self.stats.sell_signals += 1;
            Signal::Sell {
                price,
                size: 0.5,
                confidence,
                reason: format!("RSI {:.1} - Overbought", rsi),
            }
        } else {
            // ⏸️ Neutral zone - HOLD
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
        self.avg_gain = 0.0;
        self.avg_loss = 0.0;
        self.current_rsi = None;
        self.prev_price = None;
        self.stats = StrategyStats::default();
        self.last_signal = None;
    }
}

impl Default for RSIStrategy {
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
    async fn test_rsi_creation() {
        let strategy = RSIStrategy::new();
        assert_eq!(strategy.name(), "RSI (14)");
    }
    
    #[tokio::test]
    async fn test_oversold_signal() {
        let mut strategy = RSIStrategy::new();
        
        // Simulate strong downtrend (prices falling)
        let prices = vec![
            200.0, 198.0, 195.0, 192.0, 189.0,
            186.0, 183.0, 180.0, 177.0, 174.0,
            171.0, 168.0, 165.0, 162.0, 159.0,
        ];
        
        let mut last_signal = Signal::Hold;
        
        for price in prices {
            let signal = strategy.analyze(price, 0).await.unwrap();
            last_signal = signal;
        }
        
        // Should detect oversold condition
        assert!(
            last_signal.is_bullish(),
            "Should generate buy signal when RSI is oversold"
        );
    }
    
    #[tokio::test]
    async fn test_overbought_signal() {
        let mut strategy = RSIStrategy::new();
        
        // Simulate strong uptrend (prices rising)
        let prices = vec![
            160.0, 163.0, 166.0, 169.0, 172.0,
            175.0, 178.0, 181.0, 184.0, 187.0,
            190.0, 193.0, 196.0, 199.0, 202.0,
        ];
        
        let mut last_signal = Signal::Hold;
        
        for price in prices {
            let signal = strategy.analyze(price, 0).await.unwrap();
            last_signal = signal;
        }
        
        // Should detect overbought condition
        assert!(
            last_signal.is_bearish(),
            "Should generate sell signal when RSI is overbought"
        );
    }
    
    #[tokio::test]
    async fn test_neutral_zone() {
        let mut strategy = RSIStrategy::new();
        
        // Simulate sideways movement
        let prices = vec![
            180.0, 181.0, 180.0, 181.0, 180.0,
            181.0, 180.0, 181.0, 180.0, 181.0,
            180.0, 181.0, 180.0, 181.0, 180.0,
        ];
        
        let mut last_signal = Signal::Hold;
        
        for price in prices {
            let signal = strategy.analyze(price, 0).await.unwrap();
            last_signal = signal;
        }
        
        // Should hold in neutral zone
        assert!(
            matches!(last_signal, Signal::Hold),
            "Should hold when RSI is in neutral zone"
        );
    }
}
