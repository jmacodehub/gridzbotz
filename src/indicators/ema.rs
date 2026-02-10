//! 📈 Exponential Moving Average (EMA)
//! 
//! EMA gives more weight to recent prices, making it more responsive
//! to price changes than Simple Moving Average.
//! 
//! Formula: EMA = (Price × Multiplier) + (Previous EMA × (1 - Multiplier))
//! Multiplier = 2 / (Period + 1)

use super::Indicator;
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct EMA {
    period: usize,
    prices: VecDeque<f64>,
    current_ema: Option<f64>,
}

impl EMA {
    /// Create new EMA with specified period
    pub fn new(period: usize) -> Self {
        Self {
            period,
            prices: VecDeque::with_capacity(period),
            current_ema: None,
        }
    }
    
    /// Get current EMA value
    pub fn value(&self) -> Option<f64> {
        self.current_ema
    }
    
    /// Calculate multiplier for EMA
    fn multiplier(&self) -> f64 {
        2.0 / (self.period as f64 + 1.0)
    }
    
    /// Calculate Simple Moving Average (used as initial EMA)
    fn calculate_sma(&self) -> Option<f64> {
        if self.prices.is_empty() {
            return None;
        }
        Some(self.prices.iter().sum::<f64>() / self.prices.len() as f64)
    }
}

impl Indicator for EMA {
    fn calculate(&mut self, price: f64) -> Option<f64> {
        self.prices.push_back(price);
        
        // Keep only required prices
        if self.prices.len() > self.period {
            self.prices.pop_front();
        }
        
        // Need full period for calculation
        if self.prices.len() < self.period {
            return None;
        }
        
        let multiplier = self.multiplier();
        
        // First EMA: use SMA as starting point
        if self.current_ema.is_none() {
            self.current_ema = self.calculate_sma();
            return self.current_ema;
        }
        
        // Subsequent EMAs: use formula
        if let Some(prev_ema) = self.current_ema {
            let ema = (price * multiplier) + (prev_ema * (1.0 - multiplier));
            self.current_ema = Some(ema);
            Some(ema)
        } else {
            None
        }
    }
    
    fn reset(&mut self) {
        self.prices.clear();
        self.current_ema = None;
    }
    
    fn name(&self) -> &str {
        "EMA"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ema_calculation() {
        let mut ema = EMA::new(5);
        
        let prices = vec![100.0, 102.0, 104.0, 103.0, 105.0, 107.0];
        
        for price in prices {
            ema.calculate(price);
        }
        
        assert!(ema.value().is_some());
        assert!(ema.value().unwrap() > 100.0);
    }
}
