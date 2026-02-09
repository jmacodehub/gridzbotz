//! ═══════════════════════════════════════════════════════════════════════════
//! ENHANCED METRICS TRACKER V1.0 - Comprehensive Trading Analytics
//! February 9, 2026
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
        self.signal_execution_ratio = if self.signal_count > 0 {
            (self.execution_count as f64 / self.signal_count as f64) * 100.0
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
    
    pub fn update_grid_stats(&mut self, total_levels: usize, used_levels: usize) {
        self.grid_levels_total = total_levels;
        self.grid_levels_used = used_levels;
        self.grid_efficiency = if total_levels > 0 {
            (used_levels as f64 / total_levels as f64) * 100.0
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
            self.avg_profit_per_trade = self.trade_pnls.iter().sum::<f64>() / self.trade_pnls.len() as f64;
        }
        
        // Trades per hour
        if self.trade_timestamps.len() >= 2 {
            let first = self.trade_timestamps.front().unwrap();
            let last = self.trade_timestamps.back().unwrap();
            let duration_hours = (*last - *first) as f64 / 3600.0;
            if duration_hours > 0.0 {
                self.trades_per_hour = self.trade_timestamps.len() as f64 / duration_hours;
            }
        }
    }
    
    pub fn display(&self) {
        println!("\n╔══════════════════════════════════════════════════════════╗");
        println!("║          📊 ENHANCED METRICS REPORT                      ║");
        println!("╚══════════════════════════════════════════════════════════╝");
        
        println!("\n🔢 TRADE-LEVEL METRICS:");
        println!("   Total Buys:           {}", self.total_buys);
        println!("   Total Sells:          {}", self.total_sells);
        println!("   Profitable Trades:    {}", self.profitable_trades);
        println!("   Unprofitable Trades:  {}", self.unprofitable_trades);
        println!("   Avg Profit/Trade:     ${:.4}", self.avg_profit_per_trade);
        println!("   Max Profit Trade:     ${:.4}", self.max_profit_trade);
        println!("   Max Loss Trade:       ${:.4}", self.max_loss_trade);
        println!("   Trades/Hour:          {:.2}", self.trades_per_hour);
        
        println!("\n⚠️  RISK METRICS:");
        println!("   Max Drawdown:         {:.2}%", self.max_drawdown);
        println!("   Current Drawdown:     {:.2}%", self.current_drawdown);
        println!("   Peak Portfolio Value: ${:.2}", self.peak_value);
        
        println!("\n⚡ EFFICIENCY METRICS:");
        println!("   Signal→Execution:     {:.1}%", self.signal_execution_ratio);
        println!("   Grid Efficiency:      {:.1}%", self.grid_efficiency);
        println!("   Grid Levels Used:     {}/{}", self.grid_levels_used, self.grid_levels_total);
        
        println!("\n📈 PRICE RANGE:");
        println!("   High:                 ${:.4}", self.price_high);
        println!("   Low:                  ${:.4}", self.price_low);
        println!("   Range:                ${:.4}", self.price_range);
        
        println!("\n🎯 COMPARISON METRICS:");
        println!("   ROI per Fee:          {:.2}x", self.roi_per_fee);
        println!("   ROI per Reposition:   {:.4}%", self.roi_per_reposition);
        println!("   Trades per ROI point: {:.2}", self.trades_per_roi);
    }
}
