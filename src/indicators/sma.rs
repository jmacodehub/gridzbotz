//! 📊 Simple Moving Average (SMA)
//! 
//! SMA is the arithmetic mean of prices over a specified period.
//! 
//! Formula: SMA = Sum of Prices / Number of Periods

use super::Indicator;
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct SMA {
    period: usize,
    prices: VecDeque<f64>,
}

impl SMA {
    /// Create new SMA with specified period
    pub fn new(period: usize) -> Self {
        Self {
            period,
            prices: VecDeque::with_capacity(period),
        }
    }
    
    /// Get current SMA value
    pub fn value(&self) -> Option<f64> {
        if self.prices.len() < self.period {
            return None;
        }
        
        Some(self.prices.iter().sum::<f64>() / self.prices.len() as f64)
    }
}

impl Indicator for SMA {
    fn calculate(&mut self, price: f64) -> Option<f64> {
        self.prices.push_back(price);
        
        // Keep only required prices
        if self.prices.len() > self.period {
            self.prices.pop_front();
        }
        
        self.value()
    }
    
    fn reset(&mut self) {
        self.prices.clear();
    }
    
    fn name(&self) -> &str {
        "SMA"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sma_calculation() {
        let mut sma = SMA::new(5);
        
        let prices = vec![100.0, 102.0, 104.0, 103.0, 105.0];
        
        for price in &prices {
            sma.calculate(*price);
        }
        
        let expected = prices.iter().sum::<f64>() / prices.len() as f64;
        assert_eq!(sma.value().unwrap(), expected);
    }
}
