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
            max_consecutive_losses: 5, // Hardcoded for now, can be config later
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
    
    /// Create with custom initial balance
    pub fn with_balance(config: &Config, initial_balance: f64) -> Self {
        let mut breaker = Self::new(config);
        breaker.peak_balance = initial_balance;
        breaker
    }
    
    /// Check if trading is allowed
    pub fn is_trading_allowed(&mut self) -> bool {
        // Check if cooldown period has passed
        if self.is_tripped {
            if let Some(trip_time) = self.trip_time {
                let elapsed = trip_time.elapsed();
                
                if elapsed >= self.cooldown_duration {
                    info!("✅ Circuit breaker cooldown complete - resuming trading");
                    self.reset();
                    return true;
                } else {
                    let remaining = self.cooldown_duration - elapsed;
                    
                    // Log every 60 seconds
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
    
    /// Record a trade result and update balance
    pub fn record_trade(&mut self, pnl: f64, new_balance: f64) {
        self.daily_pnl += pnl;
        self.last_trade_time = Some(Instant::now());
        
        // Update peak balance and calculate drawdown
        if new_balance > self.peak_balance {
            self.peak_balance = new_balance;
            self.current_drawdown_pct = 0.0;
        } else {
            self.current_drawdown_pct = ((self.peak_balance - new_balance) / self.peak_balance) * 100.0;
        }
        
        // Track consecutive losses
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
        
        // Check all trip conditions
        self.check_trip_conditions();
    }
    
    /// Check if any trip conditions are met
    fn check_trip_conditions(&mut self) {
        if self.is_tripped {
            return; // Already tripped
        }
        
        // Check 1: Daily loss limit
        if self.daily_pnl.abs() >= self.max_daily_loss_pct {
            error!("🚨 CIRCUIT BREAKER TRIPPED - Daily loss limit exceeded!");
            error!("   Daily P&L:    {:.2}%", self.daily_pnl);
            error!("   Limit:        -{:.1}%", self.max_daily_loss_pct);
            self.trip(TripReason::DailyLossLimit);
            return;
        }
        
        // Check 2: Maximum drawdown
        if self.current_drawdown_pct >= self.max_drawdown_pct {
            error!("🚨 CIRCUIT BREAKER TRIPPED - Maximum drawdown exceeded!");
            error!("   Current DD:   -{:.2}%", self.current_drawdown_pct);
            error!("   Max allowed:  -{:.1}%", self.max_drawdown_pct);
            error!("   Peak balance: ${:.2}", self.peak_balance);
            self.trip(TripReason::MaxDrawdown);
            return;
        }
        
        // Check 3: Consecutive losses
        if self.consecutive_losses >= self.max_consecutive_losses {
            error!("🚨 CIRCUIT BREAKER TRIPPED - Too many consecutive losses!");
            error!("   Consecutive:  {}", self.consecutive_losses);
            error!("   Max allowed:  {}", self.max_consecutive_losses);
            self.trip(TripReason::ConsecutiveLosses);
            return;
        }
    }
    
    /// Trip the circuit breaker
    fn trip(&mut self, reason: TripReason) {
        self.is_tripped = true;
        self.trip_reason = Some(reason);
        self.trip_time = Some(Instant::now());
        
        error!("⚠️  ═══════════════════════════════════════════════════════");
        error!("⚠️  ALL TRADING HALTED FOR {}s", self.cooldown_duration.as_secs());
        error!("⚠️  Reason: {:?}", reason);
        error!("⚠️  ═══════════════════════════════════════════════════════");
    }
    
    /// Reset circuit breaker after cooldown
    fn reset(&mut self) {
        info!("🔄 Resetting circuit breaker state");
        self.is_tripped = false;
        self.trip_reason = None;
        self.trip_time = None;
        self.consecutive_losses = 0;
    }
    
    /// Reset daily statistics (call at start of new trading day)
    pub fn reset_daily(&mut self) {
        info!("📅 Resetting daily circuit breaker statistics");
        info!("   Final daily P&L:     {:.2}%", self.daily_pnl);
        info!("   Consecutive losses:  {}", self.consecutive_losses);
        info!("   Current drawdown:    -{:.2}%", self.current_drawdown_pct);
        
        self.daily_pnl = 0.0;
        
        // Don't reset consecutive losses - they carry over
        // Don't reset drawdown - it's cumulative from peak
    }
    
    /// Force trip the circuit breaker (for testing/emergency)
    pub fn force_trip(&mut self, reason: TripReason) {
        warn!("🚨 Manual circuit breaker trip triggered!");
        self.trip(reason);
    }
    
    /// Get current status
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

/// Circuit breaker status snapshot
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

    fn test_config() -> Config {
        Config {
            bot: BotConfig {
                name: "Test".to_string(),
                version: "1.0".to_string(),
                environment: "test".to_string(),
            },
            network: NetworkConfig {
                cluster: "devnet".to_string(),
                rpc_url: "http://localhost".to_string(),
                commitment: "confirmed".to_string(),
            },
            trading: TradingConfig {
                grid_levels: 10,
                grid_spacing_percent: 0.2,
                min_order_size: 0.01,
                max_position_size: 1.0,
                enable_auto_rebalance: true,
                min_usdc_reserve: 100.0,
                min_sol_reserve: 0.1,
                ..Default::default()
            },
            strategies: StrategiesConfig::default(),
            risk: RiskConfig {
                max_position_size_pct: 80.0,
                max_drawdown_pct: 10.0,
                stop_loss_pct: 5.0,
                take_profit_pct: 10.0,
                enable_circuit_breaker: true,
                circuit_breaker_threshold_pct: 15.0,
                circuit_breaker_cooldown_secs: 60,
            },
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
        
        // Record losses
        for _ in 0..4 {
            breaker.record_trade(-100.0, 9900.0);
            assert!(!breaker.is_tripped);
        }
        
        // 5th loss should trip
        breaker.record_trade(-100.0, 9800.0);
        assert!(breaker.is_tripped);
    }

    #[test]
    fn test_profit_resets_streak() {
        let config = test_config();
        let mut breaker = CircuitBreaker::with_balance(&config, 10000.0);
        
        // 3 losses
        for _ in 0..3 {
            breaker.record_trade(-50.0, 9900.0);
        }
        
        assert_eq!(breaker.consecutive_losses, 3);
        
        // 1 profit resets streak
        breaker.record_trade(100.0, 10000.0);
        assert_eq!(breaker.consecutive_losses, 0);
    }
}
