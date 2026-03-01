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
//! ```text
//! RSI = 25 (Oversold)
//! Signal: STRONG BUY 🟢
//! Reason: "Price oversold, bounce expected"
//! ```

use super::{Strategy, Signal, StrategyStats};
use async_trait::async_trait;
use anyhow::Result;
use std::collections::VecDeque;

// ═══════════════════════════════════════════════════════════════════════════
// DEFAULTS (module-private — callers use RsiConfig)
// ═══════════════════════════════════════════════════════════════════════════

const DEFAULT_RSI_PERIOD: usize = 14;
const DEFAULT_OVERSOLD_THRESHOLD: f64 = 30.0;
const DEFAULT_OVERBOUGHT_THRESHOLD: f64 = 70.0;
const DEFAULT_EXTREME_OVERSOLD: f64 = 20.0;
const DEFAULT_EXTREME_OVERBOUGHT: f64 = 80.0;

// ═══════════════════════════════════════════════════════════════════════════
// RSI CONFIG
// ═══════════════════════════════════════════════════════════════════════════

/// Runtime-tunable parameters for the RSI strategy.
/// Sourced from TOML at startup — zero hardcoded decisions in the hot path.
#[derive(Debug, Clone)]
pub struct RsiConfig {
    /// RSI calculation period (default: 14)
    pub rsi_period: usize,
    /// Oversold threshold — RSI below this = potential buy (default: 30.0)
    pub oversold_threshold: f64,
    /// Overbought threshold — RSI above this = potential sell (default: 70.0)
    pub overbought_threshold: f64,
    /// Extreme oversold — triggers StrongBuy (default: 20.0)
    pub extreme_oversold: f64,
    /// Extreme overbought — triggers StrongSell (default: 80.0)
    pub extreme_overbought: f64,
}

impl Default for RsiConfig {
    fn default() -> Self {
        Self {
            rsi_period: DEFAULT_RSI_PERIOD,
            oversold_threshold: DEFAULT_OVERSOLD_THRESHOLD,
            overbought_threshold: DEFAULT_OVERBOUGHT_THRESHOLD,
            extreme_oversold: DEFAULT_EXTREME_OVERSOLD,
            extreme_overbought: DEFAULT_EXTREME_OVERBOUGHT,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 🆕 V5.1: TOML → RsiConfig ADAPTER
// Zero-cost bridge — converts config::RsiParams → RsiConfig for RSIStrategy.
// ═══════════════════════════════════════════════════════════════════════════

impl From<&crate::config::RsiParams> for RsiConfig {
    fn from(p: &crate::config::RsiParams) -> Self {
        Self {
            rsi_period: p.rsi_period,
            oversold_threshold: p.oversold_threshold,
            overbought_threshold: p.overbought_threshold,
            extreme_oversold: p.extreme_oversold,
            extreme_overbought: p.extreme_overbought,
        }
    }
}

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

    // ── Config (captured at construction, immutable in hot path) ──────────
    rsi_period: usize,
    oversold_threshold: f64,
    overbought_threshold: f64,
    extreme_oversold: f64,
    extreme_overbought: f64,
}

impl RSIStrategy {
    /// Create from explicit config — preferred in production.
    pub fn new_from_config(cfg: &RsiConfig) -> Self {
        Self {
            name: format!("RSI ({})", cfg.rsi_period),
            price_history: VecDeque::with_capacity(cfg.rsi_period + 1),
            avg_gain: 0.0,
            avg_loss: 0.0,
            current_rsi: None,
            prev_price: None,
            stats: StrategyStats::default(),
            last_signal: None,
            rsi_period: cfg.rsi_period,
            oversold_threshold: cfg.oversold_threshold,
            overbought_threshold: cfg.overbought_threshold,
            extreme_oversold: cfg.extreme_oversold,
            extreme_overbought: cfg.extreme_overbought,
        }
    }

    /// Create new RSI strategy with default parameters.
    pub fn new() -> Self {
        Self::new_from_config(&RsiConfig::default())
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
    
    /// Update average gain and loss with new price using Wilder's smoothing.
    /// Uses (period-1)/period ratio — correct for any configured RSI period.
    fn update_averages(&mut self, price: f64) {
        if let Some(prev_price) = self.prev_price {
            let change = price - prev_price;
            
            // First RSI calculation (initial averages)
            if self.price_history.len() == self.rsi_period {
                let (gains, losses) = self.calculate_initial_averages();
                self.avg_gain = gains;
                self.avg_loss = losses;
            } else if self.price_history.len() > self.rsi_period {
                let smooth_prev = (self.rsi_period - 1) as f64;
                let smooth_denom = self.rsi_period as f64;
                // Subsequent calculations (smoothed)
                if change > 0.0 {
                    self.avg_gain = ((self.avg_gain * smooth_prev) + change) / smooth_denom;
                    self.avg_loss = (self.avg_loss * smooth_prev) / smooth_denom;
                } else {
                    self.avg_gain = (self.avg_gain * smooth_prev) / smooth_denom;
                    self.avg_loss = ((self.avg_loss * smooth_prev) + change.abs()) / smooth_denom;
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
        
        (total_gain / self.rsi_period as f64, total_loss / self.rsi_period as f64)
    }
    
    /// Calculate confidence based on RSI extremity
    /// 
    /// More extreme RSI = Higher confidence
    fn calculate_confidence(&self, rsi: f64) -> f64 {
        if rsi <= self.extreme_oversold {
            1.0
        } else if rsi < self.oversold_threshold {
            0.7 + (0.3 * (self.oversold_threshold - rsi) / (self.oversold_threshold - self.extreme_oversold))
        } else if rsi >= self.extreme_overbought {
            1.0
        } else if rsi > self.overbought_threshold {
            0.7 + (0.3 * (rsi - self.overbought_threshold) / (self.extreme_overbought - self.overbought_threshold))
        } else {
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
        if self.price_history.len() > self.rsi_period + 1 {
            self.price_history.pop_front();
        }
        
        // STEP 2: Update averages
        self.update_averages(price);
        
        // STEP 3: Need enough data to calculate RSI
        if self.price_history.len() < self.rsi_period {
            self.stats.signals_generated += 1;
            self.stats.hold_signals += 1;
            return Ok(Signal::Hold { reason: None });
        }
        
        // STEP 4: Calculate RSI
        let rsi = self.calculate_rsi().unwrap_or(50.0);
        self.current_rsi = Some(rsi);
        
        // STEP 5: Calculate confidence
        let confidence = self.calculate_confidence(rsi);
        
        // STEP 6: Generate trading signal
        let signal = if rsi <= self.extreme_oversold {
            // 🟢 Extremely oversold - STRONG BUY!
            self.stats.buy_signals += 1;
            Signal::StrongBuy {
                price,
                size: 1.0,
                confidence,
                reason: format!("RSI {:.1} - Extremely oversold!", rsi),
                level_id: None,
            }
        } else if rsi < self.oversold_threshold {
            // 🟩 Oversold - BUY
            self.stats.buy_signals += 1;
            Signal::Buy {
                price,
                size: 0.5,
                confidence,
                reason: format!("RSI {:.1} - Oversold", rsi),
                level_id: None,
            }
        } else if rsi >= self.extreme_overbought {
            // 🔴 Extremely overbought - STRONG SELL!
            self.stats.sell_signals += 1;
            Signal::StrongSell {
                price,
                size: 1.0,
                confidence,
                reason: format!("RSI {:.1} - Extremely overbought!", rsi),
                level_id: None,
            }
        } else if rsi > self.overbought_threshold {
            // 🟥 Overbought - SELL
            self.stats.sell_signals += 1;
            Signal::Sell {
                price,
                size: 0.5,
                confidence,
                reason: format!("RSI {:.1} - Overbought", rsi),
                level_id: None,
            }
        } else {
            // ⏸️ Neutral zone - HOLD
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
    async fn test_config_driven_creation() {
        let cfg = RsiConfig {
            rsi_period: 10,
            oversold_threshold: 25.0,
            overbought_threshold: 75.0,
            extreme_oversold: 15.0,
            extreme_overbought: 85.0,
        };
        let s = RSIStrategy::new_from_config(&cfg);
        assert_eq!(s.name(), "RSI (10)");
        assert_eq!(s.rsi_period, 10);
        assert!((s.oversold_threshold - 25.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_toml_params_adapter() {
        use crate::config::RsiParams;
        let toml_params = RsiParams {
            rsi_period: 12,
            oversold_threshold: 35.0,
            overbought_threshold: 65.0,
            extreme_oversold: 25.0,
            extreme_overbought: 75.0,
        };
        let cfg = RsiConfig::from(&toml_params);
        let s = RSIStrategy::new_from_config(&cfg);
        assert_eq!(s.name(), "RSI (12)");
        assert_eq!(s.rsi_period, 12);
        assert!((s.oversold_threshold - 35.0).abs() < f64::EPSILON);
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
        
        let mut last_signal = Signal::Hold { reason: None };
        
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
        
        let mut last_signal = Signal::Hold { reason: None };
        
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
        
        let mut last_signal = Signal::Hold { reason: None };
        
        for price in prices {
            let signal = strategy.analyze(price, 0).await.unwrap();
            last_signal = signal;
        }
        
        // Should hold in neutral zone
        assert!(
            matches!(last_signal, Signal::Hold { .. }),
            "Should hold when RSI is in neutral zone"
        );
    }
}
