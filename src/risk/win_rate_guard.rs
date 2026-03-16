//! ═══════════════════════════════════════════════════════════════════════════
//! 📉 WIN RATE GUARD  (V1.0 — PR #131 C4)
//!
//! V1.0 (PR #131 C4): Initial implementation.
//!   Monitors rolling win-rate and suppresses trading when it drops below a
//!   configurable threshold. Uses a hysteresis band (resume_pct > min_pct) to
//!   prevent rapid oscillation at the boundary — mirrors the vol_floor_resume_pct
//!   pattern used in the regime gate exactly.
//!
//! ## Config source  (`[risk]` section in TOML)
//!
//! | Field                      | Default | Description |
//! |----------------------------|---------|-------------|
//! | `enable_win_rate_guard`    | `false` | Opt-in feature flag |
//! | `min_win_rate_pct`         | `40.0`  | Suppress below this % |
//! | `win_rate_guard_resume_pct`| `45.0`  | Only resume above this % (hysteresis) |
//! | `min_trades_before_guard`  | `10`    | Warmup — ignore guard until N fills |
//!
//! ## Hysteresis band
//!
//! ```text
//! suppress at:  min_win_rate_pct      = 40%
//! resume  at:   win_rate_guard_resume_pct = 45%
//!
//! win rate:  38% → SUPPRESS ← 41% (stays suppressed — below resume band)
//!                              46% → RESUME ← 43% (stays active)
//! ```
//!
//! ## Warmup window
//!
//! The guard is inactive until `total_trades >= min_trades_before_guard`.
//! This prevents a single cold-start loss (e.g. 0/1 = 0%) from silencing the
//! bot before it has a statistically meaningful sample.
//!
//! ## Integration
//! Constructed once per `GridBot` via `WinRateGuard::new(&config)` (C4 Step 2).
//! `process_tick()` calls `evaluate(perf.win_rate, self.successful_trades)` and
//! returns `TickResult::paused` when suppressed.
//!
//! ## Escalation ladder position
//! CB shutdown     → TickResult::shutdown()    🔴  permanent
//! SL/TP cooldown  → TickResult::paused(Xs)    🛑  timed, auto-reset
//! Regime gate     → TickResult::paused(vol)   ⛔  conditional
//! Win Rate Guard  → TickResult::paused(rate%) 📉  conditional  ← this module
//! Normal          → TickResult::active()      ✅
//! ═══════════════════════════════════════════════════════════════════════════

use crate::Config;
use log::{info, warn};

pub struct WinRateGuard {
    /// False → all methods are no-ops (safe default for existing instances).
    enabled: bool,
    /// Suppress trading when win-rate (%) falls below this threshold.
    min_win_rate_pct: f64,
    /// Only resume trading when win-rate (%) rises back above this value.
    /// Must be >= min_win_rate_pct. Prevents boundary oscillation.
    resume_pct: f64,
    /// Number of completed trades required before the guard activates.
    /// Prevents false suppression on cold-start.
    min_trades: u64,
    /// True when the guard has suppressed trading.
    is_suppressed: bool,
    /// Human-readable reason string surfaced in TickResult::paused().
    suppression_reason: String,
}

impl WinRateGuard {
    /// Construct from master config.  Called once per GridBot instance.
    pub fn new(config: &Config) -> Self {
        let enabled       = config.risk.enable_win_rate_guard;
        let min_pct       = config.risk.min_win_rate_pct;
        let resume_pct    = config.risk.win_rate_guard_resume_pct;
        let min_trades    = config.risk.min_trades_before_guard;

        info!("📉 Initializing Win Rate Guard V1.0");
        if enabled {
            info!("   Suppress below:  {:.1}%", min_pct);
            info!("   Resume above:    {:.1}%  (hysteresis band)", resume_pct);
            info!("   Warmup trades:   {}", min_trades);
        } else {
            info!("   Win Rate Guard DISABLED (enable_win_rate_guard = false)");
        }

        Self {
            enabled,
            min_win_rate_pct: min_pct,
            resume_pct,
            min_trades,
            is_suppressed:      false,
            suppression_reason: String::new(),
        }
    }

    /// Evaluate current win-rate and update suppression state.
    ///
    /// # Arguments
    /// * `win_rate_fraction` — win rate as a 0.0–1.0 fraction (matches `perf_stats.win_rate`)
    /// * `total_trades`      — total completed trades (fills) this session
    ///
    /// # Returns
    /// `true`  → trading is allowed
    /// `false` → trading is suppressed (call `reason()` for the pause message)
    pub fn evaluate(&mut self, win_rate_fraction: f64, total_trades: u64) -> bool {
        // Guard disabled — always allow
        if !self.enabled {
            return true;
        }

        // Warmup window — insufficient data, don't gate
        if total_trades < self.min_trades {
            return true;
        }

        let pct = win_rate_fraction * 100.0;

        if self.is_suppressed {
            // Only resume once we clear the hysteresis band
            if pct >= self.resume_pct {
                info!(
                    "✅ Win Rate Guard: recovered to {:.1}% (resume threshold {:.1}%) — resuming",
                    pct, self.resume_pct
                );
                self.is_suppressed      = false;
                self.suppression_reason = String::new();
                true
            } else {
                // Still suppressed — refresh reason string with latest rate
                self.suppression_reason = format!(
                    "{:.1}% win rate below {:.1}% resume threshold ({} trades)",
                    pct, self.resume_pct, total_trades
                );
                warn!("📉 Win Rate Guard: suppressed — {}", self.suppression_reason);
                false
            }
        } else {
            // Not suppressed — check if we should suppress
            if pct < self.min_win_rate_pct {
                self.suppression_reason = format!(
                    "{:.1}% win rate below {:.1}% minimum ({} trades)",
                    pct, self.min_win_rate_pct, total_trades
                );
                warn!("📉 Win Rate Guard TRIGGERED: {}", self.suppression_reason);
                self.is_suppressed = true;
                false
            } else {
                true
            }
        }
    }

    /// Returns `true` when trading is currently allowed (not suppressed).
    #[inline]
    pub fn is_trading_allowed(&self) -> bool {
        !self.is_suppressed
    }

    /// Human-readable suppression reason. Empty string when not suppressed.
    #[inline]
    pub fn reason(&self) -> &str {
        &self.suppression_reason
    }

    /// True when the guard is currently suppressing trades.
    #[inline]
    pub fn is_suppressed(&self) -> bool {
        self.is_suppressed
    }

    /// True when the guard is enabled in config.
    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.enabled
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
                vol_floor_resume_pct: 0.05,
                enable_order_lifecycle: false,
                order_max_age_minutes: 60,
                order_refresh_interval_minutes: 30,
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
                enable_win_rate_guard: true,
                min_win_rate_pct: 40.0,
                win_rate_guard_resume_pct: 45.0,
                min_trades_before_guard: 10,
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

    fn disabled_config() -> Config {
        let mut cfg = test_config();
        cfg.risk.enable_win_rate_guard = false;
        cfg
    }

    // ── Disabled guard tests ──────────────────────────────────────────────

    #[test]
    fn test_disabled_guard_always_allows() {
        let config = disabled_config();
        let mut guard = WinRateGuard::new(&config);
        // Even 0% win rate with 1000 trades must be allowed when disabled
        assert!(guard.evaluate(0.0, 1000));
        assert!(guard.is_trading_allowed());
        assert!(!guard.is_suppressed());
    }

    // ── Warmup window tests ───────────────────────────────────────────────

    #[test]
    fn test_warmup_window_allows_below_threshold() {
        let config = test_config(); // min_trades = 10
        let mut guard = WinRateGuard::new(&config);
        // 0% win rate but only 9 trades — still in warmup
        assert!(guard.evaluate(0.0, 9), "warmup window must allow all trades");
        assert!(!guard.is_suppressed());
    }

    #[test]
    fn test_warmup_boundary_exactly_min_trades_activates_guard() {
        let config = test_config(); // min_trades = 10, min_pct = 40.0
        let mut guard = WinRateGuard::new(&config);
        // Exactly 10 trades, 0% win rate — guard should now fire
        assert!(!guard.evaluate(0.0, 10), "guard must activate at min_trades boundary");
        assert!(guard.is_suppressed());
    }

    // ── Suppression tests ─────────────────────────────────────────────────

    #[test]
    fn test_suppresses_below_min_win_rate() {
        let config = test_config(); // min_pct = 40.0
        let mut guard = WinRateGuard::new(&config);
        assert!(!guard.evaluate(0.35, 20), "39.9% should suppress");
        assert!(guard.is_suppressed());
    }

    #[test]
    fn test_does_not_suppress_at_min_win_rate() {
        let config = test_config(); // min_pct = 40.0
        let mut guard = WinRateGuard::new(&config);
        assert!(guard.evaluate(0.40, 20), "exactly 40% should NOT suppress");
        assert!(!guard.is_suppressed());
    }

    #[test]
    fn test_does_not_suppress_above_threshold() {
        let config = test_config();
        let mut guard = WinRateGuard::new(&config);
        assert!(guard.evaluate(0.60, 20));
        assert!(!guard.is_suppressed());
    }

    // ── Hysteresis band tests ─────────────────────────────────────────────

    #[test]
    fn test_hysteresis_no_premature_resume_below_resume_pct() {
        let config = test_config(); // min=40%, resume=45%
        let mut guard = WinRateGuard::new(&config);
        // Trigger suppression
        assert!(!guard.evaluate(0.35, 20));
        assert!(guard.is_suppressed());
        // Win rate recovers to 41% — still below resume band (45%)
        assert!(!guard.evaluate(0.41, 21),
            "41% is above suppress but below resume — must stay suppressed");
        assert!(guard.is_suppressed(),
            "hysteresis: must NOT resume until above resume_pct");
    }

    #[test]
    fn test_hysteresis_resumes_at_resume_pct() {
        let config = test_config(); // resume = 45.0
        let mut guard = WinRateGuard::new(&config);
        assert!(!guard.evaluate(0.35, 20));
        assert!(guard.is_suppressed());
        // Win rate reaches exactly 45%
        assert!(guard.evaluate(0.45, 25),
            "exactly 45% must trigger resume");
        assert!(!guard.is_suppressed(),
            "is_suppressed must clear after resume");
    }

    #[test]
    fn test_hysteresis_resumes_above_resume_pct() {
        let config = test_config();
        let mut guard = WinRateGuard::new(&config);
        assert!(!guard.evaluate(0.35, 20));
        assert!(guard.evaluate(0.60, 30), "60% well above resume band — must allow");
        assert!(!guard.is_suppressed());
    }

    #[test]
    fn test_can_re_suppress_after_resume() {
        let config = test_config();
        let mut guard = WinRateGuard::new(&config);
        // Suppress → resume → suppress again
        assert!(!guard.evaluate(0.35, 20)); // suppress
        assert!(guard.evaluate(0.50, 30));  // resume
        assert!(!guard.is_suppressed());
        assert!(!guard.evaluate(0.30, 40)); // suppress again
        assert!(guard.is_suppressed());
    }

    // ── Reason string tests ───────────────────────────────────────────────

    #[test]
    fn test_reason_empty_when_not_suppressed() {
        let config = test_config();
        let guard = WinRateGuard::new(&config);
        assert!(guard.reason().is_empty(),
            "reason must be empty when not suppressed");
    }

    #[test]
    fn test_reason_non_empty_when_suppressed() {
        let config = test_config();
        let mut guard = WinRateGuard::new(&config);
        assert!(!guard.evaluate(0.35, 20));
        assert!(!guard.reason().is_empty(),
            "reason must be non-empty when suppressed");
    }

    #[test]
    fn test_reason_cleared_after_resume() {
        let config = test_config();
        let mut guard = WinRateGuard::new(&config);
        assert!(!guard.evaluate(0.35, 20));
        assert!(!guard.reason().is_empty());
        assert!(guard.evaluate(0.50, 30));
        assert!(guard.reason().is_empty(),
            "reason must clear after resume");
    }

    // ── Config binding tests ──────────────────────────────────────────────

    #[test]
    fn test_reads_correct_config_fields() {
        let config = test_config();
        let guard = WinRateGuard::new(&config);
        assert!(guard.is_enabled());
        assert!((config.risk.min_win_rate_pct - 40.0).abs() < 1e-9);
        assert!((config.risk.win_rate_guard_resume_pct - 45.0).abs() < 1e-9);
        assert_eq!(config.risk.min_trades_before_guard, 10);
    }
}
