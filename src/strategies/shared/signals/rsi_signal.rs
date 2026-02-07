// ═══════════════════════════════════════════════════════════════════════════
// 📊 RSI SIGNAL MODULE - PROJECT FLASH V5+ (Phase 3 Modular Edition)
// ═══════════════════════════════════════════════════════════════════════════
//
// V5+ Enhancements:
//   ✅ Uses SHARED SignalModule trait from parent mod.rs
//   ✅ Clean modular architecture - NO duplicate trait definitions
//   ✅ Configurable Period, Thresholds, Async-safe math
//   ✅ Ready for cross-strategy signal aggregation in Phase 4
//
// RSI Logic:
//   - RSI < 30 → Bullish (Oversold)
//   - RSI > 70 → Bearish (Overbought)
//   - RSI < 20 → Strong Buy
//   - RSI > 80 → Strong Sell
//
// October 2025 - V5+ Production Edition
// ═══════════════════════════════════════════════════════════════════════════

use async_trait::async_trait;
use std::collections::VecDeque;

// ✅ CRITICAL: Import the SHARED trait from parent module (signals/mod.rs)
use super::SignalModule;

// ═══════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════

const DEFAULT_PERIOD: usize = 14;
const OVERSOLD: f64 = 30.0;
const OVERBOUGHT: f64 = 70.0;
const EXT_OVERSOLD: f64 = 20.0;
const EXT_OVERBOUGHT: f64 = 80.0;

// ═══════════════════════════════════════════════════════════════════════════
// RSI STRUCT - encapsulated indicator logic
// ═══════════════════════════════════════════════════════════════════════════

pub struct RsiSignal {
    prices: VecDeque<f64>,
    avg_gain: f64,
    avg_loss: f64,
    prev_price: Option<f64>,
    current_rsi: Option<f64>,
    period: usize,
}

impl Default for RsiSignal {
    fn default() -> Self {
        Self::new(DEFAULT_PERIOD)
    }
}

impl RsiSignal {
    pub fn new(period: usize) -> Self {
        Self {
            prices: VecDeque::with_capacity(period + 1),
            avg_gain: 0.0,
            avg_loss: 0.0,
            prev_price: None,
            current_rsi: None,
            period,
        }
    }

    fn calc_initial_avgs(&self) -> (f64, f64) {
        if self.prices.len() <= 1 {
            return (0.0, 0.0);
        }

        let mut gain = 0.0;
        let mut loss = 0.0;
        for i in 1..self.prices.len() {
            let delta = self.prices[i] - self.prices[i - 1];
            if delta > 0.0 {
                gain += delta;
            } else {
                loss += delta.abs();
            }
        }
        (gain / self.period as f64, loss / self.period as f64)
    }

    fn update_averages(&mut self, price: f64) {
        if let Some(prev) = self.prev_price {
            let change = price - prev;
            if self.prices.len() == self.period {
                let (g, l) = self.calc_initial_avgs();
                self.avg_gain = g;
                self.avg_loss = l;
            } else if self.prices.len() > self.period {
                if change > 0.0 {
                    self.avg_gain = ((self.avg_gain * (self.period as f64 - 1.0)) + change)
                        / self.period as f64;
                    self.avg_loss =
                        (self.avg_loss * (self.period as f64 - 1.0)) / self.period as f64;
                } else {
                    self.avg_gain =
                        (self.avg_gain * (self.period as f64 - 1.0)) / self.period as f64;
                    self.avg_loss = ((self.avg_loss * (self.period as f64 - 1.0)) + change.abs())
                        / self.period as f64;
                }
            }
        }
        self.prev_price = Some(price);
    }

    fn calc_rsi(&self) -> Option<f64> {
        if self.avg_loss == 0.0 {
            return Some(100.0);
        }
        let rs = self.avg_gain / self.avg_loss;
        Some(100.0 - (100.0 / (1.0 + rs)))
    }

    pub fn classify_signal(&self) -> &'static str {
        match self.current_rsi {
            Some(rsi) if rsi <= EXT_OVERSOLD => "strong_buy",
            Some(rsi) if rsi < OVERSOLD => "buy",
            Some(rsi) if rsi >= EXT_OVERBOUGHT => "strong_sell",
            Some(rsi) if rsi > OVERBOUGHT => "sell",
            Some(_) => "neutral",
            None => "warming_up",
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// SIGNALMODULE IMPLEMENTATION - Using SHARED Trait
// ═══════════════════════════════════════════════════════════════════════════

#[async_trait]
impl SignalModule for RsiSignal {
    fn name(&self) -> &str {
        "RSI Signal"
    }

    async fn compute(&mut self, price: f64) -> f64 {
        self.prices.push_back(price);
        if self.prices.len() > self.period + 1 {
            self.prices.pop_front();
        }
        self.update_averages(price);

        if self.prices.len() < self.period {
            self.current_rsi = None;
            return 50.0;
        }

        let rsi = self.calc_rsi().unwrap_or(50.0);
        self.current_rsi = Some(rsi);
        rsi
    }

    fn last_value(&self) -> Option<f64> {
        self.current_rsi
    }

    fn reset(&mut self) {
        self.prices.clear();
        self.avg_gain = 0.0;
        self.avg_loss = 0.0;
        self.prev_price = None;
        self.current_rsi = None;
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS - V5+ Edition
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::runtime::Runtime;

    #[test]
    fn test_rsi_signal_basic() {
        let mut sig = RsiSignal::new(14);
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let mut price = 100.0;
            for _ in 0..20 {
                sig.compute(price).await;
                price += 1.0;
            }
            let val = sig.last_value().unwrap();
            assert!(val > 0.0 && val <= 100.0, "RSI should be between 0-100");
        });
    }

    #[test]
    fn test_signal_classification() {
        let mut sig = RsiSignal::new(14);
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            // Simulate downtrend for bearish RSI
            for p in [
                200.0, 198.0, 195.0, 192.0, 189.0, 186.0, 183.0, 180.0, 177.0, 174.0, 171.0, 168.0,
                165.0, 162.0, 159.0,
            ] {
                sig.compute(p).await;
            }
            let classification = sig.classify_signal();
            assert!(
                matches!(classification, "buy" | "strong_buy"),
                "Expected bullish classification, got: {}",
                classification
            );
        });
    }

    #[test]
    fn test_rsi_reset() {
        let mut sig = RsiSignal::new(14);
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            sig.compute(100.0).await;
            sig.compute(105.0).await;
            assert!(sig.last_value().is_some() || sig.prices.len() > 0);

            sig.reset();
            assert!(sig.last_value().is_none(), "RSI should be None after reset");
            assert!(
                sig.prices.is_empty(),
                "Prices should be cleared after reset"
            );
        });
    }
}
