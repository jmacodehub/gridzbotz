//! Risk configuration — position limits, drawdown, stop-loss, circuit breaker.

use serde::{Deserialize, Serialize};
use anyhow::{Result, bail};
use log::warn;
use super::{
    default_true, default_max_consecutive_losses,
    default_trailing_stop, default_stop_loss_cooldown_secs,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RiskConfig {
    pub max_position_size_pct: f64,
    pub max_drawdown_pct: f64,
    pub stop_loss_pct: f64,
    pub take_profit_pct: f64,
    #[serde(default = "default_true")]
    pub enable_circuit_breaker: bool,
    pub circuit_breaker_threshold_pct: f64,
    pub circuit_breaker_cooldown_secs: u64,
    #[serde(default = "default_max_consecutive_losses")]
    pub max_consecutive_losses: u32,
    #[serde(default = "default_trailing_stop")]
    pub enable_trailing_stop: bool,
    /// Cooldown (seconds) after a stop-loss trip before trading resumes.
    /// Consumed by StopLossManager::new() — wired in grid_bot.rs.
    /// Default: 300s (5 min). Warn if 0 (may re-trip immediately).
    #[serde(default = "default_stop_loss_cooldown_secs")]
    pub stop_loss_cooldown_secs: u64,
}

impl RiskConfig {
    pub fn validate(&self) -> Result<()> {
        if self.max_position_size_pct <= 0.0 || self.max_position_size_pct > 100.0 {
            bail!("max_position_size_pct must be between 0-100%");
        }
        if self.max_drawdown_pct <= 0.0 || self.max_drawdown_pct > 100.0 {
            bail!("max_drawdown_pct must be between 0-100%");
        }
        if self.max_consecutive_losses == 0 {
            bail!("max_consecutive_losses must be > 0");
        }
        if self.enable_circuit_breaker {
            if self.circuit_breaker_threshold_pct <= 0.0 {
                bail!("circuit_breaker_threshold_pct must be positive");
            }
            if self.circuit_breaker_cooldown_secs == 0 {
                warn!("⚠️ circuit_breaker_cooldown_secs is 0 - may trigger repeatedly");
            }
        }
        if self.stop_loss_cooldown_secs == 0 {
            warn!("⚠️ risk.stop_loss_cooldown_secs is 0 — SL may re-trip immediately after reset");
        }
        Ok(())
    }
}
