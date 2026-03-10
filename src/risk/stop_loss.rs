//! ═══════════════════════════════════════════════════════════════════════════
//! 🛑 STOP-LOSS & TAKE-PROFIT MANAGER
//!
//! Single responsibility: decide whether an open position should be closed
//! based on price movement relative to entry.
//!
//! ## Wiring
//! Constructed once per `RealTradingEngine` instance via `new(&global_config)`.
//! `execute_trade()` calls `should_stop_loss()` / `should_take_profit()`
//! *before* building the Jupiter swap — no network call is wasted on a
//! position that must be closed.
//!
//! ## Trailing stop
//! Currently fixed-stop only (trailing_stop = false).
//! TODO(tech-debt): add `enable_trailing_stop` to `RiskConfig` and wire here
//!                  as part of PR #89 layered-stop expansion.
//!
//! ## Config source
//! All thresholds come exclusively from `[risk]` in master.toml:
//!   stop_loss_pct   → RiskConfig::stop_loss_pct
//!   take_profit_pct → RiskConfig::take_profit_pct
//!
//! PR #88 — fix/risk-stop-loss-wiring
//! ═══════════════════════════════════════════════════════════════════════════

use crate::Config;
use log::{info, warn};

pub struct StopLossManager {
    /// Always true when stop_loss_pct > 0.0 — guards are no-ops otherwise.
    enabled: bool,
    /// % drop from reference price that triggers a stop-loss exit.
    /// Source: config.risk.stop_loss_pct  (NOT enable_circuit_breaker).
    stop_loss_pct: f64,
    /// % gain from entry price that triggers a take-profit exit.
    /// Source: config.risk.take_profit_pct (NOT enable_circuit_breaker).
    take_profit_pct: f64,
    /// When true, reference price ratchets upward with price gains.
    /// TODO(tech-debt): wire from config.risk.enable_trailing_stop (PR #89).
    trailing_stop: bool,
    /// Highest price seen since position open (trailing-stop anchor).
    /// Lazily initialised from entry_price on the first check.
    highest_price: f64,
}

impl StopLossManager {
    /// Construct from master config.
    ///
    /// Reads exclusively from `config.risk` — no other config section.
    /// `enabled` is derived from `stop_loss_pct > 0.0` so a zero threshold
    /// acts as a runtime disable without a separate flag.
    pub fn new(config: &Config) -> Self {
        // ✅ PR #88 fix: read stop_loss_pct and take_profit_pct directly.
        //    Previous code incorrectly used enable_circuit_breaker for both
        //    `enabled` and `trailing_stop` — wrong field, wrong concept.
        let enabled = config.risk.stop_loss_pct > 0.0;

        info!("🛑 Initializing Stop-Loss Manager");
        if enabled {
            info!("   Stop-loss:    -{:.1}%  (fixed stop)", config.risk.stop_loss_pct);
            info!("   Take-profit:  +{:.1}%",               config.risk.take_profit_pct);
        } else {
            warn!("   Stop-loss DISABLED (stop_loss_pct = 0.0)");
        }

        Self {
            enabled,
            stop_loss_pct:   config.risk.stop_loss_pct,
            take_profit_pct: config.risk.take_profit_pct,
            // TODO(tech-debt): replace with config.risk.enable_trailing_stop (PR #89)
            trailing_stop:   false,
            // Lazily anchored from entry_price on first call to should_stop_loss().
            highest_price:   0.0,
        }
    }

    /// Returns true if the position should be closed at a loss.
    ///
    /// When `trailing_stop` is enabled the reference price ratchets upward
    /// with the highest observed price since the position opened, anchored
    /// from `entry_price` on the very first call.
    ///
    /// Currently `trailing_stop = false` → reference is always `entry_price`.
    pub fn should_stop_loss(&mut self, entry_price: f64, current_price: f64) -> bool {
        if !self.enabled {
            return false;
        }

        // Lazily anchor the trailing-stop high from entry_price on first call.
        if self.trailing_stop && self.highest_price == 0.0 {
            self.highest_price = entry_price;
        }

        // Ratchet the trailing high upward.
        if self.trailing_stop && current_price > self.highest_price {
            self.highest_price = current_price;
        }

        let reference_price = if self.trailing_stop {
            self.highest_price
        } else {
            entry_price
        };

        let loss_pct = ((current_price - reference_price) / reference_price) * 100.0;

        if loss_pct <= -self.stop_loss_pct {
            warn!("🛑 STOP-LOSS TRIGGERED!");
            warn!(
                "   Entry: ${:.4} | Current: ${:.4} | Loss: {:.2}% | Threshold: -{:.1}%",
                entry_price, current_price, loss_pct, self.stop_loss_pct
            );
            return true;
        }

        false
    }

    /// Returns true if the position should be closed at a profit.
    pub fn should_take_profit(&self, entry_price: f64, current_price: f64) -> bool {
        if !self.enabled {
            return false;
        }

        let profit_pct = ((current_price - entry_price) / entry_price) * 100.0;

        if profit_pct >= self.take_profit_pct {
            info!("🎯 TAKE-PROFIT TRIGGERED!");
            info!(
                "   Entry: ${:.4} | Current: ${:.4} | Profit: {:.2}% | Threshold: +{:.1}%",
                entry_price, current_price, profit_pct, self.take_profit_pct
            );
            return true;
        }

        false
    }

    /// Reset for a new position — call before entering each trade.
    /// Clears the trailing-stop high so it re-anchors from new entry price.
    pub fn reset(&mut self, entry_price: f64) {
        self.highest_price = entry_price;
    }

    /// Alias for `reset()` — preferred name at call sites for clarity.
    #[inline]
    pub fn reset_for_new_position(&mut self, entry_price: f64) {
        self.reset(entry_price);
    }

    /// Expose thresholds for display / logging.
    pub fn thresholds(&self) -> (f64, f64) {
        (self.stop_loss_pct, self.take_profit_pct)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::*;

    fn test_config() -> Config {
        Config {
            bot: BotConfig {
                name: "Test".to_string(),
                version: "1.0".to_string(),
                environment: "test".to_string(),
                execution_mode: "paper".to_string(),
                instance_id: None,
            },
            network: NetworkConfig {
                cluster: "devnet".to_string(),
                rpc_url: "http://localhost".to_string(),
                commitment: "confirmed".to_string(),
                ws_url: None,
            },
            security: SecurityConfig::default(),
            trading: TradingConfig {
                grid_levels: 10,
                grid_spacing_percent: 0.2,
                min_order_size: 0.01,
                max_position_size: 1.0,
                min_usdc_reserve: 100.0,
                min_sol_reserve: 0.1,
                enable_dynamic_grid: false,
                reposition_threshold: 1.0,
                volatility_window: 50,
                enable_auto_rebalance: true,
                enable_smart_rebalance: false,
                rebalance_threshold_pct: 10.0,
                rebalance_cooldown_secs: 300,
                max_orders_per_side: 5,
                order_refresh_interval_secs: 600,
                enable_market_orders: false,
                enable_fee_optimization: false,
                min_profit_threshold_pct: 0.5,
                max_slippage_pct: 2.0,
                enable_price_bounds: false,
                lower_price_bound: 50.0,
                upper_price_bound: 150.0,
                enable_regime_gate: false,
                min_volatility_to_trade: 0.0,
                pause_in_very_low_vol: false,
                enable_order_lifecycle: false,
                order_max_age_minutes: 60,
                order_refresh_interval_minutes: 30,
                min_orders_to_maintain: 5,
                enable_adaptive_spacing: false,
                enable_smart_position_sizing: false,
            },
            strategies: StrategiesConfig::default(),
            execution: ExecutionConfig::default(),
            risk: RiskConfig {
                max_position_size_pct: 80.0,
                max_drawdown_pct: 10.0,
                stop_loss_pct: 5.0,
                take_profit_pct: 10.0,
                enable_circuit_breaker: true,
                circuit_breaker_threshold_pct: 15.0,
                circuit_breaker_cooldown_secs: 60,
                max_consecutive_losses: 5,
            },
            fees: FeesConfig::default(),
            priority_fees: PriorityFeeConfig::default(),
            pyth: PythConfig::default(),
            performance: PerformanceConfig::default(),
            logging: LoggingConfig::default(),
            metrics: MetricsConfig::default(),
            paper_trading: PaperTradingConfig::default(),
            database: DatabaseConfig::default(),
            alerts: AlertsConfig::default(),
        }
    }

    // ── Field binding tests ─────────────────────────────────────────────────

    #[test]
    fn test_reads_correct_config_fields() {
        let config = test_config(); // stop_loss_pct=5.0, take_profit_pct=10.0
        let mgr = StopLossManager::new(&config);
        let (sl, tp) = mgr.thresholds();
        assert_eq!(sl, 5.0,  "stop_loss_pct must come from RiskConfig");
        assert_eq!(tp, 10.0, "take_profit_pct must come from RiskConfig");
    }

    #[test]
    fn test_enabled_when_stop_loss_pct_positive() {
        let config = test_config(); // stop_loss_pct = 5.0 > 0
        let mut mgr = StopLossManager::new(&config);
        // Should trigger at exactly -5%
        assert!(mgr.should_stop_loss(100.0, 95.0));
    }

    #[test]
    fn test_disabled_when_stop_loss_pct_zero() {
        let mut config = test_config();
        config.risk.stop_loss_pct = 0.0;
        let mut mgr = StopLossManager::new(&config);
        // Any loss must be ignored when threshold is 0
        assert!(!mgr.should_stop_loss(100.0, 50.0));
    }

    // ── Stop-loss threshold tests ───────────────────────────────────────────

    #[test]
    fn test_stop_loss_triggers_at_threshold() {
        let config = test_config(); // stop_loss_pct = 5.0
        let mut mgr = StopLossManager::new(&config);
        // Exactly at threshold → trigger
        assert!(mgr.should_stop_loss(100.0, 95.0));
    }

    #[test]
    fn test_stop_loss_does_not_trigger_below_threshold() {
        let config = test_config();
        let mut mgr = StopLossManager::new(&config);
        // Just above threshold (-4.9%) → no trigger
        assert!(!mgr.should_stop_loss(100.0, 95.1));
    }

    #[test]
    fn test_stop_loss_does_not_trigger_on_small_loss() {
        let config = test_config();
        let mut mgr = StopLossManager::new(&config);
        assert!(!mgr.should_stop_loss(100.0, 99.5));
    }

    // ── Take-profit threshold tests ─────────────────────────────────────────

    #[test]
    fn test_take_profit_triggers_at_threshold() {
        let config = test_config(); // take_profit_pct = 10.0
        let mgr = StopLossManager::new(&config);
        assert!(mgr.should_take_profit(100.0, 110.0));
    }

    #[test]
    fn test_take_profit_does_not_trigger_below_threshold() {
        let config = test_config();
        let mgr = StopLossManager::new(&config);
        assert!(!mgr.should_take_profit(100.0, 109.9));
    }

    // ── Reset test ──────────────────────────────────────────────────────────

    #[test]
    fn test_reset_for_new_position() {
        let config = test_config();
        let mut mgr = StopLossManager::new(&config);
        mgr.reset_for_new_position(200.0);
        assert_eq!(mgr.highest_price, 200.0);
    }
}
