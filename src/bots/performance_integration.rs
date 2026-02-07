//! Performance tracking integration for GridBot
//! Connects PerformanceTracker to live trading

use crate::metrics::performance::PerformanceTracker;
use crate::trading::Trade;
use log::info;

pub struct BotPerformanceMonitor {
    tracker: PerformanceTracker,
    session_start: std::time::Instant,
}

impl BotPerformanceMonitor {
    pub fn new(initial_capital: f64) -> Self {
        Self {
            tracker: PerformanceTracker::new(initial_capital),
            session_start: std::time::Instant::now(),
        }
    }

    pub fn record_trade(&mut self, trade: Trade) {
        info!("📊 Recording trade: ${:.2} P&L", trade.net_pnl);
        self.tracker.record_trade(trade);
    }

    pub fn session_duration(&self) -> std::time::Duration {
        self.session_start.elapsed()
    }

    pub fn display_summary(&self) {
        let report = self.tracker.generate_report();
        report.display();
    }
}