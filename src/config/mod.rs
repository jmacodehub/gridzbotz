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
pub mod secrets;
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

      /// Security and wallet configuration
    #[serde(default)]
    pub security: SecurityConfig,

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
// 🔐 SECURITY CONFIGURATION (New in PR #41 - Step 1/3)
// ═══════════════════════════════════════════════════════════════════════════

/// Security and wallet configuration.
///
/// Controls keypair loading, encryption, and hot wallet access.
/// Future: hardware wallet support, multi-sig, whitelisting.
///
/// # Example (config/master.toml)
/// ```toml
/// [security]
/// wallet_path = "~/.config/solana/id.json"
/// require_password = false
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecurityConfig {
    /// Path to Solana keypair file (JSON format)
    #[serde(default = "default_wallet_path")]
    pub wallet_path: String,

    /// Require password to decrypt keypair (future)
    #[serde(default)]
    pub require_password: bool,

    /// Optional list of authorized program IDs (future whitelisting)
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
        /// 🆕 V5.2 PR #45: Live mode security validation
    pub fn validate_for_live_mode(&self) -> Result<()> {
        use std::path::PathBuf;

        // Step 1: Expand ~ if present
        let expanded_path = if self.wallet_path.starts_with('~') {
            let home = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .context("Cannot determine home directory for ~ expansion")?;
            PathBuf::from(self.wallet_path.replacen('~', &home, 1))
        } else {
            PathBuf::from(&self.wallet_path)
        };

        // Step 2: Check file exists
        if !expanded_path.exists() {
            bail!(
                "Wallet file not found: {}\n\
                 Ensure security.wallet_path in your config points to a valid keypair file.",
                expanded_path.display()
            );
        }

        // Step 3: Check file is readable
        if let Err(e) = fs::File::open(&expanded_path) {
            bail!(
                "Wallet file exists but cannot be read: {}\n\
                 Error: {}\n\
                 Check file permissions and ownership.",
                expanded_path.display(), e
            );
        }

        // Step 4: Check for insecure permissions on Unix
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
// 🆕 V5.0: EXECUTION CONFIGURATION (Stage 1)
// All live trading execution knobs — now 100% TOML-driven
// ═══════════════════════════════════════════════════════════════════════════

/// Live execution configuration for Jupiter swaps on Solana.
///
/// These settings are only active when `bot.execution_mode = "live"`.
/// In paper mode they are parsed but ignored — safe to include in any config.
///
/// # Example (config/master.toml)
/// ```toml
/// [execution]
/// max_trade_sol = 0.5
/// priority_fee_microlamports = 50_000
/// max_slippage_bps = 100
/// jito_tip_lamports = 10_000
/// confirmation_timeout_secs = 60
/// max_retries = 3
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExecutionConfig {
    /// Maximum SOL amount per single Jupiter swap.
    /// Hard safety cap — no single trade exceeds this regardless of grid sizing.
    #[serde(default = "default_max_trade_sol")]
    pub max_trade_sol: f64,

      /// Maximum USDC amount per single Jupiter swap.
    /// Complementary cap — whichever limit hits first wins.
    #[serde(default = "default_max_trade_size_usdc")]
    pub max_trade_size_usdc: f64,

    /// Priority fee in microlamports added to each transaction.
    /// Higher = faster inclusion, higher cost.
    /// 50_000 µlamports ≈ 0.00005 SOL per tx at current base fee.
    #[serde(default = "default_priority_fee_microlamports")]
    pub priority_fee_microlamports: u64,

    /// Maximum acceptable slippage in basis points (BPS).
    /// 100 BPS = 1.0%. Jupiter rejects quotes exceeding this.
    /// Recommended: 50–150 BPS for liquid SOL/USDC pair.
    #[serde(default = "default_slippage_bps")]
    pub max_slippage_bps: u16,

    /// Optional Jito bundle tip in lamports (MEV protection).
    /// Set to Some(10_000) to enable. None = skip Jito entirely.
    /// Adds ~0.00001 SOL per trade but protects against sandwich attacks.
    #[serde(default)]
    pub jito_tip_lamports: Option<u64>,

    /// Optional RPC fallback URLs (tried in order if primary fails).
    /// Complements network.rpc_url — no need to duplicate primary here.
    #[serde(default)]
    pub rpc_fallback_urls: Option<Vec<String>>,

    /// How long to wait for on-chain confirmation (seconds).
    /// If not confirmed in time, the tx is treated as failed and retried.
    #[serde(default = "default_confirm_timeout_secs")]
    pub confirmation_timeout_secs: u64,

    /// Maximum retry attempts per failed transaction.
    /// Uses exponential backoff between retries.
    #[serde(default = "default_max_tx_retries")]
    pub max_retries: u8,
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
        }
    }
}

impl ExecutionConfig {
    pub fn validate(&self) -> Result<()> {
        if self.max_trade_sol <= 0.0 {
            bail!("execution.max_trade_sol must be positive");
        }
        if self.max_trade_sol > 100.0 {
            warn!(
                "⚠️ execution.max_trade_sol ({:.2}) is very large — \
                 double-check capital allocation",
                self.max_trade_sol
            );
        }
        if self.max_slippage_bps == 0 {
            bail!("execution.max_slippage_bps cannot be 0 — Jupiter requires > 0 BPS");
        }
        if self.max_slippage_bps > 500 {
            warn!(
                "⚠️ execution.max_slippage_bps ({}) > 5% — very high slippage tolerance!",
                self.max_slippage_bps
            );
        }
        if self.confirmation_timeout_secs == 0 {
            bail!("execution.confirmation_timeout_secs must be > 0");
        }
        if self.max_retries == 0 {
            warn!("⚠️ execution.max_retries = 0 — failed txs will NOT be retried");
        }
        Ok(())
    }

    /// Convert BPS slippage to percentage (e.g., 100 BPS → 1.0%)
    pub fn slippage_pct(&self) -> f64 {
        self.max_slippage_bps as f64 / 100.0
    }

    /// Returns true if Jito MEV protection is enabled
    pub fn jito_enabled(&self) -> bool {
        self.jito_tip_lamports.is_some()
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
/// ```ignore
/// use crate::config::{TradingConfig, RegimeGateConfig};
///
/// let trading_config = TradingConfig { ... };
/// let regime_config = RegimeGateConfig::from(&trading_config);
/// ```
///
/// # Conversion Example
/// ```text
/// TradingConfig:           RegimeGateConfig:
/// min_volatility = 0.5%  -> min_volatility_bps = 50.0
/// enable_gate = true     -> enable_gate = true
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MomentumStrategyConfig {
    pub enabled: bool,
    pub weight: f64,
    pub min_confidence: f64,
    pub lookback_period: usize,
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

// ── V5.1: Param mirror structs ────────────────────────────────────────────
//
// Plain-data structs that mirror the Config structs in
// src/strategies/{rsi,mean_reversion,momentum_macd}.rs.
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
fn default_wallet_path() -> String { "~/.config/solana/id.json".to_string() }
fn default_max_trade_size_usdc() -> f64 { 250.0 }

// ═══════════════════════════════════════════════════════════════════════════
// MAIN CONFIG IMPLEMENTATION - V5.1 PRODUCTION GRADE! 🚀
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
        // Bot validation (includes execution_mode check)
        self.bot.validate()
            .context("Bot config validation failed")?;

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

        // Execution validation — only required when mode = "live"
        if self.bot.is_live() {
            info!("🔴 LIVE MODE DETECTED — validating execution config");
            self.execution.validate()
                .context("Execution config validation failed")?;

                        // 🆕 V5.2: Validate wallet file exists and is readable
            self.security.validate_for_live_mode()
                .context("Security config validation failed for live mode")?;

            // Extra safety: live mode requires production environment
            if self.network.cluster != "mainnet-beta" {
                warn!(
                    "⚠️ execution_mode=live but cluster={}. \
                     Are you sure you want to trade live on {}?",
                    self.network.cluster, self.network.cluster
                );
            }
        }

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
        println!("  🤖 GRIDZBOTZ V5.1 - CONFIGURATION");
        println!("{}\n", border);

        println!("📋 BOT: {} v{} [{}]",
            self.bot.name, self.bot.version, self.bot.environment);
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
                    format!("✅ {} lamports",
                        self.execution.jito_tip_lamports.unwrap_or(0))
                } else {
                    "❌ disabled".to_string()
                });
            println!("   Confirm Timeout:  {}s | Retries: {}",
                self.execution.confirmation_timeout_secs, self.execution.max_retries);
        } else {
            println!("   Paper Balance:    ${:.0} USDC + {:.1} SOL",
                self.paper_trading.initial_usdc, self.paper_trading.initial_sol);
        }

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
                    name: "GridzBot-Builder".to_string(),
                    version: "5.1.0".to_string(),
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
        self.config.bot.environment = env.to_string();
        self
    }

    /// Set execution mode: "paper" or "live"
    pub fn execution_mode(mut self, mode: &str) -> Self {
        self.config.bot.execution_mode = mode.to_string();
        self
    }

    /// Set instance ID for multi-bot runs
    pub fn instance_id(mut self, id: &str) -> Self {
        self.config.bot.instance_id = Some(id.to_string());
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
