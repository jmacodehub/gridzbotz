//! 📊 ATR (Average True Range)
//! 
//! ATR measures market volatility by calculating the average range between
//! high and low prices over a specified period.
//! 
//! ATR Percentile: Normalized ATR ranking for adaptive strategies.
//! Used for dynamic grid spacing and regime detection.
//! 
//! Formula: ATR = Moving Average of True Range over N periods
//! True Range = max(High - Low, |High - Close_prev|, |Low - Close_prev|)

use super::{Indicator, calculate_percentile};
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct ATR {
    period: usize,
    lookback_window: usize,
    
    true_ranges: VecDeque<f64>,
    historical_atr: VecDeque<f64>,
    
    prev_close: Option<f64>,
    current_atr: Option<f64>,
    current_percentile: Option<f64>,
}

impl ATR {
    /// Create new ATR with standard period (14) and lookback (150)
    pub fn new() -> Self {
        Self::with_config(14, 150)
    }
    
    /// Create ATR with custom configuration
    pub fn with_config(period: usize, lookback: usize) -> Self {
        Self {
            period,
            lookback_window: lookback,
            true_ranges: VecDeque::with_capacity(period),
            historical_atr: VecDeque::with_capacity(lookback),
            prev_close: None,
            current_atr: None,
            current_percentile: None,
        }
    }
    
    /// Calculate True Range for current bar
    /// 
    /// TR = max(High - Low, |High - Close_prev|, |Low - Close_prev|)
    /// For crypto (no separate high/low): use price range
    pub fn calculate_true_range(&self, high: f64, low: f64) -> f64 {
        if let Some(prev_close) = self.prev_close {
            let hl = high - low;
            let hc = (high - prev_close).abs();
            let lc = (low - prev_close).abs();
            
            hl.max(hc).max(lc)
        } else {
            high - low
        }
    }
    
    /// Get current ATR value
    pub fn value(&self) -> Option<f64> {
        self.current_atr
    }
    
    /// Get ATR percentile rank (0.0 - 1.0)
    /// 
    /// Used for adaptive strategies:
    /// - High percentile (>0.65) = High volatility
    /// - Low percentile (<0.35) = Low volatility
    pub fn percentile(&self) -> Option<f64> {
        self.current_percentile
    }
    
    /// Calculate ATR using Simple Moving Average of True Ranges
    fn calculate_atr(&self) -> Option<f64> {
        if self.true_ranges.len() < self.period {
            return None;
        }
        
        let sum: f64 = self.true_ranges.iter().sum();
        Some(sum / self.true_ranges.len() as f64)
    }
    
    /// Update with OHLC data (preferred for accurate ATR)
    pub fn update_ohlc(&mut self, high: f64, low: f64, close: f64) -> Option<f64> {
        let tr = self.calculate_true_range(high, low);
        
        self.true_ranges.push_back(tr);
        if self.true_ranges.len() > self.period {
            self.true_ranges.pop_front();
        }
        
        // Calculate ATR
        if let Some(atr) = self.calculate_atr() {
            self.current_atr = Some(atr);
            
            // Update historical ATR for percentile calculation
            self.historical_atr.push_back(atr);
            if self.historical_atr.len() > self.lookback_window {
                self.historical_atr.pop_front();
            }
            
            // Calculate percentile
            self.current_percentile = Some(calculate_percentile(atr, &self.historical_atr));
            
            self.prev_close = Some(close);
            return Some(atr);
        }
        
        self.prev_close = Some(close);
        None
    }
}

impl Indicator for ATR {
    /// Calculate ATR using price only (simplified for single price feed)
    /// For accurate ATR, use update_ohlc() instead
    fn calculate(&mut self, price: f64) -> Option<f64> {
        // Simplified: use price change as proxy for true range
        if let Some(prev) = self.prev_close {
            let tr = (price - prev).abs();
            
            self.true_ranges.push_back(tr);
            if self.true_ranges.len() > self.period {
                self.true_ranges.pop_front();
            }
            
            if let Some(atr) = self.calculate_atr() {
                self.current_atr = Some(atr);
                
                self.historical_atr.push_back(atr);
                if self.historical_atr.len() > self.lookback_window {
                    self.historical_atr.pop_front();
                }
                
                self.current_percentile = Some(calculate_percentile(atr, &self.historical_atr));
                
                self.prev_close = Some(price);
                return Some(atr);
            }
        }
        
        self.prev_close = Some(price);
        None
    }
    
    fn reset(&mut self) {
        self.true_ranges.clear();
        self.historical_atr.clear();
        self.prev_close = None;
        self.current_atr = None;
        self.current_percentile = None;
    }
    
    fn name(&self) -> &str {
        "ATR"
    }
}

impl Default for ATR {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_atr_creation() {
        let atr = ATR::new();
        assert_eq!(atr.period, 14);
        assert_eq!(atr.lookback_window, 150);
    }
    
    #[test]
    fn test_atr_calculation() {
        let mut atr = ATR::new();
        
        // Feed prices with volatility
        for i in 0..30 {
            let price = 100.0 + (i as f64 * 0.5) + (i % 3) as f64;
            atr.calculate(price);
        }
        
        assert!(atr.value().is_some());
        assert!(atr.value().unwrap() > 0.0);
    }
    
    #[test]
    fn test_atr_percentile() {
        let mut atr = ATR::new();
        
        // Feed enough data for percentile
        for i in 0..200 {
            let price = 100.0 + (i as f64 * 0.1);
            atr.calculate(price);
        }
        
        assert!(atr.percentile().is_some());
        let percentile = atr.percentile().unwrap();
        assert!(percentile >= 0.0 && percentile <= 1.0);
    }
}
