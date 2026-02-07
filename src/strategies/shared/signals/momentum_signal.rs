// ═══════════════════════════════════════════════════════════════════════════
// 📈 MOMENTUM SIGNAL MODULE - PROJECT FLASH V5+ (Phase 3 Modular Edition)
// ═══════════════════════════════════════════════════════════════════════════
//
// V5+ Enhancements:
//   ✅ Uses SHARED SignalModule trait from parent mod.rs
//   ✅ Pluggable momentum indicator for Shared::AnalyticsContext
//   ✅ Fast/slow EMA crossover detection (Golden / Death Cross)
//   ✅ Returns float signal strength for cross-strategy use
//   ✅ Deterministic tests for CI + low latency EMA math
//
// Momentum Signal Logic:
//   - Golden Cross (fast > slow) → positive signal (+1 → buy bias)
//   - Death Cross (fast < slow) → negative signal (-1 → sell bias)
//   - Neutral → 0
//
// October 2025 - V5+ Production Edition
// ═══════════════════════════════════════════════════════════════════════════

use async_trait::async_trait;
use std::cmp::Ordering;
use std::collections::VecDeque;

// ✅ CRITICAL: Import the SHARED trait from parent module (signals/mod.rs)
use super::SignalModule;

// ═══════════════════════════════════════════════════════════════════════════
// CROSS EVENT ENUM
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cross {
    Golden,
    Death,
}

// ═══════════════════════════════════════════════════════════════════════════
// MOMENTUM STRUCT
// ═══════════════════════════════════════════════════════════════════════════

pub struct MomentumSignal {
    fast_prices: VecDeque<f64>,
    slow_prices: VecDeque<f64>,
    prev_fast_ema: Option<f64>,
    prev_slow_ema: Option<f64>,
    current_value: Option<f64>,
    fast: usize,
    slow: usize,
}

impl Default for MomentumSignal {
    fn default() -> Self {
        Self::new(9, 21)
    }
}

impl MomentumSignal {
    pub fn new(fast: usize, slow: usize) -> Self {
        Self {
            fast_prices: VecDeque::with_capacity(fast),
            slow_prices: VecDeque::with_capacity(slow),
            prev_fast_ema: None,
            prev_slow_ema: None,
            current_value: None,
            fast,
            slow,
        }
    }

    fn ema(prices: &VecDeque<f64>, prev: Option<f64>) -> Option<f64> {
        if prices.is_empty() {
            return None;
        }
        let multiplier = 2.0 / (prices.len() as f64 + 1.0);
        let latest = *prices.back()?;
        let ema = match prev {
            None => prices.iter().sum::<f64>() / prices.len() as f64,
            Some(p) => (latest * multiplier) + (p * (1.0 - multiplier)),
        };
        Some(ema)
    }

    fn detect_cross(prev_f: f64, prev_s: f64, f: f64, s: f64) -> Option<Cross> {
        match (prev_f.partial_cmp(&prev_s), f.partial_cmp(&s)) {
            (Some(Ordering::Less), Some(Ordering::Greater)) => Some(Cross::Golden),
            (Some(Ordering::Greater), Some(Ordering::Less)) => Some(Cross::Death),
            _ => None,
        }
    }

    fn calc_strength(fast: f64, slow: f64) -> f64 {
        ((fast - slow) / slow).clamp(-1.0, 1.0)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// SIGNALMODULE IMPLEMENTATION - Using SHARED Trait
// ═══════════════════════════════════════════════════════════════════════════

#[async_trait]
impl SignalModule for MomentumSignal {
    fn name(&self) -> &str {
        "Momentum Signal"
    }

    async fn compute(&mut self, price: f64) -> f64 {
        self.fast_prices.push_back(price);
        self.slow_prices.push_back(price);

        if self.fast_prices.len() > self.fast {
            self.fast_prices.pop_front();
        }
        if self.slow_prices.len() > self.slow {
            self.slow_prices.pop_front();
        }

        // Need enough data for slow EMA
        if self.slow_prices.len() < self.slow {
            self.current_value = None;
            return 0.0;
        }

        let fast_ema = Self::ema(&self.fast_prices, self.prev_fast_ema).unwrap_or(price);
        let slow_ema = Self::ema(&self.slow_prices, self.prev_slow_ema).unwrap_or(price);

        let cross = Self::detect_cross(
            self.prev_fast_ema.unwrap_or(fast_ema),
            self.prev_slow_ema.unwrap_or(slow_ema),
            fast_ema,
            slow_ema,
        );

        let strength = Self::calc_strength(fast_ema, slow_ema);

        // Encode cross events as boosted signal bias
        let signal_value = match cross {
            Some(Cross::Golden) => 1.0,
            Some(Cross::Death) => -1.0,
            _ => strength,
        };

        self.prev_fast_ema = Some(fast_ema);
        self.prev_slow_ema = Some(slow_ema);
        self.current_value = Some(signal_value);
        signal_value
    }

    fn last_value(&self) -> Option<f64> {
        self.current_value
    }

    fn reset(&mut self) {
        self.fast_prices.clear();
        self.slow_prices.clear();
        self.prev_fast_ema = None;
        self.prev_slow_ema = None;
        self.current_value = None;
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
    fn test_momentum_signal_uptrend() {
        let mut s = MomentumSignal::new(5, 9);
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let mut val = 0.0;
            for p in [
                100.0, 102.0, 105.0, 108.0, 111.0, 114.0, 118.0, 121.0, 125.0,
            ] {
                val = s.compute(p).await;
            }
            assert!(val > 0.0, "Expected positive momentum signal, got: {}", val);
        });
    }

    #[test]
    fn test_momentum_signal_downtrend() {
        let mut s = MomentumSignal::new(5, 9);
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let mut val = 0.0;
            for p in [120.0, 118.0, 115.0, 112.0, 109.0, 106.0, 102.0, 98.0, 95.0] {
                val = s.compute(p).await;
            }
            assert!(val < 0.0, "Expected negative momentum signal, got: {}", val);
        });
    }

    #[test]
    fn test_cross_event_detection() {
        let mut s = MomentumSignal::new(3, 5);
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            for p in [100.0, 101.0, 102.0, 103.0, 104.0, 105.0] {
                s.compute(p).await;
            }
            let v = s.last_value().unwrap();
            assert!(v > 0.0, "Expected positive momentum after uptrend");
        });
    }

    #[test]
    fn test_momentum_reset() {
        let mut s = MomentumSignal::new(5, 9);
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            s.compute(100.0).await;
            s.compute(105.0).await;
            assert!(s.last_value().is_some() || !s.fast_prices.is_empty());

            s.reset();
            assert!(s.last_value().is_none(), "Value should be None after reset");
            assert!(s.fast_prices.is_empty(), "Fast prices should be cleared");
            assert!(s.slow_prices.is_empty(), "Slow prices should be cleared");
        });
    }
}
