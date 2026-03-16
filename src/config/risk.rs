//! Risk configuration — position limits, drawdown, stop-loss, circuit breaker.

use serde::{Deserialize, Serialize};
use anyhow::{Result, bail};
use log::warn;
use super::{
    default_true, default_max_consecutive_losses,
    default_trailing_stop, default_stop_loss_cooldown_secs,
};

// ── WinRateGuard defaults ─────────────────────────────────────────────────────────────────────────
fn default_enable_win_rate_guard()    -> bool { false }
fn default_min_win_rate_pct()         -> f64  { 40.0  }
fn default_win_rate_guard_resume_pct() -> f64 { 45.0  }
fn default_min_trades_before_guard()  -> u64  { 10    }

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

    // ── Win Rate Guard (PR #131 C4) ────────────────────────────────────────────────────
    /// Opt-in feature flag. Default false — safe for all existing instances.
    #[serde(default = "default_enable_win_rate_guard")]
    pub enable_win_rate_guard: bool,
    /// Suppress trading when rolling win-rate falls below this percentage.
    /// Default 40.0%. Only evaluated when enable_win_rate_guard = true.
    #[serde(default = "default_min_win_rate_pct")]
    pub min_win_rate_pct: f64,
    /// Resume trading only when win-rate rises above this percentage.
    /// Must be >= min_win_rate_pct. Hysteresis prevents boundary oscillation.
    /// Default 45.0%.
    #[serde(default = "default_win_rate_guard_resume_pct")]
    pub win_rate_guard_resume_pct: f64,
    /// Number of completed trades required before the guard activates.
    /// Prevents false suppression on cold-start (e.g. 0/1 = 0%).
    /// Default 10.
    #[serde(default = "default_min_trades_before_guard")]
    pub min_trades_before_guard: u64,
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
        // ✅ clippy::collapsible_if: merged outer + inner guard into single &&
        if self.enable_win_rate_guard
            && self.win_rate_guard_resume_pct < self.min_win_rate_pct
        {
            bail!(
                "win_rate_guard_resume_pct ({:.1}%) must be >= min_win_rate_pct ({:.1}%) \
                 to enforce hysteresis band",
                self.win_rate_guard_resume_pct,
                self.min_win_rate_pct
            );
        }
        Ok(())
    }
}
