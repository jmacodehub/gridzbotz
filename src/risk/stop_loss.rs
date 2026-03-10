//! ═══════════════════════════════════════════════════════════════════════════
//! 🛑 STOP-LOSS & TAKE-PROFIT MANAGER  (V5.2 — PR #89)
//!
//! Single responsibility: decide whether an open position should be closed
//! based on price movement relative to entry.
//!
//! ## Wiring
//! Constructed once per `RealTradingEngine` instance via `new(&global_config)`.
//! `execute_trade()` / `process_price_update()` call `should_stop_loss()` /
//! `should_take_profit()` on each price tick so the bot exits immediately
//! when a threshold is crossed — no network round-trip is wasted.
//!
//! ## Stop modes (controlled by `[risk] enable_trailing_stop` in TOML)
//!
//! | `enable_trailing_stop` | Behaviour |
//! |------------------------|-----------|
//! | `false` (default)      | Fixed stop — reference is always `entry_price` |
//! | `true`                 | Trailing stop — reference ratchets up with the
//!                            highest price seen since position open |
//!
//! Trailing stop guarantees the stop never moves **down**: gains are locked
//! in as price rises; if price reverses the stop fires at
//! `highest_price * (1 - stop_loss_pct / 100)`.
//!
//! ## Config source
//! All values come exclusively from `[risk]` in the active TOML:
//!   `stop_loss_pct`        → loss threshold (%)
//!   `take_profit_pct`      → gain threshold (%)
//!   `enable_trailing_stop` → fixed vs trailing mode
//!
//! ## Changelog
//! V5.2 (PR #89): Wire `enable_trailing_stop` from `RiskConfig`.
//!                Add `entry_price()` accessor for `RealTradingEngine`.
//!                Resolve all TODO(tech-debt) notes from PR #88.
//! V5.1 (PR #88): Initial implementation — fixed stop only.
//! ═══════════════════════════════════════════════════════════════════════════

use crate::Config;
use log::{info, warn};

pub struct StopLossManager {
    /// True when stop_loss_pct > 0.0 — guards are no-ops otherwise.
    enabled: bool,
    /// % drop from reference price that triggers a stop-loss exit.
    /// Source: config.risk.stop_loss_pct
    stop_loss_pct: f64,
    /// % gain from entry price that triggers a take-profit exit.
    /// Source: config.risk.take_profit_pct
    take_profit_pct: f64,
    /// When true, reference ratchets upward with the highest observed price.
    /// Source: config.risk.enable_trailing_stop (wired in PR #89).
    trailing_stop: bool,
    /// Highest price seen since position open (trailing-stop anchor).
    /// Lazily initialised from entry_price on the first check.
    highest_price: f64,
    /// Entry price stored for inspection / logging by callers.
    /// Updated by `reset()` / `reset_for_new_position()`.
    entry_price: f64,
}

impl StopLossManager {
    /// Construct from master config.
    ///
    /// Reads exclusively from `config.risk` — no other config section.
    /// `enabled` is derived from `stop_loss_pct > 0.0` so a zero threshold
    /// acts as a runtime disable without a separate flag.
    pub fn new(config: &Config) -> Self {
        let enabled       = config.risk.stop_loss_pct > 0.0;
        // ✅ PR #89: read directly from RiskConfig, replacing the hardcoded `false`
        let trailing_stop = config.risk.enable_trailing_stop;

        info!("🛑 Initializing Stop-Loss Manager");
        if enabled {
            let mode = if trailing_stop { "trailing stop" } else { "fixed stop" };
            info!("   Stop-loss:    -{:.1}%  ({})", config.risk.stop_loss_pct, mode);
            info!("   Take-profit:  +{:.1}%",      config.risk.take_profit_pct);
        } else {
            warn!("   Stop-loss DISABLED (stop_loss_pct = 0.0)");
        }

        Self {
            enabled,
            stop_loss_pct:   config.risk.stop_loss_pct,
            take_profit_pct: config.risk.take_profit_pct,
            trailing_stop,
            // Lazily anchored from entry_price on first call to should_stop_loss().
            highest_price:   0.0,
            entry_price:     0.0,
        }
    }

    /// Returns true if the position should be closed at a loss.
    ///
    /// **Fixed stop** (`enable_trailing_stop = false`, default):
    ///   Fires when `current_price` drops `stop_loss_pct`% below `entry_price`.
    ///
    /// **Trailing stop** (`enable_trailing_stop = true`):
    ///   Reference ratchets up with `highest_price` (anchored to `entry_price`
    ///   on the first call). Fires when price drops `stop_loss_pct`% below the
    ///   highest observed price since position open.
    ///   The stop never moves downward — gains are locked in.
    pub fn should_stop_loss(&mut self, entry_price: f64, current_price: f64) -> bool {
        if !self.enabled {
            return false;
        }

        // Lazily anchor the trailing-stop high from entry_price on first call.
        if self.trailing_stop && self.highest_price == 0.0 {
            self.highest_price = entry_price;
        }

        // Ratchet the trailing high upward — never downward.
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
            let mode = if self.trailing_stop { "trailing" } else { "fixed" };
            warn!("🛑 STOP-LOSS TRIGGERED! ({})", mode);
            warn!(
                "   Entry: ${:.4} | Reference: ${:.4} | Current: ${:.4} | Loss: {:.2}% | Threshold: -{:.1}%",
                entry_price, reference_price, current_price, loss_pct, self.stop_loss_pct
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
    /// Updates the stored entry_price and clears the trailing-stop high so it
    /// re-anchors from the new entry price on the next `should_stop_loss()` call.
    pub fn reset(&mut self, entry_price: f64) {
        self.entry_price  = entry_price;
        self.highest_price = entry_price;
    }

    /// Alias for `reset()` — preferred name at call sites for clarity.
    #[inline]
    pub fn reset_for_new_position(&mut self, entry_price: f64) {
        self.reset(entry_price);
    }

    /// Returns the stored entry price (set by the last `reset()` call).
    /// Used by `RealTradingEngine::process_price_update()` to pass the
    /// correct reference price on each price tick without storing it twice.
    #[inline]
    pub fn entry_price(&self) -> f64 {
        self.entry_price
    }

    /// Expose thresholds for display / logging.
    pub fn thresholds(&self) -> (f64, f64) {
        (self.stop_loss_pct, self.take_profit_pct)
    }

    /// True if trailing-stop mode is active.
    #[inline]
    pub fn is_trailing(&self) -> bool {
        self.trailing_stop
    }

    /// Current trailing-stop high (0.0 if not yet anchored).
    #[inline]
    pub fn highest_observed_price(&self) -> f64 {
        self.highest_price
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
                // ✅ PR #89: include new field in test helper
                enable_trailing_stop: false,
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

    fn test_config_trailing() -> Config {
        let mut cfg = test_config();
        cfg.risk.enable_trailing_stop = true;
        cfg
    }

    // ── Field binding tests ───────────────────────────────────────────────────────

    #[test]
    fn test_reads_correct_config_fields() {
        let config = test_config();
        let mgr = StopLossManager::new(&config);
        let (sl, tp) = mgr.thresholds();
        assert_eq!(sl, 5.0,  "stop_loss_pct must come from RiskConfig");
        assert_eq!(tp, 10.0, "take_profit_pct must come from RiskConfig");
    }

    #[test]
    fn test_enabled_when_stop_loss_pct_positive() {
        let config = test_config();
        let mut mgr = StopLossManager::new(&config);
        assert!(mgr.should_stop_loss(100.0, 95.0));
    }

    #[test]
    fn test_disabled_when_stop_loss_pct_zero() {
        let mut config = test_config();
        config.risk.stop_loss_pct = 0.0;
        let mut mgr = StopLossManager::new(&config);
        assert!(!mgr.should_stop_loss(100.0, 50.0));
    }

    // ── Fixed stop tests ────────────────────────────────────────────────────────────

    #[test]
    fn test_fixed_stop_triggers_at_threshold() {
        let config = test_config();
        let mut mgr = StopLossManager::new(&config);
        assert!(mgr.should_stop_loss(100.0, 95.0));
    }

    #[test]
    fn test_fixed_stop_does_not_trigger_below_threshold() {
        let config = test_config();
        let mut mgr = StopLossManager::new(&config);
        assert!(!mgr.should_stop_loss(100.0, 95.1));
    }

    #[test]
    fn test_fixed_stop_does_not_trigger_on_small_loss() {
        let config = test_config();
        let mut mgr = StopLossManager::new(&config);
        assert!(!mgr.should_stop_loss(100.0, 99.5));
    }

    // ── Trailing stop tests (PR #89) ──────────────────────────────────────────────

    #[test]
    fn test_trailing_stop_config_wired_from_risk_config() {
        let config = test_config_trailing();
        let mgr = StopLossManager::new(&config);
        assert!(mgr.is_trailing(), "enable_trailing_stop=true must set trailing_stop=true");
    }

    #[test]
    fn test_fixed_stop_config_not_trailing() {
        let config = test_config();
        let mgr = StopLossManager::new(&config);
        assert!(!mgr.is_trailing(), "enable_trailing_stop=false must keep trailing_stop=false");
    }

    #[test]
    fn test_trailing_stop_ratchets_high() {
        // entry=100, price rises to 120 → highest=120
        // price drops to 115 → loss from 120 = -4.17%, threshold=5% → no trigger
        let config = test_config_trailing();
        let mut mgr = StopLossManager::new(&config);
        assert!(!mgr.should_stop_loss(100.0, 120.0)); // ratchet high to 120
        assert!(!mgr.should_stop_loss(100.0, 115.0)); // 115 < 120 but < 5% drop
        assert_eq!(mgr.highest_observed_price(), 120.0);
    }

    #[test]
    fn test_trailing_stop_triggers_after_ratchet() {
        // entry=100, price rises to 120 → highest=120
        // price drops to 113 → loss from 120 = -5.83% > 5% threshold → trigger
        let config = test_config_trailing();
        let mut mgr = StopLossManager::new(&config);
        assert!(!mgr.should_stop_loss(100.0, 120.0)); // ratchet high
        assert!(mgr.should_stop_loss(100.0, 113.0));  // fire: -5.83% from 120
    }

    #[test]
    fn test_trailing_stop_does_not_move_down() {
        // Verify highest_price only ever increases
        let config = test_config_trailing();
        let mut mgr = StopLossManager::new(&config);
        mgr.should_stop_loss(100.0, 120.0);
        mgr.should_stop_loss(100.0, 110.0); // lower than 120 → should NOT update high
        assert_eq!(mgr.highest_observed_price(), 120.0,
            "highest_price must never decrease");
    }

    // ── Take-profit tests ───────────────────────────────────────────────────────────

    #[test]
    fn test_take_profit_triggers_at_threshold() {
        let config = test_config();
        let mgr = StopLossManager::new(&config);
        assert!(mgr.should_take_profit(100.0, 110.0));
    }

    #[test]
    fn test_take_profit_does_not_trigger_below_threshold() {
        let config = test_config();
        let mgr = StopLossManager::new(&config);
        assert!(!mgr.should_take_profit(100.0, 109.9));
    }

    // ── Reset + entry_price accessor tests ─────────────────────────────────────

    #[test]
    fn test_reset_sets_entry_and_highest() {
        let config = test_config();
        let mut mgr = StopLossManager::new(&config);
        mgr.reset_for_new_position(200.0);
        assert_eq!(mgr.highest_price, 200.0);
        assert_eq!(mgr.entry_price(), 200.0);
    }

    #[test]
    fn test_entry_price_accessor_returns_stored_entry() {
        let config = test_config();
        let mut mgr = StopLossManager::new(&config);
        mgr.reset(150.0);
        assert_eq!(mgr.entry_price(), 150.0);
    }
}
