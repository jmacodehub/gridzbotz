// ═══════════════════════════════════════════════════════════════════════════
// VOLATILITY ANALYZER - V2.2 (PROJECT FLASH V5 READY)
// ═══════════════════════════════════════════════════════════════════════════
//
// Purpose:
//   - Provides shared volatility metrics for all strategies
//   - Supports StdDev + Range + Combined hybrid index
//   - Extensively tested for stability, accuracy, and modular integration
//
// New in V2.2 (Test-Ready):
//   ✅ Expanded test suite with multi-scenario coverage
//   ✅ Edge case handling: low sample volumes, NaN, negative values
//   ✅ Public reset() and mean return helpers for cleaner testing
//   ✅ Synchronous and deterministic test output
//
// ═══════════════════════════════════════════════════════════════════════════

use log::{debug, warn};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolatilityConfig {
    pub max_samples: usize,
    pub min_samples: usize,
    #[serde(default)]
    pub verbose: bool,
}

impl Default for VolatilityConfig {
    fn default() -> Self {
        Self {
            max_samples: 600,
            min_samples: 30,
            verbose: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VolatilityCalculator {
    price_history: VecDeque<f64>,
    config: VolatilityConfig,
    current_stddev_vol: f64,
    display_volatility: f64,
}

impl VolatilityCalculator {
    pub fn new(config: VolatilityConfig) -> Self {
        Self {
            price_history: VecDeque::with_capacity(config.max_samples),
            config,
            current_stddev_vol: 0.0,
            display_volatility: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.price_history.clear();
        self.current_stddev_vol = 0.0;
        self.display_volatility = 0.0;
    }

    pub fn add_price(&mut self, price: f64) {
        if !price.is_finite() || price <= 0.0 {
            warn!("Invalid price {:.6} ignored", price);
            return;
        }
        self.price_history.push_back(price);
        if self.price_history.len() > self.config.max_samples {
            self.price_history.pop_front();
        }
        if self.price_history.len() >= self.config.min_samples {
            self.calculate_stddev();
            self.calculate_range();
        }
    }

    fn calculate_stddev(&mut self) {
        if self.price_history.len() < 2 {
            self.current_stddev_vol = 0.0;
            return;
        }

        let mut returns = Vec::new();
        for i in 1..self.price_history.len() {
            let prev = self.price_history[i - 1];
            let curr = self.price_history[i];
            if prev > 0.0 {
                returns.push(((curr - prev) / prev) * 100.0);
            }
        }

        if returns.is_empty() {
            self.current_stddev_vol = 0.0;
            return;
        }

        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance =
            returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / returns.len() as f64;
        self.current_stddev_vol = variance.sqrt();

        if self.config.verbose {
            debug!(
                "StdDev Volatility {:.4}% from {} samples",
                self.current_stddev_vol,
                returns.len()
            );
        }
    }

    fn calculate_range(&mut self) {
        if self.price_history.len() < self.config.min_samples {
            self.display_volatility = 0.0;
            return;
        }
        let high = self
            .price_history
            .iter()
            .fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let low = self
            .price_history
            .iter()
            .fold(f64::INFINITY, |a, &b| a.min(b));
        self.display_volatility = if low > 0.0 {
            ((high - low) / low) * 100.0
        } else {
            0.0
        };
        if self.config.verbose {
            debug!(
                "Range Volatility {:.3}%, high {:.3}, low {:.3}",
                self.display_volatility, high, low
            );
        }
    }

    pub fn stddev_volatility(&self) -> f64 {
        self.current_stddev_vol
    }
    pub fn range_volatility(&self) -> f64 {
        self.display_volatility
    }
    pub fn combined_index(&self) -> f64 {
        (self.current_stddev_vol * 0.6) + (self.display_volatility * 0.4)
    }

    pub fn optimal_spacing(&self) -> f64 {
        let v = self.current_stddev_vol;
        match v {
            v if v < 0.5 => 0.10,
            v if v < 1.0 => 0.15,
            v if v < 1.5 => 0.20,
            v if v < 2.0 => 0.30,
            v if v < 3.0 => 0.50,
            _ => 0.75,
        }
    }

    pub fn sample_count(&self) -> usize {
        self.price_history.len()
    }

    pub fn stats(&self) -> Option<VolatilityStats> {
        if self.price_history.is_empty() {
            return None;
        }
        let high = self
            .price_history
            .iter()
            .fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let low = self
            .price_history
            .iter()
            .fold(f64::INFINITY, |a, &b| a.min(b));
        let mean = self.price_history.iter().sum::<f64>() / self.price_history.len() as f64;
        Some(VolatilityStats {
            high,
            low,
            mean,
            range: high - low,
            stddev_volatility: self.current_stddev_vol,
            range_volatility: self.display_volatility,
            samples: self.price_history.len(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VolatilityStats {
    pub high: f64,
    pub low: f64,
    pub mean: f64,
    pub range: f64,
    pub stddev_volatility: f64,
    pub range_volatility: f64,
    pub samples: usize,
}

// ═══════════════════════════════════════════════════════════════════════════
// TEST SUITE (Phase 3 Enhancements)
// ═══════════════════════════════════════════════════════════════════════════
#[cfg(test)]
mod tests {
    use super::*;

    fn build_calc() -> VolatilityCalculator {
        VolatilityCalculator::new(VolatilityConfig {
            verbose: false,
            ..Default::default()
        })
    }

    #[test]
    fn test_low_volatility_should_respect_thresholds() {
        let mut calc = build_calc();
        for i in 0..60 {
            calc.add_price(100.0 + i as f64 * 0.01);
        }
        assert!(
            calc.stddev_volatility() < 0.5,
            "Vol too high for calm market"
        );
        assert!(calc.optimal_spacing() <= 0.10);
    }

    #[test]
    fn test_high_volatility_detects_range_and_stddev() {
        let mut calc = build_calc();
        for i in 0..120 {
            let price = if i % 2 == 0 { 100.0 } else { 105.0 };
            calc.add_price(price);
        }
        assert!(calc.range_volatility() > 4.0, "Range vol should exceed 4%");
        assert!(calc.stddev_volatility() > 1.0, "Stddev vol >1%");
    }

    #[test]
    fn test_combined_index_weighted_value() {
        let mut calc = build_calc();
        for i in 0..60 {
            calc.add_price(100.0 + (i as f64 * 0.2));
        }
        let combined = calc.combined_index();
        let sv = calc.stddev_volatility();
        let rv = calc.range_volatility();
        assert!((combined - (sv * 0.6 + rv * 0.4)).abs() < 1e-6);
    }

    #[test]
    fn test_small_sample_behavior() {
        let mut calc = build_calc();
        calc.add_price(100.0);
        calc.add_price(101.0);
        assert_eq!(
            calc.stddev_volatility(),
            0.0,
            "Small sample should not compute"
        );
        assert_eq!(calc.range_volatility(), 0.0);
    }

    #[test]
    fn test_reset_functionality() {
        let mut calc = build_calc();
        for i in 0..100 {
            calc.add_price(90.0 + i as f64 * 0.5);
        }
        calc.reset();
        assert_eq!(calc.sample_count(), 0);
        assert_eq!(calc.stddev_volatility(), 0.0);
    }

    #[test]
    fn test_stats_output_consistency() {
        let mut calc = build_calc();
        for i in 0..80 {
            calc.add_price(200.0 + i as f64 * 0.2);
        }
        let stats = calc.stats().unwrap();
        assert!(stats.high > stats.low);
        assert!(stats.range_volatility >= 0.0);
    }
}
