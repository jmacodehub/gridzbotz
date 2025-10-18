//! Volatility-Based Grid Spacing Calculator
//! Adapts grid spacing to market conditions
//! Based on GIGA test insight: 0.15% optimal for normal vol

use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct VolatilityCalculator {
    /// Price samples for volatility calculation
    price_history: VecDeque<f64>,
    
    /// Maximum samples to keep (10 minutes at 1 second intervals)
    max_samples: usize,
    
    /// Current volatility (standard deviation of returns)
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
    
    /// Add new price sample and update volatility
    pub fn add_price(&mut self, price: f64) {
        self.price_history.push_back(price);
        
        if self.price_history.len() > self.max_samples {
            self.price_history.pop_front();
        }
        
        if self.price_history.len() >= 30 {
            self.calculate_volatility();
        }
    }
    
    /// Calculate volatility as standard deviation of returns
    fn calculate_volatility(&mut self) {
        if self.price_history.len() < 2 {
            return;
        }
        
        // Calculate returns
        let mut returns = Vec::new();
        for i in 1..self.price_history.len() {
            let prev_price = self.price_history[i - 1];
            let curr_price = self.price_history[i];
            let return_pct = ((curr_price - prev_price) / prev_price) * 100.0;
            returns.push(return_pct);
        }
        
        // Calculate mean
        let mean: f64 = returns.iter().sum::<f64>() / returns.len() as f64;
        
        // Calculate variance
        let variance: f64 = returns.iter()
            .map(|r| (r - mean).powi(2))
            .sum::<f64>() / returns.len() as f64;
        
        // Standard deviation = volatility
        self.current_volatility = variance.sqrt();
    }
    
    /// Get current volatility
    pub fn get_volatility(&self) -> f64 {
        self.current_volatility
    }
    
    /// Get optimal grid spacing based on current volatility
    pub fn get_optimal_spacing(&self) -> f64 {
        // Based on GIGA test results:
        // 0.15% optimal for normal volatility (~1.6 ATR)
        // Scale up/down based on current conditions
        
        match self.current_volatility {
            v if v < 0.5 => 0.10,   // Very low vol: tighter spacing
            v if v < 1.0 => 0.15,   // Low vol: optimal spacing (winner!)
            v if v < 1.5 => 0.20,   // Normal vol: balanced
            v if v < 2.0 => 0.30,   // High vol: conservative
            v if v < 3.0 => 0.50,   // Very high vol: wide spacing
            _ => 0.75,              // Extreme vol: ultra wide
        }
    }
    
    /// Get optimal number of grid levels based on volatility
    pub fn get_optimal_levels(&self) -> u32 {
        // Based on GIGA test: 35 levels optimal at 0.15%
        // Scale with spacing
        match self.get_optimal_spacing() {
            s if s <= 0.10 => 40,
            s if s <= 0.15 => 35,  // Winner configuration
            s if s <= 0.20 => 25,
            s if s <= 0.30 => 15,
            s if s <= 0.50 => 10,
            _ => 8,
        }
    }
    
    /// Get market regime description
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
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_volatility_calculation() {
        let mut calc = VolatilityCalculator::new(100);
        
        // Add stable prices (low volatility)
        for i in 0..50 {
            calc.add_price(100.0 + (i as f64 * 0.01));
        }
        
        let vol = calc.get_volatility();
        assert!(vol < 0.1, "Low volatility should be detected");
        
        let spacing = calc.get_optimal_spacing();
        assert_eq!(spacing, 0.10, "Low vol should give tight spacing");
    }
    
    #[test]
    fn test_dynamic_spacing_adjustment() {
        let mut calc = VolatilityCalculator::new(100);
        
        // Simulate volatile market
        let mut price = 100.0;
        for _ in 0..50 {
            price += (rand::random::<f64>() - 0.5) * 5.0; // Random walk
            calc.add_price(price);
        }
        
        let spacing = calc.get_optimal_spacing();
        assert!(spacing >= 0.20, "High vol should give wider spacing");
    }
}
