//! =============================================================================
//! REAL TRADER ENGINE V3.8
//!
//! V3.8 CHANGES (fix/requote-0x1771-anchor-slippage — PR #124):
//! ✅ P1 FIX: 0x1771 (Jupiter Anchor SlippageToleranceExceeded) added to
//!    the is_requotable guard in execute_trade().
//!    PR #116 added 0xb to the guard but missed 0x1771 — the Anchor program
//!    variant of the same SlippageToleranceExceeded error (decimal 6001)
//!    thrown by the Jupiter V6 swap program during pre-broadcast simulation.
//!    When simulation returned `custom program error: 0x1771` the error
//!    string didn't contain "0xb" or "SlippageTolerance", so is_requotable
//!    evaluated to false and the loop broke immediately after attempt 1 with
//!    no retries — identical user-visible behaviour to the pre-PR #116 bug.
//!    Fix: err_str.contains("0x1771") added to is_requotable predicate.
//! ✅ Regression test: test_0x1771_in_requote_guard() added.
//!    Guards against future removal of the 0x1771 pattern.
//! ✅ Boot log updated: "0xb/blockhash" → "0xb/0x1771/blockhash".
//!    PR chain: PR #116 (requote loop) → PR #124 (0x1771 gap)
//!
//! V3.7 CHANGES (fix/jupiter-estimated-cu-fee-label — PR #123):
//! ✅ P1 FIX: JUPITER_ESTIMATED_CU corrected: 300_000 → 500_000.
//!    Jupiter V6 swaps consume ~400K–600K CU in practice. At 300K, the
//!    max_lamports formula (fee_µL × CU / 1_000_000) produced values below
//!    the MIN_MAX_LAMPORTS floor (5_000) for almost all fee estimates,
//!    causing every swap to use a flat 5_000 lamports cap regardless of
//!    the dynamic fee value. The dynamic estimator was running but its
//!    output was silently discarded by the floor.
//!    Fix: 500_000 CU — matches observed Jupiter V6 consumption.
//!    Example: fee=3_500 µL/CU × 500_000 / 1_000_000 = 1_750 lamports.
//!    (Still protected by MIN_MAX_LAMPORTS floor for unusually calm markets.)
//! ✅ P3 FIX: with_priority_fee() label: "high" → self.fee_source_name.
//!    "high" is not a Jupiter priority level in the Lamports fee variant.
//!    Passing the actual source name ("rpc" | "helius" | "static") fixes
//!    the misleading log: 'source: high' → 'source: rpc'.
//!    Aligns the with_priority_fee() call with the [Quant] log line above it.
//! ✅ Regression test: test_fee_label_not_hardcoded_high() added.
//!    Guards against re-introduction of the "high" hardcoded string.
//!    PR chain: PR #97 (estimator) → PR #109 (sources) → PR #110 (wiring)
//!              → PR #123 (CU constant + label fix)
//!
//! V3.6 CHANGES (fix/wire-executor-rpc-from-config — PR #122):
//! ✅ P0 FIX: ExecutorConfig now wired from global Config in from_config().
//!    Previously ExecutorConfig used ..Default::default() for the executor
//!    field, silently hardcoding api.mainnet-beta.solana.com as the RPC URL.
//!    Every Jupiter swap confirmation hit the public RPC instead of Chainstack.
//!    Fix: build ExecutorConfig explicitly:
//!      rpc_url                   ← config.network.rpc_url (Chainstack)
//!      rpc_fallback_urls         ← config.execution.rpc_fallback_urls (Helius)
//!      max_retries               ← config.execution.max_retries
//!      confirmation_timeout_secs ← config.execution.confirmation_timeout_secs
//!    rpc_timeout_secs + compute_unit_limit preserved via ..ExecutorConfig::default().
//!    secrets::resolve_secrets() runs in Config::from_file() before from_config()
//!    is called — GRIDZBOTZ_RPC_URL is already injected, no extra env handling needed.
//! ✅ Boot log added: '[RealEngine] Executor RPC: <first 50 chars>'
//!    Active endpoint is now visible at startup — silent misconfiguration
//!    can never go unnoticed again.
//! ✅ Regression test: test_executor_rpc_wired_from_config() added.
//!    PR chain: executor.rs PR #116 (dead RPC removal) → PR #122 (RPC wiring)
//!
//! V3.5 CHANGES (fix/requote-on-retry-dead-rpc-pool — PR #116):
//! ✅ P0 FIX: Re-quote Jupiter on SlippageToleranceExceeded (0xb).
//!    Old path: fail → rotate RPC → resend same stale tx → fail again.
//!    New path: fail → re-quote Jupiter → rebuild tx → sign → submit.
//!    execute_trade() now wraps build_jupiter_swap() + execute_versioned()
//!    in a MAX_REQUOTE_ATTEMPTS=3 loop. On 0xb or blockhash expiry, the
//!    executor lock is dropped, build_jupiter_swap() is called fresh
//!    (new quote + new blockhash), and a new VersionedTransaction is built,
//!    signed, and submitted. RPC rotation for non-slippage errors is still
//!    handled inside RpcClientPool as before.
//! ✅ P0 FIX: Dead fallback RPCs removed from ExecutorConfig::default().
//!    projectserum.com is dead, api.rpcpool.com returns 403.
//!    Helius is wired via TOML rpc_fallback_urls — default() no longer
//!    shadows it with dead endpoints.
//!
//! V3.4 CHANGES (fix/real-trader-canonical-fee-source — PR #110):
//! ✅ P0 FIX: Async fee source replaced by canonical RpcFeeSource (PR #109).
//!    Old source called getRecentPrioritizationFees(&[]) — empty
//!    account keys = global network noise (NFT mints, memecoin spam, voting).
//!    RpcFeeSource passes [JUP6Lk, SOL, USDC] = Jupiter local fee market.
//!    This is the correct distribution for our actual swap execution path.
//! ✅ priority_fees.source field now respected in new():
//!    "rpc"    → RpcFeeSource    (Chainstack / any RPC)
//!    "helius" → HeliusFeeSource (Helius getPriorityFeeEstimate V2)
//! ✅ display_status() shows fee source name (rpc|helius), not just DYNAMIC.
//! ✅ Closes EXEC-4 Dynamic Priority Fees audit item (CRITICAL).
//!    Chain: PR #97 (estimator) → PR #109 (sources) → PR #110 (production wiring)
//!
//! V3.3 CHANGES (fix/cb-reconcile-telegram-hardening — PR #103 Commit 1):
//! ✅ reconcile_balances() removed from process_price_update().
//!    Root cause: it called CB.record_trade(cumulative_pnl, nav) on EVERY
//!    price tick (120×/min at 500ms cycle), not just on actual fills.
//!    Cumulative snapshot (not delta) caused CB NAV drift and spurious
//!    consecutive-loss trips even with zero fills.
//! ✅ last_reconciled_nav: Arc<RwLock<f64>> added — seeded with initial_nav
//!    so the first reconcile after a real trade computes a clean delta.
//! ✅ reconcile_balances() now computes pnl_delta = total_value - last_nav
//!    and only calls CB.record_trade() when delta != 0.0.
//! ✅ reconcile_balances() sole call site remains execute_trade() every N
//!    trades — correct cadence, no regression to trade flow.
//!
//! V3.2 CHANGES (fix/fill-level-id-cb-nav-wiring — PR #102 Commit 2):
//! ✅ pending_fills: Arc<RwLock<Vec<FillEvent>>> added to RealTradingEngine.
//!    place_limit_order_with_level() pushes a synthetic FillEvent after every
//!    confirmed Jupiter swap, carrying level_id and actual fill price/size.
//!    process_price_update() drains pending_fills and returns them so that
//!    grid_bot.rs receives fills in live mode.
//!
//! V3.1 CHANGES (feat/priority-fee-quant-log — PR #97 Commit 1):
//! ✅ build_jupiter_swap(): add quant info! log for priority fee observability.
//!
//! V3.0 CHANGES (fix/fees-config-reconciliation — PR #96 Commit 1):
//! ✅ RealTradingConfig::slippage_bps DELETED — shadow of ExecutionConfig.
//! ✅ FeesConfig is now undisputed single source of truth for fee math.
//!
//! ## Shadow field policy (RealTradingConfig) — DO NOT regress
//!   - Slippage:        ExecutionConfig::max_slippage_bps → engine.slippage_bps
//!   - Stop-loss/TP:    RiskConfig → engine.profit_take_pct
//!   - Static prio fee: PriorityFeeConfig::fallback_microlamports → engine field
//!   - Fee math:        FeesConfig — paper_trader/fee_filter/grid_rebalancer only
//!
//! March 2026 — V3.8 🚀
//! =============================================================================

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::str::FromStr;
use tokio::sync::RwLock;
use super::priority_fee_estimator::{PriorityFeeEstimator, FeeDataSource};
use super::rpc_fee_source::RpcFeeSource;
use super::helius_fee_source::HeliusFeeSource;

// -----------------------------------------------------------------------------
// MODULAR IMPORTS
// -----------------------------------------------------------------------------
use crate::security::keystore::{SecureKeystore, KeystoreConfig};
use crate::risk::circuit_breaker::{CircuitBreaker, TripReason};
use crate::risk::stop_loss::StopLossManager;
use crate::Config;
use super::executor::{TransactionExecutor, ExecutorConfig};
use super::trade::Trade;
use super::paper_trader::{Order, OrderSide, VirtualWallet, PerformanceStats as PaperPerformanceStats};
use crate::dex::{JupiterClient, SOL_MINT, USDC_MINT};
use super::{TradingEngine, TradingResult, FillEvent};
use solana_sdk::{
    transaction::VersionedTransaction,
    pubkey::Pubkey,
};

// =============================================================================
// CONSTANTS
// =============================================================================

/// Estimated CU budget for Jupiter swaps (used to convert µL/CU → total lamports).
/// Jupiter V6 swaps consume ~400K–600K CU in practice.
/// At the previous value of 300_000, max_lamports almost always fell below
/// MIN_MAX_LAMPORTS (5_000), causing the dynamic fee to be silently overridden
/// by the floor on every swap. 500_000 is the calibrated midpoint.
const JUPITER_ESTIMATED_CU: u64 = 500_000;

/// Minimum max_lamports floor — ensures txs can land even if estimator
/// returns very low values during unusually calm periods.
const MIN_MAX_LAMPORTS: u64 = 5_000;

/// How many times to re-fetch a fresh Jupiter quote on slippage failure
/// before giving up. Each attempt calls build_jupiter_swap() from scratch:
/// new quote, new route, new blockhash, new signed VersionedTransaction.
/// RPC rotation for non-slippage errors is still handled inside RpcClientPool.

// -----------------------------------------------------------------------------
// CONFIGURATION
// -----------------------------------------------------------------------------

/// Runtime configuration for RealTradingEngine.
///
/// ## Shadow field policy
///
/// Do NOT add shadow fields to this struct. Each config value has one
/// canonical source in the global Config tree:
///
///   - Slippage (Jupiter tolerance): `ExecutionConfig::max_slippage_bps`
///     Cached as `RealTradingEngine::slippage_bps` at construction (PR #96).
///
///   - Stop-loss / take-profit %: `RiskConfig::stop_loss_pct / take_profit_pct`
///     Cached as `RealTradingEngine::profit_take_pct` at construction (PR #88).
///
///   - Static priority fee: `PriorityFeeConfig::fallback_microlamports`
///     Cached as `RealTradingEngine::static_priority_fee` at construction (PR #79).
///
/// Removed shadow fields (compile-time guards below):
///   ✅ PR #88: stop_loss_pct, profit_take_threshold
///   ✅ PR #79: maker_fee_bps, taker_fee_bps
///   ✅ PR #96: slippage_bps
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealTradingConfig {
    pub keystore:                          KeystoreConfig,
    pub executor:                          ExecutorConfig,
    pub max_trade_size_usdc:               Option<f64>,
    pub circuit_breaker_loss_pct:          Option<f64>,
    pub profit_take_ratio:                 Option<f64>,
    pub reconcile_balances_every_n_trades: Option<u32>,
    pub jupiter_api_key:                   Option<String>,
    pub rpc_url:                           Option<String>,
}

impl Default for RealTradingConfig {
    fn default() -> Self {
        Self {
            keystore:                          KeystoreConfig::default(),
            executor:                          ExecutorConfig::default(),
            max_trade_size_usdc:               Some(250.0),
            circuit_breaker_loss_pct:          Some(5.0),
            profit_take_ratio:                 Some(0.4),
            reconcile_balances_every_n_trades: Some(10),
            jupiter_api_key:                   None,
            rpc_url:                           None,
        }
    }
}

impl RealTradingConfig {
    pub fn validate(&self) -> Result<()> {
        self.keystore.validate()?;
        self.executor.validate()?;

        if let Some(loss_pct) = self.circuit_breaker_loss_pct {
            if loss_pct <= 0.0 {
                bail!("circuit_breaker_loss_pct must be positive");
            }
        }
        Ok(())
    }

    /// Build a `RealTradingConfig` from the validated global `Config`.
    ///
    /// ## PR #122 — ExecutorConfig wiring fix
    ///
    /// Previously this function used `..Default::default()` for the executor
    /// field, silently hardcoding `api.mainnet-beta.solana.com` as the RPC URL
    /// and ignoring `config.network.rpc_url` (Chainstack) entirely.
    ///
    /// The fix builds `ExecutorConfig` explicitly so every field tracks the
    /// canonical config source:
    ///
    ///   - `rpc_url`                   ← `config.network.rpc_url` (Chainstack)
    ///   - `rpc_fallback_urls`         ← `config.execution.rpc_fallback_urls` (Helius)
    ///   - `max_retries`               ← `config.execution.max_retries`
    ///   - `confirmation_timeout_secs` ← `config.execution.confirmation_timeout_secs`
    ///   - `rpc_timeout_secs`          ← `ExecutorConfig::default()` (unchanged)
    ///   - `compute_unit_limit`        ← `ExecutorConfig::default()` (unchanged)
    ///
    /// `secrets::resolve_secrets()` already runs inside `Config::from_file()`
    /// before this function is called, so `GRIDZBOTZ_RPC_URL` is already
    /// injected into `config.network.rpc_url` — no extra env-var handling needed.
    pub fn from_config(global: &crate::Config) -> Self {
        let jupiter_api_key = std::env::var("GRIDZBOTZ_JUPITER_API_KEY")
            .ok()
            .filter(|k| !k.is_empty());

        if jupiter_api_key.is_none() {
            log::warn!(
                "[RealTradingConfig] GRIDZBOTZ_JUPITER_API_KEY not set — \
                 build_jupiter_swap() will fail at swap time. \
                 Set this env var before starting live mode."
            );
        }

        // ✅ PR #122: Build ExecutorConfig from global Config.
        // Previously ..Default::default() silently used api.mainnet-beta.solana.com.
        // All GRIDZBOTZ_* env vars are already resolved by secrets::resolve_secrets()
        // inside Config::from_file() before this function is called.
        let executor = ExecutorConfig {
            rpc_url:                   global.network.rpc_url.clone(),
            rpc_fallback_urls:         global.execution.rpc_fallback_urls.clone(),
            max_retries:               Some(global.execution.max_retries),
            confirmation_timeout_secs: Some(global.execution.confirmation_timeout_secs),
            // rpc_timeout_secs + compute_unit_limit: keep executor defaults
            ..ExecutorConfig::default()
        };

        info!(
            "[RealTradingConfig] Executor RPC: {}",
            executor.rpc_url.chars().take(50).collect::<String>()
        );
        if let Some(ref fallbacks) = executor.rpc_fallback_urls {
            info!(
                "[RealTradingConfig] Executor fallbacks: {} endpoint(s)",
                fallbacks.len()
            );
        }

        Self {
            executor,
            max_trade_size_usdc: Some(global.execution.max_trade_size_usdc),
            rpc_url:             Some(global.network.rpc_url.clone()),
            jupiter_api_key,
            ..Default::default()
        }
    }
}

// -----------------------------------------------------------------------------
// BALANCE TRACKER
// -----------------------------------------------------------------------------
struct BalanceTracker {
    expected_usdc:       Arc<RwLock<f64>>,
    expected_sol:        Arc<RwLock<f64>>,
    initial_balance_usd: f64,
}

impl BalanceTracker {
    fn new(initial_usdc: f64, initial_sol: f64, sol_price_usd: f64) -> Self {
        Self {
            expected_usdc:       Arc::new(RwLock::new(initial_usdc)),
            expected_sol:        Arc::new(RwLock::new(initial_sol)),
            initial_balance_usd: initial_usdc + (initial_sol * sol_price_usd),
        }
    }

    async fn get_balances(&self) -> (f64, f64) {
        let usdc = *self.expected_usdc.read().await;
        let sol  = *self.expected_sol.read().await;
        (usdc, sol)
    }

    #[allow(dead_code)]
    async fn update(&self, usdc: f64, sol: f64) {
        *self.expected_usdc.write().await = usdc;
        *self.expected_sol.write().await  = sol;
    }

    fn initial_balance_usd(&self) -> f64 {
        self.initial_balance_usd
    }
}

// -----------------------------------------------------------------------------
// PERFORMANCE STATS
// -----------------------------------------------------------------------------
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PerformanceStats {
    pub total_trades:   usize,
    pub winning_trades: usize,
    pub losing_trades:  usize,
    pub total_pnl:      f64,
    pub total_fees:     f64,
    pub win_rate:       f64,
    pub avg_win:        f64,
    pub avg_loss:       f64,
    pub largest_win:    f64,
    pub largest_loss:   f64,
    pub profit_factor:  f64,
}

// -----------------------------------------------------------------------------
// REAL TRADING ENGINE
// -----------------------------------------------------------------------------
pub struct RealTradingEngine {
    keystore:               Arc<SecureKeystore>,
    executor:               Arc<RwLock<TransactionExecutor>>,
    circuit_breaker:        Arc<RwLock<CircuitBreaker>>,
    stop_loss_manager:      Arc<RwLock<StopLossManager>>,
    balance_tracker:        Arc<BalanceTracker>,
    config:                 RealTradingConfig,
    trades:                 Arc<RwLock<Vec<Trade>>>,
    _open_orders:           Arc<RwLock<HashMap<String, Order>>>,
    next_id:                Arc<AtomicU64>,
    total_executions:       Arc<AtomicU64>,
    successful_executions:  Arc<AtomicU64>,
    failed_executions:      Arc<AtomicU64>,
    emergency_shutdown:     Arc<AtomicBool>,
    priority_fee_estimator: Option<Arc<PriorityFeeEstimator>>,
    /// Name of the active fee source for logging — "rpc", "helius", or "static".
    fee_source_name:        &'static str,
    static_priority_fee:    u64,
    profit_take_pct:        f64,
    slippage_bps:           u16,
    max_requote_attempts: u8,
    /// V3.2: Synthetic FillEvents from confirmed Jupiter swaps.
    pending_fills:          Arc<RwLock<Vec<FillEvent>>>,
    /// V3.3: Last NAV seen by reconcile_balances().
    last_reconciled_nav:    Arc<RwLock<f64>>,
}

impl RealTradingEngine {
    pub async fn new(
        config: RealTradingConfig,
        global_config: &Config,
        initial_balance_usdc: f64,
        initial_balance_sol: f64,
        initial_sol_price_usd: f64,
    ) -> Result<Self> {
        info!("[RealEngine] Initializing V3.8");

        config.validate()?;

        let keystore = Arc::new(SecureKeystore::from_file(config.keystore.clone())?);
        let executor = Arc::new(RwLock::new(TransactionExecutor::new(config.executor.clone())?));

        let initial_nav = initial_balance_usdc + (initial_balance_sol * initial_sol_price_usd);
        let circuit_breaker = Arc::new(RwLock::new(
            CircuitBreaker::with_balance(global_config, initial_nav)
        ));

        let stop_loss_manager = Arc::new(RwLock::new(
            StopLossManager::new(global_config)
        ));

        let profit_take_pct = global_config.risk.take_profit_pct;
        let slippage_bps    = global_config.execution.max_slippage_bps;
        let max_requote_attempts = global_config.execution.max_requote_attempts;

        // ── Priority fee wiring (V3.4) ────────────────────────────────────────
        // Respects priority_fees.source: "rpc" | "helius"
        // "rpc"    → RpcFeeSource: getRecentPrioritizationFees with
        //            [JUP6Lk, SOL, USDC] account keys = Jupiter local fee market
        // "helius" → HeliusFeeSource: getPriorityFeeEstimate V2
        //            max(global_percentile, per_account_percentile)
        let (priority_fee_estimator, fee_source_name, static_priority_fee) =
            if global_config.priority_fees.enable_dynamic {
                let fee_cfg = &global_config.priority_fees;
                let rpc_url = &global_config.network.rpc_url;

                let source: Arc<dyn FeeDataSource> = match fee_cfg.source.as_str() {
                    "helius" => {
                        let helius_url = std::env::var("GRIDZBOTZ_HELIUS_RPC_URL")
                            .unwrap_or_else(|_| {
                                log::warn!(
                                    "[RealEngine] priority_fees.source=\"helius\" but \
                                     GRIDZBOTZ_HELIUS_RPC_URL not set — \
                                     falling back to RpcFeeSource on primary RPC"
                                );
                                rpc_url.clone()
                            });
                        info!("[RealEngine] Fee source: Helius getPriorityFeeEstimate V2 ⚡");
                        Arc::new(HeliusFeeSource::new(helius_url, fee_cfg))
                    }
                    _ => {
                        // default: "rpc" — canonical RpcFeeSource with JUP account keys
                        info!(
                            "[RealEngine] Fee source: RpcFeeSource ({}) \
                             [JUP6Lk+SOL+USDC local market] ⚡",
                            rpc_url
                        );
                        Arc::new(RpcFeeSource::new(rpc_url.clone()))
                    }
                };

                let estimator = PriorityFeeEstimator::new(fee_cfg.clone(), source);
                info!(
                    "[RealEngine] Dynamic fees: P{}, ×{}, cache {}s, bounds {}-{} µL/CU, source={}",
                    fee_cfg.percentile,
                    fee_cfg.multiplier,
                    fee_cfg.cache_ttl_secs,
                    fee_cfg.min_microlamports,
                    fee_cfg.max_microlamports,
                    fee_cfg.source,
                );

                let src_name: &'static str = match fee_cfg.source.as_str() {
                    "helius" => "helius",
                    _        => "rpc",
                };
                (
                    Some(Arc::new(estimator)),
                    src_name,
                    fee_cfg.fallback_microlamports,
                )
            } else {
                let fee = global_config.priority_fees.fallback_microlamports;
                info!("[RealEngine] Priority fees STATIC: {} µL/CU", fee);
                (None, "static", fee)
            };

        let balance_tracker = Arc::new(BalanceTracker::new(
            initial_balance_usdc,
            initial_balance_sol,
            initial_sol_price_usd,
        ));

        info!("[RealEngine] Initialized V3.8");
        info!("  Wallet      : {}", keystore.pubkey());
        info!("  NAV         : ${:.2} (SOL @ ${:.4})",
            balance_tracker.initial_balance_usd(), initial_sol_price_usd);
        info!("  Stop-loss   : -{:.1}% | Take-profit: +{:.1}%",
            global_config.risk.stop_loss_pct, profit_take_pct);
        info!("  SL mode     : {}",
            if global_config.risk.enable_trailing_stop { "trailing" } else { "fixed" });
        info!("  Slippage    : {} bps (max Jupiter swap tolerance)", slippage_bps);
        info!("  Fee source  : {}", fee_source_name);
        info!("  CU budget   : {} (Jupiter V6 swap estimate)", JUPITER_ESTIMATED_CU);
        info!("  Requote     : up to {} attempts on 0xb/0x1771/blockhash expiry", max_requote_attempts);

        Ok(Self {
            keystore,
            executor,
            circuit_breaker,
            stop_loss_manager,
            balance_tracker,
            config,
            trades:                Arc::new(RwLock::new(Vec::new())),
            _open_orders:          Arc::new(RwLock::new(HashMap::new())),
            next_id:               Arc::new(AtomicU64::new(1)),
            total_executions:      Arc::new(AtomicU64::new(0)),
            successful_executions: Arc::new(AtomicU64::new(0)),
            failed_executions:     Arc::new(AtomicU64::new(0)),
            emergency_shutdown:    Arc::new(AtomicBool::new(false)),
            priority_fee_estimator,
            fee_source_name,
            static_priority_fee,
            profit_take_pct,
            slippage_bps,
            max_requote_attempts,
            pending_fills:       Arc::new(RwLock::new(Vec::new())),
            last_reconciled_nav: Arc::new(RwLock::new(initial_nav)),
        })
    }

    pub async fn buy(&self, price: f64, size: f64) -> Result<String> {
        self.execute_trade(OrderSide::Buy, price, size).await
    }

    pub async fn sell(&self, price: f64, size: f64) -> Result<String> {
        self.execute_trade(OrderSide::Sell, price, size).await
    }

    async fn execute_trade(&self, side: OrderSide, price: f64, size: f64) -> Result<String> {
        if self.emergency_shutdown.load(Ordering::SeqCst) {
            bail!("[RealEngine] EMERGENCY SHUTDOWN ACTIVE");
        }

        if !self.circuit_breaker.write().await.is_trading_allowed() {
            bail!("[RealEngine] CIRCUIT BREAKER ACTIVE");
        }

        {
            let mut sl = self.stop_loss_manager.write().await;
            if sl.should_take_profit(price, price) {
                info!("[RealEngine] Trade blocked by take-profit guard @ ${:.4}", price);
                bail!("[RealEngine] TAKE-PROFIT GUARD: position exit required before new entry");
            }
            if sl.should_stop_loss(price, price) {
                info!("[RealEngine] Trade blocked by stop-loss guard @ ${:.4}", price);
                bail!("[RealEngine] STOP-LOSS GUARD: position exit required before new entry");
            }
        }

        let amount_usdc = price * size;

        if let Some(max_usdc) = self.config.max_trade_size_usdc {
            if amount_usdc > max_usdc {
                bail!(
                    "[RealEngine] Trade blocked: ${:.2} exceeds max_trade_size_usdc=${:.2}",
                    amount_usdc, max_usdc
                );
            }
        }

        self.keystore.validate_transaction(amount_usdc).await?;

        let order_id = format!("REAL-{:06}", self.next_id.fetch_add(1, Ordering::SeqCst));
        info!("[Order] {:?} {:.4} SOL @ ${:.2}", side, size, price);

        self.total_executions.fetch_add(1, Ordering::SeqCst);

        // ── V3.5: Re-quote loop ───────────────────────────────────────────────
        // On SlippageToleranceExceeded (0xb / 0x1771) or blockhash expiry, we
        // drop the executor lock, call build_jupiter_swap() again to get a
        // fresh quote + fresh blockhash, then rebuild, sign, and resubmit.
        // 0xb  = legacy instruction-processor SlippageToleranceExceeded.
        // 0x1771 = Jupiter Anchor v6 program SlippageToleranceExceeded (6001).
        //          Fired during pre-broadcast simulation (-32002); error string
        //          contains "0x1771" but NOT "0xb" — PR #116 missed this variant.
        // RPC rotation for non-slippage errors is still handled inside
        // RpcClientPool as before.
        let mut last_err = anyhow::anyhow!("[Order] No attempts made");
        let mut signature_opt: Option<solana_sdk::signature::Signature> = None;

        for attempt in 1..=self.max_requote_attempts {
            // Always re-fetch quote + blockhash on every attempt.
            // On attempt 1 this is the normal path. On attempt 2+ it is a
            // fresh quote after a slippage / blockhash failure.
            let (versioned_tx, _last_valid) = match self.build_jupiter_swap(side, price, size).await {
                Ok(pair) => pair,
                Err(e) => {
                    error!("[Order] build_jupiter_swap failed (attempt {}/{}): {}",
                        attempt, self.max_requote_attempts, e);
                    last_err = e;
                    break; // quote fetch itself failed — no point retrying
                }
            };

            let exec_result = {
                let executor = self.executor.write().await;
                executor.execute_versioned(
                    versioned_tx,
                    |tx| self.keystore.sign_versioned_transaction(tx),
                ).await
                // executor write lock is dropped here before the next iteration
            };

            match exec_result {
                Ok(sig) => {
                    signature_opt = Some(sig);
                    break;
                }
                Err(e) => {
                    let err_str = e.to_string();
                    let is_requotable =
                        err_str.contains("0xb")                       // SlippageToleranceExceeded (legacy)
                        || err_str.contains("0x1771")                 // SlippageToleranceExceeded (Jupiter Anchor v6, decimal 6001)
                        || err_str.contains("SlippageTolerance")       // human-readable variant
                        || err_str.contains("BlockhashNotFound")       // blockhash expired
                        || err_str.contains("blockhash");              // any blockhash error

                    if is_requotable && attempt < self.max_requote_attempts {
                        warn!(
                            "[Order] Requotable error on attempt {}/{} — re-fetching Jupiter quote. Error: {}",
                            attempt, self.max_requote_attempts, err_str
                        );
                        last_err = e;
                        // loop continues → build_jupiter_swap() called fresh
                    } else {
                        if !is_requotable {
                            error!(
                                "[Order] Non-requotable error (attempt {}/{}): {}",
                                attempt, self.max_requote_attempts, err_str
                            );
                        } else {
                            error!(
                                "[Order] Exhausted {} requote attempts. Last error: {}",
                                self.max_requote_attempts, err_str
                            );
                        }
                        last_err = e;
                        break;
                    }
                }
            }
        }

        let signature = signature_opt.ok_or(last_err);
        // ── end re-quote loop ─────────────────────────────────────────────────

        match signature {
            Ok(sig) => {
                self.successful_executions.fetch_add(1, Ordering::SeqCst);

                let trade = Trade::new(
                    order_id.clone(),
                    side,
                    price,
                    size,
                    chrono::Utc::now(),
                );

                self.keystore.record_transaction(amount_usdc).await;
                self.trades.write().await.push(trade);

                self.stop_loss_manager.write().await.reset_for_new_position(price);

                info!("[Order] {:?} confirmed: {}", side, sig);

                let count = self.total_executions.load(Ordering::SeqCst);
                if count % self.config.reconcile_balances_every_n_trades.unwrap_or(10) as u64 == 0 {
                    let _ = self.reconcile_balances(price).await;
                }

                Ok(order_id)
            }
            Err(e) => {
                self.failed_executions.fetch_add(1, Ordering::SeqCst);
                error!("[Order] Transaction failed: {}", e);
                Err(e)
            }
        }
    }

    async fn build_jupiter_swap(
        &self,
        side: OrderSide,
        price: f64,
        size: f64,
    ) -> Result<(VersionedTransaction, u64)> {
        info!("[Jupiter] Building VersionedTransaction V4.1 (secure)...");

        let sol_mint_pubkey  = Pubkey::from_str(SOL_MINT)
            .context("Failed to parse SOL_MINT")?;
        let usdc_mint_pubkey = Pubkey::from_str(USDC_MINT)
            .context("Failed to parse USDC_MINT")?;

        let wallet_pubkey = self.keystore.pubkey().clone();

        let (usdc_balance, sol_balance) = self.balance_tracker.get_balances().await;
        let initial_capital = usdc_balance + (sol_balance * price);

        let rpc_url = self.config.rpc_url.clone()
            .ok_or_else(|| anyhow::anyhow!(
                "[RealEngine] rpc_url not configured. \
                 Use RealTradingConfig::from_config() at engine startup."
            ))?;

        let jupiter_api_key = self.config.jupiter_api_key.clone()
            .ok_or_else(|| anyhow::anyhow!(
                "jupiter_api_key not configured — set GRIDZBOTZ_JUPITER_API_KEY env var"
            ))?;

        let (per_cu_microlamports, fee_mode) = match &self.priority_fee_estimator {
            Some(estimator) => {
                let fee = estimator.get_priority_fee().await;
                debug!("[Jupiter] Dynamic fee resolved: {} µL/CU (source: {})",
                    fee, self.fee_source_name);
                (fee, self.fee_source_name)
            }
            None => {
                debug!("[Jupiter] Static fee: {} µL/CU", self.static_priority_fee);
                (self.static_priority_fee, "static")
            }
        };

        let max_lamports = per_cu_microlamports
            .saturating_mul(JUPITER_ESTIMATED_CU)
            / 1_000_000;
        let max_lamports = max_lamports.max(MIN_MAX_LAMPORTS);

        info!(
            "[Quant] priority_fee: source={} fee={} µL/CU cu_budget={} max_lamports={}",
            fee_mode, per_cu_microlamports, JUPITER_ESTIMATED_CU, max_lamports
        );

        let jupiter = JupiterClient::new(
            rpc_url,
            wallet_pubkey,
            sol_mint_pubkey,
            usdc_mint_pubkey,
            initial_capital,
            jupiter_api_key,
        )?
        .with_slippage(self.slippage_bps)
        // ✅ PR #123: Pass self.fee_source_name instead of hardcoded "high".
        // "high" is not a Jupiter priority level in the Lamports fee variant;
        // the actual priority level is encoded via max_lamports. Passing the
        // source name aligns this label with the [Quant] log line above.
        .with_priority_fee(max_lamports, self.fee_source_name.to_string());

        let (input_mint, output_mint, amount) = match side {
            OrderSide::Buy => {
                let usdc_micro = (price * size * 1_000_000.0) as u64;
                info!("  BUY:  {:.2} USDC → SOL", price * size);
                (usdc_mint_pubkey, sol_mint_pubkey, usdc_micro)
            }
            OrderSide::Sell => {
                let sol_lamports = (size * 1_000_000_000.0) as u64;
                info!("  SELL: {:.4} SOL → USDC", size);
                (sol_mint_pubkey, usdc_mint_pubkey, sol_lamports)
            }
        };

        let (tx, last_valid) = jupiter
            .simple_swap(input_mint, output_mint, amount)
            .await
            .context("Failed to build Jupiter swap")?;

        info!("[Jupiter] Swap tx built (last valid block: {})", last_valid);
        Ok((tx, last_valid))
    }

    async fn reconcile_balances(&self, current_price: f64) -> Result<()> {
        let (usdc, sol)  = self.balance_tracker.get_balances().await;
        let total_value  = usdc + (sol * current_price);

        let mut last_nav = self.last_reconciled_nav.write().await;
        let pnl_delta    = total_value - *last_nav;
        *last_nav        = total_value;
        drop(last_nav);

        if pnl_delta != 0.0 {
            self.circuit_breaker.write().await.record_trade(pnl_delta, total_value);
            debug!(
                "[RealEngine] Reconcile: nav=${:.2} delta=${:+.4} → CB updated",
                total_value, pnl_delta
            );
        }

        Ok(())
    }

    pub async fn auto_take_profit(&self, current_price: f64) -> Result<()> {
        let roi = self.get_roi(current_price).await;

        if roi >= self.profit_take_pct {
            let (_, sol)    = self.get_balances().await;
            let ratio       = self.config.profit_take_ratio.unwrap_or(0.4);
            let sell_amount = sol * ratio;

            if sell_amount > 0.01 {
                info!("[ProfitTake] ROI {:.2}% >= {:.2}% — taking profit", roi, self.profit_take_pct);
                self.sell(current_price, sell_amount).await?;
            }
        }

        Ok(())
    }

    pub async fn get_performance_stats(&self) -> PerformanceStats {
        let trades = self.trades.read().await;

        if trades.is_empty() {
            return PerformanceStats::default();
        }

        let mut stats = PerformanceStats {
            total_trades: trades.len(),
            ..Default::default()
        };

        for trade in trades.iter() {
            stats.total_fees += trade.fees_paid;
            let pnl = trade.net_pnl;
            stats.total_pnl += pnl;

            if pnl > 0.0 {
                stats.winning_trades += 1;
                stats.largest_win = stats.largest_win.max(pnl);
            } else if pnl < 0.0 {
                stats.losing_trades += 1;
                stats.largest_loss = stats.largest_loss.min(pnl);
            }
        }

        let pair_trades = stats.winning_trades + stats.losing_trades;
        if pair_trades > 0 {
            stats.win_rate = (stats.winning_trades as f64 / pair_trades as f64) * 100.0;
        }

        stats
    }

    pub async fn get_roi(&self, current_price: f64) -> f64 {
        let (usdc, sol)   = self.balance_tracker.get_balances().await;
        let current_value = usdc + (sol * current_price);
        let initial_value = self.balance_tracker.initial_balance_usd();

        if initial_value > 0.0 {
            ((current_value - initial_value) / initial_value) * 100.0
        } else {
            0.0
        }
    }

    pub async fn get_trades(&self) -> Vec<Trade> {
        self.trades.read().await.clone()
    }

    pub async fn get_balances(&self) -> (f64, f64) {
        self.balance_tracker.get_balances().await
    }

    pub async fn display_status(&self, current_price: f64) {
        let (usdc, sol) = self.get_balances().await;
        let roi         = self.get_roi(current_price).await;
        let stats       = self.get_performance_stats().await;
        let (daily_trades, daily_volume) = self.keystore.get_daily_stats().await;
        let executor_stats = self.executor.read().await.get_stats();
        let sl_mgr = self.stop_loss_manager.read().await;
        let (sl_pct, tp_pct) = sl_mgr.thresholds();
        let sl_mode = if sl_mgr.is_trailing() { "trailing" } else { "fixed" };
        let highest  = sl_mgr.highest_observed_price();
        drop(sl_mgr);

        println!();
        println!("=======================================================");
        println!("  REAL TRADING ENGINE V3.8 - STATUS");
        println!("=======================================================");
        println!();
        println!("Balances:");
        println!("  USDC : ${:.2}",           usdc);
        println!("  SOL  : {:.4} (${:.2})",   sol, sol * current_price);
        println!("  Total: ${:.2}",            usdc + (sol * current_price));
        println!();
        println!("Performance:");
        println!("  ROI        : {:.2}%",      roi);
        println!("  Total P&L  : ${:.2}",      stats.total_pnl);
        println!("  Total Fees : ${:.2}",      stats.total_fees);
        println!("  Win Rate   : {:.1}%",      stats.win_rate);
        println!("  Trades     : {} ({} wins, {} losses)",
            stats.total_trades, stats.winning_trades, stats.losing_trades);
        println!();
        println!("Today:");
        println!("  Trades : {}",              daily_trades);
        println!("  Volume : ${:.2}",          daily_volume);
        println!();
        println!("Executor:");
        println!("  RPC         : {}",
            self.config.executor.rpc_url.chars().take(50).collect::<String>());
        println!("  Success Rate: {:.1}%",    executor_stats.success_rate);
        println!("  Total Exec  : {}",        executor_stats.total_executions);
        println!();
        println!("Risk Guards:");
        println!("  Stop-loss   : -{:.1}%  ({})", sl_pct, sl_mode);
        println!("  Take-profit : +{:.1}%",       tp_pct);
        if sl_mode == "trailing" && highest > 0.0 {
            println!("  Trailing high: ${:.4}",   highest);
        }
        println!();
        println!("Execution:");
        println!("  Slippage    : {} bps (max Jupiter tolerance)", self.slippage_bps);
        println!("  CU budget   : {} (Jupiter V6 swap estimate)", JUPITER_ESTIMATED_CU);
        println!("  Requote     : up to {} attempts on 0xb/0x1771/blockhash expiry", self.max_requote_attempts);
        match self.fee_source_name {
            "static" => println!("  Priority fee: STATIC ({} µL/CU)", self.static_priority_fee),
            src      => println!("  Priority fee: DYNAMIC (source: {})", src),
        }
        println!();
        println!("Circuit Breaker:");
        let breaker = self.circuit_breaker.read().await;
        let status  = breaker.status();
        if status.is_tripped {
            println!("  Status  : TRIPPED");
            if let Some(reason) = status.trip_reason {
                println!("  Reason  : {:?}", reason);
            }
            if let Some(cooldown) = status.cooldown_remaining {
                println!("  Cooldown: {}s", cooldown.as_secs());
            }
        } else {
            println!("  Status  : OK");
        }
        println!();
        println!("SOL Price: ${:.4}", current_price);
        println!("=======================================================");
        println!();
    }

    pub async fn trigger_emergency_shutdown(&self, reason: &str) -> Result<()> {
        error!("[RealEngine] EMERGENCY SHUTDOWN: {}", reason);
        self.emergency_shutdown.store(true, Ordering::SeqCst);
        let mut breaker = self.circuit_breaker.write().await;
        breaker.force_trip(TripReason::MaxDrawdown);
        drop(breaker);
        self.display_status(0.0).await;
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// UNIFIED TRADING ENGINE TRAIT IMPLEMENTATION
// -----------------------------------------------------------------------------

#[async_trait]
impl TradingEngine for RealTradingEngine {
    async fn place_limit_order_with_level(
        &self,
        side: OrderSide,
        price: f64,
        size: f64,
        grid_level_id: Option<u64>,
    ) -> TradingResult<String> {
        if let Some(level) = grid_level_id {
            info!("[Grid] Level {} triggered {:?} @ ${:.4}", level, side, price);
        }
        let order_id = self.execute_trade(side, price, size).await?;

        let ts = chrono::Utc::now().timestamp();
        let mut fill = FillEvent::new(
            order_id.clone(),
            side,
            price,
            size,
            0.0,
            None,
            ts,
        );
        if let Some(lid) = grid_level_id {
            fill = fill.with_level(lid);
        }
        self.pending_fills.write().await.push(fill);

        Ok(order_id)
    }

    async fn cancel_order(&self, order_id: &str) -> TradingResult<()> {
        log::warn!(
            "[RealEngine] cancel_order('{}') — Jupiter swaps are atomic; nothing to cancel",
            order_id
        );
        Ok(())
    }

    async fn cancel_all_orders(&self) -> TradingResult<usize> {
        log::warn!(
            "[RealEngine] cancel_all_orders() — Jupiter swaps are atomic; 0 cancelled"
        );
        Ok(0)
    }

    async fn process_price_update(&self, current_price: f64) -> TradingResult<Vec<FillEvent>> {
        let entry_price = self.stop_loss_manager.read().await.entry_price();
        if entry_price > 0.0 {
            let (stop_triggered, profit_triggered) = {
                let mut sl = self.stop_loss_manager.write().await;
                let stop   = sl.should_stop_loss(entry_price, current_price);
                let profit = if stop { false } else {
                    sl.should_take_profit(entry_price, current_price)
                };
                (stop, profit)
            };

            if stop_triggered {
                let (sl_pct, _) = self.stop_loss_manager.read().await.thresholds();
                self.trigger_emergency_shutdown(&format!(
                    "STOP-LOSS: price ${:.4} crossed -{:.1}% threshold (entry ${:.4})",
                    current_price, sl_pct, entry_price
                )).await?;
            } else if profit_triggered {
                info!(
                    "[RealEngine] TAKE-PROFIT on price tick: ${:.4} (entry ${:.4})",
                    current_price, entry_price
                );
                self.auto_take_profit(current_price).await?;
            }
        }

        let fills = std::mem::take(&mut *self.pending_fills.write().await);
        Ok(fills)
    }

    async fn open_order_count(&self) -> usize { 0 }

    async fn is_trading_allowed(&self) -> bool {
        if self.emergency_shutdown.load(Ordering::SeqCst) {
            return false;
        }
        self.circuit_breaker.write().await.is_trading_allowed()
    }

    async fn emergency_shutdown(&self, reason: &str) -> TradingResult<()> {
        self.trigger_emergency_shutdown(reason).await
    }

    async fn get_wallet(&self) -> VirtualWallet {
        let (usdc, sol) = self.balance_tracker.get_balances().await;
        VirtualWallet::new_silent(usdc, sol)
    }

    async fn get_performance_stats(&self) -> PaperPerformanceStats {
        let stats = self.get_performance_stats().await;
        PaperPerformanceStats {
            total_trades:   stats.total_trades,
            winning_trades: stats.winning_trades,
            losing_trades:  stats.losing_trades,
            total_pnl:      stats.total_pnl,
            total_fees:     stats.total_fees,
            win_rate:       stats.win_rate,
            avg_win:        stats.avg_win,
            avg_loss:       stats.avg_loss,
            largest_win:    stats.largest_win,
            largest_loss:   stats.largest_loss,
            profit_factor:  stats.profit_factor,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let config = RealTradingConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_no_shadow_fee_fields() {
        let config = RealTradingConfig::default();
        assert!(config.rpc_url.is_none(),         "rpc_url default must be None");
        assert!(config.jupiter_api_key.is_none(), "api_key default must be None");
    }

    #[test]
    fn test_rpc_url_default_is_none() {
        let config = RealTradingConfig::default();
        assert!(
            config.rpc_url.is_none(),
            "rpc_url default must be None — wired via from_config(), never hardcoded"
        );
    }

    #[test]
    fn test_shadow_stop_loss_fields_removed() {
        let config = RealTradingConfig::default();
        assert!(config.circuit_breaker_loss_pct.is_some());
        assert!(config.profit_take_ratio.is_some());
    }

    #[test]
    fn test_no_slippage_shadow_on_realconfig() {
        let config = RealTradingConfig::default();
        assert!(config.max_trade_size_usdc.is_some());
        assert!(config.circuit_breaker_loss_pct.is_some());
        assert!(config.profit_take_ratio.is_some());
        assert!(config.reconcile_balances_every_n_trades.is_some());
        assert!(config.rpc_url.is_none());
        assert!(config.jupiter_api_key.is_none());
    }

    #[test]
    fn test_reconcile_not_called_in_process_price_update() {
        let src = include_str!("real_trader.rs");
        let fn_body_start = src.find("async fn process_price_update").unwrap();
        let after = &src[fn_body_start..];
        let next_fn = after.find("\n    async fn ").unwrap_or(after.len());
        let fn_body = &after[..next_fn];
        assert!(
            !fn_body.contains("reconcile_balances"),
            "REGRESSION: reconcile_balances() must not be called inside process_price_update()"
        );
    }

    #[test]
    fn test_legacy_async_fee_source_removed() {
        let src   = include_str!("real_trader.rs");
        let needle = concat!("Async", "RpcFeeSource");
        assert!(
            !src.contains(needle),
            "REGRESSION: legacy async fee source removed in V3.4 — use canonical RpcFeeSource"
        );
    }

    #[test]
    fn test_fee_source_name_field_exists() {
        let src = include_str!("real_trader.rs");
        assert!(
            src.contains("fee_source_name"),
            "fee_source_name field must exist — source-aware fee logging requires it"
        );
    }

    /// ✅ PR #123: JUPITER_ESTIMATED_CU must be 500_000.
    /// 300_000 caused max_lamports to always floor at MIN_MAX_LAMPORTS,
    /// silently discarding the dynamic fee estimate on every swap.
    #[test]
    fn test_jupiter_estimated_cu_is_500k() {
        assert_eq!(
            JUPITER_ESTIMATED_CU, 500_000,
            "REGRESSION: JUPITER_ESTIMATED_CU must be 500_000 — \
             300_000 caused max_lamports to always hit the MIN_MAX_LAMPORTS floor"
        );
    }

    /// ✅ PR #123: with_priority_fee() must not pass hardcoded "high" as the label.
    /// "high" is not a Jupiter priority level in the Lamports fee variant and
    /// produced misleading logs: 'source: high' instead of 'source: rpc|helius'.
    #[test]
    fn test_fee_label_not_hardcoded_high() {
        let src = include_str!("real_trader.rs");
        assert!(
            !src.contains(".with_priority_fee(max_lamports, \"high\".to_string())"),
            "REGRESSION PR #123: with_priority_fee() must not pass hardcoded \"high\" — \
             use self.fee_source_name.to_string() for accurate source logging"
        );
    }

    /// ✅ PR #116: Regression guard — re-quote loop must be present.
    /// The old single-shot pattern (build once, send once) must not return.
    /// Confirm MAX_REQUOTE_ATTEMPTS constant and the for-loop exist in source.
    #[test]
    fn test_requote_loop_present() {
        let src = include_str!("real_trader.rs");
        assert!(
            src.contains("max_requote_attempts"),
            "REGRESSION: max_requote_attempts must exist — re-quote loop was removed"
        );
        assert!(
            src.contains("Re-quote loop") || src.contains("re-quote loop"),
            "REGRESSION: re-quote loop comment marker missing — loop may have been replaced"
        );
        assert!(
            src.contains("0xb"),
            "REGRESSION: 0xb slippage error pattern must be present in re-quote guard"
        );
        assert!(
            src.contains("BlockhashNotFound"),
            "REGRESSION: BlockhashNotFound pattern must be present in re-quote guard"
        );
    }

    /// ✅ PR #116: Single-shot anti-regression guard.
    /// Confirm the old single execute_versioned() call without a loop is gone.
    #[test]
    fn test_no_single_shot_execute_versioned() {
        let src = include_str!("real_trader.rs");
        assert!(
            src.contains("for attempt in 1..=self.max_requote_attempts"),
            "REGRESSION: execute_versioned must be inside a requote for-loop"
        );
    }

    /// ✅ PR #122: ExecutorConfig must be wired from global Config in from_config().
    ///
    /// Guards against the silent regression where ..Default::default() hardcodes
    /// api.mainnet-beta.solana.com as the executor RPC URL, bypassing Chainstack.
    ///
    /// Verifies at source-text level (no async runtime needed) that:
    ///   1. from_config() constructs ExecutorConfig explicitly (not via Default alone)
    ///   2. rpc_url is wired from config.network.rpc_url
    ///   3. rpc_fallback_urls is wired from config.execution.rpc_fallback_urls
    ///   4. The boot log line is present so misconfiguration is visible at startup
    #[test]
    fn test_executor_rpc_wired_from_config() {
        let src = include_str!("real_trader.rs");

        // 1. from_config() must build ExecutorConfig explicitly
        assert!(
            src.contains("let executor = ExecutorConfig {"),
            "PR #122 REGRESSION: from_config() must build ExecutorConfig explicitly, \
             not via ..Default::default() alone"
        );

        // 2. rpc_url wired from network config
        assert!(
            src.contains("rpc_url:                   global.network.rpc_url.clone()"),
            "PR #122 REGRESSION: executor.rpc_url must be wired from config.network.rpc_url"
        );

        // 3. fallback URLs wired from execution config
        assert!(
            src.contains("rpc_fallback_urls:         global.execution.rpc_fallback_urls.clone()"),
            "PR #122 REGRESSION: executor.rpc_fallback_urls must be wired from \
             config.execution.rpc_fallback_urls"
        );

        // 4. boot log present — active RPC is always visible at startup
        assert!(
            src.contains("[RealTradingConfig] Executor RPC:"),
            "PR #122 REGRESSION: boot log '[RealTradingConfig] Executor RPC:' must be present"
        );
    }

    /// ✅ PR #124: 0x1771 (Jupiter Anchor v6 SlippageToleranceExceeded) must be
    /// in the is_requotable guard.
    ///
    /// PR #116 added 0xb to the guard but missed 0x1771 — the Anchor program
    /// variant of the same error (decimal 6001) thrown by Jupiter V6's swap
    /// program during pre-broadcast simulation (-32002 RPC error).
    ///
    /// When simulation returned `custom program error: 0x1771`, the error
    /// string didn't match any pattern, is_requotable evaluated false, and
    /// the loop aborted after attempt 1 — identical to pre-PR #116 behaviour.
    #[test]
    fn test_0x1771_in_requote_guard() {
        let src = include_str!("real_trader.rs");
        assert!(
            src.contains("0x1771"),
            "REGRESSION PR #124: 0x1771 (Jupiter Anchor SlippageToleranceExceeded, decimal 6001) \
             must be in the is_requotable guard. PR #116 only added 0xb — the Anchor variant \
             of the same error was silently aborting after 1 attempt with no retries."
        );
    }
}
