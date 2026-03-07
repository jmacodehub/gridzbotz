//! ═══════════════════════════════════════════════════════════════════════════
//! Volatility Calculator — Real-Time Regime Detection
//!
//! Relocated from src/strategies/volatility_calc.rs (PR #70).
//! Pure computation — no trading decisions, no strategy coupling.
//!
//! Computes rolling standard deviation of percentage returns from a
//! price stream.  Used by GridRebalancer and other strategies for
//! adaptive spacing and regime detection.
//!
//! Based on GIGA test insight: 0.15% optimal for normal vol (~1.6 ATR)
//! ═══════════════════════════════════════════════════════════════════════════

use std::collections::VecDeque;

/// Real-time volatility calculator using rolling standard deviation of returns.
///
/// Feed prices via `add_price()`, read volatility via `get_volatility()`.
/// Requires ≥30 samples before producing a non-zero value.
#[derive(Debug, Clone)]
pub struct VolatilityCalculator {
    /// Price samples for volatility calculation
    price_history: VecDeque<f64>,

    /// Maximum samples to keep (e.g. 600 = 10 minutes at 1s intervals)
    max_samples: usize,

    /// Current volatility (standard deviation of % returns)
    current_volatility: f64,
}

impl VolatilityCalculator {
    pub fn new(max_samples: usize) -> Self {
        Self {
            price_history: VecDeque::with_capacity(max_samples),
            max_samples,
            current_volatility: 0.0,
        }
    }

    /// Add new price sample and recalculate volatility.
    /// Requires ≥30 samples before producing a non-zero value.
    pub fn add_price(&mut self, price: f64) {
        self.price_history.push_back(price);

        if self.price_history.len() > self.max_samples {
            self.price_history.pop_front();
        }

        if self.price_history.len() >= 30 {
            self.recalculate();
        }
    }

    /// Recalculate volatility as standard deviation of percentage returns.
    fn recalculate(&mut self) {
        if self.price_history.len() < 2 {
            return;
        }

        let mut returns = Vec::with_capacity(self.price_history.len() - 1);
        for i in 1..self.price_history.len() {
            let prev = self.price_history[i - 1];
            let curr = self.price_history[i];
            returns.push(((curr - prev) / prev) * 100.0);
        }

        let mean: f64 = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance: f64 = returns.iter()
            .map(|r| (r - mean).powi(2))
            .sum::<f64>() / returns.len() as f64;

        self.current_volatility = variance.sqrt();
    }

    /// Current rolling volatility (std dev of % returns).
    pub fn get_volatility(&self) -> f64 {
        self.current_volatility
    }

    /// Optimal grid spacing based on volatility regime.
    /// Based on GIGA test insight: 0.15% optimal for normal vol (~1.6 ATR).
    pub fn get_optimal_spacing(&self) -> f64 {
        match self.current_volatility {
            v if v < 0.5 => 0.10,   // Very low vol: tighter spacing
            v if v < 1.0 => 0.15,   // Low vol: optimal spacing (winner!)
            v if v < 1.5 => 0.20,   // Normal vol: balanced
            v if v < 2.0 => 0.30,   // High vol: conservative
            v if v < 3.0 => 0.50,   // Very high vol: wide spacing
            _ => 0.75,              // Extreme vol: ultra wide
        }
    }

    /// Optimal number of grid levels based on volatility.
    /// Based on GIGA test: 35 levels optimal at 0.15%.
    pub fn get_optimal_levels(&self) -> u32 {
        match self.get_optimal_spacing() {
            s if s <= 0.10 => 40,
            s if s <= 0.15 => 35,   // Winner configuration
            s if s <= 0.20 => 25,
            s if s <= 0.30 => 15,
            s if s <= 0.50 => 10,
            _ => 8,
        }
    }

    /// Human-readable market regime label.
    pub fn get_market_regime(&self) -> &str {
        match self.current_volatility {
            v if v < 0.5 => "VERY_LOW_VOLATILITY",
            v if v < 1.0 => "LOW_VOLATILITY",
            v if v < 1.5 => "NORMAL_VOLATILITY",
            v if v < 2.0 => "HIGH_VOLATILITY",
            v if v < 3.0 => "VERY_HIGH_VOLATILITY",
            _ => "EXTREME_VOLATILITY",
        }
    }

    /// Number of price samples currently buffered.
    pub fn sample_count(&self) -> usize {
        self.price_history.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_low_volatility_detection() {
        let mut calc = VolatilityCalculator::new(100);
        // Stable uptrend: +$0.01 per tick
        for i in 0..50 {
            calc.add_price(100.0 + (i as f64 * 0.01));
        }
        assert!(calc.get_volatility() < 0.1, "Stable trend should be low vol");
        assert_eq!(calc.get_optimal_spacing(), 0.10);
        assert_eq!(calc.get_market_regime(), "VERY_LOW_VOLATILITY");
    }

    #[test]
    fn test_needs_minimum_samples() {
        let mut calc = VolatilityCalculator::new(100);
        for i in 0..29 {
            calc.add_price(100.0 + i as f64);
        }
        assert_eq!(calc.get_volatility(), 0.0, "< 30 samples should give 0");
    }

    #[test]
    fn test_sample_count_caps_at_max() {
        let mut calc = VolatilityCalculator::new(50);
        for i in 0..60 {
            calc.add_price(100.0 + i as f64);
        }
        assert_eq!(calc.sample_count(), 50, "Should cap at max_samples");
    }

    #[test]
    fn test_optimal_levels_tracks_spacing() {
        let mut calc = VolatilityCalculator::new(100);
        // Force low vol → tight spacing → many levels
        for i in 0..50 {
            calc.add_price(100.0 + (i as f64 * 0.001));
        }
        assert!(calc.get_optimal_levels() >= 35);
    }

    #[test]
    fn test_high_volatility_widens_spacing() {
        let mut calc = VolatilityCalculator::new(100);
        // Simulate volatile market: alternating +/- $2
        for i in 0..50 {
            let swing = if i % 2 == 0 { 2.0 } else { -2.0 };
            calc.add_price(100.0 + swing);
        }
        assert!(calc.get_volatility() > 1.0, "Swinging market should be high vol");
        assert!(calc.get_optimal_spacing() >= 0.20, "High vol → wider spacing");
    }
}
