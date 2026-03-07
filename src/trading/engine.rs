//! ═══════════════════════════════════════════════════════════════════════════
//! 🏭 ENGINE FACTORY V2.0 — Config-Driven Engine Selection
//!
//! PR #72 — Phase 2: Engine Wiring
//!
//! The single entry point for creating a TradingEngine from config.
//! Reads `bot.execution_mode` and returns the correct engine:
//!
//!   "paper" → PaperTradingEngine  (instant, no network, with fees + slippage)
//!   "live"  → RealTradingEngine   (wallet balances, keystore, capital check)
//!
//! V2.0 CHANGES (PR #72):
//! ✅ EngineParams struct — runtime context (live price, wallet balances)
//! ✅ Paper mode: fees (2/4 bps) + slippage from config.execution
//! ✅ Live mode: wallet_balances from params, keystore wiring, $10 minimum
//! ✅ fetch_pyth_price extracted to price_feed_utils module
//! ✅ Matches main.rs V5.4 behavior exactly — convergence, not change
//!
//! Usage (in main.rs or any orchestrator):
//! ```ignore
//! use crate::trading::engine::{create_engine, EngineParams};
//!
//! // Paper mode — no runtime context needed
//! let engine = create_engine(&config, EngineParams::default()).await?;
//!
//! // Live mode — provide wallet balances + price from running feed
//! let params = EngineParams {
//!     live_price: Some(initial_price),
//!     wallet_balances: Some((usdc, sol)),
//! };
//! let engine = create_engine(&config, params).await?;
//! ```
//!
//! March 2026 — V2.0 LFG 🚀
//! ═══════════════════════════════════════════════════════════════════════════

use std::sync::Arc;
use anyhow::{Result, Context, bail};
use log::{info, warn};

use crate::config::Config;
use crate::security::keystore::KeystoreConfig;
use super::{
    TradingEngine,
    PaperTradingEngine,
    RealTradingEngine,
    RealTradingConfig,
};
use super::price_feed_utils::fetch_pyth_price;

// ═══════════════════════════════════════════════════════════════════════════
// PUBLIC API
// ═══════════════════════════════════════════════════════════════════════════

/// Runtime parameters for engine creation.
///
/// Provides context that only the caller (main.rs) knows at startup:
/// - Live price from the already-running price feed
/// - On-chain wallet balances from RPC query
///
/// Paper mode: use `EngineParams::default()` — everything from config.
/// Live mode: populate `live_price` and `wallet_balances` for production.
#[derive(Default)]
pub struct EngineParams {
    /// Pre-fetched SOL price from running price feed.
    /// If None in live mode, factory will fetch from Pyth HTTP.
    pub live_price: Option<f64>,

    /// On-chain wallet balances: (usdc, sol).
    /// Required for live mode production use.
    /// If None in live mode, falls back to paper_trading config values (with warning).
    pub wallet_balances: Option<(f64, f64)>,
}

/// Create a TradingEngine from config + runtime params.
///
/// This is the **only** function the orchestrator needs to call.
/// It reads `config.bot.execution_mode` and returns the correct engine
/// wrapped in `Arc<dyn TradingEngine>` for thread-safe sharing.
///
/// # Paper Mode (`execution_mode = "paper"`)
/// - Reads capital from `config.paper_trading`
/// - Applies fees: maker=2bps, taker=4bps
/// - Applies slippage from `config.execution.max_slippage_bps`
/// - No network calls, safe on any cluster
///
/// # Live Mode (`execution_mode = "live"`)
/// - Uses `params.wallet_balances` for real on-chain funds
/// - Uses `params.live_price` or falls back to Pyth HTTP fetch
/// - Wires `KeystoreConfig` with keypair path from security config
/// - Validates minimum capital ($10)
///
/// # Errors
/// - Invalid `execution_mode` (not "paper" or "live")
/// - Paper: both initial_usdc and initial_sol are zero/negative
/// - Live: insufficient capital (<$10)
/// - Live: Pyth unreachable (when no live_price provided)
pub async fn create_engine(
    config: &Config,
    params: EngineParams,
) -> Result<Arc<dyn TradingEngine>> {
    let instance = config.bot.instance_name();
    let mode = config.bot.execution_mode.as_str();

    info!(
        "[{}] 🏭 Engine Factory V2.0: mode='{}'",
        instance, mode
    );

    match mode {
        "paper" => {
            let engine = from_config_paper(config)?;
            info!(
                "[{}] ✅ PaperTradingEngine ready (${:.0} USDC + {:.1} SOL, fees=2/4bps)",
                instance,
                config.paper_trading.initial_usdc,
                config.paper_trading.initial_sol
            );
            Ok(Arc::new(engine))
        }
        "live" => {
            let engine = from_config_live(config, &params).await?;
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
/// V2.0: Now applies fees and slippage to match main.rs V5.4 behavior.
/// - Maker fee: 2 BPS (0.02%)
/// - Taker fee: 4 BPS (0.04%)
/// - Slippage: from config.execution.max_slippage_bps
fn from_config_paper(config: &Config) -> Result<PaperTradingEngine> {
    let usdc = config.paper_trading.initial_usdc;
    let sol = config.paper_trading.initial_sol;

    // Match main.rs V5.4: both must be positive
    if usdc <= 0.0 || sol <= 0.0 {
        bail!(
            "Paper trading requires positive initial capital for both currencies. \
             Got: initial_usdc={}, initial_sol={}. \
             Check [paper_trading] in your TOML config.",
            usdc, sol
        );
    }

    // Fees: 2 BPS maker, 4 BPS taker (matching main.rs V5.4)
    let maker_fee = 2.0_f64 / 10_000.0;
    let taker_fee = 4.0_f64 / 10_000.0;
    let slippage = config.execution.max_slippage_bps as f64 / 10_000.0;

    info!(
        "   Paper config: ${:.2} USDC + {:.4} SOL | fees {:.4}%/{:.4}% | slippage {:.4}%",
        usdc, sol,
        maker_fee * 100.0, taker_fee * 100.0,
        slippage * 100.0
    );

    Ok(
        PaperTradingEngine::new(usdc, sol)
            .with_fees(maker_fee, taker_fee)
            .with_slippage(slippage)
    )
}

/// Construct a RealTradingEngine from config + runtime params.
///
/// V2.0 Steps:
/// 1. Resolve wallet balances (from params or config fallback)
/// 2. Resolve live SOL price (from params or Pyth fetch)
/// 3. Capital safety check ($10 minimum)
/// 4. Build RealTradingConfig with KeystoreConfig
/// 5. Construct engine
async fn from_config_live(config: &Config, params: &EngineParams) -> Result<RealTradingEngine> {
    let instance = config.bot.instance_name();

    // Step 1: Resolve wallet balances
    let (initial_usdc, initial_sol) = match params.wallet_balances {
        Some((usdc, sol)) => {
            info!("[{}] 💰 Using provided wallet balances: ${:.2} USDC + {:.4} SOL",
                  instance, usdc, sol);
            (usdc, sol)
        }
        None => {
            warn!(
                "[{}] ⚠️  No wallet_balances in EngineParams — falling back to paper_trading config. \
                 In production, pass real on-chain balances via EngineParams.",
                instance
            );
            (config.paper_trading.initial_usdc, config.paper_trading.initial_sol)
        }
    };

    // Step 2: Resolve live SOL price
    let sol_price = match params.live_price {
        Some(price) => {
            info!("[{}] 📈 Using provided live price: ${:.4}", instance, price);
            price
        }
        None => {
            info!("[{}] 📡 No live_price in params — fetching from Pyth...", instance);
            let feed_id = config.pyth.feed_ids.first()
                .context("No Pyth feed IDs configured. Add at least one to [pyth] feed_ids.")?;
            fetch_pyth_price(&config.pyth.http_endpoint, feed_id).await
                .context(format!(
                    "[{}] Failed to fetch SOL price from Pyth. \
                     Cannot start live engine without a price reference.",
                    instance
                ))?
        }
    };

    // Step 3: Capital safety check
    let capital_usd = initial_usdc + (initial_sol * sol_price);
    if capital_usd < 10.0 {
        bail!(
            "[{}] Live mode requires minimum $10 capital.\n\
             Current: ${:.2} (USDC: ${:.2} + SOL: {:.4} @ ${:.4})\n\
             Fund your wallet before starting live trading.",
            instance, capital_usd, initial_usdc, initial_sol, sol_price
        );
    }
    info!("[{}] 💰 Capital: ${:.2} (USDC: ${:.2} + SOL: {:.4} @ ${:.4})",
          instance, capital_usd, initial_usdc, initial_sol, sol_price);

    // Step 4: Build RealTradingConfig with KeystoreConfig
    let mut real_config = RealTradingConfig::from_execution_config(&config.execution);
    real_config.keystore = KeystoreConfig {
        keypair_path: config.security.wallet_path.clone(),
        max_transaction_amount_usdc: Some(config.execution.max_trade_size_usdc),
        max_daily_trades: None,
        max_daily_volume_usdc: None,
    };

    info!("[{}] 🔑 Keypair: {}", instance, config.security.wallet_path);
    info!("[{}] ⚙️  Slippage: {:.4}%",
          instance,
          real_config.slippage_bps.unwrap_or(50) as f64 / 100.0);

    // Step 5: Construct engine
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
    use crate::config::ConfigBuilder;

    /// Helper: paper-mode config with specified balances.
    /// Uses ConfigBuilder for valid base, then overrides balances
    /// AFTER build() to test engine-level validation.
    fn paper_config(usdc: f64, sol: f64) -> Config {
        let mut config = ConfigBuilder::new()
            .execution_mode("paper")
            .build()
            .expect("base test config should be valid");
        config.paper_trading.initial_usdc = usdc;
        config.paper_trading.initial_sol = sol;
        config
    }

    // ── Paper Mode Tests ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_create_engine_paper_mode() {
        let config = paper_config(5000.0, 10.0);
        let engine = create_engine(&config, EngineParams::default()).await;
        assert!(engine.is_ok(), "Paper engine creation should succeed");

        let engine = engine.unwrap();
        assert!(engine.is_trading_allowed().await, "Paper engine should allow trading");
        assert_eq!(engine.open_order_count().await, 0);
    }

    #[tokio::test]
    async fn test_create_engine_paper_zero_usdc_fails() {
        // V2.0: Both USDC and SOL must be positive (|| check)
        let config = paper_config(0.0, 10.0);
        let result = create_engine(&config, EngineParams::default()).await;
        assert!(result.is_err(), "Zero USDC should fail");
        let err = result.err().unwrap().to_string();
        assert!(
            err.contains("positive initial capital"),
            "Error should mention capital: {}", err
        );
    }

    #[tokio::test]
    async fn test_create_engine_paper_zero_sol_fails() {
        let config = paper_config(5000.0, 0.0);
        let result = create_engine(&config, EngineParams::default()).await;
        assert!(result.is_err(), "Zero SOL should fail");
        let err = result.err().unwrap().to_string();
        assert!(
            err.contains("positive initial capital"),
            "Error should mention capital: {}", err
        );
    }

    #[tokio::test]
    async fn test_create_engine_paper_both_zero_fails() {
        let config = paper_config(0.0, 0.0);
        let result = create_engine(&config, EngineParams::default()).await;
        assert!(result.is_err(), "Both zero should fail");
    }

    // ── from_config_paper Tests ──────────────────────────────────────────

    #[test]
    fn test_from_config_paper_applies_fees() {
        // Verify that from_config_paper returns Ok for valid capital.
        // (Fee verification requires PaperTradingEngine internals,
        //  but we at least confirm the builder chain doesn't panic.)
        let config = paper_config(1000.0, 5.0);
        let result = from_config_paper(&config);
        assert!(result.is_ok(), "Valid paper config should produce engine");
    }

    // ── Engine Mode Label Tests ──────────────────────────────────────────

    #[test]
    fn test_engine_mode_label_paper() {
        let config = paper_config(1000.0, 1.0);
        assert_eq!(engine_mode_label(&config), "🟡 PAPER");
    }

    #[test]
    fn test_engine_mode_label_live() {
        let mut config = ConfigBuilder::new()
            .build()
            .expect("default test config should be valid");
        config.bot.execution_mode = "live".to_string();
        assert_eq!(engine_mode_label(&config), "🔴 LIVE");
    }

    // ── Invalid Mode Tests ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_create_engine_invalid_mode_fails() {
        let mut config = paper_config(1000.0, 5.0);
        config.bot.execution_mode = "yolo".to_string();
        let result = create_engine(&config, EngineParams::default()).await;
        assert!(result.is_err(), "Invalid mode should fail");
        let err = result.err().unwrap().to_string();
        assert!(
            err.contains("Invalid execution_mode"),
            "Error should mention invalid mode: {}", err
        );
    }

    // ── EngineParams Tests ───────────────────────────────────────────────

    #[test]
    fn test_engine_params_default() {
        let params = EngineParams::default();
        assert!(params.live_price.is_none());
        assert!(params.wallet_balances.is_none());
    }

    #[test]
    fn test_engine_params_with_values() {
        let params = EngineParams {
            live_price: Some(147.35),
            wallet_balances: Some((5000.0, 10.0)),
        };
        assert_eq!(params.live_price.unwrap(), 147.35);
        let (usdc, sol) = params.wallet_balances.unwrap();
        assert_eq!(usdc, 5000.0);
        assert_eq!(sol, 10.0);
    }
}
