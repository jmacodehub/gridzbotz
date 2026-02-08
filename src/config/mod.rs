//! ═══════════════════════════════════════════════════════════════════════════
//! 🎛️ UNIFIED CONFIGURATION SYSTEM V4.1 - PROJECT FLASH
//!
//! Single source of truth for ALL bot settings with best practices:
//!
//! V4.1 ENHANCEMENTS - RegimeGate Analytics Bridge:
//! ✅ RegimeGateConfig for analytics module compatibility
//! ✅ BPS (basis points) conversion from percentage format
//! ✅ Type-safe bridging between config systems
//!
//! V3.5 ENHANCEMENTS - Production-Grade Architecture:
//! ✅ Environment-Aware Defaults (testing, dev, production)
//! ✅ Comprehensive Validation with Clear Error Messages
//! ✅ Builder Pattern for Programmatic Construction
//! ✅ Config Presets for Common Scenarios
//! ✅ Hot-Reload Support (future)
//! ✅ Type-Safe with Strong Validation
//! ✅ Multiple Duration Formats (hours, minutes, seconds, cycles)
//! ✅ Zero Hardcoded Values - 100% Configurable
//!
//! Architecture:
//! • `Config` - Main TOML-based configuration
//! • `ConfigBuilder` - Programmatic builder for tests
//! • `ConfigPresets` - Pre-configured scenarios (conservative, aggressive, etc.)
//! • Environment-specific overrides
//! • Comprehensive validation
//!
//! February 7, 2026 - V4.1 ANALYTICS BRIDGE ADDED 🚀
//! ═══════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use anyhow::{Result, Context, bail};
use std::path::{Path};
use std::fs;
use log::{info, warn};

// ═══════════════════════════════════════════════════════════════════════════
// MAIN CONFIGURATION - The Heart of Project Flash
// ═══════════════════════════════════════════════════════════════════════════

/// Master Configuration Structure
///
/// This is the single source of truth for all bot behavior.
/// Every setting can be customized via TOML files.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    /// Bot metadata and environment
    pub bot: BotConfig,

    /// Network and blockchain settings
    pub network: NetworkConfig,

    /// Core trading configuration
    pub trading: TradingConfig,

    /// Strategy settings
    pub strategies: StrategiesConfig,

    /// Risk management rules
    pub risk: RiskConfig,

    /// Price feed configuration
    #[serde(default)]
    pub pyth: PythConfig,

    /// Performance tuning
    #[serde(default)]
    pub performance: PerformanceConfig,

    /// Logging configuration
    #[serde(default)]
    pub logging: LoggingConfig,

    /// Metrics and monitoring
    #[serde(default)]
    pub metrics: MetricsConfig,

    /// Paper trading settings
    #[serde(default)]
    pub paper_trading: PaperTradingConfig,

    /// Database configuration
    #[serde(default)]
    pub database: DatabaseConfig,

    /// Alert system
    #[serde(default)]
    pub alerts: AlertsConfig,
}

// ═══════════════════════════════════════════════════════════════════════════
// BOT CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BotConfig {
    /// Bot name (e.g., "GridBot-Master-v3")
    pub name: String,

    /// Bot version (e.g., "3.5.0")
    pub version: String,

    /// Environment: "testing", "development", "production"
    /// This controls safety features and default behaviors
    pub environment: String,
}

impl BotConfig {
    pub fn is_production(&self) -> bool {
        self.environment == "production"
    }

    pub fn is_testing(&self) -> bool {
        self.environment == "testing"
    }

    pub fn is_development(&self) -> bool {
        self.environment == "development"
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// NETWORK CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NetworkConfig {
    /// Solana cluster: "devnet", "testnet", "mainnet-beta"
    pub cluster: String,

    /// RPC endpoint URL
    pub rpc_url: String,

    /// Commitment level: "processed", "confirmed", "finalized"
    pub commitment: String,

    /// Optional WebSocket URL for subscriptions
    #[serde(default)]
    pub ws_url: Option<String>,
}

impl NetworkConfig {
    pub fn validate(&self) -> Result<()> {
        let valid_clusters = ["devnet", "testnet", "mainnet-beta"];
        if !valid_clusters.contains(&self.cluster.as_str()) {
            bail!("Invalid cluster '{}'. Must be one of: {:?}",
                  self.cluster, valid_clusters);
        }

        let valid_commitments = ["processed", "confirmed", "finalized"];
        if !valid_commitments.contains(&self.commitment.as_str()) {
            bail!("Invalid commitment '{}'. Must be one of: {:?}",
                  self.commitment, valid_commitments);
        }

        if self.cluster == "mainnet-beta" {
            warn!("⚠️ MAINNET CLUSTER DETECTED - Use with caution!");
        }

        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TRADING CONFIGURATION - V3.5 ENHANCED! 🔥
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TradingConfig {
    // ─────────────────────────────────────────────────────────────────────────
    // Core Grid Settings (Required)
    // ─────────────────────────────────────────────────────────────────────────

    /// Number of grid levels (e.g., 35)
    pub grid_levels: u32,

    /// Grid spacing as percentage (e.g., 0.15 = 0.15%)
    pub grid_spacing_percent: f64,

    /// Minimum order size in SOL
    pub min_order_size: f64,

    /// Maximum position size in SOL
    pub max_position_size: f64,

    /// Minimum USDC balance to maintain
    pub min_usdc_reserve: f64,

    /// Minimum SOL balance to maintain
    pub min_sol_reserve: f64,

    // ─────────────────────────────────────────────────────────────────────────
    // Dynamic Grid Features (V2.0+)
    // ─────────────────────────────────────────────────────────────────────────

    /// Enable dynamic grid spacing based on volatility
    #[serde(default)]
    pub enable_dynamic_grid: bool,

    /// Price change % to trigger grid repositioning
    #[serde(default = "default_reposition_threshold")]
    pub reposition_threshold: f64,

    /// Volatility calculation window (cycles)
    #[serde(default = "default_volatility_window")]
    pub volatility_window: u32,

    // ─────────────────────────────────────────────────────────────────────────
    // Auto-Rebalancing
    // ─────────────────────────────────────────────────────────────────────────

    /// Enable automatic grid rebalancing
    #[serde(default = "default_true")]
    pub enable_auto_rebalance: bool,

    /// Enable smart rebalancing (ML-enhanced)
    #[serde(default = "default_true")]
    pub enable_smart_rebalance: bool,

    /// Portfolio imbalance % to trigger rebalance
    #[serde(default = "default_rebalance_threshold")]
    pub rebalance_threshold_pct: f64,

    /// Cooldown between rebalances (seconds)
    #[serde(default = "default_cooldown")]
    pub rebalance_cooldown_secs: u64,

    // ─────────────────────────────────────────────────────────────────────────
    // Order Management
    // ─────────────────────────────────────────────────────────────────────────

    /// Maximum orders per side (buy/sell)
    #[serde(default = "default_max_orders")]
    pub max_orders_per_side: u32,

    /// Order refresh interval (seconds)
    #[serde(default = "default_refresh_interval")]
    pub order_refresh_interval_secs: u64,

    /// Allow market orders (vs limit only)
    #[serde(default)]
    pub enable_market_orders: bool,

    /// Enable fee optimization
    #[serde(default = "default_true")]
    pub enable_fee_optimization: bool,

    // ─────────────────────────────────────────────────────────────────────────
    // Risk Limits
    // ─────────────────────────────────────────────────────────────────────────

    /// Minimum profit threshold % to place orders
    #[serde(default = "default_profit_threshold")]
    pub min_profit_threshold_pct: f64,

    /// Maximum allowed slippage %
    #[serde(default = "default_slippage")]
    pub max_slippage_pct: f64,

    // ─────────────────────────────────────────────────────────────────────────
    // Price Bounds (Optional Safety)
    // ─────────────────────────────────────────────────────────────────────────

    /// Enable price bounds checking
    #[serde(default)]
    pub enable_price_bounds: bool,

    /// Lower price bound (USD)
    #[serde(default = "default_lower_bound")]
    pub lower_price_bound: f64,

    /// Upper price bound (USD)
    #[serde(default = "default_upper_bound")]
    pub upper_price_bound: f64,

    // ─────────────────────────────────────────────────────────────────────────
    // V3.0+: MARKET REGIME GATE 🚫 (100% Config-Driven!)
    // ─────────────────────────────────────────────────────────────────────────

    /// 🔥 CRITICAL: Enable/disable market regime gate
    /// - true: Respects min_volatility_to_trade threshold
    /// - false: Trades in ANY market condition (testing mode)
    #[serde(default = "default_true")]
    pub enable_regime_gate: bool,

    /// 🔥 CRITICAL: Minimum volatility required to trade
    /// - 0.0: No threshold (trades always)
    /// - 0.1: Very permissive (testing)
    /// - 0.3: Moderate (development)
    /// - 0.5: Conservative (production)
    #[serde(default = "default_min_volatility")]
    pub min_volatility_to_trade: f64,

    /// Pause trading when VERY_LOW_VOL regime detected
    #[serde(default = "default_true")]
    pub pause_in_very_low_vol: bool,

    // ─────────────────────────────────────────────────────────────────────────
    // V3.0+: ORDER LIFECYCLE MANAGEMENT 🔄
    // ─────────────────────────────────────────────────────────────────────────

    /// Enable automatic order lifecycle management
    #[serde(default = "default_true")]
    pub enable_order_lifecycle: bool,

    /// Maximum age before refreshing orders (minutes)
    #[serde(default = "default_order_max_age")]
    pub order_max_age_minutes: u64,

    /// Interval between lifecycle checks (minutes)
    #[serde(default = "default_lifecycle_check")]
    pub order_refresh_interval_minutes: u64,

    /// Minimum number of active orders to maintain
    #[serde(default = "default_min_orders")]
    pub min_orders_to_maintain: usize,

    // ─────────────────────────────────────────────────────────────────────────
    // V3.5+: ADVANCED FEATURES 🚀
    // ─────────────────────────────────────────────────────────────────────────

    /// Enable adaptive grid spacing (volatility-based)
    #[serde(default)]
    pub enable_adaptive_spacing: bool,

    /// Enable smart position sizing (confidence-based)
    #[serde(default)]
    pub enable_smart_position_sizing: bool,
}

impl TradingConfig {
    /// Comprehensive validation with helpful error messages
    pub fn validate(&self) -> Result<()> {
        // Grid levels validation
        if self.grid_levels < 2 {
            bail!("grid_levels must be at least 2 (current: {})", self.grid_levels);
        }
        if self.grid_levels > 100 {
            warn!("⚠️ Very high grid_levels ({}) - may cause performance issues",
                  self.grid_levels);
        }

        // Grid spacing validation
        if self.grid_spacing_percent <= 0.0 {
            bail!("grid_spacing_percent must be positive (current: {})",
                  self.grid_spacing_percent);
        }
        if self.grid_spacing_percent > 10.0 {
            warn!("⚠️ Very wide grid spacing ({:.2}%) - trades may be infrequent",
                  self.grid_spacing_percent);
        }
        if self.grid_spacing_percent < 0.05 {
            warn!("⚠️ Very tight grid spacing ({:.2}%) - may not profit after fees",
                  self.grid_spacing_percent);
        }

        // Order size validation
        if self.min_order_size <= 0.0 {
            bail!("min_order_size must be positive");
        }
        if self.max_position_size <= self.min_order_size {
            bail!("max_position_size must be > min_order_size");
        }

        // Reserve validation
        if self.min_usdc_reserve < 0.0 {
            bail!("min_usdc_reserve cannot be negative");
        }
        if self.min_sol_reserve < 0.0 {
            bail!("min_sol_reserve cannot be negative");
        }

        // Regime gate validation
        if self.enable_regime_gate {
            if self.min_volatility_to_trade < 0.0 {
                bail!("min_volatility_to_trade cannot be negative");
            }
            if self.min_volatility_to_trade > 5.0 {
                warn!("⚠️ Very high min_volatility_to_trade ({:.2}%) - bot may rarely trade!",
                      self.min_volatility_to_trade);
            }
        }

        // Order lifecycle validation
        if self.enable_order_lifecycle {
            if self.order_max_age_minutes == 0 {
                bail!("order_max_age_minutes must be > 0");
            }
            if self.order_refresh_interval_minutes == 0 {
                bail!("order_refresh_interval_minutes must be > 0");
            }
            if self.order_refresh_interval_minutes > self.order_max_age_minutes {
                warn!("⚠️ refresh_interval > max_age - orders will never trigger refresh");
            }
            if self.min_orders_to_maintain < 2 {
                bail!("min_orders_to_maintain must be at least 2");
            }
        }

        // Price bounds validation
        if self.enable_price_bounds {
            if self.lower_price_bound >= self.upper_price_bound {
                bail!("lower_price_bound must be < upper_price_bound");
            }
            if self.lower_price_bound <= 0.0 {
                bail!("lower_price_bound must be positive");
            }
        }

        Ok(())
    }

    /// Apply environment-specific overrides
    pub fn apply_environment(&mut self, environment: &str) {
        match environment {
            "testing" => {
                info!("🧪 Testing environment: Relaxing safety constraints");
                self.enable_regime_gate = false;
                self.min_volatility_to_trade = 0.0;
                self.pause_in_very_low_vol = false;
                self.enable_price_bounds = false;
            }
            "development" => {
                info!("🔧 Development environment: Moderate safety");
                if self.min_volatility_to_trade > 0.5 {
                    info!("   Lowering min_volatility from {:.2}% to 0.3%",
                          self.min_volatility_to_trade);
                    self.min_volatility_to_trade = 0.3;
                }
            }
            "production" => {
                info!("🔒 Production environment: Enforcing safety");
                if !self.enable_regime_gate {
                    warn!("⚠️ Force-enabling regime gate for production!");
                    self.enable_regime_gate = true;
                }
                if self.min_volatility_to_trade < 0.3 {
                    warn!("⚠️ Raising min_volatility to 0.3% for production safety");
                    self.min_volatility_to_trade = 0.3;
                }
                if !self.enable_order_lifecycle {
                    warn!("⚠️ Force-enabling order lifecycle for production!");
                    self.enable_order_lifecycle = true;
                }
            }
            _ => {
                warn!("⚠️ Unknown environment '{}' - using config as-is", environment);
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 🔥 V4.1 NEW: REGIME GATE CONFIGURATION (Analytics Module Bridge)
// ═══════════════════════════════════════════════════════════════════════════

/// Regime gate configuration bridge for analytics module
///
/// The analytics module (`src/strategies/shared/analytics`) expects configuration
/// in BPS (basis points) format, while TradingConfig uses percentage format.
/// This struct provides the conversion layer between the two systems.
///
/// # BPS (Basis Points) Explanation
/// - 1 BPS = 0.01%
/// - Example: 0.5% = 50 BPS
/// - Why: Analytics module uses BPS internally for precision
///
/// # Usage
/// ```rust
/// use crate::config::{TradingConfig, RegimeGateConfig};
///
/// let trading_config = TradingConfig { ... };
/// let regime_config = RegimeGateConfig::from(&trading_config);
/// ```
///
/// # Conversion Example
/// ```
/// TradingConfig:           RegimeGateConfig:
/// min_volatility = 0.5%  → min_volatility_bps = 50.0
/// enable_gate = true     → enable_gate = true
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegimeGateConfig {
    /// Enable regime-based trading gates
    pub enable_regime_gate: bool,

    /// Volatility threshold in basis points (BPS)
    /// Analytics uses this to set regime detection thresholds
    pub volatility_threshold_bps: f64,

    /// Trend sensitivity threshold (dimensionless)
    pub trend_threshold: f64,

    /// Minimum volatility required to trade (BPS)
    pub min_volatility_to_trade_bps: f64,

    /// Pause trading in very low volatility regimes
    pub pause_in_very_low_vol: bool,
}

impl From<&TradingConfig> for RegimeGateConfig {
    fn from(trading: &TradingConfig) -> Self {
        // Convert percentage to BPS: multiply by 100
        // Formula: percentage * 100 = BPS
        // Example: 0.5% * 100 = 50 BPS
        let volatility_bps = trading.min_volatility_to_trade * 100.0;

        info!("🔧 Converting TradingConfig → RegimeGateConfig:");
        info!("   Min volatility: {:.2}% → {} BPS",
              trading.min_volatility_to_trade, volatility_bps);
        info!("   Regime gate: {}", if trading.enable_regime_gate { "ENABLED" } else { "DISABLED" });

        Self {
            enable_regime_gate: trading.enable_regime_gate,
            volatility_threshold_bps: volatility_bps,
            trend_threshold: 3.0,  // Default trend sensitivity
            min_volatility_to_trade_bps: volatility_bps,
            pause_in_very_low_vol: trading.pause_in_very_low_vol,
        }
    }
}

impl Default for RegimeGateConfig {
    fn default() -> Self {
        Self {
            enable_regime_gate: true,
            volatility_threshold_bps: 2.0,   // 0.02%
            trend_threshold: 3.0,
            min_volatility_to_trade_bps: 3.0,  // 0.03%
            pause_in_very_low_vol: true,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// STRATEGIES CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StrategiesConfig {
    /// Active strategies (e.g., ["grid", "momentum"])
    pub active: Vec<String>,

    /// Consensus mode: "single", "weighted", "majority", "unanimous"
    pub consensus_mode: String,

    /// Grid strategy configuration
    pub grid: GridStrategyConfig,

    /// Momentum strategy configuration
    #[serde(default)]
    pub momentum: MomentumStrategyConfig,

    /// Mean reversion strategy configuration
    #[serde(default)]
    pub mean_reversion: MeanReversionStrategyConfig,

    /// RSI strategy configuration
    #[serde(default)]
    pub rsi: RsiStrategyConfig,

    /// Enable multi-timeframe analysis
    #[serde(default)]
    pub enable_multi_timeframe: bool,

    /// Require all timeframes to align
    #[serde(default)]
    pub require_timeframe_alignment: bool,
}

impl StrategiesConfig {
    pub fn validate(&self) -> Result<()> {
        let valid_modes = ["single", "weighted", "majority", "unanimous"];
        if !valid_modes.contains(&self.consensus_mode.as_str()) {
            bail!("Invalid consensus_mode '{}'. Must be one of: {:?}",
                  self.consensus_mode, valid_modes);
        }

        if self.active.is_empty() {
            bail!("At least one strategy must be active");
        }

        // Validate strategy weights sum to reasonable value
        let mut total_weight = 0.0;
        if self.grid.enabled {
            total_weight += self.grid.weight;
        }
        if self.momentum.enabled {
            total_weight += self.momentum.weight;
        }
        if self.mean_reversion.enabled {
            total_weight += self.mean_reversion.weight;
        }
        if self.rsi.enabled {
            total_weight += self.rsi.weight;
        }

        if total_weight == 0.0 {
            bail!("No strategies are enabled");
        }

        Ok(())
    }
}

impl Default for StrategiesConfig {
    fn default() -> Self {
        Self {
            active: vec!["grid".to_string()],
            consensus_mode: "single".to_string(),
            grid: GridStrategyConfig {
                enabled: true,
                weight: 1.0,
                min_confidence: 0.5,
            },
            momentum: MomentumStrategyConfig::default(),
            mean_reversion: MeanReversionStrategyConfig::default(),
            rsi: RsiStrategyConfig::default(),
            enable_multi_timeframe: false,
            require_timeframe_alignment: false,
        }
    }
}

// Strategy sub-configs
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GridStrategyConfig {
    pub enabled: bool,
    pub weight: f64,
    #[serde(default = "default_confidence")]
    pub min_confidence: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MomentumStrategyConfig {
    pub enabled: bool,
    pub weight: f64,
    pub min_confidence: f64,
    pub lookback_period: usize,
    pub threshold: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MeanReversionStrategyConfig {
    pub enabled: bool,
    pub weight: f64,
    pub min_confidence: f64,
    pub sma_period: usize,
    pub std_dev_multiplier: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RsiStrategyConfig {
    pub enabled: bool,
    pub weight: f64,
    pub min_confidence: f64,
    pub period: usize,
    pub oversold_threshold: f64,
    pub overbought_threshold: f64,
}

impl Default for MomentumStrategyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            weight: 0.8,
            min_confidence: 0.6,
            lookback_period: 20,
            threshold: 0.02,
        }
    }
}

impl Default for MeanReversionStrategyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            weight: 0.7,
            min_confidence: 0.6,
            sma_period: 20,
            std_dev_multiplier: 2.0,
        }
    }
}

impl Default for RsiStrategyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            weight: 0.9,
            min_confidence: 0.7,
            period: 14,
            oversold_threshold: 30.0,
            overbought_threshold: 70.0,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// RISK CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RiskConfig {
    /// Maximum position size as % of portfolio
    pub max_position_size_pct: f64,

    /// Maximum drawdown % before halting
    pub max_drawdown_pct: f64,

    /// Stop loss %
    pub stop_loss_pct: f64,

    /// Take profit %
    pub take_profit_pct: f64,

    /// Enable circuit breaker
    #[serde(default = "default_true")]
    pub enable_circuit_breaker: bool,

    /// Circuit breaker threshold %
    pub circuit_breaker_threshold_pct: f64,

    /// Circuit breaker cooldown (seconds)
    pub circuit_breaker_cooldown_secs: u64,
}

impl RiskConfig {
    pub fn validate(&self) -> Result<()> {
        if self.max_position_size_pct <= 0.0 || self.max_position_size_pct > 100.0 {
            bail!("max_position_size_pct must be between 0-100%");
        }

        if self.max_drawdown_pct <= 0.0 || self.max_drawdown_pct > 100.0 {
            bail!("max_drawdown_pct must be between 0-100%");
        }

        if self.enable_circuit_breaker {
            if self.circuit_breaker_threshold_pct <= 0.0 {
                bail!("circuit_breaker_threshold_pct must be positive");
            }
            if self.circuit_breaker_cooldown_secs == 0 {
                warn!("⚠️ circuit_breaker_cooldown_secs is 0 - may trigger repeatedly");
            }
        }

        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// PYTH PRICE FEED CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PythConfig {
    pub http_endpoint: String,
    pub feed_ids: Vec<String>,

    #[serde(default = "default_update_interval")]
    pub update_interval_ms: u64,

    /// Enable WebSocket feed (future)
    #[serde(default)]
    pub enable_websocket: bool,

    /// WebSocket endpoint (optional)
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
            update_interval_ms: 500,
            enable_websocket: false,
            websocket_endpoint: None,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// PERFORMANCE CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PerformanceConfig {
    /// Main cycle interval (milliseconds)
    #[serde(default = "default_cycle_interval")]
    pub cycle_interval_ms: u64,

    /// Startup delay (milliseconds)
    #[serde(default = "default_startup_delay")]
    pub startup_delay_ms: u64,

    /// Request timeout (milliseconds)
    #[serde(default = "default_request_timeout")]
    pub request_timeout_ms: u64,
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            cycle_interval_ms: 100,  // 10Hz
            startup_delay_ms: 1000,
            request_timeout_ms: 5000,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// LOGGING CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    /// Log level: "trace", "debug", "info", "warn", "error"
    pub level: String,

    /// Log file path
    pub file_path: String,

    /// Enable file logging
    #[serde(default = "default_true")]
    pub enable_file_logging: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            file_path: "logs/gridbot.log".to_string(),
            enable_file_logging: true,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// METRICS CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MetricsConfig {
    #[serde(default = "default_true")]
    pub enable_metrics: bool,

    /// Report stats every N cycles
    #[serde(default = "default_stats_interval")]
    pub stats_interval: u64,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enable_metrics: true,
            stats_interval: 50,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// PAPER TRADING CONFIGURATION - V3.5 ENHANCED! 🎮
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PaperTradingConfig {
    /// Initial USDC balance
    #[serde(default = "default_initial_usdc")]
    pub initial_usdc: f64,

    /// Initial SOL balance
    #[serde(default = "default_initial_sol")]
    pub initial_sol: f64,

    // ─────────────────────────────────────────────────────────────────────────
    // V3.5+: FLEXIBLE DURATION OPTIONS 🕐
    // ─────────────────────────────────────────────────────────────────────────

    /// Test duration in hours (integer only)
    #[serde(default)]
    pub test_duration_hours: Option<usize>,

    /// Test duration in minutes (more flexible)
    #[serde(default)]
    pub test_duration_minutes: Option<usize>,

    /// Test duration in seconds (precise control)
    #[serde(default)]
    pub test_duration_seconds: Option<usize>,

    /// Exact number of cycles (expert mode)
    #[serde(default)]
    pub test_cycles: Option<usize>,
}

impl PaperTradingConfig {
    /// Calculate total test duration in seconds
    pub fn duration_seconds(&self) -> usize {
        if let Some(secs) = self.test_duration_seconds {
            return secs;
        }
        if let Some(mins) = self.test_duration_minutes {
            return mins * 60;
        }
        if let Some(hours) = self.test_duration_hours {
            return hours * 3600;
        }
        // Default: 1 hour
        3600
    }

    /// Calculate total cycles based on duration and cycle interval
    pub fn calculate_cycles(&self, cycle_interval_ms: u64) -> usize {
        if let Some(cycles) = self.test_cycles {
            return cycles;
        }

        let duration_secs = self.duration_seconds();
        let cycles_per_sec = 1000 / cycle_interval_ms as usize;
        duration_secs * cycles_per_sec
    }

    pub fn validate(&self) -> Result<()> {
        if self.initial_usdc <= 0.0 {
            bail!("initial_usdc must be positive");
        }
        if self.initial_sol <= 0.0 {
            bail!("initial_sol must be positive");
        }

        // Check at least one duration is set
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
            initial_usdc: 5000.0,
            initial_sol: 10.0,
            test_duration_hours: Some(1),
            test_duration_minutes: None,
            test_duration_seconds: None,
            test_cycles: None,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// DATABASE & ALERTS (Optional)
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DatabaseConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default)]
    pub url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AlertsConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default)]
    pub telegram_bot_token: Option<String>,

    #[serde(default)]
    pub telegram_chat_id: Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════════
// DEFAULT VALUE HELPERS
// ═══════════════════════════════════════════════════════════════════════════

fn default_true() -> bool { true }
fn default_confidence() -> f64 { 0.5 }
fn default_reposition_threshold() -> f64 { 0.5 }
fn default_volatility_window() -> u32 { 100 }
fn default_rebalance_threshold() -> f64 { 5.0 }
fn default_cooldown() -> u64 { 60 }
fn default_max_orders() -> u32 { 10 }
fn default_refresh_interval() -> u64 { 300 }
fn default_profit_threshold() -> f64 { 0.1 }
fn default_slippage() -> f64 { 1.0 }
fn default_lower_bound() -> f64 { 100.0 }
fn default_upper_bound() -> f64 { 200.0 }
fn default_min_volatility() -> f64 { 0.5 }
fn default_order_max_age() -> u64 { 10 }
fn default_lifecycle_check() -> u64 { 5 }
fn default_min_orders() -> usize { 8 }
fn default_update_interval() -> u64 { 500 }
fn default_cycle_interval() -> u64 { 100 }
fn default_startup_delay() -> u64 { 1000 }
fn default_stats_interval() -> u64 { 50 }
fn default_initial_usdc() -> f64 { 5000.0 }
fn default_initial_sol() -> f64 { 10.0 }
fn default_request_timeout() -> u64 { 5000 }

// ═══════════════════════════════════════════════════════════════════════════
// MAIN CONFIG IMPLEMENTATION - V4.1 PRODUCTION GRADE! 🚀
// ═══════════════════════════════════════════════════════════════════════════

impl Config {
    /// Load configuration from default location
    pub fn load() -> Result<Self> {
        Self::from_file("config/master.toml")
    }

    /// Load configuration from specific file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        info!("🔧 Loading configuration from: {}", path.display());

        // Read file
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        // Parse TOML
        let mut config: Config = toml::from_str(&content)
            .context("Failed to parse TOML configuration")?;

        // Apply environment-specific overrides
        info!("🌍 Applying environment overrides: {}", config.bot.environment);
        config.apply_environment_defaults();

        // Validate
        config.validate()
            .context("Configuration validation failed")?;

        info!("✅ Configuration loaded and validated successfully!\n");

        Ok(config)
    }

    /// Save configuration to file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        let toml_string = toml::to_string_pretty(self)
            .context("Failed to serialize config to TOML")?;

        fs::write(path, toml_string)
            .with_context(|| format!("Failed to write config to: {}", path.display()))?;

        info!("💾 Configuration saved to: {}", path.display());
        Ok(())
    }

    /// Apply environment-specific defaults
    pub fn apply_environment_defaults(&mut self) {
        let env = self.bot.environment.clone();
        self.trading.apply_environment(&env);
    }

    /// Comprehensive validation
    pub fn validate(&self) -> Result<()> {
        // Bot validation
        if self.bot.name.is_empty() {
            bail!("Bot name cannot be empty");
        }
        if self.bot.version.is_empty() {
            bail!("Bot version cannot be empty");
        }

        // Network validation
        self.network.validate()
            .context("Network config validation failed")?;

        // Trading validation
        self.trading.validate()
            .context("Trading config validation failed")?;

        // Strategies validation
        self.strategies.validate()
            .context("Strategies config validation failed")?;

        // Risk validation
        self.risk.validate()
            .context("Risk config validation failed")?;

        // Paper trading validation
        self.paper_trading.validate()
            .context("Paper trading config validation failed")?;

        info!("✅ All configuration sections validated");
        Ok(())
    }

    /// Display comprehensive configuration summary
    pub fn display_summary(&self) {
        let border = "═".repeat(78);

        println!("\n{}", border);
        println!("  🤖 PROJECT FLASH V4.1 - CONFIGURATION");
        println!("{}\n", border);

        println!("📋 BOT: {} v{} [{}]",
            self.bot.name, self.bot.version, self.bot.environment);

        println!("\n📈 TRADING:");
        println!("   Grid:             {} levels @ {:.3}%",
            self.trading.grid_levels, self.trading.grid_spacing_percent);
        println!("   Order Size:       {} SOL", self.trading.min_order_size);
        println!("   Auto-Rebalance:   {}", if self.trading.enable_auto_rebalance { "✅" } else { "❌" });
        println!("   Smart Rebalance:  {}", if self.trading.enable_smart_rebalance { "✅" } else { "❌" });
        println!("   Reserves:         ${:.0} USDC + {:.1} SOL",
            self.trading.min_usdc_reserve, self.trading.min_sol_reserve);

        println!("\n🆕 MARKET INTELLIGENCE:");
        println!("   Regime Gate:      {} (min vol: {:.2}%)",
            if self.trading.enable_regime_gate { "✅" } else { "❌" },
            self.trading.min_volatility_to_trade);
        println!("   Pause Low Vol:    {}",
            if self.trading.pause_in_very_low_vol { "✅" } else { "❌" });

        println!("\n🔄 ORDER LIFECYCLE:");
        println!("   Enabled:          {}",
            if self.trading.enable_order_lifecycle { "✅" } else { "❌" });
        if self.trading.enable_order_lifecycle {
            println!("   Refresh:          Every {}min",
                self.trading.order_refresh_interval_minutes);
            println!("   Min Orders:       {}",
                self.trading.min_orders_to_maintain);
            println!("   Max Age:          {}min",
                self.trading.order_max_age_minutes);
        }

        println!("\n🎯 STRATEGIES:");
        println!("   Active:           {}", self.strategies.active.join(", "));
        println!("   Mode:             {}", self.strategies.consensus_mode);

        println!("\n🛡️  RISK MANAGEMENT:");
        println!("   Max Position:     {:.0}%", self.risk.max_position_size_pct);
        println!("   Max Drawdown:     {:.1}%", self.risk.max_drawdown_pct);
        println!("   Stop Loss:        {:.1}%", self.risk.stop_loss_pct);
        println!("   Take Profit:      {:.1}%", self.risk.take_profit_pct);
        println!("   Circuit Breaker:  {} ({:.1}%)",
            if self.risk.enable_circuit_breaker { "✅" } else { "❌" },
            self.risk.circuit_breaker_threshold_pct);

        println!("\n{}\n", border);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// BUILDER PATTERN - For Programmatic Construction
// ═══════════════════════════════════════════════════════════════════════════

pub struct ConfigBuilder {
    config: Config,
}

impl ConfigBuilder {
    pub fn new() -> Self {
        // Start with sensible defaults
        Self {
            config: Config {
                bot: BotConfig {
                    name: "GridBot-Builder".to_string(),
                    version: "4.1.0".to_string(),
                    environment: "testing".to_string(),
                },
                network: NetworkConfig {
                    cluster: "devnet".to_string(),
                    rpc_url: "https://api.devnet.solana.com".to_string(),
                    commitment: "confirmed".to_string(),
                    ws_url: None,
                },
                trading: TradingConfig {
                    grid_levels: 35,
                    grid_spacing_percent: 0.15,
                    min_order_size: 0.1,
                    max_position_size: 100.0,
                    min_usdc_reserve: 300.0,
                    min_sol_reserve: 2.0,
                    enable_dynamic_grid: true,
                    reposition_threshold: 0.5,
                    volatility_window: 100,
                    enable_auto_rebalance: true,
                    enable_smart_rebalance: true,
                    rebalance_threshold_pct: 5.0,
                    rebalance_cooldown_secs: 60,
                    max_orders_per_side: 10,
                    order_refresh_interval_secs: 300,
                    enable_market_orders: false,
                    enable_fee_optimization: true,
                    min_profit_threshold_pct: 0.1,
                    max_slippage_pct: 1.0,
                    enable_price_bounds: false,
                    lower_price_bound: 100.0,
                    upper_price_bound: 200.0,
                    enable_regime_gate: false,
                    min_volatility_to_trade: 0.0,
                    pause_in_very_low_vol: false,
                    enable_order_lifecycle: true,
                    order_max_age_minutes: 10,
                    order_refresh_interval_minutes: 5,
                    min_orders_to_maintain: 8,
                    enable_adaptive_spacing: false,
                    enable_smart_position_sizing: false,
                },
                strategies: StrategiesConfig::default(),
                risk: RiskConfig {
                    max_position_size_pct: 30.0,
                    max_drawdown_pct: 10.0,
                    stop_loss_pct: 5.0,
                    take_profit_pct: 10.0,
                    enable_circuit_breaker: true,
                    circuit_breaker_threshold_pct: 8.0,
                    circuit_breaker_cooldown_secs: 300,
                },
                pyth: PythConfig::default(),
                performance: PerformanceConfig::default(),
                logging: LoggingConfig::default(),
                metrics: MetricsConfig::default(),
                paper_trading: PaperTradingConfig::default(),
                database: DatabaseConfig::default(),
                alerts: AlertsConfig::default(),
            },
        }
    }

    pub fn environment(mut self, env: &str) -> Self {
        self.config.bot.environment = env.to_string();
        self
    }

    pub fn grid_spacing(mut self, spacing: f64) -> Self {
        self.config.trading.grid_spacing_percent = spacing;
        self
    }

    pub fn grid_levels(mut self, levels: u32) -> Self {
        self.config.trading.grid_levels = levels;
        self
    }

    pub fn enable_regime_gate(mut self, enabled: bool) -> Self {
        self.config.trading.enable_regime_gate = enabled;
        self
    }

    pub fn min_volatility(mut self, vol: f64) -> Self {
        self.config.trading.min_volatility_to_trade = vol;
        self
    }

    pub fn paper_trading_capital(mut self, usdc: f64, sol: f64) -> Self {
        self.config.paper_trading.initial_usdc = usdc;
        self.config.paper_trading.initial_sol = sol;
        self
    }

    pub fn build(mut self) -> Result<Config> {
        self.config.apply_environment_defaults();
        self.config.validate()?;
        Ok(self.config)
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}
