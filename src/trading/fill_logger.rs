//! =============================================================================
//! Fill Logger — append-only CSV persistence for every FillEvent
//! Stage 3 / Step 3B  |  Zero external dependencies (std::fs only)
//! =============================================================================
//!
//! Creates one CSV file per calendar day:
//!
//!   fills/fills_YYYYMMDD.csv
//!
//! Schema (one row per fill):
//!
//!   timestamp, order_id, side, price, size, fee, pnl,
//!   grid_level_id, spacing, total_pnl
//!
//! The file is opened in append mode on every write — no persistent file
//! handle, no Mutex needed. Fill frequency is low enough that this is fine.
//!
//! Usage:
//!   let logger = FillLogger::new("fills")?;
//!   logger.append(&fill, Some(grid_spacing), running_total_pnl)?;

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use anyhow::Result;
use chrono::Utc;

use super::{FillEvent, OrderSide};

// =============================================================================
// FILL LOGGER
// =============================================================================

/// Appends `FillEvent` rows to a date-stamped CSV inside `dir`.
///
/// Thread-safe for read (&self) — opens/closes the file on every append.
pub struct FillLogger {
    dir: PathBuf,
}

impl FillLogger {
    /// Create a new logger.  `dir` is created if it does not exist.
    pub fn new(dir: impl Into<PathBuf>) -> Result<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        log::info!("[FillLogger] Logging fills to {}/fills_YYYYMMDD.csv", dir.display());
        Ok(Self { dir })
    }

    /// Path to today's fill log file.
    pub fn path(&self) -> PathBuf {
        let date_str = Utc::now().format("%Y%m%d").to_string();
        self.dir.join(format!("fills_{}.csv", date_str))
    }

    /// Append one `FillEvent` row.
    ///
    /// - `spacing`   — current grid spacing in % (pass `None` if unknown at
    ///                 call site; grid_bot can supply it later).
    /// - `total_pnl` — running session-total realised P&L in USDC at this
    ///                 moment (sum of all level_pnl entries).
    ///
    /// Writes the CSV header on the first write of each calendar day.
    /// Opens, writes, and closes the file atomically — safe for concurrent
    /// processes reading the same file.
    pub fn append(
        &self,
        fill: &FillEvent,
        spacing: Option<f64>,
        total_pnl: f64,
    ) -> Result<()> {
        let path = self.path();
        let is_new = !path.exists();

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;

        if is_new {
            writeln!(
                file,
                "timestamp,order_id,side,price,size,fee,pnl,\
                 grid_level_id,spacing,total_pnl"
            )?;
        }

        writeln!(
            file,
            "{},{},{},{:.6},{:.6},{:.6},{},{},{},{:.6}",
            fill.timestamp,
            fill.order_id,
            match fill.side {
                OrderSide::Buy  => "Buy",
                OrderSide::Sell => "Sell",
            },
            fill.price,
            fill.size,
            fill.fee,
            fill.pnl
                .map(|p| format!("{:.6}", p))
                .unwrap_or_default(),
            fill.grid_level_id
                .map(|l| l.to_string())
                .unwrap_or_default(),
            spacing
                .map(|s| format!("{:.6}", s))
                .unwrap_or_default(),
            total_pnl,
        )?;

        Ok(())
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn fill(order_id: &str, side: OrderSide, pnl: Option<f64>) -> FillEvent {
        FillEvent::new(
            order_id.to_string(),
            side,
            150.0,
            0.1,
            0.003,
            pnl,
            1_700_000_000,
        )
    }

    fn tmp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("gridzbotz_fill_logger_{}", name))
    }

    #[test]
    fn test_logger_creates_dir_and_file() {
        let dir = tmp("creates");
        let _ = fs::remove_dir_all(&dir);

        let logger = FillLogger::new(&dir).unwrap();
        logger.append(&fill("ORDER-000001-L1", OrderSide::Buy, None), Some(2.5), 0.0).unwrap();

        assert!(logger.path().exists(), "CSV file must be created after first append");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_logger_writes_header_on_first_fill() {
        let dir = tmp("header");
        let _ = fs::remove_dir_all(&dir);

        let logger = FillLogger::new(&dir).unwrap();
        logger.append(&fill("ORDER-000002-L2", OrderSide::Sell, Some(9.5)), None, 9.5).unwrap();

        let content = fs::read_to_string(logger.path()).unwrap();
        assert!(
            content.starts_with("timestamp,order_id,side,"),
            "First line must be the CSV header"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_logger_appends_multiple_fills() {
        let dir = tmp("multiple");
        let _ = fs::remove_dir_all(&dir);

        let logger = FillLogger::new(&dir).unwrap();
        logger.append(&fill("ORDER-000003-L3", OrderSide::Buy,  None),       Some(1.0), 0.0).unwrap();
        logger.append(&fill("ORDER-000004-L3", OrderSide::Sell, Some(4.2)),  Some(1.0), 4.2).unwrap();

        let content = fs::read_to_string(logger.path()).unwrap();
        let lines: Vec<&str> = content.trim().lines().collect();

        // header + 2 data rows
        assert_eq!(lines.len(), 3, "Expected header + 2 data rows, got {}", lines.len());
        assert!(lines[1].contains("Buy"),  "Row 1 must be Buy");
        assert!(lines[2].contains("Sell"), "Row 2 must be Sell");
        assert!(lines[2].contains("4.2"),  "Row 2 must contain pnl");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_logger_header_written_exactly_once() {
        let dir = tmp("no_double_header");
        let _ = fs::remove_dir_all(&dir);

        let logger = FillLogger::new(&dir).unwrap();
        let f = fill("ORDER-000005-L1", OrderSide::Buy, None);
        logger.append(&f, None, 0.0).unwrap();
        logger.append(&f, None, 0.0).unwrap();
        logger.append(&f, None, 0.0).unwrap();

        let content = fs::read_to_string(logger.path()).unwrap();
        let header_count = content
            .lines()
            .filter(|l| l.starts_with("timestamp,"))
            .count();
        assert_eq!(header_count, 1, "Header must appear exactly once no matter how many fills");
        let _ = fs::remove_dir_all(&dir);
    }
}
