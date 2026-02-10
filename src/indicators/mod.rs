//! 📊 Technical Indicators Module
//! 
//! Reusable technical analysis indicators for all trading strategies.
//! Built with 2026 research-backed implementations.

pub mod atr;
pub mod ema;
pub mod macd;
pub mod sma;

pub use atr::ATR;
pub use ema::EMA;
pub use macd::MACD;
pub use sma::SMA;

use std::collections::VecDeque;

/// Common trait for all indicators
pub trait Indicator {
    /// Calculate indicator value from price data
    fn calculate(&mut self, price: f64) -> Option<f64>;
    
    /// Reset indicator state
    fn reset(&mut self);
    
    /// Get indicator name
    fn name(&self) -> &str;
}

/// Helper: Calculate percentile rank
/// Used for ATR percentile and other normalized indicators
pub fn calculate_percentile(value: f64, historical: &VecDeque<f64>) -> f64 {
    if historical.is_empty() {
        return 0.5;
    }
    
    let rank = historical.iter()
        .filter(|&&v| v < value)
        .count();
    
    rank as f64 / historical.len() as f64
}

/// Helper: Calculate standard deviation
pub fn calculate_std_dev(values: &VecDeque<f64>) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    
    let mean: f64 = values.iter().sum::<f64>() / values.len() as f64;
    let variance: f64 = values.iter()
        .map(|v| (v - mean).powi(2))
        .sum::<f64>() / values.len() as f64;
    
    variance.sqrt()
}
