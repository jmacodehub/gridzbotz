//! ═══════════════════════════════════════════════════════════════════════════
//! ENHANCED METRICS TRACKER V1.1 - Comprehensive Trading Analytics
//!
//! PR #92 CHANGES:
//! [fix] P1: record_signal() clamps signal_execution_ratio to 100.0 max.
//!    Root cause: record_trade() also increments execution_count, so on fills
//!    execution_count marginally exceeds signal_count -> ratio > 100%.
//!    Fix: (ratio * 100.0).min(100.0) - belt-and-suspenders clamp at source.
//! [fix] P1: display() guards trades_per_hour - prints "- (< 2 fills)" instead
//!    of "0.00" when fewer than 2 timestamps exist (single fill = no delta).
//!
//! February 9, 2026  - V1.0 initial
//! March   10, 2026  - V1.1 PR #92 clamp + display guard
//! ═══════════════════════════════════════════════════════════════════════════

use serde::{Serialize, Deserialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EnhancedMetrics {
    // Trade-Level Metrics
    pub total_buys: usize,
    pub total_sells: usize,
    pub profitable_trades: usize,
    pub unprofitable_trades: usize,
    pub avg_profit_per_trade: f64,
    pub max_profit_trade: f64,
    pub max_loss_trade: f64,
    pub trades_per_hour: f64,

    // Risk Metrics
    pub max_drawdown: f64,
    pub current_drawdown: f64,
    pub peak_value: f64,
    pub sharpe_ratio: f64,
    pub volatility_captured: f64,
    pub volatility_missed: f64,

    // Efficiency Metrics
    pub signal_count: usize,
    pub execution_count: usize,
    pub signal_execution_ratio: f64,
    pub grid_levels_total: usize,
    pub grid_levels_used: usize,
    /// Fraction 0.0-1.0 (used_levels / total_levels).
    /// AdaptiveOptimizer thresholds are 0.70 / 0.30 - keep as fraction.
    pub grid_efficiency: f64,
    pub opportunity_capture_rate: f64,

    // Price Range Metrics
    pub price_high: f64,
    pub price_low: f64,
    pub price_range: f64,
    pub price_range_utilized: f64,

    // Comparison Metrics
    pub roi_per_fee: f64,
    pub roi_per_reposition: f64,
    pub trades_per_roi: f64,

    // Internal tracking
    trade_pnls: VecDeque<f64>,
    trade_timestamps: VecDeque<i64>,
    value_history: VecDeque<f64>,
}

impl EnhancedMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_trade(&mut self, is_buy: bool, pnl: f64, timestamp: i64) {
        if is_buy {
            self.total_buys += 1;
        } else {
            self.total_sells += 1;
            // Only count P&L on sells (completing a round trip)
            if pnl > 0.0 {
                self.profitable_trades += 1;
            } else {
                self.unprofitable_trades += 1;
            }

            self.trade_pnls.push_back(pnl);
            self.max_profit_trade = self.max_profit_trade.max(pnl);
            self.max_loss_trade = self.max_loss_trade.min(pnl);
        }

        self.trade_timestamps.push_back(timestamp);
        self.execution_count += 1;

        // Keep last 1000 trades
        if self.trade_pnls.len() > 1000 {
            self.trade_pnls.pop_front();
        }
        if self.trade_timestamps.len() > 1000 {
            self.trade_timestamps.pop_front();
        }

        self.recalculate_averages();
    }

    pub fn update_portfolio_value(&mut self, value: f64) {
        self.value_history.push_back(value);

        // Update peak and drawdown
        if value > self.peak_value {
            self.peak_value = value;
            self.current_drawdown = 0.0;
        } else {
            self.current_drawdown = ((self.peak_value - value) / self.peak_value) * 100.0;
            self.max_drawdown = self.max_drawdown.max(self.current_drawdown);
        }

        // Keep last 10000 values
        if self.value_history.len() > 10000 {
            self.value_history.pop_front();
        }
    }

    pub fn record_signal(&mut self, executed: bool) {
        self.signal_count += 1;
        if executed {
            self.execution_count += 1;
        }
        // PR #92 P1: Clamp to 100.0 max.
        // record_trade() also increments execution_count on the same cycle
        // as record_signal(true) for fill ticks, causing execution_count to
        // marginally exceed signal_count and the ratio to read as 100.1%.
        // Belt-and-suspenders clamp here at the source.
        self.signal_execution_ratio = if self.signal_count > 0 {
            ((self.execution_count as f64 / self.signal_count as f64) * 100.0).min(100.0)
        } else {
            0.0
        };
    }

    pub fn update_price_range(&mut self, price: f64) {
        if self.price_high == 0.0 {
            self.price_high = price;
            self.price_low = price;
        } else {
            self.price_high = self.price_high.max(price);
            self.price_low = self.price_low.min(price);
        }
        self.price_range = self.price_high - self.price_low;
    }

    /// Store grid_efficiency as a 0.0-1.0 fraction so that AdaptiveOptimizer
    /// (which uses thresholds of 0.70 / 0.30) reads it correctly.
    /// Multiply by 100 only when displaying to humans.
    pub fn update_grid_stats(&mut self, total_levels: usize, used_levels: usize) {
        self.grid_levels_total = total_levels;
        self.grid_levels_used = used_levels;
        self.grid_efficiency = if total_levels > 0 {
            used_levels as f64 / total_levels as f64
        } else {
            0.0
        };
    }

    pub fn calculate_comparison_metrics(&mut self, roi: f64, total_fees: f64, repositions: u64) {
        self.roi_per_fee = if total_fees > 0.0 {
            roi / total_fees
        } else {
            0.0
        };

        self.roi_per_reposition = if repositions > 0 {
            roi / repositions as f64
        } else {
            roi
        };

        let total_trades = self.total_buys + self.total_sells;
        self.trades_per_roi = if roi != 0.0 {
            total_trades as f64 / roi.abs()
        } else {
            0.0
        };
    }

    fn recalculate_averages(&mut self) {
        // Avg profit per trade
        if !self.trade_pnls.is_empty() {
            self.avg_profit_per_trade =
                self.trade_pnls.iter().sum::<f64>() / self.trade_pnls.len() as f64;
        }

        // Trades per hour: requires at least 2 timestamps for a meaningful delta.
        // With only 1 fill the VecDeque has 1 entry - no elapsed time to divide by.
        // trades_per_hour stays 0.0 in that case; display() guards the output.
        if self.trade_timestamps.len() >= 2 {
            let first = self.trade_timestamps.front().unwrap();
            let last  = self.trade_timestamps.back().unwrap();
            let duration_hours = (*last - *first) as f64 / 3600.0;
            if duration_hours > 0.0 {
                self.trades_per_hour =
                    self.trade_timestamps.len() as f64 / duration_hours;
            }
        }
    }

    pub fn display(&self) {
        println!("\n╬══════════════════════════════════════════════════════════╬");
        println!("║          [METRICS] ENHANCED METRICS REPORT              ║");
        println!("╚══════════════════════════════════════════════════════════╝");

        println!("\n[TRADE] TRADE-LEVEL METRICS:");
        println!("   Total Buys:           {}", self.total_buys);
        println!("   Total Sells:          {}", self.total_sells);
        println!("   Profitable Trades:    {}", self.profitable_trades);
        println!("   Unprofitable Trades:  {}", self.unprofitable_trades);
        println!("   Avg Profit/Trade:     ${:.4}", self.avg_profit_per_trade);
        println!("   Max Profit Trade:     ${:.4}", self.max_profit_trade);
        println!("   Max Loss Trade:       ${:.4}", self.max_loss_trade);
        // PR #92 P1: Guard trades_per_hour display.
        // With only 1 fill, trade_timestamps has 1 entry - no time delta exists.
        // Printing 0.00 is misleading; make the data gap explicit instead.
        if self.trade_timestamps.len() < 2 {
            println!("   Trades/Hour:          - (< 2 fills)");
        } else {
            println!("   Trades/Hour:          {:.2}", self.trades_per_hour);
        }

        println!("\n[RISK] RISK METRICS:");
        println!("   Max Drawdown:         {:.2}%", self.max_drawdown);
        println!("   Current Drawdown:     {:.2}%", self.current_drawdown);
        println!("   Peak Portfolio Value: ${:.2}", self.peak_value);

        println!("\n[EFF] EFFICIENCY METRICS:");
        // signal_execution_ratio is already clamped to 100.0 at record_signal().
        println!("   Signal->Execution:    {:.1}%", self.signal_execution_ratio);
        // grid_efficiency is stored as 0.0-1.0; multiply by 100 for human display
        println!("   Grid Efficiency:      {:.1}%", self.grid_efficiency * 100.0);
        println!("   Grid Levels Used:     {}/{}", self.grid_levels_used, self.grid_levels_total);

        println!("\n[PRICE] PRICE RANGE:");
        println!("   High:                 ${:.4}", self.price_high);
        println!("   Low:                  ${:.4}", self.price_low);
        println!("   Range:                ${:.4}", self.price_range);

        println!("\n[CMP] COMPARISON METRICS:");
        println!("   ROI per Fee:          {:.2}x", self.roi_per_fee);
        println!("   ROI per Reposition:   {:.4}%", self.roi_per_reposition);
        println!("   Trades per ROI point: {:.2}", self.trades_per_roi);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_trade_increments_sides() {
        let mut m = EnhancedMetrics::new();
        m.record_trade(true,  0.0, 1000);
        m.record_trade(false, 5.0, 2000);
        assert_eq!(m.total_buys,  1);
        assert_eq!(m.total_sells, 1);
        assert_eq!(m.profitable_trades, 1);
        assert_eq!(m.unprofitable_trades, 0);
    }

    #[test]
    fn test_record_trade_loss() {
        let mut m = EnhancedMetrics::new();
        m.record_trade(false, -2.0, 1000);
        assert_eq!(m.profitable_trades, 0);
        assert_eq!(m.unprofitable_trades, 1);
    }

    #[test]
    fn test_update_grid_stats_fraction() {
        let mut m = EnhancedMetrics::new();
        m.update_grid_stats(10, 7);
        assert!(
            (m.grid_efficiency - 0.7).abs() < 1e-9,
            "grid_efficiency must be 0.0-1.0 fraction, got {}",
            m.grid_efficiency
        );
    }

    #[test]
    fn test_drawdown_tracking() {
        let mut m = EnhancedMetrics::new();
        m.update_portfolio_value(1000.0);
        m.update_portfolio_value(900.0);
        assert!((m.current_drawdown - 10.0).abs() < 1e-6);
        assert!((m.max_drawdown    - 10.0).abs() < 1e-6);
    }

    /// PR #92 P1: signal_execution_ratio must never exceed 100.0.
    #[test]
    fn test_signal_execution_ratio_clamped_to_100() {
        let mut m = EnhancedMetrics::new();
        // Each fill tick bumps execution_count twice (record_signal + record_trade).
        // Without clamp: execution=6, signal=3 -> 200%. With clamp: must be <= 100.0.
        for i in 0..3_i64 {
            m.record_signal(true);
            m.record_trade(i % 2 == 0, 1.0, 1000 + i);
        }
        assert!(
            m.signal_execution_ratio <= 100.0,
            "signal_execution_ratio {} exceeded 100.0",
            m.signal_execution_ratio
        );
    }

    /// PR #92 P1: trades_per_hour stays 0.0 when fewer than 2 fills.
    #[test]
    fn test_trades_per_hour_zero_on_single_fill() {
        let mut m = EnhancedMetrics::new();
        m.record_trade(false, 1.0, 1000);
        assert_eq!(
            m.trades_per_hour, 0.0,
            "trades_per_hour should be 0.0 with only 1 fill, got {}",
            m.trades_per_hour
        );
    }

    /// PR #92 P1: trades_per_hour is positive when 2+ fills exist.
    #[test]
    fn test_trades_per_hour_positive_on_multiple_fills() {
        let mut m = EnhancedMetrics::new();
        m.record_trade(false, 1.0, 0);
        m.record_trade(false, 1.0, 3600);
        assert!(
            m.trades_per_hour > 0.0,
            "trades_per_hour should be positive with 2 fills, got {}",
            m.trades_per_hour
        );
    }
}
