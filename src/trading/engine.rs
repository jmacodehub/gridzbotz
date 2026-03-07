//! ═══════════════════════════════════════════════════════════════════════════
//! 🏭 ENGINE FACTORY V1.0 — Config-Driven Engine Selection
//!
//! PR #71 — Phase 2: Engine Factory
//!
//! The single entry point for creating a TradingEngine from config.
//! Reads `bot.execution_mode` and returns the correct engine:
//!
//!   "paper" → PaperTradingEngine  (instant, no network)
//!   "live"  → RealTradingEngine   (fetches SOL price, validates wallet)
//!
//! Usage (in gridz_bot.rs or any orchestrator):
//! ```ignore
//! use crate::trading::engine::create_engine;
//!
//! let engine: Arc<dyn TradingEngine> = create_engine(&config).await?;
//! // That's it. Paper or live, determined purely by TOML.
//! ```
//!
//! Design Principles:
//! • Zero hardcoded values — everything from Config
//! • Fail fast — validates all requirements before constructing
//! • Multi-bot safe — stateless factory, instance_name() in all logs
//! • Single return type — Arc<dyn TradingEngine> for both modes
//!
//! March 2026 — V1.0 LFG 🚀
//! ═══════════════════════════════════════════════════════════════════════════

use std::sync::Arc;
use anyhow::{Result, Context, bail};
use log::info;

use crate::config::Config;
use super::{
    TradingEngine,
    PaperTradingEngine,
    RealTradingEngine,
    RealTradingConfig,
};

// ═══════════════════════════════════════════════════════════════════════════
// PUBLIC API
// ═══════════════════════════════════════════════════════════════════════════

/// Create a TradingEngine from config.
///
/// This is the **only** function the orchestrator needs to call.
/// It reads `config.bot.execution_mode` and returns the correct engine
/// wrapped in `Arc<dyn TradingEngine>` for thread-safe sharing.
///
/// # Paper Mode (`execution_mode = "paper"`)
/// - Instant construction, no network calls
/// - Uses `config.paper_trading.initial_usdc` and `initial_sol`
/// - Safe to run on any cluster (devnet, testnet, mainnet)
///
/// # Live Mode (`execution_mode = "live"`)
/// - Fetches current SOL price from Pyth HTTP feed
/// - Validates wallet file exists and is readable
/// - Validates execution config (slippage, fees, retries)
/// - Requires `config.network.cluster` and valid RPC
///
/// # Errors
/// - Invalid `execution_mode` value (not "paper" or "live")
/// - Live mode: Pyth price feed unreachable
/// - Live mode: Wallet file missing or unreadable
/// - Live mode: Execution config validation failure
pub async fn create_engine(config: &Config) -> Result<Arc<dyn TradingEngine>> {
    let instance = config.bot.instance_name();
    let mode = config.bot.execution_mode.as_str();

    info!(
        "[{}] 🏭 Engine Factory: creating engine for mode='{}'",
        instance, mode
    );

    match mode {
        "paper" => {
            let engine = from_config_paper(config)?;
            info!(
                "[{}] ✅ PaperTradingEngine ready (${:.0} USDC + {:.1} SOL)",
                instance,
                config.paper_trading.initial_usdc,
                config.paper_trading.initial_sol
            );
            Ok(Arc::new(engine))
        }
        "live" => {
            let engine = from_config_live(config).await?;
            info!(
                "[{}] 🔴 RealTradingEngine ready (slippage {} BPS)",
                instance,
                config.execution.max_slippage_bps
            );
            Ok(Arc::new(engine))
        }
        other => {
            bail!(
                "[{}] Invalid execution_mode '{}'. Must be 'paper' or 'live'. \
                 Check [bot] execution_mode in your TOML config.",
                instance, other
            );
        }
    }
}

/// Returns a human-readable label for the current engine mode.
///
/// Useful for log prefixes, metrics labels, and status displays.
pub fn engine_mode_label(config: &Config) -> &'static str {
    if config.bot.is_live() {
        "🔴 LIVE"
    } else {
        "🟡 PAPER"
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// INTERNAL CONSTRUCTORS
// ═══════════════════════════════════════════════════════════════════════════

/// Construct a PaperTradingEngine from config.
///
/// Reads initial balances from `config.paper_trading`.
/// No network calls, no validation beyond what Config::validate() already did.
fn from_config_paper(config: &Config) -> Result<PaperTradingEngine> {
    let usdc = config.paper_trading.initial_usdc;
    let sol = config.paper_trading.initial_sol;

    if usdc <= 0.0 && sol <= 0.0 {
        bail!(
            "Paper trading requires positive initial capital. \
             Got: initial_usdc={}, initial_sol={}. \
             Check [paper_trading] in your TOML config.",
            usdc, sol
        );
    }

    Ok(PaperTradingEngine::new(usdc, sol))
}

/// Construct a RealTradingEngine from config.
///
/// Steps:
/// 1. Build RealTradingConfig via from_execution_config() bridge
/// 2. Fetch live SOL price from Pyth HTTP feed
/// 3. Construct engine with validated config + live price
///
/// Fails fast if Pyth is unreachable or wallet/keystore is invalid.
async fn from_config_live(config: &Config) -> Result<RealTradingEngine> {
    let instance = config.bot.instance_name();

    // Step 1: Build RealTradingConfig using the canonical bridge
    // from_execution_config() maps [execution] TOML fields into the
    // RealTradingConfig struct with sensible defaults for keystore,
    // executor, circuit breaker, and fee settings.
    info!("[{}] 📋 Building RealTradingConfig from [execution] section", instance);
    let real_config = RealTradingConfig::from_execution_config(&config.execution);

    // Step 2: Fetch live SOL price from Pyth
    info!("[{}] 📡 Fetching live SOL price from Pyth...", instance);
    let feed_id = config.pyth.feed_ids.first()
        .context("No Pyth feed IDs configured. Add at least one to [pyth] feed_ids.")?;

    let sol_price = super::get_live_price(feed_id).await
        .context(format!(
            "[{}] Failed to fetch SOL price from Pyth. \
             Cannot start live engine without a price reference. \
             Check network connectivity and [pyth] config.",
            instance
        ))?;

    info!("[{}] 💰 SOL price: ${:.4}", instance, sol_price);

    // Step 3: Initial capital from paper_trading config
    // (In production, these would come from on-chain wallet query)
    let initial_usdc = config.paper_trading.initial_usdc;
    let initial_sol = config.paper_trading.initial_sol;

    // Step 4: Construct engine (async + returns Result)
    let engine = RealTradingEngine::new(
        real_config,
        config,
        initial_usdc,
        initial_sol,
        sol_price,
    ).await?;

    Ok(engine)
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to build a minimal paper-mode config for testing.
    fn paper_config(usdc: f64, sol: f64) -> Config {
        let mut config = Config::default();
        config.bot.execution_mode = "paper".to_string();
        config.paper_trading.initial_usdc = usdc;
        config.paper_trading.initial_sol = sol;
        config
    }

    #[tokio::test]
    async fn test_create_engine_paper_mode() {
        let config = paper_config(5000.0, 10.0);
        let engine = create_engine(&config).await;
        assert!(engine.is_ok(), "Paper engine creation should succeed");

        let engine = engine.unwrap();
        assert!(engine.is_trading_allowed().await, "Paper engine should allow trading");
        assert_eq!(engine.open_order_count().await, 0);
    }

    #[tokio::test]
    async fn test_create_engine_paper_zero_capital_fails() {
        let config = paper_config(0.0, 0.0);
        let result = create_engine(&config).await;
        assert!(result.is_err(), "Zero capital should fail");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("positive initial capital"),
            "Error should mention capital: {}", err
        );
    }

    #[test]
    fn test_engine_mode_label_paper() {
        let config = paper_config(1000.0, 0.0);
        assert_eq!(engine_mode_label(&config), "🟡 PAPER");
    }

    #[test]
    fn test_engine_mode_label_live() {
        let mut config = Config::default();
        config.bot.execution_mode = "live".to_string();
        assert_eq!(engine_mode_label(&config), "🔴 LIVE");
    }

    #[tokio::test]
    async fn test_create_engine_invalid_mode_fails() {
        let mut config = paper_config(1000.0, 0.0);
        config.bot.execution_mode = "yolo".to_string();
        let result = create_engine(&config).await;
        assert!(result.is_err(), "Invalid mode should fail");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Invalid execution_mode"),
            "Error should mention invalid mode: {}", err
        );
    }
}
