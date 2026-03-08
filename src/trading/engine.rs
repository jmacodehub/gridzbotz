//! ═══════════════════════════════════════════════════════════════════════════
//! 🏭 ENGINE FACTORY V2.2 — Config-Driven Engine Selection
//!
//! PR #72 — Phase 2: Engine Wiring
//! PR #77 — Phase 4: FeesConfig Wiring (single source of truth)
//! PR #79 — Commit 1: Update from_execution_config → from_config
//!
//! The single entry point for creating a TradingEngine from config.
//! Reads `bot.execution_mode` and returns the correct engine:
//!
//!   "paper" → PaperTradingEngine  (instant, no network, with fees/slippage)
//!   "live"  → RealTradingEngine   (on-chain balances, Pyth price, keystore)
//!
//! V2.0 CHANGES (PR #72):
//! ✅ EngineParams for runtime context (live_price, wallet_balances)
//! ✅ Paper mode: fees + slippage from config.execution
//! ✅ Live mode: KeystoreConfig wiring, $10 capital safety check
//! ✅ fetch_pyth_price extracted to price_feed_utils module
//! ✅ Matches all behavior from main.rs initialize_components()
//!
//! V2.1 CHANGES (PR #77):
//! ✅ Paper mode: fees + slippage from [fees] config (single source of truth)
//! ✅ Removed manual BPS→fraction conversions — uses FeesConfig helpers
//! ✅ Fixed slippage source: config.fees (expected cost) not config.execution
//! ✅ Dynamic log lines — no more hardcoded "2/4bps"
//!
//! V2.2 CHANGES (PR #79 Commit 1):
//! ✅ from_execution_config(&config.execution) → from_config(config)
//!    RealTradingConfig now receives full Config so it can wire:
//!    rpc_url (Chainstack), jupiter_api_key (env var), slippage, trade size.
//!    Passing only ExecutionConfig was insufficient — silently left rpc_url
//!    as None and jupiter_api_key as None, causing live mode failures.
//!
//! March 2026 — V2.2 🚀
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
/// Provides context that only the caller knows (live SOL price from
/// an already-running feed, on-chain wallet balances).
///
/// - **Paper mode**: Use `EngineParams::default()` — everything comes from config.
/// - **Live mode**: Provide `live_price` and `wallet_balances` for production.
///   If omitted, the factory will attempt to fetch/fallback automatically.
#[derive(Debug, Clone, Default)]
pub struct EngineParams {
    /// Pre-fetched SOL price from running price feed.
    /// If None in live mode, factory fetches from Pyth HTTP.
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
/// - Reads initial balances from `config.paper_trading`
/// - Applies fees + slippage from `config.fees` (single source of truth)
/// - No network calls required
///
/// # Live Mode (`execution_mode = "live"`)
/// - Uses `params.wallet_balances` for real on-chain capital
/// - Uses `params.live_price` or fetches from Pyth HTTP
/// - Validates capital ≥ $10
/// - Wires KeystoreConfig with wallet path and trade limits
///
/// # Errors
/// - Invalid `execution_mode` (not "paper" or "live")
/// - Paper: both USDC and SOL are ≤ 0
/// - Live: capital below $10, Pyth unreachable, wallet config invalid
pub async fn create_engine(
    config: &Config,
    params: EngineParams,
) -> Result<Arc<dyn TradingEngine>> {
    let instance = config.bot.instance_name();
    let mode = config.bot.execution_mode.as_str();

    info!(
        "[{}] 🏭 Engine Factory V2.2: creating engine for mode='{}'",
        instance, mode
    );

    match mode {
        "paper" => {
            let engine = from_config_paper(config)?;
            info!(
                "[{}] ✅ PaperTradingEngine ready (${:.0} USDC + {:.1} SOL, fees={:.0}/{:.0}bps, slippage={:.0}bps)",
                instance,
                config.paper_trading.initial_usdc,
                config.paper_trading.initial_sol,
                config.fees.maker_fee_bps,
                config.fees.taker_fee_bps,
                config.fees.slippage_bps
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

/// Paper mode: balances from config + fees from FeesConfig.
///
/// All fee parameters sourced from `config.fees` (FeesConfig):
/// - maker_fee → config.fees.maker_fee_fraction()
/// - taker_fee → config.fees.taker_fee_fraction()
/// - slippage  → config.fees.slippage_fraction()
///
/// Zero hardcoded values — single source of truth.
fn from_config_paper(config: &Config) -> Result<PaperTradingEngine> {
    let usdc = config.paper_trading.initial_usdc;
    let sol  = config.paper_trading.initial_sol;

    if usdc <= 0.0 || sol <= 0.0 {
        bail!(
            "Paper trading requires positive initial capital for both tokens. \
             Got: initial_usdc={}, initial_sol={}. \
             Check [paper_trading] in your TOML config.",
            usdc, sol
        );
    }

    info!(
        "   Capital: ${:.2} USDC + {:.4} SOL | Fees: maker {:.2}%, taker {:.2}% | Slippage: {:.2}%",
        usdc, sol,
        config.fees.maker_fee_percent(),
        config.fees.taker_fee_percent(),
        config.fees.slippage_percent()
    );

    let engine = PaperTradingEngine::new(usdc, sol)
        .with_fees_config(&config.fees);

    Ok(engine)
}

/// Live mode: on-chain balances + Pyth price + keystore + capital check.
///
/// Uses EngineParams for runtime context:
/// - wallet_balances: from fetch_wallet_balances() in main.rs
/// - live_price: from running PriceFeed
///
/// Falls back gracefully if params are not provided (testing, CLI tools).
async fn from_config_live(config: &Config, params: &EngineParams) -> Result<RealTradingEngine> {
    let instance = config.bot.instance_name();

    // ── Step 1: Resolve wallet balances ──────────────────────────────────────
    let (initial_usdc, initial_sol) = match params.wallet_balances {
        Some((usdc, sol)) => {
            info!("[{}] 💰 Using pre-fetched on-chain balances: ${:.2} USDC + {:.4} SOL",
                  instance, usdc, sol);
            (usdc, sol)
        }
        None => {
            warn!(
                "[{}] ⚠️  No wallet balances provided — falling back to paper_trading config. \
                 For production, pass wallet_balances via EngineParams.",
                instance
            );
            (config.paper_trading.initial_usdc, config.paper_trading.initial_sol)
        }
    };

    // ── Step 2: Resolve live SOL price ───────────────────────────────────────
    let sol_price = match params.live_price {
        Some(price) if price > 0.0 => {
            info!("[{}] 📡 Using pre-fetched SOL price: ${:.4}", instance, price);
            price
        }
        _ => {
            info!("[{}] 📡 No live price provided — fetching from Pyth...", instance);
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

    info!("[{}] 💰 SOL price: ${:.4}", instance, sol_price);

    // ── Step 3: Capital safety check ($10 minimum) ──────────────────────────
    let capital_usd = initial_usdc + (initial_sol * sol_price);
    if capital_usd < 10.0 {
        bail!(
            "[{}] Live mode requires minimum $10 capital.\n\
             On-chain: ${:.2} (USDC: ${:.2} + SOL: {:.4} @ ${:.4})\n\
             Fund your wallet before starting live trading.",
            instance, capital_usd, initial_usdc, initial_sol, sol_price
        );
    }
    info!("[{}] 💵 Total capital: ${:.2} USD", instance, capital_usd);

    // ── Step 4: Build RealTradingConfig + KeystoreConfig ─────────────────────
    // ✅ V2.2 (PR #79): from_config(config) wires rpc_url + jupiter_api_key.
    //    Previously from_execution_config(&config.execution) left both as None,
    //    causing silent public RPC fallback and swap failures at runtime.
    let mut real_config = RealTradingConfig::from_config(config);
    real_config.keystore = KeystoreConfig {
        keypair_path:                config.security.wallet_path.clone(),
        max_transaction_amount_usdc: Some(config.execution.max_trade_size_usdc),
        max_daily_trades:            None,
        max_daily_volume_usdc:       None,
    };

    info!("[{}]    Slippage: {:.4}%", instance,
          real_config.slippage_bps.unwrap_or(50) as f64 / 100.0);
    info!("[{}]    Keypair:  {}", instance, config.security.wallet_path);

    // ── Step 5: Construct engine ─────────────────────────────────────────────
    let engine = RealTradingEngine::new(
        real_config,
        config,
        initial_usdc,
        initial_sol,
        sol_price,
    ).await
        .context(format!("[{}] Failed to construct RealTradingEngine", instance))?;

    Ok(engine)
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigBuilder;

    /// Helper to build a minimal paper-mode config for testing.
    fn paper_config(usdc: f64, sol: f64) -> Config {
        let mut config = ConfigBuilder::new()
            .execution_mode("paper")
            .build()
            .expect("base test config should be valid");
        config.paper_trading.initial_usdc = usdc;
        config.paper_trading.initial_sol  = sol;
        config
    }

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

    #[test]
    fn test_engine_params_default() {
        let params = EngineParams::default();
        assert!(params.live_price.is_none());
        assert!(params.wallet_balances.is_none());
    }

    #[test]
    fn test_engine_params_with_values() {
        let params = EngineParams {
            live_price:      Some(147.35),
            wallet_balances: Some((500.0, 3.5)),
        };
        assert_eq!(params.live_price.unwrap(), 147.35);
        let (usdc, sol) = params.wallet_balances.unwrap();
        assert_eq!(usdc, 500.0);
        assert_eq!(sol, 3.5);
    }
}
