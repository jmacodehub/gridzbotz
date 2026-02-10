//! 📊 Technical Indicators Module
//! 
//! Reusable technical analysis indicators for all trading strategies.
//! Built with performance and accuracy in mind.

use std::collections::VecDeque;

// ═══════════════════════════════════════════════════════════════════════════
// INDICATOR TRAIT (BASE INTERFACE)
// ═══════════════════════════════════════════════════════════════════════════

/// Base trait for all technical indicators
pub trait Indicator {
    /// Calculate indicator with new price data
    fn calculate(&mut self, price: f64) -> Option<f64>;
    
    /// Reset indicator state
    fn reset(&mut self);
    
    /// Get indicator name
    fn name(&self) -> &str;
}

// ═══════════════════════════════════════════════════════════════════════════
// SUBMODULES
// ═══════════════════════════════════════════════════════════════════════════

pub mod atr;
pub mod ema;
pub mod sma;
pub mod macd;

// Re-export public types
pub use atr::ATR;
pub use ema::EMA;
pub use sma::SMA;
pub use macd::MACD;

// ═══════════════════════════════════════════════════════════════════════════
// HELPER FUNCTIONS
// ═══════════════════════════════════════════════════════════════════════════

/// Calculate percentile rank of a value in a distribution
/// 
/// Returns: 0.0 (lowest) to 1.0 (highest)
pub fn calculate_percentile(value: f64, distribution: &VecDeque<f64>) -> f64 {
    if distribution.is_empty() {
        return 0.5; // Neutral
    }
    
    let rank = distribution.iter()
        .filter(|&&v| v < value)
        .count();
    
    rank as f64 / distribution.len() as f64
}

// ═══════════════════════════════════════════════════════════════════════════
// MOVING AVERAGES (LEGACY FUNCTIONS - Keep for backward compatibility)
// ═══════════════════════════════════════════════════════════════════════════

/// Calculate Simple Moving Average (SMA)
/// 
/// # Example:
/// ```
/// let prices = vec![100.0, 101.0, 102.0, 103.0, 104.0];
/// let sma = calculate_sma(&prices);
/// assert_eq!(sma, Some(102.0)); // Average of all prices
/// ```
pub fn calculate_sma(prices: &[f64]) -> Option<f64> {
    if prices.is_empty() {
        return None;
    }
    
    let sum: f64 = prices.iter().sum();
    Some(sum / prices.len() as f64)
}

/// Calculate Exponential Moving Average (EMA)
/// 
/// EMA gives more weight to recent prices, making it more responsive.
/// 
/// Formula: EMA = (Price × Multiplier) + (Previous EMA × (1 - Multiplier))
/// Multiplier = 2 / (Period + 1)
/// 
/// # Example:
/// ```
/// let prices = vec![100.0, 101.0, 102.0, 103.0, 104.0];
/// let ema = calculate_ema(&prices, None, 5);
/// ```
pub fn calculate_ema(prices: &[f64], prev_ema: Option<f64>, period: usize) -> Option<f64> {
    if prices.is_empty() {
        return None;
    }
    
    let multiplier = 2.0 / (period as f64 + 1.0);
    
    if let Some(prev) = prev_ema {
        let latest_price = *prices.last().unwrap();
        Some((latest_price * multiplier) + (prev * (1.0 - multiplier)))
    } else {
        // First EMA: use SMA as starting point
        calculate_sma(prices)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// ATR (AVERAGE TRUE RANGE)
// ═══════════════════════════════════════════════════════════════════════════

/// Calculate True Range for a single period
/// 
/// TR = max(High - Low, |High - Previous Close|, |Low - Previous Close|)
pub fn calculate_true_range(high: f64, low: f64, prev_close: Option<f64>) -> f64 {
    let range1 = high - low;
    
    if let Some(prev) = prev_close {
        let range2 = (high - prev).abs();
        let range3 = (low - prev).abs();
        range1.max(range2).max(range3)
    } else {
        range1
    }
}

/// Calculate Average True Range (ATR)
/// 
/// ATR measures volatility by averaging True Range over a period.
/// Higher ATR = Higher volatility
/// 
/// # Arguments:
/// * `highs` - High prices
/// * `lows` - Low prices
/// * `closes` - Close prices
/// * `period` - ATR period (typically 14)
pub fn calculate_atr(
    highs: &[f64],
    lows: &[f64],
    closes: &[f64],
    period: usize,
) -> Option<f64> {
    if highs.len() < period || lows.len() < period || closes.len() < period {
        return None;
    }
    
    let mut true_ranges = Vec::with_capacity(period);
    
    // Calculate True Range for each period
    for i in 1..=period {
        let tr = calculate_true_range(
            highs[i],
            lows[i],
            Some(closes[i - 1]),
        );
        true_ranges.push(tr);
    }
    
    // ATR is the SMA of True Ranges
    calculate_sma(&true_ranges)
}

/// Calculate ATR Percentile
/// 
/// Returns where current ATR ranks in historical distribution (0.0 - 1.0).
/// Used for regime detection and adaptive grid spacing.
/// 
/// # Example:
/// ```
/// let current_atr = 2.5;
/// let historical_atr = vec![1.0, 1.5, 2.0, 2.2, 2.8, 3.0];
/// let percentile = calculate_atr_percentile(current_atr, &historical_atr);
/// // Returns: 0.67 (current ATR is higher than 67% of historical values)
/// ```
pub fn calculate_atr_percentile(current_atr: f64, historical_atr: &[f64]) -> f64 {
    if historical_atr.is_empty() {
        return 0.5; // Default to neutral
    }
    
    let rank = historical_atr.iter()
        .filter(|&&atr| atr < current_atr)
        .count();
    
    rank as f64 / historical_atr.len() as f64
}

// ═══════════════════════════════════════════════════════════════════════════
// MACD (MOVING AVERAGE CONVERGENCE DIVERGENCE)
// ═══════════════════════════════════════════════════════════════════════════

/// MACD Indicator values
#[derive(Debug, Clone, Copy)]
pub struct MACDValues {
    /// MACD line (Fast EMA - Slow EMA)
    pub macd_line: f64,
    
    /// Signal line (EMA of MACD line)
    pub signal_line: f64,
    
    /// Histogram (MACD - Signal)
    pub histogram: f64,
}

/// MACD State for incremental calculation
pub struct MACDState {
    fast_ema: Option<f64>,
    slow_ema: Option<f64>,
    signal_ema: Option<f64>,
    fast_period: usize,
    slow_period: usize,
    signal_period: usize,
}

impl MACDState {
    /// Create new MACD state with standard periods (12, 26, 9)
    pub fn new() -> Self {
        Self::with_periods(12, 26, 9)
    }
    
    /// Create MACD state with custom periods
    pub fn with_periods(fast: usize, slow: usize, signal: usize) -> Self {
        Self {
            fast_ema: None,
            slow_ema: None,
            signal_ema: None,
            fast_period: fast,
            slow_period: slow,
            signal_period: signal,
        }
    }
    
    /// Update MACD with new price
    /// 
    /// Returns MACD values if enough data has been accumulated
    pub fn update(&mut self, price: f64) -> Option<MACDValues> {
        // Calculate Fast EMA (12-period)
        self.fast_ema = Some(if let Some(prev) = self.fast_ema {
            let multiplier = 2.0 / (self.fast_period as f64 + 1.0);
            (price * multiplier) + (prev * (1.0 - multiplier))
        } else {
            price // First value
        });
        
        // Calculate Slow EMA (26-period)
        self.slow_ema = Some(if let Some(prev) = self.slow_ema {
            let multiplier = 2.0 / (self.slow_period as f64 + 1.0);
            (price * multiplier) + (prev * (1.0 - multiplier))
        } else {
            price // First value
        });
        
        // MACD Line = Fast EMA - Slow EMA
        let macd_line = self.fast_ema.unwrap() - self.slow_ema.unwrap();
        
        // Signal Line = EMA of MACD Line (9-period)
        self.signal_ema = Some(if let Some(prev) = self.signal_ema {
            let multiplier = 2.0 / (self.signal_period as f64 + 1.0);
            (macd_line * multiplier) + (prev * (1.0 - multiplier))
        } else {
            macd_line // First value
        });
        
        let signal_line = self.signal_ema.unwrap();
        
        // Histogram = MACD - Signal
        let histogram = macd_line - signal_line;
        
        Some(MACDValues {
            macd_line,
            signal_line,
            histogram,
        })
    }
    
    /// Reset MACD state
    pub fn reset(&mut self) {
        self.fast_ema = None;
        self.slow_ema = None;
        self.signal_ema = None;
    }
}

impl Default for MACDState {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// RSI HELPERS
// ═══════════════════════════════════════════════════════════════════════════

/// Calculate RSI from average gains and losses
/// 
/// Formula: RSI = 100 - (100 / (1 + RS))
/// Where RS = Average Gain / Average Loss
pub fn calculate_rsi_from_averages(avg_gain: f64, avg_loss: f64) -> f64 {
    if avg_loss == 0.0 {
        return 100.0; // Maximum RSI (no losses)
    }
    
    let rs = avg_gain / avg_loss;
    100.0 - (100.0 / (1.0 + rs))
}

/// Detect RSI divergence
/// 
/// Divergence occurs when price and RSI move in opposite directions.
/// This often signals a trend reversal.
/// 
/// # Types:
/// - **Bullish Divergence**: Price makes lower lows, RSI makes higher lows
/// - **Bearish Divergence**: Price makes higher highs, RSI makes lower highs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RSIDivergence {
    /// Bullish divergence (potential bottom)
    Bullish,
    
    /// Bearish divergence (potential top)
    Bearish,
    
    /// No divergence detected
    None,
}

/// Detect RSI divergence
/// 
/// # Arguments:
/// * `prices` - Recent price history (need at least 3 points)
/// * `rsi_values` - Corresponding RSI values
/// * `lookback` - How many periods to check (typically 3-5)
pub fn detect_rsi_divergence(
    prices: &[f64],
    rsi_values: &[f64],
    lookback: usize,
) -> RSIDivergence {
    if prices.len() < lookback || rsi_values.len() < lookback {
        return RSIDivergence::None;
    }
    
    let recent_prices = &prices[prices.len() - lookback..];
    let recent_rsi = &rsi_values[rsi_values.len() - lookback..];
    
    // Check for bullish divergence (price lower low, RSI higher low)
    let price_min_idx = recent_prices.iter()
        .enumerate()
        .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(idx, _)| idx)
        .unwrap();
    
    let rsi_min_idx = recent_rsi.iter()
        .enumerate()
        .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(idx, _)| idx)
        .unwrap();
    
    // Bullish: Latest price < old price, but latest RSI > old RSI
    if price_min_idx == recent_prices.len() - 1 && rsi_min_idx == 0 {
        return RSIDivergence::Bullish;
    }
    
    // Check for bearish divergence (price higher high, RSI lower high)
    let price_max_idx = recent_prices.iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(idx, _)| idx)
        .unwrap();
    
    let rsi_max_idx = recent_rsi.iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(idx, _)| idx)
        .unwrap();
    
    // Bearish: Latest price > old price, but latest RSI < old RSI
    if price_max_idx == recent_prices.len() - 1 && rsi_max_idx == 0 {
        return RSIDivergence::Bearish;
    }
    
    RSIDivergence::None
}

// ═══════════════════════════════════════════════════════════════════════════
// BOLLINGER BANDS
// ═══════════════════════════════════════════════════════════════════════════

/// Bollinger Bands values
#[derive(Debug, Clone, Copy)]
pub struct BollingerBands {
    /// Middle band (SMA)
    pub middle: f64,
    
    /// Upper band (SMA + 2 * StdDev)
    pub upper: f64,
    
    /// Lower band (SMA - 2 * StdDev)
    pub lower: f64,
    
    /// Bandwidth (Upper - Lower)
    pub bandwidth: f64,
}

/// Calculate Bollinger Bands
/// 
/// # Arguments:
/// * `prices` - Price history
/// * `period` - SMA period (typically 20)
/// * `std_dev_multiplier` - Standard deviation multiplier (typically 2.0)
pub fn calculate_bollinger_bands(
    prices: &[f64],
    period: usize,
    std_dev_multiplier: f64,
) -> Option<BollingerBands> {
    if prices.len() < period {
        return None;
    }
    
    let recent_prices = &prices[prices.len() - period..];
    
    // Middle band = SMA
    let middle = calculate_sma(recent_prices)?;
    
    // Calculate standard deviation
    let variance: f64 = recent_prices.iter()
        .map(|&price| {
            let diff = price - middle;
            diff * diff
        })
        .sum::<f64>() / period as f64;
    
    let std_dev = variance.sqrt();
    
    // Upper and lower bands
    let upper = middle + (std_dev_multiplier * std_dev);
    let lower = middle - (std_dev_multiplier * std_dev);
    let bandwidth = upper - lower;
    
    Some(BollingerBands {
        middle,
        upper,
        lower,
        bandwidth,
    })
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sma() {
        let prices = vec![100.0, 102.0, 104.0, 106.0, 108.0];
        let sma = calculate_sma(&prices).unwrap();
        assert_eq!(sma, 104.0);
    }
    
    #[test]
    fn test_ema() {
        let prices = vec![100.0, 101.0, 102.0, 103.0, 104.0];
        let ema = calculate_ema(&prices, None, 5).unwrap();
        assert!(ema > 101.0 && ema < 104.0); // Should be weighted toward recent prices
    }
    
    #[test]
    fn test_atr_percentile() {
        let historical = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let percentile = calculate_atr_percentile(3.5, &historical);
        assert!(percentile > 0.5 && percentile < 0.8);
    }
    
    #[test]
    fn test_macd() {
        let mut macd = MACDState::new();
        
        // Feed prices
        for price in [100.0, 101.0, 102.0, 103.0, 104.0].iter() {
            let _ = macd.update(*price);
        }
        
        let result = macd.update(105.0).unwrap();
        
        // MACD should be positive in uptrend
        assert!(result.macd_line > 0.0);
    }
    
    #[test]
    fn test_bollinger_bands() {
        let prices = vec![
            100.0, 101.0, 99.0, 102.0, 98.0,
            103.0, 97.0, 104.0, 96.0, 105.0,
            100.0, 101.0, 99.0, 102.0, 98.0,
            103.0, 97.0, 104.0, 96.0, 105.0,
        ];
        
        let bands = calculate_bollinger_bands(&prices, 20, 2.0).unwrap();
        
        assert!(bands.upper > bands.middle);
        assert!(bands.lower < bands.middle);
        assert!(bands.bandwidth > 0.0);
    }
}
