//! ═══════════════════════════════════════════════════════════════════════════
//! 🛑 STOP-LOSS & TAKE-PROFIT MANAGER  (V5.3 — PR #126 C1)
//!
//! V5.3 (PR #126 C1): Add cooldown-based `is_trading_allowed()` so stop-loss
//!   and take-profit events pause the bot (TickResult::paused) rather than
//!   triggering a permanent shutdown. Mirrors CircuitBreaker's trip/cooldown
//!   auto-reset pattern exactly. New fields: `tripped_at`, `cooldown_secs`.
//!   New methods: `is_trading_allowed()`, `trip()`, `reset_sl()`,
//!   `sl_cooldown_remaining()`, `trip_reason()`.
//!   Config source: `[risk] stop_loss_cooldown_secs` (default 300s).
//!
//! V5.2 (PR #89): Wire `enable_trailing_stop` from `RiskConfig`.
//!                Add `entry_price()` accessor for `RealTradingEngine`.
//!                Resolve all TODO(tech-debt) notes from PR #88.
//! V5.1 (PR #88): Initial implementation — fixed stop only.
//!
//! ## Wiring
//! Constructed once per `GridBot` instance via `new(&config)` (PR #126 C1)
//! and once per `RealTradingEngine` via `new(&config)` (existing).
//! `process_tick()` calls `is_trading_allowed()` each cycle and returns
//! `TickResult::paused` when tripped, auto-resuming after `cooldown_secs`.
//!
//! ## Stop modes (controlled by `[risk] enable_trailing_stop` in TOML)
//!
//! | `enable_trailing_stop` | Behaviour |
//! |------------------------|-----------|
//! | `false` (default) | Fixed stop — reference is always `entry_price` |
//! | `true` | Trailing stop — reference ratchets up with the highest price seen since position open |
//!
//! ## Escalation ladder
//! SL fires → paused(cooldown_secs) → auto-resume → reset_for_new_position()
//! If losses persist → CircuitBreaker trips on NAV drawdown → longer halt
//! Only CircuitBreaker should ever return TickResult::shutdown().
//! ═══════════════════════════════════════════════════════════════════════════

use crate::Config;
use std::time::{Duration, Instant};
use log::{info, warn, error};

/// Reason the stop-loss manager is currently paused.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SlTripReason {
    /// Price dropped below stop-loss threshold.
    StopLoss,
    /// Price rose above take-profit threshold.
    TakeProfit,
}

impl std::fmt::Display for SlTripReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SlTripReason::StopLoss  => write!(f, "stop-loss triggered"),
            SlTripReason::TakeProfit => write!(f, "take-profit triggered"),
        }
    }
}

pub struct StopLossManager {
    /// True when stop_loss_pct > 0.0 — guards are no-ops otherwise.
    enabled: bool,
    /// % drop from reference price that triggers a stop-loss exit.
    stop_loss_pct: f64,
    /// % gain from entry price that triggers a take-profit exit.
    take_profit_pct: f64,
    /// When true, reference ratchets upward with the highest observed price.
    trailing_stop: bool,
    /// Highest price seen since position open (trailing-stop anchor).
    highest_price: f64,
    /// Entry price stored for inspection / logging by callers.
    entry_price: f64,
    // ── PR #126 C1: cooldown state (mirrors CircuitBreaker pattern) ──────
    /// Set when SL/TP fires; cleared after `cooldown_secs` elapses.
    tripped_at: Option<Instant>,
    /// How long to pause after a stop-loss or take-profit event.
    /// Source: config.risk.stop_loss_cooldown_secs (default 300s).
    cooldown_secs: u64,
    /// Why the manager is currently tripped (None when not tripped).
    trip_reason: Option<SlTripReason>,
}

impl StopLossManager {
    /// Construct from master config.
    pub fn new(config: &Config) -> Self {
        let enabled       = config.risk.stop_loss_pct > 0.0;
        let trailing_stop = config.risk.enable_trailing_stop;
        let cooldown_secs = config.risk.stop_loss_cooldown_secs;

        info!("🛑 Initializing Stop-Loss Manager V5.3");
        if enabled {
            let mode = if trailing_stop { "trailing stop" } else { "fixed stop" };
            info!("   Stop-loss:    -{:.1}%  ({})", config.risk.stop_loss_pct, mode);
            info!("   Take-profit:  +{:.1}%",      config.risk.take_profit_pct);
            info!("   SL cooldown:  {}s (PR #126)", cooldown_secs);
        } else {
            warn!("   Stop-loss DISABLED (stop_loss_pct = 0.0)");
        }

        Self {
            enabled,
            stop_loss_pct:   config.risk.stop_loss_pct,
            take_profit_pct: config.risk.take_profit_pct,
            trailing_stop,
            highest_price:   0.0,
            entry_price:     0.0,
            tripped_at:      None,
            cooldown_secs,
            trip_reason:     None,
        }
    }

    // ── PR #126 C1: cooldown gate ────────────────────────────────────────

    /// Returns `true` when the bot may trade; `false` during SL/TP cooldown.
    ///
    /// Auto-resets after `cooldown_secs` elapses — callers do not need to
    /// call `reset_sl()` manually. Mirrors `CircuitBreaker::is_trading_allowed()`.
    pub fn is_trading_allowed(&mut self) -> bool {
        let Some(tripped_at) = self.tripped_at else {
            return true;
        };
        let elapsed  = tripped_at.elapsed();
        let cooldown = Duration::from_secs(self.cooldown_secs);
        if elapsed >= cooldown {
            info!("✅ Stop-loss cooldown complete — resuming trading");
            self.reset_sl();
            true
        } else {
            let remaining = cooldown - elapsed;
            warn!("⏸️  SL cooldown active — {}s remaining ({:?})",
                  remaining.as_secs(), self.trip_reason);
            false
        }
    }

    /// Trip the manager with the given reason and start the cooldown clock.
    /// Called internally by `should_stop_loss()` / `should_take_profit()`
    /// when a threshold is crossed. Safe to call multiple times — subsequent
    /// calls while already tripped are silently ignored.
    fn trip(&mut self, reason: SlTripReason) {
        if self.tripped_at.is_some() {
            return; // already tripped — don't reset the clock
        }
        error!("🚨 SL MANAGER TRIPPED: {} | cooldown={}s", reason, self.cooldown_secs);
        self.tripped_at  = Some(Instant::now());
        self.trip_reason = Some(reason);
    }

    /// Manually clear the tripped state (also called by `is_trading_allowed()`
    /// after the cooldown expires). Resets the trailing-stop high so the next
    /// `reset_for_new_position()` starts fresh.
    pub fn reset_sl(&mut self) {
        info!("🔄 Resetting stop-loss manager state");
        self.tripped_at  = None;
        self.trip_reason = None;
        self.highest_price = 0.0;
    }

    /// Remaining cooldown duration. `None` when not tripped or already expired.
    pub fn sl_cooldown_remaining(&self) -> Option<Duration> {
        let tripped_at = self.tripped_at?;
        let cooldown   = Duration::from_secs(self.cooldown_secs);
        let elapsed    = tripped_at.elapsed();
        if elapsed < cooldown { Some(cooldown - elapsed) } else { None }
    }

    /// Current trip reason. `None` when not tripped.
    pub fn trip_reason(&self) -> Option<SlTripReason> {
        self.trip_reason
    }

    // ── Predicate checks (unchanged public API from V5.2) ────────────────

    /// Returns true if the position should be closed at a loss.
    /// Also trips the cooldown gate when threshold is crossed.
    pub fn should_stop_loss(&mut self, entry_price: f64, current_price: f64) -> bool {
        if !self.enabled {
            return false;
        }

        if self.trailing_stop && self.highest_price == 0.0 {
            self.highest_price = entry_price;
        }
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
                "   Entry: ${:.4} | Ref: ${:.4} | Current: ${:.4} | Loss: {:.2}% | Threshold: -{:.1}%",
                entry_price, reference_price, current_price, loss_pct, self.stop_loss_pct
            );
            self.trip(SlTripReason::StopLoss);
            return true;
        }

        false
    }

    /// Returns true if the position should be closed at a profit.
    /// Also trips the cooldown gate when threshold is crossed.
    pub fn should_take_profit(&mut self, entry_price: f64, current_price: f64) -> bool {
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
            self.trip(SlTripReason::TakeProfit);
            return true;
        }

        false
    }

    /// Reset for a new position — call before entering each trade.
    pub fn reset(&mut self, entry_price: f64) {
        self.entry_price   = entry_price;
        self.highest_price = entry_price;
    }

    /// Alias for `reset()` — preferred name at call sites for clarity.
    #[inline]
    pub fn reset_for_new_position(&mut self, entry_price: f64) {
        self.reset(entry_price);
    }

    /// Returns the stored entry price (set by the last `reset()` call).
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

    /// True when currently in a cooldown period.
    #[inline]
    pub fn is_tripped(&self) -> bool {
        self.tripped_at.is_some()
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
                vol_floor_resume_pct: 0.05,
                min_orders_to_maintain: 5,
                enable_adaptive_spacing: false,
                enable_smart_position_sizing: false,
                optimizer_interval_cycles: 50,
                fee_filter: FeeFilterConfig::default(),
                signal_size_multiplier: 1.0,
                max_grid_spacing_pct: 0.0075,
                min_grid_spacing_pct: 0.001,
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
                enable_trailing_stop: false,
                stop_loss_cooldown_secs: 300,
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

    fn test_config_short_cooldown() -> Config {
        let mut cfg = test_config();
        cfg.risk.stop_loss_cooldown_secs = 1;
        cfg
    }

    // ── V5.2 field binding tests (unchanged) ─────────────────────────────

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

    // ── Fixed stop tests ──────────────────────────────────────────────────

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

    // ── Trailing stop tests ────────────────────────────────────────────────

    #[test]
    fn test_trailing_stop_config_wired_from_risk_config() {
        let config = test_config_trailing();
        let mgr = StopLossManager::new(&config);
        assert!(mgr.is_trailing());
    }

    #[test]
    fn test_fixed_stop_config_not_trailing() {
        let config = test_config();
        let mgr = StopLossManager::new(&config);
        assert!(!mgr.is_trailing());
    }

    #[test]
    fn test_trailing_stop_ratchets_high() {
        let config = test_config_trailing();
        let mut mgr = StopLossManager::new(&config);
        assert!(!mgr.should_stop_loss(100.0, 120.0));
        assert!(!mgr.should_stop_loss(100.0, 115.0));
        assert_eq!(mgr.highest_observed_price(), 120.0);
    }

    #[test]
    fn test_trailing_stop_triggers_after_ratchet() {
        let config = test_config_trailing();
        let mut mgr = StopLossManager::new(&config);
        assert!(!mgr.should_stop_loss(100.0, 120.0));
        assert!(mgr.should_stop_loss(100.0, 113.0));
    }

    #[test]
    fn test_trailing_stop_does_not_move_down() {
        let config = test_config_trailing();
        let mut mgr = StopLossManager::new(&config);
        mgr.should_stop_loss(100.0, 120.0);
        mgr.should_stop_loss(100.0, 110.0);
        assert_eq!(mgr.highest_observed_price(), 120.0);
    }

    // ── Take-profit tests ─────────────────────────────────────────────────

    #[test]
    fn test_take_profit_triggers_at_threshold() {
        let config = test_config();
        let mut mgr = StopLossManager::new(&config);
        assert!(mgr.should_take_profit(100.0, 110.0));
    }

    #[test]
    fn test_take_profit_does_not_trigger_below_threshold() {
        let config = test_config();
        let mut mgr = StopLossManager::new(&config);
        assert!(!mgr.should_take_profit(100.0, 109.9));
    }

    // ── Reset + entry_price accessor tests ───────────────────────────────

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

    // ── PR #126 C1: cooldown gate tests ───────────────────────────────────

    #[test]
    fn test_not_tripped_initially() {
        let config = test_config();
        let mut mgr = StopLossManager::new(&config);
        assert!(!mgr.is_tripped());
        assert!(mgr.is_trading_allowed());
        assert!(mgr.trip_reason().is_none());
    }

    #[test]
    fn test_stop_loss_trips_cooldown_gate() {
        let config = test_config();
        let mut mgr = StopLossManager::new(&config);
        // Trigger SL
        assert!(mgr.should_stop_loss(100.0, 94.9));
        // Gate must now be closed
        assert!(mgr.is_tripped());
        assert!(!mgr.is_trading_allowed());
        assert_eq!(mgr.trip_reason(), Some(SlTripReason::StopLoss));
    }

    #[test]
    fn test_take_profit_trips_cooldown_gate() {
        let config = test_config();
        let mut mgr = StopLossManager::new(&config);
        assert!(mgr.should_take_profit(100.0, 110.1));
        assert!(mgr.is_tripped());
        assert!(!mgr.is_trading_allowed());
        assert_eq!(mgr.trip_reason(), Some(SlTripReason::TakeProfit));
    }

    #[test]
    fn test_cooldown_auto_resets_after_expiry() {
        // Use a 1-second cooldown so the test doesn't have to sleep 300s.
        let config = test_config_short_cooldown();
        let mut mgr = StopLossManager::new(&config);
        assert!(mgr.should_stop_loss(100.0, 94.9));
        assert!(mgr.is_tripped());
        // Sleep past cooldown
        std::thread::sleep(std::time::Duration::from_millis(1100));
        // is_trading_allowed() must auto-reset
        assert!(mgr.is_trading_allowed(),
            "is_trading_allowed() must return true after cooldown expires");
        assert!(!mgr.is_tripped(), "tripped_at must be cleared after auto-reset");
        assert!(mgr.trip_reason().is_none(), "trip_reason must be None after reset");
    }

    #[test]
    fn test_sl_cooldown_remaining_is_some_when_tripped() {
        let config = test_config();
        let mut mgr = StopLossManager::new(&config);
        mgr.should_stop_loss(100.0, 94.9);
        let remaining = mgr.sl_cooldown_remaining();
        assert!(remaining.is_some(), "cooldown_remaining must be Some while tripped");
        assert!(remaining.unwrap().as_secs() > 290,
            "remaining must be close to 300s on fresh trip, got {:?}", remaining);
    }

    #[test]
    fn test_sl_cooldown_remaining_is_none_when_not_tripped() {
        let config = test_config();
        let mgr = StopLossManager::new(&config);
        assert!(mgr.sl_cooldown_remaining().is_none());
    }

    #[test]
    fn test_double_trip_does_not_reset_clock() {
        // Firing SL twice while already tripped must NOT reset tripped_at
        // (would extend the cooldown indefinitely if it did).
        let config = test_config();
        let mut mgr = StopLossManager::new(&config);
        mgr.should_stop_loss(100.0, 94.9); // first trip
        let first_remaining = mgr.sl_cooldown_remaining().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        mgr.should_stop_loss(100.0, 94.9); // second call while tripped
        let second_remaining = mgr.sl_cooldown_remaining().unwrap();
        // Clock was NOT reset — second_remaining must be <= first_remaining
        assert!(second_remaining <= first_remaining,
            "double-trip must not reset cooldown clock: first={:?} second={:?}",
            first_remaining, second_remaining);
    }

    #[test]
    fn test_reset_sl_clears_trip_state() {
        let config = test_config();
        let mut mgr = StopLossManager::new(&config);
        mgr.should_stop_loss(100.0, 94.9);
        assert!(mgr.is_tripped());
        mgr.reset_sl();
        assert!(!mgr.is_tripped());
        assert!(mgr.trip_reason().is_none());
        assert!(mgr.is_trading_allowed());
    }

    #[test]
    fn test_cooldown_secs_read_from_config() {
        let mut config = test_config();
        config.risk.stop_loss_cooldown_secs = 600;
        let mgr = StopLossManager::new(&config);
        assert_eq!(mgr.cooldown_secs, 600);
    }
}
