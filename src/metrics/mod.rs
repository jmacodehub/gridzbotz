//! ═══════════════════════════════════════════════════════════════════════════
//! Advanced Metrics & Analytics System
//! Tracks performance, risk, and efficiency metrics
//! ═══════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub win_rate: f64,
    pub total_pnl: f64,
    pub max_drawdown: f64,
    pub sharpe_ratio: f64,
    pub profit_factor: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub largest_win: f64,
    pub largest_loss: f64,
    pub total_fees: f64,
    pub net_profit: f64,
}

impl PerformanceMetrics {
    pub fn calculate(
        total_trades: usize,
        wins: usize,
        total_pnl: f64,
        max_dd: f64,
        total_fees: f64,
    ) -> Self {
        let losses = total_trades.saturating_sub(wins);
        let win_rate = if total_trades > 0 {
            (wins as f64 / total_trades as f64) * 100.0
        } else {
            0.0
        };
        
        let sharpe = if max_dd > 0.0 {
            total_pnl / max_dd
        } else {
            0.0
        };
        
        let profit_factor = if losses > 0 {
            wins as f64 / losses as f64
        } else {
            wins as f64
        };
        
        Self {
            total_trades,
            winning_trades: wins,
            losing_trades: losses,
            win_rate,
            total_pnl,
            max_drawdown: max_dd,
            sharpe_ratio: sharpe,
            profit_factor,
            avg_win: if wins > 0 { total_pnl / wins as f64 } else { 0.0 },
            avg_loss: 0.0,
            largest_win: 0.0,
            largest_loss: 0.0,
            total_fees,
            net_profit: total_pnl - total_fees,
        }
    }
}

#[derive(Debug)]
pub struct PriceTracker {
    prices: VecDeque<f64>,
    max_size: usize,
}

impl PriceTracker {
    pub fn new(max_size: usize) -> Self {
        Self {
            prices: VecDeque::with_capacity(max_size),
            max_size,
        }
    }
    
    pub fn add(&mut self, price: f64) {
        if self.prices.len() >= self.max_size {
            self.prices.pop_front();
        }
        self.prices.push_back(price);
    }
    
    pub fn volatility(&self) -> f64 {
        if self.prices.len() < 2 {
            return 0.0;
        }
        
        let mean = self.prices.iter().sum::<f64>() / self.prices.len() as f64;
        let variance = self.prices.iter()
            .map(|p| (p - mean).powi(2))
            .sum::<f64>() / self.prices.len() as f64;
        
        variance.sqrt()
    }
    
    pub fn range(&self) -> (f64, f64) {
        if self.prices.is_empty() {
            return (0.0, 0.0);
        }
        
        let min = self.prices.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = self.prices.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        
        (min, max)
    }
}
