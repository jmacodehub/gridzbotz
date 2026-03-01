//! ═══════════════════════════════════════════════════════════════════════════
//! 🎛️  UNIFIED CONFIGURATION SYSTEM V5.1 - GRIDZBOTZ
//!
//! Stage 2: Per-Strategy Tuning Params Wired to TOML
//!
//! V5.1 ADDITIONS (Stage 2 — Mar 1, 2026):
//! ✅ RsiStrategyConfig: extreme_oversold + extreme_overbought (defaults: 20.0 / 80.0)
//! ✅ MeanReversionStrategyConfig: 4 signal thresholds (strong_buy/sell 5.0, buy/sell 2.5)
//! ✅ MomentumMACDStrategyConfig: full TOML config for momentum_macd strategy
//!    - min_confidence, strong_histogram_threshold, min_warmup_periods
//! ✅ to_rsi_params() / to_mean_reversion_params() / to_momentum_macd_params() helpers
//!    — bridge from TOML config → new_from_config() constructors (PR #27)
//! ✅ All new fields have serde(default) — zero breaking changes
//! ✅ StrategiesConfig::validate() updated to include momentum_macd weight
//! ✅ MomentumStrategyConfig.slow_period wired (V5.1 cleanup)
//! ✅ to_momentum_params() + MomentumParams mirror struct added
//! ✅ RESERVED field annotations for operator clarity
//!
//! V5.0 ADDITIONS (Stage 1 — Feb 23, 2026):
//! ✅ execution_mode: "paper" | "live" — one TOML line to flip paper → live
//! ✅ instance_id: unique bot identifier for multi-instance runs (1-5 bots)
//! ✅ ExecutionConfig: all live Jupiter/RPC execution knobs exposed to TOML
//!    - max_trade_sol, priority_fee_microlamports, max_slippage_bps
//!    - jito_tip_lamports (optional MEV protection)
//!    - rpc_fallback_urls, confirmation_timeout_secs, max_retries
//! ✅ BotConfig::is_live() / is_paper() / instance_name() helpers
//! ✅ ExecutionConfig::slippage_pct() / jito_enabled() helpers
//! ✅ Execution validation only runs when mode = "live" (safe for paper)
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
//! • `Config`        - Main TOML-based configuration
//! • `ConfigBuilder` - Programmatic builder for tests
//! • `ConfigPresets` - Pre-configured scenarios (conservative, aggressive, etc.)
//! • Environment-specific overrides
//! • Comprehensive validation
//!
//! March 1, 2026 - V5.1 STAGE 2: PER-STRATEGY TUNING PARAMS 🚀
//! ═══════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use anyhow::{Result, Context, bail};
use std::path::{Path};
use std::fs;
use log::{info, warn};

// ═══════════════════════════════════════════════════════════════════════════
// MAIN CONFIGURATION - The Heart of GridzBotz
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

    /// Live execution settings (Jupiter, priority fees, slippage)
    /// Active when bot.execution_mode = "live"
    #[serde(default)]
    pub execution: ExecutionConfig,

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

// ... (all structs from BotConfig through StrategiesConfig remain identical) ...

// ═══════════════════════════════════════════════════════════════════════════
// BOT CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BotConfig {
    /// Bot name (e.g., "GridzBot-Live-1")
    pub name: String,

    /// Bot version (e.g., "5.1.0")
    pub version: String,

    /// Environment: "testing", "development", "production"
    /// This controls safety features and default behaviors
    pub environment: String,

    /// Execution mode: "paper" | "live"
    /// Routes to PaperTradingEngine or RealTradingEngine at startup.
    /// Change to "live" when ready to trade real funds on-chain.
    #[serde(default = "default_paper_mode")]
    pub execution_mode: String,

    /// Unique instance identifier for multi-bot runs.
    /// e.g., "live-aggressive", "test-shadow-A", "bot-conservative-1"
    /// Used as log prefix and metrics label. Defaults to bot.name if not set.
    #[serde(default)]
    pub instance_id: Option<String>,
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

    /// Returns true if this instance is trading live (real Jupiter swaps on-chain)
    pub fn is_live(&self) -> bool {
        self.execution_mode == "live"
    }

    /// Returns true if this instance is in paper/simulation mode
    pub fn is_paper(&self) -> bool {
        self.execution_mode != "live"
    }

    /// Returns the instance name for log prefixes and metrics labels.
    /// Uses instance_id if set, falls back to bot name.
    pub fn instance_name(&self) -> &str {
        self.instance_id.as_deref().unwrap_or(&self.name)
    }

    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            bail!("Bot name cannot be empty");
        }
        if self.version.is_empty() {
            bail!("Bot version cannot be empty");
        }
        let valid_modes = ["paper", "live"];
        if !valid_modes.contains(&self.execution_mode.as_str()) {
            bail!(
                "Invalid execution_mode '{}'. Must be one of: {:?}",
                self.execution_mode, valid_modes
            );
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

// ... (NetworkConfig, ExecutionConfig, TradingConfig, RegimeGateConfig all remain identical) ...

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

    /// 🆕 V5.1: Momentum MACD strategy configuration
    #[serde(default)]
    pub momentum_macd: MomentumMACDStrategyConfig,

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
        if self.momentum_macd.enabled {
            total_weight += self.momentum_macd.weight;
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
            momentum_macd: MomentumMACDStrategyConfig::default(),
            enable_multi_timeframe: false,
            require_timeframe_alignment: false,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Strategy sub-configs
// V5.1: Existing configs extended with per-strategy tuning params.
//       All new fields have serde(default) — existing TOMLs parse unchanged.
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GridStrategyConfig {
    pub enabled: bool,
    pub weight: f64,
    #[serde(default = "default_confidence")]
    pub min_confidence: f64,
}

/// Momentum strategy config.
///
/// V5.1 cleanup: `slow_period` now wired from TOML → `MomentumConfig.slow_period`.
/// Usage: call `to_momentum_params()` to get a `MomentumParams`
/// suitable for `MomentumStrategy::new_from_config()`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MomentumStrategyConfig {
    pub enabled: bool,
    pub weight: f64,
    pub min_confidence: f64,
    /// Fast MA lookback period — maps to `MomentumConfig.fast_period`.
    pub lookback_period: usize,
    /// Slow MA lookback period — maps to `MomentumConfig.slow_period`. Default: 50.
    #[serde(default = "default_momentum_slow_period")]
    pub slow_period: usize,
    /// Minimum momentum threshold to emit a signal.
    ///
    /// ⚠️ RESERVED — internal `MomentumStrategy` constant; **not** in `MomentumConfig`.
    /// Setting this in TOML currently has no effect. Kept for future promotion.
    pub threshold: f64,
}

/// Mean reversion strategy config.
///
/// V5.1 additions (all optional with sensible defaults):
/// - `strong_buy_threshold`  — deviation % for StrongBuy  (default 5.0)
/// - `buy_threshold`         — deviation % for Buy        (default 2.5)
/// - `strong_sell_threshold` — deviation % for StrongSell (default 5.0)
/// - `sell_threshold`        — deviation % for Sell       (default 2.5)
///
/// Usage: call `to_mean_reversion_params()` to get a `MeanReversionConfig`
/// suitable for `MeanReversionStrategy::new_from_config()`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MeanReversionStrategyConfig {
    pub enabled: bool,
    pub weight: f64,
    pub min_confidence: f64,
    pub sma_period: usize,
    /// Bollinger band width multiplier (e.g. 2.0 = 2 std-dev bands).
    ///
    /// ⚠️ RESERVED — `MeanReversionConfig` uses threshold-based signals, not Bollinger bands.
    /// Setting this in TOML currently has no effect. Kept for future promotion.
    pub std_dev_multiplier: f64,

    // ── V5.1: signal threshold tuning ──────────────────────────────────────
    /// Deviation % above mean to trigger StrongBuy signal (default: 5.0)
    #[serde(default = "default_strong_threshold")]
    pub strong_buy_threshold: f64,

    /// Deviation % above mean to trigger Buy signal (default: 2.5)
    #[serde(default = "default_normal_threshold")]
    pub buy_threshold: f64,

    /// Deviation % below mean to trigger StrongSell signal (default: 5.0)
    #[serde(default = "default_strong_threshold")]
    pub strong_sell_threshold: f64,

    /// Deviation % below mean to trigger Sell signal (default: 2.5)
    #[serde(default = "default_normal_threshold")]
    pub sell_threshold: f64,
}

/// RSI strategy config.
///
/// V5.1 additions (all optional with sensible defaults):
/// - `extreme_oversold`   — RSI level for StrongBuy  (default 20.0)
/// - `extreme_overbought` — RSI level for StrongSell (default 80.0)
///
/// Usage: call `to_rsi_params()` to get an `RsiConfig`
/// suitable for `RsiStrategy::new_from_config()`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RsiStrategyConfig {
    pub enabled: bool,
    pub weight: f64,
    pub min_confidence: f64,
    pub period: usize,
    pub oversold_threshold: f64,
    pub overbought_threshold: f64,

    // ── V5.1: extreme zone tuning ───────────────────────────────────────────
    /// RSI level that triggers StrongBuy (deeper oversold). Default: 20.0
    #[serde(default = "default_extreme_oversold")]
    pub extreme_oversold: f64,

    /// RSI level that triggers StrongSell (deeper overbought). Default: 80.0
    #[serde(default = "default_extreme_overbought")]
    pub extreme_overbought: f64,
}

/// 🆕 V5.1: Momentum MACD strategy config.
///
/// Controls the MACD-based momentum strategy introduced in PR #27.
/// Corresponds to `MomentumMACDConfig` in `src/strategies/momentum_macd.rs`.
///
/// Usage: call `to_momentum_macd_params()` → pass to
/// `MomentumMACDStrategy::new_from_config()`.
///
/// # Example (config/master.toml)
/// ```toml
/// [strategies.momentum_macd]
/// enabled = true
/// weight = 0.8
/// min_confidence = 0.65
/// strong_histogram_threshold = 0.5
/// min_warmup_periods = 26
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MomentumMACDStrategyConfig {
    /// Include this strategy in the consensus vote
    pub enabled: bool,

    /// Consensus weight (relative to other strategies)
    pub weight: f64,

    /// Minimum confidence to emit a non-Hold signal (0.0–1.0)
    #[serde(default = "default_macd_min_confidence")]
    pub min_confidence: f64,

    /// MACD histogram threshold to qualify as a "strong" signal
    /// Values > this trigger StrongBuy/StrongSell vs plain Buy/Sell
    #[serde(default = "default_macd_histogram_threshold")]
    pub strong_histogram_threshold: f64,

    /// Minimum price ticks before emitting signals (warmup guard)
    /// Must be >= 26 (slow MACD period) for meaningful values
    #[serde(default = "default_macd_warmup_periods")]
    pub min_warmup_periods: usize,
}

// ── V5.1: Conversion helpers (config → strategy constructors) ─────────────
//
// These provide the bridge from TOML config → new_from_config() constructors
// (PR #27). Call them in bot initialisation when constructing strategies:
//
//   let rsi = RsiStrategy::new_from_config(config.strategies.rsi.to_rsi_params());
//   let mr  = MeanReversionStrategy::new_from_config(
//                 config.strategies.mean_reversion.to_mean_reversion_params());
//   let mmacd = MomentumMACDStrategy::new_from_config(
//                 config.strategies.momentum_macd.to_momentum_macd_params());
//   let momentum = MomentumStrategy::new_from_config(
//                 config.strategies.momentum.to_momentum_params());
// ─────────────────────────────────────────────────────────────────────────

impl RsiStrategyConfig {
    /// Convert to the `RsiConfig` expected by `RsiStrategy::new_from_config()`.
    pub fn to_rsi_params(&self) -> RsiParams {
        RsiParams {
            rsi_period: self.period,
            oversold_threshold: self.oversold_threshold,
            overbought_threshold: self.overbought_threshold,
            extreme_oversold: self.extreme_oversold,
            extreme_overbought: self.extreme_overbought,
        }
    }
}

impl MeanReversionStrategyConfig {
    /// Convert to the `MeanReversionConfig` expected by
    /// `MeanReversionStrategy::new_from_config()`.
    pub fn to_mean_reversion_params(&self) -> MeanReversionParams {
        MeanReversionParams {
            mean_period: self.sma_period,
            strong_buy_threshold: self.strong_buy_threshold,
            buy_threshold: self.buy_threshold,
            strong_sell_threshold: self.strong_sell_threshold,
            sell_threshold: self.sell_threshold,
            min_confidence: self.min_confidence,
        }
    }
}

impl MomentumMACDStrategyConfig {
    /// Convert to the `MomentumMACDConfig` expected by
    /// `MomentumMACDStrategy::new_from_config()`.
    pub fn to_momentum_macd_params(&self) -> MomentumMACDParams {
        MomentumMACDParams {
            min_confidence: self.min_confidence,
            strong_histogram_threshold: self.strong_histogram_threshold,
            min_warmup_periods: self.min_warmup_periods,
        }
    }
}

impl MomentumStrategyConfig {
    /// Convert to `MomentumParams` for `MomentumStrategy::new_from_config()`.
    ///
    /// TOML → `MomentumConfig` field map:
    /// - `lookback_period` → `fast_period`    (fast MA lookback)
    /// - `slow_period`     → `slow_period`    (slow MA lookback, default 50)
    /// - `min_confidence`  → `min_confidence`
    ///
    /// Note: `threshold` is an internal constant — not exposed in `MomentumConfig`.
    pub fn to_momentum_params(&self) -> MomentumParams {
        MomentumParams {
            fast_period:    self.lookback_period,
            slow_period:    self.slow_period,
            min_confidence: self.min_confidence,
        }
    }
}

// ── V5.1: Param mirror structs ────────────────────────────────────────────
//
// Plain-data structs that mirror the Config structs in
// src/strategies/{rsi,mean_reversion,momentum_macd,momentum}.rs.
//
// Why mirror instead of referencing directly?
//   • Keeps src/config free of src/strategies dependencies.
//   • The call site (bot init) owns the conversion; config stays pure data.
//   • If a strategy changes its internal Config, only the call site updates.
// ─────────────────────────────────────────────────────────────────────────

/// Mirror of `RsiConfig` in `src/strategies/rsi.rs`.
/// Passed to `RsiStrategy::new_from_config()` at bot startup.
#[derive(Debug, Clone)]
pub struct RsiParams {
    pub rsi_period: usize,
    pub oversold_threshold: f64,
    pub overbought_threshold: f64,
    pub extreme_oversold: f64,
    pub extreme_overbought: f64,
}

/// Mirror of `MeanReversionConfig` in `src/strategies/mean_reversion.rs`.
/// Passed to `MeanReversionStrategy::new_from_config()` at bot startup.
#[derive(Debug, Clone)]
pub struct MeanReversionParams {
    pub mean_period: usize,
    pub strong_buy_threshold: f64,
    pub buy_threshold: f64,
    pub strong_sell_threshold: f64,
    pub sell_threshold: f64,
    pub min_confidence: f64,
}

/// Mirror of `MomentumMACDConfig` in `src/strategies/momentum_macd.rs`.
/// Passed to `MomentumMACDStrategy::new_from_config()` at bot startup.
#[derive(Debug, Clone)]
pub struct MomentumMACDParams {
    pub min_confidence: f64,
    pub strong_histogram_threshold: f64,
    pub min_warmup_periods: usize,
}

/// Mirror of `MomentumConfig` in `src/strategies/momentum.rs`.
/// Passed to `MomentumStrategy::new_from_config()` at bot startup.
#[derive(Debug, Clone)]
pub struct MomentumParams {
    /// Fast MA lookback period
    pub fast_period: usize,
    /// Slow MA lookback period
    pub slow_period: usize,
    /// Minimum confidence to emit a non-Hold signal (0.0–1.0)
    pub min_confidence: f64,
}

// ─────────────────────────────────────────────────────────────────────────
// Default impls for strategy sub-configs
// ─────────────────────────────────────────────────────────────────────────

impl Default for MomentumStrategyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            weight: 0.8,
            min_confidence: 0.6,
            lookback_period: 20,
            slow_period: default_momentum_slow_period(),
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
            strong_buy_threshold: default_strong_threshold(),
            buy_threshold: default_normal_threshold(),
            strong_sell_threshold: default_strong_threshold(),
            sell_threshold: default_normal_threshold(),
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
            extreme_oversold: default_extreme_oversold(),
            extreme_overbought: default_extreme_overbought(),
        }
    }
}

impl Default for MomentumMACDStrategyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            weight: 0.8,
            min_confidence: default_macd_min_confidence(),
            strong_histogram_threshold: default_macd_histogram_threshold(),
            min_warmup_periods: default_macd_warmup_periods(),
        }
    }
}

// ... (RiskConfig, PythConfig, PerformanceConfig, etc. all remain identical) ...
// ... (default helper functions — add new one for momentum slow_period) ...

// ═══════════════════════════════════════════════════════════════════════════
// DEFAULT VALUE HELPERS
// ═══════════════════════════════════════════════════════════════════════════

// --- Existing defaults ---
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

// --- V5.0 Stage 1: Execution defaults ---
fn default_paper_mode() -> String { "paper".to_string() }
fn default_max_trade_sol() -> f64 { 0.5 }
fn default_priority_fee_microlamports() -> u64 { 50_000 }
fn default_slippage_bps() -> u16 { 100 }
fn default_confirm_timeout_secs() -> u64 { 60 }
fn default_max_tx_retries() -> u8 { 3 }

// --- V5.1 Stage 2: Per-strategy tuning defaults ---
fn default_extreme_oversold() -> f64 { 20.0 }
fn default_extreme_overbought() -> f64 { 80.0 }
fn default_strong_threshold() -> f64 { 5.0 }
fn default_normal_threshold() -> f64 { 2.5 }
fn default_macd_min_confidence() -> f64 { 0.65 }
fn default_macd_histogram_threshold() -> f64 { 0.5 }
fn default_macd_warmup_periods() -> usize { 26 }
fn default_momentum_slow_period() -> usize { 50 }

// ... (Config impl, ConfigBuilder, all remaining code identical) ...