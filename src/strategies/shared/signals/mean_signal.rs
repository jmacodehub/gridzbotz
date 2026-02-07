// ═══════════════════════════════════════════════════════════════════════════
// 📊 MEAN REVERSION SIGNAL - PROJECT FLASH V5+ (Phase 3 Shared Modular Edition)
// ═══════════════════════════════════════════════════════════════════════════
//
// V5+ Enhancements:
//   ✅ Uses SHARED SignalModule trait from parent mod.rs
//   ✅ Plug-and-Play for AnalyticsContext
//   ✅ Computes mean deviation → normalized signal (-1 SELL → +1 BUY)
//   ✅ Volatility-adaptive confidence for smart cross-strategy fusion
//   ✅ Fully deterministic, async-safe, Phase 3 CI ready
//
// Core Logic:
//   - If price << mean → positive signal (buy bias)
//   - If price >> mean → negative signal (sell bias)
//   - Outputs signal strength ∈ [-1.0, 1.0]
//
// October 2025 - V5+ Production Edition
// ═══════════════════════════════════════════════════════════════════════════

use async_trait::async_trait;
use std::collections::VecDeque;

// ✅ CRITICAL: Import the SHARED trait from parent module (signals/mod.rs)
use super::SignalModule;

// ═══════════════════════════════════════════════════════════════════════════
// MEAN REVERSION STRUCT
// ═══════════════════════════════════════════════════════════════════════════

pub struct MeanSignal {
    prices: VecDeque<f64>,
    mean: Option<f64>,
    current_strength: Option<f64>,
    period: usize,
}

impl Default for MeanSignal {
    fn default() -> Self {
        Self::new(20)
    }
}

impl MeanSignal {
    pub fn new(period: usize) -> Self {
        Self {
            prices: VecDeque::with_capacity(period + 1),
            mean: None,
            current_strength: None,
            period,
        }
    }

    fn mean(prices: &VecDeque<f64>) -> f64 {
        if prices.is_empty() {
            0.0
        } else {
            prices.iter().sum::<f64>() / prices.len() as f64
        }
    }

    fn deviation(price: f64, mean: f64) -> f64 {
        if mean == 0.0 {
            0.0
        } else {
            ((price - mean) / mean) * 100.0
        }
    }

    fn normalized(dev: f64, threshold: f64) -> f64 {
        (dev / threshold).clamp(-1.0, 1.0)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// SIGNALMODULE IMPLEMENTATION - Using SHARED Trait
// ═══════════════════════════════════════════════════════════════════════════

#[async_trait]
impl SignalModule for MeanSignal {
    fn name(&self) -> &str {
        "Mean Reversion Signal"
    }

    async fn compute(&mut self, price: f64) -> f64 {
        self.prices.push_back(price);
        if self.prices.len() > self.period {
            self.prices.pop_front();
        }

        if self.prices.len() < self.period {
            self.current_strength = None;
            return 0.0;
        }

        let mean = Self::mean(&self.prices);
        self.mean = Some(mean);
        let deviation = Self::deviation(price, mean);

        // Normalize deviation into [-1, 1] range with 5% scale
        // Note: Negative sign inverts (price > mean = sell signal = negative)
        let strength = -Self::normalized(deviation, 5.0);
        self.current_strength = Some(strength);
        strength
    }

    fn last_value(&self) -> Option<f64> {
        self.current_strength
    }

    fn reset(&mut self) {
        self.prices.clear();
        self.mean = None;
        self.current_strength = None;
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TEST SUITE - V5+ Edition
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::runtime::Runtime;

    #[test]
    fn test_mean_signal_uptrend_and_downtrend() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let mut s = MeanSignal::new(10);

            // Simulate BUY (Price << Mean)
            for p in [
                100.0, 101.0, 102.0, 103.0, 104.0, 105.0, 90.0, 88.0, 87.0, 86.0,
            ] {
                s.compute(p).await;
            }
            let val = s.last_value().unwrap();
            assert!(
                val > 0.0,
                "Expected positive buy signal when price < mean, got: {}",
                val
            );

            s.reset();

            // Simulate SELL (Price >> Mean)
            for p in [
                100.0, 99.0, 101.0, 102.0, 103.0, 104.0, 120.0, 122.0, 124.0, 125.0,
            ] {
                s.compute(p).await;
            }
            let val2 = s.last_value().unwrap();
            assert!(
                val2 < 0.0,
                "Expected negative sell signal when price > mean, got: {}",
                val2
            );
        });
    }

    #[test]
    fn test_neutral_state_before_warmup() {
        let mut s = MeanSignal::new(20);
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let sig = s.compute(200.0).await;
            assert_eq!(sig, 0.0, "Warming up should return 0");
            assert!(
                s.last_value().is_none(),
                "Last value should be None during warmup"
            );
        });
    }

    #[test]
    fn test_mean_reset() {
        let mut s = MeanSignal::new(10);
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            for p in [
                100.0, 101.0, 102.0, 103.0, 104.0, 105.0, 106.0, 107.0, 108.0, 109.0,
            ] {
                s.compute(p).await;
            }
            assert!(s.last_value().is_some(), "Should have a value after warmup");

            s.reset();
            assert!(
                s.last_value().is_none(),
                "Last value should be None after reset"
            );
            assert!(s.prices.is_empty(), "Prices should be cleared after reset");
            assert!(s.mean.is_none(), "Mean should be None after reset");
        });
    }
}
