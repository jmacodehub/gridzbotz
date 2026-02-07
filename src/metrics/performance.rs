//! ═══════════════════════════════════════════════════════════════════════════
//! Advanced Performance Tracking System V3.5
//! Works with the new Trade struct (src/trading/trade.rs)
//! ═══════════════════════════════════════════════════════════════════════════

use crate::trading::Trade;
use serde::{Deserialize, Serialize};

/// Advanced performance tracker with full trade history
pub struct PerformanceTracker {
    trades: Vec<Trade>,
    equity_curve: Vec<f64>,
    initial_capital: f64,

    // Running totals for efficiency
    gross_profit: f64,
    gross_loss: f64,
    total_fees: f64,
}

impl PerformanceTracker {
    /// Create a new performance tracker
    pub fn new(initial_capital: f64) -> Self {
        Self {
            trades: Vec::new(),
            equity_curve: vec![initial_capital],
            initial_capital,
            gross_profit: 0.0,
            gross_loss: 0.0,
            total_fees: 0.0,
        }
    }

    /// Record a completed trade
    pub fn record_trade(&mut self, trade: Trade) {
        // Update running totals
        if trade.net_pnl > 0.0 {
            self.gross_profit += trade.net_pnl;
        } else {
            self.gross_loss += trade.net_pnl.abs();
        }

        self.total_fees += trade.fees_paid;

        // Update equity curve
        let current_equity = self.current_equity();
        self.equity_curve.push(current_equity + trade.net_pnl);

        // Store trade
        self.trades.push(trade);
    }

    /// Get current equity (portfolio value)
    pub fn current_equity(&self) -> f64 {
        self.equity_curve
            .last()
            .copied()
            .unwrap_or(self.initial_capital)
    }

    /// Calculate total return percentage
    pub fn total_return(&self) -> f64 {
        let current = self.current_equity();
        ((current - self.initial_capital) / self.initial_capital) * 100.0
    }

    /// Calculate win rate (percentage of winning trades)
    pub fn win_rate(&self) -> f64 {
        if self.trades.is_empty() {
            return 0.0;
        }

        let wins = self.trades.iter().filter(|t| t.is_winner()).count();
        (wins as f64 / self.trades.len() as f64) * 100.0
    }

    /// Calculate profit factor (gross profit / gross loss)
    pub fn profit_factor(&self) -> f64 {
        if self.gross_loss == 0.0 {
            return if self.gross_profit > 0.0 {
                f64::INFINITY
            } else {
                0.0
            };
        }
        self.gross_profit / self.gross_loss
    }

    /// Average P&L per trade
    pub fn avg_trade_pnl(&self) -> f64 {
        if self.trades.is_empty() {
            return 0.0;
        }

        let total: f64 = self.trades.iter().map(|t| t.net_pnl).sum();
        total / self.trades.len() as f64
    }

    /// Average winning trade
    pub fn avg_winner(&self) -> f64 {
        let winners: Vec<&Trade> = self.trades.iter().filter(|t| t.is_winner()).collect();

        if winners.is_empty() {
            return 0.0;
        }

        let total: f64 = winners.iter().map(|t| t.net_pnl).sum();
        total / winners.len() as f64
    }

    /// Average losing trade
    pub fn avg_loser(&self) -> f64 {
        let losers: Vec<&Trade> = self.trades.iter().filter(|t| !t.is_winner()).collect();

        if losers.is_empty() {
            return 0.0;
        }

        let total: f64 = losers.iter().map(|t| t.net_pnl).sum();
        total / losers.len() as f64
    }

    /// Calculate maximum drawdown (peak to trough decline)
    pub fn max_drawdown(&self) -> f64 {
        let mut peak = self.initial_capital;
        let mut max_dd = 0.0;

        for &equity in &self.equity_curve {
            if equity > peak {
                peak = equity;
            }

            let drawdown = ((peak - equity) / peak) * 100.0;
            if drawdown > max_dd {
                max_dd = drawdown;
            }
        }

        max_dd
    }

    /// Calculate Sharpe ratio (risk-adjusted return)
    pub fn sharpe_ratio(&self, risk_free_rate: f64) -> f64 {
        if self.trades.len() < 2 {
            return 0.0;
        }

        // Calculate returns
        let returns: Vec<f64> = self.trades.iter().map(|t| t.pnl_percent).collect();

        // Mean return
        let mean: f64 = returns.iter().sum::<f64>() / returns.len() as f64;

        // Standard deviation
        let variance: f64 =
            returns.iter().map(|r| (r - mean).powi(2)).sum::<f64>() / (returns.len() - 1) as f64;
        let std_dev = variance.sqrt();

        if std_dev == 0.0 {
            return 0.0;
        }

        (mean - risk_free_rate) / std_dev
    }

    /// Generate comprehensive performance report
    pub fn generate_report(&self) -> PerformanceReport {
        PerformanceReport {
            total_trades: self.trades.len() as u64,
            winning_trades: self.trades.iter().filter(|t| t.is_winner()).count() as u64,
            losing_trades: self.trades.iter().filter(|t| !t.is_winner()).count() as u64,

            initial_capital: self.initial_capital,
            current_equity: self.current_equity(),
            total_return_pct: self.total_return(),

            gross_profit: self.gross_profit,
            gross_loss: self.gross_loss,
            net_profit: self.gross_profit - self.gross_loss,
            total_fees: self.total_fees,

            win_rate: self.win_rate(),
            profit_factor: self.profit_factor(),

            avg_trade: self.avg_trade_pnl(),
            avg_winner: self.avg_winner(),
            avg_loser: self.avg_loser(),

            largest_win: self
                .trades
                .iter()
                .map(|t| t.net_pnl)
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap_or(0.0),
            largest_loss: self
                .trades
                .iter()
                .map(|t| t.net_pnl)
                .min_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap_or(0.0),

            max_drawdown: self.max_drawdown(),
            sharpe_ratio: self.sharpe_ratio(0.0),
        }
    }

    /// Get all trades (for detailed analysis)
    pub fn trades(&self) -> &[Trade] {
        &self.trades
    }

    /// Get equity curve (for plotting)
    pub fn equity_curve(&self) -> &[f64] {
        &self.equity_curve
    }
}

/// Comprehensive performance report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    pub total_trades: u64,
    pub winning_trades: u64,
    pub losing_trades: u64,

    pub initial_capital: f64,
    pub current_equity: f64,
    pub total_return_pct: f64,

    pub gross_profit: f64,
    pub gross_loss: f64,
    pub net_profit: f64,
    pub total_fees: f64,

    pub win_rate: f64,
    pub profit_factor: f64,

    pub avg_trade: f64,
    pub avg_winner: f64,
    pub avg_loser: f64,
    pub largest_win: f64,
    pub largest_loss: f64,

    pub max_drawdown: f64,
    pub sharpe_ratio: f64,
}

impl PerformanceReport {
    pub fn display(&self) {
        println!("\n╔═══════════════════════════════════════════════════════════╗");
        println!("║          📊 PERFORMANCE REPORT                            ║");
        println!("╚═══════════════════════════════════════════════════════════╝");

        println!("\n💼 ACCOUNT:");
        println!("   Initial Capital:    ${:.2}", self.initial_capital);
        println!("   Current Equity:     ${:.2}", self.current_equity);
        println!("   Net Profit:         ${:+.2}", self.net_profit);
        println!("   Total Return:       {:+.2}%", self.total_return_pct);

        println!("\n📈 TRADES:");
        println!("   Total Trades:       {}", self.total_trades);
        println!(
            "   Winners:            {} ({:.1}%)",
            self.winning_trades, self.win_rate
        );
        println!("   Losers:             {}", self.losing_trades);

        println!("\n💰 PROFITABILITY:");
        println!("   Gross Profit:       ${:.2}", self.gross_profit);
        println!("   Gross Loss:         ${:.2}", self.gross_loss);
        println!("   Total Fees:         ${:.2}", self.total_fees);
        println!("   Profit Factor:      {:.2}x", self.profit_factor);

        println!("\n📊 TRADE STATS:");
        println!("   Avg Trade:          ${:+.2}", self.avg_trade);
        println!("   Avg Winner:         ${:+.2}", self.avg_winner);
        println!("   Avg Loser:          ${:+.2}", self.avg_loser);
        println!("   Largest Win:        ${:+.2}", self.largest_win);
        println!("   Largest Loss:       ${:+.2}", self.largest_loss);

        println!("\n⚠️  RISK:");
        println!("   Max Drawdown:       {:.2}%", self.max_drawdown);
        println!("   Sharpe Ratio:       {:.2}", self.sharpe_ratio);

        println!();
    }
}
