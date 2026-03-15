//! ═══════════════════════════════════════════════════════════════════════════
//! 🎛️  UNIFIED CONFIGURATION SYSTEM V6.0 - GRIDZBOTZ
//!
//! V5.9 ADDITIONS (fix/wire-resolve-secrets-into-from-file):
//! ✅ Config::from_file(): secrets::resolve_secrets() now called automatically
//!    - Inserted between apply_environment_defaults() and validate()
//!    - Env vars GRIDZBOTZ_RPC_URL / GRIDZBOTZ_WALLET_PATH /
//!      GRIDZBOTZ_JUPITER_API_KEY / GRIDZBOTZ_JITO_TIP_LAMPORTS /
//!      GRIDZBOTZ_FALLBACK_RPC_URL override TOML values at load time
//!    - Paper mode: missing env vars tolerated (warn only)
//!    - Live mode:  GRIDZBOTZ_JUPITER_API_KEY required; wallet + RPC enforced
//!    - Zero breaking changes — all 46 TOMLs, all tests unchanged
//!
//! V5.8 ADDITIONS (PR #107 — fix/fee-reconciliation):
//! ✅ TradingConfig: max_grid_spacing_pct + min_grid_spacing_pct added
//!    - Replace hardcoded max_spacing=0.0075, min_spacing=0.001 in
//!      GridRebalancerConfig init (grid_bot.rs) — Commit 2 wires them in
//!    - Default: 0.0075 / 0.001 — all 46 TOMLs unchanged (serde default)
//!    - Validation: min < max, both positive — gated on enable_dynamic_grid
//!    - Zero breaking changes
//!
//! V5.7 ADDITIONS (PR #100 — fix/grid-seed-bypass):
//! ✅ GridStrategyConfig: seed_orders_bypass added
//!    - Mirrors GridRebalancerConfig.seed_orders_bypass (default: true)
//!    - Without this, TOML key `strategies.grid.seed_orders_bypass = false`
//!      was rejected by deny_unknown_fields → startup crash
//!    - Default: true — all 46 existing TOMLs parse unchanged
//!    - Validation: none needed (bool)
//!    - Zero breaking changes
//!
//! V5.6 ADDITIONS (PR #99 — Commit 1):
//! ✅ StrategiesConfig: wma_confidence_threshold added
//!    - Config-driven WMA confidence gate (was hardcoded 0.65 in consensus_wma.rs)
//!    - Default: 0.50 — permissive, matches strategies.grid.min_confidence
//!    - Validation: must be in [0.0, 1.0]
//!    - Zero breaking changes — all 46 TOMLs parse unchanged (serde default)
//!
//! V5.5 ADDITIONS (PR #94 — Commit 5a):
//! ✅ TradingConfig: signal_size_multiplier added
//!    - Controls how strongly consensus signal strength scales order size
//!    - Default: 1.0 (flat — zero effect, safe opt-in)
//!    - Active only when enable_smart_position_sizing = true
//!    - Formula: effective_size = base_size * (1.0 + strength * (multiplier - 1.0))
//!    - strength() from Signal enum: Hold=0.0, Buy/Sell=0.25–0.5, StrongBuy/Sell=0.5–1.0
//!    - Validation: must be in [0.5, 3.0] when smart sizing is enabled
//!    - Zero breaking changes — all 46 TOMLs parse unchanged (serde default)
//!
//! V5.4 ADDITIONS (PR #94 — Commit 3):
//! ✅ FeeFilterConfig sub-section added under [trading.fee_filter]
//!    - min_fee_threshold_bps (default 8)  — skip grid levels below this
//!    - max_fee_threshold_bps (default 50) — skip grid levels above this
//!    - fee_filter_window_secs (default 30) — rolling avg fee window
//!    - enable_smart_fee_filter (default true)
//!    - FeeFilterConfig::validate(): bail if min >= max, warn if window < 5s
//!    - Zero breaking changes — all 46 TOMLs parse unchanged (serde default)
//!
//! V5.3 ADDITIONS (PR #94 — Commit 1):
//! ✅ TradingConfig: optimizer_interval_cycles added (OPT-1)
//!    - Replaces hardcoded `const OPTIMIZATION_INTERVAL_CYCLES = 50` in grid_bot.rs
//!    - Default: 50 cycles — all 46 existing TOMLs parse unchanged
//!    - Validation: bail if == 0
//!    - Wired into grid_bot.rs Commit 2
//!
//! V5.2 ADDITIONS (PR #89 — fix/risk-continuous-sl-monitoring):
//! ✅ RiskConfig: enable_trailing_stop added (default: false)
//!
//! V5.1 ADDITIONS (Stage 2 — Mar 1, 2026):
//! ✅ RsiStrategyConfig: extreme_oversold + extreme_overbought
//! ✅ MeanReversionStrategyConfig: 4 signal thresholds
//! ✅ MomentumMACDStrategyConfig: full TOML config
//! ✅ to_rsi_params() / to_mean_reversion_params() / to_momentum_macd_params()
//!
//! V5.0 ADDITIONS (Stage 1 — Feb 23, 2026):
//! ✅ execution_mode, instance_id, ExecutionConfig, BotConfig helpers
//!
//! March 14, 2026 - V6.0: vol_floor_resume_pct — configurable prod vol floor (PR fix/regime-gate-configurable-vol-floor) 🎛️
//! March 13, 2026 - V5.9: resolve_secrets wired into from_file() 🔐
//! March 13, 2026 - V5.8: max/min_grid_spacing_pct added (PR #107 Commit 1) 📐
//! March 12, 2026 - V5.7: seed_orders_bypass added to GridStrategyConfig (PR #100) 🔧
//! March 11, 2026 - V5.6: wma_confidence_threshold added (PR #99 Commit 1) 🎯
//! March 11, 2026 - V5.5: signal_size_multiplier added (PR #94 Commit 5a) 📐
//! March 10, 2026 - V5.4: FeeFilterConfig added (PR #94 Commit 3) 🔥
//! March 10, 2026 - V5.3: optimizer_interval_cycles added (PR #94 OPT-1) ⚙️
//! March 10, 2026 - V5.2: enable_trailing_stop added to RiskConfig 🛑
//! ═══════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use anyhow::{Result, Context, bail};
use std::path::{Path};
use std::fs;
use log::{info, warn};
pub mod secrets;
pub mod fees;
pub use fees::FeesConfig;
pub mod priority_fees;
pub use priority_fees::PriorityFeeConfig;

// ═══════════════════════════════════════════════════════════════════════════
// MAIN CONFIGURATION
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
// BOT CONFIGURATION
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
    pub fn is_production(&self) -> bool { self.environment == "production" }
    pub fn is_testing(&self) -> bool    { self.environment == "testing" }
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
// NETWORK CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkConfig {
    pub cluster: String,
    pub rpc_url: String,
    pub commitment: String,
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
// SECURITY CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SecurityConfig {
    #[serde(default = "default_wallet_path")]
    pub wallet_path: String,
    #[serde(default)]
    pub require_password: bool,
    #[serde(default)]
    pub authorized_programs: Option<Vec<String>>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            wallet_path: default_wallet_path(),
            require_password: false,
            authorized_programs: None,
        }
    }
}

impl SecurityConfig {
    pub fn validate(&self) -> Result<()> {
        if self.wallet_path.is_empty() {
            bail!("security.wallet_path cannot be empty");
        }
        Ok(())
    }

    pub fn validate_for_live_mode(&self) -> Result<()> {
        use std::path::PathBuf;
        let expanded_path = if self.wallet_path.starts_with('~') {
            let home = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .context("Cannot determine home directory for ~ expansion")?;
            PathBuf::from(self.wallet_path.replacen('~', &home, 1))
        } else {
            PathBuf::from(&self.wallet_path)
        };
        if !expanded_path.exists() {
            bail!(
                "Wallet file not found: {}\n\
                 Ensure security.wallet_path in your config points to a valid keypair file.",
                expanded_path.display()
            );
        }
        if let Err(e) = fs::File::open(&expanded_path) {
            bail!(
                "Wallet file exists but cannot be read: {}\n\
                 Error: {}\n\
                 Check file permissions and ownership.",
                expanded_path.display(), e
            );
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = fs::metadata(&expanded_path) {
                let mode = metadata.permissions().mode();
                if mode & 0o004 != 0 {
                    warn!(
                        "⚠️ SECURITY: Wallet file is world-readable: {}\n\
                         Fix with: chmod 600 {}",
                        expanded_path.display(), expanded_path.display()
                    );
                }
            }
        }
        info!("✅ Wallet file validated: {}", expanded_path.display());
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// EXECUTION CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionConfig {
    #[serde(default = "default_max_trade_sol")]
    pub max_trade_sol: f64,
    #[serde(default = "default_max_trade_size_usdc")]
    pub max_trade_size_usdc: f64,
    #[serde(default = "default_priority_fee_microlamports")]
    pub priority_fee_microlamports: u64,
    #[serde(default = "default_slippage_bps")]
    pub max_slippage_bps: u16,
    #[serde(default)]
    pub jito_tip_lamports: Option<u64>,
    #[serde(default)]
    pub rpc_fallback_urls: Option<Vec<String>>,
    #[serde(default = "default_confirm_timeout_secs")]
    pub confirmation_timeout_secs: u64,
    #[serde(default = "default_max_tx_retries")]
    pub max_retries: u8,
    #[serde(default = "default_max_requote_attempts")]
    pub max_requote_attempts: u8,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            max_trade_sol: default_max_trade_sol(),
            max_trade_size_usdc: default_max_trade_size_usdc(),
            priority_fee_microlamports: default_priority_fee_microlamports(),
            max_slippage_bps: default_slippage_bps(),
            jito_tip_lamports: None,
            rpc_fallback_urls: None,
            confirmation_timeout_secs: default_confirm_timeout_secs(),
            max_retries: default_max_tx_retries(),
            max_requote_attempts: default_max_requote_attempts(),
        }
    }
}

impl ExecutionConfig {
    pub fn validate(&self) -> Result<()> {
        if self.max_trade_sol <= 0.0 {
            bail!("execution.max_trade_sol must be positive");
        }
        if self.max_trade_sol > 100.0 {
            warn!("⚠️ execution.max_trade_sol ({:.2}) is very large — double-check capital allocation",
                  self.max_trade_sol);
        }
        if self.max_slippage_bps == 0 {
            bail!("execution.max_slippage_bps cannot be 0 — Jupiter requires > 0 BPS");
        }
        if self.max_slippage_bps > 500 {
            warn!("⚠️ execution.max_slippage_bps ({}) > 5% — very high slippage tolerance!",
                  self.max_slippage_bps);
        }
        if self.confirmation_timeout_secs == 0 {
            bail!("execution.confirmation_timeout_secs must be > 0");
        }
        if self.max_retries == 0 {
            warn!("⚠️ execution.max_retries = 0 — failed txs will NOT be retried");
        }
                if self.max_requote_attempts == 0 {
            bail!("execution.max_requote_attempts must be > 0");
        }
        Ok(())
    }

    pub fn slippage_pct(&self) -> f64 { self.max_slippage_bps as f64 / 100.0 }
    pub fn jito_enabled(&self) -> bool { self.jito_tip_lamports.is_some() }
}

// ═══════════════════════════════════════════════════════════════════════════
// 🆕 V5.4 (PR #94 Commit 3): FEE FILTER CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

/// Per-level fee filtering configuration for the SmartFeeFilter.
///
/// Controls which grid levels are placed based on their projected fee cost
/// relative to the expected spread capture. Levels whose estimated fee
/// (in basis points) falls outside [min, max] are silently skipped.
///
/// Owned by `TradingConfig` under the `[trading.fee_filter]` TOML sub-section.
/// All fields have `serde(default)` — omitting the entire section is fine.
///
/// # Example (config/master.toml)
/// ```toml
/// [trading.fee_filter]
/// enable_smart_fee_filter  = true
/// min_fee_threshold_bps    = 8    # skip levels that are too cheap to be real
/// max_fee_threshold_bps    = 50   # skip levels that eat the whole spread
/// fee_filter_window_secs   = 30   # rolling fee average window
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FeeFilterConfig {
    /// Master on/off switch for the SmartFeeFilter.
    /// When false, all grid levels are placed regardless of fee cost.
    /// Default: true.
    #[serde(default = "default_true")]
    pub enable_smart_fee_filter: bool,

    /// Minimum acceptable fee in basis points (BPS).
    /// Levels with estimated fee < this are suspiciously cheap
    /// (stale quote, bad route) and are skipped.
    /// Default: 8 BPS (0.08%).
    #[serde(default = "default_min_fee_threshold_bps")]
    pub min_fee_threshold_bps: u32,

    /// Maximum acceptable fee in basis points (BPS).
    /// Levels with estimated fee > this consume the entire spread
    /// and are skipped as unprofitable.
    /// Default: 50 BPS (0.50%).
    #[serde(default = "default_max_fee_threshold_bps")]
    pub max_fee_threshold_bps: u32,

    /// Rolling window for computing the moving-average fee baseline
    /// (seconds). Shorter = more reactive; longer = more stable.
    /// Default: 30 seconds.
    #[serde(default = "default_fee_filter_window_secs")]
    pub fee_filter_window_secs: u64,
}

impl Default for FeeFilterConfig {
    fn default() -> Self {
        Self {
            enable_smart_fee_filter: true,
            min_fee_threshold_bps:   default_min_fee_threshold_bps(),
            max_fee_threshold_bps:   default_max_fee_threshold_bps(),
            fee_filter_window_secs:  default_fee_filter_window_secs(),
        }
    }
}

impl FeeFilterConfig {
    pub fn validate(&self) -> Result<()> {
        if self.enable_smart_fee_filter {
            if self.min_fee_threshold_bps == 0 {
                bail!("trading.fee_filter.min_fee_threshold_bps must be > 0");
            }
            if self.min_fee_threshold_bps >= self.max_fee_threshold_bps {
                bail!(
                    "trading.fee_filter.min_fee_threshold_bps ({}) must be < \
                     max_fee_threshold_bps ({})",
                    self.min_fee_threshold_bps, self.max_fee_threshold_bps
                );
            }
            if self.max_fee_threshold_bps > 500 {
                warn!(
                    "⚠️ fee_filter.max_fee_threshold_bps ({}) > 500 BPS (5%) — \
                     very permissive fee ceiling",
                    self.max_fee_threshold_bps
                );
            }
            if self.fee_filter_window_secs < 5 {
                warn!(
                    "⚠️ fee_filter.fee_filter_window_secs ({}) < 5s — \
                     fee average may be too noisy",
                    self.fee_filter_window_secs
                );
            }
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TRADING CONFIGURATION - V5.9 ENHANCED! 🔐
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TradingConfig {
    // ─────────────────────────────────────────────────────────────────────────
    // Core Grid Settings (Required)
    // ─────────────────────────────────────────────────────────────────────────
    pub grid_levels: u32,
    pub grid_spacing_percent: f64,
    pub min_order_size: f64,
    pub max_position_size: f64,
    pub min_usdc_reserve: f64,
    pub min_sol_reserve: f64,

    // ─────────────────────────────────────────────────────────────────────────
    // Dynamic Grid Features
    // ─────────────────────────────────────────────────────────────────────────
    #[serde(default)]
    pub enable_dynamic_grid: bool,
    #[serde(default = "default_reposition_threshold")]
    pub reposition_threshold: f64,
    #[serde(default = "default_volatility_window")]
    pub volatility_window: u32,

    // ─────────────────────────────────────────────────────────────────────────
    // Auto-Rebalancing
    // ─────────────────────────────────────────────────────────────────────────
    #[serde(default = "default_true")]
    pub enable_auto_rebalance: bool,
    #[serde(default = "default_true")]
    pub enable_smart_rebalance: bool,
    #[serde(default = "default_rebalance_threshold")]
    pub rebalance_threshold_pct: f64,
    #[serde(default = "default_cooldown")]
    pub rebalance_cooldown_secs: u64,

    // ─────────────────────────────────────────────────────────────────────────
    // Order Management
    // ─────────────────────────────────────────────────────────────────────────
    #[serde(default = "default_max_orders")]
    pub max_orders_per_side: u32,
    #[serde(default = "default_refresh_interval")]
    pub order_refresh_interval_secs: u64,
    #[serde(default)]
    pub enable_market_orders: bool,

    /// Master switch: enable fee optimization via SmartFeeFilter.
    /// Fine-grained params live in `fee_filter` sub-section.
    #[serde(default = "default_true")]
    pub enable_fee_optimization: bool,

    // ─────────────────────────────────────────────────────────────────────────
    // Risk Limits
    // ─────────────────────────────────────────────────────────────────────────
    #[serde(default = "default_profit_threshold")]
    pub min_profit_threshold_pct: f64,
    #[serde(default = "default_slippage")]
    pub max_slippage_pct: f64,

    // ─────────────────────────────────────────────────────────────────────────
    // Price Bounds
    // ─────────────────────────────────────────────────────────────────────────
    #[serde(default)]
    pub enable_price_bounds: bool,
    #[serde(default = "default_lower_bound")]
    pub lower_price_bound: f64,
    #[serde(default = "default_upper_bound")]
    pub upper_price_bound: f64,

    // ─────────────────────────────────────────────────────────────────────────
    // Market Regime Gate
    // ─────────────────────────────────────────────────────────────────────────
    #[serde(default = "default_true")]
    pub enable_regime_gate: bool,
    #[serde(default = "default_min_volatility")]
    pub min_volatility_to_trade: f64,
    #[serde(default = "default_true")]
    pub pause_in_very_low_vol: bool,

    // ─────────────────────────────────────────────────────────────────────────
    // V6.0 (PR fix/regime-gate-configurable-vol-floor): Configurable Prod Vol Floor
    // ─────────────────────────────────────────────────────────────────────────

    /// Minimum vol% the production enforcer will raise min_volatility_to_trade to.
    /// Replaces hardcoded 0.3 floor in apply_environment("production").
    /// Default: 0.05 — matches live mainnet tuning Mar 14, 2026.
    #[serde(default = "default_vol_floor_resume_pct")]
    pub vol_floor_resume_pct: f64,

    // ─────────────────────────────────────────────────────────────────────────
    // Order Lifecycle Management
    // ─────────────────────────────────────────────────────────────────────────
    #[serde(default = "default_true")]
    pub enable_order_lifecycle: bool,
    #[serde(default = "default_order_max_age")]
    pub order_max_age_minutes: u64,
    #[serde(default = "default_lifecycle_check")]
    pub order_refresh_interval_minutes: u64,
    #[serde(default = "default_min_orders")]
    pub min_orders_to_maintain: usize,

    // ─────────────────────────────────────────────────────────────────────────
    // Advanced Features
    // ─────────────────────────────────────────────────────────────────────────
    #[serde(default)]
    pub enable_adaptive_spacing: bool,

    /// Enable smart position sizing (consensus-signal-driven).
    /// When true, order size scales with signal strength via signal_size_multiplier.
    #[serde(default)]
    pub enable_smart_position_sizing: bool,

    // ─────────────────────────────────────────────────────────────────────────
    // V5.5 (PR #94 Commit 5a): Consensus Signal Position Sizing
    // ─────────────────────────────────────────────────────────────────────────

    /// Scales how strongly the consensus signal strength multiplies order size.
    ///
    /// Formula (in place_grid_orders):
    ///   multiplier   = 1.0 + signal.strength() * (signal_size_multiplier - 1.0)
    ///   effective_sz = (base_size * multiplier).clamp(min_order_size, max_position_size)
    ///
    /// Only active when enable_smart_position_sizing = true.
    /// Validation: must be in [0.5, 3.0] when smart sizing is enabled.
    /// Default: 1.0 — safe, zero behaviour change even if flag is set.
    #[serde(default = "default_signal_size_multiplier")]
    pub signal_size_multiplier: f64,

    // ─────────────────────────────────────────────────────────────────────────
    // V5.3 (PR #94 Commit 1): Adaptive Optimizer Tuning
    // ─────────────────────────────────────────────────────────────────────────

    /// Cycles between AdaptiveOptimizer update ticks.
    /// Replaces hardcoded `const OPTIMIZATION_INTERVAL_CYCLES = 50`.
    /// Default: 50.
    #[serde(default = "default_optimizer_interval_cycles")]
    pub optimizer_interval_cycles: u64,

    // ─────────────────────────────────────────────────────────────────────────
    // V5.4 (PR #94 Commit 3): Fee Filter Sub-Section
    // ─────────────────────────────────────────────────────────────────────────

    /// SmartFeeFilter parameters — controls per-level fee profitability check.
    /// Mapped to `[trading.fee_filter]` in TOML.
    /// Defaults to `FeeFilterConfig::default()` when the section is absent.
    #[serde(default)]
    pub fee_filter: FeeFilterConfig,

    // ─────────────────────────────────────────────────────────────────────────
    // V5.8 (PR #107 Commit 1): Dynamic Grid Spacing Bounds
    // ─────────────────────────────────────────────────────────────────────────

    /// Maximum grid spacing (fraction) when dynamic spacing is active.
    /// Replaces hardcoded `max_spacing: 0.0075` in GridRebalancerConfig init.
    /// Stored as fraction (0.0075 = 0.75%). Active only when enable_dynamic_grid = true.
    /// Default: 0.0075 — all 46 TOMLs parse unchanged.
    #[serde(default = "default_max_grid_spacing_pct")]
    pub max_grid_spacing_pct: f64,

    /// Minimum grid spacing (fraction) when dynamic spacing is active.
    /// Replaces hardcoded `min_spacing: 0.001` in GridRebalancerConfig init.
    /// Stored as fraction (0.001 = 0.10%). Active only when enable_dynamic_grid = true.
    /// Default: 0.001 — all 46 TOMLs parse unchanged.
    #[serde(default = "default_min_grid_spacing_pct")]
    pub min_grid_spacing_pct: f64,
}

impl TradingConfig {
    pub fn validate(&self) -> Result<()> {
        if self.grid_levels < 2 {
            bail!("grid_levels must be at least 2 (current: {})", self.grid_levels);
        }
        if self.grid_levels > 100 {
            warn!("⚠️ Very high grid_levels ({}) - may cause performance issues", self.grid_levels);
        }
        if self.grid_spacing_percent <= 0.0 {
            bail!("grid_spacing_percent must be positive (current: {})", self.grid_spacing_percent);
        }
        if self.grid_spacing_percent > 10.0 {
            warn!("⚠️ Very wide grid spacing ({:.2}%) - trades may be infrequent", self.grid_spacing_percent);
        }
        if self.grid_spacing_percent < 0.05 {
            warn!("⚠️ Very tight grid spacing ({:.2}%) - may not profit after fees", self.grid_spacing_percent);
        }
        if self.min_order_size <= 0.0 {
            bail!("min_order_size must be positive");
        }
        if self.max_position_size <= self.min_order_size {
            bail!("max_position_size must be > min_order_size");
        }
        if self.min_usdc_reserve < 0.0 {
            bail!("min_usdc_reserve cannot be negative");
        }
        if self.min_sol_reserve < 0.0 {
            bail!("min_sol_reserve cannot be negative");
        }
        if self.enable_regime_gate {
            if self.min_volatility_to_trade < 0.0 {
                bail!("min_volatility_to_trade cannot be negative");
            }
            if self.min_volatility_to_trade > 5.0 {
                warn!("⚠️ Very high min_volatility_to_trade ({:.2}%) - bot may rarely trade!",
                      self.min_volatility_to_trade);
            }
        }
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
        if self.enable_price_bounds {
            if self.lower_price_bound >= self.upper_price_bound {
                bail!("lower_price_bound must be < upper_price_bound");
            }
            if self.lower_price_bound <= 0.0 {
                bail!("lower_price_bound must be positive");
            }
        }
        if self.optimizer_interval_cycles == 0 {
            bail!("trading.optimizer_interval_cycles must be > 0 (got 0)");
        }
        // V5.5: validate signal_size_multiplier range when smart sizing is on
        if self.enable_smart_position_sizing
            && !(0.5_f64..=3.0_f64).contains(&self.signal_size_multiplier)
        {
            bail!(
                "trading.signal_size_multiplier must be in [0.5, 3.0] when \
                 enable_smart_position_sizing=true (got {:.3})",
                self.signal_size_multiplier
            );
        }
        // V5.4: delegate fee_filter validation
        self.fee_filter.validate().context("trading.fee_filter validation failed")?;
        // V5.8: dynamic spacing bounds — only validated when feature is active
        if self.enable_dynamic_grid {
            if self.min_grid_spacing_pct <= 0.0 {
                bail!("trading.min_grid_spacing_pct must be positive");
            }
            if self.min_grid_spacing_pct >= self.max_grid_spacing_pct {
                bail!(
                    "trading.min_grid_spacing_pct ({:.5}) must be < max_grid_spacing_pct ({:.5})",
                    self.min_grid_spacing_pct, self.max_grid_spacing_pct
                );
            }
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
                if self.min_volatility_to_trade < self.vol_floor_resume_pct {
                    warn!("⚠️ Raising min_volatility to {:.2}% for production safety",
                          self.vol_floor_resume_pct);
                    self.min_volatility_to_trade = self.vol_floor_resume_pct;
                }
                if !self.enable_order_lifecycle {
                    warn!("⚠️ Force-enabling order lifecycle for production!");
                    self.enable_order_lifecycle = true;
                }
            }
            _ => { warn!("⚠️ Unknown environment '{}' - using config as-is", environment); }
        }
    }
}

impl Default for TradingConfig {
    fn default() -> Self {
        Self {
            grid_levels:            35,
            grid_spacing_percent:   0.15,
            min_order_size:         0.1,
            max_position_size:      100.0,
            min_usdc_reserve:       300.0,
            min_sol_reserve:        2.0,
            enable_dynamic_grid:             false,
            reposition_threshold:            default_reposition_threshold(),
            volatility_window:               default_volatility_window(),
            enable_auto_rebalance:           true,
            enable_smart_rebalance:          true,
            rebalance_threshold_pct:         default_rebalance_threshold(),
            rebalance_cooldown_secs:         default_cooldown(),
            max_orders_per_side:             default_max_orders(),
            order_refresh_interval_secs:     default_refresh_interval(),
            enable_market_orders:            false,
            enable_fee_optimization:         true,
            min_profit_threshold_pct:        default_profit_threshold(),
            max_slippage_pct:                default_slippage(),
            enable_price_bounds:             false,
            lower_price_bound:               default_lower_bound(),
            upper_price_bound:               default_upper_bound(),
            enable_regime_gate:              false,
            min_volatility_to_trade:         0.0,
            pause_in_very_low_vol:           false,
            enable_order_lifecycle:          true,
            order_max_age_minutes:           default_order_max_age(),
            order_refresh_interval_minutes:  default_lifecycle_check(),
            min_orders_to_maintain:          default_min_orders(),
            enable_adaptive_spacing:         false,
            enable_smart_position_sizing:    false,
            // V5.5 PR #94 Commit 5a
            signal_size_multiplier:          default_signal_size_multiplier(),
            optimizer_interval_cycles:       default_optimizer_interval_cycles(),
            // V5.4 PR #94 Commit 3
            fee_filter:                      FeeFilterConfig::default(),
            // V5.8 PR #107 Commit 1
            max_grid_spacing_pct:            default_max_grid_spacing_pct(),
            min_grid_spacing_pct:            default_min_grid_spacing_pct(),
            // V6.0 PR fix/regime-gate-configurable-vol-floor
            vol_floor_resume_pct:            default_vol_floor_resume_pct(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// REGIME GATE CONFIG (Analytics Module Bridge)
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
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
        info!("🔧 Converting TradingConfig → RegimeGateConfig:");
        info!("   Min volatility: {:.2}% → {} BPS", trading.min_volatility_to_trade, volatility_bps);
        info!("   Regime gate: {}", if trading.enable_regime_gate { "ENABLED" } else { "DISABLED" });
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
#[serde(deny_unknown_fields)]
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
    pub momentum_macd: MomentumMACDStrategyConfig,
    #[serde(default)]
    pub enable_multi_timeframe: bool,
    #[serde(default)]
    pub require_timeframe_alignment: bool,

    // ─────────────────────────────────────────────────────────────────────────
    // PR #99 Commit 1: WMA Confidence Gate (config-driven)
    // ─────────────────────────────────────────────────────────────────────────

    /// Minimum confidence a strategy's signal must carry to participate
    /// in the WMA weighted vote.
    ///
    /// Tuning guide:
    ///   0.50 -> permissive  (single-strategy mode, paper/dev)
    ///   0.65 -> balanced    (multi-strategy production default)
    ///   0.75 -> conservative (high-conviction signals only)
    ///
    /// Default: 0.50 — matches `strategies.grid.min_confidence`.
    /// Validation: must be in [0.0, 1.0].
    #[serde(default = "default_wma_confidence_threshold")]
    pub wma_confidence_threshold: f64,
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
        let mut total_weight = 0.0;
        if self.grid.enabled         { total_weight += self.grid.weight; }
        if self.momentum.enabled     { total_weight += self.momentum.weight; }
        if self.mean_reversion.enabled { total_weight += self.mean_reversion.weight; }
        if self.rsi.enabled          { total_weight += self.rsi.weight; }
        if self.momentum_macd.enabled { total_weight += self.momentum_macd.weight; }
        if total_weight == 0.0 {
            bail!("No strategies are enabled");
        }
        // PR #99 Commit 1: validate WMA confidence gate range
        if !(0.0_f64..=1.0_f64).contains(&self.wma_confidence_threshold) {
            bail!(
                "strategies.wma_confidence_threshold must be in [0.0, 1.0] (got {:.3})",
                self.wma_confidence_threshold
            );
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
                seed_orders_bypass: default_grid_seed_bypass(),
            },
            momentum: MomentumStrategyConfig::default(),
            mean_reversion: MeanReversionStrategyConfig::default(),
            rsi: RsiStrategyConfig::default(),
            momentum_macd: MomentumMACDStrategyConfig::default(),
            enable_multi_timeframe: false,
            require_timeframe_alignment: false,
            // PR #99 Commit 1: WMA confidence gate — permissive default
            wma_confidence_threshold: default_wma_confidence_threshold(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Strategy sub-configs
// ─────────────────────────────────────────────────────────────────────────

/// Config for the grid strategy, including the seed-orders bypass flag.
///
/// `seed_orders_bypass` mirrors `GridRebalancerConfig::seed_orders_bypass`.
/// When `true` (default), `GridRebalancer::rebalance()` skips the initial
/// seed-order placement and jumps straight to live grid management.
/// Set to `false` only if you want the full seed sequence on (re)start.
///
/// PR #100: added here so TOML keys are accepted instead of rejected by
/// `deny_unknown_fields` and causing a startup crash.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GridStrategyConfig {
    pub enabled: bool,
    pub weight: f64,
    #[serde(default = "default_confidence")]
    pub min_confidence: f64,
    /// Skip initial seed-order placement on (re)start.
    /// Default: true — matches GridRebalancerConfig default.
    #[serde(default = "default_grid_seed_bypass")]
    pub seed_orders_bypass: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MomentumStrategyConfig {
    pub enabled: bool,
    pub weight: f64,
    pub min_confidence: f64,
    pub lookback_period: usize,
    pub threshold: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MeanReversionStrategyConfig {
    pub enabled: bool,
    pub weight: f64,
    pub min_confidence: f64,
    pub sma_period: usize,
    pub std_dev_multiplier: f64,
    #[serde(default = "default_strong_threshold")]
    pub strong_buy_threshold: f64,
    #[serde(default = "default_normal_threshold")]
    pub buy_threshold: f64,
    #[serde(default = "default_strong_threshold")]
    pub strong_sell_threshold: f64,
    #[serde(default = "default_normal_threshold")]
    pub sell_threshold: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RsiStrategyConfig {
    pub enabled: bool,
    pub weight: f64,
    pub min_confidence: f64,
    pub period: usize,
    pub oversold_threshold: f64,
    pub overbought_threshold: f64,
    #[serde(default = "default_extreme_oversold")]
    pub extreme_oversold: f64,
    #[serde(default = "default_extreme_overbought")]
    pub extreme_overbought: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MomentumMACDStrategyConfig {
    pub enabled: bool,
    pub weight: f64,
    #[serde(default = "default_macd_min_confidence")]
    pub min_confidence: f64,
    #[serde(default = "default_macd_histogram_threshold")]
    pub strong_histogram_threshold: f64,
    #[serde(default = "default_macd_warmup_periods")]
    pub min_warmup_periods: usize,
}

// ── Conversion helpers ────────────────────────────────────────────────────

impl RsiStrategyConfig {
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
    pub fn to_momentum_macd_params(&self) -> MomentumMACDParams {
        MomentumMACDParams {
            min_confidence: self.min_confidence,
            strong_histogram_threshold: self.strong_histogram_threshold,
            min_warmup_periods: self.min_warmup_periods,
        }
    }
}

// ── Param mirror structs ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RsiParams {
    pub rsi_period: usize,
    pub oversold_threshold: f64,
    pub overbought_threshold: f64,
    pub extreme_oversold: f64,
    pub extreme_overbought: f64,
}

#[derive(Debug, Clone)]
pub struct MeanReversionParams {
    pub mean_period: usize,
    pub strong_buy_threshold: f64,
    pub buy_threshold: f64,
    pub strong_sell_threshold: f64,
    pub sell_threshold: f64,
    pub min_confidence: f64,
}

#[derive(Debug, Clone)]
pub struct MomentumMACDParams {
    pub min_confidence: f64,
    pub strong_histogram_threshold: f64,
    pub min_warmup_periods: usize,
}

// ── Default impls ─────────────────────────────────────────────────────────

impl Default for MomentumStrategyConfig {
    fn default() -> Self {
        Self { enabled: false, weight: 0.8, min_confidence: 0.6, lookback_period: 20, threshold: 0.02 }
    }
}

impl Default for MeanReversionStrategyConfig {
    fn default() -> Self {
        Self {
            enabled: false, weight: 0.7, min_confidence: 0.6, sma_period: 20, std_dev_multiplier: 2.0,
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
            enabled: false, weight: 0.9, min_confidence: 0.7, period: 14,
            oversold_threshold: 30.0, overbought_threshold: 70.0,
            extreme_oversold: default_extreme_oversold(),
            extreme_overbought: default_extreme_overbought(),
        }
    }
}

impl Default for MomentumMACDStrategyConfig {
    fn default() -> Self {
        Self {
            enabled: false, weight: 0.8,
            min_confidence: default_macd_min_confidence(),
            strong_histogram_threshold: default_macd_histogram_threshold(),
            min_warmup_periods: default_macd_warmup_periods(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// RISK CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RiskConfig {
    pub max_position_size_pct: f64,
    pub max_drawdown_pct: f64,
    pub stop_loss_pct: f64,
    pub take_profit_pct: f64,
    #[serde(default = "default_true")]
    pub enable_circuit_breaker: bool,
    pub circuit_breaker_threshold_pct: f64,
    pub circuit_breaker_cooldown_secs: u64,
    #[serde(default = "default_max_consecutive_losses")]
    pub max_consecutive_losses: u32,
    #[serde(default = "default_trailing_stop")]
    pub enable_trailing_stop: bool,
}

impl RiskConfig {
    pub fn validate(&self) -> Result<()> {
        if self.max_position_size_pct <= 0.0 || self.max_position_size_pct > 100.0 {
            bail!("max_position_size_pct must be between 0-100%");
        }
        if self.max_drawdown_pct <= 0.0 || self.max_drawdown_pct > 100.0 {
            bail!("max_drawdown_pct must be between 0-100%");
        }
        if self.max_consecutive_losses == 0 {
            bail!("max_consecutive_losses must be > 0");
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
#[serde(deny_unknown_fields)]
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

// ═══════════════════════════════════════════════════════════════════════════
// PERFORMANCE CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
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
        Self { cycle_interval_ms: 100, startup_delay_ms: 1000, request_timeout_ms: 5000 }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// LOGGING CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LoggingConfig {
    pub level: String,
    pub file_path: String,
    #[serde(default = "default_true")]
    pub enable_file_logging: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self { level: "info".to_string(), file_path: "logs/gridbot.log".to_string(), enable_file_logging: true }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// METRICS CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MetricsConfig {
    #[serde(default = "default_true")]
    pub enable_metrics: bool,
    #[serde(default = "default_stats_interval")]
    pub stats_interval: u64,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self { enable_metrics: true, stats_interval: 50 }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// PAPER TRADING CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
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
        if let Some(secs) = self.test_duration_seconds { return secs; }
        if let Some(mins) = self.test_duration_minutes { return mins * 60; }
        if let Some(hours) = self.test_duration_hours  { return hours * 3600; }
        3600
    }
    pub fn calculate_cycles(&self, cycle_interval_ms: u64) -> usize {
        if let Some(cycles) = self.test_cycles { return cycles; }
        let duration_secs  = self.duration_seconds();
        let cycles_per_sec = 1000 / cycle_interval_ms as usize;
        duration_secs * cycles_per_sec
    }
    pub fn validate(&self) -> Result<()> {
        if self.initial_usdc <= 0.0 { bail!("initial_usdc must be positive"); }
        if self.initial_sol  <= 0.0 { bail!("initial_sol must be positive"); }
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
            initial_usdc: 5000.0, initial_sol: 10.0,
            test_duration_hours: Some(1),
            test_duration_minutes: None, test_duration_seconds: None, test_cycles: None,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// DATABASE & ALERTS
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct DatabaseConfig {
    #[serde(default)] pub enabled: bool,
    #[serde(default)] pub url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(deny_unknown_fields)]
pub struct AlertsConfig {
    #[serde(default)] pub enabled: bool,
    #[serde(default)] pub telegram_bot_token: Option<String>,
    #[serde(default)] pub telegram_chat_id:   Option<String>,
}

// ═══════════════════════════════════════════════════════════════════════════
// DEFAULT VALUE HELPERS
// ═══════════════════════════════════════════════════════════════════════════

fn default_true()                     -> bool   { true }
fn default_confidence()               -> f64    { 0.5 }
fn default_reposition_threshold()     -> f64    { 0.5 }
fn default_volatility_window()        -> u32    { 100 }
fn default_rebalance_threshold()      -> f64    { 5.0 }
fn default_cooldown()                 -> u64    { 60 }
fn default_max_orders()               -> u32    { 10 }
fn default_refresh_interval()         -> u64    { 300 }
fn default_profit_threshold()         -> f64    { 0.1 }
fn default_slippage()                 -> f64    { 1.0 }
fn default_lower_bound()              -> f64    { 100.0 }
fn default_upper_bound()              -> f64    { 200.0 }
fn default_min_volatility()           -> f64    { 0.5 }
fn default_order_max_age()            -> u64    { 10 }
fn default_lifecycle_check()          -> u64    { 5 }
fn default_min_orders()               -> usize  { 8 }
fn default_update_interval()          -> u64    { 500 }
fn default_cycle_interval()           -> u64    { 100 }
fn default_startup_delay()            -> u64    { 1000 }
fn default_stats_interval()           -> u64    { 50 }
fn default_initial_usdc()             -> f64    { 5000.0 }
fn default_initial_sol()              -> f64    { 10.0 }
fn default_request_timeout()          -> u64    { 5000 }
fn default_paper_mode()               -> String { "paper".to_string() }
fn default_max_trade_sol()            -> f64    { 0.5 }
fn default_priority_fee_microlamports()-> u64   { 50_000 }
fn default_slippage_bps()             -> u16    { 100 }
fn default_confirm_timeout_secs()     -> u64    { 60 }
fn default_max_tx_retries()           -> u8     { 3 }
fn default_max_requote_attempts()      -> u8     { 3 }
fn default_extreme_oversold()         -> f64    { 20.0 }
fn default_extreme_overbought()       -> f64    { 80.0 }
fn default_strong_threshold()         -> f64    { 5.0 }
fn default_normal_threshold()         -> f64    { 2.5 }
fn default_macd_min_confidence()      -> f64    { 0.65 }
fn default_macd_histogram_threshold() -> f64    { 0.5 }
fn default_macd_warmup_periods()      -> usize  { 26 }
fn default_wallet_path()              -> String { "~/.config/solana/id.json".to_string() }
fn default_max_trade_size_usdc()      -> f64    { 250.0 }
fn default_max_consecutive_losses()   -> u32    { 5 }
fn default_trailing_stop()            -> bool   { false }
/// Replaces `const OPTIMIZATION_INTERVAL_CYCLES = 50` in grid_bot.rs.
fn default_optimizer_interval_cycles() -> u64   { 50 }
/// SmartFeeFilter: minimum fee that is considered real (not stale/bad route).
fn default_min_fee_threshold_bps()    -> u32    { 8 }
/// SmartFeeFilter: maximum fee before a level is considered unprofitable.
fn default_max_fee_threshold_bps()    -> u32    { 50 }
/// SmartFeeFilter: rolling window for moving-average fee baseline.
fn default_fee_filter_window_secs()   -> u64    { 30 }
/// Consensus signal size multiplier — 1.0 = flat (safe default, zero behaviour change).
fn default_signal_size_multiplier()   -> f64    { 1.0 }
/// PR #99 Commit 1: WMA confidence gate default — 0.50 (permissive).
fn default_wma_confidence_threshold() -> f64    { 0.50 }
/// PR #100: seed_orders_bypass default — true, matches GridRebalancerConfig default.
fn default_grid_seed_bypass()         -> bool   { true }
/// PR #107 Commit 1: Max dynamic grid spacing fraction.
/// Replaces hardcoded 0.0075 in GridRebalancerConfig init (grid_bot.rs).
fn default_max_grid_spacing_pct()     -> f64    { 0.0075 }
/// PR #107 Commit 1: Min dynamic grid spacing fraction.
/// Replaces hardcoded 0.001 in GridRebalancerConfig init (grid_bot.rs).
fn default_min_grid_spacing_pct()     -> f64    { 0.001  }
/// PR fix/regime-gate: configurable production vol floor (was hardcoded 0.3).
/// Default: 0.05 — matches live mainnet tuning (Mar 14, 2026).
fn default_vol_floor_resume_pct()     -> f64    { 0.05 }

// ═══════════════════════════════════════════════════════════════════════════
// MAIN CONFIG IMPLEMENTATION
// ═══════════════════════════════════════════════════════════════════════════

impl Config {
    pub fn load() -> Result<Self> {
        Self::from_file("config/master.toml")
    }

    /// Load, resolve secrets, and validate a config file.
    ///
    /// Pipeline (V5.9):
    ///   1. Parse TOML → Config
    ///   2. apply_environment_defaults() — env-specific field overrides
    ///   3. secrets::resolve_secrets()  — GRIDZBOTZ_* env vars override TOML
    ///   4. validate()                  — bail on invalid state
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        info!("🔧 Loading configuration from: {}", path.display());
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;
        let mut config: Config = toml::from_str(&content)
            .context("Failed to parse TOML configuration")?;
        info!("🌍 Applying environment overrides: {}", config.bot.environment);
        config.apply_environment_defaults();
        // V5.9: inject secrets from GRIDZBOTZ_* env vars before validation
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
        println!("  🤖 GRIDZBOTZ V5.9 - CONFIGURATION");
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
            );        } else {
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
                self.strategies.rsi.period, self.strategies.rsi.oversold_threshold,
                self.strategies.rsi.overbought_threshold,
                self.strategies.rsi.extreme_oversold, self.strategies.rsi.extreme_overbought);
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
        println!("   Stop Loss:        {:.1}% ({})",
            self.risk.stop_loss_pct,
            if self.risk.enable_trailing_stop { "trailing" } else { "fixed" });
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
// BUILDER PATTERN
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
                    version: "5.9.0".to_string(),
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
                security: SecurityConfig::default(),
                trading: TradingConfig::default(),
                strategies: StrategiesConfig::default(),
                risk: RiskConfig {
                    max_position_size_pct: 30.0,
                    max_drawdown_pct: 10.0,
                    stop_loss_pct: 5.0,
                    take_profit_pct: 10.0,
                    enable_circuit_breaker: true,
                    circuit_breaker_threshold_pct: 8.0,
                    circuit_breaker_cooldown_secs: 300,
                    max_consecutive_losses: default_max_consecutive_losses(),
                    enable_trailing_stop: false,
                },
                fees: FeesConfig::default(),
                priority_fees: PriorityFeeConfig::default(),
                execution: ExecutionConfig::default(),
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
    /// Set the AdaptiveOptimizer update cadence in main-loop cycles. Default: 50.
    pub fn optimizer_interval_cycles(mut self, cycles: u64) -> Self {
        self.config.trading.optimizer_interval_cycles = cycles; self
    }
    /// Override SmartFeeFilter thresholds. Must satisfy min < max.
    pub fn fee_filter(mut self, min_bps: u32, max_bps: u32, window_secs: u64) -> Self {
        self.config.trading.fee_filter.min_fee_threshold_bps   = min_bps;
        self.config.trading.fee_filter.max_fee_threshold_bps   = max_bps;
        self.config.trading.fee_filter.fee_filter_window_secs  = window_secs;
        self
    }
    /// Enable consensus-signal-driven position sizing with given multiplier.
    pub fn signal_size_multiplier(mut self, multiplier: f64) -> Self {
        self.config.trading.enable_smart_position_sizing = true;
        self.config.trading.signal_size_multiplier = multiplier;
        self
    }
    /// PR #99 Commit 1: Set WMA confidence gate for multi-strategy voting.
    pub fn wma_confidence_threshold(mut self, threshold: f64) -> Self {
        self.config.strategies.wma_confidence_threshold = threshold;
        self
    }
    /// PR #100: Control seed-orders bypass for grid strategy rebalancer.
    pub fn seed_orders_bypass(mut self, bypass: bool) -> Self {
        self.config.strategies.grid.seed_orders_bypass = bypass;
        self
    }
    /// PR #107 Commit 1: Set dynamic grid spacing bounds (fractions, e.g. 0.001=0.1%, 0.0075=0.75%).
    /// Only active when enable_dynamic_grid = true.
    pub fn dynamic_grid_spacing(mut self, min: f64, max: f64) -> Self {
        self.config.trading.min_grid_spacing_pct = min;
        self.config.trading.max_grid_spacing_pct = max;
        self
    }

        /// PR fix/regime-gate: Set configurable production vol floor.
    /// Replaces hardcoded 0.3 in apply_environment("production").
    pub fn vol_floor_resume_pct(mut self, floor: f64) -> Self {
        self.config.trading.vol_floor_resume_pct = floor; self
    }

    /// Set maximum number of re-quote attempts for a single trade execution.
    /// Mirrors ExecutionConfig::max_requote_attempts (default: 3).
    pub fn max_requote_attempts(mut self, n: u8) -> Self {
        self.config.execution.max_requote_attempts = n;
        self
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
// TESTS — V5.9 resolve_secrets wiring + carried-forward suites
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

    // ── PR #99 Commit 1: wma_confidence_threshold tests ────────────────────

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

    // ── PR #100: seed_orders_bypass tests ───────────────────────────────────

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
            enabled: true,
            weight: 1.0,
            min_confidence: 0.5,
            seed_orders_bypass: false,
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
        assert!(!config.strategies.grid.seed_orders_bypass,
            "builder must wire seed_orders_bypass=false");
    }

    // ── PR #107 Commit 1: max/min_grid_spacing_pct tests ───────────────────

    #[test]
    fn test_grid_spacing_bounds_defaults() {
        let cfg = TradingConfig::default();
        assert!((cfg.max_grid_spacing_pct - 0.0075).abs() < 1e-9,
            "max default must be 0.0075, got {}", cfg.max_grid_spacing_pct);
        assert!((cfg.min_grid_spacing_pct - 0.001).abs() < 1e-9,
            "min default must be 0.001, got {}", cfg.min_grid_spacing_pct);
    }

    #[test]
    fn test_grid_spacing_bounds_absent_toml_defaults() {
        let toml_str = r#"
grid_levels = 10
grid_spacing_percent = 0.15
min_order_size = 0.1
max_position_size = 10.0
min_usdc_reserve = 100.0
min_sol_reserve = 1.0
"#;
        let cfg: TradingConfig = toml::from_str(toml_str).expect("deserialise");
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
        assert!(
            err.to_string().contains("min_grid_spacing_pct"),
            "error must name the field; got: {}", err
        );
    }

    #[test]
    fn test_grid_spacing_bounds_skipped_when_dynamic_disabled() {
        let mut cfg = TradingConfig::default();
        cfg.enable_dynamic_grid = false;
        cfg.min_grid_spacing_pct = 0.01;
        cfg.max_grid_spacing_pct = 0.005;
        assert!(cfg.validate().is_ok(),
            "bounds must not be validated when enable_dynamic_grid=false");
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
    fn test_vol_floor_resume_pct_absent_toml_defaults() {
        let toml_str = r#"
grid_levels = 10
grid_spacing_percent = 0.15
min_order_size = 0.1
max_position_size = 10.0
min_usdc_reserve = 100.0
min_sol_reserve = 1.0
"#;
        let cfg: TradingConfig = toml::from_str(toml_str).expect("deserialise");
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
        assert!((cfg.min_volatility_to_trade - 0.05).abs() < 1e-9,
            "production must raise to vol_floor_resume_pct, got {}",
            cfg.min_volatility_to_trade);
    }

    #[test]
    fn test_production_env_does_not_stomp_above_floor() {
        let mut cfg = TradingConfig::default();
        cfg.vol_floor_resume_pct = 0.05;
        cfg.min_volatility_to_trade = 0.20;
        cfg.apply_environment("production");
        assert!((cfg.min_volatility_to_trade - 0.20).abs() < 1e-9,
            "production must NOT override vol already above floor");
    }

        // ── V6.1 max_requote_attempts tests ─────────────────────────────────────

    #[test]
    fn test_max_requote_attempts_default() {
        let cfg = ExecutionConfig::default();
        assert_eq!(cfg.max_requote_attempts, 3);
    }

    #[test]
    fn test_max_requote_attempts_absent_toml_defaults() {
        let toml_str = r#"
max_trade_sol = 0.5
max_trade_size_usdc = 250.0
priority_fee_microlamports = 50000
max_slippage_bps = 100
confirmation_timeout_secs = 60
max_retries = 3
"#;
        let cfg: ExecutionConfig = toml::from_str(toml_str).expect("deserialise minimal ExecutionConfig");
        assert_eq!(cfg.max_requote_attempts, 3);
    }

    #[test]
    fn test_max_requote_attempts_zero_rejected() {
        let mut cfg = ExecutionConfig::default();
        cfg.max_requote_attempts = 0;
        let err = cfg.validate().unwrap_err();
        assert!(
            err.to_string().contains("max_requote_attempts"),
            "error must mention the field; got: {}", err
        );
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


}
