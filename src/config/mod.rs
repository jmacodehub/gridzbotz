//! ═══════════════════════════════════════════════════════════════════════════
//! 🎛️ UNIFIED CONFIGURATION SYSTEM V4.2 - PROJECT FLASH + AI
//!
//! Single source of truth for ALL bot settings with best practices:
//!
//! V4.2 ENHANCEMENTS - Adaptive Optimizer Integration:
//! ✅ AdaptiveOptimizerConfig for AI-powered trading
//! ✅ Self-learning grid spacing and position sizing
//! ✅ Fully configurable via TOML
//!
//! V4.1 ENHANCEMENTS - RegimeGate Analytics Bridge:
//! ✅ RegimeGateConfig for analytics module compatibility
//! ✅ BPS (basis points) conversion from percentage format
//! ✅ Type-safe bridging between config systems
//!
//! Architecture:
//! • `Config` - Main TOML-based configuration
//! • `ConfigBuilder` - Programmatic builder for tests
//! • Environment-specific overrides
//! • Comprehensive validation
//!
//! February 11, 2026 - V4.2 AI INTEGRATION! 🧠🚀
//! ═══════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use anyhow::{Result, Context, bail};
use std::path::Path;
use std::fs;
use log::{info, warn};

// ═══════════════════════════════════════════════════════════════════════════
// MODULE DECLARATIONS
// ═══════════════════════════════════════════════════════════════════════════

pub mod adaptive_optimizer;

// Re-export for convenience
pub use adaptive_optimizer::AdaptiveOptimizerConfig;

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

    /// 🧠 V4.2 NEW: Adaptive Optimizer (AI-powered trading)
    #[serde(default)]
    pub adaptive_optimizer: AdaptiveOptimizerConfig,
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
// TRADING CONFIGURATION - V4.2 WITH AI! 🔥🧠
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
    // V3.0+: MARKET REGIME GATE 🚫
    // ─────────────────────────────────────────────────────────────────────────

    /// Enable market regime gate
    #[serde(default = "default_true")]
    pub enable_regime_gate: bool,

    /// Minimum volatility required to trade
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
    pub fn validate(&self) -> Result<()> {
        if self.grid_levels < 2 {
            bail!("grid_levels must be at least 2");
        }
        if self.grid_spacing_percent <= 0.0 {
            bail!("grid_spacing_percent must be positive");
        }
        if self.min_order_size <= 0.0 {
            bail!("min_order_size must be positive");
        }
        if self.max_position_size <= self.min_order_size {
            bail!("max_position_size must be > min_order_size");
        }
        Ok(())
    }

    pub fn apply_environment(&mut self, environment: &str) {
        match environment {
            "testing" => {
                info!("🧪 Testing environment: Relaxing safety constraints");
                self.enable_regime_gate = false;
                self.min_volatility_to_trade = 0.0;
                self.pause_in_very_low_vol = false;
            }
            "development" => {
                info!("🔧 Development environment: Moderate safety");
            }
            "production" => {
                info!("🔒 Production environment: Enforcing safety");
                if !self.enable_regime_gate {
                    warn!("⚠️ Force-enabling regime gate for production!");
                    self.enable_regime_gate = true;
                }
            }
            _ => {}
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// REGIME GATE CONFIGURATION (Analytics Bridge)
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RegimeGateConfig {
    pub enable_regime_gate: bool,
    pub volatility_threshold_bps: f64,
    pub trend_threshold: f64,
    pub min_volatility_to_trade_bps: f64,
    pub pause_in_very_low_vol: bool,
}

impl From<&TradingConfig> for RegimeGateConfig {
    fn from(trading: &TradingConfig) -> Self {
        let volatility_bps = trading.min_volatility_to_trade * 100.0;
        Self {
            enable_regime_gate: trading.enable_regime_gate,
            volatility_threshold_bps: volatility_bps,
            trend_threshold: 3.0,
            min_volatility_to_trade_bps: volatility_bps,
            pause_in_very_low_vol: trading.pause_in_very_low_vol,
        }
    }
}

impl Default for RegimeGateConfig {
    fn default() -> Self {
        Self {
            enable_regime_gate: true,
            volatility_threshold_bps: 2.0,
            trend_threshold: 3.0,
            min_volatility_to_trade_bps: 3.0,
            pause_in_very_low_vol: true,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// STRATEGIES CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StrategiesConfig {
    pub active: Vec<String>,
    pub consensus_mode: String,
    pub grid: GridStrategyConfig,

    #[serde(default)]
    pub momentum: MomentumStrategyConfig,

    #[serde(default)]
    pub mean_reversion: MeanReversionStrategyConfig,

    #[serde(default)]
    pub rsi: RsiStrategyConfig,

    #[serde(default)]
    pub enable_multi_timeframe: bool,

    #[serde(default)]
    pub require_timeframe_alignment: bool,
}

impl StrategiesConfig {
    pub fn validate(&self) -> Result<()> {
        let valid_modes = ["single", "weighted", "majority", "unanimous"];
        if !valid_modes.contains(&self.consensus_mode.as_str()) {
            bail!("Invalid consensus_mode");
        }
        if self.active.is_empty() {
            bail!("At least one strategy must be active");
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
    pub max_position_size_pct: f64,
    pub max_drawdown_pct: f64,
    pub stop_loss_pct: f64,
    pub take_profit_pct: f64,

    #[serde(default = "default_true")]
    pub enable_circuit_breaker: bool,

    pub circuit_breaker_threshold_pct: f64,
    pub circuit_breaker_cooldown_secs: u64,
}

impl RiskConfig {
    pub fn validate(&self) -> Result<()> {
        if self.max_position_size_pct <= 0.0 || self.max_position_size_pct > 100.0 {
            bail!("max_position_size_pct must be 0-100%");
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// REMAINING CONFIG STRUCTS (Simplified)
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Deserialize, Serialize)]
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
            update_interval_ms: 500,
            enable_websocket: false,
            websocket_endpoint: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
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
        Self {
            cycle_interval_ms: 100,
            startup_delay_ms: 1000,
            request_timeout_ms: 5000,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    pub level: String,
    pub file_path: String,

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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MetricsConfig {
    #[serde(default = "default_true")]
    pub enable_metrics: bool,

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

#[derive(Debug, Clone, Deserialize, Serialize)]
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
        if let Some(secs) = self.test_duration_seconds {
            return secs;
        }
        if let Some(mins) = self.test_duration_minutes {
            return mins * 60;
        }
        if let Some(hours) = self.test_duration_hours {
            return hours * 3600;
        }
        3600
    }

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
// DEFAULT HELPERS
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
// MAIN CONFIG IMPLEMENTATION
// ═══════════════════════════════════════════════════════════════════════════

impl Config {
    pub fn load() -> Result<Self> {
        Self::from_file("config/master.toml")
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        info!("🔧 Loading configuration from: {}", path.display());

        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let mut config: Config = toml::from_str(&content)
            .context("Failed to parse TOML configuration")?;

        info!("🌍 Applying environment overrides: {}", config.bot.environment);
        config.apply_environment_defaults();

        config.validate()
            .context("Configuration validation failed")?;

        info!("✅ Configuration loaded and validated successfully!\n");

        Ok(config)
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        let toml_string = toml::to_string_pretty(self)
            .context("Failed to serialize config to TOML")?;

        fs::write(path, toml_string)
            .with_context(|| format!("Failed to write config to: {}", path.display()))?;

        info!("💾 Configuration saved to: {}", path.display());
        Ok(())
    }

    pub fn apply_environment_defaults(&mut self) {
        let env = self.bot.environment.clone();
        self.trading.apply_environment(&env);
    }

    pub fn validate(&self) -> Result<()> {
        if self.bot.name.is_empty() {
            bail!("Bot name cannot be empty");
        }
        self.network.validate().context("Network validation failed")?;
        self.trading.validate().context("Trading validation failed")?;
        self.strategies.validate().context("Strategies validation failed")?;
        self.risk.validate().context("Risk validation failed")?;
        self.paper_trading.validate().context("Paper trading validation failed")?;
        self.adaptive_optimizer.validate().context("Adaptive optimizer validation failed")?;
        Ok(())
    }

    pub fn display_summary(&self) {
        println!("\n═══════════════════════════════════════════════════════════════════════");
        println!("  🤖 GridBot V4.2 - AI-POWERED CONFIGURATION");
        println!("═══════════════════════════════════════════════════════════════════════\n");
        println!("📋 BOT: {} v{} [{}]", self.bot.name, self.bot.version, self.bot.environment);
        println!("\n🧠 AI: {}", if self.adaptive_optimizer.enabled { "✅ ENABLED" } else { "❌ DISABLED" });
        println!("\n═══════════════════════════════════════════════════════════════════════\n");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// CONFIG BUILDER
// ═══════════════════════════════════════════════════════════════════════════

pub struct ConfigBuilder {
    config: Config,
}

impl ConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: Config {
                bot: BotConfig {
                    name: "GridBot".to_string(),
                    version: "4.2.0".to_string(),
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
                adaptive_optimizer: AdaptiveOptimizerConfig::default(),
            },
        }
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
