//! ═══════════════════════════════════════════════════════════════════════════
//! ⚠️  CIRCUIT BREAKER - Emergency Stop System V3.0
//!
//! Automatic trading halt when:
//! • Daily loss exceeds maximum drawdown limit
//! • Maximum drawdown threshold is breached
//! • Consecutive losses mount up
//!
//! Features:
//! • Configurable cooldown period
//! • Automatic reset after cooldown
//! • Daily statistics tracking
//! • Real-time trip detection
//!
//! Safety First! 🛡️
//! ═══════════════════════════════════════════════════════════════════════════

use crate::Config;
use std::time::{Duration, Instant};
use log::{error, warn, info};

/// Circuit breaker for emergency trading halt
#[derive(Debug)]
pub struct CircuitBreaker {
    // Configuration
    max_daily_loss_pct: f64,
    max_drawdown_pct: f64,
    max_consecutive_losses: u32,
    cooldown_duration: Duration,

    // State tracking
    consecutive_losses: u32,
    daily_pnl: f64,
    peak_balance: f64,
    current_drawdown_pct: f64,
    /// Timestamp of the last executed trade.
    /// Set per-trade; reserved for future inactivity detection and
    /// latency analytics — not yet consumed by any read path.
    #[allow(dead_code)]
    last_trade_time: Option<Instant>,

    // Circuit breaker state
    is_tripped: bool,
    trip_reason: Option<TripReason>,
    trip_time: Option<Instant>,
}

/// Reason why circuit breaker was tripped
#[derive(Debug, Clone, Copy)]
pub enum TripReason {
    DailyLossLimit,
    MaxDrawdown,
    ConsecutiveLosses,
}

impl CircuitBreaker {
    /// Create new circuit breaker from config
    pub fn new(config: &Config) -> Self {
        info!("⚠️  Initializing Circuit Breaker V3.0");
        info!("   Max drawdown:          -{:.1}%", config.risk.max_drawdown_pct);
        info!("   Circuit threshold:     -{:.1}%", config.risk.circuit_breaker_threshold_pct);
        info!("   Cooldown period:       {}s", config.risk.circuit_breaker_cooldown_secs);
        info!("   Max consecutive loss:  5 trades");

        Self {
            max_daily_loss_pct: config.risk.max_drawdown_pct,
            max_drawdown_pct: config.risk.circuit_breaker_threshold_pct,
            max_consecutive_losses: 5,
            cooldown_duration: Duration::from_secs(config.risk.circuit_breaker_cooldown_secs),

            consecutive_losses: 0,
            daily_pnl: 0.0,
            peak_balance: 0.0,
            current_drawdown_pct: 0.0,
            last_trade_time: None,

            is_tripped: false,
            trip_reason: None,
            trip_time: None,
        }
    }

    /// Create with custom initial balance (full portfolio NAV in USD).
    /// Pass `initial_usdc + (initial_sol * sol_price_usd)` — NOT just USDC.
    pub fn with_balance(config: &Config, initial_balance: f64) -> Self {
        let mut breaker = Self::new(config);
        breaker.peak_balance = initial_balance;
        breaker
    }

    /// Check if trading is allowed
    pub fn is_trading_allowed(&mut self) -> bool {
        if self.is_tripped {
            if let Some(trip_time) = self.trip_time {
                let elapsed = trip_time.elapsed();

                if elapsed >= self.cooldown_duration {
                    info!("✅ Circuit breaker cooldown complete - resuming trading");
                    self.reset();
                    return true;
                } else {
                    let remaining = self.cooldown_duration - elapsed;

                    if elapsed.as_secs() % 60 == 0 {
                        warn!("⏸️  Circuit breaker active - {}s remaining", remaining.as_secs());
                        if let Some(reason) = self.trip_reason {
                            warn!("   Reason: {:?}", reason);
                        }
                    }

                    return false;
                }
            }
        }

        true
    }

    /// Record a trade result and update balance.
    ///
    /// No-ops when the breaker is already tripped — prevents the consecutive-
    /// loss counter from climbing past the threshold and generating log spam.
    /// State is cleanly reset inside `is_trading_allowed()` once the cooldown
    /// expires, so calling code must tick `is_trading_allowed()` each cycle.
    pub fn record_trade(&mut self, pnl: f64, new_balance: f64) {
        if self.is_tripped {
            return;
        }

        self.daily_pnl += pnl;
        self.last_trade_time = Some(Instant::now());

        if new_balance > self.peak_balance {
            self.peak_balance = new_balance;
            self.current_drawdown_pct = 0.0;
        } else {
            self.current_drawdown_pct = ((self.peak_balance - new_balance) / self.peak_balance) * 100.0;
        }

        if pnl < 0.0 {
            self.consecutive_losses += 1;
            warn!("📉 Loss recorded - consecutive: {}/{}",
                self.consecutive_losses, self.max_consecutive_losses);
        } else if pnl > 0.0 {
            if self.consecutive_losses > 0 {
                info!("✅ Profit recorded - consecutive loss streak broken");
            }
            self.consecutive_losses = 0;
        }

        self.check_trip_conditions();
    }

    fn check_trip_conditions(&mut self) {
        if self.is_tripped {
            return;
        }

        if self.peak_balance > 0.0 {
            let daily_loss_pct = (-self.daily_pnl / self.peak_balance) * 100.0;
            if daily_loss_pct >= self.max_daily_loss_pct {
                error!("🚨 CIRCUIT BREAKER TRIPPED - Daily loss limit exceeded!");
                error!("   Daily loss:   -{:.2}%", daily_loss_pct);
                error!("   Limit:        -{:.1}%", self.max_daily_loss_pct);
                self.trip(TripReason::DailyLossLimit);
                return;
            }
        }

        if self.current_drawdown_pct >= self.max_drawdown_pct {
            error!("🚨 CIRCUIT BREAKER TRIPPED - Maximum drawdown exceeded!");
            error!("   Current DD:   -{:.2}%", self.current_drawdown_pct);
            error!("   Max allowed:  -{:.1}%", self.max_drawdown_pct);
            error!("   Peak balance: ${:.2}", self.peak_balance);
            self.trip(TripReason::MaxDrawdown);
            return;
        }

        if self.consecutive_losses >= self.max_consecutive_losses {
            error!("🚨 CIRCUIT BREAKER TRIPPED - Too many consecutive losses!");
            error!("   Consecutive:  {}", self.consecutive_losses);
            error!("   Max allowed:  {}", self.max_consecutive_losses);
            self.trip(TripReason::ConsecutiveLosses);
            return;
        }
    }

    fn trip(&mut self, reason: TripReason) {
        self.is_tripped = true;
        self.trip_reason = Some(reason);
        self.trip_time = Some(Instant::now());

        error!("⚠️  ═══════════════════════════════════════════════════════");
        error!("⚠️  ALL TRADING HALTED FOR {}s", self.cooldown_duration.as_secs());
        error!("⚠️  Reason: {:?}", reason);
        error!("⚠️  ═══════════════════════════════════════════════════════");
    }

    fn reset(&mut self) {
        info!("🔄 Resetting circuit breaker state");
        self.is_tripped = false;
        self.trip_reason = None;
        self.trip_time = None;
        self.consecutive_losses = 0;
    }

    pub fn reset_daily(&mut self) {
        info!("📅 Resetting daily circuit breaker statistics");
        info!("   Final daily P&L:     {:.2}%",
            if self.peak_balance > 0.0 { (self.daily_pnl / self.peak_balance) * 100.0 } else { 0.0 });
        info!("   Consecutive losses:  {}", self.consecutive_losses);
        info!("   Current drawdown:    -{:.2}%", self.current_drawdown_pct);

        self.daily_pnl = 0.0;
    }

    pub fn force_trip(&mut self, reason: TripReason) {
        warn!("🚨 Manual circuit breaker trip triggered!");
        self.trip(reason);
    }

    pub fn status(&self) -> CircuitBreakerStatus {
        CircuitBreakerStatus {
            is_tripped: self.is_tripped,
            trip_reason: self.trip_reason,
            consecutive_losses: self.consecutive_losses,
            daily_pnl: self.daily_pnl,
            current_drawdown_pct: self.current_drawdown_pct,
            cooldown_remaining: self.trip_time.map(|t| {
                let elapsed = t.elapsed();
                if elapsed < self.cooldown_duration {
                    self.cooldown_duration - elapsed
                } else {
                    Duration::ZERO
                }
            }),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CircuitBreakerStatus {
    pub is_tripped: bool,
    pub trip_reason: Option<TripReason>,
    pub consecutive_losses: u32,
    pub daily_pnl: f64,
    pub current_drawdown_pct: f64,
    pub cooldown_remaining: Option<Duration>,
}

impl std::fmt::Display for TripReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TripReason::DailyLossLimit => write!(f, "Daily Loss Limit Exceeded"),
            TripReason::MaxDrawdown => write!(f, "Maximum Drawdown Exceeded"),
            TripReason::ConsecutiveLosses => write!(f, "Too Many Consecutive Losses"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::*;

    /// Canonical test config — must stay in sync with the Config struct.
    /// When new fields are added to Config / BotConfig / etc., add them here
    /// with a sensible default so circuit breaker tests keep compiling.
    ///
    /// BotConfig field types (from config/mod.rs):
    ///   execution_mode: String  ("paper" | "live") — NOT an enum
    ///   instance_id:    Option<String>             — NOT plain String
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
            pyth: PythConfig::default(),
            performance: PerformanceConfig::default(),
            logging: LoggingConfig::default(),
            metrics: MetricsConfig::default(),
            paper_trading: PaperTradingConfig::default(),
            database: DatabaseConfig::default(),
            alerts: AlertsConfig::default(),
        }
    }

    #[test]
    fn test_circuit_breaker_creation() {
        let config = test_config();
        let breaker = CircuitBreaker::new(&config);

        assert!(!breaker.is_tripped);
        assert_eq!(breaker.consecutive_losses, 0);
        assert_eq!(breaker.daily_pnl, 0.0);
    }

    #[test]
    fn test_consecutive_losses() {
        let config = test_config();
        let mut breaker = CircuitBreaker::with_balance(&config, 10000.0);

        // Record 4 losses — each is 1% of balance, total 4% < 10% daily limit
        // so only consecutive-loss check is relevant here
        for _ in 0..4 {
            breaker.record_trade(-100.0, 9900.0);
            assert!(!breaker.is_tripped);
        }

        // 5th loss hits max_consecutive_losses (5) → trip
        breaker.record_trade(-100.0, 9800.0);
        assert!(breaker.is_tripped);
    }

    #[test]
    fn test_record_trade_noop_when_tripped() {
        let config = test_config();
        let mut breaker = CircuitBreaker::with_balance(&config, 10000.0);

        // Trip via consecutive losses
        for _ in 0..5 {
            breaker.record_trade(-100.0, 9900.0);
        }
        assert!(breaker.is_tripped);
        let count_at_trip = breaker.consecutive_losses;

        // Further calls must not increment the counter
        breaker.record_trade(-100.0, 9800.0);
        breaker.record_trade(-100.0, 9700.0);
        assert_eq!(breaker.consecutive_losses, count_at_trip,
            "consecutive_losses must not grow after trip");
    }

    #[test]
    fn test_profit_resets_streak() {
        let config = test_config();
        let mut breaker = CircuitBreaker::with_balance(&config, 10000.0);

        for _ in 0..3 {
            breaker.record_trade(-50.0, 9900.0);
        }

        assert_eq!(breaker.consecutive_losses, 3);

        breaker.record_trade(100.0, 10000.0);
        assert_eq!(breaker.consecutive_losses, 0);
    }

    #[test]
    fn test_daily_loss_limit_fires_as_percentage() {
        let config = test_config(); // max_drawdown_pct = 10.0
        let mut breaker = CircuitBreaker::with_balance(&config, 1000.0);

        // 9% loss — should NOT trip (9 < 10)
        breaker.record_trade(-90.0, 910.0);
        assert!(!breaker.is_tripped, "9% loss should not trip 10% daily limit");

        // Another 2% loss — total 11% — should trip
        breaker.record_trade(-20.0, 890.0);
        assert!(breaker.is_tripped, "11% total daily loss should trip 10% limit");
    }
}
