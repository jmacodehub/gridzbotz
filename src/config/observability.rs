//! Observability configuration — Pyth, performance, logging, metrics,
//! paper trading, database, alerts.

use serde::{Deserialize, Serialize};
use anyhow::{Result, bail};
use log::warn;
use super::{
    default_true,
    default_update_interval, default_cycle_interval,
    default_startup_delay, default_request_timeout,
    default_stats_interval,
    default_initial_usdc, default_initial_sol,
};

// ─────────────────────────────────────────────────────────────────────────────
// PythConfig
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PythConfig {
    pub http_endpoint: String,
    pub feed_ids: Vec<String>,
    #[serde(default = "default_update_interval")]
    pub update_interval_ms: u64,
    #[serde(default)]
    pub enable_websocket: bool,
    #[serde(default)]
    pub websocket_endpoint: Option<String>,
}

impl Default for PythConfig {
    fn default() -> Self {
        Self {
            http_endpoint: "https://hermes.pyth.network".to_string(),
            feed_ids: vec![
                "0xef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d".to_string()
            ],
            update_interval_ms:  500,
            enable_websocket:    false,
            websocket_endpoint:  None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PerformanceConfig
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PerformanceConfig {
    #[serde(default = "default_cycle_interval")]
    pub cycle_interval_ms: u64,
    #[serde(default = "default_startup_delay")]
    pub startup_delay_ms: u64,
    #[serde(default = "default_request_timeout")]
    pub request_timeout_ms: u64,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self { cycle_interval_ms: 100, startup_delay_ms: 1000, request_timeout_ms: 5000 }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// LoggingConfig
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LoggingConfig {
    pub level: String,
    pub file_path: String,
    #[serde(default = "default_true")]
    pub enable_file_logging: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level:                "info".to_string(),
            file_path:            "logs/gridbot.log".to_string(),
            enable_file_logging:  true,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MetricsConfig
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MetricsConfig {
    #[serde(default = "default_true")]
    pub enable_metrics: bool,
    #[serde(default = "default_stats_interval")]
    pub stats_interval: u64,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self { enable_metrics: true, stats_interval: 50 }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PaperTradingConfig
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PaperTradingConfig {
    #[serde(default = "default_initial_usdc")]
    pub initial_usdc: f64,
    #[serde(default = "default_initial_sol")]
    pub initial_sol: f64,
    #[serde(default)]
    pub test_duration_hours: Option<usize>,
    #[serde(default)]
    pub test_duration_minutes: Option<usize>,
    #[serde(default)]
    pub test_duration_seconds: Option<usize>,
    #[serde(default)]
    pub test_cycles: Option<usize>,
}

impl PaperTradingConfig {
    pub fn duration_seconds(&self) -> usize {
        if let Some(secs)  = self.test_duration_seconds { return secs; }
        if let Some(mins)  = self.test_duration_minutes { return mins * 60; }
        if let Some(hours) = self.test_duration_hours   { return hours * 3600; }
        3600
    }
    pub fn calculate_cycles(&self, cycle_interval_ms: u64) -> usize {
        if let Some(cycles) = self.test_cycles { return cycles; }
        let duration_secs  = self.duration_seconds();
        let cycles_per_sec = 1000 / cycle_interval_ms as usize;
        duration_secs * cycles_per_sec
    }
    pub fn validate(&self) -> Result<()> {
        if self.initial_usdc <= 0.0 { bail!("initial_usdc must be positive"); }
        if self.initial_sol  <= 0.0 { bail!("initial_sol must be positive"); }
        if self.test_duration_hours.is_none()
            && self.test_duration_minutes.is_none()
            && self.test_duration_seconds.is_none()
            && self.test_cycles.is_none() {
            warn!("⚠️ No test duration specified - using default 1 hour");
        }
        Ok(())
    }
}

impl Default for PaperTradingConfig {
    fn default() -> Self {
        Self {
            initial_usdc:           5000.0,
            initial_sol:            10.0,
            test_duration_hours:    Some(1),
            test_duration_minutes:  None,
            test_duration_seconds:  None,
            test_cycles:            None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DatabaseConfig + AlertsConfig
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct DatabaseConfig {
    #[serde(default)] pub enabled: bool,
    #[serde(default)] pub url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct AlertsConfig {
    #[serde(default)] pub enabled: bool,
    #[serde(default)] pub telegram_bot_token: Option<String>,
    #[serde(default)] pub telegram_chat_id:   Option<String>,
}
