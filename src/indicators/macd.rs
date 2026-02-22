//! 📉 MACD (Moving Average Convergence Divergence)
//! 
//! MACD is a trend-following momentum indicator that shows the relationship
//! between two moving averages of prices.
//! 
//! Components:
//! - MACD Line: 12-period EMA - 26-period EMA
//! - Signal Line: 9-period EMA of MACD Line
//! - Histogram: MACD Line - Signal Line
//! 
//! Signals:
//! - MACD crosses above Signal = BULLISH
//! - MACD crosses below Signal = BEARISH
//! - Histogram > 0 = Upward momentum
//! - Histogram < 0 = Downward momentum

use super::{Indicator, EMA};
use std::collections::VecDeque;

// period fields are used in construction and test assertions;
// macd_values is reserved for future streaming/buffering logic
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct MACD {
    fast_period: usize,
    slow_period: usize,
    signal_period: usize,
    
    fast_ema: EMA,
    slow_ema: EMA,
    signal_ema: EMA,
    
    macd_values: VecDeque<f64>,
    
    current_macd: Option<f64>,
    current_signal: Option<f64>,
    current_histogram: Option<f64>,
}

impl MACD {
    /// Create new MACD with standard parameters (12, 26, 9)
    pub fn new() -> Self {
        Self::with_periods(12, 26, 9)
    }
    
    /// Create MACD with custom periods
    pub fn with_periods(fast: usize, slow: usize, signal: usize) -> Self {
        Self {
            fast_period: fast,
            slow_period: slow,
            signal_period: signal,
            fast_ema: EMA::new(fast),
            slow_ema: EMA::new(slow),
            signal_ema: EMA::new(signal),
            macd_values: VecDeque::with_capacity(signal),
            current_macd: None,
            current_signal: None,
            current_histogram: None,
        }
    }
    
    /// Get MACD line value
    pub fn macd(&self) -> Option<f64> {
        self.current_macd
    }
    
    /// Get Signal line value
    pub fn signal(&self) -> Option<f64> {
        self.current_signal
    }
    
    /// Get Histogram value (MACD - Signal)
    pub fn histogram(&self) -> Option<f64> {
        self.current_histogram
    }
    
    /// Check if MACD crossed above Signal (BULLISH)
    pub fn is_bullish_crossover(&self, prev_macd: Option<f64>, prev_signal: Option<f64>) -> bool {
        if let (Some(macd), Some(signal), Some(p_macd), Some(p_signal)) = 
            (self.current_macd, self.current_signal, prev_macd, prev_signal) {
            p_macd <= p_signal && macd > signal
        } else {
            false
        }
    }
    
    /// Check if MACD crossed below Signal (BEARISH)
    pub fn is_bearish_crossover(&self, prev_macd: Option<f64>, prev_signal: Option<f64>) -> bool {
        if let (Some(macd), Some(signal), Some(p_macd), Some(p_signal)) = 
            (self.current_macd, self.current_signal, prev_macd, prev_signal) {
            p_macd >= p_signal && macd < signal
        } else {
            false
        }
    }
    
    /// Get trend strength (0.0 - 1.0) based on histogram
    pub fn trend_strength(&self) -> f64 {
        if let Some(histogram) = self.current_histogram {
            // Normalize to 0-1 range
            // Assume ±2.0 histogram = strong trend
            (histogram.abs() / 2.0).min(1.0)
        } else {
            0.0
        }
    }
}

impl Indicator for MACD {
    fn calculate(&mut self, price: f64) -> Option<f64> {
        // Calculate EMAs
        self.fast_ema.calculate(price);
        self.slow_ema.calculate(price);
        
        // Calculate MACD line
        if let (Some(fast), Some(slow)) = (self.fast_ema.value(), self.slow_ema.value()) {
            let macd = fast - slow;
            self.current_macd = Some(macd);
            
            // Calculate Signal line (EMA of MACD)
            if let Some(signal) = self.signal_ema.calculate(macd) {
                self.current_signal = Some(signal);
                
                // Calculate Histogram
                let histogram = macd - signal;
                self.current_histogram = Some(histogram);
                
                return Some(histogram);
            }
        }
        
        None
    }
    
    fn reset(&mut self) {
        self.fast_ema.reset();
        self.slow_ema.reset();
        self.signal_ema.reset();
        self.macd_values.clear();
        self.current_macd = None;
        self.current_signal = None;
        self.current_histogram = None;
    }
    
    fn name(&self) -> &str {
        "MACD"
    }
}

impl Default for MACD {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_macd_creation() {
        let macd = MACD::new();
        assert_eq!(macd.fast_period, 12);
        assert_eq!(macd.slow_period, 26);
        assert_eq!(macd.signal_period, 9);
    }
    
    #[test]
    fn test_macd_calculation() {
        let mut macd = MACD::new();
        
        // Feed enough prices for MACD calculation
        for i in 0..50 {
            let price = 100.0 + (i as f64 * 0.5);
            macd.calculate(price);
        }
        
        assert!(macd.macd().is_some());
        assert!(macd.signal().is_some());
        assert!(macd.histogram().is_some());
    }
}
