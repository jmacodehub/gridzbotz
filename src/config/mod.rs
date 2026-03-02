//! ═══════════════════════════════════════════════════════════════════════════
//! 🎛️  UNIFIED CONFIGURATION SYSTEM V5.2 - GRIDZBOTZ
//!
//! Stage 2: Per-Strategy Tuning Params Wired to TOML
//!
//! V5.2 ADDITIONS (PR #41 — Mar 2, 2026):
//! ✅ SecurityConfig: keypair_path + max_transaction_amount_usdc safety caps
//! ✅ Validation: keypair_path required when execution_mode = "live"
//! ✅ Backward compatible: serde(default) means old TOMLs still parse
//! ✅ max_trade_size_usdc added to ExecutionConfig (was missing)
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
//! March 2, 2026 - V5.2 SECURITY CONFIG WIRING 🔒
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

    /// Security configuration (keypair, transaction limits)
    /// Required when bot.execution_mode = "live"
    #[serde(default)]
    pub security: SecurityConfig,

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

    /// Bot version (e.g., "5.2.0")
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
// 🆕 V5.2: SECURITY CONFIGURATION (PR #41)
// Keypair management and transaction safety limits for live trading
// ═══════════════════════════════════════════════════════════════════════════

/// Security configuration for live trading.
///
/// Required when `bot.execution_mode = "live"`. Ignored in paper mode.
///
/// # Example (config/master.toml)
/// ```toml
/// [security]
/// keypair_path = "~/.config/solana/id.json"
/// max_transaction_amount_usdc = 50.0
/// max_daily_trades = 100
/// max_daily_volume_usdc = 500.0
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecurityConfig {
    /// Path to Solana keypair file (JSON format).
    /// Standard Solana CLI default: ~/.config/solana/id.json
    /// For devnet testing: ~/.config/solana/devnet-keypair.json
    #[serde(default = "default_keypair_path")]
    pub keypair_path: String,

    /// Maximum USDC amount per single transaction (safety cap).
    /// Prevents accidental large trades. None = no limit.
    #[serde(default)]
    pub max_transaction_amount_usdc: Option<f64>,

    /// Maximum number of trades per 24-hour period.
    /// Prevents runaway bot behavior. None = no limit.
    #[serde(default)]
    pub max_daily_trades: Option<u32>,

    /// Maximum total trading volume (USDC) per 24-hour period.
    /// Cumulative safety cap. None = no limit.
    #[serde(default)]
    pub max_daily_volume_usdc: Option<f64>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            keypair_path: default_keypair_path(),
            max_transaction_amount_usdc: Some(50.0),  // $50 per trade cap
            max_daily_trades: Some(100),              // 100 trades/day max
            max_daily_volume_usdc: Some(500.0),       // $500/day volume cap
        }
    }
}

impl SecurityConfig {
    pub fn validate(&self, is_live_mode: bool) -> Result<()> {
        // Only enforce validation in live mode
        if !is_live_mode {
            return Ok(());
        }

        if self.keypair_path.is_empty() {
            bail!(
                "security.keypair_path is required for live mode.\n\
                 Add [security] section to your config with keypair_path."
            );
        }

        // Validate file exists (expand tilde if present)
        let expanded_path = shellexpand::tilde(&self.keypair_path);
        let path = Path::new(expanded_path.as_ref());
        if !path.exists() {
            bail!(
                "Keypair file not found: {}\n\
                 Generate one with: solana-keygen new -o {}",
                expanded_path, expanded_path
            );
        }

        // Validate safety caps are reasonable
        if let Some(max_tx) = self.max_transaction_amount_usdc {
            if max_tx <= 0.0 {
                bail!("max_transaction_amount_usdc must be positive");
            }
            if max_tx > 10_000.0 {
                warn!(
                    "⚠️ Very high max_transaction_amount_usdc: ${:.2}. \
                     Consider lowering for safety.",
                    max_tx
                );
            }
        }

        if let Some(max_trades) = self.max_daily_trades {
            if max_trades == 0 {
                bail!("max_daily_trades cannot be 0");
            }
            if max_trades > 1000 {
                warn!("⚠️ Very high max_daily_trades: {}. High-frequency trading?", max_trades);
            }
        }

        if let Some(max_vol) = self.max_daily_volume_usdc {
            if max_vol <= 0.0 {
                bail!("max_daily_volume_usdc must be positive");
            }
            if max_vol > 100_000.0 {
                warn!("⚠️ Very high max_daily_volume_usdc: ${:.2}", max_vol);
            }
        }

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
/// max_trade_size_usdc = 50.0
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

    /// Maximum USDC amount per single trade (safety cap).
    /// Complements max_trade_sol for fiat-denominated risk management.
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
        if self.max_trade_size_usdc <= 0.0 {
            bail!("execution.max_trade_size_usdc must be positive");
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

// [Rest of the file continues unchanged from here...]
// (I'm truncating for readability — the rest stays identical)

