//! Real-time trade logging system
//!
//! Logs all trades to CSV for analysis

use crate::trading::Trade;
use std::fs::OpenOptions;
use std::io::Write;
// use std::path::Path;
use chrono::Utc;
use log::{error, info};

/// Trade logger for CSV export
pub struct TradeLogger {
    output_path: String,
    session_id: String,
}

impl TradeLogger {
    /// Create new trade logger
    pub fn new(output_dir: &str) -> Self {
        let session_id = Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let output_path = format!("{}/trades_{}.csv", output_dir, session_id);

        // Create directory if it doesn't exist
        std::fs::create_dir_all(output_dir).ok();

        // Write header
        if let Err(e) = Self::write_header(&output_path) {
            error!("Failed to write CSV header: {}", e);
        } else {
            info!("📊 Trade logger initialized: {}", output_path);
        }

        Self {
            output_path,
            session_id,
        }
    }

    /// Write CSV header
    fn write_header(path: &str) -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;

        writeln!(file, "timestamp,trade_id,order_id,side,entry_price,exit_price,size,value_usd,gross_pnl,fees_paid,net_pnl,pnl_percent,duration_secs,volatility")?;

        Ok(())
    }

    /// Log a trade
    pub fn log_trade(&self, trade: &Trade) {
        if let Err(e) = self.write_trade(trade) {
            error!("Failed to log trade: {}", e);
        }
    }

    /// Write trade to CSV
    fn write_trade(&self, trade: &Trade) -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.output_path)?;

        writeln!(
            file,
            "{},{},{},{:?},{},{},{},{},{},{},{},{},{},{}",
            trade.exit_time.format("%Y-%m-%d %H:%M:%S"),
            trade.id,
            trade.order_id,
            trade.side,
            trade.entry_price,
            trade.exit_price,
            trade.size,
            trade.value_usd,
            trade.gross_pnl,
            trade.fees_paid,
            trade.net_pnl,
            trade.pnl_percent,
            trade.duration_secs,
            trade.volatility
        )?;

        Ok(())
    }

    /// Get session ID
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Get output path
    pub fn output_path(&self) -> &str {
        &self.output_path
    }
}
