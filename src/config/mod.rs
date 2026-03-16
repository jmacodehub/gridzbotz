//! ═══════════════════════════════════════════════════════════════════════════
//! 🎛️  UNIFIED CONFIGURATION SYSTEM V6.1 - GRIDZBOTZ
//!
//! Struct definitions live in focused submodules:
//!   network.rs       → NetworkConfig
//!   security.rs      → SecurityConfig
//!   execution.rs     → ExecutionConfig, FeeFilterConfig
//!   trading.rs       → TradingConfig, RegimeGateConfig
//!   strategies.rs    → StrategiesConfig + all sub-configs + param structs
//!   risk.rs          → RiskConfig
//!   observability.rs → PythConfig, PerformanceConfig, LoggingConfig,
//!                       MetricsConfig, PaperTradingConfig, DatabaseConfig,
//!                       AlertsConfig
//!
//! This file: Config struct, ConfigBuilder, default_*() helpers, tests.
//! ═══════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use anyhow::{Result, Context, bail};
use std::path::Path;
use std::fs;
use log::{info, warn};

// ── submodule declarations ──────────────────────────────────────────────────
pub mod secrets;
pub mod fees;
pub mod priority_fees;
pub mod network;
pub mod security;
pub mod execution;
pub mod trading;
pub mod strategies;
pub mod risk;
pub mod observability;
pub mod feed_ids;
pub mod presets;

// ── re-exports (keep existing import paths across the codebase unchanged) ───
pub use fees::FeesConfig;
pub use priority_fees::PriorityFeeConfig;
pub use network::NetworkConfig;
pub use security::SecurityConfig;
pub use execution::{ExecutionConfig, FeeFilterConfig};
pub use trading::{TradingConfig, RegimeGateConfig};
pub use strategies::{
    StrategiesConfig, GridStrategyConfig,
    MomentumStrategyConfig, MeanReversionStrategyConfig,
    RsiStrategyConfig, MomentumMACDStrategyConfig,
    RsiParams, MeanReversionParams, MomentumMACDParams,
};
pub use risk::RiskConfig;
pub use observability::{
    PythConfig, PerformanceConfig, LoggingConfig, MetricsConfig,
    PaperTradingConfig, DatabaseConfig, AlertsConfig,
};

// ═══════════════════════════════════════════════════════════════════════════
// MAIN CONFIGURATION STRUCT
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub bot: BotConfig,
    pub network: NetworkConfig,
    #[serde(default)]
    pub security: SecurityConfig,
    pub trading: TradingConfig,
    pub strategies: StrategiesConfig,
    pub risk: RiskConfig,
    #[serde(default)]
    pub fees: FeesConfig,
    #[serde(default)]
    pub priority_fees: PriorityFeeConfig,
    #[serde(default)]
    pub execution: ExecutionConfig,
    #[serde(default)]
    pub pyth: PythConfig,
    #[serde(default)]
    pub performance: PerformanceConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub metrics: MetricsConfig,
    #[serde(default)]
    pub paper_trading: PaperTradingConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub alerts: AlertsConfig,
}

// ═══════════════════════════════════════════════════════════════════════════
// BOT CONFIGURATION (stays in mod.rs — tightly coupled to Config)
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BotConfig {
    pub name: String,
    pub version: String,
    pub environment: String,
    #[serde(default = "default_paper_mode")]
    pub execution_mode: String,
    #[serde(default)]
    pub instance_id: Option<String>,
}

impl BotConfig {
    pub fn is_production(&self) -> bool  { self.environment == "production" }
    pub fn is_testing(&self) -> bool     { self.environment == "testing" }
    pub fn is_development(&self) -> bool { self.environment == "development" }
    pub fn is_live(&self) -> bool        { self.execution_mode == "live" }
    pub fn is_paper(&self) -> bool       { self.execution_mode != "live" }
    pub fn instance_name(&self) -> &str {
        self.instance_id.as_deref().unwrap_or(&self.name)
    }

    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty()    { bail!("Bot name cannot be empty"); }
        if self.version.is_empty() { bail!("Bot version cannot be empty"); }
        let valid_modes = ["paper", "live"];
        if !valid_modes.contains(&self.execution_mode.as_str()) {
            bail!("Invalid execution_mode '{}'. Must be one of: {:?}",
                  self.execution_mode, valid_modes);
        }
        if self.execution_mode == "live" && self.environment != "production" {
            warn!(
                "⚠️ execution_mode=live but environment={}. \
                 Set environment=\"production\" for full safety enforcement.",
                self.environment
            );
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// DEFAULT VALUE HELPERS  (pub(crate) so submodules can import via super::)
// ═══════════════════════════════════════════════════════════════════════════

pub(crate) fn default_true()                      -> bool   { true }
pub(crate) fn default_confidence()                -> f64    { 0.5 }
pub(crate) fn default_reposition_threshold()      -> f64    { 0.5 }
pub(crate) fn default_volatility_window()         -> u32    { 100 }
pub(crate) fn default_rebalance_threshold()       -> f64    { 5.0 }
pub(crate) fn default_cooldown()                  -> u64    { 60 }
pub(crate) fn default_max_orders()                -> u32    { 10 }
pub(crate) fn default_refresh_interval()          -> u64    { 300 }
pub(crate) fn default_profit_threshold()          -> f64    { 0.1 }
pub(crate) fn default_slippage()                  -> f64    { 1.0 }
pub(crate) fn default_lower_bound()               -> f64    { 100.0 }
pub(crate) fn default_upper_bound()               -> f64    { 200.0 }
pub(crate) fn default_order_max_age()             -> u64    { 10 }
pub(crate) fn default_lifecycle_check()           -> u64    { 5 }
pub(crate) fn default_min_orders()                -> usize  { 8 }
pub(crate) fn default_update_interval()           -> u64    { 500 }
pub(crate) fn default_cycle_interval()            -> u64    { 100 }
pub(crate) fn default_startup_delay()             -> u64    { 1000 }
pub(crate) fn default_stats_interval()            -> u64    { 50 }
pub(crate) fn default_initial_usdc()              -> f64    { 5000.0 }
pub(crate) fn default_initial_sol()               -> f64    { 10.0 }
pub(crate) fn default_request_timeout()           -> u64    { 5000 }
pub(crate) fn default_paper_mode()                -> String { "paper".to_string() }
pub(crate) fn default_max_trade_sol()             -> f64    { 0.5 }
pub(crate) fn default_priority_fee_microlamports() -> u64   { 50_000 }
pub(crate) fn default_slippage_bps()              -> u16    { 100 }
pub(crate) fn default_confirm_timeout_secs()      -> u64    { 60 }
pub(crate) fn default_max_tx_retries()            -> u8     { 3 }
pub(crate) fn default_max_requote_attempts()      -> u8     { 3 }
pub(crate) fn default_extreme_oversold()          -> f64    { 20.0 }
pub(crate) fn default_extreme_overbought()        -> f64    { 80.0 }
pub(crate) fn default_strong_threshold()          -> f64    { 5.0 }
pub(crate) fn default_normal_threshold()          -> f64    { 2.5 }
pub(crate) fn default_macd_min_confidence()       -> f64    { 0.65 }
pub(crate) fn default_macd_histogram_threshold()  -> f64    { 0.5 }
pub(crate) fn default_macd_warmup_periods()       -> usize  { 26 }
pub(crate) fn default_wallet_path()               -> String { "~/.config/solana/id.json".to_string() }
pub(crate) fn default_max_trade_size_usdc()       -> f64    { 250.0 }
pub(crate) fn default_max_consecutive_losses()    -> u32    { 5 }
pub(crate) fn default_trailing_stop()             -> bool   { false }
pub(crate) fn default_optimizer_interval_cycles() -> u64    { 50 }
pub(crate) fn default_min_fee_threshold_bps()     -> u32    { 8 }
pub(crate) fn default_max_fee_threshold_bps()     -> u32    { 50 }
pub(crate) fn default_fee_filter_window_secs()    -> u64    { 30 }
pub(crate) fn default_signal_size_multiplier()    -> f64    { 1.0 }
pub(crate) fn default_wma_confidence_threshold()  -> f64    { 0.50 }
pub(crate) fn default_grid_seed_bypass()          -> bool   { true }
pub(crate) fn default_max_grid_spacing_pct()      -> f64    { 0.0075 }
pub(crate) fn default_min_grid_spacing_pct()      -> f64    { 0.001 }
pub(crate) fn default_vol_floor_resume_pct()      -> f64    { 0.05 }
pub(crate) fn default_stop_loss_cooldown_secs()   -> u64    { 300 }
pub(crate) fn default_min_volatility()            -> f64    { 0.5 }

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
        secrets::resolve_secrets(&mut config)
            .context("Secret resolution failed")?;
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

    pub fn trading_pair(&self) -> String {
        "SOL/USDC".to_string()
    }

    pub fn validate(&self) -> Result<()> {
        self.bot.validate().context("Bot config validation failed")?;
        self.network.validate().context("Network config validation failed")?;
        self.trading.validate().context("Trading config validation failed")?;
        self.strategies.validate().context("Strategies config validation failed")?;
        self.risk.validate().context("Risk config validation failed")?;
        self.fees.validate()
            .map_err(|e| anyhow::anyhow!(e))
            .context("Fees config validation failed")?;
        self.priority_fees.validate().context("Priority fees config validation failed")?;
        if self.bot.is_live() {
            info!("🔴 LIVE MODE DETECTED — validating execution config");
            self.execution.validate().context("Execution config validation failed")?;
            self.security.validate_for_live_mode()
                .context("Security config validation failed for live mode")?;
            if self.network.cluster != "mainnet-beta" {
                warn!(
                    "⚠️ execution_mode=live but cluster={}. \
                     Are you sure you want to trade live on {}?",
                    self.network.cluster, self.network.cluster
                );
            }
        }
        self.paper_trading.validate().context("Paper trading config validation failed")?;
        info!("✅ All configuration sections validated");
        Ok(())
    }

    pub fn display_summary(&self) {
        let border = "═".repeat(78);
        println!("\n{}", border);
        println!("  🤖 GRIDZBOTZ V6.1 - CONFIGURATION");
        println!("{}\n", border);

        println!("📋 BOT: {} v{} [{}]", self.bot.name, self.bot.version, self.bot.environment);
        println!("   Instance:         {}", self.bot.instance_name());

        println!("\n⚡ EXECUTION:");
        let mode_emoji = if self.bot.is_live() { "🔴 LIVE" } else { "🟡 PAPER" };
        println!("   Mode:             {}", mode_emoji);
        if self.bot.is_live() {
            println!("   Max Trade:        {:.3} SOL", self.execution.max_trade_sol);
            println!("   Priority Fee:     {} µlamports", self.execution.priority_fee_microlamports);
            println!("   Slippage:         {} BPS ({:.1}%)",
                self.execution.max_slippage_bps, self.execution.slippage_pct());
            println!("   Jito MEV:         {}",
                if self.execution.jito_enabled() {
                    format!("✅ {} lamports", self.execution.jito_tip_lamports.unwrap_or(0))
                } else {
                    "❌ disabled".to_string()
                });
            println!("   Confirm Timeout:  {}s | Retries: {} | Requote: {} attempts",
                self.execution.confirmation_timeout_secs,
                self.execution.max_retries,
                self.execution.max_requote_attempts,
            );
        } else {
            println!("   Paper Balance:    ${:.0} USDC + {:.1} SOL",
                self.paper_trading.initial_usdc, self.paper_trading.initial_sol);
        }

        println!("\n📈 TRADING:");
        println!("   Grid:             {} levels @ {:.3}%",
            self.trading.grid_levels, self.trading.grid_spacing_percent);
        if self.trading.enable_dynamic_grid {
            println!("   Dyn Spacing:      {:.4}–{:.4} (fractions)",
                self.trading.min_grid_spacing_pct, self.trading.max_grid_spacing_pct);
        }
        println!("   Order Size:       {} SOL", self.trading.min_order_size);
        println!("   Auto-Rebalance:   {}", if self.trading.enable_auto_rebalance { "✅" } else { "❌" });
        println!("   Smart Rebalance:  {}", if self.trading.enable_smart_rebalance { "✅" } else { "❌" });
        println!("   Reserves:         ${:.0} USDC + {:.1} SOL",
            self.trading.min_usdc_reserve, self.trading.min_sol_reserve);
        println!("   Fee Optimization: {}", if self.trading.enable_fee_optimization { "✅ SmartFeeFilter" } else { "❌" });
        if self.trading.enable_fee_optimization && self.trading.fee_filter.enable_smart_fee_filter {
            println!("   Fee Filter:       {}-{} BPS | window {}s",
                self.trading.fee_filter.min_fee_threshold_bps,
                self.trading.fee_filter.max_fee_threshold_bps,
                self.trading.fee_filter.fee_filter_window_secs);
        }
        if self.trading.enable_smart_position_sizing {
            println!("   Smart Sizing:     ✅ consensus-driven | multiplier={:.2}×",
                self.trading.signal_size_multiplier);
        } else {
            println!("   Smart Sizing:     ❌");
        }
        println!("   Optimizer Cadence:{} cycles", self.trading.optimizer_interval_cycles);

        println!("\n🆕 MARKET INTELLIGENCE:");
        println!("   Regime Gate:      {} (min vol: {:.2}%)",
            if self.trading.enable_regime_gate { "✅" } else { "❌" },
            self.trading.min_volatility_to_trade);
        println!("   Pause Low Vol:    {}",
            if self.trading.pause_in_very_low_vol { "✅" } else { "❌" });

        println!("\n🔄 ORDER LIFECYCLE:");
        println!("   Enabled:          {}", if self.trading.enable_order_lifecycle { "✅" } else { "❌" });
        if self.trading.enable_order_lifecycle {
            println!("   Refresh:          Every {}min", self.trading.order_refresh_interval_minutes);
            println!("   Min Orders:       {}", self.trading.min_orders_to_maintain);
            println!("   Max Age:          {}min", self.trading.order_max_age_minutes);
        }

        println!("\n🎯 STRATEGIES:");
        println!("   Active:           {}", self.strategies.active.join(", "));
        println!("   Mode:             {}", self.strategies.consensus_mode);
        println!("   WMA Conf Gate:    {:.2}", self.strategies.wma_confidence_threshold);
        println!("   Seed Bypass:      {}",
            if self.strategies.grid.seed_orders_bypass { "✅" } else { "❌" });
        if self.strategies.rsi.enabled {
            println!("   RSI:              period={} oversold={:.0} overbought={:.0} extreme={:.0}/{:.0}",
                self.strategies.rsi.period,
                self.strategies.rsi.oversold_threshold,
                self.strategies.rsi.overbought_threshold,
                self.strategies.rsi.extreme_oversold,
                self.strategies.rsi.extreme_overbought);
        }
        if self.strategies.mean_reversion.enabled {
            println!("   MeanRev:          period={} strong={:.1}/{:.1} normal={:.1}/{:.1}",
                self.strategies.mean_reversion.sma_period,
                self.strategies.mean_reversion.strong_buy_threshold,
                self.strategies.mean_reversion.strong_sell_threshold,
                self.strategies.mean_reversion.buy_threshold,
                self.strategies.mean_reversion.sell_threshold);
        }
        if self.strategies.momentum_macd.enabled {
            println!("   MACD:             conf={:.2} hist_thresh={:.2} warmup={}",
                self.strategies.momentum_macd.min_confidence,
                self.strategies.momentum_macd.strong_histogram_threshold,
                self.strategies.momentum_macd.min_warmup_periods);
        }

        println!("\n🛡️  RISK MANAGEMENT:");
        println!("   Max Position:     {:.0}%", self.risk.max_position_size_pct);
        println!("   Max Drawdown:     {:.1}%", self.risk.max_drawdown_pct);
        println!("   Stop Loss:        {:.1}% ({}) | cooldown: {}s",
            self.risk.stop_loss_pct,
            if self.risk.enable_trailing_stop { "trailing" } else { "fixed" },
            self.risk.stop_loss_cooldown_secs);
        println!("   Take Profit:      {:.1}%", self.risk.take_profit_pct);
        println!("   Circuit Breaker:  {} ({:.1}%)",
            if self.risk.enable_circuit_breaker { "✅" } else { "❌" },
            self.risk.circuit_breaker_threshold_pct);
        if self.risk.enable_circuit_breaker {
            println!("   Max Consec Loss:  {} trades", self.risk.max_consecutive_losses);
        }
        if self.priority_fees.enable_dynamic {
            println!("   Priority Fees:    ⚡ dynamic (P{}, {:.1}x, {}-{} µL)",
                self.priority_fees.percentile, self.priority_fees.multiplier,
                self.priority_fees.min_microlamports, self.priority_fees.max_microlamports);
        } else {
            println!("   Priority Fees:    static");
        }
        println!("\n{}\n", border);
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
                    name: "GridzBot-Builder".to_string(),
                    version: "6.1.0".to_string(),
                    environment: "testing".to_string(),
                    execution_mode: "paper".to_string(),
                    instance_id: None,
                },
                network: NetworkConfig {
                    cluster: "devnet".to_string(),
                    rpc_url: "https://api.devnet.solana.com".to_string(),
                    commitment: "confirmed".to_string(),
                    ws_url: None,
                },
                security:      SecurityConfig::default(),
                trading:       TradingConfig::default(),
                strategies:    StrategiesConfig::default(),
                risk: RiskConfig {
                    max_position_size_pct:         30.0,
                    max_drawdown_pct:               10.0,
                    stop_loss_pct:                  5.0,
                    take_profit_pct:                10.0,
                    enable_circuit_breaker:         true,
                    circuit_breaker_threshold_pct:  8.0,
                    circuit_breaker_cooldown_secs:  300,
                    max_consecutive_losses:         default_max_consecutive_losses(),
                    enable_trailing_stop:           false,
                    stop_loss_cooldown_secs:        default_stop_loss_cooldown_secs(),
                    // ✅ PR #131 C4: WinRateGuard fields
                    enable_win_rate_guard:          false,
                    min_win_rate_pct:               40.0,
                    win_rate_guard_resume_pct:      45.0,
                    min_trades_before_guard:        10,
                },
                fees:          FeesConfig::default(),
                priority_fees: PriorityFeeConfig::default(),
                execution:     ExecutionConfig::default(),
                pyth:          PythConfig::default(),
                performance:   PerformanceConfig::default(),
                logging:       LoggingConfig::default(),
                metrics:       MetricsConfig::default(),
                paper_trading: PaperTradingConfig::default(),
                database:      DatabaseConfig::default(),
                alerts:        AlertsConfig::default(),
            },
        }
    }

    pub fn environment(mut self, env: &str) -> Self {
        self.config.bot.environment = env.to_string(); self
    }
    pub fn execution_mode(mut self, mode: &str) -> Self {
        self.config.bot.execution_mode = mode.to_string(); self
    }
    pub fn instance_id(mut self, id: &str) -> Self {
        self.config.bot.instance_id = Some(id.to_string()); self
    }
    pub fn grid_spacing(mut self, spacing: f64) -> Self {
        self.config.trading.grid_spacing_percent = spacing; self
    }
    pub fn grid_levels(mut self, levels: u32) -> Self {
        self.config.trading.grid_levels = levels; self
    }
    pub fn enable_regime_gate(mut self, enabled: bool) -> Self {
        self.config.trading.enable_regime_gate = enabled; self
    }
    pub fn min_volatility(mut self, vol: f64) -> Self {
        self.config.trading.min_volatility_to_trade = vol; self
    }
    pub fn paper_trading_capital(mut self, usdc: f64, sol: f64) -> Self {
        self.config.paper_trading.initial_usdc = usdc;
        self.config.paper_trading.initial_sol  = sol;
        self
    }
    pub fn optimizer_interval_cycles(mut self, cycles: u64) -> Self {
        self.config.trading.optimizer_interval_cycles = cycles; self
    }
    pub fn fee_filter(mut self, min_bps: u32, max_bps: u32, window_secs: u64) -> Self {
        self.config.trading.fee_filter.min_fee_threshold_bps  = min_bps;
        self.config.trading.fee_filter.max_fee_threshold_bps  = max_bps;
        self.config.trading.fee_filter.fee_filter_window_secs = window_secs;
        self
    }
    pub fn signal_size_multiplier(mut self, multiplier: f64) -> Self {
        self.config.trading.enable_smart_position_sizing = true;
        self.config.trading.signal_size_multiplier = multiplier;
        self
    }
    pub fn wma_confidence_threshold(mut self, threshold: f64) -> Self {
        self.config.strategies.wma_confidence_threshold = threshold; self
    }
    pub fn seed_orders_bypass(mut self, bypass: bool) -> Self {
        self.config.strategies.grid.seed_orders_bypass = bypass; self
    }
    pub fn dynamic_grid_spacing(mut self, min: f64, max: f64) -> Self {
        self.config.trading.min_grid_spacing_pct = min;
        self.config.trading.max_grid_spacing_pct = max;
        self
    }
    pub fn vol_floor_resume_pct(mut self, floor: f64) -> Self {
        self.config.trading.vol_floor_resume_pct = floor; self
    }
    pub fn max_requote_attempts(mut self, n: u8) -> Self {
        self.config.execution.max_requote_attempts = n; self
    }
    pub fn stop_loss_cooldown_secs(mut self, secs: u64) -> Self {
        self.config.risk.stop_loss_cooldown_secs = secs; self
    }

    pub fn build(mut self) -> Result<Config> {
        self.config.apply_environment_defaults();
        self.config.validate()?;
        Ok(self.config)
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self { Self::new() }
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── OPT-1 tests ────────────────────────────────────────────────────────

    #[test]
    fn test_optimizer_interval_cycles_default() {
        let cfg = TradingConfig::default();
        assert_eq!(cfg.optimizer_interval_cycles, 50);
    }

    #[test]
    fn test_optimizer_interval_cycles_zero_rejected() {
        let mut cfg = TradingConfig::default();
        cfg.optimizer_interval_cycles = 0;
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("optimizer_interval_cycles"),
            "validation error should mention field name; got: {}", err);
    }

    #[test]
    fn test_optimizer_interval_cycles_serde_roundtrip() {
        let mut cfg = TradingConfig::default();
        cfg.optimizer_interval_cycles = 100;
        let toml_str = toml::to_string(&cfg).expect("serialise");
        let restored: TradingConfig = toml::from_str(&toml_str).expect("deserialise");
        assert_eq!(restored.optimizer_interval_cycles, 100);
    }

    #[test]
    fn test_optimizer_interval_cycles_absent_toml_defaults() {
        let toml_str = r#"
grid_levels = 10
grid_spacing_percent = 0.15
min_order_size = 0.1
max_position_size = 10.0
min_usdc_reserve = 100.0
min_sol_reserve = 1.0
"#;
        let cfg: TradingConfig = toml::from_str(toml_str).expect("deserialise minimal TOML");
        assert_eq!(cfg.optimizer_interval_cycles, 50);
    }

    #[test]
    fn test_builder_optimizer_interval_cycles() {
        let config = ConfigBuilder::new()
            .optimizer_interval_cycles(75)
            .build()
            .expect("build");
        assert_eq!(config.trading.optimizer_interval_cycles, 75);
    }

    // ── V5.4 FeeFilterConfig tests ──────────────────────────────────────────

    #[test]
    fn test_fee_filter_config_defaults() {
        let cfg = FeeFilterConfig::default();
        assert!(cfg.enable_smart_fee_filter);
        assert_eq!(cfg.min_fee_threshold_bps,  8);
        assert_eq!(cfg.max_fee_threshold_bps,  50);
        assert_eq!(cfg.fee_filter_window_secs, 30);
    }

    #[test]
    fn test_fee_filter_validation_rejects_min_gte_max() {
        let cfg = FeeFilterConfig {
            enable_smart_fee_filter: true,
            min_fee_threshold_bps:   50,
            max_fee_threshold_bps:   50,
            fee_filter_window_secs:  30,
        };
        let err = cfg.validate().unwrap_err();
        assert!(
            err.to_string().contains("min_fee_threshold_bps"),
            "error must mention the field; got: {}", err
        );
    }

    #[test]
    fn test_fee_filter_validation_rejects_zero_min() {
        let cfg = FeeFilterConfig {
            enable_smart_fee_filter: true,
            min_fee_threshold_bps:   0,
            max_fee_threshold_bps:   50,
            fee_filter_window_secs:  30,
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_fee_filter_serde_roundtrip() {
        let cfg = FeeFilterConfig {
            enable_smart_fee_filter: true,
            min_fee_threshold_bps:   10,
            max_fee_threshold_bps:   40,
            fee_filter_window_secs:  60,
        };
        let toml_str = toml::to_string(&cfg).expect("serialise");
        let restored: FeeFilterConfig = toml::from_str(&toml_str).expect("deserialise");
        assert_eq!(restored.min_fee_threshold_bps,  10);
        assert_eq!(restored.max_fee_threshold_bps,  40);
        assert_eq!(restored.fee_filter_window_secs, 60);
    }

    #[test]
    fn test_fee_filter_absent_section_gets_defaults() {
        let toml_str = r#"
grid_levels = 10
grid_spacing_percent = 0.15
min_order_size = 0.1
max_position_size = 10.0
min_usdc_reserve = 100.0
min_sol_reserve = 1.0
"#;
        let cfg: TradingConfig = toml::from_str(toml_str).expect("deserialise");
        assert!(cfg.fee_filter.enable_smart_fee_filter);
        assert_eq!(cfg.fee_filter.min_fee_threshold_bps,  8);
        assert_eq!(cfg.fee_filter.max_fee_threshold_bps,  50);
        assert_eq!(cfg.fee_filter.fee_filter_window_secs, 30);
    }

    #[test]
    fn test_builder_fee_filter_setter() {
        let config = ConfigBuilder::new()
            .fee_filter(5, 30, 45)
            .build()
            .expect("build");
        assert_eq!(config.trading.fee_filter.min_fee_threshold_bps,  5);
        assert_eq!(config.trading.fee_filter.max_fee_threshold_bps,  30);
        assert_eq!(config.trading.fee_filter.fee_filter_window_secs, 45);
    }

    // ── V5.5 signal_size_multiplier tests ──────────────────────────────────

    #[test]
    fn test_signal_size_multiplier_default() {
        let cfg = TradingConfig::default();
        assert!(
            (cfg.signal_size_multiplier - 1.0).abs() < 1e-9,
            "default must be 1.0, got {}", cfg.signal_size_multiplier
        );
    }

    #[test]
    fn test_signal_size_multiplier_absent_toml_defaults() {
        let toml_str = r#"
grid_levels = 10
grid_spacing_percent = 0.15
min_order_size = 0.1
max_position_size = 10.0
min_usdc_reserve = 100.0
min_sol_reserve = 1.0
"#;
        let cfg: TradingConfig = toml::from_str(toml_str).expect("deserialise minimal TOML");
        assert!(
            (cfg.signal_size_multiplier - 1.0).abs() < 1e-9,
            "absent field must default to 1.0"
        );
    }

    #[test]
    fn test_signal_size_multiplier_out_of_range_rejected() {
        let mut cfg = TradingConfig::default();
        cfg.enable_smart_position_sizing = true;
        cfg.signal_size_multiplier = 5.0;
        let err = cfg.validate().unwrap_err();
        assert!(
            err.to_string().contains("signal_size_multiplier"),
            "error must name the field; got: {}", err
        );
    }

    #[test]
    fn test_signal_size_multiplier_ignored_when_sizing_disabled() {
        let mut cfg = TradingConfig::default();
        cfg.enable_smart_position_sizing = false;
        cfg.signal_size_multiplier = 99.0;
        assert!(cfg.validate().is_ok(), "out-of-range multiplier must be ignored when flag=false");
    }

    #[test]
    fn test_builder_signal_size_multiplier_setter() {
        let config = ConfigBuilder::new()
            .signal_size_multiplier(1.5)
            .build()
            .expect("build");
        assert!(config.trading.enable_smart_position_sizing);
        assert!(
            (config.trading.signal_size_multiplier - 1.5).abs() < 1e-9,
            "multiplier must be 1.5"
        );
    }

    // ── PR #99 wma_confidence_threshold tests ──────────────────────────────

    #[test]
    fn test_wma_confidence_threshold_default() {
        let cfg = StrategiesConfig::default();
        assert!(
            (cfg.wma_confidence_threshold - 0.50).abs() < 1e-9,
            "default must be 0.50, got {}", cfg.wma_confidence_threshold
        );
    }

    #[test]
    fn test_wma_confidence_threshold_above_one_rejected() {
        let mut cfg = StrategiesConfig::default();
        cfg.wma_confidence_threshold = 1.1;
        let err = cfg.validate().unwrap_err();
        assert!(
            err.to_string().contains("wma_confidence_threshold"),
            "error must name the field; got: {}", err
        );
    }

    #[test]
    fn test_wma_confidence_threshold_negative_rejected() {
        let mut cfg = StrategiesConfig::default();
        cfg.wma_confidence_threshold = -0.1;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_wma_confidence_threshold_boundary_values_accepted() {
        let mut cfg = StrategiesConfig::default();
        cfg.wma_confidence_threshold = 0.0;
        assert!(cfg.validate().is_ok(), "0.0 must be valid");
        cfg.wma_confidence_threshold = 1.0;
        assert!(cfg.validate().is_ok(), "1.0 must be valid");
    }

    #[test]
    fn test_builder_wma_confidence_threshold_setter() {
        let config = ConfigBuilder::new()
            .wma_confidence_threshold(0.65)
            .build()
            .expect("build");
        assert!(
            (config.strategies.wma_confidence_threshold - 0.65).abs() < 1e-9,
            "threshold must be 0.65"
        );
    }

    #[test]
    fn test_wma_confidence_threshold_serde_roundtrip() {
        let mut cfg = StrategiesConfig::default();
        cfg.wma_confidence_threshold = 0.72;
        let toml_str = toml::to_string(&cfg).expect("serialise");
        let restored: StrategiesConfig = toml::from_str(&toml_str).expect("deserialise");
        assert!(
            (restored.wma_confidence_threshold - 0.72).abs() < 1e-9,
            "roundtrip must preserve 0.72"
        );
    }

    // ── PR #100 seed_orders_bypass tests ───────────────────────────────────

    #[test]
    fn test_seed_orders_bypass_default_is_true() {
        let cfg = StrategiesConfig::default();
        assert!(cfg.grid.seed_orders_bypass, "seed_orders_bypass must default to true");
    }

    #[test]
    fn test_seed_orders_bypass_absent_toml_defaults_true() {
        let toml_str = r#"
enabled = true
weight = 1.0
min_confidence = 0.5
"#;
        let cfg: GridStrategyConfig = toml::from_str(toml_str).expect("deserialise");
        assert!(cfg.seed_orders_bypass, "absent seed_orders_bypass must default to true");
    }

    #[test]
    fn test_seed_orders_bypass_false_roundtrip() {
        let original = GridStrategyConfig {
            enabled: true, weight: 1.0, min_confidence: 0.5, seed_orders_bypass: false,
        };
        let toml_str = toml::to_string(&original).expect("serialise");
        let restored: GridStrategyConfig = toml::from_str(&toml_str).expect("deserialise");
        assert!(!restored.seed_orders_bypass, "explicit false must survive serde roundtrip");
    }

    #[test]
    fn test_builder_seed_orders_bypass_setter() {
        let config = ConfigBuilder::new()
            .seed_orders_bypass(false)
            .build()
            .expect("build");
        assert!(!config.strategies.grid.seed_orders_bypass);
    }

    // ── PR #107 max/min_grid_spacing_pct tests ─────────────────────────────

    #[test]
    fn test_grid_spacing_bounds_defaults() {
        let cfg = TradingConfig::default();
        assert!((cfg.max_grid_spacing_pct - 0.0075).abs() < 1e-9);
        assert!((cfg.min_grid_spacing_pct - 0.001).abs() < 1e-9);
    }

    #[test]
    fn test_grid_spacing_bounds_min_gte_max_rejected() {
        let mut cfg = TradingConfig::default();
        cfg.enable_dynamic_grid = true;
        cfg.min_grid_spacing_pct = 0.0075;
        cfg.max_grid_spacing_pct = 0.0075;
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("min_grid_spacing_pct"));
    }

    #[test]
    fn test_grid_spacing_bounds_skipped_when_dynamic_disabled() {
        let mut cfg = TradingConfig::default();
        cfg.enable_dynamic_grid = false;
        cfg.min_grid_spacing_pct = 0.01;
        cfg.max_grid_spacing_pct = 0.005;
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_builder_dynamic_grid_spacing_setter() {
        let config = ConfigBuilder::new()
            .dynamic_grid_spacing(0.002, 0.01)
            .build()
            .expect("build");
        assert!((config.trading.min_grid_spacing_pct - 0.002).abs() < 1e-9);
        assert!((config.trading.max_grid_spacing_pct - 0.010).abs() < 1e-9);
    }

    // ── V6.0 vol_floor_resume_pct tests ────────────────────────────────────

    #[test]
    fn test_vol_floor_resume_pct_default() {
        let cfg = TradingConfig::default();
        assert!((cfg.vol_floor_resume_pct - 0.05).abs() < 1e-9);
    }

    #[test]
    fn test_vol_floor_resume_pct_serde_roundtrip() {
        let mut cfg = TradingConfig::default();
        cfg.vol_floor_resume_pct = 0.10;
        let toml_str = toml::to_string(&cfg).expect("serialise");
        let restored: TradingConfig = toml::from_str(&toml_str).expect("deserialise");
        assert!((restored.vol_floor_resume_pct - 0.10).abs() < 1e-9);
    }

    #[test]
    fn test_production_env_respects_vol_floor_resume_pct() {
        let mut cfg = TradingConfig::default();
        cfg.vol_floor_resume_pct = 0.05;
        cfg.min_volatility_to_trade = 0.01;
        cfg.apply_environment("production");
        assert!((cfg.min_volatility_to_trade - 0.05).abs() < 1e-9);
    }

    #[test]
    fn test_production_env_does_not_stomp_above_floor() {
        let mut cfg = TradingConfig::default();
        cfg.vol_floor_resume_pct = 0.05;
        cfg.min_volatility_to_trade = 0.20;
        cfg.apply_environment("production");
        assert!((cfg.min_volatility_to_trade - 0.20).abs() < 1e-9);
    }

    // ── V6.1 max_requote_attempts tests ────────────────────────────────────

    #[test]
    fn test_max_requote_attempts_default() {
        let cfg = ExecutionConfig::default();
        assert_eq!(cfg.max_requote_attempts, 3);
    }

    #[test]
    fn test_max_requote_attempts_zero_rejected() {
        let mut cfg = ExecutionConfig::default();
        cfg.max_requote_attempts = 0;
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("max_requote_attempts"));
    }

    #[test]
    fn test_max_requote_attempts_serde_roundtrip() {
        let mut cfg = ExecutionConfig::default();
        cfg.max_requote_attempts = 5;
        let toml_str = toml::to_string(&cfg).expect("serialise");
        let restored: ExecutionConfig = toml::from_str(&toml_str).expect("deserialise");
        assert_eq!(restored.max_requote_attempts, 5);
    }

    #[test]
    fn test_builder_max_requote_attempts_setter() {
        let config = ConfigBuilder::new()
            .max_requote_attempts(4)
            .build()
            .expect("build");
        assert_eq!(config.execution.max_requote_attempts, 4);
    }

    // ── V6.1 PR #126 C2: stop_loss_cooldown_secs tests ─────────────────────

    #[test]
    fn test_stop_loss_cooldown_secs_default() {
        let builder = ConfigBuilder::new().build().expect("build");
        assert_eq!(builder.risk.stop_loss_cooldown_secs, 300);
    }

    #[test]
    fn test_stop_loss_cooldown_secs_absent_toml_defaults() {
        let toml_str = r#"
max_position_size_pct = 30.0
max_drawdown_pct = 10.0
stop_loss_pct = 5.0
take_profit_pct = 10.0
circuit_breaker_threshold_pct = 8.0
circuit_breaker_cooldown_secs = 300
"#;
        let cfg: RiskConfig = toml::from_str(toml_str).expect("deserialise RiskConfig");
        assert_eq!(cfg.stop_loss_cooldown_secs, 300);
    }

    #[test]
    fn test_stop_loss_cooldown_secs_serde_roundtrip() {
        let config = ConfigBuilder::new()
            .stop_loss_cooldown_secs(600)
            .build()
            .expect("build");
        let toml_str = toml::to_string(&config.risk).expect("serialise");
        let restored: RiskConfig = toml::from_str(&toml_str).expect("deserialise");
        assert_eq!(restored.stop_loss_cooldown_secs, 600);
    }

    #[test]
    fn test_builder_stop_loss_cooldown_secs_setter() {
        let config = ConfigBuilder::new()
            .stop_loss_cooldown_secs(120)
            .build()
            .expect("build");
        assert_eq!(config.risk.stop_loss_cooldown_secs, 120);
    }
}
